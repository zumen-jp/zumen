//! 表（#391）。Excel の xlsx（ECMA-376 の SpreadsheetML）と CSV（RFC 4180）を、様式を知らずに読む。
//!
//! 返すのは、シートごとのセルの値・数式・結合セルである。数式は計算せず、ファイルに保存された値を返す。
//! 書式（日付の表示形式など）は解釈しないので、日付は保存された数のまま返す。

mod csv;
mod xlsx;

use zumen_base::{Cx, Stop};
use zumen_sdk::Error;

pub use csv::read as read_csv;
pub use xlsx::read as read_xlsx;

/// 読んだ表
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Book {
    /// シート。ブックの順。CSV は 1 つ
    pub sheets: Vec<Sheet>,
    /// CSV を読んだ文字コード。xlsx は `None`
    pub encoding: Option<Encoding>,
}

/// シート
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Sheet {
    /// 名前。CSV は空
    pub name: String,
    /// 行の数（値のある最後の行まで）
    pub rows: u32,
    /// 列の数（値のある最後の列まで）
    pub columns: u32,
    /// 値か数式のあるセル。行の順、行の中は列の順
    pub cells: Vec<Cell>,
    /// 結合セル。ファイルの順
    pub merged: Vec<Area>,
}

/// セル。行と列は 0 から数える
#[derive(Clone, Debug, PartialEq)]
pub struct Cell {
    /// 行
    pub row: u32,
    /// 列
    pub column: u32,
    /// 値の種類
    pub kind: Kind,
    /// 値。数は保存された字のまま。真偽は `TRUE`・`FALSE`
    pub value: String,
    /// 数式（先頭の `=` を除く）。共有の数式は、元のセルだけが持つ
    pub formula: Option<String>,
}

/// セルの値の種類
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// 文字
    Text,
    /// 数
    Number,
    /// 真偽
    Boolean,
    /// エラーの値（`#DIV/0!` など）
    Error,
}

impl Kind {
    /// 境界の名前
    pub fn name(self) -> &'static str {
        match self {
            Kind::Text => "text",
            Kind::Number => "number",
            Kind::Boolean => "boolean",
            Kind::Error => "error",
        }
    }
}

/// 結合セルの範囲。両端を含む
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Area {
    /// 最初の行
    pub first_row: u32,
    /// 最初の列
    pub first_column: u32,
    /// 最後の行
    pub last_row: u32,
    /// 最後の列
    pub last_column: u32,
}

/// CSV の文字コード
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Encoding {
    /// UTF-8
    Utf8,
    /// Shift_JIS（Windows の拡張を含む）
    ShiftJis,
}

impl Encoding {
    /// 境界の名前
    pub fn name(self) -> &'static str {
        match self {
            Encoding::Utf8 => "utf_8",
            Encoding::ShiftJis => "shift_jis",
        }
    }
}

impl Sheet {
    /// セルと結合セルから、行と列の数を決める
    fn measure(&mut self) {
        let cells = self.cells.iter().map(|cell| (cell.row, cell.column));
        let areas = self
            .merged
            .iter()
            .map(|area| (area.last_row, area.last_column));
        for (row, column) in cells.chain(areas) {
            self.rows = self.rows.max(row.saturating_add(1));
            self.columns = self.columns.max(column.saturating_add(1));
        }
    }
}

fn stopped(cx: &mut Cx, stop: Stop) -> Error {
    cx.report(stop.to_diagnostic());
    Error::Stopped(stop)
}
