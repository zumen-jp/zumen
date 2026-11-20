use zumen_base::{Budget, CancelToken, Diagnostics, Limit, Limits, Stop};

use super::*;
use crate::Dxf;
use crate::drawing::live::AcadLive;
use crate::drawing::live::equivalence::shared_budget_mtext_dxf;
use crate::drawing::live::tests::{fresh_cx, synthetic_rich_dxf};
use zumen_sdk::Codec;

#[expect(clippy::print_stdout, reason = "ネイティブと WASI の比較記録")]
fn shared_lift_budget(
    label: &str,
    expected: &Element<ModelPoint>,
    mut operation: impl FnMut(&mut Cx<'_>) -> Result<Element<ModelPoint>, Error>,
) {
    let mut measured = Budget::default();
    let mut diagnostics = Diagnostics::new();
    let cancel = CancelToken::new();
    let actual = operation(&mut Cx::new(&mut measured, &mut diagnostics, &cancel)).unwrap();
    assert_eq!(&actual, expected);
    assert!(diagnostics.is_empty());
    for kind in [Kind::Work, Kind::DecodedBytes] {
        let needed = measured.used(kind);
        assert!(needed > 0);
        for available in [0, needed / 2, needed - 1, needed] {
            let mut budget =
                Budget::new(Limits::default().with(kind, Limit::Finite(available + 11)));
            budget.spend(kind, 11).unwrap();
            let mut diagnostics = Diagnostics::new();
            let result = operation(&mut Cx::new(&mut budget, &mut diagnostics, &cancel));
            if available < needed {
                assert!(matches!(result, Err(Error::Stopped(_))));
                assert!(
                    diagnostics
                        .iter()
                        .any(|item| item.code() == kind.exceeded_code())
                );
            } else {
                assert_eq!(result.unwrap(), actual);
                assert_eq!(budget.used(kind), needed + 11);
                assert!(diagnostics.is_empty());
            }
        }
    }
    cancel.cancel();
    let mut budget = Budget::default();
    let mut diagnostics = Diagnostics::new();
    assert_eq!(
        operation(&mut Cx::new(&mut budget, &mut diagnostics, &cancel)),
        Err(Error::Stopped(Stop::Cancelled))
    );
    assert_eq!(budget.used(Kind::Work), 0);
    println!(
        "live-lift-budget {label} hash={:?} used={:?}",
        Hash::element(&actual),
        Kind::ALL.map(|kind| measured.used(kind))
    );
}

#[test]
fn model_and_block_lifts_share_budget_and_cancellation() {
    for threads in [1, 4] {
        let live =
            fresh_cx(|cx| AcadLive::open_with_threads(&shared_budget_mtext_dxf(), threads, cx))
                .0
                .unwrap();
        let row_id = live.table().find_handle(0x120).unwrap();
        let row = *live.table().row(row_id).unwrap();
        let expected = live.element_of_row(row_id).unwrap();
        let LiveBacking::AsciiDxf(backing) = &live.state.backing else {
            panic!("ASCII DXF required");
        };
        let (kind, values) = backing.decode_current(row.record, b"MTEXT").unwrap();
        shared_lift_budget("model", &expected, |cx| {
            stage_lift_model_entity(
                &live.state,
                row.record,
                row_origin(&row),
                row.raw_handle(),
                row.flags & ROW_UNIQUE_HANDLE != 0,
                false,
                &kind,
                values.clone(),
                &[],
                cx,
            )
            .map(|result| result.unwrap().1)
        });

        let live = fresh_cx(|cx| AcadLive::open_with_threads(&synthetic_rich_dxf(), threads, cx))
            .0
            .unwrap();
        let (space, expected) = live
            .state
            .symbols
            .iter()
            .find_map(|symbol| {
                symbol
                    .space
                    .elements
                    .iter()
                    .find(|element| element.source.key() == Some(Key::Handle(0x55)))
                    .map(|element| (symbol.space.id, element))
            })
            .unwrap();
        let record = expected.source.record().unwrap() as u32;
        let LiveBacking::AsciiDxf(backing) = &live.state.backing else {
            panic!("ASCII DXF required");
        };
        let (kind, values) = backing.decode_current(record, b"CIRCLE").unwrap();
        shared_lift_budget("block", expected, |cx| {
            stage_lift_block_element(
                &live.state,
                space,
                expected.source.clone(),
                Origin::Dxf {
                    record: record as usize,
                },
                &expected.shape,
                &kind,
                values.clone(),
                cx,
            )
            .map(|result| result.unwrap())
        });
    }
}

#[test]
fn dwg_windowed_open_and_malformed_dxf_fallback_share_budget_and_cancellation() {
    let base_dwg = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../samples/dwg/zumen-layout-r2018.dwg"
    ));
    // 1,024 件の窓を複数またぐ DWG を作って 1〜4 スレッドの境界停止と診断を照合する
    let mut multi_window_live = fresh_cx(|cx| AcadLive::open_dwg(base_dwg, cx)).0.unwrap();
    let base_model = fresh_cx(|cx| multi_window_live.drawing(cx)).0.unwrap();
    let template = base_model.spaces.first().unwrap().elements.first().unwrap();
    let space = base_model.spaces.first().unwrap().id;
    let mut add_edits = Vec::with_capacity(1100);
    for idx in 0..1100_u32 {
        let mut elem = template.clone();
        elem.shape = Shape::Line {
            start: ModelPoint {
                space,
                x: f64::from(idx),
                y: 0.0,
            },
            end: ModelPoint {
                space,
                x: f64::from(idx) + 1.0,
                y: 1.0,
            },
        };
        add_edits.push(Edit::Add {
            place: "space/0".to_owned(),
            element: Box::new(zumen_drawing::Placed::Model(elem)),
        });
    }
    fresh_cx(|cx| {
        multi_window_live.apply(
            &Changes {
                document: None,
                edits: add_edits,
            },
            cx,
        )
    })
    .0
    .unwrap();
    let large_dwg = fresh_cx(|cx| multi_window_live.save(cx)).0.unwrap();

    let probe_dwg = |bytes: &[u8], threads: usize, limits: Limits, cancel: &CancelToken| {
        let mut budget = Budget::new(limits);
        let mut diag = Diagnostics::new();
        let res = AcadLive::open_dwg_with_threads(
            bytes,
            threads,
            &mut Cx::new(&mut budget, &mut diag, cancel),
        )
        .map(|live| fresh_cx(|cx| live.save(cx)).0.unwrap());
        (
            res,
            diag.into_vec(),
            Kind::ALL.map(|kind| budget.used(kind)),
        )
    };
    let cancel = CancelToken::new();
    let baseline = probe_dwg(&large_dwg, 1, Limits::default(), &cancel);
    assert!(baseline.0.is_ok());
    for threads in [1, 2, 3, 4] {
        assert_eq!(
            probe_dwg(&large_dwg, threads, Limits::default(), &cancel),
            baseline
        );
        for kind in [Kind::Work, Kind::DecodedBytes] {
            let idx = Kind::ALL.iter().position(|&k| k == kind).unwrap();
            let needed = *baseline.2.get(idx).unwrap();
            assert!(needed > 0);
            for avail in [0, needed / 3, needed / 2, needed - 1, needed] {
                let lim = Limits::default().with(kind, Limit::Finite(avail));
                let actual = probe_dwg(&large_dwg, threads, lim, &cancel);
                assert_eq!(actual, probe_dwg(&large_dwg, 1, lim, &cancel));
                if avail < needed {
                    assert!(matches!(actual.0, Err(Error::Stopped(_))));
                } else {
                    assert_eq!(actual, baseline);
                }
            }
        }
    }
    let cancelled = CancelToken::new();
    cancelled.cancel();
    for threads in [1, 4] {
        let res = probe_dwg(&large_dwg, threads, Limits::default(), &cancelled);
        assert_eq!(res.0, Err(Error::Stopped(Stop::Cancelled)));
    }

    // 不正タグで `open_ascii_via_dxf_file_after_input_check` に入る DXF で、
    // `InputBytes` が二重計上されず、`Dxf::read` と同じ予算使用量・診断・中断で止まることを確かめる
    let invalid_dxf = b"0\nSECTION\n2\nHEADER\n9\n$INSUNITS\n70\n4\n0\nENDSEC\n\
0\nSECTION\n2\nENTITIES\n0\nLINE\n5\n10\n8\n0\n10\n0\n20\n0\nabc\nxyz\n0\nENDSEC\n0\nEOF\n";
    let exact_in =
        Limits::default().with(Kind::InputBytes, Limit::Finite(invalid_dxf.len() as u64));
    let mut b_ref = Budget::new(exact_in);
    let mut d_ref = Diagnostics::new();
    let ref_err = Dxf::read(
        invalid_dxf,
        &mut Cx::new(&mut b_ref, &mut d_ref, &CancelToken::new()),
    )
    .err();
    assert_eq!(ref_err, Some(Error::Invalid));
    let ref_used = Kind::ALL.map(|k| b_ref.used(k));
    let ref_diag = d_ref.into_vec();

    for threads in [1, 4] {
        let mut b_live = Budget::new(exact_in);
        let mut d_live = Diagnostics::new();
        let live_err = AcadLive::open_dxf_with_threads(
            invalid_dxf,
            threads,
            &mut Cx::new(&mut b_live, &mut d_live, &CancelToken::new()),
        )
        .err();
        assert_eq!(live_err, ref_err);
        assert_eq!(Kind::ALL.map(|k| b_live.used(k)), ref_used);
        assert_eq!(d_live.into_vec(), ref_diag);

        for kind in [Kind::Work, Kind::Records] {
            let mut b_stop = Budget::new(exact_in.with(kind, Limit::Finite(1)));
            let mut d_stop = Diagnostics::new();
            let res = AcadLive::open_dxf_with_threads(
                invalid_dxf,
                threads,
                &mut Cx::new(&mut b_stop, &mut d_stop, &CancelToken::new()),
            );
            assert!(matches!(res, Err(Error::Stopped(_))));
            assert!(
                d_stop
                    .iter()
                    .any(|item| item.code() == kind.exceeded_code())
            );
        }
        let mut b_cancel = Budget::new(exact_in);
        let mut d_cancel = Diagnostics::new();
        assert_eq!(
            AcadLive::open_dxf_with_threads(
                invalid_dxf,
                threads,
                &mut Cx::new(&mut b_cancel, &mut d_cancel, &cancelled),
            )
            .err(),
            Some(Error::Stopped(Stop::Cancelled))
        );
    }
}

