//! 図形のレコードを、列に置く値へ持ち上げる。要素になる条件は、`zumen_acad::single` の共通関数をそのまま呼んで
//! 一括変換の読み方（`crates/zumen-acad/src/drawing`）と 1 つの実装に揃える。
//! 開くときも、変更のあとで読み直すときも、ここを通る。

use zumen_acad::Charset;
use zumen_acad::single::{
    self, DxfFields, FieldSource as _, LiftFail, PolylineSink, RawLineType, RawTextPlace,
};
pub use zumen_acad::single::{Follow, Followed, RawTag};
use zumen_base::{Key, Ref};
use zumen_drawing::{
    Attribute, Clip, Color, ColorCode, ColorSystem, Edge, Element, Fit, FormattedText, Grid,
    Height, HeightMeaning, Horizontal, Inherit, LayerId, Length, LineTypeId, LineWeight,
    ModelLength, ModelPoint, Placement, Polyline, Shape, SpaceId, Style, Symbol, SymbolKey, Text,
    Transparency, Vertex, Vertical,
};

use crate::paged::{BytePool, Paged, TextSpan};

/// この試作が要素として持つ図形の種類
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u8)]
pub enum Kind {
    /// LINE
    Line = 0,
    /// CIRCLE
    Circle = 1,
    /// ARC
    Arc = 2,
    /// LWPOLYLINE
    Polyline = 3,
    /// TEXT
    Text = 4,
    /// INSERT（ブロックの挿入）。付いた ATTRIB は、この行の属性として持つ
    Insert = 5,
}

impl Kind {
    /// すべての種類
    pub const ALL: [Kind; 6] = [
        Kind::Line,
        Kind::Circle,
        Kind::Arc,
        Kind::Polyline,
        Kind::Text,
        Kind::Insert,
    ];

    /// レコードの種類（グループ・コード 0 の値。前後の空白を除いたもの）から
    pub fn of(name: &[u8]) -> Option<Kind> {
        Some(match name {
            b"LINE" => Kind::Line,
            b"CIRCLE" => Kind::Circle,
            b"ARC" => Kind::Arc,
            b"LWPOLYLINE" => Kind::Polyline,
            b"TEXT" => Kind::Text,
            b"INSERT" => Kind::Insert,
            _ => return None,
        })
    }

    /// DXF のグループ・コード 0 の種類名
    pub fn dxf_name(self) -> &'static [u8] {
        match self {
            Kind::Line => b"LINE",
            Kind::Circle => b"CIRCLE",
            Kind::Arc => b"ARC",
            Kind::Polyline => b"LWPOLYLINE",
            Kind::Text => b"TEXT",
            Kind::Insert => b"INSERT",
        }
    }
}

/// 折れ線の閉じる指定（70 の 1）
pub const POLY_CLOSED: u8 = 1;
/// 折れ線の線種の生成（70 の 128）
pub const POLY_GENERATION: u8 = 2;
/// 折れ線が頂点ごとの幅を持つ
pub const POLY_WIDTHS: u8 = 4;

/// 文字の置き方
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Place {
    /// 1 点に揃える
    Anchor {
        /// 揃える点
        point: [f64; 2],
        /// 横の揃え
        horizontal: Horizontal,
        /// 縦の揃え
        vertical: Vertical,
    },
    /// 2 点の間に収める
    Between {
        /// 1 点め
        first: [f64; 2],
        /// 2 点め
        second: [f64; 2],
        /// 収め方
        fit: Fit,
    },
}

/// 文字の左右を反転（71 の 2）
pub const TEXT_BACKWARD: u8 = 1;
/// 文字の上下を反転（71 の 4）
pub const TEXT_UPSIDE_DOWN: u8 = 2;
/// 属性を表示しない（ATTRIB の 70 の 1）
pub const ATTRIB_HIDDEN: u8 = 4;
/// 属性が自身のハンドルを持つ（ATTRIB の 5）
pub const ATTRIB_HAS_HANDLE: u8 = 8;
/// 属性が複数行の書式（101）を持つ
pub const ATTRIB_FORMATTED: u8 = 16;

/// 文字のまれな指定（幅の係数が 1 でない、傾きが 0 でない、または 2 点の間に置く）
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextExtra {
    /// 2 点め（`Between` のとき）
    pub second: [f64; 2],
    /// 幅の係数（41）
    pub width_factor: f64,
    /// 傾き（51、度）
    pub oblique: f64,
}

/// 文字の行（56 バイト）
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextRow {
    /// 置く点（`Anchor` の点、または `Between` の 1 点め）
    pub point: [f64; 2],
    /// 高さ（40）
    pub height: f64,
    /// 回転（50、度）
    pub rotation: f64,
    /// 中身（1）。UTF-8
    pub content: TextSpan,
    /// 文字スタイルの名前（7）。UTF-8（既定は `TextSpan::STANDARD`）
    pub style: TextSpan,
    /// `Pools::text_extras` の 1 始まりの番号（0 なら既定）
    pub extra: u32,
    /// 横の揃え（0..=3: Left/Center/Right/Middle、4: Aligned、5: Fit）
    pub h_align: u8,
    /// 縦の揃え（0..=3: Baseline/Bottom/Middle/Top）
    pub v_align: u8,
    /// `TEXT_*` / `ATTRIB_*`
    pub flags: u8,
    /// パディング
    pub pad: u8,
}

impl TextRow {
    /// 置き方を復元する
    pub fn place(&self, pools: &Pools) -> Place {
        match self.h_align {
            4 | 5 => {
                let second = self
                    .extra
                    .checked_sub(1)
                    .and_then(|at| pools.text_extras.get(at as usize))
                    .map_or(self.point, |extra| extra.second);
                Place::Between {
                    first: self.point,
                    second,
                    fit: if self.h_align == 5 {
                        Fit::Fit
                    } else {
                        Fit::Aligned
                    },
                }
            }
            h => {
                let horizontal = match h {
                    1 => Horizontal::Center,
                    2 => Horizontal::Right,
                    3 => Horizontal::Middle,
                    _ => Horizontal::Left,
                };
                let vertical = match self.v_align {
                    1 => Vertical::Bottom,
                    2 => Vertical::Middle,
                    3 => Vertical::Top,
                    _ => Vertical::Baseline,
                };
                Place::Anchor {
                    point: self.point,
                    horizontal,
                    vertical,
                }
            }
        }
    }

    /// 幅の係数（41）
    #[inline]
    pub fn width_factor(&self, pools: &Pools) -> f64 {
        match self.extra.checked_sub(1) {
            None => 1.0,
            Some(at) => pools
                .text_extras
                .get(at as usize)
                .map_or(1.0, |extra| extra.width_factor),
        }
    }

