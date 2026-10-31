//! Swift の包み（`bindings/swift`）の型と定数を作る（決定 0011・0225）。
//!
//! - `Messages.swift`: メッセージのスキーマ（`schema/messages.json`）の型
//! - `Constants.swift`: 境界の版・状態コード（`schema/boundary.toml`）と、シーンの配置（`schema/scene.toml`）
//!
//! 型は、読んだ JSON のオブジェクト（`[String: JSON]`）を包む見方の構造体で、知らない欄と値もそのまま持つ（決定 0015）。

use std::collections::BTreeSet;
use std::fmt::Write;

use crate::boundary::Boundary;
use crate::scene::Scene;
use crate::schema::{Def, Field, Schema, Shape, Ty, pascal};

/// op の共用体の定義の名前
const REQUEST: &str = "request";

/// 手で書く応答の定義の名前
const RESPONSE: &str = "response";

/// 生成したコードと包みが使う型の名前。定義の名前と重ならないようにする
const RESERVED: &[&str] = &[
    "String",
    "Bool",
    "Int64",
    "Double",
    "Data",
    "Array",
    "Dictionary",
    "Optional",
    "Error",
    "JSON",
    "JSONRepresentable",
    "Message",
    "Operations",
    "Constants",
    "Session",
    "Scene",
    "Zumen",
    "ZumenError",
];

/// 見方の構造体が持つメンバー。欄の名前と重なれば、後ろに `Field` を付ける
const MEMBERS: &[&str] = &[
    "json",
    "jsonValue",
    "jsonText",
    "readField",
    "writeField",
    "taggedView",
    "with",
];

/// 識別子にするときに、バッククォートで囲む語
const KEYWORDS: &[&str] = &[
    "Any",
    "Self",
    "Type",
    "as",
    "associatedtype",
    "async",
    "await",
    "break",
    "case",
    "catch",
    "class",
    "continue",
    "default",
    "defer",
    "deinit",
    "do",
    "else",
    "enum",
    "extension",
    "fallthrough",
    "false",
    "fileprivate",
    "for",
    "func",
    "guard",
    "if",
    "import",
    "in",
    "init",
    "inout",
    "internal",
    "is",
    "let",
    "nil",
    "open",
    "operator",
    "package",
    "private",
    "protocol",
    "public",
    "repeat",
    "rethrows",
    "return",
    "self",
    "static",
    "struct",
    "subscript",
    "super",
    "switch",
    "throw",
    "throws",
    "true",
    "try",
    "typealias",
    "var",
    "where",
    "while",
];

/// 区画の値の型と、`Constants.ColumnType` の場合
const COLUMNS: &[&str] = &["u8", "u32", "f32", "f64"];

/// 説明をドキュメントのコメントにする
fn doc(out: &mut String, indent: &str, text: &str) {
    for line in text.lines() {
        if line.is_empty() {
            let _ = writeln!(out, "{indent}///");
        } else {
            let _ = writeln!(out, "{indent}/// {line}");
        }
    }
}

