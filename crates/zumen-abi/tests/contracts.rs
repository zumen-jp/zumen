//! 登録した図面の形式で、テストキットの予算と ID の契約試験を流す（#398、決定 0010・0156・0017）。
//! 入力は、公開サンプルの `samples/dwg` と、適合試験の組（`bindings/conformance/cases`）の合成のもの。
//! ID の記録は `tests/ids/` に 1 行に 1 つ置く。ID を変えるには、先に決定の記録が要る。

use zumen_acad::{Dwg, Dxf};
use zumen_base::{Budget, CancelToken, Cx, Diagnostics};
use zumen_drawing::{Drawing2D, id};
use zumen_json::Value;
use zumen_jww::Jww;
use zumen_sdk::{Codec, Lift, testkit};
use zumen_sxf::Sxf;

const DXF: &[u8] = include_bytes!("../../../samples/dwg/zumen-layout-r2000_ascii.dxf");
const DWG: &[u8] = include_bytes!("../../../samples/dwg/zumen-layout-r2000.dwg");
const OPEN_SXF: &str = include_str!("../../../bindings/conformance/cases/open_sxf.json");
const OPEN_JWW: &str = include_str!("../../../bindings/conformance/cases/open_jww.json");

/// 適合試験の組の、`name` を入れる手順の中身
#[cfg(test)]
fn pushed(case: &str, name: &str) -> Vec<u8> {
    let case = zumen_json::parse(case.as_bytes()).unwrap();
    let Some(Value::Array(steps)) = case.pointer("/steps") else {
        panic!("steps がない");
    };
    let push = steps
        .iter()
        .find(|step| step.pointer("/push/name") == Some(&Value::String(name.to_string())))
        .unwrap();
    match (push.pointer("/push/text"), push.pointer("/push/hex")) {
        (Some(Value::String(text)), _) => text.as_bytes().to_vec(),
        (_, Some(Value::String(hex))) => hex
            .as_bytes()
            .chunks(2)
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect(),
        _ => panic!("{name} の中身がない"),
    }
}

/// 読んで図面モデルにし、要素の ID を振る順に並べる
#[cfg(test)]
fn ids<C: Codec>(src: &[u8]) -> Result<Vec<String>, String>
where
    C::File: Lift<Drawing2D>,
{
    let file = testkit::read::<C>(src)
        .0
        .map_err(|e| format!("読めない: {e:?}"))?;
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let drawing = file
        .lift(&mut Cx::new(&mut budget, &mut diag, &cancel))
        .map_err(|e| format!("図面モデルにできない: {e:?}"))?;
    let mut out = Vec::new();
    id::each_element(&drawing, |_, id, _| {
        out.push(id);
        Ok::<(), ()>(())
    })
    .map_err(|()| "ID を振れない".to_string())?;
    Ok(out)
}

#[cfg(test)]
fn check<C: Codec>(src: &[u8], recorded: &str)
where
    C::File: Lift<Drawing2D>,
{
    testkit::stops_at_same_place::<C>(src).unwrap();
    testkit::ids_match(src, recorded, ids::<C>).unwrap();
}

#[test]
fn dxf_keeps_its_budget_and_ids() {
    check::<Dxf>(DXF, include_str!("ids/dxf.txt"));
}

#[test]
fn dwg_keeps_its_budget_and_ids() {
    check::<Dwg>(DWG, include_str!("ids/dwg.txt"));
}

#[test]
fn sxf_keeps_its_budget_and_ids() {
    check::<Sxf>(&pushed(OPEN_SXF, "a.sfc"), include_str!("ids/sxf.txt"));
}

#[test]
fn jww_keeps_its_budget_and_ids() {
    check::<Jww>(&pushed(OPEN_JWW, "a.jww"), include_str!("ids/jww.txt"));
}

/// 途中で切った入力も、fuzz の道具と同じく、小さい予算の中で止まり、2 回読んで同じになる
#[cfg(test)]
fn survives_cuts<C: Codec>(src: &[u8]) {
    let step = (src.len() / 64).max(1);
    for end in (0..src.len()).step_by(step) {
        let cut = src.get(..end).unwrap();
        testkit::survives::<C>(cut).unwrap_or_else(|message| panic!("{end} バイト: {message}"));
    }
}

#[test]
fn cut_inputs_survive() {
    survives_cuts::<Dxf>(DXF);
    survives_cuts::<Dwg>(DWG);
    survives_cuts::<Sxf>(&pushed(OPEN_SXF, "a.sfc"));
    survives_cuts::<Jww>(&pushed(OPEN_JWW, "a.jww"));
}
