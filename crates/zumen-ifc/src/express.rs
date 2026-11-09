//! 利用者が渡す EXPRESS（ISO 10303-11）で、IFC をスキーマに照らしてチェックする（決定 0207・0216）。
//!
//! EXPRESS からは、スキーマの名前・実体（上位の型・属性・導く属性・省略できる属性）・型（定義された型・列挙・
//! 選択・集まり）を読む。`WHERE` の規則・`UNIQUE`・大域の `RULE`・関数は読み飛ばして評価せず、その数を返す。
//! zumen は EXPRESS を同梱しない。
//!
//! 読むときは、EXPRESS のバイトと、循環を調べる手順を work で数え、集まりの型の入れ子を 1 段ごとに depth で数える
//! （決定 0010・0216）。1 つの宣言に並べた属性の名前は、読んだ型を共有し、名前ごとに写さない。

use core::ops::Range;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::sync::Arc;

use zumen_base::{Diagnostic, Kind, codes};
use zumen_sdk::{Cx, Error, Stop};

use crate::{IfcFile, Record, Value, at_bytes, stopped};

/// 読んだ EXPRESS のスキーマ
#[derive(Clone, Debug, PartialEq)]
pub struct Express {
    name: String,
    /// 大文字の名前から実体
    entities: BTreeMap<String, Entity>,
    /// 大文字の名前から型
    types: BTreeMap<String, TypeDef>,
    /// 評価しない規則の数
    rules: u64,
}

#[derive(Clone, Debug, PartialEq)]
struct Entity {
    name: String,
    is_abstract: bool,
    supertypes: Vec<String>,
    /// 自分で宣言した明示の属性
    attributes: Vec<Attribute>,
    /// 上位の型の属性を、型を変えて宣言し直したもの
    redeclared: Vec<(String, Arc<Type>, bool)>,
    /// 上位の型の属性を、導く属性にしたもの
    derived: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
struct Attribute {
    name: String,
    /// 書いたままの名前
    written: String,
    /// 型。同じ宣言に並べた名前の間で共有する（#668）
    ty: Arc<Type>,
    optional: bool,
}

/// 集まりの段を外から順に持つ。写すときも捨てるときも、型の深さに応じてスタックを使わない
#[derive(Clone, Debug, PartialEq)]
struct Type {
    levels: Vec<Level>,
    item: Item,
}

#[derive(Clone, Debug, PartialEq)]
enum Item {
    Integer,
    Real,
    Number,
    Boolean,
    Logical,
    String,
    Binary,
    /// 型か実体の名前（大文字）
    Named(String),
}

#[derive(Clone, Debug, PartialEq)]
enum TypeDef {
    /// 大文字の名前（診断の値）と、元の型
    Defined(String, Type),
    Enumeration(String, Vec<String>),
    Select(String, Vec<String>),
}

/// 実体の明示の属性の名前と型（IDS の照合が使う）
#[cfg(feature = "ids")]
#[derive(Clone, Debug)]
pub(crate) struct Explicit {
    /// 書いたままの名前
    pub(crate) name: String,
    pub(crate) derived: bool,
    /// 宣言の型の名前（大文字）。名前のない型なら `None`
    pub(crate) type_name: Option<String>,
    pub(crate) base: Base,
}

/// 定義された型をたどった、もとの単純な型
#[cfg(feature = "ids")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Base {
    Integer,
    Real,
    Boolean,
    Logical,
    Other,
}

/// 実体のインスタンスの属性の 1 つ
#[derive(Clone, Debug)]
struct Slot {
    name: String,
    /// 書いたままの名前。IDS の照合が使う
    #[cfg_attr(not(feature = "ids"), expect(dead_code))]
    written: String,
    ty: Arc<Type>,
    optional: bool,
    derived: bool,
}

impl Express {
    /// EXPRESS を読む。読めないか、宣言のない名前を使っているか、同じ名前を 2 度宣言していれば、
    /// `express.file.invalid` を返して失敗する。集まりの型の入れ子が予算の depth を超えたら、止める
    pub fn read(src: &[u8], cx: &mut Cx) -> Result<Self, Error> {
        cx.check(Kind::InputBytes, src.len() as u64)
            .map_err(|stop| stopped(cx, stop, None))?;
        let tokens = match tokenize(src, cx)? {
            Ok(tokens) => tokens,
            Err(line) => return Err(invalid(cx, line, "")),
        };
        let mut parser = Parser {
            src,
            tokens: &tokens,
            at: 0,
            stop: None,
        };
        let parsed = parser.schema(cx);
        if let Some((stop, at)) = parser.stop {
            let place = tokens.get(at).map(|t| t.bytes.clone());
            return Err(stopped(cx, stop, place));
        }
        let schema = match parsed {
            Ok(schema) => schema,
            Err(at) => {
                let (line, near) = tokens
                    .get(at)
                    .map_or((0, String::new()), |t| (t.line, t.text.clone()));
                return Err(invalid(cx, line, &near));
            }
        };
        if let Some(name) = schema.unresolved() {
            return Err(invalid(cx, 0, &name));
        }
        if let Some(name) = schema.cyclic(cx)? {
            return Err(invalid(cx, 0, &name));
        }
        Ok(schema)
    }

    /// スキーマの名前
    pub fn name(&self) -> &str {
        &self.name
    }

    /// 評価しない規則（`WHERE`・`UNIQUE`・`RULE`）の数
    pub fn rules(&self) -> u64 {
        self.rules
    }

    /// 実体か型として宣言した名前（大文字）か
    fn declared(&self, name: &str) -> bool {
        self.entities.contains_key(name) || self.types.contains_key(name)
    }

