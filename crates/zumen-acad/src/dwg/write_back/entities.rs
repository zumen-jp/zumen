//! 図形の種類ごとのフィールドの符号化。

use super::*;

/// LINE のフィールド。両方の Z が 0 なら、Z を書かない
pub(super) fn line(fields: &mut BitWriter, object: &Object) -> Option<()> {
    let [x0, y0, z0, x1, y1, z1] = [10, 20, 30, 11, 21, 31].map(|code| real(object, code));
    let (x0, y0, z0, x1, y1, z1) = (x0??, y0??, z0??, x1??, y1??, z1??);
    let flat = z0.to_bits() == 0 && z1.to_bits() == 0;
    fields.bit(flat);
    fields.rd(x0);
    fields.dd(x1, x0);
    fields.rd(y0);
    fields.dd(y1, y0);
    if !flat {
        fields.rd(z0);
        fields.dd(z1, z0);
    }
    thickness_and_extrusion(fields, object)
}

/// SOLID・TRACE のフィールド。4 つの角の Z（30〜33）は、どれも同じ標高でなければならない
pub(super) fn solid(fields: &mut BitWriter, object: &Object) -> Option<()> {
    let elevation = real(object, 30)??;
    let mut corners = Vec::new();
    for code in 10..14 {
        let z = real(object, code + 20)??;
        if z.to_bits() != elevation.to_bits() {
            return None;
        }
        corners.extend([real(object, code)??, real(object, code + 10)??]);
    }
    thickness(fields, object)?;
    fields.bd(elevation);
    for value in corners {
        fields.rd(value);
    }
    extrusion(fields, object)
}

/// 3DFACE のフィールド。見えない辺のフラグ（70）がなければ、フラグを書かない
pub(super) fn face3d(fields: &mut BitWriter, object: &Object) -> Option<()> {
    let mut corners = [[0.0; 3]; 4];
    for (corner, code) in corners.iter_mut().zip(10..) {
        *corner = [
            real(object, code)??,
            real(object, code + 10)??,
            real(object, code + 20)??,
        ];
    }
    let invisible = small(object, 70)?;
    let [first, ..] = corners;
    let z_is_zero = first[2].to_bits() == 0;
    fields.bit(invisible.is_none());
    fields.bit(z_is_zero);
    fields.rd(first[0]);
    fields.rd(first[1]);
    if !z_is_zero {
        fields.rd(first[2]);
    }
    for (previous, corner) in corners.iter().zip(&corners[1..]) {
        for (value, default) in corner.iter().zip(previous) {
            fields.dd(*value, *default);
        }
    }
    if let Some(flags) = invisible {
        fields.bs(flags);
    }
    Some(())
}

/// TEXT のフィールド（R2000）。初めのフラグ（RC）は、標高・合わせの点・斜めの角度・回転・幅の比・合わせの向きの、
/// ないフィールドを表す。標高（30）が 0 なら書かない。合わせの点の Z（31）は、標高と同じでなければならない。
/// `vertical` は縦の合わせのグループ・コード
pub(super) fn text(fields: &mut BitWriter, object: &Object, vertical: i16) -> Option<()> {
    let [x, y, elevation, height] = [10, 20, 30, 40].map(|code| real(object, code));
    let (x, y, elevation, height) = (x??, y??, elevation??, height??);
    let alignment = match [11, 21, 31].map(|code| real(object, code)) {
        [Some(None), Some(None), Some(None)] => None,
        [Some(Some(x)), Some(Some(y)), Some(Some(z))] if z.to_bits() == elevation.to_bits() => {
            Some((x, y))
        }
        _ => return None,
    };
    let [oblique, rotation, width] = [51, 50, 41].map(|code| real(object, code));
    let (oblique, rotation, width) = (oblique?, rotation?, width?);
    let [generation, horizontal, vertical] = [71, 72, vertical].map(|code| small(object, code));
    let justification = [generation?, horizontal?, vertical?];
    let mut flags = 0;
    for (absent, bit) in [
        (elevation.to_bits() == 0, 0x01),
        (alignment.is_none(), 0x02),
        (oblique.is_none(), 0x04),
        (rotation.is_none(), 0x08),
        (width.is_none(), 0x10),
        (justification[0].is_none(), 0x20),
        (justification[1].is_none(), 0x40),
        (justification[2].is_none(), 0x80),
    ] {
        if absent {
            flags |= bit;
        }
    }
    fields.rc(flags);
    if elevation.to_bits() != 0 {
        fields.rd(elevation);
    }
    fields.rd(x);
    fields.rd(y);
    if let Some((ax, ay)) = alignment {
        fields.dd(ax, x);
        fields.dd(ay, y);
    }
    // TEXT は、押し出しの向きが厚さの前にある
    extrusion(fields, object)?;
    thickness(fields, object)?;
    // DXF は度、DWG はラジアンで書く
    for angle in [oblique, rotation].into_iter().flatten() {
        fields.rd(angle.to_radians());
    }
    fields.rd(height);
    if let Some(width) = width {
        fields.rd(width);
    }
    string(fields, object, 1)?;
    for value in justification.into_iter().flatten() {
        fields.bs(value);
    }
    Some(())
}

