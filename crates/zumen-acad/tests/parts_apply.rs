//! 寸法（DIMENSION・ARC_DIMENSION）と表（ACAD_TABLE）の書き戻しと、匿名ブロック（`*D<n>`・`*T<n>`）の作り直しの試験（#314）。
#![cfg(feature = "drawing")]

mod support;

use std::path::PathBuf;
use zumen_acad::{Data, Dwg, DwgFile, Dxf, DxfFile};
use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits, codes};
use zumen_drawing::{
    Affine, Axis, CellContent, Changes, DimensionKind, Drawing2D, Edge, Edit, Element, Hash, Label,
    Length, ModelLength, ModelPoint, PaperLength, PaperPoint, Placed, Seen, Shape, SpaceId, id,
};
use zumen_sdk::{Apply, Codec, Error, Lift};

#[cfg(test)]
fn cx_run<T>(f: impl FnOnce(&mut Cx<'_>) -> T) -> (T, Diagnostics) {
    let mut budget = Budget::default();
    let mut diagnostics = Diagnostics::new();
    let cancel = CancelToken::new();
    let out = f(&mut Cx::new(&mut budget, &mut diagnostics, &cancel));
    (out, diagnostics)
}

#[cfg(test)]
fn read_dxf(bytes: &[u8]) -> DxfFile {
    cx_run(|cx| Dxf::read(bytes, cx).unwrap()).0
}

#[cfg(test)]
fn write_dxf(file: &DxfFile) -> Vec<u8> {
    cx_run(|cx| {
        let mut out = Vec::new();
        Dxf::write(file, &mut out, cx).unwrap();
        out
    })
    .0
}

#[cfg(test)]
fn lift_dxf(file: &DxfFile) -> Drawing2D {
    cx_run(|cx| file.lift(cx).unwrap()).0
}

#[cfg(test)]
fn seen_model(drawing: &Drawing2D, index: usize) -> (Seen, Element<ModelPoint>) {
    let space = &drawing.spaces[0];
    let place = id::space(space.id);
    let mut namer = id::Namer::new(&place);
    for (i, el) in space.elements.iter().enumerate() {
        let elem_id = namer.next(i, el);
        if i == index {
            return (
                Seen {
                    id: elem_id,
                    hash: Hash::element(el),
                },
                el.clone(),
            );
        }
    }
    panic!("要素 {index} がない");
}

#[cfg(test)]
fn symbol_elements<'a>(drawing: &'a Drawing2D, name: &str) -> &'a [Element<ModelPoint>] {
    &drawing
        .symbols
        .iter()
        .find(|def| def.key.name.eq_ignore_ascii_case(name))
        .unwrap_or_else(|| panic!("ブロック {name} がない"))
        .space
        .elements
}

#[cfg(test)]
fn mpt(x: f64, y: f64) -> ModelPoint {
    ModelPoint {
        space: SpaceId(0),
        x,
        y,
    }
}

fn mlen(value: f64) -> Length {
    Length::Model(ModelLength {
        space: SpaceId(0),
        value,
    })
}

