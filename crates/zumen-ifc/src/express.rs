//! 利用者が渡す EXPRESS（ISO 10303-11）で、IFC をスキーマに照らしてチェックする（決定 0207・0216）。
//!
//! EXPRESS からは、スキーマの名前・実体（上位の型・属性・導く属性・省略できる属性）・型（定義された型・列挙・
//! 選択・集まり）を読む。`WHERE` の規則・`UNIQUE`・大域の `RULE`・関数は読み飛ばして評価せず、その数を返す。
//! zumen は EXPRESS を同梱しない。

use std::collections::{BTreeMap, BTreeSet};

use zumen_base::{Diagnostic, Kind, codes};
use zumen_sdk::{Cx, Error};

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
    redeclared: Vec<(String, Type, bool)>,
    /// 上位の型の属性を、導く属性にしたもの
    derived: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
struct Attribute {
    name: String,
    ty: Type,
    optional: bool,
}

#[derive(Clone, Debug, PartialEq)]
enum Type {
    Integer,
    Real,
    Number,
    Boolean,
    Logical,
    String,
    Binary,
    /// 型か実体の名前（大文字）
    Named(String),
    Aggregate {
        kind: &'static str,
        lower: Option<u64>,
        upper: Option<u64>,
        optional_items: bool,
        of: Box<Type>,
    },
}

#[derive(Clone, Debug, PartialEq)]
enum TypeDef {
    /// 大文字の名前（診断の値）と、元の型
    Defined(String, Type),
    Enumeration(String, Vec<String>),
    Select(String, Vec<String>),
}

/// 実体のインスタンスの属性の 1 つ
#[derive(Clone, Debug)]
struct Slot {
    name: String,
    ty: Type,
    optional: bool,
    derived: bool,
}

