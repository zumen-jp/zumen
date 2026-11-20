//! 保持モデル・保存・取り消しを従来の逐次 apply と直接比較する。

use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Key, Kind, Limit, Limits, codes};
use zumen_drawing::{
    Affine, Applied, Changes, Drawing2D, Edge, Edit, Element, Font, FontKind, Hash, Inherit,
    PaperPoint, Placed, Restyle, Seen, Shape, id, transform,
};
use zumen_live::RowId;
use zumen_sdk::{Apply, Codec, Error, Lift};

use super::AcadLive;
use super::tests::{
    all_element_ids_and_hashes, fresh_cx, synthetic_associative_dxf, synthetic_rich_dxf,
};
use crate::{Dwg, Dxf};

struct AcadPair<C: Codec> {
    live: AcadLive,
    reference: C::File,
}

type DxfPair = AcadPair<Dxf>;
type DwgPair = AcadPair<Dwg>;

#[test]
#[expect(clippy::print_stdout, reason = "ネイティブと WASI の比較記録")]
fn paged_open_preserves_mixed_elements_and_budget_boundaries() {
    let input = String::from_utf8(synthetic_rich_dxf()).unwrap();
    let (prefix, suffix) = input.rsplit_once("  0\nENDSEC\n").unwrap();
    let mut input = prefix.to_owned();
    for index in 0..16_390 {
        let handle = 0x1000 + if index == 16_380 { 10 } else { index } * 3;
        if index == 246 {
            input.push_str(&format!(
                "0\nINSERT\n5\n{handle:X}\n8\n0\n2\nMARK\n66\n1\n10\n0\n20\n0\n\
                 0\nATTRIB\n5\n{:X}\n330\n{handle:X}\n8\n0\n1\nVALUE\n2\nCODE\n10\n1\n20\n1\n40\n2\n\
                 0\nSEQEND\n5\n{:X}\n330\n{handle:X}\n",
                handle + 1,
                handle + 2
            ));
            continue;
        }
        // 一般図形だけの区間、用紙、キーなし要素を、平面図形と同じ入力に置く。
        let kind = if (256..768).contains(&index) {
            "POINT"
        } else {
            "LINE"
        };
        input.push_str(&format!("0\n{kind}\n"));
        if index % 17 != 0 {
            input.push_str(&format!("5\n{handle:X}\n"));
        }
        if index % 71 == 0 {
            input.push_str("67\n1\n");
        }
        input.push_str(&format!("8\n0\n10\n{index}\n20\n2\n"));
        if kind == "LINE" {
            input.push_str(&format!("11\n{}\n21\n3\n", index + 1));
        }
    }
    input.push_str("  0\nENDSEC\n");
    input.push_str(suffix);
    let bytes = input.as_bytes();
    let (expected, expected_diag) = fresh_cx(|cx| Dxf::read(bytes, cx)?.lift(cx));
    let expected: Drawing2D = expected.unwrap();
    let probe = |threads, limits, cancel: &CancelToken| {
        let mut budget = Budget::new(limits);
        let mut diagnostics = Diagnostics::new();
        let result = AcadLive::open_with_threads(
            bytes,
            threads,
            &mut Cx::new(&mut budget, &mut diagnostics, cancel),
        )
        .map(|live| {
            (
                fresh_cx(|cx| live.drawing(cx)).0.unwrap(),
                fresh_cx(|cx| live.save(cx)).0.unwrap(),
            )
        });
        (
            result,
            diagnostics.into_vec(),
            Kind::ALL.map(|kind| budget.used(kind)),
        )
    };
    let cancel = CancelToken::new();
    let baseline = probe(1, Limits::default(), &cancel);
    assert_eq!(baseline.0.as_ref().unwrap().0, expected);
    assert_eq!(baseline.1, expected_diag);
    let saved = &baseline.0.as_ref().unwrap().1;
    let expected_bytes = fresh_cx(|cx| {
        let file = Dxf::read(bytes, cx)?;
        let mut output = Vec::new();
        Dxf::write(&file, &mut output, cx).map(|()| output)
    })
    .0
    .unwrap();
    assert_eq!(saved, &expected_bytes);
    let mut records = String::new();
    // 3 スレッド以上では、平面図形の復号を入力順の処理と重ねる。
    for threads in [1, 2, 3, 4] {
        assert_eq!(probe(threads, Limits::default(), &cancel), baseline);
        for kind in [Kind::Work, Kind::DecodedBytes] {
            let needed = baseline.2[Kind::ALL.iter().position(|&value| value == kind).unwrap()];
            assert!(needed > 0);
            for available in [0, needed / 2, needed - 1, needed] {
                let limits = Limits::default().with(kind, Limit::Finite(available));
                let actual = probe(threads, limits, &cancel);
                assert_eq!(actual, probe(1, limits, &cancel));
                if available < needed {
                    assert!(matches!(actual.0, Err(Error::Stopped(_))));
                } else {
                    assert_eq!(actual, baseline);
                }
                records.push_str(&format!(
                    "{threads} {kind:?} {available} {:?}\n",
                    Hash::document(format!("{actual:?}").as_bytes())
                ));
            }
        }
    }
    cancel.cancel();
    let stopped = probe(1, Limits::default(), &cancel);
    assert!(matches!(stopped.0, Err(Error::Stopped(_))));
    assert_eq!(probe(4, Limits::default(), &cancel), stopped);
    records.push_str(&format!("{stopped:?}"));
    println!(
        "live-open-budget saved={:?} model={:?} used={:?} boundaries={:?}",
        Hash::document(saved),
        Hash::document(format!("{expected:?}").as_bytes()),
        baseline.2,
        Hash::document(records.as_bytes())
    );
}

#[test]
fn duplicate_and_keyless_ids_keep_edit_results_after_open() {
    let input = b"0\nSECTION\n2\nHEADER\n9\n$INSUNITS\n70\n4\n0\nENDSEC\n\
        0\nSECTION\n2\nENTITIES\n\
        0\nLINE\n5\n10\n8\n0\n10\n0\n20\n0\n11\n1\n21\n0\n\
        0\nLINE\n5\n11\n8\n0\n10\n5\n20\n0\n11\n6\n21\n0\n\
        0\nLINE\n5\n10\n8\n0\n10\n10\n20\n0\n11\n11\n21\n0\n\
        0\nLINE\n8\n0\n10\n15\n20\n0\n11\n16\n21\n0\n\
        0\nENDSEC\n0\nEOF\n";
    let unique = String::from_utf8(input.to_vec())
        .unwrap()
        .replace("5\n10\n8\n0\n10\n10\n", "5\n12\n8\n0\n10\n10\n");
    for (bytes, duplicated) in [(input.as_slice(), true), (unique.as_bytes(), false)] {
        for threads in [1, 4] {
            let mut pair = DxfPair::open(bytes, threads);
            let model = fresh_cx(|cx| pair.reference.lift(cx)).0.unwrap();
            let names = all_element_ids_and_hashes(&model);
            assert_eq!(
                pair.live.state.non_unique_model_ids.len(),
                if duplicated { 3 } else { 1 }
            );
            for ((name, _), element) in names.iter().zip(&model.spaces[0].elements) {
                if element.source.key().is_none()
                    || (duplicated && element.source.key() == Some(Key::Handle(0x10)))
                {
                    let row = *pair.live.state.non_unique_model_ids.get(name).unwrap();
                    assert_eq!(&pair.live.element_of_row(row).unwrap(), element);
                }
            }
            for (name, _) in names {
                let model = fresh_cx(|cx| pair.reference.lift(cx)).0.unwrap();
                let (_, hash) = all_element_ids_and_hashes(&model)
                    .into_iter()
                    .find(|(current, _)| current == &name)
                    .unwrap();
                let edit = Edit::Transform {
                    target: Seen { id: name, hash },
                    transform: Affine::translation(1.0, 2.0),
                };
                if duplicated {
                    pair.reject(edit);
                } else {
                    pair.apply(edit);
                }
            }
            // ハンドルが重複する文書は、書き戻しの参照が空の変更でも `duplicate_handle` で断る。文書エンジンも
            // 参照の経路に入り、同じ結果と診断を返す。重複しない文書の空の変更は、読み直さずに通す
            let empty = Changes {
                document: None,
                edits: Vec::new(),
            };
            let before = fresh_cx(|cx| pair.live.save(cx)).0.unwrap();
            let ((actual, rereads), diagnostics) =
                fresh_cx(|cx| pair.live.apply_rereading(&empty, cx));
            let expected = fresh_cx(|cx| pair.reference.apply(&empty, cx));
            assert_eq!((actual, diagnostics), expected);
            assert_eq!(expected.0.is_err(), duplicated);
            assert_eq!(rereads, duplicated);
            assert_eq!(fresh_cx(|cx| pair.live.save(cx)).0.unwrap(), before);
        }
    }
}

/// 図面の範囲（`$EXTMIN`・`$EXTMAX`）を持つ ASCII の DXF で、文書エンジンが書き戻しの参照と同じヘッダーを書く。
/// 参照は、値を変えた図形と足した図形だけで範囲を広げ、除いた図形と値の変わらない図形は使わない。
/// 範囲が空（`$EXTMIN` が 1e+20）の図面と、範囲の Z がわずかに負の図面の両方で確かめる。
/// これまでの比較は、この 2 つの変数を持たない DXF と DWG だけを使っていたので、除いた図形で範囲を広げる誤りを見逃した
#[test]
fn unitless_dxf_drawing_reports_units_missing_like_lift() {
    let input = b"0
SECTION
2
HEADER
9
$ACADVER
1
AC1015
0
ENDSEC
        0
SECTION
2
ENTITIES
        0
LINE
5
10
8
0
10
0
20
0
11
1
21
0
        0
LINE
5
11
8
0
10
5
20
0
11
6
21
0
        0
ENDSEC
0
EOF
";
    for threads in [1, 4] {
        let mut pair = DxfPair::open(input, threads);
        let check = |pair: &DxfPair| {
            let (_, expected) = fresh_cx(|cx| pair.reference.lift(cx));
            let (_, actual) = fresh_cx(|cx| pair.live.drawing(cx));
            assert_eq!(actual, expected);
            assert!(
                actual
                    .iter()
                    .any(|diagnostic| diagnostic.code() == codes::DXF_UNITS_MISSING)
            );
        };
        check(&pair);
        let edit = Edit::Transform {
            target: pair.seen(0x10),
            transform: Affine::translation(1.0, 2.0),
        };
        pair.apply(edit);
        check(&pair);
        assert!(fresh_cx(|cx| pair.live.undo(cx)).0.unwrap());
        let (_, actual) = fresh_cx(|cx| pair.live.drawing(cx));
        let (_, expected) = fresh_cx(|cx| Dxf::read(input, cx)?.lift(cx));
        assert_eq!(actual, expected);
    }
}

#[test]
fn element_lift_diagnostics_follow_edits_like_reading_the_saved_file() {
    let input = synthetic_associative_dxf();
    let opened = DxfPair::open(&input, 1);
    let noted = opened.live.state.noted_records.clone();
    let last_noted = *noted.last().unwrap();
    let model = fresh_cx(|cx| opened.reference.lift(cx)).0.unwrap();
    let mut shifted = false;
    let mut gated = false;
    for element in &model.spaces[0].elements {
        let (Some(Key::Handle(handle)), Some(record)) =
            (element.source.key(), element.source.record())
        else {
            continue;
        };
        let remove = Edit::Remove {
            target: opened.seen(handle),
        };
        let changes = Changes {
            document: None,
            edits: vec![remove.clone()],
        };
        let mut probe = opened.live.clone();
        let (result, rereads) = probe.apply_rereading(
            &changes,
            &mut Cx::new(
                &mut Budget::default(),
                &mut Diagnostics::new(),
                &CancelToken::new(),
            ),
        );
        if result.is_err() {
            continue;
        }
        if noted.contains(&record) {
            // 診断の出た図形を差分の経路で除くと、保持した診断が古くなるので構造経路に任せる
            assert!(rereads);
            gated = true;
        }
        if rereads || record > last_noted {
            continue;
        }
        // 診断の出た図形より前を差分の経路で除くと、診断の場所のレコードの番号がずれる
        shifted = true;
        for threads in [1, 4] {
            let mut pair = DxfPair::open(&input, threads);
            pair.apply(remove.clone());
        }
    }
    assert!(shifted);
    assert!(gated);
}

#[test]
fn dxf_extents_follow_only_changed_and_added_elements() {
    for (bytes, handle) in [
        (
            include_bytes!("../../../../../samples/dxf/text_alignments.dxf").as_slice(),
            0x4E,
        ),
        (
            include_bytes!("../../../../../samples/dwg/zumen-layout-r2000_ascii.dxf").as_slice(),
            0x2DA,
        ),
        (
            include_bytes!("../../../../../samples/dwg/zumen-layout-r2018_ascii.dxf").as_slice(),
            0x2DA,
        ),
    ] {
        let element = |pair: &DxfPair| {
            fresh_cx(|cx| pair.reference.lift(cx))
                .0
                .unwrap()
                .spaces
                .into_iter()
                .flat_map(|space| space.elements)
                .find(|element| element.source.key() == Some(Key::Handle(handle)))
                .unwrap()
        };
        let mut pair = DxfPair::open(bytes, 1);
        pair.apply(Edit::Remove {
            target: pair.seen(handle),
        });
        let mut pair = DxfPair::open(bytes, 1);
        // 今と同じ色にする変更は値を変えないので、範囲を広げない
        let same = element(&pair).style.color;
        pair.apply(Edit::Restyle {
            target: pair.seen(handle),
            style: Restyle {
                color: Some(same),
                ..Restyle::default()
            },
        });
        pair.apply(Edit::Restyle {
            target: pair.seen(handle),
            style: Restyle {
                color: Some(Inherit::Own(zumen_drawing::Color {
                    code: Some(zumen_drawing::ColorCode {
                        system: zumen_drawing::ColorSystem::Aci,
                        value: 5,
                    }),
                    rgb: None,
                })),
                ..Restyle::default()
            },
        });
        pair.apply(Edit::Transform {
            target: pair.seen(handle),
            transform: Affine::translation(2.0, 3.0),
        });
        let mut pair = DxfPair::open(bytes, 1);
        let mut circle = element(&pair);
        let at = match &circle.shape {
            Shape::Line { start, .. } => *start,
            Shape::Circle { center, .. } => *center,
            shape => panic!("{shape:?}"),
        };
        circle.shape = Shape::Circle {
            center: zumen_drawing::ModelPoint {
                x: 100.0,
                y: 50.0,
                ..at
            },
            radius: zumen_drawing::Length::Model(zumen_drawing::ModelLength {
                space: at.space,
                value: 30.0,
            }),
        };
        pair.apply(Edit::Replace {
            target: pair.seen(handle),
            element: Box::new(Placed::Model(circle)),
        });
        pair.apply(Edit::Remove {
            target: pair.seen(handle),
        });
    }
}

