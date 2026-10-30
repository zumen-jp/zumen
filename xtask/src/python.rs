//! Python の包み（`bindings/python`）の型と定数を作る（決定 0011・0195）。
//!
//! - `messages.py`: メッセージのスキーマ（`schema/messages.json`）の型
//! - `constants.py`: 境界の版・状態コード・関数の形（`schema/boundary.toml`）と、シーンの配置
//!   （`schema/scene.toml`）
//!
//! Python 3.9（docs/support.md の下限）で読めるように、標準の `typing` の `TypedDict`・`Literal`・`Union` だけを使う。

use std::fmt::Write;

use crate::boundary::Boundary;
use crate::scene::Scene;
use crate::schema::{Def, Field, Member, Schema, Shape, Ty, pascal};

/// op の共用体の定義の名前
const REQUEST: &str = "request";

/// 手で書く応答の定義の名前
const RESPONSE: &str = "response";

/// `messages.py` が `typing` から読み込む名前。定義の名前と重ならないようにする
const TYPING: &[&str] = &["Any", "Dict", "List", "Literal", "TypedDict", "Union"];

/// 境界の引数と戻り値の型と、ctypes の型
const CTYPES: &[(&str, &str)] = &[
    ("void", "None"),
    ("u32", "ctypes.c_uint32"),
    ("size", "ctypes.c_size_t"),
    ("status", "ctypes.c_int32"),
    ("bytes", "ctypes.c_char_p"),
    ("bytes_mut", "ctypes.c_void_p"),
    ("session", "ctypes.c_void_p"),
    ("session_out", "ctypes.POINTER(ctypes.c_void_p)"),
    ("buf", "ctypes.POINTER(Buf)"),
    ("arrow_stream", "ctypes.c_void_p"),
];

/// 区画の値の型と、memoryview・array の形式の文字
const COLUMNS: &[(&str, &str)] = &[("u8", "B"), ("u32", "I"), ("f32", "f"), ("f64", "d")];

/// 説明を `#` のコメントにする
fn comment(out: &mut String, indent: &str, text: &str) {
    for line in text.lines() {
        let _ = writeln!(out, "{indent}# {line}");
    }
}

/// 文字列を Python の文字列の式にする。定義の名前と説明は検査済みなので、`"` と `\` だけを崩す
fn quote(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}

