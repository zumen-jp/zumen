//! メッセージのスキーマ（`schema/messages.json`）から、Rust の型を作る（決定 0011・0012）。
//!
//! JSON Schema のうち、どの言語の型にも素直に写せる部分だけを受け付ける。書き方は `schema/README.md` にある。

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;

use zumen_json::{Object, Value};

/// JSON Schema の版
const DIALECT: &str = "https://json-schema.org/draft/2020-12/schema";

/// 生成せず、コアが手で書く定義
const HANDWRITTEN: &[&str] = &["response"];

/// Rust の予約語。欄の名前に使わない
const KEYWORDS: &[&str] = &[
    "as", "async", "await", "box", "break", "const", "continue", "crate", "do", "dyn", "else",
    "enum", "extern", "false", "final", "fn", "for", "gen", "if", "impl", "in", "let", "loop",
    "macro", "match", "mod", "move", "mut", "override", "priv", "pub", "ref", "return", "self",
    "static", "struct", "super", "trait", "true", "try", "type", "typeof", "unsafe", "unsized",
    "use", "virtual", "where", "while", "yield",
];

const MAX_SAFE: i64 = zumen_json::MAX_SAFE_INTEGER;

/// 読んだスキーマ
#[derive(Debug)]
pub struct Schema {
    pub defs: Vec<Def>,
}

/// `$defs` の 1 つの定義
#[derive(Debug)]
pub struct Def {
    pub name: String,
    pub description: String,
    pub shape: Shape,
}

/// 定義の形
#[derive(Debug)]
pub enum Shape {
    /// オブジェクト。`closed` でなければ、`allOf` で取り込まれるだけのもの
    Object {
        fields: Vec<Field>,
        compose: Vec<String>,
        closed: bool,
    },
    /// 文字列の列挙
    Enum { values: Vec<String> },
    /// タグつきの共用体。どの形もオブジェクトで、同じ名前の `const` の欄を目印に持つ
    Union { variants: Vec<String> },
    /// JSON の型で見分けられる、スカラーの共用体
    Scalars { members: Vec<Member> },
    /// コアが手で書く
    Handwritten,
}

/// オブジェクトの欄
#[derive(Debug)]
pub struct Field {
    pub name: String,
    pub description: String,
    pub ty: Ty,
    pub required: bool,
}

/// 欄の型
#[derive(Clone, Debug, PartialEq)]
pub enum Ty {
    String,
    Bool,
    UInt {
        min: u64,
        max: u64,
    },
    Int {
        min: i64,
        max: i64,
    },
    Float,
    Ref(String),
    Array(Box<Ty>),
    Map(Box<Ty>),
    /// 決まった文字列。共用体の目印にだけ使う
    Const(String),
}

/// スカラーの共用体の 1 つの形
#[derive(Debug)]
pub enum Member {
    Integer {
        min: i64,
        max: i64,
        description: String,
    },
    Number {
        description: String,
    },
    String {
        description: String,
    },
    Const {
        value: String,
        description: String,
    },
}

impl Member {
    fn variant(&self) -> String {
        match self {
            Member::Integer { .. } => "Integer".to_string(),
            Member::Number { .. } => "Number".to_string(),
            Member::String { .. } => "String".to_string(),
            Member::Const { value, .. } => pascal(value),
        }
    }

    pub(crate) fn description(&self) -> &str {
        match self {
            Member::Integer { description, .. }
            | Member::Number { description }
            | Member::String { description }
            | Member::Const { description, .. } => description,
        }
    }
}

