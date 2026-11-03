//! L0: ファイルの構造（ISO 32000-2 の 7.5）を、元のバイト範囲のレコードに分ける。
//!
//! 見出し・間接オブジェクト・相互参照表・トレーラー・`startxref`・注記を、ファイルの順に並べる。どれにも当たらない
//! 空白でないバイトは、未知のレコードにする。オブジェクトの中身は解釈せず、文字列・辞書・ストリームの区切りだけを
//! たどって `endobj` を探す。繰り返しは入力の長さで終わり、入れ子は数えるだけで再帰しない。

use zumen_base::{Diagnostic, Kind, Ref, codes};
use zumen_sdk::{Cx, Error};

/// レコードの種類
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecordKind {
    /// ファイルの見出し（`%PDF-`、7.5.2）
    Header,
    /// 間接オブジェクト（`n g obj` から `endobj` まで、7.3.10）
    Object,
    /// 相互参照表（`xref`、7.5.4）
    Xref,
    /// トレーラー（`trailer` と辞書、7.5.5）
    Trailer,
    /// 相互参照の位置（`startxref` と数）
    StartXref,
    /// 注記（`%` から行の終わりまで。`%%EOF` を含む、7.2.4）
    Comment,
    /// どの構造にも当たらないバイト
    Unknown,
}

impl RecordKind {
    /// [`zumen_sdk::RecordInfo::kind`] に入れる名前
    pub fn name(self) -> &'static [u8] {
        match self {
            RecordKind::Header => b"header",
            RecordKind::Object => b"obj",
            RecordKind::Xref => b"xref",
            RecordKind::Trailer => b"trailer",
            RecordKind::StartXref => b"startxref",
            RecordKind::Comment => b"comment",
            RecordKind::Unknown => b"unknown",
        }
    }
}

/// レコード
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Record {
    /// 種類
    pub kind: RecordKind,
    /// 元のバイト範囲の始まり
    pub start: u64,
    /// 元のバイト範囲の終わり（含まない）
    pub end: u64,
    /// 間接オブジェクトの番号と世代番号。オブジェクトでなければ `None`
    pub object: Option<(u32, u16)>,
}

/// 白空白（7.2.3 の表 1）
fn is_white(byte: u8) -> bool {
    matches!(byte, 0x00 | 0x09 | 0x0a | 0x0c | 0x0d | 0x20)
}

/// 区切りの文字（7.2.3 の表 2）
fn is_delimiter(byte: u8) -> bool {
    matches!(
        byte,
        b'(' | b')' | b'<' | b'>' | b'[' | b']' | b'{' | b'}' | b'/' | b'%'
    )
}

fn is_regular(byte: u8) -> bool {
    !is_white(byte) && !is_delimiter(byte)
}

/// ファイルの全体をレコードに分ける。レコードごとに予算の `records` を 1 使う
pub(crate) fn records(src: &[u8], cx: &mut Cx<'_>) -> Result<Vec<Record>, Error> {
    let mut scanner = Scanner {
        src,
        at: 0,
        out: Vec::new(),
        header: false,
    };
    while scanner.at < src.len() {
        let Some(start) = scanner.skip_white() else {
            break;
        };
        let record = scanner.next_record(start, cx);
        if let Err(stop) = cx.check(Kind::Records, 1) {
            cx.report(
                stop.to_diagnostic()
                    .at(Ref::at_bytes(record.start, record.end)),
            );
            return Err(Error::Stopped(stop));
        }
        scanner.push(record);
    }
    Ok(scanner.out)
}

struct Scanner<'s> {
    src: &'s [u8],
    at: usize,
    out: Vec<Record>,
    header: bool,
}

