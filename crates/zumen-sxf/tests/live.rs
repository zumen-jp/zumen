//! `SxfLive` の契約検査（`docs/live.md`、#878）。
//! SFC・P21 の双方について、現行の `SxfFile::apply` + `Sxf::write` とのバイト列完全一致、
//! 常駐図面モデル・ID・ハッシュ・診断・SAF 属性結合の一致、多段 `undo` / `redo`、
//! シート・部分図の空間索引と差分表示バッファー、並列読み込みの決定性、予算・中断耐性を確かめる。

#![cfg(feature = "drawing")]

use std::collections::BTreeSet;
use std::path::PathBuf;

use zumen_base::{
    Budget, CancelToken, Code, Cx, Diagnostics, Key, Kind, Limit, Limits, Ref, codes,
};
use zumen_drawing::{
    Affine, Applied, Changes, Color, ColorCode, ColorSystem, Drawing2D, Edge, Edit, Element, Hash,
    Inherit, LayerId, Length, LineTypeId, LineWeight, MarkerKey, ModelPoint, PaperLength,
    PaperPoint, Placed, Point, PointLook, PointMark, PointSize, PointStyle, Polyline, Restyle,
    Seen, Shape, SheetId, SpaceId, Style, Vertex, id,
};
use zumen_live::display::Display;
use zumen_sdk::{Apply, Codec, Error, Lift, testkit};
use zumen_sxf::saf::{self, AttributeFile};
use zumen_sxf::{Mode, Sxf, SxfFile, SxfLive};

#[cfg(test)]
fn read_file(src: &[u8]) -> SxfFile {
    testkit::read::<Sxf>(src).0.unwrap()
}

#[cfg(test)]
fn lift_file(file: &SxfFile) -> (Drawing2D, Vec<Code>) {
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let d = file
        .lift(&mut Cx::new(&mut budget, &mut diag, &cancel))
        .unwrap();
    (d, diag.iter().map(|x| x.code()).collect())
}

#[cfg(test)]
fn write_file_bytes(file: &SxfFile) -> Vec<u8> {
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let mut out = Vec::new();
    Sxf::write(
        file,
        &mut out,
        &mut Cx::new(&mut budget, &mut diag, &cancel),
    )
    .unwrap();
    out
}

#[cfg(test)]
fn open_live(src: &[u8]) -> (SxfLive, Vec<Code>) {
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let live = SxfLive::open(src, &mut Cx::new(&mut budget, &mut diag, &cancel)).unwrap();
    (live, diag.iter().map(|x| x.code()).collect())
}

#[cfg(test)]
fn live_drawing(live: &SxfLive) -> (Drawing2D, Vec<Code>) {
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let d = live
        .drawing(&mut Cx::new(&mut budget, &mut diag, &cancel))
        .unwrap();
    (d, diag.iter().map(|x| x.code()).collect())
}

#[cfg(test)]
fn live_save(live: &SxfLive) -> Vec<u8> {
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    live.save(&mut Cx::new(&mut budget, &mut diag, &cancel))
        .unwrap()
}

#[cfg(test)]
fn live_undo(live: &mut SxfLive) -> bool {
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    live.undo(&mut Cx::new(&mut budget, &mut diag, &cancel))
        .unwrap()
}

#[cfg(test)]
fn live_redo(live: &mut SxfLive) -> bool {
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    live.redo(&mut Cx::new(&mut budget, &mut diag, &cancel))
        .unwrap()
}

#[cfg(test)]
#[expect(clippy::type_complexity)]
fn apply_both(
    file: &mut SxfFile,
    live: &mut SxfLive,
    edits: Vec<Edit>,
    document: Option<Hash>,
) -> (
    (Result<Vec<Applied>, Error>, Vec<Code>),
    (Result<Vec<Applied>, Error>, Vec<Code>),
) {
    let changes = Changes { edits, document };

    let mut b1 = Budget::default();
    let mut d1 = Diagnostics::new();
    let c1 = CancelToken::new();
    let r1 = file.apply(&changes, &mut Cx::new(&mut b1, &mut d1, &c1));
    let codes1: Vec<_> = d1.iter().map(|d| d.code()).collect();

    let mut b2 = Budget::default();
    let mut d2 = Diagnostics::new();
    let c2 = CancelToken::new();
    let r2 = live.apply(&changes, &mut Cx::new(&mut b2, &mut d2, &c2));
    let codes2: Vec<_> = d2.iter().map(|d| d.code()).collect();

    ((r1, codes1), (r2, codes2))
}

#[cfg(test)]
fn strip_ref_offsets(r: &Ref) -> Option<Key> {
    r.key()
}

#[cfg(test)]
fn collect_named_elements(drawing: &Drawing2D) -> Vec<(String, Hash, Option<Key>)> {
    let mut out = Vec::new();
    for space in &drawing.spaces {
        let place = id::space(space.id);
        let mut namer = id::Namer::new(&place);
        for (idx, elem) in space.elements.iter().enumerate() {
            out.push((
                namer.next(idx, elem),
                Hash::element(elem),
                strip_ref_offsets(&elem.source),
            ));
        }
    }
    let mut def_namer = id::DefinitionNamer::default();
    for sym in &drawing.symbols {
        let place = def_namer.next(sym);
        let mut namer = id::Namer::new(&place);
        for (idx, elem) in sym.space.elements.iter().enumerate() {
            out.push((
                namer.next(idx, elem),
                Hash::element(elem),
                strip_ref_offsets(&elem.source),
            ));
        }
    }
    for sheet in &drawing.sheets {
        let place = id::sheet(sheet);
        let mut namer = id::Namer::new(&place);
        for (idx, elem) in sheet.elements.iter().enumerate() {
            out.push((
                namer.next(idx, elem),
                Hash::element(elem),
                strip_ref_offsets(&elem.source),
            ));
        }
    }
    out
}

#[cfg(test)]
fn normalize_drawing(mut d: Drawing2D) -> Drawing2D {
    let clear = |r: &mut Ref| {
        *r = match r.key() {
            Some(k) => Ref::default().with_key(k),
            None => Ref::default(),
        };
    };
    for sp in &mut d.spaces {
        for e in &mut sp.elements {
            clear(&mut e.source);
        }
    }
    for sym in &mut d.symbols {
        clear(&mut sym.source);
        for e in &mut sym.space.elements {
            clear(&mut e.source);
        }
    }
    for sh in &mut d.sheets {
        clear(&mut sh.source);
        for v in &mut sh.views {
            clear(&mut v.source);
        }
        for e in &mut sh.elements {
            clear(&mut e.source);
        }
    }
    for l in &mut d.layers {
        clear(&mut l.source);
    }
    for lt in &mut d.line_types {
        clear(&mut lt.source);
    }
    for tb in &mut d.title_blocks {
        clear(&mut tb.source);
    }
    for ext in &mut d.extensions {
        clear(&mut ext.source);
    }
    d
}

