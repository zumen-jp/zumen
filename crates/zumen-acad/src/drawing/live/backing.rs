//! DXF・DWG の元バイト保持・単一レコード復号・差分ステージング・保存（`docs/live.md`、#878）。
//!
//! - ASCII DXF は [`SourceBytes`]（64 KiB ページ列）と変更・追加レコードの [`OverlayRecord`] だけを持ち、
//!   未変更の区間は元のページからそのまま写す。ヘッダー変数（`$EXTMIN`・`$EXTMAX`・`$HANDSEED`・`$PDMODE`・`$PDSIZE`）は
//!   元の `HEADER` スパン列と現在値だけを持ち、保存時に [`DxfFile::write_back`] と同じ順序で差分を組む。
//! - バイナリ DXF は [`DxfFile`] を保持し、編集は [`DxfFile::apply`] と整合させる。
//! - DWG は [`DwgFile`] と [`DwgSingle`] に加え、各 `apply` 段階の軽量ログを持つ。全体を書く保存（`save`）は
//!   有効な段階を [`DwgSingle::apply_fast_step`] で順に当てて [`DwgFile::apply`] と 1 バイト単位で一致させ、
//!   後ろに足す保存（`save_compact`）は 1 回のトランザクションに畳んで中間リビジョンを省く。

use core::ops::Range;
use std::collections::{BTreeMap, BTreeSet};

use zumen_base::{Cx, Diagnostic, Key, Kind, Ref, codes};
use zumen_drawing::{CalloutContent, Drawing2D, Element, Shape};
use zumen_live::SourceBytes;
use zumen_sdk::{Codec, Error};

use crate::db::{Data, Section, Value};
use crate::dwg::single::{
    DwgFastStep, DwgSingle, DwgSourceLayout, DwgSourceSpans, UpdatedDwgRecord,
};
use crate::dwg::{Dwg, DwgFile};
use crate::dxf::live_io::{
    AppliedTagOp, OTag, OverlayRecord, Span, apply_tag_edits, decode_record, replay_tag_ops,
    revert_tag_ops, save_dxf_overlay_with_cx,
};
use crate::dxf::{
    Dxf, DxfFile, advance_handseed, diff_values, entity_extents, merge_extents, on_tag,
    value_bytes, widen_extents,
};

/// 常駐文書エンジンが開いている元の形式
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LiveFormat {
    /// ASCII の DXF
    AsciiDxf,
    /// バイナリの DXF
    BinaryDxf,
    /// DWG（R2000〜R2018）
    Dwg,
}

/// DXF ヘッダーのうち編集で変わりうる 5 変数の現在値
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DxfHeaderValues {
    /// `$EXTMIN`
    pub extmin: Option<Vec<Value>>,
    /// `$EXTMAX`
    pub extmax: Option<Vec<Value>>,
    /// `$HANDSEED`
    pub handseed: Option<Vec<Value>>,
    /// `$PDMODE`
    pub pdmode: Option<Vec<Value>>,
    /// `$PDSIZE`
    pub pdsize: Option<Vec<Value>>,
}

/// ASCII DXF の 1 レコードに対する変更記録 `(record, applied_ops, before_edited_values, after_edited_values, created_overlay)`
pub type DxfRecordOp = (u32, Vec<AppliedTagOp>, Option<Vec<Value>>, Vec<Value>, bool);

/// ASCII DXF の 1 回の高速 `apply` が残す取り消し・やり直し用パッチ
#[derive(Clone, Debug)]
pub struct DxfStepPatch {
    /// 既存・追加レコードに対する差分操作列
    pub record_ops: Vec<DxfRecordOp>,
    /// 新しく追加されたレコード `(record_id, overlay_record, values)`
    pub born_added: Vec<(u32, OverlayRecord, Vec<Value>)>,
    /// 削除されたレコード番号
    pub removed_records: Vec<u32>,
    /// 変更前のヘッダー変数
    pub before_header: DxfHeaderValues,
    /// 変更後のヘッダー変数
    pub after_header: DxfHeaderValues,
    /// 変更前の最大ハンドル（元からあった最大ハンドルを含む）
    pub before_max_handle: Option<u64>,
    /// 変更後の最大ハンドル
    pub after_max_handle: Option<u64>,
}

const TAG_LOGICAL_BYTES: usize = {
    assert!(core::mem::size_of::<OTag>() <= 64);
    64
};
const TAG_OP_LOGICAL_BYTES: usize = {
    assert!(core::mem::size_of::<AppliedTagOp>() <= 128);
    128
};

fn tag_payload_bytes(tag: &OTag) -> usize {
    tag.edited.as_ref().map_or(0, Vec::len)
}

fn tag_op_logical_bytes(operation: &AppliedTagOp) -> usize {
    TAG_OP_LOGICAL_BYTES.saturating_add(match operation {
        AppliedTagOp::Set { before, after, .. } => before
            .as_ref()
            .map_or(0, Vec::len)
            .saturating_add(after.len()),
        AppliedTagOp::Remove { removed, .. } => tag_payload_bytes(removed),
        AppliedTagOp::Insert { inserted, .. } => tag_payload_bytes(inserted),
    })
}

fn overlay_logical_bytes(overlay: &OverlayRecord) -> usize {
    overlay.tags.iter().fold(
        overlay
            .raw
            .len()
            .saturating_add(overlay.tags.len().saturating_mul(TAG_LOGICAL_BYTES)),
        |bytes, tag| bytes.saturating_add(tag_payload_bytes(tag)),
    )
}

impl DxfStepPatch {
    /// 履歴予算用の概算バイト数
    pub fn approx_bytes(&self) -> usize {
        let ops_bytes: usize = self
            .record_ops
            .iter()
            .map(|(_, ops, b, a, _)| {
                32 + ops.iter().map(tag_op_logical_bytes).sum::<usize>()
                    + b.as_ref().map_or(0, |v| v.len() * 24)
                    + a.len() * 24
            })
            .sum();
        let born_bytes: usize = self
            .born_added
            .iter()
            .map(|(_, ov, vals)| 24 + overlay_logical_bytes(ov) + vals.len() * 24)
            .sum();
        ops_bytes + born_bytes + self.removed_records.len() * 4 + 128
    }
}

