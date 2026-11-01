//! IFC4.3 の線形を土木の領域モデルに移す（決定 0230・0235、#375）。
//!
//! `IfcAlignment` の入れ子（`IfcRelNests`）の平面・縦断・カントの区間を、書いた順に読む。属性の位置は、
//! IFC 4.3 ADD2 の実体の定義（「属性」の節）による。点は、線形の配置（`IfcLocalPlacement`）と
//! `IfcMapConversion` で地図の空間に直す。地表面（`IfcTriangulatedIrregularNetwork`）は、形を持つ要素の配置で
//! 直して TIN にする。横断（断面の形）は、まだ移さず、種類ごとに診断する。

use zumen_base::{Diagnostic, Kind, codes};
use zumen_civil::{
    Alignment, Cant, CantKind, CivilModel, Horizontal, HorizontalKind, StationBreak, Stationing,
    SurfaceRole, Tin, TransitionCurve, Vertical, VerticalKind,
};
use zumen_crs::{Crs, HeightDatum, MapPoint, Realization, VerticalCrs};
use zumen_sdk::{Cx, Error, Lift};

use crate::building::{arg, decode, reference, refs};
use crate::{IfcFile, IfcZipFile, Inverse, Record, Value, at_bytes, stopped};

/// 配置の連鎖をたどる深さの上限
const DEPTH: usize = 64;

/// 移さない形の種類
const DROPPED: &[&[u8]] = &[
    b"IFCSECTIONEDSOLIDHORIZONTAL",
    b"IFCSECTIONEDSURFACE",
    b"IFCOPENCROSSPROFILEDEF",
];

impl Lift<CivilModel> for IfcFile {
    /// IFC4X3 の線形から作る。ほかの版は、診断して作らない
    fn lift(&self, cx: &mut Cx) -> Result<CivilModel, Error> {
        if !self.schema().to_ascii_uppercase().starts_with(b"IFC4X3") {
            cx.report(Diagnostic::new(codes::IFC_VERSION_UNSUPPORTED).with(
                "schema",
                String::from_utf8_lossy(self.schema()).into_owned(),
            ));
            return Err(Error::Unsupported);
        }
        let inverse = self.inverse(cx)?;
        let mut lifter = Lifter {
            file: self,
            inverse,
            length: 1.0,
            angle: 1.0,
            map: Map::default(),
        };
        lifter.run(cx)
    }
}

impl Lift<CivilModel> for IfcZipFile {
    /// 書庫の中の IFC-SPF から作る
    fn lift(&self, cx: &mut Cx) -> Result<CivilModel, Error> {
        self.ifc().lift(cx)
    }
}

/// プロジェクトの空間から地図の空間への変換（`IfcMapConversion`）
#[derive(Clone, Copy, Debug)]
struct Map {
    eastings: f64,
    northings: f64,
    height: f64,
    /// プロジェクトの x 軸の、地図の東からの向き
    cos: f64,
    sin: f64,
    scale: f64,
    scale_z: f64,
}

impl Default for Map {
    fn default() -> Self {
        Self {
            eastings: 0.0,
            northings: 0.0,
            height: 0.0,
            cos: 1.0,
            sin: 0.0,
            scale: 1.0,
            scale_z: 1.0,
        }
    }
}

impl Map {
    /// プロジェクトの空間の点（メートル）
    fn point(&self, [x, y]: [f64; 2]) -> MapPoint {
        MapPoint {
            northing: self.northings + self.scale * (x * self.sin + y * self.cos),
            easting: self.eastings + self.scale * (x * self.cos - y * self.sin),
        }
    }

    /// プロジェクトの空間の高さ（メートル）
    fn height(&self, z: f64) -> f64 {
        self.height + self.scale_z * z
    }

    /// プロジェクトの空間の x 軸から反時計回りの角を、北から時計回りの方向角にする
    fn direction(&self, angle: f64) -> f64 {
        let east = angle + libm::atan2(self.sin, self.cos);
        let north = core::f64::consts::FRAC_PI_2 - east;
        libm::atan2(libm::sin(north), libm::cos(north))
    }
}

/// 線形の配置。プロジェクトの空間での原点（メートル）と、x 軸の向き
#[derive(Clone, Copy, Debug)]
struct Place {
    origin: [f64; 3],
    x: [f64; 2],
}

impl Place {
    const IDENTITY: Place = Place {
        origin: [0.0, 0.0, 0.0],
        x: [1.0, 0.0],
    };

    fn apply(&self, [px, py]: [f64; 2]) -> [f64; 2] {
        let [cx, cy] = self.x;
        let [ox, oy, _] = self.origin;
        [ox + px * cx - py * cy, oy + px * cy + py * cx]
    }

    fn then(&self, child: &Place) -> Place {
        let [ox, oy] = self.apply([child.origin[0], child.origin[1]]);
        let [cx, cy] = self.x;
        let [kx, ky] = child.x;
        Place {
            origin: [ox, oy, self.origin[2] + child.origin[2]],
            x: [kx * cx - ky * cy, kx * cy + ky * cx],
        }
    }

