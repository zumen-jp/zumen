//! IFC の L1 の変更（決定 0207・0244、#393）。
//!
//! 変更を、触れたインスタンスの写しに重ねて当て、触れたインスタンスだけを書き直す。足すインスタンスには、今の最大
//! の番号の次から振り、データの節の終わり（`ENDSEC;`）の前に書く。名前を挙げないインスタンスは、元のバイトのまま
//! 写す。共有されたプロパティのセットとプロパティは、写しを作って付け替え、ほかの要素の値を変えない。

use std::collections::{BTreeMap, BTreeSet};

use zumen_base::{Diagnostic, Kind, codes};
use zumen_building::{GlobalId, Hash};
use zumen_sdk::{Cx, Error};

use crate::building::{Category, category, decode};
use crate::{Form, IfcFile, Value, at_bytes, stopped};

/// 変更 1 つ
#[derive(Clone, Debug, PartialEq)]
pub enum Edit {
    /// 要素のプロパティのセット `set` の、プロパティ `name`（`IfcPropertySingleValue`）の値を変える。セットか
    /// プロパティがなければ足す
    Property {
        /// 要素のインスタンスの番号
        element: u64,
        /// プロパティのセットの名前
        set: String,
        /// プロパティの名前
        name: String,
        /// 値
        value: NewValue,
    },
    /// 要素（`IfcRoot`）の名前か説明を変える。`None` は値をなくす（`$`）
    Attribute {
        /// 要素のインスタンスの番号
        element: u64,
        /// 変える属性
        attribute: RootAttribute,
        /// 値
        value: Option<String>,
    },
}

/// `IfcRoot` の属性
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RootAttribute {
    /// 3 Name
    Name,
    /// 4 Description
    Description,
}

/// 書く値。型の名前（`IfcLabel` など）と値
#[derive(Clone, Debug, PartialEq)]
pub struct NewValue {
    /// 値の型の名前。英字で始まり、英数字と `_` だけのもの
    pub type_name: String,
    /// 値
    pub literal: NewLiteral,
}

/// 値
#[derive(Clone, Debug, PartialEq)]
pub enum NewLiteral {
    /// 文字列
    Text(String),
    /// 実数。有限のもの
    Real(f64),
    /// 整数
    Integer(i64),
    /// 真偽
    Boolean(bool),
    /// 真・偽・不明（`None`）
    Logical(Option<bool>),
}

/// 当てた結果
#[derive(Clone, Debug, PartialEq)]
pub struct Applied {
    /// 書き直したファイル
    pub bytes: Vec<u8>,
    /// 書き直したインスタンスと、足したインスタンスの数
    pub changed: usize,
}

/// 変更を順に当て、書き直したファイルを返す。当てられない変更が 1 つでもあれば、診断して失敗する。
/// `seed` は、足す GlobalId を決めるための値で、同じ入力と変更なら同じ GlobalId になる
pub fn apply(file: &IfcFile, edits: &[Edit], seed: &[u8], cx: &mut Cx) -> Result<Applied, Error> {
    let mut draft = Draft::new(file, seed, cx)?;
    let mut rejected = false;
    for edit in edits {
        cx.check(Kind::Work, 1)
            .map_err(|stop| stopped(cx, stop, None))?;
        let ok = match edit {
            Edit::Property {
                element,
                set,
                name,
                value,
            } => draft.property(*element, set, name, value, cx)?,
            Edit::Attribute {
                element,
                attribute,
                value,
            } => draft.attribute(*element, *attribute, value.as_deref(), cx),
        };
        rejected |= !ok;
    }
    if rejected {
        return Err(Error::Invalid);
    }
    let changed = draft.changed.len();
    let bytes = draft.write(cx)?;
    Ok(Applied { bytes, changed })
}

/// 書き直す値。参照と並びは中を見られるように持ち、ほかは書いたバイト列で持つ
#[derive(Clone, Debug, PartialEq)]
enum Item {
    Ref(u64),
    List(Vec<Item>),
    Raw(Vec<u8>),
}

/// 書き直すインスタンス。名前と引数
#[derive(Clone, Debug, PartialEq)]
struct Stmt {
    name: Vec<u8>,
    args: Vec<Item>,
}

impl Stmt {
    fn arg(&self, at: usize) -> Option<&Item> {
        self.args.get(at.checked_sub(1)?)
    }

    fn set(&mut self, at: usize, item: Item) {
        if let Some(slot) = at.checked_sub(1).and_then(|at| self.args.get_mut(at)) {
            *slot = item;
        }
    }
}

