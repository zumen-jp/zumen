//! 汎用の render op を、SVG・PNG 描画へ接続する（決定 0275・0277）。

use zumen_base::Cx;

use crate::messages::{RenderFormat, RenderRequest, RenderResult, SceneResult};
use crate::session::{Failed, fail, invalid_value};

/// サイズ・色・範囲は、図面を変換する前に確かめる。
pub(crate) fn options(
    request: &RenderRequest,
    cx: &mut Cx<'_>,
) -> Result<zumen_render::Options, Failed> {
    let region = match request.region.as_slice() {
        &[a, b, c, d]
            if [a, b, c, d].iter().all(|v| v.is_finite())
                && (c - a).is_finite()
                && c > a
                && (d - b).is_finite()
                && d > b =>
        {
            [a, b, c, d]
        }
        _ => return Err(fail(cx, invalid_value("/region"))),
    };
    if !request.dpi.is_finite() || request.dpi <= 0.0 {
        return Err(fail(cx, invalid_value("/dpi")));
    }
    let size = |value, pointer, cx: &mut Cx<'_>| match u32::try_from(value) {
        Ok(value) if value > 0 => Ok(value),
        _ => Err(fail(cx, invalid_value(pointer))),
    };
    let width = size(request.width, "/width", cx)?;
    let height = size(request.height, "/height", cx)?;
    if request.format == RenderFormat::Png
        && u64::from(width) * u64::from(height) > zumen_render::MAX_PNG_PIXELS
    {
        let pointer = if u64::from(width) > zumen_render::MAX_PNG_PIXELS {
            "/width"
        } else {
            "/height"
        };
        return Err(fail(cx, invalid_value(pointer)));
    }
    Ok(zumen_render::Options {
        width,
        height,
        region,
        dpi: request.dpi,
        background: rgba(&request.background, "/background", cx)?,
        foreground: rgba(&request.foreground, "/foreground", cx)?,
    })
}

fn rgba(values: &[u64], pointer: &str, cx: &mut Cx<'_>) -> Result<[u8; 4], Failed> {
    if values.len() != 4 {
        return Err(fail(cx, invalid_value(pointer)));
    }
    let mut rgba = [0; 4];
    for (out, &value) in rgba.iter_mut().zip(values) {
        *out = u8::try_from(value).map_err(|_| fail(cx, invalid_value(pointer)))?;
    }
    Ok(rgba)
}

