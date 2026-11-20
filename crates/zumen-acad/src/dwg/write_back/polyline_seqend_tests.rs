use std::borrow::Cow;
use std::collections::BTreeMap;

use super::*;
use crate::{AcadLive, CopyFrom, Dwg};
use zumen_base::{Budget, CancelToken, Diagnostics, Key};
use zumen_drawing::{
    Affine, Changes, Color, ColorCode, ColorSystem, Edit, Inherit, LayerId, LineType,
    LineTypeDefinition, LineTypeId, PatternBasis, Restyle as DrawingRestyle, Seen, Shape, id,
};
use zumen_sdk::{Apply, Codec, Lift};

fn int_val(code: i16, value: i64) -> Value {
    Value {
        code,
        data: Data::Int(value),
    }
}

fn real_val(code: i16, value: f64) -> Value {
    Value {
        code,
        data: Data::Real(value),
    }
}

fn encode_r2010_vertex2d(
    entity: &NewEntity<'_>,
    release: Release,
    x: f64,
    y: f64,
) -> Option<Vec<u8>> {
    let object = &*entity.object;
    let mut out = BitWriter::default();
    out.bot(i16::try_from(kinds::VERTEX_2D).ok()?);
    out.handle(0, entity.handle);
    out.bs(0);
    out.bit(false);
    out.bb(entity.mode);
    out.bl(0);
    out.bit(true);
    if release >= Release::R2013 {
        out.bit(false);
    }
    out.bs(small(object, 62)?.unwrap_or(256));
    out.bd(1.0);
    out.bb(entity.ltype.0);
    out.bb(0);
    out.bb(0);
    out.rc(0);
    for _ in 0..3 {
        out.bit(false);
    }
    out.bs(0);
    out.rc(lineweight_index(-1)?.try_into().ok()?);

    let mut fields = BitWriter::with_strings();
    fields.rc(0);
    fields.bd(x);
    fields.bd(y);
    fields.bd(0.0);
    fields.bd(0.0);
    fields.bd(0.0);
    fields.bd(0.0);
    fields.bl(0);
    fields.bd(0.0);

    let mut handles = BitWriter::default();
    if entity.mode == 0 {
        handles.handle(4, entity.owner);
    }
    handles.handle(5, entity.layer);
    if entity.ltype.0 == 3 {
        handles.handle(5, entity.ltype.1);
    }
    r2010_assemble(out, fields, handles)
}

