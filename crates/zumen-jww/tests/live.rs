//! `JwwLive` の契約検査（`docs/live.md`、#878）。
//! 現行の `JwwFile::apply` + `Jww::write` とのバイト列完全一致、常駐図面モデル・ID・ハッシュ・`shifted`・診断の一致、
//! 多段 `undo` / `redo`、空間索引と差分表示バッファー、並列読み込みの決定性、予算・中断耐性を確かめる。

#![cfg(feature = "drawing")]

mod common;

use core::f64::consts::PI;

use common::{Archive, Spec};
use zumen_base::{Budget, CancelToken, Code, Cx, Diagnostics, Kind, Limit, Limits, codes};
use zumen_drawing::{
    Affine, Applied, Changes, Color, ColorCode, ColorSystem, Drawing2D, Edge, Edit, Element, Hash,
    Inherit, LayerId, Length, LineTypeId, LineWeight, MarkerKey, ModelLength, ModelPoint,
    PaperLength, Placed, Point, PointLook, PointMark, PointSize, PointStyle, Restyle, Seen, Shape,
    SpaceId, Style, id,
};
use zumen_jww::{Jww, JwwFile, JwwLive};
use zumen_sdk::{Apply, Codec, Error, Lift, testkit};

#[cfg(test)]
fn base_src() -> Vec<u8> {
    let mut spec = Spec::new(700);
    let mut layers = [0; 16];
    layers[0] = 3;
    layers[1] = 2;
    spec.groups[0] = (3, 100.0, layers);
    spec.max_draw_width = -300;
    spec.pens[2] = 0x0000_00FF;
    spec.printer_widths[2] = 25;
    let mut archive = Archive::new(700);
    archive.layer = 0;
    archive.group = 0;
    archive.pen_width = 0;
    archive.count(5);
    archive.line();
    archive.tag("CDataEnko");
    archive.base(1, 1);
    archive.reals(&[1.0, 2.0, 5.0, 0.0, 2.0 * PI, 0.0, 1.0]);
    archive.out.extend(1u32.to_le_bytes());
    archive.tag("CDataEnko");
    archive.base(1, 1);
    archive.reals(&[0.0, 0.0, 5.0, 0.0, PI / 2.0, 0.0, 1.0]);
    archive.out.extend(0u32.to_le_bytes());
    archive.tag("CDataTen");
    archive.base(1, 2);
    archive.reals(&[1.0, 1.0]);
    archive.out.extend(0u32.to_le_bytes());
    archive.tag("CDataMoji");
    archive.base(1, 1);
    archive.text_body("図面");
    archive.count(1);
    archive.block("部品");
    archive.out.extend(0u32.to_le_bytes());
    let mut src = spec.build();
    src.extend(&archive.out);
    src
}

#[cfg(test)]
fn read_file(src: &[u8]) -> JwwFile {
    testkit::read::<Jww>(src).0.unwrap()
}

#[cfg(test)]
fn lift_file(file: &JwwFile) -> (Drawing2D, Vec<Code>) {
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let d = file
        .lift(&mut Cx::new(&mut budget, &mut diag, &cancel))
        .unwrap();
    (d, diag.iter().map(|x| x.code()).collect())
}

#[cfg(test)]
fn write_file_bytes(file: &JwwFile) -> Vec<u8> {
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let mut out = Vec::new();
    Jww::write(
        file,
        &mut out,
        &mut Cx::new(&mut budget, &mut diag, &cancel),
    )
    .unwrap();
    out
}

#[cfg(test)]
fn open_live(src: &[u8]) -> (JwwLive, Vec<Code>) {
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let live = JwwLive::open(src, &mut Cx::new(&mut budget, &mut diag, &cancel)).unwrap();
    (live, diag.iter().map(|x| x.code()).collect())
}

#[cfg(test)]
fn live_drawing(live: &JwwLive) -> (Drawing2D, Vec<Code>) {
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let d = live
        .drawing(&mut Cx::new(&mut budget, &mut diag, &cancel))
        .unwrap();
    (d, diag.iter().map(|x| x.code()).collect())
}

#[cfg(test)]
fn live_save(live: &JwwLive) -> Vec<u8> {
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    live.save(&mut Cx::new(&mut budget, &mut diag, &cancel))
        .unwrap()
}

