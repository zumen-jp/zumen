//! スタイルと拡張を持つ自作の図面モデル。隣の JSON から適合入力を作る。
#![cfg_attr(test, allow(dead_code))]

use zumen_base::{Key, Ref};
use zumen_drawing::*;

/// 継承・固有値と、現在も有効な拡張・失効した拡張を持つ合成の図面。
pub fn fixture() -> (binary::Source, Drawing2D) {
    let own = Style {
        color: Inherit::Own(Color {
            code: Some(ColorCode {
                system: ColorSystem::Aci,
                value: 1,
            }),
            rgb: Some([12, 34, 56]),
        }),
        line_type: Inherit::Own(LineTypeId(8)),
        line_type_scale: 2.5,
        line_weight: Some(Inherit::Own(LineWeight::Width(PaperLength(0.35)))),
        transparency: Inherit::Own(Transparency { alpha: 128 }),
    };
    let styles = [
        own,
        Style {
            color: Inherit::Layer,
            line_type: Inherit::Block,
            line_type_scale: 1.0,
            line_weight: None,
            transparency: Inherit::Layer,
        },
        Style {
            line_weight: Some(Inherit::Own(LineWeight::Default)),
            ..Style::BLOCK
        },
    ];
    let elements: Vec<_> = styles
        .into_iter()
        .enumerate()
        .map(|(index, style)| Element {
            source: Ref::at_record(index as u64).with_key(Key::Handle(if index == 1 {
                1
            } else {
                index as u64 + 1
            })),
            layer: LayerId(0),
            style,
            shape: Shape::Line {
                start: ModelPoint {
                    space: SpaceId(0),
                    x: 0.0,
                    y: index as f64,
                },
                end: ModelPoint {
                    space: SpaceId(0),
                    x: 10.0,
                    y: index as f64,
                },
            },
        })
        .collect();
    let mut drawing = Drawing2D {
        line_types: [(7, "first"), (8, "second")]
            .into_iter()
            .map(|(id, name)| LineType {
                id: LineTypeId(id),
                name: name.to_owned(),
                definition: None,
                source: Ref::at_record(u64::from(id)).with_key(Key::Handle(0x20)),
            })
            .collect(),
        spaces: vec![ModelSpace {
            id: SpaceId(0),
            unit: Unit::Millimeter,
            axes: Axes::EastNorth,
            order: vec![0, 1, 2],
            elements,
        }],
        ..Drawing2D::default()
    };
    let fields = [
        ("same", Some(1000), FieldValue::Text("文字\n値".into())),
        ("same", Some(1071), FieldValue::Integer(i64::MAX)),
        ("min", Some(-32768), FieldValue::Integer(i64::MIN)),
        ("real", None, FieldValue::Real(-0.125)),
        ("point", Some(1010), FieldValue::Point([1.0, 2.0, 3.0])),
        ("handle", Some(1005), FieldValue::Handle(u64::MAX)),
        (
            "bytes",
            Some(1004),
            FieldValue::Bytes(vec![0, 15, 128, 255]),
        ),
        ("empty", None, FieldValue::Bytes(Vec::new())),
    ]
    .into_iter()
    .map(|(key, code, value)| Field {
        key: key.into(),
        code,
        value,
    })
    .collect();
    let first = drawing
        .spaces
        .first()
        .and_then(|space| space.elements.first())
        .map(Hash::element);
    let second = drawing
        .spaces
        .first()
        .and_then(|space| space.elements.get(1))
        .map(Hash::element);
    let extension = |name: &str, target: Target, basis: Basis, fields: Vec<Field>| Extension {
        namespace: "test.values".into(),
        name: name.into(),
        version: Some("1".into()),
        schema: 3,
        target,
        basis,
        fields,
        source: Ref::at_record(17).with_key(Key::Handle(0xFF)),
    };
    drawing.extensions = vec![
        extension(
            "z-first",
            Target::Element("space/0/h1".into()),
            first.map(Basis::Hash).unwrap_or(Basis::Document),
            fields,
        ),
        extension("document", Target::Document, Basis::Document, Vec::new()),
        extension(
            "second",
            Target::Element("space/0/h1/1".into()),
            second.map(Basis::Hash).unwrap_or(Basis::Document),
            Vec::new(),
        ),
        extension(
            "a-stale",
            Target::Element("space/0/h1".into()),
            Basis::Hash(Hash([0; 16])),
            Vec::new(),
        ),
        extension(
            "orphan",
            Target::Element("space/0/missing".into()),
            Basis::Document,
            Vec::new(),
        ),
    ];
    let point = |x| ModelPoint {
        space: SpaceId(1),
        x,
        y: 0.0,
    };
    let symbol_element = Element {
        source: Ref::at_record(30),
        layer: LayerId(0),
        style: own,
        shape: Shape::Line {
            start: point(0.0),
            end: point(1.0),
        },
    };
    drawing.extensions.push(extension(
        "symbol",
        Target::Element("space/h30/r30".into()),
        Basis::Hash(Hash::element(&symbol_element)),
        Vec::new(),
    ));
    drawing.symbols.push(SymbolDefinition {
        key: SymbolKey {
            namespace: "test.symbol".into(),
            name: "part".into(),
        },
        space: ModelSpace {
            id: SpaceId(1),
            unit: Unit::Millimeter,
            axes: Axes::EastNorth,
            elements: vec![symbol_element],
            order: vec![0],
        },
        base: None,
        attributes: Vec::new(),
        anonymous: false,
        external: None,
        source: Ref::at_record(31).with_key(Key::Handle(0x30)),
    });
    (
        binary::Source {
            format: "test-entity-details".into(),
            family: "drawing".into(),
            hash: Hash([0x55; 16]),
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
    let result = include_str!("entity_details_case.json").replace("$HEX", &hex);
    let output = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../bindings/conformance/cases/entity_details.json");
    if std::env::args().nth(1).as_deref() == Some("--check") {
        let current = std::fs::read_to_string(output).map_err(|e| e.to_string())?;
        if current.replace("\r\n", "\n") != result.replace("\r\n", "\n") {
            return Err("要素の属性の適合入力が古い".into());
        }
    } else {
        std::fs::write(output, result).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
fn main() {}