#[cfg(test)]
fn assert_state_parity(file: &SxfFile, live: &SxfLive) {
    let file_bytes = write_file_bytes(file);
    let saved_bytes = live_save(live);
    assert_eq!(
        String::from_utf8_lossy(&saved_bytes),
        String::from_utf8_lossy(&file_bytes),
        "saved text mismatch"
    );
    assert_eq!(saved_bytes, file_bytes, "saved bytes mismatch");

    let (file_d, _) = lift_file(file);
    let (live_d, _) = live_drawing(live);
    assert_eq!(
        collect_named_elements(&live_d),
        collect_named_elements(&file_d),
        "named element IDs/hashes mismatch"
    );
    assert_eq!(
        normalize_drawing(live_d),
        normalize_drawing(file_d),
        "normalized Drawing2D mismatch"
    );
}

#[cfg(test)]
fn paper(x: f64, y: f64) -> PaperPoint {
    PaperPoint {
        sheet: SheetId(0),
        x,
        y,
    }
}

#[cfg(test)]
fn sxf_color(value: u32) -> Inherit<Color> {
    Inherit::Own(Color {
        code: Some(ColorCode {
            system: ColorSystem::Sxf,
            value,
        }),
        rgb: None,
    })
}

#[cfg(test)]
fn seen_sheet(drawing: &Drawing2D, instance: u64) -> (Seen, Element<PaperPoint>) {
    let sheet = &drawing.sheets[0];
    let place = id::sheet(sheet);
    let mut namer = id::Namer::new(&place);
    for (idx, elem) in sheet.elements.iter().enumerate() {
        let cid = namer.next(idx, elem);
        if elem.source.key() == Some(Key::Instance(instance)) {
            return (
                Seen {
                    id: cid,
                    hash: Hash::element(elem),
                },
                elem.clone(),
            );
        }
    }
    panic!("sheet element #{instance} not found");
}

#[cfg(test)]
fn seen_space(
    drawing: &Drawing2D,
    space_id: SpaceId,
    instance: u64,
) -> (Seen, Element<ModelPoint>) {
    let space = drawing
        .spaces
        .iter()
        .find(|s| s.id == space_id)
        .expect("space not found");
    let place = id::space(space.id);
    let mut namer = id::Namer::new(&place);
    for (idx, elem) in space.elements.iter().enumerate() {
        let cid = namer.next(idx, elem);
        if elem.source.key() == Some(Key::Instance(instance)) {
            return (
                Seen {
                    id: cid,
                    hash: Hash::element(elem),
                },
                elem.clone(),
            );
        }
    }
    panic!("space element #{instance} not found");
}

/// 部分図・点マーカー・複合曲線・利用者定義色・線幅を含む SFC
#[cfg(test)]
fn rich_sfc_src() -> Vec<u8> {
    let features: [&[u8]; 16] = [
        b"layer_feature(\\'A\\','1')",
        b"layer_feature(\\'B\\','1')",
        b"user_defined_colour_feature('10','20','30')",
        b"width_feature('0.3')",
        // 部分図 (flag '1') の線分 #50 -> sfig_org #60
        b"line_feature('1','1','1','1','0.0','0.0','20.0','10.0')",
        b"sfig_org_feature(\\'SUB1\\','1')",
        // 複合曲線のメンバー #70 -> composite_curve_org #80
        b"line_feature('2','1','1','1','20.0','0.0','30.0','0.0')",
        b"composite_curve_org_feature('1','1','1','1')",
        // 用紙要素 #90..#140
        b"line_feature('1','1','1','1','0.0','0.0','10.0','0.0')",
        b"circle_feature('1','2','1','2','5.0','5.0','3.0')",
        b"arc_feature('1','1','1','1','0.0','0.0','5.0','0','0.0','90.0')",
        b"polyline_feature('1','1','1','1','3','(0,1,2)','(0,1,0)')",
        b"text_string_feature('1','1','1',\\'T\\','1.0','1.0','2.0','2.0','0.0','0.0','1','1')",
        b"point_marker_feature('1','1','5.0','6.0','2','0.0','1.0')",
        b"sfig_locate_feature('1',\\'SUB1\\','100.0','200.0','0.0','1.0','1.0')",
        b"drawing_sheet_feature(\\'S\\','3','1','0','0')",
    ];
    let mut src = b"ISO-10303-21;\r\nHEADER;\r\nFILE_DESCRIPTION(('SCADEC level2 feature_mode'),'2;1');\r\nFILE_SCHEMA(('ASSOCIATIVE_DRAUGHTING'));\r\nENDSEC;\r\nDATA;\r\n".to_vec();
    for (at, feature) in features.iter().enumerate() {
        src.extend_from_slice(format!("/*SXF\r\n#{} = ", (at + 1) * 10).as_bytes());
        src.extend_from_slice(feature);
        src.extend_from_slice(b"\r\nSXF*/\r\n");
    }
    src.extend_from_slice(b"ENDSEC;\r\nEND-ISO-10303-21;\r\n");
    src
}