#[cfg(test)]
#[expect(clippy::type_complexity)]
fn apply_both(
    file: &mut JwwFile,
    live: &mut JwwLive,
    edits: Vec<Edit>,
    document: Option<Hash>,
) -> (
    (Result<(Vec<Applied>, bool), Error>, Vec<Code>),
    (Result<(Vec<Applied>, bool), Error>, Vec<Code>),
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
fn element_at(drawing: &Drawing2D, at: usize) -> (String, &Element<ModelPoint>) {
    let space = drawing.spaces.iter().find(|s| s.id == SpaceId(0)).unwrap();
    let element = &space.elements[at];
    let mut namer = id::Namer::new("space/0");
    for (index, other) in space.elements.iter().enumerate().take(at) {
        namer.next(index, other);
    }
    (namer.next(at, element), element)
}

#[cfg(test)]
fn seen_at(drawing: &Drawing2D, at: usize) -> Seen {
    let (id, element) = element_at(drawing, at);
    Seen {
        id,
        hash: Hash::element(element),
    }
}

#[cfg(test)]
fn model(x: f64, y: f64) -> ModelPoint {
    ModelPoint {
        space: SpaceId(0),
        x,
        y,
    }
}

#[cfg(test)]
fn length(value: f64) -> Length {
    Length::Model(ModelLength {
        space: SpaceId(0),
        value,
    })
}

#[cfg(test)]
fn assert_state_parity(file: &JwwFile, live: &JwwLive) {
    let file_bytes = write_file_bytes(file);
    let saved_bytes = live_save(live);
    assert_eq!(saved_bytes, file_bytes);

    let (file_d, file_codes) = lift_file(file);
    let (live_d, live_codes) = live_drawing(live);
    assert_eq!(live_codes, file_codes);
    assert_eq!(live_d, file_d);
}

#[test]
fn parity_with_sequential_apply_and_multi_step_undo_redo() {
    let mut src = base_src();
    let tmp = read_file(&src);
    let rec1_body = tmp.records()[1].body().start;
    // 未読の上位ビットも Restyle で保たれることを同時に確かめる
    src[rec1_body + 9..rec1_body + 11].copy_from_slice(&0xCD00u16.to_le_bytes());
    src[rec1_body + 11..rec1_body + 13].copy_from_slice(&0xAB00u16.to_le_bytes());

    let mut file = read_file(&src);
    let (mut live, open_codes) = open_live(&src);
    assert!(open_codes.is_empty());

    let state0 = live.state_id();
    let (d0, _) = live_drawing(&live);
    assert_state_parity(&file, &live);

    // Step 1: Restyle + Transform (model & block inner) + Replace（一覧再構築なし）
    let def0 = &d0.symbols[0];
    let inner_seen = Seen {
        id: format!("{}/r7", id::DefinitionNamer::default().next(def0)),
        hash: Hash::element(&def0.space.elements[0]),
    };
    let step1_edits = vec![
        Edit::Restyle {
            target: seen_at(&d0, 0),
            style: Restyle {
                layer: Some(LayerId(2)),
                color: Some(Inherit::Own(Color {
                    code: Some(ColorCode {
                        system: ColorSystem::Jww,
                        value: 2,
                    }),
                    rgb: None,
                })),
                line_type: Some(Inherit::Own(LineTypeId(2))),
                line_weight: Some(Inherit::Own(LineWeight::Width(PaperLength(0.5)))),
                ..Restyle::default()
            },
        },
        Edit::Transform {
            target: seen_at(&d0, 1),
            transform: Affine {
                m: [1.0, 0.0, 10.0, 0.0, 1.0, -20.0],
            },
        },
        Edit::Replace {
            target: seen_at(&d0, 2),
            element: Box::new(Placed::Model(Element {
                shape: Shape::Curve(Edge::Arc {
                    center: model(50.0, 50.0),
                    radius: length(30.0),
                    start: 180.0,
                    end: 90.0,
                    counterclockwise: false,
                }),
                ..element_at(&d0, 2).1.clone()
            })),
        },
        Edit::Transform {
            target: inner_seen,
            transform: Affine {
                m: [1.0, 0.0, 2.5, 0.0, 1.0, -1.5],
            },
        },
    ];
    let ((r_file, c_file), (r_live, c_live)) = apply_both(
        &mut file,
        &mut live,
        step1_edits,
        Some(Hash::document(&src)),
    );
    assert_eq!(c_live, c_file);
    assert_eq!(r_live.unwrap(), r_file.unwrap());
    assert_state_parity(&file, &live);
    let state1 = live.state_id();
    let bytes1 = live_save(&live);
    let (d1, _) = live_drawing(&live);
    assert_ne!(state1, state0);

    // Step 2: Remove（点） + Add（線と記号点）で一覧再構築モードに入る
    let added_line = Element {
        source: Default::default(),
        layer: LayerId(2),
        style: Style {
            color: Inherit::Own(Color {
                code: None,
                rgb: Some([0xFF, 0, 0]),
            }),
            line_type: Inherit::Own(LineTypeId(1)),
            line_type_scale: 1.0,
            line_weight: Some(Inherit::Layer),
            transparency: Inherit::Layer,
        },
        shape: Shape::Line {
            start: model(0.0, 0.0),
            end: model(0.0, 300.0),
        },
    };
    let added_marker = Element {
        source: Default::default(),
        layer: LayerId(1),
        style: Style {
            color: Inherit::Own(Color {
                code: Some(ColorCode {
                    system: ColorSystem::Jww,
                    value: 2,
                }),
                rgb: None,
            }),
            line_type: Inherit::Layer,
            line_type_scale: 1.0,
            line_weight: None,
            transparency: Inherit::Layer,
        },
        shape: Shape::Point(Point {
            position: model(200.0, 300.0),
            rotation: 30.0,
            look: PointLook::Marker {
                key: MarkerKey {
                    namespace: "jww.point".into(),
                    code: "3".to_owned(),
                },
                scale: 2.0,
            },
            construction: false,
        }),
    };
    let step2_edits = vec![
        Edit::Remove {
            target: seen_at(&d1, 3),
        },
        Edit::Add {
            place: "space/0".into(),
            element: Box::new(Placed::Model(added_line)),
        },
        Edit::Add {
            place: "space/0".into(),
            element: Box::new(Placed::Model(added_marker)),
        },
    ];
    let ((r_file, c_file), (r_live, c_live)) = apply_both(
        &mut file,
        &mut live,
        step2_edits,
        Some(Hash::document(&bytes1)),
    );
    assert_eq!(c_live, c_file);
    assert_eq!(r_live.unwrap(), r_file.unwrap());
    assert_state_parity(&file, &live);
    let state2 = live.state_id();
    let bytes2 = live_save(&live);
    let (d2, _) = live_drawing(&live);

    // Step 3: Step 2 で追加した Born 要素の Transform と Remove
    let step3_edits = vec![
        Edit::Transform {
            target: seen_at(&d2, 4),
            transform: Affine {
                m: [1.0, 0.0, 15.0, 0.0, 1.0, 25.0],
            },
        },
        Edit::Remove {
            target: seen_at(&d2, 5),
        },
    ];
    let ((r_file, c_file), (r_live, c_live)) = apply_both(
        &mut file,
        &mut live,
        step3_edits,
        Some(Hash::document(&bytes2)),
    );
    assert_eq!(c_live, c_file);
    assert_eq!(r_live.unwrap(), r_file.unwrap());
    assert_state_parity(&file, &live);
    let state3 = live.state_id();
    let bytes3 = live_save(&live);
    let (d3, _) = live_drawing(&live);

    // 多段 undo で各ステップの state_id・drawing・save バイト列が完全復元されること
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diag, &cancel);

    assert!(live.undo(&mut cx).unwrap());
    assert_eq!(live.state_id(), state2);
    assert_eq!(live_drawing(&live).0, d2);
    assert_eq!(live_save(&live), bytes2);

    assert!(live.undo(&mut cx).unwrap());
    assert_eq!(live.state_id(), state1);
    assert_eq!(live_drawing(&live).0, d1);
    assert_eq!(live_save(&live), bytes1);

    assert!(live.undo(&mut cx).unwrap());
    assert_eq!(live.state_id(), state0);
    assert_eq!(live_drawing(&live).0, d0);
    assert_eq!(live_save(&live), src);

    assert!(!live.undo(&mut cx).unwrap());

    // 多段 redo で再び一致すること
    assert!(live.redo(&mut cx).unwrap());
    assert_eq!(live.state_id(), state1);
    assert_eq!(live_save(&live), bytes1);

    assert!(live.redo(&mut cx).unwrap());
    assert_eq!(live.state_id(), state2);
    assert_eq!(live_save(&live), bytes2);

    assert!(live.redo(&mut cx).unwrap());
    assert_eq!(live.state_id(), state3);
    assert_eq!(live_drawing(&live).0, d3);
    assert_eq!(live_save(&live), bytes3);

    assert!(!live.redo(&mut cx).unwrap());
}

