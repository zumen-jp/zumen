//! IFC4.3 の線形を土木の領域モデルに移す（決定 0235、#375）。ファイルは、どれもテストの中で作る。
#![cfg(feature = "civil")]

use zumen_base::{Budget, CancelToken, Code, Cx, Diagnostics, codes};
use zumen_civil::{CivilModel, CrossSectionHeight, HorizontalKind, LinearPosition, VerticalKind};
use zumen_crs::{Crs, HeightDatum, Zone};
use zumen_ifc::Ifc;
use zumen_landxml::LandXml;
use zumen_sdk::{Codec, Lift};

/// 北へ 100 m の直線、半径 200 m へ右に曲がるクロソイド（A = 100、50 m）、半径 200 m の円弧 40 m、直線 60 m。
/// 縦断は、勾配 2 % から -2 % へ、長さ 40 m の 2 次放物線でつなぐ。測点は 1000 から数え、追加距離 150 m で
/// 1150 から 1140 に数え直す
const LANDXML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<LandXML xmlns="http://www.landxml.org/schema/LandXML-1.2" version="1.2">
  <CoordinateSystem horizontalDatum="JGD2011" verticalDatum="T.P" horizontalCoordinateSystemName="9(X,Y)"/>
  <Alignments name="道路">
    <Alignment name="本線" length="250" staStart="1000">
      <CoordGeom>
        <Line length="100"><Start>0 0</Start><End>100 0</End></Line>
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
      </Profile>
    </Alignment>
  </Alignments>
</LandXML>
"#;

