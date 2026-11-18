//! 記号の定義（ブロック）を足し、足した定義を INSERT で置いて再読する（#684）。
#![cfg(feature = "drawing")]

use std::path::Path;
use zumen_acad::{Dwg, Dxf};
use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Ref, codes};
use zumen_drawing::{
    Applied, Changes, Drawing2D, Edge, Edit, Element, Hash, Length, ModelLength, ModelPoint,
    ModelSpace, Placed, Polyline, Shape, SpaceId, Symbol, SymbolDefinition, SymbolKey, Vertex, id,
};
use zumen_sdk::{Apply, Codec, FileModel, Lift, testkit};

const NAME: &str = "ZUMEN_KIT";

/// 足す定義。中に線・円・閉じた折れ線（膨らみを含む）を置く
#[cfg(test)]
fn definition(drawing: &Drawing2D, name: &str) -> SymbolDefinition {
    let model = &drawing.spaces[0];
    let source = &model.elements[0];
    let space = SpaceId(
        drawing
            .spaces
            .iter()
            .map(|space| space.id.0)
            .chain(drawing.symbols.iter().map(|symbol| symbol.space.id.0))
            .max()
            .unwrap()
            + 1,
    );
    let point = |x, y| ModelPoint { space, x, y };
    let element = |shape| Element {
        source: Ref::default(),
        layer: source.layer,
        style: source.style,
        shape,
    };
    let elements = vec![
        element(Shape::Line {
            start: point(0.0, 0.0),
            end: point(10.0, 5.0),
        }),
        element(Shape::Circle {
            center: point(5.0, 5.0),
            radius: Length::Model(ModelLength { space, value: 2.5 }),
        }),
        element(Shape::Polyline(Polyline {
            vertices: [(0.0, 0.0, 0.0), (10.0, 0.0, 0.5), (10.0, 10.0, 0.0)]
                .into_iter()
                .map(|(x, y, bulge)| Vertex {
                    point: point(x, y),
                    bulge,
                })
                .collect(),
            closed: true,
            width: None,
            vertex_widths: Vec::new(),
            line_type_generation: false,
        })),
    ];
    SymbolDefinition {
        key: SymbolKey {
            namespace: "dxf.block".into(),
            name: name.to_owned(),
        },
        space: ModelSpace {
            id: space,
            unit: model.unit,
            axes: model.axes,
            order: (0..elements.len()).collect(),
            elements,
        },
        base: Some(point(5.0, 2.0)),
        attributes: Vec::new(),
        anonymous: false,
        external: None,
        source: Ref::default(),
    }
}

/// 足した定義を置く INSERT
#[cfg(test)]
fn insert(drawing: &Drawing2D) -> Element<ModelPoint> {
    let space = drawing.spaces[0].id;
    let source = &drawing.spaces[0].elements[0];
    Element {
        source: Ref::default(),
        layer: source.layer,
        style: source.style,
        shape: Shape::Symbol(Symbol {
            key: SymbolKey {
                namespace: "dxf.block".into(),
                name: NAME.to_owned(),
            },
            position: ModelPoint {
                space,
                x: 100.0,
                y: 50.0,
            },
            scale: [2.0, 2.0],
            rotation: 30.0,
            grid: None,
            attributes: Vec::new(),
            clip: None,
        }),
    }
}

#[cfg(test)]
fn summary(drawing: &Drawing2D) -> Vec<(String, Hash)> {
    let mut found = Vec::new();
    id::each_element(drawing, |_, name, element| {
        found.push((name, element.hash()));
        Ok::<_, ()>(())
    })
    .unwrap();
    found
}

