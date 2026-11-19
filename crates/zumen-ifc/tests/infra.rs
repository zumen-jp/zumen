//! IFC4.3 の道路・鉄道・橋の空間・要素・型と `IfcLinearPlacement` を確かめる（#376）。
//! 試験の入力はすべてこのファイルの中で組み立てる。
#![cfg(all(feature = "building", feature = "civil"))]
#![cfg(test)]

use zumen_base::{Budget, CancelToken, Code, Cx, Diagnostics, Kind, Limit, Limits, codes};
use zumen_building::{
    Building, ElementRef, GlobalId, Id, Local, ParentKind, SpatialKind, Transform,
};
use zumen_civil::{CivilModel, CrossSectionHeight, LinearPosition};
use zumen_ifc::{Ifc, IfcFile};
use zumen_sdk::{Codec, Error, Lift};

fn gid(n: u32) -> String {
    format!("{n:0>22}")
}

fn with_limits<T>(limits: Limits, f: impl FnOnce(&mut Cx<'_>) -> T) -> (T, Diagnostics, Budget) {
    let mut budget = Budget::new(limits);
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let out = f(&mut Cx::new(&mut budget, &mut diag, &cancel));
    (out, diag, budget)
}

fn with_cx<T>(f: impl FnOnce(&mut Cx<'_>) -> T) -> (T, Diagnostics) {
    let (out, diag, _) = with_limits(Limits::default(), f);
    (out, diag)
}

fn read_ifc(src: &str) -> IfcFile {
    with_cx(|cx| Ifc::read(src.as_bytes(), cx).unwrap()).0
}

fn by_gid(b: &Building, id: &str) -> Option<ElementRef> {
    b.find(Id::Global(GlobalId::parse(id)?))
}

fn world_transform(b: &Building, start: ElementRef) -> Option<Transform> {
    let mut chain = Vec::new();
    let mut cur = Some(start);
    while let Some(el) = cur {
        let p = b.components.placement.get(el)?;
        let Local::Transform(t) = p.local else {
            return None;
        };
        chain.push(t);
        cur = p.relative_to;
    }
    let identity = Transform([
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
    ]);
    Some(
        chain
            .into_iter()
            .rev()
            .fold(identity, |parent, child| parent.then(&child)),
    )
}

fn near(a: f64, b: f64, tol: f64) -> bool {
    libm::fabs(a - b) <= tol
}

fn near_vec(a: [f64; 3], b: [f64; 3], tol: f64) -> bool {
    near(a[0], b[0], tol) && near(a[1], b[1], tol) && near(a[2], b[2], tol)
}

/// 道路・鉄道・橋の空間と要素・型、および線形配置（直線 100 m → クロソイド 50 m → 円弧 40 m、
/// 縦断は勾配 2 % → 放物線 40 m → -2 %）を含む合成 IFC4.3。
fn synthetic_infra_ifc(scale: f64, degree: bool) -> String {
    let units = if degree {
        format!(
            "#10=IFCUNITASSIGNMENT((#11,#12));\n\
             #11=(IFCNAMEDUNIT(*,.LENGTHUNIT.)IFCSIUNIT({prefix},.METRE.));\n\
             #12=IFCCONVERSIONBASEDUNIT(#13,.PLANEANGLEUNIT.,'degree',#14);\n\
             #13=IFCDIMENSIONALEXPONENTS(0,0,0,0,0,0,0);\n\
             #14=IFCMEASUREWITHUNIT(IFCPLANEANGLEMEASURE(0.017453292519943295),#15);\n\
             #15=IFCSIUNIT(*,.PLANEANGLEUNIT.,$,.RADIAN.);",
            prefix = if scale > 1.0 { ".MILLI." } else { "$" }
        )
    } else {
        format!(
            "#10=IFCUNITASSIGNMENT((#11,#12));\n\
             #11=IFCSIUNIT(*,.LENGTHUNIT.,{prefix},.METRE.);\n\
             #12=IFCSIUNIT(*,.PLANEANGLEUNIT.,$,.RADIAN.);",
            prefix = if scale > 1.0 { ".MILLI." } else { "$" }
        )
    };
    let ang = if degree {
        180.0 / core::f64::consts::PI
    } else {
        1.0
    };
    // 施設・線形の局所配置にオフセット (10 m, 20 m, 5 m) を与え、PlacementRelTo の畳み込みも検証する
    let facility_offset = [10.0 * scale, 20.0 * scale, 5.0 * scale];
    let align_offset = [2.0 * scale, -3.0 * scale, 1.0 * scale];

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
    ];
    let vertical = [
        (0.0, 80.0, 50.0, 0.02, 0.02, "CONSTANTGRADIENT", "$"),
        (80.0, 40.0, 51.6, 0.02, -0.02, "PARABOLICARC", "$"),
        (120.0, 70.0, 51.6, -0.02, -0.02, "CONSTANTGRADIENT", "$"),
    ];

    let mut segs = Vec::new();
    let mut h_refs = Vec::new();
    for (i, (x, y, dir, r0, r1, len, kind)) in horizontal.iter().enumerate() {
        let id = 200 + (i as u32) * 3;
        segs.push(format!(
            "#{id}=IFCALIGNMENTSEGMENT('{}',$,'H{i}',$,$,$,$,#{});",
            gid(id),
            id + 1
        ));
        segs.push(format!(
            "#{}=IFCALIGNMENTHORIZONTALSEGMENT($,$,#{},{:?},{:?},{:?},{:?},$,.{kind}.);",
            id + 1,
            id + 2,
            dir * ang,
            r0 * scale,
            r1 * scale,
            len * scale
        ));
        segs.push(format!(
            "#{}=IFCCARTESIANPOINT(({:?},{:?}));",
            id + 2,
            x * scale,
            y * scale
        ));
        h_refs.push(format!("#{id}"));
    }
    let mut v_refs = Vec::new();
    for (i, (s0, len, h0, g0, g1, kind, rad)) in vertical.iter().enumerate() {
        let id = 300 + (i as u32) * 2;
        segs.push(format!(
            "#{id}=IFCALIGNMENTSEGMENT('{}',$,'V{i}',$,$,$,$,#{});",
            gid(id),
            id + 1
        ));
        segs.push(format!(
            "#{}=IFCALIGNMENTVERTICALSEGMENT($,$,{:?},{:?},{:?},{g0:?},{g1:?},{rad},.{kind}.);",
            id + 1,
            s0 * scale,
            len * scale,
            h0 * scale
        ));
        v_refs.push(format!("#{id}"));
    }

    format!(
        "ISO-10303-21;\n\
HEADER;\n\
FILE_DESCRIPTION(('ViewDefinition [Alignment-basedView]'),'2;1');\n\
FILE_NAME('infra.ifc','2026-11-19T00:00:00',(''),(''),'','','');\n\
FILE_SCHEMA(('IFC4X3_ADD2'));\n\
ENDSEC;\n\
DATA;\n\
#1=IFCPROJECT('{p1}',$,'InfraProject',$,$,$,$,(#20),#10);\n\
{units}\n\
#20=IFCGEOMETRICREPRESENTATIONCONTEXT('3D','Model',3,1.E-05,#21,$);\n\
#21=IFCAXIS2PLACEMENT3D(#22,#23,#24);\n\
#22=IFCCARTESIANPOINT((0.,0.,0.));\n\
#23=IFCDIRECTION((0.,0.,1.));\n\
#24=IFCDIRECTION((1.,0.,0.));\n\
#25=IFCDIRECTION((0.,1.,0.));\n\
#30=IFCMAPCONVERSION(#20,#31,1000.,2000.,10.,0.8,0.6,1.,$,1.);\n\
#31=IFCPROJECTEDCRS('EPSG:6677',$,'JGD2011','T.P.',$,$,$);\n\
#40=IFCSITE('{p40}',$,'Site',$,$,#41,$,'MainSite',.ELEMENT.,$,$,$,$,$);\n\
#41=IFCLOCALPLACEMENT($,#21);\n\
#50=IFCRAILWAY('{p50}',$,'MainLine',$,'rail',#51,$,'本線',.ELEMENT.,.NOTDEFINED.);\n\
#51=IFCLOCALPLACEMENT(#41,#52);\n\
#52=IFCAXIS2PLACEMENT3D(#53,#23,#24);\n\
#53=IFCCARTESIANPOINT(({fx:?},{fy:?},{fz:?}));\n\
#54=IFCRAILWAYPART('{p54}',$,'Track-1',$,'track',#55,$,'軌道部',.ELEMENT.,.LONGITUDINAL.,.TRACK.);\n\
#55=IFCLOCALPLACEMENT(#51,#21);\n\
#60=IFCROAD('{p60}',$,'AccessRoad',$,'road',#61,$,'取付道路',.ELEMENT.,.HIGHWAY.);\n\
#61=IFCLOCALPLACEMENT(#41,#21);\n\
#62=IFCROADPART('{p62}',$,'Carriageway-1',$,'carriageway',#63,$,'車道部',.ELEMENT.,.LATERAL.,.CARRIAGEWAY.);\n\
#63=IFCLOCALPLACEMENT(#61,#21);\n\
#70=IFCBRIDGE('{p70}',$,'Overpass',$,'girder',#71,$,'跨線橋',.ELEMENT.,.GIRDER.);\n\
#71=IFCLOCALPLACEMENT(#41,#21);\n\
#72=IFCBRIDGEPART('{p72}',$,'Pier-1',$,'pier',#73,$,'橋脚P1',.ELEMENT.,.VERTICAL.,.PIER.);\n\
#73=IFCLOCALPLACEMENT(#71,#21);\n\
#80=IFCRELAGGREGATES('{p80}',$,$,$,#1,(#40,#100));\n\
#81=IFCRELAGGREGATES('{p81}',$,$,$,#40,(#50,#60,#70));\n\
#82=IFCRELAGGREGATES('{p82}',$,$,$,#50,(#54));\n\
#83=IFCRELAGGREGATES('{p83}',$,$,$,#60,(#62));\n\
#84=IFCRELAGGREGATES('{p84}',$,$,$,#70,(#72));\n\
#100=IFCALIGNMENT('{p100}',$,'Alignment-1',$,$,#101,#105,.USERDEFINED.);\n\
#101=IFCLOCALPLACEMENT(#51,#102);\n\
#102=IFCAXIS2PLACEMENT3D(#103,#23,#24);\n\
#103=IFCCARTESIANPOINT(({ax:?},{ay:?},{az:?}));\n\
#105=IFCPRODUCTDEFINITIONSHAPE($,$,(#106));\n\
#106=IFCSHAPEREPRESENTATION(#20,'Axis','Curve3D',(#107));\n\
#107=IFCGRADIENTCURVE((),.F.,#108,$);\n\
#108=IFCCOMPOSITECURVE((),.F.);\n\
#109=IFCSEGMENTEDREFERENCECURVE((),.F.,#107,$);\n\
#110=IFCALIGNMENTHORIZONTAL('{p110}',$,$,$,$,$,$);\n\
#111=IFCALIGNMENTVERTICAL('{p111}',$,$,$,$,$,$);\n\
#112=IFCREFERENT('{p112}',$,'STA0',$,$,#113,$,.STATION.);\n\
#113=IFCLINEARPLACEMENT(#101,#114,$);\n\
#114=IFCAXIS2PLACEMENTLINEAR(#115,$,$);\n\
#115=IFCPOINTBYDISTANCEEXPRESSION(IFCNONNEGATIVELENGTHMEASURE(0.),$,$,$,#107);\n\
#116=IFCPROPERTYSET('{p116}',$,'Pset_Stationing',$,(#117));\n\
#117=IFCPROPERTYSINGLEVALUE('Station',$,IFCLENGTHMEASURE(0.),$);\n\
#118=IFCRELDEFINESBYPROPERTIES('{p118}',$,$,$,(#112),#116);\n\
#120=IFCRELNESTS('{p120}',$,$,$,#100,(#110,#111,#112));\n\
#121=IFCRELNESTS('{p121}',$,$,$,#110,({h_list}));\n\
#122=IFCRELNESTS('{p122}',$,$,$,#111,({v_list}));\n\
{segs}\n\
#400=IFCSIGNALTYPE('{p400}',$,'MainSignalType',$,$,$,$,'ST-1','visual',.VISUAL.);\n\
#401=IFCSIGNAL('{p401}',$,'Signal-Grad',$,'visual',#402,$,'SIG-1',.VISUAL.);\n\
#402=IFCLINEARPLACEMENT(#101,#403,$);\n\
#403=IFCAXIS2PLACEMENTLINEAR(#404,$,$);\n\
#404=IFCPOINTBYDISTANCEEXPRESSION(IFCLENGTHMEASURE({d1:?}),{lat1:?},{vert1:?},{lon1:?},#107);\n\
#410=IFCSIGNTYPE('{p410}',$,'RoadSignType',$,$,$,$,'RS-1','pictoral',.PICTORAL.);\n\
#411=IFCSIGN('{p411}',$,'Sign-Comp',$,'pictoral',#412,$,'SGN-1',.PICTORAL.);\n\
#412=IFCLINEARPLACEMENT($,#413,$);\n\
#413=IFCAXIS2PLACEMENTLINEAR(#414,#23,$);\n\
#414=IFCPOINTBYDISTANCEEXPRESSION(IFCLENGTHMEASURE({d2:?}),{lat2:?},{vert2:?},$,#108);\n\
#420=IFCCOURSETYPE('{p420}',$,'AsphaltSurface',$,$,$,$,'CS-1','pavement',.PAVEMENT.);\n\
#421=IFCCOURSE('{p421}',$,'SurfaceCourse',$,'pavement',#63,$,'CRS-1',.PAVEMENT.);\n\
#422=IFCEARTHWORKSFILL('{p422}',$,'Subgrade',$,'subgrade',#63,$,'EWF-1',.SUBGRADE.);\n\
#423=IFCKERB('{p423}',$,'RoadKerb',$,'kerb',#63,$,'KRB-1',$);\n\
#424=IFCPAVEMENT('{p424}',$,'RoadPavement',$,'flexible',#63,$,'PAV-1',.FLEXIBLE.);\n\
#430=IFCRAILTYPE('{p430}',$,'60kgRail',$,$,$,$,'RL-1','rail',.RAIL.);\n\
#431=IFCRAIL('{p431}',$,'LeftRail',$,'rail',#55,$,'R-L',.RAIL.);\n\
#432=IFCTRACKELEMENTTYPE('{p432}',$,'PCSleeper',$,$,$,$,'TE-1','sleeper',.SLEEPER.);\n\
#433=IFCTRACKELEMENT('{p433}',$,'Sleeper-01',$,'sleeper',#55,$,'SL-1',.SLEEPER.);\n\
#440=IFCBEARINGTYPE('{p440}',$,'RubberBearing',$,$,$,$,'BRG-T','elastomeric',.ELASTOMERIC.);\n\
#441=IFCBEARING('{p441}',$,'Bearing-P1',$,'elastomeric',#442,$,'BRG-1',.ELASTOMERIC.);\n\
#442=IFCLINEARPLACEMENT(#101,#443,#445);\n\
#443=IFCAXIS2PLACEMENTLINEAR(#444,$,$);\n\
#444=IFCPOINTBYDISTANCEEXPRESSION(IFCLENGTHMEASURE({d3:?}),0.,0.,0.,#109);\n\
#445=IFCAXIS2PLACEMENT3D(#446,#23,#25);\n\
#446=IFCCARTESIANPOINT(({bx:?},{by:?},{bz:?}));\n\
#447=IFCCAISSONFOUNDATION('{p447}',$,'Caisson-P1',$,'caisson',#73,$,'CF-1',$);\n\
#448=IFCGEOGRAPHICELEMENT('{p448}',$,'ExistingTerrain',$,'terrain',#41,$,'GEO-1',.TERRAIN.);\n\
#460=IFCRELDEFINESBYTYPE('{p460}',$,$,$,(#401),#400);\n\
#461=IFCRELDEFINESBYTYPE('{p461}',$,$,$,(#411),#410);\n\
#462=IFCRELDEFINESBYTYPE('{p462}',$,$,$,(#421),#420);\n\
#463=IFCRELDEFINESBYTYPE('{p463}',$,$,$,(#431),#430);\n\
#464=IFCRELDEFINESBYTYPE('{p464}',$,$,$,(#433),#432);\n\
#465=IFCRELDEFINESBYTYPE('{p465}',$,$,$,(#441),#440);\n\
#470=IFCRELCONTAINEDINSPATIALSTRUCTURE('{p470}',$,$,$,(#401,#431,#433),#54);\n\
#471=IFCRELCONTAINEDINSPATIALSTRUCTURE('{p471}',$,$,$,(#411,#421,#422,#423,#424),#62);\n\
#472=IFCRELCONTAINEDINSPATIALSTRUCTURE('{p472}',$,$,$,(#441,#447),#72);\n\
#473=IFCRELCONTAINEDINSPATIALSTRUCTURE('{p473}',$,$,$,(#448),#40);\n\
ENDSEC;\n\
END-ISO-10303-21;\n",
        p1 = gid(1),
        units = units,
        p40 = gid(40),
        p50 = gid(50),
        fx = facility_offset[0],
        fy = facility_offset[1],
        fz = facility_offset[2],
        p54 = gid(54),
        p60 = gid(60),
        p62 = gid(62),
        p70 = gid(70),
        p72 = gid(72),
        p80 = gid(80),
        p81 = gid(81),
        p82 = gid(82),
        p83 = gid(83),
        p84 = gid(84),
        p100 = gid(100),
        ax = align_offset[0],
        ay = align_offset[1],
        az = align_offset[2],
        p110 = gid(110),
        p111 = gid(111),
        p112 = gid(112),
        p116 = gid(116),
        p118 = gid(118),
        p120 = gid(120),
        p121 = gid(121),
        p122 = gid(122),
        h_list = h_refs.join(","),
        v_list = v_refs.join(","),
        segs = segs.join("\n"),
        p400 = gid(400),
        p401 = gid(401),
        d1 = 130.0 * scale,
        lat1 = 3.0 * scale,
        vert1 = 2.5 * scale,
        lon1 = 0.4 * scale,
        p410 = gid(410),
        p411 = gid(411),
        d2 = 165.0 * scale,
        lat2 = -2.0 * scale,
        vert2 = 4.0 * scale,
        p420 = gid(420),
        p421 = gid(421),
        p422 = gid(422),
        p423 = gid(423),
        p424 = gid(424),
        p430 = gid(430),
        p431 = gid(431),
        p432 = gid(432),
        p433 = gid(433),
        p440 = gid(440),
        p441 = gid(441),
        d3 = 50.0 * scale,
        bx = 15.0 * scale,
        by = 50.0 * scale,
        bz = 52.0 * scale,
        p447 = gid(447),
        p448 = gid(448),
        p460 = gid(460),
        p461 = gid(461),
        p462 = gid(462),
        p463 = gid(463),
        p464 = gid(464),
        p465 = gid(465),
        p470 = gid(470),
        p471 = gid(471),
        p472 = gid(472),
        p473 = gid(473),
    )
}

#[test]
fn lifts_road_railway_bridge_spatial_hierarchy_elements_types_and_linear_placements() {
    for (scale, degree) in [(1.0, false), (1000.0, true)] {
        let src = synthetic_infra_ifc(scale, degree);
        let file = read_ifc(&src);
        let (building, b_diag) = with_cx(|cx| Lift::<Building>::lift(&file, cx).unwrap());
        let (civil, c_diag) = with_cx(|cx| Lift::<CivilModel>::lift(&file, cx).unwrap());
        let (positions, _) = with_cx(|cx| Lift::<Vec<LinearPosition>>::lift(&file, cx).unwrap());
        assert!(c_diag.is_empty(), "{c_diag:?}");
        assert!(
            !b_diag
                .iter()
                .any(|d| d.code() == codes::IFC_PLACEMENT_INVALID
                    || d.code() == codes::IFC_PLACEMENT_UNSUPPORTED),
            "{b_diag:?}"
        );

        // 空間階層の検証: Project -> Site -> {Railway -> RailwayPart, Road -> RoadPart, Bridge -> BridgePart}
        let project = by_gid(&building, &gid(1)).unwrap();
        let site = by_gid(&building, &gid(40)).unwrap();
        let railway = by_gid(&building, &gid(50)).unwrap();
        let railway_part = by_gid(&building, &gid(54)).unwrap();
        let road = by_gid(&building, &gid(60)).unwrap();
        let road_part = by_gid(&building, &gid(62)).unwrap();
        let bridge = by_gid(&building, &gid(70)).unwrap();
        let bridge_part = by_gid(&building, &gid(72)).unwrap();

        for (child, parent) in [
            (site, project),
            (railway, site),
            (railway_part, railway),
            (road, site),
            (road_part, road),
            (bridge, site),
            (bridge_part, bridge),
        ] {
            let m = building
                .components
                .parent
                .get(child)
                .unwrap()
                .of
                .as_ref()
                .unwrap();
            assert_eq!(m.target, parent);
            assert_eq!(m.kind, ParentKind::Aggregates);
        }

        assert_eq!(
            building.components.spatial.get(railway_part).unwrap().kind,
            SpatialKind::Other("IfcRailwayPart".to_owned())
        );
        assert_eq!(
            building
                .components
                .class
                .get(railway_part)
                .unwrap()
                .predefined
                .as_deref(),
            Some("TRACK")
        );
        assert_eq!(
            building
                .components
                .identity
                .get(railway_part)
                .unwrap()
                .long_name
                .as_deref(),
            Some("軌道部")
        );
        assert_eq!(
            building
                .components
                .class
                .get(road_part)
                .unwrap()
                .predefined
                .as_deref(),
            Some("CARRIAGEWAY")
        );
        assert_eq!(
            building
                .components
                .class
                .get(bridge_part)
                .unwrap()
                .predefined
                .as_deref(),
            Some("PIER")
        );

        // 物理要素・型・空間所属の検証
        let signal = by_gid(&building, &gid(401)).unwrap();
        let signal_type = by_gid(&building, &gid(400)).unwrap();
        let sign = by_gid(&building, &gid(411)).unwrap();
        let course = by_gid(&building, &gid(421)).unwrap();
        let fill = by_gid(&building, &gid(422)).unwrap();
        let kerb = by_gid(&building, &gid(423)).unwrap();
        let pavement = by_gid(&building, &gid(424)).unwrap();
        let rail = by_gid(&building, &gid(431)).unwrap();
        let sleeper = by_gid(&building, &gid(433)).unwrap();
        let bearing = by_gid(&building, &gid(441)).unwrap();
        let caisson = by_gid(&building, &gid(447)).unwrap();
        let geo = by_gid(&building, &gid(448)).unwrap();

        assert_eq!(
            building.components.r#type.get(signal).unwrap().target,
            signal_type
        );
        assert_eq!(
            building
                .components
                .class
                .get(signal_type)
                .unwrap()
                .predefined
                .as_deref(),
            Some("VISUAL")
        );
        assert_eq!(
            building
                .components
                .parent
                .get(signal)
                .unwrap()
                .of
                .as_ref()
                .unwrap()
                .target,
            railway_part
        );
        for el in [rail, sleeper] {
            assert_eq!(
                building
                    .components
                    .parent
                    .get(el)
                    .unwrap()
                    .of
                    .as_ref()
                    .unwrap()
                    .target,
                railway_part
            );
        }
        for el in [sign, course, fill, kerb, pavement] {
            assert_eq!(
                building
                    .components
                    .parent
                    .get(el)
                    .unwrap()
                    .of
                    .as_ref()
                    .unwrap()
                    .target,
                road_part
            );
        }
        for el in [bearing, caisson] {
            assert_eq!(
                building
                    .components
                    .parent
                    .get(el)
                    .unwrap()
                    .of
                    .as_ref()
                    .unwrap()
                    .target,
                bridge_part
            );
        }
        assert_eq!(
            building
                .components
                .parent
                .get(geo)
                .unwrap()
                .of
                .as_ref()
                .unwrap()
                .target,
            site
        );

        // PlacementRelTo の検証:
        // Signal-Grad (#402) は #101 (IfcAlignment の配置 → #51 IfcRailway) を指すため relative_to == Some(railway)
        assert_eq!(
            building
                .components
                .placement
                .get(signal)
                .unwrap()
                .relative_to,
            Some(railway)
        );
        // Sign-Comp (#412) は PlacementRelTo が $ のため relative_to == None かつ IfcAlignment の配置が畳み込まれる
        assert_eq!(
            building.components.placement.get(sign).unwrap().relative_to,
            None
        );

        // 建物モデルの 3D 世界座標（IfcMapConversion 適用後）と、土木モデルの LinearPosition + zumen_civil::place の完全一致検証
        let map = building.document.map.as_ref().unwrap();
        let (cos, sin) = (
            map.x_axis_abscissa.unwrap_or(1.0),
            map.x_axis_ordinate.unwrap_or(0.0),
        );
        let to_map = |[px, py, pz]: [f64; 3]| -> [f64; 3] {
            let xm = px / scale;
            let ym = py / scale;
            let zm = pz / scale;
            [
                map.eastings + (xm * cos - ym * sin),
                map.northings + (xm * sin + ym * cos),
                map.height + zm,
            ]
        };

        let alignment = &civil.alignments[0];
        for (el, gid_str) in [(signal, gid(401)), (sign, gid(411))] {
            let wt = world_transform(&building, el).unwrap();
            let b_map = to_map([wt.0[0][3], wt.0[1][3], wt.0[2][3]]);
            let pos = positions.iter().find(|p| p.element == gid_str).unwrap();
            let (placed, _) =
                with_cx(|cx| zumen_civil::place(alignment, pos.distance, cx).unwrap());
            let te = libm::sin(placed.direction);
            let tn = libm::cos(placed.direction);
            // 左向き水平法線 = (-tn, te, 0)、pos.offset は右が正なので左は -pos.offset
            let lat = -pos.offset;
            let (g, base_z) = match pos.height_reference {
                CrossSectionHeight::FormationOffset => {
                    (placed.gradient.unwrap(), placed.height.unwrap())
                }
                CrossSectionHeight::Elevation => (0.0, pos.height),
            };
            let inv_h = 1.0 / libm::hypot(1.0, g);
            let t3 = [te * inv_h, tn * inv_h, g * inv_h];
            let n3 = [-g * te * inv_h, -g * tn * inv_h, inv_h];
            let vert = match pos.height_reference {
                CrossSectionHeight::FormationOffset => pos.height,
                CrossSectionHeight::Elevation => 0.0,
            };
            let c_map = [
                placed.point.easting + lat * (-tn) + vert * n3[0] + pos.longitudinal * t3[0],
                placed.point.northing + lat * te + vert * n3[1] + pos.longitudinal * t3[1],
                base_z + vert * n3[2] + pos.longitudinal * t3[2],
            ];
            assert!(
                near_vec(b_map, c_map, 1e-9),
                "scale={scale} {gid_str}: building={b_map:?} vs civil={c_map:?}"
            );
        }

        // IfcGradientCurve 上の信号 (#401) の Z 軸は縦断勾配に応じて傾き、
        // Axis=(0,0,1) を明示した標識 (#411) の Z 軸は鉛直 (0,0,1) になる
        let sig_wt = world_transform(&building, signal).unwrap();
        assert!(libm::fabs(sig_wt.0[0][2]) > 1e-4 || libm::fabs(sig_wt.0[1][2]) > 1e-4);
        assert!(sig_wt.0[2][2] > 0.999);
        let sign_wt = world_transform(&building, sign).unwrap();
        assert!(near_vec(
            [sign_wt.0[0][2], sign_wt.0[1][2], sign_wt.0[2][2]],
            [0.0, 0.0, 1.0],
            1e-12
        ));

        // IfcSegmentedReferenceCurve (#109) 上の支承 (#441) は CartesianPosition (#445) にフォールバックする
        let bearing_wt = world_transform(&building, bearing).unwrap();
        assert!(near_vec(
            [bearing_wt.0[0][3], bearing_wt.0[1][3], bearing_wt.0[2][3]],
            [
                (10.0 + 2.0 + 15.0) * scale,
                (20.0 - 3.0 + 50.0) * scale,
                (5.0 + 1.0 + 52.0) * scale
            ],
            1e-9
        ));
    }
}

#[test]
fn reports_unsupported_and_invalid_linear_placements_and_old_schema_drops() {
    let base = synthetic_infra_ifc(1.0, false);

    // IfcSegmentedReferenceCurve で CartesianPosition が $ なら ifc.placement.unsupported
    let no_cp = base.replace(
        "#442=IFCLINEARPLACEMENT(#101,#443,#445);",
        "#442=IFCLINEARPLACEMENT(#101,#443,$);",
    );
    let (b, diag) = with_cx(|cx| Lift::<Building>::lift(&read_ifc(&no_cp), cx).unwrap());
    let bearing = by_gid(&b, &gid(441)).unwrap();
    assert_eq!(
        b.components.placement.get(bearing).unwrap().local,
        Local::Unsupported("IfcLinearPlacement".to_owned())
    );
    assert!(
        diag.iter()
            .any(|d| d.code() == codes::IFC_PLACEMENT_UNSUPPORTED)
    );

    // Axis と RefDirection が平行（WR2 違反）なら ifc.placement.invalid
    let parallel = base.replace(
        "#413=IFCAXIS2PLACEMENTLINEAR(#414,#23,$);",
        "#413=IFCAXIS2PLACEMENTLINEAR(#414,#23,#23);",
    );
    let (b, diag) = with_cx(|cx| Lift::<Building>::lift(&read_ifc(&parallel), cx).unwrap());
    let sign = by_gid(&b, &gid(411)).unwrap();
    assert!(b.components.placement.get(sign).is_none());
    assert!(
        diag.iter()
            .any(|d| d.code() == codes::IFC_PLACEMENT_INVALID)
    );

    // DistanceAlong が線形範囲外なら ifc.placement.invalid
    let out_of_range = base.replace(
        "#404=IFCPOINTBYDISTANCEEXPRESSION(IFCLENGTHMEASURE(130.0)",
        "#404=IFCPOINTBYDISTANCEEXPRESSION(IFCLENGTHMEASURE(999.0)",
    );
    let (b, diag) = with_cx(|cx| Lift::<Building>::lift(&read_ifc(&out_of_range), cx).unwrap());
    let sig = by_gid(&b, &gid(401)).unwrap();
    assert!(b.components.placement.get(sig).is_none());
    assert!(
        diag.iter()
            .any(|d| d.code() == codes::IFC_PLACEMENT_INVALID)
    );

    // IFC4 では IfcGeographicElement は残るが、IFC4.3 固有の IfcSignal・IfcRoadPart などは ifc.entity.dropped になる
    let ifc4 = base.replace("FILE_SCHEMA(('IFC4X3_ADD2'));", "FILE_SCHEMA(('IFC4'));");
    let (b4, diag4) = with_cx(|cx| Lift::<Building>::lift(&read_ifc(&ifc4), cx).unwrap());
    assert!(by_gid(&b4, &gid(448)).is_some());
    assert!(by_gid(&b4, &gid(401)).is_none());
    assert!(by_gid(&b4, &gid(62)).is_none());
    assert!(diag4.iter().any(|d| d.code() == codes::IFC_ENTITY_DROPPED));
}

#[test]
fn respects_budgets_and_cancellation_on_infra_lift() {
    let file = read_ifc(&synthetic_infra_ifc(1.0, false));
    let (_, _, full_budget) = with_limits(Limits::default(), |cx| {
        Lift::<Building>::lift(&file, cx).unwrap()
    });
    for (kind, expected_code) in [
        (Kind::Elements, codes::BUDGET_ELEMENTS_EXCEEDED),
        (Kind::Work, codes::BUDGET_WORK_EXCEEDED),
        (Kind::DecodedBytes, codes::BUDGET_DECODED_BYTES_EXCEEDED),
    ] {
        let used = full_budget.used(kind);
        assert!(used > 1, "{kind:?} used={used}");
        let limits = Limits::default().with(kind, Limit::Finite(used - 1));
        let (res, diag, _) = with_limits(limits, |cx| Lift::<Building>::lift(&file, cx));
        assert!(matches!(res, Err(Error::Stopped(_))), "{kind:?}: {res:?}");
        let codes: Vec<Code> = diag.iter().map(|d| d.code()).collect();
        assert!(codes.contains(&expected_code), "{kind:?}: {codes:?}");
    }

    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    cancel.cancel();
    let res = Lift::<Building>::lift(&file, &mut Cx::new(&mut budget, &mut diag, &cancel));
    assert!(matches!(res, Err(Error::Stopped(_))));
}

/// 手元に公開サンプルを置いたときだけ走る外部サンプル照合（リポジトリには同梱しない）。
#[test]
#[allow(clippy::disallowed_methods)]
fn verifies_external_public_samples_when_present() {
    let Some(dir) = std::env::var_os("ZUMEN_IFC_INFRA_SAMPLES") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);

    for (file_name, expected) in [
        (
            "Infra-Road.ifc",
            &[
                ("IfcRoad", 5usize),
                ("IfcRoadPart", 26),
                ("IfcCourse", 16),
                ("IfcCourseType", 6),
                ("IfcEarthworksFill", 16),
            ][..],
        ),
        (
            "Infra-Rail.ifc",
            &[
                ("IfcRailway", 2),
                ("IfcRailwayPart", 2),
                ("IfcRail", 4),
                ("IfcRailType", 1),
                ("IfcTrackElement", 66),
                ("IfcTrackElementType", 1),
                ("IfcCourse", 2),
                ("IfcCourseType", 1),
            ][..],
        ),
        (
            "Infra-Bridge.ifc",
            &[
                ("IfcBridge", 3),
                ("IfcBridgePart", 18),
                ("IfcEarthworksFill", 4),
                ("IfcSign", 4),
                ("IfcSignType", 1),
            ][..],
        ),
        (
            "Infra-Landscaping.ifc",
            &[
                ("IfcGeographicElement", 76),
                ("IfcGeographicElementType", 20),
                ("IfcSign", 10),
                ("IfcSignType", 1),
            ][..],
        ),
    ] {
        let bytes = std::fs::read(dir.join(file_name)).unwrap();
        let (b, _) = with_cx(|cx| {
            let f = Ifc::read(&bytes, cx).unwrap();
            Lift::<Building>::lift(&f, cx).unwrap()
        });
        for &(class_name, count) in expected {
            let actual = b
                .elements()
                .iter()
                .enumerate()
                .filter(|(i, _)| {
                    b.components
                        .class
                        .get(ElementRef(*i as u32))
                        .is_some_and(|c| c.name == class_name)
                })
                .count();
            assert_eq!(actual, count, "{file_name} {class_name}");
        }
    }

    // linear-placement-of-signal.ifc: CartesianPosition あり・なしの双方で 2 信号の 3D 位置と軸が一致する
    let sig_src = std::fs::read_to_string(dir.join("linear-placement-of-signal.ifc")).unwrap();
    let sig_no_cp = sig_src
        .replace(
            "#3021 = IFCLINEARPLACEMENT(#2279, #3022, #6303);",
            "#3021 = IFCLINEARPLACEMENT(#2279, #3022, $);",
        )
        .replace(
            "#3032 = IFCLINEARPLACEMENT(#2279, #3033, #6307);",
            "#3032 = IFCLINEARPLACEMENT(#2279, #3033, $);",
        );
    let (b_with, _) = with_cx(|cx| Lift::<Building>::lift(&read_ifc(&sig_src), cx).unwrap());
    let (b_no_cp, _) = with_cx(|cx| Lift::<Building>::lift(&read_ifc(&sig_no_cp), cx).unwrap());

    let expected_signals = [
        (
            "0t_GtwGFT0nPszZsjyA9w4",
            [452600.861459492, 4539527.81761349, 7.5],
            [0.939398720423267, 0.342826550995001, 0.0],
            [0.0, 0.0, 1.0],
        ),
        (
            "1gnEPB6qvEY89wQ0JBMc5c",
            [453043.983513108, 4539754.92863765, 4.5],
            [0.900181207129064, 0.435515550045759, 0.0],
            [0.0, 0.0, 1.0],
        ),
    ];
    for b in [&b_with, &b_no_cp] {
        let railway = by_gid(b, "382GLQs9z2x9xXxKh7gZqf").unwrap();
        let sig_type = by_gid(b, "08tHBMWRHDiBAH7pFrGVfm").unwrap();
        assert_eq!(
            b.components
                .class
                .get(sig_type)
                .unwrap()
                .predefined
                .as_deref(),
            Some("VISUAL")
        );
        for (gid_str, exp_p, exp_x, exp_z) in expected_signals {
            let sig = by_gid(b, gid_str).unwrap();
            assert_eq!(
                b.components
                    .parent
                    .get(sig)
                    .unwrap()
                    .of
                    .as_ref()
                    .unwrap()
                    .target,
                railway
            );
            assert_eq!(
                b.components.placement.get(sig).unwrap().relative_to,
                Some(railway)
            );
            let wt = world_transform(b, sig).unwrap();
            assert!(
                near_vec([wt.0[0][3], wt.0[1][3], wt.0[2][3]], exp_p, 1e-6),
                "{gid_str} pos={wt:?}"
            );
            assert!(
                near_vec([wt.0[0][0], wt.0[1][0], wt.0[2][0]], exp_x, 1e-6),
                "{gid_str} x={wt:?}"
            );
            assert!(
                near_vec([wt.0[0][2], wt.0[1][2], wt.0[2][2]], exp_z, 1e-6),
                "{gid_str} z={wt:?}"
            );
        }
    }
}