/// 同じ線形の IFC4.3。プロジェクトの x を東、y を北にする
fn ifc(units: &str, map: &str, scale: f64) -> String {
    let horizontal = [
        (
            0.0,
            0.0,
            core::f64::consts::FRAC_PI_2,
            0.0,
            0.0,
            100.0,
            "LINE",
        ),
        (
            0.0,
            100.0,
            core::f64::consts::FRAC_PI_2,
            0.0,
            -200.0,
            50.0,
            "CLOTHOID",
        ),
        (
            2.0810093401773635,
            149.92193149366025,
            1.4457963267948966,
            -200.0,
            -200.0,
            40.0,
            "CIRCULARARC",
        ),
        (
            10.990397503080032,
            188.84874198801492,
            1.2457963267948966,
            0.0,
            0.0,
            60.0,
            "LINE",
        ),
        (
            30.14892465450009,
            245.70778557290387,
            1.2457963267948966,
            0.0,
            0.0,
            0.0,
            "LINE",
        ),
    ];
    let vertical = [
        (0.0, 80.0, 50.0, 0.02, 0.02, "CONSTANTGRADIENT"),
        (80.0, 40.0, 51.6, 0.02, -0.02, "PARABOLICARC"),
        (120.0, 130.0, 51.6, -0.02, -0.02, "CONSTANTGRADIENT"),
        (250.0, 0.0, 49.0, -0.02, -0.02, "CONSTANTGRADIENT"),
    ];
    let angle = if units.contains("degree") {
        180.0 / core::f64::consts::PI
    } else {
        1.0
    };
    let mut data = Vec::new();
    let mut horizontal_segments = Vec::new();
    for (i, (x, y, direction, start, end, length, kind)) in horizontal.iter().enumerate() {
        let n = 200 + 3 * i;
        data.push(format!(
            "#{n}=IFCALIGNMENTSEGMENT('h{i:0>21}',$,$,$,$,$,$,#{});",
            n + 1
        ));
        data.push(format!(
            "#{}=IFCALIGNMENTHORIZONTALSEGMENT($,$,#{},{:?},{:?},{:?},{:?},$,.{kind}.);",
            n + 1,
            n + 2,
            direction * angle,
            start * scale,
            end * scale,
            length * scale
        ));
        data.push(format!(
            "#{}=IFCCARTESIANPOINT(({:?},{:?}));",
            n + 2,
            x * scale,
            y * scale
        ));
        horizontal_segments.push(format!("#{n}"));
    }
    let mut vertical_segments = Vec::new();
    for (i, (start, length, height, gradient, end, kind)) in vertical.iter().enumerate() {
        let n = 300 + 2 * i;
        data.push(format!(
            "#{n}=IFCALIGNMENTSEGMENT('v{i:0>21}',$,$,$,$,$,$,#{});",
            n + 1
        ));
        let radius = if *kind == "PARABOLICARC" {
            format!("{:?}", 1000.0 * scale)
        } else {
            "$".to_string()
        };
        data.push(format!(
            "#{}=IFCALIGNMENTVERTICALSEGMENT($,$,{:?},{:?},{:?},{gradient:?},{end:?},{radius},.{kind}.);",
            n + 1,
            start * scale,
            length * scale,
            height * scale
        ));
        vertical_segments.push(format!("#{n}"));
    }
    format!(
        "ISO-10303-21;
HEADER;
FILE_DESCRIPTION(('ViewDefinition [Alignment-basedView]'),'2;1');
FILE_NAME('a.ifc','2026-11-01T00:00:00',(''),(''),'','','');
FILE_SCHEMA(('IFC4X3_ADD2'));
ENDSEC;
DATA;
#1=IFCPROJECT('0000000000000000000001',$,'テスト',$,$,$,$,(#20),#10);
{units}
#20=IFCGEOMETRICREPRESENTATIONCONTEXT($,'Model',3,1.E-05,#21,$);
#21=IFCAXIS2PLACEMENT3D(#22,$,$);
#22=IFCCARTESIANPOINT((0.,0.,0.));
{map}
#40=IFCALIGNMENT('0000000000000000000040',$,'\\X2\\672C7DDA\\X0\\',$,$,#41,$,.USERDEFINED.);
#41=IFCLOCALPLACEMENT($,#21);
#50=IFCRELNESTS('0000000000000000000050',$,$,$,#40,(#60,#70,#80,#90));
#60=IFCALIGNMENTHORIZONTAL('0000000000000000000060',$,$,$,$,$,$);
#61=IFCRELNESTS('0000000000000000000061',$,$,$,#60,({}));
#70=IFCALIGNMENTVERTICAL('0000000000000000000070',$,$,$,$,$,$);
#71=IFCRELNESTS('0000000000000000000071',$,$,$,#70,({}));
#80=IFCREFERENT('0000000000000000000080',$,'No.0',$,$,#81,$,.STATION.);
#81=IFCLINEARPLACEMENT(#41,#82,$);
#82=IFCAXIS2PLACEMENTLINEAR(#83,$,$);
#83=IFCPOINTBYDISTANCEEXPRESSION(IFCNONNEGATIVELENGTHMEASURE(0.),$,$,$,$);
#84=IFCRELDEFINESBYPROPERTIES('0000000000000000000084',$,$,$,(#80),#85);
#85=IFCPROPERTYSET('0000000000000000000085',$,'Pset_Stationing',$,(#86));
#86=IFCPROPERTYSINGLEVALUE('Station',$,IFCLENGTHMEASURE({:?}),$);
#90=IFCREFERENT('0000000000000000000090',$,'BRK',$,$,#91,$,.STATION.);
#91=IFCLINEARPLACEMENT(#41,#92,$);
#92=IFCAXIS2PLACEMENTLINEAR(#93,$,$);
#93=IFCPOINTBYDISTANCEEXPRESSION(IFCNONNEGATIVELENGTHMEASURE({:?}),$,$,$,$);
#94=IFCRELDEFINESBYPROPERTIES('0000000000000000000094',$,$,$,(#90),#95);
#95=IFCPROPERTYSET('0000000000000000000095',$,'Pset_Stationing',$,(#96,#97));
#96=IFCPROPERTYSINGLEVALUE('Station',$,IFCLENGTHMEASURE({:?}),$);
#97=IFCPROPERTYSINGLEVALUE('IncomingStation',$,IFCLENGTHMEASURE({:?}),$);
{}
ENDSEC;
END-ISO-10303-21;
",
        horizontal_segments.join(","),
        vertical_segments.join(","),
        1000.0 * scale,
        150.0 * scale,
        1140.0 * scale,
        1150.0 * scale,
        data.join("\n"),
    )
}

const METRE: &str = "#10=IFCUNITASSIGNMENT((#11,#12));
#11=IFCSIUNIT(*,.LENGTHUNIT.,$,.METRE.);
#12=IFCSIUNIT(*,.PLANEANGLEUNIT.,$,.RADIAN.);";

const MAP: &str = "#30=IFCMAPCONVERSION(#20,#31,0.,0.,0.,1.,0.,1.,$,$);
#31=IFCPROJECTEDCRS('EPSG:6677',$,'JGD2011','T.P.',$,$,$);";

