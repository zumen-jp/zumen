//! 1 つのオブジェクトの読み方（持ち上げ）と書き戻し（変更・追加・削除）の共通規則。
//!
//! 一括の変換（`lift`・`apply`）と、常駐する文書エンジンが、同じ規則を 1 か所から呼ぶために切り出した入口。

use std::collections::BTreeMap;
use std::sync::Arc;

use zumen_base::{Budget, CancelToken, Cx, Diagnostics};
use zumen_drawing::{
    Affine, Color, ColorCode, ColorSystem, Element, Fit, Horizontal, Inherit, LayerId, LineTypeId,
    LineWeight, ModelPoint, PaperLength, Style, Transparency, Vertical,
};

use super::MODEL;
use super::apply::{
    Born, Encoder, InSpace, Target, anchor, children_of_values, followed_values,
    moved_with_attribs, moving_xdata_code, next_handle_from, replacement, restyled,
};
use super::style::transparency as decode_transparency;
use crate::charset::Charset;
use crate::db::{Data, Object, Origin, Section, Value};
use crate::dxf::{self, RawData};

/// DXF のレコードの、グループ・コード 0 の次からの生のタグ
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RawTag<'a> {
    /// グループ・コード
    pub code: i16,
    /// 値の行（改行を除く）
    pub value: &'a [u8],
}

/// 続くレコードを親に付けるときの、レコードの役
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Follow {
    /// INSERT。`opens` は、66 が 1（続く ATTRIB を、SEQEND まで自分に付ける）
    Insert {
        /// 続くレコードを受けるか
        opens: bool,
    },
    /// POLYLINE。`opens` は、66 が 0 でない（続く VERTEX を、SEQEND まで自分に付ける）
    Polyline {
        /// 続くレコードを受けるか
        opens: bool,
    },
    /// ATTRIB
    Attrib,
    /// VERTEX
    Vertex,
    /// SEQEND
    Seqend,
}

/// 続くレコードを親に付けるために見る値
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Followed {
    /// レコードの役
    pub kind: Follow,
    /// ハンドル（5 の最初の有効な値）
    pub handle: Option<u64>,
    /// 所有者（102 の組の外の最初の 330）
    pub owner: Option<u64>,
}

/// 続くレコード `follower` が、親 `parent` に付く種類か
pub fn followed_expected(follower: Follow, parent: Follow) -> bool {
    matches!(
        (follower, parent),
        (Follow::Attrib, Follow::Insert { .. })
            | (Follow::Vertex, Follow::Polyline { .. })
            | (
                Follow::Seqend,
                Follow::Insert { .. } | Follow::Polyline { .. }
            )
    )
}

/// 共有の表のオブジェクトから、親子の対応付けに見る値を読む
pub fn followed_object(object: &Object) -> Option<Followed> {
    let kind = match object.kind.as_slice() {
        b"INSERT" => Follow::Insert {
            opens: matches!(object.first(66), Some(Data::Int(1) | Data::Bool(true))),
        },
        b"POLYLINE" => Follow::Polyline {
            opens: !matches!(object.first(66), Some(Data::Int(0) | Data::Bool(false))),
        },
        b"ATTRIB" => Follow::Attrib,
        b"VERTEX" => Follow::Vertex,
        b"SEQEND" => Follow::Seqend,
        _ => return None,
    };
    Some(Followed {
        kind,
        handle: object.handle(),
        owner: object.owner(),
    })
}

