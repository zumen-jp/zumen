//! 要素を複写し（`Edit::Copy`）、写しが元の値と拡張を保つことを再読で確かめる（#889）。
#![cfg(feature = "drawing")]

use std::path::Path;
use zumen_acad::{Dwg, Dxf, DxfFile};
use zumen_base::{Budget, CancelToken, Cx, Diagnostics, codes};
use zumen_drawing::{
    Affine, Applied, Changes, Drawing2D, Edit, Element, Hash, ModelPoint, Seen, Shape, id,
    transform,
};
use zumen_sdk::{Apply, Codec, Lift, testkit};

/// 写す図形を持つ DXF。LINE（1A）は、線種の倍率・透過性・色と、文字・整数・動く点の XDATA を持つ。INSERT（1B）は
/// 属性（1C）を持つ。LINE（1E）はリアクターを、LINE（1F）は XDATA のハンドルを持つ
#[cfg(test)]
fn source() -> Vec<u8> {
    let tags: &[(i16, &str)] = &[
        (0, "SECTION"),
        (2, "HEADER"),
        (9, "$ACADVER"),
        (1, "AC1015"),
        (9, "$INSUNITS"),
        (70, "4"),
        (9, "$HANDSEED"),
        (5, "80"),
        (0, "ENDSEC"),
        (0, "SECTION"),
        (2, "BLOCKS"),
        (0, "BLOCK"),
        (5, "30"),
        (8, "0"),
        (2, "B"),
        (70, "2"),
        (10, "0.0"),
        (20, "0.0"),
        (30, "0.0"),
        (0, "ENDBLK"),
        (5, "31"),
        (8, "0"),
        (0, "ENDSEC"),
        (0, "SECTION"),
        (2, "ENTITIES"),
        (0, "LINE"),
        (5, "1A"),
        (8, "0"),
        (62, "1"),
        (48, "2.5"),
        (440, "33554560"),
        (10, "0.0"),
        (20, "0.0"),
        (30, "0.0"),
        (11, "4.0"),
        (21, "3.0"),
        (31, "0.0"),
        (1001, "ZUMEN"),
        (1000, "abc"),
        (1071, "42"),
        (1011, "1.0"),
        (1021, "2.0"),
        (1031, "0.0"),
        (0, "INSERT"),
        (5, "1B"),
        (8, "0"),
        (66, "1"),
        (2, "B"),
        (10, "5.0"),
        (20, "5.0"),
        (30, "0.0"),
        (0, "ATTRIB"),
        (5, "1C"),
        (330, "1B"),
        (8, "0"),
        (10, "5.0"),
        (20, "6.0"),
        (30, "0.0"),
        (40, "1.0"),
        (1, "V"),
        (2, "TAG"),
        (70, "0"),
        (0, "SEQEND"),
        (5, "1D"),
        (330, "1B"),
        (8, "0"),
        (0, "LINE"),
        (5, "1E"),
        (102, "{ACAD_REACTORS"),
        (330, "1A"),
        (102, "}"),
        (8, "0"),
        (10, "0.0"),
        (20, "0.0"),
        (30, "0.0"),
        (11, "1.0"),
        (21, "0.0"),
        (31, "0.0"),
        (0, "LINE"),
        (5, "1F"),
        (8, "0"),
        (10, "0.0"),
        (20, "0.0"),
        (30, "0.0"),
        (11, "0.0"),
        (21, "1.0"),
        (31, "0.0"),
        (1001, "ZUMEN"),
        (1005, "1A"),
        (0, "ENDSEC"),
        (0, "EOF"),
    ];
    tags.iter()
        .map(|(code, value)| format!("{code:>3}\n{value}\n"))
        .collect::<String>()
        .into_bytes()
}