#[cfg(test)]
fn with_cx<T>(f: impl FnOnce(&mut Cx<'_>) -> T) -> (T, Diagnostics) {
    let mut diag = Diagnostics::new();
    let mut budget = Budget::default();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
    let out = f(&mut cx);
    (out, diag)
}

#[cfg(test)]
fn from_ifc(text: &str) -> (CivilModel, Diagnostics) {
    let (model, diag) = with_cx(|cx| {
        let file = Ifc::read(text.as_bytes(), cx).unwrap();
        Lift::<CivilModel>::lift(&file, cx).unwrap()
    });
    (model, diag)
}

#[cfg(test)]
fn from_landxml() -> CivilModel {
    with_cx(|cx| {
        let file = LandXml::read(LANDXML.as_bytes(), cx).unwrap();
        file.lift(cx).unwrap()
    })
    .0
}

#[cfg(test)]
fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-7
}

/// 区間の値が、J-LandXML から移したものと同じか
#[cfg(test)]
fn same_alignment(ifc: &CivilModel, landxml: &CivilModel) {
    let (Some(a), Some(b)) = (ifc.alignments.first(), landxml.alignments.first()) else {
        panic!("{:?} {:?}", ifc.alignments, landxml.alignments);
    };
    assert_eq!(a.name, b.name);
    assert_eq!(a.horizontal.len(), b.horizontal.len());
    for (x, y) in a.horizontal.iter().zip(&b.horizontal) {
        assert!(near(x.start.northing, y.start.northing), "{x:?} {y:?}");
        assert!(near(x.start.easting, y.start.easting), "{x:?} {y:?}");
        assert!(near(x.direction, y.direction), "{x:?} {y:?}");
        assert!(near(x.length, y.length), "{x:?} {y:?}");
        match (&x.kind, &y.kind) {
            (HorizontalKind::Line, HorizontalKind::Line) => {}
            (HorizontalKind::Arc { radius: r }, HorizontalKind::Arc { radius: s }) => {
                assert!(near(*r, *s));
            }
            (
                HorizontalKind::Transition {
                    start_radius: r0,
                    end_radius: r1,
                    curve: c,
                },
                HorizontalKind::Transition {
                    start_radius: s0,
                    end_radius: s1,
                    curve: d,
                },
            ) => {
                assert_eq!(c, d);
                assert_eq!(r0.is_some(), s0.is_some());
                assert!(near(r1.unwrap_or(0.0), s1.unwrap_or(0.0)), "{r1:?} {s1:?}");
            }
            other => panic!("{other:?}"),
        }
    }
    assert_eq!(a.vertical.len(), b.vertical.len(), "{:?}", a.vertical);
    for (x, y) in a.vertical.iter().zip(&b.vertical) {
        assert!(near(x.start_distance, y.start_distance), "{x:?} {y:?}");
        assert!(near(x.start_height, y.start_height), "{x:?} {y:?}");
        assert!(near(x.start_gradient, y.start_gradient), "{x:?} {y:?}");
        assert!(near(x.length, y.length), "{x:?} {y:?}");
        match (&x.kind, &y.kind) {
            (VerticalKind::ConstantGradient, VerticalKind::ConstantGradient) => {}
            (
                VerticalKind::Parabola { end_gradient: g },
                VerticalKind::Parabola { end_gradient: h },
            ) => assert!(near(*g, *h)),
            other => panic!("{other:?}"),
        }
    }
    assert!(near(a.stations.start, b.stations.start));
    assert_eq!(a.stations.breaks.len(), b.stations.breaks.len());
    for (x, y) in a.stations.breaks.iter().zip(&b.stations.breaks) {
        assert!(near(x.distance, y.distance));
        assert!(near(x.back, y.back));
        assert!(near(x.ahead, y.ahead));
    }
    // 区間の初めの点は、前の区間の終わりから計算した点と合う
    let ((), checked) = with_cx(|cx| zumen_civil::check(a, 1e-6, cx));
    assert!(checked.is_empty(), "{checked:?}");
}

#[test]
fn an_alignment_moves_like_the_same_landxml() {
    let (model, diag) = from_ifc(&ifc(METRE, MAP, 1.0));
    let landxml = from_landxml();
    same_alignment(&model, &landxml);
    assert_eq!(model.crs, Some(Crs::Plane(Zone::new(9).unwrap())));
    assert_eq!(model.crs, landxml.crs);
    assert_eq!(model.realization, landxml.realization);
    assert_eq!(model.height_datum, Some(HeightDatum::TokyoBay));
    let alignment = &model.alignments[0];
    assert_eq!(alignment.id, "0000000000000000000040");
    assert!(diag.is_empty(), "{:?}", diag.iter().collect::<Vec<_>>());
}

