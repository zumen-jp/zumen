use super::*;
use crate::Dxf;
use zumen_base::{Budget, CancelToken, Diagnostics, Limit, Limits};
use zumen_sdk::Codec;

impl AcadDb {
    fn reencode_r2004_dxf(&self, cx: &mut Cx<'_>) -> Result<Option<ReencodedDxf>, Error> {
        try_reencode(self, 0, cx, |_, _| Ok(fixture_source()))
    }
}

fn fixture_source() -> DxfSource<'static> {
    DxfSource {
        order: BTreeMap::from([(10, vec![41]), (11, vec![]), (12, vec![40])]),
        ..DxfSource::default()
    }
}

#[test]
fn lost_reference_index_checks_each_visit_and_each_new_allocation() {
    let db = fixture();
    let object = db.get(50).unwrap();
    let references = [(object, 350), (object, 360)];
    let mut budget = Budget::default();
    let mut diagnostics = Diagnostics::new();
    let cancel = CancelToken::new();
    let handles = lost_reference_handles(
        &references,
        &mut Cx::new(&mut budget, &mut diagnostics, &cancel),
    )
    .unwrap();
    assert_eq!(handles, BTreeSet::from([50]));
    assert_eq!(budget.used(Kind::DecodedBytes), 128);
    assert_eq!(budget.used(Kind::Work), 3);
    for limits in [
        Limits::default().with(Kind::DecodedBytes, Limit::Finite(127)),
        Limits::default().with(Kind::Work, Limit::Finite(0)),
    ] {
        let mut budget = Budget::new(limits);
        assert!(matches!(
            lost_reference_handles(
                &references,
                &mut Cx::new(&mut budget, &mut diagnostics, &cancel)
            ),
            Err(Error::Stopped(_))
        ));
        assert_eq!(budget.used(Kind::DecodedBytes), 0);
    }
    cancel.cancel();
    assert!(matches!(
        lost_reference_handles(
            &references,
            &mut Cx::new(&mut budget, &mut diagnostics, &cancel)
        ),
        Err(Error::Stopped(zumen_base::Stop::Cancelled))
    ));
}

#[test]
fn report_counts_each_record_once_and_keeps_the_source_handle() {
    let db = fixture();
    let run = |limit| {
        let mut budget = Budget::default();
        let mut diagnostics = Diagnostics::new();
        let cancel = CancelToken::new();
        let output = try_reencode(
            &db,
            limit,
            &mut Cx::new(&mut budget, &mut diagnostics, &cancel),
            |_, _| Ok(fixture_source()),
        )
        .unwrap()
        .unwrap();
        (output, diagnostics.into_vec())
    };
    let (full, diagnostics) = run(1000);
    assert_eq!(full.report.total, db.objects().len() as u64 + 1);
    assert_eq!(
        full.report
            .counts
            .iter()
            .map(|count| count.count)
            .sum::<u64>(),
        full.report.total
    );
    assert_eq!(
        full.report
            .counts
            .iter()
            .map(|count| count.output_elements)
            .sum::<u64>(),
        full.elements
    );
    let dimension = full
        .report
        .details
        .iter()
        .find(|detail| detail.kind == "ARC_DIMENSION")
        .unwrap();
    assert_eq!(dimension.fidelity, ConversionFidelity::Native);
    assert_eq!(dimension.output_elements, 1);
    assert_eq!(dimension.source.key(), Some(zumen_base::Key::Handle(41)));
    for limit in [0, 1] {
        let (short, notices) = run(limit);
        assert_eq!(short.bytes, full.bytes);
        assert_eq!(short.elements, full.elements);
        assert_eq!(short.report.counts, full.report.counts);
        assert_eq!(short.report.total, full.report.total);
        assert_eq!(short.report.details.len(), limit as usize);
        assert!(short.report.truncated);
        assert_eq!(notices, diagnostics);
    }
}

fn s(code: i16, name: &[u8]) -> Value {
    Value {
        code,
        data: Data::Text(name.to_vec()),
    }
}
fn h(code: i16, id: u64) -> Value {
    Value {
        code,
        data: Data::Handle(id),
    }
}
fn i(code: i16, n: i64) -> Value {
    Value {
        code,
        data: Data::Int(n),
    }
}
fn r(code: i16, n: f64) -> Value {
    Value {
        code,
        data: Data::Real(n),
    }
}