/// DXF の生のタグの列から、親子の対応付けに見る値を読む
pub fn followed_tags(name: &[u8], tags: &[RawTag<'_>]) -> Option<Followed> {
    let first_int = |code: i16| {
        tags.iter()
            .find(|tag| tag.code == code)
            .map(|tag| dxf::parse_raw(name, code, tag.value))
    };
    let kind = match name {
        b"INSERT" => Follow::Insert {
            opens: matches!(
                first_int(66),
                Some(Some(RawData::Int(1) | RawData::Bool(true)))
            ),
        },
        b"POLYLINE" => Follow::Polyline {
            opens: !matches!(
                first_int(66),
                Some(Some(RawData::Int(0) | RawData::Bool(false)))
            ),
        },
        b"ATTRIB" => Follow::Attrib,
        b"VERTEX" => Follow::Vertex,
        b"SEQEND" => Follow::Seqend,
        _ => return None,
    };
    let handle = tags
        .iter()
        .filter(|tag| tag.code == 5)
        .find_map(|tag| dxf::parse_hex(tag.value.trim_ascii()));
    let mut owner = None;
    let mut depth = 0usize;
    for tag in tags {
        match tag.code {
            102 if tag.value.starts_with(b"{") => depth += 1,
            102 if tag.value.starts_with(b"}") => depth = depth.saturating_sub(1),
            330 if depth == 0 => {
                owner = dxf::parse_hex(tag.value.trim_ascii());
                break;
            }
            _ => {}
        }
    }
    Some(Followed {
        kind,
        handle,
        owner,
    })
}

/// 1 オブジェクトの持ち上げが失敗した理由
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LiftFail {
    /// グループ・コードの値が型・範囲に合わない
    Invalid(i16),
    /// 押し出しの向きや Z 座標が平面（Z = 0）でない
    NonPlanar,
}

/// 1 オブジェクトのグループ・コードごとの最初の値を読む口
pub trait FieldSource<'a> {
    /// 有限の実数。なければ `default`、それもなければ `Invalid(code)`
    fn real(&self, code: i16, default: Option<f64>) -> Result<f64, LiftFail>;
    /// 整数。なければ `default`
    fn int(&self, code: i16, default: i64) -> Result<i64, LiftFail>;
    /// 整数。なければ `None`
    fn raw_int(&self, code: i16) -> Result<Option<i64>, LiftFail>;
    /// 文字のバイト列。なければ `None`
    fn raw_text(&self, code: i16) -> Result<Option<&'a [u8]>, LiftFail>;
    /// グループ・コードがあるか
    fn has_code(&self, code: i16) -> bool;
    /// INSERT の 66 が 0 でない整数か
    fn insert_followed(&self) -> bool;
}

impl<'a> FieldSource<'a> for &'a Object {
    fn real(&self, code: i16, default: Option<f64>) -> Result<f64, LiftFail> {
        match self.first(code) {
            Some(Data::Real(value)) if value.is_finite() => Ok(*value),
            Some(_) => Err(LiftFail::Invalid(code)),
            None => default.ok_or(LiftFail::Invalid(code)),
        }
    }

    fn int(&self, code: i16, default: i64) -> Result<i64, LiftFail> {
        match self.first(code) {
            Some(Data::Int(value)) => Ok(*value),
            Some(_) => Err(LiftFail::Invalid(code)),
            None => Ok(default),
        }
    }

    fn raw_int(&self, code: i16) -> Result<Option<i64>, LiftFail> {
        match self.first(code) {
            None => Ok(None),
            Some(Data::Int(value)) => Ok(Some(*value)),
            Some(_) => Err(LiftFail::Invalid(code)),
        }
    }

    fn raw_text(&self, code: i16) -> Result<Option<&'a [u8]>, LiftFail> {
        match self.first(code) {
            None => Ok(None),
            Some(Data::Text(bytes)) => Ok(Some(bytes.as_slice())),
            Some(_) => Err(LiftFail::Invalid(code)),
        }
    }

    fn has_code(&self, code: i16) -> bool {
        self.first(code).is_some()
    }

    fn insert_followed(&self) -> bool {
        matches!(self.first(66), Some(Data::Int(value)) if *value != 0)
    }
}

const SLOTS: usize = 38;

const fn slot(code: i16) -> usize {
    match code {
        1 => 1,
        6 => 2,
        7 => 3,
        8 => 4,
        10 => 5,
        11 => 6,
        20 => 7,
        21 => 8,
        30 => 9,
        31 => 10,
        38 => 11,
        40 => 12,
        41 => 13,
        43 => 14,
        48 => 15,
        50 => 16,
        51 => 17,
        62 => 18,
        67 => 19,
        70 => 20,
        71 => 21,
        72 => 22,
        73 => 23,
        90 => 24,
        210 => 25,
        220 => 26,
        230 => 27,
        370 => 28,
        420 => 29,
        440 => 30,
        2 => 31,
        42 => 32,
        44 => 33,
        45 => 34,
        66 => 35,
        74 => 36,
        101 => 37,
        _ => 0,
    }
}

/// DXF のタグの列から、持ち上げで使うグループ・コードの最初の値を集めた表（ヒープ確保なし）
pub struct DxfFields<'a> {
    first: [Option<&'a [u8]>; SLOTS],
    /// ハンドル（5 の、16 進として読める最初の値）
    pub handle: Option<u64>,
}