#[test]
fn millimetres_degrees_and_the_map_conversion_are_resolved() {
    let units = "#10=IFCUNITASSIGNMENT((#11,#12));
#11=IFCSIUNIT(*,.LENGTHUNIT.,.MILLI.,.METRE.);
#12=IFCCONVERSIONBASEDUNIT(#13,.PLANEANGLEUNIT.,'degree',#14);
#13=IFCDIMENSIONALEXPONENTS(0,0,0,0,0,0,0);
#14=IFCMEASUREWITHUNIT(IFCPLANEANGLEMEASURE(0.017453292519943295),#15);
#15=IFCSIUNIT(*,.PLANEANGLEUNIT.,$,.RADIAN.);";
    // プロジェクトの x 軸を、地図の北に向け、東へ 1000 m、北へ 2000 m ずらす
    let map = "#30=IFCMAPCONVERSION(#20,#31,1000.,2000.,10.,0.,1.,$,$,$);
#31=IFCPROJECTEDCRS('JGD2011 / Japan Plane Rectangular CS IX',$,$,'T.P.',$,$,$);";
    let (model, diag) = from_ifc(&ifc(units, map, 1000.0));
    assert!(diag.is_empty(), "{:?}", diag.iter().collect::<Vec<_>>());
    let alignment = &model.alignments[0];
    let first = &alignment.horizontal[0];
    // 北へ向かう直線は、地図では西へ向かう
    assert!(
        near(first.direction, -core::f64::consts::FRAC_PI_2),
        "{first:?}"
    );
    assert!(near(first.start.easting, 1000.0) && near(first.start.northing, 2000.0));
    let line = &alignment.horizontal[3];
    assert!(near(line.start.easting, 1000.0 - 188.84874198801492));
    assert!(near(line.start.northing, 2000.0 + 10.990397503080032));
    assert!(near(line.length, 60.0));
    assert!(near(alignment.vertical[0].start_height, 60.0));
    assert!(near(alignment.stations.start, 1000.0));
    let ((), checked) = with_cx(|cx| zumen_civil::check(alignment, 1e-6, cx));
    assert!(checked.is_empty(), "{checked:?}");
}

#[test]
fn without_a_map_conversion_the_coordinate_system_is_unknown() {
    let (model, diag) = from_ifc(&ifc(METRE, "", 1.0));
    assert_eq!(model.crs, None);
    let found: Vec<Code> = diag.iter().map(|d| d.code()).collect();
    assert_eq!(found, [codes::IFC_MAP_CRS_MISSING]);
}

#[test]
fn unknown_segments_and_other_versions_are_reported() {
    let text = ifc(METRE, MAP, 1.0).replace(".CLOTHOID.", ".SPIRALX.");
    let (model, diag) = from_ifc(&text);
    assert_eq!(model.alignments[0].horizontal.len(), 3);
    assert!(
        diag.iter()
            .any(|d| d.code() == codes::IFC_ALIGNMENT_SEGMENT_UNSUPPORTED)
    );
    let text = ifc(METRE, MAP, 1.0).replace("IFC4X3_ADD2", "IFC4");
    let ((), diag) = with_cx(|cx| {
        let file = Ifc::read(text.as_bytes(), cx).unwrap();
        assert!(Lift::<CivilModel>::lift(&file, cx).is_err());
    });
    assert!(
        diag.iter()
            .any(|d| d.code() == codes::IFC_VERSION_UNSUPPORTED)
    );
}

/// 4 つの点と 2 つの三角形の現況の地表面
const LANDXML_TIN: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<LandXML xmlns="http://www.landxml.org/schema/LandXML-1.2" version="1.2">
  <CoordinateSystem horizontalDatum="JGD2011" verticalDatum="T.P" horizontalCoordinateSystemName="9(X,Y)"/>
  <Surfaces>
    <Surface name="現況" desc="ExistingGround">
      <Definition surfType="TIN">
        <Pnts><P id="1">100 200 10</P><P id="2">100 220 11</P><P id="3">120 200 12</P><P id="4">120 220 13</P></Pnts>
        <Faces><F>1 2 3</F><F>2 4 3</F></Faces>
      </Definition>
    </Surface>
  </Surfaces>