// DWG のリーダーが渡す L1 の最小の図面。1 個の寸法は 1 個の点を含む匿名ブロックを指す。
fn fixture() -> AcadDb {
    use Section::{Blocks, Entities, Objects, Tables};
    let mut db = AcadDb::default();
    db.set_variable(b"$ACADVER", vec![s(1, b"AC1018")]);
    db.set_variable(b"$DWGCODEPAGE", vec![s(3, b"ANSI_932")]);
    let mut record = |section, kind: &[u8], id: u64, owner: u64, body: Vec<Value>| {
        let offset = id as usize * 16;
        let mut values = vec![
            h(if kind == b"DIMSTYLE" { 105 } else { 5 }, id),
            h(330, owner),
        ];
        values.extend(body);
        db.push(Object {
            kind: kind.to_vec(),
            section,
            values,
            origin: Origin::Dwg {
                offset,
                end: offset + 8,
            },
        });
    };
    for id in 1..=5 {
        record(Tables, b"TABLE", id, 0, vec![]);
    }
    record(Tables, b"LTYPE", 6, 1, vec![s(2, b"Continuous")]);
    record(
        Tables,
        b"LAYER",
        7,
        2,
        vec![s(2, b"0"), s(6, b"Continuous")],
    );
    record(Tables, b"STYLE", 8, 3, vec![s(2, b"Standard")]);
    record(Tables, b"DIMSTYLE", 9, 4, vec![s(2, b"D"), h(340, 8)]);
    for (id, name, flags) in [
        (10, b"*Model_Space".as_slice(), 0),
        (11, b"*Paper_Space", 0),
        (12, b"*D1", 1),
    ] {
        record(Tables, b"BLOCK_RECORD", id, 5, vec![s(2, name)]);
        record(
            Blocks,
            b"BLOCK",
            id + 10,
            id,
            vec![s(8, b"0"), i(70, flags), r(10, 0.0), r(20, 0.0)],
        );
        record(Blocks, b"ENDBLK", id + 20, id, vec![s(8, b"0")]);
    }
    record(
        Blocks,
        b"POINT",
        40,
        12,
        vec![s(8, b"0"), r(10, 2.0), r(20, 1.0)],
    );
    record(
        Entities,
        b"ARC_DIMENSION",
        41,
        10,
        vec![
            s(8, b"0"),
            r(10, 1.0),
            r(20, 0.0),
            r(11, 0.0),
            r(21, 1.0),
            i(70, 37),
            i(71, 5),
            r(13, 1.0),
            r(23, 0.0),
            r(14, 0.0),
            r(24, 1.0),
            r(15, 0.0),
            r(25, 0.0),
            i(70, 0),
            r(40, 0.0),
            r(41, 1.0),
            i(71, 0),
            s(2, b"*D1"),
            s(3, b"D"),
        ],
    );
    record(
        Objects,
        b"DICTIONARY",
        50,
        0,
        vec![s(3, b"ACAD_LAYOUT"), h(350, 51), s(3, b"extra"), h(350, 54)],
    );
    record(
        Objects,
        b"DICTIONARY",
        51,
        50,
        vec![s(3, b"Model"), h(350, 52), s(3, b"Layout1"), h(350, 53)],
    );
    for (id, name, block) in [(52, b"Model".as_slice(), 10), (53, b"Layout1", 11)] {
        record(
            Objects,
            b"LAYOUT",
            id,
            51,
            vec![s(1, b""), s(1, name), h(330, block)],
        );
    }
    record(Objects, b"XRECORD", 54, 50, vec![]);
    db
}

