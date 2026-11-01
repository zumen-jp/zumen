//! IFC の建物モデルの文書と、`building`・`building_elements`・`building_apply`・`ids_check` の op（決定 0207・0217・0219・
//! 0244）。

use std::collections::BTreeMap;

use zumen_base::{Cx, Diagnostic, Diagnostics, Kind, Stop, codes};
use zumen_building::{
    Building, ElementRef, Hash, Id, Literal, Local, MaterialKind, Opening, ParentKind,
    QuantityKind, Relations, SpatialKind, TypedValue, Unit,
};
use zumen_ids::{Ids, Report};
use zumen_ifc::{Edit, Ifc, IfcFile, IfcZip, NewLiteral, NewValue, RootAttribute};
use zumen_sdk::{Codec, Lift};

use crate::messages::{
    self, BuildingApplyRequest, BuildingApplyResult, BuildingAttribute, BuildingComponent,
    BuildingEdit, BuildingElement, BuildingElementsRequest, BuildingElementsResult, BuildingResult,
    BuildingUnit, BuildingValue, BuildingValueKind, ClassComponent, ClassCount,
    ClassificationReference, ConnectionLink, IdentityComponent, IdsCheckResult, IdsFinding,
    IdsSpecification, IdsStatus, LevelComponent, MaterialComponent, MaterialPart, Measured,
    Membership, OpeningComponent, ParentComponent, PlacementComponent, Property, PropertySet,
    Quantity, QuantitySet, RelationsComponent, SpatialComponent, StyleComponent, ZoneComponent,
};
use crate::session::{Failed, fail, invalid_value, message_of, stopped};

/// limit を書かなかったときに返す数
const LIMIT: u64 = 1000;

/// 書き忘れたときに返す成分
const DEFAULT_COMPONENTS: [BuildingComponent; 2] =
    [BuildingComponent::Class, BuildingComponent::Identity];

/// 開いた IFC。L1 と建物モデルと、入力のバイト列のハッシュ
#[derive(Debug)]
pub(crate) struct Model {
    pub(crate) file: IfcFile,
    pub(crate) building: Building,
    pub(crate) hash: Hash,
    /// ifcZIP で開いたなら、中の IFC-SPF の項目の名前
    pub(crate) entry: Option<String>,
}

/// IFC-SPF なら読む。IFC-SPF でなければ `None`
pub(crate) fn read_spf(bytes: &[u8], cx: &mut Cx<'_>) -> Option<Result<Model, Failed>> {
    if !Ifc::SIGNATURES.iter().any(|s| s.matches(bytes)) {
        return None;
    }
    Some(
        Ifc::read(bytes, cx)
            .map_err(|_| Failed)
            .and_then(|file| model(file, bytes, None, cx)),
    )
}

/// ifcZIP なら読む。ZIP でなければ `None`。ZIP の署名はほかの形式とも重なるので、最後に試す
pub(crate) fn read_zip(bytes: &[u8], cx: &mut Cx<'_>) -> Option<Result<Model, Failed>> {
    if !IfcZip::SIGNATURES.iter().any(|s| s.matches(bytes)) {
        return None;
    }
    Some(
        IfcZip::read(bytes, cx)
            .map_err(|_| Failed)
            .and_then(|zipped| {
                let entry = Some(zipped.name().to_owned());
                model(zipped.ifc().clone(), bytes, entry, cx)
            }),
    )
}

fn model(
    file: IfcFile,
    bytes: &[u8],
    entry: Option<String>,
    cx: &mut Cx<'_>,
) -> Result<Model, Failed> {
    let building = file.lift(cx).map_err(|_| Failed)?;
    let hash = Hash::document_checked(bytes, cx).map_err(|stop| stopped(cx, stop))?;
    Ok(Model {
        file,
        building,
        hash,
        entry,
    })
}

