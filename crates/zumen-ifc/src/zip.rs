//! ifcZIP（決定 0211）。ZIP の書庫の中の、ただ 1 つの IFC-SPF を読む。

use zumen_base::{ByteRange, Diagnostic, codes};
use zumen_sdk::{Codec, Cx, Error, FileModel, RecordInfo, Signature};

use crate::{Ifc, IfcFile};

/// ifcZIP のコーデック
pub struct IfcZip;

/// 読んだ ifcZIP。元の書庫のバイト列と、中の IFC-SPF
#[derive(Clone, Debug, PartialEq)]
pub struct IfcZipFile {
    src: Vec<u8>,
    name: String,
    ifc: IfcFile,
}

impl IfcZipFile {
    /// 中の IFC-SPF の項目の名前
    pub fn name(&self) -> &str {
        &self.name
    }

    /// 中の IFC-SPF
    pub fn ifc(&self) -> &IfcFile {
        &self.ifc
    }
}

/// レコードは中の IFC-SPF のもの。バイト範囲は、展開した IFC-SPF の中を指す
impl FileModel for IfcZipFile {
    fn record_count(&self) -> usize {
        self.ifc.record_count()
    }

    fn record(&self, index: usize) -> Option<RecordInfo<'_>> {
        let record = self.ifc.records().get(index)?;
        Some(RecordInfo {
            kind: self.ifc.kind(record),
            bytes: Some(ByteRange {
                start: record.span().start as u64,
                end: record.span().end as u64,
            }),
        })
    }
}

impl Codec for IfcZip {
    type File = IfcZipFile;

    const FORMAT: &'static str = "ifc";

    const SIGNATURES: &'static [Signature] = &[Signature::Bytes {
        offset: 0,
        bytes: b"PK\x03\x04",
    }];

    fn read(src: &[u8], cx: &mut Cx) -> Result<IfcZipFile, Error> {
        let archive = zumen_zip::read(src, cx)?;
        let mut found = archive.entries().iter().filter(|entry| {
            let name = entry.name().as_bytes();
            !entry.is_directory()
                && name
                    .len()
                    .checked_sub(4)
                    .and_then(|at| name.get(at..))
                    .is_some_and(|end| end.eq_ignore_ascii_case(b".ifc"))
        });
        let Some(entry) = found.next() else {
            cx.report(Diagnostic::new(codes::IFC_ENTRY_MISSING));
            return Err(Error::Invalid);
        };
        if found.next().is_some() {
            cx.report(Diagnostic::new(codes::IFC_ENTRY_UNSUPPORTED));
            return Err(Error::Unsupported);
        }
        let data = archive.extract(entry, cx)?;
        let ifc = Ifc::read(&data, cx)?;
        Ok(IfcZipFile {
            src: src.to_vec(),
            name: entry.name().to_owned(),
            ifc,
        })
    }

    /// 元の書庫のバイト列をそのまま書く。インスタンスを変える操作はまだない（#393）
    fn write(file: &IfcZipFile, out: &mut Vec<u8>, _cx: &mut Cx) -> Result<(), Error> {
        out.extend_from_slice(&file.src);
        Ok(())
    }
}
