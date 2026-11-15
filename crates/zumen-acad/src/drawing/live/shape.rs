//! 常駐する図形の列・画層と線種の番号表・囲む箱・表示バッファーへの書き出し（`docs/live.md`、#878）。
//!
//! モデル空間の大半を占める 6 種類（`LINE`・`CIRCLE`・`ARC`・`LWPOLYLINE`・`TEXT`・`INSERT`）のうち、
//! 厚さ（39）・拡張データ（1000 以上）・リアクターや拡張辞書（102・360）を持たない平面の図形は、
//! `Element<ModelPoint>` や `Vec<Value>` を作らずに [`Paged`] 列へ直接置く。
//! 残りの 17 種類（寸法・ハッチング・MTEXT・スプライン・楕円・引出線・表など）と拡張情報つきの図形は
//! [`GeneralRow`] に持ち、同じ [`VersionedTable`]・[`zumen_live::SpatialGrid`]・[`zumen_live::Display`] の行番号で統合する。

use std::collections::BTreeMap;

use zumen_base::{Key, Ref};
use zumen_drawing::{
    Attribute, CalloutContent, Clip, Color, ColorCode, ColorSystem, Draft, Edge, Element, Fit,
    FormattedText, Grid, Hatch, Height, HeightMeaning, Horizontal, Inherit, Layer, LayerId, Leader,
    Length, LineType, LineTypeId, LineWeight, ModelLength, ModelPoint, PaperLength, Placement,
    Polyline, Shape, Style, Symbol, SymbolDefinition, SymbolKey, Table, Text, Transparency,
    Underlay, Vertex, Vertical,
};
use zumen_live::display::{DisplayWriter, radians_from_deg};
use zumen_live::{BytePool, Paged, TableRow, TextSpan};

use super::super::single::{
    self, EntityStyle, FieldSource, LiftFail, PolylineSink, PolylineStream, RawLineType, RawText,
    RawTextPlace,
};
use super::super::{BLOCK, MODEL};
use crate::charset::Charset;
use crate::db::Origin;

/// 行が有効なハンドル（グループ・コード 5、値が `0` の場合も含む）を持つことを表すビット
pub const ROW_HAS_HANDLE: u8 = 1 << 0;
/// 行のハンドルがモデル空間の中で一意であることを表すビット
pub const ROW_UNIQUE_HANDLE: u8 = 1 << 1;
/// 行が `102` または `360`（リアクター・拡張辞書など）を持つことを表すビット
pub const ROW_HAS_102_OR_360: u8 = 1 << 2;
/// 開いた後に追加されたレコードであることを表すビット
pub const ROW_BORN: u8 = 1 << 3;
/// DWG の出所（バイト範囲）を持つことを表すビット
pub const ROW_DWG_ORIGIN: u8 = 1 << 4;

/// 対応するブロック定義がないときの番兵
pub const NO_DEFINITION: u32 = u32::MAX;

const DEFAULT_DISPLAY_TOLERANCE: f64 = 0.5;
const MAX_SYMBOL_DEPTH: u8 = 8;

/// 常駐するモデル空間の行の種類
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum LiveKind {
    /// 線分（`LINE`）
    Line = 0,
    /// 円（`CIRCLE`）
    Circle = 1,
    /// 円弧（`ARC`）
    Arc = 2,
    /// 軽量ポリライン（`LWPOLYLINE`）
    Polyline = 3,
    /// 1 行文字（`TEXT`）
    Text = 4,
    /// ブロック挿入（`INSERT`）
    Insert = 5,
    /// そのほかの種類または拡張情報つきの図形
    General = 6,
}

/// [`VersionedTable`](zumen_live::VersionedTable) に置くモデル空間の 1 行
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LiveRow {
    /// 元のレコード番号（追加レコードは元の件数からの連番）
    pub record: u32,
    /// 種類ごとの列（[`LiveShapes`]）の中の添字
    pub shape: u32,
    /// [`SlotCatalog`] の画層スロット番号
    pub layer_slot: u32,
    /// 表示用の 32 ビット色（上位 8 ビットが種別）
    pub color: u32,
    /// 既定でない共通スタイル（[`Pools::extras`] の添字、`u32::MAX` なら既定）
    pub extra: u32,
    /// ハンドル（[`ROW_HAS_HANDLE`] が立っているときのみ有効）
    pub handle: u64,
    /// DWG の出所の開始バイト（DXF では `0`）
    pub origin_start: u32,
    /// DWG の出所の終了バイト（DXF では `0`）
    pub origin_end: u32,
    /// 図形の格納種類
    pub kind: LiveKind,
    /// 行の状態ビット（`ROW_*`）
    pub flags: u8,
}

impl TableRow for LiveRow {
    const INLINE_BYTES: usize = {
        assert!(core::mem::size_of::<Self>() <= 64);
        64
    };

    #[inline]
    fn row_handle(&self) -> Option<u64> {
        ((self.flags & (ROW_HAS_HANDLE | ROW_UNIQUE_HANDLE))
            == (ROW_HAS_HANDLE | ROW_UNIQUE_HANDLE))
            .then_some(self.handle)
    }
}

impl LiveRow {
    /// ハンドルがあれば `Some(handle)` を返す（重複ハンドルや `0` も含む）
    #[inline]
    pub const fn raw_handle(&self) -> Option<u64> {
        if self.flags & ROW_HAS_HANDLE != 0 {
            Some(self.handle)
        } else {
            None
        }
    }

    /// 図面モデルの出所 [`Ref`] を組み立てる
    #[inline]
    pub fn source_ref(&self) -> Ref {
        let base = if self.flags & ROW_BORN != 0 {
            Ref::default()
        } else if self.flags & ROW_DWG_ORIGIN != 0 {
            Ref::at_bytes(u64::from(self.origin_start), u64::from(self.origin_end))
        } else {
            Ref::at_record(u64::from(self.record))
        };
        match self.raw_handle() {
            Some(h) => base.with_key(Key::Handle(h)),
            None => base,
        }
    }
}

/// 線種スロットの参照方法
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineTypeSlotRef {
    /// 画層に従う
    Layer,
    /// ブロックに従う
    Block,
    /// 個別の線種スロットを指す
    Own(u32),
}

/// 既定でない共通スタイル属性
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ExtraStyle {
    /// 色
    pub color: Inherit<Color>,
    /// 線種
    pub line_type: LineTypeSlotRef,
    /// 線種の尺度
    pub line_type_scale: f64,
    /// 線の太さ
    pub line_weight: Inherit<LineWeight>,
    /// 透過性
    pub transparency: Inherit<Transparency>,
}

impl ExtraStyle {
    /// すべて `ByLayer`・尺度 `1.0` の既定スタイルか
    pub fn is_default_layer(&self) -> bool {
        self.color == Inherit::Layer
            && self.line_type == LineTypeSlotRef::Layer
            && self.line_type_scale == 1.0
            && self.line_weight == Inherit::Layer
            && self.transparency == Inherit::Layer
    }

    /// 線種スロットを正規の [`LineTypeId`] に写して [`Style`] に変換する
    pub fn to_style(self, catalog: &SlotCatalog) -> Style {
        let line_type = match self.line_type {
            LineTypeSlotRef::Layer => Inherit::Layer,
            LineTypeSlotRef::Block => Inherit::Block,
            LineTypeSlotRef::Own(slot) => Inherit::Own(catalog.line_type_id(slot)),
        };
        Style {
            color: self.color,
            line_type,
            line_type_scale: self.line_type_scale,
            line_weight: Some(self.line_weight),
            transparency: self.transparency,
        }
    }
}

/// 表示バッファーに載せる 32 ビット色の符号化
pub fn pack_color(color: Inherit<Color>) -> u32 {
    match color {
        Inherit::Layer => 0,
        Inherit::Block => 1 << 24,
        Inherit::Own(Color {
            rgb: Some([r, g, b]),
            ..
        }) => (3 << 24) | (u32::from(r) << 16) | (u32::from(g) << 8) | u32::from(b),
        Inherit::Own(Color {
            code:
                Some(ColorCode {
                    system: ColorSystem::Aci,
                    value,
                }),
            rgb: None,
        }) => (2 << 24) | (value & 0xFF),
        Inherit::Own(_) => 0,
    }
}

/// 表示用の 32 ビット色から [`Inherit<Color>`] を戻す
pub fn unpack_color(bits: u32) -> Inherit<Color> {
    match bits >> 24 {
        1 => Inherit::Block,
        2 => Inherit::Own(Color {
            code: Some(ColorCode {
                system: ColorSystem::Aci,
                value: bits & 0xFF,
            }),
            rgb: None,
        }),
        3 => Inherit::Own(Color {
            code: None,
            rgb: Some([
                ((bits >> 16) & 0xFF) as u8,
                ((bits >> 8) & 0xFF) as u8,
                (bits & 0xFF) as u8,
            ]),
        }),
        _ => Inherit::Layer,
    }
}

/// `LINE` の幾何
#[derive(Clone, Copy, Debug)]
pub struct LineGeom {
    /// `[x0, y0, x1, y1]`
    pub ends: [f64; 4],
}

/// `CIRCLE` の幾何
#[derive(Clone, Copy, Debug)]
pub struct CircleGeom {
    /// 中心座標
    pub center: [f64; 2],
    /// 半径
    pub radius: f64,
}

/// `ARC` の幾何（度、反時計回り）
#[derive(Clone, Copy, Debug)]
pub struct ArcGeom {
    /// 中心座標
    pub center: [f64; 2],
    /// 半径
    pub radius: f64,
    /// 始角（度）
    pub start: f64,
    /// 終角（度）
    pub end: f64,
}

/// `LWPOLYLINE` の幾何（頂点座標・膨らみ・幅は [`Pools`] 側）
#[derive(Clone, Copy, Debug)]
pub struct PolyGeom {
    /// `Pools::xy` の開始添字
    pub xy_start: u32,
    /// 頂点数
    pub count: u32,
    /// `Pools::bulges` の開始添字（膨らみが全 0 なら `u32::MAX`）
    pub bulge_start: u32,
    /// `Pools::widths` の開始添字（頂点ごとの幅が全 0 なら `u32::MAX`）
    pub width_start: u32,
    /// 一定幅（なければ `NaN`）
    pub constant_width: f64,
    /// 閉じているか（ビット 0）、線種生成を行うか（ビット 1）
    pub flags: u8,
}

/// `TEXT` / `ATTRIB` の幾何
#[derive(Clone, Copy, Debug)]
pub struct TextGeom {
    /// 基点（`Between` のときは始点）
    pub point: [f64; 2],
    /// 終点（`Between` のみ使用）
    pub second: [f64; 2],
    /// 文字高さ
    pub height: f64,
    /// 回転（度）
    pub rotation: f64,
    /// 幅の係数
    pub width_factor: f64,
    /// 傾斜角（度）
    pub oblique: f64,
    /// UTF-8 文字列の `Pools::bytes` 範囲
    pub content: TextSpan,
    /// 文字スタイル名の `Pools::bytes` 範囲
    pub style: TextSpan,
    /// 横揃え（`0..=3`、または `4 = Aligned`・`5 = Fit`）
    pub horizontal: u8,
    /// 縦揃え（`0..=3`）
    pub vertical: u8,
    /// 反転ビット（`1 = backward`、`2 = upside_down`）
    pub flags: u8,
}

/// `INSERT` に付く 1 つの `ATTRIB`
#[derive(Clone, Copy, Debug)]
pub struct AttribRow {
    /// レコード番号
    pub record: u32,
    /// DWG 出所の開始バイト（DXF では `0`）
    pub origin_start: u32,
    /// DWG 出所の終了バイト（DXF では `0`）
    pub origin_end: u32,
    /// ハンドル（`has_handle` が真のときのみ有効）
    pub handle: u64,
    /// 画層スロット
    pub layer_slot: u32,
    /// 表示用 32 ビット色
    pub color: u32,
    /// 共通スタイルの追加添字（`u32::MAX` なら既定）
    pub extra: u32,
    /// タグ名の `Pools::bytes` 範囲
    pub tag: TextSpan,
    /// 文字の幾何
    pub text: TextGeom,
    /// 状態ビット（ビット 0: `hidden`、ビット 1: `has_handle`、ビット 2: `dwg_origin`、ビット 3: `born`）
    pub attrib_flags: u8,
}

impl AttribRow {
    /// 出所 [`Ref`] を組み立てる
    pub fn source_ref(&self) -> Ref {
        let base = if self.attrib_flags & (1 << 3) != 0 {
            Ref::default()
        } else if self.attrib_flags & (1 << 2) != 0 {
            Ref::at_bytes(u64::from(self.origin_start), u64::from(self.origin_end))
        } else {
            Ref::at_record(u64::from(self.record))
        };
        if self.attrib_flags & (1 << 1) != 0 {
            base.with_key(Key::Handle(self.handle))
        } else {
            base
        }
    }
}

/// `INSERT` の幾何
#[derive(Clone, Copy, Debug)]
pub struct InsertGeom {
    /// 挿入基点
    pub position: [f64; 2],
    /// X・Y 尺度
    pub scale: [f64; 2],
    /// 回転角（度）
    pub rotation: f64,
    /// 配列の列数・行数
    pub grid: [u32; 2],
    /// 配列の列間隔・行間隔
    pub spacing: [f64; 2],
    /// ブロック名の `Pools::bytes` 範囲
    pub name: TextSpan,
    /// 対応する `SymbolDefinition` の添字（なければ [`NO_DEFINITION`]）
    pub definition: u32,
    /// `Pools::attribs` の開始添字
    pub attrib_start: u32,
    /// 持ち上がった `ATTRIB` の件数
    pub attrib_count: u32,
    /// 元のファイルで親に付いていた `ATTRIB` の総数
    pub raw_attrib_count: u32,
    /// 切り抜き境界（`Pools::clips` の添字、なければ `u32::MAX`）
    pub clip_index: u32,
}

