//! ST-Bridge を読み、建物モデルへ移す。合成のサンプルは `tests/fixtures` に、MIT の公開サンプルは
//! `samples/stbridge` にある（`docs/formats/stbridge.md`）。

use zumen_abi1::{Session, Status};
use zumen_base::{Budget, CancelToken, Cx, Diagnostic, Diagnostics, codes};
use zumen_building::{Building, ElementRef, Id, Local, Opening, ParentKind};
use zumen_sdk::{Codec, Lift, testkit};
use zumen_stbridge::Stb;

const FRAME: &[u8] = include_bytes!("fixtures/frame-2.1.stb");
const WALL: &[u8] = include_bytes!("fixtures/wall-2.0.stb");
// MIT の公開サンプル。著作権の表示と許諾の文は licenses/ にある（samples/ledger.toml）
const HOARYFOX: &[u8] = include_bytes!("../../../samples/stbridge/hoaryfox-sample-building.stb");
const MINI_A: &[u8] = include_bytes!("../../../samples/stbridge/stb-diffchecker-mini210-a.stb");
const MINI_B: &[u8] = include_bytes!("../../../samples/stbridge/stb-diffchecker-mini210-b.stb");

#[cfg(test)]
fn lift(src: &[u8]) -> (Building, Vec<Diagnostic>) {
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
    let building = Stb::read(src, &mut cx).unwrap().lift(&mut cx).unwrap();
    (building, diag.into_vec())
}

#[cfg(test)]
fn classes(building: &Building) -> Vec<(String, String)> {
    building
        .components
        .class
        .iter()
        .map(|(_, class)| (class.name.clone(), class.original.clone()))
        .collect()
}

#[cfg(test)]
fn named(building: &Building, original: &str, tag: &str) -> ElementRef {
    building
        .components
        .class
        .iter()
        .find(|(element, class)| {
            class.original == original
                && building
                    .components
                    .identity
                    .get(*element)
                    .and_then(|identity| identity.tag.as_deref())
                    == Some(tag)
        })
        .map(|(element, _)| element)
        .unwrap()
}

#[cfg(test)]
fn call(session: &mut Session, request: &str) -> (Status, String) {
    let reply = session.call(request.as_bytes(), &CancelToken::new());
    (reply.status, String::from_utf8(reply.response).unwrap())
}

#[test]
fn the_codec_keeps_the_sdk_contracts() {
    for src in [FRAME, WALL] {
        testkit::roundtrip::<Stb>(src).unwrap();
        testkit::deterministic::<Stb>(src).unwrap();
        testkit::stops_at_same_place::<Stb>(src).unwrap();
        for end in (0..src.len()).step_by(7) {
            testkit::survives::<Stb>(&src[..end]).unwrap();
        }
    }
    for src in [HOARYFOX, MINI_A, MINI_B] {
        testkit::roundtrip::<Stb>(src).unwrap();
        testkit::deterministic::<Stb>(src).unwrap();
    }
}

#[test]
fn lifts_the_hoaryfox_sample() {
    // HoaryFox が書いた 2.0.2 の鉄骨の架構。StbCommon に app_version がなく、XML Schema には合わない
    let (building, diagnostics) = lift(HOARYFOX);
    let count = |name: &str| {
        classes(&building)
            .iter()
            .filter(|(class, _)| class == name)
            .count()
    };
    assert_eq!(building.document.header.schemas, ["ST-Bridge 2.0.2"]);
    assert_eq!(count("IfcBuildingStorey"), 6);
    assert_eq!(count("IfcGridAxis"), 10);
    assert_eq!(count("IfcColumn"), 105);
    assert_eq!(count("IfcBeam"), 192);
    assert_eq!(count("IfcMember"), 10);
    let sets = &building
        .components
        .properties
        .get(named(&building, "StbColumn", "33"))
        .unwrap()
        .sets;
    assert_eq!(sets[1].name, "StbSecColumn_S");
    // 移さないのは、継手・計算の条件・解析のモデル・鋼材の規格の一覧と、断面の形を書く子の要素
    let found: Vec<String> = diagnostics.iter().map(|d| format!("{d:?}")).collect();
    assert_eq!(found.len(), 8, "{found:?}");
    assert!(
        diagnostics
            .iter()
            .all(|d| d.code() == codes::STBRIDGE_ELEMENT_UNSUPPORTED),
        "{found:?}"
    );
    for element in [
        "StbJoints",
        "StbCalData",
        "StbAnaModels",
        "StbSecSteel\"",
        "StbSecSteelFigureColumn_S",
        "StbSecSteelFigureBeam_S",
        "StbSecSteelFigureBrace_S",
        "StbSecFigureBeam_RC",
    ] {
        assert!(
            found.iter().any(|m| m.contains(element)),
            "{element}: {found:?}"
        );
    }
}

