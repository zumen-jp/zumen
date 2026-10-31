//! ZIP の L0 の読み取り（決定 0210）。書庫は、どれも試験の中で作る。
//!
//! deflate のデータは、zlib（`compressobj(9, DEFLATED, -15)`）で `TEXT` を圧縮したもの。

use zumen_base::crc::crc32;
use zumen_base::{Budget, CancelToken, Code, Cx, Diagnostics, Kind, Limit, Limits, Value, codes};
use zumen_sdk::Error;
use zumen_zip::{Archive, Method};

/// 動的なハフマン符号のブロック 1 つ
const DYNAMIC: &str = "6dd2b10d83400043d19e293202b609c98dc71c888616290d4d9475a243620b7afeb5bf7ab25ce7fd5cd6fff439e66ffd6d8fbeabb72214a304654079a28c282f94374aa1b081a65a648b6e112eca45ba6817f1a2ded4bbb139f5a6ded49b7a536fea4dbda90ff5a13e8dcb501fea437da80ff5a13ea5bb00";
/// 固定のハフマン符号のブロック 1 つ（`Z_FIXED`）
const FIXED: &str = "7b3a7bf3cbb98b1e37ae7b367bc7d35dab150cb89ea2891862881861881863889860889862889861889863885860885862ba108ba3315d6d88e96c434c771b623adc10d3e586984e37c474bb21a6e30d315d6f84e97a232c618ee97a234cd71b61bade08d3f54698ae37c274bd11a6eb8d305d6f8ce97a634cd71b63493298ae37c674bd31a6eb8d315d6f8ce97a634cd71b5b720100";

fn text() -> Vec<u8> {
    (0..40)
        .map(|i| format!("図面の書庫 {i}\n"))
        .collect::<String>()
        .into_bytes()
}

fn hex(src: &str) -> Vec<u8> {
    (0..src.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(src.get(i..i + 2).unwrap_or_default(), 16).unwrap_or_default())
        .collect()
}

/// 書庫の項目。`offset` を与えると、局所の見出しを書かず、その位置を指す
#[derive(Clone)]
struct Item {
    name: Vec<u8>,
    flags: u16,
    method: u16,
    data: Vec<u8>,
    crc: u32,
    size: u32,
    offset: Option<u32>,
}

fn stored(name: &str, data: &[u8]) -> Item {
    Item {
        name: name.as_bytes().to_vec(),
        flags: 1 << 11,
        method: 0,
        data: data.to_vec(),
        crc: crc32(data),
        size: data.len() as u32,
        offset: None,
    }
}

fn deflated(name: &str, compressed: &str) -> Item {
    let text = text();
    Item {
        method: 8,
        data: hex(compressed),
        crc: crc32(&text),
        size: text.len() as u32,
        ..stored(name, b"")
    }
}

fn push16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn push32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn push64(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_le_bytes());
}