/// `*D1`〜`*D7` を持つ 7 種類の DIMENSION を並べた R2018 DXF
fn all_dimensions_dxf() -> Vec<u8> {
    let mut out = String::new();
    let mut push = |code: i16, val: &str| {
        out.push_str(&format!("{code}\n{val}\n"));
    };
    for (c, v) in [
        (0, "SECTION"),
        (2, "HEADER"),
        (9, "$ACADVER"),
        (1, "AC1032"),
        (9, "$INSUNITS"),
        (70, "4"),
        (9, "$HANDSEED"),
        (5, "500"),
        (0, "ENDSEC"),
        (0, "SECTION"),
        (2, "TABLES"),
        (0, "TABLE"),
        (2, "LTYPE"),
        (5, "5"),
        (330, "0"),
        (100, "AcDbSymbolTable"),
        (70, "1"),
        (0, "LTYPE"),
        (5, "14"),
        (330, "5"),
        (100, "AcDbSymbolTableRecord"),
        (100, "AcDbLinetypeTableRecord"),
        (2, "Continuous"),
        (70, "0"),
        (3, "Solid"),
        (72, "65"),
        (73, "0"),
        (40, "0.0"),
        (0, "ENDTAB"),
        (0, "TABLE"),
        (2, "LAYER"),
        (5, "2"),
        (330, "0"),
        (100, "AcDbSymbolTable"),
        (70, "1"),
        (0, "LAYER"),
        (5, "10"),
        (330, "2"),
        (100, "AcDbSymbolTableRecord"),
        (100, "AcDbLayerTableRecord"),
        (2, "0"),
        (70, "0"),
        (62, "7"),
        (6, "Continuous"),
        (0, "ENDTAB"),
        (0, "TABLE"),
        (2, "STYLE"),
        (5, "3"),
        (330, "0"),
        (100, "AcDbSymbolTable"),
        (70, "1"),
        (0, "STYLE"),
        (5, "11"),
        (330, "3"),
        (100, "AcDbSymbolTableRecord"),
        (100, "AcDbTextStyleTableRecord"),
        (2, "Standard"),
        (70, "0"),
        (40, "0.0"),
        (41, "1.0"),
        (50, "0.0"),
        (71, "0"),
        (42, "2.5"),
        (3, "txt"),
        (4, ""),
        (0, "ENDTAB"),
        (0, "TABLE"),
        (2, "BLOCK_RECORD"),
        (5, "1"),
        (330, "0"),
        (100, "AcDbSymbolTable"),
        (70, "10"),
        (0, "BLOCK_RECORD"),
        (5, "1F"),
        (330, "1"),
        (100, "AcDbSymbolTableRecord"),
        (100, "AcDbBlockTableRecord"),
        (2, "*Model_Space"),
        (0, "BLOCK_RECORD"),
        (5, "1B"),
        (330, "1"),
        (100, "AcDbSymbolTableRecord"),
        (100, "AcDbBlockTableRecord"),
        (2, "*Paper_Space"),
        (0, "BLOCK_RECORD"),
        (5, "30"),
        (330, "1"),
        (100, "AcDbSymbolTableRecord"),
        (100, "AcDbBlockTableRecord"),
        (2, "ARR"),
    ] {
        push(c, v);
    }
    for n in 1..=7 {
        for (c, v) in [
            (0, "BLOCK_RECORD"),
            (5, &format!("{:X}", 0x40 + n)),
            (330, "1"),
            (100, "AcDbSymbolTableRecord"),
            (100, "AcDbBlockTableRecord"),
            (2, &format!("*D{n}")),
        ] {
            push(c, v);
        }
    }
    for (c, v) in [
        (0, "ENDTAB"),
        (0, "TABLE"),
        (2, "DIMSTYLE"),
        (5, "A"),
        (330, "0"),
        (100, "AcDbSymbolTable"),
        (70, "3"),
        (100, "AcDbDimStyleTable"),
        (71, "0"),
        (0, "DIMSTYLE"),
        (105, "2A"),
        (330, "A"),
        (100, "AcDbSymbolTableRecord"),
        (100, "AcDbDimStyleTableRecord"),
        (2, "ISO-25"),
        (70, "0"),
        (40, "1.0"),
        (41, "2.5"),
        (42, "0.625"),
        (44, "1.25"),
        (140, "2.5"),
        (271, "2"),
        (179, "1"),
        (340, "11"),
        (0, "DIMSTYLE"),
        (105, "2B"),
        (330, "A"),
        (100, "AcDbSymbolTableRecord"),
        (100, "AcDbDimStyleTableRecord"),
        (2, "TICK"),
        (70, "0"),
        (40, "1.0"),
        (41, "2.5"),
        (142, "2.0"),
        (140, "2.5"),
        (340, "11"),
        (0, "DIMSTYLE"),
        (105, "2C"),
        (330, "A"),
        (100, "AcDbSymbolTableRecord"),
        (100, "AcDbDimStyleTableRecord"),
        (2, "BLK"),
        (70, "0"),
        (40, "1.0"),
        (41, "2.5"),
        (140, "2.5"),
        (340, "11"),
        (342, "30"),
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
        (3, "*Model_Space"),
        (1, ""),
        (0, "ENDBLK"),
        (5, "21"),
        (330, "1F"),
        (100, "AcDbEntity"),
        (8, "0"),
        (100, "AcDbBlockEnd"),
        (0, "BLOCK"),
        (5, "22"),
        (330, "1B"),
        (100, "AcDbEntity"),
        (8, "0"),
        (67, "1"),
        (100, "AcDbBlockBegin"),
        (2, "*Paper_Space"),
        (70, "0"),
        (10, "0.0"),
        (20, "0.0"),
        (30, "0.0"),
        (3, "*Paper_Space"),
        (1, ""),
        (0, "ENDBLK"),
        (5, "23"),
        (330, "1B"),
        (100, "AcDbEntity"),
        (8, "0"),
        (67, "1"),
        (100, "AcDbBlockEnd"),
        (0, "BLOCK"),
        (5, "31"),
        (330, "30"),
        (100, "AcDbEntity"),
        (8, "0"),
        (100, "AcDbBlockBegin"),
        (2, "ARR"),
        (70, "0"),
        (10, "0.0"),
        (20, "0.0"),
        (30, "0.0"),
        (3, "ARR"),
        (1, ""),
        (0, "LINE"),
        (5, "32"),
        (330, "30"),
        (100, "AcDbEntity"),
        (8, "0"),
        (100, "AcDbLine"),
        (10, "0.0"),
        (20, "0.0"),
        (30, "0.0"),
        (11, "-1.0"),
        (21, "0.5"),
        (31, "0.0"),
        (0, "ENDBLK"),
        (5, "33"),
        (330, "30"),
        (100, "AcDbEntity"),
        (8, "0"),
        (100, "AcDbBlockEnd"),
    ] {
        push(c, v);
    }
    for n in 1..=7 {
        let owner = format!("{:X}", 0x40 + n);
        let blk_h = format!("{:X}", 0x100 + n * 10);
        let lin_h = format!("{:X}", 0x101 + n * 10);
        let end_h = format!("{:X}", 0x102 + n * 10);
        let name = format!("*D{n}");
        for (c, v) in [
            (0, "BLOCK"),
            (5, &blk_h),
            (330, &owner),
            (100, "AcDbEntity"),
            (8, "0"),
            (100, "AcDbBlockBegin"),
            (2, &name),
            (70, "1"),
            (10, "0.0"),
            (20, "0.0"),
            (30, "0.0"),
            (3, &name),
            (1, ""),
            (0, "LINE"),
            (5, &lin_h),
            (330, &owner),
            (100, "AcDbEntity"),
            (8, "0"),
            (100, "AcDbLine"),
            (10, "0.0"),
            (20, "0.0"),
            (30, "0.0"),
            (11, "1.0"),
            (21, "0.0"),
            (31, "0.0"),
            (0, "ENDBLK"),
            (5, &end_h),
            (330, &owner),
            (100, "AcDbEntity"),
            (8, "0"),
            (100, "AcDbBlockEnd"),
        ] {
            push(c, v);
        }
    }
    push(0, "ENDSEC");
    push(0, "SECTION");
    push(2, "ENTITIES");

    // 1. Linear (*D1, ISO-25)
    for (c, v) in [
        (0, "DIMENSION"),
        (5, "201"),
        (330, "1F"),
        (100, "AcDbEntity"),
        (8, "0"),
        (100, "AcDbDimension"),
        (2, "*D1"),
        (10, "100.0"),
        (20, "20.0"),
        (30, "0.0"),
        (11, "50.0"),
        (21, "20.0"),
        (31, "0.0"),
        (70, "32"),
        (71, "5"),
        (42, "100.0"),
        (3, "ISO-25"),
        (100, "AcDbAlignedDimension"),
        (13, "0.0"),
        (23, "0.0"),
        (33, "0.0"),
        (14, "100.0"),
        (24, "0.0"),
        (34, "0.0"),
        (100, "AcDbRotatedDimension"),
    ] {
        push(c, v);
    }
    // 2. Aligned (*D2, TICK)
    for (c, v) in [
        (0, "DIMENSION"),
        (5, "202"),
        (330, "1F"),
        (100, "AcDbEntity"),
        (8, "0"),
        (100, "AcDbDimension"),
        (2, "*D2"),
        (10, "30.0"),
        (20, "50.0"),
        (30, "0.0"),
        (11, "15.0"),
        (21, "25.0"),
        (31, "0.0"),
        (70, "33"),
        (71, "5"),
        (42, "50.0"),
        (3, "TICK"),
        (100, "AcDbAlignedDimension"),
        (13, "0.0"),
        (23, "0.0"),
        (33, "0.0"),
        (14, "30.0"),
        (24, "40.0"),
        (34, "0.0"),
    ] {
        push(c, v);
    }
    // 3. Radius (*D3, BLK)
    for (c, v) in [
        (0, "DIMENSION"),
        (5, "203"),
        (330, "1F"),
        (100, "AcDbEntity"),
        (8, "0"),
        (100, "AcDbDimension"),
        (2, "*D3"),
        (10, "0.0"),
        (20, "0.0"),
        (30, "0.0"),
        (11, "12.5"),
        (21, "0.0"),
        (31, "0.0"),
        (70, "36"),
        (71, "5"),
        (42, "25.0"),
        (3, "BLK"),
        (100, "AcDbRadialDimension"),
        (15, "25.0"),
        (25, "0.0"),
        (35, "0.0"),
        (40, "0.0"),
    ] {
        push(c, v);
    }
    // 4. Diameter (*D4, ISO-25)
    for (c, v) in [
        (0, "DIMENSION"),
        (5, "204"),
        (330, "1F"),
        (100, "AcDbEntity"),
        (8, "0"),
        (100, "AcDbDimension"),
        (2, "*D4"),
        (10, "-20.0"),
        (20, "0.0"),
        (30, "0.0"),
        (11, "0.0"),
        (21, "0.0"),
        (31, "0.0"),
        (70, "35"),
        (71, "5"),
        (42, "40.0"),
        (3, "ISO-25"),
        (100, "AcDbDiametricDimension"),
        (15, "20.0"),
        (25, "0.0"),
        (35, "0.0"),
        (40, "0.0"),
    ] {
        push(c, v);
    }
    // 5. Angular 3-pt (*D5, ISO-25)
    let ang_rad = format!("{:.15}", std::f64::consts::FRAC_PI_2);
    for (c, v) in [
        (0, "DIMENSION"),
        (5, "205"),
        (330, "1F"),
        (100, "AcDbEntity"),
        (8, "0"),
        (100, "AcDbDimension"),
        (2, "*D5"),
        (10, "15.0"),
        (20, "15.0"),
        (30, "0.0"),
        (11, "15.0"),
        (21, "15.0"),
        (31, "0.0"),
        (70, "37"),
        (71, "5"),
        (42, &ang_rad),
        (3, "ISO-25"),
        (100, "AcDb3PointAngularDimension"),
        (13, "20.0"),
        (23, "0.0"),
        (33, "0.0"),
        (14, "0.0"),
        (24, "20.0"),
        (34, "0.0"),
        (15, "0.0"),
        (25, "0.0"),
        (35, "0.0"),
    ] {
        push(c, v);
    }
    // 6. Angular 2-line (*D6, ISO-25)
    for (c, v) in [
        (0, "DIMENSION"),
        (5, "206"),
        (330, "1F"),
        (100, "AcDbEntity"),
        (8, "0"),
        (100, "AcDbDimension"),
        (2, "*D6"),
        (10, "0.0"),
        (20, "20.0"),
        (30, "0.0"),
        (11, "12.0"),
        (21, "12.0"),
        (31, "0.0"),
        (70, "34"),
        (71, "5"),
        (42, &ang_rad),
        (3, "ISO-25"),
        (100, "AcDb2LineAngularDimension"),
        (13, "0.0"),
        (23, "0.0"),
        (33, "0.0"),
        (14, "20.0"),
        (24, "0.0"),
        (34, "0.0"),
        (15, "0.0"),
        (25, "0.0"),
        (35, "0.0"),
        (16, "12.0"),
        (26, "12.0"),
        (36, "0.0"),
    ] {
        push(c, v);
    }
    // 7. Ordinate X (*D7, ISO-25)
    for (c, v) in [
        (0, "DIMENSION"),
        (5, "207"),
        (330, "1F"),
        (100, "AcDbEntity"),
        (8, "0"),
        (100, "AcDbDimension"),
        (2, "*D7"),
        (10, "0.0"),
        (20, "0.0"),
        (30, "0.0"),
        (11, "35.0"),
        (21, "20.0"),
        (31, "0.0"),
        (70, "102"),
        (71, "5"),
        (42, "35.0"),
        (3, "ISO-25"),
        (100, "AcDbOrdinateDimension"),
        (13, "35.0"),
        (23, "10.0"),
        (33, "0.0"),
        (14, "35.0"),
        (24, "20.0"),
        (34, "0.0"),
    ] {
        push(c, v);
    }
    push(0, "ENDSEC");
    push(0, "EOF");
    out.into_bytes()
}

