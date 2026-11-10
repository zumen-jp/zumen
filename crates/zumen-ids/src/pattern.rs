//! IDS の `xs:pattern` の正規表現（XML Schema Part 2 の附属書 F）。文字列の全体に合うかを見る。
//!
//! 照合は状態の集まりを 1 文字ずつ進めるので、入力の長さと式の大きさの積の手間で終わる。その手間は
//! [`Pattern::work`] で、照合の前に予算に数える（#732）。読むときは、入れ子の深さと、展開する手間に上限を
//! 付ける。
//! `\p{…}`・`\P{…}` は読まない。`\d`・`\w`・`\i`・`\c` とその否定は、ASCII の文字だけを決め、
//! ほかの文字では決められないとする。

/// 読んだ式
#[derive(Clone, Debug, PartialEq)]
pub struct Pattern {
    nodes: Vec<Node>,
    /// 文字の集まり。`{n,m}` で同じ集まりを何度も展開しても、写さずに番号で指す
    sets: Vec<Set>,
    start: usize,
    /// 1 文字を進める手間の上限。状態の数と、文字の集まりの項目の数の和
    cost: u64,
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
    /// 1 文字が集まり（`sets` の番号）に合えば `next` へ
    Char(usize, usize),
    /// 2 つに分かれる
    Split(usize, usize),
    /// 合った
    Match,
}

/// 式の木
#[derive(Clone, Debug)]
enum Ast {
    /// 文字の集まり（`sets` の番号）
    Set(usize),
    Concat(Vec<Ast>),
    Alt(Vec<Ast>),
    Repeat(Box<Ast>, u32, Option<u32>),
}

/// 展開した後の状態の数の上限。`{n,m}` の大きな数で、式が膨らみすぎないようにする
const MAX_NODES: usize = 10_000;

/// 展開する手間（式の木の節を展開する回数）の上限。空の式を `{n,m}` で重ねても、状態を足さずに回り続けない
/// ようにする（#732）
const MAX_STEPS: usize = 4 * MAX_NODES;

/// 括弧と、文字の集まりの引き算（`[a-z-[aeiou]]`）の入れ子の深さの上限。読む・展開する・照合する再帰が、
/// スタックを使い切らないようにする（#732）
const MAX_DEPTH: usize = 64;

impl Pattern {
    /// 式を読む。読めない式か、読まない書き方（`\p{…}` など）を含めば `None`
    #[cfg(test)]
    fn new(src: &str) -> Option<Self> {
        Self::read(src).0
    }

    /// 式を読み、読んだ手間（式の文字の数と、展開した回数）も返す。手間は、式の文字の数と [`MAX_STEPS`] + 1 の和を
    /// 超えない（#732）
    pub fn read(src: &str) -> (Option<Self>, u64) {
        let chars: Vec<char> = src.chars().collect();
        let mut parser = Parser {
            chars: &chars,
            at: 0,
            depth: 0,
            sets: Vec::new(),
        };
        let read = chars.len() as u64;
        let ast = match parser.alternation() {
            Some(ast) if parser.at == chars.len() => ast,
            _ => return (None, read),
        };
        let mut pattern = Self {
            nodes: Vec::new(),
            sets: parser.sets,
            start: 0,
            cost: 0,
        };
        let mut steps = 0;
        let compiled = pattern
            .push(Node::Match)
            .and_then(|end| pattern.compile(&ast, end, &mut steps));
        let work = read + steps as u64;
        let Some(start) = compiled else {
            return (None, work);
        };
        pattern.start = start;
        pattern.cost = pattern
            .nodes
            .iter()
            .map(|node| match node {
                Node::Char(set, _) => 1 + pattern.sets.get(*set).map_or(0, Set::cost),
                _ => 1,
            })
            .fold(0u64, u64::saturating_add);
        (Some(pattern), work)
    }

