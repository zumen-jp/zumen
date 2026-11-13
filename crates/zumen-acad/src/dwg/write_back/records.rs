//! 表のレコードと図形でないオブジェクトの、種類ごとのフィールドの符号化。

use super::*;

/// LAYER のフィールド（決定 0043）。DXF の 70 の凍結 1・新しいビューポートで凍結 2・ロック 4 と、負の色（62）の
/// 表示しないフラグ、印刷しないフラグ（290）、線の太さ（370）から、DWG のフラグ（BS）を作る。フラグのほかのビットは元のまま
/// 写す。外部参照に依るフラグ（70 の 16）は、元のものと同じでなければ受け付けない。元のフィールドの終わりの位置を返す
pub(super) fn layer(
    fields: &mut BitWriter,
    object: &Object,
    xdep: bool,
    data: &[u8],
    own_at: usize,
    release: Release,
) -> Option<usize> {
    let mut bits = Bits::with_release(data, release);
    bits.seek(own_at);
    let old = bits.bs()?;
    let color_at = bits.pos();
    let old_color = bits.cmc()?;
    let (flags, color) = layer_flags(object, xdep, old)?;
    fields.bs(flags);
    match integer(object, 420)? {
        // 真の色（R2004 から、#723）。番号（BS）も書き、読むときに 62 に戻す。番号を持たない（AutoCAD の）真の色の画層は、
        // 表示しないときは 62 を -7 に読み、図面モデルからは 7 で書くので、7 のままなら元の CMC を写す（#743）
        Some(rgb)
            if old_color.rgb.map(i64::from) == Some(rgb)
                && (old_color.index == color
                    || (!(1..=255).contains(&old_color.index) && color == 7)) =>
        {
            fields.copy(data, color_at..bits.pos())?;
        }
        // 色見本帳の色の名前（430）は書かないので、名前を持つ画層の色は変えない
        Some(rgb) => {
            (release >= Release::R2004 && !object.values.iter().any(|value| value.code == 430))
                .then_some(())?;
            let rgb = u32::try_from(rgb).ok().filter(|rgb| *rgb <= 0x00FF_FFFF)?;
            fields.bs(color);
            fields.bl(i32::from_ne_bytes(((0xC2 << 24) | rgb).to_ne_bytes()));
            fields.rc(0);
        }
        None => {
            // 組み直す CMC は名前を持たないので、色見本帳の色の名前（430）を持つ画層の色は変えない
            let copied = old_color.rgb.is_none() && old_color.index == color;
            (copied || !object.values.iter().any(|value| value.code == 430)).then_some(())?;
            same_cmc(
                fields,
                data,
                color_at..bits.pos(),
                old_color,
                color,
                release,
            )?;
        }
    }
    Some(bits.pos())
}

/// LAYER の DWG のフラグ（BS）と色の番号。フラグのほかのビットは `old` から写す（決定 0043）
pub(super) fn layer_flags(object: &Object, xdep: bool, old: i16) -> Option<(i16, i16)> {
    let dxf = small(object, 70)??;
    (dxf & !(1 | 2 | 4 | 16) == 0 && (dxf & 16 != 0) == xdep).then_some(())?;
    let color = small(object, 62)??;
    let mut flags = old & !0x03FF;
    for (dxf_bit, dwg_bit) in [(1, 1), (2, 4), (4, 8)] {
        if dxf & dxf_bit != 0 {
            flags |= dwg_bit;
        }
    }
    if color < 0 {
        flags |= 2;
    }
    if new_plot(object)? {
        flags |= 16;
    }
    flags |= lineweight_index(integer(object, 370)??)? << 5;
    let color = color.checked_abs()?;
    (1..=255).contains(&color).then_some((flags, color))
}

/// 表のオブジェクトの項目の変え方
#[derive(Clone, Copy)]
pub(super) enum Entry {
    /// 項目のハンドルの終わりに足す（決定 0061）
    Add(u64),
    /// 項目のハンドルから除く（決定 0162）。項目に 1 つだけなければ `None`
    Remove(u64),
}

impl Entry {
    /// 項目の数 `count` の新しい値
    pub(super) fn count(self, count: i32) -> Option<i32> {
        match self {
            Self::Add(_) => count.checked_add(1),
            Self::Remove(_) => count.checked_sub(1),
        }
    }

    /// `count` 個の項目のハンドルを `bits` から読み、変えて `out` に書く
    pub(super) fn copy(self, bits: &mut Bits<'_>, count: i32, out: &mut BitWriter) -> Option<()> {
        let mut removed = 0;
        for _ in 0..count {
            let found = bits.handle()?;
            if matches!(self, Self::Remove(handle) if handle == found.value) {
                removed += 1;
                continue;
            }
            out.handle(found.code, found.value);
        }
        match self {
            Self::Add(handle) => out.handle(2, handle),
            Self::Remove(_) => (removed == 1).then_some(())?,
        }
        Some(())
    }
}

