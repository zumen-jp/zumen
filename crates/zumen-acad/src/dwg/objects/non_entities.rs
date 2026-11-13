//! 図形でないオブジェクト（MLINESTYLE・TABLESTYLE・DICTIONARY・XRECORD・GROUP・SCALE・IMAGEDEF・SPATIAL_FILTER・LAYOUT）のフィールドの読み取り。

use super::*;

/// MLINESTYLE のフィールド（ODS 5.4.1 の 20.4.73、R2000）。名前（2）・フラグ（70）・説明（3）・塗りの色（62）・
/// 始めと終わりの角度（51・52、度に直す）・線の数（71）と、線ごとの位置（49）と色（62）を置く。フラグは、
/// DWG と DXF でビットの並びが違うので並べ直す。線種は番号で記録されているので、まだ置かない
/// R2004 までの TABLESTYLE（サンプルの DWG の値と DXF で確かめた）。名前（3）・向き（70）・フラグ（71）・余白（40・41）・
/// 表題と見出しを出さないフラグ（280・281）の後に、データ・表題・見出しの行ごとに、文字スタイル（7。ハンドルの
/// ストリームで埋める仮の値）・高さ（140）・位置合わせ（170）・文字の色（62）・背景の色（63）・背景を塗るフラグ（283）と、
/// 6 つの枠の線幅（274〜279）・表示（284〜289）・色（64〜69）を、DXF と同じ
/// 並びで置く。色は、ACAD_TABLE と同じく R2000 でも R2004 の形の CMC で持つ
pub(super) fn tablestyle(bits: &mut Bits) -> Option<Vec<Value>> {
    let mut values = vec![
        text_value(3, &bits.text()?),
        int(70, i64::from(bits.bs()?)),
        int(71, i64::from(bits.bs()?)),
        real(40, bits.bd()?),
        real(41, bits.bd()?),
        int(280, i64::from(bits.bit()?)),
        int(281, i64::from(bits.bit()?)),
    ];
    for _ in 0..3 {
        values.push(Value {
            code: 7,
            data: Data::Handle(SLOT),
        });
        values.push(real(140, bits.bd()?));
        values.push(int(170, i64::from(bits.bs()?)));
        values.push(int(62, i64::from(bits.cmc_r2004()?.index)));
        values.push(int(63, i64::from(bits.cmc_r2004()?.index)));
        values.push(int(283, i64::from(bits.bit()?)));
        let mut weights = Vec::new();
        let mut shown = Vec::new();
        let mut colors = Vec::new();
        for edge in 0..6 {
            weights.push(int(274 + edge, i64::from(bits.bs()?)));
            shown.push(int(284 + edge, i64::from(bits.bit()?)));
            colors.push(int(64 + edge, i64::from(bits.cmc_r2004()?.index)));
        }
        values.extend(weights);
        values.extend(shown);
        values.extend(colors);
    }
    Some(values)
}

pub(super) fn mlinestyle(bits: &mut Bits) -> Option<Vec<Value>> {
    let name = bits.text()?;
    let description = bits.text()?;
    let flags = bits.bs()?;
    // DWG のビットと、DXF のビット
    let moved = [
        (1, 2),
        (2, 1),
        (16, 16),
        (32, 64),
        (64, 32),
        (256, 256),
        (512, 1024),
        (1024, 512),
    ];
    let flags = moved
        .iter()
        .filter(|(dwg, _)| flags & dwg != 0)
        .fold(0, |sum, (_, dxf)| sum | dxf);
    let mut values = vec![
        Value {
            code: 2,
            data: Data::Text(name),
        },
        int(70, i64::from(flags)),
        text_value(3, &description),
        int(62, i64::from(bits.cmc()?.index)),
        real(51, bits.bd()?.to_degrees()),
        real(52, bits.bd()?.to_degrees()),
    ];
    let lines = bits.rc()?;
    values.push(int(71, i64::from(lines)));
    for _ in 0..lines {
        values.push(real(49, bits.bd()?));
        values.push(int(62, i64::from(bits.cmc()?.index)));
        // R2018 からは、線種を番号でなくハンドルで指す
        if bits.release() < Release::R2018 {
            bits.bs()?;
        }
    }
    Some(values)
}

