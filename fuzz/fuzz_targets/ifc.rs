//! IFC（IFC-SPF と ifcZIP）の fuzz の target（#501）。読んで建物モデルと土木の領域モデルに移すまでが、小さい予算の
//! 中で止まり、2 回で同じ結果になることを確かめる。ZIP の署名で始まる入力は、ifcZIP として読む。

#![no_main]

use libfuzzer_sys::fuzz_target;
use zumen_building::Building;
use zumen_civil::CivilModel;
use zumen_ifc::{Ifc, IfcZip};
use zumen_sdk::{Codec, Cx, Error, Lift};

/// 建物モデルと土木の領域モデルに移す。土木の領域モデルは IFC4X3 からだけ作るので、ほかのバージョンの `Unsupported` は
/// 失敗にしない。止めたときだけ失敗を返す
fn lift<F: Lift<Building> + Lift<CivilModel>>(file: &F, cx: &mut Cx) -> Result<(), Error> {
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
            lift(&IfcZip::read(src, cx)?, cx)
        } else {
            lift(&Ifc::read(src, cx)?, cx)
        }
    });
    if let Err(message) = read {
        panic!("{message}");
    }
});