impl<C: Codec> AcadPair<C>
where
    C::File: Lift<Drawing2D> + Apply<Changes, Output = Vec<Applied>>,
{
    fn open(bytes: &[u8], threads: usize) -> Self {
        let (reference, reference_diag) = fresh_cx(|cx| {
            let file = C::read(bytes, cx)?;
            file.lift(cx).map(|_: Drawing2D| file)
        });
        let (live, live_diag) = fresh_cx(|cx| AcadLive::open_with_threads(bytes, threads, cx));
        assert_eq!(live_diag, reference_diag);
        let pair = Self {
            live: live.unwrap(),
            reference: reference.unwrap(),
        };
        pair.assert_model();
        pair
    }

    fn assert_model(&self) {
        let (expected, expected_diag) = fresh_cx(|cx| self.reference.lift(cx));
        let (actual, actual_diag) = fresh_cx(|cx| self.live.drawing(cx));
        let actual = actual.unwrap();
        let expected = expected.unwrap();
        assert_eq!(actual, expected);
        assert_eq!(actual_diag, expected_diag);
        assert_eq!(
            all_element_ids_and_hashes(&actual),
            all_element_ids_and_hashes(&expected)
        );
        let saved = fresh_cx(|cx| self.live.save(cx)).0.unwrap();
        let (reloaded, diagnostics) =
            fresh_cx(|cx| C::read(&saved, cx).and_then(|file| file.lift(cx)));
        assert_eq!(reloaded.unwrap(), actual);
        assert_eq!(diagnostics, expected_diag);
        let model = &expected.spaces[0];
        let mut namer = id::Namer::new("space/0");
        let expected_targets: Vec<_> = model
            .elements
            .iter()
            .enumerate()
            .map(|(index, element)| Seen {
                id: namer.next(index, element),
                hash: Hash::element(element),
            })
            .collect();
        for index in 0..self.live.table().len() {
            let row = RowId::from_index(index).unwrap();
            if !self.live.table().is_alive(row) {
                assert_eq!(
                    fresh_cx(|cx| self.live.seen_of_row(row, cx)).0.unwrap(),
                    None
                );
                continue;
            }
            let seen = fresh_cx(|cx| self.live.seen_of_row(row, cx))
                .0
                .unwrap()
                .unwrap();
            assert!(expected_targets.contains(&seen), "row={index}: {seen:?}");
            let element = fresh_cx(|cx| self.live.element_with_source(row, cx))
                .0
                .unwrap()
                .unwrap();
            let expected = expected
                .spaces
                .iter()
                .flat_map(|space| &space.elements)
                .find(|candidate| candidate.source == element.source)
                .unwrap();
            assert_eq!(&element, expected, "row={index}");
            assert_eq!(Hash::element(&element), Hash::element(expected));
        }
    }

    fn seen(&self, handle: u64) -> Seen {
        let drawing = fresh_cx(|cx| self.reference.lift(cx)).0.unwrap();
        let (id, hash) = all_element_ids_and_hashes(&drawing)
            .into_iter()
            .find(|(id, _)| id.ends_with(&format!("/h{handle:X}")))
            .unwrap();
        Seen { id, hash }
    }

    fn apply(&mut self, edit: Edit) -> Vec<Applied> {
        self.apply_edits(vec![edit])
    }

    fn apply_edits(&mut self, edits: Vec<Edit>) -> Vec<Applied> {
        let before = fresh_cx(|cx| self.live.drawing(cx)).0.unwrap();
        let before_bytes = fresh_cx(|cx| self.live.save(cx)).0.unwrap();
        let changes = Changes {
            document: None,
            edits,
        };
        let (actual, diagnostics) = fresh_cx(|cx| self.live.apply(&changes, cx));
        let (expected, expected_diag) = fresh_cx(|cx| self.reference.apply(&changes, cx));
        assert!(expected.is_ok(), "{changes:?}: {expected_diag:?}");
        assert_eq!(actual, expected);
        assert_eq!(diagnostics, expected_diag);
        let saved = fresh_cx(|cx| self.live.save(cx)).0.unwrap();
        let expected_bytes = fresh_cx(|cx| {
            let mut bytes = Vec::new();
            C::write(&self.reference, &mut bytes, cx).map(|()| bytes)
        })
        .0
        .unwrap();
        assert_eq!(saved, expected_bytes);
        self.assert_model();
        assert!(fresh_cx(|cx| self.live.undo(cx)).0.unwrap());
        assert_eq!(fresh_cx(|cx| self.live.drawing(cx)).0.unwrap(), before);
        assert_eq!(fresh_cx(|cx| self.live.save(cx)).0.unwrap(), before_bytes);
        assert!(fresh_cx(|cx| self.live.redo(cx)).0.unwrap());
        assert_eq!(fresh_cx(|cx| self.live.save(cx)).0.unwrap(), saved);
        self.assert_model();
        actual.unwrap()
    }

    #[expect(clippy::print_stdout, reason = "ネイティブと WASI の比較記録")]
    fn apply_with_budgets(&mut self, edit: Edit) -> Vec<Applied> {
        let mut original = self.live.clone();
        original.display_mut().clear_dirty();
        original.clear_dirty_symbols();
        let before = fresh_cx(|cx| original.drawing(cx)).0.unwrap();
        let before_bytes = fresh_cx(|cx| original.save(cx)).0.unwrap();
        let history = (
            original.state_id(),
            original.undo_len(),
            original.redo_len(),
        );
        let changes = Changes {
            document: None,
            edits: vec![edit.clone()],
        };
        let expected = self.apply(edit);
        let after = fresh_cx(|cx| self.live.drawing(cx)).0.unwrap();
        let after_bytes = fresh_cx(|cx| self.live.save(cx)).0.unwrap();
        let mut live = original.clone();
        let mut measured = Budget::default();
        let mut diagnostics = Diagnostics::new();
        let cancel = CancelToken::new();
        assert_eq!(
            live.apply(
                &changes,
                &mut Cx::new(&mut measured, &mut diagnostics, &cancel)
            )
            .unwrap(),
            expected
        );
        let expected_diagnostics = diagnostics.into_vec();
        let mut boundary_record = String::new();
        for kind in [Kind::Work, Kind::DecodedBytes] {
            let needed = measured.used(kind);
            assert!(needed > 0);
            for available in [0, needed / 2, needed - 1, needed] {
                let mut live = original.clone();
                let mut budget =
                    Budget::new(Limits::default().with(kind, Limit::Finite(available + 11)));
                budget.spend(kind, 11).unwrap();
                let mut diagnostics = Diagnostics::new();
                let actual = live.apply(
                    &changes,
                    &mut Cx::new(&mut budget, &mut diagnostics, &cancel),
                );
                boundary_record.push_str(&format!(
                    "{kind:?} {available} {actual:?} {:?} {:?} {}\n",
                    Kind::ALL.map(|kind| budget.used(kind)),
                    diagnostics.iter().collect::<Vec<_>>(),
                    live.history_bytes()
                ));
                if available < needed {
                    assert_stop_reported(&actual, &diagnostics);
                    assert_eq!((live.state_id(), live.undo_len(), live.redo_len()), history);
                    assert!(!live.display().has_dirty());
                    assert!(live.dirty_symbols().is_empty());
                    assert_eq!(fresh_cx(|cx| live.drawing(cx)).0.unwrap(), before);
                    assert_eq!(fresh_cx(|cx| live.save(cx)).0.unwrap(), before_bytes);
                } else {
                    assert_eq!(actual.unwrap(), expected);
                    assert_eq!(diagnostics.into_vec(), expected_diagnostics);
                    assert_eq!(budget.used(kind), needed + 11);
                    assert_eq!(fresh_cx(|cx| live.drawing(cx)).0.unwrap(), after);
                    assert_eq!(fresh_cx(|cx| live.save(cx)).0.unwrap(), after_bytes);
                }
            }
        }
        let mut live = original;
        cancel.cancel();
        let mut budget = Budget::default();
        let mut diagnostics = Diagnostics::new();
        let actual = live.apply(
            &changes,
            &mut Cx::new(&mut budget, &mut diagnostics, &cancel),
        );
        assert_stop_reported(&actual, &diagnostics);
        assert_eq!((live.state_id(), live.undo_len(), live.redo_len()), history);
        assert!(!live.display().has_dirty());
        assert!(live.dirty_symbols().is_empty());
        assert_eq!(fresh_cx(|cx| live.drawing(cx)).0.unwrap(), before);
        assert_eq!(fresh_cx(|cx| live.save(cx)).0.unwrap(), before_bytes);
        println!(
            "live-edit-budget hash={:?} used={:?} history={} boundaries={:?}",
            Hash::document(&after_bytes),
            Kind::ALL.map(|kind| measured.used(kind)),
            self.live.history_bytes(),
            Hash::document(boundary_record.as_bytes())
        );
        expected
    }

    fn reject(&mut self, edit: Edit) {
        let before = fresh_cx(|cx| self.live.drawing(cx)).0.unwrap();
        let bytes = fresh_cx(|cx| self.live.save(cx)).0.unwrap();
        let history = self.live.undo_len();
        let changes = Changes {
            document: None,
            edits: vec![edit],
        };
        let actual = fresh_cx(|cx| self.live.apply(&changes, cx));
        let expected = fresh_cx(|cx| self.reference.apply(&changes, cx));
        assert!(expected.0.is_err());
        assert_eq!(actual, expected);
        assert_eq!(fresh_cx(|cx| self.live.drawing(cx)).0.unwrap(), before);
        assert_eq!(fresh_cx(|cx| self.live.save(cx)).0.unwrap(), bytes);
        assert_eq!(self.live.undo_len(), history);
        self.assert_model();
    }

    fn reject_with_budgets(&mut self, edit: Edit) {
        self.reject(edit.clone());
        let before = fresh_cx(|cx| self.live.drawing(cx)).0.unwrap();
        let bytes = fresh_cx(|cx| self.live.save(cx)).0.unwrap();
        let history = (
            self.live.state_id(),
            self.live.undo_len(),
            self.live.redo_len(),
        );
        let changes = Changes {
            document: None,
            edits: vec![edit],
        };
        let cancel = CancelToken::new();
        let mut measured = Budget::default();
        let mut diagnostics = Diagnostics::new();
        assert_eq!(
            self.live.apply(
                &changes,
                &mut Cx::new(&mut measured, &mut diagnostics, &cancel)
            ),
            Err(Error::Rejected)
        );
        assert!(measured.used(Kind::Work) > 0);
        for kind in [Kind::Work, Kind::DecodedBytes, Kind::InputBytes] {
            let needed = measured.used(kind);
            for available in [0, needed / 2, needed.saturating_sub(1), needed] {
                let mut limited =
                    Budget::new(Limits::default().with(kind, Limit::Finite(available + 7)));
                limited.spend(kind, 7).unwrap();
                let mut diagnostics = Diagnostics::new();
                let result = self.live.apply(
                    &changes,
                    &mut Cx::new(&mut limited, &mut diagnostics, &cancel),
                );
                if available < needed {
                    assert_stop_reported(&result, &diagnostics);
                } else {
                    assert_eq!(result, Err(Error::Rejected));
                }
                assert_eq!(
                    (
                        self.live.state_id(),
                        self.live.undo_len(),
                        self.live.redo_len()
                    ),
                    history
                );
                assert_eq!(fresh_cx(|cx| self.live.save(cx)).0.unwrap(), bytes);
            }
        }
        cancel.cancel();
        let mut budget = Budget::default();
        let mut diagnostics = Diagnostics::new();
        let result = self.live.apply(
            &changes,
            &mut Cx::new(&mut budget, &mut diagnostics, &cancel),
        );
        assert_stop_reported(&result, &diagnostics);
        assert_eq!(
            (
                self.live.state_id(),
                self.live.undo_len(),
                self.live.redo_len()
            ),
            history
        );
        assert_eq!(fresh_cx(|cx| self.live.drawing(cx)).0.unwrap(), before);
        assert_eq!(fresh_cx(|cx| self.live.save(cx)).0.unwrap(), bytes);
    }
}

/// 止まった `apply` は、止まった理由の診断を呼び出し元へ報告する。保存したバイト列を読み直す経路は
/// 変換の診断を捨てる収集先で読むので、そこで止まっても理由が呼び出し元に届くことを確かめる
#[track_caller]
fn assert_stop_reported<T: std::fmt::Debug>(result: &Result<T, Error>, diagnostics: &Diagnostics) {
    let Err(Error::Stopped(stop)) = result else {
        panic!("止まるはずが {result:?}");
    };
    let code = stop.to_diagnostic().code();
    assert!(
        diagnostics.iter().any(|item| item.code() == code),
        "{stop:?} の診断がない: {:?}",
        diagnostics.iter().collect::<Vec<_>>()
    );
}

#[test]
fn added_and_replaced_elements_share_budget_and_cancellation() {
    for threads in [1, 4] {
        let mut pair = DxfPair::open(&shared_budget_mtext_dxf(), threads);
        let model = fresh_cx(|cx| pair.reference.lift(cx)).0.unwrap();
        let element = model.spaces[0]
            .elements
            .iter()
            .find(|element| element.source.key() == Some(Key::Handle(0x120)))
            .unwrap()
            .clone();
        let added = pair
            .apply_with_budgets(Edit::Add {
                place: "space/0".to_owned(),
                element: Box::new(Placed::Model(element.clone())),
            })
            .pop()
            .unwrap();
        let mut replacement = element;
        replacement.shape =
            transform::transform(&replacement.shape, &Affine::translation(2.0, 3.0)).unwrap();
        let replaced = pair
            .apply_with_budgets(Edit::Replace {
                target: Seen {
                    id: added.id.unwrap(),
                    hash: added.hash.unwrap(),
                },
                element: Box::new(Placed::Model(replacement)),
            })
            .pop()
            .unwrap();
        pair.apply_with_budgets(Edit::Remove {
            target: Seen {
                id: replaced.id.unwrap(),
                hash: replaced.hash.unwrap(),
            },
        });
    }
}

#[test]
fn model_and_block_edits_share_budget_and_cancellation() {
    for (input, handles) in [
        (synthetic_rich_dxf(), vec![0x101, 0x55]),
        (shared_budget_mtext_dxf(), vec![0x120]),
    ] {
        for threads in [1, 4] {
            let mut pair = DxfPair::open(&input, threads);
            for handle in &handles {
                pair.apply_with_budgets(Edit::Transform {
                    target: pair.seen(*handle),
                    transform: Affine::translation(2.0, 3.0),
                });
            }
        }
    }
}

pub(super) fn shared_budget_mtext_dxf() -> Vec<u8> {
    let mut input = String::from_utf8(synthetic_rich_dxf()).unwrap();
    let at = input.rfind("  0\nENDSEC\n").unwrap();
    let content = "NOTE \\C1;red\\P".repeat(64);
    input.insert_str(
        at,
        &format!(
            "0\nMTEXT\n5\n120\n330\n41\n100\nAcDbEntity\n8\n0\n100\nAcDbMText\n10\n50\n20\n25\n30\n0\n40\n2.5\n41\n20\n71\n1\n72\n5\n1\n{content}\n7\nStandard\n"
        ),
    );
    input.into_bytes()
}

#[test]
fn dwg_model_edits_share_budget_and_cancellation() {
    let text_model: Drawing2D =
        fresh_cx(|cx| Dxf::read(&shared_budget_mtext_dxf(), cx).and_then(|file| file.lift(cx)))
            .0
            .unwrap();
    let original_text = text_model.spaces[0]
        .elements
        .iter()
        .find(|element| element.source.key() == Some(Key::Handle(0x120)))
        .unwrap();
    for bytes in [
        include_bytes!("../../../../../samples/dwg/zumen-layout-r2000.dwg").as_slice(),
        include_bytes!("../../../../../samples/dwg/zumen-layout-r2004.dwg").as_slice(),
        include_bytes!("../../../../../samples/dwg/zumen-layout-r2018.dwg").as_slice(),
    ] {
        for threads in [1, 4] {
            let mut pair = DwgPair::open(bytes, threads);
            let drawing = fresh_cx(|cx| pair.reference.lift(cx)).0.unwrap();
            let original = drawing.spaces[0]
                .elements
                .iter()
                .find(|element| matches!(element.shape, Shape::Line { .. } | Shape::Circle { .. }))
                .unwrap();
            let Some(Key::Handle(handle)) = original.source.key() else {
                panic!("handle required");
            };
            pair.apply_with_budgets(Edit::Transform {
                target: pair.seen(handle),
                transform: Affine::translation(2.0, 3.0),
            });
            pair.apply_with_budgets(Edit::Restyle {
                target: pair.seen(handle),
                style: Restyle {
                    line_type_scale: Some(2.0),
                    ..Restyle::default()
                },
            });
            let current = fresh_cx(|cx| pair.reference.lift(cx)).0.unwrap();
            let mut replacement = current.spaces[0]
                .elements
                .iter()
                .find(|element| element.source.key() == Some(Key::Handle(handle)))
                .unwrap()
                .clone();
            replacement.shape =
                transform::transform(&replacement.shape, &Affine::translation(-3.0, 4.0)).unwrap();
            pair.apply_with_budgets(Edit::Replace {
                target: pair.seen(handle),
                element: Box::new(Placed::Model(replacement)),
            });
            let mut text = original_text.clone();
            text.layer = drawing.layers[0].id;
            text.style = original.style;
            let added = pair
                .apply_with_budgets(Edit::Add {
                    place: "space/0".to_owned(),
                    element: Box::new(Placed::Model(text.clone())),
                })
                .pop()
                .unwrap();
            let moved = pair
                .apply_with_budgets(Edit::Transform {
                    target: Seen {
                        id: added.id.unwrap(),
                        hash: added.hash.unwrap(),
                    },
                    transform: Affine::translation(2.0, 3.0),
                })
                .pop()
                .unwrap();
            let current = fresh_cx(|cx| pair.reference.lift(cx)).0.unwrap();
            let mut text = current.spaces[0].elements.last().unwrap().clone();
            text.shape =
                transform::transform(&text.shape, &Affine::translation(-3.0, 4.0)).unwrap();
            let replaced = pair
                .apply_with_budgets(Edit::Replace {
                    target: Seen {
                        id: moved.id.unwrap(),
                        hash: moved.hash.unwrap(),
                    },
                    element: Box::new(Placed::Model(text)),
                })
                .pop()
                .unwrap();
            pair.apply_with_budgets(Edit::Remove {
                target: Seen {
                    id: replaced.id.unwrap(),
                    hash: replaced.hash.unwrap(),
                },
            });
        }
    }
}

#[test]
fn dwg_block_edits_share_budget_and_cancellation() {
    for bytes in [
        include_bytes!("../../../../../samples/dwg/zumen-minsert-r2000.dwg").as_slice(),
        include_bytes!("../../../../../samples/dwg/zumen-minsert-r2004.dwg").as_slice(),
        include_bytes!("../../../../../samples/dwg/zumen-minsert-r2018.dwg").as_slice(),
    ] {
        for threads in [1, 4] {
            let mut pair = DwgPair::open(bytes, threads);
            let drawing = fresh_cx(|cx| pair.reference.lift(cx)).0.unwrap();
            let element = drawing
                .symbols
                .iter()
                .flat_map(|symbol| &symbol.space.elements)
                .find(|element| matches!(element.shape, Shape::Polyline(_)))
                .unwrap();
            let Some(Key::Handle(handle)) = element.source.key() else {
                panic!("handle required");
            };
            pair.apply_with_budgets(Edit::Transform {
                target: pair.seen(handle),
                transform: Affine::translation(2.0, 3.0),
            });
            pair.apply_with_budgets(Edit::Restyle {
                target: pair.seen(handle),
                style: Restyle {
                    line_type_scale: Some(2.0),
                    ..Restyle::default()
                },
            });
            let current = fresh_cx(|cx| pair.reference.lift(cx)).0.unwrap();
            let mut replacement = current
                .symbols
                .iter()
                .flat_map(|symbol| &symbol.space.elements)
                .find(|element| element.source.key() == Some(Key::Handle(handle)))
                .unwrap()
                .clone();
            replacement.shape =
                transform::transform(&replacement.shape, &Affine::translation(-3.0, 4.0)).unwrap();
            pair.apply_with_budgets(Edit::Replace {
                target: pair.seen(handle),
                element: Box::new(Placed::Model(replacement)),
            });
        }
    }
}

/// 属性を持つ挿入を動かしてから、属性の値を変えて置き換える。置き換えは保存したバイト列を読み直す経路に入り、
/// 読み直しと作り直しの途中で予算が尽きても、止まった理由の診断を呼び出し元へ返す（#878 への独立レビューの報告）
#[test]
fn dwg_attributed_symbol_replace_reports_stops_while_rereading() {
    for bytes in [
        include_bytes!("../../../../../samples/dwg/zumen-minsert-r2000.dwg").as_slice(),
        include_bytes!("../../../../../samples/dwg/zumen-minsert-r2018.dwg").as_slice(),
    ] {
        for threads in [1, 4] {
            let mut pair = DwgPair::open(bytes, threads);
            let drawing = fresh_cx(|cx| pair.reference.lift(cx)).0.unwrap();
            let handle = drawing.spaces[0]
                .elements
                .iter()
                .find_map(|element| match (&element.shape, element.source.key()) {
                    (Shape::Symbol(symbol), Some(Key::Handle(handle)))
                        if !symbol.attributes.is_empty() =>
                    {
                        Some(handle)
                    }
                    _ => None,
                })
                .unwrap();
            pair.apply(Edit::Transform {
                target: pair.seen(handle),
                transform: Affine::translation(2.0, 3.0),
            });
            let current = fresh_cx(|cx| pair.reference.lift(cx)).0.unwrap();
            let mut replacement = current.spaces[0]
                .elements
                .iter()
                .find(|element| element.source.key() == Some(Key::Handle(handle)))
                .unwrap()
                .clone();
            let Shape::Symbol(symbol) = &mut replacement.shape else {
                panic!("symbol required");
            };
            for attribute in &mut symbol.attributes {
                attribute.text.content.push_str("-edited");
            }
            let edit = Edit::Replace {
                target: pair.seen(handle),
                element: Box::new(Placed::Model(replacement)),
            };
            // 書き戻しの参照（今までの `Session` の経路）も、変換し直す途中で止まった理由を返す
            let changes = Changes {
                document: None,
                edits: vec![edit.clone()],
            };
            let cancel = CancelToken::new();
            let mut measured = Budget::default();
            let mut diagnostics = Diagnostics::new();
            pair.reference
                .clone()
                .apply(
                    &changes,
                    &mut Cx::new(&mut measured, &mut diagnostics, &cancel),
                )
                .unwrap();
            for kind in [Kind::Work, Kind::DecodedBytes] {
                let needed = measured.used(kind);
                if needed == 0 {
                    continue;
                }
                for step in 0..64 {
                    let available = needed * step / 64;
                    let mut file = pair.reference.clone();
                    let mut budget =
                        Budget::new(Limits::default().with(kind, Limit::Finite(available)));
                    let mut diagnostics = Diagnostics::new();
                    let result = file.apply(
                        &changes,
                        &mut Cx::new(&mut budget, &mut diagnostics, &cancel),
                    );
                    assert_stop_reported(&result, &diagnostics);
                }
            }
            pair.apply_with_budgets(edit);
        }
    }
}

#[test]
fn implicit_layer_removal_preserves_ids_hashes_and_undo() {
    let mut input =
        "0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1015\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n"
            .to_owned();
    for (handle, layer) in [(10, "FIRST"), (11, "SECOND"), (12, "FIRST"), (13, "THIRD")] {
        input.push_str(&format!(
            "0\nLINE\n5\n{handle:X}\n8\n{layer}\n10\n0\n20\n0\n11\n10\n21\n5\n"
        ));
    }
    input.push_str("0\nENDSEC\n0\nEOF\n");
    for threads in [1, 4] {
        let mut pair = DxfPair::open(input.as_bytes(), threads);
        for handle in [10, 12] {
            pair.apply(Edit::Remove {
                target: pair.seen(handle),
            });
        }
        let model = fresh_cx(|cx| pair.live.drawing(cx)).0.unwrap();
        assert_eq!(
            model
                .layers
                .iter()
                .map(|layer| layer.name.as_str())
                .collect::<Vec<_>>(),
            ["SECOND", "THIRD"]
        );
    }
}

/// POLYLINE のスタイルを変えると、文書エンジンも続く VERTEX と SEQEND を従来の経路と同じに書く（#885）。
/// 実寸の空間・用紙（67 が 1）・名前付きブロック（MARK）の定義の中で確かめる
#[test]
fn polyline_restyles_reach_the_vertices_like_the_batch_apply() {
    let polyline = |owner: &str, paper: &str| {
        let vertex = |handle: &str, x: &str| {
            format!(
                "0\nVERTEX\n5\n{handle}\n330\n1F0\n100\nAcDbEntity\n{paper}8\n0\n62\n2\n\
                 100\nAcDbVertex\n100\nAcDb2dVertex\n10\n{x}\n20\n0\n30\n0\n"
            )
        };
        format!(
            "0\nPOLYLINE\n5\n1F0\n330\n{owner}\n100\nAcDbEntity\n{paper}8\n0\n62\n2\n\
             100\nAcDb2dPolyline\n66\n1\n10\n0\n20\n0\n30\n0\n70\n0\n{}{}\
             0\nSEQEND\n5\n1F3\n330\n1F0\n100\nAcDbEntity\n{paper}8\n0\n",
            vertex("1F1", "0"),
            vertex("1F2", "3"),
        )
    };
    let rich = String::from_utf8(synthetic_rich_dxf()).unwrap();
    let insert = |at: usize, body: &str| {
        let mut input = rich.clone();
        input.insert_str(at, body);
        input
    };
    let entities_end = rich.rfind("  0\nENDSEC\n").unwrap();
    let block_end = rich.find("  0\nENDBLK\n  5\n57\n").unwrap();
    let inputs = [
        ("model", insert(entities_end, &polyline("41", ""))),
        ("paper", insert(entities_end, &polyline("42", "67\n1\n"))),
        ("block", insert(block_end, &polyline("43", ""))),
    ];
    for (placement, input) in inputs {
        for threads in [1, 4] {
            let mut pair = DxfPair::open(input.as_bytes(), threads);
            let style = Restyle {
                layer: Some(pair.layer_id("WALL")),
                color: Some(Inherit::Own(zumen_drawing::Color {
                    code: Some(zumen_drawing::ColorCode {
                        system: zumen_drawing::ColorSystem::Aci,
                        value: 6,
                    }),
                    rgb: None,
                })),
                line_type: Some(Inherit::Own(pair.line_type_id("DASHED"))),
                ..Restyle::default()
            };
            pair.apply(Edit::Restyle {
                target: pair.seen(0x1F0),
                style,
            });
            let saved = String::from_utf8(fresh_cx(|cx| pair.live.save(cx)).0.unwrap()).unwrap();
            // ハンドルの行から次のレコード（0 の組）の前までを切り出す。用紙では 67 が画層の前に入る
            let record = |handle: &str| -> String {
                let Some(start) = saved.find(&format!("\n{handle}\n")) else {
                    return String::new();
                };
                let rest = saved.get(start + 1..).unwrap_or_default();
                let end = [rest.find("\n0\n"), rest.find("\n  0\n")]
                    .into_iter()
                    .flatten()
                    .min()
                    .map_or(rest.len(), |at| at + 1);
                rest.get(..end).unwrap_or_default().to_owned()
            };
            for handle in ["1F1", "1F2"] {
                let vertex = record(handle);
                for expected in ["8\nWALL\n", "6\nDASHED\n", "62\n6\n"] {
                    assert!(
                        vertex.contains(expected),
                        "{placement}: {handle} {expected}\n{saved}"
                    );
                }
            }
            let seqend = record("1F3");
            assert!(seqend.contains("8\nWALL\n"), "{placement}: {seqend:?}");
        }
    }
}

