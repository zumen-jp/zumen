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
fn keyless_and_duplicate_row_target_work_does_not_grow_with_document_size() {
    let cancel = CancelToken::new();
    for duplicate in [false, true] {
        let mut baseline = None;
        for size in [10_usize, 10_000] {
            let handles: Vec<Option<u64>> = (0..size)
                .map(|_| if duplicate { Some(0x20) } else { None })
                .collect();
            let input = lines(&handles);
            let live = fresh_cx(|cx| AcadLive::open(&input, cx)).0.unwrap();
            // 末尾付近でない固定行 RowId(5) の逆引きコストが要素数 10 と 10,000 で完全一致する
            let result = probe(&live, RowId(5), Limits::default(), &cancel, None);
            assert!(result.0.as_ref().unwrap().is_some());
            if let Some(expected) = &baseline {
                assert_eq!(&result, expected);
            } else {
                baseline = Some(result);
            }
        }
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

type SelectionProbe = (Result<(), Error>, Vec<RowId>, Vec<Diagnostic>, [u64; 7]);

fn select_probe(
    live: &AcadLive,
    rect: [f64; 4],
    limits: Limits,
    cancel: &CancelToken,
    prior: Option<Kind>,
) -> SelectionProbe {
    let mut budget = Budget::new(limits);
    if let Some(kind) = prior {
        budget.spend(kind, 1).unwrap();
    }
    let mut diagnostics = Diagnostics::new();
    let mut rows = vec![RowId(999), RowId(998)];
    let result = live.select_rect_with_cx(
        rect,
        &mut rows,
        &mut Cx::new(&mut budget, &mut diagnostics, cancel),
    );
    (
        result,
        rows,
        diagnostics.into_vec(),
        Kind::ALL.map(|kind| budget.used(kind)),
    )
}

fn check_selections(live: &AcadLive, records: &mut String) {
    let all = [
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
        f64::INFINITY,
        f64::INFINITY,
    ];
    let cancel = CancelToken::new();
    for rect in [
        all,
        [0.25, 0.25, 0.75, 0.75],
        [100.0, 200.0, 101.0, 201.0],
        [f64::NAN, 0.0, 1.0, 1.0],
        [2.0, 1.0, 1.0, 2.0],
    ] {
        let mut expected = vec![RowId(999), RowId(998)];
        live.select_rect(rect, &mut expected);
        let actual = select_probe(live, rect, Limits::default(), &cancel, None);
        assert_eq!(actual.0, Ok(()));
        assert_eq!(actual.1, expected);
        assert!(actual.2.is_empty());
        assert_eq!(actual.3[0], 0);
        records.push_str(&format!("{actual:?}\n"));
    }
    let normal = select_probe(live, all, Limits::default(), &cancel, None);
    for kind in [Kind::Elements, Kind::DecodedBytes, Kind::Work] {
        let needed = normal.3[Kind::ALL.iter().position(|&value| value == kind).unwrap()];
        assert!(needed > 0);
        for limit in [0, needed / 2, needed - 1, needed] {
            let limits = Limits::default().with(kind, Limit::Finite(limit));
            let actual = select_probe(live, all, limits, &cancel, None);
            assert_eq!(actual, select_probe(live, all, limits, &cancel, None));
            if limit == needed {
                assert_eq!(actual, normal);
            } else {
                assert!(matches!(actual.0, Err(Error::Stopped(_))));
                assert_eq!(actual.1, vec![RowId(999), RowId(998)]);
                assert_eq!(actual.2.len(), 1);
            }
            records.push_str(&format!("{kind:?} {limit} {actual:?}\n"));
        }
        let limits = Limits::default().with(kind, Limit::Finite(needed));
        let used = select_probe(live, all, limits, &cancel, Some(kind));
        assert!(matches!(used.0, Err(Error::Stopped(_))));
        assert_eq!(used.1, vec![RowId(999), RowId(998)]);
    }
    cancel.cancel();
    let stopped = select_probe(live, all, Limits::default(), &cancel, None);
    assert_eq!(stopped.0, Err(Error::Stopped(zumen_base::Stop::Cancelled)));
    assert_eq!(stopped.1, vec![RowId(999), RowId(998)]);
    assert_eq!(stopped.2.len(), 1);
    records.push_str(&format!("{stopped:?}\n"));
}

#[test]
#[expect(clippy::print_stdout, reason = "ネイティブと WASI の比較記録")]
fn rectangle_selection_is_budgeted_and_follows_edits_and_history() {
    let dxf = lines(&[Some(0x10), None, Some(0x11), None]);
    let dwg: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../samples/dwg/zumen-points-a-r2018.dwg"
    ));
    let mut records = String::new();
    for input in [dxf.as_slice(), dwg] {
        let mut baseline = None;
        for threads in [1, 4] {
            let mut live = fresh_cx(|cx| AcadLive::open_with_threads(input, threads, cx))
                .0
                .unwrap();
            let initial = fresh_cx(|cx| live.save(cx)).0.unwrap();
            let mut trace = String::new();
            for step in 0..5 {
                let before = fresh_cx(|cx| live.save(cx)).0.unwrap();
                let display = format!("{:?}", live.display());
                let revision = live.revision();
                check_selections(&live, &mut trace);
                assert_eq!(fresh_cx(|cx| live.save(cx)).0.unwrap(), before);
                assert_eq!(format!("{:?}", live.display()), display);
                assert_eq!(live.revision(), revision);
                match step {
                    0 | 1 => {
                        let target = fresh_cx(|cx| live.seen_of_row(RowId(0), cx))
                            .0
                            .unwrap()
                            .unwrap();
                        let edit = if step == 0 {
                            Edit::Transform {
                                target,
                                transform: Affine::translation(100.0, 200.0),
                            }
                        } else {
                            Edit::Remove { target }
                        };
                        fresh_cx(|cx| {
                            live.apply(
                                &Changes {
                                    document: None,
                                    edits: vec![edit],
                                },
                                cx,
                            )
                        })
                        .0
                        .unwrap();
                    }
                    2 | 3 => {
                        assert!(fresh_cx(|cx| live.undo(cx)).0.unwrap());
                    }
                    _ => {}
                }
            }
            assert_eq!(fresh_cx(|cx| live.save(cx)).0.unwrap(), initial);
            assert!(fresh_cx(|cx| live.redo(cx)).0.unwrap());
            assert!(fresh_cx(|cx| live.redo(cx)).0.unwrap());
            check_selections(&live, &mut trace);
            if let Some(expected) = &baseline {
                assert_eq!(&trace, expected);
            } else {
                baseline = Some(trace.clone());
            }
            records.push_str(&trace);
        }
    }
    println!(
        "live-rectangle-selection {:?}",
        Hash::document(records.as_bytes())
    );
}