/// 型 `type_code` のレコードの表のオブジェクトのデータ `data` の、項目の数（BL）と項目のハンドルを `entry` の
/// とおりに変えたもの（決定 0061・0162）。ハンドルのストリームの後の埋めるビットは写さない
pub(super) fn add_entry(
    data: &[u8],
    entry: Entry,
    type_code: u16,
    release: Release,
) -> Option<Vec<u8>> {
    let [size_at, start, handles_at] = object_fields(data, release)?;
    let mut bits = Bits::with_release(data, release);
    bits.seek(size_at + 32);
    bits.handle()?;
    skip_extended_data(&mut bits)?;
    let reactors = bits.bl()?;
    // R2004 からは、拡張辞書を持たなければ、そのハンドルもない
    let xdic = release < Release::R2004 || !bits.bit()?;
    let count = bits.bl()?;
    let mut fields = BitWriter::default();
    fields.bl(entry.count(count)?);
    // 寸法スタイルの表は、項目の数の後に RC を持ち、項目の後にその数だけハンドルが続く。ブロックの表は、
    // 項目の後にモデル空間と用紙空間のハンドルが続く（決定 0068）。線種の表は、項目の後に ByBlock と ByLayer の
    // 線種のハンドルが続き、項目の数はこの 2 つを数えない（#683）
    let extra = match type_code {
        kinds::DIMSTYLE => bits.rc()?,
        kinds::BLOCK_RECORD | kinds::LTYPE => 2,
        _ => 0,
    };
    if type_code == kinds::DIMSTYLE {
        fields.rc(extra);
    }
    (bits.pos() == handles_at).then_some(())?;
    let mut handles = BitWriter::default();
    // 所有者・リアクター・拡張辞書と、項目
    for _ in 0..(1 + i64::from(reactors) + i64::from(xdic)) {
        let found = bits.handle()?;
        handles.handle(found.code, found.value);
    }
    entry.copy(&mut bits, count, &mut handles)?;
    // 寸法スタイルの表の項目の後のハンドルが指すものは、除かない（決定 0165）
    for _ in 0..extra {
        let found = bits.handle()?;
        (!matches!(entry, Entry::Remove(handle) if handle == found.value)).then_some(())?;
        handles.handle(found.code, found.value);
    }
    let mut out = BitWriter::default();
    out.copy(data, 0..size_at)?;
    out.rl(i32::try_from(start + fields.len()).ok()?);
    out.copy(data, size_at + 32..start)?;
    for part in [fields, handles] {
        let len = part.len();
        out.copy(&part.into_bytes(), 0..len)?;
    }
    Some(out.into_bytes())
}

/// 型 `type_code` のレコードの、R2010 から後の表のオブジェクトのデータ `data`（ハンドルのストリームの大きさ（MC）の
/// 後）の、項目の数（BL）と項目のハンドルを `entry` のとおりに変えたもの（決定 0094・0162）。`handles_at` は
/// ハンドルのストリームの始まり。ハンドルのストリームの後の埋めるビットは写さない
pub(super) fn add_entry_r2010(
    data: &[u8],
    handles_at: usize,
    entry: Entry,
    type_code: u16,
    release: Release,
) -> Option<Vec<u8>> {
    let mut bits = Bits::with_release(data, release);
    bits.bot()?;
    let end = bits.split_strings(handles_at)?;
    bits.handle()?;
    skip_extended_data(&mut bits)?;
    let reactors = bits.bl()?;
    let xdic = !bits.bit()?;
    if release >= Release::R2013 {
        bits.bit()?;
    }
    let count_at = bits.pos();
    let count = bits.bl()?;
    let count_end = bits.pos();
    let extra = match type_code {
        kinds::DIMSTYLE => bits.rc()?,
        kinds::BLOCK_RECORD | kinds::LTYPE => 2,
        _ => 0,
    };
    (bits.pos() == end).then_some(())?;
    let mut out = BitWriter::default();
    out.copy(data, 0..count_at)?;
    out.bl(entry.count(count)?);
    out.copy(data, count_end..handles_at)?;
    let at = out.len();
    bits.seek(handles_at);
    // 所有者・リアクター・拡張辞書と、項目
    for _ in 0..(1 + i64::from(reactors) + i64::from(xdic)) {
        let found = bits.handle()?;
        out.handle(found.code, found.value);
    }
    entry.copy(&mut bits, count, &mut out)?;
    // 寸法スタイルの表の項目の後のハンドルが指すものは、除かない（決定 0165）
    for _ in 0..extra {
        let found = bits.handle()?;
        (!matches!(entry, Entry::Remove(handle) if handle == found.value)).then_some(())?;
        out.handle(found.code, found.value);
    }
    with_modular_size(out, at)
}

/// 印刷する画層か。290 がなければ印刷し、偽なら印刷しない
pub(super) fn new_plot(object: &Object) -> Option<bool> {
    let mut found = object.values.iter().filter(|value| value.code == 290);
    match (found.next().map(|value| &value.data), found.next()) {
        (None, _) => Some(true),
        (Some(Data::Bool(plot)), None) => Some(*plot),
        _ => None,
    }
}

/// DXF の線の太さ（370）の、DWG の番号
pub(super) fn lineweight_index(width: i64) -> Option<i16> {
    const WIDTHS: [i64; 24] = [
        0, 5, 9, 13, 15, 18, 20, 25, 30, 35, 40, 50, 53, 60, 70, 80, 90, 100, 106, 120, 140, 158,
        200, 211,
    ];
    match width {
        -1 => Some(29),
        -2 => Some(30),
        -3 => Some(31),
        _ => WIDTHS
            .iter()
            .position(|&w| w == width)
            .and_then(|index| i16::try_from(index).ok()),
    }
}

