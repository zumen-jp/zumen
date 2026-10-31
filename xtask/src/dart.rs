//! Dart の包み（`bindings/dart`）の型と定数を作る（決定 0011・0227）。
//!
//! - `messages.dart`: メッセージのスキーマ（`schema/messages.json`）の型
//! - `constants.dart`: 境界の版・状態コード（`schema/boundary.toml`）と、シーンの配置（`schema/scene.toml`）
//!
//! 型は、読んだ JSON のオブジェクト（`Map<String, Object?>`）を包む見方のクラスで、知らない欄と値もそのまま持つ（決定 0015）。

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
    "Object",
    "List",
    "Map",
    "Function",
    "Message",
    "Operations",
    "Constants",
    "Status",
    "ColumnType",
    "SceneSection",
    "Zumen",
    "Session",
    "Scene",
    "ZumenException",
];

/// 見方のクラスが持つメンバー。欄の名前と重なれば、後ろに `Field` を付ける
const MEMBERS: &[&str] = &[
    "json",
    "readField",
    "writeField",
    "taggedView",
    "toJson",
    "toString",
    "hashCode",
    "runtimeType",
    "noSuchMethod",
    "resultAs",
];

/// 識別子にできないか、メンバーの名前にすると紛らわしい語。後ろに `Field`（列挙の値は `Value`）を付ける
const KEYWORDS: &[&str] = &[
    "abstract",
    "as",
    "assert",
    "async",
    "await",
    "base",
    "break",
    "case",
    "catch",
    "class",
    "const",
    "continue",
    "covariant",
    "default",
    "deferred",
    "do",
    "dynamic",
    "else",
    "enum",
    "export",
    "extends",
    "extension",
    "external",
    "factory",
    "false",
    "final",
    "finally",
    "for",
    "get",
    "hide",
    "if",
    "implements",
    "import",
    "in",
    "interface",
    "is",
    "late",
    "library",
    "mixin",
    "new",
    "null",
    "of",
    "on",
    "operator",
    "part",
    "required",
    "rethrow",
    "return",
    "sealed",
    "set",
    "show",
    "static",
    "super",
    "switch",
    "sync",
    "this",
    "throw",
    "true",
    "try",
    "type",
    "typedef",
    "var",
    "void",
    "when",
    "while",
    "with",
    "yield",
];

/// 区画の値の型と、`ColumnType` の値
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

