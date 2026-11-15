//! 文書エンジン向けの ASCII DXF のタグ走査・1 レコード差分保持・全体保存（`docs/live.md`、#878）。
//!
//! 元のバイト列は [`zumen_live::SourceBytes`] の 16 KiB ページ圧縮のまま持ち、触れたレコードだけを
//! [`OverlayRecord`] として展開・保持する。タグの切り方と書き戻しの規則は [`super::DxfFile`] と
//! [`super::write_back`] と一致させる。

use core::ops::Range;
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::thread;

use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits, Stop};
use zumen_live::{RecordSlice, SourceBytes};

pub use super::parse_code;
use super::{TagEdit, parse_value};
use crate::db::Value;

/// タグの場所。位置はすべて入力の先頭（またはレコードスライスの先頭）からのバイト数
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Span {
    /// グループ・コード
    pub code: i16,
    /// グループ・コードの行の先頭
    pub start: u32,
    /// 値の行の先頭
    pub value: u32,
    /// 値の終わり（改行を除く）
    pub value_end: u32,
    /// 次のタグの先頭
    pub end: u32,
}

impl Span {
    /// 全オフセットに `base` を足した `Span` を返す（`u32` を超えるときは `None`）
    #[inline]
    pub const fn checked_shift(self, base: u32) -> Option<Self> {
        let Some(start) = self.start.checked_add(base) else {
            return None;
        };
        let Some(value) = self.value.checked_add(base) else {
            return None;
        };
        let Some(value_end) = self.value_end.checked_add(base) else {
            return None;
        };
        let Some(end) = self.end.checked_add(base) else {
            return None;
        };
        Some(Self {
            code: self.code,
            start,
            value,
            value_end,
            end,
        })
    }
}

/// 次のタグを読んだ結果
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Next {
    /// タグ
    Tag(Span),
    /// 入力の終わり
    End,
    /// 値の行がないか、グループ・コードを読めないか、4 GiB を超える位置
    Invalid,
}

/// 行の中身の終わりと、次の行の先頭。`pos` は入力の中にあること
#[inline]
fn line(src: &[u8], pos: usize) -> (usize, usize) {
    let rest = src.get(pos..).unwrap_or_default();
    let (mut end, next) = match rest.iter().position(|&b| b == b'\n') {
        Some(n) => (pos + n, pos + n + 1),
        None => (src.len(), src.len()),
    };
    if end > pos && src.get(end - 1) == Some(&b'\r') {
        end -= 1;
    }
    (end, next)
}

/// `pos` から始まるタグを 1 つ読む。`u32::MAX` を超えるオフセットは [`Next::Invalid`] を返す
pub fn next_tag(src: &[u8], pos: usize) -> Next {
    if pos >= src.len() {
        return Next::End;
    }
    let (code_end, value) = line(src, pos);
    if value >= src.len() {
        return Next::Invalid;
    }
    let Some(code) = parse_code(src.get(pos..code_end).unwrap_or_default()) else {
        return Next::Invalid;
    };
    let (value_end, end) = line(src, value);
    let (Ok(start), Ok(value), Ok(value_end), Ok(end)) = (
        u32::try_from(pos),
        u32::try_from(value),
        u32::try_from(value_end),
        u32::try_from(end),
    ) else {
        return Next::Invalid;
    };
    Next::Tag(Span {
        code,
        start,
        value,
        value_end,
        end,
    })
}

