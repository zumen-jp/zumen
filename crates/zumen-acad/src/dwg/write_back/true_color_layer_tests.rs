use super::*;

fn int(code: i16, value: i64) -> Value {
    Value {
        code,
        data: Data::Int(value),
    }
}

/// R2004 の LAYER の、フラグと色のフィールド。印刷し、既定の線幅で、色は番号 `index` と真の色 `rgb`
fn fields(index: i16, rgb: u32) -> Vec<u8> {
    let mut w = BitWriter::default();
    w.bs(16 | (31 << 5));
    w.bs(index);
    w.bl(i32::from_ne_bytes(((0xC2_u32 << 24) | rgb).to_ne_bytes()));
    w.rc(0);
    w.into_bytes()
}

/// 画層を `color`（62）で書き直し、フラグと CMC を読み直す
fn rewrite(index: i16, color: i64) -> Option<(i16, i16, Option<u32>)> {
    let data = fields(index, 0x12_34_56);
    let object = Object {
        kind: b"LAYER".to_vec(),
        section: Section::Tables,
        origin: Origin::New,
        values: vec![
            int(70, 0),
            int(62, color),
            int(420, 0x12_34_56),
            int(370, -3),
        ],
    };
    let mut out = BitWriter::default();
    layer(&mut out, &object, false, &data, 0, Release::R2004)?;
    let bytes = out.into_bytes();
    let mut bits = Bits::with_release(&bytes, Release::R2004);
    let flags = bits.bs()?;
    let cmc = bits.cmc()?;
    Some((flags, cmc.index, cmc.rgb))
}

/// 番号を持たない（AutoCAD の）真の色の画層は、読むときに 62 を 7 にする。7 のまま表示しない状態だけを変えたら、
/// 元の CMC（番号 0）を写し、表示しないフラグ（2）を立てる（#743）
#[test]
fn hiding_true_color_layers_keeps_their_cmc() {
    assert_eq!(
        rewrite(0, -7),
        Some((16 | (31 << 5) | 2, 0, Some(0x12_34_56)))
    );
    assert_eq!(rewrite(0, 7), Some((16 | (31 << 5), 0, Some(0x12_34_56))));
    // 番号を変えたら、その番号を書く
    assert_eq!(
        rewrite(0, -3),
        Some((16 | (31 << 5) | 2, 3, Some(0x12_34_56)))
    );
    // 番号を持つ画層（zumen が書いた画層）は、番号を保つ
    assert_eq!(
        rewrite(5, -5),
        Some((16 | (31 << 5) | 2, 5, Some(0x12_34_56)))
    );
}
