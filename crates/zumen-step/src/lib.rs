//! ISO 10303-21 の交換構造（L0。決定 0008・0120・0207）。
//!
//! 交換構造（`ISO-10303-21;` で始まり、見出しとデータの節を持つ）を、文（`;` で終わるもの）ごとのレコードに分けて
//! 読む。値は元のバイト列の範囲で持ち、文字列のエスケープは戻さない。戻すのは [`decode`] で、形式の lift が使う。
//! スキーマは知らない。SXF の SFC のフィーチャのコメントのような、形式に固有の書き方は [`Dialect`] で渡す。

mod parse;
mod text;

use core::ops::Range;

use zumen_base::Code;

pub use parse::read;
pub use text::decode;

/// 形式ごとの読み方
#[derive(Clone, Copy, Debug)]
pub struct Dialect {
    /// 読めない文やコメントを報告する診断。場所は、その文のバイト範囲
    pub record_invalid: Code,
    /// 交換構造になっていないときの診断
    pub file_invalid: Code,
    /// データの節で、レコードとして読むコメントの開きと閉じ。長いものから並べる。中は `#n = 名前(引数)` の 1 つで、
    /// 文字列は次の `'` で閉じ、`\'…\'` の囲みと囲まない値を読む（SXF の SFC のフィーチャ）
    pub features: &'static [(&'static [u8], &'static [u8])],
    /// データの節の前の `FILE_DESCRIPTION` の最初の文字列がこの語を含めば、`features` をただのコメントとする
    pub plain_if_described: Option<&'static [u8]>,
}

/// 読んだ交換構造
#[derive(Clone, Debug, PartialEq)]
pub struct Exchange {
    /// レコード。書いた順
    pub records: Vec<Record>,
    /// `FILE_DESCRIPTION` の最初の文字列の内側
    pub description: Option<Range<usize>>,
    /// `FILE_SCHEMA` の最初のスキーマの文字列の内側
    pub schema: Option<Range<usize>>,
}

/// レコードの形
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Form {
    /// 引数のない語（`HEADER;` など）
    Keyword,
    /// 名前と引数（`FILE_NAME(…);`・`#1=LINE(…);`・フィーチャのコメント）
    Entity,
    /// 部分を並べた複合のインスタンス（`#1=(A() B());`）
    Complex,
}

/// レコード。文か、フィーチャのコメント
#[derive(Clone, Debug, PartialEq)]
pub struct Record {
    span: Range<usize>,
    id: Option<u64>,
    feature: Option<usize>,
    form: Form,
    parts: Vec<Part>,
}

impl Record {
    /// 元のバイト範囲。文は `;` まで、フィーチャはコメントの閉じまで
    pub fn span(&self) -> Range<usize> {
        self.span.clone()
    }

    /// インスタンスの番号（`#n`）。見出しの文と語にはない
    pub fn id(&self) -> Option<u64> {
        self.id
    }

    /// フィーチャのコメントなら、[`Dialect::features`] の何番目の開きと閉じか。文にはない
    pub fn feature(&self) -> Option<usize> {
        self.feature
    }

    /// 形
    pub fn form(&self) -> Form {
        self.form
    }

    /// 部分。語と名前と引数のものは 1 つ、複合のインスタンスは書いた順
    pub fn parts(&self) -> &[Part] {
        &self.parts
    }
}

/// 名前と引数
#[derive(Clone, Debug, PartialEq)]
pub struct Part {
    name: Range<usize>,
    args: Vec<Value>,
}

impl Part {
    /// 名前の範囲
    pub fn name(&self) -> Range<usize> {
        self.name.clone()
    }

    /// 引数
    pub fn args(&self) -> &[Value] {
        &self.args
    }
}

/// 文字列の囲み方
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Quote {
    /// `'…'`。文では `''` で `'` を表す。フィーチャのコメントでは次の `'` で閉じる
    Plain,
    /// フィーチャのコメントの `\'…\'`。Shift_JIS の 2 バイト目と `\\` を飛ばし、後に `,` か `)` が続く `\'` で閉じる
    Backslash,
}

/// 引数の値。範囲は元のバイト列の中を指す
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    /// `$`（値がない）
    Null,
    /// `*`（導く値）
    Derived,
    /// `#n`（インスタンスの参照）
    Ref(u64),
    /// `.E.` の内側
    Enum(Range<usize>),
    /// 文字列の内側。エスケープは戻さない
    Text {
        /// 内側の範囲
        raw: Range<usize>,
        /// 囲み方
        quote: Quote,
    },
    /// 数
    Number(Range<usize>),
    /// `"…"` の内側の 16 進
    Binary(Range<usize>),
    /// 型を付けた値（`LENGTH_MEASURE(1.)`）
    Typed(Part),
    /// `(…)`
    List(Vec<Value>),
    /// フィーチャのコメントの、囲まない値。前後の空白を除いた範囲
    Token(Range<usize>),
}
