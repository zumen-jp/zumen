//! 試験の中で作った基盤地図情報（GML）を読み、地理の領域モデルに移す（決定 0233、#378）。

use zumen_base::{Budget, CancelToken, Code, Cx, Diagnostics, Kind, Limit, Limits, codes};
use zumen_crs::{Crs, GeoPoint, VerticalCrs};
use zumen_fgd::Fgd;
use zumen_geo::{Form, GeoModel, Location, Position, Value};
use zumen_geom3d::Shape;
use zumen_sdk::{Codec, Lift};

/// 基本項目の地物。道路縁（線要素 L）・建築物（面要素 A）・標高点（点要素 P）・測量の基準点と、境界の曲線を
/// 参照する行政区画
const BASIC: &str = r##"<?xml version="1.0" encoding="UTF-8"?>
<Dataset xmlns="http://fgd.gsi.go.jp/spec/2008/FGD_GMLSchema"
  xmlns:gml="http://www.opengis.net/gml/3.2"
  xmlns:xlink="http://www.w3.org/1999/xlink"
  gml:id="Dataset1">
  <description>基盤地図情報ダウンロードデータ（GML版）</description>
  <RdEdg gml:id="K1_1">
    <fid>fgoid:10-00100-11-6640-1</fid>
    <lfSpanFr gml:id="K1_1-1"><gml:timePosition>2025-08-01</gml:timePosition></lfSpanFr>
    <devDate gml:id="K1_1-2"><gml:timePosition>2025-08-01</gml:timePosition></devDate>
    <orgGILvl>2500</orgGILvl>
    <vis>表示</vis>
    <loc>
      <gml:Curve gml:id="K1_1-g" srsName="fguuid:jgd2024.bl">
        <gml:segments>
          <gml:LineStringSegment>
            <gml:posList>
