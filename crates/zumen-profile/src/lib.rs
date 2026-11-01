//! プロファイルの照合（決定 0138・0173・0179）。
//!
//! プロファイルの JSON を読み、`extends` を解いて、配置をたどって解いた図面モデルの値（決定 0159）を、規則と
//! 語彙と比べる。指摘は診断と同じ形の値で返し、重要度は規則の `level` にする。照合できない値は、重要度
//! `info` の `profile.<種類>.unsupported` の指摘にし、黙って通さない。

mod civil;
mod profile;

use std::collections::{BTreeMap, BTreeSet};

use zumen_base::{Cx, Diagnostic, Kind, Ref, Severity, Stop, Value as Param, codes};
use zumen_drawing::id::{self, DefinitionNamer, SheetNamer};
use zumen_drawing::{
    Affine, CalloutContent, CellContent, Color, ColorSystem, Drawing2D, Height, HeightMeaning,
    Item, Label, LayerId, Length, Origin, Shape, Visit,
};
use zumen_json::Value;
use zumen_scene::Palette;

pub use crate::civil::{CIVIL_DRAWING, CivilDrawing, civil_drawing};
pub use crate::profile::{FORMAT_MAJOR, Identity};
use crate::profile::{GetKey, text};

/// 照合の種類と、規則に書ける種類ごとの欄（決定 0173）。`xtask` のプロファイルの検査も、ここから引く
pub const CHECKS: &[(&str, &[&str])] = &[
    (
        "layer-name",
        &["parts", "user-part", "separator", "max-width"],
    ),
    ("color", &[]),
    ("line-type", &[]),
    ("line-width", &["tolerance"]),
    ("text-height", &["tolerance"]),
    ("characters", &[]),
    ("sheet-size", &["tolerance"]),
    ("orientation", &["orientation"]),
    ("scale", &[]),
    ("line-width-by-sheet", &["tolerance", "sheets"]),
    (
        "file-name",
        &["parts", "user-part", "extensions", "max-length"],
    ),
    // 点群と設計面の標高の差（決定 0234）。図面の照合では使わない
    (
        "surface-elevation",
        &[
            "work", "part", "slope", "mean", "each", "density", "exclude",
        ],
    ),
    // 電子納品の成果品（決定 0241）。`delivery_check` の op だけで使う
    (
        "delivery-name",
        &[
            "folders",
            "except",
            "when",
            "characters",
            "stem",
            "extension",
            "folder",
            "depth",
            "prefix",
        ],
    ),
    ("delivery-required", &["folders", "except", "when", "names"]),
    ("delivery-link", &["folders", "except", "when", "elements"]),
    ("delivery-unreferenced", &["folders", "except", "when"]),
    (
        "delivery-standard",
        &["folders", "except", "when", "element", "values"],
    ),
    (
        "delivery-characters",
        &["folders", "except", "when", "fullwidth-alphanumerics"],
    ),
    ("delivery-doctype", &["folders", "except", "when", "values"]),
    (
        "delivery-photo-date",
        &["folders", "except", "when", "entry", "file", "date"],
    ),
    (
        "delivery-photo-pixels",
        &["folders", "except", "when", "min", "max"],
    ),
];

/// 語彙を使わない照合
pub const WITHOUT_VOCABULARY: &[&str] = &[
    "layer-name",
    "orientation",
    "file-name",
    "surface-elevation",
    "delivery-name",
    "delivery-required",
    "delivery-link",
    "delivery-unreferenced",
    "delivery-standard",
    "delivery-characters",
    "delivery-doctype",
    "delivery-photo-date",
    "delivery-photo-pixels",
];

/// `tolerance` を書かないときに、同じとみなす差（mm、決定 0173）
pub const DEFAULT_TOLERANCE: f64 = 1e-6;

/// 縮尺の仮数を比べるときに、同じとみなす差
const MANTISSA_TOLERANCE: f64 = 1e-6;

/// 縦横の尺度が同じとみなす相対の差
const UNIFORM_TOLERANCE: f64 = 1e-9;

/// 照合の条件
#[derive(Clone, Debug)]
pub struct Options<'a> {
    /// たどる起点。[`origins`] で作る
    pub origins: Vec<Origin>,
    /// 色の表（決定 0172）。ファイルが RGB を持たない色の RGB を引く
    pub palette: &'a Palette,
    /// 図面の形式の系統（`acad`・`sxf`・`jww`）。線種の語彙の、比べる列の名前にする
    pub family: &'a str,
    /// 照合するファイルの名前（拡張子を含む）。`file-name` の規則で比べる。なければ、その規則は照合できない
    pub file_name: Option<&'a str>,
}

/// 照合の結果
#[derive(Clone, Debug)]
pub struct Report {
    /// 照合に使ったプロファイルの鎖。基から順
    pub chain: Vec<Identity>,
    /// 照合した規則ごとの数。プロファイルの順
    pub rules: Vec<RuleSummary>,
    /// 指摘。規則の順に、規則ごとにたどった順
    pub findings: Vec<Finding>,
}

/// 規則ごとの数
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuleSummary {
    /// 規則の id
    pub id: String,
    /// 照合の種類
    pub check: String,
    /// 指摘の数（照合できなかったものを除く）
    pub findings: u64,
    /// 照合できなかった数
    pub unsupported: u64,
}

/// 指摘
#[derive(Clone, Debug, PartialEq)]
pub struct Finding {
    /// 規則の id
    pub rule: String,
    /// 重要度。規則の `level`。照合できない値は `info`
    pub level: Severity,
    /// code・場所・値。code の重要度は使わない
    pub diagnostic: Diagnostic,
}