</LandXML>
"#;

/// 同じ地表面の IFC4.3。地図の原点を東 150 m・北 50 m にずらし、要素を (40, 30, 2) に 90° 回して置く。
/// 点は逆の順に書き、PnIndex で並べ直す
#[cfg(test)]
fn ifc_tin() -> String {
    let world = [
        (100.0, 200.0, 10.0),
        (100.0, 220.0, 11.0),
        (120.0, 200.0, 12.0),
        (120.0, 220.0, 13.0),
    ];
    let local: Vec<String> = world
        .iter()
        .rev()
        .map(|(n, e, z)| {
            let (x, y) = (e - 150.0 - 40.0, n - 50.0 - 30.0);
            // 90° 回した軸の上の値に戻す
            format!("({:?},{:?},{:?})", y, -x, z - 2.0)
        })
        .collect();
    format!(
        "ISO-10303-21;
HEADER;
FILE_DESCRIPTION(('ViewDefinition [Alignment-basedView]'),'2;1');
FILE_NAME('t.ifc','2026-11-01T00:00:00',(''),(''),'','','');
FILE_SCHEMA(('IFC4X3_ADD2'));
ENDSEC;
DATA;
#1=IFCPROJECT('0000000000000000000001',$,'テスト',$,$,$,$,(#20),#10);
{METRE}
#20=IFCGEOMETRICREPRESENTATIONCONTEXT($,'Model',3,1.E-05,#21,$);
#21=IFCAXIS2PLACEMENT3D(#22,$,$);
#22=IFCCARTESIANPOINT((0.,0.,0.));
#30=IFCMAPCONVERSION(#20,#31,150.,50.,0.,1.,0.,1.,$,$);
#31=IFCPROJECTEDCRS('EPSG:6677',$,'JGD2011','T.P.',$,$,$);
#40=IFCGEOGRAPHICELEMENT('0000000000000000000040',$,'\\X2\\73FE6CC1\\X0\\',$,$,#41,#45,$,.TERRAIN.);
#41=IFCLOCALPLACEMENT($,#42);
#42=IFCAXIS2PLACEMENT3D(#43,$,#44);
#43=IFCCARTESIANPOINT((40.,30.,2.));
#44=IFCDIRECTION((0.,1.,0.));
#45=IFCPRODUCTDEFINITIONSHAPE($,$,(#46));
#46=IFCSHAPEREPRESENTATION(#20,'Body','Tessellation',(#47));
#47=IFCTRIANGULATEDIRREGULARNETWORK(#48,$,.F.,((1,2,3),(2,4,3)),(4,3,2,1),(0,0));
#48=IFCCARTESIANPOINTLIST3D(({}),$);
ENDSEC;
END-ISO-10303-21;
",
        local.join(",")
    )
}

#[test]
fn a_tin_moves_like_the_same_landxml() {
    let (model, diag) = from_ifc(&ifc_tin());
    assert!(diag.is_empty(), "{:?}", diag.iter().collect::<Vec<_>>());
    let landxml = with_cx(|cx| {
        let file = LandXml::read(LANDXML_TIN.as_bytes(), cx).unwrap();
        Lift::<CivilModel>::lift(&file, cx).unwrap()
    })
    .0;
    let (Some(a), Some(b)) = (model.surfaces.first(), landxml.surfaces.first()) else {
        panic!("{:?} {:?}", model.surfaces, landxml.surfaces);
    };
    assert_eq!(a.id, "0000000000000000000040");
    assert_eq!(a.name, b.name);
    assert_eq!(a.role, b.role);
    assert_eq!(a.triangles, b.triangles);
    assert_eq!(a.points.len(), b.points.len());
    for ((p, h), (q, k)) in a.points.iter().zip(&b.points) {
        assert!(
            near(p.northing, q.northing) && near(p.easting, q.easting),
            "{p:?} {q:?}"
        );
        assert!(near(*h, *k), "{h} {k}");
    }
    assert_eq!(model.crs, landxml.crs);
}

#[test]
fn triangle_flags_are_reported() {
    let text = ifc_tin().replace("(4,3,2,1),(0,0)", "(4,3,2,1),(0,-1)");
    let (model, diag) = from_ifc(&text);
    assert_eq!(model.surfaces[0].triangles.len(), 2);
    let codes: Vec<Code> = diag.iter().map(|d| d.code()).collect();
    assert_eq!(codes, [codes::IFC_CIVIL_GEOMETRY_DROPPED]);
}