#[test]
fn shifted_flag_matches_file_apply_edge_cases() {
    // 1. 2 本の線のうち 1 本目を削除し、2 本目を同時に Restyle する場合:
    //    変更していない生存要素は存在しないため shifted == false になる
    let mut spec = Spec::new(700);
    spec.groups[0] = (3, 100.0, [3; 16]);
    let mut archive = Archive::new(700);
    archive.group = 0;
    archive.layer = 0;
    archive.count(2);
    archive.line();
    archive.line();
    archive.count(0);
    archive.out.extend(0u32.to_le_bytes());
    let mut src2 = spec.build();
    src2.extend(&archive.out);

    let mut file = read_file(&src2);
    let (mut live, _) = open_live(&src2);
    let (d, _) = live_drawing(&live);
    let edits = vec![
        Edit::Remove {
            target: seen_at(&d, 0),
        },
        Edit::Restyle {
            target: seen_at(&d, 1),
            style: Restyle {
                layer: Some(LayerId(3)),
                ..Restyle::default()
            },
        },
    ];
    let ((r_file, _), (r_live, _)) = apply_both(&mut file, &mut live, edits, None);
    let (app_file, shifted_file) = r_file.unwrap();
    let (app_live, shifted_live) = r_live.unwrap();
    assert!(!shifted_file);
    assert_eq!(shifted_live, shifted_file);
    assert_eq!(app_live, app_file);
    assert_state_parity(&file, &live);

    // 2. 最上位に線 1 本、ブロック定義の中に線 1 本ある図面で、最上位の線を削除しつつ同時に 1 本追加する場合:
    //    最上位の件数が変わらずブロック内要素のレコード番号もずれないため shifted == false になる
    let mut archive_blk = Archive::new(700);
    archive_blk.group = 0;
    archive_blk.layer = 0;
    archive_blk.count(1);
    archive_blk.line();
    archive_blk.count(1);
    archive_blk.block("B1");
    archive_blk.out.extend(0u32.to_le_bytes());
    let mut src_blk = spec.build();
    src_blk.extend(&archive_blk.out);

    let mut file_b = read_file(&src_blk);
    let (mut live_b, _) = open_live(&src_blk);
    let (db, _) = live_drawing(&live_b);
    let replacement_line = element_at(&db, 0).1.clone();
    let edits_b = vec![
        Edit::Remove {
            target: seen_at(&db, 0),
        },
        Edit::Add {
            place: "space/0".into(),
            element: Box::new(Placed::Model(replacement_line)),
        },
    ];
    let ((rf_b, _), (rl_b, _)) = apply_both(&mut file_b, &mut live_b, edits_b, None);
    let (af_b, sf_b) = rf_b.unwrap();
    let (al_b, sl_b) = rl_b.unwrap();
    assert!(!sf_b);
    assert_eq!(sl_b, sf_b);
    assert_eq!(al_b, af_b);
    assert_state_parity(&file_b, &live_b);
}

