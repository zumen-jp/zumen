//! 建物モデルから新しい IFC を書く（#395）。
//!
//! バージョンごとのひな形（[`super::create`]）に、[`super::apply`] と同じ書き手の変更で、空間・要素・所属・配置・
//! 開口と建具の関係を足す。元の L1 のレコードは写さない。書き手がまだ書けない要素と値（`IfcSpace`・型・
//! プロパティと数量のセット・形など）は書かず、変換の忠実度と診断で返す。書いた要素のうち値を落としたものは
//! 近似として、落とした値のキーごとに数える。
//!
//! - GlobalId は、元の ID が GlobalId ならその値、ほかは名前空間と元の ID から [`super::derive_global_id`] で
//!   導く。名前空間は、呼び出し元の値、なければ元の `IfcProject` の GlobalId、それもなければ元の文書のハッシュ
//! - 配置は、親の配置を持つ要素を書いたならその要素からの局所の変換で書く。書かなかった要素からの配置は、
//!   書いた祖先まで変換を畳む。解けない配置と、正規直交でない変換は書かない
//! - 階の高さは、IFC2X3 と IFC4 では 10 Elevation に書く。元に値がなければ、建物モデルの階の高さを書く。IFC4.3
//!   では配置だけが高さを持つので、元の値が親からの配置の z と同じでなければ落とす
//! - 書き手が足す関係とセットの GlobalId は、名前空間と、種類・結ぶ要素の GlobalId・セットの名前から導く

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use zumen_base::{Diagnostic, Kind, codes};
use zumen_building::{
    Acquired, AreaProfile, AreaProfileKind, AxisPlacement2, AxisPlacement3, Building,
    ConnectedFaces, Coordinates, CurveGraph, CurveKind, CurveRef, ElementRef, EnclosureCoordinates,
    FaceBoundRef, FaceBoundary, FaceGraph, FaceLoopRef, FaceModelKind, FacePointRef, FacePolygon,
    FaceRef, FaceSetRef, GeometryData, GeometryDefinition, GeometryLinks, GeometryNode,
    GeometryRef, GlobalId, HalfSpaceLimit, Id, IndexedFaceSet, IndexedFaces, Literal, Local,
    Material, MaterialKind, Measured, Opening, ParentKind, PolygonFace, Property, Quantity,
    SpatialKind, Unit,
};
use zumen_sdk::conversion::{ConversionFidelity, ConversionReport, Report, note};
use zumen_sdk::{Codec, Cx, Error};

use super::author::{
    NewAxis2, NewAxis3, NewBoolean, NewConstituent, NewFaces, NewHalfSpace, NewHalfSpaceLimit,
    NewItem, NewLayerUsage, NewMapped, NewMaterial, NewMaterialLayer, NewProfile, NewProfileKind,
    NewRevolution, NewShape, NewSolid, NewSweptDisk, NewType, SHAPE_NESTING, enumeration,
    material_rejected, predefined_at, rigid, shape_rejected, writable_type,
};
use super::create::{Header, LengthUnit, OwnerHistory, Template, create, derive_global_id};
use super::quantity::{NewQuantity, NewUnit, quantity_value, si_unit};
use super::{
    Edit, NewElement, NewLiteral, NewPlacement, NewSpace, NewValue, SpaceKind, Target, apply,
};
use crate::building::{Version, arg};
use crate::{Ifc, IfcFile, Value, stopped};

/// 変換の設定
#[derive(Clone, Debug, PartialEq)]
pub struct ConvertOptions {
    /// 書くバージョン
    pub version: Version,
    /// ヘッダーの値
    pub header: Header,
    /// 所有履歴の値。IFC2X3 では必須
    pub owner: Option<OwnerHistory>,
    /// GlobalId を導く名前空間。なければ元の `IfcProject` の GlobalId か、元の文書のハッシュ
    pub namespace: Option<Vec<u8>>,
    /// 忠実度の明細の最大件数
    pub limit: u64,
}

/// 変換の結果
#[derive(Clone, Debug, PartialEq)]
pub struct Converted {
    /// 書いた IFC
    pub bytes: Vec<u8>,
    /// 種類と忠実度ごとの件数と明細
    pub report: ConversionReport,
}

/// 建物モデルを新しい IFC に書く。`document` は元の文書のハッシュのバイト列
pub fn convert(
    building: &Building,
    document: &[u8],
    options: &ConvertOptions,
    cx: &mut Cx,
) -> Result<Converted, Error> {
    let format = options.version.file_schema();
    let Some(unit) = length_unit(building) else {
        cx.report(
            Diagnostic::new(codes::CONVERT_ELEMENT_UNSUPPORTED)
                .with("format", format)
                .with("kind", "LENGTHUNIT"),
        );
        return Err(Error::Invalid);
    };
    let project = building.document.project;
    let project_global = project
        .and_then(|p| building.id(p))
        .and_then(|id| match id {
            Id::Global(global) => Some(global),
            Id::Instance(_) => None,
        });
    let namespace = match (&options.namespace, project_global) {
        (Some(namespace), _) => namespace.clone(),
        (None, Some(global)) => global.to_string().into_bytes(),
        (None, None) => document.to_vec(),
    };
    let template = Template {
        version: options.version,
        unit,
        project: project_global.unwrap_or_else(|| derive_global_id(&namespace, b"project")),
        name: building.document.project_name.clone(),
        header: options.header.clone(),
        owner: options.owner.clone(),
    };
    let bytes = create(&template, cx)?;
    let file = Ifc::read(&bytes, cx)?;

    let mut plan = Plan {
        building,
        version: options.version,
        format,
        namespace: &namespace,
        written: BTreeMap::new(),
        notes: BTreeMap::new(),
        edits: Vec::new(),
        added: 0,
        units: Vec::new(),
        materials: Vec::new(),
        used_materials: BTreeSet::new(),
        report: Report::new(options.limit),
    };
    if let Some(project) = project {
        plan.project(project);
    }
    for element in order(building, project) {
        cx.check(Kind::Work, 1)
            .map_err(|stop| stopped(cx, stop, None))?;
        plan.element(element, cx)?;
    }
    plan.openings();
    plan.material_elements(cx)?;
    let edits = std::mem::take(&mut plan.edits);
    let mut bytes = apply(&file, &edits, &namespace, cx)?.bytes;
    // プロパティの変更は、文書にあるインスタンスの番号で指すので、書いた後に GlobalId で引いて当てる
    let wanted = plan.properties();
    if !wanted.is_empty() {
        let file = Ifc::read(&bytes, cx)?;
        let instances = instances(&file, cx)?;
        let edits = plan.property_edits(&wanted, &instances);
        if !edits.is_empty() {
            let mut seed = namespace.clone();
            seed.extend_from_slice(b"\0properties");
            bytes = apply(&file, &edits, &seed, cx)?.bytes;
        }
    }
    Ok(Converted {
        bytes,
        report: plan.finish(cx)?,
    })
}

