//! 図面の試料を比較するときの共通の処理。

/// 図面モデルの Debug 表示から、出所の `Ref` の値だけを除く。
/// 文字列の中の同じ文字並びは図面の内容なので残す。出所の有無と個数も変えない。
pub fn without_source_refs(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut quoted = false;
    let mut escaped = false;
    let mut skipped = false;
    let mut depth = 0usize;
    for (at, ch) in text.char_indices() {
        if !quoted
            && !skipped
            && text
                .get(at..)
                .is_some_and(|rest| rest.starts_with("Ref { "))
            && !text
                .get(..at)
                .and_then(|before| before.chars().next_back())
                .is_some_and(|before| before.is_alphanumeric() || before == '_')
        {
            out.push_str("Ref");
            skipped = true;
        }
        if !skipped {
            out.push(ch);
        }
        if escaped {
            escaped = false;
        } else if quoted && ch == '\\' {
            escaped = true;
        } else if ch == '"' {
            quoted = !quoted;
        } else if skipped && !quoted {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth = depth.saturating_sub(1);
                    skipped = depth != 0;
                }
                _ => {}
            }
        }
    }
    out
}

#[test]
fn source_normalization_keeps_drawing_text_numbers_and_reference_counts() {
    use zumen_base::Ref;
    let dump =
        |source: Ref, x: f64, text: &str| format!("{:?}", (source.clone(), vec![source], x, text));
    let text = "Ref { 9 } \"文字\"";
    let first = dump(Ref::at_record(1), 1.0, text);
    let second = dump(Ref::at_bytes(10, 20).with_id("} \""), 1.0, text);
    let normalized = without_source_refs(&first);
    assert_eq!(normalized, without_source_refs(&second));
    assert!(normalized.contains(&format!("{text:?}")));
    assert_ne!(
        normalized,
        without_source_refs(&dump(Ref::at_record(1), 2.0, text))
    );
    assert_ne!(
        normalized,
        without_source_refs(&dump(Ref::at_record(1), 1.0, "Ref { 10 }"))
    );
    assert_ne!(
        without_source_refs(&format!("{:?}", vec![Ref::at_record(1)])),
        without_source_refs(&format!("{:?}", vec![Ref::at_record(1), Ref::at_record(2)]))
    );
}

/// 拡張を、付く先と契約のキーごとに比べる。変換の順は形式ごとに違ってよい。
/// XRECORD のハンドルは、DXF を書くときに振り直すことがあるので値だけを 0 にする。
/// ハンドル以外の型と値、項目の順序、重複する拡張の個数は保つ。
pub fn normalize_extensions(drawing: &mut zumen_drawing::Drawing2D) {
    use zumen_drawing::FieldValue;
    for extension in &mut drawing.extensions {
        if extension.namespace == "dxf.xrecord" {
            for field in &mut extension.fields {
                if let FieldValue::Handle(handle) = &mut field.value {
                    *handle = 0;
                }
            }
        }
    }
    drawing.extensions.sort_by_cached_key(extension_key);
}

#[test]
fn extension_normalization_keeps_types_values_and_counts() {
    use zumen_base::Ref;
    use zumen_drawing::{Basis, Drawing2D, Extension, Field, FieldValue, Target};
    let extension = |name: &str, value| Extension {
        namespace: "dxf.xrecord",
        name: name.to_owned(),
        version: None,
        schema: 1,
        target: Target::Document,
        basis: Basis::Document,
        fields: vec![Field {
            key: "value",
            code: None,
            value,
        }],
        source: Ref::default(),
    };
    let mut first = Drawing2D {
        extensions: vec![
            extension("INTEGER", FieldValue::Integer(1)),
            extension("HANDLE", FieldValue::Handle(20)),
        ],
        ..Drawing2D::default()
    };
    let mut second = first.clone();
    second.extensions.reverse();
    second.extensions[0].fields[0].value = FieldValue::Handle(30);
    normalize_extensions(&mut first);
    normalize_extensions(&mut second);
    assert_eq!(first, second);
    let mut changed = second.clone();
    changed.extensions[1].fields[0].value = FieldValue::Real(1.0);
    assert_ne!(first, changed);
    changed.extensions[1].fields[0].value = FieldValue::Integer(2);
    assert_ne!(first, changed);
    changed = second.clone();
    changed.extensions[0].namespace = "other";
    changed.extensions[0].fields[0].value = FieldValue::Handle(30);
    normalize_extensions(&mut changed);
    assert!(
        changed
            .extensions
            .iter()
            .any(|extension| extension.fields[0].value == FieldValue::Handle(30))
    );
}

/// 拡張の付く先と、名前空間・名前・版・型の契約の版。
fn extension_key(
    extension: &zumen_drawing::Extension,
) -> (String, &'static str, String, Option<String>, u32) {
    (
        format!("{:?}", extension.target),
        extension.namespace,
        extension.name.clone(),
        extension.version.clone(),
        extension.schema,
    )
}
