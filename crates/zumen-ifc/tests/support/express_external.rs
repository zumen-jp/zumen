//! 実行時に渡されたスキーマの観測口。スキーマと取得値はリポジトリへ同梱しない。

use super::*;
use zumen_base::{Budget, CancelToken, Diagnostics, Limits};
use zumen_json::Writer;

fn strings(out: &mut Writer, values: impl Iterator<Item = impl AsRef<str>>) {
    out.begin_array();
    for value in values {
        out.string(value.as_ref());
    }
    out.end_array();
}

fn ty(out: &mut Writer, value: &Type, schema: &Express) {
    out.begin_object();
    out.key("levels");
    out.begin_array();
    for level in &value.levels {
        out.begin_object();
        out.key("kind");
        out.string(level.kind);
        for (key, bound) in [("lower", level.lower), ("upper", level.upper)] {
            out.key(key);
            if let Some(number) = bound {
                out.uint(number);
            } else {
                out.null();
            }
        }
        out.end_object();
    }
    out.end_array();
    out.key("item");
    out.string(match value.item {
        Item::Integer => "INTEGER",
        Item::Real => "REAL",
        Item::Number => "NUMBER",
        Item::Boolean => "BOOLEAN",
        Item::Logical => "LOGICAL",
        Item::String => "STRING",
        Item::Binary => "BINARY",
        Item::Named(index) => &schema.decls[index].0,
    });
    out.end_object();
}

#[test]
#[allow(clippy::disallowed_methods)] // 実行時の外部スキーマと観測値だけを指定された一時領域で扱う。
fn writes_external_declarations() {
    let Ok(input) = std::env::var("ZUMEN_EXPRESS_SOURCE") else {
        return;
    };
    let output = std::env::var("ZUMEN_EXPRESS_OBSERVED").expect("観測先が必要");
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .canonicalize()
        .unwrap();
    let input = std::path::Path::new(&input).canonicalize().unwrap();
    let output = std::path::Path::new(&output);
    assert!(!input.starts_with(&root));
    assert!(
        !output
            .parent()
            .unwrap()
            .canonicalize()
            .unwrap()
            .starts_with(root)
    );
    assert!(!output.exists(), "観測値を上書きしない");
    assert!(std::fs::metadata(&input).unwrap().len() <= 4 * 1024 * 1024);
    let source = std::fs::read(input).unwrap();
    let mut budget = Budget::new(Limits::default());
    let mut diagnostics = Diagnostics::new();
    let cancel = CancelToken::new();
    let schema = Express::read(
        &source,
        &mut Cx::new(&mut budget, &mut diagnostics, &cancel),
    )
    .unwrap_or_else(|error| panic!("{error:?}: {diagnostics:?}"));
    assert!(diagnostics.is_empty());
    let mut out = Writer::new();
    out.begin_object();
    out.key("schema");
    out.string(schema.name());
    out.key("rulesNotEvaluated");
    out.uint(schema.rules());
    out.key("entities");
    out.begin_array();
    for (name, index) in &schema.symbols {
        let Some(Decl::Entity(entity)) = &schema.decls[*index].1 else {
            continue;
        };
        out.begin_object();
        out.key("name");
        out.string(name);
        out.key("abstract");
        out.bool(entity.is_abstract);
        out.key("supertypes");
        strings(
            &mut out,
            entity
                .supertypes
                .iter()
                .map(|index| &schema.decls[*index].0),
        );
        out.key("attributes");
        out.begin_array();
        for attribute in &entity.attributes {
            out.begin_object();
            out.key("name");
            out.string(&attribute.name);
            out.key("optional");
            out.bool(attribute.optional);
            out.key("type");
            ty(&mut out, &attribute.ty, &schema);
            out.end_object();
        }
        out.end_array();
        out.end_object();
    }
    out.end_array();
    out.end_object();
    std::fs::write(output, out.into_bytes()).unwrap();
}