/// 要素（`IfcRoot`）の GlobalId から、インスタンスの番号
fn instances(file: &IfcFile, cx: &mut Cx) -> Result<BTreeMap<String, u64>, Error> {
    let mut found = BTreeMap::new();
    for id in file.ids() {
        cx.check(Kind::Work, 1)
            .map_err(|stop| stopped(cx, stop, None))?;
        let Some(Value::Text { raw, .. }) = file.instance(id).and_then(|r| arg(r, 1)) else {
            continue;
        };
        let text = file.bytes(raw.clone());
        if text.len() == 22
            && let Ok(text) = core::str::from_utf8(text)
        {
            found.entry(text.to_owned()).or_insert(id);
        }
    }
    Ok(found)
}

/// 書けるプロパティの値。値の型の名前があり、単一の値のもの
fn property_value(property: &Property) -> Option<NewValue> {
    let value = property.value.as_ref()?;
    let literal = match &value.value {
        Literal::Real(real) => NewLiteral::Real(*real),
        Literal::Integer(integer) => NewLiteral::Integer(*integer),
        Literal::Text(text) => NewLiteral::Text(text.clone()),
        Literal::Boolean(boolean) => NewLiteral::Boolean(*boolean),
        Literal::Logical(logical) => NewLiteral::Logical(*logical),
        Literal::Enumeration(_) | Literal::Element(_) | Literal::List(_) => return None,
    };
    Some(NewValue {
        type_name: value.type_name.clone()?,
        literal,
    })
}

/// 元の文書の長さの単位。ひな形に書けなければ `None`
fn length_unit(building: &Building) -> Option<LengthUnit> {
    let unit = building
        .document
        .units
        .iter()
        .find(|unit| unit.kind.eq_ignore_ascii_case("LENGTHUNIT"))?;
    if unit.scale.is_none() && unit.name.eq_ignore_ascii_case("METRE") {
        return match unit.prefix.as_deref() {
            None => Some(LengthUnit::Meter),
            Some(p) if p.eq_ignore_ascii_case("MILLI") => Some(LengthUnit::Millimeter),
            Some(p) if p.eq_ignore_ascii_case("CENTI") => Some(LengthUnit::Centimeter),
            Some(_) => None,
        };
    }
    let near = |a: f64, b: f64| (a - b).abs() <= 1e-12;
    match unit.scale {
        Some(s) if near(s, 0.0254) && unit.name.eq_ignore_ascii_case("inch") => {
            Some(LengthUnit::Inch)
        }
        Some(s) if near(s, 0.3048) && unit.name.eq_ignore_ascii_case("foot") => {
            Some(LengthUnit::Foot)
        }
        _ => None,
    }
}

/// 書く順。所属の先と、親の配置を持つ要素を先にする。輪になった残りは、元の順で後に置く
fn order(building: &Building, project: Option<ElementRef>) -> Vec<ElementRef> {
    let count = building.elements().len();
    let mut waiting: BTreeMap<ElementRef, usize> = BTreeMap::new();
    let mut dependents: BTreeMap<ElementRef, Vec<ElementRef>> = BTreeMap::new();
    for element in (0..count).filter_map(|n| u32::try_from(n).ok().map(ElementRef)) {
        if Some(element) == project {
            continue;
        }
        let mut needs = Vec::new();
        if let Some(of) = building
            .components
            .parent
            .get(element)
            .and_then(|p| p.of.as_ref())
        {
            needs.push(of.target);
        }
        if let Some(to) = building
            .components
            .placement
            .get(element)
            .and_then(|p| p.relative_to)
        {
            needs.push(to);
        }
        // 型は、型を使う要素より先に書く
        if let Some(of) = building.components.r#type.get(element) {
            needs.push(of.target);
        }
        needs.retain(|&n| n != element && Some(n) != project && (n.0 as usize) < count);
        needs.sort_unstable();
        needs.dedup();
        waiting.insert(element, needs.len());
        for need in needs {
            dependents.entry(need).or_default().push(element);
        }
    }
    let mut ready: VecDeque<ElementRef> = waiting
        .iter()
        .filter(|&(_, &n)| n == 0)
        .map(|(&e, _)| e)
        .collect();
    let mut out = Vec::with_capacity(waiting.len());
    let mut done = BTreeSet::new();
    loop {
        while let Some(element) = ready.pop_front() {
            if !done.insert(element) {
                continue;
            }
            out.push(element);
            for next in dependents.get(&element).into_iter().flatten() {
                if let Some(n) = waiting.get_mut(next) {
                    *n = n.saturating_sub(1);
                    if *n == 0 {
                        ready.push_back(*next);
                    }
                }
            }
        }
        // 輪になったもの。元の順で最初の 1 つを出して続ける
        match waiting.keys().find(|e| !done.contains(*e)) {
            Some(&next) => ready.push_back(next),
            None => break,
        }
    }
    out
}

struct Plan<'b, 'n> {
    building: &'b Building,
    version: Version,
    format: &'static str,
    namespace: &'n [u8],
    /// 書いた要素と、変更の中での指し方
    written: BTreeMap<ElementRef, Target>,
    /// 書いた要素の種類と、落とした値
    notes: BTreeMap<ElementRef, (&'static str, Vec<&'static str>)>,
    edits: Vec<Edit>,
    /// 足す変更が足した要素の数
    added: usize,
    /// 書いた文書の単位（元の値）。長さの単位と、足した SI の単位
    units: Vec<&'b Unit>,
    /// 材料（`IfcMaterial`）の要素。結んだ構成から使われたものだけを書く
    materials: Vec<ElementRef>,
    /// 書いた構成が使う材料
    used_materials: BTreeSet<ElementRef>,
    report: Report,
}

impl Plan<'_, '_> {
    /// 要素ごと落とす
    fn drop_element(&mut self, element: ElementRef, kind: &str, cx: &mut Cx) -> Result<(), Error> {
        let at = self.building.place(element);
        self.report
            .record(kind, ConversionFidelity::Dropped, &at, 0, cx)?;
        cx.report(
            Diagnostic::new(codes::CONVERT_ELEMENT_UNSUPPORTED)
                .with("format", self.format)
                .with("kind", kind.to_owned())
                .at(at),
        );
        Ok(())
    }