/// 書庫を組み立てる。`zip64` なら、中央ディレクトリの値を ZIP64 の欄と記録に書く
fn archive(items: &[Item], zip64: bool) -> Vec<u8> {
    let mut out = b"prefix".to_vec();
    let mut offsets = Vec::new();
    for item in items {
        if let Some(offset) = item.offset {
            offsets.push(offset);
            continue;
        }
        offsets.push(out.len() as u32);
        push32(&mut out, 0x0403_4b50);
        push16(&mut out, 20);
        push16(&mut out, item.flags);
        push16(&mut out, item.method);
        push32(&mut out, 0);
        push32(&mut out, item.crc);
        push32(&mut out, item.data.len() as u32);
        push32(&mut out, item.size);
        push16(&mut out, item.name.len() as u16);
        push16(&mut out, 0);
        out.extend_from_slice(&item.name);
        out.extend_from_slice(&item.data);
    }
    let start = out.len();
    for (item, offset) in items.iter().zip(offsets) {
        let wide = |value: u32| if zip64 { u32::MAX } else { value };
        push32(&mut out, 0x0201_4b50);
        push16(&mut out, 45);
        push16(&mut out, 20);
        push16(&mut out, item.flags);
        push16(&mut out, item.method);
        push32(&mut out, 0);
        push32(&mut out, item.crc);
        push32(&mut out, wide(item.data.len() as u32));
        push32(&mut out, wide(item.size));
        push16(&mut out, item.name.len() as u16);
        push16(&mut out, if zip64 { 28 } else { 0 });
        push16(&mut out, 0);
        push16(&mut out, 0);
        push16(&mut out, 0);
        push32(&mut out, 0);
        push32(&mut out, wide(offset));
        out.extend_from_slice(&item.name);
        if zip64 {
            push16(&mut out, 0x0001);
            push16(&mut out, 24);
            push64(&mut out, u64::from(item.size));
            push64(&mut out, item.data.len() as u64);
            push64(&mut out, u64::from(offset));
        }
    }
    let size = out.len() - start;
    let count = items.len() as u16;
    if zip64 {
        let end64 = out.len() as u64;
        push32(&mut out, 0x0606_4b50);
        push64(&mut out, 44);
        push16(&mut out, 45);
        push16(&mut out, 45);
        push32(&mut out, 0);
        push32(&mut out, 0);
        push64(&mut out, items.len() as u64);
        push64(&mut out, items.len() as u64);
        push64(&mut out, size as u64);
        push64(&mut out, start as u64);
        push32(&mut out, 0x0706_4b50);
        push32(&mut out, 0);
        push64(&mut out, end64);
        push32(&mut out, 1);
    }
    push32(&mut out, 0x0605_4b50);
    push16(&mut out, 0);
    push16(&mut out, 0);
    push16(&mut out, if zip64 { u16::MAX } else { count });
    push16(&mut out, if zip64 { u16::MAX } else { count });
    push32(&mut out, if zip64 { u32::MAX } else { size as u32 });
    push32(&mut out, if zip64 { u32::MAX } else { start as u32 });
    push16(&mut out, 7);
    out.extend_from_slice(b"comment");
    out
}

/// 予算と診断を作って `f` を呼ぶ
fn with<T>(limits: Limits, f: impl FnOnce(&mut Cx) -> T) -> (T, Diagnostics) {
    let mut budget = Budget::new(limits);
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let result = f(&mut Cx::new(&mut budget, &mut diag, &cancel));
    (result, diag)
}

fn found(diag: &Diagnostics) -> Vec<Code> {
    diag.iter().map(|d| d.code()).collect()
}

fn names(archive: &Archive) -> Vec<String> {
    archive
        .entries()
        .iter()
        .map(|entry| entry.name().to_owned())
        .collect()
}

#[test]
fn reads_and_extracts_stored_and_deflated_entries() {
    for zip64 in [false, true] {
        let src = archive(
            &[
                stored("dir/", b""),
                stored("dir/plain.txt", b"plain"),
                deflated("dir/dynamic.txt", DYNAMIC),
                deflated("fixed.txt", FIXED),
            ],
            zip64,
        );
        let ((), diag) = with(Limits::default(), |cx| {
            let archive = zumen_zip::read(&src, cx).unwrap();
            assert_eq!(
                names(&archive),
                ["dir/", "dir/plain.txt", "dir/dynamic.txt", "fixed.txt"]
            );
            let dir = archive.find("dir/").unwrap();
            assert!(dir.is_directory());
            assert_eq!(archive.extract(dir, cx).unwrap(), b"");
            let plain = archive.find("dir/plain.txt").unwrap();
            assert_eq!(plain.method(), Method::Stored);
            assert_eq!(&src[plain.data()], b"plain");
            assert_eq!(archive.extract(plain, cx).unwrap(), b"plain");
            for name in ["dir/dynamic.txt", "fixed.txt"] {
                let entry = archive.find(name).unwrap();
                assert_eq!(entry.method(), Method::Deflate);
                assert_eq!(entry.size(), text().len() as u64);
                assert_eq!(archive.extract(entry, cx).unwrap(), text());
            }
        });
        assert!(diag.is_empty(), "{diag:?}");
    }
}

