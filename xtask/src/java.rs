//! Java の包み（`bindings/java`）の型と定数を作る（決定 0011・0224）。
//!
//! - `Messages.java`: メッセージのスキーマ（`schema/messages.json`）の型
//! - `Constants.java`: 境界の版・状態コード（`schema/boundary.toml`）と、シーンの配置（`schema/scene.toml`）
//!
//! Java 8 と Android 7.0（docs/support.md の下限）で読めるように、標準の `java.util` だけを使う。型は、読んだ JSON
//! （`Map`・`List`・`String`・`Long`・`Double`・`Boolean`）を包む見方で、知らない欄と値もそのまま持つ（決定 0015）。

use std::fmt::Write;

use crate::boundary::Boundary;
use crate::scene::Scene;
use crate::schema::{Def, Field, Schema, Shape, Ty, pascal};

/// op の共用体の定義の名前
const REQUEST: &str = "request";

/// 手で書く応答の定義の名前
const RESPONSE: &str = "response";

/// 生成したコードが単純名で使う型の名前。定義の名前と重ならないようにする
const RESERVED: &[&str] = &[
    "String",
    "Object",
    "Long",
    "Double",
    "Boolean",
    "Map",
    "List",
    "LinkedHashMap",
    "Json",
    "Message",
    "Messages",
    "Operations",
    "Constants",
];

/// 区画の値の型と、`Scene` の列の型
const COLUMNS: &[(&str, &str)] = &[("u8", "U8"), ("u32", "U32"), ("f32", "F32"), ("f64", "F64")];

/// 説明を Javadoc にする
fn javadoc(out: &mut String, indent: &str, text: &str) {
    let text = text.replace("*/", "*&#47;");
    let mut lines = text.lines();
    let Some(first) = lines.next() else {
        return;
    };
    let rest: Vec<&str> = lines.collect();
    if rest.is_empty() {
        let _ = writeln!(out, "{indent}/** {first} */");
        return;
    }
    let _ = writeln!(out, "{indent}/**");
    let _ = writeln!(out, "{indent} * {first}");
    for line in rest {
        let _ = writeln!(out, "{indent} * {line}");
    }
    let _ = writeln!(out, "{indent} */");
}

/// 文字列を Java の文字列の式にする
fn quote(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            c if u32::from(c) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// 欄の名前を、メソッドの名前の後ろの部分にする（`from_unit` → `FromUnit`）
fn member(name: &str) -> String {
    let name = pascal(name);
    if !name.chars().next().is_some_and(|c| c.is_ascii_alphabetic()) {
        format!("Value{name}")
    } else if name == "Class" {
        // Object の getClass と重なる
        "ClassValue".to_string()
    } else {
        name
    }
}

/// 名前を定数の名前にする（`bcf_read` → `BCF_READ`）
fn constant(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_uppercase());
        } else {
            out.push('_');
        }
    }
    if out.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        out.insert(0, '_');
    }
    out
}

/// 欄の型の、Java の型と、JSON の値から読む関数の式
fn java_type(schema: &Schema, ty: &Ty) -> (String, String) {
    java_type_at(schema, ty, 0)
}

