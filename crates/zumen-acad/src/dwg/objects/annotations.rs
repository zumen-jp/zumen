//! 文字・属性・公差・引出線・マルチライン・ビューポート・挿入・寸法の図形フィールドの読み取り。

use super::*;

/// TEXT のフィールド（ODS 5.4.1 の 20.4.3、R2000）。DataFlags のビットが立つフィールドは記録されず、既定の値なので
/// 置かない。高さ（40）は DD で書かれないのでいつも置く
pub(super) fn text(bits: &mut Bits) -> Option<Vec<Value>> {
    let flags = bits.rc()?;
    let absent = |bit: u8| flags & bit != 0;
    let elevation = if absent(0x01) { 0.0 } else { bits.rd()? };
    let (x, y) = (bits.rd()?, bits.rd()?);
    let mut values = vec![real(10, x), real(20, y), real(30, elevation)];
    let alignment = if absent(0x02) {
        None
    } else {
        Some((bits.dd(x)?, bits.dd(y)?))
    };
    // BE の押し出しの向きと、BT の厚さ。LINE などとは並びが逆
    let extrusion = if bits.bit()? {
        None
    } else {
        Some(point3(bits, 210)?)
    };
    if !bits.bit()? {
        values.push(real(39, bits.bd()?));
    }
    if !absent(0x04) {
        values.push(real(51, bits.rd()?.to_degrees()));
    }
    if !absent(0x08) {
        values.push(real(50, bits.rd()?.to_degrees()));
    }
    values.push(real(40, bits.rd()?));
    if !absent(0x10) {
        values.push(real(41, bits.rd()?));
    }
    values.push(text_value(1, &bits.text()?));
    for (bit, code) in [(0x20, 71), (0x40, 72), (0x80, 73)] {
        if !absent(bit) {
            values.push(int(code, i64::from(bits.bs()?)));
        }
    }
    if let Some((x, y)) = alignment {
        values.extend([real(11, x), real(21, y), real(31, elevation)]);
    }
    values.extend(extrusion.into_iter().flatten());
    Some(values)
}

/// ATTRIB・ATTDEF のフィールド（ODS 5.4.1 の 20.4.4・20.4.5、R2000）。TEXT のフィールドに続けて、タグ（2）・フィールドの長さ
/// （73）・フラグ（70）を、ATTDEF は問い（3）も置く。DXF は縦の合わせを 74 に書くので、TEXT の 73 を 74 に
/// 移す。フィールドの長さは 0 でなければ置く
pub(super) fn attribute(bits: &mut Bits, definition: bool) -> Option<Vec<Value>> {
    let mut values = text(bits)?;
    for value in &mut values {
        if value.code == 73 {
            value.code = 74;
        }
    }
    // R2010 からは、名前の前に属性のバージョン（RC、DXF の 280）が、フラグの後に位置を固定するか（B、DXF の
    // 2 つ目の 280）が、定義の問いの前にもう 1 つのバージョン（RC）がある
    let r2010 = bits.release() >= Release::R2010;
    if r2010 {
        values.push(int(280, i64::from(bits.rc()?)));
    }
    // R2018 からは、バージョンの後に属性の種類（RC、DXF の 71）が続く。1 は 1 行の属性で、2（ATTRIB）と
    // 4（ATTDEF）は複数行の属性。複数行の属性は、意味の分からない 37 ビット（図面の 2 つの属性で
    // 同じ値）の後に、埋め込みの MTEXT のフィールドと、DXF の 72 にあたる BS を持つ（図面で確かめた）。
    // DXF と同じく、属性の値の後に、種類（71）と 72 と、埋め込みのオブジェクトのマーカー（101）と MTEXT の値を
    // 置く（決定 0102）
    let mut embedded = Vec::new();
    if bits.release() >= Release::R2018 {
        let kind = bits.rc()?;
        if kind != 1 {
            bits.seek(bits.pos() + 37);
            let mtext = mtext(bits)?;
            mtext_columns(bits)?;
            embedded.push(int(71, i64::from(kind)));
            embedded.push(int(72, i64::from(bits.bs()?)));
            embedded.push(text_value(101, b"Embedded Object"));
            embedded.extend(mtext);
        }
    }
    values.push(text_value(2, &bits.text()?));
    let length = bits.bs()?;
    if length != 0 {
        values.push(int(73, i64::from(length)));
    }
    values.push(int(70, i64::from(bits.rc()?)));
    if r2010 {
        values.push(int(280, i64::from(bits.bit()?)));
    }
    if definition {
        if r2010 {
            bits.rc()?;
        }
        values.push(text_value(3, &bits.text()?));
    }
    values.extend(embedded);
    Some(values)
}

