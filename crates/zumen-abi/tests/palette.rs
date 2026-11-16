//! 色表を選ぶ公開入口と、図面内 RGB・ID・非表示・再利用の契約。
#![cfg(feature = "drawing")]
#![allow(clippy::unwrap_used)] // 合成入力と、成功した応答の列を確かめるテスト。

use zumen_abi1::{Session, Status};
use zumen_base::{Budget, CancelToken, Cx, Diagnostics};
use zumen_drawing::binary;
use zumen_json::Value;
use zumen_scene::layout as l;

#[path = "../examples/palette_case.rs"]
mod fixture;

const PALETTE: &[u8] = br##"{"values":[{"aci":1,"jww":102,"rgb":"#ff0000"},{"sxf":4,"rgb":"#0000ff"},{"jww":1,"rgb":"#00c0c0"},{"aci":7,"rgb":"#ffffff"}]}"##;
const PROFILE: &[u8] = br##"{"format":"0.1","id":"base","edition":"1","revision":1,"vocabularies":{"color":{"values":[{"name":"red","rgb":"#ff0000"},{"name":"blue","rgb":"#0000ff"},{"name":"cyan","rgb":"#00c0c0"},{"name":"white","rgb":"#ffffff"},{"name":"file1","rgb":"#0c2238"},{"name":"file2","rgb":"#22384e"},{"name":"file3","rgb":"#384e5a"}]}},"rules":[{"id":"color","check":"color","level":"warning","vocabulary":"color"}]}"##;