    /// 足した要素を覚える
    fn wrote(&mut self, element: ElementRef, kind: &'static str, dropped: Vec<&'static str>) {
        self.written.insert(element, Target::Added(self.added));
        self.added += 1;
        self.notes.insert(element, (kind, dropped));
    }

    /// 書いた要素の、落とした値を足す
    fn lost(&mut self, element: ElementRef, value: &'static str) {
        if let Some((_, dropped)) = self.notes.get_mut(&element)
            && !dropped.contains(&value)
        {
            dropped.push(value);
        }
    }

    /// 書いた要素ごとに忠実度を記録する。値を落としたものは近似として、落とした値ごとに診断する
    fn finish(self, cx: &mut Cx) -> Result<ConversionReport, Error> {
        let Plan {
            building,
            format,
            notes,
            mut report,
            ..
        } = self;
        for (element, (kind, mut dropped)) in notes {
            dropped.sort_unstable();
            let fidelity = if dropped.is_empty() {
                ConversionFidelity::Native
            } else {
                ConversionFidelity::Approximated
            };
            let at = building.place(element);
            note(&mut report, format, kind, fidelity, &at, 1, &dropped, cx)?;
        }
        report.finish(cx)
    }

    /// プロジェクト。文書の値のうち、ひな形に書かないものを落とす
    fn project(&mut self, project: ElementRef) {
        self.written.insert(project, Target::Instance(1));
        let document = &self.building.document;
        let mut dropped = Vec::new();
        // 長さとラジアンはひな形が持つ。ほかの SI の単位は種類ごとに 1 つ足し、換算する単位などは落とす
        let mut lost_units = false;
        for unit in &document.units {
            if unit.kind.eq_ignore_ascii_case("LENGTHUNIT") {
                if !self
                    .units
                    .iter()
                    .any(|u| u.kind.eq_ignore_ascii_case("LENGTHUNIT"))
                {
                    self.units.push(unit);
                }
                continue;
            }
            if unit.kind.eq_ignore_ascii_case("PLANEANGLEUNIT")
                && unit.scale.is_none()
                && unit.prefix.is_none()
                && unit.name.eq_ignore_ascii_case("RADIAN")
            {
                continue;
            }
            let new = NewUnit {
                kind: unit.kind.clone(),
                prefix: unit.prefix.clone(),
                name: unit.name.clone(),
            };
            let taken = self
                .units
                .iter()
                .any(|u| u.kind.eq_ignore_ascii_case(&unit.kind));
            if unit.scale.is_none() && si_unit(&new).is_some() && !taken {
                self.units.push(unit);
                self.edits.push(Edit::Unit(new));
            } else {
                lost_units = true;
            }
        }
        if lost_units {
            dropped.push("units");
        }
        if document.phase.is_some() {
            dropped.push("phase");
        }
        if document.true_north.is_some() {
            dropped.push("true_north");
        }
        if document.map.is_some() {
            dropped.push("map");
        }
        dropped.extend(self.components(project));
        self.notes.insert(project, ("IfcProject", dropped));
    }

    fn element(&mut self, element: ElementRef, cx: &mut Cx) -> Result<(), Error> {
        let building = self.building;
        let Some(class) = building.components.class.get(element) else {
            return self.drop_element(element, "element", cx);
        };
        // 材料は要素として書かず、結んだ構成の中に書く
        if class.name == "IfcMaterial" {
            self.materials.push(element);
            return Ok(());
        }
        match building.components.spatial.get(element).map(|s| &s.kind) {
            Some(SpatialKind::Site) => self.space(element, SpaceKind::Site, cx),
            Some(SpatialKind::Building) => self.space(element, SpaceKind::Building, cx),
            Some(SpatialKind::Storey) => self.space(element, SpaceKind::Storey, cx),
            Some(_) => self.drop_element(element, &class.name, cx),
            None => match writable_type(&class.name, self.version) {
                Some(kind) => {
                    self.element_type(element, kind);
                    Ok(())
                }
                None => self.product(element, &class.name, cx),
            },
        }
    }

    /// 型（`IfcWallType` など）を書く。PredefinedType は必須なので、元になければ `NOTDEFINED` にする
    fn element_type(&mut self, element: ElementRef, kind: &'static str) {
        let source = self.building.components.class.get(element);
        let mut dropped = Vec::new();
        let predefined = match source.and_then(|c| c.predefined.clone()) {
            Some(value) if enumeration(&value).is_some() => Some(value),
            Some(_) => {
                dropped.push("predefined");
                None
            }
            None => None,
        };
        dropped.extend(self.components(element));
        // 建具の型の OperationType・PartitioningType は必須なので、書き手が NOTDEFINED を書く
        let defaults = self
            .building
            .components
            .extension
            .get(element)
            .is_some_and(|extension| {
                extension.values.iter().all(|v| {
                    v.namespace == "ifc.attribute"
                        && matches!(v.name.as_str(), "OperationType" | "PartitioningType")
                        && matches!(&v.value.value, Literal::Enumeration(e)
                            if e.eq_ignore_ascii_case("NOTDEFINED"))
                })
            });
        if defaults {
            dropped.retain(|&value| value != "extension");
        }
        self.edits.push(Edit::ElementType(NewType {
            class: kind.to_owned(),
            predefined,
            element_type: source.and_then(|c| c.user_defined.clone()),
            name: self.name(element),
            global_id: self.global_id(element),
        }));
        self.wrote(element, kind, dropped);
        self.material(element);
    }

    /// 値の単位が、書いた文書の単位か
    fn unit_written(&self, value: &Measured) -> bool {
        match &value.unit {
            None => true,
            Some(unit) => self.units.contains(&unit),
        }
    }

    /// 書いた要素に形を置く。書けないか近似した形は、落とした値に足す
    fn shape(&mut self, element: ElementRef) {
        let Some(geometry) = self.building.geometry(element) else {
            return;
        };
        let Some(target) = self.target(element).filter(|_| self.shaped(element)) else {
            return;
        };
        let Some((shape, exact)) = self.new_shape(geometry.data()) else {
            self.lost(element, "representation");
            return;
        };
        self.edits.push(Edit::Shape {
            element: target,
            shape,
        });
        if exact {
            if let Some((_, dropped)) = self.notes.get_mut(&element) {
                dropped.retain(|&value| value != "representation");
            }
        } else {
            self.lost(element, "representation");
        }
    }

    /// 要素が形の表現を持つか。読んだ形に根がなければ、表現はない
    fn shaped(&self, element: ElementRef) -> bool {
        self.building
            .geometry(element)
            .is_some_and(|geometry| geometry.data().root.is_some())
    }