/// STYLE のフィールド（決定 0043）。70 の形のファイル 1 と縦書き 4 をフラグに書き、角度（50）は度からラジアンに直す。
/// 外部参照に依るフラグ（70 の 16）は、元のものと同じでなければ受け付けない。文字列（3・4）が変わらなければ、
/// 元のビットを写す。元のフィールドの終わりの位置を返す
pub(super) fn style(
    fields: &mut BitWriter,
    old: &Object,
    new: &Object,
    xdep: bool,
    data: &[u8],
    own_at: usize,
    release: Release,
) -> Option<usize> {
    let mut bits = Bits::with_release(data, release);
    bits.seek(own_at);
    bits.bit()?;
    bits.bit()?;
    for _ in 0..3 {
        bits.bd()?;
    }
    bits.rc()?;
    bits.bd()?;
    let font_at = bits.pos();
    bits.text()?;
    let big_font_at = bits.pos();
    bits.text()?;
    let texts = [(3, font_at..big_font_at), (4, big_font_at..bits.pos())];
    let flags = small(new, 70)??;
    (flags & !(1 | 4 | 16) == 0 && (flags & 16 != 0) == xdep).then_some(())?;
    fields.bit(flags & 1 != 0);
    fields.bit(flags & 4 != 0);
    fields.bd(real(new, 40)??);
    fields.bd(real(new, 41)??);
    fields.bd(real(new, 50)??.to_radians());
    fields.rc(u8::try_from(small(new, 71)??).ok()?);
    fields.bd(real(new, 42)??);
    for (code, range) in texts {
        // R2010 からは、文字列は文字列のストリームに書く（決定 0089）
        if text_of(old, code) == text_of(new, code) && release < Release::R2010 {
            fields.copy(data, range)?;
        } else {
            string(fields, new, code)?;
        }
    }
    Some(bits.pos())
}

/// LTYPE のフィールド（決定 0044）。要素の数（73）と、要素ごとの形と文字のフラグ（74 の 2・4）は、元のものと同じでなければ
/// 受け付けない。要素のフラグの 7 より上のビットと、形も文字もない要素の形の番号・ずれ・尺度・回転は、元のまま写す。
/// 説明（3）と文字の要素の文字列（9）が変わらなければ元のビットを写し、文字列が変われば、256 バイト（R2010 からは
/// 512 バイト）の領域に初めから順に詰め直す。元のフィールドの終わりの位置を返す
pub(super) fn ltype(
    fields: &mut BitWriter,
    old: &Object,
    new: &Object,
    xdep: bool,
    data: &[u8],
    own_at: usize,
    release: Release,
) -> Option<usize> {
    let mut bits = Bits::with_release(data, release);
    bits.seek(own_at);
    bits.text()?;
    let description = own_at..bits.pos();
    bits.bd()?;
    bits.rc()?;
    let count = bits.rc()?;
    let mut dashes = Vec::new();
    for _ in 0..count {
        bits.bd()?;
        let shape = bits.bs()?;
        let (x, y) = (bits.rd()?, bits.rd()?);
        let (scale, rotation) = (bits.bd()?, bits.bd()?);
        dashes.push((shape, x, y, scale, rotation, bits.bs()?));
    }
    // R2010 からは、文字の要素があるときだけ、UTF-16LE の 512 バイトの領域がある（決定 0089）
    let r2010 = release >= Release::R2010;
    let area_at = bits.pos();
    if !r2010 {
        bits.skip_bytes(256)?;
    } else if dashes.iter().any(|dash| dash.5 & 2 != 0) {
        bits.skip_bytes(512)?;
    }
    let texts = |object: &Object| -> Vec<_> {
        object
            .values
            .iter()
            .filter(|value| value.code == 9)
            .cloned()
            .collect()
    };
    let same_texts = texts(old) == texts(new);
    // R2013 からは、DXF の 74 も文字の要素のフラグ（8）を持つ
    let mask = if release >= Release::R2013 { 15 } else { 7 };
    let mut values = Cursor(
        new.values
            .iter()
            .filter(|value| LTYPE.contains(&value.code))
            .collect::<Vec<_>>()
            .into_iter()
            .peekable(),
    );
    (values.int(70)? == if xdep { 16 } else { 0 }).then_some(())?;
    match values.next(3)? {
        Data::Text(_) if text_of(old, 3) == text_of(new, 3) && !r2010 => {
            fields.copy(data, description)?;
        }
        Data::Text(_) => string(fields, new, 3)?,
        _ => return None,
    }
    let align = u8::try_from(values.int(72)?).ok()?;
    (values.int(73)? == i32::from(count)).then_some(())?;
    fields.bd(values.real(40)?);
    fields.rc(align);
    fields.rc(count);
    let mut area = Vec::new();
    for (shape, x, y, scale, rotation, flags) in dashes {
        fields.bd(values.real(49)?);
        let dxf = i16::try_from(values.int(74)?).ok()?;
        (dxf & !mask == 0 && dxf & 6 == flags & 6).then_some(())?;
        if flags & 6 == 0 {
            fields.bs(shape);
            fields.rd(x);
            fields.rd(y);
            fields.bd(scale);
            fields.bd(rotation);
        } else {
            let number = i16::try_from(values.int(75)?).ok()?;
            let (scale, rotation) = (values.real(46)?, values.real(50)?.to_radians());
            let (x, y) = (values.real(44)?, values.real(45)?);
            // 文字の要素の 75 は 0 と決まっている
            let shape = if flags & 2 == 0 {
                number
            } else if number != 0 {
                return None;
            } else if same_texts {
                values.next(9)?;
                shape
            } else {
                let Data::Text(text) = values.next(9)? else {
                    return None;
                };
                // R2010 からは、UTF-16LE で詰め、形の番号はバイトの位置とする（決定 0100）
                let at = i16::try_from(area.len()).ok()?;
                let text = uncaret(text)?;
                if r2010 {
                    for unit in core::str::from_utf8(&text).ok()?.encode_utf16().chain([0]) {
                        area.extend(unit.to_le_bytes());
                    }
                } else {
                    area.extend(text);
                    area.push(0);
                }
                at
            };
            fields.bs(shape);
            fields.rd(x);
            fields.rd(y);
            fields.bd(scale);
            fields.bd(rotation);
        }
        fields.bs(flags & !mask | dxf);
    }
    values.0.next().is_none().then_some(())?;
    if same_texts {
        fields.copy(data, area_at..bits.pos())?;
    } else {
        let size = if r2010 { 512 } else { 256 };
        (area.len() <= size).then_some(())?;
        area.resize(size, 0);
        for byte in area {
            fields.rc(byte);
        }
    }
    Some(bits.pos())
}

