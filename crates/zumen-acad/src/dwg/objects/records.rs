//! 表のレコード（LAYER・STYLE・LTYPE・VIEW・UCS・VPORT・DIMSTYLE・BLOCK_RECORD・APPID）のフィールドの読み取り。

use super::*;

/// 表のレコードの共通のフィールド（名前 2・参照されるフラグ・外部参照の番号・外部参照に依るフラグ）と、
/// LAYER・STYLE・LTYPE のフィールド（ODS 5.4.1、R2000）。外部参照に依るフラグは、DXF と同じく 70 の 16 とする。
/// ハンドルの並びは、外部参照のブロックの後に型ごとのハンドルが続く
pub(super) fn table_record(
    bits: &mut Bits,
    type_code: u16,
    handles: &mut Vec<i16>,
) -> Option<Vec<Value>> {
    let name = bits.text()?;
    // 参照されるフラグ（64）は、DXF に書かれないので置かない。外部参照の番号は、R2007 からの図面には
    // ない（ODS 5.4.1）
    bits.bit()?;
    if bits.release() < Release::R2007 {
        bits.bs()?;
    }
    let xdep = if bits.bit()? { 16 } else { 0 };
    handles.push(0);
    let mut values = vec![Value {
        code: 2,
        data: Data::Text(name),
    }];
    match type_code {
        LAYER => {
            // 凍結 1・表示しない 2・新しいビューポートで凍結 4・ロック 8・印刷 16 と、線の太さの番号
            let flags = bits.bs()?;
            let dxf = [(1, 1), (4, 2), (8, 4)]
                .iter()
                .filter(|(dwg, _)| flags & dwg != 0)
                .fold(xdep, |sum, (_, dxf)| sum | dxf);
            values.push(int(70, dxf));
            // 表示しない画層は、DXF と同じく色を負にする
            // 真の色は 420 に置き、色見本帳の色なら名前（430）も置く。真の色の番号（BS）は、AutoCAD では 0 で（ODS 5.4.1 の
            // 2.11）、zumen が書いた画層は番号を持つ（#723）。DXF は真の色の画層にも 62 に近い番号を置くが、近さの決まりが
            // 分からないので、番号が 1〜255 でない画層は、表示しないときだけ 62 に -7 を置き、表示しない状態を移す。
            // 表示する画層に 7 を置くと、描くときに ACI 7 の前景の色になり、真の色を失うので置かない（#743）
            let (color, name) = bits.cmc_named()?;
            let index = i64::from(color.index);
            let hidden = flags & 2 != 0;
            match color.rgb {
                Some(_) if !(1..=255).contains(&index) => {
                    if hidden {
                        values.push(int(62, -7));
                    }
                }
                _ => values.push(int(62, if hidden { -index } else { index })),
            }
            if let Some(rgb) = color.rgb {
                values.push(int(420, i64::from(rgb)));
            }
            if let Some(name) = name {
                values.push(text_value(430, &name));
            }
            if flags & 16 == 0 {
                values.push(Value {
                    code: 290,
                    data: Data::Bool(false),
                });
            }
            values.push(int(
                370,
                lineweight(u16::try_from((flags & 0x03E0) >> 5).ok()?)?,
            ));
            // 印刷スタイル（390）と線種（6）。R2007 からは、その間に材質（347）がある（決定 0094）
            if bits.release() >= Release::R2007 {
                handles.extend([390, 347, 6]);
            } else {
                handles.extend([390, 6]);
            }
        }
        STYLE => {
            // 形のファイルのフラグ（70 の 1）と、縦書きのフラグ（70 の 4）
            let shape = bits.bit()?;
            let vertical = bits.bit()?;
            let flags = xdep | if shape { 1 } else { 0 } | if vertical { 4 } else { 0 };
            values.push(int(70, flags));
            values.push(real(40, bits.bd()?));
            values.push(real(41, bits.bd()?));
            values.push(real(50, bits.bd()?.to_degrees()));
            values.push(int(71, i64::from(bits.rc()?)));
            values.push(real(42, bits.bd()?));
            values.push(text_value(3, &bits.text()?));
            values.push(text_value(4, &bits.text()?));
        }
        LTYPE => {
            values.push(int(70, xdep));
            values.push(text_value(3, &bits.text()?));
            let length = bits.bd()?;
            values.push(int(72, i64::from(bits.rc()?)));
            let count = bits.rc()?;
            values.push(int(73, i64::from(count)));
            values.push(real(40, length));
            let mut dashes = Vec::new();
            for _ in 0..count {
                let dash = bits.bd()?;
                let shape = bits.bs()?;
                let (x, y) = (bits.rd()?, bits.rd()?);
                let (scale, rotation) = (bits.bd()?, bits.bd()?);
                let flags = bits.bs()?;
                dashes.push((dash, shape, x, y, scale, rotation, flags));
            }
            // 文字の要素の文字列は、256 バイトの領域に、形の番号の位置から 0 で終わる。R2007 からは、
            // 文字の要素があるときだけ 512 バイトの領域があり、文字列は UTF-16LE
            let unicode = bits.release() >= Release::R2007;
            let area = if !unicode {
                raw_bytes(bits, 256)?
            } else if dashes.iter().any(|dash| dash.6 & 2 != 0) {
                raw_bytes(bits, 512)?
            } else {
                Vec::new()
            };
            // DWG の 8 は文字の要素に付くフラグで、R2013 より前の DXF には書かれない
            let mask = if bits.release() >= Release::R2013 {
                15
            } else {
                7
            };
            for (dash, shape, x, y, scale, rotation, flags) in dashes {
                values.push(real(49, dash));
                values.push(int(74, i64::from(flags & mask)));
                // 要素ごとに、形のファイルの文字スタイルのハンドル（340）を持つ。形も文字もない要素の
                // ハンドルは置かない。DXF と同じく形の番号（75）の後に置くので、ハンドルのストリームを読むまで
                // 仮の値（`SLOT`）を置く
                if flags & 6 == 0 {
                    handles.push(0);
                    continue;
                }
                handles.push(340);
                let text = flags & 2 != 0;
                values.push(int(75, if text { 0 } else { i64::from(shape) }));
                values.push(Value {
                    code: 340,
                    data: Data::Handle(SLOT),
                });
                values.extend([
                    real(46, scale),
                    real(50, rotation.to_degrees()),
                    real(44, x),
                    real(45, y),
                ]);
                if text {
                    let rest = area.get(usize::try_from(shape).ok()?..)?;
                    let text = if unicode {
                        let units: Vec<u16> = rest
                            .as_chunks::<2>()
                            .0
                            .iter()
                            .map(|&pair| u16::from_le_bytes(pair))
                            .take_while(|&unit| unit != 0)
                            .collect();
                        String::from_utf16_lossy(&units).into_bytes()
                    } else {
                        let end = rest.iter().position(|&b| b == 0)?;
                        rest.get(..end)?.to_vec()
                    };
                    values.push(text_value(9, &text));
                }
            }
        }
        VIEW => values.extend(view(bits, xdep, handles)?),
        UCS => {
            values.push(int(70, xdep));
            values.extend(ucs(bits)?);
            handles.extend([346, 0]);
        }
        VPORT => {
            values.push(int(70, xdep));
            values.extend(vport(bits)?);
            // R2007 からは、外部参照のブロックの後に、背景・表示スタイル・太陽のハンドルが続く
            if bits.release() >= Release::R2007 {
                handles.extend([0, 0, 0]);
            }
            handles.extend([345, 346]);
        }
        DIMSTYLE => {
            values.push(int(70, xdep));
            values.extend(dimstyle(bits)?);
            handles.extend([340, 341, 342, 343, 344]);
            // R2007 からは、寸法線と 2 つの補助線の線種のハンドルが続く
            if bits.release() >= Release::R2007 {
                handles.extend([345, 346, 347]);
            }
        }
        // BLOCK_RECORD のフラグ・基点・外部参照のパス・説明は、DXF では BLOCK に書かれる。ここで読んで、
        // 表を作るときに BLOCK へ移す（BLOCK_FIELDS）
        BLOCK_RECORD => {
            // 名前のない 1・属性の定義を持つ 2・外部参照 4・重ねる外部参照 8 と、読み込んだ外部参照（32）
            let mut flags = xdep;
            for bit in [1, 2, 4, 8] {
                if bits.bit()? {
                    flags |= bit;
                }
            }
            if bits.bit()? && flags & 4 != 0 {
                flags |= 32;
            }
            if bits.release() >= Release::R2004 {
                bits.bl()?;
            }
            values.push(int(70, flags));
            values.extend(point3(bits, 10)?);
            let path = bits.text()?;
            if !path.is_empty() {
                values.push(text_value(1, &path));
            }
            // 参照する INSERT の数だけ 0 でない RC が並び、0 で終わる
            while bits.rc()? != 0 {}
            let description = bits.text()?;
            if !description.is_empty() {
                values.push(text_value(4, &description));
            }
            // R2000 からは、絵の大きさ（BL）と絵のバイト列が続き、R2007 から（読めるバージョンでは R2010 から）は、挿入の単位（BS）・
            // 分解できるか（B）・縮尺を揃えるか（RC）が続く（ODS の BLOCK_HEADER の節）。挿入の単位は、
            // DXF と同じく BLOCK_RECORD の 70 に置く。フラグの 70 より後に置くので、BLOCK へは移らない
            if bits.release() >= Release::R2000 {
                let size = usize::try_from(bits.bl()?).ok()?;
                bits.skip_bytes(size)?;
            }
            if bits.release() >= Release::R2007 {
                values.push(int(70, i64::from(bits.bs()?)));
                values.push(int(280, i64::from(bits.bit()?)));
                values.push(int(281, i64::from(bits.rc()?)));
            }
        }
        _ => {
            values.push(int(70, xdep));
            // APPID は、使い道の分からない RC を持つ
            if type_code == APPID {
                bits.rc()?;
            }
        }
    }
    Some(values)
}

