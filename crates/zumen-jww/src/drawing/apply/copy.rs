//! JWW の要素を複写する（`Edit::Copy`、#889）。
//!
//! 元の図形レコードの共通フィールド（`flag` を含む）と文字種・字間などの拡張を保ったまま、変換後の座標で新しい図形レコードを
//! 追加する。曲線属性番号（`curve != 0`）で複数レコードにまたがる要素や、1 レコードから複数要素になったものは、関連が壊れるため
//! `change.copy.unsupported` で変更の一覧全体を拒む。

use std::collections::BTreeSet;

use zumen_base::{Diagnostic, Ref, codes};
use zumen_drawing::{
    Affine, Drawing2D, Element, Field, FieldValue, Hash, Inherit, LayerId, ModelPoint, Seen, Shape,
    SpaceId, Style, SymbolDefinition, Target, Unit, transform,
};
use zumen_sdk::{Cx, Error};

use super::{
    ARC, Base, CIRCLE_SOLID, Expected, FORMAT, Header, Index, LINE, NEAR, POINT, POINT_SYMBOL,
    Plan, SOLID_OWN_COLOR, TEXT, hash, shape_bytes, space_of, text, unturned,
};
use crate::parse::Reader;
use crate::{JwwFile, RecordKind};

/// `Edit::Copy` ごとの配置先と複写元の指定（#889）。`Changes::edits` と同じ添字で対応させる
#[derive(Clone, Copy, Debug, Default)]
pub struct CopyFrom<'a> {
    /// 配置先の空間の ID。`None` なら元の要素と同じ場所
    pub place: Option<&'a str>,
    /// 複写元の文書。`None` なら当てる文書と同じ文書
    pub from: Option<&'a JwwFile>,
}

pub(super) struct SourceJww {
    pub(super) file: JwwFile,
    pub(super) header: Header,
    pub(super) drawing: Drawing2D,
}

#[derive(Clone, Copy, Default)]
pub(super) struct CopySpec<'a> {
    pub(super) place: Option<&'a str>,
    pub(super) from: Option<&'a SourceJww>,
}

type Refusal = (&'static str, &'static str);
pub(super) type ExpectedExt = (String, Vec<Field>);

const BLOCK_REF: &[u8] = b"CDataBlock";
const SOLID: &[u8] = b"CDataSolid";