/// TOLERANCE のフィールド（ODS 5.4.1 の 20.4.49、R2000）。挿入点（10）と文字（1）を置き、X 軸の向き（11）と
/// 押し出しの向きは既定の値でなければ置く
pub(super) fn tolerance(bits: &mut Bits) -> Option<Vec<Value>> {
    let mut values = point3(bits, 10)?;
    let direction = point3(bits, 11)?;
    let extrusion = point3(bits, 210)?;
    values.push(text_value(1, &bits.text()?));
    if reals(&direction) != [Some(1.0), Some(0.0), Some(0.0)] {
        values.extend(direction);
    }
    if !is_up(&extrusion) {
        values.extend(extrusion);
    }
    Some(values)
}

/// LEADER のフィールド（ODS 5.4.1 の 20.4.47、R2000）。注釈の種類（73）・注釈の高さと幅（40・41）・鉤の線の
/// 向き（74）・矢印のフラグ（71）・頂点の数（76）と頂点（10）を置き、押し出しの向き（210）と X 軸の向き（211）は既定の値で
/// なければ置く。DXF は寸法スタイルから鉤の線の点を足して書くことがあり、そのときは頂点が 1 つ多い
pub(in crate::dwg) fn leader(bits: &mut Bits) -> Option<Vec<Value>> {
    bits.bit()?;
    let annotation = bits.bs()?;
    let path = bits.bs()?;
    let count = bits.bl()?;
    let mut vertices = Vec::new();
    for _ in 0..count {
        vertices.extend(point3(bits, 10)?);
    }
    // 終わりの点の射影。DXF には書かれない
    point3(bits, 10)?;
    let extrusion = point3(bits, 210)?;
    let direction = point3(bits, 211)?;
    let mut values = vec![int(73, i64::from(annotation))];
    if path != 0 {
        values.push(int(72, i64::from(path)));
    }
    // 鉤の線の点の差と、注釈への差。どちらも DXF で確かめていない。0 でなければ、DXF と同じ 212・213 に置く
    // （決定 0191）
    let offsets = [point3(bits, 212)?, point3(bits, 213)?];
    // 注釈の枠の高さと幅。R2007 までだけが持つ（AutoCAD 2027 が書いた R2010・R2018 の
    // DWG で、このフィールドがないことを確かめた、#345）
    if bits.release() < Release::R2010 {
        let (height, width) = (bits.bd()?, bits.bd()?);
        values.push(real(40, height));
        values.push(real(41, width));
    }
    values.push(int(74, i64::from(bits.bit()?)));
    // 矢印のフラグ。DWG の LEADER は必ず持つので、DXF と同じく 71 に置く（決定 0185）
    values.push(int(71, i64::from(bits.bit()?)));
    values.push(int(76, i64::from(count)));
    values.extend(vertices);
    if reals(&direction) != [Some(1.0), Some(0.0), Some(0.0)] {
        values.extend(direction);
    }
    for offset in offsets {
        if reals(&offset) != [Some(0.0); 3] {
            values.extend(offset);
        }
    }
    if !is_up(&extrusion) {
        values.extend(extrusion);
    }
    Some(values)
}