struct Draft<'f> {
    file: &'f IfcFile,
    /// 書き直すか足すインスタンス
    changed: BTreeMap<u64, Stmt>,
    /// 足したインスタンス。足した順
    added: Vec<u64>,
    next: u64,
    /// 逆向きの参照。重ねた変更も入れる
    referrers: BTreeMap<u64, BTreeSet<u64>>,
    seed: Vec<u8>,
    minted: u64,
}

impl<'f> Draft<'f> {
    fn new(file: &'f IfcFile, seed: &[u8], cx: &mut Cx) -> Result<Self, Error> {
        let inverse = file.inverse(cx)?;
        let mut referrers: BTreeMap<u64, BTreeSet<u64>> = BTreeMap::new();
        let mut next = 0;
        for id in file.ids() {
            next = next.max(id);
            for &by in inverse.referrers(id) {
                referrers.entry(id).or_default().insert(by);
            }
        }
        Ok(Self {
            file,
            changed: BTreeMap::new(),
            added: Vec::new(),
            next: next.saturating_add(1),
            referrers,
            seed: seed.to_vec(),
            minted: 0,
        })
    }

    /// 今のインスタンス。複合のインスタンスと、ないものは `None`
    fn stmt(&self, id: u64) -> Option<Stmt> {
        if let Some(stmt) = self.changed.get(&id) {
            return Some(stmt.clone());
        }
        let record = self.file.instance(id)?;
        if record.form() != Form::Entity {
            return None;
        }
        let part = record.parts().first()?;
        Some(Stmt {
            name: self.file.bytes(part.name()).to_ascii_uppercase(),
            args: part.args().iter().map(|v| self.item(v)).collect(),
        })
    }

    fn item(&self, value: &Value) -> Item {
        match value {
            Value::Ref(id) => Item::Ref(*id),
            Value::List(values) => Item::List(values.iter().map(|v| self.item(v)).collect()),
            _ => {
                let mut out = Vec::new();
                self.raw(value, &mut out);
                Item::Raw(out)
            }
        }
    }

    /// 元の値を、書いたとおりに書く
    fn raw(&self, value: &Value, out: &mut Vec<u8>) {
        match value {
            Value::Null => out.push(b'$'),
            Value::Derived => out.push(b'*'),
            Value::Ref(id) => out.extend_from_slice(format!("#{id}").as_bytes()),
            Value::Enum(range) => {
                out.push(b'.');
                out.extend_from_slice(self.file.bytes(range.clone()));
                out.push(b'.');
            }
            // IFC の文字列は `'…'` だけ。フィーチャのコメントの囲みは、IFC の L1 では読まない
            Value::Text { raw, .. } => {
                out.push(b'\'');
                out.extend_from_slice(self.file.bytes(raw.clone()));
                out.push(b'\'');
            }
            Value::Number(range) | Value::Token(range) => {
                out.extend_from_slice(self.file.bytes(range.clone()));
            }
            Value::Binary(range) => {
                out.push(b'"');
                out.extend_from_slice(self.file.bytes(range.clone()));
                out.push(b'"');
            }
            Value::Typed(part) => {
                out.extend_from_slice(self.file.bytes(part.name()));
                out.push(b'(');
                for (at, arg) in part.args().iter().enumerate() {
                    if at > 0 {
                        out.push(b',');
                    }
                    self.raw(arg, out);
                }
                out.push(b')');
            }
            Value::List(values) => {
                out.push(b'(');
                for (at, value) in values.iter().enumerate() {
                    if at > 0 {
                        out.push(b',');
                    }
                    self.raw(value, out);
                }
                out.push(b')');
            }
        }
    }

    /// インスタンスの名前（`IFCWALL` など）
    fn kind(&self, id: u64) -> Vec<u8> {
        match self.changed.get(&id) {
            Some(stmt) => stmt.name.clone(),
            None => self
                .file
                .instance(id)
                .map(|record| self.file.kind(record).to_ascii_uppercase())
                .unwrap_or_default(),
        }
    }

    fn referrers(&self, id: u64) -> Vec<u64> {
        self.referrers
            .get(&id)
            .map(|set| set.iter().copied().collect())
            .unwrap_or_default()
    }

    /// インスタンスを書き直す。逆向きの参照も直す
    fn put(&mut self, id: u64, stmt: Stmt) {
        if let Some(old) = self.stmt(id) {
            for target in refs_in(&old.args) {
                if let Some(set) = self.referrers.get_mut(&target) {
                    set.remove(&id);
                }
            }
        }
        for target in refs_in(&stmt.args) {
            self.referrers.entry(target).or_default().insert(id);
        }
        self.changed.insert(id, stmt);
    }