/// 表のない ASCII DXF。`lines` は（ハンドル・画層・線種）の LINE、`blocks` は BLOCKS の中身
fn implicit_dxf(blocks: &str, lines: &[(u64, &str, Option<&str>)]) -> Vec<u8> {
    let mut input = "0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1015\n0\nENDSEC\n".to_owned();
    if !blocks.is_empty() {
        input.push_str("0\nSECTION\n2\nBLOCKS\n");
        input.push_str(blocks);
        input.push_str("0\nENDSEC\n");
    }
    input.push_str("0\nSECTION\n2\nENTITIES\n");
    for &(handle, layer, line_type) in lines {
        input.push_str(&format!("0\nLINE\n5\n{handle:X}\n8\n{layer}\n"));
        if let Some(line_type) = line_type {
            input.push_str(&format!("6\n{line_type}\n"));
        }
        input.push_str(&format!("10\n{handle}\n20\n0\n11\n10\n21\n5\n"));
    }
    input.push_str("0\nENDSEC\n0\nEOF\n");
    input.into_bytes()
}

/// ブロック定義の中だけで使う画層 BLK
const BLOCK_ONLY_LAYER: &str = "0\nBLOCK\n5\n20\n8\nBLK\n2\nB1\n70\n0\n10\n0\n20\n0\n30\n0\n3\nB1\n\
0\nLINE\n5\n21\n8\nBLK\n10\n0\n20\n0\n11\n1\n21\n1\n0\nENDBLK\n5\n22\n8\nBLK\n";

/// 生きた行の表示スタイルを要素の ID ごとに集める
fn display_styles_by_id(live: &AcadLive) -> std::collections::BTreeMap<String, [Vec<[u32; 2]>; 4]> {
    let mut styles = std::collections::BTreeMap::new();
    for index in 0..live.table().len() {
        let row = RowId::from_index(index).unwrap();
        if !live.table().is_alive(row) {
            continue;
        }
        let seen = fresh_cx(|cx| live.seen_of_row(row, cx)).0.unwrap().unwrap();
        styles.insert(
            seen.id,
            live.display().row_styles(u32::try_from(index).unwrap()),
        );
    }
    styles
}

impl DxfPair {
    fn layer_id(&self, name: &str) -> zumen_drawing::LayerId {
        let model = fresh_cx(|cx| self.live.drawing(cx)).0.unwrap();
        model
            .layers
            .iter()
            .find(|layer| layer.name == name)
            .unwrap()
            .id
    }

    fn line_type_id(&self, name: &str) -> zumen_drawing::LineTypeId {
        let model = fresh_cx(|cx| self.live.drawing(cx)).0.unwrap();
        model
            .line_types
            .iter()
            .find(|line_type| line_type.name == name)
            .unwrap()
            .id
    }

    /// まだない画層「0」の ID（表のない文書で編集が新しく名指せる唯一の名前）
    fn new_zero_layer(&self) -> zumen_drawing::LayerId {
        let model = fresh_cx(|cx| self.live.drawing(cx)).0.unwrap();
        assert!(model.layers.iter().all(|layer| layer.name != "0"));
        zumen_drawing::LayerId(u32::try_from(model.layers.len()).unwrap())
    }

    fn element(&self, handle: u64) -> Element<zumen_drawing::ModelPoint> {
        let model = fresh_cx(|cx| self.live.drawing(cx)).0.unwrap();
        model.spaces[0]
            .elements
            .iter()
            .find(|element| element.source.key() == Some(Key::Handle(handle)))
            .unwrap()
            .clone()
    }

    fn assert_display_matches_reopen(&self) {
        let saved = fresh_cx(|cx| self.live.save(cx)).0.unwrap();
        let reopened = fresh_cx(|cx| AcadLive::open_with_threads(&saved, 1, cx))
            .0
            .unwrap();
        assert_eq!(
            display_styles_by_id(&self.live),
            display_styles_by_id(&reopened)
        );
    }

    /// 編集を 1 回の apply で適用して従来の apply と比べ、高速経路かどうかと表示を確かめる
    fn apply_implicit(&mut self, edits: Vec<Edit>, fast: bool) -> Vec<Applied> {
        let applied = self.apply_edits(edits);
        assert_eq!(self.live.journal.last_is_fast(), Some(fast));
        self.assert_display_matches_reopen();
        assert!(fresh_cx(|cx| self.live.undo(cx)).0.unwrap());
        self.assert_display_matches_reopen();
        assert!(fresh_cx(|cx| self.live.redo(cx)).0.unwrap());
        self.assert_display_matches_reopen();
        applied
    }
}

#[test]
fn implicit_first_reference_removal_reorders_on_fast_path() {
    let input = implicit_dxf(
        "",
        &[(0x10, "A", None), (0x11, "B", None), (0x12, "A", None)],
    );
    let names = |pair: &DxfPair| {
        fresh_cx(|cx| pair.live.drawing(cx))
            .0
            .unwrap()
            .layers
            .into_iter()
            .map(|layer| layer.name)
            .collect::<Vec<_>>()
    };
    for threads in [1, 4] {
        let mut pair = DxfPair::open(&input, threads);
        pair.assert_display_matches_reopen();
        pair.apply_implicit(
            vec![Edit::Remove {
                target: pair.seen(0x10),
            }],
            true,
        );
        assert_eq!(names(&pair), ["B", "A"]);
        pair.apply_implicit(
            vec![Edit::Remove {
                target: pair.seen(0x12),
            }],
            true,
        );
        assert_eq!(names(&pair), ["B"]);
    }
}

#[test]
fn implicit_last_reference_removal_drops_layer_on_fast_path() {
    let input = implicit_dxf(
        "",
        &[(0x10, "A", None), (0x11, "B", None), (0x12, "C", None)],
    );
    for threads in [1, 4] {
        let mut pair = DxfPair::open(&input, threads);
        pair.apply_implicit(
            vec![Edit::Remove {
                target: pair.seen(0x11),
            }],
            true,
        );
        pair.apply_implicit(
            vec![Edit::Transform {
                target: pair.seen(0x12),
                transform: Affine::translation(1.0, 2.0),
            }],
            true,
        );
    }
}

#[test]
fn implicit_restyle_and_replace_onto_new_and_existing_layers_use_fast_path() {
    let input = implicit_dxf(
        "",
        &[(0x10, "A", None), (0x11, "B", None), (0x12, "C", None)],
    );
    for threads in [1, 4] {
        let mut pair = DxfPair::open(&input, threads);
        // 既存の暗黙の画層へ移し、A が消えて番号が詰まる
        pair.apply_implicit(
            vec![Edit::Restyle {
                target: pair.seen(0x10),
                style: Restyle {
                    layer: Some(pair.layer_id("C")),
                    ..Restyle::default()
                },
            }],
            true,
        );
        // 置き換えで画層を変えることは従来の apply も断る
        let mut replacement = pair.element(0x11);
        replacement.layer = pair.new_zero_layer();
        pair.reject(Edit::Replace {
            target: pair.seen(0x11),
            element: Box::new(Placed::Model(replacement)),
        });
        // まだない画層「0」へ移す
        pair.apply_implicit(
            vec![Edit::Restyle {
                target: pair.seen(0x11),
                style: Restyle {
                    layer: Some(pair.new_zero_layer()),
                    ..Restyle::default()
                },
            }],
            true,
        );
        // 番号が動いたあとの形だけの置き換え
        let mut replacement = pair.element(0x12);
        if let Shape::Line { end, .. } = &mut replacement.shape {
            end.x += 1.0;
        }
        pair.apply_implicit(
            vec![Edit::Replace {
                target: pair.seen(0x12),
                element: Box::new(Placed::Model(replacement)),
            }],
            true,
        );
        // 既存の「0」へ移し、C が消える
        pair.apply_implicit(
            vec![Edit::Restyle {
                target: pair.seen(0x12),
                style: Restyle {
                    layer: Some(pair.layer_id("0")),
                    ..Restyle::default()
                },
            }],
            true,
        );

        let mut pair = DxfPair::open(&input, threads);
        pair.apply_implicit(
            vec![Edit::Restyle {
                target: pair.seen(0x12),
                style: Restyle {
                    layer: Some(pair.new_zero_layer()),
                    ..Restyle::default()
                },
            }],
            true,
        );
    }
}

#[test]
fn implicit_layer_add_uses_fast_path_unless_block_layer_ids_move() {
    let lines = [(0x10, "A", None), (0x11, "B", None)];
    for threads in [1, 4] {
        let mut pair = DxfPair::open(&implicit_dxf("", &lines), threads);
        let mut element = pair.element(0x10);
        element.layer = pair.new_zero_layer();
        pair.apply_implicit(
            vec![Edit::Add {
                place: "space/0".to_owned(),
                element: Box::new(Placed::Model(element)),
            }],
            true,
        );
        let mut element = pair.element(0x11);
        element.layer = pair.layer_id("A");
        pair.apply_implicit(
            vec![Edit::Add {
                place: "space/0".to_owned(),
                element: Box::new(Placed::Model(element)),
            }],
            true,
        );

        // ブロック定義だけが使う画層の番号が動く追加は構造経路に任せる
        let mut pair = DxfPair::open(&implicit_dxf(BLOCK_ONLY_LAYER, &lines), threads);
        pair.assert_display_matches_reopen();
        let mut element = pair.element(0x11);
        element.layer = pair.layer_id("A");
        pair.apply_implicit(
            vec![Edit::Add {
                place: "space/0".to_owned(),
                element: Box::new(Placed::Model(element)),
            }],
            true,
        );
        pair.apply_implicit(
            vec![Edit::Remove {
                target: pair.seen(0x10),
            }],
            true,
        );
        let mut element = pair.element(0x11);
        element.layer = pair.new_zero_layer();
        pair.apply_implicit(
            vec![Edit::Add {
                place: "space/0".to_owned(),
                element: Box::new(Placed::Model(element)),
            }],
            false,
        );
    }
}

#[test]
fn implicit_line_type_edits_use_fast_path() {
    let input = implicit_dxf(
        "",
        &[
            (0x10, "A", Some("DASHED")),
            (0x11, "A", Some("CENTER")),
            (0x12, "A", Some("DASHED")),
        ],
    );
    for threads in [1, 4] {
        let mut pair = DxfPair::open(&input, threads);
        pair.apply_implicit(
            vec![Edit::Remove {
                target: pair.seen(0x10),
            }],
            true,
        );
        pair.apply_implicit(
            vec![Edit::Restyle {
                target: pair.seen(0x11),
                style: Restyle {
                    line_type: Some(Inherit::Own(pair.line_type_id("DASHED"))),
                    ..Restyle::default()
                },
            }],
            true,
        );
        pair.apply_implicit(
            vec![Edit::Restyle {
                target: pair.seen(0x12),
                style: Restyle {
                    line_type: Some(Inherit::Layer),
                    ..Restyle::default()
                },
            }],
            true,
        );
    }
}

#[test]
fn several_implicit_edits_in_one_apply_use_fast_path() {
    let input = implicit_dxf(
        "",
        &[
            (0x10, "A", Some("DASHED")),
            (0x11, "B", None),
            (0x12, "A", Some("CENTER")),
            (0x13, "C", None),
        ],
    );
    for threads in [1, 4] {
        let mut pair = DxfPair::open(&input, threads);
        let mut added = pair.element(0x13);
        added.layer = pair.layer_id("B");
        let edits = vec![
            Edit::Remove {
                target: pair.seen(0x10),
            },
            Edit::Restyle {
                target: pair.seen(0x11),
                style: Restyle {
                    layer: Some(pair.new_zero_layer()),
                    ..Restyle::default()
                },
            },
            Edit::Transform {
                target: pair.seen(0x13),
                transform: Affine::translation(1.0, 2.0),
            },
            Edit::Add {
                place: "space/0".to_owned(),
                element: Box::new(Placed::Model(added)),
            },
        ];
        pair.apply_implicit(edits, true);
    }
}

#[test]
fn stopped_implicit_edits_leave_state_unchanged() {
    let input = implicit_dxf(
        "",
        &[(0x10, "A", None), (0x11, "B", None), (0x12, "A", None)],
    );
    for threads in [1, 4] {
        let mut pair = DxfPair::open(&input, threads);
        pair.apply_with_budgets(Edit::Remove {
            target: pair.seen(0x10),
        });
        assert_eq!(pair.live.journal.last_is_fast(), Some(true));
        pair.apply_with_budgets(Edit::Restyle {
            target: pair.seen(0x11),
            style: Restyle {
                layer: Some(pair.new_zero_layer()),
                ..Restyle::default()
            },
        });
        assert_eq!(pair.live.journal.last_is_fast(), Some(true));
        pair.assert_display_matches_reopen();
    }
}

fn implicit_entities_dxf(entities: &str) -> Vec<u8> {
    format!(
        "0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1015\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n{entities}0\nENDSEC\n0\nEOF\n"
    )
    .into_bytes()
}

#[test]
fn general_entity_spelling_variant_falls_back_when_head_changes() {
    // 一般の経路の図形（ELLIPSE）の綴りの違いも、先頭の参照が変わると構造経路に任せる
    let input = implicit_entities_dxf(
        "0\nLINE\n5\n10\n8\nFoo\n6\nDashed\n10\n0\n20\n0\n11\n10\n21\n5\n\
0\nELLIPSE\n5\n11\n8\nFOO\n6\nDASHED\n10\n0\n20\n0\n30\n0\n11\n5\n21\n0\n31\n0\n40\n0.5\n41\n0\n42\n6.283185307179586\n\
0\nLINE\n5\n12\n8\nFoo\n10\n0\n20\n0\n11\n10\n21\n5\n",
    );
    for threads in [1, 4] {
        let mut pair = DxfPair::open(&input, threads);
        pair.apply_implicit(
            vec![Edit::Remove {
                target: pair.seen(0x10),
            }],
            false,
        );
        let model = fresh_cx(|cx| pair.live.drawing(cx)).0.unwrap();
        assert_eq!(model.layers[0].name, "FOO");
        // 先頭の参照が変わらない変更は高速経路のまま
        pair.apply_implicit(
            vec![Edit::Remove {
                target: pair.seen(0x12),
            }],
            true,
        );
    }
}

#[test]
fn names_left_by_dropped_block_entities_keep_their_position() {
    // ブロック定義の LINE は画層（8）がなく持ち上げに失敗するが、線種 DASHED はその前に足される
    let blocks = "0\nBLOCK\n5\n20\n8\nBLK\n2\nB1\n70\n0\n10\n0\n20\n0\n30\n0\n3\nB1\n\
0\nLINE\n5\n21\n6\nDASHED\n10\n0\n20\n0\n11\n1\n21\n1\n0\nENDBLK\n5\n22\n8\nBLK\n";
    let input = implicit_dxf(
        blocks,
        &[
            (0x10, "A", Some("DASHED")),
            (0x11, "A", None),
            (0x12, "A", Some("CENTER")),
        ],
    );
    for threads in [1, 4] {
        let mut pair = DxfPair::open(&input, threads);
        // モデル空間の参照が残る間は先頭が分かる
        pair.apply_implicit(
            vec![Edit::Remove {
                target: pair.seen(0x12),
            }],
            true,
        );
        // 最後のモデル空間の参照が消えると、ブロック定義の中の位置は分からない
        pair.apply_implicit(
            vec![Edit::Remove {
                target: pair.seen(0x10),
            }],
            false,
        );
        let model = fresh_cx(|cx| pair.live.drawing(cx)).0.unwrap();
        assert!(model.line_types.iter().any(|lt| lt.name == "DASHED"));
    }
}

/// 古い形式の POLYLINE（VERTEX・SEQEND が画層・線種を持つ）を含む文書
fn polyline_line_types_dxf() -> Vec<u8> {
    implicit_entities_dxf(
        "0\nLINE\n5\n10\n8\nA\n10\n0\n20\n0\n11\n10\n21\n5\n\
0\nLINE\n5\n11\n8\nA\n6\nCENTER\n10\n1\n20\n0\n11\n10\n21\n5\n\
0\nLINE\n5\n12\n8\nA\n6\nDASHED\n10\n2\n20\n0\n11\n10\n21\n5\n\
0\nPOLYLINE\n5\n13\n8\nA\n66\n1\n70\n0\n10\n0\n20\n0\n30\n0\n\
0\nVERTEX\n5\n14\n8\nA\n6\nCENTER\n10\n0\n20\n0\n30\n0\n70\n0\n\
0\nVERTEX\n5\n15\n8\nA\n10\n1\n20\n1\n30\n0\n70\n0\n\
0\nSEQEND\n5\n16\n8\nA\n",
    )
}

#[test]
fn restyle_onto_existing_implicit_name_moves_its_head_earlier() {
    for threads in [1, 4] {
        let mut pair = DxfPair::open(&polyline_line_types_dxf(), threads);
        // VERTEX・SEQEND の名前は持ち上げで足さないので、並びを作り直せる
        assert!(pair.live.state.implicit.line_types.usable);
        pair.apply_implicit(
            vec![Edit::Restyle {
                target: pair.seen(0x10),
                style: Restyle {
                    line_type: Some(Inherit::Own(pair.line_type_id("DASHED"))),
                    ..Restyle::default()
                },
            }],
            true,
        );
        let model = fresh_cx(|cx| pair.live.drawing(cx)).0.unwrap();
        let names: Vec<_> = model.line_types.iter().map(|lt| lt.name.as_str()).collect();
        assert_eq!(names.first(), Some(&"DASHED"));
    }
}

#[test]
fn unusable_documents_only_append_references_from_added_elements() {
    for threads in [1, 4] {
        let mut pair = DxfPair::open(&polyline_line_types_dxf(), threads);
        // 並びを作り直せない文書として扱う
        pair.live.state.implicit.line_types.usable = false;
        pair.live.state.implicit.layers.usable = false;
        // 元の図形の位置に参照を足すと先頭が前へ動きうるので、構造経路に任せる
        pair.apply_implicit(
            vec![Edit::Restyle {
                target: pair.seen(0x10),
                style: Restyle {
                    line_type: Some(Inherit::Own(pair.line_type_id("DASHED"))),
                    ..Restyle::default()
                },
            }],
            false,
        );
        let mut pair = DxfPair::open(&polyline_line_types_dxf(), threads);
        pair.live.state.implicit.line_types.usable = false;
        pair.live.state.implicit.layers.usable = false;
        // 追加する図形から既存の名前を参照するだけなら高速経路
        let mut added = pair.element(0x11);
        added.style.line_type = Inherit::Own(pair.line_type_id("DASHED"));
        pair.apply_implicit(
            vec![Edit::Add {
                place: "space/0".to_owned(),
                element: Box::new(Placed::Model(added)),
            }],
            true,
        );
    }
}

