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

#[test]
fn copies_keep_lineweight_rgb_transparency_and_xdata_in_dwg_and_refuse_r2000() {
    use zumen_acad::{AcadLive, CopyFrom, DwgFile};
    use zumen_drawing::{Color, Inherit, LineWeight, PaperLength, Restyle};

    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../samples/dwg");
    for version in ["r2004", "r2018"] {
        let src = std::fs::read(root.join(format!("zumen-layout-{version}.dwg"))).unwrap();
        let mut file = testkit::read::<Dwg>(&src).0.unwrap();
        let drawing = cx_run(|cx| file.lift(cx)).0.unwrap();
        let (id, hash, _) = elements(&drawing)
            .into_iter()
            .find(|(id, _, el)| {
                id.starts_with("space/0/")
                    && el.source.key().is_some()
                    && matches!(el.shape, Shape::Polyline(_) | Shape::Circle { .. })
            })
            .unwrap();
        let restyled = cx_run(|cx| {
            file.apply(
                &Changes {
                    document: None,
                    edits: vec![Edit::Restyle {
                        target: Seen {
                            id: id.clone(),
                            hash,
                        },
                        style: Restyle {
                            layer: None,
                            color: Some(Inherit::Own(Color {
                                code: None,
                                rgb: Some([10, 20, 30]),
                            })),
                            line_type: None,
                            line_weight: Some(Inherit::Own(LineWeight::Width(PaperLength(0.35)))),
                            line_type_scale: Some(2.5),
                        },
                    }],
                },
                cx,
            )
        })
        .0
        .unwrap();
        let styled_bytes = testkit::write::<Dwg>(&file).unwrap();
        let (results, before, after, _) =
            copy::<Dwg>(&styled_bytes, &[restyled[0].id.as_deref().unwrap()], SHIFT);
        let results = results.unwrap_or_else(|diag| panic!("{version}: {diag:?}"));
        check_copies(
            &before,
            &after,
            &results,
            &[restyled[0].id.as_deref().unwrap()],
        );
        let copied_id = results[0].id.as_deref().unwrap();
        let (_, _, copied_el) = elements(&after)
            .into_iter()
            .find(|(i, ..)| i == copied_id)
            .unwrap();
        assert_eq!(
            copied_el.style.line_weight,
            Some(Inherit::Own(LineWeight::Width(PaperLength(0.35))))
        );
        assert_eq!(
            copied_el.style.color,
            Inherit::Own(Color {
                code: None,
                rgb: Some([10, 20, 30])
            })
        );
        assert_eq!(copied_el.style.line_type_scale, 2.5);

        // R2000 DWG への別文書複写では、RGB（420）・透過性（440）を落とさずバッチ全体を拒む
        let r2000_bytes = std::fs::read(root.join("zumen-layout-r2000.dwg")).unwrap();
        let mut dst: DwgFile = testkit::read::<Dwg>(&r2000_bytes).0.unwrap();
        let src_file: DwgFile = testkit::read::<Dwg>(&styled_bytes).0.unwrap();
        let src_drawing = cx_run(|cx| src_file.lift(cx)).0.unwrap();
        let (src_id, src_hash, _) = elements(&src_drawing)
            .into_iter()
            .find(|(i, ..)| Some(i.as_str()) == restyled[0].id.as_deref())
            .unwrap();
        let (res, diag) = cx_run(|cx| {
            dst.apply_with_copies(
                &Changes {
                    document: None,
                    edits: vec![Edit::Copy {
                        target: Seen {
                            id: src_id,
                            hash: src_hash,
                        },
                        transform: SHIFT,
                    }],
                },
                &[CopyFrom {
                    place: None,
                    from: Some(&src_file),
                }],
                cx,
            )
        });
        assert!(res.is_err(), "{version}");
        assert!(
            diag.iter()
                .any(|d| d.code() == codes::CHANGE_COPY_UNSUPPORTED),
            "{diag:?}"
        );
        assert_eq!(testkit::write::<Dwg>(&dst).unwrap(), r2000_bytes);

        // AcadLive での複数要素 copy バッチの 1 回 undo / 1 回 redo
        let mut live = cx_run(|cx| AcadLive::open_dwg(&styled_bytes, cx))
            .0
            .unwrap();
        let initial_drawing = cx_run(|cx| live.drawing(cx)).0.unwrap();
        let initial_saved = cx_run(|cx| live.save(cx)).0.unwrap();
        let targets: Vec<_> = elements(&initial_drawing)
            .into_iter()
            .filter(|(i, _, el)| {
                i.starts_with("space/0/")
                    && el.source.key().is_some()
                    && matches!(el.shape, Shape::Polyline(_) | Shape::Circle { .. })
            })
            .take(2)
            .map(|(i, h, _)| Edit::Copy {
                target: Seen { id: i, hash: h },
                transform: SHIFT,
            })
            .collect();
        assert_eq!(targets.len(), 2);
        let applied = cx_run(|cx| {
            live.apply(
                &Changes {
                    document: None,
                    edits: targets,
                },
                cx,
            )
        })
        .0
        .unwrap();
        assert_eq!(applied.len(), 2);
        assert_eq!(live.undo_len(), 1);
        let after_copy = cx_run(|cx| live.drawing(cx)).0.unwrap();
        assert_eq!(
            elements(&after_copy).len(),
            elements(&initial_drawing).len() + 2
        );
        assert!(cx_run(|cx| live.undo(cx)).0.unwrap());
        assert_eq!(live.undo_len(), 0);
        assert_eq!(live.redo_len(), 1);
        assert_eq!(cx_run(|cx| live.save(cx)).0.unwrap(), initial_saved);
        assert!(cx_run(|cx| live.redo(cx)).0.unwrap());
        assert_eq!(
            elements(&cx_run(|cx| live.drawing(cx)).0.unwrap()).len(),
            elements(&after_copy).len()
        );
    }
}