/// 照合を始められない理由
#[derive(Clone, Debug, PartialEq)]
pub enum Error {
    /// 選んだプロファイルが、渡した入力にない
    NotFound,
    /// プロファイルを読めないか、`extends` を解けない
    Profile(Box<Diagnostic>),
    /// 予算か中断の合図で止まった
    Stop(Stop, Box<Ref>),
}

/// `from` と `view` から、たどる起点を決める。`from` がなければ、すべての用紙から、用紙がなければ実寸の空間
/// からたどる。使えない値なら、要求の欄の JSON ポインター（`/from`・`/view`）を返す
pub fn origins(
    drawing: &Drawing2D,
    from: Option<&str>,
    view: Option<u64>,
) -> Result<Vec<Origin>, &'static str> {
    let Some(from) = from else {
        if view.is_some() {
            return Err("/view");
        }
        if drawing.sheets.is_empty() {
            return Ok(drawing
                .spaces
                .iter()
                .map(|space| Origin::Space(space.id))
                .collect());
        }
        return Ok(drawing
            .sheets
            .iter()
            .map(|sheet| Origin::Sheet(sheet.id))
            .collect());
    };
    let space = drawing
        .spaces
        .iter()
        .find(|space| id::space(space.id) == from)
        .or_else(|| {
            let mut namer = DefinitionNamer::default();
            drawing
                .symbols
                .iter()
                .find(|definition| namer.next(definition) == from)
                .map(|definition| &definition.space)
        });
    if let Some(space) = space {
        if view.is_some() {
            return Err("/view");
        }
        return Ok(vec![Origin::Space(space.id)]);
    }
    let mut namer = SheetNamer::default();
    let Some(sheet) = drawing
        .sheets
        .iter()
        .find(|sheet| namer.next(sheet) == from)
    else {
        return Err("/from");
    };
    match view {
        None => Ok(vec![Origin::Sheet(sheet.id)]),
        Some(view) => match usize::try_from(view) {
            Ok(view) if view < sheet.views.len() => Ok(vec![Origin::View {
                sheet: sheet.id,
                view,
            }]),
            _ => Err("/view"),
        },
    }
}

/// `extends` を解いた鎖と、規則の id と値
pub type ResolvedRules = (Vec<Identity>, Vec<(String, Value)>);

/// `profiles`（入力の名前とバイト列）から `id` と `edition` のプロファイルを選び、`extends` を解いた鎖と、規則の
/// id と値を返す。図面でない照合（決定 0234）が、規則を自分で読むのに使う
pub fn rules(
    profiles: &[(&str, &[u8])],
    id: &str,
    edition: &str,
    cx: &mut Cx<'_>,
) -> Result<ResolvedRules, Error> {
    let resolved = profile::resolve(profiles, id, edition, cx)?;
    Ok((resolved.chain, resolved.rules))
}

/// `profiles`（入力の名前とバイト列）から `id` と `edition` のプロファイルを選び、`extends` を解いて、図面を
/// 照合する。使えない規則は `profile.rule.unsupported` の診断を文脈に足して、照合しない
pub fn check(
    drawing: &Drawing2D,
    profiles: &[(&str, &[u8])],
    id: &str,
    edition: &str,
    options: &Options<'_>,
    cx: &mut Cx<'_>,
) -> Result<Report, Error> {
    let resolved = profile::resolve(profiles, id, edition, cx)?;
    let rules: Vec<(String, String, Severity, Plan)> = resolved
        .rules
        .iter()
        .filter_map(|(id, rule)| {
            let kind = rule.get_key("check").and_then(text).unwrap_or_default();
            match plan(rule, kind, &resolved.vocabularies, options.family) {
                Some((level, plan)) => Some((id.clone(), kind.to_string(), level, plan)),
                None => {
                    cx.report(
                        Diagnostic::new(codes::PROFILE_RULE_UNSUPPORTED)
                            .with("rule", id.as_str())
                            .with("check", kind),
                    );
                    None
                }
            }
        })
        .collect();
    let context = Context::new(drawing, options, cx)?;
    let mut report = Report {
        chain: resolved.chain,
        rules: Vec::new(),
        findings: Vec::new(),
    };
    for (id, kind, level, plan) in rules {
        let mut out = Out {
            rule: &id,
            level,
            findings: Vec::new(),
            invalid: 0,
            unsupported: 0,
        };
        context.run(&plan, &mut out, cx)?;
        report.rules.push(RuleSummary {
            id: id.clone(),
            check: kind,
            findings: out.invalid,
            unsupported: out.unsupported,
        });
        report.findings.append(&mut out.findings);
    }
    Ok(report)
}

/// 語彙を読んだ規則
#[derive(Clone, Debug)]
enum Plan {
    LayerName {
        parts: Vec<Vec<PartValue>>,
        user_part: bool,
        separator: String,
        max_width: Option<u64>,
    },
    Color {
        vocabulary: String,
        values: Vec<ColorValue>,
    },
    LineType {
        vocabulary: String,
        /// 図面の形式の列の名前。語彙がその列を持たなければ `None`
        names: Option<Vec<String>>,
    },
    LineWidth {
        vocabulary: String,
        values: Vec<f64>,
        tolerance: f64,
    },
    TextHeight {
        vocabulary: String,
        values: Vec<f64>,
        tolerance: f64,
    },
    Characters {
        vocabulary: String,
        ranges: Vec<(u32, u32)>,
    },
    SheetSize {
        vocabulary: String,
        sizes: Vec<[f64; 2]>,
        tolerance: f64,
    },
    Orientation {
        expected: String,
    },
    Scale {
        vocabulary: String,
        mantissas: Vec<f64>,
    },
    LineWidthBySheet {
        vocabulary: String,
        /// 用紙の大きさ（mm）と、その用紙で使える線幅
        rows: Vec<([f64; 2], String, Vec<f64>)>,
        tolerance: f64,
    },
    FileName {
        parts: Vec<NamePart>,
        /// 利用者が決める部分の始めの文字列と、始めの文字列を含めた文字の数の上限
        user_part: Option<(String, usize)>,
        extensions: Vec<String>,
        max_length: Option<usize>,
    },
}