    /// 宣言のない型の名前。見つけた所で返し、名前を集めて写さない。1 つの宣言に並べた属性の名前は型を共有するので、
    /// 続く同じ型は 1 度だけたどる。手間は、入力のバイト数に比例する（#668）
    fn unresolved(&self) -> Option<String> {
        let unknown = |name: &String| (!self.declared(name)).then(|| name.clone());
        for entity in self.entities.values() {
            if let Some(name) = entity.supertypes.iter().find_map(unknown) {
                return Some(name);
            }
            let types = entity
                .attributes
                .iter()
                .map(|attribute| &attribute.ty)
                .chain(entity.redeclared.iter().map(|(_, ty, _)| ty));
            let mut last: Option<&Arc<Type>> = None;
            for ty in types {
                if last.is_some_and(|last| Arc::ptr_eq(last, ty)) {
                    continue;
                }
                last = Some(ty);
                if let Some(name) = ty.item.name().and_then(unknown) {
                    return Some(name);
                }
            }
        }
        for def in self.types.values() {
            let name = match def {
                TypeDef::Defined(_, ty) => ty.item.name().and_then(unknown),
                TypeDef::Select(_, items) => items.iter().find_map(unknown),
                TypeDef::Enumeration(..) => None,
            };
            if name.is_some() {
                return name;
            }
        }
        None
    }

    /// 上位の型か、定義された型が循環する名前
    fn cyclic(&self, cx: &mut Cx) -> Result<Option<String>, Error> {
        if let Some(name) = self.cyclic_entity(cx)? {
            return Ok(Some(name));
        }
        // 定義された型の鎖。たどり終えた型を覚えておき、型ごとに 1 度だけたどる。1 歩ごとに work で数え、
        // 中断の合図も見る
        let mut done = BTreeSet::new();
        for name in self.types.keys() {
            let mut path = BTreeSet::new();
            let mut next = name;
            while !done.contains(next) {
                cx.check(Kind::Work, 1)
                    .map_err(|stop| stopped(cx, stop, None))?;
                if !path.insert(next) {
                    return Ok(Some(name.clone()));
                }
                let Some(TypeDef::Defined(_, ty)) = self.types.get(next) else {
                    break;
                };
                let Some(target) = ty.name() else {
                    break;
                };
                next = target;
            }
            // 1 つずつ足す。`BTreeSet::append` は、範囲が重なると `done` の全体を作り直すので、型の名前の順と
            // たどる順が食い違うと（`A{i} = Z{i}` など）、手間が型の数の 2 乗になる
            done.extend(path);
        }
        Ok(None)
    }

    /// 上位の型をたどると自分に戻る実体のうち、名前の順で最初のもの。
    ///
    /// 強連結成分を Tarjan の方法で求める。実体ごとに 1 度入り、上位の型ごとに 1 度たどる。
    /// work で数える探索の歩数は、実体と上位の型の数に比例する。1 歩ごとに中断の合図も見る
    fn cyclic_entity(&self, cx: &mut Cx) -> Result<Option<String>, Error> {
        // 入った実体の、入った順と、そこからたどれて成分の決まっていない実体のうち、もっとも早く入ったものの順（low-link）
        let mut order: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
        // 入ったが、成分の決まっていない実体
        let mut open = Vec::new();
        let mut on_open = BTreeSet::new();
        let mut cyclic = BTreeSet::new();
        for root in self.entities.keys() {
            if order.contains_key(root.as_str()) {
                continue;
            }
            // たどっている途中の実体と、次にたどる上位の型の位置
            let mut path = vec![(root.as_str(), 0usize)];
            while let Some(&(name, at)) = path.last() {
                cx.check(Kind::Work, 1)
                    .map_err(|stop| stopped(cx, stop, None))?;
                if !order.contains_key(name) {
                    order.insert(name, (order.len(), order.len()));
                    open.push(name);
                    on_open.insert(name);
                }
                let supertypes = self
                    .entities
                    .get(name)
                    .map_or(&[][..], |entity| entity.supertypes.as_slice());
                if let Some(supertype) = supertypes.get(at) {
                    if let Some(top) = path.last_mut() {
                        top.1 += 1;
                    }
                    let supertype = supertype.as_str();
                    match order.get(supertype) {
                        None if self.entities.contains_key(supertype) => {
                            path.push((supertype, 0));
                        }
                        Some(&(index, _)) if on_open.contains(supertype) => {
                            if let Some(entry) = order.get_mut(name) {
                                entry.1 = entry.1.min(index);
                            }
                        }
                        _ => {}
                    }
                    continue;
                }
                path.pop();
                let (index, low) = order.get(name).copied().unwrap_or_default();
                if let Some(entry) = path.last().and_then(|&(parent, _)| order.get_mut(parent)) {
                    entry.1 = entry.1.min(low);
                }
                if low == index {
                    // `name` を根とする成分を閉じる
                    let mut members = Vec::new();
                    while let Some(member) = open.pop() {
                        on_open.remove(member);
                        members.push(member);
                        if member == name {
                            break;
                        }
                    }
                    if members.len() > 1 || supertypes.iter().any(|s| s == name) {
                        cyclic.extend(members);
                    }
                }
            }
        }
        Ok(self
            .entities
            .keys()
            .find(|name| cyclic.contains(name.as_str()))
            .cloned())
    }

    /// `name` の上位の型（自分を含む）
    fn ancestors(&self, name: &str) -> BTreeSet<String> {
        let mut seen = BTreeSet::new();
        let mut stack = vec![name.to_owned()];
        while let Some(next) = stack.pop() {
            if !seen.insert(next.clone()) {
                continue;
            }
            if let Some(entity) = self.entities.get(&next) {
                stack.extend(entity.supertypes.iter().cloned());
            }
        }
        seen
    }