    /// 書ける形と、そのまま書けたか。`Axis` の折れ線と、`Body` の押し出しだけを書く。`Body` は項目が
    /// すべて書けるときだけ書き、ほかの表現・表示は落とす
    fn new_shape(&self, data: &GeometryData) -> Option<(NewShape, bool)> {
        let node = |at: GeometryRef| data.nodes.get(at.0 as usize);
        let GeometryLinks::Definition(representations) = &node(data.root?)?.links else {
            return None;
        };
        let mut exact = data.grid.is_none()
            && data.styles.iter().all(|style| {
                matches!(&style.presentation, Acquired::Value(p)
                    if p.direct.is_empty() && p.layers.is_empty())
            });
        let mut shape = NewShape {
            axis: None,
            body: Vec::new(),
        };
        for &representation in representations {
            let Some(GeometryLinks::Representation {
                identifier, items, ..
            }) = node(representation).map(|n| &n.links)
            else {
                exact = false;
                continue;
            };
            let identifier = identifier.as_deref().unwrap_or_default();
            if identifier.eq_ignore_ascii_case("Axis") && shape.axis.is_none() {
                let axis = match items.as_slice() {
                    [item] => node(*item).and_then(|n| self.axis_line(n)),
                    _ => None,
                };
                exact &= axis.is_some();
                shape.axis = axis;
            } else if identifier.eq_ignore_ascii_case("Body") && shape.body.is_empty() {
                let mut budget = BODY_ITEMS;
                let body = items
                    .iter()
                    .map(|&item| self.new_item(data, item, 0, &mut budget))
                    .collect::<Option<Vec<_>>>();
                match body {
                    Some(body) => {
                        for (item, same) in body {
                            exact &= same;
                            shape.body.push(item);
                        }
                    }
                    None => exact = false,
                }
            } else {
                exact = false;
            }
        }
        // 書けない組み合わせの `Body` は落とし、芯の線だけを書く
        if shape_rejected(&shape, self.version).is_some() && !shape.body.is_empty() {
            exact = false;
            shape.body.clear();
        }
        shape_rejected(&shape, self.version)
            .is_none()
            .then_some((shape, exact))
    }

    /// 形の項目と、そのまま書けたか。集合演算と写像はたどり、共有する写像元は項目ごとに展開する。深さと、
    /// 展開した項目の数（`budget`）を超えたら書かない
    fn new_item(
        &self,
        data: &GeometryData,
        at: GeometryRef,
        depth: usize,
        budget: &mut usize,
    ) -> Option<(NewItem, bool)> {
        if depth >= SHAPE_NESTING {
            return None;
        }
        *budget = budget.checked_sub(1)?;
        let node = data.nodes.get(at.0 as usize)?;
        match &node.links {
            GeometryLinks::Boolean {
                operation,
                first,
                second,
            } => {
                let (first, a) = self.new_item(data, *first, depth + 1, budget)?;
                let (second, b) = self.new_item(data, *second, depth + 1, budget)?;
                let boolean = NewBoolean {
                    operation: *operation,
                    first,
                    second,
                };
                return Some((NewItem::Boolean(Box::new(boolean)), a && b));
            }
            GeometryLinks::Mapped(map) => {
                let GeometryDefinition::Mapping {
                    definition,
                    length_unit,
                } = &node.definition
                else {
                    return None;
                };
                if !self.length_written(length_unit.as_ref()) {
                    return None;
                }
                let GeometryLinks::Map(representation) = &data.nodes.get(map.0 as usize)?.links
                else {
                    return None;
                };
                let GeometryLinks::Representation { items, .. } =
                    &data.nodes.get(representation.0 as usize)?.links
                else {
                    return None;
                };
                let mut same = true;
                let mut inner = Vec::with_capacity(items.len());
                for &item in items {
                    let (item, exact) = self.new_item(data, item, depth + 1, budget)?;
                    same &= exact;
                    inner.push(item);
                }
                let mapped = NewMapped {
                    origin: new_axis3(&definition.origin),
                    target: definition.target.clone(),
                    items: inner,
                };
                return Some((NewItem::Mapped(Box::new(mapped)), same));
            }
            _ => {}
        }
        if let GeometryDefinition::HalfSpace {
            definition,
            boundary,
            length_unit,
        } = &node.definition
        {
            if !self.length_written(length_unit.as_ref()) {
                return None;
            }
            // 箱の座標系は書くバージョンで決まる。違えば囲みを落とす（計算用の囲みで、形は変わらない）
            let mut same = true;
            let limit = match &definition.limit {
                HalfSpaceLimit::Unbounded => NewHalfSpaceLimit::Unbounded,
                HalfSpaceLimit::Box {
                    corner,
                    dimensions,
                    coordinates,
                } => {
                    let written = if self.version == Version::Ifc2x3 {
                        EnclosureCoordinates::BaseSurface
                    } else {
                        EnclosureCoordinates::Object
                    };
                    if *coordinates == written {
                        NewHalfSpaceLimit::Box {
                            corner: *corner,
                            dimensions: *dimensions,
                        }
                    } else {
                        same = false;
                        NewHalfSpaceLimit::Unbounded
                    }
                }
                HalfSpaceLimit::Polygonal {
                    position,
                    boundary: at,
                } => {
                    let graph = boundary.as_ref()?;
                    if !self.length_written(graph.length_unit.as_ref()) {
                        return None;
                    }
                    NewHalfSpaceLimit::Polygonal {
                        position: new_axis3(position),
                        boundary: ring(graph, *at)?,
                    }
                }
            };
            let half = NewHalfSpace {
                plane: new_axis3(&definition.plane),
                agreement: definition.agreement,
                limit,
            };
            return Some((NewItem::HalfSpace(half), same));
        }
        if let Some(solid) = self.new_solid(node) {
            return Some((NewItem::Solid(solid), true));
        }
        if let Some(revolution) = self.new_revolution(node) {
            return Some((NewItem::Revolution(revolution), true));
        }
        if let Some(disk) = self.new_disk(node) {
            return Some((NewItem::SweptDisk(disk), true));
        }
        self.new_faces(node)
            .map(|(faces, same)| (NewItem::Faces(faces), same))
    }

