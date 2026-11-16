//! ZMS3 の配置を、2D と独立した定数として各言語へ出す。
use crate::scene::{self, Scene};
use std::{fmt::Write, fs, path::Path};

pub fn generate(root: &Path) -> Result<Vec<(&'static str, String)>, String> {
    let source = fs::read_to_string(root.join("schema/scene3d.toml")).map_err(|e| e.to_string())?;
    let model: Scene = toml::from_str(&source).map_err(|e| format!("schema/scene3d.toml: {e}"))?;
    let errors = scene::check_with_width(&model, 12);
    if !errors.is_empty() {
        return Err(format!(
            "schema/scene3d.toml に問題がある:\n- {}",
            errors.join("\n- ")
        ));
    }
    let rust = scene::render_rust_for(
        &model,
        "schema/scene3d.toml",
        "3D のシーンの配置（#417・#421）。",
        "ZMS3",
    );
    let header = scene::render_c_header(&model)
        .replace("schema/scene.toml", "schema/scene3d.toml")
        .replace("2D", "3D")
        .replace("ZMSC", "ZMS3")
        .replace("ZUMEN_SCENE", "ZUMEN_SCENE3D")
        .replace("ZM_SCENE", "ZM_SCENE3D")
        .replace("zm_scene", "zm_scene3d")
        .replace("scene の op", "scene3d の op")
        .replace("zumen::Scene", "zumen::Scene3D")
        .replace("決定 0137・0172・0178", "#417・#421");
    Ok(vec![
        (
            "crates/zumen-scene3d/src/layout.rs",
            crate::rustfmt(root, &rust)?,
        ),
        ("bindings/c/include/zumen_scene3d.h", header),
        ("bindings/js/src/scene3d-layout.js", javascript(&model)),
        ("bindings/js/src/scene3d-layout.d.ts", typescript(&model)),
        ("bindings/python/zumen/scene3d_layout.py", python(&model)),
        ("bindings/dotnet/Zumen/Scene3DLayout.g.cs", dotnet(&model)),
        ("bindings/go/scene3d_layout.go", go(&model)),
        (
            "bindings/java/src/main/java/dev/zumen/Scene3DLayout.java",
            java(&model),
        ),
        (
            "bindings/swift/Sources/Zumen/Scene3DLayout.swift",
            swift(&model),
        ),
        ("bindings/dart/lib/src/scene3d_layout.dart", dart(&model)),
        ("bindings/vba/ZumenScene3DLayout.bas", vba(&model)),
    ])
}

const GENERATED: &str =
    "// 生成したファイル。元は schema/scene3d.toml。cargo xtask codegen で作り直す。\n";

fn dotnet(model: &Scene) -> String {
    let mut out = format!(
        "{GENERATED}using System;\nusing System.Collections.Generic;\n\nnamespace Zumen {{\n    /// <summary>3D シーンの配置</summary>\n    public static class Scene3DLayout {{\n        /// <summary>ZMS3 の識別子</summary>\n        public const uint Magic = 0x33534d5a;\n        /// <summary>メジャーバージョン</summary>\n        public const int Major = {};\n        /// <summary>マイナーバージョン</summary>\n        public const int Minor = {};\n        /// <summary>参照なし</summary>\n        public const uint None = 0xffffffff;\n        /// <summary>セクションの定義</summary>\n        public static readonly IReadOnlyList<SceneSection> Sections = Array.AsReadOnly(new SceneSection[] {{\n",
        model.versions.major, model.versions.minor
    );
    for (i, s) in model.sections.iter().enumerate() {
        let ty = match s.ty.as_str() {
            "u8" => "byte",
            "u32" => "uint",
            "f32" => "float",
            _ => "double",
        };
        let _ = writeln!(
            out,
            "            // {}\n            new SceneSection({i}, \"{}\", typeof({ty}), {}),",
            s.description, s.name, s.width
        );
    }
    out.push_str("        });\n");
    for v in &model.values {
        for item in &v.items {
            let _ = writeln!(
                out,
                "        /// <summary>{}</summary>\n        public const uint {}{} = {};",
                item.description,
                crate::schema::pascal(&v.name),
                crate::schema::pascal(&item.name),
                item.value
            );
        }
    }
    out.push_str("    }\n}\n");
    out
}

