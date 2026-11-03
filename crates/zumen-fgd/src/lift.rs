//! 基盤地図情報の要素を、地理の領域モデル（決定 0233）に移す。
//!
//! - 根の `Dataset` の子で、基盤地図情報の名前空間の要素を地物にする（ファイル仕様書 4.2.1 節）。型は `fgd:` と
//!   要素の名前（`fgd:RdEdg` など、4.2.6 節の表 4-3）。
//! - 性質 `type`（種別）を分類コード、`name`（名称）を名前にする。ほかの性質は、応用スキーマ（2.2.2 節）の型で
//!   属性にする。
//! - 空間属性は、点要素 P・線要素 L・面要素 A・面要素 B の型（2.2.3 節）を読む。境界の曲線の `xlink:href` は、
//!   同じ文書の `gml:id` を引く。
//! - DEM 区画の被覆（`CV_DiscreteGridPointCoverage`）は、セルの中心の点を結ぶ三角形の集まりにする。

use std::collections::BTreeMap;

use zumen_base::{Diagnostic, Kind, Ref, Stop, codes};
use zumen_crs::{Crs, GeoPoint, MapPoint, VerticalCrs};
use zumen_geo::{Attribute, Feature, Form, GeoModel, Geometry, Location, Position, Value};
use zumen_geom3d::{Shape, Xyz};
use zumen_sdk::{Cx, Error};
use zumen_xml::{Document, Element, Node};

use crate::NAMESPACE;

const GML: &str = "http://www.opengis.net/gml/3.2";
const XLINK: &str = "http://www.w3.org/1999/xlink";

/// `xlink:href` を引く深さの上限。循環した参照で止まらないようにする
const MAX_DEPTH: u32 = 32;

/// DEM 構成点の「データなし」の標高値（2.2.2 節の DEM 構成点）
const NO_DATA: f64 = -9999.0;

pub(crate) fn lift(doc: &Document, cx: &mut Cx) -> Result<GeoModel, Error> {
    let mut reader = Reader {
        doc,
        cx,
        ids: BTreeMap::new(),
        model: GeoModel::default(),
        srs: None,
    };
    reader.model()?;
    Ok(reader.model)
}

struct Reader<'d, 'c, 'b> {
    doc: &'d Document,
    cx: &'c mut Cx<'b>,
    /// `gml:id` と要素の番号
    ids: BTreeMap<String, usize>,
    model: GeoModel,
    /// 文書の座標系の `srsName`
    srs: Option<String>,
}

impl<'d> Reader<'d, '_, '_> {
    fn model(&mut self) -> Result<(), Error> {
        let doc = self.doc;
        let Some(root) = doc.element(doc.root()) else {
            return Err(Error::Invalid);
        };
        self.spend(root, doc.elements().len() as u64)?;
        for (index, element) in doc.elements().iter().enumerate() {
            if let Some(id) = doc.attribute(element, Some(GML), b"id") {
                self.ids.entry(doc.value(id)).or_insert(index);
            }
        }
        self.crs();
        let mut order = 0;
        for child in self.children(root) {
            let local = self.local(child);
            if doc.is(child.name(), Some(GML), local) {
                if !matches!(
                    local,
                    b"name" | b"description" | b"identifier" | b"boundedBy"
                ) {
                    self.unsupported(child);
                }
                continue;
            }
            if !doc.is(child.name(), Some(NAMESPACE), local) {
                self.unsupported(child);
                continue;
            }
            // データセットの説明（`description`）。地物でない
            if local == b"description" && self.children(child).is_empty() {
                continue;
            }
            self.feature(child, None, order)?;
            order += 1;
        }
        Ok(())
    }