/// VIEW・VPORT の見え方のフラグ（71）。4 つのビットは、透視（1）・前の切り取り（2）・後ろの切り取り（4）・
/// 前の切り取りが目の位置にあること（DXF の 16 の逆）の順
pub(super) fn view_mode(bits: &mut Bits) -> Option<i64> {
    let mut mode = 0;
    for flag in [1, 2, 4] {
        if bits.bit()? {
            mode |= flag;
        }
    }
    if !bits.bit()? {
        mode |= 16;
    }
    Some(mode)
}

/// VIEW のフィールド（ODS 5.4.1 の VIEW の節、R2000）。高さ（40）・中心（10）・幅（41）・向き（11）・
/// 注視点（12）・レンズ（42）・切り取り（43・44）・ねじれ（50、度に直す）・見え方のフラグ（71）・
/// 描き方（281）と、UCS を伴うフラグ（72）を置く。UCS を伴うときは、UCS の原点と軸（110〜112）・
/// 正投影の種類（79）・高さ（146）も置き、ハンドルの並びでは基準の UCS（346）と名前の UCS（345）が続く。
/// 紙空間のフラグは、70 の 1 とする
pub(in crate::dwg) fn view(
    bits: &mut Bits,
    xdep: i64,
    handles: &mut Vec<i16>,
) -> Option<Vec<Value>> {
    let height = bits.bd()?;
    let width = bits.bd()?;
    let center = point2(bits, 10)?;
    let target = point3(bits, 12)?;
    let direction = point3(bits, 11)?;
    let twist = bits.bd()?;
    let (lens, front, back) = (bits.bd()?, bits.bd()?, bits.bd()?);
    let mode = view_mode(bits)?;
    let render = bits.rc()?;
    // R2007 からは、既定の照明を使うか（B）・照明の種類（RC）・明るさと対比（BD）・環境の色（CMC）が
    // 続く（図面で確かめた）
    if bits.release() >= Release::R2007 {
        bits.bit()?;
        bits.rc()?;
        bits.bd()?;
        bits.bd()?;
        bits.cmc()?;
    }
    let paper = bits.bit()?;
    let mut values = vec![int(70, xdep | i64::from(paper)), real(40, height)];
    values.extend(center);
    values.push(real(41, width));
    values.extend(direction);
    values.extend(target);
    values.extend([
        real(42, lens),
        real(43, front),
        real(44, back),
        real(50, twist.to_degrees()),
        int(71, mode),
        int(281, i64::from(render)),
    ]);
    let associated = bits.bit()?;
    values.push(int(72, i64::from(associated)));
    // R2007 からは、外部参照のブロックの後に、背景・表示スタイル・太陽のハンドルが続く
    if bits.release() >= Release::R2007 {
        handles.extend([0, 0, 0]);
    }
    if associated {
        for code in [110, 111, 112] {
            values.extend(point3(bits, code)?);
        }
        let elevation = bits.bd()?;
        values.push(int(79, i64::from(bits.bs()?)));
        values.push(real(146, elevation));
        handles.extend([346, 345]);
    }
    Some(values)
}