#[test]
fn documents_without_implicit_names_skip_the_implicit_plan() {
    for threads in [1, 4] {
        let mut pair = DxfPair::open(&synthetic_rich_dxf(), threads);
        assert!(!pair.live.state.catalog.has_implicit);
        super::edit::PLAN_IMPLICIT_CALLS.with(|calls| calls.set(0));
        pair.apply_with_budgets(Edit::Transform {
            target: pair.seen(0x101),
            transform: Affine::translation(1.0, 2.0),
        });
        let layer = fresh_cx(|cx| pair.live.drawing(cx)).0.unwrap().layers[0].id;
        pair.apply_with_budgets(Edit::Restyle {
            target: pair.seen(0x101),
            style: Restyle {
                layer: Some(layer),
                line_type_scale: Some(2.0),
                ..Restyle::default()
            },
        });
        pair.apply_with_budgets(Edit::Remove {
            target: pair.seen(0x101),
        });
        assert_eq!(
            super::edit::PLAN_IMPLICIT_CALLS.with(core::cell::Cell::get),
            0
        );
        assert_eq!(pair.live.journal.last_is_fast(), Some(true));
    }
}

#[test]
fn individual_rows_match_the_retained_model_after_fast_removal() {
    for threads in [1, 4] {
        let mut pair = DxfPair::open(&synthetic_rich_dxf(), threads);
        pair.apply(Edit::Remove {
            target: pair.seen(0x101),
        });
        let drawing = fresh_cx(|cx| pair.live.drawing(cx)).0.unwrap();
        let insert = drawing.spaces[0]
            .elements
            .iter()
            .find(|element| element.source.key() == Some(Key::Handle(0x106)))
            .unwrap()
            .clone();
        pair.apply(Edit::Add {
            place: "space/0".to_owned(),
            element: Box::new(Placed::Model(insert)),
        });
    }
}

fn general_shapes_dxf() -> Vec<u8> {
    let mut source = String::from_utf8(synthetic_rich_dxf()).unwrap();
    let at = source.rfind("  0\nENDSEC\n").unwrap();
    let records = [
        ("POINT", "120", "AcDbPoint", "10\n1\n20\n2\n30\n0\n"),
        ("SOLID", "121", "AcDbTrace", "10\n0\n20\n0\n11\n10\n21\n0\n12\n0\n22\n10\n13\n10\n23\n10\n"),
        ("SPLINE", "122", "AcDbSpline", "70\n8\n71\n1\n72\n4\n73\n2\n74\n0\n40\n0\n40\n0\n40\n1\n40\n1\n10\n0\n20\n0\n30\n0\n10\n10\n20\n5\n30\n0\n"),
    ].into_iter().map(|(kind, handle, subclass, values)| {
        format!("0\n{kind}\n5\n{handle}\n330\n41\n100\nAcDbEntity\n8\n0\n100\n{subclass}\n{values}")
    }).collect::<String>();
    source.insert_str(at, &records);
    source.into_bytes()
}

#[test]
fn general_shapes_transform_and_replace_match_sequential_apply() {
    for (input, handles) in [
        (general_shapes_dxf(), vec![0x109, 0x120]),
        (synthetic_associative_dxf(), vec![0x103, 0x104, 0x105]),
    ] {
        for threads in [1, 4] {
            let mut pair = DxfPair::open(&input, threads);
            for handle in &handles {
                pair.apply(Edit::Transform {
                    target: pair.seen(*handle),
                    transform: Affine::translation(2.0, 3.0),
                });
                let model = fresh_cx(|cx| pair.reference.lift(cx)).0.unwrap();
                let mut element = model.spaces[0]
                    .elements
                    .iter()
                    .find(|element| element.source.key() == Some(Key::Handle(*handle)))
                    .unwrap()
                    .clone();
                element.shape =
                    transform::transform(&element.shape, &Affine::translation(-3.0, 4.0)).unwrap();
                pair.apply(Edit::Replace {
                    target: pair.seen(*handle),
                    element: Box::new(Placed::Model(element)),
                });
            }
        }
    }
}

#[test]
fn solid_and_hatch_styling_and_refused_moves_match_apply() {
    for (input, handle) in [
        (general_shapes_dxf(), 0x121),
        (synthetic_associative_dxf(), 0x102),
    ] {
        for threads in [1, 4] {
            let mut pair = DxfPair::open(&input, threads);
            pair.apply(Edit::Restyle {
                target: pair.seen(handle),
                style: Restyle {
                    line_type_scale: Some(2.0),
                    ..Restyle::default()
                },
            });
            pair.reject(Edit::Transform {
                target: pair.seen(handle),
                transform: Affine::translation(2.0, 3.0),
            });
            pair.apply(Edit::Remove {
                target: pair.seen(handle),
            });
        }
    }
}

#[test]
fn spline_replacement_and_refused_transform_match_apply() {
    for threads in [1, 4] {
        let mut pair = DxfPair::open(&general_shapes_dxf(), threads);
        pair.reject(Edit::Transform {
            target: pair.seen(0x122),
            transform: Affine::translation(2.0, 3.0),
        });
        let model = fresh_cx(|cx| pair.reference.lift(cx)).0.unwrap();
        let mut element = model.spaces[0]
            .elements
            .iter()
            .find(|element| element.source.key() == Some(Key::Handle(0x122)))
            .unwrap()
            .clone();
        let Shape::Curve(Edge::Spline(spline)) = &mut element.shape else {
            panic!("SPLINE must remain a spline");
        };
        for control in &mut spline.control {
            *control = Affine::translation(2.0, 3.0).point(*control);
        }
        pair.apply(Edit::Replace {
            target: pair.seen(0x122),
            element: Box::new(Placed::Model(element)),
        });
    }
}

#[test]
fn paper_and_block_elements_match_sequential_apply() {
    let mut input = String::from_utf8(synthetic_rich_dxf()).unwrap();
    let at = input.rfind("  0\nENDSEC\n").unwrap();
    input.insert_str(at, "0\nLINE\n5\n130\n330\n42\n100\nAcDbEntity\n67\n1\n8\n0\n100\nAcDbLine\n10\n1\n20\n2\n30\n0\n11\n3\n21\n4\n31\n0\n");
    for threads in [1, 4] {
        let mut pair = DxfPair::open(input.as_bytes(), threads);
        for handle in [0x130, 0x55] {
            pair.apply(Edit::Transform {
                target: pair.seen(handle),
                transform: Affine::translation(2.0, 3.0),
            });
            let model = fresh_cx(|cx| pair.reference.lift(cx)).0.unwrap();
            let affine = Affine::translation(-3.0, 4.0);
            let element = if handle == 0x130 {
                let mut element = model
                    .sheets
                    .iter()
                    .flat_map(|sheet| &sheet.elements)
                    .find(|element| element.source.key() == Some(Key::Handle(handle)))
                    .unwrap()
                    .clone();
                element.shape = transform::transform(&element.shape, &affine).unwrap();
                Placed::Paper(element)
            } else {
                let mut element = model
                    .symbols
                    .iter()
                    .flat_map(|symbol| &symbol.space.elements)
                    .find(|element| element.source.key() == Some(Key::Handle(handle)))
                    .unwrap()
                    .clone();
                element.shape = transform::transform(&element.shape, &affine).unwrap();
                Placed::Model(element)
            };
            pair.apply(Edit::Replace {
                target: pair.seen(handle),
                element: Box::new(element),
            });
            pair.apply(Edit::Remove {
                target: pair.seen(handle),
            });
        }
    }
}

fn check_layer_table_changes<C: Codec>(mut pair: AcadPair<C>)
where
    C::File: Lift<Drawing2D> + Apply<Changes, Output = Vec<Applied>>,
{
    let model = fresh_cx(|cx| pair.reference.lift(cx)).0.unwrap();
    let mut layer = model.layers[0].clone();
    layer.name = "LIVE_NEW".to_owned();
    pair.apply(Edit::AddLayer {
        layer: Box::new(layer),
    });
    let model = fresh_cx(|cx| pair.reference.lift(cx)).0.unwrap();
    let mut layer = model
        .layers
        .iter()
        .find(|layer| layer.name == "LIVE_NEW")
        .unwrap()
        .clone();
    let target = Seen {
        id: id::layer(&layer),
        hash: Hash::layer(&layer),
    };
    layer.name = "LIVE_RENAMED".to_owned();
    pair.apply(Edit::ReplaceLayer {
        target,
        layer: Box::new(layer),
    });
    let model = fresh_cx(|cx| pair.reference.lift(cx)).0.unwrap();
    let layer = model
        .layers
        .iter()
        .find(|layer| layer.name == "LIVE_RENAMED")
        .unwrap();
    pair.apply(Edit::RemoveLayer {
        target: Seen {
            id: id::layer(layer),
            hash: Hash::layer(layer),
        },
    });
}

#[test]
fn layer_table_changes_match_sequential_apply() {
    for threads in [1, 4] {
        check_layer_table_changes(DxfPair::open(&synthetic_rich_dxf(), threads));
    }
}

#[test]
fn dwg_layer_table_changes_match_sequential_apply() {
    for name in [
        "zumen-layout-r2000.dwg",
        "zumen-layout-r2004.dwg",
        "zumen-layout-r2018.dwg",
    ] {
        for threads in [1, 4] {
            check_layer_table_changes(DwgPair::open(&dwg_sample(name), threads));
        }
    }
}

fn check_text_style_table_changes<C: Codec>(mut pair: AcadPair<C>)
where
    C::File: Lift<Drawing2D> + Apply<Changes, Output = Vec<Applied>>,
{
    let model = fresh_cx(|cx| pair.reference.lift(cx)).0.unwrap();
    let mut style = model.text_styles[0].clone();
    style.name = "LIVE_NEW".to_owned();
    style.definition.as_mut().unwrap().font = Some(Font {
        name: "LIVE_TEST.shx".to_owned(),
        kind: FontKind::Shape,
    });
    pair.apply(Edit::AddTextStyle {
        style: Box::new(style),
    });
    let model = fresh_cx(|cx| pair.reference.lift(cx)).0.unwrap();
    let (index, style) = model
        .text_styles
        .iter()
        .enumerate()
        .find(|(_, style)| style.name == "LIVE_NEW")
        .unwrap();
    let target = Seen {
        id: id::text_style(style, index),
        hash: Hash::text_style(style),
    };
    let mut style = style.clone();
    style.name = "LIVE_RENAMED".to_owned();
    style.definition.as_mut().unwrap().width = 1.5;
    pair.apply(Edit::ReplaceTextStyle {
        target,
        style: Box::new(style),
    });
    let model = fresh_cx(|cx| pair.reference.lift(cx)).0.unwrap();
    let (index, style) = model
        .text_styles
        .iter()
        .enumerate()
        .find(|(_, style)| style.name == "LIVE_RENAMED")
        .unwrap();
    pair.apply(Edit::RemoveTextStyle {
        target: Seen {
            id: id::text_style(style, index),
            hash: Hash::text_style(style),
        },
    });
}

#[test]
fn text_style_table_changes_match_sequential_apply() {
    for threads in [1, 4] {
        check_text_style_table_changes(DxfPair::open(&synthetic_rich_dxf(), threads));
    }
}

#[test]
fn dwg_text_style_table_changes_match_sequential_apply() {
    for name in [
        "zumen-layout-r2000.dwg",
        "zumen-layout-r2004.dwg",
        "zumen-layout-r2018.dwg",
    ] {
        for threads in [1, 4] {
            let mut pair = DwgPair::open(&dwg_sample(name), threads);
            let model = fresh_cx(|cx| pair.reference.lift(cx)).0.unwrap();
            let mut named = model.text_styles[0].clone();
            assert!(matches!(
                named
                    .definition
                    .as_ref()
                    .unwrap()
                    .font
                    .as_ref()
                    .unwrap()
                    .kind,
                FontKind::Named { .. }
            ));
            named.name = "LIVE_NAMED".to_owned();
            pair.reject(Edit::AddTextStyle {
                style: Box::new(named),
            });
            check_text_style_table_changes(pair);
        }
    }
}

fn check_line_type_table_changes<C: Codec>(mut pair: AcadPair<C>)
where
    C::File: Lift<Drawing2D> + Apply<Changes, Output = Vec<Applied>>,
{
    let model = fresh_cx(|cx| pair.reference.lift(cx)).0.unwrap();
    let mut line_type = model
        .line_types
        .iter()
        .find(|line_type| line_type.definition.is_some())
        .unwrap()
        .clone();
    line_type.name = "LIVE_NEW".to_owned();
    pair.apply(Edit::AddLineType {
        line_type: Box::new(line_type),
    });
    let model = fresh_cx(|cx| pair.reference.lift(cx)).0.unwrap();
    let line_type = model
        .line_types
        .iter()
        .find(|line_type| line_type.name == "LIVE_NEW")
        .unwrap();
    let target = Seen {
        id: id::line_type(line_type),
        hash: Hash::line_type(line_type),
    };
    let mut line_type = line_type.clone();
    line_type.name = "LIVE_RENAMED".to_owned();
    pair.apply(Edit::ReplaceLineType {
        target,
        line_type: Box::new(line_type),
    });
    let model = fresh_cx(|cx| pair.reference.lift(cx)).0.unwrap();
    let line_type = model
        .line_types
        .iter()
        .find(|line_type| line_type.name == "LIVE_RENAMED")
        .unwrap();
    pair.apply(Edit::RemoveLineType {
        target: Seen {
            id: id::line_type(line_type),
            hash: Hash::line_type(line_type),
        },
    });
}

#[test]
fn line_type_table_changes_match_sequential_apply() {
    for threads in [1, 4] {
        check_line_type_table_changes(DxfPair::open(&synthetic_rich_dxf(), threads));
    }
}

#[test]
fn dwg_line_type_table_changes_match_sequential_apply() {
    for name in [
        "zumen-layout-r2000.dwg",
        "zumen-layout-r2004.dwg",
        "zumen-layout-r2018.dwg",
    ] {
        for threads in [1, 4] {
            check_line_type_table_changes(DwgPair::open(&dwg_sample(name), threads));
        }
    }
}

fn check_dimension_style_table_changes<C: Codec>(mut pair: AcadPair<C>)
where
    C::File: Lift<Drawing2D> + Apply<Changes, Output = Vec<Applied>>,
{
    let model = fresh_cx(|cx| pair.reference.lift(cx)).0.unwrap();
    let mut style = model.dimension_styles[0].clone();
    style.name = "LIVE_NEW".to_owned();
    pair.apply(Edit::AddDimensionStyle {
        style: Box::new(style),
    });
    let model = fresh_cx(|cx| pair.reference.lift(cx)).0.unwrap();
    let (index, style) = model
        .dimension_styles
        .iter()
        .enumerate()
        .find(|(_, style)| style.name == "LIVE_NEW")
        .unwrap();
    let target = Seen {
        id: id::dimension_style(style, index),
        hash: Hash::dimension_style(style),
    };
    let mut style = style.clone();
    style.name = "LIVE_RENAMED".to_owned();
    style.size = 2.5;
    pair.apply(Edit::ReplaceDimensionStyle {
        target,
        style: Box::new(style),
    });
    let model = fresh_cx(|cx| pair.reference.lift(cx)).0.unwrap();
    let (index, style) = model
        .dimension_styles
        .iter()
        .enumerate()
        .find(|(_, style)| style.name == "LIVE_RENAMED")
        .unwrap();
    pair.apply(Edit::RemoveDimensionStyle {
        target: Seen {
            id: id::dimension_style(style, index),
            hash: Hash::dimension_style(style),
        },
    });
}

#[test]
fn dimension_style_table_changes_match_sequential_apply() {
    for threads in [1, 4] {
        check_dimension_style_table_changes(DxfPair::open(&synthetic_associative_dxf(), threads));
    }
}

#[test]
fn dwg_dimension_style_table_changes_match_sequential_apply() {
    for name in [
        "zumen-layout-r2000.dwg",
        "zumen-layout-r2004.dwg",
        "zumen-layout-r2018.dwg",
    ] {
        for threads in [1, 4] {
            check_dimension_style_table_changes(DwgPair::open(&dwg_sample(name), threads));
        }
    }
}

#[test]
fn dimension_restyle_preserves_external_dependencies_through_undo() {
    fn assert_dependencies(live: &AcadLive) {
        let line = live.table().find_handle(0x101).unwrap();
        let dimension = live.table().find_handle(0x105).unwrap();
        let affected = fresh_cx(|cx| live.affected_rows(&[line], cx)).0.unwrap();
        assert!(
            affected.contains(&dimension),
            "DIMASSOC dependency disappeared"
        );
    }
    for threads in [1, 4] {
        let mut pair = DxfPair::open(&synthetic_associative_dxf(), threads);
        assert_dependencies(&pair.live);
        pair.apply(Edit::Restyle {
            target: pair.seen(0x105),
            style: Restyle {
                line_type_scale: Some(2.0),
                ..Restyle::default()
            },
        });
        assert_dependencies(&pair.live);
        assert!(fresh_cx(|cx| pair.live.undo(cx)).0.unwrap());
        assert_dependencies(&pair.live);
        assert!(fresh_cx(|cx| pair.live.redo(cx)).0.unwrap());
        assert_dependencies(&pair.live);
        pair.apply(Edit::Transform {
            target: pair.seen(0x101),
            transform: Affine::translation(2.0, 3.0),
        });
        assert_dependencies(&pair.live);
    }
}

fn reactor_only_leader_dxf() -> Vec<u8> {
    let original = String::from_utf8(synthetic_associative_dxf()).unwrap();
    assert_eq!(original.matches("340\n103\n").count(), 1);
    original.replace("340\n103\n", "").into_bytes()
}

#[test]
fn reactor_dependency_survives_annotation_free_leader_edits() {
    let input = reactor_only_leader_dxf();
    for threads in [1, 4] {
        let mut pair = DxfPair::open(&input, threads);
        let source = pair.live.table().find_handle(0x103).unwrap();
        let dependent = pair.live.table().find_handle(0x104).unwrap();
        assert!(
            pair.live
                .dep_graph()
                .sources_of(dependent)
                .contains(&source)
        );
        pair.apply(Edit::Restyle {
            target: pair.seen(0x104),
            style: Restyle {
                line_type_scale: Some(2.0),
                ..Restyle::default()
            },
        });
        assert!(fresh_cx(|cx| pair.live.undo(cx)).0.unwrap());
        assert!(
            pair.live
                .dep_graph()
                .sources_of(dependent)
                .contains(&source)
        );
        assert!(fresh_cx(|cx| pair.live.redo(cx)).0.unwrap());
        assert!(
            pair.live
                .dep_graph()
                .sources_of(dependent)
                .contains(&source)
        );
        pair.apply(Edit::Transform {
            target: pair.seen(0x104),
            transform: Affine::translation(2.0, 3.0),
        });
        assert!(
            pair.live
                .dep_graph()
                .sources_of(dependent)
                .contains(&source)
        );
        pair.reject(Edit::Remove {
            target: pair.seen(0x103),
        });
        assert!(
            pair.live
                .dep_graph()
                .sources_of(dependent)
                .contains(&source)
        );
    }
}

#[test]
fn stopped_external_dependency_edits_preserve_state_and_history() {
    let pair = DxfPair::open(&reactor_only_leader_dxf(), 1);
    let changes = Changes {
        document: None,
        edits: vec![Edit::Restyle {
            target: pair.seen(0x104),
            style: Restyle {
                line_type_scale: Some(2.0),
                ..Restyle::default()
            },
        }],
    };
    let before = fresh_cx(|cx| pair.live.drawing(cx)).0.unwrap();
    let before_bytes = fresh_cx(|cx| pair.live.save(cx)).0.unwrap();
    let dependent = pair.live.table().find_handle(0x104).unwrap();
    let sources = pair.live.dep_graph().sources_of(dependent);
    let mut measured = pair.live.clone();
    let mut full_budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    measured
        .apply(&changes, &mut Cx::new(&mut full_budget, &mut diag, &cancel))
        .unwrap();
    for kind in [Kind::Work, Kind::DecodedBytes] {
        let total = full_budget.used(kind);
        let mut succeeded = false;
        for limit in (0..=total.min(2048)).chain([total / 2, total.saturating_sub(1), total]) {
            let mut live = pair.live.clone();
            let mut budget = Budget::new(Limits::default().with(kind, Limit::Finite(limit)));
            let mut diag = Diagnostics::new();
            let cancel = CancelToken::new();
            match live.apply(&changes, &mut Cx::new(&mut budget, &mut diag, &cancel)) {
                Ok(_) => {
                    assert_eq!(live.dep_graph().sources_of(dependent), sources);
                    succeeded = true;
                    break;
                }
                Err(Error::Stopped(_)) => {
                    assert_eq!(fresh_cx(|cx| live.drawing(cx)).0.unwrap(), before);
                    assert_eq!(fresh_cx(|cx| live.save(cx)).0.unwrap(), before_bytes);
                    assert_eq!(live.dep_graph().sources_of(dependent), sources);
                    assert_eq!(live.state_id(), pair.live.state_id());
                    assert_eq!(live.undo_len(), 0);
                }
                other => panic!("unexpected edit result: {other:?}"),
            }
        }
        assert!(succeeded, "no successful edit for {kind:?}");
    }
}