/// DICTIONARY のフィールド（ODS 5.4.1 の 20.4.44、R2000）。所有するフラグ（280）は 1 なら、複製の扱い（281）は
/// いつも置く。項目の名前は `entries` に集め、ハンドルの並びの項目と組にして後で置く
pub(super) fn dictionary(bits: &mut Bits, entries: &mut Vec<Vec<u8>>) -> Option<Vec<Value>> {
    let count = usize::try_from(bits.bl()?).unwrap_or(0);
    // 項目は、文字列のストリームがなく名前が空でも、1 バイト以上のハンドルを持つ（ODS 5.4.1 の 2.13）。
    // 名前を読むだけでは入力が進まないので、データにない項目の数を、確保する前に拒む（#582）
    if count > bits.size() {
        return None;
    }
    let cloning = bits.bs()?;
    let hard_owner = bits.rc()?;
    let mut values = Vec::new();
    if hard_owner != 0 {
        values.push(int(280, i64::from(hard_owner)));
    }
    values.push(int(281, i64::from(cloning)));
    for _ in 0..count {
        entries.push(bits.text()?);
    }
    Some(values)
}

/// XRECORD のフィールド（ODS 5.4.1 の 20.4.89、R2000。文字列は R2007 から UTF-16LE）。複製の扱い（280）を DXF と同じく先に置き、
/// 続けてデータの値を置く。データは、グループ・コード（RS）と、コードの型で決まる値の並びで、
/// 記録されたバイト数をちょうど読み切らなければ読めないとする
pub(super) fn xrecord(bits: &mut Bits) -> Option<Vec<Value>> {
    let len = usize::try_from(bits.bl()?).ok()?;
    let end = bits.pos().checked_add(len.checked_mul(8)?)?;
    let mut data = Vec::new();
    while bits.pos() < end {
        let code = bits.rs()?;
        match DataType::of(code) {
            // 点は、X のコードに 3RD で記録され、DXF では Y と Z が別のコードになる
            DataType::Real if matches!(code, 10..=19 | 1010..=1019) => {
                data.extend([
                    real(code, bits.rd()?),
                    real(code + 10, bits.rd()?),
                    real(code + 20, bits.rd()?),
                ]);
            }
            DataType::Real => data.push(real(code, bits.rd()?)),
            DataType::Int => {
                let value = match code {
                    280..=289 => i64::from(bits.rc()?),
                    90..=99 | 420..=429 | 440..=459 | 1071 => i64::from(bits.rl()?),
                    160..=169 => i64::from_le_bytes(raw_bytes(bits, 8)?.try_into().ok()?),
                    _ => i64::from(bits.rs()?),
                };
                data.push(int(code, value));
            }
            DataType::Bool => data.push(Value {
                code,
                data: Data::Bool(bits.rc()? != 0),
            }),
            DataType::Handle => data.push(Value {
                code,
                data: Data::Handle(u64::from_le_bytes(raw_bytes(bits, 8)?.try_into().ok()?)),
            }),
            DataType::Binary => {
                let len = usize::from(bits.rc()?);
                data.push(Value {
                    code,
                    data: Data::Binary(raw_bytes(bits, len)?),
                });
            }
            // R2007 からは、RS の文字の数と UTF-16LE。コード・ページを持たない
            DataType::Text if bits.release() >= Release::R2007 => {
                let len = usize::try_from(bits.rs()?).ok()?;
                let units: Vec<u16> = raw_bytes(bits, len.checked_mul(2)?)?
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|unit| u16::from_le_bytes(*unit))
                    .collect();
                data.push(text_value(
                    code,
                    String::from_utf16_lossy(&units).as_bytes(),
                ));
            }
            DataType::Text => {
                let len = usize::try_from(bits.rs()?).ok()?;
                // コード・ページ
                bits.rc()?;
                data.push(text_value(code, &raw_bytes(bits, len)?));
            }
        }
    }
    if bits.pos() != end {
        return None;
    }
    let mut values = vec![int(280, i64::from(bits.bs()?))];
    values.extend(data);
    Some(values)
}