    /// 文書の座標系。初めに `srsName` を持つ要素の値から決める（3.1 節）
    fn crs(&mut self) {
        let doc = self.doc;
        let found = doc.elements().iter().find_map(|element| {
            let name = doc.attribute(element, None, b"srsName")?;
            Some((doc.value(name).trim().to_string(), element))
        });
        let Some((name, element)) = found else {
            // 形を持たない地物だけの文書。座標系は要らない
            return;
        };
        let (crs, height) = crs(&name);
        if crs.is_none() {
            let at = self.at(element);
            self.cx.report(
                Diagnostic::new(codes::FGD_CRS_UNSUPPORTED)
                    .with("name", name.as_str())
                    .at(at),
            );
        }
        self.model.crs = crs;
        self.model.height = height;
        self.srs = Some(name);
    }

    fn feature(
        &mut self,
        element: &'d Element,
        parent: Option<usize>,
        order: usize,
    ) -> Result<(), Error> {
        self.spend(element, 1)?;
        let doc = self.doc;
        let kind = format!("fgd:{}", String::from_utf8_lossy(self.local(element)));
        let id = match doc.attribute(element, Some(GML), b"id") {
            Some(id) => doc.value(id),
            None => {
                let parent_id = parent
                    .and_then(|index| self.model.features.get(index))
                    .map(|feature| format!("{}/", feature.id))
                    .unwrap_or_default();
                format!("{parent_id}{kind}/{order}")
            }
        };
        let index = self.model.features.len();
        self.model.features.push(Feature {
            id,
            kind,
            parent,
            source: self.at(element),
            ..Feature::default()
        });
        let mut name = None;
        let mut code = None;
        let mut attributes = Vec::new();
        let mut geometries = Vec::new();
        let mut height = None;
        let mut children = 0;
        for property in self.children(element) {
            let local = self.local(property);
            if doc.is(property.name(), Some(GML), local) {
                match local {
                    b"name" if name.is_none() => {
                        name = Some(self.text(property).trim().to_string())
                    }
                    b"description" => attributes.push(Attribute {
                        name: "gml:description".to_string(),
                        value: Value::Text(self.text(property).trim().to_string()),
                    }),
                    _ => {}
                }
                continue;
            }
            if !doc.is(property.name(), Some(NAMESPACE), local) {
                self.unsupported(property);
                continue;
            }
            match local {
                b"type" if code.is_none() => {
                    code = Some(self.text(property).trim().to_string());
                    continue;
                }
                b"name" if name.is_none() => {
                    name = Some(self.text(property).trim().to_string());
                    continue;
                }
                // 整備データ登録日・削除日・整備完了日（`TM_Instant`、yyyy-mm-dd）
                b"lfSpanFr" | b"lfSpanTo" | b"devDate" => {
                    let text = self
                        .named(property, b"timePosition")
                        .into_iter()
                        .next()
                        .map(|position| self.text(position).trim().to_string());
                    match text {
                        Some(text) => attributes.push(Attribute {
                            name: self.qualified(property),
                            value: if is_date(&text) {
                                Value::Date(text)
                            } else {
                                Value::Text(text)
                            },
                        }),
                        None => self.invalid(property, "timePosition"),
                    }
                    continue;
                }
                b"coverage" => {
                    if let Some(form) = self.coverage(element, property, &mut attributes)? {
                        geometries.push(Geometry { lod: None, form });
                    }
                    continue;
                }
                _ => {}
            }
            let inner = self.children(property);
            if inner.is_empty() {
                let value = self.value(property, local);
                if let Some(value) = value {
                    if local == b"alti"
                        && let Value::Real(alti) = value
                    {
                        height = Some(alti);
                    }
                    attributes.push(Attribute {
                        name: self.qualified(property),
                        value,
                    });
                }
                continue;
            }
            for child in inner {
                if doc.is(child.name(), Some(GML), self.local(child)) {
                    geometries.extend(
                        self.forms(child, 0)?
                            .into_iter()
                            .map(|form| Geometry { lod: None, form }),
                    );
                } else if doc.is(child.name(), Some(NAMESPACE), self.local(child)) {
                    // 関連の先の地物を、関連の中に書いたもの
                    self.feature(child, Some(index), children)?;
                    children += 1;
                } else {
                    self.unsupported(child);
                }
            }
        }
        // 標高点・等高線・測量の基準点の標高（`alti`）を、2 次元の点の標高にする
        if let Some(alti) = height {
            for geometry in &mut geometries {
                with_height(&mut geometry.form, alti);
            }
        }
        if let Some(feature) = self.model.features.get_mut(index) {
            feature.name = name;
            feature.code = code;
            feature.attributes = attributes;
            feature.geometries = geometries;
        }
        Ok(())
    }