/// 共有スタイル・共有座標・専用線幅・点マーカー・複合曲線を含む P21
#[cfg(test)]
fn rich_p21_src() -> Vec<u8> {
    let data = [
        "#10=(LENGTH_UNIT()NAMED_UNIT(*)SI_UNIT(.MILLI.,.METRE.));",
        "#20=DRAUGHTING_PRE_DEFINED_COLOUR('red');",
        "#21=DRAUGHTING_PRE_DEFINED_COLOUR('green');",
        "#30=DRAUGHTING_PRE_DEFINED_CURVE_FONT('continuous');",
        "#40=LENGTH_MEASURE_WITH_UNIT(POSITIVE_LENGTH_MEASURE(0.25),#10);",
        "#45=LENGTH_MEASURE_WITH_UNIT(POSITIVE_LENGTH_MEASURE(0.42),#10);",
        "#50=COLOUR_RGB('',0.2,0.4,0.6);",
        "#60=CURVE_STYLE(' ',#30,#40,#20);",
        "#65=CURVE_STYLE(' ',#30,#45,#20);",
        "#70=PRESENTATION_STYLE_ASSIGNMENT((#60));",
        "#75=PRESENTATION_STYLE_ASSIGNMENT((#65));",
        "#80=SYMBOL_COLOUR(#21);",
        "#81=SYMBOL_STYLE(' ',#80);",
        "#82=PRESENTATION_STYLE_ASSIGNMENT((#81));",
        "#83=CARTESIAN_POINT(' ',(5.,6.));",
        "#84=DIRECTION(' ',(1.,0.));",
        "#85=AXIS2_PLACEMENT_2D(' ',#83,#84);",
        "#86=SYMBOL_TARGET(' ',#85,1.5,1.5);",
        "#87=PRE_DEFINED_POINT_MARKER_SYMBOL('circle');",
        "#88=DEFINED_SYMBOL(' ',#87,#86);",
        "#89=ANNOTATION_SYMBOL_OCCURRENCE(' ',(#82),#88);",
        "#100=CARTESIAN_POINT(' ',(0.,0.));",
        "#110=CARTESIAN_POINT(' ',(10.,0.));",
        "#120=DIRECTION(' ',(1.,0.));",
        "#130=VECTOR(' ',#120,10.);",
        "#140=LINE(' ',#100,#130);",
        "#150=TRIMMED_CURVE(' ',#140,(#100),(#110),.T.,.CARTESIAN.);",
        "#160=(\nANNOTATION_CURVE_OCCURRENCE()\nANNOTATION_OCCURRENCE()\nDRAUGHTING_ANNOTATION_OCCURRENCE()\nGEOMETRIC_REPRESENTATION_ITEM()\nREPRESENTATION_ITEM(' ')\nSTYLED_ITEM((#70),#150)\n);",
        "#200=CARTESIAN_POINT(' ',(5.,5.));",
        "#210=AXIS2_PLACEMENT_2D(' ',#200,$);",
        "#220=CIRCLE(' ',#210,3.);",
        "#230=ANNOTATION_CURVE_OCCURRENCE(' ',(#70),#220);",
        "#300=CARTESIAN_POINT(' ',(0.,0.));",
        "#310=AXIS2_PLACEMENT_2D(' ',#300,$);",
        "#320=CIRCLE(' ',#310,5.);",
        "#330=TRIMMED_CURVE(' ',#320,(PARAMETER_VALUE(0.)),(PARAMETER_VALUE(1.5707963267948966)),.T.,.PARAMETER.);",
        "#340=ANNOTATION_CURVE_OCCURRENCE(' ',(#75),#330);",
        "#400=CARTESIAN_POINT(' ',(0.,0.));",
        "#410=CARTESIAN_POINT(' ',(1.,1.));",
        "#420=CARTESIAN_POINT(' ',(2.,0.));",
        "#430=POLYLINE(' ',(#400,#410,#420));",
        "#440=ANNOTATION_CURVE_OCCURRENCE(' ',(#70),#430);",
        "#500=CARTESIAN_POINT(' ',(0.,10.));",
        "#510=DIRECTION(' ',(0.,1.));",
        "#520=VECTOR(' ',#510,10.);",
        "#530=LINE(' ',#100,#520);",
        "#540=TRIMMED_CURVE(' ',#530,(#100),(#500),.T.,.CARTESIAN.);",
        "#560=ANNOTATION_CURVE_OCCURRENCE(' ',(#70),#540);",
        "#600=CARTESIAN_POINT(' ',(20.,0.));",
        "#610=CARTESIAN_POINT(' ',(30.,0.));",
        "#620=POLYLINE(' ',(#600,#610));",
        "#630=COMPOSITE_CURVE_SEGMENT(.CONTINUOUS.,.T.,#620);",
        "#640=COMPOSITE_CURVE(' ',(#630),.F.);",
        "#650=ANNOTATION_CURVE_OCCURRENCE(' ',(#70),#640);",
        "#900=PRESENTATION_LAYER_ASSIGNMENT('A',' ',(#89,#160,#230,#340,#440,#650));",
        "#910=PRESENTATION_LAYER_ASSIGNMENT('B',' ',(#560));",
        "#950=DRAWING_SHEET_REVISION('A3_horizontal',(#89,#160,#230,#340,#440,#560,#650),' ',' ');",
    ];
    let mut src = "ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION(('SCADEC level2 AP202_mode'),'2;1');\nFILE_SCHEMA(('ASSOCIATIVE_DRAUGHTING'));\nENDSEC;\nDATA;\n".to_owned();
    for line in data {
        src.push_str(line);
        src.push('\n');
    }
    src.push_str("ENDSEC;\nEND-ISO-10303-21;\n");
    src.into_bytes()
}

#[cfg(test)]
#[allow(
    clippy::disallowed_methods,
    reason = "環境変数を読むのはテストだけで、コアは読まない（AGENTS.md の図面と秘密）"
)]
fn sample_paths() -> Vec<PathBuf> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../samples");
    let dirs = [root.join("sxf")]
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
                .is_some_and(|e| e.eq_ignore_ascii_case("sfc") || e.eq_ignore_ascii_case("p21"))
        })
        .collect();
    found.sort();
    found
}

#[test]
fn open_parity_across_samples_and_parallel_threads() {
    let paths = sample_paths();
    assert!(!paths.is_empty());
    for path in paths {
        let src = std::fs::read(&path).unwrap();
        let file = read_file(&src);
        let (expected, expected_codes) = lift_file(&file);

        let mut b1 = Budget::default();
        let mut d1 = Diagnostics::new();
        let c1 = CancelToken::new();
        let l1 = SxfLive::open_with_threads(&src, 1, &mut Cx::new(&mut b1, &mut d1, &c1)).unwrap();
        let (draw1, codes1) = live_drawing(&l1);

        let mut b4 = Budget::default();
        let mut d4 = Diagnostics::new();
        let c4 = CancelToken::new();
        let l4 = SxfLive::open_with_threads(&src, 4, &mut Cx::new(&mut b4, &mut d4, &c4)).unwrap();
        let (draw4, codes4) = live_drawing(&l4);

        assert_eq!(draw1, expected, "Drawing2D mismatch on {}", path.display());
        assert_eq!(
            draw4,
            expected,
            "4-thread Drawing2D mismatch on {}",
            path.display()
        );
        assert_eq!(codes1, codes4, "1 vs 4 thread codes on {}", path.display());
        assert_eq!(
            d1.iter().map(|x| x.code()).collect::<Vec<_>>(),
            expected_codes.into_iter().collect::<Vec<_>>().as_slice()[..0]
                .iter()
                .copied()
                .chain(d4.iter().map(|x| x.code()))
                .collect::<Vec<_>>(),
            "parallel open diag determinism on {}",
            path.display()
        );
        assert_eq!(
            live_save(&l1),
            src,
            "clean save mismatch on {}",
            path.display()
        );
        assert_eq!(
            live_save(&l4),
            src,
            "4-thread clean save mismatch on {}",
            path.display()
        );
    }
}