/// MLINE のフィールド（ODS 5.4.1 の 20.4.50、R2000）。尺度（40）・合わせ（70）・開閉のフラグ（71）・頂点の数（72）・
/// 線の数（73）・基点（10）・押し出しの向き（210）を置き、頂点ごとに点（11）・向き（12）・留めの向き
/// （13）と、線ごとの区切り（74・41）と塗りの区切り（75・42）を置く
pub(super) fn mline(bits: &mut Bits) -> Option<Vec<Value>> {
    let mut values = vec![real(40, bits.bd()?)];
    let justification = bits.rc()?;
    let base = point3(bits, 10)?;
    let extrusion = point3(bits, 210)?;
    let flags = bits.bs()?;
    let lines = bits.rc()?;
    let count = bits.bs()?;
    values.extend([
        int(70, i64::from(justification)),
        int(71, i64::from(flags)),
        int(72, i64::from(count)),
        int(73, i64::from(lines)),
    ]);
    values.extend(base);
    values.extend(extrusion);
    for _ in 0..count {
        values.extend(point3(bits, 11)?);
        values.extend(point3(bits, 12)?);
        values.extend(point3(bits, 13)?);
        for _ in 0..lines {
            for code in [74, 75] {
                let params = bits.bs()?;
                values.push(int(code, i64::from(params)));
                for _ in 0..params {
                    values.push(real(code - 33, bits.bd()?));
                }
            }
        }
    }
    Some(values)
}

/// VIEWPORT のフィールド（ODS 5.4.1 の 20.4.38、R2000）。仕様のフィールドを、DXF の並び（中心・大きさ・ビューの中心・
/// スナップ・グリッド・ビューの向きと目標・ビュー・状態・UCS）で置く。角度は度に直す。ビューポートの
/// 番号（69）と状態（68）は DWG に記録されていないので置かない。凍結した画層の数を `frozen` に返す
pub(super) fn viewport(bits: &mut Bits, frozen: &mut u32) -> Option<Vec<Value>> {
    let mut values = point3(bits, 10)?;
    values.push(real(40, bits.bd()?));
    values.push(real(41, bits.bd()?));
    let target = point3(bits, 17)?;
    let direction = point3(bits, 16)?;
    let twist = bits.bd()?;
    let height = bits.bd()?;
    let lens = bits.bd()?;
    let front = bits.bd()?;
    let back = bits.bd()?;
    let snap = bits.bd()?;
    for code in [12, 13, 14, 15] {
        values.push(real(code, bits.rd()?));
        values.push(real(code + 10, bits.rd()?));
    }
    values.extend(direction);
    values.extend(target);
    values.extend([
        real(42, lens),
        real(43, front),
        real(44, back),
        real(45, height),
        real(50, snap.to_degrees()),
        real(51, twist.to_degrees()),
        int(72, i64::from(bits.bs()?)),
    ]);
    // R2007 からは、主なグリッドの線の間隔（BS、DXF の 61）が続く
    if bits.release() >= Release::R2007 {
        values.push(int(61, i64::from(bits.bs()?)));
    }
    *frozen = u32::try_from(bits.bl()?).ok()?;
    values.push(int(90, i64::from(bits.bl()?)));
    values.push(text_value(1, &bits.text()?));
    values.push(int(281, i64::from(bits.rc()?)));
    let at_origin = bits.bit()?;
    let per_viewport = bits.bit()?;
    values.extend([
        int(71, i64::from(per_viewport)),
        int(74, i64::from(at_origin)),
    ]);
    for code in [110, 111, 112] {
        values.extend(point3(bits, code)?);
    }
    let elevation = bits.bd()?;
    values.push(int(79, i64::from(bits.bs()?)));
    values.push(real(146, elevation));
    Some(values)
}

