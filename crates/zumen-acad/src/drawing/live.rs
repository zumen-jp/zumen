//! DXF・DWG の常駐文書エンジン [`AcadLive`]（`docs/live.md`、#878）。
//!
//! 形式非依存の基盤 [`zumen_live`]（ページ配列・コピーオンライト列・格子空間索引・64 要素単位の差分表示バッファー・
//! 原子トランザクションと履歴）の上に、DXF（ASCII・バイナリ）と DWG（R2000〜R2018）の単一レコード復号・差分書き戻し・
//! 全体を書く保存（[`AcadLive::save`]）と後ろに足す保存（[`AcadLive::save_compact`]）を統合する。

mod backing;
mod edit;
mod open;
mod shape;

use zumen_base::Cx;
use zumen_drawing::{Applied, Changes, Drawing2D, Element, ModelPoint};
use zumen_live::display::Display;
use zumen_live::{Revision, RowId, SpatialGrid, VersionedTable};
use zumen_sdk::Error;

pub use backing::LiveFormat;
use edit::{DEFAULT_HISTORY_MAX_BYTES, LiveJournal, apply_changes};
use open::{LiveState, open_auto, open_dwg, open_dxf};
pub use shape::{LiveKind, LiveRow};

/// DXF・DWG を常駐させ、差分の対話編集・履歴・差分表示同期・保存を行う文書エンジン
#[derive(Clone)]
pub struct AcadLive {
    state: LiveState,
    journal: LiveJournal,
    threads: usize,
}

impl AcadLive {
    /// 先頭バイトから DXF（ASCII・バイナリ）と DWG を自動判定して開く
    pub fn open(src: &[u8], cx: &mut Cx<'_>) -> Result<Self, Error> {
        Self::open_with_threads(src, 1, cx)
    }

    /// 並列スレッド数を指定して DXF または DWG を開く（`threads` に依らず結果と診断は決定的に一致する）
    pub fn open_with_threads(src: &[u8], threads: usize, cx: &mut Cx<'_>) -> Result<Self, Error> {
        let threads = threads.max(1);
        let state = open_auto(src, threads, cx)?;
        Ok(Self {
            state,
            journal: LiveJournal::new(DEFAULT_HISTORY_MAX_BYTES),
            threads,
        })
    }

    /// DXF（ASCII またはバイナリ）として開く
    pub fn open_dxf(src: &[u8], cx: &mut Cx<'_>) -> Result<Self, Error> {
        Self::open_dxf_with_threads(src, 1, cx)
    }

    /// 並列スレッド数を指定して DXF を開く
    pub fn open_dxf_with_threads(
        src: &[u8],
        threads: usize,
        cx: &mut Cx<'_>,
    ) -> Result<Self, Error> {
        let threads = threads.max(1);
        let state = open_dxf(src, threads, cx)?;
        Ok(Self {
            state,
            journal: LiveJournal::new(DEFAULT_HISTORY_MAX_BYTES),
            threads,
        })
    }

    /// DWG として開く
    pub fn open_dwg(src: &[u8], cx: &mut Cx<'_>) -> Result<Self, Error> {
        Self::open_dwg_with_threads(src, 1, cx)
    }

    /// 並列スレッド数を指定して DWG を開く
    pub fn open_dwg_with_threads(
        src: &[u8],
        threads: usize,
        cx: &mut Cx<'_>,
    ) -> Result<Self, Error> {
        let threads = threads.max(1);
        let state = open_dwg(src, threads, cx)?;
        Ok(Self {
            state,
            journal: LiveJournal::new(DEFAULT_HISTORY_MAX_BYTES),
            threads,
        })
    }

    /// 取り消し履歴が保持する最大バイト数を設定する
    pub fn with_history_limit(mut self, max_bytes: usize) -> Self {
        self.journal = LiveJournal::new(max_bytes);
        self
    }

    /// 開いている元の形式種別
    #[inline]
    pub const fn format(&self) -> LiveFormat {
        self.state.backing.format()
    }

    /// 現在の版の番号（`apply`・`undo`・`redo` のたびに単調増加する）
    #[inline]
    pub const fn revision(&self) -> Revision {
        self.state.table.revision()
    }

    /// 現在の内容の状態番号（`undo` で元の版に戻ると元の状態番号へ戻る）
    #[inline]
    pub const fn state_id(&self) -> u64 {
        self.state.table.state_id()
    }

    /// モデル空間で生きている要素の数
    #[inline]
    pub const fn live_count(&self) -> u32 {
        self.state.table.live_count()
    }

    /// 削除済みの行を含むモデル空間の全スロット数
    #[inline]
    pub const fn row_count(&self) -> usize {
        self.state.table.len()
    }

    /// モデル空間の版管理テーブルを借りる
    #[inline]
    pub const fn table(&self) -> &VersionedTable<LiveRow> {
        &self.state.table
    }

    /// モデル空間の格子空間索引を借りる
    #[inline]
    pub const fn spatial(&self) -> &SpatialGrid {
        &self.state.spatial
    }

    /// GPU 転送用の差分表示バッファーを借りる
    #[inline]
    pub const fn display(&self) -> &Display {
        &self.state.display
    }

    /// GPU 転送用の差分表示バッファーを可変で借りる（[`Display::sync_gpu_mirror`] や [`Display::clear_dirty`] 用）
    #[inline]
    pub fn display_mut(&mut self) -> &mut Display {
        &mut self.state.display
    }

    /// モデル空間の矩形 `query`（`[min_x, min_y, max_x, max_y]`）と交差し、かつ生きている行を昇順で `out` に集める
    pub fn select_rect(&self, query: [f64; 4], out: &mut Vec<RowId>) {
        let mut raw = Vec::new();
        self.state
            .spatial
            .query(self.state.table.bounds_column(), query, &mut raw);
        out.extend(
            raw.into_iter()
                .map(RowId)
                .filter(|&id| self.state.table.is_alive(id)),
        );
    }

    /// 生きている行 `row` から図面モデルの [`Element<ModelPoint>`] を 1 件だけ復元する
    pub fn element_of_row(&self, row: RowId) -> Option<Element<ModelPoint>> {
        let r = self.state.table.row(row)?;
        self.state.shapes.to_element(r, &self.state.catalog)
    }

    /// 現在の常駐状態から [`Drawing2D`] を組み立てる
    pub fn drawing(&self, cx: &mut Cx<'_>) -> Result<Drawing2D, Error> {
        self.state.to_drawing(cx)
    }

    /// 変更 `changes` を当てて履歴に積み、各項目の適用後 ID・ハッシュを返す
    pub fn apply(&mut self, changes: &Changes, cx: &mut Cx<'_>) -> Result<Vec<Applied>, Error> {
        apply_changes(
            &mut self.state,
            &mut self.journal,
            changes,
            self.threads,
            cx,
        )
    }