/// 変更の一覧を IFC に当て、書き戻した文書と結果と、書き戻したファイルを返す（決定 0244）。元の文書は変えない
pub(crate) fn apply(
    model: &Model,
    request: &BuildingApplyRequest,
    cx: &mut Cx<'_>,
) -> Result<(Model, BuildingApplyResult, Vec<u8>), Failed> {
    if let Some(text) = &request.hash {
        let hash = text
            .parse::<Hash>()
            .map_err(|()| fail(cx, invalid_value("/hash")))?;
        if hash != model.hash {
            return Err(fail(cx, Diagnostic::new(codes::CHANGE_DOCUMENT_STALE)));
        }
    }
    let mut edits = Vec::with_capacity(request.edits.len());
    let mut targets = Vec::with_capacity(request.edits.len());
    let mut rejected = false;
    for (index, edit) in request.edits.iter().enumerate() {
        cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
        let (id, hash) = match edit {
            BuildingEdit::Property(edit) => (&edit.id, &edit.hash),
            BuildingEdit::Attribute(edit) => (&edit.id, &edit.hash),
        };
        let expected = hash
            .parse::<Hash>()
            .map_err(|()| fail(cx, invalid_value(format!("/edits/{index}/hash"))))?;
        // 値の形は、要素を引く前に確かめる
        let value = match edit {
            BuildingEdit::Property(edit) => Some(
                new_value(&edit.value)
                    .ok_or_else(|| fail(cx, invalid_value(format!("/edits/{index}/value"))))?,
            ),
            BuildingEdit::Attribute(_) => None,
        };
        let Some(element) = id.parse::<Id>().ok().and_then(|id| model.building.find(id)) else {
            cx.report(Diagnostic::new(codes::CHANGE_ELEMENT_MISSING).with("id", id.as_str()));
            rejected = true;
            continue;
        };
        let current = Hash::element_checked(&model.building, element, cx)
            .map_err(|stop| stopped(cx, stop))?;
        if current != Some(expected) {
            cx.report(Diagnostic::new(codes::CHANGE_ELEMENT_STALE).with("id", id.as_str()));
            rejected = true;
            continue;
        }
        let instance = model
            .building
            .element(element)
            .and_then(|e| e.source().instance)
            .unwrap_or_default();
        edits.push(match (edit, value) {
            (BuildingEdit::Property(edit), Some(value)) => Edit::Property {
                element: instance,
                set: edit.set.clone(),
                name: edit.name.clone(),
                value,
            },
            (BuildingEdit::Property(_), None) => return Err(Failed),
            (BuildingEdit::Attribute(edit), _) => Edit::Attribute {
                element: instance,
                attribute: match edit.attribute {
                    BuildingAttribute::Name => RootAttribute::Name,
                    BuildingAttribute::Description => RootAttribute::Description,
                },
                value: edit.value.clone(),
            },
        });
        targets.push(element);
    }
    if rejected {
        return Err(Failed);
    }
    let applied = zumen_ifc::apply(&model.file, &edits, &model.hash.0, cx).map_err(|_| Failed)?;
    // 書き戻したものを読み直す。読んだときの診断は、開いたときに返したので捨てる
    let mut scratch = Diagnostics::new();
    let reread = {
        let mut inner = Cx {
            budget: &mut *cx.budget,
            diag: &mut scratch,
            cancel: cx.cancel,
        };
        Ifc::read(&applied.bytes, &mut inner)
            .and_then(|file| file.lift(&mut inner).map(|building| (file, building)))
    };
    let (file, building) = match reread {
        Ok(read) => read,
        Err(zumen_sdk::Error::Stopped(stop)) => return Err(stopped(cx, stop)),
        // 書いたものを読めないのは不具合
        Err(_) => return Err(fail(cx, Diagnostic::new(codes::INTERNAL_PANIC_CAUGHT))),
    };
    let bytes = match &model.entry {
        Some(entry) => zumen_zip::write(&[(entry.as_str(), &applied.bytes)])
            .ok_or_else(|| fail(cx, Diagnostic::new(codes::INTERNAL_PANIC_CAUGHT)))?,
        None => applied.bytes,
    };
    let hash = Hash::document_checked(&bytes, cx).map_err(|stop| stopped(cx, stop))?;
    let updated = Model {
        file,
        building,
        hash,
        entry: model.entry.clone(),
    };
    let mut results = Vec::with_capacity(targets.len());
    for element in targets {
        // 変えた要素の ID は変わらない。足したインスタンスは、元の要素の後ろに並ぶ
        let id = model.building.id(element);
        let after = id.and_then(|id| updated.building.find(id));
        let hash = match after {
            Some(after) => Hash::element_checked(&updated.building, after, cx)
                .map_err(|stop| stopped(cx, stop))?,
            None => None,
        };
        results.push(messages::Applied {
            id: after.and(id).map(|id| id.to_string()),
            hash: hash.map(|hash| hash.to_string()),
        });
    }
    let result = BuildingApplyResult {
        applied: results,
        hash: updated.hash.to_string(),
    };
    Ok((updated, result, bytes))
}

