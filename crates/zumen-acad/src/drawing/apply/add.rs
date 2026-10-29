//! 図面モデルの実寸の空間と用紙の要素を、表の新しい図形として足す（決定 0155）。実寸の空間の要素は
//! モデル空間に、用紙の要素は、その用紙のほかの図形と同じ所（今のレイアウトのペーパー空間か、ほかの
//! レイアウトのペーパー空間のブロック）に足す。
//!
//! 足す図形は、レイヤ・線種・文字の体裁・ブロックを名前で指す。指す定義が図面になければ足さない。
//! 表が DXF の区切り（100）を持てば、DXF Reference の並びで区切りも書く。ハンドルは、表のどのハンドル
//! よりも大きく、見出しの HANDSEED より小さくない値から順に付ける。

use std::collections::{BTreeMap, BTreeSet};

use super::{Frame, contents, nonnegative, positive, real, turned};
use crate::db::{AcadDb, Data, Object, Origin, Section, Value};
use crate::drawing::{BLOCK, Home, mtext};
use zumen_drawing::{
    Attribute, ColorSystem, Drawing2D, Edge, Element, Fit, Flow, FormattedText, HeightMeaning,
    Horizontal, Inherit, LayerId, LineTypeId, LineWeight, Placement, Polyline, Rotation, Shape,
    Style, Symbol, Text, Vertical,
};