    /// 文字だけを持つ性質の値。型は応用スキーマ（2.2.2 節）の属性の型と、XML スキーマの型から決める。文字がなく
    /// `xlink:href` だけなら（関連）、参照の文字にする
    fn value(&mut self, element: &Element, local: &[u8]) -> Option<Value> {
        let doc = self.doc;
        let text = self.text(element).trim().to_string();
        if text.is_empty() {
            return doc
                .attribute(element, Some(XLINK), b"href")
                .map(|href| Value::Text(doc.value(href)));
        }
        match local {
            // 緯度・経度（秒）・標高・ジオイド高・基準面補正量（`Real`）
            b"B" | b"L" | b"alti" | b"gh" | b"hrc" => {
                let value = real(&text);
                if value.is_none() {
                    self.invalid(element, "text");
                }
                value.map(Value::Real)
            }
            // 標高値有効小数桁数（`Integer`）
            b"altiAcc" => {
                let value = text.parse::<i64>().ok();
                if value.is_none() {
                    self.invalid(element, "text");
                }
                value.map(Value::Integer)
            }
            // 列挙型の属性（出典地図情報レベル・表示区分・管理主体）
            b"orgGILvl" | b"vis" | b"admOffice" => Some(Value::Code {
                space: None,
                value: text,
            }),
            // ほかは文字（基盤地図情報レコード ID・行政コード・街区符号なども、数字の並びのまま）
            _ => Some(Value::Text(text)),
        }
    }

    // 形

    /// GML の空間属性の要素を、形にする（2.2.3 節の空間属性の適用パターン）
    fn forms(&mut self, element: &'d Element, depth: u32) -> Result<Vec<Form>, Error> {
        self.spend(element, 1)?;
        if self.model.crs.is_none() || !self.same_crs(element) {
            return Ok(Vec::new());
        }
        let local = self.local(element);
        Ok(match local {
            // 点要素 P パターン
            b"Point" => self
                .locations(element)
                .into_iter()
                .next()
                .map(Form::Point)
                .into_iter()
                .collect(),
            // 線要素 L パターン
            b"Curve" | b"LineString" | b"OrientableCurve" => self
                .curve(element, depth)?
                .map(|points| Form::Line(self.to_locations(points)))
                .into_iter()
                .collect(),
            // 面要素 A パターン（`gml:Surface` の `gml:PolygonPatch`）と、面要素 B パターン
            // （`gml:PolyhedralSurface` の `gml:Polygon`）
            b"Surface" | b"PolyhedralSurface" => {
                let mut forms = Vec::new();
                for patches in self.children(element) {
                    if !matches!(self.local(patches), b"patches" | b"polygonPatches") {
                        continue;
                    }
                    for patch in self.children(patches) {
                        forms.extend(self.polygon(patch, depth)?);
                    }
                }
                forms
            }
            b"Polygon" => self.polygon(element, depth)?.into_iter().collect(),
            _ => {
                self.unsupported(element);
                Vec::new()
            }
        })
    }

    /// 要素の `srsName` が文書の座標系と同じか。違えば診断する。書いていなければ、文書の座標系とみなす
    fn same_crs(&mut self, element: &Element) -> bool {
        let doc = self.doc;
        let Some(name) = doc
            .attribute(element, None, b"srsName")
            .map(|a| doc.value(a))
        else {
            return true;
        };
        let name = name.trim();
        if self.srs.as_deref() == Some(name) || crs(name).0 == self.model.crs {
            return true;
        }
        let at = self.at(element);
        self.cx.report(
            Diagnostic::new(codes::FGD_CRS_UNSUPPORTED)
                .with("name", name)
                .at(at),
        );
        false
    }

