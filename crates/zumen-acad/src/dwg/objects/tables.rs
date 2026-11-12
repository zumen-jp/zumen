//! IMAGE・MULTILEADER・ACAD_TABLE の図形フィールドの読み取り。

use super::*;

/// IMAGE のフィールド（ODS 5.4.1 の 20.4.80。クラスで決まる型）。DXF と同じく、切り抜きの境界が矩形（71 = 1）でも、
/// 頂点の数（91 = 2）を置く。画像の定義とリアクターのハンドルは、ハンドルのストリームで読む
pub(super) fn image(bits: &mut Bits, release: Release) -> Option<Vec<Value>> {
    let mut values = vec![int(90, i64::from(bits.bl()?))];
    values.extend(point3(bits, 10)?);
    values.extend(point3(bits, 11)?);
    values.extend(point3(bits, 12)?);
    values.extend(point2(bits, 13)?);
    values.push(int(70, i64::from(bits.bs()?)));
    values.push(int(280, i64::from(bits.bit()?)));
    for code in [281, 282, 283] {
        values.push(int(code, i64::from(bits.rc()?)));
    }
    if release >= Release::R2010 {
        values.push(Value {
            code: 290,
            data: Data::Bool(bits.bit()?),
        });
    }
    let kind = bits.bs()?;
    values.push(int(71, i64::from(kind)));
    let count = if kind == 1 { 2 } else { bits.bl()? };
    values.push(int(91, i64::from(count)));
    for _ in 0..count {
        values.extend(point2(bits, 14)?);
    }
    Some(values)
}

/// データの中のハンドルの仮の値の始まり。`SLOT + n` は、データの n 番目のハンドル
pub(super) const SLOT: u64 = 1 << 63;

/// データの中にハンドルを持つ型（MULTILEADER・ACAD_TABLE）のフィールドを読む助け。データの中のハンドルを数える
pub(super) struct Slotted<'b, 'a> {
    bits: &'b mut Bits<'a>,
    slots: &'b mut u64,
    /// 実数（BD）のビットの範囲を、読んだ順に足す所。あれば、実数の値の代わりに、その番号を置く（決定 0191）
    reals: Option<&'b mut Vec<Range<usize>>>,
}

impl Slotted<'_, '_> {
    /// データの中のハンドル。次のハンドルの仮の値を返す
    fn handle(&mut self) -> Data {
        let slot = Data::Handle(SLOT + *self.slots);
        *self.slots += 1;
        slot
    }

    /// CMC の色を、DXF と同じく RGB の値（BL）で置く。番号だけを持つ R2000 は、DXF と同じく、ByLayer（256）を
    /// 0xC0000000、ByBlock（0）を 0xC1000000、ほかの番号を 0xC3000000 に番号を足した値に直す（ByLayer と
    /// ByBlock はサンプルの DWG と DXF で確かめた）
    fn color(&mut self, code: i16) -> Option<Value> {
        let index = self.bits.bs()?;
        if self.bits.release() < Release::R2004 {
            let rgb: u32 = match index {
                256 => 0xC000_0000,
                0 => 0xC100_0000,
                _ => 0xC300_0000 | u32::from(index as u16 & 0xFF),
            };
            return Some(int(code, i64::from(rgb as i32)));
        }
        let rgb = self.bits.bl()?;
        let flags = self.bits.rc()?;
        if flags & 1 != 0 {
            self.bits.text()?;
        }
        if flags & 2 != 0 {
            self.bits.text()?;
        }
        Some(int(code, i64::from(rgb)))
    }

    /// ACAD_TABLE の CMC の色を、DXF と同じく色の番号で置く。バージョンに依らず R2004 からの形で読む
    fn table_color(&mut self, code: i16) -> Option<Value> {
        Some(int(code, i64::from(self.bits.cmc_r2004()?.index)))
    }

    fn flag(&mut self, code: i16) -> Option<Value> {
        Some(Value {
            code,
            data: Data::Bool(self.bits.bit()?),
        })
    }

    fn bs(&mut self, code: i16) -> Option<Value> {
        Some(int(code, i64::from(self.bits.bs()?)))
    }

    fn bl(&mut self, code: i16) -> Option<Value> {
        Some(int(code, i64::from(self.bits.bl()?)))
    }

