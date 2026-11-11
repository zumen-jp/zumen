//! SXF の属性ファイル（SAF）を読む（SXF Ver.3.1 仕様書・同解説の附属書の属性付加機構編の 3 章、決定 0249）。
//!
//! 属性ファイルは XML で、図形識別番号ごとの図形（`Figure`）に、属性セット（`AttrSetRef`）ごとの属性（`Attr`）と、
//! 属性のグループ（`AttrGroup`）を書く。図面の属性ファイル用の属性付加機構（`$$ATRF$$図形識別番号$$属性ファイル名`、
//! 2-4-1）の要素に結ぶのは、図面モデルへの変換（[`crate::SxfFile::lift_with`]）で行う。

use zumen_base::{Diagnostic, Diagnostics, Kind, Ref, Stop, codes};
use zumen_sdk::{Cx, Error};
use zumen_xml::{Document, Element, Node};

/// 属性のグループの入れ子の上限。仕様は 2 階層まで（3-1 の図 1）だが、深いものも上限までは読む
const MAX_DEPTH: usize = 8;

/// 読んだ属性ファイル
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AttributeFile {
    /// SXF のバージョン（`version`）
    pub version: String,
    /// 作成日（`date`）
    pub date: String,
    /// 対応する SXF ファイルの名前（`sxfFile`）
    pub sxf_file: String,
    /// 作ったアプリケーション（`application`）
    pub application: String,
    /// 属性セット。順序は入力の順
    pub sets: Vec<AttributeSet>,
    /// 図形。順序は入力の順。図形識別番号が同じものは初めのものだけ
    pub figures: Vec<Figure>,
}

/// 属性セット（`AttributeSet`）
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AttributeSet {
    /// 属性セット識別番号（タグの内容）
    pub id: String,
    /// 名称
    pub name: String,
    /// バージョン
    pub version: String,
    /// 設計者
    pub designed_by: String,
}

/// 図形（`Figure`）
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Figure {
    /// 図形識別番号
    pub id: String,
    /// 図形名称
    pub name: String,
    /// 属性。順序は入力の順
    pub attributes: Vec<Attribute>,
    /// 属性ファイルの中の要素のバイトの範囲
    pub source: Ref,
}

/// 属性（`Attr`）
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Attribute {
    /// 属する属性セットの識別番号（`AttrSetRef` の `id`）
    pub set: String,
    /// 外から順の属性のグループの名前（`AttrGroup` の `name`）
    pub groups: Vec<String>,
    /// 属性名
    pub name: String,
    /// 属性タイプ（`STR`・`LEN` など、5-1 の表 7）。既定義属性は省略できる
    pub kind: Option<String>,
    /// 単位。省略すると属性タイプの既定の単位
    pub unit: Option<String>,
    /// 属性値。`$$$` は、文字列の内容を属性値にすることを表す（2-4-1 の特別な属性値）。そのまま持つ
    pub value: String,
}

/// 属性ファイルを読む。根が `SxfAttributeXML` でなければ、診断して失敗する。欠けた値と重なる図形識別番号は、
/// 診断して読み進める。属性ファイルはどれも仕様の DTD（`SXFV31.DTD`）の文書型宣言を持つので、
/// `xml.dtd.dropped` は返さない
pub fn read(src: &[u8], cx: &mut Cx) -> Result<AttributeFile, Error> {
    cx.check(Kind::InputBytes, src.len() as u64)?;
    let mut quiet = Diagnostics::new();
    let doc = {
        let mut inner = Cx::new(cx.budget, &mut quiet, cx.cancel);
        zumen_xml::read(src, &mut inner)
    };
    for diagnostic in quiet.iter() {
        if diagnostic.code() != codes::XML_DTD_DROPPED {
            cx.report(diagnostic.clone());
        }
    }
    let doc = doc?;
    let mut reader = Reader {
        doc: &doc,
        cx,
        file: AttributeFile::default(),
    };
    reader.file()?;
    Ok(reader.file)
}

struct Reader<'d, 'c, 'b> {
    doc: &'d Document,
    cx: &'c mut Cx<'b>,
    file: AttributeFile,
}

