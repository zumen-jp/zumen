//! 保持モデル・保存・取り消しを従来の逐次 apply と直接比較する。

use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Key, Kind, Limit, Limits};
use zumen_drawing::{
    Affine, Applied, Changes, Drawing2D, Edge, Edit, Hash, Placed, Restyle, Seen, Shape, id,
    transform,
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
        let actual = fresh_cx(|cx| self.live.drawing(cx)).0.unwrap();
        let expected = expected.unwrap();
        assert_eq!(actual, expected);
        assert_eq!(
            all_element_ids_and_hashes(&actual),
            all_element_ids_and_hashes(&expected)
        );
        let saved = fresh_cx(|cx| self.live.save(cx)).0.unwrap();
        let (reloaded, diagnostics) =
            fresh_cx(|cx| C::read(&saved, cx).and_then(|file| file.lift(cx)));
        assert_eq!(reloaded.unwrap(), actual);
        assert_eq!(diagnostics, expected_diag);
        for index in 0..self.live.table().len() {
            let row = RowId::from_index(index).unwrap();
            if !self.live.table().is_alive(row) {
                continue;
            }
            let element = fresh_cx(|cx| self.live.element_with_source(row, cx))
                .0
                .unwrap()
                .unwrap();
            let expected = expected
                .spaces
                .iter()
                .flat_map(|space| &space.elements)
                .find(|candidate| candidate.source.key() == element.source.key())
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
        let before = fresh_cx(|cx| self.live.drawing(cx)).0.unwrap();
        let before_bytes = fresh_cx(|cx| self.live.save(cx)).0.unwrap();
        let changes = Changes {
            document: None,
            edits: vec![edit],
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

#[test]
fn layer_table_changes_match_sequential_apply() {
    for threads in [1, 4] {
        let mut pair = DxfPair::open(&synthetic_rich_dxf(), threads);
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
}

#[test]
fn text_style_table_changes_match_sequential_apply() {
    for threads in [1, 4] {
        let mut pair = DxfPair::open(&synthetic_rich_dxf(), threads);
        let model = fresh_cx(|cx| pair.reference.lift(cx)).0.unwrap();
        let mut style = model.text_styles[0].clone();
        style.name = "LIVE_NEW".to_owned();
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
}

#[test]
fn line_type_table_changes_match_sequential_apply() {
    for threads in [1, 4] {
        let mut pair = DxfPair::open(&synthetic_rich_dxf(), threads);
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
}

#[test]
fn dimension_style_table_changes_match_sequential_apply() {
    for threads in [1, 4] {
        let mut pair = DxfPair::open(&synthetic_associative_dxf(), threads);
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
fn dwg_general_shape_additions_match_sequential_apply() {
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
            }
            added.push(element);
        }
    }
    assert_eq!(added.len(), 5);
    for name in ["zumen-layout-r2000.dwg", "zumen-layout-r2018.dwg"] {
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
                let edit = Edit::Add {
                    place: "space/0".to_owned(),
                    element: Box::new(Placed::Model(element)),
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