/// ファイルの名前の部分
#[derive(Clone, Debug)]
enum NamePart {
    /// 決まった数の文字。文字はどれかの範囲に入る
    Fixed {
        length: usize,
        ranges: Vec<(char, char)>,
    },
    /// 語彙の符号のどれか
    Codes(Vec<String>),
}

/// レイヤの名前の部分の語彙の値
#[derive(Clone, Debug)]
struct PartValue {
    code: String,
    /// ほかの部分（添字）が取れる符号を限る。`responsibility = ["C"]` など
    limits: Vec<(usize, Vec<String>)>,
}

/// 色の語彙の値
#[derive(Clone, Debug)]
struct ColorValue {
    rgb: Option<[u8; 3]>,
    codes: Vec<(ColorSystem, u32)>,
}

/// 規則を読む。使えなければ `None`
fn plan(
    rule: &Value,
    kind: &str,
    vocabularies: &BTreeMap<String, Value>,
    family: &str,
) -> Option<(Severity, Plan)> {
    let level = match rule.get_key("level").and_then(text)? {
        "error" => Severity::Error,
        "warning" => Severity::Warning,
        "info" => Severity::Info,
        _ => return None,
    };
    let name = || rule.get_key("vocabulary").and_then(text);
    let values = || match vocabularies.get(name()?)?.get_key("values")? {
        Value::Array(values) => Some(values.as_slice()),
        _ => None,
    };
    let tolerance = || match rule.get_key("tolerance") {
        None => Some(DEFAULT_TOLERANCE),
        Some(value) => number(value).filter(|t| *t >= 0.0),
    };
    let plan = match kind {
        "layer-name" => {
            let Value::Array(names) = rule.get_key("parts")? else {
                return None;
            };
            let names: Vec<&str> = names.iter().map(text).collect::<Option<_>>()?;
            let mut parts = Vec::with_capacity(names.len());
            for part in &names {
                let Value::Array(values) = vocabularies.get(*part)?.get_key("values")? else {
                    return None;
                };
                let mut out = Vec::with_capacity(values.len());
                for value in values {
                    let Value::Object(object) = value else {
                        return None;
                    };
                    let mut limits = Vec::new();
                    for (key, allowed) in object.iter() {
                        let Some(index) = names.iter().position(|name| *name == key) else {
                            continue;
                        };
                        let Value::Array(allowed) = allowed else {
                            return None;
                        };
                        let allowed = allowed
                            .iter()
                            .map(|code| text(code).map(str::to_string))
                            .collect::<Option<_>>()?;
                        limits.push((index, allowed));
                    }
                    out.push(PartValue {
                        code: text(value.get_key("code")?)?.to_string(),
                        limits,
                    });
                }
                parts.push(out);
            }
            let separator = text(rule.get_key("separator")?)?.to_string();
            if separator.is_empty() {
                return None;
            }
            let user_part = match rule.get_key("user-part") {
                None => false,
                Some(Value::Bool(flag)) => *flag,
                Some(_) => return None,
            };
            let max_width = match rule.get_key("max-width") {
                None => None,
                Some(Value::Number(n)) => Some(u64::try_from(n.to_safe_integer()?).ok()?),
                Some(_) => return None,
            };
            Plan::LayerName {
                parts,
                user_part,
                separator,
                max_width,
            }
        }
        "color" => {
            let mut out = Vec::new();
            for value in values()? {
                let rgb = match value.get_key("rgb") {
                    None => None,
                    Some(rgb) => Some(hex(text(rgb)?)?),
                };
                let mut codes = Vec::new();
                for (key, system) in SYSTEMS {
                    match value.get_key(key) {
                        None => {}
                        Some(Value::Number(n)) => {
                            codes.push((system, u32::try_from(n.to_safe_integer()?).ok()?));
                        }
                        Some(_) => return None,
                    }
                }
                out.push(ColorValue { rgb, codes });
            }
            Plan::Color {
                vocabulary: name()?.to_string(),
                values: out,
            }
        }
        "line-type" => {
            let values = values()?;
            let names = if values.iter().any(|value| value.get_key(family).is_some()) {
                let mut names = Vec::new();
                for value in values {
                    if let Some(name) = value.get_key(family) {
                        names.push(text(name)?.to_string());
                    }
                }
                Some(names)
            } else {
                None
            };
            Plan::LineType {
                vocabulary: name()?.to_string(),
                names,
            }
        }
        "line-width" | "text-height" => {
            let numbers = values()?.iter().map(number).collect::<Option<Vec<_>>>()?;
            let (vocabulary, tolerance) = (name()?.to_string(), tolerance()?);
            if kind == "line-width" {
                Plan::LineWidth {
                    vocabulary,
                    values: numbers,
                    tolerance,
                }
            } else {
                Plan::TextHeight {
                    vocabulary,
                    values: numbers,
                    tolerance,
                }
            }
        }
        "characters" => Plan::Characters {
            vocabulary: name()?.to_string(),
            ranges: values()?
                .iter()
                .map(|value| range(text(value)?))
                .collect::<Option<_>>()?,
        },
        "sheet-size" => Plan::SheetSize {
            vocabulary: name()?.to_string(),
            sizes: values()?
                .iter()
                .map(|value| {
                    Some([
                        number(value.get_key("width")?)?,
                        number(value.get_key("height")?)?,
                    ])
                })
                .collect::<Option<_>>()?,
            tolerance: tolerance()?,
        },
        "orientation" => match rule.get_key("orientation").and_then(text)? {
            expected @ ("landscape" | "portrait") => Plan::Orientation {
                expected: expected.to_string(),
            },
            _ => return None,
        },
        "scale" => Plan::Scale {
            vocabulary: name()?.to_string(),
            mantissas: values()?
                .iter()
                .map(|value| number(value.get_key("mantissa").unwrap_or(value)))
                .collect::<Option<Vec<_>>>()?
                .into_iter()
                .filter(|m| (1.0..10.0).contains(m))
                .collect(),
        },
        "line-width-by-sheet" => {
            let sheets = match vocabularies
                .get(text(rule.get_key("sheets")?)?)?
                .get_key("values")?
            {
                Value::Array(values) => values.as_slice(),
                _ => return None,
            };
            let mut rows = Vec::new();
            for value in values()? {
                let Value::Object(object) = value else {
                    return None;
                };
                let name = text(value.get_key("sheet")?)?;
                let sheet = sheets
                    .iter()
                    .find(|sheet| sheet.get_key("name").and_then(text) == Some(name))?;
                let size = [
                    number(sheet.get_key("width")?)?,
                    number(sheet.get_key("height")?)?,
                ];
                let widths = object
                    .iter()
                    .filter(|(key, _)| *key != "sheet")
                    .map(|(_, width)| number(width))
                    .collect::<Option<Vec<_>>>()?;
                rows.push((size, name.to_string(), widths));
            }
            Plan::LineWidthBySheet {
                vocabulary: name()?.to_string(),
                rows,
                tolerance: tolerance()?,
            }
        }
        "file-name" => {
            let Value::Array(items) = rule.get_key("parts")? else {
                return None;
            };
            let mut parts = Vec::with_capacity(items.len());
            for item in items {
                let part = match (item.get_key("vocabulary"), item.get_key("length")) {
                    (Some(name), None) => {
                        let Value::Array(values) =
                            vocabularies.get(text(name)?)?.get_key("values")?
                        else {
                            return None;
                        };
                        let codes = values
                            .iter()
                            .map(|value| text(value.get_key("code")?).map(str::to_string))
                            .collect::<Option<Vec<_>>>()?;
                        NamePart::Codes(codes)
                    }
                    (None, Some(Value::Number(length))) => NamePart::Fixed {
                        length: usize::try_from(length.to_safe_integer()?)
                            .ok()
                            .filter(|n| *n > 0)?,
                        ranges: char_ranges(text(item.get_key("characters")?)?)?,
                    },
                    _ => return None,
                };
                parts.push(part);
            }
            let user_part = match rule.get_key("user-part") {
                None => None,
                Some(user) => {
                    let prefix = text(user.get_key("prefix")?)?.to_string();
                    let Value::Number(max) = user.get_key("max-length")? else {
                        return None;
                    };
                    Some((prefix, usize::try_from(max.to_safe_integer()?).ok()?))
                }
            };
            let Value::Array(extensions) = rule.get_key("extensions")? else {
                return None;
            };
            let extensions = extensions
                .iter()
                .map(|e| text(e).map(str::to_string))
                .collect::<Option<Vec<_>>>()?;
            let max_length = match rule.get_key("max-length") {
                None => None,
                Some(Value::Number(n)) => Some(usize::try_from(n.to_safe_integer()?).ok()?),
                Some(_) => return None,
            };
            Plan::FileName {
                parts,
                user_part,
                extensions,
                max_length,
            }
        }
        _ => return None,
    };
    Some((level, plan))
}