#[test]
fn class_schemas_and_empty_definition_tails_match_file_apply() {
    // 1. 同一クラス名で異なるスキーマ [1, 2] を持つ図面への追加
    let mut spec = Spec::new(700);
    spec.groups[0] = (3, 100.0, [3; 16]);
    let mut archive = Archive::new(700);
    archive.group = 0;
    archive.layer = 0;
    archive.count(2);
    archive.line();
    archive.out.extend([0xFF, 0xFF, 2, 0, 8, 0]);
    archive.out.extend(b"CDataSen");
    archive.base(1, 1);
    archive.reals(&[0.0, 1.0, 10.0, 1.0]);
    archive.count(0);
    archive.out.extend(0u32.to_le_bytes());
    let mut input = spec.build();
    input.extend(&archive.out);

    let mut file = read_file(&input);
    let (mut live, _) = open_live(&input);
    let (d, _) = live_drawing(&live);
    let added = element_at(&d, 0).1.clone();
    let ((rf, _), (rl, _)) = apply_both(
        &mut file,
        &mut live,
        vec![Edit::Add {
            place: "space/0".into(),
            element: Box::new(Placed::Model(added)),
        }],
        None,
    );
    assert_eq!(rl.unwrap(), rf.unwrap());
    assert_state_parity(&file, &live);

    // 2. CDataEnko(schema 1) と CDataSen(schema 2) がある図面で:
    //    Step A: CDataSen を追加（schema 2 が選ばれる）
    //    Step B: CDataSen を 2 本とも削除してから、次の Step C で CDataSen を追加（残存クラス CDataEnko の schema 1 が選ばれる）
    let mut archive2 = Archive::new(700);
    archive2.group = 0;
    archive2.layer = 0;
    archive2.count(2);
    archive2.tag("CDataEnko");
    archive2.base(1, 1);
    archive2.reals(&[0.0, 0.0, 5.0, 0.0, 2.0 * PI, 0.0, 1.0]);
    archive2.out.extend(1u32.to_le_bytes());
    archive2.out.extend([0xFF, 0xFF, 2, 0, 8, 0]);
    archive2.out.extend(b"CDataSen");
    archive2.base(1, 1);
    archive2.reals(&[0.0, 0.0, 10.0, 0.0]);
    archive2.count(0);
    archive2.out.extend(0u32.to_le_bytes());
    let mut input2 = spec.build();
    input2.extend(&archive2.out);

    let mut file2 = read_file(&input2);
    let (mut live2, _) = open_live(&input2);
    let (d_init, _) = live_drawing(&live2);
    let sen_template = element_at(&d_init, 1).1.clone();

    let ((rf, _), (rl, _)) = apply_both(
        &mut file2,
        &mut live2,
        vec![Edit::Add {
            place: "space/0".into(),
            element: Box::new(Placed::Model(sen_template.clone())),
        }],
        None,
    );
    assert_eq!(rl.unwrap(), rf.unwrap());
    assert_state_parity(&file2, &live2);

    let (d_after_a, _) = live_drawing(&live2);
    let ((rf, _), (rl, _)) = apply_both(
        &mut file2,
        &mut live2,
        vec![
            Edit::Remove {
                target: seen_at(&d_after_a, 1),
            },
            Edit::Remove {
                target: seen_at(&d_after_a, 2),
            },
        ],
        None,
    );
    assert_eq!(rl.unwrap(), rf.unwrap());
    assert_state_parity(&file2, &live2);

    let ((rf, _), (rl, _)) = apply_both(
        &mut file2,
        &mut live2,
        vec![Edit::Add {
            place: "space/0".into(),
            element: Box::new(Placed::Model(sen_template)),
        }],
        None,
    );
    assert_eq!(rl.unwrap(), rf.unwrap());
    assert_state_parity(&file2, &live2);

    // 3. ブロック定義が 0 件で末尾が image / trailer の全パターンで完全一致
    let mut image_tail = 1u32.to_le_bytes().to_vec();
    common::cstring(&mut image_tail, "a.bmp");
    image_tail.extend(3u32.to_le_bytes());
    image_tail.extend(b"abc");
    for (version, tail) in [
        (700, 0u32.to_le_bytes().to_vec()),
        (700, image_tail),
        (600, Vec::new()),
        (600, vec![0xEE]),
        (700, vec![0, 0, 0, 0, 0xEE]),
    ] {
        let mut s = Spec::new(version);
        s.groups[0] = (3, 100.0, [3; 16]);
        let mut a = Archive::new(version);
        a.group = 0;
        a.layer = 0;
        a.count(1);
        a.line();
        a.count(0);
        let mut raw = s.build();
        raw.extend(&a.out);
        raw.extend(&tail);

        for is_add in [true, false] {
            let mut f = read_file(&raw);
            let (mut l, _) = open_live(&raw);
            let (dl, _) = live_drawing(&l);
            let edit = if is_add {
                Edit::Add {
                    place: "space/0".into(),
                    element: Box::new(Placed::Model(element_at(&dl, 0).1.clone())),
                }
            } else {
                Edit::Remove {
                    target: seen_at(&dl, 0),
                }
            };
            let ((rf, cf), (rl, cl)) = apply_both(&mut f, &mut l, vec![edit], None);
            assert_eq!(cl, cf);
            assert_eq!(rl.unwrap(), rf.unwrap());
            assert_state_parity(&f, &l);
        }
    }
}

