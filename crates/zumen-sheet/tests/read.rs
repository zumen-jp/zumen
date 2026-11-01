//! 試験の中で作った xlsx と CSV を読む（#391）。

use zumen_base::{Budget, CancelToken, Code, Cx, Diagnostics, codes};
use zumen_sheet::{Area, Book, Encoding, Kind, read_csv, read_xlsx};

const MAIN: &str = r#"xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships""#;
const REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

#[cfg(test)]
fn with_cx<T>(f: impl FnOnce(&mut Cx<'_>) -> T) -> (T, Vec<Code>) {
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let out = {
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        f(&mut cx)
    };
    (out, diag.iter().map(|d| d.code()).collect())
}

#[cfg(test)]
fn xlsx() -> Vec<u8> {
    let package = format!(
        r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="{REL}/officeDocument" Target="xl/workbook.xml"/></Relationships>"#
    );
    let workbook = format!(
        r#"<workbook {MAIN}><sheets><sheet name="内訳" sheetId="1" r:id="rId1"/><sheet name="グラフ" sheetId="2" r:id="rId2"/><sheet name="数量" sheetId="3" r:id="rId3"/></sheets></workbook>"#
    );
    let rels = format!(
        r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="{REL}/worksheet" Target="worksheets/sheet1.xml"/><Relationship Id="rId2" Type="{REL}/chartsheet" Target="chartsheets/sheet1.xml"/><Relationship Id="rId3" Type="{REL}/worksheet" Target="/xl/worksheets/sheet2.xml"/><Relationship Id="rId4" Type="{REL}/sharedStrings" Target="sharedStrings.xml"/></Relationships>"#
    );
    let strings = format!(
        r#"<sst {MAIN}><si><t>工種</t></si><si><r><t>土</t></r><r><rPr><b/></rPr><t>工</t></r><rPh sb="0" eb="1"><t>ド</t></rPh></si><si><t xml:space="preserve">一行目_x000D_
二行目 </t></si></sst>"#
    );
    let sheet1 = format!(
        r#"<worksheet {MAIN}><sheetData>
<row r="1"><c r="A1" t="s"><v>0</v></c><c r="B1" t="s"><v>1</v></c><c r="C1" s="1"/></row>
<row r="3"><c r="B3"><v>12.5</v></c><c r="C3"><f>B3*2</f><v>25</v></c><c r="A3" t="b"><v>1</v></c></row>
<row><c t="e"><f>1/0</f><v>#DIV/0!</v></c><c t="inlineStr"><is><t>直接</t></is></c><c t="str"><f t="shared" ref="C4:C5" si="0">A1&amp;"x"</f><v>工種x</v></c></row>
<row r="5"><c r="C5" t="str"><f t="shared" si="0"/><v>土工x</v></c><c r="D5" t="s"><v>9</v></c><c r="1E"><v>1</v></c></row>
</sheetData><mergeCells count="2"><mergeCell ref="A1:B2"/><mergeCell ref="E7:D6"/></mergeCells></worksheet>"#
    );
    let sheet2 = format!(
        r#"<worksheet {MAIN}><sheetData><row r="2"><c r="AA2" t="s"><v>2</v></c></row></sheetData></worksheet>"#
    );
    zumen_zip::write(&[
        ("[Content_Types].xml", b"<Types/>".as_slice()),
        ("_rels/.rels", package.as_bytes()),
        ("xl/workbook.xml", workbook.as_bytes()),
        ("xl/_rels/workbook.xml.rels", rels.as_bytes()),
        ("xl/sharedStrings.xml", strings.as_bytes()),
        ("xl/worksheets/sheet1.xml", sheet1.as_bytes()),
        ("xl/worksheets/sheet2.xml", sheet2.as_bytes()),
    ])
    .unwrap()
}

#[cfg(test)]
fn cells(book: &Book, sheet: usize) -> Vec<(u32, u32, Kind, &str, Option<&str>)> {
    book.sheets[sheet]
        .cells
        .iter()
        .map(|c| {
            (
                c.row,
                c.column,
                c.kind,
                c.value.as_str(),
                c.formula.as_deref(),
            )
        })
        .collect()
}

#[test]
fn xlsx_values_formulas_and_merged_cells_are_read() {
    let (book, diag) = with_cx(|cx| read_xlsx(&xlsx(), cx));
    let book = book.unwrap();
    // グラフのシート。範囲を超える共有文字列の番号・読めない参照・逆の範囲の結合は、同じ診断にまとまる
    assert_eq!(diag.len(), 2, "{diag:?}");
    assert!(diag.contains(&codes::XLSX_SHEET_UNSUPPORTED));
    assert!(diag.contains(&codes::XLSX_CELL_INVALID));
    let names: Vec<&str> = book.sheets.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, ["内訳", "数量"]);
    assert_eq!(book.encoding, None);
    assert_eq!(
        cells(&book, 0),
        [
            (0, 0, Kind::Text, "工種", None),
            (0, 1, Kind::Text, "土工", None),
            (2, 0, Kind::Boolean, "TRUE", None),
            (2, 1, Kind::Number, "12.5", None),
            (2, 2, Kind::Number, "25", Some("B3*2")),
            (3, 0, Kind::Error, "#DIV/0!", Some("1/0")),
            (3, 1, Kind::Text, "直接", None),
            (3, 2, Kind::Text, "工種x", Some("A1&\"x\"")),
            (4, 2, Kind::Text, "土工x", None),
        ]
    );
    let sheet = &book.sheets[0];
    assert_eq!(
        sheet.merged,
        [Area {
            first_row: 0,
            first_column: 0,
            last_row: 1,
            last_column: 1
        }]
    );
    assert_eq!((sheet.rows, sheet.columns), (5, 3));
    assert_eq!(
        cells(&book, 1),
        [(1, 26, Kind::Text, "一行目\r\n二行目 ", None)]
    );
    assert_eq!((book.sheets[1].rows, book.sheets[1].columns), (2, 27));
}