/// GROUP のフィールド（ODS 5.4.1 の GROUP の節、R2000）。説明（300）・名前のないフラグ（70）・選べるフラグ（71）を置く。
/// 図形の数だけ、ハンドルの並びの図形（340）を後で置く
pub(super) fn group(bits: &mut Bits, handles: &mut Vec<i16>) -> Option<Vec<Value>> {
    let values = vec![
        text_value(300, &bits.text()?),
        int(70, i64::from(bits.bs()?)),
        int(71, i64::from(bits.bs()?)),
    ];
    placeholders(bits, handles, 340)?;
    Some(values)
}

/// BL の数を読み、ハンドルの並びの終わりに、その数だけ `code` を積む。参照のハンドルは 1 つで 1 バイト以上
/// （ODS 5.4.1 の 2.13）なので、データのバイト数を超える数は壊れているとして読まない（#501。数だけ積むと、
/// 数 GB を取った）。負の数は 0 個とする
pub(super) fn placeholders(bits: &mut Bits, handles: &mut Vec<i16>, code: i16) -> Option<()> {
    let count = usize::try_from(bits.bl()?).unwrap_or(0);
    if count > bits.size() {
        return None;
    }
    handles.extend(std::iter::repeat_n(code, count));
    Some(())
}

/// SCALE のフィールド（クラスで決まる型）。名前（300）・紙の単位（140）・図面の単位（141）・単位の尺度のフラグ（290）
pub(super) fn scale(bits: &mut Bits) -> Option<Vec<Value>> {
    Some(vec![
        int(70, i64::from(bits.bs()?)),
        text_value(300, &bits.text()?),
        real(140, bits.bd()?),
        real(141, bits.bd()?),
        Value {
            code: 290,
            data: Data::Bool(bits.bit()?),
        },
    ])
}

/// IMAGEDEF のフィールド（ODS 5.4.1 の 20.4.81。クラスで決まる型）。DXF の順に、バージョン（90）・ファイルの場所（1）・
/// 画素の数（10）・1 画素の大きさ（11）・読み込んだフラグ（280）・解像度の単位（281）を置く
pub(super) fn imagedef(bits: &mut Bits) -> Option<Vec<Value>> {
    let version = int(90, i64::from(bits.bl()?));
    let size = point2(bits, 10)?;
    let path = text_value(1, &bits.text()?);
    let loaded = int(280, i64::from(bits.bit()?));
    let units = int(281, i64::from(bits.rc()?));
    let pixel = point2(bits, 11)?;
    let mut values = vec![version, path];
    values.extend(size);
    values.extend(pixel);
    values.extend([loaded, units]);
    Some(values)
}

/// SPATIAL_FILTER のフィールド（ODS 5.4.1 の SPATIAL_FILTER の節、決定 0149）。DXF と同じく、境界の点の数（70）・点（10・20）・
/// 押し出しの向き（210〜230）・境界の原点（11〜31）・境界を表示するか（71）・前と後ろで切るか（72・73）と、
/// 切るならその距離（40・41）、挿入の逆の変換と境界の変換の 12 ずつの値（40）の順に置く
pub(super) fn spatial_filter(bits: &mut Bits) -> Option<Vec<Value>> {
    let count = bits.bs()?;
    let mut values = vec![int(70, i64::from(count))];
    for _ in 0..count {
        values.extend(point2(bits, 10)?);
    }
    values.extend(point3(bits, 210)?);
    values.extend(point3(bits, 11)?);
    values.push(int(71, i64::from(bits.bs()?)));
    for (flag, distance) in [(72, 40), (73, 41)] {
        let on = bits.bs()?;
        values.push(int(flag, i64::from(on)));
        if on != 0 {
            values.push(real(distance, bits.bd()?));
        }
    }
    for _ in 0..24 {
        values.push(real(40, bits.bd()?));
    }
    Some(values)
}

