//! 復号と外部ファイル指定を、元のストリームと辞書の範囲で確かめる（7.3.8・7.6）。

use std::collections::BTreeSet;

use hayro_syntax::object::{Dict, MaybeRef, Object, ObjectIdentifier, Stream};
use hayro_syntax::reader::{Readable, Reader, ReaderContext, ReaderExt};
use zumen_base::{Cx, Kind, Stop};

use super::metadata::Metadata;

#[derive(Clone, Copy, Default)]
pub(crate) struct Source<'a> {
    file: &'a [u8],
    encrypted: bool,
    largest_container: u64,
    file_refs: Option<&'a BTreeSet<(u32, i32)>>,
    table_bytes: u64,
}

impl<'a> Source<'a> {
    pub(super) fn metadata_bound(self) -> u64 {
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
        // 暗号化した空文字列でも、2 回の鍵生成には固定の確保と手間がある。
        let fixed = if self.encrypted { 4096 } else { 0 };
        if self.encrypted {
            cx.check(Kind::Work, bound.saturating_mul(8).saturating_add(fixed))?;
        }
        reserve_metadata(bound.max(24).saturating_mul(16).saturating_add(fixed), cx)
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

    /// raw_data が読む Type の文字列、本文の Vec の全伸長、鍵生成の固定費を先に数える。
    /// 単独圧縮・系列は確認と本来の展開の 2 回、ほかの段は本来の 1 回。診断の再読も別に渡す。
    pub(crate) fn reserve_raw(
        self,
        stream: &Stream<'_>,
        copies: u64,
        cx: &mut Cx<'_>,
    ) -> Result<(), Stop> {
        if self.encrypted {
            // ciphertext_bound の Length の型付き取得と、固定長の境界確認も予約より先に行わない。
            cx.check(
                Kind::Work,
                32 + (stream.dict().data().len() as u64)
                    .div_ceil(16)
                    .saturating_mul(4),
            )?;
        }
        if let Some(bytes) = self.ciphertext_bound(stream) {
            Metadata::new(self, copies).reserve_string_key(stream.dict(), b"Type", cx)?;
            // RC4/AES の object key と MD5 の一時 Vec も含める。AES-CBC は 16 バイトずつ
            // Vec を伸ばすため、出力長だけでなく全確保を数える。Type が文字列 XRef の
            // 場合や Identity でも、ここでは復号しうる上限を予約し、値を読む回数を増やさない。
            let allocated = bytes
                .max(16)
                .saturating_mul(4)
                .saturating_add(2048)
                .saturating_mul(copies);
            cx.check(
                Kind::Work,
                bytes
                    .saturating_mul(4)
                    .saturating_add(2048)
                    .saturating_mul(copies),
            )?;
            cx.check(Kind::DecodedBytes, allocated)?;
        }
        Ok(())
    }

    /// F が外部ファイルを表す場合、hayro は Filter の前に失敗する。文字列の中身を型の判定のために作らない。
    pub(super) fn external(self, dict: &Dict<'_>, cx: &mut Cx<'_>) -> Result<bool, Stop> {
        // get_raw 自体も、長い数を ObjRef として試す。
        cx.check(
            Kind::Work,
            1 + (dict.data().len() as u64).div_ceil(16).saturating_mul(4),
        )?;
        match dict.get_raw::<Tail<'_>>(b"F") {
            None => Ok(false),
            Some(MaybeRef::NotRef(Tail(tail))) if matches!(tail.first(), Some(b'(' | b'<')) => {
                // Object が辞書を Stream として再読する分も含め、skip の成功範囲に縮めない。
                let dictionary = tail.starts_with(b"<<");
                Metadata::new(self, if dictionary { 2 } else { 1 })
                    .reserve_object_key(dict, b"F", cx)?;
                if dictionary {
                    return Ok(matches!(
                        dict.get::<Object<'_>>(b"F"),
                        Some(Object::Dict(_))
                    ));
                }
                // 直接の文字列は中身を作らず型を確かめる。上流が実体化する 1 回分は予約済み。
                cx.check(Kind::Work, (tail.len() as u64).div_ceil(16))?;
                let mut r = Reader::new(tail);
                Ok(r.skip::<Object<'_>>(false).is_some())
            }
            Some(MaybeRef::NotRef(Tail(tail))) => {
                // Name の所有は Metadata::decoding の追加 1 回分に含める。Array は範囲を借りる。
                cx.check(Kind::Work, (tail.len() as u64).div_ceil(16))?;
                Ok(false)
            }
            Some(MaybeRef::Ref(_)) => {
                // 確認と本来の展開の 2 回。型の再試行は Metadata が別に含める。
                Metadata::new(self, 2).reserve_object_key(dict, b"F", cx)?;
                Ok(matches!(
                    dict.get::<Object<'_>>(b"F"),
                    Some(Object::String(_) | Object::Dict(_))
                ))
            }
        }
    }
}

fn reserve_metadata(bytes: u64, cx: &mut Cx<'_>) -> Result<(), Stop> {
    cx.check(Kind::Work, bytes.div_ceil(16))?;
    cx.check(Kind::DecodedBytes, bytes)
}

struct Tail<'a>(&'a [u8]);

impl<'a> Readable<'a> for Tail<'a> {
    fn read(r: &mut Reader<'a>, _: &ReaderContext<'a>) -> Option<Self> {
        r.tail().map(Tail)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zumen_base::{Budget, CancelToken, Diagnostics, Limit, Limits};

    fn reserve(input: &[u8], copies: u64, limits: Limits) -> (Result<(), Stop>, Budget) {
        let stream = Reader::new(input)
            .read_with_context::<Stream<'_>>(&ReaderContext::dummy())
            .unwrap();
        let source = Source::new(input, true, u64::MAX);
        let mut budget = Budget::new(limits);
        let mut diagnostics = Diagnostics::new();
        let cancel = CancelToken::new();
        let result = source.reserve_raw(
            &stream,
            copies,
            &mut Cx::new(&mut budget, &mut diagnostics, &cancel),
        );
        (result, budget)
    }

    #[test]
    fn empty_ciphertext_still_reserves_key_allocations_for_each_read() {
        let input = b"<< /Length 0 >>\nstream\n\nendstream";
        for copies in [1, 2, 3] {
            let (result, budget) = reserve(input, copies, Limits::default());
            assert_eq!(result, Ok(()));
            // 実 raw_data の allocator 観測値は、本文が空でも RC4 229、AESV2 287 バイト。
            assert!(budget.used(Kind::DecodedBytes) >= 287 * copies);
            assert!(budget.used(Kind::Work) >= 2048 * copies);
        }
    }

    #[test]
    fn unknown_ciphertext_and_type_reference_bounds_saturate_before_reading() {
        for input in [
            b"<< >>\nstream\n\nendstream".as_slice(),
            b"<< /Length 0 /Type 1 0 R >>\nstream\n\nendstream",
        ] {
            for kind in [Kind::Work, Kind::DecodedBytes] {
                let limit = if kind == Kind::Work { 1024 } else { 8 };
                let limits = Limits::default()
                    .with(Kind::Work, Limit::Unlimited)
                    .with(Kind::DecodedBytes, Limit::Unlimited)
                    .with(kind, Limit::Finite(limit));
                let (result, budget) = reserve(input, u64::MAX, limits);
                assert!(
                    matches!(result, Err(Stop::Exceeded(e)) if e.kind == kind && e.requested > u64::from(u32::MAX))
                );
                assert!(budget.used(kind) <= limit);
            }
        }
    }
}