/// 足す図形
pub(super) struct Record {
    kind: &'static [u8],
    /// ハンドルと所有者の後の値
    values: Vec<Value>,
    /// 置き場所
    home: Home,
    /// 図形の後に続き、図形が所有するレコード（ATTRIB と SEQEND）。種類と、ハンドルと所有者の後の値
    followers: Vec<(&'static [u8], Vec<Value>)>,
}

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
    /// ENDBLK を持つブロックのレコードのハンドル。BLOCKS の所に足す図形は、その ENDBLK の前に置く
    ends: BTreeSet<u64>,
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
        let ends = db
            .objects()
            .iter()
            .filter(|object| object.kind == b"ENDBLK")
            .filter_map(Object::owner)
            .collect();
        Born {
            ends,
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

    /// モデル空間の図形の置き場所
    pub(super) fn model(&self) -> Home {
        Home {
            owner: self.owner,
            blocks: false,
            paper: false,
        }
    }

    /// 要素を、座標の対応 `frame` で `home` に足す図形。表せない要素なら `None`
    pub(super) fn record<P: Copy>(
        &self,
        element: &Element<P>,
        frame: &impl Frame<P>,
        home: Home,
    ) -> Option<Record> {
        // 足した図形は、ハンドルで指されるので、ハンドルを付けられない図面には足さない
        self.next?;
        // BLOCKS の所に足す図形は、所有者のブロックの ENDBLK の前に置く。DWG は、その図形に 67 を持たない
        if home.blocks && !home.owner.is_some_and(|owner| self.ends.contains(&owner)) {
            return None;
        }
        let paper = home.paper && !(home.blocks && self.dwg());
        let mut values = self.entity(element.layer, &element.style, paper)?;
        let mut followers = Vec::new();
        let kind = match &element.shape {
            Shape::Line { start, end } | Shape::Curve(Edge::Line { start, end }) => {
                values.push(text(100, "AcDbLine"));
                point(&mut values, frame, 10, start)?;
                point(&mut values, frame, 11, end)?;
                b"LINE".as_slice()
            }
            Shape::Circle { center, radius } => {
                values.push(text(100, "AcDbCircle"));
                point(&mut values, frame, 10, center)?;
                values.push(real(40, positive(frame, radius)?)?);
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
                point(&mut values, frame, 10, center)?;
                values.push(real(40, positive(frame, radius)?)?);
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
                point(&mut values, frame, 10, center)?;
                values.extend([real(11, axis[0])?, real(21, axis[1])?, real(31, 0.0)?]);
                values.push(real(40, *ratio)?);
                values.push(real(41, start.to_radians())?);
                values.push(real(42, end.to_radians())?);
                b"ELLIPSE"
            }
            Shape::Polyline(line) => {
                values.push(text(100, "AcDbPolyline"));
                lwpolyline(&mut values, frame, line)?;
                b"LWPOLYLINE"
            }
            Shape::Text(lettering) => {
                let vertical = self.lettering(&mut values, frame, lettering, "dxf.TEXT.40")?;
                values.push(text(100, "AcDbText"));
                if vertical != 0 {
                    values.push(int(73, vertical));
                }
                b"TEXT"
            }
            Shape::Formatted(text) => {
                self.formatting(&mut values, frame, text)?;
                b"MTEXT"
            }
            Shape::Symbol(symbol) => {
                followers = self.placing(&mut values, frame, symbol, element.layer, home)?;
                b"INSERT"
            }
            _ => return None,
        };
        if !self.markers {
            values.retain(|value| value.code != 100);
        }
        if !self.markers {
            for (_, values) in &mut followers {
                values.retain(|value| value.code != 100);
            }
        }
        Some(Record {
            kind,
            values,
            home,
            followers,
        })
    }

    /// DWG に書き戻すか
    fn dwg(&self) -> bool {
        matches!(self.target, Target::Dwg(_))
    }

    /// 図形に共通の値（DXF Reference「Common Group Codes for Entities」）。区切り・ペーパー空間の印（`paper`
    /// なら 67 = 1）・レイヤ・線種・色・線幅・線種の倍率
    fn entity(&self, layer: LayerId, style: &Style, paper: bool) -> Option<Vec<Value>> {
        let mut values = vec![text(100, "AcDbEntity")];
        if paper {
            values.push(int(67, 1));
        }
        let layer = self.layers.get(&layer)?;
        values.push(self.named(8, b"LAYER", layer)?);
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
        Some(values)
    }

    /// TEXT と ATTRIB（DXF Reference「TEXT」「ATTRIB」）の、文字の区切り（AcDbText）の値。高さの意味が `meaning`
    /// のものに限る。位置合わせが左の基準線でなければ、11 を基点とし、10 も同じ点にする。10 は位置合わせから
    /// 決まる点で、読み直すときに使わず、AutoCAD は図面を開き直すと決め直す。垂直の位置合わせ（TEXT は 73、
    /// ATTRIB は 74）は、後の区切りに書くので、値を返す
    fn lettering<P>(
        &self,
        values: &mut Vec<Value>,
        frame: &impl Frame<P>,
        lettering: &Text<P>,
        meaning: &'static str,
    ) -> Option<i64> {
        if lettering.height.meaning != HeightMeaning::Declared(meaning) {
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
        Some(vertical)
    }

    /// MTEXT（DXF Reference「MTEXT」）の値。高さの意味が MTEXT の高さ（`dxf.MTEXT.40`）で、背景を持たず、
    /// 区間が内容を解釈し直した区間と合うものに限る。向きは、DXF では元の形（50 のラジアンか、11・21 の向き）で、
    /// DWG では 11・21 の向きで書く。DWG に足す MTEXT は角度（50）を持てないので、0 でない角度の MTEXT は DWG に
    /// 足さない。DWG から読む MTEXT は、どれも向きを持つ。
    /// 枠の高さ（46）は、DWG には R2010 からだけ書き、R2010 からは、なくても 0 を書く
    fn formatting<P>(
        &self,
        values: &mut Vec<Value>,
        frame: &impl Frame<P>,
        lettering: &FormattedText<P>,
    ) -> Option<()> {
        if lettering.height.meaning != HeightMeaning::Declared("dxf.MTEXT.40")
            || lettering.background.is_some()
            || lettering.spans != mtext::spans(&lettering.content, &|value| frame.lifted(value))
        {
            return None;
        }
        let dwg = self.dwg();
        let style = self.named(7, b"STYLE", &lettering.style)?;
        let row = match lettering.vertical {
            Vertical::Top => 0,
            Vertical::Middle => 1,
            Vertical::Bottom => 2,
            Vertical::Baseline => return None,
        };
        let column = match lettering.horizontal {
            Horizontal::Left => 1,
            Horizontal::Center => 2,
            Horizontal::Right => 3,
            Horizontal::Middle => return None,
        };
        let flow = match lettering.flow {
            Flow::LeftToRight => 1,
            Flow::TopToBottom => 3,
            Flow::ByStyle => 5,
        };
        let spacing = nonzero_positive(lettering.line_spacing.factor)?;
        values.push(text(100, "AcDbMText"));
        point(values, frame, 10, &lettering.point)?;
        values.push(real(40, positive(frame, &lettering.height.value)?)?);
        let width = match &lettering.width {
            Some(width) => positive(frame, width)?,
            None => 0.0,
        };
        values.push(real(41, width)?);
        // R2010 からの DWG は、枠の高さを必ず持ち、内容に合わせるなら 0 とする
        let since_r2010 = self.since(b"AC1024");
        match &lettering.frame_height {
            Some(_) if dwg && !since_r2010 => return None,
            Some(height) => values.push(real(46, positive(frame, height)?)?),
            None if dwg && since_r2010 => values.push(real(46, 0.0)?),
            None => {}
        }
        values.push(int(71, row * 3 + column));
        values.push(int(72, flow));
        values.push(text(1, ""));
        contents(values, &lettering.content, self.unicode)?;
        values.push(style);
        let direction = match lettering.rotation {
            Rotation::Direction(direction) => Some(direction),
            // 角度を向きにするには三角関数が要るので、DWG には 0 の角度だけを、X 軸の向きとして書く
            Rotation::Angle(angle) if dwg => (angle == 0.0).then_some([1.0, 0.0]),
            Rotation::Angle(angle) => {
                if angle != 0.0 {
                    values.push(real(50, angle.to_radians())?);
                }
                None
            }
        };
        if let Some([x, y]) = direction {
            if [x, y] == [0.0, 0.0] {
                return None;
            }
            values.extend([real(11, x)?, real(21, y)?, real(31, 0.0)?]);
        }
        values.push(int(73, if lettering.line_spacing.exact { 2 } else { 1 }));
        values.push(real(44, spacing)?);
        Some(())
    }

    /// INSERT（DXF Reference「INSERT」）の値と、続く ATTRIB と SEQEND。格子・切り抜きを持つ記号と、空間の
    /// ブロックは足さない。属性を持てば、属性が続く印（66 = 1）を書き、属性ごとの ATTRIB と、記号のレイヤの
    /// SEQEND を続ける
    fn placing<P>(
        &self,
        values: &mut Vec<Value>,
        frame: &impl Frame<P>,
        symbol: &Symbol<P>,
        layer: LayerId,
        home: Home,
    ) -> Option<Vec<(&'static [u8], Vec<Value>)>> {
        let name = &symbol.key.name;
        let space = name.to_lowercase();
        if symbol.key.namespace != BLOCK
            || symbol.grid.is_some()
            || symbol.clip.is_some()
            || space.starts_with("*model_space")
            || space.starts_with("*paper_space")
        {
            return None;
        }
        values.push(text(100, "AcDbBlockReference"));
        let attributed = !symbol.attributes.is_empty();
        if attributed {
            values.push(int(66, 1));
        }
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
        let mut followers = Vec::new();
        if attributed {
            // DWG の ATTRIB と SEQEND は、所有者を INSERT とし、ペーパー空間の印を持たない
            let paper = home.paper && !self.dwg();
            for attribute in &symbol.attributes {
                followers.push((
                    b"ATTRIB".as_slice(),
                    self.attribute(frame, attribute, paper)?,
                ));
            }
            let by_layer = Style {
                color: Inherit::Layer,
                line_type: Inherit::Layer,
                line_type_scale: 1.0,
                line_weight: None,
            };
            followers.push((b"SEQEND", self.entity(layer, &by_layer, paper)?));
        }
        Some(followers)
    }

    /// ATTRIB（DXF Reference「ATTRIB」）の値。高さの意味が属性の高さ（`dxf.ATTRIB.40`）で、複数行の値を持たない
    /// ものに限る。R2010 からの DWG には、属性の版（280）をタグの前に、位置を固定するか（280）を終わりに、どちらも
    /// 0 として書く（決定 0093）
    fn attribute<P>(
        &self,
        frame: &impl Frame<P>,
        attribute: &Attribute<P>,
        paper: bool,
    ) -> Option<Vec<Value>> {
        if attribute.formatted.is_some() {
            return None;
        }
        let mut values = self.entity(attribute.layer, &attribute.style, paper)?;
        let vertical = self.lettering(&mut values, frame, &attribute.text, "dxf.ATTRIB.40")?;
        values.push(text(100, "AcDbAttribute"));
        let versions = self.dwg() && self.since(b"AC1024");
        if versions {
            values.push(int(280, 0));
        }
        values.push(self.string(2, &attribute.tag)?);
        values.push(int(70, i64::from(attribute.hidden)));
        if vertical != 0 {
            values.push(int(74, vertical));
        }
        if versions {
            values.push(int(280, 0));
        }
        Some(values)
    }

    /// 足す図形を表に足す。ハンドルを順に付け、所有者を置き場所のブロックのレコードにする。DXF の BLOCKS の
    /// 所の図形は、所有者のブロックの ENDBLK の前に置く。DWG は、足したオブジェクトを表の終わりにだけ置け、
    /// 所有者のブロックの図形の列には書き戻しがつなぐ（決定 0079）
    pub(super) fn push(&self, db: &mut AcadDb, added: Vec<Record>) {
        let Some(mut next) = self.next else {
            return;
        };
        for Record {
            kind,
            values,
            home,
            followers,
        } in added
        {
            // ATTRIB と SEQEND は、図形のすぐ後に置き、所有者を図形とする
            let parent = next;
            let objects = std::iter::once((kind, home.owner, values)).chain(
                followers
                    .into_iter()
                    .map(|(kind, values)| (kind, Some(parent), values)),
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
                let object = Object {
                    kind: kind.to_vec(),
                    section: if home.blocks {
                        Section::Blocks
                    } else {
                        Section::Entities
                    },
                    values,
                    origin: Origin::New,
                };
                let end = db.objects().iter().position(|object| {
                    object.kind == b"ENDBLK"
                        && home.blocks
                        && self.target == Target::Dxf
                        && object.owner() == home.owner
                });
                match end {
                    Some(index) => db.insert(index, object),
                    None => db.push(object),
                }
                next = next.saturating_add(1);
            }
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
fn lwpolyline<P>(values: &mut Vec<Value>, frame: &impl Frame<P>, line: &Polyline<P>) -> Option<()> {
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
fn point<P>(values: &mut Vec<Value>, frame: &impl Frame<P>, code: i16, at: &P) -> Option<()> {
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
