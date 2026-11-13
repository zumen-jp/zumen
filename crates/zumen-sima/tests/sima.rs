//! SIMA（カンマ区切り `.sim` と JPGIS 版 XML）の読み書き・3 領域モデル変換・診断・予算のテスト（`#374`）。

#![cfg(test)]

use core::f64::consts::PI;

use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits, Ref, codes};
use zumen_civil::{
    Alignment, CivilModel, Horizontal as CivilHorizontal, HorizontalKind, Stationing,
    TransitionCurve,
};
use zumen_crs::{Axes, Crs, GeoPoint, MapPoint, Realization, Zone};
use zumen_drawing::{
    Drawing2D, Edge, Element, Inherit, LayerId, Length, ModelLength, ModelPoint, ModelSpace, Shape,
    SpaceId, Style, Unit,
};
use zumen_geo::{Feature, Form, GeoModel, Geometry, Location, Position, Value};
use zumen_geom2d::Ramp;
use zumen_sdk::{Codec, Error, FileModel, Lift, testkit};
use zumen_sima::{
    Sima, SimaFile, lower_civil_to_sima, lower_drawing_to_sima, lower_geo_to_sima,
    lower_sima_to_sima,
};

fn read_sima(src: &[u8]) -> (Result<SimaFile, Error>, Diagnostics) {
    let mut budget = Budget::default();
    let mut diags = Diagnostics::new();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diags, &cancel);
    let res = Sima::read(src, &mut cx);
    (res, diags)
}

fn lift_all(
    src: &[u8],
) -> (
    Result<(Drawing2D, CivilModel, GeoModel), Error>,
    Diagnostics,
) {
    let mut budget = Budget::default();
    let mut diags = Diagnostics::new();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diags, &cancel);
    let res = (|| {
        let file = Sima::read(src, &mut cx)?;
        let drawing: Drawing2D = file.lift(&mut cx)?;
        let civil: CivilModel = file.lift(&mut cx)?;
        let geo: GeoModel = file.lift(&mut cx)?;
        Ok((drawing, civil, geo))
    })();
    (res, diags)
}

#[test]
fn reads_csv_sim_points_parcels_and_routes_with_axis_swap() {
    let csv = concat!(
        "G00,01,境界確定測量,\r\n",
        "Z00,テスト用コメント,\r\n",
        "A00,\r\n",
        "A01,1,T1,100.250,200.500,12.340,\r\n",
        "A01,2,T2,110.250,200.500,12.500,\r\n",
        "A01,3,T3,110.250,220.500,,\r\n",
        "A01,4,T4,100.250,220.500,-0.000,\r\n",
        "A99,\r\n",
        "D00,10,1番1,1,\r\n",
        "B01,1,T1,\r\n",
        "B01,2,T2,\r\n",
        "B01,3,T3,\r\n",
        "B01,4,T4,\r\n",
        "D99,\r\n",
        "D00,11,道路境界,2,\r\n",
        "B01,1,,\r\n",
        "B01,,T3,\r\n",
        "D99,\r\n",
        "F00,1,本線,0.0,\r\n",
        "F01,舗装道路,\r\n",
        "C00,\r\n",
        "B01,1,T1,\r\n",
        "B01,2,T2,\r\n",
        "C99,\r\n",
        "C00,\r\n",
        "C01,50.0,\r\n",
        "B01,2,T2,\r\n",
        "B01,3,T3,\r\n",
        "C99,\r\n",
        "C00,\r\n",
        "C02,30.0,15.0,,60.0,\r\n",
        "B01,3,T3,\r\n",
        "B01,4,T4,\r\n",
        "C99,\r\n",
        "F99,\r\n",
    );

    // Shift_JIS に符号化しても UTF-8 BOM 付きでも同じ意味で読めること
    let (sjis, _, _) = encoding_rs::SHIFT_JIS.encode(csv);
    let mut utf8_bom = b"\xEF\xBB\xBF".to_vec();
    utf8_bom.extend_from_slice(csv.as_bytes());

    for bytes in [sjis.as_ref(), csv.as_bytes(), utf8_bom.as_slice()] {
        let (res, diags) = lift_all(bytes);
        let (drawing, civil, geo) = res.unwrap();

        // 通常の .sim は座標系を持たないため sima.crs.missing が出て GeoModel の幾何は空になる
        assert!(
            diags.iter().any(|d| d.code() == codes::SIMA_CRS_MISSING),
            "expected sima.crs.missing: {diags:?}"
        );
        assert_eq!(geo.crs, None);
        assert!(geo.features.iter().all(|f| f.geometries.is_empty()));
        assert_eq!(geo.features.len(), 7); // 測点 4 + 画地 2 + 路線 1

        // Drawing2D は実寸（東が X、北が Y）に移されること（X=100.25 北, Y=200.50 東 -> x=200.50, y=100.25）
        assert_eq!(drawing.spaces[0].axes, Axes::EastNorth);
        let elems = &drawing.spaces[0].elements;
        let first_pt = elems
            .iter()
            .find_map(|el| match &el.shape {
                Shape::Point(p) => Some(p.position),
                _ => None,
            })
            .unwrap();
        assert!((first_pt.x - 200.500).abs() < 1e-9);
        assert!((first_pt.y - 100.250).abs() < 1e-9);

        // 閉じた画地（closed=true）と開放折れ線（closed=false）が両方とも Polyline になっていること
        let polylines: Vec<_> = elems
            .iter()
            .filter_map(|el| match &el.shape {
                Shape::Polyline(poly) => Some(poly),
                _ => None,
            })
            .collect();
        assert_eq!(polylines.len(), 2);
        assert!(polylines[0].closed);
        assert_eq!(polylines[0].vertices.len(), 4);
        assert!(!polylines[1].closed);
        assert_eq!(polylines[1].vertices.len(), 2);

        // 路線が CivilModel の Alignment（直線 + 右円弧 + クロソイド）になっていること
        assert_eq!(civil.alignments.len(), 1);
        let al = &civil.alignments[0];
        assert_eq!(al.name.as_deref(), Some("本線"));
        assert_eq!(al.horizontal.len(), 3);
        assert!((al.horizontal[0].start.northing - 100.250).abs() < 1e-9);
        assert!((al.horizontal[0].start.easting - 200.500).abs() < 1e-9);
        assert!(matches!(al.horizontal[0].kind, HorizontalKind::Line));
        assert!(matches!(
            al.horizontal[1].kind,
            HorizontalKind::Arc { radius } if (radius - 50.0).abs() < 1e-9
        ));
        assert!(matches!(
            al.horizontal[2].kind,
            HorizontalKind::Transition {
                start_radius: None,
                end_radius: Some(r),
                curve: TransitionCurve::Clothoid,
            } if (r - 60.0).abs() < 1e-9
        ));

        // Drawing2D にも円弧とクロソイドの Curve 要素が出ていること
        let has_arc_edge = elems
            .iter()
            .any(|el| matches!(&el.shape, Shape::Curve(Edge::Arc { .. })));
        let has_clothoid_edge = elems
            .iter()
            .any(|el| matches!(&el.shape, Shape::Curve(Edge::Clothoid { .. })));
        assert!(has_arc_edge);
        assert!(has_clothoid_edge);
    }
}

