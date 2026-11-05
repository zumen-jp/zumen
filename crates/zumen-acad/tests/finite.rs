//! 有限の入力の計算が overflow しても、非有限値を図面モデルに移さない（#594）。
#![cfg(feature = "drawing")]

use zumen_acad::Dxf;
use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Key, Ref, codes};
use zumen_drawing::{Drawing2D, Hash, Piece, Scaled, Shape, SpaceId, id};
use zumen_sdk::{Lift, testkit};

const LINE: &[(i16, &str)] = &[
    (0, "LINE"),
    (5, "B"),
    (8, "0"),
    (10, "0"),
    (20, "0"),
    (11, "1"),
    (21, "1"),
];

fn dxf(unit: &str, header: &[(i16, &str)], entities: &[(i16, &str)]) -> Vec<u8> {
    let mut tags = vec![(0, "SECTION"), (2, "HEADER"), (9, "$INSUNITS"), (70, unit)];
    tags.extend_from_slice(header);
    tags.extend_from_slice(&[(0, "ENDSEC"), (0, "SECTION"), (2, "ENTITIES")]);
    tags.extend_from_slice(entities);
    // IMAGE の参照先。ファイルは開かない。
    tags.extend_from_slice(&[
        (0, "ENDSEC"),
        (0, "SECTION"),
        (2, "OBJECTS"),
        (0, "IMAGEDEF"),
        (5, "D"),
        (1, "synthetic.png"),
        (0, "BLOCK_RECORD"),
        (5, "E"),
        (2, "B"),
        (0, "ENDSEC"),
        (0, "EOF"),
    ]);
    tags.iter()
        .map(|(code, value)| format!("{code}\n{value}\n"))
        .collect::<String>()
        .into_bytes()
}

#[cfg(test)]
fn lift(src: &[u8]) -> (Drawing2D, Diagnostics) {
    let (file, read_diag) = testkit::read::<Dxf>(src);
    assert!(read_diag.is_empty(), "{read_diag:?}");
    let file = file.unwrap();
    let mut diag = Diagnostics::new();
    let drawing = file
        .lift(&mut Cx::new(
            &mut Budget::default(),
            &mut diag,
            &CancelToken::new(),
        ))
        .unwrap();
    // lift の失敗箇所も、L1 から変えずに書ける。
    assert_eq!(testkit::write::<Dxf>(&file).unwrap(), src);
    (drawing, diag)
}