/// 入力を、レコードの先頭（グループ・コードが 0 のタグの先頭）で最大 `parts` 個の区間に分ける位置を返す。
/// 最初は `0`、最後は `src.len()`。行の偶奇は手前の改行の数から決める
pub fn split_points(src: &[u8], parts: usize) -> Vec<usize> {
    let n = src.len();
    let max_parts = (n / 4096).min(1024);
    let parts = parts.clamp(1, max_parts.max(1));
    if parts <= 1 {
        return vec![0, n];
    }
    let raw: Vec<usize> = (0..=parts)
        .map(|k| if k == parts { n } else { (n / parts) * k })
        .collect();
    let counts: Vec<usize> = if cfg!(target_arch = "wasm32") {
        raw.windows(2)
            .map(|pair| {
                let (Some(&a), Some(&b)) = (pair.first(), pair.get(1)) else {
                    return 0;
                };
                src.get(a..b)
                    .unwrap_or_default()
                    .iter()
                    .filter(|&&ch| ch == b'\n')
                    .count()
            })
            .collect()
    } else {
        thread::scope(|scope| {
            let handles: Vec<_> = raw
                .windows(2)
                .map(|pair| {
                    let part = pair
                        .first()
                        .zip(pair.get(1))
                        .and_then(|(&a, &b)| src.get(a..b))
                        .unwrap_or_default();
                    scope.spawn(move || part.iter().filter(|&&ch| ch == b'\n').count())
                })
                .collect();
            handles
                .into_iter()
                .map(|handle| handle.join().unwrap_or(0))
                .collect()
        })
    };
    let mut out = vec![0usize];
    let mut before = 0usize;
    for k in 1..parts {
        before = before.saturating_add(counts.get(k - 1).copied().unwrap_or(0));
        let at = raw.get(k).copied().unwrap_or(n);
        let (mut pos, number) = if at > 0 && src.get(at - 1) == Some(&b'\n') {
            (at, before)
        } else {
            (line(src, at).1, before.saturating_add(1))
        };
        // 値の行なら次のグループ・コードの行へ進む
        if number % 2 == 1 && pos < n {
            pos = line(src, pos).1;
        }
        let found = loop {
            match next_tag(src, pos) {
                Next::Tag(tag) if tag.code == 0 => break Some(pos),
                Next::Tag(tag) => pos = tag.end as usize,
                Next::End | Next::Invalid => break None,
            }
        };
        if let Some(pos) = found
            && out.last().is_some_and(|&last| pos > last)
        {
            out.push(pos);
        }
    }
    out.push(n);
    out
}

/// 展開済みのレコード区間 `rs` から、ファイル全体のオフセットに揃えた [`Span`] 列を読む
pub fn read_record_spans_slice(rs: &RecordSlice<'_>) -> Vec<Span> {
    let Ok(base32) = u32::try_from(rs.base) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut local_pos = 0usize;
    while local_pos < rs.bytes.len() {
        let Next::Tag(span) = next_tag(rs.bytes, local_pos) else {
            break;
        };
        local_pos = span.end as usize;
        let Some(shifted) = span.checked_shift(base32) else {
            break;
        };
        out.push(shifted);
    }
    out
}

/// [`SourceBytes`] の `range` からファイル全体のオフセットに揃えた [`Span`] 列を読む
pub fn read_record_spans(src: &SourceBytes, range: Range<usize>) -> Vec<Span> {
    src.with_range(range, |rs| read_record_spans_slice(&rs))
}

/// 展開済みのレコード区間 `rs` を `(種類名, 値列)` に復号する（先頭タグがグループ・コード 0 でないときは `None`）
pub fn decode_record_slice(rs: &RecordSlice<'_>) -> Option<(Vec<u8>, Vec<Value>)> {
    let spans = read_record_spans_slice(rs);
    let first = spans.first()?;
    if first.code != 0 {
        return None;
    }
    let kind = rs.slice(first.value, first.value_end).trim_ascii().to_vec();
    let values = spans
        .get(1..)
        .unwrap_or_default()
        .iter()
        .map(|s| {
            let raw = rs.slice(s.value, s.value_end);
            parse_value(kind.as_slice(), s.code, raw)
        })
        .collect();
    Some((kind, values))
}

/// [`SourceBytes`] の `range` にある 1 レコードを `(種類名, 値列)` に復号する
pub fn decode_record(src: &SourceBytes, range: Range<usize>) -> Option<(Vec<u8>, Vec<Value>)> {
    src.with_range(range, |rs| decode_record_slice(&rs))
}

