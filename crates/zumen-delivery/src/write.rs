//! 管理 XML の値と記録を、元のバイト列を保って変更する。

use super::{Cx, Diagnostic, Diagnostics, Error, Kind, Ref, codes, stopped};
use zumen_xml::{Document, Edit as XmlEdit, Element, Node};

pub use zumen_xml::NewElement;

/// 根からたどる子の指定。名前は接頭辞を含めて区別する。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PathStep {
    /// 接頭辞を含む元の名前。
    pub name: String,
    /// 同じ名前の兄弟の中の位置。0 から数える。
    pub index: u64,
}

/// 元の文書で解く変更。空の経路は根を指す。
#[derive(Clone, Copy, Debug)]
pub enum Edit<'a> {
    /// 子の要素を持たない項目の文字を置き換える。
    SetText {
        /// 根からの経路。
        path: &'a [PathStep],
        /// 文字参照にする前の値。
        value: &'a str,
    },
    /// 親の末尾に記録を足す。同じ親への追加は指定の順。
    Append {
        /// 親の根からの経路。
        path: &'a [PathStep],
        /// 追加する木。
        child: &'a NewElement,
    },
}

/// 宣言・属性・DTD・未変更の範囲を保ち、変更後の XML を同じ予算で再読してから返す。
/// DTD の取得や検証はしない。失敗や中断では結果を返さない。
pub fn write_xml(src: &[u8], edits: &[Edit<'_>], cx: &mut Cx<'_>) -> Result<Vec<u8>, Error> {
    let doc = read(src, true, cx)?;
    cx.check(Kind::Work, edits.len() as u64)
        .map_err(|stop| stopped(cx, stop))?;
    cx.check(Kind::DecodedBytes, (edits.len() as u64).saturating_mul(64))
        .map_err(|stop| stopped(cx, stop))?;
    let mut resolved = Vec::with_capacity(edits.len());
    for (index, edit) in edits.iter().enumerate() {
        resolved.push(match edit {
            Edit::SetText { path, value } => XmlEdit::SetText {
                element: resolve(&doc, path, index as u64, cx)?,
                value,
            },
            Edit::Append { path, child } => XmlEdit::Append {
                element: resolve(&doc, path, index as u64, cx)?,
                child,
            },
        });
    }
    let bytes = zumen_xml::write(&doc, &resolved, cx)?;
    read(&bytes, false, cx)?;
    cx.check(Kind::Work, 0).map_err(|stop| stopped(cx, stop))?;
    Ok(bytes)
}

fn read(src: &[u8], report: bool, cx: &mut Cx<'_>) -> Result<Document, Error> {
    // read が所有する入力の複製も、確保前に数える。
    cx.check(Kind::DecodedBytes, src.len() as u64)
        .map_err(|stop| stopped(cx, stop))?;
    let mut quiet = Diagnostics::new();
    let depth = cx.budget.used(Kind::Depth);
    let result = {
        let mut inner = Cx::new(cx.budget, &mut quiet, cx.cancel);
        zumen_xml::read(src, &mut inner)
    };
    // 読み取りが途中で止まっても、この操作で入った階層を共有予算に残さない。
    for _ in depth..cx.budget.used(Kind::Depth) {
        cx.leave();
    }
    if report || result.is_err() {
        for diagnostic in quiet.iter() {
            if diagnostic.code() != codes::XML_DTD_DROPPED {
                cx.report(diagnostic.clone());
            }
        }
    }
    result
}

fn resolve(doc: &Document, path: &[PathStep], edit: u64, cx: &mut Cx<'_>) -> Result<u64, Error> {
    let mut index = doc.root();
    for (step, selector) in path.iter().enumerate() {
        cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
        let parent = doc.element(index).ok_or(Error::Rejected)?;
        let mut count = 0_u64;
        let mut found = None;
        for node in parent.children() {
            cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
            let Node::Element(number) = node else {
                continue;
            };
            let Some(element) = doc.element(*number) else {
                continue;
            };
            if name(doc, element, cx)? == selector.name {
                if count == selector.index {
                    found = Some(*number);
                    break;
                }
                count += 1;
            }
        }
        let Some(number) = found else {
            cx.report(
                Diagnostic::new(codes::DELIVERY_EDIT_PATH_MISSING)
                    .with("index", edit)
                    .with("step", step as u64)
                    .at(Ref::at_bytes(
                        parent.span().start as u64,
                        parent.span().end as u64,
                    )),
            );
            return Err(Error::Rejected);
        };
        index = number;
    }
    Ok(index as u64)
}

fn name(doc: &Document, element: &Element, cx: &mut Cx<'_>) -> Result<String, Error> {
    let range = element.name().raw();
    cx.check(Kind::Work, range.len() as u64)
        .map_err(|stop| stopped(cx, stop))?;
    cx.check(Kind::DecodedBytes, (range.len() as u64).saturating_mul(6))
        .map_err(|stop| stopped(cx, stop))?;
    Ok(doc.cdata(range))
}
