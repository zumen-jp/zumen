//! 試験の中で作ったボーリング交換用データ（個人情報のない合成のもの）を読む（#390）。要素の名前は要領の
//! 付属資料の構造図による。値は試験のために決めたもの。

use zumen_base::{Budget, CancelToken, Code, Cx, Diagnostics, codes};
use zumen_crs::{HeightDatum, Realization};
use zumen_geology::{Datum, read};

#[cfg(test)]
fn bed(datum: &str, end: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE ボーリング情報 SYSTEM "BED0500.DTD">
<ボーリング情報 DTD_version="5.00">
  <基礎情報>
    <適用規格>JIS A 0205-2019</適用規格>
    <公開フラグ><公開フラグ_コード>1</公開フラグ_コード><公開フラグ_備考/></公開フラグ>
  </基礎情報>
  <標題情報>
    <調査基本情報>
      <調査名>試験の地質調査</調査名>
      <調査目的>01</調査目的>
      <ボーリング名>No.1</ボーリング名>
      <ボーリング総数>2</ボーリング総数>
      <ボーリング連番>1</ボーリング連番>
    </調査基本情報>
    <経度緯度情報>
      <経度_度>139</経度_度><経度_分>45</経度_分><経度_秒>0.0000</経度_秒>
      <緯度_度>35</緯度_度><緯度_分>40</緯度_分><緯度_秒>30.0000</緯度_秒>
      <取得方法コード>02</取得方法コード>
      <読取精度コード>1</読取精度コード>
      <測地系>{datum}</測地系>
    </経度緯度情報>
    <調査位置><調査位置住所>試験県試験市</調査位置住所></調査位置>
    <調査期間>
      <調査期間_開始年月日>2026-10-01</調査期間_開始年月日>
      <調査期間_終了年月日>{end}</調査期間_終了年月日>
    </調査期間>
    <調査会社><調査会社_名称>試験</調査会社_名称></調査会社>
    <ボーリング基本情報>
      <孔口標高>12.30</孔口標高>
      <総削孔長>10.00</総削孔長>
      <柱状図様式>11</柱状図様式>
      <角度>90.00</角度>
    </ボーリング基本情報>
  </標題情報>
  <コア情報>
    <工学的地質区分名現場土質名>
      <工学的地質区分名現場土質名_下端深度>1.80</工学的地質区分名現場土質名_下端深度>
      <工学的地質区分名現場土質名_工学的地質区分名現場土質名>盛土</工学的地質区分名現場土質名_工学的地質区分名現場土質名>
      <工学的地質区分名現場土質名_工学的地質区分名現場土質名記号>B</工学的地質区分名現場土質名_工学的地質区分名現場土質名記号>
    </工学的地質区分名現場土質名>
    <工学的地質区分名現場土質名>
      <工学的地質区分名現場土質名_下端深度>10.00</工学的地質区分名現場土質名_下端深度>
      <工学的地質区分名現場土質名_工学的地質区分名現場土質名>砂</工学的地質区分名現場土質名_工学的地質区分名現場土質名>
    </工学的地質区分名現場土質名>
    <色調><色調_下端深度>10.00</色調_下端深度><色調_色調名>灰</色調_色調名></色調>
    <観察記事><観察記事_上端深度>0.00</観察記事_上端深度><観察記事_下端深度>1.80</観察記事_下端深度><観察記事_記事>礫を含む</観察記事_記事></観察記事>
    <標準貫入試験>
      <標準貫入試験_開始深度>1.15</標準貫入試験_開始深度>
      <標準貫入試験_合計打撃回数>3</標準貫入試験_合計打撃回数>
      <標準貫入試験_合計貫入量>450</標準貫入試験_合計貫入量>
    </標準貫入試験>
    <標準貫入試験>
      <標準貫入試験_開始深度>2.15</標準貫入試験_開始深度>
      <標準貫入試験_合計打撃回数>abc</標準貫入試験_合計打撃回数>
      <標準貫入試験_合計貫入量>300</標準貫入試験_合計貫入量>
    </標準貫入試験>
    <孔内水位>
      <孔内水位_測定年月日>2026-10-02</孔内水位_測定年月日>
      <孔内水位_削孔状況コード>1</孔内水位_削孔状況コード>
      <孔内水位_孔内水位>-0.50</孔内水位_孔内水位>
      <孔内水位_水位種別備考>自噴</孔内水位_水位種別備考>
    </孔内水位>
    <孔内水位>
      <孔内水位_測定年月日>2026-10-03</孔内水位_測定年月日>
      <孔内水位_削孔状況コード>3</孔内水位_削孔状況コード>
      <孔内水位_孔内水位/>
    </孔内水位>
    <コア採取率><コア採取率_下端深度>10.00</コア採取率_下端深度></コア採取率>
  </コア情報>
</ボーリング情報>
"#
    )
}