impl<'a> DxfFields<'a> {
    /// タグの列から最初の値の表を作る
    pub fn new(tags: &[RawTag<'a>]) -> Self {
        let mut first: [Option<&'a [u8]>; SLOTS] = [None; SLOTS];
        let mut handle = None;
        for tag in tags {
            if tag.code == 5 && handle.is_none() {
                handle = dxf::parse_hex(tag.value.trim_ascii());
                continue;
            }
            let at = slot(tag.code);
            if at != 0
                && let Some(place) = first.get_mut(at)
                && place.is_none()
            {
                *place = Some(tag.value);
            }
        }
        Self { first, handle }
    }

    /// グループ・コードの最初の生の値
    pub fn raw(&self, code: i16) -> Option<&'a [u8]> {
        let at = slot(code);
        if at == 0 {
            return None;
        }
        self.first.get(at).copied().flatten()
    }

    fn first_typed(&self, code: i16) -> Option<Option<RawData<'a>>> {
        self.raw(code).map(|raw| dxf::parse_raw(b"", code, raw))
    }
}

impl<'a> FieldSource<'a> for DxfFields<'a> {
    fn real(&self, code: i16, default: Option<f64>) -> Result<f64, LiftFail> {
        match self.raw(code) {
            Some(raw) => dxf::parse_real(raw).ok_or(LiftFail::Invalid(code)),
            None => default.ok_or(LiftFail::Invalid(code)),
        }
    }

    fn int(&self, code: i16, default: i64) -> Result<i64, LiftFail> {
        match self.first_typed(code) {
            Some(Some(RawData::Int(value))) => Ok(value),
            Some(_) => Err(LiftFail::Invalid(code)),
            None => Ok(default),
        }
    }

    fn raw_int(&self, code: i16) -> Result<Option<i64>, LiftFail> {
        match self.first_typed(code) {
            None => Ok(None),
            Some(Some(RawData::Int(value))) => Ok(Some(value)),
            Some(_) => Err(LiftFail::Invalid(code)),
        }
    }

    fn raw_text(&self, code: i16) -> Result<Option<&'a [u8]>, LiftFail> {
        match self.first_typed(code) {
            None => Ok(None),
            Some(Some(RawData::Text(bytes))) => Ok(Some(bytes)),
            Some(_) => Err(LiftFail::Invalid(code)),
        }
    }

    fn has_code(&self, code: i16) -> bool {
        self.raw(code).is_some()
    }

    fn insert_followed(&self) -> bool {
        matches!(self.first_typed(66), Some(Some(RawData::Int(value))) if value != 0)
    }
}

/// 押し出しの向き（210・220・230）が既定の `(0, 0, 1)` か確かめる
pub fn check_upright<'a>(fields: &impl FieldSource<'a>) -> Result<(), LiftFail> {
    let extrusion = [
        fields.real(210, Some(0.0))?,
        fields.real(220, Some(0.0))?,
        fields.real(230, Some(1.0))?,
    ];
    if extrusion == [0.0, 0.0, 1.0] {
        Ok(())
    } else {
        Err(LiftFail::NonPlanar)
    }
}

/// Z（`x + 20`）が 0 の平面の点（`x`・`x + 10`）を読む
pub fn planar_point<'a>(fields: &impl FieldSource<'a>, x: i16) -> Result<[f64; 2], LiftFail> {
    if fields.real(x + 20, Some(0.0))? != 0.0 {
        return Err(LiftFail::NonPlanar);
    }
    Ok([fields.real(x, None)?, fields.real(x + 10, None)?])
}

/// 半径（40）。正の有限数でなければ `Invalid(40)`
pub fn positive_radius<'a>(fields: &impl FieldSource<'a>) -> Result<f64, LiftFail> {
    let radius = fields.real(40, None)?;
    if radius > 0.0 {
        Ok(radius)
    } else {
        Err(LiftFail::Invalid(40))
    }
}

/// LINE の始点と終点 `[x0, y0, x1, y1]` を読む
pub fn line_ends<'a>(fields: &impl FieldSource<'a>) -> Result<[f64; 4], LiftFail> {
    let end = |x: i16| -> Result<[f64; 3], LiftFail> {
        let z = fields.real(x + 20, Some(0.0))?;
        Ok([fields.real(x, None)?, fields.real(x + 10, None)?, z])
    };
    let (start, end) = (end(10)?, end(11)?);
    if start[2] != 0.0 || end[2] != 0.0 {
        return Err(LiftFail::NonPlanar);
    }
    Ok([start[0], start[1], end[0], end[1]])
}