35.0 139.0
35.0 139.001
35.001 139.001
</gml:posList>
          </gml:LineStringSegment>
        </gml:segments>
      </gml:Curve>
    </loc>
    <type>真幅道路</type>
    <admOffice>市区町村</admOffice>
  </RdEdg>
  <BldA gml:id="K2_1">
    <fid>fgoid:10-00100-11-6640-2</fid>
    <lfSpanFr gml:id="K2_1-1"><gml:timePosition>2025-08-01</gml:timePosition></lfSpanFr>
    <orgGILvl>2500</orgGILvl>
    <area>
      <gml:Surface gml:id="K2_1-g" srsName="fguuid:jgd2024.bl">
        <gml:patches>
          <gml:PolygonPatch>
            <gml:exterior>
              <gml:Ring>
                <gml:curveMember>
                  <gml:Curve gml:id="K2_1-c">
                    <gml:segments>
                      <gml:LineStringSegment>
                        <gml:posList>35.0 139.0 35.0 139.0001 35.0001 139.0001 35.0001 139.0 35.0 139.0</gml:posList>
                      </gml:LineStringSegment>
                    </gml:segments>
                  </gml:Curve>
                </gml:curveMember>
              </gml:Ring>
            </gml:exterior>
          </gml:PolygonPatch>
        </gml:patches>
      </gml:Surface>
    </area>
    <type>普通建物</type>
    <compL xlink:href="#K3_1"/>
  </BldA>
  <AdmBdry gml:id="K4_1">
    <fid>fgoid:10-00100-11-6640-4</fid>
    <lfSpanFr gml:id="K4_1-1"><gml:timePosition>2025-08-01</gml:timePosition></lfSpanFr>
    <orgGILvl>2500</orgGILvl>
    <loc>
      <gml:Curve gml:id="K4_1-g" srsName="fguuid:jgd2024.bl">
        <gml:segments>
          <gml:LineStringSegment><gml:posList>35.1 139.1 35.1 139.2 35.2 139.2</gml:posList></gml:LineStringSegment>
        </gml:segments>
      </gml:Curve>
    </loc>
    <type>町村・指定都市の区界</type>
  </AdmBdry>
  <AdmArea gml:id="K5_1">
    <fid>fgoid:10-00100-11-6640-5</fid>
    <lfSpanFr gml:id="K5_1-1"><gml:timePosition>2025-08-01</gml:timePosition></lfSpanFr>
    <orgGILvl>2500</orgGILvl>
    <area>
      <gml:Surface gml:id="K5_1-g" srsName="fguuid:jgd2024.bl">
        <gml:patches>
          <gml:PolygonPatch>
            <gml:exterior>
              <gml:Ring>
                <gml:curveMember xlink:href="#K4_1-g"/>
                <gml:curveMember>
                  <gml:OrientableCurve orientation="-">
                    <gml:baseCurve xlink:href="#K6_1-g"/>
                  </gml:OrientableCurve>
                </gml:curveMember>
              </gml:Ring>
            </gml:exterior>
          </gml:PolygonPatch>
        </gml:patches>
      </gml:Surface>
    </area>
    <type>町村・指定都市の区</type>
    <name>千代田区</name>
    <admCode>13101</admCode>
  </AdmArea>
  <AdmBdry gml:id="K6_1">
    <fid>fgoid:10-00100-11-6640-6</fid>
    <lfSpanFr gml:id="K6_1-1"><gml:timePosition>2025-08-01</gml:timePosition></lfSpanFr>
    <orgGILvl>2500</orgGILvl>
    <loc>
      <gml:Curve gml:id="K6_1-g" srsName="fguuid:jgd2024.bl">
        <gml:segments>
          <gml:LineStringSegment><gml:posList>35.1 139.1 35.2 139.1 35.2 139.2</gml:posList></gml:LineStringSegment>
        </gml:segments>
      </gml:Curve>
    </loc>
    <type>町村・指定都市の区界</type>
  </AdmBdry>
  <ElevPt gml:id="K7_1">
    <fid>fgoid:10-00100-11-6640-7</fid>
    <lfSpanFr gml:id="K7_1-1"><gml:timePosition>2025-08-01</gml:timePosition></lfSpanFr>
    <orgGILvl>2500</orgGILvl>
    <pos>
      <gml:Point gml:id="K7_1-g" srsName="fguuid:jgd2024.bl"><gml:pos>35.05 139.05</gml:pos></gml:Point>
    </pos>
    <type>標高点（測点）</type>
    <alti>12.3</alti>
  </ElevPt>
  <GCP gml:id="K8_1">
    <fid>fgoid:10-00100-11-6640-8</fid>
    <lfSpanFr gml:id="K8_1-1"><gml:timePosition>2025-08-01</gml:timePosition></lfSpanFr>
    <orgGILvl>KJ13</orgGILvl>
    <pos>
      <gml:Point gml:id="K8_1-g" srsName="fguuid:jgd2024.bl"><gml:pos>35.06 139.06</gml:pos></gml:Point>
    </pos>
    <orgName>国土地理院</orgName>
    <type>三角点</type>
    <gcpCode>TR35339000001</gcpCode>
    <name>丸の内</name>
    <B>126216.1234</B>
    <L>500616.5</L>
    <alti>5.67</alti>
    <altiAcc>2</altiAcc>
  </GCP>
</Dataset>
"##;

