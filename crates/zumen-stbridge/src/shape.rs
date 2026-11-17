//! 部材の断面を、建物モデルの断面の押し出しにする（#400）。
//!
//! - 断面の形を書く子の要素（`StbSecFigureColumn_RC`・`StbSecSteelFigureBeam_S` など）が、どこでも同じ形を 1 つだけ
//!   持つときに移す。テーパー・ハンチ・継手ごとに違う形は移さない。
//! - 鉄骨の形は、鋼材の規格の一覧（`StbSecSteel`）の H 形・角形・円形鋼管の寸法で決める。丸み（`r`）は形に入れない。
//! - 位置の決め方は `docs/formats/stbridge.md` の「断面の形」に書く。

use std::collections::BTreeMap;

use zumen_building::{
    AreaProfile, AreaProfileKind, AxisPlacement2, AxisPlacement3, ContextPlacement, Coordinates,
    CurveGraph, CurveItem, CurveKind, CurvePosition, CurveRef, CurveSource, GeometricContext,
    GeometryData, GeometryDefinition, GeometryLinks, GeometryNode, GeometryRef, GeometrySource,
    ProfileExtrusion, RepresentationContext, SubContext, TargetView, Unit,
};

use crate::StbFile;

/// 線の部材の断面の外形。長さは mm
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Outline {
    /// 中心を原点とする矩形。`x` は局所の X、`y` は局所の Y の幅
    Rectangle { x: f64, y: f64 },
    /// 中心を原点とする円
    Circle { radius: f64 },
    /// H 形。`width` はフランジの幅（局所の X）、`depth` はせい（局所の Y）
    IShape {
        width: f64,
        depth: f64,
        web: f64,
        flange: f64,
    },
    /// 角形鋼管。`web` は左右の板（局所の Y に沿う）、`flange` は上下の板の厚さ
    Box {
        width: f64,
        depth: f64,
        web: f64,
        flange: f64,
    },
    /// 円形鋼管
    Pipe { radius: f64, thickness: f64 },
}

impl Outline {
    /// 局所の Y の全体の高さ
    fn height(&self) -> f64 {
        match self {
            Self::Rectangle { y, .. } => *y,
            Self::Circle { radius } | Self::Pipe { radius, .. } => 2.0 * radius,
            Self::IShape { depth, .. } | Self::Box { depth, .. } => *depth,
        }
    }
}

/// 断面の形
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Figure {
    /// 線の部材の断面。`steel` は寸法を引いた鋼材の規格の要素、`rounded` は形に入れなかった丸みがあるか
    Line {
        outline: Outline,
        steel: Option<usize>,
        rounded: bool,
    },
    /// 面の部材の厚さ
    Plate { thickness: f64 },
}

/// 断面の要素から形を探した結果
pub(crate) enum Found {
    /// 形と、形を決めた要素（断面の子から形の要素まで）
    Figure { figure: Figure, used: Vec<usize> },
    /// 鉄骨の形の名前が、鋼材の規格の一覧にない
    Missing { element: usize, name: String },
    /// 移せる形がない
    None,
}

/// 子を 1 つだけ持つラッパーの要素。中を見る
const WRAPPERS: &[&[u8]] = &[b"StbSecSteelBeam_S_Shape"];

/// 鋼材の規格の一覧のうち、寸法を引ける要素
pub(crate) const STEEL: &[&[u8]] = &[
    b"StbSecRoll-H",
    b"StbSecBuild-H",
    b"StbSecRoll-BOX",
    b"StbSecBuild-BOX",
    b"StbSecPipe",
];

fn positive(file: &StbFile, element: usize, attribute: &str) -> Option<f64> {
    file.attribute(element, attribute)
        .and_then(|t| t.trim().parse::<f64>().ok())
        .filter(|v| v.is_finite() && *v > 0.0)
}