#[test]
fn reads_stored_deflate_blocks() {
    // 圧縮しないブロック（種類 0）を 2 つ。初めのブロックは終わりの印を持たない
    let mut data = vec![0b000, 3, 0, !3, 0xff];
    data.extend_from_slice(b"abc");
    data.extend_from_slice(&[0b001, 2, 0, !2, 0xff]);
    data.extend_from_slice(b"de");
    let item = Item {
        method: 8,
        data,
        crc: crc32(b"abcde"),
        size: 5,
        ..stored("blocks", b"")
    };
    let src = archive(&[item], false);
    let (data, _) = with(Limits::default(), |cx| {
        let archive = zumen_zip::read(&src, cx).unwrap();
        archive.extract(&archive.entries()[0], cx)
    });
    assert_eq!(data.unwrap(), b"abcde");
}

#[test]
fn reads_names_as_utf8_or_shift_jis() {
    let mut sjis = stored("", b"x");
    // 「図面」は Shift_JIS で 0x90 0x7D 0x96 0xCA。UTF-8 の印はない
    sjis.name = vec![0x90, 0x7d, 0x96, 0xca, b'.', b'p', b'2', b'z'];
    sjis.flags = 0;
    let src = archive(&[sjis, stored("図面.sfz", b"y")], false);
    let (archive, diag) = with(Limits::default(), |cx| zumen_zip::read(&src, cx).unwrap());
    assert!(diag.is_empty());
    assert_eq!(names(&archive), ["図面.p2z", "図面.sfz"]);
}

#[test]
fn skips_entries_that_are_unsafe_or_not_read() {
    let mut encrypted = stored("secret", b"x");
    encrypted.flags |= 1;
    let mut bzip2 = stored("bzip2", b"x");
    bzip2.method = 12;
    let mut broken_utf8 = stored("", b"x");
    broken_utf8.name = vec![b'a', 0xff];
    let mut missing = stored("missing", b"x");
    missing.offset = Some(1);
    let src = archive(
        &[
            stored("../up", b"x"),
            stored("/root", b"x"),
            stored("C:/drive", b"x"),
            stored("a\\b", b"x"),
            stored("a//b", b"x"),
            broken_utf8,
            encrypted,
            bzip2,
            missing,
            stored("ok", b"x"),
        ],
        false,
    );
    let (archive, diag) = with(Limits::default(), |cx| zumen_zip::read(&src, cx).unwrap());
    assert_eq!(names(&archive), ["ok"]);
    let unsafe_names = diag
        .iter()
        .find(|d| d.code() == codes::ZIP_PATH_UNSAFE)
        .unwrap();
    assert_eq!(unsafe_names.count(), 6);
    assert_eq!(
        unsafe_names.param("name"),
        Some(&Value::Text("../up".to_owned()))
    );
    let mut rest = found(&diag);
    rest.retain(|code| *code != codes::ZIP_PATH_UNSAFE);
    assert_eq!(
        rest,
        [
            codes::ZIP_ENCRYPTION_UNSUPPORTED,
            codes::ZIP_METHOD_UNSUPPORTED,
            codes::ZIP_ENTRY_INVALID
        ]
    );
    let method = diag
        .iter()
        .find(|d| d.code() == codes::ZIP_METHOD_UNSUPPORTED)
        .unwrap();
    assert_eq!(method.param("method"), Some(&Value::UInt(12)));
}

#[test]
fn entries_sharing_data_are_not_read() {
    // 2 つ目と 3 つ目は、1 つ目の局所の見出しを指す（重なる範囲の書庫の爆弾の形）
    let first = deflated("a.txt", DYNAMIC);
    let mut second = deflated("b.txt", DYNAMIC);
    second.offset = Some(6);
    let mut third = second.clone();
    third.name = b"c.txt".to_vec();
    let src = archive(&[first, stored("own", b"own"), second, third], false);
    let (archive, diag) = with(Limits::default(), |cx| zumen_zip::read(&src, cx).unwrap());
    assert_eq!(names(&archive), ["own"]);
    assert_eq!(found(&diag), [codes::ZIP_RANGE_UNSAFE]);
    assert_eq!(diag.iter().next().unwrap().count(), 3);
}