#[test]
fn sfc_sequential_edits_match_apply_bytes_and_undo_redo() {
    let src = rich_sfc_src();
    let mut file = read_file(&src);
    let (mut live, _) = open_live(&src);
    assert_eq!(live.mode(), Mode::Sfc);
    assert!(!live.display().has_dirty());

    let mut gpu_mirror = Display::default();
    live.display_mut().mark_all_dirty();
    live.display_mut().sync_gpu_mirror(&mut gpu_mirror);
    assert_eq!(gpu_mirror.canonical(), live.display().canonical());

    let snap0_bytes = live_save(&live);
    let snap0_state = live.state_id();
    let snap0_disp = live.display().canonical();
    let snap0_space_sel = live.select_in_space(SpaceId(0), [-5.0, -5.0, 25.0, 15.0]);
    assert_eq!(snap0_space_sel.len(), 1);

    // Step 1: Restyle + Transform (sheet & space) + Replace (circle & point_marker)
    let (d0, _) = live_drawing(&live);
    let (line90, _) = seen_sheet(&d0, 90);
    let (circ100, circ100_elem) = seen_sheet(&d0, 100);
    let (pm140, pm140_elem) = seen_sheet(&d0, 140);
    let (sub_line50, _) = seen_space(&d0, SpaceId(0), 50);

    let step1 = vec![
        Edit::Restyle {
            target: line90,
            style: Restyle {
                layer: Some(LayerId(2)),
                color: Some(Inherit::Own(Color {
                    code: None,
                    rgb: Some([10, 20, 30]),
                })),
                line_weight: Some(Inherit::Own(LineWeight::Width(PaperLength(0.3)))),
                ..Restyle::default()
            },
        },
        Edit::Transform {
            target: sub_line50,
            transform: Affine::translation(50.0, 25.0),
        },
        Edit::Replace {
            target: circ100,
            element: Box::new(Placed::Paper(Element {
                shape: Shape::Circle {
                    center: paper(8.0, 9.0),
                    radius: Length::Paper(PaperLength(4.0)),
                },
                ..circ100_elem
            })),
        },
        Edit::Replace {
            target: pm140,
            element: Box::new(Placed::Paper(Element {
                style: Style {
                    color: sxf_color(3),
                    ..pm140_elem.style
                },
                shape: Shape::Point(Point {
                    position: paper(25.0, 35.0),
                    rotation: 45.0,
                    look: PointLook::Marker {
                        key: MarkerKey {
                            namespace: "sxf.marker".into(),
                            code: "4".to_owned(),
                        },
                        scale: 2.0,
                    },
                    construction: false,
                }),
                ..pm140_elem
            })),
        },
    ];

    let ((r_file, c_file), (r_live, c_live)) = apply_both(&mut file, &mut live, step1, None);
    assert_eq!(c_live, c_file);
    assert_eq!(r_live.unwrap(), r_file.unwrap());
    assert_state_parity(&file, &live);
    live.display_mut().sync_gpu_mirror(&mut gpu_mirror);
    assert_eq!(gpu_mirror.canonical(), live.display().canonical());
    assert!(
        live.select_in_space(SpaceId(0), [45.0, 20.0, 75.0, 40.0])
            .len()
            == 1
    );
    let snap1_bytes = live_save(&live);
    let snap1_state = live.state_id();
    let snap1_disp = live.display().canonical();

    // Step 2: Remove text (#130) & arc (#110), Add line + polyline + point_marker onto sheet
    let (d1, _) = live_drawing(&live);
    let sheet_place = id::sheet(&d1.sheets[0]);
    let (arc110, _) = seen_sheet(&d1, 110);
    let (txt130, _) = seen_sheet(&d1, 130);
    let base_style = Style {
        color: sxf_color(1),
        line_type: Inherit::Own(LineTypeId(1)),
        line_type_scale: 1.0,
        line_weight: Some(Inherit::Own(LineWeight::Width(PaperLength(0.25)))),
        transparency: Inherit::Layer,
    };
    let step2 = vec![
        Edit::Remove { target: arc110 },
        Edit::Remove { target: txt130 },
        Edit::Add {
            place: sheet_place.clone(),
            element: Box::new(Placed::Paper(Element {
                source: Ref::default(),
                layer: LayerId(1),
                style: base_style,
                shape: Shape::Line {
                    start: paper(30.0, 40.0),
                    end: paper(60.0, 80.0),
                },
            })),
        },
        Edit::Add {
            place: sheet_place.clone(),
            element: Box::new(Placed::Paper(Element {
                source: Ref::default(),
                layer: LayerId(2),
                style: base_style,
                shape: Shape::Polyline(Polyline {
                    vertices: vec![
                        Vertex {
                            point: paper(0.0, 0.0),
                            bulge: 0.0,
                        },
                        Vertex {
                            point: paper(12.0, 5.0),
                            bulge: 0.0,
                        },
                        Vertex {
                            point: paper(20.0, 0.0),
                            bulge: 0.0,
                        },
                    ],
                    closed: false,
                    width: None,
                    vertex_widths: Vec::new(),
                    line_type_generation: false,
                }),
            })),
        },
        Edit::Add {
            place: sheet_place.clone(),
            element: Box::new(Placed::Paper(Element {
                source: Ref::default(),
                layer: LayerId(1),
                style: Style {
                    color: sxf_color(2),
                    line_type: Inherit::Layer,
                    line_type_scale: 1.0,
                    line_weight: None,
                    transparency: Inherit::Layer,
                },
                shape: Shape::Point(Point {
                    position: paper(15.0, 15.0),
                    rotation: 0.0,
                    look: PointLook::Marker {
                        key: MarkerKey {
                            namespace: "sxf.marker".into(),
                            code: "3".to_owned(),
                        },
                        scale: 1.5,
                    },
                    construction: false,
                }),
            })),
        },
    ];

    let ((r_file, c_file), (r_live, c_live)) = apply_both(&mut file, &mut live, step2, None);
    assert_eq!(c_live, c_file);
    let applied2 = r_live.unwrap();
    assert_eq!(applied2, r_file.unwrap());
    assert_state_parity(&file, &live);
    live.display_mut().sync_gpu_mirror(&mut gpu_mirror);
    assert_eq!(gpu_mirror.canonical(), live.display().canonical());
    let snap2_bytes = live_save(&live);
    let snap2_state = live.state_id();
    let snap2_disp = live.display().canonical();

    // Step 3: Edit a born element, remove the highest-ID born element, and add a new circle
    let born_line_id = applied2[2].id.clone().unwrap();
    let born_line_hash = applied2[2].hash.unwrap();
    let born_pm_id = applied2[4].id.clone().unwrap();
    let born_pm_hash = applied2[4].hash.unwrap();
    let step3 = vec![
        Edit::Transform {
            target: Seen {
                id: born_line_id,
                hash: born_line_hash,
            },
            transform: Affine::translation(10.0, -5.0),
        },
        Edit::Remove {
            target: Seen {
                id: born_pm_id,
                hash: born_pm_hash,
            },
        },
        Edit::Add {
            place: sheet_place,
            element: Box::new(Placed::Paper(Element {
                source: Ref::default(),
                layer: LayerId(1),
                style: base_style,
                shape: Shape::Circle {
                    center: paper(70.0, 70.0),
                    radius: Length::Paper(PaperLength(6.0)),
                },
            })),
        },
    ];
    let ((r_file, c_file), (r_live, c_live)) = apply_both(&mut file, &mut live, step3, None);
    assert_eq!(c_live, c_file);
    assert_eq!(r_live.unwrap(), r_file.unwrap());
    assert_state_parity(&file, &live);
    live.display_mut().sync_gpu_mirror(&mut gpu_mirror);
    assert_eq!(gpu_mirror.canonical(), live.display().canonical());
    let snap3_bytes = live_save(&live);
    let snap3_state = live.state_id();
    let snap3_disp = live.display().canonical();

    // Undo all 3 steps and verify exact restoration
    assert!(live_undo(&mut live));
    assert_eq!(live.state_id(), snap2_state);
    assert_eq!(live_save(&live), snap2_bytes);
    assert_eq!(live.display().canonical(), snap2_disp);

    assert!(live_undo(&mut live));
    assert_eq!(live.state_id(), snap1_state);
    assert_eq!(live_save(&live), snap1_bytes);
    assert_eq!(live.display().canonical(), snap1_disp);

    assert!(live_undo(&mut live));
    assert_eq!(live.state_id(), snap0_state);
    assert_eq!(live_save(&live), snap0_bytes);
    assert_eq!(live.display().canonical(), snap0_disp);
    assert_eq!(
        live.select_in_space(SpaceId(0), [-5.0, -5.0, 25.0, 15.0]),
        snap0_space_sel
    );
    assert!(!live_undo(&mut live));

    // Redo all 3 steps and verify exact restoration
    assert!(live_redo(&mut live));
    assert_eq!(live.state_id(), snap1_state);
    assert_eq!(live_save(&live), snap1_bytes);

    assert!(live_redo(&mut live));
    assert_eq!(live.state_id(), snap2_state);
    assert_eq!(live_save(&live), snap2_bytes);

    assert!(live_redo(&mut live));
    assert_eq!(live.state_id(), snap3_state);
    assert_eq!(live_save(&live), snap3_bytes);
    assert_eq!(live.display().canonical(), snap3_disp);
    assert!(!live_redo(&mut live));
}

