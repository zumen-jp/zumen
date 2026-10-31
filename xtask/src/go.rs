//! Go の包み（`bindings/go`）の型と定数を作る（決定 0011・0226）。
//!
//! - `messages.go`: メッセージのスキーマ（`schema/messages.json`）の型
//! - `constants.go`: 境界の版・状態コード（`schema/boundary.toml`）と、シーンの配置（`schema/scene.toml`）
//!
//! 型は `encoding/json` で読み書きする構造体で、欄はどれも省ける（ポインターか、nil で省く型）。列挙は文字列の型で、
//! 知らない値も受け取る（決定 0015）。gofmt が整える形のまま書く。宣言と欄の間に空の行を置き、列をそろえさせない。

use std::collections::BTreeSet;
use std::fmt::Write;

use crate::boundary::Boundary;
use crate::scene::Scene;
use crate::schema::{Def, Field, Schema, Shape, Ty};

/// op の共用体の定義の名前
const REQUEST: &str = "request";

/// 手で書く応答の定義の名前
const RESPONSE: &str = "response";

/// 手で書いたコードの名前。生成する名前と重ならないようにする
const RESERVED: &[&str] = &[
    "Core",
    "Session",
    "Scene",
    "ZumenError",
    "ResultOf",
    "New",
    "NewFromWasm",
    "Ptr",
];

/// 区画の値の型と、`ColumnType` の定数の名前
const COLUMNS: &[(&str, &str)] = &[
    ("u8", "ColumnU8"),
    ("u32", "ColumnU32"),
    ("f32", "ColumnF32"),
    ("f64", "ColumnF64"),
];

/// 説明をコメントにする。gofmt が書き換える形（字下げ・箇条書きの印）を避け、各行を段落の文にする
fn comment(out: &mut String, indent: &str, text: &str) {
    for line in text.lines() {
        let line = line.trim();
        let line = line
            .strip_prefix("- ")
            .or_else(|| line.strip_prefix("* "))
            .map(|rest| format!("・{rest}"))
            .unwrap_or_else(|| line.to_string());
        if line.is_empty() {
            let _ = writeln!(out, "{indent}//");
        } else {
            let _ = writeln!(out, "{indent}// {line}");
        }
    }
}

/// 文字列を Go の文字列の式にする
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
                let _ = write!(out, "\\x{:02x}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// 名前を、外に出す Go の識別子にする（`from_unit` → `FromUnit`）
fn exported(name: &str) -> String {
    let mut out = String::new();
    for part in name
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|part| !part.is_empty())
    {
        let mut chars = part.chars();
        if let Some(first) = chars.next() {
            out.push(first.to_ascii_uppercase());
            out.extend(chars);
        }
    }
    if !out.starts_with(|c: char| c.is_ascii_alphabetic()) {
        out.insert_str(0, "Value");
    }
    out
}

/// 要素・値としての Go の型（ポインターにしない）
fn value_type(schema: &Schema, ty: &Ty) -> String {
    match ty {
        Ty::String | Ty::Const(_) => "string".to_string(),
        Ty::Bool => "bool".to_string(),
        Ty::UInt { .. } | Ty::Int { .. } => "int64".to_string(),
        Ty::Float => "float64".to_string(),
        Ty::Ref(name) => match schema.def(name).map(|def| &def.shape) {
            Some(Shape::Enum { .. } | Shape::Object { .. } | Shape::Union { .. }) => exported(name),
            Some(Shape::Handwritten) if name == RESPONSE => "Response".to_string(),
            _ => "json.RawMessage".to_string(),
        },
        Ty::Array(items) => format!("[]{}", value_type(schema, items)),
        Ty::Map(values) => format!("map[string]{}", value_type(schema, values)),
    }
}

