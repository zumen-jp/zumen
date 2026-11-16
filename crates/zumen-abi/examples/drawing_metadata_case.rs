//! 全種類・属性定義・回転と反転とせん断を持つ自作図面。
#![cfg_attr(test, allow(dead_code))]

use zumen_base::{Key, Ref};
use zumen_drawing::*;

/// 全種類を 1 つずつ置き、属性定義と既知・未知の単位のビューを持つ図面。
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
    let text = Text {
        content: "既定値".into(),
        height: Height {
            value: length(1.0),
            meaning: HeightMeaning::Frame,
        },
        placement: Placement::Anchor {
            point: point(1.0, 2.0),
            horizontal: Horizontal::Left,
            vertical: Vertical::Baseline,
        },
        rotation: 0.0,
        width_factor: 1.0,
        oblique: 0.0,
        backward: false,
        upside_down: false,
        style: String::new(),
    };
    let formatted = FormattedText {
        content: "書式\\Pつき".into(),
        spans: Vec::new(),
        height: text.height.clone(),
        point: point(1.0, 2.0),
        horizontal: Horizontal::Left,
        vertical: Vertical::Top,
        rotation: Rotation::Angle(0.0),
        width: None,
        frame_height: None,
        flow: Flow::LeftToRight,
        line_spacing: LineSpacing {
            factor: 1.0,
            exact: false,
        },
        background: None,
        style: String::new(),
    };
    let symbol = Symbol {
        key: SymbolKey {
            namespace: "dxf.block".into(),
            name: "part".into(),
        },
        position: point(1.0, 2.0),
        scale: [1.0; 2],
        rotation: 0.0,
        grid: None,
        attributes: Vec::new(),
        clip: None,
    };
    let shapes = vec![
        Shape::Line {
            start: point(0.0, 0.0),
            end: point(2.0, 3.0),
        },
        Shape::Text(text.clone()),
        Shape::Circle {
            center: point(0.0, 0.0),
            radius: length(1.0),
        },
        Shape::Curve(Edge::Arc {
            center: point(0.0, 0.0),
            radius: length(1.0),
            start: 0.0,
            end: 90.0,
            counterclockwise: true,
        }),
        Shape::Polyline(Polyline {
            vertices: vec![
                Vertex {
                    point: point(0.0, 0.0),
                    bulge: 0.0,
                },
                Vertex {
                    point: point(1.0, 1.0),
                    bulge: 0.0,
                },
            ],
            closed: false,
            width: None,
            vertex_widths: Vec::new(),
            line_type_generation: false,
        }),
        Shape::Formatted(formatted.clone()),
        Shape::Dimension(Dimension {
            kind: DimensionKind::Aligned {
                first: point(0.0, 0.0),
                second: point(2.0, 0.0),
                line: point(0.0, 1.0),
                oblique: 0.0,
            },
            measurement: None,
            label: Label::Measurement,
            text: DimensionText {
                position: None,
                moved: false,
                rotation: None,
                attachment: None,
            },
            style: String::new(),
            block: None,
        }),
        Shape::Leader(Leader {
            path: vec![point(0.0, 0.0), point(1.0, 1.0)],
            spline: false,
            arrow: None,
            annotation: None,
            style: String::new(),
        }),
        Shape::Callout(Callout {
            branches: Vec::new(),
            content: CalloutContent::None,
            path: PathKind::Straight,
            arrowhead: Arrowhead::Default,
            arrow_size: length(1.0),
            landing: false,
            scale: 1.0,
            line_style: Style::BLOCK,
            content_color: Inherit::Block,
            style: None,
        }),
        Shape::Table(Table {
            position: point(0.0, 0.0),
            direction: [1.0, 0.0],
            row_heights: Vec::new(),
            column_widths: Vec::new(),
            cells: Vec::new(),
            style: None,
            formats: None,
            drawn: None,
        }),
        Shape::Underlay(Underlay {
            kind: UnderlayKind::Pdf,
            resource: Resource {
                path: "unopened.pdf".into(),
                definition: Ref::default(),
            },
            page: None,
            frame: UnderlayFrame::Corners {
                origin: point(0.0, 0.0),
                right: point(1.0, 0.0),
                top: point(0.0, 1.0),
            },
            clip: None,
            visible: true,
        }),
        Shape::Symbol(symbol),
        Shape::Hatch(Hatch {
            loops: Vec::new(),
            islands: Islands::Nested,
            fill: Fill::Solid,
            associative: false,
            seeds: Vec::new(),
        }),
        Shape::Point(Point {
            position: point(0.0, 0.0),
            rotation: 0.0,
            look: PointLook::Document,
            construction: false,
        }),
    ];
    let elements: Vec<_> = shapes
        .into_iter()
        .enumerate()
        .map(|(index, shape)| Element {
            layer: LayerId(0),
            style: Style::BLOCK,
            shape,
            source: Ref::at_record(index as u64).with_key(Key::Handle(index as u64 + 1)),
        })
        .collect();
    let mut drawing = Drawing2D {
        spaces: vec![
            ModelSpace {
                id: SpaceId(0),
                unit: Unit::Meter,
                axes: Axes::EastNorth,
                order: (0..elements.len()).collect(),
                elements,
            },
            ModelSpace {
                id: SpaceId(9),
                unit: Unit::Unknown,
                axes: Axes::EastNorth,
                elements: Vec::new(),
                order: Vec::new(),
            },
        ],
        ..Drawing2D::default()
    };
    for (index, name) in ["part", "unused"].into_iter().enumerate() {
        let space = SpaceId(index as u32 + 1);
        let mut text = text.clone();
        text.placement = Placement::Anchor {
            point: ModelPoint {
                space,
                x: 3.0,
                y: 4.0,
            },
            horizontal: Horizontal::Left,
            vertical: Vertical::Baseline,
        };
        text.height.value = Length::Model(ModelLength { space, value: 1.0 });
        let mut formatted = formatted.clone();
        formatted.point.space = space;
        formatted.height = text.height.clone();
        drawing.symbols.push(SymbolDefinition {
            key: SymbolKey {
                namespace: "dxf.block".into(),
                name: name.into(),
            },
            space: ModelSpace {
                id: space,
                unit: Unit::Millimeter,
                axes: Axes::EastNorth,
                elements: Vec::new(),
                order: Vec::new(),
            },
            base: Some(ModelPoint {
                space,
                x: 3.0,
                y: 4.0,
            }),
            attributes: vec![AttributeDefinition {
                attribute: Attribute {
                    tag: "TAG".into(),
                    text,
                    formatted: Some(formatted),
                    hidden: true,
                    layer: LayerId(0),
                    style: Style::BLOCK,
                    source: Ref::at_record(41).with_key(Key::Handle(0x31)),
                },
                prompt: "値".into(),
                constant: index == 0,
                verify: true,
                preset: false,
            }],
            anonymous: index == 1,
            external: (index == 1).then(|| "unopened.dxf".into()),
            source: Ref::at_record(40 + index as u64).with_key(Key::Handle(0x30)),
        });
    }
    let paper = |x, y| PaperPoint {
        sheet: SheetId(5),
        x,
        y,
    };
    drawing.sheets.push(Sheet {
        id: SheetId(5),
        name: Some("配置".into()),
        size: Some([PaperLength(420.0), PaperLength(297.0)]),
        elements: vec![Element {
            layer: LayerId(0),
            style: Style::BLOCK,
            shape: Shape::Line {
                start: paper(0.0, 0.0),
                end: paper(2.0, 3.0),
            },
            source: Ref::default(),
        }],
        views: vec![
            View {
                space: SpaceId(0),
                sheet: SheetId(5),
                transform: Affine {
                    m: [0.0, -2.0, 10.0, 2.0, 0.0, 20.0],
                },
                clip: vec![
                    paper(10.0, 20.0),
                    paper(10.0, 24.0),
                    paper(6.0, 24.0),
                    paper(6.0, 20.0),
                ],
                hidden_layers: Vec::new(),
                source: Ref::at_record(50).with_key(Key::Handle(0x50)),
            },
            View {
                space: SpaceId(0),
                sheet: SheetId(5),
                transform: Affine {
                    m: [1.0, 1.0, 0.0, 0.0, -1.0, 0.0],
                },
                clip: Vec::new(),
                hidden_layers: Vec::new(),
                source: Ref::default(),
            },
            View {
                space: SpaceId(9),
                sheet: SheetId(5),
                transform: Affine {
                    m: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
                },
                clip: Vec::new(),
                hidden_layers: Vec::new(),
                source: Ref::default(),
            },
        ],
        order: vec![
            SheetItem::Element(0),
            SheetItem::View(0),
            SheetItem::View(1),
            SheetItem::View(2),
        ],
        source: Ref::at_record(51).with_key(Key::Handle(0x51)),
    });
    (
        binary::Source {
            format: "test-drawing-metadata".into(),
            family: "drawing".into(),
            hash: Hash([0x56; 16]),
        },
        drawing,
    )
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
    .map_err(|error| format!("{error:?}"))?;
    let hex = bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
    let result = include_str!("drawing_metadata_case.json").replace("$HEX", &hex);
    let output = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../bindings/conformance/cases/drawing_metadata.json");
    if std::env::args().nth(1).as_deref() == Some("--check") {
        if std::fs::read_to_string(output)
            .map_err(|e| e.to_string())?
            .replace("\r\n", "\n")
            != result.replace("\r\n", "\n")
        {
            return Err("図面の詳細の適合入力が古い".into());
        }
    } else {
        std::fs::write(output, result).map_err(|e| e.to_string())?;
    }
    Ok(())
}