/// 境界の値を、書く値にする。書けない種類か、欄が足りなければ `None`
fn new_value(value: &BuildingValue) -> Option<NewValue> {
    let literal = match value.kind {
        BuildingValueKind::Real => NewLiteral::Real(value.number.filter(|n| n.is_finite())?),
        BuildingValueKind::Integer => match (value.number, &value.text) {
            (Some(number), None) => {
                // 2^53 − 1 までの整数だけを数で受ける
                (number.fract() == 0.0 && number.abs() <= 9_007_199_254_740_991.0)
                    .then_some(NewLiteral::Integer(number as i64))?
            }
            (None, Some(text)) => NewLiteral::Integer(text.parse().ok()?),
            _ => return None,
        },
        BuildingValueKind::Text => NewLiteral::Text(value.text.clone()?),
        BuildingValueKind::Boolean => NewLiteral::Boolean(value.boolean?),
        BuildingValueKind::Logical => NewLiteral::Logical(value.boolean),
        _ => return None,
    };
    Some(NewValue {
        type_name: value.type_name.clone()?,
        literal,
    })
}

impl Model {
    fn id(&self, element: ElementRef) -> String {
        self.building
            .id(element)
            .map(|id| id.to_string())
            .unwrap_or_default()
    }

    fn ids(&self, elements: &[ElementRef]) -> Vec<String> {
        elements.iter().map(|e| self.id(*e)).collect()
    }

    /// 要素の ID と成分のハッシュ。要素の書いた順
    pub(crate) fn hashes(&self, cx: &mut Cx<'_>) -> Result<BTreeMap<String, String>, Failed> {
        let mut hashes = BTreeMap::new();
        for index in 0..self.building.elements().len() {
            let element = ElementRef(u32::try_from(index).unwrap_or(u32::MAX));
            if let Some(hash) = Hash::element_checked(&self.building, element, cx)
                .map_err(|stop| stopped(cx, stop))?
            {
                hashes.insert(self.id(element), hash.to_string());
            }
        }
        Ok(hashes)
    }
}

/// 建物モデルの要約
pub(crate) fn summary(model: &Model) -> BuildingResult {
    let document = &model.building.document;
    let mut classes: BTreeMap<&str, u64> = BTreeMap::new();
    for (_, class) in model.building.components.class.iter() {
        *classes.entry(class.name.as_str()).or_default() += 1;
    }
    BuildingResult {
        schema: String::from_utf8_lossy(model.file.schema()).into_owned(),
        hash: model.hash.to_string(),
        project: document.project.map(|p| model.id(p)),
        project_name: document.project_name.clone(),
        units: document.units.iter().map(unit).collect(),
        elements: model.building.elements().len() as u64,
        classes: classes
            .into_iter()
            .map(|(name, count)| ClassCount {
                name: name.to_string(),
                count,
            })
            .collect(),
    }
}

/// 要素を選び、選んだ成分とともに一部を返す
pub(crate) fn elements(
    model: &Model,
    request: &BuildingElementsRequest,
    cx: &mut Cx<'_>,
) -> Result<BuildingElementsResult, Failed> {
    let components = request.components.as_deref().unwrap_or(&DEFAULT_COMPONENTS);
    for (index, component) in components.iter().enumerate() {
        if components.iter().take(index).any(|c| c == component) {
            return Err(fail(cx, invalid_value(format!("/components/{index}"))));
        }
    }
    if components.is_empty() {
        return Err(fail(cx, invalid_value("/components")));
    }
    let building = &model.building;
    let chosen: Vec<ElementRef> = match &request.ids {
        Some(ids) => ids
            .iter()
            .filter_map(|id| id.parse::<Id>().ok().and_then(|id| building.find(id)))
            .collect(),
        None => (0..building.elements().len())
            .map(|index| ElementRef(u32::try_from(index).unwrap_or(u32::MAX)))
            .collect(),
    };
    let chosen: Vec<ElementRef> = match &request.class {
        Some(name) => chosen
            .into_iter()
            .filter(|e| {
                building
                    .components
                    .class
                    .get(*e)
                    .is_some_and(|class| class.name.eq_ignore_ascii_case(name))
            })
            .collect(),
        None => chosen,
    };
    let relations = if components.contains(&BuildingComponent::Relations) {
        Some(building.relations(cx).map_err(|stop| stopped(cx, stop))?)
    } else {
        None
    };
    let total = chosen.len() as u64;
    let start = request.offset.unwrap_or(0);
    let end = start.saturating_add(request.limit.unwrap_or(LIMIT));
    let mut elements = Vec::new();
    for element in chosen
        .into_iter()
        .skip(usize::try_from(start).unwrap_or(usize::MAX))
        .take(usize::try_from(end - start).unwrap_or(usize::MAX))
    {
        elements.push(
            self::element(model, element, components, relations.as_ref(), cx)
                .map_err(|stop| stopped(cx, stop))?,
        );
    }
    Ok(BuildingElementsResult {
        elements,
        total,
        next_offset: (end < total).then_some(end),
    })
}