/// DIMSTYLE のフィールド（決定 0046）。値が変わらないフィールドは元のビットを写し、変わったフィールドは書き直す。表にないフィールドは、
/// 読む側と同じ既定の値で書く。色は R2000 の CMC の番号（BS）で書く。元のフィールドの終わりの位置を返す
pub(super) fn dimstyle(
    fields: &mut BitWriter,
    old: &Object,
    new: &Object,
    xdep: bool,
    data: &[u8],
    own_at: usize,
    release: Release,
) -> Option<usize> {
    record_flags(new, xdep, 0)?;
    let of = |object: &Object, code: i16| -> Vec<_> {
        object
            .values
            .iter()
            .filter(|value| value.code == code)
            .cloned()
            .collect()
    };
    let mut bits = Bits::with_release(data, release);
    bits.seek(own_at);
    // R2010 からは、増えたフィールドも並べ、文字列は文字列のストリームに書く（決定 0089）
    let r2010 = release >= Release::R2010;
    for (code, field) in dim_fields(release) {
        let at = bits.pos();
        match field {
            Dim::Text => bits.text().map(drop)?,
            Dim::Real(_) => bits.bd().map(drop)?,
            Dim::Bit(_) => bits.bit().map(drop)?,
            Dim::Short(_) => bits.bs().map(drop)?,
            Dim::Color(_) => bits.cmc().map(drop)?,
        }
        if of(old, code) == of(new, code) && !(r2010 && matches!(field, Dim::Text)) {
            fields.copy(data, at..bits.pos())?;
            continue;
        }
        dim_field(fields, new, code, field, release)?;
    }
    Some(bits.pos())
}

/// DIMSTYLE の `code` のフィールドを、`new` の値で書く。値がなければ、読む側と同じ既定の値で書く
pub(super) fn dim_field(
    fields: &mut BitWriter,
    new: &Object,
    code: i16,
    field: Dim,
    release: Release,
) -> Option<()> {
    match field {
        Dim::Text if text_of(new, code).is_some() => string(fields, new, code)?,
        Dim::Text => {
            new.values
                .iter()
                .all(|value| value.code != code)
                .then_some(())?;
            fields.text(&[0])?;
        }
        Dim::Real(default) => fields.bd(real(new, code)?.unwrap_or(default)),
        Dim::Bit(default) => match small(new, code)? {
            None => fields.bit(default),
            Some(0) => fields.bit(false),
            Some(1) => fields.bit(true),
            Some(_) => return None,
        },
        Dim::Short(default) => fields.bs(small(new, code)?.unwrap_or(default)),
        Dim::Color(default) => cmc(fields, small(new, code)?.unwrap_or(default), release)?,
    }
    Some(())
}

/// 色の番号 `index` の CMC。元の CMC `old`（`data` の `at` のビット）が同じ番号の色なら、元のビットを写す
pub(super) fn same_cmc(
    fields: &mut BitWriter,
    data: &[u8],
    at: Range<usize>,
    old: Color,
    index: i16,
    release: Release,
) -> Option<()> {
    if old.rgb.is_none() && old.index == index {
        fields.copy(data, at)
    } else {
        cmc(fields, index, release)
    }
}

/// 色の番号 `index` の CMC（決定 0075）。R2004 からは、番号（BS）の後に、色の決め方（上位の
/// バイト）と番号（下位のバイト）の BL と、名前を持たないフラグ（RC の 0）を書く。決め方は、ByLayer（256）が
/// 0xC0、ByBlock（0）が 0xC1、ほかの番号が 0xC3
pub(super) fn cmc(fields: &mut BitWriter, index: i16, release: Release) -> Option<()> {
    fields.bs(index);
    if release >= Release::R2004 {
        let method: u32 = match index {
            256 => 0xC0 << 24,
            0 => 0xC1 << 24,
            1..=255 => (0xC3 << 24) | u32::from(index.unsigned_abs()),
            _ => return None,
        };
        fields.bl(i32::from_ne_bytes(method.to_ne_bytes()));
        fields.rc(0);
    }
    Some(())
}

/// LAYOUT のフィールドの形。読む側は、すべてのフィールドを DWG の並びで置く
#[derive(Clone, Copy)]
pub(super) enum Field {
    Text,
    Short,
    Bd,
    Rd,
}

/// LAYOUT のフィールドの形の並び（印刷の設定の 28 のフィールドと、レイアウトの 26 のフィールド）
pub(super) const LAYOUT_FIELDS: [Field; 54] = {
    use Field::{Bd, Rd, Short, Text};
    [
        Text, Text, Short, Bd, Bd, Bd, Bd, Bd, Bd, Text, Bd, Bd, Short, Short, Short, Bd, Bd, Bd,
        Bd, Text, Bd, Bd, Text, Short, Bd, Bd, Bd, Text, Short, Short, Bd, Bd, Bd, Rd, Rd, Rd, Rd,
        Bd, Bd, Bd, Bd, Bd, Bd, Bd, Bd, Bd, Bd, Short, Bd, Bd, Bd, Bd, Bd, Bd,
    ]
};