#[test]
fn geographic_projected_crs_is_rejected() {
    for crs_name in ["EPSG:4326", "WGS 84", "EPSG:6668"] {
        let text = ifc_tin().replace("EPSG:6677", crs_name);
        let (model, diag) = from_ifc(&text);
        assert_eq!(model.crs, None, "projected CRS should reject {crs_name}");
        assert!(
            diag.iter()
                .any(|d| d.code() == codes::IFC_MAP_CRS_UNSUPPORTED),
            "expected ifc.map-crs.unsupported for {crs_name}"
        );
    }
}

/// 同じ線形に、追加距離 20 m と 140 m の横断を足した J-LandXML。20 m は幅 3.5 m・2 % の拝み勾配、140 m は
/// 中心から左へ斜めに 3 m・6 % の片勾配。高さは計画高からの差
#[cfg(test)]
fn landxml_cross() -> String {
    let rise = 3.0 * libm::sin(libm::atan(0.06));
    let run = 3.0 * libm::cos(libm::atan(0.06));
    LANDXML.replace(
        "      </Profile>\n",
        &format!(
            r#"      </Profile>
      <CrossSects name="横断">
        <CrossSect sta="1020">
          <DesignCrossSectSurf name="舗装">
            <CrossSectPnt>-3.5 -0.07</CrossSectPnt><CrossSectPnt>0 0</CrossSectPnt><CrossSectPnt>3.5 -0.07</CrossSectPnt>
            <Feature><Property label="heightType" value="fhOffset"/></Feature>
          </DesignCrossSectSurf>
        </CrossSect>
        <CrossSect sta="1140">
          <DesignCrossSectSurf name="舗装">
            <CrossSectPnt>0 0</CrossSectPnt><CrossSectPnt>{:?} {rise:?}</CrossSectPnt>
            <Feature><Property label="heightType" value="fhOffset"/></Feature>
          </DesignCrossSectSurf>
        </CrossSect>
      </CrossSects>
"#,
            -run
        ),
    )
}

/// 同じ横断の IFC4.3。線形の表現に `IfcGradientCurve` を置き、舗装の `IfcSectionedSurface` の準線にする。
/// 140 m の断面は後ろに書き、幅を勾配に沿って測る。`scale` は長さの単位、`angle` は 1 ラジアンの角の値
#[cfg(test)]
fn ifc_cross(units: &str, scale: f64, angle: f64) -> String {
    let slope = libm::atan(0.02);
    let records = format!(
        "#100=IFCPRODUCTDEFINITIONSHAPE($,$,(#101));
#101=IFCSHAPEREPRESENTATION(#20,'Axis','Curve3D',(#102));
#102=IFCGRADIENTCURVE((),.F.,#103,$);
#103=IFCCOMPOSITECURVE((),.F.);
#110=IFCPAVEMENT('0000000000000000000110',$,'舗装',$,$,#41,#111,$,$);
#111=IFCPRODUCTDEFINITIONSHAPE($,$,(#112));
#112=IFCSHAPEREPRESENTATION(#20,'Body','SectionedSurface',(#113));
#113=IFCSECTIONEDSURFACE(#102,(#121,#123),(#131,#130));
#120=IFCPOINTBYDISTANCEEXPRESSION(IFCLENGTHMEASURE({:?}),$,$,$,#102);
#121=IFCAXIS2PLACEMENTLINEAR(#122,$,$);
#122=IFCPOINTBYDISTANCEEXPRESSION(IFCLENGTHMEASURE({:?}),$,$,$,#102);
#123=IFCAXIS2PLACEMENTLINEAR(#120,$,$);
#130=IFCOPENCROSSPROFILEDEF(.CURVE.,$,.T.,({:?},{:?}),({:?},{:?}),$,#132);
#131=IFCOPENCROSSPROFILEDEF(.CURVE.,$,.F.,({:?}),({:?}),$,$);
#132=IFCCARTESIANPOINT(({:?},{:?}));",
        20.0 * scale,
        140.0 * scale,
        3.5 * scale,
        3.5 * scale,
        (core::f64::consts::PI - slope) * angle,
        (core::f64::consts::PI + slope) * angle,
        3.0 * scale,
        libm::atan(0.06) * angle,
        3.5 * scale,
        -0.07 * scale,
    );
    ifc(units, MAP, scale)
        .replace("#41,$,.USERDEFINED.", "#41,#100,.USERDEFINED.")
        .replace("ENDSEC;\nEND-ISO", &format!("{records}\nENDSEC;\nEND-ISO"))
}