    /// 面の外周と穴。`gml:PolygonPatch` と `gml:Polygon` の `exterior`・`interior` の輪
    fn polygon(&mut self, element: &'d Element, depth: u32) -> Result<Option<Form>, Error> {
        self.spend(element, 1)?;
        if !matches!(self.local(element), b"PolygonPatch" | b"Polygon") {
            self.unsupported(element);
            return Ok(None);
        }
        let mut outer = None;
        let mut holes = Vec::new();
        let mut exterior = false;
        for boundary in self.children(element) {
            let local = self.local(boundary);
            if !matches!(local, b"exterior" | b"interior") {
                continue;
            }
            exterior |= local == b"exterior";
            let mut ring = None;
            for member in self.members(boundary, depth) {
                ring = self.ring(member, depth + 1)?;
            }
            let Some(ring) = ring.map(open) else {
                self.invalid(boundary, "ring");
                continue;
            };
            if ring.len() < 3 {
                self.invalid(boundary, "ring");
                continue;
            }
            let ring = self.to_locations(ring);
            if local == b"exterior" {
                outer = Some(ring);
            } else {
                holes.push(ring);
            }
        }
        let Some(outer) = outer else {
            // 外周の輪が読めなかったときは、輪の診断を返してある
            if !exterior {
                self.invalid(element, "exterior");
            }
            return Ok(None);
        };
        Ok(Some(Form::Area { outer, holes }))
    }

    /// 輪の点。`gml:Ring` は境界の曲線（`curveMember`）を順につなぎ、`gml:LinearRing` は座標の列を読む
    fn ring(&mut self, element: &'d Element, depth: u32) -> Result<Option<Vec<Coordinate>>, Error> {
        self.spend(element, 1)?;
        match self.local(element) {
            b"LinearRing" => Ok(Some(self.coordinates(element))),
            b"Ring" => {
                let mut points = Vec::new();
                for member in self.named(element, b"curveMember") {
                    for curve in self.members(member, depth) {
                        let Some(part) = self.curve(curve, depth + 1)? else {
                            return Ok(None);
                        };
                        join(&mut points, part);
                    }
                }
                Ok(Some(points))
            }
            _ => {
                self.unsupported(element);
                Ok(None)
            }
        }
    }

    /// 曲線の点。`gml:Curve` は区間（`gml:LineStringSegment`）を順につなぐ。`gml:OrientableCurve` は基の曲線を
    /// 引き、`orientation="-"` なら逆に辿る（面要素 B パターンのマイナス方向の参照）
    fn curve(
        &mut self,
        element: &'d Element,
        depth: u32,
    ) -> Result<Option<Vec<Coordinate>>, Error> {
        self.spend(element, 1)?;
        let doc = self.doc;
        match self.local(element) {
            b"LineString" | b"LineStringSegment" => Ok(Some(self.coordinates(element))),
            b"Curve" => {
                let mut points = Vec::new();
                for segments in self.named(element, b"segments") {
                    for segment in self.children(segments) {
                        if self.local(segment) != b"LineStringSegment" {
                            self.unsupported(segment);
                            return Ok(None);
                        }
                        join(&mut points, self.coordinates(segment));
                    }
                }
                Ok(Some(points))
            }
            b"OrientableCurve" => {
                let reversed = doc
                    .attribute(element, None, b"orientation")
                    .is_some_and(|value| doc.value(value).trim() == "-");
                let mut points = None;
                for base in self.named(element, b"baseCurve") {
                    for curve in self.members(base, depth) {
                        points = self.curve(curve, depth + 1)?;
                    }
                }
                Ok(points.map(|mut points| {
                    if reversed {
                        points.reverse();
                    }
                    points
                }))
            }
            _ => {
                self.unsupported(element);
                Ok(None)
            }
        }
    }

