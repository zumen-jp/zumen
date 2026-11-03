//! 国土地理院の基盤地図情報ダウンロードデータ（台帳の `fgd_dl_5_2`。JPGIS 2014 の GML）を読み、地理の領域モデル
//! （決定 0233）に移す。
//!
//! L1 は XML の要素のグラフ（`zumen-xml`）のまま持ち、要素をレコードとする。スキーマは同梱せず、地物と属性は
//! ファイル仕様書の応用スキーマ（2.2 節）と符号化のタグ名（4.2.6 節）から読む（`docs/formats/fgd.md`）。

mod lift;

use zumen_base::{ByteRange, Kind};
use zumen_geo::GeoModel;
use zumen_sdk::{Codec, Cx, Error, FileModel, Lift, RecordInfo, Signature};
use zumen_xml::Document;

/// 基盤地図情報の名前空間（ファイル仕様書 4.2.4 節の表 4-2）
pub const NAMESPACE: &str = "http://fgd.gsi.go.jp/spec/2008/FGD_GMLSchema";

/// 基盤地図情報のコーデック
pub struct Fgd;

/// 読んだ基盤地図情報。XML の要素のグラフを持つ
#[derive(Debug)]
pub struct FgdFile {
    doc: Document,
}

impl FgdFile {
    /// XML の文書
    pub fn document(&self) -> &Document {
        &self.doc
    }
}

impl FileModel for FgdFile {
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

impl Codec for Fgd {
    type File = FgdFile;

    const FORMAT: &'static str = "fgd";

    const SIGNATURES: &'static [Signature] = &[Signature::XmlRoot {
        name: b"Dataset",
        namespace: Some(NAMESPACE.as_bytes()),
    }];

    fn read(src: &[u8], cx: &mut Cx) -> Result<FgdFile, Error> {
        cx.check(Kind::InputBytes, src.len() as u64)?;
        let doc = zumen_xml::read(src, cx)?;
        Ok(FgdFile { doc })
    }

    /// 書き戻しはしないので、読んだバイト列をそのまま書く
    fn write(file: &FgdFile, out: &mut Vec<u8>, _cx: &mut Cx) -> Result<(), Error> {
        out.extend_from_slice(file.doc.src());
        Ok(())
    }
}

impl Lift<GeoModel> for FgdFile {
    fn lift(&self, cx: &mut Cx) -> Result<GeoModel, Error> {
        lift::lift(&self.doc, cx)
    }
}
