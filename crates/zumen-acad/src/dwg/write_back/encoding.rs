//! 図形・表のレコード・図形でないオブジェクトを、リリースごとの並びで符号化し直す。

use super::*;

/// `old` から `new` に値を変えた、バージョン `release` の図形のデータ `data` を、符号化し直す。共通のフィールドと型ごとのフィールドの
/// ほかの値が変わったか、符号化できない値なら `None`。R2004 は、R2000 と型ごとのフィールドが同じ種類だけを
/// 扱う（決定 0072）
pub(super) fn encode(
    data: &[u8],
    old: &Object,
    new: &Object,
    release: Release,
    restyle: Restyle,
) -> Option<Vec<u8>> {
    // MINSERT も、表では INSERT とする。VERTEX と POLYLINE も、型でフィールドが分かれる
    let type_code = u16::try_from(Bits::with_release(data, Release::R2000).bs()?).ok()?;
    let own = own_codes(&new.kind, type_code);
    if release == Release::R2004 && !R2004_KINDS.contains(&new.kind.as_slice()) {
        return None;
    }
    let only = |codes: &[i16], object: &Object, keep: bool| -> Vec<_> {
        object
            .values
            .iter()
            .filter(|value| codes.contains(&value.code) == keep)
            .cloned()
            .collect()
    };
    let rest = |object| only(&[COMMON, STYLED, own].concat(), object, false);
    if rest(old) != rest(new) {
        return None;
    }
    let [size_at, color_at, start, handles_at] = entity_fields(data, release)?;
    // 画層と線種を変えなければ、ハンドルのストリームを読まない（DBCOLOR の色は、共通のデータのフラグでも断る）
    let slots = match restyle.is_empty() {
        true => StyleSlots::default(),
        false => style_handles(data, release, None)?,
    };
    let common = common(data, [color_at, start], old, new, release, restyle, &slots)?;
    let fields = if only(own, old, true) == only(own, new, true) {
        let mut fields = BitWriter::default();
        fields.copy(data, start..handles_at)?;
        fields
    } else if release == Release::R2004 && new.kind == b"HATCH" {
        // R2004 の初めのグラデーションのフィールド（表では 450〜470）は、元のビットを写す（決定 0074）
        let mut bits = Bits::with_release(data, release);
        bits.seek(start);
        hatch_gradient(&mut bits)?;
        let mut fields = BitWriter::default();
        fields.copy(data, start..bits.pos())?;
        hatch(&mut fields, old, new)?;
        fields
    } else if release == Release::R2004 && type_code == kinds::MINSERT {
        r2004_minsert_fields(
            BitWriter::default,
            old,
            new,
            data,
            [start, handles_at],
            release,
        )?
    } else if release == Release::R2004 && matches!(new.kind.as_slice(), b"INSERT" | b"MTEXT") {
        r2004_own_fields(
            BitWriter::default,
            old,
            new,
            type_code,
            data,
            start,
            handles_at,
        )?
    } else {
        own_fields(BitWriter::default(), old, new, type_code, data, start)?
    };
    let mut out = BitWriter::default();
    out.copy(data, 0..size_at)?;
    out.rl(i32::try_from(color_at + common.len() + fields.len()).ok()?);
    out.copy(data, size_at + 32..color_at)?;
    for part in [common, fields] {
        let len = part.len();
        out.copy(&part.into_bytes(), 0..len)?;
    }
    // ハンドルのストリームの後ろの、バイトの終わりまでの埋めのビットも写す
    let handles = restyle.handles(data, handles_at, &slots)?;
    let len = handles.len();
    out.copy(&handles.into_bytes(), 0..len)?;
    Some(out.into_bytes())
}

/// 種類 `kind`（型 `type_code`）の図形の、型ごとのフィールドの組の符号。扱わない種類は空
pub(super) fn own_codes(kind: &[u8], type_code: u16) -> &'static [i16] {
    match kind {
        b"LINE" => LINE,
        b"CIRCLE" => CIRCLE,
        b"ARC" => ARC,
        b"POINT" => POINT,
        b"ELLIPSE" => ELLIPSE,
        b"RAY" | b"XLINE" => RAY,
        b"SOLID" | b"TRACE" => SOLID,
        b"3DFACE" => FACE3D,
        b"TEXT" => TEXT,
        b"ATTRIB" => ATTRIB,
        b"ATTDEF" => ATTDEF,
        b"MTEXT" => MTEXT,
        b"LWPOLYLINE" => LWPOLYLINE,
        b"SPLINE" => SPLINE,
        b"HATCH" => HATCH,
        b"VERTEX" => match type_code {
            kinds::VERTEX_2D => VERTEX_2D,
            kinds::VERTEX_3D..=kinds::VERTEX_PFACE => VERTEX_3D,
            kinds::VERTEX_PFACE_FACE => FACE_RECORD,
            _ => &[],
        },
        b"POLYLINE" => match type_code {
            kinds::POLYLINE_2D => POLYLINE_2D,
            kinds::POLYLINE_3D => POLYLINE_3D,
            kinds::POLYLINE_PFACE => POLYFACE,
            kinds::POLYLINE_MESH => MESH,
            _ => &[],
        },
        b"DIMENSION" => DIMENSION_ALL,
        b"INSERT" if type_code == kinds::MINSERT => MINSERT,
        b"INSERT" => INSERT,
        b"TOLERANCE" => TOLERANCE,
        b"LEADER" => LEADER,
        b"MLINE" => MLINE,
        b"VIEWPORT" => VIEWPORT,
        b"SHAPE" => SHAPE,
        _ => &[],
    }
}

