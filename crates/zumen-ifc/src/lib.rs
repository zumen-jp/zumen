//! IFC の L1（決定 0008・0207・0211）。
//!
//! IFC-SPF（ISO 10303-21）を、スキーマを知らずにインスタンスのグラフとして読む。レコードは `zumen-step` で読み、
//! インスタンスの番号から位置を引く表と、値の中の参照（`#n`）の引き方を置く。知らない実体も元のバイトのまま持ち、
//! 変えずに書くと元のバイト列に戻る。版（IFC2X3・IFC4・IFC4X3）は問わない。

use core::ops::Range;
use std::collections::BTreeMap;

use zumen_base::{ByteRange, Diagnostic, Kind, Ref, codes};
use zumen_sdk::{Codec, Cx, Error, FileModel, RecordInfo, Signature, Stop};
use zumen_step::Dialect;

pub use zumen_step::{Form, Part, Quote, Record, Value};

mod zip;

pub use zip::{IfcZip, IfcZipFile};

/// IFC-SPF のコーデック
pub struct Ifc;

/// 読んだ IFC-SPF。元のバイト列と、レコードと、番号の表
#[derive(Clone, Debug, PartialEq)]
pub struct IfcFile {
    src: Vec<u8>,
    records: Vec<Record>,
    schema: Range<usize>,
    /// インスタンスの番号から、レコードの番号。同じ番号は初めのもの
    index: BTreeMap<u64, usize>,
}

/// 逆向きの参照。インスタンスの番号から、それを参照するインスタンスの番号
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Inverse {
    referrers: BTreeMap<u64, Vec<u64>>,
}

impl Inverse {
    /// `id` を参照するインスタンスの番号。ファイルに書いた順で、同じものは 1 つにする
    pub fn referrers(&self, id: u64) -> &[u64] {
        self.referrers
            .get(&id)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }
}

/// IFC の L1 の読みの規則。SXF のフィーチャのコメントはない
const DIALECT: Dialect = Dialect {
    record_invalid: codes::IFC_RECORD_INVALID,
    file_invalid: codes::IFC_FILE_INVALID,
    features: &[],
    plain_if_described: None,
};

/// IFC のスキーマの名前の始まり（IFC2X3・IFC4・IFC4X3_ADD2 など）
pub(crate) const SCHEMA: &[u8] = b"IFC";

impl IfcFile {
    /// 元のバイト列の範囲。外なら空
    pub fn bytes(&self, range: Range<usize>) -> &[u8] {
        self.src.get(range).unwrap_or_default()
    }

    /// レコード。見出しの文と、データの節のインスタンス。書いた順
    pub fn records(&self) -> &[Record] {
        &self.records
    }

    /// `FILE_SCHEMA` の最初のスキーマの名前（`IFC4` など）
    pub fn schema(&self) -> &[u8] {
        self.bytes(self.schema.clone())
    }

    /// 番号 `id` のインスタンス
    pub fn instance(&self, id: u64) -> Option<&Record> {
        self.records.get(*self.index.get(&id)?)
    }

    /// インスタンスの番号。書いた順で、同じ番号は初めのもの
    pub fn ids(&self) -> impl Iterator<Item = u64> + '_ {
        self.records.iter().enumerate().filter_map(|(at, record)| {
            let id = record.id()?;
            (self.index.get(&id) == Some(&at)).then_some(id)
        })
    }

    /// 実体の名前（`IFCWALL` など）。複合のインスタンスは最初の部分の名前
    pub fn kind(&self, record: &Record) -> &[u8] {
        record
            .parts()
            .first()
            .map(|part| self.bytes(part.name()))
            .unwrap_or_default()
    }

    /// レコードの値の中の参照。書いた順で、同じものも残す
    pub fn references(&self, record: &Record) -> Vec<u64> {
        let mut found = Vec::new();
        // 書いた順に取り出すので、後ろから積む
        let mut stack: Vec<&Value> = record.parts().iter().flat_map(Part::args).collect();
        stack.reverse();
        while let Some(value) = stack.pop() {
            match value {
                Value::Ref(id) => found.push(*id),
                Value::List(values) => stack.extend(values.iter().rev()),
                Value::Typed(part) => stack.extend(part.args().iter().rev()),
                _ => {}
            }
        }
        found
    }

    /// 逆向きの参照を作る。参照ごとに `work` を 1 つ数える
    pub fn inverse(&self, cx: &mut Cx) -> Result<Inverse, Error> {
        let mut inverse = Inverse::default();
        for id in self.ids() {
            let Some(record) = self.instance(id) else {
                continue;
            };
            let references = self.references(record);
            cx.check(Kind::Work, 1 + references.len() as u64)
                .map_err(|stop| stopped(cx, stop, Some(record.span())))?;
            for target in references {
                let referrers = inverse.referrers.entry(target).or_default();
                // インスタンスは書いた順に 1 度だけ見るので、同じものは続く
                if referrers.last() != Some(&id) {
                    referrers.push(id);
                }
            }
        }
        Ok(inverse)
    }
}

