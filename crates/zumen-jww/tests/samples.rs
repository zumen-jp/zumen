//! 試料の JWW で確かめる。試料は公開サンプルの `samples/jww` と、受け取った図面を置く `ZUMEN_SAMPLES` の
//! フォルダーから集める

use std::path::PathBuf;

use zumen_base::{Budget, CancelToken, Cx, Diagnostics};
use zumen_drawing::{
    Affine, Changes, Drawing2D, Edge, Edit, Element, Hash, ModelPoint, Placed, Seen, Shape, id,
};
use zumen_jww::Jww;
use zumen_sdk::{Apply, Codec, Lift, testkit};

/// フォルダーの中の JWW
#[cfg(test)]
#[allow(
    clippy::disallowed_methods,
    reason = "環境変数を読むのは試験だけで、コアは読まない（AGENTS.md の図面と秘密）"
)]
fn samples() -> Vec<PathBuf> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../samples");
    let dirs = [root.join("jww")]
        .into_iter()
        .filter(|dir| dir.is_dir())
        .chain(
            std::env::var_os("ZUMEN_SAMPLES")
                .filter(|d| !d.is_empty())
                .map(PathBuf::from),
        );
    let mut found: Vec<_> = dirs
        .flat_map(|dir| std::fs::read_dir(dir).unwrap())
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("jww"))
        })
        .collect();
    found.sort();
    found
}

#[cfg(test)]
fn lift(file: &zumen_jww::JwwFile) -> Drawing2D {
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    file.lift(&mut Cx::new(&mut budget, &mut diag, &cancel))
        .unwrap()
}

#[cfg(test)]
fn shaped(shape: &Shape<ModelPoint>) -> bool {
    matches!(
        shape,
        Shape::Line { .. } | Shape::Circle { .. } | Shape::Curve(Edge::Arc { .. })
    )
}

#[cfg(test)]
fn count(drawing: &Drawing2D, which: fn(&Shape<ModelPoint>) -> bool) -> usize {
    drawing
        .spaces
        .iter()
        .flat_map(|space| &space.elements)
        .chain(
            drawing
                .symbols
                .iter()
                .flat_map(|symbol| &symbol.space.elements),
        )
        .filter(|element| which(&element.shape))
        .count()
}

/// 線・円・円弧を動かし、文字を除き、線を足して書き戻す。読み直すと、要素の数が合う
#[test]
fn jww_samples_take_back_moved_removed_and_added_elements() {
    let mut written = 0;
    for path in samples() {
        let src = std::fs::read(&path).unwrap();
        let file = testkit::read::<Jww>(&src).0.unwrap();
        let drawing = lift(&file);
        let shift = Affine {
            m: [1.0, 0.0, 5.0, 0.0, 1.0, -3.0],
        };
        let mut edits = Vec::new();
        let mut line: Option<(String, Element<ModelPoint>)> = None;
        let mut removed = 0;
        let mut note = |place: &str, elements: &[Element<ModelPoint>], model: bool| {
            let mut namer = id::Namer::new(place);
            for (index, element) in elements.iter().enumerate() {
                let target = Seen {
                    id: namer.next(index, element),
                    hash: Hash::element(element),
                };
                if shaped(&element.shape) {
                    if model && line.is_none() && matches!(element.shape, Shape::Line { .. }) {
                        line = Some((place.to_owned(), element.clone()));
                    }
                    edits.push(Edit::Transform {
                        target,
                        transform: shift,
                    });
                } else if model && matches!(element.shape, Shape::Text(_)) {
                    removed += 1;
                    edits.push(Edit::Remove { target });
                }
            }
        };
        for space in &drawing.spaces {
            note(&id::space(space.id), &space.elements, true);
        }
        let mut definitions = id::DefinitionNamer::default();
        for symbol in &drawing.symbols {
            note(&definitions.next(symbol), &symbol.space.elements, false);
        }
        let added = usize::from(line.is_some());
        if let Some((place, line)) = line {
            edits.push(Edit::Add {
                place,
                element: Box::new(Placed::Model(line)),
            });
        }
        let mut updated = file.clone();
        let mut budget = Budget::default();
        let mut diag = Diagnostics::new();
        let cancel = CancelToken::new();
        let changes = Changes {
            edits: edits.clone(),
            document: Some(Hash::document(&src)),
        };
        let result = updated.apply(&changes, &mut Cx::new(&mut budget, &mut diag, &cancel));
        let diagnostics: Vec<_> = diag.iter().collect();
        let (applied, _) = result.unwrap_or_else(|_| panic!("{}: {diagnostics:?}", path.display()));
        assert_eq!(applied.len(), edits.len());
        let mut out = Vec::new();
        Jww::write(
            &updated,
            &mut out,
            &mut Cx::new(&mut budget, &mut diag, &cancel),
        )
        .unwrap();
        let reread = testkit::read::<Jww>(&out).0.unwrap();
        let after = lift(&reread);
        let text = |shape: &Shape<ModelPoint>| matches!(shape, Shape::Text(_));
        let model_texts = drawing
            .spaces
            .iter()
            .flat_map(|space| &space.elements)
            .filter(|element| text(&element.shape))
            .count();
        assert_eq!(model_texts, removed);
        assert_eq!(
            count(&after, text),
            count(&drawing, text) - removed,
            "{}",
            path.display()
        );
        assert_eq!(
            count(&after, shaped),
            count(&drawing, shaped) + added,
            "{}",
            path.display()
        );
        written += edits.len();
    }
    if !samples().is_empty() {
        assert!(written > 0);
    }
}
