//! R2004・R2010 以降の新規 ACAD_TABLE。既定の書式を使う文字と空のセルを扱う（ODS 5.4.1 の 20.4.96）。

use super::*;

pub(super) const CODES: &[i16] = &[
    10, 20, 30, 11, 21, 31, 280, 90, 91, 92, 93, 94, 95, 96, 141, 142, 171, 172, 173, 174, 175,
    176, 177, 178, 145, 1,
];

/// 表の表示用ブロックと表スタイルを、ハンドルのストリームに書く順で返す。
pub(super) fn references(db: &AcadDb, object: &Object) -> Option<(Option<u64>, Option<u64>)> {
    let first_cell = object.values.iter().position(|v| v.code == 171)?;
    let head = object.values.get(..first_cell)?;
    if head.iter().any(|v| matches!(v.code, 1 | 145 | 172..=178))
        || [2, 342, 343]
            .iter()
            .any(|code| head.iter().filter(|v| v.code == *code).count() != 1)
    {
        return None;
    }
    let Data::Text(name) = object.first(2)? else {
        return None;
    };
    (anon_dwg_block_name(name)? == b"*T\0").then_some(())?;
    let block = record_handle(db, b"BLOCK_RECORD", name)?;
    (object.first(343) == Some(&Data::Handle(block))).then_some(())?;
    let Data::Handle(style) = object.first(342)? else {
        return None;
    };
    db.get(*style)
        .filter(|record| record.kind == b"TABLESTYLE")?;
    Some((Some(block), Some(*style)))
}

/// 共通の INSERT の後に表ヘッダーと幅・高さ・セルを並べる。
pub(super) fn fields(object: &Object) -> Option<BitWriter> {
    let first_cell = object.values.iter().position(|v| v.code == 171)?;
    let (head, cells) = object.values.split_at(first_cell);
    for code in [280, 93, 94, 95, 96] {
        (integer(object, code)?? == 0).then_some(())?;
    }
    let mut out = BitWriter::default();
    insert(&mut out, object, false, None)?;
    out.bs(small(object, 90)??);
    point3(&mut out, object, 11)?;
    let columns = i32::try_from(integer(object, 92)??).ok()?;
    let rows = i32::try_from(integer(object, 91)??).ok()?;
    (columns > 0 && rows > 0).then_some(())?;
    out.bl(columns);
    out.bl(rows);
    for (code, dimension) in [(142, columns), (141, rows)] {
        let values = head.iter().filter(|value| value.code == code);
        (values.clone().count() == usize::try_from(dimension).ok()?).then_some(())?;
        for value in values {
            let Data::Real(length) = value.data else {
                return None;
            };
            (length.is_finite() && length > 0.0).then_some(())?;
            out.bd(length);
        }
    }
    let count = usize::try_from(columns)
        .ok()?
        .checked_mul(usize::try_from(rows).ok()?)?;
    let mut groups = cells.chunk_by(|_, next| next.code != 171);
    for _ in 0..count {
        text_cell(&mut out, groups.next()?)?;
    }
    groups.next().is_none().then_some(())?;
    // 表の書式、枠の色・太さ・表示の上書きはない。
    for _ in 0..4 {
        out.bit(false);
    }
    Some(out)
}

fn cell_text(cell: &[Value], flags_code: i16, covered: bool, span: i64) -> Option<Vec<u8>> {
    let mut cell = cell.iter();
    for (code, expected) in [
        (171, 1),
        (172, 0),
        (173, i64::from(covered)),
        (174, 0),
        (175, span),
        (176, 1),
        (flags_code, 0),
        (178, 0),
    ] {
        let value = cell.next()?;
        (value.code == code && value.data == Data::Int(expected)).then_some(())?;
    }
    let rotation = cell.next()?;
    (rotation.code == 145 && rotation.data == Data::Real(0.0)).then_some(())?;
    let (last, chunks) = cell.as_slice().split_last()?;
    (last.code == 1 && chunks.iter().all(|part| part.code == 2)).then_some(())?;
    let mut text = Vec::new();
    for value in cell {
        let Data::Text(bytes) = &value.data else {
            return None;
        };
        (!bytes.contains(&0)).then_some(())?;
        text.extend_from_slice(bytes);
    }
    Some(text)
}

fn text_cell(out: &mut BitWriter, cell: &[Value]) -> Option<()> {
    let text = cell_text(cell, 177, false, 1)?;
    out.bs(1);
    out.rc(0);
    out.bit(false);
    out.bit(false);
    out.bl(1);
    out.bl(1);
    out.bd(0.0);
    out.bs(i16::try_from(text.len().checked_add(1)?).ok()?);
    out.raw(&text);
    out.rc(0);
    out.bit(false);
    Some(())
}