#[test]
fn reads_jpgis_xml_schema_features_and_spatial_references() {
    let xml = r##"<?xml version="1.0" encoding="UTF-8"?>
<GI xmlns="http://www.jsima.or.jp/JSIMASchema/201206"
    xmlns:jps="http://www.gsi.go.jp/GIS/jpgis/standardSchemas2.1_2009-05"
    xmlns:xlink="http://www.w3.org/1999/xlink"
    version="1.0" timeStamp="2026-11-13T12:00:00">
  <exchangeMetadata>
    <datasetCitation><title>地籍・基準点交換データ</title></datasetCitation>
    <encodingRule><encodingRuleCitation><title>JPGIS 2.1</title></encodingRuleCitation><toolName>test</toolName></encodingRule>
  </exchangeMetadata>
  <dataset>
    <GenbaJoho>
      <Name>テスト地区</Name>
      <CoordinateSystem>9</CoordinateSystem>
      <Crs>3</Crs>
      <Worker>測量太郎</Worker>
    </GenbaJoho>
    <object>
      <jps:GM_Point id="pt1">
        <jps:position><jps:coordinate>1000.0 2000.0 15.5</jps:coordinate></jps:position>
      </jps:GM_Point>
      <jps:GM_Point id="pt2">
        <jps:position><jps:coordinate>1020.0 2000.0</jps:coordinate></jps:position>
      </jps:GM_Point>
      <jps:GM_Point id="pt3">
        <jps:position><jps:coordinate>1020.0 2030.0</jps:coordinate></jps:position>
      </jps:GM_Point>
      <jps:GM_Point id="pt4">
        <jps:position><jps:coordinate>1000.0 2030.0</jps:coordinate></jps:position>
      </jps:GM_Point>
      <jps:GM_Curve id="cv_outer">
        <jps:segment>
          <jps:GM_LineString>
            <jps:controlPoint>
              <jps:column><jps:indirect><jps:point idref="pt1"/></jps:indirect></jps:column>
              <jps:column><jps:indirect><jps:point xlink:href="#pt2"/></jps:indirect></jps:column>
              <jps:column><jps:indirect><jps:point idref="pt3"/></jps:indirect></jps:column>
              <jps:column><jps:indirect><jps:point idref="pt4"/></jps:indirect></jps:column>
              <jps:column><jps:indirect><jps:point idref="pt1"/></jps:indirect></jps:column>
            </jps:controlPoint>
          </jps:GM_LineString>
        </jps:segment>
      </jps:GM_Curve>
      <jps:GM_Curve id="cv_hole">
        <jps:segment>
          <jps:GM_LineString>
            <jps:controlPoint>
              <jps:column><jps:direct><jps:coordinate>1005.0 2005.0</jps:coordinate></jps:direct></jps:column>
              <jps:column><jps:direct><jps:coordinate>1005.0 2015.0</jps:coordinate></jps:direct></jps:column>
              <jps:column><jps:direct><jps:coordinate>1015.0 2015.0</jps:coordinate></jps:direct></jps:column>
              <jps:column><jps:direct><jps:coordinate>1005.0 2005.0</jps:coordinate></jps:direct></jps:column>
            </jps:controlPoint>
          </jps:GM_LineString>
        </jps:segment>
      </jps:GM_Curve>
      <jps:GM_OrientableCurve id="oc_rev">
        <jps:orientation>-</jps:orientation>
        <jps:primitive idref="cv_arc"/>
      </jps:GM_OrientableCurve>
      <jps:GM_Curve id="cv_arc">
        <jps:segment>
          <jps:GM_Arc>
            <jps:controlPoint>
              <jps:column><jps:direct><jps:coordinate>1000.0 2000.0</jps:coordinate></jps:direct></jps:column>
              <jps:column><jps:direct><jps:coordinate>1010.0 2010.0</jps:coordinate></jps:direct></jps:column>
              <jps:column><jps:direct><jps:coordinate>1020.0 2000.0</jps:coordinate></jps:direct></jps:column>
            </jps:controlPoint>
          </jps:GM_Arc>
        </jps:segment>
      </jps:GM_Curve>
      <jps:GM_Surface id="sf1">
        <jps:patch>
          <jps:GM_Polygon>
            <jps:boundary>
              <jps:exterior><jps:GM_Ring><jps:generator idref="cv_outer"/></jps:GM_Ring></jps:exterior>
              <jps:interior><jps:GM_Ring><jps:generator idref="cv_hole"/></jps:GM_Ring></jps:interior>
            </jps:boundary>
          </jps:GM_Polygon>
        </jps:patch>
      </jps:GM_Surface>
      <Sokuten id="sk1">
        <Number>1</Number>
        <Name>KBM1</Name>
        <Altitude>15.5</Altitude>
        <RefPoint idref="pt1"/>
      </Sokuten>
      <Kijyunten id="kj1">
        <Class>2級基準点</Class>
        <RefSokuten idref="sk1"/>
      </Kijyunten>
      <Suijyunten id="sj1">
        <Class>1級水準点</Class>
        <AveAltitude>15.512</AveAltitude>
        <RefSokuten idref="sk1"/>
      </Suijyunten>
      <Kakuchi id="kk1">
        <Number>100</Number>
        <Name>百番</Name>
        <RefSurface idref="sf1"/>
      </Kakuchi>
      <Chiban id="cb1">
        <Chimoku>宅地</Chimoku>
        <Area>600.0</Area>
        <Owner>山田</Owner>
        <Rotation>1</Rotation>
        <RefKakuchi idref="kk1"/>
      </Chiban>
      <KaihouKakuchi id="kh1">
        <Number>200</Number>
        <Name>水路中心</Name>
        <RefCurve idref="oc_rev"/>
      </KaihouKakuchi>
    </object>
  </dataset>
</GI>"##
        .as_bytes();

    let (res, diags) = lift_all(xml);
    assert!(
        diags
            .iter()
            .all(|d| d.code() == codes::SIMA_CURVE_APPROXIMATED),
        "unexpected diagnostics: {diags:?}"
    );
    let (drawing, civil, geo) = res.unwrap();

    let zone9 = Crs::Plane(Zone::new(9).unwrap());
    assert_eq!(geo.crs, Some(zone9));
    assert_eq!(civil.crs, Some(zone9));
    assert_eq!(civil.realization, Some(Realization::Survey2011));

    // Sokuten・Kijyunten・Suijyunten と Kakuchi・Chiban が上書きされず独立の地物として保たれること
    assert!(geo.features.iter().any(|f| f.kind == "sima:Sokuten"));
    assert!(geo.features.iter().any(|f| f.kind == "sima:Kijyunten"));
    assert!(geo.features.iter().any(|f| f.kind == "sima:Suijyunten"));
    assert!(geo.features.iter().any(|f| f.kind == "sima:Kakuchi"));

    // Chiban が Kakuchi に結び付き、穴つき Area 幾何・実数 Area・整数 Rotation 属性を持つこと
    let chiban_feat = geo
        .features
        .iter()
        .find(|f| f.kind == "sima:Chiban")
        .expect("Chiban feature");
    assert_eq!(chiban_feat.geometries.len(), 1);
    if let Form::Area { outer, holes } = &chiban_feat.geometries[0].form {
        assert_eq!(outer.len(), 4);
        assert_eq!(holes.len(), 1);
        assert_eq!(holes[0].len(), 3);
    } else {
        panic!("expected Form::Area");
    }
    assert!(chiban_feat.attributes.iter().any(|a| {
        a.name == "Area" && matches!(a.value, Value::Real(v) if (v - 600.0).abs() < 1e-9)
    }));
    assert!(
        chiban_feat
            .attributes
            .iter()
            .any(|a| { a.name == "Rotation" && a.value == Value::Integer(1) })
    );

    // 逆向き（orientation="-"）の 3 点円弧 KaihouKakuchi が CivilModel の円弧 Alignment になり、GeoModel では曲線離散化されていること
    assert_eq!(civil.alignments.len(), 1);
    assert_eq!(civil.alignments[0].name.as_deref(), Some("水路中心"));
    assert!(matches!(
        civil.alignments[0].horizontal[0].kind,
        HorizontalKind::Arc { .. }
    ));
    let kh_feat = geo
        .features
        .iter()
        .find(|f| f.kind == "sima:KaihouKakuchi")
        .expect("KaihouKakuchi feature");
    if let Form::Line(pts) = &kh_feat.geometries[0].form {
        assert!(
            pts.len() > 3,
            "expected sampled arc points, got {}",
            pts.len()
        );
    } else {
        panic!("expected Form::Line");
    }

    // Kakuchi と Chiban の両方について外周 + 穴（計 4 個）の閉じた折れ線が Drawing2D に出ていること
    let closed_polys = drawing.spaces[0]
        .elements
        .iter()
        .filter(|el| matches!(&el.shape, Shape::Polyline(p) if p.closed))
        .count();
    assert_eq!(closed_polys, 4);
}