fn is_snake(text: &str) -> bool {
    let mut chars = text.chars();
    chars.next().is_some_and(|c| c.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        && !text.contains("__")
        && !text.ends_with('_')
}

pub(crate) fn pascal(text: &str) -> String {
    text.split('_')
        .map(|word| {
            let mut chars = word.chars();
            chars
                .next()
                .map(|first| first.to_ascii_uppercase().to_string() + chars.as_str())
                .unwrap_or_default()
        })
        .collect()
}

struct Reader {
    errors: Vec<String>,
}

impl Reader {
    fn error(&mut self, at: &str, message: impl AsRef<str>) {
        self.errors.push(format!("{at}: {}", message.as_ref()));
    }

    fn object<'a>(&mut self, value: &'a Value, at: &str) -> Option<&'a Object> {
        match value {
            Value::Object(object) => Some(object),
            _ => {
                self.error(at, "オブジェクトでない");
                None
            }
        }
    }

    fn allow(&mut self, object: &Object, at: &str, allowed: &[&str]) {
        for (key, _) in object.iter() {
            if !allowed.contains(&key) {
                self.error(at, format!("`{key}` は使えない（schema/README.md）"));
            }
        }
    }

    fn description(&mut self, object: &Object, at: &str) -> String {
        match object.get("description") {
            Some(Value::String(text)) if !text.trim().is_empty() => text.clone(),
            _ => {
                self.error(at, "description（日本語の説明）が要る");
                String::new()
            }
        }
    }

    fn integer(&mut self, object: &Object, key: &str, at: &str) -> Option<i64> {
        let value = object.get(key)?;
        let integer = match value {
            Value::Number(number) => number.to_safe_integer(),
            _ => None,
        };
        if integer.is_none() {
            self.error(at, format!("`{key}` は ±(2^53 − 1) の中の整数にする"));
        }
        integer
    }

    fn reference(&mut self, value: &Value, at: &str) -> Option<String> {
        let target = match value {
            Value::String(text) => text.strip_prefix("#/$defs/"),
            _ => None,
        };
        if target.is_none() {
            self.error(at, "$ref は #/$defs/<名前> にする");
        }
        target.map(str::to_string)
    }

    fn def(&mut self, name: &str, value: &Value) -> Option<Def> {
        let at = format!("$defs/{name}");
        if !is_snake(name) {
            self.error(&at, "名前は英小文字・数字・_ にする");
        }
        let object = self.object(value, &at)?;
        let description = self.description(object, &at);
        let shape = if HANDWRITTEN.contains(&name) {
            Shape::Handwritten
        } else if let Some(members) = object.get("oneOf") {
            self.allow(object, &at, &["description", "oneOf"]);
            self.one_of(members, &at)?
        } else {
            match object.get("type") {
                Some(Value::String(ty)) if ty == "string" => {
                    self.allow(object, &at, &["description", "type", "enum"]);
                    Shape::Enum {
                        values: self.enum_values(object, &at),
                    }
                }
                Some(Value::String(ty)) if ty == "object" => self.object_def(object, &at),
                _ => {
                    self.error(&at, "形が分からない（object・文字列の enum・oneOf にする）");
                    return None;
                }
            }
        };
        Some(Def {
            name: name.to_string(),
            description,
            shape,
        })
    }

    fn enum_values(&mut self, object: &Object, at: &str) -> Vec<String> {
        let mut values = Vec::new();
        let Some(Value::Array(items)) = object.get("enum") else {
            self.error(at, "文字列の型の定義には enum が要る");
            return values;
        };
        for item in items {
            match item {
                Value::String(value) if is_snake(value) && !values.contains(value) => {
                    values.push(value.clone());
                }
                _ => self.error(
                    at,
                    "enum の値は、重ならない英小文字・数字・_ の文字列にする",
                ),
            }
        }
        if values.is_empty() {
            self.error(at, "enum に値が要る");
        }
        values
    }

    fn one_of(&mut self, members: &Value, at: &str) -> Option<Shape> {
        let Value::Array(items) = members else {
            self.error(at, "oneOf は配列にする");
            return None;
        };
        let references: Vec<&Value> = items
            .iter()
            .filter_map(|item| match item {
                Value::Object(object) if object.len() == 1 => object.get("$ref"),
                _ => None,
            })
            .collect();
        if !items.is_empty() && references.len() == items.len() {
            let variants = references
                .into_iter()
                .filter_map(|reference| self.reference(reference, at))
                .collect();
            return Some(Shape::Union { variants });
        }
        let mut members = Vec::new();
        for (index, item) in items.iter().enumerate() {
            let member_at = format!("{at}/oneOf/{index}");
            let object = self.object(item, &member_at)?;
            let description = self.description(object, &member_at);
            if let Some(constant) = object.get("const") {
                self.allow(object, &member_at, &["description", "const"]);
                match constant {
                    Value::String(value) if is_snake(value) => members.push(Member::Const {
                        value: value.clone(),
                        description,
                    }),
                    _ => self.error(&member_at, "const は英小文字・数字・_ の文字列にする"),
                }
                continue;
            }
            match object.get("type") {
                Some(Value::String(ty)) if ty == "integer" => {
                    self.allow(
                        object,
                        &member_at,
                        &["description", "type", "minimum", "maximum"],
                    );
                    let min = self
                        .integer(object, "minimum", &member_at)
                        .unwrap_or(-MAX_SAFE);
                    let max = self
                        .integer(object, "maximum", &member_at)
                        .unwrap_or(MAX_SAFE);
                    members.push(Member::Integer {
                        min,
                        max,
                        description,
                    });
                }
                Some(Value::String(ty)) if ty == "number" => {
                    self.allow(object, &member_at, &["description", "type"]);
                    members.push(Member::Number { description });
                }
                Some(Value::String(ty)) if ty == "string" => {
                    self.allow(object, &member_at, &["description", "type"]);
                    members.push(Member::String { description });
                }
                _ => self.error(
                    &member_at,
                    "スカラーの共用体の形は integer・number・string・文字列の const にする",
                ),
            }
        }
        let numbers = members
            .iter()
            .filter(|m| matches!(m, Member::Integer { .. } | Member::Number { .. }))
            .count();
        let strings = members
            .iter()
            .filter(|m| matches!(m, Member::String { .. }))
            .count();
        let constants: Vec<String> = members
            .iter()
            .filter_map(|m| match m {
                Member::Const { value, .. } => Some(value.clone()),
                _ => None,
            })
            .collect();
        let unique: BTreeSet<&String> = constants.iter().collect();
        if numbers > 1
            || strings > 1
            || (strings == 1 && !constants.is_empty())
            || unique.len() != constants.len()
            || members.len() < 2
        {
            self.error(
                at,
                "スカラーの共用体の形は、JSON の値から 1 つに決まるようにする",
            );
        }
        Some(Shape::Scalars { members })
    }

    fn object_def(&mut self, object: &Object, at: &str) -> Shape {
        self.allow(
            object,
            at,
            &[
                "description",
                "type",
                "properties",
                "required",
                "additionalProperties",
                "allOf",
                "unevaluatedProperties",
            ],
        );
        let mut compose = Vec::new();
        let closed = if let Some(all_of) = object.get("allOf") {
            match all_of {
                Value::Array(items) => {
                    for item in items {
                        match item {
                            Value::Object(member) if member.len() == 1 => {
                                if let Some(name) = member
                                    .get("$ref")
                                    .and_then(|reference| self.reference(reference, at))
                                {
                                    compose.push(name);
                                }
                            }
                            _ => self.error(at, "allOf の中は {\"$ref\": ...} だけにする"),
                        }
                    }
                }
                _ => self.error(at, "allOf は配列にする"),
            }
            if object.get("unevaluatedProperties") != Some(&Value::Bool(false))
                || object.get("additionalProperties").is_some()
            {
                self.error(
                    at,
                    "allOf を使うときは unevaluatedProperties: false にし、additionalProperties を書かない",
                );
            }
            true
        } else {
            if object.get("unevaluatedProperties").is_some() {
                self.error(at, "unevaluatedProperties は allOf と一緒にだけ使う");
            }
            match object.get("additionalProperties") {
                Some(Value::Bool(false)) => true,
                None => false,
                Some(_) => {
                    self.error(
                        at,
                        "additionalProperties は false にする（名前から値への表は、欄の型にだけ使う）",
                    );
                    true
                }
            }
        };

        let required: Vec<String> = match object.get("required") {
            Some(Value::Array(items)) => items
                .iter()
                .filter_map(|item| match item {
                    Value::String(name) => Some(name.clone()),
                    _ => None,
                })
                .collect(),
            Some(_) => {
                self.error(at, "required は文字列の配列にする");
                Vec::new()
            }
            None => Vec::new(),
        };

        let mut fields = Vec::new();
        match object.get("properties") {
            Some(Value::Object(properties)) => {
                for (name, schema) in properties.iter() {
                    let field_at = format!("{at}/properties/{name}");
                    if !is_snake(name) || KEYWORDS.contains(&name) {
                        self.error(
                            &field_at,
                            "欄の名前は英小文字・数字・_ にし、Rust の予約語にしない",
                        );
                    }
                    let Some(field) = self.object(schema, &field_at) else {
                        continue;
                    };
                    let description = self.description(field, &field_at);
                    if let Some(ty) = self.ty(field, &field_at) {
                        fields.push(Field {
                            name: name.to_string(),
                            description,
                            ty,
                            required: required.iter().any(|r| r == name),
                        });
                    }
                }
            }
            Some(_) => self.error(at, "properties はオブジェクトにする"),
            None => {}
        }
        for name in &required {
            if !fields.iter().any(|field| &field.name == name) {
                self.error(at, format!("required の `{name}` が properties にない"));
            }
        }
        Shape::Object {
            fields,
            compose,
            closed,
        }
    }

    /// 欄の型。オブジェクト・列挙・共用体は、$defs に名前をつけて $ref で指す
    fn ty(&mut self, object: &Object, at: &str) -> Option<Ty> {
        if let Some(reference) = object.get("$ref") {
            self.allow(object, at, &["description", "$ref"]);
            return self.reference(reference, at).map(Ty::Ref);
        }
        if let Some(constant) = object.get("const") {
            self.allow(object, at, &["description", "const"]);
            return match constant {
                Value::String(value) if is_snake(value) => Some(Ty::Const(value.clone())),
                _ => {
                    self.error(at, "const は英小文字・数字・_ の文字列にする");
                    None
                }
            };
        }
        let ty = match object.get("type") {
            Some(Value::String(ty)) => ty.as_str(),
            _ => {
                self.error(at, "type か $ref が要る");
                return None;
            }
        };
        match ty {
            "string" | "boolean" | "number" => {
                self.allow(object, at, &["description", "type"]);
                Some(match ty {
                    "string" => Ty::String,
                    "boolean" => Ty::Bool,
                    _ => Ty::Float,
                })
            }
            "integer" => {
                self.allow(object, at, &["description", "type", "minimum", "maximum"]);
                let min = self.integer(object, "minimum", at).unwrap_or(-MAX_SAFE);
                let max = self.integer(object, "maximum", at).unwrap_or(MAX_SAFE);
                if min > max {
                    self.error(at, "minimum が maximum より大きい");
                }
                Some(match (u64::try_from(min), u64::try_from(max)) {
                    (Ok(min), Ok(max)) => Ty::UInt { min, max },
                    _ => Ty::Int { min, max },
                })
            }
            "array" => {
                self.allow(object, at, &["description", "type", "items"]);
                let items = object.get("items").and_then(|items| {
                    let items_at = format!("{at}/items");
                    let items = self.object(items, &items_at)?;
                    self.ty(items, &items_at)
                });
                if items.is_none() {
                    self.error(at, "配列には items が要る");
                }
                items.map(|items| Ty::Array(Box::new(items)))
            }
            "object" => {
                self.allow(object, at, &["description", "type", "additionalProperties"]);
                let values = object
                    .get("additionalProperties")
                    .and_then(|values| match values {
                        Value::Object(_) => {
                            let values_at = format!("{at}/additionalProperties");
                            let values = self.object(values, &values_at)?;
                            self.ty(values, &values_at)
                        }
                        _ => None,
                    });
                if values.is_none() {
                    self.error(
                        at,
                        "欄のオブジェクトは、名前から値への表（additionalProperties に値の型）にだけ使う。ほかは $defs に置く",
                    );
                }
                values.map(|values| Ty::Map(Box::new(values)))
            }
            _ => {
                self.error(at, format!("type `{ty}` は使えない"));
                None
            }
        }
    }
}

