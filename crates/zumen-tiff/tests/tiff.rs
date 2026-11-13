//! `zumen-tiff` の単体・統合テスト（#877）。

#![cfg(test)]

use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits, codes};
use zumen_crs::{Crs, VerticalCrs};
use zumen_drawing::{Drawing2D, Shape, UnderlayFrame, UnderlayKind, Unit};
use zumen_geo::{Form, GeoModel, Position};
use zumen_pointcloud::PointCloud;
use zumen_sdk::{Codec, Error, FileModel, Lift, testkit};
use zumen_tiff::{Endian, Photometric, PlanarConfig, ResolutionUnit, SampleFormat, Tiff, TiffFile};

type RawTag = (u16, u16, u32, Vec<u8>);
type PageSpec = (Vec<RawTag>, Vec<Vec<u8>>, u16, u16);

/// テスト用の最小クラシック TIFF ビルダー（リトルエンディアン・ビッグエンディアン両対応）
struct TiffBuilder {
    be: bool,
}

impl TiffBuilder {
    fn new(be: bool) -> Self {
        Self { be }
    }

    fn u16_bytes(&self, v: u16) -> [u8; 2] {
        if self.be {
            v.to_be_bytes()
        } else {
            v.to_le_bytes()
        }
    }

    fn u32_bytes(&self, v: u32) -> [u8; 4] {
        if self.be {
            v.to_be_bytes()
        } else {
            v.to_le_bytes()
        }
    }

    fn f32_bytes(&self, v: f32) -> [u8; 4] {
        self.u32_bytes(v.to_bits())
    }

    fn f64_bytes(&self, v: f64) -> [u8; 8] {
        if self.be {
            v.to_be_bytes()
        } else {
            v.to_le_bytes()
        }
    }

    fn short(&self, tag: u16, vals: &[u16]) -> RawTag {
        let mut raw = Vec::with_capacity(vals.len() * 2);
        for &v in vals {
            raw.extend_from_slice(&self.u16_bytes(v));
        }
        (tag, 3, vals.len() as u32, raw)
    }

    fn long(&self, tag: u16, vals: &[u32]) -> RawTag {
        let mut raw = Vec::with_capacity(vals.len() * 4);
        for &v in vals {
            raw.extend_from_slice(&self.u32_bytes(v));
        }
        (tag, 4, vals.len() as u32, raw)
    }

    fn rational(&self, tag: u16, num: u32, den: u32) -> RawTag {
        let mut raw = Vec::with_capacity(8);
        raw.extend_from_slice(&self.u32_bytes(num));
        raw.extend_from_slice(&self.u32_bytes(den));
        (tag, 5, 1, raw)
    }

    fn double(&self, tag: u16, vals: &[f64]) -> RawTag {
        let mut raw = Vec::with_capacity(vals.len() * 8);
        for &v in vals {
            raw.extend_from_slice(&self.f64_bytes(v));
        }
        (tag, 12, vals.len() as u32, raw)
    }

    fn ascii(tag: u16, s: &str) -> RawTag {
        let mut raw = s.as_bytes().to_vec();
        raw.push(0);
        (tag, 2, raw.len() as u32, raw)
    }
}

/// 任意のページ群（各ページは追加タグ一覧 + ストリップまたはタイルのチャンク列 + オフセット・バイト数タグ番号）から TIFF バイト列を組み立てる
fn build_tiff(be: bool, pages: Vec<PageSpec>) -> Vec<u8> {
    let b = TiffBuilder::new(be);
    let mut out = Vec::new();
    if be {
        out.extend_from_slice(b"MM\x00*");
    } else {
        out.extend_from_slice(b"II*\x00");
    }
    out.extend_from_slice(&[0; 4]); // first IFD pointer placeholder

    let page_total = pages.len();
    let mut prev_next_ptr_pos: Option<usize> = None;

    for (page_idx, (mut entries, chunks, offsets_tag, counts_tag)) in pages.into_iter().enumerate()
    {
        while out.len() % 2 != 0 {
            out.push(0);
        }
        // チャンク本体を先に配置する
        let mut chunk_offsets = Vec::with_capacity(chunks.len());
        let mut chunk_counts = Vec::with_capacity(chunks.len());
        for ch in &chunks {
            chunk_offsets.push(out.len() as u32);
            chunk_counts.push(ch.len() as u32);
            out.extend_from_slice(ch);
            while out.len() % 2 != 0 {
                out.push(0);
            }
        }
        entries.push(b.long(offsets_tag, &chunk_offsets));
        entries.push(b.long(counts_tag, &chunk_counts));
        entries.sort_by_key(|e| e.0);

        let ifd_offset = out.len() as u32;
        if page_idx == 0 {
            out[4..8].copy_from_slice(&b.u32_bytes(ifd_offset));
        }
        if let Some(prev) = prev_next_ptr_pos {
            out[prev..prev + 4].copy_from_slice(&b.u32_bytes(ifd_offset));
        }

        out.extend_from_slice(&b.u16_bytes(entries.len() as u16));
        let entries_pos = out.len();
        out.resize(entries_pos + entries.len() * 12 + 4, 0);
        let next_ptr_pos = entries_pos + entries.len() * 12;

        for (i, (tag, ftype, count, raw)) in entries.into_iter().enumerate() {
            let pos = entries_pos + i * 12;
            out[pos..pos + 2].copy_from_slice(&b.u16_bytes(tag));
            out[pos + 2..pos + 4].copy_from_slice(&b.u16_bytes(ftype));
            out[pos + 4..pos + 8].copy_from_slice(&b.u32_bytes(count));
            if raw.len() <= 4 {
                out[pos + 8..pos + 8 + raw.len()].copy_from_slice(&raw);
            } else {
                while out.len() % 2 != 0 {
                    out.push(0);
                }
                let ext_off = out.len() as u32;
                out[pos + 8..pos + 12].copy_from_slice(&b.u32_bytes(ext_off));
                out.extend_from_slice(&raw);
            }
        }

        prev_next_ptr_pos = if page_idx + 1 < page_total {
            Some(next_ptr_pos)
        } else {
            None
        };
    }
    out
}