/// ASCII DXF のページ保持とレコード差分
#[derive(Clone)]
pub struct DxfLiveBacking {
    /// 64 KiB ページ列に分割した元のバイト列
    pub src: SourceBytes,
    /// 元の各レコードの開始オフセット（`0..source_records`）
    pub record_starts: Vec<u32>,
    /// `EOF` の直後（トレーラーの開始オフセット）
    pub trailer_start: u32,
    /// `ENTITIES` セクションの最初の `ENDSEC` レコード番号
    pub entities_end_record: Option<u32>,
    /// `HEADER` セクションの `SECTION` レコード番号
    pub header_record: Option<u32>,
    /// `HEADER` レコードの元のスパン列
    pub header_spans: Box<[Span]>,
    /// `HEADER` に元からあった全変数の開始タグ添字（名前のタグの次）
    pub header_known_starts: BTreeMap<Vec<u8>, usize>,
    /// 編集で変わりうる 5 変数の初期値
    pub base_header: DxfHeaderValues,
    /// 編集で変わりうる 5 変数の現在値
    pub header: DxfHeaderValues,
    /// 変更・追加されたレコードのタグ差分（`HEADER` 以外）
    pub overlay: BTreeMap<u32, OverlayRecord>,
    /// 削除されたレコード番号
    pub gone: BTreeSet<u32>,
    /// 編集・追加済みレコードの現在の `Value` 列
    pub edited_values: BTreeMap<u32, Vec<Value>>,
    /// 次に割り当てる追加レコード番号
    pub next_born_record: u32,
    /// 現在生きているオブジェクトの最大ハンドル（削除で厳密に減らしたいとき以外は単調でもよいが、元ファイルの全非モデル最大値も保持する）
    pub non_model_max_handle: Option<u64>,
    /// 現在の全体最大ハンドルのキャッシュ
    pub current_max_handle: Option<u64>,
}

/// 差分保存の順序に合わせて、内部のレコード番号を保存後の番号へ移す。
pub struct DxfRecordMapper {
    source_records: u32,
    removed: Vec<u32>,
    insertions: Vec<(u32, u32)>,
    born: BTreeMap<u32, u32>,
}

/// 保持モデルの出所を、全体保存した DWG のオブジェクトの位置へ移す。
pub struct DwgSourceMapper {
    by_handle: DwgSourceSpans,
    by_base_span: BTreeMap<(u64, u64), Range<usize>>,
}

impl DwgSourceMapper {
    pub fn remap_ref(&self, source: &mut Ref) {
        if let Some(Key::Handle(handle)) = source.key()
            && let Some(span) = self.by_handle.get(&handle)
        {
            *source = source
                .clone()
                .with_bytes(span.start as u64, span.end as u64);
        } else if source.key().is_none()
            && let Some(bytes) = source.bytes()
            && let Some(span) = self.by_base_span.get(&(bytes.start, bytes.end))
        {
            *source = source
                .clone()
                .with_bytes(span.start as u64, span.end as u64);
        }
    }

    pub fn remap_element<P>(&self, element: &mut Element<P>) {
        self.remap_ref(&mut element.source);
        match &mut element.shape {
            Shape::Symbol(symbol) => {
                for attribute in &mut symbol.attributes {
                    self.remap_ref(&mut attribute.source);
                }
            }
            Shape::Leader(leader) => {
                if let Some(annotation) = &mut leader.annotation {
                    self.remap_ref(annotation);
                }
            }
            Shape::Hatch(hatch) => {
                for boundary in &mut hatch.loops {
                    for source in &mut boundary.sources {
                        self.remap_ref(source);
                    }
                }
            }
            Shape::Callout(callout) => {
                if let Some(style) = &mut callout.style {
                    self.remap_ref(style);
                }
                if let CalloutContent::Symbol { values, .. } = &mut callout.content {
                    for value in values {
                        self.remap_ref(&mut value.definition);
                    }
                }
            }
            Shape::Table(table) => {
                if let Some(style) = &mut table.style {
                    self.remap_ref(style);
                }
            }
            Shape::Underlay(underlay) => self.remap_ref(&mut underlay.resource.definition),
            _ => {}
        }
    }

    pub fn remap_non_model_sources(&self, drawing: &mut Drawing2D) {
        for layer in &mut drawing.layers {
            self.remap_ref(&mut layer.source);
        }
        for line_type in &mut drawing.line_types {
            self.remap_ref(&mut line_type.source);
        }
        for style in &mut drawing.dimension_styles {
            self.remap_ref(&mut style.source);
        }
        for style in &mut drawing.text_styles {
            self.remap_ref(&mut style.source);
        }
        for symbol in &mut drawing.symbols {
            self.remap_ref(&mut symbol.source);
            for element in &mut symbol.space.elements {
                self.remap_element(element);
            }
            for attribute in &mut symbol.attributes {
                self.remap_ref(&mut attribute.attribute.source);
            }
        }
        for sheet in &mut drawing.sheets {
            self.remap_ref(&mut sheet.source);
            for element in &mut sheet.elements {
                self.remap_element(element);
            }
            for view in &mut sheet.views {
                self.remap_ref(&mut view.source);
            }
        }
    }
}

impl DxfRecordMapper {
    pub fn effective(&self, record: u32) -> Option<u32> {
        if record >= self.source_records {
            return self.born.get(&record).copied();
        }
        let removed_before = self.removed.partition_point(|&r| r < record);
        if self.removed.get(removed_before) == Some(&record) {
            return None;
        }
        let inserted_before = self.insertions.partition_point(|&(r, _)| r <= record);
        let added = inserted_before
            .checked_sub(1)
            .and_then(|i| self.insertions.get(i))
            .map_or(0, |&(_, count)| count);
        record
            .checked_sub(removed_before as u32)?
            .checked_add(added)
    }

    pub fn remap_ref(&self, source: &mut Ref) {
        if let Some(record) = source.record()
            && let Ok(record) = u32::try_from(record)
            && let Some(effective) = self.effective(record)
        {
            *source = source.clone().with_record(u64::from(effective));
        }
    }

    pub fn remap_shape_links<P>(&self, shape: &mut Shape<P>) {
        match shape {
            Shape::Hatch(hatch) => {
                for boundary in &mut hatch.loops {
                    for source in &mut boundary.sources {
                        self.remap_ref(source);
                    }
                }
            }
            Shape::Callout(callout) => {
                if let Some(style) = &mut callout.style {
                    self.remap_ref(style);
                }
                if let CalloutContent::Symbol { values, .. } = &mut callout.content {
                    for value in values {
                        self.remap_ref(&mut value.definition);
                    }
                }
            }
            Shape::Table(table) => {
                if let Some(style) = &mut table.style {
                    self.remap_ref(style);
                }
            }
            Shape::Underlay(underlay) => self.remap_ref(&mut underlay.resource.definition),
            _ => {}
        }
    }

