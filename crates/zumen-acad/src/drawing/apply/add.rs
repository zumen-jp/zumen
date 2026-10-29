//! 図面モデルの実寸の空間の要素を、表のモデル空間の新しい図形として足す（決定 0155）。
//!
//! 足す図形は、レイヤ・線種・文字の体裁・ブロックを名前で指す。指す定義が図面になければ足さない。
//! 表が DXF の区切り（100）を持てば、DXF Reference の並びで区切りも書く。ハンドルは、表のどのハンドル
//! よりも大きく、見出しの HANDSEED より小さくない値から順に付ける。

use std::collections::BTreeMap;

use super::{Frame, InSpace, nonnegative, positive, real, turned};
use crate::db::{AcadDb, Data, Object, Origin, Section, Value};
use crate::drawing::{BLOCK, MODEL};
use zumen_drawing::{
    ColorSystem, Drawing2D, Edge, Element, Fit, HeightMeaning, Horizontal, Inherit, LayerId,
    LineTypeId, LineWeight, ModelPoint, Placement, Polyline, Shape, Symbol, Text, Vertical,
};

/// 足す図形の種類と、ハンドルと所有者の後の値
pub(super) type Record = (&'static [u8], Vec<Value>);

/// 書き戻す先
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Target {
    /// DXF。HANDSEED は表の `$HANDSEED` にある
    Dxf,
    /// DWG と、見出しの HANDSEED。DWG に足す図形は、まだ線の太さと RGB の色を持てない（決定 0079）
    Dwg(Option<u64>),
}

/// 足す図形のレコードを組むための、表の決まり
pub(super) struct Born {
    /// モデル空間のブロックのレコードのハンドル。ブロックのレコードを持たない図面なら `None`
    owner: Option<u64>,
    /// 図形が区切り（100）を持つ表か
    markers: bool,
    /// 版（`$ACADVER`）。なければ空
    version: Vec<u8>,
    /// 文字を UTF-8 で書けるか
    unicode: bool,
    target: Target,
    /// 表（LAYER・LTYPE・STYLE・BLOCK_RECORD）ごとの、定義の名前。小文字にした名前から引く
    names: BTreeMap<&'static [u8], BTreeMap<String, String>>,
    /// レイヤと線種の名前
    layers: BTreeMap<LayerId, String>,
    line_types: BTreeMap<LineTypeId, String>,
    /// 最初に付けるハンドル。表がハンドルを持たなければ `None`
    next: Option<u64>,
}

/// 名前で指す定義の表
const TABLES: [&[u8]; 4] = [b"LAYER", b"LTYPE", b"STYLE", b"BLOCK_RECORD"];

impl Born {
    pub(super) fn new(db: &AcadDb, drawing: &Drawing2D, unicode: bool, target: Target) -> Self {
        let name = |object: &Object| match object.first(2) {
            Some(Data::Text(name)) => Some(String::from_utf8_lossy(name).into_owned()),
            _ => None,
        };
        let mut names: BTreeMap<&'static [u8], BTreeMap<String, String>> = BTreeMap::new();
        let mut owner = None;
        for object in db.objects() {
            let Some(table) = TABLES.iter().find(|table| object.kind == **table) else {
                continue;
            };
            let Some(name) = name(object) else {
                continue;
            };
            if *table == b"BLOCK_RECORD" && name.eq_ignore_ascii_case("*Model_Space") {
                owner = object.handle();
            }
            names
                .entry(table)
                .or_default()
                .insert(name.to_lowercase(), name);
        }
        let markers = db
            .objects()
            .iter()
            .filter(|object| object.section == Section::Entities)
            .any(|object| object.values.iter().any(|value| value.code == 100));
        let version = match db.variable(b"$ACADVER").and_then(<[Value]>::first) {
            Some(Value {
                data: Data::Text(text),
                ..
            }) => text.trim_ascii().to_vec(),
            _ => Vec::new(),
        };
        let seed = match target {
            Target::Dwg(seed) => seed,
            Target::Dxf => db.variable(b"$HANDSEED").and_then(|values| {
                values.iter().find_map(|value| match value.data {
                    Data::Handle(seed) if value.code == 5 => Some(seed),
                    _ => None,
                })
            }),
        };
        let max = db.objects().iter().filter_map(Object::handle).max();
        let next = match (seed, max) {
            (None, None) => None,
            (seed, max) => Some(
                seed.unwrap_or(0)
                    .max(max.map_or(1, |max| max.saturating_add(1))),
            ),
        };
        Born {
            owner,
            markers,
            version,
            unicode,
            target,
            names,
            layers: drawing
                .layers
                .iter()
                .map(|layer| (layer.id, layer.name.clone()))
                .collect(),
            line_types: drawing
                .line_types
                .iter()
                .map(|line_type| (line_type.id, line_type.name.clone()))
                .collect(),
            next,
        }
    }

    /// 表の `table` の、名前 `name` の定義を指す値。名前は大文字と小文字を区別せずに引き、定義の名前を書く。
    /// 定義がなければ `None`。DXF で、その表を持たない図面なら、名前だけで指す
    fn named(&self, code: i16, table: &[u8], name: &str) -> Option<Value> {
        let name = match self.names.get(table) {
            Some(names) => names.get(&name.to_lowercase())?,
            None if self.target == Target::Dxf => name,
            None => return None,
        };
        self.string(code, name)
    }

    /// 文字の値。2007 より前の版には、ASCII の文字だけを書く（読む側と同じ）
    fn string(&self, code: i16, value: &str) -> Option<Value> {
        (self.unicode || value.is_ascii()).then(|| text(code, value))
    }

    /// 版が `release` 以後か
    fn since(&self, release: &[u8]) -> bool {
        self.version.as_slice() >= release
    }

    /// 要素を足す図形のレコード。表せない要素なら `None`
    pub(super) fn record(&self, element: &Element<ModelPoint>) -> Option<Record> {
        // 足した図形は、ハンドルで指されるので、ハンドルを付けられない図面には足さない
        self.next?;
        let mut values = vec![text(100, "AcDbEntity")];
        let layer = self.layers.get(&element.layer)?;
        values.push(self.named(8, b"LAYER", layer)?);
        let style = &element.style;
        match style.line_type {
            Inherit::Layer => {}
            Inherit::Block => values.push(text(6, "ByBlock")),
            Inherit::Own(id) => {
                let name = self.line_types.get(&id)?;
                values.push(self.named(6, b"LTYPE", name)?);
            }
        }
        let mut rgb = None;
        match style.color {
            Inherit::Layer => {}
            Inherit::Block => values.push(int(62, 0)),
            Inherit::Own(color) => {
                match color.code {
                    Some(code)
                        if code.system == ColorSystem::Aci && (1..=255).contains(&code.value) =>
                    {
                        values.push(int(62, i64::from(code.value)));
                    }
                    Some(_) => return None,
                    None if color.rgb.is_none() => return None,
                    None => {}
                }
                rgb = color.rgb;
            }
        }
        match style.line_weight {
            None | Some(Inherit::Layer) => {}
            Some(weight) if self.target == Target::Dxf && self.since(b"AC1015") => {
                values.push(int(370, line_weight(weight)?));
            }
            Some(_) => return None,
        }
        if style.line_type_scale != 1.0 {
            values.push(real(48, nonzero_positive(style.line_type_scale)?)?);
        }
        if let Some([r, g, b]) = rgb {
            if !(self.target == Target::Dxf && self.since(b"AC1018")) {
                return None;
            }
            values.push(int(420, i64::from(u32::from_be_bytes([0, r, g, b]))));
        }
        let frame = InSpace(MODEL);
        let kind = match &element.shape {
            Shape::Line { start, end } | Shape::Curve(Edge::Line { start, end }) => {
                values.push(text(100, "AcDbLine"));
                point(&mut values, &frame, 10, start)?;
                point(&mut values, &frame, 11, end)?;
                b"LINE".as_slice()
            }
            Shape::Circle { center, radius } => {
                values.push(text(100, "AcDbCircle"));
                point(&mut values, &frame, 10, center)?;
                values.push(real(40, positive(&frame, radius)?)?);
                b"CIRCLE"
            }
            Shape::Curve(Edge::Arc {
                center,
                radius,
                start,
                end,
                counterclockwise,
            }) => {
                let (start, end) = turned(*start, *end, *counterclockwise);
                values.push(text(100, "AcDbCircle"));
                point(&mut values, &frame, 10, center)?;
                values.push(real(40, positive(&frame, radius)?)?);
                values.push(text(100, "AcDbArc"));
                values.push(real(50, start)?);
                values.push(real(51, end)?);
                b"ARC"
            }
            Shape::Curve(Edge::Elliptic {
                center,
                major,
                ratio,
                start,
                end,
                counterclockwise,
            }) => {
                let (start, end) = turned(*start, *end, *counterclockwise);
                let [x, y] = frame.point(center)?;
                let [end_x, end_y] = frame.point(major)?;
                let axis = [end_x - x, end_y - y];
                if axis == [0.0, 0.0] || !(*ratio > 0.0 && *ratio <= 1.0) {
                    return None;
                }
                values.push(text(100, "AcDbEllipse"));
                point(&mut values, &frame, 10, center)?;
                values.extend([real(11, axis[0])?, real(21, axis[1])?, real(31, 0.0)?]);
                values.push(real(40, *ratio)?);
                values.push(real(41, start.to_radians())?);
                values.push(real(42, end.to_radians())?);
                b"ELLIPSE"
            }
            Shape::Polyline(line) => {
                values.push(text(100, "AcDbPolyline"));
                lwpolyline(&mut values, &frame, line)?;
                b"LWPOLYLINE"
            }
            Shape::Text(text) => {
                self.lettering(&mut values, &frame, text)?;
                b"TEXT"
            }
            Shape::Symbol(symbol) => {
                self.placing(&mut values, &frame, symbol)?;
                b"INSERT"
            }
            _ => return None,
        };
        if !self.markers {
            values.retain(|value| value.code != 100);
        }
        Some((kind, values))
    }

    /// TEXT（DXF Reference「TEXT」）の値。位置合わせが左の基準線でなければ、11 を基点とし、10 も同じ点にする。
    /// 10 は位置合わせから決まる点で、読み直すときに使わず、AutoCAD は図面を開き直すと決め直す
    fn lettering(
        &self,
        values: &mut Vec<Value>,
        frame: &InSpace,
        lettering: &Text<ModelPoint>,
    ) -> Option<()> {
        if lettering.height.meaning != HeightMeaning::Declared("dxf.TEXT.40") {
            return None;
        }
        let style = self.named(7, b"STYLE", &lettering.style)?;
        let content = &lettering.content;
        // DXF の値は 1 行なので、改行は書けない
        if content.contains(['\n', '\r']) {
            return None;
        }
        let width = nonzero_positive(lettering.width_factor)?;
        let (first, second, horizontal, vertical) = match &lettering.placement {
            Placement::Anchor {
                point,
                horizontal,
                vertical,
            } => {
                let horizontal = match horizontal {
                    Horizontal::Left => 0,
                    Horizontal::Center => 1,
                    Horizontal::Right => 2,
                    Horizontal::Middle => 4,
                };
                let vertical = match vertical {
                    Vertical::Baseline => 0,
                    Vertical::Bottom => 1,
                    Vertical::Middle => 2,
                    Vertical::Top => 3,
                };
                let aligned = (horizontal, vertical) != (0, 0);
                (point, aligned.then_some(point), horizontal, vertical)
            }
            Placement::Between { first, second, fit } => {
                let horizontal = match fit {
                    Fit::Aligned => 3,
                    Fit::Fit => 5,
                };
                (first, Some(second), horizontal, 0)
            }
        };
        values.push(text(100, "AcDbText"));
        point(values, frame, 10, first)?;
        values.push(real(40, positive(frame, &lettering.height.value)?)?);
        values.push(self.string(1, content)?);
        if lettering.rotation != 0.0 {
            values.push(real(50, lettering.rotation)?);
        }
        if width != 1.0 {
            values.push(real(41, width)?);
        }
        if lettering.oblique != 0.0 {
            values.push(real(51, lettering.oblique)?);
        }
        values.push(style);
        let flags = i64::from(lettering.backward) * 2 + i64::from(lettering.upside_down) * 4;
        if flags != 0 {
            values.push(int(71, flags));
        }
        if horizontal != 0 {
            values.push(int(72, horizontal));
        }
        if let Some(second) = second {
            point(values, frame, 11, second)?;
        }
        values.push(text(100, "AcDbText"));
        if vertical != 0 {
            values.push(int(73, vertical));
        }
        Some(())
    }

    /// INSERT（DXF Reference「INSERT」）の値。格子・属性・切り抜きを持つ記号と、空間のブロックは足さない
    fn placing(
        &self,
        values: &mut Vec<Value>,
        frame: &InSpace,
        symbol: &Symbol<ModelPoint>,
    ) -> Option<()> {
        let name = &symbol.key.name;
        let space = name.to_lowercase();
        if symbol.key.namespace != BLOCK
            || symbol.grid.is_some()
            || !symbol.attributes.is_empty()
            || symbol.clip.is_some()
            || space.starts_with("*model_space")
            || space.starts_with("*paper_space")
        {
            return None;
        }
        values.push(text(100, "AcDbBlockReference"));
        values.push(self.named(2, b"BLOCK_RECORD", name)?);
        point(values, frame, 10, &symbol.position)?;
        let [x, y] = symbol.scale;
        for (code, scale) in [(41, x), (42, y)] {
            if !(scale.is_finite() && scale != 0.0) {
                return None;
            }
            if scale != 1.0 {
                values.push(real(code, scale)?);
            }
        }
        if symbol.rotation != 0.0 {
            values.push(real(50, symbol.rotation)?);
        }
        Some(())
    }

    /// 足す図形を、表のモデル空間に足す。ハンドルを順に付け、所有者をモデル空間のブロックにする
    pub(super) fn push(&self, db: &mut AcadDb, added: Vec<Record>) {
        let Some(mut next) = self.next else {
            return;
        };
        for (kind, own) in added {
            let mut values = vec![Value {
                code: 5,
                data: Data::Handle(next),
            }];
            if let Some(owner) = self.owner {
                values.push(Value {
                    code: 330,
                    data: Data::Handle(owner),
                });
            }
            values.extend(own);
            db.push(Object {
                kind: kind.to_vec(),
                section: Section::Entities,
                values,
                origin: Origin::New,
            });
            next = next.saturating_add(1);
        }
    }
}

/// 足せない要素の診断に書く、形の種類
pub(super) fn kind_of<P>(shape: &Shape<P>) -> &'static str {
    match shape {
        Shape::Line { .. } | Shape::Curve(Edge::Line { .. }) => "LINE",
        Shape::Circle { .. } => "CIRCLE",
        Shape::Curve(Edge::Arc { .. }) => "ARC",
        Shape::Curve(Edge::Elliptic { .. }) => "ELLIPSE",
        Shape::Curve(Edge::Spline(_)) => "SPLINE",
        Shape::Curve(Edge::Clothoid { .. }) => "CLOTHOID",
        Shape::Polyline(_) => "LWPOLYLINE",
        Shape::Text(_) => "TEXT",
        Shape::Formatted(_) => "MTEXT",
        Shape::Dimension(_) => "DIMENSION",
        Shape::Leader(_) => "LEADER",
        Shape::Callout(_) => "MULTILEADER",
        Shape::Table(_) => "ACAD_TABLE",
        Shape::Underlay(_) => "UNDERLAY",
        Shape::Symbol(_) => "INSERT",
        Shape::Hatch(_) => "HATCH",
    }
}

