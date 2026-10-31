//! IDS の `xs:pattern` の正規表現（XML Schema Part 2 の附属書 F）。文字列の全体に合うかを見る。
//!
//! 照合は状態の集まりを 1 文字ずつ進めるので、入力の長さと式の大きさの積の手間で終わる。
//! `\p{…}`・`\P{…}` は読まない。`\d`・`\w`・`\i`・`\c` とその否定は、ASCII の文字だけを決め、
//! ほかの文字では決められないとする。

/// 読んだ式
#[derive(Clone, Debug, PartialEq)]
pub struct Pattern {
    nodes: Vec<Node>,
    start: usize,
}

/// 文字の集まりの 1 つの項目
#[derive(Clone, Debug, PartialEq)]
enum Item {
    Range(char, char),
    Class(Class),
}

/// 複数の文字の略記
#[derive(Clone, Copy, Debug, PartialEq)]
enum Class {
    /// `\s`
    Space,
    /// `\d`
    Digit,
    /// `\w`
    Word,
    /// `\i`
    NameStart,
    /// `\c`
    NameChar,
    /// `.`
    Any,
}

#[derive(Clone, Debug, PartialEq)]
struct Set {
    negated: bool,
    items: Vec<(Item, bool)>,
    subtract: Option<Box<Set>>,
}

#[derive(Clone, Debug, PartialEq)]
enum Node {
    /// 1 文字に合えば `next` へ
    Char(Set, usize),
    /// 2 つに分かれる
    Split(usize, usize),
    /// 合った
    Match,
}

/// 式の木
#[derive(Clone, Debug)]
enum Ast {
    Set(Set),
    Concat(Vec<Ast>),
    Alt(Vec<Ast>),
    Repeat(Box<Ast>, u32, Option<u32>),
}

/// 展開した後の状態の数の上限。`{n,m}` の大きな数で、式が膨らみすぎないようにする
const MAX_NODES: usize = 10_000;

impl Pattern {
    /// 式を読む。読めない式か、読まない書き方（`\p{…}` など）を含めば `None`
    pub fn new(src: &str) -> Option<Self> {
        let chars: Vec<char> = src.chars().collect();
        let mut parser = Parser {
            chars: &chars,
            at: 0,
        };
        let ast = parser.alternation()?;
        if parser.at != chars.len() {
            return None;
        }
        let mut pattern = Self {
            nodes: Vec::new(),
            start: 0,
        };
        let end = pattern.push(Node::Match)?;
        pattern.start = pattern.compile(&ast, end)?;
        Some(pattern)
    }

    /// 文字列の全体が式に合うか。決められない文字があれば `None`
    pub fn matches(&self, text: &str) -> Option<bool> {
        let mut current = Vec::new();
        let mut seen = vec![false; self.nodes.len()];
        self.close(self.start, &mut current, &mut seen);
        let mut unknown = false;
        for c in text.chars() {
            let mut next = Vec::new();
            let mut seen = vec![false; self.nodes.len()];
            for &state in &current {
                if let Some(Node::Char(set, to)) = self.nodes.get(state) {
                    match set.contains(c) {
                        Some(true) => self.close(*to, &mut next, &mut seen),
                        Some(false) => {}
                        None => unknown = true,
                    }
                }
            }
            if next.is_empty() {
                return if unknown { None } else { Some(false) };
            }
            current = next;
        }
        let matched = current
            .iter()
            .any(|&state| matches!(self.nodes.get(state), Some(Node::Match)));
        match (matched, unknown) {
            (true, _) => Some(true),
            (false, true) => None,
            (false, false) => Some(false),
        }
    }

    /// `state` から、文字を読まずに届く状態を集める
    fn close(&self, state: usize, out: &mut Vec<usize>, seen: &mut [bool]) {
        let mut stack = vec![state];
        while let Some(state) = stack.pop() {
            match seen.get_mut(state) {
                Some(flag) if !*flag => *flag = true,
                _ => continue,
            }
            match self.nodes.get(state) {
                Some(Node::Split(a, b)) => {
                    stack.push(*b);
                    stack.push(*a);
                }
                Some(_) => out.push(state),
                None => {}
            }
        }
    }

    fn push(&mut self, node: Node) -> Option<usize> {
        if self.nodes.len() >= MAX_NODES {
            return None;
        }
        self.nodes.push(node);
        Some(self.nodes.len() - 1)
    }

    /// `ast` を、終わりが `next` へ続く状態にする。初めの状態を返す
    fn compile(&mut self, ast: &Ast, next: usize) -> Option<usize> {
        match ast {
            Ast::Set(set) => self.push(Node::Char(set.clone(), next)),
            Ast::Concat(parts) => {
                let mut at = next;
                for part in parts.iter().rev() {
                    at = self.compile(part, at)?;
                }
                Some(at)
            }
            Ast::Alt(branches) => {
                let mut starts = Vec::with_capacity(branches.len());
                for branch in branches {
                    starts.push(self.compile(branch, next)?);
                }
                let mut at = starts.pop()?;
                while let Some(start) = starts.pop() {
                    at = self.push(Node::Split(start, at))?;
                }
                Some(at)
            }
            Ast::Repeat(inner, min, max) => {
                // 後ろから作る
                let mut at = next;
                match max {
                    None => {
                        let split = self.push(Node::Split(0, next))?;
                        let body = self.compile(inner, split)?;
                        if let Some(Node::Split(first, _)) = self.nodes.get_mut(split) {
                            *first = body;
                        }
                        at = split;
                    }
                    Some(max) => {
                        // 任意の分の 1 つを省けば、残りも省いて `next` へ
                        for _ in *min..*max {
                            let body = self.compile(inner, at)?;
                            at = self.push(Node::Split(body, next))?;
                        }
                    }
                }
                for _ in 0..*min {
                    at = self.compile(inner, at)?;
                }
                Some(at)
            }
        }
    }
}