/// スキーマを読み、決めた書き方に合うかを確かめる。
pub fn parse(source: &[u8]) -> Result<Schema, Vec<String>> {
    let root = zumen_json::parse(source)
        .map_err(|error| vec![format!("JSON として読めない（{} バイト目）", error.offset)])?;
    let mut reader = Reader { errors: Vec::new() };
    let Some(object) = reader.object(&root, "/") else {
        return Err(reader.errors);
    };
    reader.allow(
        object,
        "/",
        &["$schema", "$id", "title", "description", "$defs"],
    );
    if object.get("$schema") != Some(&Value::String(DIALECT.to_string())) {
        reader.error("/", format!("$schema は {DIALECT} にする"));
    }
    let mut defs = Vec::new();
    match object.get("$defs") {
        Some(Value::Object(entries)) => {
            for (name, value) in entries.iter() {
                if let Some(def) = reader.def(name, value) {
                    defs.push(def);
                }
            }
        }
        _ => reader.error("/", "$defs が要る"),
    }
    let schema = Schema { defs };
    let mut errors = reader.errors;
    errors.extend(validate(&schema));
    if errors.is_empty() {
        Ok(schema)
    } else {
        Err(errors)
    }
}

impl Schema {
    pub(crate) fn def(&self, name: &str) -> Option<&Def> {
        self.defs.iter().find(|def| def.name == name)
    }

