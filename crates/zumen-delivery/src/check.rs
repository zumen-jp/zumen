//! 成果品をプロファイルの規則と照らす（決定 0241、#384）。規則の値は、要領の表から照合に要る値だけを持つ。
//!
//! 照合の種類は次のとおり。知らない種類と読めない規則は、`profile.rule.unsupported` を返して照合しない
//! （決定 0179）。
//!
//! - `delivery-name`: フォルダーとファイルの名前の文字・長さ・深さ
//! - `delivery-required`: 要るファイルの有無
//! - `delivery-link`: 管理ファイルが指すファイルの有無
//! - `delivery-unreferenced`: どの管理ファイルも指さないファイル
//! - `delivery-standard`: 管理ファイルの要素の値（適用要領基準など）
//! - `delivery-characters`: 管理ファイルの値の文字（JIS X 0201 のラテン文字と JIS X 0208）
//! - `delivery-doctype`: 管理ファイルの DTD の名前と版
//! - `delivery-photo-date`: 写真の管理ファイルの撮影年月日と、写真の Exif の撮影の日（#388）
//! - `delivery-photo-pixels`: 写真の画素数（#388）

use std::collections::BTreeSet;

use zumen_base::{Cx, Diagnostic, Kind, Ref, Severity, Stop, codes};
use zumen_json::{Object, Value};
use zumen_profile::{Finding, RuleSummary};

use crate::{Managed, Package, Record};

/// 名前の文字・長さ・深さ
#[derive(Clone, Debug, Default)]
struct Name {
    /// 使える文字。空なら比べない
    characters: String,
    /// 拡張子を除いたファイルの名前の文字の数の上限
    stem: Option<usize>,
    /// 拡張子の文字の数の上限
    extension: Option<usize>,
    /// フォルダーの名前の文字の数の上限
    folder: Option<usize>,
    /// 名前の部分の数の上限（ファイルの名前を含む）
    depth: Option<usize>,
    /// ファイルの名前の始まり（写真の `P` など）
    prefix: Option<String>,
}

#[derive(Clone, Debug)]
enum Plan {
    Name(Name),
    Required {
        names: Vec<String>,
    },
    Link {
        /// 照らす要素の名前。空ならすべて
        elements: Vec<String>,
    },
    Unreferenced,
    Standard {
        element: String,
        values: Vec<String>,
    },
    Characters {
        alphanumerics: bool,
    },
    Doctype {
        values: Vec<String>,
    },
    PhotoDate {
        /// 写真 1 枚の記録の要素の名前
        entry: String,
        /// その中の、写真のファイルの名前の要素の名前
        file: String,
        /// その中の、撮影の日（`YYYY-MM-DD`）の要素の名前
        date: String,
    },
    PhotoPixels {
        min: Option<usize>,
        max: Option<usize>,
    },
}

/// 読んだ規則
#[derive(Clone, Debug)]
struct Rule {
    id: String,
    check: String,
    level: Severity,
    /// 照らすフォルダーか管理ファイルの名前の頭。空ならすべて
    folders: Vec<String>,
    /// 照らさない名前の頭
    except: Vec<String>,
    /// この名前の頭のファイルがあるときだけ照らす
    when: Option<String>,
    plan: Plan,
}

/// 照合の結果
#[derive(Clone, Debug, Default)]
pub struct Report {
    /// 規則ごとの数
    pub rules: Vec<RuleSummary>,
    /// 指摘
    pub findings: Vec<Finding>,
}

fn text(value: Option<&Value>) -> Option<&str> {
    match value? {
        Value::String(text) => Some(text),
        _ => None,
    }
}

/// 文字の列の欄。書かなければ空
fn texts(value: Option<&Value>) -> Option<Vec<String>> {
    match value {
        None => Some(Vec::new()),
        Some(Value::Array(values)) => values
            .iter()
            .map(|value| text(Some(value)).map(str::to_string))
            .collect(),
        Some(_) => None,
    }
}

/// 数の上限の欄。書かなければ `Some(None)`
fn limit(value: Option<&Value>) -> Option<Option<usize>> {
    match value {
        None => Some(None),
        Some(Value::Number(n)) => Some(Some(usize::try_from(n.to_safe_integer()?).ok()?)),
        Some(_) => None,
    }
}