/// CIRCLE の `[x, y, radius]` を読む
pub fn circle_geom<'a>(fields: &impl FieldSource<'a>) -> Result<[f64; 3], LiftFail> {
    let [x, y] = planar_point(fields, 10)?;
    let radius = positive_radius(fields)?;
    Ok([x, y, radius])
}

/// ARC の `[x, y, radius, start_deg, end_deg]` を読む
pub fn arc_geom<'a>(fields: &impl FieldSource<'a>) -> Result<[f64; 5], LiftFail> {
    let [x, y] = planar_point(fields, 10)?;
    let radius = positive_radius(fields)?;
    let start = fields.real(50, None)?;
    let end = fields.real(51, None)?;
    Ok([x, y, radius, start, end])
}

/// LWPOLYLINE の頂点列を受け取る口
pub trait PolylineSink {
    /// これまでに足した頂点の数
    fn len(&self) -> usize;
    /// 頂点数が 0 か
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// 頂点 `(x, y)` を足す
    fn push_vertex(&mut self, x: f64, y: f64);
    /// 直前の頂点の始まり（`is_end = false`）か終わり（`true`）の幅（40・41）を設定する
    fn set_width(&mut self, is_end: bool, width: f64);
    /// 直前の頂点の膨らみ（42）を設定する
    fn set_bulge(&mut self, bulge: f64);
}

/// LWPOLYLINE の値列を先頭から順に読む口
pub trait PolylineStream {
    /// 次の要素のグループ・コード
    fn peek_code(&self) -> Option<i16>;
    /// 先頭を 1 つ進める
    fn skip(&mut self);
    /// 次の要素が `code` の有限実数なら進めて返す
    fn take_real(&mut self, code: i16) -> Result<f64, LiftFail>;
}

impl PolylineStream for &[Value] {
    fn peek_code(&self) -> Option<i16> {
        self.first().map(|v| v.code)
    }

    fn skip(&mut self) {
        *self = self.get(1..).unwrap_or_default();
    }

    fn take_real(&mut self, code: i16) -> Result<f64, LiftFail> {
        match self.split_first() {
            Some((
                Value {
                    code: found,
                    data: Data::Real(v),
                },
                rest,
            )) if *found == code && v.is_finite() => {
                *self = rest;
                Ok(*v)
            }
            _ => Err(LiftFail::Invalid(code)),
        }
    }
}

impl<'a> PolylineStream for &'a [RawTag<'a>] {
    fn peek_code(&self) -> Option<i16> {
        self.first().map(|t| t.code)
    }

    fn skip(&mut self) {
        *self = self.get(1..).unwrap_or_default();
    }

    fn take_real(&mut self, code: i16) -> Result<f64, LiftFail> {
        match self.split_first() {
            Some((tag, rest)) if tag.code == code => {
                let v = dxf::parse_real(tag.value).ok_or(LiftFail::Invalid(code))?;
                *self = rest;
                Ok(v)
            }
            _ => Err(LiftFail::Invalid(code)),
        }
    }
}

/// LWPOLYLINE の全体の属性
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LwPolylineMeta {
    /// 閉じているか（70 の 1）
    pub closed: bool,
    /// 線種の生成を行うか（70 の 128）
    pub line_type_generation: bool,
    /// 一定の幅（43）。0 なら `None`
    pub width: Option<f64>,
}

/// LWPOLYLINE の頂点列と全体属性を読む
pub fn lwpolyline_scan<'a>(
    fields: &impl FieldSource<'a>,
    mut stream: impl PolylineStream,
    sink: &mut impl PolylineSink,
) -> Result<LwPolylineMeta, LiftFail> {
    if fields.real(38, Some(0.0))? != 0.0 {
        return Err(LiftFail::NonPlanar);
    }
    while let Some(code) = stream.peek_code() {
        match code {
            10 => {
                let x = stream.take_real(10)?;
                let y = stream.take_real(20)?;
                sink.push_vertex(x, y);
            }
            40..=42 => {
                let value = stream.take_real(code)?;
                if sink.is_empty() {
                    return Err(LiftFail::Invalid(code));
                }
                match code {
                    40 => sink.set_width(false, value),
                    41 => sink.set_width(true, value),
                    _ => sink.set_bulge(value),
                }
            }
            _ => stream.skip(),
        }
    }
    if usize::try_from(fields.int(90, -1)?).ok() != Some(sink.len()) {
        return Err(LiftFail::Invalid(90));
    }
    let width = match fields.real(43, Some(0.0))? {
        0.0 => None,
        width if width > 0.0 => Some(width),
        _ => return Err(LiftFail::Invalid(43)),
    };
    let flags = fields.int(70, 0)?;
    Ok(LwPolylineMeta {
        closed: flags & 1 != 0,
        line_type_generation: flags & 128 != 0,
        width,
    })
}

