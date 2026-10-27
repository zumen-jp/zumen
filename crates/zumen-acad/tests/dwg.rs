//! DWG（R2000）の試作を、試験の中で組み立てた DWG で確かめる。受け取った図面は使わない。

use zumen_acad::{AcadDb, Data, Dwg, DwgFile, Dxf, Line, Origin, Section, Value};
use zumen_base::{Budget, CancelToken, Code, Cx, Diagnostics, Kind, Limit, Limits, codes};
use zumen_sdk::{Codec, Error, FileModel, Transact, Transaction};

/// ビットを上位から詰める
#[cfg(test)]
#[derive(Default)]
struct BitWriter {
    bits: Vec<bool>,
}

#[cfg(test)]
impl BitWriter {
    fn bits(&mut self, value: u64, n: u32) {
        for i in (0..n).rev() {
            self.bits.push((value >> i) & 1 == 1);
        }
    }
    fn bit(&mut self, value: bool) {
        self.bits.push(value);
    }
    fn bytes(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.bits(u64::from(b), 8);
        }
    }
    fn rl(&mut self, value: u32) {
        self.bytes(&value.to_le_bytes());
    }
    fn rd(&mut self, value: f64) {
        self.bytes(&value.to_le_bytes());
    }
    /// BS。値によって短い形を選ぶ
    fn bs(&mut self, value: u16) {
        match value {
            0 => self.bits(2, 2),
            256 => self.bits(3, 2),
            1..=255 => {
                self.bits(1, 2);
                self.bits(u64::from(value), 8);
            }
            _ => {
                self.bits(0, 2);
                self.bytes(&value.to_le_bytes());
            }
        }
    }
    fn bl_zero(&mut self) {
        self.bits(2, 2);
    }
    fn bd(&mut self, value: f64) {
        if value == 1.0 {
            self.bits(1, 2);
        } else if value == 0.0 {
            self.bits(2, 2);
        } else {
            self.bits(0, 2);
            self.rd(value);
        }
    }
    /// DD。既定の値と同じなら 00、違えば 11 と RD
    fn dd(&mut self, value: f64, default: f64) {
        if value == default {
            self.bits(0, 2);
        } else {
            self.bits(3, 2);
            self.rd(value);
        }
    }
    fn handle(&mut self, code: u8, value: u64) {
        let bytes: Vec<u8> = value
            .to_be_bytes()
            .into_iter()
            .skip_while(|b| *b == 0)
            .collect();
        self.bits(u64::from(code), 4);
        self.bits(bytes.len() as u64, 4);
        self.bytes(&bytes);
    }
    /// `at` ビット目から 32 ビットを、RL の `value` で書き換える
    fn patch_rl(&mut self, at: usize, value: u32) {
        let mut patch = BitWriter::default();
        patch.rl(value);
        self.bits.splice(at..at + 32, patch.bits);
    }
    fn finish(&self) -> Vec<u8> {
        self.bits
            .chunks(8)
            .map(|chunk| {
                chunk
                    .iter()
                    .enumerate()
                    .fold(0u8, |byte, (i, bit)| byte | (u8::from(*bit) << (7 - i)))
            })
            .collect()
    }
}

#[cfg(test)]
fn crc16(seed: u16, bytes: &[u8]) -> u16 {
    bytes.iter().fold(seed, |mut crc, &byte| {
        crc ^= u16::from(byte);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xA001
            } else {
                crc >> 1
            };
        }
        crc
    })
}

/// 剰余の文字（MC）。`signed` なら、最後のバイトの 0x40 を負の印にする
#[cfg(test)]
fn modular_char(value: i64, signed: bool) -> Vec<u8> {
    let mut rest = value.unsigned_abs();
    let mut out = Vec::new();
    loop {
        let group = (rest & 0x7F) as u8;
        rest >>= 7;
        let last = rest == 0 && !(signed && group & 0x40 != 0);
        if last {
            out.push(if value < 0 { group | 0x40 } else { group });
            return out;
        }
        out.push(group | 0x80);
    }
}

/// オブジェクトの形。`body` は自分のハンドルの後のデータ、`handles` はハンドルの並びを書く
#[cfg(test)]
fn object(
    type_code: u16,
    handle: u64,
    body: impl Fn(&mut BitWriter),
    handles: impl Fn(&mut BitWriter),
) -> Vec<u8> {
    let mut w = BitWriter::default();
    w.bs(type_code);
    let rl_at = w.bits.len();
    w.rl(0);
    w.handle(0, handle);
    w.bs(0); // 拡張データはない
    body(&mut w);
    let handles_at = w.bits.len();
    w.patch_rl(rl_at, handles_at as u32);
    handles(&mut w);
    let data = w.finish();
    let mut out = (data.len() as u16).to_le_bytes().to_vec();
    out.extend(&data);
    let crc = crc16(0xC0C1, &out);
    out.extend(crc.to_le_bytes());
    out
}

/// 図形の共通の欄。entmode と色・線種の尺度・非表示を選ぶ
#[cfg(test)]
fn entity_common(w: &mut BitWriter, entmode: u8, color: u16, ltype_scale: f64, invisible: u16) {
    w.bit(false); // 図形の画像はない
    w.bits(u64::from(entmode), 2);
    w.bl_zero(); // 反応子はない
    w.bit(true); // 前後の図形へのリンクはない
    w.bs(color);
    w.bd(ltype_scale);
    w.bits(0, 2); // 線種は画層による
    w.bits(0, 2); // 印刷スタイルは画層による
    w.bs(invisible);
    w.bytes(&[0x1D]); // 線の太さ
}

/// 所有者を持たず、線種の印を選んだ図形の共通の欄
#[cfg(test)]
fn entity_common_with_ltype(w: &mut BitWriter, ltype_flags: u8) {
    w.bit(false);
    w.bits(2, 2);
    w.bl_zero();
    w.bit(true);
    w.bs(256);
    w.bd(1.0);
    w.bits(u64::from(ltype_flags), 2);
    w.bits(0, 2);
    w.bs(0);
    w.bytes(&[0x1D]);
}

/// LINE の欄
#[cfg(test)]
fn line_data(w: &mut BitWriter, start: [f64; 3], end: [f64; 3], thickness: f64) {
    let flat = start[2] == 0.0 && end[2] == 0.0;
    w.bit(flat);
    w.rd(start[0]);
    w.dd(end[0], start[0]);
    w.rd(start[1]);
    w.dd(end[1], start[1]);
    if !flat {
        w.rd(start[2]);
        w.dd(end[2], start[2]);
    }
    if thickness == 0.0 {
        w.bit(true);
    } else {
        w.bit(false);
        w.bd(thickness);
    }
    w.bit(true); // 押し出しの向きは (0, 0, 1)
}

/// 名前を持つ表のレコードの欄。反応子の数と、終わりの NUL を含む長さの名前と、表のレコードの
/// 共通の欄。型ごとの欄は、どの型でも 0 と空の文字列として読める 0 のビットで埋める
#[cfg(test)]
fn record_named(name: &'static [u8]) -> impl Fn(&mut BitWriter) {
    move |w| {
        w.bl_zero();
        w.bs(name.len() as u16 + 1);
        w.bytes(name);
        w.bytes(&[0]);
        w.bit(false); // 参照される印
        w.bs(0); // 外部参照の番号
        w.bit(false); // 外部参照に依る印
        for _ in 0..40 {
            w.bits(0, 64);
        }
    }
}

/// 表のレコードのハンドル。所有者・拡張辞書・外部参照のブロックと、型ごとの 0 のハンドル（DIMSTYLE の 5 つまで）
#[cfg(test)]
fn record_handles(owner: u64) -> impl Fn(&mut BitWriter) {
    move |w| {
        w.handle(4, owner);
        w.handle(3, 0);
        for _ in 0..6 {
            w.handle(5, 0);
        }
    }
}

/// 所有者を持たない図形の、拡張辞書と画層のハンドル
#[cfg(test)]
fn entity_handles(layer: u64) -> impl Fn(&mut BitWriter) {
    move |w| {
        w.handle(3, 0);
        w.handle(5, layer);
    }
}

/// 所有者を持たない図形の、拡張辞書・画層・型ごとに指す表のレコードのハンドル
#[cfg(test)]
fn entity_handles_to(layer: u64, record: u64) -> impl Fn(&mut BitWriter) {
    move |w| {
        entity_handles(layer)(w);
        w.handle(5, record);
    }
}

#[cfg(test)]
fn objects_in_sample() -> Vec<(u64, Vec<u8>)> {
    // 画層の表と、画層
    let control = object(0x32, 2, |w| w.bl_zero(), |w| w.handle(4, 0));
    let layer = object(0x33, 0x10, record_named(b"0"), record_handles(2));
    // 所有者を持つ LINE（entmode 0）
    let line = object(
        0x13,
        0x2A,
        |w| {
            entity_common(w, 0, 256, 1.0, 0);
            line_data(w, [1.5, 2.0, 0.0], [100.0, 50.0, 0.0], 0.0);
        },
        |w| {
            w.handle(4, 0x1F); // 所有者
            w.handle(3, 0); // 拡張辞書
            w.handle(5, 0x10); // 画層
        },
    );
    // モデル空間の LINE（entmode 2）。共通の欄も既定の値でない
    let line3d = object(
        0x13,
        0x2B,
        |w| {
            entity_common(w, 2, 1, 2.0, 1);
            line_data(w, [0.0, 0.0, 3.0], [0.0, 4.0, 3.0], 0.5);
        },
        |w| {
            w.handle(3, 0);
            w.handle(5, 0x10);
        },
    );
    // クラスで決まる型
    let custom = object(500, 0x2C, |w| w.bl_zero(), |w| w.handle(4, 0));
    // 欄の途中で尽きる CIRCLE
    let mut circle = BitWriter::default();
    circle.bs(0x12);
    circle.rl(0);
    let data = circle.finish();
    let mut broken = (data.len() as u16).to_le_bytes().to_vec();
    broken.extend(&data);
    let crc = crc16(0xC0C1, &broken);
    broken.extend(crc.to_le_bytes());
    vec![
        (2, control),
        (0x10, layer),
        (0x2A, line),
        (0x2B, line3d),
        (0x2C, custom),
        (0x2D, broken),
    ]
}

/// 見出し・オブジェクト・オブジェクトの地図を並べた R2000 の DWG
#[cfg(test)]
fn dwg(objects: &[(u64, Vec<u8>)]) -> Vec<u8> {
    dwg_in_sections(objects, usize::MAX)
}

/// 地図の区切りごとに、`per_section` 個までのオブジェクトを置く DWG
#[cfg(test)]
fn dwg_in_sections(objects: &[(u64, Vec<u8>)], per_section: usize) -> Vec<u8> {
    const SENTINEL: [u8; 16] = [
        0x95, 0xA0, 0x4E, 0x28, 0x99, 0x82, 0x1A, 0xE5, 0x5E, 0x41, 0xE0, 0x5F, 0x9D, 0x3A, 0x4D,
        0x00,
    ];
    let header_size = 0x19 + 3 * 9 + 2 + 16;
    let mut body: Vec<u8> = Vec::new();
    let mut maps = Vec::new();
    for chunk in objects.chunks(per_section) {
        // 差分は、区切りごとに 0 から数える
        let mut map = Vec::new();
        let (mut last_handle, mut last_location) = (0u64, 0i64);
        for (handle, bytes) in chunk {
            let location = (header_size + body.len()) as i64;
            map.extend(modular_char((handle - last_handle) as i64, false));
            map.extend(modular_char(location - last_location, true));
            (last_handle, last_location) = (*handle, location);
            body.extend(bytes);
        }
        maps.push(map);
    }
    maps.push(Vec::new());
    let mut sections = Vec::new();
    for data in maps {
        let mut section = ((data.len() + 2) as u16).to_be_bytes().to_vec();
        section.extend(data);
        let crc = crc16(0xC0C1, &section);
        section.extend(crc.to_be_bytes());
        sections.extend(section);
    }
    let map_at = header_size + body.len();

    let mut out = b"AC1015".to_vec();
    out.resize(0x13, 0);
    out.extend(30u16.to_le_bytes()); // コード・ページ
    out.extend(3u32.to_le_bytes());
    for (number, seeker, size) in [
        (0u8, 0u32, 0u32),
        (1, 0, 0),
        (2, map_at as u32, sections.len() as u32),
    ] {
        out.push(number);
        out.extend(seeker.to_le_bytes());
        out.extend(size.to_le_bytes());
    }
    let crc = crc16(0, &out) ^ 0xA598;
    out.extend(crc.to_le_bytes());
    out.extend(SENTINEL);
    assert_eq!(out.len(), header_size);
    out.extend(body);
    out.extend(sections);
    out
}

/// DWG の終わりにクラスの区切りを足し、区切りの記録の 1 番をそれに向ける。クラスは番号・DXF の名前・
/// 図形を作るか
#[cfg(test)]
fn with_classes(mut src: Vec<u8>, classes: &[(u16, &[u8], bool)]) -> Vec<u8> {
    let mut w = BitWriter::default();
    for (number, name, entity) in classes {
        w.bs(*number);
        w.bs(0);
        for text in [&b"ZumenTest"[..], b"AcDbTest", name] {
            w.bs(text.len() as u16 + 1);
            w.bytes(text);
            w.bytes(&[0]);
        }
        w.bit(false);
        w.bs(if *entity { 0x1F2 } else { 0x1F3 });
    }
    let data = w.finish();
    let mut section = vec![
        0x8D, 0xA1, 0xC4, 0xB8, 0xC4, 0xA9, 0xF8, 0xC5, 0xC0, 0xDC, 0xF4, 0x5F, 0xE7, 0xCF, 0xB6,
        0x8A,
    ];
    let mut sized = (data.len() as u32).to_le_bytes().to_vec();
    sized.extend(&data);
    section.extend(&sized);
    section.extend(crc16(0xC0C1, &sized).to_le_bytes());
    section.extend([
        0x72, 0x5E, 0x3B, 0x47, 0x3B, 0x56, 0x07, 0x3A, 0x3F, 0x23, 0x0B, 0xA0, 0x18, 0x30, 0x49,
        0x75,
    ]);
    let at = 0x19 + 9;
    assert_eq!(src[at], 1);
    let seeker = src.len() as u32;
    src[at + 1..at + 5].copy_from_slice(&seeker.to_le_bytes());
    src[at + 5..at + 9].copy_from_slice(&(section.len() as u32).to_le_bytes());
    let crc_at = 0x19 + 27;
    let crc = crc16(0, &src[..crc_at]) ^ 0xA598;
    src[crc_at..crc_at + 2].copy_from_slice(&crc.to_le_bytes());
    src.extend(section);
    src
}

/// DWG の終わりに見出しの変数の区切りを足し、区切りの記録の 0 番をそれに向ける。変数は
/// 2 つの空間のハンドルのほかは 0 か空で、CEPSNTYPE は 3（CPSNID のハンドルが続く）
#[cfg(test)]
fn with_header_variables(
    src: Vec<u8>,
    paper: u64,
    model: u64,
    units: u16,
    limits: [f64; 4],
) -> Vec<u8> {
    with_header(src, paper, model, units, limits, 0)
}

/// [`with_header_variables`] と同じく、HANDSEED を `seed` にした見出しの変数の区切りを足したもの
#[cfg(test)]
fn with_header(
    mut src: Vec<u8>,
    paper: u64,
    model: u64,
    units: u16,
    limits: [f64; 4],
    seed: u64,
) -> Vec<u8> {
    // 型（b・s・l・d・r・t・h は B・BS・BL・BD・RD・TV・H）と数を、R2000 の順に並べる。
    // 用紙空間の LIMMIN・LIMMAX（P）と INSUNITS（U）と HANDSEED（S）には値を書く
    let space = "d9 r4 d10 h2 s1 h1 d18";
    let layout = format!(
        "d4 t4 l2 h1 b20 s27 d21 t1 l8 s1 S1 h5 d1 d9 P4 d10 h2 s1 h1 d18 {space} \
         t2 d9 b6 s3 d9 b1 s1 b4 s3 s11 b2 s4 b1 s1 h5 s2 h13 s2 t2 h3 l1 U1"
    );
    let mut w = BitWriter::default();
    let mut limits = limits.into_iter();
    for item in layout.split_whitespace() {
        let (kind, count) = item.split_at_checked(1).unwrap();
        let count: usize = count.parse().unwrap();
        for _ in 0..count {
            match kind {
                "b" => w.bit(false),
                "s" | "t" => w.bs(0),
                "l" => w.bl_zero(),
                "d" => w.bd(0.0),
                "r" => w.rd(0.0),
                "P" => w.rd(limits.next().unwrap()),
                "U" => w.bs(units),
                "S" => w.handle(0, seed),
                _ => w.handle(5, 0),
            }
        }
    }
    w.bs(3);
    w.handle(5, 0);
    w.bs(0);
    w.bs(0);
    w.handle(5, paper);
    w.handle(5, model);
    let data = w.finish();
    let mut section = vec![
        0xCF, 0x7B, 0x1F, 0x23, 0xFD, 0xDE, 0x38, 0xA9, 0x5F, 0x7C, 0x68, 0xB8, 0x4E, 0x6D, 0x33,
        0x5F,
    ];
    let mut sized = (data.len() as u32).to_le_bytes().to_vec();
    sized.extend(&data);
    section.extend(&sized);
    section.extend(crc16(0xC0C1, &sized).to_le_bytes());
    section.extend([
        0x30, 0x84, 0xE0, 0xDC, 0x02, 0x21, 0xC7, 0x56, 0xA0, 0x83, 0x97, 0x47, 0xB1, 0x92, 0xCC,
        0xA0,
    ]);
    let at = 0x19;
    assert_eq!(src[at], 0);
    let seeker = src.len() as u32;
    src[at + 1..at + 5].copy_from_slice(&seeker.to_le_bytes());
    src[at + 5..at + 9].copy_from_slice(&(section.len() as u32).to_le_bytes());
    let crc_at = 0x19 + 27;
    let crc = crc16(0, &src[..crc_at]) ^ 0xA598;
    src[crc_at..crc_at + 2].copy_from_slice(&crc.to_le_bytes());
    src.extend(section);
    src
}

#[cfg(test)]
fn sample() -> Vec<u8> {
    dwg(&objects_in_sample())
}

#[cfg(test)]
fn read(src: &[u8], limits: Limits) -> (Result<DwgFile, Error>, Diagnostics) {
    let mut diag = Diagnostics::new();
    let mut budget = Budget::new(limits);
    let result = Dwg::read(
        src,
        &mut Cx::new(&mut budget, &mut diag, &CancelToken::new()),
    );
    (result, diag)
}

#[cfg(test)]
fn objects(src: &[u8]) -> (AcadDb, Diagnostics) {
    let (file, mut diag) = read(src, Limits::default());
    let db = file
        .unwrap()
        .objects(&mut Cx::new(
            &mut Budget::default(),
            &mut diag,
            &CancelToken::new(),
        ))
        .unwrap();
    (db, diag)
}

#[cfg(test)]
fn codes_of(diag: &Diagnostics) -> Vec<Code> {
    diag.iter().map(|d| d.code()).collect()
}

#[test]
fn the_header_and_the_object_map_are_read() {
    let (file, diag) = read(&sample(), Limits::default());
    let file = file.unwrap();
    assert!(diag.is_empty(), "{:?}", codes_of(&diag));
    assert_eq!(file.version(), b"AC1015");
    assert_eq!(file.code_page(), 30);
    let numbers: Vec<_> = file.locators().iter().map(|l| l.number).collect();
    assert_eq!(numbers, [0, 1, 2]);
    assert_eq!(file.record_count(), 6);
    let kinds: Vec<_> = (0..6).map(|i| file.record(i).unwrap().kind).collect();
    assert_eq!(
        kinds,
        [&b"TABLE"[..], b"LAYER", b"LINE", b"LINE", b"", b"CIRCLE"]
    );
    // オブジェクトは、見出しの後から隙間なく並ぶ
    let first = file.record(0).unwrap().bytes.unwrap();
    let second = file.record(1).unwrap().bytes.unwrap();
    assert_eq!(first.start, 0x19 + 27 + 18);
    assert_eq!(first.end, second.start);
}

#[test]
fn each_map_section_counts_its_offsets_from_zero() {
    let one = read(&sample(), Limits::default()).0.unwrap();
    let (split, diag) = read(&dwg_in_sections(&objects_in_sample(), 2), Limits::default());
    let split = split.unwrap();
    assert!(diag.is_empty(), "{:?}", codes_of(&diag));
    assert_eq!(split.record_count(), one.record_count());
    for i in 0..one.record_count() {
        let (a, b) = (one.record(i).unwrap(), split.record(i).unwrap());
        assert_eq!((a.kind, a.bytes), (b.kind, b.bytes));
    }
}

#[test]
fn objects_go_into_the_shared_table() {
    let (db, diag) = objects(&sample());
    assert_eq!(
        codes_of(&diag),
        [codes::DWG_TYPE_UNSUPPORTED, codes::DWG_OBJECT_INVALID]
    );
    let found: Vec<_> = db
        .objects()
        .iter()
        .map(|o| (o.kind.as_slice(), o.section.clone(), o.handle(), o.owner()))
        .collect();
    assert_eq!(
        found,
        [
            (&b"TABLE"[..], Section::Tables, Some(2), Some(0)),
            (b"LAYER", Section::Tables, Some(0x10), Some(2)),
            (b"LINE", Section::Entities, Some(0x2A), Some(0x1F)),
            (b"LINE", Section::Entities, Some(0x2B), None),
            (b"", Section::Other(Vec::new()), Some(0x2C), None),
            (b"CIRCLE", Section::Entities, Some(0x2D), None),
        ]
    );
    assert!(
        matches!(db.objects()[0].origin, Origin::Dwg { offset, .. } if offset == 0x19 + 27 + 18)
    );
}

#[test]
fn a_line_has_the_same_values_as_in_dxf() {
    let (db, _) = objects(&sample());
    let text = "  0\nSECTION\n  2\nENTITIES\n  0\nLINE\n  5\n2A\n330\n1F\n  8\n0\n 10\n1.5\n 20\n2\n 30\n0\n 11\n100\n 21\n50\n 31\n0\n  0\nENDSEC\n  0\nEOF\n";
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let mut budget = Budget::default();
    let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
    let from_dxf = Dxf::read(text.as_bytes(), &mut cx)
        .unwrap()
        .objects(&mut cx)
        .unwrap();
    let dwg_line = db.get(0x2A).unwrap();
    let dxf_line = from_dxf.get(0x2A).unwrap();
    for code in [10, 20, 30, 11, 21, 31] {
        assert_eq!(dwg_line.real(code), dxf_line.real(code), "{code}");
    }
    assert_eq!(dwg_line.owner(), dxf_line.owner());
    assert_eq!(dwg_line.first(8), dxf_line.first(8));
    assert_eq!(dwg_line.first(8), Some(&Data::Text(b"0".to_vec())));
    // 既定の値の欄は置かない
    for code in [39, 48, 60, 62, 210] {
        assert_eq!(dwg_line.first(code), None, "{code}");
    }
}

#[test]
fn fields_that_differ_from_their_defaults_are_kept() {
    let (db, _) = objects(&sample());
    let line = db.get(0x2B).unwrap();
    assert_eq!(line.first(62), Some(&Data::Int(1)));
    assert_eq!(line.real(48), Some(Some(2.0)));
    assert_eq!(line.first(60), Some(&Data::Int(1)));
    assert_eq!(line.real(30), Some(Some(3.0)));
    assert_eq!(line.real(31), Some(Some(3.0)));
    assert_eq!(line.real(11), Some(Some(0.0)));
    assert_eq!(line.real(21), Some(Some(4.0)));
    assert_eq!(line.real(39), Some(Some(0.5)));
    assert_eq!(line.real(210), None);
}

#[test]
fn layers_keep_their_names() {
    let (db, _) = objects(&sample());
    assert_eq!(
        db.get(0x10).unwrap().first(2),
        Some(&Data::Text(b"0".to_vec()))
    );
    // 所有者がなくても、画層の名前は置く
    assert_eq!(
        db.get(0x2B).unwrap().first(8),
        Some(&Data::Text(b"0".to_vec()))
    );
}

#[cfg(test)]
fn close(found: Option<Option<f64>>, expected: f64) -> bool {
    found
        .flatten()
        .is_some_and(|f| (f - expected).abs() < 1e-12)
}

#[test]
fn arcs_circles_and_points_have_their_fields() {
    let layer = object(0x33, 0x10, record_named(b"WALL"), record_handles(2));
    let circle = object(
        0x12,
        0x20,
        |w| {
            entity_common(w, 2, 256, 1.0, 0);
            for v in [1.0, 2.0, 0.0, 5.0] {
                w.bd(v);
            }
            w.bit(false);
            w.bd(0.25); // 厚さ
            w.bit(true);
        },
        entity_handles(0x10),
    );
    let arc = object(
        0x11,
        0x21,
        |w| {
            entity_common(w, 2, 256, 1.0, 0);
            for v in [0.0, 0.0, 0.0, 3.0] {
                w.bd(v);
            }
            w.bit(true);
            w.bit(false); // 押し出しの向きを書く
            for v in [0.0, 0.0, -1.0] {
                w.bd(v);
            }
            w.bd(std::f64::consts::FRAC_PI_2);
            w.bd(std::f64::consts::PI);
        },
        entity_handles(0x10),
    );
    let point = object(
        0x1B,
        0x22,
        |w| {
            entity_common(w, 2, 256, 1.0, 0);
            for v in [7.0, 8.0, 9.0] {
                w.bd(v);
            }
            w.bit(true);
            w.bit(true);
            w.bd(std::f64::consts::FRAC_PI_4);
        },
        entity_handles(0x10),
    );
    let (db, diag) = objects(&dwg(&[
        (0x10, layer),
        (0x20, circle),
        (0x21, arc),
        (0x22, point),
    ]));
    assert!(diag.is_empty(), "{:?}", codes_of(&diag));
    let wall = Some(&Data::Text(b"WALL".to_vec()));

    let circle = db.get(0x20).unwrap();
    assert_eq!(circle.kind, b"CIRCLE");
    assert_eq!(circle.first(8), wall);
    for (code, value) in [(10, 1.0), (20, 2.0), (30, 0.0), (40, 5.0), (39, 0.25)] {
        assert!(close(circle.real(code), value), "{code}");
    }
    assert_eq!(circle.first(210), None);

    let arc = db.get(0x21).unwrap();
    assert_eq!(arc.kind, b"ARC");
    assert_eq!(arc.first(8), wall);
    // 角度は度に直す
    for (code, value) in [(40, 3.0), (230, -1.0), (50, 90.0), (51, 180.0)] {
        assert!(close(arc.real(code), value), "{code}");
    }
    assert_eq!(arc.first(39), None);

    let point = db.get(0x22).unwrap();
    assert_eq!(point.kind, b"POINT");
    for (code, value) in [(10, 7.0), (20, 8.0), (30, 9.0), (50, 45.0)] {
        assert!(close(point.real(code), value), "{code}");
    }
}

#[test]
fn text_fields_are_read_by_their_flags() {
    let layer = object(0x33, 0x10, record_named(b"0"), record_handles(2));
    let standard = object(0x35, 0x11, record_named(b"Standard"), record_handles(3));
    let mono = object(0x35, 0x12, record_named(b"Mono"), record_handles(3));
    // DataFlags が 0xFF: 標高・合わせの点・角度・幅・生成・合わせの欄を持たない
    let plain = object(
        0x01,
        0x20,
        |w| {
            entity_common(w, 2, 256, 1.0, 0);
            w.bytes(&[0xFF]);
            w.rd(1.0);
            w.rd(2.0);
            w.bit(true);
            w.bit(true);
            w.rd(2.5);
            w.bs(6);
            w.bytes(b"Hello\0");
        },
        entity_handles_to(0x10, 0x11),
    );
    // DataFlags が 0: どの欄も持つ
    let full = object(
        0x01,
        0x21,
        |w| {
            entity_common(w, 2, 256, 1.0, 0);
            w.bytes(&[0]);
            w.rd(3.0); // 標高
            w.rd(1.0);
            w.rd(2.0);
            w.dd(5.0, 1.0);
            w.dd(2.0, 2.0);
            w.bit(true);
            w.bit(false);
            w.bd(0.5); // 厚さ
            w.rd(std::f64::consts::FRAC_PI_6); // 斜めの角度
            w.rd(std::f64::consts::FRAC_PI_2); // 回転
            w.rd(2.5);
            w.rd(0.8);
            w.bs(2);
            w.bytes(b"A\0");
            w.bs(2);
            w.bs(1);
            w.bs(3);
        },
        entity_handles_to(0x10, 0x12),
    );
    let (db, diag) = objects(&dwg(&[
        (0x10, layer),
        (0x11, standard),
        (0x12, mono),
        (0x20, plain),
        (0x21, full),
    ]));
    assert!(diag.is_empty(), "{:?}", codes_of(&diag));

    let plain = db.get(0x20).unwrap();
    assert_eq!(plain.kind, b"TEXT");
    assert_eq!(plain.first(1), Some(&Data::Text(b"Hello".to_vec())));
    for (code, value) in [(10, 1.0), (20, 2.0), (30, 0.0), (40, 2.5)] {
        assert!(close(plain.real(code), value), "{code}");
    }
    // 文字の体裁が Standard なら、7 は置かない
    for code in [7, 11, 39, 41, 50, 51, 71, 72, 73, 210] {
        assert_eq!(plain.first(code), None, "{code}");
    }

    let full = db.get(0x21).unwrap();
    assert_eq!(full.first(1), Some(&Data::Text(b"A".to_vec())));
    assert_eq!(full.first(7), Some(&Data::Text(b"Mono".to_vec())));
    let reals = [
        (30, 3.0),
        (11, 5.0),
        (21, 2.0),
        (31, 3.0),
        (39, 0.5),
        (51, 30.0),
        (50, 90.0),
        (41, 0.8),
    ];
    for (code, value) in reals {
        assert!(close(full.real(code), value), "{code}");
    }
    for (code, value) in [(71, 2), (72, 1), (73, 3)] {
        assert_eq!(full.first(code), Some(&Data::Int(value)), "{code}");
    }
}

#[test]
fn inserts_and_mtexts_have_their_fields() {
    let layer = object(0x33, 0x10, record_named(b"0"), record_handles(2));
    let block = object(0x31, 0x11, record_named(b"DOOR"), record_handles(1));
    let style = object(0x35, 0x12, record_named(b"Mono"), record_handles(3));
    // 尺度の印 00: 41 は RD、42・43 は 41 を既定の値とする DD
    let insert = object(
        0x07,
        0x20,
        |w| {
            entity_common(w, 2, 256, 1.0, 0);
            for v in [4.0, 5.0, 0.0] {
                w.bd(v);
            }
            w.bits(0, 2);
            w.rd(2.0);
            w.dd(3.0, 2.0);
            w.dd(2.0, 2.0);
            w.bd(std::f64::consts::PI);
            for v in [0.0, 0.0, 1.0] {
                w.bd(v);
            }
            w.bit(false); // 属性は続かない
        },
        entity_handles_to(0x10, 0x11),
    );
    // 尺度の印 11: 尺度は (1, 1, 1)
    let plain = object(
        0x07,
        0x21,
        |w| {
            entity_common(w, 2, 256, 1.0, 0);
            for v in [0.0, 0.0, 0.0] {
                w.bd(v);
            }
            w.bits(3, 2);
            w.bd(0.0);
            for v in [0.0, 0.0, 1.0] {
                w.bd(v);
            }
            w.bit(true);
        },
        entity_handles_to(0x10, 0x11),
    );
    let long: Vec<u8> = (0..600).map(|i| b'a' + (i % 26) as u8).collect();
    let mtext = object(
        0x2C,
        0x22,
        |w| {
            entity_common(w, 2, 256, 1.0, 0);
            for v in [1.0, 2.0, 0.0, 0.0, 0.0, 1.0, 0.0, 1.0, 0.0] {
                w.bd(v);
            }
            w.bd(10.0); // 幅
            w.bd(2.5); // 高さ
            w.bs(1);
            w.bs(5);
            w.bd(3.0); // 文字の範囲（DXF にない）
            w.bd(9.0);
            w.bs(601);
            w.bytes(&long);
            w.bytes(&[0]);
            w.bs(1);
            w.bd(1.0);
            w.bit(false);
        },
        entity_handles_to(0x10, 0x12),
    );
    let (db, diag) = objects(&dwg(&[
        (0x10, layer),
        (0x11, block),
        (0x12, style),
        (0x20, insert),
        (0x21, plain),
        (0x22, mtext),
    ]));
    assert!(diag.is_empty(), "{:?}", codes_of(&diag));
    assert_eq!(
        db.get(0x11).unwrap().first(2),
        Some(&Data::Text(b"DOOR".to_vec()))
    );

    let insert = db.get(0x20).unwrap();
    assert_eq!(insert.kind, b"INSERT");
    assert_eq!(insert.first(2), Some(&Data::Text(b"DOOR".to_vec())));
    let reals = [
        (10, 4.0),
        (20, 5.0),
        (41, 2.0),
        (42, 3.0),
        (43, 2.0),
        (50, 180.0),
    ];
    for (code, value) in reals {
        assert!(close(insert.real(code), value), "{code}");
    }
    for code in [66, 210] {
        assert_eq!(insert.first(code), None, "{code}");
    }
    let plain = db.get(0x21).unwrap();
    assert_eq!(plain.first(66), Some(&Data::Int(1)));
    for code in [41, 42, 43, 50] {
        assert_eq!(plain.first(code), None, "{code}");
    }

    let mtext = db.get(0x22).unwrap();
    assert_eq!(mtext.kind, b"MTEXT");
    assert_eq!(mtext.first(7), Some(&Data::Text(b"Mono".to_vec())));
    // 文字列は、250 バイトずつの 3 と残りの 1 に分ける
    let parts: Vec<_> = mtext
        .values
        .iter()
        .filter(|v| matches!(v.code, 1 | 3))
        .map(|v| match &v.data {
            Data::Text(t) => (v.code, t.len()),
            _ => (v.code, 0),
        })
        .collect();
    assert_eq!(parts, [(3, 250), (3, 250), (1, 100)]);
    for (code, value) in [
        (10, 1.0),
        (20, 2.0),
        (11, 0.0),
        (21, 1.0),
        (40, 2.5),
        (41, 10.0),
    ] {
        assert!(close(mtext.real(code), value), "{code}");
    }
    for (code, value) in [(71, 1), (72, 5), (73, 1)] {
        assert_eq!(mtext.first(code), Some(&Data::Int(value)), "{code}");
    }
    for code in [42, 43, 210] {
        assert_eq!(mtext.first(code), None, "{code}");
    }
}