    /// 性質の子の要素。子がなければ `xlink:href` で引く
    fn members(&mut self, property: &'d Element, depth: u32) -> Vec<&'d Element> {
        let children = self.children(property);
        if !children.is_empty() {
            return children;
        }
        self.reference(property, depth).into_iter().collect()
    }

    fn reference(&mut self, element: &Element, depth: u32) -> Option<&'d Element> {
        let doc = self.doc;
        let href = doc.value(doc.attribute(element, Some(XLINK), b"href")?);
        let target = href
            .strip_prefix('#')
            .and_then(|id| self.ids.get(id))
            .and_then(|index| doc.element(*index))
            .filter(|_| depth < MAX_DEPTH);
        if target.is_none() {
            let at = self.at(element);
            self.cx.report(
                Diagnostic::new(codes::FGD_REFERENCE_UNSUPPORTED)
                    .with("href", href)
                    .at(at),
            );
        }
        target
    }

    // DEM 区画の被覆

    /// DEM 区画の被覆（2.2.2 節の `CV_DiscreteGridPointCoverage`・`CV_GridValuesMatrix`）を、セルの中心の点を
    /// 結ぶ三角形の集まりにする。DEM 構成点の種別は、種別ごとの点の数を属性にする。ジオイド・モデルと基準面補正
    /// パラメーターの被覆は、標高でないので移さない
    fn coverage(
        &mut self,
        feature: &Element,
        property: &'d Element,
        attributes: &mut Vec<Attribute>,
    ) -> Result<Option<Form>, Error> {
        self.spend(property, 1)?;
        if self.local(feature) != b"DEM" {
            self.unsupported(property);
            return Ok(None);
        }
        let Some(grid) = self.grid(property) else {
            return Ok(None);
        };
        let Grid {
            south,
            west,
            north,
            east,
            columns,
            rows,
            start,
            values,
            ordered,
        } = grid;
        if !ordered {
            self.unsupported(property);
            return Ok(None);
        }
        let Some(cells) = columns.checked_mul(rows) else {
            self.invalid(property, "limits");
            return Ok(None);
        };
        if let Err(stop) = self.cx.check(Kind::Segments, cells as u64) {
            let at = self.at(property);
            self.cx.report(stop.to_diagnostic().at(at));
            return Err(Error::Stopped(stop));
        }
        let first = start.1.saturating_mul(columns).saturating_add(start.0);
        if first.saturating_add(values.len()) > cells {
            self.invalid(property, "tupleList");
        }
        let step_lat = (north - south) / rows as f64;
        let step_lon = (east - west) / columns as f64;
        let mut slots = vec![None; cells];
        let mut positions: Vec<Xyz> = Vec::new();
        let mut kinds: Vec<(String, i64)> = Vec::new();
        for (offset, (kind, alti)) in values.into_iter().enumerate() {
            let cell = first + offset;
            let Some(slot) = slots.get_mut(cell) else {
                break;
            };
            match kinds.iter_mut().find(|(name, _)| *name == kind) {
                Some((_, count)) => *count += 1,
                None => kinds.push((kind.clone(), 1)),
            }
            if kind == "データなし" || alti <= NO_DATA {
                continue;
            }
            let (m, n) = (cell % columns, cell / columns);
            // 先頭のセルは北西の端。x（経度）の正の向きに並び、東の端で y（緯度）の負の向きに進む（"+x -y"）
            let latitude = north - (n as f64 + 0.5) * step_lat;
            let longitude = west + (m as f64 + 0.5) * step_lon;
            *slot = u32::try_from(positions.len()).ok();
            positions.push(Coordinate([latitude, longitude, alti], true).xyz(self.model.crs));
        }
        let mut triangles = Vec::new();
        let slot = |m: usize, n: usize| slots.get(n * columns + m).copied().flatten();
        for n in 0..rows.saturating_sub(1) {
            for m in 0..columns.saturating_sub(1) {
                let (nw, ne, sw, se) = (
                    slot(m, n),
                    slot(m + 1, n),
                    slot(m, n + 1),
                    slot(m + 1, n + 1),
                );
                if let (Some(nw), Some(ne), Some(sw)) = (nw, ne, sw) {
                    triangles.push([nw, ne, sw]);
                }
                if let (Some(ne), Some(se), Some(sw)) = (ne, se, sw) {
                    triangles.push([ne, se, sw]);
                }
            }
        }
        let surfaces = kinds
            .iter()
            .filter(|(kind, _)| kind != "データなし")
            .count();
        if surfaces > 1 {
            let at = self.at(property);
            self.cx.report(
                Diagnostic::new(codes::FGD_POINT_KIND_DROPPED)
                    .with("element", self.qualified(feature))
                    .at(at),
            );
        }
        for (kind, count) in kinds {
            attributes.push(Attribute {
                name: format!("fgd:coverage/{kind}"),
                value: Value::Integer(count),
            });
        }
        Ok(Some(Form::Solid(Shape::Triangles {
            positions,
            triangles,
            closed: false,
        })))
    }