/// 語彙の色の列と、図面の色の系統
const SYSTEMS: [(&str, ColorSystem); 3] = [
    ("aci", ColorSystem::Aci),
    ("sxf", ColorSystem::Sxf),
    ("jww", ColorSystem::Jww),
];

/// 有限の数
fn number(value: &Value) -> Option<f64> {
    match value {
        Value::Number(n) => n.to_f64(),
        _ => None,
    }
}

/// `#rrggbb` の RGB
fn hex(text: &str) -> Option<[u8; 3]> {
    let digits = text.strip_prefix('#')?;
    if digits.len() != 6 || !digits.is_ascii() {
        return None;
    }
    let byte = |at: usize| u8::from_str_radix(digits.get(at..at + 2)?, 16).ok();
    Some([byte(0)?, byte(2)?, byte(4)?])
}

/// `XXXX` か `XXXX-YYYY`（16 進の符号位置）の範囲
fn range(text: &str) -> Option<(u32, u32)> {
    let (first, last) = text.split_once('-').unwrap_or((text, text));
    let first = u32::from_str_radix(first, 16).ok()?;
    let last = u32::from_str_radix(last, 16).ok()?;
    (first <= last).then_some((first, last))
}

/// `0-9A-Z` の形の文字の範囲。1 文字だけも書ける
fn char_ranges(text: &str) -> Option<Vec<(char, char)>> {
    let chars: Vec<char> = text.chars().collect();
    let mut ranges = Vec::new();
    let mut rest = chars.as_slice();
    while let Some((&first, tail)) = rest.split_first() {
        match tail {
            ['-', last, tail @ ..] => {
                if first > *last {
                    return None;
                }
                ranges.push((first, *last));
                rest = tail;
            }
            _ => {
                ranges.push((first, first));
                rest = tail;
            }
        }
    }
    (!ranges.is_empty()).then_some(ranges)
}

