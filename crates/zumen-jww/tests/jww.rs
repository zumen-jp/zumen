//! JWW の読み書き（決定 0133）。入力は、どれも試験の中で作る。

use std::collections::BTreeMap;

use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits, Ref, Stop, codes};
use zumen_jww::{Jww, JwwFile, RecordKind, Tag};
use zumen_sdk::{Codec, Error, testkit};

/// 知っている欄だけの見出し。メモと名前は空、値は 0
fn header(version: u32) -> Vec<u8> {
    let since = |v: u32, n: usize| if version >= v { n } else { 0 };
    let mut out = b"JwwData.".to_vec();
    out.extend(version.to_le_bytes());
    out.push(0);
    out.extend(vec![0; 8 + 16 * 148 + 156 + 256 + 16]);
    let jumps = if version >= 300 { 224 } else { 96 };
    let display = 28
        + since(300, 16)
        + 4
        + 24
        + 24
        + jumps
        + since(300, 56)
        + 80
        + 8
        + 80
        + 160
        + 128
        + 100
        + 64
        + 44
        + since(223, 60)
        + since(225, 32)
        + since(230, 8);
    out.extend(vec![0; display]);
    if version >= 420 {
        out.extend(vec![0; 257 * 8 + 257 * 17 + 33 * 16 + 33 * 85]);
    }
    out.extend(vec![0; 380]);
    out
}

/// CArchive のオブジェクトを書く。クラスとオブジェクトの番号は 1 から数える
struct Archive {
    version: u32,
    out: Vec<u8>,
    classes: BTreeMap<&'static str, u16>,
    next: u16,
    /// 書いたオブジェクトの始まり
    starts: Vec<usize>,
}

impl Archive {
    fn new(version: u32) -> Self {
        Self {
            version,
            out: Vec::new(),
            classes: BTreeMap::new(),
            next: 1,
            starts: Vec::new(),
        }
    }

    fn count(&mut self, n: u16) {
        self.out.extend(n.to_le_bytes());
    }

    fn tag(&mut self, class: &'static str) {
        self.starts.push(self.out.len());
        if let Some(&number) = self.classes.get(class) {
            self.out.extend((0x8000 | number).to_le_bytes());
        } else {
            self.out.extend([0xFF, 0xFF, 1, 0]);
            self.out.extend((class.len() as u16).to_le_bytes());
            self.out.extend(class.as_bytes());
            self.classes.insert(class, self.next);
            self.next += 1;
        }
        self.next += 1;
    }

    fn null(&mut self) {
        self.starts.push(self.out.len());
        self.out.extend([0, 0]);
    }

    fn base(&mut self, pen_style: u8, pen_color: u16) {
        self.out.extend(7u32.to_le_bytes());
        self.out.push(pen_style);
        self.out.extend(pen_color.to_le_bytes());
        if self.version >= 351 {
            self.out.extend(1u16.to_le_bytes());
        }
        self.out.extend([2, 0, 3, 0, 0, 0]);
    }

    fn reals(&mut self, values: &[f64]) {
        for value in values {
            self.out.extend(value.to_le_bytes());
        }
    }

    fn cstring(&mut self, text: &str) {
        if text.is_ascii() {
            self.out.push(text.len() as u8);
            self.out.extend(text.as_bytes());
        } else {
            let units: Vec<u16> = text.encode_utf16().collect();
            self.out.extend([0xFF, 0xFE, 0xFF, units.len() as u8]);
            for unit in units {
                self.out.extend(unit.to_le_bytes());
            }
        }
    }

    fn line(&mut self) {
        self.tag("CDataSen");
        self.base(1, 1);
        self.reals(&[0.0, 0.0, 10.0, 0.0]);
    }

    fn text_body(&mut self, content: &str) {
        self.reals(&[0.0, 0.0, 5.0, 0.0]);
        self.out.extend(1u32.to_le_bytes());
        self.reals(&[3.0, 3.0, 0.0, 15.0]);
        self.cstring("ＭＳ ゴシック");
        self.cstring(content);
    }