    fn angle(&self) -> f64 {
        libm::atan2(self.x[1], self.x[0])
    }
}

struct Lifter<'f> {
    file: &'f IfcFile,
    inverse: Inverse,
    /// 長さの単位の倍率（メートル）
    length: f64,
    /// 平面角の単位の倍率（ラジアン）
    angle: f64,
    map: Map,
}

impl<'f> Lifter<'f> {
    fn run(&mut self, cx: &mut Cx) -> Result<CivilModel, Error> {
        let mut model = CivilModel::default();
        self.units(cx);
        self.coordinates(&mut model, cx);
        let ids: Vec<u64> = self.file.ids().collect();
        for id in ids {
            let Some(record) = self.file.instance(id) else {
                continue;
            };
            let kind = self.file.kind(record).to_ascii_uppercase();
            if kind == b"IFCALIGNMENT" {
                cx.check(Kind::Elements, 1)
                    .map_err(|stop| stopped(cx, stop, Some(record.span())))?;
                let alignment = self.alignment(id, record, cx)?;
                model.alignments.push(alignment);
            } else if kind == b"IFCTRIANGULATEDIRREGULARNETWORK" {
                if let Some(mut tin) = self.tin(id, record, cx)? {
                    if model.surfaces.iter().any(|other| other.id == tin.id) {
                        tin.id = format!("{}-i{id}", tin.id);
                    }
                    model.surfaces.push(tin);
                }
            } else if DROPPED.contains(&kind.as_slice()) {
                cx.report(
                    Diagnostic::new(codes::IFC_CIVIL_GEOMETRY_DROPPED)
                        .with(
                            "kind",
                            String::from_utf8_lossy(self.file.kind(record)).into_owned(),
                        )
                        .at(at_bytes(record.span())),
                );
            }
        }
        Ok(model)
    }

    fn is(&self, id: u64, kind: &[u8]) -> bool {
        self.file
            .instance(id)
            .is_some_and(|record| self.file.kind(record).eq_ignore_ascii_case(kind))
    }

    fn find(&self, kind: &[u8]) -> Option<u64> {
        self.file.ids().find(|&id| self.is(id, kind))
    }