    /// 直前の変更を取り消す。取り消したら `Ok(true)`、履歴が空なら `Ok(false)` を返す
    pub fn undo(&mut self, cx: &mut Cx<'_>) -> Result<bool, Error> {
        self.journal.undo(&mut self.state, cx)
    }

    /// 直前に取り消した変更をやり直す。やり直したら `Ok(true)`、履歴が空なら `Ok(false)` を返す
    pub fn redo(&mut self, cx: &mut Cx<'_>) -> Result<bool, Error> {
        self.journal.redo(&mut self.state, cx)
    }

    /// 取り消せる履歴があるか
    #[inline]
    pub fn can_undo(&self) -> bool {
        self.journal.can_undo()
    }

    /// やり直せる履歴があるか
    #[inline]
    pub fn can_redo(&self) -> bool {
        self.journal.can_redo()
    }

    /// 取り消せる段数
    #[inline]
    pub fn undo_len(&self) -> usize {
        self.journal.undo_len()
    }

    /// やり直せる段数
    #[inline]
    pub fn redo_len(&self) -> usize {
        self.journal.redo_len()
    }

    /// 履歴が保持している概算バイト数
    #[inline]
    pub fn history_bytes(&self) -> usize {
        self.journal.bytes()
    }

    /// 履歴を消去する
    pub fn clear_history(&mut self) {
        self.journal.clear();
    }

    /// 全体を書く保存（既存の [`crate::DxfFile::apply`] / [`crate::DwgFile::apply`] を順に当てた結果と 1 バイト単位で一致）
    pub fn save(&self, cx: &mut Cx<'_>) -> Result<Vec<u8>, Error> {
        self.state.backing.save(cx)
    }