/// オブジェクトのマップ順で `POLYLINE < SEQEND < VERTEX` となる DWG を組み立てる（#914）。
/// `subentity_mode` が 2 のときは `VERTEX`・`SEQEND` が自身の所有者に親を持たない書き方（ACadSharp など）、
/// 0 のときは親ハンドルを直接指す書き方を表す
fn build_seqend_before_vertex_dwg(sample_name: &str, subentity_mode: u8) -> (Vec<u8>, u64) {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../samples/dwg");
    let mut budget = Budget::default();
    let mut diagnostics = Diagnostics::new();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
    let src = std::fs::read(dir.join(sample_name)).unwrap();
    let mut file = Dwg::read(&src, &mut cx).unwrap();
    let base_drawing = file.lift(&mut cx).unwrap();
    let mut wall_layer = base_drawing.layers.first().unwrap().clone();
    wall_layer.id = LayerId(0);
    wall_layer.name = "WALL".to_owned();
    wall_layer.follows_placement = false;

    // スタイル変更先となる画層 WALL と線種 DASHED を先に足しておく
    let setup = Changes {
        edits: vec![
            Edit::AddLayer {
                layer: Box::new(wall_layer),
            },
            Edit::AddLineType {
                line_type: Box::new(LineType {
                    id: LineTypeId(0),
                    name: "DASHED".to_owned(),
                    definition: Some(LineTypeDefinition {
                        description: "Dashed".to_owned(),
                        pattern: vec![5.0, -2.5],
                        basis: PatternBasis::Element,
                        marks: Vec::new(),
                    }),
                    source: zumen_base::Ref::default(),
                }),
            },
        ],
        document: None,
    };
    file.apply(&setup, &mut cx).unwrap();

    let db = file.objects(&mut cx).unwrap();
    let (hdr_bytes, hdr_span) = file.header_section().unwrap();
    let hdr = header::header(hdr_bytes, hdr_span, file.release, &mut cx).unwrap();
    let model_owner = hdr.model;
    let records: BTreeMap<u64, usize> = file
        .entries
        .iter()
        .enumerate()
        .map(|(idx, e)| (e.handle, idx))
        .collect();
    let model_record = records[&model_owner];
    let layer_0 = record_handle(&db, b"LAYER", b"0").unwrap();

    let base_handle = file.entries.iter().map(|e| e.handle).max().unwrap() + 0x100;
    let poly_h = base_handle;
    let seqend_h = base_handle + 1;
    let verts_h = [
        base_handle + 2,
        base_handle + 3,
        base_handle + 4,
        base_handle + 5,
    ];
    let coords = [(10.0, 20.0), (22.0, 20.0), (25.0, 28.0), (12.0, 33.0)];

    let mut block_changes = BTreeMap::new();
    let poly_prev = if file.release >= Release::R2010 {
        let (data, at) = file.current_r2010(&block_changes, model_record).unwrap();
        let patched = adopt_r2010(&data, at, model_owner, &[poly_h], file.release).unwrap();
        block_changes.insert(model_record, patched);
        0
    } else if file.release == Release::R2004 {
        let data = file.current(&block_changes, model_record).unwrap();
        let patched = adopt(&data, model_owner, &[poly_h]).unwrap();
        block_changes.insert(model_record, patched);
        0
    } else {
        let (first, _, chain) = file
            .chain(model_record, model_owner, &records, &block_changes)
            .unwrap();
        let joined = file
            .join(
                model_record,
                model_owner,
                first,
                &chain,
                &[poly_h],
                &mut block_changes,
            )
            .unwrap();
        joined[0].0
    };

    let poly_obj = Object {
        kind: b"POLYLINE".to_vec(),
        section: Section::Entities,
        origin: Origin::New,
        values: vec![
            int_val(66, 1),
            int_val(70, 1),
            real_val(10, 0.0),
            real_val(20, 0.0),
            real_val(30, 0.0),
            int_val(62, 2),
        ],
    };
    let poly_ent = NewEntity {
        object: Cow::Borrowed(&poly_obj),
        eed: Vec::new(),
        handle: poly_h,
        type_code: kinds::POLYLINE_2D,
        mode: 2,
        owner: model_owner,
        layer: layer_0,
        ltype: (2, 0),
        pointed: None,
        extra_pointed: None,
        embedded: None,
        children: Some((verts_h.to_vec(), seqend_h)),
    };
    let poly_bytes = poly_ent.encode(poly_prev, 0, file.release).unwrap();

    let sub_owner = if subentity_mode == 0 {
        poly_h
    } else {
        model_owner
    };
    let seqend_obj = Object {
        kind: b"SEQEND".to_vec(),
        section: Section::Entities,
        origin: Origin::New,
        values: Vec::new(),
    };
    let seqend_ent = NewEntity {
        object: Cow::Borrowed(&seqend_obj),
        eed: Vec::new(),
        handle: seqend_h,
        type_code: kinds::SEQEND,
        mode: subentity_mode,
        owner: sub_owner,
        layer: layer_0,
        ltype: (0, 0),
        pointed: None,
        extra_pointed: None,
        embedded: None,
        children: None,
    };
    let seqend_bytes = seqend_ent.encode(0, 0, file.release).unwrap();

    let mut vert_encoded = Vec::with_capacity(coords.len());
    for (i, &(x, y)) in coords.iter().enumerate() {
        let prev_v = i.checked_sub(1).map_or(0, |p| verts_h[p]);
        let next_v = verts_h.get(i + 1).copied().unwrap_or(0);
        let vert_obj = Object {
            kind: b"VERTEX".to_vec(),
            section: Section::Entities,
            origin: Origin::New,
            values: vec![
                int_val(70, 0),
                real_val(10, x),
                real_val(20, y),
                real_val(30, 0.0),
                int_val(62, 2),
            ],
        };
        let vert_ent = NewEntity {
            object: Cow::Borrowed(&vert_obj),
            eed: Vec::new(),
            handle: verts_h[i],
            type_code: kinds::VERTEX_2D,
            mode: subentity_mode,
            owner: sub_owner,
            layer: layer_0,
            ltype: (2, 0),
            pointed: None,
            extra_pointed: None,
            embedded: None,
            children: None,
        };
        let bytes = if file.release >= Release::R2010 {
            encode_r2010_vertex2d(&vert_ent, file.release, x, y).unwrap()
        } else {
            vert_ent.encode(prev_v, next_v, file.release).unwrap()
        };
        vert_encoded.push((verts_h[i], bytes));
    }

    let mut txn = file.transaction();
    for (rec, data) in block_changes {
        txn.set_data(rec, &data).unwrap();
    }
    // POLYLINE -> SEQEND -> VERTEX_0..3 のハンドル順で追加する
    txn.add(poly_h, &poly_bytes).unwrap();
    txn.add(seqend_h, &seqend_bytes).unwrap();
    for (vh, data) in vert_encoded {
        txn.add(vh, &data).unwrap();
    }
    txn.set_handle_seed(base_handle + 6).unwrap();
    txn.commit(&mut cx).unwrap();

    let mut out = Vec::new();
    Dwg::write(&file, &mut out, &mut cx).unwrap();
    assert!(diagnostics.is_empty(), "{sample_name}: {diagnostics:?}");
    (out, poly_h)
}