    fn remap_element<P>(&self, element: &mut Element<P>) {
        self.remap_ref(&mut element.source);
        match &mut element.shape {
            Shape::Symbol(symbol) => {
                for attribute in &mut symbol.attributes {
                    self.remap_ref(&mut attribute.source);
                }
            }
            Shape::Leader(leader) => {
                if let Some(annotation) = &mut leader.annotation {
                    self.remap_ref(annotation);
                }
            }
            _ => self.remap_shape_links(&mut element.shape),
        }
    }

    pub fn remap_non_model_sources(&self, drawing: &mut Drawing2D) {
        for layer in &mut drawing.layers {
            self.remap_ref(&mut layer.source);
        }
        for line_type in &mut drawing.line_types {
            self.remap_ref(&mut line_type.source);
        }
        for style in &mut drawing.dimension_styles {
            self.remap_ref(&mut style.source);
        }
        for style in &mut drawing.text_styles {
            self.remap_ref(&mut style.source);
        }
        for symbol in &mut drawing.symbols {
            self.remap_ref(&mut symbol.source);
            for element in &mut symbol.space.elements {
                self.remap_element(element);
            }
            for attribute in &mut symbol.attributes {
                self.remap_ref(&mut attribute.attribute.source);
            }
        }
        for sheet in &mut drawing.sheets {
            self.remap_ref(&mut sheet.source);
            for element in &mut sheet.elements {
                self.remap_element(element);
            }
            for view in &mut sheet.views {
                self.remap_ref(&mut view.source);
            }
        }
    }
}

impl DxfLiveBacking {
    /// 単体取得の補正用の対応表を、呼び出し元の予算で作る。
    pub fn record_mapper_with_cx(&self, cx: &mut Cx<'_>) -> Result<Option<DxfRecordMapper>, Error> {
        let count = (self.gone.len() as u64).saturating_add(self.overlay.len() as u64);
        let work = count.saturating_mul(u64::from(count.checked_ilog2().unwrap_or(0)) + 8);
        for (kind, amount) in [
            (Kind::Work, work),
            (Kind::DecodedBytes, count.saturating_mul(256)),
        ] {
            cx.check(kind, amount).map_err(|stop| {
                cx.report(stop.to_diagnostic());
                Error::Stopped(stop)
            })?;
        }
        Ok(self.record_mapper())
    }

    /// 追加と削除がある場合だけ、保存時と同じ挿入位置・作成順の対応表を作る。
    pub fn record_mapper(&self) -> Option<DxfRecordMapper> {
        let source_records = self.source_records();
        if self.gone.is_empty() && self.overlay.range(source_records..).next().is_none() {
            return None;
        }
        let removed: Vec<_> = self.gone.range(..source_records).copied().collect();
        let mut inserted: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
        for (&record, overlay) in self.overlay.range(source_records..) {
            if !self.gone.contains(&record)
                && let Some(target) = overlay.insert_before.or(self.entities_end_record)
            {
                inserted.entry(target).or_default().push(record);
            }
        }
        let mut insertions = Vec::with_capacity(inserted.len());
        let mut born = BTreeMap::new();
        let mut count = 0_u32;
        for (target, records) in inserted {
            let removed_before = removed.partition_point(|&r| r < target) as u32;
            for record in records {
                let effective = target.checked_sub(removed_before)?.checked_add(count)?;
                born.insert(record, effective);
                count = count.checked_add(1)?;
            }
            insertions.push((target, count));
        }
        Some(DxfRecordMapper {
            source_records,
            removed,
            insertions,
            born,
        })
    }

    /// 元のレコード件数
    #[inline]
    pub fn source_records(&self) -> u32 {
        self.record_starts.len() as u32
    }

    /// 元のレコード `record` のバイト範囲を返す
    #[inline]
    pub fn record_range(&self, record: u32) -> Range<usize> {
        let idx = record as usize;
        let Some(&start) = self.record_starts.get(idx) else {
            return self.src.len()..self.src.len();
        };
        let end = self
            .record_starts
            .get(idx + 1)
            .copied()
            .unwrap_or(self.trailer_start);
        (start as usize)..(end as usize)
    }

    /// レコード `record` の現在の `(kind, values)` を 1 件だけ復号する
    pub fn decode_current(
        &self,
        record: u32,
        fallback_kind: &[u8],
    ) -> Option<(Vec<u8>, Vec<Value>)> {
        if self.gone.contains(&record) {
            return None;
        }
        if let Some(vals) = self.edited_values.get(&record) {
            let kind = if record < self.source_records() {
                decode_record(&self.src, self.record_range(record))
                    .map_or_else(|| fallback_kind.to_vec(), |(k, _)| k)
            } else if let Some(ov) = self.overlay.get(&record) {
                ov.tags
                    .first()
                    .and_then(|t| t.edited.clone())
                    .unwrap_or_else(|| fallback_kind.to_vec())
            } else {
                fallback_kind.to_vec()
            };
            return Some((kind, vals.clone()));
        }
        if record < self.source_records() {
            decode_record(&self.src, self.record_range(record))
        } else {
            None
        }
    }

    /// 新しいレコードのタグ列を組み立てる（符号化できない実数があればそのグループ・コードを `Err` で返す）
    pub fn build_born_overlay(kind: &[u8], values: &[Value]) -> Result<OverlayRecord, i16> {
        let mut tags = Vec::with_capacity(values.len().saturating_add(1));
        tags.push(OTag::added(0, kind.to_vec()));
        for val in values {
            let bytes = value_bytes(&val.data)
                .map(|b| b.into_owned())
                .ok_or(val.code)?;
            tags.push(OTag::added(val.code, bytes));
        }
        Ok(OverlayRecord::born(tags))
    }

    /// 触れた・追加したモデル空間図形と最大ハンドルから、更新後のヘッダー変数を計算する
    pub fn compute_updated_header(
        &self,
        touched_model: &[(&[u8], &[Value])],
        pdmode_override: Option<Vec<Value>>,
        pdsize_override: Option<Vec<Value>>,
        new_max_handle: Option<u64>,
    ) -> Result<(DxfHeaderValues, bool), Error> {
        let mut next = self.header.clone();
        if let Some(m) = pdmode_override {
            next.pdmode = Some(m);
        }
        if let Some(s) = pdsize_override {
            next.pdsize = Some(s);
        }
        let mut extent_box = None;
        for &(kind, values) in touched_model {
            extent_box = merge_extents(extent_box, entity_extents(kind, true, values));
        }
        if let Some(found) = extent_box {
            if let Some(cur_min) = &next.extmin
                && let Some(widened) = widen_extents(false, cur_min, found)
            {
                next.extmin = Some(widened);
            }
            if let Some(cur_max) = &next.extmax
                && let Some(widened) = widen_extents(true, cur_max, found)
            {
                next.extmax = Some(widened);
            }
        }
        if let (Some(cur_seed), Some(max_h)) = (&next.handseed, new_max_handle)
            && let Some(advanced) = advance_handseed(cur_seed, max_h)
        {
            next.handseed = Some(advanced);
        }
        let any_changed_vs_base = next != self.base_header;
        if any_changed_vs_base && self.header_record.is_none() {
            return Err(Error::Rejected);
        }
        Ok((next, any_changed_vs_base))
    }