/// `*T1` を持つ 2x2 の `ACAD_TABLE` DXF（`r2018` が真なら AC1032 + CELL_VALUE つき、偽なら AC1018）
fn table_dxf(r2018: bool) -> Vec<u8> {
    let ver = if r2018 { "AC1032" } else { "AC1018" };
    let mut out = String::new();
    let mut push = |code: i16, val: &str| {
        out.push_str(&format!("{code}\n{val}\n"));
    };
    for (c, v) in [
        (0, "SECTION"),
        (2, "HEADER"),
        (9, "$ACADVER"),
        (1, ver),
        (9, "$DWGCODEPAGE"),
        (3, "ANSI_932"),
        (9, "$INSUNITS"),
        (70, "4"),
        (9, "$HANDSEED"),
        (5, "600"),
        (0, "ENDSEC"),
        (0, "SECTION"),
        (2, "TABLES"),
        (0, "TABLE"),
        (2, "LTYPE"),
        (5, "5"),
        (330, "0"),
        (100, "AcDbSymbolTable"),
        (70, "1"),
        (0, "LTYPE"),
        (5, "14"),
        (330, "5"),
        (100, "AcDbSymbolTableRecord"),
        (100, "AcDbLinetypeTableRecord"),
        (2, "Continuous"),
        (70, "0"),
        (3, "Solid"),
        (72, "65"),
        (73, "0"),
        (40, "0.0"),
        (0, "ENDTAB"),
        (0, "TABLE"),
        (2, "LAYER"),
        (5, "2"),
        (330, "0"),
        (100, "AcDbSymbolTable"),
        (70, "1"),
        (0, "LAYER"),
        (5, "10"),
        (330, "2"),
        (100, "AcDbSymbolTableRecord"),
        (100, "AcDbLayerTableRecord"),
        (2, "0"),
        (70, "0"),
        (62, "7"),
        (6, "Continuous"),
        (0, "ENDTAB"),
        (0, "TABLE"),
        (2, "STYLE"),
        (5, "3"),
        (330, "0"),
        (100, "AcDbSymbolTable"),
        (70, "1"),
        (0, "STYLE"),
        (5, "11"),
        (330, "3"),
        (100, "AcDbSymbolTableRecord"),
        (100, "AcDbTextStyleTableRecord"),
        (2, "Standard"),
        (70, "0"),
        (40, "0.0"),
        (41, "1.0"),
        (50, "0.0"),
        (71, "0"),
        (42, "2.5"),
        (3, "txt"),
        (4, ""),
        (0, "ENDTAB"),
        (0, "TABLE"),
        (2, "BLOCK_RECORD"),
        (5, "1"),
        (330, "0"),
        (100, "AcDbSymbolTable"),
        (70, "3"),
        (0, "BLOCK_RECORD"),
        (5, "1F"),
        (330, "1"),
        (100, "AcDbSymbolTableRecord"),
        (100, "AcDbBlockTableRecord"),
        (2, "*Model_Space"),
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
        (3, "*Model_Space"),
        (1, ""),
        (0, "ENDBLK"),
        (5, "21"),
        (330, "1F"),
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
        (3, "*T1"),
        (1, ""),
        (0, "LINE"),
        (5, "52"),
        (330, "50"),
        (100, "AcDbEntity"),
        (8, "0"),
        (100, "AcDbLine"),
        (10, "0.0"),
        (20, "0.0"),
        (30, "0.0"),
        (11, "40.0"),
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
        (91, "2"),
        (92, "2"),
        (93, "0"),
        (94, "0"),
        (95, "0"),
        (96, "0"),
        (141, "8.0"),
        (141, "6.0"),
        (142, "20.0"),
        (142, "30.0"),
    ] {
        push(c, v);
    }
    let cells = [("Title", true), ("", false), ("Row1", true), ("100", true)];
    for (i, (txt, has_text)) in cells.into_iter().enumerate() {
        let span = if i == 0 { "2" } else { "1" };
        let covered = if i == 1 { "1" } else { "0" };
        for (c, v) in [
            (171, "1"),
            (172, "0"),
            (173, covered),
            (174, "0"),
            (175, "1"),
            (176, span),
            (91, "0"),
            (178, "0"),
            (145, "0.0"),
        ] {
            push(c, v);
        }
        if has_text {
            push(140, "3.0");
            push(170, "5");
        }
        if r2018 {
            push(301, "CELL_VALUE");
            if has_text {
                if txt == "100" {
                    for (c, v) in [
                        (90, "1"),
                        (93, "0"),
                        (91, "100"),
                        (94, "0"),
                        (300, "%lu2"),
                        (302, "100"),
                        (304, "ACVALUE_END"),
                    ] {
                        push(c, v);
                    }
                } else {
                    for (c, v) in [
                        (90, "4"),
                        (93, "0"),
                        (1, txt),
                        (94, "0"),
                        (300, ""),
                        (302, txt),
                        (304, "ACVALUE_END"),
                    ] {
                        push(c, v);
                    }
                }
            } else {
                for (c, v) in [(90, "0"), (93, "1"), (302, ""), (304, "ACVALUE_END")] {
                    push(c, v);
                }
            }
        } else if has_text {
            push(1, txt);
        } else {
            push(1, "");
        }
    }
    for (c, v) in [
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
        (7, "Standard"),
        (140, "2.5"),
        (170, "5"),
        (62, "7"),
        (7, "Standard"),
        (140, "3.5"),
        (170, "5"),
        (62, "7"),
        (7, "Standard"),
        (140, "3.0"),
        (170, "5"),
        (62, "7"),
        (0, "ENDSEC"),
        (0, "EOF"),
    ] {
        push(c, v);
    }
    out.into_bytes()
}

