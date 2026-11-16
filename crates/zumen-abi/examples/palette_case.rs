//! 色表とレイヤ非表示に使う合成 ZMDR。palette_case.json と合わせて適合入力を作る。
#![cfg_attr(test, allow(dead_code))]

use zumen_base::{Key, Ref};
use zumen_drawing::*;

/// 3 系統の符号、ファイルの RGB、利用者定義色、背景で変わる ACI 7 を分ける。
pub fn fixture() -> (binary::Source, Drawing2D) {
    let colors = [
        (ColorSystem::Aci, 1, None),
        (ColorSystem::Sxf, 4, None),
        (ColorSystem::Jww, 1, None),
        (ColorSystem::Aci, 1, Some([12, 34, 56])),
        (ColorSystem::Sxf, 4, Some([34, 56, 78])),
        (ColorSystem::Jww, 1, Some([56, 78, 90])),
        (ColorSystem::Jww, 200, None),
        (ColorSystem::Aci, 7, None),
        (ColorSystem::Jww, 102, None),
    ];
    let elements = colors
        .into_iter()
        .enumerate()
        .map(|(index, (system, value, rgb))| {
            let y = 10.0 * (index + 1) as f64;
            Element {
                source: Ref::at_record(index as u64).with_key(Key::Handle(index as u64 + 1)),
                layer: LayerId(if index == 6 { 1 } else { 0 }),
                style: Style {
                    color: Inherit::Own(Color {
                        code: Some(ColorCode { system, value }),
                        rgb,
                    }),
                    line_weight: Some(Inherit::Own(LineWeight::Width(PaperLength(2.0)))),
                    ..Style::BLOCK
                },
                shape: Shape::Line {
                    start: ModelPoint {
                        space: SpaceId(0),
                        x: 10.0,
                        y,
                    },
                    end: ModelPoint {
                        space: SpaceId(0),
                        x: 90.0,
                        y,
                    },
                },
            }
        })
        .collect::<Vec<_>>();
    let drawing = Drawing2D {
        layers: ["color", "custom"]
            .into_iter()
            .enumerate()
            .map(|(id, name)| Layer {
                id: LayerId(id as u32),
                name: name.to_owned(),
                state: None,
                style: None,
                follows_placement: false,
                description: None,
                source: Ref::at_record(id as u64 + 20).with_key(Key::Handle(id as u64 + 0x20)),
            })
            .collect(),
        spaces: vec![ModelSpace {
            id: SpaceId(0),
            unit: Unit::Millimeter,
            axes: Axes::EastNorth,
            order: (0..elements.len()).collect(),
            elements,
        }],
        ..Drawing2D::default()
    };
    (
        binary::Source {
            format: "test-palette".to_owned(),
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
    let result = include_str!("palette_case.json").replace("$HEX", &hex);
    let output = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../bindings/conformance/cases/palette.json");
    if std::env::args().nth(1).as_deref() == Some("--check") {
        let current = std::fs::read_to_string(output).map_err(|e| e.to_string())?;
        if current.replace("\r\n", "\n") != result.replace("\r\n", "\n") {
            return Err("色表の適合入力が古い".to_owned());
        }
    } else {
        std::fs::write(output, result).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
fn main() {}