    fn bd(&mut self, code: i16) -> Option<Value> {
        let at = self.bits.pos();
        let value = self.bits.bd()?;
        Some(real(
            code,
            match &mut self.reals {
                Some(reals) => {
                    reals.push(at..self.bits.pos());
                    (reals.len() - 1) as f64
                }
                None => value,
            },
        ))
    }

    fn point(&mut self, code: i16) -> Option<Vec<Value>> {
        Some(vec![
            self.bd(code)?,
            self.bd(code + 10)?,
            self.bd(code + 20)?,
        ])
    }

    fn count(&mut self) -> Option<usize> {
        usize::try_from(self.bits.bl()?).ok()
    }
}

/// MULTILEADER のフィールド（ODS 5.4.1 の 20.4.48 と、注記の文脈の 20.4.86。クラスで決まる型）。DXF と同じく、文脈を
/// 300〜301 のセクションに置き、枝（302〜303）と線（304〜305）を、文脈の内容の後に置く（DWG では内容の前にある）。
/// 文脈が文字を持つときも、DXF と同じく記号を持つフラグ（296）を偽で置く。線ごとの上書きのフィールドは、DXF と同じく上書きの
/// フラグ（93）が立つものだけを置く。2 つのフィールドを持つ文脈と図形の上と下の寄せ方は、DWG では上・下の順だが、DXF の
/// 下（272）・上（273）の順に置く。データの中のハンドルは、仮の値で置く（`slots` にその数を足す）。`reals` が
/// あれば、実数のビットの範囲を足し、実数の値の代わりにその番号を置く（書き戻しが使う。決定 0191）
pub(in crate::dwg) fn multileader(
    bits: &mut Bits,
    release: Release,
    slots: &mut u64,
    reals: Option<&mut Vec<Range<usize>>>,
) -> Option<Vec<Value>> {
    let r2010 = release >= Release::R2010;
    let mut r = Slotted { bits, slots, reals };
    let mut values = Vec::new();
    if r2010 {
        values.push(r.bs(270)?);
    }
    // 枝と線
    let mut leaders = Vec::new();
    for _ in 0..r.count()? {
        leaders.push(text_value(302, b"LEADER{"));
        leaders.extend([r.flag(290)?, r.flag(291)?]);
        leaders.extend(r.point(10)?);
        leaders.extend(r.point(11)?);
        for _ in 0..r.count()? {
            leaders.extend(r.point(12)?);
            leaders.extend(r.point(13)?);
        }
        leaders.extend([r.bl(90)?, r.bd(40)?]);
        for _ in 0..r.count()? {
            leaders.push(text_value(304, b"LEADER_LINE{"));
            for _ in 0..r.count()? {
                leaders.extend(r.point(10)?);
            }
            for _ in 0..r.count()? {
                leaders.push(r.bl(90)?);
                for _ in 0..r.count()? {
                    leaders.extend(r.point(11)?);
                    leaders.extend(r.point(12)?);
                }
            }
            leaders.push(r.bl(91)?);
            if r2010 {
                let line_type = r.handle();
                let arrow = r.handle();
                let overrides = [
                    r.bs(170)?,
                    r.color(92)?,
                    Value {
                        code: 340,
                        data: line_type,
                    },
                    r.bl(171)?,
                    r.bd(40)?,
                    Value {
                        code: 341,
                        data: arrow,
                    },
                ];
                let flags = r.bits.bl()?;
                for (bit, value) in overrides.into_iter().enumerate() {
                    // 置かないハンドルも、仮の値を数えたので、ハンドルのストリームからは読む
                    if (flags >> bit) & 1 != 0 {
                        leaders.push(value);
                    }
                }
                if flags != 0 {
                    leaders.push(int(93, i64::from(flags)));
                }
            }
            leaders.push(text_value(305, b"}"));
        }
        if r2010 {
            leaders.push(r.bs(271)?);
        }
        leaders.push(text_value(303, b"}"));
    }
    // 文脈の内容
    values.push(text_value(300, b"CONTEXT_DATA{"));
    values.push(r.bd(40)?);
    values.extend(r.point(10)?);
    values.extend([r.bd(41)?, r.bd(140)?, r.bd(145)?]);
    values.extend([r.bs(174)?, r.bs(175)?, r.bs(176)?, r.bs(177)?]);
    let text = r.bits.bit()?;
    values.push(Value {
        code: 290,
        data: Data::Bool(text),
    });
    if text {
        values.push(text_value(304, &r.bits.text()?));
        values.extend(r.point(11)?);
        let style = r.handle();
        values.push(Value {
            code: 340,
            data: style,
        });
        values.extend(r.point(12)?);
        values.extend(r.point(13)?);
        values.extend([r.bd(42)?, r.bd(43)?, r.bd(44)?, r.bd(45)?]);
        values.extend([r.bs(170)?, r.color(90)?, r.bs(171)?, r.bs(172)?]);
        values.extend([r.color(91)?, r.bd(141)?, r.bl(92)?]);
        values.extend([r.flag(291)?, r.flag(292)?, r.bs(173)?, r.flag(293)?]);
        values.extend([r.bd(142)?, r.bd(143)?, r.flag(294)?]);
        for _ in 0..r.count()? {
            values.push(r.bd(144)?);
        }
        values.push(r.flag(295)?);
        r.bits.bit()?;
        values.push(Value {
            code: 296,
            data: Data::Bool(false),
        });
    } else {
        let symbol = r.bits.bit()?;
        values.push(Value {
            code: 296,
            data: Data::Bool(symbol),
        });
        if symbol {
            let block = r.handle();
            values.push(Value {
                code: 341,
                data: block,
            });
            values.extend(r.point(14)?);
            values.extend(r.point(15)?);
            values.extend(r.point(16)?);
            values.extend([r.bd(46)?, r.color(93)?]);
            for _ in 0..16 {
                values.push(r.bd(47)?);
            }
        }
    }
    values.extend(r.point(110)?);
    values.extend(r.point(111)?);
    values.extend(r.point(112)?);
    values.push(r.flag(297)?);
    values.extend(leaders);
    if r2010 {
        let top = r.bs(273)?;
        values.extend([r.bs(272)?, top]);
    }
    values.push(text_value(301, b"}"));
    // 図形のフィールド
    let style = r.handle();
    values.push(Value {
        code: 340,
        data: style,
    });
    values.extend([r.bl(90)?, r.bs(170)?, r.color(91)?]);
    let line_type = r.handle();
    values.push(Value {
        code: 341,
        data: line_type,
    });
    values.extend([r.bl(171)?, r.flag(290)?, r.flag(291)?, r.bd(41)?]);
    let arrow = r.handle();
    values.push(Value {
        code: 342,
        data: arrow,
    });
    values.extend([r.bd(42)?, r.bs(172)?]);
    let text_style = r.handle();
    values.push(Value {
        code: 343,
        data: text_style,
    });
    values.extend([r.bs(173)?, r.bs(95)?, r.bs(174)?, r.bs(175)?]);
    values.extend([r.color(92)?, r.flag(292)?]);
    let block = r.handle();
    values.push(Value {
        code: 344,
        data: block,
    });
    values.push(r.color(93)?);
    values.extend(r.point(10)?);
    values.extend([r.bd(43)?, r.bs(176)?, r.flag(293)?]);
    // 矢印は R2007 までだけが持つ（R2010 からは線ごとに持つ。サンプルの DWG で、数が 1 つだけなことを確かめた）
    let arrows = if r2010 { 0 } else { r.count()? };
    for _ in 0..arrows {
        values.push(r.flag(94)?);
        let arrow = r.handle();
        values.push(Value {
            code: 345,
            data: arrow,
        });
    }
    for _ in 0..r.count()? {
        let definition = r.handle();
        values.push(Value {
            code: 330,
            data: definition,
        });
        values.push(text_value(302, &r.bits.text()?));
        values.extend([r.bs(177)?, r.bd(44)?]);
    }
    values.extend([r.flag(294)?, r.bs(178)?, r.bs(179)?, r.bd(45)?]);
    if r2010 {
        values.push(r.bs(271)?);
        let top = r.bs(273)?;
        values.extend([r.bs(272)?, top]);
    }
    if release >= Release::R2013 {
        values.push(r.flag(295)?);
    }
    Some(values)
}

