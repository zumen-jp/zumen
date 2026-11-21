//! 指定した版の内容規則で変更した項目だけを照合する。
#![allow(clippy::unwrap_used)] // 自作の固定入力。
use zumen_base::{Budget, CancelToken, Cx, Diagnostics};
use zumen_delivery::{Edit, NewElement, PathStep, write_xml, write_xml_checked};

#[test]
fn text_and_repeated_children_have_separate_results_on_the_written_bytes() {
    let source = b"<!DOCTYPE root SYSTEM 'one.dtd'><root><group/><note>old</note></root>";
    let group = [PathStep {
        name: "group".into(),
        index: 0,
    }];
    let note = [PathStep {
        name: "note".into(),
        index: 0,
    }];
    let child = NewElement {
        name: "group".into(),
        value: None,
        children: vec![],
    };
    let edits = [
        Edit::SetText {
            path: &group,
            value: "bad",
        },
        Edit::SetText {
            path: &note,
            value: "new",
        },
        Edit::Append {
            path: &[],
            child: &child,
        },
    ];
    let rule = |element, fields| {
        let json = format!(
            r#"{{"check":"delivery-edit-content","doctype":"one.dtd","element":"{element}",{fields}}}"#
        );
        (
            String::from(element),
            zumen_json::parse(json.as_bytes()).unwrap(),
        )
    };
    let rules = [
        rule("group", r#""text":"whitespace""#),
        rule("note", r#""text":"any""#),
        rule("root", r#""children":{"group":1}"#),
    ];
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
    let plain = write_xml(source, &edits, &mut cx).unwrap();
    let (bytes, checks) = write_xml_checked(source, &edits, Some(&rules), &mut cx).unwrap();
    assert_eq!(bytes, plain);
    let values = checks
        .iter()
        .map(|item| {
            let span = &bytes[item.start as usize..item.end as usize];
            (
                item.edit,
                item.rule.as_deref(),
                item.status,
                item.reason,
                span,
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        values,
        [
            (
                0,
                Some("group"),
                "violated",
                "text",
                b"<group>bad</group>".as_slice()
            ),
            (
                1,
                Some("note"),
                "matched",
                "text",
                b"<note>new</note>".as_slice()
            ),
            (
                2,
                Some("root"),
                "violated",
                "child_count",
                &bytes[bytes.iter().position(|b| *b == b'>').unwrap() + 1..]
            ),
        ]
    );
    let (_, missing) = write_xml_checked(source, &edits, Some(&[]), &mut cx).unwrap();
    assert!(
        missing
            .iter()
            .all(|item| item.status == "unchecked" && item.reason == "no_rule")
    );
}