/// 文字列を Swift の文字列の式にする
fn quote(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if u32::from(c) < 0x20 => {
                let _ = write!(out, "\\u{{{:x}}}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// 名前を、先頭が小文字のキャメルケースの識別子にする（`from_unit` → `fromUnit`）
fn camel(name: &str) -> String {
    let mut out = String::new();
    for (index, part) in name
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|part| !part.is_empty())
        .enumerate()
    {
        let mut chars = part.chars();
        if let Some(first) = chars.next() {
            if index == 0 {
                out.push(first.to_ascii_lowercase());
            } else {
                out.push(first.to_ascii_uppercase());
            }
            out.extend(chars);
        }
    }
    if !out.starts_with(|c: char| c.is_ascii_alphabetic()) {
        out.insert_str(0, "value");
    }
    out
}

/// 識別子が語と重なれば、バッククォートで囲む
fn ident(name: String) -> String {
    if KEYWORDS.contains(&name.as_str()) {
        format!("`{name}`")
    } else {
        name
    }
}

/// 欄の名前を、プロパティの名前にする
fn member(name: &str) -> String {
    let name = camel(name);
    if MEMBERS.contains(&name.as_str()) {
        format!("{name}Field")
    } else {
        ident(name)
    }
}

/// 欄の型の、Swift の型
fn swift_type(schema: &Schema, ty: &Ty) -> String {
    match ty {
        Ty::String | Ty::Const(_) => "String".to_string(),
        Ty::Bool => "Bool".to_string(),
        Ty::UInt { .. } | Ty::Int { .. } => "Int64".to_string(),
        Ty::Float => "Double".to_string(),
        Ty::Ref(name) => match schema.def(name).map(|def| &def.shape) {
            Some(Shape::Enum { .. }) => "String".to_string(),
            Some(Shape::Object { .. } | Shape::Union { .. }) => pascal(name),
            Some(Shape::Handwritten) if name == RESPONSE => "Response".to_string(),
            _ => "JSON".to_string(),
        },
        Ty::Array(items) => format!("[{}]", swift_type(schema, items)),
        Ty::Map(values) => format!("[String: {}]", swift_type(schema, values)),
    }
}

/// 取り込む定義（`allOf`）の欄を先に、自分の欄を後に並べる
fn collect_fields<'a>(schema: &'a Schema, def: &'a Def, out: &mut Vec<&'a Field>, depth: usize) {
    let Shape::Object {
        fields, compose, ..
    } = &def.shape
    else {
        return;
    };
    if depth < 8 {
        for name in compose {
            if let Some(parent) = schema.def(name) {
                collect_fields(schema, parent, out, depth + 1);
            }
        }
    }
    for field in fields {
        if !out.iter().any(|seen| seen.name == field.name) {
            out.push(field);
        }
    }
}

/// 見方の構造体の始まり。JSON のオブジェクトを受ける作り方を置く
fn struct_head(out: &mut String, name: &str, description: &str) {
    out.push('\n');
    doc(out, "", description);
    let _ = writeln!(
        out,
        "public struct {name}: Message {{\n    /// 包んでいる JSON のオブジェクト\n    public var json: [String: JSON]\n\n    /// 読んだ JSON のオブジェクトを包む\n    public init(json: [String: JSON]) {{\n        self.json = json\n    }}"
    );
}

/// 欄ごとの読み書きのプロパティ。ない欄と形の違う値は nil で、nil を書くと欄を除く
fn render_field(out: &mut String, name: &str, description: &str, ty: &str) {
    out.push('\n');
    doc(out, "    ", description);
    let key = quote(name);
    let _ = writeln!(
        out,
        "    public var {}: {ty}? {{\n        get {{ readField({key}) }}\n        set {{ writeField({key}, newValue) }}\n    }}",
        member(name)
    );
}

/// 名前の重なりを調べる
fn check_unique(
    what: &str,
    owner: &str,
    names: impl IntoIterator<Item = String>,
) -> Result<(), String> {
    let mut seen = BTreeSet::new();
    for name in names {
        if !seen.insert(name.clone()) {
            return Err(format!("{owner} の{what}が、Swift の名前 {name} で重なる"));
        }
    }
    Ok(())
}

/// オブジェクトの定義の構造体
fn render_object(out: &mut String, schema: &Schema, def: &Def) -> Result<(), String> {
    let name = pascal(&def.name);
    let mut fields = Vec::new();
    collect_fields(schema, def, &mut fields, 0);
    check_unique(
        "欄",
        &def.name,
        fields.iter().map(|field| member(&field.name)),
    )?;
    struct_head(out, &name, &def.description);
    let consts: Vec<String> = fields
        .iter()
        .filter_map(|field| match &field.ty {
            Ty::Const(value) => Some(format!("{}: .string({})", quote(&field.name), quote(value))),
            _ => None,
        })
        .collect();
    if consts.is_empty() {
        out.push_str(
            "\n    /// 空のオブジェクトから作る\n    public init() {\n        json = [:]\n    }\n",
        );
    } else {
        let _ = writeln!(
            out,
            "\n    /// 目印の欄だけを持つオブジェクトから作る\n    public init() {{\n        json = [{}]\n    }}",
            consts.join(", ")
        );
    }
    for field in fields {
        render_field(
            out,
            &field.name,
            &field.description,
            &swift_type(schema, &field.ty),
        );
    }
    out.push_str("}\n");
    Ok(())
}

/// タグつきの共用体の構造体。形ごとの見方に移すプロパティを持つ
fn render_union(out: &mut String, schema: &Schema, def: &Def, variants: &[String]) {
    let name = pascal(&def.name);
    struct_head(out, &name, &def.description);
    for variant in variants {
        let Some((field, value)) = schema.tag_of(variant) else {
            continue;
        };
        let class = pascal(variant);
        let _ = writeln!(
            out,
            "\n    /// {field} が {value} の形なら、その見方。違えば nil\n    public var as{class}: {class}? {{\n        taggedView({}, {})\n    }}",
            quote(field),
            quote(value)
        );
    }
    out.push_str("}\n");
}

/// 手で書く応答の構造体
fn render_response(out: &mut String, schema: &Schema, def: &Def) {
    struct_head(out, "Response", &def.description);
    let fields = [
        (
            "v",
            "メッセージのスキーマの大版",
            Ty::UInt { min: 1, max: 1 },
        ),
        (
            "diagnostics",
            "診断。入力の順",
            Ty::Array(Box::new(Ty::Ref("diagnostic".to_string()))),
        ),
        (
            "blobs",
            "バイナリの結果。take で取り出す。次の call で捨てる",
            Ty::Array(Box::new(Ty::Ref("blob".to_string()))),
        ),
        (
            "result",
            "op ごとの結果（<op>_result の形の JSON のオブジェクト）。失敗したときは nil",
            Ty::Map(Box::new(Ty::Ref(String::new()))),
        ),
    ];
    for (field, description, ty) in &fields {
        render_field(out, field, description, &swift_type(schema, ty));
    }
    out.push_str("\n    /// op ごとの結果を、その op の結果の型で読む（例: `response.result(as: OpenResult.self)`）。失敗したときは nil\n    public func result<T: Message>(as type: T.Type) -> T? {\n        readField(\"result\")\n    }\n}\n");
}

/// `bindings/swift/Sources/Zumen/Messages.swift` の中身を作る
pub fn render_types(schema: &Schema) -> Result<String, String> {
    if let Some(def) = schema
        .defs
        .iter()
        .find(|def| def.name != RESPONSE && RESERVED.contains(&pascal(&def.name).as_str()))
    {
        return Err(format!(
            "schema/messages.json の {} が、Swift の包みが使う型の名前と重なる",
            def.name
        ));
    }
    let mut out = String::new();
    out.push_str("// 生成したファイル。手で直さない。\n");
    out.push_str("// 元は schema/messages.json で、`cargo xtask codegen` で作り直す。\n");
    out.push_str("//\n// 境界のメッセージの型（決定 0012・0225）。スキーマの定義ごとに 1 つの型がある。型は JSON のオブジェクトを包む\n// 見方で、知らない欄もそのまま持つ。列挙は文字列で読み、知らない値も受け取る（決定 0015）。\n");
    for def in &schema.defs {
        match &def.shape {
            Shape::Object { .. } => render_object(&mut out, schema, def)?,
            Shape::Union { variants } => render_union(&mut out, schema, def, variants),
            Shape::Enum { values } => {
                check_unique("値", &def.name, values.iter().map(|value| camel(value)))?;
                out.push('\n');
                doc(
                    &mut out,
                    "",
                    &format!("{}\n\n列挙の値。知らない値も受け取る", def.description),
                );
                let _ = writeln!(out, "public enum {} {{", pascal(&def.name));
                for value in values {
                    let _ = writeln!(
                        out,
                        "    public static let {} = {}",
                        ident(camel(value)),
                        quote(value)
                    );
                }
                out.push_str("}\n");
            }
            Shape::Handwritten if def.name == RESPONSE => render_response(&mut out, schema, def),
            Shape::Scalars { .. } | Shape::Handwritten => {}
        }
    }
    render_operations(&mut out, schema)?;
    Ok(out)
}

/// op の名前
fn render_operations(out: &mut String, schema: &Schema) -> Result<(), String> {
    let Some(Shape::Union { variants }) = schema.def(REQUEST).map(|def| &def.shape) else {
        return Ok(());
    };
    let ops: Vec<&str> = variants
        .iter()
        .filter_map(|variant| schema.tag_of(variant).map(|(_, op)| op))
        .collect();
    check_unique("op", REQUEST, ops.iter().map(|op| camel(op)))?;
    out.push_str("\n/// op の名前。要求の型は `<Op>Request`、結果の型は `<Op>Result`\npublic enum Operations {\n");
    for op in ops {
        let _ = writeln!(
            out,
            "    public static let {} = {}",
            ident(camel(op)),
            quote(op)
        );
    }
    out.push_str("}\n");
    Ok(())
}

/// `bindings/swift/Sources/Zumen/Constants.swift` の中身を作る
pub fn render_constants(
    version: &str,
    boundary: &Boundary,
    scene: &Scene,
) -> Result<String, String> {
    let mut out = String::new();
    out.push_str("// 生成したファイル。手で直さない。\n");
    out.push_str(
        "// 元は schema/boundary.toml と schema/scene.toml で、`cargo xtask codegen` で作り直す。\n",
    );
    out.push_str("\n/// 境界の版・状態コードと、シーンの配置（決定 0012・0137・0225）\npublic enum Constants {\n");
    let versions = &boundary.versions;
    let _ = writeln!(
        out,
        "    /// パッケージの版（決定 0015）\n    public static let version = {}\n\n    /// ABI の大版\n    public static let abiVersion: UInt32 = {}\n\n    /// メッセージのスキーマの大版（要求と応答の v）\n    public static let schemaMajor: Int64 = {}\n\n    /// メッセージのスキーマの小版\n    public static let schemaMinor: Int64 = {}",
        quote(version),
        versions.abi,
        versions.schema_major,
        versions.schema_minor
    );

    check_unique(
        "状態",
        "boundary",
        boundary.status.iter().map(|s| camel(&s.name)),
    )?;
    out.push_str("\n    /// 状態コード\n    public enum Status {\n");
    for status in &boundary.status {
        doc(&mut out, "        ", &status.description);
        let _ = writeln!(
            out,
            "        public static let {}: Int32 = {}\n",
            ident(camel(&status.name)),
            status.value
        );
    }
    out.push_str("        /// 状態コードの名前。知らない値は `unknown(<値>)`\n        public static func name(_ code: Int32) -> String {\n            switch code {\n");
    for status in &boundary.status {
        let _ = writeln!(
            out,
            "            case {}: return {}",
            status.value,
            quote(&status.name)
        );
    }
    out.push_str(
        "            default: return \"unknown(\\(code))\"\n            }\n        }\n    }\n",
    );

    let _ = writeln!(
        out,
        "\n    /// シーンの大版。区画の意味を変えるときに上げる\n    public static let sceneMajor: UInt16 = {}\n\n    /// シーンの小版。区画か値を足すたびに上げる\n    public static let sceneMinor: UInt16 = {}\n\n    /// 番号の「なし」（u32 の最大）\n    public static let sceneNone = UInt32.max",
        scene.versions.major, scene.versions.minor
    );
    out.push_str("\n    /// シーンの区画の値の型\n    public enum ColumnType: Sendable {\n        case u8, u32, f32, f64\n\n        /// 値のバイト数\n        public var size: Int {\n            switch self {\n            case .u8: return 1\n            case .u32, .f32: return 4\n            case .f64: return 8\n            }\n        }\n    }\n");
    out.push_str("\n    /// シーンの区画の定義\n    public struct SceneSection: Sendable {\n        /// 名前\n        public let name: String\n        /// 値の型\n        public let type: ColumnType\n        /// 行ごとの値の数\n        public let width: Int\n    }\n");
    out.push_str("\n    /// シーンの区画の定義。番号の順\n    public static let sceneSections: [SceneSection] = [\n");
    for section in &scene.sections {
        if !COLUMNS.contains(&section.ty.as_str()) {
            return Err(format!(
                "区画 {} の型 {} を読めない",
                section.name, section.ty
            ));
        }
        doc(
            &mut out,
            "        ",
            &format!(
                "{}（{} × {}）",
                section.description, section.ty, section.width
            ),
        );
        let _ = writeln!(
            out,
            "        SceneSection(name: {}, type: .{}, width: {}),",
            quote(&section.name),
            section.ty,
            section.width
        );
    }
    out.push_str("    ]\n");

    out.push_str("\n    /// シーンの列に入れる値の組。組の名前から、値の名前と値の表を引く\n    public static let sceneValues: [String: [String: UInt32]] = [\n");
    for values in &scene.values {
        let _ = writeln!(
            out,
            "        // {}\n        {}: [",
            values.description.lines().next().unwrap_or(""),
            quote(&values.name)
        );
        for item in &values.items {
            let _ = writeln!(out, "            {}: {},", quote(&item.name), item.value);
        }
        out.push_str("        ],\n");
    }
    out.push_str("    ]\n}\n");
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_views_enums_and_operations() {
        let schema = crate::schema::parse(
            r##"{
              "$schema": "https://json-schema.org/draft/2020-12/schema",
              "$defs": {
                "request": { "description": "要求", "oneOf": [{ "$ref": "#/$defs/open_request" }] },
                "request_common": {
                  "description": "共通",
                  "type": "object",
                  "properties": { "v": { "description": "版", "type": "integer", "minimum": 1 } },
                  "required": ["v"]
                },
                "open_request": {
                  "description": "開く",
                  "type": "object",
                  "allOf": [{ "$ref": "#/$defs/request_common" }],
                  "properties": {
                    "op": { "description": "op", "const": "open" },
                    "from_unit": { "description": "元", "$ref": "#/$defs/unit" },
                    "class": { "description": "類", "type": "string" },
                    "json": { "description": "値", "type": "boolean" },
                    "names": { "description": "名前", "type": "array", "items": { "type": "string" } }
                  },
                  "required": ["op"],
                  "unevaluatedProperties": false
                },
                "unit": { "description": "単位", "type": "string", "enum": ["meter", "open"] },
                "response": { "description": "応答" }
              }
            }"##
            .as_bytes(),
        )
        .unwrap();
        let types = render_types(&schema).unwrap();
        assert!(types.contains("public struct OpenRequest: Message {"));
        assert!(types.contains("        json = [\"op\": .string(\"open\")]"));
        assert!(types.contains("    public var v: Int64? {\n        get { readField(\"v\") }"));
        assert!(types.contains("    public var fromUnit: String? {"));
        assert!(types.contains("    public var `class`: String? {"));
        assert!(types.contains("    public var jsonField: Bool? {"));
        assert!(types.contains("    public var names: [String]? {"));
        assert!(types.contains("    public static let `open` = \"open\""));
        assert!(types.contains("    public var asOpenRequest: OpenRequest? {"));
        assert!(
            types.contains("public enum Operations {\n    public static let `open` = \"open\"")
        );
        assert!(types.contains("public struct Response: Message {"));
        assert!(types.contains("    public var result: [String: JSON]? {"));
    }

    #[test]
    fn rejects_names_that_meet_in_swift() {
        assert_eq!(camel("bcf_read"), "bcfRead");
        assert_eq!(camel("2d"), "value2d");
        assert!(check_unique("値", "x", ["a".to_string(), "a".to_string()]).is_err());
    }
}