    /// 実体の属性の並び。上位の型の属性から順に、同じ上位の型は 1 度だけ並べる
    fn slots(&self, name: &str) -> Vec<Slot> {
        let mut order = Vec::new();
        let mut seen = BTreeSet::new();
        // 上位の型を先に並べる。深さ優先で、帰りがけに置く
        let mut stack: Vec<(String, bool)> = vec![(name.to_owned(), false)];
        while let Some((next, expanded)) = stack.pop() {
            if expanded {
                order.push(next);
                continue;
            }
            if !seen.insert(next.clone()) {
                continue;
            }
            stack.push((next.clone(), true));
            if let Some(entity) = self.entities.get(&next) {
                for supertype in entity.supertypes.iter().rev() {
                    if !seen.contains(supertype) {
                        stack.push((supertype.clone(), false));
                    }
                }
            }
        }
        let mut slots: Vec<Slot> = Vec::new();
        for entity in order.iter().filter_map(|n| self.entities.get(n)) {
            for attribute in &entity.attributes {
                slots.push(Slot {
                    name: attribute.name.clone(),
                    written: attribute.written.clone(),
                    ty: Arc::clone(&attribute.ty),
                    optional: attribute.optional,
                    derived: false,
                });
            }
            for (attribute, ty, optional) in &entity.redeclared {
                if let Some(slot) = slots.iter_mut().rev().find(|s| &s.name == attribute) {
                    slot.ty = Arc::clone(ty);
                    slot.optional = *optional;
                }
            }
            for attribute in &entity.derived {
                if let Some(slot) = slots.iter_mut().rev().find(|s| &s.name == attribute) {
                    slot.derived = true;
                }
            }
        }
        slots
    }

    /// 実体の明示の属性を、インスタンスに書く順に。導く属性も位置を持つ。知らない実体なら `None`
    #[cfg(feature = "ids")]
    pub(crate) fn explicit(&self, kind: &str) -> Option<Vec<Explicit>> {
        self.entities.get(kind)?;
        Some(
            self.slots(kind)
                .into_iter()
                .map(|slot| {
                    let type_name = slot.ty.name().cloned();
                    Explicit {
                        base: self.base(&slot.ty),
                        name: slot.written,
                        derived: slot.derived,
                        type_name,
                    }
                })
                .collect(),
        )
    }

    /// 定義された型をたどった、もとの単純な型
    #[cfg(feature = "ids")]
    fn base(&self, ty: &Type) -> Base {
        let mut ty = ty;
        // 型の循環は読むときに断っている。深さは念のため
        for _ in 0..64 {
            match (ty.levels.as_slice(), &ty.item) {
                ([], Item::Integer) => return Base::Integer,
                ([], Item::Real | Item::Number) => return Base::Real,
                ([], Item::Boolean) => return Base::Boolean,
                ([], Item::Logical) => return Base::Logical,
                ([], Item::Named(name)) => match self.types.get(name) {
                    Some(TypeDef::Defined(_, inner)) => ty = inner,
                    _ => return Base::Other,
                },
                _ => return Base::Other,
            }
        }
        Base::Other
    }

    /// 選択の型が受ける実体と定義された型（大文字）。入れ子の選択もたどる
    fn select_members(&self, name: &str) -> (BTreeSet<String>, BTreeSet<String>) {
        let mut entities = BTreeSet::new();
        let mut types = BTreeSet::new();
        let mut seen = BTreeSet::new();
        let mut stack = vec![name.to_owned()];
        while let Some(next) = stack.pop() {
            if !seen.insert(next.clone()) {
                continue;
            }
            match self.types.get(&next) {
                Some(TypeDef::Select(_, items)) => stack.extend(items.iter().cloned()),
                Some(_) => {
                    types.insert(next);
                }
                None if self.entities.contains_key(&next) => {
                    entities.insert(next);
                }
                None => {}
            }
        }
        (entities, types)
    }

    /// 型の書き方（診断の値）。集まりの段は、自分を呼ばずに外から順に書く
    fn display(&self, ty: &Type) -> String {
        let mut out = String::new();
        let bound = |b: &Option<u64>| b.map_or("?".to_owned(), |b| b.to_string());
        for Level {
            kind, lower, upper, ..
        } in &ty.levels
        {
            let _ = write!(out, "{kind} [{}:{}] OF ", bound(lower), bound(upper));
        }
        out.push_str(match &ty.item {
            Item::Integer => "INTEGER",
            Item::Real => "REAL",
            Item::Number => "NUMBER",
            Item::Boolean => "BOOLEAN",
            Item::Logical => "LOGICAL",
            Item::String => "STRING",
            Item::Binary => "BINARY",
            Item::Named(name) => match (self.types.get(name), self.entities.get(name)) {
                (
                    Some(
                        TypeDef::Defined(n, _) | TypeDef::Enumeration(n, _) | TypeDef::Select(n, _),
                    ),
                    _,
                ) => n,
                (None, Some(entity)) => &entity.name,
                (None, None) => name,
            },
        });
        out
    }
}

impl Type {
    /// 名前で指定した型だけ、その名前を返す
    fn name(&self) -> Option<&String> {
        if self.levels.is_empty() {
            self.item.name()
        } else {
            None
        }
    }
}

impl Item {
    fn name(&self) -> Option<&String> {
        match self {
            Self::Named(name) => Some(name),
            _ => None,
        }
    }
}

fn invalid(cx: &mut Cx, line: u64, near: &str) -> Error {
    cx.report(
        Diagnostic::new(codes::EXPRESS_FILE_INVALID)
            .with("line", line)
            .with("near", near),
    );
    Error::Invalid
}

// 字句

#[derive(Clone, Debug, PartialEq)]
struct Token {
    /// 名前は大文字にする。文字列は `'` を含めたまま
    text: String,
    line: u64,
    /// 入力の中の位置。書いたままの字句は、入力のこの範囲から読む（字句ごとに写して持たない。#668）
    bytes: Range<usize>,
}

