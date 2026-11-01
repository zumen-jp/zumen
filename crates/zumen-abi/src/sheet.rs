//! `sheet` の op（#391）。xlsx と CSV の表を読み、境界の形にする。

use zumen_base::Cx;

use crate::messages::{
    Sheet, SheetArea, SheetCell, SheetCellKind, SheetDelimiter, SheetEncoding, SheetFormat,
    SheetRequest, SheetResult,
};
use crate::session::{Failed, stopped};

/// ZIP の署名
const ZIP: &[u8] = b"PK\x03\x04";

pub(crate) fn sheet(
    request: &SheetRequest,
    bytes: &[u8],
    cx: &mut Cx<'_>,
) -> Result<SheetResult, Failed> {
    let format = request.format.unwrap_or(if bytes.starts_with(ZIP) {
        SheetFormat::Xlsx
    } else {
        SheetFormat::Csv
    });
    let encoding = request.encoding.map(|encoding| match encoding {
        SheetEncoding::Utf8 => zumen_sheet::Encoding::Utf8,
        SheetEncoding::ShiftJis => zumen_sheet::Encoding::ShiftJis,
    });
    let delimiter = match request.delimiter.unwrap_or(SheetDelimiter::Comma) {
        SheetDelimiter::Comma => ',',
        SheetDelimiter::Tab => '\t',
        SheetDelimiter::Semicolon => ';',
    };
    let book = match format {
        SheetFormat::Xlsx => zumen_sheet::read_xlsx(bytes, cx),
        SheetFormat::Csv => zumen_sheet::read_csv(bytes, encoding, delimiter, cx),
    }
    .map_err(|error| match error {
        zumen_sdk::Error::Stopped(stop) => stopped(cx, stop),
        // ほかの理由は、読む処理が診断として足してある
        _ => Failed,
    })?;
    Ok(SheetResult {
        format,
        encoding: book.encoding.map(|encoding| match encoding {
            zumen_sheet::Encoding::Utf8 => SheetEncoding::Utf8,
            zumen_sheet::Encoding::ShiftJis => SheetEncoding::ShiftJis,
        }),
        sheets: book
            .sheets
            .into_iter()
            .map(|sheet| Sheet {
                name: sheet.name,
                rows: u64::from(sheet.rows),
                columns: u64::from(sheet.columns),
                cells: sheet
                    .cells
                    .into_iter()
                    .map(|cell| SheetCell {
                        row: u64::from(cell.row),
                        column: u64::from(cell.column),
                        kind: match cell.kind {
                            zumen_sheet::Kind::Text => SheetCellKind::Text,
                            zumen_sheet::Kind::Number => SheetCellKind::Number,
                            zumen_sheet::Kind::Boolean => SheetCellKind::Boolean,
                            zumen_sheet::Kind::Error => SheetCellKind::Error,
                        },
                        value: cell.value,
                        formula: cell.formula,
                    })
                    .collect(),
                merged: sheet
                    .merged
                    .into_iter()
                    .map(|area| SheetArea {
                        first_row: u64::from(area.first_row),
                        first_column: u64::from(area.first_column),
                        last_row: u64::from(area.last_row),
                        last_column: u64::from(area.last_column),
                    })
                    .collect(),
            })
            .collect(),
    })
}