/// DEM 区画。3 列 2 行の格子で、北西の端の 1 つは「データなし」、ほかは地表面と海水面
const DEM: &str = r##"<?xml version="1.0" encoding="UTF-8"?>
<Dataset xmlns="http://fgd.gsi.go.jp/spec/2008/FGD_GMLSchema"
  xmlns:gml="http://www.opengis.net/gml/3.2" gml:id="Dataset1">
  <description>基盤地図情報ダウンロードデータ（GML版）</description>
  <DEM gml:id="DEM001">
    <fid>fgoid:10-00100-15-60101-533945294</fid>
    <lfSpanFr gml:id="DEM001-1"><gml:timePosition>2025-08-01</gml:timePosition></lfSpanFr>
    <devDate gml:id="DEM001-2"><gml:timePosition>2025-08-01</gml:timePosition></devDate>
    <orgGILvl>0</orgGILvl>
    <orgMDId>R07H0001</orgMDId>
    <type>5mメッシュ（標高）</type>
    <mesh>53394529</mesh>
    <coverage gml:id="DEM001-3">
      <gml:boundedBy>
        <gml:Envelope srsName="fguuid:jgd2024.bl">
          <gml:lowerCorner>35.0 139.0</gml:lowerCorner>
          <gml:upperCorner>35.002 139.003</gml:upperCorner>
        </gml:Envelope>
      </gml:boundedBy>
      <gml:gridDomain>
        <gml:Grid gml:id="DEM001-4" dimension="2">
          <gml:limits>
            <gml:GridEnvelope>
              <gml:low>0 0</gml:low>
              <gml:high>2 1</gml:high>
            </gml:GridEnvelope>
          </gml:limits>
          <gml:axisLabels>x y</gml:axisLabels>
        </gml:Grid>
      </gml:gridDomain>
      <gml:rangeSet>
        <gml:DataBlock>
          <gml:rangeParameters><gml:QuantityList uom="DEM構成点"/></gml:rangeParameters>
          <gml:tupleList>
データなし,-9999.
地表面,10.5
地表面,11.
地表面,12.25
海水面,0.
地表面,13.
</gml:tupleList>
        </gml:DataBlock>
      </gml:rangeSet>
      <gml:coverageFunction>
        <gml:GridFunction>
          <gml:sequenceRule order="+x-y">Linear</gml:sequenceRule>
          <gml:startPoint>0 0</gml:startPoint>
        </gml:GridFunction>
      </gml:coverageFunction>
    </coverage>
  </DEM>
</Dataset>
"##;

fn lift_with(src: &str, budget: Budget) -> (Result<GeoModel, zumen_sdk::Error>, Vec<Code>) {
    let mut budget = budget;
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let model = {
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        Fgd::read(src.as_bytes(), &mut cx).and_then(|file| file.lift(&mut cx))
    };
    (model, diag.iter().map(|d| d.code()).collect())
}

#[cfg(test)]
fn lift(src: &str) -> (GeoModel, Vec<Code>) {
    let (model, codes) = lift_with(src, Budget::default());
    (model.unwrap(), codes)
}

fn geo(latitude: f64, longitude: f64) -> Position {
    Position::Geo(GeoPoint {
        latitude,
        longitude,
    })
}

#[test]
fn detects_the_dataset_root() {
    assert!(Fgd::SIGNATURES.iter().any(|s| s.matches(BASIC.as_bytes())));
    assert!(!Fgd::SIGNATURES.iter().any(|s| s.matches(b"<Dataset/>")));
}

