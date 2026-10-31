//! IDS の仕様を、建物モデルの要素ごとに照合する（決定 0217）。
//!
//! facet の結果は、満たす・満たさない・確かめられないの 3 つにする。建物モデルが持たない値（呼び手が
//! [`Lookup::missing`] で示す）に掛かる facet と、決められない値の比べ方は、確かめられないとする。

use std::collections::{BTreeSet, VecDeque};

use zumen_base::{Diagnostic, Kind, Ref, Severity, codes};
use zumen_building::{
    Building, ElementRef, Literal, Measured, Opening, ParentKind, Property, QuantityKind, Unit,
};
use zumen_sdk::{Cx, Error};

use crate::{Cardinality, Constraint, Facet, Ids, Relation, Restriction, Specification, Value};

/// 建物モデルの作り手が持たない値
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Missing {
    /// 分類（`classification` の成分を作っていない）
    Classification,
    /// 材料の分類（`Category`）と、層・断面・構成要素の組の名前
    MaterialNames,
    /// 単一の値でないプロパティ（列挙・範囲・並び・表・複合）
    PropertyKinds,
}

/// 成分にない属性の値
#[derive(Clone, Debug, PartialEq)]
pub enum AttributeValue {
    /// `$` か `*`
    Null,
    /// 値
    Literal(Literal),
    /// 単位を持つ量。単位が解けなければ `unit` は `None`
    Measured(Measured),
    /// ほかのインスタンスへの参照
    Reference,
}

/// 照合の入力の、建物モデルのほかの値。建物モデルだけで照合するなら、[`Building`] を渡す
pub trait Lookup {
    /// 建物モデルが持たない値か
    fn missing(&self, _what: Missing) -> bool {
        false
    }

    /// 要素のすべての明示の属性の名前と値。属性の並びを知らなければ `None`。そのときは、成分にある属性
    /// （`GlobalId`・`Name`・`Description`・`Tag`・`LongName`・`PredefinedType`）だけを確かめる
    fn attributes(
        &self,
        _building: &Building,
        _element: ElementRef,
    ) -> Option<Vec<(String, AttributeValue)>> {
        None
    }
}

/// 建物モデルだけで照合する
impl Lookup for Building {}

/// 仕様ごとの結果
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    /// 指摘なし
    Passed,
    /// 指摘がある
    Failed,
    /// 確かめられなかった項目があり、ほかに指摘がない
    Unknown,
}

/// 仕様ごとの数
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpecificationSummary {
    /// 仕様の名前
    pub name: String,
    /// 識別子
    pub identifier: Option<String>,
    /// 結果
    pub status: Status,
    /// 適用の条件に合った要素の数
    pub applicable: u64,
    /// 要件を満たさない要素の数
    pub failed: u64,
    /// 適用か要件を確かめられなかった要素の数
    pub unsupported: u64,
}

/// 指摘。診断は、要素の ID を場所に持つ
#[derive(Clone, Debug, PartialEq)]
pub struct Finding {
    /// 仕様の名前
    pub specification: String,
    /// 重さ。必須の要件と禁じる要件は error、任意の要件は warning、確かめられないものは info
    pub level: Severity,
    /// 診断
    pub diagnostic: Diagnostic,
}

/// 照合の結果
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Report {
    /// 仕様ごとの数。書いた順
    pub specifications: Vec<SpecificationSummary>,
    /// 指摘。仕様の順に、要素の順
    pub findings: Vec<Finding>,
}

/// 3 つの値
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tri {
    Yes,
    No,
    Unknown,
}

impl Tri {
    fn of(value: bool) -> Self {
        if value { Self::Yes } else { Self::No }
    }

    fn not(self) -> Self {
        match self {
            Self::Yes => Self::No,
            Self::No => Self::Yes,
            Self::Unknown => Self::Unknown,
        }
    }

    fn and(self, other: Self) -> Self {
        match (self, other) {
            (Self::No, _) | (_, Self::No) => Self::No,
            (Self::Unknown, _) | (_, Self::Unknown) => Self::Unknown,
            _ => Self::Yes,
        }
    }

    fn or(self, other: Self) -> Self {
        match (self, other) {
            (Self::Yes, _) | (_, Self::Yes) => Self::Yes,
            (Self::Unknown, _) | (_, Self::Unknown) => Self::Unknown,
            _ => Self::No,
        }
    }

    fn from(value: Option<bool>) -> Self {
        value.map_or(Self::Unknown, Self::of)
    }
}