/// 字句に分ける。注釈（`(* … *)` は入れ子もある、`--` から行の終わり）を除く。閉じない注釈か文字列は、行の番号を返す。
/// 読んだバイトは、字句・空白・注釈ごとに、読み終えてからまとめて work で数える（決定 0216）
fn tokenize(src: &[u8], cx: &mut Cx) -> Result<Result<Vec<Token>, u64>, Error> {
    let mut tokens = Vec::new();
    let mut line = 1u64;
    let mut at = 0usize;
    let byte = |i: usize| src.get(i).copied();
    while let Some(c) = byte(at) {
        let start = at;
        // 閉じない注釈か文字列なら、報告する行の番号
        let mut unclosed = None;
        match c {
            b'\n' => {
                line += 1;
                at += 1;
            }
            c if c.is_ascii_whitespace() => at += 1,
            b'(' if byte(at + 1) == Some(b'*') => {
                let first = line;
                let mut depth = 1u32;
                at += 2;
                while depth > 0 {
                    match (byte(at), byte(at + 1)) {
                        (None, _) => {
                            unclosed = Some(first);
                            break;
                        }
                        (Some(b'('), Some(b'*')) => {
                            depth += 1;
                            at += 2;
                        }
                        (Some(b'*'), Some(b')')) => {
                            depth -= 1;
                            at += 2;
                        }
                        (Some(b'\n'), _) => {
                            line += 1;
                            at += 1;
                        }
                        _ => at += 1,
                    }
                }
            }
            b'-' if byte(at + 1) == Some(b'-') => {
                while byte(at).is_some_and(|c| c != b'\n') {
                    at += 1;
                }
            }
            b'\'' => {
                at += 1;
                loop {
                    match (byte(at), byte(at + 1)) {
                        (None, _) => {
                            unclosed = Some(line);
                            break;
                        }
                        (Some(b'\''), Some(b'\'')) => at += 2,
                        (Some(b'\''), _) => {
                            at += 1;
                            break;
                        }
                        (Some(b'\n'), _) => {
                            line += 1;
                            at += 1;
                        }
                        _ => at += 1,
                    }
                }
                tokens.push(Token {
                    text: String::from_utf8_lossy(src.get(start..at).unwrap_or_default())
                        .into_owned(),
                    line,
                    bytes: start..at,
                });
            }
            c if c.is_ascii_alphanumeric() || c == b'_' => {
                while byte(at).is_some_and(|c| c.is_ascii_alphanumeric() || c == b'_') {
                    at += 1;
                }
                // 実数の `1.5E-3`
                while byte(at).is_some_and(|c| c == b'.' || c == b'-' || c == b'+')
                    && src.get(start).is_some_and(u8::is_ascii_digit)
                    && byte(at + 1).is_some_and(|c| c.is_ascii_alphanumeric())
                {
                    at += 1;
                    while byte(at).is_some_and(|c| c.is_ascii_alphanumeric()) {
                        at += 1;
                    }
                }
                tokens.push(Token {
                    text: String::from_utf8_lossy(src.get(start..at).unwrap_or_default())
                        .to_ascii_uppercase(),
                    line,
                    bytes: start..at,
                });
            }
            _ => {
                // 2 文字の記号
                let pair = [c, byte(at + 1).unwrap_or(0)];
                let len = match &pair {
                    b":=" | b"<=" | b">=" | b"<>" | b"<*" | b"||" | b"**" => 2,
                    _ => 1,
                };
                tokens.push(Token {
                    text: String::from_utf8_lossy(src.get(at..at + len).unwrap_or_default())
                        .into_owned(),
                    line,
                    bytes: at..at + len,
                });
                at += len;
            }
        }
        // どの枝も 1 バイト以上を読む
        cx.check(Kind::Work, (at - start) as u64)
            .map_err(|stop| stopped(cx, stop, Some(start..at)))?;
        if let Some(line) = unclosed {
            return Ok(Err(line));
        }
    }
    Ok(Ok(tokens))
}

// 構文

struct Parser<'t> {
    /// 入力。書いたままの名前を読む
    src: &'t [u8],
    tokens: &'t [Token],
    at: usize,
    /// 予算か中断の合図で止めた理由と、そのときの字句の位置。止めたら、読めなかったとして返す
    stop: Option<(Stop, usize)>,
}

/// 読めなかった字句の位置
type Parsed<T> = Result<T, usize>;

/// 集まりの型の 1 段
#[derive(Clone, Debug, PartialEq)]
struct Level {
    kind: &'static str,
    lower: Option<u64>,
    upper: Option<u64>,
    optional_items: bool,
}

/// 型の始まりから読んだもの
enum Head {
    /// 集まりの 1 段。後ろに項目の型が続く
    Level(Level),
    /// 集まりでない型
    Item(Item),
}