/// 一般行（6 種類以外の図形または拡張情報つきの図形）
#[derive(Clone, Debug)]
pub struct GeneralRow {
    /// 図面モデルの要素（`layer` と `style.line_type` は [`SlotCatalog`] のスロット番号を仮置きして持つ）
    pub element: Element<ModelPoint>,
    /// 属性ごとの `(画層スロット, 線種スロット参照)`（`Shape::Symbol` のときのみ使用）
    pub attrib_slots: Vec<(u32, LineTypeSlotRef)>,
    /// 引出線つき注記の線の線種スロット参照（`Shape::Callout` のときのみ使用）
    pub callout_line_type_slot: LineTypeSlotRef,
    /// DXF の図形種類名（`DIMENSION`・`HATCH` など）
    pub dxf_kind: Box<[u8]>,
    /// 親に続く子レコード番号（`POLYLINE` の `VERTEX`・`SEQEND` や `INSERT` の `ATTRIB`・`SEQEND`）
    pub follower_records: Box<[u32]>,
    /// 親に付いていた生の `ATTRIB` の件数
    pub raw_attrib_count: u32,
    /// この要素に付く拡張情報のひな形（`Position` は `drawing()` で確定させる）
    pub drafts: Vec<Draft>,
}

/// ページ分割された可変長の補助列
#[derive(Clone, Default)]
pub struct Pools {
    /// ポリライン頂点の座標列
    pub xy: Paged<[f64; 2]>,
    /// ポリライン頂点の膨らみ列
    pub bulges: Paged<f64>,
    /// ポリライン頂点の始端・終端幅の列
    pub widths: Paged<[f64; 2]>,
    /// UTF-8 文字列プール
    pub bytes: BytePool,
    /// `INSERT` に付く `ATTRIB` 列
    pub attribs: Paged<AttribRow>,
    /// `INSERT` の切り抜き境界プール
    pub clips: Vec<Clip>,
    /// 既定でない共通スタイル列
    pub extras: Paged<ExtraStyle>,
}

impl Pools {
    /// UTF-8 文字列をプールに積み、[`TextSpan`] を返す
    pub fn push_str(&mut self, text: &str) -> TextSpan {
        self.bytes.push(text.as_bytes()).unwrap_or(TextSpan::EMPTY)
    }

    /// [`TextSpan`] から所有文字列を復元する
    pub fn get_string(&self, span: TextSpan) -> String {
        self.bytes.str_of(span).to_owned()
    }

    /// 共通スタイルを登録し、既定なら `u32::MAX`、それ以外なら添字を返す
    pub fn intern_extra(&mut self, extra: ExtraStyle) -> u32 {
        if extra.is_default_layer() {
            u32::MAX
        } else {
            let idx = self.extras.len() as u32;
            self.extras.push(extra);
            idx
        }
    }
}

/// モデル空間の全図形の格納列
#[derive(Clone, Default)]
pub struct LiveShapes {
    /// 線分列
    pub lines: Paged<LineGeom>,
    /// 円列
    pub circles: Paged<CircleGeom>,
    /// 円弧列
    pub arcs: Paged<ArcGeom>,
    /// 軽量ポリライン列
    pub polylines: Paged<PolyGeom>,
    /// 1 行文字列
    pub texts: Paged<TextGeom>,
    /// ブロック挿入列
    pub inserts: Paged<InsertGeom>,
    /// 一般図形列
    pub generals: Vec<GeneralRow>,
    /// 可変長の補助プール
    pub pools: Pools,
}

/// 画層スロットの 1 項目
#[derive(Clone, Debug)]
pub struct LayerSlotEntry {
    /// 画層の表示名
    pub name: String,
    /// `TABLES` に定義があればその定義（`id` は出力時に正規番号へ振り直す）
    pub defined: Option<Layer>,
    /// 画層レコードに付く拡張辞書のひな形
    pub drafts: Vec<Draft>,
    /// ペーパー空間またはブロック定義が参照しているか（真ならモデル要素が消えても残る）
    pub pinned_outside_model: bool,
    /// 生きているモデル空間要素（属性を含む）からの参照件数
    pub model_refs: u32,
}

/// 線種スロットの 1 項目
#[derive(Clone, Debug)]
pub struct LineTypeSlotEntry {
    /// 線種の表示名
    pub name: String,
    /// `TABLES` に定義があればその定義
    pub defined: Option<LineType>,
    /// 線種レコードに付く拡張辞書のひな形
    pub drafts: Vec<Draft>,
    /// ペーパー空間またはブロック定義が参照しているか
    pub pinned_outside_model: bool,
    /// 生きているモデル空間要素からの参照件数
    pub model_refs: u32,
}

/// 画層・線種の内部スロットと正規の [`LayerId`]・[`LineTypeId`] の対応表
#[derive(Clone, Debug, Default)]
pub struct SlotCatalog {
    /// 画層スロット一覧
    pub layers: Vec<LayerSlotEntry>,
    /// 小文字の画層名から画層スロット番号を引く表
    pub by_layer_lower: BTreeMap<String, u32>,
    /// 線種スロット一覧
    pub line_types: Vec<LineTypeSlotEntry>,
    /// 小文字の線種名から線種スロット番号を引く表
    pub by_line_type_lower: BTreeMap<String, u32>,
    /// 画層スロットから正規 [`LayerId`] への写像
    pub layer_remap: Vec<LayerId>,
    /// 線種スロットから正規 [`LineTypeId`] への写像
    pub line_type_remap: Vec<LineTypeId>,
    /// 正規順に並べた生きている画層スロット番号
    pub active_layer_slots: Vec<u32>,
    /// 正規順に並べた生きている線種スロット番号
    pub active_line_type_slots: Vec<u32>,
    /// 暗黙の（`TABLES` に定義がない）スロットを含むか
    pub has_implicit: bool,
}

impl SlotCatalog {
    /// `TABLES` 持ち上げ直後の画層・線種一覧から初期カタログを組む（同名レコードの重複もそのまま保つ）
    pub fn from_tables(layers: Vec<Layer>, line_types: Vec<LineType>) -> Self {
        let mut cat = Self::default();
        for lt in line_types {
            let slot = cat.line_types.len() as u32;
            cat.by_line_type_lower
                .entry(lt.name.to_ascii_lowercase())
                .or_insert(slot);
            let is_defined = lt.definition.is_some() || lt.source != Ref::default();
            cat.line_types.push(LineTypeSlotEntry {
                name: lt.name.clone(),
                defined: is_defined.then_some(lt),
                drafts: Vec::new(),
                pinned_outside_model: true,
                model_refs: 0,
            });
            cat.line_type_remap.push(LineTypeId(slot));
            cat.active_line_type_slots.push(slot);
        }
        for layer in layers {
            let slot = cat.layers.len() as u32;
            cat.by_layer_lower
                .entry(layer.name.to_ascii_lowercase())
                .or_insert(slot);
            cat.layers.push(LayerSlotEntry {
                name: layer.name.clone(),
                defined: Some(layer),
                drafts: Vec::new(),
                pinned_outside_model: true,
                model_refs: 0,
            });
            cat.layer_remap.push(LayerId(slot));
            cat.active_layer_slots.push(slot);
        }
        cat
    }

    /// 画層名からスロットを引き、なければ暗黙の画層スロットとして追加する
    pub fn intern_layer(&mut self, name: &str) -> u32 {
        let lower = name.to_ascii_lowercase();
        if let Some(&slot) = self.by_layer_lower.get(&lower) {
            return slot;
        }
        let slot = self.layers.len() as u32;
        self.by_layer_lower.insert(lower, slot);
        self.layers.push(LayerSlotEntry {
            name: name.to_owned(),
            defined: None,
            drafts: Vec::new(),
            pinned_outside_model: false,
            model_refs: 0,
        });
        self.layer_remap.push(LayerId(slot));
        self.active_layer_slots.push(slot);
        self.has_implicit = true;
        slot
    }

    /// 線種名からスロットを引き、なければ暗黙の線種スロットとして追加する
    pub fn intern_line_type(&mut self, name: &str) -> u32 {
        let lower = name.to_ascii_lowercase();
        if let Some(&slot) = self.by_line_type_lower.get(&lower) {
            return slot;
        }
        let slot = self.line_types.len() as u32;
        self.by_line_type_lower.insert(lower, slot);
        self.line_types.push(LineTypeSlotEntry {
            name: name.to_owned(),
            defined: None,
            drafts: Vec::new(),
            pinned_outside_model: false,
            model_refs: 0,
        });
        self.line_type_remap.push(LineTypeId(slot));
        self.active_line_type_slots.push(slot);
        self.has_implicit = true;
        slot
    }

    /// 画層スロットから正規の [`LayerId`] を返す
    #[inline]
    pub fn layer_id(&self, slot: u32) -> LayerId {
        self.layer_remap
            .get(slot as usize)
            .copied()
            .unwrap_or(LayerId(slot))
    }

    /// 正規の [`LayerId`] から画層スロット番号を返す
    #[inline]
    pub fn slot_of_layer_id(&self, id: LayerId) -> Option<u32> {
        self.active_layer_slots.get(id.0 as usize).copied()
    }

    /// 線種スロットから正規の [`LineTypeId`] を返す
    #[inline]
    pub fn line_type_id(&self, slot: u32) -> LineTypeId {
        self.line_type_remap
            .get(slot as usize)
            .copied()
            .unwrap_or(LineTypeId(slot))
    }

    /// 正規の [`LineTypeId`] から線種スロット番号を返す
    #[inline]
    pub fn slot_of_line_type_id(&self, id: LineTypeId) -> Option<u32> {
        self.active_line_type_slots.get(id.0 as usize).copied()
    }

    /// 画層スロットの表示名を返す
    #[inline]
    pub fn layer_name(&self, slot: u32) -> &str {
        self.layers
            .get(slot as usize)
            .map_or("0", |entry| entry.name.as_str())
    }

    /// 線種スロットの表示名を返す
    #[inline]
    pub fn line_type_name(&self, slot: u32) -> Option<&str> {
        self.line_types
            .get(slot as usize)
            .map(|entry| entry.name.as_str())
    }