#[test]
fn lowers_and_roundtrips_drawing_civil_and_geo_models() {
    let src_xml = r##"<?xml version="1.0" encoding="UTF-8"?>
<GI xmlns="http://www.jsima.or.jp/JSIMASchema/201004"
    xmlns:jps="http://www.gsi.go.jp/GIS/jpgis/standardSchemas2.1_2009-05"
    version="1.0" timeStamp="2026-11-13T12:00:00">
  <dataset>
    <GenbaJoho>
      <Name>往復テスト</Name>
      <CoordinateSystem>9</CoordinateSystem>
      <Crs>4</Crs>
    </GenbaJoho>
    <object>
      <jps:GM_Point id="p1"><jps:position><jps:coordinate>50.0 120.0 3.25</jps:coordinate></jps:position></jps:GM_Point>
      <jps:GM_Point id="p2"><jps:position><jps:coordinate>80.0 120.0</jps:coordinate></jps:position></jps:GM_Point>
      <jps:GM_Point id="p3"><jps:position><jps:coordinate>80.0 160.0</jps:coordinate></jps:position></jps:GM_Point>
      <jps:GM_Curve id="c1">
        <jps:segment>
          <jps:GM_LineString>
            <jps:controlPoint>
              <jps:column><jps:indirect><jps:point idref="p1"/></jps:indirect></jps:column>
              <jps:column><jps:indirect><jps:point idref="p2"/></jps:indirect></jps:column>
              <jps:column><jps:indirect><jps:point idref="p3"/></jps:indirect></jps:column>
              <jps:column><jps:indirect><jps:point idref="p1"/></jps:indirect></jps:column>
            </jps:controlPoint>
          </jps:GM_LineString>
        </jps:segment>
      </jps:GM_Curve>
      <jps:GM_Surface id="s1">
        <jps:patch><jps:GM_Polygon><jps:boundary>
          <jps:exterior><jps:generator idref="c1"/></jps:exterior>
        </jps:boundary></jps:GM_Polygon></jps:patch>
      </jps:GM_Surface>
      <Sokuten id="sk1"><Number>1</Number><Name>P1</Name><Altitude>3.25</Altitude><RefPoint idref="p1"/></Sokuten>
      <Kakuchi id="k1"><Number>1</Number><Name>区画A</Name><RefSurface idref="s1"/></Kakuchi>
    </object>
  </dataset>
</GI>"##
        .as_bytes();

    let (res, diags) = lift_all(src_xml);
    assert!(diags.is_empty(), "{diags:?}");
    let (drawing, _, geo) = res.unwrap();

    // 1. GeoModel -> SIMA (.sim) -> GeoModel 往復（座標系 EPSG:6677 も G00 の第 4 列で保たれる）
    let mut budget = Budget::default();
    let mut diags2 = Diagnostics::new();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diags2, &cancel);
    let lowered_geo = lower_geo_to_sima(&geo, &mut cx).unwrap();
    assert!(lowered_geo.bytes.starts_with(b"G00,01,"));
    assert_eq!(lowered_geo.parcels, 1);

    let (re_res, re_diags) = lift_all(&lowered_geo.bytes);
    assert!(re_diags.is_empty(), "{re_diags:?}");
    let (_, _, re_geo) = re_res.unwrap();
    assert_eq!(re_geo.crs, Some(Crs::Plane(Zone::new(9).unwrap())));
    let area_feat = re_geo
        .features
        .iter()
        .find(|f| {
            f.geometries
                .iter()
                .any(|g| matches!(g.form, Form::Area { .. }))
        })
        .expect("roundtripped area");
    if let Form::Area { outer, .. } = &area_feat.geometries[0].form {
        assert_eq!(outer.len(), 3);
        let Position::Map(mp) = outer[0].position else {
            panic!("expected Map position");
        };
        assert!((mp.northing - 50.0).abs() < 1e-6);
        assert!((mp.easting - 120.0).abs() < 1e-6);
    }

    // 2. Drawing2D -> SIMA (.sim) -> Drawing2D 往復（東・北と北・東の相互変換が狂わないこと）
    let mut budget = Budget::default();
    let mut diags3 = Diagnostics::new();
    let mut cx = Cx::new(&mut budget, &mut diags3, &cancel);
    let lowered_dwg = lower_drawing_to_sima(&drawing, &mut cx).unwrap();
    let (re_dwg_res, _) = lift_all(&lowered_dwg.bytes);
    let (re_dwg, _, _) = re_dwg_res.unwrap();
    let pt_back = re_dwg.spaces[0]
        .elements
        .iter()
        .find_map(|el| match &el.shape {
            Shape::Point(p) => Some(p.position),
            _ => None,
        })
        .unwrap();
    assert!((pt_back.x - 120.0).abs() < 1e-6);
    assert!((pt_back.y - 50.0).abs() < 1e-6);

    // 3. CivilModel -> SIMA (.sim) -> CivilModel 往復
    let csv_route = concat!(
        "G00,01,路線テスト,EPSG:6677,\r\n",
        "A00,\r\n",
        "A01,1,BP,0.0,0.0,0.0,\r\n",
        "A01,2,KA,100.0,0.0,0.0,\r\n",
        "A01,3,KE,150.0,50.0,0.0,\r\n",
        "A99,\r\n",
        "F00,1,R1,0.0,\r\n",
        "C00,\r\n",
        "B01,1,BP,\r\n",
        "B01,2,KA,\r\n",
        "C99,\r\n",
        "C00,\r\n",
        "C01,50.0,\r\n",
        "B01,2,KA,\r\n",
        "B01,3,KE,\r\n",
        "C99,\r\n",
        "F99,\r\n",
    );
    let (r_res, _) = lift_all(csv_route.as_bytes());
    let (_, civil, _) = r_res.unwrap();
    let mut budget = Budget::default();
    let mut diags4 = Diagnostics::new();
    let mut cx = Cx::new(&mut budget, &mut diags4, &cancel);
    let lowered_civ = lower_civil_to_sima(&civil, &mut cx).unwrap();
    assert_eq!(lowered_civ.routes, 1);
    let (re_civ_res, _) = lift_all(&lowered_civ.bytes);
    let (_, re_civil, _) = re_civ_res.unwrap();
    assert_eq!(re_civil.alignments.len(), 1);
    assert_eq!(re_civil.alignments[0].horizontal.len(), 2);
    assert!(matches!(
        re_civil.alignments[0].horizontal[1].kind,
        HorizontalKind::Arc { radius } if (radius - 50.0).abs() < 1e-4
    ));

    // 4. WGS 84 経緯度の GeoModel は平面直角座標に換算できないため sima.write-crs.unsupported で失敗すること
    let wgs_geo = GeoModel {
        crs: Some(Crs::Wgs84),
        height: None,
        features: vec![Feature {
            id: "f1".to_string(),
            kind: "Point".to_string(),
            code: None,
            name: None,
            attributes: Vec::new(),
            geometries: vec![Geometry {
                lod: None,
                form: Form::Point(Location {
                    position: Position::Geo(GeoPoint {
                        latitude: 35.68,
                        longitude: 139.76,
                    }),
                    height: None,
                }),
            }],
            parent: None,
            source: Ref::default(),
        }],
    };
    let mut budget = Budget::default();
    let mut diags5 = Diagnostics::new();
    let mut cx = Cx::new(&mut budget, &mut diags5, &cancel);
    assert!(lower_geo_to_sima(&wgs_geo, &mut cx).is_err());
    assert!(
        diags5
            .iter()
            .any(|d| d.code() == codes::SIMA_WRITE_CRS_UNSUPPORTED)
    );
}