    /// 面集合と、そのまま書けたか。添字の面は IFC2X3 へ多角形の面にし、IFC4 へは点のタグを落とす。面の模型は
    /// IFC4 から非推奨なので、開殻の殻の模型にする
    fn new_faces(&self, node: &GeometryNode) -> Option<(NewFaces, bool)> {
        match &node.definition {
            GeometryDefinition::Faces {
                definition,
                length_unit,
            } if self.length_written(length_unit.as_ref()) => {
                let mut graph = definition.clone();
                if graph.kind != FaceModelKind::FaceSurface || self.version == Version::Ifc2x3 {
                    return Some((NewFaces::Polygons(graph), true));
                }
                graph.kind = FaceModelKind::ShellSurface;
                for set in &mut graph.sets {
                    set.closed.get_or_insert(false);
                }
                Some((NewFaces::Polygons(graph), false))
            }
            GeometryDefinition::Tessellation {
                definition,
                length_unit,
            } if self.length_written(length_unit.as_ref()) => {
                if self.version == Version::Ifc2x3 {
                    return polygon_faces(definition)
                        .map(|graph| (NewFaces::Polygons(graph), false));
                }
                let mut set = definition.clone();
                let mut same = true;
                if self.version != Version::Ifc4x3 && set.tags.take().is_some() {
                    same = false;
                }
                Some((NewFaces::Indexed(set), same))
            }
            _ => None,
        }
    }

    /// `Axis` の 2 次元の折れ線
    fn axis_line(&self, node: &GeometryNode) -> Option<Vec<[f64; 2]>> {
        let GeometryDefinition::Curve(graph) = &node.definition else {
            return None;
        };
        if !self.length_written(graph.length_unit.as_ref()) {
            return None;
        }
        let CurveKind::Polyline(points) = &graph.item(graph.root)?.kind else {
            return None;
        };
        points.iter().map(planar).collect()
    }

    /// 矩形・円・折れ線の断面の押し出し
    fn new_solid(&self, node: &GeometryNode) -> Option<NewSolid> {
        let GeometryDefinition::Extrusion {
            definition,
            boundary,
            length_unit,
        } = &node.definition
        else {
            return None;
        };
        if !self.length_written(length_unit.as_ref()) {
            return None;
        }
        Some(NewSolid {
            profile: self.new_profile(&definition.profile, boundary.as_ref())?,
            position: definition.position.as_ref().map(new_axis3),
            direction: definition.direction,
            depth: definition.depth,
        })
    }

    /// 断面の回転。軸が断面の平面にあり、角度をラジアンに直せるもの
    fn new_revolution(&self, node: &GeometryNode) -> Option<NewRevolution> {
        let GeometryDefinition::Revolution {
            definition,
            boundary,
            length_unit,
        } = &node.definition
        else {
            return None;
        };
        if !self.length_written(length_unit.as_ref()) {
            return None;
        }
        let axis = &definition.axis;
        let [x, y, 0.0] = axis.location else {
            return None;
        };
        let [dx, dy, 0.0] = axis.direction else {
            return None;
        };
        // 角度は文書の平面角の単位のまま持つ。換算する単位は SI の単位（ラジアン）での大きさを掛ける
        let unit = definition.angle.unit.as_ref()?;
        let scale = match unit.scale {
            Some(scale) => scale,
            None if unit.prefix.is_none() && unit.name.eq_ignore_ascii_case("RADIAN") => 1.0,
            None => return None,
        };
        Some(NewRevolution {
            profile: self.new_profile(&definition.profile, boundary.as_ref())?,
            position: definition.position.as_ref().map(new_axis3),
            location: [x, y],
            direction: [dx, dy],
            angle: definition.angle.value * scale,
        })
    }

    /// 3 次元の折れ線に沿う円盤の掃引
    fn new_disk(&self, node: &GeometryNode) -> Option<NewSweptDisk> {
        let GeometryDefinition::SweptDisk {
            definition,
            directrix,
        } = &node.definition
        else {
            return None;
        };
        if !self.length_written(directrix.length_unit.as_ref()) {
            return None;
        }
        let CurveKind::Polyline(points) = &directrix.item(definition.directrix)?.kind else {
            return None;
        };
        let points = points
            .iter()
            .map(|point| match point {
                Coordinates::Three(point) => Some(*point),
                Coordinates::Two(_) => None,
            })
            .collect::<Option<Vec<_>>>()?;
        Some(NewSweptDisk {
            directrix: points,
            radius: definition.radius,
            inner_radius: definition.inner_radius,
            start: definition.start_parameter,
            end: definition.end_parameter,
        })
    }

    /// 矩形・円・折れ線の面の断面
    fn new_profile(
        &self,
        profile: &AreaProfile,
        boundary: Option<&CurveGraph>,
    ) -> Option<NewProfile> {
        if boundary.is_some_and(|graph| !self.length_written(graph.length_unit.as_ref())) {
            return None;
        }
        let axis2 = |p: &AxisPlacement2| NewAxis2 {
            location: p.location,
            reference: p.reference_direction,
        };
        let kind = match &profile.kind {
            AreaProfileKind::Rectangle { x_dim, y_dim } => NewProfileKind::Rectangle {
                x: *x_dim,
                y: *y_dim,
                position: profile.position.as_ref().map(axis2),
            },
            AreaProfileKind::Circle { radius } => NewProfileKind::Circle {
                radius: *radius,
                position: profile.position.as_ref().map(axis2),
            },
            AreaProfileKind::ArbitraryClosed { boundary: outer } => NewProfileKind::Polygon {
                outer: ring(boundary?, *outer)?,
                holes: Vec::new(),
            },
            AreaProfileKind::ArbitraryWithVoids {
                boundary: outer,
                holes,
            } => {
                let graph = boundary?;
                NewProfileKind::Polygon {
                    outer: ring(graph, *outer)?,
                    holes: holes
                        .iter()
                        .map(|&hole| ring(graph, hole))
                        .collect::<Option<_>>()?,
                }
            }
            _ => return None,
        };
        Some(NewProfile {
            name: profile.name.clone(),
            kind,
        })
    }

    /// 長さの単位を書いたか。単位のない値は文書の単位とみる
    fn length_written(&self, unit: Option<&Unit>) -> bool {
        unit.is_none_or(|unit| self.units.contains(&unit))
    }

    /// 書いた要素か型に材料の構成を結ぶ。そのまま書けたものは、落とした値から外す
    fn material(&mut self, element: ElementRef) {
        let Some(material) = self.building.components.material.get(element) else {
            return;
        };
        let Some(target) = self.target(element) else {
            return;
        };
        let Some((new, exact)) = self.new_material(material) else {
            return;
        };
        self.used_materials
            .extend(material.parts.iter().filter_map(|part| part.material));
        self.edits.push(Edit::Material {
            element: target,
            material: new,
        });
        if exact && let Some((_, dropped)) = self.notes.get_mut(&element) {
            dropped.retain(|&value| value != "material");
        }
    }