    /// `HEADER` の差分 `OverlayRecord` を現在の変数状態から組み立てる
    fn build_header_overlay(&self) -> Result<Option<(u32, OverlayRecord)>, (u32, i16)> {
        type TrackedVar<'a> = (&'a [u8], Option<&'a [Value]>, Option<&'a [Value]>);
        type ChangedVar<'a> = (&'a [u8], Option<&'a [Value]>, &'a [Value]);
        if self.header == self.base_header {
            return Ok(None);
        }
        let Some(hdr_rec) = self.header_record else {
            return Ok(None);
        };
        let tracked: [TrackedVar<'_>; 5] = [
            (
                b"$EXTMAX",
                self.base_header.extmax.as_deref(),
                self.header.extmax.as_deref(),
            ),
            (
                b"$EXTMIN",
                self.base_header.extmin.as_deref(),
                self.header.extmin.as_deref(),
            ),
            (
                b"$HANDSEED",
                self.base_header.handseed.as_deref(),
                self.header.handseed.as_deref(),
            ),
            (
                b"$PDMODE",
                self.base_header.pdmode.as_deref(),
                self.header.pdmode.as_deref(),
            ),
            (
                b"$PDSIZE",
                self.base_header.pdsize.as_deref(),
                self.header.pdsize.as_deref(),
            ),
        ];
        // `AcadDb::variables` は変数名の辞書順（BTreeMap）で並ぶため、同じ順序で変更分を集める
        let mut changed: Vec<ChangedVar<'_>> = Vec::new();
        for (name, base_vals, cur_vals) in tracked {
            if let Some(cur) = cur_vals
                && base_vals != Some(cur)
            {
                changed.push((name, base_vals, cur));
            }
        }
        if changed.is_empty() {
            return Ok(None);
        }
        let mut ov = OverlayRecord::from_spans(&self.header_spans);
        let mut end = ov.tags.len();
        for &(name, _, cur) in &changed {
            if !self.header_known_starts.contains_key(name) {
                ov.tags.insert(end, OTag::added(9, name.to_vec()));
                end += 1;
                let edits = diff_values(end, &[], cur).map_err(|code| (hdr_rec, code))?;
                let _ = apply_tag_edits(&mut ov.tags, edits);
                end += cur.len();
            }
        }
        let mut known: Vec<(usize, &[Value], &[Value])> = changed
            .iter()
            .filter_map(|&(name, base_vals, cur)| {
                let &start = self.header_known_starts.get(name)?;
                Some((start, base_vals.unwrap_or(&[]), cur))
            })
            .collect();
        known.sort_by_key(|&(start, ..)| core::cmp::Reverse(start));
        for (start, old, new) in known {
            let edits = diff_values(start, old, new).map_err(|code| (hdr_rec, code))?;
            let _ = apply_tag_edits(&mut ov.tags, edits);
        }
        Ok(Some((hdr_rec, ov)))
    }

    /// 1 段階分のパッチを確定させる
    pub fn commit_patch(&mut self, patch: &DxfStepPatch) {
        for (rec, ops, _, after_vals, _) in &patch.record_ops {
            if !self.overlay.contains_key(rec) && *rec < self.source_records() {
                let range = self.record_range(*rec);
                let ov = self.src.with_range(range, OverlayRecord::from_slice);
                self.overlay.insert(*rec, ov);
            }
            if let Some(ov) = self.overlay.get_mut(rec) {
                replay_tag_ops(&mut ov.tags, ops);
            }
            self.edited_values.insert(*rec, after_vals.clone());
        }
        for (rec, ov, vals) in &patch.born_added {
            self.overlay.insert(*rec, ov.clone());
            self.edited_values.insert(*rec, vals.clone());
            self.gone.remove(rec);
            if *rec >= self.next_born_record {
                self.next_born_record = rec.saturating_add(1);
            }
        }
        for &rec in &patch.removed_records {
            self.gone.insert(rec);
        }
        self.header = patch.after_header.clone();
        self.current_max_handle = patch.after_max_handle;
    }

    /// 1 段階分のパッチを取り消す
    pub fn undo_patch(&mut self, patch: &DxfStepPatch) {
        for &rec in patch.removed_records.iter().rev() {
            self.gone.remove(&rec);
        }
        for (rec, _, _) in patch.born_added.iter().rev() {
            self.overlay.remove(rec);
            self.edited_values.remove(rec);
            self.gone.remove(rec);
        }
        for (rec, ops, before_vals, _, created_overlay) in patch.record_ops.iter().rev() {
            if *created_overlay {
                self.overlay.remove(rec);
                self.edited_values.remove(rec);
            } else {
                if let Some(ov) = self.overlay.get_mut(rec) {
                    revert_tag_ops(&mut ov.tags, ops);
                }
                match before_vals {
                    Some(prev) => {
                        self.edited_values.insert(*rec, prev.clone());
                    }
                    None => {
                        self.edited_values.remove(rec);
                    }
                }
            }
        }
        self.header = patch.before_header.clone();
        self.current_max_handle = patch.before_max_handle;
    }

    /// 現在の状態を ASCII DXF バイト列として書き出す（[`DxfFile::apply`] と 1 バイト単位で一致）
    pub fn save(&self, cx: &mut Cx<'_>) -> Result<Vec<u8>, Error> {
        let header_ov = self.build_header_overlay().map_err(|(rec, code)| {
            cx.report(on_tag(codes::DXF_REAL_UNSUPPORTED, rec as usize, code));
            Error::Rejected
        })?;
        let source_records = self.source_records();
        if self.overlay.is_empty() && self.gone.is_empty() && header_ov.is_none() {
            let len = self.src.len();
            cx.check(Kind::Work, ((len / 16384) + 1) as u64)
                .map_err(|stop| {
                    cx.report(stop.to_diagnostic());
                    Error::Stopped(stop)
                })?;
            cx.check(Kind::DecodedBytes, len as u64).map_err(|stop| {
                cx.report(stop.to_diagnostic());
                Error::Stopped(stop)
            })?;
            let mut out = Vec::with_capacity(len);
            self.src.page_reader().append_range(&mut out, 0..len);
            return Ok(out);
        }
        let mut combined = self.overlay.clone();
        if let Some((hdr_rec, ov)) = header_ov {
            combined.insert(hdr_rec, ov);
        }
        save_dxf_overlay_with_cx(
            cx,
            &self.src,
            source_records,
            |r| self.record_range(r),
            &combined,
            &self.gone,
            self.entities_end_record,
        )
        .map_err(|stop| {
            cx.report(stop.to_diagnostic());
            Error::Stopped(stop)
        })
    }
}

