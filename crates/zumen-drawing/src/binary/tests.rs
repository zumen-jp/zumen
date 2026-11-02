use zumen_base::{Budget, CancelToken, Diagnostics, Limit, Limits};

use super::*;

type Loaded = Result<(Source, Drawing2D), Error>;

/// 読んだ結果と、足した診断の code
fn load_with(bytes: &[u8], limits: Limits) -> (Loaded, Vec<&'static str>) {
    let mut budget = Budget::new(limits);
    let mut diagnostics = Diagnostics::new();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
    let result = read(bytes, &mut cx);
    let codes = diagnostics.iter().map(|d| d.code().name()).collect();
    (result, codes)
}

fn load(bytes: &[u8]) -> (Loaded, Vec<&'static str>) {
    load_with(bytes, Limits::default())
}

/// 1 つの値を読む
fn decode<T: Unpack>(bytes: &[u8]) -> Result<T, Bad> {
    decode_with(bytes, Limits::default())
}

fn decode_with<T: Unpack>(bytes: &[u8], limits: Limits) -> Result<T, Bad> {
    let mut budget = Budget::new(limits);
    let mut diagnostics = Diagnostics::new();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
    let mut reader = Reader {
        bytes,
        at: 0,
        end: bytes.len(),
        minor: MINOR,
        elements: 0,
        limit: u64::MAX,
        cx: &mut cx,
    };
    let value = T::unpack(&mut reader)?;
    reader.finish()?;
    Ok(value)
}

/// 予算の既定の上限で `pack` が書いたものと、書いた要素の数
fn packed(pack: impl FnOnce(&mut Writer)) -> (Vec<u8>, u64) {
    let mut budget = Budget::default();
    let mut diagnostics = Diagnostics::new();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
    let mut out = Writer {
        bytes: Vec::new(),
        elements: 0,
        cx: &mut cx,
        stop: None,
    };
    pack(&mut out);
    (out.bytes, out.elements)
}

fn encode<T: Pack>(value: &T) -> Vec<u8> {
    packed(|out| value.pack(out)).0
}

/// 予算の既定の上限で書いたバイナリ
fn written(source: &Source, drawing: &Drawing2D) -> Vec<u8> {
    let mut budget = Budget::default();
    let mut diagnostics = Diagnostics::new();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
    write(source, drawing, &mut cx).unwrap()
}

fn source() -> Source {
    Source {
        format: "dxf".to_owned(),
        family: "acad".to_owned(),
        hash: Hash([7; 16]),
    }
}

/// 要素を持つ乱数の図面
fn with_elements() -> (Source, Drawing2D, Vec<u8>) {
    (0..)
        .map(|seed| random(seed, true))
        .map(|(source, drawing)| {
            let bytes = written(&source, &drawing);
            (source, drawing, bytes)
        })
        .find(|(_, _, bytes)| u32_at(bytes, 8).unwrap() > 0)
        .unwrap()
}

#[test]
fn reads_back_what_it_wrote() {
    let mut elements = 0;
    for seed in 0..300 {
        let (source, drawing) = random(seed, true);
        let bytes = written(&source, &drawing);
        elements += u32_at(&bytes, 8).unwrap();
        let (result, codes) = load(&bytes);
        assert_eq!(result, Ok((source, drawing)), "seed {seed}");
        assert!(codes.is_empty(), "seed {seed}: {codes:?}");
    }
    assert!(elements > 0);
}

#[test]
fn writes_the_same_bytes_again_even_with_nan_and_negative_zero() {
    for seed in 0..300 {
        let (source, drawing) = random(seed, false);
        let bytes = written(&source, &drawing);
        let (source, drawing) = load(&bytes).0.unwrap();
        assert_eq!(written(&source, &drawing), bytes, "seed {seed}");
    }
}

