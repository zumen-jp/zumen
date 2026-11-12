use super::*;

fn leader_object(path: &[[f64; 2]], arrow: i64) -> Object {
    let int = |code, value| Value {
        code,
        data: Data::Int(value),
    };
    let real = |code, value| Value {
        code,
        data: Data::Real(value),
    };
    let mut values = vec![
        Value {
            code: 3,
            data: Data::Text(b"Standard".to_vec()),
        },
        int(71, arrow),
        int(72, 0),
        int(73, 3),
        int(74, 1),
        int(75, 0),
        real(40, 1.0),
        real(41, 1.0),
        int(76, i64::try_from(path.len()).unwrap()),
    ];
    for [x, y] in path {
        values.extend([real(10, *x), real(20, *y), real(30, 0.0)]);
    }
    Object {
        kind: b"LEADER".to_vec(),
        section: Section::Entities,
        origin: Origin::New,
        values,
    }
}

fn split(data: &[u8]) -> (&[u8], usize) {
    let mut at = 0;
    let handle_bits = usize::try_from(modular_char(data, &mut at, false).unwrap()).unwrap();
    let data = data.get(at..).unwrap();
    (data, data.len() * 8 - handle_bits)
}

/// R2010 から後の LEADER は、注釈の枠の高さと幅を持たない。AutoCAD 2027 が書いた R2010・R2018 の DWG と
/// 同じ並びのフィールドを、矢印のフラグまで読む（#345）
#[test]
fn r2010_leaders_have_no_annotation_box() {
    let mut w = BitWriter::default();
    w.bit(false);
    w.bs(3);
    w.bs(0);
    w.bl(2);
    // 頂点 2 つ・終わりの点の射影・押し出しの向き・X 軸の向き・鉤の線の点の差・注釈への差
    for v in [
        60.0, 0.0, 0.0, 70.0, 10.0, 0.0, 70.0, 10.0, 0.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0,
        0.0, 0.0, 0.0, 0.0,
    ] {
        w.bd(v);
    }
    w.bit(false);
    w.bit(true);
    // 読む側は矢印のフラグまでを読む
    let end = w.len();
    w.bs(0);
    w.bit(false);
    w.bit(false);
    let data = w.into_bytes();
    for release in [Release::R2010, Release::R2013, Release::R2018] {
        let mut bits = Bits::with_release(&data, release);
        let values = super::super::objects::leader(&mut bits).unwrap();
        assert_eq!(bits.pos(), end, "{release:?}");
        let first = |code| {
            values
                .iter()
                .find(|value| value.code == code)
                .map(|value| value.data.clone())
        };
        assert_eq!(first(71), Some(Data::Int(1)), "{release:?}");
        assert_eq!(first(74), Some(Data::Int(0)), "{release:?}");
        assert_eq!(first(40), None, "{release:?}");
    }
}