#[test]
fn underlay_record_shift_is_rejected_and_in_place_edit_succeeds() {
    let mut spec = Spec::new(700);
    spec.groups[0] = (3, 100.0, [3; 16]);
    let mut archive = Archive::new(700);
    archive.group = 0;
    archive.layer = 0;
    archive.count(2);
    archive.line();
    archive.tag("CDataMoji");
    archive.base(1, 1);
    archive.text_body("^@BM%temp%a.bmp,20,10");
    archive.count(0);
    archive.out.extend(1u32.to_le_bytes());
    common::cstring(&mut archive.out, "a.bmp");
    archive.out.extend(3u32.to_le_bytes());
    archive.out.extend(b" PNG");
    let mut src = spec.build();
    src.extend(&archive.out);

    let mut file = read_file(&src);
    let (mut live, _) = open_live(&src);
    let state_before = live.state_id();
    let (d, _) = live_drawing(&live);

    // 画像レコードの番号がずれる Remove は JwwFile::apply と同じ診断で拒否し、状態を変えない
    let ((rf, cf), (rl, cl)) = apply_both(
        &mut file,
        &mut live,
        vec![Edit::Remove {
            target: seen_at(&d, 0),
        }],
        None,
    );
    assert_eq!(rl, Err(Error::Rejected));
    assert_eq!(rf, Err(Error::Rejected));
    assert_eq!(cl, cf);
    assert_eq!(cl, [codes::CHANGE_EDIT_UNSUPPORTED]);
    assert_eq!(live.state_id(), state_before);
    assert_eq!(live_save(&live), src);

    // レコード番号がずれない Transform は通り、バイト列も完全一致する
    let ((rf, cf), (rl, cl)) = apply_both(
        &mut file,
        &mut live,
        vec![Edit::Transform {
            target: seen_at(&d, 0),
            transform: Affine {
                m: [1.0, 0.0, 5.0, 0.0, 1.0, 10.0],
            },
        }],
        None,
    );
    assert_eq!(cl, cf);
    assert_eq!(rl.unwrap(), rf.unwrap());
    assert_state_parity(&file, &live);
}

#[test]
fn rejections_and_diagnostics_match_file_apply() {
    let src = base_src();
    let mut file = read_file(&src);
    let (mut live, _) = open_live(&src);
    let state0 = live.state_id();
    let (d, _) = live_drawing(&live);

    // Stale document と存在しない要素 ID を同時に渡すと両方の診断が報告される
    let ((rf, cf), (rl, cl)) = apply_both(
        &mut file,
        &mut live,
        vec![Edit::Remove {
            target: Seen {
                id: "space/0/r999".into(),
                hash: Hash::document(b"x"),
            },
        }],
        Some(Hash::document(b"stale")),
    );
    assert_eq!(rl, Err(Error::Rejected));
    assert_eq!(rf, Err(Error::Rejected));
    assert_eq!(cl, cf);
    assert_eq!(
        cl,
        [codes::CHANGE_DOCUMENT_STALE, codes::CHANGE_ELEMENT_MISSING]
    );
    assert_eq!(live.state_id(), state0);

    // 同じ要素を 1 回の Changes で 2 度指定すると shared 拒否になる
    let t0 = seen_at(&d, 0);
    let ((rf, cf), (rl, cl)) = apply_both(
        &mut file,
        &mut live,
        vec![
            Edit::Remove { target: t0.clone() },
            Edit::Remove { target: t0 },
        ],
        None,
    );
    assert_eq!(rl, Err(Error::Rejected));
    assert_eq!(rf, Err(Error::Rejected));
    assert_eq!(cl, cf);

    // SetPointStyle・未対応スタイル（色欠落）も一致する
    let ((rf, cf), (rl, cl)) = apply_both(
        &mut file,
        &mut live,
        vec![Edit::SetPointStyle {
            style: PointStyle {
                mark: PointMark::Plus,
                circle: false,
                square: false,
                size: PointSize::Length(1.0),
            },
        }],
        None,
    );
    assert_eq!(rl, Err(Error::Rejected));
    assert_eq!(rf, Err(Error::Rejected));
    assert_eq!(cl, cf);

    let bad_line = Element {
        source: Default::default(),
        layer: LayerId(1),
        style: Style {
            color: Inherit::Layer,
            line_type: Inherit::Own(LineTypeId(1)),
            line_type_scale: 1.0,
            line_weight: None,
            transparency: Inherit::Layer,
        },
        shape: Shape::Line {
            start: model(0.0, 0.0),
            end: model(10.0, 0.0),
        },
    };
    let ((rf, cf), (rl, cl)) = apply_both(
        &mut file,
        &mut live,
        vec![Edit::Add {
            place: "space/0".into(),
            element: Box::new(Placed::Model(bad_line)),
        }],
        None,
    );
    assert_eq!(rl, Err(Error::Rejected));
    assert_eq!(rf, Err(Error::Rejected));
    assert_eq!(cl, cf);
    assert_eq!(cl, [codes::CHANGE_STYLE_UNSUPPORTED]);
    assert_eq!(live_save(&live), src);
}