/// UCS のフィールド（ODS 5.4.1 の UCS の節、R2000）。原点（10）・X 軸（11）・Y 軸（12）・正投影の種類（79）・
/// 高さ（146）を置く。後に続く正投影の型（BS）は、DXF の 71 と 13 の組と並びが合わないので置かない
pub(super) fn ucs(bits: &mut Bits) -> Option<Vec<Value>> {
    let mut values = point3(bits, 10)?;
    values.extend(point3(bits, 11)?);
    values.extend(point3(bits, 12)?);
    let elevation = bits.bd()?;
    values.push(int(79, i64::from(bits.bs()?)));
    values.push(real(146, elevation));
    bits.bs()?;
    Some(values)
}

/// VPORT のフィールド（ODS 5.4.1 の VPORT の節、R2000）。DWG は幅を高さとの積で持つので、DXF と同じく
/// 高さとの比（41）に直す。UCS に従うフラグは、見え方のフラグ（71）の 8 とする
pub(in crate::dwg) fn vport(bits: &mut Bits) -> Option<Vec<Value>> {
    let height = bits.bd()?;
    let width = bits.bd()?;
    let center = point2(bits, 12)?;
    let target = point3(bits, 17)?;
    let direction = point3(bits, 16)?;
    let twist = bits.bd()?;
    let (lens, front, back) = (bits.bd()?, bits.bd()?, bits.bd()?);
    let mode = view_mode(bits)?;
    let render = bits.rc()?;
    // R2007 からは、VIEW と同じく照明のフィールドが続く（図面で確かめた、決定 0096）
    if bits.release() >= Release::R2007 {
        bits.bit()?;
        bits.rc()?;
        bits.bd()?;
        bits.bd()?;
        bits.cmc()?;
    }
    let mut values = point2(bits, 10)?;
    values.extend(point2(bits, 11)?);
    values.extend(center);
    let follow = if bits.bit()? { 8 } else { 0 };
    let zoom = bits.bs()?;
    let fast = bits.bit()?;
    let icon = bits.bb()?;
    let grid = bits.bit()?;
    let grid_spacing = point2(bits, 15)?;
    let snap = bits.bit()?;
    let snap_style = bits.bit()?;
    let isopair = bits.bs()?;
    let snap_rotation = bits.bd()?;
    values.extend(point2(bits, 13)?);
    values.extend(point2(bits, 14)?);
    values.extend(grid_spacing);
    values.extend(direction);
    values.extend(target);
    values.extend([
        real(40, height),
        real(41, if height == 0.0 { width } else { width / height }),
        real(42, lens),
        real(43, front),
        real(44, back),
        real(50, snap_rotation.to_degrees()),
        real(51, twist.to_degrees()),
        int(71, mode | follow),
        int(72, i64::from(zoom)),
        int(73, i64::from(fast)),
        int(74, i64::from(icon)),
        int(75, i64::from(snap)),
        int(76, i64::from(grid)),
        int(77, i64::from(snap_style)),
        int(78, i64::from(isopair)),
        int(281, i64::from(render)),
    ]);
    // 使い道の分からない B の後に、ビューポートごとの UCS のフラグが続く
    bits.bit()?;
    values.push(int(65, i64::from(bits.bit()?)));
    for code in [110, 111, 112] {
        values.extend(point3(bits, code)?);
    }
    let elevation = bits.bd()?;
    values.push(int(79, i64::from(bits.bs()?)));
    values.push(real(146, elevation));
    Some(values)
}