#[test]
fn p21_sequential_edits_shared_styles_garbage_collection_and_id_reuse() {
    let src = rich_p21_src();
    let mut file = read_file(&src);
    let (mut live, _) = open_live(&src);
    assert_eq!(live.mode(), Mode::P21);

    let mut gpu_mirror = Display::default();
    live.display_mut().mark_all_dirty();
    live.display_mut().sync_gpu_mirror(&mut gpu_mirror);

    let snap0_bytes = live_save(&live);

    // Step 1: Restyle #160 (copy-on-write of shared #70, allocating new predefined yellow colour),
    // Transform #230 and #560 (#100 still referenced by #160 so #100 stays), Replace point marker #89
    let (d0, _) = live_drawing(&live);
    let (line160, _) = seen_sheet(&d0, 160);
    let (circ230, _) = seen_sheet(&d0, 230);
    let (line560, _) = seen_sheet(&d0, 560);
    let (pm89, pm89_elem) = seen_sheet(&d0, 89);

    let step1 = vec![
        Edit::Restyle {
            target: line160,
            style: Restyle {
                layer: Some(LayerId(2)),
                color: Some(sxf_color(5)), // yellow -> born DRAUGHTING_PRE_DEFINED_COLOUR('yellow')
                ..Restyle::default()
            },
        },
        Edit::Transform {
            target: circ230,
            transform: Affine::translation(12.0, -4.0),
        },
        Edit::Transform {
            target: line560,
            transform: Affine::translation(3.0, 7.0),
        },
        Edit::Replace {
            target: pm89,
            element: Box::new(Placed::Paper(Element {
                shape: Shape::Point(Point {
                    position: paper(50.0, 40.0),
                    rotation: 30.0,
                    look: PointLook::Marker {
                        key: MarkerKey {
                            namespace: "sxf.marker".into(),
                            code: "7".to_owned(),
                        },
                        scale: 2.0,
                    },
                    construction: false,
                }),
                ..pm89_elem
            })),
        },
    ];

    let ((r_file, c_file), (r_live, c_live)) = apply_both(&mut file, &mut live, step1, None);
    assert_eq!(c_live, c_file);
    assert_eq!(r_live.unwrap(), r_file.unwrap());
    assert_state_parity(&file, &live);
    let snap1_bytes = live_save(&live);

    // Step 2: Remove #160 (now #100 has 0 referrers -> GC!), Remove #340 (sole user of width 0.42 -> GC #65/#45!),
    // and Add two elements using yellow colour (reusing the born yellow colour from Step 1!)
    let (d1, _) = live_drawing(&live);
    let sheet_place = id::sheet(&d1.sheets[0]);
    let (line160_s1, _) = seen_sheet(&d1, 160);
    let (arc340_s1, _) = seen_sheet(&d1, 340);
    let yellow_style = Style {
        color: sxf_color(5),
        line_type: Inherit::Own(LineTypeId(2)), // dashed -> born predefined curve font
        line_type_scale: 1.0,
        line_weight: Some(Inherit::Own(LineWeight::Width(PaperLength(0.25)))),
        transparency: Inherit::Layer,
    };
    let step2 = vec![
        Edit::Remove { target: line160_s1 },
        Edit::Remove { target: arc340_s1 },
        Edit::Add {
            place: sheet_place.clone(),
            element: Box::new(Placed::Paper(Element {
                source: Ref::default(),
                layer: LayerId(1),
                style: yellow_style,
                shape: Shape::Line {
                    start: paper(5.0, 5.0),
                    end: paper(25.0, 15.0),
                },
            })),
        },
        Edit::Add {
            place: sheet_place.clone(),
            element: Box::new(Placed::Paper(Element {
                source: Ref::default(),
                layer: LayerId(2),
                style: Style {
                    color: sxf_color(5),
                    line_type: Inherit::Layer,
                    line_type_scale: 1.0,
                    line_weight: None,
                    transparency: Inherit::Layer,
                },
                shape: Shape::Point(Point {
                    position: paper(8.0, 12.0),
                    rotation: 15.0,
                    look: PointLook::Marker {
                        key: MarkerKey {
                            namespace: "sxf.marker".into(),
                            code: "2".to_owned(),
                        },
                        scale: 1.0,
                    },
                    construction: false,
                }),
            })),
        },
    ];

    let ((r_file, c_file), (r_live, c_live)) = apply_both(&mut file, &mut live, step2, None);
    assert_eq!(c_live, c_file);
    let applied2 = r_live.unwrap();
    assert_eq!(applied2, r_file.unwrap());
    assert_state_parity(&file, &live);
    let snap2_bytes = live_save(&live);

    // User-defined width 0.42 was only on #65 (garbage-collected when #340 was removed),
    // so adding a 0.42-wide element in Step 2b must be rejected identically!
    let bad_width_add = vec![Edit::Add {
        place: sheet_place.clone(),
        element: Box::new(Placed::Paper(Element {
            source: Ref::default(),
            layer: LayerId(1),
            style: Style {
                line_weight: Some(Inherit::Own(LineWeight::Width(PaperLength(0.42)))),
                ..yellow_style
            },
            shape: Shape::Line {
                start: paper(0.0, 0.0),
                end: paper(1.0, 1.0),
            },
        })),
    }];
    let ((r_file_bad, c_file_bad), (r_live_bad, c_live_bad)) =
        apply_both(&mut file, &mut live, bad_width_add, None);
    assert_eq!(r_live_bad, Err(Error::Rejected));
    assert_eq!(r_file_bad, Err(Error::Rejected));
    assert_eq!(c_live_bad, c_file_bad);
    assert_eq!(live_save(&live), snap2_bytes);

    // Step 3: Remove the highest-ID born element (applied2[3]), then in Step 4 Add a circle
    // that reuses the highest instance ID!
    let step3 = vec![Edit::Remove {
        target: Seen {
            id: applied2[3].id.clone().unwrap(),
            hash: applied2[3].hash.unwrap(),
        },
    }];
    let ((r_file, c_file), (r_live, c_live)) = apply_both(&mut file, &mut live, step3, None);
    assert_eq!(c_live, c_file);
    assert_eq!(r_live.unwrap(), r_file.unwrap());
    assert_state_parity(&file, &live);
    let snap3_bytes = live_save(&live);

    // Step 4: Add a circle reusing dashed curve font + yellow colour, and Restyle the earlier born line
    let step4 = vec![
        Edit::Restyle {
            target: Seen {
                id: applied2[2].id.clone().unwrap(),
                hash: applied2[2].hash.unwrap(),
            },
            style: Restyle {
                color: Some(Inherit::Own(Color {
                    code: None,
                    rgb: Some([51, 102, 153]), // matches #50 COLOUR_RGB('',0.2,0.4,0.6)
                })),
                ..Restyle::default()
            },
        },
        Edit::Add {
            place: sheet_place,
            element: Box::new(Placed::Paper(Element {
                source: Ref::default(),
                layer: LayerId(1),
                style: yellow_style,
                shape: Shape::Circle {
                    center: paper(40.0, 30.0),
                    radius: Length::Paper(PaperLength(7.5)),
                },
            })),
        },
    ];
    let ((r_file, c_file), (r_live, c_live)) = apply_both(&mut file, &mut live, step4, None);
    assert_eq!(c_live, c_file);
    assert_eq!(r_live.unwrap(), r_file.unwrap());
    assert_state_parity(&file, &live);
    live.display_mut().sync_gpu_mirror(&mut gpu_mirror);
    assert_eq!(gpu_mirror.canonical(), live.display().canonical());
    let snap4_bytes = live_save(&live);

    // Full undo & redo walk across ID reuse and GC boundaries!
    assert!(live_undo(&mut live));
    assert_eq!(live_save(&live), snap3_bytes);
    assert!(live_undo(&mut live));
    assert_eq!(live_save(&live), snap2_bytes);
    assert!(live_undo(&mut live));
    assert_eq!(live_save(&live), snap1_bytes);
    assert!(live_undo(&mut live));
    assert_eq!(live_save(&live), snap0_bytes);
    assert!(!live_undo(&mut live));

    assert!(live_redo(&mut live));
    assert_eq!(live_save(&live), snap1_bytes);
    assert!(live_redo(&mut live));
    assert_eq!(live_save(&live), snap2_bytes);
    assert!(live_redo(&mut live));
    assert_eq!(live_save(&live), snap3_bytes);
    assert!(live_redo(&mut live));
    assert_eq!(live_save(&live), snap4_bytes);
    assert!(!live_redo(&mut live));
}