    /// 被覆の範囲・格子・値の列。読めなければ診断して `None`
    fn grid(&mut self, coverage: &'d Element) -> Option<Grid> {
        let envelope = self
            .named(coverage, b"boundedBy")
            .into_iter()
            .flat_map(|bounded| self.named(bounded, b"Envelope"))
            .next();
        let Some(envelope) = envelope else {
            self.invalid(coverage, "Envelope");
            return None;
        };
        if !self.same_crs(envelope) {
            return None;
        }
        let corner = |reader: &Self, local: &[u8]| {
            reader
                .named(envelope, local)
                .into_iter()
                .next()
                .map(|corner| numbers(&reader.text(corner)))
        };
        let (Some([south, west]), Some([north, east])) = (
            corner(self, b"lowerCorner").and_then(|v| <[f64; 2]>::try_from(v).ok()),
            corner(self, b"upperCorner").and_then(|v| <[f64; 2]>::try_from(v).ok()),
        ) else {
            self.invalid(envelope, "corner");
            return None;
        };
        if !(south < north && west < east)
            || ![south, west, north, east].iter().all(|v| v.is_finite())
        {
            self.invalid(envelope, "corner");
            return None;
        }
        let limits = self
            .named(coverage, b"gridDomain")
            .into_iter()
            .flat_map(|domain| self.named(domain, b"Grid"))
            .flat_map(|grid| self.named(grid, b"limits"))
            .flat_map(|limits| self.named(limits, b"GridEnvelope"))
            .next();
        let index = |reader: &Self, element: Option<&Element>, local: &[u8]| {
            let element = element?;
            let text = reader.text(reader.named(element, local).into_iter().next()?);
            let values = text
                .split_whitespace()
                .map(|value| value.parse::<usize>().ok())
                .collect::<Option<Vec<_>>>()?;
            <[usize; 2]>::try_from(values).ok()
        };
        let (Some(low), Some(high)) = (index(self, limits, b"low"), index(self, limits, b"high"))
        else {
            self.invalid(coverage, "limits");
            return None;
        };
        let (Some(columns), Some(rows)) = (
            high[0].checked_sub(low[0]).and_then(|d| d.checked_add(1)),
            high[1].checked_sub(low[1]).and_then(|d| d.checked_add(1)),
        ) else {
            self.invalid(coverage, "limits");
            return None;
        };
        let function = self
            .named(coverage, b"coverageFunction")
            .into_iter()
            .flat_map(|function| self.named(function, b"GridFunction"))
            .next();
        let doc = self.doc;
        let ordered = match function {
            // 並びの規則を書いていなければ、GML の既定（"+x +y" の Linear）。基盤地図情報は "+x -y" にする
            None => false,
            Some(function) => self
                .named(function, b"sequenceRule")
                .into_iter()
                .next()
                .is_some_and(|rule| {
                    let order = doc
                        .attribute(rule, None, b"order")
                        .map(|a| doc.value(a).split_whitespace().collect::<String>());
                    self.text(rule).trim() == "Linear" && order.as_deref() == Some("+x-y")
                }),
        };
        let start = function
            .and_then(|function| index(self, Some(function), b"startPoint"))
            .map(|[x, y]| (x.saturating_sub(low[0]), y.saturating_sub(low[1])))
            .unwrap_or((0, 0));
        let tuples = self
            .named(coverage, b"rangeSet")
            .into_iter()
            .flat_map(|range| self.named(range, b"DataBlock"))
            .flat_map(|block| self.named(block, b"tupleList"))
            .next();
        let Some(tuples) = tuples else {
            self.invalid(coverage, "tupleList");
            return None;
        };
        let mut values = Vec::new();
        for tuple in self.text(tuples).split_whitespace() {
            let parsed = tuple
                .split_once(',')
                .and_then(|(kind, alti)| Some((kind.to_string(), real(alti)?)));
            match parsed {
                Some(value) => values.push(value),
                None => {
                    self.invalid(tuples, "tupleList");
                    return None;
                }
            }
        }
        Some(Grid {
            south,
            west,
            north,
            east,
            columns,
            rows,
            start,
            values,
            ordered,
        })
    }