#[test]
fn reports_each_omitted_record_and_dictionary_entry_at_its_source() {
    let db = fixture();
    let mut budget = Budget::new(Limits::default());
    let mut diagnostics = Diagnostics::new();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
    let out = db.reencode_r2004_dxf(&mut cx).unwrap().unwrap();
    assert_eq!(out.elements, 2);
    let read = Dxf::read(&out.bytes, &mut cx)
        .unwrap()
        .objects(&mut cx)
        .unwrap();
    assert_eq!(read.get(41).unwrap().first(2), Some(&s(2, b"*D1").data));
    assert_eq!(read.get(40).unwrap().owner(), Some(12));
    assert_eq!(read.get(54), None);
    assert!(!read.get(50).unwrap().values.contains(&h(350, 54)));
    let lost = diagnostics
        .iter()
        .find(|d| d.code() == codes::DWG_DXF_RECORD_DROPPED)
        .unwrap();
    assert_eq!(lost.count(), 1);
    assert_eq!(lost.param("kind"), Some(&"XRECORD".into()));
    assert_eq!(lost.places(), &[Ref::at_bytes(864, 872)]);
    let reference = diagnostics
        .iter()
        .find(|d| d.code() == codes::DWG_DXF_REFERENCE_DROPPED)
        .unwrap();
    assert_eq!(reference.count(), 1);
    assert_eq!(reference.param("code"), Some(&350_i64.into()));
    assert_eq!(reference.places(), &[Ref::at_bytes(800, 808)]);
}

#[cfg(feature = "cp932")]
#[test]
fn cp932_names_keep_trail_bytes_and_fold_ascii_references() {
    let mut db = fixture();
    // アとヂの後続バイトは A と a、ソの後続バイトはバックスラッシュと同じ値。
    for (id, name, entity, reference) in [
        (60, b"A\x83\x41\x83\x5c", 40, b"a\x83\x41\x83\x5c"),
        (61, b"A\x83\x61\x83\x5c", 41, b"a\x83\x61\x83\x5c"),
    ] {
        let mut layer = db.get(7).unwrap().clone();
        layer.values[0] = h(5, id);
        layer.values[2] = s(2, name);
        db.push(layer);
        let entity = db.object_mut(db.position(entity).unwrap()).unwrap();
        *entity.values.iter_mut().find(|v| v.code == 8).unwrap() = s(8, reference);
    }
    let mut budget = Budget::new(Limits::default());
    let mut diagnostics = Diagnostics::new();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
    let out = db.reencode_r2004_dxf(&mut cx).unwrap().unwrap();
    let read = Dxf::read(&out.bytes, &mut cx)
        .unwrap()
        .objects(&mut cx)
        .unwrap();
    for (id, code) in [(60, 2), (61, 2), (40, 8), (41, 8)] {
        assert_eq!(
            read.get(id).unwrap().first(code),
            db.get(id).unwrap().first(code)
        );
    }
}

#[test]
fn symbol_names_use_the_declared_code_page_and_character_count() {
    let so = b"\x83\x5c";
    assert_eq!(
        valid_name(Charset::ShiftJis, b"LAYER", &so.repeat(255)),
        cfg!(feature = "cp932")
    );
    assert!(!valid_name(Charset::ShiftJis, b"LAYER", &so.repeat(256)));
    assert!(!valid_name(Charset::Other, b"LAYER", so));
    assert!(valid_name(Charset::Other, b"LAYER", b"Walls"));
}

#[test]
fn unrepresentable_drawings_discard_the_trial_output_and_diagnostics() {
    for (id, value) in [
        (9, h(340, 7)),
        (41, s(3, b"missing")),
        (41, r(14, f64::NAN)),
        (7, s(2, b"bad/name")),
        (7, s(2, b"\x83")),
        (7, s(2, b"\x87\x90")), // ≒ の別の符号。元のバイト列に戻らない。
        (40, s(8, b"\x83\x41")),
        (40, s(8, b"0\n0")),
    ] {
        let mut db = fixture();
        let record = db.object_mut(db.position(id).unwrap()).unwrap();
        *record
            .values
            .iter_mut()
            .find(|v| v.code == value.code)
            .unwrap() = value.clone();
        let mut budget = Budget::new(Limits::default());
        let mut diagnostics = Diagnostics::new();
        let cancel = CancelToken::new();
        assert!(
            db.reencode_r2004_dxf(&mut Cx::new(&mut budget, &mut diagnostics, &cancel))
                .unwrap()
                .is_none()
        );
        assert_eq!(diagnostics.iter().count(), 0);
    }
}