#[test]
fn reports_diagnostics_for_invalid_files_missing_points_and_cycles() {
    // 先頭が G00 でない CSV は sima.file.invalid
    let (_, bad_diags) = read_sima(b"A01,1,P1,10.0,20.0,0.0,\r\n");
    assert!(
        bad_diags
            .iter()
            .any(|d| d.code() == codes::SIMA_FILE_INVALID)
    );

    // 未知レコード・壊れた数値・存在しない点参照・退化区画
    let dirty_csv = concat!(
        "G00,01,診断テスト,UNKNOWN_CRS,\r\n",
        "X99,謎レコード,\r\n",
        "A00,\r\n",
        "A01,1,P1,NaN,20.0,0.0,\r\n",
        "A01,2,P2,10.0,20.0,0.0,\r\n",
        "A01,3,P3,30.0,40.0,0.0,\r\n",
        "A99,\r\n",
        "D00,1,退化区画,1,\r\n",
        "B01,2,P2,\r\n",
        "B01,3,P3,\r\n",
        "B01,999,NOPE,\r\n",
        "D99,\r\n",
    );
    let (res, diags) = lift_all(dirty_csv.as_bytes());
    assert!(res.is_ok());
    assert!(
        diags
            .iter()
            .any(|d| d.code() == codes::SIMA_CRS_UNSUPPORTED)
    );
    assert!(
        diags
            .iter()
            .any(|d| d.code() == codes::SIMA_RECORD_UNSUPPORTED)
    );
    assert!(diags.iter().any(|d| d.code() == codes::SIMA_VALUE_INVALID));
    assert!(diags.iter().any(|d| d.code() == codes::SIMA_POINT_MISSING));
    assert!(
        diags
            .iter()
            .any(|d| d.code() == codes::SIMA_GEOMETRY_DROPPED)
    );

    // XML の自己参照サイクル（oc1 -> oc2 -> oc1）で無限再帰せず sima.value.invalid を報告して止まること
    let cycle_xml = r##"<?xml version="1.0" encoding="UTF-8"?>
<GI xmlns="http://www.jsima.or.jp/JSIMASchema/201206"
    xmlns:jps="http://www.gsi.go.jp/GIS/jpgis/standardSchemas2.1_2009-05"
    version="1.0" timeStamp="2026-11-13T12:00:00">
  <dataset>
    <GenbaJoho><Name>循環</Name><CoordinateSystem>9</CoordinateSystem></GenbaJoho>
    <object>
      <jps:GM_OrientableCurve id="oc1"><jps:orientation>+</jps:orientation><jps:primitive idref="oc2"/></jps:GM_OrientableCurve>
      <jps:GM_OrientableCurve id="oc2"><jps:orientation>+</jps:orientation><jps:primitive idref="oc1"/></jps:GM_OrientableCurve>
      <KaihouKakuchi id="kh1"><Number>1</Number><RefCurve idref="oc1"/></KaihouKakuchi>
    </object>
  </dataset>
</GI>"##
        .as_bytes();
    let (cyc_res, cyc_diags) = lift_all(cycle_xml);
    assert!(cyc_res.is_ok());
    assert!(
        cyc_diags
            .iter()
            .any(|d| d.code() == codes::SIMA_VALUE_INVALID)
    );
}

