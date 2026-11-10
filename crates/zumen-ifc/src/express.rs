//! 利用者が渡す EXPRESS（ISO 10303-11）で、IFC をスキーマに照らしてチェックする（決定 0207・0216）。
//!
//! EXPRESS からは、スキーマの名前・実体（上位の型・属性・導く属性・省略できる属性）・型（定義された型・列挙・
//! 選択・集まり）を読む。`WHERE` の規則・`UNIQUE`・大域の `RULE`・関数は読み飛ばして評価せず、その数を返す。
//! zumen は EXPRESS を同梱しない。
//!
//! 読むときは、EXPRESS のバイトと、循環を調べる手順を work で数え、集まりの型の入れ子を 1 段ごとに depth で数える
//! （決定 0010・0216）。1 つの宣言に並べた属性の名前は、読んだ型を共有し、名前ごとに写さない。型・実体の名前には、
//! 初めて読んだ順に番号を振り、型・上位の型・選択の項目は番号で指す。チェックでは、値ごとに名前を引き直さない（#672）。
//!
//! チェックは、インスタンスごと・値ごとに work で数える。実体の属性の数と並び、上位の型と選択の判定は、実体か組ごとに
//! 1 度だけ作って覚え、作る手順を work で数える。上位の型か選択の項目をすべてたどるときは、すでにたどった名前を
//! 飛ばすときも、並びの名前ごとに数える。インスタンスと値ごとには写さない（#672）。

use core::ops::Range;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::rc::Rc;
use std::sync::Arc;

use zumen_base::{Diagnostic, Kind, codes};
use zumen_sdk::{Cx, Error, Stop};

use crate::{IfcFile, Part, Record, Value, at_bytes, stopped};

/// 読んだ EXPRESS のスキーマ
#[derive(Clone, Debug, PartialEq)]
pub struct Express {
    name: String,
    /// 大文字の名前から、宣言の番号。名前の順に並ぶ
    symbols: BTreeMap<String, usize>,
    /// 番号ごとの、大文字の名前と宣言。番号は、名前を初めて読んだ順に振る。読めたスキーマは、すべての名前を宣言する
    decls: Vec<(String, Option<Decl>)>,
    /// 評価しない規則の数
    rules: u64,
}

#[derive(Clone, Debug, PartialEq)]
enum Decl {
    Entity(Entity),
    Type(TypeDef),
}

#[derive(Clone, Debug, PartialEq)]
struct Entity {
    is_abstract: bool,
    /// 上位の型の番号
    supertypes: Vec<usize>,
    /// 自分で宣言した明示の属性
    attributes: Vec<Attribute>,
    /// 上位の型の属性を、型を変えて宣言し直したもの。属性の名前は番号で持つ
    redeclared: Vec<(usize, Arc<Type>, bool)>,
    /// 上位の型の属性を、導く属性にしたもの。属性の名前の番号
    derived: Vec<usize>,
}

#[derive(Clone, Debug, PartialEq)]
struct Attribute {
    /// 大文字の名前（診断の値）
    name: String,
    /// 大文字の名前の番号。宣言し直した属性と導く属性を、名前の長さによらない手間で引く（#672）
    key: usize,
    /// 書いたままの名前。IDS の照合が使う
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
    /// 型か実体の番号
    Named(usize),
}

#[derive(Clone, Debug, PartialEq)]
enum TypeDef {
    /// 元の型
    Defined(Type),
    /// 列挙の項目（大文字）。値ごとに項目を前から比べないように、集合で引く（#672）
    Enumeration(BTreeSet<String>),
    /// 選択の項目の番号
    Select(Vec<usize>),
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

/// 実体のインスタンスの属性の 1 つ。属性と型は、スキーマのものを指して写さない（#672）
#[derive(Clone, Copy, Debug)]
pub(crate) struct Slot<'a> {
    attribute: &'a Attribute,
    ty: &'a Type,
    optional: bool,
    derived: bool,
}