/// R2010 から後の図形を符号化し直したデータ（決定 0085・0086）。`data` はハンドルのストリームの大きさ（MC）の後の
/// データ、`handles_at` はハンドルのストリームの始まり。型ごとのフィールドを変えたときは、主のデータの後の文字列のストリームも
/// 書き直す。ハンドルのストリームは写し、MC を付けて返す
pub(super) fn encode_r2010(
    data: &[u8],
    handles_at: usize,
    old: &Object,
    new: &Object,
    release: Release,
    restyle: Restyle,
) -> Option<Vec<u8>> {
    let type_code = u16::try_from(Bits::with_release(data, release).bot()?).ok()?;
    let own = match new.kind.as_slice() {
        b"MTEXT" => MTEXT_R2010,
        _ => own_codes(&new.kind, type_code),
    };
    if !R2010_KINDS.contains(&new.kind.as_slice()) {
        return None;
    }
    // R2018 からの段組みの MTEXT は、埋め込みのオブジェクトのマーカー（101）の後に段組みのフィールドを持つ。フィールドは背景のフィールドの
    // 後ろにあり、元のビットを写すので、元と同じときだけ受け付け、マーカーから後を除いて比べる
    let (old, new) = (&columns_removed(old)?, &columns_removed(new)?);
    if old.1 != new.1 {
        return None;
    }
    let (old, new) = (&old.0, &new.0);
    let only = |codes: &[i16], object: &Object, keep: bool| -> Vec<_> {
        object
            .values
            .iter()
            .filter(|value| codes.contains(&value.code) == keep)
            .cloned()
            .collect()
    };
    let rest = |object| only(&[COMMON, STYLED, own].concat(), object, false);
    if rest(old) != rest(new) {
        return None;
    }
    let [color_at, start, end] = r2010_entity_fields(data, handles_at, release)?;
    let slots = match restyle.is_empty() {
        true => StyleSlots::default(),
        false => style_handles(data, release, Some(handles_at))?,
    };
    let common = common(data, [color_at, start], old, new, release, restyle, &slots)?;
    let mut out = BitWriter::default();
    out.copy(data, 0..color_at)?;
    let common_len = common.len();
    out.copy(&common.into_bytes(), 0..common_len)?;
    let handles = if only(own, old, true) == only(own, new, true) {
        out.copy(data, start..end)?;
        // 文字列のストリームを写し、ハンドルのストリームは画層と線種を書き直して写す
        out.copy(data, end..handles_at)?;
        let handles = out.len();
        let rewritten = restyle.handles(data, handles_at, &slots)?;
        let len = rewritten.len();
        out.copy(&rewritten.into_bytes(), 0..len)?;
        handles
    } else {
        // 元の値を符号化したフィールドが、主のデータの終わりまでの長さを持ち、文字列のストリームが元と同じことを確かめる。
        // 角度は度から直すので、主のデータのビットが元と同じとは限らない
        let mut same = r2010_own_fields(old, old, type_code, data, start, end, release)?;
        (start + same.len() == end).then_some(())?;
        let strings_end = r2010_strings_end(data, end, handles_at)?;
        let mut found = BitWriter::default();
        found.copy(data, end..strings_end)?;
        (found.into_bytes() == same.take_strings().into_bytes()).then_some(())?;
        let fields = r2010_own_fields(old, new, type_code, data, start, end, release)?;
        return r2010_assemble(out, fields, restyle.handles(data, handles_at, &slots)?);
    };
    with_modular_size(out, handles)
}

/// MTEXT `object` から、埋め込みのオブジェクトのマーカー（101）と後の値を除いたものと、除いた値。MTEXT でなければ
/// そのまま
pub(super) fn columns_removed(object: &Object) -> Option<(Object, Vec<Value>)> {
    let at = match object.kind.as_slice() {
        b"MTEXT" => object.values.iter().position(|value| value.code == 101),
        _ => None,
    };
    let Some(at) = at else {
        return Some((object.clone(), Vec::new()));
    };
    let (values, columns) = object.values.split_at_checked(at)?;
    let kept = Object {
        values: values.to_vec(),
        ..object.clone()
    };
    Some((kept, columns.to_vec()))
}

/// R2010 から後のオブジェクトのデータを、`out` に続けて、型ごとのフィールド `fields` の主のデータと文字列のストリーム、
/// ストリームのビットの数と有無のフラグ、元のハンドルのストリームの順に書き、MC を付けて返す
pub(super) fn r2010_finish(
    out: BitWriter,
    fields: BitWriter,
    data: &[u8],
    handles_at: usize,
) -> Option<Vec<u8>> {
    let mut handles = BitWriter::default();
    handles.copy(data, handles_at..data.len() * 8)?;
    r2010_assemble(out, fields, handles)
}

/// `out` に、主のデータと文字列のストリームを持つ `fields` と、その大きさと有無のフラグと、ハンドルのストリーム `handles` を続け、
/// ハンドルのストリームの大きさ（MC）を前に付けたデータ
pub(super) fn r2010_assemble(
    mut out: BitWriter,
    mut fields: BitWriter,
    handles: BitWriter,
) -> Option<Vec<u8>> {
    let strings = fields.take_strings();
    let fields_len = fields.len();
    out.copy(&fields.into_bytes(), 0..fields_len)?;
    let size = strings.len();
    out.copy(&strings.into_bytes(), 0..size)?;
    if size > 0 {
        // ストリームのビットの数（RS）。0x8000 に収まらなければ、上位の RS を前に置き、下位に 0x8000 を立てる
        if size >= 0x8000 {
            out.rs(i16::try_from(size >> 15).ok()?);
            out.rs(i16::from_ne_bytes(
                (u16::try_from(size & 0x7FFF).ok()? | 0x8000).to_ne_bytes(),
            ));
        } else {
            out.rs(i16::try_from(size).ok()?);
        }
    }
    out.bit(size > 0);
    let at = out.len();
    let len = handles.len();
    out.copy(&handles.into_bytes(), 0..len)?;
    with_modular_size(out, at)
}