impl Scanner<'_> {
    /// 白空白を飛ばし、次のバイトの位置を返す。入力の終わりなら `None`
    fn skip_white(&mut self) -> Option<usize> {
        while let Some(&byte) = self.src.get(self.at) {
            if !is_white(byte) {
                return Some(self.at);
            }
            self.at += 1;
        }
        None
    }

    fn push(&mut self, record: Record) {
        // 続く未知のバイトは、1 つのレコードにまとめる
        if record.kind == RecordKind::Unknown
            && let Some(last) = self.out.last_mut()
            && last.kind == RecordKind::Unknown
            && self
                .src
                .get(to_usize(last.end)..to_usize(record.start))
                .is_some_and(|gap| gap.iter().all(|&b| is_white(b)))
        {
            last.end = record.end;
            return;
        }
        self.out.push(record);
    }

    fn record(&self, kind: RecordKind, start: usize, object: Option<(u32, u16)>) -> Record {
        Record {
            kind,
            start: start as u64,
            end: self.at as u64,
            object,
        }
    }

    fn next_record(&mut self, start: usize, cx: &mut Cx<'_>) -> Record {
        let rest = self.src.get(start..).unwrap_or_default();
        if rest.first() == Some(&b'%') {
            self.at = line_end(self.src, start);
            let header = !self.header && rest.starts_with(b"%PDF-");
            if header {
                self.header = true;
                return self.record(RecordKind::Header, start, None);
            }
            return self.record(RecordKind::Comment, start, None);
        }
        if let Some((number, generation, body)) = object_start(self.src, start) {
            self.at = body;
            self.object_body(start, cx);
            return self.record(RecordKind::Object, start, Some((number, generation)));
        }
        if keyword_at(self.src, start, b"xref") {
            self.at = start + 4;
            self.xref_body();
            return self.record(RecordKind::Xref, start, None);
        }
        if keyword_at(self.src, start, b"trailer") {
            self.at = start + 7;
            self.trailer_body();
            return self.record(RecordKind::Trailer, start, None);
        }
        if keyword_at(self.src, start, b"startxref") {
            self.at = start + 9;
            if let Some(next) = self.skip_white()
                && self.src.get(next).is_some_and(u8::is_ascii_digit)
            {
                self.at = token_end(self.src, next);
            }
            return self.record(RecordKind::StartXref, start, None);
        }
        // 知らないバイトは、行の終わりまでを未知のレコードにする
        self.at = line_end(self.src, start).max(start + 1);
        self.record(RecordKind::Unknown, start, None)
    }

    /// 相互参照表の行（`開始 個数` と `位置 世代 n|f`）を飛ばす。数と `n`・`f` のほかが来たら終わる
    fn xref_body(&mut self) {
        loop {
            let Some(next) = self.skip_white() else {
                return;
            };
            let end = token_end(self.src, next);
            let token = self.src.get(next..end).unwrap_or_default();
            let entry = !token.is_empty()
                && (token.iter().all(u8::is_ascii_digit) || token == b"n" || token == b"f");
            if !entry {
                return;
            }
            self.at = end;
        }
    }

    /// トレーラーの辞書を飛ばす
    fn trailer_body(&mut self) {
        let Some(next) = self.skip_white() else {
            return;
        };
        if self.src.get(next..next + 2) == Some(b"<<") {
            self.at = next;
            let mut lexer = Lexer::new(self.src, next);
            let mut depth = 0u64;
            while let Some(token) = lexer.next() {
                match token {
                    Token::DictOpen => depth += 1,
                    Token::DictClose => {
                        depth = depth.saturating_sub(1);
                        if depth == 0 {
                            self.at = lexer.at;
                            return;
                        }
                    }
                    _ => {}
                }
            }
            self.at = lexer.at;
        }
    }

    /// 間接オブジェクトの中身を飛ばし、`endobj` の後ろに進む。`endobj` がなく、次のオブジェクトが始まるか入力が
    /// 終われば、そこまでをオブジェクトにする（直して読む）
    fn object_body(&mut self, start: usize, cx: &mut Cx<'_>) {
        let mut lexer = Lexer::new(self.src, self.at);
        let mut depth = 0u64;
        // 外側の辞書の /Length の直接の値。間接参照なら使わない
        let mut length: Option<u64> = None;
        // 直前の 2 つの字句と、その始まり
        let mut previous: [Option<(Token<'_>, usize)>; 2] = [None, None];
        loop {
            let Some(token) = lexer.next() else {
                self.at = lexer.at;
                self.repaired(start, cx);
                return;
            };
            let begun = lexer.start;
            match token {
                Token::DictOpen => depth += 1,
                Token::DictClose => depth = depth.saturating_sub(1),
                Token::Name(b"Length") if depth == 1 => length = direct_length(&lexer),
                Token::Keyword(b"endobj") => {
                    self.at = lexer.at;
                    return;
                }
                Token::Keyword(b"stream") if depth == 0 => {
                    lexer.at = stream_end(self.src, lexer.at, length);
                    length = None;
                }
                Token::Keyword(b"obj") if depth == 0 => {
                    // `endobj` がなく、次のオブジェクトの `n g obj` が来た。2 つの数は次のオブジェクトに入れる
                    if let [Some((Token::Number(_), first)), Some((Token::Number(_), _))] = previous
                    {
                        self.at = first;
                        self.repaired(start, cx);
                        return;
                    }
                }
                _ => {}
            }
            previous = [previous[1], Some((token, begun))];
        }
    }

    /// 終わりのないオブジェクトを、後ろの白空白を除いた所で終える
    fn repaired(&mut self, start: usize, cx: &mut Cx<'_>) {
        while self.at > start && self.src.get(self.at - 1).is_some_and(|&b| is_white(b)) {
            self.at -= 1;
        }
        cx.report(
            Diagnostic::new(codes::PDF_OBJECT_REPAIRED)
                .at(Ref::at_bytes(start as u64, self.at as u64)),
        );
    }
}

/// 字句
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Token<'s> {
    DictOpen,
    DictClose,
    /// 名前。`/` の後ろの生のバイト列
    Name(&'s [u8]),
    /// 符号のない整数なら、その値
    Number(Option<u64>),
    /// そのほかの普通の文字の並び
    Keyword(&'s [u8]),
    /// 文字列・配列の区切り・手続きの区切り
    Other,
}

/// `/Length` の後ろの値が直接の整数なら、その値。`n g R` の間接参照なら `None`
fn direct_length(lexer: &Lexer<'_>) -> Option<u64> {
    let mut ahead = Lexer::new(lexer.src, lexer.at);
    let Some(Token::Number(Some(value))) = ahead.next() else {
        return None;
    };
    let indirect = matches!(ahead.next(), Some(Token::Number(Some(_))))
        && matches!(ahead.next(), Some(Token::Keyword(b"R")));
    (!indirect).then_some(value)
}

/// オブジェクトの中身を区切る字句の読み手（7.2・7.3）。注記は飛ばす
struct Lexer<'s> {
    src: &'s [u8],
    /// 次に読む位置
    at: usize,
    /// 最後に読んだ字句の始まり
    start: usize,
}

impl<'s> Lexer<'s> {
    fn new(src: &'s [u8], at: usize) -> Self {
        Self { src, at, start: at }
    }

    fn next(&mut self) -> Option<Token<'s>> {
        loop {
            let byte = *self.src.get(self.at)?;
            if is_white(byte) {
                self.at += 1;
                continue;
            }
            if byte == b'%' {
                self.at = line_end(self.src, self.at);
                continue;
            }
            break;
        }
        let start = self.at;
        self.start = start;
        let byte = *self.src.get(start)?;
        let token = match byte {
            b'(' => {
                self.at = literal_end(self.src, start);
                Token::Other
            }
            b'<' if self.src.get(start + 1) == Some(&b'<') => {
                self.at = start + 2;
                Token::DictOpen
            }
            b'<' => {
                // 16 進の文字列（7.3.4.3）
                self.at = self
                    .src
                    .get(start..)
                    .and_then(|rest| rest.iter().position(|&b| b == b'>'))
                    .map_or(self.src.len(), |end| start + end + 1);
                Token::Other
            }
            b'>' if self.src.get(start + 1) == Some(&b'>') => {
                self.at = start + 2;
                Token::DictClose
            }
            b'/' => {
                self.at = token_end(self.src, start + 1);
                Token::Name(self.src.get(start + 1..self.at).unwrap_or_default())
            }
            b')' | b'>' | b'[' | b']' | b'{' | b'}' => {
                self.at = start + 1;
                Token::Other
            }
            _ => {
                self.at = token_end(self.src, start);
                let word = self.src.get(start..self.at).unwrap_or_default();
                if word
                    .first()
                    .is_some_and(|b| b.is_ascii_digit() || *b == b'+' || *b == b'-' || *b == b'.')
                {
                    Token::Number(unsigned(word))
                } else {
                    Token::Keyword(word)
                }
            }
        };
        Some(token)
    }
}