impl<'d> Reader<'d, '_, '_> {
    fn file(&mut self) -> Result<(), Error> {
        let doc = self.doc;
        let Some(root) = doc.element(doc.root()) else {
            return Err(Error::Invalid);
        };
        if self.local(root) != b"SxfAttributeXML" {
            self.invalid(root, "SxfAttributeXML");
            return Err(Error::Invalid);
        }
        self.spend(root, doc.elements().len() as u64)?;
        self.file.version = self.required(root, "version");
        self.file.date = self.required(root, "date");
        self.file.sxf_file = self.required(root, "sxfFile");
        self.file.application = self.required(root, "application");
        for child in self.children(root) {
            match self.local(child) {
                b"AttributeSet" => {
                    let set = AttributeSet {
                        id: self.text(child),
                        name: self.required(child, "name"),
                        version: self.required(child, "version"),
                        designed_by: self.required(child, "designedBy"),
                    };
                    self.file.sets.push(set);
                }
                b"Figure" => self.figure(child),
                _ => self.unsupported(child),
            }
        }
        Ok(())
    }

    fn figure(&mut self, element: &'d Element) {
        let id = self.required(element, "id");
        if self.file.figures.iter().any(|figure| figure.id == id) {
            // 同じ id の Figure は重ねられない（3-4 の (3)）
            self.invalid(element, "id");
            return;
        }
        let mut figure = Figure {
            id,
            name: self.required(element, "name"),
            attributes: Vec::new(),
            source: self.at(element),
        };
        for reference in self.children(element) {
            if self.local(reference) != b"AttrSetRef" {
                self.unsupported(reference);
                continue;
            }
            let set = self.required(reference, "id");
            self.items(reference, &set, &mut Vec::new(), &mut figure.attributes);
        }
        self.file.figures.push(figure);
    }

    /// `AttrSetRef` と `AttrGroup` の中の属性
    fn items(
        &mut self,
        element: &'d Element,
        set: &str,
        groups: &mut Vec<String>,
        out: &mut Vec<Attribute>,
    ) {
        for item in self.children(element) {
            match self.local(item) {
                b"Attr" => {
                    let attribute = Attribute {
                        set: set.to_string(),
                        groups: groups.clone(),
                        name: self.required(item, "name"),
                        kind: self.attribute(item, "type"),
                        unit: self.attribute(item, "unit"),
                        value: self.text(item),
                    };
                    out.push(attribute);
                }
                b"AttrGroup" if groups.len() < MAX_DEPTH => {
                    groups.push(self.required(item, "name"));
                    self.items(item, set, groups, out);
                    groups.pop();
                }
                _ => self.unsupported(item),
            }
        }
    }

    // 要素をたどるツール

    fn local(&self, element: &Element) -> &'d [u8] {
        self.doc.parsed_local_name(element.name())
    }

    fn children(&self, element: &Element) -> Vec<&'d Element> {
        element
            .children()
            .iter()
            .filter_map(|node| match node {
                Node::Element(index) => self.doc.element(*index),
                _ => None,
            })
            .collect()
    }

    /// 要素の文字。前後の空白を除く
    fn text(&self, element: &Element) -> String {
        element
            .children()
            .iter()
            .filter_map(|node| match node {
                Node::Text(range) => Some(self.doc.text(range.clone())),
                Node::CData(range) => Some(self.doc.cdata(range.clone())),
                Node::Element(_) => None,
            })
            .collect::<String>()
            .trim()
            .to_string()
    }

    fn attribute(&self, element: &Element, name: &str) -> Option<String> {
        let doc = self.doc;
        doc.attribute(element, None, name.as_bytes())
            .map(|attribute| doc.value(attribute))
    }

    /// 省略できないタグ属性。なければ診断して空にする
    fn required(&mut self, element: &Element, name: &'static str) -> String {
        match self.attribute(element, name) {
            Some(value) => value,
            None => {
                self.invalid(element, name);
                String::new()
            }
        }
    }

    fn at(&self, element: &Element) -> Ref {
        Ref::at_bytes(element.span().start as u64, element.span().end as u64)
    }

    fn name(&self, element: &Element) -> String {
        self.doc.local_name_text(element.name()).into_owned()
    }

    fn unsupported(&mut self, element: &Element) {
        let name = self.name(element);
        let at = self.at(element);
        self.cx.report(
            Diagnostic::new(codes::SXF_SAF_ELEMENT_UNSUPPORTED)
                .with("element", name)
                .at(at),
        );
    }

    fn invalid(&mut self, element: &Element, field: &'static str) {
        let name = self.name(element);
        let at = self.at(element);
        self.cx.report(
            Diagnostic::new(codes::SXF_SAF_VALUE_INVALID)
                .with("element", name)
                .with("field", field)
                .at(at),
        );
    }

    fn spend(&mut self, element: &Element, amount: u64) -> Result<(), Error> {
        self.cx.check(Kind::Elements, amount).map_err(|stop: Stop| {
            let at = self.at(element);
            self.cx.report(stop.to_diagnostic().at(at));
            Error::Stopped(stop)
        })
    }
}