impl Parser<'_> {
    fn peek(&self) -> Option<&str> {
        self.tokens.get(self.at).map(|t| t.text.as_str())
    }

    fn next(&mut self) -> Option<&str> {
        let token = self.tokens.get(self.at)?;
        self.at += 1;
        Some(token.text.as_str())
    }

    fn eat(&mut self, text: &str) -> bool {
        if self.peek() == Some(text) {
            self.at += 1;
            true
        } else {
            false
        }
    }

    fn expect(&mut self, text: &str) -> Parsed<()> {
        if self.eat(text) { Ok(()) } else { Err(self.at) }
    }

    /// 名前。予約語でも読む（スキーマの名前と属性の名前に使われうる）
    fn name(&mut self) -> Parsed<String> {
        match self.peek() {
            Some(text) if text.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_') => {
                let text = text.to_owned();
                self.at += 1;
                Ok(text)
            }
            _ => Err(self.at),
        }
    }

    /// `;` まで飛ばす。括弧の中の `;` は飛ばさない
    fn skip_statement(&mut self) -> Parsed<()> {
        let mut depth = 0i64;
        loop {
            match self.next() {
                None => return Err(self.at),
                Some("(" | "[" | "{") => depth += 1,
                Some(")" | "]" | "}") => depth -= 1,
                Some(";") if depth <= 0 => return Ok(()),
                Some(_) => {}
            }
        }
    }

    /// `end` まで飛ばす。`begin` の入れ子を数える
    fn skip_block(&mut self, begin: &str, end: &str) -> Parsed<()> {
        let mut depth = 1u32;
        loop {
            match self.next() {
                None => return Err(self.at),
                Some(text) if text == begin => depth += 1,
                Some(text) if text == end => {
                    depth -= 1;
                    if depth == 0 {
                        return self.expect(";");
                    }
                }
                Some(_) => {}
            }
        }
    }

    /// スキーマ。字句のバイトは、字句に分けるときに work で数えてある。同じ名前の宣言は、どれを使うか決められない
    /// ので断る
    fn schema(&mut self, cx: &mut Cx) -> Parsed<Express> {
        self.expect("SCHEMA")?;
        let name = self.name()?;
        // バージョンの識別子（`'{…}'`）は読み飛ばす
        self.skip_statement()?;
        let mut schema = Express {
            name,
            entities: BTreeMap::new(),
            types: BTreeMap::new(),
            rules: 0,
        };
        loop {
            match self.next() {
                Some("END_SCHEMA") => {
                    self.expect(";")?;
                    return Ok(schema);
                }
                Some("TYPE") => {
                    let at = self.at;
                    let (name, def, rules) = self.type_decl(cx)?;
                    if schema.declared(&name) {
                        return Err(at);
                    }
                    schema.rules += rules;
                    schema.types.insert(name, def);
                }
                Some("ENTITY") => {
                    let at = self.at;
                    let (entity, rules) = self.entity(cx)?;
                    let name = entity.name.to_ascii_uppercase();
                    if schema.declared(&name) {
                        return Err(at);
                    }
                    schema.rules += rules;
                    schema.entities.insert(name, entity);
                }
                Some("FUNCTION") => self.skip_block("FUNCTION", "END_FUNCTION")?,
                Some("PROCEDURE") => self.skip_block("PROCEDURE", "END_PROCEDURE")?,
                Some("RULE") => {
                    schema.rules += 1;
                    self.skip_block("RULE", "END_RULE")?;
                }
                Some("CONSTANT") => self.skip_block("CONSTANT", "END_CONSTANT")?,
                Some("SUBTYPE_CONSTRAINT") => {
                    self.skip_block("SUBTYPE_CONSTRAINT", "END_SUBTYPE_CONSTRAINT")?;
                }
                Some("USE" | "REFERENCE") => self.skip_statement()?,
                _ => return Err(self.at.saturating_sub(1)),
            }
        }
    }

    /// `TYPE 名前 = 元の型 ; [WHERE …] END_TYPE ;`
    fn type_decl(&mut self, cx: &mut Cx) -> Parsed<(String, TypeDef, u64)> {
        let name = self.name()?;
        let display = name.clone();
        self.expect("=")?;
        let def = match self.peek() {
            Some("ENUMERATION") => {
                self.at += 1;
                self.expect("OF")?;
                TypeDef::Enumeration(display, self.names()?)
            }
            Some("SELECT") => {
                self.at += 1;
                TypeDef::Select(display, self.names()?)
            }
            _ => TypeDef::Defined(display, self.ty(cx)?),
        };
        self.expect(";")?;
        let rules = self.rules_until("END_TYPE")?;
        Ok((name, def, rules))
    }

    /// `( a, b, c )`
    fn names(&mut self) -> Parsed<Vec<String>> {
        self.expect("(")?;
        let mut names = vec![self.name()?];
        while self.eat(",") {
            names.push(self.name()?);
        }
        self.expect(")")?;
        Ok(names)
    }

    /// 型。集まりの入れ子は、自分を呼ばずに外から 1 段ずつ読み、1 段ごとに予算の depth で数える（決定 0010）
    fn ty(&mut self, cx: &mut Cx) -> Parsed<Type> {
        let mut levels = Vec::new();
        let item = loop {
            let start = self.at;
            match self.head() {
                Ok(Head::Level(level)) => {
                    if let Err(stop) = cx.enter() {
                        self.stop = Some((stop, start));
                        break Err(start);
                    }
                    levels.push(level);
                }
                Ok(Head::Item(item)) => break Ok(item),
                Err(at) => break Err(at),
            }
        };
        // 型を読み終えたら、入った段をすべて出る
        cx.budget.leave(levels.len() as u64);
        Ok(Type {
            levels,
            item: item?,
        })
    }

    /// 型の始まり。集まりなら、`OF` の後ろまでの 1 段だけを読む
    fn head(&mut self) -> Parsed<Head> {
        let Some(word) = self.next().map(str::to_owned) else {
            return Err(self.at);
        };
        let simple = match word.as_str() {
            "INTEGER" => Some(Item::Integer),
            "REAL" => Some(Item::Real),
            "NUMBER" => Some(Item::Number),
            "BOOLEAN" => Some(Item::Boolean),
            "LOGICAL" => Some(Item::Logical),
            "STRING" => Some(Item::String),
            "BINARY" => Some(Item::Binary),
            _ => None,
        };
        if let Some(simple) = simple {
            // 精度・幅（`(10)`・`(22) FIXED`）は読み飛ばす
            if self.eat("(") {
                while !self.eat(")") {
                    let at = self.at;
                    self.next().ok_or(at)?;
                }
                self.eat("FIXED");
            }
            return Ok(Head::Item(simple));
        }
        let kind = match word.as_str() {
            "LIST" => "LIST",
            "SET" => "SET",
            "BAG" => "BAG",
            "ARRAY" => "ARRAY",
            _ if word.starts_with(|c: char| c.is_ascii_alphabetic()) => {
                return Ok(Head::Item(Item::Named(word)));
            }
            _ => return Err(self.at.saturating_sub(1)),
        };
        let (mut lower, mut upper) = (None, None);
        if self.eat("[") {
            lower = self.bound()?;
            self.expect(":")?;
            upper = self.bound()?;
            self.expect("]")?;
        }
        self.expect("OF")?;
        let optional_items = self.eat("OPTIONAL");
        self.eat("UNIQUE");
        Ok(Head::Level(Level {
            kind,
            lower,
            upper,
            optional_items,
        }))
    }

    /// 集まりの上下の限り。数でなければ（`?` や式）、限りなしにする
    fn bound(&mut self) -> Parsed<Option<u64>> {
        let start = self.at;
        let mut depth = 0i64;
        while let Some(text) = self.peek() {
            match text {
                "(" | "[" => depth += 1,
                ")" | "]" if depth == 0 => break,
                ")" | "]" => depth -= 1,
                ":" if depth == 0 => break,
                _ => {}
            }
            self.at += 1;
        }
        let tokens = self.tokens.get(start..self.at).ok_or(self.at)?;
        Ok(match tokens {
            [only] => only.text.parse().ok(),
            _ => None,
        })
    }

    /// `ENTITY` の宣言
    fn entity(&mut self, cx: &mut Cx) -> Parsed<(Entity, u64)> {
        let name = self.name()?;
        let mut entity = Entity {
            name: name.clone(),
            is_abstract: false,
            supertypes: Vec::new(),
            attributes: Vec::new(),
            redeclared: Vec::new(),
            derived: Vec::new(),
        };
        // ヘッダー: ABSTRACT・SUPERTYPE OF (…)・SUBTYPE OF (…)
        loop {
            match self.next() {
                Some(";") => break,
                Some("ABSTRACT") => entity.is_abstract = true,
                Some("SUPERTYPE") => {
                    self.expect("OF")?;
                    self.expect("(")?;
                    let mut depth = 1u32;
                    while depth > 0 {
                        match self.next() {
                            None => return Err(self.at),
                            Some("(") => depth += 1,
                            Some(")") => depth -= 1,
                            Some(_) => {}
                        }
                    }
                }
                Some("SUBTYPE") => {
                    self.expect("OF")?;
                    entity.supertypes = self.names()?;
                }
                _ => return Err(self.at.saturating_sub(1)),
            }
        }
        // 明示の属性
        loop {
            match self.peek() {
                Some("DERIVE" | "INVERSE" | "UNIQUE" | "WHERE" | "END_ENTITY") | None => break,
                _ => {}
            }
            let mut names = vec![self.attribute_name()?];
            while self.eat(",") {
                names.push(self.attribute_name()?);
            }
            self.expect(":")?;
            let optional = self.eat("OPTIONAL");
            // 並べた名前は、読んだ型を共有する。名前ごとに写すと、メモリが名前の数と型の大きさの積で増える（#668）
            let ty = Arc::new(self.ty(cx)?);
            self.expect(";")?;
            for (inherited, name, written) in names {
                if inherited {
                    entity.redeclared.push((name, Arc::clone(&ty), optional));
                } else {
                    entity.attributes.push(Attribute {
                        name,
                        written,
                        ty: Arc::clone(&ty),
                        optional,
                    });
                }
            }
        }
        let mut rules = 0;
        loop {
            match self.next() {
                Some("END_ENTITY") => {
                    self.expect(";")?;
                    return Ok((entity, rules));
                }
                Some("DERIVE") => {
                    while !matches!(
                        self.peek(),
                        Some("INVERSE" | "UNIQUE" | "WHERE" | "END_ENTITY") | None
                    ) {
                        let (inherited, name, _) = self.attribute_name()?;
                        if inherited {
                            entity.derived.push(name);
                        }
                        self.skip_statement()?;
                    }
                }
                Some("INVERSE") => {
                    while !matches!(self.peek(), Some("UNIQUE" | "WHERE" | "END_ENTITY") | None) {
                        self.skip_statement()?;
                    }
                }
                Some("UNIQUE" | "WHERE") => {
                    while !matches!(self.peek(), Some("UNIQUE" | "WHERE" | "END_ENTITY") | None) {
                        rules += 1;
                        self.skip_statement()?;
                    }
                }
                _ => return Err(self.at.saturating_sub(1)),
            }
        }
    }

    /// 属性の名前。`SELF\上位.名前` なら、上位の属性を宣言し直したもの。`RENAMED` の名前は読み飛ばす
    fn attribute_name(&mut self) -> Parsed<(bool, String, String)> {
        let inherited = self.eat("SELF");
        if inherited {
            self.expect("\\")?;
            self.name()?;
            self.expect(".")?;
        }
        let name = self.name()?;
        // 書いたままの名前は、字句の位置から入力を読む
        let written = self
            .tokens
            .get(self.at.saturating_sub(1))
            .and_then(|t| self.src.get(t.bytes.clone()))
            .map_or_else(|| name.clone(), |b| String::from_utf8_lossy(b).into_owned());
        if inherited && self.eat("RENAMED") {
            self.name()?;
        }
        Ok((inherited, name, written))
    }

    /// `END_TYPE` などまでの `WHERE` の規則の数
    fn rules_until(&mut self, end: &str) -> Parsed<u64> {
        let mut rules = 0;
        if self.eat("WHERE") {
            while self.peek() != Some(end) {
                if self.peek().is_none() {
                    return Err(self.at);
                }
                rules += 1;
                self.skip_statement()?;
            }
        }
        self.expect(end)?;
        self.expect(";")?;
        Ok(rules)
    }
}