#[test]
fn replaces_and_transforms_all_dimension_subtypes_and_rebuilds_blocks() {
    let mut file = read_dxf(&all_dimensions_dxf());
    let before = lift_dxf(&file);
    assert_eq!(before.spaces[0].elements.len(), 7);

    // 1. Linear: 2 点の距離を 100 -> 160 に変え、文字が自動で中央（80, 20）に寄り、`*D1` が作り直される
    let (target, mut elem) = seen_model(&before, 0);
    let Shape::Dimension(ref mut dim) = elem.shape else {
        panic!("Dimension でない");
    };
    dim.kind = DimensionKind::Linear {
        first: mpt(0.0, 0.0),
        second: mpt(160.0, 0.0),
        line: mpt(160.0, 20.0),
        rotation: 0.0,
        oblique: 0.0,
    };
    dim.text.moved = false;

    // 2. Aligned (*D2, TICK): 斜線（tick）で部品が作り直される
    let (target2, mut elem2) = seen_model(&before, 1);
    let Shape::Dimension(ref mut dim2) = elem2.shape else {
        panic!("Dimension でない");
    };
    dim2.kind = DimensionKind::Aligned {
        first: mpt(0.0, 0.0),
        second: mpt(60.0, 80.0),
        line: mpt(50.0, 90.0),
        oblique: 0.0,
    };

    // 3. Radius (*D3, BLK): カスタムブロック矢印（INSERT ARR）で部品が作り直される
    let (target3, mut elem3) = seen_model(&before, 2);
    let Shape::Dimension(ref mut dim3) = elem3.shape else {
        panic!("Dimension でない");
    };
    dim3.kind = DimensionKind::Radius {
        center: mpt(0.0, 0.0),
        chord: mpt(40.0, 0.0),
        leader: mlen(5.0),
    };
    dim3.label = Label::Override {
        content: "<>\\P注記".to_owned(),
        spans: Vec::new(),
    };

    // 4. Diameter (*D4): 直径を 40 -> 60 に変え、文字を利用者指定の位置へ動かし回転させる
    let (target4, mut elem4) = seen_model(&before, 3);
    let Shape::Dimension(ref mut dim4) = elem4.shape else {
        panic!("Dimension でない");
    };
    dim4.kind = DimensionKind::Diameter {
        chord: mpt(30.0, 0.0),
        opposite: mpt(-30.0, 0.0),
        leader: mlen(0.0),
    };
    dim4.text.position = Some(mpt(5.0, 8.0));
    dim4.text.moved = true;
    dim4.text.rotation = Some(15.0);

    // 5. Angular 3-pt (*D5): 文字を Hidden にする
    let (target5, mut elem5) = seen_model(&before, 4);
    let Shape::Dimension(ref mut dim5) = elem5.shape else {
        panic!("Dimension でない");
    };
    dim5.label = Label::Hidden;

    // 6. Angular 2-line (*D6): Transform で平行移動 + 90 度回転
    let (target6, _) = seen_model(&before, 5);

    // 7. Ordinate (*D7): X -> Y 軸と端点を変える
    let (target7, mut elem7) = seen_model(&before, 6);
    let Shape::Dimension(ref mut dim7) = elem7.shape else {
        panic!("Dimension でない");
    };
    dim7.kind = DimensionKind::Ordinate {
        origin: mpt(0.0, 0.0),
        feature: mpt(10.0, 45.0),
        leader_end: mpt(25.0, 45.0),
        axis: Axis::Y,
    };

    let changes = Changes {
        document: None,
        edits: vec![
            Edit::Replace {
                target,
                element: Box::new(Placed::Model(elem)),
            },
            Edit::Replace {
                target: target2,
                element: Box::new(Placed::Model(elem2)),
            },
            Edit::Replace {
                target: target3,
                element: Box::new(Placed::Model(elem3)),
            },
            Edit::Replace {
                target: target4,
                element: Box::new(Placed::Model(elem4)),
            },
            Edit::Replace {
                target: target5,
                element: Box::new(Placed::Model(elem5)),
            },
            Edit::Transform {
                target: target6,
                transform: Affine {
                    m: [0.0, -1.0, 10.0, 1.0, 0.0, 20.0],
                },
            },
            Edit::Replace {
                target: target7,
                element: Box::new(Placed::Model(elem7)),
            },
        ],
    };

    let (applied, diag) = cx_run(|cx| file.apply(&changes, cx).unwrap());
    assert!(diag.is_empty(), "{diag:?}");
    assert_eq!(applied.len(), 7);
    assert!(applied.iter().all(|a| a.id.is_some() && a.hash.is_some()));

    let re_bytes = write_dxf(&file);
    let mut reopened = read_dxf(&re_bytes);
    let after = lift_dxf(&reopened);

    // 1. Linear: 42 が 160、文字位置が自動で (80, 20)、*D1 に LINE・SOLID・MTEXT("160") がある
    let Shape::Dimension(d1) = &after.spaces[0].elements[0].shape else {
        panic!();
    };
    assert!((d1.measurement.as_ref().unwrap().value - 160.0).abs() < 1e-9);
    assert!(!d1.text.moved);
    assert!((d1.text.position.unwrap().x - 80.0).abs() < 1e-9);
    let d1_parts = symbol_elements(&after, "*D1");
    assert!(
        d1_parts
            .iter()
            .any(|e| matches!(&e.shape, Shape::Formatted(t) if t.content == "160"))
    );
    let db = cx_run(|cx| reopened.objects(cx).unwrap()).0;
    let d1_record = db
        .objects()
        .iter()
        .find(|o| o.kind == b"BLOCK_RECORD" && o.first(2) == Some(&Data::Text(b"*D1".to_vec())))
        .unwrap()
        .handle()
        .unwrap();
    let solid_count = db
        .objects()
        .iter()
        .filter(|o| o.owner() == Some(d1_record) && o.kind == b"SOLID")
        .count();
    assert_eq!(solid_count, 2);

    // 2. Aligned (*D2, TICK): SOLID はなく、斜線も含めて LINE と MTEXT("100") がある
    let d2_parts = symbol_elements(&after, "*D2");
    assert!(
        d2_parts
            .iter()
            .any(|e| matches!(&e.shape, Shape::Formatted(t) if t.content == "100"))
    );
    assert!(
        d2_parts
            .iter()
            .filter(|e| matches!(e.shape, Shape::Line { .. }))
            .count()
            >= 5
    );

    // 3. Radius (*D3, BLK): カスタム矢印 INSERT("ARR") と MTEXT("R40\\P注記") がある
    let d3_parts = symbol_elements(&after, "*D3");
    assert!(
        d3_parts
            .iter()
            .any(|e| matches!(&e.shape, Shape::Symbol(s) if s.key.name == "ARR"))
    );
    assert!(
        d3_parts
            .iter()
            .any(|e| matches!(&e.shape, Shape::Formatted(t) if t.content == "R40\\P注記"))
    );

    // 4. Diameter (*D4): moved = true、text_position = (5, 8)、rotation = 15、MTEXT("%%c60")
    let Shape::Dimension(d4) = &after.spaces[0].elements[3].shape else {
        panic!();
    };
    assert!(d4.text.moved);
    assert!((d4.text.position.unwrap().x - 5.0).abs() < 1e-9);
    assert!((d4.text.position.unwrap().y - 8.0).abs() < 1e-9);
    assert!((d4.text.rotation.unwrap() - 15.0).abs() < 1e-9);
    let d4_parts = symbol_elements(&after, "*D4");
    assert!(
        d4_parts
            .iter()
            .any(|e| matches!(&e.shape, Shape::Formatted(t) if t.content == "\\U+220560"))
    );

    // 5. Angular 3-pt (*D5): Hidden のため MTEXT がなく、ARC がある
    let d5_parts = symbol_elements(&after, "*D5");
    assert!(
        !d5_parts
            .iter()
            .any(|e| matches!(e.shape, Shape::Formatted(_)))
    );
    assert!(
        d5_parts
            .iter()
            .any(|e| matches!(e.shape, Shape::Curve(Edge::Arc { .. })))
    );

    // 7. Ordinate (*D7): Y 軸になり測定値が 45
    let Shape::Dimension(d7) = &after.spaces[0].elements[6].shape else {
        panic!();
    };
    assert!((d7.measurement.as_ref().unwrap().value - 45.0).abs() < 1e-9);

    // 続けて 2 回目の apply（同じ *D1 をさらに Transform）も通る
    let (next_target, _) = seen_model(&after, 0);
    let second_changes = Changes {
        document: None,
        edits: vec![Edit::Transform {
            target: next_target,
            transform: Affine {
                m: [1.0, 0.0, 10.0, 0.0, 1.0, 5.0],
            },
        }],
    };
    let (applied2, diag2) = cx_run(|cx| reopened.apply(&second_changes, cx).unwrap());
    assert!(diag2.is_empty(), "{diag2:?}");
    assert_eq!(applied2.len(), 1);
}