/// ファイルの名前が規則に合うか。合わなければ理由の鍵を返す
fn file_name_problem(
    name: &str,
    parts: &[NamePart],
    user_part: Option<&(String, usize)>,
    extensions: &[String],
    max_length: Option<usize>,
) -> Option<&'static str> {
    if max_length.is_some_and(|max| name.chars().count() > max) {
        return Some("length");
    }
    let Some((stem, extension)) = name.rsplit_once('.') else {
        return Some("extension");
    };
    if !extensions.iter().any(|known| known == extension) {
        return Some("extension");
    }
    let mut rest = stem;
    for part in parts {
        match part {
            NamePart::Fixed { length, ranges } => {
                let mut chars = rest.char_indices();
                for _ in 0..*length {
                    match chars.next() {
                        Some((_, c)) if ranges.iter().any(|(a, b)| (*a..=*b).contains(&c)) => {}
                        _ => return Some("part"),
                    }
                }
                rest = chars.as_str();
            }
            NamePart::Codes(codes) => {
                let Some(code) = codes
                    .iter()
                    .filter(|code| rest.starts_with(code.as_str()))
                    .max_by_key(|code| code.len())
                else {
                    return Some("part");
                };
                rest = rest.get(code.len()..).unwrap_or_default();
            }
        }
    }
    if rest.is_empty() {
        return None;
    }
    let Some((prefix, max)) = user_part else {
        return Some("part");
    };
    let Some(user) = rest.strip_prefix(prefix.as_str()) else {
        return Some("part");
    };
    if rest.chars().count() > *max {
        return Some("user-length");
    }
    // 半角の文字は、英大文字と数字に限る
    if user
        .chars()
        .any(|c| c.is_ascii() && !(c.is_ascii_uppercase() || c.is_ascii_digit()))
    {
        return Some("user-characters");
    }
    None
}

/// 1 つの規則の指摘を集める
struct Out<'r> {
    rule: &'r str,
    level: Severity,
    findings: Vec<Finding>,
    invalid: u64,
    unsupported: u64,
}

impl Out<'_> {
    fn invalid(&mut self, diagnostic: Diagnostic) {
        self.invalid += 1;
        self.findings.push(Finding {
            rule: self.rule.to_string(),
            level: self.level,
            diagnostic: diagnostic.with("rule", self.rule),
        });
    }

    fn unsupported(&mut self, diagnostic: Diagnostic) {
        self.unsupported += 1;
        self.findings.push(Finding {
            rule: self.rule.to_string(),
            level: Severity::Info,
            diagnostic: diagnostic.with("rule", self.rule),
        });
    }
}

/// たどった要素と、用紙の範囲
struct Context<'a> {
    drawing: &'a Drawing2D,
    palette: &'a Palette,
    /// たどった要素と、たどった起点の用紙（`drawing.sheets` の添字）。実寸の空間からたどれば `None`
    visits: Vec<(Option<usize>, Visit<'a>)>,
    /// 照合する用紙（添字）と、用紙の大きさと向きも比べるかと、比べるビューの添字
    sheets: Vec<(usize, bool, Vec<usize>)>,
    /// 用紙の ID
    sheet_ids: Vec<String>,
    /// 照合するファイルの名前
    file_name: Option<&'a str>,
}

impl<'a> Context<'a> {
    fn new(drawing: &'a Drawing2D, options: &Options<'a>, cx: &mut Cx<'_>) -> Result<Self, Error> {
        let mut namer = SheetNamer::default();
        let sheet_ids = drawing.sheets.iter().map(|s| namer.next(s)).collect();
        let index = |id| drawing.sheets.iter().position(|sheet| sheet.id == id);
        let mut visits = Vec::new();
        let mut sheets = Vec::new();
        for &origin in &options.origins {
            let sheet = match origin {
                Origin::Space(_) => None,
                Origin::Sheet(id) => index(id).inspect(|&at| {
                    let views = drawing.sheets.get(at).map_or(0, |s| s.views.len());
                    sheets.push((at, true, (0..views).collect()));
                }),
                Origin::View { sheet, view } => index(sheet).inspect(|&at| {
                    sheets.push((at, false, vec![view]));
                }),
            };
            let walked = drawing
                .walk(origin, cx)
                .map_err(|(stop, at)| Error::Stop(stop, Box::new(at)))?;
            visits.extend(
                walked
                    .into_iter()
                    .filter(|visit| visit.visible)
                    .map(|visit| (sheet, visit)),
            );
        }
        Ok(Self {
            drawing,
            palette: options.palette,
            visits,
            sheets,
            sheet_ids,
            file_name: options.file_name,
        })
    }