fn plan(check: &str, object: &Object) -> Option<Plan> {
    Some(match check {
        "delivery-name" => Plan::Name(Name {
            characters: text(object.get("characters"))
                .unwrap_or_default()
                .to_string(),
            stem: limit(object.get("stem"))?,
            extension: limit(object.get("extension"))?,
            folder: limit(object.get("folder"))?,
            depth: limit(object.get("depth"))?,
            prefix: match object.get("prefix") {
                None => None,
                some => Some(text(some)?.to_string()),
            },
        }),
        "delivery-required" => Plan::Required {
            names: texts(object.get("names")).filter(|names| !names.is_empty())?,
        },
        "delivery-link" => Plan::Link {
            elements: texts(object.get("elements"))?,
        },
        "delivery-unreferenced" => Plan::Unreferenced,
        "delivery-standard" => Plan::Standard {
            element: text(object.get("element"))?.to_string(),
            values: texts(object.get("values")).filter(|values| !values.is_empty())?,
        },
        "delivery-characters" => Plan::Characters {
            alphanumerics: match object.get("fullwidth-alphanumerics") {
                None => true,
                Some(Value::Bool(flag)) => *flag,
                Some(_) => return None,
            },
        },
        "delivery-doctype" => Plan::Doctype {
            values: texts(object.get("values")).filter(|values| !values.is_empty())?,
        },
        "delivery-photo-date" => Plan::PhotoDate {
            entry: text(object.get("entry"))?.to_string(),
            file: text(object.get("file"))?.to_string(),
            date: text(object.get("date"))?.to_string(),
        },
        "delivery-photo-pixels" => Plan::PhotoPixels {
            min: limit(object.get("min"))?,
            max: limit(object.get("max"))?,
        },
        _ => return None,
    })
}

fn rule(id: &str, value: &Value) -> Option<Rule> {
    let Value::Object(object) = value else {
        return None;
    };
    let check = text(object.get("check"))?;
    let level = match text(object.get("level"))? {
        "error" => Severity::Error,
        "warning" => Severity::Warning,
        "info" => Severity::Info,
        _ => return None,
    };
    let when = match object.get("when") {
        None => None,
        some => Some(text(some)?.to_string()),
    };
    Some(Rule {
        id: id.to_string(),
        check: check.to_string(),
        level,
        folders: texts(object.get("folders"))?,
        except: texts(object.get("except"))?,
        when,
        plan: plan(check, object)?,
    })
}

/// 照合の種類が電子納品のものか
fn is_delivery(value: &Value) -> bool {
    match value {
        Value::Object(object) => {
            text(object.get("check")).is_some_and(|check| check.starts_with("delivery-"))
        }
        _ => false,
    }
}

/// 解いた規則から、電子納品の規則を読む。使えない規則は `profile.rule.unsupported` を返して使わない。ほかの
/// 種類の規則（図面の照合など）は黙って飛ばす
fn rules(values: &[(String, Value)], cx: &mut Cx<'_>) -> Vec<Rule> {
    let mut found = Vec::new();
    for (id, value) in values {
        let check = match value {
            Value::Object(object) => text(object.get("check")).unwrap_or_default(),
            _ => "",
        };
        if !is_delivery(value) && !check.is_empty() {
            continue;
        }
        match rule(id, value) {
            Some(rule) => found.push(rule),
            None => cx.report(
                Diagnostic::new(codes::PROFILE_RULE_UNSUPPORTED)
                    .with("rule", id.as_str())
                    .with("check", check),
            ),
        }
    }
    found
}

impl Rule {
    /// 名前が規則の範囲にあるか
    fn covers(&self, name: &str) -> bool {
        (self.folders.is_empty() || self.folders.iter().any(|head| name.starts_with(head)))
            && !self.except.iter().any(|head| name.starts_with(head))
    }

    fn finding(&self, diagnostic: Diagnostic) -> Finding {
        Finding {
            rule: self.id.clone(),
            level: self.level,
            diagnostic: diagnostic.with("rule", self.id.as_str()),
        }
    }
}

/// 名前の場所
fn at(name: &str) -> Ref {
    Ref::default().with_id(name)
}