    /// 共用体の形のオブジェクトから、目印の欄の名前と値を引く
    pub(crate) fn tag_of(&self, variant: &str) -> Option<(&str, &str)> {
        match &self.def(variant)?.shape {
            Shape::Object { fields, .. } => fields.iter().find_map(|field| match &field.ty {
                Ty::Const(value) => Some((field.name.as_str(), value.as_str())),
                _ => None,
            }),
            _ => None,
        }
    }

    /// 共用体の形として使われる定義の名前
    fn variants(&self) -> BTreeSet<&str> {
        self.defs
            .iter()
            .filter_map(|def| match &def.shape {
                Shape::Union { variants } => Some(variants.iter().map(String::as_str)),
                _ => None,
            })
            .flatten()
            .collect()
    }
}

fn references(ty: &Ty, out: &mut Vec<String>) {
    match ty {
        Ty::Ref(name) => out.push(name.clone()),
        Ty::Array(items) | Ty::Map(items) => references(items, out),
        _ => {}
    }
}

/// 定義の間のつながりを確かめる
fn validate(schema: &Schema) -> Vec<String> {
    let mut errors = Vec::new();
    let variants = schema.variants();
    for def in &schema.defs {
        let at = format!("$defs/{}", def.name);
        match &def.shape {
            Shape::Object {
                fields,
                compose,
                closed,
            } => {
                for field in fields {
                    let mut targets = Vec::new();
                    references(&field.ty, &mut targets);
                    for target in targets {
                        match schema.def(&target).map(|def| &def.shape) {
                            None => errors.push(format!("{at}/properties/{}: `{target}` がない", field.name)),
                            Some(Shape::Handwritten) => errors.push(format!(
                                "{at}/properties/{}: `{target}` は手で書く定義なので、指せない",
                                field.name
                            )),
                            Some(Shape::Object { closed: false, .. }) => errors.push(format!(
                                "{at}/properties/{}: `{target}` は allOf で取り込むだけの定義なので、指せない",
                                field.name
                            )),
                            Some(_) => {}
                        }
                    }
                }
                let constants = fields
                    .iter()
                    .filter(|field| matches!(field.ty, Ty::Const(_)))
                    .count();
                let is_variant = variants.contains(def.name.as_str());
                if is_variant && (constants != 1 || !closed) {
                    errors.push(format!(
                        "{at}: 共用体の形は、const の欄を 1 つだけ持つ閉じたオブジェクトにする"
                    ));
                }
                if !is_variant && constants > 0 {
                    errors.push(format!("{at}: const の欄は、共用体の形にだけ置く"));
                }
                if fields
                    .iter()
                    .any(|field| matches!(field.ty, Ty::Const(_)) && !field.required)
                {
                    errors.push(format!("{at}: const の欄は required にする"));
                }
                let mut names: BTreeSet<&str> = fields.iter().map(|f| f.name.as_str()).collect();
                if names.len() != fields.len() {
                    errors.push(format!("{at}: 欄の名前が重なっている"));
                }
                for target in compose {
                    match schema.def(target).map(|def| &def.shape) {
                        Some(Shape::Object {
                            closed: false,
                            compose: inner,
                            fields: composed,
                        }) => {
                            if !inner.is_empty() {
                                errors.push(format!("{at}: 取り込む `{target}` は、さらに取り込まない"));
                            }
                            if !names.insert(target.as_str())
                                || composed.iter().any(|f| names.contains(f.name.as_str()))
                            {
                                errors.push(format!("{at}: `{target}` の欄の名前が重なっている"));
                            }
                        }
                        _ => errors.push(format!(
                            "{at}: allOf の `{target}` は、additionalProperties を書かないオブジェクトにする"
                        )),
                    }
                }
            }
            Shape::Union { variants: members } => {
                let mut tags = BTreeMap::new();
                let mut tag_names = BTreeSet::new();
                for variant in members {
                    match schema.tag_of(variant) {
                        Some((name, value)) => {
                            tag_names.insert(name);
                            if tags.insert(value, variant).is_some() {
                                errors.push(format!("{at}: 目印の値 `{value}` が重なっている"));
                            }
                        }
                        None => errors.push(format!(
                            "{at}: `{variant}` は、const の欄を持つオブジェクトにする"
                        )),
                    }
                }
                if tag_names.len() > 1 {
                    errors.push(format!("{at}: 目印の欄の名前をそろえる"));
                }
            }
            Shape::Enum { .. } | Shape::Scalars { .. } | Shape::Handwritten => {}
        }
    }
    for name in HANDWRITTEN {
        if schema.def(name).is_none() {
            errors.push(format!("$defs/{name}: 手で書く定義も、スキーマに書く"));
        }
    }
    errors
}

