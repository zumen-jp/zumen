//! DXF のレコードから、系統で共有するオブジェクトの表を作る（決定 0023）。
//!
//! セクションの中のグループ・コード 0 のレコードを、1 つのオブジェクトにする。`SECTION`・`ENDSEC`・`EOF` の
//! 印は、オブジェクトにしない。見出しの変数は、オブジェクトでなく変数として置く。値は、グループ・コードの
//! 範囲で決めた型で読む。型どおりに読めない値は、元のバイト列のまま文字列として置き、診断を返す。

use super::{DxfFile, Record, Tag, parse_real, stopped};
use crate::db::{AcadDb, Data, DataType, Object, Origin, Section, Value};
use zumen_base::{Cx, Diagnostic, Kind, Ref, codes};
use zumen_sdk::Error;

impl DxfFile {
    /// オブジェクトの表を作る。予算の上限か中断の合図で止めたときだけ失敗する
    pub fn objects(&self, cx: &mut Cx) -> Result<AcadDb, Error> {
        self.table(cx, true)
    }

    /// オブジェクトの表を作る。`report` が偽なら、型どおりに読めない値の診断を返さない
    /// （図面モデルへの変換は、その値を使うときに自分の診断を返す）
    pub(crate) fn table(&self, cx: &mut Cx, report: bool) -> Result<AcadDb, Error> {
        let mut db = AcadDb::default();
        let mut section: Option<Section> = None;
        for (index, record) in self.records.iter().enumerate() {
            cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
            match self.kind(record) {
                b"SECTION" => {
                    let found = self.section(record);
                    if found == Section::Other(b"HEADER".to_vec()) {
                        self.variables(record, index, report, &mut db, cx);
                    }
                    section = Some(found);
                }
                b"ENDSEC" => section = None,
                b"EOF" => break,
                kind => {
                    // セクションの外のもの（先頭のコメントなど）は、オブジェクトにしない
                    let Some(section) = &section else { continue };
                    let values = record
                        .tags
                        .get(1..)
                        .unwrap_or_default()
                        .iter()
                        .map(|tag| self.typed(tag, kind, index, report, cx))
                        .collect();
                    db.push(Object {
                        kind: kind.to_vec(),
                        section: section.clone(),
                        values,
                        origin: Origin::Dxf { record: index },
                    });
                }
            }
        }
        Ok(db)
    }

    /// SECTION のレコードの名前（2）から、属する所を決める
    pub(super) fn section(&self, record: &Record) -> Section {
        let name = record
            .tags
            .iter()
            .find(|tag| tag.code == 2)
            .map(|tag| self.value(tag).trim_ascii())
            .unwrap_or_default();
        match name {
            b"CLASSES" => Section::Classes,
            b"TABLES" => Section::Tables,
            b"BLOCKS" => Section::Blocks,
            b"ENTITIES" => Section::Entities,
            b"OBJECTS" => Section::Objects,
            other => Section::Other(other.to_vec()),
        }
    }

    /// HEADER のレコードの変数（9 の名前と、続く値）を置く。同じ名前が 2 度あれば、最初のもの
    fn variables(&self, record: &Record, index: usize, report: bool, db: &mut AcadDb, cx: &mut Cx) {
        for chunk in record.tags.chunk_by(|_, b| b.code != 9) {
            let Some((head, tags)) = chunk.split_first() else {
                continue;
            };
            let name = self.value(head).trim_ascii();
            if head.code != 9 || db.variable(name).is_some() {
                continue;
            }
            let values = tags
                .iter()
                .map(|tag| self.typed(tag, b"HEADER", index, report, cx))
                .collect();
            db.set_variable(name, values);
        }
    }

    fn typed(&self, tag: &Tag, kind: &[u8], index: usize, report: bool, cx: &mut Cx) -> Value {
        let raw = self.value(tag);
        let text = raw.trim_ascii();
        let data = match DataType::of(tag.code) {
            DataType::Text => Some(Data::Text(raw.to_vec())),
            DataType::Int => parse_int(text).map(Data::Int),
            DataType::Real => parse_real(raw).map(Data::Real),
            DataType::Bool => match parse_int(text) {
                Some(0) => Some(Data::Bool(false)),
                Some(1) => Some(Data::Bool(true)),
                _ => None,
            },
            DataType::Handle => parse_hex(text).map(Data::Handle),
            DataType::Binary => parse_binary(text).map(Data::Binary),
        };
        let data = data.unwrap_or_else(|| {
            if !report {
                return Data::Text(raw.to_vec());
            }
            cx.report(
                Diagnostic::new(codes::DXF_VALUE_INVALID)
                    .with("kind", String::from_utf8_lossy(kind).into_owned())
                    .with("code", i64::from(tag.code))
                    .at(Ref::at_record(index as u64)),
            );
            Data::Text(raw.to_vec())
        });
        Value {
            code: tag.code,
            data,
        }
    }
}

fn parse_int(text: &[u8]) -> Option<i64> {
    core::str::from_utf8(text).ok()?.parse().ok()
}

/// 16 進のハンドル。空のものは読めないとする
fn parse_hex(text: &[u8]) -> Option<u64> {
    let text = core::str::from_utf8(text).ok()?;
    if text.is_empty() || text.starts_with(['+', '-']) {
        return None;
    }
    u64::from_str_radix(text, 16).ok()
}

/// 16 進の 2 文字ずつで 1 バイトのバイナリ
fn parse_binary(text: &[u8]) -> Option<Vec<u8>> {
    let (pairs, rest) = text.as_chunks::<2>();
    if !rest.is_empty() {
        return None;
    }
    let digit = |b: u8| char::from(b).to_digit(16);
    pairs
        .iter()
        .map(|&[high, low]| u8::try_from(digit(high)? * 16 + digit(low)?).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handles_and_binary_are_hex() {
        assert_eq!(parse_hex(b"2A"), Some(0x2A));
        assert_eq!(parse_hex(b"ff"), Some(0xFF));
        assert_eq!(parse_hex(b""), None);
        assert_eq!(parse_hex(b"+1"), None);
        assert_eq!(parse_hex(b"10000000000000000"), None);
        assert_eq!(parse_binary(b"00FF7a"), Some(vec![0, 0xFF, 0x7A]));
        assert_eq!(parse_binary(b"0"), None);
        assert_eq!(parse_binary(b"0G"), None);
        assert_eq!(parse_int(b"-12"), Some(-12));
        assert_eq!(parse_int(b"1.5"), None);
    }
}
