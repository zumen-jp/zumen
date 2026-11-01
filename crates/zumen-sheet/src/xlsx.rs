//! xlsx（ECMA-376 の SpreadsheetML）。パッケージの関係（`_rels/.rels`）からブックを、ブックの関係からシートと
//! 共有文字列を引く。要素は名前空間を問わず、接頭辞を除いた名前で見る（Transitional と Strict の両方を読む）。

use std::collections::BTreeMap;

use zumen_base::{Cx, Diagnostic, Kind as Budget, codes};
use zumen_sdk::Error;
use zumen_xml::{Document, Element, Node};
use zumen_zip::Archive;

use crate::{Area, Book, Cell, Kind, Sheet, stopped};

/// 関係の種類の名前の終わり
const OFFICE_DOCUMENT: &str = "/officeDocument";
const WORKSHEET: &str = "/worksheet";
const SHARED_STRINGS: &str = "/sharedStrings";

/// ブックの部品の名前（関係がないとき）
const WORKBOOK: &str = "xl/workbook.xml";

fn part_invalid(cx: &mut Cx, name: &str) {
    cx.report(Diagnostic::new(codes::XLSX_PART_INVALID).with("name", name));
}

/// 部品を展開して XML として読む。ないか読めなければ `None`。`required` なら、ないときに `xlsx.part.invalid`
/// を足す
fn part(
    archive: &Archive,
    name: &str,
    required: bool,
    cx: &mut Cx,
) -> Result<Option<Document>, Error> {
    let Some(entry) = archive.find(name) else {
        if required {
            part_invalid(cx, name);
        }
        return Ok(None);
    };
    let bytes = match archive.extract(entry, cx) {
        Ok(bytes) => bytes,
        Err(Error::Stopped(stop)) => return Err(Error::Stopped(stop)),
        // 展開の診断は、展開する処理が足してある
        Err(_) => return Ok(None),
    };
    match zumen_xml::read(&bytes, cx) {
        Ok(doc) => Ok(Some(doc)),
        Err(Error::Stopped(stop)) => Err(Error::Stopped(stop)),
        Err(_) => {
            part_invalid(cx, name);
            Ok(None)
        }
    }
}

/// 子の要素で、接頭辞を除いた名前が `local` のもの
fn children<'d>(
    doc: &'d Document,
    element: &'d Element,
    local: &'d [u8],
) -> impl Iterator<Item = &'d Element> {
    element
        .children()
        .iter()
        .filter_map(move |node| match node {
            Node::Element(index) => doc
                .element(*index)
                .filter(|child| doc.local_name(child.name()) == local),
            _ => None,
        })
}

/// 名前空間を問わない属性の値
fn attribute(doc: &Document, element: &Element, local: &[u8]) -> Option<String> {
    element
        .attributes()
        .iter()
        .find(|attribute| doc.local_name(attribute.name()) == local)
        .map(|attribute| doc.value(attribute))
}

/// 名前空間のある属性（`r:id`）の値
fn qualified(doc: &Document, element: &Element, local: &[u8]) -> Option<String> {
    element
        .attributes()
        .iter()
        .find(|attribute| {
            doc.local_name(attribute.name()) == local && doc.namespace(attribute.name()).is_some()
        })
        .map(|attribute| doc.value(attribute))
}

/// 要素の中の文字（子の要素の中は含めない）
fn text(doc: &Document, element: &Element) -> String {
    let mut out = String::new();
    for node in element.children() {
        match node {
            Node::Text(range) => out.push_str(&doc.text(range.clone())),
            Node::CData(range) => out.push_str(&doc.cdata(range.clone())),
            Node::Element(_) => {}
        }
    }
    out
}

/// `_xHHHH_` の書き方（ST_Xstring）を字に戻す
fn unescape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find("_x") {
        let (before, after) = rest.split_at(at);
        out.push_str(before);
        let code = after
            .get(2..6)
            .filter(|_| after.get(6..7) == Some("_"))
            .and_then(|hex| u32::from_str_radix(hex, 16).ok())
            .and_then(char::from_u32);
        match code {
            Some(c) => {
                out.push(c);
                rest = after.get(7..).unwrap_or_default();
            }
            None => {
                out.push_str("_x");
                rest = after.get(2..).unwrap_or_default();
            }
        }
    }
    out.push_str(rest);
    out
}

/// 文字の項目（`si`・`is`）の値。`t` と、書式つきの区切り（`r`）の `t` をつなぐ。ふりがな（`rPh`）は含めない
fn rich(doc: &Document, element: &Element) -> String {
    let mut out = String::new();
    for node in element.children() {
        let Node::Element(index) = node else {
            continue;
        };
        let Some(child) = doc.element(*index) else {
            continue;
        };
        match doc.local_name(child.name()) {
            b"t" => out.push_str(&text(doc, child)),
            b"r" => {
                for t in children(doc, child, b"t") {
                    out.push_str(&text(doc, t));
                }
            }
            _ => {}
        }
    }
    unescape(&out)
}

