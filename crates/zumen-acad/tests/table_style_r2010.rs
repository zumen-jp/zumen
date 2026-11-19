//! R2010・R2013・R2018 の DWG の `TABLESTYLE` と `ACAD_TABLE` から、行とセルの文字高・文字色を
//! 図面モデルへ移し、未設定と上書きの区別と未対応フィールドの診断を確かめる回帰試験（#906）。
#![cfg(feature = "drawing")]

use std::path::Path;

use zumen_acad::{AcadLive, Data, Dwg, Dxf};
use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Key, codes};
use zumen_drawing::{
    CellContent, CellData, CellFormat, CellValue, Color, ColorCode, ColorSystem, Drawing2D,
    Inherit, Length, ModelLength, Shape, SpaceId, ValueUnit,
};
use zumen_sdk::{Codec, Lift};

fn fixture_bytes(name: &str) -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name);
    std::fs::read(&path).unwrap_or_else(|err| panic!("{}: {err}", path.display()))
}

fn lift_dwg(src: &[u8]) -> (Drawing2D, Diagnostics) {
    let mut diag = Diagnostics::new();
    let mut budget = Budget::default();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
    let drawing = Dwg::read(src, &mut cx).unwrap().lift(&mut cx).unwrap();
    (drawing, diag)
}

fn lift_dxf(src: &[u8]) -> (Drawing2D, Diagnostics) {
    let mut diag = Diagnostics::new();
    let mut budget = Budget::default();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
    let drawing = Dxf::read(src, &mut cx).unwrap().lift(&mut cx).unwrap();
    (drawing, diag)
}

fn model_len(value: f64) -> Length {
    Length::Model(ModelLength {
        space: SpaceId(0),
        value,
    })
}

fn aci_own(index: u32) -> Inherit<Color> {
    Inherit::Own(Color {
        code: Some(ColorCode {
            system: ColorSystem::Aci,
            value: index,
        }),
        rgb: None,
    })
}

