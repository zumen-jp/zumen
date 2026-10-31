//! .NET の包み（`bindings/dotnet`）の型と定数を作る（決定 0011・0196）。
//!
//! - `Messages.g.cs`: メッセージのスキーマ（`schema/messages.json`）の型
//! - `Constants.g.cs`: 境界の版・状態コード・関数の形（`schema/boundary.toml`）と、シーンの配置
//!   （`schema/scene.toml`）
//!
//! netstandard2.0（.NET Framework 4.8）と net8.0 の両方で読めるように書く。System.Text.Json は net8.0 の側にだけあるので、
//! 欄の名前の属性は netstandard2.0 では包みの中の代わりの属性で受け、形の決まらない値は `JsonElement` か `object` にする。

use std::collections::BTreeSet;
use std::fmt::Write;

use crate::boundary::Boundary;
use crate::scene::Scene;
use crate::schema::{Def, Field, Member, Schema, Shape, Ty, pascal};

/// op の共用体の定義の名前
const REQUEST: &str = "request";

/// 手で書く応答の定義の名前
const RESPONSE: &str = "response";

/// 生成したコードが使う型の名前。定義の名前と重ならないようにする
const RESERVED: &[&str] = &[
    "IReadOnlyList",
    "IReadOnlyDictionary",
    "Dictionary",
    "JsonAny",
    "JsonPropertyName",
    "JsonPropertyNameAttribute",
    "Operations",
];

/// record が持つメンバーの名前。欄の名前と重なったら、後ろに `Value` を付ける
const RECORD_MEMBERS: &[&str] = &[
    "EqualityContract",
    "Equals",
    "GetHashCode",
    "ToString",
    "GetType",
    "PrintMembers",
    "Deconstruct",
    "MemberwiseClone",
    "Finalize",
];

/// 境界の引数と戻り値の型と、C# の型
const CS_TYPES: &[(&str, &str)] = &[
    ("void", "void"),
    ("u32", "uint"),
    ("size", "nuint"),
    ("status", "int"),
    ("bytes", "byte*"),
    ("bytes_mut", "byte*"),
    ("session", "IntPtr"),
    ("session_out", "IntPtr*"),
    ("buf", "Buf*"),
    ("arrow_stream", "IntPtr"),
];

/// 区画の値の型と、C# の型
const COLUMNS: &[(&str, &str)] = &[
    ("u8", "byte"),
    ("u32", "uint"),
    ("f32", "float"),
    ("f64", "double"),
];

