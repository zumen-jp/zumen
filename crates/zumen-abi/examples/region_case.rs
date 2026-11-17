//! 閉領域に使う合成 ZMDR。`cargo run -p zumen-abi --example region_case [--check]`。
#![cfg_attr(test, allow(dead_code))]

use zumen_base::{Key, Ref};
use zumen_drawing::*;

/// 重なる正方形・穴・枝のある線・円・退化・丸め・未対応・範囲外を持つ、自作の有限の図面。
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
    let vertex = |x, y| Vertex {
        point: point(x, y),
        bulge: 0.0,
    };
    let closed = |points: &[(f64, f64)]| {
        Shape::Polyline(Polyline {
            vertices: points.iter().map(|&(x, y)| vertex(x, y)).collect(),
            closed: true,
            width: None,
            vertex_widths: Vec::new(),
            line_type_generation: false,
        })
    };
    let shapes = [
        (
            0x20,
            closed(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)]),
        ),
        (
            0x21,
            closed(&[(5.0, 5.0), (15.0, 5.0), (15.0, 15.0), (5.0, 15.0)]),
        ),
        (0x23, line((0.0, -20.0), (10.0, -20.0))),
        (0x24, line((10.0, -20.0), (10.0, -10.0))),
        (0x25, line((10.0, -10.0), (0.0, -10.0))),
        (0x26, line((0.0, -10.0), (0.0, -20.0))),
        (0x27, line((10.0, -10.0), (14.0, -10.0))),
        (0x28, line((0.0, -20.0), (10.0, -10.0))),
        (
            0x29,
            Shape::Circle {
                center: point(30.0, 0.0),
                radius: length(2.0),
            },
        ),
        (0x2a, closed(&[(0.0, 30.0), (5.0, 30.0), (10.0, 30.0)])),
        (
            0x2b,
            Shape::Leader(Leader {
                path: vec![point(0.0, 0.0), point(1.0, 1.0)],
                spline: false,
                arrow: None,
                annotation: None,
                style: String::new(),
            }),
        ),
        (0x2c, line((0.0, 50.0), (1.0, 51.0))),
        (0x2d, line((0.0, 51.0), (0.999, 50.0))),
        (0x2e, line((0.0, 50.0), (0.0, 51.0))),
        (0x2f, line((1e13, 0.0), (1e13, 1.0))),
        (
            0x30,
            closed(&[(2.0, 2.0), (4.0, 2.0), (4.0, 4.0), (2.0, 4.0)]),
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
            format: "test-regions".to_owned(),
            family: "drawing".to_owned(),
            hash: Hash([0x45; 16]),
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
    let result = include_str!("region_case.json").replace("$HEX", &hex);
    let output = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../bindings/conformance/cases/regions.json");
    if std::env::args().nth(1).as_deref() == Some("--check") {
        let current = std::fs::read_to_string(output).map_err(|e| e.to_string())?;
        if current.replace("\r\n", "\n") != result.replace("\r\n", "\n") {
            return Err("閉領域の適合入力が古い".to_owned());
        }
    } else {
        std::fs::write(output, result).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
fn main() {}
