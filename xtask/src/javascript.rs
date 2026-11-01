//! TypeScript・JavaScript の包み（`bindings/js`）の型と定数を作る（決定 0011・0194）。
//!
//! - `messages.d.ts`: メッセージのスキーマ（`schema/messages.json`）の型
//! - `constants.js`・`constants.d.ts`: 境界の版と状態コード（`schema/boundary.toml`）と、シーンの配置
//!   （`schema/scene.toml`）

use std::fmt::Write;

use crate::boundary::{Boundary, RuleKind};
use crate::scene::Scene;
use crate::schema::{Def, Member, Schema, Shape, Ty, pascal};

/// op の共用体の定義の名前
const REQUEST: &str = "request";

/// 手で書く応答の定義の名前
const RESPONSE: &str = "response";

/// 区画の値の型と、型付き配列
const COLUMNS: &[(&str, &str)] = &[
    ("u8", "Uint8Array"),
    ("u32", "Uint32Array"),
    ("f32", "Float32Array"),
    ("f64", "Float64Array"),
];

/// JSDoc のコメントを書く。`*/` はコメントを閉じないように崩す
fn doc(out: &mut String, indent: &str, text: &str) {
    let text = text.replace("*/", "*\\/");
    let lines: Vec<&str> = text.lines().collect();
    match lines.as_slice() {
        [] => {}
        [line] => {
            let _ = writeln!(out, "{indent}/** {line} */");
        }
        _ => {
            let _ = writeln!(out, "{indent}/**");
            for line in lines {
                let _ = writeln!(out, "{indent} * {line}");
            }
            let _ = writeln!(out, "{indent} */");
        }
    }
}

fn ts_type(ty: &Ty) -> String {
    match ty {
        Ty::String => "string".to_string(),
        Ty::Const(value) => format!("\"{value}\""),
        Ty::Bool => "boolean".to_string(),
        Ty::UInt { .. } | Ty::Int { .. } | Ty::Float => "number".to_string(),
        Ty::Ref(name) => pascal(name),
        Ty::Array(items) => format!("{}[]", ts_type(items)),
        Ty::Map(values) => format!("Record<string, {}>", ts_type(values)),
    }
}

/// `bindings/js/src/messages.d.ts` の中身を作る
pub fn render_types(schema: &Schema) -> String {
    let mut out = String::new();
    out.push_str("// 生成したファイル。手で直さない。\n");
    out.push_str("// 元は schema/messages.json で、`cargo xtask codegen` で作り直す。\n");
    out.push_str(
        "//\n// 境界のメッセージの型（決定 0012・0194）。スキーマの定義ごとに 1 つの型がある。\n",
    );
    out.push_str("// 列挙は、知らない値も受け取れるように、ほかの文字列も許す（決定 0015）。\n");
    for def in &schema.defs {
        out.push('\n');
        match &def.shape {
            Shape::Object {
                fields, compose, ..
            } => {
                doc(&mut out, "", &def.description);
                let extends = if compose.is_empty() {
                    String::new()
                } else {
                    let parents: Vec<String> = compose.iter().map(|name| pascal(name)).collect();
                    format!(" extends {}", parents.join(", "))
                };
                let _ = writeln!(out, "export interface {}{extends} {{", pascal(&def.name));
                for field in fields {
                    doc(&mut out, "  ", &field.description);
                    let optional = if field.required { "" } else { "?" };
                    let _ = writeln!(out, "  {}{optional}: {};", field.name, ts_type(&field.ty));
                }
                out.push_str("}\n");
            }
            Shape::Enum { values } => {
                doc(&mut out, "", &def.description);
                let _ = write!(out, "export type {} =", pascal(&def.name));
                for value in values {
                    let _ = write!(out, "\n  | \"{value}\"");
                }
                out.push_str("\n  | (string & {});\n");
            }
            Shape::Union { variants } => {
                doc(&mut out, "", &def.description);
                let _ = write!(out, "export type {} =", pascal(&def.name));
                for variant in variants {
                    let _ = write!(out, "\n  | {}", pascal(variant));
                }
                out.push_str(";\n");
            }
            Shape::Scalars { members } => {
                doc(&mut out, "", &def.description);
                let _ = write!(out, "export type {} =", pascal(&def.name));
                for member in members {
                    let ty = match member {
                        Member::Integer { .. } | Member::Number { .. } => "number".to_string(),
                        Member::String { .. } => "string".to_string(),
                        Member::Const { value, .. } => format!("\"{value}\""),
                    };
                    let _ = write!(out, "\n  /** {} */\n  | {ty}", member.description());
                }
                out.push_str(";\n");
            }
            Shape::Handwritten if def.name == RESPONSE => render_response(&mut out, def),
            Shape::Handwritten => {}
        }
    }
    render_operations(&mut out, schema);
    out
}

