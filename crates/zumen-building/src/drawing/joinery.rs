//! 建具の Plan の曲線、型付きの開き方とプロファイルの記号、切断の形の順に選ぶ。

use std::collections::BTreeSet;
use zumen_base::{Cx, Diagnostic, Kind, codes};
use zumen_geom2d::topology::Grid;
use zumen_geom3d::{Failure, MeshChanges, Transform, Xyz, add, scale, sub};

use super::{
    Frame, Options, Role, Source, View,
    budget::{push, work},
    output::Stroke,
    reason, report_changes,
};
use crate::{
    Building, ContextPlacement, CurveGraph, ElementRef, GeometryData, GeometryDefinition,
    GeometryLinks, GeometryRef, Id, Literal, TargetView,
    evaluation::{self, curves, values},
};

/// 呼び出し側のプロファイルが渡す、建具の開き方に対応する平面の記号。
///
/// 照合する値は要素か型が持つ型付きの列挙値。名前のない値や NOTDEFINED から推測しない。
/// 曲線は建具の局所の座標で、各 CurveGraph の長さの単位を持つ。
#[derive(Clone, Debug, PartialEq)]
pub struct JoinerySymbol<'a> {
    /// この要素だけに使う場合の ID。None なら同じ開き方の建具に使う。
    pub element: Option<Id>,
    /// 開き方を持つプロパティセットの名前。
    pub set: &'a str,
    /// 開き方を持つプロパティの名前。
    pub property: &'a str,
    /// 列挙の型名。名前だけが同じ別の値を採用しない。
    pub type_name: &'a str,
    /// 開き方の列挙値。
    pub operation: &'a str,
    /// プロファイルが用意した実寸の曲線。幅や開く側をここで推測しない。
    pub curves: &'a [CurveGraph],
}

/// プロパティではない、要素か割り当てた型の拡張属性に対応する記号。
/// 値の選択・優先順位・寸法はプロファイルが決め、ここでは参照と型付きの値の一致を確かめる。
#[derive(Clone, Debug, PartialEq)]
pub struct AttributeSymbol<'a> {
    /// 描く建具の ID。
    pub element: Id,
    /// 属性を持つ要素。建具そのものか、その type の参照先に限る。
    pub owner: ElementRef,
    /// 拡張属性の名前空間。
    pub namespace: &'a str,
    /// 属性の名前。
    pub attribute: &'a str,
    /// 列挙の型名。
    pub type_name: &'a str,
    /// 明記された開き方。空と NOTDEFINED は使わない。
    pub operation: &'a str,
    /// 建具の局所の座標で用意した実寸の曲線。
    pub curves: &'a [CurveGraph],
}

struct Rules<'a> {
    properties: &'a [JoinerySymbol<'a>],
    attributes: &'a [AttributeSymbol<'a>],
}

#[derive(Default)]
pub(super) struct Prepared {
    pub replaced: BTreeSet<ElementRef>,
    pub skipped: BTreeSet<ElementRef>,
    pub sources: Vec<Source>,
    pub strokes: Vec<Stroke>,
    pub complete: bool,
}

#[derive(Clone, Copy)]
pub(super) struct Settings {
    pub frame: Frame,
    pub grid: Grid,
    pub units: f64,
    pub chord: f64,
}

pub(super) fn world(
    building: &Building,
    at: ElementRef,
    units: f64,
    cx: &mut Cx<'_>,
) -> Result<Transform, Failure> {
    let value = match building.world_placement(at, cx)? {
        Some(value) => value,
        None if building.components.placement.get(at).is_none() => crate::Transform::IDENTITY,
        None => return Err(Failure::Invalid("element_placement")),
    };
    let mut transform = Transform(value.0);
    for row in &mut transform.0 {
        row[3] *= units;
    }
    if !transform.is_finite() || transform.determinant() == 0.0 {
        return Err(Failure::Invalid("element_placement"));
    }
    Ok(transform)
}

pub(super) fn is_joinery(building: &Building, at: ElementRef) -> bool {
    building
        .components
        .class
        .get(at)
        .is_some_and(|class| matches!(class.name.as_str(), "IfcDoor" | "IfcWindow"))
}

