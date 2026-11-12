//! XML の L0（決定 0008・0209）。
//!
//! 形式の意味を知らずに、要素・属性・文字と名前空間を、元のバイト範囲のまま読む。
//! UTF-8・Shift_JIS・UTF-16LE・UTF-16BE に対応する。UTF-16 の解析用の UTF-8 と原本の位置を
//! 対応づけ、公開する範囲と [`Document::src`] は原本を保つ。文字にするのは [`Document::text`] と
//! [`Document::value`] で行う。名前の比較には [`Document::parsed_local_name`] を使う。
//!
//! 文書型宣言（DTD）は処理しない。外部の DTD と実体を解決せず、DTD で宣言する実体の参照は戻さない。

mod name;
mod parse;
mod text;
mod utf16;

use core::ops::Range;
use std::borrow::Cow;

pub use parse::read;

/// 文書の文字コード
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Encoding {
    /// UTF-8。XML 宣言がないか、宣言が `UTF-8` のもの
    Utf8,
    /// Shift_JIS（WHATWG の Shift_JIS。`Shift_JIS`・`windows-31j` などの名前）
    ShiftJis,
    /// UTF-16 のリトルエンディアン
    Utf16Le,
    /// UTF-16 のビッグエンディアン
    Utf16Be,
}

/// 名前空間
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Namespace {
    /// 文書が宣言した名前空間。範囲は、宣言の属性の値の内側
    Declared(Range<usize>),
    /// 接頭辞 `xml` の名前空間（`http://www.w3.org/XML/1998/namespace`）
    Xml,
    /// 名前空間の宣言の属性（`xmlns`・`xmlns:…`）の名前空間（`http://www.w3.org/2000/xmlns/`）
    Xmlns,
}

/// 要素か属性の名前
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Name {
    raw: Range<usize>,
    local: usize,
    prefix_end: Option<usize>,
    namespace: Option<Namespace>,
}

impl Name {
    /// 書いたままの名前の範囲（`p:name`）
    pub fn raw(&self) -> Range<usize> {
        self.raw.clone()
    }

    /// 接頭辞の範囲。なければ `None`
    pub fn prefix(&self) -> Option<Range<usize>> {
        self.prefix_end.map(|end| self.raw.start..end)
    }

    /// 接頭辞を除いた名前の範囲
    pub fn local(&self) -> Range<usize> {
        self.local..self.raw.end
    }

    /// 名前空間。名前空間のない名前と、宣言されていない接頭辞の名前は `None`
    pub fn namespace(&self) -> Option<&Namespace> {
        self.namespace.as_ref()
    }
}

/// 属性
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Attribute {
    name: Name,
    value: Range<usize>,
}

impl Attribute {
    /// 名前
    pub fn name(&self) -> &Name {
        &self.name
    }

    /// 値の引用符の内側の範囲。参照は戻さない
    pub fn raw(&self) -> Range<usize> {
        self.value.clone()
    }
}

/// 要素の中身
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Node {
    /// 子の要素。[`Document::element`] の番号
    Element(usize),
    /// 文字。参照は戻さない
    Text(Range<usize>),
    /// `<![CDATA[…]]>` の内側
    CData(Range<usize>),
}

/// 要素
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Element {
    span: Range<usize>,
    name: Name,
    attributes: Vec<Attribute>,
    children: Vec<Node>,
    parent: Option<usize>,
}

impl Element {
    /// 開始タグから終了タグまで（空要素はそのタグ）の範囲
    pub fn span(&self) -> Range<usize> {
        self.span.clone()
    }

    /// 名前
    pub fn name(&self) -> &Name {
        &self.name
    }

    /// 属性。書いた順
    pub fn attributes(&self) -> &[Attribute] {
        &self.attributes
    }

    /// 中身。書いた順。注記と処理命令は入れない
    pub fn children(&self) -> &[Node] {
        &self.children
    }

    /// 親の要素の番号。ルートにはない
    pub fn parent(&self) -> Option<usize> {
        self.parent
    }
}

/// 読んだ XML の文書。元のバイト列と、要素
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Document {
    src: Vec<u8>,
    encoding: Encoding,
    elements: Vec<Element>,
    doctype: Option<Range<usize>>,
    decoded: Option<utf16::Decoded>,
}

impl Document {
    /// 文字コード
    pub fn encoding(&self) -> Encoding {
        self.encoding
    }

    /// 元のバイト列
    pub fn src(&self) -> &[u8] {
        &self.src
    }

