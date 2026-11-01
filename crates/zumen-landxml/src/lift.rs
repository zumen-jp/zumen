//! J-LandXML の要素を、土木の領域モデル（決定 0230）に移す。
//!
//! 測点（`sta`・`staStart`・`staInternal`）は、線形の初めからの連続した内部の値（メートル）なので、追加距離は
//! 測点から線形の `staStart` を引いて求める。平面線形の区間の初めの向きは、要素の点から求める（交換標準は
//! 方向角の属性を書かない）。移さない要素と読めない値は、診断にして元の要素を指す。

use std::collections::{BTreeMap, BTreeSet};

use zumen_base::{Diagnostic, Kind, Ref, Stop, codes};
use zumen_civil::{
    Alignment, CivilModel, CrossSection, CrossSectionLine, Horizontal, HorizontalKind,
    StationBreak, Stationing, SurfaceRole, Tin, TransitionCurve, Vertical, VerticalKind,
};
use zumen_crs::{Crs, HeightDatum, MapPoint, Realization, Zone};
use zumen_sdk::{Cx, Error};
use zumen_xml::{Document, Element, Node};

pub(crate) fn lift(doc: &Document, cx: &mut Cx) -> Result<CivilModel, Error> {
    let mut reader = Reader { doc, cx };
    reader.model()
}

struct Reader<'d, 'c, 'b> {
    doc: &'d Document,
    cx: &'c mut Cx<'b>,
}