fn go(model: &Scene) -> String {
    let mut out = format!(
        "{GENERATED}package zumen\n\nconst Scene3DMajor uint16 = {}\nconst Scene3DMinor uint16 = {}\nconst Scene3DNone uint32 = 0xffffffff\n\nvar Scene3DSections = []SceneSection{{\n",
        model.versions.major, model.versions.minor
    );
    for s in &model.sections {
        let _ = writeln!(
            out,
            "\t// {}\n\t{{Name: \"{}\", Type: Column{}, Width: {}}},",
            s.description,
            s.name,
            s.ty.to_uppercase(),
            s.width
        );
    }
    out.push_str("}\n");
    for v in &model.values {
        for item in &v.items {
            let _ = writeln!(
                out,
                "\nconst Scene3D{}{} uint32 = {}",
                crate::schema::pascal(&v.name),
                crate::schema::pascal(&item.name),
                item.value
            );
        }
    }
    out
}

fn java(model: &Scene) -> String {
    let mut out = format!(
        "{GENERATED}package dev.zumen;\n\n/** 3D シーンの配置 */\npublic final class Scene3DLayout {{\n    private Scene3DLayout() {{}}\n    public static final int MAJOR = {};\n    public static final int MINOR = {};\n    public static final int NONE = -1;\n    public static final Constants.SceneSection[] SECTIONS = {{\n",
        model.versions.major, model.versions.minor
    );
    for s in &model.sections {
        let _ = writeln!(
            out,
            "        // {}\n        new Constants.SceneSection(\"{}\", Constants.ColumnType.{}, {}),",
            s.description,
            s.name,
            s.ty.to_uppercase(),
            s.width
        );
    }
    out.push_str("    };\n");
    for v in &model.values {
        for item in &v.items {
            let _ = writeln!(
                out,
                "    public static final int {}_{} = {};",
                v.name.to_uppercase(),
                item.name.to_uppercase(),
                item.value
            );
        }
    }
    out.push_str("}\n");
    out
}

fn swift(model: &Scene) -> String {
    let mut out = format!(
        "{GENERATED}\n/// 3D シーンの配置\npublic enum Scene3DLayout {{\n    public static let major: UInt16 = {}\n    public static let minor: UInt16 = {}\n    public static let none = UInt32.max\n    public static let sections: [Constants.SceneSection] = [\n",
        model.versions.major, model.versions.minor
    );
    for s in &model.sections {
        let _ = writeln!(
            out,
            "        // {}\n        Constants.SceneSection(name: \"{}\", type: .{}, width: {}),",
            s.description, s.name, s.ty, s.width
        );
    }
    out.push_str("    ]\n");
    for v in &model.values {
        for item in &v.items {
            let _ = writeln!(
                out,
                "    public static let {}_{}: UInt32 = {}",
                v.name, item.name, item.value
            );
        }
    }
    out.push_str("}\n");
    out
}

fn camel(name: &str) -> String {
    let mut value = crate::schema::pascal(name);
    if let Some(first) = value.get_mut(..1) {
        first.make_ascii_lowercase();
    }
    value
}

fn dart(model: &Scene) -> String {
    let mut out = format!(
        "{GENERATED}import 'constants.dart';\n\n/// 3D シーンの配置\nabstract final class Scene3DLayout {{\n  static const major = {};\n  static const minor = {};\n  static const none = 0xffffffff;\n  static const sections = <SceneSection>[\n",
        model.versions.major, model.versions.minor
    );
    for s in &model.sections {
        let _ = writeln!(
            out,
            "    // {}\n    SceneSection('{}', ColumnType.{}, {}),",
            s.description, s.name, s.ty, s.width
        );
    }
    out.push_str("  ];\n");
    for v in &model.values {
        for item in &v.items {
            let _ = writeln!(
                out,
                "  static const {}{} = {};",
                camel(&v.name),
                crate::schema::pascal(&item.name),
                item.value
            );
        }
    }
    out.push_str("}\n");
    out
}

fn vba(model: &Scene) -> String {
    let mut out = format!(
        "Attribute VB_Name = \"ZumenScene3DLayout\"\nOption Explicit\n\n' Generated by cargo xtask codegen. Source: schema/scene3d.toml.\nPublic Const ZM_SCENE3D_MAJOR As Long = {}\nPublic Const ZM_SCENE3D_MINOR As Long = {}\nPublic Const ZM_SCENE3D_SECTION_COUNT As Long = {}\n",
        model.versions.major,
        model.versions.minor,
        model.sections.len()
    );
    for v in &model.values {
        for item in &v.items {
            let _ = writeln!(
                out,
                "Public Const ZM_SCENE3D_{}_{} As Long = {}",
                v.name.to_uppercase(),
                item.name.to_uppercase(),
                item.value
            );
        }
    }
    for suffix in ["Name", "Type", "Width"] {
        let result = if suffix == "Width" { "Long" } else { "String" };
        let _ = writeln!(
            out,
            "\nPublic Function ZmScene3DSection{suffix}(ByVal index As Long) As {result}\n    Select Case index"
        );
        for (i, s) in model.sections.iter().enumerate() {
            let value = match suffix {
                "Name" => format!("\"{}\"", s.name),
                "Width" => s.width.to_string(),
                _ => format!("\"{}\"", s.ty),
            };
            let _ = writeln!(out, "        Case {i}: ZmScene3DSection{suffix} = {value}");
        }
        out.push_str("        Case Else: Err.Raise 5, \"Zumen\", \"Unknown 3D scene section\"\n    End Select\nEnd Function\n");
    }
    out
}