/// INSERT のフィールド（ODS 5.4.1 の 20.4.9、R2000）。挿入点はいつも置き、尺度（41・42・43）・回転（50）・
/// 押し出しの向き（210・220・230）は既定の値でなければ、属性が続くこと（66）は続くときだけ置く。
/// MINSERT（`grid`）は、列と行の数（70・71）と間隔（44・45）も、既定の値（1・1・0・0）でなければ置く
pub(super) fn insert(bits: &mut Bits, grid: bool) -> Option<Vec<Value>> {
    let mut values = point3(bits, 10)?;
    let (x, y, z) = match bits.bb()? {
        3 => (1.0, 1.0, 1.0),
        1 => (1.0, bits.dd(1.0)?, bits.dd(1.0)?),
        2 => {
            let x = bits.rd()?;
            (x, x, x)
        }
        _ => {
            let x = bits.rd()?;
            (x, bits.dd(x)?, bits.dd(x)?)
        }
    };
    for (code, scale) in [(41, x), (42, y), (43, z)] {
        if scale != 1.0 {
            values.push(real(code, scale));
        }
    }
    let rotation = bits.bd()?;
    if rotation != 0.0 {
        values.push(real(50, rotation.to_degrees()));
    }
    let extrusion = point3(bits, 210)?;
    if !is_up(&extrusion) {
        values.extend(extrusion);
    }
    let attribs = bits.bit()?;
    if attribs {
        values.push(int(66, 1));
    }
    if grid {
        // R2004 からは、属性を持つときだけ、列と行のフィールドの前に持つオブジェクトの数（BL）が入る（ODS 5.4.1 の
        // 20.4.9）。INSERT ではフィールドの終わりに入るので読まない
        if attribs && bits.release() >= Release::R2004 {
            bits.bl()?;
        }
        for code in [70, 71] {
            let count = bits.bs()?;
            if count != 1 {
                values.push(int(code, i64::from(count)));
            }
        }
        for code in [44, 45] {
            let spacing = bits.bd()?;
            if spacing != 0.0 {
                values.push(real(code, spacing));
            }
        }
    }
    Some(values)
}

/// MTEXT のフィールド（ODS 5.4.1 の 20.4.46、R2000）。文字列は DXF と同じく、250 バイトずつの 3 と、残りの 1 に
/// 分けて置く。X 軸の向き（11・21・31）と押し出しの向きは既定の値でなければ置く。文字の範囲の大きさは
/// DXF にないので置かない
pub(super) fn mtext(bits: &mut Bits) -> Option<Vec<Value>> {
    let mut values = point3(bits, 10)?;
    let extrusion = point3(bits, 210)?;
    let x_axis = point3(bits, 11)?;
    values.push(real(41, bits.bd()?));
    // R2007 からは、枠の高さ（46）が続く
    if bits.release() >= Release::R2007 {
        values.push(real(46, bits.bd()?));
    }
    values.push(real(40, bits.bd()?));
    values.push(int(71, i64::from(bits.bs()?)));
    values.push(int(72, i64::from(bits.bs()?)));
    bits.bd()?;
    bits.bd()?;
    // DXF は、制御文字を ^ の形に直す前の文字列を 250 バイトずつに分ける
    let text = bits.text()?;
    let mut rest = text.as_slice();
    while rest.len() > 250 {
        let (chunk, tail) = rest.split_at(250);
        values.push(Value {
            code: 3,
            data: Data::Text(caret(chunk)),
        });
        rest = tail;
    }
    values.push(Value {
        code: 1,
        data: Data::Text(caret(rest)),
    });
    values.push(int(73, i64::from(bits.bs()?)));
    values.push(real(44, bits.bd()?));
    bits.bit()?;
    // R2004 からは背景のフィールドが続く。フラグ（90）の 1 は、背景の大きさの倍率（45）・色（63）・透明度（441）が
    // 続くことを表す。DXF と同じく、フラグが 0 でなければ置く
    if bits.release() >= Release::R2004 {
        let background = bits.bl()?;
        if background != 0 {
            values.push(int(90, i64::from(background)));
        }
        if background & 1 != 0 {
            values.push(real(45, bits.bd()?));
            // 真の色は、図形と同じく 421 だけを置く
            let color = bits.cmc()?;
            match color.rgb {
                Some(rgb) => values.push(int(421, i64::from(rgb))),
                None => values.push(int(63, i64::from(color.index))),
            }
            values.push(int(441, i64::from(bits.bl()?)));
        }
    }
    if reals(&x_axis) != [Some(1.0), Some(0.0), Some(0.0)] {
        values.extend(x_axis);
    }
    if !is_up(&extrusion) {
        values.extend(extrusion);
    }
    Some(values)
}

/// R2018 からの MTEXT の、背景のフィールドの後のフィールドを読み飛ばす。注釈でないか（B）・バージョン（BS）・既定か（B）・
/// 寄せ方（BL）・X 軸の向き・挿入点・幅と高さと範囲の幅と高さ（BD）・段組みの種類（BL）の順（図面で
/// 確かめた）。段組みのあるフィールドは、まだ確かめていないので読まない
pub(super) fn mtext_columns(bits: &mut Bits) -> Option<()> {
    bits.bit()?;
    bits.bs()?;
    bits.bit()?;
    bits.bl()?;
    point3(bits, 10)?;
    point3(bits, 11)?;
    for _ in 0..4 {
        bits.bd()?;
    }
    (bits.bl()? == 0).then_some(())
}