/// facet が指す値のありさま
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    /// 値がない
    Absent,
    /// 値があり、合うか
    Present(Tri),
    /// 値があるかを決められない
    Unknown,
}

impl State {
    fn result(self, cardinality: Cardinality) -> Tri {
        match (self, cardinality) {
            (Self::Unknown, _) => Tri::Unknown,
            (Self::Absent, Cardinality::Required) => Tri::No,
            (Self::Absent, _) => Tri::Yes,
            (Self::Present(tri), Cardinality::Prohibited) => tri.not(),
            (Self::Present(tri), _) => tri,
        }
    }
}

/// 建物モデルを IDS で照合する。`source` は、建物モデルが持たない値と、成分にない属性を引く
pub fn check(
    building: &Building,
    ids: &Ids,
    source: &dyn Lookup,
    cx: &mut Cx,
) -> Result<Report, Error> {
    let schema = building.document.header.schemas.first().cloned();
    let checker = Checker {
        building,
        source,
        schema: schema.as_deref().unwrap_or("").to_ascii_uppercase(),
    };
    let mut report = Report::default();
    for spec in &ids.specifications {
        checker.specification(spec, &mut report, cx)?;
    }
    Ok(report)
}

struct Checker<'a> {
    building: &'a Building,
    source: &'a dyn Lookup,
    schema: String,
}

/// 要件の 1 つの、要素での結果
struct Outcome {
    facet: &'static str,
    result: Tri,
    cardinality: Cardinality,
}