/// 符号のない整数の字句の値。整数でないか、大きすぎれば `None`
fn unsigned(word: &[u8]) -> Option<u64> {
    if word.is_empty() || !word.iter().all(u8::is_ascii_digit) {
        return None;
    }
    word.iter().try_fold(0u64, |value, &digit| {
        value.checked_mul(10)?.checked_add(u64::from(digit - b'0'))
    })
}

/// リテラルの文字列（7.3.4.2）の終わりの次の位置。括弧の入れ子と `\` の退避をたどる
fn literal_end(src: &[u8], start: usize) -> usize {
    let mut depth = 0u64;
    let mut at = start;
    while let Some(&byte) = src.get(at) {
        match byte {
            b'\\' => at += 1,
            b'(' => depth += 1,
            b')' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return at + 1;
                }
            }
            _ => {}
        }
        at += 1;
    }
    src.len()
}

/// 普通の文字の並びの終わり
fn token_end(src: &[u8], start: usize) -> usize {
    src.get(start..)
        .and_then(|rest| rest.iter().position(|&b| !is_regular(b)))
        .map_or(src.len(), |end| start + end)
}

/// 行の終わり（CR か LF の位置）
fn line_end(src: &[u8], start: usize) -> usize {
    src.get(start..)
        .and_then(|rest| rest.iter().position(|&b| b == b'\r' || b == b'\n'))
        .map_or(src.len(), |end| start + end)
}