/// `out` の前に、ビット `handles` から終わりまでのビットの数（MC）を付ける
pub(super) fn with_modular_size(out: BitWriter, handles: usize) -> Option<Vec<u8>> {
    let out = out.into_bytes();
    let mut bytes = Vec::new();
    push_modular_char(
        &mut bytes,
        i64::try_from((out.len() * 8).checked_sub(handles)?).ok()?,
        false,
    );
    bytes.extend(out);
    Some(bytes)
}

/// R2010 から後の型ごとのフィールドを、文字列のストリームを分けて符号化する。R2004 で後ろにフィールドが増えた種類は、増えたフィールドを
/// 主のデータの終わり `end` まで写す
pub(super) fn r2010_own_fields(
    old: &Object,
    new: &Object,
    type_code: u16,
    data: &[u8],
    start: usize,
    end: usize,
    release: Release,
) -> Option<BitWriter> {
    if matches!(new.kind.as_slice(), b"ATTRIB" | b"ATTDEF") {
        let mut fields = BitWriter::with_strings();
        r2010_attribute(&mut fields, Some(old), new, data, [start, end], release)?;
        Some(fields)
    } else if new.kind == b"VIEWPORT" {
        let mut fields = BitWriter::with_strings();
        viewport(&mut fields, old, new, data, [start, end], release)?;
        Some(fields)
    } else if new.kind == b"TOLERANCE" {
        let mut fields = BitWriter::with_strings();
        tolerance(&mut fields, old, new, data, [start, end], release)?;
        Some(fields)
    } else if new.kind == b"SHAPE" {
        let mut fields = BitWriter::with_strings();
        shape(&mut fields, new, data, [start, end], release)?;
        Some(fields)
    } else if new.kind == b"LEADER" {
        let mut fields = BitWriter::with_strings();
        leader(&mut fields, new, data, [start, end], release)?;
        Some(fields)
    } else if new.kind == b"DIMENSION" {
        let mut fields = BitWriter::with_strings();
        dimension(&mut fields, old, new, type_code, data, start, release)?;
        Some(fields)
    } else if new.kind == b"MTEXT" {
        let mut fields = BitWriter::with_strings();
        r2010_mtext(&mut fields, Some(old), new, data, start, end, release)?;
        Some(fields)
    } else if new.kind == b"VERTEX" && type_code == kinds::VERTEX_2D {
        // 2D の頂点は、膨らみの後に頂点の番号（BL）を持つ。元のビットを写し、接線の向きを後に書く
        let mut bits = Bits::with_release(data, release);
        bits.seek(start);
        bits.rc()?;
        for _ in 0..3 {
            bits.bd()?;
        }
        if bits.bd()? >= 0.0 {
            bits.bd()?;
        }
        bits.bd()?;
        let id_at = bits.pos();
        bits.bl()?;
        let id = id_at..bits.pos();
        let mut whole = BitWriter::with_strings();
        vertex(&mut whole, new, type_code)?;
        // 接線の向き（BD）の前までを写し、番号と接線の向きを続ける
        let tangent = real(new, 50)?.unwrap_or(0.0).to_radians();
        let mut last = BitWriter::default();
        last.bd(tangent);
        let head = whole.len().checked_sub(last.len())?;
        let mut fields = BitWriter::with_strings();
        fields.copy(&whole.into_bytes(), 0..head)?;
        fields.copy(data, id)?;
        fields.bd(tangent);
        Some(fields)
    } else if new.kind == b"HATCH" {
        // 初めのグラデーションのフィールドは、元のビットを写す（決定 0074）。その名前は文字列のストリームの初めにあるので、
        // ストリームの初めの TU も写す（決定 0088）
        let mut bits = Bits::with_release(data, release);
        bits.seek(start);
        hatch_gradient(&mut bits)?;
        let mut fields = BitWriter::with_strings();
        fields.copy(data, start..bits.pos())?;
        bits.seek(end);
        let units = usize::try_from(bits.bs()?).ok()?;
        fields.copy_strings(data, end..bits.pos() + units * 16)?;
        hatch(&mut fields, old, new)?;
        Some(fields)
    } else if new.kind == b"SPLINE" {
        let mut fields = BitWriter::with_strings();
        spline(&mut fields, new, data, start, release)?;
        Some(fields)
    } else if type_code == kinds::MINSERT {
        r2004_minsert_fields(
            BitWriter::with_strings,
            old,
            new,
            data,
            [start, end],
            release,
        )
    } else if new.kind == b"INSERT" {
        r2004_own_fields(
            BitWriter::with_strings,
            old,
            new,
            type_code,
            data,
            start,
            end,
        )
    } else {
        own_fields(BitWriter::with_strings(), old, new, type_code, data, start)
    }
}