/// 応答は、結果の形を型の引数にする
fn render_response(out: &mut String, def: &Def) {
    doc(out, "", &def.description);
    out.push_str("export interface Response<R = unknown> {\n");
    out.push_str("  /** メッセージのスキーマの大版 */\n  v: number;\n");
    out.push_str(
        "  /** op ごとの結果（<op>_result の形）。失敗したときは持たない */\n  result?: R;\n",
    );
    out.push_str("  /** 診断。入力の順 */\n  diagnostics: Diagnostic[];\n");
    out.push_str(
        "  /** バイナリの結果。take で取り出す。次の call で捨てる */\n  blobs: Blob[];\n",
    );
    out.push_str("}\n");
}

/// op ごとの要求と結果の型の表
fn render_operations(out: &mut String, schema: &Schema) {
    let Some(Shape::Union { variants }) = schema.def(REQUEST).map(|def| &def.shape) else {
        return;
    };
    out.push_str("\n/** op ごとの要求と結果の型 */\nexport interface Operations {\n");
    for variant in variants {
        let Some((_, op)) = schema.tag_of(variant) else {
            continue;
        };
        let result_name = format!("{op}_result");
        let result = schema
            .def(&result_name)
            .map_or("unknown".to_string(), |_| pascal(&result_name));
        let _ = writeln!(
            out,
            "  {op}: {{ request: {}; result: {result} }};",
            pascal(variant)
        );
    }
    out.push_str("}\n");
}

/// 文字列を JavaScript の文字列の式にする。定義の名前と説明は検査済みなので、`"` と `\` だけを崩す
fn quote(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}

fn header(out: &mut String) {
    out.push_str("// 生成したファイル。手で直さない。\n");
    out.push_str(
        "// 元は schema/boundary.toml と schema/scene.toml で、`cargo xtask codegen` で作り直す。\n",
    );
}

fn wasm_exports(boundary: &Boundary) -> Vec<&str> {
    let mut names: Vec<&str> = boundary
        .functions
        .iter()
        .filter(|function| function.targets.iter().any(|target| target == "wasm"))
        .map(|function| function.name.as_str())
        .collect();
    names.sort_unstable();
    names
}

/// wasm のモジュールの表と、形式の判定の順と規則（決定 0013・0221）
fn render_modules(out: &mut String, boundary: &Boundary) {
    let list = |items: &[String]| {
        items
            .iter()
            .map(|item| quote(item))
            .collect::<Vec<_>>()
            .join(", ")
    };
    out.push_str("\n/** wasm のモジュール。名前から、持つ形式と op を引く（決定 0013・0221） */\nexport const MODULES = Object.freeze({\n");
    for module in &boundary.modules {
        let _ = writeln!(
            out,
            "  {}: Object.freeze({{\n    description: {},\n    formats: Object.freeze([{}]),\n    ops: Object.freeze([{}]),\n  }}),",
            module.name,
            quote(&module.description),
            list(&module.formats),
            list(&module.ops)
        );
    }
    out.push_str("});\n\n/** open が形式を判定する順と規則。rules のどれかに合えば format とする */\nexport const DETECTION = Object.freeze([\n");
    for detection in &boundary.detection {
        let _ = writeln!(
            out,
            "  Object.freeze({{\n    format: {},\n    module: {},\n    rules: Object.freeze([",
            quote(&detection.format),
            quote(&detection.module)
        );
        for rule in &detection.rules {
            match rule.kind() {
                Some(RuleKind::Bytes { offset, bytes }) => {
                    let bytes: Vec<String> = bytes.iter().map(|b| format!("0x{b:02x}")).collect();
                    let _ = writeln!(
                        out,
                        "      Object.freeze({{ kind: \"bytes\", offset: {offset}, bytes: Object.freeze([{}]) }}),",
                        bytes.join(", ")
                    );
                }
                Some(RuleKind::Lines(lines)) => {
                    let _ = writeln!(
                        out,
                        "      Object.freeze({{ kind: \"lines\", lines: Object.freeze([{}]) }}),",
                        list(lines)
                    );
                }
                Some(RuleKind::StepSchema(prefix)) => {
                    let _ = writeln!(
                        out,
                        "      Object.freeze({{ kind: \"step_schema\", prefix: {} }}),",
                        quote(prefix)
                    );
                }
                Some(RuleKind::XmlRoot { name, namespace }) => {
                    let namespace = namespace.map_or("null".to_string(), quote);
                    let _ = writeln!(
                        out,
                        "      Object.freeze({{ kind: \"xml_root\", name: {}, namespace: {namespace} }}),",
                        quote(name)
                    );
                }
                None => {}
            }
        }
        out.push_str("    ]),\n  }),\n");
    }
    out.push_str("]);\n");
}