#[test]
fn replaces_and_transforms_arc_dimensions_on_autocad_samples() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../samples/dwg");
    for name in [
        "zumen-arcdim-r2004_ascii.dxf",
        "zumen-arcdim-r2018_ascii.dxf",
    ] {
        let raw = std::fs::read(dir.join(name)).unwrap();
        let mut file = read_dxf(&raw);
        let before = lift_dxf(&file);

        let arc_indices: Vec<usize> = before.spaces[0]
            .elements
            .iter()
            .enumerate()
            .filter_map(|(i, e)| match &e.shape {
                Shape::Dimension(d) if matches!(d.kind, DimensionKind::ArcLength { .. }) => Some(i),
                _ => None,
            })
            .collect();
        // 部分（70=1）と引出線（164=1）を除く弧長寸法は書き戻せる
        let (target0, mut el0) = seen_model(&before, arc_indices[0]);
        let Shape::Dimension(ref mut d0) = el0.shape else {
            panic!();
        };
        let DimensionKind::ArcLength {
            center,
            first,
            second,
            arc,
        } = d0.kind
        else {
            panic!();
        };
        d0.kind = DimensionKind::ArcLength {
            center,
            first,
            second,
            arc: mpt(arc.x, arc.y + 15.0),
        };
        d0.text.moved = false;

        let (target1, _) = seen_model(&before, arc_indices[1]);
        let changes = Changes {
            document: None,
            edits: vec![
                Edit::Replace {
                    target: target0,
                    element: Box::new(Placed::Model(el0)),
                },
                Edit::Transform {
                    target: target1,
                    transform: Affine {
                        m: [1.0, 0.0, 25.0, 0.0, 1.0, -10.0],
                    },
                },
            ],
        };
        let (applied, diag) = cx_run(|cx| file.apply(&changes, cx).unwrap());
        assert!(diag.is_empty(), "{name}: {diag:?}");
        assert_eq!(applied.len(), 2);

        let out_bytes = write_dxf(&file);
        let reopened = read_dxf(&out_bytes);
        let after = lift_dxf(&reopened);

        // DIMARCSYM=1（2 つ目の弧長寸法）は XDATA 上書きを保ち、MTEXT が `\U+2312\P` で始まる
        let Shape::Dimension(after_d1) = &after.spaces[0].elements[arc_indices[1]].shape else {
            panic!();
        };
        let blk1 = after_d1.block.as_deref().unwrap();
        let parts1 = symbol_elements(&after, blk1);
        assert!(
            parts1.iter().any(
                |e| matches!(&e.shape, Shape::Formatted(t) if t.content.starts_with("\\U+2312\\P"))
            ),
            "{name}: {parts1:?}"
        );

        // 部分（4 つ目）と引出線つき（5 つ目）は安全に拒む
        for &unsupported_idx in &arc_indices[3..5] {
            let mut copy = read_dxf(&raw);
            let (t_bad, _) = seen_model(&before, unsupported_idx);
            let bad_changes = Changes {
                document: None,
                edits: vec![Edit::Transform {
                    target: t_bad,
                    transform: Affine {
                        m: [1.0, 0.0, 5.0, 0.0, 1.0, 0.0],
                    },
                }],
            };
            let (res, err_diag) = cx_run(|cx| copy.apply(&bad_changes, cx));
            assert!(matches!(res, Err(Error::Rejected)), "{name}");
            assert_eq!(
                err_diag.iter().next().unwrap().code(),
                codes::CHANGE_EDIT_UNSUPPORTED
            );
        }
    }
}