/// 鋼材の規格の要素の外形と、丸みの有無
fn steel(file: &StbFile, element: usize) -> Option<(Outline, bool)> {
    let a = positive(file, element, "A");
    let b = positive(file, element, "B");
    let r = file
        .attribute(element, "r")
        .and_then(|t| t.trim().parse::<f64>().ok())
        .is_some_and(|v| v.is_finite() && v != 0.0);
    let outline = match file.name(element) {
        b"StbSecRoll-H" | b"StbSecBuild-H" => Outline::IShape {
            width: b?,
            depth: a?,
            web: positive(file, element, "t1")?,
            flange: positive(file, element, "t2")?,
        },
        b"StbSecRoll-BOX" => {
            let t = positive(file, element, "t")?;
            Outline::Box {
                width: b?,
                depth: a?,
                web: t,
                flange: t,
            }
        }
        b"StbSecBuild-BOX" => Outline::Box {
            width: b?,
            depth: a?,
            web: positive(file, element, "t1")?,
            flange: positive(file, element, "t2")?,
        },
        b"StbSecPipe" => Outline::Pipe {
            radius: positive(file, element, "D")? / 2.0,
            thickness: positive(file, element, "t")?,
        },
        _ => return None,
    };
    let fits = match &outline {
        Outline::IShape {
            width,
            depth,
            web,
            flange,
        } => web < width && 2.0 * flange < *depth,
        Outline::Box {
            width,
            depth,
            web,
            flange,
        } => 2.0 * web < *width && 2.0 * flange < *depth,
        Outline::Pipe { radius, thickness } => thickness < radius,
        Outline::Rectangle { .. } | Outline::Circle { .. } => true,
    };
    fits.then_some((outline, r))
}

/// 形の要素（断面の子の子）の形。鉄骨は鋼材の規格の名前を `shape` で引く
fn leaf(file: &StbFile, element: usize, steels: &BTreeMap<String, usize>) -> Found {
    let used = |figure: Option<Figure>| match figure {
        Some(figure) => Found::Figure {
            figure,
            used: vec![element],
        },
        None => Found::None,
    };
    let line = |outline: Option<Outline>| {
        outline.map(|outline| Figure::Line {
            outline,
            steel: None,
            rounded: false,
        })
    };
    match file.name(element) {
        b"StbSecColumn_RC_Rect" | b"StbSecColumn_SRC_Rect" | b"StbSecColumnRect" => {
            used(line((|| {
                Some(Outline::Rectangle {
                    x: positive(file, element, "width_X")?,
                    y: positive(file, element, "width_Y")?,
                })
            })()))
        }
        b"StbSecColumn_RC_Circle" | b"StbSecColumn_SRC_Circle" | b"StbSecColumnCircle" => used(
            line(positive(file, element, "D").map(|d| Outline::Circle { radius: d / 2.0 })),
        ),
        b"StbSecBeam_RC_Straight" | b"StbSecBeam_SRC_Straight" | b"StbSecBeamStraight" => {
            used(line((|| {
                Some(Outline::Rectangle {
                    x: positive(file, element, "width")?,
                    y: positive(file, element, "depth")?,
                })
            })()))
        }
        b"StbSecWall_RC_Straight" => {
            used(positive(file, element, "t").map(|thickness| Figure::Plate { thickness }))
        }
        b"StbSecSlab_RC_Straight" | b"StbSecSlab_RC_ConventionalStraight" => {
            used(positive(file, element, "depth").map(|thickness| Figure::Plate { thickness }))
        }
        b"StbSecSteelColumn_S_Same"
        | b"StbSecSteelColumn_CFT_Same"
        | b"StbSecSteelBeam_S_Straight"
        | b"StbSecSteelBeamStraight"
        | b"StbSecSteelBrace_S_Same" => {
            let name = file
                .attribute(element, "shape")
                .unwrap_or_default()
                .trim()
                .to_owned();
            let Some(&found) = steels.get(&name) else {
                return Found::Missing { element, name };
            };
            match steel(file, found) {
                Some((outline, rounded)) => Found::Figure {
                    figure: Figure::Line {
                        outline,
                        steel: Some(found),
                        rounded,
                    },
                    used: vec![element],
                },
                None => Found::None,
            }
        }
        _ => Found::None,
    }
}

