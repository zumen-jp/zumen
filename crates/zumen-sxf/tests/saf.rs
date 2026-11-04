//! 属性ファイル（SAF）を読み、属性ファイル用の属性付加機構の要素に結ぶ（決定 0249、#386）。入力は、どれもテストの
//! 中で作る。

use zumen_base::{Budget, CancelToken, Code, Cx, Diagnostics, codes};
use zumen_drawing::{Drawing2D, FieldValue};
use zumen_sdk::testkit;
use zumen_sxf::saf::{self, AttributeFile};
use zumen_sxf::{Sxf, SxfFile};

/// 附属書の付録 A の形の属性ファイル。図形 10 は 2 つの属性セットとグループを、図形 21 は図面にない
const SAF: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE SxfAttributeXML SYSTEM "SXFV31.DTD">
<SxfAttributeXML version="3.1" date="2026-11-03" sxfFile="D0PL001Z.SFC" application="XX-CAD">
  <AttributeSet name="計画設計" version="1.0" designedBy="○○協会">1</AttributeSet>
  <AttributeSet name="土量計算方法" version="1.0" designedBy="△△協会">2</AttributeSet>
  <Figure id="10" name="道路横断図形">
    <AttrSetRef id="1">
      <AttrGroup name="測点">
        <Attr name="測点名" type="STR">No.1</Attr>
        <Attr name="現地盤高" type="LEN" unit="m">102.0400</Attr>
      </AttrGroup>
    </AttrSetRef>
    <AttrSetRef id="2">
      <Attr name="計算方法" type="STR">平均断面法</Attr>
      <Attr name="ターゲット">11</Attr>
    </AttrSetRef>
  </Figure>
  <Figure id="21" name="切盛土エリア">
    <AttrSetRef id="1">
      <Attr name="切盛土区分" type="STR">切土</Attr>
    </AttrSetRef>
  </Figure>
</SxfAttributeXML>
"#;

#[cfg(test)]
fn sfc(features: &[&[u8]]) -> Vec<u8> {
    let mut src = b"ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION(('SCADEC level2 feature_mode'),'2;1');\nFILE_SCHEMA(('ASSOCIATIVE_DRAUGHTING'));\nENDSEC;\nDATA;\n".to_vec();
    for (at, feature) in features.iter().enumerate() {
        src.extend_from_slice(format!("/*SXF3\n#{} = ", (at + 1) * 10).as_bytes());
        src.extend_from_slice(feature);
        src.extend_from_slice(b"\nSXF3*/\n");
    }
    src.extend_from_slice(b"ENDSEC;\nEND-ISO-10303-21;\n");
    src
}

/// 図形識別番号 10 と 30 の作図グループを 1 つずつ置いた図面
#[cfg(test)]
fn drawing_source() -> Vec<u8> {
    sfc(&[
        b"layer_feature(\\'L\\','1')",
        b"line_feature('1','1','1','1','0','0','1','0')",
        b"line_feature('1','1','1','1','0','0','0','1')",
        b"sfig_org_feature(\\'$$ATRF$$10$$D0PL001Z.SAF\\','3')",
        b"line_feature('1','1','1','1','0','0','2','2')",
        b"sfig_org_feature(\\'$$ATRF$$30\\','3')",
        b"sfig_locate_feature('1',\\'$$ATRF$$10$$D0PL001Z.SAF\\','0','0','0','1','1')",
        b"sfig_locate_feature('1',\\'$$ATRF$$30\\','0','0','0','1','1')",
        b"drawing_sheet_feature(\\'S\\','3','1','0','0')",
    ])
}

#[cfg(test)]
fn read(src: &str) -> (Result<AttributeFile, zumen_sdk::Error>, Vec<Code>) {
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let file = saf::read(
        src.as_bytes(),
        &mut Cx::new(&mut budget, &mut diag, &cancel),
    );
    (file, diag.iter().map(|d| d.code()).collect())
}

#[cfg(test)]
fn lift(attributes: Option<&AttributeFile>) -> (Drawing2D, Vec<Code>) {
    let file: SxfFile = testkit::read::<Sxf>(&drawing_source()).0.unwrap();
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let drawing = file
        .lift_with(attributes, &mut Cx::new(&mut budget, &mut diag, &cancel))
        .unwrap();
    (drawing, diag.iter().map(|d| d.code()).collect())
}

