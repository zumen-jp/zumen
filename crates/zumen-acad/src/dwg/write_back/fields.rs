//! 図形の共通のフィールドと型ごとのフィールドの振り分けと、値を読む小さな関数。

use super::*;

/// バージョン `release` の共通のフィールドの、色から型ごとのフィールドの前まで。`color` なら色を書き、色（62）がなければ ByLayer に
/// する。線種の尺度（48）がなければ 1、非表示（60）がなければ 0 を書く。線種と印刷スタイルのフラグと、線の太さは
/// 元のまま写す
/// 図形の共通のフィールド（色から型ごとのフィールドの前まで、`at` は始まりと終わり）を、`old` から `new` に変えて書いたもの。
/// 変わらなければ元のビットを写す。真の色（420）は R2004 から書き、DBCOLOR で色を指す図形と、DBCOLOR を指すように変える
/// 図形は扱わない
pub(super) fn common(
    data: &[u8],
    [color_at, start]: [usize; 2],
    old: &Object,
    new: &Object,
    release: Release,
    restyle: Restyle,
    slots: &StyleSlots,
) -> Option<BitWriter> {
    let only = |codes: &[i16], object: &Object| -> Vec<_> {
        object
            .values
            .iter()
            .filter(|value| codes.contains(&value.code))
            .cloned()
            .collect()
    };
    if only(&[COMMON, &[370, 420]].concat(), old) == only(&[COMMON, &[370, 420]].concat(), new)
        && restyle.ltype.is_none()
    {
        let mut common = BitWriter::default();
        common.copy(data, color_at..start)?;
        return Some(common);
    }
    let color = only(&[62, 420], old) != only(&[62, 420], new);
    if color && (slots.color_handle || (release == Release::R2000 && !only(&[420], new).is_empty()))
    {
        return None;
    }
    let weight = only(&[370], old) != only(&[370], new);
    common_fields(
        data,
        color_at,
        start,
        new,
        release,
        Changed {
            color,
            weight,
            ltype: restyle.ltype.map(|(flags, _)| flags),
        },
    )
}

/// 共通のフィールドのうち、書き直すもの
#[derive(Clone, Copy, Default)]
pub(super) struct Changed {
    /// 色（62・420）
    pub(super) color: bool,
    /// 線の太さ（370）
    pub(super) weight: bool,
    /// 線種のフラグ
    pub(super) ltype: Option<u8>,
}

pub(super) fn common_fields(
    data: &[u8],
    color_at: usize,
    start: usize,
    object: &Object,
    release: Release,
    changed: Changed,
) -> Option<BitWriter> {
    let mut bits = Bits::with_release(data, release);
    bits.seek(color_at);
    let flags = bits.bs()? as u16;
    let index_end = bits.pos();
    bits.seek(color_at);
    bits.enc()?;
    let scale_at = bits.pos();
    bits.bd()?;
    let flags_at = bits.pos();
    // 線種と印刷スタイルのフラグ。R2010 からは、材質と影のフラグと、表示スタイルを持つかの 3 つの B が続く
    bits.seek(flags_at + 4);
    if release >= Release::R2010 {
        bits.bb()?;
        bits.rc()?;
        for _ in 0..3 {
            bits.bit()?;
        }
    }
    let invisible_at = bits.pos();
    bits.bs()?;
    let lineweight_at = bits.pos();
    let mut fields = BitWriter::default();
    if !changed.color {
        fields.copy(data, color_at..scale_at)?;
    } else if release == Release::R2000 {
        fields.bs(small(object, 62)?.unwrap_or(256));
    } else {
        // R2004 からの色（ODS 5.4.1 の 2.11）。DBCOLOR を指す色（0x4000）は扱わない。真の色（420）は 0x8000 を立て、
        // 0xC2 を上位のバイトにした RGB の値（BL）を続ける。番号は、フラグの下の 12 ビットに収まらなければならない。
        // 透明度（0x2000 と BL）は元のまま写す
        if flags & 0x4000 != 0 {
            return None;
        }
        let mut old = Bits::with_release(data, release);
        old.seek(index_end);
        if flags & 0x8000 != 0 {
            old.bl()?;
        }
        let rgb_end = old.pos();
        match integer(object, 420)? {
            Some(rgb) => {
                let rgb = u32::try_from(rgb).ok().filter(|rgb| *rgb <= 0x00FF_FFFF)?;
                fields.bs(i16::from_ne_bytes(
                    (0x8000 | (flags & 0x2000)).to_ne_bytes(),
                ));
                fields.bl(i32::from_ne_bytes(((0xC2 << 24) | rgb).to_ne_bytes()));
            }
            None => {
                let index = small(object, 62)?.unwrap_or(256);
                if !(0..=0x0FFF).contains(&index) {
                    return None;
                }
                fields.bs(index | (flags & 0x2000) as i16);
            }
        }
        fields.copy(data, rgb_end..scale_at)?;
    }
    fields.bd(real(object, 48)?.unwrap_or(1.0));
    match changed.ltype {
        Some(ltype) => {
            fields.bb(ltype);
            fields.copy(data, flags_at + 2..invisible_at)?;
        }
        None => fields.copy(data, flags_at..invisible_at)?,
    }
    fields.bs(small(object, 60)?.unwrap_or(0));
    if changed.weight {
        // 線の太さ（RC）。値がなければ ByLayer
        fields.rc(u8::try_from(lineweight_index(integer(object, 370)?.unwrap_or(-1))?).ok()?);
        fields.copy(data, lineweight_at + 8..start)?;
    } else {
        fields.copy(data, lineweight_at..start)?;
    }
    Some(fields)
}