fn rust_type(ty: &Ty) -> String {
    match ty {
        Ty::String | Ty::Const(_) => "String".to_string(),
        Ty::Bool => "bool".to_string(),
        Ty::UInt { .. } => "u64".to_string(),
        Ty::Int { .. } => "i64".to_string(),
        Ty::Float => "f64".to_string(),
        Ty::Ref(name) => pascal(name),
        Ty::Array(items) => format!("Vec<{}>", rust_type(items)),
        Ty::Map(values) => format!("BTreeMap<String, {}>", rust_type(values)),
    }
}

/// 値を読む式（`fn(&Value) -> Result<T, Invalid>` になるもの）
fn reader(ty: &Ty) -> String {
    match ty {
        Ty::String | Ty::Const(_) => "json::string".to_string(),
        Ty::Bool => "json::boolean".to_string(),
        Ty::UInt { min, max } => format!("|value| json::uint(value, {min}, {max})"),
        Ty::Int { min, max } => format!("|value| json::int(value, {min}, {max})"),
        Ty::Float => "json::number".to_string(),
        Ty::Ref(name) => format!("{}::from_json", pascal(name)),
        Ty::Array(items) => format!("|value| json::array(value, {})", reader(items)),
        Ty::Map(values) => format!("|value| json::map(value, {})", reader(values)),
    }
}

/// 値を書く式。`place` は、`by_ref` なら値への参照の式（`value` など）、そうでなければ欄（`self.v` など）
fn writer(ty: &Ty, place: &str, by_ref: bool) -> String {
    let (copied, borrowed) = if by_ref {
        (format!("*{place}"), place.to_string())
    } else {
        (place.to_string(), format!("&{place}"))
    };
    match ty {
        Ty::String | Ty::Const(_) => format!("out.string({borrowed})"),
        Ty::Bool => format!("out.bool({copied})"),
        Ty::UInt { .. } => format!("out.uint({copied})"),
        Ty::Int { .. } => format!("out.int({copied})"),
        Ty::Float => format!("out.float({copied})"),
        Ty::Ref(_) => format!("{place}.to_json(out)"),
        Ty::Array(items) => format!(
            "out.array({borrowed}, |out, item| {})",
            writer(items, "item", true)
        ),
        Ty::Map(values) => format!(
            "out.map({borrowed}, |out, item| {})",
            writer(values, "item", true)
        ),
    }
}

fn doc(out: &mut String, indent: &str, text: &str) {
    for line in text.lines() {
        let _ = writeln!(out, "{indent}/// {line}");
    }
}

fn uses_map(ty: &Ty) -> bool {
    match ty {
        Ty::Map(_) => true,
        Ty::Array(items) => uses_map(items),
        _ => false,
    }
}

/// `crates/zumen-abi/src/messages.rs` の中身を作る。rustfmt で整える前のもの。
pub fn render(schema: &Schema) -> String {
    let mut out = String::new();
    out.push_str("// 生成したファイル。手で直さない。\n");
    out.push_str("// 元は schema/messages.json で、`cargo xtask codegen` で作り直す。\n\n");
    out.push_str(
        "//! 境界のメッセージの型（決定 0012）。スキーマの定義ごとに 1 つの型がある。\n\n",
    );
    let map = schema.defs.iter().any(|def| match &def.shape {
        Shape::Object { fields, .. } => fields.iter().any(|field| uses_map(&field.ty)),
        _ => false,
    });
    if map {
        out.push_str("use std::collections::BTreeMap;\n\n");
    }
    out.push_str(
        "use zumen_json::{self as json, FromJson, Invalid, ObjectReader, Problem, ToJson, Value, Writer};\n\n",
    );
    for def in &schema.defs {
        match &def.shape {
            Shape::Object {
                fields,
                compose,
                closed,
            } => render_object(&mut out, def, fields, compose, *closed),
            Shape::Enum { values } => render_enum(&mut out, def, values),
            Shape::Union { variants } => render_union(&mut out, schema, def, variants),
            Shape::Scalars { members } => render_scalars(&mut out, def, members),
            Shape::Handwritten => {}
        }
    }
    out
}