    /// 正規の [`Layer`] 一覧を組み立てる
    pub fn build_layers(&self) -> Vec<Layer> {
        let mut out = Vec::with_capacity(self.active_layer_slots.len());
        for (idx, &slot) in self.active_layer_slots.iter().enumerate() {
            let id = LayerId(idx as u32);
            let Some(entry) = self.layers.get(slot as usize) else {
                continue;
            };
            if let Some(mut layer) = entry.defined.clone() {
                if let Some(style) = &mut layer.style
                    && let Some(lt_slot_id) = style.line_type
                {
                    style.line_type = Some(self.line_type_id(lt_slot_id.0));
                }
                layer.id = id;
                out.push(layer);
            } else {
                out.push(Layer {
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
        out
    }

    /// 正規の [`LineType`] 一覧を組み立てる
    pub fn build_line_types(&self) -> Vec<LineType> {
        let mut out = Vec::with_capacity(self.active_line_type_slots.len());
        for (idx, &slot) in self.active_line_type_slots.iter().enumerate() {
            let id = LineTypeId(idx as u32);
            let Some(entry) = self.line_types.get(slot as usize) else {
                continue;
            };
            if let Some(mut lt) = entry.defined.clone() {
                lt.id = id;
                out.push(lt);
            } else {
                out.push(LineType {
                    id,
                    name: entry.name.clone(),
                    definition: None,
                    source: Ref::default(),
                });
            }
        }
        out
    }
}

/// 6 種類の図形の持ち上げ結果（まだ共有プールに積む前の値）
#[derive(Clone, Debug)]
pub enum PlainGeom {
    /// 線分
    Line([f64; 4]),
    /// 円
    Circle([f64; 3]),
    /// 円弧
    Arc([f64; 5]),
    /// 軽量ポリライン
    Polyline {
        /// 頂点座標列
        xy: Vec<[f64; 2]>,
        /// 膨らみ列（すべて 0 なら空）
        bulges: Vec<f64>,
        /// 始端・終端幅列（すべて 0 なら空）
        widths: Vec<[f64; 2]>,
        /// 一定幅
        constant_width: Option<f64>,
        /// 閉じているか
        closed: bool,
        /// 線種生成フラグ
        line_type_generation: bool,
    },
    /// 1 行文字
    Text(PlainText),
    /// ブロック挿入
    Insert {
        /// ブロック名
        name: String,
        /// 基点
        position: [f64; 2],
        /// 尺度
        scale: [f64; 2],
        /// 回転（度）
        rotation: f64,
        /// 配列の列・行数
        grid: [u32; 2],
        /// 配列の間隔
        spacing: [f64; 2],
        /// 66 != 0 か
        followed: bool,
        /// 持ち上げた `ATTRIB` 列
        attribs: Vec<PlainAttrib>,
        /// 元のレコード上の `ATTRIB` 件数
        raw_attrib_count: u32,
        /// 切り抜き境界
        clip: Option<Clip>,
    },
}

/// 持ち上げた 1 行文字の値
#[derive(Clone, Debug)]
pub struct PlainText {
    /// 基点
    pub point: [f64; 2],
    /// 終点（`Between` のみ使用）
    pub second: [f64; 2],
    /// 高さ
    pub height: f64,
    /// 回転（度）
    pub rotation: f64,
    /// 幅の係数
    pub width_factor: f64,
    /// 傾斜角（度）
    pub oblique: f64,
    /// 内容（UTF-8）
    pub content: String,
    /// 文字スタイル名
    pub style: String,
    /// 横揃え符号
    pub horizontal: u8,
    /// 縦揃え符号
    pub vertical: u8,
    /// 反転ビット
    pub flags: u8,
}

/// 持ち上げた共通スタイル（名前は UTF-8 復号済み）
#[derive(Clone, Debug)]
pub struct PlainStyle {
    /// 画層名
    pub layer: String,
    /// 色
    pub color: Inherit<Color>,
    /// 線種（`None` = `ByLayer`、`Some("")` は使わず個別名または `ByBlock` を区別）
    pub line_type: PlainLineType,
    /// 線種の尺度
    pub line_type_scale: f64,
    /// 線の太さ
    pub line_weight: Inherit<LineWeight>,
    /// 透過性
    pub transparency: Inherit<Transparency>,
}

/// 復号済みの線種指定
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlainLineType {
    /// 画層に従う
    Layer,
    /// ブロックに従う
    Block,
    /// 個別の線種名
    Own(String),
}

/// 持ち上げた `ATTRIB` 1 件
#[derive(Clone, Debug)]
pub struct PlainAttrib {
    /// レコード番号
    pub record: u32,
    /// 出所
    pub origin: Origin,
    /// ハンドル
    pub handle: Option<u64>,
    /// タグ名
    pub tag: String,
    /// 非表示か
    pub hidden: bool,
    /// 共通スタイル
    pub style: PlainStyle,
    /// 文字の幾何
    pub text: PlainText,
}

/// 持ち上げた平面 6 種類の 1 図形
#[derive(Clone, Debug)]
pub struct PlainLifted {
    /// レコード番号
    pub record: u32,
    /// 出所
    pub origin: Origin,
    /// ハンドル
    pub handle: Option<u64>,
    /// 共通スタイル
    pub style: PlainStyle,
    /// 幾何
    pub geom: PlainGeom,
}

#[derive(Default)]
struct VecPolySink {
    xy: Vec<[f64; 2]>,
    bulges: Vec<f64>,
    widths: Vec<[f64; 2]>,
}

impl PolylineSink for VecPolySink {
    fn len(&self) -> usize {
        self.xy.len()
    }

    fn push_vertex(&mut self, x: f64, y: f64) {
        self.xy.push([x, y]);
    }

    fn set_width(&mut self, is_end: bool, width: f64) {
        if self.widths.len() < self.xy.len() {
            self.widths.resize(self.xy.len(), [0.0, 0.0]);
        }
        if let Some(pair) = self.widths.last_mut() {
            if is_end {
                pair[1] = width;
            } else {
                pair[0] = width;
            }
        }
    }

    fn set_bulge(&mut self, bulge: f64) {
        if bulge == 0.0 {
            return;
        }
        if self.bulges.len() < self.xy.len() {
            self.bulges.resize(self.xy.len(), 0.0);
        }
        if let Some(last) = self.bulges.last_mut() {
            *last = bulge;
        }
    }
}

fn decode_str(charset: Charset, bytes: &[u8], code: i16) -> Result<String, LiftFail> {
    if charset == Charset::Other && !bytes.is_ascii() {
        return Err(LiftFail::Invalid(code));
    }
    charset.decode(bytes).ok_or(LiftFail::Invalid(code))
}

fn decode_plain_style<'a>(
    fields: &impl FieldSource<'a>,
    charset: Charset,
) -> Result<PlainStyle, LiftFail> {
    let layer_raw = fields.raw_text(8)?.ok_or(LiftFail::Invalid(8))?;
    let layer = decode_str(charset, layer_raw, 8)?;
    if layer.is_empty() {
        return Err(LiftFail::Invalid(8));
    }
    let st: EntityStyle<'a> = single::entity_style(fields)?;
    let line_type = match st.line_type {
        RawLineType::Layer => PlainLineType::Layer,
        RawLineType::Block => PlainLineType::Block,
        RawLineType::Named(raw) => {
            let decoded = decode_str(charset, raw, 6)?;
            if decoded.is_empty() {
                return Err(LiftFail::Invalid(6));
            }
            PlainLineType::Own(decoded)
        }
    };
    Ok(PlainStyle {
        layer,
        color: st.color,
        line_type,
        line_type_scale: st.line_type_scale,
        line_weight: st.line_weight,
        transparency: st.transparency,
    })
}

fn convert_raw_text(raw: RawText<'_>, charset: Charset) -> Result<PlainText, LiftFail> {
    if raw.height <= 0.0 || raw.width_factor <= 0.0 || raw.oblique.abs() >= 85.0 {
        return Err(LiftFail::Invalid(40));
    }
    let content = decode_str(charset, raw.content, 1)?;
    let style = match raw.style {
        Some(bytes) => decode_str(charset, bytes, 7)?,
        None => "STANDARD".to_owned(),
    };
    let (point, second, horizontal, vertical) = match raw.place {
        RawTextPlace::Anchor {
            point,
            horizontal,
            vertical,
        } => {
            let h = match horizontal {
                Horizontal::Left => 0,
                Horizontal::Center => 1,
                Horizontal::Right => 2,
                Horizontal::Middle => 3,
            };
            let v = match vertical {
                Vertical::Baseline => 0,
                Vertical::Bottom => 1,
                Vertical::Middle => 2,
                Vertical::Top => 3,
            };
            (point, [0.0, 0.0], h, v)
        }
        RawTextPlace::Between { first, second, fit } => {
            let h = match fit {
                Fit::Aligned => 4,
                Fit::Fit => 5,
            };
            (first, second, h, 0)
        }
    };
    let flags = u8::from(raw.backward) | (u8::from(raw.upside_down) << 1);
    Ok(PlainText {
        point,
        second,
        height: raw.height,
        rotation: raw.rotation,
        width_factor: raw.width_factor,
        oblique: raw.oblique,
        content,
        style,
        horizontal,
        vertical,
        flags,
    })
}

/// 1 つの `ATTRIB` を高速経路で持ち上げる（101 埋め込み MTEXT や失敗があれば `None`）
pub fn try_lift_plain_attrib<'a>(
    fields: &impl FieldSource<'a>,
    charset: Charset,
    record: u32,
    origin: Origin,
    handle: Option<u64>,
) -> Option<PlainAttrib> {
    if fields.has_code(101) || fields.int(67, 0).ok()? == 1 {
        return None;
    }
    single::check_upright(fields).ok()?;
    let tag_raw = fields.raw_text(2).ok()??;
    let tag = decode_str(charset, tag_raw, 2).ok()?;
    let flags = fields.int(70, 0).ok()?;
    let hidden = flags & 1 != 0;
    let raw_text = single::text_geom(fields, 74).ok()?;
    let text = convert_raw_text(raw_text, charset).ok()?;
    let style = decode_plain_style(fields, charset).ok()?;
    Some(PlainAttrib {
        record,
        origin,
        handle,
        tag,
        hidden,
        style,
        text,
    })
}

/// モデル空間の 6 種類の図形を高速経路で持ち上げる。失敗や非対応条件なら `None`（呼び出し側が一般経路に回す）
#[allow(clippy::too_many_arguments)]
pub fn try_lift_plain_entity<'a, F, S>(
    kind: &[u8],
    fields: &F,
    poly_stream: Option<S>,
    charset: Charset,
    record: u32,
    origin: Origin,
    handle: Option<u64>,
    attribs: Vec<PlainAttrib>,
    raw_attrib_count: u32,
) -> Option<PlainLifted>
where
    F: FieldSource<'a>,
    S: PolylineStream,
{
    if fields.int(67, 0).ok()? == 1 {
        return None;
    }
    single::check_upright(fields).ok()?;
    let style = decode_plain_style(fields, charset).ok()?;
    let geom = match kind {
        b"LINE" => PlainGeom::Line(single::line_ends(fields).ok()?),
        b"CIRCLE" => PlainGeom::Circle(single::circle_geom(fields).ok()?),
        b"ARC" => PlainGeom::Arc(single::arc_geom(fields).ok()?),
        b"LWPOLYLINE" => {
            let stream = poly_stream?;
            let mut sink = VecPolySink::default();
            let meta = single::lwpolyline_scan(fields, stream, &mut sink).ok()?;
            if sink.xy.len() < 2 {
                return None;
            }
            if !sink.bulges.is_empty() {
                sink.bulges.resize(sink.xy.len(), 0.0);
            }
            if !sink.widths.is_empty() {
                sink.widths.resize(sink.xy.len(), [0.0, 0.0]);
            }
            PlainGeom::Polyline {
                xy: sink.xy,
                bulges: sink.bulges,
                widths: sink.widths,
                constant_width: meta.width,
                closed: meta.closed,
                line_type_generation: meta.line_type_generation,
            }
        }
        b"TEXT" => {
            let raw = single::text_geom(fields, 73).ok()?;
            PlainGeom::Text(convert_raw_text(raw, charset).ok()?)
        }
        b"INSERT" => {
            let raw = single::insert_geom(fields).ok()?;
            let name = decode_str(charset, raw.name, 2).ok()?;
            if name.is_empty() {
                return None;
            }
            PlainGeom::Insert {
                name,
                position: raw.position,
                scale: raw.scale,
                rotation: raw.rotation,
                grid: raw.grid,
                spacing: raw.spacing,
                followed: raw.followed,
                attribs,
                raw_attrib_count,
                clip: None,
            }
        }
        _ => return None,
    };
    Some(PlainLifted {
        record,
        origin,
        handle,
        style,
        geom,
    })
}

impl LiveShapes {
    /// [`PlainStyle`] をプールと [`SlotCatalog`] に登録し、`(layer_slot, packed_color, extra_idx)` を返す
    pub fn store_style(
        &mut self,
        style: &PlainStyle,
        catalog: &mut SlotCatalog,
        count_model_ref: bool,
    ) -> (u32, u32, u32) {
        let layer_slot = catalog.intern_layer(&style.layer);
        if count_model_ref && let Some(entry) = catalog.layers.get_mut(layer_slot as usize) {
            entry.model_refs = entry.model_refs.saturating_add(1);
        }
        let line_type = match &style.line_type {
            PlainLineType::Layer => LineTypeSlotRef::Layer,
            PlainLineType::Block => LineTypeSlotRef::Block,
            PlainLineType::Own(name) => {
                let lt_slot = catalog.intern_line_type(name);
                if count_model_ref && let Some(entry) = catalog.line_types.get_mut(lt_slot as usize)
                {
                    entry.model_refs = entry.model_refs.saturating_add(1);
                }
                LineTypeSlotRef::Own(lt_slot)
            }
        };
        let color = pack_color(style.color);
        let extra = self.pools.intern_extra(ExtraStyle {
            color: style.color,
            line_type,
            line_type_scale: style.line_type_scale,
            line_weight: style.line_weight,
            transparency: style.transparency,
        });
        (layer_slot, color, extra)
    }

    fn store_text(&mut self, text: &PlainText) -> TextGeom {
        let content = self.pools.push_str(&text.content);
        let style = self.pools.push_str(&text.style);
        TextGeom {
            point: text.point,
            second: text.second,
            height: text.height,
            rotation: text.rotation,
            width_factor: text.width_factor,
            oblique: text.oblique,
            content,
            style,
            horizontal: text.horizontal,
            vertical: text.vertical,
            flags: text.flags,
        }
    }