/// `encode_raster_png` が出力した stored-block PNG から RGBA8 画素列を取り出すテスト補助
fn decode_stored_png(png: &[u8]) -> (u32, u32, Vec<u8>) {
    assert_eq!(&png[0..8], b"\x89PNG\r\n\x1a\n");
    let mut pos = 8usize;
    let mut width = 0u32;
    let mut height = 0u32;
    let mut idat = Vec::new();
    while pos < png.len() {
        let len = u32::from_be_bytes(png[pos..pos + 4].try_into().unwrap()) as usize;
        let kind = &png[pos + 4..pos + 8];
        let data = &png[pos + 8..pos + 8 + len];
        let crc_got = u32::from_be_bytes(png[pos + 8 + len..pos + 12 + len].try_into().unwrap());
        let crc_exp = zumen_base::crc::crc32(&png[pos + 4..pos + 8 + len]);
        assert_eq!(crc_got, crc_exp, "PNG CRC mismatch on {:?}", kind);
        match kind {
            b"IHDR" => {
                width = u32::from_be_bytes(data[0..4].try_into().unwrap());
                height = u32::from_be_bytes(data[4..8].try_into().unwrap());
                assert_eq!(&data[8..13], &[8, 6, 0, 0, 0]);
            }
            b"IDAT" => idat.extend_from_slice(data),
            b"IEND" => break,
            _ => {}
        }
        pos += 12 + len;
    }
    assert_eq!(&idat[0..2], &[0x78, 0x01]);
    let mut raw = Vec::new();
    let mut zp = 2usize;
    loop {
        let header = idat[zp];
        let bfinal = header & 1;
        assert_eq!((header >> 1) & 3, 0, "expected stored block");
        let len = u16::from_le_bytes([idat[zp + 1], idat[zp + 2]]) as usize;
        let nlen = u16::from_le_bytes([idat[zp + 3], idat[zp + 4]]);
        assert_eq!(len as u16, !nlen);
        raw.extend_from_slice(&idat[zp + 5..zp + 5 + len]);
        zp += 5 + len;
        if bfinal == 1 {
            break;
        }
    }
    let row_bytes = (width as usize) * 4;
    assert_eq!(raw.len(), (height as usize) * (1 + row_bytes));
    let mut rgba = Vec::with_capacity((width as usize) * (height as usize) * 4);
    for y in 0..(height as usize) {
        let base = y * (1 + row_bytes);
        assert_eq!(raw[base], 0, "expected filter 0");
        rgba.extend_from_slice(&raw[base + 1..base + 1 + row_bytes]);
    }
    (width, height, rgba)
}

fn read_tiff_with_diag(bytes: &[u8]) -> (Result<TiffFile, Error>, Diagnostics) {
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
    let res = Tiff::read(bytes, &mut cx);
    (res, diag)
}

/// テスト用の最小 TIFF LZW エンコーダー（MSB 先頭・early-change）
fn encode_tiff_lzw(data: &[u8]) -> Vec<u8> {
    struct MsbWriter {
        buf: Vec<u8>,
        acc: u64,
        bits: u32,
    }
    impl MsbWriter {
        fn push(&mut self, code: u16, width: u32) {
            self.acc = (self.acc << width) | u64::from(code);
            self.bits += width;
            while self.bits >= 8 {
                self.bits -= 8;
                self.buf.push(((self.acc >> self.bits) & 0xFF) as u8);
            }
        }
        fn finish(mut self) -> Vec<u8> {
            if self.bits > 0 {
                self.buf.push(((self.acc << (8 - self.bits)) & 0xFF) as u8);
            }
            self.buf
        }
    }

    let mut w = MsbWriter {
        buf: Vec::new(),
        acc: 0,
        bits: 0,
    };
    let mut code_size = 9u32;
    w.push(256, code_size); // CLEAR
    if data.is_empty() {
        w.push(257, code_size);
        return w.finish();
    }
    let mut table: std::collections::BTreeMap<Vec<u8>, u16> = std::collections::BTreeMap::new();
    for i in 0..=255u16 {
        table.insert(vec![i as u8], i);
    }
    let mut next_code = 258u16;
    let mut omega = vec![data[0]];
    for &k in &data[1..] {
        let mut cand = omega.clone();
        cand.push(k);
        if table.contains_key(&cand) {
            omega = cand;
        } else {
            w.push(*table.get(&omega).unwrap(), code_size);
            if next_code < 4095 {
                table.insert(cand, next_code);
                next_code += 1;
                if u32::from(next_code) == (1u32 << code_size) && code_size < 12 {
                    code_size += 1;
                }
            } else {
                w.push(256, code_size);
                table.clear();
                for i in 0..=255u16 {
                    table.insert(vec![i as u8], i);
                }
                next_code = 258;
                code_size = 9;
            }
            omega = vec![k];
        }
    }
    w.push(*table.get(&omega).unwrap(), code_size);
    w.push(257, code_size); // EOI
    w.finish()
}