    /// 傾き（51、度）
    #[inline]
    pub fn oblique(&self, pools: &Pools) -> f64 {
        match self.extra.checked_sub(1) {
            None => 0.0,
            Some(at) => pools
                .text_extras
                .get(at as usize)
                .map_or(0.0, |extra| extra.oblique),
        }
    }

    /// 別の `BytePool` を繋いだときに文字列の位置をずらす
    #[inline]
    pub const fn shifted_spans(self, text_base: u32, text_extra_base: u32) -> Self {
        Self {
            content: self.content.shifted(text_base),
            style: self.style.shifted(text_base),
            extra: if self.extra == 0 {
                0
            } else {
                self.extra + text_extra_base
            },
            ..self
        }
    }
}

/// 定義が見つからない記号の、定義の番号
pub const NO_DEFINITION: u32 = u32::MAX;

/// INSERT の 66 が 0 でない整数であること
pub const INSERT_FOLLOWED: u8 = 1;

/// INSERT のまれな指定（MINSERT の格子または XCLIP の切り抜き）
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InsertExtra {
    /// 列と行の数（70・71）
    pub grid: [u32; 2],
    /// 列と行の間隔（44・45）
    pub spacing: [f64; 2],
    /// `Pools::clips` の 1 始まりの番号（0 なら切り抜きなし）
    pub clip: u32,
}

/// 記号（INSERT）の行（72 バイト）
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InsertRow {
    /// 置く点（10・20）
    pub position: [f64; 2],
    /// 尺度（41・42）
    pub scale: [f64; 2],
    /// 回転（50、度）
    pub rotation: f64,
    /// 定義の名前（2）。UTF-8
    pub name: TextSpan,
    /// 定義の番号。なければ `NO_DEFINITION`
    pub definition: u32,
    /// 属性の最初の番号（`Pools::attribs`）
    pub attrib_first: u32,
    /// 属性の数
    pub attrib_count: u32,
    /// 付いた ATTRIB レコードの数（読めなかったものも数える）
    pub attached: u32,
    /// `Pools::insert_extras` の 1 始まりの番号（0 なら格子も切り抜きもなし）
    pub extra: u32,
    /// `INSERT_*`
    pub flags: u8,
    /// パディング
    pub pad: [u8; 3],
}

impl InsertRow {
    /// 66 が 0 でない整数か
    #[inline]
    pub const fn followed(&self) -> bool {
        self.flags & INSERT_FOLLOWED != 0
    }

    /// 列と行の数
    #[inline]
    pub fn grid(&self, pools: &Pools) -> [u32; 2] {
        match self.extra.checked_sub(1) {
            None => [1, 1],
            Some(at) => pools
                .insert_extras
                .get(at as usize)
                .map_or([1, 1], |extra| extra.grid),
        }
    }

    /// 列と行の間隔
    #[inline]
    pub fn spacing(&self, pools: &Pools) -> [f64; 2] {
        match self.extra.checked_sub(1) {
            None => [0.0, 0.0],
            Some(at) => pools
                .insert_extras
                .get(at as usize)
                .map_or([0.0, 0.0], |extra| extra.spacing),
        }
    }

    /// 切り抜き境界
    #[inline]
    pub fn clip<'p>(&self, pools: &'p Pools) -> Option<&'p Clip> {
        let at = self.extra.checked_sub(1)?;
        let clip_idx = pools.insert_extras.get(at as usize)?.clip.checked_sub(1)?;
        pools.clips.get(clip_idx as usize)
    }
}

/// 線種の参照
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineTypeRef {
    /// 画層に従う
    Layer,
    /// ブロックに従う
    Block,
    /// 名前で指す（UTF-8）
    Named(TextSpan),
}

/// 既定でない図形スタイル（線種・倍率・線幅・透過性・62+420 併記）
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ExtraStyle {
    /// 線種
    pub line_type: LineTypeRef,
    /// 解決した `LineTypeId`（`LineTypeRef::Named` のときに使う）
    pub line_type_id: u32,
    /// 線種の倍率（48）
    pub line_type_scale: f64,
    /// 線幅（370）
    pub line_weight: Inherit<LineWeight>,
    /// 透過性（440）
    pub transparency: Inherit<Transparency>,
    /// 420（RGB）と 62（1..=255）の両方を持つときの色番号
    pub dual_aci: Option<u8>,
}

impl ExtraStyle {
    /// すべて既定値か
    pub fn is_default(&self) -> bool {
        self.line_type == LineTypeRef::Layer
            && self.line_type_scale == 1.0
            && self.line_weight == Inherit::Layer
            && self.transparency == Inherit::Layer
            && self.dual_aci.is_none()
    }
}

/// 記号の属性（ATTRIB）（88 バイト）
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AttribRow {
    /// ハンドル（`ATTRIB_HAS_HANDLE` が立つとき有効）
    pub handle: u64,
    /// 文字
    pub text: TextRow,
    /// タグ（2）。UTF-8
    pub tag: TextSpan,
    /// レコードの番号
    pub record: u32,
    /// 画層の番号
    pub layer: u32,
    /// 色の指定（`color`）
    pub color: u32,
    /// `Pools::extra_styles` の 1 始まりの番号（0 なら既定）
    pub extra_style: u32,
}

impl AttribRow {
    /// 表示しない属性か（70 の 1）
    #[inline]
    pub const fn hidden(&self) -> bool {
        self.text.flags & ATTRIB_HIDDEN != 0
    }

    /// ハンドル
    #[inline]
    pub const fn handle_opt(&self) -> Option<u64> {
        if self.text.flags & ATTRIB_HAS_HANDLE != 0 {
            Some(self.handle)
        } else {
            None
        }
    }

    /// 複数行の書式（101）を持つか
    #[inline]
    pub const fn is_formatted(&self) -> bool {
        self.text.flags & ATTRIB_FORMATTED != 0
    }
}