    /// 持ち上げた平面 6 種類の図形を列に積み、[`LiveRow`] を返す
    pub fn push_plain(
        &mut self,
        item: PlainLifted,
        catalog: &mut SlotCatalog,
        unique_handle: bool,
        born: bool,
        block_defs: &BTreeMap<String, u32>,
    ) -> LiveRow {
        let (layer_slot, color, extra) = self.store_style(&item.style, catalog, true);
        let (kind, shape) = match item.geom {
            PlainGeom::Line(ends) => {
                let idx = self.lines.len() as u32;
                self.lines.push(LineGeom { ends });
                (LiveKind::Line, idx)
            }
            PlainGeom::Circle([x, y, radius]) => {
                let idx = self.circles.len() as u32;
                self.circles.push(CircleGeom {
                    center: [x, y],
                    radius,
                });
                (LiveKind::Circle, idx)
            }
            PlainGeom::Arc([x, y, radius, start, end]) => {
                let idx = self.arcs.len() as u32;
                self.arcs.push(ArcGeom {
                    center: [x, y],
                    radius,
                    start,
                    end,
                });
                (LiveKind::Arc, idx)
            }
            PlainGeom::Polyline {
                xy,
                bulges,
                widths,
                constant_width,
                closed,
                line_type_generation,
            } => {
                let xy_start = self.pools.xy.len() as u32;
                let count = xy.len() as u32;
                self.pools.xy.extend_from_slice(&xy);
                let bulge_start = if bulges.is_empty() {
                    u32::MAX
                } else {
                    let s = self.pools.bulges.len() as u32;
                    self.pools.bulges.extend_from_slice(&bulges);
                    s
                };
                let width_start = if widths.is_empty() {
                    u32::MAX
                } else {
                    let s = self.pools.widths.len() as u32;
                    self.pools.widths.extend_from_slice(&widths);
                    s
                };
                let flags = u8::from(closed) | (u8::from(line_type_generation) << 1);
                let idx = self.polylines.len() as u32;
                self.polylines.push(PolyGeom {
                    xy_start,
                    count,
                    bulge_start,
                    width_start,
                    constant_width: constant_width.unwrap_or(f64::NAN),
                    flags,
                });
                (LiveKind::Polyline, idx)
            }
            PlainGeom::Text(text) => {
                let geom = self.store_text(&text);
                let idx = self.texts.len() as u32;
                self.texts.push(geom);
                (LiveKind::Text, idx)
            }
            PlainGeom::Insert {
                name,
                position,
                scale,
                rotation,
                grid,
                spacing,
                followed: _,
                attribs,
                raw_attrib_count,
                clip,
            } => {
                let definition = block_defs
                    .get(&name.to_lowercase())
                    .copied()
                    .unwrap_or(NO_DEFINITION);
                let name_span = self.pools.push_str(&name);
                let attrib_start = self.pools.attribs.len() as u32;
                let attrib_count = attribs.len() as u32;
                for attr in attribs {
                    let (a_layer, a_color, a_extra) = self.store_style(&attr.style, catalog, true);
                    let tag = self.pools.push_str(&attr.tag);
                    let text = self.store_text(&attr.text);
                    let (orig_start, orig_end, dwg_flag, born_flag) = match attr.origin {
                        Origin::Dwg { offset, end } => (offset as u32, end as u32, 1 << 2, 0),
                        Origin::New => (0, 0, 0, 1 << 3),
                        Origin::Dxf { .. } => (0, 0, 0, 0),
                    };
                    let attrib_flags = u8::from(attr.hidden)
                        | (u8::from(attr.handle.is_some()) << 1)
                        | dwg_flag
                        | born_flag;
                    self.pools.attribs.push(AttribRow {
                        record: attr.record,
                        origin_start: orig_start,
                        origin_end: orig_end,
                        handle: attr.handle.unwrap_or(0),
                        layer_slot: a_layer,
                        color: a_color,
                        extra: a_extra,
                        tag,
                        text,
                        attrib_flags,
                    });
                }
                let clip_index = match clip {
                    Some(c) => {
                        let idx = self.pools.clips.len() as u32;
                        self.pools.clips.push(c);
                        idx
                    }
                    None => u32::MAX,
                };
                let idx = self.inserts.len() as u32;
                self.inserts.push(InsertGeom {
                    position,
                    scale,
                    rotation,
                    grid,
                    spacing,
                    name: name_span,
                    definition,
                    attrib_start,
                    attrib_count,
                    raw_attrib_count,
                    clip_index,
                });
                (LiveKind::Insert, idx)
            }
        };
        let mut flags = 0u8;
        if item.handle.is_some() {
            flags |= ROW_HAS_HANDLE;
            if unique_handle {
                flags |= ROW_UNIQUE_HANDLE;
            }
        }
        if born {
            flags |= ROW_BORN;
        }
        let (origin_start, origin_end) = match item.origin {
            Origin::Dwg { offset, end } => {
                flags |= ROW_DWG_ORIGIN;
                (offset as u32, end as u32)
            }
            Origin::Dxf { .. } | Origin::New => (0, 0),
        };
        LiveRow {
            record: item.record,
            shape,
            layer_slot,
            color,
            extra,
            handle: item.handle.unwrap_or(0),
            origin_start,
            origin_end,
            kind,
            flags,
        }
    }

    /// 一般図形を列に積み、[`LiveRow`] を返す。`element` の `layer`・`style.line_type` はすでにスロット番号に変換済みとする
    #[allow(clippy::too_many_arguments)]
    pub fn push_general(
        &mut self,
        record: u32,
        origin: Origin,
        handle: Option<u64>,
        unique_handle: bool,
        has_102_or_360: bool,
        born: bool,
        general: GeneralRow,
    ) -> LiveRow {
        let layer_slot = general.element.layer.0;
        let color = pack_color(general.element.style.color);
        let shape = self.generals.len() as u32;
        self.generals.push(general);
        let mut flags = 0u8;
        if handle.is_some() {
            flags |= ROW_HAS_HANDLE;
            if unique_handle {
                flags |= ROW_UNIQUE_HANDLE;
            }
        }
        if has_102_or_360 {
            flags |= ROW_HAS_102_OR_360;
        }
        if born {
            flags |= ROW_BORN;
        }
        let (origin_start, origin_end) = match origin {
            Origin::Dwg { offset, end } => {
                flags |= ROW_DWG_ORIGIN;
                (offset as u32, end as u32)
            }
            Origin::Dxf { .. } | Origin::New => (0, 0),
        };
        LiveRow {
            record,
            shape,
            layer_slot,
            color,
            extra: u32::MAX,
            handle: handle.unwrap_or(0),
            origin_start,
            origin_end,
            kind: LiveKind::General,
            flags,
        }
    }

    /// 行から [`Element<ModelPoint>`] を組み立てる（画層・線種 ID は正規番号に写す）
    pub fn to_element(&self, row: &LiveRow, catalog: &SlotCatalog) -> Option<Element<ModelPoint>> {
        let source = row.source_ref();
        let layer = catalog.layer_id(row.layer_slot);
        let style = if row.extra == u32::MAX {
            Style {
                color: unpack_color(row.color),
                line_type: Inherit::Layer,
                line_type_scale: 1.0,
                line_weight: Some(Inherit::Layer),
                transparency: Inherit::Layer,
            }
        } else {
            self.pools.extras.get(row.extra as usize)?.to_style(catalog)
        };
        let pt = |[x, y]: [f64; 2]| ModelPoint { space: MODEL, x, y };
        let len = |value: f64| {
            Length::Model(ModelLength {
                space: MODEL,
                value,
            })
        };
        let shape = match row.kind {
            LiveKind::Line => {
                let g = self.lines.get(row.shape as usize)?;
                Shape::Line {
                    start: pt([g.ends[0], g.ends[1]]),
                    end: pt([g.ends[2], g.ends[3]]),
                }
            }
            LiveKind::Circle => {
                let g = self.circles.get(row.shape as usize)?;
                Shape::Circle {
                    center: pt(g.center),
                    radius: len(g.radius),
                }
            }
            LiveKind::Arc => {
                let g = self.arcs.get(row.shape as usize)?;
                Shape::Curve(Edge::Arc {
                    center: pt(g.center),
                    radius: len(g.radius),
                    start: g.start,
                    end: g.end,
                    counterclockwise: true,
                })
            }
            LiveKind::Polyline => {
                let g = self.polylines.get(row.shape as usize)?;
                let mut vertices = Vec::with_capacity(g.count as usize);
                let mut vertex_widths = if g.width_start == u32::MAX {
                    Vec::new()
                } else {
                    Vec::with_capacity(g.count as usize)
                };
                for i in 0..g.count {
                    let xy = *self.pools.xy.get((g.xy_start + i) as usize)?;
                    let bulge = if g.bulge_start == u32::MAX {
                        0.0
                    } else {
                        *self.pools.bulges.get((g.bulge_start + i) as usize)?
                    };
                    if g.width_start != u32::MAX {
                        let [s, e] = *self.pools.widths.get((g.width_start + i) as usize)?;
                        vertex_widths.push([len(s), len(e)]);
                    }
                    vertices.push(Vertex {
                        point: pt(xy),
                        bulge,
                    });
                }
                Shape::Polyline(Polyline {
                    vertices,
                    closed: g.flags & 1 != 0,
                    width: (!g.constant_width.is_nan()).then(|| len(g.constant_width)),
                    vertex_widths,
                    line_type_generation: g.flags & 2 != 0,
                })
            }
            LiveKind::Text => {
                let g = self.texts.get(row.shape as usize)?;
                Shape::Text(self.text_to_model(g, "dxf.TEXT.40"))
            }
            LiveKind::Insert => {
                let g = self.inserts.get(row.shape as usize)?;
                let name = self.pools.get_string(g.name);
                let grid = (g.grid != [1, 1]).then(|| Grid {
                    columns: g.grid[0],
                    rows: g.grid[1],
                    column_spacing: len(g.spacing[0]),
                    row_spacing: len(g.spacing[1]),
                });
                let mut attributes = Vec::with_capacity(g.attrib_count as usize);
                for i in 0..g.attrib_count {
                    let attr = self.pools.attribs.get((g.attrib_start + i) as usize)?;
                    let a_style = if attr.extra == u32::MAX {
                        Style {
                            color: unpack_color(attr.color),
                            line_type: Inherit::Layer,
                            line_type_scale: 1.0,
                            line_weight: Some(Inherit::Layer),
                            transparency: Inherit::Layer,
                        }
                    } else {
                        self.pools
                            .extras
                            .get(attr.extra as usize)?
                            .to_style(catalog)
                    };
                    attributes.push(Attribute {
                        tag: self.pools.get_string(attr.tag),
                        text: self.text_to_model(&attr.text, "dxf.ATTRIB.40"),
                        formatted: None,
                        hidden: attr.attrib_flags & 1 != 0,
                        layer: catalog.layer_id(attr.layer_slot),
                        style: a_style,
                        source: attr.source_ref(),
                    });
                }
                let clip = if g.clip_index == u32::MAX {
                    None
                } else {
                    self.pools.clips.get(g.clip_index as usize).cloned()
                };
                Shape::Symbol(Symbol {
                    key: SymbolKey {
                        namespace: BLOCK.into(),
                        name,
                    },
                    position: pt(g.position),
                    scale: g.scale,
                    rotation: g.rotation,
                    grid,
                    attributes,
                    clip,
                })
            }
            LiveKind::General => {
                let g = self.generals.get(row.shape as usize)?;
                let mut el = g.element.clone();
                el.source = source;
                el.layer = catalog.layer_id(row.layer_slot);
                if let Inherit::Own(lt_slot) = el.style.line_type {
                    el.style.line_type = Inherit::Own(catalog.line_type_id(lt_slot.0));
                }
                if let Shape::Symbol(sym) = &mut el.shape {
                    for (attr, &(a_layer, a_lt)) in sym.attributes.iter_mut().zip(&g.attrib_slots) {
                        attr.layer = catalog.layer_id(a_layer);
                        attr.style.line_type = match a_lt {
                            LineTypeSlotRef::Layer => Inherit::Layer,
                            LineTypeSlotRef::Block => Inherit::Block,
                            LineTypeSlotRef::Own(s) => Inherit::Own(catalog.line_type_id(s)),
                        };
                    }
                }
                if let Shape::Callout(callout) = &mut el.shape {
                    callout.line_style.line_type = match g.callout_line_type_slot {
                        LineTypeSlotRef::Layer => Inherit::Layer,
                        LineTypeSlotRef::Block => Inherit::Block,
                        LineTypeSlotRef::Own(s) => Inherit::Own(catalog.line_type_id(s)),
                    };
                }
                return Some(el);
            }
        };
        Some(Element {
            source,
            layer,
            style,
            shape,
        })
    }

    fn text_to_model(&self, g: &TextGeom, height_key: &'static str) -> Text<ModelPoint> {
        let pt = |[x, y]: [f64; 2]| ModelPoint { space: MODEL, x, y };
        let placement = match g.horizontal {
            4 => Placement::Between {
                first: pt(g.point),
                second: pt(g.second),
                fit: Fit::Aligned,
            },
            5 => Placement::Between {
                first: pt(g.point),
                second: pt(g.second),
                fit: Fit::Fit,
            },
            h => Placement::Anchor {
                point: pt(g.point),
                horizontal: match h {
                    1 => Horizontal::Center,
                    2 => Horizontal::Right,
                    3 => Horizontal::Middle,
                    _ => Horizontal::Left,
                },
                vertical: match g.vertical {
                    1 => Vertical::Bottom,
                    2 => Vertical::Middle,
                    3 => Vertical::Top,
                    _ => Vertical::Baseline,
                },
            },
        };
        Text {
            content: self.pools.get_string(g.content),
            height: Height {
                value: Length::Model(ModelLength {
                    space: MODEL,
                    value: g.height,
                }),
                meaning: HeightMeaning::Declared(height_key.into()),
            },
            placement,
            rotation: g.rotation,
            width_factor: g.width_factor,
            oblique: g.oblique,
            backward: g.flags & 1 != 0,
            upside_down: g.flags & 2 != 0,
            style: self.pools.get_string(g.style),
        }
    }

    /// 行の軸平行境界矩形 `[min_x, min_y, max_x, max_y]` を求める
    pub fn row_bounds(&self, row: &LiveRow, symbols: &[SymbolDefinition]) -> [f64; 4] {
        match row.kind {
            LiveKind::Line => match self.lines.get(row.shape as usize) {
                Some(g) => [
                    g.ends[0].min(g.ends[2]),
                    g.ends[1].min(g.ends[3]),
                    g.ends[0].max(g.ends[2]),
                    g.ends[1].max(g.ends[3]),
                ],
                None => empty_box(),
            },
            LiveKind::Circle => match self.circles.get(row.shape as usize) {
                Some(g) => [
                    g.center[0] - g.radius,
                    g.center[1] - g.radius,
                    g.center[0] + g.radius,
                    g.center[1] + g.radius,
                ],
                None => empty_box(),
            },
            LiveKind::Arc => match self.arcs.get(row.shape as usize) {
                Some(g) => arc_bounds(g.center, g.radius, g.start, g.end, true),
                None => empty_box(),
            },
            LiveKind::Polyline => match self.polylines.get(row.shape as usize) {
                Some(g) => self.poly_bounds(g),
                None => empty_box(),
            },
            LiveKind::Text => match self.texts.get(row.shape as usize) {
                Some(g) => text_geom_bounds(g, &self.pools),
                None => empty_box(),
            },
            LiveKind::Insert => match self.inserts.get(row.shape as usize) {
                Some(g) => self.insert_bounds(g, symbols),
                None => empty_box(),
            },
            LiveKind::General => match self.generals.get(row.shape as usize) {
                Some(g) => shape_bounds(&g.element.shape, symbols, 0),
                None => empty_box(),
            },
        }
    }