#[test]
fn lifts_the_diffchecker_pair() {
    // 節点 2 の X と梁の断面だけが違う 2.1.0 の組
    let (a, diagnostics_a) = lift(MINI_A);
    let (b, diagnostics_b) = lift(MINI_B);
    // 断面の形と配筋の子の要素を移さないことだけを知らせる
    for diagnostics in [&diagnostics_a, &diagnostics_b] {
        let found: Vec<String> = diagnostics.iter().map(|d| format!("{d:?}")).collect();
        assert_eq!(found.len(), 2, "{found:?}");
        assert!(
            found.iter().all(|m| m.contains("element.unsupported")),
            "{found:?}"
        );
        assert!(
            found.iter().any(|m| m.contains("StbSecFigureBeam_RC")),
            "{found:?}"
        );
        assert!(
            found
                .iter()
                .any(|m| m.contains("StbSecBarArrangementBeam_RC")),
            "{found:?}"
        );
    }
    assert_eq!(a.document.header.schemas, ["ST-Bridge 2.1.0"]);
    assert_eq!(classes(&a), classes(&b));
    let girder_a = named(&a, "StbGirder", "201");
    let girder_b = named(&b, "StbGirder", "201");
    // 違いは断面の寸法（移さない子の要素）と終わりの節点の位置で、どちらも建物モデルには残らない。
    // 梁は始めの節点を原点にし、材軸を全体の X に向ける
    assert_eq!(
        a.components.properties.get(girder_a),
        b.components.properties.get(girder_b)
    );
    for (building, girder) in [(&a, girder_a), (&b, girder_b)] {
        let Local::Transform(t) = &building.components.placement.get(girder).unwrap().local else {
            panic!("配置がない");
        };
        let axis = t.apply([0.0, 0.0, 1.0]);
        assert!(
            (axis[0] - 1.0).abs() < 1e-9 && axis[1].abs() < 1e-9,
            "{axis:?}"
        );
    }
}

#[test]
fn lifts_the_frame() {
    let (building, diagnostics) = lift(FRAME);
    let count = |name: &str| {
        classes(&building)
            .iter()
            .filter(|(class, _)| class == name)
            .count()
    };
    assert_eq!(count("IfcProject"), 1);
    assert_eq!(count("IfcBuilding"), 1);
    assert_eq!(count("IfcBuildingStorey"), 2);
    assert_eq!(count("IfcGrid"), 2);
    assert_eq!(count("IfcGridAxis"), 4);
    assert_eq!(count("IfcColumn"), 4);
    assert_eq!(count("IfcBeam"), 4);
    assert_eq!(count("IfcMember"), 1);
    assert_eq!(count("IfcSlab"), 1);
    assert_eq!(count("IfcWall"), 1);
    assert_eq!(count("IfcFooting"), 1);
    assert_eq!(count("IfcOpeningElement"), 1);
    assert_eq!(
        building.document.project_name.as_deref(),
        Some("試験の架構")
    );
    assert_eq!(building.document.header.schemas, ["ST-Bridge 2.1.0"]);

    // guid の UUID から GlobalId を作る。guid がなければ要素の番号で作る
    let c1 = named(&building, "StbColumn", "1");
    assert!(matches!(building.id(c1), Some(Id::Global(_))));
    let c2 = named(&building, "StbColumn", "2");
    assert!(matches!(building.id(c2), Some(Id::Instance(_))));

    // 柱は上の節点の階（RFL）に属し、材軸を局所の Z にして、rotate だけ回す
    let parent = building
        .components
        .parent
        .get(c2)
        .unwrap()
        .of
        .clone()
        .unwrap();
    assert_eq!(parent.kind, ParentKind::Contained);
    let rfl = building
        .components
        .identity
        .get(parent.target)
        .unwrap()
        .name
        .clone();
    assert_eq!(rfl.as_deref(), Some("RFL"));
    let Local::Transform(t) = &building.components.placement.get(c2).unwrap().local else {
        panic!("配置がない");
    };
    let top = t.apply([0.0, 0.0, 3500.0]);
    assert!((top[0] - 6000.0).abs() < 1e-9 && (top[2] - 3500.0).abs() < 1e-9);
    let x = t.apply([1.0, 0.0, 0.0]);
    assert!(
        (x[0] - 6000.0).abs() < 1e-9 && (x[1] - 1.0).abs() < 1e-9,
        "{x:?}"
    );

    // 部材と断面の属性を、要素の名前のセットに持つ
    let sets = &building.components.properties.get(c1).unwrap().sets;
    let names: Vec<&str> = sets.iter().map(|set| set.name.as_str()).collect();
    assert_eq!(names, ["StbColumn", "StbSecColumn_RC"]);
    assert!(
        sets[1]
            .properties
            .iter()
            .any(|p| p.name == "strength_concrete")
    );

    // 開口は壁に穴をあけ、壁の面に対する位置に置く
    let open = named(&building, "StbOpenArrangement", "1");
    let wall = named(&building, "StbWall", "1");
    assert_eq!(
        building.components.opening.get(open),
        Some(&Opening::Voids(wall))
    );
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let world = building
        .world_placement(open, &mut Cx::new(&mut budget, &mut diag, &cancel))
        .unwrap()
        .unwrap();
    // 壁は節点 4 (0, 5000, 0) から 3 (6000, 5000, 0) へ向かうので、面の X は全体の X
    let corner = world.apply([0.0, 0.0, 0.0]);
    assert!(
        (corner[0] - 2000.0).abs() < 1e-9 && (corner[1] - 5000.0).abs() < 1e-9,
        "{corner:?}"
    );

    // 節点 99 がない梁は、配置を持たない
    let g4 = named(&building, "StbGirder", "4");
    assert!(building.components.placement.get(g4).is_none());
    let found: Vec<&str> = diagnostics.iter().map(|d| d.code().name()).collect();
    assert!(
        found.contains(&codes::STBRIDGE_REFERENCE_MISSING.name()),
        "{found:?}"
    );
    assert!(
        found.contains(&codes::STBRIDGE_PLACEMENT_DROPPED.name()),
        "{found:?}"
    );
}

