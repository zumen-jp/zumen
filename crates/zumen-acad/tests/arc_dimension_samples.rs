//! AutoCAD で作った弧長寸法（ARC_DIMENSION）の公開試料を読む（#316）。

use std::path::PathBuf;
use zumen_acad::{AcadDb, Data, Dwg, Dxf, Object};
use zumen_base::{Budget, CancelToken, Cx, Diagnostics};
use zumen_sdk::Codec;

/// 各版の DWG と DXF の組を固定の名前で開く。欠けていれば試験を失敗させる。
#[cfg(test)]
fn pairs() -> Vec<(PathBuf, PathBuf)> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../samples/dwg");
    ["r2004", "r2018"]
        .into_iter()
        .map(|version| {
            (
                dir.join(format!("zumen-arcdim-{version}.dwg")),
                dir.join(format!("zumen-arcdim-{version}_ascii.dxf")),
            )
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

/// 弧長寸法の欄を、符号と値の組で返す。DXF では 100 の区切りの後、DWG では最初の 13 から後とする。DWG では
/// ハンドルの流れから読む名前（2・3）も後に置くので除く
#[cfg(test)]
fn arc_values(object: &Object, dwg: bool) -> Vec<(i16, Data)> {
    let start = object
        .values
        .iter()
        .position(|value| {
            if dwg {
                value.code == 13
            } else {
                value.code == 100 && value.data == Data::Text(b"AcDbArcDimension".to_vec())
            }
        })
        .unwrap();
    let start = if dwg { start } else { start + 1 };
    let mut values: Vec<_> = object.values[start..]
        .iter()
        .filter(|value| !matches!(value.code, 2 | 3))
        .map(|value| (value.code, value.data.clone()))
        .collect();
    values.sort_by_key(|(code, _)| *code);
    values
}

/// DXF は実数を 16 桁で書くので、丸めの違いを許して比べる
#[cfg(test)]
fn same(a: &[(i16, Data)], b: &[(i16, Data)]) -> bool {
    a.len() == b.len()
        && a.iter().zip(b).all(|((ca, da), (cb, db))| {
            ca == cb
                && match (da, db) {
                    (Data::Real(x), Data::Real(y)) => (x - y).abs() <= 1e-9 * x.abs().max(1.0),
                    _ => da == db,
                }
        })
}

/// 試料を作る台本（xtask/autocad/samples.lsp）の 6 つの弧長寸法を、DWG からも DXF と同じ値で読む。共通の欄の
/// 70・71 は、弧長寸法の欄より先に読む。DWG の値は 100 の区切りを持たない
#[test]
fn autocad_arc_dimension_values_match_between_dwg_and_dxf() {
    for (dwg, dxf) in pairs() {
        let arcs = |db: &AcadDb, dwg: bool| {
            db.objects()
                .iter()
                .filter(|object| object.kind == b"ARC_DIMENSION")
                .map(|object| {
                    let mut values: Vec<_> = [10, 20, 11, 21, 42, 70, 71]
                        .into_iter()
                        .filter_map(|code| object.first(code).map(|data| (code, data.clone())))
                        .collect();
                    values.extend(arc_values(object, dwg));
                    (object.handle(), values)
                })
                .collect::<Vec<_>>()
        };
        let from_dwg = arcs(&table(&dwg), true);
        let from_dxf = arcs(&table(&dxf), false);
        assert_eq!(from_dwg.len(), 6, "{}", dwg.display());
        assert_eq!(from_dwg.len(), from_dxf.len(), "{}", dxf.display());
        for ((handle, a), (other, b)) in from_dwg.iter().zip(&from_dxf) {
            assert_eq!(handle, other, "{}", dwg.display());
            assert!(same(a, b), "{} {handle:?}\n{a:?}\n{b:?}", dwg.display());
        }
    }
}

/// 弧長の寸法として、円弧の中心・両端・寸法の弧の点と、寸法値の上書きを移す。保存した測定値は、図面の単位の
/// 弧の長さになる。引出線は診断する。
#[cfg(feature = "drawing")]
#[test]
fn autocad_arc_dimensions_lift_to_arc_lengths() {
    use zumen_base::codes;
    use zumen_drawing::{DimensionKind, Label, Shape};
    use zumen_sdk::Lift;

    for (dwg, dxf) in pairs() {
        for path in [dwg, dxf] {
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
            let at = path.display();
            let dimensions: Vec<_> = drawing
                .spaces
                .iter()
                .flat_map(|space| &space.elements)
                .filter_map(|element| match &element.shape {
                    Shape::Dimension(dimension) => Some(dimension),
                    _ => None,
                })
                .collect();
            assert_eq!(dimensions.len(), 6, "{at}");
            // 台本の円弧の中心・半径・角度（度）と、寸法値の上書き
            for (dimension, (cx0, cy0, r, a1, a2, label)) in dimensions.iter().zip([
                (0.0, 0.0, 50.0, 30.0, 120.0, None),
                (150.0, 0.0, 50.0, 30.0, 120.0, None),
                (300.0, 0.0, 50.0, 30.0, 120.0, None),
                (0.0, -150.0, 40.0, 30.0, 60.0, None),
                (150.0, -150.0, 40.0, 0.0, 150.0, None),
                (300.0, -150.0, 40.0, 0.0, 90.0, Some("上書き<>")),
            ]) {
                let DimensionKind::ArcLength {
                    center,
                    first,
                    second,
                    ..
                } = dimension.kind
                else {
                    panic!("{at}: {:?}", dimension.kind);
                };
                let near = |x: f64, y: f64| (x - y).abs() < 1e-9;
                assert!(near(center.x, cx0) && near(center.y, cy0), "{at}");
                // 両端の順は決まっていない。部分の寸法では、13 が終わりの角度の点だった
                let on = |point: &zumen_drawing::ModelPoint, angle: f64| {
                    let angle = f64::to_radians(angle);
                    near(point.x, cx0 + r * libm::cos(angle))
                        && near(point.y, cy0 + r * libm::sin(angle))
                };
                assert!(
                    on(&first, a1) && on(&second, a2) || on(&first, a2) && on(&second, a1),
                    "{at}: {first:?} {second:?}"
                );
                let length = r * f64::to_radians(a2 - a1);
                let stored = dimension.measurement.unwrap().value;
                assert!((stored - length).abs() < 1e-9, "{at}: {stored} {length}");
                match label {
                    Some(text) => assert!(
                        matches!(&dimension.label, Label::Override { content, .. } if content == text),
                        "{at}"
                    ),
                    None => assert_eq!(dimension.label, Label::Measurement, "{at}"),
                }
            }
            let leaders = diag
                .iter()
                .filter(|d| d.code() == codes::DXF_ARC_DIMENSION_LEADER_DROPPED)
                .count();
            assert_eq!(leaders, 1, "{at}");
        }
    }
}