/// 断面の要素から、形を書く子を探して形にする。鉄骨（`_S`・`_CFT`）は鉄骨の形、ほかはコンクリートの形を使う
pub(crate) fn figure(file: &StbFile, section: usize, steels: &BTreeMap<String, usize>) -> Found {
    let name = file.name(section);
    let steel = name.ends_with(b"_S") || name.ends_with(b"_CFT");
    let holders: Vec<usize> = file
        .children(section)
        .filter(|&child| {
            let child = file.name(child);
            if steel {
                child.starts_with(b"StbSecSteelFigure")
            } else {
                child.starts_with(b"StbSecFigure") || child == b"StbSecSlab_RC_Conventional"
            }
        })
        .collect();
    let [holder] = holders.as_slice() else {
        return Found::None;
    };
    let mut used = vec![*holder];
    let mut at = *holder;
    // 2.1 のスラブは、在来のスラブの子のうち形の子だけを見る
    if file.name(at) == b"StbSecSlab_RC_Conventional" {
        let Some(figure) = file.child(at, b"StbSecFigureSlab_RC_Conventional") else {
            return Found::None;
        };
        used.push(figure);
        at = figure;
    }
    loop {
        let children: Vec<usize> = file.children(at).collect();
        let [only] = children.as_slice() else {
            return Found::None;
        };
        if WRAPPERS.contains(&file.name(*only)) {
            used.push(*only);
            at = *only;
            continue;
        }
        return match leaf(file, *only, steels) {
            Found::Figure {
                figure,
                used: mut tail,
            } => {
                used.append(&mut tail);
                Found::Figure { figure, used }
            }
            other => other,
        };
    }
}

fn millimetre() -> Unit {
    Unit {
        kind: "LENGTHUNIT".to_owned(),
        prefix: Some("MILLI".to_owned()),
        name: "METRE".to_owned(),
        scale: None,
    }
}

fn origin() -> AxisPlacement3 {
    AxisPlacement3 {
        location: [0.0; 3],
        axis: None,
        reference_direction: None,
    }
}

fn node(links: GeometryLinks, definition: GeometryDefinition) -> GeometryNode {
    GeometryNode {
        links,
        definition,
        source: GeometrySource::default(),
    }
}

fn graph(items: Vec<CurveKind>) -> CurveGraph {
    CurveGraph {
        root: CurveRef(0),
        items: items
            .into_iter()
            .map(|kind| CurveItem {
                kind,
                source: CurveSource::default(),
            })
            .collect(),
        length_unit: Some(millimetre()),
        angle_unit: None,
    }
}

/// 閉じた折れ線。最後に最初の点を足す
fn closed(points: &[[f64; 2]]) -> CurveKind {
    let mut out: Vec<Coordinates> = points.iter().copied().map(Coordinates::Two).collect();
    if let Some(first) = points.first() {
        out.push(Coordinates::Two(*first));
    }
    CurveKind::Polyline(out)
}

fn rectangle(x: f64, y: f64) -> CurveKind {
    let (x, y) = (x / 2.0, y / 2.0);
    closed(&[[-x, -y], [x, -y], [x, y], [-x, y]])
}

fn circle(radius: f64) -> CurveKind {
    CurveKind::Circle {
        position: CurvePosition {
            location: Coordinates::Two([0.0; 2]),
            axis: None,
            reference_direction: None,
        },
        radius,
    }
}