/// DIMSTYLE のフィールドの読み方
#[derive(Clone, Copy)]
pub(in crate::dwg) enum Dim {
    Text,
    Real(f64),
    Bit(bool),
    Short(i16),
    /// CMC の色
    Color(i16),
}

/// DIMSTYLE のフィールド（ODS 5.4.1 の DIMSTYLE の節、R2000）の、DWG での並びと、グループ・コードと、
/// DXF が書かない既定の値
pub(in crate::dwg) const DIM_FIELDS: [(i16, Dim); 59] = {
    use Dim::{Bit, Color, Real, Short, Text};
    [
        (3, Text),
        (4, Text),
        (40, Real(1.0)),
        (41, Real(0.18)),
        (42, Real(0.0625)),
        (43, Real(0.38)),
        (44, Real(0.18)),
        (45, Real(0.0)),
        (46, Real(0.0)),
        (47, Real(0.0)),
        (48, Real(0.0)),
        (71, Bit(false)),
        (72, Bit(false)),
        (73, Bit(true)),
        (74, Bit(true)),
        (75, Bit(false)),
        (76, Bit(false)),
        (77, Short(0)),
        (78, Short(0)),
        (79, Short(0)),
        (140, Real(0.18)),
        (141, Real(0.09)),
        (142, Real(0.0)),
        (143, Real(25.4)),
        (144, Real(1.0)),
        (145, Real(0.0)),
        (146, Real(1.0)),
        (147, Real(0.09)),
        (148, Real(0.0)),
        (170, Bit(false)),
        (171, Short(2)),
        (172, Bit(false)),
        (173, Bit(false)),
        (174, Bit(false)),
        (175, Bit(false)),
        (176, Color(0)),
        (177, Color(0)),
        (178, Color(0)),
        (179, Short(0)),
        (271, Short(4)),
        (272, Short(4)),
        (273, Short(2)),
        (274, Short(2)),
        (275, Short(0)),
        (276, Short(0)),
        (277, Short(2)),
        (278, Short(46)),
        (279, Short(0)),
        (280, Short(0)),
        (281, Bit(false)),
        (282, Bit(false)),
        (283, Short(1)),
        (284, Short(0)),
        (285, Short(0)),
        (286, Short(0)),
        (288, Bit(false)),
        (289, Short(3)),
        (371, Short(-2)),
        (372, Short(-2)),
    ]
};