// チェック

/// IFC のインスタンスを、スキーマに照らしてチェックする。指摘は書いた順に返す（文脈には足さない）。
///
/// `FILE_SCHEMA` の名前がスキーマの名前と違えば、`express.schema.mixed` を文脈に足して、そのままチェックする。
/// 評価しない規則があれば、`express.rule.unsupported` を文脈に足す
pub fn check(file: &IfcFile, schema: &Express, cx: &mut Cx) -> Result<Vec<Diagnostic>, Error> {
    let file_schema = String::from_utf8_lossy(file.schema()).into_owned();
    if !file_schema.eq_ignore_ascii_case(&schema.name) {
        cx.report(
            Diagnostic::new(codes::EXPRESS_SCHEMA_MIXED)
                .with("schema", schema.name.as_str())
                .with("file_schema", file_schema),
        );
    }
    if schema.rules > 0 {
        cx.report(Diagnostic::new(codes::EXPRESS_RULE_UNSUPPORTED).with("rules", schema.rules));
    }
    let mut checker = Checker {
        file,
        schema,
        slots: BTreeMap::new(),
        ancestors: BTreeMap::new(),
        selects: BTreeMap::new(),
        findings: Vec::new(),
    };
    for id in file.ids() {
        let Some(record) = file.instance(id) else {
            continue;
        };
        cx.check(Kind::Work, 1)
            .map_err(|stop| stopped(cx, stop, Some(record.span())))?;
        checker.instance(record, cx)?;
    }
    Ok(checker.findings)
}

