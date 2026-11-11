use super::*;

/// 文字列とハンドルのストリームを分けた LAYER のデータを、テストの中で作る。
fn layer_data(release: Release, name: &str, external: bool) -> Vec<u8> {
    let mut head = BitWriter::default();
    head.bot(i16::try_from(kinds::LAYER).unwrap());
    head.handle(0, 0x10);
    head.bs(0); // 拡張データなし
    head.bl(0); // リアクターなし
    head.bit(true); // 拡張辞書なし
    if release >= Release::R2013 {
        head.bit(false); // DS なし
    }
    let mut fields = BitWriter::with_strings();
    fields.text(name.as_bytes()).unwrap();
    fields.bit(false); // 参照されるフラグ
    fields.bit(external);
    fields.bs(16 | (31 << 5)); // 印刷し、既定の線幅
    cmc(&mut fields, 7, release).unwrap();
    fields.rc(0xA5); // 型の残りはそのまま写す
    let mut handles = BitWriter::default();
    handles.handle(4, 2); // 所有者
    handles.handle(5, 0x42); // 型ごとの参照
    r2010_assemble(head, fields, handles).unwrap()
}

fn split(data: &[u8]) -> (&[u8], usize) {
    let mut at = 0;
    let handle_bits = usize::try_from(modular_char(data, &mut at, false).unwrap()).unwrap();
    let data = data.get(at..).unwrap();
    (data, data.len() * 8 - handle_bits)
}

fn layer_object(name: &str, external: bool) -> Object {
    Object {
        kind: b"LAYER".to_vec(),
        section: Section::Tables,
        origin: Origin::New,
        values: vec![
            Value {
                code: 2,
                data: Data::Text(name.as_bytes().to_vec()),
            },
            Value {
                code: 70,
                data: Data::Int(if external { 16 } else { 0 }),
            },
            Value {
                code: 62,
                data: Data::Int(7),
            },
            Value {
                code: 290,
                data: Data::Bool(true),
            },
            Value {
                code: 370,
                data: Data::Int(-3),
            },
        ],
    }
}

/// 文字列の長さが変わっても、名前・色・後ろのフィールド・ハンドルを読み直せる。
#[test]
fn r2010_layer_names_are_rewritten_in_the_string_stream() {
    for release in [Release::R2010, Release::R2013, Release::R2018] {
        for (old_name, new_name) in [
            ("A", "図面🧱"),
            ("長いレイヤ名", "B"),
            ("Walls", "Doors"),
            ("Walls", "Walls"),
        ] {
            let encoded = layer_data(release, old_name, false);
            let (data, handles_at) = split(&encoded);
            let old = layer_object(old_name, false);
            let mut new = layer_object(new_name, false);
            new.values
                .iter_mut()
                .find(|value| value.code == 62)
                .unwrap()
                .data = Data::Int(3);
            let rewritten =
                encode_record_r2010(data, handles_at, &old, &new, release, None).unwrap();
            let (data, handles_at) = split(&rewritten);
            let mut bits = Bits::with_release(data, release);
            assert_eq!(bits.bot(), i16::try_from(kinds::LAYER).ok());
            let header = bits.pos();
            let fields_end = bits.split_strings(handles_at).unwrap();
            bits.seek(header);
            assert_eq!(bits.handle().unwrap().value, 0x10);
            assert_eq!(bits.bs(), Some(0));
            assert_eq!(bits.bl(), Some(0));
            assert_eq!(bits.bit(), Some(true));
            if release >= Release::R2013 {
                assert_eq!(bits.bit(), Some(false));
            }
            assert_eq!(bits.text().unwrap(), new_name.as_bytes());
            assert_eq!(bits.bit(), Some(false));
            assert_eq!(bits.bit(), Some(false));
            assert_eq!(bits.bs(), Some(16 | (31 << 5)));
            assert_eq!(bits.cmc().unwrap().index, 3);
            assert_eq!(bits.rc(), Some(0xA5));
            assert_eq!(bits.pos(), fields_end);
            bits.seek(handles_at);
            let owner = bits.handle().unwrap();
            let other = bits.handle().unwrap();
            assert_eq!((owner.code, owner.value), (4, 2));
            assert_eq!((other.code, other.value), (5, 0x42));
        }
    }
}

#[test]
fn r2010_external_layer_names_cannot_be_changed() {
    let release = Release::R2010;
    let encoded = layer_data(release, "xref|old", true);
    let (data, handles_at) = split(&encoded);
    let old = layer_object("xref|old", true);
    let new = layer_object("xref|new", true);
    assert!(encode_record_r2010(data, handles_at, &old, &new, release, None).is_none());
}