/// R2018 からの MTEXT の、背景のフィールドの後のフィールド。並びは [`mtext_columns`] と同じで、寄せ方（BL）は DXF の 70、
/// 2 つの点は 10・11、4 つの BD は 40 から 43 にあたる。段組みの種類（BL、DXF の
/// 71）が 0 でなければ、高さの数（BL、72）・段の幅（BD、44）・段の間隔（BD、45）・高さを自動で決めるか
/// （B、73）・逆に流すか（B、74）と、自動でない手動の段組み（種類 2）なら段ごとの高さ（BD、46）が続く
/// （図面で確かめた）。DXF と同じく、段組みがあれば埋め込みのオブジェクトのマーカー（101）の後に置き、なければ
/// 何も置かない。段ごとの高さは、同じ図面の DXF が 0.0 と書く所に負の値を持つことがあるが、DWG の値を
/// そのまま置く（docs/formats/dwg.md）
pub(super) fn mtext_embedded(bits: &mut Bits) -> Option<Vec<Value>> {
    bits.bit()?;
    bits.bs()?;
    bits.bit()?;
    let attachment = bits.bl()?;
    let mut values = vec![
        text_value(101, b"Embedded Object"),
        int(70, i64::from(attachment)),
    ];
    values.extend(point3(bits, 10)?);
    values.extend(point3(bits, 11)?);
    for code in 40..=43 {
        values.push(real(code, bits.bd()?));
    }
    let kind = bits.bl()?;
    if kind == 0 {
        return Some(Vec::new());
    }
    values.push(int(71, i64::from(kind)));
    let count = bits.bl()?;
    values.push(int(72, i64::from(count)));
    values.push(real(44, bits.bd()?));
    values.push(real(45, bits.bd()?));
    let automatic = bits.bit()?;
    values.push(int(73, i64::from(automatic)));
    values.push(int(74, i64::from(bits.bit()?)));
    if !automatic && kind == 2 {
        for _ in 0..count {
            values.push(real(46, bits.bd()?));
        }
    }
    Some(values)
}

/// ARC_DIMENSION を、DIMENSION の型の番号と重ならない仮の番号で分ける
pub(super) const ARC_DIMENSION: u16 = u16::MAX;