/// テスト用の最小 PackBits エンコーダー
fn encode_packbits(row: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < row.len() {
        let mut run = 1usize;
        while i + run < row.len() && run < 128 && row[i + run] == row[i] {
            run += 1;
        }
        if run >= 2 {
            out.push((257usize - run) as u8);
            out.push(row[i]);
            i += run;
        } else {
            let start = i;
            i += 1;
            while i < row.len() && (i - start) < 128 && !(i + 1 < row.len() && row[i] == row[i + 1])
            {
                i += 1;
            }
            let lit = i - start;
            out.push((lit - 1) as u8);
            out.extend_from_slice(&row[start..i]);
        }
    }
    out
}

/// テスト用の最小 zlib stored-block エンコーダー
fn encode_zlib_stored(raw: &[u8]) -> Vec<u8> {
    let mut out = vec![0x78, 0x01];
    assert!(raw.len() <= 65535);
    out.push(1);
    let len = raw.len() as u16;
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(&(!len).to_le_bytes());
    out.extend_from_slice(raw);
    let mut s1 = 1u32;
    let mut s2 = 0u32;
    for &b in raw {
        s1 = (s1 + u32::from(b)) % 65521;
        s2 = (s2 + s1) % 65521;
    }
    out.extend_from_slice(&((s2 << 16) | s1).to_be_bytes());
    out
}

#[test]
fn reads_little_and_big_endian_rgb_and_encodes_png_and_downsamples() {
    for be in [false, true] {
        let b = TiffBuilder::new(be);
        // 4x2 RGB8 画像
        let pixels: [u8; 24] = [
            255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 0, // row 0
            0, 255, 255, 255, 0, 255, 128, 64, 32, 10, 20, 30, // row 1
        ];
        let tags = vec![
            b.long(256, &[4]),
            b.long(257, &[2]),
            b.short(258, &[8, 8, 8]),
            b.short(259, &[1]),
            b.short(262, &[2]),
            b.short(277, &[3]),
            b.long(278, &[1]),
            b.rational(282, 300, 1),
            b.rational(283, 300, 1),
            b.short(296, &[2]), // inch
        ];
        let bytes = build_tiff(
            be,
            vec![(
                tags,
                vec![pixels[0..12].to_vec(), pixels[12..24].to_vec()],
                273,
                279,
            )],
        );
        let (res, diag) = read_tiff_with_diag(&bytes);
        assert!(diag.is_empty(), "{diag:?}");
        let file = res.unwrap();
        assert!(file.record_count() >= 2);
        assert_eq!(file.endian(), if be { Endian::Big } else { Endian::Little });
        assert_eq!(file.page_count(), 1);
        let p0 = file.page(0).unwrap();
        assert_eq!(p0.width, 4);
        assert_eq!(p0.height, 2);
        assert_eq!(p0.photometric, Some(Photometric::Rgb));
        assert_eq!(p0.planar_config, PlanarConfig::Chunky);
        assert_eq!(p0.resolution_unit, ResolutionUnit::Inch);

        let mut budget = Budget::default();
        let mut d2 = Diagnostics::new();
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut d2, &cancel);
        assert!(matches!(
            Tiff::write(&file, &mut Vec::new(), &mut cx),
            Err(Error::Unsupported)
        ));

        let png = file.encode_raster_png(1, None, &mut cx).unwrap();
        assert_eq!(
            (png.width, png.height, png.source_width, png.source_height),
            (4, 2, 4, 2)
        );
        let (pw, ph, rgba) = decode_stored_png(&png.bytes);
        assert_eq!((pw, ph), (4, 2));
        assert_eq!(&rgba[0..4], &[255, 0, 0, 255]);
        assert_eq!(&rgba[4..8], &[0, 255, 0, 255]);
        assert_eq!(&rgba[28..32], &[10, 20, 30, 255]);

        // max_side = 2 で 2x1 にエリア平均縮小されることを確かめる
        let small = file.encode_raster_png(1, Some(2), &mut cx).unwrap();
        assert_eq!(
            (
                small.width,
                small.height,
                small.source_width,
                small.source_height
            ),
            (2, 1, 4, 2)
        );
        let (sw, sh, srgba) = decode_stored_png(&small.bytes);
        assert_eq!((sw, sh), (2, 1));
        assert_eq!(srgba.len(), 8);
        // 左半分 4 画素 (255,0,0), (0,255,0), (0,255,255), (255,0,255) の平均 = (128, 128, 128, 255)
        assert_eq!(&srgba[0..4], &[128, 128, 128, 255]);

        // Drawing2D への持ち上げ（300 DPI -> mm 実寸）
        let drawing: Drawing2D = file.lift(&mut cx).unwrap();
        assert_eq!(drawing.spaces.len(), 1);
        assert_eq!(drawing.spaces[0].unit, Unit::Millimeter);
        assert_eq!(drawing.spaces[0].elements.len(), 2);
        match &drawing.spaces[0].elements[0].shape {
            Shape::Underlay(u) => {
                assert_eq!(u.kind, UnderlayKind::Image);
                assert_eq!(u.resource.path, ".");
                assert_eq!(u.page.as_deref(), Some("1"));
                match &u.frame {
                    UnderlayFrame::Pixels {
                        size, right, top, ..
                    } => {
                        assert_eq!(*size, [4.0, 2.0]);
                        assert!((right.x - (4.0 * 25.4 / 300.0)).abs() < 1e-9);
                        assert!((top.y - (2.0 * 25.4 / 300.0)).abs() < 1e-9);
                    }
                    other => panic!("expected Pixels frame, got {other:?}"),
                }
            }
            other => panic!("expected Underlay, got {other:?}"),
        }

        // SDK の決定性・レコード順・切り詰め耐性契約
        testkit::records_in_order::<Tiff>(&bytes).unwrap();
        testkit::deterministic::<Tiff>(&bytes).unwrap();
        testkit::stops_at_same_place::<Tiff>(&bytes).unwrap();
        testkit::survives_lift::<Tiff, Drawing2D>(&bytes).unwrap();
        testkit::survives_lift::<Tiff, GeoModel>(&bytes).unwrap();
    }
}