/// LAYOUT のフィールド（ODS 5.4.1 の LAYOUT の節）。印刷の設定のフィールドに続けて、レイアウトのフィールドを置く。R2004 からは、
/// 印刷の設定に陰影の印刷のフィールド（76〜78）が加わり、レイアウトのフィールドの後にビューポートの数が続く（決定 0082）
pub(super) fn layout(
    bits: &mut Bits,
    release: Release,
    handles: &mut Vec<i16>,
) -> Option<Vec<Value>> {
    let r2004 = release >= Release::R2004;
    // R2004 からは、印刷するビューの名前（6）の代わりに、そのハンドルを持つ。値には置かない
    if r2004 {
        handles.push(0);
    }
    // R2007 からは、表示スタイルのハンドルも持つ
    if release >= Release::R2007 {
        handles.push(0);
    }
    // 紙空間のブロックのレコード・最後に使ったビューポート・基準の UCS・名前の UCS
    handles.extend([330, 331, 346, 345]);
    let mut values = vec![
        text_value(1, &bits.text()?),
        text_value(2, &bits.text()?),
        int(70, i64::from(bits.bs()?)),
    ];
    // 余白（左・下・右・上）と紙の幅・高さ
    for code in 40..=45 {
        values.push(real(code, bits.bd()?));
    }
    values.push(text_value(4, &bits.text()?));
    values.extend([real(46, bits.bd()?), real(47, bits.bd()?)]);
    for code in [72, 73, 74] {
        values.push(int(code, i64::from(bits.bs()?)));
    }
    // 印刷する窓の隅
    values.extend([real(48, bits.bd()?), real(49, bits.bd()?)]);
    values.extend([real(140, bits.bd()?), real(141, bits.bd()?)]);
    if !r2004 {
        values.push(text_value(6, &bits.text()?));
    }
    values.extend([real(142, bits.bd()?), real(143, bits.bd()?)]);
    values.push(text_value(7, &bits.text()?));
    values.push(int(75, i64::from(bits.bs()?)));
    values.push(real(147, bits.bd()?));
    values.extend([real(148, bits.bd()?), real(149, bits.bd()?)]);
    // 陰影の印刷の方法・解像度・DPI
    if r2004 {
        for code in [76, 77, 78] {
            values.push(int(code, i64::from(bits.bs()?)));
        }
    }
    // レイアウトのフィールド
    values.push(text_value(1, &bits.text()?));
    values.push(int(71, i64::from(bits.bs()?)));
    values.push(int(70, i64::from(bits.bs()?)));
    values.extend(point3(bits, 13)?);
    values.extend(point2(bits, 10)?);
    values.extend(point2(bits, 11)?);
    values.extend(point3(bits, 12)?);
    values.extend(point3(bits, 16)?);
    values.extend(point3(bits, 17)?);
    values.push(real(146, bits.bd()?));
    values.push(int(76, i64::from(bits.bs()?)));
    values.extend(point3(bits, 14)?);
    values.extend(point3(bits, 15)?);
    if r2004 {
        // ビューポートの数。ODS は RL とするが、サンプルの図面では BL だった。ビューポートのハンドルは置かない
        placeholders(bits, handles, 0)?;
    }
    Some(values)
}