impl Checker<'_> {
    fn specification(
        &self,
        spec: &Specification,
        report: &mut Report,
        cx: &mut Cx,
    ) -> Result<(), Error> {
        if !self.schema.is_empty()
            && !spec.versions.iter().any(|v| {
                v.eq_ignore_ascii_case(&self.schema)
                    || self.schema.starts_with(&v.to_ascii_uppercase())
            })
        {
            cx.report(
                Diagnostic::new(codes::IDS_VERSION_MIXED)
                    .with("specification", spec.name.as_str())
                    .with("schema", self.schema.as_str()),
            );
        }
        let prohibited = spec.occurs.max == Some(0);
        let required = spec.occurs.min > 0;
        let mut summary = SpecificationSummary {
            name: spec.name.clone(),
            identifier: spec.identifier.clone(),
            status: Status::Passed,
            applicable: 0,
            failed: 0,
            unsupported: 0,
        };
        let mut applicable_unknown = false;
        for (index, _) in self.building.elements().iter().enumerate() {
            let element = ElementRef(index as u32);
            cx.check(Kind::Work, 1).map_err(|stop| {
                cx.report(stop.to_diagnostic());
                Error::Stopped(stop)
            })?;
            if self.building.components.class.get(element).is_none() {
                continue;
            }
            let mut applies = Tri::Yes;
            for facet in &spec.applicability {
                applies = applies.and(
                    self.facet(facet, element, cx)?
                        .0
                        .result(Cardinality::Required),
                );
                if applies == Tri::No {
                    break;
                }
            }
            match applies {
                Tri::No => continue,
                Tri::Unknown => {
                    applicable_unknown = true;
                    summary.unsupported += 1;
                    self.find(
                        report,
                        spec,
                        Severity::Info,
                        Diagnostic::new(codes::IDS_APPLICABILITY_UNSUPPORTED)
                            .with("specification", spec.name.as_str()),
                        element,
                    );
                    continue;
                }
                Tri::Yes => {}
            }
            summary.applicable += 1;
            if prohibited {
                summary.failed += 1;
                self.find(
                    report,
                    spec,
                    Severity::Error,
                    Diagnostic::new(codes::IDS_APPLICABILITY_EXISTS)
                        .with("specification", spec.name.as_str()),
                    element,
                );
                continue;
            }
            let mut failed = false;
            let mut unknown = false;
            for facet in &spec.requirements {
                let (state, name) = self.facet(facet, element, cx)?;
                let outcome = Outcome {
                    facet: name,
                    result: state.result(facet.cardinality()),
                    cardinality: facet.cardinality(),
                };
                match outcome.result {
                    Tri::Yes => {}
                    Tri::No => {
                        failed = true;
                        let level = match outcome.cardinality {
                            Cardinality::Optional => Severity::Warning,
                            _ => Severity::Error,
                        };
                        self.find(
                            report,
                            spec,
                            level,
                            Diagnostic::new(codes::IDS_REQUIREMENT_INVALID)
                                .with("specification", spec.name.as_str())
                                .with("facet", outcome.facet)
                                .with("cardinality", outcome.cardinality.name()),
                            element,
                        );
                    }
                    Tri::Unknown => {
                        unknown = true;
                        self.find(
                            report,
                            spec,
                            Severity::Info,
                            Diagnostic::new(codes::IDS_REQUIREMENT_UNSUPPORTED)
                                .with("specification", spec.name.as_str())
                                .with("facet", outcome.facet),
                            element,
                        );
                    }
                }
            }
            if failed {
                summary.failed += 1;
            } else if unknown {
                summary.unsupported += 1;
            }
        }
        if required && summary.applicable == 0 && !applicable_unknown {
            summary.failed += 1;
            report.findings.push(Finding {
                specification: spec.name.clone(),
                level: Severity::Error,
                diagnostic: Diagnostic::new(codes::IDS_APPLICABILITY_MISSING)
                    .with("specification", spec.name.as_str())
                    .at(Ref::at_bytes(spec.span.start as u64, spec.span.end as u64)),
            });
        }
        summary.status = if summary.failed > 0 {
            Status::Failed
        } else if summary.unsupported > 0 {
            Status::Unknown
        } else {
            Status::Passed
        };
        report.specifications.push(summary);
        Ok(())
    }

    fn find(
        &self,
        report: &mut Report,
        spec: &Specification,
        level: Severity,
        diagnostic: Diagnostic,
        element: ElementRef,
    ) {
        let place = match self.building.id(element) {
            Some(id) => Ref::default().with_id(id.to_string()),
            None => Ref::default(),
        };
        report.findings.push(Finding {
            specification: spec.name.clone(),
            level,
            diagnostic: diagnostic.at(place),
        });
    }

    /// facet の値のありさまと、facet の名前
    fn facet(
        &self,
        facet: &Facet,
        element: ElementRef,
        cx: &mut Cx,
    ) -> Result<(State, &'static str), Error> {
        cx.check(Kind::Work, 1).map_err(|stop| {
            cx.report(stop.to_diagnostic());
            Error::Stopped(stop)
        })?;
        Ok(match facet {
            Facet::Entity { name, predefined } => (
                State::Present(self.entity(name, predefined.as_ref(), element)),
                "entity",
            ),
            Facet::Attribute { name, value, .. } => {
                (self.attribute(name, value.as_ref(), element), "attribute")
            }
            Facet::Classification { value, system, .. } => (
                self.classification(value.as_ref(), system.as_ref(), element),
                "classification",
            ),
            Facet::Property {
                set,
                name,
                data_type,
                value,
                ..
            } => (
                self.property(set, name, data_type.as_deref(), value.as_ref(), element),
                "property",
            ),
            Facet::Material { value, .. } => (self.material(value.as_ref(), element), "material"),
            Facet::PartOf {
                entity, relation, ..
            } => (self.part_of(entity, *relation, element, cx)?, "partOf"),
            Facet::Unsupported { .. } => (State::Unknown, "unsupported"),
        })
    }

    // entity

    fn entity(&self, name: &Value, predefined: Option<&Value>, element: ElementRef) -> Tri {
        let Some(class) = self.building.components.class.get(element) else {
            return Tri::No;
        };
        let class_name = class.original.to_ascii_uppercase();
        let named = text_matches(name, &class_name);
        // IFC2X3 の総称のクラスは、型と組んで IFC4 のクラスに当たる。その対応の表は持たない
        let named = if named == Tri::No
            && self.schema.starts_with("IFC2X3")
            && is_generic_2x3(&class.original)
        {
            Tri::Unknown
        } else {
            named
        };
        let Some(predefined) = predefined else {
            return named;
        };
        let effective = self.predefined(element);
        let literal = class.predefined.as_deref();
        let typed = match (effective.as_deref(), literal) {
            (None, None) => Tri::No,
            (effective, literal) => {
                let by_effective = effective.map_or(Tri::No, |e| text_matches(predefined, e));
                let by_literal = match literal {
                    Some("USERDEFINED") => text_matches(predefined, "USERDEFINED"),
                    _ => Tri::No,
                };
                by_effective.or(by_literal)
            }
        };
        named.and(typed)
    }

    /// 型が決める種類を先に、なければ要素の種類。`USERDEFINED` なら利用者の付けた種類
    fn predefined(&self, element: ElementRef) -> Option<String> {
        let effective = |element: ElementRef| {
            let class = self.building.components.class.get(element)?;
            match class.predefined.as_deref() {
                Some("USERDEFINED") => class.user_defined.clone(),
                Some(other) => Some(other.to_owned()),
                None => None,
            }
        };
        self.building
            .components
            .r#type
            .get(element)
            .and_then(|t| effective(t.target))
            .or_else(|| effective(element))
    }

    // attribute

    fn attribute(&self, name: &Value, value: Option<&Value>, element: ElementRef) -> State {
        let list = match self.source.attributes(self.building, element) {
            Some(list) => list,
            None => {
                let Value::Simple(simple) = name else {
                    return State::Unknown;
                };
                match self.known_attribute(simple, element) {
                    Some(found) => vec![(simple.clone(), found)],
                    None => return State::Unknown,
                }
            }
        };
        // 名前の合う属性のすべてが、値を持って決まりに合う
        let mut state = State::Absent;
        let mut absent = false;
        for (attribute, found) in &list {
            let named = text_matches(name, attribute);
            if named == Tri::No {
                continue;
            }
            let this = match found {
                AttributeValue::Null => State::Absent,
                // 参照と並びの値は、値の決まりに合わない
                AttributeValue::Reference => State::Present(Tri::of(value.is_none())),
                AttributeValue::Literal(Literal::List(_)) if value.is_some() => {
                    State::Present(Tri::No)
                }
                AttributeValue::Literal(literal) => literal_state(value, literal, Some(1.0)),
                AttributeValue::Measured(measured) => {
                    let scale = measured.unit.as_ref().and_then(si_factor);
                    literal_state(value, &Literal::Real(measured.value), scale)
                }
            };
            if named == Tri::Unknown {
                state = combine(state, State::Unknown);
            } else if this == State::Absent {
                absent = true;
            } else {
                state = combine(state, this);
            }
        }
        match state {
            State::Present(_) if absent => State::Present(Tri::No),
            state => state,
        }
    }

    /// 成分にある属性。成分にない名前なら `None`
    fn known_attribute(&self, name: &str, element: ElementRef) -> Option<AttributeValue> {
        let components = &self.building.components;
        let text = |value: Option<&String>| match value {
            Some(text) => AttributeValue::Literal(Literal::Text(text.clone())),
            None => AttributeValue::Null,
        };
        let identity = components.identity.get(element);
        Some(match name {
            "GlobalId" => text(
                self.building
                    .element(element)
                    .and_then(|e| e.source().global_id.as_ref()),
            ),
            "Name" => text(identity.and_then(|i| i.name.as_ref())),
            "Description" => text(identity.and_then(|i| i.description.as_ref())),
            "Tag" => text(identity.and_then(|i| i.tag.as_ref())),
            "LongName" => text(identity.and_then(|i| i.long_name.as_ref())),
            "PredefinedType" => match components
                .class
                .get(element)
                .and_then(|c| c.predefined.as_ref())
            {
                Some(value) => AttributeValue::Literal(Literal::Enumeration(value.clone())),
                None => AttributeValue::Null,
            },
            _ => return None,
        })
    }

    // classification

    fn classification(
        &self,
        value: Option<&Value>,
        system: Option<&Value>,
        element: ElementRef,
    ) -> State {
        if self.source.missing(Missing::Classification) {
            return State::Unknown;
        }
        let components = &self.building.components;
        let own = components.classification.get(element);
        let inherited = components
            .r#type
            .get(element)
            .and_then(|t| components.classification.get(t.target));
        // 要素の参照が、同じ体系の型の参照に代わる
        let mut references: Vec<_> = own
            .map(|c| c.references.iter().collect())
            .unwrap_or_default();
        if let Some(inherited) = inherited {
            for reference in &inherited.references {
                if !references.iter().any(|r| r.system == reference.system) {
                    references.push(reference);
                }
            }
        }
        if references.is_empty() {
            return State::Absent;
        }
        let mut any = Tri::No;
        for reference in references {
            let by_system = match (system, reference.system.as_deref()) {
                (None, _) => Tri::Yes,
                (Some(_), None) => Tri::No,
                (Some(system), Some(name)) => text_matches(system, name),
            };
            // 上位の参照のコードは持たないので、合わなければ決められない
            let by_value = match (value, reference.code.as_deref()) {
                (None, _) => Tri::Yes,
                (Some(_), None) => Tri::Unknown,
                (Some(value), Some(code)) => match text_matches(value, code) {
                    Tri::No => Tri::Unknown,
                    other => other,
                },
            };
            any = any.or(by_system.and(by_value));
        }
        State::Present(any)
    }

    // property

    fn property(
        &self,
        set: &Value,
        name: &Value,
        data_type: Option<&str>,
        value: Option<&Value>,
        element: ElementRef,
    ) -> State {
        let components = &self.building.components;
        let type_of = components.r#type.get(element).map(|t| t.target);
        // セットの名前ごとに、プロパティの名前で要素の値が型の値に代わる
        let mut sets: Vec<(String, Vec<Item>)> = Vec::new();
        for (owner, from_type) in [(Some(element), false), (type_of, true)] {
            let Some(owner) = owner else { continue };
            if let Some(properties) = components.properties.get(owner) {
                for pset in &properties.sets {
                    let Some(items) = set_items(&mut sets, &pset.name) else {
                        continue;
                    };
                    for property in &pset.properties {
                        add_item(items, Item::Property(property), from_type);
                    }
                }
            }
            if let Some(quantities) = components.quantities.get(owner) {
                for qset in &quantities.sets {
                    let Some(items) = set_items(&mut sets, &qset.name) else {
                        continue;
                    };
                    for quantity in &qset.quantities {
                        add_item(
                            items,
                            Item::Quantity(&quantity.name, quantity.kind, &quantity.value),
                            from_type,
                        );
                    }
                }
            }
        }
        let dropped = self.source.missing(Missing::PropertyKinds);
        let mut state = State::Absent;
        let mut set_matched = false;
        for (set_name, items) in &sets {
            let set_named = text_matches(set, set_name);
            if set_named == Tri::No {
                continue;
            }
            set_matched = true;
            let mut found = false;
            for item in items {
                let named = text_matches(name, item.name());
                if named == Tri::No {
                    continue;
                }
                found = true;
                let this = item.state(data_type, value);
                state = combine(
                    state,
                    if named == Tri::Unknown || set_named == Tri::Unknown {
                        State::Unknown
                    } else {
                        this
                    },
                );
            }
            if !found {
                // セットはあるが、プロパティがない
                state = combine(
                    state,
                    if dropped {
                        State::Unknown
                    } else {
                        State::Present(Tri::No)
                    },
                );
            }
        }
        if !set_matched && dropped {
            // 単一の値でないプロパティだけのセットは、建物モデルにない
            return State::Unknown;
        }
        state
    }

    // material

    fn material(&self, value: Option<&Value>, element: ElementRef) -> State {
        let components = &self.building.components;
        let material = components.material.get(element).or_else(|| {
            components
                .r#type
                .get(element)
                .and_then(|t| components.material.get(t.target))
        });
        let Some(material) = material else {
            return State::Absent;
        };
        let Some(value) = value else {
            return State::Present(Tri::Yes);
        };
        let mut any = Tri::No;
        for part in &material.parts {
            if let Some(name) = &part.name {
                any = any.or(text_matches(value, name));
            }
            let named = part
                .material
                .and_then(|m| components.identity.get(m))
                .and_then(|i| i.name.as_deref());
            if let Some(name) = named {
                any = any.or(text_matches(value, name));
            }
        }
        if any == Tri::No && self.source.missing(Missing::MaterialNames) {
            any = Tri::Unknown;
        }
        State::Present(any)
    }

    // partOf

    fn part_of(
        &self,
        entity: &Facet,
        relation: Option<Relation>,
        element: ElementRef,
        cx: &mut Cx,
    ) -> Result<State, Error> {
        let Facet::Entity { name, predefined } = entity else {
            return Ok(State::Unknown);
        };
        let components = &self.building.components;
        let mut seen = BTreeSet::from([element]);
        let mut queue = VecDeque::from([element]);
        let mut any = Tri::No;
        while let Some(part) = queue.pop_front() {
            cx.check(Kind::Work, 1).map_err(|stop| {
                cx.report(stop.to_diagnostic());
                Error::Stopped(stop)
            })?;
            let mut wholes = Vec::new();
            if let Some(membership) = components.parent.get(part).and_then(|p| p.of.as_ref()) {
                let kind = match membership.kind {
                    ParentKind::Aggregates => Relation::Aggregates,
                    ParentKind::Nests => Relation::Nests,
                    ParentKind::Contained => Relation::ContainedInSpatialStructure,
                };
                if relation.is_none_or(|r| r == kind) {
                    wholes.push(membership.target);
                }
            }
            if relation.is_none_or(|r| r == Relation::AssignsToGroup)
                && let Some(group) = components.group.get(part)
            {
                wholes.extend(group.groups.iter().copied());
            }
            if relation.is_none_or(|r| r == Relation::VoidsFills)
                && let Some(Opening::Voids(host) | Opening::Fills(host)) =
                    components.opening.get(part)
            {
                wholes.push(*host);
            }
            for whole in wholes {
                if seen.insert(whole) {
                    any = any.or(self.entity(name, predefined.as_ref(), whole));
                    queue.push_back(whole);
                }
            }
        }
        Ok(State::Present(any))
    }
}