#[test]
fn copies_dimensions_arc_dimensions_tables_and_multileaders_in_dxf() {
    // DIMENSION（*D1）・ARC_DIMENSION（*D2）・ACAD_TABLE（*T1・343 ハンドルつき）・MULTILEADER を合成 DXF で一括複写し、
    // 新しい匿名ブロックが割当てられて元と写しが独立に動くことを確かめる
    let mut table_src = String::new();
    for (c, v) in [
        (0, "SECTION"),
        (2, "HEADER"),
        (9, "$ACADVER"),
        (1, "AC1024"),
        (9, "$INSUNITS"),
        (70, "4"),
        (9, "$HANDSEED"),
        (5, "600"),
        (0, "ENDSEC"),
        (0, "SECTION"),
        (2, "TABLES"),
        (0, "TABLE"),
        (2, "BLOCK_RECORD"),
        (5, "1"),
        (100, "AcDbSymbolTable"),
        (70, "4"),
        (0, "BLOCK_RECORD"),
        (5, "1F"),
        (330, "1"),
        (100, "AcDbSymbolTableRecord"),
        (100, "AcDbBlockTableRecord"),
        (2, "*Model_Space"),
        (0, "BLOCK_RECORD"),
        (5, "41"),
        (330, "1"),
        (100, "AcDbSymbolTableRecord"),
        (100, "AcDbBlockTableRecord"),
        (2, "*D1"),
        (0, "BLOCK_RECORD"),
        (5, "42"),
        (330, "1"),
        (100, "AcDbSymbolTableRecord"),
        (100, "AcDbBlockTableRecord"),
        (2, "*D2"),
        (0, "BLOCK_RECORD"),
        (5, "50"),
        (330, "1"),
        (100, "AcDbSymbolTableRecord"),
        (100, "AcDbBlockTableRecord"),
        (2, "*T1"),
        (0, "ENDTAB"),
        (0, "ENDSEC"),
        (0, "SECTION"),
        (2, "BLOCKS"),
        (0, "BLOCK"),
        (5, "20"),
        (330, "1F"),
        (100, "AcDbEntity"),
        (8, "0"),
        (100, "AcDbBlockBegin"),
        (2, "*Model_Space"),
        (70, "0"),
        (10, "0.0"),
        (20, "0.0"),
        (30, "0.0"),
        (0, "ENDBLK"),
        (5, "21"),
        (330, "1F"),
        (100, "AcDbEntity"),
        (8, "0"),
        (100, "AcDbBlockEnd"),
        (0, "BLOCK"),
        (5, "43"),
        (330, "41"),
        (100, "AcDbEntity"),
        (8, "0"),
        (100, "AcDbBlockBegin"),
        (2, "*D1"),
        (70, "1"),
        (10, "0.0"),
        (20, "0.0"),
        (30, "0.0"),
        (0, "LINE"),
        (5, "44"),
        (330, "41"),
        (100, "AcDbEntity"),
        (8, "0"),
        (100, "AcDbLine"),
        (10, "0.0"),
        (20, "10.0"),
        (30, "0.0"),
        (11, "20.0"),
        (21, "10.0"),
        (31, "0.0"),
        (0, "ENDBLK"),
        (5, "45"),
        (330, "41"),
        (100, "AcDbEntity"),
        (8, "0"),
        (100, "AcDbBlockEnd"),
        (0, "BLOCK"),
        (5, "46"),
        (330, "42"),
        (100, "AcDbEntity"),
        (8, "0"),
        (100, "AcDbBlockBegin"),
        (2, "*D2"),
        (70, "1"),
        (10, "0.0"),
        (20, "0.0"),
        (30, "0.0"),
        (0, "ARC"),
        (5, "47"),
        (330, "42"),
        (100, "AcDbEntity"),
        (8, "0"),
        (100, "AcDbCircle"),
        (10, "0.0"),
        (20, "0.0"),
        (30, "0.0"),
        (40, "15.0"),
        (100, "AcDbArc"),
        (50, "0.0"),
        (51, "90.0"),
        (0, "ENDBLK"),
        (5, "48"),
        (330, "42"),
        (100, "AcDbEntity"),
        (8, "0"),
        (100, "AcDbBlockEnd"),
        (0, "BLOCK"),
        (5, "51"),
        (330, "50"),
        (100, "AcDbEntity"),
        (8, "0"),
        (100, "AcDbBlockBegin"),
        (2, "*T1"),
        (70, "1"),
        (10, "0.0"),
        (20, "0.0"),
        (30, "0.0"),
        (0, "LINE"),
        (5, "52"),
        (330, "50"),
        (100, "AcDbEntity"),
        (8, "0"),
        (100, "AcDbLine"),
        (10, "0.0"),
        (20, "0.0"),
        (30, "0.0"),
        (11, "20.0"),
        (21, "0.0"),
        (31, "0.0"),
        (0, "ENDBLK"),
        (5, "53"),
        (330, "50"),
        (100, "AcDbEntity"),
        (8, "0"),
        (100, "AcDbBlockEnd"),
        (0, "ENDSEC"),
        (0, "SECTION"),
        (2, "ENTITIES"),
        (0, "DIMENSION"),
        (5, "298"),
        (330, "1F"),
        (100, "AcDbEntity"),
        (8, "0"),
        (100, "AcDbDimension"),
        (2, "*D1"),
        (10, "20.0"),
        (20, "10.0"),
        (30, "0.0"),
        (11, "10.0"),
        (21, "10.0"),
        (31, "0.0"),
        (70, "32"),
        (71, "5"),
        (42, "20.0"),
        (3, "Standard"),
        (100, "AcDbAlignedDimension"),
        (13, "0.0"),
        (23, "0.0"),
        (33, "0.0"),
        (14, "20.0"),
        (24, "0.0"),
        (34, "0.0"),
        (50, "0.0"),
        (100, "AcDbRotatedDimension"),
        (1001, "ACAD"),
        (1000, "DSTYLE"),
        (1002, "{"),
        (1070, "140"),
        (1040, "0.75"),
        (1002, "}"),
        (0, "ARC_DIMENSION"),
        (5, "299"),
        (330, "1F"),
        (100, "AcDbEntity"),
        (8, "0"),
        (100, "AcDbDimension"),
        (2, "*D2"),
        (10, "10.606601717798213"),
        (20, "10.606601717798213"),
        (30, "0.0"),
        (11, "10.606601717798213"),
        (21, "10.606601717798213"),
        (31, "0.0"),
        (70, "37"),
        (71, "5"),
        (42, "15.707963267948966"),
        (3, "Standard"),
        (100, "AcDbArcDimension"),
        (13, "10.0"),
        (23, "0.0"),
        (33, "0.0"),
        (14, "0.0"),
        (24, "10.0"),
        (34, "0.0"),
        (15, "0.0"),
        (25, "0.0"),
        (35, "0.0"),
        (40, "0.0"),
        (41, "1.5707963267948966"),
        (70, "0"),
        (71, "0"),
        (0, "ACAD_TABLE"),
        (5, "300"),
        (330, "1F"),
        (100, "AcDbEntity"),
        (8, "0"),
        (100, "AcDbBlockReference"),
        (2, "*T1"),
        (10, "10.0"),
        (20, "50.0"),
        (30, "0.0"),
        (100, "AcDbTable"),
        (280, "0"),
        (342, "400"),
        (343, "50"),
        (11, "1.0"),
        (21, "0.0"),
        (31, "0.0"),
        (90, "22"),
        (91, "1"),
        (92, "1"),
        (93, "0"),
        (94, "0"),
        (95, "0"),
        (96, "0"),
        (141, "8.0"),
        (142, "20.0"),
        (171, "1"),
        (172, "0"),
        (173, "0"),
        (174, "0"),
        (175, "1"),
        (176, "1"),
        (91, "0"),
        (178, "0"),
        (145, "0.0"),
        (140, "3.0"),
        (170, "5"),
        (301, "CELL_VALUE"),
        (90, "4"),
        (93, "0"),
        (1, "A1"),
        (94, "0"),
        (300, ""),
        (302, "A1"),
        (304, "ACVALUE_END"),
        (0, "MULTILEADER"),
        (5, "301"),
        (330, "1F"),
        (100, "AcDbEntity"),
        (8, "0"),
        (100, "AcDbMLeader"),
        (300, "CONTEXT_DATA{"),
        (40, "1.0"),
        (10, "5.0"),
        (20, "5.0"),
        (30, "0.0"),
        (41, "2.5"),
        (140, "2.5"),
        (145, "1.0"),
        (174, "1"),
        (175, "1"),
        (176, "0"),
        (177, "0"),
        (290, "1"),
        (304, "Note"),
        (11, "0.0"),
        (21, "0.0"),
        (31, "1.0"),
        (12, "5.0"),
        (22, "5.0"),
        (32, "0.0"),
        (13, "1.0"),
        (23, "0.0"),
        (33, "0.0"),
        (42, "0.0"),
        (43, "0.0"),
        (44, "0.0"),
        (45, "1.0"),
        (170, "1"),
        (90, "-1073741824"),
        (171, "1"),
        (172, "5"),
        (91, "-1073741824"),
        (141, "0.0"),
        (92, "0"),
        (291, "0"),
        (292, "0"),
        (173, "0"),
        (293, "0"),
        (142, "0.0"),
        (143, "0.0"),
        (294, "0"),
        (295, "0"),
        (296, "0"),
        (110, "0.0"),
        (120, "0.0"),
        (130, "0.0"),
        (111, "1.0"),
        (121, "0.0"),
        (131, "0.0"),
        (112, "0.0"),
        (122, "1.0"),
        (132, "0.0"),
        (297, "0"),
        (302, "LEADER{"),
        (290, "1"),
        (291, "1"),
        (10, "5.0"),
        (20, "5.0"),
        (30, "0.0"),
        (11, "1.0"),
        (21, "0.0"),
        (31, "0.0"),
        (90, "0"),
        (40, "2.0"),
        (304, "LEADER_LINE{"),
        (10, "0.0"),
        (20, "0.0"),
        (30, "0.0"),
        (91, "0"),
        (305, "}"),
        (271, "0"),
        (303, "}"),
        (272, "9"),
        (273, "9"),
        (301, "}"),
        (340, "401"),
        (90, "0"),
        (170, "1"),
        (91, "-1073741824"),
        (341, "0"),
        (171, "-2"),
        (290, "1"),
        (291, "1"),
        (41, "0.0"),
        (42, "2.5"),
        (172, "2"),
        (173, "1"),
        (95, "1"),
        (174, "1"),
        (175, "1"),
        (92, "-1073741824"),
        (292, "0"),
        (93, "-1073741824"),
        (10, "1.0"),
        (20, "1.0"),
        (30, "0.0"),
        (43, "0.0"),
        (176, "0"),
        (293, "0"),
        (294, "0"),
        (178, "0"),
        (179, "1"),
        (45, "1.0"),
        (271, "0"),
        (272, "9"),
        (273, "9"),
        (0, "ENDSEC"),
        (0, "SECTION"),
        (2, "OBJECTS"),
        (0, "TABLESTYLE"),
        (5, "400"),
        (330, "0"),
        (100, "AcDbTableStyle"),
        (280, "0"),
        (3, "Standard"),
        (70, "0"),
        (71, "0"),
        (40, "1.5"),
        (41, "1.5"),
        (280, "0"),
        (281, "0"),
        (0, "MLEADERSTYLE"),
        (5, "401"),
        (330, "0"),
        (100, "AcDbMLeaderStyle"),
        (179, "2"),
        (170, "2"),
        (171, "1"),
        (172, "0"),
        (90, "2"),
        (40, "0.0"),
        (41, "0.0"),
        (290, "1"),
        (42, "2.0"),
        (291, "1"),
        (43, "4.0"),
        (3, "Standard"),
        (44, "2.5"),
        (300, ""),
        (174, "1"),
        (178, "1"),
        (175, "1"),
        (176, "0"),
        (45, "2.5"),
        (292, "0"),
        (46, "1.0"),
        (92, "-1073741824"),
        (293, "0"),
        (294, "0"),
        (47, "0.0"),
        (49, "0.0"),
        (140, "1.0"),
        (295, "0"),
        (142, "1.0"),
        (296, "0"),
        (0, "ENDSEC"),
        (0, "EOF"),
    ] {
        table_src.push_str(&format!("{c}\n{v}\n"));
    }
    let table_bytes = table_src.into_bytes();
    let (res, before_tm, after_tm, out_tm) = copy::<Dxf>(
        &table_bytes,
        &[
            "space/0/h298",
            "space/0/h299",
            "space/0/h300",
            "space/0/h301",
        ],
        SHIFT,
    );
    let res = res.unwrap_or_else(|diag| panic!("{diag:?}"));
    assert_eq!(res.len(), 4);
    let copy_id = res[0].id.as_deref().unwrap();
    let override_basis = |drawing: &Drawing2D, target: &str| {
        drawing
            .dimension_display
            .iter()
            .find_map(|r| match &r.target {
                zumen_drawing::DimensionDisplayTarget::Element { id, basis } if id == target => {
                    assert_eq!(r.values.text_height, Some(0.75));
                    Some(*basis)
                }
                _ => None,
            })
            .unwrap()
    };
    let original_basis = override_basis(&before_tm, "space/0/h298");
    assert_eq!(override_basis(&after_tm, "space/0/h298"), original_basis);
    assert_eq!(override_basis(&after_tm, copy_id), res[0].hash.unwrap());
    let before_els = elements(&before_tm);
    let after_els = elements(&after_tm);
    for (id, hash, _) in &before_els {
        assert!(
            after_els.iter().any(|(i, h, _)| i == id && h == hash),
            "{id}"
        );
    }
    let out_file = testkit::read::<Dxf>(&out_tm).0.unwrap();
    assert_eq!(records(&out_file, b"DIMENSION").len(), 2);
    assert_eq!(records(&out_file, b"ARC_DIMENSION").len(), 2);
    assert_eq!(records(&out_file, b"ACAD_TABLE").len(), 2);
    assert_eq!(records(&out_file, b"MULTILEADER").len(), 2);
    // 写した DIMENSION・ARC_DIMENSION・ACAD_TABLE を動かしても元は不変（*D/*T ブロックと 343 ハンドルを共有しない）
    let mut tm_move = out_file;
    cx_run(|cx| {
        tm_move.apply(
            &Changes {
                document: None,
                edits: (0..3)
                    .map(|idx| Edit::Transform {
                        target: Seen {
                            id: res[idx].id.clone().unwrap(),
                            hash: res[idx].hash.unwrap(),
                        },
                        transform: SHIFT,
                    })
                    .collect(),
            },
            cx,
        )
    })
    .0
    .unwrap();
    let after_tm_move = cx_run(|cx| {
        testkit::read::<Dxf>(&testkit::write::<Dxf>(&tm_move).unwrap())
            .0
            .unwrap()
            .lift(cx)
    })
    .0
    .unwrap();
    assert_eq!(
        override_basis(&after_tm_move, "space/0/h298"),
        original_basis
    );
    let moved = elements(&after_tm_move)
        .into_iter()
        .find(|e| e.0 == copy_id)
        .unwrap();
    assert_ne!(res[0].hash, Some(moved.1));
    assert_eq!(override_basis(&after_tm_move, copy_id), moved.1);
    for orig in &before_els[..3] {
        assert!(
            elements(&after_tm_move)
                .iter()
                .any(|(i, h, _)| i == &orig.0 && h == &orig.1),
            "original changed: {}",
            orig.0
        );
    }
}