#[test]
fn satisfies_contracts_budgets_and_cancellation_for_csv_and_xml() {
    let csv_sample = concat!(
        "G00,01,予算テスト,EPSG:6677,\r\n",
        "A00,\r\n",
        "A01,1,P1,10.0,20.0,1.0,\r\n",
        "A01,2,P2,20.0,20.0,1.0,\r\n",
        "A01,3,P3,20.0,30.0,1.0,\r\n",
        "A01,4,P4,10.0,30.0,1.0,\r\n",
        "A99,\r\n",
        "D00,1,K1,1,\r\n",
        "B01,1,P1,\r\n",
        "B01,2,P2,\r\n",
        "B01,3,P3,\r\n",
        "B01,4,P4,\r\n",
        "D99,\r\n",
    )
    .as_bytes();

    let xml_sample = r##"<?xml version="1.0" encoding="UTF-8"?>
<GI xmlns="http://www.jsima.or.jp/JSIMASchema/201206"
    xmlns:jps="http://www.gsi.go.jp/GIS/jpgis/standardSchemas2.1_2009-05"
    version="1.0" timeStamp="2026-11-13T12:00:00">
  <exchangeMetadata><datasetCitation><title>T</title></datasetCitation></exchangeMetadata>
  <dataset>
    <GenbaJoho><Name>XML契約</Name><CoordinateSystem>9</CoordinateSystem><Crs>3</Crs></GenbaJoho>
    <object>
      <jps:GM_Point id="p1"><jps:position><jps:coordinate>10.0 20.0</jps:coordinate></jps:position></jps:GM_Point>
      <jps:GM_Point id="p2"><jps:position><jps:coordinate>20.0 20.0</jps:coordinate></jps:position></jps:GM_Point>
      <jps:GM_Point id="p3"><jps:position><jps:coordinate>20.0 30.0</jps:coordinate></jps:position></jps:GM_Point>
      <Sokuten id="sk1"><Number>1</Number><Name>A</Name><RefPoint idref="p1"/></Sokuten>
      <Sokuten id="sk2"><Number>2</Number><Name>B</Name><RefPoint idref="p2"/></Sokuten>
      <Sokuten id="sk3"><Number>3</Number><Name>C</Name><RefPoint idref="p3"/></Sokuten>
    </object>
  </dataset>
</GI>"##
        .as_bytes();

    for sample in [csv_sample, xml_sample] {
        let (file, _) = read_sima(sample);
        assert!(file.unwrap().record_count() >= 3);
        testkit::roundtrip::<Sima>(sample).unwrap();
        testkit::records_in_order::<Sima>(sample).unwrap();
        testkit::deterministic::<Sima>(sample).unwrap();
        testkit::stops_at_same_place::<Sima>(sample).unwrap();
        testkit::survives_lift::<Sima, Drawing2D>(sample).unwrap();
        testkit::survives_lift::<Sima, CivilModel>(sample).unwrap();
        testkit::survives_lift::<Sima, GeoModel>(sample).unwrap();
    }

    testkit::ids_match(
        xml_sample,
        "Sokuten/sk1\nSokuten/sk2\nSokuten/sk3\n",
        |src| {
            let (res, _) = lift_all(src);
            res.map_err(|e| format!("{e:?}"))
                .map(|(_, _, g)| g.features.into_iter().map(|f| f.id).collect())
        },
    )
    .unwrap();

    // 6 種すべての予算上限と CancelToken で、停止時に必ず診断が報告されること
    for (kind, lim) in [
        (Kind::InputBytes, 8),
        (Kind::DecodedBytes, 24),
        (Kind::Records, 2),
        (Kind::Elements, 2),
        (Kind::Depth, 0),
        (Kind::Work, 1),
    ] {
        let limits = Limits::default().with(kind, Limit::Finite(lim));
        let mut budget = Budget::new(limits);
        let mut diags = Diagnostics::new();
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diags, &cancel);
        let res = Sima::read(csv_sample, &mut cx).and_then(|f| {
            let _: Drawing2D = f.lift(&mut cx)?;
            Ok(())
        });
        assert!(
            matches!(res, Err(Error::Stopped(_))),
            "expected Stopped for {kind:?}, got {res:?}"
        );
        assert!(
            !diags.is_empty(),
            "expected budget diagnostic for {kind:?}, got empty diagnostics"
        );
    }

    let mut budget = Budget::default();
    let mut diags = Diagnostics::new();
    let cancel = CancelToken::new();
    cancel.cancel();
    let mut cx = Cx::new(&mut budget, &mut diags, &cancel);
    assert!(matches!(
        Sima::read(csv_sample, &mut cx),
        Err(Error::Stopped(_))
    ));
    assert!(!diags.is_empty());

    // 書き出し側の予算超過でも診断が必ず出ること
    let (ok_res, _) = lift_all(csv_sample);
    let (drawing, _, _) = ok_res.unwrap();
    let limits = Limits::default().with(Kind::DecodedBytes, Limit::Finite(16));
    let mut budget = Budget::new(limits);
    let mut diags = Diagnostics::new();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diags, &cancel);
    assert!(matches!(
        lower_drawing_to_sima(&drawing, &mut cx),
        Err(Error::Stopped(_))
    ));
    assert!(!diags.is_empty());
}