#[test]
fn saf_attributes_and_refused_edits_preserve_invariants() {
    let saf_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<SxfAttributeXML version="3.1" date="2026-11-03" sxfFile="D0PL001Z.SFC" application="XX-CAD">
  <AttributeSet name="計画設計" version="1.0" designedBy="協会">1</AttributeSet>
  <Figure id="10" name="道路横断図形">
    <AttrSetRef id="1">
      <Attr name="測点名" type="STR">No.1</Attr>
    </AttrSetRef>
  </Figure>
</SxfAttributeXML>"#;
    let mut b = Budget::default();
    let mut d = Diagnostics::new();
    let c = CancelToken::new();
    let attr: AttributeFile =
        saf::read(saf_xml.as_bytes(), &mut Cx::new(&mut b, &mut d, &c)).unwrap();

    let features: [&[u8]; 6] = [
        b"layer_feature(\\'L\\','1')",
        b"line_feature('1','1','1','1','0','0','1','0')",
        b"sfig_org_feature(\\'$$ATRF$$10$$D0PL001Z.SAF\\','3')",
        b"sfig_locate_feature('1',\\'$$ATRF$$10$$D0PL001Z.SAF\\','0','0','0','1','1')",
        b"circle_feature('1','1','1','1','10','10','5')",
        b"drawing_sheet_feature(\\'S\\','3','1','0','0')",
    ];
    let mut sfc_src = b"ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION(('SCADEC level2 feature_mode'),'2;1');\nFILE_SCHEMA(('ASSOCIATIVE_DRAUGHTING'));\nENDSEC;\nDATA;\n".to_vec();
    for (at, f) in features.iter().enumerate() {
        sfc_src.extend_from_slice(format!("/*SXF3\n#{} = ", (at + 1) * 10).as_bytes());
        sfc_src.extend_from_slice(f);
        sfc_src.extend_from_slice(b"\nSXF3*/\n");
    }
    sfc_src.extend_from_slice(b"ENDSEC;\nEND-ISO-10303-21;\n");

    let file = read_file(&sfc_src);
    let mut b1 = Budget::default();
    let mut d1 = Diagnostics::new();
    let expected_d = file
        .lift_with(Some(&attr), &mut Cx::new(&mut b1, &mut d1, &c))
        .unwrap();

    let mut b2 = Budget::default();
    let mut d2 = Diagnostics::new();
    let mut live =
        SxfLive::open_with_attributes(&sfc_src, Some(attr), &mut Cx::new(&mut b2, &mut d2, &c))
            .unwrap();
    let (live_d, _) = live_drawing(&live);
    assert_eq!(live_d, expected_d);
    assert!(
        live_d
            .extensions
            .iter()
            .any(|e| e.namespace == "sxf.saf" && e.is_current(&live_d))
    );

    // Move unrelated circle #50; SAF attribute on #20 must remain current
    let (circ50, _) = seen_sheet(&live_d, 50);
    let mut b3 = Budget::default();
    let mut d3 = Diagnostics::new();
    live.apply(
        &Changes {
            edits: vec![Edit::Transform {
                target: circ50.clone(),
                transform: Affine::translation(4.0, 0.0),
            }],
            document: None,
        },
        &mut Cx::new(&mut b3, &mut d3, &c),
    )
    .unwrap();
    let (after_d, _) = live_drawing(&live);
    assert!(
        after_d
            .extensions
            .iter()
            .any(|e| e.namespace == "sxf.saf" && e.is_current(&after_d))
    );

    // Stale document hash, stale element hash, missing element, SetPointStyle must all reject cleanly
    let rev_before = live.revision();
    let saved_before = live_save(&live);
    for (edits, doc_hash, expected_code) in [
        (
            vec![Edit::Remove {
                target: seen_sheet(&after_d, 50).0,
            }],
            Some(Hash::document(b"wrong")),
            codes::CHANGE_DOCUMENT_STALE,
        ),
        (
            vec![Edit::Remove { target: circ50 }],
            None,
            codes::CHANGE_ELEMENT_STALE,
        ),
        (
            vec![Edit::Remove {
                target: Seen {
                    id: "sheet/0/i99999".to_owned(),
                    hash: Hash::document(b"x"),
                },
            }],
            None,
            codes::CHANGE_ELEMENT_MISSING,
        ),
        (
            vec![Edit::SetPointStyle {
                style: PointStyle {
                    mark: PointMark::Plus,
                    circle: false,
                    square: false,
                    size: PointSize::Length(1.0),
                },
            }],
            None,
            codes::CHANGE_EDIT_UNSUPPORTED,
        ),
    ] {
        let mut bx = Budget::default();
        let mut dx = Diagnostics::new();
        let res = live.apply(
            &Changes {
                edits,
                document: doc_hash,
            },
            &mut Cx::new(&mut bx, &mut dx, &c),
        );
        assert_eq!(res, Err(Error::Rejected));
        assert!(dx.iter().any(|diag| diag.code() == expected_code));
        assert_eq!(live.revision(), rev_before);
        assert_eq!(live_save(&live), saved_before);
    }
}