#[test]
fn decodes_lzw_packbits_deflate_predictor2_tiles_planar_and_group4() {
    let b = TiffBuilder::new(false);

    // 1. LZW + Predictor=2（16-bit RGB、タイル分割）
    // 3x2 画素、タイルは 2x2（右端・下端でクリップが働く）
    // タイル (0,0): 2x2 画素、16-bit RGB (1 行 12 バイト)
    let mut tile0_raw = Vec::new();
    // row 0: pixel0=(0x1000, 0x2000, 0x3000), pixel1=(0x1100, 0x2200, 0x3300)
    // Predictor=2 差分後: pixel0=(0x1000,0x2000,0x3000), diff=(0x0100,0x0200,0x0300)
    for v in [0x1000u16, 0x2000, 0x3000, 0x0100, 0x0200, 0x0300] {
        tile0_raw.extend_from_slice(&v.to_le_bytes());
    }
    // row 1: pixel0=(0x4000, 0x5000, 0x6000), pixel1=(0x4000, 0x5000, 0x6000) -> diff=(0,0,0)
    for v in [0x4000u16, 0x5000, 0x6000, 0, 0, 0] {
        tile0_raw.extend_from_slice(&v.to_le_bytes());
    }
    let mut tile1_raw = Vec::new();
    // タイル (1,0): 右端 1 列有効 + 1 列パディング
    for v in [
        0x8000u16, 0x9000, 0xA000, 0, 0, 0, 0xFFFF, 0x0000, 0x8080, 0, 0, 0,
    ] {
        tile1_raw.extend_from_slice(&v.to_le_bytes());
    }
    let lzw_tags = vec![
        b.long(256, &[3]),
        b.long(257, &[2]),
        b.short(258, &[16, 16, 16]),
        b.short(259, &[5]), // LZW
        b.short(262, &[2]), // RGB
        b.short(277, &[3]),
        b.short(317, &[2]), // Predictor = 2
        b.long(322, &[2]),  // TileWidth = 2
        b.long(323, &[2]),  // TileLength = 2
    ];
    let lzw_tiff = build_tiff(
        false,
        vec![(
            lzw_tags,
            vec![encode_tiff_lzw(&tile0_raw), encode_tiff_lzw(&tile1_raw)],
            324,
            325,
        )],
    );
    let (file, diag) = read_tiff_with_diag(&lzw_tiff);
    assert!(diag.is_empty(), "{diag:?}");
    let file = file.unwrap();
    let mut budget = Budget::default();
    let mut d2 = Diagnostics::new();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut d2, &cancel);
    let png = file.encode_raster_png(1, None, &mut cx).unwrap();
    let (_, _, rgba) = decode_stored_png(&png.bytes);
    // 0x1000 / 257 = 16, 0x1100 / 257 = 17, 0xFFFF / 257 = 255
    assert_eq!(&rgba[0..4], &[16, 32, 48, 255]);
    assert_eq!(&rgba[4..8], &[17, 34, 51, 255]);
    assert_eq!(&rgba[20..24], &[255, 0, 128, 255]);

    // 2. PackBits + PlanarConfiguration=2 (Separate planes) + Deflate マルチページ
    let plane_r = encode_packbits(&[200, 200, 200, 50]);
    let plane_g = encode_packbits(&[10, 20, 30, 40]);
    let plane_b = encode_packbits(&[100, 100, 100, 100]);
    let page1_tags = vec![
        b.long(256, &[4]),
        b.long(257, &[1]),
        b.short(258, &[8, 8, 8]),
        b.short(259, &[32773]), // PackBits
        b.short(262, &[2]),
        b.short(277, &[3]),
        b.long(278, &[1]),
        b.short(284, &[2]), // Planar
    ];
    let deflate_chunk = encode_zlib_stored(&[0, 64, 128, 255]);
    let page2_tags = vec![
        b.long(256, &[2]),
        b.long(257, &[2]),
        b.short(258, &[8]),
        b.short(259, &[8]), // Deflate (Adobe)
        b.short(262, &[1]), // BlackIsZero
        b.short(277, &[1]),
        b.long(278, &[2]),
    ];
    let multi_tiff = build_tiff(
        false,
        vec![
            (page1_tags, vec![plane_r, plane_g, plane_b], 273, 279),
            (page2_tags, vec![deflate_chunk], 273, 279),
        ],
    );
    let (mfile, mdiag) = read_tiff_with_diag(&multi_tiff);
    assert!(mdiag.is_empty(), "{mdiag:?}");
    let mfile = mfile.unwrap();
    assert_eq!(mfile.page_count(), 2);
    let p1_png = mfile.encode_raster_png(1, None, &mut cx).unwrap();
    assert_eq!(p1_png.pages, 2);
    let (_, _, p1_rgba) = decode_stored_png(&p1_png.bytes);
    assert_eq!(&p1_rgba[0..4], &[200, 10, 100, 255]);
    assert_eq!(&p1_rgba[12..16], &[50, 40, 100, 255]);

    let p2_png = mfile.encode_raster_png(2, None, &mut cx).unwrap();
    let (_, _, p2_rgba) = decode_stored_png(&p2_png.bytes);
    assert_eq!(&p2_rgba[0..4], &[0, 0, 0, 255]);
    assert_eq!(&p2_rgba[12..16], &[255, 255, 255, 255]);

    // 3. CCITT Group 4（Compression = 4、ITU-T T.6）8x2 2 値画像
    // 参照行は全白（8 画素）。
    // 行 0: 白 4 画素 + 黒 4 画素 -> H モード `001` + 白4 (`1011`) + 黒4 (`011`) = `001_1011_011`
    // 行 1: 行 0 と同一（a1=4, b1=4 -> V(0)=`1`、次の a1=8, b1=8 -> V(0)=`1`）+ EOFB `000000000001000000000001`
    // ビット列: `00110110_11110000_00000001_00000000_00010000`
    let g4_stream = vec![
        0b0011_0110,
        0b1111_0000,
        0b0000_0001,
        0b0000_0000,
        0b0001_0000,
    ];
    let g4_tags = vec![
        b.long(256, &[8]),
        b.long(257, &[2]),
        b.short(258, &[1]),
        b.short(259, &[4]), // CCITT Group 4
        b.short(262, &[0]), // WhiteIsZero: 0=白(255), 1=黒(0)
        b.short(277, &[1]),
        b.long(278, &[2]),
        b.long(293, &[0]), // T6Options = 0
    ];
    let g4_tiff = build_tiff(false, vec![(g4_tags, vec![g4_stream], 273, 279)]);
    let (g4_file, g4_diag) = read_tiff_with_diag(&g4_tiff);
    assert!(g4_diag.is_empty(), "{g4_diag:?}");
    let g4_png = g4_file
        .unwrap()
        .encode_raster_png(1, None, &mut cx)
        .unwrap();
    let (_, _, g4_rgba) = decode_stored_png(&g4_png.bytes);
    // 先頭 4 画素は白 (255)、後半 4 画素は黒 (0)
    assert_eq!(&g4_rgba[0..4], &[255, 255, 255, 255]);
    assert_eq!(&g4_rgba[12..16], &[255, 255, 255, 255]);
    assert_eq!(&g4_rgba[16..20], &[0, 0, 0, 255]);
    assert_eq!(&g4_rgba[28..32], &[0, 0, 0, 255]);
}