#[test]
fn names_beside_layer_zero_check_the_round_trip_and_character_count() {
    let so = b"\x83\x5c";
    let mut so_255 = so.repeat(253);
    so_255.extend_from_slice(b"ab");
    // 256 文字・510 バイト。バイト数の上限には収まる。
    let mut so_256 = so.repeat(254);
    so_256.extend_from_slice(b"ab");
    for (name, valid) in [
        (b"\x87\x90".to_vec(), false), // ≒ の別の符号。元のバイト列に戻らない。
        (b"\x83".to_vec(), false),
        (b"a".repeat(255), true),
        (b"a".repeat(256), false),
        (so_255, cfg!(feature = "cp932")),
        (so_256, false),
    ] {
        // 画層 0 を残し、別のハンドルの画層を足す。
        let mut db = fixture();
        let mut layer = db.get(7).unwrap().clone();
        layer.values[0] = h(5, 60);
        layer.values[2] = s(2, &name);
        db.push(layer);
        let mut budget = Budget::new(Limits::default());
        let mut diagnostics = Diagnostics::new();
        let cancel = CancelToken::new();
        let out = db
            .reencode_r2004_dxf(&mut Cx::new(&mut budget, &mut diagnostics, &cancel))
            .unwrap();
        assert_eq!(out.is_some(), valid, "{name:?}");
        if !valid {
            assert_eq!(diagnostics.iter().count(), 0);
        }
    }
}

#[test]
fn cancellation_and_output_budget_stop_the_conversion() {
    let db = fixture();
    for cancelled in [true, false] {
        let cancel = CancelToken::new();
        if cancelled {
            cancel.cancel();
        }
        let mut budget = Budget::new(Limits::default().with(Kind::DecodedBytes, Limit::Finite(8)));
        let mut diagnostics = Diagnostics::new();
        let out = db.reencode_r2004_dxf(&mut Cx::new(&mut budget, &mut diagnostics, &cancel));
        assert!(matches!(out, Err(Error::Stopped(_))));
        let stop = diagnostics.iter().next().unwrap();
        assert_eq!(
            stop.code(),
            if cancelled {
                codes::CALL_REQUEST_CANCELLED
            } else {
                codes::BUDGET_DECODED_BYTES_EXCEEDED
            }
        );
    }
}

#[test]
fn a_dimension_mixed_with_an_unsupported_entity_uses_the_domain_model() {
    let mut db = fixture();
    db.push(Object {
        kind: b"SPLINE".to_vec(),
        section: Section::Entities,
        values: vec![h(5, 60), h(330, 10), s(8, b"0")],
        origin: Origin::Dwg {
            offset: 960,
            end: 968,
        },
    });
    let mut budget = Budget::new(Limits::default());
    let mut diagnostics = Diagnostics::new();
    let cancel = CancelToken::new();
    assert!(
        db.reencode_r2004_dxf(&mut Cx::new(&mut budget, &mut diagnostics, &cancel))
            .unwrap()
            .is_none()
    );
    assert_eq!(diagnostics.iter().count(), 0);
}