/// R2010 から後の MTEXT のフィールド（決定 0087）。幅（41）の後に枠の高さ（46）を書く。範囲の大きさと背景のフィールドは
/// 元のビットを写し、R2018 からの段組みでない MTEXT は、背景の後に続く埋め込みの注釈フィールドも合わせる（#880）
pub(super) fn r2010_mtext(
    fields: &mut BitWriter,
    old: Option<&Object>,
    object: &Object,
    data: &[u8],
    start: usize,
    end: usize,
    release: Release,
) -> Option<()> {
    // 足した MTEXT は元のデータを持たない（`data` が空）。範囲の大きさを 0 にし、後の B と背景のフラグ（BL）を 0 に
    // する（決定 0093）
    let copied = match data.is_empty() {
        true => None,
        false => {
            // 文字列は文字列のストリームにあるので、主のデータの位置を求めるときは読まない
            let mut bits = Bits::with_release(data, release);
            bits.seek(start);
            for _ in 0..12 {
                bits.bd()?;
            }
            bits.bs()?;
            bits.bs()?;
            let extents_at = bits.pos();
            bits.bd()?;
            bits.bd()?;
            let extents_end = bits.pos();
            bits.bs()?;
            bits.bd()?;
            Some((extents_at..extents_end, bits.pos()))
        }
    };
    point3(fields, object, 10)?;
    for (code, default) in [(210, [0.0, 0.0, 1.0]), (11, [1.0, 0.0, 0.0])] {
        match [code, code + 10, code + 20].map(|code| real(object, code)) {
            [Some(None), Some(None), Some(None)] => default.map(|v| fields.bd(v)),
            [Some(Some(x)), Some(Some(y)), Some(Some(z))] => [x, y, z].map(|v| fields.bd(v)),
            _ => return None,
        };
    }
    for code in [41, 46, 40] {
        fields.bd(real(object, code)??);
    }
    for code in [71, 72] {
        fields.bs(small(object, code)??);
    }
    match &copied {
        Some((extents, _)) => fields.copy(data, extents.clone())?,
        None => {
            fields.bd(0.0);
            fields.bd(0.0);
        }
    }
    let mut text = Vec::new();
    for value in object.values.iter().filter(|value| value.code == 3) {
        let Data::Text(chunk) = &value.data else {
            return None;
        };
        text.extend(uncaret(chunk)?);
    }
    text.extend(uncaret(text_of(object, 1)?)?);
    fields.text(&text)?;
    fields.bs(small(object, 73)??);
    fields.bd(real(object, 44)??);
    match copied {
        Some((_, flag_at)) => r2010_mtext_tail(fields, old, object, data, [flag_at, end], release),
        None => {
            fields.bit(false);
            fields.bl(0);
            // R2018 からは、注釈的でないフラグ（B）と、バージョン（BS）・既定のフラグ（B）・揃え・X 軸・挿入点・枠の幅と高さ・
            // 範囲の幅と高さ・段の種類（BL）が続く。サンプルの図面と同じく、注釈的でなく、バージョンは 4、既定のフラグは真、
            // 段はないとし、ほかは型ごとのフィールドと同じ値を書く（決定 0098）
            if release >= Release::R2018 {
                fields.bit(true);
                fields.bs(4);
                fields.bit(true);
                fields.bl(i32::from(small(object, 71)??));
                for (code, default) in [(11, 1.0), (21, 0.0), (31, 0.0)] {
                    fields.bd(real(object, code)?.unwrap_or(default));
                }
                point3(fields, object, 10)?;
                for code in [41, 46] {
                    fields.bd(real(object, code)??);
                }
                fields.bd(0.0);
                fields.bd(0.0);
                fields.bl(0);
            }
            Some(())
        }
    }
}

/// 既存の MTEXT の、行の間隔の後のフラグ `flag_at` から主のデータの終わり `end` までを書く。R2018 からの段組みでない
/// MTEXT は、AutoCAD がここにある揃え・X 軸・挿入点・枠の幅と高さを読むので、変わったフィールドだけを書き直し、
/// 変わらないフィールドと前後のビットは元のまま写す（#880）
fn r2010_mtext_tail(
    fields: &mut BitWriter,
    old: Option<&Object>,
    object: &Object,
    data: &[u8],
    [flag_at, end]: [usize; 2],
    release: Release,
) -> Option<()> {
    if release < Release::R2018 {
        return fields.copy(data, flag_at..end);
    }
    let old = old?;
    let axis = |src: &Object| match [11, 21, 31].map(|code| real(src, code)) {
        [Some(None), Some(None), Some(None)] => Some([1.0, 0.0, 0.0]),
        [Some(Some(x)), Some(Some(y)), Some(Some(z))] => Some([x, y, z]),
        _ => None,
    };
    let point = |src: &Object| Some([real(src, 10)??, real(src, 20)??, real(src, 30)??]);
    let (old_attachment, attachment) = (small(old, 71)??, small(object, 71)??);
    let (old_axis, new_axis) = (axis(old)?, axis(object)?);
    let (old_point, new_point) = (point(old)?, point(object)?);
    let (old_width, new_width) = (real(old, 41)??, real(object, 41)??);
    let (old_height, new_height) = (real(old, 46)??, real(object, 46)??);
    if old_attachment == attachment
        && old_axis.map(f64::to_bits) == new_axis.map(f64::to_bits)
        && old_point.map(f64::to_bits) == new_point.map(f64::to_bits)
        && old_width.to_bits() == new_width.to_bits()
        && old_height.to_bits() == new_height.to_bits()
    {
        return fields.copy(data, flag_at..end);
    }
    let mut bits = Bits::with_release(data, release);
    bits.seek(flag_at);
    bits.bit()?;
    if bits.bl()? & 1 != 0 {
        bits.bd()?;
        bits.cmc()?;
        bits.bl()?;
    }
    let not_annotative = bits.bit()?;
    bits.bs()?;
    let is_default = bits.bit()?;
    if !not_annotative || !is_default {
        return None;
    }
    let attachment_at = bits.pos();
    bits.bl()?;
    let axis_at = bits.pos();
    for _ in 0..3 {
        bits.bd()?;
    }
    let point_at = bits.pos();
    for _ in 0..3 {
        bits.bd()?;
    }
    let width_at = bits.pos();
    bits.bd()?;
    let height_at = bits.pos();
    bits.bd()?;
    let extents_at = bits.pos();
    bits.bd()?;
    bits.bd()?;
    let kind = bits.bl()?;
    if kind != 0 {
        // 段組みの MTEXT（101 を持つ）は、埋め込みのオブジェクトの値を元のまま写す（決定 0098）
        return fields.copy(data, flag_at..end);
    }
    (bits.pos() == end).then_some(())?;
    fields.copy(data, flag_at..attachment_at)?;
    if old_attachment == attachment {
        fields.copy(data, attachment_at..axis_at)?;
    } else {
        fields.bl(i32::from(attachment));
    }
    if old_axis.map(f64::to_bits) == new_axis.map(f64::to_bits) {
        fields.copy(data, axis_at..point_at)?;
    } else {
        for v in new_axis {
            fields.bd(v);
        }
    }
    if old_point.map(f64::to_bits) == new_point.map(f64::to_bits) {
        fields.copy(data, point_at..width_at)?;
    } else {
        for v in new_point {
            fields.bd(v);
        }
    }
    if old_width.to_bits() == new_width.to_bits() {
        fields.copy(data, width_at..height_at)?;
    } else {
        fields.bd(new_width);
    }
    if old_height.to_bits() == new_height.to_bits() {
        fields.copy(data, height_at..extents_at)?;
    } else {
        fields.bd(new_height);
    }
    fields.copy(data, extents_at..end)
}