    /// `chars` 文字の文字列に [`matches`](Self::matches) を呼ぶ手間の上限。1 文字ごとに、すべての状態と、
    /// その文字の集まりの項目を見る（#732）
    pub fn work(&self, chars: u64) -> u64 {
        chars.saturating_add(1).saturating_mul(self.cost)
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
                    let Some(set) = self.sets.get(*set) else {
                        continue;
                    };
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

    /// `ast` を、終わりが `next` へ続く状態にする。初めの状態を返す。展開した回数を `steps` に数え、
    /// [`MAX_STEPS`] を超えたら `None`
    fn compile(&mut self, ast: &Ast, next: usize, steps: &mut usize) -> Option<usize> {
        *steps += 1;
        if *steps > MAX_STEPS {
            return None;
        }
        match ast {
            Ast::Set(set) => self.push(Node::Char(*set, next)),
            Ast::Concat(parts) => {
                let mut at = next;
                for part in parts.iter().rev() {
                    at = self.compile(part, at, steps)?;
                }
                Some(at)
            }
            Ast::Alt(branches) => {
                let mut starts = Vec::with_capacity(branches.len());
                for branch in branches {
                    starts.push(self.compile(branch, next, steps)?);
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
                        let body = self.compile(inner, split, steps)?;
                        if let Some(Node::Split(first, _)) = self.nodes.get_mut(split) {
                            *first = body;
                        }
                        at = split;
                    }
                    Some(max) => {
                        // 任意の分の 1 つを省けば、残りも省いて `next` へ
                        for _ in *min..*max {
                            let body = self.compile(inner, at, steps)?;
                            at = self.push(Node::Split(body, next))?;
                        }
                    }
                }
                for _ in 0..*min {
                    at = self.compile(inner, at, steps)?;
                }
                Some(at)
            }
        }
    }
}

impl Set {
    /// 1 文字を確かめる手間。項目の数と、引く集まりの手間の和
    fn cost(&self) -> u64 {
        let mut cost = 0u64;
        let mut set = Some(self);
        while let Some(this) = set {
            cost = cost.saturating_add(1 + this.items.len() as u64);
            set = this.subtract.as_deref();
        }
        cost
    }

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
    /// 括弧と引き算の入れ子の深さ
    depth: usize,
    /// 読んだ文字の集まり
    sets: Vec<Set>,
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

    /// 文字の集まりを足し、その番号の木を返す
    fn set(&mut self, set: Set) -> Ast {
        self.sets.push(set);
        Ast::Set(self.sets.len() - 1)
    }

    /// 入れ子を 1 段深くして `f` を呼ぶ。[`MAX_DEPTH`] を超えたら `None`
    fn nested<T>(&mut self, f: impl FnOnce(&mut Self) -> Option<T>) -> Option<T> {
        if self.depth >= MAX_DEPTH {
            return None;
        }
        self.depth += 1;
        let out = f(self);
        self.depth -= 1;
        out
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
                let inner = self.nested(Self::alternation)?;
                self.eat(')').then_some(inner)
            }
            '[' => {
                let set = self.class_body()?;
                Some(self.set(set))
            }
            '.' => Some(self.set(single(Item::Class(Class::Any), false))),
            '\\' => {
                let (item, negated) = self.escape()?;
                Some(self.set(single(item, negated)))
            }
            '?' | '*' | '+' | '{' | '}' | ']' | ')' => None,
            c => Some(self.set(single(Item::Range(c, c), false))),
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
                    subtract = Some(Box::new(self.nested(Self::class_body)?));
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
    use super::{MAX_DEPTH, MAX_STEPS, Pattern};

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

    /// 深い入れ子は、スタックを使い切る前に読めない式にする（#732）
    #[test]
    fn rejects_deep_nesting() {
        let depth = 100_000;
        let groups = format!("{}a{}", "(".repeat(depth), ")".repeat(depth));
        assert!(Pattern::new(&groups).is_none());
        let subtract = format!("{}{}", "[a-".repeat(depth), "]".repeat(depth));
        assert!(Pattern::new(&subtract).is_none());
        // 上限までの深さは読む
        let within = format!("{}a{}", "(".repeat(MAX_DEPTH), ")".repeat(MAX_DEPTH));
        assert_eq!(
            Pattern::new(&within).and_then(|p| p.matches("a")),
            Some(true)
        );
    }

    /// 状態を足さない空の式を `{n,m}` で重ねても、展開の上限で止まる（#732）
    #[test]
    fn stops_expanding_empty_repeats() {
        let (pattern, work) = Pattern::read("((){10000}){10000}");
        assert!(pattern.is_none());
        assert!(work <= 18 + MAX_STEPS as u64 + 1, "{work}");
    }

    /// 文字の集まりは、展開しても写さない。照合の手間は、文字の数 + 1 と、状態と集まりの項目の数の積
    #[test]
    fn shares_sets_and_counts_the_work_of_matching() {
        let (pattern, work) = Pattern::read("[a-z0-9_]{2000}");
        let pattern = pattern.unwrap();
        assert_eq!(pattern.sets.len(), 1);
        assert!(work >= 2000, "{work}");
        // 合った状態と、2,000 個の 1 文字の状態（それぞれ 1 と、集まりの手間 4）
        assert_eq!(pattern.work(0), 1 + 2000 * 5);
        assert_eq!(pattern.work(9), 10 * (1 + 2000 * 5));
        assert_eq!(pattern.matches(&"a".repeat(2000)), Some(true));
    }
}