#[cfg(test)]
fn cx_run<T>(f: impl FnOnce(&mut Cx<'_>) -> T) -> (T, Diagnostics) {
    let mut diagnostics = Diagnostics::new();
    let mut budget = Budget::default();
    let cancel = CancelToken::new();
    let value = f(&mut Cx::new(&mut budget, &mut diagnostics, &cancel));
    (value, diagnostics)
}

/// 要素の ID・ハッシュ・要素
#[cfg(test)]
fn elements(drawing: &Drawing2D) -> Vec<(String, Hash, Element<ModelPoint>)> {
    let mut found = Vec::new();
    id::each_element(drawing, |_, name, element| {
        if let id::Found::Model(element) = element {
            found.push((name, Hash::element(element), element.clone()));
        }
        Ok::<_, ()>(())
    })
    .unwrap();
    found
}

/// 要素 `id` の拡張の、名前と値
#[cfg(test)]
fn extensions(drawing: &Drawing2D, id: &str) -> Vec<String> {
    drawing
        .extensions
        .iter()
        .filter(|extension| extension.target == zumen_drawing::Target::Element(id.to_owned()))
        .map(|extension| format!("{} {:?}", extension.name, extension.fields))
        .collect()
}

/// 図形の種類が `kind` のレコードの、ハンドル（5）を除く値
#[cfg(test)]
fn records(file: &DxfFile, kind: &[u8]) -> Vec<Vec<(i16, String)>> {
    file.records()
        .iter()
        .filter(|record| file.kind(record) == kind)
        .map(|record| {
            record
                .tags()
                .iter()
                .filter(|tag| tag.code() != 5)
                .map(|tag| {
                    let value = String::from_utf8_lossy(file.value(tag)).into_owned();
                    (tag.code(), value)
                })
                .collect()
        })
        .collect()
}

const SHIFT: Affine = Affine {
    m: [1.0, 0.0, 10.0, 0.0, 1.0, 20.0],
};

/// 要素 `name` を `transform` で写す変更を当て、結果と読み直した図面を返す
#[cfg(test)]
fn copy<C: Codec>(
    src: &[u8],
    names: &[&str],
    transform: Affine,
) -> (
    Result<Vec<Applied>, Diagnostics>,
    Drawing2D,
    Drawing2D,
    Vec<u8>,
)
where
    C::File: Lift<Drawing2D> + Apply<Changes, Output = Vec<Applied>>,
{
    let mut file = testkit::read::<C>(src).0.unwrap();
    let before = cx_run(|cx| file.lift(cx)).0.unwrap();
    let found = elements(&before);
    let edits = names
        .iter()
        .map(|name| {
            let (id, hash, _) = found.iter().find(|(id, ..)| id == name).unwrap();
            Edit::Copy {
                target: Seen {
                    id: id.clone(),
                    hash: *hash,
                },
                transform,
            }
        })
        .collect();
    let (results, diagnostics) = cx_run(|cx| {
        file.apply(
            &Changes {
                document: None,
                edits,
            },
            cx,
        )
    });
    let out = testkit::write::<C>(&file).unwrap();
    let after = cx_run(|cx| testkit::read::<C>(&out).0.unwrap().lift(cx))
        .0
        .unwrap();
    (results.map_err(|_| diagnostics), before, after, out)
}

/// 属性の出所を除いた形。写しの属性は、新しい ATTRIB から読む
#[cfg(test)]
fn sourceless(shape: &Shape<ModelPoint>) -> Shape<ModelPoint> {
    let mut shape = shape.clone();
    if let Shape::Symbol(symbol) = &mut shape {
        for attribute in &mut symbol.attributes {
            attribute.source = zumen_base::Ref::default();
        }
    }
    shape
}

/// 写しを確かめる。元の要素は ID とハッシュを保ち、写しは元のレイヤとスタイルと、変換した形を持つ
#[cfg(test)]
fn check_copies(before: &Drawing2D, after: &Drawing2D, results: &[Applied], sources: &[&str]) {
    let was = elements(before);
    let now = elements(after);
    for (id, hash, _) in &was {
        assert!(now.iter().any(|(i, h, _)| i == id && h == hash), "{id}");
    }
    assert_eq!(now.len(), was.len() + results.len());
    for (result, source) in results.iter().zip(sources) {
        let (_, _, original) = was.iter().find(|(id, ..)| id == source).unwrap();
        let new_id = result.id.as_deref().unwrap();
        let (_, hash, copied) = now.iter().find(|(id, ..)| id == new_id).unwrap();
        assert_eq!(result.hash, Some(*hash));
        assert_eq!(
            extensions(after, new_id).len(),
            extensions(before, source).len()
        );
        assert_eq!(copied.layer, original.layer);
        assert_eq!(copied.style, original.style);
        assert_eq!(
            Some(sourceless(&copied.shape)),
            transform::transform(&original.shape, &SHIFT).map(|shape| sourceless(&shape))
        );
    }
}

#[test]
fn copies_keep_style_extensions_and_attributes_in_dxf() {
    let src = source();
    let names: Vec<String> = {
        let file = testkit::read::<Dxf>(&src).0.unwrap();
        let drawing = cx_run(|cx| file.lift(cx)).0.unwrap();
        elements(&drawing)
            .into_iter()
            .take(2)
            .map(|(id, ..)| id)
            .collect()
    };
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    let (results, before, after, out) = copy::<Dxf>(&src, &names, SHIFT);
    let results = results.unwrap_or_else(|diagnostics| panic!("{diagnostics:?}"));
    check_copies(&before, &after, &results, &names);
    let line = results[0].id.as_deref().unwrap();
    // XDATA は、文字・整数を保ち、動く点を同じ変換で写す
    let original = extensions(&before, names[0]);
    let copied = extensions(&after, line);
    assert_eq!(copied.len(), original.len());
    let file = testkit::read::<Dxf>(&out).0.unwrap();
    let lines = records(&file, b"LINE");
    let shifted = |code: i16, value: &str| match code {
        10 | 11 | 1011 => format!("{:?}", value.parse::<f64>().unwrap() + 10.0),
        20 | 21 | 1021 => format!("{:?}", value.parse::<f64>().unwrap() + 20.0),
        _ => value.to_owned(),
    };
    let expected: Vec<(i16, String)> = lines[0]
        .iter()
        .map(|(code, value)| (*code, shifted(*code, value)))
        .collect();
    let number = |values: &[(i16, String)]| -> Vec<(i16, String)> {
        values
            .iter()
            .map(|(code, value)| match value.parse::<f64>() {
                Ok(n) if value.contains('.') => (*code, format!("{n:?}")),
                _ => (*code, value.clone()),
            })
            .collect()
    };
    let written = lines.last().unwrap();
    assert_eq!(number(written), number(&expected));
    // 属性は続く ATTRIB と SEQEND として写す
    assert_eq!(records(&file, b"ATTRIB").len(), 2);
    assert_eq!(records(&file, b"SEQEND").len(), 2);
    // 元と同じ変更の一覧で読み直すと、写しの写しも作れる
    let (again, ..) = copy::<Dxf>(&out, &[line], SHIFT);
    assert!(again.is_ok());
}

#[test]
fn copies_with_references_are_refused_whole() {
    let src = source();
    let file = testkit::read::<Dxf>(&src).0.unwrap();
    let drawing = cx_run(|cx| file.lift(cx)).0.unwrap();
    let names: Vec<String> = elements(&drawing).into_iter().map(|(id, ..)| id).collect();
    for (at, value) in [(2, "group"), (3, "xdata_handle")] {
        let (results, ..) = copy::<Dxf>(&src, &[&names[0], &names[at]], SHIFT);
        let diagnostics = results.unwrap_err();
        let found = diagnostics
            .iter()
            .find(|d| d.code() == codes::CHANGE_COPY_UNSUPPORTED)
            .unwrap_or_else(|| panic!("{diagnostics:?}"));
        assert_eq!(found.param("field"), Some(&"reference".into()));
        assert_eq!(found.param("value"), Some(&value.into()));
        assert_eq!(found.param("edit"), Some(&1u64.into()));
    }
    // 拒んだ変更の一覧は、図面を変えない
    let mut file = testkit::read::<Dxf>(&src).0.unwrap();
    let names: Vec<_> = elements(&drawing)
        .into_iter()
        .map(|(id, hash, _)| Seen { id, hash })
        .collect();
    let (results, _) = cx_run(|cx| {
        file.apply(
            &Changes {
                document: None,
                edits: vec![
                    Edit::Copy {
                        target: names[0].clone(),
                        transform: SHIFT,
                    },
                    Edit::Copy {
                        target: names[2].clone(),
                        transform: SHIFT,
                    },
                ],
            },
            cx,
        )
    });
    assert!(results.is_err());
    assert_eq!(testkit::write::<Dxf>(&file).unwrap(), src);
}

#[test]
fn copies_lines_in_public_dwg_versions() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../samples/dwg");
    for version in ["r2000", "r2004", "r2018"] {
        let src = std::fs::read(root.join(format!("zumen-layout-{version}.dwg"))).unwrap();
        let file = testkit::read::<Dwg>(&src).0.unwrap();
        let drawing = cx_run(|cx| file.lift(cx)).0.unwrap();
        let found = elements(&drawing);
        let names: Vec<&str> = found
            .iter()
            .filter(|(_, _, element)| element.source.key().is_some())
            .filter(|(id, ..)| id.starts_with("space/0/"))
            .filter(|(.., element)| {
                matches!(element.shape, Shape::Line { .. } | Shape::Circle { .. })
            })
            .map(|(id, ..)| id.as_str())
            .take(3)
            .collect();
        assert!(!names.is_empty(), "{version}");
        let (results, before, after, _) = copy::<Dwg>(&src, &names, SHIFT);
        let results = results.unwrap_or_else(|diagnostics| panic!("{version}: {diagnostics:?}"));
        check_copies(&before, &after, &results, &names);
    }
}

