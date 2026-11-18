//! 公開サンプル（`samples/boring/`）のボーリング交換用データを読む（#505）。期待の値は、同じファイルを Python の
//! 標準の XML の読み（`xml.etree.ElementTree`）で読んだ値。

use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Severity, codes};
use zumen_crs::{HeightDatum, Realization};
use zumen_geology::{Boring, Datum, read};

/// 国土地盤情報検索サイトの、関東地方整備局のボーリング 1 本（BED0210.DTD、Shift_JIS）
const KUNIJIBAN: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../samples/boring/kunijiban-150326024.xml"
);

#[cfg(test)]
fn load() -> (Boring, Vec<(String, Severity, Option<String>)>) {
    let bytes = std::fs::read(KUNIJIBAN).unwrap();
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let boring = {
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        read(&bytes, &mut cx)
    };
    let boring = boring.unwrap();
    let found = diag
        .iter()
        .map(|d| {
            let name = match d.param("name") {
                Some(zumen_base::Value::Text(text)) => Some(text.to_string()),
                _ => None,
            };
            (d.code().name().to_string(), d.code().severity(), name)
        })
        .collect();
    (boring, found)
}

#[test]
fn kunijiban_boring_matches_the_standard_xml_reader() {
    let (boring, found) = load();
    assert_eq!(
        boring.survey.as_deref(),
        Some("新大宮バイパス赤塚大門地区地質調査")
    );
    assert_eq!(boring.name, None);
    assert_eq!(boring.count, None);
    assert_eq!(boring.serial, Some(4));
    let Some(position) = boring.position else {
        panic!("位置がない");
    };
    assert!((position.longitude - (139.0 + 39.0 / 60.0 + 1.41 / 3600.0)).abs() < 1e-12);
    assert!((position.latitude - (35.0 + 46.0 / 60.0 + 44.594 / 3600.0)).abs() < 1e-12);
    // 2.10 の測地系のコード 1 は測地成果2000
    assert_eq!(boring.datum, Some(Datum::Jgd2000));
    assert_eq!(boring.address, None);
    assert_eq!(boring.start, None);
    assert_eq!(boring.end.as_deref(), Some("1987-11"));
    let Some(elevation) = boring.elevation else {
        panic!("標高がない");
    };
    assert_eq!(elevation.value, 27.47);
    assert_eq!(elevation.datum, HeightDatum::TokyoBay);
    assert_eq!(elevation.realization, Realization::Survey2011);
    assert_eq!(boring.length, Some(18.3));
    assert_eq!(boring.angle, None);
    assert_eq!(boring.direction, Some(0.0));

    let layers: Vec<(f64, &str)> = boring
        .layers
        .iter()
        .map(|layer| (layer.bottom, layer.name.as_str()))
        .collect();
    assert_eq!(
        layers,
        [
            (2.25, "埋土"),
            (3.8, "ローム"),
            (7.6, "有機質粘性土"),
            (9.3, "砂"),
            (11.0, "砂"),
            (13.25, "砂質粘性土"),
            (14.2, "粘土質シルト"),
            (14.7, "砂質粘性土"),
            (15.8, "砂"),
            (18.3, "礫"),
        ]
    );
    assert!(boring.layers.iter().all(|layer| layer.symbol.is_none()));
    let colors: Vec<(f64, Option<&str>)> = boring
        .colors
        .iter()
        .map(|color| (color.bottom, color.name.as_deref()))
        .collect();
    assert_eq!(
        colors,
        [
            (2.25, Some("暗褐")),
            (3.8, Some("黄褐")),
            (7.6, Some("淡黄灰")),
            (9.3, Some("暗黄灰")),
            (11.0, Some("暗黄灰")),
            (13.25, Some("褐灰")),
            (14.2, Some("暗灰")),
            (14.7, Some("黄灰")),
            (15.8, Some("暗黄灰")),
            (18.3, Some("黄暗灰")),
        ]
    );
    // 観察記事は 1 つあるが、深度も記事も空
    assert!(boring.observations.is_empty());
    let penetrations: Vec<(f64, u32, f64)> = boring
        .penetrations
        .iter()
        .map(|p| (p.depth, p.blows, p.penetration))
        .collect();
    assert_eq!(
        penetrations,
        [
            (1.15, 7, 30.0),
            (2.15, 5, 30.0),
            (3.15, 4, 30.0),
            (4.15, 6, 30.0),
            (5.15, 2, 30.0),
            (6.15, 8, 30.0),
            (7.15, 6, 30.0),
            (8.15, 28, 30.0),
            (9.15, 37, 30.0),
            (10.15, 31, 30.0),
            (11.15, 27, 30.0),
            (12.15, 29, 30.0),
            (13.15, 7, 30.0),
            (14.15, 35, 30.0),
            (15.15, 50, 27.0),
            (16.0, 50, 1.0),
            (17.15, 50, 27.0),
            (18.15, 50, 15.0),
        ]
    );
    assert_eq!(boring.water_levels.len(), 1);
    let water = boring.water_levels.first();
    assert_eq!(water.and_then(|w| w.depth), Some(3.9));
    assert_eq!(water.and_then(|w| w.date.clone()), None);

    // 診断は情報だけ。成果の仮定と、読まない要素
    assert!(
        found
            .iter()
            .all(|(_, severity, _)| *severity == Severity::Info),
        "{found:?}"
    );
    assert!(
        found
            .iter()
            .any(|(code, _, _)| code == codes::CRS_HEIGHT_REALIZATION_ASSUMED.name())
    );
    assert!(found.iter().any(|(code, _, name)| {
        code == codes::BORING_ELEMENT_UNSUPPORTED.name() && name.as_deref() == Some("調査会社")
    }));
}