/// `java_type` の本体。入れ子のラムダの引数の名前は、深さで分ける
fn java_type_at(schema: &Schema, ty: &Ty, depth: usize) -> (String, String) {
    match ty {
        Ty::String | Ty::Const(_) => ("String".to_string(), "Json::string".to_string()),
        Ty::Bool => ("Boolean".to_string(), "Json::bool".to_string()),
        Ty::UInt { .. } | Ty::Int { .. } => ("Long".to_string(), "Json::integer".to_string()),
        Ty::Float => ("Double".to_string(), "Json::number".to_string()),
        Ty::Ref(name) => match schema.def(name).map(|def| &def.shape) {
            Some(Shape::Enum { .. }) => ("String".to_string(), "Json::string".to_string()),
            Some(Shape::Object { .. } | Shape::Union { .. }) => {
                let class = pascal(name);
                (class.clone(), format!("{class}::of"))
            }
            Some(Shape::Handwritten) if name == RESPONSE => {
                ("Response".to_string(), "Response::of".to_string())
            }
            _ => ("Object".to_string(), "Json::value".to_string()),
        },
        Ty::Array(items) => {
            let (item, read) = java_type_at(schema, items, depth + 1);
            (
                format!("List<{item}>"),
                format!("v{depth} -> Json.list(v{depth}, {read})"),
            )
        }
        Ty::Map(values) => {
            let (value, read) = java_type_at(schema, values, depth + 1);
            (
                format!("Map<String, {value}>"),
                format!("v{depth} -> Json.map(v{depth}, {read})"),
            )
        }
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

/// 見方のクラスの始まり。`of` と、JSON の値を受ける作り方を置く
fn class_head(out: &mut String, name: &str, description: &str) {
    out.push('\n');
    javadoc(out, "    ", description);
    let _ = writeln!(
        out,
        "    public static final class {name} extends Message {{"
    );
    let _ = writeln!(
        out,
        "        /** 読んだ JSON のオブジェクトを包む */\n        public {name}(Map<String, Object> json) {{\n            super(json);\n        }}\n"
    );
    let _ = writeln!(
        out,
        "        /** JSON の値を包む。オブジェクトでなければ null */\n        public static {name} of(Object value) {{\n            Map<String, Object> json = Json.object(value);\n            return json == null ? null : new {name}(json);\n        }}"
    );
}

/// 欄ごとの読み書きのメソッド
fn render_field(
    out: &mut String,
    class: &str,
    name: &str,
    description: &str,
    ty: &str,
    read: &str,
) {
    let method = member(name);
    let key = quote(name);
    out.push('\n');
    javadoc(out, "        ", description);
    let _ = writeln!(
        out,
        "        public {ty} get{method}() {{\n            return Json.read(json.get({key}), {read});\n        }}"
    );
    let _ = writeln!(
        out,
        "\n        /** {{@link #get{method}()}} を書く。null なら欄を除く */\n        public {class} with{method}({ty} value) {{\n            put({key}, value);\n            return this;\n        }}"
    );
}

/// オブジェクトの定義のクラス
fn render_object(out: &mut String, schema: &Schema, def: &Def) {
    let name = pascal(&def.name);
    class_head(out, &name, &def.description);
    let mut fields = Vec::new();
    collect_fields(schema, def, &mut fields, 0);
    let consts: Vec<(&str, &str)> = fields
        .iter()
        .filter_map(|field| match &field.ty {
            Ty::Const(value) => Some((field.name.as_str(), value.as_str())),
            _ => None,
        })
        .collect();
    out.push('\n');
    if consts.is_empty() {
        let _ = writeln!(
            out,
            "        /** 空のオブジェクトから作る */\n        public {name}() {{\n            super(new LinkedHashMap<String, Object>());\n        }}"
        );
    } else {
        let _ = writeln!(
            out,
            "        /** 目印の欄だけを持つオブジェクトから作る */\n        public {name}() {{\n            super(new LinkedHashMap<String, Object>());"
        );
        for (field, value) in &consts {
            let _ = writeln!(
                out,
                "            json.put({}, {});",
                quote(field),
                quote(value)
            );
        }
        out.push_str("        }\n");
    }
    for field in fields {
        let (ty, read) = java_type(schema, &field.ty);
        render_field(out, &name, &field.name, &field.description, &ty, &read);
    }
    out.push_str("    }\n");
}

/// タグつきの共用体のクラス。形ごとの見方に移すメソッドを持つ
fn render_union(out: &mut String, schema: &Schema, def: &Def, variants: &[String]) {
    let name = pascal(&def.name);
    class_head(out, &name, &def.description);
    for variant in variants {
        let class = pascal(variant);
        let Some((field, value)) = schema.tag_of(variant) else {
            continue;
        };
        let _ = writeln!(
            out,
            "\n        /** {} が {} の形なら、その見方。違えば null */\n        public {class} as{class}() {{\n            return {}.equals(json.get({})) ? new {class}(json) : null;\n        }}",
            field,
            value,
            quote(value),
            quote(field)
        );
    }
    out.push_str("    }\n");
}

/// 手で書く応答のクラス
fn render_response(out: &mut String, schema: &Schema, def: &Def) {
    let name = pascal(&def.name);
    class_head(out, &name, &def.description);
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
    ];
    for (field, description, ty) in &fields {
        let (ty, read) = java_type(schema, ty);
        render_field(out, &name, field, description, &ty, &read);
    }
    out.push_str("\n        /** op ごとの結果（<op>_result の形の JSON のオブジェクト）。失敗したときは null */\n        public Map<String, Object> getResult() {\n            return Json.object(json.get(\"result\"));\n        }\n");
    out.push_str("\n        /**\n         * op ごとの結果を、その op の結果の型で読む（例: {@code response.result(OpenResult::of)}）\n         *\n         * 失敗したときは null\n         */\n        public <T> T result(java.util.function.Function<Object, T> read) {\n            return Json.read(json.get(\"result\"), read);\n        }\n");
    out.push_str("    }\n");
}

/// `bindings/java/src/main/java/dev/zumen/Messages.java` の中身を作る
pub fn render_types(schema: &Schema) -> Result<String, String> {
    if let Some(def) = schema
        .defs
        .iter()
        .find(|def| RESERVED.contains(&pascal(&def.name).as_str()))
    {
        return Err(format!(
            "schema/messages.json の {} が、Java の包みが使う型の名前と重なる",
            def.name
        ));
    }
    let mut out = String::new();
    out.push_str("// 生成したファイル。手で直さない。\n");
    out.push_str("// 元は schema/messages.json で、`cargo xtask codegen` で作り直す。\n");
    out.push_str("\npackage dev.zumen;\n\nimport java.util.LinkedHashMap;\nimport java.util.List;\nimport java.util.Map;\n\n");
    out.push_str("/**\n * 境界のメッセージの型（決定 0012・0224）。スキーマの定義ごとに 1 つの型がある。\n *\n * <p>型は、JSON のオブジェクト（{@link Map}）を包む見方で、知らない欄もそのまま持つ。列挙は文字列で読み、\n * 知らない値も受け取る（決定 0015）。欄は {@code getXxx} で読み（ない欄と形の違う値は null）、{@code withXxx} で書く。\n */\npublic final class Messages {\n    private Messages() {}\n");
    out.push_str("\n    /** メッセージの型が継ぐ、JSON のオブジェクトの見方 */\n    public abstract static class Message {\n        /** 包んでいる JSON のオブジェクト */\n        protected final Map<String, Object> json;\n\n        protected Message(Map<String, Object> json) {\n            if (json == null) {\n                throw new NullPointerException(\"json\");\n            }\n            this.json = json;\n        }\n\n        /** 包んでいる JSON のオブジェクト。書き換えると、この見方からも見える */\n        public Map<String, Object> json() {\n            return json;\n        }\n\n        /** 欄を書く。null なら欄を除く。メッセージの型と、その列・表は JSON の値に直して持つ */\n        protected void put(String key, Object value) {\n            if (value == null) {\n                json.remove(key);\n            } else {\n                json.put(key, Json.unwrap(value));\n            }\n        }\n\n        /** JSON の文字列にする */\n        @Override\n        public String toString() {\n            return Json.write(json);\n        }\n    }\n");
    for def in &schema.defs {
        match &def.shape {
            Shape::Object { .. } => render_object(&mut out, schema, def),
            Shape::Union { variants } => render_union(&mut out, schema, def, variants),
            Shape::Enum { values } => {
                out.push('\n');
                javadoc(
                    &mut out,
                    "    ",
                    &format!("{}\n\n列挙の値。知らない値も受け取る", def.description),
                );
                let _ = writeln!(
                    out,
                    "    public static final class {} {{\n        private {}() {{}}",
                    pascal(&def.name),
                    pascal(&def.name)
                );
                for value in values {
                    let _ = writeln!(
                        out,
                        "\n        public static final String {} = {};",
                        constant(value),
                        quote(value)
                    );
                }
                out.push_str("    }\n");
            }
            Shape::Handwritten if def.name == RESPONSE => render_response(&mut out, schema, def),
            Shape::Scalars { .. } | Shape::Handwritten => {}
        }
    }
    render_operations(&mut out, schema);
    out.push_str("}\n");
    Ok(out)
}

/// op の名前
fn render_operations(out: &mut String, schema: &Schema) {
    let Some(Shape::Union { variants }) = schema.def(REQUEST).map(|def| &def.shape) else {
        return;
    };
    out.push_str("\n    /** op の名前。要求の型は {@code <Op>Request}、結果の型は {@code <Op>Result} */\n    public static final class Operations {\n        private Operations() {}\n");
    for variant in variants {
        if let Some((_, op)) = schema.tag_of(variant) {
            let _ = writeln!(
                out,
                "\n        public static final String {} = {};",
                constant(op),
                quote(op)
            );
        }
    }
    out.push_str("    }\n");
}

/// `bindings/java/src/main/java/dev/zumen/Constants.java` の中身を作る
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
    out.push_str("\npackage dev.zumen;\n\nimport java.util.Collections;\nimport java.util.LinkedHashMap;\nimport java.util.Map;\n\n/** 境界の版・状態コードと、シーンの配置（決定 0012・0137・0224） */\npublic final class Constants {\n    private Constants() {}\n");
    let versions = &boundary.versions;
    let _ = writeln!(
        out,
        "\n    /** パッケージの版（決定 0015） */\n    public static final String VERSION = {};",
        quote(version)
    );
    let _ = writeln!(
        out,
        "\n    /** ABI の大版 */\n    public static final int ABI_VERSION = {};",
        versions.abi
    );
    let _ = writeln!(
        out,
        "\n    /** メッセージのスキーマの大版（要求と応答の v） */\n    public static final int SCHEMA_MAJOR = {};",
        versions.schema_major
    );
    let _ = writeln!(
        out,
        "\n    /** メッセージのスキーマの小版 */\n    public static final int SCHEMA_MINOR = {};",
        versions.schema_minor
    );

    out.push_str("\n    /** 状態コード */\n    public static final class Status {\n        private Status() {}\n");
    for status in &boundary.status {
        out.push('\n');
        javadoc(&mut out, "        ", &status.description);
        let _ = writeln!(
            out,
            "        public static final int {} = {};",
            constant(&status.name),
            status.value
        );
    }
    out.push_str("\n        /** 状態コードの名前。知らない値は {@code unknown(<値>)} */\n        public static String name(int code) {\n            switch (code) {\n");
    for status in &boundary.status {
        let _ = writeln!(
            out,
            "                case {}:\n                    return {};",
            status.value,
            quote(&status.name)
        );
    }
    out.push_str("                default:\n                    return \"unknown(\" + code + \")\";\n            }\n        }\n    }\n");

    let _ = writeln!(
        out,
        "\n    /** シーンの大版。区画の意味を変えるときに上げる */\n    public static final int SCENE_MAJOR = {};",
        scene.versions.major
    );
    let _ = writeln!(
        out,
        "\n    /** シーンの小版。区画か値を足すたびに上げる */\n    public static final int SCENE_MINOR = {};",
        scene.versions.minor
    );
    out.push_str("\n    /** 番号の「なし」（u32 の最大）を int で読んだ値 */\n    public static final int SCENE_NONE = -1;\n");

    out.push_str("\n    /** シーンの区画の値の型 */\n    public enum ColumnType {\n        /** u8 */\n        U8(1),\n        /** u32 */\n        U32(4),\n        /** f32 */\n        F32(4),\n        /** f64 */\n        F64(8);\n\n        /** 値のバイト数 */\n        public final int size;\n\n        ColumnType(int size) {\n            this.size = size;\n        }\n    }\n");
    out.push_str("\n    /** シーンの区画の定義 */\n    public static final class SceneSection {\n        /** 名前 */\n        public final String name;\n        /** 値の型 */\n        public final ColumnType type;\n        /** 行ごとの値の数 */\n        public final int width;\n\n        SceneSection(String name, ColumnType type, int width) {\n            this.name = name;\n            this.type = type;\n            this.width = width;\n        }\n    }\n");
    out.push_str("\n    /** シーンの区画の定義。番号の順。書き換えない */\n    public static final SceneSection[] SCENE_SECTIONS = {\n");
    for section in &scene.sections {
        let ty = COLUMNS
            .iter()
            .find(|(ty, _)| *ty == section.ty)
            .map(|(_, ty)| *ty)
            .ok_or_else(|| format!("区画 {} の型 {} を読めない", section.name, section.ty))?;
        javadoc(
            &mut out,
            "        ",
            &format!(
                "{}（{} × {}）",
                section.description, section.ty, section.width
            ),
        );
        let _ = writeln!(
            out,
            "        new SceneSection({}, ColumnType.{ty}, {}),",
            quote(&section.name),
            section.width
        );
    }
    out.push_str("    };\n");

    out.push_str("\n    /** シーンの列に入れる値の組。組の名前から、値の名前と値の表を引く */\n    public static final Map<String, Map<String, Integer>> SCENE_VALUES;\n\n    static {\n        Map<String, Map<String, Integer>> values = new LinkedHashMap<String, Map<String, Integer>>();\n        Map<String, Integer> items;\n");
    for values in &scene.values {
        let _ = writeln!(
            out,
            "        // {}\n        items = new LinkedHashMap<String, Integer>();",
            values.description.lines().next().unwrap_or("")
        );
        for item in &values.items {
            let _ = writeln!(
                out,
                "        items.put({}, {});",
                quote(&item.name),
                item.value
            );
        }
        let _ = writeln!(
            out,
            "        values.put({}, Collections.unmodifiableMap(items));",
            quote(&values.name)
        );
    }
    out.push_str("        SCENE_VALUES = Collections.unmodifiableMap(values);\n    }\n}\n");
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
        assert!(types.contains("    public static final class OpenRequest extends Message {"));
        assert!(types.contains("            json.put(\"op\", \"open\");"));
        assert!(types.contains("        public Long getV() {\n            return Json.read(json.get(\"v\"), Json::integer);"));
        assert!(types.contains("        public String getFrom() {"));
        assert!(types.contains("        public List<String> getNames() {\n            return Json.read(json.get(\"names\"), v0 -> Json.list(v0, Json::string));"));
        assert!(types.contains("        public OpenRequest withNames(List<String> value) {"));
        assert!(types.contains("        public static final String MILLIMETER = \"millimeter\";"));
        assert!(types.contains("        public OpenRequest asOpenRequest() {"));
        assert!(types.contains("        public static final String OPEN = \"open\";"));
        assert!(types.contains("    public static final class Response extends Message {"));
        assert!(types.contains("        public Long getCount() {"));
    }
}