/// 変更・追加されたレコードの 1 つのタグ
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OTag {
    /// グループ・コード
    pub code: i16,
    /// 元のバイト範囲（新しく足したタグでは使わない）
    pub span: Span,
    /// 書き換えた値（改行を除く）
    pub edited: Option<Vec<u8>>,
    /// 新しく足したタグか
    pub fresh: bool,
}

impl OTag {
    /// 既存の [`Span`] から未編集のタグを作る
    #[inline]
    pub const fn from_span(span: Span) -> Self {
        Self {
            code: span.code,
            span,
            edited: None,
            fresh: false,
        }
    }

    /// 新しく足すタグを作る
    #[inline]
    pub const fn added(code: i16, value: Vec<u8>) -> Self {
        Self {
            code,
            span: Span {
                code,
                start: 0,
                value: 0,
                value_end: 0,
                end: 0,
            },
            edited: Some(value),
            fresh: true,
        }
    }

    /// 元の全体バイト列 `src` から値の行のバイト列（改行を除く）を借りる
    #[inline]
    pub fn raw<'a>(&'a self, src: &'a [u8]) -> &'a [u8] {
        match &self.edited {
            Some(bytes) => bytes.as_slice(),
            None => src
                .get(self.span.value as usize..self.span.value_end as usize)
                .unwrap_or_default(),
        }
    }

    /// 展開済みのレコード区間から値の行のバイト列（改行を除く）を借りる
    #[inline]
    pub fn raw_in<'a>(&'a self, slice: &RecordSlice<'a>) -> &'a [u8] {
        match &self.edited {
            Some(bytes) => bytes.as_slice(),
            None => slice.slice(self.span.value, self.span.value_end),
        }
    }

    /// 一般レコードの規則で [`Value`] に変換する
    #[inline]
    pub fn to_value_in(&self, slice: &RecordSlice<'_>) -> Value {
        parse_value(b"", self.code, self.raw_in(slice))
    }

    /// レコードの種類 `kind` に合わせて [`Value`] に変換する（`DIMSTYLE` の 105 などの規則を含む）
    #[inline]
    pub fn to_value_for_in(&self, kind: &[u8], slice: &RecordSlice<'_>) -> Value {
        parse_value(kind, self.code, self.raw_in(slice))
    }

    /// 保持している追加ヒープバイト数
    #[inline]
    pub fn heap_bytes(&self) -> usize {
        self.edited.as_ref().map_or(0, Vec::capacity)
    }
}

/// 変更または追加された 1 レコードの差分保持
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OverlayRecord {
    /// `raw` の先頭バイトのファイル内オフセット
    pub base: u32,
    /// 元のレコードの未圧縮バイト列（`HEADER` や追加レコードでは空）
    pub raw: Box<[u8]>,
    /// 今のタグ列
    pub tags: Vec<OTag>,
    /// 追加レコードのときの挿入先（元のレコード番号。`None` なら呼び出し側の既定挿入位置を使う）
    pub insert_before: Option<u32>,
}

impl OverlayRecord {
    /// 既定挿入位置（`ENTITIES` の `ENDSEC` の前など）へ足す追加レコードを作る
    pub fn born(tags: Vec<OTag>) -> Self {
        Self {
            base: 0,
            raw: Box::new([]),
            tags,
            insert_before: None,
        }
    }

    /// 元の `before` 番目のレコードの直前へ足す追加レコードを作る（`TABLES`・`BLOCKS` への追加や複数レコード置換など）
    pub fn born_before(before: u32, tags: Vec<OTag>) -> Self {
        Self {
            base: 0,
            raw: Box::new([]),
            tags,
            insert_before: Some(before),
        }
    }

    /// 走査済みの [`Span`] 列から `raw` を複製せずに差分レコードを作る（`HEADER` など大きなレコード向け）
    pub fn from_spans(spans: &[Span]) -> Self {
        Self {
            base: 0,
            raw: Box::new([]),
            tags: spans.iter().copied().map(OTag::from_span).collect(),
            insert_before: None,
        }
    }