impl Express {
    /// EXPRESS を読む。読めないか、宣言のない名前を使っていれば、`express.file.invalid` を返して失敗する
    pub fn read(src: &[u8], cx: &mut Cx) -> Result<Self, Error> {
        cx.check(Kind::InputBytes, src.len() as u64)
            .map_err(|stop| stopped(cx, stop, None))?;
        let tokens = match tokenize(src, cx)? {
            Ok(tokens) => tokens,
            Err(line) => return Err(invalid(cx, line, "")),
        };
        let mut parser = Parser {
            tokens: &tokens,
            at: 0,
        };
        let schema = match parser.schema() {
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

    /// 宣言のない型の名前
    fn unresolved(&self) -> Option<String> {
        let known =
            |name: &String| self.entities.contains_key(name) || self.types.contains_key(name);
        let mut named = Vec::new();
        for entity in self.entities.values() {
            named.extend(entity.supertypes.iter().cloned());
            for attribute in &entity.attributes {
                attribute.ty.names(&mut named);
            }
            for (_, ty, _) in &entity.redeclared {
                ty.names(&mut named);
            }
        }
        for def in self.types.values() {
            match def {
                TypeDef::Defined(_, ty) => ty.names(&mut named),
                TypeDef::Select(_, items) => named.extend(items.iter().cloned()),
                TypeDef::Enumeration(..) => {}
            }
        }
        named.into_iter().find(|name| !known(name))
    }

    /// 上位の型か、定義された型が循環する名前
    fn cyclic(&self, cx: &mut Cx) -> Result<Option<String>, Error> {
        for name in self.entities.keys() {
            let mut seen = BTreeSet::new();
            let mut stack = vec![name.clone()];
            while let Some(next) = stack.pop() {
                cx.check(Kind::Work, 1)
                    .map_err(|stop| stopped(cx, stop, None))?;
                let Some(entity) = self.entities.get(&next) else {
                    continue;
                };
                for supertype in &entity.supertypes {
                    if supertype == name {
                        return Ok(Some(name.clone()));
                    }
                    if seen.insert(supertype.clone()) {
                        stack.push(supertype.clone());
                    }
                }
            }
        }
        for name in self.types.keys() {
            let mut seen = BTreeSet::new();
            let mut next = name;
            while let Some(TypeDef::Defined(_, Type::Named(target))) = self.types.get(next) {
                if !seen.insert(target.clone()) || target == name {
                    return Ok(Some(name.clone()));
                }
                next = target;
            }
        }
        Ok(None)
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
                    ty: attribute.ty.clone(),
                    optional: attribute.optional,
                    derived: false,
                });
            }
            for (attribute, ty, optional) in &entity.redeclared {
                if let Some(slot) = slots.iter_mut().rev().find(|s| &s.name == attribute) {
                    slot.ty = ty.clone();
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

    /// 型の書き方（診断の値）
    fn display(&self, ty: &Type) -> String {
        match ty {
            Type::Integer => "INTEGER".to_owned(),
            Type::Real => "REAL".to_owned(),
            Type::Number => "NUMBER".to_owned(),
            Type::Boolean => "BOOLEAN".to_owned(),
            Type::Logical => "LOGICAL".to_owned(),
            Type::String => "STRING".to_owned(),
            Type::Binary => "BINARY".to_owned(),
            Type::Named(name) => match (self.types.get(name), self.entities.get(name)) {
                (
                    Some(
                        TypeDef::Defined(n, _) | TypeDef::Enumeration(n, _) | TypeDef::Select(n, _),
                    ),
                    _,
                ) => n.clone(),
                (None, Some(entity)) => entity.name.clone(),
                (None, None) => name.clone(),
            },
            Type::Aggregate {
                kind,
                lower,
                upper,
                of,
                ..
            } => {
                let bound = |b: &Option<u64>| b.map_or("?".to_owned(), |b| b.to_string());
                format!(
                    "{kind} [{}:{}] OF {}",
                    bound(lower),
                    bound(upper),
                    self.display(of)
                )
            }
        }
    }
}

impl Type {
    fn names(&self, out: &mut Vec<String>) {
        match self {
            Self::Named(name) => out.push(name.clone()),
            Self::Aggregate { of, .. } => of.names(out),
            _ => {}
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
}

/// 字句に分ける。注釈（`(* … *)` は入れ子もある、`--` から行の終わり）を除く。閉じない注釈か文字列は、行の番号を返す
fn tokenize(src: &[u8], cx: &mut Cx) -> Result<Result<Vec<Token>, u64>, Error> {
    let mut tokens = Vec::new();
    let mut line = 1u64;
    let mut at = 0usize;
    let byte = |i: usize| src.get(i).copied();
    while let Some(c) = byte(at) {
        cx.check(Kind::Work, 1)
            .map_err(|stop| stopped(cx, stop, Some(at..at + 1)))?;
        match c {
            b'\n' => {
                line += 1;
                at += 1;
            }
            c if c.is_ascii_whitespace() => at += 1,
            b'(' if byte(at + 1) == Some(b'*') => {
                let start = line;
                let mut depth = 1u32;
                at += 2;
                while depth > 0 {
                    match (byte(at), byte(at + 1)) {
                        (None, _) => return Ok(Err(start)),
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
                let start = at;
                at += 1;
                loop {
                    match (byte(at), byte(at + 1)) {
                        (None, _) => return Ok(Err(line)),
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
                let text =
                    String::from_utf8_lossy(src.get(start..at).unwrap_or_default()).into_owned();
                tokens.push(Token { text, line });
            }
            c if c.is_ascii_alphanumeric() || c == b'_' => {
                let start = at;
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
                let text = String::from_utf8_lossy(src.get(start..at).unwrap_or_default())
                    .to_ascii_uppercase();
                tokens.push(Token { text, line });
            }
            _ => {
                // 2 文字の記号
                let pair = [c, byte(at + 1).unwrap_or(0)];
                let len = match &pair {
                    b":=" | b"<=" | b">=" | b"<>" | b"<*" | b"||" | b"**" => 2,
                    _ => 1,
                };
                let text =
                    String::from_utf8_lossy(src.get(at..at + len).unwrap_or_default()).into_owned();
                tokens.push(Token { text, line });
                at += len;
            }
        }
    }
    Ok(Ok(tokens))
}

// 構文

struct Parser<'t> {
    tokens: &'t [Token],
    at: usize,
}

/// 読めなかった字句の位置
type Parsed<T> = Result<T, usize>;

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

    /// スキーマ。字句の数は、字句に分けるときに work で数えてある
    fn schema(&mut self) -> Parsed<Express> {
        self.expect("SCHEMA")?;
        let name = self.name()?;
        // 版の識別子（`'{…}'`）は読み飛ばす
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
                    let (name, def, rules) = self.type_decl()?;
                    schema.rules += rules;
                    schema.types.insert(name, def);
                }
                Some("ENTITY") => {
                    let (entity, rules) = self.entity()?;
                    schema.rules += rules;
                    schema
                        .entities
                        .insert(entity.name.to_ascii_uppercase(), entity);
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
    fn type_decl(&mut self) -> Parsed<(String, TypeDef, u64)> {
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
            _ => TypeDef::Defined(display, self.ty()?),
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

    /// 型
    fn ty(&mut self) -> Parsed<Type> {
        let Some(word) = self.next().map(str::to_owned) else {
            return Err(self.at);
        };
        let simple = match word.as_str() {
            "INTEGER" => Some(Type::Integer),
            "REAL" => Some(Type::Real),
            "NUMBER" => Some(Type::Number),
            "BOOLEAN" => Some(Type::Boolean),
            "LOGICAL" => Some(Type::Logical),
            "STRING" => Some(Type::String),
            "BINARY" => Some(Type::Binary),
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
            return Ok(simple);
        }
        let kind = match word.as_str() {
            "LIST" => "LIST",
            "SET" => "SET",
            "BAG" => "BAG",
            "ARRAY" => "ARRAY",
            _ if word.starts_with(|c: char| c.is_ascii_alphabetic()) => {
                return Ok(Type::Named(word));
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
        let of = self.ty()?;
        Ok(Type::Aggregate {
            kind,
            lower,
            upper,
            optional_items,
            of: Box::new(of),
        })
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
    fn entity(&mut self) -> Parsed<(Entity, u64)> {
        let name = self.name()?;
        let mut entity = Entity {
            name: name.clone(),
            is_abstract: false,
            supertypes: Vec::new(),
            attributes: Vec::new(),
            redeclared: Vec::new(),
            derived: Vec::new(),
        };
        // 見出し: ABSTRACT・SUPERTYPE OF (…)・SUBTYPE OF (…)
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
            let ty = self.ty()?;
            self.expect(";")?;
            for (inherited, name) in names {
                if inherited {
                    entity.redeclared.push((name, ty.clone(), optional));
                } else {
                    entity.attributes.push(Attribute {
                        name,
                        ty: ty.clone(),
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
                        let (inherited, name) = self.attribute_name()?;
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
    fn attribute_name(&mut self) -> Parsed<(bool, String)> {
        if self.eat("SELF") {
            self.expect("\\")?;
            self.name()?;
            self.expect(".")?;
            let name = self.name()?;
            if self.eat("RENAMED") {
                self.name()?;
            }
            return Ok((true, name));
        }
        Ok((false, self.name()?))
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
                        ty: a.ty.clone(),
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
                    (false, value) => (!self.value(&slot.ty, value, cx)?)
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
    fn value(&mut self, ty: &Type, value: &Value, cx: &mut Cx) -> Result<bool, Error> {
        cx.check(Kind::Work, 1)
            .map_err(|stop| stopped(cx, stop, None))?;
        Ok(match ty {
            Type::Integer => match value {
                Value::Number(range) => !self
                    .file
                    .bytes(range.clone())
                    .iter()
                    .any(|b| matches!(b, b'.' | b'E' | b'e')),
                _ => false,
            },
            Type::Real | Type::Number => matches!(value, Value::Number(_)),
            Type::Boolean => matches!(self.enumeration(value).as_deref(), Some("T" | "F")),
            Type::Logical => matches!(self.enumeration(value).as_deref(), Some("T" | "F" | "U")),
            Type::String => matches!(value, Value::Text { .. }),
            Type::Binary => matches!(value, Value::Binary(_)),
            Type::Aggregate {
                lower,
                upper,
                optional_items,
                of,
                ..
            } => {
                let Value::List(items) = value else {
                    return Ok(false);
                };
                let count = items.len() as u64;
                if lower.is_some_and(|l| count < l) || upper.is_some_and(|u| count > u) {
                    return Ok(false);
                }
                cx.enter().map_err(|stop| stopped(cx, stop, None))?;
                let mut fits = true;
                for item in items {
                    let ok = match item {
                        Value::Null => *optional_items,
                        item => self.value(of, item, cx).inspect_err(|_| cx.leave())?,
                    };
                    if !ok {
                        fits = false;
                        break;
                    }
                }
                cx.leave();
                fits
            }
            Type::Named(name) => self.named(name, value, cx)?,
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
                let ty = ty.clone();
                cx.enter().map_err(|stop| stopped(cx, stop, None))?;
                let fits = self.value(&ty, value, cx);
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