/// 横断が、J-LandXML から移したものと同じか
#[cfg(test)]
fn same_cross_sections(ifc: &CivilModel, landxml: &CivilModel) {
    let (a, b) = (&ifc.alignments[0], &landxml.alignments[0]);
    assert_eq!(a.cross_sections.len(), 2, "{:?}", a.cross_sections);
    assert_eq!(a.cross_sections.len(), b.cross_sections.len());
    for (p, q) in a.cross_sections.iter().zip(&b.cross_sections) {
        assert!(
            near(p.distance, q.distance),
            "{} {}",
            p.distance,
            q.distance
        );
        assert_eq!(p.lines.len(), q.lines.len());
        for (l, m) in p.lines.iter().zip(&q.lines) {
            assert_eq!(l.name, m.name);
            assert_eq!(l.role, m.role);
            assert_eq!(l.height_reference, m.height_reference);
            assert_eq!(l.points.len(), m.points.len(), "{l:?} {m:?}");
            for (s, t) in l.points.iter().zip(&m.points) {
                assert!(near(s[0], t[0]) && near(s[1], t[1]), "{s:?} {t:?}");
            }
        }
    }
}

#[test]
fn open_cross_profiles_move_like_the_same_landxml() {
    let landxml = with_cx(|cx| {
        let text = landxml_cross();
        let file = LandXml::read(text.as_bytes(), cx).unwrap();
        file.lift(cx).unwrap()
    })
    .0;
    let (model, diag) = from_ifc(&ifc_cross(METRE, 1.0, 1.0));
    assert!(diag.is_empty(), "{:?}", diag.iter().collect::<Vec<_>>());
    same_cross_sections(&model, &landxml);
    let units = "#10=IFCUNITASSIGNMENT((#11,#12));
#11=IFCSIUNIT(*,.LENGTHUNIT.,.MILLI.,.METRE.);
#12=IFCCONVERSIONBASEDUNIT(#13,.PLANEANGLEUNIT.,'degree',#14);
#13=IFCDIMENSIONALEXPONENTS(0,0,0,0,0,0,0);
#14=IFCMEASUREWITHUNIT(IFCPLANEANGLEMEASURE(0.017453292519943295),#15);
#15=IFCSIUNIT(*,.PLANEANGLEUNIT.,$,.RADIAN.);";
    let (model, diag) = from_ifc(&ifc_cross(units, 1000.0, 180.0 / core::f64::consts::PI));
    assert!(diag.is_empty(), "{:?}", diag.iter().collect::<Vec<_>>());
    same_cross_sections(&model, &landxml);
}

#[test]
fn cross_sections_that_cannot_be_placed_are_dropped() {
    let codes_of = |text: &str| {
        let (model, diag) = from_ifc(text);
        assert!(model.alignments[0].cross_sections.is_empty());
        diag.iter().map(|d| d.code()).collect::<Vec<Code>>()
    };
    let base = ifc_cross(METRE, 1.0, 1.0);
    // 準線が線形の表現にない
    let text = base.replace(",(#102));", ",());");
    let codes = codes_of(&text);
    assert!(
        !codes.is_empty()
            && codes
                .iter()
                .all(|c| *c == codes::IFC_CIVIL_GEOMETRY_DROPPED)
    );
    // 横へずらした位置
    let text = base.replace(
        "IFCLENGTHMEASURE(20.0),$,$,$",
        "IFCLENGTHMEASURE(20.0),1.,$,$",
    );
    assert!(codes_of(&text).contains(&codes::IFC_ATTRIBUTE_INVALID));
    // 鉛直でない軸
    let text = base.replace(
        "#121=IFCAXIS2PLACEMENTLINEAR(#122,$,$);",
        "#121=IFCAXIS2PLACEMENTLINEAR(#122,#124,$);\n#124=IFCDIRECTION((0.,1.,0.));",
    );
    assert!(codes_of(&text).contains(&codes::IFC_PLACEMENT_UNSUPPORTED));
    // 水平の幅で、鉛直の勾配
    let text = base.replace(
        &format!("{:?}),$,$);", libm::atan(0.06)),
        &format!("{:?}),$,$);", core::f64::consts::FRAC_PI_2),
    );
    let text = text.replace(".CURVE.,$,.F.,", ".CURVE.,$,.T.,");
    assert!(codes_of(&text).contains(&codes::IFC_ATTRIBUTE_INVALID));
}