#[test]
fn lays_out_the_header_and_sections_like_the_scene() {
    let (source, drawing, bytes) = with_elements();
    assert!(matches(&bytes));
    assert!(!matches(b"ZMSC"));
    assert_eq!(&bytes[..4], b"ZMDR");
    assert_eq!(u16_at(&bytes, 4), Some(MAJOR));
    assert_eq!(u16_at(&bytes, 6), Some(MINOR));
    let (section, elements) = packed(|out| drawing.pack(out));
    assert_eq!(u32_at(&bytes, 8), Some(elements as usize));
    assert_eq!(u32_at(&bytes, 12), Some(SECTIONS));
    // 区画は表のすぐ後ろから、8 バイトの境界に並ぶ
    let (first, first_len) = (u32_at(&bytes, 16).unwrap(), u32_at(&bytes, 20).unwrap());
    let (second, second_len) = (u32_at(&bytes, 24).unwrap(), u32_at(&bytes, 28).unwrap());
    assert_eq!(first, 32);
    assert_eq!(&bytes[first..first + first_len], encode(&source));
    assert_eq!(second, aligned(first + first_len));
    assert_eq!(&bytes[second..second + second_len], section);
    assert_eq!(second + second_len, bytes.len());
}

#[test]
fn refuses_every_cut() {
    let (_, _, bytes) = with_elements();
    for len in 0..bytes.len() {
        let (result, codes) = load(&bytes[..len]);
        assert_eq!(result, Err(Error::Invalid), "len {len}");
        assert_eq!(codes, ["drawing.binary.invalid"], "len {len}");
    }
}

#[test]
fn survives_broken_bytes() {
    let mut state = 0x1234_5678_u64;
    for seed in 0..40 {
        let (source, drawing) = random(seed, false);
        let bytes = written(&source, &drawing);
        for _ in 0..200 {
            let mut broken = bytes.clone();
            for _ in 0..1 + state % 3 {
                state = state
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1_442_695_040_888_963_407);
                let at = (state >> 33) as usize % broken.len();
                broken[at] ^= (state >> 17) as u8 | 1;
            }
            // 壊れたバイト列を読めたときは、書き直したものを、同じ値として読める
            if let (Ok((source, drawing)), _) = load(&broken) {
                let again = written(&source, &drawing);
                let (source, drawing) = load(&again).0.unwrap();
                assert_eq!(written(&source, &drawing), again);
            }
        }
    }
}

/// 元の文書の区画。`extra` は、後の小版で足した欄のつもり
fn source_section(extra: &[u8]) -> Vec<u8> {
    packed(|out| {
        out.record(|out| {
            out.text("dxf");
            out.text("acad");
            Hash([7; 16]).pack(out);
            out.bytes.extend_from_slice(extra);
        });
    })
    .0
}

#[test]
fn skips_fields_and_sections_of_later_minor_versions_only() {
    let empty = encode(&Drawing2D::default());
    let later = MINOR + 1;

    let bytes = assemble(later, 0, &[source_section(&[1, 2]), empty.clone()]);
    let (result, codes) = load(&bytes);
    assert_eq!(result, Ok((source(), Drawing2D::default())));
    assert_eq!(codes, ["drawing.binary-field.dropped"]);
    // 後の小版の書き手は、読み飛ばす欄の要素も見出しに数える
    let bytes = assemble(later, 3, &[source_section(&[1, 2]), empty.clone()]);
    let (result, codes) = load(&bytes);
    assert_eq!(result, Ok((source(), Drawing2D::default())));
    assert_eq!(codes, ["drawing.binary-field.dropped"]);
    let (result, codes) = load(&assemble(
        MINOR,
        0,
        &[source_section(&[1, 2]), empty.clone()],
    ));
    assert_eq!(result, Err(Error::Invalid));
    assert_eq!(codes, ["drawing.binary.invalid"]);

    let sections = [source_section(&[]), empty.clone(), vec![9; 5]];
    let (result, codes) = load(&assemble(later, 0, &sections));
    assert_eq!(result, Ok((source(), Drawing2D::default())));
    assert_eq!(codes, ["drawing.binary-section.dropped"]);
    let (result, codes) = load(&assemble(MINOR, 0, &sections));
    assert_eq!(result, Err(Error::Invalid));
    assert_eq!(codes, ["drawing.binary.invalid"]);

    // 区画が足りない・重なる・境界にないものは、壊れている
    let (result, _) = load(&assemble(MINOR, 0, &[source_section(&[])]));
    assert_eq!(result, Err(Error::Invalid));
    let mut bytes = assemble(MINOR, 0, &[source_section(&[]), empty]);
    bytes[24..28].copy_from_slice(&20u32.to_le_bytes());
    assert_eq!(load(&bytes).0, Err(Error::Invalid));
    bytes[24..28].copy_from_slice(&33u32.to_le_bytes());
    assert_eq!(load(&bytes).0, Err(Error::Invalid));
}

