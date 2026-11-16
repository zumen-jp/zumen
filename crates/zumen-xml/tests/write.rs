//! XML の一部だけを書き換え、原本の書式と値を保つ。
#![allow(clippy::unwrap_used, clippy::panic)] // 自作の最小入力と期待値。

use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Error, Kind, Limit, Limits};
use zumen_xml::{Document, Edit, Encoding, NewElement, Node};

const SOURCE: &str = "<?xml version='1.0' encoding='UTF-8'?>\r\n<!DOCTYPE 管理 SYSTEM 'synthetic.dtd'>\r\n<管理 xmlns:p='urn:test' version='1'><!--keep-->\r\n\t<項目 code='x'> old <![CDATA[<part>]]><?note x?> tail </項目>\r\n\t<空 属性 = \"保つ\" />\r\n\t<反復>first</反復>\r\n\t<反復>second</反復>\r\n</管理>\r\n";

#[test]
fn mutated_sources_stop_deterministically_before_or_after_writing() {
    for encoding in [
        Encoding::Utf8,
        Encoding::ShiftJis,
        Encoding::Utf16Le,
        Encoding::Utf16Be,
    ] {
        let seed = input(encoding, SOURCE);
        for position in 0..seed.len() {
            for byte in [0, b'<', b'/', 0xff] {
                let mut source = seed.clone();
                if let Some(value) = source.get_mut(position) {
                    *value = byte;
                }
                zumen_sdk::testkit::survives_with(&source, |source, cx| {
                    let doc = zumen_xml::read(source, cx)?;
                    let child = leaf("added", "<&>\n\r");
                    let written = zumen_xml::write(
                        &doc,
                        &[Edit::Append {
                            element: doc.root() as u64,
                            child: &child,
                        }],
                        cx,
                    )?;
                    zumen_xml::read(&written, cx)
                })
                .unwrap();
            }
        }
    }
}

fn input(encoding: Encoding, source: &str) -> Vec<u8> {
    match encoding {
        Encoding::Utf8 => source.as_bytes().to_vec(),
        Encoding::ShiftJis => {
            let text = source.replace("UTF-8", "Shift_JIS");
            let (data, _, changed) = encoding_rs::SHIFT_JIS.encode(&text);
            assert!(!changed);
            data.into_owned()
        }
        Encoding::Utf16Le => [
            vec![0xff, 0xfe],
            source
                .replace("UTF-8", "UTF-16")
                .encode_utf16()
                .flat_map(u16::to_le_bytes)
                .collect(),
        ]
        .concat(),
        Encoding::Utf16Be => [
            vec![0xfe, 0xff],
            source
                .replace("UTF-8", "UTF-16")
                .encode_utf16()
                .flat_map(u16::to_be_bytes)
                .collect(),
        ]
        .concat(),
        _ => panic!("未対応の文字コード"),
    }
}

fn read(bytes: &[u8]) -> Document {
    zumen_xml::read(
        bytes,
        &mut Cx::new(
            &mut Budget::default(),
            &mut Diagnostics::new(),
            &CancelToken::new(),
        ),
    )
    .unwrap()
}

