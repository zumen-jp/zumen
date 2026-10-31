//! AutoCAD で作った MINSERT の公開試料を読む（#316、決定 0161）。

use std::path::PathBuf;
use zumen_acad::{AcadDb, Data, Dwg, Dxf, Object};
use zumen_base::{Budget, CancelToken, Cx, Diagnostics};
use zumen_sdk::Codec;

/// 各試料を固定の名前で開く。欠けていれば試験を失敗させる。
#[cfg(test)]
fn samples() -> Vec<PathBuf> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../samples/dwg");
    ["r2000", "r2004", "r2018"]
        .into_iter()
        .flat_map(|version| {
            let dir = dir.clone();
            [".dwg", "_ascii.dxf"]
                .into_iter()
                .map(move |suffix| dir.join(format!("zumen-minsert-{version}{suffix}")))
        })
        .collect()
}

#[cfg(test)]
fn table(path: &PathBuf) -> AcadDb {
    let src = std::fs::read(path).unwrap();
    let mut diag = Diagnostics::new();
    let mut budget = Budget::default();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
    if path.extension().is_some_and(|extension| extension == "dwg") {
        Dwg::read(&src, &mut cx).unwrap().objects(&mut cx).unwrap()
    } else {
        Dxf::read(&src, &mut cx).unwrap().objects(&mut cx).unwrap()
    }
}

#[cfg(test)]
fn value(object: &Object, code: i16) -> Option<&Data> {
    object
        .values
        .iter()
        .find(|value| value.code == code)
        .map(|value| &value.data)
}

/// 試料を作る台本（xtask/autocad/samples.lsp）の数と間隔、属性を DWG・DXF から読む。
#[test]
fn autocad_minsert_values_and_owned_attributes_are_read() {
    for path in samples() {
        let db = table(&path);
        let inserts: Vec<_> = db
            .objects()
            .iter()
            .filter(|object| object.kind == b"INSERT")
            .collect();
        assert_eq!(inserts.len(), 4, "{}", path.display());
        for (handle, name, columns, rows, dx, dy, attributes) in [
            (0x2e7, b"CELL".as_slice(), 3, 2, 20.0, 10.0, false),
            (0x2e8, b"CELL".as_slice(), 2, 3, 25.0, 15.0, false),
            (0x2e9, b"TAGGED".as_slice(), 3, 2, 20.0, 10.0, true),
            (0x2ef, b"TAGGED".as_slice(), 2, 2, 30.0, 12.0, true),
        ] {
            let found = inserts
                .iter()
                .find(|object| object.handle() == Some(handle))
                .unwrap();
            let at = format!("{} {handle:X}", path.display());
            assert_eq!(value(found, 2), Some(&Data::Text(name.to_vec())), "{at}");
            assert_eq!(value(found, 70), Some(&Data::Int(columns)), "{at}");
            assert_eq!(value(found, 71), Some(&Data::Int(rows)), "{at}");
            assert_eq!(value(found, 44), Some(&Data::Real(dx)), "{at}");
            assert_eq!(value(found, 45), Some(&Data::Real(dy)), "{at}");
            assert_eq!(
                value(found, 66),
                attributes.then_some(&Data::Int(1)),
                "{at}"
            );
            let owned: Vec<_> = db
                .objects()
                .iter()
                .filter(|object| object.owner() == Some(handle))
                .collect();
            let tags: Vec<_> = owned
                .iter()
                .filter(|object| object.kind == b"ATTRIB")
                .filter_map(|object| value(object, 2))
                .collect();
            let ends = owned
                .iter()
                .filter(|object| object.kind == b"SEQEND")
                .count();
            if attributes {
                assert_eq!(
                    tags,
                    [&Data::Text(b"NO".to_vec()), &Data::Text(b"KIND".to_vec())],
                    "{at}"
                );
                assert_eq!(ends, 1, "{at}");
            } else {
                assert!(tags.is_empty(), "{at}");
                assert_eq!(ends, 0, "{at}");
            }
        }
    }
}

/// 点・尺度・回転と格子と属性を図面モデルへ移す（決定 0161）。
#[cfg(feature = "drawing")]
#[test]
fn autocad_minsert_values_and_attributes_lift_to_symbols() {
    use zumen_drawing::{Length, Shape};
    use zumen_sdk::Lift;

    for path in samples() {
        let src = std::fs::read(&path).unwrap();
        let mut diag = Diagnostics::new();
        let mut budget = Budget::default();
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        let drawing = if path.extension().is_some_and(|extension| extension == "dwg") {
            Dwg::read(&src, &mut cx).unwrap().lift(&mut cx).unwrap()
        } else {
            Dxf::read(&src, &mut cx).unwrap().lift(&mut cx).unwrap()
        };
        let symbols: Vec<_> = drawing
            .spaces
            .iter()
            .flat_map(|space| &space.elements)
            .filter_map(|element| {
                if let Shape::Symbol(symbol) = &element.shape {
                    Some(symbol)
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(symbols.len(), 4, "{}", path.display());
        for (x, y, name, scale, rotation, columns, rows, dx, dy, has_attributes) in [
            (0.0, 0.0, "CELL", [1.0, 1.0], 0.0, 3, 2, 20.0, 10.0, false),
            (
                100.0,
                0.0,
                "CELL",
                [2.0, 1.5],
                30.0,
                2,
                3,
                25.0,
                15.0,
                false,
            ),
            (
                0.0,
                100.0,
                "TAGGED",
                [1.0, 1.0],
                0.0,
                3,
                2,
                20.0,
                10.0,
                true,
            ),
            (
                100.0,
                100.0,
                "TAGGED",
                [1.5, 1.5],
                15.0,
                2,
                2,
                30.0,
                12.0,
                true,
            ),
        ] {
            let symbol = symbols
                .iter()
                .find(|symbol| symbol.position.x == x && symbol.position.y == y)
                .unwrap();
            let at = format!("{} ({x},{y})", path.display());
            assert_eq!(symbol.key.name, name, "{at}");
            assert_eq!(symbol.scale, scale, "{at}");
            assert!((symbol.rotation - rotation).abs() < 1e-9, "{at}");
            let grid = symbol.grid.unwrap();
            assert_eq!((grid.columns, grid.rows), (columns, rows), "{at}");
            assert!(
                matches!(grid.column_spacing, Length::Model(length) if length.value == dx),
                "{at}"
            );
            assert!(
                matches!(grid.row_spacing, Length::Model(length) if length.value == dy),
                "{at}"
            );
            if has_attributes {
                let attrs: Vec<_> = symbol
                    .attributes
                    .iter()
                    .map(|attr| (attr.tag.as_str(), attr.text.content.as_str()))
                    .collect();
                assert_eq!(attrs, [("NO", "A1"), ("KIND", "壁")], "{at}");
            } else {
                assert!(symbol.attributes.is_empty(), "{at}");
            }
        }
    }
}
