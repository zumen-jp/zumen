//! CityGML の要素を、地理の領域モデル（決定 0233）に移す。
//!
//! - `core:cityObjectMember` の子を地物にする。地物の中の子の要素で、`gml:id` を持つか LOD の形の性質を持つもの
//!   （建築物の部分・境界の面・交通の領域など）は、子の地物にする。
//! - `lod` と数字で始まる性質は、その LOD の形にする。`gen:` の汎用の属性と、文字だけを持つ性質は属性にする。
//!   子の要素を持つが地物でないもの（i-UR のデータ型など）は、葉の性質を `/` でつないだ名前の属性にする。
//! - 性質 `class` は、地物の分類コードにする。
//! - 形の中の `xlink:href` は、同じ文書の `gml:id` を引く。

use std::collections::BTreeMap;

use zumen_base::{Diagnostic, Kind, Ref, Stop, codes};
use zumen_crs::{Crs, GeoPoint, MapPoint, VerticalCrs};
use zumen_geo::{Attribute, Feature, Form, GeoModel, Geometry, Location, Position, Value};
use zumen_geom3d::{Face, Shape, Transform, Xyz};
use zumen_sdk::{Cx, Error};
use zumen_xml::{Document, Element, Node};

const GML: &str = "http://www.opengis.net/gml";
const XLINK: &str = "http://www.w3.org/1999/xlink";
const GENERICS: &str = "http://www.opengis.net/citygml/generics/2.0";

/// 名前空間ごとの接頭辞。文書の接頭辞によらず、型と属性の名前をこの接頭辞で書く
const PREFIXES: &[(&str, &str)] = &[
    ("http://www.opengis.net/citygml/2.0", "core"),
    ("http://www.opengis.net/citygml/building/2.0", "bldg"),
    ("http://www.opengis.net/citygml/transportation/2.0", "tran"),
    ("http://www.opengis.net/citygml/landuse/2.0", "luse"),
    ("http://www.opengis.net/citygml/relief/2.0", "dem"),
    ("http://www.opengis.net/citygml/bridge/2.0", "brid"),
    ("http://www.opengis.net/citygml/tunnel/2.0", "tun"),
    ("http://www.opengis.net/citygml/cityfurniture/2.0", "frn"),
    ("http://www.opengis.net/citygml/vegetation/2.0", "veg"),
    ("http://www.opengis.net/citygml/waterbody/2.0", "wtr"),
    ("http://www.opengis.net/citygml/cityobjectgroup/2.0", "grp"),
    ("http://www.opengis.net/citygml/generics/2.0", "gen"),
    ("http://www.opengis.net/citygml/appearance/2.0", "app"),
    ("http://www.opengis.net/gml", "gml"),
];

/// `xlink:href` を引く深さの上限。循環した参照で止まらないようにする
const MAX_DEPTH: u32 = 32;