#[test]
fn minserts_have_their_grid() {
    let layer = object(0x33, 0x10, record_named(b"0"), record_handles(2));
    let block = object(0x31, 0x11, record_named(b"DOOR"), record_handles(1));
    let minsert = |columns: u16, spacing: f64| {
        move |w: &mut BitWriter| {
            entity_common(w, 2, 256, 1.0, 0);
            for v in [4.0, 5.0, 0.0] {
                w.bd(v);
            }
            w.bits(3, 2);
            w.bd(0.0);
            for v in [0.0, 0.0, 1.0] {
                w.bd(v);
            }
            w.bit(false);
            w.bs(columns);
            w.bs(1);
            w.bd(spacing);
            w.bd(0.0);
        }
    };
    let grid = object(0x08, 0x20, minsert(3, 2.5), entity_handles_to(0x10, 0x11));
    let single = object(0x08, 0x21, minsert(1, 0.0), entity_handles_to(0x10, 0x11));
    let (db, diag) = objects(&dwg(&[
        (0x10, layer),
        (0x11, block),
        (0x20, grid),
        (0x21, single),
    ]));
    assert!(diag.is_empty(), "{:?}", codes_of(&diag));

    let grid = db.get(0x20).unwrap();
    assert_eq!(grid.kind, b"INSERT");
    assert_eq!(grid.first(2), Some(&Data::Text(b"DOOR".to_vec())));
    assert_eq!(grid.first(70), Some(&Data::Int(3)));
    assert!(close(grid.real(44), 2.5));
    for code in [71, 45] {
        assert_eq!(grid.first(code), None, "{code}");
    }
    let single = db.get(0x21).unwrap();
    for code in [70, 71, 44, 45] {
        assert_eq!(single.first(code), None, "{code}");
    }
}

#[test]
fn linetypes_are_named_by_their_flags() {
    let layer = object(0x33, 0x10, record_named(b"0"), record_handles(2));
    let dashed = object(0x39, 0x11, record_named(b"DASHED"), record_handles(2));
    let by_layer = object(0x39, 0x12, record_named(b"ByLayer"), record_handles(2));
    let line = |handle: u64, flags: u8, ltype: Option<u64>| {
        object(
            0x13,
            handle,
            move |w| {
                entity_common_with_ltype(w, flags);
                line_data(w, [0.0; 3], [1.0, 0.0, 0.0], 0.0);
            },
            move |w| {
                entity_handles(0x10)(w);
                if let Some(ltype) = ltype {
                    w.handle(5, ltype);
                }
            },
        )
    };
    let (db, diag) = objects(&dwg(&[
        (0x10, layer),
        (0x11, dashed),
        (0x12, by_layer),
        (0x20, line(0x20, 0, None)),
        (0x21, line(0x21, 1, None)),
        (0x22, line(0x22, 2, None)),
        (0x23, line(0x23, 3, Some(0x11))),
        (0x24, line(0x24, 3, Some(0x12))),
    ]));
    assert!(diag.is_empty(), "{:?}", codes_of(&diag));
    let ltype = |handle| db.get(handle).unwrap().first(6).cloned();
    let text = |t: &[u8]| Some(Data::Text(t.to_vec()));
    // 画層による線種（印が 0 か、ByLayer のレコード）は、DXF と同じく置かない
    assert_eq!(ltype(0x20), None);
    assert_eq!(ltype(0x21), text(b"ByBlock"));
    assert_eq!(ltype(0x22), text(b"Continuous"));
    assert_eq!(ltype(0x23), text(b"DASHED"));
    assert_eq!(ltype(0x24), None);
}

#[test]
fn solids_faces_ellipses_and_rays_have_their_fields() {
    let layer = object(0x33, 0x10, record_named(b"0"), record_handles(2));
    let solid = object(
        0x1F,
        0x20,
        |w| {
            entity_common(w, 2, 256, 1.0, 0);
            w.bit(true); // 厚さは 0
            w.bd(2.0); // 標高
            for v in [0.0, 0.0, 4.0, 0.0, 0.0, 3.0, 4.0, 3.0] {
                w.rd(v);
            }
            w.bit(true);
        },
        entity_handles(0x10),
    );
    let face = object(
        0x1C,
        0x21,
        |w| {
            entity_common(w, 2, 256, 1.0, 0);
            w.bit(false); // 見えない辺の印がある
            w.bit(false); // Z が 0 でない
            for v in [1.0, 2.0, 3.0] {
                w.rd(v);
            }
            // 2 つ目の角は X だけ変わる。3 つ目・4 つ目は前の角と同じ
            w.dd(5.0, 1.0);
            w.dd(2.0, 2.0);
            w.dd(3.0, 3.0);
            for _ in 0..6 {
                w.dd(0.0, 0.0);
            }
            w.bs(5);
        },
        entity_handles(0x10),
    );
    let ellipse = object(
        0x23,
        0x22,
        |w| {
            entity_common(w, 2, 256, 1.0, 0);
            for v in [1.0, 1.0, 0.0, 4.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.5, 0.0, 3.0] {
                w.bd(v);
            }
        },
        entity_handles(0x10),
    );
    let xline = object(
        0x29,
        0x23,
        |w| {
            entity_common(w, 2, 256, 1.0, 0);
            for v in [0.0, 5.0, 0.0, 0.6, 0.8, 0.0] {
                w.bd(v);
            }
        },
        entity_handles(0x10),
    );
    let (db, diag) = objects(&dwg(&[
        (0x10, layer),
        (0x20, solid),
        (0x21, face),
        (0x22, ellipse),
        (0x23, xline),
    ]));
    assert!(diag.is_empty(), "{:?}", codes_of(&diag));

    let solid = db.get(0x20).unwrap();
    assert_eq!(solid.kind, b"SOLID");
    for (code, value) in [(12, 0.0), (22, 3.0), (13, 4.0), (23, 3.0), (33, 2.0)] {
        assert!(close(solid.real(code), value), "{code}");
    }
    assert_eq!(solid.first(39), None);

    let face = db.get(0x21).unwrap();
    assert_eq!(face.kind, b"3DFACE");
    for (code, value) in [
        (10, 1.0),
        (11, 5.0),
        (21, 2.0),
        (31, 3.0),
        (13, 5.0),
        (33, 3.0),
    ] {
        assert!(close(face.real(code), value), "{code}");
    }
    assert_eq!(face.first(70), Some(&Data::Int(5)));

    let ellipse = db.get(0x22).unwrap();
    assert_eq!(ellipse.kind, b"ELLIPSE");
    // 角度は DXF と同じくラジアンのまま
    for (code, value) in [(10, 1.0), (11, 4.0), (40, 0.5), (41, 0.0), (42, 3.0)] {
        assert!(close(ellipse.real(code), value), "{code}");
    }
    assert_eq!(ellipse.first(210), None);

    let xline = db.get(0x23).unwrap();
    assert_eq!(xline.kind, b"XLINE");
    for (code, value) in [(20, 5.0), (11, 0.6), (21, 0.8)] {
        assert!(close(xline.real(code), value), "{code}");
    }
}

#[test]
fn shapes_keep_fields_that_differ_from_their_defaults() {
    let layer = object(0x33, 0x10, record_named(b"0"), record_handles(2));
    let shape = |rotation: f64, width: f64| {
        move |w: &mut BitWriter| {
            entity_common(w, 2, 256, 1.0, 0);
            for v in [1.0, 2.0, 0.0, 0.5, rotation, width, 0.0, 0.0] {
                w.bd(v);
            }
            w.bs(3); // 形の番号
            for v in [0.0, 0.0, 1.0] {
                w.bd(v);
            }
        }
    };
    let turned = object(
        0x21,
        0x20,
        shape(std::f64::consts::FRAC_PI_2, 2.0),
        entity_handles(0x10),
    );
    let plain = object(0x21, 0x21, shape(0.0, 1.0), entity_handles(0x10));
    let (db, diag) = objects(&dwg(&[(0x10, layer), (0x20, turned), (0x21, plain)]));
    assert!(diag.is_empty(), "{:?}", codes_of(&diag));

    let turned = db.get(0x20).unwrap();
    assert_eq!(turned.kind, b"SHAPE");
    for (code, value) in [(10, 1.0), (20, 2.0), (40, 0.5), (50, 90.0), (41, 2.0)] {
        assert!(close(turned.real(code), value), "{code}");
    }
    let plain = db.get(0x21).unwrap();
    for code in [50, 41, 51, 39, 210, 2] {
        assert_eq!(plain.first(code), None, "{code}");
    }
}

#[test]
fn lwpolylines_have_their_vertices() {
    let layer = object(0x33, 0x10, record_named(b"0"), record_handles(2));
    let lwpolyline = object(
        0x4D,
        0x20,
        |w| {
            entity_common(w, 2, 256, 1.0, 0);
            w.bs(0x200 | 16 | 8); // 閉じている・膨らみと標高がある
            w.bd(2.0);
            w.bits(1, 2); // 頂点の数（BL の RC）
            w.bytes(&[3]);
            w.bits(1, 2); // 膨らみの数
            w.bytes(&[3]);
            w.rd(0.0);
            w.rd(0.0);
            w.dd(4.0, 0.0);
            w.dd(0.0, 0.0);
            w.dd(4.0, 4.0);
            w.dd(3.0, 0.0);
            for v in [0.0, 0.5, 0.0] {
                w.bd(v);
            }
        },
        entity_handles(0x10),
    );
    let (db, diag) = objects(&dwg(&[(0x10, layer), (0x20, lwpolyline)]));
    assert!(diag.is_empty(), "{:?}", codes_of(&diag));
    let lwpolyline = db.get(0x20).unwrap();
    assert_eq!(lwpolyline.kind, b"LWPOLYLINE");
    // DWG の閉じている印は、DXF の 70 の 1
    assert_eq!(lwpolyline.first(70), Some(&Data::Int(1)));
    assert_eq!(lwpolyline.first(90), Some(&Data::Int(3)));
    for (code, value) in [(43, 0.0), (38, 2.0)] {
        assert!(close(lwpolyline.real(code), value), "{code}");
    }
    let all = |code| -> Vec<&Data> {
        lwpolyline
            .values
            .iter()
            .filter(|v| v.code == code)
            .map(|v| &v.data)
            .collect()
    };
    assert_eq!(
        all(10),
        [&Data::Real(0.0), &Data::Real(4.0), &Data::Real(4.0)]
    );
    assert_eq!(
        all(20),
        [&Data::Real(0.0), &Data::Real(0.0), &Data::Real(3.0)]
    );
    // 膨らみは 0 でないものだけ
    assert_eq!(all(42), [&Data::Real(0.5)]);
}

#[test]
fn splines_have_their_knots_or_fit_points() {
    let layer = object(0x33, 0x10, record_named(b"0"), record_handles(2));
    let by_knots = object(
        0x24,
        0x20,
        |w| {
            entity_common(w, 2, 256, 1.0, 0);
            w.bits(1, 2); // 場合 1（BL の RC）
            w.bytes(&[1]);
            w.bits(1, 2); // 次数 1
            w.bytes(&[1]);
            w.bit(false);
            w.bit(false);
            w.bit(false);
            w.bd(1e-9);
            w.bd(1e-10);
            w.bits(1, 2); // 節点 4 つ
            w.bytes(&[4]);
            w.bits(1, 2); // 制御点 2 つ
            w.bytes(&[2]);
            w.bit(true); // 重みがある
            for v in [0.0, 0.0, 1.0, 1.0] {
                w.bd(v);
            }
            for v in [0.0, 0.0, 0.0, 1.0, 2.0, 3.0, 0.0, 0.5] {
                w.bd(v);
            }
        },
        entity_handles(0x10),
    );
    let by_fit = object(
        0x24,
        0x21,
        |w| {
            entity_common(w, 2, 256, 1.0, 0);
            w.bits(1, 2); // 場合 2
            w.bytes(&[2]);
            w.bits(1, 2); // 次数 3
            w.bytes(&[3]);
            w.bd(0.25);
            for v in [1.0, 0.0, 0.0, 0.0, 0.0, 0.0] {
                w.bd(v);
            }
            w.bits(1, 2); // 当てはめる点 2 つ
            w.bytes(&[2]);
            for v in [0.0, 0.0, 0.0, 5.0, 6.0, 0.0] {
                w.bd(v);
            }
        },
        entity_handles(0x10),
    );
    let (db, diag) = objects(&dwg(&[(0x10, layer), (0x20, by_knots), (0x21, by_fit)]));
    assert!(diag.is_empty(), "{:?}", codes_of(&diag));

    let spline = db.get(0x20).unwrap();
    assert_eq!(spline.kind, b"SPLINE");
    for (code, value) in [(71, 1), (72, 4), (73, 2), (74, 0)] {
        assert_eq!(spline.first(code), Some(&Data::Int(value)), "{code}");
    }
    let count =
        |object: &zumen_acad::Object, code| object.values.iter().filter(|v| v.code == code).count();
    assert_eq!(count(spline, 40), 4);
    assert_eq!(count(spline, 10), 2);
    // 重みは制御点ごと
    let weights: Vec<_> = spline.values.iter().filter(|v| v.code == 41).collect();
    assert_eq!(weights.len(), 2);
    assert_eq!(weights[1].data, Data::Real(0.5));
    // DXF を書くときに決まる印と法線は置かない
    assert_eq!(spline.first(70), None);
    assert_eq!(spline.first(210), None);

    let spline = db.get(0x21).unwrap();
    assert_eq!(spline.first(74), Some(&Data::Int(2)));
    assert!(close(spline.real(44), 0.25));
    // 始まりの接線は 0 でないので置き、終わりの接線は 0 なので置かない
    assert!(close(spline.real(12), 1.0));
    assert_eq!(spline.first(13), None);
    assert_eq!(count(spline, 11), 2);
    assert!(close(spline.real(11), 0.0));
}

#[test]
fn polylines_and_vertices_have_their_fields() {
    let layer = object(0x33, 0x10, record_named(b"0"), record_handles(2));
    let entity = |type_code, handle, data: &dyn Fn(&mut BitWriter)| {
        object(
            type_code,
            handle,
            |w| {
                entity_common(w, 2, 256, 1.0, 0);
                data(w);
            },
            entity_handles(0x10),
        )
    };
    let polyline2d = entity(0x0F, 0x20, &|w| {
        w.bs(1); // 閉じている
        w.bs(0);
        w.bd(0.5);
        w.bd(0.5);
        w.bit(true); // 厚さは 0
        w.bd(2.0); // 標高
        w.bit(true); // 押し出しの向きは既定
    });
    let vertex2d = entity(0x0A, 0x21, &|w| {
        w.bytes(&[0]);
        for v in [1.0, 2.0, 0.0] {
            w.bd(v);
        }
        w.bd(-0.25); // 負なので、終わりの幅は書かれない
        w.bd(0.5);
        w.bd(std::f64::consts::FRAC_PI_2);
    });
    let polyline3d = entity(0x10, 0x22, &|w| w.bytes(&[1, 1]));
    let polyface = entity(0x1D, 0x23, &|w| {
        w.bs(4);
        w.bs(2);
    });
    let face = entity(0x0E, 0x24, &|w| {
        for index in [1, 2, -3, 0_i16] {
            w.bs(index as u16);
        }
    });
    let mesh = entity(0x1E, 0x25, &|w| {
        for v in [16, 0, 3, 2, 0, 0] {
            w.bs(v);
        }
    });
    let (db, diag) = objects(&dwg(&[
        (0x10, layer),
        (0x20, polyline2d),
        (0x21, vertex2d),
        (0x22, polyline3d),
        (0x23, polyface),
        (0x24, face),
        (0x25, mesh),
    ]));
    assert!(diag.is_empty(), "{:?}", codes_of(&diag));
    let ints = |handle, codes: &[i16]| -> Vec<Option<i64>> {
        let object = db.get(handle).unwrap();
        codes
            .iter()
            .map(|&code| match object.first(code) {
                Some(Data::Int(i)) => Some(*i),
                _ => None,
            })
            .collect()
    };
    let polyline = db.get(0x20).unwrap();
    assert_eq!(polyline.kind, b"POLYLINE");
    assert_eq!(ints(0x20, &[66, 70, 75]), [Some(1), Some(1), None]);
    for (code, value) in [(40, 0.5), (41, 0.5), (10, 0.0), (30, 2.0)] {
        assert!(close(polyline.real(code), value), "{code}");
    }
    let vertex = db.get(0x21).unwrap();
    assert_eq!(vertex.kind, b"VERTEX");
    for (code, value) in [
        (10, 1.0),
        (20, 2.0),
        (40, 0.25),
        (41, 0.25),
        (42, 0.5),
        (50, 90.0),
    ] {
        assert!(close(vertex.real(code), value), "{code}");
    }
    // 3D の印 8・閉じている 1・曲線の 4 と、曲線の種類 5
    assert_eq!(ints(0x22, &[70, 75]), [Some(13), Some(5)]);
    assert_eq!(ints(0x23, &[70, 71, 72]), [Some(64), Some(4), Some(2)]);
    // 負の番号は見えない辺。0 の番号は置かない
    assert_eq!(
        ints(0x24, &[70, 71, 72, 73, 74]),
        [Some(128), Some(1), Some(2), Some(-3), None]
    );
    assert_eq!(
        ints(0x25, &[70, 75, 71, 72, 73]),
        [Some(16), None, Some(3), Some(2), Some(0)]
    );
}

#[test]
fn dimensions_have_their_points_and_records() {
    let layer = object(0x33, 0x10, record_named(b"0"), record_handles(2));
    let style = object(0x45, 0x11, record_named(b"ISO-25"), record_handles(3));
    let block = object(0x31, 0x12, record_named(b"*D1"), record_handles(1));
    // 寸法の共通の欄。標高は 1.5、文字の中点は (5, 6)、測った値は 10
    let common = |w: &mut BitWriter, flags: u8, text: &[u8]| {
        for v in [0.0, 0.0, 1.0] {
            w.bd(v);
        }
        w.rd(5.0);
        w.rd(6.0);
        w.bd(1.5);
        w.bytes(&[flags]);
        if text.is_empty() {
            w.bs(0);
        } else {
            w.bs(text.len() as u16 + 1);
            w.bytes(text);
            w.bytes(&[0]);
        }
        for v in [0.0, 0.0, 1.0, 1.0, 1.0, 0.0] {
            w.bd(v);
        }
        w.bs(5);
        w.bs(1);
        w.bd(1.0);
        w.bd(10.0);
        w.rd(0.0);
        w.rd(0.0);
    };
    let handles = |w: &mut BitWriter| {
        entity_handles(0x10)(w);
        w.handle(5, 0x11);
        w.handle(5, 0x12);
    };
    let linear = object(
        0x15,
        0x20,
        |w| {
            entity_common(w, 2, 256, 1.0, 0);
            common(w, 3, b"<> mm");
            for v in [0.0, 0.0, 0.0, 10.0, 0.0, 0.0, 10.0, 4.0, 0.0] {
                w.bd(v);
            }
            w.bd(0.0);
            w.bd(std::f64::consts::FRAC_PI_6);
        },
        handles,
    );
    let diameter = object(
        0x1A,
        0x21,
        |w| {
            entity_common(w, 2, 256, 1.0, 0);
            common(w, 2, b"");
            for v in [-5.0, 0.0, 0.0, 5.0, 0.0, 0.0] {
                w.bd(v);
            }
            w.bd(0.0);
        },
        handles,
    );
    let (db, diag) = objects(&dwg(&[
        (0x10, layer),
        (0x11, style),
        (0x12, block),
        (0x20, linear),
        (0x21, diameter),
    ]));
    assert!(diag.is_empty(), "{:?}", codes_of(&diag));
    let linear = db.get(0x20).unwrap();
    assert_eq!(linear.kind, b"DIMENSION");
    // 線形の寸法は種類 0 で、ブロックだけを使う印の 32
    assert_eq!(linear.first(70), Some(&Data::Int(32)));
    assert_eq!(linear.first(71), Some(&Data::Int(5)));
    assert_eq!(linear.first(3), Some(&Data::Text(b"ISO-25".to_vec())));
    assert_eq!(linear.first(2), Some(&Data::Text(b"*D1".to_vec())));
    assert_eq!(linear.first(1), Some(&Data::Text(b"<> mm".to_vec())));
    for (code, value) in [
        (10, 10.0),
        (20, 4.0),
        (11, 5.0),
        (21, 6.0),
        (31, 1.5),
        (13, 0.0),
        (14, 10.0),
        (42, 10.0),
        (50, 30.0),
    ] {
        assert!(close(linear.real(code), value), "{code}");
    }
    // 既定の値の欄と、0 の 12 の点は置かない
    for code in [12, 41, 72, 52, 53, 51, 210] {
        assert!(linear.first(code).is_none(), "{code}");
    }
    let diameter = db.get(0x21).unwrap();
    // 直径の種類 3 に、文字の位置を利用者が決めた 128 とブロックだけを使う 32 を足す
    assert_eq!(diameter.first(70), Some(&Data::Int(163)));
    assert!(diameter.first(1).is_none());
    for (code, value) in [(15, -5.0), (10, 5.0), (40, 0.0)] {
        assert!(close(diameter.real(code), value), "{code}");
    }
}

#[test]
fn attributes_and_tolerances_have_their_fields() {
    let layer = object(0x33, 0x10, record_named(b"0"), record_handles(2));
    let standard = object(0x35, 0x11, record_named(b"Standard"), record_handles(3));
    let dimstyle = object(0x45, 0x12, record_named(b"ISO-25"), record_handles(3));
    // TEXT の欄。DataFlags が 0x7F で、縦の合わせ（DXF の 74）だけを持つ
    let text = |w: &mut BitWriter, value: &[u8]| {
        entity_common(w, 2, 256, 1.0, 0);
        w.bytes(&[0x7F]);
        w.rd(1.0);
        w.rd(2.0);
        w.bit(true);
        w.bit(true);
        w.rd(2.5);
        w.bs(value.len() as u16 + 1);
        w.bytes(value);
        w.bytes(&[0]);
        w.bs(2);
    };
    let attrib = object(
        0x02,
        0x20,
        |w| {
            text(w, b"A^B");
            w.bs(4);
            w.bytes(b"TAG\0");
            w.bs(0);
            w.bytes(&[1]);
        },
        entity_handles_to(0x10, 0x11),
    );
    let attdef = object(
        0x03,
        0x21,
        |w| {
            text(w, b"");
            w.bs(4);
            w.bytes(b"TAG\0");
            w.bs(8);
            w.bytes(&[2]);
            w.bs(5);
            w.bytes(b"Name\0");
        },
        entity_handles_to(0x10, 0x11),
    );
    let tolerance = object(
        0x2E,
        0x22,
        |w| {
            entity_common(w, 2, 256, 1.0, 0);
            for v in [3.0, 4.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0] {
                w.bd(v);
            }
            w.bs(7);
            w.bytes(b"{\\Fgd\n\0");
        },
        |w| {
            entity_handles(0x10)(w);
            w.handle(5, 0x12);
        },
    );
    let (db, diag) = objects(&dwg(&[
        (0x10, layer),
        (0x11, standard),
        (0x12, dimstyle),
        (0x20, attrib),
        (0x21, attdef),
        (0x22, tolerance),
    ]));
    assert!(diag.is_empty(), "{:?}", codes_of(&diag));
    let attrib = db.get(0x20).unwrap();
    assert_eq!(attrib.kind, b"ATTRIB");
    // `^` は DXF と同じく `^ ` と書く
    assert_eq!(attrib.first(1), Some(&Data::Text(b"A^ B".to_vec())));
    assert_eq!(attrib.first(2), Some(&Data::Text(b"TAG".to_vec())));
    assert_eq!(attrib.first(70), Some(&Data::Int(1)));
    assert_eq!(attrib.first(74), Some(&Data::Int(2)));
    assert!(attrib.first(73).is_none());
    let attdef = db.get(0x21).unwrap();
    assert_eq!(attdef.kind, b"ATTDEF");
    assert_eq!(attdef.first(3), Some(&Data::Text(b"Name".to_vec())));
    assert_eq!(attdef.first(73), Some(&Data::Int(8)));
    assert_eq!(attdef.first(70), Some(&Data::Int(2)));
    let tolerance = db.get(0x22).unwrap();
    assert_eq!(tolerance.kind, b"TOLERANCE");
    // 改行は DXF と同じく `^J` と書く
    assert_eq!(tolerance.first(1), Some(&Data::Text(b"{\\Fgd^J".to_vec())));
    assert_eq!(tolerance.first(3), Some(&Data::Text(b"ISO-25".to_vec())));
    for (code, value) in [(10, 3.0), (20, 4.0), (30, 0.0)] {
        assert!(close(tolerance.real(code), value), "{code}");
    }
    for code in [11, 210] {
        assert!(tolerance.first(code).is_none(), "{code}");
    }
}

#[test]
fn leaders_and_mlines_have_their_fields() {
    let layer = object(0x33, 0x10, record_named(b"0"), record_handles(2));
    let dimstyle = object(0x45, 0x11, record_named(b"ISO-25"), record_handles(3));
    // 2 本の線を持つ多重線の体裁。DWG の印 1 と 32 は、DXF の 2 と 64
    let style = object(
        0x49,
        0x31,
        |w| {
            w.bl_zero();
            w.bs(9);
            w.bytes(b"STANDARD\0");
            w.bs(0);
            w.bs(33);
            w.bs(256);
            w.bd(std::f64::consts::FRAC_PI_2);
            w.bd(std::f64::consts::FRAC_PI_2);
            w.bytes(&[2]);
            for offset in [0.5, -0.5] {
                w.bd(offset);
                w.bs(1);
                w.bs(0);
            }
        },
        |w| w.handle(4, 0x30),
    );
    // 注釈が MTEXT（0）で、2 つの点を持つ引出線
    let leader = object(
        0x2D,
        0x20,
        |w| {
            entity_common(w, 2, 256, 1.0, 0);
            w.bit(false);
            w.bs(0);
            w.bs(0);
            w.bs(2); // 点の数（BL の短い形は BS と同じ）
            for v in [0.0, 0.0, 0.0, 5.0, 5.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0] {
                w.bd(v);
            }
            for v in [1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0] {
                w.bd(v);
            }
            w.bd(2.5);
            w.bd(10.0);
            w.bit(true);
            w.bit(true);
            w.bs(0);
            w.bit(false);
            w.bit(false);
        },
        |w| {
            entity_handles(0x10)(w);
            w.handle(5, 0x30);
            w.handle(5, 0x11);
        },
    );
    // 2 つの頂点と 1 本の線を持つ多重線
    let mline = object(
        0x2F,
        0x21,
        |w| {
            entity_common(w, 2, 256, 1.0, 0);
            w.bd(1.5);
            w.bytes(&[1]);
            for v in [0.0, 0.0, 0.0, 0.0, 0.0, 1.0] {
                w.bd(v);
            }
            w.bs(1);
            w.bytes(&[1]);
            w.bs(2);
            for x in [0.0, 10.0] {
                for v in [x, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0] {
                    w.bd(v);
                }
                w.bs(2);
                w.bd(0.5);
                w.bd(0.0);
                w.bs(0);
            }
        },
        |w| {
            entity_handles(0x10)(w);
            w.handle(5, 0x31);
        },
    );
    let (db, diag) = objects(&dwg(&[
        (0x10, layer),
        (0x11, dimstyle),
        (0x20, leader),
        (0x21, mline),
        (0x31, style),
    ]));
    assert!(diag.is_empty(), "{:?}", codes_of(&diag));
    let leader = db.get(0x20).unwrap();
    assert_eq!(leader.kind, b"LEADER");
    assert_eq!(leader.first(73), Some(&Data::Int(0)));
    assert_eq!(leader.first(74), Some(&Data::Int(1)));
    assert_eq!(leader.first(340), Some(&Data::Handle(0x30)));
    assert_eq!(leader.first(3), Some(&Data::Text(b"ISO-25".to_vec())));
    for (code, value) in [(40, 2.5), (41, 10.0)] {
        assert!(close(leader.real(code), value), "{code}");
    }
    // 点は DXF が鉤の線の点を足すので置かない。既定の値の欄も置かない
    for code in [10, 76, 72, 210, 211] {
        assert!(leader.first(code).is_none(), "{code}");
    }
    let mline = db.get(0x21).unwrap();
    assert_eq!(mline.kind, b"MLINE");
    assert_eq!(mline.first(340), Some(&Data::Handle(0x31)));
    assert_eq!(mline.first(2), Some(&Data::Text(b"STANDARD".to_vec())));
    for (code, value) in [(70, 1), (71, 1), (72, 2), (73, 1)] {
        assert_eq!(mline.first(code), Some(&Data::Int(value)), "{code}");
    }
    let xs: Vec<_> = mline.values.iter().filter(|v| v.code == 11).collect();
    assert_eq!(xs.len(), 2);
    assert_eq!(xs[1].data, Data::Real(10.0));
    let params: Vec<_> = mline.values.iter().filter(|v| v.code == 41).collect();
    assert_eq!(params.len(), 4);
    assert!(close(mline.real(40), 1.5));
    let style = db.get(0x31).unwrap();
    assert_eq!(style.kind, b"MLINESTYLE");
    assert_eq!(style.first(2), Some(&Data::Text(b"STANDARD".to_vec())));
    assert_eq!(style.first(70), Some(&Data::Int(66)));
    assert_eq!(style.first(71), Some(&Data::Int(2)));
    assert!(close(style.real(51), 90.0));
    let colors: Vec<_> = style.values.iter().filter(|v| v.code == 62).collect();
    assert_eq!(colors.len(), 3);
    assert_eq!(colors[1].data, Data::Int(1));
    let offsets: Vec<_> = style.values.iter().filter(|v| v.code == 49).collect();
    assert_eq!(offsets[1].data, Data::Real(-0.5));
}

#[test]
fn viewports_have_their_view_and_ucs() {
    let layer = object(0x33, 0x10, record_named(b"0"), record_handles(2));
    let viewport = object(
        0x22,
        0x20,
        |w| {
            entity_common(w, 2, 256, 1.0, 0);
            for v in [5.0, 4.0, 0.0, 8.0, 6.0] {
                w.bd(v); // 中心・幅・高さ
            }
            for v in [1.0, 2.0, 3.0, 0.0, 0.0, 1.0] {
                w.bd(v); // ビューの目標と向き
            }
            for v in [std::f64::consts::FRAC_PI_2, 9.0, 50.0, 0.0, 0.0, 0.0] {
                w.bd(v); // ねじれ・ビューの高さ・レンズ・前後の切り取り・スナップの角度
            }
            for v in [6.0, 4.5, 0.0, 0.0, 0.5, 0.5, 0.5, 0.5] {
                w.rd(v);
            }
            w.bs(1000);
            w.bl_zero(); // 凍結した画層の数
            w.bl_zero(); // 状態
            w.bs(0);
            w.bytes(&[0]);
            w.bit(false);
            w.bit(true);
            for v in [0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0] {
                w.bd(v);
            }
            w.bs(0);
        },
        entity_handles(0x10),
    );
    let (db, diag) = objects(&dwg(&[(0x10, layer), (0x20, viewport)]));
    assert!(diag.is_empty(), "{:?}", codes_of(&diag));
    let viewport = db.get(0x20).unwrap();
    assert_eq!(viewport.kind, b"VIEWPORT");
    for (code, value) in [
        (10, 5.0),
        (40, 8.0),
        (41, 6.0),
        (12, 6.0),
        (22, 4.5),
        (17, 1.0),
        (37, 3.0),
        (16, 0.0),
        (36, 1.0),
        (42, 50.0),
        (45, 9.0),
        (51, 90.0),
        (111, 1.0),
        (122, 1.0),
    ] {
        assert!(close(viewport.real(code), value), "{code}");
    }
    assert_eq!(viewport.first(72), Some(&Data::Int(1000)));
    assert_eq!(viewport.first(71), Some(&Data::Int(1)));
    assert_eq!(viewport.first(74), Some(&Data::Int(0)));
    assert_eq!(viewport.first(1), Some(&Data::Text(Vec::new())));
    // 番号と状態は DWG に記録されていない
    for code in [68, 69] {
        assert!(viewport.first(code).is_none(), "{code}");
    }
}