/// TEXT・ATTRIB の配置（生の座標）
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RawTextPlace {
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
        /// 始点
        first: [f64; 2],
        /// 終点
        second: [f64; 2],
        /// 収め方
        fit: Fit,
    },
}

/// TEXT・ATTRIB の生の形と文字列フィールド
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RawText<'a> {
    /// 配置
    pub place: RawTextPlace,
    /// 内容（1）
    pub content: &'a [u8],
    /// 高さ（40）
    pub height: f64,
    /// 回転（50、度）
    pub rotation: f64,
    /// 幅の係数（41）
    pub width_factor: f64,
    /// 傾き（51、度）
    pub oblique: f64,
    /// 左右反転（71 の 2）
    pub backward: bool,
    /// 上下反転（71 の 4）
    pub upside_down: bool,
    /// 文字スタイルの名前（7）。省略時は `None`
    pub style: Option<&'a [u8]>,
}

/// TEXT（`vertical_code = 73`）または ATTRIB（`74`）の生の値を読む
pub fn text_geom<'a>(
    fields: &impl FieldSource<'a>,
    vertical_code: i16,
) -> Result<RawText<'a>, LiftFail> {
    let first = planar_point(fields, 10)?;
    let horizontal = fields.int(72, 0)?;
    let vertical = fields.int(vertical_code, 0)?;
    let place = match (horizontal, vertical) {
        (0, 0) => RawTextPlace::Anchor {
            point: first,
            horizontal: Horizontal::Left,
            vertical: Vertical::Baseline,
        },
        (3, _) | (5, _) => RawTextPlace::Between {
            first,
            second: planar_point(fields, 11)?,
            fit: if horizontal == 3 {
                Fit::Aligned
            } else {
                Fit::Fit
            },
        },
        _ => RawTextPlace::Anchor {
            point: planar_point(fields, 11)?,
            horizontal: match horizontal {
                0 => Horizontal::Left,
                1 => Horizontal::Center,
                2 => Horizontal::Right,
                4 => Horizontal::Middle,
                _ => return Err(LiftFail::Invalid(72)),
            },
            vertical: match vertical {
                0 => Vertical::Baseline,
                1 => Vertical::Bottom,
                2 => Vertical::Middle,
                3 => Vertical::Top,
                _ => return Err(LiftFail::Invalid(vertical_code)),
            },
        },
    };
    let flags = fields.int(71, 0)?;
    let content = fields.raw_text(1)?.ok_or(LiftFail::Invalid(1))?;
    let height = fields.real(40, None)?;
    let rotation = fields.real(50, Some(0.0))?;
    let width_factor = fields.real(41, Some(1.0))?;
    let oblique = fields.real(51, Some(0.0))?;
    let style = fields.raw_text(7)?;
    Ok(RawText {
        place,
        content,
        height,
        rotation,
        width_factor,
        oblique,
        backward: flags & 2 != 0,
        upside_down: flags & 4 != 0,
        style,
    })
}

/// INSERT の生の形
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RawInsert<'a> {
    /// ブロック名（2）
    pub name: &'a [u8],
    /// 置く点（10・20）
    pub position: [f64; 2],
    /// 尺度（41・42）
    pub scale: [f64; 2],
    /// 回転（50、度）
    pub rotation: f64,
    /// 列と行の数（70・71）
    pub grid: [u32; 2],
    /// 列と行の間隔（44・45）。`grid == [1, 1]` のときは読まず `[0.0, 0.0]`
    pub spacing: [f64; 2],
    /// 66 が 0 でない整数か
    pub followed: bool,
}

