//! R2000・R2007・R2018 の段組み MTEXT の持ち上げ・後続段の関連付け・常駐エンジンとの一致を確かめる回帰試験（#904）。
#![cfg(feature = "drawing")]

use zumen_acad::{AcadLive, Dxf};
use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Key, Ref, codes};
use zumen_drawing::Drawing2D;
use zumen_sdk::{Codec, Lift};

#[cfg(test)]
fn lift_both(src: &[u8]) -> (Drawing2D, Diagnostics) {
    let mut diag = Diagnostics::new();
    let mut budget = Budget::default();
    let cancel = CancelToken::new();
    let drawing = Dxf::read(src, &mut Cx::new(&mut budget, &mut diag, &cancel))
        .unwrap()
        .lift(&mut Cx::new(&mut budget, &mut diag, &cancel))
        .unwrap();

    let mut live_diag = Diagnostics::new();
    let mut live_budget = Budget::default();
    let live =
        AcadLive::open(src, &mut Cx::new(&mut live_budget, &mut live_diag, &cancel)).unwrap();
    let mut draw_diag = Diagnostics::new();
    let live_drawing = live
        .drawing(&mut Cx::new(&mut live_budget, &mut draw_diag, &cancel))
        .unwrap();
    assert_eq!(live_drawing, drawing);
    assert_eq!(live_diag.into_vec(), diag.clone().into_vec());
    (drawing, diag)
}

#[test]
fn dxf_mtext_columns_match_across_r2000_r2007_and_r2018() {
    let r2000 = include_bytes!("../../../samples/dxf/ezdxf-mtext-columns-r2000.dxf");
    let r2007 = include_bytes!("../../../samples/dxf/ezdxf-mtext-columns-r2007.dxf");
    let r2018 = include_bytes!("../../../samples/dxf/ezdxf-mtext-columns-r2018.dxf");

    for (label, src, handles) in [
        ("r2000", &r2000[..], [0x2Du64, 0x30, 0x33]),
        ("r2007", &r2007[..], [0x2Du64, 0x30, 0x33]),
        ("r2018", &r2018[..], [0x2Du64, 0x2E, 0x2F]),
    ] {
        let (drawing, diag) = lift_both(src);
        assert!(drawing.spaces[0].elements.is_empty(), "{label}");
        let found: Vec<_> = diag
            .iter()
            .filter(|d| d.code() == codes::DXF_MTEXT_COLUMNS_UNSUPPORTED)
            .collect();
        assert_eq!(found.len(), 1, "{label}: {diag:?}");
        assert_eq!(found[0].count(), 3, "{label}");
        let found_handles: Vec<_> = found[0].places().iter().map(Ref::key).collect();
        assert_eq!(
            found_handles,
            handles.map(|h| Some(Key::Handle(h))),
            "{label}"
        );
    }
}

#[test]
fn block_definition_and_paper_space_absorb_linked_column_followers() {
    let dxf = b"0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1021\n9\n$INSUNITS\n70\n4\n0\nENDSEC\n\
0\nSECTION\n2\nBLOCKS\n\
0\nBLOCK\n5\n10\n8\n0\n2\nNOTE_BLK\n70\n0\n10\n0.0\n20\n0.0\n3\nNOTE_BLK\n\
0\nMTEXT\n5\n11\n8\n0\n10\n0.0\n20\n0.0\n40\n2.0\n41\n20.0\n71\n1\n1\nb1\n\
1001\nACAD\n1000\nACAD_MTEXT_COLUMN_INFO_BEGIN\n1070\n75\n1070\n2\n1070\n50\n1070\n2\n1040\n15.0\n1040\n0.0\n1000\nACAD_MTEXT_COLUMN_INFO_END\n\
1000\nACAD_MTEXT_COLUMNS_BEGIN\n1070\n47\n1070\n2\n1005\n12\n1000\nACAD_MTEXT_COLUMNS_END\n\
0\nMTEXT\n5\n12\n8\n0\n10\n21.0\n20\n0.0\n40\n2.0\n41\n20.0\n71\n1\n1\n\n\
0\nLINE\n5\n13\n8\n0\n10\n0.0\n20\n0.0\n11\n5.0\n21\n0.0\n\
0\nENDBLK\n5\n14\n8\n0\n\
0\nENDSEC\n\
0\nSECTION\n2\nENTITIES\n\
0\nMTEXT\n5\n21\n67\n1\n8\n0\n10\n0.0\n20\n0.0\n40\n2.5\n41\n20.0\n71\n1\n1\np1\n\
1001\nACAD\n1000\nACAD_MTEXT_COLUMN_INFO_BEGIN\n1070\n75\n1070\n1\n1000\nACAD_MTEXT_COLUMN_INFO_END\n\
1000\nACAD_MTEXT_COLUMNS_BEGIN\n1070\n47\n1070\n2\n1005\n22\n1000\nACAD_MTEXT_COLUMNS_END\n\
0\nMTEXT\n5\n22\n67\n1\n8\n0\n10\n21.0\n20\n0.0\n40\n2.5\n41\n20.0\n71\n1\n1\np2\n\
0\nLINE\n5\n23\n67\n1\n8\n0\n10\n0.0\n20\n0.0\n11\n10.0\n21\n0.0\n\
0\nENDSEC\n0\nEOF\n";

    let (drawing, diag) = lift_both(dxf);
    assert_eq!(drawing.symbols.len(), 1);
    assert_eq!(drawing.symbols[0].space.elements.len(), 1);
    assert_eq!(
        drawing.symbols[0].space.elements[0].source.key(),
        Some(Key::Handle(0x13))
    );
    assert_eq!(drawing.sheets.len(), 1);
    assert_eq!(drawing.sheets[0].elements.len(), 1);
    assert_eq!(
        drawing.sheets[0].elements[0].source.key(),
        Some(Key::Handle(0x23))
    );
    let found: Vec<_> = diag
        .iter()
        .filter(|d| d.code() == codes::DXF_MTEXT_COLUMNS_UNSUPPORTED)
        .collect();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].count(), 2);
    let handles: Vec<_> = found[0].places().iter().map(Ref::key).collect();
    assert_eq!(
        handles,
        vec![Some(Key::Handle(0x21)), Some(Key::Handle(0x11))]
    );
}