/// 文字列を Dart の文字列の式にする
fn quote(text: &str) -> String {
    let mut out = String::from("'");
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\'' => out.push_str("\\'"),
            '$' => out.push_str("\\$"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if u32::from(c) < 0x20 => {
                let _ = write!(out, "\\u{{{:x}}}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out.push('\'');
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

/// 欄の名前を、ゲッターの名前にする
fn member(name: &str) -> String {
    let name = camel(name);
    if MEMBERS.contains(&name.as_str()) || KEYWORDS.contains(&name.as_str()) {
        format!("{name}Field")
    } else {
        name
    }
}

/// 列挙の値・op・状態の名前を、定数の名前にする
fn constant(name: &str) -> String {
    let name = camel(name);
    if KEYWORDS.contains(&name.as_str()) {
        format!("{name}Value")
    } else {
        name
    }
}

/// 欄の型の、Dart の型
fn dart_type(schema: &Schema, ty: &Ty) -> String {
    match ty {
        Ty::String | Ty::Const(_) => "String".to_string(),
        Ty::Bool => "bool".to_string(),
        Ty::UInt { .. } | Ty::Int { .. } => "int".to_string(),
        Ty::Float => "double".to_string(),
        Ty::Ref(name) => match schema.def(name).map(|def| &def.shape) {
            Some(Shape::Enum { .. }) => "String".to_string(),
            Some(Shape::Object { .. } | Shape::Union { .. }) => pascal(name),
            Some(Shape::Handwritten) if name == RESPONSE => "Response".to_string(),
            _ => "Object?".to_string(),
        },
        Ty::Array(items) => format!("List<{}>", dart_type(schema, items)),
        Ty::Map(values) => format!("Map<String, {}>", dart_type(schema, values)),
    }
}

/// 欄の型の値を JSON の値から読む関数の式（`message.dart` の関数）
fn decoder(schema: &Schema, ty: &Ty) -> String {
    match ty {
        Ty::String | Ty::Const(_) => "asString".to_string(),
        Ty::Bool => "asBool".to_string(),
        Ty::UInt { .. } | Ty::Int { .. } => "asInt".to_string(),
        Ty::Float => "asDouble".to_string(),
        Ty::Ref(name) => match schema.def(name).map(|def| &def.shape) {
            Some(Shape::Enum { .. }) => "asString".to_string(),
            Some(Shape::Object { .. } | Shape::Union { .. }) => {
                format!("objectOf({}.fromJson)", pascal(name))
            }
            Some(Shape::Handwritten) if name == RESPONSE => {
                "objectOf(Response.fromJson)".to_string()
            }
            _ => "asJson".to_string(),
        },
        Ty::Array(items) => format!("listOf({})", decoder(schema, items)),
        Ty::Map(values) => format!("mapOf({})", decoder(schema, values)),
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

/// 見方のクラスの始まり。JSON のオブジェクトを受ける作り方を置く
fn class_head(out: &mut String, name: &str, description: &str, init: &str) {
    out.push('\n');
    doc(out, "", description);
    let _ = writeln!(
        out,
        "class {name} extends Message {{\n  /// 読んだ JSON のオブジェクトを包む\n  {name}.fromJson(super.json);\n\n  /// {init}\n  {name}() : super({{}});"
    );
}

/// null を許す型にする
fn nullable(ty: String) -> String {
    if ty.ends_with('?') {
        ty
    } else {
        format!("{ty}?")
    }
}

/// 欄ごとの読み書きのゲッターとセッター。ない欄と形の違う値は null で、null を書くと欄を除く
fn render_field(out: &mut String, schema: &Schema, name: &str, description: &str, ty: &Ty) {
    out.push('\n');
    doc(out, "  ", description);
    let key = quote(name);
    let member = member(name);
    let ty_name = nullable(dart_type(schema, ty));
    let _ = writeln!(
        out,
        "  {ty_name} get {member} => readField({key}, {});\n  set {member}({ty_name} value) => writeField({key}, value);",
        decoder(schema, ty)
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
            return Err(format!("{owner} の{what}が、Dart の名前 {name} で重なる"));
        }
    }
    Ok(())
}

/// オブジェクトの定義のクラス
fn render_object(out: &mut String, schema: &Schema, def: &Def) -> Result<(), String> {
    let name = pascal(&def.name);
    let mut fields = Vec::new();
    collect_fields(schema, def, &mut fields, 0);
    check_unique(
        "欄",
        &def.name,
        fields.iter().map(|field| member(&field.name)),
    )?;
    let consts: Vec<String> = fields
        .iter()
        .filter_map(|field| match &field.ty {
            Ty::Const(value) => Some(format!("{}: {}", quote(&field.name), quote(value))),
            _ => None,
        })
        .collect();
    out.push('\n');
    doc(out, "", &def.description);
    if consts.is_empty() {
        let _ = writeln!(
            out,
            "class {name} extends Message {{\n  /// 読んだ JSON のオブジェクトを包む\n  {name}.fromJson(super.json);\n\n  /// 空のオブジェクトから作る\n  {name}() : super({{}});"
        );
    } else {
        let _ = writeln!(
            out,
            "class {name} extends Message {{\n  /// 読んだ JSON のオブジェクトを包む\n  {name}.fromJson(super.json);\n\n  /// 目印の欄だけを持つオブジェクトから作る\n  {name}() : super({{{}}});",
            consts.join(", ")
        );
    }
    for field in fields {
        render_field(out, schema, &field.name, &field.description, &field.ty);
    }
    out.push_str("}\n");
    Ok(())
}

/// タグつきの共用体のクラス。形ごとの見方に移すゲッターを持つ
fn render_union(out: &mut String, schema: &Schema, def: &Def, variants: &[String]) {
    let name = pascal(&def.name);
    class_head(out, &name, &def.description, "空のオブジェクトから作る");
    for variant in variants {
        let Some((field, value)) = schema.tag_of(variant) else {
            continue;
        };
        let class = pascal(variant);
        let _ = writeln!(
            out,
            "\n  /// {field} が {value} の形なら、その見方。違えば null\n  {class}? get as{class} => taggedView({}, {}, {class}.fromJson);",
            quote(field),
            quote(value)
        );
    }
    out.push_str("}\n");
}

/// 手で書く応答のクラス
fn render_response(out: &mut String, schema: &Schema, def: &Def) {
    class_head(
        out,
        "Response",
        &def.description,
        "空のオブジェクトから作る",
    );
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
            "op ごとの結果（<op>_result の形の JSON のオブジェクト）。失敗したときは null",
            Ty::Map(Box::new(Ty::Ref(String::new()))),
        ),
    ];
    for (field, description, ty) in &fields {
        render_field(out, schema, field, description, ty);
    }
    out.push_str("\n  /// op ごとの結果を、その op の結果の型で読む（例: `response.resultAs(OpenResult.fromJson)`）。失敗したときは null\n  T? resultAs<T>(T Function(Map<String, Object?>) of) => readField('result', objectOf(of));\n}\n");
}

/// `bindings/dart/lib/src/messages.dart` の中身を作る
pub fn render_types(schema: &Schema) -> Result<String, String> {
    if let Some(def) = schema
        .defs
        .iter()
        .find(|def| def.name != RESPONSE && RESERVED.contains(&pascal(&def.name).as_str()))
    {
        return Err(format!(
            "schema/messages.json の {} が、Dart の包みが使う型の名前と重なる",
            def.name
        ));
    }
    let mut out = String::new();
    out.push_str("// 生成したファイル。手で直さない。\n");
    out.push_str("// 元は schema/messages.json で、`cargo xtask codegen` で作り直す。\n");
    out.push_str("//\n// 境界のメッセージの型（決定 0012・0227）。スキーマの定義ごとに 1 つの型がある。型は JSON のオブジェクトを包む\n// 見方で、知らない欄もそのまま持つ。列挙は文字列で読み、知らない値も受け取る（決定 0015）。\n\n// ignore_for_file: lines_longer_than_80_chars\n\nimport 'message.dart';\n");
    for def in &schema.defs {
        match &def.shape {
            Shape::Object { .. } => render_object(&mut out, schema, def)?,
            Shape::Union { variants } => render_union(&mut out, schema, def, variants),
            Shape::Enum { values } => {
                check_unique("値", &def.name, values.iter().map(|value| constant(value)))?;
                out.push('\n');
                doc(
                    &mut out,
                    "",
                    &format!("{}\n\n列挙の値。知らない値も受け取る", def.description),
                );
                let _ = writeln!(out, "abstract final class {} {{", pascal(&def.name));
                for value in values {
                    let _ = writeln!(
                        out,
                        "  static const {} = {};",
                        constant(value),
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
    check_unique("op", REQUEST, ops.iter().map(|op| constant(op)))?;
    out.push_str("\n/// op の名前。要求の型は `<Op>Request`、結果の型は `<Op>Result`\nabstract final class Operations {\n");
    for op in ops {
        let _ = writeln!(out, "  static const {} = {};", constant(op), quote(op));
    }
    out.push_str("}\n");
    Ok(())
}

/// `bindings/dart/lib/src/constants.dart` の中身を作る
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
    out.push_str("\n/// 境界の版と、シーンの配置（決定 0012・0137・0227）\nabstract final class Constants {\n");
    let versions = &boundary.versions;
    let _ = writeln!(
        out,
        "  /// パッケージの版（決定 0015）\n  static const version = {};\n\n  /// ABI の大版\n  static const abiVersion = {};\n\n  /// メッセージのスキーマの大版（要求と応答の v）\n  static const schemaMajor = {};\n\n  /// メッセージのスキーマの小版\n  static const schemaMinor = {};",
        quote(version),
        versions.abi,
        versions.schema_major,
        versions.schema_minor
    );
    let _ = writeln!(
        out,
        "\n  /// シーンの大版。区画の意味を変えるときに上げる\n  static const sceneMajor = {};\n\n  /// シーンの小版。区画か値を足すたびに上げる\n  static const sceneMinor = {};\n\n  /// 番号の「なし」（u32 の最大）\n  static const sceneNone = 0xffffffff;",
        scene.versions.major, scene.versions.minor
    );
    out.push_str(
        "\n  /// シーンの区画の定義。番号の順\n  static const sceneSections = <SceneSection>[\n",
    );
    for section in &scene.sections {
        if !COLUMNS.contains(&section.ty.as_str()) {
            return Err(format!(
                "区画 {} の型 {} を読めない",
                section.name, section.ty
            ));
        }
        doc(
            &mut out,
            "    ",
            &format!(
                "{}（{} × {}）",
                section.description, section.ty, section.width
            ),
        );
        let _ = writeln!(
            out,
            "    SceneSection({}, ColumnType.{}, {}),",
            quote(&section.name),
            section.ty,
            section.width
        );
    }
    out.push_str("  ];\n");

    out.push_str("\n  /// シーンの列に入れる値の組。組の名前から、値の名前と値の表を引く\n  static const sceneValues = <String, Map<String, int>>{\n");
    for values in &scene.values {
        let _ = writeln!(
            out,
            "    // {}\n    {}: {{",
            values.description.lines().next().unwrap_or(""),
            quote(&values.name)
        );
        for item in &values.items {
            let _ = writeln!(out, "      {}: {},", quote(&item.name), item.value);
        }
        out.push_str("    },\n");
    }
    out.push_str("  };\n}\n");

    check_unique(
        "状態",
        "boundary",
        boundary.status.iter().map(|s| constant(&s.name)),
    )?;
    out.push_str("\n/// 状態コード\nabstract final class Status {\n");
    for status in &boundary.status {
        doc(&mut out, "  ", &status.description);
        let _ = writeln!(
            out,
            "  static const {} = {};\n",
            constant(&status.name),
            status.value
        );
    }
    out.push_str("  /// 状態コードの名前。知らない値は `unknown(<値>)`\n  static String name(int code) => switch (code) {\n");
    for status in &boundary.status {
        let _ = writeln!(out, "        {} => {},", status.value, quote(&status.name));
    }
    out.push_str("        _ => 'unknown($code)',\n      };\n}\n");

    out.push_str("\n/// シーンの区画の値の型\nenum ColumnType {\n  u8(1),\n  u32(4),\n  f32(4),\n  f64(8);\n\n  const ColumnType(this.size);\n\n  /// 値のバイト数\n  final int size;\n}\n");
    out.push_str("\n/// シーンの区画の定義\nfinal class SceneSection {\n  const SceneSection(this.name, this.type, this.width);\n\n  /// 名前\n  final String name;\n\n  /// 値の型\n  final ColumnType type;\n\n  /// 行ごとの値の数\n  final int width;\n}\n");
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
                "unit": { "description": "単位", "type": "string", "enum": ["meter", "in"] },
                "response": { "description": "応答" }
              }
            }"##
            .as_bytes(),
        )
        .unwrap();
        let types = render_types(&schema).unwrap();
        assert!(types.contains("class OpenRequest extends Message {"));
        assert!(types.contains("  OpenRequest() : super({'op': 'open'});"));
        assert!(types.contains(
            "  int? get v => readField('v', asInt);\n  set v(int? value) => writeField('v', value);"
        ));
        assert!(types.contains("  String? get fromUnit => readField('from_unit', asString);"));
        assert!(types.contains("  String? get classField => readField('class', asString);"));
        assert!(types.contains("  bool? get jsonField => readField('json', asBool);"));
        assert!(
            types.contains("  List<String>? get names => readField('names', listOf(asString));")
        );
        assert!(types.contains("  static const inValue = 'in';"));
        assert!(types.contains(
            "  OpenRequest? get asOpenRequest => taggedView('op', 'open', OpenRequest.fromJson);"
        ));
        assert!(types.contains("abstract final class Operations {\n  static const open = 'open';"));
        assert!(types.contains("class Response extends Message {"));
        assert!(
            types.contains(
                "  Map<String, Object?>? get result => readField('result', mapOf(asJson));"
            )
        );
    }

    #[test]
    fn rejects_names_that_meet_in_dart() {
        assert_eq!(camel("bcf_read"), "bcfRead");
        assert_eq!(camel("2d"), "value2d");
        assert_eq!(quote("a'$b"), "'a\\'\\$b'");
        assert!(check_unique("値", "x", ["a".to_string(), "a".to_string()]).is_err());
    }
}