/// 関係の部品の名前（`xl/workbook.xml` なら `xl/_rels/workbook.xml.rels`）
fn rels_name(name: &str) -> String {
    match name.rsplit_once('/') {
        Some((dir, file)) => format!("{dir}/_rels/{file}.rels"),
        None => format!("_rels/{name}.rels"),
    }
}

/// 関係の行き先を、部品 `from` からの相対として、書庫の中の名前にする
fn resolve(from: &str, target: &str) -> String {
    if let Some(absolute) = target.strip_prefix('/') {
        return absolute.to_string();
    }
    let mut parts: Vec<&str> = match from.rsplit_once('/') {
        Some((dir, _)) => dir.split('/').collect(),
        None => Vec::new(),
    };
    for segment in target.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            _ => parts.push(segment),
        }
    }
    parts.join("/")
}

/// 関係の部品の、Id から（種類, 行き先の名前）の表
fn relationships(
    archive: &Archive,
    from: &str,
    cx: &mut Cx,
) -> Result<BTreeMap<String, (String, String)>, Error> {
    let mut out = BTreeMap::new();
    let name = if from.is_empty() {
        "_rels/.rels".to_string()
    } else {
        rels_name(from)
    };
    let Some(doc) = part(archive, &name, false, cx)? else {
        return Ok(out);
    };
    let Some(root) = doc.element(doc.root()) else {
        return Ok(out);
    };
    for relationship in children(&doc, root, b"Relationship") {
        let (Some(id), Some(kind), Some(target)) = (
            attribute(&doc, relationship, b"Id"),
            attribute(&doc, relationship, b"Type"),
            attribute(&doc, relationship, b"Target"),
        ) else {
            continue;
        };
        if attribute(&doc, relationship, b"TargetMode").as_deref() == Some("External") {
            continue;
        }
        out.insert(id, (kind, resolve(from, &target)));
    }
    Ok(out)
}

/// `A1` の形の参照を、0 から数えた（行, 列）にする
fn reference(text: &str) -> Option<(u32, u32)> {
    let text = text.trim_start_matches('$');
    let letters = text.bytes().take_while(u8::is_ascii_alphabetic).count();
    let (letters, digits) = text.split_at(letters);
    let digits = digits.trim_start_matches('$');
    if letters.is_empty() || letters.len() > 3 {
        return None;
    }
    let mut column = 0u32;
    for b in letters.bytes() {
        column = column * 26 + u32::from(b.to_ascii_uppercase() - b'A' + 1);
    }
    let row: u32 = digits.parse().ok()?;
    Some((row.checked_sub(1)?, column - 1))
}

fn area(text: &str) -> Option<Area> {
    let (first, last) = text.split_once(':').unwrap_or((text, text));
    let (first_row, first_column) = reference(first)?;
    let (last_row, last_column) = reference(last)?;
    (first_row <= last_row && first_column <= last_column).then_some(Area {
        first_row,
        first_column,
        last_row,
        last_column,
    })
}

fn cell_invalid(cx: &mut Cx, sheet: &str, at: &str) {
    cx.report(
        Diagnostic::new(codes::XLSX_CELL_INVALID)
            .with("sheet", sheet)
            .with("reference", at),
    );
}