fn render_object(out: &mut String, def: &Def, fields: &[Field], compose: &[String], closed: bool) {
    let name = pascal(&def.name);
    let data: Vec<&Field> = fields
        .iter()
        .filter(|field| !matches!(field.ty, Ty::Const(_)))
        .collect();
    doc(out, "", &def.description);
    let default = compose.is_empty() && data.iter().all(|field| !field.required);
    let _ = writeln!(
        out,
        "#[derive(Clone, Debug{}, PartialEq)]\npub struct {name} {{",
        if default { ", Default" } else { "" }
    );
    for target in compose {
        let _ = writeln!(
            out,
            "    /// `{target}` の欄\n    pub {target}: {},",
            pascal(target)
        );
    }
    for field in &data {
        doc(out, "    ", &field.description);
        let ty = rust_type(&field.ty);
        if field.required {
            let _ = writeln!(out, "    pub {}: {ty},", field.name);
        } else {
            let _ = writeln!(out, "    pub {}: Option<{ty}>,", field.name);
        }
    }
    out.push_str("}\n\n");

    let _ = writeln!(out, "impl {name} {{");
    let object = if compose.is_empty() && fields.is_empty() {
        "_object"
    } else {
        "object"
    };
    let _ = writeln!(
        out,
        "    fn read_fields({object}: &mut ObjectReader<'_>) -> Result<Self, Invalid> {{"
    );
    for target in compose {
        let _ = writeln!(
            out,
            "        let {target} = {}::read_fields(object)?;",
            pascal(target)
        );
    }
    for field in fields {
        match &field.ty {
            Ty::Const(value) => {
                let _ = writeln!(
                    out,
                    "        object.constant(\"{}\", \"{value}\")?;",
                    field.name
                );
            }
            ty => {
                let method = if field.required {
                    "required"
                } else {
                    "optional"
                };
                let _ = writeln!(
                    out,
                    "        let {} = object.{method}(\"{}\", {})?;",
                    field.name,
                    field.name,
                    reader(ty)
                );
            }
        }
    }
    let names: Vec<&str> = compose
        .iter()
        .map(String::as_str)
        .chain(data.iter().map(|field| field.name.as_str()))
        .collect();
    let _ = writeln!(out, "        Ok(Self {{ {} }})\n    }}\n", names.join(", "));
    let out_name = if compose.is_empty() && fields.is_empty() {
        "_out"
    } else {
        "out"
    };
    let _ = writeln!(
        out,
        "    fn write_fields(&self, {out_name}: &mut Writer) {{"
    );
    for target in compose {
        let _ = writeln!(out, "        self.{target}.write_fields(out);");
    }
    for field in fields {
        match &field.ty {
            Ty::Const(value) => {
                let _ = writeln!(
                    out,
                    "        out.key(\"{}\");\n        out.string(\"{value}\");",
                    field.name
                );
            }
            ty if field.required => {
                let _ = writeln!(
                    out,
                    "        out.key(\"{}\");\n        {};",
                    field.name,
                    writer(ty, &format!("self.{}", field.name), false)
                );
            }
            ty => {
                let _ = writeln!(
                    out,
                    "        if let Some(value) = &self.{} {{\n            out.key(\"{}\");\n            {};\n        }}",
                    field.name,
                    field.name,
                    writer(ty, "value", true)
                );
            }
        }
    }
    out.push_str("    }\n}\n\n");

    if closed {
        let _ = writeln!(
            out,
            "impl FromJson for {name} {{\n    fn from_json(value: &Value) -> Result<Self, Invalid> {{\n        let mut object = ObjectReader::new(value)?;\n        let this = Self::read_fields(&mut object)?;\n        object.finish()?;\n        Ok(this)\n    }}\n}}\n"
        );
        let _ = writeln!(
            out,
            "impl ToJson for {name} {{\n    fn to_json(&self, out: &mut Writer) {{\n        out.begin_object();\n        self.write_fields(out);\n        out.end_object();\n    }}\n}}\n"
        );
    }
}

fn render_enum(out: &mut String, def: &Def, values: &[String]) {
    let name = pascal(&def.name);
    doc(out, "", &def.description);
    let _ = writeln!(
        out,
        "#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]\npub enum {name} {{"
    );
    for value in values {
        let _ = writeln!(out, "    /// `{value}`\n    {},", pascal(value));
    }
    out.push_str("}\n\n");
    let _ = writeln!(
        out,
        "impl {name} {{\n    /// すべての値（スキーマの順）\n    pub const ALL: &[Self] = &["
    );
    for value in values {
        let _ = writeln!(out, "        Self::{},", pascal(value));
    }
    out.push_str("    ];\n\n    /// JSON での名前\n    pub const fn name(self) -> &'static str {\n        match self {\n");
    for value in values {
        let _ = writeln!(out, "            Self::{} => \"{value}\",", pascal(value));
    }
    out.push_str("        }\n    }\n\n    /// JSON での名前から引く\n    pub fn from_name(name: &str) -> Option<Self> {\n        match name {\n");
    for value in values {
        let _ = writeln!(
            out,
            "            \"{value}\" => Some(Self::{}),",
            pascal(value)
        );
    }
    out.push_str("            _ => None,\n        }\n    }\n}\n\n");
    let _ = writeln!(
        out,
        "impl FromJson for {name} {{\n    fn from_json(value: &Value) -> Result<Self, Invalid> {{\n        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))\n    }}\n}}\n"
    );
    let _ = writeln!(
        out,
        "impl ToJson for {name} {{\n    fn to_json(&self, out: &mut Writer) {{\n        out.string(self.name());\n    }}\n}}\n"
    );
}