/// レコードの種類とタグから、親に付けるために見る値を読む。INSERT・POLYLINE・ATTRIB・VERTEX・SEQEND で
/// なければ `None`
pub fn followed(name: &[u8], tags: &[RawTag<'_>]) -> Option<Followed> {
    single::followed_tags(name, tags)
}

/// 持ち上げた形。折れ線の頂点と文字の中身は、渡した置き場に足す
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Geom {
    /// 始点と終点
    Line([f64; 4]),
    /// 中心と半径
    Circle([f64; 3]),
    /// 中心・半径・始まりと終わりの角（度、反時計回り）
    Arc([f64; 5]),
    /// 折れ線
    Polyline {
        /// 置き場の中の最初の頂点
        first: u32,
        /// 頂点の数
        count: u32,
        /// `POLY_*`
        flags: u8,
        /// 一定の幅（43）。0 は、なし
        width: f64,
    },
    /// 文字
    Text(TextRow),
    /// 記号
    Insert(InsertRow),
}

/// 巻き戻し用の印
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PoolMarks {
    vertices: usize,
    widths: usize,
    width_first: usize,
    text: (usize, usize),
    attribs: usize,
    text_extras: usize,
    insert_extras: usize,
    extra_styles: usize,
    clips: usize,
    formatted_attribs: usize,
}

/// 折れ線の頂点・文字の中身・記号の属性・まれな追加指定の置き場
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Pools {
    /// 記号の属性。記号ごとに、続けて並ぶ
    pub attribs: Paged<AttribRow>,
    /// 頂点（x、y、膨らみ）
    pub vertices: Paged<[f64; 3]>,
    /// 頂点ごとの幅（始まり、終わり）。幅を持つ折れ線の頂点だけが並ぶ
    pub widths: Paged<[f64; 2]>,
    /// 幅を持つ折れ線ごとの、`widths` の中の最初の番号。`Geom::Polyline` の `first` から引く
    pub width_first: Vec<(u32, u32)>,
    /// UTF-8 文字列の置き場
    pub text: BytePool,
    /// 文字のまれな指定
    pub text_extras: Paged<TextExtra>,
    /// 記号のまれな指定
    pub insert_extras: Paged<InsertExtra>,
    /// 既定でないスタイル
    pub extra_styles: Paged<ExtraStyle>,
    /// 記号の切り抜き境界
    pub clips: Vec<Clip>,
    /// 複数行属性の書式 `(attrib_index, formatted)`
    pub formatted_attribs: Vec<(u32, FormattedText<ModelPoint>)>,
}

impl Pools {
    /// 折れ線 `first` の頂点ごとの幅を新しい `Vec` で返す
    pub fn widths_of(&self, first: u32, count: u32) -> Vec<[f64; 2]> {
        let Ok(at) = self
            .width_first
            .binary_search_by_key(&first, |&(key, _)| key)
        else {
            return Vec::new();
        };
        let start = self.width_first.get(at).map_or(0, |&(_, start)| start) as usize;
        self.widths.slice_to_vec(start, start + count as usize)
    }

    /// 折れ線 `first` の頂点ごとの幅を上書きする
    pub fn overwrite_widths(&mut self, first: u32, src: &[[f64; 2]]) -> bool {
        let Ok(at) = self
            .width_first
            .binary_search_by_key(&first, |&(key, _)| key)
        else {
            return false;
        };
        let start = self.width_first.get(at).map_or(0, |&(_, start)| start) as usize;
        if start + src.len() > self.widths.len() {
            return false;
        }
        self.widths.copy_range_from_slice(start, src);
        true
    }

    /// UTF-8 のバイト列を借りる
    #[inline]
    pub fn text_of(&self, span: TextSpan) -> &[u8] {
        self.text.get(span)
    }

    /// UTF-8 文字列を借りる
    #[inline]
    pub fn str_of(&self, span: TextSpan) -> &str {
        self.text.str_of(span)
    }

    /// UTF-8 文字列をそのまま置いて位置を返す
    #[inline]
    pub fn push_utf8(&mut self, value: &str) -> Option<TextSpan> {
        self.text.push(value.as_bytes())
    }

    /// 図面の文字コードでバイト列を復号して UTF-8 として置く。失敗時は理由を返す
    pub fn push_decoded(&mut self, charset: Charset, bytes: &[u8]) -> Result<TextSpan, DropReason> {
        if bytes.is_ascii() {
            return self.text.push(bytes).ok_or(DropReason::Invalid);
        }
        if charset == Charset::Other {
            return Err(DropReason::CodePage);
        }
        match charset {
            Charset::Utf8 => {
                if core::str::from_utf8(bytes).is_err() {
                    return Err(DropReason::Invalid);
                }
                self.text.push(bytes).ok_or(DropReason::Invalid)
            }
            _ => {
                let decoded = charset.decode(bytes).ok_or(DropReason::Invalid)?;
                self.text
                    .push(decoded.as_bytes())
                    .ok_or(DropReason::Invalid)
            }
        }
    }

    /// 記号の属性を順に返す
    #[inline]
    pub fn attribs_of(&self, row: &InsertRow) -> impl Iterator<Item = &AttribRow> + '_ {
        (row.attrib_first..row.attrib_first.saturating_add(row.attrib_count))
            .filter_map(|at| self.attribs.get(at as usize))
    }

    /// 折れ線の頂点を順に返す
    #[inline]
    pub fn vertices_of(&self, first: u32, count: u32) -> impl Iterator<Item = [f64; 3]> + '_ {
        (first..first.saturating_add(count))
            .filter_map(|at| self.vertices.get(at as usize).copied())
    }

    /// 折れ線の辺 `(from, to, bulge)` を順に返す
    pub fn edges_of(
        &self,
        first: u32,
        count: u32,
        closed: bool,
    ) -> impl Iterator<Item = ([f64; 2], [f64; 2], f64)> + '_ {
        let count_usize = count as usize;
        let last = if closed && count_usize > 1 {
            count_usize
        } else {
            count_usize.saturating_sub(1)
        };
        (0..last).filter_map(move |index| {
            let [x0, y0, bulge] = *self.vertices.get(first as usize + index)?;
            let [x1, y1, _] = *self
                .vertices
                .get(first as usize + ((index + 1) % count_usize))?;
            Some(([x0, y0], [x1, y1], bulge))
        })
    }

    /// 記号に XCLIP の切り抜きを設定する
    pub fn set_insert_clip(&mut self, insert: &mut InsertRow, clip: Clip) -> Option<()> {
        let clip_id = u32::try_from(self.clips.len() + 1).ok()?;
        self.clips.push(clip);
        if let Some(at) = insert.extra.checked_sub(1) {
            if let Some(extra) = self.insert_extras.get_mut(at as usize) {
                extra.clip = clip_id;
            }
        } else {
            let extra_id = u32::try_from(self.insert_extras.len() + 1).ok()?;
            self.insert_extras.push(InsertExtra {
                grid: [1, 1],
                spacing: [0.0, 0.0],
                clip: clip_id,
            });
            insert.extra = extra_id;
        }
        Some(())
    }

    /// 色と追加スタイル番号から図面モデルの `Style` を復元する
    pub fn style_of(&self, color_bits: u32, extra_style: u32) -> Style {
        let extra = extra_style
            .checked_sub(1)
            .and_then(|at| self.extra_styles.get(at as usize))
            .copied();
        let dual_aci = extra.and_then(|e| e.dual_aci);
        let color = match color_bits & 0xFF00_0000 {
            color::BY_LAYER => Inherit::Layer,
            color::BY_BLOCK => Inherit::Block,
            color::INDEX => Inherit::Own(Color {
                code: Some(ColorCode {
                    system: ColorSystem::Aci,
                    value: color_bits & 0xFF,
                }),
                rgb: None,
            }),
            _ => {
                let [_, r, g, b] = color_bits.to_be_bytes();
                Inherit::Own(Color {
                    code: dual_aci.map(|aci| ColorCode {
                        system: ColorSystem::Aci,
                        value: u32::from(aci),
                    }),
                    rgb: Some([r, g, b]),
                })
            }
        };
        match extra {
            None => Style {
                color,
                line_type: Inherit::Layer,
                line_type_scale: 1.0,
                line_weight: Some(Inherit::Layer),
                transparency: Inherit::Layer,
            },
            Some(extra) => Style {
                color,
                line_type: match extra.line_type {
                    LineTypeRef::Layer => Inherit::Layer,
                    LineTypeRef::Block => Inherit::Block,
                    LineTypeRef::Named(_) => Inherit::Own(LineTypeId(extra.line_type_id)),
                },
                line_type_scale: extra.line_type_scale,
                line_weight: Some(extra.line_weight),
                transparency: extra.transparency,
            },
        }
    }

    /// 既に置いた [`TextRow`] から [`Text`] を組み立てるときに、まれな指定を足す
    #[allow(clippy::too_many_arguments)]
    pub fn push_text_row(
        &mut self,
        place: Place,
        height: f64,
        rotation: f64,
        width_factor: f64,
        oblique: f64,
        flags: u8,
        content: TextSpan,
        style: TextSpan,
    ) -> Option<TextRow> {
        let (point, second, h_align, v_align) = match place {
            Place::Anchor {
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
                (point, None, h, v)
            }
            Place::Between { first, second, fit } => (
                first,
                Some(second),
                match fit {
                    Fit::Aligned => 4,
                    Fit::Fit => 5,
                },
                0,
            ),
        };
        let extra = if second.is_some() || width_factor != 1.0 || oblique != 0.0 {
            let id = u32::try_from(self.text_extras.len() + 1).ok()?;
            self.text_extras.push(TextExtra {
                second: second.unwrap_or(point),
                width_factor,
                oblique,
            });
            id
        } else {
            0
        };
        Some(TextRow {
            point,
            height,
            rotation,
            content,
            style,
            extra,
            h_align,
            v_align,
            flags,
            pad: 0,
        })
    }

    /// 足したものを除くための、今の長さ
    pub fn marks(&self) -> PoolMarks {
        PoolMarks {
            vertices: self.vertices.len(),
            widths: self.widths.len(),
            width_first: self.width_first.len(),
            text: self.text.marks(),
            attribs: self.attribs.len(),
            text_extras: self.text_extras.len(),
            insert_extras: self.insert_extras.len(),
            extra_styles: self.extra_styles.len(),
            clips: self.clips.len(),
            formatted_attribs: self.formatted_attribs.len(),
        }
    }

    /// `marks` のときの長さに戻す
    pub fn truncate(&mut self, marks: PoolMarks) {
        self.vertices.truncate(marks.vertices);
        self.widths.truncate(marks.widths);
        self.width_first.truncate(marks.width_first);
        self.text.truncate(marks.text);
        self.attribs.truncate(marks.attribs);
        self.text_extras.truncate(marks.text_extras);
        self.insert_extras.truncate(marks.insert_extras);
        self.extra_styles.truncate(marks.extra_styles);
        self.clips.truncate(marks.clips);
        self.formatted_attribs.truncate(marks.formatted_attribs);
    }

    /// 空にする
    pub fn clear(&mut self) {
        self.truncate(PoolMarks::default());
    }

    /// 保持しているバイト数の概算
    pub fn bytes(&self) -> usize {
        self.attribs.bytes()
            + self.vertices.bytes()
            + self.widths.bytes()
            + self.width_first.len() * 8
            + self.text.len()
            + self.text_extras.bytes()
            + self.insert_extras.bytes()
            + self.extra_styles.bytes()
            + self.clips.len() * core::mem::size_of::<Clip>()
    }
}

