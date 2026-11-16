//! 合成 DXF・DWG・JWW・SFC・P21 で、一覧の定義名から配置し、書き戻しを再読する。
#![cfg_attr(test, allow(dead_code))]

#[path = "../../zumen-jww/tests/common/mod.rs"]
mod jww;
use zumen_abi1::{Session, Status};
use zumen_base::CancelToken;
use zumen_json::{ToJson, Value, Writer};

fn sxf(body: &str, sfc: bool) -> Vec<u8> {
    format!("ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION(('SCADEC level2 {}'),'2;1');\nFILE_NAME('synthetic','2000-01-01T00:00:00',(''),(''),'zumen$$3.1','','');\nFILE_SCHEMA(('ASSOCIATIVE_DRAUGHTING'));\nENDSEC;\nDATA;\n{body}ENDSEC;\nEND-ISO-10303-21;\n", if sfc { "feature_mode" } else { "AP202" }).into_bytes()
}

/// 4 形式・5 符号化の自作入力と、配置する空間・レイヤ。
pub fn inputs() -> Vec<(&'static str, Vec<u8>, &'static str, &'static str)> {
    let dxf = b"0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1015\n9\n$INSUNITS\n70\n4\n0\nENDSEC\n\
        0\nSECTION\n2\nBLOCKS\n0\nBLOCK\n5\n20\n8\n0\n2\nDOOR\n70\n2\n10\n0\n20\n0\n3\nDOOR\n\
        0\nLINE\n5\n21\n8\n0\n10\n0\n20\n0\n11\n1\n21\n0\n\
        0\nATTDEF\n5\n23\n8\n0\n10\n0.5\n20\n1\n40\n0.25\n1\nD1\n3\nNumber\n2\nNO\n70\n0\n\
        0\nENDBLK\n5\n22\n8\n0\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n0\nENDSEC\n0\nEOF\n"
        .to_vec();
    let mut spec = jww::Spec::new(700);
    spec.groups[0] = (3, 100.0, [3; 16]);
    spec.layer_names.push((0, "L"));
    let mut archive = jww::Archive::new(700);
    archive.layer = 0;
    archive.group = 0;
    archive.pen_width = 0;
    archive.count(1);
    archive.line();
    archive.count(1);
    archive.block("未配置");
    archive.out.extend(0u32.to_le_bytes());
    let mut jww = spec.build();
    jww.extend(archive.out);
    let mut sfc = String::new();
    for (index, value) in [
        r"layer_feature(\'L\','1')",
        "line_feature('1','1','1','1','0','0','1','0')",
        r"sfig_org_feature(\'Part\','4')",
        r"drawing_sheet_feature(\'S\','3','1','420','297')",
    ]
    .into_iter()
    .enumerate()
    {
        sfc.push_str(&format!("/*SXF\n#{} = {value}\nSXF*/\n", (index + 1) * 10));
    }
    let p21 = concat!(
        "#1=DRAUGHTING_PRE_DEFINED_COLOUR('black');\n#2=DRAUGHTING_PRE_DEFINED_CURVE_FONT('continuous');\n",
        "#3=(LENGTH_UNIT()NAMED_UNIT(*)SI_UNIT(.MILLI.,.METRE.));\n#4=LENGTH_MEASURE_WITH_UNIT(POSITIVE_LENGTH_MEASURE(0.13),#3);\n",
        "#5=CURVE_STYLE(' ',#2,#4,#1);\n#6=PRESENTATION_STYLE_ASSIGNMENT((#5));\n",
        "#7=DIRECTION(' ',(1.,0.));\n#8=CARTESIAN_POINT(' ',(0.,0.));\n#9=CARTESIAN_POINT(' ',(2.,3.));\n",
        "#10=AXIS2_PLACEMENT_2D(' ',#8,#7);\n#11=GEOMETRIC_REPRESENTATION_CONTEXT(' ',' ',2);\n",
        "#12=POLYLINE(' ',(#8,#9));\n#13=ANNOTATION_CURVE_OCCURRENCE(' ',(#6),#12);\n",
        "#14=DRAUGHTING_SUBFIGURE_REPRESENTATION('$$SXF_P_Part',(#13,#10),#11);\n",
        "#15=PRESENTATION_LAYER_ASSIGNMENT('L',' ',(#13));\n#16=DRAWING_SHEET_REVISION('A3_horizontal',(),$,'01');\n"
    );
    vec![
        ("DXF", dxf, "space/0", "0"),
        (
            "DWG",
            include_bytes!("../../zumen-acad/tests/fixtures/table-r2018.dwg").to_vec(),
            "space/0",
            "0",
        ),
        ("JWW", jww, "space/0", "L"),
        ("SFC", sxf(&sfc, true), "sheet/0", "L"),
        ("P21", sxf(p21, false), "sheet/0", "L"),
    ]
}