#[test]
fn cross_document_copies_check_dimension_display_even_when_public_hashes_match() {
    use zumen_acad::CopyFrom;

    // B の寸法にはスタイルに加えて個別の精度があり、C は B を置く。
    let src = "0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1021\n9\n$INSUNITS\n70\n4\n9\n$HANDSEED\n5\n100\n0\nENDSEC\n\
0\nSECTION\n2\nTABLES\n0\nTABLE\n2\nDIMSTYLE\n0\nDIMSTYLE\n105\n27\n2\nStandard\n140\n0.25\n340\n28\n0\nENDTAB\n\
0\nTABLE\n2\nSTYLE\n0\nSTYLE\n5\n28\n2\nLabel\n3\nsimple.shx\n0\nENDTAB\n\
0\nTABLE\n2\nBLOCK_RECORD\n0\nBLOCK_RECORD\n5\n1F\n2\n*Model_Space\n0\nBLOCK_RECORD\n5\n2F\n2\nB\n0\nBLOCK_RECORD\n5\n3F\n2\nC\n0\nENDTAB\n0\nENDSEC\n\
0\nSECTION\n2\nBLOCKS\n0\nBLOCK\n5\n30\n330\n2F\n2\nB\n10\n0\n20\n0\n\
0\nDIMENSION\n5\n40\n330\n2F\n8\n0\n70\n0\n3\nStandard\n10\n12\n20\n6\n13\n0\n23\n0\n14\n12\n24\n0\n11\n6\n21\n6\n\
1001\nACAD\n1000\nDSTYLE\n1002\n{\n1070\n271\n1070\n2\n1002\n}\n0\nENDBLK\n5\n31\n330\n2F\n\
0\nBLOCK\n5\n32\n330\n3F\n2\nC\n10\n0\n20\n0\n0\nINSERT\n5\n41\n330\n3F\n8\n0\n2\nB\n10\n0\n20\n0\n0\nENDBLK\n5\n33\n330\n3F\n0\nENDSEC\n\
0\nSECTION\n2\nENTITIES\n0\nINSERT\n5\n42\n330\n1F\n8\n0\n2\nC\n10\n0\n20\n0\n\
0\nLEADER\n5\n43\n330\n1F\n8\n0\n76\n2\n10\n0\n20\n0\n10\n3\n20\n4\n0\nENDSEC\n0\nEOF\n";

    let file = testkit::read::<Dxf>(src.as_bytes()).0.unwrap();
    let drawing = cx_run(|cx| file.lift(cx)).0.unwrap();
    assert!(
        drawing
            .dimension_display
            .iter()
            .any(|r| r.values.linear_precision == Some(2))
    );
    for (id, change, field) in [
        ("space/0/h42", None, ""),
        ("space/0/h43", None, ""),
        ("space/0/h42", Some(("140\n0.25", "140\n0.75")), "symbol"),
        (
            "space/0/h42",
            Some(("271\n1070\n2", "271\n1070\n4")),
            "symbol",
        ),
        (
            "space/0/h43",
            Some(("140\n0.25", "140\n0.75")),
            "dimension_style",
        ),
        (
            "space/0/h43",
            Some(("simple.shx", "other.shx")),
            "dimension_style",
        ),
    ] {
        let bytes = change.map_or_else(|| src.to_owned(), |(old, new)| src.replace(old, new));
        let mut dst = testkit::read::<Dxf>(bytes.as_bytes()).0.unwrap();
        let dest = cx_run(|cx| dst.lift(cx)).0.unwrap();
        assert_eq!(
            Hash::dimension_style(&drawing.dimension_styles[0]),
            Hash::dimension_style(&dest.dimension_styles[0])
        );
        let (_, hash, _) = elements(&drawing).into_iter().find(|e| e.0 == id).unwrap();
        let (result, diagnostics) = cx_run(|cx| {
            dst.apply_with_copies(
                &Changes {
                    document: None,
                    edits: vec![Edit::Copy {
                        target: Seen {
                            id: id.to_owned(),
                            hash,
                        },
                        transform: SHIFT,
                    }],
                },
                &[CopyFrom {
                    place: None,
                    from: Some(&file),
                }],
                cx,
            )
        });
        if change.is_some() {
            assert!(result.is_err(), "{id} {change:?}");
            assert!(
                diagnostics
                    .iter()
                    .any(|d| d.code() == codes::CHANGE_COPY_UNSUPPORTED
                        && d.param("field") == Some(&field.into())
                        && d.param("value") == Some(&"conflict".into())),
                "{id} {change:?}: {diagnostics:?}"
            );
            assert_eq!(testkit::write::<Dxf>(&dst).unwrap(), bytes.as_bytes());
        } else {
            result.unwrap_or_else(|_| panic!("{id}: {diagnostics:?}"));
        }
    }
    // 以前の寸法スタイルのハッシュで端部を編集しても、今の文字高を保つ。
    // 読み取った時点の文書ハッシュも指定すれば、文字高だけの差も検出する。
    let newer = src.replace("140\n0.25", "140\n0.75");
    let mut dst = testkit::read::<Dxf>(newer.as_bytes()).0.unwrap();
    let was = &drawing.dimension_styles[0];
    let mut style = was.clone();
    style.size = 1.25;
    let mut changes = Changes {
        document: Some(Hash::document(src.as_bytes())),
        edits: vec![Edit::ReplaceDimensionStyle {
            target: Seen {
                id: id::dimension_style(was, 0),
                hash: Hash::dimension_style(was),
            },
            style: Box::new(style),
        }],
    };
    let (stale, diagnostics) = cx_run(|cx| dst.apply(&changes, cx));
    assert!(stale.is_err());
    assert!(
        diagnostics
            .iter()
            .any(|d| d.code() == codes::CHANGE_DOCUMENT_STALE)
    );
    assert_eq!(testkit::write::<Dxf>(&dst).unwrap(), newer.as_bytes());
    changes.document = None;
    let (result, diagnostics) = cx_run(|cx| dst.apply(&changes, cx));
    result.unwrap_or_else(|_| panic!("{diagnostics:?}"));
    let latest = cx_run(|cx| dst.lift(cx)).0.unwrap();
    assert_eq!(latest.dimension_styles[0].size, 1.25);
    assert!(latest.dimension_display.iter().any(|r|
        matches!(&r.target, zumen_drawing::DimensionDisplayTarget::Style(name) if name == "Standard") && r.values.text_height == Some(0.75)
    ));
}