fn dwg_sample(name: &str) -> Vec<u8> {
    std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../samples/dwg")
            .join(name),
    )
    .unwrap()
}

#[test]
fn explicit_copy_place_matches_batch_and_undoes_both_copies_together() {
    fn check<C: Codec>(
        bytes: &[u8],
        batch: impl Fn(
            &mut C::File,
            &Changes,
            &[crate::CopyFrom<'_, C::File>],
            &mut Cx,
        ) -> Result<Vec<Applied>, Error>,
    ) where
        C::File: Lift<Drawing2D> + Apply<Changes, Output = Vec<Applied>>,
    {
        let mut pair = AcadPair::<C>::open(bytes, 1);
        let before = fresh_cx(|cx| pair.live.drawing(cx)).0.unwrap();
        let saved = fresh_cx(|cx| pair.live.save(cx)).0.unwrap();
        let destination = "sheet/h2DC";
        let edits = [0x2D9, 0x2DA].map(|handle| Edit::Copy {
            target: pair.seen(handle),
            transform: Affine::translation(2.0, 3.0),
        });
        let changes = Changes {
            edits: edits.into(),
            document: None,
        };
        let mut stopped_budget = Budget::new(Limits::default().with(Kind::Work, Limit::Finite(0)));
        let stopped = pair.live.apply_with_copy_place(
            &changes,
            destination,
            &mut Cx::new(
                &mut stopped_budget,
                &mut Diagnostics::new(),
                &CancelToken::new(),
            ),
        );
        assert!(matches!(stopped, Err(Error::Stopped(_))));
        assert_eq!(pair.live.undo_len(), 0);
        assert_eq!(fresh_cx(|cx| pair.live.save(cx)).0.unwrap(), saved);
        for place in ["sheet/missing", destination] {
            let mut rejected = changes.clone();
            if place == destination
                && let Edit::Copy { target, .. } = &mut rejected.edits[1]
            {
                target.id = "space/0/hFFFFFFFF".to_owned();
            }
            assert!(
                fresh_cx(|cx| pair.live.apply_with_copy_place(&rejected, place, cx))
                    .0
                    .is_err()
            );
            assert_eq!(fresh_cx(|cx| pair.live.save(cx)).0.unwrap(), saved);
            assert_eq!(pair.live.undo_len(), 0);
        }
        let specs = [0, 1].map(|_| crate::CopyFrom {
            place: Some(destination),
            from: None,
        });
        let expected = fresh_cx(|cx| batch(&mut pair.reference, &changes, &specs, cx));
        let actual = fresh_cx(|cx| pair.live.apply_with_copy_place(&changes, destination, cx));
        assert_eq!(actual, expected);
        let applied = actual.0.unwrap();
        assert_eq!(applied.len(), 2);
        assert!(applied.iter().all(|a| {
            a.id.as_deref()
                .is_some_and(|id| id.starts_with("sheet/h2DC/"))
                && a.hash.is_some()
        }));
        pair.assert_model();
        let after = fresh_cx(|cx| pair.live.drawing(cx)).0.unwrap();
        let model_ids = |drawing: &Drawing2D| {
            all_element_ids_and_hashes(drawing)
                .into_iter()
                .filter(|(id, _)| id.starts_with("space/0/"))
                .collect::<Vec<_>>()
        };
        assert_eq!(model_ids(&before), model_ids(&after));
        let bytes = fresh_cx(|cx| pair.live.save(cx)).0.unwrap();
        assert_eq!(
            zumen_sdk::testkit::write::<C>(&pair.reference).unwrap(),
            bytes
        );
        assert_eq!(pair.live.undo_len(), 1);
        assert!(fresh_cx(|cx| pair.live.undo(cx)).0.unwrap());
        assert_eq!(fresh_cx(|cx| pair.live.save(cx)).0.unwrap(), saved);
        assert!(fresh_cx(|cx| pair.live.redo(cx)).0.unwrap());
        assert_eq!(fresh_cx(|cx| pair.live.save(cx)).0.unwrap(), bytes);
    }
    check::<Dxf>(
        &dwg_sample("zumen-layout-r2018_ascii.dxf"),
        crate::DxfFile::apply_with_copies,
    );
    check::<Dwg>(
        &dwg_sample("zumen-layout-r2018.dwg"),
        crate::DwgFile::apply_with_copies,
    );
}

#[test]
fn dwg_public_samples_retain_full_models() {
    for name in [
        "zumen-layout-r2000.dwg",
        "zumen-layout-r2018.dwg",
        "zumen-points-a-r2000.dwg",
        "zumen-points-a-r2018.dwg",
        "zumen-arcdim-r2018.dwg",
    ] {
        let bytes = dwg_sample(name);
        for threads in [1, 4] {
            DwgPair::open(&bytes, threads);
        }
    }
}

#[test]
fn dwg_added_line_removal_does_not_reread_the_document() {
    for name in [
        "zumen-layout-r2000.dwg",
        "zumen-layout-r2004.dwg",
        "zumen-layout-r2018.dwg",
    ] {
        for threads in [1, 4] {
            let mut pair = DwgPair::open(&dwg_sample(name), threads);
            let model = fresh_cx(|cx| pair.reference.lift(cx)).0.unwrap();
            let mut element = model.spaces[0].elements[0].clone();
            let point = |x, y| zumen_drawing::ModelPoint {
                space: model.spaces[0].id,
                x,
                y,
            };
            element.shape = Shape::Line {
                start: point(0.0, 0.0),
                end: point(10.0, 20.0),
            };
            let added = pair
                .apply(Edit::Add {
                    place: "space/0".to_owned(),
                    element: Box::new(Placed::Model(element)),
                })
                .pop()
                .unwrap();
            let remove = Edit::Remove {
                target: Seen {
                    id: added.id.unwrap(),
                    hash: added.hash.unwrap(),
                },
            };
            let mut live = pair.live.clone();
            let mut budget =
                Budget::new(Limits::default().with(Kind::InputBytes, Limit::Finite(0)));
            let mut diagnostics = Diagnostics::new();
            let cancel = CancelToken::new();
            let actual = live
                .apply(
                    &Changes {
                        document: None,
                        edits: vec![remove.clone()],
                    },
                    &mut Cx::new(&mut budget, &mut diagnostics, &cancel),
                )
                .unwrap();
            assert_eq!(budget.used(Kind::InputBytes), 0);
            assert_eq!(actual, pair.apply(remove));
            assert_eq!(
                fresh_cx(|cx| live.drawing(cx)).0.unwrap(),
                fresh_cx(|cx| pair.live.drawing(cx)).0.unwrap()
            );
            assert_eq!(
                fresh_cx(|cx| live.save(cx)).0.unwrap(),
                fresh_cx(|cx| pair.live.save(cx)).0.unwrap()
            );
        }
    }
}

#[test]
fn dwg_general_shape_edits_match_sequential_apply() {
    let mut added = Vec::new();
    for (bytes, handles) in [
        (general_shapes_dxf(), vec![0x109, 0x120, 0x122]),
        (synthetic_associative_dxf(), vec![0x103, 0x104]),
    ] {
        let model: Drawing2D = fresh_cx(|cx| Dxf::read(&bytes, cx).and_then(|file| file.lift(cx)))
            .0
            .unwrap();
        for handle in handles {
            let mut element = model.spaces[0]
                .elements
                .iter()
                .find(|element| element.source.key() == Some(Key::Handle(handle)))
                .unwrap()
                .clone();
            if let Shape::Leader(leader) = &mut element.shape {
                leader.annotation = None;
                let mut explicit_leader = leader.clone();
                explicit_leader.arrow = Some(true);
                let mut explicit = element.clone();
                explicit.shape = Shape::Leader(explicit_leader);
                added.push(explicit);
            }
            added.push(element);
        }
    }
    assert_eq!(added.len(), 6);
    for name in [
        "zumen-layout-r2000.dwg",
        "zumen-layout-r2004.dwg",
        "zumen-layout-r2018.dwg",
    ] {
        let bytes = dwg_sample(name);
        for threads in [1, 4] {
            let mut pair = DwgPair::open(&bytes, threads);
            let model = fresh_cx(|cx| pair.reference.lift(cx)).0.unwrap();
            let style = model.spaces[0].elements[0].style;
            let layer = model.layers[0].id;
            for original in &added {
                let mut element = original.clone();
                element.layer = layer;
                element.style = style;
                if let Shape::Leader(leader) = &mut element.shape {
                    leader.style = model.dimension_styles[0].name.clone();
                }
                let edit = Edit::Add {
                    place: "space/0".to_owned(),
                    element: Box::new(Placed::Model(element)),
                };
                if matches!(&original.shape, Shape::Leader(leader) if leader.arrow.is_none()) {
                    pair.reject(edit);
                } else {
                    pair.apply(edit);
                    let model = fresh_cx(|cx| pair.reference.lift(cx)).0.unwrap();
                    let Some(Key::Handle(handle)) =
                        model.spaces[0].elements.last().unwrap().source.key()
                    else {
                        panic!("handle required");
                    };
                    pair.apply(Edit::Restyle {
                        target: pair.seen(handle),
                        style: Restyle {
                            line_type_scale: Some(2.0),
                            ..Restyle::default()
                        },
                    });
                    let edit = Edit::Transform {
                        target: pair.seen(handle),
                        transform: Affine::translation(2.0, 3.0),
                    };
                    if matches!(original.shape, Shape::Curve(Edge::Spline(_))) {
                        pair.reject(edit);
                    } else {
                        pair.apply(edit);
                    }
                    let model = fresh_cx(|cx| pair.reference.lift(cx)).0.unwrap();
                    let mut replacement = model.spaces[0].elements.last().unwrap().clone();
                    let affine = Affine::translation(-3.0, 4.0);
                    if let Shape::Curve(Edge::Spline(spline)) = &mut replacement.shape {
                        for control in &mut spline.control {
                            *control = affine.point(*control);
                        }
                    } else {
                        replacement.shape =
                            transform::transform(&replacement.shape, &affine).unwrap();
                    }
                    pair.apply(Edit::Replace {
                        target: pair.seen(handle),
                        element: Box::new(Placed::Model(replacement)),
                    });
                    let edit = Edit::Remove {
                        target: pair.seen(handle),
                    };
                    if matches!(original.shape, Shape::Leader(_)) {
                        pair.reject(edit);
                    } else {
                        pair.apply(edit);
                    }
                }
            }
        }
    }
}

#[test]
fn dwg_source_positions_follow_repeated_edits_and_removals() {
    for name in [
        "zumen-layout-r2000.dwg",
        "zumen-layout-r2004.dwg",
        "zumen-layout-r2018.dwg",
    ] {
        for threads in [1, 4] {
            let mut pair = DwgPair::open(&dwg_sample(name), threads);
            let drawing = fresh_cx(|cx| pair.reference.lift(cx)).0.unwrap();
            let original = drawing.spaces[0]
                .elements
                .iter()
                .find(|element| matches!(element.shape, Shape::Line { .. } | Shape::Circle { .. }))
                .unwrap()
                .clone();
            pair.apply(Edit::Add {
                place: "space/0".to_owned(),
                element: Box::new(Placed::Model(original)),
            });
            pair.apply(Edit::SetPointStyle {
                style: zumen_drawing::PointStyle {
                    mark: zumen_drawing::PointMark::Cross,
                    circle: true,
                    square: false,
                    size: zumen_drawing::PointSize::Length(2.5),
                },
            });
            let drawing = fresh_cx(|cx| pair.reference.lift(cx)).0.unwrap();
            let handles: Vec<_> = drawing.spaces[0]
                .elements
                .iter()
                .filter(|element| {
                    matches!(element.shape, Shape::Line { .. } | Shape::Circle { .. })
                })
                .filter_map(|element| match element.source.key() {
                    Some(Key::Handle(h)) => Some(h),
                    _ => None,
                })
                .take(2)
                .collect();
            assert_eq!(handles.len(), 2);
            for handle in &handles {
                pair.apply(Edit::Transform {
                    target: pair.seen(*handle),
                    transform: Affine::translation(2.0, 3.0),
                });
                pair.apply(Edit::Restyle {
                    target: pair.seen(*handle),
                    style: Restyle {
                        line_type_scale: Some(2.0),
                        ..Restyle::default()
                    },
                });
                let current = fresh_cx(|cx| pair.reference.lift(cx)).0.unwrap();
                let mut replacement = current.spaces[0]
                    .elements
                    .iter()
                    .find(|element| element.source.key() == Some(Key::Handle(*handle)))
                    .unwrap()
                    .clone();
                replacement.shape =
                    transform::transform(&replacement.shape, &Affine::translation(-3.0, 4.0))
                        .unwrap();
                pair.apply(Edit::Replace {
                    target: pair.seen(*handle),
                    element: Box::new(Placed::Model(replacement)),
                });
            }
            for handle in handles {
                pair.apply(Edit::Remove {
                    target: pair.seen(handle),
                });
            }
        }
    }
}

#[test]
fn dwg_paper_edits_match_sequential_apply() {
    for name in [
        "zumen-layout-r2000.dwg",
        "zumen-layout-r2004.dwg",
        "zumen-layout-r2018.dwg",
    ] {
        for threads in [1, 4] {
            let mut pair = DwgPair::open(&dwg_sample(name), threads);
            let model = fresh_cx(|cx| pair.reference.lift(cx)).0.unwrap();
            let sheet = &model.sheets[0];
            let template = &model.spaces[0].elements[0];
            let point = |x, y| PaperPoint {
                sheet: sheet.id,
                x,
                y,
            };
            pair.apply(Edit::Add {
                place: id::sheet(sheet),
                element: Box::new(Placed::Paper(Element {
                    source: Default::default(),
                    layer: template.layer,
                    style: template.style,
                    shape: Shape::Line {
                        start: point(1.0, 2.0),
                        end: point(3.0, 4.0),
                    },
                })),
            });
            let model = fresh_cx(|cx| pair.reference.lift(cx)).0.unwrap();
            let element = model.sheets[0].elements.last().unwrap();
            let Some(Key::Handle(handle)) = element.source.key() else {
                panic!("handle required")
            };
            pair.apply(Edit::Transform {
                target: pair.seen(handle),
                transform: Affine::translation(2.0, 3.0),
            });
            let model = fresh_cx(|cx| pair.reference.lift(cx)).0.unwrap();
            let mut element = model.sheets[0].elements.last().unwrap().clone();
            element.shape =
                transform::transform(&element.shape, &Affine::translation(-3.0, 4.0)).unwrap();
            pair.apply(Edit::Replace {
                target: pair.seen(handle),
                element: Box::new(Placed::Paper(element)),
            });
            pair.apply(Edit::Restyle {
                target: pair.seen(handle),
                style: Restyle {
                    line_type_scale: Some(2.0),
                    ..Restyle::default()
                },
            });
            pair.apply(Edit::Remove {
                target: pair.seen(handle),
            });
        }
    }
}

#[test]
fn dwg_hatch_and_solid_edits_match_sequential_apply() {
    let model: Drawing2D =
        fresh_cx(|cx| Dxf::read(&synthetic_associative_dxf(), cx).and_then(|file| file.lift(cx)))
            .0
            .unwrap();
    let hatch = model.spaces[0]
        .elements
        .iter()
        .find(|element| element.source.key() == Some(Key::Handle(0x102)))
        .unwrap();
    for name in ["zumen-layout-r2000.dwg", "zumen-layout-r2018.dwg"] {
        for threads in [1, 4] {
            let mut pair = DwgPair::open(&dwg_sample(name), threads);
            let model = fresh_cx(|cx| pair.reference.lift(cx)).0.unwrap();
            let mut added = hatch.clone();
            added.layer = model.layers[0].id;
            added.style = model.spaces[0].elements[0].style;
            pair.reject(Edit::Add {
                place: "space/0".to_owned(),
                element: Box::new(Placed::Model(added.clone())),
            });
            let Shape::Hatch(shape) = &mut added.shape else {
                panic!("HATCH required")
            };
            shape.associative = false;
            for boundary in &mut shape.loops {
                boundary.sources.clear();
            }
            pair.apply(Edit::Add {
                place: "space/0".to_owned(),
                element: Box::new(Placed::Model(added)),
            });
            let model = fresh_cx(|cx| pair.reference.lift(cx)).0.unwrap();
            let Some(Key::Handle(handle)) = model.spaces[0].elements.last().unwrap().source.key()
            else {
                panic!("handle required")
            };
            pair.apply(Edit::Restyle {
                target: pair.seen(handle),
                style: Restyle {
                    line_type_scale: Some(2.0),
                    ..Restyle::default()
                },
            });
            pair.reject(Edit::Transform {
                target: pair.seen(handle),
                transform: Affine::translation(2.0, 3.0),
            });
            pair.reject_with_budgets(Edit::Remove {
                target: pair.seen(handle),
            });
        }
    }
    for name in ["zumen-arcdim-r2004.dwg", "zumen-arcdim-r2018.dwg"] {
        for threads in [1, 4] {
            let mut pair = DwgPair::open(&dwg_sample(name), threads);
            let db = fresh_cx(|cx| pair.reference.objects(cx)).0.unwrap();
            let handle = db
                .objects()
                .iter()
                .find(|object| object.kind == b"SOLID")
                .unwrap()
                .handle()
                .unwrap();
            pair.apply(Edit::Restyle {
                target: pair.seen(handle),
                style: Restyle {
                    line_type_scale: Some(2.0),
                    ..Restyle::default()
                },
            });
            pair.reject(Edit::Transform {
                target: pair.seen(handle),
                transform: Affine::translation(2.0, 3.0),
            });
            pair.apply(Edit::Remove {
                target: pair.seen(handle),
            });
        }
    }
}

#[test]
fn dwg_dimension_edits_match_sequential_apply() {
    for name in ["zumen-arcdim-r2004.dwg", "zumen-arcdim-r2018.dwg"] {
        for threads in [1, 4] {
            let mut pair = DwgPair::open(&dwg_sample(name), threads);
            let model = fresh_cx(|cx| pair.reference.lift(cx)).0.unwrap();
            let element = model.spaces[0]
                .elements
                .iter()
                .find(|element| matches!(element.shape, Shape::Dimension(_)))
                .unwrap();
            let Some(Key::Handle(handle)) = element.source.key() else {
                panic!("handle required")
            };
            pair.reject(Edit::Transform {
                target: pair.seen(handle),
                transform: Affine {
                    m: [2.0, 0.0, 0.0, 0.0, 1.0, 0.0],
                },
            });
            pair.apply(Edit::Restyle {
                target: pair.seen(handle),
                style: Restyle {
                    line_type_scale: Some(2.0),
                    ..Restyle::default()
                },
            });
            pair.apply(Edit::Transform {
                target: pair.seen(handle),
                transform: Affine::translation(2.0, 3.0),
            });
            let model = fresh_cx(|cx| pair.reference.lift(cx)).0.unwrap();
            let mut replacement = model.spaces[0]
                .elements
                .iter()
                .find(|element| element.source.key() == Some(Key::Handle(handle)))
                .unwrap()
                .clone();
            replacement.shape =
                transform::transform(&replacement.shape, &Affine::translation(-3.0, 4.0)).unwrap();
            pair.apply(Edit::Replace {
                target: pair.seen(handle),
                element: Box::new(Placed::Model(replacement)),
            });
            pair.reject(Edit::Remove {
                target: pair.seen(handle),
            });
        }
    }
}

#[test]
fn dwg_block_definition_edits_match_sequential_apply() {
    for name in [
        "zumen-minsert-r2000.dwg",
        "zumen-minsert-r2004.dwg",
        "zumen-minsert-r2018.dwg",
    ] {
        for threads in [1, 4] {
            let mut pair = DwgPair::open(&dwg_sample(name), threads);
            let model = fresh_cx(|cx| pair.reference.lift(cx)).0.unwrap();
            let element = model
                .symbols
                .iter()
                .flat_map(|symbol| &symbol.space.elements)
                .find(|element| matches!(element.shape, Shape::Polyline(_)))
                .unwrap();
            let Some(Key::Handle(handle)) = element.source.key() else {
                panic!("handle required")
            };
            pair.apply(Edit::Transform {
                target: pair.seen(handle),
                transform: Affine::translation(2.0, 3.0),
            });
            let model = fresh_cx(|cx| pair.reference.lift(cx)).0.unwrap();
            let mut element = model
                .symbols
                .iter()
                .flat_map(|symbol| &symbol.space.elements)
                .find(|element| element.source.key() == Some(Key::Handle(handle)))
                .unwrap()
                .clone();
            element.shape =
                transform::transform(&element.shape, &Affine::translation(-3.0, 4.0)).unwrap();
            pair.apply(Edit::Replace {
                target: pair.seen(handle),
                element: Box::new(Placed::Model(element)),
            });
            pair.apply(Edit::Restyle {
                target: pair.seen(handle),
                style: Restyle {
                    line_type_scale: Some(2.0),
                    ..Restyle::default()
                },
            });
            pair.apply(Edit::Remove {
                target: pair.seen(handle),
            });
        }
    }
}

#[test]
fn dwg_source_queries_share_budget_and_preserve_history() {
    for name in ["zumen-layout-r2000.dwg", "zumen-layout-r2018.dwg"] {
        let mut pair = DwgPair::open(&dwg_sample(name), 1);
        let row = RowId::from_index(0).unwrap();
        let Some(Key::Handle(handle)) = pair.live.element_of_row(row).unwrap().source.key() else {
            panic!("handle required");
        };
        pair.apply(Edit::Transform {
            target: pair.seen(handle),
            transform: Affine::translation(2.0, 3.0),
        });
        let before = fresh_cx(|cx| pair.live.drawing(cx)).0.unwrap();
        let bytes = fresh_cx(|cx| pair.live.save(cx)).0.unwrap();
        let history = (
            pair.live.state_id(),
            pair.live.undo_len(),
            pair.live.redo_len(),
        );
        for whole in [false, true] {
            let query = |cx: &mut Cx<'_>| {
                if whole {
                    pair.live.drawing(cx).map(|_| ())
                } else {
                    pair.live.element_with_source(row, cx).map(|_| ())
                }
            };
            let mut budget = Budget::default();
            let mut diagnostics = Diagnostics::new();
            let cancel = CancelToken::new();
            query(&mut Cx::new(&mut budget, &mut diagnostics, &cancel)).unwrap();
            for kind in [Kind::Work, Kind::DecodedBytes] {
                let used = budget.used(kind);
                assert!(used > 0);
                for remaining in [0, used / 2, used - 1, used] {
                    let mut limited =
                        Budget::new(Limits::default().with(kind, Limit::Finite(remaining + 7)));
                    limited.spend(kind, 7).unwrap();
                    let mut diagnostics = Diagnostics::new();
                    let result = query(&mut Cx::new(&mut limited, &mut diagnostics, &cancel));
                    assert_eq!(
                        result.is_ok(),
                        remaining == used,
                        "{name}: {kind:?} {remaining}/{used}"
                    );
                    if remaining != used {
                        assert!(matches!(result, Err(Error::Stopped(_))));
                    }
                }
            }
            cancel.cancel();
            let mut budget = Budget::default();
            let mut diagnostics = Diagnostics::new();
            assert!(matches!(
                query(&mut Cx::new(&mut budget, &mut diagnostics, &cancel)),
                Err(Error::Stopped(_))
            ));
        }
        assert_eq!(
            (
                pair.live.state_id(),
                pair.live.undo_len(),
                pair.live.redo_len()
            ),
            history
        );
        assert_eq!(fresh_cx(|cx| pair.live.drawing(cx)).0.unwrap(), before);
        assert_eq!(fresh_cx(|cx| pair.live.save(cx)).0.unwrap(), bytes);
    }
}