/// バージョン `release` の DIMSTYLE のフィールドの並び。コードが 0 のフィールドは、DXF に書かれない
pub(in crate::dwg) fn dim_fields(release: Release) -> Vec<(i16, Dim)> {
    // R2007 からは、ヘッダーの変数と同じく、DIMTM（48）の後に DIMFXL・DIMJOGANG・DIMTFILL・
    // DIMTFILLCLR、DIMAZIN（79）の後に DIMARCSYM、DIMATFIT（289）の後に DIMFXLON が入り、
    // R2010 からはさらに DIMTXTDIRECTION・DIMALTMZF・DIMALTMZS・DIMMZF・DIMMZS が入る
    let added: &[(i16, &[(i16, Dim)])] = match release {
        Release::R2007 => &[
            (
                48,
                &[
                    (49, Dim::Real(1.0)),
                    (50, Dim::Real(std::f64::consts::FRAC_PI_4)),
                    (69, Dim::Short(0)),
                    (70, Dim::Color(0)),
                ],
            ),
            (79, &[(90, Dim::Short(0))]),
            (289, &[(290, Dim::Bit(false))]),
        ],
        _ if release >= Release::R2010 => &[
            (
                48,
                &[
                    (49, Dim::Real(1.0)),
                    (50, Dim::Real(std::f64::consts::FRAC_PI_4)),
                    (69, Dim::Short(0)),
                    (70, Dim::Color(0)),
                ],
            ),
            (79, &[(90, Dim::Short(0))]),
            (
                289,
                &[
                    (290, Dim::Bit(false)),
                    (294, Dim::Bit(false)),
                    (0, Dim::Real(0.0)),
                    (0, Dim::Text),
                    (0, Dim::Real(0.0)),
                    (0, Dim::Text),
                ],
            ),
        ],
        _ => &[],
    };
    DIM_FIELDS
        .iter()
        .flat_map(|field| {
            let extra = added
                .iter()
                .find(|(after, _)| *after == field.0)
                .map_or(&[][..], |(_, extra)| *extra);
            std::iter::once(field).chain(extra)
        })
        .copied()
        .collect()
}