#[test]
fn keeps_the_first_of_the_same_names() {
    let src = archive(&[stored("same", b"1"), stored("same", b"2")], false);
    let (data, diag) = with(Limits::default(), |cx| {
        let archive = zumen_zip::read(&src, cx).unwrap();
        assert_eq!(archive.entries().len(), 1);
        archive.extract(archive.find("same").unwrap(), cx).unwrap()
    });
    assert_eq!(data, b"1");
    assert_eq!(found(&diag), [codes::ZIP_DUPLICATE_DROPPED]);
}

#[test]
fn broken_data_is_not_extracted() {
    let mut crc = stored("crc", b"data");
    crc.crc ^= 1;
    // 記録した大きさより多く展開されるデータは、上限で止める
    let mut longer = deflated("longer", DYNAMIC);
    longer.size -= 1;
    let mut shorter = deflated("shorter", DYNAMIC);
    shorter.size += 1;
    let mut cut = deflated("cut", DYNAMIC);
    cut.data.truncate(cut.data.len() / 2);
    let mut reserved = stored("reserved", b"");
    reserved.method = 8;
    reserved.data = vec![0b111];
    let mut stored_size = stored("stored-size", b"data");
    stored_size.size = 3;
    let src = archive(&[crc, longer, shorter, cut, reserved, stored_size], false);
    let (archive, diag) = with(Limits::default(), |cx| zumen_zip::read(&src, cx).unwrap());
    // 圧縮しない項目で、大きさが合わないものは見出しが壊れている
    assert_eq!(found(&diag), [codes::ZIP_ENTRY_INVALID]);
    for (name, code) in [
        ("crc", codes::ZIP_CRC_INVALID),
        ("longer", codes::ZIP_DATA_INVALID),
        ("shorter", codes::ZIP_DATA_INVALID),
        ("cut", codes::ZIP_DATA_INVALID),
        ("reserved", codes::ZIP_DATA_INVALID),
    ] {
        let entry = archive.find(name).unwrap();
        let (result, diag) = with(Limits::default(), |cx| archive.extract(entry, cx));
        assert!(matches!(result, Err(Error::Invalid)), "{name}");
        assert_eq!(found(&diag), [code], "{name}");
        let place = diag.iter().next().unwrap().places()[0].bytes().unwrap();
        assert_eq!(place.start as usize, entry.data().start);
    }
}

#[test]
fn broken_archives_are_invalid() {
    let good = archive(&[stored("a", b"x")], false);
    let mut split = good.clone();
    let end = split.len() - 22 - 7;
    split[end + 4] = 1;
    let mut outside = good.clone();
    outside[end + 16] = 0xff;
    let mut many = good.clone();
    many[end + 8] = 9;
    many[end + 10] = 9;
    for (src, code) in [
        (&b""[..], codes::ZIP_ARCHIVE_INVALID),
        (b"PK\x03\x04 no directory", codes::ZIP_ARCHIVE_INVALID),
        (&split, codes::ZIP_SPLIT_UNSUPPORTED),
        (&outside, codes::ZIP_ARCHIVE_INVALID),
        (&many, codes::ZIP_ARCHIVE_INVALID),
    ] {
        let (result, diag) = with(Limits::default(), |cx| zumen_zip::read(src, cx).map(|_| ()));
        assert!(result.is_err());
        assert_eq!(found(&diag), [code]);
    }
}