#[test]
fn lifts_geotiff_crs_footprint_and_dem_pointcloud_with_nodata_filtering() {
    let b = TiffBuilder::new(false);
    // 2x2 Float32 DEM:
    // JGD2011 平面直角 IX 系 (EPSG:6677), 標高基準 JGD2011 (EPSG:6695), PixelIsArea (1)
    // 左上原点 Easting=1000.0, Northing=2000.0, scale=(2.0, 2.0)
    // セル中心は:
    // (col=0, row=0) -> E=1001.0, N=1999.0, z=10.5
    // (col=1, row=0) -> E=1003.0, N=1999.0, z=-9999.0 (GDAL_NODATA で除外)
    // (col=0, row=1) -> E=1001.0, N=1997.0, z=12.25
    // (col=1, row=1) -> E=1003.0, N=1997.0, z=f32::NAN (非有限値として除外)
    let mut dem_bytes = Vec::new();
    for v in [10.5f32, -9999.0, 12.25, f32::NAN] {
        dem_bytes.extend_from_slice(&b.f32_bytes(v));
    }
    // GeoKeyDirectoryTag (34735): [1, 1, 0, 4,
    //   1024(GTModelTypeGeoKey), 0, 1, 1,
    //   1025(GTRasterTypeGeoKey), 0, 1, 1(PixelIsArea),
    //   3072(ProjectedCSTypeGeoKey), 0, 1, 6677,
    //   4096(VerticalCSTypeGeoKey), 0, 1, 6695]
    let geokeys = [
        1u16, 1, 0, 4, 1024, 0, 1, 1, 1025, 0, 1, 1, 3072, 0, 1, 6677, 4096, 0, 1, 6695,
    ];
    let tags = vec![
        b.long(256, &[2]),
        b.long(257, &[2]),
        b.short(258, &[32]),
        b.short(259, &[1]),
        b.short(262, &[1]),
        b.short(277, &[1]),
        b.long(278, &[2]),
        b.short(339, &[3]), // SampleFormat = IEEEFP (3)
        b.double(33550, &[2.0, 2.0, 0.0]),
        b.double(33922, &[0.0, 0.0, 0.0, 1000.0, 2000.0, 0.0]),
        b.short(34735, &geokeys),
        TiffBuilder::ascii(42113, "-9999"),
    ];
    let bytes = build_tiff(false, vec![(tags, vec![dem_bytes], 273, 279)]);
    let (file, diag) = read_tiff_with_diag(&bytes);
    assert!(diag.is_empty(), "{diag:?}");
    let file = file.unwrap();
    assert!(file.has_dem_grid());
    let p0 = file.page(0).unwrap();
    assert_eq!(p0.sample_format, SampleFormat::Float);
    let geo = p0.geo.as_ref().unwrap();
    assert_eq!(geo.crs, Crs::from_epsg(6677));
    assert_eq!(geo.vertical_crs, Some(VerticalCrs::Jgd2011));

    let mut budget = Budget::default();
    let mut d2 = Diagnostics::new();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut d2, &cancel);

    // GeoModel: 閉じた 5 点の矩形フットプリント [BL(1000,1996), BR(1004,1996), TR(1004,2000), TL(1000,2000), BL]
    let gm: GeoModel = file.lift(&mut cx).unwrap();
    assert_eq!(gm.crs, Crs::from_epsg(6677));
    assert_eq!(gm.height, Some(VerticalCrs::Jgd2011));
    assert_eq!(gm.features.len(), 1);
    assert_eq!(gm.features[0].kind, "raster_footprint");
    match &gm.features[0].geometries[0].form {
        Form::Area { outer, holes } => {
            assert!(holes.is_empty());
            assert_eq!(outer.len(), 5);
            match outer[0].position {
                Position::Map(mp) => {
                    assert!((mp.easting - 1000.0).abs() < 1e-9);
                    assert!((mp.northing - 1996.0).abs() < 1e-9);
                }
                other => panic!("expected MapPoint, got {other:?}"),
            }
        }
        other => panic!("expected Area, got {other:?}"),
    }

    // PointCloud: 有効な 2 セルのみ持ち上げられる
    let pc: PointCloud = file.lift(&mut cx).unwrap();
    assert_eq!(pc.crs, Crs::from_epsg(6677));
    assert_eq!(pc.vertical, Some(VerticalCrs::Jgd2011));
    assert_eq!(pc.points.len(), 2);
    assert_eq!(pc.points.easting, vec![1001.0, 1001.0]);
    assert_eq!(pc.points.northing, vec![1999.0, 1997.0]);
    assert_eq!(pc.points.height, vec![10.5, 12.25]);

    testkit::survives_lift::<Tiff, PointCloud>(&bytes).unwrap();
}

