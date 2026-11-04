//! 試験の中で作った基盤地図情報の配布の書庫（ZIP）を読み、1 つの地理の領域モデルにまとめる（決定 0251、#493）。

use zumen_base::{Budget, CancelToken, Code, Cx, Diagnostics, codes};
use zumen_crs::{Crs, VerticalCrs};
use zumen_geo::GeoModel;

/// 地物を 1 つ持つ GML。`srs` はその座標系
fn gml(kind: &str, id: &str, srs: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Dataset xmlns="http://fgd.gsi.go.jp/spec/2008/FGD_GMLSchema" xmlns:gml="http://www.opengis.net/gml/3.2" gml:id="Dataset1">
  <{kind} gml:id="{id}">
    <fid>fgoid:{id}</fid>
    <lfSpanFr gml:id="{id}-1"><gml:timePosition>2025-08-01</gml:timePosition></lfSpanFr>
    <loc><gml:Curve gml:id="{id}-g" srsName="{srs}"><gml:segments><gml:LineStringSegment><gml:posList>35.0 139.0 35.1 139.1</gml:posList></gml:LineStringSegment></gml:segments></gml:Curve></loc>
    <type>真幅道路</type>
  </{kind}>
</Dataset>
"#
    )
}

#[cfg(test)]
fn zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
    zumen_zip::write(entries).unwrap()
}

#[cfg(test)]
fn lift(src: &[u8]) -> (GeoModel, Vec<Code>) {
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let model = {
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        zumen_fgd::lift_archive(src, &mut cx).unwrap()
    };
    (model, diag.iter().map(|d| d.code()).collect())
}

const ROAD: &str = "FG-GML-533945-RdEdg-20250801-0001.xml";
const RAIL: &str = "FG-GML-533945-RailCL-20250801-0001.xml";

#[test]
fn detects_archives_by_the_names_of_their_entries() {
    let road = gml("RdEdg", "K1_1", "fguuid:jgd2024.bl");
    let archive = zip(&[(ROAD, road.as_bytes())]);
    assert!(zumen_fgd::ARCHIVE.iter().any(|rule| rule.matches(&archive)));
    // ほかの ZIP（SFZ や、名前の違う XML）は基盤地図情報の書庫としない
    let other = zip(&[("D0PL001Z.SFC", b"x"), ("data.xml", b"<a/>")]);
    assert!(!zumen_fgd::ARCHIVE.iter().any(|rule| rule.matches(&other)));
}

#[test]
fn merges_the_gml_entries_into_one_model() {
    let road = gml("RdEdg", "K1_1", "fguuid:jgd2024.bl");
    let rail = gml("RailCL", "K1_1", "fguuid:jgd2024.bl");
    // 全項目の書庫は、提供クラスごとの書庫を入れることがある
    let inner = zip(&[(RAIL, rail.as_bytes())]);
    let archive = zip(&[
        (ROAD, road.as_bytes()),
        ("readme.txt", b"note"),
        ("FG-GML-533945-08-20250801.zip", &inner),
    ]);
    let (model, codes) = lift(&archive);
    assert_eq!(codes, [codes::FGD_ENTRY_UNSUPPORTED]);
    assert_eq!(model.crs, Some(Crs::Geographic));
    assert_eq!(model.height, Some(VerticalCrs::Jgd2024));
    let ids: Vec<(&str, &str)> = model
        .features
        .iter()
        .map(|f| (f.id.as_str(), f.kind.as_str()))
        .collect();
    // 同じ gml:id でも、項目の名前で分かれる
    assert_eq!(
        ids,
        [
            ("FG-GML-533945-RdEdg-20250801-0001.xml#K1_1", "fgd:RdEdg"),
            (
                "FG-GML-533945-08-20250801.zip/FG-GML-533945-RailCL-20250801-0001.xml#K1_1",
                "fgd:RailCL"
            ),
        ]
    );
    assert!(model.features.iter().all(|f| f.geometries.len() == 1));
}

#[test]
fn drops_the_shapes_of_entries_in_another_coordinate_system() {
    let road = gml("RdEdg", "K1_1", "fguuid:jgd2024.bl");
    let rail = gml("RailCL", "K2_1", "fguuid:jgd2011.bl");
    let archive = zip(&[(ROAD, road.as_bytes()), (RAIL, rail.as_bytes())]);
    let (model, codes) = lift(&archive);
    assert_eq!(codes, [codes::FGD_CRS_UNSUPPORTED]);
    assert_eq!(model.height, Some(VerticalCrs::Jgd2024));
    assert_eq!(model.features[0].geometries.len(), 1);
    assert!(model.features[1].geometries.is_empty());
    assert_eq!(model.features.len(), 2);
}