/// 欄としての Go の型。nil で省けない型はポインターにする
fn field_type(schema: &Schema, ty: &Ty) -> String {
    if matches!(ty, Ty::Const(_)) {
        // 目印の欄は、MarshalJSON が入れる
        return "string".to_string();
    }
    let value = value_type(schema, ty);
    let nilable = value.starts_with("[]")
        || value.starts_with("map[")
        || value == "json.RawMessage"
        || matches!(ty, Ty::Ref(name) if matches!(schema.def(name).map(|def| &def.shape), Some(Shape::Union { .. })));
    if nilable { value } else { format!("*{value}") }
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

/// 名前の重なりを調べる
fn check_unique(what: &str, names: impl IntoIterator<Item = String>) -> Result<(), String> {
    let mut seen = BTreeSet::new();
    for name in names {
        if RESERVED.contains(&name.as_str()) || !seen.insert(name.clone()) {
            return Err(format!("{what}が、Go の名前 {name} で重なる"));
        }
    }
    Ok(())
}

/// 構造体の欄
fn render_field(out: &mut String, name: &str, description: &str, ty: &str, first: bool) {
    if !first {
        out.push('\n');
    }
    comment(out, "\t", description);
    let _ = writeln!(
        out,
        "\t{} {ty} `json:\"{},omitempty\"`",
        exported(name),
        name
    );
}

/// オブジェクトの定義の構造体。目印の欄は、書くときに入れる
fn render_object(out: &mut String, schema: &Schema, def: &Def) -> Result<(), String> {
    let name = exported(&def.name);
    let mut fields = Vec::new();
    collect_fields(schema, def, &mut fields, 0);
    check_unique(
        &format!("{} の欄", def.name),
        fields.iter().map(|field| exported(&field.name)),
    )?;
    out.push('\n');
    comment(out, "", &format!("{name}: {}", def.description));
    let _ = writeln!(out, "type {name} struct {{");
    for (index, field) in fields.iter().enumerate() {
        render_field(
            out,
            &field.name,
            &field.description,
            &field_type(schema, &field.ty),
            index == 0,
        );
    }
    out.push_str("}\n");
    let consts: Vec<(&str, &str)> = fields
        .iter()
        .filter_map(|field| match &field.ty {
            Ty::Const(value) => Some((field.name.as_str(), value.as_str())),
            _ => None,
        })
        .collect();
    if !consts.is_empty() {
        let _ = writeln!(
            out,
            "\n// MarshalJSON は、目印の欄を入れて書く。\nfunc (m {name}) MarshalJSON() ([]byte, error) {{\n\ttype plain {name}"
        );
        for (field, value) in consts {
            let _ = writeln!(out, "\tm.{} = {}", exported(field), quote(value));
        }
        out.push_str("\treturn json.Marshal(plain(m))\n}\n");
    }
    Ok(())
}

/// タグつきの共用体。読んだ JSON をそのまま持ち、形ごとの型に移すメソッドを持つ
fn render_union(out: &mut String, schema: &Schema, def: &Def, variants: &[String]) {
    let name = exported(&def.name);
    out.push('\n');
    comment(
        out,
        "",
        &format!(
            "{name}: {}\n\n読んだ JSON をそのまま持つ。形は As で始まるメソッドで読み、{name}Of で作る。",
            def.description
        ),
    );
    let _ = writeln!(out, "type {name} json.RawMessage");
    let _ = writeln!(
        out,
        "\n// {name}Of は、形の値を JSON にして {name} にする。\nfunc {name}Of(value any) ({name}, error) {{\n\tdata, err := json.Marshal(value)\n\treturn {name}(data), err\n}}"
    );
    let _ = writeln!(
        out,
        "\n// MarshalJSON は、持っている JSON をそのまま書く。\nfunc (u {name}) MarshalJSON() ([]byte, error) {{\n\treturn rawJSON(u), nil\n}}"
    );
    let _ = writeln!(
        out,
        "\n// UnmarshalJSON は、読んだ JSON をそのまま持つ。\nfunc (u *{name}) UnmarshalJSON(data []byte) error {{\n\t*u = append((*u)[:0], data...)\n\treturn nil\n}}"
    );
    for variant in variants {
        let Some((field, value)) = schema.tag_of(variant) else {
            continue;
        };
        let class = exported(variant);
        let _ = writeln!(
            out,
            "\n// As{class} は、{field} が {value} の形なら、その型で読む。違えば nil。\nfunc (u {name}) As{class}() *{class} {{\n\treturn asTagged[{class}](u, {}, {})\n}}",
            quote(field),
            quote(value)
        );
    }
}

/// 手で書く応答の構造体
fn render_response(out: &mut String, schema: &Schema, def: &Def) {
    out.push('\n');
    comment(
        out,
        "",
        &format!(
            "Response: {}\n\nop ごとの結果は、ResultOf でその op の結果の型で読む。",
            def.description
        ),
    );
    out.push_str("type Response struct {\n");
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
            "バイナリの結果。Take で取り出す。次の Call で捨てる",
            Ty::Array(Box::new(Ty::Ref("blob".to_string()))),
        ),
        (
            "result",
            "op ごとの結果（<op>_result の形の JSON のオブジェクト）。失敗したときは nil",
            Ty::Ref(String::new()),
        ),
    ];
    for (index, (field, description, ty)) in fields.iter().enumerate() {
        render_field(out, field, description, &field_type(schema, ty), index == 0);
    }
    out.push_str("}\n");
}