    fn instance_of(&self, value: Option<&Value>, kind: &[u8]) -> Option<&'f Record> {
        let id = reference(value).filter(|&id| self.is(id, kind))?;
        self.file.instance(id)
    }

    // 値

    fn text(&self, value: Option<&Value>, place: &Record, cx: &mut Cx) -> Option<String> {
        let Some(Value::Text { raw, .. }) = value else {
            return None;
        };
        let (text, replaced) = decode(self.file.bytes(raw.clone()));
        if replaced {
            cx.report(Diagnostic::new(codes::IFC_TEXT_INVALID).at(at_bytes(place.span())));
        }
        Some(text)
    }

    fn enumeration(&self, value: Option<&Value>) -> Option<String> {
        match value {
            Some(Value::Enum(range)) => {
                Some(String::from_utf8_lossy(self.file.bytes(range.clone())).to_ascii_uppercase())
            }
            _ => None,
        }
    }

    /// 数。型を付けた値（`IFCLENGTHMEASURE(1.)`）なら、その中の数
    fn real(&self, value: Option<&Value>) -> Option<f64> {
        match value? {
            Value::Number(range) => core::str::from_utf8(self.file.bytes(range.clone()))
                .ok()?
                .parse::<f64>()
                .ok()
                .filter(|v| v.is_finite()),
            Value::Typed(part) => match part.args() {
                [only] => self.real(Some(only)),
                _ => None,
            },
            _ => None,
        }
    }

    fn reals(&self, value: Option<&Value>) -> Option<Vec<f64>> {
        match value? {
            Value::List(values) => values.iter().map(|v| self.real(Some(v))).collect(),
            _ => None,
        }
    }

    fn invalid(&self, record: &Record, position: u64, cx: &mut Cx) {
        cx.report(
            Diagnostic::new(codes::IFC_ATTRIBUTE_INVALID)
                .with(
                    "kind",
                    String::from_utf8_lossy(self.file.kind(record)).into_owned(),
                )
                .with("position", position)
                .at(at_bytes(record.span())),
        );
    }

    /// `IfcCartesianPoint` の 1 Coordinates（メートル）
    fn point(&self, value: Option<&Value>) -> Option<Vec<f64>> {
        let record = self.instance_of(value, b"IFCCARTESIANPOINT")?;
        let values = self.reals(arg(record, 1))?;
        Some(values.iter().map(|v| v * self.length).collect())
    }

    /// `IfcDirection` の 1 DirectionRatios
    fn ratios(&self, value: Option<&Value>) -> Option<Vec<f64>> {
        let record = self.instance_of(value, b"IFCDIRECTION")?;
        self.reals(arg(record, 1))
    }

    /// 入れ子の子。`IfcRelNests` の 5 RelatingObject・6 RelatedObjects を、書いた順に
    fn nested(&self, id: u64) -> Vec<u64> {
        let mut children = Vec::new();
        for &rel in self.inverse.referrers(id) {
            let Some(record) = self.file.instance(rel) else {
                continue;
            };
            if self.file.kind(record).eq_ignore_ascii_case(b"IFCRELNESTS")
                && reference(arg(record, 5)) == Some(id)
            {
                children.extend(refs(arg(record, 6)));
            }
        }
        children
    }

    // 単位と座標系

    /// プロジェクトの長さと平面角の単位。`IfcProject` の 9 UnitsInContext → `IfcUnitAssignment` の 1 Units
    fn units(&mut self, cx: &mut Cx) {
        let units = self
            .find(b"IFCPROJECT")
            .and_then(|id| self.file.instance(id))
            .and_then(|project| self.instance_of(arg(project, 9), b"IFCUNITASSIGNMENT"))
            .map(|assignment| refs(arg(assignment, 1)))
            .unwrap_or_default();
        let mut length = None;
        let mut angle = None;
        for id in units {
            let Some(record) = self.file.instance(id) else {
                continue;
            };
            match self.unit(record, 0).as_ref() {
                Some((kind, factor)) if kind == "LENGTHUNIT" => length = Some(*factor),
                Some((kind, factor)) if kind == "PLANEANGLEUNIT" => angle = Some(*factor),
                _ => {}
            }
        }
        for (factor, kind, field) in [
            (length, "IfcLengthMeasure", &mut self.length),
            (angle, "IfcPlaneAngleMeasure", &mut self.angle),
        ] {
            match factor {
                Some(factor) => *field = factor,
                None => {
                    cx.report(Diagnostic::new(codes::IFC_CIVIL_UNIT_ASSUMED).with("type", kind))
                }
            }
        }
    }

    /// 単位の種類と、SI の単位（メートル・ラジアン）への倍率。`IfcSIUnit` は 2 UnitType・3 Prefix・4 Name、
    /// `IfcConversionBasedUnit` は 2 UnitType・4 ConversionFactor（`IfcMeasureWithUnit` の 1 ValueComponent・
    /// 2 UnitComponent）
    fn unit(&self, record: &Record, depth: usize) -> Option<(String, f64)> {
        let kind = self.enumeration(arg(record, 2))?;
        let name = self.file.kind(record);
        if name.eq_ignore_ascii_case(b"IFCSIUNIT") {
            let base = match self.enumeration(arg(record, 4))?.as_str() {
                "METRE" | "RADIAN" => 1.0,
                _ => return None,
            };
            let prefix = match self.enumeration(arg(record, 3)).as_deref() {
                None => 1.0,
                Some(prefix) => PREFIXES
                    .iter()
                    .find(|(name, _)| *name == prefix)
                    .map(|(_, factor)| *factor)?,
            };
            return Some((kind, base * prefix));
        }
        if name.eq_ignore_ascii_case(b"IFCCONVERSIONBASEDUNIT") && depth < 4 {
            let measure = self.instance_of(arg(record, 4), b"IFCMEASUREWITHUNIT")?;
            let value = self.real(arg(measure, 1))?;
            let component = self.file.instance(reference(arg(measure, 2))?)?;
            let (_, factor) = self.unit(component, depth + 1)?;
            return Some((kind, value * factor));
        }
        None
    }

    /// 初めの `IfcMapConversion`。3 Eastings・4 Northings・5 OrthogonalHeight・6 XAxisAbscissa・
    /// 7 XAxisOrdinate・8 Scale・10 ScaleZ と、2 TargetCRS → `IfcProjectedCRS` の 1 Name・3 GeodeticDatum・
    /// 4 VerticalDatum
    fn coordinates(&mut self, model: &mut CivilModel, cx: &mut Cx) {
        let Some(record) = self
            .find(b"IFCMAPCONVERSION")
            .and_then(|id| self.file.instance(id))
        else {
            cx.report(Diagnostic::new(codes::IFC_MAP_CRS_MISSING));
            return;
        };
        let (Some(eastings), Some(northings), Some(height)) = (
            self.real(arg(record, 3)),
            self.real(arg(record, 4)),
            self.real(arg(record, 5)),
        ) else {
            self.invalid(record, 3, cx);
            return;
        };
        let abscissa = self.real(arg(record, 6)).unwrap_or(1.0);
        let ordinate = self.real(arg(record, 7)).unwrap_or(0.0);
        let norm = libm::hypot(abscissa, ordinate);
        if norm > 0.0 {
            self.map.cos = abscissa / norm;
            self.map.sin = ordinate / norm;
        } else {
            self.invalid(record, 6, cx);
        }
        // 地図の座標は、地図の単位（メートルとする）で書く
        self.map.eastings = eastings;
        self.map.northings = northings;
        self.map.height = height;
        self.map.scale = self.real(arg(record, 8)).unwrap_or(1.0);
        self.map.scale_z = self.real(arg(record, 10)).unwrap_or(1.0);
        let Some(crs) = self.instance_of(arg(record, 2), b"IFCPROJECTEDCRS") else {
            cx.report(Diagnostic::new(codes::IFC_MAP_CRS_MISSING).at(at_bytes(record.span())));
            return;
        };
        let name = self.text(arg(crs, 1), crs, cx).unwrap_or_default();
        let datum = self.text(arg(crs, 3), crs, cx).unwrap_or_default();
        model.crs = Crs::from_name(&name);
        if model.crs.is_none() {
            cx.report(
                Diagnostic::new(codes::IFC_MAP_CRS_UNSUPPORTED)
                    .with("name", name.as_str())
                    .at(at_bytes(crs.span())),
            );
        }
        let vertical = self.text(arg(crs, 4), crs, cx).unwrap_or_default();
        let named = |text: &str| {
            if text.contains("JGD2024") {
                Some(Realization::Survey2024)
            } else if text.contains("JGD2011") {
                Some(Realization::Survey2011)
            } else {
                None
            }
        };
        model.realization = named(&name).or_else(|| named(&datum));
        match (VerticalCrs::from_name(&vertical), vertical.trim()) {
            (Some(height), _) => {
                model.height_datum = Some(HeightDatum::TokyoBay);
                model.realization = Some(height.realization());
            }
            (None, "T.P" | "T.P.") => model.height_datum = Some(HeightDatum::TokyoBay),
            _ => cx.report(
                Diagnostic::new(codes::IFC_HEIGHT_DATUM_UNSUPPORTED)
                    .with("datum", vertical.as_str())
                    .at(at_bytes(crs.span())),
            ),
        }
    }

    /// 配置の連鎖をたどる。`IfcLocalPlacement` の 1 PlacementRelTo・2 RelativePlacement を読む。鉛直でない軸と、
    /// ほかの配置は解けないので、診断して原点に置く
    fn placement(&self, start: Option<u64>, cx: &mut Cx) -> Place {
        let mut chain = Vec::new();
        let mut at = start;
        while let Some(id) = at {
            let Some(record) = self.file.instance(id) else {
                break;
            };
            if !self
                .file
                .kind(record)
                .eq_ignore_ascii_case(b"IFCLOCALPLACEMENT")
            {
                cx.report(
                    Diagnostic::new(codes::IFC_PLACEMENT_UNSUPPORTED)
                        .with(
                            "kind",
                            String::from_utf8_lossy(self.file.kind(record)).into_owned(),
                        )
                        .at(at_bytes(record.span())),
                );
                return Place::IDENTITY;
            }
            if chain.len() >= DEPTH {
                cx.report(
                    Diagnostic::new(codes::IFC_PLACEMENT_INVALID)
                        .with("id", format!("i{id}"))
                        .at(at_bytes(record.span())),
                );
                return Place::IDENTITY;
            }
            match self.axes(arg(record, 2)) {
                Some(place) => chain.push(place),
                None => {
                    cx.report(
                        Diagnostic::new(codes::IFC_PLACEMENT_UNSUPPORTED)
                            .with("kind", "IfcAxis2Placement3D")
                            .at(at_bytes(record.span())),
                    );
                    return Place::IDENTITY;
                }
            }
            at = reference(arg(record, 1));
        }
        chain
            .iter()
            .rev()
            .fold(Place::IDENTITY, |parent, child| parent.then(child))
    }

    /// `IfcAxis2Placement3D`（1 Location・2 Axis・3 RefDirection）か `IfcAxis2Placement2D`（1 Location・
    /// 2 RefDirection）。軸が鉛直でなければ `None`
    fn axes(&self, value: Option<&Value>) -> Option<Place> {
        let id = reference(value)?;
        let record = self.file.instance(id)?;
        let kind = self.file.kind(record);
        let (axis, direction) = if kind.eq_ignore_ascii_case(b"IFCAXIS2PLACEMENT3D") {
            (arg(record, 2), arg(record, 3))
        } else if kind.eq_ignore_ascii_case(b"IFCAXIS2PLACEMENT2D") {
            (None, arg(record, 2))
        } else {
            return None;
        };
        let location = self.point(arg(record, 1))?;
        if let Some(Value::Ref(_)) = axis {
            let ratios = self.ratios(axis)?;
            let [x, y, z] = ratios.as_slice() else {
                return None;
            };
            if !(*z > 0.0 && libm::hypot(*x, *y) <= 1e-9 * z) {
                return None;
            }
        }
        let x = match direction {
            Some(Value::Ref(_)) => {
                let ratios = self.ratios(direction)?;
                let (x, y) = (*ratios.first()?, *ratios.get(1)?);
                let norm = libm::hypot(x, y);
                if norm <= 0.0 {
                    return None;
                }
                [x / norm, y / norm]
            }
            _ => [1.0, 0.0],
        };
        let at = |i: usize| location.get(i).copied().unwrap_or(0.0);
        Some(Place {
            origin: [at(0), at(1), at(2)],
            x,
        })
    }

    // 線形

    /// `IfcAlignment`。1 GlobalId・3 Name・5 ObjectType・6 ObjectPlacement・8 PredefinedType
    fn alignment(&mut self, id: u64, record: &'f Record, cx: &mut Cx) -> Result<Alignment, Error> {
        let global = self.text(arg(record, 1), record, cx);
        let alignment_id = match global {
            Some(global) => global,
            None => {
                cx.report(
                    Diagnostic::new(codes::IFC_GLOBAL_ID_MISSING)
                        .at(at_bytes(record.span()).with_id(format!("i{id}"))),
                );
                format!("i{id}")
            }
        };
        let usage = match self.enumeration(arg(record, 8)).as_deref() {
            None | Some("NOTDEFINED") => None,
            Some("USERDEFINED") => self.text(arg(record, 5), record, cx),
            Some(other) => Some(other.to_string()),
        };
        let place = self.placement(reference(arg(record, 6)), cx);
        let mut alignment = Alignment {
            id: alignment_id,
            name: self.text(arg(record, 3), record, cx),
            usage,
            source: at_bytes(record.span()),
            ..Alignment::default()
        };
        let mut referents = Vec::new();
        for child in self.nested(id) {
            let Some(layout) = self.file.instance(child) else {
                continue;
            };
            let kind = self.file.kind(layout).to_ascii_uppercase();
            match kind.as_slice() {
                b"IFCALIGNMENTHORIZONTAL" => {
                    for (segment, design) in self.segments(child, cx)? {
                        if let Some(horizontal) = self.horizontal(segment, design, &place, cx) {
                            alignment.horizontal.push(horizontal);
                        }
                    }
                }
                b"IFCALIGNMENTVERTICAL" => {
                    for (segment, design) in self.segments(child, cx)? {
                        if let Some(vertical) = self.vertical(segment, design, &place, cx) {
                            alignment.vertical.push(vertical);
                        }
                    }
                }
                b"IFCALIGNMENTCANT" => {
                    // 8 RailHeadDistance
                    let gauge = self.real(arg(layout, 8)).map(|v| v * self.length * 1000.0);
                    for (segment, design) in self.segments(child, cx)? {
                        if let Some(cant) = self.cant(segment, design, gauge, cx) {
                            alignment.cant.push(cant);
                        }
                    }
                }
                b"IFCREFERENT" => referents.push(child),
                _ => {}
            }
        }
        alignment.stations = self.stations(&referents, cx);
        Ok(alignment)
    }

    // 地表面

    /// 形の表現の項目を持つ要素。`IfcShapeRepresentation`（4 Items）→ `IfcProductDefinitionShape`
    /// （3 Representations）→ 要素（7 Representation）とたどる
    fn owner(&self, item: u64) -> Option<&'f Record> {
        for &representation in self.inverse.referrers(item) {
            if !self.is(representation, b"IFCSHAPEREPRESENTATION") {
                continue;
            }
            for &shape in self.inverse.referrers(representation) {
                if !self.is(shape, b"IFCPRODUCTDEFINITIONSHAPE") {
                    continue;
                }
                for &product in self.inverse.referrers(shape) {
                    let Some(record) = self.file.instance(product) else {
                        continue;
                    };
                    if reference(arg(record, 7)) == Some(shape) {
                        return Some(record);
                    }
                }
            }
        }
        None
    }

    /// `IfcTriangulatedIrregularNetwork`。1 Coordinates（`IfcCartesianPointList3D` の 1 CoordList）・
    /// 4 CoordIndex・5 PnIndex・6 Flags を読む。点は、形を持つ要素の 6 ObjectPlacement で直す。ID・名前・役割は、
    /// その要素の 1 GlobalId・3 Name と、`IfcGeographicElement` の 9 PredefinedType から取る
    fn tin(&self, id: u64, record: &'f Record, cx: &mut Cx) -> Result<Option<Tin>, Error> {
        let owner = self.owner(id);
        let place = match owner {
            Some(owner) => self.placement(reference(arg(owner, 6)), cx),
            None => {
                cx.report(
                    Diagnostic::new(codes::IFC_PLACEMENT_UNSUPPORTED)
                        .with("kind", "IfcTriangulatedIrregularNetwork")
                        .at(at_bytes(record.span())),
                );
                Place::IDENTITY
            }
        };
        let Some(list) = self.instance_of(arg(record, 1), b"IFCCARTESIANPOINTLIST3D") else {
            self.invalid(record, 1, cx);
            return Ok(None);
        };
        let Some(Value::List(rows)) = arg(list, 1) else {
            self.invalid(list, 1, cx);
            return Ok(None);
        };
        let mut coordinates = Vec::with_capacity(rows.len());
        for row in rows {
            cx.check(Kind::Work, 1)
                .map_err(|stop| stopped(cx, stop, Some(list.span())))?;
            match self.reals(Some(row)).as_deref() {
                Some([x, y, z]) => {
                    let [px, py] = place.apply([x * self.length, y * self.length]);
                    let height = self.map.height(place.origin[2] + z * self.length);
                    coordinates.push((self.map.point([px, py]), height));
                }
                _ => {
                    self.invalid(list, 1, cx);
                    return Ok(None);
                }
            }
        }
        // 番号は 1 から数える。PnIndex があれば、CoordIndex は PnIndex の番号で、PnIndex が点の番号を持つ
        let number = |value: f64, len: usize| {
            let index = value as usize;
            (value.fract() == 0.0 && value >= 1.0 && index <= len)
                .then(|| u32::try_from(index - 1).ok())
                .flatten()
        };
        let points = match arg(record, 5) {
            Some(Value::List(_)) => {
                let Some(order) = self.reals(arg(record, 5)).and_then(|values| {
                    values
                        .iter()
                        .map(|v| number(*v, coordinates.len()))
                        .collect::<Option<Vec<u32>>>()
                }) else {
                    self.invalid(record, 5, cx);
                    return Ok(None);
                };
                order
                    .iter()
                    .filter_map(|&i| coordinates.get(i as usize).copied())
                    .collect()
            }
            _ => coordinates,
        };
        let Some(Value::List(faces)) = arg(record, 4) else {
            self.invalid(record, 4, cx);
            return Ok(None);
        };
        let mut triangles = Vec::with_capacity(faces.len());
        for face in faces {
            cx.check(Kind::Work, 1)
                .map_err(|stop| stopped(cx, stop, Some(record.span())))?;
            let corners = self.reals(Some(face)).and_then(|values| {
                values
                    .iter()
                    .map(|v| number(*v, points.len()))
                    .collect::<Option<Vec<u32>>>()
            });
            match corners.as_deref() {
                Some([a, b, c]) => triangles.push([*a, *b, *c]),
                _ => {
                    self.invalid(record, 4, cx);
                    return Ok(None);
                }
            }
        }
        // 三角形ごとの印は、意味を確かめるまで読まない。0 でない印があれば知らせる
        if self
            .reals(arg(record, 6))
            .is_some_and(|flags| flags.iter().any(|flag| *flag != 0.0))
        {
            cx.report(
                Diagnostic::new(codes::IFC_CIVIL_GEOMETRY_DROPPED)
                    .with("kind", "IfcTriangulatedIrregularNetwork.Flags")
                    .at(at_bytes(record.span())),
            );
        }
        let global = owner.and_then(|owner| self.text(arg(owner, 1), owner, cx));
        let role = match owner {
            Some(owner)
                if self
                    .file
                    .kind(owner)
                    .eq_ignore_ascii_case(b"IFCGEOGRAPHICELEMENT") =>
            {
                match self.enumeration(arg(owner, 9)).as_deref() {
                    Some("TERRAIN") => SurfaceRole::Existing,
                    Some(other) => SurfaceRole::Other(other.to_string()),
                    None => SurfaceRole::Other(String::new()),
                }
            }
            _ => SurfaceRole::Other(String::new()),
        };
        Ok(Some(Tin {
            id: global.unwrap_or_else(|| format!("i{id}")),
            name: owner.and_then(|owner| self.text(arg(owner, 3), owner, cx)),
            role,
            points,
            triangles,
            boundaries: Vec::new(),
            breaklines: Vec::new(),
            source: at_bytes(record.span()),
        }))
    }

    /// 配置の区間（`IfcAlignmentSegment` の 8 DesignParameters）を、書いた順に
    fn segments(&self, layout: u64, cx: &mut Cx) -> Result<Vec<(&'f Record, &'f Record)>, Error> {
        let mut segments = Vec::new();
        for child in self.nested(layout) {
            let Some(segment) = self.file.instance(child) else {
                continue;
            };
            if !self
                .file
                .kind(segment)
                .eq_ignore_ascii_case(b"IFCALIGNMENTSEGMENT")
            {
                continue;
            }
            cx.check(Kind::Records, 1)
                .map_err(|stop| stopped(cx, stop, Some(segment.span())))?;
            match reference(arg(segment, 8)).and_then(|id| self.file.instance(id)) {
                Some(design) => segments.push((segment, design)),
                None => self.invalid(segment, 8, cx),
            }
        }
        Ok(segments)
    }

    fn unsupported(&self, design: &Record, kind: &str, cx: &mut Cx) {
        cx.report(
            Diagnostic::new(codes::IFC_ALIGNMENT_SEGMENT_UNSUPPORTED)
                .with(
                    "kind",
                    String::from_utf8_lossy(self.file.kind(design)).into_owned(),
                )
                .with("type", kind)
                .at(at_bytes(design.span())),
        );
    }

    /// `IfcAlignmentHorizontalSegment`。3 StartPoint・4 StartDirection・5 StartRadiusOfCurvature・
    /// 6 EndRadiusOfCurvature・7 SegmentLength・9 PredefinedType。半径は左へ曲がるものを正、0 を直線とする。
    /// 長さ 0 の区間（線形の終わりの印）は移さない
    fn horizontal(
        &self,
        segment: &Record,
        design: &Record,
        place: &Place,
        cx: &mut Cx,
    ) -> Option<Horizontal> {
        let point = self.point(arg(design, 3));
        let direction = self.real(arg(design, 4));
        let length = self.real(arg(design, 7)).filter(|v| *v >= 0.0);
        let kind = self.enumeration(arg(design, 9));
        let (Some(point), Some(direction), Some(length), Some(kind)) =
            (point, direction, length, kind)
        else {
            self.invalid(design, 3, cx);
            return None;
        };
        if length == 0.0 {
            return None;
        }
        let scale = self.map.scale * self.length;
        let radius = |at: usize| {
            self.real(arg(design, at))
                .filter(|v| *v != 0.0)
                .map(|v| -v * scale)
        };
        let curve = match kind.as_str() {
            "LINE" => None,
            "CIRCULARARC" => {
                let Some(radius) = radius(5) else {
                    self.invalid(design, 5, cx);
                    return None;
                };
                return Some(Horizontal {
                    start: self.map.point(place.apply(xy(&point))),
                    direction: self.map.direction(place.angle() + direction * self.angle),
                    length: length * scale,
                    kind: HorizontalKind::Arc { radius },
                    source: at_bytes(segment.span()),
                });
            }
            "CLOTHOID" => Some(TransitionCurve::Clothoid),
            "CUBIC" => Some(TransitionCurve::CubicParabola),
            "HELMERTCURVE" => Some(TransitionCurve::Helmert),
            "BLOSSCURVE" => Some(TransitionCurve::Bloss),
            "COSINECURVE" => Some(TransitionCurve::Cosine),
            "SINECURVE" => Some(TransitionCurve::SineHalfWave),
            "VIENNESEBEND" => Some(TransitionCurve::Viennese),
            other => {
                self.unsupported(design, other, cx);
                return None;
            }
        };
        Some(Horizontal {
            start: self.map.point(place.apply(xy(&point))),
            direction: self.map.direction(place.angle() + direction * self.angle),
            length: length * scale,
            kind: match curve {
                None => HorizontalKind::Line,
                Some(curve) => HorizontalKind::Transition {
                    start_radius: radius(5),
                    end_radius: radius(6),
                    curve,
                },
            },
            source: at_bytes(segment.span()),
        })
    }

    /// `IfcAlignmentVerticalSegment`。3 StartDistAlong・4 HorizontalLength・5 StartHeight・6 StartGradient・
    /// 7 EndGradient・8 RadiusOfCurvature・9 PredefinedType。円弧の半径の符号は、勾配の変わり方から決める
    fn vertical(
        &self,
        segment: &Record,
        design: &Record,
        place: &Place,
        cx: &mut Cx,
    ) -> Option<Vertical> {
        let start = self.real(arg(design, 3));
        let length = self.real(arg(design, 4)).filter(|v| *v >= 0.0);
        let height = self.real(arg(design, 5));
        let gradient = self.real(arg(design, 6));
        let kind = self.enumeration(arg(design, 9));
        let (Some(start), Some(length), Some(height), Some(gradient), Some(kind)) =
            (start, length, height, gradient, kind)
        else {
            self.invalid(design, 3, cx);
            return None;
        };
        if length == 0.0 {
            return None;
        }
        let scale = self.map.scale * self.length;
        let slope = self.map.scale_z / self.map.scale;
        let end_gradient = self.real(arg(design, 7)).unwrap_or(gradient);
        let kind = match kind.as_str() {
            "CONSTANTGRADIENT" => VerticalKind::ConstantGradient,
            "PARABOLICARC" => VerticalKind::Parabola {
                end_gradient: end_gradient * slope,
            },
            "CIRCULARARC" => {
                let Some(radius) = self.real(arg(design, 8)).filter(|v| *v != 0.0) else {
                    self.invalid(design, 8, cx);
                    return None;
                };
                let crest = match end_gradient.partial_cmp(&gradient) {
                    Some(core::cmp::Ordering::Less) => true,
                    Some(core::cmp::Ordering::Greater) => false,
                    _ => radius > 0.0,
                };
                let radius = radius.abs() * scale;
                VerticalKind::CircularArc {
                    radius: if crest { radius } else { -radius },
                }
            }
            other => {
                self.unsupported(design, other, cx);
                return None;
            }
        };
        Some(Vertical {
            start_distance: start * scale,
            start_height: self.map.height(place.origin[2] + height * self.length),
            start_gradient: gradient * slope,
            length: length * scale,
            kind,
            source: at_bytes(segment.span()),
        })
    }

    /// `IfcAlignmentCantSegment`。3 StartDistAlong・4 HorizontalLength・5 StartCantLeft・6 EndCantLeft・
    /// 7 StartCantRight・8 EndCantRight・9 PredefinedType。カントは mm にする
    fn cant(
        &self,
        segment: &Record,
        design: &Record,
        gauge: Option<f64>,
        cx: &mut Cx,
    ) -> Option<Cant> {
        let start = self.real(arg(design, 3));
        let length = self.real(arg(design, 4)).filter(|v| *v >= 0.0);
        let left = self.real(arg(design, 5));
        let right = self.real(arg(design, 7));
        let kind = self.enumeration(arg(design, 9));
        let (Some(start), Some(length), Some(left), Some(right), Some(kind)) =
            (start, length, left, right, kind)
        else {
            self.invalid(design, 3, cx);
            return None;
        };
        if length == 0.0 {
            return None;
        }
        let scale = self.map.scale * self.length;
        let mm = self.length * 1000.0;
        let end_left = self.real(arg(design, 6)).unwrap_or(left);
        let end_right = self.real(arg(design, 8)).unwrap_or(right);
        Some(Cant {
            start_distance: start * scale,
            length: length * scale,
            kind: CantKind::Rail {
                start: [left * mm, right * mm],
                end: [end_left * mm, end_right * mm],
                gauge,
            },
            transition: Some(kind),
            source: at_bytes(segment.span()),
        })
    }

    /// 測点。`IfcReferent`（8 PredefinedType が `STATION`）の `Pset_Stationing` の `Station`・`IncomingStation`
    /// と、6 ObjectPlacement の `IfcLinearPlacement` の距離。`IncomingStation` のないものが初めの測点を決め、
    /// あるものは測点の不連続にする
    fn stations(&self, referents: &[u64], cx: &mut Cx) -> Stationing {
        let mut stations = Stationing::default();
        let mut start = None;
        for &id in referents {
            let Some(record) = self.file.instance(id) else {
                continue;
            };
            if self.enumeration(arg(record, 8)).as_deref() != Some("STATION") {
                continue;
            }
            let Some(distance) = self.distance_along(arg(record, 6)) else {
                self.invalid(record, 6, cx);
                continue;
            };
            let (station, incoming) = self.stationing(id, cx);
            let Some(station) = station else {
                self.invalid(record, 8, cx);
                continue;
            };
            let distance = distance * self.map.scale * self.length;
            let station = station * self.length;
            match incoming {
                Some(incoming) => stations.breaks.push(StationBreak {
                    distance,
                    back: incoming * self.length,
                    ahead: station,
                    source: at_bytes(record.span()),
                }),
                None if start.is_none() => start = Some(station - distance),
                None => {}
            }
        }
        match start {
            Some(start) => stations.start = start,
            None => cx.report(Diagnostic::new(codes::IFC_STATION_MISSING)),
        }
        stations
            .breaks
            .sort_by(|a, b| a.distance.total_cmp(&b.distance));
        stations
    }

    /// `IfcLinearPlacement` の 2 RelativePlacement → `IfcAxis2PlacementLinear` の 1 Location →
    /// `IfcPointByDistanceExpression` の 1 DistanceAlong
    fn distance_along(&self, value: Option<&Value>) -> Option<f64> {
        let linear = self.instance_of(value, b"IFCLINEARPLACEMENT")?;
        let axes = self.instance_of(arg(linear, 2), b"IFCAXIS2PLACEMENTLINEAR")?;
        let point = self.instance_of(arg(axes, 1), b"IFCPOINTBYDISTANCEEXPRESSION")?;
        self.real(arg(point, 1))
    }

    /// `Pset_Stationing` の `Station` と `IncomingStation`。`IfcRelDefinesByProperties` の 5 RelatedObjects・
    /// 6 RelatingPropertyDefinition → `IfcPropertySet` の 3 Name・5 HasProperties →
    /// `IfcPropertySingleValue` の 1 Name・3 NominalValue
    fn stationing(&self, id: u64, cx: &mut Cx) -> (Option<f64>, Option<f64>) {
        let (mut station, mut incoming) = (None, None);
        for &rel in self.inverse.referrers(id) {
            let Some(record) = self.file.instance(rel) else {
                continue;
            };
            if !self
                .file
                .kind(record)
                .eq_ignore_ascii_case(b"IFCRELDEFINESBYPROPERTIES")
                || !refs(arg(record, 5)).contains(&id)
            {
                continue;
            }
            let Some(set) = self.instance_of(arg(record, 6), b"IFCPROPERTYSET") else {
                continue;
            };
            if self.text(arg(set, 3), set, cx).as_deref() != Some("Pset_Stationing") {
                continue;
            }
            for property in refs(arg(set, 5)) {
                let Some(property) = self.file.instance(property) else {
                    continue;
                };
                if !self
                    .file
                    .kind(property)
                    .eq_ignore_ascii_case(b"IFCPROPERTYSINGLEVALUE")
                {
                    continue;
                }
                let value = self.real(arg(property, 3));
                match self.text(arg(property, 1), property, cx).as_deref() {
                    Some("Station") => station = value,
                    Some("IncomingStation") => incoming = value,
                    _ => {}
                }
            }
        }
        (station, incoming)
    }
}

/// 点の初めの 2 つの値
fn xy(point: &[f64]) -> [f64; 2] {
    [
        point.first().copied().unwrap_or(0.0),
        point.get(1).copied().unwrap_or(0.0),
    ]
}

/// SI の接頭辞の倍率
const PREFIXES: &[(&str, f64)] = &[
    ("KILO", 1e3),
    ("HECTO", 1e2),
    ("DECA", 1e1),
    ("DECI", 1e-1),
    ("CENTI", 1e-2),
    ("MILLI", 1e-3),
    ("MICRO", 1e-6),
];