#[test]
fn not_a_workbook_fails() {
    let (book, diag) = with_cx(|cx| {
        read_xlsx(
            &zumen_zip::write(&[("a.txt", b"x".as_slice())]).unwrap(),
            cx,
        )
    });
    assert!(book.is_err());
    assert_eq!(diag, [codes::XLSX_FILE_INVALID]);
    let (book, _) = with_cx(|cx| read_xlsx(b"not a zip", cx));
    assert!(book.is_err());
}

#[test]
fn csv_quotes_line_breaks_and_encodings_are_read() {
    let src =
        "\u{FEFF}工種,数量,備考\r\n\"土工, 掘削\",12.5,\"「\"\"注\"\"」\r\n2 行\"\r\n\r\n,,末尾\n"
            .as_bytes();
    let (book, diag) = with_cx(|cx| read_csv(src, None, ',', cx));
    assert!(diag.is_empty(), "{diag:?}");
    let book = book.unwrap();
    assert_eq!(book.encoding, Some(Encoding::Utf8));
    assert_eq!(
        cells(&book, 0),
        [
            (0, 0, Kind::Text, "工種", None),
            (0, 1, Kind::Text, "数量", None),
            (0, 2, Kind::Text, "備考", None),
            (1, 0, Kind::Text, "土工, 掘削", None),
            (1, 1, Kind::Text, "12.5", None),
            (1, 2, Kind::Text, "「\"注\"」\r\n2 行", None),
            (3, 2, Kind::Text, "末尾", None),
        ]
    );
    assert_eq!((book.sheets[0].rows, book.sheets[0].columns), (4, 3));

    // Shift_JIS（「工種」「数量」）、タブの区切り
    let sjis = b"\x8d\x48\x8e\xed\t\x90\x94\x97\xca\n";
    let (book, diag) = with_cx(|cx| read_csv(sjis, None, '\t', cx));
    assert!(diag.is_empty(), "{diag:?}");
    let book = book.unwrap();
    assert_eq!(book.encoding, Some(Encoding::ShiftJis));
    assert_eq!(
        cells(&book, 0),
        [
            (0, 0, Kind::Text, "工種", None),
            (0, 1, Kind::Text, "数量", None)
        ]
    );
}

#[test]
fn broken_csv_is_reported() {
    let (book, diag) = with_cx(|cx| read_csv(b"a,\"b\nc", None, ',', cx));
    assert_eq!(
        cells(&book.unwrap(), 0),
        [
            (0, 0, Kind::Text, "a", None),
            (0, 1, Kind::Text, "b\nc", None)
        ]
    );
    assert_eq!(diag, [codes::CSV_QUOTE_INVALID]);
    let (book, diag) = with_cx(|cx| read_csv(b"\xff\xfe", Some(Encoding::Utf8), ',', cx));
    assert_eq!(book.unwrap().sheets[0].cells[0].value, "\u{FFFD}\u{FFFD}");
    assert_eq!(diag, [codes::CSV_ENCODING_INVALID]);
}