/// バージョン `release` の LAYOUT のフィールドの形の並び。R2004 からは、印刷するビューの名前がハンドルのストリームに移り、
/// 印刷の設定の終わりに陰影の印刷の 3 つのフィールドが加わる（決定 0082）
pub(super) fn layout_fields(release: Release) -> Vec<Field> {
    let mut fields = LAYOUT_FIELDS.to_vec();
    if release >= Release::R2004 {
        fields.remove(19);
        fields.splice(26..26, [Field::Short; 3]);
    }
    fields
}

/// LAYOUT のフィールドを `fields` に符号化し直し、元のフィールドの終わりの位置を返す（決定 0049・0082）。値の番号の並びが元と同じで、レイアウトの名前（2 つめの 1）
/// とハンドルの並びから作る値が変わらないものだけを扱う。フィールドは DWG の並びのまま、すべて書き直す
pub(super) fn layout(
    fields: &mut BitWriter,
    old: &Object,
    new: &Object,
    data: &[u8],
    start: usize,
    release: Release,
) -> Option<usize> {
    let codes = |object: &Object| -> Vec<i16> { object.values.iter().map(|v| v.code).collect() };
    if codes(old) != codes(new) {
        return None;
    }
    // フィールドは、ハンドル（5）と所有者（330）の後に、ハンドルの並びから作る値の前に並ぶ
    let at = new.values.iter().position(|value| value.code == 1)?;
    let layout = layout_fields(release);
    let range = at..at + layout.len();
    // レイアウトの名前は、印刷の設定のフィールドの次
    let name = at + layout.len() - 27;
    let own = new.values.get(range.clone())?;
    let rest = |object: &Object| -> Option<Vec<Value>> {
        let mut values = object.values.clone();
        values.drain(range.clone());
        // レイアウトの名前
        values.push(object.values.get(name)?.clone());
        Some(values)
    };
    if rest(old)? != rest(new)? {
        return None;
    }
    let mut bits = Bits::with_release(data, release);
    bits.seek(start);
    for (field, value) in layout.iter().zip(own) {
        match (field, &value.data) {
            (Field::Text, Data::Text(text)) => {
                bits.text()?;
                let mut text = uncaret(text)?;
                text.push(0);
                fields.text(&text)?;
            }
            (Field::Short, Data::Int(v)) => {
                bits.bs()?;
                fields.bs(i16::try_from(*v).ok()?);
            }
            (Field::Bd, Data::Real(v)) => {
                bits.bd()?;
                fields.bd(*v);
            }
            (Field::Rd, Data::Real(v)) => {
                bits.rd()?;
                fields.rd(*v);
            }
            _ => return None,
        }
    }
    Some(bits.pos())
}

/// XRECORD のフィールドを `fields` に符号化し直し、元のフィールドの終わりの位置を返す（決定 0050）。値の番号の並びが元と同じものだけを扱う。データは、
/// 元のデータを読みながら値を書き直し、文字列のコード・ページのバイトは元の値のものを写す。R2010 からの文字列は UTF-16LE で書く
pub(super) fn xrecord(
    fields: &mut BitWriter,
    old: &Object,
    new: &Object,
    data: &[u8],
    start: usize,
    release: Release,
) -> Option<usize> {
    let codes = |object: &Object| -> Vec<i16> { object.values.iter().map(|v| v.code).collect() };
    if codes(old) != codes(new) {
        return None;
    }
    // 複製の扱い（280）の前は、ハンドル（5）と所有者（330）
    let at = new.values.iter().position(|value| value.code == 280)?;
    if old.values.get(..at) != new.values.get(..at) {
        return None;
    }
    let Data::Int(cloning) = new.values.get(at)?.data else {
        return None;
    };
    let mut values = new.values.get(at + 1..)?.iter();
    let mut next = |code: i16| -> Option<&Data> {
        let value = values.next()?;
        (value.code == code).then_some(&value.data)
    };
    let mut bits = Bits::with_release(data, release);
    bits.seek(start);
    let len = usize::try_from(bits.bl()?).ok()?;
    let end = bits.pos().checked_add(len.checked_mul(8)?)?;
    let mut items = BitWriter::default();
    while bits.pos() < end {
        let code = bits.rs()?;
        items.rs(code);
        match DataType::of(code) {
            DataType::Real if matches!(code, 10..=19 | 1010..=1019) => {
                for code in [code, code + 10, code + 20] {
                    bits.rd()?;
                    let &Data::Real(v) = next(code)? else {
                        return None;
                    };
                    items.rd(v);
                }
            }
            DataType::Real => {
                bits.rd()?;
                let &Data::Real(v) = next(code)? else {
                    return None;
                };
                items.rd(v);
            }
            DataType::Int => {
                let &Data::Int(v) = next(code)? else {
                    return None;
                };
                match code {
                    280..=289 => {
                        bits.rc()?;
                        items.rc(u8::try_from(v).ok()?);
                    }
                    90..=99 | 420..=429 | 440..=459 | 1071 => {
                        bits.rl()?;
                        items.rl(i32::try_from(v).ok()?);
                    }
                    160..=169 => {
                        bits.skip_bytes(8)?;
                        items.raw(&v.to_le_bytes());
                    }
                    _ => {
                        bits.rs()?;
                        items.rs(i16::try_from(v).ok()?);
                    }
                }
            }
            DataType::Bool => {
                bits.rc()?;
                let &Data::Bool(v) = next(code)? else {
                    return None;
                };
                items.rc(u8::from(v));
            }
            DataType::Handle => {
                bits.skip_bytes(8)?;
                let &Data::Handle(v) = next(code)? else {
                    return None;
                };
                items.raw(&v.to_le_bytes());
            }
            DataType::Binary => {
                let len = usize::from(bits.rc()?);
                bits.skip_bytes(len)?;
                let Data::Binary(v) = next(code)? else {
                    return None;
                };
                items.rc(u8::try_from(v.len()).ok()?);
                items.raw(v);
            }
            // R2010 からは、文字の数（RS）と UTF-16LE。コード・ページを持たない
            DataType::Text if release >= Release::R2010 => {
                let len = usize::try_from(bits.rs()?).ok()?;
                bits.skip_bytes(len.checked_mul(2)?)?;
                let Data::Text(v) = next(code)? else {
                    return None;
                };
                let text = uncaret(v)?;
                let units: Vec<u16> = core::str::from_utf8(&text).ok()?.encode_utf16().collect();
                items.rs(i16::try_from(units.len()).ok()?);
                for unit in units {
                    items.raw(&unit.to_le_bytes());
                }
            }
            DataType::Text => {
                let len = usize::try_from(bits.rs()?).ok()?;
                let code_page = bits.rc()?;
                bits.skip_bytes(len)?;
                let Data::Text(v) = next(code)? else {
                    return None;
                };
                let text = uncaret(v)?;
                items.rs(i16::try_from(text.len()).ok()?);
                items.rc(code_page);
                items.raw(&text);
            }
        }
    }
    if bits.pos() != end || values.next().is_some() {
        return None;
    }
    bits.bs()?;
    fields.bl(i32::try_from(items.len() / 8).ok()?);
    let len = items.len();
    fields.copy(&items.into_bytes(), 0..len)?;
    fields.bs(i16::try_from(cloning).ok()?);
    Some(bits.pos())
}