    fn run(&self, plan: &Plan, out: &mut Out<'_>, cx: &mut Cx<'_>) -> Result<(), Error> {
        let work = |cx: &mut Cx<'_>| {
            cx.check(Kind::Work, 1)
                .map_err(|stop| Error::Stop(stop, Box::default()))
        };
        match plan {
            Plan::LayerName {
                parts,
                user_part,
                separator,
                max_width,
            } => {
                let used: BTreeSet<LayerId> = self.visits.iter().map(|(_, v)| v.layer).collect();
                for layer in &self.drawing.layers {
                    work(cx)?;
                    if !used.contains(&layer.id) {
                        continue;
                    }
                    if !layer_name_ok(&layer.name, parts, *user_part, separator, *max_width) {
                        out.invalid(
                            Diagnostic::new(codes::PROFILE_LAYER_NAME_INVALID)
                                .with("layer", layer.name.as_str())
                                .at(Ref::default().with_id(id::layer(layer))),
                        );
                    }
                }
            }
            Plan::FileName {
                parts,
                user_part,
                extensions,
                max_length,
            } => match self.file_name {
                Some(name) => {
                    if let Some(reason) =
                        file_name_problem(name, parts, user_part.as_ref(), extensions, *max_length)
                    {
                        out.invalid(
                            Diagnostic::new(codes::PROFILE_FILE_NAME_INVALID)
                                .with("name", name)
                                .with("reason", Param::Key(reason)),
                        );
                    }
                }
                None => out.unsupported(Diagnostic::new(codes::PROFILE_FILE_NAME_UNSUPPORTED)),
            },
            Plan::Orientation { .. } | Plan::SheetSize { .. } | Plan::Scale { .. } => {
                for (at, whole, views) in &self.sheets {
                    work(cx)?;
                    self.sheet(plan, *at, *whole, views, out);
                }
            }
            _ => {
                for (sheet, visit) in &self.visits {
                    work(cx)?;
                    self.element(plan, *sheet, visit, out);
                }
            }
        }
        Ok(())
    }

    /// 要素の場所。要素・実効のレイヤ・起点の用紙の ID
    fn places(&self, sheet: Option<usize>, visit: &Visit<'_>) -> Vec<Ref> {
        let mut places = vec![Ref::default().with_id(visit.id.clone())];
        if let Some(layer) = self.drawing.layer(visit.layer) {
            places.push(Ref::default().with_id(id::layer(layer)));
        }
        if let Some(id) = sheet.and_then(|at| self.sheet_ids.get(at)) {
            places.push(Ref::default().with_id(id.clone()));
        }
        places
    }

    fn element(&self, plan: &Plan, sheet: Option<usize>, visit: &Visit<'_>, out: &mut Out<'_>) {
        let summary = summary(visit.item);
        let at = |diagnostic: Diagnostic| {
            self.places(sheet, visit)
                .into_iter()
                .fold(diagnostic, Diagnostic::at)
        };
        match plan {
            Plan::Color { vocabulary, values } if summary.colored => {
                match color_check(visit.color, values, self.palette) {
                    ColorResult::Ok => {}
                    ColorResult::Invalid(found) => {
                        out.invalid(at(Diagnostic::new(codes::PROFILE_COLOR_INVALID)
                            .with("color", found)
                            .with("vocabulary", vocabulary.as_str())))
                    }
                    ColorResult::Unsupported => {
                        out.unsupported(at(Diagnostic::new(codes::PROFILE_COLOR_UNSUPPORTED)));
                    }
                }
            }
            Plan::LineType { vocabulary, names } if summary.lines => {
                let name = visit
                    .line_type
                    .and_then(|id| self.drawing.line_type(id))
                    .map(|line_type| line_type.name.as_str());
                match (name, names) {
                    (Some(name), Some(names)) => {
                        if !names.iter().any(|known| known.eq_ignore_ascii_case(name)) {
                            out.invalid(at(Diagnostic::new(codes::PROFILE_LINE_TYPE_INVALID)
                                .with("line_type", name)
                                .with("vocabulary", vocabulary.as_str())));
                        }
                    }
                    _ => out.unsupported(at(Diagnostic::new(codes::PROFILE_LINE_TYPE_UNSUPPORTED))),
                }
            }
            Plan::LineWidth {
                vocabulary,
                values,
                tolerance,
            } if summary.lines => match visit.line_weight {
                Some(width) if width.0.is_finite() => {
                    if !values.iter().any(|v| (v - width.0).abs() <= *tolerance) {
                        out.invalid(at(Diagnostic::new(codes::PROFILE_LINE_WIDTH_INVALID)
                            .with("width", width.0)
                            .with("vocabulary", vocabulary.as_str())));
                    }
                }
                _ => out.unsupported(at(Diagnostic::new(codes::PROFILE_LINE_WIDTH_UNSUPPORTED))),
            },
            Plan::LineWidthBySheet {
                vocabulary,
                rows,
                tolerance,
            } if summary.lines => {
                // 用紙を通さない要素と、表にない大きさの用紙は比べない
                let Some([w, h]) = sheet
                    .and_then(|at| self.drawing.sheets.get(at))
                    .and_then(|s| s.size)
                else {
                    return;
                };
                let near = |a: f64, b: f64| (a - b).abs() <= *tolerance;
                let Some((_, name, widths)) = rows.iter().find(|([sw, sh], _, _)| {
                    (near(w.0, *sw) && near(h.0, *sh)) || (near(w.0, *sh) && near(h.0, *sw))
                }) else {
                    return;
                };
                match visit.line_weight {
                    Some(width) if width.0.is_finite() => {
                        if !widths.iter().any(|v| (v - width.0).abs() <= *tolerance) {
                            out.invalid(at(Diagnostic::new(
                                codes::PROFILE_LINE_WIDTH_BY_SHEET_INVALID,
                            )
                            .with("width", width.0)
                            .with("sheet", name.as_str())
                            .with("vocabulary", vocabulary.as_str())));
                        }
                    }
                    _ => out.unsupported(at(Diagnostic::new(
                        codes::PROFILE_LINE_WIDTH_BY_SHEET_UNSUPPORTED,
                    ))),
                }
            }
            Plan::TextHeight {
                vocabulary,
                values,
                tolerance,
            } => {
                let Some(height) = summary.height else {
                    return;
                };
                let value = match height.value {
                    Length::Paper(paper) => Some(paper.0),
                    // 実寸の高さは、用紙を通したときだけ用紙の高さにする（決定 0173）
                    Length::Model(model) => sheet.map(|_| model.value),
                };
                match value
                    .zip(uniform_scale(&visit.transform))
                    .map(|(value, scale)| value * scale)
                    .filter(|h| h.is_finite())
                {
                    Some(h) => {
                        if !values.iter().any(|v| (v - h).abs() <= *tolerance) {
                            out.invalid(at(Diagnostic::new(codes::PROFILE_TEXT_HEIGHT_INVALID)
                                .with("height", h)
                                .with("meaning", Param::Key(meaning(&height.meaning)))
                                .with("vocabulary", vocabulary.as_str())));
                        }
                    }
                    None => {
                        out.unsupported(at(Diagnostic::new(
                            codes::PROFILE_TEXT_HEIGHT_UNSUPPORTED,
                        )));
                    }
                }
            }
            Plan::Characters { vocabulary, ranges } => {
                let found: BTreeSet<char> = summary
                    .texts
                    .iter()
                    .flat_map(|text| text.chars())
                    .filter(|c| {
                        let c = u32::from(*c);
                        ranges
                            .iter()
                            .any(|(first, last)| (*first..=*last).contains(&c))
                    })
                    .collect();
                for c in found {
                    out.invalid(at(Diagnostic::new(codes::PROFILE_CHARACTERS_INVALID)
                        .with("character", format!("U+{:04X}", u32::from(c)))
                        .with("vocabulary", vocabulary.as_str())));
                }
            }
            _ => {}
        }
    }

    fn sheet(&self, plan: &Plan, at: usize, whole: bool, views: &[usize], out: &mut Out<'_>) {
        let Some(sheet) = self.drawing.sheets.get(at) else {
            return;
        };
        let place = || Ref::default().with_id(self.sheet_ids.get(at).cloned().unwrap_or_default());
        match plan {
            Plan::SheetSize {
                vocabulary,
                sizes,
                tolerance,
            } if whole => match sheet.size {
                Some([w, h]) => {
                    let near = |a: f64, b: f64| (a - b).abs() <= *tolerance;
                    let fits = sizes.iter().any(|[sw, sh]| {
                        (near(w.0, *sw) && near(h.0, *sh)) || (near(w.0, *sh) && near(h.0, *sw))
                    });
                    if !fits {
                        out.invalid(
                            Diagnostic::new(codes::PROFILE_SHEET_SIZE_INVALID)
                                .with("width", w.0)
                                .with("height", h.0)
                                .with("vocabulary", vocabulary.as_str())
                                .at(place()),
                        );
                    }
                }
                None => out.unsupported(
                    Diagnostic::new(codes::PROFILE_SHEET_SIZE_UNSUPPORTED).at(place()),
                ),
            },
            Plan::Orientation { expected } if whole => match sheet.size {
                Some([w, h]) => {
                    let found = if w.0 >= h.0 { "landscape" } else { "portrait" };
                    if found != expected {
                        out.invalid(
                            Diagnostic::new(codes::PROFILE_ORIENTATION_INVALID)
                                .with("orientation", Param::Key(found))
                                .with("expected", expected.as_str())
                                .at(place()),
                        );
                    }
                }
                None => out.unsupported(
                    Diagnostic::new(codes::PROFILE_ORIENTATION_UNSUPPORTED).at(place()),
                ),
            },
            Plan::Scale {
                vocabulary,
                mantissas,
            } => {
                for &index in views {
                    let Some(view) = sheet.views.get(index) else {
                        continue;
                    };
                    let unit = self
                        .drawing
                        .spaces
                        .iter()
                        .chain(self.drawing.symbols.iter().map(|d| &d.space))
                        .find(|space| space.id == view.space)
                        .and_then(|space| space.unit.millimeters());
                    let denominator = unit
                        .zip(uniform_scale(&view.transform))
                        .map(|(unit, scale)| unit / scale)
                        .filter(|d| d.is_finite() && *d > 0.0);
                    let number = index as u64;
                    match denominator {
                        Some(denominator) => {
                            let m = mantissa(denominator);
                            if !mantissas
                                .iter()
                                .any(|known| (known - m).abs() <= MANTISSA_TOLERANCE)
                            {
                                out.invalid(
                                    Diagnostic::new(codes::PROFILE_SCALE_INVALID)
                                        .with("denominator", denominator)
                                        .with("view", number)
                                        .with("vocabulary", vocabulary.as_str())
                                        .at(place()),
                                );
                            }
                        }
                        None => out.unsupported(
                            Diagnostic::new(codes::PROFILE_SCALE_UNSUPPORTED)
                                .with("view", number)
                                .at(place()),
                        ),
                    }
                }
            }
            _ => {}
        }
    }
}