/// DIMENSION のフィールド（ODS 5.4.1 の 20.4.22〜20.4.29、R2000）と、それに続く ARC_DIMENSION のフィールド。DXF の 70 は、型で決まる寸法の種類に、
/// 共通のフラグの 1 が 0 なら 128、2 なら 32、座標寸法のフラグの 1 なら 64 を足す。文字の中点（11）と
/// 12 の点の Z は標高とし、12 の点は 0 でなければ置く。ブロックの挿入の尺度と回転は、DXF の 41・42 が
/// 別のフィールドに使われるので置かない
pub(super) fn dimension(bits: &mut Bits, type_code: u16) -> Option<Vec<Value>> {
    // R2010 からは、寸法のバージョン（RC、DXF の 280）が先にある
    let r2010 = bits.release() >= Release::R2010;
    let version = if r2010 { Some(bits.rc()?) } else { None };
    let extrusion = point3(bits, 210)?;
    let text_x = bits.rd()?;
    let text_y = bits.rd()?;
    let elevation = bits.bd()?;
    let flags = bits.rc()?;
    let user_text = bits.text()?;
    let text_rotation = bits.bd()?;
    let horizontal = bits.bd()?;
    for _ in 0..4 {
        bits.bd()?;
    }
    let attachment = bits.bs()?;
    let spacing_style = bits.bs()?;
    let spacing = bits.bd()?;
    let measurement = bits.bd()?;
    // R2007 からは、3 つの B（DXF の 73 と、矢印を裏返すかの 74・75）が続く
    let arrows = if bits.release() >= Release::R2007 {
        Some([bits.bit()?, bits.bit()?, bits.bit()?])
    } else {
        None
    };
    let point12 = (bits.rd()?, bits.rd()?);
    let mut arc = Vec::new();
    let (kind, mut values) = match type_code {
        DIMENSION_ORDINATE => {
            let mut values = point3(bits, 10)?;
            values.extend(point3(bits, 13)?);
            values.extend(point3(bits, 14)?);
            let kind = if bits.rc()? & 1 != 0 { 6 | 64 } else { 6 };
            (kind, values)
        }
        DIMENSION_LINEAR | DIMENSION_ALIGNED => {
            let mut values = point3(bits, 13)?;
            values.extend(point3(bits, 14)?);
            values.extend(point3(bits, 10)?);
            let oblique = bits.bd()?.to_degrees();
            if oblique != 0.0 {
                values.push(real(52, oblique));
            }
            if type_code == DIMENSION_LINEAR {
                let rotation = bits.bd()?.to_degrees();
                if rotation != 0.0 {
                    values.push(real(50, rotation));
                }
                (0, values)
            } else {
                (1, values)
            }
        }
        DIMENSION_ANGULAR_3PT => {
            let mut values = point3(bits, 10)?;
            for code in [13, 14, 15] {
                values.extend(point3(bits, code)?);
            }
            (5, values)
        }
        DIMENSION_ANGULAR_2LINE => {
            let mut values = point2(bits, 16)?;
            values.push(real(36, elevation));
            for code in [13, 14, 15, 10] {
                values.extend(point3(bits, code)?);
            }
            (2, values)
        }
        DIMENSION_RADIUS => {
            let mut values = point3(bits, 10)?;
            values.extend(point3(bits, 15)?);
            values.push(real(40, bits.bd()?));
            (4, values)
        }
        // 弧長寸法のフィールドは、DXF の AcDbArcDimension と同じ順に並ぶ（推定。AutoCAD で作ったサンプルの DWG と DXF の
        // 組で確かめた）。70・71 が共通のフィールドと重なるので、共通のフィールドより後に、13 から置く。種類は DXF と同じく
        // 3 点の角度の 5 とする
        ARC_DIMENSION => {
            let values = point3(bits, 10)?;
            for code in [13, 14, 15] {
                arc.extend(point3(bits, code)?);
            }
            arc.push(int(70, i64::from(bits.bit()?)));
            arc.push(real(40, bits.bd()?));
            arc.push(real(41, bits.bd()?));
            arc.push(int(71, i64::from(bits.bit()?)));
            arc.extend(point3(bits, 16)?);
            arc.extend(point3(bits, 17)?);
            (5, values)
        }
        _ => {
            let mut values = point3(bits, 15)?;
            values.extend(point3(bits, 10)?);
            values.push(real(40, bits.bd()?));
            (3, values)
        }
    };
    let user_location = if flags & 1 == 0 { 128 } else { 0 };
    let block_only = if flags & 2 != 0 { 32 } else { 0 };
    values.extend([
        real(11, text_x),
        real(21, text_y),
        real(31, elevation),
        int(70, kind | user_location | block_only),
        int(71, i64::from(attachment)),
        real(42, measurement),
    ]);
    if let Some(version) = version {
        values.push(int(280, i64::from(version)));
    }
    if let Some(arrows) = arrows {
        for (code, flag) in [73, 74, 75].into_iter().zip(arrows) {
            values.push(int(code, i64::from(flag)));
        }
    }
    if point12 != (0.0, 0.0) {
        values.extend([
            real(12, point12.0),
            real(22, point12.1),
            real(32, elevation),
        ]);
    }
    if spacing_style != 1 {
        values.push(int(72, i64::from(spacing_style)));
    }
    if spacing != 1.0 {
        values.push(real(41, spacing));
    }
    if !user_text.is_empty() {
        values.push(text_value(1, &user_text));
    }
    for (code, angle) in [(53, text_rotation), (51, horizontal)] {
        if angle != 0.0 {
            values.push(real(code, angle.to_degrees()));
        }
    }
    if !is_up(&extrusion) {
        values.extend(extrusion);
    }
    values.extend(arc);
    Some(values)
}