/// 欄の型の式。定義の名前は、定義の順によらずに引けるように文字列で書く
fn py_type(ty: &Ty) -> String {
    match ty {
        Ty::String => "str".to_string(),
        Ty::Const(value) => format!("Literal[{}]", quote(value)),
        Ty::Bool => "bool".to_string(),
        Ty::UInt { .. } | Ty::Int { .. } => "int".to_string(),
        Ty::Float => "float".to_string(),
        Ty::Ref(name) => quote(&pascal(name)),
        Ty::Array(items) => format!("List[{}]", py_type(items)),
        Ty::Map(values) => format!("Dict[str, {}]", py_type(values)),
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

/// `TypedDict` を関数の形で書く。欄の名前に `from` などの予約語があるので、クラスの形は使わない
fn typed_dict(out: &mut String, name: &str, fields: &[(&str, &str, String)], total: bool) {
    if fields.is_empty() {
        let _ = writeln!(out, "{name} = TypedDict({}, {{}})", quote(name));
        return;
    }
    let _ = writeln!(out, "{name} = TypedDict(");
    let _ = writeln!(out, "    {},", quote(name));
    out.push_str("    {\n");
    for (field, description, ty) in fields {
        comment(out, "        ", description);
        let _ = writeln!(out, "        {}: {ty},", quote(field));
    }
    out.push_str("    },\n");
    if !total {
        out.push_str("    total=False,\n");
    }
    out.push_str(")\n");
}

/// 欠かせない欄と、なくてよい欄を分けて書く。両方あれば、2 つを継いだクラスにする
fn render_object(
    out: &mut String,
    name: &str,
    description: &str,
    fields: &[(&str, &str, String, bool)],
) {
    let required: Vec<(&str, &str, String)> = fields
        .iter()
        .filter(|field| field.3)
        .map(|(n, d, t, _)| (*n, *d, t.clone()))
        .collect();
    let optional: Vec<(&str, &str, String)> = fields
        .iter()
        .filter(|field| !field.3)
        .map(|(n, d, t, _)| (*n, *d, t.clone()))
        .collect();
    comment(out, "", description);
    if optional.is_empty() || required.is_empty() {
        typed_dict(
            out,
            name,
            if optional.is_empty() {
                &required
            } else {
                &optional
            },
            optional.is_empty(),
        );
        return;
    }
    typed_dict(out, &format!("_{name}Required"), &required, true);
    typed_dict(out, &format!("_{name}Optional"), &optional, false);
    let _ = writeln!(out, "\n\nclass {name}(_{name}Required, _{name}Optional):");
    let _ = writeln!(
        out,
        "    {}",
        quote(description.lines().next().unwrap_or(name))
    );
}

/// `bindings/python/zumen_provisional/messages.py` の中身を作る
pub fn render_types(schema: &Schema) -> Result<String, String> {
    if let Some(def) = schema
        .defs
        .iter()
        .find(|def| TYPING.contains(&pascal(&def.name).as_str()))
    {
        return Err(format!(
            "schema/messages.json の {} が、Python の typing の名前と重なる",
            def.name
        ));
    }
    let mut out = String::new();
    out.push_str("# 生成したファイル。手で直さない。\n");
    out.push_str("# 元は schema/messages.json で、`cargo xtask codegen` で作り直す。\n");
    out.push_str(
        "#\n# 境界のメッセージの型（決定 0012・0195）。スキーマの定義ごとに 1 つの型がある。\n",
    );
    out.push_str("# 列挙は、知らない値も受け取れるように、ほかの文字列も許す（決定 0015）。\n");
    out.push_str(
        "# Python 3.9 で読めるように、TypedDict を関数の形で書き、定義の名前は文字列で引く。\n",
    );
    out.push_str("\nfrom typing import Any, Dict, List, Literal, TypedDict, Union\n");
    for def in &schema.defs {
        let name = pascal(&def.name);
        match &def.shape {
            Shape::Object { .. } => {
                out.push_str("\n\n");
                let mut fields = Vec::new();
                collect_fields(schema, def, &mut fields, 0);
                let fields: Vec<(&str, &str, String, bool)> = fields
                    .iter()
                    .map(|field| {
                        (
                            field.name.as_str(),
                            field.description.as_str(),
                            py_type(&field.ty),
                            field.required,
                        )
                    })
                    .collect();
                render_object(&mut out, &name, &def.description, &fields);
            }
            Shape::Enum { values } => {
                out.push('\n');
                comment(&mut out, "", &def.description);
                let _ = writeln!(out, "{name} = Union[");
                out.push_str("    Literal[\n");
                for value in values {
                    let _ = writeln!(out, "        {},", quote(value));
                }
                out.push_str("    ],\n    str,\n]\n");
            }
            Shape::Union { variants } => {
                out.push('\n');
                comment(&mut out, "", &def.description);
                let _ = writeln!(out, "{name} = Union[");
                for variant in variants {
                    let _ = writeln!(out, "    {},", quote(&pascal(variant)));
                }
                out.push_str("]\n");
            }
            Shape::Scalars { members } => {
                out.push('\n');
                comment(&mut out, "", &def.description);
                let _ = writeln!(out, "{name} = Union[");
                for member in members {
                    let ty = match member {
                        Member::Integer { .. } => "int".to_string(),
                        Member::Number { .. } => "float".to_string(),
                        Member::String { .. } => "str".to_string(),
                        Member::Const { value, .. } => format!("Literal[{}]", quote(value)),
                    };
                    comment(&mut out, "    ", member.description());
                    let _ = writeln!(out, "    {ty},");
                }
                out.push_str("]\n");
            }
            Shape::Handwritten if def.name == RESPONSE => {
                out.push_str("\n\n");
                let fields = [
                    ("v", "メッセージのスキーマの大版", "int".to_string(), true),
                    (
                        "diagnostics",
                        "診断。入力の順",
                        "List[\"Diagnostic\"]".to_string(),
                        true,
                    ),
                    (
                        "blobs",
                        "バイナリの結果。take で取り出す。次の call で捨てる",
                        "List[\"Blob\"]".to_string(),
                        true,
                    ),
                    (
                        "result",
                        "op ごとの結果（<op>_result の形）。失敗したときは持たない",
                        "Dict[str, Any]".to_string(),
                        false,
                    ),
                ];
                render_object(&mut out, &name, &def.description, &fields);
            }
            Shape::Handwritten => {}
        }
    }
    render_operations(&mut out, schema);
    Ok(out)
}

/// op の名前と、op ごとの要求と結果の型の名前
fn render_operations(out: &mut String, schema: &Schema) {
    let Some(Shape::Union { variants }) = schema.def(REQUEST).map(|def| &def.shape) else {
        return;
    };
    let operations: Vec<(&str, String, String)> = variants
        .iter()
        .filter_map(|variant| {
            let (_, op) = schema.tag_of(variant)?;
            let result_name = format!("{op}_result");
            let result = schema
                .def(&result_name)
                .map_or("Dict[str, Any]".to_string(), |_| pascal(&result_name));
            Some((op, pascal(variant), result))
        })
        .collect();
    out.push_str("\n# op の名前\nOperation = Literal[\n");
    for (op, _, _) in &operations {
        let _ = writeln!(out, "    {},", quote(op));
    }
    out.push_str("]\n");
    out.push_str("\n# op ごとの要求と結果の型の名前。結果は応答の result を typing.cast で読む型\nOPERATIONS: Dict[str, Any] = {\n");
    for (op, request, result) in &operations {
        let _ = writeln!(
            out,
            "    {}: ({}, {}),",
            quote(op),
            quote(request),
            quote(result)
        );
    }
    out.push_str("}\n");
}

/// `bindings/python/zumen_provisional/constants.py` の中身を作る
pub fn render_constants(
    version: &str,
    boundary: &Boundary,
    scene: &Scene,
) -> Result<String, String> {
    let mut out = String::new();
    out.push_str("# 生成したファイル。手で直さない。\n");
    out.push_str(
        "# 元は schema/boundary.toml と schema/scene.toml で、`cargo xtask codegen` で作り直す。\n",
    );
    out.push_str("\nimport ctypes\nfrom types import MappingProxyType\nfrom typing import Literal, Mapping, NamedTuple, Tuple\n");
    let versions = &boundary.versions;
    let _ = writeln!(
        out,
        "\n# パッケージの版（決定 0015）\nVERSION = {}",
        quote(version)
    );
    let _ = writeln!(out, "# ABI の大版\nABI_VERSION = {}", versions.abi);
    let _ = writeln!(
        out,
        "# メッセージのスキーマの大版（要求と応答の v）\nSCHEMA_MAJOR = {}",
        versions.schema_major
    );
    let _ = writeln!(
        out,
        "# メッセージのスキーマの小版\nSCHEMA_MINOR = {}",
        versions.schema_minor
    );

    out.push_str("\n# 状態コードの名前\nStatusName = Literal[\n");
    for status in &boundary.status {
        comment(&mut out, "    ", &status.description);
        let _ = writeln!(out, "    {},", quote(&status.name));
    }
    out.push_str("]\n\n# 状態コードの名前。値から引く\nSTATUS: Mapping[int, str] = MappingProxyType(\n    {\n");
    for status in &boundary.status {
        let _ = writeln!(out, "        {}: {},", status.value, quote(&status.name));
    }
    out.push_str("    }\n)\n");

    out.push_str("\n\nclass Buf(ctypes.Structure):\n    \"\"\"コアが返すバッファー（zm_buf）。zm_free で返す\"\"\"\n\n    _fields_ = [(\"ptr\", ctypes.c_void_p), (\"len\", ctypes.c_size_t)]\n");
    out.push_str("\n\n# ネイティブのライブラリが出す関数の、戻り値と引数の ctypes の型\nFUNCTIONS: Mapping[str, Tuple[object, Tuple[object, ...]]] = MappingProxyType(\n    {\n");
    for function in &boundary.functions {
        if !function.targets.iter().any(|target| target == "native") {
            continue;
        }
        let ctype = |name: &str| {
            CTYPES
                .iter()
                .find(|(ty, _)| *ty == name)
                .map(|(_, ctype)| *ctype)
                .ok_or_else(|| format!("{} の型 {name} を ctypes に直せない", function.name))
        };
        let returns = ctype(&function.returns)?;
        let params = function
            .params
            .iter()
            .map(|param| ctype(&param.ty))
            .collect::<Result<Vec<_>, _>>()?;
        let params = match params.as_slice() {
            [] => "()".to_string(),
            [one] => format!("({one},)"),
            many => format!("({})", many.join(", ")),
        };
        let _ = writeln!(
            out,
            "        {}: ({returns}, {params}),",
            quote(&function.name)
        );
    }
    out.push_str("    }\n)\n");

    out.push_str("\n# シーンの見出しの始まりの 4 バイト\nSCENE_MAGIC = b\"ZMSC\"\n");
    let _ = writeln!(
        out,
        "# シーンの大版。区画の意味を変えるときに上げる\nSCENE_MAJOR = {}",
        scene.versions.major
    );
    let _ = writeln!(
        out,
        "# シーンの小版。区画か値を足すたびに上げる\nSCENE_MINOR = {}",
        scene.versions.minor
    );
    out.push_str("# 番号の「なし」\nSCENE_NONE = 4294967295\n");

    out.push_str("\n# シーンの区画の名前\nSceneSectionName = Literal[\n");
    for section in &scene.sections {
        let _ = writeln!(out, "    {},", quote(&section.name));
    }
    out.push_str("]\n");
    out.push_str("\n\nclass SceneSection(NamedTuple):\n    \"\"\"シーンの区画の定義\"\"\"\n\n    # 名前\n    name: str\n    # 値の型（u8・u32・f32・f64）\n    type: str\n    # memoryview と array の形式の文字\n    format: str\n    # 行ごとの値の数\n    width: int\n");
    out.push_str(
        "\n\n# シーンの区画の定義。番号の順\nSCENE_SECTIONS: Tuple[SceneSection, ...] = (\n",
    );
    for section in &scene.sections {
        let format = COLUMNS
            .iter()
            .find(|(ty, _)| *ty == section.ty)
            .map(|(_, format)| *format)
            .ok_or_else(|| format!("区画 {} の型 {} を読めない", section.name, section.ty))?;
        comment(
            &mut out,
            "    ",
            &format!(
                "{}（{} × {}）",
                section.description, section.ty, section.width
            ),
        );
        let _ = writeln!(
            out,
            "    SceneSection({}, {}, {}, {}),",
            quote(&section.name),
            quote(&section.ty),
            quote(format),
            section.width
        );
    }
    out.push_str(")\n");

    out.push_str("\n# シーンの列に入れる値の組\nSCENE_VALUES: Mapping[str, Mapping[str, int]] = MappingProxyType(\n    {\n");
    for values in &scene.values {
        comment(&mut out, "        ", &values.description);
        let _ = writeln!(
            out,
            "        {}: MappingProxyType(\n            {{",
            quote(&values.name)
        );
        for item in &values.items {
            comment(&mut out, "                ", &item.description);
            let _ = writeln!(
                out,
                "                {}: {},",
                quote(&item.name),
                item.value
            );
        }
        out.push_str("            }\n        ),\n");
    }
    out.push_str("    }\n)\n");
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_objects_enums_and_operations() {
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
                    "from": { "description": "元", "$ref": "#/$defs/unit" },
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
            }"##
            .as_bytes(),
        )
        .unwrap();
        let types = render_types(&schema).unwrap();
        assert!(types.contains("_OpenRequestRequired = TypedDict(\n    \"_OpenRequestRequired\",\n    {\n        # 版\n        \"v\": int,\n        # op\n        \"op\": Literal[\"open\"],\n    },\n)"));
        assert!(types.contains("        \"from\": \"Unit\",\n        # 名前\n        \"names\": List[str],\n    },\n    total=False,\n)"));
        assert!(types.contains("class OpenRequest(_OpenRequestRequired, _OpenRequestOptional):"));
        assert!(types.contains("Unit = Union[\n    Literal[\n        \"meter\",\n        \"millimeter\",\n    ],\n    str,\n]"));
        assert!(types.contains("Request = Union[\n    \"OpenRequest\",\n]"));
        assert!(types.contains("    \"open\": (\"OpenRequest\", \"OpenResult\"),"));
        assert!(types.contains("class Response(_ResponseRequired, _ResponseOptional):"));
    }
}