/// 要素の、照合に使う性質
#[derive(Default)]
struct Summary<'a> {
    /// 線を描く。線種と線幅を比べる
    lines: bool,
    /// 色を比べる
    colored: bool,
    /// 文字の高さ
    height: Option<&'a Height>,
    /// 文字の内容
    texts: Vec<&'a str>,
}

/// 要素の種類ごとに、比べる値を決める（決定 0179）
fn summary<'a>(item: Item<'a>) -> Summary<'a> {
    match item {
        Item::Model(element) => shape_summary(&element.shape),
        Item::Paper(element) => shape_summary(&element.shape),
        Item::ModelAttribute { symbol, index } => match &symbol.shape {
            Shape::Symbol(symbol) => symbol
                .attributes
                .get(index)
                .map(|a| {
                    attribute_summary(
                        &a.text.content,
                        &a.text.height,
                        a.formatted.as_ref().map(|f| f.content.as_str()),
                    )
                })
                .unwrap_or_default(),
            _ => Summary::default(),
        },
        Item::PaperAttribute { symbol, index } => match &symbol.shape {
            Shape::Symbol(symbol) => symbol
                .attributes
                .get(index)
                .map(|a| {
                    attribute_summary(
                        &a.text.content,
                        &a.text.height,
                        a.formatted.as_ref().map(|f| f.content.as_str()),
                    )
                })
                .unwrap_or_default(),
            _ => Summary::default(),
        },
    }
}

fn attribute_summary<'a>(
    content: &'a str,
    height: &'a Height,
    formatted: Option<&'a str>,
) -> Summary<'a> {
    Summary {
        lines: false,
        colored: true,
        height: Some(height),
        texts: std::iter::once(content).chain(formatted).collect(),
    }
}