pub(crate) fn lift(doc: &Document, cx: &mut Cx) -> Result<GeoModel, Error> {
    let mut reader = Reader {
        doc,
        cx,
        ids: BTreeMap::new(),
        model: GeoModel::default(),
        dimension: 3,
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
    /// 座標の次元の既定（`srsDimension`）
    dimension: usize,
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
        for member in self.children(root) {
            match self.local(member) {
                b"cityObjectMember" | b"featureMember" => {
                    for child in self.children(member) {
                        self.feature(child, None, order)?;
                        order += 1;
                    }
                }
                b"boundedBy" | b"name" | b"description" => {}
                _ => self.unsupported(member),
            }
        }
        Ok(())
    }

    /// 文書の座標系。初めに `srsName` を持つ要素（ふつうは `gml:Envelope`）の値から決める
    fn crs(&mut self) {
        let doc = self.doc;
        let found = doc.elements().iter().find_map(|element| {
            let name = doc.attribute(element, None, b"srsName")?;
            let dimension = doc
                .attribute(element, None, b"srsDimension")
                .and_then(|value| doc.value(value).trim().parse::<usize>().ok());
            Some((doc.value(name), element, dimension))
        });
        let Some((name, element, dimension)) = found else {
            self.cx
                .report(Diagnostic::new(codes::CITYGML_CRS_UNSUPPORTED).with("name", ""));
            return;
        };
        if let Some(dimension) = dimension.filter(|d| (2..=3).contains(d)) {
            self.dimension = dimension;
        }
        let (crs, height) = crs(name.trim());
        if crs.is_none() {
            let at = self.at(element);
            self.cx.report(
                Diagnostic::new(codes::CITYGML_CRS_UNSUPPORTED)
                    .with("name", name.trim())
                    .at(at),
            );
        }
        self.model.crs = crs;
        self.model.height = height;
    }

    fn feature(
        &mut self,
        element: &'d Element,
        parent: Option<usize>,
        order: usize,
    ) -> Result<(), Error> {
        self.spend(element, 1)?;
        let doc = self.doc;
        let kind = self.qualified(element);
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
            if let Some(lod) = lod(local) {
                for form in self.property_forms(property, local)? {
                    geometries.push(Geometry {
                        lod: Some(lod),
                        form,
                    });
                }
                continue;
            }
            if doc.is(property.name(), Some(GENERICS), local) {
                self.generic(property, "", &mut attributes);
                continue;
            }
            let inner = self.children(property);
            if inner.is_empty() {
                if local == b"class" && code.is_none() {
                    code = Some(self.text(property).trim().to_string());
                } else if let Some(value) = self.value(property) {
                    attributes.push(Attribute {
                        name: self.qualified(property),
                        value,
                    });
                }
                continue;
            }
            let path = self.qualified(property);
            for child in inner {
                if self.is_feature(child) {
                    self.feature(child, Some(index), children)?;
                    children += 1;
                } else {
                    self.data(child, &path, &mut attributes);
                }
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

    /// 子の地物か。`gml:id` を持つか、LOD の形の性質を持つ要素
    fn is_feature(&self, element: &Element) -> bool {
        self.doc.attribute(element, Some(GML), b"id").is_some()
            || self
                .children(element)
                .iter()
                .any(|child| lod(self.local(child)).is_some())
    }

    /// 地物でないデータ型の要素の葉の性質を、`path` に続けた名前の属性にする
    fn data(&mut self, element: &Element, path: &str, attributes: &mut Vec<Attribute>) {
        for property in self.children(element) {
            let name = format!("{path}/{}", self.qualified(property));
            let inner = self.children(property);
            if inner.is_empty() {
                if let Some(value) = self.value(property) {
                    attributes.push(Attribute { name, value });
                }
            } else {
                for child in inner {
                    self.data(child, &name, attributes);
                }
            }
        }
    }

    /// 汎用の属性（`gen:stringAttribute` など）。属性の組（`gen:genericAttributeSet`）は、組の名前を `/` で
    /// つなぐ
    fn generic(&mut self, element: &Element, prefix: &str, attributes: &mut Vec<Attribute>) {
        let doc = self.doc;
        let Some(name) = doc.attribute(element, None, b"name").map(|a| doc.value(a)) else {
            self.invalid(element, "name");
            return;
        };
        let name = format!("{prefix}{name}");
        let local = self.local(element);
        if local == b"genericAttributeSet" {
            let inner = format!("{name}/");
            for child in self.children(element) {
                self.generic(child, &inner, attributes);
            }
            return;
        }
        let Some(holder) = self.named(element, b"value").into_iter().next() else {
            self.invalid(element, "value");
            return;
        };
        let text = self.text(holder).trim().to_string();
        let unit = doc.attribute(holder, None, b"uom").map(|a| doc.value(a));
        let parsed = match local {
            b"stringAttribute" | b"uriAttribute" => Some(Value::Text(text)),
            b"intAttribute" => text.parse().ok().map(Value::Integer),
            b"doubleAttribute" => real(&text).map(Value::Real),
            b"dateAttribute" => Some(Value::Date(text)),
            b"measureAttribute" => real(&text).map(|value| Value::Measure { value, unit }),
            _ => {
                self.unsupported(element);
                return;
            }
        };
        match parsed {
            Some(value) => attributes.push(Attribute {
                name: format!("gen:{name}"),
                value,
            }),
            None => self.invalid(element, "value"),
        }
    }

    /// 文字だけを持つ性質の値。`codeSpace` があればコード、`uom` があれば単位つきの量にする。どちらもなければ、
    /// 文字の形から真偽・日付・整数・実数を決め、どれでもなければ文字にする
    fn value(&mut self, element: &Element) -> Option<Value> {
        let doc = self.doc;
        let text = self.text(element).trim().to_string();
        if text.is_empty() {
            return doc
                .attribute(element, Some(XLINK), b"href")
                .map(|href| Value::Text(doc.value(href)));
        }
        if let Some(space) = doc.attribute(element, None, b"codeSpace") {
            return Some(Value::Code {
                space: Some(doc.value(space)),
                value: text,
            });
        }
        if let Some(unit) = doc.attribute(element, None, b"uom") {
            let Some(value) = real(&text) else {
                self.invalid(element, "text");
                return None;
            };
            return Some(Value::Measure {
                value,
                unit: Some(doc.value(unit)),
            });
        }
        Some(typed(text))
    }

    // 形

    /// LOD の形の性質の中身。中身がなく `xlink:href` だけなら、引いた先の形にする
    fn property_forms(&mut self, property: &'d Element, local: &[u8]) -> Result<Vec<Form>, Error> {
        if self.model.crs.is_none() {
            return Ok(Vec::new());
        }
        if local.ends_with(b"ImplicitRepresentation") {
            return self.implicit(property);
        }
        let mut forms = Vec::new();
        for geometry in self.members(property, 0) {
            forms.extend(self.forms(geometry, 0)?);
        }
        Ok(forms)
    }

    /// 性質の子の形の要素。子がなければ `xlink:href` で引く
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
                Diagnostic::new(codes::CITYGML_REFERENCE_UNSUPPORTED)
                    .with("href", href)
                    .at(at),
            );
        }
        target
    }

    /// GML の形の要素を、形にする
    fn forms(&mut self, element: &'d Element, depth: u32) -> Result<Vec<Form>, Error> {
        self.spend(element, 1)?;
        let local = self.local(element);
        if !self.doc.is(element.name(), Some(GML), local) {
            self.unsupported(element);
            return Ok(Vec::new());
        }
        Ok(match local {
            b"Point" => self
                .locations(element)
                .into_iter()
                .next()
                .map(Form::Point)
                .into_iter()
                .collect(),
            b"MultiPoint" => {
                let mut forms = Vec::new();
                for member in self.children(element) {
                    for point in self.members(member, depth) {
                        forms.extend(self.forms(point, depth + 1)?);
                    }
                }
                forms
            }
            b"LineString" => vec![Form::Line(self.locations(element))],
            b"MultiCurve" | b"CompositeCurve" => {
                let mut forms = Vec::new();
                for member in self.children(element) {
                    for curve in self.members(member, depth) {
                        forms.extend(self.forms(curve, depth + 1)?);
                    }
                }
                forms
            }
            b"Polygon" => {
                let (outer, holes) = self.rings(element);
                vec![Form::Area {
                    outer: outer.into_iter().map(|p| self.location(p)).collect(),
                    holes: holes
                        .into_iter()
                        .map(|ring| ring.into_iter().map(|p| self.location(p)).collect())
                        .collect(),
                }]
            }
            b"MultiSolid" | b"CompositeSolid" => {
                let mut forms = Vec::new();
                for member in self.children(element) {
                    for solid in self.members(member, depth) {
                        forms.extend(self.forms(solid, depth + 1)?);
                    }
                }
                forms
            }
            b"Solid"
            | b"MultiSurface"
            | b"CompositeSurface"
            | b"Shell"
            | b"TriangulatedSurface"
            | b"Tin"
            | b"OrientableSurface"
            | b"Surface" => {
                let mut faces = Vec::new();
                self.faces(element, false, depth, &mut faces)?;
                vec![Form::Solid(Shape::FaceSet {
                    faces,
                    closed: local == b"Solid",
                })]
            }
            _ => {
                self.unsupported(element);
                Vec::new()
            }
        })
    }

    /// 面の集まりの面。`reversed` なら輪を逆に回す（`gml:OrientableSurface` の `orientation="-"`）
    fn faces(
        &mut self,
        element: &'d Element,
        reversed: bool,
        depth: u32,
        faces: &mut Vec<Face>,
    ) -> Result<(), Error> {
        self.spend(element, 1)?;
        let doc = self.doc;
        let local = self.local(element);
        match local {
            b"Polygon" | b"Triangle" => {
                let (outer, holes) = self.rings(element);
                let point = |p: Coordinate| p.xyz(self.model.crs);
                let mut face = Face {
                    outer: outer.into_iter().map(point).collect(),
                    holes: holes
                        .into_iter()
                        .map(|ring| ring.into_iter().map(point).collect())
                        .collect(),
                };
                if reversed {
                    face.outer.reverse();
                    face.holes.iter_mut().for_each(|ring| ring.reverse());
                }
                if face.outer.len() >= 3 {
                    faces.push(face);
                } else {
                    self.invalid(element, "posList");
                }
            }
            b"OrientableSurface" => {
                let flip = doc
                    .attribute(element, None, b"orientation")
                    .is_some_and(|value| doc.value(value).trim() == "-");
                for base in self.named(element, b"baseSurface") {
                    for surface in self.members(base, depth) {
                        self.faces(surface, reversed != flip, depth + 1, faces)?;
                    }
                }
            }
            b"Solid" => {
                for exterior in self.named(element, b"exterior") {
                    for shell in self.members(exterior, depth) {
                        self.faces(shell, reversed, depth + 1, faces)?;
                    }
                }
                // 内側の殻（空洞）は、面の集まりで表せないので読まない
                if let Some(interior) = self.named(element, b"interior").into_iter().next() {
                    self.unsupported(interior);
                }
            }
            b"MultiSurface"
            | b"CompositeSurface"
            | b"Shell"
            | b"Surface"
            | b"TriangulatedSurface"
            | b"Tin" => {
                for member in self.children(element) {
                    let local = self.local(member);
                    let patches = matches!(local, b"patches" | b"trianglePatches");
                    if !patches && !matches!(local, b"surfaceMember" | b"surfaceMembers") {
                        continue;
                    }
                    let surfaces = if patches || local == b"surfaceMembers" {
                        self.children(member)
                    } else {
                        self.members(member, depth)
                    };
                    for surface in surfaces {
                        self.faces(surface, reversed, depth + 1, faces)?;
                    }
                }
            }
            b"PolygonPatch" => {
                let (outer, holes) = self.rings(element);
                let point = |p: Coordinate| p.xyz(self.model.crs);
                faces.push(Face {
                    outer: outer.into_iter().map(point).collect(),
                    holes: holes
                        .into_iter()
                        .map(|ring| ring.into_iter().map(point).collect())
                        .collect(),
                });
            }
            _ => self.unsupported(element),
        }
        Ok(())
    }

    /// 暗黙の形（`core:ImplicitGeometry`）。相対の形を、変換の行列と基準点で置く。行列の平行移動に基準点の
    /// 座標を足し、座標系の単位は直さない
    fn implicit(&mut self, property: &'d Element) -> Result<Vec<Form>, Error> {
        let mut forms = Vec::new();
        for geometry in self.children(property) {
            let matrix = self
                .named(geometry, b"transformationMatrix")
                .into_iter()
                .next()
                .map(|element| numbers(&self.text(element)));
            let mut m = match matrix.as_deref() {
                None => Transform::IDENTITY.0,
                Some([a, b, c, d, e, f, g, h, i, j, k, l, ..]) => {
                    [[*a, *b, *c, *d], [*e, *f, *g, *h], [*i, *j, *k, *l]]
                }
                Some(_) => {
                    self.invalid(geometry, "transformationMatrix");
                    continue;
                }
            };
            let points: Vec<&Element> = self
                .named(geometry, b"referencePoint")
                .into_iter()
                .flat_map(|point| self.children(point))
                .collect();
            let reference = points
                .into_iter()
                .find_map(|point| self.coordinates(point).into_iter().next());
            let Some(reference) = reference else {
                self.invalid(geometry, "referencePoint");
                continue;
            };
            let [x, y, z] = reference.xyz(self.model.crs);
            for (row, offset) in m.iter_mut().zip([x, y, z]) {
                row[3] += offset;
            }
            for relative in self.named(geometry, b"relativeGMLGeometry") {
                for shape in self.members(relative, 0) {
                    for form in self.forms(shape, 0)? {
                        let Form::Solid(shape) = form else {
                            self.unsupported(relative);
                            continue;
                        };
                        forms.push(Form::Solid(Shape::Mapped {
                            transform: Transform(m),
                            shape: Box::new(shape),
                        }));
                    }
                }
            }
        }
        Ok(forms)
    }

    /// 面の外周と穴の輪の座標。閉じる点（初めと同じ終わりの点）は除く
    fn rings(&mut self, element: &'d Element) -> (Vec<Coordinate>, Vec<Vec<Coordinate>>) {
        let mut outer = Vec::new();
        let mut holes = Vec::new();
        for boundary in self.children(element) {
            let ring = self
                .members(boundary, 0)
                .into_iter()
                .next()
                .map(|ring| open(self.coordinates(ring)))
                .unwrap_or_default();
            match self.local(boundary) {
                b"exterior" | b"outerBoundaryIs" => outer = ring,
                b"interior" | b"innerBoundaryIs" => holes.push(ring),
                _ => {}
            }
        }
        (outer, holes)
    }

    fn locations(&mut self, element: &Element) -> Vec<Location> {
        self.coordinates(element)
            .into_iter()
            .map(|p| self.location(p))
            .collect()
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

    /// 要素の座標（`gml:posList`・`gml:pos`）
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
                .unwrap_or(self.dimension);
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

    /// 接頭辞をつけた名前。名前空間の接頭辞は `PREFIXES` から決め、知らない名前空間は文書の接頭辞を使う
    fn qualified(&self, element: &Element) -> String {
        let doc = self.doc;
        let local = String::from_utf8_lossy(self.local(element));
        let known = doc.namespace(element.name()).and_then(|namespace| {
            PREFIXES
                .iter()
                .find(|(uri, _)| *uri == namespace)
                .map(|(_, prefix)| *prefix)
        });
        let prefix = match known {
            Some(prefix) => Some(prefix.to_string()),
            None => element
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
            Diagnostic::new(codes::CITYGML_ELEMENT_UNSUPPORTED)
                .with("element", name)
                .at(at),
        );
    }

    fn invalid(&mut self, element: &Element, field: &'static str) {
        let name = self.qualified(element);
        let at = self.at(element);
        self.cx.report(
            Diagnostic::new(codes::CITYGML_VALUE_INVALID)
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

/// 閉じた輪の終わりの点（初めと同じ点）を除く
fn open(mut ring: Vec<Coordinate>) -> Vec<Coordinate> {
    if ring.len() > 1 && ring.first() == ring.last() {
        ring.pop();
    }
    ring
}

/// `srsName` の座標系と高さの基準。PLATEAU の EPSG:6697 は、JGD2011 の経緯度と JGD2011 の標高の複合の座標系
fn crs(name: &str) -> (Option<Crs>, Option<VerticalCrs>) {
    let code = [
        "EPSG:",
        "urn:ogc:def:crs:EPSG::",
        "http://www.opengis.net/def/crs/EPSG/0/",
        "https://www.opengis.net/def/crs/EPSG/0/",
    ]
    .iter()
    .find_map(|prefix| name.strip_prefix(prefix))
    .and_then(|code| code.parse::<u32>().ok());
    match code {
        Some(6697) => (Some(Crs::Geographic), Some(VerticalCrs::Jgd2011)),
        _ => (Crs::from_name(name), None),
    }
}

/// `lod` と数字で始まる名前の LOD
fn lod(local: &[u8]) -> Option<u8> {
    match local {
        [b'l', b'o', b'd', digit @ b'0'..=b'4', ..] => Some(digit - b'0'),
        _ => None,
    }
}

fn numbers(text: &str) -> Vec<f64> {
    text.split_whitespace()
        .map(|value| value.parse::<f64>().unwrap_or(f64::NAN))
        .collect()
}

fn real(text: &str) -> Option<f64> {
    text.parse::<f64>().ok().filter(|value| value.is_finite())
}

/// 文字の形から値の型を決める。先頭が 0 の数字の並び（コードや番号）は文字のままにする
fn typed(text: String) -> Value {
    match text.as_str() {
        "true" => return Value::Bool(true),
        "false" => return Value::Bool(false),
        _ => {}
    }
    let bytes = text.as_bytes();
    let digits = |range: &[u8]| !range.is_empty() && range.iter().all(u8::is_ascii_digit);
    if let [y0, y1, y2, y3, b'-', m0, m1, b'-', d0, d1] = bytes
        && digits(&[*y0, *y1, *y2, *y3, *m0, *m1, *d0, *d1])
    {
        return Value::Date(text);
    }
    let unsigned = bytes.strip_prefix(b"-").unwrap_or(bytes);
    let leading_zero = unsigned.len() > 1 && unsigned.first() == Some(&b'0');
    if digits(unsigned)
        && !leading_zero
        && let Ok(value) = text.parse::<i64>()
    {
        return Value::Integer(value);
    }
    let decimal = unsigned.split(|b| *b == b'.').collect::<Vec<_>>();
    if let [whole, fraction] = decimal.as_slice()
        && digits(whole)
        && digits(fraction)
        && (whole.len() == 1 || whole.first() != Some(&b'0'))
        && let Some(value) = real(&text)
    {
        return Value::Real(value);
    }
    Value::Text(text)
}