    /// インスタンスを足し、番号を返す
    fn add(&mut self, stmt: Stmt) -> u64 {
        let id = self.next;
        self.next = self.next.saturating_add(1);
        self.added.push(id);
        self.put(id, stmt);
        id
    }

    /// 足す GlobalId。`seed` と、足した数から決める
    fn global_id(&mut self) -> Item {
        let mut input = self.seed.clone();
        input.extend_from_slice(&self.minted.to_le_bytes());
        self.minted += 1;
        let Hash(bytes) = Hash::document(&input);
        text(&GlobalId::from_uuid(bytes).to_string())
    }

    /// 要素（`IfcRoot`）の 2 OwnerHistory。要素でなければ `None`
    fn root(&self, element: u64, edit: &str, cx: &mut Cx) -> Option<(Stmt, Category)> {
        let stmt = self.stmt(element);
        let found = stmt.as_ref().and_then(|stmt| {
            let global_id = text_of(stmt.arg(1)?)?;
            (global_id.len() == 22).then_some(())?;
            Some(category(&stmt.name, self.file.schema()))
        });
        match (stmt, found) {
            (Some(stmt), Some(category)) => Some((stmt, category.unwrap_or(Category::Element))),
            (stmt, _) => {
                let kind = stmt.map(|s| s.name).unwrap_or_default();
                cx.report(unsupported(edit, &kind));
                None
            }
        }
    }

    fn attribute(
        &mut self,
        element: u64,
        attribute: RootAttribute,
        value: Option<&str>,
        cx: &mut Cx,
    ) -> bool {
        let edit = match attribute {
            RootAttribute::Name => "name",
            RootAttribute::Description => "description",
        };
        let Some((mut stmt, _)) = self.root(element, edit, cx) else {
            return false;
        };
        let at = match attribute {
            RootAttribute::Name => 3,
            RootAttribute::Description => 4,
        };
        if stmt.arg(at).is_none() {
            cx.report(unsupported(edit, &stmt.name));
            return false;
        }
        stmt.set(at, value.map_or_else(|| Item::Raw(b"$".to_vec()), text));
        self.put(element, stmt);
        true
    }

    fn property(
        &mut self,
        element: u64,
        set: &str,
        name: &str,
        value: &NewValue,
        cx: &mut Cx,
    ) -> Result<bool, Error> {
        let Some((owner, category)) = self.root(element, "property", cx) else {
            return Ok(false);
        };
        let Some(written) = typed(value) else {
            cx.report(unsupported("property", &owner.name));
            return Ok(false);
        };
        let typed_object = matches!(category, Category::Type | Category::Style);
        let found = if typed_object {
            self.type_set(element, &owner, set)
        } else {
            self.occurrence_set(element, set, cx)?
        };
        let target = match found {
            Some((holder, pset)) => {
                let exclusive = self.referrers(pset) == [holder]
                    && (typed_object
                        || self
                            .stmt(holder)
                            .map(|rel| refs_of(rel.arg(5)) == [element])
                            .unwrap_or_default());
                if exclusive {
                    pset
                } else {
                    self.copy_set(element, &owner, holder, pset, typed_object)
                }
            }
            None => self.new_set(element, &owner, set, typed_object),
        };
        self.set_property(target, name, written, &owner.name, cx)
    }

    /// 要素に `IfcRelDefinesByProperties` で付いた、名前が `set` の `IfcPropertySet` と、その関係
    fn occurrence_set(
        &self,
        element: u64,
        set: &str,
        cx: &mut Cx,
    ) -> Result<Option<(u64, u64)>, Error> {
        for rel in self.referrers(element) {
            cx.check(Kind::Work, 1)
                .map_err(|stop| stopped(cx, stop, None))?;
            if self.kind(rel) != b"IFCRELDEFINESBYPROPERTIES" {
                continue;
            }
            let Some(stmt) = self.stmt(rel) else {
                continue;
            };
            if !refs_of(stmt.arg(5)).contains(&element) {
                continue;
            }
            for pset in refs_of(stmt.arg(6)) {
                if self.is_set(pset, set) {
                    return Ok(Some((rel, pset)));
                }
            }
        }
        Ok(None)
    }

