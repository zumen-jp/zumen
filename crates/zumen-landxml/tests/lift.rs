//! 試験の中で書いた小さな J-LandXML を読み、線形・縦断・横断・地表面を土木の領域モデルに移す（#368）。

use zumen_base::{Budget, CancelToken, Diagnostics, codes};
use zumen_civil::{CivilModel, HorizontalKind, SurfaceRole, TransitionCurve, VerticalKind};
use zumen_crs::{Crs, HeightDatum, Realization, Zone};
use zumen_landxml::LandXml;
use zumen_sdk::{Codec, Cx, Lift, Signature};

/// 北へ 100 m の直線、半径 200 m へ右に曲がるクロソイド（A = 100、50 m）、半径 200 m の円弧 40 m、直線 60 m。
/// 点は、クロソイドの式と円弧の中心から求めた値
const ALIGNMENT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<LandXML xmlns="http://www.landxml.org/schema/LandXML-1.2" version="1.2" date="2026-11-01" time="00:00:00">
  <Project name="試験"><Feature><Property label="applicationCriterion" value="MlitLandXmlVer.1.8"/></Feature></Project>
  <Units><Metric areaUnit="squareMeter" linearUnit="meter" volumeUnit="cubicMeter" temperatureUnit="celsius" pressureUnit="HPA" angularUnit="decimal dd.mm.ss" directionUnit="decimal dd.mm.ss"/></Units>
  <CoordinateSystem horizontalDatum="JGD2011" verticalDatum="T.P" horizontalCoordinateSystemName="9(X,Y)"/>
  <CgPoints name="ControlPnts"><CgPoint name="K1">0 0 0</CgPoint></CgPoints>
  <Alignments name="道路">
    <Alignment name="本線" length="250" staStart="1000">
      <CoordGeom>
        <Line length="100"><Start name="BP">0 0</Start><End name="KA1-1">100 0</End></Line>
        <Spiral length="50" radiusStart="INF" radiusEnd="200" rot="cw" spiType="clothoid">
          <Start>100 0</Start><PI>133.36065595095215 0</PI><End>149.92193149366025 2.0810093401773635</End>
        </Spiral>
        <Curve rot="cw" radius="200" length="40">
          <Start>149.92193149366025 2.0810093401773635</Start><Center>124.98698481661472 200.52054278604317</Center><End>188.84874198801492 10.990397503080032</End>
        </Curve>
        <Line><Start>188.84874198801492 10.990397503080032</Start><End>245.70778557290387 30.14892465450009</End></Line>
      </CoordGeom>
      <StaEquation staInternal="1150" staBack="1150" staAhead="1140"/>
      <Profile name="縦断">
        <ProfAlign name="計画">
          <PVI>1000 50</PVI>
          <ParaCurve length="40">1100 52</ParaCurve>
          <PVI>1250 49</PVI>
        </ProfAlign>
        <ProfSurf name="地盤"><PntList2D>1000 49 1250 48</PntList2D></ProfSurf>
      </Profile>
      <CrossSects name="横断">
        <CrossSect sta="1100" name="No.5+0.0000">
          <DesignCrossSectSurf name="Carriageway" side="right" desc="道路面">
            <CrossSectPnt code="CL">0 52</CrossSectPnt>
            <CrossSectPnt code="R1" dataFormat="Offset Elevation">3.5 51.93</CrossSectPnt>
          </DesignCrossSectSurf>
          <DesignCrossSectSurf name="SubGrade" side="right" desc="路床面">
            <CrossSectPnt code="CL">0 -0.5</CrossSectPnt>
            <Feature><Property label="heightType" value="fhOffset"/></Feature>
          </DesignCrossSectSurf>
          <CrossSectSurf name="ExistingGround"><PntList2D>-10 50 0 50.5 10 51</PntList2D></CrossSectSurf>
        </CrossSect>
      </CrossSects>
      <Superelevation staStart="1110" staEnd="1140"><FullSuperelev>-6</FullSuperelev></Superelevation>
      <Feature name="Interval"><Property label="main" value="20"/></Feature>
    </Alignment>
  </Alignments>
  <Surfaces>
    <Surface name="計画面" desc="FinishedGrade">
      <Definition surfType="TIN">
        <Pnts><P id="1">0 0 10</P><P id="2">0 10 11</P><P id="3">10 10 12</P><P id="4">10 0 11</P></Pnts>
        <Faces><F>1 2 3</F><F>1 3 4</F><F i="1">2 3 4</F></Faces>
      </Definition>
      <Feature name="AreaPnts"><Property label="areaList" value="1 4 3 2"/></Feature>
    </Surface>
  </Surfaces>
</LandXML>
"#;