/// R2010・R2013・R2018 の合成 DWG と対照 DXF で、データ行・各セルの文字高と色・色見本帳の色が一致し、
/// 未対応の位置合わせ（170 = 5）を持つ表にだけ `dwg.table-format.unsupported` が残ることを確かめる。
#[test]
fn style_pairs_match_control_dxf_across_r2010_r2013_and_r2018() {
    for tag in ["r2010", "r2013", "r2018"] {
        let dwg_src = fixture_bytes(&format!("table-style-{tag}.dwg"));
        let dxf_src = fixture_bytes(&format!("table-style-{tag}.dxf"));
        let (dwg_draw, dwg_diag) = lift_dwg(&dwg_src);
        let (dxf_draw, dxf_diag) = lift_dxf(&dxf_src);

        assert_eq!(dwg_draw.spaces[0].elements.len(), 2, "{tag}");
        assert_eq!(dxf_draw.spaces[0].elements.len(), 2, "{tag}");

        // 色見本帳を参照する円（#49 -> DBCOLOR #48）の色名と RGB を DWG・DXF で確認する
        let dwg_circle = &dwg_draw.spaces[0].elements[0];
        let dxf_circle = &dxf_draw.spaces[0].elements[0];
        assert_eq!(dwg_circle.source.key(), Some(Key::Handle(0x49)), "{tag}");
        let (Inherit::Own(dwg_color), Inherit::Own(dxf_color)) =
            (&dwg_circle.style.color, &dxf_circle.style.color)
        else {
            panic!("{tag}: {:?}", dwg_circle.style.color);
        };
        assert_eq!(dwg_color.rgb, Some([13, 97, 184]), "{tag}");
        assert_eq!(dxf_color.rgb, Some([13, 97, 184]), "{tag}");
        let mut diag_db = Diagnostics::new();
        let mut budget_db = Budget::default();
        let cancel_db = CancelToken::new();
        let mut cx_db = Cx::new(&mut budget_db, &mut diag_db, &cancel_db);
        let dwg_db = Dwg::read(&dwg_src, &mut cx_db)
            .unwrap()
            .objects(&mut cx_db)
            .unwrap();
        let dxf_db = Dxf::read(&dxf_src, &mut cx_db)
            .unwrap()
            .objects(&mut cx_db)
            .unwrap();
        assert_eq!(
            dwg_db.get(0x49).unwrap().first(430),
            Some(&Data::Text(b"SamplePocket$Harbor".to_vec())),
            "{tag}"
        );
        assert_eq!(
            dwg_db.get(0x49).unwrap().first(430),
            dxf_db.get(0x49).unwrap().first(430),
            "{tag}"
        );

        // 表（#4E -> TABLESTYLE #4A）のセルとデータ行の文字高・色が対照 DXF と一致する
        let dwg_el = &dwg_draw.spaces[0].elements[1];
        let dxf_el = &dxf_draw.spaces[0].elements[1];
        assert_eq!(dwg_el.source.key(), Some(Key::Handle(0x4E)), "{tag}");
        let (Shape::Table(dwg_t), Shape::Table(dxf_t)) = (&dwg_el.shape, &dxf_el.shape) else {
            panic!("{tag}");
        };
        assert_eq!(
            dwg_t.style.as_ref().and_then(|r| r.key()),
            Some(Key::Handle(0x4A)),
            "{tag}"
        );
        assert_eq!(
            dxf_t.style.as_ref().and_then(|r| r.key()),
            Some(Key::Handle(0x4A)),
            "{tag}"
        );
        assert_eq!(dwg_t.row_heights, dxf_t.row_heights, "{tag}");
        assert_eq!(dwg_t.column_widths, dxf_t.column_widths, "{tag}");
        assert_eq!(dwg_t.cells.len(), 2, "{tag}");
        for k in 0..2 {
            assert_eq!(
                dwg_t.cells[k].content, dxf_t.cells[k].content,
                "{tag} cell {k}"
            );
            assert_eq!(dwg_t.cells[k].value, dxf_t.cells[k].value, "{tag} cell {k}");
            assert_eq!(
                dwg_t.cells[k].format, dxf_t.cells[k].format,
                "{tag} cell {k}"
            );
            assert_eq!(
                dwg_t.cells[k].format.text_height,
                Some(model_len(2.5 + k as f64)),
                "{tag} cell {k}"
            );
            assert_eq!(
                dwg_t.cells[k].format.text_color,
                Some(aci_own(3 + k as u32)),
                "{tag} cell {k}"
            );
        }

        let dwg_fmt = dwg_t.formats.as_deref().expect("dwg formats");
        let dxf_fmt = dxf_t.formats.as_deref().expect("dxf formats");
        assert_eq!(
            dwg_fmt.data_format.text_height,
            Some(model_len(1.75)),
            "{tag}"
        );
        assert_eq!(dwg_fmt.data_format.text_color, Some(aci_own(2)), "{tag}");
        assert_eq!(
            dwg_fmt.data_format.text_style.as_deref(),
            Some("Standard"),
            "{tag}"
        );
        assert_eq!(
            dwg_fmt.data_format.text_height, dxf_fmt.data_format.text_height,
            "{tag}"
        );
        assert_eq!(
            dwg_fmt.data_format.text_color, dxf_fmt.data_format.text_color,
            "{tag}"
        );
        assert_eq!(
            dwg_fmt.data_format.text_style, dxf_fmt.data_format.text_style,
            "{tag}"
        );
        assert_eq!(
            dwg_fmt.title_format.text_height, dxf_fmt.title_format.text_height,
            "{tag}"
        );
        assert_eq!(
            dwg_fmt.header_format.text_height, dxf_fmt.header_format.text_height,
            "{tag}"
        );

        // SampleFormat (#4A) は位置合わせ（170 = 5）も持つので、表 #4E に未対応診断が残る
        let unsupported: Vec<_> = dwg_diag
            .iter()
            .filter(|d| d.code() == codes::DWG_TABLE_FORMAT_UNSUPPORTED)
            .collect();
        assert_eq!(unsupported.len(), 1, "{tag}: {dwg_diag:?}");
        assert_eq!(unsupported[0].places(), &[dwg_el.source.clone()], "{tag}");
        assert!(
            dxf_diag
                .iter()
                .all(|d| d.code() != codes::DWG_TABLE_FORMAT_UNSUPPORTED),
            "{tag}"
        );

        // 常駐文書エンジン（AcadLive）でも同じ図面モデルになる
        let mut live_diag = Diagnostics::new();
        let mut live_budget = Budget::default();
        let live_cancel = CancelToken::new();
        let mut live_cx = Cx::new(&mut live_budget, &mut live_diag, &live_cancel);
        let live = AcadLive::open_dwg(&dwg_src, &mut live_cx).unwrap();
        let live_drawing = live.drawing(&mut live_cx).unwrap();
        assert_eq!(live_drawing, dwg_draw, "{tag} live parity");
    }
}

