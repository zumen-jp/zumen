//! 元のバイト範囲だけを書き換える XML のトランザクション。

mod encoding;

use crate::{Document, Element, Encoding, Namespace, Node, name};
use core::ops::Range;
use encoding::{Writer, append};
use zumen_base::{Code, Cx, Diagnostic, Error, Kind, Ref, codes};

/// 追加する要素。属性と名前空間の宣言は、既存の親から変えない。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NewElement {
    /// 接頭辞があれば含めた XML の名前。親で有効な名前空間を使う。
    pub name: String,
    /// 要素の初めに置く文字。文字参照にする前の値。
    pub value: Option<String>,
    /// 追加する子の要素。書く順。
    pub children: Vec<NewElement>,
}

/// 元の文書の要素番号で指定する変更。同じ親への追加は指定の順に書く。
#[derive(Clone, Copy, Debug)]
pub enum Edit<'a> {
    /// 子の要素を持たない項目の文字を置き換える。注記と処理命令は保つ。
    SetText {
        /// 元の文書の要素番号。
        element: u64,
        /// 文字参照にする前の値。
        value: &'a str,
    },
    /// 親の末尾に要素を足す。
    Append {
        /// 元の文書の親の番号。
        element: u64,
        /// 追加する木。
        child: &'a NewElement,
    },
}

impl Edit<'_> {
    fn element(&self) -> u64 {
        match self {
            Self::SetText { element, .. } | Self::Append { element, .. } => *element,
        }
    }
}

struct Patch {
    range: Range<usize>,
    bytes: Vec<u8>,
}