/// シーンと同じ Cx を使い、成功したときだけ全体の画像を返す。
pub(crate) fn render(
    request: &RenderRequest,
    options: &zumen_render::Options,
    scene: SceneResult,
    bytes: &[u8],
    cx: &mut Cx<'_>,
) -> Result<(RenderResult, Vec<u8>), Failed> {
    let output = match request.format {
        RenderFormat::Svg => zumen_render::svg(bytes, options, cx),
        RenderFormat::Png => zumen_render::png(bytes, options, cx),
    }
    .map_err(|_| Failed)?; // 描画部品が、停止を含む失敗の診断を返す。
    Ok((
        RenderResult {
            format: request.format,
            width: request.width,
            height: request.height,
            from: scene.from,
            view: scene.view,
            unit: scene.unit,
            tolerance: scene.tolerance,
            blob: 0,
        },
        output,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Session, Status};
    use zumen_base::{Budget, CancelToken, Diagnostics};
    use zumen_json::{FromJson, ToJson, Value, Writer};

    const REQUEST: &str = r#"{"v":1,"op":"render","doc":"a.dxf","from":"space/0","format":"svg","width":100,"height":100,"region":[0,0,100,100],"dpi":25.4,"background":[255,255,255,255],"foreground":[0,0,0,255],"tolerance":0.1}"#;
    const DRAWING: &[u8] = b"0\nSECTION\n2\nHEADER\n9\n$INSUNITS\n70\n4\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n0\nLINE\n5\n1\n8\n0\n420\n255\n10\n0\n20\n0\n11\n100\n21\n100\n0\nTEXT\n5\n2\n8\n0\n420\n65280\n10\n20\n20\n20\n40\n7\n1\nH\n0\nENDSEC\n0\nEOF\n";

    fn request(changes: &[(&str, &str)]) -> Value {
        let Value::Object(fields) = zumen_json::parse(REQUEST.as_bytes()).unwrap() else {
            panic!("要求がオブジェクトでない")
        };
        let mut fields: std::collections::BTreeMap<String, Value> = fields
            .iter()
            .map(|(key, value)| (key.into(), value.clone()))
            .collect();
        for &(key, json) in changes {
            fields.insert(key.into(), zumen_json::parse(json.as_bytes()).unwrap());
        }
        object(&fields)
    }

    fn object(fields: &std::collections::BTreeMap<String, Value>) -> Value {
        let mut out = Writer::new();
        out.begin_object();
        for (key, value) in fields {
            out.key(key);
            out.value(value);
        }
        out.end_object();
        zumen_json::parse(&out.into_bytes()).unwrap()
    }

    fn json(value: &impl ToJson) -> Vec<u8> {
        let mut out = Writer::new();
        value.to_json(&mut out);
        out.into_bytes()
    }

    fn scene_request(request: &Value) -> Value {
        let Value::Object(fields) = request else {
            panic!("要求がオブジェクトでない")
        };
        let mut fields: std::collections::BTreeMap<String, Value> = fields
            .iter()
            .filter(|(key, _)| {
                ![
                    "format",
                    "width",
                    "height",
                    "dpi",
                    "background",
                    "foreground",
                ]
                .contains(key)
            })
            .map(|(key, value)| (key.into(), value.clone()))
            .collect();
        fields.insert("op".into(), Value::String("scene".into()));
        object(&fields)
    }

    fn call(session: &mut Session, request: &impl ToJson) -> (Status, Value) {
        let reply = session.call(&json(request), &CancelToken::new());
        (reply.status, zumen_json::parse(&reply.response).unwrap())
    }

    fn session() -> Session {
        let mut session = Session::new(b"").unwrap();
        session.push(b"a.dxf", DRAWING).unwrap();
        let reply = session.call(
            br#"{"v":1,"op":"open","input":"a.dxf"}"#,
            &CancelToken::new(),
        );
        assert_eq!(reply.status, Status::Ok);
        session
    }

    fn text<'a>(response: &'a Value, pointer: &str) -> &'a str {
        match response.pointer(pointer).unwrap() {
            Value::String(value) => value,
            other => panic!("{pointer}: {other:?}"),
        }
    }

    #[test]
    fn renders_an_image_and_keeps_the_original_id_and_document() {
        let mut session = session();
        let req = request(&[]);
        let (status, response) = call(&mut session, &req);
        assert_eq!(status, Status::Ok, "{response:?}");
        assert_eq!(text(&response, "/result/format"), "svg");
        assert_eq!(text(&response, "/result/unit"), "millimeter");
        assert_eq!(text(&response, "/blobs/0/kind"), "image");
        assert_eq!(
            text(&response, "/diagnostics/0/code"),
            "render.text.dropped"
        );
        assert_eq!(text(&response, "/diagnostics/0/at/0/id"), "space/0/h2");
        let first = session.take(0).unwrap();
        let svg = std::str::from_utf8(&first).unwrap();
        assert!(svg.contains("M0 100L100 0") && svg.contains("#0000ff"));
        assert!(svg.contains("clip-path=\"url(#viewport)\""));
        let (status, repeated) = call(&mut session, &req);
        assert_eq!(status, Status::Ok);
        assert_eq!(response, repeated);
        assert_eq!(first, session.take(0).unwrap());
    }

    #[test]
    fn png_returns_an_image_and_keeps_ids_after_stopping() {
        let mut session = session();
        let req = request(&[("format", r#""png""#)]);
        let (status, response) = call(&mut session, &req);
        assert_eq!(status, Status::Ok);
        assert_eq!(text(&response, "/result/format"), "png");
        assert_eq!(text(&response, "/blobs/0/kind"), "image");
        assert_eq!(text(&response, "/diagnostics/0/at/0/id"), "space/0/h2");
        let image = session.take(0).unwrap();
        assert!(image.starts_with(b"\x89PNG\r\n\x1a\n"));
        for budget in [
            r#"{"work":20000}"#,
            r#"{"decoded_bytes":50000}"#,
            r#"{"segments":1}"#,
        ] {
            let (status, response) = call(
                &mut session,
                &request(&[("format", r#""png""#), ("budget", budget)]),
            );
            assert_eq!(status, Status::Failed, "{response:?}");
            assert!(text(&response, "/diagnostics/0/code").starts_with("budget."));
            assert!(response.pointer("/result").is_none());
            assert!(session.take(0).is_none());
        }
        let cancel = CancelToken::new();
        cancel.cancel();
        assert_eq!(session.call(&json(&req), &cancel).status, Status::Failed);
        assert!(session.take(0).is_none());
        for (width, height, pointer) in [("16777217", "1", "/width"), ("4096", "4097", "/height")] {
            let (status, response) = call(
                &mut session,
                &request(&[
                    ("format", r#""png""#),
                    ("width", width),
                    ("height", height),
                    ("doc", r#""missing""#),
                ]),
            );
            assert_eq!(status, Status::Failed);
            assert_eq!(text(&response, "/diagnostics/0/params/pointer"), pointer);
        }
        assert_eq!(call(&mut session, &req), (Status::Ok, response));
        assert_eq!(session.take(0).unwrap(), image);
    }

    #[test]
    fn matches_the_existing_scene_path_with_fonts_palette_and_heights() {
        let mut session = session();
        let fixture = zumen_json::parse(include_bytes!(
            "../../../bindings/conformance/cases/scene_fonts.json"
        ))
        .unwrap();
        let hex = text(&fixture, "/steps/1/push/hex");
        let bytes: Vec<_> = hex
            .as_bytes()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect();
        session.push(b"font.ttf", &bytes).unwrap();
        let indexed = std::str::from_utf8(DRAWING)
            .unwrap()
            .replace("420\n255\n", "62\n1\n");
        session.push(b"indexed.dxf", indexed.as_bytes()).unwrap();
        let opened = session.call(
            br#"{"v":1,"op":"open","input":"indexed.dxf"}"#,
            &CancelToken::new(),
        );
        assert_eq!(opened.status, Status::Ok);
        session
            .push(
                b"palette.json",
                br##"{"values":[{"rgb":"#123456","aci":1}]}"##,
            )
            .unwrap();
        for format in [r#""svg""#, r#""png""#] {
            let req = request(&[
                ("format", format),
                ("doc", r#""indexed.dxf""#),
                ("fonts", r#"[{"input":"font.ttf"}]"#),
                ("heights", r#"{"dxf.TEXT.40":"em"}"#),
                ("palette", r#""palette.json""#),
                ("print", "true"),
                ("width", "200"),
                ("background", "[10,20,30,128]"),
            ]);
            let (status, response) = call(&mut session, &req);
            assert_eq!(status, Status::Ok, "{response:?}");
            assert_eq!(text(&response, "/diagnostics/0/code"), "scene.font.assumed");
            assert!(response.pointer("/diagnostics/1").is_none());
            let image = session.take(0).unwrap();
            if format == r#""svg""# {
                assert!(std::str::from_utf8(&image).unwrap().contains("#123456"));
                assert!(
                    std::str::from_utf8(&image)
                        .unwrap()
                        .contains("fill-rule=\"nonzero\"")
                );
            }
            let (status, _) = call(&mut session, &scene_request(&req));
            assert_eq!(status, Status::Ok);
            let scene = session.take(0).unwrap();
            let request = RenderRequest::from_json(&req).unwrap();
            let mut budget = Budget::default();
            let mut diagnostics = Diagnostics::new();
            let cancel = CancelToken::new();
            let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
            let settings = options(&request, &mut cx).ok().unwrap();
            assert_eq!(
                image,
                (if format == r#""svg""# {
                    zumen_render::svg
                } else {
                    zumen_render::png
                })(&scene, &settings, &mut cx)
                .unwrap()
            );
        }
    }

    #[test]
    fn validates_settings_before_lifting_and_keeps_failures_atomic() {
        let mut session = session();
        for (field, value, pointer) in [
            ("width", "0", "/width"),
            ("height", "4294967296", "/height"),
            ("region", "[]", "/region"),
            ("region", "[0,0,0,1]", "/region"),
            ("region", "[-1e308,0,1e308,1]", "/region"),
            ("dpi", "0", "/dpi"),
            ("background", "[0,0,0]", "/background"),
            ("foreground", "[0,0,0,256]", "/foreground/3"),
            ("format", r#""webp""#, "/format"),
            ("tolerance", "0", "/tolerance"),
            ("view", "0", "/view"),
            ("from", r#""space/99""#, "/from"),
        ] {
            let (status, response) = call(&mut session, &request(&[(field, value)]));
            assert_eq!(status, Status::Failed, "{field}: {response:?}");
            assert_eq!(text(&response, "/diagnostics/0/code"), "call.value.invalid");
            assert_eq!(text(&response, "/diagnostics/0/params/pointer"), pointer);
            assert!(response.pointer("/result").is_none());
            assert_eq!(session.take(0), None);
        }
        let (status, response) = call(
            &mut session,
            &request(&[("doc", r#""missing""#), ("dpi", "0")]),
        );
        assert_eq!(status, Status::Failed);
        assert_eq!(text(&response, "/diagnostics/0/params/pointer"), "/dpi");
    }

    #[test]
    fn budgets_and_cancellation_return_no_image_and_do_not_close_the_document() {
        let mut session = session();
        for budget in [
            r#"{"work":0}"#,
            r#"{"decoded_bytes":1}"#,
            r#"{"input_bytes":1}"#,
            r#"{"segments":1}"#,
        ] {
            assert_eq!(call(&mut session, &request(&[])).0, Status::Ok);
            let (status, response) = call(&mut session, &request(&[("budget", budget)]));
            assert_eq!(status, Status::Failed, "{response:?}");
            let code = text(&response, "/diagnostics/0/code");
            assert!(code.starts_with("budget."), "{response:?}");
            assert!(response.pointer("/result").is_none());
            assert_eq!(session.take(0), None);
        }
        let cancel = CancelToken::new();
        cancel.cancel();
        let reply = session.call(&json(&request(&[])), &cancel);
        assert_eq!(reply.status, Status::Failed);
        let response = zumen_json::parse(&reply.response).unwrap();
        assert_eq!(
            text(&response, "/diagnostics/0/code"),
            "call.request.cancelled"
        );
        assert_eq!(session.take(0), None);
        assert_eq!(call(&mut session, &request(&[])).0, Status::Ok);
    }

    #[test]
    fn counts_scene_and_image_work_in_one_budget() {
        use zumen_base::budget::Kind;

        for format in [r#""svg""#, r#""png""#] {
            let mut session = session();
            // 同じシーンを作れる最小の作業量を、決まった回数以内で求める。
            let mut low = 0;
            let mut high = 4096;
            let limited = |work| {
                request(&[
                    ("format", format),
                    ("budget", &format!(r#"{{"work":{work}}}"#)),
                ])
            };
            assert_eq!(
                call(&mut session, &scene_request(&limited(high))).0,
                Status::Ok
            );
            while low < high {
                let middle = low + (high - low) / 2;
                let (status, response) = call(&mut session, &scene_request(&limited(middle)));
                if status == Status::Ok {
                    high = middle;
                } else {
                    assert_eq!(
                        text(&response, "/diagnostics/0/code"),
                        "budget.work.exceeded"
                    );
                    low = middle + 1;
                }
            }
            assert!(low > 0);
            assert_eq!(
                call(&mut session, &scene_request(&limited(low))).0,
                Status::Ok
            );
            let scene = session.take(0).unwrap();
            let mut budget = Budget::default();
            let mut diagnostics = Diagnostics::new();
            let cancel = CancelToken::new();
            let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
            let settings = options(&RenderRequest::from_json(&request(&[])).unwrap(), &mut cx)
                .ok()
                .unwrap();
            (if format == r#""svg""# {
                zumen_render::svg
            } else {
                zumen_render::png
            })(&scene, &settings, &mut cx)
            .unwrap();
            let image_work = budget.used(Kind::Work);
            assert!(image_work > 0);

            // 各段だけなら足りる量でも、合計には足りない。段の間で予算を戻すと通ってしまう。
            let (status, response) = call(&mut session, &limited(low.max(image_work)));
            assert_eq!(status, Status::Failed, "{response:?}");
            assert_eq!(
                text(
                    &response,
                    if format == r#""png""# {
                        "/diagnostics/1/code"
                    } else {
                        "/diagnostics/0/code"
                    }
                ),
                "budget.work.exceeded"
            );
            if format == r#""png""# {
                // 描画の診断まで終わり、PNG の符号化で合計の予算が尽きる。
                assert_eq!(
                    text(&response, "/diagnostics/0/code"),
                    "render.text.dropped"
                );
            }
            assert_eq!(session.take(0), None);
            assert_eq!(call(&mut session, &limited(low + image_work)).0, Status::Ok);
            assert!(session.take(0).is_some());
        }
    }
}