/// 未設定フィールドと上書きフィールド（セルスタイル上書き・内容書式上書き）を区別し、対応済みのフィールドだけを使う表では
/// `dwg.table-format.unsupported` を出さず、真の色上書きを持つ表にだけ出すことを確かめる。
#[test]
fn distinguishes_unset_from_overridden_and_isolates_unsupported_diagnostics() {
    let raw = fixture_bytes("table-format-cases-r2018.dwg");
    let (drawing, diag) = lift_dwg(&raw);
    assert_eq!(drawing.spaces[0].elements.len(), 2);

    let el1 = &drawing.spaces[0].elements[0];
    let el2 = &drawing.spaces[0].elements[1];
    let (Shape::Table(t1), Shape::Table(t2)) = (&el1.shape, &el2.shape) else {
        panic!("expected two tables");
    };

    // CleanFormat の表題・見出し・データ行それぞれの文字高・文字色・文字スタイルが移る
    let fmt1 = t1.formats.as_deref().expect("CleanFormat");
    assert_eq!(fmt1.title_format.text_height, Some(model_len(4.0)));
    assert_eq!(fmt1.title_format.text_color, Some(aci_own(1)));
    assert_eq!(fmt1.title_format.text_style.as_deref(), Some("Standard"));
    assert_eq!(fmt1.header_format.text_height, Some(model_len(3.0)));
    assert_eq!(fmt1.header_format.text_color, Some(aci_own(6)));
    assert_eq!(fmt1.header_format.text_style.as_deref(), Some("Standard"));
    assert_eq!(fmt1.data_format.text_height, Some(model_len(2.25)));
    assert_eq!(fmt1.data_format.text_color, Some(aci_own(5)));
    assert_eq!(fmt1.data_format.text_style.as_deref(), Some("Standard"));

    // 4 セル: (0,0) 未設定、(0,1) 高さのみ、(1,0) 色のみ（ByBlock）、(1,1) 内容書式で高さと色（ByLayer）
    assert_eq!(t1.cells.len(), 4);
    assert_eq!(t1.cells[0].format, CellFormat::default());
    assert_eq!(
        t1.cells[1].format,
        CellFormat {
            text_height: Some(model_len(3.25)),
            ..CellFormat::default()
        }
    );
    assert_eq!(
        t1.cells[2].format,
        CellFormat {
            text_color: Some(Inherit::Block),
            ..CellFormat::default()
        }
    );
    assert_eq!(
        t1.cells[3].format,
        CellFormat {
            text_height: Some(model_len(4.5)),
            text_color: Some(Inherit::Layer),
            ..CellFormat::default()
        }
    );
    for (idx, expected) in ["c00", "c01", "c10", "c11"].into_iter().enumerate() {
        assert!(
            matches!(&t1.cells[idx].content, CellContent::Text { content, .. } if content == expected)
        );
        assert_eq!(
            t1.cells[idx].value,
            Some(CellValue {
                data: CellData::Text(expected.to_owned()),
                unit: ValueUnit::None,
                format: None,
            })
        );
    }

    // 表 1 は未対応フィールドを持たず、表 2（セルの RGB 真の色上書き）だけが dwg.table-format.unsupported を出す
    assert_eq!(t2.cells[0].format, CellFormat::default());
    let unsupported: Vec<_> = diag
        .iter()
        .filter(|d| d.code() == codes::DWG_TABLE_FORMAT_UNSUPPORTED)
        .collect();
    assert_eq!(unsupported.len(), 1, "{diag:?}");
    assert_eq!(unsupported[0].places(), &[el2.source.clone()]);

    // 空の既定スタイルを持つ既存フィクスチャでも未対応診断を出さず書式も空のまま保つ
    for fixture in [
        "table-r2010.dwg",
        "table-r2013.dwg",
        "table-r2018.dwg",
        "table-styled-r2010.dwg",
        "table-styled-r2013.dwg",
        "table-styled-r2018.dwg",
    ] {
        let (draw, blank_diag) = lift_dwg(&fixture_bytes(fixture));
        let Shape::Table(t) = &draw.spaces[0].elements[0].shape else {
            panic!("{fixture}");
        };
        assert_eq!(t.formats, None, "{fixture}");
        assert!(
            t.cells.iter().all(|c| c.format == CellFormat::default()),
            "{fixture}"
        );
        assert!(
            blank_diag
                .iter()
                .all(|d| d.code() != codes::DWG_TABLE_FORMAT_UNSUPPORTED),
            "{fixture}: {blank_diag:?}"
        );
    }
}