/// `bindings/js/src/constants.js` の中身を作る
pub fn render_constants(version: &str, boundary: &Boundary, scene: &Scene) -> String {
    let mut out = String::new();
    header(&mut out);
    let versions = &boundary.versions;
    let _ = writeln!(
        out,
        "\n/** パッケージの版（決定 0015） */\nexport const VERSION = {};",
        quote(version)
    );
    let _ = writeln!(
        out,
        "/** ABI の大版 */\nexport const ABI_VERSION = {};",
        versions.abi
    );
    let _ = writeln!(
        out,
        "/** メッセージのスキーマの大版（要求と応答の v） */\nexport const SCHEMA_MAJOR = {};",
        versions.schema_major
    );
    let _ = writeln!(
        out,
        "/** メッセージのスキーマの小版 */\nexport const SCHEMA_MINOR = {};",
        versions.schema_minor
    );
    out.push_str("\n/** 状態コードの名前。値から引く */\nexport const STATUS = Object.freeze({\n");
    for status in &boundary.status {
        let _ = writeln!(out, "  {}: {},", status.value, quote(&status.name));
    }
    out.push_str(
        "});\n\n/** wasm のモジュールが出す関数 */\nexport const WASM_EXPORTS = Object.freeze([\n",
    );
    for name in wasm_exports(boundary) {
        let _ = writeln!(out, "  {},", quote(name));
    }
    out.push_str("]);\n");
    render_modules(&mut out, boundary);

    out.push_str(
        "\n/** シーンの見出しの始まりの 4 バイト */\nexport const SCENE_MAGIC = \"ZMSC\";\n",
    );
    let _ = writeln!(
        out,
        "/** シーンの大版。区画の意味を変えるときに上げる */\nexport const SCENE_MAJOR = {};",
        scene.versions.major
    );
    let _ = writeln!(
        out,
        "/** シーンの小版。区画か値を足すたびに上げる */\nexport const SCENE_MINOR = {};",
        scene.versions.minor
    );
    out.push_str("/** 番号の「なし」 */\nexport const SCENE_NONE = 4294967295;\n");
    out.push_str(
        "\n/** シーンの区画の定義。番号の順 */\nexport const SCENE_SECTIONS = Object.freeze([\n",
    );
    for section in &scene.sections {
        let _ = writeln!(
            out,
            "  Object.freeze({{ name: {}, type: {}, width: {} }}),",
            quote(&section.name),
            quote(&section.ty),
            section.width
        );
    }
    out.push_str(
        "]);\n\n/** シーンの列に入れる値の組 */\nexport const SCENE_VALUES = Object.freeze({\n",
    );
    for values in &scene.values {
        let _ = writeln!(out, "  {}: Object.freeze({{", values.name);
        for item in &values.items {
            let _ = writeln!(out, "    {}: {},", item.name, item.value);
        }
        out.push_str("  }),\n");
    }
    out.push_str("});\n");
    out
}

