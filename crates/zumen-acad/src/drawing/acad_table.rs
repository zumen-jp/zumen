//! ACAD_TABLE を表にする（決定 0113）。セルの値と書式、表のスタイルの組の書式を読む。

use super::lettering::attachment;
use super::{BLOCK, Lifted, Lifter, aci, invalid, mtext, non_planar, source};
use crate::db::{AcadDb, Data, Object, Origin, Value};
use zumen_base::{Cx, Diagnostic, codes};
use zumen_drawing::{
    Border, Borders, Cell, CellContent, CellData, CellFormat, CellValue, Color, DateTime, Inherit,
    Length, LineWeight, PaperLength, SymbolKey, Table, TableFormats, ValueUnit,
};

impl Lifter<'_> {
    /// ACAD_TABLE（DXF Reference「TABLE」、決定 0113）。AcDbTable（100）より前の挿入点（10）と、表を描いた
    /// ブロック（2）を読む。後の値は、行と列の数（91・92）・横の向き（11・21・31）・表のスタイル（342）・行の高さ
    /// （141）・列の幅（142）と、種類（171）で始まるセルの並び。セルは、覆われる（173）・広げる列と行の数（175・
    /// 176）を持ち、種類が 1 なら文字（2 の区切りと 1）、2 ならブロックのレコード（340）を内容とする。文字がなく、
    /// 2007 からの値（301 から）の文字（302）があれば、それを内容とする（docs/formats/dxf.md）。数が合わない
    /// ものは読めないとする。セルの値の型と単位と表示の書式は `cell_value` で、セルの書式と枠は `cell_format` で、
    /// 表のスタイルの書式は `table_formats` で読む。R2010 からの DWG の読み手は書式を読まないので、書式は空になる
    /// （`element` で診断する）
    pub(super) fn table<P>(
        &self,
        db: &AcadDb,
        object: &Object,
        point: &impl Fn(f64, f64) -> P,
        length: &impl Fn(f64) -> Length,
    ) -> Lifted<Table<P>> {
        let kind = object.kind.as_slice();
        // DWG は AcDbTable（100）を持たないので、表の欄の初め（342・11・90）で分ける。表を描いたブロック（2）は、
        // DWG では値の終わりにある（docs/formats/dwg.md）
        let dwg = matches!(object.origin, Origin::Dwg { .. });
        let marker = object
            .values
            .iter()
            .position(|value| match (value.code, &value.data) {
                (342 | 11 | 90, _) => dwg,
                (100, Data::Text(text)) => !dwg && text == b"AcDbTable",
                _ => false,
            })
            .ok_or_else(|| invalid(kind, 100))?;
        let (reference, rest) = object
            .values
            .split_at_checked(marker)
            .ok_or_else(|| invalid(kind, 100))?;
        let part = |values: &[Value]| Object {
            values: values.to_vec(),
            ..object.clone()
        };
        let mut reference = part(reference);
        let mut rest = rest.to_vec();
        if dwg && let Some(at) = rest.iter().rposition(|value| value.code == 2) {
            reference.values.push(rest.remove(at));
        }
        let rest = rest.as_slice();
        let first = rest
            .iter()
            .position(|value| value.code == 171)
            .unwrap_or(rest.len());
        let (head, body) = rest.split_at_checked(first).unwrap_or((rest, &[]));
        let head = part(head);
        let count = |code: i16| -> Lifted<usize> {
            usize::try_from(self.int(&head, code, 0)?)
                .ok()
                .filter(|count| *count > 0)
                .ok_or_else(|| invalid(kind, code))
        };
        let [rows, columns] = [count(91)?, count(92)?];
        if self.real(&head, 31, Some(0.0))? != 0.0 {
            return Err(non_planar(kind));
        }
        let sizes = |code: i16, expected: usize| -> Lifted<Vec<Length>> {
            let sizes: Vec<Length> = head
                .values
                .iter()
                .filter(|value| value.code == code)
                .map(|value| match value.data {
                    Data::Real(size) if size.is_finite() && size > 0.0 => Ok(length(size)),
                    _ => Err(invalid(kind, code)),
                })
                .collect::<Lifted<_>>()?;
            if sizes.len() == expected {
                Ok(sizes)
            } else {
                Err(invalid(kind, code))
            }
        };
        let row_heights = sizes(141, rows)?;
        let column_widths = sizes(142, columns)?;
        let mut cells = Vec::new();
        for chunk in body.chunk_by(|_, next| next.code != 171) {
            cells.push(self.cell(db, &part(chunk), length)?);
        }
        if Some(cells.len()) != rows.checked_mul(columns) {
            return Err(invalid(kind, 171));
        }
        let style = match head.first(342) {
            None | Some(Data::Handle(0)) => None,
            Some(Data::Handle(handle)) => {
                Some(source(db.get(*handle).ok_or_else(|| invalid(kind, 342))?))
            }
            Some(_) => return Err(invalid(kind, 342)),
        };
        let formats = match head.first(342) {
            Some(Data::Handle(handle)) => db
                .get(*handle)
                .filter(|style| style.kind == b"TABLESTYLE")
                .map(|style| self.table_formats(db, style, length))
                .transpose()?
                .flatten()
                .map(Box::new),
            _ => None,
        };
        Ok(Table {
            position: self.planar_point(&reference, 10, point)?,
            direction: [
                self.real(&head, 11, Some(1.0))?,
                self.real(&head, 21, Some(0.0))?,
            ],
            row_heights,
            column_widths,
            cells,
            style,
            formats,
            drawn: self.string(&reference, 2)?.map(|name| SymbolKey {
                namespace: BLOCK,
                name,
            }),
        })
    }

    /// 表のセル 1 つの値。2007 からの値（301〜304）の中のコードは、セルの欄として読まない
    fn cell(&self, db: &AcadDb, cell: &Object, length: &impl Fn(f64) -> Length) -> Lifted<Cell> {
        let kind = cell.kind.as_slice();
        let end = cell
            .values
            .iter()
            .position(|value| value.code == 301)
            .unwrap_or(cell.values.len());
        let (fields, value) = cell
            .values
            .split_at_checked(end)
            .unwrap_or((&cell.values, &[]));
        let fields = Object {
            values: fields.to_vec(),
            ..cell.clone()
        };
        let span = |code: i16| -> Lifted<u32> {
            u32::try_from(self.int(&fields, code, 1)?)
                .ok()
                .filter(|span| *span > 0)
                .ok_or_else(|| invalid(kind, code))
        };
        let content = match self.int(&fields, 171, 1)? {
            1 => {
                let mut content = String::new();
                for value in fields
                    .values
                    .iter()
                    .filter(|value| matches!(value.code, 1 | 2))
                {
                    content.push_str(&self.decode(kind, value.code, &value.data)?);
                }
                if fields.first(1).is_none()
                    && let Some(value) = value.iter().find(|value| value.code == 302)
                {
                    content = self.decode(kind, 302, &value.data)?;
                }
                if content.is_empty() {
                    CellContent::Empty
                } else {
                    let spans = mtext::spans(&content, length);
                    CellContent::Text { content, spans }
                }
            }
            2 => CellContent::Symbol(
                self.block_key(db, &fields, 340)?
                    .ok_or_else(|| invalid(kind, 340))?,
            ),
            _ => return Err(invalid(kind, 171)),
        };
        Ok(Cell {
            content,
            span: [span(175)?, span(176)?],
            covered: self.int(&fields, 173, 0)? != 0,
            value: self.cell_value(cell, value)?,
            rotation: self.real(&fields, 145, Some(0.0))?,
            format: self.cell_format(db, &fields, length, &CELL_BORDERS)?,
        })
    }

    /// 表のスタイル（DXF Reference「TABLESTYLE」）の、行の種類ごとの書式。説明（3）の後の 280 が 1 ならタイトルの
    /// 行を、281 が 1 なら見出しの行を持たない。文字スタイル（7）から始まる組が、データ・タイトル・見出しの順に
    /// 続く（試料の表を描いたブロックの文字の高さと位置合わせで確かめた）。組の枠は、上・内側の横・下・左・
    /// 内側の縦・右の順に、線幅（274〜279）・表示（284〜289）・色（64〜69）を持つ（推定）。組がなければ `None`。
    /// R2010 からの DWG の読み手は、TABLESTYLE の欄をまだ読まないので `None` になる（`element` で診断する）
    pub(super) fn table_formats(
        &self,
        db: &AcadDb,
        style: &Object,
        length: &impl Fn(f64) -> Length,
    ) -> Lifted<Option<TableFormats>> {
        let kind = style.kind.as_slice();
        let first = style
            .values
            .iter()
            .position(|value| value.code == 7)
            .unwrap_or(style.values.len());
        let (head, rows) = style.values.split_at_checked(first).unwrap_or_default();
        let after = head
            .iter()
            .position(|value| value.code == 3)
            .map_or(head, |at| head.get(at..).unwrap_or_default());
        let head = Object {
            values: after.to_vec(),
            ..style.clone()
        };
        if rows.is_empty() {
            return Ok(None);
        }
        let mut formats = Vec::new();
        for row in rows.chunk_by(|_, next| next.code != 7) {
            let row = Object {
                values: row.to_vec(),
                ..style.clone()
            };
            formats.push(self.cell_format(db, &row, length, &STYLE_BORDERS)?);
        }
        let [data_format, title_format, header_format] =
            <[CellFormat; 3]>::try_from(formats).map_err(|_| invalid(kind, 7))?;
        Ok(Some(TableFormats {
            title: self.int(&head, 280, 0)? == 0,
            header: self.int(&head, 281, 0)? == 0,
            title_format,
            header_format,
            data_format,
        }))
    }

    /// セルか、表のスタイルの組の書式。文字スタイル（7。名前か STYLE のハンドル）・高さ（140）・位置合わせ
    /// （170。1〜9）・文字の色（62 か、セルでは 64）・背景を塗るか（283）と色（63）・枠（`borders` の欄）を読む。
    /// 色は ACI の符号（0 は配置から、256 はレイヤから継承）、線幅は -1 と -2 を継承、-3 を文書の既定、0〜211 を
    /// 1/100 mm とする
    fn cell_format(
        &self,
        db: &AcadDb,
        object: &Object,
        length: &impl Fn(f64) -> Length,
        borders: &BorderCodes,
    ) -> Lifted<CellFormat> {
        let kind = object.kind.as_slice();
        let color = |code: i16| -> Lifted<Option<Inherit<Color>>> {
            match object.first(code) {
                None => Ok(None),
                Some(Data::Int(0)) => Ok(Some(Inherit::Block)),
                Some(Data::Int(256)) => Ok(Some(Inherit::Layer)),
                Some(Data::Int(index @ 1..=255)) => Ok(Some(Inherit::Own(Color {
                    code: Some(aci(*index)),
                    rgb: None,
                }))),
                Some(_) => Err(invalid(kind, code)),
            }
        };
        let weight = |code: i16| -> Lifted<Option<Inherit<LineWeight>>> {
            match object.first(code) {
                None => Ok(None),
                Some(Data::Int(-1)) => Ok(Some(Inherit::Layer)),
                Some(Data::Int(-2)) => Ok(Some(Inherit::Block)),
                Some(Data::Int(-3)) => Ok(Some(Inherit::Own(LineWeight::Default))),
                Some(Data::Int(width @ 0..=211)) => Ok(Some(Inherit::Own(LineWeight::Width(
                    PaperLength(*width as f64 / 100.0),
                )))),
                Some(_) => Err(invalid(kind, code)),
            }
        };
        let flag = |code: i16| -> Lifted<Option<bool>> {
            match object.first(code) {
                None => Ok(None),
                Some(Data::Int(value)) => Ok(Some(*value != 0)),
                Some(Data::Bool(value)) => Ok(Some(*value)),
                Some(_) => Err(invalid(kind, code)),
            }
        };
        let border = |[color_code, weight_code, visible]: [i16; 3]| -> Lifted<Border> {
            Ok(Border {
                color: color(color_code)?,
                weight: weight(weight_code)?,
                visible: flag(visible)?,
            })
        };
        let text_style = match object.first(7) {
            None | Some(Data::Handle(0)) => None,
            Some(Data::Handle(handle)) => Some(
                db.get(*handle)
                    .filter(|record| record.kind == b"STYLE")
                    .map(|record| self.string(record, 2))
                    .transpose()?
                    .flatten()
                    .ok_or_else(|| invalid(kind, 7))?,
            ),
            Some(data) => Some(self.decode(kind, 7, data)?),
        };
        let text_height = match object.first(140) {
            None => None,
            Some(Data::Real(height)) if height.is_finite() && *height > 0.0 => {
                Some(length(*height))
            }
            Some(_) => return Err(invalid(kind, 140)),
        };
        let alignment = match object.first(170) {
            None => None,
            Some(Data::Int(value)) => Some(attachment(kind, *value, 170)?),
            Some(_) => return Err(invalid(kind, 170)),
        };
        Ok(CellFormat {
            text_style,
            text_height,
            alignment,
            text_color: color(borders.text_color)?,
            fill: flag(283)?,
            fill_color: color(63)?,
            borders: Borders {
                top: border(borders.top)?,
                right: border(borders.right)?,
                bottom: border(borders.bottom)?,
                left: border(borders.left)?,
                horizontal: borders
                    .horizontal
                    .map(border)
                    .transpose()?
                    .unwrap_or_default(),
                vertical: borders
                    .vertical
                    .map(border)
                    .transpose()?
                    .unwrap_or_default(),
            },
        })
    }

    /// セルの 2007 からの値（301 `CELL_VALUE` から 304 `ACVALUE_END` まで）の、型のある値。印（93）の 1 が立つか、
    /// 型（90）が 0 なら `None`。整数（1）は 91、実数（2）は 140、文字列（4）は 2 の並びと 1、日時（8）は 310 の
    /// SYSTEMTIME（年・月・曜日・日・時・分・秒・ミリ秒の 16 バイト。推定）、点（16・32）は 11・21・31 から読む。
    /// ほかの型（64・128・256・512）は移さない（`element` で診断する）。単位（94）は 0・1・2・4・8・16・32 を
    /// なし・長さ・角度・面積・体積・通貨・百分率とし、書式（300）は空なら `None` とする（docs/formats/dxf.md）
    fn cell_value(&self, cell: &Object, values: &[Value]) -> Lifted<Option<CellValue>> {
        let kind = cell.kind.as_slice();
        let end = values
            .iter()
            .position(|value| value.code == 304)
            .unwrap_or(values.len());
        let object = Object {
            values: values.get(..end).unwrap_or_default().to_vec(),
            ..cell.clone()
        };
        if object.values.is_empty() || self.int(&object, 93, 0)? & 1 != 0 {
            return Ok(None);
        }
        let data = match self.int(&object, 90, 0)? {
            0 | 64 | 128 | 256 | 512 => return Ok(None),
            1 => match object.first(91) {
                Some(Data::Int(value)) => CellData::Integer(*value),
                _ => return Err(invalid(kind, 91)),
            },
            2 => CellData::Real(self.real(&object, 140, None)?),
            4 => {
                let mut text = String::new();
                for value in object
                    .values
                    .iter()
                    .filter(|value| matches!(value.code, 1 | 2))
                {
                    text.push_str(&self.decode(kind, value.code, &value.data)?);
                }
                CellData::Text(text)
            }
            8 => match object.first(310) {
                Some(Data::Binary(bytes)) => {
                    CellData::Date(system_time(bytes).ok_or_else(|| invalid(kind, 310))?)
                }
                _ => return Err(invalid(kind, 310)),
            },
            16 | 32 => CellData::Point([
                self.real(&object, 11, None)?,
                self.real(&object, 21, None)?,
                self.real(&object, 31, Some(0.0))?,
            ]),
            _ => return Err(invalid(kind, 90)),
        };
        let unit = match self.int(&object, 94, 0)? {
            0 => ValueUnit::None,
            1 => ValueUnit::Distance,
            2 => ValueUnit::Angle,
            4 => ValueUnit::Area,
            8 => ValueUnit::Volume,
            16 => ValueUnit::Currency,
            32 => ValueUnit::Percentage,
            _ => return Err(invalid(kind, 94)),
        };
        Ok(Some(CellValue {
            data,
            unit,
            format: self
                .string(&object, 300)?
                .filter(|format| !format.is_empty()),
        }))
    }

    /// ブロックのレコードを指すハンドルを、記号のキーにする。ないか 0 なら `None`
    pub(super) fn block_key(
        &self,
        db: &AcadDb,
        object: &Object,
        code: i16,
    ) -> Lifted<Option<SymbolKey>> {
        let kind = object.kind.as_slice();
        match object.first(code) {
            None | Some(Data::Handle(0)) => Ok(None),
            Some(Data::Handle(handle)) => {
                let record = db
                    .get(*handle)
                    .filter(|record| record.kind == b"BLOCK_RECORD")
                    .ok_or_else(|| invalid(kind, code))?;
                let name = self.string(record, 2)?.ok_or_else(|| invalid(kind, code))?;
                Ok(Some(SymbolKey {
                    namespace: BLOCK,
                    name,
                }))
            }
            Some(_) => Err(invalid(kind, code)),
        }
    }
}