/// DWG の既存レコード 1 件に対する編集状態
#[derive(Clone, Debug)]
pub struct DwgRecordPatch {
    /// 種別名
    pub kind: Vec<u8>,
    /// セクション
    pub section: Section,
    /// ラジアン往復後の現在の `Value` 列
    pub values: Vec<Value>,
    /// 現在の符号化済みレコードバイト列
    pub encoded: Vec<u8>,
}

/// DWG に追加されたレコード 1 件の状態
#[derive(Clone, Debug)]
pub struct DwgBornRecord {
    /// ハンドル
    pub handle: u64,
    /// 種別名
    pub kind: Vec<u8>,
    /// ラジアン往復後の現在の `Value` 列
    pub values: Vec<Value>,
    /// 現在の符号化済みレコードバイト列
    pub encoded: Vec<u8>,
    /// モデル空間の図形か
    pub in_model: bool,
}

/// DWG の 1 回の高速 `apply` が残すデータ（取り消し・やり直しと全体を書く保存の両方に使う）
#[derive(Clone, Debug)]
pub struct DwgStepPatch {
    /// 全体を書く保存用の `(handle, section, kind, old_values, new_values)`
    pub step_updated: Vec<UpdatedDwgRecord>,
    /// 全体を書く保存用の削除ハンドル集合
    pub step_removed_handles: BTreeSet<u64>,
    /// 全体を書く保存用の削除モデル空間ハンドル集合
    pub step_removed_model_handles: BTreeSet<u64>,
    /// 全体を書く保存用の追加レコード `(handle, encoded, in_model)`
    pub step_added: Vec<(u64, Vec<u8>, bool)>,
    /// 全体を書く保存用の新 `$HANDSEED`（この段階で追加があったときのみ `Some`）
    pub step_new_seed: Option<u64>,
    /// 全体を書く保存用の `PDMODE` 更新
    pub step_point_mode: Option<i16>,
    /// 全体を書く保存用の `PDSIZE` 更新
    pub step_point_size: Option<f64>,

    /// 既存レコードの更新 `(record, before_patch, after_patch)`
    pub patched_diffs: Vec<(u32, Option<DwgRecordPatch>, DwgRecordPatch)>,
    /// 追加レコードの更新 `(record, before_born, after_born)`
    pub born_diffs: Vec<(u32, Option<DwgBornRecord>, DwgBornRecord)>,
    /// 削除されたレコード番号
    pub removed_records: Vec<u32>,
    /// 削除されたモデル空間ハンドル
    pub removed_model_handles: Vec<u64>,
    /// 変更前後の `(handseed, point_mode, point_size, max_handle)`
    pub before_meta: (Option<u64>, Option<i16>, Option<f64>, Option<u64>),
    /// 変更後の `(handseed, point_mode, point_size, max_handle)`
    pub after_meta: (Option<u64>, Option<i16>, Option<f64>, Option<u64>),
}

impl DwgStepPatch {
    /// 履歴予算用の概算バイト数
    pub fn approx_bytes(&self) -> usize {
        let u_bytes: usize = self
            .step_updated
            .iter()
            .map(|(_, _, k, o, n)| 48 + k.len() + (o.len() + n.len()) * 24)
            .sum();
        let p_bytes: usize = self
            .patched_diffs
            .iter()
            .map(|(_, b, a)| {
                32 + b
                    .as_ref()
                    .map_or(0, |x| x.encoded.len() + x.values.len() * 24)
                    + a.encoded.len()
                    + a.values.len() * 24
            })
            .sum();
        let b_bytes: usize = self
            .born_diffs
            .iter()
            .map(|(_, b, a)| {
                32 + b
                    .as_ref()
                    .map_or(0, |x| x.encoded.len() + x.values.len() * 24)
                    + a.encoded.len()
                    + a.values.len() * 24
            })
            .sum();
        u_bytes + p_bytes + b_bytes + self.removed_records.len() * 12 + 128
    }
}

/// DWG の常駐バックエンド
#[derive(Clone)]
pub struct DwgLiveBacking {
    /// 開いた時点の `DwgFile`
    pub base_file: DwgFile,
    /// 単一レコード復号・符号化索引
    pub single: DwgSingle,
    /// 変更された既存レコード
    pub patched: BTreeMap<u32, DwgRecordPatch>,
    /// 追加されたレコード（キーは `source_records..` の仮想レコード番号）
    pub born: BTreeMap<u32, DwgBornRecord>,
    /// 削除されたレコード番号（既存・追加の両方）
    pub gone: BTreeSet<u32>,
    /// 削除された既存のモデル空間図形のハンドル集合
    pub removed_base_model_handles: BTreeSet<u64>,
    /// 現在の `HANDSEED`
    pub handseed: Option<u64>,
    /// 変更された `PDMODE`（未変更なら `None`）
    pub point_mode_override: Option<i16>,
    /// 変更された `PDSIZE`（未変更なら `None`）
    pub point_size_override: Option<f64>,
    /// 現在の最大ハンドル
    pub current_max_handle: Option<u64>,
    /// 次に割り当てる仮想レコード番号
    pub next_born_record: u32,
    /// 有効な高速 `apply` 段階の履歴（`0..step_cursor` が現在有効）
    pub steps: Vec<DwgStepPatch>,
    /// 有効な段階数
    pub step_cursor: usize,
}

/// DWG レコードの現在値 `(kind, section, encoded, values)`
pub type CurrentDwgRecord = (Vec<u8>, Section, Vec<u8>, Vec<Value>);

