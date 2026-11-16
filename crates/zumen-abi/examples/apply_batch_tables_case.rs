//! 同じ apply 内の表の参照、再読、失敗時の原子性を照らす。
#![cfg_attr(test, allow(dead_code))]
#![allow(clippy::unwrap_used, clippy::panic)] // 合成入力と適合テストの生成だけに使う。
use zumen_abi1::{Session, Status};
use zumen_base::CancelToken;
use zumen_json::{Value, Writer};

/// 各表の既定の定義と、線・文字を持つ最小の合成 DXF。
pub const DXF: &[u8] = b"0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1018\n9\n$INSUNITS\n70\n4\n9\n$HANDSEED\n5\n40\n0\nENDSEC\n\
    0\nSECTION\n2\nTABLES\n0\nTABLE\n2\nLTYPE\n0\nLTYPE\n5\n11\n2\nContinuous\n70\n0\n0\nENDTAB\n\
    0\nTABLE\n2\nLAYER\n0\nLAYER\n5\n10\n2\n0\n70\n0\n62\n7\n6\nContinuous\n0\nENDTAB\n\
    0\nTABLE\n2\nSTYLE\n0\nSTYLE\n5\n12\n2\nStandard\n70\n0\n40\n0\n41\n1\n50\n0\n71\n0\n42\n1\n3\nsynthetic\n4\n\n0\nENDTAB\n\
    0\nTABLE\n2\nDIMSTYLE\n0\nDIMSTYLE\n105\n13\n2\nStandard\n70\n0\n41\n2.5\n0\nENDTAB\n0\nENDSEC\n\
    0\nSECTION\n2\nENTITIES\n0\nLINE\n5\n20\n8\n0\n10\n0\n20\n0\n11\n1\n21\n1\n\
    0\nTEXT\n5\n21\n8\n0\n10\n0\n20\n0\n40\n2.5\n1\nOriginal\n7\nStandard\n0\nENDSEC\n0\nEOF\n";

fn fixture(case: &[u8], index: usize) -> Vec<u8> {
    let case = zumen_json::parse(case).unwrap();
    let push = case.pointer(&format!("/steps/{index}/push")).unwrap();
    if let Some(Value::String(text)) = push.pointer("/text") {
        return text.as_bytes().to_vec();
    }
    let Some(Value::String(hex)) = push.pointer("/hex") else {
        panic!("入力がない")
    };
    hex.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|b| u8::from_str_radix(std::str::from_utf8(b).unwrap(), 16).unwrap())
        .collect()
}

/// 未使用のレイヤ枠を持つ JWW と、用紙上に線を持つ SFC・P21。
pub fn other_inputs() -> Vec<(&'static str, Vec<u8>, &'static str)> {
    vec![
        (
            "JWW",
            fixture(
                include_bytes!("../../../bindings/conformance/cases/open_jww.json"),
                0,
            ),
            "space/0",
        ),
        (
            "SFC",
            fixture(
                include_bytes!("../../../bindings/conformance/cases/open_sxf.json"),
                0,
            ),
            "sheet/0",
        ),
        (
            "P21",
            fixture(
                include_bytes!("../../../bindings/conformance/cases/open_sxf.json"),
                2,
            ),
            "sheet/0",
        ),
    ]
}