#[test]
fn reads_features_with_their_kind_code_and_attributes() {
    let (model, codes) = lift(BASIC);
    assert!(codes.is_empty(), "{codes:?}");
    assert_eq!(model.crs, Some(Crs::Geographic));
    assert_eq!(model.height, Some(VerticalCrs::Jgd2024));
    let kinds: Vec<&str> = model.features.iter().map(|f| f.kind.as_str()).collect();
    assert_eq!(
        kinds,
        [
            "fgd:RdEdg",
            "fgd:BldA",
            "fgd:AdmBdry",
            "fgd:AdmArea",
            "fgd:AdmBdry",
            "fgd:ElevPt",
            "fgd:GCP"
        ]
    );

    let road = &model.features[0];
    assert_eq!(road.id, "K1_1");
    assert_eq!(road.code.as_deref(), Some("真幅道路"));
    let attribute = |name: &str| {
        road.attributes
            .iter()
            .find(|a| a.name == name)
            .map(|a| a.value.clone())
    };
    assert_eq!(
        attribute("fgd:fid"),
        Some(Value::Text("fgoid:10-00100-11-6640-1".into()))
    );
    assert_eq!(
        attribute("fgd:lfSpanFr"),
        Some(Value::Date("2025-08-01".into()))
    );
    assert_eq!(
        attribute("fgd:orgGILvl"),
        Some(Value::Code {
            space: None,
            value: "2500".into()
        })
    );
    assert_eq!(
        attribute("fgd:admOffice"),
        Some(Value::Code {
            space: None,
            value: "市区町村".into()
        })
    );
    let Form::Line(points) = &road.geometries[0].form else {
        panic!("{:?}", road.geometries);
    };
    assert_eq!(points.len(), 3);
    assert_eq!(points[1].position, geo(35.0, 139.001));
    assert_eq!(points[1].height, None);
    assert_eq!(road.geometries[0].lod, None);
    // 出所は元の要素のバイトの範囲
    let start = BASIC.find("<RdEdg").unwrap() as u64;
    assert_eq!(road.source.bytes().map(|b| b.start), Some(start));

    // 行政コードは数字の並びでも文字のまま。関連は参照の文字
    let area = &model.features[3];
    assert_eq!(area.name.as_deref(), Some("千代田区"));
    assert!(
        area.attributes
            .iter()
            .any(|a| a.name == "fgd:admCode" && a.value == Value::Text("13101".into()))
    );
    let building = &model.features[1];
    assert!(
        building
            .attributes
            .iter()
            .any(|a| a.name == "fgd:compL" && a.value == Value::Text("#K3_1".into()))
    );

    // 測量の基準点の B・L・標高は実数、標高値有効小数桁数は整数
    let gcp = &model.features[6];
    assert_eq!(gcp.name.as_deref(), Some("丸の内"));
    assert!(
        gcp.attributes
            .iter()
            .any(|a| a.name == "fgd:B" && a.value == Value::Real(126216.1234))
    );
    assert!(
        gcp.attributes
            .iter()
            .any(|a| a.name == "fgd:altiAcc" && a.value == Value::Integer(2))
    );
}

#[test]
fn reads_surfaces_and_rings_of_referenced_curves() {
    let (model, _) = lift(BASIC);
    // 面要素 A。輪の閉じる点は除く
    let Form::Area { outer, holes } = &model.features[1].geometries[0].form else {
        panic!();
    };
    assert_eq!(outer.len(), 4);
    assert!(holes.is_empty());

    // 境界の曲線を参照してつなぐ。マイナス方向の参照は逆に辿り、つなぎ目と閉じる点は除く
    let Form::Area { outer, .. } = &model.features[3].geometries[0].form else {
        panic!();
    };
    let positions: Vec<Position> = outer.iter().map(|l| l.position).collect();
    assert_eq!(
        positions,
        [
            geo(35.1, 139.1),
            geo(35.1, 139.2),
            geo(35.2, 139.2),
            geo(35.2, 139.1)
        ]
    );
}

#[test]
fn gives_points_the_altitude_of_the_feature() {
    let (model, _) = lift(BASIC);
    let Form::Point(Location { position, height }) = model.features[5].geometries[0].form else {
        panic!();
    };
    assert_eq!(position, geo(35.05, 139.05));
    assert_eq!(height, Some(12.3));
}

#[test]
fn reports_missing_references_and_other_coordinate_systems() {
    let src = BASIC
        .replace(
            "<gml:baseCurve xlink:href=\"#K6_1-g\"/>",
            "<gml:baseCurve xlink:href=\"#none\"/>",
        )
        .replace(
            "<gml:Point gml:id=\"K7_1-g\" srsName=\"fguuid:jgd2024.bl\">",
            "<gml:Point gml:id=\"K7_1-g\" srsName=\"fguuid:jgd2000.bl\">",
        );
    let (model, codes) = lift(&src);
    assert_eq!(
        codes,
        [
            codes::FGD_REFERENCE_UNSUPPORTED,
            codes::FGD_VALUE_INVALID,
            codes::FGD_CRS_UNSUPPORTED
        ]
    );
    // 輪が作れない面と、座標系の違う点は移さない。地物と属性は移す
    assert!(model.features[3].geometries.is_empty());
    assert!(model.features[5].geometries.is_empty());
    assert_eq!(model.features[5].attributes.len(), 4);
}

