//! レジストリに登録する形式（#398、決定 0017）。capabilities の形式の一覧と、図面の形式の判定は、この一覧から引く。
//! 登録の権利は、台帳（`formats/ledger.toml`）と合うことをテストで確かめる。

use zumen_acad::{Dwg, Dxf};
use zumen_jww::Jww;
use zumen_sdk::{Registration, Rights};
use zumen_sxf::Sxf;

/// 図面の形式。判定はこの順に試す。ID の規則は、AutoCAD の系統はハンドルで、SXF はインスタンスの番号で作る 2
/// （決定 0156）。書けるのは、apply で書き戻せる形式（決定 0139）。SXF は SFC と P21 に書き戻す（決定 0167・0169）
pub(crate) const DRAWING: &[Registration] = &[
    Registration::of::<Dxf>(Rights::Public, "acad", true, 2),
    Registration::of::<Dwg>(Rights::Public, "acad", true, 2),
    Registration::of::<Sxf>(Rights::Public, "sxf", true, 2),
    Registration::of::<Jww>(Rights::Public, "jww", true, 1),
];

/// 図面のモジュールが読む、図面でない形式
pub(crate) const READ_ONLY: &[Registration] = &[
    Registration::of::<zumen_landxml::LandXml>(Rights::Public, "landxml", false, 1),
    Registration::of::<zumen_citygml::CityGml>(Rights::Public, "citygml", false, 1),
    Registration::of::<zumen_fgd::Fgd>(Rights::Public, "fgd", false, 1),
    Registration::of::<zumen_extdm::ExtDm>(Rights::Public, "extdm", false, 1),
    Registration::of::<zumen_las::Las>(Rights::Public, "las", false, 1),
    Registration::of::<zumen_ply::Ply>(Rights::Public, "ply", false, 1),
    Registration::of::<zumen_xyz::Xyz>(Rights::Public, "xyz", false, 1),
    Registration::of::<zumen_e57::E57>(Rights::Public, "e57", false, 1),
];

#[cfg(test)]
mod tests {
    use super::*;

    const LEDGER: &str = include_str!("../../../formats/ledger.toml");

    /// 台帳の `[formats.<key>]` の表の行
    fn table(key: &str) -> impl Iterator<Item = &'static str> {
        let header = format!("[formats.{key}]");
        LEDGER
            .lines()
            .skip_while(move |line| line.trim() != header)
            .skip(1)
            .take_while(|line| !line.starts_with('['))
    }

    /// 台帳の `[formats.<key>]` の `name = "…"` の値
    fn field(key: &str, name: &str) -> Option<&'static str> {
        let prefix = format!("{name} = \"");
        table(key)
            .find_map(|line| line.strip_prefix(prefix.as_str()))
            .and_then(|rest| rest.strip_suffix('"'))
    }

    /// 台帳に、読みか書きの確かめた度合いがあるか（決定 0254）。表の見出し `[formats.<key>.<direction>]` と、
    /// `[formats.<key>]` の中のインラインの表（`read = { … }`）・ドットのキー（`read.level = …`）を数える
    fn verified(key: &str, direction: &str) -> bool {
        let header = format!("[formats.{key}.{direction}]");
        let inline = format!("{direction} = {{");
        let dotted = format!("{direction}.");
        LEDGER.lines().any(|line| line.trim() == header)
            || table(key).any(|line| line.starts_with(&inline) || line.starts_with(&dotted))
    }

    #[test]
    fn registrations_match_the_ledger() {
        for registration in DRAWING.iter().chain(READ_ONLY) {
            let format = registration.format;
            assert_eq!(
                field(format, "rights"),
                Some(registration.rights.key()),
                "{format}"
            );
            assert_eq!(field(format, "place"), Some("core"), "{format}");
            // capabilities の read・write と同じ向きに、確かめた度合いがある
            assert!(
                verified(format, "read"),
                "{format}: 読みの確かめた度合いがない"
            );
            assert_eq!(
                verified(format, "write"),
                registration.write,
                "{format}: 書きの確かめた度合いが、書けるかと合わない"
            );
        }
    }
}