/// LWPOLYLINE（DXF Reference「LWPOLYLINE」）の値。頂点の数（90）・閉じていること（70）・一定の幅（43）と、
/// 頂点ごとに 10・20、頂点ごとの幅（40・41）、0 でない膨らみ（42）を並べる
fn lwpolyline(values: &mut Vec<Value>, frame: &InSpace, line: &Polyline<ModelPoint>) -> Option<()> {
    let count = line.vertices.len();
    let widths = !line.vertex_widths.is_empty();
    if count == 0 || (widths && (line.vertex_widths.len() != count || line.width.is_some())) {
        return None;
    }
    values.push(int(90, i64::try_from(count).ok()?));
    values.push(int(70, i64::from(line.closed)));
    if let Some(width) = &line.width {
        values.push(real(43, nonnegative(frame, width)?)?);
    }
    for (index, corner) in line.vertices.iter().enumerate() {
        let [x, y] = frame.point(&corner.point)?;
        values.extend([real(10, x)?, real(20, y)?]);
        if let Some([start, end]) = line.vertex_widths.get(index) {
            values.push(real(40, nonnegative(frame, start)?)?);
            values.push(real(41, nonnegative(frame, end)?)?);
        }
        if corner.bulge != 0.0 {
            values.push(real(42, corner.bulge)?);
        }
    }
    Some(())
}

