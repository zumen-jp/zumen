//! 復号と外部ファイル指定を、元のストリームと辞書の範囲で確かめる（7.3.8・7.6）。

use std::collections::BTreeSet;

use hayro_syntax::object::{Dict, MaybeRef, Object, ObjectIdentifier, Stream};
use hayro_syntax::reader::{Readable, Reader, ReaderContext, ReaderExt};
use zumen_base::{Cx, Kind, Stop};

#[derive(Clone, Copy, Default)]
pub(crate) struct Source<'a> {
    file: &'a [u8],
    encrypted: bool,
    largest_container: u64,
    file_refs: Option<&'a BTreeSet<(u32, i32)>>,
    table_bytes: u64,
}

impl<'a> Source<'a> {
    pub(crate) fn metadata_bound(self) -> u64 {
        (self.file.len() as u64).max(self.largest_container)
    }

    pub(super) fn metadata_table(self) -> u64 {
        self.table_bytes
    }

    pub(super) fn metadata_encrypted(self) -> bool {
        self.encrypted
    }

    pub(crate) fn new(file: &'a [u8], encrypted: bool, largest_container: u64) -> Self {
        Self {
            file,
            encrypted,
            largest_container,
            file_refs: None,
            table_bytes: 0,
        }
    }

    pub(crate) fn with_file_refs(
        mut self,
        refs: &'a BTreeSet<(u32, i32)>,
        table_bytes: u64,
    ) -> Self {
        self.file_refs = Some(refs);
        self.table_bytes = table_bytes;
        self
    }

    /// F の参照先を解かずに読むときの上限。相互参照表は、対象の元の位置を公開しない。
    pub(crate) fn file_ref_bound(self, id: ObjectIdentifier) -> Option<u64> {
        self.file_refs?
            .contains(&(id.obj_number, id.gen_number))
            .then(|| (self.file.len() as u64).max(self.largest_container))
    }