fn with_cx<T>(f: impl FnOnce(&mut Cx<'_>) -> T) -> (T, Diagnostics) {
    let mut diag = Diagnostics::new();
    let mut budget = Budget::default();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
    let out = f(&mut cx);
    (out, diag)
}

fn read(src: &str) -> Option<(CivilModel, Diagnostics)> {
    let (model, diag) = with_cx(|cx| {
        let file = LandXml::read(src.as_bytes(), cx).ok()?;
        file.lift(cx).ok()
    });
    Some((model?, diag))
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-7
}

#[test]
fn the_signature_is_the_landxml_root_element() {
    assert!(LandXml::SIGNATURES.iter().any(|signature| matches!(
        signature,
        Signature::XmlRoot {
            name: b"LandXML",
            ..
        }
    )));
}

#[test]
fn alignments_move_with_their_segments_stations_and_coordinate_system() {
    let (model, diag) = read(ALIGNMENT).unwrap();
    assert_eq!(model.crs, Some(Crs::Plane(Zone::new(9).unwrap())));
    assert_eq!(model.realization, Some(Realization::Survey2011));
    assert_eq!(model.height_datum, Some(HeightDatum::TokyoBay));
    let [alignment] = model.alignments.as_slice() else {
        panic!("{:?}", model.alignments);
    };
    assert_eq!(alignment.id, "本線");
    assert_eq!(alignment.stations.start, 1000.0);
    assert_eq!(alignment.stations.pitch, Some(20.0));
    let kinds: Vec<_> = alignment
        .horizontal
        .iter()
        .map(|segment| &segment.kind)
        .collect();
    assert!(matches!(
        kinds.as_slice(),
        [
            HorizontalKind::Line,
            HorizontalKind::Transition {
                start_radius: None,
                end_radius: Some(200.0),
                curve: TransitionCurve::Clothoid
            },
            HorizontalKind::Arc { radius: 200.0 },
            HorizontalKind::Line,
        ]
    ));
    assert!(near(alignment.horizontal[2].direction, 0.125));
    assert!(near(alignment.horizontal[3].direction, 0.325));
    assert!(near(alignment.horizontal[3].length, 60.0));
    // 区間の初めの点は、前の区間の終わりから計算した点と合う
    let ((), checked) = with_cx(|cx| zumen_civil::check(alignment, 1e-6, cx));
    assert!(checked.is_empty(), "{checked:?}");
    // 測点の不連続。追加距離 150 m で、測点の値を 1140 から数え直す
    let [at] = alignment.stations.breaks.as_slice() else {
        panic!()
    };
    assert_eq!((at.distance, at.back, at.ahead), (150.0, 1150.0, 1140.0));
    assert_eq!(alignment.stations.value(160.0), 1150.0);
    // 移さなかった要素を知らせる
    let unsupported: Vec<String> = diag
        .iter()
        .filter(|d| d.code() == codes::LANDXML_ELEMENT_UNSUPPORTED)
        .map(|d| format!("{d:?}"))
        .collect();
    for element in ["CgPoints", "ProfSurf", "Superelevation"] {
        assert!(
            unsupported.iter().any(|d| d.contains(element)),
            "{element} {unsupported:?}"
        );
    }
}

#[test]
fn the_profile_becomes_gradients_and_a_parabola() {
    let (model, _) = read(ALIGNMENT).unwrap();
    let alignment = &model.alignments[0];
    let segments: Vec<_> = alignment
        .vertical
        .iter()
        .map(|v| {
            (
                v.start_distance,
                v.start_height,
                v.start_gradient,
                v.length,
                v.kind.clone(),
            )
        })
        .collect();
    assert_eq!(segments.len(), 3, "{segments:?}");
    assert!(
        matches!(segments[1].4, VerticalKind::Parabola { end_gradient } if near(end_gradient, -0.02))
    );
    for (distance, height) in [(0.0, 50.0), (100.0, 51.8), (250.0, 49.0)] {
        let (placed, _) = with_cx(|cx| zumen_civil::place(alignment, distance, cx).unwrap());
        assert!(
            near(placed.height.unwrap(), height),
            "{distance} {placed:?}"
        );
    }
}

#[test]
fn cross_sections_and_surfaces_keep_their_points() {
    let (model, diag) = read(ALIGNMENT).unwrap();
    let [section] = model.alignments[0].cross_sections.as_slice() else {
        panic!()
    };
    assert_eq!(section.distance, 100.0);
    assert_eq!(section.name.as_deref(), Some("No.5+0.0000"));
    let lines: Vec<_> = section
        .lines
        .iter()
        .map(|l| (l.name.as_deref(), &l.role, l.points.len()))
        .collect();
    assert_eq!(
        lines,
        [
            (Some("Carriageway"), &SurfaceRole::Design, 2),
            (Some("ExistingGround"), &SurfaceRole::Existing, 3),
        ]
    );
    assert!(
        diag.iter()
            .any(|d| d.code() == codes::LANDXML_CROSS_SECTION_LINE_UNSUPPORTED)
    );
    let [tin] = model.surfaces.as_slice() else {
        panic!()
    };
    assert_eq!(
        (tin.id.as_str(), &tin.role),
        ("計画面", &SurfaceRole::Design)
    );
    assert_eq!(tin.points.len(), 4);
    assert_eq!(tin.points[1].0.easting, 10.0);
    assert_eq!(tin.triangles, [[0, 1, 2], [0, 2, 3]]);
    assert_eq!(tin.boundaries, [vec![0, 3, 2, 1]]);
    assert!(diag.iter().any(|d| d.code() == codes::LANDXML_FACE_DROPPED));
}

#[test]
fn missing_coordinate_systems_and_broken_values_are_reported() {
    let src = r#"<LandXML xmlns="http://www.landxml.org/schema/LandXML-1.2" version="1.2">
      <Alignments><Alignment name="a" length="10" staStart="0"><CoordGeom>
        <Line><Start>0 0</Start><End>x 10</End></Line>
        <Curve rot="up" radius="5"><Start>0 0</Start><Center>0 5</Center><End>5 5</End></Curve>
      </CoordGeom></Alignment><Alignment name="a" length="0" staStart="0"/></Alignments>
    </LandXML>"#;
    let (model, diag) = read(src).unwrap();
    assert_eq!(model.crs, None);
    assert!(
        diag.iter()
            .any(|d| d.code() == codes::LANDXML_CRS_UNSUPPORTED)
    );
    assert_eq!(
        diag.iter()
            .filter(|d| d.code() == codes::LANDXML_VALUE_INVALID)
            .count(),
        2
    );
    assert!(model.alignments[0].horizontal.is_empty());
    // 名前が重なる線形は、順の番号で ID を作る
    assert_eq!(model.alignments[1].id, "alignment/1");
}