#[test]
fn preserves_clothoids_major_arcs_circles_axes_and_rejects_nonplanar_crs() {
    let cancel = CancelToken::new();

    // 1. クロソイド（加速 0->100, 卵形 100->50, 減速 50->0）の CivilModel -> .sim -> CivilModel & Drawing2D 精度検証
    let p0 = MapPoint {
        northing: 1000.0,
        easting: 2000.0,
    };
    let dir0 = 0.35;
    let l1 = 30.0;
    let ([n1, e1], dir1) = zumen_geom2d::along(
        [p0.northing, p0.easting],
        dir0,
        [0.0, 0.01],
        l1,
        Some(Ramp::Clothoid),
        l1,
    );
    let p1 = MapPoint {
        northing: n1,
        easting: e1,
    };
    let l2 = 25.0;
    let ([n2, e2], dir2) =
        zumen_geom2d::along([n1, e1], dir1, [0.01, 0.02], l2, Some(Ramp::Clothoid), l2);
    let p2 = MapPoint {
        northing: n2,
        easting: e2,
    };
    let l3 = 20.0;
    let ([n3, e3], _) =
        zumen_geom2d::along([n2, e2], dir2, [0.02, 0.0], l3, Some(Ramp::Clothoid), l3);

    let civil_in = CivilModel {
        alignments: vec![Alignment {
            id: "al1".to_string(),
            name: Some("緩和路線".to_string()),
            usage: None,
            horizontal: vec![
                CivilHorizontal {
                    start: p0,
                    direction: dir0,
                    length: l1,
                    kind: HorizontalKind::Transition {
                        start_radius: None,
                        end_radius: Some(100.0),
                        curve: TransitionCurve::Clothoid,
                    },
                    source: Ref::default(),
                },
                CivilHorizontal {
                    start: p1,
                    direction: dir1,
                    length: l2,
                    kind: HorizontalKind::Transition {
                        start_radius: Some(100.0),
                        end_radius: Some(50.0),
                        curve: TransitionCurve::Clothoid,
                    },
                    source: Ref::default(),
                },
                CivilHorizontal {
                    start: p2,
                    direction: dir2,
                    length: l3,
                    kind: HorizontalKind::Transition {
                        start_radius: Some(50.0),
                        end_radius: None,
                        curve: TransitionCurve::Clothoid,
                    },
                    source: Ref::default(),
                },
            ],
            vertical: Vec::new(),
            cant: Vec::new(),
            stations: Stationing {
                start: 120.5,
                breaks: Vec::new(),
                pitch: None,
            },
            cross_sections: Vec::new(),
            source: Ref::default(),
        }],
        surfaces: Vec::new(),
        crs: Some(Crs::Plane(Zone::new(9).unwrap())),
        height_datum: None,
        realization: Some(Realization::Survey2011),
    };

    let mut budget = Budget::default();
    let mut diags = Diagnostics::new();
    let mut cx = Cx::new(&mut budget, &mut diags, &cancel);
    let sim_out = lower_civil_to_sima(&civil_in, &mut cx).unwrap();
    assert!(diags.is_empty(), "{diags:?}");

    let (re_res, _) = lift_all(&sim_out.bytes);
    let (re_dwg, re_civil, _) = re_res.unwrap();
    let re_h = &re_civil.alignments[0].horizontal;
    assert_eq!(re_h.len(), 3);
    assert!((re_civil.alignments[0].stations.start - 120.5).abs() < 1e-9);
    assert!((re_h[0].direction - dir0).abs() < 1e-6);
    assert!((re_h[1].direction - dir1).abs() < 1e-6);
    assert!((re_h[2].direction - dir2).abs() < 1e-6);

    // 末尾区間（減速クロソイド）の終点も along で一致すること
    let ([re_n3, re_e3], _) = zumen_geom2d::along(
        [re_h[2].start.northing, re_h[2].start.easting],
        re_h[2].direction,
        [0.02, 0.0],
        re_h[2].length,
        Some(Ramp::Clothoid),
        re_h[2].length,
    );
    assert!((re_n3 - n3).abs() < 1e-6);
    assert!((re_e3 - e3).abs() < 1e-6);

    // Drawing2D 経由の往復でも Edge::Clothoid が保たれること
    let mut budget = Budget::default();
    let mut diags = Diagnostics::new();
    let mut cx = Cx::new(&mut budget, &mut diags, &cancel);
    let dwg_sim = lower_drawing_to_sima(&re_dwg, &mut cx).unwrap();
    assert_eq!(dwg_sim.routes, 1);

    // 2. 270° 優弧（orientation="-"）と GM_Circle（全円）の保持・分割往復
    let arc_xml = r##"<?xml version="1.0" encoding="UTF-8"?>
<GI xmlns="http://www.jsima.or.jp/JSIMASchema/201206"
    xmlns:jps="http://www.gsi.go.jp/GIS/jpgis/standardSchemas2.1_2009-05"
    version="1.0" timeStamp="2026-11-13T12:00:00">
  <dataset>
    <GenbaJoho><Name>優弧円</Name><CoordinateSystem>9</CoordinateSystem></GenbaJoho>
    <object>
      <jps:GM_Curve id="c_major">
        <jps:segment>
          <jps:GM_Arc>
            <jps:controlPoint>
              <jps:column><jps:direct><jps:coordinate>0.0 10.0</jps:coordinate></jps:direct></jps:column>
              <jps:column><jps:direct><jps:coordinate>0.0 -10.0</jps:coordinate></jps:direct></jps:column>
              <jps:column><jps:direct><jps:coordinate>-10.0 0.0</jps:coordinate></jps:direct></jps:column>
            </jps:controlPoint>
          </jps:GM_Arc>
        </jps:segment>
      </jps:GM_Curve>
      <jps:GM_OrientableCurve id="oc_major">
        <jps:orientation>-</jps:orientation>
        <jps:primitive idref="c_major"/>
      </jps:GM_OrientableCurve>
      <jps:GM_Curve id="c_circle">
        <jps:segment>
          <jps:GM_Circle>
            <jps:controlPoint>
              <jps:column><jps:direct><jps:coordinate>0.0 20.0</jps:coordinate></jps:direct></jps:column>
              <jps:column><jps:direct><jps:coordinate>20.0 0.0</jps:coordinate></jps:direct></jps:column>
              <jps:column><jps:direct><jps:coordinate>0.0 -20.0</jps:coordinate></jps:direct></jps:column>
            </jps:controlPoint>
          </jps:GM_Circle>
        </jps:segment>
      </jps:GM_Curve>
      <KaihouKakuchi id="kh_major"><Number>1</Number><Name>優弧</Name><RefCurve idref="oc_major"/></KaihouKakuchi>
      <KaihouKakuchi id="kh_circle"><Number>2</Number><Name>全円</Name><RefCurve idref="c_circle"/></KaihouKakuchi>
    </object>
  </dataset>
</GI>"##
        .as_bytes();
    let (file_res, _) = read_sima(arc_xml);
    let sima_file = file_res.unwrap();
    let (arc_res, _) = lift_all(arc_xml);
    let (_, arc_civil, _) = arc_res.unwrap();
    assert_eq!(arc_civil.alignments.len(), 2);
    let major_len: f64 = arc_civil.alignments[0]
        .horizontal
        .iter()
        .map(|h| h.length)
        .sum();
    assert!((major_len - 1.5 * PI * 10.0).abs() < 1e-6);
    let circle_len: f64 = arc_civil.alignments[1]
        .horizontal
        .iter()
        .map(|h| h.length)
        .sum();
    assert!((circle_len - 2.0 * PI * 20.0).abs() < 1e-6);

    // lower_sima_to_sima で .sim に書き出して再読込しても 270° 優弧と全円の弧長が保たれること
    let mut budget = Budget::default();
    let mut diags = Diagnostics::new();
    let mut cx = Cx::new(&mut budget, &mut diags, &cancel);
    let self_sim = lower_sima_to_sima(&sima_file, &mut cx).unwrap();
    let (re_arc_res, _) = lift_all(&self_sim.bytes);
    let (_, re_arc_civil, _) = re_arc_res.unwrap();
    let re_major_len: f64 = re_arc_civil.alignments[0]
        .horizontal
        .iter()
        .map(|h| h.length)
        .sum();
    let re_circle_len: f64 = re_arc_civil.alignments[1]
        .horizontal
        .iter()
        .map(|h| h.length)
        .sum();
    assert!((re_major_len - 1.5 * PI * 10.0).abs() < 1e-5);
    assert!((re_circle_len - 2.0 * PI * 20.0).abs() < 1e-5);

    // 3. Axes::NorthEast の Drawing2D 円弧を .sim に書き出したとき右カーブ符号が合うこと
    let ne_space = SpaceId(0);
    let ne_dwg = Drawing2D {
        spaces: vec![ModelSpace {
            id: ne_space,
            unit: Unit::Meter,
            axes: Axes::NorthEast,
            elements: vec![Element {
                source: Ref::default(),
                layer: LayerId(0),
                style: Style {
                    color: Inherit::Layer,
                    line_type: Inherit::Layer,
                    line_type_scale: 1.0,
                    line_weight: None,
                    transparency: Inherit::Layer,
                },
                shape: Shape::Curve(Edge::Arc {
                    center: ModelPoint {
                        space: ne_space,
                        x: 0.0,
                        y: 0.0,
                    },
                    radius: Length::Model(ModelLength {
                        space: ne_space,
                        value: 10.0,
                    }),
                    start: 0.0,
                    end: 90.0,
                    counterclockwise: true,
                }),
            }],
            order: vec![0],
        }],
        ..Drawing2D::default()
    };
    let mut budget = Budget::default();
    let mut diags = Diagnostics::new();
    let mut cx = Cx::new(&mut budget, &mut diags, &cancel);
    let ne_sim = lower_drawing_to_sima(&ne_dwg, &mut cx).unwrap();
    let (ne_re, _) = lift_all(&ne_sim.bytes);
    let (_, ne_civil, _) = ne_re.unwrap();
    assert!(matches!(
        ne_civil.alignments[0].horizontal[0].kind,
        HorizontalKind::Arc { radius } if (radius - 10.0).abs() < 1e-6
    ));

    // 4. A01 重複更新・C03 未対応診断・非平面直角 CRS 拒否
    let dup_csv = concat!(
        "G00,01,重複テスト,EPSG:4326,\r\n",
        "A00,\r\n",
        "A01,1,P1,10.0,20.0,1.0,\r\n",
        "A01,1,P1,30.0,40.0,2.0,\r\n",
        "A01,2,P2,50.0,60.0,3.0,\r\n",
        "A99,\r\n",
        "F00,1,R1,0.0,\r\n",
        "C00,\r\n",
        "C03,100.0,\r\n",
        "B01,1,P1,\r\n",
        "B01,2,P2,\r\n",
        "C99,\r\n",
        "F99,\r\n",
    );
    let (dup_res, dup_diags) = lift_all(dup_csv.as_bytes());
    let (_, _, dup_geo) = dup_res.unwrap();
    assert!(
        dup_diags
            .iter()
            .any(|d| d.code() == codes::SIMA_CRS_UNSUPPORTED)
    );
    assert!(
        dup_diags
            .iter()
            .any(|d| d.code() == codes::SIMA_RECORD_UNSUPPORTED)
    );
    let sokuten_count = dup_geo
        .features
        .iter()
        .filter(|f| f.kind == "sima:Sokuten")
        .count();
    assert_eq!(sokuten_count, 2);

    // 幾何要素が空でも WGS 84 の GeoModel / CivilModel は sima.write-crs.unsupported で拒否されること
    let empty_wgs = GeoModel {
        crs: Some(Crs::Wgs84),
        height: None,
        features: Vec::new(),
    };
    let mut budget = Budget::default();
    let mut diags = Diagnostics::new();
    let mut cx = Cx::new(&mut budget, &mut diags, &cancel);
    assert!(lower_geo_to_sima(&empty_wgs, &mut cx).is_err());
    assert!(
        diags
            .iter()
            .any(|d| d.code() == codes::SIMA_WRITE_CRS_UNSUPPORTED)
    );
}

