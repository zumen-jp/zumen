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

#[cfg(test)]
fn objects_in_sample() -> Vec<(u64, Vec<u8>)> {
    // 画層の表と、画層
    let control = object(0x32, 2, |w| w.bl_zero(), |w| w.handle(4, 0));
    let layer = object(0x33, 0x10, |w| w.bl_zero(), |w| w.handle(4, 2));
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
    const SENTINEL: [u8; 16] = [
        0x95, 0xA0, 0x4E, 0x28, 0x99, 0x82, 0x1A, 0xE5, 0x5E, 0x41, 0xE0, 0x5F, 0x9D, 0x3A, 0x4D,
        0x00,
    ];
    let header_size = 0x19 + 3 * 9 + 2 + 16;
    let mut body: Vec<u8> = Vec::new();
    let mut map = Vec::new();
    let (mut last_handle, mut last_location) = (0u64, 0i64);
    for (handle, bytes) in objects {
        let location = (header_size + body.len()) as i64;
        map.extend(modular_char((handle - last_handle) as i64, false));
        map.extend(modular_char(location - last_location, true));
        (last_handle, last_location) = (*handle, location);
        body.extend(bytes);
    }
    let mut sections = Vec::new();
    for data in [map, Vec::new()] {
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