#[test]
fn spatial_selection_display_sync_threads_and_budget_safety() {
    // 320 本の線を持つ図面で 1 スレッドと 4 スレッドの決定性、矩形選択、表示同期、予算耐性を確かめる
    let mut spec = Spec::new(700);
    spec.paper = 3; // A3 (420 x 297, origin = [210, 148.5])
    spec.groups[0] = (3, 1.0, [3; 16]);
    let mut archive = Archive::new(700);
    archive.group = 0;
    archive.layer = 0;
    archive.count(320);
    for i in 0..320 {
        let x = f64::from(i) * 10.0;
        archive.tag("CDataSen");
        archive.base(1, 1);
        archive.reals(&[x, 0.0, x + 5.0, 5.0]);
    }
    archive.count(1);
    archive.block("B");
    archive.out.extend(0u32.to_le_bytes());
    let mut big_src = spec.build();
    big_src.extend(&archive.out);

    let mut b1 = Budget::default();
    let mut d1 = Diagnostics::new();
    let c1 = CancelToken::new();
    let mut live1 =
        JwwLive::open_with_threads(&big_src, 1, &mut Cx::new(&mut b1, &mut d1, &c1)).unwrap();

    let mut b4 = Budget::default();
    let mut d4 = Diagnostics::new();
    let c4 = CancelToken::new();
    let live4 =
        JwwLive::open_with_threads(&big_src, 4, &mut Cx::new(&mut b4, &mut d4, &c4)).unwrap();

    assert_eq!(live_drawing(&live1).0, live_drawing(&live4).0);
    assert_eq!(live_save(&live1), live_save(&live4));
    assert_eq!(live1.display().canonical(), live4.display().canonical());

    // i = 0 (x = 0..5) と i = 1 (x = 10..15) だけを含む範囲を選択
    let mut cx = Cx::new(&mut b1, &mut d1, &c1);
    let hits = live1
        .select_in_space(SpaceId(0), [-1.0, -1.0, 16.0, 6.0], &mut cx)
        .unwrap();
    assert_eq!(hits, ["space/0/r1", "space/0/r2"]);

    // Paper 座標系では origin [210, 148.5] が足される
    let sheet_hits = live1
        .select_sheet([209.0, 147.5, 226.0, 154.5], &mut cx)
        .unwrap();
    assert_eq!(sheet_hits, ["space/0/r1", "space/0/r2"]);

    // ブロック定義空間の選択
    let def_hits = live1
        .select_in_space(SpaceId(16), [-1.0, -1.0, 11.0, 1.0], &mut cx)
        .unwrap();
    assert_eq!(def_hits, ["space/16/r322"]);

    // r1 を遠く (x += 5000) へ動かすと選択範囲から消え、undo で戻る
    let (drw, _) = live_drawing(&live1);
    let t0 = seen_at(&drw, 0);
    live1
        .apply(
            &Changes {
                edits: vec![Edit::Transform {
                    target: t0,
                    transform: Affine {
                        m: [1.0, 0.0, 5000.0, 0.0, 1.0, 0.0],
                    },
                }],
                document: None,
            },
            &mut Cx::new(&mut b1, &mut d1, &c1),
        )
        .unwrap();
    assert!(live1.display().has_dirty());
    let hits_after = live1
        .select_in_space(
            SpaceId(0),
            [-1.0, -1.0, 16.0, 6.0],
            &mut Cx::new(&mut b1, &mut d1, &c1),
        )
        .unwrap();
    assert_eq!(hits_after, ["space/0/r2"]);

    live1.undo(&mut Cx::new(&mut b1, &mut d1, &c1)).unwrap();
    let hits_restored = live1
        .select_in_space(
            SpaceId(0),
            [-1.0, -1.0, 16.0, 6.0],
            &mut Cx::new(&mut b1, &mut d1, &c1),
        )
        .unwrap();
    assert_eq!(hits_restored, ["space/0/r1", "space/0/r2"]);

    // キャンセル済みトークンおよび厳しい予算でパニックせず状態を守る
    let state_before_stop = live1.state_id();
    let cancelled = CancelToken::new();
    cancelled.cancel();
    let mut bc = Budget::default();
    let mut dc = Diagnostics::new();
    let res_cancel = live1.apply(
        &Changes {
            edits: vec![Edit::Remove {
                target: seen_at(&drw, 0),
            }],
            document: None,
        },
        &mut Cx::new(&mut bc, &mut dc, &cancelled),
    );
    assert!(matches!(res_cancel, Err(Error::Stopped(_))));
    assert_eq!(live1.state_id(), state_before_stop);

    let mut tight = Budget::new(Limits::default().with(Kind::Work, Limit::Finite(0)));
    let mut dt = Diagnostics::new();
    let res_tight = JwwLive::open(&big_src, &mut Cx::new(&mut tight, &mut dt, &c1));
    assert!(matches!(res_tight, Err(Error::Stopped(_))));
}