#[test]
fn types_of_classes_are_named_by_the_classes() {
    let layer = object(0x33, 0x10, record_named(b"0"), record_handles(2));
    let table = object(
        500,
        0x20,
        |w| entity_common(w, 2, 1, 1.0, 0),
        entity_handles(0x10),
    );
    let var = object(
        501,
        0x21,
        |w| {
            w.bl_zero();
            w.bytes(&[0]);
            w.bs(0);
        },
        |w| w.handle(4, 0),
    );
    let src = with_classes(
        dwg(&[(0x10, layer), (0x20, table), (0x21, var)]),
        &[(500, b"ACAD_TABLE", true), (501, b"DICTIONARYVAR", false)],
    );
    let (file, diag) = read(&src, Limits::default());
    assert!(diag.is_empty(), "{:?}", codes_of(&diag));
    assert_eq!(file.unwrap().record(1).unwrap().kind, b"ACAD_TABLE");

    let (db, diag) = objects(&src);
    assert!(diag.is_empty(), "{:?}", codes_of(&diag));
    let table = db.get(0x20).unwrap();
    assert_eq!(table.kind, b"ACAD_TABLE");
    assert_eq!(table.section, Section::Entities);
    // 図形を作るクラスなので、図形の共通の欄を読む
    assert_eq!(table.first(8), Some(&Data::Text(b"0".to_vec())));
    assert_eq!(table.first(62), Some(&Data::Int(1)));
    let var = db.get(0x21).unwrap();
    assert_eq!(var.kind, b"DICTIONARYVAR");
    assert_eq!(var.section, Section::Objects);
}

#[test]
fn dictionaries_and_xrecords_have_their_entries_and_data() {
    let text = |w: &mut BitWriter, text: &[u8]| {
        w.bs(text.len() as u16 + 1);
        w.bytes(text);
        w.bytes(&[0]);
    };
    // 所有する辞書。項目は XRECORD を指す
    let dictionary = object(
        0x2A,
        0x20,
        |w| {
            w.bl_zero();
            w.bits(1, 2); // 項目の数（BL の RC）
            w.bytes(&[1]);
            w.bs(1); // 複製の扱い
            w.bytes(&[1]); // 所有する
            text(w, b"MY_DATA");
        },
        |w| {
            w.handle(4, 0xC); // 所有者
            w.handle(3, 0); // 拡張辞書
            w.handle(2, 0x21);
        },
    );
    let mut data = BitWriter::default();
    let rs = |w: &mut BitWriter, v: i16| w.bytes(&v.to_le_bytes());
    rs(&mut data, 1);
    rs(&mut data, 3);
    data.bytes(&[30]); // コード・ページ
    data.bytes(b"a\nb");
    rs(&mut data, 10);
    for v in [1.0, 2.0, 3.0] {
        data.rd(v);
    }
    rs(&mut data, 40);
    data.rd(0.5);
    rs(&mut data, 70);
    rs(&mut data, -2);
    rs(&mut data, 90);
    data.rl(70000);
    rs(&mut data, 160);
    data.bytes(&(1i64 << 40).to_le_bytes());
    rs(&mut data, 280);
    data.bytes(&[7]);
    rs(&mut data, 290);
    data.bytes(&[1]);
    rs(&mut data, 310);
    data.bytes(&[2, 0xAB, 0xCD]);
    rs(&mut data, 340);
    data.bytes(&0x20u64.to_le_bytes());
    let xrecord = object(
        500,
        0x21,
        move |w| {
            w.bl_zero();
            w.bits(0, 2); // データのバイト数（BL の RL）
            w.rl((data.bits.len() / 8) as u32);
            w.bits.extend(&data.bits);
            w.bs(1); // 複製の扱い
        },
        |w| {
            w.handle(4, 0x20);
            w.handle(3, 0);
        },
    );
    let src = with_classes(
        dwg(&[(0x20, dictionary), (0x21, xrecord)]),
        &[(500, b"XRECORD", false)],
    );
    let (db, diag) = objects(&src);
    assert!(diag.is_empty(), "{:?}", codes_of(&diag));

    let dictionary = db.get(0x20).unwrap();
    assert_eq!(dictionary.kind, b"DICTIONARY");
    let values: Vec<_> = dictionary
        .values
        .iter()
        .map(|v| (v.code, &v.data))
        .collect();
    assert_eq!(
        values[2..],
        [
            (280, &Data::Int(1)),
            (281, &Data::Int(1)),
            (3, &Data::Text(b"MY_DATA".to_vec())),
            (360, &Data::Handle(0x21)),
        ]
    );

    let xrecord = db.get(0x21).unwrap();
    assert_eq!(xrecord.kind, b"XRECORD");
    assert_eq!(xrecord.first(330), Some(&Data::Handle(0x20)));
    let values: Vec<_> = xrecord.values.iter().map(|v| (v.code, &v.data)).collect();
    assert_eq!(
        values[2..],
        [
            (280, &Data::Int(1)),
            (1, &Data::Text(b"a^Jb".to_vec())),
            (10, &Data::Real(1.0)),
            (20, &Data::Real(2.0)),
            (30, &Data::Real(3.0)),
            (40, &Data::Real(0.5)),
            (70, &Data::Int(-2)),
            (90, &Data::Int(70000)),
            (160, &Data::Int(1 << 40)),
            (280, &Data::Int(7)),
            (290, &Data::Bool(true)),
            (310, &Data::Binary(vec![0xAB, 0xCD])),
            (340, &Data::Handle(0x20)),
        ]
    );
}

#[test]
fn groups_layouts_and_scales_have_their_fields() {
    let text = |w: &mut BitWriter, text: &[u8]| {
        w.bs(text.len() as u16 + 1);
        w.bytes(text);
        w.bytes(&[0]);
    };
    let group = object(
        0x48,
        0x20,
        |w| {
            w.bl_zero();
            text(w, b"two lines");
            w.bs(0); // 名前を持つ
            w.bs(1); // 選べる
            w.bits(1, 2); // 図形の数（BL の RC）
            w.bytes(&[2]);
        },
        |w| {
            w.handle(4, 0xC);
            w.handle(3, 0);
            w.handle(5, 0x30);
            w.handle(5, 0x31);
        },
    );
    // 既定の項目を持つ辞書
    let defaulted = object(
        500,
        0x21,
        |w| {
            w.bl_zero();
            w.bits(1, 2);
            w.bytes(&[1]);
            w.bs(1);
            w.bytes(&[0]);
            text(w, b"Normal");
        },
        |w| {
            w.handle(4, 0xC);
            w.handle(3, 0);
            w.handle(2, 0x22);
            w.handle(5, 0x22);
        },
    );
    let scale = object(
        501,
        0x22,
        |w| {
            w.bl_zero();
            w.bs(0);
            text(w, b"1:2");
            w.bd(1.0);
            w.bd(2.0);
            w.bit(false);
        },
        |w| {
            w.handle(4, 0x21);
            w.handle(3, 0);
        },
    );
    let layout = object(
        502,
        0x23,
        |w| {
            w.bl_zero();
            text(w, b"");
            text(w, b"none_device");
            w.bs(688);
            for v in [6.35, 6.35, 6.35, 6.35, 215.9, 279.4] {
                w.bd(v);
            }
            text(w, b"Letter");
            w.bd(1.0);
            w.bd(2.0);
            for v in [0, 1, 5] {
                w.bs(v);
            }
            for v in [3.0, 4.0, 5.0, 6.0] {
                w.bd(v);
            }
            text(w, b"");
            w.bd(1.0);
            w.bd(1.0);
            text(w, b"mono.ctb");
            w.bs(16);
            w.bd(1.0);
            w.bd(0.0);
            w.bd(0.0);
            // レイアウトの欄
            text(w, b"Layout1");
            w.bs(1); // タブの順
            w.bs(1);
            for v in [0.0, 0.0, 0.0] {
                w.bd(v);
            }
            for v in [-0.25, -0.25, 10.75, 8.25] {
                w.rd(v);
            }
            for v in [0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0] {
                w.bd(v);
            }
            w.bs(0);
            for v in [1.0, 0.5, 0.0, 9.5, 7.0, 0.0] {
                w.bd(v);
            }
        },
        |w| {
            w.handle(4, 0xC);
            w.handle(3, 0);
            w.handle(4, 0x40); // 紙空間のブロックのレコード
            w.handle(4, 0x41); // 最後に使ったビューポート
            w.handle(5, 0); // 基準の UCS
            w.handle(5, 0); // 名前の UCS
        },
    );
    let src = with_classes(
        dwg(&[
            (0x20, group),
            (0x21, defaulted),
            (0x22, scale),
            (0x23, layout),
        ]),
        &[
            (500, b"ACDBDICTIONARYWDFLT", false),
            (501, b"SCALE", false),
            (502, b"LAYOUT", false),
        ],
    );
    let (db, diag) = objects(&src);
    assert!(diag.is_empty(), "{:?}", codes_of(&diag));
    let values = |handle: u64| -> Vec<(i16, Data)> {
        let object = db.get(handle).unwrap();
        object.values[2..]
            .iter()
            .map(|v| (v.code, v.data.clone()))
            .collect()
    };
    let text = |text: &[u8]| Data::Text(text.to_vec());

    assert_eq!(db.get(0x20).unwrap().kind, b"GROUP");
    assert_eq!(
        values(0x20),
        [
            (300, text(b"two lines")),
            (70, Data::Int(0)),
            (71, Data::Int(1)),
            (340, Data::Handle(0x30)),
            (340, Data::Handle(0x31)),
        ]
    );
    assert_eq!(
        values(0x21),
        [
            (281, Data::Int(1)),
            (3, text(b"Normal")),
            (350, Data::Handle(0x22)),
            (340, Data::Handle(0x22)),
        ]
    );
    assert_eq!(
        values(0x22),
        [
            (70, Data::Int(0)),
            (300, text(b"1:2")),
            (140, Data::Real(1.0)),
            (141, Data::Real(2.0)),
            (290, Data::Bool(false)),
        ]
    );
    let layout = db.get(0x23).unwrap();
    assert_eq!(layout.kind, b"LAYOUT");
    let firsts: Vec<_> = [2, 4, 7, 48, 141, 75]
        .map(|code| layout.first(code).cloned())
        .into_iter()
        .collect();
    assert_eq!(
        firsts,
        [
            Some(text(b"none_device")),
            Some(text(b"Letter")),
            Some(text(b"mono.ctb")),
            Some(Data::Real(3.0)),
            Some(Data::Real(6.0)),
            Some(Data::Int(16)),
        ]
    );
    // 印刷の設定とレイアウトは、名前（1）と印（70）を順に持つ
    let all = |code: i16| -> Vec<&Data> {
        layout
            .values
            .iter()
            .filter(|v| v.code == code)
            .map(|v| &v.data)
            .collect()
    };
    assert_eq!(all(1), [&text(b""), &text(b"Layout1")]);
    assert_eq!(all(70), [&Data::Int(688), &Data::Int(1)]);
    assert_eq!(all(11), [&Data::Real(10.75)]);
    assert_eq!(all(15), [&Data::Real(9.5)]);
    assert_eq!(all(330), [&Data::Handle(0xC), &Data::Handle(0x40)]);
    assert_eq!(all(331), [&Data::Handle(0x41)]);
    assert!(all(345).is_empty() && all(346).is_empty());
}

#[test]
fn xrecord_data_that_overruns_its_length_is_not_read() {
    let xrecord = object(
        500,
        0x21,
        |w| {
            w.bl_zero();
            w.bits(1, 2);
            w.bytes(&[3]); // 3 バイトと記録するが、値は 10 バイト
            w.bytes(&40i16.to_le_bytes());
            w.rd(0.5);
            w.bs(1);
        },
        |w| {
            w.handle(4, 0xC);
            w.handle(3, 0);
        },
    );
    let src = with_classes(dwg(&[(0x21, xrecord)]), &[(500, b"XRECORD", false)]);
    let (db, diag) = objects(&src);
    assert!(!diag.is_empty());
    assert_eq!(db.get(0x21).unwrap().first(40), None);
}

#[test]
fn a_wrong_class_section_is_reported() {
    let src = with_classes(dwg(&objects_in_sample()), &[(500, b"HATCH", true)]);
    let end = src.len();
    // CRC が合わないときは、知らせて読み続ける
    let mut wrong_crc = src.clone();
    wrong_crc[end - 17] ^= 0xFF;
    let (result, diag) = read(&wrong_crc, Limits::default());
    assert!(result.is_ok());
    assert_eq!(codes_of(&diag), [codes::DWG_CRC_INVALID]);
    // 終わりの印が合わないときは止める
    let mut wrong_sentinel = src.clone();
    wrong_sentinel[end - 1] ^= 0xFF;
    let (result, diag) = read(&wrong_sentinel, Limits::default());
    assert!(matches!(result, Err(Error::Invalid)));
    assert!(codes_of(&diag).contains(&codes::DWG_FILE_INVALID));
    // 途中で尽きるファイルも、パニックせずに失敗する
    for len in end - 60..end {
        let (result, _) = read(&src[..len], Limits::default());
        assert!(result.is_err(), "{len}");
    }
}

#[test]
fn hatches_have_their_paths_and_pattern() {
    let bl = |w: &mut BitWriter, n: u8| {
        w.bits(1, 2);
        w.bytes(&[n]);
    };
    let text = |w: &mut BitWriter, text: &[u8]| {
        w.bs(text.len() as u16 + 1);
        w.bytes(text);
        w.bytes(&[0]);
    };
    let layer = object(0x33, 0x10, record_named(b"0"), record_handles(2));
    let pattern = object(
        500,
        0x20,
        |w| {
            entity_common(w, 2, 256, 1.0, 0);
            w.bd(2.0); // 標高
            for v in [0.0, 0.0, 1.0] {
                w.bd(v);
            }
            text(w, b"ANSI31");
            w.bit(false); // 塗りつぶしでない
            w.bit(true);
            bl(w, 2);
            // 辺で囲む境界。印 4 は画素の大きさを持たせる
            bl(w, 4);
            bl(w, 2);
            w.bytes(&[2]); // 円弧
            w.rd(1.0);
            w.rd(2.0);
            w.bd(3.0);
            w.bd(0.0);
            w.bd(std::f64::consts::FRAC_PI_2);
            w.bit(true);
            w.bytes(&[4]); // SPLINE
            bl(w, 3);
            w.bit(true); // 重みがある
            w.bit(false);
            bl(w, 2);
            bl(w, 1);
            w.bd(0.0);
            w.bd(1.0);
            w.rd(5.0);
            w.rd(6.0);
            w.bd(0.5);
            w.bl_zero();
            // 多角形の境界
            bl(w, 2);
            w.bit(true); // 膨らみがある
            w.bit(true);
            bl(w, 2);
            w.rd(0.0);
            w.rd(0.0);
            w.bd(0.25);
            w.rd(1.0);
            w.rd(0.0);
            w.bd(0.0);
            w.bl_zero();
            w.bs(1);
            w.bs(1);
            w.bd(std::f64::consts::FRAC_PI_4);
            w.bd(2.0);
            w.bit(false);
            w.bs(1);
            w.bd(std::f64::consts::FRAC_PI_2);
            for v in [0.1, 0.2, 0.3, 0.4] {
                w.bd(v);
            }
            w.bs(2);
            w.bd(1.0);
            w.bd(-0.5);
            w.bd(0.01); // 画素の大きさ
            bl(w, 1);
            w.rd(7.0);
            w.rd(8.0);
        },
        entity_handles(0x10),
    );
    let solid = object(
        500,
        0x21,
        |w| {
            entity_common(w, 2, 256, 1.0, 0);
            w.bd(0.0);
            for v in [0.0, 0.0, 1.0] {
                w.bd(v);
            }
            text(w, b"SOLID");
            w.bit(true);
            w.bit(false);
            bl(w, 1);
            bl(w, 1);
            bl(w, 1);
            w.bytes(&[1]); // 線分
            for v in [0.0, 0.0, 1.0, 1.0] {
                w.rd(v);
            }
            w.bl_zero();
            w.bs(0);
            w.bs(1);
            w.bl_zero();
        },
        entity_handles(0x10),
    );
    let src = with_classes(
        dwg(&[(0x10, layer), (0x20, pattern), (0x21, solid)]),
        &[(500, b"HATCH", true)],
    );
    let (db, diag) = objects(&src);
    assert!(diag.is_empty(), "{:?}", codes_of(&diag));
    let all = |object: &zumen_acad::Object, code| -> Vec<Data> {
        object
            .values
            .iter()
            .filter(|v| v.code == code)
            .map(|v| v.data.clone())
            .collect()
    };

    let hatch = db.get(0x20).unwrap();
    assert!(close(hatch.real(30), 2.0));
    assert_eq!(hatch.first(210), None);
    assert_eq!(hatch.first(2), Some(&Data::Text(b"ANSI31".to_vec())));
    assert_eq!(all(hatch, 92), [Data::Int(4), Data::Int(2)]);
    assert_eq!(all(hatch, 93), [Data::Int(2), Data::Int(2)]);
    assert_eq!(all(hatch, 72), [Data::Int(2), Data::Int(4), Data::Int(1)]);
    // 角度は度に直す
    assert!(close(hatch.real(51), 90.0));
    assert!(close(hatch.real(52), 45.0));
    assert!(close(hatch.real(53), 90.0));
    assert_eq!(hatch.first(94), Some(&Data::Int(3)));
    // SPLINE の辺の重みと、多角形の膨らみは、どちらも 42
    assert_eq!(
        all(hatch, 42),
        [Data::Real(0.5), Data::Real(0.25), Data::Real(0.0)]
    );
    assert_eq!(all(hatch, 49), [Data::Real(1.0), Data::Real(-0.5)]);
    assert!(close(hatch.real(47), 0.01));
    assert_eq!(hatch.first(98), Some(&Data::Int(1)));

    let hatch = db.get(0x21).unwrap();
    assert_eq!(hatch.first(70), Some(&Data::Int(1)));
    assert!(close(hatch.real(11), 1.0));
    // 塗りつぶしは、模様の欄と画素の大きさを持たない
    assert_eq!(hatch.first(52), None);
    assert_eq!(hatch.first(47), None);
    assert_eq!(hatch.first(98), Some(&Data::Int(0)));
}

#[test]
fn a_missing_record_is_reported() {
    let layer = object(0x33, 0x10, record_named(b"0"), record_handles(2));
    // ブロックのハンドルが、LAYER を指している
    let insert = object(
        0x07,
        0x20,
        |w| {
            entity_common(w, 2, 256, 1.0, 0);
            for v in [0.0, 0.0, 0.0] {
                w.bd(v);
            }
            w.bits(3, 2);
            w.bd(0.0);
            for v in [0.0, 0.0, 1.0] {
                w.bd(v);
            }
            w.bit(false);
        },
        entity_handles_to(0x10, 0x10),
    );
    let (db, diag) = objects(&dwg(&[(0x10, layer), (0x20, insert)]));
    assert_eq!(codes_of(&diag), [codes::DWG_RECORD_MISSING]);
    assert_eq!(db.get(0x20).unwrap().first(2), None);
}

#[test]
fn a_missing_layer_is_reported() {
    let line = object(
        0x13,
        0x20,
        |w| {
            entity_common(w, 2, 256, 1.0, 0);
            line_data(w, [0.0; 3], [1.0, 0.0, 0.0], 0.0);
        },
        entity_handles(0x99),
    );
    let (db, diag) = objects(&dwg(&[(0x20, line)]));
    assert_eq!(codes_of(&diag), [codes::DWG_LAYER_MISSING]);
    let line = db.get(0x20).unwrap();
    assert_eq!(line.first(8), None);
    assert!(close(line.real(11), 1.0));
}

#[test]
fn other_versions_are_not_read_yet() {
    let mut src = sample();
    src[..6].copy_from_slice(b"AC1021");
    let (result, diag) = read(&src, Limits::default());
    assert!(matches!(result, Err(Error::Unsupported)));
    assert_eq!(codes_of(&diag), [codes::DWG_VERSION_UNSUPPORTED]);
    src[..6].copy_from_slice(b"ABCDEF");
    let (result, diag) = read(&src, Limits::default());
    assert!(matches!(result, Err(Error::Invalid)));
    assert_eq!(codes_of(&diag), [codes::DWG_FILE_INVALID]);
}

#[test]
fn a_wrong_crc_is_reported_and_reading_continues() {
    let mut src = sample();
    // コード・ページを変えると、見出しの CRC が合わなくなる
    src[0x13] = 31;
    let (result, diag) = read(&src, Limits::default());
    assert!(result.is_ok());
    assert_eq!(codes_of(&diag), [codes::DWG_CRC_INVALID]);

    // オブジェクトの最後のバイト（CRC）を変える
    let mut src = sample();
    let (file, _) = read(&src, Limits::default());
    let end = file.unwrap().record(0).unwrap().bytes.unwrap().end as usize;
    src[end - 1] ^= 0xFF;
    let (result, diag) = read(&src, Limits::default());
    assert!(result.is_ok());
    assert_eq!(codes_of(&diag), [codes::DWG_CRC_INVALID]);
}

#[test]
fn a_wrong_sentinel_is_invalid() {
    let mut src = sample();
    src[0x19 + 27 + 2] ^= 0xFF;
    let (result, diag) = read(&src, Limits::default());
    assert!(matches!(result, Err(Error::Invalid)));
    assert!(codes_of(&diag).contains(&codes::DWG_FILE_INVALID));
}

#[test]
fn truncated_files_fail_without_panicking() {
    let src = sample();
    for len in 0..src.len() {
        let (result, _) = read(&src[..len], Limits::default());
        assert!(result.is_err(), "{len}");
    }
}

#[test]
fn reading_stops_at_the_record_limit() {
    let (result, diag) = read(
        &sample(),
        Limits::default().with(Kind::Records, Limit::Finite(4)),
    );
    assert!(matches!(result, Err(Error::Stopped(_))));
    assert_eq!(
        diag.iter().last().map(|d| d.code()),
        Some(codes::BUDGET_RECORDS_EXCEEDED)
    );
}

#[test]
fn writing_keeps_the_bytes() {
    let src = sample();
    let (file, _) = read(&src, Limits::default());
    let mut out = Vec::new();
    let mut diag = Diagnostics::new();
    Dwg::write(
        &file.unwrap(),
        &mut out,
        &mut Cx::new(&mut Budget::default(), &mut diag, &CancelToken::new()),
    )
    .unwrap();
    assert_eq!(out, src);
}

#[test]
fn layers_styles_and_linetypes_have_their_fields() {
    let text = |w: &mut BitWriter, text: &[u8]| {
        w.bs(text.len() as u16 + 1);
        w.bytes(text);
        w.bytes(&[0]);
    };
    let common = |w: &mut BitWriter, name: &[u8]| {
        w.bl_zero();
        text(w, name);
        w.bit(false);
        w.bs(0);
        w.bit(false);
    };
    // 凍結・表示しない・ロックで、印刷しない。線の太さの番号 11 は 0.50 mm
    let layer = object(
        0x33,
        0x10,
        |w| {
            common(w, b"WALL");
            w.bs(1 | 2 | 8 | (11 << 5));
            w.bs(3);
        },
        |w| {
            w.handle(4, 2);
            w.handle(3, 0);
            w.handle(5, 0);
            w.handle(5, 0x50); // 印刷スタイル
            w.handle(5, 0x12); // 線種
        },
    );
    let style = object(
        0x35,
        0x11,
        |w| {
            common(w, b"VERT");
            w.bit(false);
            w.bit(true);
            w.bd(2.5);
            w.bd(0.8);
            w.bd(std::f64::consts::FRAC_PI_2);
            w.bytes(&[2]);
            w.bd(3.0);
            text(w, b"txt");
            text(w, b"");
        },
        |w| {
            w.handle(4, 3);
            w.handle(3, 0);
            w.handle(5, 0);
        },
    );
    // 線の要素と、文字 AB の要素
    let ltype = object(
        0x39,
        0x12,
        |w| {
            common(w, b"DASHED");
            text(w, b"__ AB");
            w.bd(1.5);
            w.bytes(&[65, 2]);
            for (dash, x, y, scale, flags) in [(1.0, 0.0, 0.0, 1.0, 0), (-0.5, 0.1, -0.2, 0.5, 2)] {
                w.bd(dash);
                w.bs(0);
                w.rd(x);
                w.rd(y);
                w.bd(scale);
                w.bd(0.0);
                w.bs(flags);
            }
            let mut area = [0; 256];
            area[..2].copy_from_slice(b"AB");
            w.bytes(&area);
        },
        |w| {
            w.handle(4, 5);
            w.handle(3, 0);
            w.handle(5, 0);
            w.handle(5, 0);
            w.handle(5, 0x11);
        },
    );
    let src = dwg(&[(0x10, layer), (0x11, style), (0x12, ltype)]);
    let (db, diag) = objects(&src);
    assert!(diag.is_empty(), "{:?}", codes_of(&diag));
    let values = |handle: u64| -> Vec<(i16, Data)> {
        let object = db.get(handle).unwrap();
        object.values[2..]
            .iter()
            .map(|v| (v.code, v.data.clone()))
            .collect()
    };
    let text = |text: &[u8]| Data::Text(text.to_vec());

    assert_eq!(
        values(0x10),
        [
            (2, text(b"WALL")),
            (70, Data::Int(5)),
            (62, Data::Int(-3)),
            (290, Data::Bool(false)),
            (370, Data::Int(50)),
            (390, Data::Handle(0x50)),
            (6, text(b"DASHED")),
        ]
    );
    assert_eq!(
        values(0x11),
        [
            (2, text(b"VERT")),
            (70, Data::Int(4)),
            (40, Data::Real(2.5)),
            (41, Data::Real(0.8)),
            (50, Data::Real(90.0)),
            (71, Data::Int(2)),
            (42, Data::Real(3.0)),
            (3, text(b"txt")),
            (4, text(b"")),
        ]
    );
    assert_eq!(
        values(0x12),
        [
            (2, text(b"DASHED")),
            (70, Data::Int(0)),
            (3, text(b"__ AB")),
            (72, Data::Int(65)),
            (73, Data::Int(2)),
            (40, Data::Real(1.5)),
            (49, Data::Real(1.0)),
            (74, Data::Int(0)),
            (49, Data::Real(-0.5)),
            (74, Data::Int(2)),
            (75, Data::Int(0)),
            (46, Data::Real(0.5)),
            (50, Data::Real(0.0)),
            (44, Data::Real(0.1)),
            (45, Data::Real(-0.2)),
            (9, text(b"AB")),
            (340, Data::Handle(0x11)),
        ]
    );
}

#[test]
fn views_ucss_vports_and_dimstyles_have_their_fields() {
    let text = |w: &mut BitWriter, text: &[u8]| {
        w.bs(text.len() as u16 + 1);
        w.bytes(text);
        w.bytes(&[0]);
    };
    let common = |w: &mut BitWriter, name: &[u8]| {
        w.bl_zero();
        text(w, name);
        w.bit(false);
        w.bs(0);
        w.bit(false);
    };
    let bd3 = |w: &mut BitWriter, point: [f64; 3]| point.iter().for_each(|&v| w.bd(v));
    let rd2 = |w: &mut BitWriter, x: f64, y: f64| {
        w.rd(x);
        w.rd(y);
    };
    let handles = |owner: u64, rest: &'static [u64]| {
        move |w: &mut BitWriter| {
            w.handle(4, owner);
            w.handle(3, 0);
            w.handle(5, 0);
            for &h in rest {
                w.handle(5, h);
            }
        }
    };
    // 紙空間の、UCS を伴う透視のビュー。後ろで切り取り、前の切り取りは目の位置にない
    let view = object(
        0x3D,
        0x10,
        |w| {
            common(w, b"V1");
            w.bd(10.0);
            w.bd(20.0);
            rd2(w, 1.0, 2.0);
            bd3(w, [3.0, 4.0, 5.0]);
            bd3(w, [0.0, 0.0, 1.0]);
            w.bd(std::f64::consts::PI);
            for v in [50.0, 0.0, 0.0] {
                w.bd(v);
            }
            for b in [true, false, true, false] {
                w.bit(b);
            }
            w.bytes(&[1]);
            w.bit(true);
            w.bit(true);
            bd3(w, [1.0, 1.0, 0.0]);
            bd3(w, [1.0, 0.0, 0.0]);
            bd3(w, [0.0, 1.0, 0.0]);
            w.bd(2.0);
            w.bs(3);
        },
        handles(6, &[0x60, 0x61]),
    );
    let ucs = object(
        0x3F,
        0x11,
        |w| {
            common(w, b"U1");
            bd3(w, [1.0, 2.0, 3.0]);
            bd3(w, [1.0, 0.0, 0.0]);
            bd3(w, [0.0, 1.0, 0.0]);
            w.bd(5.0);
            w.bs(1);
            w.bs(0);
        },
        handles(7, &[0x62, 0x63]),
    );
    let vport = object(
        0x41,
        0x12,
        |w| {
            common(w, b"*Active");
            w.bd(100.0);
            w.bd(250.0);
            rd2(w, 5.0, 6.0);
            bd3(w, [0.0, 0.0, 0.0]);
            bd3(w, [0.0, 0.0, 1.0]);
            for v in [0.0, 50.0, 0.0, 0.0] {
                w.bd(v);
            }
            for b in [false, false, false, true] {
                w.bit(b);
            }
            w.bytes(&[0]);
            rd2(w, 0.0, 0.0);
            rd2(w, 1.0, 1.0);
            w.bit(true); // UCS に従う
            w.bs(1000);
            w.bit(true);
            w.bits(3, 2);
            w.bit(true);
            rd2(w, 10.0, 10.0);
            w.bit(false);
            w.bit(false);
            w.bs(0);
            w.bd(0.0);
            rd2(w, 0.0, 0.0);
            rd2(w, 10.0, 10.0);
            w.bit(false);
            w.bit(true);
            bd3(w, [0.0, 0.0, 0.0]);
            bd3(w, [1.0, 0.0, 0.0]);
            bd3(w, [0.0, 1.0, 0.0]);
            w.bd(0.0);
            w.bs(0);
        },
        handles(8, &[0x64, 0]),
    );
    // 既定の値から変えた欄は、DIMPOST・DIMSCALE・DIMTIH・DIMDEC・DIMLWD だけ
    let dimstyle = object(
        0x45,
        0x13,
        |w| {
            common(w, b"D1");
            text(w, b"<> mm");
            text(w, b"");
            for v in [2.0, 0.18, 0.0625, 0.38, 0.18, 0.0, 0.0, 0.0, 0.0] {
                w.bd(v);
            }
            for b in [false, false, false, true, false, false] {
                w.bit(b);
            }
            for v in [0, 0, 0] {
                w.bs(v);
            }
            for v in [0.18, 0.09, 0.0, 25.4, 1.0, 0.0, 1.0, 0.09, 0.0] {
                w.bd(v);
            }
            w.bit(false);
            w.bs(2);
            for _ in 0..4 {
                w.bit(false);
            }
            for v in [0, 0, 0, 0, 2, 4, 2, 2, 0, 0, 2, 46, 0, 0] {
                w.bs(v);
            }
            w.bit(false);
            w.bit(false);
            for v in [1, 0, 0, 0] {
                w.bs(v);
            }
            w.bit(false);
            for v in [3, 15, 0xFFFE] {
                w.bs(v);
            }
        },
        handles(9, &[0x11, 0, 0x70, 0, 0]),
    );
    let src = dwg(&[(0x10, view), (0x11, ucs), (0x12, vport), (0x13, dimstyle)]);
    let (db, diag) = objects(&src);
    assert!(diag.is_empty(), "{:?}", codes_of(&diag));
    let values = |handle: u64| -> Vec<(i16, Data)> {
        let object = db.get(handle).unwrap();
        object.values[2..]
            .iter()
            .map(|v| (v.code, v.data.clone()))
            .collect()
    };
    let text = |text: &[u8]| Data::Text(text.to_vec());
    let reals = |codes: [i16; 3], point: [f64; 3]| codes.into_iter().zip(point.map(Data::Real));

    let mut view = vec![
        (2, text(b"V1")),
        (70, Data::Int(1)),
        (40, Data::Real(10.0)),
        (10, Data::Real(1.0)),
        (20, Data::Real(2.0)),
        (41, Data::Real(20.0)),
    ];
    view.extend(reals([11, 21, 31], [0.0, 0.0, 1.0]));
    view.extend(reals([12, 22, 32], [3.0, 4.0, 5.0]));
    view.extend([
        (42, Data::Real(50.0)),
        (43, Data::Real(0.0)),
        (44, Data::Real(0.0)),
        (50, Data::Real(180.0)),
        (71, Data::Int(1 | 4 | 16)),
        (281, Data::Int(1)),
        (72, Data::Int(1)),
    ]);
    view.extend(reals([110, 120, 130], [1.0, 1.0, 0.0]));
    view.extend(reals([111, 121, 131], [1.0, 0.0, 0.0]));
    view.extend(reals([112, 122, 132], [0.0, 1.0, 0.0]));
    view.extend([
        (79, Data::Int(3)),
        (146, Data::Real(2.0)),
        (346, Data::Handle(0x60)),
        (345, Data::Handle(0x61)),
    ]);
    assert_eq!(values(0x10), view);

    let mut ucs = vec![(2, text(b"U1")), (70, Data::Int(0))];
    ucs.extend(reals([10, 20, 30], [1.0, 2.0, 3.0]));
    ucs.extend(reals([11, 21, 31], [1.0, 0.0, 0.0]));
    ucs.extend(reals([12, 22, 32], [0.0, 1.0, 0.0]));
    ucs.extend([
        (79, Data::Int(1)),
        (146, Data::Real(5.0)),
        (346, Data::Handle(0x62)),
    ]);
    assert_eq!(values(0x11), ucs);

    let vport = db.get(0x12).unwrap();
    for (code, data) in [
        (12, Data::Real(5.0)),
        (41, Data::Real(2.5)),
        (71, Data::Int(8)),
        (72, Data::Int(1000)),
        (73, Data::Int(1)),
        (74, Data::Int(3)),
        (76, Data::Int(1)),
        (65, Data::Int(1)),
        (345, Data::Handle(0x64)),
    ] {
        assert_eq!(vport.first(code), Some(&data), "{code}");
    }
    assert_eq!(vport.first(346), None);

    assert_eq!(
        values(0x13),
        [
            (2, text(b"D1")),
            (70, Data::Int(0)),
            (3, text(b"<> mm")),
            (40, Data::Real(2.0)),
            (73, Data::Int(0)),
            (271, Data::Int(2)),
            (371, Data::Int(15)),
            (340, Data::Handle(0x11)),
            (342, Data::Handle(0x70)),
        ]
    );
}