    /// 元のレコードのスライスから `raw` と `tags` を組む
    pub fn from_slice(rs: RecordSlice<'_>) -> Self {
        let tags = read_record_spans_slice(&rs)
            .into_iter()
            .map(OTag::from_span)
            .collect();
        Self {
            base: rs.base as u32,
            raw: rs.bytes.into(),
            tags,
            insert_before: None,
        }
    }

    /// 保持しているヒープバイト数
    pub fn heap_bytes(&self) -> usize {
        self.raw.len()
            + self.tags.capacity() * core::mem::size_of::<OTag>()
            + self.tags.iter().map(OTag::heap_bytes).sum::<usize>()
    }
}

/// [`OverlayRecord::tags`] に適用済みの 1 タグ編集（取り消し・やり直し用）
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AppliedTagOp {
    /// `at` 番目のタグの値を書き換えた
    Set {
        /// タグ番号
        at: usize,
        /// 変更前の `edited`
        before: Option<Vec<u8>>,
        /// 変更後の値
        after: Vec<u8>,
    },
    /// `at` 番目のタグを除いた
    Remove {
        /// タグ番号
        at: usize,
        /// 除いたタグ
        removed: OTag,
    },
    /// `at` 番目に新しいタグを差し込んだ
    Insert {
        /// タグ番号
        at: usize,
        /// 差し込んだタグ
        inserted: OTag,
    },
}

impl AppliedTagOp {
    /// この編集履歴が保持するバイト数
    pub fn bytes(&self) -> usize {
        core::mem::size_of::<Self>()
            + match self {
                Self::Set { before, after, .. } => {
                    before.as_ref().map_or(0, Vec::capacity) + after.capacity()
                }
                Self::Remove { removed, .. } => removed.heap_bytes(),
                Self::Insert { inserted, .. } => inserted.heap_bytes(),
            }
    }
}

/// [`TagEdit`] の列を `tags` にその場で当て、取り消し用の [`AppliedTagOp`] 列を返す。
/// 途中で範囲外の添字があれば、それまでの変更を巻き戻して `None` を返す。
pub fn apply_tag_edits(tags: &mut Vec<OTag>, edits: Vec<TagEdit>) -> Option<Vec<AppliedTagOp>> {
    let mut ops = Vec::with_capacity(edits.len());
    for edit in edits {
        match edit {
            TagEdit::Set { at, value } => {
                let Some(slot) = tags.get_mut(at) else {
                    revert_tag_ops(tags, &ops);
                    return None;
                };
                let before = slot.edited.replace(value.clone());
                ops.push(AppliedTagOp::Set {
                    at,
                    before,
                    after: value,
                });
            }
            TagEdit::Remove { at } => {
                if at >= tags.len() {
                    revert_tag_ops(tags, &ops);
                    return None;
                }
                let removed = tags.remove(at);
                ops.push(AppliedTagOp::Remove { at, removed });
            }
            TagEdit::Insert { at, code, value } => {
                if at > tags.len() {
                    revert_tag_ops(tags, &ops);
                    return None;
                }
                let inserted = OTag::added(code, value);
                tags.insert(at, inserted.clone());
                ops.push(AppliedTagOp::Insert { at, inserted });
            }
        }
    }
    Some(ops)
}

/// [`AppliedTagOp`] の列を逆順にたどって `tags` を変更前の状態に戻す
pub fn revert_tag_ops(tags: &mut Vec<OTag>, ops: &[AppliedTagOp]) {
    for op in ops.iter().rev() {
        match op {
            AppliedTagOp::Set { at, before, .. } => {
                if let Some(slot) = tags.get_mut(*at) {
                    slot.edited.clone_from(before);
                }
            }
            AppliedTagOp::Remove { at, removed } => {
                if *at <= tags.len() {
                    tags.insert(*at, removed.clone());
                }
            }
            AppliedTagOp::Insert { at, .. } => {
                if *at < tags.len() {
                    tags.remove(*at);
                }
            }
        }
    }
}