fn javascript(model: &Scene) -> String {
    let mut out = String::from(
        "// 生成したファイル。元は schema/scene3d.toml。cargo xtask codegen で作り直す。\n\n",
    );
    let _ = writeln!(
        out,
        "export const SCENE3D_MAGIC = \"ZMS3\";\nexport const SCENE3D_MAJOR = {};\nexport const SCENE3D_MINOR = {};\nexport const SCENE3D_NONE = 4294967295;",
        model.versions.major, model.versions.minor
    );
    out.push_str("\nexport const SCENE3D_SECTIONS = Object.freeze([\n");
    for s in &model.sections {
        let _ = writeln!(
            out,
            "  // {}\n  Object.freeze({{ name: \"{}\", type: \"{}\", width: {} }}),",
            s.description, s.name, s.ty, s.width
        );
    }
    out.push_str("]);\n\nexport const SCENE3D_VALUES = Object.freeze({\n");
    for values in &model.values {
        let _ = writeln!(out, "  {}: Object.freeze({{", values.name);
        for item in &values.items {
            let _ = writeln!(out, "    {}: {},", item.name, item.value);
        }
        out.push_str("  }),\n");
    }
    out.push_str("});\n");
    out
}

fn typescript(model: &Scene) -> String {
    let mut out = String::from(
        "// 生成したファイル。元は schema/scene3d.toml。cargo xtask codegen で作り直す。\n\n",
    );
    let _ = writeln!(
        out,
        "export declare const SCENE3D_MAGIC: \"ZMS3\";\nexport declare const SCENE3D_MAJOR: {};\nexport declare const SCENE3D_MINOR: number;\nexport declare const SCENE3D_NONE: 4294967295;",
        model.versions.major
    );
    out.push_str("\nexport type Scene3DSectionName =\n");
    for s in &model.sections {
        let _ = writeln!(out, "  | \"{}\"", s.name);
    }
    out.push_str(";\n\nexport interface Scene3DSection {\n  readonly name: Scene3DSectionName;\n  readonly type: \"u8\" | \"u32\" | \"f32\" | \"f64\";\n  readonly width: number;\n}\n\nexport declare const SCENE3D_SECTIONS: readonly Scene3DSection[];\nexport declare const SCENE3D_VALUES: {\n");
    for values in &model.values {
        let _ = writeln!(out, "  readonly {}: {{", values.name);
        for item in &values.items {
            let _ = writeln!(out, "    readonly {}: {};", item.name, item.value);
        }
        out.push_str("  };\n");
    }
    out.push_str("};\n");
    out
}

fn python(model: &Scene) -> String {
    let mut out = String::from(
        "# 生成したファイル。元は schema/scene3d.toml。cargo xtask codegen で作り直す。\n\nfrom types import MappingProxyType\nfrom .constants import SceneSection\n\n",
    );
    let _ = writeln!(
        out,
        "SCENE3D_MAGIC = b\"ZMS3\"\nSCENE3D_MAJOR = {}\nSCENE3D_MINOR = {}\nSCENE3D_NONE = 4294967295\n\nSCENE3D_SECTIONS = (",
        model.versions.major, model.versions.minor
    );
    for s in &model.sections {
        let format = match s.ty.as_str() {
            "u8" => "B",
            "u32" => "I",
            "f32" => "f",
            "f64" => "d",
            _ => "",
        };
        let _ = writeln!(
            out,
            "    # {}\n    SceneSection(\"{}\", \"{}\", \"{}\", {}),",
            s.description, s.name, s.ty, format, s.width
        );
    }
    out.push_str(")\n\nSCENE3D_VALUES = MappingProxyType({\n");
    for values in &model.values {
        let _ = writeln!(out, "    \"{}\": MappingProxyType({{", values.name);
        for item in &values.items {
            let _ = writeln!(out, "        \"{}\": {},", item.name, item.value);
        }
        out.push_str("    }),\n");
    }
    out.push_str("})\n");
    out
}