fn element(
    model: &Model,
    element: ElementRef,
    components: &[BuildingComponent],
    relations: Option<&Relations>,
    cx: &mut Cx<'_>,
) -> Result<BuildingElement, Stop> {
    let building = &model.building;
    let c = &building.components;
    let has = |component| components.contains(&component);
    let id = |e: &ElementRef| model.id(*e);
    let mut out = BuildingElement {
        id: model.id(element),
        hash: Hash::element_checked(building, element, cx)?
            .map(|hash| hash.to_string())
            .unwrap_or_default(),
        identity: None,
        class: None,
        classification: None,
        spatial: None,
        parent: None,
        placement: None,
        type_of: None,
        properties: None,
        quantities: None,
        material: None,
        opening: None,
        group: None,
        connection: None,
        level: None,
        zone: None,
        style: None,
        relations: None,
    };
    if has(BuildingComponent::Identity) {
        out.identity = c.identity.get(element).map(|i| IdentityComponent {
            name: i.name.clone(),
            description: i.description.clone(),
            tag: i.tag.clone(),
            long_name: i.long_name.clone(),
        });
    }
    if has(BuildingComponent::Class) {
        out.class = c.class.get(element).map(|class| ClassComponent {
            name: class.name.clone(),
            predefined: class.predefined.clone(),
            user_defined: class.user_defined.clone(),
            original: class.original.clone(),
        });
    }
    if has(BuildingComponent::Classification) {
        out.classification = c.classification.get(element).map(|found| {
            found
                .references
                .iter()
                .map(|r| ClassificationReference {
                    system: r.system.clone(),
                    edition: r.edition.clone(),
                    code: r.code.clone(),
                    name: r.name.clone(),
                })
                .collect()
        });
    }
    if has(BuildingComponent::Spatial) {
        out.spatial = c.spatial.get(element).map(|s| SpatialComponent {
            kind: match &s.kind {
                SpatialKind::Site => "site".to_string(),
                SpatialKind::Building => "building".to_string(),
                SpatialKind::Storey => "storey".to_string(),
                SpatialKind::Space => "space".to_string(),
                SpatialKind::SpatialZone => "spatial_zone".to_string(),
                SpatialKind::Other(name) => name.clone(),
            },
            elevation: s.elevation.as_ref().map(measured),
            ref_elevation: s.ref_elevation.as_ref().map(measured),
        });
    }
    if has(BuildingComponent::Parent) {
        out.parent = c.parent.get(element).map(|p| ParentComponent {
            of: p.of.as_ref().map(|m| Membership {
                target: id(&m.target),
                kind: match m.kind {
                    ParentKind::Aggregates => messages::ParentKind::Aggregates,
                    ParentKind::Nests => messages::ParentKind::Nests,
                    ParentKind::Contained => messages::ParentKind::Contained,
                },
                order: m.order.map(u64::from),
            }),
            referenced_in: model.ids(&p.referenced_in),
        });
    }
    if has(BuildingComponent::Placement)
        && let Some(placement) = c.placement.get(element)
    {
        let world = building.world_placement(element, cx)?;
        out.placement = Some(PlacementComponent {
            relative_to: placement.relative_to.as_ref().map(id),
            world: world.map(|t| t.0.iter().map(|row| row.to_vec()).collect()),
            unsupported: match &placement.local {
                Local::Unsupported(name) => Some(name.clone()),
                Local::Transform(_) => None,
            },
        });
    }
    if has(BuildingComponent::TypeOf) {
        out.type_of = c.r#type.get(element).map(|t| id(&t.target));
    }
    if has(BuildingComponent::Properties) {
        out.properties = c.properties.get(element).map(|found| {
            found
                .sets
                .iter()
                .map(|set| PropertySet {
                    name: set.name.clone(),
                    properties: set
                        .properties
                        .iter()
                        .map(|p| Property {
                            name: p.name.clone(),
                            value: p.value.as_ref().map(|v| value(model, v)),
                            unit: p.unit.as_ref().map(unit),
                        })
                        .collect(),
                })
                .collect()
        });
    }
    if has(BuildingComponent::Quantities) {
        out.quantities = c.quantities.get(element).map(|found| {
            found
                .sets
                .iter()
                .map(|set| QuantitySet {
                    name: set.name.clone(),
                    method: set.method.clone(),
                    quantities: set
                        .quantities
                        .iter()
                        .map(|q| Quantity {
                            name: q.name.clone(),
                            kind: match q.kind {
                                QuantityKind::Length => messages::QuantityKind::Length,
                                QuantityKind::Area => messages::QuantityKind::Area,
                                QuantityKind::Volume => messages::QuantityKind::Volume,
                                QuantityKind::Count => messages::QuantityKind::Count,
                                QuantityKind::Weight => messages::QuantityKind::Weight,
                                QuantityKind::Time => messages::QuantityKind::Time,
                            },
                            value: measured(&q.value),
                            formula: q.formula.clone(),
                        })
                        .collect(),
                })
                .collect()
        });
    }
    if has(BuildingComponent::Material) {
        out.material = c.material.get(element).map(|m| MaterialComponent {
            kind: match m.kind {
                MaterialKind::Single => messages::MaterialKind::Single,
                MaterialKind::Layers => messages::MaterialKind::Layers,
                MaterialKind::Profiles => messages::MaterialKind::Profiles,
                MaterialKind::Constituents => messages::MaterialKind::Constituents,
                MaterialKind::List => messages::MaterialKind::List,
            },
            parts: m
                .parts
                .iter()
                .map(|part| MaterialPart {
                    material: part.material.as_ref().map(id),
                    name: part.name.clone(),
                    thickness: part.thickness.as_ref().map(measured),
                })
                .collect(),
            direction: m.direction.clone(),
            offset: m.offset.as_ref().map(measured),
        });
    }
    if has(BuildingComponent::Opening) {
        out.opening = c.opening.get(element).map(|o| match o {
            Opening::Voids(target) => OpeningComponent {
                voids: Some(id(target)),
                fills: None,
            },
            Opening::Fills(target) => OpeningComponent {
                voids: None,
                fills: Some(id(target)),
            },
        });
    }
    if has(BuildingComponent::Group) {
        out.group = c.group.get(element).map(|g| model.ids(&g.groups));
    }
    if has(BuildingComponent::Connection) {
        out.connection = c.connection.get(element).map(|found| {
            found
                .links
                .iter()
                .map(|link| ConnectionLink {
                    kind: link.kind.clone(),
                    target: id(&link.target),
                })
                .collect()
        });
    }
    if has(BuildingComponent::Level) {
        out.level = c.level.get(element).map(|l| LevelComponent {
            name: l.name.clone(),
            kind: l.kind.clone(),
            height: measured(&l.height),
            datum: l.datum.clone(),
        });
    }
    if has(BuildingComponent::Zone) {
        out.zone = c.zone.get(element).map(|z| ZoneComponent {
            kind: z.kind.clone(),
            members: model.ids(&z.members),
        });
    }
    if has(BuildingComponent::Style) {
        out.style = c.style.get(element).map(|s| StyleComponent {
            surface_color: s.surface_color.map(|rgb| rgb.to_vec()),
            transparency: s.transparency,
            curve_color: s.curve_color.map(|rgb| rgb.to_vec()),
            layers: s.layers.clone(),
        });
    }
    if let Some(relations) = relations {
        out.relations = Some(RelationsComponent {
            children: model.ids(relations.children(element)),
            referenced: model.ids(relations.referenced(element)),
            instances: model.ids(relations.instances(element)),
            openings: model.ids(relations.openings(element)),
            fillings: model.ids(relations.fillings(element)),
            members: model.ids(relations.members(element)),
        });
    }
    Ok(out)
}