/// ACAD_TABLE のフィールド（ODS 5.4.1 の 20.4.96.1。クラスで決まる型。R2004 まで）。INSERT のフィールドに続けて、表のフィールドを
/// DXF と同じ並び（表のスタイル（342）・横の向き（11）・フラグ（90）・行と列の数（91・92）・表の上書き・行の高さ
/// （141）・列の幅（142）・セル）で置く。ブロック（2）は、名前に直して値の終わりに置く。セルの文字は、DXF と
/// 同じく 250 バイトずつの 2 と、残りの 1 に分ける。枠の線幅と表示の上書きは、DXF のグループ・コードが
/// 分からないので、フラグ（95・96）だけを置く。データの中のハンドルは、仮の値で置く（`slots` にその数を足す）
pub(super) fn table(bits: &mut Bits, release: Release, slots: &mut u64) -> Option<Vec<Value>> {
    let mut values = insert(bits, false)?;
    let attribs = values.iter().any(|v| v.code == 66);
    values.retain(|v| v.code != 66);
    // R2004 からは、属性を持つときだけ、持つオブジェクトの数（BL）が続く（サンプルの DWG で確かめた）
    let owned = if attribs && release >= Release::R2004 {
        bits.bl()?
    } else {
        0
    };
    let mut r = Slotted {
        bits,
        slots,
        reals: None,
    };
    // ハンドルのストリーム: ブロック・属性（R2000 は最初と最後、R2004 は持つ数だけ）・SEQEND・表のスタイル・セル
    let block = r.handle();
    values.push(Value {
        code: 2,
        data: block,
    });
    let attributes = match (attribs, release >= Release::R2004) {
        (false, _) => 0,
        (true, false) => 2,
        (true, true) => owned,
    };
    for _ in 0..attributes {
        r.handle();
    }
    if attribs {
        r.handle();
    }
    let style = r.handle();
    values.push(Value {
        code: 342,
        data: style,
    });
    let flags = r.bs(90)?;
    values.extend(r.point(11)?);
    values.push(flags);
    let columns = r.count()?;
    let rows = r.count()?;
    values.push(int(91, i64::try_from(rows).ok()?));
    values.push(int(92, i64::try_from(columns).ok()?));
    let mut widths = Vec::new();
    for _ in 0..columns {
        widths.push(r.bd(142)?);
    }
    let mut heights = Vec::new();
    for _ in 0..rows {
        heights.push(r.bd(141)?);
    }
    let mut cells = Vec::new();
    for _ in 0..rows.checked_mul(columns)? {
        cell(&mut r, &mut cells)?;
    }
    // 表の上書き
    if r.bits.bit()? {
        let flags = r.bits.bl()?;
        values.push(int(93, i64::from(flags)));
        let set = |bit: i32| flags & bit != 0;
        if set(0x1) {
            values.push(int(280, i64::from(r.bits.bit()?)));
        }
        if set(0x4) {
            values.push(r.bs(70)?);
        }
        if set(0x8) {
            values.push(r.bd(40)?);
        }
        if set(0x10) {
            values.push(r.bd(41)?);
        }
        // 表題・見出し・データの行の上書き。DWG はこの順に持ち、DXF はデータ・表題・見出しの順に書く（推定。
        // サンプルでは、見出しとデータの行の値が同じ）
        let mut rows = |bits: [i32; 3], read: &mut dyn FnMut(&mut Slotted) -> Option<Value>| {
            let mut found: [Option<Value>; 3] = [None, None, None];
            for (slot, bit) in found.iter_mut().zip(bits) {
                if set(bit) {
                    *slot = Some(read(&mut r)?);
                }
            }
            let [title, header, data] = found;
            values.extend([data, title, header].into_iter().flatten());
            Some(())
        };
        rows([0x20, 0x40, 0x80], &mut |r| r.table_color(64))?;
        rows([0x100, 0x200, 0x400], &mut |r| {
            Some(int(283, i64::from(r.bits.bit()?)))
        })?;
        rows([0x800, 0x1000, 0x2000], &mut |r| r.table_color(63))?;
        rows([0x4000, 0x8000, 0x1_0000], &mut |r| r.bs(170))?;
        rows([0x2_0000, 0x4_0000, 0x8_0000], &mut |r| {
            Some(Value {
                code: 7,
                data: r.handle(),
            })
        })?;
        rows([0x10_0000, 0x20_0000, 0x40_0000], &mut |r| r.bd(140))?;
    }
    // 枠の色・線幅・表示の上書き。表題・見出し・データの行ごとに、横の上・中・下と縦の左・中・右の 6 つ
    if r.bits.bit()? {
        let flags = r.bits.bl()?;
        values.push(int(94, i64::from(flags)));
        for bit in 0..18 {
            if (flags >> bit) & 1 != 0 {
                let code = *[64, 65, 66, 63, 68, 69].get(bit % 6)?;
                values.push(r.table_color(code)?);
            }
        }
    }
    for code in [95, 96] {
        if r.bits.bit()? {
            let flags = r.bits.bl()?;
            values.push(int(code, i64::from(flags)));
            for bit in 0..18 {
                if (flags >> bit) & 1 != 0 {
                    r.bits.bs()?;
                }
            }
        }
    }
    values.extend(heights);
    values.extend(widths);
    values.extend(cells);
    Some(values)
}

