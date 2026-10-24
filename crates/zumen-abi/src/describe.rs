//! describe の op: 診断の文を、カタログから組み立てる（決定 0009）。
//!
//! 文は、Rust のこの 1 か所で組み立てる。どの言語の結合からも、同じ文になる。数の書き方は、カタログの規則に従い、
//! OS のロケールは使わない。

use std::collections::BTreeMap;

use zumen_base::{Cx, Diagnostic, Kind, codes, name};
use zumen_json::{FromJson, Writer};

use crate::messages::{self, Catalog, DescribeRequest, DescribeResult, NumberFormat, ParamValue};
use crate::session::{Failed, fail, input_missing, invalid_value, stopped};

/// カタログの形の大版
const CATALOG_VERSION: u64 = 1;

pub(crate) fn describe(
    inputs: &BTreeMap<String, Vec<u8>>,
    request: &DescribeRequest,
    cx: &mut Cx<'_>,
) -> Result<DescribeResult, Failed> {
    let catalog = match &request.catalog {
        Some(input) => Some(load(inputs, input, cx)?),
        None => None,
    };
    let mut texts = Vec::new();
    for diagnostic in &request.diagnostics {
        cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
        texts.push(text(catalog.as_ref(), diagnostic));
    }
    Ok(DescribeResult { texts })
}

/// カタログの入力を読む
fn load(
    inputs: &BTreeMap<String, Vec<u8>>,
    input: &str,
    cx: &mut Cx<'_>,
) -> Result<Catalog, Failed> {
    if !name::is_safe(input) {
        return Err(fail(cx, invalid_value("/catalog")));
    }
    let Some(bytes) = inputs.get(input) else {
        return Err(fail(cx, input_missing(input)));
    };
    cx.check(Kind::InputBytes, bytes.len() as u64)
        .map_err(|stop| stopped(cx, stop))?;
    zumen_json::parse(bytes)
        .ok()
        .and_then(|value| Catalog::from_json(&value).ok())
        .filter(|catalog| catalog.v == CATALOG_VERSION)
        .ok_or_else(|| {
            fail(
                cx,
                Diagnostic::new(codes::CATALOG_FILE_INVALID).with("name", input),
            )
        })
}

/// 1 つの診断の文。カタログがないか、カタログにない code なら、code と値をそのまま書く
fn text(catalog: Option<&Catalog>, diagnostic: &messages::Diagnostic) -> String {
    let template = catalog.and_then(|catalog| {
        let template = catalog.codes.get(&diagnostic.code)?;
        Some((template, &catalog.number))
    });
    match template {
        Some((template, number)) => fill(template, diagnostic, number),
        None => plain(diagnostic),
    }
}