impl Set {
    fn contains(&self, c: char) -> Option<bool> {
        let mut found = Some(false);
        for (item, negated) in &self.items {
            let hit = match item {
                Item::Range(low, high) => Some(*low <= c && c <= *high),
                Item::Class(class) => class.contains(c),
            }
            .map(|hit| hit != *negated);
            found = match (found, hit) {
                (Some(true), _) | (_, Some(true)) => Some(true),
                (None, _) | (_, None) => None,
                _ => Some(false),
            };
        }
        let found = found.map(|f| f != self.negated);
        match (&self.subtract, found) {
            (_, Some(false)) => Some(false),
            (None, found) => found,
            (Some(subtract), found) => match subtract.contains(c) {
                Some(true) => Some(false),
                Some(false) => found,
                None => None,
            },
        }
    }
}

impl Class {
    fn contains(self, c: char) -> Option<bool> {
        match self {
            Self::Space => Some(matches!(c, ' ' | '\t' | '\n' | '\r')),
            Self::Any => Some(!matches!(c, '\n' | '\r')),
            _ if !c.is_ascii() => None,
            Self::Digit => Some(c.is_ascii_digit()),
            // ASCII では、句読点・空白・制御の文字でないもの
            Self::Word => Some(
                c.is_ascii_alphanumeric()
                    || matches!(c, '$' | '+' | '<' | '=' | '>' | '^' | '`' | '|' | '~'),
            ),
            Self::NameStart => Some(c.is_ascii_alphabetic() || matches!(c, '_' | ':')),
            Self::NameChar => Some(c.is_ascii_alphanumeric() || matches!(c, '_' | ':' | '-' | '.')),
        }
    }
}

struct Parser<'c> {
    chars: &'c [char],
    at: usize,
}