/// ワークシートのセルと結合セルを読む
fn worksheet(
    doc: &Document,
    sheet: &mut Sheet,
    strings: &[String],
    cx: &mut Cx,
) -> Result<(), Error> {
    let Some(root) = doc.element(doc.root()) else {
        return Ok(());
    };
    let mut next_row = 0u32;
    for data in children(doc, root, b"sheetData") {
        for row in children(doc, data, b"row") {
            let row_at = match attribute(doc, row, b"r") {
                Some(r) => match r.parse::<u32>().ok().and_then(|r| r.checked_sub(1)) {
                    Some(r) => r,
                    None => {
                        cell_invalid(cx, &sheet.name, &r);
                        continue;
                    }
                },
                None => next_row,
            };
            next_row = row_at.saturating_add(1);
            let mut next_column = 0u32;
            for c in children(doc, row, b"c") {
                cx.check(Budget::Records, 1)
                    .map_err(|stop| stopped(cx, stop))?;
                let (row_index, column) = match attribute(doc, c, b"r") {
                    Some(r) => match reference(&r) {
                        Some(at) => at,
                        None => {
                            cell_invalid(cx, &sheet.name, &r);
                            continue;
                        }
                    },
                    None => (row_at, next_column),
                };
                next_column = column.saturating_add(1);
                let kind = attribute(doc, c, b"t").unwrap_or_default();
                let raw = children(doc, c, b"v").next().map(|v| text(doc, v));
                let formula = children(doc, c, b"f")
                    .next()
                    .map(|f| text(doc, f))
                    .filter(|f| !f.is_empty());
                let (kind, value) = match kind.as_str() {
                    "s" => {
                        let Some(raw) = raw else {
                            continue;
                        };
                        match raw
                            .trim()
                            .parse::<usize>()
                            .ok()
                            .and_then(|i| strings.get(i))
                        {
                            Some(value) => (Kind::Text, value.clone()),
                            None => {
                                let at = attribute(doc, c, b"r").unwrap_or_default();
                                cell_invalid(cx, &sheet.name, &at);
                                continue;
                            }
                        }
                    }
                    "inlineStr" => match children(doc, c, b"is").next() {
                        Some(is) => (Kind::Text, rich(doc, is)),
                        None => (Kind::Text, String::new()),
                    },
                    "str" => (Kind::Text, unescape(&raw.unwrap_or_default())),
                    "b" => {
                        let value = match raw.as_deref().map(str::trim) {
                            Some("1") => "TRUE",
                            Some("0") => "FALSE",
                            _ => "",
                        };
                        (Kind::Boolean, value.to_string())
                    }
                    "e" => (Kind::Error, raw.unwrap_or_default()),
                    // 日付（ISO 8601 の字）は、字のまま返す
                    "d" => (Kind::Text, raw.unwrap_or_default()),
                    _ => (Kind::Number, raw.unwrap_or_default().trim().to_string()),
                };
                if value.is_empty() && formula.is_none() {
                    continue;
                }
                sheet.cells.push(Cell {
                    row: row_index,
                    column,
                    kind,
                    value,
                    formula,
                });
            }
        }
    }
    for list in children(doc, root, b"mergeCells") {
        for merge in children(doc, list, b"mergeCell") {
            let text = attribute(doc, merge, b"ref").unwrap_or_default();
            match area(&text) {
                Some(found) => sheet.merged.push(found),
                None => cell_invalid(cx, &sheet.name, &text),
            }
        }
    }
    // ファイルの順が行と列の順でなくても、行の順、行の中は列の順に並べる
    sheet.cells.sort_by_key(|cell| (cell.row, cell.column));
    Ok(())
}

/// xlsx を読む。ZIP でないか、ブックの部品がなければ失敗する。読めないシートは `xlsx.part.invalid` を足して
/// 飛ばし、ワークシートでないシート（グラフのシートなど）は `xlsx.sheet.unsupported` を足して飛ばす
pub fn read(src: &[u8], cx: &mut Cx) -> Result<Book, Error> {
    let archive = zumen_zip::read(src, cx)?;
    let package = relationships(&archive, "", cx)?;
    let workbook_name = package
        .values()
        .find(|(kind, _)| kind.ends_with(OFFICE_DOCUMENT))
        .map_or_else(|| WORKBOOK.to_string(), |(_, target)| target.clone());
    let Some(workbook) = part(&archive, &workbook_name, false, cx)? else {
        cx.report(Diagnostic::new(codes::XLSX_FILE_INVALID));
        return Err(Error::Invalid);
    };
    let rels = relationships(&archive, &workbook_name, cx)?;
    let mut strings = Vec::new();
    if let Some((_, name)) = rels
        .values()
        .find(|(kind, _)| kind.ends_with(SHARED_STRINGS))
        && let Some(doc) = part(&archive, name, true, cx)?
        && let Some(root) = doc.element(doc.root())
    {
        for si in children(&doc, root, b"si") {
            cx.check(Budget::Records, 1)
                .map_err(|stop| stopped(cx, stop))?;
            strings.push(rich(&doc, si));
        }
    }
    let mut book = Book::default();
    let Some(root) = workbook.element(workbook.root()) else {
        return Ok(book);
    };
    for list in children(&workbook, root, b"sheets") {
        for entry in children(&workbook, list, b"sheet") {
            let name = attribute(&workbook, entry, b"name").unwrap_or_default();
            let target = qualified(&workbook, entry, b"id").and_then(|id| rels.get(&id));
            let Some((kind, target)) = target else {
                part_invalid(cx, &name);
                continue;
            };
            if !kind.ends_with(WORKSHEET) {
                cx.report(
                    Diagnostic::new(codes::XLSX_SHEET_UNSUPPORTED).with("name", name.as_str()),
                );
                continue;
            }
            let Some(doc) = part(&archive, target, true, cx)? else {
                continue;
            };
            let mut sheet = Sheet {
                name,
                ..Sheet::default()
            };
            worksheet(&doc, &mut sheet, &strings, cx)?;
            sheet.measure();
            book.sheets.push(sheet);
        }
    }
    Ok(book)
}
