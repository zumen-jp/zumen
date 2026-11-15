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
    // 通り芯から軸・曲線へ進む。形の最初の 16 要素に grid がない場合も、この経路を通す。
    let mut grids = 0;
    for id in file.ids() {
        cx.check(zumen_base::Kind::Work, 1)?;
        if file.instance(id).is_some_and(|r| file.kind(r).eq_ignore_ascii_case(b"IFCGRID")) {
            if let Err(Error::Stopped(stop)) = file.grid_axes(id, cx) { return Err(Error::Stopped(stop)); }
            grids += 1;
            if grids == 16 { break; }
        }
    }
    // 要素から共有写像をたどる入口。値を写す範囲は 16 要素まで、曲線の取得とも予算を共有する
    let mut shapes = 0;
    let mut extrusions = 0;
    let mut sweeps = 0;
    let mut revolutions = 0;
    let mut faces = 0;
    let mut tessellations = 0;
    let mut half_spaces = 0;
    let mut mappings = 0;
    let mut styles = 0;
    let mut presentations = 0;
    for id in file.ids() {
        cx.check(zumen_base::Kind::Work, 1)?;
        if file.instance(id).is_some_and(|r| {
            [b"IFCWALL".as_slice(), b"IFCCOLUMN", b"IFCBEAM", b"IFCSLAB", b"IFCGRID", b"IFCSPACE", b"IFCBUILDINGELEMENTPROXY"].iter().any(|kind| file.kind(r).eq_ignore_ascii_case(kind))
        }) {
            if let Err(Error::Stopped(stop)) = file.product_geometry(id, cx) { return Err(Error::Stopped(stop)); }
            match file.product_shape(id, cx) {
                Ok(shape) => {
                    for node in &shape.nodes {
                        cx.check(zumen_base::Kind::Work, 1)?;
                        if let zumen_ifc::ShapeLinks::Representation { items, .. } = &node.links {
                            for at in items.iter().take(16 - presentations) {
                                cx.check(zumen_base::Kind::Work, 1)?;
                                if let Some(item) = shape.node(*at) {
                                    if let Err(Error::Stopped(stop)) = file.item_presentation(item, Some(node), cx) { return Err(Error::Stopped(stop)); }
                                    presentations += 1;
                                }
                            }
                        }
                        if styles < 16 {
                            if let Err(Error::Stopped(stop)) = file.surface_styles(node, cx) { return Err(Error::Stopped(stop)); }
                            styles += 1;
                        }
                        if file.kind(node.source).eq_ignore_ascii_case(b"IFCEXTRUDEDAREASOLID") && extrusions < 16 {
                            if let Err(Error::Stopped(stop)) = file.extruded_area(node, cx) { return Err(Error::Stopped(stop)); }
                            if let Err(Error::Stopped(stop)) = file.extrusion(node, cx) { return Err(Error::Stopped(stop)); }
                            extrusions += 1;
                        }
                        if file.kind(node.source).eq_ignore_ascii_case(b"IFCSWEPTDISKSOLID") && sweeps < 16 {
                            if let Err(Error::Stopped(stop)) = file.swept_disk(node, cx) { return Err(Error::Stopped(stop)); }
                            sweeps += 1;
                        }
                        if file.kind(node.source).eq_ignore_ascii_case(b"IFCREVOLVEDAREASOLID") && revolutions < 16 {
                            if let Err(Error::Stopped(stop)) = file.revolved_area(node, cx) { return Err(Error::Stopped(stop)); }
                            revolutions += 1;
                        }
                        if faces < 16 && [b"IFCFACETEDBREP".as_slice(), b"IFCSHELLBASEDSURFACEMODEL", b"IFCFACEBASEDSURFACEMODEL"].iter().any(|kind| file.kind(node.source).eq_ignore_ascii_case(kind)) {
                            if let Err(Error::Stopped(stop)) = file.faceted_faces(node, cx) { return Err(Error::Stopped(stop)); }
                            faces += 1;
                        }
                        if tessellations < 16 && [b"IFCTRIANGULATEDFACESET".as_slice(), b"IFCPOLYGONALFACESET"].iter().any(|kind| file.kind(node.source).eq_ignore_ascii_case(kind)) {
                            if let Err(Error::Stopped(stop)) = file.tessellated_faces(node, cx) { return Err(Error::Stopped(stop)); }
                            tessellations += 1;
                        }
                        if half_spaces < 16 && [b"IFCHALFSPACESOLID".as_slice(), b"IFCBOXEDHALFSPACE", b"IFCPOLYGONALBOUNDEDHALFSPACE"].iter().any(|kind| file.kind(node.source).eq_ignore_ascii_case(kind)) {
                            if let Err(Error::Stopped(stop)) = file.half_space(node, cx) { return Err(Error::Stopped(stop)); }
                            half_spaces += 1;
                        }
                        if mappings < 16 && file.kind(node.source).eq_ignore_ascii_case(b"IFCMAPPEDITEM") {
                            if let Err(Error::Stopped(stop)) = file.mapped_shape3(node, cx) { return Err(Error::Stopped(stop)); }
                            mappings += 1;
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