/// 指定した文字と挿入箇所だけを書き換える。文字コード・BOM・宣言・それ以外のバイトを保つ。
/// 失敗時は何も返さず、元の文書は変えない。DTD と外部の実体は解決しない。
pub fn write(doc: &Document, edits: &[Edit<'_>], cx: &mut Cx<'_>) -> Result<Vec<u8>, Error> {
    let at = Ref::at_bytes(0, doc.src().len() as u64);
    let writer = Writer::new(doc, cx, &at)?;
    spend(cx, Kind::Work, (edits.len() as u64).saturating_mul(64), &at)?;
    spend(
        cx,
        Kind::DecodedBytes,
        (edits.len() as u64).saturating_mul(64),
        &at,
    )?;
    let mut order: Vec<_> = edits.iter().enumerate().collect();
    order.sort_unstable_by_key(|(index, edit)| (edit.element(), *index));
    let mut patches = Vec::new();
    for group in order.chunk_by(|a, b| a.1.element() == b.1.element()) {
        let Some((_, first)) = group.first() else {
            continue;
        };
        let number = first.element();
        let Some(element) = usize::try_from(number).ok().and_then(|n| doc.element(n)) else {
            cx.report(Diagnostic::new(codes::XML_EDIT_TARGET_MISSING).with("index", number));
            return Err(Error::Rejected);
        };
        let at = place(element);
        match first {
            Edit::SetText { value, .. } if group.len() == 1 => {
                replace(doc, element, value, &writer, &mut patches, cx)?
            }
            Edit::SetText { .. } => return Err(invalid(cx, codes::XML_EDIT_RANGE_INVALID, &at)),
            Edit::Append { .. } => {
                let mut bytes = Vec::new();
                let closing = closing(doc, element, &writer, cx)?;
                let indentation = indentation(doc, element, closing.start, &writer, cx)?;
                let mut insert_at = closing.start;
                if let Some((position, _)) = &indentation {
                    insert_at = *position;
                }
                for (_, edit) in group {
                    let Edit::Append { child, .. } = edit else {
                        return Err(invalid(cx, codes::XML_EDIT_RANGE_INVALID, &at));
                    };
                    if let Some((_, indent)) = &indentation {
                        append(&mut bytes, indent, cx, &at)?;
                    }
                    new_element(doc, element, child, &writer, &mut bytes, cx)?;
                    if indentation.is_some() {
                        writer.push(&mut bytes, writer.newline, cx, &at)?;
                    }
                }
                if closing.end > closing.start {
                    let mut expanded = writer.encode(">", cx, &at)?;
                    append(&mut expanded, &bytes, cx, &at)?;
                    close_tag(doc, element, &writer, &mut expanded, cx)?;
                    patch(&mut patches, closing, expanded, cx, &at)?;
                } else {
                    patch(&mut patches, insert_at..insert_at, bytes, cx, &at)?;
                }
            }
        }
    }
    spend(
        cx,
        Kind::Work,
        (patches.len() as u64).saturating_mul(64),
        &at,
    )?;
    patches.sort_unstable_by_key(|patch| (patch.range.start, patch.range.end));
    let mut end = 0;
    let mut length = doc.src().len() as u64;
    for patch in &patches {
        if patch.range.start < end {
            return Err(invalid(cx, codes::XML_EDIT_RANGE_INVALID, &at));
        }
        end = patch.range.end;
        length = length
            .saturating_sub(patch.range.len() as u64)
            .saturating_add(patch.bytes.len() as u64);
    }
    spend(cx, Kind::DecodedBytes, length, &at)?;
    spend(cx, Kind::Work, length, &at)?;
    let capacity =
        usize::try_from(length).map_err(|_| invalid(cx, codes::XML_WRITE_FAILED, &at))?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(capacity)
        .map_err(|_| invalid(cx, codes::XML_WRITE_FAILED, &at))?;
    let mut cursor = 0;
    for patch in patches {
        output.extend_from_slice(
            doc.src()
                .get(cursor..patch.range.start)
                .ok_or_else(|| invalid(cx, codes::XML_EDIT_RANGE_INVALID, &at))?,
        );
        output.extend_from_slice(&patch.bytes);
        cursor = patch.range.end;
    }
    output.extend_from_slice(
        doc.src()
            .get(cursor..)
            .ok_or_else(|| invalid(cx, codes::XML_EDIT_RANGE_INVALID, &at))?,
    );
    spend(cx, Kind::Work, 0, &at)?;
    Ok(output)
}

fn patch(
    patches: &mut Vec<Patch>,
    range: Range<usize>,
    bytes: Vec<u8>,
    cx: &mut Cx<'_>,
    at: &Ref,
) -> Result<(), Error> {
    spend(cx, Kind::DecodedBytes, 64, at)?;
    patches.push(Patch { range, bytes });
    Ok(())
}

/// 空要素なら / から末尾まで、通常の要素なら終了タグの初めの空範囲。
fn closing(
    doc: &Document,
    element: &Element,
    writer: &Writer,
    cx: &mut Cx<'_>,
) -> Result<Range<usize>, Error> {
    let span = element.span();
    spend(cx, Kind::Work, span.len() as u64, &place(element))?;
    let width = writer.width();
    if writer.ascii(doc.src(), span.end - 2 * width) == Some(b'/') {
        return Ok(span.end - 2 * width..span.end);
    }
    let mut index = span.end - width;
    while index > span.start {
        if writer.ascii(doc.src(), index) == Some(b'<') {
            return Ok(index..index);
        }
        index -= width;
    }
    Err(invalid(cx, codes::XML_EDIT_RANGE_INVALID, &place(element)))
}

fn close_tag(
    doc: &Document,
    element: &Element,
    writer: &Writer,
    out: &mut Vec<u8>,
    cx: &mut Cx<'_>,
) -> Result<(), Error> {
    let at = place(element);
    writer.push(out, "</", cx, &at)?;
    append(out, doc.bytes(element.name().raw()), cx, &at)?;
    writer.push(out, ">", cx, &at)
}

fn replace(
    doc: &Document,
    element: &Element,
    value: &str,
    writer: &Writer,
    patches: &mut Vec<Patch>,
    cx: &mut Cx<'_>,
) -> Result<(), Error> {
    let at = place(element);
    spend(cx, Kind::Work, element.children().len() as u64, &at)?;
    if element
        .children()
        .iter()
        .any(|node| matches!(node, Node::Element(_)))
    {
        return Err(invalid(cx, codes::XML_EDIT_CHILDREN_UNSUPPORTED, &at));
    }
    let mut value = Some(writer.text(value, cx, &at)?);
    let closing = closing(doc, element, writer, cx)?;
    if closing.end > closing.start {
        let mut expanded = writer.encode(">", cx, &at)?;
        append(&mut expanded, value.as_deref().unwrap_or_default(), cx, &at)?;
        close_tag(doc, element, writer, &mut expanded, cx)?;
        return patch(patches, closing, expanded, cx, &at);
    }
    for node in element.children() {
        let range = match node {
            Node::Text(range) => range.clone(),
            Node::CData(range) => range.start - 9 * writer.width()..range.end + 3 * writer.width(),
            Node::Element(_) => continue,
        };
        patch(patches, range, value.take().unwrap_or_default(), cx, &at)?;
    }
    if let Some(value) = value {
        patch(patches, closing, value, cx, &at)?;
    }
    Ok(())
}

fn indentation(
    doc: &Document,
    element: &Element,
    closing: usize,
    writer: &Writer,
    cx: &mut Cx<'_>,
) -> Result<Option<(usize, Vec<u8>)>, Error> {
    let at = place(element);
    spend(cx, Kind::Work, (closing - element.span().start) as u64, &at)?;
    let line = |end: usize| {
        let mut start = end;
        while start > element.span().start {
            let before = start - writer.width();
            match writer.ascii(doc.src(), before) {
                Some(b'\n' | b'\r') => return Some(start),
                Some(b' ' | b'\t') => start = before,
                _ => return None,
            }
        }
        None
    };
    let Some(start) = line(closing) else {
        return Ok(None);
    };
    for child in element.children() {
        if let Node::Element(index) = child
            && let Some(child) = doc.element(*index)
            && let Some(child_start) = line(child.span().start)
        {
            let raw = doc.bytes(child_start..child.span().start);
            spend(cx, Kind::DecodedBytes, raw.len() as u64, &at)?;
            return Ok(Some((start, raw.to_vec())));
        }
    }
    let mut indent = Vec::new();
    append(&mut indent, doc.bytes(start..closing), cx, &at)?;
    writer.push(&mut indent, "  ", cx, &at)?;
    Ok(Some((start, indent)))
}

fn namespace<'a>(
    doc: &'a Document,
    mut parent: Option<&'a Element>,
    prefix: &str,
    cx: &mut Cx<'_>,
    at: &Ref,
) -> Result<bool, Error> {
    // Namespaces in XML 1.0 §6.1: https://www.w3.org/TR/xml-names/#scoping
    if prefix == "xml" {
        return Ok(true);
    }
    if prefix == "xmlns" {
        return Ok(false);
    }
    while let Some(element) = parent {
        spend(cx, Kind::Work, 1, at)?;
        for attribute in element.attributes() {
            spend(cx, Kind::Work, 1, at)?;
            if attribute.name().namespace() != Some(&Namespace::Xmlns)
                || attribute.name().prefix().is_none()
            {
                continue;
            }
            let raw = attribute.name().local();
            spend(cx, Kind::Work, raw.len() as u64, at)?;
            spend(
                cx,
                Kind::DecodedBytes,
                (raw.len() as u64).saturating_mul(6),
                at,
            )?;
            if doc.cdata(raw) == prefix {
                let raw = attribute.raw();
                spend(cx, Kind::Work, raw.len() as u64, at)?;
                spend(
                    cx,
                    Kind::DecodedBytes,
                    (raw.len() as u64).saturating_mul(6),
                    at,
                )?;
                return Ok(!doc.value(attribute).is_empty());
            }
        }
        parent = element.parent().and_then(|index| doc.element(index));
    }
    Ok(false)
}

