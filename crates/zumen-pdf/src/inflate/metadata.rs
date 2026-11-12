//! 展開引数の表・値・段配列を、上流が解析する前に予約する（7.3・7.4、#776）。

use hayro_syntax::object::{Array, Dict, MaybeRef, Name};
use hayro_syntax::reader::{Readable, Reader, ReaderContext};
use zumen_base::{Cx, Kind, Stop};

use super::{Source, allocation};

/// 64 ビットの辞書の初期表・Arc・参照の包装も含む。キーの伸長は別に数える。
const FIXED: u64 = 2048;
/// guard と同じ、辞書のキーと表の全伸長の上界。
const KEY: u64 = 176;

#[derive(Clone, Copy)]
pub(super) struct Metadata<'a> {
    source: Source<'a>,
    copies: u64,
    file_name: bool,
}

impl<'a> Metadata<'a> {
    pub(super) fn new(source: Source<'a>, copies: u64) -> Self {
        Self {
            source,
            copies,
            file_name: false,
        }
    }

    pub(super) fn decoding(source: Source<'a>) -> Self {
        Self {
            source,
            copies: if source.metadata_encrypted() { 5 } else { 3 },
            file_name: true,
        }
    }

    pub(super) fn name<'b>(
        self,
        dict: &Dict<'b>,
        key: &[u8],
        cx: &mut Cx<'_>,
    ) -> Result<Option<Name<'b>>, Stop> {
        lookup(dict, cx)?;
        if let Some(raw) = dict.get_raw::<Tail<'_>>(key) {
            let bytes = match raw {
                MaybeRef::Ref(_) => self.reference(Reference::Name),
                MaybeRef::NotRef(Tail(tail)) => {
                    if tail.first() == Some(&b'/') {
                        (tail.len() as u64).saturating_mul(4)
                    } else {
                        0
                    }
                }
            };
            // decoded_image は、外部ファイルの判定でも F を Object として読む。
            // F が所有する Name なら、引数解析の 3/5 回とは別にもう 1 回確保する。
            // plain_flate の診断は decoded_image を呼ばないので追加しない。
            let reservation = Self {
                copies: self
                    .copies
                    .saturating_add(u64::from(self.file_name && key == b"F")),
                ..self
            };
            reservation.reserve(bytes, bytes, cx)?;
        }
        Ok(dict.get(key))
    }

    pub(super) fn dict<'b>(
        self,
        dict: &Dict<'b>,
        key: &[u8],
        cx: &mut Cx<'_>,
    ) -> Result<Option<Dict<'b>>, Stop> {
        lookup(dict, cx)?;
        if let Some(raw) = dict.get_raw::<Tail<'_>>(key) {
            let (bytes, read) = match raw {
                MaybeRef::Ref(_) => (
                    self.reference(Reference::Dict),
                    self.source.metadata_bound(),
                ),
                MaybeRef::NotRef(Tail(tail)) => {
                    if !tail.starts_with(b"<<") {
                        self.reserve(FIXED, 1, cx)?;
                        return Ok(dict.get(key));
                    }
                    let stats = Stats::read(tail, cx)?;
                    // Dict::read は開始タグを確かめる前にも初期表を作る。
                    // skip の成功範囲に縮めず、失敗までに読む tail 全体を使う。
                    let bytes = stats
                        .names
                        .saturating_mul(KEY)
                        .saturating_add(stats.bytes.saturating_mul(4))
                        .saturating_add(FIXED)
                        .saturating_add(stats.refs.saturating_mul(self.value_reference()));
                    (bytes, stats.bytes)
                }
            };
            self.reserve(bytes, read, cx)?;
        }
        Ok(dict.get(key))
    }

    pub(super) fn array<'b>(
        self,
        dict: &Dict<'b>,
        key: &[u8],
        cx: &mut Cx<'_>,
    ) -> Result<Option<Array<'b>>, Stop> {
        lookup(dict, cx)?;
        if let Some(raw) = dict.get_raw::<Tail<'_>>(key) {
            let (bytes, read) = match raw {
                MaybeRef::Ref(_) => (FIXED, self.source.metadata_bound()),
                MaybeRef::NotRef(Tail(tail)) => (0, tail.len() as u64),
            };
            // Array::read は範囲を借り、項目をまだ実体化しない。参照の包装は先に予約する。
            self.reserve(bytes, read, cx)?;
        }
        Ok(dict.get(key))
    }

    /// 名前の走査と、上流の Filter・Dict の SmallVec。Stage Vec の予約とは別。
    pub(super) fn names(self, array: &Array<'_>, cx: &mut Cx<'_>) -> Result<u64, Stop> {
        let stats = Stats::read(array.data(), cx)?;
        // 名前は /、参照は数の後ろの R を必ず消費する。文字列などの中も数える。
        // guard は実際の段を 64 個までに制限するが、ここでは生の上界を切り詰めない。
        let count = stats.names.saturating_add(stats.refs);
        let bytes = stats
            .bytes
            .saturating_mul(4)
            .saturating_add(stats.refs.saturating_mul(self.reference(Reference::Name)))
            .saturating_add(count.saturating_mul(64));
        self.reserve(bytes, stats.bytes, cx)?;
        Ok(count)
    }

    /// Object の失敗は Reader を戻すため、同じ項目を段ごとに再試行しうる。
    /// 1 個の skip 範囲や既知フィルターの数だけから、引数の確保を決めない。
    pub(super) fn objects(
        self,
        array: &Array<'_>,
        attempts: u64,
        cx: &mut Cx<'_>,
    ) -> Result<(), Stop> {
        if attempts == 0 {
            return cx.check(Kind::Work, 0);
        }
        let stats = Stats::read(array.data(), cx)?;
        // Object の直接 stream は、Dict と Stream で同じ辞書を 2 回読む。
        let local = stats
            .names
            .saturating_mul(KEY)
            .saturating_mul(2)
            .saturating_add(stats.bytes.saturating_mul(8))
            .saturating_add(FIXED.saturating_mul(2))
            .saturating_add(stats.refs.saturating_mul(self.value_reference()));
        // 参照の解析が成功すれば、解決が失敗しても Reader は先へ進む。
        // 対象の確保を attempts 倍にはしない。R の候補の数は解く回数以上になる。
        let targets = stats.refs.saturating_mul(self.reference(Reference::Object));
        self.reserve(
            local.saturating_mul(attempts).saturating_add(targets),
            stats
                .bytes
                .saturating_mul(attempts)
                .saturating_add(self.source.metadata_bound().saturating_mul(stats.refs)),
            cx,
        )
    }

    fn value_reference(self) -> u64 {
        // Stream Length と予測子の整数を引く参照、暗号化の Crypt Name を読む参照。
        // 後者は所有する Name を作りうる。値でない場所の参照候補も数える。
        FIXED.saturating_add(if self.source.metadata_encrypted() {
            self.source.metadata_bound().saturating_mul(4)
        } else {
            0
        })
    }

    fn reference(self, kind: Reference) -> u64 {
        let bound = self.source.metadata_bound();
        let bytes = match kind {
            Reference::Name => bound.saturating_mul(4).saturating_add(FIXED),
            Reference::Dict => self.source.metadata_table().saturating_add(FIXED),
            Reference::Object => self
                .source
                .metadata_table()
                .saturating_mul(2)
                .saturating_add(bound.saturating_mul(8))
                .saturating_add(16 * FIXED),
        };
        // XRef は未修復なら同じ型をもう 1 回読む。参照の鎖を解くこととは別。
        bytes.saturating_mul(2)
    }

    fn reserve(self, bytes: u64, read: u64, cx: &mut Cx<'_>) -> Result<(), Stop> {
        let bytes = bytes.saturating_mul(self.copies);
        let work = bytes.div_ceil(16).saturating_add(
            read.div_ceil(16)
                .saturating_mul(8)
                .saturating_mul(self.copies),
        );
        allocation::reserve((work, bytes), cx)
    }
}

