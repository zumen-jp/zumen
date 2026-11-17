//! ST-Bridge（構造の設計データを交換する XML、`.stb`）を読み、zumen の建物モデル（L2）へ移す（#400）。
//!
//! 読むのは 2.0 系（2.0.0〜2.0.2）と 2.1 系（2.1.0・2.1.1）。L1 は XML の要素の木で、元のバイト列のまま書き戻せる。
//! 建物モデルへの移し方は [`lift`] と `docs/formats/stbridge.md` にある。仕様書と XML Schema は同梱しない。

use zumen_base::{ByteRange, Cx, Diagnostic, Kind, Ref, codes};
use zumen_sdk::{Codec, Error, FileModel, RecordInfo, Signature};
use zumen_xml::{Document, Node};

pub mod lift;

/// ST-Bridge のコーデック
#[derive(Clone, Copy, Debug)]
pub struct Stb;

/// ST-Bridge の版
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version {
    /// メジャーバージョン
    pub major: u32,
    /// マイナーバージョン
    pub minor: u32,
    /// 修正の版
    pub patch: u32,
}

impl Version {
    fn parse(text: &str) -> Option<Self> {
        let mut parts = text.trim().split('.');
        let mut next = || -> Option<u32> {
            match parts.next() {
                None => Some(0),
                Some(part) if !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()) => {
                    part.parse().ok()
                }
                Some(_) => None,
            }
        };
        let major = next()?;
        let minor = next()?;
        let patch = next()?;
        parts.next().is_none().then_some(Version {
            major,
            minor,
            patch,
        })
    }
}

impl core::fmt::Display for Version {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// L1: 読んだ XML の要素の木と、根の版。レコードは XML の要素で、文書の順
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StbFile {
    xml: Document,
    version: Version,
}

impl StbFile {
    /// XML の文書
    pub fn xml(&self) -> &Document {
        &self.xml
    }

    /// 根の `version`
    pub fn version(&self) -> Version {
        self.version
    }

    /// 要素の、名前空間と接頭辞を除いた名前
    pub fn name(&self, element: usize) -> &[u8] {
        self.xml
            .element(element)
            .map_or(&[][..], |e| self.xml.local_name(e.name()))
    }

    /// 要素の属性の値
    pub fn attribute(&self, element: usize, name: &str) -> Option<String> {
        let e = self.xml.element(element)?;
        self.xml
            .attribute(e, None, name.as_bytes())
            .map(|a| self.xml.value(a))
    }

    /// 要素のすべての属性の名前と値。書いた順。名前空間の宣言は除く
    pub fn attributes(&self, element: usize) -> Vec<(String, String)> {
        let Some(e) = self.xml.element(element) else {
            return Vec::new();
        };
        e.attributes()
            .iter()
            .filter(|a| {
                let raw = self.xml.bytes(a.name().raw());
                raw != b"xmlns" && !raw.starts_with(b"xmlns:")
            })
            .map(|a| {
                (
                    String::from_utf8_lossy(self.xml.local_name(a.name())).into_owned(),
                    self.xml.value(a),
                )
            })
            .collect()
    }

    /// 子の要素の番号。書いた順
    pub fn children(&self, element: usize) -> impl Iterator<Item = usize> + '_ {
        self.xml
            .element(element)
            .map(|e| e.children())
            .unwrap_or_default()
            .iter()
            .filter_map(|node| match node {
                Node::Element(index) => Some(*index),
                _ => None,
            })
    }

    /// 名前の合う最初の子の要素
    pub fn child(&self, element: usize, name: &[u8]) -> Option<usize> {
        self.children(element).find(|&c| self.name(c) == name)
    }

    /// 要素の中の文字
    pub fn text(&self, element: usize) -> String {
        let Some(e) = self.xml.element(element) else {
            return String::new();
        };
        let mut out = String::new();
        for node in e.children() {
            match node {
                Node::Text(range) => out.push_str(&self.xml.text(range.clone())),
                Node::CData(range) => out.push_str(&self.xml.cdata(range.clone())),
                Node::Element(_) => {}
            }
        }
        out
    }

    /// 要素の元のバイト範囲
    pub fn span(&self, element: usize) -> Option<ByteRange> {
        let span = self.xml.element(element)?.span();
        Some(ByteRange {
            start: span.start as u64,
            end: span.end as u64,
        })
    }
}

/// 根の要素の名前
const ROOT: &[u8] = b"ST_BRIDGE";

impl Codec for Stb {
    type File = StbFile;

    const FORMAT: &'static str = "stbridge";

    // 版によって名前空間が違うので、名前空間を問わない。1.x も判定してから、版を知らせて読まない
    const SIGNATURES: &'static [Signature] = &[Signature::XmlRoot {
        name: ROOT,
        namespace: None,
    }];

    fn read(src: &[u8], cx: &mut Cx) -> Result<StbFile, Error> {
        cx.check(Kind::InputBytes, src.len() as u64)?;
        let xml = zumen_xml::read(src, cx)?;
        let root = xml.root();
        let file = StbFile {
            xml,
            version: Version {
                major: 0,
                minor: 0,
                patch: 0,
            },
        };
        if file.name(root) != ROOT {
            cx.report(Diagnostic::new(codes::STBRIDGE_FILE_INVALID));
            return Err(Error::Invalid);
        }
        cx.check(Kind::Records, file.xml.elements().len() as u64)?;
        let Some(text) = file.attribute(root, "version") else {
            cx.report(Diagnostic::new(codes::STBRIDGE_FILE_INVALID));
            return Err(Error::Invalid);
        };
        match Version::parse(&text) {
            Some(version) if version.major == 2 && version.minor <= 1 => {
                Ok(StbFile { version, ..file })
            }
            _ => {
                let at = file
                    .span(root)
                    .map_or_else(Ref::default, |span| Ref::at_bytes(span.start, span.end));
                cx.report(
                    Diagnostic::new(codes::STBRIDGE_VERSION_UNSUPPORTED)
                        .with("version", text)
                        .at(at),
                );
                Err(Error::Unsupported)
            }
        }
    }

    /// 変えていない文書なので、元のバイト列のまま書く
    fn write(file: &StbFile, out: &mut Vec<u8>, _: &mut Cx) -> Result<(), Error> {
        out.extend_from_slice(file.xml.src());
        Ok(())
    }
}

impl FileModel for StbFile {
    fn record_count(&self) -> usize {
        self.xml.elements().len()
    }

    fn record(&self, index: usize) -> Option<RecordInfo<'_>> {
        let element = self.xml.element(index)?;
        let span = element.span();
        Some(RecordInfo {
            kind: self.xml.local_name(element.name()),
            bytes: Some(ByteRange {
                start: span.start as u64,
                end: span.end as u64,
            }),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_versions() {
        assert_eq!(
            Version::parse("2.1.1"),
            Some(Version {
                major: 2,
                minor: 1,
                patch: 1
            })
        );
        assert_eq!(
            Version::parse("2.0"),
            Some(Version {
                major: 2,
                minor: 0,
                patch: 0
            })
        );
        assert_eq!(Version::parse("2.x"), None);
        assert_eq!(Version::parse("2.0.1.4"), None);
    }
}