struct Checker<'a> {
    file: &'a IfcFile,
    schema: &'a Express,
    slots: BTreeMap<String, Vec<Slot>>,
    ancestors: BTreeMap<String, BTreeSet<String>>,
    selects: BTreeMap<String, (BTreeSet<String>, BTreeSet<String>)>,
    findings: Vec<Diagnostic>,
}

impl Checker<'_> {
    fn instance(&mut self, record: &Record, cx: &mut Cx) -> Result<(), Error> {
        let complex = record.parts().len() > 1;
        // 複合のインスタンスで、ほかの部分が導く属性にしたもの
        let derived_elsewhere: BTreeSet<String> = if complex {
            record
                .parts()
                .iter()
                .filter_map(|part| {
                    let name =
                        String::from_utf8_lossy(self.file.bytes(part.name())).to_ascii_uppercase();
                    self.schema.entities.get(&name)
                })
                .flat_map(|entity| entity.derived.iter().cloned())
                .collect()
        } else {
            BTreeSet::new()
        };
        for part in record.parts() {
            let kind = String::from_utf8_lossy(self.file.bytes(part.name())).to_ascii_uppercase();
            let Some(entity) = self.schema.entities.get(&kind) else {
                self.find(
                    Diagnostic::new(codes::EXPRESS_ENTITY_MISSING).with("kind", kind.as_str()),
                    record,
                );
                continue;
            };
            if entity.is_abstract && !complex {
                self.find(
                    Diagnostic::new(codes::EXPRESS_ABSTRACT_INVALID).with("kind", kind.as_str()),
                    record,
                );
            }
            // 複合のインスタンスの部分は、その実体が宣言した属性だけを持つ
            let slots: Vec<Slot> = if complex {
                entity
                    .attributes
                    .iter()
                    .map(|a| Slot {
                        name: a.name.clone(),
                        written: a.written.clone(),
                        ty: Arc::clone(&a.ty),
                        optional: a.optional,
                        derived: derived_elsewhere.contains(&a.name),
                    })
                    .collect()
            } else {
                self.slots
                    .entry(kind.clone())
                    .or_insert_with(|| self.schema.slots(&kind))
                    .clone()
            };
            let args = part.args();
            if args.len() != slots.len() {
                self.find(
                    Diagnostic::new(codes::EXPRESS_ATTRIBUTE_COUNT_INVALID)
                        .with("kind", kind.as_str())
                        .with("expected", slots.len() as u64)
                        .with("found", args.len() as u64),
                    record,
                );
                continue;
            }
            for (position, (slot, value)) in slots.iter().zip(args).enumerate() {
                let position = position as u64 + 1;
                let finding = match (slot.derived, value) {
                    (true, Value::Derived) => None,
                    (true, _) => Some(("*".to_owned(), false)),
                    (false, Value::Derived) => Some((self.schema.display(&slot.ty), false)),
                    (false, Value::Null) if slot.optional => None,
                    (false, Value::Null) => Some((self.schema.display(&slot.ty), true)),
                    (false, value) => (!self.value(&slot.ty.levels, &slot.ty.item, value, cx)?)
                        .then(|| (self.schema.display(&slot.ty), false)),
                };
                if let Some((expected, missing)) = finding {
                    let code = if missing {
                        codes::EXPRESS_VALUE_MISSING
                    } else {
                        codes::EXPRESS_VALUE_INVALID
                    };
                    let mut diagnostic = Diagnostic::new(code)
                        .with("kind", kind.as_str())
                        .with("position", position)
                        .with("attribute", slot.name.as_str());
                    if !missing {
                        diagnostic = diagnostic.with("expected", expected);
                    }
                    self.find(diagnostic, record);
                }
            }
        }
        Ok(())
    }

    fn find(&mut self, diagnostic: Diagnostic, record: &Record) {
        let place = at_bytes(record.span());
        let place = match record.id() {
            Some(id) => place.with_id(format!("#{id}")),
            None => place,
        };
        self.findings.push(diagnostic.at(place));
    }

    /// 値が型に合うか。集まりの入れ子は `depth` で数える
    fn value(
        &mut self,
        levels: &[Level],
        item_type: &Item,
        value: &Value,
        cx: &mut Cx,
    ) -> Result<bool, Error> {
        cx.check(Kind::Work, 1)
            .map_err(|stop| stopped(cx, stop, None))?;
        if let Some((level, inner)) = levels.split_first() {
            let Value::List(items) = value else {
                return Ok(false);
            };
            let count = items.len() as u64;
            if level.lower.is_some_and(|l| count < l) || level.upper.is_some_and(|u| count > u) {
                return Ok(false);
            }
            cx.enter().map_err(|stop| stopped(cx, stop, None))?;
            let mut fits = true;
            for value in items {
                fits = match value {
                    Value::Null => level.optional_items,
                    value => self
                        .value(inner, item_type, value, cx)
                        .inspect_err(|_| cx.leave())?,
                };
                if !fits {
                    break;
                }
            }
            cx.leave();
            return Ok(fits);
        }
        Ok(match item_type {
            Item::Integer => match value {
                Value::Number(range) => !self
                    .file
                    .bytes(range.clone())
                    .iter()
                    .any(|b| matches!(b, b'.' | b'E' | b'e')),
                _ => false,
            },
            Item::Real | Item::Number => matches!(value, Value::Number(_)),
            Item::Boolean => matches!(self.enumeration(value).as_deref(), Some("T" | "F")),
            Item::Logical => matches!(self.enumeration(value).as_deref(), Some("T" | "F" | "U")),
            Item::String => matches!(value, Value::Text { .. }),
            Item::Binary => matches!(value, Value::Binary(_)),
            Item::Named(name) => self.named(name, value, cx)?,
        })
    }

    fn named(&mut self, name: &str, value: &Value, cx: &mut Cx) -> Result<bool, Error> {
        if self.schema.entities.contains_key(name) {
            return Ok(match value {
                Value::Ref(id) => self.refers_to(*id, |ancestors| ancestors.contains(name)),
                _ => false,
            });
        }
        match self.schema.types.get(name) {
            Some(TypeDef::Defined(_, ty)) => {
                cx.enter().map_err(|stop| stopped(cx, stop, None))?;
                let fits = self.value(&ty.levels, &ty.item, value, cx);
                cx.leave();
                fits
            }
            Some(TypeDef::Enumeration(_, items)) => Ok(self
                .enumeration(value)
                .is_some_and(|v| items.iter().any(|item| item.eq_ignore_ascii_case(&v)))),
            Some(TypeDef::Select(..)) => {
                let (entities, types) = self
                    .selects
                    .entry(name.to_owned())
                    .or_insert_with(|| self.schema.select_members(name))
                    .clone();
                match value {
                    Value::Ref(id) => {
                        Ok(self.refers_to(*id, |ancestors| !ancestors.is_disjoint(&entities)))
                    }
                    Value::Typed(part) => {
                        let type_name = String::from_utf8_lossy(self.file.bytes(part.name()))
                            .to_ascii_uppercase();
                        if !types.contains(&type_name) {
                            return Ok(false);
                        }
                        let [inner] = part.args() else {
                            return Ok(false);
                        };
                        cx.enter().map_err(|stop| stopped(cx, stop, None))?;
                        let fits = self.named(&type_name, inner, cx);
                        cx.leave();
                        fits
                    }
                    _ => Ok(false),
                }
            }
            None => Ok(false),
        }
    }

    /// 参照の先のインスタンスの、どれかの部分の上位の型が `fits` を満たすか。先がなければ、L1 で診断してあるので満たすとする
    fn refers_to(&mut self, id: u64, fits: impl Fn(&BTreeSet<String>) -> bool) -> bool {
        let Some(target) = self.file.instance(id) else {
            return true;
        };
        let kinds: Vec<String> = target
            .parts()
            .iter()
            .map(|part| String::from_utf8_lossy(self.file.bytes(part.name())).to_ascii_uppercase())
            .collect();
        let mut all = BTreeSet::new();
        for kind in kinds {
            let ancestors = self
                .ancestors
                .entry(kind.clone())
                .or_insert_with(|| self.schema.ancestors(&kind));
            all.extend(ancestors.iter().cloned());
        }
        fits(&all)
    }

    fn enumeration(&self, value: &Value) -> Option<String> {
        match value {
            Value::Enum(range) => {
                Some(String::from_utf8_lossy(self.file.bytes(range.clone())).to_ascii_uppercase())
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use zumen_base::{Budget, CancelToken, Diagnostics, Limits};

    use super::*;

    fn read(src: &str) -> Express {
        let mut budget = Budget::new(Limits::default());
        let mut diag = Diagnostics::new();
        let cancel = CancelToken::new();
        Express::read(
            src.as_bytes(),
            &mut Cx::new(&mut budget, &mut diag, &cancel),
        )
        .unwrap_or_else(|e| panic!("{e:?} {diag:?}"))
    }

    /// 1 つの宣言に並べた属性の名前は、宣言し直したものも含めて、読んだ型を 1 つ共有する。名前ごとに写すと、
    /// メモリが名前の数と型の大きさの積で増える。別の宣言とは、同じ書き方でも共有しない（#668）
    #[test]
    fn names_of_one_declaration_share_the_type() {
        let schema = read(
            "SCHEMA S;\nENTITY P;\n  A : INTEGER;\nEND_ENTITY;\nENTITY E SUBTYPE OF (P);\n  \
             B, SELF\\P.A, C : LIST [1:?] OF INTEGER;\n  D : LIST [1:?] OF INTEGER;\nEND_ENTITY;\n\
             END_SCHEMA;\n",
        );
        let entity = &schema.entities["E"];
        let names: Vec<_> = entity.attributes.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, ["B", "C", "D"]);
        let (b, c, d) = (
            &entity.attributes[0].ty,
            &entity.attributes[1].ty,
            &entity.attributes[2].ty,
        );
        let a = &entity.redeclared[0].1;
        assert!(Arc::ptr_eq(b, a) && Arc::ptr_eq(b, c));
        assert_eq!(Arc::strong_count(b), 3);
        assert!(!Arc::ptr_eq(b, d));
        assert_eq!(b, d);
    }
}
