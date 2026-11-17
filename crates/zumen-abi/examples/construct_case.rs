//! 作図に使う合成 ZMDR。`cargo run -p zumen-abi --example construct_case [--check]`。
#![cfg_attr(test, allow(dead_code))]

use zumen_base::{Key, Ref};
use zumen_drawing::*;

/// 角を共有する線分・境界の線・円・両向きの円弧・楕円・未対応・範囲外を持つ、自作の有限の図面。
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
    let arc = |start, end, counterclockwise| {
        Shape::Curve(Edge::Arc {
            center: point(0.0, 0.0),
            radius: length(5.0),
            start,
            end,
            counterclockwise,
        })
    };
    let shapes = [
        (0x40, line((0.0, 0.0), (10.0, 0.0))),
        (0x41, line((0.0, 0.0), (0.0, 10.0))),
        (0x42, line((5.0, -5.0), (5.0, 5.0))),
        (0x43, line((8.0, -5.0), (8.0, 5.0))),
        (
            0x44,
            Shape::Circle {
                center: point(20.0, 0.0),
                radius: length(3.0),
            },
        ),
        (0x45, arc(0.0, 90.0, true)),
        (0x46, line((-10.0, 3.0), (10.0, 3.0))),
        (0x47, line((20.0, -10.0), (20.0, 10.0))),
        (
            0x48,
            Shape::Leader(Leader {
                path: vec![point(0.0, 0.0), point(1.0, 1.0)],
                spline: false,
                arrow: None,
                annotation: None,
                style: String::new(),
            }),
        ),
        (
            0x49,
            Shape::Curve(Edge::Elliptic {
                center: point(30.0, 0.0),
                major: point(34.0, 0.0),
                ratio: 0.5,
                start: 0.0,
                end: 360.0,
                counterclockwise: true,
            }),
        ),
        (0x4a, line((1e13, 0.0), (1e13, 1.0))),
        (0x4c, line((10.0, 10.0), (20.0, 10.0))),
        (0x4d, line((10.0, 0.0), (20.0, 0.0))),
        (0x4e, arc(90.0, 0.0, false)),
        (0x4f, line((4.0, -10.0), (4.0, -1.0))),
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
            format: "test-construct".to_owned(),
            family: "drawing".to_owned(),
            hash: Hash([0x46; 16]),
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
    let result = include_str!("construct_case.json").replace("$HEX", &hex);
    let output = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../bindings/conformance/cases/construct.json");
    if std::env::args().nth(1).as_deref() == Some("--check") {
        let current = std::fs::read_to_string(output).map_err(|e| e.to_string())?;
        if current.replace("\r\n", "\n") != result.replace("\r\n", "\n") {
            return Err("作図の適合入力が古い".to_owned());
        }
    } else {
        std::fs::write(output, result).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
fn main() {}