/// 型ごとのフィールド。`grid` は MINSERT か。表にないフィールドは、元のデータ `data` の `start` からの型ごとのフィールドから写す。
/// 符号化できない種類なら `None`
pub(super) fn own_fields(
    mut fields: BitWriter,
    old: &Object,
    new: &Object,
    type_code: u16,
    data: &[u8],
    start: usize,
) -> Option<BitWriter> {
    match new.kind.as_slice() {
        b"LINE" => line(&mut fields, new)?,
        b"CIRCLE" | b"ARC" => {
            for code in [10, 20, 30, 40] {
                fields.bd(real(new, code)??);
            }
            thickness_and_extrusion(&mut fields, new)?;
            if new.kind == b"ARC" {
                // DXF は度、DWG はラジアンで書く
                for code in [50, 51] {
                    fields.bd(real(new, code)??.to_radians());
                }
            }
        }
        b"POINT" => {
            point3(&mut fields, new, 10)?;
            thickness_and_extrusion(&mut fields, new)?;
            // X 軸の角度は、なければ 0
            fields.bd(real(new, 50)?.unwrap_or(0.0).to_radians());
        }
        b"ELLIPSE" => {
            point3(&mut fields, new, 10)?;
            point3(&mut fields, new, 11)?;
            // 押し出しの向きは、なければ (0, 0, 1)
            match [210, 220, 230].map(|code| real(new, code)) {
                [Some(None), Some(None), Some(None)] => [0.0, 0.0, 1.0].map(|v| fields.bd(v)),
                [Some(Some(x)), Some(Some(y)), Some(Some(z))] => [x, y, z].map(|v| fields.bd(v)),
                _ => return None,
            };
            for code in [40, 41, 42] {
                fields.bd(real(new, code)??);
            }
        }
        b"RAY" | b"XLINE" => {
            point3(&mut fields, new, 10)?;
            point3(&mut fields, new, 11)?;
        }
        b"SOLID" | b"TRACE" => solid(&mut fields, new)?,
        b"3DFACE" => face3d(&mut fields, new)?,
        b"TEXT" => text(&mut fields, new, 73)?,
        b"ATTRIB" | b"ATTDEF" => attribute(&mut fields, new)?,
        b"MTEXT" => mtext(&mut fields, new, data, start)?,
        b"LWPOLYLINE" => lwpolyline(&mut fields, new, data, start)?,
        b"SPLINE" => spline(&mut fields, new, data, start, Release::R2000)?,
        b"HATCH" => hatch(&mut fields, old, new)?,
        b"VERTEX" => vertex(&mut fields, new, type_code)?,
        b"POLYLINE" => polyline(&mut fields, new, type_code, data, start)?,
        b"DIMENSION" => dimension(
            &mut fields,
            old,
            new,
            type_code,
            data,
            start,
            Release::R2000,
        )?,
        b"INSERT" => {
            // 回転が変わらなければ、元のラジアンを書く。度から直すと、元のビットと同じになるとは限らない
            // （決定 0101）
            let kept = (real(old, 50) == real(new, 50))
                .then(|| insert_rotation(data, start))
                .flatten();
            insert(&mut fields, new, type_code == kinds::MINSERT, kept)?
        }
        b"TOLERANCE" => {
            let end = entity_fields(data, Release::R2000)?[3];
            tolerance(&mut fields, old, new, data, [start, end], Release::R2000)?;
        }
        b"LEADER" => {
            let end = match data.is_empty() {
                true => 0,
                false => entity_fields(data, Release::R2000)?[3],
            };
            leader(&mut fields, new, data, [start, end], Release::R2000)?;
        }
        b"MLINE" => mline(&mut fields, new)?,
        b"VIEWPORT" => {
            let end = entity_fields(data, Release::R2000)?[3];
            viewport(&mut fields, old, new, data, [start, end], Release::R2000)?;
        }
        b"SHAPE" => {
            let end = entity_fields(data, Release::R2000)?[3];
            shape(&mut fields, new, data, [start, end], Release::R2000)?;
        }
        // SEQEND は型ごとのフィールドを持たない
        b"SEQEND" | b"ENDBLK" => {}
        b"BLOCK" => string(&mut fields, new, 2)?,
        _ => return None,
    }
    Some(fields)
}

