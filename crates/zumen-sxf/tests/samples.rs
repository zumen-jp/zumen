//! 試料の SXF（`ZUMEN_SAMPLES` のフォルダー）で確かめる。図面はリポジトリに入れない

use std::collections::BTreeSet;
use std::path::PathBuf;

use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Key};
use zumen_drawing::id;
use zumen_sdk::{Lift, testkit};
use zumen_sxf::Sxf;

/// フォルダーの中の SFC と P21
#[cfg(test)]
#[allow(
    clippy::disallowed_methods,
    reason = "環境変数を読むのは試験だけで、コアは読まない（AGENTS.md の図面と秘密）"
)]
fn samples() -> Vec<PathBuf> {
    let Some(dir) = std::env::var_os("ZUMEN_SAMPLES").filter(|d| !d.is_empty()) else {
        return Vec::new();
    };
    let mut found: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("sfc") || e.eq_ignore_ascii_case("p21"))
        })
        .collect();
    found.sort();
    found
}

/// どの要素も、インスタンスの番号の鍵を持ち、ID はその番号で作られて重ならない（決定 0156）
#[test]
fn every_element_is_named_by_its_instance_number() {
    for path in samples() {
        let src = std::fs::read(&path).unwrap();
        let file = testkit::read::<Sxf>(&src).0.unwrap();
        let mut budget = Budget::default();
        let mut diag = Diagnostics::new();
        let cancel = CancelToken::new();
        let drawing = file
            .lift(&mut Cx::new(&mut budget, &mut diag, &cancel))
            .unwrap();
        let mut ids = BTreeSet::new();
        let mut check = |place: &str, sources: Vec<(&zumen_base::Ref, String)>| {
            for (source, id) in sources {
                assert!(
                    matches!(source.key(), Some(Key::Instance(_))),
                    "{} {id}",
                    path.display()
                );
                assert!(id.starts_with(place), "{} {id}", path.display());
                assert!(id.contains("/i"), "{} {id}", path.display());
                assert!(ids.insert(id), "{}", path.display());
            }
        };
        for space in &drawing.spaces {
            let place = id::space(space.id);
            let mut namer = id::Namer::new(&place);
            let named = space
                .elements
                .iter()
                .enumerate()
                .map(|(index, element)| (&element.source, namer.next(index, element)))
                .collect();
            check(&place, named);
        }
        for sheet in &drawing.sheets {
            let place = id::sheet(sheet);
            let mut namer = id::Namer::new(&place);
            let named = sheet
                .elements
                .iter()
                .enumerate()
                .map(|(index, element)| (&element.source, namer.next(index, element)))
                .collect();
            check(&place, named);
        }
    }
}