/// プロパティか数量
enum Item<'a> {
    Property(&'a Property),
    Quantity(&'a str, QuantityKind, &'a Measured),
}

impl Item<'_> {
    fn name(&self) -> &str {
        match self {
            Self::Property(property) => &property.name,
            Self::Quantity(name, ..) => name,
        }
    }

    fn state(&self, data_type: Option<&str>, value: Option<&Value>) -> State {
        match self {
            Self::Property(property) => {
                let Some(typed) = &property.value else {
                    return State::Absent;
                };
                let type_name = typed.type_name.as_deref().map(str::to_ascii_uppercase);
                if let Some(data_type) = data_type
                    && type_name.as_deref() != Some(data_type)
                {
                    return State::Present(Tri::No);
                }
                let scale = match (&typed.value, type_name.as_deref()) {
                    (Literal::Real(_) | Literal::Integer(_), Some(name)) if needs_unit(name) => {
                        property.unit.as_ref().and_then(si_factor)
                    }
                    _ => Some(1.0),
                };
                literal_state(value, &typed.value, scale)
            }
            Self::Quantity(_, kind, measured) => {
                if let Some(data_type) = data_type
                    && quantity_type(*kind) != data_type
                {
                    return State::Present(Tri::No);
                }
                let scale = match kind {
                    QuantityKind::Count => Some(1.0),
                    _ => measured.unit.as_ref().and_then(si_factor),
                };
                literal_state(value, &Literal::Real(measured.value), scale)
            }
        }
    }
}