/// INSERT の生の値を読む
pub fn insert_geom<'a>(fields: &impl FieldSource<'a>) -> Result<RawInsert<'a>, LiftFail> {
    let name = fields.raw_text(2)?.ok_or(LiftFail::Invalid(2))?;
    let mut scale = [1.0; 2];
    for (code, slot) in [41, 42].into_iter().zip(&mut scale) {
        *slot = fields.real(code, Some(1.0))?;
        if *slot == 0.0 {
            return Err(LiftFail::Invalid(code));
        }
    }
    let count = |code: i16| -> Result<u32, LiftFail> {
        u32::try_from(fields.int(code, 1)?)
            .ok()
            .filter(|count| *count > 0)
            .ok_or(LiftFail::Invalid(code))
    };
    let grid = [count(70)?, count(71)?];
    let spacing = if grid == [1, 1] {
        [0.0, 0.0]
    } else {
        [fields.real(44, Some(0.0))?, fields.real(45, Some(0.0))?]
    };
    let position = planar_point(fields, 10)?;
    let rotation = fields.real(50, Some(0.0))?;
    Ok(RawInsert {
        name,
        position,
        scale,
        rotation,
        grid,
        spacing,
        followed: fields.insert_followed(),
    })
}

/// 線種の生の指定（6）
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RawLineType<'a> {
    /// 画層に従う（省略または `ByLayer`）
    Layer,
    /// ブロックに従う（`ByBlock`）
    Block,
    /// 線種の名前
    Named(&'a [u8]),
}

/// 図形の共通スタイル（色・線種・線種尺度・線幅・透過性）の生の値
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EntityStyle<'a> {
    /// 色（62・420）
    pub color: Inherit<Color>,
    /// 線種（6）
    pub line_type: RawLineType<'a>,
    /// 線種の尺度（48）
    pub line_type_scale: f64,
    /// 線の太さ（370）
    pub line_weight: Inherit<LineWeight>,
    /// 透過性（440）
    pub transparency: Inherit<Transparency>,
}

/// 図形の共通スタイルを読む
pub fn entity_style<'a>(fields: &impl FieldSource<'a>) -> Result<EntityStyle<'a>, LiftFail> {
    let rgb = match fields.raw_int(420)? {
        None => None,
        Some(value @ 0..=0xFF_FFFF) => {
            let [_, r, g, b] = (value as u32).to_be_bytes();
            Some([r, g, b])
        }
        Some(_) => return Err(LiftFail::Invalid(420)),
    };
    let color = match (fields.int(62, 256)?, rgb) {
        (index @ 1..=255, _) => Inherit::Own(Color {
            code: Some(ColorCode {
                system: ColorSystem::Aci,
                value: index as u32,
            }),
            rgb,
        }),
        (_, Some(rgb)) => Inherit::Own(Color {
            code: None,
            rgb: Some(rgb),
        }),
        (0, None) => Inherit::Block,
        (256, None) => Inherit::Layer,
        _ => return Err(LiftFail::Invalid(62)),
    };
    let line_type = match fields.raw_text(6)? {
        None => RawLineType::Layer,
        Some(name) if name.eq_ignore_ascii_case(b"ByLayer") => RawLineType::Layer,
        Some(name) if name.eq_ignore_ascii_case(b"ByBlock") => RawLineType::Block,
        Some(name) => RawLineType::Named(name),
    };
    let line_type_scale = fields.real(48, Some(1.0))?;
    if line_type_scale <= 0.0 {
        return Err(LiftFail::Invalid(48));
    }
    let line_weight = match fields.raw_int(370)? {
        None | Some(-1) => Inherit::Layer,
        Some(-2) => Inherit::Block,
        Some(-3) => Inherit::Own(LineWeight::Default),
        Some(width @ 0..=211) => Inherit::Own(LineWeight::Width(PaperLength(width as f64 / 100.0))),
        Some(_) => return Err(LiftFail::Invalid(370)),
    };
    let transparency = match fields.raw_int(440)? {
        None => Inherit::Layer,
        Some(value) => decode_transparency(value).ok_or(LiftFail::Invalid(440))?,
    };
    Ok(EntityStyle {
        color,
        line_type,
        line_type_scale,
        line_weight,
        transparency,
    })
}

/// モデル空間の 1 図形を置き換えた後の値。書き戻せなければ `None`
pub fn replace_model(
    was: &Element<ModelPoint>,
    now: &Element<ModelPoint>,
    kind: &[u8],
    old: &[Value],
    charset: Charset,
) -> Option<Vec<Value>> {
    let space = anchor(&was.shape)?.space;
    let object = Object {
        kind: kind.to_vec(),
        section: Section::Entities,
        values: old.to_vec(),
        origin: Origin::New,
    };
    let encoder = Encoder::new(charset);
    replacement(was, now, &object, &encoder, &InSpace(space))
}