/// 用紙空間の LINE（entmode 1）を足したオブジェクト
#[cfg(test)]
fn objects_with_paper_space() -> Vec<(u64, Vec<u8>)> {
    let mut objects = objects_in_sample();
    let line = object(
        0x13,
        0x2E,
        |w| {
            entity_common(w, 1, 256, 1.0, 0);
            line_data(w, [0.0, 0.0, 0.0], [1.0, 1.0, 0.0], 0.0);
        },
        entity_handles(0x10),
    );
    objects.push((0x2E, line));
    objects
}

#[test]
fn entities_in_model_and_paper_space_are_owned_by_their_blocks() {
    let src = with_header_variables(dwg(&objects_with_paper_space()), 0x58, 0x1F, 0, [0.0; 4]);
    let (db, diag) = objects(&src);
    assert_eq!(
        codes_of(&diag),
        [codes::DWG_TYPE_UNSUPPORTED, codes::DWG_OBJECT_INVALID]
    );
    // 所有者のハンドルを持つ図形は、そのハンドルのまま
    assert_eq!(db.get(0x2A).unwrap().owner(), Some(0x1F));
    assert_eq!(db.get(0x2B).unwrap().owner(), Some(0x1F));
    assert_eq!(db.get(0x2E).unwrap().owner(), Some(0x58));
}

#[test]
fn broken_header_variables_leave_the_spaces_without_owners() {
    let mut src = with_header_variables(dwg(&objects_with_paper_space()), 0x58, 0x1F, 0, [0.0; 4]);
    let last = src.len() - 1;
    src[last] ^= 0xFF; // 終わりの印を壊す
    let (db, diag) = objects(&src);
    assert!(codes_of(&diag).contains(&codes::DWG_FILE_INVALID));
    assert_eq!(db.get(0x2B).unwrap().owner(), None);
    assert_eq!(db.get(0x2E).unwrap().owner(), None);
}

/// 用紙空間の LINE と、ほかのブロックの LINE を足したオブジェクト
#[cfg(test)]
fn objects_with_a_block() -> Vec<(u64, Vec<u8>)> {
    let mut objects = objects_with_paper_space();
    let line = object(
        0x13,
        0x2F,
        |w| {
            entity_common(w, 0, 256, 1.0, 0);
            line_data(w, [0.0, 0.0, 0.0], [2.0, 2.0, 0.0], 0.0);
        },
        |w| {
            w.handle(4, 0x60); // ほかのブロックのレコード
            w.handle(3, 0);
            w.handle(5, 0x10);
        },
    );
    objects.push((0x2F, line));
    objects
}

#[test]
fn header_variables_and_paper_space_flags_are_in_the_table() {
    let src = with_header_variables(
        dwg(&objects_with_a_block()),
        0x58,
        0x1F,
        4,
        [10.0, 10.0, 307.0, 220.0],
    );
    let (db, _) = objects(&src);
    let variable = |name: &[u8]| -> Vec<(i16, Data)> {
        db.variable(name)
            .unwrap()
            .iter()
            .map(|v| (v.code, v.data.clone()))
            .collect()
    };
    assert_eq!(variable(b"$ACADVER"), [(1, Data::Text(b"AC1015".to_vec()))]);
    assert_eq!(variable(b"$INSUNITS"), [(70, Data::Int(4))]);
    assert_eq!(
        variable(b"$PLIMMIN"),
        [(10, Data::Real(10.0)), (20, Data::Real(10.0))]
    );
    assert_eq!(
        variable(b"$PLIMMAX"),
        [(10, Data::Real(307.0)), (20, Data::Real(220.0))]
    );
    // 用紙空間の図形だけが 67 = 1 を持つ
    assert_eq!(db.get(0x2E).unwrap().first(67), Some(&Data::Int(1)));
    assert_eq!(db.get(0x2B).unwrap().first(67), None);
    // ほかのブロックの図形は、DXF と同じくブロックの所に属する
    assert_eq!(db.get(0x2A).unwrap().section, Section::Entities);
    assert_eq!(db.get(0x2E).unwrap().section, Section::Entities);
    assert_eq!(db.get(0x2F).unwrap().section, Section::Blocks);
}

#[cfg(feature = "drawing")]
#[test]
fn a_dwg_lifts_to_the_drawing_model_through_the_table() {
    use zumen_base::Ref;
    use zumen_drawing::{
        Element, ModelPoint, PaperLength, PaperPoint, Shape, SheetId, SpaceId, Unit,
    };
    use zumen_sdk::Lift;
    let src = with_header_variables(
        dwg(&objects_with_a_block()),
        0x58,
        0x1F,
        4,
        [10.0, 10.0, 307.0, 220.0],
    );
    let (db, _) = objects(&src);
    let source = |handle: u64| match db.get(handle).unwrap().origin {
        Origin::Dwg { offset, end } => Ref::at_bytes(offset as u64, end as u64),
        Origin::Dxf { .. } | Origin::New => panic!("DWG のオブジェクトではない"),
    };
    let (file, mut diag) = read(&src, Limits::default());
    let drawing = file
        .unwrap()
        .lift(&mut Cx::new(
            &mut Budget::default(),
            &mut diag,
            &CancelToken::new(),
        ))
        .unwrap();
    // 表を作るときの診断に続けて、Z のある線分と、欄の途中で尽きた CIRCLE を移さない
    assert_eq!(
        codes_of(&diag),
        [
            codes::DWG_TYPE_UNSUPPORTED,
            codes::DWG_OBJECT_INVALID,
            codes::DXF_NON_PLANAR_DROPPED,
            codes::DXF_LIFT_DROPPED,
        ]
    );
    let model = |x: f64, y: f64| ModelPoint {
        space: SpaceId(0),
        x,
        y,
    };
    let space = &drawing.spaces[0];
    assert_eq!(space.unit, Unit::Millimeter);
    assert_eq!(
        space.elements,
        [Element {
            source: source(0x2A),
            layer: "0".to_owned(),
            shape: Shape::Line {
                start: model(1.5, 2.0),
                end: model(100.0, 50.0),
            },
        }]
    );
    // 用紙の点は PLIMMIN を原点にした mm。ほかのブロックの線分は移さない
    let paper = |x: f64, y: f64| PaperPoint {
        sheet: SheetId(0),
        x,
        y,
    };
    let sheet = &drawing.sheets[0];
    assert_eq!(sheet.size, Some([PaperLength(297.0), PaperLength(210.0)]));
    assert_eq!(
        sheet.elements,
        [Element {
            source: source(0x2E),
            layer: "0".to_owned(),
            shape: Shape::Line {
                start: paper(-10.0, -10.0),
                end: paper(-9.0, -9.0),
            },
        }]
    );
}

#[test]
fn dwg_lines_have_the_same_typed_view_as_dxf() {
    let (db, _) = objects(&dwg(&objects_in_sample()));
    let line = |handle: u64| db.get(handle).unwrap().line();
    assert_eq!(
        line(0x2A),
        Some(Ok(Line {
            layer: b"0",
            start: [1.5, 2.0, 0.0],
            end: [100.0, 50.0, 0.0],
        }))
    );
    assert_eq!(
        line(0x2B),
        Some(Ok(Line {
            layer: b"0",
            start: [0.0, 0.0, 3.0],
            end: [0.0, 4.0, 3.0],
        }))
    );
    assert_eq!(line(0x10), None);
}

#[cfg(test)]
/// R2004 の地図のページの検査の値（ODS の 4）
fn adler(seed: u32, data: &[u8]) -> u32 {
    let (mut low, mut high) = (seed & 0xFFFF, seed >> 16);
    for chunk in data.chunks(0x15B0) {
        for &byte in chunk {
            low += u32::from(byte);
            high += low;
        }
        low %= 0xFFF1;
        high %= 0xFFF1;
    }
    (high << 16) | low
}