/// セルか表のスタイルの組の、文字の色と枠の辺ごとの色・線幅・表示のグループ・コード
struct BorderCodes {
    text_color: i16,
    top: [i16; 3],
    right: [i16; 3],
    bottom: [i16; 3],
    left: [i16; 3],
    horizontal: Option<[i16; 3]>,
    vertical: Option<[i16; 3]>,
}

/// セルの枠（DXF Reference「ACAD_TABLE」）。上・右・下・左の枠を持つ。左と上と右は、試料の表を描いたブロックの
/// 線の色と線幅で確かめた。下は推定。表示（285〜289）は、DXF が書かない
const CELL_BORDERS: BorderCodes = BorderCodes {
    text_color: 64,
    top: [69, 279, 289],
    right: [65, 275, 285],
    bottom: [66, 276, 286],
    left: [68, 278, 288],
    horizontal: None,
    vertical: None,
};

/// 表のスタイルの組の枠（DXF Reference「TABLESTYLE」。推定）
const STYLE_BORDERS: BorderCodes = BorderCodes {
    text_color: 62,
    top: [64, 274, 284],
    right: [69, 279, 289],
    bottom: [66, 276, 286],
    left: [67, 277, 287],
    horizontal: Some([65, 275, 285]),
    vertical: Some([68, 278, 288]),
};