    pub(crate) fn file_refs(self) -> impl Iterator<Item = (u32, i32)> + 'a {
        self.file_refs.into_iter().flatten().copied()
    }

    pub(crate) fn reserve_tables(self, copies: u64, cx: &mut Cx<'_>) -> Result<(), Stop> {
        reserve_metadata(self.table_bytes.saturating_mul(copies), cx)
    }

    /// 相互参照の修復前後の 2 回、文字列の展開と復号の余りも含めて数える。
    pub(crate) fn reserve_scalar(self, bound: u64, cx: &mut Cx<'_>) -> Result<(), Stop> {
        reserve_metadata(bound.max(24).saturating_mul(16), cx)
    }

    /// 参照を調べるための直接の F。文字列は参照を持たず、辞書は表を作らず元の範囲を使う。
    pub(crate) fn direct_file_data<'b>(dict: &Dict<'b>) -> Option<&'b [u8]> {
        let MaybeRef::NotRef(Tail(tail)) = dict.get_raw::<Tail<'_>>(b"F")? else {
            return None;
        };
        if tail.starts_with(b"<<") {
            Reader::new(tail).skip::<Dict<'_>>(false)
        } else if matches!(tail.first(), Some(b'(' | b'<')) {
            Some(&[])
        } else {
            None
        }
    }

    /// raw_data() は暗号化した文書では復号後の Vec を作る。読む前に元のファイル側で長さを求める。
    /// Length だけは信じず、辞書・stream・指定長の直後の endstream が同じバイト列にあることを確かめる。
    /// 境界の空白・コメントが長い場合や壊れた Length では、上流の終端の探索を繰り返さず、残り全体を上限にする。
    pub(crate) fn ciphertext_bound(self, stream: &Stream<'_>) -> Option<u64> {
        if !self.encrypted {
            return None;
        }
        let dict = stream.dict();
        let bytes = dict.data();
        let end = bytes
            .as_ptr()
            .addr()
            .checked_sub(self.file.as_ptr().addr())
            .and_then(|n| n.checked_add(bytes.len()));
        let Some(tail) = end.and_then(|n| self.file.get(n..)) else {
            return Some((self.file.len() as u64).max(self.largest_container));
        };
        let header = tail.get(..64).unwrap_or(tail);
        let mut r = Reader::new(header);
        r.skip_white_spaces_and_comments();
        let data = r
            .forward_tag(b"stream")
            .and_then(|_| {
                r.forward_tag(b"\r\n")
                    .or_else(|| r.forward_tag(b"\r"))
                    .or_else(|| r.forward_tag(b"\n"))
            })
            .and_then(|_| tail.get(r.offset()..));
        let Some(data) = data else {
            return Some(tail.len() as u64);
        };
        if let Some(MaybeRef::NotRef(length)) = dict.get_raw::<u32>(b"Length")
            && let Some(after) = data.get(length as usize..)
        {
            let mut r = Reader::new(after.get(..64).unwrap_or(after));
            r.skip_white_spaces();
            if r.forward_tag(b"endstream").is_some() {
                return Some(u64::from(length));
            }
        }
        Some(data.len() as u64)
    }

    /// ASCII の確認と hayro の本来の展開で 2 回復号する。AES の 16 バイトずつ伸びる Vec も含める。
    pub(super) fn reserve_decryption(
        self,
        stream: &Stream<'_>,
        cx: &mut Cx<'_>,
    ) -> Result<(), Stop> {
        if let Some(bytes) = self.ciphertext_bound(stream) {
            let allocated = bytes.max(16).saturating_mul(4).saturating_mul(2);
            cx.check(Kind::Work, allocated.div_ceil(16))?;
            cx.check(Kind::DecodedBytes, allocated)?;
        }
        Ok(())
    }

    /// F が外部ファイルを表す場合、hayro は Filter の前に失敗する。文字列の中身を型の判定のために作らない。
    pub(super) fn external(self, dict: &Dict<'_>, cx: &mut Cx<'_>) -> Result<bool, Stop> {
        cx.check(Kind::Work, 1)?;
        match dict.get_raw::<Tail<'_>>(b"F") {
            None => Ok(false),
            Some(MaybeRef::NotRef(Tail(tail))) if matches!(tail.first(), Some(b'(' | b'<')) => {
                // この範囲は辞書の読み取りで既に値として飛ばせている。上流と同じ境界を確かめる。
                cx.check(Kind::Work, (tail.len() as u64).div_ceil(16))?;
                let mut r = Reader::new(tail);
                let Some(raw) = r.skip::<Object<'_>>(false) else {
                    return Ok(false);
                };
                let len = raw.len() as u64;
                if raw.starts_with(b"<<") {
                    // Object::read は辞書の後ろが stream なら Stream を読む。元の判定と同じ型を使う。
                    let names = names_bound(raw, cx)?;
                    let bytes = dict_bytes(len, names);
                    reserve_metadata(bytes, cx)?;
                    return Ok(matches!(
                        dict.get::<Object<'_>>(b"F"),
                        Some(Object::Dict(_))
                    ));
                }
                // 上流が実際に F の文字列を読む 1 回分。通常の非暗号のリテラルは元の列を借りる。
                let bytes = if raw.first() == Some(&b'<') {
                    len.div_ceil(2)
                } else if raw.iter().any(|b| matches!(b, b'\\' | b'\n' | b'\r')) {
                    len.max(24).saturating_mul(4)
                } else {
                    0
                };
                let decrypt = if self.encrypted {
                    len.max(16).saturating_mul(4)
                } else {
                    0
                };
                reserve_metadata(bytes.saturating_add(decrypt), cx)?;
                Ok(true)
            }
            Some(MaybeRef::NotRef(_)) => Ok(false),
            Some(MaybeRef::Ref(_)) => {
                // 上流の API は参照を解く前の対象範囲を公開しない。元のファイルか展開した ObjStm の最大長、
                // 辞書のキー、文字列を読む余りで上限を決め、確認と本来の展開を両方数えてから解く。
                let size = (self.file.len() as u64).max(self.largest_container);
                let names = names_bound(self.file, cx)?.max(self.largest_container);
                reserve_metadata(
                    dict_bytes(size, names).max(size.max(24).saturating_mul(16)),
                    cx,
                )?;
                Ok(matches!(
                    dict.get::<Object<'_>>(b"F"),
                    Some(Object::String(_) | Object::Dict(_))
                ))
            }
        }
    }
}

// 1 キー 176 バイト（guard.rs）、最初の表、エスケープした名前と、確認・上流の 2 回の読み。
fn dict_bytes(size: u64, names: u64) -> u64 {
    names
        .saturating_mul(176)
        .saturating_add(2048)
        .saturating_add(size.saturating_mul(4))
        .saturating_mul(2)
}

fn reserve_metadata(bytes: u64, cx: &mut Cx<'_>) -> Result<(), Stop> {
    cx.check(Kind::Work, bytes.div_ceil(16))?;
    cx.check(Kind::DecodedBytes, bytes)
}

fn names_bound(bytes: &[u8], cx: &mut Cx<'_>) -> Result<u64, Stop> {
    let mut names = 0;
    for chunk in bytes.chunks(256) {
        cx.check(Kind::Work, (chunk.len() as u64).div_ceil(16))?;
        names += chunk.iter().filter(|&&b| b == b'/').count() as u64;
    }
    Ok(names)
}

struct Tail<'a>(&'a [u8]);

impl<'a> Readable<'a> for Tail<'a> {
    fn read(r: &mut Reader<'a>, _: &ReaderContext<'a>) -> Option<Self> {
        r.tail().map(Tail)
    }
}