/// R2018 からの複数行の属性の、意味の分からない 37 ビット。サンプルの図面の属性はどれもこの値を持つ（決定 0102）
pub(super) const EMBEDDED_BITS: &[u8; 37] = b"0010101101111100000000000001000011101";

/// 複数行の属性の種類（71）の位置。属性のバージョン（280）の後の 71 で、バージョンの前の 71 は文字の向き。属性の値は、その前に
/// 並ぶ（決定 0102）
pub(super) fn multiline_at(object: &Object) -> Option<usize> {
    let version = object.values.iter().position(|value| value.code == 280)?;
    object
        .values
        .iter()
        .skip(version)
        .position(|value| value.code == 71)
        .map(|at| at + version)
}

/// R2018 からの複数行の属性の元のフィールドの、種類の後の 37 ビットの始まりと、埋め込みの MTEXT のフィールドの始まりと、
/// 段の種類の後の終わりのビットの位置。型ごとのフィールドは `start` から始まる。文字列は文字列のストリームにあるので読まない
pub(super) fn embedded_span(data: &[u8], start: usize, release: Release) -> Option<[usize; 3]> {
    let mut bits = Bits::with_release(data, release);
    bits.seek(start);
    // TEXT のフィールド（R2000）。DataFlags のビットが立つフィールドはない
    let flags = bits.rc()?;
    if flags & 0x01 == 0 {
        bits.rd()?;
    }
    bits.rd()?;
    bits.rd()?;
    if flags & 0x02 == 0 {
        bits.dd(0.0)?;
        bits.dd(0.0)?;
    }
    if !bits.bit()? {
        for _ in 0..3 {
            bits.bd()?;
        }
    }
    if !bits.bit()? {
        bits.bd()?;
    }
    for bit in [0x04, 0x08] {
        if flags & bit == 0 {
            bits.rd()?;
        }
    }
    bits.rd()?;
    if flags & 0x10 == 0 {
        bits.rd()?;
    }
    for bit in [0x20, 0x40, 0x80] {
        if flags & bit == 0 {
            bits.bs()?;
        }
    }
    // 属性のバージョンと種類
    bits.rc()?;
    (bits.rc()? != 1).then_some(())?;
    let at = bits.pos();
    let mtext_at = at.checked_add(EMBEDDED_BITS.len())?;
    bits.seek(mtext_at);
    // 埋め込みの MTEXT のフィールド。挿入点・押し出しと X 軸の向き・幅・枠の高さ・文字の高さ、揃えと向き、範囲の大きさ、
    // 行の間隔の取り方と倍率、B と背景のフィールド
    for _ in 0..12 {
        bits.bd()?;
    }
    bits.bs()?;
    bits.bs()?;
    bits.bd()?;
    bits.bd()?;
    bits.bs()?;
    bits.bd()?;
    bits.bit()?;
    if bits.bl()? & 1 != 0 {
        bits.bd()?;
        bits.cmc()?;
        bits.bl()?;
    }
    // R2018 から増えたフィールド（決定 0098）。段のあるものは、まだ扱わない
    bits.bit()?;
    bits.bs()?;
    bits.bit()?;
    bits.bl()?;
    for _ in 0..10 {
        bits.bd()?;
    }
    (bits.bl()? == 0).then_some(())?;
    Some([at, mtext_at, bits.pos()])
}