/// [`AppliedTagOp`] の列を順方向にたどって `tags` に再び当てる
pub fn replay_tag_ops(tags: &mut Vec<OTag>, ops: &[AppliedTagOp]) {
    for op in ops {
        match op {
            AppliedTagOp::Set { at, after, .. } => {
                if let Some(slot) = tags.get_mut(*at) {
                    slot.edited = Some(after.clone());
                }
            }
            AppliedTagOp::Remove { at, .. } => {
                if *at < tags.len() {
                    tags.remove(*at);
                }
            }
            AppliedTagOp::Insert { at, inserted } => {
                if *at <= tags.len() {
                    tags.insert(*at, inserted.clone());
                }
            }
        }
    }
}

/// 1 つの [`OTag`] を出力バイト列へ書く。
/// 新しく足したタグはグループ・コードを 3 桁右寄せで書き、既存のタグは前後の空白と改行をそのまま保つ
pub fn write_tag(out: &mut Vec<u8>, slice: &RecordSlice<'_>, tag: &OTag, newline: &[u8]) {
    match (&tag.edited, tag.fresh) {
        (Some(value), true) => {
            let _ = write!(out, "{:>3}", tag.code);
            out.extend_from_slice(newline);
            out.extend_from_slice(value);
            out.extend_from_slice(newline);
        }
        (Some(value), false) => {
            out.extend_from_slice(slice.slice(tag.span.start, tag.span.value));
            out.extend_from_slice(value);
            out.extend_from_slice(slice.slice(tag.span.value_end, tag.span.end));
        }
        (None, _) => out.extend_from_slice(slice.slice(tag.span.start, tag.span.end)),
    }
}

fn estimate_tag_bytes(tag: &OTag) -> u64 {
    match (&tag.edited, tag.fresh) {
        (Some(value), true) => (value.len() as u64).saturating_add(8),
        (Some(value), false) => {
            let head = tag.span.value.saturating_sub(tag.span.start) as u64;
            let tail = tag.span.end.saturating_sub(tag.span.value_end) as u64;
            head.saturating_add(value.len() as u64).saturating_add(tail)
        }
        (None, _) => tag.span.end.saturating_sub(tag.span.start) as u64,
    }
}