/// `bindings/js/src/constants.d.ts` の中身を作る
pub fn render_constants_types(boundary: &Boundary, scene: &Scene) -> String {
    let mut out = String::new();
    header(&mut out);
    let versions = &boundary.versions;
    out.push_str("\n/** パッケージの版（決定 0015） */\nexport declare const VERSION: string;\n");
    let _ = writeln!(
        out,
        "/** ABI の大版 */\nexport declare const ABI_VERSION: {};",
        versions.abi
    );
    let _ = writeln!(
        out,
        "/** メッセージのスキーマの大版（要求と応答の v） */\nexport declare const SCHEMA_MAJOR: {};",
        versions.schema_major
    );
    out.push_str("/** メッセージのスキーマの小版 */\nexport declare const SCHEMA_MINOR: number;\n");
    out.push_str("\n/** 状態コードの名前 */\nexport type StatusName =");
    for status in &boundary.status {
        let _ = write!(
            out,
            "\n  /** {} */\n  | {}",
            status.description,
            quote(&status.name)
        );
    }
    out.push_str(";\n\n/** 状態コードの名前。値から引く */\nexport declare const STATUS: Readonly<Record<number, StatusName>>;\n");
    out.push_str("\n/** wasm のモジュールが出す関数 */\nexport declare const WASM_EXPORTS: readonly string[];\n");
    out.push_str("\n/** wasm のモジュールの名前（決定 0013・0221） */\nexport type ModuleName =");
    for module in &boundary.modules {
        let _ = write!(
            out,
            "\n  /** {} */\n  | {}",
            module.description,
            quote(&module.name)
        );
    }
    out.push_str(";\n\n/** wasm のモジュールが持つ形式と op */\nexport interface ModuleInfo {\n  readonly description: string;\n  readonly formats: readonly string[];\n  readonly ops: readonly string[];\n}\n");
    out.push_str("\n/** wasm のモジュール。名前から引く */\nexport declare const MODULES: Readonly<Record<ModuleName, ModuleInfo>>;\n");
    out.push_str("\n/** 形式の判定の規則（zumen-sdk の Signature） */\nexport type DetectionRule =\n  | { readonly kind: \"bytes\"; readonly offset: number; readonly bytes: readonly number[] }\n  | { readonly kind: \"lines\"; readonly lines: readonly string[] }\n  | { readonly kind: \"step_schema\"; readonly prefix: string }\n  | { readonly kind: \"xml_root\"; readonly name: string; readonly namespace: string | null };\n");
    out.push_str("\n/** 形式の判定。rules のどれかに合えば format とする */\nexport interface Detection {\n  readonly format: string;\n  readonly module: ModuleName;\n  readonly rules: readonly DetectionRule[];\n}\n");
    out.push_str("\n/** open が形式を判定する順と規則 */\nexport declare const DETECTION: readonly Detection[];\n");

    out.push_str(
        "\n/** シーンの見出しの始まりの 4 バイト */\nexport declare const SCENE_MAGIC: \"ZMSC\";\n",
    );
    let _ = writeln!(
        out,
        "/** シーンの大版。区画の意味を変えるときに上げる */\nexport declare const SCENE_MAJOR: {};",
        scene.versions.major
    );
    out.push_str("/** シーンの小版。区画か値を足すたびに上げる */\nexport declare const SCENE_MINOR: number;\n");
    out.push_str("/** 番号の「なし」 */\nexport declare const SCENE_NONE: 4294967295;\n");

    out.push_str("\n/** シーンの区画の名前 */\nexport type SceneSectionName =");
    for section in &scene.sections {
        let _ = write!(out, "\n  | {}", quote(&section.name));
    }
    out.push_str(";\n\n/** シーンの区画の値の型 */\nexport type SceneValueType =");
    for (ty, _) in COLUMNS {
        let _ = write!(out, " | {}", quote(ty));
    }
    out.push_str(";\n\n/** シーンの区画の定義 */\nexport interface SceneSection {\n  readonly name: SceneSectionName;\n  readonly type: SceneValueType;\n  /** 行ごとの値の数 */\n  readonly width: number;\n}\n");
    out.push_str("\n/** シーンの区画の定義。番号の順 */\nexport declare const SCENE_SECTIONS: readonly SceneSection[];\n");
    out.push_str("\n/** シーンの区画ごとの列。行ごとに width 個の値を並べる */\nexport interface SceneColumns {\n");
    for section in &scene.sections {
        let array = COLUMNS
            .iter()
            .find(|(ty, _)| *ty == section.ty)
            .map_or("Uint8Array", |(_, array)| array);
        doc(
            &mut out,
            "  ",
            &format!(
                "{}（{} × {}）",
                section.description, section.ty, section.width
            ),
        );
        let _ = writeln!(out, "  readonly {}: {array};", section.name);
    }
    out.push_str("}\n\n/** シーンの列に入れる値の組 */\nexport declare const SCENE_VALUES: {\n");
    for values in &scene.values {
        doc(&mut out, "  ", &values.description);
        let _ = writeln!(out, "  readonly {}: {{", values.name);
        for item in &values.items {
            doc(&mut out, "    ", &item.description);
            let _ = writeln!(out, "    readonly {}: {};", item.name, item.value);
        }
        out.push_str("  };\n");
    }
    out.push_str("};\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn schema(source: &str) -> Schema {
        crate::schema::parse(source.as_bytes()).unwrap()
    }

    #[test]
    fn renders_requests_enums_and_operations() {
        let schema = schema(
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
                    "unit": { "description": "単位 */", "$ref": "#/$defs/unit" },
                    "names": { "description": "名前", "type": "array", "items": { "type": "string" } }
                  },
                  "required": ["op"],
                  "unevaluatedProperties": false
                },
                "open_result": {
                  "description": "結果",
                  "type": "object",
                  "properties": { "count": { "description": "数", "type": "integer", "minimum": 0 } },
                  "required": ["count"],
                  "additionalProperties": false
                },
                "unit": { "description": "単位", "type": "string", "enum": ["meter", "millimeter"] },
                "response": { "description": "応答" }
              }
            }"##,
        );
        let types = render_types(&schema);
        assert!(types.contains("export interface OpenRequest extends RequestCommon {"));
        assert!(types.contains("  op: \"open\";"));
        assert!(types.contains("  unit?: Unit;"));
        assert!(types.contains("  names?: string[];"));
        assert!(types.contains("/** 単位 *\\/ */"));
        assert!(types.contains("  | \"millimeter\"\n  | (string & {});"));
        assert!(types.contains("export type Request =\n  | OpenRequest;"));
        assert!(types.contains("  open: { request: OpenRequest; result: OpenResult };"));
        assert!(types.contains("/** 応答 */\nexport interface Response<R = unknown> {"));
    }
}