fn copy_dwg_record(
    kind: &[u8],
    section: &Section,
    encoded: &[u8],
    values: &[Value],
    cx: &mut Cx<'_>,
) -> Result<CurrentDwgRecord, Error> {
    let stopped = |cx: &mut Cx<'_>, stop: zumen_base::Stop| {
        cx.report(stop.to_diagnostic());
        Error::Stopped(stop)
    };
    let section_bytes = match section {
        Section::Other(name) => name.len() as u64,
        _ => 0,
    };
    let fixed = (kind.len() as u64)
        .saturating_add(encoded.len() as u64)
        .saturating_add(section_bytes);
    cx.check(Kind::Work, (values.len() as u64).saturating_add(1))
        .map_err(|stop| stopped(cx, stop))?;
    let payload = values.iter().fold(fixed, |size, value| {
        size.saturating_add(match &value.data {
            Data::Text(bytes) | Data::Binary(bytes) => bytes.len() as u64,
            _ => 0,
        })
    });
    cx.check(Kind::Work, payload)
        .map_err(|stop| stopped(cx, stop))?;
    cx.check(
        Kind::DecodedBytes,
        (values.len() as u64)
            .saturating_mul(96)
            .saturating_add(payload)
            .saturating_add(128),
    )
    .map_err(|stop| stopped(cx, stop))?;
    let record = (
        kind.to_vec(),
        section.clone(),
        encoded.to_vec(),
        values.to_vec(),
    );
    cx.check(Kind::Work, 0).map_err(|stop| stopped(cx, stop))?;
    Ok(record)
}

impl DwgLiveBacking {
    /// 元のレコード件数
    #[inline]
    pub fn source_records(&self) -> u32 {
        self.base_file.entry_count() as u32
    }

    /// レコード `record` の現在の `(kind, section, encoded, values)` を取り出す
    pub fn current_record_data(
        &self,
        record: u32,
        cx: &mut Cx<'_>,
    ) -> Result<Option<CurrentDwgRecord>, Error> {
        cx.check(Kind::Work, 1).map_err(|stop| {
            cx.report(stop.to_diagnostic());
            Error::Stopped(stop)
        })?;
        if self.gone.contains(&record) {
            return Ok(None);
        }
        if let Some(p) = self.patched.get(&record) {
            return copy_dwg_record(&p.kind, &p.section, &p.encoded, &p.values, cx).map(Some);
        }
        if let Some(b) = self.born.get(&record) {
            return copy_dwg_record(&b.kind, &Section::Entities, &b.encoded, &b.values, cx)
                .map(Some);
        }
        let rec_usize = record as usize;
        let Some(obj) = self
            .single
            .decode_record_with_cx(&self.base_file, rec_usize, cx)?
        else {
            return Ok(None);
        };
        let Some(enc) = self.single.base_encoded(&self.base_file, rec_usize) else {
            return Ok(None);
        };
        Ok(Some((obj.kind, obj.section, enc, obj.values)))
    }

    /// 1 段階分のパッチを確定させる（新しい編集のとき、`is_redo` が偽なら後ろの段階を切り詰めて積む）
    pub fn commit_patch(&mut self, patch: &DwgStepPatch, is_redo: bool) {
        for (rec, _, after) in &patch.patched_diffs {
            self.patched.insert(*rec, after.clone());
        }
        for (rec, _, after) in &patch.born_diffs {
            self.born.insert(*rec, after.clone());
            self.gone.remove(rec);
            if *rec >= self.next_born_record {
                self.next_born_record = rec.saturating_add(1);
            }
        }
        for &rec in &patch.removed_records {
            self.gone.insert(rec);
        }
        for &h in &patch.removed_model_handles {
            if self
                .single
                .record_of_handle(h)
                .is_some_and(|r| (r as u32) < self.source_records())
            {
                self.removed_base_model_handles.insert(h);
            }
        }
        let (seed, pmode, psize, max_h) = patch.after_meta;
        self.handseed = seed;
        self.point_mode_override = pmode;
        self.point_size_override = psize;
        self.current_max_handle = max_h;
        if is_redo {
            self.step_cursor = self.step_cursor.saturating_add(1);
        } else {
            self.steps.truncate(self.step_cursor);
            self.steps.push(patch.clone());
            self.step_cursor = self.steps.len();
        }
    }

    /// 1 段階分のパッチを取り消す
    pub fn undo_patch(&mut self, patch: &DwgStepPatch) {
        for &h in patch.removed_model_handles.iter().rev() {
            self.removed_base_model_handles.remove(&h);
        }
        for &rec in patch.removed_records.iter().rev() {
            self.gone.remove(&rec);
        }
        for (rec, before, _) in patch.born_diffs.iter().rev() {
            match before {
                Some(prev) => {
                    self.born.insert(*rec, prev.clone());
                }
                None => {
                    self.born.remove(rec);
                }
            }
        }
        for (rec, before, _) in patch.patched_diffs.iter().rev() {
            match before {
                Some(prev) => {
                    self.patched.insert(*rec, prev.clone());
                }
                None => {
                    self.patched.remove(rec);
                }
            }
        }
        let (seed, pmode, psize, max_h) = patch.before_meta;
        self.handseed = seed;
        self.point_mode_override = pmode;
        self.point_size_override = psize;
        self.current_max_handle = max_h;
        self.step_cursor = self.step_cursor.saturating_sub(1);
    }

    /// 全体を書く保存（[`DwgFile::apply`] を 1 回ずつ当てた結果と 1 バイト単位で一致するバイト列を返す）
    pub fn save(&self, cx: &mut Cx<'_>) -> Result<Vec<u8>, Error> {
        let active = self.steps.get(..self.step_cursor).unwrap_or_default();
        if active.is_empty() {
            let mut out = Vec::new();
            Dwg::write(&self.base_file, &mut out, cx)?;
            return Ok(out);
        }
        let file = self.replay_file(cx)?;
        let mut out = Vec::new();
        Dwg::write(&file, &mut out, cx)?;
        Ok(out)
    }

    /// 保持した形状を復号し直さず、保存と同じ変更列から出所だけを得る。
    pub fn source_mapper(&self, cx: &mut Cx<'_>) -> Result<Option<DwgSourceMapper>, Error> {
        if self.step_cursor == 0 {
            return Ok(None);
        }
        let mut layout = DwgSourceLayout::new(&self.base_file, cx)?;
        for step in self.steps.get(..self.step_cursor).unwrap_or_default() {
            self.single.apply_layout_step(
                &mut layout,
                DwgFastStep {
                    updated: &step.step_updated,
                    removed: &step.step_removed_handles,
                    removed_model: &step.step_removed_model_handles,
                    added: &step.step_added,
                    seed: step.step_new_seed,
                    point_mode: step.step_point_mode,
                    point_size: step.step_point_size,
                },
                cx,
            )?;
        }
        Ok(Some(DwgSourceMapper {
            by_handle: layout.source_spans(cx)?,
            by_base_span: layout.base_span_shifts(cx)?,
        }))
    }