#[test]
fn writes_owned_order_and_reports_only_reactors_whose_target_was_omitted() {
    use crate::dwg::reencode::Extra;
    let mut db = fixture();
    let mut point = db.get(40).unwrap().clone();
    point.values[0] = h(5, 60);
    db.push(point);
    let mut input = fixture_source();
    input.order.insert(12, vec![60, 40]);
    input.extras.insert(
        41,
        Extra {
            reactors: vec![9, 54],
            picture: vec![0x42; 300],
            plotstyle: false,
        },
    );
    let mut budget = Budget::new(Limits::default());
    let mut diagnostics = Diagnostics::new();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
    let out = try_reencode(&db, 0, &mut cx, |_, _| Ok(input))
        .unwrap()
        .unwrap();
    let read = Dxf::read(&out.bytes, &mut cx)
        .unwrap()
        .objects(&mut cx)
        .unwrap();
    let points: Vec<_> = read
        .objects()
        .iter()
        .filter(|o| o.kind == b"POINT")
        .filter_map(Object::handle)
        .collect();
    assert_eq!(points, [60, 40]);
    let dim = read.get(41).unwrap();
    assert_eq!(dim.owner(), Some(10));
    assert!(dim.values.contains(&h(330, 9)));
    assert!(!dim.values.contains(&h(330, 54)));
    assert_eq!(dim.first(92), Some(&Data::Int(300)));
    let graphics: Vec<_> = dim
        .values
        .iter()
        .filter_map(|v| match &v.data {
            Data::Binary(chunk) if v.code == 310 => Some(chunk.len()),
            _ => None,
        })
        .collect();
    assert_eq!(graphics, [127, 127, 46]);
    let lost = diagnostics
        .iter()
        .find(|d| {
            d.code() == codes::DWG_DXF_REFERENCE_DROPPED && d.param("code") == Some(&330_i64.into())
        })
        .unwrap();
    assert_eq!(lost.count(), 1);
    assert_eq!(lost.places(), &[Ref::at_bytes(656, 664)]);
}