#[test]
fn replaces_and_transforms_dwg_dimensions_across_samples() {
    for path in support::sample_files() {
        if path.extension().is_none_or(|e| e != "dwg") {
            continue;
        }
        let Ok(src) = std::fs::read(&path) else {
            continue;
        };
        if ![b"AC1015", b"AC1018", b"AC1024", b"AC1027", b"AC1032"]
            .iter()
            .any(|v| src.starts_with(*v))
        {
            continue;
        }
        let Ok(dwg): Result<DwgFile, _> = cx_run(|cx| Dwg::read(&src, cx)).0 else {
            continue;
        };
        let Ok(drawing): Result<Drawing2D, _> = cx_run(|cx| dwg.lift(cx)).0 else {
            continue;
        };
        let Some(space) = drawing.spaces.first() else {
            continue;
        };
        let place = id::space(space.id);
        let mut namer = id::Namer::new(&place);
        for (i, el) in space.elements.iter().enumerate() {
            let elem_id = namer.next(i, el);
            let Shape::Dimension(dim) = &el.shape else {
                continue;
            };
            if dim.block.is_none() || matches!(dim.kind, DimensionKind::ArcLength { .. }) {
                continue;
            }
            let target = Seen {
                id: elem_id,
                hash: Hash::element(el),
            };
            let changes = Changes {
                document: None,
                edits: vec![Edit::Transform {
                    target,
                    transform: Affine {
                        m: [1.0, 0.0, 12.0, 0.0, 1.0, -8.0],
                    },
                }],
            };
            let mut trial = cx_run(|cx| dwg.snapshot(cx).unwrap()).0;
            if cx_run(|cx| trial.apply(&changes, cx)).0.is_ok() {
                let written = cx_run(|cx| {
                    let mut out = Vec::new();
                    Dwg::write(&trial, &mut out, cx).unwrap();
                    out
                })
                .0;
                let reread = cx_run(|cx| Dwg::read(&written, cx).unwrap()).0;
                let after = cx_run(|cx| reread.lift(cx).unwrap()).0;
                let Shape::Dimension(after_dim) = &after.spaces[0].elements[i].shape else {
                    panic!("DWG 書き戻し後に Dimension でなくなった");
                };
                let blk = after_dim.block.as_deref().unwrap();
                assert!(!symbol_elements(&after, blk).is_empty());
                break;
            }
        }
    }
}