/// R2010 からの ACAD_TABLE のフィールド（ODS 5.4.1 の 20.4.96.2 と、20.4.97 の TABLECONTENT）。
pub(super) fn table_content(
    bits: &mut Bits,
    release: Release,
    slots: &mut u64,
) -> Option<Vec<Value>> {
    let mut values = insert(bits, false)?;
    let attribs = values.iter().any(|v| v.code == 66);
    values.retain(|v| v.code != 66);
    let owned = if attribs { bits.bl()? } else { 0 };
    let mut r = Slotted {
        bits,
        slots,
        reals: None,
    };
    let block = r.handle();
    for _ in 0..owned {
        r.handle();
    }
    if attribs {
        r.handle();
    }
    // 20.4.96.2 の意味の分からないフィールド
    r.bits.rc()?;
    r.handle();
    r.bits.bl()?;
    if release == Release::R2010 {
        r.bits.bit()?;
    } else {
        r.bits.bl()?;
    }
    // AcDbLinkedData
    r.bits.text()?;
    r.bits.text()?;
    let columns = r.count()?;
    let mut widths = Vec::new();
    for _ in 0..columns {
        r.bits.text()?;
        r.bits.bl()?;
        custom_data(&mut r)?;
        cell_style(&mut r)?;
        r.bits.bl()?;
        widths.push(r.bd(142)?);
    }
    let rows = r.count()?;
    let mut heights = Vec::new();
    let mut cells = Vec::new();
    for row in 0..rows {
        for column in 0..r.count()? {
            r.bits.bl()?;
            r.bits.text()?;
            r.bits.bl()?;
            custom_data(&mut r)?;
            if r.bits.bl()? != 0 {
                r.handle();
                r.bits.bl()?;
                r.bits.bl()?;
                r.bits.bl()?;
            }
            let contents = r.count()?;
            let mut value = Vec::new();
            let mut kind = 1;
            let mut symbol = None;
            for _ in 0..contents {
                match r.bits.bl()? {
                    1 => value = table_value(&mut r)?,
                    2 => {
                        r.handle();
                    }
                    4 => {
                        kind = 2;
                        symbol = Some(r.handle());
                    }
                    _ => return None,
                }
                for _ in 0..r.count()? {
                    r.handle();
                    r.bits.text()?;
                    r.bits.bl()?;
                }
                if r.bits.bs()? != 0 {
                    content_format(&mut r)?;
                }
            }
            cell_style(&mut r)?;
            r.bits.bl()?;
            if r.bits.bl()? != 0 {
                r.bits.bl()?;
                r.bits.bd()?;
                r.bits.bd()?;
                let geometry = r.bits.bl()?;
                r.handle();
                if geometry != 0 {
                    for _ in 0..6 {
                        r.bits.bd()?;
                    }
                    for _ in 0..4 {
                        r.bits.bd()?;
                    }
                    r.bits.bl()?;
                }
            }
            cells.push((row, column, kind, symbol, value));
        }
        r.bits.bl()?;
        custom_data(&mut r)?;
        cell_style(&mut r)?;
        r.bits.bl()?;
        heights.push(r.bd(141)?);
    }
    for _ in 0..r.count()? {
        r.handle();
    }
    // AcDbFormattedTableData
    cell_style(&mut r)?;
    // 結合の範囲は、上・左・下・右の行と列の番号
    let mut merged = Vec::new();
    for _ in 0..r.count()? {
        let top = r.count()?;
        let left = r.count()?;
        let bottom = r.count()?;
        let right = r.count()?;
        merged.push((top, left, bottom, right));
    }
    let style = r.handle();
    r.bits.bs()?;
    let direction = r.point(11)?;
    values.push(Value {
        code: 342,
        data: style,
    });
    values.extend(direction);
    values.push(int(91, i64::try_from(rows).ok()?));
    values.push(int(92, i64::try_from(columns).ok()?));
    values.extend(heights);
    values.extend(widths);
    for (row, column, kind, symbol, value) in cells {
        let (covered, columns, rows) = merged
            .iter()
            .find(|m| m.0 <= row && row <= m.2 && m.1 <= column && column <= m.3)
            .map_or((0, 1, 1), |m| {
                if (m.0, m.1) == (row, column) {
                    (0, m.3 - m.1 + 1, m.2 - m.0 + 1)
                } else {
                    (1, 1, 1)
                }
            });
        values.push(int(171, kind));
        values.push(int(173, covered));
        values.push(int(175, i64::try_from(columns).ok()?));
        values.push(int(176, i64::try_from(rows).ok()?));
        if let Some(symbol) = symbol {
            values.push(Value {
                code: 340,
                data: symbol,
            });
        }
        values.push(text_value(301, b"CELL_VALUE"));
        if value.is_empty() {
            values.push(text_value(302, b""));
        } else {
            values.extend(value);
        }
        values.push(text_value(304, b"ACVALUE_END"));
    }
    values.push(Value {
        code: 2,
        data: block,
    });
    Some(values)
}