#[test]
fn guards_extreme_floats_clothoid_precision_and_discontinuous_routes() {
    let cancel = CancelToken::new();

    // 1. 極端な有限浮動小数点（A^2 のオーバーフロー・アンダーフロー、2*|R| のオーバーフロー）
    let extreme_csv = concat!(
        "G00,01,極端値テスト,EPSG:6677,\r\n",
        "A00,\r\n",
        "A01,1,P1,0.0,0.0,0.0,\r\n",
        "A01,2,P2,10.0,0.0,0.0,\r\n",
        "A99,\r\n",
        "F00,1,R1,0.0,\r\n",
        "C00,\r\n",
        "B01,1,P1,\r\n",
        "C02,1e200,,1e-200,\r\n",
        "B01,2,P2,\r\n",
        "C99,\r\n",
        "C00,\r\n",
        "B01,1,P1,\r\n",
        "C02,1e-200,10.0,\r\n",
        "B01,2,P2,\r\n",
        "C99,\r\n",
        "C00,\r\n",
        "B01,1,P1,\r\n",
        "C01,1e308,\r\n",
        "B01,2,P2,\r\n",
        "C99,\r\n",
        "F99,\r\n",
    );
    let (res, diags) = lift_all(extreme_csv.as_bytes());
    let (_dwg, civil, _geo) = res.unwrap();
    assert!(civil.alignments.is_empty());
    assert!(
        diags
            .iter()
            .filter(|d| d.code() == codes::SIMA_VALUE_INVALID)
            .map(zumen_base::Diagnostic::count)
            .sum::<u64>()
            >= 3
    );

    // 2. lower_drawing_to_sima の Edge::Clothoid 端点精度（A=60, s=-120..0, Δφ=2.0 rad）
    let clothoid_dwg = Drawing2D {
        spaces: vec![ModelSpace {
            id: SpaceId(0),
            unit: Unit::Meter,
            axes: Axes::EastNorth,
            elements: vec![Element {
                source: Ref::default(),
                layer: LayerId(0),
                style: Style {
                    color: Inherit::Layer,
                    line_type: Inherit::Layer,
                    line_type_scale: 1.0,
                    line_weight: None,
                    transparency: Inherit::Layer,
                },
                shape: Shape::Curve(Edge::Clothoid {
                    origin: ModelPoint {
                        space: SpaceId(0),
                        x: 0.0,
                        y: 0.0,
                    },
                    parameter: Length::Model(ModelLength {
                        space: SpaceId(0),
                        value: 60.0,
                    }),
                    angle: 0.0,
                    counterclockwise: true,
                    start: Length::Model(ModelLength {
                        space: SpaceId(0),
                        value: -120.0,
                    }),
                    end: Length::Model(ModelLength {
                        space: SpaceId(0),
                        value: 0.0,
                    }),
                }),
            }],
            order: vec![0],
        }],
        ..Drawing2D::default()
    };
    let mut budget = Budget::default();
    let mut c_diags = Diagnostics::new();
    let mut cx = Cx::new(&mut budget, &mut c_diags, &cancel);
    let lowered_clothoid = lower_drawing_to_sima(&clothoid_dwg, &mut cx).unwrap();
    let text = String::from_utf8(lowered_clothoid.bytes.clone()).unwrap();
    // 原点 (0, 0) が終点、始点の東座標 x ≈ -80.111622 m（真値との差 1 mm 未満）
    assert!(text.contains("-80.1116"));

    // 3. lower_civil_to_sima で不連続な Horizontal 区間が隣接していても次の始点へワープしないこと
    let gap_src = concat!(
        "G00,01,ギャップ路線,EPSG:6677,\r\n",
        "A00,\r\n",
        "A01,1,P1,0.0,0.0,0.0,\r\n",
        "A01,2,P2,10.0,10.0,0.0,\r\n",
        "A01,3,P3,100.0,100.0,0.0,\r\n",
        "A01,4,P4,110.0,100.0,0.0,\r\n",
        "A99,\r\n",
        "F00,1,R1,0.0,\r\n",
        "C00,\r\n",
        "B01,1,P1,\r\n",
        "C01,10.0,\r\n",
        "B01,2,P2,\r\n",
        "C99,\r\n",
        "C00,\r\n",
        "B01,3,P3,\r\n",
        "B01,4,P4,\r\n",
        "C99,\r\n",
        "F99,\r\n",
    );
    let (gap_res, _) = lift_all(gap_src.as_bytes());
    let (_, gap_civil, _) = gap_res.unwrap();
    assert_eq!(gap_civil.alignments[0].horizontal.len(), 2);
    let mut budget = Budget::default();
    let mut g_diags = Diagnostics::new();
    let mut cx = Cx::new(&mut budget, &mut g_diags, &cancel);
    let re_sim = lower_civil_to_sima(&gap_civil, &mut cx).unwrap();
    let (re_res, re_diags) = lift_all(&re_sim.bytes);
    let (_, re_civil, _) = re_res.unwrap();
    assert!(
        !re_diags
            .iter()
            .any(|d| d.code() == codes::SIMA_VALUE_INVALID)
    );
    assert_eq!(re_civil.alignments[0].horizontal.len(), 2);
}