#[test]
fn samples_edit_undo_redo_exact_byte_parity() {
    fn shaped<P>(shape: &Shape<P>) -> bool {
        matches!(
            shape,
            Shape::Line { .. }
                | Shape::Circle { .. }
                | Shape::Polyline(_)
                | Shape::Curve(Edge::Arc { .. })
        )
    }

    let paths = sample_paths();
    let mut verified = 0usize;
    for path in paths {
        let src = std::fs::read(&path).unwrap();
        let file = read_file(&src);
        let (drawing, _) = lift_file(&file);
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

        let try_file = |candidate: &[Edit]| {
            let mut f = file.clone();
            let mut b = Budget::default();
            let mut d = Diagnostics::new();
            let c = CancelToken::new();
            let res = f.apply(
                &Changes {
                    edits: candidate.to_vec(),
                    document: None,
                },
                &mut Cx::new(&mut b, &mut d, &c),
            );
            (res, f, d)
        };

        let (first_res, _, first_diag) = try_file(&edits);
        if first_res.is_err() {
            // SxfLive must also reject the unfiltered batch with identical diagnostic codes
            let (mut live_probe, _) = open_live(&src);
            let mut bp = Budget::default();
            let mut dp = Diagnostics::new();
            let cp = CancelToken::new();
            let probe_res = live_probe.apply(
                &Changes {
                    edits: edits.clone(),
                    document: None,
                },
                &mut Cx::new(&mut bp, &mut dp, &cp),
            );
            assert_eq!(probe_res, Err(Error::Rejected), "{}", path.display());
            assert_eq!(live_save(&live_probe), src, "{}", path.display());

            let refused: BTreeSet<u64> = first_diag
                .iter()
                .filter(|d| d.code() == codes::CHANGE_EDIT_UNSUPPORTED)
                .flat_map(|d| d.places().iter().filter_map(|p| p.record()))
                .collect();
            let record = |edit: &Edit| {
                let id = match edit {
                    Edit::Transform { target, .. } | Edit::Remove { target } => &target.id,
                    _ => return None,
                };
                let instance: u64 = id.rsplit_once("/i")?.1.parse().ok()?;
                file.records()
                    .iter()
                    .position(|rec| rec.id() == Some(instance))
                    .map(|at| at as u64)
            };
            edits.retain(|edit| record(edit).is_none_or(|at| !refused.contains(&at)));
        }

        let mut file_ok = file.clone();
        let (mut live, _) = open_live(&src);
        let snap0_state = live.state_id();
        let snap0_disp = live.display().canonical();

        let ((r_file, c_file), (r_live, c_live)) = apply_both(&mut file_ok, &mut live, edits, None);
        assert_eq!(c_live, c_file, "{}", path.display());
        assert_eq!(
            r_live.unwrap_or_else(|e| panic!("{} live failed: {e:?}", path.display())),
            r_file.unwrap_or_else(|e| panic!("{} file failed: {e:?}", path.display())),
            "{}",
            path.display()
        );

        let expected_bytes = write_file_bytes(&file_ok);
        let live_bytes = live_save(&live);
        assert_eq!(
            live_bytes,
            expected_bytes,
            "byte parity failed on {}",
            path.display()
        );

        assert!(live_undo(&mut live));
        assert_eq!(live.state_id(), snap0_state, "{}", path.display());
        assert_eq!(live_save(&live), src, "undo bytes on {}", path.display());
        assert_eq!(
            live.display().canonical(),
            snap0_disp,
            "undo display on {}",
            path.display()
        );

        assert!(live_redo(&mut live));
        assert_eq!(
            live_save(&live),
            expected_bytes,
            "redo bytes on {}",
            path.display()
        );
        verified += 1;
    }
    assert!(verified > 0);
}