/// R2010 から後の ATTRIB・ATTDEF のフィールド。名前の前に属性のバージョン（RC、1 つ目の 280）を、フラグの後に位置を固定するか
/// （B、2 つ目の 280）を書く。ATTDEF の問いの前のバージョン（RC）は、主のデータの終わり `end` の前の元のビットを写す。
/// 型ごとのフィールドは `start` から始まり、足したものは元のデータを持たない（`data` が空）
pub(super) fn r2010_attribute(
    fields: &mut BitWriter,
    old: Option<&Object>,
    object: &Object,
    data: &[u8],
    [start, end]: [usize; 2],
    release: Release,
) -> Option<()> {
    // 複数行の属性は、属性の値の後に、種類（71）・72・埋め込みのオブジェクトのマーカー（101）と MTEXT の値を持つ
    let mut own = object.clone();
    let rest = match multiline_at(object) {
        Some(at) => own.values.split_off(at),
        None => Vec::new(),
    };
    let object = &own;
    let mut versions = object.values.iter().filter(|value| value.code == 280);
    let mut next = || match versions.next().map(|value| &value.data) {
        Some(Data::Int(value)) => u8::try_from(*value).ok(),
        _ => None,
    };
    let (version, locked) = (next()?, next()?);
    text(fields, object, 74)?;
    fields.rc(version);
    // R2018 からは、バージョンの後に属性の種類（RC）が続く。複数行の属性は、37 ビットの後に、埋め込みの MTEXT のフィールドと
    // DXF の 72 にあたる BS を持つ。37 ビットは元のものを写し、足したものはサンプルの図面と同じ値を書く。
    // MTEXT のフィールドは、MTEXT と同じく書く（決定 0098・0102）
    match rest.as_slice() {
        [] if release >= Release::R2018 => fields.rc(1),
        [] => {}
        [kind, after, marker, mtext @ ..]
            if release >= Release::R2018
                && (kind.code, after.code, marker.code) == (71, 72, 101) =>
        {
            let int = |value: &Value| match value.data {
                Data::Int(value) => Some(value),
                _ => None,
            };
            let kind = u8::try_from(int(kind)?).ok().filter(|&kind| kind != 1)?;
            fields.rc(kind);
            let embedded = Object {
                values: mtext.to_vec(),
                ..object.clone()
            };
            if data.is_empty() {
                for &bit in EMBEDDED_BITS {
                    fields.bit(bit == b'1');
                }
                r2010_mtext(fields, None, &embedded, &[], 0, 0, release)?;
            } else {
                let prev = old?;
                let old_mtext = match prev.values.get(multiline_at(prev)?..) {
                    Some([k, a, m, tail @ ..]) if (k.code, a.code, m.code) == (71, 72, 101) => {
                        Object {
                            values: tail.to_vec(),
                            ..object.clone()
                        }
                    }
                    _ => return None,
                };
                let [at, mtext_at, mtext_end] = embedded_span(data, start, release)?;
                fields.copy(data, at..mtext_at)?;
                r2010_mtext(
                    fields,
                    Some(&old_mtext),
                    &embedded,
                    data,
                    mtext_at,
                    mtext_end,
                    release,
                )?;
            }
            fields.bs(i16::try_from(int(after)?).ok()?);
        }
        _ => return None,
    }
    string(fields, object, 2)?;
    fields.bs(small(object, 73)?.unwrap_or(0));
    fields.rc(u8::try_from(small(object, 70)??).ok()?);
    fields.bit(locked != 0);
    if object.kind == b"ATTDEF" {
        fields.copy(data, end.checked_sub(8)?..end)?;
        string(fields, object, 3)?;
    }
    Some(())
}

/// 主のデータの終わり `end` から始まる文字列のストリームの終わり（ストリームのビットの数（RS）の始まり）のビットの位置。
/// ストリームがなければ `end`
pub(super) fn r2010_strings_end(data: &[u8], end: usize, handles_at: usize) -> Option<usize> {
    let mut bits = Bits::new(data);
    bits.seek(handles_at.checked_sub(1)?);
    if !bits.bit()? {
        return Some(end);
    }
    let at = handles_at.checked_sub(17)?;
    bits.seek(at);
    let low = bits.rs()?;
    if low < 0 {
        at.checked_sub(16)
    } else {
        Some(at)
    }
}

/// 表のレコードの、型ごとのフィールドでない値が変わっていなければ `Some`（決定 0043〜0046）
pub(super) fn same_record_rest(old: &Object, new: &Object) -> Option<()> {
    // DIMSTYLE は、70 と DWG の並びのフィールド
    let dimstyle_codes: Vec<i16> = std::iter::once(70)
        .chain(DIM_FIELDS.iter().map(|field| field.0))
        .collect();
    let own: &[i16] = match new.kind.as_slice() {
        // 線種（6）と真の色（420）も書き直せる（#723）
        b"LAYER" => &[LAYER, &[6, 420, 430]].concat(),
        b"STYLE" => STYLE,
        b"LTYPE" => LTYPE,
        b"VIEW" => VIEW,
        b"UCS" => UCS,
        b"VPORT" => VPORT,
        b"DIMSTYLE" => &dimstyle_codes,
        _ => return None,
    };
    // LAYER・STYLE・LTYPE・DIMSTYLE の名前は書き直せる（決定 0162〜0165。[`record_fields`]）
    let renamable = renamable(new);
    let rest = |object: &Object| -> Vec<_> {
        object
            .values
            .iter()
            .filter(|value| !own.contains(&value.code) && !(renamable && value.code == 2))
            .cloned()
            .collect()
    };
    (rest(old) == rest(new)).then_some(())
}

/// 名前を書き直せる表のレコード（LAYER・STYLE・LTYPE・DIMSTYLE。決定 0162〜0165）か
pub(super) fn renamable(object: &Object) -> bool {
    matches!(
        object.kind.as_slice(),
        b"LAYER" | b"STYLE" | b"LTYPE" | b"DIMSTYLE"
    )
}