#[test]
fn rejects_bigtiff_ifd_cycles_and_enforces_decoded_bytes_budget() {
    // 1. BigTIFF (magic = 43) は tiff.format.unsupported と tiff.file.invalid を返す
    let bigtiff = b"II+\x00\x08\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00";
    let (res, diag) = read_tiff_with_diag(bigtiff);
    assert!(res.is_err());
    let codes_got: Vec<_> = diag.iter().map(|d| d.code()).collect();
    assert!(codes_got.contains(&codes::TIFF_FORMAT_UNSUPPORTED));
    assert!(codes_got.contains(&codes::TIFF_FILE_INVALID));

    // 2. IFD の循環ポインター（next_ifd が自分自身を指す）は tiff.file.invalid で止まる
    let b = TiffBuilder::new(false);
    let tags = vec![
        b.long(256, &[1]),
        b.long(257, &[1]),
        b.short(258, &[8]),
        b.short(259, &[1]),
        b.short(262, &[1]),
        b.short(277, &[1]),
        b.long(278, &[1]),
    ];
    let mut cyc = build_tiff(false, vec![(tags, vec![vec![0u8]], 273, 279)]);
    let ifd_off = u32::from_le_bytes(cyc[4..8].try_into().unwrap()) as usize;
    let cnt = u16::from_le_bytes(cyc[ifd_off..ifd_off + 2].try_into().unwrap()) as usize;
    let next_pos = ifd_off + 2 + cnt * 12;
    let self_off = (ifd_off as u32).to_le_bytes();
    cyc[next_pos..next_pos + 4].copy_from_slice(&self_off);
    let (cres, cdiag) = read_tiff_with_diag(&cyc);
    assert!(cres.is_err());
    assert!(cdiag.iter().any(|d| d.code() == codes::TIFF_FILE_INVALID));

    // 3. 圧縮爆弾（小さな PackBits 入力から大きな展開バイト列を要求）は budget.decoded-bytes.exceeded で止まる
    // 128 バイトの繰り返しを 16 行 = 2048 バイト要求し、decoded_bytes 上限を 512 に絞る
    let bomb_strip: Vec<u8> = (0..16).flat_map(|_| [0x81u8, 0xFF]).collect();
    let bomb_tags = vec![
        b.long(256, &[128]),
        b.long(257, &[16]),
        b.short(258, &[8]),
        b.short(259, &[32773]),
        b.short(262, &[1]),
        b.short(277, &[1]),
        b.long(278, &[16]),
    ];
    let bomb_tiff = build_tiff(false, vec![(bomb_tags, vec![bomb_strip], 273, 279)]);
    let mut budget = Budget::new(Limits::default().with(Kind::DecodedBytes, Limit::Finite(512)));
    let mut bdiag = Diagnostics::new();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut bdiag, &cancel);
    assert!(matches!(
        Tiff::read(&bomb_tiff, &mut cx),
        Err(Error::Stopped(_))
    ));
    assert!(
        bdiag
            .iter()
            .any(|d| d.code() == codes::BUDGET_DECODED_BYTES_EXCEEDED)
    );

    // 4. CCITT Group 4 の超広幅行（w = 8,000,000、1 行 1,000,000 バイト）は行バッファー確保前に DecodedBytes 予算で止まる
    let wide_g4_tags = vec![
        b.long(256, &[8_000_000]),
        b.long(257, &[1]),
        b.short(258, &[1]),
        b.short(259, &[4]),
        b.short(262, &[0]),
        b.short(277, &[1]),
        b.long(278, &[1]),
    ];
    let wide_g4_tiff = build_tiff(false, vec![(wide_g4_tags, vec![vec![0xFF; 16]], 273, 279)]);
    let mut g4_budget = Budget::new(Limits::default().with(Kind::DecodedBytes, Limit::Finite(512)));
    let mut g4_bdiag = Diagnostics::new();
    let mut g4_cx = Cx::new(&mut g4_budget, &mut g4_bdiag, &cancel);
    assert!(matches!(
        Tiff::read(&wide_g4_tiff, &mut g4_cx),
        Err(Error::Stopped(_))
    ));
    assert!(
        g4_bdiag
            .iter()
            .any(|d| d.code() == codes::BUDGET_DECODED_BYTES_EXCEEDED)
    );
    // ImageWidth = u32::MAX（row_bytes = 536,870,912 <= MAX_ALLOC だがタイル幅過大や予算停止でもパニックしない）
    let u32max_g4_tags = vec![
        b.long(256, &[u32::MAX]),
        b.long(257, &[1]),
        b.short(258, &[1]),
        b.short(259, &[4]),
        b.short(262, &[0]),
        b.short(277, &[1]),
        b.long(278, &[1]),
    ];
    let u32max_g4_tiff = build_tiff(
        false,
        vec![(u32max_g4_tags, vec![vec![0xFF; 16]], 273, 279)],
    );
    let mut max_budget =
        Budget::new(Limits::default().with(Kind::DecodedBytes, Limit::Finite(512)));
    let mut max_diag = Diagnostics::new();
    let mut max_cx = Cx::new(&mut max_budget, &mut max_diag, &cancel);
    assert!(matches!(
        Tiff::read(&u32max_g4_tiff, &mut max_cx),
        Err(Error::Stopped(_))
    ));

    // 5. タイル画像で小さな 1 タイルを共有して巨大キャンバス（2048x2048 = 4 MiB）を要求する入力は DecodedBytes で止まる
    let tile_count = (2048 / 16) * (2048 / 16);
    // 全タイルが同一の圧縮チャンクを指す最小 TIFF を構築する
    let tile_tiff = {
        let chunk = encode_zlib_stored(&[0u8; 256]);
        let mut buf = Vec::from(b"II*\x00\x08\x00\x00\x00".as_slice());
        let entries_count = 10u16;
        let ifd_bytes = 2 + usize::from(entries_count) * 12 + 4;
        let chunk_off = (8 + ifd_bytes) as u32;
        let offsets_off = chunk_off + (chunk.len() as u32);
        let counts_off = offsets_off + (tile_count as u32) * 4;
        buf.extend_from_slice(&entries_count.to_le_bytes());
        let mut write_entry = |tag: u16, ftype: u16, count: u32, val_or_off: u32| {
            buf.extend_from_slice(&tag.to_le_bytes());
            buf.extend_from_slice(&ftype.to_le_bytes());
            buf.extend_from_slice(&count.to_le_bytes());
            buf.extend_from_slice(&val_or_off.to_le_bytes());
        };
        write_entry(256, 4, 1, 2048);
        write_entry(257, 4, 1, 2048);
        write_entry(258, 3, 1, 8);
        write_entry(259, 3, 1, 8);
        write_entry(262, 3, 1, 1);
        write_entry(277, 3, 1, 1);
        write_entry(322, 4, 1, 16);
        write_entry(323, 4, 1, 16);
        write_entry(324, 4, tile_count as u32, offsets_off);
        write_entry(325, 4, tile_count as u32, counts_off);
        buf.extend_from_slice(&0u32.to_le_bytes());
        buf.extend_from_slice(&chunk);
        for _ in 0..tile_count {
            buf.extend_from_slice(&chunk_off.to_le_bytes());
        }
        for _ in 0..tile_count {
            buf.extend_from_slice(&(chunk.len() as u32).to_le_bytes());
        }
        buf
    };
    let mut t_budget =
        Budget::new(Limits::default().with(Kind::DecodedBytes, Limit::Finite(256 * 1024)));
    let mut t_diag = Diagnostics::new();
    let mut t_cx = Cx::new(&mut t_budget, &mut t_diag, &cancel);
    assert!(matches!(
        Tiff::read(&tile_tiff, &mut t_cx),
        Err(Error::Stopped(_))
    ));
    assert!(
        t_diag
            .iter()
            .any(|d| d.code() == codes::BUDGET_DECODED_BYTES_EXCEEDED)
    );
}