fn new_element(
    doc: &Document,
    parent: &Element,
    child: &NewElement,
    writer: &Writer,
    out: &mut Vec<u8>,
    cx: &mut Cx<'_>,
) -> Result<(), Error> {
    enum Visit<'a> {
        Open(&'a NewElement),
        Close(&'a str),
    }
    let at = place(parent);
    spend(cx, Kind::DecodedBytes, 32, &at)?;
    let mut stack = vec![Visit::Open(child)];
    let mut entered = 0_u64;
    let result = (|| {
        // 追加後の深さは、既存の親までの階層も含める。
        let mut ancestor = Some(parent);
        while let Some(element) = ancestor {
            spend(cx, Kind::Work, 1, &at)?;
            spend(cx, Kind::Depth, 1, &at)?;
            entered += 1;
            ancestor = element.parent().and_then(|index| doc.element(index));
        }
        while let Some(visit) = stack.pop() {
            spend(cx, Kind::Work, 1, &at)?;
            match visit {
                Visit::Close(name) => {
                    writer.push(out, "</", cx, &at)?;
                    writer.push(out, name, cx, &at)?;
                    writer.push(out, ">", cx, &at)?;
                    cx.leave();
                    entered -= 1;
                }
                Visit::Open(element) => {
                    spend(cx, Kind::Depth, 1, &at)?;
                    entered += 1;
                    spend(cx, Kind::Records, 1, &at)?;
                    spend(cx, Kind::Work, element.name.len() as u64, &at)?;
                    if !name::valid_utf8(element.name.as_bytes()) {
                        return Err(invalid(cx, codes::XML_EDIT_NAME_INVALID, &at));
                    }
                    if let Some((prefix, _)) = element.name.split_once(':')
                        && !namespace(doc, Some(parent), prefix, cx, &at)?
                    {
                        cx.report(
                            Diagnostic::new(codes::XML_EDIT_NAMESPACE_MISSING)
                                .with("prefix", prefix)
                                .at(at.clone()),
                        );
                        return Err(Error::Rejected);
                    }
                    writer.push(out, "<", cx, &at)?;
                    writer.push(out, &element.name, cx, &at)?;
                    if element.value.is_none() && element.children.is_empty() {
                        writer.push(out, "/>", cx, &at)?;
                        cx.leave();
                        entered -= 1;
                        continue;
                    }
                    writer.push(out, ">", cx, &at)?;
                    if let Some(value) = &element.value {
                        append(out, &writer.text(value, cx, &at)?, cx, &at)?;
                    }
                    spend(cx, Kind::Work, element.children.len() as u64, &at)?;
                    spend(
                        cx,
                        Kind::DecodedBytes,
                        (element.children.len() as u64)
                            .saturating_add(1)
                            .saturating_mul(32),
                        &at,
                    )?;
                    stack.push(Visit::Close(&element.name));
                    stack.extend(element.children.iter().rev().map(Visit::Open));
                }
            }
        }
        Ok(())
    })();
    for _ in 0..entered {
        cx.leave();
    }
    result
}

fn place(element: &Element) -> Ref {
    Ref::at_bytes(element.span().start as u64, element.span().end as u64)
}
fn spend(cx: &mut Cx<'_>, kind: Kind, amount: u64, at: &Ref) -> Result<(), Error> {
    cx.check(kind, amount).map_err(|stop| {
        cx.report(stop.to_diagnostic().at(at.clone()));
        Error::Stopped(stop)
    })
}
fn invalid(cx: &mut Cx<'_>, code: Code, at: &Ref) -> Error {
    cx.report(Diagnostic::new(code).at(at.clone()));
    Error::Rejected
}