/// DICTIONARY のフィールド（決定 0048）。項目の数は元のまま写し、複製の扱い（281）・所有するフラグ（280）・項目の
/// 名前（3）を書く。読む側は 0 の所有するフラグを置かないので、置いたフラグは 1〜255 だけを受け付ける
pub(super) fn dictionary(
    fields: &mut BitWriter,
    bits: &mut Bits,
    new: &Object,
    data: &[u8],
) -> Option<()> {
    let start = bits.pos();
    bits.bl()?;
    fields.copy(data, start..bits.pos())?;
    bits.bs()?;
    bits.rc()?;
    fields.bs(small(new, 281)??);
    let owner = small(new, 280)?.map_or(Some(0), |owner| {
        u8::try_from(owner).ok().filter(|o| *o != 0)
    })?;
    fields.rc(owner);
    for value in new.values.iter().filter(|value| value.code == 3) {
        bits.text()?;
        let Data::Text(name) = &value.data else {
            return None;
        };
        let mut name = uncaret(name)?;
        name.push(0);
        fields.text(&name)?;
    }
    Some(())
}

/// MLINESTYLE のフィールド（決定 0048）。名前は元のまま写す（R2010 からは書き直す）。フラグ（70）は DXF のビットを DWG のビットに並べ直し、
/// DXF にない DWG のビットは元のまま写す。角度は度からラジアンに直す。線の線種の番号（R2018 からはハンドル）は
/// 元のまま写す
pub(super) fn mlinestyle(
    fields: &mut BitWriter,
    bits: &mut Bits,
    new: &Object,
    data: &[u8],
    release: Release,
) -> Option<()> {
    // DWG のビットと、DXF のビット（読む側と同じ）
    const MOVED: [(i16, i16); 8] = [
        (1, 2),
        (2, 1),
        (16, 16),
        (32, 64),
        (64, 32),
        (256, 256),
        (512, 1024),
        (1024, 512),
    ];
    // R2010 からは、名前を文字列のストリームに書く
    let start = bits.pos();
    bits.text()?;
    if release >= Release::R2010 {
        string(fields, new, 2)?;
    } else {
        fields.copy(data, start..bits.pos())?;
    }
    bits.text()?;
    let flags = bits.bs()?;
    let color_at = bits.pos();
    let old_color = bits.cmc()?;
    let color_end = bits.pos();
    for _ in 0..2 {
        bits.bd()?;
    }
    let lines = bits.rc()?;
    // 名前の後の、型ごとの値の並び（フラグ・説明・色・2 つの角度・線の数・線ごとの位置と色）
    let at = new.values.iter().position(|value| value.code == 2)?;
    let own = new.values.get(at + 1..)?;
    let int = |value: &Value| match value.data {
        Data::Int(value) => i16::try_from(value).ok(),
        _ => None,
    };
    let float = |value: &Value| match value.data {
        Data::Real(value) => Some(value),
        _ => None,
    };
    let [dxf, about, color, begin, end, count, rest @ ..] = own else {
        return None;
    };
    let dxf = int(dxf)?;
    if dxf & !MOVED.iter().fold(0, |sum, (_, bit)| sum | bit) != 0
        || int(count)? != i16::from(lines)
    {
        return None;
    }
    let mut dwg = MOVED.iter().fold(flags, |sum, (bit, _)| sum & !bit);
    for (bit, dxf_bit) in MOVED {
        if dxf & dxf_bit != 0 {
            dwg |= bit;
        }
    }
    fields.text(&{
        let Data::Text(about) = &about.data else {
            return None;
        };
        let mut about = uncaret(about)?;
        about.push(0);
        about
    })?;
    fields.bs(dwg);
    same_cmc(
        fields,
        data,
        color_at..color_end,
        old_color,
        int(color)?,
        release,
    )?;
    fields.bd(float(begin)?.to_radians());
    fields.bd(float(end)?.to_radians());
    fields.rc(lines);
    for line in rest.chunks(2) {
        let [offset, color] = line else {
            return None;
        };
        bits.bd()?;
        let color_at = bits.pos();
        let old_color = bits.cmc()?;
        let ltype = bits.pos();
        // R2018 からは、線種を番号でなくハンドルで指す。ハンドルは元のまま写す（決定 0099）
        if release < Release::R2018 {
            bits.bs()?;
        }
        fields.bd(float(offset)?);
        same_cmc(
            fields,
            data,
            color_at..ltype,
            old_color,
            int(color)?,
            release,
        )?;
        fields.copy(data, ltype..bits.pos())?;
    }
    Some(())
}