#[allow(clippy::too_many_arguments)]
pub(super) fn copy_element(
    plan: &mut Plan<'_>,
    drawing: &Drawing2D,
    before: &Index<'_>,
    targets: &BTreeSet<usize>,
    target: &Seen,
    transform: &Affine,
    spec: CopySpec<'_>,
    edit_index: usize,
    cx: &mut Cx,
) -> Result<Option<Expected>, Error> {
    if let Some(source) = spec.from {
        return copy_from_other(
            plan, drawing, before, source, target, transform, spec.place, edit_index, cx,
        );
    }

    let Some(&element) = before.elements.get(&target.id) else {
        cx.report(Diagnostic::new(codes::CHANGE_ELEMENT_MISSING).with("id", target.id.as_str()));
        return Ok(None);
    };
    if hash(element, cx)? != target.hash {
        cx.report(Diagnostic::new(codes::CHANGE_ELEMENT_STALE).with("id", target.id.as_str()));
        return Ok(None);
    }

    let record_num = element
        .source
        .record()
        .and_then(|r| usize::try_from(r).ok());
    let kind_name = record_num
        .map(|r| plan.kind(r))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "element".to_owned());
    let refuse = |cx: &mut Cx, (field, value): Refusal| {
        cx.report(
            Diagnostic::new(codes::CHANGE_COPY_UNSUPPORTED)
                .with("edit", edit_index as u64)
                .with("kind", kind_name.as_str())
                .with("field", field)
                .with("value", value)
                .with("format", FORMAT)
                .at(element.source.clone()),
        );
    };

    let Some(found) = record_num
        .and_then(|r| before.found(r))
        .filter(|found| found.id == target.id)
    else {
        refuse(cx, ("source", "shared"));
        return Ok(None);
    };
    if targets.contains(&found.record) {
        refuse(cx, ("source", "changed"));
        return Ok(None);
    }

    if let Some(place_id) = spec.place {
        let Some(&dst_space) = before.all_spaces.get(place_id) else {
            cx.report(Diagnostic::new(codes::CHANGE_ELEMENT_MISSING).with("id", place_id));
            return Ok(None);
        };
        let src_space = before.element_spaces.get(&target.id).copied();
        if Some(dst_space) != src_space {
            let reason = match (src_space, dst_space) {
                (Some(s), d) if s.0 < 16 && d.0 < 16 => ("layer", "group_mismatch"),
                _ => ("place", "unsupported"),
            };
            refuse(cx, reason);
            return Ok(None);
        }
    }

    let block_def = plan
        .file
        .records()
        .get(found.record)
        .and_then(crate::Record::block);
    match encode_copied(
        plan.file,
        plan.header,
        plan.version(),
        found.record,
        found.element,
        transform,
        cx,
    )? {
        Ok((class, bytes, layer, style, shape, exts)) => {
            if plan.file.classes().is_empty() {
                refuse(cx, ("format", "classes_missing"));
                return Ok(None);
            }
            plan.added.push((block_def, class, bytes));
            let _ = drawing;
            Ok(Some(Expected::Copied(
                plan.added.len() - 1,
                layer,
                style,
                shape,
                exts,
            )))
        }
        Err(reason) => {
            refuse(cx, reason);
            Ok(None)
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn copy_from_other(
    plan: &mut Plan<'_>,
    dst_drawing: &Drawing2D,
    before: &Index<'_>,
    source: &SourceJww,
    target: &Seen,
    transform: &Affine,
    place_id: Option<&str>,
    edit_index: usize,
    cx: &mut Cx,
) -> Result<Option<Expected>, Error> {
    let src_index = Index::new(&source.drawing, cx)?;
    let Some(&element) = src_index.elements.get(&target.id) else {
        cx.report(Diagnostic::new(codes::CHANGE_ELEMENT_MISSING).with("id", target.id.as_str()));
        return Ok(None);
    };
    if hash(element, cx)? != target.hash {
        cx.report(Diagnostic::new(codes::CHANGE_ELEMENT_STALE).with("id", target.id.as_str()));
        return Ok(None);
    }

    let record_num = element
        .source
        .record()
        .and_then(|r| usize::try_from(r).ok());
    let kind_name = record_num
        .and_then(|r| source.file.records().get(r))
        .map(|rec| String::from_utf8_lossy(source.file.kind_name(rec)).into_owned())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "element".to_owned());
    let refuse = |cx: &mut Cx, (field, value): Refusal| {
        cx.report(
            Diagnostic::new(codes::CHANGE_COPY_UNSUPPORTED)
                .with("edit", edit_index as u64)
                .with("kind", kind_name.as_str())
                .with("field", field)
                .with("value", value)
                .with("format", FORMAT)
                .at(element.source.clone()),
        );
    };

    let default_place = target.id.rsplit_once('/').map(|(p, _)| p);
    let Some(place_id) = place_id.or(default_place) else {
        refuse(cx, ("place", "missing"));
        return Ok(None);
    };
    let Some(&dst_space) = before.all_spaces.get(place_id) else {
        cx.report(Diagnostic::new(codes::CHANGE_ELEMENT_MISSING).with("id", place_id));
        return Ok(None);
    };

    let Some(found) = record_num
        .and_then(|r| src_index.found(r))
        .filter(|f| f.id == target.id)
    else {
        refuse(cx, ("source", "shared"));
        return Ok(None);
    };

    let Some(src_space) = src_index.element_spaces.get(&target.id).copied() else {
        refuse(cx, ("place", "unsupported"));
        return Ok(None);
    };
    if src_space.0 >= 16 || dst_space.0 >= 16 {
        refuse(cx, ("place", "unsupported"));
        return Ok(None);
    }
    if src_space != dst_space {
        refuse(cx, ("layer", "group_mismatch"));
        return Ok(None);
    }

    let src_unit = source
        .drawing
        .spaces
        .iter()
        .find(|s| s.id == src_space)
        .map(|s| s.unit)
        .unwrap_or(Unit::Unknown);
    let dst_unit = dst_drawing
        .spaces
        .iter()
        .find(|s| s.id == dst_space)
        .map(|s| s.unit)
        .unwrap_or(Unit::Unknown);
    let group = usize::try_from(src_space.0).unwrap_or(0);
    if src_unit == Unit::Unknown
        || src_unit != dst_unit
        || (source.header.scale(group) - plan.header.scale(group)).abs() > NEAR
    {
        refuse(cx, ("unit", "mismatch"));
        return Ok(None);
    }

    // レイヤ名の衝突を確かめる
    let src_layer = source
        .drawing
        .layers
        .iter()
        .find(|l| l.id == found.element.layer);
    let dst_layer = dst_drawing
        .layers
        .iter()
        .find(|l| l.id == found.element.layer);
    match (src_layer, dst_layer) {
        (Some(sl), Some(dl)) => {
            if !sl.name.is_empty() && !dl.name.is_empty() && sl.name != dl.name {
                refuse(cx, ("layer", "conflict"));
                return Ok(None);
            }
        }
        _ => {
            refuse(cx, ("layer", "missing"));
            return Ok(None);
        }
    }

    // 線種の衝突を確かめる
    if let Inherit::Own(lt_id) = found.element.style.line_type {
        let src_lt = source.drawing.line_types.iter().find(|l| l.id == lt_id);
        let dst_lt = dst_drawing.line_types.iter().find(|l| l.id == lt_id);
        match (src_lt, dst_lt) {
            (Some(s), Some(d)) if s.definition == d.definition => {}
            (Some(_), Some(_)) => {
                refuse(cx, ("line_type", "conflict"));
                return Ok(None);
            }
            _ => {
                refuse(cx, ("line_type", "missing"));
                return Ok(None);
            }
        }
    }

    // ブロック参照なら記号定義の一致を確かめる
    if let Shape::Symbol(sym) = &found.element.shape {
        let src_sym = source.drawing.symbols.iter().find(|s| s.key == sym.key);
        let dst_sym = dst_drawing.symbols.iter().find(|s| s.key == sym.key);
        match (src_sym, dst_sym) {
            (Some(s), Some(d)) => {
                if !same_symbol(s, d, cx, &found.element.source)? {
                    refuse(cx, ("symbol", "conflict"));
                    return Ok(None);
                }
            }
            _ => {
                refuse(cx, ("symbol", "missing"));
                return Ok(None);
            }
        }
    }

    match encode_copied(
        &source.file,
        &source.header,
        plan.version(),
        found.record,
        found.element,
        transform,
        cx,
    )? {
        Ok((class, bytes, layer, style, shape, exts)) => {
            // 貼り付け先のヘッダーで同じスタイルになることを確かめる
            let rec = source.file.records().get(found.record);
            let at = rec.map(|r| r.body().start).unwrap_or(0);
            if let Some(base) = Base::read(&source.file.src, at, source.header.version) {
                if class != BLOCK_REF && class != SOLID {
                    let dst_style = plan.style(&base, class);
                    if dst_style != style {
                        refuse(cx, ("style", "conflict"));
                        return Ok(None);
                    }
                } else if class == SOLID
                    && base.pen_color != SOLID_OWN_COLOR
                    && source.header.color(base.pen_color) != plan.header.color(base.pen_color)
                {
                    refuse(cx, ("style", "conflict"));
                    return Ok(None);
                }
            }
            if plan.file.classes().is_empty() {
                refuse(cx, ("format", "classes_missing"));
                return Ok(None);
            }
            plan.added.push((None, class, bytes));
            Ok(Some(Expected::Copied(
                plan.added.len() - 1,
                layer,
                style,
                shape,
                exts,
            )))
        }
        Err(reason) => {
            refuse(cx, reason);
            Ok(None)
        }
    }
}

fn same_symbol(
    a: &SymbolDefinition,
    b: &SymbolDefinition,
    cx: &mut Cx,
    at: &Ref,
) -> Result<bool, Error> {
    if a.space.elements.len() != b.space.elements.len() || a.base != b.base {
        return Ok(false);
    }
    for (ea, eb) in a.space.elements.iter().zip(&b.space.elements) {
        let ha = Hash::element_checked(ea, cx).map_err(|stop| {
            cx.report(stop.to_diagnostic().at(at.clone()));
            Error::Stopped(stop)
        })?;
        let hb = Hash::element_checked(eb, cx).map_err(|stop| {
            cx.report(stop.to_diagnostic().at(at.clone()));
            Error::Stopped(stop)
        })?;
        if ha != hb {
            return Ok(false);
        }
    }
    Ok(true)
}

#[allow(clippy::type_complexity)]
fn encode_copied(
    src_file: &JwwFile,
    src_header: &Header,
    dst_version: u32,
    record: usize,
    element: &Element<ModelPoint>,
    transform: &Affine,
    cx: &mut Cx,
) -> Result<
    Result<
        (
            &'static [u8],
            Vec<u8>,
            LayerId,
            Style,
            Shape<ModelPoint>,
            Vec<ExpectedExt>,
        ),
        Refusal,
    >,
    Error,
> {
    let Some(rec) = src_file
        .records()
        .get(record)
        .filter(|r| r.kind() == RecordKind::Entity)
    else {
        return Ok(Err(("kind", "unsupported")));
    };
    let name = src_file.kind_name(rec);
    let body = rec.body();
    let Some(base) = Base::read(&src_file.src, body.start, src_header.version) else {
        return Ok(Err(("kind", "unsupported")));
    };
    if base.curve != 0 {
        return Ok(Err(("reference", "curve")));
    }
    if base.pen_width != 0 && dst_version < super::PEN_WIDTH_SINCE {
        return Ok(Err(("style", "line_weight")));
    }
    if ((100..=356).contains(&base.pen_color) || (30..=62).contains(&base.pen_style))
        && dst_version < super::SXF_SINCE
    {
        return Ok(Err(("style", "sxf_version")));
    }

    let in_definition = rec.block().is_some();
    let scale = if in_definition {
        1.0
    } else {
        src_header.scale(usize::from(base.group & 0xF))
    };
    let src_base_len = base.bytes(src_header.version).len();
    let Some(orig_shape_bytes) = src_file.src.get(body.start + src_base_len..body.end) else {
        return Ok(Err(("kind", "unsupported")));
    };

    text::count(&element.shape, cx)?;
    let transformed_opt = transform::transform(&element.shape, transform).map(unturned);
    if transformed_opt.is_none() && name != SOLID {
        return Ok(Err(("transform", "unsupported")));
    }

    let mut out = base.bytes(dst_version);
    let mut exts = Vec::new();
    if base.flag != 0 {
        exts.push((
            "jww.entity".to_owned(),
            vec![
                Field {
                    key: "curve".into(),
                    code: None,
                    value: FieldValue::Integer(0),
                },
                Field {
                    key: "flag".into(),
                    code: None,
                    value: FieldValue::Integer(i64::from(base.flag)),
                },
            ],
        ));
    }

    let (static_class, final_shape) = match name {
        LINE => {
            let Some(transformed_shape) = transformed_opt else {
                return Ok(Err(("transform", "unsupported")));
            };
            let Some(space) = space_of(&transformed_shape) else {
                return Ok(Err(("transform", "unsupported")));
            };
            let Some(geom) = shape_bytes(&transformed_shape, space, scale, false) else {
                return Ok(Err(("transform", "unsupported")));
            };
            out.extend(geom);
            (LINE, transformed_shape)
        }
        ARC => {
            let Some(transformed_shape) = transformed_opt else {
                return Ok(Err(("transform", "unsupported")));
            };
            let Some(space) = space_of(&transformed_shape) else {
                return Ok(Err(("transform", "unsupported")));
            };
            let Some((geom, normalized)) = arc_or_ellipse_bytes(&transformed_shape, space, scale)
            else {
                return Ok(Err(("transform", "unsupported")));
            };
            out.extend(geom);
            (ARC, normalized)
        }
        POINT => {
            let Some(transformed_shape) = transformed_opt else {
                return Ok(Err(("transform", "unsupported")));
            };
            let Some(space) = space_of(&transformed_shape) else {
                return Ok(Err(("transform", "unsupported")));
            };
            let Some(geom) = shape_bytes(
                &transformed_shape,
                space,
                scale,
                base.pen_style == POINT_SYMBOL,
            ) else {
                return Ok(Err(("transform", "unsupported")));
            };
            out.extend(geom);
            (POINT, transformed_shape)
        }
        TEXT => {
            let Some(transformed_shape) = transformed_opt else {
                return Ok(Err(("transform", "unsupported")));
            };
            let Shape::Text(ref txt) = transformed_shape else {
                return Ok(Err(("kind", "unsupported")));
            };
            let Some(space) = space_of(&transformed_shape) else {
                return Ok(Err(("transform", "unsupported")));
            };
            let Some((mut geom, normalized)) =
                text::bytes(txt, space, scale, Some(orig_shape_bytes), Some(transform))
            else {
                return Ok(Err(("transform", "unsupported")));
            };
            let mut r = Reader::new(&geom, 32);
            let (Ok(kind_code), Ok(_), Ok(new_h), Ok(mut spacing)) =
                (r.u32(), r.f64(), r.f64(), r.f64())
            else {
                return Ok(Err(("transform", "unsupported")));
            };
            if let Ok(old_h) = Reader::new(orig_shape_bytes, 44).f64()
                && old_h > 0.0
                && (new_h - old_h).abs() > NEAR
            {
                spacing *= new_h / old_h;
                if !spacing.is_finite() {
                    return Ok(Err(("transform", "unsupported")));
                }
                let Some(slot) = geom.get_mut(52..60) else {
                    return Ok(Err(("transform", "unsupported")));
                };
                slot.copy_from_slice(&spacing.to_le_bytes());
            }
            exts.push((
                "jww.moji".to_owned(),
                vec![
                    Field {
                        key: "kind".into(),
                        code: None,
                        value: FieldValue::Integer(i64::from(kind_code)),
                    },
                    Field {
                        key: "spacing".into(),
                        code: None,
                        value: FieldValue::Real(spacing),
                    },
                ],
            ));
            out.extend(geom);
            (TEXT, Shape::Text(normalized))
        }
        BLOCK_REF => {
            let Some(transformed_shape) = transformed_opt else {
                return Ok(Err(("transform", "unsupported")));
            };
            let Shape::Symbol(ref sym) = transformed_shape else {
                return Ok(Err(("kind", "unsupported")));
            };
            if sym.key.namespace != "jww.block"
                || sym.grid.is_some()
                || !sym.attributes.is_empty()
                || sym.clip.is_some()
                || sym.scale.contains(&0.0)
            {
                return Ok(Err(("kind", "unsupported")));
            }
            let Ok(number) = sym.key.name.parse::<u32>() else {
                return Ok(Err(("kind", "unsupported")));
            };
            if number.to_string() != sym.key.name {
                return Ok(Err(("kind", "unsupported")));
            }
            let values = [
                sym.position.x / scale,
                sym.position.y / scale,
                sym.scale[0] / scale,
                sym.scale[1] / scale,
                sym.rotation.to_radians(),
            ];
            if values.iter().any(|v| !v.is_finite()) {
                return Ok(Err(("transform", "unsupported")));
            }
            out.extend(values.iter().flat_map(|v| v.to_le_bytes()));
            out.extend(number.to_le_bytes());
            (BLOCK_REF, transformed_shape)
        }
        SOLID => {
            if !transform.invertible() {
                return Ok(Err(("transform", "unsupported")));
            }
            if base.pen_style >= CIRCLE_SOLID {
                return Ok(Err(("kind", "circle_solid")));
            }
            let Shape::Hatch(mut hatch) = element.shape.clone() else {
                return Ok(Err(("kind", "unsupported")));
            };
            let Some(zumen_drawing::Boundary::Polyline { vertices, .. }) =
                hatch.loops.first_mut().map(|l| &mut l.boundary)
            else {
                return Ok(Err(("kind", "unsupported")));
            };
            if vertices.len() != 4 {
                return Ok(Err(("kind", "unsupported")));
            }
            let expected_len = if base.pen_color == SOLID_OWN_COLOR {
                68
            } else {
                64
            };
            if orig_shape_bytes.len() < expected_len {
                return Ok(Err(("kind", "unsupported")));
            }
            let mut r = Reader::new(orig_shape_bytes, 0);
            let mut moved_pairs = [[0.0f64; 2]; 4];
            for (pair, vertex) in moved_pairs.iter_mut().zip(vertices.iter_mut()) {
                let (Ok(rx), Ok(ry)) = (r.f64(), r.f64()) else {
                    return Ok(Err(("kind", "unsupported")));
                };
                let (mx, my) = transform.apply(rx * scale, ry * scale);
                let (nx, ny) = (mx / scale, my / scale);
                let (lx, ly) = (nx * scale, ny * scale);
                if !nx.is_finite() || !ny.is_finite() || !lx.is_finite() || !ly.is_finite() {
                    return Ok(Err(("transform", "unsupported")));
                }
                *pair = [nx, ny];
                vertex.point.x = lx;
                vertex.point.y = ly;
            }
            out.extend(moved_pairs.iter().flatten().flat_map(|v| v.to_le_bytes()));
            if base.pen_color == SOLID_OWN_COLOR {
                let Some(color_bytes) = orig_shape_bytes.get(64..68) else {
                    return Ok(Err(("kind", "unsupported")));
                };
                out.extend_from_slice(color_bytes);
            }
            (SOLID, Shape::Hatch(hatch))
        }
        _ => return Ok(Err(("kind", "unsupported"))),
    };

    Ok(Ok((
        static_class,
        out,
        element.layer,
        element.style,
        final_shape,
        exts,
    )))
}

fn arc_or_ellipse_bytes(
    shape: &Shape<ModelPoint>,
    space: SpaceId,
    scale: f64,
) -> Option<(Vec<u8>, Shape<ModelPoint>)> {
    if let Some(bytes) = shape_bytes(shape, space, scale, false) {
        return Some((bytes, shape.clone()));
    }
    let Shape::Curve(zumen_drawing::Edge::Elliptic {
        center,
        major,
        ratio,
        start,
        end,
        counterclockwise,
    }) = shape
    else {
        return None;
    };
    if center.space != space || major.space != space || !(*ratio > 0.0 && *ratio <= 1.0) {
        return None;
    }
    let (cx, cy) = (center.x / scale, center.y / scale);
    let (dx, dy) = ((major.x - center.x) / scale, (major.y - center.y) / scale);
    let radius = libm::hypot(dx, dy);
    if !(radius > 0.0 && radius.is_finite()) {
        return None;
    }
    let tilt = libm::atan2(dy, dx);
    let full = (*end - *start).abs() >= 360.0 - 1e-9;
    let (start_rad, sweep_rad, full_flag, normalized) = if full {
        (
            0.0,
            super::TURN,
            1u32,
            Shape::Curve(zumen_drawing::Edge::Elliptic {
                center: *center,
                major: *major,
                ratio: *ratio,
                start: 0.0,
                end: 360.0,
                counterclockwise: true,
            }),
        )
    } else {
        let sweep_deg = if *counterclockwise {
            (end - start).rem_euclid(360.0)
        } else {
            -(start - end).rem_euclid(360.0)
        };
        if sweep_deg == 0.0 {
            return None;
        }
        (
            start.to_radians(),
            sweep_deg.to_radians(),
            0u32,
            shape.clone(),
        )
    };
    let values = [cx, cy, radius, start_rad, sweep_rad, tilt, *ratio];
    if values.iter().any(|v| !v.is_finite()) {
        return None;
    }
    let mut out: Vec<u8> = values.iter().flat_map(|v| v.to_le_bytes()).collect();
    out.extend(full_flag.to_le_bytes());
    Some((out, normalized))
}

/// 読み直した要素の拡張が、期待した拡張と一致するかを確かめる
pub(super) fn extensions_match(drawing: &Drawing2D, id: &str, expected: &[ExpectedExt]) -> bool {
    let actual: Vec<(&str, &[Field])> = drawing
        .extensions
        .iter()
        .filter(|ext| matches!(&ext.target, Target::Element(eid) if eid == id))
        .map(|ext| (ext.namespace.as_ref(), ext.fields.as_slice()))
        .collect();
    if actual.len() != expected.len() {
        return false;
    }
    actual
        .iter()
        .zip(expected)
        .all(|((ns_a, fields_a), (ns_b, fields_b))| {
            *ns_a == ns_b.as_str()
                && fields_a.len() == fields_b.len()
                && fields_a.iter().zip(fields_b).all(|(fa, fb)| {
                    fa.key == fb.key && fa.code == fb.code && field_near(&fa.value, &fb.value)
                })
        })
}

fn field_near(a: &FieldValue, b: &FieldValue) -> bool {
    match (a, b) {
        (FieldValue::Real(x), FieldValue::Real(y)) => {
            (x - y).abs() <= NEAR * x.abs().max(y.abs()).max(1.0)
        }
        _ => a == b,
    }
}