    fn poly_bounds(&self, g: &PolyGeom) -> [f64; 4] {
        let mut bx = empty_box();
        if g.count == 0 {
            return bx;
        }
        let closed = g.flags & 1 != 0;
        let segs = if closed && g.count > 1 {
            g.count
        } else {
            g.count.saturating_sub(1)
        };
        if segs == 0 {
            if let Some(&p) = self.pools.xy.get(g.xy_start as usize) {
                expand_pt(&mut bx, p);
            }
            return bx;
        }
        for i in 0..segs {
            let Some(&p0) = self.pools.xy.get((g.xy_start + i) as usize) else {
                continue;
            };
            let j = if i + 1 < g.count { i + 1 } else { 0 };
            let Some(&p1) = self.pools.xy.get((g.xy_start + j) as usize) else {
                continue;
            };
            expand_pt(&mut bx, p0);
            expand_pt(&mut bx, p1);
            if g.bulge_start != u32::MAX
                && let Some(&b) = self.pools.bulges.get((g.bulge_start + i) as usize)
                && let Some((c, r, start, sweep)) = bulge_arc_deg(p0, p1, b)
            {
                let ab = if sweep >= 0.0 {
                    arc_bounds(c, r, start, start + sweep, true)
                } else {
                    arc_bounds(c, r, start + sweep, start, true)
                };
                merge_box(&mut bx, ab);
            }
        }
        bx
    }

    fn insert_bounds(&self, g: &InsertGeom, symbols: &[SymbolDefinition]) -> [f64; 4] {
        let mut bx = empty_box();
        for i in 0..g.attrib_count {
            if let Some(attr) = self.pools.attribs.get((g.attrib_start + i) as usize)
                && attr.attrib_flags & 1 == 0
            {
                merge_box(&mut bx, text_geom_bounds(&attr.text, &self.pools));
            }
        }
        if g.definition != NO_DEFINITION
            && let Some(def) = symbols.get(g.definition as usize)
        {
            let mut local = empty_box();
            for el in &def.space.elements {
                merge_box(&mut local, shape_bounds(&el.shape, symbols, 1));
            }
            if local[0] <= local[2] {
                let rad = g.rotation * (core::f64::consts::PI / 180.0);
                let (s, c) = (libm::sin(rad), libm::cos(rad));
                let cols = g.grid[0].clamp(1, 256);
                let rows = g.grid[1].clamp(1, 256);
                for col in 0..cols {
                    for row in 0..rows {
                        let ox = f64::from(col) * g.spacing[0];
                        let oy = f64::from(row) * g.spacing[1];
                        let base = [
                            g.position[0] + ox * c - oy * s,
                            g.position[1] + ox * s + oy * c,
                        ];
                        for corner in [
                            [local[0], local[1]],
                            [local[2], local[1]],
                            [local[2], local[3]],
                            [local[0], local[3]],
                        ] {
                            let sx = corner[0] * g.scale[0];
                            let sy = corner[1] * g.scale[1];
                            expand_pt(
                                &mut bx,
                                [base[0] + sx * c - sy * s, base[1] + sx * s + sy * c],
                            );
                        }
                    }
                }
            }
        }
        if bx[0] > bx[2] {
            expand_pt(&mut bx, g.position);
        }
        bx
    }

    /// 表示バッファーに載せるプリミティブ数 `[線分, 円弧, 文字, 挿入]` を見積もる
    pub fn display_counts(&self, row: &LiveRow, symbols: &[SymbolDefinition]) -> [u32; 4] {
        match row.kind {
            LiveKind::Line => [1, 0, 0, 0],
            LiveKind::Circle | LiveKind::Arc => [0, 1, 0, 0],
            LiveKind::Polyline => match self.polylines.get(row.shape as usize) {
                Some(g) => self.poly_display_counts(g),
                None => [0; 4],
            },
            LiveKind::Text => [0, 0, 1, 0],
            LiveKind::Insert => match self.inserts.get(row.shape as usize) {
                Some(g) => self.insert_display_counts(g),
                None => [0; 4],
            },
            LiveKind::General => match self.generals.get(row.shape as usize) {
                Some(g) => shape_display_counts(&g.element.shape, symbols, 0),
                None => [0; 4],
            },
        }
    }

    fn poly_display_counts(&self, g: &PolyGeom) -> [u32; 4] {
        let closed = g.flags & 1 != 0;
        let n = if closed && g.count > 1 {
            g.count
        } else {
            g.count.saturating_sub(1)
        };
        if g.bulge_start == u32::MAX {
            return [n, 0, 0, 0];
        }
        let mut segs = 0u32;
        let mut arcs = 0u32;
        for i in 0..n {
            let Some(&p0) = self.pools.xy.get((g.xy_start + i) as usize) else {
                continue;
            };
            let j = if i + 1 < g.count { i + 1 } else { 0 };
            let Some(&p1) = self.pools.xy.get((g.xy_start + j) as usize) else {
                continue;
            };
            let b = self
                .pools
                .bulges
                .get((g.bulge_start + i) as usize)
                .copied()
                .unwrap_or(0.0);
            if bulge_arc_deg(p0, p1, b).is_some() {
                arcs = arcs.saturating_add(1);
            } else {
                segs = segs.saturating_add(1);
            }
        }
        [segs, arcs, 0, 0]
    }

    fn insert_display_counts(&self, g: &InsertGeom) -> [u32; 4] {
        let mut texts = 0u32;
        for i in 0..g.attrib_count {
            if let Some(attr) = self.pools.attribs.get((g.attrib_start + i) as usize)
                && attr.attrib_flags & 1 == 0
            {
                texts = texts.saturating_add(1);
            }
        }
        [0, 0, texts, 1]
    }

    /// 行の幾何を [`DisplayWriter`] に書き出す
    pub fn write_display(
        &self,
        row: &LiveRow,
        symbols: &[SymbolDefinition],
        w: &mut DisplayWriter<'_>,
    ) {
        match row.kind {
            LiveKind::Line => {
                if let Some(g) = self.lines.get(row.shape as usize) {
                    w.push_segment([g.ends[0], g.ends[1]], [g.ends[2], g.ends[3]]);
                }
            }
            LiveKind::Circle => {
                if let Some(g) = self.circles.get(row.shape as usize) {
                    w.push_circle(g.center, g.radius);
                }
            }
            LiveKind::Arc => {
                if let Some(g) = self.arcs.get(row.shape as usize) {
                    w.push_arc_deg(g.center, g.radius, g.start, g.end);
                }
            }
            LiveKind::Polyline => {
                if let Some(g) = self.polylines.get(row.shape as usize) {
                    self.write_poly_display(g, w);
                }
            }
            LiveKind::Text => {
                if let Some(g) = self.texts.get(row.shape as usize) {
                    write_text_geom(g, &self.pools, None, w);
                }
            }
            LiveKind::Insert => {
                if let Some(g) = self.inserts.get(row.shape as usize) {
                    self.write_insert_display(g, w);
                }
            }
            LiveKind::General => {
                if let Some(g) = self.generals.get(row.shape as usize) {
                    write_shape_display(&g.element.shape, symbols, &Xform::IDENTITY, 0, w);
                }
            }
        }
    }

    fn write_poly_display(&self, g: &PolyGeom, w: &mut DisplayWriter<'_>) {
        let closed = g.flags & 1 != 0;
        let n = if closed && g.count > 1 {
            g.count
        } else {
            g.count.saturating_sub(1)
        };
        for i in 0..n {
            let Some(&p0) = self.pools.xy.get((g.xy_start + i) as usize) else {
                continue;
            };
            let j = if i + 1 < g.count { i + 1 } else { 0 };
            let Some(&p1) = self.pools.xy.get((g.xy_start + j) as usize) else {
                continue;
            };
            if g.bulge_start != u32::MAX {
                let b = self
                    .pools
                    .bulges
                    .get((g.bulge_start + i) as usize)
                    .copied()
                    .unwrap_or(0.0);
                if bulge_arc_deg(p0, p1, b).is_some() {
                    w.push_bulged(p0, p1, b);
                    continue;
                }
            }
            w.push_segment(p0, p1);
        }
    }

    fn write_insert_display(&self, g: &InsertGeom, w: &mut DisplayWriter<'_>) {
        for i in 0..g.attrib_count {
            if let Some(attr) = self.pools.attribs.get((g.attrib_start + i) as usize)
                && attr.attrib_flags & 1 == 0
            {
                write_text_geom(
                    &attr.text,
                    &self.pools,
                    Some([attr.layer_slot, attr.color]),
                    w,
                );
            }
        }
        w.push_instance(
            g.position,
            g.scale,
            radians_from_deg(g.rotation),
            g.spacing,
            g.definition,
            [g.grid[0].max(1), g.grid[1].max(1)],
        );
    }
}

#[derive(Clone, Copy)]
struct Xform {
    m: [f64; 6],
}

impl Xform {
    const IDENTITY: Self = Self {
        m: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
    };

    #[inline]
    fn apply(&self, p: [f64; 2]) -> [f64; 2] {
        [
            self.m[0] * p[0] + self.m[1] * p[1] + self.m[2],
            self.m[3] * p[0] + self.m[4] * p[1] + self.m[5],
        ]
    }

    fn compose(&self, inner: &Self) -> Self {
        let a = &self.m;
        let b = &inner.m;
        Self {
            m: [
                a[0] * b[0] + a[1] * b[3],
                a[0] * b[1] + a[1] * b[4],
                a[0] * b[2] + a[1] * b[5] + a[2],
                a[3] * b[0] + a[4] * b[3],
                a[3] * b[1] + a[4] * b[4],
                a[3] * b[2] + a[4] * b[5] + a[5],
            ],
        }
    }