#[test]
fn budgets_stop_at_entries_bytes_and_nesting() {
    let src = archive(
        &[stored("a", b"x"), stored("b", b"x"), stored("c", b"x")],
        false,
    );
    let (result, diag) = with(
        Limits::default().with(Kind::Records, Limit::Finite(2)),
        |cx| zumen_zip::read(&src, cx).map(|_| ()),
    );
    assert!(matches!(result, Err(Error::Stopped(_))));
    assert_eq!(found(&diag), [codes::BUDGET_RECORDS_EXCEEDED]);

    // 展開した大きさは、記録した値で、展開する前に数える
    let src = archive(&[deflated("big", DYNAMIC)], false);
    let (result, diag) = with(
        Limits::default().with(Kind::DecodedBytes, Limit::Finite(100)),
        |cx| {
            let archive = zumen_zip::read(&src, cx).unwrap();
            archive.extract(&archive.entries()[0], cx).map(|_| ())
        },
    );
    assert!(matches!(result, Err(Error::Stopped(_))));
    assert_eq!(found(&diag), [codes::BUDGET_DECODED_BYTES_EXCEEDED]);

    // 書庫の中の書庫の中の書庫
    let inner = archive(&[stored("leaf", b"leaf")], false);
    let middle = archive(&[stored("inner.zip", &inner)], false);
    let outer = archive(&[stored("middle.zip", &middle)], false);
    let open = |limit: u64| {
        with(
            Limits::default().with(Kind::Depth, Limit::Finite(limit)),
            |cx| {
                let outer = zumen_zip::read(&outer, cx)?;
                let middle = outer.open(outer.find("middle.zip").unwrap(), cx)?;
                let inner = middle.open(middle.find("inner.zip").unwrap(), cx)?;
                inner.extract(inner.find("leaf").unwrap(), cx)
            },
        )
    };
    let (result, diag) = open(1);
    assert!(matches!(result, Err(Error::Stopped(_))));
    assert_eq!(found(&diag), [codes::BUDGET_DEPTH_EXCEEDED]);
    let (result, diag) = open(2);
    assert_eq!(result.unwrap(), b"leaf");
    assert!(diag.is_empty());
}

/// どこで切っても、どのバイトを変えても、パニックせずに読むか断る
#[test]
fn truncated_and_mutated_archives_do_not_panic() {
    let src = archive(
        &[
            stored("dir/plain.txt", b"plain"),
            deflated("dynamic.txt", DYNAMIC),
            deflated("fixed.txt", FIXED),
        ],
        true,
    );
    let try_all = |src: &[u8]| {
        let _ = with(Limits::default(), |cx| {
            if let Ok(archive) = zumen_zip::read(src, cx) {
                for entry in archive.entries() {
                    let _ = archive.extract(entry, cx);
                }
            }
        });
    };
    for end in 0..src.len() {
        try_all(&src[..end]);
    }
    for at in 0..src.len() {
        for byte in [0, 1, 0x7f, 0xff] {
            let mut changed = src.clone();
            changed[at] = byte;
            try_all(&changed);
        }
    }
}

#[test]
fn places_point_at_the_central_directory_header() {
    let src = archive(&[stored("../x", b"x")], false);
    let (_, diag) = with(Limits::default(), |cx| {
        zumen_zip::read(&src, cx).map(|_| ())
    });
    let place = diag.iter().next().unwrap().places()[0].bytes().unwrap();
    assert_eq!(
        &src[place.start as usize..place.start as usize + 4],
        b"PK\x01\x02"
    );
}

#[test]
fn writes_stored_entries_that_read_back_the_same() {
    let entries: [(&str, &[u8]); 3] = [
        ("topic/", b""),
        ("topic/markup.bcf", b"<Markup/>"),
        ("\u{56f3}\u{9762}.txt", b"zumen"),
    ];
    let bytes = zumen_zip::write(&entries).unwrap();
    // 同じ項目からは同じバイト列
    assert_eq!(zumen_zip::write(&entries).unwrap(), bytes);
    let (archive, diag) = with(Limits::default(), |cx| zumen_zip::read(&bytes, cx));
    assert!(diag.is_empty(), "{diag:?}");
    let archive = archive.unwrap();
    let names: Vec<&str> = archive.entries().iter().map(|e| e.name()).collect();
    assert_eq!(
        names,
        ["topic/", "topic/markup.bcf", "\u{56f3}\u{9762}.txt"]
    );
    assert!(archive.entries()[0].is_directory());
    for (name, data) in &entries[1..] {
        let entry = archive.find(name).unwrap();
        assert_eq!(entry.method(), Method::Stored);
        let (extracted, _) = with(Limits::default(), |cx| archive.extract(entry, cx));
        assert_eq!(extracted.unwrap(), *data);
    }
    // 空の書庫も読める
    let empty = zumen_zip::write(&[]).unwrap();
    let (archive, _) = with(Limits::default(), |cx| zumen_zip::read(&empty, cx));
    assert!(archive.unwrap().entries().is_empty());
}
