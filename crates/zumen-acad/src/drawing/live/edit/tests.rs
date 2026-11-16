use zumen_base::{Budget, CancelToken, Diagnostics, Limit, Limits, Stop};

use super::*;
use crate::drawing::live::AcadLive;
use crate::drawing::live::equivalence::shared_budget_mtext_dxf;
use crate::drawing::live::tests::{fresh_cx, synthetic_rich_dxf};

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