/// 色の指定。上位のバイトが種類
pub mod color {
    /// 色番号（下位 8 ビット）
    pub const INDEX: u32 = 0;
    /// RGB（下位 24 ビット）
    pub const RGB: u32 = 1 << 24;
    /// 画層に従う
    pub const BY_LAYER: u32 = 2 << 24;
    /// ブロックに従う
    pub const BY_BLOCK: u32 = 3 << 24;
}

/// 要素にならなかった理由（診断の照合に使う）
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum DropReason {
    /// グループ・コードの値が不正、または文字を復号できない（`dxf.lift.invalid`）
    Invalid = 0,
    /// 平面（Z = 0・法線 +Z）でない（`dxf.non-planar.dropped`）
    NonPlanar = 1,
    /// 対応していないコード・ページの非 ASCII 文字（`dxf.code-page.unsupported`）
    CodePage = 2,
}

impl From<LiftFail> for DropReason {
    fn from(err: LiftFail) -> Self {
        match err {
            LiftFail::Invalid(_) => Self::Invalid,
            LiftFail::NonPlanar => Self::NonPlanar,
        }
    }
}

/// 持ち上げた行
#[derive(Clone, Debug, PartialEq)]
pub struct Lifted {
    /// ハンドル（5 の、16 進として読める最初の値）
    pub handle: Option<u64>,
    /// 形
    pub geom: Geom,
    /// 画層の名前（8）。復号した UTF-8
    pub layer: TextSpan,
    /// 色の指定（`color`）
    pub color: u32,
    /// `Pools::extra_styles` の 1 始まりの番号（0 なら既定）
    pub extra_style: u32,
}

/// 持ち上げの結果
#[derive(Clone, Debug, PartialEq)]
pub enum Lift {
    /// モデル空間の要素になる
    Element(Lifted),
    /// ペーパー空間（67 = 1）。この試作は読まない
    Paper,
    /// 要素にならない（今の実装でも要素にならない）
    Not(DropReason),
}

/// ATTRIB の持ち上げ結果
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AttribLift {
    /// 単一行の属性として持ち上がった（行と画層名の範囲）
    Row(AttribRow, TextSpan),
    /// 101（埋め込み MTEXT）を持つので、組み立て時に `SingleCatalog` で読む
    Multiline,
    /// 要素にならない
    Not(DropReason),
}