#[cfg(test)]
fn cx_run<T>(f: impl FnOnce(&mut Cx<'_>) -> T) -> (T, Diagnostics) {
    let mut diagnostics = Diagnostics::new();
    let mut budget = Budget::default();
    let cancel = CancelToken::new();
    let value = f(&mut Cx::new(&mut budget, &mut diagnostics, &cancel));
    (value, diagnostics)
}

/// 定義を足し、続く変更の一覧で INSERT を置く。元の要素の ID とハッシュを保ち、足した定義と配置を読み直す
#[cfg(test)]
fn roundtrip<C: Codec>(src: &[u8]) -> Vec<u8>
where
    C::File: Lift<Drawing2D> + Apply<Changes, Output = Vec<Applied>>,
{
    let mut file = testkit::read::<C>(src).0.unwrap();
    let before = cx_run(|cx| file.lift(cx)).0.unwrap();
    let added = definition(&before, NAME);
    let (results, diagnostics) = cx_run(|cx| {
        file.apply(
            &Changes {
                document: None,
                edits: vec![Edit::AddSymbol {
                    symbol: Box::new(added.clone()),
                }],
            },
            cx,
        )
    });
    let results = results.unwrap_or_else(|_| panic!("{diagnostics:?}"));
    assert_eq!(
        results,
        [Applied {
            id: None,
            hash: None
        }]
    );
    let middle = cx_run(|cx| file.lift(cx)).0.unwrap();
    let placed = insert(&middle);
    let (results, diagnostics) = cx_run(|cx| {
        file.apply(
            &Changes {
                document: None,
                edits: vec![Edit::Add {
                    place: id::space(middle.spaces[0].id),
                    element: Box::new(Placed::Model(placed.clone())),
                }],
            },
            cx,
        )
    });
    assert!(results.is_ok(), "{diagnostics:?}");
    let out = testkit::write::<C>(&file).unwrap();
    let after = cx_run(|cx| testkit::read::<C>(&out).0.unwrap().lift(cx))
        .0
        .unwrap();
    let found = summary(&after);
    for known in summary(&before) {
        assert!(found.contains(&known), "{known:?}");
    }
    let read = after
        .symbols
        .iter()
        .find(|symbol| symbol.key.name == NAME)
        .unwrap();
    assert!(!read.anonymous && read.external.is_none() && read.attributes.is_empty());
    let at = |point: &ModelPoint| (point.x, point.y);
    assert_eq!(read.base.as_ref().map(at), added.base.as_ref().map(at));
    // 読み直した定義の空間の ID は、足した定義のものと限らない。ID を揃えて比べる
    let shapes = |space: &ModelSpace| {
        let shapes: Vec<_> = space
            .elements
            .iter()
            .map(|element| (element.layer, &element.shape))
            .collect();
        format!("{shapes:?}").replace(&format!("{:?}", space.id), &format!("{:?}", added.space.id))
    };
    assert_eq!(shapes(&read.space), shapes(&added.space));
    let actual = &after.spaces[0].elements;
    // DWG は回転を弧度で持つので、度に戻した値は末尾がずれる
    let same = |shape: &Shape<ModelPoint>| match (shape, &placed.shape) {
        (Shape::Symbol(read), Shape::Symbol(placed)) => {
            let mut read = read.clone();
            let near = (read.rotation - placed.rotation).abs() < 1e-9;
            read.rotation = placed.rotation;
            near && &read == placed
        }
        _ => false,
    };
    assert!(
        actual
            .iter()
            .any(|e| e.layer == placed.layer && same(&e.shape)),
        "{:?}",
        actual
            .iter()
            .filter(|e| matches!(e.shape, Shape::Symbol(_)))
            .collect::<Vec<_>>()
    );
    out
}

#[test]
fn symbols_are_defined_and_placed_in_public_dxf_and_dwg_versions() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../samples/dwg");
    for version in ["r2000", "r2004", "r2018"] {
        let src = std::fs::read(root.join(format!("zumen-layout-{version}_ascii.dxf"))).unwrap();
        let out = roundtrip::<Dxf>(&src);
        // 触れていないレコードは元のバイトのまま
        let before = testkit::read::<Dxf>(&src).0.unwrap();
        let after = testkit::read::<Dxf>(&out).0.unwrap();
        for (index, record) in before.records().iter().enumerate() {
            if matches!(before.kind(record), b"SECTION" | b"TABLE") {
                continue;
            }
            let Some(handle) = record.tags().iter().find(|tag| tag.code() == 5) else {
                continue;
            };
            let found = after
                .records()
                .iter()
                .position(|candidate| {
                    candidate
                        .tags()
                        .iter()
                        .any(|tag| tag.code() == 5 && after.value(tag) == before.value(handle))
                })
                .unwrap();
            let a = before.record(index).unwrap().bytes.unwrap();
            let b = after.record(found).unwrap().bytes.unwrap();
            assert_eq!(
                &src[a.start as usize..a.end as usize],
                &out[b.start as usize..b.end as usize],
                "{version}: {index}"
            );
        }
        let src = std::fs::read(root.join(format!("zumen-layout-{version}.dwg"))).unwrap();
        let out = roundtrip::<Dwg>(&src);
        let before = testkit::read::<Dwg>(&src).0.unwrap();
        let after = testkit::read::<Dwg>(&out).0.unwrap();
        let mut compared = 0;
        for record in 0..before.entry_count() {
            let (handle, kind, _, _) = before.entry_summary(record).unwrap();
            // LAYER・STYLE・LTYPE・APPID・DIMSTYLE は、定義の追加で所有や連結を変えない。
            if !matches!(kind, 0x33 | 0x35 | 0x39 | 0x43 | 0x45) {
                continue;
            }
            let found = (0..after.entry_count())
                .find(|&index| after.entry_summary(index).is_some_and(|e| e.0 == handle))
                .unwrap();
            assert_eq!(
                before.object_data(record),
                after.object_data(found),
                "{version}: {handle:X}"
            );
            compared += 1;
        }
        assert!(compared >= 10, "{version}: {compared}");
    }
}

