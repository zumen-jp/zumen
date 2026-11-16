use zumen_base::{Budget, CancelToken, Cx, Diagnostic, Diagnostics, Kind, Limit, Limits};
use zumen_drawing::{Affine, Changes, Edit, Hash, ModelPoint, Placed, Seen, Shape, id};
use zumen_live::RowId;
use zumen_sdk::Error;

use super::super::AcadLive;
use super::super::tests::{fresh_cx, synthetic_rich_dxf};

fn lines(handles: &[Option<u64>]) -> Vec<u8> {
    let mut text = "0\nSECTION\n2\nHEADER\n9\n$INSUNITS\n70\n4\n0\nENDSEC\n\
        0\nSECTION\n2\nENTITIES\n"
        .to_owned();
    for (index, handle) in handles.iter().enumerate() {
        text.push_str("0\nLINE\n");
        if let Some(handle) = handle {
            text.push_str(&format!("5\n{handle:X}\n"));
        }
        text.push_str(&format!(
            "8\n0\n10\n{index}\n20\n0\n11\n{}\n21\n1\n",
            index + 1
        ));
    }
    text.push_str("0\nENDSEC\n0\nEOF\n");
    text.into_bytes()
}

fn targets(live: &AcadLive) -> Vec<Seen> {
    let model = fresh_cx(|cx| live.drawing(cx)).0.unwrap();
    let mut namer = id::Namer::new("space/0");
    let mut expected: Vec<_> = model.spaces[0]
        .elements
        .iter()
        .enumerate()
        .map(|(index, element)| Seen {
            id: namer.next(index, element),
            hash: Hash::element(element),
        })
        .collect();
    let mut actual = Vec::new();
    for index in 0..live.row_count() {
        let row = RowId::from_index(index).unwrap();
        if let Some(seen) = fresh_cx(|cx| live.seen_of_row(row, cx)).0.unwrap() {
            let element = fresh_cx(|cx| live.element_with_source(row, cx))
                .0
                .unwrap()
                .unwrap();
            assert_eq!(seen.hash, Hash::element(&element));
            actual.push(seen);
        } else {
            assert!(!live.table().is_alive(row));
        }
    }
    expected.sort_by(|left, right| left.id.cmp(&right.id));
    actual.sort_by(|left, right| left.id.cmp(&right.id));
    assert_eq!(actual, expected);
    assert_eq!(
        fresh_cx(|cx| live.seen_of_row(RowId(u32::MAX), cx))
            .0
            .unwrap(),
        None
    );
    actual
}

#[test]
fn row_targets_match_zero_large_duplicate_and_keyless_ids() {
    let input = lines(&[Some(0), Some(u64::MAX), Some(0x10), None, Some(0x10), None]);
    for threads in [1, 4] {
        let live = fresh_cx(|cx| AcadLive::open_with_threads(&input, threads, cx))
            .0
            .unwrap();
        let actual = targets(&live);
        assert!(actual.iter().any(|target| target.id == "space/0/h0"));
        assert!(
            actual
                .iter()
                .any(|target| target.id == "space/0/hFFFFFFFFFFFFFFFF")
        );
        assert!(actual.iter().any(|target| target.id == "space/0/h10/1"));
    }
}