/// 名前の部分ごとに、外れた理由を返す
fn name_reason(name: &str, plan: &Name) -> Option<&'static str> {
    let parts: Vec<&str> = name.split('/').collect();
    if plan.depth.is_some_and(|depth| parts.len() > depth) {
        return Some("深さ");
    }
    let count = |part: &str| part.chars().count();
    let (folders, file) = parts.split_at(parts.len().saturating_sub(1));
    let characters = |part: &str| {
        plan.characters.is_empty() || part.chars().all(|c| plan.characters.contains(c))
    };
    for folder in folders {
        if !characters(folder) {
            return Some("文字");
        }
        if plan.folder.is_some_and(|max| count(folder) > max) {
            return Some("フォルダーの名前の長さ");
        }
    }
    let file = file.first().copied().unwrap_or_default();
    if plan
        .prefix
        .as_ref()
        .is_some_and(|prefix| !file.starts_with(prefix.as_str()))
    {
        return Some("名前の始まり");
    }
    let (stem, extension) = file.rsplit_once('.').unwrap_or((file, ""));
    if !characters(stem) || !characters(extension) {
        return Some("文字");
    }
    if plan.stem.is_some_and(|max| count(stem) > max) {
        return Some("名前の長さ");
    }
    if plan.extension.is_some_and(|max| count(extension) > max) {
        return Some("拡張子の長さ");
    }
    None
}

/// 管理ファイルの記録を、親から順にたどる
fn walk<'a>(record: &'a Record, out: &mut Vec<&'a Record>) {
    out.push(record);
    for child in &record.children {
        walk(child, out);
    }
}

fn records(managed: &Managed) -> Vec<&Record> {
    let mut out = Vec::new();
    walk(&managed.root, &mut out);
    out
}

/// 記録の下（自分を除く）で、名前が `name` の最初の記録
fn below<'a>(record: &'a Record, name: &str) -> Option<&'a Record> {
    let mut out = Vec::new();
    walk(record, &mut out);
    out.into_iter().skip(1).find(|found| found.name == name)
}

/// 照合できなかったことを表す指摘か。数を分け、水準を info にする
fn is_unsupported(diagnostic: &Diagnostic) -> bool {
    [
        codes::PROFILE_DELIVERY_STANDARD_UNSUPPORTED,
        codes::PROFILE_DELIVERY_PHOTO_DATE_UNSUPPORTED,
        codes::PROFILE_DELIVERY_PHOTO_PIXELS_UNSUPPORTED,
    ]
    .contains(&diagnostic.code())
}

/// 文字が、JIS X 0201 のラテン文字（空白と制御の文字を含む）か JIS X 0208 の文字か。`alphanumerics` が偽なら、
/// JIS X 0208 の数字とラテン文字（全角の英数字）を外にする
fn is_allowed(c: char, alphanumerics: bool) -> bool {
    if c.is_ascii() {
        return true;
    }
    if !alphanumerics
        && ('０'..='９')
            .chain('Ａ'..='Ｚ')
            .chain('ａ'..='ｚ')
            .any(|x| x == c)
    {
        return false;
    }
    let mut buffer = [0_u8; 4];
    let (bytes, _, errors) = encoding_rs::SHIFT_JIS.encode(c.encode_utf8(&mut buffer));
    let [lead, trail] = bytes.as_ref() else {
        // 1 バイトの非 ASCII は、JIS X 0201 の片仮名か、ASCII と重なる記号
        return false;
    };
    if errors {
        return false;
    }
    // Shift_JIS の 2 バイトを、JIS X 0208 の区に直す。1〜8 区と 16〜84 区だけが JIS X 0208 の文字で、13 区
    // （NEC の特殊文字）と 89 区より後（IBM の拡張）は外
    let row = match *lead {
        0x81..=0x9F => u32::from(*lead - 0x81) * 2 + 1,
        0xE0..=0xEF => u32::from(*lead - 0xC1) * 2 + 1,
        _ => return false,
    } + u32::from(*trail >= 0x9F);
    (1..=8).contains(&row) || (16..=84).contains(&row)
}