/// R2004 で後ろにフィールドが増えた種類（属性を持つ INSERT の、持つオブジェクトの数（BL）と、MTEXT の背景のフィールド）の
/// 型ごとのフィールド。R2000 の形で符号化し、増えたフィールドは元のビットを写す。元の値を R2000 の形で符号化したフィールドが、元の
/// ビットの初めと同じでなければ `None`（決定 0073）
pub(super) fn r2004_own_fields(
    writer: fn() -> BitWriter,
    old: &Object,
    new: &Object,
    type_code: u16,
    data: &[u8],
    start: usize,
    handles_at: usize,
) -> Option<BitWriter> {
    let mut fields = own_fields(writer(), old, new, type_code, data, start)?;
    let same = own_fields(writer(), old, old, type_code, data, start)?;
    let end = start
        .checked_add(same.len())
        .filter(|&end| end <= handles_at)?;
    let mut found = BitWriter::default();
    found.copy(data, start..end)?;
    (found.into_bytes() == same.into_bytes()).then_some(())?;
    fields.copy(data, end..handles_at)?;
    Some(fields)
}

/// 表の値を、先頭から 1 つずつ読む
pub(super) struct Cursor<'a>(pub(super) std::iter::Peekable<std::vec::IntoIter<&'a Value>>);

impl<'a> Cursor<'a> {
    /// 次の値が `code` なら、その値
    pub(super) fn next(&mut self, code: i16) -> Option<&'a Data> {
        self.0
            .next_if(|value| value.code == code)
            .map(|value| &value.data)
    }

    pub(super) fn next_is(&mut self, code: i16) -> bool {
        self.0.peek().is_some_and(|value| value.code == code)
    }

    pub(super) fn real(&mut self, code: i16) -> Option<f64> {
        match self.next(code)? {
            Data::Real(value) => Some(*value),
            _ => None,
        }
    }

    /// BL に収まる整数
    pub(super) fn int(&mut self, code: i16) -> Option<i32> {
        match self.next(code)? {
            Data::Int(value) => i32::try_from(*value).ok(),
            _ => None,
        }
    }

    /// 0 か 1 の整数
    pub(super) fn bit(&mut self, code: i16) -> Option<bool> {
        match self.int(code)? {
            0 => Some(false),
            1 => Some(true),
            _ => None,
        }
    }
}

