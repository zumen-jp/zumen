//! XML の L0 の文書から要素を引く道具と、XML を書く道具

use zumen_xml::{Document, Element, Encoding, Node};

/// 読んだ XML の文書。BCF は名前空間を使わないので、名前は接頭辞を除いた部分で比べる
pub(crate) struct Xml<'d> {
    pub(crate) doc: &'d Document,
}

impl<'d> Xml<'d> {
    pub(crate) fn root(&self) -> Option<&'d Element> {
        self.doc.element(self.doc.root())
    }

    pub(crate) fn local(&self, element: &Element) -> &'d [u8] {
        self.doc.local_name(element.name())
    }

    /// 子の要素。書いた順
    pub(crate) fn children(&self, element: &Element) -> Vec<&'d Element> {
        element
            .children()
            .iter()
            .filter_map(|node| match node {
                Node::Element(index) => self.doc.element(*index),
                _ => None,
            })
            .collect()
    }

    /// 名前の合う初めの子
    pub(crate) fn child(&self, element: &Element, local: &[u8]) -> Option<&'d Element> {
        self.children(element)
            .into_iter()
            .find(|child| self.local(child) == local)
    }

    /// 名前の合う子のすべて
    pub(crate) fn all(&self, element: &Element, local: &[u8]) -> Vec<&'d Element> {
        self.children(element)
            .into_iter()
            .filter(|child| self.local(child) == local)
            .collect()
    }

    /// 要素の中の文字。前後の空白を除く
    pub(crate) fn text(&self, element: &Element) -> String {
        let mut out = String::new();
        for node in element.children() {
            match node {
                Node::Text(range) => out.push_str(&self.doc.text(range.clone())),
                Node::CData(range) => out.push_str(&self.doc.cdata(range.clone())),
                Node::Element(_) => {}
            }
        }
        out.trim().to_string()
    }

    /// 名前の合う子の文字。子がないか、文字が空なら `None`
    pub(crate) fn child_text(&self, element: &Element, local: &[u8]) -> Option<String> {
        self.child(element, local)
            .map(|child| self.text(child))
            .filter(|text| !text.is_empty())
    }

    pub(crate) fn attribute(&self, element: &Element, local: &[u8]) -> Option<String> {
        element
            .attributes()
            .iter()
            .find(|a| a.name().namespace().is_none() && self.doc.local_name(a.name()) == local)
            .map(|a| self.doc.value(a).trim().to_string())
    }

    /// 要素の元のバイト列。UTF-8 の文書のときだけ。書き戻す文書は UTF-8 なので、ほかの文字コードは写さない
    pub(crate) fn raw(&self, element: &Element) -> Option<&'d [u8]> {
        (self.doc.encoding() == Encoding::Utf8).then(|| self.doc.bytes(element.span()))
    }

    /// 属性の元のバイト列（`name="value"`）
    pub(crate) fn raw_attributes(&self, element: &Element, known: &[&[u8]]) -> Vec<&'d [u8]> {
        if self.doc.encoding() != Encoding::Utf8 {
            return Vec::new();
        }
        element
            .attributes()
            .iter()
            .filter(|a| {
                let local = self.doc.local_name(a.name());
                // 名前空間の宣言と、書き直す属性は写さない
                a.name()
                    .prefix()
                    .is_none_or(|p| self.doc.bytes(p) != b"xmlns")
                    && local != b"xmlns"
                    && !(a.name().namespace().is_none() && known.contains(&local))
            })
            .map(|a| {
                let start = a.name().raw().start;
                let end = a.raw().end.saturating_add(1);
                self.doc.bytes(start..end)
            })
            .collect()
    }
}

/// XML を書く。要素は字下げして 1 行ずつ書く
#[derive(Default)]
pub(crate) struct Writer {
    pub(crate) out: Vec<u8>,
    depth: usize,
}

impl Writer {
    pub(crate) fn new() -> Self {
        let mut writer = Self::default();
        writer
            .out
            .extend_from_slice(b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
        writer
    }

    fn indent(&mut self) {
        for _ in 0..self.depth {
            self.out.extend_from_slice(b"  ");
        }
    }

    fn head(&mut self, name: &str, attributes: &[(&str, &str)], raw: &[&[u8]]) {
        self.indent();
        self.out.push(b'<');
        self.out.extend_from_slice(name.as_bytes());
        for (key, value) in attributes {
            self.out.push(b' ');
            self.out.extend_from_slice(key.as_bytes());
            self.out.extend_from_slice(b"=\"");
            escape(&mut self.out, value, true);
            self.out.push(b'"');
        }
        for attribute in raw {
            self.out.push(b' ');
            self.out.extend_from_slice(attribute);
        }
    }

    /// 開きタグ
    pub(crate) fn open(&mut self, name: &str, attributes: &[(&str, &str)], raw: &[&[u8]]) {
        self.head(name, attributes, raw);
        self.out.extend_from_slice(b">\n");
        self.depth += 1;
    }

    pub(crate) fn close(&mut self, name: &str) {
        self.depth = self.depth.saturating_sub(1);
        self.indent();
        self.out.extend_from_slice(b"</");
        self.out.extend_from_slice(name.as_bytes());
        self.out.extend_from_slice(b">\n");
    }

    /// 中身のない要素
    pub(crate) fn empty(&mut self, name: &str, attributes: &[(&str, &str)]) {
        self.head(name, attributes, &[]);
        self.out.extend_from_slice(b"/>\n");
    }

    /// 文字だけを持つ要素
    pub(crate) fn text(&mut self, name: &str, text: &str) {
        self.head(name, &[], &[]);
        self.out.push(b'>');
        escape(&mut self.out, text, false);
        self.out.extend_from_slice(b"</");
        self.out.extend_from_slice(name.as_bytes());
        self.out.extend_from_slice(b">\n");
    }

    /// 値があれば、文字だけを持つ要素
    pub(crate) fn optional(&mut self, name: &str, text: Option<&str>) {
        if let Some(text) = text.filter(|t| !t.trim().is_empty()) {
            self.text(name, text);
        }
    }

    pub(crate) fn number(&mut self, name: &str, value: f64) {
        self.text(name, &number(value));
    }

    /// 元のバイト列の要素
    pub(crate) fn raw(&mut self, bytes: &[u8]) {
        self.indent();
        self.out.extend_from_slice(bytes);
        self.out.push(b'\n');
    }
}

/// 数を、読み直して同じ値になる最も短い形で書く
pub(crate) fn number(value: f64) -> String {
    if value == 0.0 {
        return "0".to_string();
    }
    format!("{value}")
}

fn escape(out: &mut Vec<u8>, text: &str, attribute: bool) {
    for c in text.chars() {
        match c {
            '&' => out.extend_from_slice(b"&amp;"),
            '<' => out.extend_from_slice(b"&lt;"),
            '>' => out.extend_from_slice(b"&gt;"),
            '"' if attribute => out.extend_from_slice(b"&quot;"),
            '\n' if attribute => out.extend_from_slice(b"&#10;"),
            '\t' if attribute => out.extend_from_slice(b"&#9;"),
            '\r' => out.extend_from_slice(b"&#13;"),
            _ => {
                let mut buffer = [0u8; 4];
                out.extend_from_slice(c.encode_utf8(&mut buffer).as_bytes());
            }
        }
    }
}