/// 予算と中断を確かめながら、[`SourceBytes`] とレコード差分から DXF 全体のバイト列を書き出す。
///
/// - `0..source_records` のうち `gone` にない未変更レコードは `src` からそのまま写す。
/// - `overlay` の `..source_records` にある変更レコードは、元の空白・改行を保ってタグごとに書き出す。
/// - `overlay` の `source_records..` にあり `gone` に含まれない追加レコードは、それぞれの
///   `insert_before.or(default_insert_before)` が指す元のレコードの直前へ作成順に差し込む。
pub fn save_dxf_overlay_with_cx(
    cx: &mut Cx<'_>,
    src: &SourceBytes,
    source_records: u32,
    record_range: impl Fn(u32) -> Range<usize>,
    overlay: &BTreeMap<u32, OverlayRecord>,
    gone: &BTreeSet<u32>,
    default_insert_before: Option<u32>,
) -> Result<Vec<u8>, Stop> {
    let mut insertions: BTreeMap<u32, Vec<&OverlayRecord>> = BTreeMap::new();
    for (&rec_id, ov) in overlay.range(source_records..) {
        if gone.contains(&rec_id) {
            continue;
        }
        if let Some(target) = ov.insert_before.or(default_insert_before) {
            insertions.entry(target).or_default().push(ov);
        }
    }

    let approx_cap = src.len().saturating_add(overlay.len().saturating_mul(64));
    cx.check(Kind::Work, 1)?;
    // 先に最小限の出力容量は実際の区間ごとに検査しつつ伸ばす（削除で短くなることもあるため初期確保は控えめにする）
    let init_cap = approx_cap.min(src.len().saturating_add(4096));
    cx.check(Kind::DecodedBytes, init_cap as u64)?;
    let mut out = Vec::with_capacity(init_cap);
    let mut accounted_bytes = init_cap as u64;
    let mut charge_out = |cx: &mut Cx<'_>, needed_total: u64| -> Result<(), Stop> {
        if needed_total > accounted_bytes {
            cx.check(Kind::DecodedBytes, needed_total - accounted_bytes)?;
            accounted_bytes = needed_total;
        }
        Ok(())
    };

    let mut reader = src.page_reader();
    let newline = src.newline();
    let empty_slice = RecordSlice {
        base: 0,
        bytes: &[],
    };
    let mut written = 0usize;
    let mut changed = overlay.range(..source_records).peekable();
    let mut removed = gone.range(..source_records).copied().peekable();
    let mut inserted = insertions.iter().peekable();

    loop {
        let stops = [
            changed.peek().map(|(record, _)| **record),
            removed.peek().copied(),
            inserted.peek().map(|(at, _)| **at),
        ];
        let Some(record) = stops.into_iter().flatten().min() else {
            break;
        };
        let raw_range = if record < source_records {
            record_range(record)
        } else {
            src.len()..src.len()
        };
        let start = raw_range.start.clamp(written, src.len());
        let end = raw_range.end.clamp(start, src.len());
        if start > written {
            let copy_len = start - written;
            cx.check(Kind::Work, ((copy_len / 16384) + 1) as u64)?;
            let next_total = (out.len() as u64).saturating_add(copy_len as u64);
            charge_out(cx, next_total)?;
            reader.append_range(&mut out, written..start);
            written = start;
        }

        if let Some((_, added_list)) = inserted.next_if(|(at, _)| **at == record) {
            for ov in added_list {
                cx.check(Kind::Work, (ov.tags.len() as u64).saturating_add(1))?;
                let est: u64 = ov.tags.iter().map(estimate_tag_bytes).sum();
                charge_out(cx, (out.len() as u64).saturating_add(est))?;
                for tag in &ov.tags {
                    write_tag(&mut out, &empty_slice, tag, newline);
                }
            }
        }

        let changed_ov = changed.next_if(|(at, _)| **at == record).map(|(_, ov)| ov);
        if removed.next_if_eq(&record).is_some() {
            cx.check(Kind::Work, 1)?;
            written = end;
        } else if let Some(ov) = changed_ov {
            cx.check(Kind::Work, (ov.tags.len() as u64).saturating_add(1))?;
            let est: u64 = ov.tags.iter().map(estimate_tag_bytes).sum();
            charge_out(cx, (out.len() as u64).saturating_add(est))?;
            if !ov.raw.is_empty() {
                let slice = RecordSlice {
                    base: ov.base as usize,
                    bytes: &ov.raw,
                };
                for tag in &ov.tags {
                    write_tag(&mut out, &slice, tag, newline);
                }
            } else {
                reader.with_range(start..end, |slice| {
                    for tag in &ov.tags {
                        write_tag(&mut out, &slice, tag, newline);
                    }
                });
            }
            written = end;
        }
    }

    if written < src.len() {
        let tail_len = src.len() - written;
        cx.check(Kind::Work, ((tail_len / 16384) + 1) as u64)?;
        let next_total = (out.len() as u64).saturating_add(tail_len as u64);
        charge_out(cx, next_total)?;
        reader.append_range(&mut out, written..src.len());
    }
    Ok(out)
}