    /// 書ける材料の構成と、そのまま書けたか。断面（形を持たない）と、名前のない材料を使うものは書かない
    fn new_material(&self, material: &Material) -> Option<(NewMaterial, bool)> {
        let name = |part: Option<ElementRef>| part.and_then(|m| self.name(m));
        let later = self.version != Version::Ifc2x3;
        let mut exact = true;
        let new = match material.kind {
            MaterialKind::Single => match material.parts.as_slice() {
                [part] => NewMaterial::Single(name(part.material)?),
                _ => return None,
            },
            // IFC4 からは並びが非推奨なので名前のない構成要素に、IFC2X3 は構成要素を持たないので並びで書く
            MaterialKind::List | MaterialKind::Constituents if later => {
                exact &= material.kind == MaterialKind::Constituents;
                NewMaterial::Constituents(
                    material
                        .parts
                        .iter()
                        .map(|part| {
                            Some(NewConstituent {
                                material: name(part.material)?,
                                name: part.name.clone(),
                            })
                        })
                        .collect::<Option<_>>()?,
                )
            }
            MaterialKind::List | MaterialKind::Constituents => {
                exact &= material.kind == MaterialKind::List;
                NewMaterial::List(
                    material
                        .parts
                        .iter()
                        .map(|part| name(part.material))
                        .collect::<Option<_>>()?,
                )
            }
            MaterialKind::Layers => {
                let mut layers = Vec::with_capacity(material.parts.len());
                for part in &material.parts {
                    let thickness = part.thickness.as_ref()?;
                    if !self.unit_written(thickness) {
                        return None;
                    }
                    // IFC2X3 の層は名前を持たない
                    if !later && part.name.is_some() {
                        exact = false;
                    }
                    layers.push(NewMaterialLayer {
                        material: match part.material {
                            Some(m) => Some(self.name(m)?),
                            None => None,
                        },
                        thickness: thickness.value,
                        name: part.name.clone().filter(|_| later),
                    });
                }
                // 層の使い方の向きの正負（DirectionSense）は、読んだ値に残らないので正にする
                let usage = match &material.direction {
                    Some(direction) => {
                        exact = false;
                        let offset = match &material.offset {
                            Some(offset) if self.unit_written(offset) => offset.value,
                            Some(_) => return None,
                            None => 0.0,
                        };
                        Some(NewLayerUsage {
                            direction: direction.clone(),
                            negative: false,
                            offset,
                        })
                    }
                    None => None,
                };
                NewMaterial::Layers { layers, usage }
            }
            MaterialKind::Profiles => return None,
        };
        material_rejected(&new, self.version)
            .is_none()
            .then_some((new, exact))
    }

    /// 材料の要素。書いた構成が使うものは書いたものとし、ほかは落とす
    fn material_elements(&mut self, cx: &mut Cx) -> Result<(), Error> {
        for element in std::mem::take(&mut self.materials) {
            if self.used_materials.contains(&element) {
                let dropped = self.components(element);
                self.notes.insert(element, ("IfcMaterial", dropped));
            } else {
                self.drop_element(element, "IfcMaterial", cx)?;
            }
        }
        Ok(())
    }

    /// 書いた要素を、変更の中で指す値
    fn target(&self, element: ElementRef) -> Option<Target> {
        self.written.get(&element).copied()
    }

    fn global_id(&self, element: ElementRef) -> Option<GlobalId> {
        match self.building.id(element)? {
            Id::Global(global) => Some(global),
            id @ Id::Instance(_) => Some(derive_global_id(
                self.namespace,
                format!("element/{id}").as_bytes(),
            )),
        }
    }

    fn name(&self, element: ElementRef) -> Option<String> {
        self.building
            .components
            .identity
            .get(element)
            .and_then(|i| i.name.clone())
    }

    /// 配置。書いた祖先まで変換を畳む。解けなければ `Err(())`、元に配置がなければ `Ok(None)`
    fn placement(&self, element: ElementRef) -> Result<Option<NewPlacement>, ()> {
        let placements = &self.building.components.placement;
        let Some(placement) = placements.get(element) else {
            return Ok(None);
        };
        let Local::Transform(mut local) = placement.local else {
            return Err(());
        };
        let mut relative_to = placement.relative_to;
        let mut seen = BTreeSet::new();
        while let Some(parent) = relative_to {
            if let Some(target) = self.target(parent) {
                return checked(NewPlacement {
                    relative_to: Some(target),
                    local,
                });
            }
            if !seen.insert(parent) {
                return Err(());
            }
            let Some(outer) = placements.get(parent) else {
                break;
            };
            let Local::Transform(outer_local) = &outer.local else {
                return Err(());
            };
            local = outer_local.then(&local);
            relative_to = outer.relative_to;
        }
        checked(NewPlacement {
            relative_to: None,
            local,
        })
    }

    fn space(&mut self, element: ElementRef, kind: SpaceKind, cx: &mut Cx) -> Result<(), Error> {
        let building = self.building;
        let class = match kind {
            SpaceKind::Site => "IfcSite",
            SpaceKind::Building => "IfcBuilding",
            SpaceKind::Storey => "IfcBuildingStorey",
        };
        let parent = building
            .components
            .parent
            .get(element)
            .and_then(|p| p.of.as_ref())
            .filter(|of| of.kind == ParentKind::Aggregates)
            .and_then(|of| self.target(of.target));
        let Some(parent) = parent else {
            return self.drop_element(element, class, cx);
        };
        let mut dropped = Vec::new();
        let placement = self.placement(element).unwrap_or_else(|()| {
            dropped.push("placement");
            None
        });
        let spatial = building.components.spatial.get(element);
        let elevation = match kind {
            SpaceKind::Site => spatial
                .and_then(|s| s.ref_elevation.as_ref())
                .map(|m| m.value),
            SpaceKind::Building | SpaceKind::Storey => {
                let written = spatial.and_then(|s| s.elevation.as_ref()).map(|m| m.value);
                let level = if kind == SpaceKind::Storey {
                    building
                        .components
                        .level
                        .get(element)
                        .map(|level| level.height.value)
                } else {
                    None
                };
                match self.version {
                    Version::Ifc4x3 => {
                        // 高さは配置だけが持つ。元の値が親からの配置の z と同じなら落とさない
                        let placed = placement
                            .as_ref()
                            .filter(|p| p.relative_to == Some(parent))
                            .map(|p| p.local.0[2][3]);
                        if written.is_some_and(|h| Some(h) != placed) {
                            dropped.push("elevation");
                        }
                        None
                    }
                    Version::Ifc2x3 | Version::Ifc4 => written.or(level),
                }
            }
        };
        if building
            .components
            .class
            .get(element)
            .is_some_and(|c| c.predefined.is_some() || c.user_defined.is_some())
        {
            dropped.push("predefined");
        }
        dropped.extend(self.components(element));
        self.edits.push(Edit::Space(NewSpace {
            kind,
            name: self.name(element),
            parent,
            placement,
            elevation,
            global_id: self.global_id(element),
        }));
        self.wrote(element, class, dropped);
        Ok(())
    }