fn render_union(out: &mut String, schema: &Schema, def: &Def, variants: &[String]) {
    let name = pascal(&def.name);
    let tags: Vec<(&str, &str, &str)> = variants
        .iter()
        .filter_map(|variant| {
            let (tag, value) = schema.tag_of(variant)?;
            Some((tag, value, variant.as_str()))
        })
        .collect();
    let tag_name = tags.first().map_or("type", |(tag, _, _)| tag);
    doc(out, "", &def.description);
    // 境界のメッセージは 1 度読んで使うだけなので、大きい形を箱に入れず、型をスキーマのとおりに保つ
    let _ = writeln!(
        out,
        "#[derive(Clone, Debug, PartialEq)]\n#[allow(clippy::large_enum_variant)]\npub enum {name} {{"
    );
    for (_, value, variant) in &tags {
        let description = schema
            .def(variant)
            .map_or("", |variant| variant.description.as_str());
        doc(out, "    ", description);
        let _ = writeln!(out, "    {}({}),", pascal(value), pascal(variant));
    }
    out.push_str("}\n\n");
    let _ = writeln!(
        out,
        "impl {name} {{\n    /// 目印の欄（`{tag_name}`）の値（スキーマの順）\n    pub const TAGS: &[&str] = &["
    );
    for (_, value, _) in &tags {
        let _ = writeln!(out, "        \"{value}\",");
    }
    let _ = writeln!(
        out,
        "    ];\n\n    /// 目印の欄（`{tag_name}`）の値\n    pub const fn tag(&self) -> &'static str {{\n        match self {{"
    );
    for (_, value, _) in &tags {
        let _ = writeln!(
            out,
            "            Self::{}(_) => \"{value}\",",
            pascal(value)
        );
    }
    out.push_str("        }\n    }\n}\n\n");
    let _ = writeln!(
        out,
        "impl FromJson for {name} {{\n    fn from_json(value: &Value) -> Result<Self, Invalid> {{\n        let mut object = ObjectReader::new(value)?;\n        let this = match object.tag(\"{tag_name}\")? {{"
    );
    for (_, value, variant) in &tags {
        let _ = writeln!(
            out,
            "            \"{value}\" => Self::{}({}::read_fields(&mut object)?),",
            pascal(value),
            pascal(variant)
        );
    }
    let _ = writeln!(
        out,
        "            other => return Err(Invalid::new(Problem::Tag(other.to_string())).in_key(\"{tag_name}\")),\n        }};\n        object.finish()?;\n        Ok(this)\n    }}\n}}\n"
    );
    let _ = writeln!(
        out,
        "impl ToJson for {name} {{\n    fn to_json(&self, out: &mut Writer) {{\n        match self {{"
    );
    for (_, value, _) in &tags {
        let _ = writeln!(
            out,
            "            Self::{}(value) => value.to_json(out),",
            pascal(value)
        );
    }
    out.push_str("        }\n    }\n}\n\n");
}