#[cfg(test)]
fn crc32(data: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &byte in data {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

#[cfg(test)]
/// リテラルだけの圧縮のデータ（4 バイト以上）
fn literal_stream(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let len = data.len();
    assert!(len >= 4);
    if len <= 0x12 {
        out.push((len - 3) as u8);
    } else {
        out.push(0);
        let mut rest = len - 0x12;
        while rest > 0xFF {
            out.push(0);
            rest -= 0xFF;
        }
        out.push(rest as u8);
    }
    out.extend_from_slice(data);
    out.push(0x11);
    out
}

#[cfg(test)]
fn system_page(kind: u32, data: &[u8]) -> Vec<u8> {
    let stream = literal_stream(data);
    let mut head = Vec::new();
    for value in [kind, data.len() as u32, stream.len() as u32, 2, 0] {
        head.extend_from_slice(&value.to_le_bytes());
    }
    let checksum = adler(adler(0, &head), &stream);
    head[16..].copy_from_slice(&checksum.to_le_bytes());
    head.extend_from_slice(&stream);
    head
}

#[cfg(test)]
/// 区切りの名前と圧縮したデータと展開した大きさから、R2004 の入れ物を作る
fn r2004(sections: &[(&[u8], Vec<u8>, u64)]) -> Vec<u8> {
    let mut src = vec![0; 0x100];
    src[..6].copy_from_slice(b"AC1018");
    src[0x13] = 30;
    let mut page_map = Vec::new();
    let mut map = Vec::new();
    let count = sections.len() as i32;
    for value in [count, 2, 0x7400, 0, count] {
        map.extend_from_slice(&value.to_le_bytes());
    }
    for (i, (name, stream, size)) in sections.iter().enumerate() {
        let number = i as i32 + 1;
        let address = src.len() as u32;
        let mask = 0x4164_536B ^ address;
        for value in [
            0x4163_043B,
            number as u32,
            stream.len() as u32,
            0,
            0,
            0,
            0,
            0,
        ] {
            src.extend_from_slice(&(value ^ mask).to_le_bytes());
        }
        src.extend_from_slice(stream);
        page_map.extend_from_slice(&number.to_le_bytes());
        page_map.extend_from_slice(&(0x20 + stream.len() as i32).to_le_bytes());
        map.extend_from_slice(&size.to_le_bytes());
        for value in [1, 0x7400, 1, 2, number, 0] {
            map.extend_from_slice(&value.to_le_bytes());
        }
        let mut field = [0; 64];
        field[..name.len()].copy_from_slice(name);
        map.extend_from_slice(&field);
        map.extend_from_slice(&number.to_le_bytes());
        map.extend_from_slice(&(stream.len() as i32).to_le_bytes());
        map.extend_from_slice(&0u64.to_le_bytes());
    }
    let section_map_id = count + 1;
    let page = system_page(0x4163_003B, &map);
    page_map.extend_from_slice(&section_map_id.to_le_bytes());
    page_map.extend_from_slice(&(page.len() as i32).to_le_bytes());
    src.extend_from_slice(&page);
    let page_map_id = count + 2;
    page_map.extend_from_slice(&page_map_id.to_le_bytes());
    page_map.extend_from_slice(&0x400i32.to_le_bytes());
    let page_map_at = src.len() as u64 - 0x100;
    src.extend_from_slice(&system_page(0x4163_0E3B, &page_map));

    let mut header = vec![0; 0x6C];
    header[..12].copy_from_slice(b"AcFssFcAJMB\0");
    header[0x10..0x14].copy_from_slice(&0x6Cu32.to_le_bytes());
    header[0x50..0x54].copy_from_slice(&page_map_id.to_le_bytes());
    header[0x54..0x5C].copy_from_slice(&page_map_at.to_le_bytes());
    header[0x5C..0x60].copy_from_slice(&section_map_id.to_le_bytes());
    let crc = crc32(&header);
    header[0x68..].copy_from_slice(&crc.to_le_bytes());
    let mut seed: u32 = 1;
    for (i, byte) in header.iter().enumerate() {
        seed = seed.wrapping_mul(0x343FD).wrapping_add(0x0026_9EC3);
        src[0x80 + i] = byte ^ (seed >> 16) as u8;
    }
    src
}

#[cfg(test)]
fn r2004_sections() -> Vec<(&'static [u8], Vec<u8>, u64)> {
    // クラスの区切りは、R2004 の前置き（最大のクラスの番号・0 の RC・0 の RC・真の B）だけを持つ
    let mut w = BitWriter::default();
    w.bs(499);
    w.bytes(&[0, 0]);
    w.bit(true);
    let data = w.finish();
    let mut classes = vec![
        0x8D, 0xA1, 0xC4, 0xB8, 0xC4, 0xA9, 0xF8, 0xC5, 0xC0, 0xDC, 0xF4, 0x5F, 0xE7, 0xCF, 0xB6,
        0x8A,
    ];
    let mut sized = (data.len() as u32).to_le_bytes().to_vec();
    sized.extend(&data);
    classes.extend(&sized);
    classes.extend(crc16(0xC0C1, &sized).to_le_bytes());
    classes.extend([
        0x72, 0x5E, 0x3B, 0x47, 0x3B, 0x56, 0x07, 0x3A, 0x3F, 0x23, 0x0B, 0xA0, 0x18, 0x30, 0x49,
        0x75,
    ]);
    // オブジェクトの地図は、空の区切り（大きさ 2 と CRC）だけを持つ
    let mut map = vec![0x00, 0x02];
    map.extend(crc16(0xC0C1, &map).to_be_bytes());
    let filler: Vec<u8> = (0..40).map(|i| i as u8).collect();
    [
        (&b"AcDb:Header"[..], filler.clone()),
        (b"AcDb:Classes", classes),
        (b"AcDb:Handles", map),
        (b"AcDb:AcDbObjects", filler),
    ]
    .into_iter()
    .map(|(name, data)| (name, literal_stream(&data), data.len() as u64))
    .collect()
}

#[test]
fn r2004_containers_are_checked_and_read() {
    let src = r2004(&r2004_sections());
    let (result, diag) = read(&src, Limits::default());
    assert!(result.is_ok());
    assert_eq!(codes_of(&diag), []);

    // 暗号化した見出しの使わない欄を変えると、見出しの CRC が合わなくなる
    let mut broken = src.clone();
    broken[0x80 + 0x24] ^= 0x01;
    let (result, diag) = read(&broken, Limits::default());
    assert!(result.is_ok());
    assert_eq!(codes_of(&diag), [codes::DWG_CRC_INVALID]);

    // 初めの印が合わなければ止める
    let mut broken = src.clone();
    broken[0x80] ^= 0x01;
    let (result, diag) = read(&broken, Limits::default());
    assert!(matches!(result, Err(Error::Invalid)));
    assert_eq!(codes_of(&diag), [codes::DWG_FILE_INVALID]);

    // ページの地図の最後のページの大きさ（使わない）を変えると、検査の値が合わなくなる
    let mut broken = src;
    let last = broken.len() - 1;
    assert_eq!(broken[last], 0x11);
    broken[last - 1] ^= 0x01;
    let (result, diag) = read(&broken, Limits::default());
    assert!(result.is_ok());
    assert_eq!(codes_of(&diag), [codes::DWG_CRC_INVALID]);
}

/// R2010 のクラスの区切り。データの初めに文字列の流れの終わりまでのビットの数（RL）を持ち、
/// 終わりのビットが 0 なので文字列の流れはない。`high` なら、大きさの後に上位の RL を置く
#[cfg(test)]
fn r2010_classes(high: bool) -> Vec<u8> {
    let mut w = BitWriter::default();
    w.bytes(&68u32.to_le_bytes());
    w.bs(499);
    w.bytes(&[0, 0]);
    w.bit(true);
    w.bit(false);
    let data = w.finish();
    let mut classes = vec![
        0x8D, 0xA1, 0xC4, 0xB8, 0xC4, 0xA9, 0xF8, 0xC5, 0xC0, 0xDC, 0xF4, 0x5F, 0xE7, 0xCF, 0xB6,
        0x8A,
    ];
    let mut sized = (data.len() as u32).to_le_bytes().to_vec();
    if high {
        sized.extend(0u32.to_le_bytes());
    }
    sized.extend(&data);
    classes.extend(&sized);
    classes.extend(crc16(0xC0C1, &sized).to_le_bytes());
    classes.extend([
        0x72, 0x5E, 0x3B, 0x47, 0x3B, 0x56, 0x07, 0x3A, 0x3F, 0x23, 0x0B, 0xA0, 0x18, 0x30, 0x49,
        0x75,
    ]);
    classes
}

#[test]
fn r2010_containers_are_read_with_or_without_the_high_size() {
    for high in [true, false] {
        let mut sections = r2004_sections();
        let classes = r2010_classes(high);
        sections[1] = (
            b"AcDb:Classes",
            literal_stream(&classes),
            classes.len() as u64,
        );
        let mut src = r2004(&sections);
        src[..6].copy_from_slice(b"AC1024");
        let (result, diag) = read(&src, Limits::default());
        assert!(result.is_ok(), "{high}");
        assert_eq!(codes_of(&diag), [], "{high}");
    }
}

#[test]
fn r2004_containers_need_their_sections_and_valid_pages() {
    let mut sections = r2004_sections();
    sections.pop();
    let (result, diag) = read(&r2004(&sections), Limits::default());
    assert!(matches!(result, Err(Error::Invalid)));
    assert_eq!(codes_of(&diag), [codes::DWG_FILE_INVALID]);

    // 展開した所より前を指す距離
    let mut sections = r2004_sections();
    sections[0].1 = vec![0x01, 1, 2, 3, 4, 0x40, 0xFF, 0x11];
    let (result, diag) = read(&r2004(&sections), Limits::default());
    assert!(matches!(result, Err(Error::Invalid)));
    assert_eq!(codes_of(&diag), [codes::DWG_FILE_INVALID]);

    // 前のバイトを写す圧縮も展開する
    let mut sections = r2004_sections();
    sections[0] = (b"AcDb:Header", vec![0x01, 1, 2, 3, 4, 0x40, 0x00, 0x11], 7);
    let (result, diag) = read(&r2004(&sections), Limits::default());
    assert!(result.is_ok());
    assert_eq!(codes_of(&diag), []);
}

/// 大きさ（MS）と CRC を除いた、オブジェクトのデータ
#[cfg(test)]
fn data_of(object: &[u8]) -> Vec<u8> {
    object[2..object.len() - 2].to_vec()
}

/// `changes` のデータの置き換えを確定し、書いたバイト列と診断を返す
#[cfg(test)]
fn edited(
    file: &mut DwgFile,
    changes: &[(usize, Vec<u8>)],
) -> (Result<Vec<u8>, Error>, Diagnostics) {
    let mut diag = Diagnostics::new();
    let mut budget = Budget::default();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
    let mut txn = file.transaction();
    let staged: Result<(), Error> = changes
        .iter()
        .try_for_each(|(record, data)| txn.set_data(*record, data));
    let committed = txn.commit(&mut cx);
    let result = staged.and(committed).and_then(|()| {
        let mut out = Vec::new();
        Dwg::write(file, &mut out, &mut cx).map(|()| out)
    });
    (result, diag)
}

#[test]
fn replaced_object_data_is_appended_with_a_new_object_map() {
    let src = dwg_in_sections(&objects_in_sample(), 2);
    let (file, before) = read(&src, Limits::default());
    let mut file = file.unwrap();
    // 厚さを足して、データを長くする
    let line = object(
        0x13,
        0x2A,
        |w| {
            entity_common(w, 0, 256, 1.0, 0);
            line_data(w, [7.0, 2.0, 0.0], [100.0, 50.0, 0.0], 0.25);
        },
        |w| {
            w.handle(4, 0x1F);
            w.handle(3, 0);
            w.handle(5, 0x10);
        },
    );
    let (out, diag) = edited(&mut file, &[(2, data_of(&line))]);
    let out = out.unwrap();
    assert!(diag.is_empty(), "{:?}", codes_of(&diag));
    // 元のバイトは残り、変えたオブジェクトと地図が後ろに足される
    assert_eq!(&out[..0x19], &src[..0x19]);
    assert_eq!(&out[0x19 + 27 + 2..src.len()], &src[0x19 + 27 + 2..]);
    let (again, after) = read(&out, Limits::default());
    let mut again = again.unwrap();
    assert_eq!(codes_of(&after), codes_of(&before));
    assert_eq!(again.record_count(), 6);
    for i in [0, 1, 3, 4, 5] {
        assert_eq!(
            again.record(i).unwrap().bytes,
            file.record(i).unwrap().bytes
        );
    }
    assert_eq!(
        again.record(2).unwrap().bytes,
        file.record(2).unwrap().bytes
    );
    assert!(again.record(2).unwrap().bytes.unwrap().start as usize >= src.len());
    let (db, _) = objects(&out);
    let line = db.get(0x2A).unwrap();
    assert_eq!(line.real(10), Some(Some(7.0)));
    assert_eq!(line.real(39), Some(Some(0.25)));
    // もう一度変えても読める
    let (twice, diag) = edited(&mut again, &[(5, data_of(&objects_in_sample()[5].1))]);
    assert!(diag.is_empty(), "{:?}", codes_of(&diag));
    let (result, diag) = read(&twice.unwrap(), Limits::default());
    assert_eq!(result.unwrap().record_count(), 6);
    assert_eq!(codes_of(&diag), codes_of(&before));
}

#[test]
fn invalid_object_changes_are_rejected_without_changing_the_file() {
    let src = sample();
    let mut file = read(&src, Limits::default()).0.unwrap();
    let first = data_of(&objects_in_sample()[0].1);
    for (record, data, code) in [
        (6, vec![0u8; 4], codes::DWG_OBJECT_MISSING),
        (2, Vec::new(), codes::DWG_DATA_INVALID),
    ] {
        let (result, diag) = edited(&mut file, &[(0, first.clone()), (record, data)]);
        assert_eq!(result, Err(Error::Rejected));
        assert_eq!(codes_of(&diag), [code]);
        let (out, _) = edited(&mut file, &[]);
        assert_eq!(out.unwrap(), src);
    }
    let mut r2004 = read(&r2004(&r2004_sections()), Limits::default())
        .0
        .unwrap();
    let (result, diag) = edited(&mut r2004, &[(0, first)]);
    assert_eq!(result, Err(Error::Rejected));
    assert_eq!(codes_of(&diag), [codes::DWG_EDIT_UNSUPPORTED]);
}

/// 表 `db` の変更を書き戻し、書いたバイト列と診断を返す
#[cfg(test)]
fn written_back(file: &mut DwgFile, db: &AcadDb) -> (Result<Vec<u8>, Error>, Diagnostics) {
    let mut diag = Diagnostics::new();
    let mut budget = Budget::default();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
    let result = file.write_back(db, &mut cx).and_then(|()| {
        let mut out = Vec::new();
        Dwg::write(file, &mut out, &mut cx).map(|()| out)
    });
    (result, diag)
}

/// 見本の DWG に、CIRCLE と ARC を足したもの
#[cfg(test)]
fn sample_with_curves() -> Vec<u8> {
    let circle = object(
        0x12,
        0x30,
        |w| {
            entity_common(w, 2, 256, 1.0, 0);
            for v in [1.0, 2.0, 0.0, 5.0] {
                w.bd(v);
            }
            w.bit(false);
            w.bd(0.25);
            w.bit(true);
        },
        entity_handles(0x10),
    );
    let arc = object(
        0x11,
        0x31,
        |w| {
            entity_common(w, 2, 256, 1.0, 0);
            for v in [0.0, 0.0, 0.0, 3.0] {
                w.bd(v);
            }
            w.bit(true);
            w.bit(false);
            for v in [0.0, 0.0, -1.0] {
                w.bd(v);
            }
            w.bd(std::f64::consts::FRAC_PI_2);
            w.bd(std::f64::consts::PI);
        },
        entity_handles(0x10),
    );
    let mut objects = objects_in_sample();
    objects.extend([(0x30, circle), (0x31, arc)]);
    dwg(&objects)
}

/// 見本の DWG に、POINT・ELLIPSE・RAY・XLINE・SOLID・TRACE・3DFACE・SHAPE を足したもの
#[cfg(test)]
fn sample_with_more_kinds() -> Vec<u8> {
    let common = |w: &mut BitWriter| entity_common(w, 2, 256, 1.0, 0);
    let point = object(
        0x1B,
        0x32,
        |w| {
            common(w);
            for v in [1.0, 2.0, 3.0] {
                w.bd(v);
            }
            w.bit(true);
            w.bit(true);
            w.bd(0.0);
        },
        entity_handles(0x10),
    );
    let ellipse = object(
        0x23,
        0x33,
        |w| {
            common(w);
            for v in [0.0, 0.0, 0.0, 4.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.5, 0.0] {
                w.bd(v);
            }
            w.bd(std::f64::consts::TAU);
        },
        entity_handles(0x10),
    );
    let ray = |type_code, handle| {
        object(
            type_code,
            handle,
            |w| {
                common(w);
                for v in [1.0, 1.0, 0.0, 1.0, 0.0, 0.0] {
                    w.bd(v);
                }
            },
            entity_handles(0x10),
        )
    };
    let solid = |type_code, handle| {
        object(
            type_code,
            handle,
            |w| {
                common(w);
                w.bit(true);
                w.bd(0.0);
                for v in [0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 1.0, 1.0] {
                    w.rd(v);
                }
                w.bit(true);
            },
            entity_handles(0x10),
        )
    };
    let face = object(
        0x1C,
        0x38,
        |w| {
            common(w);
            w.bit(false);
            w.bit(true);
            w.rd(0.0);
            w.rd(0.0);
            for corner in [[1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0]] {
                for v in corner {
                    w.bits(3, 2);
                    w.rd(v);
                }
            }
            w.bs(2);
        },
        entity_handles(0x10),
    );
    let shape = object(
        0x21,
        0x39,
        |w| {
            entity_common(w, 2, 3, 2.0, 0);
            for v in [1.0, 2.0, 0.0, 2.5, 0.0, 1.0, 0.0, 0.0] {
                w.bd(v);
            }
            w.bs(7);
            for v in [0.0, 0.0, 1.0] {
                w.bd(v);
            }
        },
        entity_handles_to(0x10, 0),
    );
    // DataFlags が 0: どの欄も持つ TEXT
    let text = object(
        0x01,
        0x3A,
        |w| {
            common(w);
            w.bytes(&[0]);
            w.rd(3.0);
            w.rd(1.0);
            w.rd(2.0);
            w.dd(5.0, 1.0);
            w.dd(2.0, 2.0);
            w.bit(true);
            w.bit(false);
            w.bd(0.5);
            w.rd(std::f64::consts::FRAC_PI_6);
            w.rd(std::f64::consts::FRAC_PI_2);
            w.rd(2.5);
            w.rd(0.8);
            w.bs(2);
            w.bytes(b"A\0");
            w.bs(2);
            w.bs(1);
            w.bs(3);
        },
        entity_handles_to(0x10, 0),
    );
    // 尺度が (1, 1, 1) の INSERT と MINSERT
    let insert = |type_code, handle| {
        object(
            type_code,
            handle,
            move |w: &mut BitWriter| {
                common(w);
                for v in [4.0, 5.0, 0.0] {
                    w.bd(v);
                }
                w.bits(3, 2);
                w.bd(0.0);
                for v in [0.0, 0.0, 1.0] {
                    w.bd(v);
                }
                w.bit(false);
                if type_code == 0x08 {
                    w.bs(3);
                    w.bs(1);
                    w.bd(2.5);
                    w.bd(0.0);
                }
            },
            entity_handles_to(0x10, 0),
        )
    };
    // DataFlags が 0x7F で、縦の合わせだけを持つ ATTRIB と ATTDEF
    let attribute = |type_code, handle| {
        object(
            type_code,
            handle,
            move |w: &mut BitWriter| {
                common(w);
                w.bytes(&[0x7F]);
                w.rd(1.0);
                w.rd(2.0);
                w.bit(true);
                w.bit(true);
                w.rd(2.5);
                w.bs(4);
                w.bytes(b"A^B\0");
                w.bs(2);
                w.bs(4);
                w.bytes(b"TAG\0");
                w.bs(0);
                w.bytes(&[1]);
                if type_code == 0x03 {
                    w.bs(5);
                    w.bytes(b"Name\0");
                }
            },
            entity_handles_to(0x10, 0),
        )
    };
    // 600 バイトの文字列の MTEXT。文字の範囲の大きさは表にない
    let long: Vec<u8> = (0..600).map(|i| b'a' + (i % 26) as u8).collect();
    let mtext = object(
        0x2C,
        0x40,
        move |w| {
            common(w);
            for v in [1.0, 2.0, 0.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0] {
                w.bd(v);
            }
            w.bd(10.0);
            w.bd(2.5);
            w.bs(1);
            w.bs(5);
            w.bd(3.0);
            w.bd(9.0);
            w.bs(601);
            w.bytes(&long);
            w.bytes(&[0]);
            w.bs(1);
            w.bd(1.0);
            w.bit(true);
        },
        entity_handles_to(0x10, 0),
    );
    // 閉じていて、頂点ごとの幅と膨らみがある LWPOLYLINE。一定の幅（0.5）は表にない
    let lwpolyline = object(
        0x4D,
        0x41,
        move |w| {
            common(w);
            w.bs(0x200 | 32 | 16 | 4);
            w.bd(0.5);
            for _ in 0..3 {
                w.bits(1, 2);
                w.bytes(&[2]);
            }
            w.rd(1.0);
            w.rd(2.0);
            w.dd(5.0, 1.0);
            w.dd(2.0, 2.0);
            for v in [0.0, 0.25, 0.1, 0.2, 0.3, 0.4] {
                w.bd(v);
            }
        },
        entity_handles_to(0x10, 0),
    );
    // 節点と制御点の SPLINE（閉じている印が立つ）と、当てはめる点の SPLINE
    let by_knots = object(
        0x24,
        0x42,
        move |w| {
            common(w);
            for v in [1, 1] {
                w.bits(1, 2);
                w.bytes(&[v]);
            }
            w.bit(false);
            w.bit(true);
            w.bit(false);
            w.bd(1e-9);
            w.bd(1e-10);
            for v in [4, 2] {
                w.bits(1, 2);
                w.bytes(&[v]);
            }
            w.bit(true);
            for v in [0.0, 0.0, 1.0, 1.0] {
                w.bd(v);
            }
            for v in [0.0, 0.0, 0.0, 1.0, 2.0, 3.0, 0.0, 0.5] {
                w.bd(v);
            }
        },
        entity_handles_to(0x10, 0),
    );
    let by_fit = object(
        0x24,
        0x43,
        move |w| {
            common(w);
            for v in [2, 3] {
                w.bits(1, 2);
                w.bytes(&[v]);
            }
            w.bd(0.25);
            for v in [1.0, 0.0, 0.0, 0.0, 0.0, 0.0] {
                w.bd(v);
            }
            w.bits(1, 2);
            w.bytes(&[2]);
            for v in [0.0, 0.0, 0.0, 5.0, 6.0, 0.0] {
                w.bd(v);
            }
        },
        entity_handles_to(0x10, 0),
    );
    let mut objects = objects_in_sample();
    objects.extend([
        (0x32, point),
        (0x33, ellipse),
        (0x34, ray(0x28, 0x34)),
        (0x35, ray(0x29, 0x35)),
        (0x36, solid(0x1F, 0x36)),
        (0x37, solid(0x20, 0x37)),
        (0x38, face),
        (0x39, shape),
        (0x3A, text),
        (0x3B, insert(0x07, 0x3B)),
        (0x3C, insert(0x07, 0x3C)),
        (0x3D, insert(0x08, 0x3D)),
        (0x3E, attribute(0x02, 0x3E)),
        (0x3F, attribute(0x03, 0x3F)),
        (0x40, mtext),
        (0x41, lwpolyline),
        (0x42, by_knots),
        (0x43, by_fit),
    ]);
    dwg(&objects)
}

/// `handle` のオブジェクトの `code` の値を `data` にする。なければ足す
#[cfg(test)]
fn set_value(db: &mut AcadDb, handle: u64, code: i16, data: Data) {
    let index = db
        .objects()
        .iter()
        .position(|o| o.handle() == Some(handle))
        .unwrap();
    let values = &mut db.object_mut(index).unwrap().values;
    match values.iter_mut().find(|v| v.code == code) {
        Some(value) => value.data = data,
        None => values.push(Value { code, data }),
    }
}

#[test]
fn changed_lines_circles_and_arcs_are_written_back() {
    let src = sample_with_curves();
    let (mut db, before) = objects(&src);
    let mut file = read(&src, Limits::default()).0.unwrap();
    // 平らな LINE の終わりの点を変え、厚さと押し出しの向きを足す
    set_value(&mut db, 0x2A, 11, Data::Real(123.25));
    set_value(&mut db, 0x2A, 21, Data::Real(-0.5));
    set_value(&mut db, 0x2A, 39, Data::Real(0.75));
    for (code, v) in [(210, 0.0), (220, 0.0), (230, -1.0)] {
        set_value(&mut db, 0x2A, code, Data::Real(v));
    }
    // Z のある LINE を平らにする
    set_value(&mut db, 0x2B, 30, Data::Real(0.0));
    set_value(&mut db, 0x2B, 31, Data::Real(0.0));
    set_value(&mut db, 0x30, 40, Data::Real(1.0));
    set_value(&mut db, 0x30, 39, Data::Real(0.0));
    set_value(&mut db, 0x31, 50, Data::Real(45.0));
    set_value(&mut db, 0x31, 10, Data::Real(-2.0e10));
    let (out, diag) = written_back(&mut file, &db);
    let out = out.unwrap();
    assert!(diag.is_empty(), "{:?}", codes_of(&diag));
    assert_eq!(&out[0x19 + 27 + 2..src.len()], &src[0x19 + 27 + 2..]);
    let (after, again) = objects(&out);
    assert_eq!(codes_of(&again), codes_of(&before));
    let values =
        |db: &AcadDb| -> Vec<_> { db.objects().iter().map(|o| o.values.clone()).collect() };
    let mut expected = values(&db);
    // 角度は、ラジアンに直して書き、度に直して読む
    let arc = &mut expected[7];
    for value in arc.iter_mut().filter(|v| v.code == 50) {
        value.data = Data::Real(45.0f64.to_radians().to_degrees());
    }
    assert_eq!(values(&after), expected);
    // 変えていないオブジェクトは、元の位置のまま
    let again = read(&out, Limits::default()).0.unwrap();
    for i in [0, 1, 4, 5] {
        assert_eq!(
            again.record(i).unwrap().bytes,
            file.record(i).unwrap().bytes
        );
    }
}

#[test]
fn changed_points_ellipses_rays_solids_and_faces_are_written_back() {
    let src = sample_with_more_kinds();
    let (mut db, before) = objects(&src);
    let mut file = read(&src, Limits::default()).0.unwrap();
    let edits: [(u64, i16, f64); 18] = [
        // POINT の位置を変え、X 軸の角度と厚さを足す
        (0x32, 20, -7.5),
        (0x32, 39, 2.0),
        (0x32, 50, 30.0),
        // ELLIPSE の比と終わりの角度を変え、押し出しの向きを足す
        (0x33, 40, 0.25),
        (0x33, 42, 1.5),
        (0x33, 210, 0.0),
        (0x33, 220, 0.0),
        (0x33, 230, -1.0),
        (0x34, 11, 0.0),
        (0x34, 21, 1.0),
        (0x35, 10, 1.0e-300),
        // SOLID を持ち上げ、厚さを足す
        (0x36, 30, 5.0),
        (0x36, 31, 5.0),
        (0x36, 32, 5.0),
        (0x36, 33, 5.0),
        (0x36, 39, 1.0),
        (0x37, 23, 9.0),
        // 3DFACE の角を持ち上げる
        (0x38, 32, 0.125),
    ];
    for (handle, code, v) in edits {
        set_value(&mut db, handle, code, Data::Real(v));
    }
    // 3DFACE の見えない辺の印を除く
    let index = db
        .objects()
        .iter()
        .position(|o| o.handle() == Some(0x38))
        .unwrap();
    db.object_mut(index)
        .unwrap()
        .values
        .retain(|v| v.code != 70);
    let (out, diag) = written_back(&mut file, &db);
    let out = out.unwrap();
    assert!(diag.is_empty(), "{:?}", codes_of(&diag));
    let (after, again) = objects(&out);
    assert_eq!(codes_of(&again), codes_of(&before));
    let values =
        |db: &AcadDb| -> Vec<_> { db.objects().iter().map(|o| o.values.clone()).collect() };
    let mut expected = values(&db);
    // 角度は、ラジアンに直して書き、度に直して読む
    let changed = |v: &&mut Value| v.code == 50 && v.data == Data::Real(30.0);
    for value in expected.iter_mut().flatten().filter(changed) {
        value.data = Data::Real(30.0f64.to_radians().to_degrees());
    }
    assert_eq!(values(&after), expected);
    // 角の Z が揃わない SOLID と、実数の見えない辺の印は符号化できない
    let (db, _) = objects(&src);
    let edits: [(&str, Edit); 2] = [
        ("SOLID", |db| set_value(db, 0x36, 31, Data::Real(1.0))),
        ("3DFACE", |db| set_value(db, 0x38, 70, Data::Real(2.0))),
    ];
    for (kind, edit) in edits {
        let mut changed = db.clone();
        edit(&mut changed);
        let mut file = read(&src, Limits::default()).0.unwrap();
        let (out, diag) = written_back(&mut file, &changed);
        assert_eq!(out, Err(Error::Rejected), "{kind}");
        assert_eq!(codes_of(&diag), [codes::DWG_CHANGE_UNSUPPORTED], "{kind}");
    }
}

#[test]
fn changed_texts_and_inserts_are_written_back() {
    let src = sample_with_more_kinds();
    let (mut db, before) = objects(&src);
    let mut file = read(&src, Limits::default()).0.unwrap();
    let edits: [(u64, i16, Data); 13] = [
        // TEXT を標高 0 に下ろし、文字列と高さと回転と合わせを変える
        (0x3A, 30, Data::Real(0.0)),
        (0x3A, 31, Data::Real(0.0)),
        (0x3A, 1, Data::Text(b"B^J^ x".to_vec())),
        (0x3A, 40, Data::Real(1.25)),
        (0x3A, 50, Data::Real(30.0)),
        (0x3A, 72, Data::Int(4)),
        // 尺度の印 00 になる INSERT
        (0x3B, 41, Data::Real(2.0)),
        (0x3B, 42, Data::Real(3.0)),
        (0x3B, 50, Data::Real(90.0)),
        // 尺度の印 01 になる INSERT
        (0x3C, 43, Data::Real(0.5)),
        // 尺度の印 10 になる MINSERT
        (0x3D, 41, Data::Real(2.0)),
        (0x3D, 42, Data::Real(2.0)),
        (0x3D, 43, Data::Real(2.0)),
    ];
    for (handle, code, data) in edits {
        set_value(&mut db, handle, code, data);
    }
    set_value(&mut db, 0x3D, 71, Data::Int(4));
    set_value(&mut db, 0x3D, 45, Data::Real(1.5));
    // TEXT の幅の比を除く
    let index = db
        .objects()
        .iter()
        .position(|o| o.handle() == Some(0x3A))
        .unwrap();
    db.object_mut(index)
        .unwrap()
        .values
        .retain(|v| v.code != 41);
    let (out, diag) = written_back(&mut file, &db);
    let out = out.unwrap();
    assert!(diag.is_empty(), "{:?}", codes_of(&diag));
    let (after, again) = objects(&out);
    assert_eq!(codes_of(&again), codes_of(&before));
    // 足した値は表の終わりに置いたので、グループ・コードの順に並べて比べる
    let values = |db: &AcadDb| -> Vec<_> {
        db.objects()
            .iter()
            .map(|o| {
                let mut values = o.values.clone();
                values.sort_by_key(|v| v.code);
                values
            })
            .collect()
    };
    let mut expected = values(&db);
    // 角度は、ラジアンに直して書き、度に直して読む
    for (object, values) in db.objects().iter().zip(&mut expected) {
        if matches!(object.handle(), Some(0x3A..=0x3D)) {
            for value in values.iter_mut().filter(|v| matches!(v.code, 50 | 51)) {
                if let Data::Real(v) = value.data {
                    value.data = Data::Real(v.to_radians().to_degrees());
                }
            }
        }
    }
    assert_eq!(values(&after), expected);
    // 標高と違う合わせの点の Z と、読みが作らない `^` の並びと、属性が続くかの変更は符号化できない
    let (db, _) = objects(&src);
    let edits: [(&str, Edit); 3] = [
        ("31", |db| set_value(db, 0x3A, 31, Data::Real(1.0))),
        ("1", |db| set_value(db, 0x3A, 1, Data::Text(b"^1".to_vec()))),
        ("66", |db| set_value(db, 0x3B, 66, Data::Int(1))),
    ];
    for (code, edit) in edits {
        let mut changed = db.clone();
        edit(&mut changed);
        let mut file = read(&src, Limits::default()).0.unwrap();
        let (out, diag) = written_back(&mut file, &changed);
        assert_eq!(out, Err(Error::Rejected), "{code}");
        assert_eq!(codes_of(&diag), [codes::DWG_CHANGE_UNSUPPORTED], "{code}");
    }
}

#[test]
fn changed_attributes_are_written_back() {
    let src = sample_with_more_kinds();
    let (mut db, before) = objects(&src);
    let mut file = read(&src, Limits::default()).0.unwrap();
    let edits: [(u64, i16, Data); 8] = [
        (0x3E, 1, Data::Text(b"X^ Y^I".to_vec())),
        (0x3E, 2, Data::Text(b"T2".to_vec())),
        (0x3E, 70, Data::Int(8)),
        (0x3E, 74, Data::Int(3)),
        (0x3E, 73, Data::Int(5)),
        (0x3F, 1, Data::Text(Vec::new())),
        (0x3F, 3, Data::Text(b"Q".to_vec())),
        (0x3F, 41, Data::Real(0.9)),
    ];
    for (handle, code, data) in edits {
        set_value(&mut db, handle, code, data);
    }
    let (out, diag) = written_back(&mut file, &db);
    let out = out.unwrap();
    assert!(diag.is_empty(), "{:?}", codes_of(&diag));
    let (after, again) = objects(&out);
    assert_eq!(codes_of(&again), codes_of(&before));
    // 足した値は表の終わりに置いたので、グループ・コードの順に並べて比べる
    let values = |db: &AcadDb| -> Vec<_> {
        db.objects()
            .iter()
            .map(|o| {
                let mut values = o.values.clone();
                values.sort_by_key(|v| v.code);
                values
            })
            .collect()
    };
    assert_eq!(values(&after), values(&db));
    // RC に収まらない印は符号化できない
    let mut changed = objects(&src).0;
    set_value(&mut changed, 0x3E, 70, Data::Int(300));
    let mut file = read(&src, Limits::default()).0.unwrap();
    let (out, diag) = written_back(&mut file, &changed);
    assert_eq!(out, Err(Error::Rejected));
    assert_eq!(codes_of(&diag), [codes::DWG_CHANGE_UNSUPPORTED]);
}

#[test]
fn changed_mtexts_are_written_back() {
    let src = sample_with_more_kinds();
    let (mut db, before) = objects(&src);
    let mut file = read(&src, Limits::default()).0.unwrap();
    let edits: [(i16, Data); 8] = [
        (1, Data::Text(b"x^Jy^ ".to_vec())),
        (40, Data::Real(1.25)),
        (71, Data::Int(7)),
        (11, Data::Real(0.0)),
        (21, Data::Real(1.0)),
        (31, Data::Real(0.0)),
        (73, Data::Int(2)),
        (44, Data::Real(1.5)),
    ];
    for (code, data) in edits {
        set_value(&mut db, 0x40, code, data);
    }
    let (out, diag) = written_back(&mut file, &db);
    let out = out.unwrap();
    assert!(diag.is_empty(), "{:?}", codes_of(&diag));
    let (after, again) = objects(&out);
    assert_eq!(codes_of(&again), codes_of(&before));
    // 足した値は表の終わりに置いたので、同じグループ・コードの値の順を保って並べて比べる
    let values = |db: &AcadDb| -> Vec<_> {
        db.objects()
            .iter()
            .map(|o| {
                let mut values = o.values.clone();
                values.sort_by_key(|v| v.code);
                values
            })
            .collect()
    };
    assert_eq!(values(&after), values(&db));
    // 文字の範囲の大きさ（3.0・9.0）と、行の間隔の後の B は元のまま残る
    let mut text: Vec<u8> = (0..500).map(|i| b'a' + (i % 26) as u8).collect();
    text.extend(b"x\ny^\0");
    let expected = object(
        0x2C,
        0x40,
        |w| {
            entity_common(w, 2, 256, 1.0, 0);
            for v in [1.0, 2.0, 0.0, 0.0, 0.0, 1.0, 0.0, 1.0, 0.0] {
                w.bd(v);
            }
            w.bd(10.0);
            w.bd(1.25);
            w.bs(7);
            w.bs(5);
            w.bd(3.0);
            w.bd(9.0);
            w.bs(text.len() as u16);
            w.bytes(&text);
            w.bs(2);
            w.bd(1.5);
            w.bit(true);
        },
        entity_handles_to(0x10, 0),
    );
    let index = db
        .objects()
        .iter()
        .position(|o| o.handle() == Some(0x40))
        .unwrap();
    let file = read(&out, Limits::default()).0.unwrap();
    let range = file.record(index).unwrap().bytes.unwrap();
    assert_eq!(&out[range.start as usize..range.end as usize], expected);
    // 文字列でない 3 は符号化できない
    let mut changed = objects(&src).0;
    set_value(&mut changed, 0x40, 3, Data::Real(1.0));
    let mut file = read(&src, Limits::default()).0.unwrap();
    let (out, diag) = written_back(&mut file, &changed);
    assert_eq!(out, Err(Error::Rejected));
    assert_eq!(codes_of(&diag), [codes::DWG_CHANGE_UNSUPPORTED]);
}

#[test]
fn changed_lwpolylines_are_written_back() {
    let src = sample_with_more_kinds();
    let (mut db, before) = objects(&src);
    let mut file = read(&src, Limits::default()).0.unwrap();
    let index = db
        .objects()
        .iter()
        .position(|o| o.handle() == Some(0x41))
        .unwrap();
    // 頂点を動かし、2 つめの頂点の終わりの幅を変え、標高を足す
    for value in db.object_mut(index).unwrap().values.iter_mut() {
        match (value.code, &mut value.data) {
            (10, Data::Real(v)) => *v += 1.0,
            (41, Data::Real(v)) if *v == 0.4 => *v = 0.75,
            _ => {}
        }
    }
    set_value(&mut db, 0x41, 38, Data::Real(3.0));
    let (out, diag) = written_back(&mut file, &db);
    let out = out.unwrap();
    assert!(diag.is_empty(), "{:?}", codes_of(&diag));
    let (after, again) = objects(&out);
    assert_eq!(codes_of(&again), codes_of(&before));
    let values = |db: &AcadDb| -> Vec<_> {
        db.objects()
            .iter()
            .map(|o| {
                let mut values = o.values.clone();
                values.sort_by_key(|v| v.code);
                values
            })
            .collect()
    };
    assert_eq!(values(&after), values(&db));
    // 一定の幅（0.5）は元のまま残り、標高の印が立つ
    let expected = object(
        0x4D,
        0x41,
        |w| {
            entity_common(w, 2, 256, 1.0, 0);
            w.bs(0x200 | 32 | 16 | 8 | 4);
            w.bd(0.5);
            w.bd(3.0);
            for _ in 0..3 {
                w.bits(1, 2);
                w.bytes(&[2]);
            }
            w.rd(2.0);
            w.rd(2.0);
            w.dd(6.0, 2.0);
            w.dd(2.0, 2.0);
            for v in [0.0, 0.25, 0.1, 0.2, 0.3, 0.75] {
                w.bd(v);
            }
        },
        entity_handles_to(0x10, 0),
    );
    let file = read(&out, Limits::default()).0.unwrap();
    let range = file.record(index).unwrap().bytes.unwrap();
    let got = &out[range.start as usize..range.end as usize];
    // ハンドルの流れの後ろの埋めのビットも写すので、データが 1 バイト延びることがある。大きさ（MS）と
    // CRC を除いて、組んだデータに 0 のバイトが続くことを確かめる
    let data = &expected[1..expected.len() - 2];
    assert_eq!(&got[1..=data.len()], data);
    assert!(got[data.len() + 1..got.len() - 2].iter().all(|b| *b == 0));
    assert!(got.len() <= expected.len() + 1);
    // 0 と 1 でない 70 は符号化できない
    let mut changed = objects(&src).0;
    set_value(&mut changed, 0x41, 70, Data::Int(2));
    let mut file = read(&src, Limits::default()).0.unwrap();
    let (out, diag) = written_back(&mut file, &changed);
    assert_eq!(out, Err(Error::Rejected));
    assert_eq!(codes_of(&diag), [codes::DWG_CHANGE_UNSUPPORTED]);
}

#[test]
fn changed_splines_are_written_back() {
    let src = sample_with_more_kinds();
    let (mut db, before) = objects(&src);
    let mut file = read(&src, Limits::default()).0.unwrap();
    let index = |db: &AcadDb, handle| {
        db.objects()
            .iter()
            .position(|o| o.handle() == Some(handle))
            .unwrap()
    };
    // 節点の SPLINE は、制御点を動かして重みを外す。当てはめる点の SPLINE は、点を動かして
    // 終わりの接線を足す
    let knots = index(&db, 0x42);
    let values = &mut db.object_mut(knots).unwrap().values;
    values.retain(|v| v.code != 41);
    for value in values.iter_mut() {
        if let (10 | 20, Data::Real(v)) = (value.code, &mut value.data) {
            *v += 1.0;
        }
    }
    let fit = index(&db, 0x43);
    for value in db.object_mut(fit).unwrap().values.iter_mut() {
        if let (11, Data::Real(v)) = (value.code, &mut value.data) {
            *v -= 1.0;
        }
    }
    for (code, v) in [(13, 0.0), (23, 1.0), (33, 0.0)] {
        set_value(&mut db, 0x43, code, Data::Real(v));
    }
    let (out, diag) = written_back(&mut file, &db);
    let out = out.unwrap();
    assert!(diag.is_empty(), "{:?}", codes_of(&diag));
    let (after, again) = objects(&out);
    assert_eq!(codes_of(&again), codes_of(&before));
    let sorted = |db: &AcadDb| -> Vec<_> {
        db.objects()
            .iter()
            .map(|o| {
                let mut values = o.values.clone();
                values.sort_by_key(|v| v.code);
                values
            })
            .collect()
    };
    assert_eq!(sorted(&after), sorted(&db));
    // 閉じている印は元のまま残り、重みの印は外れる
    let by_knots = object(
        0x24,
        0x42,
        |w| {
            entity_common(w, 2, 256, 1.0, 0);
            for v in [1, 1] {
                w.bits(1, 2);
                w.bytes(&[v]);
            }
            w.bit(false);
            w.bit(true);
            w.bit(false);
            w.bd(1e-9);
            w.bd(1e-10);
            for v in [4, 2] {
                w.bits(1, 2);
                w.bytes(&[v]);
            }
            w.bit(false);
            for v in [0.0, 0.0, 1.0, 1.0] {
                w.bd(v);
            }
            for v in [1.0, 1.0, 0.0, 3.0, 4.0, 0.0] {
                w.bd(v);
            }
        },
        entity_handles_to(0x10, 0),
    );
    let by_fit = object(
        0x24,
        0x43,
        |w| {
            entity_common(w, 2, 256, 1.0, 0);
            for v in [2, 3] {
                w.bits(1, 2);
                w.bytes(&[v]);
            }
            w.bd(0.25);
            for v in [1.0, 0.0, 0.0, 0.0, 1.0, 0.0] {
                w.bd(v);
            }
            w.bits(1, 2);
            w.bytes(&[2]);
            for v in [-1.0, 0.0, 0.0, 4.0, 6.0, 0.0] {
                w.bd(v);
            }
        },
        entity_handles_to(0x10, 0),
    );
    let file = read(&out, Limits::default()).0.unwrap();
    for (index, expected) in [(knots, by_knots), (fit, by_fit)] {
        let range = file.record(index).unwrap().bytes.unwrap();
        let got = &out[range.start as usize..range.end as usize];
        // 埋めのビットを写すので、データが 1 バイト延びることがある（決定 0035）
        let data = &expected[1..expected.len() - 2];
        assert_eq!(&got[1..=data.len()], data);
        assert!(got[data.len() + 1..got.len() - 2].iter().all(|b| *b == 0));
        assert!(got.len() <= expected.len() + 1);
    }
    // 制御点の数と合わない 73 と、場合 1 の当てはめる点は符号化できない
    for (code, data) in [(73, Data::Int(5)), (44, Data::Real(0.5))] {
        let mut changed = objects(&src).0;
        set_value(&mut changed, 0x42, code, data);
        let mut file = read(&src, Limits::default()).0.unwrap();
        let (out, diag) = written_back(&mut file, &changed);
        assert_eq!(out, Err(Error::Rejected));
        assert_eq!(codes_of(&diag), [codes::DWG_CHANGE_UNSUPPORTED]);
    }
}

/// 線分と円弧の辺の境界と、多角形の境界を持つ、模様の HATCH（R2000）。`line_end` は線分の終わりの x、
/// `bulge` は多角形の 2 つめの頂点の膨らみ、`scale` は模様の尺度
#[cfg(test)]
fn hatch_with(line_end: f64, bulge: f64, scale: f64) -> Vec<u8> {
    let bl = |w: &mut BitWriter, n: u8| {
        w.bits(1, 2);
        w.bytes(&[n]);
    };
    object(
        500,
        0x44,
        |w| {
            entity_common(w, 2, 256, 1.0, 0);
            w.bd(2.0);
            for v in [0.0, 0.0, 1.0] {
                w.bd(v);
            }
            w.bs(7);
            w.bytes(b"ANSI31\0");
            w.bit(false);
            w.bit(false);
            bl(w, 2);
            bl(w, 1);
            bl(w, 2);
            w.bytes(&[1]); // 線分
            for v in [0.0, 0.0, line_end, 0.0] {
                w.rd(v);
            }
            w.bytes(&[2]); // 円弧
            w.rd(1.0);
            w.rd(2.0);
            w.bd(3.0);
            w.bd(0.0);
            w.bd(std::f64::consts::FRAC_PI_2);
            w.bit(true);
            w.bl_zero();
            bl(w, 2); // 多角形の境界
            w.bit(true);
            w.bit(true);
            bl(w, 2);
            w.rd(0.0);
            w.rd(0.0);
            w.bd(0.25);
            w.rd(1.0);
            w.rd(0.0);
            w.bd(bulge);
            w.bl_zero();
            w.bs(1);
            w.bs(1);
            w.bd(std::f64::consts::FRAC_PI_4);
            w.bd(scale);
            w.bit(false);
            w.bs(1);
            w.bd(std::f64::consts::FRAC_PI_2);
            for v in [0.1, 0.2, 0.3, 0.4] {
                w.bd(v);
            }
            w.bs(2);
            w.bd(1.0);
            w.bd(-0.5);
            bl(w, 1);
            w.rd(7.0);
            w.rd(8.0);
        },
        entity_handles_to(0x10, 0),
    )
}

#[test]
fn changed_hatches_are_written_back() {
    let mut list = objects_in_sample();
    list.push((0x44, hatch_with(4.0, 0.0, 2.0)));
    let src = with_classes(dwg(&list), &[(500, b"HATCH", true)]);
    let (mut db, before) = objects(&src);
    let mut file = read(&src, Limits::default()).0.unwrap();
    let index = db
        .objects()
        .iter()
        .position(|o| o.handle() == Some(0x44))
        .unwrap();
    // 線分の終わりを動かし、多角形の 2 つめの頂点に膨らみを付け、模様の尺度を変える
    let values = &mut db.object_mut(index).unwrap().values;
    let mut bulges = 0;
    for value in values.iter_mut() {
        match (value.code, &mut value.data) {
            (11, Data::Real(v)) if *v == 4.0 => *v = 5.0,
            (42, Data::Real(v)) => {
                bulges += 1;
                if bulges == 2 {
                    *v = 0.5;
                }
            }
            (41, Data::Real(v)) => *v = 3.0,
            _ => {}
        }
    }
    let (out, diag) = written_back(&mut file, &db);
    let out = out.unwrap();
    assert!(diag.is_empty(), "{:?}", codes_of(&diag));
    let (after, again) = objects(&out);
    assert_eq!(codes_of(&again), codes_of(&before));
    let values =
        |db: &AcadDb| -> Vec<_> { db.objects().iter().map(|o| o.values.clone()).collect() };
    assert_eq!(values(&after), values(&db));
    let expected = hatch_with(5.0, 0.5, 3.0);
    let file = read(&out, Limits::default()).0.unwrap();
    let range = file.record(index).unwrap().bytes.unwrap();
    let got = &out[range.start as usize..range.end as usize];
    // 埋めのビットを写すので、データが 1 バイト延びることがある（決定 0035）
    let data = &expected[1..expected.len() - 2];
    assert_eq!(&got[1..=data.len()], data);
    assert!(got[data.len() + 1..got.len() - 2].iter().all(|b| *b == 0));
    assert!(got.len() <= expected.len() + 1);
    // 0 でない 10 と、境界の図形の数（97）の変更は符号化できない
    for (code, data) in [(10, Data::Real(1.0)), (97, Data::Int(1))] {
        let mut changed = objects(&src).0;
        set_value(&mut changed, 0x44, code, data);
        let mut file = read(&src, Limits::default()).0.unwrap();
        let (out, diag) = written_back(&mut file, &changed);
        assert_eq!(out, Err(Error::Rejected));
        assert_eq!(codes_of(&diag), [codes::DWG_CHANGE_UNSUPPORTED]);
    }
}

/// 2D・3D・多面・網の POLYLINE と VERTEX。`v` は 2D の幅と頂点の x、`closed` は 3D の閉じている印
#[cfg(test)]
fn polylines_with(v: f64, closed: u8) -> Vec<(u64, Vec<u8>)> {
    let entity = |type_code, handle, data: &dyn Fn(&mut BitWriter)| {
        let bytes = object(
            type_code,
            handle,
            |w| {
                entity_common(w, 2, 256, 1.0, 0);
                data(w);
            },
            entity_handles_to(0x10, 0),
        );
        (handle, bytes)
    };
    vec![
        entity(0x0F, 0x50, &|w| {
            w.bs(1);
            w.bs(0);
            w.bd(v);
            w.bd(v);
            w.bit(true);
            w.bd(2.0);
            w.bit(true);
        }),
        entity(0x0A, 0x51, &|w| {
            w.bytes(&[0]);
            for x in [v, 2.0, 0.0] {
                w.bd(x);
            }
            w.bd(-v);
            w.bd(0.5);
            w.bd(std::f64::consts::FRAC_PI_2);
        }),
        entity(0x10, 0x52, &|w| w.bytes(&[1, closed])),
        entity(0x0B, 0x53, &|w| {
            w.bytes(&[32]);
            for x in [v, 1.0, 2.0] {
                w.bd(x);
            }
        }),
        entity(0x1D, 0x54, &|w| {
            w.bs(4);
            w.bs(2);
        }),
        entity(0x0E, 0x55, &|w| {
            for index in [1, 2, -3, 0_i16] {
                w.bs(index as u16);
            }
        }),
        entity(0x1E, 0x56, &|w| {
            for x in [16, 0, 3, 2, 0, 0] {
                w.bs(x);
            }
        }),
    ]
}

#[test]
fn changed_polylines_and_vertices_are_written_back() {
    let mut list = objects_in_sample();
    list.extend(polylines_with(1.0, 1));
    let src = dwg(&list);
    let (mut db, before) = objects(&src);
    let mut file = read(&src, Limits::default()).0.unwrap();
    // 2D の幅と頂点を動かし、3D の POLYLINE を開き、多面の面の番号と網の数を変える
    for (handle, code, data) in [
        (0x50, 40, Data::Real(2.0)),
        (0x50, 41, Data::Real(2.0)),
        (0x51, 10, Data::Real(2.0)),
        (0x51, 40, Data::Real(2.0)),
        (0x51, 41, Data::Real(2.0)),
        (0x52, 70, Data::Int(12)),
        (0x53, 10, Data::Real(2.0)),
    ] {
        set_value(&mut db, handle, code, data);
    }
    let (out, diag) = written_back(&mut file, &db);
    let out = out.unwrap();
    assert!(diag.is_empty(), "{:?}", codes_of(&diag));
    let (after, again) = objects(&out);
    assert_eq!(codes_of(&again), codes_of(&before));
    let values =
        |db: &AcadDb| -> Vec<_> { db.objects().iter().map(|o| o.values.clone()).collect() };
    assert_eq!(values(&after), values(&db));
    let file = read(&out, Limits::default()).0.unwrap();
    for (handle, expected) in polylines_with(2.0, 0) {
        let index = db
            .objects()
            .iter()
            .position(|o| o.handle() == Some(handle))
            .unwrap();
        let range = file.record(index).unwrap().bytes.unwrap();
        let got = &out[range.start as usize..range.end as usize];
        // 埋めのビットを写すので、データが 1 バイト延びることがある（決定 0035）
        let data = &expected[1..expected.len() - 2];
        assert_eq!(&got[1..=data.len()], data, "{handle:#x}");
        assert!(got[data.len() + 1..got.len() - 2].iter().all(|b| *b == 0));
        assert!(got.len() <= expected.len() + 1);
    }
    // 頂点が続く印、2D の 0 でない 10、多面の面の印、3D の 3D でない印の変更は符号化できない
    for (handle, code, data) in [
        (0x50, 66, Data::Int(0)),
        (0x50, 10, Data::Real(1.0)),
        (0x55, 70, Data::Int(0)),
        (0x52, 70, Data::Int(1)),
    ] {
        let mut changed = objects(&src).0;
        set_value(&mut changed, handle, code, data);
        let mut file = read(&src, Limits::default()).0.unwrap();
        let (out, diag) = written_back(&mut file, &changed);
        assert_eq!(out, Err(Error::Rejected));
        assert_eq!(codes_of(&diag), [codes::DWG_CHANGE_UNSUPPORTED]);
    }
}

/// 長さ・直径・座標の DIMENSION と、その画層・寸法の様式・ブロック。`x` は文字の中点の x、`text` は
/// 長さの寸法の利用者の文字、`rotation` はその回転、`radius` は直径の寸法の 40、`flags` は直径の
/// 寸法の印、`ordinate` は座標の寸法の印
#[cfg(test)]
fn dimensions_with(
    x: f64,
    text: &[u8],
    rotation: f64,
    radius: f64,
    flags: u8,
    ordinate: u8,
) -> Vec<(u64, Vec<u8>)> {
    let common = |w: &mut BitWriter, flags: u8, text: &[u8]| {
        for v in [0.0, 0.0, 1.0] {
            w.bd(v);
        }
        w.rd(x);
        w.rd(6.0);
        w.bd(1.5);
        w.bytes(&[flags]);
        if text.is_empty() {
            w.bs(0);
        } else {
            w.bs(text.len() as u16 + 1);
            w.bytes(text);
            w.bytes(&[0]);
        }
        // 挿入の尺度と回転は 2・3・4 と 0.5。表にはない
        for v in [0.0, 0.0, 2.0, 3.0, 4.0, 0.5] {
            w.bd(v);
        }
        w.bs(5);
        w.bs(1);
        w.bd(1.0);
        w.bd(10.0);
        w.rd(0.0);
        w.rd(0.0);
    };
    let handles = |w: &mut BitWriter| {
        entity_handles(0x10)(w);
        w.handle(5, 0x11);
        w.handle(5, 0x12);
    };
    let dimension = |type_code, handle, data: &dyn Fn(&mut BitWriter)| {
        let bytes = object(
            type_code,
            handle,
            |w| {
                entity_common(w, 2, 256, 1.0, 0);
                data(w);
            },
            handles,
        );
        (handle, bytes)
    };
    vec![
        (
            0x10,
            object(0x33, 0x10, record_named(b"0"), record_handles(2)),
        ),
        (
            0x11,
            object(0x45, 0x11, record_named(b"ISO-25"), record_handles(3)),
        ),
        (
            0x12,
            object(0x31, 0x12, record_named(b"*D1"), record_handles(1)),
        ),
        dimension(0x15, 0x20, &|w| {
            common(w, 3, text);
            for v in [0.0, 0.0, 0.0, 10.0, 0.0, 0.0, 10.0, 4.0, 0.0] {
                w.bd(v);
            }
            w.bd(0.0);
            w.bd(rotation);
        }),
        dimension(0x1A, 0x21, &|w| {
            common(w, flags, b"");
            for v in [-5.0, 0.0, 0.0, 5.0, 0.0, 0.0] {
                w.bd(v);
            }
            w.bd(radius);
        }),
        dimension(0x14, 0x22, &|w| {
            common(w, 1, b"");
            for v in [0.0, 0.0, 0.0, 1.0, 2.0, 0.0, 3.0, 4.0, 0.0] {
                w.bd(v);
            }
            w.bytes(&[ordinate]);
        }),
    ]
}

#[test]
fn changed_dimensions_are_written_back() {
    let src = dwg(&dimensions_with(
        5.0,
        b"<> mm",
        std::f64::consts::FRAC_PI_4,
        0.0,
        2,
        1,
    ));
    let (mut db, before) = objects(&src);
    let mut file = read(&src, Limits::default()).0.unwrap();
    // 文字の中点・利用者の文字・回転と、直径・印を変える
    for (handle, code, data) in [
        (0x20, 11, Data::Real(7.0)),
        (0x21, 11, Data::Real(7.0)),
        (0x22, 11, Data::Real(7.0)),
        (0x20, 1, Data::Text(b"<> cm".to_vec())),
        (0x20, 50, Data::Real(90.0)),
        (0x21, 40, Data::Real(2.0)),
        (0x21, 70, Data::Int(35)),
        (0x22, 70, Data::Int(6)),
    ] {
        set_value(&mut db, handle, code, data);
    }
    let (out, diag) = written_back(&mut file, &db);
    let out = out.unwrap();
    assert!(diag.is_empty(), "{:?}", codes_of(&diag));
    let (after, again) = objects(&out);
    assert_eq!(codes_of(&again), codes_of(&before));
    let values =
        |db: &AcadDb| -> Vec<_> { db.objects().iter().map(|o| o.values.clone()).collect() };
    assert_eq!(values(&after), values(&db));
    let file = read(&out, Limits::default()).0.unwrap();
    for (handle, expected) in dimensions_with(7.0, b"<> cm", std::f64::consts::FRAC_PI_2, 2.0, 3, 0)
    {
        let index = db
            .objects()
            .iter()
            .position(|o| o.handle() == Some(handle))
            .unwrap();
        let range = file.record(index).unwrap().bytes.unwrap();
        let got = &out[range.start as usize..range.end as usize];
        // 埋めのビットを写すので、データが 1 バイト延びることがある（決定 0035）
        let data = &expected[1..expected.len() - 2];
        assert_eq!(&got[1..=data.len()], data, "{handle:#x}");
        assert!(got[data.len() + 1..got.len() - 2].iter().all(|b| *b == 0));
        assert!(got.len() <= expected.len() + 1);
    }
    // 直径の寸法の回転、型と合わない種類、標高と違う 12 の点の Z は符号化できない
    for (handle, code, data) in [
        (0x21, 50, Data::Real(30.0)),
        (0x20, 70, Data::Int(33)),
        (0x20, 32, Data::Real(2.0)),
    ] {
        let mut changed = objects(&src).0;
        set_value(&mut changed, handle, code, data);
        let mut file = read(&src, Limits::default()).0.unwrap();
        let (out, diag) = written_back(&mut file, &changed);
        assert_eq!(out, Err(Error::Rejected));
        assert_eq!(codes_of(&diag), [codes::DWG_CHANGE_UNSUPPORTED]);
    }
}

/// 公差・引出線・多重線を持つ図面。`x` と `text` は公差の挿入点と文字、`height` と `hook` は引出線の注釈の
/// 高さと鉤の線の向き、`scale` と `start` は多重線の尺度と初めの頂点の X
#[cfg(test)]
fn annotations_with(
    x: f64,
    text: &[u8],
    height: f64,
    hook: bool,
    scale: f64,
    start: f64,
) -> Vec<(u64, Vec<u8>)> {
    let style = object(
        0x49,
        0x31,
        |w| {
            w.bl_zero();
            w.bs(9);
            w.bytes(b"STANDARD\0");
            w.bs(0);
            w.bs(33);
            w.bs(256);
            w.bd(std::f64::consts::FRAC_PI_2);
            w.bd(std::f64::consts::FRAC_PI_2);
            w.bytes(&[1]);
            w.bd(0.5);
            w.bs(1);
            w.bs(0);
        },
        |w| w.handle(4, 0x30),
    );
    let tolerance = object(
        0x2E,
        0x20,
        |w| {
            entity_common(w, 2, 256, 1.0, 0);
            for v in [x, 4.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0] {
                w.bd(v);
            }
            w.bs(text.len() as u16 + 1);
            w.bytes(text);
            w.bytes(&[0]);
        },
        |w| {
            entity_handles(0x10)(w);
            w.handle(5, 0x11);
        },
    );
    let leader = object(
        0x2D,
        0x21,
        |w| {
            entity_common(w, 2, 256, 1.0, 0);
            w.bit(false);
            w.bs(0);
            w.bs(0);
            w.bs(2);
            for v in [0.0, 0.0, 0.0, 5.0, 5.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0] {
                w.bd(v);
            }
            for v in [1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0] {
                w.bd(v);
            }
            w.bd(height);
            w.bd(10.0);
            w.bit(hook);
            // 表にない後ろの欄
            w.bit(true);
            w.bs(0);
            w.bit(false);
            w.bit(false);
        },
        |w| {
            entity_handles(0x10)(w);
            w.handle(5, 0x30);
            w.handle(5, 0x11);
        },
    );
    let mline = object(
        0x2F,
        0x22,
        |w| {
            entity_common(w, 2, 256, 1.0, 0);
            w.bd(scale);
            w.bytes(&[1]);
            for v in [0.0, 0.0, 0.0, 0.0, 0.0, 1.0] {
                w.bd(v);
            }
            w.bs(1);
            w.bytes(&[1]);
            w.bs(2);
            for x in [start, 10.0] {
                for v in [x, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0] {
                    w.bd(v);
                }
                w.bs(2);
                w.bd(0.5);
                w.bd(0.0);
                w.bs(0);
            }
        },
        |w| {
            entity_handles(0x10)(w);
            w.handle(5, 0x31);
        },
    );
    vec![
        (
            0x10,
            object(0x33, 0x10, record_named(b"0"), record_handles(2)),
        ),
        (
            0x11,
            object(0x45, 0x11, record_named(b"ISO-25"), record_handles(3)),
        ),
        (0x20, tolerance),
        (0x21, leader),
        (0x22, mline),
        (0x31, style),
    ]
}

#[test]
fn changed_annotations_are_written_back() {
    let src = dwg(&annotations_with(3.0, b"{\\Fgd\n", 2.5, true, 1.5, 0.0));
    let (mut db, before) = objects(&src);
    let mut file = read(&src, Limits::default()).0.unwrap();
    // 公差の挿入点と文字、引出線の高さと鉤の線の向き、多重線の尺度と初めの頂点を変える
    for (handle, code, data) in [
        (0x20, 10, Data::Real(7.0)),
        (0x20, 1, Data::Text(b"{\\Fgdt;n}^J0.1".to_vec())),
        (0x21, 40, Data::Real(3.0)),
        (0x21, 74, Data::Int(0)),
        (0x22, 40, Data::Real(2.0)),
        (0x22, 11, Data::Real(-4.0)),
    ] {
        set_value(&mut db, handle, code, data);
    }
    let (out, diag) = written_back(&mut file, &db);
    let out = out.unwrap();
    assert!(diag.is_empty(), "{:?}", codes_of(&diag));
    let (after, again) = objects(&out);
    assert_eq!(codes_of(&again), codes_of(&before));
    let values =
        |db: &AcadDb| -> Vec<_> { db.objects().iter().map(|o| o.values.clone()).collect() };
    assert_eq!(values(&after), values(&db));
    let file = read(&out, Limits::default()).0.unwrap();
    for (handle, expected) in annotations_with(7.0, b"{\\Fgdt;n}\n0.1", 3.0, false, 2.0, -4.0) {
        let index = db
            .objects()
            .iter()
            .position(|o| o.handle() == Some(handle))
            .unwrap();
        let range = file.record(index).unwrap().bytes.unwrap();
        let got = &out[range.start as usize..range.end as usize];
        // 埋めのビットを写すので、データが 1 バイト延びることがある（決定 0035）
        let data = &expected[1..expected.len() - 2];
        assert_eq!(&got[1..=data.len()], data, "{handle:#x}");
        assert!(got[data.len() + 1..got.len() - 2].iter().all(|b| *b == 0));
        assert!(got.len() <= expected.len() + 1);
    }
    // 0 か 1 でない鉤の線の向き、線の数と合わない区切り、表の頂点の数と合わない頂点の数は符号化できない
    for (handle, code, data) in [
        (0x21, 74, Data::Int(2)),
        (0x22, 73, Data::Int(2)),
        (0x22, 72, Data::Int(3)),
    ] {
        let mut changed = objects(&src).0;
        set_value(&mut changed, handle, code, data);
        let mut file = read(&src, Limits::default()).0.unwrap();
        let (out, diag) = written_back(&mut file, &changed);
        assert_eq!(out, Err(Error::Rejected));
        assert_eq!(codes_of(&diag), [codes::DWG_CHANGE_UNSUPPORTED]);
    }
}

/// ビューポートを持つ図面。`width` は幅、`twist` はねじれ（ラジアン）、`x` は 12 の X、`circle` は円の
/// 細かさ（72）、`at_origin` は UCS が原点にあるか（74）
#[cfg(test)]
fn viewports_with(
    width: f64,
    twist: f64,
    x: f64,
    circle: u16,
    at_origin: bool,
) -> Vec<(u64, Vec<u8>)> {
    let viewport = object(
        0x22,
        0x20,
        |w| {
            entity_common(w, 2, 256, 1.0, 0);
            for v in [5.0, 4.0, 0.0, width, 6.0] {
                w.bd(v);
            }
            for v in [1.0, 2.0, 3.0, 0.0, 0.0, 1.0] {
                w.bd(v);
            }
            for v in [twist, 9.0, 50.0, 0.0, 0.0, 0.0] {
                w.bd(v);
            }
            for v in [x, 4.5, 0.0, 0.0, 0.5, 0.5, 0.5, 0.5] {
                w.rd(v);
            }
            w.bs(circle);
            w.bl_zero();
            w.bl_zero();
            w.bs(0);
            w.bytes(&[0]);
            w.bit(at_origin);
            w.bit(true);
            for v in [0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0] {
                w.bd(v);
            }
            w.bs(0);
        },
        entity_handles(0x10),
    );
    vec![
        (
            0x10,
            object(0x33, 0x10, record_named(b"0"), record_handles(2)),
        ),
        (0x20, viewport),
    ]
}

#[test]
fn changed_viewports_are_written_back() {
    let src = dwg(&viewports_with(
        8.0,
        std::f64::consts::FRAC_PI_2,
        6.0,
        1000,
        false,
    ));
    let (mut db, before) = objects(&src);
    let mut file = read(&src, Limits::default()).0.unwrap();
    // 幅・ねじれ・左下の角の X・円の細かさ・UCS が原点にあるかを変える
    for (code, data) in [
        (40, Data::Real(10.0)),
        (51, Data::Real(0.0)),
        (12, Data::Real(7.0)),
        (72, Data::Int(500)),
        (74, Data::Int(1)),
    ] {
        set_value(&mut db, 0x20, code, data);
    }
    let (out, diag) = written_back(&mut file, &db);
    let out = out.unwrap();
    assert!(diag.is_empty(), "{:?}", codes_of(&diag));
    let (after, again) = objects(&out);
    assert_eq!(codes_of(&again), codes_of(&before));
    let values =
        |db: &AcadDb| -> Vec<_> { db.objects().iter().map(|o| o.values.clone()).collect() };
    assert_eq!(values(&after), values(&db));
    let file = read(&out, Limits::default()).0.unwrap();
    for (handle, expected) in viewports_with(10.0, 0.0, 7.0, 500, true) {
        let index = db
            .objects()
            .iter()
            .position(|o| o.handle() == Some(handle))
            .unwrap();
        let range = file.record(index).unwrap().bytes.unwrap();
        let got = &out[range.start as usize..range.end as usize];
        // 埋めのビットを写すので、データが 1 バイト延びることがある（決定 0035）
        let data = &expected[1..expected.len() - 2];
        assert_eq!(&got[1..=data.len()], data, "{handle:#x}");
        assert!(got[data.len() + 1..got.len() - 2].iter().all(|b| *b == 0));
        assert!(got.len() <= expected.len() + 1);
    }
    // 0 か 1 でない 74、RC に収まらない描き方（281）は符号化できない
    for (code, data) in [(74, Data::Int(2)), (281, Data::Int(300))] {
        let mut changed = objects(&src).0;
        set_value(&mut changed, 0x20, code, data);
        let mut file = read(&src, Limits::default()).0.unwrap();
        let (out, diag) = written_back(&mut file, &changed);
        assert_eq!(out, Err(Error::Rejected));
        assert_eq!(codes_of(&diag), [codes::DWG_CHANGE_UNSUPPORTED]);
    }
}

/// 形を持つ図面。`height` は高さ、`rotation` は回転（ラジアン）、`width` は幅の尺度、`thickness` は厚さ
#[cfg(test)]
fn shapes_with(height: f64, rotation: f64, width: f64, thickness: f64) -> Vec<(u64, Vec<u8>)> {
    let shape = object(
        0x21,
        0x20,
        |w| {
            entity_common(w, 2, 256, 1.0, 0);
            for v in [1.0, 2.0, 0.0, height, rotation, width, 0.0, thickness] {
                w.bd(v);
            }
            w.bs(3); // 形の番号
            for v in [0.0, 0.0, 1.0] {
                w.bd(v);
            }
        },
        entity_handles(0x10),
    );
    vec![
        (
            0x10,
            object(0x33, 0x10, record_named(b"0"), record_handles(2)),
        ),
        (0x20, shape),
    ]
}

#[test]
fn changed_shapes_are_written_back() {
    let src = dwg(&shapes_with(0.5, 0.0, 1.0, 0.0));
    let (mut db, before) = objects(&src);
    let mut file = read(&src, Limits::default()).0.unwrap();
    // 高さを変え、既定の値で表になかった回転・幅の尺度・厚さを足す
    for (code, data) in [
        (40, Data::Real(2.0)),
        (50, Data::Real(90.0)),
        (41, Data::Real(3.0)),
        (39, Data::Real(0.25)),
    ] {
        set_value(&mut db, 0x20, code, data);
    }
    let (out, diag) = written_back(&mut file, &db);
    let out = out.unwrap();
    assert!(diag.is_empty(), "{:?}", codes_of(&diag));
    let (after, again) = objects(&out);
    assert_eq!(codes_of(&again), codes_of(&before));
    let shape = after.get(0x20).unwrap();
    for (code, value) in [(40, 2.0), (50, 90.0), (41, 3.0), (39, 0.25)] {
        assert!(close(shape.real(code), value), "{code}");
    }
    let file = read(&out, Limits::default()).0.unwrap();
    for (handle, expected) in shapes_with(2.0, std::f64::consts::FRAC_PI_2, 3.0, 0.25) {
        let index = db
            .objects()
            .iter()
            .position(|o| o.handle() == Some(handle))
            .unwrap();
        let range = file.record(index).unwrap().bytes.unwrap();
        let got = &out[range.start as usize..range.end as usize];
        // 埋めのビットを写すので、データが 1 バイト延びることがある（決定 0035）
        let data = &expected[1..expected.len() - 2];
        assert_eq!(&got[1..=data.len()], data, "{handle:#x}");
        assert!(got[data.len() + 1..got.len() - 2].iter().all(|b| *b == 0));
        assert!(got.len() <= expected.len() + 1);
    }
    // 3 つそろわない押し出しの向きは符号化できない
    let mut changed = objects(&src).0;
    set_value(&mut changed, 0x20, 210, Data::Real(1.0));
    let mut file = read(&src, Limits::default()).0.unwrap();
    let (out, diag) = written_back(&mut file, &changed);
    assert_eq!(out, Err(Error::Rejected));
    assert_eq!(codes_of(&diag), [codes::DWG_CHANGE_UNSUPPORTED]);
}

/// `handle` の図形に、`code` の共通の欄の値を足す。読む側は、共通の欄を画層（8）の後に置く
#[cfg(test)]
fn insert_after_layer(db: &mut AcadDb, handle: u64, code: i16, data: Data) {
    let index = db
        .objects()
        .iter()
        .position(|o| o.handle() == Some(handle))
        .unwrap();
    let values = &mut db.object_mut(index).unwrap().values;
    let at = values.iter().position(|v| v.code == 8).unwrap() + 1;
    values.insert(at, Value { code, data });
}

#[test]
fn changed_colors_ltype_scales_and_visibility_are_written_back() {
    let src = sample_with_more_kinds();
    let (mut db, before) = objects(&src);
    let mut file = read(&src, Limits::default()).0.unwrap();
    // LINE の色と線種の尺度と非表示を変え、終わりの点も変える
    set_value(&mut db, 0x2B, 62, Data::Int(256));
    set_value(&mut db, 0x2B, 48, Data::Real(0.5));
    set_value(&mut db, 0x2B, 60, Data::Int(0));
    set_value(&mut db, 0x2B, 11, Data::Real(7.0));
    // SHAPE の共通の欄も書き戻せる
    let index = db
        .objects()
        .iter()
        .position(|o| o.handle() == Some(0x39))
        .unwrap();
    db.object_mut(index)
        .unwrap()
        .values
        .retain(|v| !matches!(v.code, 62 | 48));
    insert_after_layer(&mut db, 0x39, 60, Data::Int(1));
    insert_after_layer(&mut db, 0x2A, 62, Data::Int(200));
    let (out, diag) = written_back(&mut file, &db);
    let out = out.unwrap();
    assert!(diag.is_empty(), "{:?}", codes_of(&diag));
    let (after, again) = objects(&out);
    assert_eq!(codes_of(&again), codes_of(&before));
    let values =
        |db: &AcadDb| -> Vec<_> { db.objects().iter().map(|o| o.values.clone()).collect() };
    let mut expected = values(&db);
    // 既定の値（ByLayer の 256、非表示でない 0）は、読み直すと表に置かない
    for (object, values) in db.objects().iter().zip(&mut expected) {
        if object.handle() == Some(0x2B) {
            values.retain(|v| {
                !matches!((v.code, &v.data), (62, Data::Int(256)) | (60, Data::Int(0)))
            });
        }
    }
    assert_eq!(values(&after), expected);
    // 2 度ある SHAPE の高さ（40）は符号化できない
    let (db, _) = objects(&src);
    let mut changed = db.clone();
    let index = changed
        .objects()
        .iter()
        .position(|o| o.handle() == Some(0x39))
        .unwrap();
    changed.object_mut(index).unwrap().values.push(Value {
        code: 40,
        data: Data::Real(3.0),
    });
    let mut file = read(&src, Limits::default()).0.unwrap();
    let (out, diag) = written_back(&mut file, &changed);
    assert_eq!(out, Err(Error::Rejected));
    assert_eq!(codes_of(&diag), [codes::DWG_CHANGE_UNSUPPORTED]);
}

#[test]
fn unchanged_tables_leave_the_file_as_it_was() {
    let src = sample_with_curves();
    let (db, _) = objects(&src);
    let mut file = read(&src, Limits::default()).0.unwrap();
    let (out, diag) = written_back(&mut file, &db);
    assert!(diag.is_empty(), "{:?}", codes_of(&diag));
    assert_eq!(out.unwrap(), src);
    let r2004_src = r2004(&r2004_sections());
    let mut r2004 = read(&r2004_src, Limits::default()).0.unwrap();
    let mut diag = Diagnostics::new();
    let db = r2004
        .objects(&mut Cx::new(
            &mut Budget::default(),
            &mut diag,
            &CancelToken::new(),
        ))
        .unwrap();
    let (out, diag) = written_back(&mut r2004, &db);
    assert!(diag.is_empty(), "{:?}", codes_of(&diag));
    assert_eq!(out.unwrap(), r2004_src);
}

/// 表の変え方
#[cfg(test)]
type Edit = fn(&mut AcadDb);

#[test]
fn changes_that_cannot_be_encoded_are_rejected_without_changing_the_file() {
    let src = sample_with_curves();
    let (db, _) = objects(&src);
    let mut file = read(&src, Limits::default()).0.unwrap();
    let edits: [(&str, Edit); 8] = [
        ("LAYER", |db| {
            set_value(db, 0x10, 2, Data::Text(b"WALL".to_vec()))
        }),
        ("LINE", |db| set_value(db, 0x2A, 420, Data::Int(0xFF0000))),
        ("LINE", |db| set_value(db, 0x2A, 62, Data::Int(1 << 20))),
        ("LINE", |db| set_value(db, 0x2A, 10, Data::Int(1))),
        ("CIRCLE", |db| {
            db.remove(6);
        }),
        // HANDSEED を持たないファイルには、図形を足せない
        ("HANDSEED", |db| {
            let mut line = db.objects()[2].clone();
            line.origin = Origin::New;
            db.push(line);
        }),
        ("ENTITIES", |db| {
            let line = db.remove(2).unwrap();
            db.push(line);
        }),
        ("$INSUNITS", |db| {
            db.set_variable(
                b"$INSUNITS",
                vec![Value {
                    code: 70,
                    data: Data::Int(4),
                }],
            );
        }),
    ];
    for (kind, edit) in edits {
        let mut changed = db.clone();
        edit(&mut changed);
        let (result, diag) = written_back(&mut file, &changed);
        assert_eq!(result, Err(Error::Rejected), "{kind}");
        assert_eq!(codes_of(&diag), [codes::DWG_CHANGE_UNSUPPORTED], "{kind}");
        let found = diag.iter().next().and_then(|d| d.param("kind"));
        assert_eq!(found, Some(&kind.to_string().into()));
        let (out, _) = edited(&mut file, &[]);
        assert_eq!(out.unwrap(), src);
    }
    let r2004_src = r2004(&r2004_sections());
    let mut r2004 = read(&r2004_src, Limits::default()).0.unwrap();
    let mut diag = Diagnostics::new();
    let mut db = r2004
        .objects(&mut Cx::new(
            &mut Budget::default(),
            &mut diag,
            &CancelToken::new(),
        ))
        .unwrap();
    db.set_variable(
        b"$INSUNITS",
        vec![Value {
            code: 70,
            data: Data::Int(4),
        }],
    );
    let (result, diag) = written_back(&mut r2004, &db);
    assert_eq!(result, Err(Error::Rejected));
    assert_eq!(codes_of(&diag), [codes::DWG_EDIT_UNSUPPORTED]);
}

/// 印と色を持つ画層と、縦書きかどうかと高さとフォントを持つ文字スタイル
#[cfg(test)]
fn records_with(
    flags: u16,
    color: u16,
    vertical: bool,
    height: f64,
    font: &'static [u8],
) -> Vec<(u64, Vec<u8>)> {
    let text = |w: &mut BitWriter, text: &[u8]| {
        w.bs(text.len() as u16 + 1);
        w.bytes(text);
        w.bytes(&[0]);
    };
    let common = move |w: &mut BitWriter, name: &[u8]| {
        w.bl_zero();
        text(w, name);
        w.bit(false);
        w.bs(0);
        w.bit(false);
    };
    let layer = object(
        0x33,
        0x10,
        move |w| {
            common(w, b"WALL");
            w.bs(flags);
            w.bs(color);
        },
        |w| {
            w.handle(4, 2);
            w.handle(3, 0);
            w.handle(5, 0);
            w.handle(5, 0);
            w.handle(5, 0);
        },
    );
    let style = object(
        0x35,
        0x11,
        move |w| {
            common(w, b"NOTE");
            w.bit(false);
            w.bit(vertical);
            w.bd(height);
            w.bd(0.8);
            w.bd(0.0);
            w.bytes(&[0]);
            w.bd(0.2);
            text(w, font);
            text(w, b"");
        },
        |w| {
            w.handle(4, 3);
            w.handle(3, 0);
            w.handle(5, 0);
        },
    );
    vec![(0x10, layer), (0x11, style)]
}

#[test]
fn changed_layers_and_styles_are_written_back() {
    // 印刷する画層で、線の太さは既定（番号 31）
    let src = dwg(&records_with(16 | (31 << 5), 7, false, 0.0, b"txt"));
    let (mut db, before) = objects(&src);
    let mut file = read(&src, Limits::default()).0.unwrap();
    // 凍結してロックし、表示しないで印刷しない。線の太さは 0.50 mm
    set_value(&mut db, 0x10, 70, Data::Int(1 | 4));
    set_value(&mut db, 0x10, 62, Data::Int(-3));
    set_value(&mut db, 0x10, 290, Data::Bool(false));
    set_value(&mut db, 0x10, 370, Data::Int(50));
    // 縦書きにし、高さとフォントを変える
    set_value(&mut db, 0x11, 70, Data::Int(4));
    set_value(&mut db, 0x11, 40, Data::Real(2.5));
    set_value(&mut db, 0x11, 3, Data::Text(b"romans".to_vec()));
    let (out, diag) = written_back(&mut file, &db);
    let out = out.unwrap();
    assert!(diag.is_empty(), "{:?}", codes_of(&diag));
    let (after, again) = objects(&out);
    assert_eq!(codes_of(&again), codes_of(&before));
    // 足した 290 は表の終わりに置いたので、番号の順に並べて比べる
    let sorted = |db: &AcadDb, handle: u64| {
        let mut values = db.get(handle).unwrap().values.clone();
        values.sort_by_key(|v| v.code);
        values
    };
    for handle in [0x10, 0x11] {
        assert_eq!(sorted(&after, handle), sorted(&db, handle), "{handle:#x}");
    }
    let file = read(&out, Limits::default()).0.unwrap();
    for (handle, expected) in records_with(1 | 2 | 8 | (11 << 5), 3, true, 2.5, b"romans") {
        let index = db
            .objects()
            .iter()
            .position(|o| o.handle() == Some(handle))
            .unwrap();
        let range = file.record(index).unwrap().bytes.unwrap();
        let got = &out[range.start as usize..range.end as usize];
        // 埋めのビットを写すので、データが 1 バイト延びることがある（決定 0035）
        let data = &expected[1..expected.len() - 2];
        assert_eq!(&got[1..=data.len()], data, "{handle:#x}");
        assert!(got[data.len() + 1..got.len() - 2].iter().all(|b| *b == 0));
        assert!(got.len() <= expected.len() + 1);
    }
    // 名前・外部参照に依る印・表にない線の太さ・形のファイルでも縦書きでもない STYLE の印は受け付けない
    for (handle, code, data) in [
        (0x10, 2, Data::Text(b"DOOR".to_vec())),
        (0x10, 70, Data::Int(16)),
        (0x10, 370, Data::Int(7)),
        (0x11, 70, Data::Int(2)),
    ] {
        let mut changed = objects(&src).0;
        set_value(&mut changed, handle, code, data);
        let mut file = read(&src, Limits::default()).0.unwrap();
        let (out, diag) = written_back(&mut file, &changed);
        assert_eq!(out, Err(Error::Rejected), "{handle:#x} {code}");
        assert_eq!(codes_of(&diag), [codes::DWG_CHANGE_UNSUPPORTED]);
    }
}

/// 線の要素と、文字の要素を 1 つずつ持つ線種
#[cfg(test)]
fn ltypes_with(
    description: &'static [u8],
    length: f64,
    first: f64,
    scale: f64,
    rotation: f64,
    text: &'static [u8],
) -> Vec<(u64, Vec<u8>)> {
    let ltype = object(
        0x39,
        0x12,
        move |w| {
            w.bl_zero();
            for name in [b"DASHED".as_slice(), description] {
                w.bs(name.len() as u16 + 1);
                w.bytes(name);
                w.bytes(&[0]);
                if name == b"DASHED" {
                    w.bit(false);
                    w.bs(0);
                    w.bit(false);
                }
            }
            w.bd(length);
            w.bytes(&[65, 2]);
            for (dash, x, y, scale, rotation, flags) in [
                (first, 0.0, 0.0, 1.0, 0.0, 0),
                (-0.5, 0.1, -0.2, scale, rotation, 2),
            ] {
                w.bd(dash);
                w.bs(0);
                w.rd(x);
                w.rd(y);
                w.bd(scale);
                w.bd(rotation);
                w.bs(flags);
            }
            let mut area = [0; 256];
            area[..text.len()].copy_from_slice(text);
            w.bytes(&area);
        },
        |w| {
            w.handle(4, 5);
            w.handle(3, 0);
            w.handle(5, 0);
            w.handle(5, 0);
            w.handle(5, 0x11);
        },
    );
    vec![(0x12, ltype)]
}

#[test]
fn changed_ltypes_are_written_back() {
    let src = dwg(&ltypes_with(b"__ AB", 1.5, 1.0, 0.5, 0.0, b"AB"));
    let (mut db, before) = objects(&src);
    let mut file = read(&src, Limits::default()).0.unwrap();
    // 説明と長さと線の要素の長さを変え、文字の要素の尺度・回転・文字列を変える
    set_value(&mut db, 0x12, 3, Data::Text(b"-- XYZ".to_vec()));
    set_value(&mut db, 0x12, 40, Data::Real(2.0));
    set_value(&mut db, 0x12, 49, Data::Real(1.25));
    set_value(&mut db, 0x12, 46, Data::Real(0.75));
    set_value(&mut db, 0x12, 50, Data::Real(90.0));
    set_value(&mut db, 0x12, 9, Data::Text(b"XYZ".to_vec()));
    let (out, diag) = written_back(&mut file, &db);
    let out = out.unwrap();
    assert!(diag.is_empty(), "{:?}", codes_of(&diag));
    let (after, again) = objects(&out);
    assert_eq!(codes_of(&again), codes_of(&before));
    let (got, want) = (after.get(0x12).unwrap(), db.get(0x12).unwrap());
    assert_eq!(got.values.len(), want.values.len());
    for (a, b) in got.values.iter().zip(&want.values) {
        assert_eq!(a.code, b.code);
        match (&a.data, &b.data) {
            (Data::Real(x), Data::Real(y)) => assert!(close(Some(Some(*x)), *y), "{}", a.code),
            (x, y) => assert_eq!(x, y, "{}", a.code),
        }
    }
    let file = read(&out, Limits::default()).0.unwrap();
    let expected = &ltypes_with(
        b"-- XYZ",
        2.0,
        1.25,
        0.75,
        std::f64::consts::FRAC_PI_2,
        b"XYZ",
    )[0]
    .1;
    let index = db
        .objects()
        .iter()
        .position(|o| o.handle() == Some(0x12))
        .unwrap();
    let range = file.record(index).unwrap().bytes.unwrap();
    let got = &out[range.start as usize..range.end as usize];
    // 埋めのビットを写すので、データが 1 バイト延びることがある（決定 0035）
    let data = &expected[1..expected.len() - 2];
    assert_eq!(&got[1..=data.len()], data);
    assert!(got[data.len() + 1..got.len() - 2].iter().all(|b| *b == 0));
    assert!(got.len() <= expected.len() + 1);
    // 要素の数・形と文字の印・文字の要素の 0 でない 75・領域に収まらない文字列・外部参照に依る印は受け付けない
    for (code, data) in [
        (73, Data::Int(3)),
        (74, Data::Int(4)),
        (75, Data::Int(1)),
        (9, Data::Text(vec![b'A'; 256])),
        (70, Data::Int(16)),
    ] {
        let mut changed = objects(&src).0;
        let index = changed
            .objects()
            .iter()
            .position(|o| o.handle() == Some(0x12))
            .unwrap();
        let values = &mut changed.object_mut(index).unwrap().values;
        // 74 と 75 は、文字の要素のものを変える
        let value = values.iter_mut().filter(|v| v.code == code).last().unwrap();
        value.data = data;
        let mut file = read(&src, Limits::default()).0.unwrap();
        let (out, diag) = written_back(&mut file, &changed);
        assert_eq!(out, Err(Error::Rejected), "{code}");
        assert_eq!(codes_of(&diag), [codes::DWG_CHANGE_UNSUPPORTED]);
    }
}

/// UCS を伴う VIEW と、UCS と、VPORT。`height` は VIEW と VPORT の高さ、`twist` は VIEW のねじれ（ラジアン）、
/// `origin` は UCS の原点の X、`icon` は VPORT の UCS のアイコンの印
#[cfg(test)]
fn views_with(height: f64, twist: f64, origin: f64, icon: u8) -> Vec<(u64, Vec<u8>)> {
    fn common(w: &mut BitWriter, name: &[u8]) {
        w.bl_zero();
        w.bs(name.len() as u16 + 1);
        w.bytes(name);
        w.bytes(&[0]);
        w.bit(false);
        w.bs(0);
        w.bit(false);
    }
    fn handles(count: usize) -> impl Fn(&mut BitWriter) {
        move |w| {
            w.handle(4, 2);
            w.handle(3, 0);
            w.handle(5, 0);
            for _ in 0..count {
                w.handle(5, 0);
            }
        }
    }
    let axes = |w: &mut BitWriter| {
        for v in [1.0, 2.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0] {
            w.bd(v);
        }
        w.bd(0.5); // 高さ
        w.bs(1); // 正投影の種類
    };
    let view = object(
        0x3D,
        0x10,
        move |w| {
            common(w, b"FRONT");
            w.bd(height);
            w.bd(6.0);
            w.rd(3.0);
            w.rd(4.0);
            for v in [0.0, 0.0, 0.0, 0.0, 0.0, 1.0, twist, 50.0, 0.0, 0.0] {
                w.bd(v);
            }
            for bit in [false, false, false, true] {
                w.bit(bit);
            }
            w.bytes(&[0]);
            w.bit(false); // 紙空間の印
            w.bit(true); // UCS を伴う
            axes(w);
        },
        handles(2),
    );
    let ucs = object(
        0x3F,
        0x11,
        move |w| {
            common(w, b"TILT");
            for v in [origin, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 2.0] {
                w.bd(v);
            }
            w.bs(0);
            w.bs(0); // 正投影の型
        },
        handles(2),
    );
    let vport = object(
        0x41,
        0x12,
        move |w| {
            common(w, b"*ACTIVE");
            w.bd(height);
            w.bd(height * 2.0);
            w.rd(5.0);
            w.rd(6.0);
            for v in [0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 50.0, 0.0, 0.0] {
                w.bd(v);
            }
            for bit in [false, false, false, true] {
                w.bit(bit);
            }
            w.bytes(&[0]);
            for v in [0.0, 0.0, 1.0, 1.0] {
                w.rd(v);
            }
            w.bit(false); // UCS に従う
            w.bs(100);
            w.bit(true);
            w.bits(u64::from(icon), 2);
            w.bit(false);
            w.rd(0.5);
            w.rd(0.5);
            w.bit(false);
            w.bit(false);
            w.bs(0);
            w.bd(0.0);
            for v in [0.0, 0.0, 0.5, 0.5] {
                w.rd(v);
            }
            w.bit(true); // 使い道の分からない B
            w.bit(true); // ビューポートごとの UCS
            axes(w);
        },
        handles(2),
    );
    vec![(0x10, view), (0x11, ucs), (0x12, vport)]
}

#[test]
fn changed_views_ucss_and_vports_are_written_back() {
    let src = dwg(&views_with(4.0, 0.0, 1.0, 3));
    let (mut db, before) = objects(&src);
    assert!(before.is_empty(), "{:?}", codes_of(&before));
    let mut file = read(&src, Limits::default()).0.unwrap();
    // VIEW と VPORT の高さを変え（VPORT は比を保つ）、VIEW のねじれと、UCS の原点と、VPORT のアイコンを変える
    set_value(&mut db, 0x10, 40, Data::Real(5.0));
    set_value(&mut db, 0x10, 50, Data::Real(90.0));
    set_value(&mut db, 0x11, 10, Data::Real(-2.0));
    set_value(&mut db, 0x12, 40, Data::Real(5.0));
    set_value(&mut db, 0x12, 74, Data::Int(1));
    let (out, diag) = written_back(&mut file, &db);
    let out = out.unwrap();
    assert!(diag.is_empty(), "{:?}", codes_of(&diag));
    let (after, again) = objects(&out);
    assert_eq!(codes_of(&again), codes_of(&before));
    for handle in [0x10, 0x11, 0x12] {
        let (got, want) = (after.get(handle).unwrap(), db.get(handle).unwrap());
        assert_eq!(got.values.len(), want.values.len());
        for (a, b) in got.values.iter().zip(&want.values) {
            assert_eq!(a.code, b.code);
            match (&a.data, &b.data) {
                (Data::Real(x), Data::Real(y)) => {
                    assert!(close(Some(Some(*x)), *y), "{handle:#x} {}", a.code)
                }
                (x, y) => assert_eq!(x, y, "{handle:#x} {}", a.code),
            }
        }
    }
    let file = read(&out, Limits::default()).0.unwrap();
    for (handle, expected) in views_with(5.0, std::f64::consts::FRAC_PI_2, -2.0, 1) {
        let index = db
            .objects()
            .iter()
            .position(|o| o.handle() == Some(handle))
            .unwrap();
        let range = file.record(index).unwrap().bytes.unwrap();
        let got = &out[range.start as usize..range.end as usize];
        // 埋めのビットを写すので、データが 1 バイト延びることがある（決定 0035）
        let data = &expected[1..expected.len() - 2];
        assert_eq!(&got[1..=data.len()], data, "{handle:#x}");
        assert!(got[data.len() + 1..got.len() - 2].iter().all(|b| *b == 0));
        assert!(got.len() <= expected.len() + 1);
    }
    // UCS を伴う印・紙空間でない 70 のビット・4 からのアイコン・知らない見え方の印・UCS の 70 は受け付けない
    for (handle, code, data) in [
        (0x10, 72, Data::Int(0)),
        (0x10, 70, Data::Int(2)),
        (0x12, 74, Data::Int(4)),
        (0x12, 71, Data::Int(32)),
        (0x11, 70, Data::Int(1)),
    ] {
        let mut changed = objects(&src).0;
        set_value(&mut changed, handle, code, data);
        let mut file = read(&src, Limits::default()).0.unwrap();
        let (out, diag) = written_back(&mut file, &changed);
        assert_eq!(out, Err(Error::Rejected), "{handle:#x} {code}");
        assert_eq!(codes_of(&diag), [codes::DWG_CHANGE_UNSUPPORTED]);
    }
}

/// 寸法の後ろに付く文字（3）・尺度（40）・文字を内側で水平に書く印（73）・文字の縦の位置（77）・寸法線の色
/// （176）のほかは、既定の値の DIMSTYLE
#[cfg(test)]
fn dimstyles_with(
    post: &'static [u8],
    scale: f64,
    inside: bool,
    tad: u16,
    color: u16,
) -> Vec<(u64, Vec<u8>)> {
    let dimstyle = object(
        0x45,
        0x10,
        move |w| {
            w.bl_zero();
            w.bs(9);
            w.bytes(b"STANDARD\0");
            w.bit(false);
            w.bs(0);
            w.bit(false);
            for text in [post, b""] {
                w.bs(text.len() as u16 + 1);
                w.bytes(text);
                w.bytes(&[0]);
            }
            for v in [scale, 0.18, 0.0625, 0.38, 0.18, 0.0, 0.0, 0.0, 0.0] {
                w.bd(v);
            }
            for bit in [false, false, inside, true, false, false] {
                w.bit(bit);
            }
            for v in [tad, 0, 0] {
                w.bs(v);
            }
            for v in [0.18, 0.09, 0.0, 25.4, 1.0, 0.0, 1.0, 0.09, 0.0] {
                w.bd(v);
            }
            w.bit(false);
            w.bs(2);
            for _ in 0..4 {
                w.bit(false);
            }
            for v in [color, 0, 0, 0, 4, 4, 2, 2, 0, 0, 2, 46, 0, 0] {
                w.bs(v);
            }
            w.bit(false);
            w.bit(false);
            for v in [1, 0, 0, 0] {
                w.bs(v);
            }
            w.bit(false);
            for v in [3, 0xFFFE, 0xFFFE] {
                w.bs(v);
            }
        },
        |w| {
            w.handle(4, 2);
            w.handle(3, 0);
            for _ in 0..6 {
                w.handle(5, 0);
            }
        },
    );
    vec![(0x10, dimstyle)]
}

#[test]
fn changed_dimstyles_are_written_back() {
    let src = dwg(&dimstyles_with(b"", 1.0, true, 0, 0));
    let (mut db, before) = objects(&src);
    assert!(before.is_empty(), "{:?}", codes_of(&before));
    let mut file = read(&src, Limits::default()).0.unwrap();
    // 既定の値で表になかった欄を足す
    set_value(&mut db, 0x10, 3, Data::Text(b"<> mm".to_vec()));
    set_value(&mut db, 0x10, 40, Data::Real(2.0));
    set_value(&mut db, 0x10, 73, Data::Int(0));
    set_value(&mut db, 0x10, 77, Data::Int(1));
    set_value(&mut db, 0x10, 176, Data::Int(3));
    let (out, diag) = written_back(&mut file, &db);
    let out = out.unwrap();
    assert!(diag.is_empty(), "{:?}", codes_of(&diag));
    let (after, again) = objects(&out);
    assert_eq!(codes_of(&again), codes_of(&before));
    // 足した欄は表の終わりに置いたので、番号の順に並べて比べる
    let sorted = |db: &AcadDb| {
        let mut values = db.get(0x10).unwrap().values.clone();
        values.sort_by_key(|v| v.code);
        values
    };
    assert_eq!(sorted(&after), sorted(&db));
    let file = read(&out, Limits::default()).0.unwrap();
    let expected = &dimstyles_with(b"<> mm", 2.0, false, 1, 3)[0].1;
    let index = db
        .objects()
        .iter()
        .position(|o| o.handle() == Some(0x10))
        .unwrap();
    let range = file.record(index).unwrap().bytes.unwrap();
    let got = &out[range.start as usize..range.end as usize];
    // 埋めのビットを写すので、データが 1 バイト延びることがある（決定 0035）
    let data = &expected[1..expected.len() - 2];
    assert_eq!(&got[1..=data.len()], data);
    assert!(got[data.len() + 1..got.len() - 2].iter().all(|b| *b == 0));
    assert!(got.len() <= expected.len() + 1);
    // 欄を除けば既定の値に戻る
    let mut back = after.clone();
    let index = back
        .objects()
        .iter()
        .position(|o| o.handle() == Some(0x10))
        .unwrap();
    back.object_mut(index)
        .unwrap()
        .values
        .retain(|v| !matches!(v.code, 3 | 40 | 73 | 77 | 176));
    let mut file = read(&out, Limits::default()).0.unwrap();
    let (again, diag) = written_back(&mut file, &back);
    assert!(diag.is_empty(), "{:?}", codes_of(&diag));
    assert_eq!(
        objects(&again.unwrap()).0.get(0x10).unwrap().values,
        objects(&src).0.get(0x10).unwrap().values
    );
    // 0 でも 1 でもない印・2 つの同じ欄・外部参照に依る印は受け付けない
    for (code, data, twice) in [
        (73, Data::Int(2), false),
        (40, Data::Real(3.0), true),
        (70, Data::Int(16), false),
    ] {
        let mut changed = objects(&src).0;
        if twice {
            set_value(&mut changed, 0x10, code, data.clone());
            let index = changed
                .objects()
                .iter()
                .position(|o| o.handle() == Some(0x10))
                .unwrap();
            changed
                .object_mut(index)
                .unwrap()
                .values
                .push(Value { code, data });
        } else {
            set_value(&mut changed, 0x10, code, data);
        }
        let mut file = read(&src, Limits::default()).0.unwrap();
        let (out, diag) = written_back(&mut file, &changed);
        assert_eq!(out, Err(Error::Rejected), "{code}");
        assert_eq!(codes_of(&diag), [codes::DWG_CHANGE_UNSUPPORTED]);
    }
}

/// 説明・名前のない印・選べる印を持つ GROUP（0x20）と、型と値を持つ DICTIONARYVAR（0x21）
#[cfg(test)]
fn groups_and_vars_with(
    about: &'static [u8],
    unnamed: u16,
    selectable: u16,
    kind: u8,
    var: &'static [u8],
) -> Vec<u8> {
    let text = |w: &mut BitWriter, text: &[u8]| {
        w.bs(text.len() as u16 + 1);
        w.bytes(text);
        w.bytes(&[0]);
    };
    let group = object(
        0x48,
        0x20,
        move |w| {
            w.bl_zero();
            text(w, about);
            w.bs(unnamed);
            w.bs(selectable);
            w.bits(1, 2); // 図形の数（BL の RC）
            w.bytes(&[1]);
        },
        |w| {
            w.handle(4, 0xC);
            w.handle(3, 0);
            w.handle(5, 0x30);
        },
    );
    let var = object(
        500,
        0x21,
        move |w| {
            w.bl_zero();
            w.bytes(&[kind]);
            text(w, var);
        },
        |w| {
            w.handle(4, 0xC);
            w.handle(3, 0);
        },
    );
    with_classes(
        dwg(&[(0x20, group), (0x21, var)]),
        &[(500, b"DICTIONARYVAR", false)],
    )
}

#[test]
fn changed_groups_and_vars_are_written_back() {
    let src = groups_and_vars_with(b"two lines", 0, 1, 0, b"1");
    let (mut db, before) = objects(&src);
    assert!(before.is_empty(), "{:?}", codes_of(&before));
    let mut file = read(&src, Limits::default()).0.unwrap();
    set_value(
        &mut db,
        0x20,
        300,
        Data::Text(b"a longer description".to_vec()),
    );
    set_value(&mut db, 0x20, 70, Data::Int(1));
    set_value(&mut db, 0x20, 71, Data::Int(0));
    set_value(&mut db, 0x21, 280, Data::Int(1));
    set_value(&mut db, 0x21, 1, Data::Text(b"2.5".to_vec()));
    let (out, diag) = written_back(&mut file, &db);
    let out = out.unwrap();
    assert!(diag.is_empty(), "{:?}", codes_of(&diag));
    let (after, again) = objects(&out);
    assert_eq!(codes_of(&again), codes_of(&before));
    for handle in [0x20, 0x21] {
        assert_eq!(
            after.get(handle).unwrap().values,
            db.get(handle).unwrap().values
        );
    }
    // 変えた欄のほかは、データが組んだものと同じになる
    let expected = objects(&groups_and_vars_with(
        b"a longer description",
        1,
        0,
        1,
        b"2.5",
    ))
    .0;
    for handle in [0x20, 0x21] {
        assert_eq!(
            after.get(handle).unwrap().values,
            expected.get(handle).unwrap().values
        );
    }
    // 0 でも 1 でもない印・ハンドルの並びの図形の変更・RC に収まらない型は受け付けない
    for (handle, code, data) in [
        (0x20, 70, Data::Int(2)),
        (0x20, 340, Data::Handle(0x31)),
        (0x21, 280, Data::Int(256)),
    ] {
        let mut changed = objects(&src).0;
        set_value(&mut changed, handle, code, data);
        let mut file = read(&src, Limits::default()).0.unwrap();
        let (out, diag) = written_back(&mut file, &changed);
        assert_eq!(out, Err(Error::Rejected), "{code}");
        assert_eq!(codes_of(&diag), [codes::DWG_CHANGE_UNSUPPORTED]);
    }
}

/// 2 つの項目を持つ辞書（0x20）と、2 本の線を持つ多重線の体裁（0x21）。辞書の所有する印は 1、多重線の
/// 体裁の DWG の印は、DXF にない 4 を持つ
#[cfg(test)]
fn dictionaries_and_mlinestyles_with(
    cloning: u16,
    names: [&'static [u8]; 2],
    about: &'static [u8],
    flags: u16,
    angle: f64,
    offsets: [f64; 2],
) -> Vec<u8> {
    let text = |w: &mut BitWriter, text: &[u8]| {
        w.bs(text.len() as u16 + 1);
        w.bytes(text);
        w.bytes(&[0]);
    };
    let dictionary = object(
        0x2A,
        0x20,
        move |w| {
            w.bl_zero();
            w.bits(1, 2); // 項目の数（BL の RC）
            w.bytes(&[2]);
            w.bs(cloning);
            w.bytes(&[1]); // 所有する
            for name in names {
                text(w, name);
            }
        },
        |w| {
            w.handle(4, 0xC);
            w.handle(3, 0);
            w.handle(2, 0x30);
            w.handle(2, 0x31);
        },
    );
    let style = object(
        0x49,
        0x21,
        move |w| {
            w.bl_zero();
            text(w, b"STANDARD");
            text(w, about);
            w.bs(flags | 4);
            w.bs(256);
            w.bd(angle);
            w.bd(std::f64::consts::FRAC_PI_2);
            w.bytes(&[2]);
            for (offset, ltype) in offsets.into_iter().zip([3, 5]) {
                w.bd(offset);
                w.bs(1);
                w.bs(ltype);
            }
        },
        |w| w.handle(4, 0xC),
    );
    dwg(&[(0x20, dictionary), (0x21, style)])
}

#[test]
fn changed_dictionaries_and_mlinestyles_are_written_back() {
    let right = std::f64::consts::FRAC_PI_2;
    let src = dictionaries_and_mlinestyles_with(1, [b"A", b"B"], b"", 1, right, [0.5, -0.5]);
    let (mut db, before) = objects(&src);
    assert!(before.is_empty(), "{:?}", codes_of(&before));
    let mut file = read(&src, Limits::default()).0.unwrap();
    let index = |db: &AcadDb, handle| {
        db.objects()
            .iter()
            .position(|o| o.handle() == Some(handle))
            .unwrap()
    };
    set_value(&mut db, 0x20, 281, Data::Int(3));
    // 2 つめの項目の名前を変える
    let at = index(&db, 0x20);
    let values = &mut db.object_mut(at).unwrap().values;
    let second = values.iter().rposition(|v| v.code == 3).unwrap();
    values[second].data = Data::Text(b"LONGER_NAME".to_vec());
    set_value(&mut db, 0x21, 3, Data::Text(b"two lines".to_vec()));
    // DWG の 1 と 32 にあたる、DXF の 2 と 64
    set_value(&mut db, 0x21, 70, Data::Int(66));
    set_value(&mut db, 0x21, 51, Data::Real(45.0));
    let at = index(&db, 0x21);
    let values = &mut db.object_mut(at).unwrap().values;
    let first = values.iter().position(|v| v.code == 49).unwrap();
    values[first].data = Data::Real(1.25);
    let (out, diag) = written_back(&mut file, &db);
    let out = out.unwrap();
    assert!(diag.is_empty(), "{:?}", codes_of(&diag));
    let (after, again) = objects(&out);
    assert_eq!(codes_of(&again), codes_of(&before));
    let built = dictionaries_and_mlinestyles_with(
        3,
        [b"A", b"LONGER_NAME"],
        b"two lines",
        33,
        right / 2.0,
        [1.25, -0.5],
    );
    let expected = objects(&built).0;
    for handle in [0x20, 0x21] {
        assert_eq!(
            after.get(handle).unwrap().values,
            db.get(handle).unwrap().values
        );
        assert_eq!(
            after.get(handle).unwrap().values,
            expected.get(handle).unwrap().values
        );
    }
    // DXF にない印のビットと、線の線種の番号は元のまま写すので、データも組んだものと同じになる
    let file = read(&out, Limits::default()).0.unwrap();
    let range = file.record(index(&db, 0x21)).unwrap().bytes.unwrap();
    let got = &out[range.start as usize..range.end as usize];
    let range = read(&built, Limits::default())
        .0
        .unwrap()
        .record(index(&db, 0x21))
        .unwrap()
        .bytes
        .unwrap();
    assert_eq!(got, &built[range.start as usize..range.end as usize]);
    // 所有しない印・項目の追加・多重線の体裁の名前・DXF にない印は受け付けない
    for (handle, code, data) in [
        (0x20, 280, Data::Int(0)),
        (0x21, 2, Data::Text(b"OTHER".to_vec())),
        (0x21, 70, Data::Int(4)),
    ] {
        let mut changed = objects(&src).0;
        set_value(&mut changed, handle, code, data);
        let mut file = read(&src, Limits::default()).0.unwrap();
        let (out, diag) = written_back(&mut file, &changed);
        assert_eq!(out, Err(Error::Rejected), "{code}");
        assert_eq!(codes_of(&diag), [codes::DWG_CHANGE_UNSUPPORTED]);
    }
    let mut changed = objects(&src).0;
    let at = index(&changed, 0x20);
    let values = &mut changed.object_mut(at).unwrap().values;
    values.push(Value {
        code: 3,
        data: Data::Text(b"C".to_vec()),
    });
    values.push(Value {
        code: 360,
        data: Data::Handle(0x32),
    });
    let mut file = read(&src, Limits::default()).0.unwrap();
    let (out, _) = written_back(&mut file, &changed);
    assert_eq!(out, Err(Error::Rejected));
}

/// 印刷の機器（2）・紙の幅（44）・印刷の単位（72）・範囲の左下の X（10）・高さ（146）のほかは決まった値の
/// LAYOUT（0x23）
#[cfg(test)]
fn layouts_with(
    device: &'static [u8],
    width: f64,
    units: u16,
    left: f64,
    elevation: f64,
) -> Vec<u8> {
    let text = |w: &mut BitWriter, text: &[u8]| {
        w.bs(text.len() as u16 + 1);
        w.bytes(text);
        w.bytes(&[0]);
    };
    let layout = object(
        500,
        0x23,
        move |w| {
            w.bl_zero();
            text(w, b"");
            text(w, device);
            w.bs(688);
            for v in [6.35, 6.35, 6.35, 6.35, width, 279.4] {
                w.bd(v);
            }
            text(w, b"Letter");
            w.bd(1.0);
            w.bd(2.0);
            for v in [units, 1, 5] {
                w.bs(v);
            }
            for v in [3.0, 4.0, 5.0, 6.0] {
                w.bd(v);
            }
            text(w, b"");
            w.bd(1.0);
            w.bd(1.0);
            text(w, b"mono.ctb");
            w.bs(16);
            w.bd(1.0);
            w.bd(0.0);
            w.bd(0.0);
            text(w, b"Layout1");
            w.bs(1);
            w.bs(1);
            for v in [0.0, 0.0, 0.0] {
                w.bd(v);
            }
            for v in [left, -0.25, 10.75, 8.25] {
                w.rd(v);
            }
            for v in [0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0] {
                w.bd(v);
            }
            w.bd(elevation);
            w.bs(0);
            for v in [1.0, 0.5, 0.0, 9.5, 7.0, 0.0] {
                w.bd(v);
            }
        },
        |w| {
            w.handle(4, 0xC);
            w.handle(3, 0);
            w.handle(4, 0x40);
            w.handle(4, 0x41);
            w.handle(5, 0);
            w.handle(5, 0);
        },
    );
    with_classes(dwg(&[(0x23, layout)]), &[(500, b"LAYOUT", false)])
}

#[test]
fn changed_layouts_are_written_back() {
    let src = layouts_with(b"none_device", 215.9, 0, -0.25, 0.0);
    let (mut db, before) = objects(&src);
    assert!(before.is_empty(), "{:?}", codes_of(&before));
    let mut file = read(&src, Limits::default()).0.unwrap();
    set_value(&mut db, 0x23, 2, Data::Text(b"DWG To PDF.pc3".to_vec()));
    set_value(&mut db, 0x23, 44, Data::Real(420.0));
    set_value(&mut db, 0x23, 72, Data::Int(1));
    set_value(&mut db, 0x23, 10, Data::Real(-1.5));
    set_value(&mut db, 0x23, 146, Data::Real(2.5));
    let (out, diag) = written_back(&mut file, &db);
    let out = out.unwrap();
    assert!(diag.is_empty(), "{:?}", codes_of(&diag));
    let (after, again) = objects(&out);
    assert_eq!(codes_of(&again), codes_of(&before));
    assert_eq!(
        after.get(0x23).unwrap().values,
        db.get(0x23).unwrap().values
    );
    let built = layouts_with(b"DWG To PDF.pc3", 420.0, 1, -1.5, 2.5);
    let index = db
        .objects()
        .iter()
        .position(|o| o.handle() == Some(0x23))
        .unwrap();
    let got = read(&out, Limits::default())
        .0
        .unwrap()
        .record(index)
        .unwrap()
        .bytes
        .unwrap();
    let range = read(&built, Limits::default())
        .0
        .unwrap()
        .record(index)
        .unwrap()
        .bytes
        .unwrap();
    assert_eq!(
        &out[got.start as usize..got.end as usize],
        &built[range.start as usize..range.end as usize]
    );
    // レイアウトの名前・紙空間のブロックのレコード・BS に収まらない値・欄の削除は受け付けない
    let layout_name = |db: &mut AcadDb| {
        let index = db
            .objects()
            .iter()
            .position(|o| o.handle() == Some(0x23))
            .unwrap();
        let values = &mut db.object_mut(index).unwrap().values;
        let at = values
            .iter()
            .position(|v| v.data == Data::Text(b"Layout1".to_vec()))
            .unwrap();
        values[at].data = Data::Text(b"Sheet".to_vec());
    };
    let mut changed = objects(&src).0;
    layout_name(&mut changed);
    let mut file = read(&src, Limits::default()).0.unwrap();
    assert_eq!(written_back(&mut file, &changed).0, Err(Error::Rejected));
    for (code, data) in [(331, Data::Handle(0x42)), (72, Data::Int(40000))] {
        let mut changed = objects(&src).0;
        set_value(&mut changed, 0x23, code, data);
        let mut file = read(&src, Limits::default()).0.unwrap();
        let (out, diag) = written_back(&mut file, &changed);
        assert_eq!(out, Err(Error::Rejected), "{code}");
        assert_eq!(codes_of(&diag), [codes::DWG_CHANGE_UNSUPPORTED]);
    }
    let mut changed = objects(&src).0;
    let index = changed
        .objects()
        .iter()
        .position(|o| o.handle() == Some(0x23))
        .unwrap();
    changed
        .object_mut(index)
        .unwrap()
        .values
        .retain(|v| v.code != 146);
    let mut file = read(&src, Limits::default()).0.unwrap();
    assert_eq!(written_back(&mut file, &changed).0, Err(Error::Rejected));
}

/// 文字列（1）・点の X（10）・実数（40）・整数（70）・真偽（290）・バイナリ（310）のほかは決まった値の
/// XRECORD（0x21）。文字列のコード・ページは 30
#[cfg(test)]
fn xrecords_with(
    text: &'static [u8],
    x: f64,
    real: f64,
    int: i16,
    flag: u8,
    binary: &'static [u8],
) -> Vec<u8> {
    let mut data = BitWriter::default();
    let rs = |w: &mut BitWriter, v: i16| w.bytes(&v.to_le_bytes());
    rs(&mut data, 1);
    rs(&mut data, text.len() as i16);
    data.bytes(&[30]);
    data.bytes(text);
    rs(&mut data, 10);
    for v in [x, 2.0, 3.0] {
        data.rd(v);
    }
    rs(&mut data, 40);
    data.rd(real);
    rs(&mut data, 70);
    rs(&mut data, int);
    rs(&mut data, 90);
    data.rl(70000);
    rs(&mut data, 160);
    data.bytes(&(1i64 << 40).to_le_bytes());
    rs(&mut data, 280);
    data.bytes(&[7]);
    rs(&mut data, 290);
    data.bytes(&[flag]);
    rs(&mut data, 310);
    data.bytes(&[binary.len() as u8]);
    data.bytes(binary);
    rs(&mut data, 340);
    data.bytes(&0x20u64.to_le_bytes());
    let xrecord = object(
        500,
        0x21,
        move |w| {
            w.bl_zero();
            w.bits(1, 2); // データのバイト数（BL の RC）
            w.bytes(&[(data.bits.len() / 8) as u8]);
            w.bits.extend(&data.bits);
            w.bs(1); // 複製の扱い
        },
        |w| {
            w.handle(4, 0xC);
            w.handle(3, 0);
        },
    );
    with_classes(dwg(&[(0x21, xrecord)]), &[(500, b"XRECORD", false)])
}

#[test]
fn changed_xrecords_are_written_back() {
    let src = xrecords_with(b"a\nb", 1.0, 0.5, -2, 1, &[0xAB, 0xCD]);
    let (mut db, before) = objects(&src);
    assert!(before.is_empty(), "{:?}", codes_of(&before));
    let mut file = read(&src, Limits::default()).0.unwrap();
    set_value(&mut db, 0x21, 1, Data::Text(b"x^Jyz".to_vec()));
    set_value(&mut db, 0x21, 10, Data::Real(-4.0));
    set_value(&mut db, 0x21, 40, Data::Real(2.5));
    set_value(&mut db, 0x21, 70, Data::Int(300));
    set_value(&mut db, 0x21, 290, Data::Bool(false));
    set_value(&mut db, 0x21, 310, Data::Binary(vec![1, 2, 3]));
    let (out, diag) = written_back(&mut file, &db);
    let out = out.unwrap();
    assert!(diag.is_empty(), "{:?}", codes_of(&diag));
    let (after, again) = objects(&out);
    assert_eq!(codes_of(&again), codes_of(&before));
    assert_eq!(
        after.get(0x21).unwrap().values,
        db.get(0x21).unwrap().values
    );
    let built = xrecords_with(b"x\nyz", -4.0, 2.5, 300, 0, &[1, 2, 3]);
    let index = db
        .objects()
        .iter()
        .position(|o| o.handle() == Some(0x21))
        .unwrap();
    let got = read(&out, Limits::default())
        .0
        .unwrap()
        .record(index)
        .unwrap()
        .bytes
        .unwrap();
    let range = read(&built, Limits::default())
        .0
        .unwrap()
        .record(index)
        .unwrap()
        .bytes
        .unwrap();
    assert_eq!(
        &out[got.start as usize..got.end as usize],
        &built[range.start as usize..range.end as usize]
    );
    // 所有者・型に収まらない値・値の追加は受け付けない
    for (code, data) in [
        (330, Data::Handle(0x42)),
        (70, Data::Int(40000)),
        (280, Data::Int(40000)),
        (300, Data::Text(b"new".to_vec())),
    ] {
        let mut changed = objects(&src).0;
        set_value(&mut changed, 0x21, code, data);
        let mut file = read(&src, Limits::default()).0.unwrap();
        let (out, diag) = written_back(&mut file, &changed);
        assert_eq!(out, Err(Error::Rejected), "{code}");
        assert_eq!(codes_of(&diag), [codes::DWG_CHANGE_UNSUPPORTED]);
    }
}

/// ブロックのレコード 0x60 と、その図形の LINE 0x70〜0x72。`links` なら、LINE は前後の図形のハンドルを持つ
#[cfg(test)]
fn objects_with_linked_lines(links: bool) -> Vec<(u64, Vec<u8>)> {
    let control = object(0x32, 2, |w| w.bl_zero(), |w| w.handle(4, 0));
    let layer = object(0x33, 0x10, record_named(b"0"), record_handles(2));
    let block = object(0x31, 0x60, record_named(b"B"), |w| {
        w.handle(4, 0x5F); // 所有者
        w.handle(3, 0); // 拡張辞書
        w.handle(5, 0); // 外部参照のブロック
        w.handle(3, 0); // BLOCK
        w.handle(4, 0x70); // 最初の図形
        w.handle(4, 0x72); // 最後の図形
        w.handle(3, 0); // ENDBLK
        w.handle(5, 0); // 配置
    });
    let mut objects = vec![(2, control), (0x10, layer), (0x60, block)];
    for handle in 0x70..=0x72u64 {
        let line = object(
            0x13,
            handle,
            move |w| {
                w.bit(false);
                w.bits(0, 2); // 所有者のハンドルを持つ
                w.bl_zero();
                w.bit(!links);
                w.bs(256);
                w.bd(1.0);
                w.bits(0, 2);
                w.bits(0, 2);
                w.bs(0);
                w.bytes(&[0x1D]);
                line_data(w, [0.0; 3], [handle as f64, 1.0, 0.0], 0.0);
            },
            move |w| {
                w.handle(4, 0x60);
                w.handle(3, 0);
                if links {
                    w.handle(4, if handle == 0x70 { 0 } else { handle - 1 });
                    w.handle(4, if handle == 0x72 { 0 } else { handle + 1 });
                }
                w.handle(5, 0x10);
            },
        );
        objects.push((handle, line));
    }
    objects
}

#[test]
fn removed_entities_are_written_back() {
    let summary = |db: &AcadDb| -> Vec<(Vec<u8>, Vec<Value>)> {
        db.objects()
            .iter()
            .map(|o| (o.kind.clone(), o.values.clone()))
            .collect()
    };
    for links in [true, false] {
        let mut src = dwg(&objects_with_linked_lines(links));
        // 真ん中・最初・最後の順に除く。並びのハンドルが崩れれば、次の削除を書き戻せない
        for handle in [0x71, 0x70, 0x72] {
            let (mut db, _) = objects(&src);
            let index = db
                .objects()
                .iter()
                .position(|o| o.handle() == Some(handle))
                .unwrap();
            db.remove(index).unwrap();
            let mut file = read(&src, Limits::default()).0.unwrap();
            let (out, diag) = written_back(&mut file, &db);
            let out = out.unwrap();
            assert!(codes_of(&diag).is_empty(), "{handle:X}");
            let (again, diag) = objects(&out);
            assert!(codes_of(&diag).is_empty(), "{handle:X}");
            assert_eq!(summary(&again), summary(&db), "{handle:X}");
            src = out;
        }
    }
    // 表のレコードと、所有者がブロックのレコードでない図形は除けない
    let cases = [
        ("LAYER", dwg(&objects_with_linked_lines(true)), 0x10),
        ("BLOCK_RECORD", dwg(&objects_with_linked_lines(true)), 0x60),
        ("LINE", dwg(&objects_in_sample()), 0x2A),
    ];
    for (kind, src, handle) in cases {
        let (mut db, _) = objects(&src);
        let index = db
            .objects()
            .iter()
            .position(|o| o.handle() == Some(handle))
            .unwrap();
        db.remove(index).unwrap();
        let mut file = read(&src, Limits::default()).0.unwrap();
        let (out, diag) = written_back(&mut file, &db);
        assert_eq!(out, Err(Error::Rejected), "{kind}");
        assert_eq!(codes_of(&diag), [codes::DWG_CHANGE_UNSUPPORTED], "{kind}");
        let found = diag.iter().next().and_then(|d| d.param("kind"));
        assert_eq!(found, Some(&kind.to_string().into()));
    }
}

/// [`objects_with_linked_lines`] に、HANDSEED が 0x80 の見出しの変数を付けたもの
#[cfg(test)]
fn linked_lines_with_seed(links: bool) -> Vec<u8> {
    with_header(
        dwg(&objects_with_linked_lines(links)),
        0,
        0,
        0,
        [0.0; 4],
        0x80,
    )
}

/// 表の終わりに、ハンドル `handle` の図形を足した表。図形は、LINE 0x72 を写して `kind` と値を変えたもの
#[cfg(test)]
fn with_added(db: &AcadDb, handle: u64, kind: &[u8], values: &[(i16, f64)]) -> AcadDb {
    let values = values
        .iter()
        .map(|&(code, value)| Value {
            code,
            data: Data::Real(value),
        })
        .collect();
    with_added_values(db, handle, kind, values)
}

/// 表の終わりに、ハンドル `handle` の図形を足した表。図形は、LINE 0x72 を写して `kind` と値を変えたもの
#[cfg(test)]
fn with_added_values(db: &AcadDb, handle: u64, kind: &[u8], values: Vec<Value>) -> AcadDb {
    let mut db = db.clone();
    let mut object = db.get(0x72).unwrap().clone();
    object.kind = kind.to_vec();
    object.origin = Origin::New;
    object.values.retain(|v| matches!(v.code, 330 | 8));
    object.values.insert(
        0,
        Value {
            code: 5,
            data: Data::Handle(handle),
        },
    );
    object.values.extend(values);
    db.push(object);
    db
}

#[test]
fn added_texts_mtexts_and_inserts_with_attribs_are_written_back() {
    let summary = |db: &AcadDb| -> Vec<(Vec<u8>, Vec<Value>)> {
        db.objects()
            .iter()
            .map(|o| (o.kind.clone(), o.values.clone()))
            .collect()
    };
    let real = |code, value| Value {
        code,
        data: Data::Real(value),
    };
    let text = |code, value: &[u8]| Value {
        code,
        data: Data::Text(value.to_vec()),
    };
    let int = |code, value| Value {
        code,
        data: Data::Int(value),
    };
    // 文字の体裁の Standard と Mono と、図形のないブロック C を足したファイル
    let mut parts = objects_with_linked_lines(true);
    parts.push((
        0x11,
        object(0x35, 0x11, record_named(b"Standard"), record_handles(3)),
    ));
    parts.push((
        0x12,
        object(0x35, 0x12, record_named(b"Mono"), record_handles(3)),
    ));
    let block = object(0x31, 0x61, record_named(b"C"), |w| {
        w.handle(4, 0x5F);
        w.handle(3, 0);
        w.handle(5, 0);
        w.handle(3, 0);
        w.handle(4, 0);
        w.handle(4, 0);
        w.handle(3, 0);
        w.handle(5, 0);
    });
    parts.push((0x61, block));
    // 地図は、ハンドルの順に並べる
    parts.sort_by_key(|(handle, _)| *handle);
    let src = with_header(dwg(&parts), 0, 0, 0, [0.0; 4], 0x80);
    let (base, diag) = objects(&src);
    assert!(codes_of(&diag).is_empty(), "{:?}", codes_of(&diag));
    let place = [real(10, 1.0), real(20, 2.0), real(30, 0.0), real(40, 2.5)];
    let db = with_added_values(
        &base,
        0x80,
        b"TEXT",
        [&place[..], &[text(1, b"Hi")]].concat(),
    );
    let db = with_added_values(
        &db,
        0x81,
        b"TEXT",
        [&place[..], &[text(1, b"Mono"), text(7, b"Mono")]].concat(),
    );
    let db = with_added_values(
        &db,
        0x82,
        b"ATTDEF",
        [
            &place[..],
            &[text(1, b"A"), text(2, b"TAG"), int(70, 0), text(3, b"Tag?")],
        ]
        .concat(),
    );
    let db = with_added_values(
        &db,
        0x83,
        b"INSERT",
        vec![real(10, 1.0), real(20, 2.0), real(30, 0.0), text(2, b"C")],
    );
    let mtext = |handle, style: &[Value]| {
        let values = [
            real(10, 1.0),
            real(20, 2.0),
            real(30, 0.0),
            real(41, 10.0),
            real(40, 2.5),
            int(71, 1),
            int(72, 1),
            text(1, b"zumen\\Pmtext"),
            int(73, 1),
            real(44, 1.0),
        ];
        let values = [&values[..], style].concat();
        move |db: &AcadDb| with_added_values(db, handle, b"MTEXT", values.clone())
    };
    let db = mtext(0x84, &[])(&db);
    let db = mtext(0x85, &[text(7, b"Mono")])(&db);
    // 属性を持つ INSERT は、ATTRIB を続け、SEQEND で閉じる。ATTRIB と SEQEND は INSERT が持つ
    let attributed = vec![
        real(10, 1.0),
        real(20, 2.0),
        real(30, 0.0),
        int(66, 1),
        text(2, b"C"),
    ];
    let attrib =
        |value: &[u8]| [&place[..], &[text(1, value), text(2, b"TAG"), int(70, 0)]].concat();
    let owned = |db: &AcadDb, handle, kind: &[u8], owner, values| {
        let mut db = with_added_values(db, handle, kind, values);
        set_value(&mut db, handle, 330, Data::Handle(owner));
        db
    };
    let db = with_added_values(&db, 0x86, b"INSERT", attributed.clone());
    let db = owned(&db, 0x87, b"ATTRIB", 0x86, attrib(b"1"));
    let db = owned(&db, 0x88, b"ATTRIB", 0x86, attrib(b"2"));
    let db = owned(&db, 0x89, b"SEQEND", 0x86, Vec::new());
    // 列と行の数と間隔を持つものは MINSERT として書き、属性も同じく続ける
    let grid = [int(70, 2), int(71, 3), real(44, 5.0), real(45, 6.0)];
    let gridded = [&attributed[..4], &grid, &attributed[4..]].concat();
    let db = with_added_values(&db, 0x8A, b"INSERT", gridded);
    let db = owned(&db, 0x8B, b"ATTRIB", 0x8A, attrib(b"3"));
    let db = owned(&db, 0x8C, b"SEQEND", 0x8A, Vec::new());
    let plain = [&attributed[..3], &grid[..2], &attributed[4..]].concat();
    let db = with_added_values(&db, 0x8D, b"INSERT", plain);
    let mut file = read(&src, Limits::default()).0.unwrap();
    let (out, diag) = written_back(&mut file, &db);
    assert!(codes_of(&diag).is_empty(), "{:?}", codes_of(&diag));
    let (again, diag) = objects(&out.unwrap());
    assert!(codes_of(&diag).is_empty(), "{:?}", codes_of(&diag));
    assert_eq!(summary(&again), summary(&db));
    // 表にない文字の体裁（TEXT・MTEXT）とブロック、図形を持つブロックそのもの、SEQEND で閉じない INSERT、
    // ATTRIB のない SEQEND、INSERT の後にない ATTRIB は、書き戻せない
    let insert = |values: Vec<Value>| with_added_values(&base, 0x80, b"INSERT", values);
    let open = insert(attributed.clone());
    let open = owned(&open, 0x81, b"ATTRIB", 0x80, attrib(b"1"));
    let cases = [
        (
            "TEXT",
            with_added_values(
                &base,
                0x80,
                b"TEXT",
                [&place[..], &[text(1, b"Hi"), text(7, b"NONE")]].concat(),
            ),
        ),
        ("MTEXT", mtext(0x80, &[text(7, b"NONE")])(&base)),
        (
            "INSERT",
            insert(vec![
                real(10, 0.0),
                real(20, 0.0),
                real(30, 0.0),
                text(2, b"X"),
            ]),
        ),
        (
            "INSERT",
            insert(vec![
                real(10, 0.0),
                real(20, 0.0),
                real(30, 0.0),
                text(2, b"B"),
            ]),
        ),
        (
            "INSERT",
            insert(vec![
                real(10, 0.0),
                real(20, 0.0),
                real(30, 0.0),
                int(66, 1),
                text(2, b"C"),
            ]),
        ),
        ("INSERT", open.clone()),
        (
            "INSERT",
            with_added_values(&open, 0x82, b"LINE", vec![real(10, 0.0)]),
        ),
        (
            "SEQEND",
            owned(
                &insert(attributed.clone()),
                0x81,
                b"SEQEND",
                0x80,
                Vec::new(),
            ),
        ),
        ("ATTRIB", owned(&base, 0x80, b"ATTRIB", 0x61, attrib(b"1"))),
        (
            "ATTRIB",
            owned(&insert(attributed), 0x81, b"ATTRIB", 0x61, attrib(b"1")),
        ),
    ];
    for (kind, db) in cases {
        let mut file = read(&src, Limits::default()).0.unwrap();
        let (out, diag) = written_back(&mut file, &db);
        assert_eq!(out, Err(Error::Rejected), "{kind}");
        let found = diag.iter().next().and_then(|d| d.param("kind"));
        assert_eq!(found, Some(&kind.to_string().into()), "{kind}");
    }
}

#[test]
fn added_lwpolylines_are_written_back() {
    let real = |code, value| Value {
        code,
        data: Data::Real(value),
    };
    let int = |code, value| Value {
        code,
        data: Data::Int(value),
    };
    let src = with_header(
        dwg(&objects_with_linked_lines(true)),
        0,
        0,
        0,
        [0.0; 4],
        0x80,
    );
    let (base, diag) = objects(&src);
    assert!(codes_of(&diag).is_empty(), "{:?}", codes_of(&diag));
    // 一定の幅のもの、閉じて膨らみと標高と厚みと押し出しの向きを持つもの、頂点ごとの幅を持つもの
    let plain = vec![
        int(90, 2),
        int(70, 0),
        real(43, 0.5),
        real(10, 0.0),
        real(20, 0.0),
        real(10, 3.0),
        real(20, 4.0),
    ];
    let bulged = vec![
        int(90, 3),
        int(70, 1),
        real(43, 0.0),
        real(38, 1.5),
        real(39, 2.0),
        real(10, 0.0),
        real(20, 0.0),
        real(42, 1.0),
        real(10, 5.0),
        real(20, 0.0),
        real(10, 5.0),
        real(20, 5.0),
        real(42, -0.5),
        real(210, 0.0),
        real(220, 0.0),
        real(230, -1.0),
    ];
    let wide = vec![
        int(90, 2),
        int(70, 0),
        real(10, 0.0),
        real(20, 0.0),
        real(40, 0.1),
        real(41, 0.2),
        real(10, 1.0),
        real(20, 1.0),
        real(40, 0.3),
        real(41, 0.4),
    ];
    let db = with_added_values(&base, 0x80, b"LWPOLYLINE", plain.clone());
    let db = with_added_values(&db, 0x81, b"LWPOLYLINE", bulged);
    let db = with_added_values(&db, 0x82, b"LWPOLYLINE", wide);
    let mut file = read(&src, Limits::default()).0.unwrap();
    let (out, diag) = written_back(&mut file, &db);
    assert!(codes_of(&diag).is_empty(), "{:?}", codes_of(&diag));
    let (again, diag) = objects(&out.unwrap());
    assert!(codes_of(&diag).is_empty(), "{:?}", codes_of(&diag));
    let summary = |db: &AcadDb| -> Vec<(Vec<u8>, Vec<Value>)> {
        db.objects()
            .iter()
            .map(|o| (o.kind.clone(), o.values.clone()))
            .collect()
    };
    assert_eq!(summary(&again), summary(&db));
    // 頂点の数（90）が頂点と合わないものは、書き戻せない
    let mut miscounted = plain;
    miscounted[0] = int(90, 3);
    let db = with_added_values(&base, 0x80, b"LWPOLYLINE", miscounted);
    let mut file = read(&src, Limits::default()).0.unwrap();
    let (out, diag) = written_back(&mut file, &db);
    assert_eq!(out, Err(Error::Rejected));
    let found = diag.iter().next().and_then(|d| d.param("kind"));
    assert_eq!(found, Some(&"LWPOLYLINE".to_string().into()));
}

#[test]
fn added_splines_are_written_back() {
    let real = |code, value| Value {
        code,
        data: Data::Real(value),
    };
    let int = |code, value| Value {
        code,
        data: Data::Int(value),
    };
    let src = with_header(
        dwg(&objects_with_linked_lines(true)),
        0,
        0,
        0,
        [0.0; 4],
        0x80,
    );
    let (base, diag) = objects(&src);
    assert!(codes_of(&diag).is_empty(), "{:?}", codes_of(&diag));
    // 3 次で、節点 8 つと制御点 4 つを持つもの
    let head = [
        int(71, 3),
        int(72, 8),
        int(73, 4),
        int(74, 0),
        real(42, 1e-10),
        real(43, 1e-10),
    ];
    let knots = [0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0].map(|k| real(40, k));
    let controls: Vec<Value> = [[0.0, 0.0], [1.0, 2.0], [3.0, 2.0], [4.0, 0.0]]
        .iter()
        .flat_map(|[x, y]| [real(10, *x), real(20, *y), real(30, 0.0)])
        .collect();
    let spline = [&head[..], &knots, &controls].concat();
    let db = with_added_values(&base, 0x80, b"SPLINE", spline.clone());
    let mut file = read(&src, Limits::default()).0.unwrap();
    let (out, diag) = written_back(&mut file, &db);
    assert!(codes_of(&diag).is_empty(), "{:?}", codes_of(&diag));
    let (again, diag) = objects(&out.unwrap());
    assert!(codes_of(&diag).is_empty(), "{:?}", codes_of(&diag));
    let summary = |db: &AcadDb| -> Vec<(Vec<u8>, Vec<Value>)> {
        db.objects()
            .iter()
            .map(|o| (o.kind.clone(), o.values.clone()))
            .collect()
    };
    assert_eq!(summary(&again), summary(&db));
    // 重みを持つもの（有理の印を立てる）と、当てはめる点だけを持つもの（場合が 2）も、書き戻せる
    let weighted: Vec<Value> = spline
        .iter()
        .cloned()
        .flat_map(|v| match v.code {
            30 => vec![v, real(41, 0.5)],
            _ => vec![v],
        })
        .collect();
    let fitted = vec![
        int(71, 3),
        int(72, 0),
        int(73, 0),
        int(74, 2),
        real(44, 1e-10),
        real(12, 1.0),
        real(22, 0.0),
        real(32, 0.0),
        real(11, 0.0),
        real(21, 0.0),
        real(31, 0.0),
        real(11, 1.0),
        real(21, 1.0),
        real(31, 0.0),
    ];
    for values in [weighted.clone(), fitted.clone()] {
        let db = with_added_values(&base, 0x80, b"SPLINE", values);
        let mut file = read(&src, Limits::default()).0.unwrap();
        let (out, diag) = written_back(&mut file, &db);
        assert!(codes_of(&diag).is_empty(), "{:?}", codes_of(&diag));
        let (again, diag) = objects(&out.unwrap());
        assert!(codes_of(&diag).is_empty(), "{:?}", codes_of(&diag));
        assert_eq!(summary(&again), summary(&db));
    }
    // 一部の制御点だけが重みを持つものと、制御点と当てはめる点の両方を持つものは、書き戻せない
    let partly: Vec<Value> = weighted
        .iter()
        .enumerate()
        .filter(|(i, v)| v.code != 41 || *i < weighted.len() - 1)
        .map(|(_, v)| v.clone())
        .collect();
    let both: Vec<Value> = [&spline[..], &fitted[8..]]
        .concat()
        .into_iter()
        .map(|v| match v.code {
            74 => int(74, 2),
            _ => v,
        })
        .collect();
    for values in [partly, both] {
        let db = with_added_values(&base, 0x80, b"SPLINE", values);
        let mut file = read(&src, Limits::default()).0.unwrap();
        let (out, diag) = written_back(&mut file, &db);
        assert_eq!(out, Err(Error::Rejected));
        let found = diag.iter().next().and_then(|d| d.param("kind"));
        assert_eq!(found, Some(&"SPLINE".to_string().into()));
    }
}

#[test]
fn added_entities_are_written_back() {
    let summary = |db: &AcadDb| -> Vec<(Vec<u8>, Section, Vec<Value>)> {
        db.objects()
            .iter()
            .map(|o| (o.kind.clone(), o.section.clone(), o.values.clone()))
            .collect()
    };
    let line = [
        (10, 1.0),
        (20, 2.0),
        (30, 0.0),
        (11, 3.0),
        (21, 4.0),
        (31, 0.0),
    ];
    let circle = [(10, 5.0), (20, 6.0), (30, 0.0), (40, 7.0)];
    for links in [true, false] {
        let src = linked_lines_with_seed(links);
        let (base, _) = objects(&src);
        let db = with_added(&base, 0x80, b"LINE", &line);
        let db = with_added(&db, 0x83, b"CIRCLE", &circle);
        let mut file = read(&src, Limits::default()).0.unwrap();
        let (out, diag) = written_back(&mut file, &db);
        let out = out.unwrap();
        assert!(codes_of(&diag).is_empty(), "{:?}", codes_of(&diag));
        let (again, diag) = objects(&out);
        assert!(codes_of(&diag).is_empty(), "{:?}", codes_of(&diag));
        assert_eq!(summary(&again), summary(&db));
        // HANDSEED は 0x84 に進む。足した図形も、並びをたどって除ける
        let mut file = read(&out, Limits::default()).0.unwrap();
        let (rejected, _) = written_back(&mut file, &with_added(&again, 0x81, b"LINE", &line));
        assert_eq!(rejected, Err(Error::Rejected));
        let mut db = with_added(&again, 0x84, b"LINE", &line);
        let index = db
            .objects()
            .iter()
            .position(|o| o.handle() == Some(0x80))
            .unwrap();
        db.remove(index).unwrap();
        let mut file = read(&out, Limits::default()).0.unwrap();
        let (out, diag) = written_back(&mut file, &db);
        assert!(codes_of(&diag).is_empty(), "{:?}", codes_of(&diag));
        let (again, _) = objects(&out.unwrap());
        assert_eq!(summary(&again), summary(&db));
    }
    // HANDSEED より小さいハンドル・足せない種類（形を示せない SHAPE も）・表にない画層・表の途中に置いた図形は、
    // 書き戻せない
    let src = linked_lines_with_seed(true);
    let (base, _) = objects(&src);
    let mut unknown_layer = with_added(&base, 0x80, b"LINE", &line);
    set_value(&mut unknown_layer, 0x80, 8, Data::Text(b"NONE".to_vec()));
    let mut moved = with_added(&base, 0x80, b"LINE", &line);
    let last = moved.objects().len() - 1;
    let object = moved.remove(last).unwrap();
    moved.insert(0, object);
    let cases = [
        ("LINE", with_added(&base, 0x7F, b"LINE", &line)),
        ("MTEXT", with_added(&base, 0x80, b"MTEXT", &line)),
        (
            "SHAPE",
            with_added(
                &base,
                0x80,
                b"SHAPE",
                &[(10, 1.0), (20, 2.0), (30, 0.0), (40, 1.0)],
            ),
        ),
        ("LINE", unknown_layer),
        ("ENTITIES", moved),
    ];
    for (kind, db) in cases {
        let mut file = read(&src, Limits::default()).0.unwrap();
        let (out, diag) = written_back(&mut file, &db);
        assert_eq!(out, Err(Error::Rejected), "{kind}");
        assert_eq!(codes_of(&diag), [codes::DWG_CHANGE_UNSUPPORTED], "{kind}");
        let found = diag.iter().next().and_then(|d| d.param("kind"));
        assert_eq!(found, Some(&kind.to_string().into()), "{kind}");
    }
}

/// 画層の表（項目は画層 0 の 1 つ）と、画層 0 と、文字の体裁の表と Standard と、線種 Continuous と、[`objects_with_linked_lines`] の
/// ブロックと LINE を持ち、HANDSEED が 0x80 の図面
#[cfg(test)]
fn layer_table_with_seed() -> Vec<u8> {
    let control = object(
        0x32,
        2,
        |w| {
            w.bl_zero();
            // 項目の数 1
            w.bits(1, 2);
            w.bytes(&[1]);
        },
        |w| {
            w.handle(4, 0);
            w.handle(3, 0);
            w.handle(2, 0x10);
        },
    );
    let layer = object(
        0x33,
        0x10,
        |w| {
            w.bl_zero();
            w.bs(2);
            w.bytes(b"0\0");
            w.bit(true); // 参照される印
            w.bs(0); // 外部参照の番号
            w.bit(false); // 外部参照に依る印
            w.bs(0x3F0); // 印刷し、線の太さは既定
            w.bs(7);
        },
        |w| {
            w.handle(4, 2);
            w.handle(3, 0);
            w.handle(5, 0); // 外部参照のブロック
            w.handle(5, 5); // 印刷スタイルの代わりの、表にあるオブジェクト
            w.handle(5, 0x12); // 線種
        },
    );
    let ltype_control = object(
        0x38,
        5,
        |w| {
            w.bl_zero();
            w.bits(1, 2);
            w.bytes(&[1]);
        },
        |w| {
            w.handle(4, 0);
            w.handle(3, 0);
            w.handle(2, 0x12);
        },
    );
    let ltype = object(0x39, 0x12, record_named(b"Continuous"), record_handles(5));
    let style_control = object(
        0x34,
        3,
        |w| {
            w.bl_zero();
            w.bits(1, 2);
            w.bytes(&[1]);
        },
        |w| {
            w.handle(4, 0);
            w.handle(3, 0);
            w.handle(2, 0x11);
        },
    );
    let style = object(0x35, 0x11, record_named(b"Standard"), record_handles(3));
    let mut objects = vec![
        (2, control),
        (3, style_control),
        (5, ltype_control),
        (0x10, layer),
        (0x11, style),
        (0x12, ltype),
    ];
    objects.extend(objects_with_linked_lines(true).into_iter().skip(2));
    with_header(dwg(&objects), 0, 0, 0, [0.0; 4], 0x80)
}

#[test]
fn added_layers_are_written_back() {
    let summary = |db: &AcadDb| -> Vec<(Vec<u8>, Vec<Value>)> {
        db.objects()
            .iter()
            .map(|o| (o.kind.clone(), o.values.clone()))
            .collect()
    };
    let src = layer_table_with_seed();
    let (base, diag) = objects(&src);
    assert!(codes_of(&diag).is_empty(), "{:?}", codes_of(&diag));
    // 画層 0 を写し、ハンドルと名前と印を変えたもの
    let layer = |db: &AcadDb, handle: u64, name: &[u8], change: &dyn Fn(&mut Vec<Value>)| {
        let mut db = db.clone();
        let mut object = db.get(0x10).unwrap().clone();
        object.origin = Origin::New;
        for value in &mut object.values {
            match value.code {
                5 => value.data = Data::Handle(handle),
                2 => value.data = Data::Text(name.to_vec()),
                _ => {}
            }
        }
        change(&mut object.values);
        db.push(object);
        db
    };
    let set = |code: i16, data: Data| {
        move |values: &mut Vec<Value>| {
            for value in values.iter_mut().filter(|v| v.code == code) {
                value.data = data.clone();
            }
        }
    };
    // 凍結して表示せず、印刷しない、線の太さ 0.25 の画層と、そのままの画層と、その画層の LINE
    let db = layer(&base, 0x80, b"Frozen", &|values| {
        set(70, Data::Int(1))(values);
        set(62, Data::Int(-3))(values);
        set(370, Data::Int(25))(values);
        let at = values.iter().position(|v| v.code == 370).unwrap();
        values.insert(
            at,
            Value {
                code: 290,
                data: Data::Bool(false),
            },
        );
    });
    let db = layer(&db, 0x81, b"Plain", &|_| {});
    let mut db = with_added(
        &db,
        0x82,
        b"LINE",
        &[
            (10, 1.0),
            (20, 2.0),
            (30, 0.0),
            (11, 3.0),
            (21, 4.0),
            (31, 0.0),
        ],
    );
    let line = db.objects().len() - 1;
    let mut object = db.objects()[line].clone();
    for value in object.values.iter_mut().filter(|v| v.code == 8) {
        value.data = Data::Text(b"Plain".to_vec());
    }
    db.remove(line).unwrap();
    db.push(object);
    let mut file = read(&src, Limits::default()).0.unwrap();
    let (out, diag) = written_back(&mut file, &db);
    assert!(codes_of(&diag).is_empty(), "{:?}", codes_of(&diag));
    let (again, diag) = objects(&out.unwrap());
    assert!(codes_of(&diag).is_empty(), "{:?}", codes_of(&diag));
    assert_eq!(summary(&again), summary(&db));
    // 表にある名前・画層の表でない所有者・真の色・外部参照に依る印を持つ画層は、書き戻せない
    let cases = [
        layer(&base, 0x80, b"0", &|_| {}),
        layer(&base, 0x80, b"Other", &set(330, Data::Handle(5))),
        layer(&base, 0x80, b"Other", &|values| {
            values.push(Value {
                code: 420,
                data: Data::Int(0x00FF_0000),
            })
        }),
        layer(&base, 0x80, b"Other", &set(70, Data::Int(16))),
    ];
    for db in cases {
        let mut file = read(&src, Limits::default()).0.unwrap();
        let (out, diag) = written_back(&mut file, &db);
        assert_eq!(out, Err(Error::Rejected));
        let found = diag.iter().next().and_then(|d| d.param("kind"));
        assert_eq!(found, Some(&"LAYER".to_string().into()));
    }
}

#[test]
fn added_styles_are_written_back() {
    let summary = |db: &AcadDb| -> Vec<(Vec<u8>, Vec<Value>)> {
        db.objects()
            .iter()
            .map(|o| (o.kind.clone(), o.values.clone()))
            .collect()
    };
    let src = layer_table_with_seed();
    let (base, diag) = objects(&src);
    assert!(codes_of(&diag).is_empty(), "{:?}", codes_of(&diag));
    // Standard を写し、ハンドルと名前と、`values` の値を変えたもの
    let style = |db: &AcadDb, handle: u64, name: &[u8], values: &[(i16, Data)]| {
        let mut db = db.clone();
        let mut object = db.get(0x11).unwrap().clone();
        object.origin = Origin::New;
        for value in &mut object.values {
            match value.code {
                5 => value.data = Data::Handle(handle),
                2 => value.data = Data::Text(name.to_vec()),
                code => {
                    if let Some((_, data)) = values.iter().find(|(c, _)| *c == code) {
                        value.data = data.clone();
                    }
                }
            }
        }
        db.push(object);
        db
    };
    // 縦書きで、高さ・幅の係数・斜めの角度・生成の印・字体の名前を持つ文字の体裁と、その体裁の TEXT
    let db = style(
        &base,
        0x80,
        b"Gothic",
        &[
            (70, Data::Int(4)),
            (40, Data::Real(2.5)),
            (41, Data::Real(0.8)),
            (50, Data::Real(45.0)),
            (71, Data::Int(2)),
            (42, Data::Real(2.5)),
            (3, Data::Text(b"txt.shx".to_vec())),
            (4, Data::Text(b"bigfont.shx".to_vec())),
        ],
    );
    let mut db = with_added(
        &db,
        0x81,
        b"TEXT",
        &[(10, 1.0), (20, 2.0), (30, 0.0), (40, 2.5)],
    );
    let text = db.objects().len() - 1;
    let mut object = db.objects()[text].clone();
    object.values.extend([
        Value {
            code: 1,
            data: Data::Text(b"zumen".to_vec()),
        },
        Value {
            code: 7,
            data: Data::Text(b"Gothic".to_vec()),
        },
    ]);
    db.remove(text).unwrap();
    db.push(object);
    let mut file = read(&src, Limits::default()).0.unwrap();
    let (out, diag) = written_back(&mut file, &db);
    assert!(codes_of(&diag).is_empty(), "{:?}", codes_of(&diag));
    let (again, diag) = objects(&out.unwrap());
    assert!(codes_of(&diag).is_empty(), "{:?}", codes_of(&diag));
    assert_eq!(summary(&again), summary(&db));
    // 表にある名前・文字の体裁の表でない所有者・外部参照に依る印を持つ文字の体裁は、書き戻せない
    let cases = [
        style(&base, 0x80, b"standard", &[]),
        style(&base, 0x80, b"Other", &[(330, Data::Handle(2))]),
        style(&base, 0x80, b"Other", &[(70, Data::Int(16))]),
    ];
    for db in cases {
        let mut file = read(&src, Limits::default()).0.unwrap();
        let (out, diag) = written_back(&mut file, &db);
        assert_eq!(out, Err(Error::Rejected));
        let found = diag.iter().next().and_then(|d| d.param("kind"));
        assert_eq!(found, Some(&"STYLE".to_string().into()));
    }
}

#[test]
fn added_ltypes_are_written_back() {
    let summary = |db: &AcadDb| -> Vec<(Vec<u8>, Vec<Value>)> {
        db.objects()
            .iter()
            .map(|o| (o.kind.clone(), o.values.clone()))
            .collect()
    };
    let src = layer_table_with_seed();
    let (base, diag) = objects(&src);
    assert!(codes_of(&diag).is_empty(), "{:?}", codes_of(&diag));
    // Continuous を写し、名前と印と要素の数と、要素ごとの長さと印 `dashes` を変えたもの
    let ltype = |name: &[u8], flags: i64, count: i64, dashes: &[(f64, i64)]| {
        let mut db = base.clone();
        let mut object = db.get(0x12).unwrap().clone();
        object.origin = Origin::New;
        for value in &mut object.values {
            match value.code {
                5 => value.data = Data::Handle(0x80),
                2 => value.data = Data::Text(name.to_vec()),
                70 => value.data = Data::Int(flags),
                3 => value.data = Data::Text(b"__ __".to_vec()),
                72 => value.data = Data::Int(65),
                73 => value.data = Data::Int(count),
                40 => value.data = Data::Real(0.75),
                _ => {}
            }
        }
        for &(dash, flags) in dashes {
            object.values.extend([
                Value {
                    code: 49,
                    data: Data::Real(dash),
                },
                Value {
                    code: 74,
                    data: Data::Int(flags),
                },
            ]);
        }
        db.push(object);
        db
    };
    // 線と空きの線種と、その線種の LINE
    let dashes = [(0.5, 0), (-0.25, 0)];
    let mut db = with_added(
        &ltype(b"Dashed", 0, 2, &dashes),
        0x81,
        b"LINE",
        &[
            (10, 1.0),
            (20, 2.0),
            (30, 0.0),
            (11, 3.0),
            (21, 4.0),
            (31, 0.0),
        ],
    );
    let line = db.objects().len() - 1;
    let mut object = db.objects()[line].clone();
    object.values.push(Value {
        code: 6,
        data: Data::Text(b"Dashed".to_vec()),
    });
    db.remove(line).unwrap();
    db.push(object);
    let mut file = read(&src, Limits::default()).0.unwrap();
    let (out, diag) = written_back(&mut file, &db);
    assert!(codes_of(&diag).is_empty(), "{:?}", codes_of(&diag));
    let (again, diag) = objects(&out.unwrap());
    assert!(codes_of(&diag).is_empty(), "{:?}", codes_of(&diag));
    assert_eq!(summary(&again), summary(&db));
    // 表にある名前・外部参照に依る印・数の合わない要素・文字の要素を持つ線種は、書き戻せない
    let cases = [
        ltype(b"continuous", 0, 2, &dashes),
        ltype(b"Other", 16, 2, &dashes),
        ltype(b"Other", 0, 3, &dashes),
        ltype(b"Other", 0, 2, &[(0.5, 0), (-0.25, 2)]),
    ];
    for db in cases {
        let mut file = read(&src, Limits::default()).0.unwrap();
        let (out, diag) = written_back(&mut file, &db);
        assert_eq!(out, Err(Error::Rejected));
        let found = diag.iter().next().and_then(|d| d.param("kind"));
        assert_eq!(found, Some(&"LTYPE".to_string().into()));
    }
}