/// 外部参照に依るフラグ（70 の 16）が元のものと同じで、ほかに `allowed` のビットだけを持つ 70
pub(super) fn record_flags(object: &Object, xdep: bool, allowed: i16) -> Option<i16> {
    let flags = small(object, 70)??;
    (flags & !(allowed | 16) == 0 && (flags & 16 != 0) == xdep).then_some(flags)
}

/// 0 か 1 の整数の値
pub(super) fn flag(object: &Object, code: i16) -> Option<bool> {
    match small(object, code)?? {
        0 => Some(false),
        1 => Some(true),
        _ => None,
    }
}

/// 2RD の点（`code`・`code + 10`）
pub(super) fn point2(fields: &mut BitWriter, object: &Object, code: i16) -> Option<()> {
    for code in [code, code + 10] {
        fields.rd(real(object, code)??);
    }
    Some(())
}

/// VIEW・VPORT の見え方のフラグ（71）の、透視・前の切り取り・後ろの切り取りと、前の切り取りが目の位置にあること
/// （DXF の 16 の逆）の 4 つのビット
pub(super) fn view_mode(fields: &mut BitWriter, mode: i16) {
    for flag in [1, 2, 4] {
        fields.bit(mode & flag != 0);
    }
    fields.bit(mode & 16 == 0);
}

/// UCS の原点と軸（110〜112）と、高さ（146）・正投影の種類（79）
pub(super) fn ucs_axes(fields: &mut BitWriter, object: &Object) -> Option<()> {
    for code in [110, 111, 112] {
        point3(fields, object, code)?;
    }
    fields.bd(real(object, 146)??);
    fields.bs(small(object, 79)??);
    Some(())
}

/// VIEW のフィールド（決定 0045）。ねじれ（50）は度からラジアンに直し、紙空間のフラグは 70 の 1 から書く。UCS を伴うフラグ
/// （72）は、ハンドルのストリームの数が変わるので、元のものと同じでなければ受け付けない。元のフィールドの終わりの位置を返す
pub(super) fn view_record(
    fields: &mut BitWriter,
    new: &Object,
    xdep: bool,
    data: &[u8],
    own_at: usize,
    release: Release,
) -> Option<usize> {
    let mut bits = Bits::with_release(data, release);
    bits.seek(own_at);
    let old = crate::dwg::objects::view(&mut bits, 0, &mut Vec::new())?;
    let was = old.iter().find(|value| value.code == 72)?;
    (was.data == Data::Int(i64::from(flag(new, 72)?))).then_some(())?;
    // R2010 からは、描画の種類（281）の後の照明のフィールドを、元のビットから写す（決定 0089）
    let lighting = if release >= Release::R2010 {
        let mut bits = Bits::with_release(data, release);
        bits.seek(own_at);
        for _ in 0..2 {
            bits.bd()?;
        }
        for _ in 0..2 {
            bits.rd()?;
        }
        for _ in 0..10 {
            bits.bd()?;
        }
        for _ in 0..4 {
            bits.bit()?;
        }
        bits.rc()?;
        let at = bits.pos();
        bits.bit()?;
        bits.rc()?;
        bits.bd()?;
        bits.bd()?;
        bits.cmc()?;
        Some((data, at..bits.pos()))
    } else {
        None
    };
    view_fields(fields, new, xdep, lighting)?;
    Some(bits.pos())
}

/// R2010 から後の VIEW・VPORT の、描画の種類の後の照明のフィールドの既定の値と、そのビットの長さ。サンプルの図面と同じく、
/// 既定の照明を使い、種類は 1、明るさと対比は 0、環境の色は真の色の 0x333333 とする（決定 0095・0096）
pub(super) fn default_lighting() -> (Vec<u8>, usize) {
    let mut lighting = BitWriter::default();
    lighting.bit(true);
    lighting.rc(1);
    lighting.bd(0.0);
    lighting.bd(0.0);
    lighting.bs(0);
    lighting.bl(i32::from_ne_bytes(0xC233_3333_u32.to_ne_bytes()));
    lighting.rc(0);
    let bits = lighting.len();
    (lighting.into_bytes(), bits)
}

/// 表のレコードの `code` の UCS のハンドル。なければ 0。図面にある UCS を指さなければ `None`
pub(super) fn ucs_handle(db: &AcadDb, object: &Object, code: i16) -> Option<u64> {
    match object.values.iter().find(|value| value.code == code) {
        Some(value) => match value.data {
            Data::Handle(h) => (db.get(h)?.kind == b"UCS").then_some(h),
            _ => None,
        },
        None => Some(0),
    }
}