#[test]
fn copies_across_places_and_documents_in_dxf() {
    use zumen_acad::CopyFrom;

    let src = source();
    // 1. 空間から記号定義への複写: 動く点の XDATA（1011）と自己参照サイクルを拒み、通常の LINE は通る
    let br_dxf = b"0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1015\n9\n$INSUNITS\n70\n4\n9\n$HANDSEED\n5\n80\n0\nENDSEC\n\
0\nSECTION\n2\nTABLES\n0\nTABLE\n2\nBLOCK_RECORD\n5\n1\n100\nAcDbSymbolTable\n70\n2\n\
0\nBLOCK_RECORD\n5\n1F\n330\n1\n100\nAcDbSymbolTableRecord\n100\nAcDbBlockTableRecord\n2\n*Model_Space\n\
0\nBLOCK_RECORD\n5\n2F\n330\n1\n100\nAcDbSymbolTableRecord\n100\nAcDbBlockTableRecord\n2\nB\n0\nENDTAB\n0\nENDSEC\n\
0\nSECTION\n2\nBLOCKS\n0\nBLOCK\n5\n20\n330\n1F\n100\nAcDbEntity\n8\n0\n100\nAcDbBlockBegin\n2\n*Model_Space\n70\n0\n10\n0.0\n20\n0.0\n30\n0.0\n0\nENDBLK\n5\n21\n330\n1F\n100\nAcDbEntity\n8\n0\n100\nAcDbBlockEnd\n\
0\nBLOCK\n5\n30\n330\n2F\n100\nAcDbEntity\n8\n0\n100\nAcDbBlockBegin\n2\nB\n70\n0\n10\n0.0\n20\n0.0\n30\n0.0\n0\nENDBLK\n5\n31\n330\n2F\n100\nAcDbEntity\n8\n0\n100\nAcDbBlockEnd\n0\nENDSEC\n\
0\nSECTION\n2\nENTITIES\n\
0\nLINE\n5\n1A\n330\n1F\n100\nAcDbEntity\n8\n0\n100\nAcDbLine\n10\n0.0\n20\n0.0\n30\n0.0\n11\n4.0\n21\n3.0\n31\n0.0\n1001\nZUMEN\n1011\n1.0\n1021\n2.0\n1031\n0.0\n\
0\nINSERT\n5\n1B\n330\n1F\n100\nAcDbEntity\n8\n0\n100\nAcDbBlockReference\n2\nB\n10\n5.0\n20\n5.0\n30\n0.0\n\
0\nLINE\n5\n1C\n330\n1F\n100\nAcDbEntity\n8\n0\n100\nAcDbLine\n10\n0.0\n20\n0.0\n30\n0.0\n11\n2.0\n21\n0.0\n31\n0.0\n\
0\nENDSEC\n0\nEOF\n";
    let mut br_file = testkit::read::<Dxf>(br_dxf).0.unwrap();
    let br_drawing = cx_run(|cx| br_file.lift(cx)).0.unwrap();
    let sym_place = id::definition(&br_drawing.symbols[0]);
    let br_els = elements(&br_drawing);
    let (bad_world, diag) = cx_run(|cx| {
        br_file.apply_with_copies(
            &Changes {
                document: None,
                edits: vec![Edit::Copy {
                    target: Seen {
                        id: br_els[0].0.clone(),
                        hash: br_els[0].1,
                    },
                    transform: SHIFT,
                }],
            },
            &[CopyFrom {
                place: Some(sym_place.as_str()),
                from: None,
            }],
            cx,
        )
    });
    assert!(bad_world.is_err());
    assert!(diag.iter().any(|d| {
        d.code() == codes::CHANGE_COPY_UNSUPPORTED
            && d.param("value") == Some(&"world_space".into())
    }));
    let (bad_cycle, diag) = cx_run(|cx| {
        br_file.apply_with_copies(
            &Changes {
                document: None,
                edits: vec![Edit::Copy {
                    target: Seen {
                        id: br_els[1].0.clone(),
                        hash: br_els[1].1,
                    },
                    transform: SHIFT,
                }],
            },
            &[CopyFrom {
                place: Some(sym_place.as_str()),
                from: None,
            }],
            cx,
        )
    });
    assert!(bad_cycle.is_err());
    assert!(
        diag.iter()
            .any(|d| d.code() == codes::CHANGE_SYMBOL_CYCLE_INVALID)
    );
    assert_eq!(testkit::write::<Dxf>(&br_file).unwrap(), br_dxf);
    // 通常の LINE（h1C）は space/0 から記号定義 B へ複写できる
    let ok_cross = cx_run(|cx| {
        br_file.apply_with_copies(
            &Changes {
                document: None,
                edits: vec![Edit::Copy {
                    target: Seen {
                        id: br_els[2].0.clone(),
                        hash: br_els[2].1,
                    },
                    transform: SHIFT,
                }],
            },
            &[CopyFrom {
                place: Some(sym_place.as_str()),
                from: None,
            }],
            cx,
        )
    })
    .0
    .unwrap();
    assert!(
        ok_cross[0]
            .id
            .as_deref()
            .unwrap()
            .starts_with(&format!("{sym_place}/"))
    );

    // 2. 別文書からの複写: APPID・表・ブロック定義が一致する文書へは通り、ブロックが欠ける文書へは全体を拒む
    let cross_bytes = String::from_utf8(src.clone())
        .unwrap()
        .replacen(
            "  0\nSECTION\n  2\nBLOCKS\n",
            "  0\nSECTION\n  2\nTABLES\n  0\nTABLE\n  2\nAPPID\n  0\nAPPID\n  5\n9\n  2\nZUMEN\n 70\n0\n  0\nENDTAB\n  0\nENDSEC\n  0\nSECTION\n  2\nBLOCKS\n",
            1,
        )
        .into_bytes();
    let src_file = testkit::read::<Dxf>(&cross_bytes).0.unwrap();
    let cross_drawing = cx_run(|cx| src_file.lift(cx)).0.unwrap();
    let els = elements(&cross_drawing);
    let mut matching_dst = testkit::read::<Dxf>(&cross_bytes).0.unwrap();
    let (applied_res, cross_diag) = cx_run(|cx| {
        matching_dst.apply_with_copies(
            &Changes {
                document: None,
                edits: vec![
                    Edit::Copy {
                        target: Seen {
                            id: els[0].0.clone(),
                            hash: els[0].1,
                        },
                        transform: SHIFT,
                    },
                    Edit::Copy {
                        target: Seen {
                            id: els[1].0.clone(),
                            hash: els[1].1,
                        },
                        transform: SHIFT,
                    },
                ],
            },
            &[
                CopyFrom {
                    place: None,
                    from: Some(&src_file),
                },
                CopyFrom {
                    place: None,
                    from: Some(&src_file),
                },
            ],
            cx,
        )
    });
    let applied = applied_res.unwrap_or_else(|_| panic!("{cross_diag:?}"));
    assert_eq!(applied.len(), 2);
    let dst_after = cx_run(|cx| {
        testkit::read::<Dxf>(&testkit::write::<Dxf>(&matching_dst).unwrap())
            .0
            .unwrap()
            .lift(cx)
    })
    .0
    .unwrap();
    check_copies(
        &cross_drawing,
        &dst_after,
        &applied,
        &[els[0].0.as_str(), els[1].0.as_str()],
    );

    // ブロック B を持たない文書へ INSERT（h1B）を別文書複写すると symbol missing で拒む
    let noblock_bytes = String::from_utf8(src.clone())
        .unwrap()
        .replace(
            "  0\nBLOCK\n  5\n30\n  8\n0\n  2\nB\n 70\n2\n 10\n0.0\n 20\n0.0\n 30\n0.0\n  0\nENDBLK\n  5\n31\n  8\n0\n",
            "",
        )
        .replace(
            "  0\nINSERT\n  5\n1B\n  8\n0\n 66\n1\n  2\nB\n 10\n5.0\n 20\n5.0\n 30\n0.0\n  0\nATTRIB\n  5\n1C\n330\n1B\n  8\n0\n 10\n5.0\n 20\n6.0\n 30\n0.0\n 40\n1.0\n  1\nV\n  2\nTAG\n 70\n0\n  0\nSEQEND\n  5\n1D\n330\n1B\n  8\n0\n",
            "",
        )
        .into_bytes();
    let mut noblock_dst = testkit::read::<Dxf>(&noblock_bytes).0.unwrap();
    let (bad_sym, diag) = cx_run(|cx| {
        noblock_dst.apply_with_copies(
            &Changes {
                document: None,
                edits: vec![Edit::Copy {
                    target: Seen {
                        id: els[1].0.clone(),
                        hash: els[1].1,
                    },
                    transform: SHIFT,
                }],
            },
            &[CopyFrom {
                place: None,
                from: Some(&src_file),
            }],
            cx,
        )
    });
    assert!(bad_sym.is_err());
    assert!(diag.iter().any(|d| {
        d.code() == codes::CHANGE_COPY_UNSUPPORTED && d.param("field") == Some(&"symbol".into())
    }));
    assert_eq!(testkit::write::<Dxf>(&noblock_dst).unwrap(), noblock_bytes);

    // 3. AcadLive では複数要素の copy バッチが 1 回の undo / redo でまとめて戻り・やり直せる（DXF・DWG）
    let mut live_dxf = cx_run(|cx| {
        zumen_acad::AcadLive::open_from_dxf_file(testkit::read::<Dxf>(&src).0.unwrap(), cx)
    })
    .0
    .unwrap();
    let dxf_before = elements(&cx_run(|cx| live_dxf.drawing(cx)).0.unwrap());
    let dxf_batch = Changes {
        document: None,
        edits: dxf_before[..2]
            .iter()
            .map(|(id, hash, _)| Edit::Copy {
                target: Seen {
                    id: id.clone(),
                    hash: *hash,
                },
                transform: SHIFT,
            })
            .collect(),
    };
    let dxf_applied = cx_run(|cx| live_dxf.apply(&dxf_batch, cx)).0.unwrap();
    assert_eq!(dxf_applied.len(), 2);
    assert_eq!(live_dxf.undo_len(), 1);
    let dxf_after = elements(&cx_run(|cx| live_dxf.drawing(cx)).0.unwrap());
    assert_eq!(dxf_after.len(), dxf_before.len() + 2);
    assert!(cx_run(|cx| live_dxf.undo(cx)).0.unwrap());
    assert_eq!(
        elements(&cx_run(|cx| live_dxf.drawing(cx)).0.unwrap()),
        dxf_before
    );
    assert!(cx_run(|cx| live_dxf.redo(cx)).0.unwrap());
    assert_eq!(
        elements(&cx_run(|cx| live_dxf.drawing(cx)).0.unwrap()),
        dxf_after
    );

    let dwg_src = include_bytes!("../../../samples/dwg/zumen-layout-r2018.dwg");
    let mut live_dwg = cx_run(|cx| zumen_acad::AcadLive::open_dwg(dwg_src, cx))
        .0
        .unwrap();
    let dwg_before = elements(&cx_run(|cx| live_dwg.drawing(cx)).0.unwrap());
    let dwg_lines: Vec<_> = dwg_before
        .iter()
        .filter(|(id, _, _)| id.starts_with("space/0/"))
        .take(2)
        .collect();
    assert_eq!(dwg_lines.len(), 2);
    let dwg_batch = Changes {
        document: None,
        edits: dwg_lines
            .iter()
            .map(|(id, hash, _)| Edit::Copy {
                target: Seen {
                    id: id.clone(),
                    hash: *hash,
                },
                transform: SHIFT,
            })
            .collect(),
    };
    let dwg_applied = cx_run(|cx| live_dwg.apply(&dwg_batch, cx)).0.unwrap();
    assert_eq!(dwg_applied.len(), dwg_lines.len());
    assert_eq!(live_dwg.undo_len(), 1);
    let dwg_after = elements(&cx_run(|cx| live_dwg.drawing(cx)).0.unwrap());
    assert_eq!(dwg_after.len(), dwg_before.len() + dwg_lines.len());
    assert!(cx_run(|cx| live_dwg.undo(cx)).0.unwrap());
    assert_eq!(
        elements(&cx_run(|cx| live_dwg.drawing(cx)).0.unwrap()),
        dwg_before
    );
    assert!(cx_run(|cx| live_dwg.redo(cx)).0.unwrap());
    assert_eq!(
        elements(&cx_run(|cx| live_dwg.drawing(cx)).0.unwrap()),
        dwg_after
    );
}