/// `bindings/go/messages.go` の中身を作る
pub fn render_types(schema: &Schema) -> Result<String, String> {
    // 型の名前・列挙の定数・op の定数・共用体を作る関数は、同じパッケージの名前の空間にある
    let mut names = Vec::new();
    for def in &schema.defs {
        match &def.shape {
            Shape::Object { .. } => names.push(exported(&def.name)),
            Shape::Union { .. } => {
                names.push(exported(&def.name));
                names.push(format!("{}Of", exported(&def.name)));
            }
            Shape::Enum { values } => {
                names.push(exported(&def.name));
                names.extend(
                    values
                        .iter()
                        .map(|value| format!("{}{}", exported(&def.name), exported(value))),
                );
            }
            Shape::Handwritten if def.name == RESPONSE => names.push("Response".to_string()),
            Shape::Scalars { .. } | Shape::Handwritten => {}
        }
    }
    names.extend(
        operations(schema)
            .iter()
            .map(|op| format!("Op{}", exported(op))),
    );
    check_unique("schema/messages.json の名前", names)?;

    let mut out = String::new();
    out.push_str("// Code generated by cargo xtask codegen. DO NOT EDIT.\n");
    out.push_str("// 元は schema/messages.json。\n");
    out.push_str("//\n// 境界のメッセージの型（決定 0012・0226）。スキーマの定義ごとに 1 つの型がある。欄はどれも省ける。\n// 列挙は文字列の型で、知らない値も受け取る（決定 0015）。\n");
    out.push_str("\npackage zumen\n\nimport \"encoding/json\"\n");
    for def in &schema.defs {
        match &def.shape {
            Shape::Object { .. } => render_object(&mut out, schema, def)?,
            Shape::Union { variants } => render_union(&mut out, schema, def, variants),
            Shape::Enum { values } => {
                let name = exported(&def.name);
                out.push('\n');
                comment(
                    &mut out,
                    "",
                    &format!(
                        "{name}: {}\n\n列挙の値。知らない値も受け取る。",
                        def.description
                    ),
                );
                let _ = writeln!(out, "type {name} string");
                for value in values {
                    let _ = writeln!(
                        out,
                        "\n// {name}{} は {}。\nconst {name}{} {name} = {}",
                        exported(value),
                        value,
                        exported(value),
                        quote(value)
                    );
                }
            }
            Shape::Handwritten if def.name == RESPONSE => render_response(&mut out, schema, def),
            Shape::Scalars { .. } | Shape::Handwritten => {}
        }
    }
    for op in operations(schema) {
        let _ = writeln!(
            out,
            "\n// Op{} は、op の名前 {op}。\nconst Op{} = {}",
            exported(op),
            exported(op),
            quote(op)
        );
    }
    Ok(out)
}

/// op の名前。要求の共用体の形の順
fn operations(schema: &Schema) -> Vec<&str> {
    let Some(Shape::Union { variants }) = schema.def(REQUEST).map(|def| &def.shape) else {
        return Vec::new();
    };
    variants
        .iter()
        .filter_map(|variant| schema.tag_of(variant).map(|(_, op)| op))
        .collect()
}

