//! J-LandXML（LandXML 1.2 に準じた 3 次元設計データ交換標準、台帳の `jlandxml_1_8`）を読み、土木の領域モデル
//! （決定 0230）に移す。
//!
//! L1 は XML の要素のグラフ（`zumen-xml`）のまま持ち、要素をレコードとする。スキーマは同梱せず、交換標準が書く
//! 要素と属性だけを読む。座標の文字は「X Y Z」（北・東・標高）の順に読む。

mod lift;

use zumen_base::{ByteRange, Kind};
use zumen_civil::CivilModel;
use zumen_sdk::{Codec, Cx, Error, FileModel, Lift, RecordInfo, Signature};
use zumen_xml::Document;

/// LandXML 1.2 の名前空間
pub const NAMESPACE: &str = "http://www.landxml.org/schema/LandXML-1.2";

/// J-LandXML のコーデック
pub struct LandXml;

/// 読んだ J-LandXML。XML の要素のグラフを持つ
#[derive(Debug)]
pub struct LandXmlFile {
    doc: Document,
}

impl LandXmlFile {
    /// XML の文書
    pub fn document(&self) -> &Document {
        &self.doc
    }
}

impl FileModel for LandXmlFile {
    fn record_count(&self) -> usize {
        self.doc.elements().len()
    }

    fn record(&self, index: usize) -> Option<RecordInfo<'_>> {
        let element = self.doc.element(index)?;
        Some(RecordInfo {
            kind: self.doc.local_name(element.name()),
            bytes: Some(ByteRange {
                start: element.span().start as u64,
                end: element.span().end as u64,
            }),
        })
    }
}

impl Codec for LandXml {
    type File = LandXmlFile;

    const FORMAT: &'static str = "landxml";

    const SIGNATURES: &'static [Signature] = &[Signature::XmlRoot {
        name: b"LandXML",
        namespace: Some(NAMESPACE.as_bytes()),
    }];

    fn read(src: &[u8], cx: &mut Cx) -> Result<LandXmlFile, Error> {
        cx.check(Kind::InputBytes, src.len() as u64)?;
        let doc = zumen_xml::read(src, cx)?;
        Ok(LandXmlFile { doc })
    }

    /// 書き戻しはまだしないので、読んだバイト列をそのまま書く
    fn write(file: &LandXmlFile, out: &mut Vec<u8>, _cx: &mut Cx) -> Result<(), Error> {
        out.extend_from_slice(file.doc.src());
        Ok(())
    }
}

impl Lift<CivilModel> for LandXmlFile {
    fn lift(&self, cx: &mut Cx) -> Result<CivilModel, Error> {
        lift::lift(&self.doc, cx)
    }
}