#[test]
fn reports_an_unknown_document_coordinate_system() {
    let src = BASIC.replace("fguuid:jgd2024.bl", "fguuid:jgd2000.bl");
    let (model, codes) = lift(&src);
    assert_eq!(codes, [codes::FGD_CRS_UNSUPPORTED]);
    assert_eq!(model.crs, None);
    assert!(model.features.iter().all(|f| f.geometries.is_empty()));
    assert_eq!(model.features.len(), 7);
}

#[test]
fn reads_jgd2011_data_with_its_heights() {
    let src = BASIC.replace("fguuid:jgd2024.bl", "fguuid:jgd2011.bl");
    let (model, codes) = lift(&src);
    assert!(codes.is_empty(), "{codes:?}");
    assert_eq!(model.crs, Some(Crs::Geographic));
    assert_eq!(model.height, Some(VerticalCrs::Jgd2011));
}

#[test]
fn makes_triangles_between_the_centers_of_the_dem_cells() {
    let (model, codes) = lift(DEM);
    assert_eq!(codes, [codes::FGD_POINT_KIND_DROPPED]);
    let dem = &model.features[0];
    assert_eq!(dem.kind, "fgd:DEM");
    assert_eq!(dem.code.as_deref(), Some("5mメッシュ（標高）"));
    assert!(
        dem.attributes
            .iter()
            .any(|a| a.name == "fgd:mesh" && a.value == Value::Text("53394529".into()))
    );
    let counts: Vec<(&str, &Value)> = dem
        .attributes
        .iter()
        .filter(|a| a.name.starts_with("fgd:coverage/"))
        .map(|a| (a.name.as_str(), &a.value))
        .collect();
    assert_eq!(
        counts,
        [
            ("fgd:coverage/データなし", &Value::Integer(1)),
            ("fgd:coverage/地表面", &Value::Integer(4)),
            ("fgd:coverage/海水面", &Value::Integer(1)),
        ]
    );
    let Form::Solid(Shape::Triangles {
        positions,
        triangles,
        closed,
    }) = &dem.geometries[0].form
    else {
        panic!("{:?}", dem.geometries);
    };
    assert!(!closed);
    // 北西の端（データなし）を除く 5 点。セルの大きさは緯度 0.001・経度 0.001 で、点はセルの中心
    assert_eq!(positions.len(), 5);
    let close = |a: [f64; 3], b: [f64; 3]| a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-9);
    assert!(close(positions[0], [35.0015, 139.0015, 10.5]));
    assert!(close(positions[2], [35.0005, 139.0005, 12.25]));
    assert!(close(positions[4], [35.0005, 139.0025, 13.0]));
    // 西の 2 列の四角形は北西の点がないので 1 つ、東の 2 列は 2 つの三角形。どれも上から見て反時計回り
    assert_eq!(triangles, &[[0, 3, 2], [0, 1, 3], [1, 4, 3]]);
}

#[test]
fn reports_coverages_it_cannot_read() {
    // 値の並びが基盤地図情報の "+x -y" でない
    let src = DEM.replace("order=\"+x-y\"", "order=\"+x+y\"");
    let (model, codes) = lift(&src);
    assert_eq!(codes, [codes::FGD_ELEMENT_UNSUPPORTED]);
    assert!(model.features[0].geometries.is_empty());

    // ジオイド・モデルの被覆は標高でないので移さない
    let src = DEM.replace("<DEM ", "<DGHM ").replace("</DEM>", "</DGHM>");
    let (model, codes) = lift(&src);
    assert_eq!(codes, [codes::FGD_ELEMENT_UNSUPPORTED]);
    assert!(model.features[0].geometries.is_empty());

    // 値が格子より多い
    let src = DEM.replace("地表面,13.\n", "地表面,13.\n地表面,14.\n");
    let (_, codes) = lift(&src);
    assert!(codes.contains(&codes::FGD_VALUE_INVALID), "{codes:?}");
}

#[test]
fn stops_at_the_budget_for_the_grid() {
    let budget = Budget::new(Limits::default().with(Kind::Segments, Limit::Finite(4)));
    let (model, codes) = lift_with(DEM, budget);
    assert!(model.is_err());
    assert_eq!(codes, [codes::BUDGET_SEGMENTS_EXCEEDED]);
}