/// `code` の文字列を、DXF の `^` の書き方を戻し、終わりの NUL を付けて T で書く
pub(super) fn string(fields: &mut BitWriter, object: &Object, code: i16) -> Option<()> {
    let mut bytes = uncaret(text_of(object, code)?)?;
    bytes.push(0);
    fields.text(&bytes)
}

/// 3BD の点（`code`・`code + 10`・`code + 20`）。どれもなければ `default` を書く
pub(super) fn point3_or(
    fields: &mut BitWriter,
    object: &Object,
    code: i16,
    default: [f64; 3],
) -> Option<()> {
    match [code, code + 10, code + 20].map(|code| real(object, code)) {
        [Some(None), Some(None), Some(None)] => default.map(|v| fields.bd(v)),
        [Some(Some(x)), Some(Some(y)), Some(Some(z))] => [x, y, z].map(|v| fields.bd(v)),
        _ => return None,
    };
    Some(())
}

/// `code` の文字列の値。ちょうど 1 度なければ `None`
pub(super) fn text_of(object: &Object, code: i16) -> Option<&[u8]> {
    let mut found = object.values.iter().filter(|value| value.code == code);
    match (found.next().map(|value| &value.data), found.next()) {
        (Some(Data::Text(text)), None) => Some(text),
        _ => None,
    }
}

/// 3BD の点（`code`・`code + 10`・`code + 20`）
pub(super) fn point3(fields: &mut BitWriter, object: &Object, code: i16) -> Option<()> {
    for code in [code, code + 10, code + 20] {
        fields.bd(real(object, code)??);
    }
    Some(())
}

/// BT の厚さ（39）と BE の押し出しの向き（210・220・230）。値がなければ既定の値のフラグを書く
pub(super) fn thickness_and_extrusion(fields: &mut BitWriter, object: &Object) -> Option<()> {
    thickness(fields, object)?;
    extrusion(fields, object)
}

/// BT の厚さ（39）
pub(super) fn thickness(fields: &mut BitWriter, object: &Object) -> Option<()> {
    match real(object, 39)? {
        None => fields.bit(true),
        Some(thickness) => {
            fields.bit(false);
            fields.bd(thickness);
        }
    }
    Some(())
}

/// BE の押し出しの向き（210・220・230）
pub(super) fn extrusion(fields: &mut BitWriter, object: &Object) -> Option<()> {
    match [210, 220, 230].map(|code| real(object, code)) {
        [Some(None), Some(None), Some(None)] => fields.bit(true),
        [Some(Some(x)), Some(Some(y)), Some(Some(z))] => {
            fields.bit(false);
            fields.bd(x);
            fields.bd(y);
            fields.bd(z);
        }
        _ => return None,
    }
    Some(())
}

/// `code` の値。ないなら `Some(None)`、2 度あるか実数でなければ `None`
pub(super) fn real(object: &Object, code: i16) -> Option<Option<f64>> {
    let mut found = object.values.iter().filter(|value| value.code == code);
    let value = match found.next() {
        None => return Some(None),
        Some(value) => value,
    };
    match (&value.data, found.next()) {
        (Data::Real(value), None) => Some(Some(*value)),
        _ => None,
    }
}

/// `code` の整数の値。ないなら `Some(None)`、2 度あるか BS に収まる整数でなければ `None`
pub(super) fn small(object: &Object, code: i16) -> Option<Option<i16>> {
    let mut found = object.values.iter().filter(|value| value.code == code);
    let value = match found.next() {
        None => return Some(None),
        Some(value) => value,
    };
    match (&value.data, found.next()) {
        (Data::Int(value), None) => i16::try_from(*value).ok().map(Some),
        _ => None,
    }
}

/// `code` の整数の値。ないなら `Some(None)`、2 度あるか整数でなければ `None`
pub(super) fn integer(object: &Object, code: i16) -> Option<Option<i64>> {
    let mut found = object.values.iter().filter(|value| value.code == code);
    let value = match found.next() {
        None => return Some(None),
        Some(value) => value,
    };
    match (&value.data, found.next()) {
        (Data::Int(value), None) => Some(Some(*value)),
        _ => None,
    }
}