fn render_scalars(out: &mut String, def: &Def, members: &[Member]) {
    let name = pascal(&def.name);
    doc(out, "", &def.description);
    let _ = writeln!(
        out,
        "#[derive(Clone, Debug, PartialEq)]\npub enum {name} {{"
    );
    for member in members {
        doc(out, "    ", member.description());
        match member {
            Member::Integer { min, .. } if *min >= 0 => {
                let _ = writeln!(out, "    {}(u64),", member.variant());
            }
            Member::Integer { .. } => {
                let _ = writeln!(out, "    {}(i64),", member.variant());
            }
            Member::Number { .. } => {
                let _ = writeln!(out, "    {}(f64),", member.variant());
            }
            Member::String { .. } => {
                let _ = writeln!(out, "    {}(String),", member.variant());
            }
            Member::Const { .. } => {
                let _ = writeln!(out, "    {},", member.variant());
            }
        }
    }
    out.push_str("}\n\n");

    let _ = writeln!(
        out,
        "impl FromJson for {name} {{\n    fn from_json(value: &Value) -> Result<Self, Invalid> {{\n        match value {{"
    );
    for member in members {
        match member {
            Member::Integer { min, max, .. } if *min >= 0 => {
                let _ = writeln!(
                    out,
                    "            Value::Number(_) => json::uint(value, {min}, {max}).map(Self::{}),",
                    member.variant()
                );
            }
            Member::Integer { min, max, .. } => {
                let _ = writeln!(
                    out,
                    "            Value::Number(_) => json::int(value, {min}, {max}).map(Self::{}),",
                    member.variant()
                );
            }
            Member::Number { .. } => {
                let _ = writeln!(
                    out,
                    "            Value::Number(_) => json::number(value).map(Self::{}),",
                    member.variant()
                );
            }
            Member::String { .. } => {
                let _ = writeln!(
                    out,
                    "            Value::String(text) => Ok(Self::{}(text.clone())),",
                    member.variant()
                );
            }
            Member::Const { .. } => {}
        }
    }
    let constants: Vec<&Member> = members
        .iter()
        .filter(|member| matches!(member, Member::Const { .. }))
        .collect();
    if !constants.is_empty() {
        out.push_str("            Value::String(text) => match text.as_str() {\n");
        for member in &constants {
            if let Member::Const { value, .. } = member {
                let _ = writeln!(
                    out,
                    "                \"{value}\" => Ok(Self::{}),",
                    member.variant()
                );
            }
        }
        out.push_str("                _ => Err(Invalid::new(Problem::Value)),\n            },\n");
    }
    out.push_str("            _ => Err(Invalid::new(Problem::Type)),\n        }\n    }\n}\n\n");

    let _ = writeln!(
        out,
        "impl ToJson for {name} {{\n    fn to_json(&self, out: &mut Writer) {{\n        match self {{"
    );
    for member in members {
        let variant = member.variant();
        let _ = match member {
            Member::Integer { min, .. } if *min >= 0 => {
                writeln!(
                    out,
                    "            Self::{variant}(value) => out.uint(*value),"
                )
            }
            Member::Integer { .. } => writeln!(
                out,
                "            Self::{variant}(value) => out.int(*value),"
            ),
            Member::Number { .. } => writeln!(
                out,
                "            Self::{variant}(value) => out.float(*value),"
            ),
            Member::String { .. } => writeln!(
                out,
                "            Self::{variant}(value) => out.string(value),"
            ),
            Member::Const { value, .. } => {
                writeln!(
                    out,
                    "            Self::{variant} => out.string(\"{value}\"),"
                )
            }
        };
    }
    out.push_str("        }\n    }\n}\n\n");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn schema(defs: &str) -> Result<Schema, Vec<String>> {
        let source = format!(
            r#"{{"$schema": "https://json-schema.org/draft/2020-12/schema", "$defs": {{{defs}, "response": {{"description": "応答", "type": "object"}}}}}}"#
        );
        parse(source.as_bytes())
    }

    const REQUEST: &str = r##"
        "request": {"description": "要求", "oneOf": [{"$ref": "#/$defs/a_request"}]},
        "common": {"description": "共通", "type": "object", "properties": {"v": {"description": "版", "type": "integer", "minimum": 1}}, "required": ["v"]},
        "a_request": {"description": "A", "type": "object", "allOf": [{"$ref": "#/$defs/common"}],
            "properties": {"op": {"description": "op", "const": "a"}, "names": {"description": "名前", "type": "array", "items": {"type": "string"}}},
            "required": ["op"], "unevaluatedProperties": false},
        "kind": {"description": "種類", "type": "string", "enum": ["one", "two_three"]},
        "limit": {"description": "上限", "oneOf": [{"description": "数", "type": "integer", "minimum": 0}, {"description": "なし", "const": "unlimited"}]},
        "table": {"description": "表", "type": "object", "properties": {"values": {"description": "値", "type": "object", "additionalProperties": {"$ref": "#/$defs/limit"}}}, "additionalProperties": false}
    "##;

    #[test]
    fn reads_every_supported_shape() {
        let schema = schema(REQUEST).unwrap();
        let names: Vec<&str> = schema.defs.iter().map(|def| def.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "request",
                "common",
                "a_request",
                "kind",
                "limit",
                "table",
                "response"
            ]
        );
        assert_eq!(schema.tag_of("a_request"), Some(("op", "a")));
        let source = render(&schema);
        assert!(source.contains("pub struct ARequest {"));
        assert!(source.contains("pub common: Common,"));
        assert!(source.contains("pub names: Option<Vec<String>>,"));
        assert!(source.contains("\"a\" => Self::A(ARequest::read_fields(&mut object)?),"));
        assert!(source.contains("TwoThree,"));
        assert!(source.contains("\"unlimited\" => Ok(Self::Unlimited),"));
        assert!(source.contains("pub values: Option<BTreeMap<String, Limit>>,"));
        assert!(!source.contains("pub struct Response"));
        assert!(!source.contains("impl FromJson for Common"));
    }

    #[test]
    fn rejects_keywords_outside_the_subset() {
        let errors = schema(
            r##""a": {"description": "A", "type": "object", "properties": {"x": {"description": "x", "type": "string", "pattern": "^a"}}, "additionalProperties": false}"##,
        )
        .unwrap_err();
        assert!(errors.iter().any(|e| e.contains("`pattern`")), "{errors:?}");
    }

    #[test]
    fn rejects_inline_objects_missing_descriptions_and_bad_references() {
        let errors = schema(
            r##""a": {"description": "A", "type": "object", "properties": {
                "inline": {"description": "x", "type": "object", "properties": {}},
                "nodoc": {"type": "string"},
                "missing": {"description": "x", "$ref": "#/$defs/nothing"},
                "Bad": {"description": "x", "type": "boolean"}
            }, "additionalProperties": false}"##,
        )
        .unwrap_err();
        // inline は、使えない properties と、表でないことの 2 つ
        assert_eq!(errors.len(), 5, "{errors:?}");
    }

    #[test]
    fn rejects_ambiguous_scalars_and_untagged_unions() {
        let errors = schema(
            r##""s": {"description": "S", "oneOf": [{"description": "a", "type": "integer"}, {"description": "b", "type": "number"}]},
               "o": {"description": "O", "type": "object", "properties": {}, "additionalProperties": false},
               "u": {"description": "U", "oneOf": [{"$ref": "#/$defs/o"}]}"##,
        )
        .unwrap_err();
        // s は形が 1 つに決まらない。u の形の o は目印がなく、o の側でも同じことを返す
        assert_eq!(errors.len(), 3, "{errors:?}");
    }

    #[test]
    fn requires_unevaluated_properties_with_all_of() {
        let errors = schema(
            r##""c": {"description": "C", "type": "object", "properties": {}},
               "a": {"description": "A", "type": "object", "allOf": [{"$ref": "#/$defs/c"}], "properties": {}, "additionalProperties": false}"##,
        )
        .unwrap_err();
        assert!(
            errors.iter().any(|e| e.contains("unevaluatedProperties")),
            "{errors:?}"
        );
    }
}