/// 上限なしの簡易版として、[`SourceBytes`] とレコード差分から DXF 全体のバイト列を書き出す
pub fn save_dxf_overlay(
    src: &SourceBytes,
    source_records: u32,
    record_range: impl Fn(u32) -> Range<usize>,
    overlay: &BTreeMap<u32, OverlayRecord>,
    gone: &BTreeSet<u32>,
    default_insert_before: Option<u32>,
) -> Vec<u8> {
    let mut budget = Budget::new(
        Limits::default()
            .with(Kind::DecodedBytes, Limit::Unlimited)
            .with(Kind::Work, Limit::Unlimited),
    );
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
    save_dxf_overlay_with_cx(
        &mut cx,
        src,
        source_records,
        record_range,
        overlay,
        gone,
        default_insert_before,
    )
    .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use zumen_sdk::{Codec, Transact, Transaction};

    use super::*;
    use crate::dxf::{Dxf, diff_values};

    #[test]
    fn tags_strip_crlf_and_preserve_last_line_without_newline() {
        let src = b"  0\r\nLINE\r\n 10\n1.5";
        let Next::Tag(first) = next_tag(src, 0) else {
            panic!("first tag");
        };
        assert_eq!(
            (
                first.code,
                first.start,
                first.value,
                first.value_end,
                first.end
            ),
            (0, 0, 5, 9, 11)
        );
        let Next::Tag(second) = next_tag(src, 11) else {
            panic!("second tag");
        };
        assert_eq!(
            (second.code, second.value, second.value_end, second.end),
            (10, 15, 18, 18)
        );
        assert_eq!(next_tag(src, 18), Next::End);
        assert_eq!(next_tag(b"  0\r\n", 0), Next::Invalid);
        assert_eq!(next_tag(b"x\r\nLINE\r\n", 0), Next::Invalid);
    }

    #[test]
    fn split_points_find_group_zero_boundaries_and_clamp_huge_parts() {
        let mut src = Vec::new();
        for i in 0..4000 {
            src.extend_from_slice(
                format!("  0\r\nLINE\r\n  8\r\n0\r\n 10\r\n{i}.0\r\n").as_bytes(),
            );
        }
        let cuts = split_points(&src, 7);
        assert_eq!(cuts.first(), Some(&0));
        assert_eq!(cuts.last(), Some(&src.len()));
        assert!(cuts.len() > 2);
        for pair in cuts.windows(2) {
            assert!(pair[0] < pair[1]);
        }
        for &cut in cuts.iter().take(cuts.len() - 1) {
            assert!(src[cut..].starts_with(b"  0\r\nLINE\r\n"), "{cut}");
        }
        // 極端に大きな分割数を渡してもオーバーフローしない
        let clamped = split_points(&src, usize::MAX);
        assert_eq!(clamped.first(), Some(&0));
        assert_eq!(clamped.last(), Some(&src.len()));
    }

    #[test]
    fn tag_edits_roll_back_on_out_of_bounds_and_round_trip_undo_redo() {
        let raw = b"  0\r\nLINE\r\n  5\r\n10\r\n 10\r\n1.0\r\n 20\r\n2.0\r\n";
        let ov = OverlayRecord::from_slice(RecordSlice {
            base: 0,
            bytes: raw,
        });
        let original = ov.tags.clone();
        let mut tags = original.clone();

        // 2 番目の操作が範囲外なら 1 番目の変更も巻き戻る
        let bad = vec![
            TagEdit::Set {
                at: 2,
                value: b"9.0".to_vec(),
            },
            TagEdit::Remove { at: 99 },
        ];
        assert!(apply_tag_edits(&mut tags, bad).is_none());
        assert_eq!(tags, original);

        let (_, old_vals) = decode_record_slice(&RecordSlice {
            base: 0,
            bytes: raw,
        })
        .expect("decode");
        let mut new_vals = old_vals.clone();
        new_vals[1].data = crate::Data::Real(5.5);
        new_vals.push(Value {
            code: 62,
            data: crate::Data::Int(1),
        });
        let edits = diff_values(1, &old_vals, &new_vals).expect("diff");
        let ops = apply_tag_edits(&mut tags, edits).expect("apply");
        let edited = tags.clone();
        assert_ne!(edited, original);

        revert_tag_ops(&mut tags, &ops);
        assert_eq!(tags, original);

        replay_tag_ops(&mut tags, &ops);
        assert_eq!(tags, edited);
    }

    #[test]
    fn save_dxf_overlay_matches_dxf_file_txn_byte_for_byte() {
        let dxf_bytes = concat!(
            "  0\r\nSECTION\r\n  2\r\nTABLES\r\n",
            "  0\r\nTABLE\r\n  2\r\nLAYER\r\n",
            "  0\r\nLAYER\r\n  2\r\n0\r\n 62\r\n     7\r\n",
            "  0\r\nENDTAB\r\n  0\r\nENDSEC\r\n",
            "  0\r\nSECTION\r\n  2\r\nENTITIES\r\n",
            "  0\r\nLINE\r\n  5\r\n10\r\n  8\r\n0\r\n 10\r\n1.0\r\n 20\r\n2.0\r\n",
            "  0\r\nCIRCLE\r\n  5\r\n11\r\n  8\r\n0\r\n 10\r\n3.0\r\n 20\r\n4.0\r\n 40\r\n5.0\r\n",
            "  0\r\nENDSEC\r\n  0\r\nEOF\r\n",
        )
        .as_bytes();

        // 現行の DxfFile + DxfTxn で画層追加・LINE 変更・CIRCLE 削除・POINT 追加を行う
        let mut budget = Budget::new(Limits::default());
        let mut diag = Diagnostics::new();
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        let mut file = Dxf::read(dxf_bytes, &mut cx).expect("read");
        // レコード番号:
        // 0: SECTION(TABLES), 1: TABLE(LAYER), 2: LAYER(0), 3: ENDTAB, 4: ENDSEC,
        // 5: SECTION(ENTITIES), 6: LINE, 7: CIRCLE, 8: ENDSEC, 9: EOF
        let ranges: Vec<Range<usize>> = file.records.iter().map(|r| r.span.clone()).collect();
        {
            let mut txn = file.transaction();
            txn.insert_record(3, &[(0, b"LAYER".as_slice()), (2, b"A"), (62, b"1")])
                .expect("ins layer");
            txn.set_value(6, 3, b"9.5").expect("set x");
            txn.remove_record(7).expect("rm circle");
            txn.insert_record(
                8,
                &[
                    (0, b"POINT".as_slice()),
                    (5, b"12"),
                    (8, b"A"),
                    (10, b"7.0"),
                ],
            )
            .expect("ins point");
            txn.commit(&mut cx).expect("commit");
        }
        let mut expected = Vec::new();
        Dxf::write(&file, &mut expected, &mut cx).expect("write");

        // SourceBytes + save_dxf_overlay_with_cx で同じ変更を書き出す
        let src = SourceBytes::compress_slice(dxf_bytes, 1);
        let mut overlay = BTreeMap::new();
        let mut line_ov = src.with_range(ranges[6].clone(), OverlayRecord::from_slice);
        line_ov.tags[3].edited = Some(b"9.5".to_vec());
        overlay.insert(6, line_ov);
        overlay.insert(
            10,
            OverlayRecord::born_before(
                3,
                vec![
                    OTag::added(0, b"LAYER".to_vec()),
                    OTag::added(2, b"A".to_vec()),
                    OTag::added(62, b"1".to_vec()),
                ],
            ),
        );
        overlay.insert(
            11,
            OverlayRecord::born(vec![
                OTag::added(0, b"POINT".to_vec()),
                OTag::added(5, b"12".to_vec()),
                OTag::added(8, b"A".to_vec()),
                OTag::added(10, b"7.0".to_vec()),
            ]),
        );
        let mut gone = BTreeSet::new();
        gone.insert(7);

        let actual = save_dxf_overlay(
            &src,
            10,
            |r| ranges[r as usize].clone(),
            &overlay,
            &gone,
            Some(8),
        );
        assert_eq!(actual, expected);

        // DecodedBytes 予算超過でパニックせず止まる
        let mut tight_budget =
            Budget::new(Limits::default().with(Kind::DecodedBytes, Limit::Finite(16)));
        let mut tight_diag = Diagnostics::new();
        let mut tight_cx = Cx::new(&mut tight_budget, &mut tight_diag, &cancel);
        let err = save_dxf_overlay_with_cx(
            &mut tight_cx,
            &src,
            10,
            |r| ranges[r as usize].clone(),
            &overlay,
            &gone,
            Some(8),
        )
        .unwrap_err();
        assert!(matches!(err, Stop::Exceeded(e) if e.kind == Kind::DecodedBytes));
    }
}