    /// 元のバイト列の範囲。外なら空
    pub fn bytes(&self, range: Range<usize>) -> &[u8] {
        self.src.get(range).unwrap_or_default()
    }

    /// ルートの要素の番号。いつも 0
    pub fn root(&self) -> usize {
        0
    }

    /// 番号の要素。番号は、開始タグを書いた順
    pub fn element(&self, index: usize) -> Option<&Element> {
        self.elements.get(index)
    }

    /// 要素。開始タグを書いた順
    pub fn elements(&self) -> &[Element] {
        &self.elements
    }

    /// 文書型宣言（`<!DOCTYPE …>`）の範囲。処理はしない
    pub fn doctype(&self) -> Option<Range<usize>> {
        self.doctype.clone()
    }

    /// 名前の、接頭辞を除いた部分のバイト列
    pub fn local_name(&self, name: &Name) -> &[u8] {
        self.bytes(name.local())
    }

    /// 字句解析に使う名前。UTF-16 は UTF-8、UTF-8 と Shift_JIS は元の文字コードのまま。
    /// ASCII の要素名の比較に使う。原本の取得は [`Self::local_name`]、文字は [`Self::local_name_text`]。
    pub fn parsed_local_name(&self, name: &Name) -> &[u8] {
        match &self.decoded {
            Some(decoded) => decoded
                .bytes
                .get(decoded.decoded_range(name.local()))
                .unwrap_or_default(),
            None => self.local_name(name),
        }
    }

    /// 接頭辞を除いた名前を文字にする。原本の範囲を返す [`Self::local_name`] と区別する。
    pub fn local_name_text(&self, name: &Name) -> Cow<'_, str> {
        if let Some(decoded) = &self.decoded {
            return String::from_utf8_lossy(
                decoded
                    .bytes
                    .get(decoded.decoded_range(name.local()))
                    .unwrap_or_default(),
            );
        }
        match self.encoding {
            Encoding::Utf8 => String::from_utf8_lossy(self.local_name(name)),
            Encoding::ShiftJis => {
                encoding_rs::SHIFT_JIS
                    .decode_without_bom_handling(self.local_name(name))
                    .0
            }
            Encoding::Utf16Le | Encoding::Utf16Be => Cow::Owned(self.text(name.local())),
        }
    }

    /// 名前空間の名前（URI）。名前空間がなければ `None`
    pub fn namespace(&self, name: &Name) -> Option<String> {
        match name.namespace()? {
            Namespace::Declared(range) => Some(self.value_of(range.clone())),
            Namespace::Xml => Some(String::from("http://www.w3.org/XML/1998/namespace")),
            Namespace::Xmlns => Some(String::from("http://www.w3.org/2000/xmlns/")),
        }
    }

    /// 名前の名前空間が `namespace`（URI）で、接頭辞を除いた名前が `local` なら真。
    /// `local` は [`Document::parsed_local_name`] とバイトで比べる。非 ASCII の文字列は [`Document::local_name_text`] で比べる
    pub fn is(&self, name: &Name, namespace: Option<&str>, local: &[u8]) -> bool {
        self.parsed_local_name(name) == local && self.namespace(name).as_deref() == namespace
    }

    /// 文字の範囲を文字にする。文字コードを戻し、改行を `\n` にそろえ、定義済みの実体と文字の参照を戻す。
    /// DTD で宣言する実体の参照は、そのまま残す
    pub fn text(&self, range: Range<usize>) -> String {
        text::decode(self.encoding, self.bytes(range), text::Mode::Text)
    }

    /// `<![CDATA[…]]>` の内側を文字にする。文字コードを戻し、改行を `\n` にそろえる。参照は戻さない
    pub fn cdata(&self, range: Range<usize>) -> String {
        text::decode(self.encoding, self.bytes(range), text::Mode::CData)
    }

    /// 属性の値を文字にする。[`Document::text`] に加え、XML の属性の値の正規化で、空白の文字を空白にする
    pub fn value(&self, attribute: &Attribute) -> String {
        self.value_of(attribute.raw())
    }

    fn value_of(&self, range: Range<usize>) -> String {
        text::decode(self.encoding, self.bytes(range), text::Mode::Attribute)
    }

    /// 要素の、名前空間と接頭辞を除いた名前が合う属性
    pub fn attribute<'e>(
        &self,
        element: &'e Element,
        namespace: Option<&str>,
        local: &[u8],
    ) -> Option<&'e Attribute> {
        element
            .attributes
            .iter()
            .find(|attribute| self.is(&attribute.name, namespace, local))
    }
}