#[test]
fn refuses_other_major_versions_and_unknown_values() {
    let mut bytes = written(&source(), &Drawing2D::default());
    bytes[4..6].copy_from_slice(&(MAJOR + 1).to_le_bytes());
    let (result, codes) = load(&bytes);
    assert_eq!(result, Err(Error::Unsupported));
    assert_eq!(codes, ["drawing.binary-version.unsupported"]);

    assert_eq!(decode::<Key>(&[1, 5]), Ok(Key::Instance(5)));
    assert_eq!(
        decode::<Key>(&[2, 5]),
        Err(Bad::Variant {
            ty: "Key",
            value: 2
        })
    );
}

#[test]
fn reads_only_the_shortest_form_of_each_value() {
    let bad = |at, reason| Bad::Invalid { at, reason };
    assert_eq!(decode::<u64>(&[0x80, 0x00]), Err(bad(0, "integer")));
    assert_eq!(decode::<u64>(&[0x00, 0x00]), Err(bad(1, "trailing")));
    let max = [0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x01];
    assert_eq!(decode::<u64>(&max), Ok(u64::MAX));
    let mut over = max;
    over[9] = 0x02;
    assert_eq!(decode::<u64>(&over), Err(bad(0, "integer")));
    assert_eq!(decode::<u16>(&[0x80, 0x80, 0x04]), Err(bad(0, "range")));
    for value in [0, 1, -1, i64::MIN, i64::MAX] {
        assert_eq!(decode::<i64>(&encode(&value)), Ok(value));
    }
    assert_eq!(encode(&-1i64), [1]);
    assert_eq!(decode::<i16>(&encode(&40_000i64)), Err(bad(0, "range")));
    assert_eq!(decode::<bool>(&[2]), Err(bad(0, "flag")));
    assert_eq!(decode::<Option<u8>>(&[2, 0]), Err(bad(0, "flag")));
    assert_eq!(decode::<char>(&encode(&0xd800u32)), Err(bad(0, "char")));
    assert_eq!(decode::<String>(&[2, 0xff, 0xfe]), Err(bad(1, "utf-8")));
    assert_eq!(decode::<String>(&[5, b'a']), Err(bad(0, "count")));
    assert_eq!(decode::<Vec<u8>>(&[3, 1, 2]), Err(bad(0, "count")));
    assert_eq!(decode::<f64>(&[0; 7]), Err(bad(0, "truncated")));
    assert_eq!(
        decode::<Cow<'static, str>>(&encode(&"図面".to_owned())),
        Ok(Cow::from("図面"))
    );
}

#[test]
fn checks_the_element_count_and_the_budget() {
    let (_, _, bytes) = with_elements();
    let count = u32_at(&bytes, 8).unwrap() as u32;
    for wrong in [count - 1, count + 1] {
        let mut broken = bytes.clone();
        broken[8..12].copy_from_slice(&wrong.to_le_bytes());
        assert_eq!(load(&broken).0, Err(Error::Invalid), "{wrong}");
    }

    let limits = Limits::default().with(Kind::Elements, Limit::Finite(0));
    let (result, codes) = load_with(&bytes, limits);
    assert!(matches!(result, Err(Error::Stopped(_))), "{result:?}");
    assert_eq!(codes, ["budget.elements.exceeded"]);

    let limits = Limits::default().with(Kind::InputBytes, Limit::Finite(bytes.len() as u64 - 1));
    let (result, codes) = load_with(&bytes, limits);
    assert!(matches!(result, Err(Error::Stopped(_))), "{result:?}");
    assert_eq!(codes, ["budget.input-bytes.exceeded"]);

    // 1 バイトの項目がメモリで大きくなるので、作る前に数える
    let limits = Limits::default().with(Kind::DecodedBytes, Limit::Finite(0));
    let (result, codes) = load_with(&bytes, limits);
    assert!(matches!(result, Err(Error::Stopped(_))), "{result:?}");
    assert_eq!(codes, ["budget.decoded-bytes.exceeded"]);
}