/// 表のセルの値（301 から 304 まで）のうち、`Lifter::cell_value` が移さない型（64・128・256・512）で、値を持つ
/// （印の 93 の 1 が立たない）ものを診断する
pub(super) fn report_cell_values(object: &Object, cx: &mut Cx) {
    let mut inside = false;
    let (mut flags, mut kind) = (0, 0);
    for value in &object.values {
        match (value.code, &value.data) {
            (301, _) => (inside, flags, kind) = (true, 0, 0),
            (93, Data::Int(value)) if inside => flags = *value,
            (90, Data::Int(value)) if inside => kind = *value,
            (304, _) if inside => {
                inside = false;
                if flags & 1 == 0 && matches!(kind, 64 | 128 | 256 | 512) {
                    cx.report(
                        Diagnostic::new(codes::DXF_CELL_VALUE_DROPPED)
                            .with("type", kind)
                            .at(source(object)),
                    );
                }
            }
            _ => {}
        }
    }
}

/// SYSTEMTIME（年・月・曜日・日・時・分・秒・ミリ秒の、リトル・エンディアンの 16 ビットの並び）。範囲の外なら `None`
fn system_time(bytes: &[u8]) -> Option<DateTime> {
    let ([year, month, _, day, hour, minute, second, millisecond], []) = bytes.as_chunks::<2>()
    else {
        return None;
    };
    let small = |pair: &[u8; 2], range: core::ops::RangeInclusive<u8>| {
        u8::try_from(u16::from_le_bytes(*pair))
            .ok()
            .filter(|value| range.contains(value))
    };
    Some(DateTime {
        year: u16::from_le_bytes(*year),
        month: small(month, 1..=12)?,
        day: small(day, 1..=31)?,
        hour: small(hour, 0..=23)?,
        minute: small(minute, 0..=59)?,
        second: small(second, 0..=59)?,
        millisecond: Some(u16::from_le_bytes(*millisecond)).filter(|ms| *ms < 1000)?,
    })
}