#[test]
fn public_jww_samples_match_file_apply_byte_for_byte() {
    let sample_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../samples/jww");
    if !sample_dir.is_dir() {
        return;
    }
    let mut paths: Vec<_> = std::fs::read_dir(sample_dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("jww"))
        })
        .collect();
    paths.sort();

    for path in paths {
        let src = std::fs::read(&path).unwrap();
        let mut file = read_file(&src);
        let (mut live, _) = open_live(&src);
        assert_state_parity(&file, &live);

        let (drawing, _) = live_drawing(&live);
        let shift = Affine {
            m: [1.0, 0.0, 5.0, 0.0, 1.0, -3.0],
        };
        let mut edits = Vec::new();
        let mut add_line: Option<(String, Element<ModelPoint>)> = None;
        for space in &drawing.spaces {
            let place = id::space(space.id);
            let mut namer = id::Namer::new(&place);
            for (index, el) in space.elements.iter().enumerate() {
                let target = Seen {
                    id: namer.next(index, el),
                    hash: Hash::element(el),
                };
                if matches!(
                    el.shape,
                    Shape::Line { .. } | Shape::Circle { .. } | Shape::Curve(Edge::Arc { .. })
                ) {
                    if add_line.is_none() && matches!(el.shape, Shape::Line { .. }) {
                        add_line = Some((place.clone(), el.clone()));
                    }
                    edits.push(Edit::Transform {
                        target,
                        transform: shift,
                    });
                } else if matches!(el.shape, Shape::Text(_)) {
                    edits.push(Edit::Remove { target });
                }
            }
        }
        if let Some((place, line)) = add_line {
            edits.push(Edit::Add {
                place,
                element: Box::new(Placed::Model(line)),
            });
        }
        let ((rf, cf), (rl, cl)) =
            apply_both(&mut file, &mut live, edits, Some(Hash::document(&src)));
        assert_eq!(cl, cf);
        assert_eq!(rl.unwrap(), rf.unwrap());
        assert_state_parity(&file, &live);

        let mut b = Budget::default();
        let mut d = Diagnostics::new();
        let c = CancelToken::new();
        assert!(live.undo(&mut Cx::new(&mut b, &mut d, &c)).unwrap());
        assert_eq!(live_save(&live), src);
    }
}

/// 文字の要素（`space/0` の最初の文字）の添字と、その要素
#[cfg(test)]
fn first_text(drawing: &Drawing2D) -> (usize, Element<ModelPoint>) {
    let space = drawing.spaces.iter().find(|s| s.id == SpaceId(0)).unwrap();
    space
        .elements
        .iter()
        .enumerate()
        .find(|(_, el)| matches!(el.shape, Shape::Text(_)))
        .map(|(at, el)| (at, el.clone()))
        .unwrap()
}

#[cfg(test)]
fn with_content(element: &Element<ModelPoint>, content: &str, dx: f64) -> Element<ModelPoint> {
    let mut element = element.clone();
    let Shape::Text(text) = &mut element.shape else {
        panic!("text: {element:?}");
    };
    text.content = content.into();
    if let zumen_drawing::Placement::Anchor { point, .. } = &mut text.placement {
        point.x += dx;
    }
    element
}

/// 文字の色・置き換え・移動・追加を、文書エンジンが断らずに当て、今までの経路と同じバイト列・図面モデル・
/// 適用の結果・診断になる。取り消すと元のバイト列に戻る（#861）
#[test]
fn text_edits_match_file_apply_without_refusing() {
    let src = base_src();
    let mut file = read_file(&src);
    let (mut live, _) = open_live(&src);

    let (d, _) = live_drawing(&live);
    let (at, text) = first_text(&d);
    let ((rf, cf), (rl, cl)) = apply_both(
        &mut file,
        &mut live,
        vec![Edit::Restyle {
            target: seen_at(&d, at),
            style: Restyle {
                color: Some(Inherit::Own(Color {
                    rgb: None,
                    code: Some(ColorCode {
                        system: ColorSystem::Jww,
                        value: 5,
                    }),
                })),
                ..Restyle::default()
            },
        }],
        None,
    );
    assert_eq!(cl, cf);
    assert_eq!(rl.unwrap(), rf.unwrap());
    assert_state_parity(&file, &live);

    // 長い文字列への置き換えと、短い文字列の追加は、一覧を数え直して書く
    let (d, _) = live_drawing(&live);
    let (at, _) = first_text(&d);
    let ((rf, cf), (rl, cl)) = apply_both(
        &mut file,
        &mut live,
        vec![
            Edit::Replace {
                target: seen_at(&d, at),
                element: Box::new(Placed::Model(with_content(
                    &text,
                    "置き換えた長い文字列",
                    3.0,
                ))),
            },
            Edit::Add {
                place: "space/0".into(),
                element: Box::new(Placed::Model(with_content(&text, "A", -2.0))),
            },
        ],
        None,
    );
    assert_eq!(cl, cf);
    assert_eq!(rl.unwrap(), rf.unwrap());
    assert_state_parity(&file, &live);

    let (d, _) = live_drawing(&live);
    let (at, _) = first_text(&d);
    let ((rf, cf), (rl, cl)) = apply_both(
        &mut file,
        &mut live,
        vec![Edit::Transform {
            target: seen_at(&d, at),
            transform: Affine {
                m: [0.0, -1.0, 4.0, 1.0, 0.0, -2.0],
            },
        }],
        None,
    );
    assert_eq!(cl, cf);
    assert_eq!(rl.unwrap(), rf.unwrap());
    assert_state_parity(&file, &live);

    let mut b = Budget::default();
    let mut diag = Diagnostics::new();
    let c = CancelToken::new();
    for _ in 0..3 {
        assert!(live.undo(&mut Cx::new(&mut b, &mut diag, &c)).unwrap());
    }
    assert_eq!(live_save(&live), src);
}