/// RC を `n` 個読む
pub(super) fn raw_bytes(bits: &mut Bits, n: usize) -> Option<Vec<u8>> {
    (0..n).map(|_| bits.rc()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dwg::bits::BitWriter;

    /// R2004 の LAYOUT のフィールド。文字は空、数は 0 にし、終わりにビューポートの数を置く。並びは [`layout`] の読む順
    fn layout_fields(viewports: i32) -> Vec<u8> {
        let mut w = BitWriter::default();
        // 印刷の設定: 名前（1）・プリンター（2）・フラグ（70）・余白と紙（40〜45）・紙の名前（4）・原点（46・47）・
        // 72〜74・窓（48・49・140・141）・尺度（142・143）・スタイルの表（7）・75・147〜149・陰影（76〜78）
        w.text(b"").unwrap();
        w.text(b"").unwrap();
        w.bs(0);
        (0..6).for_each(|_| w.bd(0.0));
        w.text(b"").unwrap();
        (0..2).for_each(|_| w.bd(0.0));
        (0..3).for_each(|_| w.bs(0));
        (0..6).for_each(|_| w.bd(0.0));
        w.text(b"").unwrap();
        w.bs(0);
        (0..3).for_each(|_| w.bd(0.0));
        (0..3).for_each(|_| w.bs(0));
        // レイアウト: 名前（1）・タブ（71）・フラグ（70）・13・10・11・12・16・17・146・76・14・15
        w.text(b"").unwrap();
        w.bs(0);
        w.bs(0);
        (0..3).for_each(|_| w.bd(0.0));
        (0..4).for_each(|_| w.rd(0.0));
        (0..10).for_each(|_| w.bd(0.0));
        w.bs(0);
        (0..6).for_each(|_| w.bd(0.0));
        w.bl(viewports);
        w.into_bytes()
    }

    fn read_layout(src: &[u8]) -> (Option<Vec<Value>>, Vec<i16>) {
        let mut handles = Vec::new();
        let values = layout(
            &mut Bits::with_release(src, Release::R2004),
            Release::R2004,
            &mut handles,
        );
        (values, handles)
    }

    /// R2000 の GROUP のフィールド。説明は空、フラグは 0 にし、終わりに図形の数を置く。並びは [`group`] の読む順
    fn group_fields(entities: i32) -> Vec<u8> {
        let mut w = BitWriter::default();
        w.text(b"").unwrap();
        w.bs(0);
        w.bs(0);
        w.bl(entities);
        w.into_bytes()
    }

    fn read_group(src: &[u8]) -> (Option<Vec<Value>>, Vec<i16>) {
        let mut handles = Vec::new();
        let values = group(&mut Bits::with_release(src, Release::R2000), &mut handles);
        (values, handles)
    }

    /// DICTIONARY の項目の数・複製の扱い・所有するフラグ。文字列のストリームはない（#582）
    fn dictionary_fields(entries: i32) -> Vec<u8> {
        let mut w = BitWriter::default();
        w.bl(entries);
        w.bs(0);
        w.rc(0);
        w.into_bytes()
    }

    /// 文字列のストリームがないと、項目の名前は入力を進めず空になる。項目のハンドルが入らない数を、
    /// 名前を積む前に拒む。境界と負の数は、既存の GROUP・LAYOUT と同じ扱いにする
    #[test]
    fn dictionary_entries_without_strings_are_bounded_by_the_data() {
        let (size, count) = at_the_limit(dictionary_fields);
        for release in [Release::R2010, Release::R2013, Release::R2018] {
            for (count, expected) in [
                (count + 1, None),
                (count, Some(size)),
                (0, Some(0)),
                (-1, Some(0)),
            ] {
                let data = dictionary_fields(count);
                let mut entries = Vec::new();
                let values = dictionary(&mut Bits::with_release(&data, release), &mut entries);
                assert_eq!(values.is_some(), expected.is_some(), "{release:?}: {count}");
                assert_eq!(entries.len(), expected.unwrap_or(0));
                assert!(entries.iter().all(Vec::is_empty));
            }
        }
    }

    /// 数がデータのバイト数と同じフィールド。BL は 1〜255 なら同じ長さなので、1 で作ったフィールドのバイト数を数にする
    fn at_the_limit(fields: impl Fn(i32) -> Vec<u8>) -> (usize, i32) {
        let size = fields(1).len();
        let count = i32::try_from(size).unwrap();
        assert!(count < 255);
        assert_eq!(fields(count).len(), size);
        (size, count)
    }

    /// fuzz で見つけた形（#501）。データにないビューポートの数だけ仮の値を積むと、数 GB を取った。上限は、
    /// データのバイト数。超えれば読まず、収まる数ならその数の仮の値を置き、負の数はビューポートがないとする
    #[test]
    fn layout_viewports_beyond_the_data_are_invalid() {
        let (size, count) = at_the_limit(layout_fields);
        let (values, handles) = read_layout(&layout_fields(count + 1));
        assert_eq!(values, None);
        assert_eq!(handles, [0, 330, 331, 346, 345]);
        let (values, handles) = read_layout(&layout_fields(count));
        assert!(values.is_some());
        assert_eq!(handles.len(), 5 + size);
        assert!(handles[5..].iter().all(|code| *code == 0));
        let (values, handles) = read_layout(&layout_fields(-1));
        assert!(values.is_some());
        assert_eq!(handles, [0, 330, 331, 346, 345]);
    }

    /// GROUP の図形の数も、LAYOUT と同じくデータのバイト数で抑える（#501）
    #[test]
    fn group_entities_beyond_the_data_are_invalid() {
        let (size, count) = at_the_limit(group_fields);
        let (values, handles) = read_group(&group_fields(count + 1));
        assert_eq!(values, None);
        assert_eq!(handles, []);
        let (values, handles) = read_group(&group_fields(count));
        assert!(values.is_some());
        assert_eq!(handles, vec![340; size]);
        let (values, handles) = read_group(&group_fields(-1));
        assert!(values.is_some());
        assert_eq!(handles, []);
    }
}
