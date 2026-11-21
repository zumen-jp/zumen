//! R2004 の新規 ACAD_TABLE。既定の書式を使う文字と空のセルを扱う（ODS 5.4.1 の 20.4.96.1）。

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

fn text_cell(out: &mut BitWriter, cell: &[Value]) -> Option<()> {
    let mut cell = cell.iter();
    for (code, expected) in [
        (171, 1),
        (172, 0),
        (173, 0),
        (174, 0),
        (175, 1),
        (176, 1),
        (177, 0),
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