    /// 型の 6 HasPropertySets の、名前が `set` の `IfcPropertySet`。持ち主は型
    fn type_set(&self, element: u64, owner: &Stmt, set: &str) -> Option<(u64, u64)> {
        refs_of(owner.arg(6))
            .into_iter()
            .find(|&pset| self.is_set(pset, set))
            .map(|pset| (element, pset))
    }

    fn is_set(&self, pset: u64, set: &str) -> bool {
        self.kind(pset) == b"IFCPROPERTYSET"
            && self
                .stmt(pset)
                .and_then(|stmt| text_of(stmt.arg(3)?))
                .is_some_and(|name| name == set)
    }

    /// 共有されたセットの写しを作り、この要素だけに付け替える
    fn copy_set(
        &mut self,
        element: u64,
        owner: &Stmt,
        holder: u64,
        pset: u64,
        typed_object: bool,
    ) -> u64 {
        let Some(mut copy) = self.stmt(pset) else {
            return pset;
        };
        copy.set(1, self.global_id());
        let copy = self.add(copy);
        if typed_object {
            if let Some(mut stmt) = self.stmt(holder) {
                let sets = swap(stmt.arg(6), pset, copy);
                stmt.set(6, sets);
                self.put(holder, stmt);
            }
            return copy;
        }
        let Some(mut rel) = self.stmt(holder) else {
            return copy;
        };
        if refs_of(rel.arg(5)) == [element] {
            let sets = swap(rel.arg(6), pset, copy);
            rel.set(6, sets);
            self.put(holder, rel);
        } else {
            let related = match rel.arg(5) {
                Some(Item::List(items)) => Item::List(
                    items
                        .iter()
                        .filter(|item| **item != Item::Ref(element))
                        .cloned()
                        .collect(),
                ),
                other => other.cloned().unwrap_or(Item::List(Vec::new())),
            };
            rel.set(5, related);
            self.put(holder, rel);
            let history = owner.arg(2).cloned().unwrap_or(Item::Raw(b"$".to_vec()));
            self.relate(element, copy, history);
        }
        copy
    }

    /// 名前が `set` の空のセットを足し、要素に付ける
    fn new_set(&mut self, element: u64, owner: &Stmt, set: &str, typed_object: bool) -> u64 {
        let history = owner.arg(2).cloned().unwrap_or(Item::Raw(b"$".to_vec()));
        let stmt = Stmt {
            name: b"IFCPROPERTYSET".to_vec(),
            args: vec![
                self.global_id(),
                history.clone(),
                text(set),
                Item::Raw(b"$".to_vec()),
                Item::List(Vec::new()),
            ],
        };
        let pset = self.add(stmt);
        if typed_object {
            if let Some(mut stmt) = self.stmt(element) {
                let sets = match stmt.arg(6) {
                    Some(Item::List(items)) => {
                        let mut items = items.clone();
                        items.push(Item::Ref(pset));
                        Item::List(items)
                    }
                    _ => Item::List(vec![Item::Ref(pset)]),
                };
                stmt.set(6, sets);
                self.put(element, stmt);
            }
        } else {
            self.relate(element, pset, history);
        }
        pset
    }

    /// 要素とセットを、新しい `IfcRelDefinesByProperties` で結ぶ
    fn relate(&mut self, element: u64, pset: u64, history: Item) {
        let stmt = Stmt {
            name: b"IFCRELDEFINESBYPROPERTIES".to_vec(),
            args: vec![
                self.global_id(),
                history,
                Item::Raw(b"$".to_vec()),
                Item::Raw(b"$".to_vec()),
                Item::List(vec![Item::Ref(element)]),
                Item::Ref(pset),
            ],
        };
        self.add(stmt);
    }