/// セットの名前の項目の列。なければ足す
fn set_items<'s, 'a>(
    sets: &'s mut Vec<(String, Vec<Item<'a>>)>,
    name: &str,
) -> Option<&'s mut Vec<Item<'a>>> {
    if !sets.iter().any(|(n, _)| n == name) {
        sets.push((name.to_owned(), Vec::new()));
    }
    sets.iter_mut()
        .find(|(n, _)| n == name)
        .map(|(_, items)| items)
}

/// 項目を足す。型の項目は、要素に同じ名前の項目があれば足さない
fn add_item<'a>(items: &mut Vec<Item<'a>>, item: Item<'a>, from_type: bool) {
    if !(from_type && items.iter().any(|i| i.name() == item.name())) {
        items.push(item);
    }
}

fn quantity_type(kind: QuantityKind) -> &'static str {
    match kind {
        QuantityKind::Length => "IFCLENGTHMEASURE",
        QuantityKind::Area => "IFCAREAMEASURE",
        QuantityKind::Volume => "IFCVOLUMEMEASURE",
        QuantityKind::Count => "IFCCOUNTMEASURE",
        QuantityKind::Weight => "IFCMASSMEASURE",
        QuantityKind::Time => "IFCTIMEMEASURE",
    }
}

/// 単位を持つ量の型か。比や数は単位を持たない
fn needs_unit(type_name: &str) -> bool {
    type_name.ends_with("MEASURE")
        && !matches!(
            type_name,
            "IFCRATIOMEASURE"
                | "IFCPOSITIVERATIOMEASURE"
                | "IFCNORMALISEDRATIOMEASURE"
                | "IFCCOUNTMEASURE"
                | "IFCNUMERICMEASURE"
                | "IFCCONTEXTDEPENDENTMEASURE"
                | "IFCDESCRIPTIVEMEASURE"
        )
}