/// ATTRIB・ATTDEF のフィールド（R2000）。TEXT のフィールドに続けて、タグ・フィールドの長さ（なければ 0）・フラグを書き、ATTDEF は
/// 問いも書く
pub(super) fn attribute(fields: &mut BitWriter, object: &Object) -> Option<()> {
    text(fields, object, 74)?;
    string(fields, object, 2)?;
    fields.bs(small(object, 73)?.unwrap_or(0));
    fields.rc(u8::try_from(small(object, 70)??).ok()?);
    if object.kind == b"ATTDEF" {
        string(fields, object, 3)?;
    }
    Some(())
}

/// MTEXT のフィールド（R2000）。押し出しの向きと X 軸の向きは、なければ (0, 0, 1)・(1, 0, 0) を書く。文字列は、
/// 250 バイトずつの 3 と、残りの 1 をつないで書く。表にない文字の範囲の大きさ（2 つの BD）と、行の間隔の後の
/// B は、元のデータから写す。足した MTEXT では、どちらも 0 にする（決定 0054）
pub(super) fn mtext(
    fields: &mut BitWriter,
    object: &Object,
    data: &[u8],
    start: usize,
) -> Option<()> {
    // 足した MTEXT は元のデータを持たない（`data` が空）。範囲の大きさを 0、B を 0 にする
    let copied = match data.is_empty() {
        true => None,
        false => {
            let mut bits = Bits::with_release(data, Release::R2000);
            bits.seek(start);
            for _ in 0..11 {
                bits.bd()?;
            }
            bits.bs()?;
            bits.bs()?;
            let extents_at = bits.pos();
            bits.bd()?;
            bits.bd()?;
            let extents_end = bits.pos();
            bits.text()?;
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
    for code in [41, 40] {
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
    text.push(0);
    fields.text(&text)?;
    fields.bs(small(object, 73)??);
    fields.bd(real(object, 44)??);
    match copied {
        Some((_, flag_at)) => fields.copy(data, flag_at..flag_at + 1),
        None => {
            fields.bit(false);
            Some(())
        }
    }
}

/// VERTEX のフィールド（決定 0038）。2D の頂点は、始まりと終わりの幅が同じで 0 でなければ、負の始まりの幅だけを
/// 書く。角度は度からラジアンに直す。多面の面は、点が 0 でフラグが 128 のものだけを受け付ける
pub(super) fn vertex(fields: &mut BitWriter, object: &Object, type_code: u16) -> Option<()> {
    let flags = |object| small(object, 70).map(|flags| flags.unwrap_or(0));
    match type_code {
        kinds::VERTEX_2D..=kinds::VERTEX_PFACE => {
            fields.rc(u8::try_from(flags(object)?).ok()?);
            point3(fields, object, 10)?;
            if type_code != kinds::VERTEX_2D {
                return Some(());
            }
            let [start, end, bulge, tangent] =
                [40, 41, 42, 50].map(|code| real(object, code).map(|v| v.unwrap_or(0.0)));
            let (start, end) = (start?, end?);
            if start.to_bits() == end.to_bits() && start > 0.0 {
                fields.bd(-start);
            } else if start < 0.0 || end < 0.0 {
                return None;
            } else {
                fields.bd(start);
                fields.bd(end);
            }
            fields.bd(bulge?);
            fields.bd(tangent?.to_radians());
        }
        kinds::VERTEX_PFACE_FACE => {
            zero_point(object)?;
            (flags(object)? == 128).then_some(())?;
            for code in 71..75 {
                fields.bs(small(object, code)?.unwrap_or(0));
            }
        }
        _ => return None,
    }
    Some(())
}

/// POLYLINE のフィールド（決定 0038）。頂点が続くフラグ（66）は 1、点（10・20）は 0 だけを受け付ける。3D の
/// POLYLINE の 2 つのフラグは、70・75 から読む値が変わらなければ元のバイトを写し、変われば読まれる
/// ビットだけを書き換える
pub(super) fn polyline(
    fields: &mut BitWriter,
    object: &Object,
    type_code: u16,
    data: &[u8],
    start: usize,
) -> Option<()> {
    (small(object, 66)? == Some(1)).then_some(())?;
    let flags = small(object, 70)?;
    let surface = small(object, 75)?.unwrap_or(0);
    match type_code {
        kinds::POLYLINE_2D => {
            for code in [10, 20] {
                (real(object, code)? == Some(0.0)).then_some(())?;
            }
            fields.bs(flags.unwrap_or(0));
            fields.bs(surface);
            for code in [40, 41] {
                fields.bd(real(object, code)?.unwrap_or(0.0));
            }
            thickness(fields, object)?;
            fields.bd(real(object, 30)??);
            extrusion(fields, object)?;
        }
        kinds::POLYLINE_3D => {
            zero_point(object)?;
            let flags = flags?;
            let curve = match surface {
                0 => 0,
                5 => 1,
                6 => 2,
                _ => return None,
            };
            // 8 は 3D、4 は曲線のフラグ。読む側が作るのは 8・4・1 だけ
            let splined = if curve != 0 { 4 } else { 0 };
            (flags & !1 == 8 | splined).then_some(())?;
            let mut bits = Bits::with_release(data, Release::R2000);
            bits.seek(start);
            let (old_curve, old_closed) = (bits.rc()?, bits.rc()?);
            let read = |curve: u8| match curve {
                c if c & 1 != 0 => 1,
                c if c & 2 != 0 => 2,
                _ => 0,
            };
            fields.rc(if read(old_curve) == curve {
                old_curve
            } else {
                old_curve & !3 | curve
            });
            fields.rc(old_closed & !1 | u8::try_from(flags & 1).ok()?);
        }
        kinds::POLYLINE_PFACE => {
            zero_point(object)?;
            (flags == Some(64)).then_some(())?;
            for code in [71, 72] {
                fields.bs(small(object, code)??);
            }
        }
        kinds::POLYLINE_MESH => {
            zero_point(object)?;
            fields.bs(flags?);
            fields.bs(surface);
            for code in 71..75 {
                fields.bs(small(object, code)??);
            }
        }
        _ => return None,
    }
    Some(())
}

/// DIMENSION のフィールド（決定 0039）。表にないブロックの挿入の尺度と回転は、元のビットを写す。フラグは、70 から
/// 読まれるビットだけを書き換える。利用者の文字（1）が変わらなければ、元のビットを写す。角度は度から
/// ラジアンに直す。標高（31）と、12 の点の Z・2 線の角度の 36 は同じ値だけを受け付ける
pub(super) fn dimension(
    fields: &mut BitWriter,
    old: &Object,
    new: &Object,
    type_code: u16,
    data: &[u8],
    start: usize,
    release: Release,
) -> Option<()> {
    let at = usize::from(type_code.checked_sub(kinds::DIMENSION_ORDINATE)?);
    let (kind, own) = (*DIMENSION_KIND.get(at)?, *DIMENSION_KINDS.get(at)?);
    // ほかの型のフィールドがあるものは受け付けない
    let allowed = |code| DIMENSION.contains(&code) || own.contains(&code);
    if new
        .values
        .iter()
        .any(|value| DIMENSION_ALL.contains(&value.code) && !allowed(value.code))
    {
        return None;
    }
    // R2010 からは、初めの寸法のバージョン（RC）と、測った値の後の 3 つの B を写し、文字列は文字列のストリームに書く
    // （決定 0088）
    let r2010 = release >= Release::R2010;
    let mut bits = Bits::with_release(data, release);
    bits.seek(start);
    let version_at = bits.pos();
    if r2010 {
        bits.rc()?;
    }
    let version = version_at..bits.pos();
    for _ in 0..3 {
        bits.bd()?;
    }
    bits.rd()?;
    bits.rd()?;
    bits.bd()?;
    let old_flags = bits.rc()?;
    let text_at = bits.pos();
    bits.text()?;
    let text_end = bits.pos();
    bits.bd()?;
    bits.bd()?;
    let scale_at = bits.pos();
    for _ in 0..4 {
        bits.bd()?;
    }
    let scale_end = bits.pos();
    bits.bs()?;
    bits.bs()?;
    bits.bd()?;
    bits.bd()?;
    let arrows_at = bits.pos();
    if r2010 {
        for _ in 0..3 {
            bits.bit()?;
        }
    }
    let arrows = arrows_at..bits.pos();
    bits.rd()?;
    bits.rd()?;
    let old_ordinate = if type_code == kinds::DIMENSION_ORDINATE {
        for _ in 0..9 {
            bits.bd()?;
        }
        bits.rc()?
    } else {
        0
    };
    let flags = small(new, 70)??;
    let ordinate = if type_code == kinds::DIMENSION_ORDINATE {
        64
    } else {
        0
    };
    (flags & !(32 | ordinate | 128) == kind).then_some(())?;
    let elevation = real(new, 31)??;
    let same = |code| real(new, code).map(|z| z.is_none_or(|z| z.to_bits() == elevation.to_bits()));
    (same(32)? && same(36)?).then_some(())?;
    fields.copy(data, version)?;
    match [210, 220, 230].map(|code| real(new, code)) {
        [Some(None), Some(None), Some(None)] => [0.0, 0.0, 1.0].map(|v| fields.bd(v)),
        [Some(Some(x)), Some(Some(y)), Some(Some(z))] => [x, y, z].map(|v| fields.bd(v)),
        _ => return None,
    };
    fields.rd(real(new, 11)??);
    fields.rd(real(new, 21)??);
    fields.bd(elevation);
    let user_location = if flags & 128 == 0 { 1 } else { 0 };
    let block_only = if flags & 32 != 0 { 2 } else { 0 };
    fields.rc(old_flags & !3 | user_location | block_only);
    let texts = |object: &Object| -> Vec<Value> {
        object
            .values
            .iter()
            .filter(|v| v.code == 1)
            .cloned()
            .collect()
    };
    if texts(old) == texts(new) && !r2010 {
        fields.copy(data, text_at..text_end)?;
    } else if texts(new).is_empty() {
        fields.text(&[])?;
    } else {
        string(fields, new, 1)?;
    }
    for code in [53, 51] {
        fields.bd(real(new, code)?.unwrap_or(0.0).to_radians());
    }
    fields.copy(data, scale_at..scale_end)?;
    fields.bs(small(new, 71)??);
    fields.bs(small(new, 72)?.unwrap_or(1));
    fields.bd(real(new, 41)?.unwrap_or(1.0));
    fields.bd(real(new, 42)??);
    fields.copy(data, arrows)?;
    let [x, y, z] = [12, 22, 32].map(|code| real(new, code));
    match [x?, y?, z?] {
        [Some(x), Some(y), Some(_)] => {
            fields.rd(x);
            fields.rd(y);
        }
        [None, None, None] => {
            fields.rd(0.0);
            fields.rd(0.0);
        }
        _ => return None,
    }
    for &code in own {
        match code {
            10 | 13 | 14 | 15 => point3(fields, new, code)?,
            16 => {
                fields.rd(real(new, 16)??);
                fields.rd(real(new, 26)??);
            }
            40 => fields.bd(real(new, 40)??),
            50 | 52 => fields.bd(real(new, code)?.unwrap_or(0.0).to_radians()),
            _ => {}
        }
    }
    if type_code == kinds::DIMENSION_ORDINATE {
        fields.rc(old_ordinate & !1 | u8::from(flags & 64 != 0));
    }
    Some(())
}

/// 点（10・20・30）がどれも 0 なら `Some`
pub(super) fn zero_point(object: &Object) -> Option<()> {
    [10, 20, 30]
        .iter()
        .all(|&code| real(object, code) == Some(Some(0.0)))
        .then_some(())
}

/// 元の INSERT・MINSERT のフィールドの回転（ラジアン）。元のデータがなければ `None`
pub(super) fn insert_rotation(data: &[u8], start: usize) -> Option<f64> {
    let mut bits = Bits::new(data);
    bits.seek(start);
    for _ in 0..3 {
        bits.bd()?;
    }
    match bits.bb()? {
        3 => {}
        1 => {
            bits.dd(1.0)?;
            bits.dd(1.0)?;
        }
        2 => {
            bits.rd()?;
        }
        _ => {
            let x = bits.rd()?;
            bits.dd(x)?;
            bits.dd(x)?;
        }
    }
    bits.bd()
}

/// INSERT・MINSERT のフィールド（R2000）。尺度（41〜43）がなければ 1、回転（50）がなければ 0、押し出しの向きが
/// なければ (0, 0, 1) を書く。回転が変わらなければ、元のラジアン `kept` を書く。尺度のフラグ（BB）は、尺度の値から
/// 選ぶ。属性が続くかのフラグは、66 から書く
pub(super) fn insert(
    fields: &mut BitWriter,
    object: &Object,
    grid: bool,
    kept: Option<f64>,
) -> Option<()> {
    point3(fields, object, 10)?;
    let [x, y, z] = [41, 42, 43].map(|code| real(object, code));
    let [x, y, z] = [x?, y?, z?].map(|scale| scale.unwrap_or(1.0));
    let one = 1.0f64.to_bits();
    if [x, y, z].iter().all(|scale| scale.to_bits() == one) {
        fields.bb(3);
    } else if x.to_bits() == one {
        fields.bb(1);
        fields.dd(y, 1.0);
        fields.dd(z, 1.0);
    } else if y.to_bits() == x.to_bits() && z.to_bits() == x.to_bits() {
        fields.bb(2);
        fields.rd(x);
    } else {
        fields.bb(0);
        fields.rd(x);
        fields.dd(y, x);
        fields.dd(z, x);
    }
    match kept {
        Some(rotation) => fields.bd(rotation),
        None => fields.bd(real(object, 50)?.unwrap_or(0.0).to_radians()),
    }
    match [210, 220, 230].map(|code| real(object, code)) {
        [Some(None), Some(None), Some(None)] => [0.0, 0.0, 1.0].map(|v| fields.bd(v)),
        [Some(Some(x)), Some(Some(y)), Some(Some(z))] => [x, y, z].map(|v| fields.bd(v)),
        _ => return None,
    };
    fields.bit(small(object, 66)?.is_some());
    if grid {
        grid_fields(fields, object)?;
    }
    Some(())
}

/// MINSERT の列と行の数（70・71、なければ 1）と間隔（44・45、なければ 0）のフィールド
pub(super) fn grid_fields(fields: &mut BitWriter, object: &Object) -> Option<()> {
    for code in [70, 71] {
        fields.bs(small(object, code)?.unwrap_or(1));
    }
    for code in [44, 45] {
        fields.bd(real(object, code)?.unwrap_or(0.0));
    }
    Some(())
}

/// R2004 から後の MINSERT の型ごとのフィールド（`start` から `end` まで）。属性を持てば、属性が続くフラグの後、列と行の
/// フィールドの前に、持つオブジェクトの数（BL）が入る（ODS 5.4.1 の 20.4.9）。元のビットが `old` の値から書いたものと
/// 合うかを確かめ、持つオブジェクトの数と、列と行のフィールドの後ろは元のビットを写す。属性の有無は変えない
pub(super) fn r2004_minsert_fields(
    writer: fn() -> BitWriter,
    old: &Object,
    new: &Object,
    data: &[u8],
    [start, end]: [usize; 2],
    release: Release,
) -> Option<BitWriter> {
    let attribs = small(old, 66)?.is_some();
    (small(new, 66)?.is_some() == attribs).then_some(())?;
    let was = insert_rotation(data, start);
    let kept = (real(old, 50) == real(new, 50)).then_some(was).flatten();
    // 元のビットが、元の値から書いたものと合うか
    let matches = |from: usize, written: BitWriter| -> Option<usize> {
        let to = from.checked_add(written.len()).filter(|&to| to <= end)?;
        let mut found = BitWriter::default();
        found.copy(data, from..to)?;
        (found.into_bytes() == written.into_bytes()).then_some(to)
    };
    let mut head = BitWriter::default();
    insert(&mut head, old, false, was)?;
    let count_at = matches(start, head)?;
    let mut bits = Bits::with_release(data, release);
    bits.seek(count_at);
    if attribs {
        bits.bl()?;
    }
    let count_end = bits.pos();
    let mut grid = BitWriter::default();
    grid_fields(&mut grid, old)?;
    let grid_end = matches(count_end, grid)?;
    let mut fields = writer();
    insert(&mut fields, new, false, kept)?;
    fields.copy(data, count_at..count_end)?;
    grid_fields(&mut fields, new)?;
    fields.copy(data, grid_end..end)?;
    Some(fields)
}

/// TOLERANCE のフィールド（決定 0040）。X 軸の向き（11）がなければ (1, 0, 0)、押し出しの向きがなければ (0, 0, 1) を
/// 書く。文字（1）が変わっていなければ、元のビットを写す
pub(super) fn tolerance(
    fields: &mut BitWriter,
    old: &Object,
    new: &Object,
    data: &[u8],
    [start, end]: [usize; 2],
    release: Release,
) -> Option<()> {
    let mut bits = Bits::with_release(data, release);
    bits.seek(start);
    for _ in 0..9 {
        bits.bd()?;
    }
    let text_at = bits.pos();
    bits.text()?;
    let text_end = bits.pos();
    point3(fields, new, 10)?;
    point3_or(fields, new, 11, [1.0, 0.0, 0.0])?;
    point3_or(fields, new, 210, [0.0, 0.0, 1.0])?;
    // R2010 からは、文字列は文字列のストリームに書く（決定 0088）
    if text_of(old, 1) == text_of(new, 1) && release < Release::R2010 {
        fields.copy(data, text_at..text_end)?;
    } else {
        string(fields, new, 1)?;
    }
    fields.copy(data, text_end..end)
}

/// LEADER のフィールド（決定 0040・0185）。注釈の種類（73）・経路の種類（72、なければ 0）・頂点の数（76）と頂点（10）・
/// 押し出しの向き（なければ (0, 0, 1)）・X 軸の向き（211、なければ (1, 0, 0)）・注釈の高さと幅（40・41）・
/// 鉤の線の向き（74）・矢印のフラグ（71）を書く。頂点が変わったら、表にない終わりの点の射影は終わりの頂点にする。
/// 表にない鉤の線の点と差のフィールドと、後ろのフィールドは、元のビットを写す。R2010 から後は表が注釈の高さと幅を持たないので、
/// なければ元のビットを写す。`data` が空なら足す LEADER で、ずれ（212・213）を表から書き、後ろのフィールド（BS・B・B）
/// を 0 にする。矢印のフラグがなければ（寸法スタイルに従う）書けない
pub(super) fn leader(
    fields: &mut BitWriter,
    object: &Object,
    data: &[u8],
    [start, end]: [usize; 2],
    release: Release,
) -> Option<()> {
    let mut values = Cursor(
        object
            .values
            .iter()
            .filter(|value| matches!(value.code, 76 | 10 | 20 | 30))
            .collect::<Vec<_>>()
            .into_iter()
            .peekable(),
    );
    let mut vertices = Vec::new();
    for _ in 0..values.int(76)? {
        vertices.push([values.real(10)?, values.real(20)?, values.real(30)?]);
    }
    if values.0.next().is_some() || vertices.len() < 2 {
        return None;
    }
    let hook = match small(object, 74)?? {
        0 => false,
        1 => true,
        _ => return None,
    };
    let arrow = match small(object, 71)?? {
        0 => false,
        1 => true,
        _ => return None,
    };
    let write_vertices = |fields: &mut BitWriter| -> Option<()> {
        let last = *vertices.last()?;
        fields.bl(i32::try_from(vertices.len()).ok()?);
        for point in vertices.iter().chain([&last]) {
            for value in point {
                fields.bd(*value);
            }
        }
        Some(())
    };
    if data.is_empty() {
        // 鉤の線は持たない
        if small(object, 75)?.is_some_and(|v| v != 0) {
            return None;
        }
        fields.bit(false);
        fields.bs(small(object, 73)?.unwrap_or(3));
        fields.bs(small(object, 72)?.unwrap_or(0));
        write_vertices(fields)?;
        point3_or(fields, object, 210, [0.0, 0.0, 1.0])?;
        point3_or(fields, object, 211, [1.0, 0.0, 0.0])?;
        point3_or(fields, object, 212, [0.0; 3])?;
        point3_or(fields, object, 213, [0.0; 3])?;
        // 注釈の枠の高さと幅は、R2007 までだけが持つ。注記がなければ、AutoCAD が書く値と同じく 0 と 0 にする
        // （#345）
        if release < Release::R2010 {
            fields.bd(real(object, 40)?.unwrap_or(0.0));
            fields.bd(real(object, 41)?.unwrap_or(0.0));
        }
        fields.bit(hook);
        fields.bit(arrow);
        fields.bs(0);
        fields.bit(false);
        fields.bit(false);
        return Some(());
    }
    let mut bits = Bits::with_release(data, release);
    bits.seek(start);
    bits.bit()?;
    let kinds_at = bits.pos();
    bits.bs()?;
    bits.bs()?;
    let points_at = bits.pos();
    let count = bits.bl()?;
    let mut old = Vec::new();
    for _ in 0..count {
        old.push([bits.bd()?, bits.bd()?, bits.bd()?]);
    }
    for _ in 0..3 {
        bits.bd()?;
    }
    let points_end = bits.pos();
    for _ in 0..6 {
        bits.bd()?;
    }
    let offsets_at = bits.pos();
    let mut offsets = [0.0; 6];
    for slot in &mut offsets {
        *slot = bits.bd()?;
    }
    let offsets_end = bits.pos();
    if release < Release::R2010 {
        bits.bd()?;
        bits.bd()?;
    }
    bits.bit()?;
    bits.bit()?;
    let rest_at = bits.pos();
    fields.copy(data, start..kinds_at)?;
    fields.bs(small(object, 73)??);
    fields.bs(small(object, 72)?.unwrap_or(0));
    if vertices == old {
        fields.copy(data, points_at..points_end)?;
    } else {
        write_vertices(fields)?;
    }
    point3_or(fields, object, 210, [0.0, 0.0, 1.0])?;
    point3_or(fields, object, 211, [1.0, 0.0, 0.0])?;
    // ずれ（212・213）は、読む側が 0 のとき置かないので、表になければ 0 とする。元と同じなら元のビットを写す
    // （決定 0191）
    let mut written = BitWriter::default();
    point3_or(&mut written, object, 212, [0.0; 3])?;
    point3_or(&mut written, object, 213, [0.0; 3])?;
    let mut again = BitWriter::default();
    for value in offsets {
        again.bd(value);
    }
    if written.len() == again.len() && written.into_bytes() == again.into_bytes() {
        fields.copy(data, offsets_at..offsets_end)?;
    } else {
        point3_or(fields, object, 212, [0.0; 3])?;
        point3_or(fields, object, 213, [0.0; 3])?;
    }
    if release < Release::R2010 {
        fields.bd(real(object, 40)??);
        fields.bd(real(object, 41)??);
    }
    fields.bit(hook);
    fields.bit(arrow);
    fields.copy(data, rest_at..end)
}

/// MLINE のフィールド（決定 0040）。値を表の並びで読み、DWG の並びで書く。頂点と線とパラメーターの数は、表の値の数に
/// 合わなければ受け付けない
pub(super) fn mline(fields: &mut BitWriter, object: &Object) -> Option<()> {
    let mut values = Cursor(
        object
            .values
            .iter()
            .filter(|value| MLINE.contains(&value.code))
            .collect::<Vec<_>>()
            .into_iter()
            .peekable(),
    );
    let scale = values.real(40)?;
    let justification = u8::try_from(values.int(70)?).ok()?;
    let flags = i16::try_from(values.int(71)?).ok()?;
    let count = i16::try_from(values.int(72)?).ok()?;
    let lines = u8::try_from(values.int(73)?).ok()?;
    let point = |fields: &mut BitWriter, values: &mut Cursor<'_>, code| {
        for code in [code, code + 10, code + 20] {
            fields.bd(values.real(code)?);
        }
        Some(())
    };
    fields.bd(scale);
    fields.rc(justification);
    point(fields, &mut values, 10)?;
    point(fields, &mut values, 210)?;
    fields.bs(flags);
    fields.rc(lines);
    fields.bs(count);
    for _ in 0..count {
        for code in [11, 12, 13] {
            point(fields, &mut values, code)?;
        }
        for _ in 0..lines {
            for code in [74, 75] {
                let params = values.int(code)?;
                fields.bs(i16::try_from(params).ok()?);
                for _ in 0..params {
                    fields.bd(values.real(code - 33)?);
                }
            }
        }
    }
    values.0.next().is_none().then_some(())
}

/// VIEWPORT のフィールド（決定 0041）。表の値を DWG の並びで書き、角度（50・51）は度からラジアンに直す。表にない
/// 凍結した画層の数と後ろのフィールドは元のビットを写し、印刷スタイルの名前（1）が変わらなければ元のビットを写す
pub(super) fn viewport(
    fields: &mut BitWriter,
    old: &Object,
    new: &Object,
    data: &[u8],
    [start, end]: [usize; 2],
    release: Release,
) -> Option<()> {
    // R2010 からは、グリッドの向き（72）の後に主なグリッドの線の間隔（BS、61）を書き、文字列は文字列の
    // ストリームに書く（決定 0088）
    let r2010 = release >= Release::R2010;
    let mut bits = Bits::with_release(data, release);
    bits.seek(start);
    for _ in 0..17 {
        bits.bd()?;
    }
    for _ in 0..8 {
        bits.rd()?;
    }
    bits.bs()?;
    if r2010 {
        bits.bs()?;
    }
    let frozen_at = bits.pos();
    bits.bl()?;
    let frozen_end = bits.pos();
    bits.bl()?;
    let text_at = bits.pos();
    bits.text()?;
    let text_end = bits.pos();
    bits.rc()?;
    bits.bit()?;
    bits.bit()?;
    for _ in 0..10 {
        bits.bd()?;
    }
    bits.bs()?;
    let rest_at = bits.pos();
    point3(fields, new, 10)?;
    for code in [40, 41] {
        fields.bd(real(new, code)??);
    }
    point3(fields, new, 17)?;
    point3(fields, new, 16)?;
    fields.bd(real(new, 51)??.to_radians());
    for code in [45, 42, 43, 44] {
        fields.bd(real(new, code)??);
    }
    fields.bd(real(new, 50)??.to_radians());
    for code in [12, 13, 14, 15] {
        fields.rd(real(new, code)??);
        fields.rd(real(new, code + 10)??);
    }
    fields.bs(small(new, 72)??);
    if r2010 {
        fields.bs(small(new, 61)??);
    }
    fields.copy(data, frozen_at..frozen_end)?;
    fields.bl(i32::try_from(integer(new, 90)??).ok()?);
    if text_of(old, 1) == text_of(new, 1) && !r2010 {
        fields.copy(data, text_at..text_end)?;
    } else {
        string(fields, new, 1)?;
    }
    fields.rc(u8::try_from(small(new, 281)??).ok()?);
    for code in [74, 71] {
        fields.bit(match small(new, code)?? {
            0 => false,
            1 => true,
            _ => return None,
        });
    }
    for code in [110, 111, 112] {
        point3(fields, new, code)?;
    }
    fields.bd(real(new, 146)??);
    fields.bs(small(new, 79)??);
    fields.copy(data, rest_at..end)
}

/// SHAPE のフィールド（決定 0042）。角度（50・51）がなければ 0、幅の尺度（41）がなければ 1、厚さ（39）がなければ 0、
/// 押し出しの向きがなければ (0, 0, 1) を書く。角度は度からラジアンに直す。表にない形の番号と後ろのフィールドは、
/// 元のビットを写す
pub(super) fn shape(
    fields: &mut BitWriter,
    object: &Object,
    data: &[u8],
    [start, end]: [usize; 2],
    release: Release,
) -> Option<()> {
    let mut bits = Bits::with_release(data, release);
    bits.seek(start);
    for _ in 0..8 {
        bits.bd()?;
    }
    let number_at = bits.pos();
    bits.bs()?;
    let number_end = bits.pos();
    for _ in 0..3 {
        bits.bd()?;
    }
    let rest_at = bits.pos();
    point3(fields, object, 10)?;
    fields.bd(real(object, 40)??);
    fields.bd(real(object, 50)?.unwrap_or(0.0).to_radians());
    fields.bd(real(object, 41)?.unwrap_or(1.0));
    fields.bd(real(object, 51)?.unwrap_or(0.0).to_radians());
    fields.bd(real(object, 39)?.unwrap_or(0.0));
    fields.copy(data, number_at..number_end)?;
    point3_or(fields, object, 210, [0.0, 0.0, 1.0])?;
    fields.copy(data, rest_at..end)
}