enum Reference {
    Name,
    Dict,
    Object,
}

#[derive(Default)]
struct Stats {
    bytes: u64,
    names: u64,
    refs: u64,
}

impl Stats {
    fn read(bytes: &[u8], cx: &mut Cx<'_>) -> Result<Self, Stop> {
        let mut stats = Self {
            bytes: bytes.len() as u64,
            ..Self::default()
        };
        let mut last = None;
        for chunk in bytes.chunks(256) {
            cx.check(Kind::Work, (chunk.len() as u64).div_ceil(16))?;
            stats.names = stats
                .names
                .saturating_add(chunk.iter().filter(|&&b| b == b'/').count() as u64);
            for &byte in chunk {
                // ObjRef::read は世代の符号だけも 0 と読む。直前の空白は 0 個でもよい。
                // 数字の read/skip の区切りを確定せず、名前や文字列内の候補も残す。
                if byte == b'R'
                    && last
                        .is_some_and(|b: u8| b.is_ascii_digit() || matches!(b, b'.' | b'-' | b'+'))
                {
                    stats.refs = stats.refs.saturating_add(1);
                }
                if !matches!(byte, 0 | 9..=13 | 32) {
                    last = Some(byte);
                }
            }
        }
        Ok(stats)
    }
}

struct Tail<'a>(&'a [u8]);