    // 座標

    fn locations(&mut self, element: &Element) -> Vec<Location> {
        let points = self.coordinates(element);
        self.to_locations(points)
    }

    fn to_locations(&self, points: Vec<Coordinate>) -> Vec<Location> {
        points.into_iter().map(|p| self.location(p)).collect()
    }

    fn location(&self, point: Coordinate) -> Location {
        let [a, b] = [point.0[0], point.0[1]];
        let position = match self.model.crs {
            Some(Crs::Geographic) | None => Position::Geo(GeoPoint {
                latitude: a,
                longitude: b,
            }),
            Some(_) => Position::Map(MapPoint {
                northing: a,
                easting: b,
            }),
        };
        Location {
            position,
            height: point.0.get(2).copied().filter(|_| point.1),
        }
    }

    /// 要素の座標（`gml:posList`・`gml:pos`）。点の次元は `srsDimension`、なければ 2（2.2.3 節の DirectPosition）
    fn coordinates(&mut self, element: &Element) -> Vec<Coordinate> {
        let doc = self.doc;
        let mut found = Vec::new();
        for child in self.children(element) {
            let local = self.local(child);
            if !matches!(local, b"posList" | b"pos") {
                continue;
            }
            let dimension = doc
                .attribute(child, None, b"srsDimension")
                .or_else(|| doc.attribute(element, None, b"srsDimension"))
                .and_then(|value| doc.value(value).trim().parse::<usize>().ok())
                .unwrap_or(2);
            let values = numbers(&self.text(child));
            if !(2..=3).contains(&dimension)
                || values.is_empty()
                || !values.len().is_multiple_of(dimension)
                || values.iter().any(|v| !v.is_finite())
            {
                self.invalid(child, "posList");
                continue;
            }
            for chunk in values.chunks(dimension) {
                let mut point = [0.0; 3];
                for (slot, value) in point.iter_mut().zip(chunk) {
                    *slot = *value;
                }
                found.push(Coordinate(point, dimension == 3));
            }
        }
        if let Err(stop) = self.cx.check(Kind::Segments, found.len() as u64) {
            self.cx.report(stop.to_diagnostic());
            return Vec::new();
        }
        found
    }

    // 要素をたどる道具

