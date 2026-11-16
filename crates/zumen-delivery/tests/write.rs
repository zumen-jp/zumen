//! 元の文書に対する経路の解決と、変更後の再読。
#![allow(clippy::unwrap_used)] // 自作入力と期待値。

use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits};
use zumen_delivery::{Edit, NewElement, PathStep, write_xml};

#[test]
fn qualified_names_and_sibling_positions_select_only_the_original() {
    let source = b"<r xmlns:p='urn:p'><item>A</item><p:item>P</p:item><item>B</item><group/></r>";
    let path = [PathStep {
        name: "item".into(),
        index: 1,
    }];
    let prefixed = [PathStep {
        name: "p:item".into(),
        index: 0,
    }];
    let group = [PathStep {
        name: "group".into(),
        index: 0,
    }];
    let child = NewElement {
        name: "p:item".into(),
        value: Some("C".into()),
        children: vec![],
    };
    let edits = [
        Edit::SetText {
            path: &path,
            value: "B2",
        },
        Edit::SetText {
            path: &prefixed,
            value: "P2",
        },
        Edit::Append {
            path: &group,
            child: &child,
        },
    ];
    let mut budget = Budget::default();
    let mut diagnostics = Diagnostics::new();
    let cancel = CancelToken::new();
    let output = write_xml(
        source,
        &edits,
        &mut Cx::new(&mut budget, &mut diagnostics, &cancel),
    )
    .unwrap();
    assert_eq!(output,b"<r xmlns:p='urn:p'><item>A</item><p:item>P2</p:item><item>B2</item><group><p:item>C</p:item></group></r>");
    assert!(diagnostics.iter().next().is_none());
    let missing = [PathStep {
        name: "item".into(),
        index: 2,
    }];
    let result = write_xml(
        source,
        &[
            Edit::Append {
                path: &[],
                child: &child,
            },
            Edit::SetText {
                path: &missing,
                value: "x",
            },
        ],
        &mut Cx::new(&mut budget, &mut diagnostics, &cancel),
    );
    assert!(result.is_err());
    let diagnostic = diagnostics.iter().next().unwrap();
    assert_eq!(diagnostic.code().name(), "delivery.edit-path.missing");
}

#[test]
fn output_rereading_shares_the_budget_and_restores_depth() {
    let source = b"<?xml version='1.0'?><!DOCTYPE r SYSTEM 'synthetic.dtd'><r><x/></r>";
    let path = [PathStep {
        name: "x".into(),
        index: 0,
    }];
    let edits = [Edit::SetText {
        path: &path,
        value: "value",
    }];
    let run = |limits| {
        let mut budget = Budget::new(limits);
        let mut diagnostics = Diagnostics::new();
        let cancel = CancelToken::new();
        let result = write_xml(
            source,
            &edits,
            &mut Cx::new(&mut budget, &mut diagnostics, &cancel),
        );
        (result, budget, diagnostics)
    };
    let (output, used, diagnostics) = run(Limits::default());
    assert!(output.is_ok());
    assert!(diagnostics.iter().next().is_none());
    for kind in [
        Kind::Work,
        Kind::DecodedBytes,
        Kind::InputBytes,
        Kind::Records,
    ] {
        let (result, budget, diagnostics) =
            run(Limits::default().with(kind, Limit::Finite(used.used(kind) - 1)));
        assert!(matches!(result, Err(zumen_sdk::Error::Stopped(_))));
        assert_eq!(budget.used(Kind::Depth), 0);
        assert_eq!(diagnostics.iter().count(), 1);
    }
}