fn lookup(dict: &Dict<'_>, cx: &mut Cx<'_>) -> Result<(), Stop> {
    // get_raw の MaybeRef も、長い数を ObjRef として試す。表や値を作らない最初の取得も先に数える。
    cx.check(
        Kind::Work,
        1 + (dict.data().len() as u64).div_ceil(16).saturating_mul(4),
    )
}

impl<'a> Readable<'a> for Tail<'a> {
    fn read(reader: &mut Reader<'a>, _: &ReaderContext<'a>) -> Option<Self> {
        reader.tail().map(Tail)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hayro_syntax::object::ObjRef;
    use hayro_syntax::reader::{Reader, ReaderExt};
    use zumen_base::{Budget, CancelToken, Diagnostics, Limit, Limits};

    #[test]
    fn short_f_key_covers_the_extra_object_name_read_at_capacity_boundaries() {
        // allocator oracle の stages + 実 decoded_data の観測値。
        // 未知名は Stage を作らないので、その Vec の余りでは追加の読みを覆えない。
        for (length, allocated) in [(4097, 65_408), (8193, 130_944)] {
            let source = format!("<< /F /#41{} >>", "a".repeat(length - 1));
            let context = ReaderContext::dummy();
            let dict = Reader::new(source.as_bytes())
                .read_with_context::<Dict<'_>>(&context)
                .unwrap();
            let mut budget = Budget::default();
            let mut diagnostics = Diagnostics::new();
            let cancel = CancelToken::new();
            let stages = super::super::stages::read(
                &dict,
                Metadata::decoding(Source::default()),
                &mut Cx::new(&mut budget, &mut diagnostics, &cancel),
            )
            .unwrap();
            assert!(stages.is_empty());
            assert!(budget.used(Kind::DecodedBytes) >= allocated);
        }
    }

    #[test]
    fn every_reference_read_by_hayro_is_counted_before_resolution() {
        let mut budget = Budget::default();
        let mut diagnostics = Diagnostics::new();
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
        let alphabet = b"01+-. \0";
        let mut accepted = 0;
        for length in 1..=6 {
            for mut value in 0..alphabet.len().pow(length) {
                let mut bytes = b"1 ".to_vec();
                for _ in 0..length {
                    bytes.push(alphabet[value % alphabet.len()]);
                    value /= alphabet.len();
                }
                bytes.push(b'R');
                if Reader::new(&bytes)
                    .read_without_context::<ObjRef>()
                    .is_some()
                {
                    accepted += 1;
                    assert!(Stats::read(&bytes, &mut cx).unwrap().refs >= 1, "{bytes:?}");
                }
            }
        }
        assert!(accepted > 1000);
        assert_eq!(Stats::read(b"/RL /Rows 3 /RGB", &mut cx).unwrap().refs, 0);
        // 256 バイトの走査の境目をまたいでも、数の末尾を失わない。
        let mut split = vec![b' '; 254];
        split.extend_from_slice(b"1 0 R");
        assert_eq!(Stats::read(&split, &mut cx).unwrap().refs, 1);
    }

    #[test]
    fn opaque_reference_bounds_saturate_and_stop_before_typed_lookup() {
        let context = ReaderContext::dummy();
        let dict = Reader::new(b"<< /Filter 1 0 R >>")
            .read_with_context::<Dict<'_>>(&context)
            .unwrap();
        for kind in [Kind::Work, Kind::DecodedBytes] {
            let limit = if kind == Kind::Work { 1024 } else { 8 };
            let mut budget = Budget::new(
                Limits::default()
                    .with(Kind::Work, Limit::Unlimited)
                    .with(Kind::DecodedBytes, Limit::Unlimited)
                    .with(kind, Limit::Finite(limit)),
            );
            let mut diagnostics = Diagnostics::new();
            let cancel = CancelToken::new();
            let metadata = Metadata::new(Source::new(&[], false, u64::MAX), 5);
            let result = metadata.name(
                &dict,
                b"Filter",
                &mut Cx::new(&mut budget, &mut diagnostics, &cancel),
            );
            assert!(
                matches!(result, Err(Stop::Exceeded(e)) if e.kind == kind && e.requested > u64::from(u32::MAX))
            );
            assert!(budget.used(kind) <= limit);
        }
    }
}