    /// 等方相似変換なら `(scale, rotation_deg, mirrored)` を返す
    fn similarity(&self) -> Option<(f64, f64, bool)> {
        let sx = libm::hypot(self.m[0], self.m[3]);
        let sy = libm::hypot(self.m[1], self.m[4]);
        if sx <= 0.0 || (sx - sy).abs() > sx * 1e-6 {
            return None;
        }
        let det = self.m[0] * self.m[4] - self.m[1] * self.m[3];
        let angle = libm::atan2(self.m[3], self.m[0]) * (180.0 / core::f64::consts::PI);
        Some((sx, angle, det < 0.0))
    }
}

fn empty_box() -> [f64; 4] {
    [
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    ]
}

fn expand_pt(bx: &mut [f64; 4], [x, y]: [f64; 2]) {
    if x.is_finite() && y.is_finite() {
        bx[0] = bx[0].min(x);
        bx[1] = bx[1].min(y);
        bx[2] = bx[2].max(x);
        bx[3] = bx[3].max(y);
    }
}

fn merge_box(dst: &mut [f64; 4], src: [f64; 4]) {
    if src[0] <= src[2] && src[1] <= src[3] {
        dst[0] = dst[0].min(src[0]);
        dst[1] = dst[1].min(src[1]);
        dst[2] = dst[2].max(src[2]);
        dst[3] = dst[3].max(src[3]);
    }
}

fn arc_bounds(center: [f64; 2], radius: f64, start: f64, end: f64, ccw: bool) -> [f64; 4] {
    let sweep = zumen_geom2d::sweep(start, end, ccw);
    if sweep.abs() >= 360.0 {
        return [
            center[0] - radius,
            center[1] - radius,
            center[0] + radius,
            center[1] + radius,
        ];
    }
    let (a0, span) = if sweep >= 0.0 {
        (start.rem_euclid(360.0), sweep)
    } else {
        ((start + sweep).rem_euclid(360.0), -sweep)
    };
    let mut bx = empty_box();
    for deg in [a0, a0 + span] {
        let rad = deg * (core::f64::consts::PI / 180.0);
        expand_pt(
            &mut bx,
            [
                center[0] + radius * libm::cos(rad),
                center[1] + radius * libm::sin(rad),
            ],
        );
    }
    for cardinal in [0.0, 90.0, 180.0, 270.0] {
        let delta = (cardinal - a0).rem_euclid(360.0);
        if delta <= span {
            let rad = cardinal * (core::f64::consts::PI / 180.0);
            expand_pt(
                &mut bx,
                [
                    center[0] + radius * libm::cos(rad),
                    center[1] + radius * libm::sin(rad),
                ],
            );
        }
    }
    bx
}

fn bulge_arc_deg(from: [f64; 2], to: [f64; 2], bulge: f64) -> Option<([f64; 2], f64, f64, f64)> {
    if !bulge.is_finite() || bulge.abs() < 1e-9 {
        return None;
    }
    let dx = to[0] - from[0];
    let dy = to[1] - from[1];
    let chord = libm::hypot(dx, dy);
    if !chord.is_finite() || chord <= 0.0 {
        return None;
    }
    let offset = (1.0 - bulge * bulge) / (4.0 * bulge);
    let center = [
        (from[0] + to[0]) * 0.5 - dy * offset,
        (from[1] + to[1]) * 0.5 + dx * offset,
    ];
    let radius = chord * (1.0 + bulge * bulge) / (4.0 * bulge.abs());
    let start_deg =
        libm::atan2(from[1] - center[1], from[0] - center[0]) * (180.0 / core::f64::consts::PI);
    let sweep_deg = 4.0 * libm::atan(bulge) * (180.0 / core::f64::consts::PI);
    Some((center, radius, start_deg, sweep_deg))
}

#[inline]
fn text_axes(rad: f64, width: f64, height: f64) -> ([f64; 2], [f64; 2]) {
    let (s, c) = (libm::sin(rad), libm::cos(rad));
    ([width * c, width * s], [-height * s, height * c])
}

fn text_geom_bounds(g: &TextGeom, pools: &Pools) -> [f64; 4] {
    let chars = pools.get_string(g.content).chars().count().max(1) as f64;
    let width = if g.horizontal >= 4 {
        libm::hypot(g.second[0] - g.point[0], g.second[1] - g.point[1]).max(g.height)
    } else {
        g.height * g.width_factor * 0.6 * chars
    };
    let rad = if g.horizontal >= 4 {
        libm::atan2(g.second[1] - g.point[1], g.second[0] - g.point[0])
    } else {
        radians_from_deg(g.rotation)
    };
    let (u, v) = text_axes(rad, width, g.height);
    let x0 = match g.horizontal {
        1 | 3 => g.point[0] - 0.5 * u[0],
        2 => g.point[0] - u[0],
        _ => g.point[0],
    };
    let y0 = match g.horizontal {
        1 | 3 => g.point[1] - 0.5 * u[1],
        2 => g.point[1] - u[1],
        _ => g.point[1],
    };
    let mut bx = empty_box();
    for corner in [
        [x0, y0],
        [x0 + u[0], y0 + u[1]],
        [x0 + u[0] + v[0], y0 + u[1] + v[1]],
        [x0 + v[0], y0 + v[1]],
    ] {
        expand_pt(&mut bx, corner);
    }
    bx
}

fn write_text_geom(
    g: &TextGeom,
    pools: &Pools,
    style: Option<[u32; 2]>,
    w: &mut DisplayWriter<'_>,
) {
    let chars = pools.get_string(g.content).chars().count().max(1) as f64;
    let width = if g.horizontal >= 4 {
        libm::hypot(g.second[0] - g.point[0], g.second[1] - g.point[1]).max(g.height)
    } else {
        g.height * g.width_factor * 0.6 * chars
    };
    let rad = if g.horizontal >= 4 {
        libm::atan2(g.second[1] - g.point[1], g.second[0] - g.point[0])
    } else {
        radians_from_deg(g.rotation)
    };
    let (u, _) = text_axes(rad, width, g.height);
    let origin = match g.horizontal {
        1 | 3 => [g.point[0] - 0.5 * u[0], g.point[1] - 0.5 * u[1]],
        2 => [g.point[0] - u[0], g.point[1] - u[1]],
        _ => g.point,
    };
    let oblique_rad = radians_from_deg(g.oblique);
    match style {
        Some(st) => w.push_text_styled(origin, g.height, rad, g.width_factor, oblique_rad, st),
        None => w.push_text(origin, g.height, rad, g.width_factor, oblique_rad),
    }
}

fn model_length(len: &Length) -> f64 {
    match len {
        Length::Model(m) => m.value,
        Length::Paper(PaperLength(v)) => *v,
    }
}

fn find_symbol<'a>(symbols: &'a [SymbolDefinition], name: &str) -> Option<&'a SymbolDefinition> {
    symbols
        .iter()
        .find(|def| def.key.namespace == BLOCK && def.key.name.eq_ignore_ascii_case(name))
}

fn find_symbol_index(symbols: &[SymbolDefinition], name: &str) -> u32 {
    symbols
        .iter()
        .position(|def| def.key.namespace == BLOCK && def.key.name.eq_ignore_ascii_case(name))
        .map_or(NO_DEFINITION, |idx| idx as u32)
}

/// 任意の [`Shape<ModelPoint>`] の軸平行境界矩形を求める
pub fn shape_bounds(
    shape: &Shape<ModelPoint>,
    symbols: &[SymbolDefinition],
    depth: u8,
) -> [f64; 4] {
    let mut bx = empty_box();
    match shape {
        Shape::Line { start, end } => {
            expand_pt(&mut bx, [start.x, start.y]);
            expand_pt(&mut bx, [end.x, end.y]);
        }
        Shape::Point(p) => expand_pt(&mut bx, [p.position.x, p.position.y]),
        Shape::Curve(edge) => edge_bounds(edge, &mut bx),
        Shape::Circle { center, radius } => {
            let r = model_length(radius).abs();
            bx = [center.x - r, center.y - r, center.x + r, center.y + r];
        }
        Shape::Polyline(poly) => poly_vertices_bounds(&poly.vertices, poly.closed, &mut bx),
        Shape::Text(t) => model_text_bounds(t, &mut bx),
        Shape::Formatted(f) => formatted_bounds(f, &mut bx),
        Shape::Hatch(h) => hatch_bounds(h, &mut bx),
        Shape::Dimension(dim) => {
            if depth < MAX_SYMBOL_DEPTH
                && let Some(block_name) = &dim.block
                && let Some(def) = find_symbol(symbols, block_name)
            {
                for el in &def.space.elements {
                    merge_box(&mut bx, shape_bounds(&el.shape, symbols, depth + 1));
                }
            }
            if bx[0] > bx[2] {
                dimension_fallback_bounds(dim, &mut bx);
            }
        }
        Shape::Leader(leader) => leader_bounds(leader, &mut bx),
        Shape::Callout(callout) => {
            for branch in &callout.branches {
                for line in &branch.lines {
                    for p in line {
                        expand_pt(&mut bx, [p.x, p.y]);
                    }
                }
                if let Some(end) = branch.end {
                    expand_pt(&mut bx, [end.x, end.y]);
                }
            }
            match &callout.content {
                CalloutContent::Text(text) => formatted_bounds(text, &mut bx),
                CalloutContent::Symbol { symbol, .. } => {
                    expand_pt(&mut bx, [symbol.position.x, symbol.position.y]);
                }
                CalloutContent::None => {}
            }
        }
        Shape::Symbol(sym) => {
            for attr in &sym.attributes {
                if !attr.hidden {
                    model_text_bounds(&attr.text, &mut bx);
                }
            }
            if depth < MAX_SYMBOL_DEPTH
                && sym.key.namespace == BLOCK
                && let Some(def) = find_symbol(symbols, &sym.key.name)
            {
                let mut local = empty_box();
                for el in &def.space.elements {
                    merge_box(&mut local, shape_bounds(&el.shape, symbols, depth + 1));
                }
                if local[0] <= local[2] {
                    let rad = sym.rotation * (core::f64::consts::PI / 180.0);
                    let (s, c) = (libm::sin(rad), libm::cos(rad));
                    for corner in [
                        [local[0], local[1]],
                        [local[2], local[1]],
                        [local[2], local[3]],
                        [local[0], local[3]],
                    ] {
                        let sx = corner[0] * sym.scale[0];
                        let sy = corner[1] * sym.scale[1];
                        expand_pt(
                            &mut bx,
                            [
                                sym.position.x + sx * c - sy * s,
                                sym.position.y + sx * s + sy * c,
                            ],
                        );
                    }
                }
            }
            if bx[0] > bx[2] {
                expand_pt(&mut bx, [sym.position.x, sym.position.y]);
            }
        }
        Shape::Table(table) => {
            if depth < MAX_SYMBOL_DEPTH
                && let Some(key) = &table.drawn
                && key.namespace == BLOCK
                && let Some(def) = find_symbol(symbols, &key.name)
            {
                for el in &def.space.elements {
                    merge_box(&mut bx, shape_bounds(&el.shape, symbols, depth + 1));
                }
            }
            if bx[0] > bx[2] {
                table_fallback_bounds(table, &mut bx);
            }
        }
        Shape::Underlay(u) => underlay_bounds(u, &mut bx),
    }
    bx
}

fn poly_vertices_bounds(vertices: &[Vertex<ModelPoint>], closed: bool, bx: &mut [f64; 4]) {
    let n = vertices.len();
    let segs = if closed && n > 1 {
        n
    } else {
        n.saturating_sub(1)
    };
    if segs == 0 {
        if let Some(v) = vertices.first() {
            expand_pt(bx, [v.point.x, v.point.y]);
        }
    } else {
        for i in 0..segs {
            let Some(v0) = vertices.get(i) else {
                continue;
            };
            let Some(v1) = vertices.get(if i + 1 < n { i + 1 } else { 0 }) else {
                continue;
            };
            let p0 = [v0.point.x, v0.point.y];
            let p1 = [v1.point.x, v1.point.y];
            expand_pt(bx, p0);
            expand_pt(bx, p1);
            if let Some((c, r, start, sweep)) = bulge_arc_deg(p0, p1, v0.bulge) {
                let ab = if sweep >= 0.0 {
                    arc_bounds(c, r, start, start + sweep, true)
                } else {
                    arc_bounds(c, r, start + sweep, start, true)
                };
                merge_box(bx, ab);
            }
        }
    }
}

fn edge_bounds(edge: &Edge<ModelPoint>, bx: &mut [f64; 4]) {
    match edge {
        Edge::Line { start, end } => {
            expand_pt(bx, [start.x, start.y]);
            expand_pt(bx, [end.x, end.y]);
        }
        Edge::Arc {
            center,
            radius,
            start,
            end,
            counterclockwise,
        } => {
            merge_box(
                bx,
                arc_bounds(
                    [center.x, center.y],
                    model_length(radius).abs(),
                    *start,
                    *end,
                    *counterclockwise,
                ),
            );
        }
        Edge::Elliptic {
            center,
            major,
            ratio,
            start,
            end,
            counterclockwise,
        } => {
            let mut pts = Vec::new();
            let sw = zumen_geom2d::sweep(*start, *end, *counterclockwise);
            zumen_geom2d::ellipse(
                [center.x, center.y],
                [major.x, major.y],
                *ratio,
                *start,
                sw,
                DEFAULT_DISPLAY_TOLERANCE,
                &mut pts,
            );
            for p in pts {
                expand_pt(bx, p);
            }
        }
        Edge::Spline(sp) => {
            let control: Vec<[f64; 2]> = sp.control.iter().map(|p| [p.x, p.y]).collect();
            if let Some(pts) = zumen_geom2d::spline(
                sp.degree,
                &sp.knots,
                &control,
                &sp.weights,
                DEFAULT_DISPLAY_TOLERANCE,
            ) {
                for p in pts {
                    expand_pt(bx, p);
                }
            } else {
                for p in &sp.control {
                    expand_pt(bx, [p.x, p.y]);
                }
                for p in &sp.fit {
                    expand_pt(bx, [p.x, p.y]);
                }
            }
        }
        Edge::Clothoid { origin, .. } => {
            expand_pt(bx, [origin.x, origin.y]);
        }
    }
}

fn model_text_bounds(t: &Text<ModelPoint>, bx: &mut [f64; 4]) {
    let h = model_length(&t.height.value).max(0.0);
    let chars = t.content.chars().count().max(1) as f64;
    match t.placement {
        Placement::Anchor { point, .. } => {
            let w = h * t.width_factor * 0.6 * chars;
            let rad = radians_from_deg(t.rotation);
            let (u, v) = text_axes(rad, w, h);
            for corner in [
                [point.x, point.y],
                [point.x + u[0], point.y + u[1]],
                [point.x + u[0] + v[0], point.y + u[1] + v[1]],
                [point.x + v[0], point.y + v[1]],
            ] {
                expand_pt(bx, corner);
            }
        }
        Placement::Between { first, second, .. } => {
            expand_pt(bx, [first.x, first.y]);
            expand_pt(bx, [second.x, second.y]);
        }
    }
}

fn formatted_rotation_rad(rot: zumen_drawing::Rotation) -> f64 {
    match rot {
        zumen_drawing::Rotation::Angle(deg) => radians_from_deg(deg),
        zumen_drawing::Rotation::Direction([dx, dy]) => libm::atan2(dy, dx),
    }
}

fn formatted_bounds(f: &FormattedText<ModelPoint>, bx: &mut [f64; 4]) {
    let h = model_length(&f.height.value).max(0.0);
    let w = f
        .width
        .as_ref()
        .map_or(h * 4.0, |len| model_length(len).max(h));
    let rad = formatted_rotation_rad(f.rotation);
    let (u, v) = text_axes(rad, w, h);
    let p = [f.point.x, f.point.y];
    for corner in [
        p,
        [p[0] + u[0], p[1] + u[1]],
        [p[0] + u[0] + v[0], p[1] + u[1] + v[1]],
        [p[0] + v[0], p[1] + v[1]],
    ] {
        expand_pt(bx, corner);
    }
}

fn hatch_bounds(h: &Hatch<ModelPoint>, bx: &mut [f64; 4]) {
    for lp in &h.loops {
        match &lp.boundary {
            zumen_drawing::Boundary::Polyline { vertices, closed } => {
                poly_vertices_bounds(vertices, *closed, bx);
            }
            zumen_drawing::Boundary::Edges(loop_edges) => {
                for e in loop_edges {
                    edge_bounds(e, bx);
                }
            }
        }
    }
}