    /// 後ろに足す保存（DWG では複数段階の編集を 1 回のトランザクションに畳んで書き出し、読み直した図面モデル・ID・ハッシュ・診断が一致する）
    pub fn save_compact(&self, cx: &mut Cx<'_>) -> Result<Vec<u8>, Error> {
        self.state.backing.save_compact(cx)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use zumen_base::{Budget, CancelToken, Cx, Diagnostic, Diagnostics, Kind, Limit, Limits, Ref};
    use zumen_drawing::{
        Affine, Attribute, Changes, Color, ColorCode, ColorSystem, Edge, Edit, Element, Fit, Hash,
        Height, HeightMeaning, Horizontal, Inherit, Layer, LayerId, LayerState, LayerStyle, Length,
        LineTypeId, LineWeight, ModelLength, ModelPoint, PaperLength, Placed, Placement, PointMark,
        PointSize, PointStyle, Polyline, Restyle, Seen, Shape, SpaceId, Style, Symbol, SymbolKey,
        Text, Vertex, Vertical, id,
    };
    use zumen_live::display::Display;
    use zumen_sdk::{Apply, Codec, Error, Lift};

    use super::{AcadLive, LiveFormat};
    use crate::{Dwg, Dxf};

    const MODEL: SpaceId = SpaceId(0);

    fn fresh_cx<R>(f: impl FnOnce(&mut Cx<'_>) -> R) -> (R, Vec<Diagnostic>) {
        let mut budget = Budget::default();
        let mut diag = Diagnostics::new();
        let cancel = CancelToken::new();
        let out = f(&mut Cx::new(&mut budget, &mut diag, &cancel));
        (out, diag.into_vec())
    }

    fn default_style() -> Style {
        Style {
            color: Inherit::Layer,
            line_type: Inherit::Layer,
            line_type_scale: 1.0,
            line_weight: Some(Inherit::Layer),
            transparency: Inherit::Layer,
        }
    }

    fn pt(x: f64, y: f64) -> ModelPoint {
        ModelPoint { space: MODEL, x, y }
    }

    fn mlen(value: f64) -> Length {
        Length::Model(ModelLength {
            space: MODEL,
            value,
        })
    }

    fn strip_ref_pos(r: &Ref, strip_record: bool) -> Ref {
        let mut out = Ref::default();
        if let Some(id) = r.id() {
            out = out.with_id(id);
        }
        if !strip_record && let Some(rec) = r.record() {
            out = out.with_record(rec);
        }
        if let Some(key) = r.key() {
            out = out.with_key(key);
        }
        out
    }

    fn strip_ref_bytes(r: &Ref) -> Ref {
        strip_ref_pos(r, false)
    }

    fn strip_element_pos<P>(el: &mut Element<P>, strip_record: bool) {
        el.source = strip_ref_pos(&el.source, strip_record);
        match &mut el.shape {
            Shape::Symbol(sym) => {
                for attr in &mut sym.attributes {
                    attr.source = strip_ref_pos(&attr.source, strip_record);
                }
            }
            Shape::Hatch(hatch) => {
                for lp in &mut hatch.loops {
                    for s in &mut lp.sources {
                        *s = strip_ref_pos(s, strip_record);
                    }
                }
            }
            Shape::Callout(callout) => {
                if let zumen_drawing::CalloutContent::Symbol { values, .. } = &mut callout.content {
                    for av in values {
                        av.definition = strip_ref_pos(&av.definition, strip_record);
                    }
                }
            }
            _ => {}
        }
    }

    fn strip_drawing_pos(
        d: &zumen_drawing::Drawing2D,
        strip_record: bool,
    ) -> zumen_drawing::Drawing2D {
        let mut out = d.clone();
        for sp in &mut out.spaces {
            for el in &mut sp.elements {
                strip_element_pos(el, strip_record);
            }
        }
        for sh in &mut out.sheets {
            sh.source = strip_ref_pos(&sh.source, strip_record);
            for el in &mut sh.elements {
                strip_element_pos(el, strip_record);
            }
            for v in &mut sh.views {
                v.source = strip_ref_pos(&v.source, strip_record);
            }
        }
        for layer in &mut out.layers {
            layer.source = strip_ref_pos(&layer.source, strip_record);
        }
        for lt in &mut out.line_types {
            lt.source = strip_ref_pos(&lt.source, strip_record);
        }
        for ds in &mut out.dimension_styles {
            ds.source = strip_ref_pos(&ds.source, strip_record);
        }
        for ts in &mut out.text_styles {
            ts.source = strip_ref_pos(&ts.source, strip_record);
        }
        for sym in &mut out.symbols {
            sym.source = strip_ref_pos(&sym.source, strip_record);
            for el in &mut sym.space.elements {
                strip_element_pos(el, strip_record);
            }
            for ad in &mut sym.attributes {
                ad.attribute.source = strip_ref_pos(&ad.attribute.source, strip_record);
            }
        }
        for ext in &mut out.extensions {
            ext.source = strip_ref_pos(&ext.source, strip_record);
        }
        for tb in &mut out.title_blocks {
            tb.source = strip_ref_pos(&tb.source, strip_record);
        }
        out
    }

    fn drawing_without_byte_ranges(d: &zumen_drawing::Drawing2D) -> zumen_drawing::Drawing2D {
        strip_drawing_pos(d, false)
    }

    fn drawing_without_positions(d: &zumen_drawing::Drawing2D) -> zumen_drawing::Drawing2D {
        strip_drawing_pos(d, true)
    }

    type StrippedDiag = (
        zumen_base::Code,
        u64,
        Option<zumen_base::SpecRef>,
        Vec<(&'static str, zumen_base::Value)>,
        Vec<Ref>,
    );

    fn diag_without_byte_ranges(diags: &[Diagnostic]) -> Vec<StrippedDiag> {
        diags
            .iter()
            .map(|d| {
                (
                    d.code(),
                    d.count(),
                    d.spec(),
                    d.params().to_vec(),
                    d.places().iter().map(strip_ref_bytes).collect(),
                )
            })
            .collect()
    }

    fn all_element_ids_and_hashes(drawing: &zumen_drawing::Drawing2D) -> Vec<(String, Hash)> {
        let mut out = Vec::new();
        for sp in &drawing.spaces {
            let place = id::space(sp.id);
            let mut namer = id::Namer::new(&place);
            for (idx, el) in sp.elements.iter().enumerate() {
                out.push((namer.next(idx, el), Hash::element(el)));
            }
        }
        let mut def_namer = id::DefinitionNamer::default();
        for sym in &drawing.symbols {
            let place = def_namer.next(sym);
            let mut namer = id::Namer::new(&place);
            for (idx, el) in sym.space.elements.iter().enumerate() {
                out.push((namer.next(idx, el), Hash::element(el)));
            }
        }
        let mut sheet_namer = id::SheetNamer::default();
        for sh in &drawing.sheets {
            let place = sheet_namer.next(sh);
            let mut namer = id::Namer::new(&place);
            for (idx, el) in sh.elements.iter().enumerate() {
                out.push((namer.next(idx, el), Hash::element(el)));
            }
        }
        out
    }

    fn samples_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../samples")
    }

    fn synthetic_rich_dxf() -> Vec<u8> {
        // 平面 6 種類（属性つき INSERT を含む）と一般図形（ELLIPSE・POINT）を含む R2000 DXF
        let mut dxf = String::new();
        let mut tag = |code: i16, val: &str| {
            dxf.push_str(&format!("{code:>3}\n{val}\n"));
        };
        tag(0, "SECTION");
        tag(2, "HEADER");
        tag(9, "$ACADVER");
        tag(1, "AC1015");
        tag(9, "$DWGCODEPAGE");
        tag(3, "ANSI_932");
        tag(9, "$INSUNITS");
        tag(70, "4");
        tag(9, "$HANDSEED");
        tag(5, "200");
        tag(9, "$EXTMIN");
        tag(10, "0.0");
        tag(20, "0.0");
        tag(30, "0.0");
        tag(9, "$EXTMAX");
        tag(10, "100.0");
        tag(20, "100.0");
        tag(30, "0.0");
        tag(9, "$PDMODE");
        tag(70, "0");
        tag(9, "$PDSIZE");
        tag(40, "0.0");
        tag(0, "ENDSEC");

        tag(0, "SECTION");
        tag(2, "TABLES");
        tag(0, "TABLE");
        tag(2, "LTYPE");
        tag(5, "10");
        tag(100, "AcDbSymbolTable");
        tag(70, "2");
        tag(0, "LTYPE");
        tag(5, "11");
        tag(100, "AcDbSymbolTableRecord");
        tag(100, "AcDbLinetypeTableRecord");
        tag(2, "Continuous");
        tag(70, "0");
        tag(3, "Solid line");
        tag(72, "65");
        tag(73, "0");
        tag(40, "0.0");
        tag(0, "LTYPE");
        tag(5, "12");
        tag(100, "AcDbSymbolTableRecord");
        tag(100, "AcDbLinetypeTableRecord");
        tag(2, "DASHED");
        tag(70, "0");
        tag(3, "Dashed");
        tag(72, "65");
        tag(73, "2");
        tag(40, "15.0");
        tag(49, "10.0");
        tag(74, "0");
        tag(49, "-5.0");
        tag(74, "0");
        tag(0, "ENDTAB");

        tag(0, "TABLE");
        tag(2, "LAYER");
        tag(5, "20");
        tag(100, "AcDbSymbolTable");
        tag(70, "2");
        tag(0, "LAYER");
        tag(5, "21");
        tag(100, "AcDbSymbolTableRecord");
        tag(100, "AcDbLayerTableRecord");
        tag(2, "0");
        tag(70, "0");
        tag(62, "7");
        tag(6, "Continuous");
        tag(0, "LAYER");
        tag(5, "22");
        tag(100, "AcDbSymbolTableRecord");
        tag(100, "AcDbLayerTableRecord");
        tag(2, "WALL");
        tag(70, "0");
        tag(62, "1");
        tag(6, "Continuous");
        tag(0, "ENDTAB");

        tag(0, "TABLE");
        tag(2, "STYLE");
        tag(5, "30");
        tag(100, "AcDbSymbolTable");
        tag(70, "1");
        tag(0, "STYLE");
        tag(5, "31");
        tag(100, "AcDbSymbolTableRecord");
        tag(100, "AcDbTextStyleTableRecord");
        tag(2, "Standard");
        tag(70, "0");
        tag(40, "0.0");
        tag(41, "1.0");
        tag(50, "0.0");
        tag(71, "0");
        tag(42, "2.5");
        tag(3, "txt");
        tag(4, "");
        tag(0, "ENDTAB");

        tag(0, "TABLE");
        tag(2, "BLOCK_RECORD");
        tag(5, "40");
        tag(100, "AcDbSymbolTable");
        tag(70, "3");
        tag(0, "BLOCK_RECORD");
        tag(5, "41");
        tag(100, "AcDbSymbolTableRecord");
        tag(100, "AcDbBlockTableRecord");
        tag(2, "*Model_Space");
        tag(0, "BLOCK_RECORD");
        tag(5, "42");
        tag(100, "AcDbSymbolTableRecord");
        tag(100, "AcDbBlockTableRecord");
        tag(2, "*Paper_Space");
        tag(0, "BLOCK_RECORD");
        tag(5, "43");
        tag(100, "AcDbSymbolTableRecord");
        tag(100, "AcDbBlockTableRecord");
        tag(2, "MARK");
        tag(0, "ENDTAB");
        tag(0, "ENDSEC");

        tag(0, "SECTION");
        tag(2, "BLOCKS");
        tag(0, "BLOCK");
        tag(5, "50");
        tag(330, "41");
        tag(100, "AcDbEntity");
        tag(8, "0");
        tag(100, "AcDbBlockBegin");
        tag(2, "*Model_Space");
        tag(70, "0");
        tag(10, "0.0");
        tag(20, "0.0");
        tag(30, "0.0");
        tag(3, "*Model_Space");
        tag(1, "");
        tag(0, "ENDBLK");
        tag(5, "51");
        tag(330, "41");
        tag(100, "AcDbEntity");
        tag(8, "0");
        tag(100, "AcDbBlockEnd");

        tag(0, "BLOCK");
        tag(5, "52");
        tag(330, "42");
        tag(100, "AcDbEntity");
        tag(67, "1");
        tag(8, "0");
        tag(100, "AcDbBlockBegin");
        tag(2, "*Paper_Space");
        tag(70, "0");
        tag(10, "0.0");
        tag(20, "0.0");
        tag(30, "0.0");
        tag(3, "*Paper_Space");
        tag(1, "");
        tag(0, "ENDBLK");
        tag(5, "53");
        tag(330, "42");
        tag(100, "AcDbEntity");
        tag(67, "1");
        tag(8, "0");
        tag(100, "AcDbBlockEnd");

        tag(0, "BLOCK");
        tag(5, "54");
        tag(330, "43");
        tag(100, "AcDbEntity");
        tag(8, "0");
        tag(100, "AcDbBlockBegin");
        tag(2, "MARK");
        tag(70, "2");
        tag(10, "0.0");
        tag(20, "0.0");
        tag(30, "0.0");
        tag(3, "MARK");
        tag(1, "");
        tag(0, "CIRCLE");
        tag(5, "55");
        tag(330, "43");
        tag(100, "AcDbEntity");
        tag(8, "0");
        tag(100, "AcDbCircle");
        tag(10, "0.0");
        tag(20, "0.0");
        tag(30, "0.0");
        tag(40, "5.0");
        tag(0, "ATTDEF");
        tag(5, "56");
        tag(330, "43");
        tag(100, "AcDbEntity");
        tag(8, "0");
        tag(100, "AcDbText");
        tag(10, "1.0");
        tag(20, "1.0");
        tag(30, "0.0");
        tag(40, "2.5");
        tag(1, "DEF");
        tag(100, "AcDbAttributeDefinition");
        tag(3, "Prompt");
        tag(2, "CODE");
        tag(70, "0");
        tag(0, "ENDBLK");
        tag(5, "57");
        tag(330, "43");
        tag(100, "AcDbEntity");
        tag(8, "0");
        tag(100, "AcDbBlockEnd");
        tag(0, "ENDSEC");

        tag(0, "SECTION");
        tag(2, "ENTITIES");
        // 1. LINE (h101)
        tag(0, "LINE");
        tag(5, "101");
        tag(330, "41");
        tag(100, "AcDbEntity");
        tag(8, "0");
        tag(100, "AcDbLine");
        tag(10, "10.0");
        tag(20, "20.0");
        tag(30, "0.0");
        tag(11, "50.0");
        tag(21, "60.0");
        tag(31, "0.0");
        // 2. CIRCLE (h102)
        tag(0, "CIRCLE");
        tag(5, "102");
        tag(330, "41");
        tag(100, "AcDbEntity");
        tag(8, "WALL");
        tag(100, "AcDbCircle");
        tag(10, "30.0");
        tag(20, "40.0");
        tag(30, "0.0");
        tag(40, "12.5");
        // 3. ARC (h103)
        tag(0, "ARC");
        tag(5, "103");
        tag(330, "41");
        tag(100, "AcDbEntity");
        tag(8, "0");
        tag(100, "AcDbCircle");
        tag(10, "80.0");
        tag(20, "20.0");
        tag(30, "0.0");
        tag(40, "15.0");
        tag(100, "AcDbArc");
        tag(50, "30.0");
        tag(51, "150.0");
        // 4. LWPOLYLINE (h104)
        tag(0, "LWPOLYLINE");
        tag(5, "104");
        tag(330, "41");
        tag(100, "AcDbEntity");
        tag(8, "0");
        tag(100, "AcDbPolyline");
        tag(90, "3");
        tag(70, "1");
        tag(10, "0.0");
        tag(20, "0.0");
        tag(10, "20.0");
        tag(20, "0.0");
        tag(42, "0.5");
        tag(10, "10.0");
        tag(20, "15.0");
        // 5. TEXT (h105)
        tag(0, "TEXT");
        tag(5, "105");
        tag(330, "41");
        tag(100, "AcDbEntity");
        tag(8, "0");
        tag(100, "AcDbText");
        tag(10, "5.0");
        tag(20, "80.0");
        tag(30, "0.0");
        tag(40, "3.5");
        tag(1, "HELLO");
        tag(7, "Standard");
        tag(100, "AcDbText");
        // 6. INSERT + ATTRIB + SEQEND (h106, h107, h108)
        tag(0, "INSERT");
        tag(5, "106");
        tag(330, "41");
        tag(100, "AcDbEntity");
        tag(8, "0");
        tag(100, "AcDbBlockReference");
        tag(66, "1");
        tag(2, "MARK");
        tag(10, "60.0");
        tag(20, "60.0");
        tag(30, "0.0");
        tag(0, "ATTRIB");
        tag(5, "107");
        tag(330, "106");
        tag(100, "AcDbEntity");
        tag(8, "0");
        tag(100, "AcDbText");
        tag(10, "61.0");
        tag(20, "61.0");
        tag(30, "0.0");
        tag(40, "2.5");
        tag(1, "A-01");
        tag(100, "AcDbAttribute");
        tag(2, "CODE");
        tag(70, "0");
        tag(0, "SEQEND");
        tag(5, "108");
        tag(330, "106");
        tag(100, "AcDbEntity");
        tag(8, "0");
        // 7. ELLIPSE (h109, general path)
        tag(0, "ELLIPSE");
        tag(5, "109");
        tag(330, "41");
        tag(100, "AcDbEntity");
        tag(8, "0");
        tag(100, "AcDbEllipse");
        tag(10, "40.0");
        tag(20, "90.0");
        tag(30, "0.0");
        tag(11, "10.0");
        tag(21, "0.0");
        tag(31, "0.0");
        tag(40, "0.5");
        tag(41, "0.0");
        tag(42, "6.283185307179586");
        tag(0, "ENDSEC");
        tag(0, "EOF");
        dxf.into_bytes()
    }

    #[test]
    fn open_parity_across_samples_and_parallel_threads() {
        let root = samples_dir();
        let mut checked = 0usize;
        for sub in ["dxf", "dwg"] {
            let dir = root.join(sub);
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            let mut paths: Vec<PathBuf> = entries
                .filter_map(|e| e.ok().map(|x| x.path()))
                .filter(|p| {
                    p.extension()
                        .and_then(|s| s.to_str())
                        .is_some_and(|ext| ext.eq_ignore_ascii_case(sub))
                })
                .collect();
            paths.sort();
            for path in paths {
                let Ok(bytes) = std::fs::read(&path) else {
                    continue;
                };
                let (ref_res, ref_diag) = fresh_cx(|cx| {
                    if sub == "dxf" {
                        let f = Dxf::read(&bytes, cx)?;
                        f.lift(cx)
                    } else {
                        let f = Dwg::read(&bytes, cx)?;
                        f.lift(cx)
                    }
                });
                let (live_1, live_diag_1) = fresh_cx(|cx| {
                    let live = AcadLive::open_with_threads(&bytes, 1, cx)?;
                    live.drawing(cx).map(|d| (live, d))
                });
                let (live_4, live_diag_4) = fresh_cx(|cx| {
                    let live = AcadLive::open_with_threads(&bytes, 4, cx)?;
                    live.drawing(cx).map(|d| (live, d))
                });
                assert_eq!(
                    ref_res.is_ok(),
                    live_1.is_ok(),
                    "open status mismatch on {}",
                    path.display()
                );
                assert_eq!(
                    live_diag_1,
                    live_diag_4,
                    "1 vs 4 thread diag mismatch on {}",
                    path.display()
                );
                if let (Ok(expected), Ok((l1, d1)), Ok((l4, d4))) = (ref_res, live_1, live_4) {
                    assert_eq!(d1, expected, "Drawing2D mismatch on {}", path.display());
                    assert_eq!(
                        d4,
                        expected,
                        "4-thread Drawing2D mismatch on {}",
                        path.display()
                    );
                    assert_eq!(
                        all_element_ids_and_hashes(&d1),
                        all_element_ids_and_hashes(&expected),
                        "ID/Hash mismatch on {}",
                        path.display()
                    );
                    assert_eq!(
                        live_diag_1,
                        ref_diag,
                        "diagnostic mismatch on {}",
                        path.display()
                    );
                    let (b1, _) = fresh_cx(|cx| l1.save(cx));
                    let (b4, _) = fresh_cx(|cx| l4.save(cx));
                    assert_eq!(b1, b4, "1 vs 4 thread save mismatch on {}", path.display());
                    checked += 1;
                }
            }
        }
        assert!(checked > 0, "expected at least one sample file");
    }

    #[test]
    fn dxf_fast_and_structural_edits_match_apply_bytes_and_undo_redo() {
        let src = synthetic_rich_dxf();
        let (dxf_ref, _) = fresh_cx(|cx| Dxf::read(&src, cx));
        let Ok(mut dxf_ref) = dxf_ref else {
            return;
        };
        let (live, _) = fresh_cx(|cx| AcadLive::open_dxf(&src, cx));
        let Ok(mut live) = live else {
            return;
        };
        assert_eq!(live.format(), LiveFormat::AsciiDxf);
        assert!(!live.display().has_dirty());

        let mut gpu_mirror = Display::default();
        live.display_mut().mark_all_dirty();
        live.display_mut().sync_gpu_mirror(&mut gpu_mirror);
        assert!(!live.display().has_dirty());

        let fallback_line = Element {
            source: Ref::default(),
            layer: LayerId(0),
            style: default_style(),
            shape: Shape::Line {
                start: pt(0.0, 0.0),
                end: pt(1.0, 0.0),
            },
        };
        let get_seen = |live: &AcadLive, target_id: &str| -> Seen {
            let (d, _) = fresh_cx(|cx| live.drawing(cx));
            let d = d.ok().unwrap_or_default();
            let pair = all_element_ids_and_hashes(&d)
                .into_iter()
                .find(|(id, _)| id == target_id);
            Seen {
                id: target_id.to_owned(),
                hash: pair.map_or_else(|| Hash::element(&fallback_line), |(_, h)| h),
            }
        };

        let mut snapshots: Vec<(Vec<u8>, u64)> = vec![(src.clone(), live.state_id())];

        // Step 1: Transform LINE (h101) and INSERT+ATTRIB (h106)
        let seen_line = get_seen(&live, "space/0/h101");
        let seen_ins = get_seen(&live, "space/0/h106");
        let step1 = Changes {
            document: None,
            edits: vec![
                Edit::Transform {
                    target: seen_line,
                    transform: Affine::translation(15.0, -5.0),
                },
                Edit::Transform {
                    target: seen_ins,
                    transform: Affine::translation(20.0, 10.0),
                },
            ],
        };
        let (ap_live_1, d_live_1) = fresh_cx(|cx| live.apply(&step1, cx));
        let (ap_ref_1, d_ref_1) = fresh_cx(|cx| dxf_ref.apply(&step1, cx));
        assert_eq!(ap_live_1, ap_ref_1);
        assert_eq!(d_live_1, d_ref_1);
        assert!(live.display().has_dirty());
        live.display_mut().sync_gpu_mirror(&mut gpu_mirror);

        // Step 2: Restyle CIRCLE (h102) + SetPointStyle
        let seen_circle = get_seen(&live, "space/0/h102");
        let step2 = Changes {
            document: None,
            edits: vec![
                Edit::Restyle {
                    target: seen_circle,
                    style: Restyle {
                        layer: Some(LayerId(0)),
                        color: Some(Inherit::Own(Color {
                            code: Some(ColorCode {
                                system: ColorSystem::Aci,
                                value: 3,
                            }),
                            rgb: None,
                        })),
                        line_type: Some(Inherit::Own(zumen_drawing::LineTypeId(1))),
                        line_type_scale: Some(2.0),
                        line_weight: Some(Inherit::Own(LineWeight::Width(PaperLength(0.25)))),
                    },
                },
                Edit::SetPointStyle {
                    style: PointStyle {
                        mark: PointMark::Cross,
                        circle: true,
                        square: false,
                        size: PointSize::Length(2.5),
                    },
                },
            ],
        };
        // Step 1 byte check before Step 2
        {
            let mut ref_bytes_1 = Vec::new();
            let _ = fresh_cx(|cx| Dxf::write(&dxf_ref, &mut ref_bytes_1, cx));
            let (live_bytes_1, _) = fresh_cx(|cx| live.save(cx));
            assert_eq!(live_bytes_1.ok().unwrap_or_default(), ref_bytes_1);
            snapshots.push((ref_bytes_1, live.state_id()));
        }

        let (ap_live_2, _) = fresh_cx(|cx| live.apply(&step2, cx));
        let (ap_ref_2, _) = fresh_cx(|cx| dxf_ref.apply(&step2, cx));
        assert_eq!(ap_live_2, ap_ref_2);
        live.display_mut().sync_gpu_mirror(&mut gpu_mirror);
        {
            let mut ref_bytes_2 = Vec::new();
            let _ = fresh_cx(|cx| Dxf::write(&dxf_ref, &mut ref_bytes_2, cx));
            let (live_bytes_2, _) = fresh_cx(|cx| live.save(cx));
            assert_eq!(live_bytes_2.ok().unwrap_or_default(), ref_bytes_2);
            snapshots.push((ref_bytes_2, live.state_id()));
        }

        // Step 3: Replace INSERT attribute text (h106) and ELLIPSE (h109)
        let (cur_d, _) = fresh_cx(|cx| live.drawing(cx));
        let cur_d = cur_d.ok().unwrap_or_default();
        let Some(sp0) = cur_d.spaces.first() else {
            return;
        };
        let Some(mut ins_el) = sp0.elements.get(5).cloned() else {
            return;
        };
        if let Shape::Symbol(sym) = &mut ins_el.shape
            && let Some(attr) = sym.attributes.first_mut()
        {
            attr.text.content = "B-99".to_owned();
        }
        let Some(mut ell_el) = sp0.elements.get(6).cloned() else {
            return;
        };
        if let Shape::Curve(Edge::Elliptic { ratio, .. }) = &mut ell_el.shape {
            *ratio = 0.75;
        }
        let step3 = Changes {
            document: None,
            edits: vec![
                Edit::Replace {
                    target: get_seen(&live, "space/0/h106"),
                    element: Box::new(Placed::Model(ins_el)),
                },
                Edit::Replace {
                    target: get_seen(&live, "space/0/h109"),
                    element: Box::new(Placed::Model(ell_el)),
                },
            ],
        };
        let (ap_live_3, _) = fresh_cx(|cx| live.apply(&step3, cx));
        let (ap_ref_3, _) = fresh_cx(|cx| dxf_ref.apply(&step3, cx));
        assert_eq!(ap_live_3, ap_ref_3);
        {
            let mut ref_bytes_3 = Vec::new();
            let _ = fresh_cx(|cx| Dxf::write(&dxf_ref, &mut ref_bytes_3, cx));
            let (live_bytes_3, _) = fresh_cx(|cx| live.save(cx));
            assert_eq!(live_bytes_3.ok().unwrap_or_default(), ref_bytes_3);
            snapshots.push((ref_bytes_3, live.state_id()));
        }

        // Step 4: Add new LINE + Remove ARC (h103)
        let new_line = Element {
            source: Ref::default(),
            layer: LayerId(1),
            style: default_style(),
            shape: Shape::Line {
                start: pt(-20.0, -10.0),
                end: pt(250.0, 180.0),
            },
        };
        let step4 = Changes {
            document: None,
            edits: vec![
                Edit::Add {
                    place: "space/0".to_owned(),
                    element: Box::new(Placed::Model(new_line)),
                },
                Edit::Remove {
                    target: get_seen(&live, "space/0/h103"),
                },
            ],
        };
        let (ap_live_4, _) = fresh_cx(|cx| live.apply(&step4, cx));
        let (ap_ref_4, _) = fresh_cx(|cx| dxf_ref.apply(&step4, cx));
        assert_eq!(ap_live_4, ap_ref_4);
        {
            let mut ref_bytes_4 = Vec::new();
            let _ = fresh_cx(|cx| Dxf::write(&dxf_ref, &mut ref_bytes_4, cx));
            let (live_bytes_4, _) = fresh_cx(|cx| live.save(cx));
            assert_eq!(live_bytes_4.ok().unwrap_or_default(), ref_bytes_4);
            let (live_d4, _) = fresh_cx(|cx| live.drawing(cx));
            let (ref_d4, _) = fresh_cx(|cx| dxf_ref.lift(cx));
            assert_eq!(
                live_d4.as_ref().map(drawing_without_positions),
                ref_d4.as_ref().map(drawing_without_positions)
            );
            snapshots.push((ref_bytes_4, live.state_id()));
        }

        // Step 5: Structural AddLayer ("PIPE") -> Step 6: Fast Restyle TEXT (h105) onto "PIPE"
        let step5 = Changes {
            document: None,
            edits: vec![Edit::AddLayer {
                layer: Box::new(Layer {
                    id: LayerId(99),
                    name: "PIPE".to_owned(),
                    follows_placement: false,
                    state: Some(LayerState {
                        visible: true,
                        frozen: false,
                        printable: true,
                        editable: true,
                    }),
                    style: Some(LayerStyle {
                        color: Some(Color {
                            code: Some(ColorCode {
                                system: ColorSystem::Aci,
                                value: 5,
                            }),
                            rgb: None,
                        }),
                        line_type: Some(LineTypeId(0)),
                        line_weight: None,
                        transparency: None,
                    }),
                    description: None,
                    source: Ref::default(),
                }),
            }],
        };
        let (ap_live_5, _) = fresh_cx(|cx| live.apply(&step5, cx));
        let (ap_ref_5, _) = fresh_cx(|cx| dxf_ref.apply(&step5, cx));
        assert!(ap_live_5.is_ok());
        assert_eq!(ap_live_5, ap_ref_5);
        {
            let mut ref_bytes_5 = Vec::new();
            let _ = fresh_cx(|cx| Dxf::write(&dxf_ref, &mut ref_bytes_5, cx));
            let (live_bytes_5, _) = fresh_cx(|cx| live.save(cx));
            assert_eq!(live_bytes_5.ok().unwrap_or_default(), ref_bytes_5);
            snapshots.push((ref_bytes_5, live.state_id()));
        }

        let (d_after_5, _) = fresh_cx(|cx| live.drawing(cx));
        let pipe_layer_id = d_after_5
            .ok()
            .and_then(|d| d.layers.iter().find(|l| l.name == "PIPE").map(|l| l.id))
            .unwrap_or(LayerId(2));
        let step6 = Changes {
            document: None,
            edits: vec![Edit::Restyle {
                target: get_seen(&live, "space/0/h105"),
                style: Restyle {
                    layer: Some(pipe_layer_id),
                    color: None,
                    line_type: None,
                    line_type_scale: None,
                    line_weight: None,
                },
            }],
        };
        let (ap_live_6, _) = fresh_cx(|cx| live.apply(&step6, cx));
        let (ap_ref_6, _) = fresh_cx(|cx| dxf_ref.apply(&step6, cx));
        assert_eq!(ap_live_6, ap_ref_6);
        {
            let mut ref_bytes_6 = Vec::new();
            let _ = fresh_cx(|cx| Dxf::write(&dxf_ref, &mut ref_bytes_6, cx));
            let (live_bytes_6, _) = fresh_cx(|cx| live.save(cx));
            assert_eq!(live_bytes_6.ok().unwrap_or_default(), ref_bytes_6);
            let (live_d6, _) = fresh_cx(|cx| live.drawing(cx));
            let (ref_d6, _) = fresh_cx(|cx| dxf_ref.lift(cx));
            assert_eq!(live_d6, ref_d6);
            snapshots.push((ref_bytes_6, live.state_id()));
        }

        // Spatial query check
        let mut hits = Vec::new();
        live.select_rect([200.0, 150.0, 260.0, 190.0], &mut hits);
        assert!(!hits.is_empty());

        // Full Undo walk back to initial state, verifying byte-for-byte save & state_id at each step!
        for idx in (0..snapshots.len().saturating_sub(1)).rev() {
            let prev_rev = live.revision();
            let (undone, _) = fresh_cx(|cx| live.undo(cx));
            assert_eq!(undone, Ok(true));
            assert!(live.revision() > prev_rev);
            if let Some((expected_bytes, expected_sid)) = snapshots.get(idx) {
                assert_eq!(live.state_id(), *expected_sid);
                let (saved, _) = fresh_cx(|cx| live.save(cx));
                assert_eq!(
                    saved.ok().unwrap_or_default(),
                    *expected_bytes,
                    "undo step {idx}"
                );
            }
        }
        assert_eq!(fresh_cx(|cx| live.undo(cx)).0, Ok(false));

        // Full Redo walk forward to final state, verifying byte-for-byte save & state_id at each step!
        for idx in 1..snapshots.len() {
            let prev_rev = live.revision();
            let (redone, _) = fresh_cx(|cx| live.redo(cx));
            assert_eq!(redone, Ok(true));
            assert!(live.revision() > prev_rev);
            if let Some((expected_bytes, expected_sid)) = snapshots.get(idx) {
                assert_eq!(live.state_id(), *expected_sid);
                let (saved, _) = fresh_cx(|cx| live.save(cx));
                assert_eq!(
                    saved.ok().unwrap_or_default(),
                    *expected_bytes,
                    "redo step {idx}"
                );
            }
        }
        assert_eq!(fresh_cx(|cx| live.redo(cx)).0, Ok(false));
    }

    #[test]
    fn dwg_fast_edits_match_apply_bytes_and_compact_save_reloads_identically() {
        let dwg_dir = samples_dir().join("dwg");
        let Ok(entries) = std::fs::read_dir(&dwg_dir) else {
            return;
        };
        let mut paths: Vec<PathBuf> = entries
            .filter_map(|e| e.ok().map(|x| x.path()))
            .filter(|p| {
                p.extension()
                    .and_then(|s| s.to_str())
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("dwg"))
            })
            .collect();
        paths.sort();

        let mut tested = 0usize;
        for path in paths {
            let Ok(src) = std::fs::read(&path) else {
                continue;
            };
            let (Ok(mut dwg_ref), _) = fresh_cx(|cx| Dwg::read(&src, cx)) else {
                continue;
            };
            if dwg_ref.version() == b"AC1021" {
                let (Ok(mut live_2007), _) = fresh_cx(|cx| AcadLive::open_dwg(&src, cx)) else {
                    continue;
                };
                let (res, _) = fresh_cx(|cx| {
                    live_2007.apply(
                        &Changes {
                            document: None,
                            edits: Vec::new(),
                        },
                        cx,
                    )
                });
                assert_eq!(res, Err(Error::Unsupported));
                continue;
            }
            let (Ok(mut live), _) = fresh_cx(|cx| AcadLive::open_dwg(&src, cx)) else {
                continue;
            };
            let (Ok(d0), _) = fresh_cx(|cx| live.drawing(cx)) else {
                continue;
            };
            let Some(sp0) = d0.spaces.first() else {
                continue;
            };
            if sp0.elements.is_empty() {
                continue;
            }

            let place = id::space(MODEL);
            let mut namer = id::Namer::new(&place);
            let mut first_line_target: Option<Seen> = None;
            for (idx, el) in sp0.elements.iter().enumerate() {
                let eid = namer.next(idx, el);
                if eid.starts_with("space/0/h")
                    && matches!(el.shape, Shape::Line { .. } | Shape::Circle { .. })
                {
                    first_line_target = Some(Seen {
                        id: eid,
                        hash: Hash::element(el),
                    });
                    break;
                }
            }

            // Step 1: Add a LINE + SetPointStyle (or Transform existing entity)
            let mut edits1 = Vec::new();
            if let Some(seen) = first_line_target {
                edits1.push(Edit::Transform {
                    target: seen,
                    transform: Affine::translation(5.0, 7.5),
                });
            }
            edits1.push(Edit::Add {
                place: "space/0".to_owned(),
                element: Box::new(Placed::Model(Element {
                    source: Ref::default(),
                    layer: LayerId(0),
                    style: default_style(),
                    shape: Shape::Line {
                        start: pt(1.0, 2.0),
                        end: pt(30.0, 40.0),
                    },
                })),
            });
            let step1 = Changes {
                document: None,
                edits: edits1,
            };
            let (ap_live_1, diag_live_1) = fresh_cx(|cx| live.apply(&step1, cx));
            let (ap_ref_1, diag_ref_1) = fresh_cx(|cx| dwg_ref.apply(&step1, cx));
            assert_eq!(
                ap_live_1,
                ap_ref_1,
                "DWG step1 applied mismatch on {}",
                path.display()
            );
            assert_eq!(diag_live_1, diag_ref_1);
            let Ok(applied_vec) = ap_live_1 else {
                continue;
            };
            let added_ap = applied_vec
                .last()
                .cloned()
                .unwrap_or(zumen_drawing::Applied {
                    id: None,
                    hash: None,
                });

            // Step 2: Transform the newly added LINE!
            if let (Some(added_id), Some(added_hash)) = (added_ap.id, added_ap.hash) {
                let step2 = Changes {
                    document: None,
                    edits: vec![Edit::Transform {
                        target: Seen {
                            id: added_id,
                            hash: added_hash,
                        },
                        transform: Affine::translation(100.0, -50.0),
                    }],
                };
                let (ap_live_2, _) = fresh_cx(|cx| live.apply(&step2, cx));
                let (ap_ref_2, _) = fresh_cx(|cx| dwg_ref.apply(&step2, cx));
                assert_eq!(
                    ap_live_2,
                    ap_ref_2,
                    "DWG step2 applied mismatch on {}",
                    path.display()
                );
            }

            // Full save MUST match DwgFile::apply byte-for-byte!
            let mut expected_dwg = Vec::new();
            let _ = fresh_cx(|cx| Dwg::write(&dwg_ref, &mut expected_dwg, cx));
            let (saved_full, _) = fresh_cx(|cx| live.save(cx));
            let saved_full = saved_full.ok().unwrap_or_default();
            assert_eq!(
                saved_full,
                expected_dwg,
                "DWG full save byte mismatch on {}",
                path.display()
            );

            // Compact save: reload model, IDs, hashes, and diagnostics MUST match full save,
            // and output bytes must be deterministic!
            let (saved_compact, _) = fresh_cx(|cx| live.save_compact(cx));
            let saved_compact = saved_compact.ok().unwrap_or_default();
            let (saved_compact_again, _) = fresh_cx(|cx| live.clone().save_compact(cx));
            assert_eq!(
                saved_compact,
                saved_compact_again.ok().unwrap_or_default(),
                "compact save determinism mismatch on {}",
                path.display()
            );
            let (reloaded_full, diag_full) = fresh_cx(|cx| {
                let f = Dwg::read(&saved_full, cx)?;
                f.lift(cx)
            });
            let (reopened_live_full, diag_live_full) = fresh_cx(|cx| {
                let l = AcadLive::open_dwg(&saved_full, cx)?;
                l.drawing(cx)
            });
            assert_eq!(reopened_live_full, reloaded_full);
            assert_eq!(diag_live_full, diag_full);
            let (reloaded_compact, diag_compact) = fresh_cx(|cx| {
                let f = Dwg::read(&saved_compact, cx)?;
                f.lift(cx)
            });
            let (resident_d, _) = fresh_cx(|cx| live.drawing(cx));
            assert_eq!(
                reloaded_compact.as_ref().map(drawing_without_byte_ranges),
                reloaded_full.as_ref().map(drawing_without_byte_ranges),
                "compact reload mismatch on {}",
                path.display()
            );
            assert_eq!(
                resident_d.as_ref().map(drawing_without_byte_ranges),
                reloaded_full.as_ref().map(drawing_without_byte_ranges),
                "resident drawing mismatch on {}",
                path.display()
            );
            assert_eq!(
                diag_without_byte_ranges(&diag_compact),
                diag_without_byte_ranges(&diag_full)
            );
            if let (Ok(df), Ok(dc), Ok(dr)) = (&reloaded_full, &reloaded_compact, &resident_d) {
                assert_eq!(
                    all_element_ids_and_hashes(dc),
                    all_element_ids_and_hashes(df)
                );
                assert_eq!(
                    all_element_ids_and_hashes(dr),
                    all_element_ids_and_hashes(df)
                );
            }

            // Undo all steps -> must match initial Dwg::write byte-for-byte!
            while fresh_cx(|cx| live.undo(cx)).0 == Ok(true) {}
            let mut orig_written = Vec::new();
            let orig_file = fresh_cx(|cx| Dwg::read(&src, cx)).0.ok();
            if let Some(of) = orig_file {
                let _ = fresh_cx(|cx| Dwg::write(&of, &mut orig_written, cx));
                let (after_undo_bytes, _) = fresh_cx(|cx| live.save(cx));
                assert_eq!(after_undo_bytes.ok().unwrap_or_default(), orig_written);
            }
            tested += 1;
        }
        assert!(tested > 0, "expected at least one editable DWG sample");
    }

    #[test]
    fn budget_and_cancellation_leave_live_state_untouched() {
        let base_dxf = {
            let mut d = zumen_drawing::Drawing2D::default();
            d.layers.push(Layer {
                id: LayerId(0),
                name: "0".to_owned(),
                follows_placement: true,
                state: None,
                style: None,
                description: None,
                source: Ref::default(),
            });
            d.spaces.push(zumen_drawing::ModelSpace {
                id: MODEL,
                unit: zumen_drawing::Unit::Millimeter,
                axes: zumen_drawing::Axes::EastNorth,
                elements: vec![
                    Element {
                        source: Ref::default(),
                        layer: LayerId(0),
                        style: default_style(),
                        shape: Shape::Polyline(Polyline {
                            vertices: vec![
                                Vertex {
                                    point: pt(0.0, 0.0),
                                    bulge: 0.0,
                                },
                                Vertex {
                                    point: pt(10.0, 0.0),
                                    bulge: 0.0,
                                },
                            ],
                            closed: false,
                            width: None,
                            vertex_widths: Vec::new(),
                            line_type_generation: false,
                        }),
                    },
                    Element {
                        source: Ref::default(),
                        layer: LayerId(0),
                        style: default_style(),
                        shape: Shape::Text(Text {
                            content: "BUDGET".to_owned(),
                            height: Height {
                                value: mlen(3.0),
                                meaning: HeightMeaning::Declared("dxf.TEXT.40".into()),
                            },
                            placement: Placement::Between {
                                first: pt(0.0, 10.0),
                                second: pt(20.0, 10.0),
                                fit: Fit::Aligned,
                            },
                            rotation: 0.0,
                            width_factor: 1.0,
                            oblique: 0.0,
                            backward: false,
                            upside_down: false,
                            style: "Standard".to_owned(),
                        }),
                    },
                ],
                order: vec![0, 1],
            });
            let (res, _) = fresh_cx(|cx| crate::lower_to_dxf(&d, cx));
            res.map(|l| l.bytes).unwrap_or_default()
        };

        let (Ok(mut live), _) = fresh_cx(|cx| AcadLive::open_dxf(&base_dxf, cx)) else {
            return;
        };
        let before_rev = live.revision();
        let before_sid = live.state_id();
        let (before_bytes, _) = fresh_cx(|cx| live.save(cx));

        // キャンセル済みのトークンで `apply` を呼んでも状態は 1 ビットも変わらない
        let mut budget = Budget::default();
        let mut diag = Diagnostics::new();
        let cancel = CancelToken::new();
        cancel.cancel();
        let add_change = Changes {
            document: None,
            edits: vec![Edit::Add {
                place: "space/0".to_owned(),
                element: Box::new(Placed::Model(Element {
                    source: Ref::default(),
                    layer: LayerId(0),
                    style: default_style(),
                    shape: Shape::Symbol(Symbol {
                        key: SymbolKey {
                            namespace: "dxf.block".into(),
                            name: "NONE".to_owned(),
                        },
                        position: pt(0.0, 0.0),
                        scale: [1.0, 1.0],
                        rotation: 0.0,
                        grid: None,
                        attributes: vec![Attribute {
                            tag: "T".to_owned(),
                            text: Text {
                                content: "V".to_owned(),
                                height: Height {
                                    value: mlen(2.0),
                                    meaning: HeightMeaning::Declared("dxf.ATTRIB.40".into()),
                                },
                                placement: Placement::Anchor {
                                    point: pt(0.0, 0.0),
                                    horizontal: Horizontal::Left,
                                    vertical: Vertical::Baseline,
                                },
                                rotation: 0.0,
                                width_factor: 1.0,
                                oblique: 0.0,
                                backward: false,
                                upside_down: false,
                                style: String::new(),
                            },
                            formatted: None,
                            hidden: false,
                            layer: LayerId(0),
                            style: default_style(),
                            source: Ref::default(),
                        }],
                        clip: None,
                    }),
                })),
            }],
        };
        let res = live.apply(&add_change, &mut Cx::new(&mut budget, &mut diag, &cancel));
        assert!(matches!(res, Err(Error::Stopped(_))));
        assert_eq!(live.revision(), before_rev);
        assert_eq!(live.state_id(), before_sid);
        assert_eq!(fresh_cx(|cx| live.save(cx)).0, before_bytes);

        // Work 上限 0 でも状態は無傷
        let mut zero_work = Budget::new(Limits::default().with(Kind::Work, Limit::Finite(0)));
        let mut diag2 = Diagnostics::new();
        let cancel2 = CancelToken::new();
        let res2 = live.apply(
            &Changes {
                document: None,
                edits: Vec::new(),
            },
            &mut Cx::new(&mut zero_work, &mut diag2, &cancel2),
        );
        assert!(matches!(res2, Err(Error::Stopped(_))));
        assert_eq!(live.revision(), before_rev);
        assert_eq!(live.state_id(), before_sid);
    }
}