pub(super) fn prepare(
    building: &Building,
    options: Options<'_>,
    attributes: &[AttributeSymbol<'_>],
    frame: Frame,
    grid: Grid,
    units: f64,
    cx: &mut Cx<'_>,
) -> Result<Prepared, Failure> {
    let mut result = Prepared {
        complete: true,
        ..Prepared::default()
    };
    if !matches!(options.view, View::Plan { .. }) {
        return Ok(result);
    }
    let settings = Settings {
        frame,
        grid,
        units,
        chord: options.chord * units,
    };
    for index in 0..building.elements().len() {
        work(cx, 1)?;
        let at = ElementRef(index as u32);
        if !is_joinery(building, at) {
            continue;
        }
        let original_sources = result.sources.len();
        let original_strokes = result.strokes.len();
        let outcome = plan(building, at, settings, &mut result, cx);
        if let Err(error) = outcome {
            if matches!(error, Failure::Stopped(_)) {
                return Err(error);
            }
            result.sources.truncate(original_sources);
            result.strokes.truncate(original_strokes);
            result.complete = false;
            cx.report(
                Diagnostic::new(codes::BUILDING_DRAWING_DROPPED)
                    .with("reason", reason(&error))
                    .at(building.place(at)),
            );
            work(cx, 0)?;
        }
        if result.strokes.len() == original_strokes {
            let symbols = symbol(
                building,
                at,
                Rules {
                    properties: options.symbols,
                    attributes,
                },
                settings,
                &mut result,
                cx,
            );
            if let Err(error) = symbols {
                if matches!(error, Failure::Stopped(_)) {
                    return Err(error);
                }
                result.sources.truncate(original_sources);
                result.strokes.truncate(original_strokes);
                result.complete = false;
                cx.report(
                    Diagnostic::new(codes::BUILDING_DRAWING_DROPPED)
                        .with("reason", reason(&error))
                        .at(building.place(at)),
                );
                work(cx, 0)?;
            }
        }
        if result.strokes.len() != original_strokes {
            cx.check(Kind::DecodedBytes, 64)?;
            result.replaced.insert(at);
            // 記号に置き換えても、実体があればほかの辺を隠す面として使う。
            if !has_body(building, at, cx)? {
                cx.check(Kind::DecodedBytes, 64)?;
                result.skipped.insert(at);
            }
        }
    }
    Ok(result)
}

fn has_body(building: &Building, at: ElementRef, cx: &mut Cx<'_>) -> Result<bool, Failure> {
    let Some(data) = building.geometry(at).map(|g| g.data()) else {
        return Ok(false);
    };
    let Some(root) = data.root else {
        return Ok(false);
    };
    let node = data
        .nodes
        .get(root.0 as usize)
        .ok_or(Failure::Invalid("geometry_reference"))?;
    let representations = match &node.links {
        GeometryLinks::Definition(items) => items.as_slice(),
        _ => std::slice::from_ref(&root),
    };
    work(cx, representations.len() as u64)?;
    Ok(representations.iter().any(|r| data.nodes.get(r.0 as usize).is_some_and(|node| {
        matches!(&node.links, GeometryLinks::Representation { identifier: Some(name), .. } if name == "Body")
    })))
}

fn plan(
    building: &Building,
    at: ElementRef,
    settings: Settings,
    out: &mut Prepared,
    cx: &mut Cx<'_>,
) -> Result<(), Failure> {
    let Some(geometry) = building.geometry(at) else {
        return Ok(());
    };
    let data = geometry.data();
    let Some(root) = data.root else {
        return Ok(());
    };
    let node = data
        .nodes
        .get(root.0 as usize)
        .ok_or(Failure::Invalid("geometry_reference"))?;
    let representations = match &node.links {
        GeometryLinks::Definition(items) => items.as_slice(),
        _ => std::slice::from_ref(&root),
    };
    for &representation in representations {
        work(cx, 1)?;
        let node = data
            .nodes
            .get(representation.0 as usize)
            .ok_or(Failure::Invalid("geometry_reference"))?;
        let GeometryDefinition::Context {
            definition,
            length_unit,
        } = &node.definition
        else {
            continue;
        };
        let plan = definition.geometric.kind.as_deref() == Some("Plan")
            || definition
                .view
                .as_ref()
                .is_some_and(|v| v.target_view == TargetView::Plan);
        if !plan {
            continue;
        }
        let factor = evaluation::length_scale(
            length_unit
                .as_ref()
                .ok_or(Failure::Invalid("unit_missing"))?,
        )?;
        let context = match &definition.geometric.world_coordinate_system {
            ContextPlacement::Space(value) => values::placement(Some(value), factor)?,
            ContextPlacement::Plane(value) => Transform::from_axes(
                [value.location[0] * factor, value.location[1] * factor, 0.0],
                Some([0.0, 0.0, 1.0]),
                value.reference_direction.map(|[x, y]| [x, y, 0.0]),
            )
            .ok_or(Failure::Invalid("plan_context"))?,
        };
        let placement = world(building, at, settings.units, cx)?.then(&context);
        Walker {
            building,
            element: at,
            data,
            settings,
            out,
        }
        .node(representation, placement, cx)?;
    }
    Ok(())
}

struct Walker<'a, 'b> {
    building: &'a Building,
    element: ElementRef,
    data: &'a GeometryData,
    settings: Settings,
    out: &'b mut Prepared,
}
impl Walker<'_, '_> {
    fn node(
        &mut self,
        at: GeometryRef,
        placement: Transform,
        cx: &mut Cx<'_>,
    ) -> Result<(), Failure> {
        cx.enter()?;
        let result = self.inner(at, placement, cx);
        cx.leave();
        result
    }
    fn inner(
        &mut self,
        at: GeometryRef,
        placement: Transform,
        cx: &mut Cx<'_>,
    ) -> Result<(), Failure> {
        work(cx, 1)?;
        let node = self
            .data
            .nodes
            .get(at.0 as usize)
            .ok_or(Failure::Invalid("geometry_reference"))?;
        match &node.links {
            GeometryLinks::Definition(items)
            | GeometryLinks::Representation { items, .. }
            | GeometryLinks::Set { items, .. } => {
                for &item in items {
                    self.node(item, placement, cx)?;
                }
            }
            GeometryLinks::Map(child) => self.node(*child, placement, cx)?,
            GeometryLinks::Mapped(child) => {
                let GeometryDefinition::Mapping {
                    definition,
                    length_unit,
                } = &node.definition
                else {
                    return Err(Failure::Invalid("mapping_definition"));
                };
                let factor = evaluation::length_scale(
                    length_unit
                        .as_ref()
                        .ok_or(Failure::Invalid("unit_missing"))?,
                )?;
                let mapping = values::mapping(definition, factor)?;
                self.node(*child, mapping.then(&placement), cx)?;
            }
            GeometryLinks::Leaf => {
                let GeometryDefinition::Curve(graph) = &node.definition else {
                    return Err(Failure::Unsupported("plan_item"));
                };
                let source = super::source(self.building, self.element, Some(at), cx)?;
                draw_curve(
                    graph,
                    placement,
                    source,
                    Role::Annotation,
                    self.settings,
                    self.out,
                    cx,
                )?;
            }
            GeometryLinks::Boolean { .. } => return Err(Failure::Unsupported("plan_boolean")),
        }
        Ok(())
    }
}

fn symbol(
    building: &Building,
    at: ElementRef,
    rules: Rules<'_>,
    settings: Settings,
    out: &mut Prepared,
    cx: &mut Cx<'_>,
) -> Result<(), Failure> {
    let mut selected = None;
    for rule in rules.properties {
        work(cx, 1)?;
        if rule.element.is_some_and(|id| Some(id) != building.id(at))
            || rule.operation.is_empty()
            || rule.operation == "NOTDEFINED"
        {
            continue;
        }
        let mut values = Vec::new();
        for owner in [
            Some(at),
            building.components.r#type.get(at).map(|t| t.target),
        ]
        .into_iter()
        .flatten()
        {
            if let Some(properties) = building.components.properties.get(owner) {
                for set in &properties.sets {
                    work(cx, 1)?;
                    if set.name != rule.set {
                        continue;
                    }
                    for property in &set.properties {
                        work(cx, 1)?;
                        if property.name == rule.property {
                            let value = property
                                .value
                                .as_ref()
                                .filter(|v| v.type_name.as_deref() == Some(rule.type_name));
                            push(&mut values, value.map(|v| &v.value), 8, cx)?;
                        }
                    }
                }
            }
            // 要素が持つ値が優先。型で不明・不正な指定を上書きしない。
            if !values.is_empty() {
                break;
            }
        }
        if !values.is_empty()
            && values
                .iter()
                .all(|v| matches!(v, Some(Literal::Enumeration(value)) if value == rule.operation))
            && selected.replace(rule.curves).is_some()
        {
            return Err(Failure::Invalid("joinery_symbol_ambiguous"));
        }
    }
    for rule in rules.attributes {
        work(cx, 1)?;
        if Some(rule.element) != building.id(at)
            || rule.operation.is_empty()
            || rule.operation == "NOTDEFINED"
        {
            continue;
        }
        if rule.owner != at
            && building.components.r#type.get(at).map(|v| v.target) != Some(rule.owner)
        {
            return Err(Failure::Invalid("joinery_symbol_owner"));
        }
        let mut count = 0;
        let mut matches = true;
        if let Some(extension) = building.components.extension.get(rule.owner) {
            for attribute in &extension.values {
                work(cx, 1)?;
                if attribute.namespace == rule.namespace && attribute.name == rule.attribute {
                    count += 1;
                    matches &= attribute.value.type_name.as_deref() == Some(rule.type_name)
                        && matches!(&attribute.value.value, Literal::Enumeration(v) if v == rule.operation);
                }
            }
        }
        if count == 1 && matches && selected.replace(rule.curves).is_some() {
            return Err(Failure::Invalid("joinery_symbol_ambiguous"));
        }
    }
    if let Some(curves) = selected {
        let placement = world(building, at, settings.units, cx)?;
        for graph in curves {
            let source = super::source(building, at, None, cx)?;
            draw_curve(
                graph,
                placement,
                source,
                Role::Annotation,
                settings,
                out,
                cx,
            )?;
        }
    }
    Ok(())
}

pub(super) fn draw_curve(
    graph: &CurveGraph,
    placement: Transform,
    source: Source,
    role: Role,
    settings: Settings,
    out: &mut Prepared,
    cx: &mut Cx<'_>,
) -> Result<(), Failure> {
    let mut changes = MeshChanges::default();
    let points = curves::evaluate(graph, graph.root, settings.chord, &mut changes, cx)?;
    changes.report(
        Some(&source.place()),
        zumen_geom3d::Tolerance {
            chord: settings.chord,
            length: settings.grid.tolerance() * settings.units,
        },
        cx,
    )?;
    let at = out.sources.len();
    let origin = settings.frame.origin();
    let offset = sub(
        placement.column(3),
        scale([origin.x, origin.y, origin.z], settings.units),
    );
    let project = |p: Xyz| {
        settings.frame.project_offset(scale(
            add(placement.vector(p), offset),
            1.0 / settings.units,
        ))
    };
    let (mut snapped, mut collapsed) = (0, 0);
    for pair in points.windows(2) {
        work(cx, 1)?;
        let [a, b] = pair else {
            return Err(Failure::Invalid("drawing_curve"));
        };
        let a = project(*a)?;
        let b = project(*b)?;
        let Some([a, b]) = super::project::clip_edge([a, b], settings.frame.depth())? else {
            continue;
        };
        let [Ok(a), Ok(b)] = [
            settings.grid.snap([a[0], a[1]]),
            settings.grid.snap([b[0], b[1]]),
        ] else {
            return Err(Failure::Invalid("drawing_grid"));
        };
        snapped += u64::from(a.offset != [0.0, 0.0]) + u64::from(b.offset != [0.0, 0.0]);
        if a.point == b.point {
            collapsed += 1;
            continue;
        }
        let [Ok(a), Ok(b)] = [
            settings.grid.position(a.point),
            settings.grid.position(b.point),
        ] else {
            return Err(Failure::Invalid("drawing_grid"));
        };
        cx.check(Kind::Segments, 1)?;
        push(
            &mut out.strokes,
            Stroke {
                ends: [a, b],
                source: at,
                role,
            },
            48,
            cx,
        )?;
    }
    report_changes(&source, settings.grid.spacing(), snapped, collapsed, cx)?;
    push(&mut out.sources, source, 112, cx)?;
    Ok(())
}