#[test]
fn source_query_counts_other_dxf_insertions_and_removals() {
    let mut pair = DxfPair::open(&synthetic_rich_dxf(), 1);
    let row = pair.live.table().find_handle(0x101).unwrap();
    let element = pair.live.element_of_row(row).unwrap();
    let mut base_budget = Budget::default();
    let mut diagnostics = Diagnostics::new();
    let cancel = CancelToken::new();
    pair.live
        .element_with_source(
            row,
            &mut Cx::new(&mut base_budget, &mut diagnostics, &cancel),
        )
        .unwrap();
    let mut edits: Vec<_> = (0..64)
        .map(|_| Edit::Add {
            place: "space/0".to_owned(),
            element: Box::new(Placed::Model(element.clone())),
        })
        .collect();
    edits.push(Edit::Remove {
        target: pair.seen(0x102),
    });
    fresh_cx(|cx| {
        pair.live.apply(
            &Changes {
                document: None,
                edits,
            },
            cx,
        )
    })
    .0
    .unwrap();
    let before = fresh_cx(|cx| pair.live.drawing(cx)).0.unwrap();
    let mut full_budget = Budget::default();
    let mut diagnostics = Diagnostics::new();
    let actual = pair
        .live
        .element_with_source(
            row,
            &mut Cx::new(&mut full_budget, &mut diagnostics, &cancel),
        )
        .unwrap()
        .unwrap();
    assert_eq!(
        &actual,
        before.spaces[0]
            .elements
            .iter()
            .find(|element| element.source.key() == Some(Key::Handle(0x101)))
            .unwrap()
    );
    for kind in [Kind::Work, Kind::DecodedBytes] {
        assert!(full_budget.used(kind) > base_budget.used(kind));
        for available in [base_budget.used(kind), full_budget.used(kind) - 1] {
            let mut limited = Budget::new(Limits::default().with(kind, Limit::Finite(available)));
            let mut diagnostics = Diagnostics::new();
            assert!(matches!(
                pair.live.element_with_source(
                    row,
                    &mut Cx::new(&mut limited, &mut diagnostics, &cancel)
                ),
                Err(Error::Stopped(_))
            ));
        }
    }
    assert_eq!(fresh_cx(|cx| pair.live.drawing(cx)).0.unwrap(), before);
}

#[test]
fn fast_entry_above_structural_entry_survives_undo_and_redo_of_both() {
    let lines = [(0x10, "A", None), (0x11, "B", None)];
    for threads in [1, 4] {
        let mut pair = DxfPair::open(&implicit_dxf(BLOCK_ONLY_LAYER, &lines), threads);
        let mut element = pair.element(0x11);
        element.layer = pair.new_zero_layer();
        pair.apply_edits(vec![Edit::Add {
            place: "space/0".to_owned(),
            element: Box::new(Placed::Model(element)),
        }]);
        assert_eq!(pair.live.journal.last_is_fast(), Some(false));
        let mut element = pair.element(0x10);
        element.layer = pair.layer_id("B");
        pair.apply_edits(vec![Edit::Add {
            place: "space/0".to_owned(),
            element: Box::new(Placed::Model(element)),
        }]);
        assert_eq!(pair.live.journal.last_is_fast(), Some(true));
        let top = fresh_cx(|cx| pair.live.drawing(cx)).0.unwrap();
        let top_bytes = fresh_cx(|cx| pair.live.save(cx)).0.unwrap();
        for _ in 0..2 {
            assert!(fresh_cx(|cx| pair.live.undo(cx)).0.unwrap());
        }
        for _ in 0..2 {
            assert!(fresh_cx(|cx| pair.live.redo(cx)).0.unwrap());
        }
        assert_eq!(fresh_cx(|cx| pair.live.drawing(cx)).0.unwrap(), top);
        assert_eq!(fresh_cx(|cx| pair.live.save(cx)).0.unwrap(), top_bytes);
        pair.assert_model();
        pair.assert_display_matches_reopen();
    }
}

/// 構造経路の記録（画層の追加）の上に、線分を動かす高速な記録を積み、両方を取り消してやり直す
fn check_fast_entry_above_layer_table_change<C: Codec>(mut pair: AcadPair<C>)
where
    C::File: Lift<Drawing2D> + Apply<Changes, Output = Vec<Applied>>,
{
    let model = fresh_cx(|cx| pair.reference.lift(cx)).0.unwrap();
    let mut layer = model.layers[0].clone();
    layer.name = "LIVE_NEW".to_owned();
    pair.apply(Edit::AddLayer {
        layer: Box::new(layer),
    });
    assert_eq!(pair.live.journal.last_is_fast(), Some(false));
    let handle = model.spaces[0]
        .elements
        .iter()
        .find_map(|element| match (&element.shape, element.source.key()) {
            (Shape::Line { .. } | Shape::Circle { .. }, Some(Key::Handle(handle))) => Some(handle),
            _ => None,
        })
        .unwrap();
    pair.apply(Edit::Transform {
        target: pair.seen(handle),
        transform: Affine::translation(3.0, 4.0),
    });
    assert_eq!(pair.live.journal.last_is_fast(), Some(true));
    let top = fresh_cx(|cx| pair.live.drawing(cx)).0.unwrap();
    let top_bytes = fresh_cx(|cx| pair.live.save(cx)).0.unwrap();
    for _ in 0..2 {
        assert!(fresh_cx(|cx| pair.live.undo(cx)).0.unwrap());
    }
    for _ in 0..2 {
        assert!(fresh_cx(|cx| pair.live.redo(cx)).0.unwrap());
    }
    assert_eq!(fresh_cx(|cx| pair.live.drawing(cx)).0.unwrap(), top);
    assert_eq!(fresh_cx(|cx| pair.live.save(cx)).0.unwrap(), top_bytes);
    pair.assert_model();
}

#[test]
fn fast_entry_above_layer_table_change_survives_undo_and_redo() {
    for threads in [1, 4] {
        check_fast_entry_above_layer_table_change(DxfPair::open(&synthetic_rich_dxf(), threads));
        for name in [
            "zumen-layout-r2000.dwg",
            "zumen-layout-r2004.dwg",
            "zumen-layout-r2018.dwg",
        ] {
            check_fast_entry_above_layer_table_change(DwgPair::open(&dwg_sample(name), threads));
        }
    }
}

/// 用紙の原点（`$PLIMMIN`）とインチの単位を持ち、今のレイアウトに線分・円・文字・ポリライン・記号を置いた DXF
fn paper_dxf() -> Vec<u8> {
    let mut input = String::from_utf8(synthetic_rich_dxf()).unwrap();
    input = input.replacen("$INSUNITS\n 70\n4\n", "$INSUNITS\n 70\n1\n", 1);
    input = input.replacen(
        "  9\n$PDMODE\n",
        "  9\n$PLIMMIN\n 10\n5.0\n 20\n7.0\n  9\n$PDMODE\n",
        1,
    );
    let head = |kind: &str, handle: &str, marker: &str| {
        format!("0\n{kind}\n5\n{handle}\n330\n42\n100\nAcDbEntity\n67\n1\n8\n0\n100\n{marker}\n")
    };
    let entities = [
        head("LINE", "130", "AcDbLine") + "10\n6\n20\n8\n30\n0\n11\n9\n21\n12\n31\n0\n",
        head("CIRCLE", "131", "AcDbCircle") + "10\n20\n20\n30\n30\n0\n40\n4\n",
        head("TEXT", "132", "AcDbText")
            + "10\n40\n20\n50\n30\n0\n40\n2.5\n1\nPAPER\n100\nAcDbText\n",
        head("LWPOLYLINE", "133", "AcDbPolyline")
            + "90\n3\n70\n0\n10\n10\n20\n10\n10\n20\n20\n10\n10\n20\n20\n20\n",
        head("INSERT", "134", "AcDbBlockReference") + "2\nMARK\n10\n60\n20\n70\n30\n0\n",
    ]
    .concat();
    let at = input.rfind("  0\nENDSEC\n").unwrap();
    input.insert_str(at, &entities);
    input.into_bytes()
}

/// 用紙の要素の `handle` を探し、図形を `affine` で動かしたものを返す
fn paper_element<C: Codec>(pair: &AcadPair<C>, handle: u64, affine: &Affine) -> Placed
where
    C::File: Lift<Drawing2D> + Apply<Changes, Output = Vec<Applied>>,
{
    let model = fresh_cx(|cx| pair.reference.lift(cx)).0.unwrap();
    let mut element = model
        .sheets
        .iter()
        .flat_map(|sheet| &sheet.elements)
        .find(|element| element.source.key() == Some(Key::Handle(handle)))
        .unwrap()
        .clone();
    element.shape = transform::transform(&element.shape, affine).unwrap();
    Placed::Paper(element)
}

/// 画層 0 でない画層の ID（なければ最初の画層）
fn other_layer<C: Codec>(pair: &AcadPair<C>) -> zumen_drawing::LayerId
where
    C::File: Lift<Drawing2D> + Apply<Changes, Output = Vec<Applied>>,
{
    let model = fresh_cx(|cx| pair.reference.lift(cx)).0.unwrap();
    model
        .layers
        .iter()
        .find(|layer| layer.name != "0")
        .or(model.layers.first())
        .unwrap()
        .id
}

/// 用紙の要素を動かし・スタイルを変え・置き換え、どれも高速経路で従来の apply と一致することを確かめる
fn check_paper_edits_on_fast_path<C: Codec>(pair: &mut AcadPair<C>, handles: &[u64])
where
    C::File: Lift<Drawing2D> + Apply<Changes, Output = Vec<Applied>>,
{
    let layer = other_layer(pair);
    for &handle in handles {
        pair.apply(Edit::Transform {
            target: pair.seen(handle),
            transform: Affine::translation(2.0, 3.0),
        });
        assert_eq!(pair.live.journal.last_is_fast(), Some(true), "h{handle:X}");
        pair.apply(Edit::Transform {
            target: pair.seen(handle),
            transform: Affine::rotation(1.0, 2.0, 30.0),
        });
        assert_eq!(pair.live.journal.last_is_fast(), Some(true), "h{handle:X}");
        pair.apply(Edit::Restyle {
            target: pair.seen(handle),
            style: Restyle {
                layer: Some(layer),
                line_type_scale: Some(2.0),
                ..Restyle::default()
            },
        });
        assert_eq!(pair.live.journal.last_is_fast(), Some(true), "h{handle:X}");
        let element = paper_element(pair, handle, &Affine::translation(-3.0, 4.0));
        pair.apply(Edit::Replace {
            target: pair.seen(handle),
            element: Box::new(element),
        });
        assert_eq!(pair.live.journal.last_is_fast(), Some(true), "h{handle:X}");
    }
}

#[test]
fn paper_transform_restyle_and_replace_use_fast_path() {
    for threads in [1, 4] {
        let mut pair = DxfPair::open(&paper_dxf(), threads);
        assert_eq!(pair.live.state.sheet_element_targets.len(), 5);
        check_paper_edits_on_fast_path(&mut pair, &[0x130, 0x131, 0x132, 0x133, 0x134]);
        pair.apply(Edit::Remove {
            target: pair.seen(0x130),
        });
        assert_eq!(pair.live.journal.last_is_fast(), Some(false));
    }
}

/// DWG の見本の今のレイアウトとほかのレイアウト（用紙の単位が違う）に線分を足し（構造経路）、それを差分で直す
fn dwg_paper_pair(name: &str, threads: usize) -> (DwgPair, Vec<u64>) {
    let mut pair = DwgPair::open(&dwg_sample(name), threads);
    let mut handles = Vec::new();
    for sheet_index in 0..3 {
        let model = fresh_cx(|cx| pair.reference.lift(cx)).0.unwrap();
        let sheet = model
            .sheets
            .iter()
            .find(|sheet| sheet.id.0 == sheet_index)
            .unwrap();
        let template = model.spaces.first().unwrap().elements.first().unwrap();
        let point = |x, y| PaperPoint {
            sheet: sheet.id,
            x,
            y,
        };
        let applied = pair.apply(Edit::Add {
            place: id::sheet(sheet),
            element: Box::new(Placed::Paper(Element {
                source: Default::default(),
                layer: template.layer,
                style: template.style,
                shape: Shape::Line {
                    start: point(1.0, 2.0),
                    end: point(3.0, 4.0),
                },
            })),
        });
        assert_eq!(pair.live.journal.last_is_fast(), Some(false));
        let id = applied.first().unwrap().id.clone().unwrap();
        let (_, handle) = id.rsplit_once("/h").unwrap();
        handles.push(u64::from_str_radix(handle, 16).unwrap());
    }
    (pair, handles)
}

#[test]
fn dwg_paper_transform_restyle_and_replace_use_fast_path() {
    // 4 スレッドは R2018 だけで照らし、試験の時間を抑える
    for (name, threads) in [
        ("zumen-layout-r2000.dwg", 1),
        ("zumen-layout-r2004.dwg", 1),
        ("zumen-layout-r2018.dwg", 1),
        ("zumen-layout-r2018.dwg", 4),
    ] {
        let (mut pair, handles) = dwg_paper_pair(name, threads);
        check_paper_edits_on_fast_path(&mut pair, &handles);
    }
}

#[test]
fn paper_edits_share_budget_and_cancellation() {
    for threads in [1, 4] {
        let mut pair = DxfPair::open(&paper_dxf(), threads);
        let layer = other_layer(&pair);
        for handle in [0x130, 0x132, 0x134] {
            pair.apply_with_budgets(Edit::Transform {
                target: pair.seen(handle),
                transform: Affine::translation(2.0, 3.0),
            });
            assert_eq!(pair.live.journal.last_is_fast(), Some(true));
        }
        pair.apply_with_budgets(Edit::Restyle {
            target: pair.seen(0x131),
            style: Restyle {
                layer: Some(layer),
                ..Restyle::default()
            },
        });
        assert_eq!(pair.live.journal.last_is_fast(), Some(true));
        let element = paper_element(&pair, 0x133, &Affine::rotation(0.0, 0.0, 45.0));
        pair.apply_with_budgets(Edit::Replace {
            target: pair.seen(0x133),
            element: Box::new(element),
        });
        assert_eq!(pair.live.journal.last_is_fast(), Some(true));
    }
    let (mut pair, handles) = dwg_paper_pair("zumen-layout-r2018.dwg", 1);
    for handle in handles {
        pair.apply_with_budgets(Edit::Transform {
            target: pair.seen(handle),
            transform: Affine::translation(2.0, 3.0),
        });
        assert_eq!(pair.live.journal.last_is_fast(), Some(true));
        let element = paper_element(&pair, handle, &Affine::translation(1.0, -1.0));
        pair.apply_with_budgets(Edit::Replace {
            target: pair.seen(handle),
            element: Box::new(element),
        });
        assert_eq!(pair.live.journal.last_is_fast(), Some(true));
    }
}

#[test]
fn model_paper_and_block_edits_in_one_apply_use_fast_path() {
    for threads in [1, 4] {
        let mut pair = DxfPair::open(&paper_dxf(), threads);
        let element = paper_element(&pair, 0x131, &Affine::translation(1.0, 1.0));
        pair.apply_edits(vec![
            Edit::Transform {
                target: pair.seen(0x101),
                transform: Affine::translation(2.0, 3.0),
            },
            Edit::Transform {
                target: pair.seen(0x130),
                transform: Affine::translation(2.0, 3.0),
            },
            Edit::Transform {
                target: pair.seen(0x55),
                transform: Affine::translation(2.0, 3.0),
            },
            Edit::Replace {
                target: pair.seen(0x131),
                element: Box::new(element),
            },
            Edit::Restyle {
                target: pair.seen(0x132),
                style: Restyle {
                    line_type_scale: Some(3.0),
                    ..Restyle::default()
                },
            },
        ]);
        assert_eq!(pair.live.journal.last_is_fast(), Some(true));
        // 同じ用紙の要素を 2 回指す一覧は、一括の apply と同じく拒む
        let before = pair.live.undo_len();
        let changes = Changes {
            document: None,
            edits: vec![
                Edit::Transform {
                    target: pair.seen(0x130),
                    transform: Affine::translation(1.0, 0.0),
                },
                Edit::Transform {
                    target: pair.seen(0x130),
                    transform: Affine::translation(0.0, 1.0),
                },
            ],
        };
        let actual = fresh_cx(|cx| pair.live.apply(&changes, cx));
        let expected = fresh_cx(|cx| pair.reference.apply(&changes, cx));
        assert!(expected.0.is_err());
        assert_eq!(actual, expected);
        assert_eq!(pair.live.undo_len(), before);
        pair.assert_model();
    }
    let (mut pair, handles) = dwg_paper_pair("zumen-layout-r2004.dwg", 1);
    let model = fresh_cx(|cx| pair.reference.lift(cx)).0.unwrap();
    let model_handle = model.spaces[0]
        .elements
        .iter()
        .find_map(|element| match (&element.shape, element.source.key()) {
            (Shape::Line { .. } | Shape::Circle { .. }, Some(Key::Handle(handle))) => Some(handle),
            _ => None,
        })
        .unwrap();
    let mut edits = vec![Edit::Transform {
        target: pair.seen(model_handle),
        transform: Affine::translation(2.0, 3.0),
    }];
    for handle in handles {
        edits.push(Edit::Transform {
            target: pair.seen(handle),
            transform: Affine::translation(-1.0, 2.0),
        });
    }
    pair.apply_edits(edits);
    assert_eq!(pair.live.journal.last_is_fast(), Some(true));
}