fn unit(unit: &Unit) -> BuildingUnit {
    BuildingUnit {
        kind: unit.kind.clone(),
        prefix: unit.prefix.clone(),
        name: unit.name.clone(),
        scale: unit.scale,
    }
}

fn measured(value: &zumen_building::Measured) -> Measured {
    Measured {
        value: value.value,
        unit: value.unit.as_ref().map(unit),
    }
}

fn value(model: &Model, value: &TypedValue) -> BuildingValue {
    let mut out = literal(model, &value.value);
    out.type_name = value.type_name.clone();
    out
}

fn literal(model: &Model, literal: &Literal) -> BuildingValue {
    let mut out = BuildingValue {
        type_name: None,
        kind: BuildingValueKind::Real,
        number: None,
        text: None,
        boolean: None,
        element: None,
        items: None,
    };
    match literal {
        Literal::Real(number) => out.number = Some(*number),
        Literal::Integer(number) => {
            out.kind = BuildingValueKind::Integer;
            // 2^53 − 1 を超える整数は、数にすると値が変わるので文字列にする
            if number.unsigned_abs() <= zumen_json::MAX_SAFE_INTEGER as u64 {
                out.number = Some(*number as f64);
            } else {
                out.text = Some(number.to_string());
            }
        }
        Literal::Text(text) => {
            out.kind = BuildingValueKind::Text;
            out.text = Some(text.clone());
        }
        Literal::Boolean(value) => {
            out.kind = BuildingValueKind::Boolean;
            out.boolean = Some(*value);
        }
        Literal::Logical(value) => {
            out.kind = BuildingValueKind::Logical;
            out.boolean = *value;
        }
        Literal::Enumeration(text) => {
            out.kind = BuildingValueKind::Enumeration;
            out.text = Some(text.clone());
        }
        Literal::Element(element) => {
            out.kind = BuildingValueKind::Element;
            out.element = Some(model.id(*element));
        }
        Literal::List(items) => {
            out.kind = BuildingValueKind::List;
            out.items = Some(
                items
                    .iter()
                    .map(|item| self::literal(model, item))
                    .collect(),
            );
        }
    }
    out
}