/// 成果品を、解いた規則の値（`zumen_profile::rules` の値）と照らす。予算か中断の合図で止まれば、診断を足さずに
/// 止まった理由を返す
pub fn check(
    package: &Package,
    values: &[(String, Value)],
    cx: &mut Cx<'_>,
) -> Result<Report, Stop> {
    let mut report = Report::default();
    // どれかの管理ファイルが指すファイルの名前
    let referenced: BTreeSet<&str> = package
        .managed
        .iter()
        .flat_map(records)
        .filter_map(|record| record.file.as_ref())
        .filter(|link| link.found)
        .map(|link| link.name.as_str())
        .collect();
    let managed_names: BTreeSet<&str> = package
        .managed
        .iter()
        .map(|managed| managed.name.as_str())
        .collect();
    for rule in rules(values, cx) {
        let mut findings = Vec::new();
        let active = rule.when.as_ref().is_none_or(|head| {
            package
                .files
                .iter()
                .any(|file| file.name.starts_with(head.as_str()))
        });
        let targets = || {
            package
                .managed
                .iter()
                .filter(|managed| rule.covers(&managed.name))
        };
        match &rule.plan {
            _ if !active => {}
            Plan::Name(plan) => {
                for file in package.files.iter().filter(|file| rule.covers(&file.name)) {
                    cx.check(Kind::Work, 1)?;
                    if let Some(reason) = name_reason(&file.name, plan) {
                        findings.push(
                            Diagnostic::new(codes::PROFILE_DELIVERY_NAME_INVALID)
                                .with("name", file.name.as_str())
                                .with("reason", reason)
                                .at(at(&file.name)),
                        );
                    }
                }
            }
            Plan::Required { names } => {
                for name in names {
                    if package.file(name).is_none() {
                        findings.push(
                            Diagnostic::new(codes::PROFILE_DELIVERY_REQUIRED_INVALID)
                                .with("name", name.as_str()),
                        );
                    }
                }
            }
            Plan::Link { elements } => {
                for managed in targets() {
                    for record in records(managed) {
                        cx.check(Kind::Work, 1)?;
                        if !elements.is_empty() && !elements.contains(&record.name) {
                            continue;
                        }
                        let Some(link) = record.file.as_ref().filter(|link| !link.found) else {
                            continue;
                        };
                        findings.push(
                            Diagnostic::new(codes::PROFILE_DELIVERY_LINK_INVALID)
                                .with("managed", managed.name.as_str())
                                .with("name", link.name.as_str())
                                .at(at(&managed.name))
                                .at(record.source.clone()),
                        );
                    }
                }
            }
            Plan::Unreferenced => {
                for file in package.files.iter().filter(|file| rule.covers(&file.name)) {
                    cx.check(Kind::Work, 1)?;
                    let name = file.name.as_str();
                    if !referenced.contains(name) && !managed_names.contains(name) {
                        findings.push(
                            Diagnostic::new(codes::PROFILE_DELIVERY_UNREFERENCED_INVALID)
                                .with("name", name)
                                .at(at(name)),
                        );
                    }
                }
            }
            Plan::Standard { element, values } => {
                for managed in targets() {
                    let found: Vec<&Record> = records(managed)
                        .into_iter()
                        .filter(|record| record.name == *element)
                        .collect();
                    if found.is_empty() {
                        findings.push(
                            Diagnostic::new(codes::PROFILE_DELIVERY_STANDARD_UNSUPPORTED)
                                .with("managed", managed.name.as_str())
                                .with("element", element.as_str())
                                .at(at(&managed.name)),
                        );
                    }
                    for record in found {
                        let value = record.value.as_deref().unwrap_or_default();
                        if !values.iter().any(|allowed| allowed == value) {
                            findings.push(
                                Diagnostic::new(codes::PROFILE_DELIVERY_STANDARD_INVALID)
                                    .with("managed", managed.name.as_str())
                                    .with("element", element.as_str())
                                    .with("found", value)
                                    .at(at(&managed.name))
                                    .at(record.source.clone()),
                            );
                        }
                    }
                }
            }
            Plan::Characters { alphanumerics } => {
                for managed in targets() {
                    for record in records(managed) {
                        cx.check(Kind::Work, 1)?;
                        let value = record.value.as_deref().unwrap_or_default();
                        let Some(c) = value.chars().find(|c| !is_allowed(*c, *alphanumerics))
                        else {
                            continue;
                        };
                        findings.push(
                            Diagnostic::new(codes::PROFILE_DELIVERY_CHARACTERS_INVALID)
                                .with("managed", managed.name.as_str())
                                .with("element", record.name.as_str())
                                .with("character", c.to_string())
                                .at(at(&managed.name))
                                .at(record.source.clone()),
                        );
                    }
                }
            }
            Plan::Doctype { values } => {
                for managed in targets() {
                    let found = managed.doctype.as_deref().unwrap_or_default();
                    if !values.iter().any(|allowed| allowed == found) {
                        findings.push(
                            Diagnostic::new(codes::PROFILE_DELIVERY_DOCTYPE_INVALID)
                                .with("managed", managed.name.as_str())
                                .with("found", found)
                                .at(at(&managed.name)),
                        );
                    }
                }
            }
            Plan::PhotoDate { entry, file, date } => {
                for managed in targets() {
                    for record in records(managed).into_iter().filter(|r| r.name == *entry) {
                        cx.check(Kind::Work, 1)?;
                        // ファイルが見つからないことは、delivery-link が指摘する
                        let Some(link) = below(record, file)
                            .and_then(|found| found.file.as_ref())
                            .filter(|link| link.found)
                        else {
                            continue;
                        };
                        let written = below(record, date).and_then(|found| found.value.as_deref());
                        let taken = package
                            .file(&link.name)
                            .and_then(|found| found.picture.as_ref())
                            .and_then(|picture| picture.taken.as_deref())
                            .and_then(|taken| taken.get(..10));
                        let (Some(written), Some(taken)) = (written, taken) else {
                            findings.push(
                                Diagnostic::new(codes::PROFILE_DELIVERY_PHOTO_DATE_UNSUPPORTED)
                                    .with("managed", managed.name.as_str())
                                    .with("name", link.name.as_str())
                                    .at(at(&managed.name))
                                    .at(record.source.clone()),
                            );
                            continue;
                        };
                        if written.trim() != taken {
                            findings.push(
                                Diagnostic::new(codes::PROFILE_DELIVERY_PHOTO_DATE_INVALID)
                                    .with("managed", managed.name.as_str())
                                    .with("name", link.name.as_str())
                                    .with("found", written.trim())
                                    .with("taken", taken)
                                    .at(at(&managed.name))
                                    .at(record.source.clone()),
                            );
                        }
                    }
                }
            }
            Plan::PhotoPixels { min, max } => {
                for file in package.files.iter().filter(|file| rule.covers(&file.name)) {
                    cx.check(Kind::Work, 1)?;
                    let size = file
                        .picture
                        .as_ref()
                        .and_then(|picture| Some((picture.width?, picture.height?)));
                    let Some((width, height)) = size else {
                        findings.push(
                            Diagnostic::new(codes::PROFILE_DELIVERY_PHOTO_PIXELS_UNSUPPORTED)
                                .with("name", file.name.as_str())
                                .at(at(&file.name)),
                        );
                        continue;
                    };
                    let pixels = usize::from(width) * usize::from(height);
                    if min.is_some_and(|min| pixels < min) || max.is_some_and(|max| pixels > max) {
                        findings.push(
                            Diagnostic::new(codes::PROFILE_DELIVERY_PHOTO_PIXELS_INVALID)
                                .with("name", file.name.as_str())
                                .with("pixels", pixels as u64)
                                .at(at(&file.name)),
                        );
                    }
                }
            }
        }
        let unsupported = findings.iter().filter(|d| is_unsupported(d)).count() as u64;
        report.rules.push(RuleSummary {
            id: rule.id.clone(),
            check: rule.check.clone(),
            findings: findings.len() as u64 - unsupported,
            unsupported,
        });
        report
            .findings
            .extend(findings.into_iter().map(|diagnostic| {
                let unsupported = is_unsupported(&diagnostic);
                let mut finding = rule.finding(diagnostic);
                if unsupported {
                    finding.level = Severity::Info;
                }
                finding
            }));
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::is_allowed;

    #[test]
    fn characters_outside_jis_x_0208_are_found() {
        for c in ['あ', '漢', '－', '￥', 'Ａ', '　'] {
            assert!(is_allowed(c, true), "{c}");
        }
        // 半角の片仮名、NEC の特殊文字（13 区）、IBM の拡張、JIS X 0208 の外
        for c in ['ｱ', '①', 'Ⅰ', '髙', '𠮷', '😀'] {
            assert!(!is_allowed(c, true), "{c}");
        }
        assert!(!is_allowed('Ａ', false));
        assert!(!is_allowed('９', false));
        assert!(is_allowed('あ', false));
    }
}