#[test]
fn budget_and_cancellation_leave_live_engine_untouched() {
    for src in [rich_sfc_src(), rich_p21_src()] {
        let (mut live, _) = open_live(&src);
        let (d0, _) = live_drawing(&live);
        let target_inst = if live.mode() == Mode::Sfc { 90 } else { 160 };
        let (target, _) = seen_sheet(&d0, target_inst);
        let changes = Changes {
            edits: vec![Edit::Transform {
                target,
                transform: Affine::translation(10.0, 20.0),
            }],
            document: None,
        };

        // Measure work needed for a clean apply on a clone
        let mut probe = live.clone();
        let mut b_full = Budget::default();
        let mut d_full = Diagnostics::new();
        let c_full = CancelToken::new();
        probe
            .apply(&changes, &mut Cx::new(&mut b_full, &mut d_full, &c_full))
            .unwrap();
        let needed_work = b_full.used(Kind::Work);
        assert!(needed_work > 2);

        let rev0 = live.revision();
        let state0 = live.state_id();
        let disp0 = live.display().canonical();

        // 1. Pre-cancelled token
        let mut b_can = Budget::default();
        let mut d_can = Diagnostics::new();
        let c_can = CancelToken::new();
        c_can.cancel();
        let res_can = live.apply(&changes, &mut Cx::new(&mut b_can, &mut d_can, &c_can));
        assert!(matches!(res_can, Err(Error::Stopped(_))));
        assert_eq!(live.revision(), rev0);
        assert_eq!(live.state_id(), state0);
        assert_eq!(live.undo_len(), 0);
        assert_eq!(live_save(&live), src);
        assert_eq!(live.display().canonical(), disp0);

        // 2. Tight work budget (stops mid-operation and rolls back cleanly)
        for limit in [1, needed_work / 2, needed_work - 1] {
            let mut b_lim = Budget::new(Limits::default().with(Kind::Work, Limit::Finite(limit)));
            let mut d_lim = Diagnostics::new();
            let c_lim = CancelToken::new();
            let res_lim = live.apply(&changes, &mut Cx::new(&mut b_lim, &mut d_lim, &c_lim));
            assert!(
                matches!(res_lim, Err(Error::Stopped(_))),
                "mode={:?} limit={limit}/{needed_work} res={res_lim:?}",
                live.mode()
            );
            assert_eq!(live.revision(), rev0);
            assert_eq!(live.state_id(), state0);
            assert_eq!(live.undo_len(), 0);
            assert_eq!(live_save(&live), src);
            assert_eq!(live.display().canonical(), disp0);
        }
    }
}

#[test]
fn p21_shift_jis_raw_values_and_remove_last_trailing_newline_parity() {
    let sjis = |s: &str| encoding_rs::SHIFT_JIS.encode(s).0.into_owned();
    let extra = r#"EXTRA_PART(tYpE((.miXed.,+001.20E+02,"0aB",$,*,#30,'図''枠\X2\56F3\X0\')))"#;
    // Put #560 AFTER #950 so #560 is the very last statement before ENDSEC;
    let base_text = String::from_utf8(rich_p21_src())
        .unwrap()
        .replace("#560=ANNOTATION_CURVE_OCCURRENCE(' ',(#70),#540);\n", "")
        .replace(
            "ENDSEC;\nEND-ISO-10303-21;\n",
            "#560=ANNOTATION_CURVE_OCCURRENCE(' ',(#70),#540);\nENDSEC;\nEND-ISO-10303-21;\n",
        )
        .replace("ASSIGNMENT('A',' '", "ASSIGNMENT('図枠','備''考'")
        .replace(
            "CURVE_STYLE(' ',#30,#40,#20);",
            "CURVE_STYLE('線''種',#30,#40,#20);",
        )
        .replace(
            "REPRESENTATION_ITEM(' ')",
            &format!("{extra}\nREPRESENTATION_ITEM('表記')"),
        );
    let src = sjis(&base_text);

    let mut file = read_file(&src);
    let (mut live, _) = open_live(&src);
    let (d0, _) = live_drawing(&live);
    let sheet_place = id::sheet(&d0.sheets[0]);
    let (line160, _) = seen_sheet(&d0, 160);
    let (last560, _) = seen_sheet(&d0, 560);

    // Removing #560 BEFORE moving another item into Layer B (#910) would empty #910 -> rejected by both!
    let bad_order = vec![
        Edit::Remove {
            target: last560.clone(),
        },
        Edit::Restyle {
            target: line160.clone(),
            style: Restyle {
                layer: Some(LayerId(2)),
                color: Some(sxf_color(17)),
                ..Restyle::default()
            },
        },
    ];
    let ((rf_bad, cf_bad), (rl_bad, cl_bad)) = apply_both(&mut file, &mut live, bad_order, None);
    assert_eq!(rl_bad, Err(Error::Rejected));
    assert_eq!(rf_bad, Err(Error::Rejected));
    assert_eq!(cl_bad, cf_bad);

    // Move #160 (complex entity with EXTRA_PART & Shift_JIS) into Layer B first, then remove last statement (#560), and Add a line
    let edits = vec![
        Edit::Restyle {
            target: line160,
            style: Restyle {
                layer: Some(LayerId(2)),
                color: Some(sxf_color(17)),
                ..Restyle::default()
            },
        },
        Edit::Remove { target: last560 },
        Edit::Add {
            place: sheet_place,
            element: Box::new(Placed::Paper(Element {
                source: Ref::default(),
                layer: LayerId(1),
                style: Style {
                    color: sxf_color(1),
                    line_type: Inherit::Own(LineTypeId(1)),
                    line_type_scale: 1.0,
                    line_weight: Some(Inherit::Own(LineWeight::Width(PaperLength(0.25)))),
                    transparency: Inherit::Layer,
                },
                shape: Shape::Line {
                    start: paper(0.0, 0.0),
                    end: paper(5.0, 5.0),
                },
            })),
        },
    ];

    let ((r_file, c_file), (r_live, c_live)) = apply_both(&mut file, &mut live, edits, None);
    assert_eq!(c_live, c_file);
    assert_eq!(r_live.unwrap(), r_file.unwrap());
    assert_state_parity(&file, &live);

    assert!(live_undo(&mut live));
    assert_eq!(live_save(&live), src);
    assert!(live_redo(&mut live));
    assert_eq!(live_save(&live), write_file_bytes(&file));
}