#[test]
fn row_targets_follow_keyless_id_shifts_and_history() {
    let input = lines(&[Some(0x10), None, Some(0x11), None]);
    for threads in [1, 4] {
        let mut live = fresh_cx(|cx| AcadLive::open_with_threads(&input, threads, cx))
            .0
            .unwrap();
        let before = targets(&live);
        let bytes = fresh_cx(|cx| live.save(cx)).0.unwrap();
        let target = before
            .iter()
            .find(|target| target.id == "space/0/h10")
            .unwrap()
            .clone();
        fresh_cx(|cx| {
            live.apply(
                &Changes {
                    document: None,
                    edits: vec![Edit::Remove { target }],
                },
                cx,
            )
        })
        .0
        .unwrap();
        let removed = targets(&live);
        let keyless = |values: &[Seen]| {
            values
                .iter()
                .filter(|value| value.id.starts_with("space/0/r"))
                .map(|value| value.id.clone())
                .collect::<Vec<_>>()
        };
        assert_ne!(keyless(&before), keyless(&removed));
        assert!(fresh_cx(|cx| live.undo(cx)).0.unwrap());
        assert_eq!(targets(&live), before);
        assert_eq!(fresh_cx(|cx| live.save(cx)).0.unwrap(), bytes);
        assert!(fresh_cx(|cx| live.redo(cx)).0.unwrap());
        assert_eq!(targets(&live), removed);
        // 戻した版から編集したときも、現在の ID とハッシュを次の編集にそのまま使える。
        assert!(fresh_cx(|cx| live.undo(cx)).0.unwrap());
        let target = fresh_cx(|cx| live.seen_of_row(RowId(1), cx))
            .0
            .unwrap()
            .unwrap();
        fresh_cx(|cx| {
            live.apply(
                &Changes {
                    document: None,
                    edits: vec![Edit::Transform {
                        target,
                        transform: Affine::translation(2.0, 3.0),
                    }],
                },
                cx,
            )
        })
        .0
        .unwrap();
        assert_ne!(targets(&live), before);
        assert!(!fresh_cx(|cx| live.redo(cx)).0.unwrap());
    }
}

type Probe = (Result<Option<Seen>, Error>, Vec<Diagnostic>, [u64; 7]);

fn probe(
    live: &AcadLive,
    row: RowId,
    limits: Limits,
    cancel: &CancelToken,
    prior: Option<(Kind, u64)>,
) -> Probe {
    let mut budget = Budget::new(limits);
    if let Some((kind, amount)) = prior {
        budget.spend(kind, amount).unwrap();
    }
    let mut diagnostics = Diagnostics::new();
    let result = live.seen_of_row(row, &mut Cx::new(&mut budget, &mut diagnostics, cancel));
    (
        result,
        diagnostics.into_vec(),
        Kind::ALL.map(|kind| budget.used(kind)),
    )
}

#[test]
#[expect(clippy::print_stdout, reason = "ネイティブと WASI の比較記録")]
fn row_targets_share_budget_and_preserve_state_on_stop() {
    let mut records = String::new();
    for input in [
        synthetic_rich_dxf(),
        lines(&[None, Some(0x10), Some(0x10), None]),
    ] {
        for threads in [1, 4] {
            let live = fresh_cx(|cx| AcadLive::open_with_threads(&input, threads, cx))
                .0
                .unwrap();
            let before = fresh_cx(|cx| live.save(cx)).0.unwrap();
            let display = format!("{:?}", live.display());
            let cancel = CancelToken::new();
            for index in 0..live.row_count() {
                let row = RowId::from_index(index).unwrap();
                let baseline = probe(&live, row, Limits::default(), &cancel, None);
                assert!(baseline.0.as_ref().unwrap().is_some());
                assert!(baseline.1.is_empty());
                assert_eq!(baseline.2[0], 0);
                for kind in [Kind::Work, Kind::DecodedBytes] {
                    let needed =
                        baseline.2[Kind::ALL.iter().position(|&value| value == kind).unwrap()];
                    assert!(needed > 0);
                    for available in [0, needed / 2, needed - 1, needed] {
                        let limits = Limits::default().with(kind, Limit::Finite(available));
                        let actual = probe(&live, row, limits, &cancel, None);
                        assert_eq!(actual, probe(&live, row, limits, &cancel, None));
                        if available < needed {
                            assert!(matches!(actual.0, Err(Error::Stopped(_))));
                            assert_eq!(actual.1.len(), 1);
                        } else {
                            assert_eq!(actual, baseline);
                        }
                        records.push_str(&format!(
                            "{threads} {index} {kind:?} {available} {actual:?}\n"
                        ));
                    }
                    let limits = Limits::default().with(kind, Limit::Finite(needed));
                    assert!(matches!(
                        probe(&live, row, limits, &cancel, Some((kind, 1))).0,
                        Err(Error::Stopped(_))
                    ));
                }
            }
            cancel.cancel();
            for row in [RowId(0), RowId(u32::MAX)] {
                let stopped = probe(&live, row, Limits::default(), &cancel, None);
                assert!(matches!(stopped.0, Err(Error::Stopped(_))));
                assert_eq!(stopped.1.len(), 1);
                records.push_str(&format!("{stopped:?}\n"));
            }
            assert_eq!(fresh_cx(|cx| live.save(cx)).0.unwrap(), before);
            assert_eq!(format!("{:?}", live.display()), display);
            assert_eq!(live.revision().0, 0);
        }
    }
    println!("live-row-targets {:?}", Hash::document(records.as_bytes()));
}