    /// 図形を 1 つずつ。版 700 の寸法は SXF の欄を持つ
    fn all_kinds(&mut self) {
        self.line();
        self.line();
        self.tag("CDataEnko");
        self.base(1, 2);
        self.reals(&[0.0, 0.0, 5.0, 0.0, 1.0, 1.0, 0.0]);
        self.out.extend(0u32.to_le_bytes());
        self.tag("CDataTen");
        self.base(100, 1);
        self.reals(&[1.0, 1.0]);
        self.out.extend([0, 0, 0, 0, 2, 0, 0, 0]);
        self.reals(&[0.0, 1.0]);
        self.tag("CDataMoji");
        self.base(1, 1);
        self.text_body("図面");
        self.tag("CDataSolid");
        self.base(1, 10);
        self.reals(&[0.0; 8]);
        self.out.extend(0x00FF_0000u32.to_le_bytes());
        self.tag("CDataBlock");
        self.base(1, 1);
        self.reals(&[5.0, 5.0, 1.0, 1.0, 0.0]);
        self.out.extend(1u32.to_le_bytes());
        self.tag("CDataSunpou");
        self.base(1, 1);
        self.base(1, 1);
        self.reals(&[0.0, 0.0, 10.0, 0.0]);
        self.base(1, 1);
        self.text_body("10");
        if self.version >= 420 {
            self.out.extend(1u16.to_le_bytes());
            for _ in 0..2 {
                self.base(1, 1);
                self.reals(&[0.0; 4]);
            }
            for _ in 0..4 {
                self.base(1, 1);
                self.reals(&[0.0; 2]);
                self.out.extend(0u32.to_le_bytes());
            }
        }
        self.null();
    }