/// VIEW の型ごとのフィールド。UCS を伴うかを返す
pub(super) fn view_fields(
    fields: &mut BitWriter,
    new: &Object,
    xdep: bool,
    lighting: Option<(&[u8], Range<usize>)>,
) -> Option<bool> {
    let associated = flag(new, 72)?;
    let paper = record_flags(new, xdep, 1)? & 1 != 0;
    let mode = small(new, 71)??;
    (mode & !(1 | 2 | 4 | 16) == 0).then_some(())?;
    fields.bd(real(new, 40)??);
    fields.bd(real(new, 41)??);
    point2(fields, new, 10)?;
    point3(fields, new, 12)?;
    point3(fields, new, 11)?;
    fields.bd(real(new, 50)??.to_radians());
    for code in [42, 43, 44] {
        fields.bd(real(new, code)??);
    }
    view_mode(fields, mode);
    fields.rc(u8::try_from(small(new, 281)??).ok()?);
    if let Some((data, range)) = lighting {
        fields.copy(data, range)?;
    }
    fields.bit(paper);
    fields.bit(associated);
    if associated {
        ucs_axes(fields, new)?;
    }
    Some(associated)
}

/// UCS のフィールド（決定 0045）。後に続く正投影の型は、表にないので元のまま写す。元のフィールドの終わりの位置を返す
pub(super) fn ucs_record(
    fields: &mut BitWriter,
    new: &Object,
    xdep: bool,
    data: &[u8],
    own_at: usize,
    release: Release,
) -> Option<usize> {
    let mut bits = Bits::with_release(data, release);
    bits.seek(own_at);
    for _ in 0..10 {
        bits.bd()?;
    }
    bits.bs()?;
    record_flags(new, xdep, 0)?;
    for code in [10, 11, 12] {
        point3(fields, new, code)?;
    }
    fields.bd(real(new, 146)??);
    fields.bs(small(new, 79)??);
    Some(bits.pos())
}

/// VPORT のフィールド（決定 0045）。幅は、高さ（40）と比（41）の積で書く。高さと比が変わらなければ、元の幅を写す。
/// 角度（50・51）は度からラジアンに直し、UCS に従うフラグは 71 の 8 から書く。使い道の分からないビットは、元のまま
/// 写す。元のフィールドの終わりの位置を返す
pub(super) fn vport_record(
    fields: &mut BitWriter,
    old: &Object,
    new: &Object,
    xdep: bool,
    data: &[u8],
    own_at: usize,
    release: Release,
) -> Option<usize> {
    let mut bits = Bits::with_release(data, release);
    bits.seek(own_at);
    bits.bd()?;
    let width = bits.bd()?;
    // 中心・注視点・向き・ねじれ・レンズ・切り取りから、スナップの基点と間隔まで
    bits.rd()?;
    bits.rd()?;
    for _ in 0..10 {
        bits.bd()?;
    }
    for _ in 0..4 {
        bits.bit()?;
    }
    bits.rc()?;
    // R2010 からは、描画の種類の後の照明のフィールドを、元のビットから写す（決定 0096）
    let lighting = if release >= Release::R2010 {
        let at = bits.pos();
        bits.bit()?;
        bits.rc()?;
        bits.bd()?;
        bits.bd()?;
        bits.cmc()?;
        Some((data, at..bits.pos()))
    } else {
        None
    };
    for _ in 0..4 {
        bits.rd()?;
    }
    bits.bit()?;
    bits.bs()?;
    bits.bit()?;
    bits.bb()?;
    bits.bit()?;
    bits.rd()?;
    bits.rd()?;
    bits.bit()?;
    bits.bit()?;
    bits.bs()?;
    bits.bd()?;
    for _ in 0..4 {
        bits.rd()?;
    }
    let unknown = bits.bit()?;
    bits.seek(own_at);
    crate::dwg::objects::vport(&mut bits)?;
    let same = real(old, 40)? == real(new, 40)? && real(old, 41)? == real(new, 41)?;
    vport_fields(fields, new, xdep, same.then_some(width), unknown, lighting)?;
    Some(bits.pos())
}

/// VPORT の型ごとのフィールド。幅は、`width` があればそれを、なければ高さ（40）と比（41）の積を書く。
/// `unknown` は、使い道の分からないビット
pub(super) fn vport_fields(
    fields: &mut BitWriter,
    new: &Object,
    xdep: bool,
    width: Option<f64>,
    unknown: bool,
    lighting: Option<(&[u8], Range<usize>)>,
) -> Option<()> {
    record_flags(new, xdep, 0)?;
    let mode = small(new, 71)??;
    (mode & !(1 | 2 | 4 | 8 | 16) == 0).then_some(())?;
    let height = real(new, 40)??;
    let ratio = real(new, 41)??;
    let width = match width {
        Some(width) => width,
        None if height == 0.0 => ratio,
        None => ratio * height,
    };
    fields.bd(height);
    fields.bd(width);
    point2(fields, new, 12)?;
    point3(fields, new, 17)?;
    point3(fields, new, 16)?;
    fields.bd(real(new, 51)??.to_radians());
    for code in [42, 43, 44] {
        fields.bd(real(new, code)??);
    }
    view_mode(fields, mode);
    fields.rc(u8::try_from(small(new, 281)??).ok()?);
    if let Some((data, range)) = lighting {
        fields.copy(data, range)?;
    }
    point2(fields, new, 10)?;
    point2(fields, new, 11)?;
    fields.bit(mode & 8 != 0);
    fields.bs(small(new, 72)??);
    fields.bit(flag(new, 73)?);
    let icon = u8::try_from(small(new, 74)??).ok()?;
    (icon < 4).then_some(())?;
    fields.bb(icon);
    fields.bit(flag(new, 76)?);
    point2(fields, new, 15)?;
    fields.bit(flag(new, 75)?);
    fields.bit(flag(new, 77)?);
    fields.bs(small(new, 78)??);
    fields.bd(real(new, 50)??.to_radians());
    point2(fields, new, 13)?;
    point2(fields, new, 14)?;
    fields.bit(unknown);
    fields.bit(flag(new, 65)?);
    ucs_axes(fields, new)
}