    fn product(&mut self, element: ElementRef, class: &str, cx: &mut Cx) -> Result<(), Error> {
        let building = self.building;
        let Some((kind, slot)) = writable(class, self.version) else {
            return self.drop_element(element, class, cx);
        };
        let mut dropped = Vec::new();
        let source = building.components.class.get(element);
        let predefined = match source.and_then(|c| c.predefined.clone()) {
            Some(value) if slot > 0 && enumeration(&value).is_some() => Some(value),
            Some(_) => {
                dropped.push("predefined");
                None
            }
            None => None,
        };
        if source.is_some_and(|c| c.user_defined.is_some()) && !dropped.contains(&"predefined") {
            dropped.push("predefined");
        }
        let membership = building
            .components
            .parent
            .get(element)
            .and_then(|p| p.of.as_ref());
        let container = match membership {
            Some(of) if of.kind == ParentKind::Contained => {
                let spatial = building.components.spatial.get(of.target).is_some_and(|s| {
                    matches!(
                        s.kind,
                        SpatialKind::Site | SpatialKind::Building | SpatialKind::Storey
                    )
                });
                let target = self.target(of.target).filter(|_| spatial);
                if target.is_none() {
                    dropped.push("parent");
                }
                target
            }
            Some(_) => {
                dropped.push("parent");
                None
            }
            None => None,
        };
        let placement = self.placement(element).unwrap_or_else(|()| {
            dropped.push("placement");
            None
        });
        // 形は配置に置くので、配置を書く要素にだけ書く
        let placed = placement.is_some();
        dropped.extend(self.components(element));
        let type_of = building
            .components
            .r#type
            .get(element)
            .and_then(|t| self.target(t.target));
        if type_of.is_some() {
            dropped.retain(|&value| value != "type");
        }
        self.edits.push(Edit::Element(NewElement {
            class: kind.to_owned(),
            predefined,
            name: self.name(element),
            container,
            placement,
            global_id: self.global_id(element),
        }));
        let added = Target::Added(self.added);
        self.wrote(element, kind, dropped);
        self.material(element);
        if placed {
            self.shape(element);
        } else if self.shaped(element) {
            self.lost(element, "representation");
        }
        if let Some(type_of) = type_of {
            self.edits.push(Edit::Type {
                element: added,
                type_of: Some(type_of),
            });
        }
        Ok(())
    }

