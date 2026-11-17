//! 要素の交わりに使う合成 ZMDR。`cargo run -p zumen-abi --example intersection_case [--check]`。
#![cfg_attr(test, allow(dead_code))]

use zumen_base::{Key, Ref};
use zumen_drawing::*;

/// 交差・接触・重なり・近似・未対応・範囲外と、重複した出所を持つ、自作の有限の図面。
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
    let line = |a: (f64, f64), b: (f64, f64)| Shape::Line {
        start: point(a.0, a.1),
        end: point(b.0, b.1),
    };
    let arc = |start, end| {
        Shape::Curve(Edge::Arc {
            center: point(5.0, 0.0),
            radius: length(3.0),
            start,
            end,
            counterclockwise: true,
        })
    };
    let vertex = |x, y| Vertex {
        point: point(x, y),
        bulge: 0.0,
    };
    let shapes = [
        (0x10, line((0.0, 0.0), (10.0, 0.0))),
        (0x11, line((5.0, -5.0), (5.0, 5.0))),
        (
            0x12,
            Shape::Circle {
                center: point(5.0, 0.0),
                radius: length(3.0),
            },
        ),
        (
            0x13,
            Shape::Circle {
                center: point(11.0, 0.0),
                radius: length(3.0),
            },
        ),
        (0x14, arc(0.0, 180.0)),
        (
            0x15,
            Shape::Polyline(Polyline {
                vertices: vec![vertex(0.0, 0.0), vertex(4.0, 0.0), vertex(4.0, 4.0)],
                closed: false,
                width: None,
                vertex_widths: Vec::new(),
                line_type_generation: false,
            }),
        ),
        (
            0x16,
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
            0x17,
            Shape::Leader(Leader {
                path: vec![point(0.0, 0.0), point(1.0, 1.0)],
                spline: false,
                arrow: None,
                annotation: None,
                style: String::new(),
            }),
        ),
        (0x18, line((0.0, -10.0), (10.0, -10.0))),
        (0x19, line((1e13, 0.0), (1e13, 1.0))),
        (0x1a, arc(180.0, 360.0)),
        (0x10, line((2.0, 0.0), (6.0, 0.0))),
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
            format: "test-intersect".to_owned(),
            family: "drawing".to_owned(),
            hash: Hash([0x44; 16]),
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
    let result = include_str!("intersection_case.json").replace("$HEX", &hex);
    let output = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../bindings/conformance/cases/intersect.json");
    if std::env::args().nth(1).as_deref() == Some("--check") {
        let current = std::fs::read_to_string(output).map_err(|e| e.to_string())?;
        if current.replace("\r\n", "\n") != result.replace("\r\n", "\n") {
            return Err("交わりの適合入力が古い".to_owned());
        }
    } else {
        std::fs::write(output, result).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
fn main() {}