/// DWG のオブジェクトのマップ順で `SEQEND` が `VERTEX` より前にある `POLYLINE` でも、
/// 一括読み込みと常駐文書で全頂点を読み、同じスタイルを書き戻す（#914）
#[test]
fn dwg_polyline_with_seqend_before_vertices_lifts_and_restyles() {
    let expected_xy = [(10.0, 20.0), (22.0, 20.0), (25.0, 28.0), (12.0, 33.0)];
    let magenta = Color {
        code: Some(ColorCode {
            system: ColorSystem::Aci,
            value: 6,
        }),
        rgb: None,
    };

    for sample_name in [
        "zumen-layout-r2000.dwg",
        "zumen-layout-r2004.dwg",
        "zumen-layout-r2018.dwg",
    ] {
        for subentity_mode in [2u8, 0u8] {
            let (dwg_bytes, poly_h) = build_seqend_before_vertex_dwg(sample_name, subentity_mode);

            let mut budget = Budget::default();
            let mut diagnostics = Diagnostics::new();
            let cancel = CancelToken::new();
            let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);

            let mut batch_file = Dwg::read(&dwg_bytes, &mut cx).unwrap();
            // オブジェクトのマップ上で POLYLINE -> SEQEND -> VERTEX 順に並んでいることを確かめる
            let poly_pos = batch_file
                .entries
                .iter()
                .position(|e| e.handle == poly_h)
                .unwrap();
            assert_eq!(batch_file.entries[poly_pos + 1].handle, poly_h + 1);
            assert_eq!(batch_file.entries[poly_pos + 1].type_code, kinds::SEQEND);
            for offset in 2..=5 {
                assert_eq!(
                    batch_file.entries[poly_pos + offset].handle,
                    poly_h + offset as u64
                );
                assert_eq!(
                    batch_file.entries[poly_pos + offset].type_code,
                    kinds::VERTEX_2D
                );
            }

            let drawing = batch_file.lift(&mut cx).unwrap();
            let poly_elem = drawing
                .spaces
                .first()
                .unwrap()
                .elements
                .iter()
                .find(|el| el.source.key() == Some(Key::Handle(poly_h)))
                .unwrap_or_else(|| {
                    panic!("{sample_name} mode={subentity_mode}: missing h{poly_h:X}")
                });
            let Shape::Polyline(poly_shape) = &poly_elem.shape else {
                panic!("expected polyline, got {:?}", poly_elem.shape);
            };
            assert!(poly_shape.closed);
            let lifted_xy: Vec<(f64, f64)> = poly_shape
                .vertices
                .iter()
                .map(|v| (v.point.x, v.point.y))
                .collect();
            assert_eq!(
                lifted_xy, expected_xy,
                "{sample_name} mode={subentity_mode}"
            );

            let wall_id = drawing.layers.iter().find(|l| l.name == "WALL").unwrap().id;
            let dashed_id = drawing
                .line_types
                .iter()
                .find(|lt| lt.name == "DASHED")
                .unwrap()
                .id;
            let target_id = format!("space/0/h{poly_h:X}");
            let seen = Seen {
                id: target_id,
                hash: zumen_drawing::Hash::element(poly_elem),
            };
            let restyle_changes = Changes {
                edits: vec![Edit::Restyle {
                    target: seen.clone(),
                    style: DrawingRestyle {
                        layer: Some(wall_id),
                        color: Some(Inherit::Own(magenta)),
                        line_type: Some(Inherit::Own(dashed_id)),
                        ..DrawingRestyle::default()
                    },
                }],
                document: None,
            };

            let batch_applied = batch_file.apply(&restyle_changes, &mut cx).unwrap();
            let mut batch_saved = Vec::new();
            Dwg::write(&batch_file, &mut batch_saved, &mut cx).unwrap();

            // 書き戻した DWG の全 VERTEX に画層・色・線種が入り、SEQEND に画層が入っていることを確かめる
            let reloaded = Dwg::read(&batch_saved, &mut cx).unwrap();
            let reloaded_db = reloaded.objects(&mut cx).unwrap();
            for vh in [poly_h, poly_h + 2, poly_h + 3, poly_h + 4, poly_h + 5] {
                let obj = reloaded_db.get(vh).unwrap();
                assert_eq!(
                    text_of(obj, 8),
                    Some(b"WALL".as_slice()),
                    "{sample_name} mode={subentity_mode} h={vh:X}"
                );
                assert_eq!(
                    text_of(obj, 6),
                    Some(b"DASHED".as_slice()),
                    "{sample_name} mode={subentity_mode} h={vh:X}"
                );
                assert_eq!(
                    small(obj, 62),
                    Some(Some(6)),
                    "{sample_name} mode={subentity_mode} h={vh:X}"
                );
            }
            let seqend_obj = reloaded_db.get(poly_h + 1).unwrap();
            assert_eq!(
                text_of(seqend_obj, 8),
                Some(b"WALL".as_slice()),
                "{sample_name} mode={subentity_mode} seqend"
            );

            // 常駐文書エンジン（1 スレッド・4 スレッド）でも読み取り・restyle・undo・redo・save が一致することを確かめる
            for threads in [1, 4] {
                let mut live =
                    AcadLive::open_dwg_with_threads(&dwg_bytes, threads, &mut cx).unwrap();
                assert_eq!(live.drawing(&mut cx).unwrap(), drawing);
                let live_applied = live.apply(&restyle_changes, &mut cx).unwrap();
                assert_eq!(live_applied, batch_applied);
                let live_saved = live.save(&mut cx).unwrap();
                assert_eq!(
                    live_saved, batch_saved,
                    "{sample_name} mode={subentity_mode} threads={threads}"
                );
                assert!(live.undo(&mut cx).unwrap());
                assert_eq!(live.drawing(&mut cx).unwrap(), drawing);
                assert!(live.redo(&mut cx).unwrap());
                assert_eq!(live.save(&mut cx).unwrap(), batch_saved);
            }

            assert!(
                diagnostics.is_empty(),
                "{sample_name} mode={subentity_mode}: {diagnostics:?}"
            );
        }
    }
}