#[test]
fn stale_paper_hash_is_rejected_like_apply() {
    for threads in [1, 4] {
        let mut pair = DxfPair::open(&paper_dxf(), threads);
        let stale = Seen {
            id: pair.seen(0x130).id,
            hash: pair.seen(0x131).hash,
        };
        pair.reject_with_budgets(Edit::Transform {
            target: stale.clone(),
            transform: Affine::translation(2.0, 3.0),
        });
        pair.reject(Edit::Restyle {
            target: stale.clone(),
            style: Restyle {
                line_type_scale: Some(2.0),
                ..Restyle::default()
            },
        });
        // 一覧の中の古いハッシュは、構造経路で一括の apply と同じ診断を返す
        let changes = Changes {
            document: None,
            edits: vec![
                Edit::Transform {
                    target: pair.seen(0x132),
                    transform: Affine::translation(1.0, 0.0),
                },
                Edit::Transform {
                    target: stale,
                    transform: Affine::translation(0.0, 1.0),
                },
            ],
        };
        let actual = fresh_cx(|cx| pair.live.apply(&changes, cx));
        let expected = fresh_cx(|cx| pair.reference.apply(&changes, cx));
        assert!(expected.0.is_err());
        assert_eq!(actual, expected);
        assert_eq!(pair.live.undo_len(), 0);
    }
    let (mut pair, handles) = dwg_paper_pair("zumen-layout-r2000.dwg", 1);
    let stale = Seen {
        id: pair.seen(handles[0]).id,
        hash: pair.seen(handles[1]).hash,
    };
    pair.reject_with_budgets(Edit::Transform {
        target: stale,
        transform: Affine::translation(2.0, 3.0),
    });
}

/// 用紙の要素の高速な記録と構造の記録（画層の追加）を交互に積み、まとめて取り消してやり直す
fn check_paper_entries_around_structural_entry<C: Codec>(mut pair: AcadPair<C>, handles: &[u64])
where
    C::File: Lift<Drawing2D> + Apply<Changes, Output = Vec<Applied>>,
{
    let (first, second) = (handles.first().unwrap(), handles.get(1).unwrap());
    let bottom = fresh_cx(|cx| pair.live.drawing(cx)).0.unwrap();
    let bottom_bytes = fresh_cx(|cx| pair.live.save(cx)).0.unwrap();
    let base = pair.live.undo_len();
    pair.apply(Edit::Transform {
        target: pair.seen(*first),
        transform: Affine::translation(3.0, 4.0),
    });
    assert_eq!(pair.live.journal.last_is_fast(), Some(true));
    let model = fresh_cx(|cx| pair.reference.lift(cx)).0.unwrap();
    let mut layer = model.layers[0].clone();
    layer.name = "LIVE_NEW".to_owned();
    pair.apply(Edit::AddLayer {
        layer: Box::new(layer),
    });
    assert_eq!(pair.live.journal.last_is_fast(), Some(false));
    let element = paper_element(&pair, *second, &Affine::rotation(0.0, 0.0, 90.0));
    pair.apply(Edit::Replace {
        target: pair.seen(*second),
        element: Box::new(element),
    });
    assert_eq!(pair.live.journal.last_is_fast(), Some(true));
    pair.apply(Edit::Restyle {
        target: pair.seen(*first),
        style: Restyle {
            line_type_scale: Some(0.5),
            ..Restyle::default()
        },
    });
    assert_eq!(pair.live.journal.last_is_fast(), Some(true));
    let top = fresh_cx(|cx| pair.live.drawing(cx)).0.unwrap();
    let top_bytes = fresh_cx(|cx| pair.live.save(cx)).0.unwrap();
    for _ in 0..4 {
        assert!(fresh_cx(|cx| pair.live.undo(cx)).0.unwrap());
    }
    assert_eq!(pair.live.undo_len(), base);
    assert_eq!(fresh_cx(|cx| pair.live.drawing(cx)).0.unwrap(), bottom);
    assert_eq!(fresh_cx(|cx| pair.live.save(cx)).0.unwrap(), bottom_bytes);
    for _ in 0..4 {
        assert!(fresh_cx(|cx| pair.live.redo(cx)).0.unwrap());
    }
    assert_eq!(fresh_cx(|cx| pair.live.drawing(cx)).0.unwrap(), top);
    assert_eq!(fresh_cx(|cx| pair.live.save(cx)).0.unwrap(), top_bytes);
    pair.assert_model();
    // 構造の記録の手前まで戻し、新しい高速な記録で上の記録を捨てても一致する
    for _ in 0..3 {
        assert!(fresh_cx(|cx| pair.live.undo(cx)).0.unwrap());
    }
    pair.reference = {
        let bytes = fresh_cx(|cx| pair.live.save(cx)).0.unwrap();
        fresh_cx(|cx| C::read(&bytes, cx)).0.unwrap()
    };
    pair.apply(Edit::Transform {
        target: pair.seen(*second),
        transform: Affine::translation(-2.0, 1.0),
    });
    assert_eq!(pair.live.journal.last_is_fast(), Some(true));
    assert_eq!(pair.live.redo_len(), 0);
    assert!(fresh_cx(|cx| pair.live.undo(cx)).0.unwrap());
    assert!(fresh_cx(|cx| pair.live.undo(cx)).0.unwrap());
    assert_eq!(fresh_cx(|cx| pair.live.drawing(cx)).0.unwrap(), bottom);
    assert_eq!(fresh_cx(|cx| pair.live.save(cx)).0.unwrap(), bottom_bytes);
}

#[test]
fn paper_fast_entries_interleave_with_structural_entries() {
    for threads in [1, 4] {
        check_paper_entries_around_structural_entry(
            DxfPair::open(&paper_dxf(), threads),
            &[0x130, 0x133],
        );
        let (pair, handles) = dwg_paper_pair("zumen-layout-r2018.dwg", threads);
        check_paper_entries_around_structural_entry(pair, &handles);
    }
}

/// 続くフラグ（66）を持たない INSERT（h150）に、所有者（330）で読めない ATTRIB（押し出し 230 が -1）を付けた DXF。
/// `paper` なら用紙、そうでなければモデル空間に置く
fn orphan_attrib_insert_dxf(paper: bool) -> Vec<u8> {
    let mut input = String::from_utf8(paper_dxf()).unwrap();
    let (owner, flag) = if paper { ("42", "67\n1\n") } else { ("41", "") };
    let entities = format!(
        "0\nINSERT\n5\n150\n330\n{owner}\n100\nAcDbEntity\n{flag}8\n0\n100\nAcDbBlockReference\n2\nMARK\n10\n60\n20\n70\n30\n0\n\
         0\nATTRIB\n5\n151\n330\n150\n100\nAcDbEntity\n{flag}8\n0\n100\nAcDbText\n10\n61\n20\n71\n30\n0\n40\n2\n1\nVAL\n230\n-1\n100\nAcDbAttribute\n2\nTAG\n70\n0\n"
    );
    let at = input.rfind("  0\nENDSEC\n").unwrap();
    input.insert_str(at, &entities);
    input.into_bytes()
}

/// 名前付きブロック OTHER の中に、続くフラグを持たない INSERT（h161）と、所有者で付く読めない ATTRIB（h162）を置いた DXF
fn orphan_attrib_block_dxf() -> Vec<u8> {
    let mut input = String::from_utf8(paper_dxf()).unwrap();
    input = input.replacen(
        "  2\nMARK\n  0\nENDTAB\n",
        "  2\nMARK\n0\nBLOCK_RECORD\n5\n15F\n100\nAcDbSymbolTableRecord\n100\nAcDbBlockTableRecord\n2\nOTHER\n  0\nENDTAB\n",
        1,
    );
    let block = "0\nBLOCK\n5\n160\n330\n15F\n100\nAcDbEntity\n8\n0\n100\nAcDbBlockBegin\n2\nOTHER\n70\n0\n10\n0\n20\n0\n30\n0\n3\nOTHER\n1\n\n\
         0\nINSERT\n5\n161\n330\n15F\n100\nAcDbEntity\n8\n0\n100\nAcDbBlockReference\n2\nMARK\n10\n60\n20\n70\n30\n0\n\
         0\nATTRIB\n5\n162\n330\n161\n100\nAcDbEntity\n8\n0\n100\nAcDbText\n10\n61\n20\n71\n30\n0\n40\n2\n1\nVAL\n230\n-1\n100\nAcDbAttribute\n2\nTAG\n70\n0\n\
         0\nENDBLK\n5\n163\n330\n15F\n100\nAcDbEntity\n8\n0\n100\nAcDbBlockEnd\n";
    let at = input
        .find("  0\nENDSEC\n  0\nSECTION\n  2\nENTITIES\n")
        .unwrap();
    input.insert_str(at, block);
    input.into_bytes()
}

#[test]
fn insert_with_owner_attached_unreadable_attrib_is_rejected_like_apply() {
    for threads in [1, 4] {
        for paper in [false, true] {
            let mut pair = DxfPair::open(&orphan_attrib_insert_dxf(paper), threads);
            pair.reject(Edit::Transform {
                target: pair.seen(0x150),
                transform: Affine::translation(2.0, 3.0),
            });
            if paper {
                // 記号を動かす置き換えも、一括の apply と同じく属性の数の食い違いで断る
                let element = paper_element(&pair, 0x150, &Affine::translation(2.0, 3.0));
                pair.reject(Edit::Replace {
                    target: pair.seen(0x150),
                    element: Box::new(element),
                });
            }
        }
        let mut pair = DxfPair::open(&orphan_attrib_block_dxf(), threads);
        pair.reject(Edit::Transform {
            target: pair.seen(0x161),
            transform: Affine::translation(2.0, 3.0),
        });
    }
}

/// 連想の合成 DXF の図形（h101〜h105）から、各図形が影響する図形のハンドルを返す
fn associative_edges(live: &AcadLive) -> Vec<(u64, Vec<u64>)> {
    dependency_edges(live, &[0x101, 0x102, 0x103, 0x104, 0x105])
}

/// DWG の読み込みは、図形のリアクター（102 `{ACAD_REACTORS`）と DIMASSOC の測る相手（331）を DXF と同じ値で置く。
/// 公開の弧長寸法の DWG（R2004・R2018）を開いた文書エンジンは、対の ASCII DXF と同じく、円弧 0x2d9 から弧長寸法
/// 0x2da への依存辺を持つ（1・4 スレッド）
#[test]
fn dwg_arc_dimension_depends_on_its_arc_like_the_ascii_dxf() {
    for name in ["zumen-arcdim-r2004", "zumen-arcdim-r2018"] {
        let dwg = dwg_sample(&format!("{name}.dwg"));
        let dxf = dwg_sample(&format!("{name}_ascii.dxf"));
        for threads in [1, 4] {
            let from_dwg = fresh_cx(|cx| AcadLive::open_dwg_with_threads(&dwg, threads, cx))
                .0
                .unwrap();
            let from_dxf = fresh_cx(|cx| AcadLive::open_dxf_with_threads(&dxf, threads, cx))
                .0
                .unwrap();
            let edges = dependency_edges(&from_dwg, &[0x2d9, 0x2da]);
            assert!(
                edges.contains(&(0x2d9, vec![0x2da])),
                "{name} {threads}: {edges:x?}"
            );
            assert_eq!(
                edges,
                dependency_edges(&from_dxf, &[0x2d9, 0x2da]),
                "{name}"
            );
        }
    }
}

/// ハンドル `handles` の図形から、各図形が影響する図形（`handles` の中）のハンドルを返す
fn dependency_edges(live: &AcadLive, handles: &[u64]) -> Vec<(u64, Vec<u64>)> {
    handles
        .iter()
        .filter_map(|&handle| {
            let row = live.table().find_handle(handle)?;
            let affected = fresh_cx(|cx| live.affected_rows(&[row], cx)).0.unwrap();
            let others = handles
                .iter()
                .copied()
                .filter(|&other| {
                    other != handle
                        && live
                            .table()
                            .find_handle(other)
                            .is_some_and(|other| affected.contains(&other))
                })
                .collect();
            Some((handle, others))
        })
        .collect()
}

/// 保存したバイト列を開き直した文書エンジンの依存辺
fn reopened_edges(live: &AcadLive) -> Vec<(u64, Vec<u64>)> {
    let saved = fresh_cx(|cx| live.save(cx)).0.unwrap();
    associative_edges(&fresh_cx(|cx| AcadLive::open_dxf(&saved, cx)).0.unwrap())
}

/// 寸法・連想ハッチング・引出線の測る相手と依存する側のそれぞれに、移動・色・線種の尺度・削除を当てる。
/// 結果・診断・保存・保持モデル・取り消し・やり直しを逐次 apply と照らし、依存辺は、編集・取り消し・
/// やり直しのたびに保存したバイト列を開き直したものと一致する。1 つずつ当てたあと、受けた編集を続けて当てる
#[test]
fn associative_dependencies_follow_every_edit_like_reopening() {
    let input = synthetic_associative_dxf();
    let edits = |pair: &DxfPair, handle: u64| {
        vec![
            Edit::Transform {
                target: pair.seen(handle),
                transform: Affine::translation(2.0, 3.0),
            },
            Edit::Restyle {
                target: pair.seen(handle),
                style: Restyle {
                    color: Some(Inherit::Own(zumen_drawing::Color {
                        code: Some(zumen_drawing::ColorCode {
                            system: zumen_drawing::ColorSystem::Aci,
                            value: 5,
                        }),
                        rgb: None,
                    })),
                    ..Restyle::default()
                },
            },
            Edit::Restyle {
                target: pair.seen(handle),
                style: Restyle {
                    line_type_scale: Some(2.0),
                    ..Restyle::default()
                },
            },
            Edit::Remove {
                target: pair.seen(handle),
            },
        ]
    };
    let accepted = |pair: &DxfPair, edit: &Edit| {
        let saved = fresh_cx(|cx| pair.live.save(cx)).0.unwrap();
        let mut probe = fresh_cx(|cx| Dxf::read(&saved, cx)).0.unwrap();
        let changes = Changes {
            document: None,
            edits: vec![edit.clone()],
        };
        fresh_cx(|cx| probe.apply(&changes, cx)).0.is_ok()
    };
    let check = |pair: &mut DxfPair, edit: Edit| -> bool {
        let before = associative_edges(&pair.live);
        assert_eq!(before, reopened_edges(&pair.live));
        if !accepted(pair, &edit) {
            pair.reject(edit);
            assert_eq!(associative_edges(&pair.live), before);
            return false;
        }
        pair.apply(edit);
        let after = associative_edges(&pair.live);
        assert_eq!(after, reopened_edges(&pair.live));
        assert!(fresh_cx(|cx| pair.live.undo(cx)).0.unwrap());
        assert_eq!(associative_edges(&pair.live), before);
        assert!(fresh_cx(|cx| pair.live.redo(cx)).0.unwrap());
        assert_eq!(associative_edges(&pair.live), after);
        true
    };
    for threads in [1, 4] {
        let opened = DxfPair::open(&input, threads);
        let edges = associative_edges(&opened.live);
        assert!(edges.contains(&(0x101, vec![0x102, 0x105])));
        assert!(edges.contains(&(0x103, vec![0x104])));
        let mut taken = Vec::new();
        for handle in 0x101..=0x105 {
            for (index, edit) in edits(&opened, handle).into_iter().enumerate() {
                let mut pair = DxfPair::open(&input, threads);
                if check(&mut pair, edit) {
                    taken.push((handle, index));
                }
            }
        }
        // 連想ハッチングの移動と、引出線が注記として指す MTEXT の削除だけを断る（逐次 apply と同じ）
        let refused = [(0x102, 0), (0x103, 3)];
        let expected: Vec<_> = (0x101..=0x105)
            .flat_map(|handle| (0..4).map(move |index| (handle, index)))
            .filter(|edit| !refused.contains(edit))
            .collect();
        assert_eq!(taken, expected);
        let mut pair = DxfPair::open(&input, threads);
        for &(handle, index) in &taken {
            if pair.live.table().find_handle(handle).is_none() {
                continue;
            }
            let edit = edits(&pair, handle).swap_remove(index);
            check(&mut pair, edit);
        }
    }
}

/// EOF の後ろに不正なタグがある DXF を、1 スレッドと 4 スレッドで同じ経路で開く。並列の読み込みは
/// EOF より後ろの区間も走査するが、その区間の不正なタグで従来のパーサーへ回らず、逐次と同じ予算の
/// 使用量・保存・図形の数になる（#890）
#[test]
fn dxf_trailing_invalid_tags_after_eof_open_like_sequential() {
    let mut src = String::from("  0\r\nSECTION\r\n  2\r\nENTITIES\r\n");
    for i in 0..600 {
        src.push_str(&format!(
            "  0\r\nLINE\r\n  5\r\n{:X}\r\n  8\r\n0\r\n 10\r\n{i}.0\r\n 20\r\n0.0\r\n 11\r\n{i}.5\r\n 21\r\n1.0\r\n",
            0x100 + i
        ));
    }
    src.push_str("  0\r\nENDSEC\r\n  0\r\nEOF\r\n");
    let eof_end = src.len();
    // EOF の後ろは、読まない末尾。group 0 で区間の切れ目を作り、続く group code を数でない値にする
    while src.len() < eof_end * 2 {
        src.push_str("  0\r\nJUNK\r\nxx\r\n1\r\n");
    }
    let src = src.into_bytes();
    assert!(
        crate::dxf::live_io::split_points(&src, 4).len() > 2,
        "区間に分かれる大きさにする"
    );

    let open = |threads: usize| {
        let mut budget = Budget::default();
        let mut diag = Diagnostics::new();
        let cancel = CancelToken::new();
        let live = AcadLive::open_dxf_with_threads(
            &src,
            threads,
            &mut Cx::new(&mut budget, &mut diag, &cancel),
        )
        .unwrap();
        let used: Vec<u64> = Kind::ALL.iter().map(|&kind| budget.used(kind)).collect();
        let saved = fresh_cx(|cx| live.save(cx)).0.unwrap();
        (used, saved, live.table().len(), diag.into_vec())
    };
    let (used1, saved1, rows1, diag1) = open(1);
    let (used4, saved4, rows4, diag4) = open(4);
    assert_eq!(rows1, rows4);
    assert_eq!(used1, used4);
    assert_eq!(saved1, saved4);
    assert_eq!(diag1, diag4);
}

