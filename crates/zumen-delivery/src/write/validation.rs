//! 版を選んだプロファイルのうち、管理 XML の変更に当たる内容規則を比べる。

use super::{Cx, Document, Edit, Error, Kind, Node, name, resolve, stopped};
use zumen_json::{Object, Value};

/// 変更後の要素のバイト範囲と、規則ごとの結果。
#[derive(Debug)]
pub struct EditCheck {
    /// 要求の edits での位置。
    pub edit: u64,
    /// 適用した規則の ID。対応する規則がなければ None。
    pub rule: Option<String>,
    /// matched・violated・unchecked のいずれか。
    pub status: &'static str,
    /// 文字、子の数、または確認できなかった理由。
    pub reason: &'static str,
    /// 出力 XML の要素の始まり。
    pub start: u64,
    /// 出力 XML の要素の終わり。
    pub end: u64,
}

fn text(value: Option<&Value>) -> Option<&str> {
    match value? {
        Value::String(text) => Some(text),
        _ => None,
    }
}

fn get<'a>(object: &'a Object, wanted: &str, cx: &mut Cx<'_>) -> Result<Option<&'a Value>, Error> {
    for (key, value) in object.iter() {
        cx.check(Kind::Work, 1 + wanted.len().min(key.len()) as u64)
            .map_err(|s| stopped(cx, s))?;
        if key == wanted {
            return Ok(Some(value));
        }
    }
    Ok(None)
}

/// XML 1.0 §2.10 の要素内容に許される空白、§3.2 の子の出現回数を別々に照らす。
/// DTD 自体は解釈せず、対象版の宣言から呼び出し側が選んだ規則だけを使う。
pub(super) fn check(
    doc: &Document,
    edits: &[Edit<'_>],
    rules: &[(String, Value)],
    cx: &mut Cx<'_>,
) -> Result<Vec<EditCheck>, Error> {
    let declaration = doc.doctype();
    let length = declaration.as_ref().map_or(0, |r| r.len() as u64);
    cx.check(Kind::Work, length).map_err(|s| stopped(cx, s))?;
    cx.check(Kind::DecodedBytes, length.saturating_mul(6))
        .map_err(|s| stopped(cx, s))?;
    let doctype = declaration.and_then(|r| crate::system_id(&doc.cdata(r)));
    let mut checks = Vec::new();
    for (index, edit) in edits.iter().enumerate() {
        let index = index as u64;
        let path = match edit {
            Edit::SetText { path, .. } | Edit::Append { path, .. } => path,
        };
        let number = resolve(doc, path, index, cx)?;
        let node = doc.element(number as usize).ok_or(Error::Rejected)?;
        let element = name(doc, node, cx)?;
        let mut child_count = 0u64;
        if let Edit::Append { child, .. } = edit {
            for member in node.children() {
                cx.check(Kind::Work, 1).map_err(|s| stopped(cx, s))?;
                if let Node::Element(i) = member
                    && let Some(member) = doc.element(*i)
                    && name(doc, member, cx)? == child.name
                {
                    child_count += 1;
                }
            }
        }
        let mut add = |rule: Option<&str>, status, reason, cx: &mut Cx<'_>| -> Result<(), Error> {
            let size = rule.map_or(0, str::len) as u64;
            cx.check(Kind::DecodedBytes, size.saturating_add(128))
                .map_err(|s| stopped(cx, s))?;
            checks.push(EditCheck {
                edit: index,
                rule: rule.map(str::to_owned),
                status,
                reason,
                start: node.span().start as u64,
                end: node.span().end as u64,
            });
            Ok(())
        };
        let mut found = false;
        let mut missing = "no_rule";
        for (id, value) in rules {
            cx.check(Kind::Work, 1 + length + element.len() as u64)
                .map_err(|s| stopped(cx, s))?;
            let Value::Object(rule) = value else { continue };
            if text(get(rule, "check", cx)?) != Some("delivery-edit-content")
                || text(get(rule, "element", cx)?) != Some(&element)
            {
                continue;
            }
            if doctype.is_none() || doctype.as_deref() != text(get(rule, "doctype", cx)?) {
                missing = "doctype_mismatch";
                continue;
            }
            found = true;
            let (matches, reason) = match edit {
                Edit::SetText { value, .. } => {
                    cx.check(Kind::Work, value.len() as u64)
                        .map_err(|s| stopped(cx, s))?;
                    let matches = match text(get(rule, "text", cx)?) {
                        Some("any") => Some(true),
                        Some("empty") => Some(value.is_empty()),
                        Some("whitespace") => Some(
                            value
                                .bytes()
                                .all(|b| matches!(b, b' ' | b'\n' | b'\r' | b'\t')),
                        ),
                        _ => None,
                    };
                    (matches, "text")
                }
                Edit::Append { child, .. } => {
                    let matches = match get(rule, "children", cx)? {
                        Some(Value::Object(limits)) => match get(limits, &child.name, cx)? {
                            None => Some(false),
                            Some(Value::Number(n)) => {
                                cx.check(Kind::Work, n.as_str().len() as u64)
                                    .map_err(|s| stopped(cx, s))?;
                                n.to_safe_integer()
                                    .and_then(|n| u64::try_from(n).ok())
                                    .map(|max| child_count <= max)
                            }
                            other if text(other) == Some("unbounded") => Some(true),
                            _ => None,
                        },
                        other if text(other) == Some("any") => Some(true),
                        _ => None,
                    };
                    (matches, "child_count")
                }
            };
            let status = match matches {
                Some(true) => "matched",
                Some(false) => "violated",
                None => "unchecked",
            };
            add(
                Some(id),
                status,
                if matches.is_some() {
                    reason
                } else {
                    "unsupported_rule"
                },
                cx,
            )?;
        }
        if !found {
            add(None, "unchecked", missing, cx)?;
        }
    }
    Ok(checks)
}