/// DIMSTYLE のフィールド。DXF と同じく、既定の値と同じフィールドは置かない
pub(super) fn dimstyle(bits: &mut Bits) -> Option<Vec<Value>> {
    let mut values = Vec::new();
    for (code, field) in dim_fields(bits.release()) {
        // コードが 0 のフィールドは、DXF に書かれないので読み飛ばす
        if code == 0 {
            match field {
                Dim::Text => bits.text().map(drop)?,
                _ => bits.bd().map(drop)?,
            }
            continue;
        }
        match field {
            Dim::Text => {
                let text = bits.text()?;
                if !text.is_empty() {
                    values.push(text_value(code, &text));
                }
            }
            Dim::Real(default) => {
                let value = bits.bd()?;
                if value != default {
                    values.push(real(code, value));
                }
            }
            Dim::Bit(default) => {
                let value = bits.bit()?;
                if value != default {
                    values.push(int(code, i64::from(value)));
                }
            }
            Dim::Short(default) => {
                let value = bits.bs()?;
                if value != default {
                    values.push(int(code, i64::from(value)));
                }
            }
            Dim::Color(default) => {
                let value = bits.cmc()?.index;
                if value != default {
                    values.push(int(code, i64::from(value)));
                }
            }
        }
    }
    Some(values)
}

/// DWG の線の太さの番号を、DXF の値（1/100 mm。-1 は ByLayer、-2 は ByBlock、-3 は既定）に直す
pub(super) fn lineweight(index: u16) -> Option<i64> {
    const WIDTHS: [i64; 24] = [
        0, 5, 9, 13, 15, 18, 20, 25, 30, 35, 40, 50, 53, 60, 70, 80, 90, 100, 106, 120, 140, 158,
        200, 211,
    ];
    match index {
        29 => Some(-1),
        30 => Some(-2),
        31 => Some(-3),
        _ => WIDTHS.get(usize::from(index)).copied(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dwg::bits::BitWriter;

    /// R2004 の LAYER の、色までのフィールド。名前は A、フラグは `flags`、色は番号 `index` と真の色 `rgb`（決め方の
    /// 0xC2）。並びは [`table_record`] の読む順
    fn layer_fields(flags: i16, index: i16, rgb: u32) -> Vec<u8> {
        let mut w = BitWriter::default();
        w.text(b"A").unwrap();
        // 参照されるフラグ・外部参照の番号（R2010 より前）・外部参照に依るフラグ
        w.bit(false);
        w.bs(0);
        w.bit(false);
        w.bs(flags);
        w.bs(index);
        w.bl(i32::from_ne_bytes(((0xC2_u32 << 24) | rgb).to_ne_bytes()));
        w.rc(0);
        w.into_bytes()
    }

    fn color_of_layer(flags: i16, index: i16) -> Vec<(i16, i64)> {
        let src = layer_fields(flags, index, 0x12_34_56);
        let values = table_record(
            &mut Bits::with_release(&src, Release::R2004),
            LAYER,
            &mut Vec::new(),
        )
        .unwrap();
        values
            .iter()
            .filter(|value| matches!(value.code, 62 | 420))
            .map(|value| match value.data {
                Data::Int(int) => (value.code, int),
                _ => (value.code, i64::MIN),
            })
            .collect()
    }

    /// 真の色の画層は、番号が 0（AutoCAD が書く）でも、表示しないなら 62 に -7 を置く。表示するなら 62 を置かず、
    /// 描く色を真の色のままにする。番号を持つ画層（zumen が書いた画層）は、その番号を置く（#743）
    #[test]
    fn true_color_layers_keep_their_visibility() {
        // 表示しない 2・印刷 16
        assert_eq!(color_of_layer(2 | 16, 0), [(62, -7), (420, 0x12_34_56)]);
        assert_eq!(color_of_layer(16, 0), [(420, 0x12_34_56)]);
        assert_eq!(color_of_layer(2 | 16, 3), [(62, -3), (420, 0x12_34_56)]);
        assert_eq!(color_of_layer(16, 3), [(62, 3), (420, 0x12_34_56)]);
    }
}