#[test]
fn replaces_and_transforms_dxf_tables_across_r2004_and_r2018() {
    for r2018 in [false, true] {
        let mut file = read_dxf(&table_dxf(r2018));
        let before = lift_dxf(&file);
        let (target, mut elem) = seen_model(&before, 0);
        let Shape::Table(ref mut table) = elem.shape else {
            panic!("Table でない");
        };
        assert_eq!(table.cells.len(), 4);
        if r2018 {
            assert!(table.cells[3].value.is_some());
        }

        // 250 バイトを超えるマルチバイト文字列の分割も確かめる
        let long_jp = "あいうえお".repeat(25);
        assert!(long_jp.len() > 300);

        // セル 0: 文字を長い日本語へ変更
        table.cells[0].content = CellContent::Text {
            content: long_jp.clone(),
            spans: Vec::new(),
        };
        // セル 2: 文字 -> Empty へ変更
        table.cells[2].content = CellContent::Empty;
        // セル 3: 数値セル（100）の文字を "250" に変更 -> 型付き CELL_VALUE は文字列型へ正規化される
        table.cells[3].content = CellContent::Text {
            content: "250".to_owned(),
            spans: Vec::new(),
        };
        table.cells[3].value = None;
        // 列幅と位置も変更
        table.column_widths[0] = mlen(25.0);
        table.position = mpt(15.0, 60.0);

        let changes = Changes {
            document: None,
            edits: vec![Edit::Replace {
                target,
                element: Box::new(Placed::Model(elem)),
            }],
        };
        let (applied, diag) = cx_run(|cx| file.apply(&changes, cx).unwrap());
        assert!(diag.is_empty(), "r2018={r2018}: {diag:?}");
        assert_eq!(applied.len(), 1);

        let re_bytes = write_dxf(&file);
        let mut reopened = read_dxf(&re_bytes);
        let after = lift_dxf(&reopened);
        let Shape::Table(after_table) = &after.spaces[0].elements[0].shape else {
            panic!();
        };
        assert_eq!(after_table.position, mpt(15.0, 60.0));
        assert_eq!(after_table.column_widths[0], mlen(25.0));
        assert!(matches!(
            &after_table.cells[0].content,
            CellContent::Text { content, .. } if content == &long_jp
        ));
        assert_eq!(after_table.cells[2].content, CellContent::Empty);
        assert!(matches!(
            &after_table.cells[3].content,
            CellContent::Text { content, .. } if content == "250"
        ));
        if r2018 {
            // 空にしたセル 2 は None、文字を変えた数値セル 3 は文字列型の CellValue になる
            assert!(after_table.cells[2].value.is_none());
            assert_eq!(
                after_table.cells[3].value.as_ref().map(|v| &v.data),
                Some(&zumen_drawing::CellData::Text("250".to_owned()))
            );
        }

        // *T1 に罫線（LINE）と 2 つの非空セルの MTEXT が作り直されている
        let t1_parts = symbol_elements(&after, "*T1");
        let mtexts: Vec<&str> = t1_parts
            .iter()
            .filter_map(|e| match &e.shape {
                Shape::Formatted(t) => Some(t.content.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(mtexts.len(), 2);
        assert!(mtexts.contains(&long_jp.as_str()));
        assert!(mtexts.contains(&"250"));

        // 続けて Transform（平行移動 + 90 度回転）も通る
        let (target2, _) = seen_model(&after, 0);
        let tr = Changes {
            document: None,
            edits: vec![Edit::Transform {
                target: target2,
                transform: Affine {
                    m: [0.0, -1.0, 5.0, 1.0, 0.0, 10.0],
                },
            }],
        };
        let (applied2, diag2) = cx_run(|cx| reopened.apply(&tr, cx).unwrap());
        assert!(diag2.is_empty(), "r2018={r2018}: {diag2:?}");
        assert_eq!(applied2.len(), 1);
        let rotated = lift_dxf(&read_dxf(&write_dxf(&reopened)));
        let Shape::Table(rot_table) = &rotated.spaces[0].elements[0].shape else {
            panic!();
        };
        assert!(rot_table.direction[0].abs() < 1e-9);
        assert!((rot_table.direction[1] - 1.0).abs() < 1e-9);
    }
}

#[test]
fn replaces_and_transforms_paper_space_dimension() {
    // all_dimensions_dxf の 1 つ目の DIMENSION を *Paper_Space（67=1、330=1B）に置き、*D1 の中身にも 67=1 を付ける
    let model = String::from_utf8(all_dimensions_dxf()).unwrap();
    let paper = model
        .replace(
            "0\nDIMENSION\n5\n201\n330\n1F\n100\nAcDbEntity\n8\n0\n",
            "0\nDIMENSION\n5\n201\n330\n1B\n100\nAcDbEntity\n67\n1\n8\n0\n",
        )
        .replace(
            "0\nLINE\n5\n10B\n330\n41\n100\nAcDbEntity\n8\n0\n",
            "0\nLINE\n5\n10B\n330\n41\n100\nAcDbEntity\n67\n1\n8\n0\n",
        );
    let mut file = read_dxf(paper.as_bytes());
    let before = lift_dxf(&file);
    assert_eq!(before.sheets.len(), 1);
    assert_eq!(before.sheets[0].elements.len(), 1);

    let sheet = &before.sheets[0];
    let mut sheet_namer = id::SheetNamer::default();
    let place = sheet_namer.next(sheet);
    let mut namer = id::Namer::new(&place);
    let el = &sheet.elements[0];
    let target = Seen {
        id: namer.next(0, el),
        hash: Hash::element(el),
    };
    let mut updated = el.clone();
    let sheet_id = sheet.id;
    let Shape::Dimension(ref mut dim) = updated.shape else {
        panic!();
    };
    dim.kind = DimensionKind::Linear {
        first: PaperPoint {
            sheet: sheet_id,
            x: 0.0,
            y: 0.0,
        },
        second: PaperPoint {
            sheet: sheet_id,
            x: 120.0,
            y: 0.0,
        },
        line: PaperPoint {
            sheet: sheet_id,
            x: 120.0,
            y: 25.0,
        },
        rotation: 0.0,
        oblique: 0.0,
    };
    dim.measurement = None;
    let _ = PaperLength(1.0);

    let changes = Changes {
        document: None,
        edits: vec![Edit::Replace {
            target,
            element: Box::new(Placed::Paper(updated)),
        }],
    };
    let (applied, diag) = cx_run(|cx| file.apply(&changes, cx).unwrap());
    assert!(diag.is_empty(), "{diag:?}");
    assert_eq!(applied.len(), 1);

    let after = lift_dxf(&read_dxf(&write_dxf(&file)));
    let Shape::Dimension(after_dim) = &after.sheets[0].elements[0].shape else {
        panic!();
    };
    assert!((after_dim.measurement.as_ref().unwrap().value - 120.0).abs() < 1e-9);
}

#[test]
fn rejects_conflicting_or_shared_blocks_and_obeys_work_budget() {
    // 1. 2 つの DIMENSION が同じ *D1 を共有するときは書き戻さない
    let shared_src = String::from_utf8(all_dimensions_dxf()).unwrap().replace(
        "0\nDIMENSION\n5\n202\n330\n1F\n100\nAcDbEntity\n8\n0\n100\nAcDbDimension\n2\n*D2\n",
        "0\nDIMENSION\n5\n202\n330\n1F\n100\nAcDbEntity\n8\n0\n100\nAcDbDimension\n2\n*D1\n",
    );
    let mut shared_file = read_dxf(shared_src.as_bytes());
    let shared_drawing = lift_dxf(&shared_file);
    let (t0, _) = seen_model(&shared_drawing, 0);
    let changes = Changes {
        document: None,
        edits: vec![Edit::Transform {
            target: t0,
            transform: Affine {
                m: [1.0, 0.0, 5.0, 0.0, 1.0, 0.0],
            },
        }],
    };
    let (res, diag) = cx_run(|cx| shared_file.apply(&changes, cx));
    assert!(matches!(res, Err(Error::Rejected)));
    assert_eq!(
        diag.iter().next().unwrap().code(),
        codes::CHANGE_EDIT_UNSUPPORTED
    );

    // 2. 同じ Changes の中で *D1 の中の図形と親の DIMENSION を同時に変えることはできない
    let raw = all_dimensions_dxf();
    let mut file = read_dxf(&raw);
    let drawing = lift_dxf(&file);
    let (dim_target, _) = seen_model(&drawing, 0);
    let mut def_namer = id::DefinitionNamer::default();
    let mut inner_seen = None;
    for def in &drawing.symbols {
        let place = def_namer.next(def);
        if def.key.name == "*D1" {
            let mut namer = id::Namer::new(&place);
            let el = &def.space.elements[0];
            inner_seen = Some(Seen {
                id: namer.next(0, el),
                hash: Hash::element(el),
            });
            break;
        }
    }
    let conflict = Changes {
        document: None,
        edits: vec![
            Edit::Transform {
                target: dim_target.clone(),
                transform: Affine {
                    m: [1.0, 0.0, 5.0, 0.0, 1.0, 0.0],
                },
            },
            Edit::Remove {
                target: inner_seen.unwrap(),
            },
        ],
    };
    let (res2, _) = cx_run(|cx| file.apply(&conflict, cx));
    assert!(matches!(res2, Err(Error::Rejected)));
    assert_eq!(write_dxf(&file), raw);

    // 3. 非等方スケールは拒む
    let non_uniform = Changes {
        document: None,
        edits: vec![Edit::Transform {
            target: dim_target.clone(),
            transform: Affine {
                m: [2.0, 0.0, 0.0, 0.0, 1.0, 0.0],
            },
        }],
    };
    assert!(matches!(
        cx_run(|cx| file.apply(&non_uniform, cx)).0,
        Err(Error::Rejected)
    ));

    // 4. DWG の ARC_DIMENSION は拒む
    let arcdim_dwg = std::fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../samples/dwg/zumen-arcdim-r2018.dwg"),
    )
    .unwrap();
    let mut dwg = cx_run(|cx| Dwg::read(&arcdim_dwg, cx).unwrap()).0;
    let dwg_drawing = cx_run(|cx| dwg.lift(cx).unwrap()).0;
    let arc_idx = dwg_drawing.spaces[0]
        .elements
        .iter()
        .position(|e| matches!(e.shape, Shape::Dimension(_)))
        .unwrap();
    let (dwg_target, _) = seen_model(&dwg_drawing, arc_idx);
    let dwg_changes = Changes {
        document: None,
        edits: vec![Edit::Transform {
            target: dwg_target,
            transform: Affine {
                m: [1.0, 0.0, 5.0, 0.0, 1.0, 0.0],
            },
        }],
    };
    assert!(matches!(
        cx_run(|cx| dwg.apply(&dwg_changes, cx)).0,
        Err(Error::Rejected)
    ));

    // 5. Work 予算で途中で止まっても元の DXF を変えない
    let ok_changes = Changes {
        document: None,
        edits: vec![Edit::Transform {
            target: dim_target,
            transform: Affine {
                m: [1.0, 0.0, 10.0, 0.0, 1.0, 0.0],
            },
        }],
    };
    let cancel = CancelToken::new();
    let mut failed = 0;
    let mut succeeded = false;
    for limit in (0..20_000).step_by(50) {
        let mut f = read_dxf(&raw);
        let mut budget = Budget::new(Limits::default().with(Kind::Work, Limit::Finite(limit)));
        let mut d = Diagnostics::new();
        if f.apply(&ok_changes, &mut Cx::new(&mut budget, &mut d, &cancel))
            .is_ok()
        {
            succeeded = true;
            break;
        }
        failed += 1;
        assert_eq!(write_dxf(&f), raw, "work={limit}");
    }
    assert!(failed > 0 && succeeded);
}

#[test]
fn linear_dimension_places_angles_before_rotated_subclass_and_dwg_removes_owned_xdictionary() {
    // 1. DXF の AcDbRotatedDimension では 50（寸法線の角度）と 52（斜角）が 100 AcDbRotatedDimension の前に入る
    let raw = all_dimensions_dxf();
    let mut file = read_dxf(&raw);
    let before = lift_dxf(&file);
    let (target, mut el) = seen_model(&before, 0);
    let Shape::Dimension(ref mut dim) = el.shape else {
        panic!();
    };
    dim.kind = DimensionKind::Linear {
        first: mpt(0.0, 0.0),
        second: mpt(100.0, 0.0),
        line: mpt(100.0, 20.0),
        rotation: 15.0,
        oblique: 75.0,
    };
    let changes = Changes {
        document: None,
        edits: vec![Edit::Replace {
            target,
            element: Box::new(Placed::Model(el)),
        }],
    };
    cx_run(|cx| file.apply(&changes, cx).unwrap());
    let written = write_dxf(&file);
    let reopened = read_dxf(&written);
    let db = cx_run(|cx| reopened.objects(cx).unwrap()).0;
    let dim_obj = db
        .objects()
        .iter()
        .find(|o| o.kind == b"DIMENSION")
        .unwrap();
    let pos_50 = dim_obj.values.iter().position(|v| v.code == 50).unwrap();
    let pos_52 = dim_obj.values.iter().position(|v| v.code == 52).unwrap();
    let pos_rot = dim_obj
        .values
        .iter()
        .position(|v| {
            v.code == 100 && matches!(&v.data, Data::Text(t) if t == b"AcDbRotatedDimension")
        })
        .unwrap();
    assert!(pos_50 < pos_52 && pos_52 < pos_rot);

    // 2. DWG でも、所有する拡張辞書の木を持つ図形（zumen-xclip-r2013.dwg の切り抜きつき INSERT）を木ごと除ける
    let xclip_dwg = std::fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../samples/dwg/zumen-xclip-r2013.dwg"),
    )
    .unwrap();
    let mut dwg = cx_run(|cx| Dwg::read(&xclip_dwg, cx).unwrap()).0;
    let dwg_before = cx_run(|cx| dwg.lift(cx).unwrap()).0;
    let clip_idx = dwg_before.spaces[0]
        .elements
        .iter()
        .position(|e| matches!(&e.shape, Shape::Symbol(s) if s.clip.is_some()))
        .unwrap();
    let (clip_target, _) = seen_model(&dwg_before, clip_idx);
    let remove_clip = Changes {
        document: None,
        edits: vec![Edit::Remove {
            target: clip_target,
        }],
    };
    let (_, diag) = cx_run(|cx| dwg.apply(&remove_clip, cx).unwrap());
    assert!(diag.is_empty(), "{diag:?}");
    let dwg_out = cx_run(|cx| {
        let mut out = Vec::new();
        Dwg::write(&dwg, &mut out, cx).unwrap();
        out
    })
    .0;
    let dwg_reopened = cx_run(|cx| Dwg::read(&dwg_out, cx).unwrap()).0;
    let dwg_after = cx_run(|cx| dwg_reopened.lift(cx).unwrap()).0;
    assert_eq!(
        dwg_after.spaces[0].elements.len() + 1,
        dwg_before.spaces[0].elements.len()
    );
}
