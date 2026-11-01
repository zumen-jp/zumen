//! 電子納品の成果品（決定 0241）を読む。
//!
//! - L1 は、成果品の名前の木（ファイルの名前と大きさ）と、管理ファイルごとの XML の要素のグラフ（`zumen-xml`）。
//! - L2（`Package`）は、管理ファイルの記録を、要素の元の名前と値の文字の組の木で持つ。要領の版ごとに要素の名前が
//!   違うので、型にしない。
//! - 管理ファイルは、成果品の起点か、その直下のフォルダーにある拡張子 `.XML` のファイルとする（`INDEX_C.XML`・
//!   `DRAWING/DRAWING.XML`・`PHOTO/PHOTO.XML` など）。
//! - DTD は処理しない。管理ファイルはどれも文書型宣言を持つので、`xml.dtd.dropped` は返さない。

use zumen_base::{Cx, Diagnostic, Diagnostics, Kind, Ref, Stop, codes};
use zumen_sdk::Error;
use zumen_xml::{Document, Element, Node};

mod check;

pub use check::{Report, check};

/// 成果品
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Package {
    /// ファイル。名前は起点からの相対の名前（`/` で区切る）。名前の順
    pub files: Vec<File>,
    /// 管理ファイル。名前の順
    pub managed: Vec<Managed>,
}

/// 成果品のファイル
#[derive(Clone, Debug, PartialEq)]
pub struct File {
    /// 起点からの相対の名前。大文字と小文字は元のまま
    pub name: String,
    /// 大きさ（バイト）
    pub size: u64,
}

/// 管理ファイル
#[derive(Clone, Debug, PartialEq)]
pub struct Managed {
    /// 起点からの相対の名前
    pub name: String,
    /// 文書型宣言の DTD の名前（`INDE_C05.DTD` など）。宣言がなければ `None`
    pub doctype: Option<String>,
    /// 根の要素の記録
    pub root: Record,
}

/// 管理ファイルの要素の記録
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Record {
    /// 要素の元の名前
    pub name: String,
    /// 文字の値。前後の空白を除く。子の要素だけを持つ要素は `None`
    pub value: Option<String>,
    /// 値がファイルの名前の形なら、成果品の中の名前に解いたもの
    pub file: Option<Link>,
    /// 子の要素の記録。書いた順
    pub children: Vec<Record>,
    /// 出所。管理ファイルの中のバイトの範囲
    pub source: Ref,
}

/// 管理ファイルの値が指すファイル
#[derive(Clone, Debug, PartialEq)]
pub struct Link {
    /// 成果品の中の名前。見つからなければ、管理ファイルのフォルダーに値をつないだ名前
    pub name: String,
    /// 成果品の中にあるか
    pub found: bool,
}

impl Package {
    /// 名前でファイルを探す
    pub fn file(&self, name: &str) -> Option<&File> {
        self.files
            .binary_search_by(|file| file.name.as_str().cmp(name))
            .ok()
            .and_then(|index| self.files.get(index))
    }
}

fn stopped(cx: &mut Cx, stop: Stop) -> Error {
    cx.report(stop.to_diagnostic());
    Error::Stopped(stop)
}

/// 名前が成果品の中の相対の名前として使えるか。空の部分・`.`・`..`・`\` を含む名前と、`/` で始まる名前は使えない
fn is_safe(name: &str) -> bool {
    !name.is_empty()
        && !name.contains('\\')
        && name
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
}

/// 起点からの相対の名前とバイト列の組から、成果品を読む。使えない名前は `delivery.name.unsafe` を返して除く
pub fn read(files: &[(&str, &[u8])], cx: &mut Cx) -> Result<Package, Error> {
    cx.check(Kind::Records, files.len() as u64)
        .map_err(|stop| stopped(cx, stop))?;
    let mut sorted: Vec<(&str, &[u8])> = Vec::with_capacity(files.len());
    for (name, bytes) in files {
        if is_safe(name) {
            sorted.push((name, bytes));
        } else {
            cx.report(Diagnostic::new(codes::DELIVERY_NAME_UNSAFE).with("name", *name));
        }
    }
    sorted.sort_by(|a, b| a.0.cmp(b.0));
    sorted.dedup_by(|a, b| a.0 == b.0);
    let mut package = Package {
        files: sorted
            .iter()
            .map(|(name, bytes)| File {
                name: (*name).to_string(),
                size: bytes.len() as u64,
            })
            .collect(),
        managed: Vec::new(),
    };
    for (name, bytes) in &sorted {
        if !is_managed(name) {
            continue;
        }
        if let Some(managed) = managed(&package, name, bytes, cx)? {
            package.managed.push(managed);
        }
    }
    Ok(package)
}

/// ZIP の成果品を読む。ディレクトリの項目は除く。展開した量は予算（`decoded-bytes`）で数える
pub fn read_zip(src: &[u8], cx: &mut Cx) -> Result<Package, Error> {
    let archive = zumen_zip::read(src, cx)?;
    let mut extracted = Vec::new();
    for entry in archive.entries() {
        if entry.is_directory() {
            continue;
        }
        let bytes = archive.extract(entry, cx)?;
        extracted.push((entry.name().to_string(), bytes));
    }
    let files: Vec<(&str, &[u8])> = extracted
        .iter()
        .map(|(name, bytes)| (name.as_str(), bytes.as_slice()))
        .collect();
    read(&files, cx)
}