/// `bindings/go/constants.go` の中身を作る
pub fn render_constants(
    version: &str,
    boundary: &Boundary,
    scene: &Scene,
) -> Result<String, String> {
    check_unique(
        "状態コードの名前",
        boundary
            .status
            .iter()
            .map(|status| format!("Status{}", exported(&status.name))),
    )?;
    let mut out = String::new();
    out.push_str("// Code generated by cargo xtask codegen. DO NOT EDIT.\n");
    out.push_str("// 元は schema/boundary.toml と schema/scene.toml。\n");
    out.push_str("//\n// 境界の版・状態コードと、シーンの配置（決定 0012・0137・0226）。\n");
    out.push_str("\npackage zumen\n\nimport \"strconv\"\n");
    let versions = &boundary.versions;
    let _ = writeln!(
        out,
        "\n// Version は、パッケージの版（決定 0015）。\nconst Version = {}\n\n// ABIVersion は、ABI の大版。\nconst ABIVersion uint32 = {}\n\n// SchemaMajor は、メッセージのスキーマの大版（要求と応答の v）。\nconst SchemaMajor int64 = {}\n\n// SchemaMinor は、メッセージのスキーマの小版。\nconst SchemaMinor int64 = {}",
        quote(version),
        versions.abi,
        versions.schema_major,
        versions.schema_minor
    );
    for status in &boundary.status {
        out.push('\n');
        comment(
            &mut out,
            "",
            &format!("Status{}: {}", exported(&status.name), status.description),
        );
        let _ = writeln!(
            out,
            "const Status{} int32 = {}",
            exported(&status.name),
            status.value
        );
    }
    out.push_str("\n// StatusName は、状態コードの名前を返す。知らない値は unknown(<値>)。\nfunc StatusName(code int32) string {\n\tswitch code {\n");
    for status in &boundary.status {
        let _ = writeln!(
            out,
            "\tcase {}:\n\t\treturn {}",
            status.value,
            quote(&status.name)
        );
    }
    out.push_str("\t}\n\treturn \"unknown(\" + strconv.Itoa(int(code)) + \")\"\n}\n");

    let _ = writeln!(
        out,
        "\n// SceneMajor は、シーンの大版。区画の意味を変えるときに上げる。\nconst SceneMajor uint16 = {}\n\n// SceneMinor は、シーンの小版。区画か値を足すたびに上げる。\nconst SceneMinor uint16 = {}\n\n// SceneNone は、番号の「なし」（u32 の最大）。\nconst SceneNone uint32 = 0xffffffff",
        scene.versions.major, scene.versions.minor
    );
    out.push_str("\n// ColumnType は、シーンの区画の値の型。\ntype ColumnType int\n");
    for (index, (ty, name)) in COLUMNS.iter().enumerate() {
        let _ = writeln!(
            out,
            "\n// {name} は {ty}。\nconst {name} ColumnType = {index}"
        );
    }
    out.push_str("\n// Size は、値のバイト数。\nfunc (t ColumnType) Size() int {\n\tswitch t {\n\tcase ColumnU8:\n\t\treturn 1\n\tcase ColumnF64:\n\t\treturn 8\n\t}\n\treturn 4\n}\n");
    out.push_str("\n// SceneSection は、シーンの区画の定義。\ntype SceneSection struct {\n\t// Name は、名前。\n\tName string\n\n\t// Type は、値の型。\n\tType ColumnType\n\n\t// Width は、行ごとの値の数。\n\tWidth int\n}\n");
    out.push_str("\n// SceneSections は、シーンの区画の定義。番号の順。書き換えない。\nvar SceneSections = []SceneSection{\n");
    for (index, section) in scene.sections.iter().enumerate() {
        let ty = COLUMNS
            .iter()
            .find(|(ty, _)| *ty == section.ty)
            .map(|(_, name)| *name)
            .ok_or_else(|| format!("区画 {} の型 {} を読めない", section.name, section.ty))?;
        if index > 0 {
            out.push('\n');
        }
        comment(
            &mut out,
            "\t",
            &format!(
                "{}（{} × {}）",
                section.description, section.ty, section.width
            ),
        );
        let _ = writeln!(
            out,
            "\t{{Name: {}, Type: {ty}, Width: {}}},",
            quote(&section.name),
            section.width
        );
    }
    out.push_str("}\n");

    out.push_str("\n// SceneValues は、シーンの列に入れる値の組。組の名前から、値の名前と値の表を引く。書き換えない。\nvar SceneValues = map[string]map[string]uint32{\n");
    for (index, values) in scene.values.iter().enumerate() {
        if index > 0 {
            out.push('\n');
        }
        let _ = writeln!(
            out,
            "\t// {}\n\t{}: {{",
            values.description.lines().next().unwrap_or(""),
            quote(&values.name)
        );
        for (index, item) in values.items.iter().enumerate() {
            if index > 0 {
                out.push('\n');
            }
            let _ = writeln!(out, "\t\t{}: {},", quote(&item.name), item.value);
        }
        out.push_str("\t},\n");
    }
    out.push_str("}\n");
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_structs_enums_and_operations() {
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
                    "from_unit": { "description": "元\n- 箇条書き", "$ref": "#/$defs/unit" },
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
        assert!(
            types.contains(
                "type OpenRequest struct {\n\t// 版\n\tV *int64 `json:\"v,omitempty\"`\n"
            )
        );
        assert!(types.contains("\tFromUnit *Unit `json:\"from_unit,omitempty\"`\n"));
        assert!(types.contains("\t// ・箇条書き\n"));
        assert!(types.contains("\tNames []string `json:\"names,omitempty\"`\n"));
        assert!(types.contains("\tm.Op = \"open\"\n"));
        assert!(types.contains("const UnitOpen Unit = \"open\""));
        assert!(types.contains("type Request json.RawMessage"));
        assert!(types.contains("func (u Request) AsOpenRequest() *OpenRequest {"));
        assert!(types.contains("const OpOpen = \"open\""));
        assert!(types.contains("\tResult json.RawMessage `json:\"result,omitempty\"`\n"));
    }

    #[test]
    fn rejects_names_that_meet_in_go() {
        assert_eq!(exported("bcf_read"), "BcfRead");
        assert_eq!(exported("2d"), "Value2d");
        assert!(check_unique("値", ["A".to_string(), "A".to_string()]).is_err());
        assert!(check_unique("値", ["Session".to_string()]).is_err());
    }
}