/// 表の名前と値の組（ODS 5.4.1 の 20.4.100）を読み飛ばす
pub(super) fn custom_data(r: &mut Slotted) -> Option<()> {
    for _ in 0..r.count()? {
        r.bits.text()?;
        table_value(r)?;
    }
    Some(())
}

/// 表の値（ODS 5.4.1 の 20.4.99、R2007 から）。DXF の 301 `CELL_VALUE` から 304 `ACVALUE_END` までの間と同じく、
/// フラグ（93）・型（90）・値・単位（94）・書式（300）・表示する文字（302）を並べる。値は、整数（1）を 91、実数（2）を
/// 140、文字列（4）を 1、日時（8）を大きさ（92）とバイト列（310）、点（16・32）を 11・21・31 に置く。ほかの型の値は
/// 置かない
pub(super) fn table_value(r: &mut Slotted) -> Option<Vec<Value>> {
    let flags = r.bits.bl()?;
    let kind = r.bits.bl()?;
    let mut values = vec![int(93, i64::from(flags)), int(90, i64::from(kind))];
    if flags & 1 == 0 {
        match kind {
            0 => {
                r.bits.bl()?;
            }
            1 => values.push(int(91, i64::from(r.bits.bl()?))),
            2 => values.push(real(140, r.bits.bd()?)),
            // 文字列も、文字列のストリームでなく、本体に大きさと UTF-16 の並びで持つ
            4 | 8 | 16 | 32 | 512 => {
                let size = usize::try_from(r.bits.bl()?).ok()?;
                let bytes = raw_bytes(r.bits, size)?;
                match kind {
                    4 => {
                        let mut units: Vec<u16> = bytes
                            .as_chunks::<2>()
                            .0
                            .iter()
                            .map(|pair| u16::from_le_bytes(*pair))
                            .collect();
                        if units.last() == Some(&0) {
                            units.pop();
                        }
                        values.push(text_value(1, String::from_utf16_lossy(&units).as_bytes()));
                    }
                    8 => values.extend([
                        int(92, i64::try_from(size).ok()?),
                        Value {
                            code: 310,
                            data: Data::Binary(bytes),
                        },
                    ]),
                    16 | 32 => {
                        for (code, chunk) in [11, 21, 31].into_iter().zip(bytes.as_chunks::<8>().0)
                        {
                            values.push(real(code, f64::from_le_bytes(*chunk)));
                        }
                    }
                    _ => {}
                }
            }
            64 => {
                r.handle();
            }
            _ => return None,
        }
    }
    values.push(int(94, i64::from(r.bits.bl()?)));
    values.push(text_value(300, &r.bits.text()?));
    values.push(text_value(302, &r.bits.text()?));
    Some(values)
}