/// 管理ファイルか。起点か、その直下のフォルダーにある拡張子 `.XML`（大文字と小文字を問わない）のファイル
fn is_managed(name: &str) -> bool {
    name.split('/').count() <= 2
        && name
            .rsplit_once('.')
            .is_some_and(|(_, extension)| extension.eq_ignore_ascii_case("xml"))
}

/// 管理ファイルを読む。XML として読めなければ、`zumen-xml` の診断に `delivery.file.invalid` を足して `None`
fn managed(
    package: &Package,
    name: &str,
    bytes: &[u8],
    cx: &mut Cx,
) -> Result<Option<Managed>, Error> {
    let mut quiet = Diagnostics::new();
    let read = {
        let mut inner = Cx::new(cx.budget, &mut quiet, cx.cancel);
        zumen_xml::read(bytes, &mut inner)
    };
    for diagnostic in quiet.iter() {
        if diagnostic.code() != codes::XML_DTD_DROPPED {
            cx.report(diagnostic.clone());
        }
    }
    let doc = match read {
        Ok(doc) => doc,
        Err(Error::Stopped(stop)) => return Err(Error::Stopped(stop)),
        Err(_) => {
            cx.report(Diagnostic::new(codes::DELIVERY_FILE_INVALID).with("name", name));
            return Ok(None);
        }
    };
    let Some(root) = doc.element(doc.root()) else {
        cx.report(Diagnostic::new(codes::DELIVERY_FILE_INVALID).with("name", name));
        return Ok(None);
    };
    let folder = name.rsplit_once('/').map(|(folder, _)| folder);
    let reader = Reader {
        doc: &doc,
        package,
        folder,
    };
    let root = reader.record(root, cx)?;
    Ok(Some(Managed {
        name: name.to_string(),
        doctype: doc.doctype().and_then(|range| system_id(doc.bytes(range))),
        root,
    }))
}

/// 文書型宣言の、最後の引用符の中の文字（システム識別子）
fn system_id(declaration: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(declaration);
    let text = text.split('[').next().unwrap_or_default();
    let quote = text.rfind(['"', '\''])?;
    let mark = text.get(quote..quote + 1)?;
    let start = text.get(..quote)?.rfind(mark)?;
    let id = text.get(start + 1..quote)?.trim();
    (!id.is_empty()).then(|| id.to_string())
}

struct Reader<'d> {
    doc: &'d Document,
    package: &'d Package,
    /// 管理ファイルのフォルダー。起点にあれば `None`
    folder: Option<&'d str>,
}

impl Reader<'_> {
    fn record(&self, element: &Element, cx: &mut Cx) -> Result<Record, Error> {
        cx.check(Kind::Elements, 1)
            .map_err(|stop| stopped(cx, stop))?;
        let mut children = Vec::new();
        let mut text = String::new();
        for node in element.children() {
            match node {
                Node::Element(index) => {
                    if let Some(child) = self.doc.element(*index) {
                        children.push(self.record(child, cx)?);
                    }
                }
                Node::Text(range) => text.push_str(&self.doc.text(range.clone())),
                Node::CData(range) => text.push_str(&self.doc.cdata(range.clone())),
            }
        }
        let value = text.trim();
        let value = (!value.is_empty() || children.is_empty()).then(|| value.to_string());
        let file = value.as_deref().and_then(|value| self.link(value));
        Ok(Record {
            // 名前は文書の文字コードのバイトなので、文字に直す
            name: self.doc.text(element.name().local()),
            value,
            file,
            children,
            source: Ref::at_bytes(element.span().start as u64, element.span().end as u64),
        })
    }

    /// 値がファイルの名前の形（空白を含まず、英字で始まる 1〜4 文字の拡張子を持つ）なら、管理ファイルの
    /// フォルダーからの名前に解く。そこになく、値がフォルダーを含まなければ、管理ファイルのフォルダーの下の
    /// フォルダー（`ORG` など）で同じ名前のファイルを探し、1 つだけならそれにする
    fn link(&self, value: &str) -> Option<Link> {
        let (stem, extension) = value.rsplit_once('.')?;
        let looks = !stem.is_empty()
            && !value.contains(char::is_whitespace)
            && (1..=4).contains(&extension.len())
            && extension
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_alphabetic())
            && extension.chars().all(|c| c.is_ascii_alphanumeric());
        if !looks || !is_safe(value) {
            return None;
        }
        let name = match self.folder {
            Some(folder) => format!("{folder}/{value}"),
            None => value.to_string(),
        };
        if self.package.file(&name).is_some() {
            return Some(Link { name, found: true });
        }
        if let Some(folder) = self.folder
            && !value.contains('/')
        {
            let head = format!("{folder}/");
            let tail = format!("/{value}");
            let mut below = self
                .package
                .files
                .iter()
                .filter(|file| file.name.starts_with(&head) && file.name.ends_with(&tail));
            if let (Some(file), None) = (below.next(), below.next()) {
                return Some(Link {
                    name: file.name.clone(),
                    found: true,
                });
            }
        }
        Some(Link { name, found: false })
    }
}
