//! DXF の版の変更（決定 0008）。いまは AC1012〜AC1032 から AC1009（R12）への変更だけを扱う。
//!
//! R12 が持たないセクション・表・図形の種類・グループ・コードのタグは書かず、診断で返す。
//! サブクラスの印（100）、所有者への参照（330・360）、アプリケーションの組（102）、
//! 表のハンドル（TABLES の中の 5 と、105）は、R12 に置き換える先のない構造なので、黙って除く。
//! 残すタグは、グループ・コードの行と改行を元のバイトのまま書く。
//! 何を R12 が持つとしたかは推定で、`docs/formats/dxf.md` に記録した。

use super::{Dxf, DxfFile, Record, Tag, stopped};
use zumen_base::{Cx, Diagnostic, Kind, Ref, codes};
use zumen_sdk::{Codec, Error};

const R12: &[u8] = b"AC1009";

/// R12 に変えられる元の版の範囲（R13 から 2018 まで）
const FROM: core::ops::RangeInclusive<&[u8]> = b"AC1012".as_slice()..=b"AC1032".as_slice();

/// UTF-8 で書く最初の版（2007）
const UTF8: &[u8] = b"AC1021";

const SECTIONS: &[&[u8]] = &[b"HEADER", b"TABLES", b"BLOCKS", b"ENTITIES"];

const TABLES: &[&[u8]] = &[
    b"VPORT",
    b"LTYPE",
    b"LAYER",
    b"STYLE",
    b"VIEW",
    b"UCS",
    b"DIMSTYLE",
    b"APPID",
];

const ENTITIES: &[&[u8]] = &[
    b"3DFACE",
    b"ARC",
    b"ATTDEF",
    b"ATTRIB",
    b"BLOCK",
    b"CIRCLE",
    b"DIMENSION",
    b"ENDBLK",
    b"INSERT",
    b"LINE",
    b"POINT",
    b"POLYLINE",
    b"SEQEND",
    b"SHAPE",
    b"SOLID",
    b"TEXT",
    b"TRACE",
    b"VERTEX",
    b"VIEWPORT",
];

impl DxfFile {
    /// 見出しの `$ACADVER` の値（前後の空白を除く）。なければ `None`
    pub fn version(&self) -> Option<&[u8]> {
        self.records
            .iter()
            .filter(|record| self.kind(record) == b"SECTION")
            .find_map(|record| {
                let mut tags = record.tags.iter();
                tags.by_ref()
                    .find(|t| t.code == 9 && self.value(t).trim_ascii() == b"$ACADVER")?;
                tags.next().map(|t| self.value(t).trim_ascii())
            })
    }
}

impl Dxf {
    /// 版を `version`（`$ACADVER` の値）に変えて書く。元と同じ版なら [`Codec::write`] と同じ。
    /// 扱えない組なら `dxf.version.unsupported` を返し、`out` には何も足さない
    pub fn write_as(
        file: &DxfFile,
        version: &[u8],
        out: &mut Vec<u8>,
        cx: &mut Cx,
    ) -> Result<(), Error> {
        let from = file.version().unwrap_or_default();
        if from == version {
            return Dxf::write(file, out, cx);
        }
        if version != R12 || !FROM.contains(&from) {
            cx.report(
                Diagnostic::new(codes::DXF_VERSION_UNSUPPORTED)
                    .with("from", text(from))
                    .with("to", text(version)),
            );
            return Err(Error::Unsupported);
        }
        let mut buf = Vec::new();
        R12Writer {
            file,
            unicode: from >= UTF8,
            out: &mut buf,
            cx,
        }
        .run()?;
        out.append(&mut buf);
        Ok(())
    }
}

struct R12Writer<'a, 'c> {
    file: &'a DxfFile,
    /// 元が UTF-8 の版なら、ASCII でない文字を `\U+XXXX` にする
    unicode: bool,
    out: &'a mut Vec<u8>,
    cx: &'a mut Cx<'c>,
}

impl R12Writer<'_, '_> {
    fn run(&mut self) -> Result<(), Error> {
        let file = self.file;
        let mut section: &[u8] = b"";
        // 書かない範囲を終えるレコードの種類（ENDSEC か ENDTAB）
        let mut skip: Option<&[u8]> = None;
        for (index, record) in file.records.iter().enumerate() {
            self.cx
                .check(Kind::Work, 1)
                .map_err(|stop| stopped(self.cx, stop))?;
            let kind = file.kind(record);
            if let Some(end) = skip {
                if kind == end {
                    skip = None;
                }
                continue;
            }
            match kind {
                b"SECTION" => {
                    let name = name(file, record);
                    if !SECTIONS.contains(&name) {
                        self.drop(
                            Diagnostic::new(codes::DXF_SECTION_DROPPED).with("name", text(name)),
                            index,
                        );
                        skip = Some(b"ENDSEC");
                        continue;
                    }
                    section = name;
                    if name == b"HEADER" {
                        self.header(index, record);
                        continue;
                    }
                }
                b"ENDSEC" => section = b"",
                b"TABLE" if section == b"TABLES" => {
                    let name = name(file, record);
                    if !TABLES.contains(&name) {
                        self.drop(
                            Diagnostic::new(codes::DXF_TABLE_DROPPED).with("name", text(name)),
                            index,
                        );
                        skip = Some(b"ENDTAB");
                        continue;
                    }
                }
                _ if (section == b"ENTITIES" || section == b"BLOCKS")
                    && !ENTITIES.contains(&kind) =>
                {
                    self.drop(
                        Diagnostic::new(codes::DXF_ENTITY_DROPPED).with("kind", text(kind)),
                        index,
                    );
                    continue;
                }
                _ => {}
            }
            self.record(index, record, section == b"TABLES");
        }
        self.out.extend_from_slice(file.trailer());
        Ok(())
    }