    /// 書いた要素の、書き手がまだ書けない値
    fn components(&self, element: ElementRef) -> Vec<&'static str> {
        let c = &self.building.components;
        let mut dropped = Vec::new();
        if c.identity
            .get(element)
            .is_some_and(|i| i.description.is_some() || i.tag.is_some() || i.long_name.is_some())
        {
            dropped.push("identity");
        }
        let present: [(&'static str, bool); 10] = [
            ("classification", c.classification.get(element).is_some()),
            ("representation", c.representation.get(element).is_some()),
            ("style", c.style.get(element).is_some()),
            ("type", c.r#type.get(element).is_some()),
            (
                "property_details",
                c.property_details.get(element).is_some(),
            ),
            ("material", c.material.get(element).is_some()),
            ("group", c.group.get(element).is_some()),
            ("connection", c.connection.get(element).is_some()),
            ("zone", c.zone.get(element).is_some()),
            ("extension", c.extension.get(element).is_some()),
        ];
        dropped.extend(present.iter().filter(|(_, p)| *p).map(|(name, _)| *name));
        if c.parent
            .get(element)
            .is_some_and(|p| !p.referenced_in.is_empty())
        {
            dropped.push("referenced_in");
        }
        dropped
    }

    /// プロパティのセットを持つ、書いた要素と GlobalId
    fn properties(&self) -> Vec<(ElementRef, GlobalId)> {
        let project = self.building.document.project;
        self.notes
            .keys()
            .filter(|&&e| {
                let c = &self.building.components;
                c.properties.get(e).is_some_and(|p| !p.sets.is_empty())
                    || c.quantities.get(e).is_some_and(|q| !q.sets.is_empty())
            })
            .filter_map(|&e| {
                let global = if Some(e) == project {
                    match self.building.id(e)? {
                        Id::Global(global) => global,
                        Id::Instance(_) => derive_global_id(self.namespace, b"project"),
                    }
                } else {
                    self.global_id(e)?
                };
                Some((e, global))
            })
            .collect()
    }

    /// 書いた要素のプロパティを、書いた文書のインスタンスへ当てる変更。書けない値は落とした値として数える
    fn property_edits(
        &mut self,
        wanted: &[(ElementRef, GlobalId)],
        instances: &BTreeMap<String, u64>,
    ) -> Vec<Edit> {
        let building = self.building;
        let mut edits = Vec::new();
        for &(element, global) in wanted {
            let Some(&instance) = instances.get(&global.to_string()) else {
                if building.components.properties.get(element).is_some() {
                    self.lost(element, "properties");
                }
                if building.components.quantities.get(element).is_some() {
                    self.lost(element, "quantities");
                }
                continue;
            };
            if let Some(sets) = building.components.quantities.get(element) {
                let mut lost = false;
                for set in &sets.sets {
                    lost |= set.quantities.is_empty();
                    for quantity in &set.quantities {
                        match self.quantity(quantity) {
                            Some((quantity, partial)) => {
                                lost |= partial;
                                edits.push(Edit::Quantity {
                                    element: instance,
                                    set: set.name.clone(),
                                    method: set.method.clone(),
                                    quantity,
                                });
                            }
                            None => lost = true,
                        }
                    }
                }
                if lost {
                    self.lost(element, "quantities");
                }
            }
            let Some(sets) = building.components.properties.get(element) else {
                continue;
            };
            let mut lost = false;
            for set in &sets.sets {
                lost |= set.properties.is_empty();
                for property in &set.properties {
                    match property_value(property) {
                        Some(value) => edits.push(Edit::Property {
                            element: instance,
                            set: set.name.clone(),
                            name: property.name.clone(),
                            value,
                        }),
                        None => lost = true,
                    }
                }
            }
            if lost {
                self.lost(element, "properties");
            }
        }
        edits
    }

    /// 書ける数量と、式を落としたか。値の単位が書いた文書の単位で、値がそのバージョンに書けるもの。IFC2X3 は式を
    /// 持たないので落とす
    fn quantity(&self, quantity: &Quantity) -> Option<(NewQuantity, bool)> {
        if !self.unit_written(&quantity.value)
            || quantity_value(quantity.kind, quantity.value.value, self.version).is_none()
        {
            return None;
        }
        let keeps_formula = self.version != Version::Ifc2x3;
        let new = NewQuantity {
            name: quantity.name.clone(),
            kind: quantity.kind,
            value: quantity.value.value,
            formula: quantity.formula.clone().filter(|_| keeps_formula),
        };
        Some((new, quantity.formula.is_some() && !keeps_formula))
    }

    /// 開口と建具の関係。両方を書いたものだけを結ぶ
    fn openings(&mut self) {
        let building = self.building;
        for (element, opening) in building.components.opening.iter() {
            let Some(this) = self.target(element) else {
                continue;
            };
            let (other, edit): (ElementRef, fn(Target, Target) -> Edit) = match *opening {
                Opening::Voids(host) => (host, |host, opening| Edit::Void { host, opening }),
                Opening::Fills(opening) => {
                    (opening, |opening, filler| Edit::Fill { opening, filler })
                }
            };
            match self.target(other) {
                Some(other) => self.edits.push(edit(other, this)),
                None => self.lost(element, "opening"),
            }
        }
    }
}

/// 書き手の表にある要素の名前（[`predefined_at`] の表と同じもの）
const WRITABLE_NAMES: &[&str] = &[
    "IfcWall",
    "IfcSlab",
    "IfcColumn",
    "IfcBeam",
    "IfcMember",
    "IfcPlate",
    "IfcCovering",
    "IfcRailing",
    "IfcRoof",
    "IfcStair",
    "IfcRamp",
    "IfcFooting",
    "IfcPile",
    "IfcDoor",
    "IfcWindow",
    "IfcOpeningElement",
    "IfcBuildingElementProxy",
];

/// 書ける要素の、表の名前と定義された種類の位置（1 から、0 は持たない）
fn writable(class: &str, version: Version) -> Option<(&'static str, usize)> {
    let slot = predefined_at(class, version)?;
    let name = WRITABLE_NAMES
        .iter()
        .find(|name| name.eq_ignore_ascii_case(class))?;
    Some((name, slot))
}

/// 書き手が受ける変換なら、そのまま
fn checked(placement: NewPlacement) -> Result<Option<NewPlacement>, ()> {
    if rigid(&placement.local) {
        Ok(Some(placement))
    } else {
        Err(())
    }
}

/// 1 つの `Body` で展開する項目の数の上限。共有する写像元を項目ごとに展開するので、入れ子の写像で増えすぎない
const BODY_ITEMS: usize = 10_000;

fn new_axis3(p: &AxisPlacement3) -> NewAxis3 {
    NewAxis3 {
        location: p.location,
        axis: p.axis,
        reference: p.reference_direction,
    }
}

/// 2 次元の点
fn planar(point: &Coordinates) -> Option<[f64; 2]> {
    match point {
        Coordinates::Two(point) => Some(*point),
        Coordinates::Three(_) => None,
    }
}

/// 折れ線の輪。始点に戻る点は外す
fn ring(graph: &CurveGraph, at: CurveRef) -> Option<Vec<[f64; 2]>> {
    let CurveKind::Polyline(points) = &graph.item(at)?.kind else {
        return None;
    };
    let mut ring = points.iter().map(planar).collect::<Option<Vec<_>>>()?;
    if ring.len() > 1 && ring.first() == ring.last() {
        ring.pop();
    }
    Some(ring)
}

/// 添字の面集合を IFC2X3 の多角形の面の模型にする。点の対応を当て、法線とタグは落とす。閉殻の申告があれば
/// 立体、開殻なら殻の面の模型、なければ面の模型
fn polygon_faces(set: &IndexedFaceSet) -> Option<FaceGraph> {
    let point = |index: u32| -> Option<u32> {
        let at = index.checked_sub(1)? as usize;
        match &set.point_index {
            Some(map) => map.get(at)?.checked_sub(1),
            None => Some(at as u32),
        }
    };
    let polygons: Vec<Vec<&[u32]>> = match &set.faces {
        IndexedFaces::Triangles { indices, .. } => {
            indices.iter().map(|row| vec![row.as_slice()]).collect()
        }
        IndexedFaces::Polygons { indices, faces } => indices
            .iter()
            .map(|at| {
                faces.get(at.0 as usize).map(|face| {
                    let mut rings = vec![face.boundary.as_slice()];
                    rings.extend(face.holes.iter().map(Vec::as_slice));
                    rings
                })
            })
            .collect::<Option<_>>()?,
    };
    let mut graph = FaceGraph {
        kind: match set.closed {
            Some(true) => FaceModelKind::FacetedSolid,
            Some(false) => FaceModelKind::ShellSurface,
            None => FaceModelKind::FaceSurface,
        },
        roots: vec![FaceSetRef(0)],
        sets: vec![ConnectedFaces {
            faces: Vec::with_capacity(polygons.len()),
            closed: set.closed,
        }],
        faces: Vec::with_capacity(polygons.len()),
        bounds: Vec::new(),
        loops: Vec::new(),
        points: Vec::new(),
    };
    // 点は最初に使った順に並べ、同じ点は共有する
    let mut known = BTreeMap::new();
    for rings in polygons {
        let mut bounds = Vec::with_capacity(rings.len());
        for (n, ring) in rings.into_iter().enumerate() {
            let mut points = Vec::with_capacity(ring.len());
            for &index in ring {
                let at = point(index)?;
                let next = u32::try_from(graph.points.len()).ok()?;
                let mapped = *known.entry(at).or_insert(next);
                if mapped == next {
                    graph.points.push(*set.points.get(at as usize)?);
                }
                points.push(FacePointRef(mapped));
            }
            bounds.push(FaceBoundRef(u32::try_from(graph.bounds.len()).ok()?));
            graph.bounds.push(FaceBoundary {
                polygon: FaceLoopRef(u32::try_from(graph.loops.len()).ok()?),
                orientation: true,
                outer: n == 0,
            });
            graph.loops.push(FacePolygon { points });
        }
        let face = FaceRef(u32::try_from(graph.faces.len()).ok()?);
        graph.faces.push(PolygonFace { bounds });
        graph.sets.first_mut()?.faces.push(face);
    }
    Some(graph)
}