fn call(session: &mut Session, request: &str) -> (Status, Value) {
    let reply = session.call(request.as_bytes(), &CancelToken::new());
    (reply.status, zumen_json::parse(&reply.response).unwrap())
}
fn session() -> Session {
    let (source, drawing) = fixture::fixture();
    let mut budget = Budget::default();
    let mut diagnostics = Diagnostics::new();
    let cancel = CancelToken::new();
    let bytes = binary::write(
        &source,
        &drawing,
        &mut Cx::new(&mut budget, &mut diagnostics, &cancel),
    )
    .unwrap();
    let mut session = Session::new(b"").unwrap();
    session.push(b"colors", &bytes).unwrap();
    session.push(b"palette.json", PALETTE).unwrap();
    session.push(b"profile.json", PROFILE).unwrap();
    assert_eq!(
        call(&mut session, r#"{"v":1,"op":"open","input":"colors"}"#).0,
        Status::Ok
    );
    session
}
fn request(op: &str, palette: Option<&str>, extra: &str) -> String {
    let palette = palette
        .map(|name| format!(r#", "palette":"{name}""#))
        .unwrap_or_default();
    let render = if op == "render" {
        r#", "width":100,"height":100,"region":[0,0,100,100],"dpi":25.4,"background":[255,255,255,255],"foreground":[0,0,0,255]"#
    } else {
        ""
    };
    format!(r#"{{"v":1,"op":"{op}","doc":"colors","from":"space/0"{palette}{render}{extra}}}"#)
}
fn section(bytes: &[u8], index: usize) -> &[u8] {
    let number =
        |at| u32::from_le_bytes(bytes.get(at..at + 4).unwrap().try_into().unwrap()) as usize;
    let offset = number(16 + index * 8);
    let len = number(20 + index * 8);
    bytes.get(offset..offset + len).unwrap()
}

#[test]
fn builtin_scene_matches_a_supplied_palette_and_preserves_file_rgb() {
    let mut session = session();
    let baseline = call(
        &mut session,
        r#"{"v":1,"op":"entities","doc":"colors","fields":["hash"]}"#,
    );
    let empty = call(&mut session, &request("scene", None, ""));
    assert_eq!(empty.0, Status::Ok, "{empty:?}");
    let unresolved = session.take(0).unwrap();
    let resolved = call(&mut session, &request("scene", Some("builtin:cad-v1"), ""));
    assert_eq!(resolved.0, Status::Ok, "{resolved:?}");
    let scene = session.take(0).unwrap();
    let rgba = section(&scene, l::COLOR_RGBA);
    for color in [
        [255, 0, 0, 255],
        [0, 0, 255, 255],
        [0, 192, 192, 255],
        [12, 34, 56, 255],
        [34, 56, 78, 255],
        [56, 78, 90, 255],
    ] {
        assert!(
            rgba.as_chunks::<4>().0.contains(&color),
            "{color:?}: {rgba:?}"
        );
    }
    for column in [
        l::ENTITY_IDS,
        l::UNIT_ENTITIES,
        l::UNIT_LAYERS,
        l::LAYER_IDS,
        l::COLOR_FILES,
        l::COLOR_CODES,
        l::COLOR_SYSTEMS,
    ] {
        assert_eq!(section(&unresolved, column), section(&scene, column));
    }
    let custom = call(&mut session, &request("scene", Some("palette.json"), ""));
    assert_eq!(custom, resolved);
    assert_eq!(session.take(0).unwrap(), scene);
    assert_eq!(call(&mut session, &request("scene", None, "")), empty);
    assert_eq!(session.take(0).unwrap(), unresolved);
    assert_eq!(
        call(
            &mut session,
            r#"{"v":1,"op":"entities","doc":"colors","fields":["hash"]}"#
        ),
        baseline
    );
}

#[test]
fn check_uses_all_three_color_systems_and_keeps_custom_colors_unresolved() {
    let mut session = session();
    let extra = r#", "profiles":["profile.json"],"profile":{"id":"base","edition":"1"}"#;
    let resolved = call(
        &mut session,
        &request("check", Some("builtin:cad-v1"), extra),
    );
    assert_eq!(resolved.0, Status::Ok, "{resolved:?}");
    assert_eq!(
        resolved
            .1
            .pointer("/result/rules/0/findings")
            .and_then(|value| zumen_json::number(value).ok()),
        Some(0.0)
    );
    assert_eq!(
        resolved
            .1
            .pointer("/result/rules/0/unsupported")
            .and_then(|value| zumen_json::number(value).ok()),
        Some(1.0)
    );
    assert_eq!(
        call(&mut session, &request("check", Some("palette.json"), extra)),
        resolved
    );
    let empty = call(&mut session, &request("check", None, extra));
    assert_eq!(empty.0, Status::Ok);
    assert_eq!(
        empty
            .1
            .pointer("/result/rules/0/unsupported")
            .and_then(|value| zumen_json::number(value).ok()),
        Some(6.0)
    );
}

#[test]
fn render_hides_public_layers_and_invalid_options_do_not_change_the_document() {
    let mut session = session();
    for format in ["svg", "png"] {
        let extra = format!(r#", "format":"{format}","hidden_layers":["layer/h21"]"#);
        let rendered = call(
            &mut session,
            &request("render", Some("builtin:cad-v1"), &extra),
        );
        assert_eq!(rendered.0, Status::Ok, "{rendered:?}");
        assert!(
            rendered.1.pointer("/diagnostics/0").is_none(),
            "{rendered:?}"
        );
        let image = session.take(0).unwrap();
        if format == "svg" {
            let svg = std::str::from_utf8(&image).unwrap();
            for color in [
                "#ff0000", "#0000ff", "#00c0c0", "#0c2238", "#22384e", "#384e5a", "#000000",
            ] {
                assert!(svg.contains(color), "{color}: {svg}");
            }
        }
        let invalid = format!(r#", "format":"{format}","hidden_layers":["layer/missing"]"#);
        let failed = call(
            &mut session,
            &request("render", Some("builtin:cad-v1"), &invalid),
        );
        assert_eq!(failed.0, Status::Failed);
        assert_eq!(
            failed.1.pointer("/diagnostics/0/code"),
            Some(&Value::String("render.options.invalid".into()))
        );
        assert!(session.take(0).is_none());
        assert_eq!(
            call(
                &mut session,
                &request("render", Some("builtin:cad-v1"), &extra)
            ),
            rendered
        );
        assert_eq!(session.take(0).unwrap(), image);
    }
    for op in ["scene", "render"] {
        let extra = if op == "render" {
            r#", "format":"svg""#
        } else {
            ""
        };
        let result = call(&mut session, &request(op, Some("builtin:unknown"), extra));
        assert_eq!(result.0, Status::Failed);
        assert_eq!(
            result.1.pointer("/diagnostics/0/params/pointer"),
            Some(&Value::String("/palette".into()))
        );
    }
}
