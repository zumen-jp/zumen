//! DXF・DWG の常駐文書エンジン [`AcadLive`]（`docs/live.md`、#878）。
//!
//! 形式非依存の基盤 [`zumen_live`]（ページ配列・コピーオンライト列・格子空間索引・64 要素単位の差分表示バッファー・
//! 原子トランザクションと履歴）の上に、DXF（ASCII・バイナリ）と DWG（R2000〜R2018）の単一レコード復号・差分書き戻し・
//! 全体を書く保存（[`AcadLive::save`]）と後ろに足す保存（[`AcadLive::save_compact`]）を統合する。

mod backing;
mod edit;
#[cfg(test)]
mod equivalence;
mod implicit;
mod ipc;
mod open;
mod query;
mod shape;

use zumen_base::Cx;
use zumen_drawing::{Applied, Changes, Drawing2D, Element, ModelPoint, Seen};
use zumen_live::display::Display;
use zumen_live::{DepGraph, Revision, RowId, SpatialGrid, VersionedTable};
use zumen_sdk::Error;

use crate::{DwgFile, DxfFile};
use backing::LiveBacking;
pub use backing::LiveFormat;
use edit::{DEFAULT_HISTORY_MAX_BYTES, LiveJournal, apply_changes};
use open::{LiveState, open_auto, open_dwg, open_dxf, open_from_dwg_file, open_from_dxf_file};
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

    /// 復号済みの [`DxfFile`] から文書エンジンを組み立てる
    pub fn open_from_dxf_file(file: DxfFile, cx: &mut Cx<'_>) -> Result<Self, Error> {
        let state = open_from_dxf_file(file, cx)?;
        Ok(Self {
            state,
            journal: LiveJournal::new(DEFAULT_HISTORY_MAX_BYTES),
            threads: 1,
        })
    }

    /// 復号済みの [`DwgFile`] から文書エンジンを組み立てる
    pub fn open_from_dwg_file(file: DwgFile, cx: &mut Cx<'_>) -> Result<Self, Error> {
        let state = open_from_dwg_file(file, 1, cx)?;
        Ok(Self {
            state,
            journal: LiveJournal::new(DEFAULT_HISTORY_MAX_BYTES),
            threads: 1,
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

    /// DWG として開いている場合のバージョン文字列（`b"AC1015"` など）を返す
    #[inline]
    pub fn dwg_version(&self) -> Option<&[u8]> {
        match &self.state.backing {
            LiveBacking::Dwg(dwg) => Some(dwg.base_file.version()),
            _ => None,
        }
    }

    /// 未編集の DWG を保持している場合にその [`DwgFile`] を借りる
    #[inline]
    pub const fn unmodified_dwg_file(&self) -> Option<&DwgFile> {
        match &self.state.backing {
            LiveBacking::Dwg(dwg) if dwg.step_cursor == 0 => Some(&dwg.base_file),
            _ => None,
        }
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

    /// ブロック定義・ハッチング境界・引出線注釈・連想寸法の依存グラフを借りる
    #[inline]
    pub const fn dep_graph(&self) -> &DepGraph {
        &self.state.dep
    }

    /// ブロック定義の添字（`Drawing2D::symbols` 上の位置）に対応する仮想依存ノード ID を返す
    #[inline]
    pub const fn symbol_dep_node(def_index: u32) -> Option<RowId> {
        RowId::virtual_node(def_index)
    }

    /// 変更された行または仮想ノード群 `changed` から推移的に影響を受けるノードを番号昇順で返す
    pub fn affected_rows(&self, changed: &[RowId], cx: &mut Cx<'_>) -> Result<Vec<RowId>, Error> {
        self.state
            .dep
            .affected_rows_with_cx(changed, cx)
            .map_err(|stop| {
                cx.report(stop.to_diagnostic());
                Error::Stopped(stop)
            })
    }

    /// 内容または入れ子先の変更により再送が必要なブロック定義の添字集合への参照を返す
    #[inline]
    pub fn dirty_symbols(&self) -> &std::collections::BTreeSet<u32> {
        &self.state.dirty_symbols
    }

    /// ブロック定義の変更フラグを消去する
    #[inline]
    pub fn clear_dirty_symbols(&mut self) {
        self.state.dirty_symbols.clear();
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

    /// 共有予算で、モデル空間の矩形と囲む箱が交差する生存行を昇順で `out` に足す。
    ///
    /// 矩形は `[min_x, min_y, max_x, max_y]`。逆転・NaN は空、無限大は全域指定に使える。
    /// 停止時は元の `out` を保ち、診断を 1 件報告する。行は取得時の [`Self::revision`] に属する。
    pub fn select_rect_with_cx(
        &self,
        query: [f64; 4],
        out: &mut Vec<RowId>,
        cx: &mut Cx<'_>,
    ) -> Result<(), Error> {
        self.state.select_rect_with_cx(query, out, cx)
    }

    /// 生きている行 `row` から図面モデルの [`Element<ModelPoint>`] を 1 件だけ復元する。
    /// DWG の出所は保持時の位置。保存結果と照らす場合は [`Self::element_with_source`] を使う。
    pub fn element_of_row(&self, row: RowId) -> Option<Element<ModelPoint>> {
        self.state.element_of_row(row)
    }

    /// 表示の行から、現在の正規 ID とハッシュを共有予算で取得する。
    ///
    /// 行番号は現在の版だけで使う。全体を作り直す編集や履歴の復元をまたぐ呼び出しでは、
    /// 呼び出す側が [`Self::revision`] を確認する。削除済み・範囲外の行は `None`。
    /// 出所の位置はハッシュに含まれないため、保存位置の全体対応表は作らない。
    pub fn seen_of_row(&self, row: RowId, cx: &mut Cx<'_>) -> Result<Option<Seen>, Error> {
        self.state.seen_of_row(row, cx)
    }

    /// 保存後と同じ出所を持つ要素を、共有予算で復元する。
    pub fn element_with_source(
        &self,
        row: RowId,
        cx: &mut Cx<'_>,
    ) -> Result<Option<Element<ModelPoint>>, Error> {
        self.state.element_with_source(row, cx)
    }

    /// 現在の常駐状態から [`Drawing2D`] を組み立てる。保存したバイト列をファイルモデルで読み直して変換したときと
    /// 同じ変換の診断を返す（開いたときの診断を、場所を今の保存の位置へ移して返す）
    pub fn drawing(&self, cx: &mut Cx<'_>) -> Result<Drawing2D, Error> {
        self.state.to_drawing(cx)
    }

    /// 変更 `changes` を当てて履歴に積み、各項目の適用後 ID・ハッシュを返す
    pub fn apply(&mut self, changes: &Changes, cx: &mut Cx<'_>) -> Result<Vec<Applied>, Error> {
        self.apply_rereading(changes, cx).0
    }

    /// [`Self::apply`] と同じく当て、保存したバイト列を読み直して書き戻しの参照（[`zumen_sdk::Apply`]）で当てる
    /// 経路に入ったかも返す。その経路は、読み直したファイルの診断（DWG の型どおりに読めないオブジェクトなど）を、
    /// ファイルモデルに当てたときと同じく返す。差分の経路と空の変更は、それらを返さない
    pub fn apply_rereading(
        &mut self,
        changes: &Changes,
        cx: &mut Cx<'_>,
    ) -> (Result<Vec<Applied>, Error>, bool) {
        let mut rereads = false;
        let result = apply_changes(
            &mut self.state,
            &mut self.journal,
            changes,
            self.threads,
            &mut rereads,
            cx,
        );
        (result, rereads)
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

    pub(super) fn fresh_cx<R>(f: impl FnOnce(&mut Cx<'_>) -> R) -> (R, Vec<Diagnostic>) {
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

    pub(super) fn all_element_ids_and_hashes(
        drawing: &zumen_drawing::Drawing2D,
    ) -> Vec<(String, Hash)> {
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

    pub(super) fn synthetic_rich_dxf() -> Vec<u8> {
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
                let (live_1, live_diag_1) =
                    fresh_cx(|cx| AcadLive::open_with_threads(&bytes, 1, cx));
                let live_1 = live_1.and_then(|live| {
                    let (drawing, drawing_diag) = fresh_cx(|cx| live.drawing(cx));
                    assert_eq!(
                        drawing_diag,
                        ref_diag,
                        "drawing diagnostics mismatch on {}",
                        path.display()
                    );
                    drawing.map(|d| (live, d))
                });
                let (live_4, live_diag_4) =
                    fresh_cx(|cx| AcadLive::open_with_threads(&bytes, 4, cx));
                let live_4 = live_4.and_then(|live| {
                    let (drawing, drawing_diag) = fresh_cx(|cx| live.drawing(cx));
                    assert_eq!(
                        drawing_diag,
                        ref_diag,
                        "drawing diagnostics mismatch on {}",
                        path.display()
                    );
                    drawing.map(|d| (live, d))
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
    fn dxf_record_sources_match_reload_after_fast_edits() {
        check_dxf_record_sources(synthetic_rich_dxf());
    }

    #[test]
    fn dxf_record_sources_match_when_definitions_follow_entities() {
        let text = String::from_utf8(synthetic_rich_dxf()).unwrap();
        let mut sections: Vec<_> = text.split_inclusive("  0\nENDSEC\n").collect();
        let index = sections
            .iter()
            .position(|section| section.starts_with("  0\nSECTION\n  2\nENTITIES\n"))
            .unwrap();
        let entities = sections.remove(index);
        sections.insert(1, entities);
        check_dxf_record_sources(sections.concat().into_bytes());
    }

    fn check_dxf_record_sources(src: Vec<u8>) {
        let mut live = fresh_cx(|cx| AcadLive::open_dxf(&src, cx)).0.unwrap();
        let mut reference = fresh_cx(|cx| Dxf::read(&src, cx)).0.unwrap();
        for step in 0..6 {
            let before = fresh_cx(|cx| live.drawing(cx)).0.unwrap();
            let before_bytes = fresh_cx(|cx| live.save(cx)).0.unwrap();
            let element = if step == 4 {
                before.spaces[0]
                    .elements
                    .iter()
                    .find(|element| matches!(element.shape, Shape::Symbol(_)))
                    .unwrap()
                    .clone()
            } else {
                before.spaces[0].elements[0].clone()
            };
            let edit = if step % 2 == 0 {
                Edit::Add {
                    place: "space/0".to_owned(),
                    element: Box::new(Placed::Model(element)),
                }
            } else {
                Edit::Remove {
                    target: Seen {
                        id: all_element_ids_and_hashes(&before)[0].0.clone(),
                        hash: Hash::element(&element),
                    },
                }
            };
            let changes = Changes {
                document: None,
                edits: vec![edit],
            };
            assert_eq!(
                fresh_cx(|cx| live.apply(&changes, cx)),
                fresh_cx(|cx| reference.apply(&changes, cx)),
                "step {step}"
            );
            let saved = fresh_cx(|cx| live.save(cx)).0.unwrap();
            let expected = fresh_cx(|cx| {
                let mut bytes = Vec::new();
                Dxf::write(&reference, &mut bytes, cx).map(|()| bytes)
            })
            .0
            .unwrap();
            assert_eq!(saved, expected, "step {step}");
            let reloaded = fresh_cx(|cx| Dxf::read(&saved, cx).and_then(|f| f.lift(cx)))
                .0
                .unwrap();
            let retained = fresh_cx(|cx| live.drawing(cx)).0.unwrap();
            assert_eq!(retained, reloaded, "step {step}");
            assert!(fresh_cx(|cx| live.undo(cx)).0.unwrap());
            assert_eq!(fresh_cx(|cx| live.drawing(cx)).0.unwrap(), before);
            assert_eq!(fresh_cx(|cx| live.save(cx)).0.unwrap(), before_bytes);
            assert!(fresh_cx(|cx| live.redo(cx)).0.unwrap());
            assert_eq!(fresh_cx(|cx| live.drawing(cx)).0.unwrap(), retained);
            assert_eq!(fresh_cx(|cx| live.save(cx)).0.unwrap(), saved);
        }
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
            let (reopened, diag_open_full) = fresh_cx(|cx| AcadLive::open_dwg(&saved_full, cx));
            let reopened = reopened.unwrap();
            let (reopened_live_full, diag_live_full) = fresh_cx(|cx| reopened.drawing(cx));
            assert_eq!(reopened_live_full, reloaded_full);
            assert_eq!(diag_open_full, diag_full);
            assert_eq!(diag_live_full, diag_full);
            let (reloaded_compact, diag_compact) = fresh_cx(|cx| {
                let f = Dwg::read(&saved_compact, cx)?;
                f.lift(cx)
            });
            let (resident_d, resident_diag) = fresh_cx(|cx| live.drawing(cx));
            assert_eq!(
                resident_diag,
                diag_full,
                "resident diagnostics mismatch on {}",
                path.display()
            );
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

    /// 構造の経路で積んだ履歴を Work 上限 0 で取り消し・やり直しすると、停止を返し、
    /// 履歴の段数・版・状態 ID・保存を変えない（#887 の合成の回帰）
    #[test]
    fn stopped_structural_undo_redo_keeps_history() {
        let src = synthetic_rich_dxf();
        let (opened, _) = fresh_cx(|cx| AcadLive::open_dxf(&src, cx));
        let mut live = opened.unwrap();
        let add_layer = Changes {
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
        let ((applied, rereads), _) = fresh_cx(|cx| live.apply_rereading(&add_layer, cx));
        applied.unwrap();
        assert!(rereads);

        let snapshot = |live: &AcadLive| {
            (
                live.undo_len(),
                live.redo_len(),
                live.revision(),
                live.state_id(),
                fresh_cx(|cx| live.save(cx)).0.unwrap(),
            )
        };
        let stopped = |live: &mut AcadLive, redo: bool| {
            let mut zero = Budget::new(Limits::default().with(Kind::Work, Limit::Finite(0)));
            let mut diagnostics = Diagnostics::new();
            let cancel = CancelToken::new();
            let mut cx = Cx::new(&mut zero, &mut diagnostics, &cancel);
            let result = if redo {
                live.redo(&mut cx)
            } else {
                live.undo(&mut cx)
            };
            assert!(matches!(result, Err(Error::Stopped(_))));
            assert_eq!(diagnostics.into_vec().len(), 1);
        };

        let applied_state = snapshot(&live);
        assert_eq!((applied_state.0, applied_state.1), (1, 0));
        stopped(&mut live, false);
        assert_eq!(snapshot(&live), applied_state);

        assert!(fresh_cx(|cx| live.undo(cx)).0.unwrap());
        let undone_state = snapshot(&live);
        assert_eq!((undone_state.0, undone_state.1), (0, 1));
        stopped(&mut live, true);
        assert_eq!(snapshot(&live), undone_state);

        // 停止のあとの取り消し・やり直しは、版を進めて元の段数と保存に戻る
        assert!(fresh_cx(|cx| live.redo(cx)).0.unwrap());
        let redone_state = snapshot(&live);
        assert_eq!((redone_state.0, redone_state.1), (1, 0));
        assert!(redone_state.4 == applied_state.4);
    }

    pub(super) fn synthetic_associative_dxf() -> Vec<u8> {
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
        tag(5, "300");
        tag(9, "$EXTMIN");
        tag(10, "0.0");
        tag(20, "-10.0");
        tag(30, "0.0");
        tag(9, "$EXTMAX");
        tag(10, "60.0");
        tag(20, "30.0");
        tag(30, "0.0");
        tag(0, "ENDSEC");

        tag(0, "SECTION");
        tag(2, "TABLES");
        tag(0, "TABLE");
        tag(2, "LTYPE");
        tag(5, "10");
        tag(100, "AcDbSymbolTable");
        tag(70, "1");
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
        tag(0, "ENDTAB");

        tag(0, "TABLE");
        tag(2, "LAYER");
        tag(5, "20");
        tag(100, "AcDbSymbolTable");
        tag(70, "1");
        tag(0, "LAYER");
        tag(5, "21");
        tag(100, "AcDbSymbolTableRecord");
        tag(100, "AcDbLayerTableRecord");
        tag(2, "0");
        tag(70, "0");
        tag(62, "7");
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
        tag(2, "DIMSTYLE");
        tag(5, "60");
        tag(100, "AcDbSymbolTable");
        tag(70, "1");
        tag(0, "DIMSTYLE");
        tag(105, "61");
        tag(100, "AcDbSymbolTableRecord");
        tag(100, "AcDbDimStyleTableRecord");
        tag(2, "Standard");
        tag(70, "0");
        tag(340, "31");
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
        tag(5, "44");
        tag(100, "AcDbSymbolTableRecord");
        tag(100, "AcDbBlockTableRecord");
        tag(2, "*D1");
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
        tag(330, "44");
        tag(100, "AcDbEntity");
        tag(8, "0");
        tag(100, "AcDbBlockBegin");
        tag(2, "*D1");
        tag(70, "1");
        tag(10, "0.0");
        tag(20, "0.0");
        tag(30, "0.0");
        tag(3, "*D1");
        tag(1, "");
        tag(0, "LINE");
        tag(5, "55");
        tag(330, "44");
        tag(100, "AcDbEntity");
        tag(8, "0");
        tag(100, "AcDbLine");
        tag(10, "0.0");
        tag(20, "-10.0");
        tag(30, "0.0");
        tag(11, "40.0");
        tag(21, "-10.0");
        tag(31, "0.0");
        tag(0, "ENDBLK");
        tag(5, "56");
        tag(330, "44");
        tag(100, "AcDbEntity");
        tag(8, "0");
        tag(100, "AcDbBlockEnd");
        tag(0, "ENDSEC");

        tag(0, "SECTION");
        tag(2, "ENTITIES");
        // 1. LINE (h101) — HATCH 境界 (330=101) および DIMASSOC 測定対象 (331=101)
        tag(0, "LINE");
        tag(5, "101");
        tag(330, "41");
        tag(100, "AcDbEntity");
        tag(8, "0");
        tag(100, "AcDbLine");
        tag(10, "0.0");
        tag(20, "0.0");
        tag(30, "0.0");
        tag(11, "40.0");
        tag(21, "0.0");
        tag(31, "0.0");
        // 2. 連想 HATCH (h102, 境界元 330=101)
        tag(0, "HATCH");
        tag(5, "102");
        tag(330, "41");
        tag(100, "AcDbEntity");
        tag(8, "0");
        tag(100, "AcDbHatch");
        tag(10, "0.0");
        tag(20, "0.0");
        tag(30, "0.0");
        tag(210, "0.0");
        tag(220, "0.0");
        tag(230, "1.0");
        tag(2, "SOLID");
        tag(70, "1");
        tag(71, "1");
        tag(91, "1");
        tag(92, "3");
        tag(72, "0");
        tag(73, "1");
        tag(93, "4");
        tag(10, "0.0");
        tag(20, "0.0");
        tag(10, "40.0");
        tag(20, "0.0");
        tag(10, "40.0");
        tag(20, "20.0");
        tag(10, "0.0");
        tag(20, "20.0");
        tag(97, "1");
        tag(330, "101");
        tag(75, "0");
        tag(76, "1");
        tag(98, "0");
        // 3. MTEXT (h103) — LEADER の注記先
        tag(0, "MTEXT");
        tag(5, "103");
        tag(102, "{ACAD_REACTORS");
        tag(330, "104");
        tag(102, "}");
        tag(330, "41");
        tag(100, "AcDbEntity");
        tag(8, "0");
        tag(100, "AcDbMText");
        tag(10, "50.0");
        tag(20, "25.0");
        tag(30, "0.0");
        tag(40, "2.5");
        tag(41, "20.0");
        tag(71, "1");
        tag(72, "5");
        tag(1, "NOTE");
        tag(7, "Standard");
        // 4. 連想 LEADER (h104, 注記 340=103)
        tag(0, "LEADER");
        tag(5, "104");
        tag(330, "41");
        tag(100, "AcDbEntity");
        tag(8, "0");
        tag(100, "AcDbLeader");
        tag(3, "Standard");
        tag(71, "1");
        tag(72, "0");
        tag(73, "0");
        tag(74, "1");
        tag(75, "0");
        tag(40, "2.5");
        tag(41, "10.0");
        tag(76, "2");
        tag(10, "10.0");
        tag(20, "10.0");
        tag(30, "0.0");
        tag(10, "48.0");
        tag(20, "25.0");
        tag(30, "0.0");
        tag(340, "103");
        // 5. 連想 DIMENSION (h105, 拡張辞書 360=201 -> DIMASSOC 202 -> 331=101)
        tag(0, "DIMENSION");
        tag(5, "105");
        tag(102, "{ACAD_XDICTIONARY");
        tag(360, "201");
        tag(102, "}");
        tag(330, "41");
        tag(100, "AcDbEntity");
        tag(8, "0");
        tag(100, "AcDbDimension");
        tag(2, "*D1");
        tag(10, "40.0");
        tag(20, "-10.0");
        tag(30, "0.0");
        tag(11, "20.0");
        tag(21, "-10.0");
        tag(31, "0.0");
        tag(70, "32");
        tag(3, "Standard");
        tag(100, "AcDbAlignedDimension");
        tag(13, "0.0");
        tag(23, "0.0");
        tag(33, "0.0");
        tag(14, "40.0");
        tag(24, "0.0");
        tag(34, "0.0");
        tag(100, "AcDbRotatedDimension");
        tag(0, "ENDSEC");

        tag(0, "SECTION");
        tag(2, "OBJECTS");
        tag(0, "DICTIONARY");
        tag(5, "200");
        tag(100, "AcDbDictionary");
        tag(281, "1");
        tag(0, "DICTIONARY");
        tag(5, "201");
        tag(330, "105");
        tag(100, "AcDbDictionary");
        tag(281, "1");
        tag(3, "ACAD_DIMASSOC");
        tag(360, "202");
        tag(0, "DIMASSOC");
        tag(5, "202");
        tag(330, "201");
        tag(100, "AcDbDimAssoc");
        tag(330, "105");
        tag(90, "1");
        tag(70, "0");
        tag(1, "AcDbOsnapPointRef");
        tag(72, "1");
        tag(331, "101");
        tag(0, "ENDSEC");
        tag(0, "EOF");
        dxf.into_bytes()
    }

    #[test]
    fn associative_hatch_leader_and_dimassoc_deps_and_parity() {
        let src = synthetic_associative_dxf();
        let (Ok(mut dxf_ref), _) = fresh_cx(|cx| Dxf::read(&src, cx)) else {
            return;
        };
        let (Ok(mut live), _) = fresh_cx(|cx| AcadLive::open_dxf(&src, cx)) else {
            return;
        };
        let Some(row_line) = live.table().find_handle(0x101) else {
            return;
        };
        let Some(row_hatch) = live.table().find_handle(0x102) else {
            return;
        };
        let Some(row_mtext) = live.table().find_handle(0x103) else {
            return;
        };
        let Some(row_leader) = live.table().find_handle(0x104) else {
            return;
        };
        let Some(row_dim) = live.table().find_handle(0x105) else {
            return;
        };

        // 1. 依存グラフが LINE -> {HATCH, DIMENSION} と MTEXT -> LEADER を保持していることを確かめる
        let (line_affected, _) = fresh_cx(|cx| live.affected_rows(&[row_line], cx));
        let line_affected = line_affected.ok().unwrap_or_default();
        assert!(line_affected.contains(&row_hatch));
        assert!(line_affected.contains(&row_dim));
        let (mtext_affected, _) = fresh_cx(|cx| live.affected_rows(&[row_mtext], cx));
        assert_eq!(mtext_affected.ok().unwrap_or_default(), vec![row_leader]);

        let get_seen = |live: &AcadLive, target_id: &str| -> Seen {
            let (d, _) = fresh_cx(|cx| live.drawing(cx));
            let d = d.ok().unwrap_or_default();
            let pair = all_element_ids_and_hashes(&d)
                .into_iter()
                .find(|(id, _)| id == target_id);
            Seen {
                id: target_id.to_owned(),
                hash: pair.map_or(Hash([0; 16]), |(_, h)| h),
            }
        };

        // 2. 連想 HATCH (h102) の Restyle と連想 LEADER (h104) の Transform が DxfFile::apply とバイト列一致
        let step1 = Changes {
            document: None,
            edits: vec![
                Edit::Restyle {
                    target: get_seen(&live, "space/0/h102"),
                    style: Restyle {
                        layer: None,
                        color: Some(Inherit::Own(Color {
                            code: Some(ColorCode {
                                system: ColorSystem::Aci,
                                value: 4,
                            }),
                            rgb: None,
                        })),
                        line_type: None,
                        line_type_scale: None,
                        line_weight: None,
                    },
                },
                Edit::Transform {
                    target: get_seen(&live, "space/0/h104"),
                    transform: Affine::translation(2.0, 3.0),
                },
            ],
        };
        let (ap_live_1, diag_live_1) = fresh_cx(|cx| live.apply(&step1, cx));
        let (ap_ref_1, diag_ref_1) = fresh_cx(|cx| dxf_ref.apply(&step1, cx));
        assert!(ap_live_1.is_ok());
        assert_eq!(ap_live_1, ap_ref_1);
        assert_eq!(diag_live_1, diag_ref_1);
        let mut ref_bytes_1 = Vec::new();
        let _ = fresh_cx(|cx| Dxf::write(&dxf_ref, &mut ref_bytes_1, cx));
        assert_eq!(
            fresh_cx(|cx| live.save(cx)).0.ok().unwrap_or_default(),
            ref_bytes_1
        );

        // 3. HATCH・DIMASSOC から参照されている LINE (h101) の削除は forget_values を伴いバイト列一致し、undo/redo で辺も復元される
        let step2 = Changes {
            document: None,
            edits: vec![Edit::Remove {
                target: get_seen(&live, "space/0/h101"),
            }],
        };
        let (ap_live_2, diag_live_2) = fresh_cx(|cx| live.apply(&step2, cx));
        let (ap_ref_2, diag_ref_2) = fresh_cx(|cx| dxf_ref.apply(&step2, cx));
        assert!(ap_live_2.is_ok());
        assert_eq!(ap_live_2, ap_ref_2);
        assert_eq!(diag_live_2, diag_ref_2);
        let mut ref_bytes_2 = Vec::new();
        let _ = fresh_cx(|cx| Dxf::write(&dxf_ref, &mut ref_bytes_2, cx));
        assert_eq!(
            fresh_cx(|cx| live.save(cx)).0.ok().unwrap_or_default(),
            ref_bytes_2
        );

        assert_eq!(fresh_cx(|cx| live.undo(cx)).0, Ok(true));
        assert_eq!(
            fresh_cx(|cx| live.save(cx)).0.ok().unwrap_or_default(),
            ref_bytes_1
        );
        assert_eq!(fresh_cx(|cx| live.undo(cx)).0, Ok(true));
        assert_eq!(fresh_cx(|cx| live.save(cx)).0.ok().unwrap_or_default(), src);
        let (restored_affected, _) = fresh_cx(|cx| live.affected_rows(&[row_line], cx));
        let restored_affected = restored_affected.ok().unwrap_or_default();
        assert!(restored_affected.contains(&row_hatch));
        assert!(restored_affected.contains(&row_dim));

        assert_eq!(fresh_cx(|cx| live.redo(cx)).0, Ok(true));
        assert_eq!(fresh_cx(|cx| live.redo(cx)).0, Ok(true));
        assert_eq!(
            fresh_cx(|cx| live.save(cx)).0.ok().unwrap_or_default(),
            ref_bytes_2
        );
    }

    fn build_nested_block_dwg_from_sample() -> Option<Vec<u8>> {
        use crate::db::{Data, Object, Origin, Section, Value};

        let dwg_dir = samples_dir().join("dwg");
        let entries = std::fs::read_dir(&dwg_dir).ok()?;
        let mut paths: Vec<PathBuf> = entries
            .filter_map(|e| e.ok().map(|x| x.path()))
            .filter(|p| {
                p.extension()
                    .and_then(|s| s.to_str())
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("dwg"))
            })
            .collect();
        paths.sort();

        for path in paths {
            let Ok(src) = std::fs::read(&path) else {
                continue;
            };
            let (Ok(mut dwg), _) = fresh_cx(|cx| Dwg::read(&src, cx)) else {
                continue;
            };
            if dwg.version() != b"AC1015" {
                continue;
            }
            let (Ok(mut db), _) = fresh_cx(|cx| dwg.objects(cx)) else {
                continue;
            };
            let Some(mut h) = dwg.handle_seed() else {
                continue;
            };
            let br_control = db
                .objects()
                .iter()
                .find(|o| o.section == Section::Tables && o.kind == b"BLOCK_RECORD")
                .and_then(Object::owner);
            let model_br = db
                .objects()
                .iter()
                .find(|o| {
                    o.section == Section::Tables
                        && o.kind == b"BLOCK_RECORD"
                        && o.values.iter().any(|v| {
                            v.code == 2
                                && matches!(&v.data, Data::Text(t) if t.eq_ignore_ascii_case(b"*Model_Space"))
                        })
                })
                .and_then(Object::handle);
            let (Some(br_control), Some(model_br)) = (br_control, model_br) else {
                continue;
            };
            let val = |code: i16, data: Data| Value { code, data };
            let push_obj =
                |db: &mut crate::AcadDb, section: Section, kind: &[u8], values: Vec<Value>| {
                    db.push(Object {
                        section,
                        kind: kind.to_vec(),
                        origin: Origin::New,
                        values,
                    });
                };

            // Step A: INNER ブロック定義を追加して書き戻す
            let h_br_in = h;
            let h_blk_in = h + 1;
            let h_cir_in = h + 2;
            let h_end_in = h + 3;
            push_obj(
                &mut db,
                Section::Tables,
                b"BLOCK_RECORD",
                vec![
                    val(5, Data::Handle(h_br_in)),
                    val(330, Data::Handle(br_control)),
                    val(2, Data::Text(b"ZZ_INNER".to_vec())),
                ],
            );
            push_obj(
                &mut db,
                Section::Blocks,
                b"BLOCK",
                vec![
                    val(5, Data::Handle(h_blk_in)),
                    val(330, Data::Handle(h_br_in)),
                    val(8, Data::Text(b"0".to_vec())),
                    val(2, Data::Text(b"ZZ_INNER".to_vec())),
                    val(10, Data::Real(0.0)),
                    val(20, Data::Real(0.0)),
                    val(30, Data::Real(0.0)),
                ],
            );
            push_obj(
                &mut db,
                Section::Blocks,
                b"CIRCLE",
                vec![
                    val(5, Data::Handle(h_cir_in)),
                    val(330, Data::Handle(h_br_in)),
                    val(8, Data::Text(b"0".to_vec())),
                    val(10, Data::Real(0.0)),
                    val(20, Data::Real(0.0)),
                    val(30, Data::Real(0.0)),
                    val(40, Data::Real(5.0)),
                ],
            );
            push_obj(
                &mut db,
                Section::Blocks,
                b"ENDBLK",
                vec![
                    val(5, Data::Handle(h_end_in)),
                    val(330, Data::Handle(h_br_in)),
                    val(8, Data::Text(b"0".to_vec())),
                ],
            );
            if fresh_cx(|cx| dwg.write_back(&db, cx)).0.is_err() {
                continue;
            }

            // Step B: OUTER ブロック定義（ZZ_INNER を挿入）とモデル空間の INSERT（ZZ_OUTER を (6000,6000) に配置）を追加
            let (Ok(mut db2), _) = fresh_cx(|cx| dwg.objects(cx)) else {
                continue;
            };
            h = dwg.handle_seed().unwrap_or(h + 10);
            let h_br_out = h;
            let h_blk_out = h + 1;
            let h_ins_in = h + 2;
            let h_end_out = h + 3;
            let h_ins_model = h + 4;
            push_obj(
                &mut db2,
                Section::Tables,
                b"BLOCK_RECORD",
                vec![
                    val(5, Data::Handle(h_br_out)),
                    val(330, Data::Handle(br_control)),
                    val(2, Data::Text(b"ZZ_OUTER".to_vec())),
                ],
            );
            push_obj(
                &mut db2,
                Section::Blocks,
                b"BLOCK",
                vec![
                    val(5, Data::Handle(h_blk_out)),
                    val(330, Data::Handle(h_br_out)),
                    val(8, Data::Text(b"0".to_vec())),
                    val(2, Data::Text(b"ZZ_OUTER".to_vec())),
                    val(10, Data::Real(0.0)),
                    val(20, Data::Real(0.0)),
                    val(30, Data::Real(0.0)),
                ],
            );
            push_obj(
                &mut db2,
                Section::Blocks,
                b"INSERT",
                vec![
                    val(5, Data::Handle(h_ins_in)),
                    val(330, Data::Handle(h_br_out)),
                    val(8, Data::Text(b"0".to_vec())),
                    val(2, Data::Text(b"ZZ_INNER".to_vec())),
                    val(10, Data::Real(10.0)),
                    val(20, Data::Real(0.0)),
                    val(30, Data::Real(0.0)),
                ],
            );
            push_obj(
                &mut db2,
                Section::Blocks,
                b"ENDBLK",
                vec![
                    val(5, Data::Handle(h_end_out)),
                    val(330, Data::Handle(h_br_out)),
                    val(8, Data::Text(b"0".to_vec())),
                ],
            );
            push_obj(
                &mut db2,
                Section::Entities,
                b"INSERT",
                vec![
                    val(5, Data::Handle(h_ins_model)),
                    val(330, Data::Handle(model_br)),
                    val(8, Data::Text(b"0".to_vec())),
                    val(2, Data::Text(b"ZZ_OUTER".to_vec())),
                    val(10, Data::Real(6000.0)),
                    val(20, Data::Real(6000.0)),
                    val(30, Data::Real(0.0)),
                ],
            );
            if fresh_cx(|cx| dwg.write_back(&db2, cx)).0.is_err() {
                continue;
            }
            let mut out = Vec::new();
            if fresh_cx(|cx| Dwg::write(&dwg, &mut out, cx)).0.is_ok() {
                return Some(out);
            }
        }
        None
    }

    #[test]
    fn block_definition_element_edits_propagate_across_nested_blocks_in_dxf_and_dwg() {
        // 1. DXF での入れ子ブロック定義要素の高速編集と依存伝播
        let dxf_bytes = {
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
            d.symbols.push(zumen_drawing::SymbolDefinition {
                key: SymbolKey {
                    namespace: "dxf.block".into(),
                    name: "ZZ_INNER".to_owned(),
                },
                space: zumen_drawing::ModelSpace {
                    id: zumen_drawing::SpaceId(1),
                    unit: zumen_drawing::Unit::Millimeter,
                    axes: zumen_drawing::Axes::EastNorth,
                    elements: vec![Element {
                        source: Ref::default(),
                        layer: LayerId(0),
                        style: default_style(),
                        shape: Shape::Circle {
                            center: pt(0.0, 0.0),
                            radius: mlen(5.0),
                        },
                    }],
                    order: vec![0],
                },
                base: Some(pt(0.0, 0.0)),
                attributes: Vec::new(),
                anonymous: false,
                external: None,
                source: Ref::default(),
            });
            d.symbols.push(zumen_drawing::SymbolDefinition {
                key: SymbolKey {
                    namespace: "dxf.block".into(),
                    name: "ZZ_OUTER".to_owned(),
                },
                space: zumen_drawing::ModelSpace {
                    id: zumen_drawing::SpaceId(2),
                    unit: zumen_drawing::Unit::Millimeter,
                    axes: zumen_drawing::Axes::EastNorth,
                    elements: vec![Element {
                        source: Ref::default(),
                        layer: LayerId(0),
                        style: default_style(),
                        shape: Shape::Symbol(Symbol {
                            key: SymbolKey {
                                namespace: "dxf.block".into(),
                                name: "ZZ_INNER".to_owned(),
                            },
                            position: pt(10.0, 0.0),
                            scale: [1.0, 1.0],
                            rotation: 0.0,
                            grid: None,
                            attributes: Vec::new(),
                            clip: None,
                        }),
                    }],
                    order: vec![0],
                },
                base: Some(pt(0.0, 0.0)),
                attributes: Vec::new(),
                anonymous: false,
                external: None,
                source: Ref::default(),
            });
            d.spaces.push(zumen_drawing::ModelSpace {
                id: MODEL,
                unit: zumen_drawing::Unit::Millimeter,
                axes: zumen_drawing::Axes::EastNorth,
                elements: vec![Element {
                    source: Ref::default(),
                    layer: LayerId(0),
                    style: default_style(),
                    shape: Shape::Symbol(Symbol {
                        key: SymbolKey {
                            namespace: "dxf.block".into(),
                            name: "ZZ_OUTER".to_owned(),
                        },
                        position: pt(6000.0, 6000.0),
                        scale: [1.0, 1.0],
                        rotation: 0.0,
                        grid: None,
                        attributes: Vec::new(),
                        clip: None,
                    }),
                }],
                order: vec![0],
            });
            fresh_cx(|cx| crate::lower_to_dxf(&d, cx))
                .0
                .map(|l| l.bytes)
                .unwrap_or_default()
        };

        let find_inner_circle_seen = |drawing: &zumen_drawing::Drawing2D| -> Option<Seen> {
            let mut def_namer = id::DefinitionNamer::default();
            for sym in &drawing.symbols {
                let place = def_namer.next(sym);
                if sym.key.name == "ZZ_INNER" {
                    let mut namer = id::Namer::new(&place);
                    let first = sym.space.elements.first()?;
                    let cid = namer.next(0, first);
                    return Some(Seen {
                        id: cid,
                        hash: Hash::element(first),
                    });
                }
            }
            None
        };

        {
            let (Ok(mut dxf_ref), _) = fresh_cx(|cx| Dxf::read(&dxf_bytes, cx)) else {
                return;
            };
            let (Ok(mut live), _) = fresh_cx(|cx| AcadLive::open_dxf(&dxf_bytes, cx)) else {
                return;
            };
            let (Ok(d0), _) = fresh_cx(|cx| live.drawing(cx)) else {
                return;
            };
            let Some(seen_circle) = find_inner_circle_seen(&d0) else {
                return;
            };

            let mut old_hits = Vec::new();
            live.select_rect([6008.0, 5998.0, 6012.0, 6002.0], &mut old_hits);
            assert_eq!(old_hits.len(), 1);
            let outer_row = old_hits.first().copied().unwrap_or(zumen_live::RowId::NONE);

            let inner_vnode = AcadLive::symbol_dep_node(0).unwrap_or(zumen_live::RowId::NONE);
            let (affected, _) = fresh_cx(|cx| live.affected_rows(&[inner_vnode], cx));
            assert!(affected.ok().unwrap_or_default().contains(&outer_row));

            live.clear_dirty_symbols();
            let step = Changes {
                document: None,
                edits: vec![Edit::Transform {
                    target: seen_circle,
                    transform: Affine::translation(500.0, 400.0),
                }],
            };
            let (ap_live, diag_live) = fresh_cx(|cx| live.apply(&step, cx));
            let (ap_ref, diag_ref) = fresh_cx(|cx| dxf_ref.apply(&step, cx));
            assert!(ap_live.is_ok());
            assert_eq!(ap_live, ap_ref);
            assert_eq!(diag_live, diag_ref);
            // ZZ_INNER (0) の編集により入れ子先の ZZ_OUTER (1) まで再送対象になる
            assert_eq!(
                live.dirty_symbols(),
                &std::collections::BTreeSet::from([0, 1])
            );

            // 空間索引がモデル空間の ZZ_OUTER 挿入位置 (6510, 6400) へ追従していることを検証
            let mut hits_old_after = Vec::new();
            live.select_rect([6008.0, 5998.0, 6012.0, 6002.0], &mut hits_old_after);
            assert!(!hits_old_after.contains(&outer_row));
            let mut hits_new_after = Vec::new();
            live.select_rect([6508.0, 6398.0, 6512.0, 6402.0], &mut hits_new_after);
            assert!(hits_new_after.contains(&outer_row));

            let mut ref_saved = Vec::new();
            let _ = fresh_cx(|cx| Dxf::write(&dxf_ref, &mut ref_saved, cx));
            assert_eq!(
                fresh_cx(|cx| live.save(cx)).0.ok().unwrap_or_default(),
                ref_saved
            );
            assert_eq!(
                fresh_cx(|cx| live.drawing(cx)).0,
                fresh_cx(|cx| dxf_ref.lift(cx)).0
            );

            // Undo / Redo で空間索引・dirty_symbols・Drawing2D・バイト列が完全対称に戻る
            live.clear_dirty_symbols();
            assert_eq!(fresh_cx(|cx| live.undo(cx)).0, Ok(true));
            assert_eq!(
                live.dirty_symbols(),
                &std::collections::BTreeSet::from([0, 1])
            );
            let mut hits_undo = Vec::new();
            live.select_rect([6008.0, 5998.0, 6012.0, 6002.0], &mut hits_undo);
            assert!(hits_undo.contains(&outer_row));
            assert_eq!(
                fresh_cx(|cx| live.save(cx)).0.ok().unwrap_or_default(),
                dxf_bytes
            );

            live.clear_dirty_symbols();
            assert_eq!(fresh_cx(|cx| live.redo(cx)).0, Ok(true));
            assert_eq!(
                live.dirty_symbols(),
                &std::collections::BTreeSet::from([0, 1])
            );
            let mut hits_redo = Vec::new();
            live.select_rect([6508.0, 6398.0, 6512.0, 6402.0], &mut hits_redo);
            assert!(hits_redo.contains(&outer_row));
            assert_eq!(
                fresh_cx(|cx| live.save(cx)).0.ok().unwrap_or_default(),
                ref_saved
            );
        }

        // 2. DWG (AC1015) での入れ子ブロック定義要素の高速編集・全体保存バイト一致・追記保存一致
        if let Some(dwg_bytes) = build_nested_block_dwg_from_sample() {
            let (Ok(mut dwg_ref), _) = fresh_cx(|cx| Dwg::read(&dwg_bytes, cx)) else {
                return;
            };
            let (Ok(mut live), _) = fresh_cx(|cx| AcadLive::open_dwg(&dwg_bytes, cx)) else {
                return;
            };
            let (Ok(d0), _) = fresh_cx(|cx| live.drawing(cx)) else {
                return;
            };
            let Some(seen_circle) = find_inner_circle_seen(&d0) else {
                return;
            };

            let mut old_hits = Vec::new();
            live.select_rect([6008.0, 5998.0, 6012.0, 6002.0], &mut old_hits);
            assert!(!old_hits.is_empty());

            live.clear_dirty_symbols();
            let step = Changes {
                document: None,
                edits: vec![Edit::Transform {
                    target: seen_circle,
                    transform: Affine::translation(500.0, 400.0),
                }],
            };
            let (ap_live, diag_live) = fresh_cx(|cx| live.apply(&step, cx));
            let (ap_ref, diag_ref) = fresh_cx(|cx| dwg_ref.apply(&step, cx));
            assert!(ap_live.is_ok());
            assert_eq!(ap_live, ap_ref);
            assert_eq!(diag_live, diag_ref);
            assert!(live.dirty_symbols().len() >= 2);

            let mut hits_new = Vec::new();
            live.select_rect([6508.0, 6398.0, 6512.0, 6402.0], &mut hits_new);
            assert!(!hits_new.is_empty());

            let mut expected_dwg = Vec::new();
            let _ = fresh_cx(|cx| Dwg::write(&dwg_ref, &mut expected_dwg, cx));
            let saved_full = fresh_cx(|cx| live.save(cx)).0.ok().unwrap_or_default();
            assert_eq!(saved_full, expected_dwg);

            let saved_compact = fresh_cx(|cx| live.save_compact(cx))
                .0
                .ok()
                .unwrap_or_default();
            let (reloaded_full, _) = fresh_cx(|cx| Dwg::read(&saved_full, cx)?.lift(cx));
            let (reloaded_compact, _) = fresh_cx(|cx| Dwg::read(&saved_compact, cx)?.lift(cx));
            assert_eq!(
                reloaded_compact.as_ref().map(drawing_without_byte_ranges),
                reloaded_full.as_ref().map(drawing_without_byte_ranges)
            );

            assert_eq!(fresh_cx(|cx| live.undo(cx)).0, Ok(true));
            assert_eq!(
                fresh_cx(|cx| live.save(cx)).0.ok().unwrap_or_default(),
                dwg_bytes
            );
            assert_eq!(fresh_cx(|cx| live.redo(cx)).0, Ok(true));
            assert_eq!(
                fresh_cx(|cx| live.save(cx)).0.ok().unwrap_or_default(),
                expected_dwg
            );
        }
    }
}