/// 成功した応答のバイト列。失敗時には合成入力の診断を返す。
pub fn call(session: &mut Session, request: &str) -> Result<Vec<u8>, String> {
    let reply = session.call(request.as_bytes(), &CancelToken::new());
    if reply.status != Status::Ok {
        return Err(String::from_utf8_lossy(&reply.response).into_owned());
    }
    Ok(reply.response)
}
fn text<'a>(value: &'a Value, path: &str) -> Result<&'a str, String> {
    match value.pointer(path) {
        Some(Value::String(value)) => Ok(value),
        _ => Err(format!("文字列がない: {path}: {value:?}")),
    }
}
fn json(value: &str) -> String {
    let mut out = Writer::new();
    Value::String(value.into()).to_json(&mut out);
    String::from_utf8_lossy(&out.into_bytes()).into_owned()
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// 定義名は tables の応答から選び、配置後の形と再読後の応答を照らす。
pub fn steps() -> Result<Vec<String>, String> {
    let mut steps = Vec::new();
    for (format, input, place, layer) in inputs() {
        let mut session = Session::new(b"").map_err(|e| format!("{e:?}"))?;
        session.push(b"a", &input).map_err(|e| format!("{e:?}"))?;
        let open = r#"{"v":1,"op":"open","input":"a"}"#;
        let tables = r#"{"v":1,"op":"tables","doc":"a"}"#;
        let query = r#"{"v":1,"op":"entities","doc":"a","fields":["geometry","hash","style"]}"#;
        let result = (|| -> Result<(), String> {
            call(&mut session, open)?;
            let before = call(&mut session, tables)?;
            let before = zumen_json::parse(&before).map_err(|e| format!("{e:?}"))?;
            let name = text(&before, "/result/symbols/0/name")?;
            let apply = format!(
                r#"{{"v":1,"op":"apply","doc":"a","edits":[{{"kind":"add","place":"{place}","layer":"{layer}","geometry":{{"shape":"symbol","name":{},"position":[10,20],"scale":[2,2],"rotation":30}}}}]}}"#,
                json(name)
            );
            let applied = call(&mut session, &apply)?;
            let written = session.take(0).ok_or("書き戻しがない")?;
            let applied = zumen_json::parse(&applied).map_err(|e| format!("{e:?}"))?;
            let id = text(&applied, "/result/applied/0/id")?;
            let one = format!(
                r#"{{"v":1,"op":"entities","doc":"a","ids":[{}],"fields":["geometry"]}}"#,
                json(id)
            );
            let shape = call(&mut session, &one)?;
            let shape = zumen_json::parse(&shape).map_err(|e| format!("{e:?}"))?;
            if text(&shape, "/result/items/0/geometry/name")? != name {
                return Err("定義の名前が変わった".into());
            }
            for (path, expected) in [("position", "[10,20]"), ("scale", "[2,2]")] {
                if shape.pointer(&format!("/result/items/0/geometry/{path}"))
                    != Some(&zumen_json::parse(expected.as_bytes()).map_err(|e| format!("{e:?}"))?)
                {
                    return Err(format!("配置の {path} が合わない: {shape:?}"));
                }
            }
            let angle = match shape.pointer("/result/items/0/geometry/rotation") {
                Some(Value::Number(value)) => {
                    value.as_str().parse::<f64>().map_err(|e| e.to_string())?
                }
                _ => return Err("回転がない".into()),
            };
            if (angle - 30.0).abs() > 1e-12 {
                return Err("回転が合わない".into());
            }
            let after = call(&mut session, query)?;
            let after_tables = call(&mut session, tables)?;
            let mut reread = Session::new(b"").map_err(|e| format!("{e:?}"))?;
            reread.push(b"a", &written).map_err(|e| format!("{e:?}"))?;
            call(&mut reread, open)?;
            if call(&mut reread, query)? != after || call(&mut reread, tables)? != after_tables {
                return Err("編集後と再読後の応答が違う".into());
            }
            steps.extend([r#"{"session":{}}"#.into(), format!(r#"{{"push":{{"name":"a","hex":"{}"}}}}"#, hex(&input)),
                format!(r#"{{"call":{open}}}"#), format!(r#"{{"call":{tables}}}"#), format!(r#"{{"call":{apply}}}"#), r#"{"take":0}"#.into(),
                format!(r#"{{"call":{one},"expect":{{"/result/items/0/geometry/name":{},"/result/items/0/geometry/position":[10,20],"/result/items/0/geometry/scale":[2,2]}}}}"#,json(name)),
                format!(r#"{{"call":{query}}}"#),format!(r#"{{"call":{tables}}}"#), r#"{"session":{}}"#.into(),
                format!(r#"{{"push":{{"name":"a","hex":"{}"}}}}"#, hex(&written)),format!(r#"{{"call":{open}}}"#),format!(r#"{{"call":{query}}}"#),format!(r#"{{"call":{tables}}}"#)]);
            Ok(())
        })();
        result.map_err(|error| format!("{format}: {error}"))?;
    }
    Ok(steps)
}

#[cfg(not(test))]
fn main() -> Result<(), String> {
    let result = format!(
        "{{\n  \"description\": \"4 形式・5 符号化で記号定義の一覧から配置し、再読する。drawing_metadata_formats.rs から生成する\",\n  \"module\": \"drawing\",\n  \"steps\": [\n    {}\n  ]\n}}\n",
        steps()?.join(",\n    ")
    );
    let output = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../bindings/conformance/cases/drawing_metadata_formats.json");
    if std::env::args().nth(1).as_deref() == Some("--check") {
        if std::fs::read_to_string(output)
            .map_err(|e| e.to_string())?
            .replace("\r\n", "\n")
            != result
        {
            return Err("記号定義の適合入力が古い".into());
        }
    } else {
        std::fs::write(output, result).map_err(|e| e.to_string())?;
    }
    Ok(())
}
