//! DWG（R2000）の試作を、試験の中で組み立てた DWG で確かめる。受け取った図面は使わない。

use zumen_acad::{AcadDb, Data, Dwg, DwgFile, Dxf, Origin, Section};
use zumen_base::{Budget, CancelToken, Code, Cx, Diagnostics, Kind, Limit, Limits, codes};
use zumen_sdk::{Codec, Error, FileModel};

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

/// 名前を持つ表のレコードの欄。反応子の数と、終わりの NUL を含む長さの名前
#[cfg(test)]
fn record_named(name: &'static [u8]) -> impl Fn(&mut BitWriter) {
    move |w| {
        w.bl_zero();
        w.bs(name.len() as u16 + 1);
        w.bytes(name);
        w.bytes(&[0]);
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
    let layer = object(0x33, 0x10, record_named(b"0"), |w| w.handle(4, 2));
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
    assert!(matches!(db.objects()[0].origin, Origin::Dwg { offset } if offset == 0x19 + 27 + 18));
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
    let layer = object(0x33, 0x10, record_named(b"WALL"), |w| w.handle(4, 2));
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
    let layer = object(0x33, 0x10, record_named(b"0"), |w| w.handle(4, 2));
    let standard = object(0x35, 0x11, record_named(b"Standard"), |w| w.handle(4, 3));
    let mono = object(0x35, 0x12, record_named(b"Mono"), |w| w.handle(4, 3));
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
    let layer = object(0x33, 0x10, record_named(b"0"), |w| w.handle(4, 2));
    let block = object(0x31, 0x11, record_named(b"DOOR"), |w| w.handle(4, 1));
    let style = object(0x35, 0x12, record_named(b"Mono"), |w| w.handle(4, 3));
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
fn linetypes_are_named_by_their_flags() {
    let layer = object(0x33, 0x10, record_named(b"0"), |w| w.handle(4, 2));
    let dashed = object(0x39, 0x11, record_named(b"DASHED"), |w| w.handle(4, 2));
    let by_layer = object(0x39, 0x12, record_named(b"ByLayer"), |w| w.handle(4, 2));
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
    let layer = object(0x33, 0x10, record_named(b"0"), |w| w.handle(4, 2));
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
fn lwpolylines_have_their_vertices() {
    let layer = object(0x33, 0x10, record_named(b"0"), |w| w.handle(4, 2));
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
    let layer = object(0x33, 0x10, record_named(b"0"), |w| w.handle(4, 2));
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
    let layer = object(0x33, 0x10, record_named(b"0"), |w| w.handle(4, 2));
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
    let layer = object(0x33, 0x10, record_named(b"0"), |w| w.handle(4, 2));
    let style = object(0x45, 0x11, record_named(b"ISO-25"), |w| w.handle(4, 3));
    let block = object(0x31, 0x12, record_named(b"*D1"), |w| w.handle(4, 1));
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
fn types_of_classes_are_named_by_the_classes() {
    let layer = object(0x33, 0x10, record_named(b"0"), |w| w.handle(4, 2));
    let table = object(
        500,
        0x20,
        |w| entity_common(w, 2, 1, 1.0, 0),
        entity_handles(0x10),
    );
    let var = object(501, 0x21, |w| w.bl_zero(), |w| w.handle(4, 0));
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
    let layer = object(0x33, 0x10, record_named(b"0"), |w| w.handle(4, 2));
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
    let layer = object(0x33, 0x10, record_named(b"0"), |w| w.handle(4, 2));
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
    src[..6].copy_from_slice(b"AC1018");
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