impl FileModel for IfcFile {
    fn record_count(&self) -> usize {
        self.records.len()
    }

    fn record(&self, index: usize) -> Option<RecordInfo<'_>> {
        let record = self.records.get(index)?;
        Some(RecordInfo {
            kind: self.kind(record),
            bytes: Some(ByteRange {
                start: record.span().start as u64,
                end: record.span().end as u64,
            }),
        })
    }
}

impl Codec for Ifc {
    type File = IfcFile;

    const FORMAT: &'static str = "ifc";

    const SIGNATURES: &'static [Signature] = &[Signature::StepSchema { prefix: SCHEMA }];

    fn read(src: &[u8], cx: &mut Cx) -> Result<IfcFile, Error> {
        cx.check(Kind::InputBytes, src.len() as u64)
            .map_err(|stop| stopped(cx, stop, None))?;
        let read = zumen_step::read(src, &DIALECT, cx)?;
        let schema = read.schema.unwrap_or(0..0);
        let name = src.get(schema.clone()).unwrap_or_default();
        let is_ifc = name
            .get(..SCHEMA.len())
            .is_some_and(|start| start.eq_ignore_ascii_case(SCHEMA));
        if !is_ifc {
            cx.report(
                Diagnostic::new(codes::IFC_SCHEMA_UNSUPPORTED)
                    .with("schema", String::from_utf8_lossy(name).into_owned()),
            );
            return Err(Error::Unsupported);
        }
        let mut index = BTreeMap::new();
        for (at, record) in read.records.iter().enumerate() {
            cx.check(Kind::Work, 1)
                .map_err(|stop| stopped(cx, stop, Some(record.span())))?;
            if let Some(id) = record.id() {
                if let std::collections::btree_map::Entry::Vacant(slot) = index.entry(id) {
                    slot.insert(at);
                } else {
                    cx.report(
                        Diagnostic::new(codes::IFC_DUPLICATE_DROPPED)
                            .with("id", id)
                            .at(at_bytes(record.span())),
                    );
                }
            }
        }
        let file = IfcFile {
            src: src.to_vec(),
            records: read.records,
            schema,
            index,
        };
        for record in &file.records {
            let references = file.references(record);
            cx.check(Kind::Work, references.len() as u64)
                .map_err(|stop| stopped(cx, stop, Some(record.span())))?;
            for id in references {
                if !file.index.contains_key(&id) {
                    cx.report(
                        Diagnostic::new(codes::IFC_REFERENCE_MISSING)
                            .with("id", id)
                            .at(at_bytes(record.span())),
                    );
                }
            }
        }
        Ok(file)
    }

    fn write(file: &IfcFile, out: &mut Vec<u8>, cx: &mut Cx) -> Result<(), Error> {
        let mut end = 0;
        for record in &file.records {
            cx.check(Kind::Work, 1)
                .map_err(|stop| stopped(cx, stop, Some(record.span())))?;
            // レコードの間（空白・コメント・読めなかった文）は、そのまま写す
            out.extend_from_slice(file.bytes(end..record.span().start));
            out.extend_from_slice(file.bytes(record.span()));
            end = record.span().end;
        }
        out.extend_from_slice(file.bytes(end..file.src.len()));
        Ok(())
    }
}

pub(crate) fn at_bytes(range: Range<usize>) -> Ref {
    Ref::at_bytes(range.start as u64, range.end as u64)
}

/// 止めた理由を報告する。扱っていたバイト範囲があれば、場所にする（決定 0119）
pub(crate) fn stopped(cx: &mut Cx, stop: Stop, place: Option<Range<usize>>) -> Error {
    let diagnostic = stop.to_diagnostic();
    cx.report(match place {
        Some(range) => diagnostic.at(at_bytes(range)),
        None => diagnostic,
    });
    Error::Stopped(stop)
}