/// 内容の書式（ODS 5.4.1 の 20.4.101.3）を読み飛ばす
pub(super) fn content_format(r: &mut Slotted) -> Option<()> {
    for _ in 0..4 {
        r.bits.bl()?;
    }
    r.bits.text()?;
    r.bits.bd()?;
    r.bits.bd()?;
    r.bits.bl()?;
    r.bits.cmc_r2004()?;
    r.handle();
    r.bits.bd()?;
    Some(())
}

/// セルのスタイル（ODS 5.4.1 の 20.4.101.4）を読み飛ばす
pub(super) fn cell_style(r: &mut Slotted) -> Option<()> {
    r.bits.bl()?;
    if r.bits.bs()? == 0 {
        return Some(());
    }
    r.bits.bl()?;
    r.bits.bl()?;
    r.bits.cmc_r2004()?;
    r.bits.bl()?;
    content_format(r)?;
    if r.bits.bs()? & 1 != 0 {
        for _ in 0..6 {
            r.bits.bd()?;
        }
    }
    for _ in 0..r.count()? {
        if r.bits.bl()? != 0 {
            r.bits.bl()?;
            r.bits.bl()?;
            r.bits.cmc_r2004()?;
            r.bits.bl()?;
            r.handle();
            r.bits.bl()?;
            r.bits.bd()?;
        }
    }
    Some(())
}