/// 点の X・Y・Z（`code`、`code + 10`、`code + 20`）。Z は 0
fn point(values: &mut Vec<Value>, frame: &InSpace, code: i16, at: &ModelPoint) -> Option<()> {
    let [x, y] = frame.point(at)?;
    values.extend([real(code, x)?, real(code + 10, y)?, real(code + 20, 0.0)?]);
    Some(())
}

/// 0 より大きい有限の値
fn nonzero_positive(value: f64) -> Option<f64> {
    (value.is_finite() && value > 0.0).then_some(value)
}

/// 線幅（370）。-2 は ByBlock、-3 は文書の既定、0〜211 は 1/100 mm
fn line_weight(weight: Inherit<LineWeight>) -> Option<i64> {
    match weight {
        Inherit::Layer => Some(-1),
        Inherit::Block => Some(-2),
        Inherit::Own(LineWeight::Default) => Some(-3),
        Inherit::Own(LineWeight::Width(width)) => {
            let hundredths = width.0 * 100.0;
            let rounded = hundredths.round();
            ((hundredths - rounded).abs() < 1e-9 && (0.0..=211.0).contains(&rounded))
                .then_some(rounded as i64)
        }
    }
}

fn text(code: i16, value: &str) -> Value {
    Value {
        code,
        data: Data::Text(value.as_bytes().to_vec()),
    }
}

fn int(code: i16, value: i64) -> Value {
    Value {
        code,
        data: Data::Int(value),
    }
}