/// 外形の断面と、任意の断面の曲線
fn profile(outline: &Outline) -> (AreaProfileKind, Option<CurveGraph>) {
    let voids = |outer: CurveKind, inner: CurveKind| {
        (
            AreaProfileKind::ArbitraryWithVoids {
                boundary: CurveRef(0),
                holes: vec![CurveRef(1)],
            },
            Some(graph(vec![outer, inner])),
        )
    };
    match *outline {
        Outline::Rectangle { x, y } => (AreaProfileKind::Rectangle { x_dim: x, y_dim: y }, None),
        Outline::Circle { radius } => (AreaProfileKind::Circle { radius }, None),
        Outline::IShape {
            width,
            depth,
            web,
            flange,
        } => (
            AreaProfileKind::IShape {
                overall_width: width,
                overall_depth: depth,
                web_thickness: web,
                flange_thickness: flange,
                fillet_radius: None,
                flange_edge_radius: None,
                flange_slope: None,
            },
            None,
        ),
        Outline::Box {
            width,
            depth,
            web,
            flange,
        } if web == flange => (
            AreaProfileKind::RectangleHollow {
                x_dim: width,
                y_dim: depth,
                wall_thickness: web,
                inner_fillet_radius: None,
                outer_fillet_radius: None,
            },
            None,
        ),
        Outline::Box {
            width,
            depth,
            web,
            flange,
        } => voids(
            rectangle(width, depth),
            rectangle(width - 2.0 * web, depth - 2.0 * flange),
        ),
        Outline::Pipe { radius, thickness } => voids(circle(radius), circle(radius - thickness)),
    }
}

/// 押し出し 1 つを、`Body` の表現の形にする
fn data(extrusion: GeometryDefinition, instance: u64) -> GeometryData {
    let nodes = vec![
        node(GeometryLinks::Leaf, extrusion),
        node(
            GeometryLinks::Representation {
                identifier: Some("Body".to_owned()),
                kind: Some("SweptSolid".to_owned()),
                items: vec![GeometryRef(0)],
            },
            GeometryDefinition::Context {
                definition: RepresentationContext {
                    geometric: GeometricContext {
                        identifier: None,
                        kind: Some("Model".to_owned()),
                        dimension: 3,
                        precision: None,
                        world_coordinate_system: ContextPlacement::Space(origin()),
                        true_north: None,
                    },
                    view: Some(SubContext {
                        identifier: Some("Body".to_owned()),
                        kind: None,
                        target_scale: None,
                        target_view: TargetView::Model,
                        user_defined_target_view: None,
                    }),
                },
                length_unit: Some(millimetre()),
            },
        ),
        node(
            GeometryLinks::Definition(vec![GeometryRef(1)]),
            GeometryDefinition::Links,
        ),
    ];
    GeometryData {
        root: Some(GeometryRef(2)),
        nodes,
        styles: Vec::new(),
        grid: None,
        source: GeometrySource {
            instance: Some(instance),
            groups: Vec::new(),
        },
    }
}

/// 線の部材の形。局所の Z（材軸）に `length` だけ押し出す。`top` なら、断面の上端（局所の +Y）を材軸に合わせる
pub(crate) fn line(outline: &Outline, length: f64, top: bool, instance: u64) -> GeometryData {
    let (kind, boundary) = profile(outline);
    let position = top.then(|| AxisPlacement2 {
        location: [0.0, -outline.height() / 2.0],
        reference_direction: None,
    });
    data(
        GeometryDefinition::Extrusion {
            definition: ProfileExtrusion {
                profile: AreaProfile {
                    name: None,
                    position,
                    kind,
                },
                position: None,
                direction: [0.0, 0.0, 1.0],
                depth: length,
            },
            boundary,
            length_unit: Some(millimetre()),
        },
        instance,
    )
}

/// 面の部材の形。局所の XY の多角形を、`from` から局所の Z の向き `sign`（1 か -1）に `thickness` だけ押し出す
pub(crate) fn plate(
    points: &[[f64; 2]],
    thickness: f64,
    from: f64,
    sign: f64,
    instance: u64,
) -> GeometryData {
    data(
        GeometryDefinition::Extrusion {
            definition: ProfileExtrusion {
                profile: AreaProfile {
                    name: None,
                    position: None,
                    kind: AreaProfileKind::ArbitraryClosed {
                        boundary: CurveRef(0),
                    },
                },
                position: Some(AxisPlacement3 {
                    location: [0.0, 0.0, from],
                    axis: None,
                    reference_direction: None,
                }),
                direction: [0.0, 0.0, sign],
                depth: thickness,
            },
            boundary: Some(graph(vec![closed(points)])),
            length_unit: Some(millimetre()),
        },
        instance,
    )
}