/// 線形に沿って置いた信号。1 つ目は縦断の曲線から左へ 3、上へ 2.5。2 つ目は平面の曲線から右へ 1、上へ 4、
/// 先へ 0.5 で、向きの軸を書く
#[cfg(test)]
fn ifc_signals(units: &str, scale: f64) -> String {
    let records = format!(
        "#140=IFCSIGNAL('0000000000000000000140',$,'信号',$,$,#141,$,$,$);
#141=IFCLINEARPLACEMENT($,#142,$);
#142=IFCAXIS2PLACEMENTLINEAR(#143,$,$);
#143=IFCPOINTBYDISTANCEEXPRESSION(IFCLENGTHMEASURE({:?}),IFCLENGTHMEASURE({:?}),IFCLENGTHMEASURE({:?}),$,#102);
#150=IFCSIGNAL('0000000000000000000150',$,$,$,$,#151,$,$,$);
#151=IFCLINEARPLACEMENT($,#152,$);
#152=IFCAXIS2PLACEMENTLINEAR(#153,#154,$);
#153=IFCPOINTBYDISTANCEEXPRESSION(IFCLENGTHMEASURE({:?}),IFCLENGTHMEASURE({:?}),IFCLENGTHMEASURE({:?}),IFCLENGTHMEASURE({:?}),#103);
#154=IFCDIRECTION((0.,0.,1.));",
        30.0 * scale,
        3.0 * scale,
        2.5 * scale,
        50.0 * scale,
        -scale,
        4.0 * scale,
        0.5 * scale,
    );
    ifc_cross(units, scale, 1.0)
        .replace("ENDSEC;\nEND-ISO", &format!("{records}\nENDSEC;\nEND-ISO"))
}

#[test]
fn linear_placements_become_positions_along_the_alignment() {
    let mm = "#10=IFCUNITASSIGNMENT((#11,#12));
#11=IFCSIUNIT(*,.LENGTHUNIT.,.MILLI.,.METRE.);
#12=IFCSIUNIT(*,.PLANEANGLEUNIT.,$,.RADIAN.);";
    for (units, scale) in [(METRE, 1.0), (mm, 1000.0)] {
        let (positions, diag) = with_cx(|cx| {
            let file = Ifc::read(ifc_signals(units, scale).as_bytes(), cx).unwrap();
            Lift::<Vec<LinearPosition>>::lift(&file, cx).unwrap()
        });
        assert_eq!(positions.len(), 2, "{positions:?}");
        let (a, b) = (&positions[0], &positions[1]);
        assert_eq!(a.element, "0000000000000000000140");
        assert_eq!(a.name.as_deref(), Some("信号"));
        assert_eq!(a.kind, "IFCSIGNAL");
        assert_eq!(a.alignment, "0000000000000000000040");
        assert!(near(a.distance, 30.0) && near(a.offset, -3.0) && near(a.height, 2.5));
        assert_eq!(a.height_reference, CrossSectionHeight::FormationOffset);
        assert!(near(a.longitudinal, 0.0));
        assert_eq!(b.element, "0000000000000000000150");
        assert_eq!(b.name, None);
        assert!(near(b.distance, 50.0) && near(b.offset, 1.0) && near(b.height, 4.0));
        assert_eq!(b.height_reference, CrossSectionHeight::Elevation);
        assert!(near(b.longitudinal, 0.5));
        let codes: Vec<Code> = diag.iter().map(|d| d.code()).collect();
        assert_eq!(codes, vec![codes::IFC_CIVIL_GEOMETRY_DROPPED], "{diag:?}");
    }
}

#[test]
fn linear_placements_on_unknown_curves_are_reported() {
    let text = ifc_signals(METRE, 1.0).replace(
        "IFCLENGTHMEASURE(2.5),$,#102);",
        "IFCLENGTHMEASURE(2.5),$,#113);",
    );
    let (positions, diag) = with_cx(|cx| {
        let file = Ifc::read(text.as_bytes(), cx).unwrap();
        Lift::<Vec<LinearPosition>>::lift(&file, cx).unwrap()
    });
    assert_eq!(positions.len(), 1, "{positions:?}");
    assert!(
        diag.iter()
            .any(|d| d.code() == codes::IFC_ATTRIBUTE_INVALID),
        "{diag:?}"
    );
}