    /// セット `pset` のプロパティ `name` に値を書く。ほかのセットと共有するプロパティは、写しを作る
    fn set_property(
        &mut self,
        pset: u64,
        name: &str,
        value: Vec<u8>,
        owner_kind: &[u8],
        cx: &mut Cx,
    ) -> Result<bool, Error> {
        let Some(mut set) = self.stmt(pset) else {
            cx.report(unsupported("property", owner_kind));
            return Ok(false);
        };
        let mut found = None;
        for property in refs_of(set.arg(5)) {
            cx.check(Kind::Work, 1)
                .map_err(|stop| stopped(cx, stop, None))?;
            let named = self
                .stmt(property)
                .and_then(|stmt| text_of(stmt.arg(1)?))
                .is_some_and(|text| text == name);
            if named {
                found = Some(property);
                break;
            }
        }
        match found {
            Some(property) => {
                if self.kind(property) != b"IFCPROPERTYSINGLEVALUE" {
                    let kind = self.kind(property);
                    cx.report(unsupported("property", &kind));
                    return Ok(false);
                }
                let Some(mut stmt) = self.stmt(property) else {
                    return Ok(false);
                };
                stmt.set(3, Item::Raw(value));
                if self.referrers(property) == [pset] {
                    self.put(property, stmt);
                } else {
                    let copy = self.add(stmt);
                    let properties = swap(set.arg(5), property, copy);
                    set.set(5, properties);
                    self.put(pset, set);
                }
            }
            None => {
                let stmt = Stmt {
                    name: b"IFCPROPERTYSINGLEVALUE".to_vec(),
                    args: vec![
                        text(name),
                        Item::Raw(b"$".to_vec()),
                        Item::Raw(value),
                        Item::Raw(b"$".to_vec()),
                    ],
                };
                let property = self.add(stmt);
                let properties = match set.arg(5) {
                    Some(Item::List(items)) => {
                        let mut items = items.clone();
                        items.push(Item::Ref(property));
                        Item::List(items)
                    }
                    _ => Item::List(vec![Item::Ref(property)]),
                };
                set.set(5, properties);
                self.put(pset, set);
            }
        }
        Ok(true)
    }

    /// 書き直したファイル。触れたインスタンスは書き直し、足したものはデータの節の終わりの前に書く
    fn write(&self, cx: &mut Cx) -> Result<Vec<u8>, Error> {
        let src = self.file.source();
        let newline: &[u8] = if src.windows(2).any(|w| w == b"\r\n") {
            b"\r\n"
        } else {
            b"\n"
        };
        let mut out = Vec::with_capacity(src.len());
        let mut end = 0;
        let mut in_data = false;
        let mut placed = self.added.is_empty();
        for record in self.file.records() {
            cx.check(Kind::Work, 1)
                .map_err(|stop| stopped(cx, stop, Some(record.span())))?;
            out.extend_from_slice(self.file.bytes(end..record.span().start));
            let name = self.file.kind(record).to_ascii_uppercase();
            let keyword = record.id().is_none();
            if keyword && name == b"DATA" {
                in_data = true;
            } else if keyword && name == b"ENDSEC" && in_data && !placed {
                for &id in &self.added {
                    if let Some(stmt) = self.changed.get(&id) {
                        write_stmt(id, stmt, &mut out);
                        out.extend_from_slice(newline);
                    }
                }
                placed = true;
                in_data = false;
            }
            match record.id().and_then(|id| {
                let first = self.file.instance(id).map(|r| r.span()) == Some(record.span());
                first.then(|| self.changed.get(&id).map(|stmt| (id, stmt)))?
            }) {
                Some((id, stmt)) => write_stmt(id, stmt, &mut out),
                None => out.extend_from_slice(self.file.bytes(record.span())),
            }
            end = record.span().end;
        }
        out.extend_from_slice(self.file.bytes(end..src.len()));
        if !placed {
            cx.report(Diagnostic::new(codes::IFC_FILE_INVALID).at(at_bytes(0..0)));
            return Err(Error::Invalid);
        }
        Ok(out)
    }
}

fn write_stmt(id: u64, stmt: &Stmt, out: &mut Vec<u8>) {
    out.extend_from_slice(format!("#{id}=").as_bytes());
    out.extend_from_slice(&stmt.name);
    out.push(b'(');
    for (at, item) in stmt.args.iter().enumerate() {
        if at > 0 {
            out.push(b',');
        }
        write_item(item, out);
    }
    out.extend_from_slice(b");");
}

fn write_item(item: &Item, out: &mut Vec<u8>) {
    match item {
        Item::Ref(id) => out.extend_from_slice(format!("#{id}").as_bytes()),
        Item::Raw(bytes) => out.extend_from_slice(bytes),
        Item::List(items) => {
            out.push(b'(');
            for (at, item) in items.iter().enumerate() {
                if at > 0 {
                    out.push(b',');
                }
                write_item(item, out);
            }
            out.push(b')');
        }
    }
}

fn refs_in(items: &[Item]) -> Vec<u64> {
    let mut found = Vec::new();
    let mut stack: Vec<&Item> = items.iter().rev().collect();
    while let Some(item) = stack.pop() {
        match item {
            Item::Ref(id) => found.push(*id),
            Item::List(items) => stack.extend(items.iter().rev()),
            Item::Raw(_) => {}
        }
    }
    found
}

