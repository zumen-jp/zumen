use super::*;
use crate::Dxf;
use zumen_base::{Budget, CancelToken, Diagnostics, Limit, Limits};
use zumen_sdk::Codec;

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

#[test]
fn unrepresentable_drawings_discard_the_trial_output_and_diagnostics() {
    for (id, value) in [
        (9, h(340, 7)),
        (41, s(3, b"missing")),
        (41, r(14, f64::NAN)),
        (7, s(2, b"bad/name")),
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