/// 表のレコードの、ビット `start` から始まるフィールドを `fields` に書き、元のデータの型ごとのフィールドの終わりを返す。
/// 参照されるフラグ・外部参照の番号・外部参照に依るフラグは、元のビットを写す。名前も写すが、LAYER・STYLE・LTYPE・DIMSTYLE は名前を
/// 書き直せる（決定 0162〜0165）。R2010 からは、名前は文字列のストリームの初めにあり、書き直すなら呼ぶ側が写さずにおく
pub(super) fn record_fields(
    fields: &mut BitWriter,
    old: &Object,
    new: &Object,
    data: &[u8],
    start: usize,
    release: Release,
) -> Option<usize> {
    let mut bits = Bits::with_release(data, release);
    bits.seek(start);
    bits.text()?;
    let named_at = bits.pos();
    bits.bit()?;
    // R2010 からは、外部参照の番号（BS）がない
    if release < Release::R2010 {
        bits.bs()?;
    }
    let xdep = bits.bit()?;
    let own_at = bits.pos();
    if text_of(old, 2) == text_of(new, 2) {
        fields.copy(data, start..own_at)?;
    } else {
        (renamable(new) && !xdep).then_some(())?;
        string(fields, new, 2)?;
        fields.copy(data, named_at..own_at)?;
    }
    match new.kind.as_slice() {
        b"LAYER" => layer(fields, new, xdep, data, own_at, release),
        b"STYLE" => style(fields, old, new, xdep, data, own_at, release),
        b"LTYPE" => ltype(fields, old, new, xdep, data, own_at, release),
        b"VIEW" => view_record(fields, new, xdep, data, own_at, release),
        b"UCS" => ucs_record(fields, new, xdep, data, own_at, release),
        b"VPORT" => vport_record(fields, old, new, xdep, data, own_at, release),
        _ => dimstyle(fields, old, new, xdep, data, own_at, release),
    }
}

/// R2010 から後の表のレコードを符号化し直したデータ（決定 0089）。`data` はハンドルのストリームの大きさ（MC）の後の
/// データ、`handles_at` はハンドルのストリームの始まり。文字列のストリームは、初めの名前を写し、型ごとのフィールドの文字列を続ける。
/// 名前を変えた LAYER は、初めに新しい名前を書く（決定 0162）
pub(super) fn encode_record_r2010(
    data: &[u8],
    handles_at: usize,
    old: &Object,
    new: &Object,
    release: Release,
    ltype: Option<(u8, u64)>,
) -> Option<Vec<u8>> {
    same_record_rest(old, new)?;
    // UCS のレコードは、サンプルの図面になく確かめていない
    (new.kind != b"UCS").then_some(())?;
    let [start, end] = r2010_object_fields(data, handles_at, release)?;
    let encode = |object: &Object| -> Option<BitWriter> {
        let mut fields = BitWriter::with_strings();
        let mut bits = Bits::with_release(data, release);
        bits.seek(end);
        let units = usize::try_from(bits.bs()?).ok()?;
        // 名前を変えたら、`record_fields` が新しい名前をストリームの初めに書く
        if text_of(old, 2) == text_of(object, 2) {
            fields.copy_strings(data, end..bits.pos() + units * 16)?;
        }
        let own_end = record_fields(&mut fields, old, object, data, start, release)?;
        fields.copy(data, own_end..end)?;
        Some(fields)
    };
    let handles = record_handles(data, handles_at, release, Some(handles_at), ltype)?;
    r2010_checked(
        data,
        [start, end, handles_at],
        encode(old)?,
        encode(new)?,
        Some(handles),
    )
}

/// 表のレコードのハンドルのストリーム（`handles_at` から終わりまで）を写したもの。LAYER の線種を変えるなら（`ltype`）、線種の
/// ハンドルを固い参照（符号 5）で書き直す（#723）。`r2010` は、R2010 からのデータのハンドルのストリームの始まり
pub(super) fn record_handles(
    data: &[u8],
    handles_at: usize,
    release: Release,
    r2010: Option<usize>,
    ltype: Option<(u8, u64)>,
) -> Option<BitWriter> {
    let mut out = BitWriter::default();
    match ltype {
        None => out.copy(data, handles_at..data.len() * 8)?,
        Some((_, handle)) => {
            let slot = layer_line_type(data, release, r2010)?;
            out.copy(data, handles_at..slot.start)?;
            out.handle(5, handle);
            out.copy(data, slot.end..data.len() * 8)?;
        }
    }
    Some(out)
}

/// 型ごとのフィールドがビット `start` から `end` まで、ハンドルのストリームが `handles_at` からの R2010 のオブジェクトに、
/// フィールド `fields` を置いたデータ。元の値を符号化した `same` が、元の主のデータの長さと文字列のストリームを持つことを確かめる
pub(super) fn r2010_checked(
    data: &[u8],
    [start, end, handles_at]: [usize; 3],
    mut same: BitWriter,
    fields: BitWriter,
    handles: Option<BitWriter>,
) -> Option<Vec<u8>> {
    (start + same.len() == end).then_some(())?;
    let strings_end = r2010_strings_end(data, end, handles_at)?;
    let mut found = BitWriter::default();
    found.copy(data, end..strings_end)?;
    (found.into_bytes() == same.take_strings().into_bytes()).then_some(())?;
    let mut out = BitWriter::default();
    out.copy(data, 0..start)?;
    match handles {
        Some(handles) => r2010_assemble(out, fields, handles),
        None => r2010_finish(out, fields, data, handles_at),
    }
}