/// 参照の列か、1 つの参照
fn refs_of(item: Option<&Item>) -> Vec<u64> {
    match item {
        Some(Item::Ref(id)) => vec![*id],
        Some(Item::List(items)) => items
            .iter()
            .filter_map(|item| match item {
                Item::Ref(id) => Some(*id),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// 参照 `from` を `to` に替えた値
fn swap(item: Option<&Item>, from: u64, to: u64) -> Item {
    match item {
        Some(Item::Ref(id)) if *id == from => Item::Ref(to),
        Some(Item::List(items)) => Item::List(
            items
                .iter()
                .map(|item| match item {
                    Item::Ref(id) if *id == from => Item::Ref(to),
                    other => other.clone(),
                })
                .collect(),
        ),
        other => other.cloned().unwrap_or(Item::Raw(b"$".to_vec())),
    }
}

/// 文字列の値の中身。文字列でなければ `None`
fn text_of(item: &Item) -> Option<String> {
    let Item::Raw(bytes) = item else {
        return None;
    };
    let inner = bytes.strip_prefix(b"'")?.strip_suffix(b"'")?;
    Some(decode(inner).0)
}

/// 文字列を ISO 10303-21 の書き方にした値。`'` と `\` は重ね、印字できる ASCII の外は `\X2\…\X0\` か
/// `\X4\…\X0\` にする
fn text(value: &str) -> Item {
    let mut out = vec![b'\''];
    let mut wide: Vec<u32> = Vec::new();
    let flush = |wide: &mut Vec<u32>, out: &mut Vec<u8>| {
        if wide.is_empty() {
            return;
        }
        let astral = wide.iter().any(|&c| c > 0xFFFF);
        if astral {
            out.extend_from_slice(b"\\X4\\");
            for c in wide.iter() {
                out.extend_from_slice(format!("{c:08X}").as_bytes());
            }
        } else {
            out.extend_from_slice(b"\\X2\\");
            for c in wide.iter() {
                out.extend_from_slice(format!("{c:04X}").as_bytes());
            }
        }
        out.extend_from_slice(b"\\X0\\");
        wide.clear();
    };
    for c in value.chars() {
        match c {
            ' '..='~' => {
                flush(&mut wide, &mut out);
                match c {
                    '\'' => out.extend_from_slice(b"''"),
                    '\\' => out.extend_from_slice(b"\\\\"),
                    _ => out.push(c as u8),
                }
            }
            _ => {
                // \X2\ と \X4\ の区切りを変えないよう、桁の違う文字の前で閉じる
                let astral = u32::from(c) > 0xFFFF;
                if wide.last().is_some_and(|&last| (last > 0xFFFF) != astral) {
                    flush(&mut wide, &mut out);
                }
                wide.push(u32::from(c));
            }
        }
    }
    flush(&mut wide, &mut out);
    out.push(b'\'');
    Item::Raw(out)
}

/// 型を付けた値（`IFCLABEL('…')`）。型の名前か値が書けなければ `None`
fn typed(value: &NewValue) -> Option<Vec<u8>> {
    let name = value.type_name.as_bytes();
    let valid = name.first().is_some_and(u8::is_ascii_alphabetic)
        && name.iter().all(|b| b.is_ascii_alphanumeric() || *b == b'_');
    if !valid {
        return None;
    }
    let inner = match &value.literal {
        NewLiteral::Text(text_value) => match text(text_value) {
            Item::Raw(bytes) => bytes,
            _ => return None,
        },
        NewLiteral::Real(real) => {
            if !real.is_finite() {
                return None;
            }
            let mut written = format!("{real}");
            if !written.contains('.') {
                written.push('.');
            }
            written.into_bytes()
        }
        NewLiteral::Integer(integer) => integer.to_string().into_bytes(),
        NewLiteral::Boolean(true) | NewLiteral::Logical(Some(true)) => b".T.".to_vec(),
        NewLiteral::Boolean(false) | NewLiteral::Logical(Some(false)) => b".F.".to_vec(),
        NewLiteral::Logical(None) => b".U.".to_vec(),
    };
    let mut out = name.to_ascii_uppercase();
    out.push(b'(');
    out.extend_from_slice(&inner);
    out.push(b')');
    Some(out)
}

/// 当てられない変更の診断
fn unsupported(edit: &str, kind: &[u8]) -> Diagnostic {
    Diagnostic::new(codes::CHANGE_EDIT_UNSUPPORTED)
        .with("edit", edit)
        .with("kind", String::from_utf8_lossy(kind).into_owned())
        .with("format", "IFC")
}