#[test]
fn keyed_row_target_work_does_not_grow_with_document_size() {
    let cancel = CancelToken::new();
    let mut baseline = None;
    for size in [10, 10_000] {
        let input = lines(
            &(0..size)
                .map(|index| Some(0x100 + index))
                .collect::<Vec<_>>(),
        );
        let mut live = fresh_cx(|cx| AcadLive::open(&input, cx)).0.unwrap();
        let result = probe(&live, RowId(0), Limits::default(), &cancel, None);
        if let Some(expected) = &baseline {
            assert_eq!(&result, expected);
        } else {
            baseline = Some(result.clone());
        }
        let target = result.0.unwrap().unwrap();
        fresh_cx(|cx| {
            live.apply(
                &Changes {
                    document: None,
                    edits: vec![Edit::Transform {
                        target,
                        transform: Affine::translation(2.0, 3.0),
                    }],
                },
                cx,
            )
        })
        .0
        .unwrap();
        let result = probe(&live, RowId(0), Limits::default(), &cancel, None);
        assert_eq!(result.2, baseline.as_ref().unwrap().2);
        assert_eq!(result.2[0], 0);
        assert_ne!(result.0, baseline.as_ref().unwrap().0);
    }
}

#[test]
#[expect(clippy::print_stdout, reason = "ネイティブと WASI の比較記録")]
fn row_targets_after_dwg_add_use_no_input_budget() {
    let inputs: [&[u8]; 3] = [
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../samples/dwg/zumen-layout-r2000.dwg"
        )),
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../samples/dwg/zumen-layout-r2004.dwg"
        )),
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../samples/dwg/zumen-layout-r2018.dwg"
        )),
    ];
    let mut records = String::new();
    for input in inputs {
        for threads in [1, 4] {
            let mut live = fresh_cx(|cx| AcadLive::open_with_threads(input, threads, cx))
                .0
                .unwrap();
            let model = fresh_cx(|cx| live.drawing(cx)).0.unwrap();
            let mut element = model.spaces[0].elements[0].clone();
            let point = |x, y| ModelPoint {
                space: model.spaces[0].id,
                x,
                y,
            };
            element.shape = Shape::Line {
                start: point(0.0, 0.0),
                end: point(10.0, 20.0),
            };
            let applied = fresh_cx(|cx| {
                live.apply(
                    &Changes {
                        document: None,
                        edits: vec![Edit::Add {
                            place: "space/0".to_owned(),
                            element: Box::new(Placed::Model(element)),
                        }],
                    },
                    cx,
                )
            })
            .0
            .unwrap()
            .pop()
            .unwrap();
            let id = applied.id.unwrap();
            let handle = u64::from_str_radix(id.strip_prefix("space/0/h").unwrap(), 16).unwrap();
            let row = live.table().find_handle(handle).unwrap();
            let cancel = CancelToken::new();
            let limits = Limits::default().with(Kind::InputBytes, Limit::Finite(0));
            let result = probe(&live, row, limits, &cancel, None);
            assert_eq!(
                result.0,
                Ok(Some(Seen {
                    id,
                    hash: applied.hash.unwrap()
                }))
            );
            assert!(result.1.is_empty());
            assert_eq!(result.2[0], 0);
            records.push_str(&format!("{threads} {result:?}\n"));
            targets(&live);
        }
    }
    println!(
        "live-dwg-row-targets {:?}",
        Hash::document(records.as_bytes())
    );
}
