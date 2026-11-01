//! CityGML 2.0（台帳の `citygml_2_0`。PLATEAU の 3D 都市モデルと i-UR の拡張を含む）を読み、地理の領域モデル
//! （決定 0233）に移す。
//!
//! L1 は XML の要素のグラフ（`zumen-xml`）のまま持ち、要素をレコードとする。スキーマは同梱せず、地物と属性は
//! 要素の形から読む（`docs/formats/citygml.md`）。座標は、`srsName` の座標系の軸の順のまま読む。

mod lift;

use zumen_base::{ByteRange, Kind};
use zumen_geo::GeoModel;
use zumen_sdk::{Codec, Cx, Error, FileModel, Lift, RecordInfo, Signature};
use zumen_xml::Document;

/// CityGML 2.0 の名前空間
pub const NAMESPACE: &str = "http://www.opengis.net/citygml/2.0";

/// CityGML のコーデック
pub struct CityGml;

/// 読んだ CityGML。XML の要素のグラフを持つ
#[derive(Debug)]
pub struct CityGmlFile {
    doc: Document,
}

impl CityGmlFile {
    /// XML の文書
    pub fn document(&self) -> &Document {
        &self.doc
    }
}

impl FileModel for CityGmlFile {
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

impl Codec for CityGml {
    type File = CityGmlFile;

    const FORMAT: &'static str = "citygml";

    const SIGNATURES: &'static [Signature] = &[Signature::XmlRoot {
        name: b"CityModel",
        namespace: Some(NAMESPACE.as_bytes()),
    }];

    fn read(src: &[u8], cx: &mut Cx) -> Result<CityGmlFile, Error> {
        cx.check(Kind::InputBytes, src.len() as u64)?;
        let doc = zumen_xml::read(src, cx)?;
        Ok(CityGmlFile { doc })
    }

    /// 書き戻しはまだしないので、読んだバイト列をそのまま書く
    fn write(file: &CityGmlFile, out: &mut Vec<u8>, _cx: &mut Cx) -> Result<(), Error> {
        out.extend_from_slice(file.doc.src());
        Ok(())
    }
}

impl Lift<GeoModel> for CityGmlFile {
    fn lift(&self, cx: &mut Cx) -> Result<GeoModel, Error> {
        lift::lift(&self.doc, cx)
    }
}