/// 文書の単位の値を SI の単位の値にする倍率。温度のように倍率で移せない単位は `None`
fn si_factor(unit: &Unit) -> Option<f64> {
    if let Some(scale) = unit.scale {
        return Some(scale);
    }
    let name = unit.name.as_str();
    if matches!(name, "DEGREE_CELSIUS") {
        return None;
    }
    let power = if name.starts_with("SQUARE_") {
        2
    } else if name.starts_with("CUBIC_") {
        3
    } else {
        1
    };
    let prefix = match unit.prefix.as_deref() {
        None => 1.0,
        Some("EXA") => 1e18,
        Some("PETA") => 1e15,
        Some("TERA") => 1e12,
        Some("GIGA") => 1e9,
        Some("MEGA") => 1e6,
        Some("KILO") => 1e3,
        Some("HECTO") => 1e2,
        Some("DECA") => 1e1,
        Some("DECI") => 1e-1,
        Some("CENTI") => 1e-2,
        Some("MILLI") => 1e-3,
        Some("MICRO") => 1e-6,
        Some("NANO") => 1e-9,
        Some("PICO") => 1e-12,
        Some("FEMTO") => 1e-15,
        Some("ATTO") => 1e-18,
        Some(_) => return None,
    };
    let mut factor = 1.0;
    for _ in 0..power {
        factor *= prefix;
    }
    // 質量の SI の単位はキログラム
    if name == "GRAM" {
        factor *= 1e-3;
    }
    Some(factor)
}