#[test]
fn apply_undo_redo_and_display_stops_never_corrupt_state_or_dirty_flags() {
    let dxf = synthetic_rich_dxf();
    let dwg = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../samples/dwg/zumen-layout-r2018.dwg"
    ));
    for bytes in [dxf.as_slice(), dwg.as_slice()] {
        let mut live = fresh_cx(|cx| AcadLive::open_with_threads(bytes, 2, cx))
            .0
            .unwrap();
        assert!(!live.display().has_dirty());
        assert!(live.dirty_symbols().is_empty());

        let snap = |engine: &AcadLive| {
            (
                engine.revision(),
                engine.state_id(),
                engine.display().has_dirty(),
                engine.dirty_symbols().clone(),
                fresh_cx(|cx| engine.save(cx)).0.unwrap(),
            )
        };
        let initial = snap(&live);

        // モデル要素とブロック定義内要素の移動を用意する
        let model = fresh_cx(|cx| live.drawing(cx)).0.unwrap();
        let model_target = fresh_cx(|cx| live.seen_of_row(zumen_live::RowId(0), cx))
            .0
            .unwrap()
            .unwrap();
        let edit_changes = Changes {
            document: None,
            edits: vec![Edit::Transform {
                target: model_target,
                transform: zumen_drawing::Affine::translation(5.0, -3.0),
            }],
        };
        let _ = model;

        // 1. `apply` の必要な予算を測り、あらゆる境界上限・中断で止めても状態と dirty フラグが一切壊れないことを確かめる
        let mut measured = Budget::default();
        let mut probe_live = live.clone();
        probe_live
            .apply(
                &edit_changes,
                &mut Cx::new(&mut measured, &mut Diagnostics::new(), &CancelToken::new()),
            )
            .unwrap();
        for kind in [Kind::Work, Kind::DecodedBytes] {
            let needed = measured.used(kind);
            if needed == 0 {
                continue;
            }
            for avail in [0, 1, needed / 2, needed - 1] {
                let mut lim_b = Budget::new(Limits::default().with(kind, Limit::Finite(avail)));
                let mut lim_d = Diagnostics::new();
                let res = live.apply(
                    &edit_changes,
                    &mut Cx::new(&mut lim_b, &mut lim_d, &CancelToken::new()),
                );
                assert!(matches!(res, Err(Error::Stopped(_))));
                assert_eq!(snap(&live), initial);
            }
        }
        let cancelled = CancelToken::new();
        cancelled.cancel();
        assert!(matches!(
            live.apply(
                &edit_changes,
                &mut Cx::new(&mut Budget::default(), &mut Diagnostics::new(), &cancelled),
            ),
            Err(Error::Stopped(Stop::Cancelled))
        ));
        assert_eq!(snap(&live), initial);

        // 正常確定させ、表示変更フラグを立てた状態で `undo` / `redo` / `display_wire::encode` の停止不変性を確かめる
        fresh_cx(|cx| live.apply(&edit_changes, cx)).0.unwrap();
        assert!(live.display().has_dirty());
        let after_edit = snap(&live);

        let mut bad_wire_b =
            Budget::new(Limits::default().with(Kind::DecodedBytes, Limit::Finite(4)));
        assert!(
            zumen_live::display_wire::encode(
                live.display(),
                false,
                &mut Cx::new(
                    &mut bad_wire_b,
                    &mut Diagnostics::new(),
                    &CancelToken::new()
                ),
            )
            .is_err()
        );
        assert_eq!(snap(&live), after_edit);

        // `undo` の途中停止
        let mut undo_measured = Budget::default();
        let mut undo_probe = live.clone();
        assert!(
            undo_probe
                .undo(&mut Cx::new(
                    &mut undo_measured,
                    &mut Diagnostics::new(),
                    &CancelToken::new(),
                ))
                .unwrap()
        );
        for kind in [Kind::Work, Kind::DecodedBytes] {
            let needed = undo_measured.used(kind);
            if needed == 0 {
                continue;
            }
            for avail in [0, needed / 2, needed - 1] {
                let mut lim_b = Budget::new(Limits::default().with(kind, Limit::Finite(avail)));
                let res = live.undo(&mut Cx::new(
                    &mut lim_b,
                    &mut Diagnostics::new(),
                    &CancelToken::new(),
                ));
                assert!(matches!(res, Err(Error::Stopped(_))));
                assert_eq!(snap(&live), after_edit);
            }
        }

        // `undo` 確定 -> `redo` の途中停止 -> `redo` 確定 -> `undo` で元のバイト列へ戻る
        live.display_mut().clear_dirty();
        assert!(fresh_cx(|cx| live.undo(cx)).0.unwrap());
        let after_undo = snap(&live);
        assert_eq!(after_undo.4, initial.4);

        assert!(matches!(
            live.redo(&mut Cx::new(
                &mut Budget::default(),
                &mut Diagnostics::new(),
                &cancelled,
            )),
            Err(Error::Stopped(Stop::Cancelled))
        ));
        assert_eq!(snap(&live), after_undo);
        assert!(fresh_cx(|cx| live.redo(cx)).0.unwrap());
        assert_eq!(fresh_cx(|cx| live.save(cx)).0.unwrap(), after_edit.4);
    }
}