impl Parser<'_> {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.at).copied()
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.at += 1;
        Some(c)
    }

    fn eat(&mut self, c: char) -> bool {
        if self.peek() == Some(c) {
            self.at += 1;
            true
        } else {
            false
        }
    }

    fn alternation(&mut self) -> Option<Ast> {
        let mut branches = vec![self.branch()?];
        while self.eat('|') {
            branches.push(self.branch()?);
        }
        Some(if branches.len() == 1 {
            branches.pop()?
        } else {
            Ast::Alt(branches)
        })
    }

    fn branch(&mut self) -> Option<Ast> {
        let mut pieces = Vec::new();
        while let Some(c) = self.peek() {
            if c == '|' || c == ')' {
                break;
            }
            let atom = self.atom()?;
            pieces.push(self.quantifier(atom)?);
        }
        Some(Ast::Concat(pieces))
    }

    fn quantifier(&mut self, atom: Ast) -> Option<Ast> {
        let (min, max) = match self.peek() {
            Some('?') => (0, Some(1)),
            Some('*') => (0, None),
            Some('+') => (1, None),
            Some('{') => {
                self.at += 1;
                let min = self.number()?;
                let max = if self.eat(',') {
                    if self.peek() == Some('}') {
                        None
                    } else {
                        Some(self.number()?)
                    }
                } else {
                    Some(min)
                };
                if !self.eat('}') || max.is_some_and(|max| max < min) {
                    return None;
                }
                return Some(Ast::Repeat(Box::new(atom), min, max));
            }
            _ => return Some(atom),
        };
        self.at += 1;
        Some(Ast::Repeat(Box::new(atom), min, max))
    }

    fn number(&mut self) -> Option<u32> {
        let start = self.at;
        let mut value: u32 = 0;
        while let Some(digit) = self.peek().and_then(|c| c.to_digit(10)) {
            value = value.checked_mul(10)?.checked_add(digit)?;
            self.at += 1;
        }
        (self.at > start && value <= MAX_NODES as u32).then_some(value)
    }

    fn atom(&mut self) -> Option<Ast> {
        match self.bump()? {
            '(' => {
                let inner = self.alternation()?;
                self.eat(')').then_some(inner)
            }
            '[' => Some(Ast::Set(self.class_body()?)),
            '.' => Some(Ast::Set(single(Item::Class(Class::Any), false))),
            '\\' => {
                let (item, negated) = self.escape()?;
                Some(Ast::Set(single(item, negated)))
            }
            '?' | '*' | '+' | '{' | '}' | ']' | ')' => None,
            c => Some(Ast::Set(single(Item::Range(c, c), false))),
        }
    }

    /// `\` の後。1 文字の逃がしか、複数の文字の略記。否定の略記は真を返す
    fn escape(&mut self) -> Option<(Item, bool)> {
        let c = self.bump()?;
        let class = |class| Some((Item::Class(class), false));
        let negated = |class| Some((Item::Class(class), true));
        match c {
            'n' => Some((Item::Range('\n', '\n'), false)),
            'r' => Some((Item::Range('\r', '\r'), false)),
            't' => Some((Item::Range('\t', '\t'), false)),
            '\\' | '|' | '.' | '-' | '^' | '?' | '*' | '+' | '{' | '}' | '(' | ')' | '[' | ']' => {
                Some((Item::Range(c, c), false))
            }
            's' => class(Class::Space),
            'S' => negated(Class::Space),
            'd' => class(Class::Digit),
            'D' => negated(Class::Digit),
            'w' => class(Class::Word),
            'W' => negated(Class::Word),
            'i' => class(Class::NameStart),
            'I' => negated(Class::NameStart),
            'c' => class(Class::NameChar),
            'C' => negated(Class::NameChar),
            _ => None,
        }
    }

    /// `[` の後から `]` まで
    fn class_body(&mut self) -> Option<Set> {
        let negated = self.eat('^');
        let mut items = Vec::new();
        let mut subtract = None;
        loop {
            match self.peek()? {
                ']' if !items.is_empty() => {
                    self.at += 1;
                    break;
                }
                '-' if self.chars.get(self.at + 1) == Some(&'[') && !items.is_empty() => {
                    self.at += 2;
                    subtract = Some(Box::new(self.class_body()?));
                    if !self.eat(']') {
                        return None;
                    }
                    break;
                }
                _ => {}
            }
            let first = match self.bump()? {
                '\\' => match self.escape()? {
                    (Item::Range(c, _), _) => c,
                    class => {
                        items.push(class);
                        continue;
                    }
                },
                '[' => return None,
                c => c,
            };
            // 範囲 `a-z`。`-` が終わりの `]` の前なら、ただの文字
            if self.peek() == Some('-')
                && !matches!(self.chars.get(self.at + 1), Some(']') | Some('['))
            {
                self.at += 1;
                let last = match self.bump()? {
                    '\\' => match self.escape()? {
                        (Item::Range(c, _), _) => c,
                        _ => return None,
                    },
                    '[' => return None,
                    c => c,
                };
                if last < first {
                    return None;
                }
                items.push((Item::Range(first, last), false));
            } else {
                items.push((Item::Range(first, first), false));
            }
        }
        Some(Set {
            negated,
            items,
            subtract,
        })
    }
}

fn single(item: Item, negated: bool) -> Set {
    Set {
        negated: false,
        items: vec![(item, negated)],
        subtract: None,
    }
}

#[cfg(test)]
mod tests {
    use super::Pattern;

    fn check(pattern: &str, text: &str) -> Option<bool> {
        Pattern::new(pattern).and_then(|p| p.matches(text))
    }

    #[test]
    fn matches_the_whole_string() {
        assert_eq!(check("abc", "abc"), Some(true));
        assert_eq!(check("abc", "xabc"), Some(false));
        assert_eq!(check("ab|cd", "cd"), Some(true));
        assert_eq!(check("a(b|c)*d", "abcbd"), Some(true));
        assert_eq!(check("a(b|c)*d", "abxd"), Some(false));
        assert_eq!(check("[A-Z]{2,3}[0-9]+", "AB12"), Some(true));
        assert_eq!(check("[A-Z]{2,3}[0-9]+", "ABCD1"), Some(false));
        assert_eq!(check("[A-Z]{2}", "A"), Some(false));
        assert_eq!(check("x{0,2}", ""), Some(true));
        assert_eq!(check("x{0,2}", "xxx"), Some(false));
        assert_eq!(check("x{2,}", "xxxx"), Some(true));
        assert_eq!(check("[^a-c]", "d"), Some(true));
        assert_eq!(check("[a-z-[aeiou]]+", "bcd"), Some(true));
        assert_eq!(check("[a-z-[aeiou]]+", "bad"), Some(false));
        assert_eq!(check("\\d{3}-\\d{4}", "123-4567"), Some(true));
        assert_eq!(check("[+\\-]?1", "-1"), Some(true));
        assert_eq!(check("a.c", "a\nc"), Some(false));
        assert_eq!(check("壁.*", "壁タイプ"), Some(true));
        assert_eq!(check("", ""), Some(true));
    }

    #[test]
    fn leaves_classes_it_cannot_decide() {
        assert_eq!(check("\\w+", "壁"), None);
        assert_eq!(check("\\w+", "ab"), Some(true));
        // ほかの文字で合わないと決まれば、決められない文字があっても合わない
        assert_eq!(check("a\\w", "b壁"), Some(false));
        // 式が読めない
        for broken in [
            "(a",
            "a)",
            "[a",
            "*a",
            "a{2,1}",
            "\\p{L}",
            "a{99999999}",
            "[]",
        ] {
            assert!(Pattern::new(broken).is_none(), "{broken}");
        }
    }
}