    fn block(&mut self, name: &str) {
        self.tag("CDataList");
        self.base(1, 1);
        self.out.extend([1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        self.cstring(name);
        self.count(1);
        self.line();
    }
}

/// 図形を 1 つずつと、ブロックの定義を 1 つ持つ図面。版 700 からは画像を 1 つ持つ
fn drawing(version: u32) -> (Vec<u8>, Archive) {
    let mut archive = Archive::new(version);
    archive.count(9);
    archive.all_kinds();
    archive.count(1);
    archive.block("部品");
    if version >= 700 {
        archive.out.extend(1u32.to_le_bytes());
        archive.cstring("a.bmp");
        archive.out.extend(3u32.to_le_bytes());
        archive.out.extend(b"BMP");
    }
    let mut src = header(version);
    src.extend(&archive.out);
    (src, archive)
}

#[cfg(test)]
fn read(src: &[u8]) -> (JwwFile, Vec<zumen_base::Diagnostic>) {
    let (file, diag) = testkit::read::<Jww>(src);
    (file.unwrap(), diag)
}

fn kinds(file: &JwwFile) -> Vec<String> {
    file.records()
        .iter()
        .map(|record| String::from_utf8_lossy(file.kind_name(record)).into_owned())
        .collect()
}

#[test]
fn records_keep_the_bytes() {
    let (src, archive) = drawing(700);
    testkit::roundtrip::<Jww>(&src).unwrap();
    testkit::records_in_order::<Jww>(&src).unwrap();
    testkit::deterministic::<Jww>(&src).unwrap();
    let (file, diag) = read(&src);
    assert!(diag.is_empty(), "{diag:?}");
    assert_eq!(file.version(), 700);
    assert_eq!(
        kinds(&file),
        [
            "header",
            "CDataSen",
            "CDataSen",
            "CDataEnko",
            "CDataTen",
            "CDataMoji",
            "CDataSolid",
            "CDataBlock",
            "CDataSunpou",
            "null",
            "CDataList",
            "CDataSen",
            "image-count",
            "image",
        ]
    );
    let records = file.records();
    let header_len = header(700).len();
    assert_eq!(records[0].span(), 0..header_len);
    for (record, start) in records[1..12].iter().zip(&archive.starts) {
        assert_eq!(record.span().start, header_len + start);
    }
    // 図形の一覧の件数は、最初の図形の前にある
    assert_eq!(records[1].span().start, header_len + 2);
    assert_eq!(records[1].tag().map(|(tag, _)| tag), Some(Tag::New));
    assert_eq!(records[2].tag().map(|(tag, _)| tag), Some(Tag::Known));
    // ブロックの定義は、中の一覧の件数までで、中の図形はその後ろ
    assert_eq!(records[10].kind(), RecordKind::Block);
    assert_eq!(records[10].span().end, records[11].span().start);
    assert_eq!(records[11].block(), Some(10));
    assert_eq!(records[1].block(), None);
    assert_eq!(file.bytes(records[13].body()), b"BMP");
    assert_eq!(records[13].span().end, src.len());
    let class = &file.classes()[0];
    assert_eq!(file.bytes(class.name()), b"CDataSen");
    assert_eq!(class.schema(), 1);
}

#[test]
fn older_versions_have_no_images_and_no_pen_width() {
    for version in [230, 300, 351, 420, 600] {
        let (src, _) = drawing(version);
        testkit::roundtrip::<Jww>(&src).unwrap();
        let (file, diag) = read(&src);
        assert!(diag.is_empty(), "{version}: {diag:?}");
        assert_eq!(file.records().len(), 12, "{version}");
        assert_eq!(file.records()[11].span().end, src.len());
    }
}

#[test]
fn an_empty_drawing_ends_with_the_image_count() {
    let mut src = header(700);
    src.extend([0, 0, 0, 0, 0, 0, 0, 0]);
    testkit::roundtrip::<Jww>(&src).unwrap();
    let (file, diag) = read(&src);
    assert!(diag.is_empty(), "{diag:?}");
    assert_eq!(kinds(&file), ["header", "image-count"]);
    assert_eq!(file.records()[0].span(), 0..src.len() - 8);
}

#[test]
fn unknown_header_fields_are_found_by_the_list_start() {
    let (src, _) = drawing(700);
    let at = header(700).len();
    let mut longer = src[..at].to_vec();
    longer.extend([1; 20]);
    longer.extend(&src[at..]);
    testkit::roundtrip::<Jww>(&longer).unwrap();
    let (file, diag) = read(&longer);
    assert!(diag.is_empty(), "{diag:?}");
    assert_eq!(file.records()[0].span(), 0..at + 20);
    assert_eq!(file.records().len(), 14);

    // ブロックの定義だけがある図面
    let mut archive = Archive::new(700);
    archive.count(0);
    archive.count(1);
    archive.block("部品");
    archive.out.extend(0u32.to_le_bytes());
    let mut src = header(700);
    src.extend([1; 20]);
    src.extend(&archive.out);
    let (file, diag) = read(&src);
    assert!(diag.is_empty(), "{diag:?}");
    assert_eq!(
        kinds(&file),
        ["header", "CDataList", "CDataSen", "image-count"]
    );
}

#[test]
fn unreadable_images_are_kept_as_they_are() {
    let (src, _) = drawing(700);
    let mut broken = src.clone();
    // 画像の大きさを、残りより大きくする
    let size = broken.len() - 7;
    broken[size..size + 4].copy_from_slice(&100u32.to_le_bytes());
    testkit::roundtrip::<Jww>(&broken).unwrap();
    let (file, diag) = read(&broken);
    assert_eq!(diag.len(), 1);
    assert_eq!(diag[0].code(), codes::JWW_TRAILER_INVALID);
    let trailer = file.records().last().unwrap();
    assert_eq!(trailer.kind(), RecordKind::Trailer);
    assert_eq!(trailer.span().end, broken.len());
    assert_eq!(
        diag[0]
            .places()
            .first()
            .and_then(Ref::bytes)
            .map(|r| r.start),
        Some(trailer.span().start as u64)
    );
}

#[test]
fn unreadable_files_are_reported() {
    let (result, diag) = testkit::read::<Jww>(b"JwwData");
    assert!(matches!(result, Err(Error::Invalid)));
    assert_eq!(diag[0].code(), codes::JWW_FILE_INVALID);

    // 知らないクラスは読めない
    let mut src = header(700);
    src.extend([1, 0, 0xFF, 0xFF, 1, 0, 8, 0]);
    src.extend(b"CDataXyz");
    src.extend([0; 40]);
    let (result, diag) = testkit::read::<Jww>(&src);
    assert!(matches!(result, Err(Error::Invalid)));
    assert_eq!(diag[0].code(), codes::JWW_BODY_INVALID);

    // 図形の一覧の途中で終わる
    let (src, _) = drawing(600);
    let (result, diag) = testkit::read::<Jww>(&src[..src.len() - 10]);
    assert!(matches!(result, Err(Error::Invalid)));
    assert_eq!(diag[0].code(), codes::JWW_BODY_INVALID);
}

#[test]
fn stopping_for_the_budget_names_the_record() {
    let (src, archive) = drawing(700);
    let mut budget = Budget::new(Limits::default().with(Kind::Records, Limit::Finite(2)));
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let result = Jww::read(&src, &mut Cx::new(&mut budget, &mut diag, &cancel));
    assert!(matches!(result, Err(Error::Stopped(Stop::Exceeded(_)))));
    let stop = diag.iter().next().unwrap();
    assert_eq!(stop.code(), codes::BUDGET_RECORDS_EXCEEDED);
    let place = stop.places().first().and_then(Ref::bytes).unwrap();
    assert_eq!(place.start as usize, header(700).len() + archive.starts[2]);
}