fn shape_summary<P>(shape: &Shape<P>) -> Summary<'_> {
    let lines = |texts| Summary {
        lines: true,
        colored: true,
        height: None,
        texts,
    };
    match shape {
        Shape::Line { .. }
        | Shape::Circle { .. }
        | Shape::Curve(_)
        | Shape::Polyline(_)
        | Shape::Leader(_) => lines(Vec::new()),
        Shape::Dimension(dimension) => lines(match &dimension.label {
            Label::Override { content, .. } => vec![content.as_str()],
            _ => Vec::new(),
        }),
        Shape::Callout(callout) => lines(match &callout.content {
            CalloutContent::Text(text) => vec![text.content.as_str()],
            _ => Vec::new(),
        }),
        Shape::Table(table) => lines(
            table
                .cells
                .iter()
                .filter_map(|cell| match &cell.content {
                    CellContent::Text { content, .. } => Some(content.as_str()),
                    _ => None,
                })
                .collect(),
        ),
        Shape::Text(text) => Summary {
            lines: false,
            colored: true,
            height: Some(&text.height),
            texts: vec![text.content.as_str()],
        },
        Shape::Formatted(text) => Summary {
            lines: false,
            colored: true,
            height: Some(&text.height),
            texts: vec![text.content.as_str()],
        },
        Shape::Hatch(_) | Shape::Point(_) => Summary {
            colored: true,
            ..Summary::default()
        },
        Shape::Symbol(_) | Shape::Underlay(_) => Summary::default(),
    }
}

/// 高さの意味のキー
fn meaning(meaning: &HeightMeaning) -> &'static str {
    match meaning {
        HeightMeaning::Frame => "frame",
        HeightMeaning::CapHeight => "cap-height",
        HeightMeaning::Em => "em",
        HeightMeaning::Declared(key) => key,
    }
}

enum ColorResult {
    Ok,
    Invalid(String),
    Unsupported,
}

/// 色を語彙と比べる。系統の列が語彙にあれば符号で、なければ RGB で比べる（決定 0173）
fn color_check(color: Option<Color>, values: &[ColorValue], palette: &Palette) -> ColorResult {
    let Some(color) = color else {
        return ColorResult::Unsupported;
    };
    if let Some(code) = color.code {
        let column = values
            .iter()
            .any(|value| value.codes.iter().any(|(s, _)| *s == code.system));
        if column {
            let known = values
                .iter()
                .any(|value| value.codes.contains(&(code.system, code.value)));
            return if known {
                ColorResult::Ok
            } else {
                let (name, _) = SYSTEMS
                    .iter()
                    .find(|(_, system)| *system == code.system)
                    .copied()
                    .unwrap_or(("", code.system));
                ColorResult::Invalid(format!("{name}:{}", code.value))
            };
        }
    }
    let rgb = color.rgb.or_else(|| {
        color
            .code
            .and_then(|code| palette.get(code.system, code.value))
    });
    match rgb {
        Some(rgb) if values.iter().any(|value| value.rgb.is_some()) => {
            if values.iter().any(|value| value.rgb == Some(rgb)) {
                ColorResult::Ok
            } else {
                let [r, g, b] = rgb;
                ColorResult::Invalid(format!("#{r:02x}{g:02x}{b:02x}"))
            }
        }
        _ => ColorResult::Unsupported,
    }
}

/// 縦横の尺度が同じ変換の尺度。回転と反転は問わない。違えば `None`
fn uniform_scale(transform: &Affine) -> Option<f64> {
    let [a, b, _, d, e, _] = transform.m;
    let sx = (a * a + d * d).sqrt();
    let sy = (b * b + e * e).sqrt();
    let tolerance = UNIFORM_TOLERANCE * sx.max(sy);
    ((sx - sy).abs() <= tolerance
        && (a * b + d * e).abs() <= tolerance * sx.max(sy)
        && sx.is_finite()
        && sx > 0.0)
        .then_some(sx)
}

/// 縮尺の分母の仮数（1 以上 10 未満）。10 に近い仮数は 1 にする
fn mantissa(denominator: f64) -> f64 {
    let exponent = libm::floor(libm::log10(denominator));
    let mut m = denominator / libm::pow(10.0, exponent);
    if m < 1.0 {
        m *= 10.0;
    }
    if m >= 10.0 - MANTISSA_TOLERANCE {
        m /= 10.0;
    }
    m
}

/// レイヤの名前が、部分の語彙と幅の上限に合う（決定 0173）
fn layer_name_ok(
    name: &str,
    parts: &[Vec<PartValue>],
    user_part: bool,
    separator: &str,
    max_width: Option<u64>,
) -> bool {
    if let Some(max) = max_width {
        // 半角を 1、全角を 2 と数える
        let width: u64 = name
            .chars()
            .map(|c| {
                let c = u32::from(c);
                if c < 0x80 || (0xFF61..=0xFF9F).contains(&c) {
                    1
                } else {
                    2
                }
            })
            .sum();
        if width > max {
            return false;
        }
    }
    let segments: Vec<&str> = name.split(separator).collect();
    if segments.len() < parts.len() || (segments.len() > parts.len() && !user_part) {
        return false;
    }
    let mut matched = Vec::with_capacity(parts.len());
    for (part, segment) in parts.iter().zip(&segments) {
        let Some(value) = part.iter().find(|value| value.code == *segment) else {
            return false;
        };
        matched.push(value);
    }
    matched.iter().all(|value| {
        value.limits.iter().all(|(index, allowed)| {
            segments
                .get(*index)
                .is_some_and(|segment| allowed.iter().any(|code| code == segment))
        })
    })
}

#[cfg(test)]
mod tests;