#[test]
fn lzw_multi_bit_transitions_orientation_geotiff_and_sample_format_diagnostics() {
    let b = TiffBuilder::new(false);

    // 1. 9->10->11->12 ビット遷移と 4096 辞書リセットをまたぐ 16 KiB の LZW ラウンドトリップ
    let mut pattern = Vec::with_capacity(16384);
    let mut state = 0x1234_5678u32;
    for i in 0..16384usize {
        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        pattern.push(((state >> 16) as u8) ^ ((i & 0x7F) as u8));
    }
    let lzw_stream = encode_tiff_lzw(&pattern);
    let lzw_tags = vec![
        b.long(256, &[128]),
        b.long(257, &[128]),
        b.short(258, &[8]),
        b.short(259, &[5]),
        b.short(262, &[1]),
        b.short(277, &[1]),
        b.long(278, &[128]),
    ];
    let lzw_bytes = build_tiff(false, vec![(lzw_tags, vec![lzw_stream], 273, 279)]);
    let (lzw_file, lzw_diag) = read_tiff_with_diag(&lzw_bytes);
    assert!(lzw_diag.is_empty(), "{lzw_diag:?}");
    let lzw_file = lzw_file.unwrap();
    let mut budget = Budget::default();
    let mut d2 = Diagnostics::new();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut d2, &cancel);
    let png = lzw_file.encode_raster_png(1, None, &mut cx).unwrap();
    let (pw, ph, rgba) = decode_stored_png(&png.bytes);
    assert_eq!((pw, ph), (128, 128));
    for (idx, &expected) in pattern.iter().enumerate() {
        assert_eq!(rgba[idx * 4], expected, "mismatch at pixel {idx}");
    }

    // 2. Orientation = 6 の GeoTIFF DEM で画素の向きと GeoTransform（外枠・点群座標）が一致する
    // 格納格子は raw_w = 2, raw_h = 1（raw(0,0)=15.0, raw(1,0)=25.0）
    // raw(col, row) に対する変換: E = 1000 + 10*col, N = 2000 - 10*row（PixelIsArea）
    // Orientation = 6 では表示寸法が (disp_w=1, disp_h=2) となり:
    // disp(0, 0) -> raw(0, 0)=15.0 (E=1005.0, N=1995.0)
    // disp(0, 1) -> raw(1, 0)=25.0 (E=1015.0, N=1995.0)
    let mut ori_dem = Vec::new();
    for v in [15.0f32, 25.0] {
        ori_dem.extend_from_slice(&b.f32_bytes(v));
    }
    let geokeys = [
        1u16, 1, 0, 3, 1024, 0, 1, 1, 1025, 0, 1, 1, 3072, 0, 1, 6677,
    ];
    let ori_tags = vec![
        b.long(256, &[2]),
        b.long(257, &[1]),
        b.short(258, &[32]),
        b.short(259, &[1]),
        b.short(262, &[1]),
        b.short(274, &[6]), // Orientation = 6
        b.short(277, &[1]),
        b.long(278, &[1]),
        b.short(339, &[3]),
        b.double(33550, &[10.0, 10.0, 0.0]),
        b.double(33922, &[0.0, 0.0, 0.0, 1000.0, 2000.0, 0.0]),
        b.short(34735, &geokeys),
    ];
    let ori_bytes = build_tiff(false, vec![(ori_tags, vec![ori_dem], 273, 279)]);
    let (ori_file, ori_diag) = read_tiff_with_diag(&ori_bytes);
    assert!(ori_diag.is_empty(), "{ori_diag:?}");
    let ori_file = ori_file.unwrap();
    assert_eq!(ori_file.page(0).unwrap().width, 1);
    assert_eq!(ori_file.page(0).unwrap().height, 2);
    let ori_pc: PointCloud = ori_file.lift(&mut cx).unwrap();
    assert_eq!(ori_pc.points.len(), 2);
    assert_eq!(ori_pc.points.easting, vec![1005.0, 1015.0]);
    assert_eq!(ori_pc.points.northing, vec![1995.0, 1995.0]);
    assert_eq!(ori_pc.points.height, vec![15.0, 25.0]);

    // 3. 未対応の SampleFormat = 99 は tiff.format.unsupported を 1 件だけ出して未対応ページとする
    let sf_tags = vec![
        b.long(256, &[1]),
        b.long(257, &[1]),
        b.short(258, &[8]),
        b.short(259, &[1]),
        b.short(262, &[1]),
        b.short(277, &[1]),
        b.long(278, &[1]),
        b.short(339, &[99]),
    ];
    let sf_bytes = build_tiff(false, vec![(sf_tags, vec![vec![0u8]], 273, 279)]);
    let (sf_file, sf_diag) = read_tiff_with_diag(&sf_bytes);
    let sf_file = sf_file.unwrap();
    let unsupported_count = sf_diag
        .iter()
        .filter(|d| d.code() == codes::TIFF_FORMAT_UNSUPPORTED)
        .count();
    assert_eq!(unsupported_count, 1, "{sf_diag:?}");
    assert!(matches!(
        sf_file.encode_raster_png(1, None, &mut cx),
        Err(Error::Unsupported)
    ));
}