#[cfg(feature = "ids")]
impl<'a> Slot<'a> {
    /// 書いたままの属性の名前
    pub(crate) fn written(&self) -> &'a str {
        &self.attribute.written
    }

    /// 導く属性か
    pub(crate) fn is_derived(&self) -> bool {
        self.derived
    }
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
            symbols: BTreeMap::new(),
            decls: Vec::new(),
            keys: BTreeMap::new(),
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

    /// 番号の宣言。宣言のない名前なら `None`
    fn decl(&self, at: usize) -> Option<&Decl> {
        self.decls.get(at)?.1.as_ref()
    }

    /// 番号の実体。実体でなければ `None`
    fn entity(&self, at: usize) -> Option<&Entity> {
        match self.decl(at)? {
            Decl::Entity(entity) => Some(entity),
            Decl::Type(_) => None,
        }
    }

    /// 番号の型。型でなければ `None`
    fn type_def(&self, at: usize) -> Option<&TypeDef> {
        match self.decl(at)? {
            Decl::Type(def) => Some(def),
            Decl::Entity(_) => None,
        }
    }

    /// 番号の大文字の名前
    fn symbol(&self, at: usize) -> &str {
        self.decls.get(at).map_or("", |(name, _)| name)
    }

    /// 大文字の名前の、宣言の番号。名前の長さと宣言の数の対数に比例する手間で引く
    fn lookup(&self, name: &str) -> Option<usize> {
        self.symbols.get(name).copied()
    }

    /// 大文字の名前の実体の番号
    pub(crate) fn entity_named(&self, name: &str) -> Option<usize> {
        self.lookup(name).filter(|&at| self.entity(at).is_some())
    }

    /// 宣言のない型の名前。見つけた所で返し、名前を集めて写さない。実体の名前の順に、上位の型・属性の型・宣言し直した
    /// 属性の型の順で見て、実体の後に型を見る。1 つの宣言に並べた属性の名前は型を共有するので、続く同じ型は 1 度だけ
    /// たどる。手間は、入力のバイト数に比例する（#668）
    fn unresolved(&self) -> Option<String> {
        let unknown = |at: usize| match self.decls.get(at) {
            Some((_, Some(_))) => None,
            Some((name, None)) => Some(name.clone()),
            None => Some(String::new()),
        };
        for &at in self.symbols.values() {
            let Some(entity) = self.entity(at) else {
                continue;
            };
            if let Some(name) = entity.supertypes.iter().find_map(|&s| unknown(s)) {
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
        for &at in self.symbols.values() {
            let name = match self.type_def(at) {
                Some(TypeDef::Defined(ty)) => ty.item.name().and_then(unknown),
                Some(TypeDef::Select(items)) => items.iter().find_map(|&s| unknown(s)),
                Some(TypeDef::Enumeration(_)) | None => None,
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
        for (name, &start) in &self.symbols {
            if self.type_def(start).is_none() {
                continue;
            }
            let mut path = BTreeSet::new();
            let mut next = start;
            while !done.contains(&next) {
                cx.check(Kind::Work, 1)
                    .map_err(|stop| stopped(cx, stop, None))?;
                if !path.insert(next) {
                    return Ok(Some(name.clone()));
                }
                let Some(TypeDef::Defined(ty)) = self.type_def(next) else {
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
        let mut order: BTreeMap<usize, (usize, usize)> = BTreeMap::new();
        // 入ったが、成分の決まっていない実体
        let mut open = Vec::new();
        let mut on_open = BTreeSet::new();
        let mut cyclic = BTreeSet::new();
        for &root in self.symbols.values() {
            if self.entity(root).is_none() || order.contains_key(&root) {
                continue;
            }
            // たどっている途中の実体と、次にたどる上位の型の位置
            let mut path = vec![(root, 0usize)];
            while let Some(&(at, next)) = path.last() {
                cx.check(Kind::Work, 1)
                    .map_err(|stop| stopped(cx, stop, None))?;
                if !order.contains_key(&at) {
                    order.insert(at, (order.len(), order.len()));
                    open.push(at);
                    on_open.insert(at);
                }
                let supertypes = self
                    .entity(at)
                    .map_or(&[][..], |entity| entity.supertypes.as_slice());
                if let Some(&supertype) = supertypes.get(next) {
                    if let Some(top) = path.last_mut() {
                        top.1 += 1;
                    }
                    match order.get(&supertype) {
                        None if self.entity(supertype).is_some() => {
                            path.push((supertype, 0));
                        }
                        Some(&(index, _)) if on_open.contains(&supertype) => {
                            if let Some(entry) = order.get_mut(&at) {
                                entry.1 = entry.1.min(index);
                            }
                        }
                        _ => {}
                    }
                    continue;
                }
                path.pop();
                let (index, low) = order.get(&at).copied().unwrap_or_default();
                if let Some(entry) = path.last().and_then(|&(parent, _)| order.get_mut(&parent)) {
                    entry.1 = entry.1.min(low);
                }
                if low == index {
                    // `at` を根とする成分を閉じる
                    let mut members = Vec::new();
                    while let Some(member) = open.pop() {
                        on_open.remove(&member);
                        members.push(member);
                        if member == at {
                            break;
                        }
                    }
                    if members.len() > 1 || supertypes.contains(&at) {
                        cyclic.extend(members);
                    }
                }
            }
        }
        Ok(self
            .symbols
            .iter()
            .find(|(_, at)| cyclic.contains(*at))
            .map(|(name, _)| name.clone()))
    }

    /// 実体の属性の並び（単独のインスタンスに書く順）。上位の型の属性から順に、同じ上位の型は 1 度だけ並べる。
    /// 宣言し直した属性と導く属性は、それまでに並べた同じ名前の属性のうち、いちばん後ろのものを変える。
    ///
    /// 上位の型をたどる 1 歩・上位の型の並びの名前・属性・宣言し直した属性・導く属性ごとに、`tick` で 1 つ数える。
    /// 並びの名前は、すでにたどった名前を飛ばすときも数える（#672）
    fn slots<F>(&self, entity: usize, tick: &mut F) -> Result<Vec<Slot<'_>>, Stop>
    where
        F: FnMut(u64) -> Result<(), Stop>,
    {
        let mut order = Vec::new();
        let mut seen = BTreeSet::new();
        // 上位の型を先に並べる。深さ優先で、帰りがけに置く
        let mut stack = vec![(entity, false)];
        while let Some((next, expanded)) = stack.pop() {
            tick(1)?;
            if expanded {
                order.push(next);
                continue;
            }
            if !seen.insert(next) {
                continue;
            }
            stack.push((next, true));
            if let Some(entity) = self.entity(next) {
                tick(entity.supertypes.len() as u64)?;
                for &supertype in entity.supertypes.iter().rev() {
                    if !seen.contains(&supertype) {
                        stack.push((supertype, false));
                    }
                }
            }
        }
        let mut slots: Vec<Slot<'_>> = Vec::new();
        // 属性の名前の番号ごとの、いちばん後ろの位置。後ろから探さずに引く
        let mut last: BTreeMap<usize, usize> = BTreeMap::new();
        for entity in order.iter().filter_map(|&at| self.entity(at)) {
            for attribute in &entity.attributes {
                tick(1)?;
                last.insert(attribute.key, slots.len());
                slots.push(Slot {
                    attribute,
                    ty: &attribute.ty,
                    optional: attribute.optional,
                    derived: false,
                });
            }
            for (key, ty, optional) in &entity.redeclared {
                tick(1)?;
                if let Some(slot) = last.get(key).and_then(|&at| slots.get_mut(at)) {
                    slot.ty = ty;
                    slot.optional = *optional;
                }
            }
            for key in &entity.derived {
                tick(1)?;
                if let Some(slot) = last.get(key).and_then(|&at| slots.get_mut(at)) {
                    slot.derived = true;
                }
            }
        }
        Ok(slots)
    }

    /// 宣言の型の名前（大文字）。名前のない型なら `None`
    #[cfg(feature = "ids")]
    pub(crate) fn type_name(&self, slot: &Slot) -> Option<&str> {
        slot.ty.name().map(|at| self.symbol(at))
    }

    /// 定義された型をたどった、もとの単純な型
    #[cfg(feature = "ids")]
    pub(crate) fn base(&self, slot: &Slot) -> Base {
        let mut ty = slot.ty;
        // 型の循環は読むときに断っている。深さは念のため
        for _ in 0..64 {
            match (ty.levels.as_slice(), &ty.item) {
                ([], Item::Integer) => return Base::Integer,
                ([], Item::Real | Item::Number) => return Base::Real,
                ([], Item::Boolean) => return Base::Boolean,
                ([], Item::Logical) => return Base::Logical,
                ([], Item::Named(at)) => match self.type_def(*at) {
                    Some(TypeDef::Defined(inner)) => ty = inner,
                    _ => return Base::Other,
                },
                _ => return Base::Other,
            }
        }
        Base::Other
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
            Item::Named(at) => self.symbol(*at),
        });
        out
    }
}

impl Type {
    /// 名前で指定した型だけ、その番号を返す
    fn name(&self) -> Option<usize> {
        if self.levels.is_empty() {
            self.item.name()
        } else {
            None
        }
    }
}

impl Item {
    fn name(&self) -> Option<usize> {
        match self {
            Self::Named(at) => Some(*at),
            _ => None,
        }
    }
}

/// 実体ごとの、単独のインスタンスの属性の数と並び。初めて使うときに作って覚え、インスタンスごとに作り直さない（#672）。
///
/// 覚える並びは、属性の数の合うインスタンスがあった実体のものだけにする。覚えるメモリは、そのインスタンスの値の数を
/// 超えない
#[derive(Clone, Debug)]
pub(crate) struct Layouts<'a> {
    schema: &'a Express,
    /// 実体の番号から、属性の数
    counts: BTreeMap<usize, usize>,
    /// 実体の番号から、属性の並び
    slots: BTreeMap<usize, Rc<[Slot<'a>]>>,
}

impl<'a> Layouts<'a> {
    pub(crate) fn new(schema: &'a Express) -> Self {
        Self {
            schema,
            counts: BTreeMap::new(),
            slots: BTreeMap::new(),
        }
    }

    /// 並びを作るスキーマ
    #[cfg(feature = "ids")]
    pub(crate) fn schema(&self) -> &'a Express {
        self.schema
    }

    /// 覚えた属性の数と、属性の並びの、実体の数
    #[cfg(all(test, feature = "ids"))]
    pub(crate) fn remembered(&self) -> (usize, usize) {
        (self.counts.len(), self.slots.len())
    }

    /// 実体の属性の数。上位の型が 1 つの実体は、上位の型の数に自分の属性の数を足して、実体ごとに 1 度だけ求める。
    /// 上位の型が 2 つ以上の実体は、同じ上位の型を 1 度だけ数えるために、上位の型をすべてたどる。
    /// 求める実体ごとに 1 つと、[`Layouts::count_all`] のとおりに、`tick` で数える。上位の型が 1 つかを決めるために
    /// 並びを見る所は数えないが、実体ごとに 1 度だけで、手間は EXPRESS の大きさに比例する
    pub(crate) fn count<F>(&mut self, entity: usize, tick: &mut F) -> Result<usize, Stop>
    where
        F: FnMut(u64) -> Result<(), Stop>,
    {
        let schema = self.schema;
        // 上位の型が 1 つの実体の鎖を、数の分かっている実体まで上る
        let mut path = Vec::new();
        let mut at = entity;
        let mut base = 0;
        // 上位の型の循環は読むときに断っている。回数は念のため、宣言の数で止める
        for _ in 0..=schema.decls.len() {
            if let Some(&known) = self.counts.get(&at) {
                base = known;
                break;
            }
            tick(1)?;
            let Some(current) = schema.entity(at) else {
                break;
            };
            let mut supertypes = current
                .supertypes
                .iter()
                .copied()
                .filter(|&s| schema.entity(s).is_some());
            match (supertypes.next(), supertypes.next()) {
                (None, _) => {
                    path.push(at);
                    break;
                }
                (Some(only), None) => {
                    path.push(at);
                    at = only;
                }
                _ => {
                    base = self.count_all(at, tick)?;
                    self.counts.insert(at, base);
                    break;
                }
            }
        }
        for &at in path.iter().rev() {
            base += schema.entity(at).map_or(0, |e| e.attributes.len());
            self.counts.insert(at, base);
        }
        Ok(self.counts.get(&entity).copied().unwrap_or(base))
    }

    /// 上位の型をすべてたどって数える属性の数。同じ上位の型は 1 度だけ数える。
    ///
    /// たどる 1 歩と、たどった実体の上位の型の並びの名前ごとに、`tick` で 1 つ数える。並びの名前は、すでにたどった
    /// 名前を飛ばすときも数える（#672）
    fn count_all<F>(&self, entity: usize, tick: &mut F) -> Result<usize, Stop>
    where
        F: FnMut(u64) -> Result<(), Stop>,
    {
        let mut seen = BTreeSet::new();
        let mut stack = vec![entity];
        let mut count = 0;
        while let Some(next) = stack.pop() {
            tick(1)?;
            if !seen.insert(next) {
                continue;
            }
            if let Some(entity) = self.schema.entity(next) {
                count += entity.attributes.len();
                tick(entity.supertypes.len() as u64)?;
                stack.extend(
                    entity
                        .supertypes
                        .iter()
                        .copied()
                        .filter(|s| !seen.contains(s)),
                );
            }
        }
        Ok(count)
    }

    /// 実体の属性の並び。初めてなら作って覚える。作る手順は [`Express::slots`] のとおりに数える
    pub(crate) fn slots<F>(&mut self, entity: usize, tick: &mut F) -> Result<Rc<[Slot<'a>]>, Stop>
    where
        F: FnMut(u64) -> Result<(), Stop>,
    {
        if let Some(known) = self.slots.get(&entity) {
            return Ok(Rc::clone(known));
        }
        let slots: Rc<[Slot<'a>]> = self.schema.slots(entity, tick)?.into();
        self.slots.insert(entity, Rc::clone(&slots));
        Ok(slots)
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
    /// 読んだ型・実体の名前の番号。名前を初めて読んだ順に振る（#672）
    symbols: BTreeMap<String, usize>,
    /// 番号ごとの名前と宣言
    decls: Vec<(String, Option<Decl>)>,
    /// 読んだ属性の名前（大文字）の番号。名前を初めて読んだ順に振る（#672）
    keys: BTreeMap<String, usize>,
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

    /// 型・実体の名前の番号。初めての名前なら、次の番号を振る
    fn intern(&mut self, name: String) -> usize {
        if let Some(&at) = self.symbols.get(&name) {
            return at;
        }
        let at = self.decls.len();
        self.decls.push((name.clone(), None));
        self.symbols.insert(name, at);
        at
    }

    /// 属性の名前の番号。初めての名前なら、次の番号を振る
    fn key(&mut self, name: &str) -> usize {
        if let Some(&key) = self.keys.get(name) {
            return key;
        }
        let key = self.keys.len();
        self.keys.insert(name.to_owned(), key);
        key
    }

    /// 名前を宣言する。同じ名前の宣言は、どれを使うか決められないので、宣言の名前の字句の位置 `at` を返して断る
    fn declare(&mut self, name: String, decl: Decl, at: usize) -> Parsed<()> {
        let index = self.intern(name);
        match self.decls.get_mut(index) {
            Some((_, slot @ None)) => {
                *slot = Some(decl);
                Ok(())
            }
            _ => Err(at),
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
        let mut rules = 0;
        loop {
            match self.next() {
                Some("END_SCHEMA") => {
                    self.expect(";")?;
                    return Ok(Express {
                        name,
                        symbols: core::mem::take(&mut self.symbols),
                        decls: core::mem::take(&mut self.decls),
                        rules,
                    });
                }
                Some("TYPE") => {
                    let at = self.at;
                    let (name, def, more) = self.type_decl(cx)?;
                    self.declare(name, Decl::Type(def), at)?;
                    rules += more;
                }
                Some("ENTITY") => {
                    let at = self.at;
                    let (name, entity, more) = self.entity(cx)?;
                    self.declare(name, Decl::Entity(entity), at)?;
                    rules += more;
                }
                Some("FUNCTION") => self.skip_block("FUNCTION", "END_FUNCTION")?,
                Some("PROCEDURE") => self.skip_block("PROCEDURE", "END_PROCEDURE")?,
                Some("RULE") => {
                    rules += 1;
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
        self.expect("=")?;
        let def = match self.peek() {
            Some("ENUMERATION") => {
                self.at += 1;
                self.expect("OF")?;
                TypeDef::Enumeration(self.names()?.into_iter().collect())
            }
            Some("SELECT") => {
                self.at += 1;
                let items = self.names()?;
                TypeDef::Select(items.into_iter().map(|item| self.intern(item)).collect())
            }
            _ => TypeDef::Defined(self.ty(cx)?),
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
                return Ok(Head::Item(Item::Named(self.intern(word))));
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

    /// `ENTITY` の宣言。名前（大文字）と実体と、評価しない規則の数を返す
    fn entity(&mut self, cx: &mut Cx) -> Parsed<(String, Entity, u64)> {
        let name = self.name()?;
        let mut entity = Entity {
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
                    let supertypes = self.names()?;
                    entity.supertypes = supertypes
                        .into_iter()
                        .map(|supertype| self.intern(supertype))
                        .collect();
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
                let key = self.key(&name);
                if inherited {
                    entity.redeclared.push((key, Arc::clone(&ty), optional));
                } else {
                    entity.attributes.push(Attribute {
                        name,
                        key,
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
                    return Ok((name, entity, rules));
                }
                Some("DERIVE") => {
                    while !matches!(
                        self.peek(),
                        Some("INVERSE" | "UNIQUE" | "WHERE" | "END_ENTITY") | None
                    ) {
                        let (inherited, name, _) = self.attribute_name()?;
                        if inherited {
                            let key = self.key(&name);
                            entity.derived.push(key);
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

/// チェックの指摘のうち、求めた範囲にあるものと、すべての指摘の数
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Findings {
    /// 書いた順で、求めた範囲にある指摘
    pub findings: Vec<Diagnostic>,
    /// すべての指摘の数
    pub total: u64,
}

/// IFC のインスタンスを、スキーマに照らしてチェックする。指摘は書いた順に、すべて返す（文脈には足さない）。
///
/// `FILE_SCHEMA` の名前がスキーマの名前と違えば、`express.schema.mixed` を文脈に足して、そのままチェックする。
/// 評価しない規則があれば、`express.rule.unsupported` を文脈に足す。指摘の一部だけが要るときは、[`check_range`] で
/// 範囲の外の指摘を作らずに数える
pub fn check(file: &IfcFile, schema: &Express, cx: &mut Cx) -> Result<Vec<Diagnostic>, Error> {
    check_range(file, schema, 0..u64::MAX, cx).map(|found| found.findings)
}

/// [`check`] と同じくチェックし、書いた順で `range` の位置にある指摘だけを作って返す。範囲の外の指摘は、作らずに
/// 数える。指摘が使うメモリは、合わない値の数によらず、範囲にある指摘の数までになる（#672）。
///
/// 手間は、インスタンスごと・値ごとに work で数える（決定 0216）。実体の属性の数と並び、上位の型と選択の判定は、
/// 実体か組ごとに 1 度だけ作り、作る手順を work で数える（#672）
pub fn check_range(
    file: &IfcFile,
    schema: &Express,
    range: Range<u64>,
    cx: &mut Cx,
) -> Result<Findings, Error> {
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
        layouts: Layouts::new(schema),
        kinds: vec![Kinds::Unseen; file.records().len()],
        parts: BTreeMap::new(),
        subtypes: BTreeMap::new(),
        select_entities: BTreeMap::new(),
        select_types: BTreeMap::new(),
        range,
        total: 0,
        findings: Vec::new(),
    };
    for id in file.ids() {
        let Some((position, record)) = file
            .position(id)
            .and_then(|at| Some((at, file.records().get(at)?)))
        else {
            continue;
        };
        cx.check(Kind::Work, 1)
            .map_err(|stop| stopped(cx, stop, Some(record.span())))?;
        checker.instance(position, record, cx)?;
    }
    Ok(Findings {
        findings: checker.findings,
        total: checker.total,
    })
}

/// レコードの部分の実体
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kinds {
    /// まだ名前を引いていない
    Unseen,
    /// 単独のインスタンスの実体の番号。実体でない名前なら `None`
    Single(Option<usize>),
    /// 部分が 1 つでないインスタンス。部分の実体は [`Checker::parts`] に置く
    Complex,
}

/// 値に求める実体
#[derive(Clone, Copy, Debug)]
enum Want {
    /// 実体かその下位の型
    Entity(usize),
    /// 選択が受ける実体かその下位の型
    Select(usize),
}

/// 合わなかった値に求めたもの
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Expected {
    /// 導く属性の `*`
    Derived,
    /// 属性の型
    Type,
    /// 省略できない属性の値
    Missing,
}

struct Checker<'a> {
    file: &'a IfcFile,
    schema: &'a Express,
    layouts: Layouts<'a>,
    /// レコードの位置ごとの部分の実体。参照の先の名前を、参照ごとに引き直さない
    kinds: Vec<Kinds>,
    /// 部分が 1 つでないレコードの位置から、部分ごとの実体
    parts: BTreeMap<usize, Rc<[Option<usize>]>>,
    /// （実体、求める実体）から、求める実体かその下位の型か
    subtypes: BTreeMap<(usize, usize), bool>,
    /// （選択、実体）から、選択が受ける実体かその下位の型か
    select_entities: BTreeMap<(usize, usize), bool>,
    /// （選択、型）から、選択が受ける型か
    select_types: BTreeMap<(usize, usize), bool>,
    /// 作って返す指摘の、書いた順での範囲
    range: Range<u64>,
    /// これまでの指摘の数
    total: u64,
    findings: Vec<Diagnostic>,
}

impl<'a> Checker<'a> {
    fn instance(&mut self, position: usize, record: &'a Record, cx: &mut Cx) -> Result<(), Error> {
        let kinds = self.kinds(position, record);
        let parts = record.parts();
        if let (Kinds::Single(kind), [part]) = (kinds, parts) {
            return self.part(record, part, kind, false, &BTreeSet::new(), cx);
        }
        let entities = self.parts.get(&position).cloned().unwrap_or_default();
        let complex = parts.len() > 1;
        // 複合のインスタンスで、ほかの部分が導く属性にしたものの名前の番号。名前ごとに 1 つ数える
        let mut derived_elsewhere = BTreeSet::new();
        if complex {
            let schema = self.schema;
            for entity in entities
                .iter()
                .flatten()
                .filter_map(|&at| schema.entity(at))
            {
                for &key in &entity.derived {
                    cx.check(Kind::Work, 1)
                        .map_err(|stop| stopped(cx, stop, Some(record.span())))?;
                    derived_elsewhere.insert(key);
                }
            }
        }
        for (part, kind) in parts.iter().zip(entities.iter()) {
            self.part(record, part, *kind, complex, &derived_elsewhere, cx)?;
        }
        Ok(())
    }

    /// インスタンスの部分を、実体の属性に照らす。複合のインスタンスの部分は、その実体が宣言した属性だけを持つ
    fn part(
        &mut self,
        record: &'a Record,
        part: &'a Part,
        kind: Option<usize>,
        complex: bool,
        derived_elsewhere: &BTreeSet<usize>,
        cx: &mut Cx,
    ) -> Result<(), Error> {
        let schema = self.schema;
        let Some((kind, entity)) = kind.and_then(|at| Some((at, schema.entity(at)?))) else {
            if self.keep() {
                let name =
                    String::from_utf8_lossy(self.file.bytes(part.name())).to_ascii_uppercase();
                self.push(
                    Diagnostic::new(codes::EXPRESS_ENTITY_MISSING).with("kind", name),
                    record,
                );
            }
            return Ok(());
        };
        // インスタンスの名前を大文字にしたものと同じ
        let name = schema.symbol(kind);
        if entity.is_abstract && !complex && self.keep() {
            self.push(
                Diagnostic::new(codes::EXPRESS_ABSTRACT_INVALID).with("kind", name),
                record,
            );
        }
        let span = record.span();
        let args = part.args();
        let expected = if complex {
            entity.attributes.len()
        } else {
            self.layouts
                .count(kind, &mut |n| cx.check(Kind::Work, n))
                .map_err(|stop| stopped(cx, stop, Some(span.clone())))?
        };
        if args.len() != expected {
            if self.keep() {
                self.push(
                    Diagnostic::new(codes::EXPRESS_ATTRIBUTE_COUNT_INVALID)
                        .with("kind", name)
                        .with("expected", expected as u64)
                        .with("found", args.len() as u64),
                    record,
                );
            }
            return Ok(());
        }
        if complex {
            for (position, (attribute, value)) in entity.attributes.iter().zip(args).enumerate() {
                let slot = Slot {
                    attribute,
                    ty: &attribute.ty,
                    optional: attribute.optional,
                    derived: derived_elsewhere.contains(&attribute.key),
                };
                self.slot(record, name, position, slot, value, cx)?;
            }
        } else if expected > 0 {
            let slots = self
                .layouts
                .slots(kind, &mut |n| cx.check(Kind::Work, n))
                .map_err(|stop| stopped(cx, stop, Some(span)))?;
            for (position, (slot, value)) in slots.iter().zip(args).enumerate() {
                self.slot(record, name, position, *slot, value, cx)?;
            }
        }
        Ok(())
    }

    /// 属性の値を 1 つ照らす
    fn slot(
        &mut self,
        record: &'a Record,
        kind: &str,
        position: usize,
        slot: Slot<'a>,
        value: &Value,
        cx: &mut Cx,
    ) -> Result<(), Error> {
        let expected = match (slot.derived, value) {
            (true, Value::Derived) => None,
            (true, _) => Some(Expected::Derived),
            (false, Value::Derived) => Some(Expected::Type),
            (false, Value::Null) if slot.optional => None,
            (false, Value::Null) => Some(Expected::Missing),
            (false, value) => {
                (!self.value(&slot.ty.levels, &slot.ty.item, value, cx)?).then_some(Expected::Type)
            }
        };
        let Some(expected) = expected else {
            return Ok(());
        };
        if !self.keep() {
            return Ok(());
        }
        let code = match expected {
            Expected::Missing => codes::EXPRESS_VALUE_MISSING,
            Expected::Derived | Expected::Type => codes::EXPRESS_VALUE_INVALID,
        };
        let mut diagnostic = Diagnostic::new(code)
            .with("kind", kind)
            .with("position", position as u64 + 1)
            .with("attribute", slot.attribute.name.as_str());
        match expected {
            Expected::Derived => diagnostic = diagnostic.with("expected", "*"),
            Expected::Type => {
                diagnostic = diagnostic.with("expected", self.schema.display(slot.ty))
            }
            Expected::Missing => {}
        }
        self.push(diagnostic, record);
        Ok(())
    }

    /// 指摘を 1 つ数え、作って返す範囲にあるか
    fn keep(&mut self) -> bool {
        let index = self.total;
        self.total = self.total.saturating_add(1);
        self.range.contains(&index)
    }

    fn push(&mut self, diagnostic: Diagnostic, record: &Record) {
        let place = at_bytes(record.span());
        let place = match record.id() {
            Some(id) => place.with_id(format!("#{id}")),
            None => place,
        };
        self.findings.push(diagnostic.at(place));
    }

    /// レコードの部分の実体。レコードごとに 1 度だけ名前を引いて覚える
    fn kinds(&mut self, position: usize, record: &Record) -> Kinds {
        match self.kinds.get(position) {
            Some(Kinds::Unseen) | None => {}
            Some(known) => return *known,
        }
        let (file, schema) = (self.file, self.schema);
        let entity = |part: &Part| {
            schema.entity_named(
                &String::from_utf8_lossy(file.bytes(part.name())).to_ascii_uppercase(),
            )
        };
        let kinds = match record.parts() {
            [part] => Kinds::Single(entity(part)),
            parts => {
                self.parts
                    .insert(position, parts.iter().map(entity).collect());
                Kinds::Complex
            }
        };
        if let Some(slot) = self.kinds.get_mut(position) {
            *slot = kinds;
        }
        kinds
    }

    /// 値が型に合うか。集まりの入れ子は `depth` で数える
    fn value(
        &mut self,
        levels: &'a [Level],
        item_type: &'a Item,
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
            Item::Named(at) => self.named(*at, value, cx)?,
        })
    }

    /// 名前の型か実体に合うか。定義された型は、写さずにスキーマの型を指して照らす
    fn named(&mut self, at: usize, value: &Value, cx: &mut Cx) -> Result<bool, Error> {
        let schema = self.schema;
        match schema.decl(at) {
            Some(Decl::Entity(_)) => match value {
                Value::Ref(id) => self.refers_to(*id, Want::Entity(at), cx),
                _ => Ok(false),
            },
            Some(Decl::Type(TypeDef::Defined(ty))) => {
                cx.enter().map_err(|stop| stopped(cx, stop, None))?;
                let fits = self.value(&ty.levels, &ty.item, value, cx);
                cx.leave();
                fits
            }
            Some(Decl::Type(TypeDef::Enumeration(items))) => Ok(self
                .enumeration(value)
                .is_some_and(|v| items.contains(v.as_str()))),
            Some(Decl::Type(TypeDef::Select(_))) => match value {
                Value::Ref(id) => self.refers_to(*id, Want::Select(at), cx),
                Value::Typed(part) => {
                    let type_name =
                        String::from_utf8_lossy(self.file.bytes(part.name())).to_ascii_uppercase();
                    let Some(ty) = schema.lookup(&type_name) else {
                        return Ok(false);
                    };
                    if !self.select_has_type(at, ty, cx)? {
                        return Ok(false);
                    }
                    let [inner] = part.args() else {
                        return Ok(false);
                    };
                    cx.enter().map_err(|stop| stopped(cx, stop, None))?;
                    let fits = self.named(ty, inner, cx);
                    cx.leave();
                    fits
                }
                _ => Ok(false),
            },
            None => Ok(false),
        }
    }

    /// 参照の先のインスタンスの、どれかの部分の実体が、求める実体に合うか。先がなければ、L1 で診断してあるので
    /// 合うとする。部分が 1 つでないインスタンスは、部分ごとに 1 つ数える
    fn refers_to(&mut self, id: u64, want: Want, cx: &mut Cx) -> Result<bool, Error> {
        let Some((position, record)) = self
            .file
            .position(id)
            .and_then(|at| Some((at, self.file.records().get(at)?)))
        else {
            return Ok(true);
        };
        match self.kinds(position, record) {
            Kinds::Single(Some(kind)) => self.fits(kind, want, cx),
            Kinds::Single(None) | Kinds::Unseen => Ok(false),
            Kinds::Complex => {
                let kinds = self.parts.get(&position).cloned().unwrap_or_default();
                cx.check(Kind::Work, kinds.len() as u64)
                    .map_err(|stop| stopped(cx, stop, None))?;
                for &kind in kinds.iter().flatten() {
                    if self.fits(kind, want, cx)? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
        }
    }

    /// 実体が、求める実体に合うか。組ごとに 1 度だけ判定して覚える
    fn fits(&mut self, kind: usize, want: Want, cx: &mut Cx) -> Result<bool, Error> {
        match want {
            Want::Entity(of) => {
                if let Some(&known) = self.subtypes.get(&(kind, of)) {
                    return Ok(known);
                }
                let fits = self.reaches(kind, |at| at == of, cx)?;
                self.subtypes.insert((kind, of), fits);
                Ok(fits)
            }
            Want::Select(select) => {
                if let Some(&known) = self.select_entities.get(&(select, kind)) {
                    return Ok(known);
                }
                let mut entities = BTreeSet::new();
                self.select_members(select, cx, |at, decl| {
                    if matches!(decl, Decl::Entity(_)) {
                        entities.insert(at);
                    }
                    false
                })?;
                let fits = self.reaches(kind, |at| entities.contains(&at), cx)?;
                self.select_entities.insert((select, kind), fits);
                Ok(fits)
            }
        }
    }

    /// 選択が、型を付けた値の型を受けるか。組ごとに 1 度だけ判定して覚える
    fn select_has_type(&mut self, select: usize, ty: usize, cx: &mut Cx) -> Result<bool, Error> {
        if let Some(&known) = self.select_types.get(&(select, ty)) {
            return Ok(known);
        }
        let mut found = false;
        self.select_members(select, cx, |at, decl| {
            found = at == ty && matches!(decl, Decl::Type(_));
            found
        })?;
        self.select_types.insert((select, ty), found);
        Ok(found)
    }

    /// 実体と、その上位の型をたどり、`want` を満たす実体があるか。たどる 1 歩と、たどった実体の上位の型の並びの
    /// 名前ごとに、work で 1 つ数える。並びの名前は、すでにたどった名前を飛ばすときも数える（#672）
    fn reaches(
        &self,
        kind: usize,
        want: impl Fn(usize) -> bool,
        cx: &mut Cx,
    ) -> Result<bool, Error> {
        let mut seen = BTreeSet::new();
        let mut stack = vec![kind];
        while let Some(next) = stack.pop() {
            cx.check(Kind::Work, 1)
                .map_err(|stop| stopped(cx, stop, None))?;
            if !seen.insert(next) {
                continue;
            }
            if want(next) {
                return Ok(true);
            }
            if let Some(entity) = self.schema.entity(next) {
                cx.check(Kind::Work, entity.supertypes.len() as u64)
                    .map_err(|stop| stopped(cx, stop, None))?;
                stack.extend(
                    entity
                        .supertypes
                        .iter()
                        .copied()
                        .filter(|s| !seen.contains(s)),
                );
            }
        }
        Ok(false)
    }

    /// 選択が受ける項目（実体と、定義された型・列挙）を、入れ子の選択もたどって 1 度ずつ `each` に渡す。`each` が
    /// `true` を返したら止める。たどる 1 歩と、たどった選択の項目の並びの名前ごとに、work で 1 つ数える。並びの名前は、
    /// すでにたどった名前を飛ばすときも数える（#672）
    fn select_members(
        &self,
        select: usize,
        cx: &mut Cx,
        mut each: impl FnMut(usize, &Decl) -> bool,
    ) -> Result<(), Error> {
        let mut seen = BTreeSet::new();
        let mut stack = vec![select];
        while let Some(next) = stack.pop() {
            cx.check(Kind::Work, 1)
                .map_err(|stop| stopped(cx, stop, None))?;
            if !seen.insert(next) {
                continue;
            }
            match self.schema.decl(next) {
                Some(Decl::Type(TypeDef::Select(items))) => {
                    cx.check(Kind::Work, items.len() as u64)
                        .map_err(|stop| stopped(cx, stop, None))?;
                    stack.extend(items.iter().copied().filter(|s| !seen.contains(s)));
                }
                Some(decl) if each(next, decl) => return Ok(()),
                Some(_) | None => {}
            }
        }
        Ok(())
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
    use core::cell::Cell;

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
        let entity = schema.entity(schema.lookup("E").unwrap()).unwrap();
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

    /// 属性の数は、上位の型が 1 つなら上位の型の数に足し、2 つ以上なら上位の型をすべてたどって、同じ上位の型を
    /// 1 度だけ数える。どの形でも、属性の並びの長さと同じになる。型の名前を上位の型に書いても、属性を持たない（#672）
    #[test]
    fn counts_agree_with_the_slots() {
        let schema = read(
            "SCHEMA SHAPES;\nTYPE T = INTEGER;\nEND_TYPE;\n\
             ENTITY R;\n  A : INTEGER;\nEND_ENTITY;\n\
             ENTITY L SUBTYPE OF (R);\n  B, C : INTEGER;\nEND_ENTITY;\n\
             ENTITY M SUBTYPE OF (R, T);\n  D : INTEGER;\nEND_ENTITY;\n\
             ENTITY X SUBTYPE OF (L, M);\n  E : INTEGER;\n  SELF\\R.A : REAL;\nDERIVE\n  SELF\\L.C : INTEGER := 1;\nEND_ENTITY;\n\
             ENTITY Y SUBTYPE OF (X);\n  F : INTEGER;\nEND_ENTITY;\n\
             ENTITY Z SUBTYPE OF (L, L);\nEND_ENTITY;\n\
             END_SCHEMA;\n",
        );
        let mut free = |_: u64| -> Result<(), Stop> { Ok(()) };
        for (name, expected) in [
            ("R", &["A"][..]),
            ("L", &["A", "B", "C"]),
            ("M", &["A", "D"]),
            ("X", &["A", "B", "C", "D", "E"]),
            ("Y", &["A", "B", "C", "D", "E", "F"]),
            ("Z", &["A", "B", "C"]),
        ] {
            let at = schema.entity_named(name).unwrap();
            let slots = schema.slots(at, &mut free).unwrap();
            let names: Vec<_> = slots.iter().map(|s| s.attribute.name.as_str()).collect();
            assert_eq!(names, expected, "{name}");
            // 実体ごとに、別の順で数えても同じ
            assert_eq!(
                Layouts::new(&schema).count(at, &mut free),
                Ok(slots.len()),
                "{name}"
            );
        }
        let mut layouts = Layouts::new(&schema);
        for name in ["Y", "X", "M", "L", "R", "Z"] {
            let at = schema.entity_named(name).unwrap();
            let slots = schema.slots(at, &mut free).unwrap();
            assert_eq!(layouts.count(at, &mut free), Ok(slots.len()), "{name}");
        }
        // 宣言し直した属性と導く属性は、下位の型の並びだけを変える
        let at = schema.entity_named("X").unwrap();
        let x = schema.slots(at, &mut free).unwrap();
        let first = x.first().unwrap();
        assert!(matches!(first.ty.item, Item::Real));
        assert!(x.get(2).unwrap().derived);
        let at = schema.entity_named("L").unwrap();
        let l = schema.slots(at, &mut free).unwrap();
        assert!(matches!(l.first().unwrap().ty.item, Item::Integer));
        assert!(!l.get(2).unwrap().derived);
    }

    /// 上位の型が 1 つの実体の鎖は、どの実体から数えても、実体ごとに 1 度だけたどる（#672）
    #[test]
    fn counts_each_entity_of_a_chain_once() {
        let n = 50;
        let mut src = String::from("SCHEMA CHAIN;\n");
        for i in 0..n {
            let _ = writeln!(
                src,
                "ENTITY E{i} SUBTYPE OF (E{});\n  A{i} : INTEGER;\nEND_ENTITY;",
                i + 1
            );
        }
        let _ = writeln!(
            src,
            "ENTITY E{n};\n  A{n} : INTEGER;\nEND_ENTITY;\nEND_SCHEMA;"
        );
        let schema = read(&src);
        let mut layouts = Layouts::new(&schema);
        let steps = Cell::new(0);
        let mut tick = |n: u64| -> Result<(), Stop> {
            steps.set(steps.get() + n);
            Ok(())
        };
        for i in [n / 2, 0, n, n / 3] {
            let at = schema.entity_named(&format!("E{i}")).unwrap();
            assert_eq!(layouts.count(at, &mut tick), Ok(n - i + 1));
        }
        assert_eq!(steps.get(), n as u64 + 1);
        // 並びは、作った実体だけを覚える
        let at = schema.entity_named("E0").unwrap();
        let first = layouts.slots(at, &mut tick).unwrap();
        let again = layouts.slots(at, &mut tick).unwrap();
        assert!(Rc::ptr_eq(&first, &again));
        assert_eq!(layouts.slots.len(), 1);
    }
}