/// 属性付き `INSERT` でもオブジェクトのマップ順で `SEQEND < ATTRIB` のとき、
/// 常駐文書の移動と用紙空間への複写で属性を保つ（#914）
#[test]
fn dwg_insert_with_seqend_before_attrib_edits_and_copies_to_sheet() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../samples/dwg");
    for sample_name in ["zumen-minsert-r2000.dwg", "zumen-minsert-r2018.dwg"] {
        for subentity_mode in [2u8, 0u8] {
            let mut budget = Budget::default();
            let mut diagnostics = Diagnostics::new();
            let cancel = CancelToken::new();
            let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
            let src = std::fs::read(dir.join(sample_name)).unwrap();
            let mut file = Dwg::read(&src, &mut cx).unwrap();
            let db = file.objects(&mut cx).unwrap();
            let (hdr_bytes, hdr_span) = file.header_section().unwrap();
            let hdr = header::header(hdr_bytes, hdr_span, file.release, &mut cx).unwrap();
            let model_owner = hdr.model;
            let records: BTreeMap<u64, usize> = file
                .entries
                .iter()
                .enumerate()
                .map(|(idx, e)| (e.handle, idx))
                .collect();
            let model_record = records[&model_owner];
            let layer_0 = record_handle(&db, b"LAYER", b"0").unwrap();
            let style_std = record_handle(&db, b"STYLE", b"Standard").unwrap();
            let (blk_handle, blk_name) = db
                .objects()
                .iter()
                .find_map(|o| {
                    if o.kind == b"BLOCK_RECORD" {
                        let name = text_of(o, 2)?;
                        if !name.starts_with(b"*") {
                            return Some((o.handle()?, name.to_vec()));
                        }
                    }
                    None
                })
                .unwrap();

            let base_h = file.entries.iter().map(|e| e.handle).max().unwrap() + 0x100;
            let ins_h = base_h;
            let seq_h = base_h + 1;
            let att_h = base_h + 2;

            let mut block_changes = BTreeMap::new();
            let ins_prev = if file.release >= Release::R2010 {
                let (data, at) = file.current_r2010(&block_changes, model_record).unwrap();
                let patched = adopt_r2010(&data, at, model_owner, &[ins_h], file.release).unwrap();
                block_changes.insert(model_record, patched);
                0
            } else {
                let (first, _, chain) = file
                    .chain(model_record, model_owner, &records, &block_changes)
                    .unwrap();
                let joined = file
                    .join(
                        model_record,
                        model_owner,
                        first,
                        &chain,
                        &[ins_h],
                        &mut block_changes,
                    )
                    .unwrap();
                joined[0].0
            };

            let ins_obj = Object {
                kind: b"INSERT".to_vec(),
                section: Section::Entities,
                origin: Origin::New,
                values: vec![
                    Value {
                        code: 2,
                        data: Data::Text(blk_name),
                    },
                    int_val(66, 1),
                    real_val(10, 5.0),
                    real_val(20, 8.0),
                    real_val(30, 0.0),
                ],
            };
            let ins_ent = NewEntity {
                object: Cow::Borrowed(&ins_obj),
                eed: Vec::new(),
                handle: ins_h,
                type_code: kinds::INSERT,
                mode: 2,
                owner: model_owner,
                layer: layer_0,
                ltype: (0, 0),
                pointed: Some(blk_handle),
                extra_pointed: None,
                embedded: None,
                children: Some((vec![att_h], seq_h)),
            };
            let ins_bytes = ins_ent.encode(ins_prev, 0, file.release).unwrap();

            let sub_owner = if subentity_mode == 0 {
                ins_h
            } else {
                model_owner
            };
            let seq_obj = Object {
                kind: b"SEQEND".to_vec(),
                section: Section::Entities,
                origin: Origin::New,
                values: Vec::new(),
            };
            let seq_ent = NewEntity {
                object: Cow::Borrowed(&seq_obj),
                eed: Vec::new(),
                handle: seq_h,
                type_code: kinds::SEQEND,
                mode: subentity_mode,
                owner: sub_owner,
                layer: layer_0,
                ltype: (0, 0),
                pointed: None,
                extra_pointed: None,
                embedded: None,
                children: None,
            };
            let seq_bytes = seq_ent.encode(0, 0, file.release).unwrap();

            let mut att_vals = vec![
                real_val(10, 6.0),
                real_val(20, 9.0),
                real_val(30, 0.0),
                real_val(40, 2.5),
                Value {
                    code: 1,
                    data: Data::Text(b"VAL1".to_vec()),
                },
                Value {
                    code: 2,
                    data: Data::Text(b"TAG1".to_vec()),
                },
                int_val(70, 0),
            ];
            if file.release >= Release::R2010 {
                att_vals.insert(0, int_val(280, 0));
                att_vals.push(int_val(280, 0));
            }
            let att_obj = Object {
                kind: b"ATTRIB".to_vec(),
                section: Section::Entities,
                origin: Origin::New,
                values: att_vals,
            };
            let att_ent = NewEntity {
                object: Cow::Borrowed(&att_obj),
                eed: Vec::new(),
                handle: att_h,
                type_code: kinds::ATTRIB,
                mode: subentity_mode,
                owner: sub_owner,
                layer: layer_0,
                ltype: (0, 0),
                pointed: Some(style_std),
                extra_pointed: None,
                embedded: None,
                children: None,
            };
            let att_bytes = att_ent.encode(0, 0, file.release).unwrap();

            let mut txn = file.transaction();
            for (rec, data) in block_changes {
                txn.set_data(rec, &data).unwrap();
            }
            txn.add(ins_h, &ins_bytes).unwrap();
            txn.add(seq_h, &seq_bytes).unwrap();
            txn.add(att_h, &att_bytes).unwrap();
            txn.set_handle_seed(base_h + 3).unwrap();
            txn.commit(&mut cx).unwrap();

            let mut dwg_bytes = Vec::new();
            Dwg::write(&file, &mut dwg_bytes, &mut cx).unwrap();

            let mut batch = Dwg::read(&dwg_bytes, &mut cx).unwrap();
            let drawing = batch.lift(&mut cx).unwrap();
            let ins_elem = drawing.spaces[0]
                .elements
                .iter()
                .find(|el| el.source.key() == Some(Key::Handle(ins_h)))
                .unwrap();
            let Shape::Symbol(sym) = &ins_elem.shape else {
                panic!("expected symbol");
            };
            assert_eq!(sym.attributes.len(), 1);
            assert_eq!(sym.attributes[0].text.content, "VAL1");

            let seen = Seen {
                id: format!("space/0/h{ins_h:X}"),
                hash: zumen_drawing::Hash::element(ins_elem),
            };
            // 高速経路が動く Transform で decode_encoded_in の所有者補正が保たれることを確かめる
            let move_changes = Changes {
                edits: vec![Edit::Transform {
                    target: seen.clone(),
                    transform: Affine::translation(3.0, 4.0),
                }],
                document: None,
            };
            let mut live = AcadLive::open_dwg(&dwg_bytes, &mut cx).unwrap();
            let (live_res, rereads) = live.apply_rereading(&move_changes, &mut cx);
            let live_applied = live_res.unwrap();
            assert!(
                !rereads,
                "{sample_name} mode={subentity_mode} should use fast path"
            );
            let batch_applied = batch.apply(&move_changes, &mut cx).unwrap();
            assert_eq!(live_applied, batch_applied);
            let mut batch_saved = Vec::new();
            Dwg::write(&batch, &mut batch_saved, &mut cx).unwrap();
            assert_eq!(live.save(&mut cx).unwrap(), batch_saved);

            // 用紙空間への Copy でも ATTRIB・SEQEND の 67 フラグが正しく整えられて書き戻せることを確かめる
            let mut dst_dwg = Dwg::read(&dwg_bytes, &mut cx).unwrap();
            let sheet_id = id::SheetNamer::default().next(drawing.sheets.first().unwrap());
            let copy_to_sheet = Changes {
                edits: vec![Edit::Copy {
                    target: seen,
                    transform: Affine::translation(1.0, 2.0),
                }],
                document: None,
            };
            let sheet_copied = dst_dwg
                .apply_with_copies(
                    &copy_to_sheet,
                    &[CopyFrom {
                        place: Some(sheet_id.as_str()),
                        from: None,
                    }],
                    &mut cx,
                )
                .unwrap();
            let copied_id = sheet_copied[0].id.as_deref().unwrap();
            let mut output = Vec::new();
            Dwg::write(&dst_dwg, &mut output, &mut cx).unwrap();
            let saved = Dwg::read(&output, &mut cx).unwrap().lift(&mut cx).unwrap();
            let sheet = &saved.sheets[0];
            let mut names = id::Namer::new(&sheet_id);
            let (_, copy) = sheet
                .elements
                .iter()
                .enumerate()
                .find(|(index, el)| names.next(*index, el) == copied_id)
                .unwrap();
            let Shape::Symbol(symbol) = &copy.shape else {
                panic!("expected copied INSERT");
            };
            assert_eq!(symbol.attributes.len(), 1);
            assert_eq!(symbol.attributes[0].text.content, "VAL1");
        }
    }
}