    fn local(&self, element: &Element) -> &'d [u8] {
        self.doc.local_name(element.name())
    }

    /// 接頭辞をつけた名前。基盤地図情報の名前空間は `fgd`、GML は `gml` にし、知らない名前空間は文書の接頭辞を使う
    fn qualified(&self, element: &Element) -> String {
        let doc = self.doc;
        let local = String::from_utf8_lossy(self.local(element));
        let prefix = match doc.namespace(element.name()).as_deref() {
            Some(NAMESPACE) => Some("fgd".to_string()),
            Some(GML) => Some("gml".to_string()),
            _ => element
                .name()
                .prefix()
                .map(|range| String::from_utf8_lossy(doc.bytes(range)).into_owned()),
        };
        match prefix {
            Some(prefix) => format!("{prefix}:{local}"),
            None => local.into_owned(),
        }
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

    fn at(&self, element: &Element) -> Ref {
        Ref::at_bytes(element.span().start as u64, element.span().end as u64)
    }

    fn unsupported(&mut self, element: &Element) {
        let name = self.qualified(element);
        let at = self.at(element);
        self.cx.report(
            Diagnostic::new(codes::FGD_ELEMENT_UNSUPPORTED)
                .with("element", name)
                .at(at),
        );
    }

    fn invalid(&mut self, element: &Element, field: &'static str) {
        let name = self.qualified(element);
        let at = self.at(element);
        self.cx.report(
            Diagnostic::new(codes::FGD_VALUE_INVALID)
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

/// DEM 区画の被覆の中身
struct Grid {
    /// 範囲（経緯度）
    south: f64,
    west: f64,
    north: f64,
    east: f64,
    /// 格子の列（x、経度）と行（y、緯度）の数
    columns: usize,
    rows: usize,
    /// 値の列が始まるセル（`startPoint`）
    start: (usize, usize),
    /// DEM 構成点の種別と標高値
    values: Vec<(String, f64)>,
    /// 並びの規則が基盤地図情報の "+x -y" の Linear か
    ordered: bool,
}

/// 読んだ座標（`srsName` の軸の順）と、3 次元か
#[derive(Clone, Copy, Debug, PartialEq)]
struct Coordinate([f64; 3], bool);

impl Coordinate {
    /// 3D の形の点。地理の空間なら（緯度・経度・標高）、地図の空間なら（`easting`・`northing`・標高）の順
    fn xyz(self, crs: Option<Crs>) -> Xyz {
        let [a, b, c] = self.0;
        match crs {
            Some(Crs::Geographic) | None => [a, b, c],
            Some(_) => [b, a, c],
        }
    }
}

/// 曲線をつなぐ。前の曲線の終わりと同じ初めの点は除く
fn join(points: &mut Vec<Coordinate>, part: Vec<Coordinate>) {
    let skip = usize::from(points.last().is_some() && points.last() == part.first());
    points.extend(part.into_iter().skip(skip));
}

/// 閉じた輪の終わりの点（初めと同じ点）を除く
fn open(mut ring: Vec<Coordinate>) -> Vec<Coordinate> {
    if ring.len() > 1 && ring.first() == ring.last() {
        ring.pop();
    }
    ring
}

/// 標高を持たない点に標高を入れる
fn with_height(form: &mut Form, alti: f64) {
    let fill = |location: &mut Location| {
        if location.height.is_none() {
            location.height = Some(alti);
        }
    };
    match form {
        Form::Point(location) => fill(location),
        Form::Line(points) => points.iter_mut().for_each(fill),
        Form::Area { outer, holes } => {
            outer.iter_mut().for_each(fill);
            holes.iter_mut().flatten().for_each(fill);
        }
        Form::Solid(_) => {}
    }
}

/// `srsName` の座標系と高さの基準（3.1 節）。基盤地図情報は `fguuid:jgd2024.bl` のような UUID 参照で、JGD2024
/// （2025-07-31 より前のデータは JGD2011）の経緯度を指す。標高は、同じ測地成果の標高とする
fn crs(name: &str) -> (Option<Crs>, Option<VerticalCrs>) {
    match name {
        "fguuid:jgd2024.bl" => (Some(Crs::Geographic), Some(VerticalCrs::Jgd2024)),
        "fguuid:jgd2011.bl" => (Some(Crs::Geographic), Some(VerticalCrs::Jgd2011)),
        _ => (Crs::from_name(name), None),
    }
}

fn numbers(text: &str) -> Vec<f64> {
    text.split_whitespace()
        .map(|value| value.parse::<f64>().unwrap_or(f64::NAN))
        .collect()
}

fn real(text: &str) -> Option<f64> {
    text.trim()
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
}

/// `YYYY-MM-DD` の形か
fn is_date(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.len() == 10
        && bytes.iter().enumerate().all(|(i, b)| {
            if i == 4 || i == 7 {
                *b == b'-'
            } else {
                b.is_ascii_digit()
            }
        })
}
