//! IFC（IFC-SPF と ifcZIP）の fuzz の target（#501）。読んで建物モデルと土木の領域モデルに移すまでが、小さい予算の
//! 中で止まり、2 回で同じ結果になることを確かめる。ZIP の署名で始まる入力は、ifcZIP として読む。

#![no_main]

use libfuzzer_sys::fuzz_target;
use zumen_building::Building;
use zumen_civil::CivilModel;
use zumen_ifc::{Ifc, IfcFile, IfcZip};
use zumen_sdk::{Codec, Cx, Error, Lift};

/// 建物モデルと土木の領域モデルに移す。土木の領域モデルは IFC4X3 からだけ作るので、ほかのバージョンの `Unsupported` は
/// 失敗にしない。止めたときだけ失敗を返す
fn lift(file: &IfcFile, cx: &mut Cx) -> Result<(), Error> {
    // 要素から共有写像をたどる入口。値を写す範囲は 16 要素まで、曲線の取得とも予算を共有する
    let mut shapes = 0;
    let mut extrusions = 0;
    for id in file.ids() {
        cx.check(zumen_base::Kind::Work, 1)?;
        if file.instance(id).is_some_and(|r| {
            [b"IFCWALL".as_slice(), b"IFCCOLUMN", b"IFCBEAM", b"IFCSLAB", b"IFCGRID", b"IFCSPACE", b"IFCBUILDINGELEMENTPROXY"].iter().any(|kind| file.kind(r).eq_ignore_ascii_case(kind))
        }) {
            match file.product_shape(id, cx) {
                Ok(shape) => {
                    for node in &shape.nodes {
                        cx.check(zumen_base::Kind::Work, 1)?;
                        if file.kind(node.source).eq_ignore_ascii_case(b"IFCEXTRUDEDAREASOLID") && extrusions < 16 {
                            if let Err(Error::Stopped(stop)) = file.extruded_area(node, cx) { return Err(Error::Stopped(stop)); }
                            extrusions += 1;
                        }
                    }
                }
                Err(Error::Stopped(stop)) => return Err(Error::Stopped(stop)),
                _ => {}
            }
            shapes += 1;
            if shapes == 16 { break; }
        }
    }
    // 必要時の曲線の入口も通す。全体を 16 度まで写し、共有する予算で探索と確保を止める
    let mut curves = 0;
    for id in file.ids() {
        cx.check(zumen_base::Kind::Work, 1)?;
        if file.instance(id).is_some_and(|r| {
            let kind = file.kind(r);
            kind.ends_with(b"CURVE")
                || [b"IFCPOLYLINE".as_slice(), b"IFCLINE", b"IFCCIRCLE"]
                    .iter()
                    .any(|name| kind.eq_ignore_ascii_case(name))
        }) {
            if let Err(Error::Stopped(stop)) = file.curve(id, cx) {
                return Err(Error::Stopped(stop));
            }
            curves += 1;
            if curves == 16 {
                break;
            }
        }
    }
    let building = Lift::<Building>::lift(file, cx).map(drop);
    let civil = Lift::<CivilModel>::lift(file, cx).map(drop);
    match (building, civil) {
        (Err(Error::Stopped(stop)), _) | (_, Err(Error::Stopped(stop))) => {
            Err(Error::Stopped(stop))
        }
        _ => Ok(()),
    }
}

fuzz_target!(|data: &[u8]| {
    let read = zumen_sdk::testkit::survives_with(data, |src, cx| {
        if src.starts_with(b"PK") {
            lift(IfcZip::read(src, cx)?.ifc(), cx)
        } else {
            lift(&Ifc::read(src, cx)?, cx)
        }
    });
    if let Err(message) = read {
        panic!("{message}");
    }
});