/// 公開の R2000 の DWG（`zumen-layout-r2000.dwg`）のモデル空間に、連想の合成 DXF の LINE・MTEXT・
/// 連想 LEADER を写した DWG と、写した 3 図形のハンドルを返す。DWG の書き戻しは、足す HATCH を連想にできず、
/// DIMASSOC も書けないので、連想ハッチングと寸法は写さない
fn associative_dwg() -> (Vec<u8>, [u64; 3]) {
    use crate::db::{Data, Origin, Section};

    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../samples/dwg/zumen-layout-r2000.dwg");
    let bytes = std::fs::read(path).unwrap();
    let mut dwg = fresh_cx(|cx| Dwg::read(&bytes, cx)).0.unwrap();
    assert_eq!(dwg.version(), b"AC1015");
    let mut db = fresh_cx(|cx| dwg.objects(cx)).0.unwrap();
    let seed = dwg.handle_seed().unwrap();
    let model = db
        .objects()
        .iter()
        .find(|object| {
            object.section == Section::Tables
                && object.kind == b"BLOCK_RECORD"
                && object.values.iter().any(|value| {
                    value.code == 2
                        && matches!(&value.data, Data::Text(name) if name.eq_ignore_ascii_case(b"*Model_Space"))
                })
        })
        .and_then(crate::db::Object::handle)
        .unwrap();
    let source = fresh_cx(|cx| {
        Dxf::read(&synthetic_associative_dxf(), cx).and_then(|file| file.objects(cx))
    })
    .0
    .unwrap();
    let handles = [seed, seed + 1, seed + 2];
    let map = |handle: u64| match handle {
        0x41 => Some(model),
        0x101 => Some(seed),
        0x103 => Some(seed + 1),
        0x104 => Some(seed + 2),
        _ => None,
    };
    for kind in [&b"LINE"[..], b"MTEXT", b"LEADER"] {
        let mut object = source
            .objects()
            .iter()
            .find(|object| object.section == Section::Entities && object.kind == kind)
            .unwrap()
            .clone();
        object.origin = Origin::New;
        // サブクラスの印（100）は DXF だけのもので、DWG の書き戻しは受けない。足す図形はリアクターの組
        // （102）を持てないので、MTEXT のリアクターは、足したあとに書き戻しで付ける
        let mut in_reactors = false;
        object.values.retain(|value| {
            let reactor = value.code == 102 || (in_reactors && value.code == 330);
            if value.code == 102 {
                in_reactors = matches!(&value.data, Data::Text(text) if text.starts_with(b"{"));
            }
            value.code != 100 && !reactor
        });
        for value in &mut object.values {
            if let Data::Handle(handle) = &mut value.data {
                *handle = map(*handle).unwrap();
            }
        }
        // サンプルの寸法スタイルは ISO-25 だけなので、引出線はそれを指す。足す MTEXT は行の間隔（73・44）を持つ
        if kind == b"LEADER" {
            for value in &mut object.values {
                if value.code == 3 {
                    value.data = Data::Text(b"ISO-25".to_vec());
                }
            }
        }
        if kind == b"MTEXT" {
            object.values.extend([
                crate::db::Value {
                    code: 73,
                    data: Data::Int(1),
                },
                crate::db::Value {
                    code: 44,
                    data: Data::Real(1.0),
                },
            ]);
        }
        db.push(object);
    }
    let (written, why) = fresh_cx(|cx| dwg.write_back(&db, cx));
    written.unwrap_or_else(|e| panic!("{e:?} {why:?}"));
    let [_, mtext, leader] = handles;
    let mut db = fresh_cx(|cx| dwg.objects(cx)).0.unwrap();
    let position = db.position(mtext).unwrap();
    let object = db.object_mut(position).unwrap();
    let at = object
        .values
        .iter()
        .position(|value| value.code == 5)
        .unwrap()
        + 1;
    let reactors = [
        (102, Data::Text(b"{ACAD_REACTORS".to_vec())),
        (330, Data::Handle(leader)),
        (102, Data::Text(b"}".to_vec())),
    ];
    for (offset, (code, data)) in reactors.into_iter().enumerate() {
        object
            .values
            .insert(at + offset, crate::db::Value { code, data });
    }
    let (written, why) = fresh_cx(|cx| dwg.write_back(&db, cx));
    written.unwrap_or_else(|e| panic!("{e:?} {why:?}"));
    let mut out = Vec::new();
    fresh_cx(|cx| Dwg::write(&dwg, &mut out, cx)).0.unwrap();
    (out, handles)
}

/// DWG の `input` の図形 `handles` のそれぞれに、移動・色・線種の尺度・削除を当て、当てられた
/// （ハンドル, 編集の番号）を返す。1 つずつ当てたときと、当てられたものを続けて当てたときの両方で、結果・診断・保存・
/// 保持モデル・取り消し・やり直しを逐次 apply と照らし、依存辺は、編集・取り消し・やり直しのたびに、保存した
/// バイト列を開き直したものと一致する
fn dwg_dependency_matrix(input: &[u8], handles: &[u64], threads: usize) -> Vec<(u64, usize)> {
    let edges = |live: &AcadLive| dependency_edges(live, handles);
    let reopened = |live: &AcadLive| {
        let saved = fresh_cx(|cx| live.save(cx)).0.unwrap();
        edges(
            &fresh_cx(|cx| AcadLive::open_with_threads(&saved, 1, cx))
                .0
                .unwrap(),
        )
    };
    let edits = |pair: &DwgPair, handle: u64| {
        vec![
            Edit::Transform {
                target: pair.seen(handle),
                transform: Affine::translation(2.0, 3.0),
            },
            Edit::Restyle {
                target: pair.seen(handle),
                style: Restyle {
                    color: Some(Inherit::Own(zumen_drawing::Color {
                        code: Some(zumen_drawing::ColorCode {
                            system: zumen_drawing::ColorSystem::Aci,
                            value: 5,
                        }),
                        rgb: None,
                    })),
                    ..Restyle::default()
                },
            },
            Edit::Restyle {
                target: pair.seen(handle),
                style: Restyle {
                    line_type_scale: Some(2.0),
                    ..Restyle::default()
                },
            },
            Edit::Remove {
                target: pair.seen(handle),
            },
        ]
    };
    let accepted = |pair: &DwgPair, edit: &Edit| {
        let saved = fresh_cx(|cx| pair.live.save(cx)).0.unwrap();
        let mut probe = fresh_cx(|cx| Dwg::read(&saved, cx)).0.unwrap();
        let changes = Changes {
            document: None,
            edits: vec![edit.clone()],
        };
        fresh_cx(|cx| probe.apply(&changes, cx)).0.is_ok()
    };
    let check = |pair: &mut DwgPair, edit: Edit| -> bool {
        let before = edges(&pair.live);
        assert_eq!(before, reopened(&pair.live));
        if !accepted(pair, &edit) {
            pair.reject(edit);
            assert_eq!(edges(&pair.live), before);
            return false;
        }
        pair.apply(edit);
        let after = edges(&pair.live);
        assert_eq!(after, reopened(&pair.live));
        assert!(fresh_cx(|cx| pair.live.undo(cx)).0.unwrap());
        assert_eq!(edges(&pair.live), before);
        assert!(fresh_cx(|cx| pair.live.redo(cx)).0.unwrap());
        assert_eq!(edges(&pair.live), after);
        true
    };
    let opened = DwgPair::open(input, threads);
    let mut taken = Vec::new();
    for &handle in handles {
        for (index, edit) in edits(&opened, handle).into_iter().enumerate() {
            let mut pair = DwgPair::open(input, threads);
            if check(&mut pair, edit) {
                taken.push((handle, index));
            }
        }
    }
    let mut pair = DwgPair::open(input, threads);
    for &(handle, index) in &taken {
        if pair.live.table().find_handle(handle).is_none() {
            continue;
        }
        let edit = edits(&pair, handle).swap_remove(index);
        check(&mut pair, edit);
    }
    taken
}

/// 公開の DWG に写した引出線の注記（340 と、MTEXT のリアクター）は、編集のたびに開き直したときと同じ依存辺を保つ
/// （1・4 スレッド）
#[test]
fn dwg_leader_dependencies_follow_every_edit_like_reopening() {
    let (input, handles) = associative_dwg();
    let [_, mtext, leader] = handles;
    for threads in [1, 4] {
        let opened = DwgPair::open(&input, threads);
        let found = dependency_edges(&opened.live, &handles);
        assert!(found.contains(&(mtext, vec![leader])), "{found:x?}");
        // 引出線と、引出線が注記として指す MTEXT の削除だけを断る（逐次 apply と同じ）
        let refused = [(mtext, 3), (leader, 3)];
        let expected: Vec<_> = handles
            .into_iter()
            .flat_map(|handle| (0..4).map(move |index| (handle, index)))
            .filter(|edit| !refused.contains(edit))
            .collect();
        assert_eq!(dwg_dependency_matrix(&input, &handles, threads), expected);
    }
}

fn check_add_symbol_and_nested_propagation<C: Codec>(mut pair: AcadPair<C>)
where
    C::File: Lift<Drawing2D> + Apply<Changes, Output = Vec<Applied>>,
{
    use zumen_drawing::{Axes, ModelSpace, SpaceId, Symbol, SymbolDefinition, SymbolKey};

    let model = fresh_cx(|cx| pair.reference.lift(cx)).0.unwrap();
    let base_style = model.spaces[0].elements[0].style;
    let layer_id = model.layers[0].id;
    let def_space = SpaceId(100);
    let inner_key = SymbolKey {
        namespace: "dxf.block".into(),
        name: "LIVE_INNER".to_owned(),
    };
    let outer_key = SymbolKey {
        namespace: "dxf.block".into(),
        name: "LIVE_OUTER".to_owned(),
    };

    let inner_def = SymbolDefinition {
        key: inner_key.clone(),
        space: ModelSpace {
            id: def_space,
            unit: model.spaces[0].unit,
            axes: Axes::EastNorth,
            elements: vec![Element {
                source: Default::default(),
                layer: layer_id,
                style: base_style,
                shape: Shape::Line {
                    start: zumen_drawing::ModelPoint {
                        space: def_space,
                        x: 0.0,
                        y: 0.0,
                    },
                    end: zumen_drawing::ModelPoint {
                        space: def_space,
                        x: 4.0,
                        y: 0.0,
                    },
                },
            }],
            order: vec![0],
        },
        base: Some(zumen_drawing::ModelPoint {
            space: def_space,
            x: 0.0,
            y: 0.0,
        }),
        attributes: Vec::new(),
        anonymous: false,
        external: None,
        source: Default::default(),
    };
    pair.apply(Edit::AddSymbol {
        symbol: Box::new(inner_def),
    });

    let outer_def = SymbolDefinition {
        key: outer_key.clone(),
        space: ModelSpace {
            id: def_space,
            unit: model.spaces[0].unit,
            axes: Axes::EastNorth,
            elements: vec![Element {
                source: Default::default(),
                layer: layer_id,
                style: base_style,
                shape: Shape::Symbol(Symbol {
                    key: inner_key.clone(),
                    position: zumen_drawing::ModelPoint {
                        space: def_space,
                        x: 10.0,
                        y: 20.0,
                    },
                    scale: [1.0, 1.0],
                    rotation: 0.0,
                    grid: None,
                    attributes: Vec::new(),
                    clip: None,
                }),
            }],
            order: vec![0],
        },
        base: Some(zumen_drawing::ModelPoint {
            space: def_space,
            x: 0.0,
            y: 0.0,
        }),
        attributes: Vec::new(),
        anonymous: false,
        external: None,
        source: Default::default(),
    };
    pair.apply(Edit::AddSymbol {
        symbol: Box::new(outer_def),
    });

    let space_id = model.spaces[0].id;
    let added = pair
        .apply(Edit::Add {
            place: "space/0".to_owned(),
            element: Box::new(Placed::Model(Element {
                source: Default::default(),
                layer: layer_id,
                style: base_style,
                shape: Shape::Symbol(Symbol {
                    key: outer_key.clone(),
                    position: zumen_drawing::ModelPoint {
                        space: space_id,
                        x: 500.0,
                        y: 600.0,
                    },
                    scale: [1.0, 1.0],
                    rotation: 0.0,
                    grid: None,
                    attributes: Vec::new(),
                    clip: None,
                }),
            })),
        })
        .pop()
        .unwrap();
    let inserted_id = added.id.unwrap();
    let (_, ins_hex) = inserted_id.rsplit_once("/h").unwrap();
    let ins_handle = u64::from_str_radix(ins_hex, 16).unwrap();
    let ins_row = pair.live.table().find_handle(ins_handle).unwrap();

    let select = |live: &AcadLive, rect: [f64; 4]| -> Vec<RowId> {
        let mut out = Vec::new();
        fresh_cx(|cx| live.select_rect_with_cx(rect, &mut out, cx))
            .0
            .unwrap();
        out
    };
    // 初期の挿入範囲は (510, 620)..(514, 620) を含む
    assert!(select(&pair.live, [509.0, 619.0, 515.0, 621.0]).contains(&ins_row));
    assert!(!select(&pair.live, [559.0, 659.0, 565.0, 661.0]).contains(&ins_row));

    let cur_model = fresh_cx(|cx| pair.reference.lift(cx)).0.unwrap();
    let inner_idx = cur_model
        .symbols
        .iter()
        .position(|s| s.key == inner_key)
        .unwrap() as u32;
    let outer_idx = cur_model
        .symbols
        .iter()
        .position(|s| s.key == outer_key)
        .unwrap() as u32;
    let Some(Key::Handle(inner_line_handle)) = cur_model.symbols[inner_idx as usize].space.elements
        [0]
    .source
    .key() else {
        panic!("handle required");
    };

    pair.live.clear_dirty_symbols();
    let before_move = fresh_cx(|cx| pair.live.drawing(cx)).0.unwrap();
    let before_move_bytes = fresh_cx(|cx| pair.live.save(cx)).0.unwrap();

    // 内側定義の線分を (+50, +40) 動かすと、高速経路で外側定義と最上位挿入の箱・変更定義集合へ伝播する
    let target = pair.seen(inner_line_handle);
    let changes = Changes {
        document: None,
        edits: vec![Edit::Transform {
            target,
            transform: Affine::translation(50.0, 40.0),
        }],
    };
    let actual = fresh_cx(|cx| pair.live.apply(&changes, cx));
    let expected = fresh_cx(|cx| pair.reference.apply(&changes, cx));
    assert_eq!(actual, expected);
    assert_eq!(pair.live.journal.last_is_fast(), Some(true));
    assert!(pair.live.dirty_symbols().contains(&inner_idx));
    assert!(pair.live.dirty_symbols().contains(&outer_idx));
    assert!(select(&pair.live, [559.0, 659.0, 565.0, 661.0]).contains(&ins_row));
    pair.assert_model();

    let after_move_bytes = fresh_cx(|cx| pair.live.save(cx)).0.unwrap();
    let expected_bytes = fresh_cx(|cx| {
        let mut out = Vec::new();
        C::write(&pair.reference, &mut out, cx).map(|()| out)
    })
    .0
    .unwrap();
    assert_eq!(after_move_bytes, expected_bytes);

    pair.live.clear_dirty_symbols();
    assert!(fresh_cx(|cx| pair.live.undo(cx)).0.unwrap());
    assert!(pair.live.dirty_symbols().contains(&inner_idx));
    assert!(pair.live.dirty_symbols().contains(&outer_idx));
    assert_eq!(fresh_cx(|cx| pair.live.drawing(cx)).0.unwrap(), before_move);
    assert_eq!(
        fresh_cx(|cx| pair.live.save(cx)).0.unwrap(),
        before_move_bytes
    );
    assert!(!select(&pair.live, [559.0, 659.0, 565.0, 661.0]).contains(&ins_row));
    assert!(select(&pair.live, [509.0, 619.0, 515.0, 621.0]).contains(&ins_row));

    assert!(fresh_cx(|cx| pair.live.redo(cx)).0.unwrap());
    assert_eq!(
        fresh_cx(|cx| pair.live.save(cx)).0.unwrap(),
        after_move_bytes
    );
    assert!(select(&pair.live, [559.0, 659.0, 565.0, 661.0]).contains(&ins_row));
    pair.assert_model();
}

#[test]
fn add_symbol_and_nested_definition_propagation_match_apply() {
    let dxf = dwg_sample("zumen-layout-r2018_ascii.dxf");
    for threads in [1, 4] {
        check_add_symbol_and_nested_propagation(DxfPair::open(&dxf, threads));
        for name in [
            "zumen-layout-r2000.dwg",
            "zumen-layout-r2004.dwg",
            "zumen-layout-r2018.dwg",
        ] {
            check_add_symbol_and_nested_propagation(DwgPair::open(&dwg_sample(name), threads));
        }
    }
}

#[test]
fn callout_table_attributed_insert_and_remaining_dwg_samples_match_apply() {
    let mut source = String::from_utf8(synthetic_rich_dxf()).unwrap();
    let at = source.rfind("  0\nENDSEC\n").unwrap();
    let extra = concat!(
        "0\nMULTILEADER\n5\n125\n330\n41\n100\nAcDbEntity\n8\n0\n100\nAcDbMLeader\n42\n2.5\n",
        "300\nCONTEXT_DATA{\n40\n1.0\n10\n0.0\n20\n0.0\n30\n0.0\n140\n2.5\n290\n1\n304\nCALL\n",
        "12\n15.0\n22\n25.0\n32\n0.0\n41\n2.5\n",
        "302\nLEADER{\n290\n1\n10\n12.0\n20\n25.0\n30\n0.0\n",
        "304\nLEADER_LINE{\n10\n5.0\n20\n10.0\n30\n0.0\n10\n10.0\n20\n25.0\n30\n0.0\n305\n}\n303\n}\n301\n}\n",
        "0\nACAD_TABLE\n5\n126\n330\n41\n100\nAcDbEntity\n8\n0\n100\nAcDbBlockReference\n",
        "10\n30.0\n20\n40.0\n30\n0.0\n100\nAcDbTable\n91\n1\n92\n2\n141\n5.0\n142\n12.0\n142\n15.0\n",
        "171\n1\n1\nA1\n171\n1\n1\nB1\n",
    );
    source.insert_str(at, extra);
    let dxf_bytes = source.into_bytes();

    for threads in [1, 4] {
        let mut pair = DxfPair::open(&dxf_bytes, threads);
        let model = fresh_cx(|cx| pair.reference.lift(cx)).0.unwrap();
        assert!(
            model.spaces[0]
                .elements
                .iter()
                .any(|e| matches!(e.shape, Shape::Callout(_)))
        );
        assert!(
            model.spaces[0]
                .elements
                .iter()
                .any(|e| matches!(e.shape, Shape::Table(_)))
        );

        // MULTILEADER と ACAD_TABLE に対するスタイル変更・平行移動・削除を逐次 apply と照らす
        for handle in [0x125_u64, 0x126] {
            for candidate in [
                Edit::Restyle {
                    target: pair.seen(handle),
                    style: Restyle {
                        line_type_scale: Some(2.0),
                        ..Restyle::default()
                    },
                },
                Edit::Transform {
                    target: pair.seen(handle),
                    transform: Affine::translation(3.0, 4.0),
                },
            ] {
                let mut probe = pair.reference.clone();
                let changes = Changes {
                    document: None,
                    edits: vec![candidate.clone()],
                };
                if fresh_cx(|cx| probe.apply(&changes, cx)).0.is_ok() {
                    pair.apply(candidate);
                } else {
                    pair.reject(candidate);
                }
            }
            let remove = Edit::Remove {
                target: pair.seen(handle),
            };
            let mut probe = pair.reference.clone();
            let changes = Changes {
                document: None,
                edits: vec![remove.clone()],
            };
            if fresh_cx(|cx| probe.apply(&changes, cx)).0.is_ok() {
                pair.apply(remove);
            } else {
                pair.reject(remove);
            }
        }

        // 属性つき INSERT（h106）の移動・属性文字書き換え・属性つき挿入追加・点スタイル変更
        pair.apply(Edit::SetPointStyle {
            style: zumen_drawing::PointStyle {
                mark: zumen_drawing::PointMark::Cross,
                circle: true,
                square: true,
                size: zumen_drawing::PointSize::Length(3.0),
            },
        });
        pair.apply(Edit::Transform {
            target: pair.seen(0x106),
            transform: Affine::translation(4.0, -2.0),
        });
        let mut insert = pair.element(0x106);
        let Shape::Symbol(sym) = &mut insert.shape else {
            panic!("symbol required");
        };
        sym.attributes[0].text.content = "B-99".to_owned();
        pair.apply(Edit::Replace {
            target: pair.seen(0x106),
            element: Box::new(Placed::Model(insert.clone())),
        });
        let added_ins = pair
            .apply(Edit::Add {
                place: "space/0".to_owned(),
                element: Box::new(Placed::Model(insert)),
            })
            .pop()
            .unwrap();
        // 属性（ATTRIB・SEQEND）を伴う INSERT の削除は逐次 apply と同じ診断で断り、状態を変えない
        pair.reject(Edit::Remove {
            target: Seen {
                id: added_ins.id.unwrap(),
                hash: added_ins.hash.unwrap(),
            },
        });
    }

    // 公開サンプルの残りの DWG（xclip・points-b）のオープン・編集・取り消し・やり直し照合
    for name in [
        "zumen-xclip-r2013.dwg",
        "zumen-xclip-r2018.dwg",
        "zumen-points-b-r2000.dwg",
        "zumen-points-b-r2018.dwg",
    ] {
        let bytes = dwg_sample(name);
        for threads in [1, 4] {
            let mut pair = DwgPair::open(&bytes, threads);
            pair.apply(Edit::SetPointStyle {
                style: zumen_drawing::PointStyle {
                    mark: zumen_drawing::PointMark::Plus,
                    circle: false,
                    square: true,
                    size: zumen_drawing::PointSize::Length(1.5),
                },
            });
            let model = fresh_cx(|cx| pair.reference.lift(cx)).0.unwrap();
            if let Some(handle) =
                model.spaces[0]
                    .elements
                    .iter()
                    .find_map(|el| match el.source.key() {
                        Some(Key::Handle(h)) => Some(h),
                        _ => None,
                    })
            {
                let restyle = Edit::Restyle {
                    target: pair.seen(handle),
                    style: Restyle {
                        line_type_scale: Some(2.0),
                        ..Restyle::default()
                    },
                };
                let mut probe = pair.reference.clone();
                if fresh_cx(|cx| {
                    probe.apply(
                        &Changes {
                            document: None,
                            edits: vec![restyle.clone()],
                        },
                        cx,
                    )
                })
                .0
                .is_ok()
                {
                    pair.apply(restyle);
                } else {
                    pair.reject(restyle);
                }
            }
        }
    }
}