/// R2010 から後の DWG に足した LEADER のフィールドと、経路と矢印のフラグ（71）を置き換えたフィールドを、読み直す（決定 0185）。
/// 寸法スタイルに従う矢印（71 がない）は書けない
#[test]
fn r2010_leaders_are_added_and_replaced_with_their_arrow() {
    for release in [Release::R2010, Release::R2013, Release::R2018] {
        let old = leader_object(&[[0.0, 0.0], [5.0, 5.0]], 1);
        let new = NewEntity {
            object: &old,
            handle: 0x40,
            type_code: kinds::LEADER,
            mode: 2,
            owner: 0x1F,
            layer: 0x10,
            ltype: (0, 0),
            pointed: Some(0x11),
            embedded: None,
            children: None,
        };
        let encoded = new.encode(0, 0, release).unwrap();
        let read = |encoded: &[u8]| {
            let (data, handles_at) = split(encoded);
            let [_, start, end] = r2010_entity_fields(data, handles_at, release).unwrap();
            let mut bits = Bits::with_release(data, release);
            bits.seek(start);
            let values = super::super::objects::leader(&mut bits).unwrap();
            // 後ろのフィールド（BS・B・B）を読むと、主のデータの終わりに着く。注釈の枠のフィールドを書くと合わない（#345）
            bits.bs().unwrap();
            bits.bit().unwrap();
            bits.bit().unwrap();
            assert_eq!(bits.pos(), end, "{release:?}");
            bits.seek(handles_at);
            let handles: Vec<u64> = (0..3).map(|_| bits.handle().unwrap().value).collect();
            (values, handles)
        };
        let (values, handles) = read(&encoded);
        let first = |values: &[Value], code| {
            values
                .iter()
                .find(|value| value.code == code)
                .map(|value| value.data.clone())
        };
        assert_eq!(first(&values, 71), Some(Data::Int(1)));
        assert_eq!(first(&values, 73), Some(Data::Int(3)));
        assert_eq!(first(&values, 76), Some(Data::Int(2)));
        // 画層・注記（なし）・寸法スタイル
        assert_eq!(handles, [0x10, 0, 0x11]);

        let changed = leader_object(&[[1.0, 0.0], [5.0, 5.0], [9.0, 5.0]], 0);
        let (data, handles_at) = split(&encoded);
        let rewritten = encode_r2010(
            data,
            handles_at,
            &old,
            &changed,
            release,
            Restyle::default(),
        )
        .unwrap();
        let (values, handles) = read(&rewritten);
        assert_eq!(first(&values, 71), Some(Data::Int(0)));
        assert_eq!(first(&values, 76), Some(Data::Int(3)));
        let xs: Vec<_> = values
            .iter()
            .filter(|value| value.code == 10)
            .map(|value| value.data.clone())
            .collect();
        assert_eq!(xs, [1.0, 5.0, 9.0].map(Data::Real));
        assert_eq!(handles, [0x10, 0, 0x11]);

        let mut following = changed.clone();
        following.values.retain(|value| value.code != 71);
        assert!(
            encode_r2010(
                data,
                handles_at,
                &old,
                &following,
                release,
                Restyle::default()
            )
            .is_none()
        );
    }
}

/// 図形にリアクターを足すと、リアクターの数とハンドルのストリームのリアクターが増え、型ごとのフィールドとほかのハンドルは変わらない
/// （決定 0191）
#[test]
fn reactors_are_added_to_entities() {
    for release in [
        Release::R2000,
        Release::R2004,
        Release::R2010,
        Release::R2013,
        Release::R2018,
    ] {
        let old = leader_object(&[[0.0, 0.0], [5.0, 5.0]], 1);
        let new = NewEntity {
            object: &old,
            handle: 0x40,
            type_code: kinds::LEADER,
            mode: 2,
            owner: 0x1F,
            layer: 0x10,
            ltype: (0, 0),
            pointed: Some(0x11),
            embedded: None,
            children: None,
        };
        let encoded = new.encode(0, 0, release).unwrap();
        // リアクターの数・リアクターのハンドル・型ごとの値・後ろのハンドル
        let read = |bytes: &[u8]| {
            let (data, handles_at, start) = if release >= Release::R2010 {
                let (data, handles_at) = split(bytes);
                let [_, start, _] = r2010_entity_fields(data, handles_at, release).unwrap();
                (data.to_vec(), handles_at, start)
            } else {
                let [_, _, start, handles_at] = entity_fields(bytes, release).unwrap();
                (bytes.to_vec(), handles_at, start)
            };
            let count = if release == Release::R2000 {
                entity_links(&data, 0x40).unwrap().reactors
            } else {
                entity_refs(&data, release).unwrap().0
            };
            let mut bits = Bits::with_release(&data, release);
            bits.seek(start);
            let values = super::super::objects::leader(&mut bits).unwrap();
            bits.seek(handles_at);
            let reactors: Vec<u64> = (0..count).map(|_| bits.handle().unwrap().value).collect();
            let rest: Vec<u64> = (0..3).map(|_| bits.handle().unwrap().value).collect();
            (count, reactors, values, rest)
        };
        let (count, reactors, values, rest) = read(&encoded);
        assert_eq!((count, reactors), (0, Vec::new()));
        let reacted = if release >= Release::R2010 {
            let (data, handles_at) = split(&encoded);
            react(data, Some(handles_at), release, &[0x51, 0x52]).unwrap()
        } else {
            react(&encoded, None, release, &[0x51, 0x52]).unwrap()
        };
        let again = read(&reacted);
        assert_eq!(again, (2, vec![0x51, 0x52], values, rest), "{release:?}");
    }
}
