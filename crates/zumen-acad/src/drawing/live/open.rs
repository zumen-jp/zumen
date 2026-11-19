//! DXF（ASCII・バイナリ）と DWG の並列・単一パスオープンおよび `Drawing2D` 再構築（`docs/live.md`、#878）。
//!
//! モデル空間の大半を占める平面 6 種類の図形は `AcadDb` の `Object` や `Element<ModelPoint>` を経ずに
//! [`LiveShapes`] へ直接積み、残りの 17 種類の図形・拡張情報つきの図形・ペーパー空間・ブロック定義は
//! 非エンティティの殻 [`AcadDb`] の上で既存の [`Lifter`] をそのまま通すことで、診断・ID・ハッシュ・拡張情報の
//! 100% 一致を保ったままメモリ常駐量と開く時間を抑える。

mod pending;

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use pending::{BATCH_PAGES, PAGE_ROWS, Pending};

use zumen_base::{Budget, Cx, Diagnostic, Diagnostics, Key, Kind, Limit, Ref, codes};
use zumen_drawing::{
    Arrowhead, Axes, CalloutContent, CellContent, Color, ColorCode, ColorSystem, DimensionStyle,
    Draft, Drawing2D, Element, Layer, LayerId, LineType, LineTypeId, ModelPoint, ModelSpace,
    PaperLength, PointStyle, Position, Shape, Sheet, SheetId, SymbolDefinition, SymbolKey,
    TextStyle, Unit, id,
};
use zumen_live::display::Display;
use zumen_live::{DepGraph, RowId, SourceBytes, SpatialGrid, VersionedTable};
use zumen_sdk::{Codec, Error};

use super::super::single::{
    DxfFields, Follow, Followed, RawTag, SingleTables, followed_expected, followed_object,
    followed_tags,
};
use super::super::{
    BLOCK, EMPTY, FIT_DATA, Lifter, MODEL, Out, PaperFrame, SHEET, TABLE_CONTENT, amount, arrange,
    attached, charset, extension_work, header_limits, header_real, invalid, layout, layout_index,
    point_style, source, unit, work,
};
use super::backing::{
    DwgLiveBacking, DxfHeaderValues, DxfLiveBacking, DxfRecordMapper, LiveBacking,
};
use super::implicit::{ImplicitRefs, KindRefs, OpenNotes};
use super::shape::{
    GeneralRow, LiveKind, LiveRow, LiveShapes, NO_DEFINITION, PlainAttrib, PlainLifted,
    ROW_UNIQUE_HANDLE, SlotCatalog, display_style, general_from_element, try_lift_plain_attrib,
    try_lift_plain_entity,
};
use crate::charset::Charset;
use crate::db::{AcadDb, Data, Object, Origin, Section, Value, forget_values};
use crate::dwg::single::{DwgSingle, EntitySpace};
use crate::dwg::{Dwg, DwgFile};
use crate::dxf::live_io::{
    Next, Span, decode_record_slice, next_tag, read_record_spans_slice, split_points,
};
use crate::dxf::{BINARY_SENTINEL, Dxf, DxfFile, parse_value};

/// `AcadLive` が保持する常駐状態一式
#[derive(Clone)]
pub struct LiveState {
    /// 形式別のバックエンド
    pub backing: LiveBacking,
    /// モデル空間の行テーブル
    pub table: VersionedTable<LiveRow>,
    /// 図形データ列とプール
    pub shapes: LiveShapes,
    /// モデル空間の空間索引
    pub spatial: SpatialGrid,
    /// GPU 転送用の 64 要素単位ダーティ表示バッファー
    pub display: Display,
    /// ブロック定義・ハッチング境界・引出線注釈・連想寸法の依存グラフ
    pub dep: DepGraph,
    /// 別レコードの DIMASSOC・リアクターから得た依存元。単独の図形を読み直しても保つ。
    pub external_dep_sources: Arc<BTreeMap<RowId, Vec<RowId>>>,
    /// 画層・線種スロット表
    pub catalog: SlotCatalog,
    /// 暗黙の画層・線種への参照の位置
    pub implicit: ImplicitRefs,
    /// 単一図形書き戻し用の表情報
    pub single_tables: SingleTables,
    /// 文字符号化
    pub charset: Charset,
    /// 文書の長さの単位
    pub unit: Unit,
    /// `$LTSCALE`
    pub line_type_scale: Option<f64>,
    /// `$LWDEFAULT`
    pub default_line_weight: Option<PaperLength>,
    /// 既定色
    pub default_color: Option<Color>,
    /// 既定線種（`Continuous`）のスロット番号
    pub default_line_type_slot: Option<u32>,
    /// 寸法スタイル一覧
    pub dimension_styles: Vec<DimensionStyle>,
    /// `TABLES` に定義された文字スタイル一覧
    pub defined_text_styles: Vec<TextStyle>,
    /// ブロック定義一覧
    pub symbols: Vec<SymbolDefinition>,
    /// 内容または入れ子先の変更により再送が必要なブロック定義の添字集合
    pub dirty_symbols: BTreeSet<u32>,
    /// 小文字のブロック名から `symbols` の添字への対応
    pub block_defs_by_lower: BTreeMap<String, u32>,
    /// ペーパー空間の用紙一覧
    pub sheets: Vec<Sheet>,
    /// 点の表示スタイル
    pub point_style: Option<PointStyle>,
    /// モデル空間の `SORTENTSTABLE`
    pub model_sort_keys: Option<(Ref, BTreeMap<u64, u64>)>,
    /// モデル要素以外の拡張情報ひな形（画層・線種・ブロック定義・用紙要素）
    pub non_model_drafts: Vec<Draft>,
    /// ファイル全体で 0 以外のハンドル重複があるか
    pub duplicate_handles_global: bool,
    /// 複数要素が共有する出所 `Ref`
    pub shared_sources: BTreeSet<Ref>,
    /// モデル以外の要素（記号定義・用紙要素）の ID 集合
    pub non_model_element_ids: BTreeSet<String>,
    /// 名前付きブロック定義の要素 ID から `(定義添字, 要素添字)` への対応
    pub block_element_targets: BTreeMap<String, (u32, u32)>,
    /// 用紙の要素 ID から `(用紙添字, 要素添字)` への対応
    pub sheet_element_targets: BTreeMap<String, (u32, u32)>,
    /// 用紙ごとの、用紙の座標と元の図形の座標の対応（一括の `apply` が同じ文書から作るものと同じ）
    pub sheet_frames: BTreeMap<SheetId, PaperFrame>,
    /// ATTRIB などが所有者（330）か続くフラグで付く INSERT のハンドル（ENTITIES・BLOCKS セクション）。
    /// 一括の `apply` が数える属性の数と読み込んだ数が違いうるので、用紙・ブロック定義の要素の差分の経路では扱わない
    pub attached_inserts: BTreeSet<u64>,
    /// ハンドル一意索引に乗らないモデル要素の `ID -> RowId`
    pub non_unique_model_ids: BTreeMap<String, RowId>,
    /// ハンドル一意索引に乗らないモデル要素の `RowId -> ID`（定数／対数時間の逆引き用）
    pub non_unique_ids_by_row: BTreeMap<RowId, String>,
    /// `DICTIONARY` や `102` 組から参照されているハンドル集合
    pub forget_referenced_handles: BTreeSet<u64>,
    /// 非モデルオブジェクトおよび従属レコード（`ATTRIB`・`VERTEX`・`SEQEND`）の最大ハンドル
    pub non_table_max_handle: Option<u64>,
    /// DWG のオブジェクト復号・ヘッダー読み取りで出た変換前の診断（開いたファイルの位置）
    pub dwg_object_diagnostics: Vec<Diagnostic>,
    /// 開いたときの変換の診断（DWG のオブジェクト復号診断＋ヘッダー・表・図形の変換診断）。届いた順にまとめ、
    /// 場所は開いたファイルの位置。[`Self::to_drawing`] は、場所を今の保存の位置へ移して同じ順で報告する。
    /// 差分の経路は、診断の出た図形を変えず（[`Self::noted`]）、表とヘッダーも変えない。作り直す経路は開き直すので、変わった文書の診断になる
    pub lift_diagnostics: Vec<Diagnostic>,
    /// `lift_diagnostics` の場所が指すレコードの番号（開いたファイルの位置）
    pub noted_records: BTreeSet<u64>,
    /// `lift_diagnostics` の場所が指すハンドル
    pub noted_handles: BTreeSet<u64>,
    /// `lift_diagnostics` の場所が指す開始バイト位置（ハンドルを持たない DWG 診断用）
    pub noted_byte_starts: BTreeSet<u64>,
}

impl LiveState {
    /// 開いたときに変換の診断が出た図形か。出所のレコード・ハンドル・開始バイト位置が、診断の場所に含まれるかで見る。
    /// その図形を差分の経路で変えると保持した診断が古くなるので、変更は構造経路に任せる
    pub(crate) fn noted(&self, source: &Ref) -> bool {
        source
            .record()
            .is_some_and(|record| self.noted_records.contains(&record))
            || matches!(source.key(), Some(Key::Handle(handle)) if self.noted_handles.contains(&handle))
            || source
                .bytes()
                .is_some_and(|bytes| self.noted_byte_starts.contains(&bytes.start))
    }

    /// DWG のオブジェクト復号時の診断を、現在の保存後のバイト位置へ移して返す
    pub fn dwg_object_diagnostics(&self, cx: &mut Cx<'_>) -> Result<Vec<Diagnostic>, Error> {
        if self.dwg_object_diagnostics.is_empty() {
            return Ok(Vec::new());
        }
        let dwg_mapper = match &self.backing {
            LiveBacking::Dwg(dwg) => dwg.source_mapper(cx)?,
            _ => None,
        };
        let mut out = Vec::with_capacity(self.dwg_object_diagnostics.len());
        for diagnostic in &self.dwg_object_diagnostics {
            cx.check(Kind::Work, 1).map_err(|stop| {
                cx.report(stop.to_diagnostic());
                Error::Stopped(stop)
            })?;
            out.push(diagnostic.clone().map_places(|place| {
                if let Some(mapper) = &dwg_mapper {
                    mapper.remap_ref(place);
                }
            }));
        }
        Ok(out)
    }

    /// ファイル全体で生きている最大のハンドルを返す
    pub fn current_max_handle(&self) -> Option<u64> {
        match (self.non_table_max_handle, self.table.max_handle()) {
            (Some(a), Some(b)) => Some(a.max(b)),
            (Some(a), None) => Some(a),
            (None, b) => b,
        }
    }

    /// 1 行の出所と参照を、全体保存したときの番号に合わせて返す。
    pub(super) fn element_of_row(&self, row_id: RowId) -> Option<Element<ModelPoint>> {
        let row = self.table.row(row_id)?;
        let mapper = match &self.backing {
            LiveBacking::AsciiDxf(dxf) => dxf.record_mapper(),
            _ => None,
        };
        self.element_from_row(row, mapper.as_ref())
    }

    pub(super) fn element_with_source(
        &self,
        row_id: RowId,
        cx: &mut Cx<'_>,
    ) -> Result<Option<Element<ModelPoint>>, Error> {
        cx.check(Kind::Work, 1).map_err(|stop| {
            cx.report(stop.to_diagnostic());
            Error::Stopped(stop)
        })?;
        let Some(row) = self.table.row(row_id) else {
            return Ok(None);
        };
        self.shapes.check_element_copy(row, cx).map_err(|stop| {
            cx.report(stop.to_diagnostic().at(row.source_ref()));
            Error::Stopped(stop)
        })?;
        let dxf_mapper = match &self.backing {
            LiveBacking::AsciiDxf(dxf) => dxf.record_mapper_with_cx(cx)?,
            _ => None,
        };
        let Some(mut element) = self.element_from_row(row, dxf_mapper.as_ref()) else {
            return Ok(None);
        };
        if let LiveBacking::Dwg(dwg) = &self.backing
            && let Some(mapper) = dwg.source_mapper(cx)?
        {
            mapper.remap_element(&mut element);
        }
        Ok(Some(element))
    }