/// IFC2X3 で、型と組んで IFC4 のクラスに当たる総称のクラス
fn is_generic_2x3(class: &str) -> bool {
    [
        "IfcFlow",
        "IfcDistribution",
        "IfcEnergyConversionDevice",
        "IfcFurnishingElement",
    ]
    .iter()
    .any(|prefix| class.starts_with(prefix))
}

/// 2 つのありさまを、すべて満たす向きにまとめる
fn combine(a: State, b: State) -> State {
    match (a, b) {
        (State::Present(Tri::No), _) | (_, State::Present(Tri::No)) => State::Present(Tri::No),
        (State::Unknown, _) | (_, State::Unknown) => State::Unknown,
        (State::Absent, other) | (other, State::Absent) => other,
        (State::Present(x), State::Present(y)) => State::Present(x.and(y)),
    }
}

/// 値の決まりと、建物モデルの値。`scale` は SI の単位への倍率で、決められなければ `None`
fn literal_state(value: Option<&Value>, literal: &Literal, scale: Option<f64>) -> State {
    match literal {
        Literal::Logical(None) => return State::Absent,
        Literal::Text(text) if text.is_empty() => return State::Present(Tri::No),
        Literal::List(items) if items.is_empty() => return State::Present(Tri::No),
        _ => {}
    }
    let Some(value) = value else {
        return State::Present(Tri::Yes);
    };
    State::Present(match literal {
        Literal::List(items) => {
            let bounds = matches!(value, Value::Restriction(r) if r.constraints.iter().any(Constraint::is_bound));
            let mut result = if bounds { Tri::Yes } else { Tri::No };
            for item in items {
                let this = match literal_state(Some(value), item, scale) {
                    State::Present(tri) => tri,
                    State::Absent => Tri::No,
                    State::Unknown => Tri::Unknown,
                };
                result = if bounds {
                    result.and(this)
                } else {
                    result.or(this)
                };
            }
            result
        }
        Literal::Text(text) | Literal::Enumeration(text) => text_matches(value, text),
        Literal::Boolean(b) | Literal::Logical(Some(b)) => boolean_matches(value, *b),
        Literal::Integer(i) => match scale {
            Some(1.0) => integer_matches(value, *i),
            Some(scale) => number_matches(value, *i as f64 * scale),
            None => Tri::Unknown,
        },
        Literal::Real(x) => match scale {
            Some(scale) => number_matches(value, x * scale),
            None => Tri::Unknown,
        },
        Literal::Element(_) | Literal::Logical(None) => Tri::No,
    })
}

impl Constraint {
    fn is_bound(&self) -> bool {
        matches!(
            self,
            Self::MinInclusive(_)
                | Self::MaxInclusive(_)
                | Self::MinExclusive(_)
                | Self::MaxExclusive(_)
        )
    }
}

impl Cardinality {
    fn name(self) -> &'static str {
        match self {
            Self::Required => "required",
            Self::Optional => "optional",
            Self::Prohibited => "prohibited",
        }
    }
}

impl Facet {
    fn cardinality(&self) -> Cardinality {
        match self {
            Self::Entity { .. } => Cardinality::Required,
            Self::Attribute { cardinality, .. }
            | Self::Classification { cardinality, .. }
            | Self::Property { cardinality, .. }
            | Self::Material { cardinality, .. }
            | Self::PartOf { cardinality, .. }
            | Self::Unsupported { cardinality, .. } => *cardinality,
        }
    }
}