/// IDS で照合する。`express` を渡せば、IFC の属性の全体を EXPRESS の名前で照らす
pub(crate) fn check(
    model: &Model,
    ids: &[u8],
    express: Option<&[u8]>,
    cx: &mut Cx<'_>,
) -> Result<(Ids, Report), Failed> {
    let ids = Ids::read(ids, cx).map_err(|_| Failed)?;
    let schema = match express {
        Some(bytes) => Some(zumen_ifc::Express::read(bytes, cx).map_err(|_| Failed)?),
        None => None,
    };
    let lookup = zumen_ifc::IdsLookup::new(&model.file, schema.as_ref());
    let report = zumen_ids::check(&model.building, &ids, &lookup, cx).map_err(|_| Failed)?;
    Ok((ids, report))
}

/// 照合の指摘を、境界の形にする。重さは仕様の重さ
pub(crate) fn finding(finding: &zumen_ids::Finding) -> IdsFinding {
    let mut diagnostic = message_of(&finding.diagnostic);
    diagnostic.severity = match finding.level {
        zumen_base::Severity::Info => messages::Severity::Info,
        zumen_base::Severity::Warning => messages::Severity::Warning,
        zumen_base::Severity::Error => messages::Severity::Error,
    };
    IdsFinding {
        specification: finding.specification.clone(),
        diagnostic,
    }
}

/// IDS で照合し、指摘の一部を返す
pub(crate) fn ids_check(
    ids: &Ids,
    report: &Report,
    offset: Option<u64>,
    limit: Option<u64>,
) -> IdsCheckResult {
    let total = report.findings.len() as u64;
    let start = offset.unwrap_or(0);
    let end = start.saturating_add(limit.unwrap_or(LIMIT));
    IdsCheckResult {
        title: ids.title().map(str::to_string),
        specifications: report
            .specifications
            .iter()
            .map(|s| IdsSpecification {
                name: s.name.clone(),
                identifier: s.identifier.clone(),
                status: match s.status {
                    zumen_ids::Status::Passed => IdsStatus::Passed,
                    zumen_ids::Status::Failed => IdsStatus::Failed,
                    zumen_ids::Status::Unknown => IdsStatus::Unknown,
                },
                applicable: s.applicable,
                failed: s.failed,
                unsupported: s.unsupported,
            })
            .collect(),
        findings: report
            .findings
            .iter()
            .skip(usize::try_from(start).unwrap_or(usize::MAX))
            .take(usize::try_from(end - start).unwrap_or(usize::MAX))
            .map(finding)
            .collect(),
        total,
        next_offset: (end < total).then_some(end),
    }
}