/// 要素の値が読めない理由（属性か、文字なら `text`）
struct Invalid(&'static str);

impl<'d> Reader<'d, '_, '_> {
    fn model(&mut self) -> Result<CivilModel, Error> {
        let mut model = CivilModel::default();
        let Some(root) = self.doc.element(self.doc.root()) else {
            return Ok(model);
        };
        let mut seen = Seen::default();
        let mut crs = false;
        for child in self.children(root) {
            match self.local(child) {
                b"CoordinateSystem" => {
                    crs = true;
                    self.coordinate_system(child, &mut model);
                }
                b"Alignments" => {
                    for alignment in self.named(child, b"Alignment") {
                        let alignment = self.alignment(alignment, &mut seen)?;
                        model.alignments.push(alignment);
                    }
                }
                b"Surfaces" => {
                    for surface in self.named(child, b"Surface") {
                        if let Some(tin) = self.surface(surface, &mut seen)? {
                            model.surfaces.push(tin);
                        }
                    }
                }
                // 版と作者と単位の情報。単位は、交換標準がメートルと決めている
                b"Project" | b"Application" | b"Units" => {}
                _ => self.unsupported(child),
            }
        }
        if !crs {
            self.cx.report(
                Diagnostic::new(codes::LANDXML_CRS_UNSUPPORTED)
                    .with("datum", "")
                    .with("system", ""),
            );
        }
        Ok(model)
    }

    /// 座標参照系。測地系は JGD2011・JGD2024 を、座標系は平面直角座標系（`1(X,Y)`〜`19(X,Y)`）を読む
    fn coordinate_system(&mut self, element: &Element, model: &mut CivilModel) {
        let datum = self
            .attribute(element, b"horizontalDatum")
            .unwrap_or_default();
        let system = self
            .attribute(element, b"horizontalCoordinateSystemName")
            .unwrap_or_default();
        let zone = system
            .trim()
            .strip_suffix("(X,Y)")
            .and_then(|number| number.trim().parse::<u8>().ok())
            .and_then(Zone::new);
        let realization = match datum.trim() {
            "JGD2011" => Some(Realization::Survey2011),
            "JGD2024" => Some(Realization::Survey2024),
            _ => None,
        };
        match (zone, realization) {
            (Some(zone), Some(realization)) => {
                model.crs = Some(Crs::Plane(zone));
                model.realization = Some(realization);
            }
            _ => self.cx.report(
                Diagnostic::new(codes::LANDXML_CRS_UNSUPPORTED)
                    .with("datum", datum.as_str())
                    .with("system", system.as_str())
                    .at(self.at(element)),
            ),
        }
        let height = self
            .attribute(element, b"verticalDatum")
            .unwrap_or_default();
        match height.trim() {
            "T.P" | "T.P." => model.height_datum = Some(HeightDatum::TokyoBay),
            _ => self.cx.report(
                Diagnostic::new(codes::LANDXML_HEIGHT_DATUM_UNSUPPORTED)
                    .with("datum", height.as_str())
                    .at(self.at(element)),
            ),
        }
    }

    fn alignment(&mut self, element: &Element, seen: &mut Seen) -> Result<Alignment, Error> {
        self.spend(element, 1)?;
        let name = self.attribute(element, b"name");
        let index = seen.alignments;
        seen.alignments += 1;
        let id = seen.id(name.as_deref(), "alignment", index);
        let start = match self.number(element, b"staStart") {
            Some(start) => start,
            None => {
                self.invalid(element, "staStart");
                0.0
            }
        };
        let mut alignment = Alignment {
            id,
            name,
            stations: Stationing {
                start,
                ..Stationing::default()
            },
            source: self.at(element),
            ..Alignment::default()
        };
        let mut profiled = false;
        for child in self.children(element) {
            match self.local(child) {
                b"CoordGeom" => self.coord_geom(child, &mut alignment)?,
                b"StaEquation" => self.station_break(child, &mut alignment),
                b"Profile" => {
                    for profile in self.children(child) {
                        if self.local(profile) == b"ProfAlign" && !profiled {
                            profiled = true;
                            alignment.vertical = self.profile(profile, start)?;
                        } else {
                            // 2 つ目からの縦断線形と、縦断地盤線
                            self.unsupported(profile);
                        }
                    }
                }
                b"CrossSects" => {
                    for section in self.named(child, b"CrossSect") {
                        if let Some(section) = self.cross_section(section, start)? {
                            alignment.cross_sections.push(section);
                        }
                    }
                }
                b"Feature" => self.alignment_feature(child, &mut alignment),
                _ => self.unsupported(child),
            }
        }
        alignment
            .stations
            .breaks
            .sort_by(|a, b| a.distance.total_cmp(&b.distance));
        Ok(alignment)
    }

    /// 線形の `Feature`。測点の間隔（`Interval` の `main`）だけを読む
    fn alignment_feature(&mut self, element: &Element, alignment: &mut Alignment) {
        if self.attribute(element, b"name").as_deref() != Some("Interval") {
            return;
        }
        for property in self.named(element, b"Property") {
            if self.attribute(property, b"label").as_deref().map(str::trim) == Some("main") {
                match self.number(property, b"value") {
                    Some(pitch) if pitch > 0.0 => alignment.stations.pitch = Some(pitch),
                    _ => self.invalid(property, "value"),
                }
            }
        }
    }

    fn station_break(&mut self, element: &Element, alignment: &mut Alignment) {
        let internal = self.number(element, b"staInternal");
        let ahead = self.number(element, b"staAhead");
        let (Some(internal), Some(ahead)) = (internal, ahead) else {
            self.invalid(
                element,
                if internal.is_none() {
                    "staInternal"
                } else {
                    "staAhead"
                },
            );
            return;
        };
        let distance = internal - alignment.stations.start;
        let back = self
            .number(element, b"staBack")
            .unwrap_or_else(|| alignment.stations.value(distance));
        alignment.stations.breaks.push(StationBreak {
            distance,
            back,
            ahead,
            source: self.at(element),
        });
    }

    /// 平面線形。要素の点から、区間の初めの点・向き・長さを求める
    fn coord_geom(&mut self, element: &Element, alignment: &mut Alignment) -> Result<(), Error> {
        for child in self.children(element) {
            self.spend(child, 1)?;
            let segment = match self.local(child) {
                b"Line" => self.line(child),
                b"Curve" => self.curve(child),
                b"Spiral" => self.spiral(child, alignment),
                _ => {
                    self.unsupported(child);
                    continue;
                }
            };
            match segment {
                Ok(segment) => alignment.horizontal.push(segment),
                Err(Invalid(field)) => self.invalid(child, field),
            }
        }
        Ok(())
    }

    fn line(&mut self, element: &Element) -> Result<Horizontal, Invalid> {
        let start = self.point_of(element, b"Start").ok_or(Invalid("Start"))?;
        let end = self.point_of(element, b"End").ok_or(Invalid("End"))?;
        let length = match self.number(element, b"length") {
            Some(length) => length,
            None => libm::hypot(end.northing - start.northing, end.easting - start.easting),
        };
        Ok(Horizontal {
            start,
            direction: azimuth(start, end),
            length: positive(length).ok_or(Invalid("length"))?,
            kind: HorizontalKind::Line,
            source: self.at(element),
        })
    }

    fn curve(&mut self, element: &Element) -> Result<Horizontal, Invalid> {
        let right = self.right(element)?;
        let start = self.point_of(element, b"Start").ok_or(Invalid("Start"))?;
        let center = self.point_of(element, b"Center").ok_or(Invalid("Center"))?;
        let end = self.point_of(element, b"End").ok_or(Invalid("End"))?;
        let radius = match self.number(element, b"radius") {
            Some(radius) => radius,
            None => libm::hypot(
                start.northing - center.northing,
                start.easting - center.easting,
            ),
        };
        let radius = positive(radius).ok_or(Invalid("radius"))?;
        // 中心は、右へ曲がるなら進む向きの右にある
        let toward = azimuth(start, center);
        let direction = if right {
            toward - core::f64::consts::FRAC_PI_2
        } else {
            toward + core::f64::consts::FRAC_PI_2
        };
        let length = match self.number(element, b"length") {
            Some(length) => length,
            None => {
                let mut turn = azimuth(center, end) - azimuth(center, start);
                if !right {
                    turn = -turn;
                }
                radius
                    * libm::fmod(
                        turn + 4.0 * core::f64::consts::PI,
                        2.0 * core::f64::consts::PI,
                    )
            }
        };
        Ok(Horizontal {
            start,
            direction,
            length: positive(length).ok_or(Invalid("length"))?,
            kind: HorizontalKind::Arc {
                radius: if right { radius } else { -radius },
            },
            source: self.at(element),
        })
    }

    /// 緩和曲線。初めの向きは、初めの点から交点（`PI`）への向きにし、交点がなければ前の区間の終わりの向きにする
    fn spiral(&mut self, element: &Element, alignment: &Alignment) -> Result<Horizontal, Invalid> {
        let right = self.right(element)?;
        let start = self.point_of(element, b"Start").ok_or(Invalid("Start"))?;
        let length = self
            .number(element, b"length")
            .and_then(positive)
            .ok_or(Invalid("length"))?;
        let radius = |reader: &Self, name: &'static [u8], field: &'static str| match reader
            .attribute(element, name)
            .as_deref()
            .map(str::trim)
        {
            Some("INF") | Some("inf") | Some("Infinity") => Ok(None),
            Some(text) => text
                .parse::<f64>()
                .ok()
                .filter(|radius| radius.is_finite() && *radius > 0.0)
                .map(|radius| Some(if right { radius } else { -radius }))
                .ok_or(Invalid(field)),
            None => Err(Invalid(field)),
        };
        let start_radius = radius(self, b"radiusStart", "radiusStart")?;
        let end_radius = radius(self, b"radiusEnd", "radiusEnd")?;
        let direction = match self.point_of(element, b"PI") {
            Some(pi) => azimuth(start, pi),
            None => {
                let before = zumen_civil::length(alignment);
                let mut quiet = zumen_base::Diagnostics::new();
                let mut cx = Cx::new(self.cx.budget, &mut quiet, self.cx.cancel);
                zumen_civil::place(alignment, before, &mut cx)
                    .map(|placed| placed.direction)
                    .map_err(|_| Invalid("PI"))?
            }
        };
        let kind = self.attribute(element, b"spiType").unwrap_or_default();
        let curve = match kind.trim() {
            "clothoid" => TransitionCurve::Clothoid,
            "cubicParabola" | "cubic" | "japaneseCubic" => TransitionCurve::CubicParabola,
            "sineHalfWave" => TransitionCurve::SineHalfWave,
            "cosine" => TransitionCurve::Cosine,
            "bloss" => TransitionCurve::Bloss,
            "weinerBogen" => TransitionCurve::Viennese,
            other => TransitionCurve::Other(other.to_string()),
        };
        Ok(Horizontal {
            start,
            direction,
            length,
            kind: HorizontalKind::Transition {
                start_radius,
                end_radius,
                curve,
            },
            source: self.at(element),
        })
    }

    /// `rot` が `cw`（右へ曲がる）なら真
    fn right(&self, element: &Element) -> Result<bool, Invalid> {
        match self.attribute(element, b"rot").as_deref().map(str::trim) {
            Some("cw") => Ok(true),
            Some("ccw") => Ok(false),
            _ => Err(Invalid("rot")),
        }
    }

    /// 縦断線形。縦断の変化点（`PVI`）と縦断曲線（`ParaCurve`）の並びを、一定の勾配と 2 次放物線の区間に直す
    fn profile(&mut self, element: &Element, start: f64) -> Result<Vec<Vertical>, Error> {
        let mut points = Vec::new();
        for child in self.children(element) {
            self.spend(child, 1)?;
            let curve = match self.local(child) {
                b"PVI" => 0.0,
                b"ParaCurve" => match self.number(child, b"length").filter(|l| *l >= 0.0) {
                    Some(length) => length,
                    None => {
                        self.invalid(child, "length");
                        return Ok(Vec::new());
                    }
                },
                _ => {
                    // 円の縦断曲線と、対称でない放物線
                    self.unsupported(child);
                    return Ok(Vec::new());
                }
            };
            let values = numbers(&self.text(child));
            let [sta, elevation] = values.as_slice() else {
                self.invalid(child, "text");
                return Ok(Vec::new());
            };
            points.push((*sta - start, *elevation, curve, self.at(child)));
        }
        match vertical(&points) {
            Some(segments) => Ok(segments),
            None => {
                self.invalid(element, "PVI");
                Ok(Vec::new())
            }
        }
    }

    fn cross_section(
        &mut self,
        element: &Element,
        start: f64,
    ) -> Result<Option<CrossSection>, Error> {
        self.spend(element, 1)?;
        let Some(sta) = self.number(element, b"sta") else {
            self.invalid(element, "sta");
            return Ok(None);
        };
        let distance = sta - start;
        let mut lines = Vec::new();
        for child in self.children(element) {
            match self.local(child) {
                b"DesignCrossSectSurf" => {
                    if let Some(line) = self.design_line(child)? {
                        lines.push(line);
                    }
                }
                b"CrossSectSurf" => {
                    let name = self.attribute(child, b"name");
                    let role = match name.as_deref() {
                        Some("ExistingGround") => SurfaceRole::Existing,
                        other => SurfaceRole::Other(other.unwrap_or_default().to_string()),
                    };
                    for list in self.children(child) {
                        match self.local(list) {
                            b"PntList2D" => {
                                let values = numbers(&self.text(list));
                                self.spend(list, values.len() as u64 / 2)?;
                                match pairs(&values) {
                                    Some(points) => lines.push(CrossSectionLine {
                                        name: name.clone(),
                                        role: role.clone(),
                                        points,
                                    }),
                                    None => self.invalid(list, "text"),
                                }
                            }
                            b"Feature" => {}
                            _ => self.unsupported(list),
                        }
                    }
                }
                b"Feature" => {}
                _ => self.unsupported(child),
            }
        }
        Ok(Some(CrossSection {
            distance,
            name: self.attribute(element, b"name"),
            lines,
            source: self.at(element),
        }))
    }

    /// 計画の横断の線。点は、中心線からの離れと標高（`OffsetElevation`）で書いたものだけを移す
    fn design_line(&mut self, element: &Element) -> Result<Option<CrossSectionLine>, Error> {
        let name = self.attribute(element, b"name");
        let label = name.clone().unwrap_or_default();
        for feature in self.named(element, b"Feature") {
            for property in self.named(feature, b"Property") {
                let label_of = self.attribute(property, b"label");
                let value = self.attribute(property, b"value");
                if label_of.as_deref().map(str::trim) == Some("heightType")
                    && value.as_deref().map(str::trim) == Some("fhOffset")
                {
                    self.cx.report(
                        Diagnostic::new(codes::LANDXML_CROSS_SECTION_LINE_UNSUPPORTED)
                            .with("name", label.as_str())
                            .with("form", "fhOffset")
                            .at(self.at(element)),
                    );
                    return Ok(None);
                }
            }
        }
        let mut points = Vec::new();
        for point in self.named(element, b"CrossSectPnt") {
            self.spend(point, 1)?;
            let form = self.attribute(point, b"dataFormat").unwrap_or_default();
            if !matches!(form.trim(), "" | "OffsetElevation" | "Offset Elevation") {
                self.cx.report(
                    Diagnostic::new(codes::LANDXML_CROSS_SECTION_LINE_UNSUPPORTED)
                        .with("name", label.as_str())
                        .with("form", form.trim())
                        .at(self.at(element)),
                );
                return Ok(None);
            }
            match numbers(&self.text(point)).as_slice() {
                [offset, height] => points.push([*offset, *height]),
                _ => {
                    self.invalid(point, "text");
                    return Ok(None);
                }
            }
        }
        Ok(Some(CrossSectionLine {
            name,
            role: SurfaceRole::Design,
            points,
        }))
    }

    fn surface(&mut self, element: &Element, seen: &mut Seen) -> Result<Option<Tin>, Error> {
        let name = self.attribute(element, b"name");
        let index = seen.surfaces;
        seen.surfaces += 1;
        let id = seen.id(name.as_deref(), "surface", index);
        let label = name.clone().unwrap_or_default();
        let role = match self.attribute(element, b"desc").as_deref().map(str::trim) {
            Some("ExistingGround") => SurfaceRole::Existing,
            Some("FinishedGrade") => SurfaceRole::Design,
            other => SurfaceRole::Other(other.unwrap_or_default().to_string()),
        };
        let mut tin = Tin {
            id,
            name,
            role,
            points: Vec::new(),
            triangles: Vec::new(),
            boundaries: Vec::new(),
            breaklines: Vec::new(),
            source: self.at(element),
        };
        let mut ids: BTreeMap<u64, u32> = BTreeMap::new();
        let mut area: Vec<u64> = Vec::new();
        let mut defined = false;
        for child in self.children(element) {
            match self.local(child) {
                b"Definition" => {
                    let kind = self
                        .attribute(child, b"surfType")
                        .or_else(|| self.attribute(child, b"SurfType"))
                        .unwrap_or_default();
                    if kind.trim() != "TIN" || defined {
                        self.unsupported(child);
                        continue;
                    }
                    defined = true;
                    if !self.definition(child, &mut tin, &mut ids, &label)? {
                        return Ok(None);
                    }
                }
                b"Feature" => {
                    if self.attribute(child, b"name").as_deref() == Some("AreaPnts") {
                        for property in self.named(child, b"Property") {
                            if self.attribute(property, b"label").as_deref().map(str::trim)
                                == Some("areaList")
                            {
                                let value = self.attribute(property, b"value").unwrap_or_default();
                                area.extend(
                                    value
                                        .split_whitespace()
                                        .filter_map(|id| id.parse::<u64>().ok()),
                                );
                            }
                        }
                    }
                }
                _ => self.unsupported(child),
            }
        }
        if !area.is_empty() {
            match area
                .iter()
                .map(|id| ids.get(id).copied())
                .collect::<Option<Vec<u32>>>()
            {
                Some(boundary) => tin.boundaries.push(boundary),
                None => self.invalid(element, "areaList"),
            }
        }
        Ok(Some(tin))
    }

    /// TIN の点と面。読めない点か面があれば、偽を返す
    fn definition(
        &mut self,
        element: &Element,
        tin: &mut Tin,
        ids: &mut BTreeMap<u64, u32>,
        label: &str,
    ) -> Result<bool, Error> {
        let mut hidden = 0_u64;
        for part in self.children(element) {
            match self.local(part) {
                b"Pnts" => {
                    for point in self.named(part, b"P") {
                        self.spend(point, 1)?;
                        let id = self
                            .attribute(point, b"id")
                            .and_then(|id| id.trim().parse::<u64>().ok());
                        let values = numbers(&self.text(point));
                        let (Some(id), [northing, easting, height]) = (id, values.as_slice())
                        else {
                            self.invalid(point, if id.is_none() { "id" } else { "text" });
                            return Ok(false);
                        };
                        let Ok(index) = u32::try_from(tin.points.len()) else {
                            self.invalid(point, "id");
                            return Ok(false);
                        };
                        if ids.insert(id, index).is_some() {
                            self.invalid(point, "id");
                            return Ok(false);
                        }
                        tin.points.push((
                            MapPoint {
                                northing: *northing,
                                easting: *easting,
                            },
                            *height,
                        ));
                    }
                }
                b"Faces" => {
                    for face in self.named(part, b"F") {
                        self.spend(face, 1)?;
                        if self.attribute(face, b"i").as_deref().map(str::trim) == Some("1") {
                            hidden += 1;
                            continue;
                        }
                        let corners: Option<Vec<u32>> = self
                            .text(face)
                            .split_whitespace()
                            .map(|id| id.parse::<u64>().ok().and_then(|id| ids.get(&id).copied()))
                            .collect();
                        match corners.as_deref() {
                            Some([a, b, c]) => tin.triangles.push([*a, *b, *c]),
                            _ => {
                                self.invalid(face, "text");
                                return Ok(false);
                            }
                        }
                    }
                }
                _ => self.unsupported(part),
            }
        }
        if hidden > 0 {
            self.cx.report(
                Diagnostic::new(codes::LANDXML_FACE_DROPPED)
                    .with("surface", label)
                    .with("faces", hidden)
                    .at(self.at(element)),
            );
        }
        Ok(true)
    }

    // 要素をたどる道具

    fn local(&self, element: &Element) -> &'d [u8] {
        self.doc.local_name(element.name())
    }

    fn children(&self, element: &Element) -> Vec<&'d Element> {
        element
            .children()
            .iter()
            .filter_map(|node| match node {
                Node::Element(index) => self.doc.element(*index),
                _ => None,
            })
            .collect()
    }

    fn named(&self, element: &Element, local: &[u8]) -> Vec<&'d Element> {
        self.children(element)
            .into_iter()
            .filter(|child| self.local(child) == local)
            .collect()
    }

    fn attribute(&self, element: &Element, local: &[u8]) -> Option<String> {
        self.doc
            .attribute(element, None, local)
            .map(|attribute| self.doc.value(attribute))
    }

    fn number(&self, element: &Element, local: &[u8]) -> Option<f64> {
        self.attribute(element, local)?
            .trim()
            .parse::<f64>()
            .ok()
            .filter(|value| value.is_finite())
    }

    fn text(&self, element: &Element) -> String {
        element
            .children()
            .iter()
            .filter_map(|node| match node {
                Node::Text(range) => Some(self.doc.text(range.clone())),
                Node::CData(range) => Some(self.doc.cdata(range.clone())),
                Node::Element(_) => None,
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// 子の要素（`Start`・`End`・`Center`・`PI`）の点
    fn point_of(&self, element: &Element, local: &[u8]) -> Option<MapPoint> {
        let child = self.named(element, local).into_iter().next()?;
        match numbers(&self.text(child)).as_slice() {
            [northing, easting, ..] => Some(MapPoint {
                northing: *northing,
                easting: *easting,
            }),
            _ => None,
        }
    }

    fn at(&self, element: &Element) -> Ref {
        Ref::at_bytes(element.span().start as u64, element.span().end as u64)
    }

    fn unsupported(&mut self, element: &Element) {
        let name = String::from_utf8_lossy(self.local(element)).into_owned();
        let at = self.at(element);
        self.cx.report(
            Diagnostic::new(codes::LANDXML_ELEMENT_UNSUPPORTED)
                .with("element", name)
                .at(at),
        );
    }

    fn invalid(&mut self, element: &Element, field: &'static str) {
        let name = String::from_utf8_lossy(self.local(element)).into_owned();
        let at = self.at(element);
        self.cx.report(
            Diagnostic::new(codes::LANDXML_VALUE_INVALID)
                .with("element", name)
                .with("field", field)
                .at(at),
        );
    }

    fn spend(&mut self, element: &Element, amount: u64) -> Result<(), Error> {
        self.cx.check(Kind::Elements, amount).map_err(|stop: Stop| {
            let at = self.at(element);
            self.cx.report(stop.to_diagnostic().at(at));
            Error::Stopped(stop)
        })
    }
}

/// 使った ID。名前のない要素と、名前が重なる要素は、種類と順の番号で ID を作る（決定 0230）
#[derive(Default)]
struct Seen {
    ids: BTreeSet<String>,
    alignments: usize,
    surfaces: usize,
}

impl Seen {
    fn id(&mut self, name: Option<&str>, kind: &str, index: usize) -> String {
        let id = match name.map(str::trim).filter(|name| !name.is_empty()) {
            Some(name) if !self.ids.contains(name) => name.to_string(),
            _ => format!("{kind}/{index}"),
        };
        self.ids.insert(id.clone());
        id
    }
}

/// 点 `from` から点 `to` への方向角（北から時計回りのラジアン）
fn azimuth(from: MapPoint, to: MapPoint) -> f64 {
    libm::atan2(to.easting - from.easting, to.northing - from.northing)
}

fn positive(value: f64) -> Option<f64> {
    (value.is_finite() && value > 0.0).then_some(value)
}

fn numbers(text: &str) -> Vec<f64> {
    text.split_whitespace()
        .map(|word| word.parse::<f64>().ok().filter(|value| value.is_finite()))
        .collect::<Option<Vec<f64>>>()
        .unwrap_or_default()
}

fn pairs(values: &[f64]) -> Option<Vec<[f64; 2]>> {
    let (pairs, rest) = values.as_chunks::<2>();
    (!pairs.is_empty() && rest.is_empty()).then(|| pairs.to_vec())
}

/// 縦断の変化点（追加距離・標高・縦断曲線の長さ・出所）の並びを、区間のならびにする。追加距離が増えないか、
/// 縦断曲線が重なれば `None`
fn vertical(points: &[(f64, f64, f64, Ref)]) -> Option<Vec<Vertical>> {
    let grades: Vec<f64> = points
        .windows(2)
        .map(|pair| match pair {
            [(s0, e0, ..), (s1, e1, ..)] if s1 > s0 => Some((e1 - e0) / (s1 - s0)),
            _ => None,
        })
        .collect::<Option<_>>()?;
    let (first, rest) = points.split_first()?;
    let mut segments = Vec::new();
    let mut cursor = (first.0, first.1, first.3.clone());
    for (index, (sta, elevation, length, source)) in rest.iter().enumerate() {
        let before = *grades.get(index)?;
        let last = index + 1 == rest.len();
        let half = if last { 0.0 } else { length / 2.0 };
        let curve_start = sta - half;
        if curve_start < cursor.0 - 1e-9 {
            return None;
        }
        if curve_start > cursor.0 {
            segments.push(Vertical {
                start_distance: cursor.0,
                start_height: cursor.1,
                start_gradient: before,
                length: curve_start - cursor.0,
                kind: VerticalKind::ConstantGradient,
                source: cursor.2.clone(),
            });
        }
        if half > 0.0 {
            let after = *grades.get(index + 1)?;
            segments.push(Vertical {
                start_distance: curve_start,
                start_height: elevation - before * half,
                start_gradient: before,
                length: *length,
                kind: VerticalKind::Parabola {
                    end_gradient: after,
                },
                source: source.clone(),
            });
            cursor = (sta + half, elevation + after * half, source.clone());
        } else {
            cursor = (*sta, *elevation, source.clone());
        }
    }
    Some(segments)
}