/// R2010 以降の ACAD_TABLE と内部の TABLECONTENT（ODS 5.4.1 の 20.4.96.2・20.4.97）。
pub(super) fn modern(out: &mut BitWriter, object: &Object, release: Release) -> Option<()> {
    let first = object.values.iter().position(|v| v.code == 171)?;
    let (head, body) = object.values.split_at(first);
    // セルの値にも 91・92 と座標が現れるので、INSERT・表の値は先頭だけで読む。
    let header = Object {
        kind: object.kind.clone(),
        section: object.section.clone(),
        origin: object.origin,
        values: head.to_vec(),
    };
    let widths = encoding::table_reals(head, 142)?;
    let heights = encoding::table_reals(head, 141)?;
    let rows = i32::try_from(heights.len()).ok()?;
    let cols = i32::try_from(widths.len()).ok()?;
    (rows > 0
        && cols > 0
        && integer(&header, 91)?? == i64::from(rows)
        && integer(&header, 92)?? == i64::from(cols))
    .then_some(())?;
    if widths.iter().chain(&heights).any(|&x| x <= 0.0) {
        return None;
    }
    insert(out, &header, false, None)?;
    out.rc(0);
    out.bl(0);
    if release == Release::R2010 {
        out.bit(true);
    } else {
        out.bl(0);
    }
    // AcDbLinkedData の名前と説明。列の名前、独自のデータと上書きも持たない。
    out.text(b"")?;
    out.text(b"")?;
    out.bl(cols);
    for width in widths {
        out.text(b"")?;
        out.bl(0);
        out.bl(0);
        empty_cell_style(out);
        out.bl(0);
        out.bd(width);
    }
    out.bl(rows);
    let mut groups = body.chunk_by(|_, next| next.code != 171);
    let title_merged = cols > 1
        && body
            .iter()
            .find(|v| v.code == 175)
            .is_some_and(|v| v.data == Data::Int(i64::from(cols)));
    for (row, height) in heights.into_iter().enumerate() {
        out.bl(cols);
        for col in 0..cols {
            let cell = groups.next()?;
            let tag = cell.iter().position(|v| v.code == 301)?;
            let (plain, cached) = cell.split_at_checked(tag)?;
            let merged = title_merged && row == 0;
            let text = cell_text(
                plain,
                91,
                merged && col > 0,
                if merged && col == 0 {
                    i64::from(cols)
                } else {
                    1
                },
            )?;
            if merged && col > 0 && !text.is_empty() {
                return None;
            }
            if !matches_cell_value(cached, &text) {
                return None;
            }
            let value = core::str::from_utf8(&text).ok()?;
            // AcDbLinkedTableData のセル。データリンクは持たず、値 1 個を置く。
            out.bl(0);
            out.text(b"")?;
            out.bl(0);
            out.bl(0);
            out.bl(0);
            out.bl(1);
            out.bl(1);
            encoding::write_r2010_table_value(out, (!value.is_empty()).then_some(value))?;
            out.bl(0);
            out.bs(0);
            empty_cell_style(out);
            out.bl(0);
            out.bl(0);
        }
        out.bl(0);
        out.bl(0);
        empty_cell_style(out);
        // Standard の表題（1）・見出し（2）・データ（3）の行書式を継ぐ。
        out.bl(match row {
            0 => 1,
            1 => 2,
            _ => 3,
        });
        out.bd(height);
    }
    groups.next().is_none().then_some(())?;
    out.bl(0);
    empty_cell_style(out);
    out.bl(i32::from(title_merged));
    if title_merged {
        out.bl(0);
        out.bl(0);
        out.bl(0);
        out.bl(cols - 1);
    }
    // 表は分割しない。元の挿入点を開始位置とし、全行を 1 つの範囲に置く。
    out.bs(38);
    point3(out, &header, 11)?;
    out.bl(0);
    out.bl(1);
    point3(out, &header, 10)?;
    out.bl(0);
    out.bl(rows - 1);
    Some(())
}

/// 20.4.101 の AcDbCellStyle。種類と個別の書式指定は 0 にする。
fn empty_cell_style(out: &mut BitWriter) {
    out.bl(0);
    out.bs(0);
}

/// L1 の文字・セルの値・表示文字列は同じ内容に限る。独自の書式や異なる値を落とさない。
fn matches_cell_value(values: &[Value], text: &[u8]) -> bool {
    let mut values = values.iter();
    let empty = text.is_empty();
    let marker = |v: Option<&Value>, code, text: &[u8]| {
        v.is_some_and(|v| v.code == code && matches!(&v.data, Data::Text(bytes) if bytes == text))
    };
    let integer = |v: Option<&Value>, code, value| {
        v.is_some_and(|v| v.code == code && v.data == Data::Int(value))
    };
    let chunks = |values: &mut core::slice::Iter<'_, Value>, more, last| {
        let mut left = text;
        loop {
            let Some(v) = values.next() else {
                return false;
            };
            let Data::Text(bytes) = &v.data else {
                return false;
            };
            let Some(tail) = left.strip_prefix(bytes.as_slice()) else {
                return false;
            };
            left = tail;
            if v.code == last {
                return left.is_empty();
            }
            if v.code != more {
                return false;
            }
        }
    };
    marker(values.next(), 301, b"CELL_VALUE")
        && integer(values.next(), 90, if empty { 0 } else { 4 })
        && integer(values.next(), 93, if empty { 1 } else { 0 })
        && (empty || (chunks(&mut values, 2, 1) && integer(values.next(), 94, 0)))
        && marker(values.next(), 300, b"")
        && chunks(&mut values, 303, 302)
        && marker(values.next(), 304, b"ACVALUE_END")
        && values.next().is_none()
}