/// レイヤの改名・状態の変更・追加を、文書エンジンが断らずに当て、今までの経路と同じバイト列・図面モデル・
/// 適用の結果・診断になる。断る変更も、同じ診断で全体を断る。取り消し・やり直しでヘッダーも戻る（#861）
#[test]
fn layer_edits_match_file_apply_without_refusing() {
    let src = base_src();
    let mut file = read_file(&src);
    let (mut live, _) = open_live(&src);
    let (d, _) = live_drawing(&live);
    let first = &d.layers[0];
    let seen_layer = |layer: &zumen_drawing::Layer| Seen {
        id: id::layer(layer),
        hash: Hash::layer(layer),
    };

    // 長い名前への改名と、同じ変更の中の文字の置き換え（ヘッダーと一覧の両方を書き直す）
    let mut renamed = first.clone();
    renamed.name = "改訂図面の注記😀".repeat(40);
    let (at, text) = first_text(&d);
    let ((rf, cf), (rl, cl)) = apply_both(
        &mut file,
        &mut live,
        vec![
            Edit::ReplaceLayer {
                target: seen_layer(first),
                layer: Box::new(renamed),
            },
            Edit::Replace {
                target: seen_at(&d, at),
                element: Box::new(Placed::Model(with_content(&text, "注記", 0.0))),
            },
        ],
        Some(Hash::document(&src)),
    );
    assert_eq!(cl, cf);
    assert_eq!(rl.unwrap(), rf.unwrap());
    assert_state_parity(&file, &live);
    let renamed_bytes = live_save(&live);

    // 状態の変更と、空いた枠への追加。ヘッダーだけを書き直す
    let (d, _) = live_drawing(&live);
    let mut hidden = d.layers[1].clone();
    hidden.name = "非表示".into();
    hidden.state = Some(zumen_drawing::LayerState {
        visible: false,
        printable: false,
        editable: false,
        frozen: false,
    });
    let mut added = d.layers[0].clone();
    added.name = "追加".into();
    let ((rf, cf), (rl, cl)) = apply_both(
        &mut file,
        &mut live,
        vec![
            Edit::ReplaceLayer {
                target: seen_layer(&d.layers[1]),
                layer: Box::new(hidden),
            },
            Edit::AddLayer {
                layer: Box::new(added),
            },
        ],
        None,
    );
    assert_eq!(cl, cf);
    assert_eq!(rl.unwrap(), rf.unwrap());
    assert_state_parity(&file, &live);
    let added_bytes = live_save(&live);

    // 古いハッシュ・書けない状態は、要素の変更とともに同じ診断で全体を断る
    let (d, _) = live_drawing(&live);
    let mut unprintable = d.layers[0].clone();
    unprintable.state.as_mut().unwrap().printable = false;
    for (target, layer) in [
        (
            Seen {
                hash: Hash::document(b"stale"),
                ..seen_layer(&d.layers[0])
            },
            d.layers[0].clone(),
        ),
        (seen_layer(&d.layers[0]), unprintable),
    ] {
        let ((rf, cf), (rl, cl)) = apply_both(
            &mut file,
            &mut live,
            vec![
                Edit::Remove {
                    target: seen_at(&d, 0),
                },
                Edit::ReplaceLayer {
                    target,
                    layer: Box::new(layer),
                },
            ],
            None,
        );
        assert_eq!(rl, Err(Error::Rejected));
        assert_eq!(rf, Err(Error::Rejected));
        assert_eq!(cl, cf);
        assert_eq!(live_save(&live), added_bytes);
    }

    let mut b = Budget::default();
    let mut diag = Diagnostics::new();
    let c = CancelToken::new();
    let mut cx = Cx::new(&mut b, &mut diag, &c);
    assert!(live.undo(&mut cx).unwrap());
    assert_eq!(live_save(&live), renamed_bytes);
    assert!(live.undo(&mut cx).unwrap());
    assert_eq!(live_save(&live), src);
    assert_eq!(live_drawing(&live).0, lift_file(&read_file(&src)).0);
    assert!(live.redo(&mut cx).unwrap());
    assert!(live.redo(&mut cx).unwrap());
    assert_eq!(live_save(&live), added_bytes);
    assert_state_parity(&file, &live);
}