#[test]
fn stopped_copies_leave_the_drawing_unchanged() {
    use zumen_base::{Kind, Limit, Limits};
    let src = source();
    let run = |limit: Option<u64>| {
        let mut file = testkit::read::<Dxf>(&src).0.unwrap();
        let drawing = cx_run(|cx| file.lift(cx)).0.unwrap();
        let edits = elements(&drawing)
            .into_iter()
            .take(2)
            .map(|(id, hash, _)| Edit::Copy {
                target: Seen { id, hash },
                transform: SHIFT,
            })
            .collect();
        let limits = match limit {
            Some(limit) => Limits::default().with(Kind::Work, Limit::Finite(limit)),
            None => Limits::default(),
        };
        let mut budget = Budget::new(limits);
        let mut diagnostics = Diagnostics::new();
        let result = file.apply(
            &Changes {
                document: None,
                edits,
            },
            &mut Cx::new(&mut budget, &mut diagnostics, &CancelToken::new()),
        );
        (
            result.is_ok(),
            budget.used(Kind::Work),
            testkit::write::<Dxf>(&file).unwrap(),
        )
    };
    let (done, used, _) = run(None);
    assert!(done);
    // 止まる所を変えても、途中まで当てた図面を残さない
    for limit in [used / 4, used / 2, used - 1] {
        let (done, _, written) = run(Some(limit));
        assert!(!done, "{limit}");
        assert_eq!(written, src, "{limit}");
    }
}