/// `start` にキーワード `word` があり、その後ろが普通の文字でない
fn keyword_at(src: &[u8], start: usize, word: &[u8]) -> bool {
    src.get(start..start + word.len()) == Some(word)
        && !src.get(start + word.len()).is_some_and(|&b| is_regular(b))
}

/// `start` から `n g obj` が始まれば、番号・世代番号と、`obj` の後ろの位置
fn object_start(src: &[u8], start: usize) -> Option<(u32, u16, usize)> {
    let number_end = token_end(src, start);
    let number = u32::try_from(unsigned(src.get(start..number_end)?)?).ok()?;
    let mut at = number_end;
    while src.get(at).is_some_and(|&b| is_white(b)) {
        at += 1;
    }
    if at == number_end {
        return None;
    }
    let generation_end = token_end(src, at);
    let generation = u16::try_from(unsigned(src.get(at..generation_end)?)?).ok()?;
    let mut at = generation_end;
    while src.get(at).is_some_and(|&b| is_white(b)) {
        at += 1;
    }
    if at == generation_end || !keyword_at(src, at, b"obj") {
        return None;
    }
    Some((number, generation, at + 3))
}

/// ストリームの終わり（`endstream` の後ろ）の位置（7.3.8.1）。`stream` の後ろの行の終わりから、直接の長さの
/// 後ろに `endstream` があればそこを、なければ次の `endstream` を探す
fn stream_end(src: &[u8], after_keyword: usize, length: Option<u64>) -> usize {
    let mut data = after_keyword;
    if src.get(data) == Some(&b'\r') {
        data += 1;
    }
    if src.get(data) == Some(&b'\n') {
        data += 1;
    }
    if let Some(length) = length.and_then(|length| usize::try_from(length).ok())
        && let Some(end) = data.checked_add(length)
    {
        let mut at = end;
        while src.get(at).is_some_and(|&b| is_white(b)) {
            at += 1;
        }
        if keyword_at(src, at, b"endstream") {
            return at + 9;
        }
    }
    find(src, data, b"endstream").map_or(src.len(), |at| at + 9)
}

/// `from` から後ろで、最初に `needle` が現れる位置
fn find(src: &[u8], from: usize, needle: &[u8]) -> Option<usize> {
    src.get(from..)?
        .windows(needle.len())
        .position(|window| window == needle)
        .map(|at| from + at)
}

fn to_usize(value: u64) -> usize {
    usize::try_from(value).unwrap_or(usize::MAX)
}