#[test]
fn overflow_drops_only_the_bad_element_or_view_and_keeps_the_good_id() {
    type Case<'a> = (&'a str, i64, &'a str, &'a [(i16, &'a str)]);
    let cases: &[Case<'_>] = &[
        (
            "ELLIPSE",
            11,
            "4",
            &[
                (10, "1e308"),
                (20, "0"),
                (11, "1e308"),
                (21, "0"),
                (40, "1"),
            ],
        ),
        (
            "ELLIPSE",
            21,
            "4",
            &[
                (10, "0"),
                (20, "1e308"),
                (11, "0"),
                (21, "1e308"),
                (40, "1"),
            ],
        ),
        (
            "ELLIPSE",
            41,
            "4",
            &[
                (10, "0"),
                (20, "0"),
                (11, "1"),
                (21, "0"),
                (40, "1"),
                (41, "1e308"),
            ],
        ),
        (
            "ELLIPSE",
            42,
            "4",
            &[
                (10, "0"),
                (20, "0"),
                (11, "1"),
                (21, "0"),
                (40, "1"),
                (42, "1e308"),
            ],
        ),
        (
            "MTEXT",
            50,
            "4",
            &[(10, "0"), (20, "0"), (40, "1"), (1, "a"), (50, "1e308")],
        ),
        (
            "LINE",
            10,
            "6",
            &[(67, "1"), (10, "1e308"), (20, "0"), (11, "0"), (21, "0")],
        ),
        (
            "LINE",
            20,
            "6",
            &[(67, "1"), (10, "0"), (20, "1e308"), (11, "0"), (21, "0")],
        ),
        (
            "CIRCLE",
            40,
            "6",
            &[(67, "1"), (10, "0"), (20, "0"), (40, "1e308")],
        ),
        (
            "VIEWPORT",
            45,
            "4",
            &[
                (67, "1"),
                (69, "2"),
                (10, "0"),
                (20, "0"),
                (40, "1"),
                (41, "1e308"),
                (45, "1e-308"),
            ],
        ),
        (
            "VIEWPORT",
            12,
            "4",
            &[
                (67, "1"),
                (69, "2"),
                (10, "0"),
                (20, "0"),
                (12, "1e308"),
                (40, "1"),
                (41, "2"),
                (45, "1"),
            ],
        ),
        (
            "VIEWPORT",
            40,
            "4",
            &[
                (67, "1"),
                (69, "2"),
                (10, "1.7e308"),
                (20, "0"),
                (40, "1e308"),
                (41, "1"),
                (45, "1"),
            ],
        ),
        (
            "IMAGE",
            11,
            "4",
            &[
                (340, "D"),
                (10, "0"),
                (20, "0"),
                (11, "1e308"),
                (21, "0"),
                (12, "0"),
                (22, "1"),
                (13, "2"),
                (23, "1"),
            ],
        ),
        (
            "IMAGE",
            22,
            "4",
            &[
                (340, "D"),
                (10, "0"),
                (20, "0"),
                (11, "1"),
                (21, "0"),
                (12, "0"),
                (22, "1e308"),
                (13, "1"),
                (23, "2"),
            ],
        ),
        (
            "HATCH",
            11,
            "4",
            &[
                (70, "1"),
                (91, "1"),
                (92, "0"),
                (93, "1"),
                (72, "3"),
                (10, "1e308"),
                (20, "0"),
                (11, "1e308"),
                (21, "0"),
                (40, "1"),
                (50, "0"),
                (51, "360"),
                (73, "1"),
                (97, "0"),
                (75, "0"),
                (76, "0"),
            ],
        ),
        (
            "HATCH",
            460,
            "4",
            &[
                (70, "1"),
                (91, "0"),
                (75, "0"),
                (76, "0"),
                (450, "1"),
                (460, "1e308"),
            ],
        ),
        (
            "MULTILEADER",
            42,
            "4",
            &[
                (300, "CONTEXT_DATA{"),
                (290, "1"),
                (304, "a"),
                (42, "1e308"),
                (301, "}"),
            ],
        ),
        (
            "MULTILEADER",
            46,
            "4",
            &[
                (300, "CONTEXT_DATA{"),
                (296, "1"),
                (341, "E"),
                (15, "0"),
                (25, "0"),
                (46, "1e308"),
                (301, "}"),
            ],
        ),
    ];
    let (baseline, diag) = lift(&dxf("4", &[], LINE));
    assert!(diag.is_empty());
    let good = &baseline.spaces[0].elements[0];
    let space = id::space(SpaceId(0));
    let expected_id = id::Namer::new(&space).next(0, good);
    for &(kind, code, unit, values) in cases {
        let mut entities = vec![(0, kind), (5, "A"), (8, "0")];
        entities.extend_from_slice(values);
        entities.extend_from_slice(LINE);
        let (drawing, diag) = lift(&dxf(unit, &[], &entities));
        let found: Vec<_> = diag.iter().collect();
        assert_eq!(found.len(), 1, "{kind}/{code}: {found:?}");
        assert_eq!(found[0].code(), codes::DXF_LIFT_INVALID);
        assert_eq!(found[0].param("kind"), Some(&kind.into()));
        assert_eq!(found[0].param("code"), Some(&code.into()));
        assert_eq!(found[0].count(), 1);
        assert_eq!(
            found[0].places(),
            [Ref::at_record(3).with_key(Key::Handle(0xA))]
        );
        let [kept] = drawing.spaces[0].elements.as_slice() else {
            panic!("{kind}: {drawing:?}")
        };
        assert_eq!(Hash::element(kept), Hash::element(good));
        assert_eq!(id::Namer::new(&space).next(0, kept), expected_id);
        assert!(drawing.sheets.is_empty(), "{kind}: {drawing:?}");
    }
}

#[test]
fn overflow_in_sheet_size_keeps_the_sheet_without_a_size() {
    let header = [
        (9, "$PLIMMIN"),
        (10, "0"),
        (20, "0"),
        (9, "$PLIMMAX"),
        (10, "1e308"),
        (20, "1"),
    ];
    let mut paper = LINE.to_vec();
    paper.push((67, "1"));
    let (drawing, diag) = lift(&dxf("6", &header, &paper));
    let found: Vec<_> = diag.iter().collect();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].code(), codes::DXF_LIFT_INVALID);
    assert_eq!(found[0].param("kind"), Some(&"$PLIMMAX".into()));
    assert_eq!(found[0].param("code"), Some(&10_i64.into()));
    assert!(found[0].places().is_empty());
    assert!(drawing.sheets[0].size.is_none());
    assert_eq!(drawing.sheets[0].elements.len(), 1);
    let Shape::Line { end, .. } = drawing.sheets[0].elements[0].shape else {
        panic!("line")
    };
    assert_eq!([end.x, end.y], [1000.0, 1000.0]);
}

#[test]
fn overflowing_mtext_factors_remain_unparsed_without_changing_the_previous_format() {
    for code in ['H', 'W', 'T'] {
        for suffix in ["", "x"] {
            let content = format!("\\{code}1e308{suffix};a\\{code}2x;b");
            let entities = [
                (0, "MTEXT"),
                (8, "0"),
                (10, "0"),
                (20, "0"),
                (40, "1"),
                (1, content.as_str()),
            ];
            let (drawing, diag) = lift(&dxf("4", &[], &entities));
            assert!(diag.is_empty());
            let Shape::Formatted(text) = &drawing.spaces[0].elements[0].shape else {
                panic!("mtext")
            };
            assert_eq!(text.spans.len(), 3);
            assert_eq!(text.spans[1].piece, Piece::Unparsed(format!("\\{code}2x;")));
            assert_eq!(text.spans[0].format, text.spans[2].format);
            let format = &text.spans[2].format;
            match code {
                'H' if suffix == "x" => assert_eq!(format.height, Some(Scaled::Factor(1e308))),
                'H' => assert!(
                    matches!(format.height, Some(Scaled::Value(zumen_drawing::Length::Model(value))) if value.value == 1e308)
                ),
                'W' => assert_eq!(
                    format.width_factor,
                    Some(if suffix == "x" {
                        Scaled::Factor(1e308)
                    } else {
                        Scaled::Value(1e308)
                    })
                ),
                'T' => assert_eq!(
                    format.tracking,
                    Some(if suffix == "x" {
                        Scaled::Factor(1e308)
                    } else {
                        Scaled::Value(1e308)
                    })
                ),
                _ => panic!("unknown format"),
            }
        }
    }
}