#[test]
fn lifts_shift_jis_and_the_2_0_openings() {
    let (building, diagnostics) = lift(WALL);
    assert_eq!(building.document.project_name.as_deref(), Some("壁の試験"));
    assert_eq!(building.document.header.schemas, ["ST-Bridge 2.0.2"]);
    let open = named(&building, "StbOpen", "1");
    let wall = named(&building, "StbWall", "1");
    assert_eq!(
        building.components.opening.get(open),
        Some(&Opening::Voids(wall))
    );
    assert_eq!(
        building
            .components
            .identity
            .get(open)
            .unwrap()
            .name
            .as_deref(),
        Some("扉")
    );
    let pile = named(&building, "StbPile", "1");
    let sets = &building.components.properties.get(pile).unwrap().sets;
    assert_eq!(sets[1].name, "StbSecPile_RC");
    // 階の節点の一覧がなければ、代表高さの近い階に属させる
    let column = named(&building, "StbColumn", "1");
    let story = building
        .components
        .parent
        .get(column)
        .unwrap()
        .of
        .clone()
        .unwrap();
    assert_eq!(
        building
            .components
            .identity
            .get(story.target)
            .unwrap()
            .name
            .as_deref(),
        Some("2FL")
    );
    // 拡張と、断面の形を書く子の要素は移さない
    let unsupported: Vec<String> = diagnostics
        .iter()
        .filter(|d| d.code() == codes::STBRIDGE_ELEMENT_UNSUPPORTED)
        .map(|d| format!("{d:?}"))
        .collect();
    assert_eq!(unsupported.len(), 5, "{unsupported:?}");
    for element in [
        "StbExtensions",
        "StbSecFigureColumn_RC",
        "StbSecFigureBeam_RC",
        "StbSecFigureWall_RC",
        "StbSecFigurePile_RC",
    ] {
        assert!(
            unsupported.iter().any(|m| m.contains(element)),
            "{element}: {unsupported:?}"
        );
    }
}

#[test]
fn rejects_other_versions_and_roots() {
    let (file, diagnostics) = testkit::read::<Stb>(
        br#"<?xml version="1.0"?><ST_BRIDGE version="1.4.00"><StbModel/></ST_BRIDGE>"#,
    );
    assert!(file.is_err());
    assert_eq!(diagnostics[0].code(), codes::STBRIDGE_VERSION_UNSUPPORTED);
    let (file, diagnostics) = testkit::read::<Stb>(br#"<ST_BRIDGE><StbModel/></ST_BRIDGE>"#);
    assert!(file.is_err());
    assert_eq!(diagnostics[0].code(), codes::STBRIDGE_FILE_INVALID);
    assert!(!Stb::SIGNATURES[0].matches(br#"<LandXML version="1.2"/>"#));
    assert!(Stb::SIGNATURES[0].matches(FRAME));
    assert!(Stb::SIGNATURES[0].matches(WALL));
}

#[cfg(test)]
fn check(session: &mut Session) {
    session.push(b"frame.stb", FRAME).unwrap();
    let (status, open) = call(session, r#"{"v": 1, "op": "open", "input": "frame.stb"}"#);
    assert_eq!(status, Status::Ok, "{open}");
    assert!(open.contains(r#""format":"stbridge""#), "{open}");
    assert!(open.contains("stbridge.reference.missing"), "{open}");
    let (status, summary) = call(session, r#"{"v": 1, "op": "building", "doc": "frame.stb"}"#);
    assert_eq!(status, Status::Ok, "{summary}");
    assert!(
        summary.contains(r#""schema":"ST-Bridge 2.1.0""#),
        "{summary}"
    );
    assert!(
        summary.contains(r#"{"name":"IfcColumn","count":4}"#),
        "{summary}"
    );
    let (status, elements) = call(
        session,
        r#"{"v": 1, "op": "building_elements", "doc": "frame.stb", "components": ["class", "properties", "placement"]}"#,
    );
    assert_eq!(status, Status::Ok, "{elements}");
    assert!(elements.contains("StbSecBeam_S"), "{elements}");
    let (_, capabilities) = call(session, r#"{"v": 1, "op": "capabilities"}"#);
    assert!(
        capabilities.contains(r#""id":"stbridge","family":"stbridge""#),
        "{capabilities}"
    );
}

#[test]
fn the_bim_session_opens_st_bridge() {
    let mut session = Session::new(b"").unwrap();
    check(&mut session);
}