    fn replay_file(&self, cx: &mut Cx<'_>) -> Result<DwgFile, Error> {
        let active = self.steps.get(..self.step_cursor).unwrap_or_default();
        let mut file = self.base_file.snapshot(cx)?;
        for step in active {
            self.single.apply_fast_step(
                &mut file,
                &step.step_updated,
                &step.step_removed_handles,
                &step.step_removed_model_handles,
                &step.step_added,
                step.step_new_seed,
                step.step_point_mode,
                step.step_point_size,
                cx,
            )?;
        }
        Ok(file)
    }

    /// 後ろに足す（畳み込み）保存。複数段階の変更を 1 回のトランザクションとして書き出し、中間リビジョンを省く
    pub fn save_compact(&self, cx: &mut Cx<'_>) -> Result<Vec<u8>, Error> {
        let source_records = self.source_records();
        let mut staged: BTreeMap<usize, Vec<u8>> = BTreeMap::new();
        for (&rec, patch) in &self.patched {
            if rec < source_records && !self.gone.contains(&rec) {
                staged.insert(rec as usize, patch.encoded.clone());
            }
        }
        let mut removed_records: BTreeSet<usize> = BTreeSet::new();
        for &rec in &self.gone {
            if rec < source_records {
                removed_records.insert(rec as usize);
            }
        }
        let mut added: Vec<(u64, Vec<u8>, bool)> = Vec::new();
        for (&rec, b) in &self.born {
            if !self.gone.contains(&rec) {
                added.push((b.handle, b.encoded.clone(), b.in_model));
            }
        }
        let new_seed = match (self.single.handle_seed(), self.handseed) {
            (base, Some(cur)) if base != Some(cur) => Some(cur),
            _ => None,
        };
        self.single.save_file_with_headers(
            &self.base_file,
            &staged,
            &removed_records,
            &self.removed_base_model_handles,
            &added,
            new_seed,
            self.point_mode_override,
            self.point_size_override,
            cx,
        )
    }
}

/// 形式ごとのバックエンド（文書ごとに 1 つだけ保持する）
#[allow(clippy::large_enum_variant)]
#[derive(Clone)]
pub enum LiveBacking {
    /// ASCII DXF
    AsciiDxf(DxfLiveBacking),
    /// バイナリ DXF
    BinaryDxf(DxfFile),
    /// DWG
    Dwg(DwgLiveBacking),
}

impl LiveBacking {
    /// 開いている形式種別
    pub const fn format(&self) -> LiveFormat {
        match self {
            Self::AsciiDxf(_) => LiveFormat::AsciiDxf,
            Self::BinaryDxf(_) => LiveFormat::BinaryDxf,
            Self::Dwg(_) => LiveFormat::Dwg,
        }
    }

    /// 全体を書く保存
    pub fn save(&self, cx: &mut Cx<'_>) -> Result<Vec<u8>, Error> {
        match self {
            Self::AsciiDxf(dxf) => dxf.save(cx),
            Self::BinaryDxf(file) => {
                let mut out = Vec::new();
                Dxf::write(file, &mut out, cx)?;
                Ok(out)
            }
            Self::Dwg(dwg) => dwg.save(cx),
        }
    }

    /// 後ろに足す保存（DWG では 1 トランザクションに畳んで書き出し、DXF では `save` と同じバイト列を返す）
    pub fn save_compact(&self, cx: &mut Cx<'_>) -> Result<Vec<u8>, Error> {
        match self {
            Self::AsciiDxf(dxf) => dxf.save(cx),
            Self::BinaryDxf(file) => {
                let mut out = Vec::new();
                Dxf::write(file, &mut out, cx)?;
                Ok(out)
            }
            Self::Dwg(dwg) => dwg.save_compact(cx),
        }
    }