/// ACAD_TABLE のセルのフィールド（ODS 5.4.1 の 20.4.96.1）。種類（171）が 1 なら文字（2 と 1）、2 ならブロック（340）と、
/// 属性の定義（331）と文字（300）を持つ。セルのハンドルは、セルの順に、内容・属性の定義・文字スタイルと読む
/// （推定。サンプルにあるのは、文字スタイルを上書きしない文字のセルだけ）
pub(super) fn cell(r: &mut Slotted, cells: &mut Vec<Value>) -> Option<()> {
    let kind = r.bits.bs()?;
    cells.push(int(171, i64::from(kind)));
    cells.push(int(172, i64::from(r.bits.rc()?)));
    cells.push(int(173, i64::from(r.bits.bit()?)));
    cells.push(int(174, i64::from(r.bits.bit()?)));
    cells.extend([r.bl(175)?, r.bl(176)?, r.bd(145)?]);
    match kind {
        1 => {
            let field = r.handle();
            cells.push(Value {
                code: 344,
                data: field,
            });
            let text = r.bits.text()?;
            let mut rest = text.as_slice();
            while rest.len() > 250 {
                let (chunk, tail) = rest.split_at(250);
                cells.push(text_value(2, chunk));
                rest = tail;
            }
            cells.push(text_value(1, rest));
        }
        2 => {
            let block = r.handle();
            cells.push(Value {
                code: 340,
                data: block,
            });
            cells.push(r.bd(144)?);
            if r.bits.bit()? {
                let count = r.bits.bs()?;
                cells.push(int(179, i64::from(count)));
                for _ in 0..count {
                    let definition = r.handle();
                    cells.push(Value {
                        code: 331,
                        data: definition,
                    });
                    r.bits.bs()?;
                    cells.push(text_value(300, &r.bits.text()?));
                }
            }
        }
        _ => return None,
    }
    if r.bits.bit()? {
        let flags = r.bits.bl()?;
        // DXF は、フラグの下位の 16 ビットを符号つきで書く
        cells.push(int(177, i64::from(flags as i16)));
        cells.push(int(178, i64::from(r.bits.rc()?)));
        let set = |bit: i32| flags & bit != 0;
        if set(0x1) {
            cells.push(r.bs(170)?);
        }
        if set(0x2) {
            cells.push(int(283, i64::from(r.bits.bit()?)));
        }
        if set(0x4) {
            cells.push(r.table_color(63)?);
        }
        if set(0x8) {
            cells.push(r.table_color(64)?);
        }
        if set(0x10) {
            let style = r.handle();
            cells.push(Value {
                code: 7,
                data: style,
            });
        }
        if set(0x20) {
            cells.push(r.bd(140)?);
        }
        // 枠の上・右・下・左の、色・線幅・表示。表示は、DXF が書かないので置かない（フラグが立っても、サンプルの DXF
        // にはない）
        for (color, weight, shift) in [(69, 279, 0), (65, 275, 1), (66, 276, 2), (68, 278, 3)] {
            if set(0x40 << shift) {
                cells.push(r.table_color(color)?);
            }
            if set(0x400 << shift) {
                cells.push(r.bs(weight)?);
            }
            if set(0x4000 << shift) {
                r.bits.bs()?;
            }
        }
    } else {
        // 上書きがないときも、DXF は 177 と 178 を 0 で書く
        cells.extend([int(177, 0), int(178, 0)]);
    }
    Some(())
}