/// 表のレコードの型ごとのフィールドを符号化し直したデータ（決定 0043〜0046）。LAYER・STYLE・LTYPE・VIEW・UCS・VPORT・DIMSTYLE だけを扱う。名前（2）などの
/// ほかの値が変わったものは `None`
pub(super) fn encode_record(
    data: &[u8],
    old: &Object,
    new: &Object,
    release: Release,
    ltype: Option<(u8, u64)>,
) -> Option<Vec<u8>> {
    same_record_rest(old, new)?;
    let [size_at, start, handles_at] = object_fields(data, release)?;
    let mut fields = BitWriter::default();
    let end = record_fields(&mut fields, old, new, data, start, release)?;
    fields.copy(data, end..handles_at)?;
    let mut out = BitWriter::default();
    out.copy(data, 0..size_at)?;
    out.rl(i32::try_from(start + fields.len()).ok()?);
    out.copy(data, size_at + 32..start)?;
    let len = fields.len();
    out.copy(&fields.into_bytes(), 0..len)?;
    let handles = record_handles(data, handles_at, release, None, ltype)?;
    let len = handles.len();
    out.copy(&handles.into_bytes(), 0..len)?;
    Some(out.into_bytes())
}

/// OBJECTS のオブジェクトの型ごとのフィールドを符号化し直したデータ（決定 0047〜0050）。GROUP・DICTIONARYVAR・
/// DICTIONARY・ACDBDICTIONARYWDFLT・MLINESTYLE・LAYOUT・XRECORD だけを扱う。ハンドルの並びから作る値などのほかの値が
/// 変わったものは `None`
pub(super) fn encode_object(
    data: &[u8],
    old: &Object,
    new: &Object,
    release: Release,
) -> Option<Vec<u8>> {
    let [size_at, start, handles_at] = object_fields(data, release)?;
    let mut fields = BitWriter::default();
    let end = object_own_fields(&mut fields, old, new, data, start, release)?;
    fields.copy(data, end..handles_at)?;
    let mut out = BitWriter::default();
    out.copy(data, 0..size_at)?;
    out.rl(i32::try_from(start + fields.len()).ok()?);
    out.copy(data, size_at + 32..start)?;
    let len = fields.len();
    out.copy(&fields.into_bytes(), 0..len)?;
    out.copy(data, handles_at..data.len() * 8)?;
    Some(out.into_bytes())
}

/// R2010 から後の OBJECTS のオブジェクトを符号化し直したデータ（決定 0090）。`data` はハンドルのストリームの
/// 大きさ（MC）の後のデータ、`handles_at` はハンドルのストリームの始まり。文字列のストリームは、型ごとのフィールドの文字列だけを持つ。
/// XRECORD の文字列の値は、文字列のストリームでなくデータの中に UTF-16LE で持つ（決定 0148）
pub(super) fn encode_object_r2010(
    data: &[u8],
    handles_at: usize,
    old: &Object,
    new: &Object,
    release: Release,
) -> Option<Vec<u8>> {
    let [start, end] = r2010_object_fields(data, handles_at, release)?;
    let encode = |object: &Object| -> Option<BitWriter> {
        let mut fields = BitWriter::with_strings();
        let own_end = object_own_fields(&mut fields, old, object, data, start, release)?;
        fields.copy(data, own_end..end)?;
        Some(fields)
    };
    r2010_checked(
        data,
        [start, end, handles_at],
        encode(old)?,
        encode(new)?,
        None,
    )
}

/// OBJECTS のオブジェクトの型ごとのフィールドを、ビット `start` から読みながら `fields` に書き、元のフィールドの終わりの位置を返す
pub(super) fn object_own_fields(
    fields: &mut BitWriter,
    old: &Object,
    new: &Object,
    data: &[u8],
    start: usize,
    release: Release,
) -> Option<usize> {
    let own: &[i16] = match new.kind.as_slice() {
        b"GROUP" => &[300, 70, 71],
        b"DICTIONARYVAR" => &[280, 1],
        b"DICTIONARY" | b"ACDBDICTIONARYWDFLT" => &[280, 281, 3],
        b"MLINESTYLE" => &[2, 70, 3, 62, 51, 52, 71, 49],
        b"LAYOUT" => return layout(fields, old, new, data, start, release),
        b"XRECORD" => return xrecord(fields, old, new, data, start, release),
        _ => return None,
    };
    // 辞書の項目の数と、マルチラインスタイルの線の数と名前は変えない
    let codes = |object: &Object| -> Vec<i16> { object.values.iter().map(|v| v.code).collect() };
    if new.kind != b"GROUP" && new.kind != b"DICTIONARYVAR" && codes(old) != codes(new) {
        return None;
    }
    if new.kind == b"MLINESTYLE" && text_of(old, 2) != text_of(new, 2) {
        return None;
    }
    let rest = |object: &Object| -> Vec<_> {
        object
            .values
            .iter()
            .filter(|value| !own.contains(&value.code))
            .cloned()
            .collect()
    };
    if rest(old) != rest(new) {
        return None;
    }
    let mut bits = Bits::with_release(data, release);
    bits.seek(start);
    if new.kind == b"GROUP" {
        // 説明・名前のないフラグ・選べるフラグ。図形の数は元のまま写す
        bits.text()?;
        bits.bs()?;
        bits.bs()?;
        string(fields, new, 300)?;
        for code in [70, 71] {
            fields.bs(i16::from(flag(new, code)?));
        }
    } else if new.kind == b"DICTIONARYVAR" {
        // 型と値
        bits.rc()?;
        bits.text()?;
        fields.rc(u8::try_from(small(new, 280)??).ok()?);
        string(fields, new, 1)?;
    } else if new.kind == b"MLINESTYLE" {
        mlinestyle(fields, &mut bits, new, data, release)?;
    } else {
        dictionary(fields, &mut bits, new, data)?;
    }
    Some(bits.pos())
}