fn json(value: &Value) -> String {
    let mut out = Writer::new();
    out.value(value);
    String::from_utf8(out.into_bytes()).unwrap()
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn call(session: &mut Session, request: &str) -> Result<Value, String> {
    let reply = session.call(request.as_bytes(), &CancelToken::new());
    if reply.status != Status::Ok {
        return Err(String::from_utf8_lossy(&reply.response).into_owned());
    }
    zumen_json::parse(&reply.response).map_err(|e| format!("{e:?}"))
}

/// 期待する参照先と再読後の全応答を確認し、環境間で同じ手順を流す。
pub fn steps(output: Option<&std::path::Path>) -> Result<Vec<String>, String> {
    let mut inputs = vec![
        ("DXF", DXF.to_vec(), "space/0"),
        (
            "DXF-keyless",
            String::from_utf8_lossy(DXF)
                .replace("5\n20\n", "")
                .replace("5\n21\n", "")
                .into_bytes(),
            "space/0",
        ),
        (
            "DWG-2000",
            include_bytes!("../../../samples/dwg/zumen-layout-r2000.dwg").to_vec(),
            "space/0",
        ),
        (
            "DWG-2004",
            include_bytes!("../../../samples/dwg/zumen-layout-r2004.dwg").to_vec(),
            "space/0",
        ),
        (
            "DWG-2018",
            include_bytes!("../../../samples/dwg/zumen-layout-r2018.dwg").to_vec(),
            "space/0",
        ),
    ];
    inputs.extend(other_inputs());
    let open = r#"{"v":1,"op":"open","input":"a"}"#;
    let query = r#"{"v":1,"op":"entities","doc":"a","fields":["geometry","hash","layer","style"]}"#;
    let tables = r#"{"v":1,"op":"tables","doc":"a"}"#;
    let mut steps = Vec::new();
    for (format, input, place) in inputs {
        let mut session = Session::new(b"").map_err(|e| format!("{e:?}"))?;
        session.push(b"a", &input).map_err(|e| format!("{e:?}"))?;
        call(&mut session, open)?;
        let before = call(&mut session, query)?;
        let first = before.pointer("/result/items/0").ok_or("要素がない")?;
        let acad = format.starts_with("DXF") || format.starts_with("DWG");
        let mut edits = Vec::new();
        if acad {
            edits.push(
                r#"{"kind":"add_line_type","name":"BatchType","pattern":[3,-1]}"#.to_string(),
            );
        }
        edits.push(format!(
            r#"{{"kind":"add_layer","name":"BatchLayer"{}}}"#,
            if acad {
                r#", "line_type":{"name":"BatchType"}"#
            } else {
                ""
            }
        ));
        edits.push(format!(
            r#"{{"kind":"restyle","id":{},"hash":{},"layer":"BatchLayer"{}}}"#,
            json(first.pointer("/id").unwrap()),
            json(first.pointer("/hash").unwrap()),
            if acad {
                r#", "line_type":{"name":"BatchType"}"#
            } else {
                ""
            }
        ));
        let style = if acad {
            r#""line_type":{"name":"BatchType"},"#
        } else if format == "JWW" {
            r#""color":{"jww":1},"line_type":{"name":"1"},"#
        } else {
            r#""color":{"sxf":1},"line_type":{"name":"continuous"},"line_weight":{"mm":0.13},"#
        };
        edits.push(format!(r#"{{"kind":"add","place":"{place}","layer":"BatchLayer",{style}"geometry":{{"shape":"line","start":[1,2],"end":[3,4]}}}}"#));
        if acad {
            edits.extend([
            r#"{"kind":"add_text_style","name":"BatchText","font":{"name":"synthetic","kind":"shape"}}"#.into(),
            r#"{"kind":"add_dimension_style","name":"BatchDim","size":3,"scale":2}"#.into(),
            r#"{"kind":"add","place":"space/0","geometry":{"shape":"text","position":[10,20],"content":"New","height":2.5,"width_factor":1,"rotation":0,"style":"BatchText"}}"#.into(),
            r#"{"kind":"add","place":"space/0","geometry":{"shape":"leader","path":[[0,0],[4,5]],"spline":false,"arrow":true,"style":"BatchDim"}}"#.into(),
        ]);
        }
        let apply = format!(
            r#"{{"v":1,"op":"apply","doc":"a","edits":[{}]}}"#,
            edits.join(",")
        );
        let applied = call(&mut session, &apply).map_err(|e| format!("{format}: {e}"))?;
        let mut written = session.take(0).ok_or("書き戻しがない")?;
        let mut replacement_steps = Vec::new();
        if acad {
            let target = applied
                .pointer("/result/applied/6")
                .ok_or("追加した文字がない")?;
            let replace = format!(
                r#"{{"v":1,"op":"apply","doc":"a","edits":[{{"kind":"add_text_style","name":"BatchReplacement","font":{{"name":"synthetic","kind":"shape"}}}},{{"kind":"replace","id":{},"hash":{},"geometry":{{"shape":"text","position":[10,20],"content":"New","height":2.5,"width_factor":1,"rotation":0,"style":"BatchReplacement"}}}}]}}"#,
                json(target.pointer("/id").unwrap()),
                json(target.pointer("/hash").unwrap())
            );
            call(&mut session, &replace)?;
            written = session.take(0).ok_or("置き換えの書き戻しがない")?;
            replacement_steps.extend([format!(r#"{{"call":{replace}}}"#), r#"{"take":0}"#.into()]);
        }
        let after = call(&mut session, query)?;
        if after.pointer("/result/items/0/layer") != Some(&Value::String("BatchLayer".into())) {
            return Err("新しいレイヤを指していない".into());
        }
        let after_tables = call(&mut session, tables)?;
        let mut reread = Session::new(b"").map_err(|e| format!("{e:?}"))?;
        reread.push(b"a", &written).map_err(|e| format!("{e:?}"))?;
        call(&mut reread, open)?;
        if call(&mut reread, query)? != after || call(&mut reread, tables)? != after_tables {
            return Err(format!("{format}: 再読が一致しない"));
        }
        if let Some(output) = output {
            std::fs::create_dir_all(output).map_err(|e| e.to_string())?;
            let extension = if format.starts_with("DXF") {
                "dxf"
            } else if acad {
                "dwg"
            } else if format == "JWW" {
                "jww"
            } else if format == "SFC" {
                "sfc"
            } else {
                "p21"
            };
            std::fs::write(output.join(format!("{format}.{extension}")), &written)
                .map_err(|e| e.to_string())?;
        }
        steps.extend([
            r#"{"session":{}}"#.into(),
            format!(r#"{{"push":{{"name":"a","hex":"{}"}}}}"#, hex(&input)),
            format!(r#"{{"call":{open}}}"#),
            format!(r#"{{"call":{query}}}"#),
            format!(r#"{{"call":{apply}}}"#),
            r#"{"take":0}"#.into(),
        ]);
        steps.extend(replacement_steps);
        steps.extend([
            format!(r#"{{"call":{query},"expect":{{"/result/items/0/layer":"BatchLayer"}}}}"#),
            format!(r#"{{"call":{tables}}}"#),
            r#"{"session":{}}"#.into(),
            format!(r#"{{"push":{{"name":"a","hex":"{}"}}}}"#, hex(&written)),
            format!(r#"{{"call":{open}}}"#),
            format!(r#"{{"call":{query},"expect":{{"/result/items/0/layer":"BatchLayer"}}}}"#),
            format!(r#"{{"call":{tables}}}"#),
        ]);
        if format == "DXF" {
            // 最後の検査まで進んだ停止も、表の追加を文書へ残さない。
            let mut low = 0;
            let mut high = 16_000_000u64;
            while low < high {
                let limit = low + (high - low) / 2;
                let mut trial = Session::new(b"").unwrap();
                trial.push(b"a", &input).unwrap();
                call(&mut trial, open)?;
                let request = apply.replacen(
                    r#""edits":"#,
                    &format!(r#""budget":{{"work":{limit}}},"edits":"#),
                    1,
                );
                if trial.call(request.as_bytes(), &CancelToken::new()).status == Status::Ok {
                    high = limit;
                } else {
                    low = limit + 1;
                }
            }
            let stopped = apply.replacen(
                r#""edits":"#,
                &format!(r#""budget":{{"work":{}}},"edits":"#, high - 1),
                1,
            );
            let forward = r#"{"v":1,"op":"apply","doc":"a","edits":[{"kind":"add_layer","name":"Ready"},{"kind":"add","place":"space/0","layer":"Ready","geometry":{"shape":"text","position":[0,0],"height":2,"rotation":0,"content":"x","style":"Future"}},{"kind":"add_text_style","name":"Future","font":{"name":"synthetic","kind":"shape"}}]}"#;
            steps.extend([r#"{"session":{}}"#.into(),format!(r#"{{"push":{{"name":"a","hex":"{}"}}}}"#,hex(&input)),format!(r#"{{"call":{open}}}"#),format!(r#"{{"call":{stopped},"status":"failed"}}"#),format!(r#"{{"call":{query},"expect":{{"/result/items/0/layer":"0"}}}}"#),format!(r#"{{"call":{tables}}}"#),format!(r#"{{"call":{forward},"status":"failed","expect":{{"/diagnostics/0/code":"call.value.invalid","/diagnostics/0/params/pointer":"/edits/1/geometry/style"}}}}"#),format!(r#"{{"call":{apply}}}"#),r#"{"take":0}"#.into()]);
        }
    }
    Ok(steps)
}

#[cfg(not(test))]
fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let first = args.next();
    let directory = if first.as_deref() == Some("--output") {
        Some(std::path::PathBuf::from(args.next().ok_or("出力先がない")?))
    } else {
        None
    };
    let result = format!(
        "{{\n  \"description\": \"同じ apply 内の表の後方参照・再読・停止と失敗の原子性。apply_batch_tables_case.rs から生成する\",\n  \"module\": \"drawing\",\n  \"steps\": [\n    {}\n  ]\n}}\n",
        steps(directory.as_deref())?.join(",\n    ")
    );
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../bindings/conformance/cases/apply_batch_tables.json");
    if first.as_deref() == Some("--check") {
        if std::fs::read_to_string(path)
            .map_err(|e| e.to_string())?
            .replace("\r\n", "\n")
            != result
        {
            return Err("表の参照の適合入力が古い".into());
        }
    } else {
        std::fs::write(path, result).map_err(|e| e.to_string())?;
    }
    Ok(())
}
