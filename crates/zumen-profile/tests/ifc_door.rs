//! IFC の建具の直接属性と型から、作図プロファイルの記号までを合成 IFC で照らす。
//!
//! zumen-ifc の dev-dependencies から zumen-profile の feature を有効にすると、cargo metadata が
//! workspace 全体で feature をまとめ、配る SBOM に実ビルドにない辺が載る。そのため、この試験は
//! zumen-profile の側に置き、zumen-ifc は既定の feature のまま使う。
#![cfg(feature = "building-drawing")]
use zumen_base::{Budget, CancelToken, Cx, Diagnostics};
use zumen_building::drawing::Role;
use zumen_crs::ProjectPoint;
use zumen_drawing::Shape;
use zumen_ifc::Ifc;
use zumen_sdk::Codec;

#[test]
fn explicit_door_attributes_reach_profile_symbols() {
    use zumen_profile::building_drawing::DrawingProfile;
    const BUILDING_DRAWING: &[u8] =
        include_bytes!("../../../profiles/zumen-building-drawing/2026-11/profile.json");
    for schema in ["IFC2X3", "IFC4", "IFC4X3_ADD2"] {
        let old = schema == "IFC2X3";
        let door_tail = if old {
            ""
        } else {
            ",.DOOR.,.SINGLE_SWING_RIGHT.,$"
        };
        let type_row = if old {
            "IFCDOORSTYLE('0000000000000000000011',$,$,$,$,$,$,$,.SINGLE_SWING_LEFT.,.NOTDEFINED.,.F.,.F.)"
        } else {
            "IFCDOORTYPE('0000000000000000000011',$,$,$,$,$,$,$,$,.DOOR.,.SINGLE_SWING_LEFT.,.F.,$)"
        };
        let bytes = format!("ISO-10303-21;HEADER;FILE_DESCRIPTION((''),'2;1');FILE_NAME('','',(''),(''),'','','');FILE_SCHEMA(('{schema}'));ENDSEC;DATA;
            #1=IFCPROJECT('0000000000000000000001',$,$,$,$,$,$,$,#2);
            #2=IFCUNITASSIGNMENT((#3));#3=IFCSIUNIT(*,.LENGTHUNIT.,.MILLI.,.METRE.);
            #10=IFCDOOR('0000000000000000000010',$,$,$,$,$,$,$,2100.,900.{door_tail});
            #11={type_row};#12=IFCRELDEFINESBYTYPE('0000000000000000000012',$,$,$,(#10),#11);
            ENDSEC;END-ISO-10303-21;");
        let (mut budget, mut diagnostics, cancel) =
            (Budget::default(), Diagnostics::new(), CancelToken::new());
        let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
        let file = Ifc::read(bytes.as_bytes(), &mut cx).unwrap();
        let building = file.lift_geometry(&mut cx).unwrap();
        let p = DrawingProfile::load(
            &[("base", BUILDING_DRAWING)],
            "zumen-building-drawing",
            "2026-11",
            &mut cx,
        )
        .unwrap();
        let view = p
            .default_plan(
                &building,
                ProjectPoint {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
                &mut cx,
            )
            .unwrap();
        let out = p.cut(&building, view, 0.01, 1.0, None, &mut cx).unwrap();
        assert!(!out.output.lines.is_empty());
        assert!(out.output.lines.iter().all(|l| l.role == Role::Annotation));
        // 型の左ヒンジを使い、occurrence にある右ヒンジで上書きしない。
        assert!(out.output.drawing.spaces[0].elements.iter().any(|e| matches!(e.shape, Shape::Line { start, end } if start.x == 0.0 && end.x == 0.0 && end.y == 900.0)));
        assert!(
            out.output
                .lines
                .iter()
                .all(|l| out.output.sources[l.source].instance == Some(10))
        );
    }
}