    /// 編集対応外の版（R2007 `AC1021` の DWG）なら診断を出して `Err(Error::Unsupported)` を返す
    pub fn check_editable(&self, cx: &mut Cx<'_>) -> Result<(), Error> {
        if let Self::Dwg(dwg) = self
            && dwg.base_file.version() == b"AC1021"
        {
            cx.report(Diagnostic::new(codes::DWG_EDIT_UNSUPPORTED).with("version", "AC1021"));
            return Err(Error::Unsupported);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use zumen_base::{Budget, CancelToken, Diagnostics, Limit, Limits, Stop};
    use zumen_drawing::Hash;

    use super::*;
    use crate::AcadLive;
    use crate::drawing::live::tests::fresh_cx;

    #[expect(clippy::print_stdout, reason = "ネイティブと WASI の比較記録")]
    fn check_current_record_budget(
        backing: &DwgLiveBacking,
        record: u32,
        expected: &CurrentDwgRecord,
    ) {
        let cancel = CancelToken::new();
        let mut measured = Budget::new(Limits::default().with(Kind::InputBytes, Limit::Finite(0)));
        let mut diagnostics = Diagnostics::new();
        assert_eq!(
            backing
                .current_record_data(
                    record,
                    &mut Cx::new(&mut measured, &mut diagnostics, &cancel),
                )
                .unwrap()
                .as_ref(),
            Some(expected)
        );
        assert!(diagnostics.is_empty());
        let mut boundary_record = String::new();
        for kind in [Kind::Work, Kind::DecodedBytes] {
            let needed = measured.used(kind);
            assert!(needed > 0);
            for available in [0, needed / 2, needed - 1, needed] {
                let mut budget =
                    Budget::new(Limits::default().with(kind, Limit::Finite(available + 11)));
                budget.spend(kind, 11).unwrap();
                let mut diagnostics = Diagnostics::new();
                let result = backing.current_record_data(
                    record,
                    &mut Cx::new(&mut budget, &mut diagnostics, &cancel),
                );
                if available < needed {
                    assert!(matches!(result, Err(Error::Stopped(_))));
                    assert!(
                        diagnostics
                            .iter()
                            .any(|item| item.code() == kind.exceeded_code())
                    );
                } else {
                    assert_eq!(result.as_ref().unwrap().as_ref(), Some(expected));
                    assert_eq!(budget.used(kind), needed + 11);
                }
                boundary_record.push_str(&format!(
                    "{kind:?} {available} {:?} {:?} {:?}\n",
                    result.as_ref().map(|record| record.is_some()),
                    Kind::ALL.map(|kind| budget.used(kind)),
                    diagnostics.iter().collect::<Vec<_>>()
                ));
            }
        }
        cancel.cancel();
        let mut budget = Budget::default();
        let mut diagnostics = Diagnostics::new();
        assert_eq!(
            backing
                .current_record_data(record, &mut Cx::new(&mut budget, &mut diagnostics, &cancel),),
            Err(Error::Stopped(Stop::Cancelled))
        );
        assert_eq!(budget.used(Kind::Work), 0);
        assert_eq!(budget.used(Kind::DecodedBytes), 0);
        println!(
            "live-record-budget hash={:?} used={:?} boundaries={:?}",
            Hash::document(format!("{expected:?}").as_bytes()),
            Kind::ALL.map(|kind| measured.used(kind)),
            Hash::document(boundary_record.as_bytes())
        );
    }

    #[test]
    fn current_records_share_budget_and_cancellation() {
        for bytes in [
            include_bytes!("../../../../../samples/dwg/zumen-layout-r2000.dwg").as_slice(),
            include_bytes!("../../../../../samples/dwg/zumen-layout-r2004.dwg").as_slice(),
            include_bytes!("../../../../../samples/dwg/zumen-layout-r2018.dwg").as_slice(),
        ] {
            for threads in [1, 4] {
                let live = fresh_cx(|cx| AcadLive::open_with_threads(bytes, threads, cx))
                    .0
                    .unwrap();
                let record = live
                    .table()
                    .row(zumen_live::RowId::from_index(0).unwrap())
                    .unwrap()
                    .record;
                let LiveBacking::Dwg(mut backing) = live.state.backing else {
                    panic!("DWG required");
                };
                let object = fresh_cx(|cx| {
                    backing
                        .single
                        .decode_record(&backing.base_file, record as usize, cx)
                })
                .0
                .unwrap();
                let encoded = backing
                    .single
                    .base_encoded(&backing.base_file, record as usize)
                    .unwrap();
                let expected = (object.kind, object.section, encoded, object.values);
                check_current_record_budget(&backing, record, &expected);
                backing.patched.insert(
                    record,
                    DwgRecordPatch {
                        kind: expected.0.clone(),
                        section: expected.1.clone(),
                        encoded: expected.2.clone(),
                        values: expected.3.clone(),
                    },
                );
                check_current_record_budget(&backing, record, &expected);
                let born = backing.source_records();
                backing.born.insert(
                    born,
                    DwgBornRecord {
                        handle: 1,
                        kind: expected.0.clone(),
                        encoded: expected.2.clone(),
                        values: expected.3.clone(),
                        in_model: true,
                    },
                );
                check_current_record_budget(&backing, born, &expected);
                backing.gone.insert(record);
                assert_eq!(
                    fresh_cx(|cx| backing.current_record_data(record, cx)).0,
                    Ok(None)
                );
                assert_eq!(
                    fresh_cx(|cx| backing.current_record_data(u32::MAX, cx)).0,
                    Ok(None)
                );
            }
        }
    }

    #[test]
    fn record_copy_budget_uses_payload_lengths() {
        let make_bytes = |capacity| {
            let mut bytes = Vec::with_capacity(capacity);
            bytes.extend_from_slice(b"value");
            bytes
        };
        let measure = |capacity| {
            let values = [
                Value {
                    code: 1,
                    data: Data::Text(make_bytes(capacity)),
                },
                Value {
                    code: 310,
                    data: Data::Binary(make_bytes(capacity)),
                },
            ];
            let mut budget = Budget::default();
            let mut diagnostics = Diagnostics::new();
            let cancel = CancelToken::new();
            let record = copy_dwg_record(
                &make_bytes(capacity),
                &Section::Other(make_bytes(capacity)),
                &make_bytes(capacity),
                &values,
                &mut Cx::new(&mut budget, &mut diagnostics, &cancel),
            )
            .unwrap();
            (
                record,
                Kind::ALL.map(|kind| budget.used(kind)),
                budget.used(Kind::DecodedBytes),
            )
        };
        assert_eq!(measure(5), measure(4096));
        let (small, _, needed) = measure(5);
        for part in 0..5 {
            let mut large = small.clone();
            let payload = match part {
                0 => &mut large.0,
                1 => match &mut large.1 {
                    Section::Other(name) => name,
                    _ => panic!("test section must carry a name"),
                },
                2 => &mut large.2,
                _ => match &mut large.3[part - 3].data {
                    Data::Text(bytes) | Data::Binary(bytes) => bytes,
                    _ => panic!("test value must carry bytes"),
                },
            };
            payload.resize(4096, 0);
            let mut budget =
                Budget::new(Limits::default().with(Kind::DecodedBytes, Limit::Finite(needed)));
            let mut diagnostics = Diagnostics::new();
            let cancel = CancelToken::new();
            assert!(matches!(
                copy_dwg_record(
                    &large.0,
                    &large.1,
                    &large.2,
                    &large.3,
                    &mut Cx::new(&mut budget, &mut diagnostics, &cancel),
                ),
                Err(Error::Stopped(_))
            ));
            assert!(
                diagnostics
                    .iter()
                    .any(|item| { item.code() == Kind::DecodedBytes.exceeded_code() })
            );
        }
    }

    fn patch_with_capacity(capacity: usize) -> DxfStepPatch {
        let bytes = || {
            let mut value = Vec::with_capacity(capacity);
            value.extend_from_slice(b"value");
            value
        };
        let tag = || OTag::added(1, bytes());
        let mut tags = Vec::with_capacity(capacity);
        tags.push(tag());
        DxfStepPatch {
            record_ops: vec![(
                0,
                vec![
                    AppliedTagOp::Set {
                        at: 0,
                        before: Some(bytes()),
                        after: bytes(),
                    },
                    AppliedTagOp::Remove {
                        at: 1,
                        removed: tag(),
                    },
                    AppliedTagOp::Insert {
                        at: 2,
                        inserted: tag(),
                    },
                ],
                None,
                Vec::new(),
                false,
            )],
            born_added: vec![(1, OverlayRecord::born(tags), Vec::new())],
            removed_records: Vec::new(),
            before_header: DxfHeaderValues::default(),
            after_header: DxfHeaderValues::default(),
            before_max_handle: None,
            after_max_handle: None,
        }
    }

    #[test]
    fn history_budget_is_independent_of_allocation_capacity() {
        let compact = patch_with_capacity(5);
        let reserved = patch_with_capacity(4096);
        assert_eq!(compact.record_ops, reserved.record_ops);
        assert_eq!(compact.born_added, reserved.born_added);
        assert_eq!(compact.approx_bytes(), reserved.approx_bytes());
        assert_eq!(reserved.approx_bytes(), reserved.clone().approx_bytes());
    }
}