fn dimension_fallback_point(dim: &zumen_drawing::Dimension<ModelPoint>) -> [f64; 2] {
    if let Some(tp) = dim.text.position {
        return [tp.x, tp.y];
    }
    match &dim.kind {
        zumen_drawing::DimensionKind::Linear { line, .. }
        | zumen_drawing::DimensionKind::Aligned { line, .. } => [line.x, line.y],
        zumen_drawing::DimensionKind::Angular { arc, .. }
        | zumen_drawing::DimensionKind::AngularLines { arc, .. }
        | zumen_drawing::DimensionKind::ArcLength { arc, .. } => [arc.x, arc.y],
        zumen_drawing::DimensionKind::Radius { chord, .. }
        | zumen_drawing::DimensionKind::Diameter { chord, .. } => [chord.x, chord.y],
        zumen_drawing::DimensionKind::Ordinate { leader_end, .. } => [leader_end.x, leader_end.y],
    }
}

fn dimension_fallback_bounds(dim: &zumen_drawing::Dimension<ModelPoint>, bx: &mut [f64; 4]) {
    if let Some(tp) = dim.text.position {
        expand_pt(bx, [tp.x, tp.y]);
    }
    match &dim.kind {
        zumen_drawing::DimensionKind::Linear {
            first,
            second,
            line,
            ..
        }
        | zumen_drawing::DimensionKind::Aligned {
            first,
            second,
            line,
            ..
        } => {
            expand_pt(bx, [first.x, first.y]);
            expand_pt(bx, [second.x, second.y]);
            expand_pt(bx, [line.x, line.y]);
        }
        zumen_drawing::DimensionKind::Angular {
            vertex,
            first,
            second,
            arc,
        }
        | zumen_drawing::DimensionKind::ArcLength {
            center: vertex,
            first,
            second,
            arc,
        } => {
            expand_pt(bx, [vertex.x, vertex.y]);
            expand_pt(bx, [first.x, first.y]);
            expand_pt(bx, [second.x, second.y]);
            expand_pt(bx, [arc.x, arc.y]);
        }
        zumen_drawing::DimensionKind::AngularLines { first, second, arc } => {
            for p in [first[0], first[1], second[0], second[1], *arc] {
                expand_pt(bx, [p.x, p.y]);
            }
        }
        zumen_drawing::DimensionKind::Radius { center, chord, .. } => {
            expand_pt(bx, [center.x, center.y]);
            expand_pt(bx, [chord.x, chord.y]);
        }
        zumen_drawing::DimensionKind::Diameter {
            chord, opposite, ..
        } => {
            expand_pt(bx, [chord.x, chord.y]);
            expand_pt(bx, [opposite.x, opposite.y]);
        }
        zumen_drawing::DimensionKind::Ordinate {
            origin,
            feature,
            leader_end,
            ..
        } => {
            expand_pt(bx, [origin.x, origin.y]);
            expand_pt(bx, [feature.x, feature.y]);
            expand_pt(bx, [leader_end.x, leader_end.y]);
        }
    }
}

fn leader_bounds(leader: &Leader<ModelPoint>, bx: &mut [f64; 4]) {
    for p in &leader.path {
        expand_pt(bx, [p.x, p.y]);
    }
}

fn table_fallback_bounds(table: &Table<ModelPoint>, bx: &mut [f64; 4]) {
    let w: f64 = table.column_widths.iter().map(model_length).sum();
    let h: f64 = table.row_heights.iter().map(model_length).sum();
    let [dx, dy] = table.direction;
    let p = [table.position.x, table.position.y];
    expand_pt(bx, p);
    expand_pt(bx, [p[0] + dx * w, p[1] + dy * w]);
    expand_pt(bx, [p[0] + dy * h, p[1] - dx * h]);
    expand_pt(bx, [p[0] + dx * w + dy * h, p[1] + dy * w - dx * h]);
}

fn underlay_corners(u: &Underlay<ModelPoint>) -> [[f64; 2]; 4] {
    match &u.frame {
        zumen_drawing::UnderlayFrame::Pixels {
            origin, right, top, ..
        }
        | zumen_drawing::UnderlayFrame::Corners { origin, right, top } => {
            let p0 = [origin.x, origin.y];
            let p1 = [right.x, right.y];
            let p3 = [top.x, top.y];
            let p2 = [p1[0] + p3[0] - p0[0], p1[1] + p3[1] - p0[1]];
            [p0, p1, p2, p3]
        }
        zumen_drawing::UnderlayFrame::Scaled {
            origin,
            scale,
            rotation,
        } => {
            let sx = model_length(&scale[0]);
            let sy = model_length(&scale[1]);
            let rad = radians_from_deg(*rotation);
            let (s, c) = (libm::sin(rad), libm::cos(rad));
            let p0 = [origin.x, origin.y];
            let ux = [sx * c, sx * s];
            let vy = [-sy * s, sy * c];
            [
                p0,
                [p0[0] + ux[0], p0[1] + ux[1]],
                [p0[0] + ux[0] + vy[0], p0[1] + ux[1] + vy[1]],
                [p0[0] + vy[0], p0[1] + vy[1]],
            ]
        }
    }
}

fn underlay_bounds(u: &Underlay<ModelPoint>, bx: &mut [f64; 4]) {
    for corner in underlay_corners(u) {
        expand_pt(bx, corner);
    }
}

fn tessellate_edge_pts(edge: &Edge<ModelPoint>) -> Option<Vec<[f64; 2]>> {
    match edge {
        Edge::Elliptic {
            center,
            major,
            ratio,
            start,
            end,
            counterclockwise,
        } => {
            let mut pts = Vec::new();
            let sw = zumen_geom2d::sweep(*start, *end, *counterclockwise);
            zumen_geom2d::ellipse(
                [center.x, center.y],
                [major.x, major.y],
                *ratio,
                *start,
                sw,
                DEFAULT_DISPLAY_TOLERANCE,
                &mut pts,
            );
            Some(pts)
        }
        Edge::Spline(sp) => {
            let control: Vec<[f64; 2]> = sp.control.iter().map(|p| [p.x, p.y]).collect();
            zumen_geom2d::spline(
                sp.degree,
                &sp.knots,
                &control,
                &sp.weights,
                DEFAULT_DISPLAY_TOLERANCE,
            )
        }
        _ => None,
    }
}

fn add_counts(dst: &mut [u32; 4], src: [u32; 4]) {
    for (d, s) in dst.iter_mut().zip(src) {
        *d = d.saturating_add(s);
    }
}

fn poly_vertices_display_counts(vertices: &[Vertex<ModelPoint>], closed: bool) -> [u32; 4] {
    let mut c = [0u32; 4];
    let n = vertices.len();
    let segs = if closed && n > 1 {
        n
    } else {
        n.saturating_sub(1)
    };
    for i in 0..segs {
        let Some(v0) = vertices.get(i) else {
            continue;
        };
        let Some(v1) = vertices.get(if i + 1 < n { i + 1 } else { 0 }) else {
            continue;
        };
        if bulge_arc_deg([v0.point.x, v0.point.y], [v1.point.x, v1.point.y], v0.bulge).is_some() {
            c[1] = c[1].saturating_add(1);
        } else {
            c[0] = c[0].saturating_add(1);
        }
    }
    c
}

fn shape_display_counts(
    shape: &Shape<ModelPoint>,
    symbols: &[SymbolDefinition],
    depth: u8,
) -> [u32; 4] {
    let mut c = [0u32; 4];
    match shape {
        Shape::Line { .. } => c[0] = 1,
        Shape::Point(_) => c[0] = 2,
        Shape::Curve(edge) => add_edge_counts(edge, true, &mut c),
        Shape::Circle { .. } => c[1] = 1,
        Shape::Polyline(poly) => {
            c = poly_vertices_display_counts(&poly.vertices, poly.closed);
        }
        Shape::Text(_) | Shape::Formatted(_) => c[2] = 1,
        Shape::Hatch(h) => {
            for lp in &h.loops {
                match &lp.boundary {
                    zumen_drawing::Boundary::Polyline { vertices, closed } => {
                        add_counts(&mut c, poly_vertices_display_counts(vertices, *closed));
                    }
                    zumen_drawing::Boundary::Edges(loop_edges) => {
                        for e in loop_edges {
                            add_edge_counts(e, true, &mut c);
                        }
                    }
                }
            }
        }
        Shape::Dimension(dim) => {
            if depth < MAX_SYMBOL_DEPTH
                && let Some(block_name) = &dim.block
                && let Some(def) = find_symbol(symbols, block_name)
                && !def.space.elements.is_empty()
            {
                for el in &def.space.elements {
                    add_counts(&mut c, shape_display_counts(&el.shape, symbols, depth + 1));
                }
            }
            if c == [0; 4] {
                c[2] = 1;
            }
        }
        Shape::Leader(leader) => {
            c[0] = leader.path.len().saturating_sub(1) as u32;
        }
        Shape::Callout(callout) => {
            for branch in &callout.branches {
                for line in &branch.lines {
                    c[0] = c[0].saturating_add(line.len().saturating_sub(1) as u32);
                    if branch.end.is_some() && !line.is_empty() {
                        c[0] = c[0].saturating_add(1);
                    }
                }
            }
            if matches!(callout.content, CalloutContent::Text(_)) {
                c[2] = c[2].saturating_add(1);
            }
        }
        Shape::Symbol(sym) => {
            for attr in &sym.attributes {
                if !attr.hidden {
                    c[2] = c[2].saturating_add(1);
                }
            }
            if depth == 0 {
                c[3] = c[3].saturating_add(1);
            } else if depth < MAX_SYMBOL_DEPTH
                && sym.key.namespace == BLOCK
                && let Some(def) = find_symbol(symbols, &sym.key.name)
            {
                let isotropic =
                    (sym.scale[0].abs() - sym.scale[1].abs()).abs() <= sym.scale[0].abs() * 1e-6;
                for el in &def.space.elements {
                    let mut sub = shape_display_counts(&el.shape, symbols, depth + 1);
                    if !isotropic && sub[1] > 0 {
                        sub[0] = sub[0].saturating_add(sub[1].saturating_mul(16));
                        sub[1] = 0;
                    }
                    add_counts(&mut c, sub);
                }
            }
        }
        Shape::Table(table) => {
            if depth < MAX_SYMBOL_DEPTH
                && let Some(key) = &table.drawn
                && key.namespace == BLOCK
                && let Some(def) = find_symbol(symbols, &key.name)
                && !def.space.elements.is_empty()
            {
                for el in &def.space.elements {
                    add_counts(&mut c, shape_display_counts(&el.shape, symbols, depth + 1));
                }
            }
            if c == [0; 4] {
                c[0] = 4;
            }
        }
        Shape::Underlay(_) => c[0] = 4,
    }
    c
}

fn add_edge_counts(edge: &Edge<ModelPoint>, isotropic: bool, c: &mut [u32; 4]) {
    match edge {
        Edge::Line { .. } | Edge::Clothoid { .. } => c[0] = c[0].saturating_add(1),
        Edge::Arc { .. } => {
            if isotropic {
                c[1] = c[1].saturating_add(1);
            } else {
                c[0] = c[0].saturating_add(16);
            }
        }
        Edge::Elliptic { .. } | Edge::Spline(_) => {
            let segs =
                tessellate_edge_pts(edge).map_or(0, |pts| pts.len().saturating_sub(1) as u32);
            c[0] = c[0].saturating_add(segs);
        }
    }
}

fn write_poly_vertices_display(
    vertices: &[Vertex<ModelPoint>],
    closed: bool,
    xf: &Xform,
    w: &mut DisplayWriter<'_>,
) {
    let n = vertices.len();
    let segs = if closed && n > 1 {
        n
    } else {
        n.saturating_sub(1)
    };
    let sim = xf.similarity();
    for i in 0..segs {
        let Some(v0) = vertices.get(i) else {
            continue;
        };
        let Some(v1) = vertices.get(if i + 1 < n { i + 1 } else { 0 }) else {
            continue;
        };
        let p0 = [v0.point.x, v0.point.y];
        let p1 = [v1.point.x, v1.point.y];
        if let Some((c, r, start, sweep)) = bulge_arc_deg(p0, p1, v0.bulge) {
            if let Some((scale, rot, mirrored)) = sim {
                let new_start = if mirrored { -start + rot } else { start + rot };
                let new_sweep = if mirrored { -sweep } else { sweep };
                w.push_arc_rad(
                    xf.apply(c),
                    r * scale,
                    radians_from_deg(new_start),
                    radians_from_deg(new_sweep),
                );
            } else {
                write_approx_arc(c, r, start, sweep, xf, w);
            }
        } else {
            w.push_segment(xf.apply(p0), xf.apply(p1));
        }
    }
}