/// モデル空間の 1 図形（と、続く ATTRIB の列）を変換 `transform` で動かした後の値。
/// 主の図形の新しい値と、動かした ATTRIB ごとの新しい値を返す。書き戻せなければ `None`
pub fn move_model(
    was: &Element<ModelPoint>,
    kind: &[u8],
    old: &[Value],
    attribs: &[&[Value]],
    transform: &Affine,
    charset: Charset,
) -> Option<(Vec<Value>, Vec<Vec<Value>>)> {
    let space = anchor(&was.shape)?.space;
    let object = Object {
        kind: kind.to_vec(),
        section: Section::Entities,
        values: old.to_vec(),
        origin: Origin::New,
    };
    let follower_objs: Vec<Object> = attribs
        .iter()
        .map(|vals| Object {
            kind: b"ATTRIB".to_vec(),
            section: Section::Entities,
            values: (*vals).to_vec(),
            origin: Origin::New,
        })
        .collect();
    let indexed: Vec<(usize, &Object)> = follower_objs.iter().enumerate().collect();
    let encoder = Encoder::new(charset);
    let moved = moved_with_attribs(
        was,
        &object,
        &indexed,
        transform,
        &encoder,
        &InSpace(space),
        true,
    )?;
    let followers = moved.followers.into_iter().map(|(_, vals)| vals).collect();
    Some((moved.object, followers))
}

/// 1 図形の追加・スタイル変更に必要な、図面の名前表と設定
#[derive(Clone, Debug, Default)]
pub struct SingleTables {
    /// モデル空間のブロック・レコード（`*Model_Space`）のハンドル
    pub model_owner: Option<u64>,
    /// サブクラス・マーカー（100）を書くか
    pub markers: bool,
    /// `$ACADVER`（前後の空白を除いたもの）
    pub version: Vec<u8>,
    /// 文字の符号化
    pub charset: Charset,
    /// DWG への書き戻しか（`Some(handseed)` なら DWG、`None` なら DXF）
    pub dwg_seed: Option<Option<u64>>,
    /// 表（`LAYER`・`LTYPE`・`STYLE`・`BLOCK_RECORD`・`DIMSTYLE`）ごとの、小文字の名前から定義の綴りへの対応
    pub names: Arc<BTreeMap<&'static [u8], BTreeMap<String, String>>>,
    /// 定義のあるブロック（小文字の名前から定義の名前）
    pub blocks: Arc<BTreeMap<String, String>>,
}

impl SingleTables {
    fn to_born(
        &self,
        layers: BTreeMap<LayerId, String>,
        line_types: BTreeMap<LineTypeId, String>,
        next: Option<u64>,
    ) -> Born {
        let target = match self.dwg_seed {
            Some(seed) => Target::Dwg(seed),
            None => Target::Dxf,
        };
        Born::single(
            self.model_owner,
            self.markers,
            self.version.clone(),
            self.charset,
            target,
            Arc::clone(&self.names),
            layers,
            line_types,
            Arc::clone(&self.blocks),
            next,
        )
    }
}

/// スタイル変更で指定する線種
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RestyleLineType<'a> {
    /// 画層に従う
    Layer,
    /// ブロックに従う
    Block,
    /// 線種の名前
    Named(&'a str),
}