// 項目のない表は、書き手の入力が表の名前を示した表のオブジェクトだけから書く（#879）。
#[test]
fn writes_tables_without_entries_from_the_control_objects_named_by_the_source() {
    let table = |id: u64| Object {
        kind: b"TABLE".to_vec(),
        section: Section::Tables,
        values: vec![h(5, id), h(330, 0)],
        origin: Origin::Dwg {
            offset: id as usize * 16,
            end: id as usize * 16 + 8,
        },
    };
    let mut db = fixture();
    for id in 60..=64 {
        db.push(table(id));
    }
    let run = |controls: &[(&'static [u8], u64)]| {
        let mut input = fixture_source();
        input.controls = controls.iter().copied().collect();
        let mut budget = Budget::new(Limits::default());
        let mut diagnostics = Diagnostics::new();
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
        let out = try_reencode(&db, 1000, &mut cx, |_, _| Ok(input)).unwrap();
        let read = out.as_ref().map(|out| {
            Dxf::read(&out.bytes, &mut cx)
                .unwrap()
                .objects(&mut cx)
                .unwrap()
        });
        (out, read, diagnostics.into_vec())
    };
    let named = [
        (b"VPORT".as_slice(), 60),
        (b"VIEW", 61),
        (b"UCS", 62),
        (b"APPID", 63),
        // 項目を持つ表は、項目の所有者を表のオブジェクトにする
        (b"LAYER", 2),
    ];
    let (out, read, diagnostics) = run(&named);
    let (out, read) = (out.unwrap(), read.unwrap());
    let tables: Vec<_> = read
        .objects()
        .iter()
        .filter(|o| o.kind == b"TABLE")
        .map(|o| (o.first(2).cloned(), o.handle(), o.first(70).cloned()))
        .collect();
    let expected = [
        (b"VPORT".as_slice(), 60, 0),
        (b"LTYPE", 1, 1),
        (b"LAYER", 2, 1),
        (b"STYLE", 3, 1),
        (b"VIEW", 61, 0),
        (b"UCS", 62, 0),
        (b"APPID", 63, 0),
        (b"DIMSTYLE", 4, 1),
        (b"BLOCK_RECORD", 5, 3),
    ]
    .map(|(name, id, count)| (Some(s(2, name).data), Some(id), Some(Data::Int(count))));
    assert_eq!(tables, expected);
    // 書いた表のオブジェクトは値と参照を保ったものに数え、名前を示されなかった 64 だけを落とす
    let fidelity = |id| {
        let detail = out
            .report
            .details
            .iter()
            .find(|d| d.source.key() == Some(zumen_base::Key::Handle(id)))
            .unwrap();
        (detail.kind.as_str(), detail.fidelity)
    };
    for id in [1, 60, 61, 62, 63] {
        assert_eq!(fidelity(id), ("TABLE", ConversionFidelity::Native));
    }
    assert_eq!(fidelity(64), ("TABLE", ConversionFidelity::Dropped));
    let lost: Vec<_> = diagnostics
        .iter()
        .filter(|d| {
            d.code() == codes::DWG_DXF_RECORD_DROPPED && d.param("kind") == Some(&"TABLE".into())
        })
        .collect();
    let [lost] = lost[..] else {
        panic!("落とした表のオブジェクトの診断が 1 つでない")
    };
    assert_eq!(lost.places(), &[Ref::at_bytes(1024, 1032)]);

    // 名前を示されない表のオブジェクトは、これまでどおり書かない
    let (_, read, _) = run(&[]);
    let names: Vec<_> = read
        .unwrap()
        .objects()
        .iter()
        .filter(|o| o.kind == b"TABLE")
        .map(|o| o.first(2).cloned())
        .collect();
    let expected = [
        b"LTYPE".as_slice(),
        b"LAYER",
        b"STYLE",
        b"DIMSTYLE",
        b"BLOCK_RECORD",
    ]
    .map(|name| Some(s(2, name).data));
    assert_eq!(names, expected);

    // 項目を持つ表と同じ表のオブジェクト・表でないもの・ないものを指す対応は、図面モデルの経路に渡す
    for id in [1, 6, 99] {
        let (out, _, diagnostics) = run(&[(b"VIEW", id)]);
        assert!(out.is_none());
        assert!(diagnostics.is_empty());
    }
}

#[test]
fn writes_a_dimension_in_an_inactive_layout_in_its_block() {
    let mut db = fixture();
    // 既存の用紙に加えてもう一つの BLOCK_RECORD と LAYOUT を作り、寸法をその用紙へ移す。
    for (from, id, owner) in [(11, 61, 5), (21, 62, 61), (31, 63, 61), (53, 64, 51)] {
        let mut obj = db.get(from).unwrap().clone();
        obj.values[0] = h(5, id);
        obj.values[1] = h(330, owner);
        if id == 61 {
            obj.values[2] = s(2, b"*Paper_Space0");
        }
        if id == 64 {
            obj.values[3] = s(1, b"Layout2");
            obj.values[4] = h(330, 61);
        }
        db.push(obj);
    }
    let dict = db.object_mut(db.position(51).unwrap()).unwrap();
    dict.values.extend([s(3, b"Layout2"), h(350, 64)]);
    let dim = db.object_mut(db.position(41).unwrap()).unwrap();
    dim.values[1] = h(330, 61);
    dim.section = Section::Blocks;
    let mut input = fixture_source();
    input.order.insert(10, vec![]);
    input.order.insert(61, vec![41]);
    let mut budget = Budget::new(Limits::default());
    let mut diagnostics = Diagnostics::new();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
    let out = try_reencode(&db, 0, &mut cx, |_, _| Ok(input))
        .unwrap()
        .unwrap();
    let read = Dxf::read(&out.bytes, &mut cx)
        .unwrap()
        .objects(&mut cx)
        .unwrap();
    assert_eq!(read.get(41).unwrap().section, Section::Blocks);
    assert_eq!(read.get(41).unwrap().owner(), Some(61));
    assert_eq!(read.get(61).unwrap().first(340), Some(&Data::Handle(64)));
}

#[test]
fn omits_dictionary_entries_that_would_create_an_ownership_cycle() {
    let mut db = fixture();
    db.object_mut(db.position(50).unwrap())
        .unwrap()
        .values
        .extend([s(3, b"self"), h(360, 50)]);
    let mut budget = Budget::new(Limits::default());
    let mut diagnostics = Diagnostics::new();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
    let out = db.reencode_r2004_dxf(&mut cx).unwrap().unwrap();
    let read = Dxf::read(&out.bytes, &mut cx)
        .unwrap()
        .objects(&mut cx)
        .unwrap();
    assert!(!read.get(50).unwrap().values.contains(&h(360, 50)));
    let lost = diagnostics
        .iter()
        .find(|d| {
            d.code() == codes::DWG_DXF_REFERENCE_DROPPED && d.param("code") == Some(&360_i64.into())
        })
        .unwrap();
    assert_eq!(lost.places(), &[Ref::at_bytes(800, 808)]);
}