#[cfg(test)]
fn with_cx<T>(f: impl FnOnce(&mut Cx<'_>) -> T) -> (T, Vec<(Code, Option<String>)>) {
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let out = {
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        f(&mut cx)
    };
    let found = diag
        .iter()
        .map(|d| {
            let name = match d.param("name") {
                Some(zumen_base::Value::Text(text)) => Some(text.to_string()),
                _ => None,
            };
            (d.code(), name)
        })
        .collect();
    (out, found)
}

#[test]
fn a_boring_is_read() {
    let src = bed("02", "2026-10-20");
    let (boring, diag) = with_cx(|cx| read(src.as_bytes(), cx));
    let boring = boring.unwrap();
    assert_eq!(boring.standards, ["JIS A 0205-2019"]);
    assert_eq!(boring.publication.as_deref(), Some("1"));
    assert_eq!(boring.survey.as_deref(), Some("試験の地質調査"));
    assert_eq!(boring.name.as_deref(), Some("No.1"));
    assert_eq!((boring.count, boring.serial), (Some(2), Some(1)));
    let position = boring.position.unwrap();
    assert!((position.longitude - 139.75).abs() < 1e-12);
    assert!((position.latitude - (35.0 + 40.0 / 60.0 + 30.0 / 3600.0)).abs() < 1e-12);
    assert_eq!(boring.datum, Some(Datum::Jgd2011));
    assert_eq!(boring.address.as_deref(), Some("試験県試験市"));
    let elevation = boring.elevation.unwrap();
    assert_eq!(elevation.value, 12.3);
    assert_eq!(elevation.datum, HeightDatum::TokyoBay);
    // 2025-04-01 より後の調査なので、測地成果2024 と仮定する
    assert_eq!(elevation.realization, Realization::Survey2024);
    assert_eq!((boring.length, boring.angle), (Some(10.0), Some(90.0)));
    let layers: Vec<(f64, &str, Option<&str>)> = boring
        .layers
        .iter()
        .map(|l| (l.bottom, l.name.as_str(), l.symbol.as_deref()))
        .collect();
    assert_eq!(layers, [(1.8, "盛土", Some("B")), (10.0, "砂", None)]);
    assert_eq!(boring.colors[0].name.as_deref(), Some("灰"));
    assert_eq!(boring.observations[0].text.as_deref(), Some("礫を含む"));
    // 打撃回数の読めない 2 つめは飛ばす
    assert_eq!(boring.penetrations.len(), 1);
    assert_eq!(
        (boring.penetrations[0].depth, boring.penetrations[0].blows),
        (1.15, 3)
    );
    assert_eq!(boring.penetrations[0].penetration, 450.0);
    assert_eq!(boring.water_levels.len(), 2);
    assert_eq!(boring.water_levels[0].depth, Some(-0.5));
    assert_eq!(boring.water_levels[0].note.as_deref(), Some("自噴"));
    assert_eq!(boring.water_levels[1].depth, None);
    assert_eq!(
        diag,
        [
            (
                codes::BORING_VALUE_INVALID,
                Some("標準貫入試験_合計打撃回数".to_string())
            ),
            (codes::CRS_HEIGHT_REALIZATION_ASSUMED, None),
            (
                codes::BORING_ELEMENT_UNSUPPORTED,
                Some("調査会社".to_string())
            ),
            (
                codes::BORING_ELEMENT_UNSUPPORTED,
                Some("コア採取率".to_string())
            ),
        ]
    );
}

#[test]
fn the_height_realization_follows_the_datum_and_the_date() {
    // JGD2024 なら測地成果2024 で、仮定しない
    let src = bed("03", "2026-10-20");
    let (boring, diag) = with_cx(|cx| read(src.as_bytes(), cx));
    let boring = boring.unwrap();
    assert_eq!(boring.datum, Some(Datum::Jgd2024));
    assert_eq!(
        boring.elevation.unwrap().realization,
        Realization::Survey2024
    );
    assert!(
        !diag
            .iter()
            .any(|(code, _)| *code == codes::CRS_HEIGHT_REALIZATION_ASSUMED)
    );
    // 2025-04-01 より前は測地成果2011。分からない測地系のコードは飛ばす
    let src = bed("09", "2024-12-01");
    let (boring, diag) = with_cx(|cx| read(src.as_bytes(), cx));
    let boring = boring.unwrap();
    assert_eq!(boring.datum, None);
    assert_eq!(
        boring.elevation.unwrap().realization,
        Realization::Survey2011
    );
    assert!(diag.contains(&(codes::BORING_VALUE_INVALID, Some("測地系".to_string()))));
}

#[test]
fn other_documents_are_rejected() {
    let (boring, diag) = with_cx(|cx| read(b"<BORING DTD_version=\"2.00\"/>", cx));
    assert!(boring.is_err());
    assert_eq!(diag, [(codes::BORING_FILE_INVALID, None)]);
    let (boring, diag) = with_cx(|cx| read(b"not xml", cx));
    assert!(boring.is_err());
    assert!(diag.contains(&(codes::BORING_FILE_INVALID, None)));
}