    fn element_from_row(
        &self,
        row: &LiveRow,
        dxf_mapper: Option<&DxfRecordMapper>,
    ) -> Option<Element<ModelPoint>> {
        let mut el = self.shapes.to_element(row, &self.catalog)?;
        if let Some(mapper) = dxf_mapper {
            el.source = row.source_ref();
            if let Some(record) = mapper.effective(row.record) {
                el.source = el.source.with_record(u64::from(record));
            }
            match row.kind {
                LiveKind::Insert => {
                    if let Shape::Symbol(sym) = &mut el.shape
                        && let Some(ins) = self.shapes.inserts.get(row.shape as usize)
                    {
                        for (i, attr) in sym.attributes.iter_mut().enumerate() {
                            if let Some(ar) = (ins.attrib_start as usize)
                                .checked_add(i)
                                .and_then(|index| self.shapes.pools.attribs.get(index))
                                && let Some(eff) = mapper.effective(ar.record)
                            {
                                attr.source = attr.source.clone().with_record(u64::from(eff));
                            }
                        }
                    }
                }
                LiveKind::General => {
                    if let Some(general) = self.shapes.generals.get(row.shape as usize) {
                        match &mut el.shape {
                            Shape::Symbol(sym) => {
                                let attribute_count = sym.attributes.len();
                                for (i, attr) in sym.attributes.iter_mut().enumerate() {
                                    if attr.source.record().is_some() {
                                        mapper.remap_ref(&mut attr.source);
                                    } else if general.raw_attrib_count as usize == attribute_count
                                        && let Some(&f_rec) = general.follower_records.get(i)
                                        && let Some(eff) = mapper.effective(f_rec)
                                    {
                                        attr.source =
                                            attr.source.clone().with_record(u64::from(eff));
                                    }
                                }
                            }
                            Shape::Leader(leader) => {
                                if let Some(ann) = &mut leader.annotation {
                                    if ann.record().is_some() {
                                        mapper.remap_ref(ann);
                                    } else if let Some(Key::Handle(h)) = ann.key()
                                        && let Some(ann_rid) = self.table.find_handle(h)
                                        && let Some(ann_row) = self.table.row(ann_rid)
                                        && let Some(eff) = mapper.effective(ann_row.record)
                                    {
                                        *ann =
                                            Ref::at_record(u64::from(eff)).with_key(Key::Handle(h));
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
            mapper.remap_shape_links(&mut el.shape);
        }
        Some(el)
    }

    /// 現在の常駐状態から [`Drawing2D`] を組み立てる
    pub fn to_drawing(&self, cx: &mut Cx<'_>) -> Result<Drawing2D, Error> {
        let layers = self.catalog.build_layers();
        let line_types = self.catalog.build_line_types();
        let default_line_type = self
            .default_line_type_slot
            .map(|slot| self.catalog.line_type_id(slot));

        let mut model_elements = Vec::with_capacity(self.table.live_count() as usize);
        let mut model_handles = Vec::with_capacity(self.table.live_count() as usize);
        let mut extensions = Vec::new();
        for (slot, &layer_slot) in self.catalog.active_layer_slots.iter().enumerate() {
            if let Some(entry) = self.catalog.layers.get(layer_slot as usize) {
                for mut draft in entry.drafts.clone() {
                    draft.at = Position::Layer(LayerId(slot as u32));
                    extensions.push(draft);
                }
            }
        }
        for (slot, &lt_slot) in self.catalog.active_line_type_slots.iter().enumerate() {
            if let Some(entry) = self.catalog.line_types.get(lt_slot as usize) {
                for mut draft in entry.drafts.clone() {
                    draft.at = Position::LineType(LineTypeId(slot as u32));
                    extensions.push(draft);
                }
            }
        }

        let dxf_mapper = match &self.backing {
            LiveBacking::AsciiDxf(dxf) => dxf.record_mapper(),
            _ => None,
        };
        let dwg_mapper = match &self.backing {
            LiveBacking::Dwg(dwg) => dwg.source_mapper(cx)?,
            _ => None,
        };
        for diagnostic in &self.lift_diagnostics {
            cx.report(diagnostic.clone().map_places(|place| {
                if let Some(mapper) = &dxf_mapper {
                    mapper.remap_ref(place);
                }
                if let Some(mapper) = &dwg_mapper {
                    mapper.remap_ref(place);
                }
            }));
        }
        for row_idx in 0..self.table.len() {
            let Some(row_id) = RowId::from_index(row_idx) else {
                break;
            };
            let Some(row) = self.table.row(row_id) else {
                continue;
            };
            let mut row_ref = row.source_ref();
            if let Some(mapper) = &dxf_mapper
                && let Some(eff) = mapper.effective(row.record)
            {
                row_ref = row_ref.with_record(u64::from(eff));
            }
            if let Some(mapper) = &dwg_mapper {
                mapper.remap_ref(&mut row_ref);
            }
            cx.check(Kind::Work, 1).map_err(|stop| {
                cx.report(stop.to_diagnostic().at(row_ref.clone()));
                Error::Stopped(stop)
            })?;
            let Some(mut el) = self.element_from_row(row, dxf_mapper.as_ref()) else {
                continue;
            };
            if let Some(mapper) = &dwg_mapper {
                mapper.remap_element(&mut el);
            }
            let model_index = model_elements.len();
            if row.kind == LiveKind::General
                && let Some(general) = self.shapes.generals.get(row.shape as usize)
            {
                for mut draft in general.drafts.clone() {
                    draft.at = Position::Model {
                        space: MODEL,
                        index: model_index,
                    };
                    extensions.push(draft);
                }
            }
            model_handles.push((model_index, row.raw_handle()));
            model_elements.push(el);
        }
        extensions.extend(self.non_model_drafts.iter().cloned());

        let order = compute_model_order(self.model_sort_keys.as_ref(), model_handles);
        let mut drawing = Drawing2D {
            layers,
            line_types,
            line_type_scale: self.line_type_scale,
            default_line_weight: self.default_line_weight,
            default_color: self.default_color,
            default_line_type,
            dimension_styles: self.dimension_styles.clone(),
            text_styles: self.defined_text_styles.clone(),
            symbols: self.symbols.clone(),
            sheets: self.sheets.clone(),
            point_style: self.point_style,
            ..Drawing2D::default()
        };
        if let Some(mapper) = &dxf_mapper {
            mapper.remap_non_model_sources(&mut drawing);
            for extension in &mut extensions {
                mapper.remap_ref(&mut extension.source);
            }
        }
        if let Some(mapper) = &dwg_mapper {
            mapper.remap_non_model_sources(&mut drawing);
            for extension in &mut extensions {
                mapper.remap_ref(&mut extension.source);
            }
        }
        drawing.spaces.push(ModelSpace {
            id: MODEL,
            unit: self.unit,
            axes: Axes::EastNorth,
            elements: model_elements,
            order,
        });
        drawing.add_named_text_styles(cx).map_err(|(stop, place)| {
            cx.report(stop.to_diagnostic().at(place));
            Error::Stopped(stop)
        })?;
        drawing.settle_extensions(extensions, cx).map_err(|stop| {
            cx.report(stop.to_diagnostic());
            Error::Stopped(stop)
        })?;
        Ok(drawing)
    }
}

/// 先頭バイトから DWG・バイナリ DXF・ASCII DXF を自動判定して開く
pub fn open_auto(src: &[u8], threads: usize, cx: &mut Cx<'_>) -> Result<LiveState, Error> {
    if is_dwg_magic(src) {
        open_dwg(src, threads, cx)
    } else {
        open_dxf(src, threads, cx)
    }
}

fn is_dwg_magic(src: &[u8]) -> bool {
    src.get(..2) == Some(b"AC")
        && src
            .get(2..6)
            .is_some_and(|digits| digits.iter().all(u8::is_ascii_digit))
}

#[derive(Clone)]
struct ScannedRecord {
    start: u32,
    end: u32,
    tag_count: u32,
    kind: Box<[u8]>,
    handle: Option<u64>,
    followed: Option<Followed>,
    has_code_100: bool,
    has_ext_or_paper: bool,
    work_extra: u32,
}

struct ChunkScan {
    invalid: bool,
    leading_tags: u32,
    records: Vec<ScannedRecord>,
}

fn is_plain_six(kind: &[u8]) -> bool {
    matches!(
        kind,
        b"LINE" | b"CIRCLE" | b"ARC" | b"LWPOLYLINE" | b"TEXT" | b"INSERT"
    )
}

fn scan_dxf_chunk(
    src: &[u8],
    chunk_start: usize,
    chunk_end: usize,
    is_first_chunk: bool,
) -> ChunkScan {
    let mut pos = chunk_start;
    let mut records: Vec<ScannedRecord> = Vec::new();
    let mut leading_tags = 0u32;

    let mut cur_start = 0u32;
    let mut cur_end = 0u32;
    let mut cur_tags = 0u32;
    let mut cur_kind: &[u8] = b"";
    let mut cur_has_record = false;
    let mut cur_handle: Option<u64> = None;
    let mut cur_has_code_100 = false;
    let mut cur_has_ext_or_paper = false;
    let mut cur_work_extra = 0u32;
    let mut raw_tags: Vec<RawTag<'_>> = Vec::new();
    let mut need_raw_tags = false;

    let flush_record = |records: &mut Vec<ScannedRecord>,
                        start: u32,
                        end: u32,
                        tag_count: u32,
                        kind: &[u8],
                        handle: Option<u64>,
                        raw_tags: &mut Vec<RawTag<'_>>,
                        has_100: bool,
                        has_ext: bool,
                        work_extra: u32| {
        let followed = if !raw_tags.is_empty() {
            let f = followed_tags(kind, raw_tags);
            raw_tags.clear();
            f
        } else {
            None
        };
        records.push(ScannedRecord {
            start,
            end,
            tag_count,
            kind: kind.into(),
            handle,
            followed,
            has_code_100: has_100,
            has_ext_or_paper: has_ext,
            work_extra,
        });
    };

    while pos < chunk_end {
        match next_tag(src, pos) {
            Next::End => break,
            Next::Invalid => {
                return ChunkScan {
                    invalid: true,
                    leading_tags: 0,
                    records: Vec::new(),
                };
            }
            Next::Tag(span) => {
                pos = span.end as usize;
                let val_bytes = src
                    .get(span.value as usize..span.value_end as usize)
                    .unwrap_or_default();
                if span.code == 0 || (is_first_chunk && !cur_has_record && records.is_empty()) {
                    if cur_has_record {
                        flush_record(
                            &mut records,
                            cur_start,
                            cur_end,
                            cur_tags,
                            cur_kind,
                            cur_handle,
                            &mut raw_tags,
                            cur_has_code_100,
                            cur_has_ext_or_paper,
                            cur_work_extra,
                        );
                    }
                    cur_has_record = true;
                    cur_start = span.start;
                    cur_end = span.end;
                    cur_tags = 1;
                    cur_kind = if span.code == 0 {
                        val_bytes.trim_ascii()
                    } else {
                        b""
                    };
                    cur_handle = None;
                    cur_has_code_100 = false;
                    cur_has_ext_or_paper = false;
                    cur_work_extra = 0;
                    need_raw_tags = matches!(
                        cur_kind,
                        b"INSERT" | b"POLYLINE" | b"ATTRIB" | b"VERTEX" | b"SEQEND"
                    );
                    if cur_kind == b"EOF" && span.code == 0 {
                        flush_record(
                            &mut records,
                            cur_start,
                            cur_end,
                            cur_tags,
                            cur_kind,
                            None,
                            &mut raw_tags,
                            false,
                            false,
                            0,
                        );
                        cur_has_record = false;
                        break;
                    }
                } else if !cur_has_record {
                    leading_tags = leading_tags.saturating_add(1);
                } else {
                    cur_end = span.end;
                    cur_tags = cur_tags.saturating_add(1);
                    let code = span.code;
                    let expected_h = if cur_kind == b"DIMSTYLE" { 105 } else { 5 };
                    if code == expected_h && cur_handle.is_none() {
                        cur_handle = crate::dxf::parse_hex(val_bytes.trim_ascii());
                    }
                    if code == 100 {
                        cur_has_code_100 = true;
                    }
                    if code >= 1000 {
                        cur_has_ext_or_paper = true;
                        cur_work_extra = cur_work_extra.saturating_add(1);
                    } else if matches!(code, 102 | 360 | 101) {
                        cur_has_ext_or_paper = true;
                    } else if code == 39 {
                        if crate::dxf::parse_real(val_bytes).is_none_or(|v| v != 0.0) {
                            cur_has_ext_or_paper = true;
                        }
                    } else if code == 67 && val_bytes.trim_ascii() == b"1" {
                        cur_has_ext_or_paper = true;
                    }
                    match cur_kind {
                        b"MTEXT" | b"ATTRIB" | b"ATTDEF" if code == 3 => {
                            cur_work_extra = cur_work_extra.saturating_add(1);
                        }
                        b"LWPOLYLINE" | b"SPLINE" if matches!(code, 10 | 11 | 40) => {
                            cur_work_extra = cur_work_extra.saturating_add(1);
                        }
                        b"ACAD_TABLE" if matches!(code, 141 | 142 | 171) => {
                            cur_work_extra = cur_work_extra.saturating_add(1);
                        }
                        b"LTYPE" if code == 49 => {
                            cur_work_extra = cur_work_extra.saturating_add(1);
                        }
                        _ => {}
                    }
                    if need_raw_tags && matches!(code, 5 | 66 | 330) {
                        raw_tags.push(RawTag {
                            code,
                            value: val_bytes,
                        });
                    }
                }
            }
        }
    }
    if cur_has_record {
        flush_record(
            &mut records,
            cur_start,
            cur_end,
            cur_tags,
            cur_kind,
            cur_handle,
            &mut raw_tags,
            cur_has_code_100,
            cur_has_ext_or_paper,
            cur_work_extra,
        );
    }
    ChunkScan {
        invalid: false,
        leading_tags,
        records,
    }
}

/// DXF（ASCII またはバイナリ）バイト列から常駐状態を開く
pub fn open_dxf(src: &[u8], threads: usize, cx: &mut Cx<'_>) -> Result<LiveState, Error> {
    if src.starts_with(BINARY_SENTINEL) || u32::try_from(src.len()).is_err() {
        let file = Dxf::read(src, cx)?;
        return open_from_dxf_file(file, cx);
    }
    cx.check(Kind::InputBytes, src.len() as u64)
        .map_err(|stop| {
            cx.report(stop.to_diagnostic());
            Error::Stopped(stop)
        })?;

    let parts = if cfg!(target_arch = "wasm32") {
        1
    } else {
        threads.max(1)
    };
    let cuts = split_points(src, parts);
    let chunk_count = cuts.len().saturating_sub(1);
    let scans: Vec<ChunkScan> = if !cfg!(target_arch = "wasm32") && chunk_count > 1 {
        std::thread::scope(|scope| {
            let mut handles = Vec::with_capacity(chunk_count);
            for i in 0..chunk_count {
                let c0 = cuts.get(i).copied().unwrap_or(0);
                let c1 = cuts.get(i + 1).copied().unwrap_or(src.len());
                handles.push(scope.spawn(move || scan_dxf_chunk(src, c0, c1, i == 0)));
            }
            handles
                .into_iter()
                .map(|h| {
                    h.join().unwrap_or(ChunkScan {
                        invalid: true,
                        leading_tags: 0,
                        records: Vec::new(),
                    })
                })
                .collect()
        })
    } else {
        vec![scan_dxf_chunk(src, 0, src.len(), true)]
    };

    let mut records: Vec<ScannedRecord> = Vec::new();
    let mut trailer_start = src.len() as u32;
    'merge: for scan in scans {
        if scan.invalid || scan.leading_tags > 0 {
            return open_ascii_via_dxf_file_after_input_check(src, cx);
        }
        for rec in scan.records {
            let is_eof = rec.kind.as_ref() == b"EOF"
                && src
                    .get(rec.start as usize..rec.end as usize)
                    .is_some_and(|b| matches!(next_tag(b, 0), Next::Tag(t) if t.code == 0));
            records.push(rec);
            if is_eof {
                if let Some(last) = records.last() {
                    trailer_start = last.end;
                }
                break 'merge;
            }
        }
    }

    // `Dxf::read` と同じ順序で `Records` と `Work` を検査する
    for rec in &records {
        if cx.check(Kind::Records, 1).is_err()
            || cx.check(Kind::Work, u64::from(rec.tag_count)).is_err()
        {
            // タグ単位の正確な停止位置を再現するため、このレコード内を先頭から走査する
            return fail_exact_dxf_read_budget(src, rec.start as usize, rec.end as usize, cx);
        }
    }

    // `DxfFile::table(cx, false)` 相当のレコード走査予算とセクション分類
    let mut shell_db = AcadDb::default();
    let mut section: Option<Section> = None;
    let mut entities_end_record: Option<u32> = None;
    let mut seen_entities_end = false;
    let mut header_record: Option<u32> = None;
    let mut header_spans: Box<[Span]> = Box::new([]);
    let mut header_known_starts: BTreeMap<Vec<u8>, usize> = BTreeMap::new();
    let mut entity_indices: Vec<u32> = Vec::new();
    let mut any_entity_code_100 = false;
    let mut seen_handles: BTreeSet<u64> = BTreeSet::new();
    let mut duplicate_handles_global = false;
    let mut ref_entity_handles: BTreeSet<u64> = BTreeSet::new();

    for (idx, rec) in records.iter().enumerate() {
        let rec_u32 = idx as u32;
        cx.check(Kind::Work, 1).map_err(|stop| {
            cx.report(stop.to_diagnostic().at(Ref::at_record(idx as u64)));
            Error::Stopped(stop)
        })?;
        match rec.kind.as_ref() {
            b"SECTION" => {
                let sec = decode_section_name(src, rec.start as usize..rec.end as usize);
                if sec == Section::Other(b"HEADER".to_vec()) {
                    parse_dxf_header_record(
                        src,
                        rec.start as usize..rec.end as usize,
                        header_record.is_none(),
                        &mut shell_db,
                        &mut header_spans,
                        &mut header_known_starts,
                    );
                    if header_record.is_none() {
                        header_record = Some(rec_u32);
                    }
                }
                section = Some(sec);
            }
            b"ENDSEC" => {
                if section == Some(Section::Entities) && !seen_entities_end {
                    entities_end_record = Some(rec_u32);
                    seen_entities_end = true;
                }
                section = None;
            }
            b"EOF" => break,
            _ => {
                let Some(sec) = &section else {
                    continue;
                };
                if let Some(h) = rec.handle
                    && h != 0
                    && !seen_handles.insert(h)
                {
                    duplicate_handles_global = true;
                }
                if *sec == Section::Entities {
                    any_entity_code_100 |= rec.has_code_100;
                    if matches!(rec.kind.as_ref(), b"HATCH" | b"LEADER")
                        && let Some((kind, values)) =
                            decode_dxf_slice(src, rec.start as usize..rec.end as usize)
                    {
                        collect_entity_ref_handles(&kind, &values, &mut ref_entity_handles);
                    }
                    entity_indices.push(rec_u32);
                } else if let Some((kind, values)) =
                    decode_dxf_slice(src, rec.start as usize..rec.end as usize)
                {
                    collect_entity_ref_handles(&kind, &values, &mut ref_entity_handles);
                    shell_db.push(Object {
                        kind,
                        section: sec.clone(),
                        values,
                        origin: Origin::Dxf { record: idx },
                    });
                }
            }
        }
    }

    if !ref_entity_handles.is_empty() {
        for &rec_u32 in &entity_indices {
            if let Some(h) = records.get(rec_u32 as usize).and_then(|r| r.handle)
                && ref_entity_handles.contains(&h)
                && shell_db.get(h).is_none()
            {
                shell_db.push(Object {
                    kind: Vec::new(),
                    section: Section::Entities,
                    values: vec![Value {
                        code: 5,
                        data: Data::Handle(h),
                    }],
                    origin: Origin::Dxf {
                        record: rec_u32 as usize,
                    },
                });
            }
        }
    }

    let base_header = DxfHeaderValues {
        extmin: shell_db.variable(b"$EXTMIN").map(<[Value]>::to_vec),
        extmax: shell_db.variable(b"$EXTMAX").map(<[Value]>::to_vec),
        handseed: shell_db.variable(b"$HANDSEED").map(<[Value]>::to_vec),
        pdmode: shell_db.variable(b"$PDMODE").map(<[Value]>::to_vec),
        pdsize: shell_db.variable(b"$PDSIZE").map(<[Value]>::to_vec),
    };

    // `ENTITIES` の親子（`ATTRIB`・`VERTEX`・`SEQEND`）を軽量情報だけで対応づける
    let (att_map, owned_set) = attach_scanned_entities(&entity_indices, &records);

    // 最初のペーパー空間図形の所有者を探す（`current_mm` と `Sheet.source` 用）
    let first_paper_owner = entity_indices.iter().find_map(|&rec_u32| {
        let rec = records.get(rec_u32 as usize)?;
        if !rec.has_ext_or_paper {
            return None;
        }
        let (_, vals) = decode_dxf_slice(src, rec.start as usize..rec.end as usize)?;
        let is_paper = vals
            .iter()
            .find(|v| v.code == 67)
            .is_some_and(|v| v.data == Data::Int(1));
        if !is_paper {
            return None;
        }
        crate::db::owner_of_values(&vals)
    });

    let version_bytes =
        shell_db
            .variable(b"$ACADVER")
            .and_then(|vals| match vals.first().map(|v| &v.data) {
                Some(Data::Text(t)) => Some(t.trim_ascii().to_vec()),
                _ => None,
            });
    let code_page_bytes =
        shell_db
            .variable(b"$DWGCODEPAGE")
            .and_then(|vals| match vals.first().map(|v| &v.data) {
                Some(Data::Text(t)) => Some(t.trim_ascii().to_vec()),
                _ => None,
            });
    let cs = charset(version_bytes.as_deref(), code_page_bytes.as_deref());

    // 平面 6 種類の対象を並列に持ち上げる
    let plain_tasks: Vec<(usize, u32)> = entity_indices
        .iter()
        .copied()
        .enumerate()
        .filter(|&(ent_pos, rec_u32)| {
            if owned_set.contains(&ent_pos) {
                return false;
            }
            let Some(rec) = records.get(rec_u32 as usize) else {
                return false;
            };
            if rec.has_ext_or_paper || !is_plain_six(&rec.kind) {
                return false;
            }
            if let Some(children) = att_map.get(&ent_pos) {
                for &child_pos in children {
                    let Some(&c_rec) = entity_indices.get(child_pos) else {
                        return false;
                    };
                    let Some(c_info) = records.get(c_rec as usize) else {
                        return false;
                    };
                    if c_info.has_ext_or_paper {
                        return false;
                    }
                }
            }
            true
        })
        .collect();

    let lift_one =
        |(ent_pos, rec_u32): (usize, u32), tags: &mut Vec<_>| -> (usize, Option<PlainLifted>) {
            let Some(rec) = records.get(rec_u32 as usize) else {
                return (ent_pos, None);
            };
            let mut attribs = Vec::new();
            let mut raw_attrib_count = 0u32;
            if rec.kind.as_ref() == b"INSERT"
                && let Some(children) = att_map.get(&ent_pos)
            {
                raw_attrib_count = children.len() as u32;
                attribs.reserve(children.len());
                for &child_pos in children {
                    let Some(&c_rec_u32) = entity_indices.get(child_pos) else {
                        return (ent_pos, None);
                    };
                    let Some(c_rec) = records.get(c_rec_u32 as usize) else {
                        return (ent_pos, None);
                    };
                    let Some(pa) = lift_plain_attrib_from_dxf(
                        src,
                        c_rec.start as usize..c_rec.end as usize,
                        cs,
                        c_rec_u32,
                        tags,
                    ) else {
                        return (ent_pos, None);
                    };
                    attribs.push(pa);
                }
            }
            let plain = lift_plain_entity_from_dxf(
                src,
                rec.start as usize..rec.end as usize,
                &rec.kind,
                cs,
                rec_u32,
                attribs,
                raw_attrib_count,
                tags,
            );
            (ent_pos, plain)
        };

    let fill_pages = |first_page: usize, pages: &mut [Option<Vec<Option<PlainLifted>>>]| {
        let mut tags = Vec::with_capacity(24);
        for (page_offset, page) in pages.iter_mut().enumerate() {
            let Some(page) = page else {
                continue;
            };
            let start = (first_page + page_offset) * PAGE_ROWS;
            let end = start.saturating_add(page.len());
            let first = plain_tasks.partition_point(|&(position, _)| position < start);
            let last = plain_tasks.partition_point(|&(position, _)| position < end);
            for &task in plain_tasks.get(first..last).unwrap_or_default() {
                let (position, item) = lift_one(task, &mut tags);
                if let Some(slot) = page.get_mut(position - start) {
                    *slot = item;
                }
            }
        }
    };

    // 全件の復号結果を保持せず、最大 64 ページ（16,384 入力位置）ずつ復号する。
    let entity_count = entity_indices.len();
    let page_count = entity_count.div_ceil(PAGE_ROWS);
    let lift_batch = |first_page: usize, workers: usize| -> Vec<Option<Vec<Option<PlainLifted>>>> {
        let last_page = first_page.saturating_add(BATCH_PAGES).min(page_count);
        let mut pages: Vec<Option<Vec<Option<PlainLifted>>>> =
            (first_page..last_page).map(|_| None).collect();
        let start = first_page * PAGE_ROWS;
        let end = last_page * PAGE_ROWS;
        let first = plain_tasks.partition_point(|&(position, _)| position < start);
        let last = plain_tasks.partition_point(|&(position, _)| position < end);
        for &(position, _) in plain_tasks.get(first..last).unwrap_or_default() {
            let page_index = position / PAGE_ROWS;
            let len = (entity_count - page_index * PAGE_ROWS).min(PAGE_ROWS);
            if let Some(page) = pages.get_mut(page_index - first_page) {
                page.get_or_insert_with(|| (0..len).map(|_| None).collect());
            }
        }
        if !cfg!(target_arch = "wasm32") && workers > 1 && last - first >= 256 {
            let chunk_size = pages.len().div_ceil(workers).max(1);
            let failed = std::thread::scope(|scope| {
                let mut handles = Vec::new();
                let fill = &fill_pages;
                for (index, chunk) in pages.chunks_mut(chunk_size).enumerate() {
                    let start = index * chunk_size;
                    let end = start + chunk.len();
                    handles.push((
                        start..end,
                        scope.spawn(move || fill(first_page + start, chunk)),
                    ));
                }
                handles
                    .into_iter()
                    .filter_map(|(range, handle)| handle.join().is_err().then_some(range))
                    .collect::<Vec<_>>()
            });
            for range in failed {
                if let Some(failed_pages) = pages.get_mut(range) {
                    for page in failed_pages {
                        *page = None;
                    }
                }
            }
        } else {
            fill_pages(first_page, &mut pages);
        }
        pages
    };
    // 3 スレッド以上では、1 本のスレッドが残りのスレッドで先の塊を復号し、入力順の処理と重ねる。
    // 2 スレッドでは、復号を 1 スレッドに減らすより、2 スレッドで復号してから処理するほうが速い。
    // 受け渡しを待つ塊は 1 つまでに限る。塊の値は復号の時期によらないので、結果は変わらない。
    let pipelined = !cfg!(target_arch = "wasm32") && parts > 2 && plain_tasks.len() >= 256;
    let lift_workers = if pipelined { parts - 1 } else { parts };

    let source_bytes = SourceBytes::compress_slice_with_cx(src, parts, cx).map_err(|stop| {
        cx.report(stop.to_diagnostic());
        Error::Stopped(stop)
    })?;
    let record_starts: Vec<u32> = records.iter().map(|r| r.start).collect();
    let source_record_count = record_starts.len() as u32;

    let mut dxf_backing = DxfLiveBacking {
        src: source_bytes,
        record_starts,
        trailer_start,
        entities_end_record,
        header_record,
        header_spans,
        header_known_starts,
        base_header: base_header.clone(),
        header: base_header,
        overlay: BTreeMap::new(),
        gone: BTreeSet::new(),
        edited_values: BTreeMap::new(),
        next_born_record: source_record_count,
        non_model_max_handle: None,
        current_max_handle: None,
    };

    let finish_inputs = EntityPassInputs {
        count: entity_indices.len(),
        owned_set: &owned_set,
        att_map: &att_map,
        first_paper_owner,
        any_entity_code_100,
        duplicate_handles_global,
        dwg_seed: None,
    };
    let lift_batch = &lift_batch;
    let mut state = std::thread::scope(|scope| {
        let (sender, receiver) = if pipelined {
            let (sender, receiver) = std::sync::mpsc::sync_channel(1);
            (Some(sender), Some(receiver))
        } else {
            (None, None)
        };
        let producer = sender.map(|sender| {
            scope.spawn(move || {
                let mut first_page = 0;
                while first_page < page_count {
                    let pages = lift_batch(first_page, lift_workers);
                    let count = pages.len();
                    if count == 0 || sender.send((first_page, pages)).is_err() {
                        break;
                    }
                    first_page += count;
                }
            })
        });
        let mut receiver = receiver;
        let mut lifted_plains = Pending::with_len(entity_count);
        let mut filled_until = 0;
        let mut take_plain = |ent_pos: usize| {
            if ent_pos < filled_until {
                return lifted_plains.take_ordered(ent_pos);
            }
            if let Some(batches) = &receiver {
                while filled_until <= ent_pos {
                    let Ok((first_page, pages)) = batches.recv() else {
                        break;
                    };
                    filled_until = (first_page + pages.len()) * PAGE_ROWS;
                    if filled_until > ent_pos {
                        lifted_plains.install(first_page, pages);
                    }
                }
                if filled_until > ent_pos {
                    return lifted_plains.take_ordered(ent_pos);
                }
                // 先に復号するスレッドが止まったときは、この場で復号する。
                receiver = None;
            }
            let first_page = ent_pos / PAGE_ROWS;
            let pages = lift_batch(first_page, lift_workers);
            filled_until = (first_page + pages.len()) * PAGE_ROWS;
            lifted_plains.install(first_page, pages);
            lifted_plains.take_ordered(ent_pos)
        };
        let result = finish_live_open(
            &shell_db,
            finish_inputs,
            &mut take_plain,
            |ent_pos| {
                let rec_u32 = *entity_indices.get(ent_pos)?;
                let rec = records.get(rec_u32 as usize)?;
                let at = match rec.handle {
                    Some(h) => Ref::at_record(u64::from(rec_u32)).with_key(Key::Handle(h)),
                    None => Ref::at_record(u64::from(rec_u32)),
                };
                Some((rec_u32, u64::from(1 + rec.work_extra), at, rec.handle))
            },
            |ent_pos, _cx| {
                let rec_u32 = *entity_indices.get(ent_pos)?;
                let rec = records.get(rec_u32 as usize)?;
                let (kind, values) = decode_dxf_slice(src, rec.start as usize..rec.end as usize)?;
                Some((
                    rec_u32,
                    Object {
                        kind,
                        section: Section::Entities,
                        values,
                        origin: Origin::Dxf {
                            record: rec_u32 as usize,
                        },
                    },
                ))
            },
            LiveBacking::AsciiDxf(DxfLiveBacking {
                src: SourceBytes::default(),
                record_starts: Vec::new(),
                trailer_start: 0,
                entities_end_record: None,
                header_record: None,
                header_spans: Box::new([]),
                header_known_starts: BTreeMap::new(),
                base_header: DxfHeaderValues::default(),
                header: DxfHeaderValues::default(),
                overlay: BTreeMap::new(),
                gone: BTreeSet::new(),
                edited_values: BTreeMap::new(),
                next_born_record: 0,
                non_model_max_handle: None,
                current_max_handle: None,
            }),
            cx,
        );
        drop(take_plain);
        drop(receiver);
        if let Some(producer) = producer {
            let _ = producer.join();
        }
        result
    })?;
    dxf_backing.non_model_max_handle = state.non_table_max_handle;
    dxf_backing.current_max_handle = state.current_max_handle();
    state.backing = LiveBacking::AsciiDxf(dxf_backing);
    Ok(state)
}

/// DWG バイト列から常駐状態を開く
pub fn open_dwg(src: &[u8], threads: usize, cx: &mut Cx<'_>) -> Result<LiveState, Error> {
    let file = Dwg::read(src, cx)?;
    open_from_dwg_file(file, threads, cx)
}

/// 読み込み済みの [`DwgFile`] から常駐状態を組む
pub fn open_from_dwg_file(
    file: DwgFile,
    threads: usize,
    cx: &mut Cx<'_>,
) -> Result<LiveState, Error> {
    let mut pre_lift_collector = Diagnostics::with_max_places(usize::MAX);
    let single_res = DwgSingle::from_file(
        &file,
        &mut Cx::new(cx.budget, &mut pre_lift_collector, cx.cancel),
    );
    let header_diags = pre_lift_collector.into_vec();
    for d in &header_diags {
        cx.report(d.clone());
    }
    let single = single_res?;
    let mut pre_lift_collector = Diagnostics::with_max_places(usize::MAX);
    for d in header_diags {
        pre_lift_collector.report(d);
    }

    let mut shell_db = AcadDb::default();
    single.populate_header_variables(&mut shell_db);

    let version_bytes = single.version().to_vec();
    let cs = charset(Some(version_bytes.as_slice()), single.code_page_name());
    let total_entries = file.entry_count();
    let parts = if cfg!(target_arch = "wasm32") {
        1
    } else {
        threads.max(1)
    };

    // `DwgFile::objects(cx)` と同じく、クラス件数分の Work を先に数える
    for _ in 0..file.class_count() {
        cx.check(Kind::Work, 1).map_err(|stop| {
            cx.report(stop.to_diagnostic());
            Error::Stopped(stop)
        })?;
    }

    // 平面 6 種類の単体モデル図形を多数並列復号するときに要素ごとのヒープ確保を増やさないため、インラインで保持する
    #[allow(clippy::large_enum_variant)]
    enum DwgEntryPayload {
        PlainModel {
            record: u32,
            work_amount: u64,
            handle: Option<u64>,
            has_100: bool,
            plain: PlainLifted,
        },
        EntityObj {
            record: u32,
            obj: Object,
        },
        ShellObj {
            obj: Object,
        },
        Empty,
    }

    #[allow(clippy::large_enum_variant)]
    enum WorkerItem {
        Ready {
            work_delta: u64,
            decoded_delta: u64,
            payload: DwgEntryPayload,
        },
        Replay,
    }

    let classify_obj = |idx: usize, opt_obj: Option<Object>| -> DwgEntryPayload {
        let Some(obj) = opt_obj else {
            return DwgEntryPayload::Empty;
        };
        if obj.section != Section::Entities {
            return DwgEntryPayload::ShellObj { obj };
        }
        let rec_u32 = idx as u32;
        let is_plain_candidate = matches!(
            obj.kind.as_slice(),
            b"LINE" | b"CIRCLE" | b"ARC" | b"LWPOLYLINE" | b"TEXT" | b"INSERT"
        ) && (obj.kind != b"INSERT"
            || !obj
                .values
                .iter()
                .any(|v| v.code == 66 && matches!(v.data, Data::Int(n) if n != 0)))
            && !has_extension_or_paper_values(&obj.values);
        if is_plain_candidate
            && let Some(plain) = try_lift_plain_entity(
                &obj.kind,
                &&obj,
                Some(obj.values.as_slice()),
                cs,
                rec_u32,
                obj.origin,
                obj.handle(),
                Vec::new(),
                0,
            )
        {
            let work_amount = amount(&obj);
            let handle = obj.handle();
            let has_100 = obj.values.iter().any(|v| v.code == 100);
            return DwgEntryPayload::PlainModel {
                record: rec_u32,
                work_amount,
                handle,
                has_100,
                plain,
            };
        }
        DwgEntryPayload::EntityObj {
            record: rec_u32,
            obj,
        }
    };

    let can_spend_both = |budget: &Budget, work_delta: u64, decoded_delta: u64| -> bool {
        if budget.exceeded().is_some() {
            return false;
        }
        let ok_work = match budget.limits().get(Kind::Work) {
            Limit::Unlimited => true,
            Limit::Finite(max) => budget
                .used(Kind::Work)
                .checked_add(work_delta)
                .is_some_and(|req| req <= max),
        };
        let ok_dec = match budget.limits().get(Kind::DecodedBytes) {
            Limit::Unlimited => true,
            Limit::Finite(max) => budget
                .used(Kind::DecodedBytes)
                .checked_add(decoded_delta)
                .is_some_and(|req| req <= max),
        };
        ok_work && ok_dec
    };

    struct DwgEntSlot {
        record: u32,
        work_amount: u64,
        handle: Option<u64>,
        followed: Option<Followed>,
        obj: Option<Object>,
    }

    // 全体分の `DwgEntryPayload` を一度に保持せず、1024 件単位の窓で復号してすぐ消費し、最大確保量を抑える
    const DWG_DECODE_WINDOW: usize = 1024;
    let mut ent_slots: Vec<DwgEntSlot> = Vec::new();
    let mut lifted_plains = Pending::with_len(0);
    let mut any_entity_code_100 = false;
    let mut seen_handles: BTreeSet<u64> = BTreeSet::new();
    let mut duplicate_handles_global = false;
    let mut ref_entity_handles: BTreeSet<u64> = BTreeSet::new();

    let model_owner = single.model_owner();
    let mut consume_payload = |item: DwgEntryPayload| match item {
        DwgEntryPayload::PlainModel {
            record,
            work_amount,
            handle,
            has_100,
            plain,
        } => {
            if let Some(h) = handle
                && h != 0
                && !seen_handles.insert(h)
            {
                duplicate_handles_global = true;
            }
            any_entity_code_100 |= has_100;
            let followed = match &plain.geom {
                super::shape::PlainGeom::Insert { followed, .. } => Some(Followed {
                    kind: Follow::Insert { opens: *followed },
                    handle,
                    owner: model_owner,
                }),
                _ => None,
            };
            ent_slots.push(DwgEntSlot {
                record,
                work_amount,
                handle,
                followed,
                obj: None,
            });
            lifted_plains.push(Some(plain));
        }
        DwgEntryPayload::EntityObj { record, obj } => {
            collect_entity_ref_handles(&obj.kind, &obj.values, &mut ref_entity_handles);
            let h = obj.handle();
            if let Some(hv) = h
                && hv != 0
                && !seen_handles.insert(hv)
            {
                duplicate_handles_global = true;
            }
            any_entity_code_100 |= obj.values.iter().any(|v| v.code == 100);
            let work_amount = amount(&obj);
            let followed = followed_object(&obj);
            ent_slots.push(DwgEntSlot {
                record,
                work_amount,
                handle: h,
                followed,
                obj: Some(obj),
            });
            lifted_plains.push(None);
        }
        DwgEntryPayload::ShellObj { obj } => {
            collect_entity_ref_handles(&obj.kind, &obj.values, &mut ref_entity_handles);
            if let Some(hv) = obj.handle()
                && hv != 0
                && !seen_handles.insert(hv)
            {
                duplicate_handles_global = true;
            }
            shell_db.push(obj);
        }
        DwgEntryPayload::Empty => {}
    };

    let decode_sequential = |idx: usize,
                             cx: &mut Cx<'_>,
                             pre_lift: &mut Diagnostics|
     -> Result<DwgEntryPayload, Error> {
        let mut local_diag = Diagnostics::with_max_places(usize::MAX);
        let res = single.decode_record_open(
            &file,
            idx,
            &mut Cx::new(cx.budget, &mut local_diag, cx.cancel),
        );
        let diags = local_diag.into_vec();
        for d in &diags {
            cx.report(d.clone());
        }
        let opt_obj = res?;
        for d in diags {
            pre_lift.report(d);
        }
        Ok(classify_obj(idx, opt_obj))
    };

    for w_start in (0..total_entries).step_by(DWG_DECODE_WINDOW) {
        let w_end = (w_start + DWG_DECODE_WINDOW).min(total_entries);
        let w_len = w_end - w_start;
        if !cfg!(target_arch = "wasm32") && parts > 1 && w_len >= 64 {
            let snap_budget = cx.budget.clone();
            let cancel = cx.cancel;
            let chunk_sz = w_len.div_ceil(parts);
            let window_items: Vec<WorkerItem> = std::thread::scope(|scope| {
                let mut handles = Vec::new();
                for sub_start in (w_start..w_end).step_by(chunk_sz) {
                    let sub_end = (sub_start + chunk_sz).min(w_end);
                    let snap_ref = &snap_budget;
                    let single_ref = &single;
                    let file_ref = &file;
                    handles.push(scope.spawn(move || {
                        let mut out = Vec::with_capacity(sub_end - sub_start);
                        for idx in sub_start..sub_end {
                            if cancel.stop().is_some() {
                                out.push(WorkerItem::Replay);
                                continue;
                            }
                            let mut local_budget = snap_ref.clone();
                            let mut local_diag = Diagnostics::with_max_places(usize::MAX);
                            let mut local_cx = Cx::new(&mut local_budget, &mut local_diag, cancel);
                            match single_ref.decode_record_open(file_ref, idx, &mut local_cx) {
                                Ok(opt_obj) if local_diag.is_empty() => {
                                    let work_delta = local_budget
                                        .used(Kind::Work)
                                        .saturating_sub(snap_ref.used(Kind::Work));
                                    let decoded_delta = local_budget
                                        .used(Kind::DecodedBytes)
                                        .saturating_sub(snap_ref.used(Kind::DecodedBytes));
                                    out.push(WorkerItem::Ready {
                                        work_delta,
                                        decoded_delta,
                                        payload: classify_obj(idx, opt_obj),
                                    });
                                }
                                _ => out.push(WorkerItem::Replay),
                            }
                        }
                        out
                    }));
                }
                let mut merged = Vec::with_capacity(w_len);
                for h in handles {
                    if let Ok(part) = h.join() {
                        merged.extend(part);
                    }
                }
                merged
            });
            for (offset, item) in window_items.into_iter().enumerate() {
                let idx = w_start + offset;
                match item {
                    WorkerItem::Ready {
                        work_delta,
                        decoded_delta,
                        payload,
                    } if cx.cancel.stop().is_none()
                        && can_spend_both(cx.budget, work_delta, decoded_delta) =>
                    {
                        let _ = cx.budget.spend(Kind::Work, work_delta);
                        let _ = cx.budget.spend(Kind::DecodedBytes, decoded_delta);
                        consume_payload(payload);
                    }
                    _ => {
                        let payload = decode_sequential(idx, cx, &mut pre_lift_collector)?;
                        consume_payload(payload);
                    }
                }
            }
        } else {
            for idx in w_start..w_end {
                let payload = decode_sequential(idx, cx, &mut pre_lift_collector)?;
                consume_payload(payload);
            }
        }
    }

    if !ref_entity_handles.is_empty() {
        for s in &ent_slots {
            if let Some(h) = s.handle
                && ref_entity_handles.contains(&h)
                && shell_db.get(h).is_none()
            {
                let origin =
                    file.entry_summary(s.record as usize)
                        .map_or(Origin::New, |(_, _, _, span)| Origin::Dwg {
                            offset: span.start,
                            end: span.end,
                        });
                shell_db.push(Object {
                    kind: Vec::new(),
                    section: Section::Entities,
                    values: vec![Value {
                        code: 5,
                        data: Data::Handle(h),
                    }],
                    origin,
                });
            }
        }
    }

    let roles: Vec<Option<Followed>> = ent_slots.iter().map(|s| s.followed).collect();
    let (att_map, owned_set) = attach_roles(&roles);

    // もし `PlainModel` の `INSERT`（66=0）に対して所有者 330 で `ATTRIB` がぶら下がっていた場合は、
    // その `ATTRIB` も高速経路または一般経路へ統合する
    for (&parent_pos, children) in &att_map {
        if children.is_empty() {
            continue;
        }
        if let Some(Some(plain)) = lifted_plains.get_mut(parent_pos)
            && let super::shape::PlainGeom::Insert {
                attribs,
                raw_attrib_count,
                ..
            } = &mut plain.geom
        {
            *raw_attrib_count = children.len() as u32;
            let mut all_ok = true;
            let mut lifted_attrs = Vec::with_capacity(children.len());
            for &c_pos in children {
                let Some(c_slot) = ent_slots.get(c_pos) else {
                    all_ok = false;
                    break;
                };
                let Some(c_obj) = c_slot.obj.as_ref() else {
                    all_ok = false;
                    break;
                };
                if has_extension_or_paper_values(&c_obj.values) {
                    all_ok = false;
                    break;
                }
                if let Some(pa) =
                    try_lift_plain_attrib(&c_obj, cs, c_slot.record, c_obj.origin, c_obj.handle())
                {
                    lifted_attrs.push(pa);
                } else {
                    all_ok = false;
                    break;
                }
            }
            if all_ok {
                *attribs = lifted_attrs;
            } else if let Some(p_slot) = ent_slots.get_mut(parent_pos) {
                // 一般経路へ戻すため親レコードを共有予算で再復号する
                p_slot.obj = single.decode_record_with_cx(&file, p_slot.record as usize, cx)?;
                if let Some(slot_opt) = lifted_plains.get_mut(parent_pos) {
                    *slot_opt = None;
                }
            }
        }
    }

    let first_paper_owner = ent_slots.iter().find_map(|s| {
        let obj = s.obj.as_ref()?;
        matches!(obj.first(67), Some(Data::Int(1)))
            .then(|| obj.owner())
            .flatten()
    });

    let entry_spans: Vec<(u64, u64)> = (0..total_entries)
        .map(|idx| {
            file.entry_summary(idx).map_or((0, 0), |(_, _, _, span)| {
                (span.start as u64, span.end as u64)
            })
        })
        .collect();

    let handseed = single.handle_seed();
    let source_records = file.entry_count() as u32;
    let dwg_backing = DwgLiveBacking {
        base_file: file,
        single,
        patched: BTreeMap::new(),
        born: BTreeMap::new(),
        gone: BTreeSet::new(),
        removed_base_model_handles: BTreeSet::new(),
        handseed,
        point_mode_override: None,
        point_size_override: None,
        current_max_handle: None,
        next_born_record: source_records,
        steps: Vec::new(),
        step_cursor: 0,
    };

    let finish_inputs = EntityPassInputs {
        count: ent_slots.len(),
        owned_set: &owned_set,
        att_map: &att_map,
        first_paper_owner,
        any_entity_code_100,
        duplicate_handles_global,
        dwg_seed: Some(handseed),
    };
    let _ = EntitySpace::Model;
    let mut state = finish_live_open_with_pre_diags(
        &shell_db,
        finish_inputs,
        |ent_pos| lifted_plains.take_ordered(ent_pos),
        |ent_pos| {
            let s = ent_slots.get(ent_pos)?;
            let &(start, end) = entry_spans.get(s.record as usize).unwrap_or(&(0, 0));
            let mut src_ref = Ref::at_bytes(start, end);
            if let Some(h) = s.handle {
                src_ref = src_ref.with_key(Key::Handle(h));
            }
            Some((s.record, s.work_amount, src_ref, s.handle))
        },
        |ent_pos, _cx| {
            let s = ent_slots.get(ent_pos)?;
            Some((s.record, s.obj.clone()?))
        },
        LiveBacking::Dwg(dwg_backing),
        pre_lift_collector.into_vec(),
        cx,
    )?;
    let max_h = state.current_max_handle();
    if let LiveBacking::Dwg(dwg) = &mut state.backing {
        dwg.current_max_handle = max_h;
    }
    Ok(state)
}

/// 読み込み済みの [`DxfFile`]（バイナリ DXF またはフォールバック）から常駐状態を組む
pub fn open_from_dxf_file(file: DxfFile, cx: &mut Cx<'_>) -> Result<LiveState, Error> {
    let db = file.table(cx, false)?;
    open_from_db_and_backing(&db, LiveBacking::BinaryDxf(file), None, cx)
}

fn open_ascii_via_dxf_file_after_input_check(
    src: &[u8],
    cx: &mut Cx<'_>,
) -> Result<LiveState, Error> {
    // `InputBytes` は `open_dxf` の入口で計上済みのため、`InputBytes` だけ無制限にしつつほかの全単位の上限・既使用量・
    // 中断トークン・診断収集先を呼び出し元と共有して `Dxf::read` を 1 回だけ走らせ、増分を親予算へ反映する。
    let sub_limits = (*cx.budget.limits()).with(Kind::InputBytes, Limit::Unlimited);
    let mut sub_budget = Budget::new(sub_limits);
    for kind in Kind::ALL {
        if kind != Kind::InputBytes {
            let used = cx.budget.used(kind);
            if used > 0 {
                let _ = sub_budget.spend(kind, used);
            }
        }
    }
    let read_res = Dxf::read(src, &mut Cx::new(&mut sub_budget, cx.diag, cx.cancel));
    for kind in Kind::ALL {
        if kind != Kind::InputBytes && kind != Kind::Depth {
            let delta = sub_budget.used(kind).saturating_sub(cx.budget.used(kind));
            if delta > 0 {
                let _ = cx.budget.spend(kind, delta);
            }
        }
    }
    if let Some(ex) = sub_budget.exceeded()
        && cx.budget.exceeded().is_none()
    {
        let need = ex.requested.saturating_sub(cx.budget.used(ex.kind)).max(1);
        let _ = cx.budget.spend(ex.kind, need);
    }
    let file = read_res?;
    open_from_dxf_file(file, cx)
}

fn open_from_db_and_backing(
    db: &AcadDb,
    backing: LiveBacking,
    dwg_seed: Option<Option<u64>>,
    cx: &mut Cx<'_>,
) -> Result<LiveState, Error> {
    let mut shell_db = AcadDb::default();
    for (name, vals) in db.variables() {
        shell_db.set_variable(name, vals.to_vec());
    }
    let mut entities: Vec<(u32, Object)> = Vec::new();
    let mut seen_handles = BTreeSet::new();
    let mut duplicate_handles_global = false;
    let mut any_entity_code_100 = false;
    let mut ref_entity_handles: BTreeSet<u64> = BTreeSet::new();

    for (idx, obj) in db.objects().iter().enumerate() {
        if let Some(h) = obj.handle()
            && h != 0
            && !seen_handles.insert(h)
        {
            duplicate_handles_global = true;
        }
        collect_entity_ref_handles(&obj.kind, &obj.values, &mut ref_entity_handles);
        if obj.section == Section::Entities {
            any_entity_code_100 |= obj.values.iter().any(|v| v.code == 100);
            let rec_u32 = match obj.origin {
                Origin::Dxf { record } => record as u32,
                Origin::Dwg { .. } | Origin::New => idx as u32,
            };
            entities.push((rec_u32, obj.clone()));
        } else {
            shell_db.push(obj.clone());
        }
    }
    if !ref_entity_handles.is_empty() {
        for (_, obj) in &entities {
            if let Some(h) = obj.handle()
                && ref_entity_handles.contains(&h)
                && shell_db.get(h).is_none()
            {
                shell_db.push(Object {
                    kind: Vec::new(),
                    section: Section::Entities,
                    values: vec![Value {
                        code: 5,
                        data: Data::Handle(h),
                    }],
                    origin: obj.origin,
                });
            }
        }
    }
    let ent_refs: Vec<&Object> = entities.iter().map(|(_, o)| o).collect();
    let (att_map, owned_set) = attached(&ent_refs);
    let first_paper_owner = ent_refs
        .iter()
        .find(|o| matches!(o.first(67), Some(Data::Int(1))))
        .and_then(|o| o.owner());
    let finish_inputs = EntityPassInputs {
        count: entities.len(),
        owned_set: &owned_set,
        att_map: &att_map,
        first_paper_owner,
        any_entity_code_100,
        duplicate_handles_global,
        dwg_seed,
    };
    finish_live_open(
        &shell_db,
        finish_inputs,
        |_| None,
        |pos| {
            let (rec_u32, obj) = entities.get(pos)?;
            Some((*rec_u32, amount(obj), source(obj), obj.handle()))
        },
        |pos, _cx| entities.get(pos).cloned(),
        backing,
        cx,
    )
}

/// 付く図形（ATTRIB・VERTEX）を持つ図形のハンドル。ENTITIES セクションは読み込みの対応から、BLOCKS セクションは
/// 一括の `apply` の属性の数え方（`attached`）と同じ規則で求める
fn attached_insert_handles(
    shell_db: &AcadDb,
    inputs: &EntityPassInputs<'_>,
    entity_meta: &impl Fn(usize) -> Option<(u32, u64, Ref, Option<u64>)>,
    cx: &mut Cx<'_>,
) -> Result<BTreeSet<u64>, Error> {
    let mut handles = BTreeSet::new();
    let place = Ref::default();
    for (&parent, children) in inputs.att_map {
        work(cx, 1, &place)?;
        if !children.is_empty()
            && let Some((_, _, _, Some(handle))) = entity_meta(parent)
        {
            handles.insert(handle);
        }
    }
    let mut blocks = Vec::new();
    for object in shell_db.objects() {
        work(cx, 1, &place)?;
        if object.section == Section::Blocks {
            blocks.push(object);
        }
    }
    let (attributes, _) = attached(&blocks);
    for (parent, children) in attributes {
        if !children.is_empty()
            && let Some(handle) = blocks.get(parent).and_then(|object| object.handle())
        {
            handles.insert(handle);
        }
    }
    Ok(handles)
}

struct EntityPassInputs<'a> {
    count: usize,
    owned_set: &'a BTreeSet<usize>,
    att_map: &'a BTreeMap<usize, Vec<usize>>,
    first_paper_owner: Option<u64>,
    any_entity_code_100: bool,
    duplicate_handles_global: bool,
    dwg_seed: Option<Option<u64>>,
}

/// 図面モデルへの変換と同じ順で常駐の状態を作る。変換の診断を別の収集先に集めて呼び出し元へ同じ順で移し、
/// 状態にも持つ（[`LiveState::lift_diagnostics`]）。届いた順にまとめる収集先へ移すので、件数・場所・順は
/// 1 件ずつ報告したときと同じになり、そのような収集先への報告は予算を使わない
fn finish_live_open(
    shell_db: &AcadDb,
    inputs: EntityPassInputs<'_>,
    take_plain: impl FnMut(usize) -> Option<PlainLifted>,
    entity_meta: impl Fn(usize) -> Option<(u32, u64, Ref, Option<u64>)>,
    decode_general: impl Fn(usize, &mut Cx<'_>) -> Option<(u32, Object)>,
    backing: LiveBacking,
    cx: &mut Cx<'_>,
) -> Result<LiveState, Error> {
    finish_live_open_with_pre_diags(
        shell_db,
        inputs,
        take_plain,
        entity_meta,
        decode_general,
        backing,
        Vec::new(),
        cx,
    )
}

#[allow(clippy::too_many_arguments)]
fn finish_live_open_with_pre_diags(
    shell_db: &AcadDb,
    inputs: EntityPassInputs<'_>,
    take_plain: impl FnMut(usize) -> Option<PlainLifted>,
    entity_meta: impl Fn(usize) -> Option<(u32, u64, Ref, Option<u64>)>,
    decode_general: impl Fn(usize, &mut Cx<'_>) -> Option<(u32, Object)>,
    backing: LiveBacking,
    pre_lift_diags: Vec<Diagnostic>,
    cx: &mut Cx<'_>,
) -> Result<LiveState, Error> {
    let mut lifted = Diagnostics::with_max_places(usize::MAX);
    let result = lift_live_state(
        shell_db,
        inputs,
        take_plain,
        entity_meta,
        decode_general,
        backing,
        &mut Cx::new(cx.budget, &mut lifted, cx.cancel),
    );
    let lift_diagnostics = lifted.into_vec();
    for diagnostic in &lift_diagnostics {
        cx.report(diagnostic.clone());
    }
    let mut state = result?;
    let mut combined = Diagnostics::with_max_places(usize::MAX);
    for d in &pre_lift_diags {
        combined.report(d.clone());
    }
    for d in lift_diagnostics {
        combined.report(d);
    }
    let all_lift_diagnostics = combined.into_vec();
    for place in all_lift_diagnostics.iter().flat_map(Diagnostic::places) {
        if let Some(record) = place.record() {
            state.noted_records.insert(record);
        }
        if let Some(Key::Handle(handle)) = place.key() {
            state.noted_handles.insert(handle);
        }
        if let Some(bytes) = place.bytes() {
            state.noted_byte_starts.insert(bytes.start);
        }
    }
    state.dwg_object_diagnostics = pre_lift_diags;
    state.lift_diagnostics = all_lift_diagnostics;
    Ok(state)
}

fn lift_live_state(
    shell_db: &AcadDb,
    inputs: EntityPassInputs<'_>,
    mut take_plain: impl FnMut(usize) -> Option<PlainLifted>,
    entity_meta: impl Fn(usize) -> Option<(u32, u64, Ref, Option<u64>)>,
    decode_general: impl Fn(usize, &mut Cx<'_>) -> Option<(u32, Object)>,
    backing: LiveBacking,
    cx: &mut Cx<'_>,
) -> Result<LiveState, Error> {
    let doc_unit = unit(shell_db, cx);
    let min = header_limits(shell_db, b"$PLIMMIN", cx);
    let max = header_limits(shell_db, b"$PLIMMAX", cx);
    let version =
        shell_db
            .variable(b"$ACADVER")
            .and_then(|values| match values.first().map(|v| &v.data) {
                Some(Data::Text(text)) => Some(text.trim_ascii()),
                _ => None,
            });
    let code_page = shell_db.variable(b"$DWGCODEPAGE").and_then(|values| {
        match values.first().map(|v| &v.data) {
            Some(Data::Text(text)) => Some(text.trim_ascii()),
            _ => None,
        }
    });
    let layouts = layout_index(shell_db, cx)?;
    let cs = charset(version, code_page);
    let base_lifter = Lifter {
        layouts: &layouts,
        sheet: SHEET,
        charset: cs,
        fit_data: version.is_some_and(|v| v >= FIT_DATA),
        table_content: version.is_some_and(|v| v >= TABLE_CONTENT),
        version: version.map(|v| String::from_utf8_lossy(v).into_owned()),
        mm: doc_unit.millimeters(),
        unit: doc_unit,
        origin: min.unwrap_or_default(),
    };
    let current_mm = inputs
        .first_paper_owner
        .and_then(|handle| base_lifter.layout_of(shell_db, handle))
        .and_then(|(_, lo)| layout::plot_mm(lo));
    let lifter = Lifter {
        mm: current_mm.or(base_lifter.mm),
        ..base_lifter
    };

    let mut out = Out::default();
    let mut layer_drafts: BTreeMap<u32, Vec<Draft>> = BTreeMap::new();
    let mut lt_drafts: BTreeMap<u32, Vec<Draft>> = BTreeMap::new();

    for kind in [&b"LTYPE"[..], b"LAYER", b"DIMSTYLE", b"STYLE"] {
        for object in shell_db.objects() {
            if object.section != Section::Tables || object.kind != kind {
                continue;
            }
            let src_ref = source(object);
            work(cx, 1, &src_ref)?;
            let lifted = match kind {
                b"LTYPE" => {
                    lifter.line_type_record(shell_db, object, src_ref.clone(), &mut out, cx)
                }
                b"LAYER" => lifter.layer_record(object, src_ref.clone(), &mut out),
                b"DIMSTYLE" => {
                    lifter.dimension_style_record(shell_db, object, src_ref.clone(), &mut out)
                }
                _ => lifter.text_style_record(object, src_ref.clone(), &mut out),
            };
            if let Err(diagnostic) = lifted {
                cx.report((*diagnostic).at(src_ref));
                continue;
            }
            let at = match kind {
                b"LTYPE" => match out.line_types.last() {
                    Some(lt) => Position::LineType(lt.id),
                    None => continue,
                },
                b"LAYER" => match out.layers.last() {
                    Some(layer) => Position::Layer(layer.id),
                    None => continue,
                },
                _ => continue,
            };
            extension_work(shell_db, object, cx)?;
            let before_ext = out.extensions.len();
            lifter.dictionary(shell_db, object, at, &mut out, cx);
            let added_drafts: Vec<Draft> = out.extensions.drain(before_ext..).collect();
            match at {
                Position::Layer(id) => {
                    layer_drafts.entry(id.0).or_default().extend(added_drafts);
                }
                Position::LineType(id) => {
                    lt_drafts.entry(id.0).or_default().extend(added_drafts);
                }
                _ => {}
            }
        }
    }

    let mut catalog = SlotCatalog::from_tables(out.layers.clone(), out.line_types.clone());
    for (slot, drafts) in layer_drafts {
        if let Some(entry) = catalog.layers.get_mut(slot as usize) {
            entry.drafts = drafts;
        }
    }
    for (slot, drafts) in lt_drafts {
        if let Some(entry) = catalog.line_types.get_mut(slot as usize) {
            entry.drafts = drafts;
        }
    }

    let layer_table = catalog.layers.len() as u32;
    let line_type_table = catalog.line_types.len() as u32;
    let mut layer_notes = OpenNotes::default();
    let mut line_type_notes = OpenNotes::default();
    let mut pending_new: Option<(u32, usize, usize)> = None;
    let mut raw_layers: Vec<u32> = Vec::new();
    let mut raw_line_types: Vec<u32> = Vec::new();
    let mut row_layers: Vec<u32> = Vec::new();
    let mut row_line_types: Vec<u32> = Vec::new();

    let mut shapes = LiveShapes::default();
    let mut rows: Vec<LiveRow> = Vec::new();
    let mut row_reactors: Vec<(usize, u64)> = Vec::new();
    let mut row_ext_dicts: Vec<(usize, u64)> = Vec::new();
    let empty_block_defs: BTreeMap<String, u32> = BTreeMap::new();
    let mut forget_referenced_handles: BTreeSet<u64> = BTreeSet::new();
    let mut non_table_max_handle: Option<u64> = None;

    for obj in shell_db.objects() {
        if obj.section == Section::Entities {
            continue;
        }
        if let Some(h) = obj.handle() {
            non_table_max_handle = Some(non_table_max_handle.map_or(h, |m| m.max(h)));
        }
        collect_forget_handles(&obj.kind, &obj.values, &mut forget_referenced_handles);
    }

    // `out` と `catalog` の暗黙画層・線種を相互同期するクロージャ
    let sync_catalog_to_out = |catalog: &SlotCatalog, out: &mut Out| {
        while out.layers.len() < catalog.layers.len() {
            let idx = out.layers.len();
            if let Some(entry) = catalog.layers.get(idx) {
                let id = LayerId(idx as u32);
                out.by_name
                    .entry(entry.name.to_ascii_lowercase())
                    .or_insert(id);
                out.layers.push(Layer {
                    id,
                    follows_placement: entry.name == "0",
                    name: entry.name.clone(),
                    state: None,
                    style: None,
                    description: None,
                    source: Ref::default(),
                });
            }
        }
        while out.line_types.len() < catalog.line_types.len() {
            let idx = out.line_types.len();
            if let Some(entry) = catalog.line_types.get(idx) {
                let id = LineTypeId(idx as u32);
                out.line_type_names
                    .entry(entry.name.to_ascii_lowercase())
                    .or_insert(id);
                out.line_types.push(LineType {
                    id,
                    name: entry.name.clone(),
                    definition: None,
                    source: Ref::default(),
                });
            }
        }
    };
    let sync_out_to_catalog = |out: &Out, catalog: &mut SlotCatalog, pinned_outside_model: bool| {
        while catalog.layers.len() < out.layers.len() {
            let idx = catalog.layers.len();
            if let Some(layer) = out.layers.get(idx) {
                let slot = catalog.intern_layer(&layer.name);
                if pinned_outside_model && let Some(entry) = catalog.layers.get_mut(slot as usize) {
                    entry.pinned_outside_model = true;
                }
            }
        }
        while catalog.line_types.len() < out.line_types.len() {
            let idx = catalog.line_types.len();
            if let Some(lt) = out.line_types.get(idx) {
                let slot = catalog.intern_line_type(&lt.name);
                if pinned_outside_model
                    && let Some(entry) = catalog.line_types.get_mut(slot as usize)
                {
                    entry.pinned_outside_model = true;
                }
            }
        }
    };

    for ent_pos in 0..inputs.count {
        if let Some((record, layers_before, line_types_before)) = pending_new.take() {
            layer_notes.note_new(layer_table, layers_before, catalog.layers.len(), record);
            line_type_notes.note_new(
                line_type_table,
                line_types_before,
                catalog.line_types.len(),
                record,
            );
        }
        let Some((rec_u32, work_amt, src_ref, handle_opt)) = entity_meta(ent_pos) else {
            continue;
        };
        pending_new = Some((rec_u32, catalog.layers.len(), catalog.line_types.len()));
        work(cx, work_amt, &src_ref)?;
        if inputs.owned_set.contains(&ent_pos) {
            if let Some(h) = handle_opt {
                non_table_max_handle = Some(non_table_max_handle.map_or(h, |m| m.max(h)));
            }
            continue;
        }
        if let Some(plain) = take_plain(ent_pos) {
            if let super::shape::PlainGeom::Insert { attribs, .. } = &plain.geom {
                for a in attribs {
                    if let Some(ah) = a.handle {
                        non_table_max_handle = Some(non_table_max_handle.map_or(ah, |m| m.max(ah)));
                    }
                }
            }
            let row = shapes.push_plain(plain, &mut catalog, true, false, &empty_block_defs);
            rows.push(row);
            continue;
        }

        // 一般図形・ペーパー空間・拡張つき図形・診断が出る図形
        sync_catalog_to_out(&catalog, &mut out);
        let Some((_, obj)) = decode_general(ent_pos, cx) else {
            continue;
        };
        collect_forget_handles(&obj.kind, &obj.values, &mut forget_referenced_handles);
        extension_work(shell_db, &obj, cx)?;
        let mut child_objs: Vec<(u32, Object)> = Vec::new();
        if let Some(children) = inputs.att_map.get(&ent_pos) {
            for &c_pos in children {
                if let Some(c_pair) = decode_general(c_pos, cx) {
                    if let Some(ch) = c_pair.1.handle() {
                        non_table_max_handle = Some(non_table_max_handle.map_or(ch, |m| m.max(ch)));
                    }
                    collect_forget_handles(
                        &c_pair.1.kind,
                        &c_pair.1.values,
                        &mut forget_referenced_handles,
                    );
                    child_objs.push(c_pair);
                }
            }
        }
        let child_refs: Vec<&Object> = child_objs.iter().map(|(_, o)| o).collect();
        let before_model = out.model.len();
        let before_paper = out.paper.len();
        let before_ext = out.extensions.len();
        if let Err(diagnostic) =
            lifter.entity(shell_db, &obj, src_ref.clone(), &child_refs, &mut out, cx)
        {
            cx.report((*diagnostic).at(src_ref));
            sync_out_to_catalog(&out, &mut catalog, false);
            raw_slot_names(
                &obj,
                &child_objs,
                cs,
                &mut catalog,
                &mut raw_layers,
                &mut raw_line_types,
            );
            note_maybe_refs(
                &raw_layers,
                &raw_line_types,
                rec_u32,
                &mut layer_notes,
                &mut line_type_notes,
            );
            if let Some(h) = obj.handle() {
                non_table_max_handle = Some(non_table_max_handle.map_or(h, |m| m.max(h)));
            }
            continue;
        }
        let added_paper = out.paper.len() > before_paper;
        sync_out_to_catalog(&out, &mut catalog, added_paper);
        raw_slot_names(
            &obj,
            &child_objs,
            cs,
            &mut catalog,
            &mut raw_layers,
            &mut raw_line_types,
        );
        if out.model.len() <= before_model {
            note_maybe_refs(
                &raw_layers,
                &raw_line_types,
                rec_u32,
                &mut layer_notes,
                &mut line_type_notes,
            );
        }
        if out.model.len() > before_model {
            if let Some(el) = out.model.pop() {
                let _ = out.model_handles.pop();
                let drafts: Vec<Draft> = out.extensions.drain(before_ext..).collect();
                let follower_records: Vec<u32> = child_objs.iter().map(|(r, _)| *r).collect();
                let raw_attrib_count = if obj.kind == b"INSERT" {
                    child_objs.len() as u32
                } else {
                    0
                };
                let has_102_or_360 = obj.values.iter().any(|v| matches!(v.code, 102 | 360));
                if has_102_or_360 {
                    collect_reactor_and_ext_dict(
                        rows.len(),
                        &obj.values,
                        &mut row_reactors,
                        &mut row_ext_dicts,
                    );
                }
                let general: GeneralRow = general_from_element(
                    el,
                    &out.layers,
                    &out.line_types,
                    &mut catalog,
                    &obj.kind,
                    follower_records,
                    raw_attrib_count,
                    drafts,
                );
                let row = shapes.push_general(
                    rec_u32,
                    obj.origin,
                    obj.handle(),
                    true,
                    has_102_or_360,
                    false,
                    general,
                );
                shapes.slot_refs(&row, &mut row_layers, &mut row_line_types);
                if !multiset_covers(&row_layers, &raw_layers) {
                    layer_notes.opaque = true;
                }
                if !multiset_covers(&row_line_types, &raw_line_types) {
                    line_type_notes.opaque = true;
                }
                rows.push(row);
            }
        } else if let Some(h) = obj.handle() {
            non_table_max_handle = Some(non_table_max_handle.map_or(h, |m| m.max(h)));
        }
    }

    if let Some((record, layers_before, line_types_before)) = pending_new.take() {
        layer_notes.note_new(layer_table, layers_before, catalog.layers.len(), record);
        line_type_notes.note_new(
            line_type_table,
            line_types_before,
            catalog.line_types.len(),
            record,
        );
    }

    sync_catalog_to_out(&catalog, &mut out);
    let ([model_keys, paper_keys], block_keys) = lifter.sort_keys(shell_db, cx)?;
    let symbols = lifter.definitions(shell_db, &block_keys, &mut out, cx)?;
    let (layers_before, line_types_before) = (catalog.layers.len(), catalog.line_types.len());
    sync_out_to_catalog(&out, &mut catalog, true);
    layer_notes.note_after_entities(layer_table, layers_before, catalog.layers.len());
    line_type_notes.note_after_entities(
        line_type_table,
        line_types_before,
        catalog.line_types.len(),
    );
    // ブロック定義の図形は、持ち上げに失敗しても名前を足していることがある（線種は画層より先に読む）
    let block_named = if catalog.layers.len() > layer_table as usize
        || catalog.line_types.len() > line_type_table as usize
    {
        block_slot_names(shell_db, cs, &mut catalog, cx)?
    } else {
        Default::default()
    };
    for sym in &symbols {
        for el in &sym.space.elements {
            pin_element_slots(el, &mut catalog);
        }
        for def in &sym.attributes {
            if let Some(entry) = catalog.layers.get_mut(def.attribute.layer.0 as usize) {
                entry.pinned_outside_model = true;
            }
            if let zumen_drawing::Inherit::Own(lt) = def.attribute.style.line_type
                && let Some(entry) = catalog.line_types.get_mut(lt.0 as usize)
            {
                entry.pinned_outside_model = true;
            }
        }
    }
    for (_, _, sheet, _) in &out.layouts {
        for el in &sheet.elements {
            pin_element_slots(el, &mut catalog);
        }
    }
    for el in &out.paper {
        pin_element_slots(el, &mut catalog);
    }
    let implicit = build_implicit_refs(
        &catalog,
        &shapes,
        &rows,
        [layer_table, line_type_table],
        [&layer_notes, &line_type_notes],
        &block_named,
    );

    let mut block_defs_by_lower: BTreeMap<String, u32> = BTreeMap::new();
    let mut blocks_exact: BTreeMap<String, String> = BTreeMap::new();
    for (idx, sym) in symbols.iter().enumerate() {
        if sym.key.namespace == BLOCK {
            let lower = sym.key.name.to_lowercase();
            block_defs_by_lower
                .entry(lower.clone())
                .or_insert(idx as u32);
            blocks_exact
                .entry(lower)
                .or_insert_with(|| sym.key.name.clone());
        }
    }

    // `INSERT` 行の `definition` 添字を解決する
    for row in &rows {
        if row.kind == LiveKind::Insert
            && let Some(ins) = shapes.inserts.get(row.shape as usize)
        {
            let name = shapes.pools.get_string(ins.name);
            let def_idx = block_defs_by_lower
                .get(&name.to_lowercase())
                .copied()
                .unwrap_or(NO_DEFINITION);
            if def_idx != ins.definition
                && let Some(slot) = shapes.inserts.get_mut(row.shape as usize)
            {
                slot.definition = def_idx;
            }
        }
    }

    check_symbols_live(&rows, &shapes, &symbols, &out, cx)?;

    let line_type_scale = shell_db
        .variable(b"$LTSCALE")
        .and_then(|values| header_real(values, 40))
        .filter(|scale| scale.is_finite() && *scale > 0.0);
    let default_line_weight = shell_db.variable(b"$LWDEFAULT").and_then(|values| {
        match values.iter().find(|v| v.code == 370)?.data {
            Data::Int(width @ 0..=211) => Some(PaperLength(width as f64 / 100.0)),
            _ => None,
        }
    });
    let default_color = Some(Color {
        code: Some(ColorCode {
            system: ColorSystem::Aci,
            value: 7,
        }),
        rgb: None,
    });
    let default_line_type_slot = catalog
        .line_types
        .iter()
        .position(|lt| lt.name.eq_ignore_ascii_case("Continuous"))
        .map(|i| i as u32);
    let pt_style = point_style(shell_db, cx);
    if let Some((ref table_ref, _)) = model_keys
        && rows.iter().any(|r| r.raw_handle().is_none())
    {
        cx.report((*invalid(b"SORTENTSTABLE", 331)).at(table_ref.clone()));
    }

    let current_layout = inputs
        .first_paper_owner
        .and_then(|handle| lifter.layout_of(shell_db, handle));
    let has_paper = !out.paper.is_empty() || !out.views.is_empty();
    let mut sheets = Vec::new();
    if has_paper {
        let size = match (max, lifter.mm) {
            (Some([x, y]), Some(mm)) => {
                let [ox, oy] = lifter.origin;
                match super::super::finite::size(b"$PLIMMAX", [10, 20], [ox, oy], [x, y], mm) {
                    Ok(sz) => sz,
                    Err(diagnostic) => {
                        cx.report(*diagnostic);
                        None
                    }
                }
            }
            _ => None,
        };
        let order = arrange(paper_keys.as_ref(), out.sheet, cx);
        let name = current_layout.and_then(|(_, lo)| lifter.layout_info(lo).name);
        sheets.push(Sheet {
            id: SHEET,
            name,
            size,
            elements: out.paper,
            views: out.views,
            order,
            source: current_layout.map_or_else(Ref::default, |(_, lo)| source(lo)),
        });
    }
    let current_layout_idx = current_layout.map(|(index, _)| index);
    out.layouts.retain(|(_, layout_idx, _, _)| {
        !(has_paper && layout_idx.is_some() && *layout_idx == current_layout_idx)
    });
    out.layouts.sort_by_key(|(tab, _, _, _)| *tab);
    let filled = out
        .layouts
        .iter()
        .filter(|(_, _, sheet, _)| sheet.id != EMPTY)
        .count();
    let mut next_sheet = u32::try_from(filled).unwrap_or(u32::MAX).saturating_add(1);
    for (_, _, sheet, frame) in &mut out.layouts {
        if sheet.id == EMPTY {
            sheet.id = SheetId(next_sheet);
            next_sheet = next_sheet.saturating_add(1);
            if let Some(frame) = frame {
                out.frames.insert(sheet.id, *frame);
            }
        }
    }
    sheets.extend(out.layouts.into_iter().map(|(_, _, sheet, _)| sheet));
    let sheet_frames = std::mem::take(&mut out.frames);
    let attached_inserts = attached_insert_handles(shell_db, &inputs, &entity_meta, cx)?;

    // ハンドル重複・共有 `Ref`・非モデル要素 ID の検査
    // 100 万行でも速いよう、全ハンドルを整列して 2 回以上現れるものだけを集める。
    let mut all_handles: Vec<u64> = Vec::with_capacity(rows.len());
    let mut source_seen: BTreeSet<Ref> = BTreeSet::new();
    let mut shared_sources: BTreeSet<Ref> = BTreeSet::new();
    let mut non_model_element_ids: BTreeSet<String> = BTreeSet::new();
    let mut block_element_targets: BTreeMap<String, (u32, u32)> = BTreeMap::new();
    all_handles.extend(rows.iter().filter_map(LiveRow::raw_handle));
    let mut def_namer = id::DefinitionNamer::default();
    for (def_idx, sym) in symbols.iter().enumerate() {
        let place = def_namer.next(sym);
        let mut namer = id::Namer::new(&place);
        for (idx, el) in sym.space.elements.iter().enumerate() {
            if let Some(Key::Handle(h)) = el.source.key() {
                all_handles.push(h);
            }
            let elem_id = namer.next(idx, el);
            if !sym.anonymous
                && sym.external.is_none()
                && let Ok(d_u32) = u32::try_from(def_idx)
                && let Ok(e_u32) = u32::try_from(idx)
            {
                block_element_targets
                    .entry(elem_id.clone())
                    .or_insert((d_u32, e_u32));
            }
            non_model_element_ids.insert(elem_id);
        }
    }
    let mut sheet_element_targets: BTreeMap<String, (u32, u32)> = BTreeMap::new();
    let mut sheet_namer = id::SheetNamer::default();
    for (sheet_idx, sheet) in sheets.iter().enumerate() {
        let place = sheet_namer.next(sheet);
        let mut namer = id::Namer::new(&place);
        for (idx, el) in sheet.elements.iter().enumerate() {
            if let Some(Key::Handle(h)) = el.source.key() {
                all_handles.push(h);
            }
            let elem_id = namer.next(idx, el);
            if let Ok(s_u32) = u32::try_from(sheet_idx)
                && let Ok(e_u32) = u32::try_from(idx)
            {
                sheet_element_targets
                    .entry(elem_id.clone())
                    .or_insert((s_u32, e_u32));
            }
            non_model_element_ids.insert(elem_id);
        }
    }
    all_handles.sort_unstable();
    let repeated_handles: BTreeSet<u64> = all_handles
        .windows(2)
        .filter_map(|pair| match pair {
            [a, b] if a == b => Some(*a),
            _ => None,
        })
        .collect();
    drop(all_handles);
    let mut seen_origins: BTreeMap<String, u64> = BTreeMap::new();
    let mut non_unique_model_ids: BTreeMap<String, RowId> = BTreeMap::new();
    let mut non_unique_ids_by_row: BTreeMap<RowId, String> = BTreeMap::new();
    for (idx, row) in rows.iter_mut().enumerate() {
        let is_unique = row
            .raw_handle()
            .is_some_and(|h| !repeated_handles.contains(&h));
        if is_unique {
            continue;
        }
        let src_ref = row.source_ref();
        if src_ref != Ref::default() && !source_seen.insert(src_ref.clone()) {
            shared_sources.insert(src_ref.clone());
        }
        let orig = id::origin(&src_ref, idx);
        let repeat =
            if src_ref.key().is_none() && src_ref.record().is_none() && src_ref.bytes().is_none() {
                0
            } else {
                let count = seen_origins.entry(orig.clone()).or_insert(0);
                *count += 1;
                *count - 1
            };
        row.flags &= !ROW_UNIQUE_HANDLE;
        if let Some(h) = row.raw_handle() {
            non_table_max_handle = Some(non_table_max_handle.map_or(h, |m| m.max(h)));
        }
        if let Some(row_id) = RowId::from_index(idx) {
            let elem_id = match repeat {
                0 => format!("space/0/{orig}"),
                n => format!("space/0/{orig}/{n}"),
            };
            if let std::collections::btree_map::Entry::Vacant(slot) =
                non_unique_model_ids.entry(elem_id.clone())
            {
                slot.insert(row_id);
                non_unique_ids_by_row.insert(row_id, elem_id);
            }
        }
    }
    // 一意なハンドルを持つ出所は、ほかの要素と共有されない。
    for source in symbols
        .iter()
        .flat_map(|symbol| symbol.space.elements.iter().map(|element| &element.source))
        .chain(
            sheets
                .iter()
                .flat_map(|sheet| sheet.elements.iter().map(|element| &element.source)),
        )
    {
        if matches!(source.key(), Some(Key::Handle(handle)) if !repeated_handles.contains(&handle))
        {
            continue;
        }
        if source != &Ref::default() && !source_seen.insert(source.clone()) {
            shared_sources.insert(source.clone());
        }
    }
    drop(seen_origins);
    drop(repeated_handles);
    drop(source_seen);

    // 要素数予算と境界矩形の計算
    let total_elements = (rows.len() as u64)
        .saturating_add(sheets.iter().map(|s| s.elements.len() as u64).sum::<u64>());
    let _ = total_elements;
    let initial_rows: Vec<(LiveRow, [f64; 4], bool)> = rows
        .into_iter()
        .map(|row| {
            let bbox = shapes.row_bounds(&row, &symbols);
            (row, bbox, true)
        })
        .collect();

    let table = VersionedTable::from_rows_with_cx(initial_rows, cx).map_err(|stop| {
        cx.report(stop.to_diagnostic());
        Error::Stopped(stop)
    })?;
    let spatial = SpatialGrid::build_with_cx(table.bounds_column(), cx).map_err(|stop| {
        cx.report(stop.to_diagnostic());
        Error::Stopped(stop)
    })?;
    let mut display = Display::with_rows_with_cx(cx, table.len()).map_err(|stop| {
        cx.report(stop.to_diagnostic());
        Error::Stopped(stop)
    })?;
    for idx in 0..table.len() {
        let (counts, style) = RowId::from_index(idx)
            .and_then(|rid| table.row(rid))
            .map_or(([0; 4], [0; 2]), |r| {
                (
                    shapes.display_counts(r, &symbols),
                    display_style(&catalog.layer_remap, r),
                )
            });
        display
            .push_with_cx(cx, counts, style, |writer| {
                if let Some(rid) = RowId::from_index(idx)
                    && let Some(r) = table.row(rid)
                {
                    shapes.write_display(r, &symbols, &catalog.layer_remap, writer);
                }
            })
            .map_err(|stop| {
                cx.report(stop.to_diagnostic());
                Error::Stopped(stop)
            })?;
    }
    display.clear_dirty();

    let (dep, external_dep_sources) = build_dep_graph(
        shell_db,
        &table,
        &shapes,
        &symbols,
        &block_defs_by_lower,
        &row_reactors,
        &row_ext_dicts,
        cx,
    )?;

    // `SingleTables` を組む
    let mut table_names_map: BTreeMap<&'static [u8], BTreeMap<String, String>> = BTreeMap::new();
    let mut model_owner: Option<u64> = None;
    for obj in shell_db.objects() {
        if obj.section == Section::Entities {
            continue;
        }
        let Some(&tkey) = [
            b"LAYER".as_slice(),
            b"LTYPE",
            b"STYLE",
            b"BLOCK_RECORD",
            b"DIMSTYLE",
        ]
        .iter()
        .find(|&&k| obj.kind == k) else {
            continue;
        };
        let Some(Data::Text(raw_name)) = obj.first(2) else {
            continue;
        };
        let Some(decoded) = cs.decode(raw_name) else {
            continue;
        };
        if tkey == b"BLOCK_RECORD" && decoded.eq_ignore_ascii_case("*Model_Space") {
            model_owner = obj.handle();
        }
        table_names_map
            .entry(tkey)
            .or_default()
            .insert(decoded.to_lowercase(), decoded);
    }
    let ver_vec = version.map_or_else(Vec::new, <[u8]>::to_vec);
    let markers = (inputs.dwg_seed.is_none() && ver_vec.as_slice() >= b"AC1012")
        || inputs.any_entity_code_100;
    let single_tables = SingleTables {
        model_owner,
        markers,
        version: ver_vec,
        charset: cs,
        dwg_seed: inputs.dwg_seed,
        names: Arc::new(table_names_map),
        blocks: Arc::new(blocks_exact),
    };

    Ok(LiveState {
        backing,
        table,
        shapes,
        spatial,
        display,
        dep,
        external_dep_sources: Arc::new(external_dep_sources),
        catalog,
        implicit,
        single_tables,
        charset: cs,
        unit: doc_unit,
        line_type_scale,
        default_line_weight,
        default_color,
        default_line_type_slot,
        dimension_styles: out.dimension_styles,
        defined_text_styles: out.text_styles,
        symbols,
        dirty_symbols: BTreeSet::new(),
        block_defs_by_lower,
        sheets,
        point_style: pt_style,
        model_sort_keys: model_keys,
        non_model_drafts: out.extensions,
        duplicate_handles_global: inputs.duplicate_handles_global,
        shared_sources,
        non_model_element_ids,
        block_element_targets,
        sheet_element_targets,
        sheet_frames,
        attached_inserts,
        non_unique_model_ids,
        non_unique_ids_by_row,
        forget_referenced_handles,
        non_table_max_handle,
        dwg_object_diagnostics: Vec::new(),
        lift_diagnostics: Vec::new(),
        noted_records: BTreeSet::new(),
        noted_handles: BTreeSet::new(),
        noted_byte_starts: BTreeSet::new(),
    })
}

fn compute_model_order(
    keys: Option<&(Ref, BTreeMap<u64, u64>)>,
    items: Vec<(usize, Option<u64>)>,
) -> Vec<usize> {
    let Some((_, sort)) = keys else {
        return items.into_iter().map(|(item, _)| item).collect();
    };
    if items.iter().any(|(_, handle)| handle.is_none()) {
        return items.into_iter().map(|(item, _)| item).collect();
    }
    let mut keyed: Vec<(u64, usize)> = items
        .into_iter()
        .map(|(item, handle)| {
            let h = handle.unwrap_or_default();
            (sort.get(&h).copied().unwrap_or(h), item)
        })
        .collect();
    keyed.sort_by_key(|&(k, _)| k);
    keyed.into_iter().map(|(_, item)| item).collect()
}

/// 図形と属性の画層（8）・線種（6）の名前が指すスロットを集める（`Lifter` が名前を足す元）
///
/// `Lifter` は POLYLINE の VERTEX と SEQEND の画層・線種を読まないので、従属レコードは ATTRIB だけを見る。
/// スロットの名前と綴りの違う参照があれば、スロットに印を付ける（先頭の参照が変わると名前が変わりうる）。
fn raw_slot_names(
    obj: &Object,
    children: &[(u32, Object)],
    cs: Charset,
    catalog: &mut SlotCatalog,
    layers: &mut Vec<u32>,
    line_types: &mut Vec<u32>,
) {
    layers.clear();
    line_types.clear();
    let attribs = children
        .iter()
        .map(|(_, o)| o)
        .filter(|o| o.kind == b"ATTRIB");
    for object in core::iter::once(obj).chain(attribs) {
        object_slot_names(object, cs, catalog, layers, line_types);
    }
}

/// 1 つのレコードの画層（8）・線種（6）の名前が指すスロットを足し、綴りの違いに印を付ける
fn object_slot_names(
    object: &Object,
    cs: Charset,
    catalog: &mut SlotCatalog,
    layers: &mut Vec<u32>,
    line_types: &mut Vec<u32>,
) {
    if let Some(Data::Text(raw)) = object.first(8)
        && let Some(name) = cs.decode(raw)
        && let Some(&slot) = catalog.by_layer_lower.get(&name.to_ascii_lowercase())
    {
        if let Some(entry) = catalog.layers.get_mut(slot as usize)
            && entry.name != name
        {
            entry.spelled_otherwise = true;
        }
        layers.push(slot);
    }
    if let Some(Data::Text(raw)) = object.first(6)
        && let Some(name) = cs.decode(raw)
        && let Some(&slot) = catalog.by_line_type_lower.get(&name.to_ascii_lowercase())
    {
        if let Some(entry) = catalog.line_types.get_mut(slot as usize)
            && entry.name != name
        {
            entry.spelled_otherwise = true;
        }
        line_types.push(slot);
    }
}

/// BLOCKS の各レコードが名前で指すスロットを集める（用紙のブロックと、持ち上げに失敗した図形を含む）
fn block_slot_names(
    db: &AcadDb,
    cs: Charset,
    catalog: &mut SlotCatalog,
    cx: &mut Cx<'_>,
) -> Result<[BTreeSet<u32>; 2], Error> {
    let mut named = [BTreeSet::new(), BTreeSet::new()];
    let mut layers = Vec::new();
    let mut line_types = Vec::new();
    for object in db.objects() {
        if object.section != Section::Blocks {
            continue;
        }
        cx.check(Kind::Work, 1).map_err(|stop| {
            cx.report(stop.to_diagnostic());
            Error::Stopped(stop)
        })?;
        layers.clear();
        line_types.clear();
        object_slot_names(object, cs, catalog, &mut layers, &mut line_types);
        let [layer_named, line_type_named] = &mut named;
        layer_named.extend(layers.iter().copied());
        line_type_named.extend(line_types.iter().copied());
    }
    Ok(named)
}

/// モデル空間の行に残らない参照を「あるかもしれない参照」として記録する
fn note_maybe_refs(
    layers: &[u32],
    line_types: &[u32],
    record: u32,
    layer_notes: &mut OpenNotes,
    line_type_notes: &mut OpenNotes,
) {
    for &slot in layers {
        layer_notes.note_maybe(slot, record);
    }
    for &slot in line_types {
        line_type_notes.note_maybe(slot, record);
    }
}

/// `kept` が `named` を多重集合として含むか
fn multiset_covers(kept: &[u32], named: &[u32]) -> bool {
    let mut kept = kept.to_vec();
    kept.sort_unstable();
    named.iter().all(|slot| match kept.binary_search(slot) {
        Ok(at) => {
            kept.remove(at);
            true
        }
        Err(_) => false,
    })
}

/// 開いた行と材料から暗黙の画層・線種の参照表を組む
fn build_implicit_refs(
    catalog: &SlotCatalog,
    shapes: &LiveShapes,
    rows: &[LiveRow],
    [layer_table, line_type_table]: [u32; 2],
    [layer_notes, line_type_notes]: [&OpenNotes; 2],
    [block_layers, block_line_types]: &[BTreeSet<u32>; 2],
) -> ImplicitRefs {
    let implicit_layers = catalog.layers.len().saturating_sub(layer_table as usize);
    let implicit_line_types = catalog
        .line_types
        .len()
        .saturating_sub(line_type_table as usize);
    let mut layer_model: Vec<BTreeMap<(u32, u32), u32>> = vec![BTreeMap::new(); implicit_layers];
    let mut line_type_model: Vec<BTreeMap<(u32, u32), u32>> =
        vec![BTreeMap::new(); implicit_line_types];
    if implicit_layers + implicit_line_types > 0 {
        let mut layers = Vec::new();
        let mut line_types = Vec::new();
        for row in rows {
            shapes.slot_refs(row, &mut layers, &mut line_types);
            for (order, &slot) in layers.iter().enumerate() {
                if let Some(idx) = slot.checked_sub(layer_table)
                    && let Some(map) = layer_model.get_mut(idx as usize)
                {
                    *map.entry((row.record, order as u32)).or_default() += 1;
                }
            }
            for (order, &slot) in line_types.iter().enumerate() {
                if let Some(idx) = slot.checked_sub(line_type_table)
                    && let Some(map) = line_type_model.get_mut(idx as usize)
                {
                    *map.entry((row.record, order as u32)).or_default() += 1;
                }
            }
        }
    }
    ImplicitRefs {
        layers: KindRefs::build(
            layer_table,
            catalog.layers.len(),
            |slot| {
                catalog
                    .layers
                    .get(slot as usize)
                    .is_some_and(|e| e.pinned_outside_model)
            },
            |slot| block_layers.contains(&slot),
            layer_model,
            layer_notes,
            &catalog.active_layer_slots,
        ),
        line_types: KindRefs::build(
            line_type_table,
            catalog.line_types.len(),
            |slot| {
                catalog
                    .line_types
                    .get(slot as usize)
                    .is_some_and(|e| e.pinned_outside_model)
            },
            |slot| block_line_types.contains(&slot),
            line_type_model,
            line_type_notes,
            &catalog.active_line_type_slots,
        ),
    }
}

fn pin_element_slots<P>(el: &Element<P>, catalog: &mut SlotCatalog) {
    if let Some(entry) = catalog.layers.get_mut(el.layer.0 as usize) {
        entry.pinned_outside_model = true;
    }
    if let zumen_drawing::Inherit::Own(lt) = el.style.line_type
        && let Some(entry) = catalog.line_types.get_mut(lt.0 as usize)
    {
        entry.pinned_outside_model = true;
    }
    if let Shape::Symbol(sym) = &el.shape {
        for attr in &sym.attributes {
            if let Some(entry) = catalog.layers.get_mut(attr.layer.0 as usize) {
                entry.pinned_outside_model = true;
            }
            if let zumen_drawing::Inherit::Own(lt) = attr.style.line_type
                && let Some(entry) = catalog.line_types.get_mut(lt.0 as usize)
            {
                entry.pinned_outside_model = true;
            }
        }
    }
    if let Shape::Callout(callout) = &el.shape
        && let zumen_drawing::Inherit::Own(lt) = callout.line_style.line_type
        && let Some(entry) = catalog.line_types.get_mut(lt.0 as usize)
    {
        entry.pinned_outside_model = true;
    }
}

fn collect_forget_handles(kind: &[u8], values: &[Value], out: &mut BTreeSet<u64>) {
    let is_dict = kind == b"DICTIONARY";
    let mut in_group = 0usize;
    for v in values {
        match (v.code, &v.data) {
            (102, Data::Text(t)) if t.starts_with(b"{") => in_group += 1,
            (102, Data::Text(t)) if t.starts_with(b"}") => in_group = in_group.saturating_sub(1),
            (330 | 360, Data::Handle(h)) if in_group > 0 => {
                out.insert(*h);
            }
            (350 | 360, Data::Handle(h)) if is_dict && in_group == 0 => {
                out.insert(*h);
            }
            _ => {}
        }
    }
    let _ = forget_values;
}

fn collect_entity_ref_handles(kind: &[u8], values: &[Value], out: &mut BTreeSet<u64>) {
    match kind {
        b"HATCH" => {
            for v in values {
                if v.code == 330
                    && let Data::Handle(h) = v.data
                    && h != 0
                {
                    out.insert(h);
                }
            }
        }
        b"LEADER" => {
            for v in values {
                if v.code == 340
                    && let Data::Handle(h) = v.data
                    && h != 0
                {
                    out.insert(h);
                }
            }
        }
        _ => {}
    }
}

fn collect_reactor_and_ext_dict(
    row_idx: usize,
    values: &[Value],
    row_reactors: &mut Vec<(usize, u64)>,
    row_ext_dicts: &mut Vec<(usize, u64)>,
) {
    let mut in_group = 0usize;
    let mut in_reactors = false;
    for v in values {
        match (v.code, &v.data) {
            (102, Data::Text(t)) if t.starts_with(b"{") => {
                if in_group == 0 && t.eq_ignore_ascii_case(b"{ACAD_REACTORS") {
                    in_reactors = true;
                }
                in_group += 1;
            }
            (102, Data::Text(t)) if t.starts_with(b"}") => {
                in_group = in_group.saturating_sub(1);
                if in_group == 0 {
                    in_reactors = false;
                }
            }
            (330, Data::Handle(h)) if in_reactors && *h != 0 => {
                row_reactors.push((row_idx, *h));
            }
            (360, Data::Handle(h)) if in_group == 0 && *h != 0 => {
                row_ext_dicts.push((row_idx, *h));
            }
            _ => {}
        }
    }
}

fn collect_shape_symbol_nodes<P>(
    shape: &Shape<P>,
    block_defs_by_lower: &BTreeMap<String, u32>,
    out: &mut Vec<RowId>,
) {
    let mut push_key = |key: &SymbolKey| {
        if key.namespace == BLOCK
            && let Some(&def_idx) = block_defs_by_lower.get(&key.name.to_lowercase())
            && let Some(vnode) = RowId::virtual_node(def_idx)
        {
            out.push(vnode);
        }
    };
    match shape {
        Shape::Symbol(sym) => push_key(&sym.key),
        Shape::Dimension(dim) => {
            if let Some(name) = &dim.block
                && let Some(&def_idx) = block_defs_by_lower.get(&name.to_lowercase())
                && let Some(vnode) = RowId::virtual_node(def_idx)
            {
                out.push(vnode);
            }
        }
        Shape::Callout(callout) => {
            if let Arrowhead::Symbol(key) = &callout.arrowhead {
                push_key(key);
            }
            if let CalloutContent::Symbol { symbol, .. } = &callout.content {
                push_key(&symbol.key);
            }
        }
        Shape::Table(table) => {
            if let Some(drawn) = &table.drawn {
                push_key(drawn);
            }
            for cell in &table.cells {
                if let CellContent::Symbol(key) = &cell.content {
                    push_key(key);
                }
            }
        }
        _ => {}
    }
}

pub(super) fn collect_row_dep_sources(
    shapes: &LiveShapes,
    block_defs_by_lower: &BTreeMap<String, u32>,
    mut find_handle: impl FnMut(u64) -> Option<RowId>,
    row: &LiveRow,
) -> Vec<RowId> {
    let mut srcs = Vec::new();
    match row.kind {
        LiveKind::Insert => {
            if let Some(ins) = shapes.inserts.get(row.shape as usize)
                && ins.definition != NO_DEFINITION
                && let Some(vnode) = RowId::virtual_node(ins.definition)
            {
                srcs.push(vnode);
            }
        }
        LiveKind::General => {
            if let Some(general) = shapes.generals.get(row.shape as usize) {
                collect_shape_symbol_nodes(&general.element.shape, block_defs_by_lower, &mut srcs);
                match &general.element.shape {
                    Shape::Hatch(hatch) => {
                        for lp in &hatch.loops {
                            for s in &lp.sources {
                                if let Some(Key::Handle(h)) = s.key()
                                    && let Some(rid) = find_handle(h)
                                {
                                    srcs.push(rid);
                                }
                            }
                        }
                    }
                    Shape::Leader(leader) => {
                        if let Some(ann) = &leader.annotation
                            && let Some(Key::Handle(h)) = ann.key()
                            && let Some(rid) = find_handle(h)
                        {
                            srcs.push(rid);
                        }
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }
    srcs.sort_unstable();
    srcs.dedup();
    srcs
}

#[allow(clippy::too_many_arguments)]
fn build_dep_graph(
    shell_db: &AcadDb,
    table: &VersionedTable<LiveRow>,
    shapes: &LiveShapes,
    symbols: &[SymbolDefinition],
    block_defs_by_lower: &BTreeMap<String, u32>,
    row_reactors: &[(usize, u64)],
    row_ext_dicts: &[(usize, u64)],
    cx: &mut Cx<'_>,
) -> Result<(DepGraph, BTreeMap<RowId, Vec<RowId>>), Error> {
    let mut dep = DepGraph::new();
    // 1. 入れ子ブロック定義どうしの辺（内側のブロック定義 -> 外側のブロック定義）
    for (outer_idx, sym) in symbols.iter().enumerate() {
        let Some(outer_node) = u32::try_from(outer_idx).ok().and_then(RowId::virtual_node) else {
            continue;
        };
        let mut srcs = Vec::new();
        for el in &sym.space.elements {
            collect_shape_symbol_nodes(&el.shape, block_defs_by_lower, &mut srcs);
        }
        srcs.retain(|&s| s != outer_node);
        srcs.sort_unstable();
        srcs.dedup();
        if !srcs.is_empty() {
            dep.set_sources(outer_node, srcs);
        }
    }

    // 2. `DIMASSOC` および拡張辞書のリアクター参照による連想寸法・注釈の依存辺を集める
    let mut extra_sources: BTreeMap<RowId, Vec<RowId>> = BTreeMap::new();
    for obj in shell_db.objects() {
        if obj.section == Section::Objects && obj.kind == b"DIMASSOC" {
            let dim_rid = obj.values.iter().find_map(|v| match (v.code, &v.data) {
                (330, Data::Handle(h)) if *h != 0 => table.find_handle(*h),
                _ => None,
            });
            if let Some(dim_rid) = dim_rid {
                for v in &obj.values {
                    if matches!(v.code, 331 | 332)
                        && let Data::Handle(src_h) = v.data
                        && src_h != 0
                        && let Some(src_rid) = table.find_handle(src_h)
                        && src_rid != dim_rid
                    {
                        extra_sources.entry(dim_rid).or_default().push(src_rid);
                    }
                }
            }
        }
    }
    if !row_reactors.is_empty() {
        let mut item_to_owner_row: BTreeMap<u64, RowId> = BTreeMap::new();
        for &(r_idx, dict_h) in row_ext_dicts {
            if let Some(owner_rid) = RowId::from_index(r_idx)
                && let Some(dict_obj) = shell_db.get(dict_h)
                && dict_obj.kind == b"DICTIONARY"
            {
                for v in &dict_obj.values {
                    if matches!(v.code, 350 | 360)
                        && let Data::Handle(item_h) = v.data
                        && item_h != 0
                    {
                        item_to_owner_row.entry(item_h).or_insert(owner_rid);
                    }
                }
            }
        }
        for &(src_idx, reactor_h) in row_reactors {
            let Some(src_rid) = RowId::from_index(src_idx) else {
                continue;
            };
            let target_rid = item_to_owner_row
                .get(&reactor_h)
                .copied()
                .or_else(|| table.find_handle(reactor_h));
            if let Some(dep_rid) = target_rid
                && dep_rid != src_rid
                && table
                    .row(dep_rid)
                    .is_some_and(|r| r.kind == LiveKind::General)
            {
                extra_sources.entry(dep_rid).or_default().push(src_rid);
            }
        }
    }

    // 依存グラフとは別に保つ列の分を計上する。履歴の状態間では Arc を共有する。
    let retained_bytes = extra_sources.values().fold(0u64, |total, sources| {
        total
            .saturating_add(64)
            .saturating_add((sources.len() as u64).saturating_mul(size_of::<RowId>() as u64))
    });
    cx.check(Kind::DecodedBytes, retained_bytes)
        .map_err(|stop| {
            cx.report(stop.to_diagnostic());
            Error::Stopped(stop)
        })?;

    // 3. モデル空間の各行について、ブロック定義・ハッチング境界・引出線注釈・連想寸法の依存元を登録する
    for idx in 0..table.len() {
        let Some(rid) = RowId::from_index(idx) else {
            break;
        };
        let Some(row) = table.row(rid) else {
            continue;
        };
        let mut srcs =
            collect_row_dep_sources(shapes, block_defs_by_lower, |h| table.find_handle(h), row);
        if let Some(extra) = extra_sources.get(&rid) {
            srcs.extend_from_slice(extra);
            srcs.retain(|&s| s != rid);
            srcs.sort_unstable();
            srcs.dedup();
        }
        if !srcs.is_empty() {
            dep.set_sources(rid, srcs);
        }
    }
    Ok((dep, extra_sources))
}

fn has_extension_or_paper_values(values: &[Value]) -> bool {
    values.iter().any(|v| {
        v.code >= 1000
            || matches!(v.code, 102 | 360 | 101)
            || (v.code == 39 && !matches!(v.data, Data::Real(r) if r == 0.0))
            || (v.code == 67 && v.data == Data::Int(1))
    })
}

fn attach_scanned_entities(
    entity_indices: &[u32],
    records: &[ScannedRecord],
) -> (BTreeMap<usize, Vec<usize>>, BTreeSet<usize>) {
    let roles: Vec<Option<Followed>> = entity_indices
        .iter()
        .map(|&rec_u32| records.get(rec_u32 as usize).and_then(|r| r.followed))
        .collect();
    attach_roles(&roles)
}

fn attach_roles(roles: &[Option<Followed>]) -> (BTreeMap<usize, Vec<usize>>, BTreeSet<usize>) {
    let by_handle: BTreeMap<u64, usize> = roles
        .iter()
        .enumerate()
        .filter_map(|(idx, role)| Some(((*role)?.handle?, idx)))
        .collect();
    let mut attributes: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    let mut owned = BTreeSet::new();
    let mut open = None;
    for (index, role) in roles.iter().enumerate() {
        match role {
            Some(Followed {
                kind: Follow::Insert { opens } | Follow::Polyline { opens },
                ..
            }) => {
                open = opens.then_some(index);
            }
            Some(Followed { kind, owner, .. }) => {
                let parent = owner
                    .and_then(|o| by_handle.get(&o).copied())
                    .or(open)
                    .filter(|parent| {
                        roles
                            .get(*parent)
                            .and_then(|r| *r)
                            .is_some_and(|parent_role| followed_expected(*kind, parent_role.kind))
                    });
                if let Some(parent) = parent {
                    owned.insert(index);
                    match kind {
                        Follow::Seqend => {
                            if open == Some(parent) {
                                open = None;
                            }
                        }
                        _ => attributes.entry(parent).or_default().push(index),
                    }
                }
            }
            None => open = None,
        }
    }
    (attributes, owned)
}

fn decode_section_name(src: &[u8], range: core::ops::Range<usize>) -> Section {
    let slice = src.get(range).unwrap_or_default();
    let mut pos = 0usize;
    let mut first = true;
    while let Next::Tag(span) = next_tag(slice, pos) {
        pos = span.end as usize;
        if first {
            first = false;
            continue;
        }
        if span.code == 2 {
            let name = slice
                .get(span.value as usize..span.value_end as usize)
                .unwrap_or_default()
                .trim_ascii();
            return match name {
                b"CLASSES" => Section::Classes,
                b"TABLES" => Section::Tables,
                b"BLOCKS" => Section::Blocks,
                b"ENTITIES" => Section::Entities,
                b"OBJECTS" => Section::Objects,
                other => Section::Other(other.to_vec()),
            };
        }
    }
    Section::Other(Vec::new())
}

fn parse_dxf_header_record(
    src: &[u8],
    range: core::ops::Range<usize>,
    is_first_header: bool,
    db: &mut AcadDb,
    header_spans: &mut Box<[Span]>,
    header_known_starts: &mut BTreeMap<Vec<u8>, usize>,
) {
    let base = range.start;
    let Some(bytes) = src.get(range) else {
        return;
    };
    let rs = zumen_live::RecordSlice { base, bytes };
    let spans = read_record_spans_slice(&rs);
    let mut start = 0usize;
    for chunk in spans.chunk_by(|_, b| b.code != 9) {
        if let Some((head, tail)) = chunk.split_first()
            && head.code == 9
        {
            let name = rs.slice(head.value, head.value_end).trim_ascii();
            if is_first_header {
                header_known_starts
                    .entry(name.to_vec())
                    .or_insert(start + 1);
            }
            if db.variable(name).is_none() {
                let vals: Vec<Value> = tail
                    .iter()
                    .map(|s| parse_value(b"HEADER", s.code, rs.slice(s.value, s.value_end)))
                    .collect();
                db.set_variable(name, vals);
            }
        }
        start += chunk.len();
    }
    if is_first_header {
        *header_spans = spans.into_boxed_slice();
    }
}

fn decode_dxf_slice(src: &[u8], range: core::ops::Range<usize>) -> Option<(Vec<u8>, Vec<Value>)> {
    let base = range.start;
    let bytes = src.get(range)?;
    let rs = zumen_live::RecordSlice { base, bytes };
    decode_record_slice(&rs)
}

fn lift_plain_attrib_from_dxf<'a>(
    src: &'a [u8],
    range: core::ops::Range<usize>,
    cs: Charset,
    record: u32,
    tags: &mut Vec<RawTag<'a>>,
) -> Option<PlainAttrib> {
    let bytes = src.get(range)?;
    tags.clear();
    let mut pos = 0usize;
    let Next::Tag(first) = next_tag(bytes, pos) else {
        return None;
    };
    if first.code != 0 {
        return None;
    }
    pos = first.end as usize;
    while let Next::Tag(span) = next_tag(bytes, pos) {
        pos = span.end as usize;
        let val = bytes.get(span.value as usize..span.value_end as usize)?;
        tags.push(RawTag {
            code: span.code,
            value: val,
        });
    }
    let fields = DxfFields::new(tags);
    try_lift_plain_attrib(
        &fields,
        cs,
        record,
        Origin::Dxf {
            record: record as usize,
        },
        fields.handle,
    )
}

#[allow(clippy::too_many_arguments)]
fn lift_plain_entity_from_dxf<'a>(
    src: &'a [u8],
    range: core::ops::Range<usize>,
    kind: &[u8],
    cs: Charset,
    record: u32,
    attribs: Vec<PlainAttrib>,
    raw_attrib_count: u32,
    tags: &mut Vec<RawTag<'a>>,
) -> Option<PlainLifted> {
    let bytes = src.get(range)?;
    tags.clear();
    let mut pos = 0usize;
    let Next::Tag(first) = next_tag(bytes, pos) else {
        return None;
    };
    if first.code != 0 {
        return None;
    }
    pos = first.end as usize;
    while let Next::Tag(span) = next_tag(bytes, pos) {
        pos = span.end as usize;
        let val = bytes.get(span.value as usize..span.value_end as usize)?;
        tags.push(RawTag {
            code: span.code,
            value: val,
        });
    }
    let fields = DxfFields::new(tags);
    let poly_stream = (kind == b"LWPOLYLINE").then_some(tags.as_slice());
    try_lift_plain_entity(
        kind,
        &fields,
        poly_stream,
        cs,
        record,
        Origin::Dxf {
            record: record as usize,
        },
        fields.handle,
        attribs,
        raw_attrib_count,
    )
}

fn fail_exact_dxf_read_budget(
    src: &[u8],
    rec_start: usize,
    rec_end: usize,
    cx: &mut Cx<'_>,
) -> Result<LiveState, Error> {
    let slice = src.get(rec_start..rec_end).unwrap_or_default();
    let mut pos = 0usize;
    let mut first = true;
    while let Next::Tag(span) = next_tag(slice, pos) {
        pos = span.end as usize;
        let place = Ref::at_bytes(
            (rec_start + span.start as usize) as u64,
            (rec_start + span.end as usize) as u64,
        );
        cx.check(Kind::Work, 1).map_err(|stop| {
            cx.report(stop.to_diagnostic().at(place.clone()));
            Error::Stopped(stop)
        })?;
        if first {
            first = false;
            cx.check(Kind::Records, 1).map_err(|stop| {
                cx.report(stop.to_diagnostic().at(place));
                Error::Stopped(stop)
            })?;
        }
    }
    let stop = cx
        .check(Kind::Work, u64::MAX / 2)
        .err()
        .or_else(|| cx.check(Kind::Records, u64::MAX / 2).err());
    match stop {
        Some(s) => {
            cx.report(
                s.to_diagnostic()
                    .at(Ref::at_bytes(rec_start as u64, rec_end as u64)),
            );
            Err(Error::Stopped(s))
        }
        None => Err(Error::Invalid),
    }
}

fn check_symbols_live(
    rows: &[LiveRow],
    shapes: &LiveShapes,
    symbols: &[SymbolDefinition],
    out: &Out,
    cx: &mut Cx<'_>,
) -> Result<(), Error> {
    fn placed_in<P>(elements: &[Element<P>]) -> impl Iterator<Item = (SymbolKey, Ref)> + '_ {
        elements.iter().filter_map(|el| match &el.shape {
            Shape::Symbol(sym) => Some((sym.key.clone(), el.source.clone())),
            _ => None,
        })
    }
    fn arrowheads_of<'a>(
        arrowheads: impl IntoIterator<Item = &'a Arrowhead>,
    ) -> impl Iterator<Item = SymbolKey> {
        arrowheads.into_iter().filter_map(|a| match a {
            Arrowhead::Symbol(key) => Some(key.clone()),
            Arrowhead::Default => None,
        })
    }
    fn referenced_in<P>(elements: &[Element<P>]) -> Vec<(SymbolKey, Ref)> {
        let mut res = Vec::new();
        for el in elements {
            referenced_shape(&el.shape, &el.source, &mut res);
        }
        res
    }
    fn referenced_shape<P>(shape: &Shape<P>, at: &Ref, res: &mut Vec<(SymbolKey, Ref)>) {
        match shape {
            Shape::Table(table) => {
                if let Some(drawn) = &table.drawn {
                    res.push((drawn.clone(), at.clone()));
                }
                for cell in &table.cells {
                    if let CellContent::Symbol(key) = &cell.content {
                        res.push((key.clone(), at.clone()));
                    }
                }
            }
            Shape::Dimension(dim) => {
                if let Some(name) = &dim.block {
                    res.push((
                        SymbolKey {
                            namespace: BLOCK.into(),
                            name: name.clone(),
                        },
                        at.clone(),
                    ));
                }
            }
            Shape::Callout(callout) => {
                for key in arrowheads_of([&callout.arrowhead]) {
                    res.push((key, at.clone()));
                }
                if let CalloutContent::Symbol { symbol, .. } = &callout.content {
                    res.push((symbol.key.clone(), at.clone()));
                }
            }
            _ => {}
        }
    }

    let mut by_name = BTreeMap::new();
    for (idx, sym) in symbols.iter().enumerate() {
        by_name.entry(&sym.key).or_insert(idx);
    }

    let mut checks: Vec<(SymbolKey, Ref)> = Vec::new();
    // 1. placements: model -> paper -> layouts -> symbols
    for row in rows {
        match row.kind {
            LiveKind::Insert => {
                if let Some(ins) = shapes.inserts.get(row.shape as usize) {
                    checks.push((
                        SymbolKey {
                            namespace: BLOCK.into(),
                            name: shapes.pools.get_string(ins.name),
                        },
                        row.source_ref(),
                    ));
                }
            }
            LiveKind::General => {
                if let Some(general) = shapes.generals.get(row.shape as usize)
                    && let Shape::Symbol(sym) = &general.element.shape
                {
                    checks.push((sym.key.clone(), row.source_ref()));
                }
            }
            _ => {}
        }
    }
    checks.extend(placed_in(&out.paper));
    for (_, _, sheet, _) in &out.layouts {
        checks.extend(placed_in(&sheet.elements));
    }
    for sym in symbols {
        checks.extend(placed_in(&sym.space.elements));
    }

    // 2. others: model -> paper -> layouts -> symbols -> dimension_styles
    for row in rows {
        if row.kind == LiveKind::General
            && let Some(general) = shapes.generals.get(row.shape as usize)
        {
            referenced_shape(&general.element.shape, &row.source_ref(), &mut checks);
        }
    }
    checks.extend(referenced_in(&out.paper));
    for (_, _, sheet, _) in &out.layouts {
        checks.extend(referenced_in(&sheet.elements));
    }
    for sym in symbols {
        checks.extend(referenced_in(&sym.space.elements));
    }
    for style in &out.dimension_styles {
        let keys: BTreeSet<SymbolKey> =
            arrowheads_of(style.ends.iter().chain([&style.leader])).collect();
        for key in keys {
            checks.push((key, style.source.clone()));
        }
    }

    for (key, at) in checks {
        work(cx, 1, &at)?;
        if key.namespace == BLOCK && !by_name.contains_key(&key) {
            cx.report(
                Diagnostic::new(codes::DXF_BLOCK_MISSING)
                    .with("name", key.name)
                    .at(at),
            );
        }
    }

    let edges: Vec<Vec<usize>> = symbols
        .iter()
        .map(|sym| {
            placed_in(&sym.space.elements)
                .filter_map(|(key, _)| by_name.get(&key).copied())
                .collect()
        })
        .collect();
    for (index, sym) in symbols.iter().enumerate() {
        let mut seen = vec![false; symbols.len()];
        let mut next = edges.get(index).cloned().unwrap_or_default();
        while let Some(at) = next.pop() {
            work(cx, 1, &sym.source)?;
            if at == index {
                cx.report(
                    Diagnostic::new(codes::DXF_BLOCK_CYCLE_INVALID)
                        .with("name", sym.key.name.clone())
                        .at(sym.source.clone()),
                );
                break;
            }
            if let Some(visited @ false) = seen.get_mut(at) {
                *visited = true;
                next.extend(edges.get(at).into_iter().flatten());
            }
        }
    }
    Ok(())
}
