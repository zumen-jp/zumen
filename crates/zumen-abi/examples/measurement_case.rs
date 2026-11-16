//! 要素の計測に使う合成 ZMDR。`cargo run -p zumen-abi --example measurement_case [--check]`。
#![cfg_attr(test, allow(dead_code))]

use zumen_base::{Key, Ref};
use zumen_drawing::*;

/// 全曲線種、重複した出所と未知単位を持つ、自作の有限の図面。
pub fn fixture() -> (binary::Source, Drawing2D) {
    let point = |x, y| ModelPoint {
        space: SpaceId(0),
        x,
        y,
    };
    let length = |value| {
        Length::Model(ModelLength {
            space: SpaceId(0),
            value,
        })
    };
    let spline = |control: Vec<ModelPoint>, knots: Vec<f64>, degree, closed| Spline {
        degree,
        knots,
        control,
        weights: Vec::new(),
        fit: Vec::new(),
        start_tangent: None,
        end_tangent: None,
        closed,
        periodic: false,
        fit_tolerance: None,
        knot_parameterization: None,
    };
    let mut fit = spline(Vec::new(), Vec::new(), 3, false);
    fit.fit = vec![point(0.0, 0.0), point(5.0, 0.0), point(10.0, 0.0)];
    fit.knot_parameterization = Some(KnotParameterization::SquareRoot);
    let shapes = [
        (
            0x10,
            Shape::Line {
                start: point(0.0, 0.0),
                end: point(3.0, 4.0),
            },
        ),
        (
            0x11,
            Shape::Circle {
                center: point(10.0, 0.0),
                radius: length(2.0),
            },
        ),
        (
            0x12,
            Shape::Curve(Edge::Arc {
                center: point(0.0, 0.0),
                radius: length(5.0),
                start: 0.0,
                end: 90.0,
                counterclockwise: true,
            }),
        ),
        (
            0x13,
            Shape::Polyline(Polyline {
                vertices: vec![
                    Vertex {
                        point: point(0.0, 0.0),
                        bulge: 0.0,
                    },
                    Vertex {
                        point: point(3.0, 0.0),
                        bulge: 0.0,
                    },
                    Vertex {
                        point: point(3.0, 4.0),
                        bulge: 0.0,
                    },
                ],
                closed: true,
                width: Some(length(2.0)),
                vertex_widths: Vec::new(),
                line_type_generation: false,
            }),
        ),
        (
            0x14,
            Shape::Curve(Edge::Elliptic {
                center: point(0.0, 0.0),
                major: point(10.0, 0.0),
                ratio: 0.5,
                start: 0.0,
                end: 360.0,
                counterclockwise: true,
            }),
        ),
        (
            0x15,
            Shape::Curve(Edge::Spline(spline(
                vec![point(0.0, 0.0), point(5.0, 10.0), point(10.0, 0.0)],
                vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
                2,
                false,
            ))),
        ),
        (
            0x16,
            Shape::Curve(Edge::Clothoid {
                origin: point(0.0, 0.0),
                parameter: length(50.0),
                angle: 0.0,
                counterclockwise: true,
                start: length(0.0),
                end: length(20.0),
            }),
        ),
        (0x17, Shape::Curve(Edge::Spline(fit))),
        (
            0x18,
            Shape::Leader(Leader {
                path: vec![point(0.0, 0.0), point(1.0, 1.0)],
                spline: false,
                arrow: None,
                annotation: None,
                style: String::new(),
            }),
        ),
        (
            0x19,
            Shape::Point(Point {
                position: point(-5.0, -5.0),
                rotation: 0.0,
                look: PointLook::Document,
                construction: false,
            }),
        ),
        (
            0x20,
            Shape::Curve(Edge::Spline(spline(
                vec![
                    point(0.0, 0.0),
                    point(10.0, 0.0),
                    point(10.0, 10.0),
                    point(0.0, 10.0),
                    point(0.0, 0.0),
                ],
                vec![0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 4.0],
                1,
                true,
            ))),
        ),
        (
            0x10,
            Shape::Line {
                start: point(0.0, 0.0),
                end: point(7.0, 0.0),
            },
        ),
    ];
    let elements = shapes
        .into_iter()
        .enumerate()
        .map(|(i, (handle, shape))| Element {
            source: Ref::at_record(i as u64).with_key(Key::Handle(handle)),
            layer: LayerId(0),
            style: Style::BLOCK,
            shape,
        })
        .collect::<Vec<_>>();
    let drawing = Drawing2D {
        spaces: vec![
            ModelSpace {
                id: SpaceId(0),
                unit: Unit::Millimeter,
                axes: Axes::EastNorth,
                order: (0..elements.len()).collect(),
                elements,
            },
            ModelSpace {
                id: SpaceId(1),
                unit: Unit::Unknown,
                axes: Axes::EastNorth,
                elements: Vec::new(),
                order: Vec::new(),
            },
        ],
        ..Drawing2D::default()
    };
    (
        binary::Source {
            format: "test-measure".to_owned(),
            family: "drawing".to_owned(),
            hash: Hash([0x33; 16]),
        },
        drawing,
    )
}

#[cfg(not(test))]
fn main() -> Result<(), String> {
    use zumen_base::{Budget, CancelToken, Cx, Diagnostics};
    let (source, drawing) = fixture();
    let mut budget = Budget::default();
    let mut diagnostics = Diagnostics::new();
    let cancel = CancelToken::new();
    let bytes = binary::write(
        &source,
        &drawing,
        &mut Cx::new(&mut budget, &mut diagnostics, &cancel),
    )
    .map_err(|e| format!("{e:?}"))?;
    let hex = bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
    let result = include_str!("measurement_case.json").replace("$HEX", &hex);
    let output = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../bindings/conformance/cases/measure.json");
    if std::env::args().nth(1).as_deref() == Some("--check") {
        let current = std::fs::read_to_string(output).map_err(|e| e.to_string())?;
        if current.replace("\r\n", "\n") != result.replace("\r\n", "\n") {
            return Err("計測の適合入力が古い".to_owned());
        }
    } else {
        std::fs::write(output, result).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
fn main() {}
