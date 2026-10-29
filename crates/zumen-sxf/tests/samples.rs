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
        let mut count = 0;
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
                count += 1;
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
        assert!(count > 0, "{}", path.display());
    }
}