/// 1 図形の共通属性（画層・色・線種・線種の尺度・線の太さ）を変えた後の値。書き戻せなければ `None`
#[allow(clippy::too_many_arguments)]
pub fn restyle_model(
    tables: &SingleTables,
    kind: &[u8],
    old: &[Value],
    current_layer: &str,
    new_layer: Option<&str>,
    new_line_type: Option<RestyleLineType<'_>>,
    color: Option<Inherit<Color>>,
    line_type_scale: Option<f64>,
    line_weight: Option<Inherit<LineWeight>>,
) -> Option<Vec<Value>> {
    let object = Object {
        kind: kind.to_vec(),
        section: Section::Entities,
        values: old.to_vec(),
        origin: Origin::New,
    };
    let current = entity_style(&(&object)).ok()?;
    let mut layers = BTreeMap::from([(LayerId(0), current_layer.to_owned())]);
    let layer_change = match new_layer {
        None => None,
        Some(name) if name.eq_ignore_ascii_case(current_layer) => Some(LayerId(0)),
        Some(name) => {
            layers.insert(LayerId(1), name.to_owned());
            Some(LayerId(1))
        }
    };
    let mut line_types = BTreeMap::new();
    let old_line_type = match current.line_type {
        RawLineType::Layer => Inherit::Layer,
        RawLineType::Block => Inherit::Block,
        RawLineType::Named(bytes) => {
            let decoded = tables.charset.decode(bytes)?;
            line_types.insert(LineTypeId(0), decoded);
            Inherit::Own(LineTypeId(0))
        }
    };
    let lt_change = match new_line_type {
        None => None,
        Some(RestyleLineType::Layer) => Some(Inherit::Layer),
        Some(RestyleLineType::Block) => Some(Inherit::Block),
        Some(RestyleLineType::Named(name)) => {
            if line_types
                .get(&LineTypeId(0))
                .is_some_and(|old_name| name.eq_ignore_ascii_case(old_name))
            {
                Some(Inherit::Own(LineTypeId(0)))
            } else {
                line_types.insert(LineTypeId(1), name.to_owned());
                Some(Inherit::Own(LineTypeId(1)))
            }
        }
    };
    let old_style = Style {
        color: current.color,
        line_type: old_line_type,
        line_type_scale: current.line_type_scale,
        line_weight: Some(current.line_weight),
        transparency: current.transparency,
    };
    let change = zumen_drawing::Restyle {
        layer: layer_change,
        color,
        line_type: lt_change,
        line_type_scale,
        line_weight,
    };
    let born = tables.to_born(layers, line_types, None);
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
    restyled(&born, &object, LayerId(0), &old_style, &change, &mut cx)
        .ok()
        .flatten()
}

/// 足す 1 図形と、続くレコード（ATTRIB・SEQEND）の組（種類と、ハンドル 5 から始まる値）
pub type BornRecords = Vec<(&'static [u8], Vec<Value>)>;

/// モデル空間に 1 図形をハンドル `handle` で足すレコードの列を作る。書き戻せなければ `None`
pub fn born_model(
    tables: &SingleTables,
    element: &Element<ModelPoint>,
    layer_name: &str,
    line_type_name: Option<&str>,
    attrib_layers: &[(LayerId, String)],
    attrib_line_types: &[(LineTypeId, String)],
    handle: u64,
) -> Option<BornRecords> {
    let mut layers = BTreeMap::from([(element.layer, layer_name.to_owned())]);
    for (id, name) in attrib_layers {
        layers.insert(*id, name.clone());
    }
    let mut line_types = BTreeMap::new();
    if let (Inherit::Own(id), Some(name)) = (&element.style.line_type, line_type_name) {
        line_types.insert(*id, name.to_owned());
    }
    for (id, name) in attrib_line_types {
        line_types.insert(*id, name.clone());
    }
    let born = tables.to_born(layers, line_types, Some(handle));
    let rec = born.record(element, &InSpace(MODEL), born.model(), 0)?;
    if rec.annotation.is_some() {
        return None;
    }
    let mut out = Vec::with_capacity(1 + rec.followers.len());
    let mut next = handle;
    let objects = std::iter::once((rec.kind, rec.home.owner, rec.values)).chain(
        rec.followers
            .into_iter()
            .map(|(kind, values)| (kind, Some(handle), values)),
    );
    for (kind, owner, own) in objects {
        let mut values = vec![Value {
            code: 5,
            data: Data::Handle(next),
        }];
        if let Some(owner) = owner {
            values.push(Value {
                code: 330,
                data: Data::Handle(owner),
            });
        }
        values.extend(own);
        out.push((kind, values));
        next = next.checked_add(1)?;
    }
    Some(out)
}

/// オブジェクトが所有を示すハンドル（拡張辞書と DICTIONARY の項目）。
/// `is_root` が真のとき、拡張辞書のほかの 102 の組（リアクターなど）や組の外の 360 があれば `None`
pub fn owned_children(kind: &[u8], values: &[Value], is_root: bool) -> Option<Vec<(u64, bool)>> {
    children_of_values(kind, values, is_root)
}

/// 続くレコード（POLYLINE の VERTEX、66 が 0 でない INSERT の ATTRIB、SEQEND）を持つ図形か
pub fn is_followed(kind: &[u8], values: &[Value]) -> bool {
    followed_values(kind, values)
}

/// 図形とともに動く XDATA の最初のグループ・コード（1011〜1013・1041・1042）
pub fn moving_xdata(values: &[Value]) -> Option<i16> {
    moving_xdata_code(values)
}

/// 最大のハンドル `max` と種 `seed` から、次に付けるハンドルを求める
pub fn next_handle(max: Option<u64>, seed: Option<u64>) -> Option<u64> {
    next_handle_from(max, seed)
}