/// 制約を、種類ごとにまとめて確かめる。列挙どうし・式どうしはどれか、種類の間はすべて
fn restriction(restriction: &Restriction, check: impl Fn(&Constraint) -> Tri) -> Tri {
    let mut enumeration: Option<Tri> = None;
    let mut pattern: Option<Tri> = None;
    let mut rest = Tri::Yes;
    for constraint in &restriction.constraints {
        let this = check(constraint);
        match constraint {
            Constraint::Enumeration(_) => {
                enumeration = Some(enumeration.map_or(this, |e| e.or(this)))
            }
            Constraint::Pattern(_) => pattern = Some(pattern.map_or(this, |p| p.or(this))),
            _ => rest = rest.and(this),
        }
    }
    rest.and(enumeration.unwrap_or(Tri::Yes))
        .and(pattern.unwrap_or(Tri::Yes))
}

/// 文字の値。大文字と小文字を区別する
fn text_matches(value: &Value, text: &str) -> Tri {
    match value {
        Value::Simple(simple) => Tri::of(simple == text),
        Value::Restriction(r) => restriction(r, |constraint| {
            let length = text.chars().count() as u64;
            match constraint {
                Constraint::Enumeration(e) => Tri::of(e == text),
                Constraint::Pattern(p) => Tri::from(p.matches(text)),
                Constraint::Length(n) => Tri::of(length == *n),
                Constraint::MinLength(n) => Tri::of(length >= *n),
                Constraint::MaxLength(n) => Tri::of(length <= *n),
                Constraint::Unsupported(_) => Tri::Unknown,
                // 文字に範囲は当てない
                _ => Tri::No,
            }
        }),
    }
}

fn boolean_matches(value: &Value, b: bool) -> Tri {
    let text = if b { "true" } else { "false" };
    let parse = |s: &str| match s {
        "true" | "1" => Some(true),
        "false" | "0" => Some(false),
        _ => None,
    };
    match value {
        Value::Simple(simple) => Tri::of(parse(simple) == Some(b)),
        Value::Restriction(r) => restriction(r, |constraint| match constraint {
            Constraint::Enumeration(e) => Tri::of(parse(e) == Some(b)),
            Constraint::Pattern(p) => Tri::from(p.matches(text)),
            Constraint::Unsupported(_) => Tri::Unknown,
            _ => Tri::No,
        }),
    }
}

/// 数の値。等しさは、IDS の値 v に対し `|v|·1e-6 + 1e-6` の幅の内側（端を含まない）。範囲は幅なしで比べる
fn number_matches(value: &Value, x: f64) -> Tri {
    let equal = |text: &str| match parse_double(text) {
        Some(v) => {
            let width = v.abs() * 1e-6 + 1e-6;
            Tri::of(v - width < x && x < v + width)
        }
        None => Tri::No,
    };
    match value {
        Value::Simple(simple) => equal(simple),
        Value::Restriction(r) => restriction(r, |constraint| match constraint {
            Constraint::Enumeration(e) => equal(e),
            Constraint::MinInclusive(v) => Tri::of(x >= *v),
            Constraint::MaxInclusive(v) => Tri::of(x <= *v),
            Constraint::MinExclusive(v) => Tri::of(x > *v),
            Constraint::MaxExclusive(v) => Tri::of(x < *v),
            Constraint::Unsupported(_) => Tri::Unknown,
            // 数に式と長さは当てない
            _ => Tri::No,
        }),
    }
}

fn integer_matches(value: &Value, i: i64) -> Tri {
    let equal = |text: &str| Tri::of(text.trim().parse::<i64>().ok() == Some(i));
    match value {
        Value::Simple(simple) => equal(simple),
        Value::Restriction(r) => restriction(r, |constraint| {
            let x = i as f64;
            match constraint {
                Constraint::Enumeration(e) => equal(e),
                Constraint::MinInclusive(v) => Tri::of(x >= *v),
                Constraint::MaxInclusive(v) => Tri::of(x <= *v),
                Constraint::MinExclusive(v) => Tri::of(x > *v),
                Constraint::MaxExclusive(v) => Tri::of(x < *v),
                Constraint::Unsupported(_) => Tri::Unknown,
                _ => Tri::No,
            }
        }),
    }
}

/// `xs:double` の字句。桁の区切りと小数点のコンマは受けない
fn parse_double(text: &str) -> Option<f64> {
    let text = text.trim();
    let ok = !text.is_empty()
        && text
            .chars()
            .all(|c| c.is_ascii_digit() || matches!(c, '.' | 'e' | 'E' | '+' | '-'));
    if !ok {
        return None;
    }
    text.parse::<f64>().ok().filter(|v| v.is_finite())
}