#[test]
fn reads_sets_figures_and_grouped_attributes() {
    let (file, codes) = read(SAF);
    let file = file.unwrap();
    assert!(codes.is_empty(), "{codes:?}");
    assert_eq!(file.version, "3.1");
    assert_eq!(file.sxf_file, "D0PL001Z.SFC");
    assert_eq!(file.sets.len(), 2);
    assert_eq!(file.sets[1].id, "2");
    assert_eq!(file.sets[1].designed_by, "△△協会");
    let figure = &file.figures[0];
    assert_eq!(
        (figure.id.as_str(), figure.name.as_str()),
        ("10", "道路横断図形")
    );
    assert_eq!(figure.attributes.len(), 4);
    let height = &figure.attributes[1];
    assert_eq!(height.set, "1");
    assert_eq!(height.groups, ["測点"]);
    assert_eq!(height.name, "現地盤高");
    assert_eq!(height.kind.as_deref(), Some("LEN"));
    assert_eq!(height.unit.as_deref(), Some("m"));
    assert_eq!(height.value, "102.0400");
    // 既定義属性は属性タイプを省略できる
    let target = &figure.attributes[3];
    assert_eq!((target.kind.clone(), target.value.as_str()), (None, "11"));
}

#[test]
fn reports_broken_attribute_files() {
    let (file, codes) = read("<Other/>");
    assert!(file.is_err());
    assert_eq!(codes, [codes::SXF_SAF_VALUE_INVALID]);

    // 省略できないタグ属性がない図形と、重なる図形識別番号と、知らない要素
    let src = SAF
        .replace(
            r#"<Figure id="21" name="切盛土エリア">"#,
            r#"<Figure id="10">"#,
        )
        .replace("</SxfAttributeXML>", "<Other/></SxfAttributeXML>");
    let (file, codes) = read(&src);
    assert_eq!(
        codes,
        [
            codes::SXF_SAF_VALUE_INVALID,
            codes::SXF_SAF_ELEMENT_UNSUPPORTED
        ]
    );
    assert_eq!(file.unwrap().figures.len(), 1);
}

#[test]
fn binds_attributes_to_the_elements_of_the_figure() {
    let (file, _) = read(SAF);
    let file = file.unwrap();
    let (drawing, codes) = lift(Some(&file));
    // 図面の 30 は属性ファイルになく、属性ファイルの 21 は図面にない
    assert_eq!(
        codes,
        [codes::SXF_SAF_FIGURE_MISSING, codes::SXF_SAF_FIGURE_DROPPED]
    );
    let attributes: Vec<_> = drawing
        .extensions
        .iter()
        .filter(|e| e.namespace == "sxf.saf")
        .collect();
    // 作図グループの 2 本の線に、図形 10 の 4 つの属性ずつ
    assert_eq!(attributes.len(), 8);
    let atrf = drawing
        .extensions
        .iter()
        .find(|e| e.namespace == "sxf.attribute")
        .unwrap();
    assert_eq!(attributes[0].target, atrf.target);
    assert!(attributes.iter().all(|e| e.is_current(&drawing)));
    let height = attributes[1];
    assert_eq!(height.name, "現地盤高");
    assert_eq!(height.version.as_deref(), Some("3.1"));
    let fields: Vec<(&str, FieldValue)> = height
        .fields
        .iter()
        .map(|f| (f.key.as_ref(), f.value.clone()))
        .collect();
    let text = |s: &str| FieldValue::Text(s.into());
    assert_eq!(
        fields,
        [
            ("figure", text("10")),
            ("figure_name", text("道路横断図形")),
            ("set", text("1")),
            ("set_name", text("計画設計")),
            ("set_version", text("1.0")),
            ("designed_by", text("○○協会")),
            ("group", text("測点")),
            ("type", text("LEN")),
            ("unit", text("m")),
            ("value", text("102.0400")),
        ]
    );
}

#[test]
fn lifts_without_an_attribute_file_as_before() {
    let (drawing, codes) = lift(None);
    assert!(codes.is_empty(), "{codes:?}");
    assert!(drawing.extensions.iter().all(|e| e.namespace != "sxf.saf"));
}