/// ひな形の `{名前}` に値を、`{count}` にまとめた件数を入れる。値のない名前と、名前にならない `{` は、そのまま残す
fn fill(template: &str, diagnostic: &messages::Diagnostic, number: &NumberFormat) -> String {
    let mut out = String::new();
    let mut rest = template;
    while let Some((before, after)) = rest.split_once('{') {
        out.push_str(before);
        let filled = after.split_once('}').and_then(|(name, next)| {
            let value = if name == "count" {
                group(&diagnostic.count.to_string(), &number.group_separator)
            } else {
                param(diagnostic.params.get(name)?, number)
            };
            Some((value, next))
        });
        match filled {
            Some((value, next)) => {
                out.push_str(&value);
                rest = next;
            }
            None => {
                out.push('{');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// 値を文に入れる形にする。図面に記録された名前などの文字列は、そのまま入れる
fn param(value: &ParamValue, number: &NumberFormat) -> String {
    match value {
        ParamValue::Number(x) => decimal(*x, number),
        ParamValue::String(text) => text.clone(),
    }
}

/// 数を、読み直すと同じ値に戻る最短の表記にし、整数部を区切る。指数のある表記は区切らない
fn decimal(x: f64, number: &NumberFormat) -> String {
    let text = zumen_json::float_text(x).unwrap_or_default();
    if text.contains('e') {
        return text.replace('.', &number.decimal_separator);
    }
    let (sign, unsigned) = match text.strip_prefix('-') {
        Some(rest) => ("-", rest),
        None => ("", text.as_str()),
    };
    let (whole, fraction) = unsigned.split_once('.').unwrap_or((unsigned, ""));
    let mut out = format!("{sign}{}", group(whole, &number.group_separator));
    if !fraction.is_empty() {
        out.push_str(&number.decimal_separator);
        out.push_str(fraction);
    }
    out
}

/// 数字の並びを、右から 3 桁ごとに区切る
fn group(digits: &str, separator: &str) -> String {
    if separator.is_empty() {
        return digits.to_string();
    }
    let len = digits.chars().count();
    let mut out = String::new();
    for (index, c) in digits.chars().enumerate() {
        if index > 0 && (len - index).is_multiple_of(3) {
            out.push_str(separator);
        }
        out.push(c);
    }
    out
}

/// カタログを使わない文: `code (名前=値, …, count=件数)`。数は区切らず、文字列は JSON の文字列として書く
fn plain(diagnostic: &messages::Diagnostic) -> String {
    let mut items: Vec<String> = diagnostic
        .params
        .iter()
        .map(|(name, value)| {
            let value = match value {
                ParamValue::Number(x) => zumen_json::float_text(*x).unwrap_or_default(),
                ParamValue::String(text) => {
                    let mut out = Writer::new();
                    out.string(text);
                    String::from_utf8_lossy(&out.into_bytes()).into_owned()
                }
            };
            format!("{name}={value}")
        })
        .collect();
    if diagnostic.count > 1 {
        items.push(format!("count={}", diagnostic.count));
    }
    if items.is_empty() {
        diagnostic.code.clone()
    } else {
        format!("{} ({})", diagnostic.code, items.join(", "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zumen_base::{Budget, CancelToken, Diagnostics, ParamType};

    fn diagnostic(code: &str, count: u64, params: &[(&str, ParamValue)]) -> messages::Diagnostic {
        messages::Diagnostic {
            code: code.to_string(),
            severity: messages::Severity::Error,
            fidelity: None,
            count,
            at: Vec::new(),
            cite: None,
            params: params
                .iter()
                .map(|(name, value)| (name.to_string(), value.clone()))
                .collect(),
        }
    }

    fn number_format(group_separator: &str, decimal_separator: &str) -> NumberFormat {
        NumberFormat {
            group_separator: group_separator.to_string(),
            decimal_separator: decimal_separator.to_string(),
        }
    }

    fn run(
        inputs: &BTreeMap<String, Vec<u8>>,
        catalog: Option<&str>,
        diagnostics: Vec<messages::Diagnostic>,
    ) -> (Result<Vec<String>, ()>, Diagnostics) {
        let request = DescribeRequest {
            request_common: messages::RequestCommon {
                v: 1,
                budget: None,
                partial: None,
                max_places: None,
                time_limit_ms: None,
            },
            catalog: catalog.map(str::to_string),
            diagnostics,
        };
        let mut budget = Budget::default();
        let mut diag = Diagnostics::new();
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        let result = describe(inputs, &request, &mut cx)
            .map(|result| result.texts)
            .map_err(|_| ());
        (result, diag)
    }

    #[test]
    fn fills_templates_and_groups_numbers() {
        let format = number_format(",", ".");
        let d = diagnostic(
            "x.y.z",
            3,
            &[
                ("limit", ParamValue::Number(2_147_483_648.0)),
                ("ratio", ParamValue::Number(-12_345.25)),
                ("name", ParamValue::String("平面図".to_string())),
            ],
        );
        assert_eq!(
            fill("{limit}・{ratio}・{name}・{count} 件", &d, &format),
            "2,147,483,648・-12,345.25・平面図・3 件"
        );
        assert_eq!(fill("{missing} {Bad} {", &d, &format), "{missing} {Bad} {");
        assert_eq!(fill("{{count}}", &d, &format), "{3}");
        assert_eq!(decimal(1e21, &number_format(",", ",")), "1e+21");
        assert_eq!(decimal(1.5e-7, &number_format("", ",")), "1,5e-7");
        assert_eq!(decimal(123.0, &format), "123");
        assert_eq!(decimal(1234.5, &number_format("", ",")), "1234,5");
    }

    #[test]
    fn writes_code_and_values_without_a_catalog() {
        let d = diagnostic(
            "zip.path.unsafe",
            2,
            &[
                ("name", ParamValue::String("a\"b".to_string())),
                ("size", ParamValue::Number(1234.0)),
            ],
        );
        assert_eq!(
            plain(&d),
            r#"zip.path.unsafe (name="a\"b", size=1234, count=2)"#
        );
        assert_eq!(
            plain(&diagnostic("call.request.cancelled", 1, &[])),
            "call.request.cancelled"
        );
        let (texts, _) = run(&BTreeMap::new(), None, vec![d.clone()]);
        assert_eq!(texts.unwrap(), [plain(&d)]);
    }

    #[test]
    fn reports_missing_and_broken_catalogs() {
        let mut inputs = BTreeMap::new();
        let (result, diag) = run(&inputs, Some("ja.json"), Vec::new());
        assert!(result.is_err());
        assert_eq!(
            diag.iter().next().map(Diagnostic::code),
            Some(codes::CALL_INPUT_MISSING)
        );
        for broken in [&b"{"[..], br#"{"v":2,"lang":"ja","package":"0","number":{"group_separator":"","decimal_separator":"."},"codes":{}}"#, br#"{"v":1}"#] {
            inputs.insert("ja.json".to_string(), broken.to_vec());
            let (result, diag) = run(&inputs, Some("ja.json"), Vec::new());
            assert!(result.is_err());
            assert_eq!(diag.iter().next().map(Diagnostic::code), Some(codes::CATALOG_FILE_INVALID));
        }
    }

    /// 試験で使う値。型は台帳に合わせる
    fn sample(ty: ParamType) -> ParamValue {
        match ty {
            ParamType::UInt => ParamValue::Number(1234.0),
            ParamType::Int => ParamValue::Number(-5.0),
            ParamType::Float => ParamValue::Number(0.25),
            ParamType::Text | ParamType::Key => ParamValue::String("x".to_string()),
        }
    }

    #[test]
    fn fills_every_code_in_the_generated_catalogs() {
        for (lang, source) in [
            ("ja", include_str!("../../../catalog/ja.json")),
            ("en", include_str!("../../../catalog/en.json")),
        ] {
            let mut inputs = BTreeMap::new();
            inputs.insert(format!("{lang}.json"), source.as_bytes().to_vec());
            let diagnostics: Vec<messages::Diagnostic> = codes::ALL
                .iter()
                .map(|code| {
                    let params: Vec<(&str, ParamValue)> = code
                        .params()
                        .iter()
                        .map(|param| (param.name(), sample(param.ty())))
                        .collect();
                    diagnostic(code.name(), 2, &params)
                })
                .collect();
            let (texts, diag) = run(&inputs, Some(&format!("{lang}.json")), diagnostics);
            assert!(diag.is_empty(), "{diag:?}");
            let texts = texts.unwrap();
            assert_eq!(texts.len(), codes::ALL.len());
            for (code, text) in codes::ALL.iter().zip(&texts) {
                assert!(!text.contains(['{', '}']), "{lang} {}: {text}", code.name());
                assert!(
                    !text.starts_with(code.name()),
                    "{lang} {}: {text}",
                    code.name()
                );
            }
        }
    }
}