fn write_shape_display(
    shape: &Shape<ModelPoint>,
    symbols: &[SymbolDefinition],
    xf: &Xform,
    depth: u8,
    w: &mut DisplayWriter<'_>,
) {
    match shape {
        Shape::Line { start, end } => {
            w.push_segment(xf.apply([start.x, start.y]), xf.apply([end.x, end.y]));
        }
        Shape::Point(p) => {
            let c = xf.apply([p.position.x, p.position.y]);
            w.push_segment([c[0] - 0.5, c[1]], [c[0] + 0.5, c[1]]);
            w.push_segment([c[0], c[1] - 0.5], [c[0], c[1] + 0.5]);
        }
        Shape::Curve(edge) => write_edge_display(edge, xf, w),
        Shape::Circle { center, radius } => {
            let r = model_length(radius).abs();
            if let Some((scale, _, _)) = xf.similarity() {
                w.push_circle(xf.apply([center.x, center.y]), r * scale);
            } else {
                write_approx_arc([center.x, center.y], r, 0.0, 360.0, xf, w);
            }
        }
        Shape::Polyline(poly) => write_poly_vertices_display(&poly.vertices, poly.closed, xf, w),
        Shape::Text(t) => write_model_text_display(t, xf, None, w),
        Shape::Formatted(f) => {
            let h = model_length(&f.height.value).max(0.1);
            let rad = formatted_rotation_rad(f.rotation);
            let p0 = xf.apply([f.point.x, f.point.y]);
            let px = xf.apply([f.point.x + libm::cos(rad), f.point.y + libm::sin(rad)]);
            let rot = libm::atan2(px[1] - p0[1], px[0] - p0[0]);
            let scale = libm::hypot(px[0] - p0[0], px[1] - p0[1]).max(1e-6);
            w.push_text(p0, h * scale, rot, 1.0, 0.0);
        }
        Shape::Hatch(h) => {
            for lp in &h.loops {
                match &lp.boundary {
                    zumen_drawing::Boundary::Polyline { vertices, closed } => {
                        write_poly_vertices_display(vertices, *closed, xf, w);
                    }
                    zumen_drawing::Boundary::Edges(loop_edges) => {
                        for e in loop_edges {
                            write_edge_display(e, xf, w);
                        }
                    }
                }
            }
        }
        Shape::Dimension(dim) => {
            let mut before = false;
            if depth < MAX_SYMBOL_DEPTH
                && let Some(block_name) = &dim.block
                && let Some(def) = find_symbol(symbols, block_name)
                && !def.space.elements.is_empty()
            {
                let mut sub_c = [0u32; 4];
                for el in &def.space.elements {
                    add_counts(
                        &mut sub_c,
                        shape_display_counts(&el.shape, symbols, depth + 1),
                    );
                }
                if sub_c != [0; 4] {
                    for el in &def.space.elements {
                        write_shape_display(&el.shape, symbols, xf, depth + 1, w);
                    }
                    before = true;
                }
            }
            if !before {
                let p = xf.apply(dimension_fallback_point(dim));
                w.push_text(p, 1.0, 0.0, 1.0, 0.0);
            }
        }
        Shape::Leader(leader) => {
            for pair in leader.path.windows(2) {
                if let [a, b] = pair {
                    w.push_segment(xf.apply([a.x, a.y]), xf.apply([b.x, b.y]));
                }
            }
        }
        Shape::Callout(callout) => {
            for branch in &callout.branches {
                for line in &branch.lines {
                    for pair in line.windows(2) {
                        if let [a, b] = pair {
                            w.push_segment(xf.apply([a.x, a.y]), xf.apply([b.x, b.y]));
                        }
                    }
                    if let (Some(last), Some(end)) = (line.last(), branch.end) {
                        w.push_segment(xf.apply([last.x, last.y]), xf.apply([end.x, end.y]));
                    }
                }
            }
            if let CalloutContent::Text(text) = &callout.content {
                write_shape_display(&Shape::Formatted((**text).clone()), symbols, xf, depth, w);
            }
        }
        Shape::Symbol(sym) => {
            for attr in &sym.attributes {
                if !attr.hidden {
                    let st = (depth == 0).then(|| [attr.layer.0, pack_color(attr.style.color)]);
                    write_model_text_display(&attr.text, xf, st, w);
                }
            }
            if depth == 0 {
                let def_idx = if sym.key.namespace == BLOCK {
                    find_symbol_index(symbols, &sym.key.name)
                } else {
                    NO_DEFINITION
                };
                let grid = sym
                    .grid
                    .map_or([1, 1], |gr| [gr.columns.max(1), gr.rows.max(1)]);
                let spacing = sym.grid.map_or([0.0, 0.0], |gr| {
                    [
                        model_length(&gr.column_spacing),
                        model_length(&gr.row_spacing),
                    ]
                });
                w.push_instance(
                    [sym.position.x, sym.position.y],
                    sym.scale,
                    radians_from_deg(sym.rotation),
                    spacing,
                    def_idx,
                    grid,
                );
            } else if depth < MAX_SYMBOL_DEPTH
                && sym.key.namespace == BLOCK
                && let Some(def) = find_symbol(symbols, &sym.key.name)
            {
                let rad = radians_from_deg(sym.rotation);
                let (s, c) = (libm::sin(rad), libm::cos(rad));
                let local = Xform {
                    m: [
                        sym.scale[0] * c,
                        -sym.scale[1] * s,
                        sym.position.x,
                        sym.scale[0] * s,
                        sym.scale[1] * c,
                        sym.position.y,
                    ],
                };
                let next_xf = xf.compose(&local);
                for el in &def.space.elements {
                    write_shape_display(&el.shape, symbols, &next_xf, depth + 1, w);
                }
            }
        }
        Shape::Table(table) => {
            let mut emitted = false;
            if depth < MAX_SYMBOL_DEPTH
                && let Some(key) = &table.drawn
                && key.namespace == BLOCK
                && let Some(def) = find_symbol(symbols, &key.name)
                && !def.space.elements.is_empty()
            {
                let mut sub_c = [0u32; 4];
                for el in &def.space.elements {
                    add_counts(
                        &mut sub_c,
                        shape_display_counts(&el.shape, symbols, depth + 1),
                    );
                }
                if sub_c != [0; 4] {
                    for el in &def.space.elements {
                        write_shape_display(&el.shape, symbols, xf, depth + 1, w);
                    }
                    emitted = true;
                }
            }
            if !emitted {
                let width: f64 = table.column_widths.iter().map(model_length).sum();
                let height: f64 = table.row_heights.iter().map(model_length).sum();
                let [dx, dy] = table.direction;
                let p0 = [table.position.x, table.position.y];
                let p1 = [p0[0] + dx * width, p0[1] + dy * width];
                let p2 = [p1[0] + dy * height, p1[1] - dx * height];
                let p3 = [p0[0] + dy * height, p0[1] - dx * height];
                for [a, b] in [[p0, p1], [p1, p2], [p2, p3], [p3, p0]] {
                    w.push_segment(xf.apply(a), xf.apply(b));
                }
            }
        }
        Shape::Underlay(u) => {
            let [p0, p1, p2, p3] = underlay_corners(u);
            for [a, b] in [[p0, p1], [p1, p2], [p2, p3], [p3, p0]] {
                w.push_segment(xf.apply(a), xf.apply(b));
            }
        }
    }
}

fn write_edge_display(edge: &Edge<ModelPoint>, xf: &Xform, w: &mut DisplayWriter<'_>) {
    match edge {
        Edge::Line { start, end } => {
            w.push_segment(xf.apply([start.x, start.y]), xf.apply([end.x, end.y]));
        }
        Edge::Arc {
            center,
            radius,
            start,
            end,
            counterclockwise,
        } => {
            let r = model_length(radius).abs();
            let sw = zumen_geom2d::sweep(*start, *end, *counterclockwise);
            if let Some((scale, rot, mirrored)) = xf.similarity() {
                let new_start = if mirrored {
                    -*start + rot
                } else {
                    *start + rot
                };
                let new_sw = if mirrored { -sw } else { sw };
                w.push_arc_rad(
                    xf.apply([center.x, center.y]),
                    r * scale,
                    radians_from_deg(new_start),
                    radians_from_deg(new_sw),
                );
            } else {
                write_approx_arc([center.x, center.y], r, *start, sw, xf, w);
            }
        }
        Edge::Elliptic { .. } | Edge::Spline(_) => {
            if let Some(pts) = tessellate_edge_pts(edge) {
                for pair in pts.windows(2) {
                    if let [a, b] = pair {
                        w.push_segment(xf.apply(*a), xf.apply(*b));
                    }
                }
            }
        }
        Edge::Clothoid { origin, .. } => {
            let p = xf.apply([origin.x, origin.y]);
            w.push_segment(p, p);
        }
    }
}

fn write_approx_arc(
    center: [f64; 2],
    radius: f64,
    start: f64,
    sweep: f64,
    xf: &Xform,
    w: &mut DisplayWriter<'_>,
) {
    let a0 = radians_from_deg(start);
    let da = radians_from_deg(sweep) / 16.0;
    let mut prev = xf.apply([
        center[0] + radius * libm::cos(a0),
        center[1] + radius * libm::sin(a0),
    ]);
    for step in 1..=16 {
        let a = a0 + da * f64::from(step);
        let cur = xf.apply([
            center[0] + radius * libm::cos(a),
            center[1] + radius * libm::sin(a),
        ]);
        w.push_segment(prev, cur);
        prev = cur;
    }
}

fn write_model_text_display(
    t: &Text<ModelPoint>,
    xf: &Xform,
    style: Option<[u32; 2]>,
    w: &mut DisplayWriter<'_>,
) {
    let h = model_length(&t.height.value).max(0.1);
    let chars = t.content.chars().count().max(1) as f64;
    let (origin, rad) = match t.placement {
        Placement::Anchor {
            point, horizontal, ..
        } => {
            let width = h * t.width_factor * 0.6 * chars;
            let rad = radians_from_deg(t.rotation);
            let (u, _) = text_axes(rad, width, h);
            let orig = match horizontal {
                Horizontal::Center | Horizontal::Middle => {
                    [point.x - 0.5 * u[0], point.y - 0.5 * u[1]]
                }
                Horizontal::Right => [point.x - u[0], point.y - u[1]],
                Horizontal::Left => [point.x, point.y],
            };
            (orig, rad)
        }
        Placement::Between { first, second, .. } => {
            let rad = libm::atan2(second.y - first.y, second.x - first.x);
            ([first.x, first.y], rad)
        }
    };
    let p0 = xf.apply(origin);
    let px = xf.apply([origin[0] + libm::cos(rad), origin[1] + libm::sin(rad)]);
    let rot = libm::atan2(px[1] - p0[1], px[0] - p0[0]);
    let scale = libm::hypot(px[0] - p0[0], px[1] - p0[1]).max(1e-6);
    let oblique_rad = radians_from_deg(t.oblique);
    match style {
        Some(st) => w.push_text_styled(p0, h * scale, rot, t.width_factor, oblique_rad, st),
        None => w.push_text(p0, h * scale, rot, t.width_factor, oblique_rad),
    }
}

/// [`GeneralRow`] を作るときに要素の `layer`・`style.line_type` を [`SlotCatalog`] のスロット番号に差し替える
#[allow(clippy::too_many_arguments)]
pub fn general_from_element(
    mut element: Element<ModelPoint>,
    raw_layers: &[Layer],
    raw_line_types: &[LineType],
    catalog: &mut SlotCatalog,
    dxf_kind: &[u8],
    follower_records: Vec<u32>,
    raw_attrib_count: u32,
    drafts: Vec<Draft>,
) -> GeneralRow {
    let map_layer = |id: LayerId, catalog: &mut SlotCatalog| -> u32 {
        let name = raw_layers
            .get(id.0 as usize)
            .map_or("0", |l| l.name.as_str());
        let slot = catalog.intern_layer(name);
        if let Some(entry) = catalog.layers.get_mut(slot as usize) {
            entry.model_refs = entry.model_refs.saturating_add(1);
        }
        slot
    };
    let map_lt = |inh: Inherit<LineTypeId>, catalog: &mut SlotCatalog| -> LineTypeSlotRef {
        match inh {
            Inherit::Layer => LineTypeSlotRef::Layer,
            Inherit::Block => LineTypeSlotRef::Block,
            Inherit::Own(id) => {
                if let Some(lt) = raw_line_types.get(id.0 as usize) {
                    let slot = catalog.intern_line_type(&lt.name);
                    if let Some(entry) = catalog.line_types.get_mut(slot as usize) {
                        entry.model_refs = entry.model_refs.saturating_add(1);
                    }
                    LineTypeSlotRef::Own(slot)
                } else {
                    LineTypeSlotRef::Layer
                }
            }
        }
    };
    let layer_slot = map_layer(element.layer, catalog);
    element.layer = LayerId(layer_slot);
    if let LineTypeSlotRef::Own(s) = map_lt(element.style.line_type, catalog) {
        element.style.line_type = Inherit::Own(LineTypeId(s));
    }
    let mut attrib_slots = Vec::new();
    if let Shape::Symbol(sym) = &mut element.shape {
        attrib_slots.reserve(sym.attributes.len());
        for attr in &mut sym.attributes {
            let a_layer = map_layer(attr.layer, catalog);
            let a_lt = map_lt(attr.style.line_type, catalog);
            attr.layer = LayerId(a_layer);
            if let LineTypeSlotRef::Own(s) = a_lt {
                attr.style.line_type = Inherit::Own(LineTypeId(s));
            }
            attrib_slots.push((a_layer, a_lt));
        }
    }
    let mut callout_line_type_slot = LineTypeSlotRef::Layer;
    if let Shape::Callout(callout) = &mut element.shape {
        callout_line_type_slot = map_lt(callout.line_style.line_type, catalog);
        if let LineTypeSlotRef::Own(s) = callout_line_type_slot {
            callout.line_style.line_type = Inherit::Own(LineTypeId(s));
        }
    }
    GeneralRow {
        element,
        attrib_slots,
        callout_line_type_slot,
        dxf_kind: dxf_kind.into(),
        follower_records: follower_records.into_boxed_slice(),
        raw_attrib_count,
        drafts,
    }
}