    /// 見出し。R12 が持たないグループ・コードの値を持つ変数は、変数ごと書かない
    fn header(&mut self, index: usize, record: &Record) {
        let file = self.file;
        let mut handling = false;
        let mut acadver: Option<&Tag> = None;
        for (n, chunk) in record.tags.chunk_by(|_, b| b.code != 9).enumerate() {
            let mut tags = chunk.iter();
            if n == 0 {
                // SECTION と HEADER のタグ
                tags.for_each(|tag| self.tag(index, tag, file.value(tag)));
                continue;
            }
            let Some(head) = tags.next() else { continue };
            let name = file.value(head).trim_ascii();
            if tags.clone().any(|t| !r12_code(t.code)) {
                self.drop(
                    Diagnostic::new(codes::DXF_HEADER_VARIABLE_DROPPED).with("name", text(name)),
                    index,
                );
                continue;
            }
            self.tag(index, head, file.value(head));
            match name {
                b"$ACADVER" => {
                    acadver = Some(head);
                    tags.for_each(|tag| self.tag(index, tag, R12));
                }
                _ => {
                    handling |= name == b"$HANDLING";
                    tags.for_each(|tag| self.tag(index, tag, file.value(tag)));
                }
            }
        }
        // 図形のハンドルを残すので、ハンドルを使うと明記する
        if let (false, Some(head)) = (handling, acadver) {
            let code_line = file
                .src
                .get(head.span.start..head.value.start)
                .unwrap_or_default();
            let newline = file
                .src
                .get(head.value.end..head.span.end)
                .unwrap_or_default();
            let width = code_line.trim_ascii_end().len();
            self.out.extend_from_slice(code_line);
            self.out.extend_from_slice(b"$HANDLING");
            self.out.extend_from_slice(newline);
            self.out
                .extend_from_slice(format!("{:>width$}", 70).as_bytes());
            self.out.extend_from_slice(newline);
            self.out.extend_from_slice(b"1");
            self.out.extend_from_slice(newline);
        }
    }

    fn record(&mut self, index: usize, record: &Record, in_tables: bool) {
        let file = self.file;
        let mut in_group = false;
        for tag in &record.tags {
            let value = file.value(tag);
            if in_group {
                in_group = !(tag.code == 102 && value.trim_ascii() == b"}");
                continue;
            }
            match tag.code {
                102 => in_group = value.trim_ascii().starts_with(b"{"),
                100 | 105 | 330 | 360 => {}
                5 if in_tables => {}
                code if !r12_code(code) => self.drop(
                    Diagnostic::new(codes::DXF_GROUP_CODE_DROPPED)
                        .with("kind", text(file.kind(record)))
                        .with("code", i64::from(code)),
                    index,
                ),
                _ => self.tag(index, tag, value),
            }
        }
    }

    /// タグを、グループ・コードの行と改行を元のまま、値を `value` にして書く
    fn tag(&mut self, index: usize, tag: &Tag, value: &[u8]) {
        let src = &self.file.src;
        self.out
            .extend_from_slice(src.get(tag.span.start..tag.value.start).unwrap_or_default());
        match core::str::from_utf8(value) {
            _ if !self.unicode || value.is_ascii() => self.out.extend_from_slice(value),
            Ok(value) => {
                escape(value, self.out);
                self.cx.report(
                    Diagnostic::new(codes::DXF_TEXT_EQUIVALENT).at(Ref::at_record(index as u64)),
                );
            }
            Err(_) => {
                self.out.extend_from_slice(value);
                self.cx.report(
                    Diagnostic::new(codes::DXF_TEXT_INVALID).at(Ref::at_record(index as u64)),
                );
            }
        }
        self.out
            .extend_from_slice(src.get(tag.value.end..tag.span.end).unwrap_or_default());
    }

    fn drop(&mut self, diagnostic: Diagnostic, index: usize) {
        self.cx.report(diagnostic.at(Ref::at_record(index as u64)));
    }
}

/// R12 のグループ・コード（推定。docs/formats/dxf.md）
fn r12_code(code: i16) -> bool {
    matches!(code, 0..=79 | 210..=239 | 999 | 1000..=1071)
}

/// 最初のグループ・コード 2 の値（前後の空白を除く）
pub(super) fn name<'a>(file: &'a DxfFile, record: &'a Record) -> &'a [u8] {
    record
        .tags
        .iter()
        .find(|t| t.code == 2)
        .map(|t| file.value(t).trim_ascii())
        .unwrap_or_default()
}

pub(super) fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// ASCII でない文字を `\U+XXXX`（UTF-16 の単位ごと、16 進は大文字）にする
fn escape(value: &str, out: &mut Vec<u8>) {
    let mut units = [0u16; 2];
    let mut ascii = [0u8; 4];
    for c in value.chars() {
        if c.is_ascii() {
            out.extend_from_slice(c.encode_utf8(&mut ascii).as_bytes());
        } else {
            for unit in c.encode_utf16(&mut units) {
                out.extend_from_slice(format!("\\U+{unit:04X}").as_bytes());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_use_utf16_units() {
        let mut out = Vec::new();
        escape("a図😀", &mut out);
        assert_eq!(out, b"a\\U+56F3\\U+D83D\\U+DE00");
    }
}
