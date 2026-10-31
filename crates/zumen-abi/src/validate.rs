//! `validate` の op（決定 0216）。zm_push で渡した IFC を、zm_push で渡した EXPRESS に照らしてチェックし、指摘を返す。

use std::collections::BTreeMap;

use zumen_base::{Cx, name};
use zumen_drawing::Hash;
use zumen_ifc::{Express, Ifc, IfcZip};
use zumen_sdk::Codec;

use crate::messages::{ValidateRequest, ValidateResult};
use crate::session::{Failed, fail, input_missing, invalid_value, message_of, stopped};

/// limit を書かなかったときに返す指摘の数
const LIMIT: u64 = 1000;

/// 入力の IFC（IFC-SPF か ifcZIP）を読み、EXPRESS に照らしてチェックして、指摘の一部を返す
pub(crate) fn validate(
    inputs: &BTreeMap<String, Vec<u8>>,
    request: &ValidateRequest,
    cx: &mut Cx<'_>,
) -> Result<ValidateResult, Failed> {
    let input = |pointer: &str, value: &str, cx: &mut Cx<'_>| {
        if !name::is_safe(value) {
            return Err(fail(cx, invalid_value(pointer)));
        }
        inputs
            .get(value)
            .ok_or_else(|| fail(cx, input_missing(value)))
    };
    let ifc = input("/input", &request.input, cx)?;
    let express = input("/express", &request.express, cx)?;
    let express_hash = Hash::document_checked(express, cx).map_err(|stop| stopped(cx, stop))?;
    // 読めなければ、理由の診断は文脈に足してある
    let schema = Express::read(express, cx).map_err(|_| Failed)?;
    let (zipped, plain);
    let file = if IfcZip::SIGNATURES.iter().any(|s| s.matches(ifc)) {
        zipped = IfcZip::read(ifc, cx).map_err(|_| Failed)?;
        zipped.ifc()
    } else {
        plain = Ifc::read(ifc, cx).map_err(|_| Failed)?;
        &plain
    };
    let findings = zumen_ifc::check(file, &schema, cx).map_err(|_| Failed)?;
    let total = findings.len() as u64;
    let start = request.offset.unwrap_or(0);
    let end = start.saturating_add(request.limit.unwrap_or(LIMIT));
    Ok(ValidateResult {
        schema: schema.name().to_owned(),
        file_schema: String::from_utf8_lossy(file.schema()).into_owned(),
        express_hash: express_hash.to_string(),
        rules_not_evaluated: schema.rules(),
        findings: findings
            .iter()
            .skip(usize::try_from(start).unwrap_or(usize::MAX))
            .take(usize::try_from(end - start).unwrap_or(usize::MAX))
            .map(message_of)
            .collect(),
        total,
        next_offset: (end < total).then_some(end),
    })
}
