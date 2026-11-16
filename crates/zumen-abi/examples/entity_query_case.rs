//! 同名のレイヤ・同じ出所・モデルと用紙を持つ、問い合わせの合成入力。
#![cfg_attr(test, allow(dead_code))]

use zumen_base::{Key, Ref};
use zumen_drawing::*;

#[cfg_attr(not(test), allow(dead_code))]
#[path = "entity_details_case.rs"]
mod details;

/// 公開 ID・レイヤ・種類・出所の条件を独立に照らす図面。
pub fn fixture() -> (binary::Source, Drawing2D) {
    let (mut source, mut drawing) = details::fixture();
    source.format = "test-entity-query".into();
    drawing.layers = [10, 20]
        .into_iter()
        .map(|number| Layer {
            id: LayerId(number),
            name: "same".into(),
            state: None,
            style: None,
            follows_placement: false,
            description: None,
            source: Ref::at_record(u64::from(number)).with_key(Key::Handle(0x10)),
        })
        .collect();
    if let Some(space) = drawing.spaces.first_mut() {
        for (index, element) in space.elements.iter_mut().enumerate() {
            element.layer = LayerId(if index == 0 { 10 } else { 20 });
            element.source = Ref::at_record(index as u64)
                .with_bytes(10 + index as u64 * 10, 20 + index as u64 * 10)
                .with_key(if index == 2 {
                    Key::Instance(20)
                } else {
                    Key::Handle(0xA)
                });
            if index == 2 {
                element.shape = Shape::Circle {
                    center: ModelPoint {
                        space: SpaceId(0),
                        x: 0.0,
                        y: 2.0,
                    },
                    radius: Length::Model(ModelLength {
                        space: SpaceId(0),
                        value: 1.0,
                    }),
                };
            }
        }
        space.elements.push(Element {
            source: Ref::default(),
            layer: LayerId(99),
            style: Style::BLOCK,
            shape: Shape::Line {
                start: ModelPoint {
                    space: SpaceId(0),
                    x: 0.0,
                    y: 3.0,
                },
                end: ModelPoint {
                    space: SpaceId(0),
                    x: 10.0,
                    y: 3.0,
                },
            },
        });
        space.order.push(3);
    }
    drawing.sheets.push(Sheet {
        id: SheetId(1),
        name: Some("sheet".into()),
        size: None,
        views: Vec::new(),
        source: Ref::at_record(20).with_key(Key::Handle(0xF0)),
        order: vec![SheetItem::Element(0)],
        elements: vec![Element {
            source: Ref::at_record(10)
                .with_bytes(10, 20)
                .with_key(Key::Handle(0xA)),
            layer: LayerId(10),
            style: Style::BLOCK,
            shape: Shape::Line {
                start: PaperPoint {
                    sheet: SheetId(1),
                    x: 0.0,
                    y: 0.0,
                },
                end: PaperPoint {
                    sheet: SheetId(1),
                    x: 10.0,
                    y: 0.0,
                },
            },
        }],
    });
    (source, drawing)
}

#[cfg(not(test))]
fn main() -> Result<(), String> {
    use zumen_base::{Budget, CancelToken, Cx, Diagnostics};
    let (source, drawing) = fixture();
    let bytes = binary::write(
        &source,
        &drawing,
        &mut Cx::new(
            &mut Budget::default(),
            &mut Diagnostics::new(),
            &CancelToken::new(),
        ),
    )
    .map_err(|e| format!("{e:?}"))?;
    let hex = bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
    let result = include_str!("entity_query_case.json").replace("$HEX", &hex);
    let output = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../bindings/conformance/cases/entity_query.json");
    if std::env::args().nth(1).as_deref() == Some("--check") {
        if std::fs::read_to_string(output)
            .map_err(|e| e.to_string())?
            .replace("\r\n", "\n")
            != result.replace("\r\n", "\n")
        {
            return Err("要素の問い合わせの適合入力が古い".into());
        }
    } else {
        std::fs::write(output, result).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
fn main() {}