/// 復号で作ったバイト数は、型から決めた大きさで数えるので、ポインターの幅によらず同じ所で止まる
#[test]
fn counts_the_decoded_bytes_by_the_sizes_of_the_types() {
    use crate::{Axes, ModelSpace, SpaceId, Unit};

    assert_eq!(<String as Unpack>::SIZE, 24);
    assert_eq!(<Vec<u8> as Unpack>::SIZE, 24);
    assert_eq!(<Option<f64> as Unpack>::SIZE, 16);
    assert_eq!(<[f64; 2] as Unpack>::SIZE, 16);
    assert_eq!(<Box<[f64; 2]> as Unpack>::SIZE, 8);
    assert_eq!(largest(&[3, 9, 4]), 9);

    // 空の実寸空間を 1 つ持つ図面。使うのは、元の文書の 2 つの文字列の中身と、実寸空間の大きさ
    let drawing = Drawing2D {
        spaces: vec![ModelSpace {
            id: SpaceId(0),
            unit: Unit::Millimeter,
            axes: Axes::EastNorth,
            elements: Vec::new(),
            order: Vec::new(),
        }],
        ..Drawing2D::default()
    };
    let bytes = written(&source(), &drawing);
    let read_with = |limit| {
        let limits = Limits::default().with(Kind::DecodedBytes, Limit::Finite(limit));
        load_with(&bytes, limits)
    };
    let used = ("dxf".len() + "acad".len()) as u64 + ModelSpace::SIZE;
    assert_eq!(
        read_with(used),
        (Ok((source(), drawing.clone())), Vec::new())
    );
    let (result, codes) = read_with(used - 1);
    assert!(matches!(result, Err(Error::Stopped(_))), "{result:?}");
    assert_eq!(codes, ["budget.decoded-bytes.exceeded"]);

    // Box の中身も、作る前に数える
    let limits = Limits::default().with(Kind::DecodedBytes, Limit::Finite(15));
    let value = encode(&Box::new([1.0, 2.0]));
    assert!(matches!(
        decode_with::<Box<[f64; 2]>>(&value, limits),
        Err(Bad::Stopped(_))
    ));
}

/// 文字列は、複製する前に数える
#[test]
fn counts_the_strings_before_copying_them() {
    let long = Source {
        format: "a".repeat(1000),
        ..source()
    };
    let bytes = written(&long, &Drawing2D::default());
    let limits = Limits::default().with(Kind::DecodedBytes, Limit::Finite(999));
    let (result, codes) = load_with(&bytes, limits);
    assert!(matches!(result, Err(Error::Stopped(_))), "{result:?}");
    assert_eq!(codes, ["budget.decoded-bytes.exceeded"]);
    let limits = Limits::default().with(Kind::DecodedBytes, Limit::Finite(1000 + 4));
    assert_eq!(
        load_with(&bytes, limits).0,
        Ok((long, Drawing2D::default()))
    );
}

/// 書くときは、要素ごとに作業量を 1 使う。上限か中断の合図で止め、書きかけのものを捨てる
#[test]
fn stops_writing_at_the_work_budget_and_the_cancel_signal() {
    let (source, drawing, bytes) = with_elements();
    let elements = u32_at(&bytes, 8).unwrap() as u64;
    let write_with = |drawing: &Drawing2D, limit, cancel: &CancelToken| {
        let mut budget = Budget::new(Limits::default().with(Kind::Work, Limit::Finite(limit)));
        let mut diagnostics = Diagnostics::new();
        let mut cx = Cx::new(&mut budget, &mut diagnostics, cancel);
        let result = write(&source, drawing, &mut cx);
        (result, budget.used(Kind::Work))
    };
    let cancel = CancelToken::new();
    assert_eq!(
        write_with(&drawing, elements, &cancel),
        (Ok(bytes), elements)
    );
    let (result, used) = write_with(&drawing, elements - 1, &cancel);
    assert!(matches!(result, Err(Stop::Exceeded(_))), "{result:?}");
    assert_eq!(used, elements - 1);

    // 要素のない図面でも、中断の合図を確かめる
    cancel.cancel();
    assert_eq!(
        write_with(&Drawing2D::default(), 0, &cancel),
        (Err(Stop::Cancelled), 0)
    );
}