/// 文字列を C# の文字列の式にする
fn quote(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// XML の文書コメントの中で使えるようにする
fn xml(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// 説明を `/// <summary>` の文書コメントにする
fn summary(out: &mut String, indent: &str, text: &str) {
    let _ = writeln!(out, "{indent}/// <summary>");
    for line in text.lines() {
        let _ = writeln!(out, "{indent}/// {}", xml(line));
    }
    let _ = writeln!(out, "{indent}/// </summary>");
}

/// 名前を C# の識別子にする。英数字と `_` 以外は `_` に変え、数字で始まれば `_` を前に付ける
fn ident(text: &str) -> String {
    let mut name: String = pascal(text)
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    if name.is_empty() || name.starts_with(|c: char| c.is_ascii_digit()) {
        name.insert(0, '_');
    }
    name
}

/// メンバーの名前。型の名前や record のメンバーと重なれば、後ろに `Value` を付ける
fn member(text: &str, owner: &str) -> String {
    let name = ident(text);
    if name == owner || RECORD_MEMBERS.contains(&name.as_str()) {
        format!("{name}Value")
    } else {
        name
    }
}

/// 欄の型の式と、どの TFM でも値の型か（欠かせない欄に既定の値が要らないか）
fn cs_type(schema: &Schema, ty: &Ty) -> Result<(String, bool), String> {
    Ok(match ty {
        Ty::String | Ty::Const(_) => ("string".to_string(), false),
        Ty::Bool => ("bool".to_string(), true),
        Ty::UInt { max, .. } if *max <= u64::from(u32::MAX) => ("uint".to_string(), true),
        Ty::UInt { .. } => ("ulong".to_string(), true),
        Ty::Int { min, max } if *min >= i64::from(i32::MIN) && *max <= i64::from(i32::MAX) => {
            ("int".to_string(), true)
        }
        Ty::Int { .. } => ("long".to_string(), true),
        Ty::Float => ("double".to_string(), true),
        Ty::Ref(name) => {
            let def = schema
                .def(name)
                .ok_or_else(|| format!("定義 {name} がない"))?;
            match def.shape {
                Shape::Object { .. } => (pascal(name), false),
                Shape::Enum { .. } => ("string".to_string(), false),
                // 形が値で決まるので、読んだ側で目印を見て読み直す
                // netstandard2.0 では参照の型なので、欠かせない欄には既定の値を書く
                Shape::Union { .. } | Shape::Scalars { .. } | Shape::Handwritten => {
                    ("JsonAny".to_string(), false)
                }
            }
        }
        Ty::Array(items) => (
            format!("IReadOnlyList<{}>", cs_type(schema, items)?.0),
            false,
        ),
        Ty::Map(values) => (
            format!(
                "IReadOnlyDictionary<string, {}>",
                cs_type(schema, values)?.0
            ),
            false,
        ),
    })
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

/// 1 つの欄。`ty` は C# の型、`value` は値の型か、`init` は既定の値の式
struct Property<'a> {
    name: &'a str,
    description: &'a str,
    ty: String,
    value: bool,
    required: bool,
    init: Option<String>,
}

/// record を書く
fn render_record(
    out: &mut String,
    name: &str,
    generics: &str,
    description: &str,
    fields: &[Property],
) -> Result<(), String> {
    out.push('\n');
    summary(out, "    ", description);
    let _ = writeln!(out, "    public sealed record {name}{generics}\n    {{");
    let mut seen = BTreeSet::new();
    for (index, field) in fields.iter().enumerate() {
        let property = member(field.name, name);
        if !seen.insert(property.clone()) {
            return Err(format!(
                "{name} の欄 {} の名前が、ほかの欄と重なる",
                field.name
            ));
        }
        if index > 0 {
            out.push('\n');
        }
        summary(out, "        ", field.description);
        let _ = writeln!(out, "        [JsonPropertyName({})]", quote(field.name));
        let ty = if field.required {
            field.ty.clone()
        } else {
            format!("{}?", field.ty)
        };
        let init = match (&field.init, field.required, field.value) {
            (Some(init), _, _) => format!(" = {init};"),
            // 欠かせない参照の欄は、読んだときに埋まる
            (None, true, false) => " = default!;".to_string(),
            _ => String::new(),
        };
        let _ = writeln!(out, "        public {ty} {property} {{ get; init; }}{init}");
    }
    out.push_str("    }\n");
    Ok(())
}

/// 目印の値と、その形の型の表
fn render_variants(out: &mut String, schema: &Schema, name: &str, variants: &[String]) {
    let tags: Vec<(&str, &str, String)> = variants
        .iter()
        .filter_map(|variant| {
            let (field, value) = schema.tag_of(variant)?;
            Some((field, value, pascal(variant)))
        })
        .collect();
    let _ = writeln!(out, "    public static class {name}\n    {{");
    if let Some((field, _, _)) = tags.first() {
        let _ = writeln!(
            out,
            "        /// <summary>目印の欄の名前</summary>\n        public const string Tag = {};\n",
            quote(field)
        );
    }
    out.push_str("        /// <summary>目印の値と、その形の型</summary>\n        public static readonly IReadOnlyDictionary<string, global::System.Type> Variants = new Dictionary<string, global::System.Type>\n        {\n");
    for (_, value, ty) in &tags {
        let _ = writeln!(out, "            [{}] = typeof({ty}),", quote(value));
    }
    out.push_str("        };\n    }\n");
}

/// `bindings/dotnet/Zumen/Messages.g.cs` の中身を作る
pub fn render_types(schema: &Schema) -> Result<String, String> {
    if let Some(def) = schema
        .defs
        .iter()
        .find(|def| RESERVED.contains(&pascal(&def.name).as_str()))
    {
        return Err(format!(
            "schema/messages.json の {} が、.NET の包みが使う名前と重なる",
            def.name
        ));
    }
    let mut out = String::new();
    out.push_str("// 生成したファイル。手で直さない。\n");
    out.push_str("// 元は schema/messages.json で、`cargo xtask codegen` で作り直す。\n");
    out.push_str(
        "//\n// 境界のメッセージの型（決定 0012・0196）。スキーマの定義ごとに 1 つの型がある。\n",
    );
    out.push_str(
        "// 列挙は、知らない値も受け取れるように string で持ち、値を定数で出す（決定 0015）。\n",
    );
    out.push_str("// 形が値で決まるもの（共用体）は JsonAny（net8.0 は JsonElement、netstandard2.0 は object）で持ち、\n");
    out.push_str("// 目印の値から型を引いて読み直す。\n\n");
    out.push_str("#nullable enable\n\nusing System.Collections.Generic;\n#if NET\nusing System.Text.Json.Serialization;\nusing JsonAny = System.Text.Json.JsonElement;\n#else\nusing JsonAny = System.Object;\n#endif\n\nnamespace Zumen.Messages\n{\n");
    out.push_str("#if !NET\n    /// <summary>欄の JSON での名前。netstandard2.0 には System.Text.Json がないので、包みの中に代わりを置く</summary>\n    [global::System.AttributeUsage(global::System.AttributeTargets.Property)]\n    internal sealed class JsonPropertyNameAttribute : global::System.Attribute\n    {\n        public JsonPropertyNameAttribute(string name) => Name = name;\n\n        public string Name { get; }\n    }\n#endif\n");
    for def in &schema.defs {
        let name = pascal(&def.name);
        match &def.shape {
            Shape::Object { .. } => {
                let mut fields = Vec::new();
                collect_fields(schema, def, &mut fields, 0);
                let properties = fields
                    .iter()
                    .map(|field| {
                        let (ty, value) = cs_type(schema, &field.ty)?;
                        Ok(Property {
                            name: &field.name,
                            description: &field.description,
                            ty,
                            value,
                            required: field.required,
                            init: match &field.ty {
                                Ty::Const(value) => Some(quote(value)),
                                _ => None,
                            },
                        })
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                render_record(&mut out, &name, "", &def.description, &properties)?;
            }
            Shape::Enum { values } => {
                out.push('\n');
                summary(&mut out, "    ", &def.description);
                let _ = writeln!(out, "    public static class {name}\n    {{");
                let mut seen = BTreeSet::new();
                for value in values {
                    let constant = member(value, &name);
                    if !seen.insert(constant.clone()) {
                        return Err(format!(
                            "{} の値 {value} の名前が、ほかの値と重なる",
                            def.name
                        ));
                    }
                    let _ = writeln!(
                        out,
                        "        /// <summary>{}</summary>\n        public const string {constant} = {};",
                        xml(value),
                        quote(value)
                    );
                }
                out.push_str("    }\n");
            }
            Shape::Union { variants } => {
                out.push('\n');
                summary(&mut out, "    ", &def.description);
                render_variants(&mut out, schema, &name, variants);
            }
            Shape::Scalars { members } => {
                out.push('\n');
                let mut text = def.description.clone();
                for item in members {
                    let shape = match item {
                        Member::Integer { .. } => "整数".to_string(),
                        Member::Number { .. } => "数".to_string(),
                        Member::String { .. } => "文字列".to_string(),
                        Member::Const { value, .. } => format!("\"{value}\""),
                    };
                    let _ = write!(text, "\n- {shape}: {}", item.description());
                }
                summary(&mut out, "    ", &text);
                let _ = writeln!(out, "    public static class {name}\n    {{");
                for item in members {
                    if let Member::Const { value, description } = item {
                        summary(&mut out, "        ", description);
                        let _ = writeln!(
                            out,
                            "        public const string {} = {};",
                            member(value, &name),
                            quote(value)
                        );
                    }
                }
                out.push_str("    }\n");
            }
            Shape::Handwritten if def.name == RESPONSE => {
                for needed in ["diagnostic", "blob"] {
                    if schema.def(needed).is_none() {
                        return Err(format!("応答が使う定義 {needed} がない"));
                    }
                }
                let properties = [
                    Property {
                        name: "v",
                        description: "メッセージのスキーマの大版",
                        ty: "uint".to_string(),
                        value: true,
                        required: true,
                        init: None,
                    },
                    Property {
                        name: "diagnostics",
                        description: "診断。入力の順",
                        ty: "IReadOnlyList<Diagnostic>".to_string(),
                        value: false,
                        required: true,
                        init: None,
                    },
                    Property {
                        name: "blobs",
                        description: "バイナリの結果。Take で取り出す。次の Call で捨てる",
                        ty: "IReadOnlyList<Blob>".to_string(),
                        value: false,
                        required: true,
                        init: None,
                    },
                    Property {
                        name: "result",
                        description: "op ごとの結果（Operations が引く結果の型）。失敗したときは持たない",
                        ty: "TResult".to_string(),
                        value: false,
                        required: false,
                        init: None,
                    },
                ];
                render_record(&mut out, &name, "<TResult>", &def.description, &properties)?;
            }
            Shape::Handwritten => {}
        }
    }
    render_operations(&mut out, schema);
    out.push_str("}\n");
    Ok(out)
}

/// op の名前と、op ごとの要求と結果の型
fn render_operations(out: &mut String, schema: &Schema) {
    let Some(Shape::Union { variants }) = schema.def(REQUEST).map(|def| &def.shape) else {
        return;
    };
    out.push_str("\n    /// <summary>op ごとの要求と結果の型。結果は Response の Result の型</summary>\n    public static class Operations\n    {\n");
    out.push_str("        /// <summary>op の名前と、要求と結果の型</summary>\n        public static readonly IReadOnlyDictionary<string, (global::System.Type Request, global::System.Type Result)> Types =\n            new Dictionary<string, (global::System.Type Request, global::System.Type Result)>\n            {\n");
    for variant in variants {
        let Some((_, op)) = schema.tag_of(variant) else {
            continue;
        };
        let result_name = format!("{op}_result");
        let result = schema
            .def(&result_name)
            .map_or("JsonAny".to_string(), |_| pascal(&result_name));
        let _ = writeln!(
            out,
            "                [{}] = (typeof({}), typeof({result})),",
            quote(op),
            pascal(variant)
        );
    }
    out.push_str("            };\n    }\n");
}

/// `bindings/dotnet/Zumen/Constants.g.cs` の中身を作る
pub fn render_constants(
    version: &str,
    boundary: &Boundary,
    scene: &Scene,
) -> Result<String, String> {
    let mut out = String::new();
    out.push_str("// 生成したファイル。手で直さない。\n");
    out.push_str(
        "// 元は schema/boundary.toml と schema/scene.toml で、`cargo xtask codegen` で作り直す。\n\n",
    );
    out.push_str("#nullable enable\n\nusing System;\nusing System.Collections.Generic;\nusing System.Runtime.InteropServices;\n\nnamespace Zumen\n{\n");

    let versions = &boundary.versions;
    out.push_str(
        "    /// <summary>版（決定 0015）</summary>\n    public static class Versions\n    {\n",
    );
    let _ = writeln!(
        out,
        "        /// <summary>パッケージの版</summary>\n        public const string Package = {};\n",
        quote(version)
    );
    let _ = writeln!(
        out,
        "        /// <summary>ABI の大版</summary>\n        public const uint Abi = {};\n",
        versions.abi
    );
    let _ = writeln!(
        out,
        "        /// <summary>メッセージのスキーマの大版（要求と応答の v）</summary>\n        public const uint SchemaMajor = {};\n",
        versions.schema_major
    );
    let _ = writeln!(
        out,
        "        /// <summary>メッセージのスキーマの小版</summary>\n        public const uint SchemaMinor = {};",
        versions.schema_minor
    );
    out.push_str("    }\n\n");

    out.push_str("    /// <summary>状態コード。知らない値も、そのまま持てる</summary>\n    public enum Status\n    {\n");
    for status in &boundary.status {
        summary(&mut out, "        ", &status.description);
        let _ = writeln!(out, "        {} = {},", ident(&status.name), status.value);
    }
    out.push_str("    }\n\n");
    out.push_str("    /// <summary>状態コードの名前</summary>\n    public static class StatusNames\n    {\n        /// <summary>値から名前を引く。知らない値は unknown(値)</summary>\n        public static string Of(int code)\n        {\n            switch (code)\n            {\n");
    for status in &boundary.status {
        let _ = writeln!(
            out,
            "                case {}: return {};",
            status.value,
            quote(&status.name)
        );
    }
    out.push_str("                default: return \"unknown(\" + code + \")\";\n            }\n        }\n    }\n\n");

    out.push_str("    /// <summary>コアが返すバッファー（zm_buf）。zm_free で返す</summary>\n    [StructLayout(LayoutKind.Sequential)]\n    internal unsafe struct Buf\n    {\n        public byte* Ptr;\n        public nuint Len;\n    }\n\n");
    out.push_str("    /// <summary>ネイティブのライブラリが出す関数の形。32 bit の Windows では stdcall、ほかでは cdecl（Winapi）</summary>\n    internal static unsafe class NativeFunctions\n    {\n");
    let mut names = Vec::new();
    for function in &boundary.functions {
        if !function.targets.iter().any(|target| target == "native") {
            continue;
        }
        let cs = |name: &str| {
            CS_TYPES
                .iter()
                .find(|(ty, _)| *ty == name)
                .map(|(_, cs)| *cs)
                .ok_or_else(|| format!("{} の型 {name} を C# に直せない", function.name))
        };
        let returns = cs(&function.returns)?;
        let params = function
            .params
            .iter()
            // 引数の名前は C# の予約語（out など）と重なるので、@ を付ける
            .map(|param| Ok(format!("{} @{}", cs(&param.ty)?, param.name)))
            .collect::<Result<Vec<_>, String>>()?;
        summary(&mut out, "        ", &function.description);
        let _ = writeln!(
            out,
            "        [UnmanagedFunctionPointer(CallingConvention.Winapi)]\n        internal delegate {returns} {}({});\n",
            ident(&function.name),
            params.join(", ")
        );
        names.push(function.name.as_str());
    }
    out.push_str("        /// <summary>出すべき関数の名前</summary>\n        internal static readonly string[] Names =\n        {\n");
    for name in names {
        let _ = writeln!(out, "            {},", quote(name));
    }
    out.push_str("        };\n    }\n\n");

    out.push_str("    /// <summary>シーンの blob の配置（決定 0137）</summary>\n    public static class SceneLayout\n    {\n");
    out.push_str("        /// <summary>見出しの始まりの 4 バイト（ZMSC）</summary>\n        public const uint Magic = 0x4353_4D5A;\n\n");
    let _ = writeln!(
        out,
        "        /// <summary>大版。区画の意味を変えるときに上げる</summary>\n        public const ushort Major = {};\n",
        scene.versions.major
    );
    let _ = writeln!(
        out,
        "        /// <summary>小版。区画か値を足すたびに上げる</summary>\n        public const ushort Minor = {};\n",
        scene.versions.minor
    );
    out.push_str("        /// <summary>番号の「なし」</summary>\n        public const uint None = uint.MaxValue;\n\n");
    out.push_str("        /// <summary>区画の定義。番号の順</summary>\n        public static readonly IReadOnlyList<SceneSection> Sections = new[]\n        {\n");
    for (index, section) in scene.sections.iter().enumerate() {
        let ty = COLUMNS
            .iter()
            .find(|(ty, _)| *ty == section.ty)
            .map(|(_, cs)| *cs)
            .ok_or_else(|| format!("区画 {} の型 {} を読めない", section.name, section.ty))?;
        let _ = writeln!(
            out,
            "            // {}（{} × {}）\n            new SceneSection({index}, {}, typeof({ty}), {}),",
            section.description.lines().collect::<Vec<_>>().join(" "),
            section.ty,
            section.width,
            quote(&section.name),
            section.width
        );
    }
    out.push_str("        };\n\n");
    out.push_str("        /// <summary>列に入れる値の組。組の名前から、値の名前と値を引く</summary>\n        public static readonly IReadOnlyDictionary<string, IReadOnlyDictionary<string, uint>> Values =\n            new Dictionary<string, IReadOnlyDictionary<string, uint>>\n            {\n");
    for values in &scene.values {
        let _ = writeln!(
            out,
            "                // {}\n                [{}] = new Dictionary<string, uint>\n                {{",
            values.description.lines().collect::<Vec<_>>().join(" "),
            quote(&values.name)
        );
        for item in &values.items {
            let _ = writeln!(
                out,
                "                    // {}\n                    [{}] = {},",
                item.description.lines().collect::<Vec<_>>().join(" "),
                quote(&item.name),
                item.value
            );
        }
        out.push_str("                },\n");
    }
    out.push_str("            };\n    }\n}\n");
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_records_enums_and_operations() {
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
                "diagnostic": {
                  "description": "診断",
                  "type": "object",
                  "properties": { "code": { "description": "code", "type": "string" } },
                  "required": ["code"],
                  "additionalProperties": false
                },
                "blob": {
                  "description": "結果",
                  "type": "object",
                  "properties": { "id": { "description": "番号", "type": "integer", "minimum": 0, "maximum": 4294967295 } },
                  "required": ["id"],
                  "additionalProperties": false
                },
                "response": { "description": "応答" }
              }
            }"##
            .as_bytes(),
        )
        .unwrap();
        let types = render_types(&schema).unwrap();
        assert!(types.contains("    public sealed record OpenRequest\n    {"));
        assert!(types.contains("        [JsonPropertyName(\"op\")]\n        public string Op { get; init; } = \"open\";"));
        assert!(types.contains(
            "        [JsonPropertyName(\"from\")]\n        public string? From { get; init; }"
        ));
        assert!(types.contains("        public IReadOnlyList<string>? Names { get; init; }"));
        assert!(types.contains("        public const string Millimeter = \"millimeter\";"));
        assert!(types.contains("            [\"open\"] = typeof(OpenRequest),"));
        assert!(
            types.contains(
                "                [\"open\"] = (typeof(OpenRequest), typeof(OpenResult)),"
            )
        );
        assert!(types.contains("    public sealed record Response<TResult>\n"));
        assert!(types.contains("        public TResult? Result { get; init; }"));
    }

    #[test]
    fn makes_identifiers() {
        assert_eq!(ident("space_0"), "Space0");
        assert_eq!(ident("2d"), "_2d");
        assert_eq!(ident("a-b"), "A_b");
        assert_eq!(member("equals", "X"), "EqualsValue");
        assert_eq!(member("unit", "Unit"), "UnitValue");
    }
}