fn decode_bytes_str(charset: Charset, bytes: &[u8]) -> Result<String, DropReason> {
    if bytes.is_ascii() {
        return core::str::from_utf8(bytes)
            .map(str::to_owned)
            .map_err(|_| DropReason::Invalid);
    }
    if charset == Charset::Other {
        return Err(DropReason::CodePage);
    }
    charset.decode(bytes).ok_or(DropReason::Invalid)
}

/// 図形のレコードを持ち上げる。`tags` は、グループ・コード 0 の次からのタグ。
/// 折れ線の頂点と文字列は `pools` に足す（要素にならなければ、足さない）
pub fn lift(charset: Charset, kind: Kind, tags: &[RawTag<'_>], pools: &mut Pools) -> Lift {
    let fields = DxfFields::new(tags);
    match fields.int(67, 0) {
        Ok(1) => return Lift::Paper,
        Ok(_) => {}
        Err(err) => return Lift::Not(err.into()),
    }
    let marks = pools.marks();
    match lifted(charset, kind, tags, &fields, pools) {
        Ok(lifted) => Lift::Element(lifted),
        Err(reason) => {
            pools.truncate(marks);
            Lift::Not(reason)
        }
    }
}

fn lifted(
    charset: Charset,
    kind: Kind,
    tags: &[RawTag<'_>],
    fields: &DxfFields<'_>,
    pools: &mut Pools,
) -> Result<Lifted, DropReason> {
    single::check_upright(fields)?;
    let geom = match kind {
        Kind::Line => Geom::Line(single::line_ends(fields)?),
        Kind::Circle => Geom::Circle(single::circle_geom(fields)?),
        Kind::Arc => Geom::Arc(single::arc_geom(fields)?),
        Kind::Polyline => polyline(tags, fields, pools)?,
        Kind::Text => Geom::Text(text(charset, fields, pools, 73)?),
        Kind::Insert => Geom::Insert(insert(charset, fields, pools)?),
    };
    let (layer, color, extra_style) = styled(charset, fields, pools)?;
    Ok(Lifted {
        handle: fields.handle,
        geom,
        layer,
        color,
        extra_style,
    })
}

/// 画層・色・線種・線種の倍率・線幅・透過性を確かめて、画層の名前・色の指定・追加スタイル番号を返す
fn styled(
    charset: Charset,
    fields: &DxfFields<'_>,
    pools: &mut Pools,
) -> Result<(TextSpan, u32, u32), DropReason> {
    let es = single::entity_style(fields)?;
    let (color_bits, dual_aci) = match es.color {
        Inherit::Own(Color {
            code,
            rgb: Some([r, g, b]),
        }) => (
            color::RGB | u32::from_be_bytes([0, r, g, b]),
            code.and_then(|c| u8::try_from(c.value).ok()),
        ),
        Inherit::Own(Color {
            code: Some(code),
            rgb: None,
        }) => (color::INDEX | code.value, None),
        Inherit::Own(Color {
            code: None,
            rgb: None,
        }) => return Err(DropReason::Invalid),
        Inherit::Block => (color::BY_BLOCK, None),
        Inherit::Layer => (color::BY_LAYER, None),
    };
    let line_type = match es.line_type {
        RawLineType::Layer => LineTypeRef::Layer,
        RawLineType::Block => LineTypeRef::Block,
        RawLineType::Named(bytes) => LineTypeRef::Named(pools.push_decoded(charset, bytes)?),
    };
    let extra = ExtraStyle {
        line_type,
        line_type_id: 0,
        line_type_scale: es.line_type_scale,
        line_weight: es.line_weight,
        transparency: es.transparency,
        dual_aci,
    };
    let extra_style = if extra.is_default() {
        0
    } else {
        let id = u32::try_from(pools.extra_styles.len() + 1).map_err(|_| DropReason::Invalid)?;
        pools.extra_styles.push(extra);
        id
    };
    let raw_layer = fields.raw_text(8)?.ok_or(DropReason::Invalid)?;
    let layer = pools.push_decoded(charset, raw_layer)?;
    Ok((layer, color_bits, extra_style))
}

/// 記号（INSERT）。定義がなくても要素になる
fn insert(
    charset: Charset,
    fields: &DxfFields<'_>,
    pools: &mut Pools,
) -> Result<InsertRow, DropReason> {
    let raw = single::insert_geom(fields)?;
    let name = pools.push_decoded(charset, raw.name)?;
    let extra = if raw.grid != [1, 1] {
        let id = u32::try_from(pools.insert_extras.len() + 1).map_err(|_| DropReason::Invalid)?;
        pools.insert_extras.push(InsertExtra {
            grid: raw.grid,
            spacing: raw.spacing,
            clip: 0,
        });
        id
    } else {
        0
    };
    Ok(InsertRow {
        position: raw.position,
        scale: raw.scale,
        rotation: raw.rotation,
        name,
        definition: NO_DEFINITION,
        attrib_first: 0,
        attrib_count: 0,
        attached: 0,
        extra,
        flags: if raw.followed { INSERT_FOLLOWED } else { 0 },
        pad: [0; 3],
    })
}

/// ATTRIB を、記号の属性として持ち上げる。空間（67）は見ない。読めなければ、置き場に何も残さない
pub fn attrib(charset: Charset, tags: &[RawTag<'_>], record: u32, pools: &mut Pools) -> AttribLift {
    let fields = DxfFields::new(tags);
    if fields.has_code(101) {
        return AttribLift::Multiline;
    }
    let marks = pools.marks();
    match attribute(charset, &fields, record, pools) {
        Ok((row, layer_name)) => AttribLift::Row(row, layer_name),
        Err(reason) => {
            pools.truncate(marks);
            AttribLift::Not(reason)
        }
    }
}

fn attribute(
    charset: Charset,
    fields: &DxfFields<'_>,
    record: u32,
    pools: &mut Pools,
) -> Result<(AttribRow, TextSpan), DropReason> {
    single::check_upright(fields)?;
    let mut text = text(charset, fields, pools, 74)?;
    let raw_tag = fields.raw_text(2)?.ok_or(DropReason::Invalid)?;
    let tag = pools.push_decoded(charset, raw_tag)?;
    if fields.int(70, 0)? & 1 != 0 {
        text.flags |= ATTRIB_HIDDEN;
    }
    if fields.handle.is_some() {
        text.flags |= ATTRIB_HAS_HANDLE;
    }
    let (layer_name, color, extra_style) = styled(charset, fields, pools)?;
    Ok((
        AttribRow {
            handle: fields.handle.unwrap_or(0),
            text,
            tag,
            record,
            layer: 0,
            color,
            extra_style,
        },
        layer_name,
    ))
}

/// `SingleCatalog` で持ち上げた複数行 `Attribute` を `AttribRow` に変換して `pools` に置く
pub fn push_lifted_attribute(
    attr: Attribute<ModelPoint>,
    record: u32,
    pools: &mut Pools,
) -> Option<AttribRow> {
    let place = match attr.text.placement {
        Placement::Anchor {
            point,
            horizontal,
            vertical,
        } => Place::Anchor {
            point: [point.x, point.y],
            horizontal,
            vertical,
        },
        Placement::Between { first, second, fit } => Place::Between {
            first: [first.x, first.y],
            second: [second.x, second.y],
            fit,
        },
    };
    let mut flags = 0u8;
    if attr.text.backward {
        flags |= TEXT_BACKWARD;
    }
    if attr.text.upside_down {
        flags |= TEXT_UPSIDE_DOWN;
    }
    if attr.hidden {
        flags |= ATTRIB_HIDDEN;
    }
    let handle = match attr.source.key() {
        Some(Key::Handle(h)) => {
            flags |= ATTRIB_HAS_HANDLE;
            h
        }
        _ => 0,
    };
    if attr.formatted.is_some() {
        flags |= ATTRIB_FORMATTED;
    }
    let content = pools.push_utf8(&attr.text.content)?;
    let style_span = if attr.text.style == "STANDARD" {
        TextSpan::STANDARD
    } else {
        pools.push_utf8(&attr.text.style)?
    };
    let Height {
        value: Length::Model(height),
        ..
    } = attr.text.height
    else {
        return None;
    };
    let text = pools.push_text_row(
        place,
        height.value,
        attr.text.rotation,
        attr.text.width_factor,
        attr.text.oblique,
        flags,
        content,
        style_span,
    )?;
    let tag = pools.push_utf8(&attr.tag)?;
    let (color_bits, dual_aci) = encode_color(&attr.style.color);
    let line_type = match attr.style.line_type {
        Inherit::Layer => LineTypeRef::Layer,
        Inherit::Block => LineTypeRef::Block,
        Inherit::Own(_) => LineTypeRef::Named(TextSpan::EMPTY),
    };
    let line_type_id = match attr.style.line_type {
        Inherit::Own(id) => id.0,
        _ => 0,
    };
    let extra = ExtraStyle {
        line_type,
        line_type_id,
        line_type_scale: attr.style.line_type_scale,
        line_weight: attr.style.line_weight.unwrap_or(Inherit::Layer),
        transparency: attr.style.transparency,
        dual_aci,
    };
    let extra_style = if extra.is_default() {
        0
    } else {
        let id = u32::try_from(pools.extra_styles.len() + 1).ok()?;
        pools.extra_styles.push(extra);
        id
    };
    let attrib_index = u32::try_from(pools.attribs.len()).ok()?;
    if let Some(formatted) = attr.formatted {
        pools.formatted_attribs.push((attrib_index, formatted));
    }
    Some(AttribRow {
        handle,
        text,
        tag,
        record,
        layer: attr.layer.0,
        color: color_bits,
        extra_style,
    })
}

fn encode_color(c: &Inherit<Color>) -> (u32, Option<u8>) {
    match c {
        Inherit::Layer => (color::BY_LAYER, None),
        Inherit::Block => (color::BY_BLOCK, None),
        Inherit::Own(Color {
            code,
            rgb: Some([r, g, b]),
        }) => (
            color::RGB | u32::from_be_bytes([0, *r, *g, *b]),
            code.and_then(|c| u8::try_from(c.value).ok()),
        ),
        Inherit::Own(Color {
            code: Some(code),
            rgb: None,
        }) => (color::INDEX | code.value, None),
        Inherit::Own(Color {
            code: None,
            rgb: None,
        }) => (color::BY_LAYER, None),
    }
}

struct PoolPolySink<'p> {
    pools: &'p mut Pools,
    first: usize,
    widths: Vec<[Option<f64>; 2]>,
    any_width: bool,
}

impl PolylineSink for PoolPolySink<'_> {
    fn len(&self) -> usize {
        self.pools.vertices.len() - self.first
    }

    fn push_vertex(&mut self, x: f64, y: f64) {
        self.pools.vertices.push([x, y, 0.0]);
        if self.any_width {
            self.widths.push([None; 2]);
        }
    }

    fn set_width(&mut self, is_end: bool, width: f64) {
        let count = self.len();
        if !self.any_width {
            self.any_width = true;
            self.widths.resize(count, [None; 2]);
        }
        if let Some(slot) = self.widths.last_mut() {
            slot[usize::from(is_end)] = Some(width);
        }
    }

    fn set_bulge(&mut self, bulge: f64) {
        if let Some(vertex) = self.pools.vertices.last_mut() {
            vertex[2] = bulge;
        }
    }
}

fn polyline(
    tags: &[RawTag<'_>],
    fields: &DxfFields<'_>,
    pools: &mut Pools,
) -> Result<Geom, DropReason> {
    let first = pools.vertices.len();
    let mut sink = PoolPolySink {
        pools,
        first,
        widths: Vec::new(),
        any_width: false,
    };
    let meta = single::lwpolyline_scan(fields, tags, &mut sink)?;
    let count = sink.len();
    let any_width = sink.any_width;
    let widths = sink.widths;
    let mut flags = 0;
    if meta.closed {
        flags |= POLY_CLOSED;
    }
    if meta.line_type_generation {
        flags |= POLY_GENERATION;
    }
    if any_width {
        flags |= POLY_WIDTHS;
        pools.width_first.push((
            u32::try_from(first).map_err(|_| DropReason::Invalid)?,
            u32::try_from(pools.widths.len()).map_err(|_| DropReason::Invalid)?,
        ));
        pools.widths.extend(
            widths
                .iter()
                .map(|[start, end]| [start.unwrap_or(0.0), end.unwrap_or(0.0)]),
        );
    }
    Ok(Geom::Polyline {
        first: u32::try_from(first).map_err(|_| DropReason::Invalid)?,
        count: u32::try_from(count).map_err(|_| DropReason::Invalid)?,
        flags,
        width: meta.width.unwrap_or(0.0),
    })
}

/// 文字。縦の揃えのグループ・コードは、TEXT では 73、ATTRIB では 74
fn text(
    charset: Charset,
    fields: &DxfFields<'_>,
    pools: &mut Pools,
    vertical: i16,
) -> Result<TextRow, DropReason> {
    let raw = single::text_geom(fields, vertical)?;
    let place = match raw.place {
        RawTextPlace::Anchor {
            point,
            horizontal,
            vertical,
        } => Place::Anchor {
            point,
            horizontal,
            vertical,
        },
        RawTextPlace::Between { first, second, fit } => Place::Between { first, second, fit },
    };
    let mut bits = 0;
    if raw.backward {
        bits |= TEXT_BACKWARD;
    }
    if raw.upside_down {
        bits |= TEXT_UPSIDE_DOWN;
    }
    let content = pools.push_decoded(charset, raw.content)?;
    let style = match raw.style {
        Some(name) => pools.push_decoded(charset, name)?,
        None => TextSpan::STANDARD,
    };
    pools
        .push_text_row(
            place,
            raw.height,
            raw.rotation,
            raw.width_factor,
            raw.oblique,
            bits,
            content,
            style,
        )
        .ok_or(DropReason::Invalid)
}

/// 任意のレコードの指定グループ・コードを文字として復号する補助
pub fn decode_tag_str(charset: Charset, tags: &[RawTag<'_>], code: i16) -> Option<String> {
    let raw = tags.iter().find(|t| t.code == code)?.value;
    decode_bytes_str(charset, raw).ok()
}

const MODEL: SpaceId = SpaceId(0);

fn point([x, y]: [f64; 2]) -> ModelPoint {
    ModelPoint { space: MODEL, x, y }
}

fn length(value: f64) -> Length {
    Length::Model(ModelLength {
        space: MODEL,
        value,
    })
}

/// 列の値から図面モデルの `Element<ModelPoint>` を復元する
pub fn element_of(
    handle: Option<u64>,
    record: Option<u32>,
    layer: u32,
    color_bits: u32,
    extra_style: u32,
    geom: &Geom,
    pools: &Pools,
) -> Option<Element<ModelPoint>> {
    let shape = shape(geom, pools)?;
    let style = pools.style_of(color_bits, extra_style);
    let base_ref = match record {
        Some(rec) => Ref::at_record(u64::from(rec)),
        None => Ref::default(),
    };
    let source = match handle {
        Some(h) => base_ref.with_key(Key::Handle(h)),
        None => base_ref,
    };
    Some(Element {
        layer: LayerId(layer),
        style,
        shape,
        source,
    })
}

/// 列の値を、図面モデルの形にする
pub fn shape(geom: &Geom, pools: &Pools) -> Option<Shape<ModelPoint>> {
    Some(match geom {
        Geom::Line([x0, y0, x1, y1]) => Shape::Line {
            start: point([*x0, *y0]),
            end: point([*x1, *y1]),
        },
        Geom::Circle([x, y, radius]) => Shape::Circle {
            center: point([*x, *y]),
            radius: length(*radius),
        },
        Geom::Arc([x, y, radius, start, end]) => Shape::Curve(Edge::Arc {
            center: point([*x, *y]),
            radius: length(*radius),
            start: *start,
            end: *end,
            counterclockwise: true,
        }),
        Geom::Polyline {
            first,
            count,
            flags,
            width,
        } => {
            let vertices: Vec<Vertex<ModelPoint>> = pools
                .vertices_of(*first, *count)
                .map(|[x, y, bulge]| Vertex {
                    point: point([x, y]),
                    bulge,
                })
                .collect();
            if vertices.len() != *count as usize {
                return None;
            }
            let vertex_widths = if flags & POLY_WIDTHS != 0 {
                pools
                    .widths_of(*first, *count)
                    .into_iter()
                    .map(|[start, end]| [length(start), length(end)])
                    .collect()
            } else {
                Vec::new()
            };
            Shape::Polyline(Polyline {
                vertices,
                closed: flags & POLY_CLOSED != 0,
                width: (*width != 0.0).then(|| length(*width)),
                vertex_widths,
                line_type_generation: flags & POLY_GENERATION != 0,
            })
        }
        Geom::Text(row) => Shape::Text(lettering(row, pools, "dxf.TEXT.40")?),
        Geom::Insert(row) => {
            let grid = row.grid(pools);
            let spacing = row.spacing(pools);
            Shape::Symbol(Symbol {
                key: SymbolKey {
                    namespace: BLOCK.into(),
                    name: pools.str_of(row.name).to_owned(),
                },
                position: point(row.position),
                scale: row.scale,
                rotation: row.rotation,
                grid: (grid != [1, 1]).then(|| Grid {
                    columns: grid[0],
                    rows: grid[1],
                    column_spacing: length(spacing[0]),
                    row_spacing: length(spacing[1]),
                }),
                attributes: (row.attrib_first..row.attrib_first.saturating_add(row.attrib_count))
                    .map(|attrib_idx| {
                        let attrib = pools.attribs.get(attrib_idx as usize)?;
                        let formatted = if attrib.is_formatted() {
                            pools
                                .formatted_attribs
                                .binary_search_by_key(&attrib_idx, |&(k, _)| k)
                                .ok()
                                .and_then(|pos| pools.formatted_attribs.get(pos))
                                .map(|(_, f)| f.clone())
                        } else {
                            None
                        };
                        let base_ref = Ref::at_record(u64::from(attrib.record));
                        let source = match attrib.handle_opt() {
                            Some(h) => base_ref.with_key(Key::Handle(h)),
                            None => base_ref,
                        };
                        Some(Attribute {
                            tag: pools.str_of(attrib.tag).to_owned(),
                            text: lettering(&attrib.text, pools, "dxf.ATTRIB.40")?,
                            formatted,
                            hidden: attrib.hidden(),
                            layer: LayerId(attrib.layer),
                            style: pools.style_of(attrib.color, attrib.extra_style),
                            source,
                        })
                    })
                    .collect::<Option<Vec<_>>>()?,
                clip: row.clip(pools).cloned(),
            })
        }
    })
}

/// 記号の定義の名前空間。今の実装の `BLOCK`（`crates/zumen-acad/src/drawing.rs`）と同じ
pub const BLOCK: &str = "dxf.block";

/// 文字の行を、図面モデルの文字にする。`meaning` は、高さの意味
fn lettering(row: &TextRow, pools: &Pools, meaning: &'static str) -> Option<Text<ModelPoint>> {
    Some(Text {
        content: pools.str_of(row.content).to_owned(),
        height: Height {
            value: length(row.height),
            meaning: HeightMeaning::Declared(meaning.into()),
        },
        placement: match row.place(pools) {
            Place::Anchor {
                point: at,
                horizontal,
                vertical,
            } => Placement::Anchor {
                point: point(at),
                horizontal,
                vertical,
            },
            Place::Between { first, second, fit } => Placement::Between {
                first: point(first),
                second: point(second),
                fit,
            },
        },
        rotation: row.rotation,
        width_factor: row.width_factor(pools),
        oblique: row.oblique(pools),
        backward: row.flags & TEXT_BACKWARD != 0,
        upside_down: row.flags & TEXT_UPSIDE_DOWN != 0,
        style: pools.str_of(row.style).to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tags(pairs: &[(i16, &'static str)]) -> Vec<RawTag<'static>> {
        pairs
            .iter()
            .map(|&(code, value)| RawTag {
                code,
                value: value.as_bytes(),
            })
            .collect()
    }

    fn element(kind: Kind, pairs: &[(i16, &'static str)], pools: &mut Pools) -> Option<Lifted> {
        match lift(Charset::Utf8, kind, &tags(pairs), pools) {
            Lift::Element(lifted) => Some(lifted),
            _ => None,
        }
    }

    #[test]
    fn compact_row_sizes_stay_within_budget() {
        assert_eq!(core::mem::size_of::<TextRow>(), 56);
        assert_eq!(core::mem::size_of::<InsertRow>(), 72);
        assert_eq!(core::mem::size_of::<AttribRow>(), 88);
    }

    #[test]
    fn a_line_needs_both_ends_and_a_layer() {
        let mut pools = Pools::default();
        let line = [
            (5, "2A"),
            (8, "0"),
            (10, "1.0"),
            (20, "2"),
            (30, "0.0"),
            (11, "3"),
            (21, "4"),
        ];
        let lifted = element(Kind::Line, &line, &mut pools).expect("線");
        assert_eq!(lifted.handle, Some(0x2A));
        assert_eq!(lifted.geom, Geom::Line([1.0, 2.0, 3.0, 4.0]));
        assert_eq!(lifted.color, color::BY_LAYER);
        assert_eq!(lifted.extra_style, 0);
        // 画層がない・Z が 0 でない・押し出しの向きが既定でないものは、要素にしない
        assert!(element(Kind::Line, &line[..1], &mut pools).is_none());
        let mut high = line.to_vec();
        high[4] = (30, "1.0");
        assert!(element(Kind::Line, &high, &mut pools).is_none());
        let mut tilted = line.to_vec();
        tilted.push((230, "-1.0"));
        assert!(element(Kind::Line, &tilted, &mut pools).is_none());
        let mut paper = line.to_vec();
        paper.push((67, "1"));
        assert_eq!(
            lift(Charset::Utf8, Kind::Line, &tags(&paper), &mut pools),
            Lift::Paper
        );
    }

    #[test]
    fn a_polyline_counts_its_vertices() {
        let mut pools = Pools::default();
        let square = [
            (8, "0"),
            (90, "3"),
            (70, "129"),
            (43, "0.0"),
            (10, "0"),
            (20, "0"),
            (10, "2"),
            (20, "0"),
            (42, "0.5"),
            (10, "2"),
            (20, "2"),
        ];
        let lifted = element(Kind::Polyline, &square, &mut pools).expect("折れ線");
        assert_eq!(
            lifted.geom,
            Geom::Polyline {
                first: 0,
                count: 3,
                flags: POLY_CLOSED | POLY_GENERATION,
                width: 0.0
            }
        );
        let verts: Vec<_> = pools.vertices.iter().copied().collect();
        assert_eq!(verts, [[0.0, 0.0, 0.0], [2.0, 0.0, 0.5], [2.0, 2.0, 0.0]]);
        let mut short = square.to_vec();
        short[1] = (90, "4");
        assert!(element(Kind::Polyline, &short, &mut pools).is_none());
        assert_eq!(pools.vertices.len(), 3);
        let mut wide = square.to_vec();
        wide.insert(8, (40, "0.25"));
        let lifted = element(Kind::Polyline, &wide, &mut pools).expect("幅つき");
        let Geom::Polyline {
            first,
            count,
            flags,
            ..
        } = lifted.geom
        else {
            panic!("折れ線");
        };
        assert_ne!(flags & POLY_WIDTHS, 0);
        assert_eq!(
            pools.widths_of(first, count),
            [[0.0, 0.0], [0.25, 0.0], [0.0, 0.0]]
        );
    }

    #[test]
    fn text_alignment_and_shift_jis_decoding_work() {
        let mut pools = Pools::default();
        let base = [(8, "0"), (10, "1"), (20, "2"), (40, "0.5"), (1, "abc")];
        let lifted = element(Kind::Text, &base, &mut pools).expect("文字");
        let Geom::Text(row) = &lifted.geom else {
            panic!("文字");
        };
        assert_eq!(pools.text_of(row.content), b"abc");
        assert_eq!(row.style, TextSpan::STANDARD);
        assert_eq!(
            row.place(&pools),
            Place::Anchor {
                point: [1.0, 2.0],
                horizontal: Horizontal::Left,
                vertical: Vertical::Baseline
            }
        );
        // Shift_JIS のバイト列も復号して UTF-8 として持つ
        let sjis = [
            RawTag {
                code: 8,
                value: &[0x8A, 0x4B], // "階" (2 バイトめが 'K')
            },
            RawTag {
                code: 10,
                value: b"0",
            },
            RawTag {
                code: 20,
                value: b"0",
            },
            RawTag {
                code: 40,
                value: b"1",
            },
            RawTag {
                code: 1,
                value: &[0x90, 0x7D, 0x96, 0xCA], // "図面"
            },
        ];
        let Lift::Element(sjis_lifted) = lift(Charset::ShiftJis, Kind::Text, &sjis, &mut pools)
        else {
            panic!("Shift_JIS 文字");
        };
        assert_eq!(pools.str_of(sjis_lifted.layer), "階");
        let Geom::Text(sjis_row) = sjis_lifted.geom else {
            panic!("文字");
        };
        assert_eq!(pools.str_of(sjis_row.content), "図面");
    }

    #[test]
    fn colors_follow_62_and_420() {
        let mut pools = Pools::default();
        let base = [(8, "0"), (10, "0"), (20, "0"), (40, "1")];
        let with = |extra: &[(i16, &'static str)], pools: &mut Pools| {
            let mut all = base.to_vec();
            all.extend_from_slice(extra);
            element(Kind::Circle, &all, pools).map(|lifted| lifted.color)
        };
        assert_eq!(with(&[(62, "1")], &mut pools), Some(color::INDEX | 1));
        assert_eq!(with(&[(62, "0")], &mut pools), Some(color::BY_BLOCK));
        assert_eq!(with(&[(420, "255")], &mut pools), Some(color::RGB | 255));
        assert_eq!(with(&[(62, "257")], &mut pools), None);
        assert_eq!(with(&[(62, "-1")], &mut pools), None);
        assert_eq!(with(&[(370, "212")], &mut pools), None);
        assert_eq!(with(&[(370, "-3")], &mut pools), Some(color::BY_LAYER));
        assert_eq!(with(&[(48, "0.0")], &mut pools), None);
        assert_eq!(with(&[(440, "50331648")], &mut pools), None);
    }
}