#[test]
fn unsupported_symbols_are_atomic() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../samples/dwg/zumen-layout-r2000_ascii.dxf");
    let src = std::fs::read(root).unwrap();
    let file = testkit::read::<Dxf>(&src).0.unwrap();
    let drawing = cx_run(|cx| file.lift(cx)).0.unwrap();
    let existing = drawing
        .symbols
        .iter()
        .find(|symbol| !symbol.anonymous && !symbol.key.name.starts_with('*'))
        .map(|symbol| symbol.key.name.to_lowercase());
    let mut cases: Vec<(Vec<SymbolDefinition>, &str, &str)> = Vec::new();
    let base = definition(&drawing, NAME);
    if let Some(name) = existing {
        cases.push((vec![definition(&drawing, &name)], "name", "exists"));
    }
    cases.push((vec![base.clone(), base.clone()], "name", "exists"));
    cases.push((vec![definition(&drawing, "*U9")], "name", "invalid"));
    cases.push((vec![definition(&drawing, "A<B")], "name", "invalid"));
    let mut anonymous = base.clone();
    anonymous.anonymous = true;
    cases.push((vec![anonymous], "anonymous", "unsupported"));
    let mut external = base.clone();
    external.external = Some("x.dwg".into());
    cases.push((vec![external], "external", "unsupported"));
    let mut namespace = base.clone();
    namespace.key.namespace = "jww.block".into();
    cases.push((vec![namespace], "namespace", "unsupported"));
    let mut moved = base.clone();
    moved.base = Some(ModelPoint {
        space: drawing.spaces[0].id,
        x: 0.0,
        y: 0.0,
    });
    cases.push((vec![moved], "base", "invalid"));
    // 中の要素の点が、定義の空間のものでない
    let mut elsewhere = base.clone();
    elsewhere.space.elements[1].shape = Shape::Curve(Edge::Line {
        start: ModelPoint {
            space: drawing.spaces[0].id,
            x: 0.0,
            y: 0.0,
        },
        end: ModelPoint {
            space: drawing.spaces[0].id,
            x: 1.0,
            y: 0.0,
        },
    });
    cases.push((vec![elsewhere], "elements/1", "unsupported"));
    for (symbols, field, value) in cases {
        let mut file = testkit::read::<Dxf>(&src).0.unwrap();
        let edits = symbols
            .into_iter()
            .map(|symbol| Edit::AddSymbol {
                symbol: Box::new(symbol),
            })
            .collect();
        let (result, diagnostics) = cx_run(|cx| {
            file.apply(
                &Changes {
                    document: None,
                    edits,
                },
                cx,
            )
        });
        assert!(result.is_err(), "{field}: {value}");
        let rejected = diagnostics
            .iter()
            .find(|d| d.code() == codes::CHANGE_SYMBOL_UNSUPPORTED)
            .unwrap_or_else(|| panic!("{field}: {diagnostics:?}"));
        assert_eq!(rejected.param("field"), Some(&field.into()), "{value}");
        assert_eq!(rejected.param("value"), Some(&value.into()), "{field}");
        assert_eq!(rejected.param("format"), Some(&"DXF".into()));
        assert_eq!(testkit::write::<Dxf>(&file).unwrap(), src);
    }
}