/// 試料の SFC と P21 で、用紙と部分図の線分・円・円弧・折線を動かし、文字を除き、用紙に線分を足して書き戻す
/// （決定 0167・0169）。複合曲線がまとめる曲線と、作図グループの展開で複数の要素になるものは断るので、断った変更を
/// 除いて当て直す。変えなかった要素の ID と値が変わらないことは、apply が読み直して確かめる
#[test]
fn samples_take_back_moved_removed_and_added_elements() {
    use zumen_base::codes;
    use zumen_drawing::{
        Affine, Changes, Edge, Edit, Element, Hash, PaperPoint, Placed, Seen, Shape,
    };
    use zumen_sdk::{Apply, Codec};

    fn shaped<P>(shape: &Shape<P>) -> bool {
        matches!(
            shape,
            Shape::Line { .. }
                | Shape::Circle { .. }
                | Shape::Polyline(_)
                | Shape::Curve(Edge::Arc { .. })
        )
    }
    let mut written = 0;
    for path in samples() {
        let src = std::fs::read(&path).unwrap();
        let file = testkit::read::<Sxf>(&src).0.unwrap();
        let mut budget = Budget::default();
        let mut diag = Diagnostics::new();
        let cancel = CancelToken::new();
        let drawing = file
            .lift(&mut Cx::new(&mut budget, &mut diag, &cancel))
            .unwrap();
        let shift = Affine {
            m: [1.0, 0.0, 5.0, 0.0, 1.0, -3.0],
        };
        let mut edits = Vec::new();
        let mut line: Option<Element<PaperPoint>> = None;
        for space in &drawing.spaces {
            let place = id::space(space.id);
            let mut namer = id::Namer::new(&place);
            for (index, element) in space.elements.iter().enumerate() {
                let target = Seen {
                    id: namer.next(index, element),
                    hash: Hash::element(element),
                };
                if shaped(&element.shape) {
                    edits.push(Edit::Transform {
                        target,
                        transform: shift,
                    });
                } else if matches!(element.shape, Shape::Formatted(_)) {
                    edits.push(Edit::Remove { target });
                }
            }
        }
        let sheet = &drawing.sheets[0];
        let place = id::sheet(sheet);
        let mut namer = id::Namer::new(&place);
        for (index, element) in sheet.elements.iter().enumerate() {
            let target = Seen {
                id: namer.next(index, element),
                hash: Hash::element(element),
            };
            if shaped(&element.shape) {
                if line.is_none() && matches!(element.shape, Shape::Line { .. }) {
                    line = Some(element.clone());
                }
                edits.push(Edit::Transform {
                    target,
                    transform: shift,
                });
            } else if matches!(element.shape, Shape::Formatted(_)) {
                edits.push(Edit::Remove { target });
            }
        }
        if let Some(line) = line {
            edits.push(Edit::Add {
                place: place.clone(),
                element: Box::new(Placed::Paper(line)),
            });
        }
        let apply = |edits: &[Edit]| {
            let mut file = file.clone();
            let mut budget = Budget::default();
            let mut diag = Diagnostics::new();
            let cancel = CancelToken::new();
            let changes = Changes {
                edits: edits.to_vec(),
                document: None,
            };
            let result = file.apply(&changes, &mut Cx::new(&mut budget, &mut diag, &cancel));
            (result, file, diag)
        };
        let (result, _, diag) = apply(&edits);
        if result.is_err() {
            // 断った変更の要素を除いて、当て直す
            let refused: BTreeSet<u64> = diag
                .iter()
                .filter(|d| d.code() == codes::CHANGE_EDIT_UNSUPPORTED)
                .flat_map(|d| d.places().iter().filter_map(|place| place.record()))
                .collect();
            assert!(
                diag.iter()
                    .all(|d| d.code() == codes::CHANGE_EDIT_UNSUPPORTED && !d.places().is_empty()),
                "{} {:?}",
                path.display(),
                diag.iter().map(|d| format!("{d:?}")).collect::<Vec<_>>()
            );
            let record = |edit: &Edit| {
                let id = match edit {
                    Edit::Transform { target, .. } | Edit::Remove { target } => &target.id,
                    _ => return None,
                };
                let instance: u64 = id.rsplit_once("/i")?.1.parse().ok()?;
                file.records()
                    .iter()
                    .position(|record| record.id() == Some(instance))
                    .map(|at| at as u64)
            };
            assert!(refused.len() * 10 < edits.len(), "{}", path.display());
            edits.retain(|edit| record(edit).is_none_or(|at| !refused.contains(&at)));
        }
        let (result, updated, mut diag) = apply(&edits);
        let applied = result.unwrap_or_else(|error| {
            panic!(
                "{} {error:?} {:?}",
                path.display(),
                diag.iter().map(|d| format!("{d:?}")).collect::<Vec<_>>()
            )
        });
        assert_eq!(applied.len(), edits.len(), "{}", path.display());
        let mut out = Vec::new();
        Sxf::write(
            &updated,
            &mut out,
            &mut Cx::new(&mut budget, &mut diag, &cancel),
        )
        .unwrap();
        let reread = testkit::read::<Sxf>(&out).0.unwrap();
        let after = reread
            .lift(&mut Cx::new(&mut budget, &mut diag, &cancel))
            .unwrap();
        let removed = edits
            .iter()
            .filter(|edit| matches!(edit, Edit::Remove { .. }))
            .count();
        let added = edits
            .iter()
            .filter(|edit| matches!(edit, Edit::Add { .. }))
            .count();
        let count = |drawing: &zumen_drawing::Drawing2D| {
            drawing.sheets[0].elements.len()
                + drawing
                    .spaces
                    .iter()
                    .map(|space| space.elements.len())
                    .sum::<usize>()
        };
        assert!(removed > 0 && added > 0, "{}", path.display());
        assert_eq!(
            count(&after) + removed,
            count(&drawing) + added,
            "{}",
            path.display()
        );
        written += 1;
    }
    if !samples().is_empty() {
        assert!(written > 0);
    }
}