fn write(
    doc: &Document,
    edits: &[Edit<'_>],
    limits: Limits,
    cancel: &CancelToken,
) -> (Result<Vec<u8>, Error>, Diagnostics, Budget) {
    let mut budget = Budget::new(limits);
    let mut diag = Diagnostics::new();
    let result = zumen_xml::write(doc, edits, &mut Cx::new(&mut budget, &mut diag, cancel));
    (result, diag, budget)
}

fn index(doc: &Document, name: &str) -> u64 {
    doc.elements()
        .iter()
        .position(|e| doc.cdata(e.name().raw()) == name)
        .unwrap() as u64
}

fn value(doc: &Document, name: &str) -> String {
    let element = doc.element(index(doc, name) as usize).unwrap();
    element
        .children()
        .iter()
        .filter_map(|node| match node {
            Node::Text(range) => Some(doc.text(range.clone())),
            Node::CData(range) => Some(doc.cdata(range.clone())),
            Node::Element(_) => None,
        })
        .collect()
}

fn leaf(name: &str, value: &str) -> NewElement {
    NewElement {
        name: name.into(),
        value: Some(value.into()),
        children: Vec::new(),
    }
}

#[test]
fn edits_only_target_bytes_in_all_encodings() {
    for encoding in [
        Encoding::Utf8,
        Encoding::ShiftJis,
        Encoding::Utf16Le,
        Encoding::Utf16Be,
    ] {
        let source = input(encoding, SOURCE);
        let doc = read(&source);
        let child = NewElement {
            name: "p:追加".into(),
            value: None,
            children: vec![leaf("子", "入れ子")],
        };
        let second = leaf("追加2", "最後");
        let edits = [
            Edit::SetText {
                element: index(&doc, "項目"),
                value: "新<&>\n\r終",
            },
            Edit::SetText {
                element: index(&doc, "空"),
                value: "値",
            },
            Edit::Append {
                element: doc.root() as u64,
                child: &child,
            },
            Edit::Append {
                element: doc.root() as u64,
                child: &second,
            },
        ];
        let (result, diag, _) = write(&doc, &edits, Limits::default(), &CancelToken::new());
        assert_eq!(diag.iter().count(), 0);
        let bytes = result.unwrap();
        let expected = SOURCE
            .replace(
                " old <![CDATA[<part>]]><?note x?> tail ",
                "新&lt;&amp;&gt;\r\n&#13;終<?note x?>",
            )
            .replace("<空 属性 = \"保つ\" />", "<空 属性 = \"保つ\" >値</空>")
            .replace(
                "</管理>",
                "\t<p:追加><子>入れ子</子></p:追加>\r\n\t<追加2>最後</追加2>\r\n</管理>",
            );
        assert_eq!(bytes, input(encoding, &expected));
        let reread = read(&bytes);
        assert_eq!(reread.encoding(), encoding);
        assert_eq!(value(&reread, "項目"), "新<&>\n\r終");
        assert_eq!(value(&reread, "空"), "値");
        assert_eq!(value(&reread, "子"), "入れ子");
        let prefixed = reread.element(index(&reread, "p:追加") as usize).unwrap();
        assert_eq!(
            reread.namespace(prefixed.name()).as_deref(),
            Some("urn:test")
        );
        assert_eq!(doc.src(), source);
    }
}

#[test]
fn preserves_bom_inline_layout_and_comments_without_text() {
    let source = [
        vec![0xef, 0xbb, 0xbf],
        b"<r><a><!--same--><?x y?></a><b /></r>".to_vec(),
    ]
    .concat();
    let doc = read(&source);
    let child = leaf("c", "x");
    let edits = [
        Edit::SetText {
            element: index(&doc, "a"),
            value: "ok",
        },
        Edit::Append {
            element: index(&doc, "b"),
            child: &child,
        },
    ];
    let result = write(&doc, &edits, Limits::default(), &CancelToken::new())
        .0
        .unwrap();
    assert_eq!(
        result,
        [
            vec![0xef, 0xbb, 0xbf],
            b"<r><a><!--same--><?x y?>ok</a><b ><c>x</c></b></r>".to_vec()
        ]
        .concat()
    );
    assert_eq!(
        write(&doc, &[], Limits::default(), &CancelToken::new())
            .0
            .unwrap(),
        source
    );
}

#[test]
fn rejects_conflicts_children_bad_names_namespace_and_characters() {
    let doc = read(b"<r><a>old</a></r>");
    for (edits, code) in [
        (
            vec![
                Edit::SetText {
                    element: 1,
                    value: "x",
                },
                Edit::SetText {
                    element: 1,
                    value: "y",
                },
            ],
            "xml.edit-range.invalid",
        ),
        (
            vec![Edit::SetText {
                element: 0,
                value: "x",
            }],
            "xml.edit-children.unsupported",
        ),
        (
            vec![Edit::SetText {
                element: u64::MAX,
                value: "x",
            }],
            "xml.edit-target.missing",
        ),
        (
            vec![Edit::SetText {
                element: 1,
                value: "\0",
            }],
            "xml.edit-character.invalid",
        ),
        (
            vec![Edit::SetText {
                element: 1,
                value: "\u{ffff}",
            }],
            "xml.edit-character.invalid",
        ),
    ] {
        let (result, diag, _) = write(&doc, &edits, Limits::default(), &CancelToken::new());
        assert!(result.is_err());
        assert_eq!(diag.iter().next().unwrap().code().name(), code);
    }
    for (name, code) in [
        ("1bad", "xml.edit-name.invalid"),
        ("a/><b", "xml.edit-name.invalid"),
        ("q:a", "xml.edit-namespace.missing"),
        ("xmlns:a", "xml.edit-namespace.missing"),
    ] {
        let child = leaf(name, "ok");
        let (result, diag, _) = write(
            &doc,
            &[Edit::Append {
                element: 0,
                child: &child,
            }],
            Limits::default(),
            &CancelToken::new(),
        );
        assert!(result.is_err());
        assert_eq!(diag.iter().next().unwrap().code().name(), code);
    }
    let child = leaf("a", "x");
    assert!(
        write(
            &doc,
            &[
                Edit::Append {
                    element: 1,
                    child: &child
                },
                Edit::SetText {
                    element: 1,
                    value: "x"
                }
            ],
            Limits::default(),
            &CancelToken::new()
        )
        .0
        .is_err()
    );
    assert_eq!(doc.src(), b"<r><a>old</a></r>");
}

#[test]
fn rejects_unrepresentable_or_changed_shift_jis_text() {
    let doc = read(&input(
        Encoding::ShiftJis,
        "<?xml version='1.0' encoding='UTF-8'?><r><a>old</a></r>",
    ));
    for value in ["😀", "¥", "‾"] {
        let (result, diag, _) = write(
            &doc,
            &[Edit::SetText { element: 1, value }],
            Limits::default(),
            &CancelToken::new(),
        );
        assert!(result.is_err());
        assert_eq!(
            diag.iter().next().unwrap().code().name(),
            "xml.edit-encoding.failed"
        );
    }
}

#[test]
fn shared_budgets_stop_before_output_and_restore_depth() {
    let doc = read(b"<r><p/></r>");
    let child = NewElement {
        name: "c".into(),
        value: None,
        children: vec![leaf("d", "value")],
    };
    let edits = [Edit::Append {
        element: 1,
        child: &child,
    }];
    let (full, _, used) = write(&doc, &edits, Limits::default(), &CancelToken::new());
    let full = full.unwrap();
    for kind in [Kind::Work, Kind::DecodedBytes, Kind::Records] {
        let limit = used.used(kind);
        assert!(limit > 0);
        assert_eq!(
            write(
                &doc,
                &edits,
                Limits::default().with(kind, Limit::Finite(limit)),
                &CancelToken::new()
            )
            .0
            .unwrap(),
            full
        );
        for amount in [0, limit / 2, limit - 1] {
            let (result, _, budget) = write(
                &doc,
                &edits,
                Limits::default().with(kind, Limit::Finite(amount)),
                &CancelToken::new(),
            );
            assert!(matches!(result, Err(Error::Stopped(_))));
            assert_eq!(budget.used(Kind::Depth), 0);
        }
    }
    assert!(
        write(
            &doc,
            &edits,
            Limits::default().with(Kind::Depth, Limit::Finite(3)),
            &CancelToken::new()
        )
        .0
        .is_err()
    );
    assert_eq!(
        write(
            &doc,
            &edits,
            Limits::default().with(Kind::Depth, Limit::Finite(4)),
            &CancelToken::new()
        )
        .0
        .unwrap(),
        full
    );
    let cancel = CancelToken::new();
    cancel.cancel();
    assert!(matches!(
        write(&doc, &edits, Limits::default(), &cancel).0,
        Err(Error::Stopped(_))
    ));
    assert_eq!(
        write(&doc, &edits, Limits::default(), &CancelToken::new())
            .0
            .unwrap(),
        full
    );
}
