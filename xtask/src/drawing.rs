//! 図面モデル（L2）のバイナリの読み書きを、図面モデルの型から作る（決定 0246）。
//!
//! `crates/zumen-drawing/src` の型の定義を読み、`Drawing2D` と `Source` からたどれる型を集める。型の契約
//! （`schema/drawing.toml`）と突き合わせ、足した型・欄・値を後ろに足して小版を上げる。消した欄と値、型を変えた欄は、
//! 大版を上げないと書けないので、誤りにする。読み書きは、契約の順に値を並べる。

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{self, Write as _};
use std::fs;
use std::path::Path;

use serde::Deserialize;

/// 型の契約
pub const CONTRACT: &str = "schema/drawing.toml";
/// 作る読み書き
pub const GENERATED: &str = "crates/zumen-drawing/src/binary/generated.rs";

/// 図面モデルの型を読むディレクトリ。下のディレクトリ（`font/` など）は読まない
const SOURCES: &str = "crates/zumen-drawing/src";
/// 図面モデルが使う、外の crate の型を読むファイル。型は、crate の根から出している
const EXTERNAL: [(&str, &str); 2] = [
    ("zumen_base", "crates/zumen-base/src/diag.rs"),
    ("zumen_crs", "crates/zumen-crs/src/point.rs"),
];
/// 根のモジュール（lib.rs）
const ROOT: &str = "lib";
/// たどり始める型の道
const ROOTS: [&[&str]; 2] = [&["Drawing2D"], &["binary", "Source"]];
/// 数える要素の型。見出しの要素の数と、読むときの予算（決定 0010）に使う
const COUNTED: &str = "Element";
/// 欄を外に見せないので、読み書きを手で書く型（zumen-drawing の binary.rs）と、その欄。欄が変わったら、手で書いた
/// 読み書きを直してから、ここを直す
const HANDWRITTEN: [(&str, &[&str]); 1] = [(
    "Ref",
    &[
        "id: Option<String>",
        "record: Option<u64>",
        "bytes: Option<ByteRange>",
        "key: Option<Key>",
    ],
)];
/// 読み書きできる言語の型
const PRIMS: [&str; 13] = [
    "bool", "u8", "u16", "u32", "u64", "usize", "i16", "i32", "i64", "f64", "char", "String", "str",
];

/// 作ったもの
pub struct Generated {
    /// 型の契約（TOML）
    pub contract: String,
    /// 読み書き（Rust。rustfmt の前）
    pub rust: String,
}

/// 図面モデルの型と型の契約から、新しい契約と読み書きを作る
pub fn generate(root: &Path) -> Result<Generated, String> {
    let mut sources = Vec::new();
    let dir = root.join(SOURCES);
    let entries = fs::read_dir(&dir).map_err(|e| format!("{SOURCES} を読めない: {e}"))?;
    for entry in entries {
        let path = entry
            .map_err(|e| format!("{SOURCES} を読めない: {e}"))?
            .path();
        if path.extension().is_some_and(|ext| ext == "rs") {
            let stem = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .ok_or_else(|| format!("{} の名前を読めない", path.display()))?;
            let text = fs::read_to_string(&path)
                .map_err(|e| format!("{} を読めない: {e}", path.display()))?;
            sources.push((stem.to_owned(), text));
        }
    }
    for (krate, file) in EXTERNAL {
        let text =
            fs::read_to_string(root.join(file)).map_err(|e| format!("{file} を読めない: {e}"))?;
        sources.push((krate.to_owned(), text));
    }
    sources.sort();
    let tree = Tree::parse(&sources)?;
    let defs = tree
        .walk()
        .map_err(|errors| format!("図面モデルの型を読めない:\n- {}", errors.join("\n- ")))?;
    check_handwritten(&defs)?;

    let old = match fs::read_to_string(root.join(CONTRACT)) {
        Ok(text) => {
            Some(toml::from_str::<Contract>(&text).map_err(|e| format!("{CONTRACT}: {e}"))?)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(format!("{CONTRACT} を読めない: {e}")),
    };
    let (major, minor, types) = merge(old, &defs)
        .map_err(|errors| format!("{CONTRACT} に合わない:\n- {}", errors.join("\n- ")))?;
    Ok(Generated {
        contract: render_contract(major, minor, &types),
        rust: render_rust(major, minor, &types),
    })
}

/// 型の書き方。契約にも、この書き方で書く
#[derive(Clone, Debug, PartialEq, Eq)]
enum Ty {
    /// 言語の型（`u32`・`String` など）
    Prim(&'static str),
    /// `Cow<'static, str>`
    Text,
    Option(Box<Ty>),
    Vec(Box<Ty>),
    Box(Box<Ty>),
    Array(Box<Ty>, usize),
    Tuple(Vec<Ty>),
    /// 型の引数
    Param(String),
    /// 図面モデルの型。`home` は、定義したモジュール
    Named {
        name: String,
        home: String,
        args: Vec<Ty>,
    },
}

impl Ty {
    /// 型の引数を、渡した型に置き換える
    fn substitute(&self, subst: &BTreeMap<&str, &Ty>) -> Ty {
        let map = |ty: &Ty| Box::new(ty.substitute(subst));
        match self {
            Ty::Prim(_) | Ty::Text => self.clone(),
            Ty::Option(ty) => Ty::Option(map(ty)),
            Ty::Vec(ty) => Ty::Vec(map(ty)),
            Ty::Box(ty) => Ty::Box(map(ty)),
            Ty::Array(ty, len) => Ty::Array(map(ty), *len),
            Ty::Tuple(items) => Ty::Tuple(items.iter().map(|ty| ty.substitute(subst)).collect()),
            Ty::Param(name) => subst
                .get(name.as_str())
                .map_or_else(|| self.clone(), |ty| (*ty).clone()),
            Ty::Named { name, home, args } => Ty::Named {
                name: name.clone(),
                home: home.clone(),
                args: args.iter().map(|ty| ty.substitute(subst)).collect(),
            },
        }
    }
}

fn join(f: &mut fmt::Formatter<'_>, items: &[Ty]) -> fmt::Result {
    for (index, item) in items.iter().enumerate() {
        if index > 0 {
            f.write_str(", ")?;
        }
        write!(f, "{item}")?;
    }
    Ok(())
}

impl fmt::Display for Ty {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Ty::Prim(name) => f.write_str(name),
            Ty::Text => f.write_str("Cow<str>"),
            Ty::Option(ty) => write!(f, "Option<{ty}>"),
            Ty::Vec(ty) => write!(f, "Vec<{ty}>"),
            Ty::Box(ty) => write!(f, "Box<{ty}>"),
            Ty::Array(ty, len) => write!(f, "[{ty}; {len}]"),
            Ty::Tuple(items) => {
                f.write_str("(")?;
                join(f, items)?;
                f.write_str(")")
            }
            Ty::Param(name) => f.write_str(name),
            Ty::Named { name, args, .. } if args.is_empty() => f.write_str(name),
            Ty::Named { name, args, .. } => {
                write!(f, "{name}<")?;
                join(f, args)?;
                f.write_str(">")
            }
        }
    }
}

/// 図面モデルの型の定義
#[derive(Debug)]
struct Def {
    /// 定義したモジュール
    home: String,
    /// Rust の道（`crate::shape::Element` など）
    path: String,
    params: Vec<String>,
    body: Body,
}

#[derive(Debug)]
enum Body {
    /// 名前のある欄。名前は Rust の識別子（`r#type` など）のまま
    Fields(Vec<(String, Ty)>),
    /// 名前のない欄
    Tuple(Vec<Ty>),
    /// 列挙の値
    Variants(Vec<(String, Payload)>),
}

#[derive(Debug)]
enum Payload {
    Unit,
    Tuple(Vec<Ty>),
    Fields(Vec<(String, Ty)>),
}

impl Def {
    /// 欄の型
    fn types(&self) -> Vec<&Ty> {
        match &self.body {
            Body::Fields(fields) => fields.iter().map(|(_, ty)| ty).collect(),
            Body::Tuple(items) => items.iter().collect(),
            Body::Variants(variants) => variants
                .iter()
                .flat_map(|(_, payload)| match payload {
                    Payload::Unit => Vec::new(),
                    Payload::Tuple(items) => items.iter().collect(),
                    Payload::Fields(fields) => fields.iter().map(|(_, ty)| ty).collect(),
                })
                .collect(),
        }
    }
}

/// 識別子の `r#` を外す
fn unraw(ident: &str) -> &str {
    ident.strip_prefix("r#").unwrap_or(ident)
}

/// 1 つのファイルの型と `use`
#[derive(Default)]
struct Module {
    items: BTreeMap<String, syn::Item>,
    /// `use` で持ち込んだ名前と、その道
    uses: BTreeMap<String, Vec<String>>,
    /// `mod` で置いた下のモジュール
    children: BTreeSet<String>,
}

fn flatten(
    tree: &syn::UseTree,
    prefix: &mut Vec<String>,
    uses: &mut BTreeMap<String, Vec<String>>,
) {
    match tree {
        syn::UseTree::Path(path) => {
            prefix.push(path.ident.to_string());
            flatten(&path.tree, prefix, uses);
            prefix.pop();
        }
        syn::UseTree::Name(name) => {
            let name = name.ident.to_string();
            if name == "self" {
                if let Some(last) = prefix.last() {
                    uses.insert(last.clone(), prefix.clone());
                }
            } else {
                let mut path = prefix.clone();
                path.push(name.clone());
                uses.insert(name, path);
            }
        }
        syn::UseTree::Rename(rename) => {
            let mut path = prefix.clone();
            path.push(rename.ident.to_string());
            uses.insert(rename.rename.to_string(), path);
        }
        syn::UseTree::Glob(_) => {}
        syn::UseTree::Group(group) => {
            for tree in &group.items {
                flatten(tree, prefix, uses);
            }
        }
    }
}

/// `#[cfg(...)]` の付いたもの。feature で変わる型は、契約に入れない
fn configured(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| attr.path().is_ident("cfg"))
}

/// 図面モデルと、外の crate の型
struct Tree {
    modules: BTreeMap<String, Module>,
}

impl Tree {
    /// モジュールの名前（`lib`・`shape`・外の crate の名前）と、ファイルの中身から作る
    fn parse(sources: &[(String, String)]) -> Result<Self, String> {
        let mut modules = BTreeMap::new();
        for (name, text) in sources {
            let file = syn::parse_file(text).map_err(|e| format!("{name}: {e}"))?;
            let mut module = Module::default();
            for item in file.items {
                match item {
                    syn::Item::Struct(ref s) if !configured(&s.attrs) => {
                        module.items.insert(s.ident.to_string(), item);
                    }
                    syn::Item::Enum(ref e) if !configured(&e.attrs) => {
                        module.items.insert(e.ident.to_string(), item);
                    }
                    syn::Item::Use(ref u) if !configured(&u.attrs) => {
                        flatten(&u.tree, &mut Vec::new(), &mut module.uses);
                    }
                    syn::Item::Mod(ref m) if m.content.is_none() => {
                        module.children.insert(m.ident.to_string());
                    }
                    _ => {}
                }
            }
            modules.insert(name.clone(), module);
        }
        Ok(Self { modules })
    }

    fn module(&self, name: &str) -> Result<&Module, String> {
        self.modules
            .get(name)
            .ok_or_else(|| format!("モジュール `{name}` を読んでいない"))
    }

    fn external(name: &str) -> bool {
        EXTERNAL.iter().any(|(krate, _)| *krate == name)
    }

    /// `module` の中で、型の道を引く。返すのは、型を定義したモジュールと名前
    fn find(
        &self,
        module: &str,
        path: &[String],
        depth: usize,
    ) -> Result<(String, String), String> {
        let shown = path.join("::");
        if depth > 16 {
            return Err(format!("{module}: `{shown}` の use をたどりきれない"));
        }
        let here = self.module(module)?;
        match path {
            [] => Err(format!("{module}: 空の道")),
            [name] => {
                if here.items.contains_key(name) {
                    Ok((module.to_owned(), name.clone()))
                } else if let Some(target) = here.uses.get(name) {
                    self.find(module, target, depth + 1)
                } else {
                    Err(format!("{module}: 型 `{name}` が見つからない"))
                }
            }
            [first, rest @ ..] => {
                let next = match first.as_str() {
                    "crate" | "super" => ROOT.to_owned(),
                    "self" => module.to_owned(),
                    other if Self::external(other) => other.to_owned(),
                    other if module == ROOT && here.children.contains(other) => other.to_owned(),
                    other => match here.uses.get(other) {
                        Some(target) => {
                            let mut full = target.clone();
                            full.extend(rest.iter().cloned());
                            return self.find(module, &full, depth + 1);
                        }
                        None => return Err(format!("{module}: `{shown}` が見つからない")),
                    },
                };
                self.find(&next, rest, depth + 1)
            }
        }
    }

    /// 言語の型か。モジュールの型でも、言語の外から持ち込んだ型でもない名前
    fn standard(&self, module: &str, name: &str) -> Result<bool, String> {
        let here = self.module(module)?;
        Ok(!here.items.contains_key(name)
            && match here.uses.get(name) {
                Some(path) => matches!(
                    path.first().map(String::as_str),
                    Some("std" | "core" | "alloc")
                ),
                None => true,
            })
    }

    fn ty(&self, module: &str, params: &[String], ty: &syn::Type) -> Result<Ty, String> {
        match ty {
            syn::Type::Paren(inner) => self.ty(module, params, &inner.elem),
            syn::Type::Group(inner) => self.ty(module, params, &inner.elem),
            syn::Type::Array(array) => {
                let syn::Expr::Lit(syn::ExprLit {
                    lit: syn::Lit::Int(len),
                    ..
                }) = &array.len
                else {
                    return Err(format!("{module}: 配列の長さが数でない"));
                };
                let len = len
                    .base10_parse::<usize>()
                    .map_err(|e| format!("{module}: 配列の長さ: {e}"))?;
                Ok(Ty::Array(
                    Box::new(self.ty(module, params, &array.elem)?),
                    len,
                ))
            }
            syn::Type::Tuple(tuple) => Ok(Ty::Tuple(
                tuple
                    .elems
                    .iter()
                    .map(|ty| self.ty(module, params, ty))
                    .collect::<Result<_, _>>()?,
            )),
            syn::Type::Path(path) if path.qself.is_none() => self.path(module, params, &path.path),
            _ => Err(format!("{module}: 扱えない型（参照・関数など）")),
        }
    }

    fn path(&self, module: &str, params: &[String], path: &syn::Path) -> Result<Ty, String> {
        let names: Vec<String> = path.segments.iter().map(|s| s.ident.to_string()).collect();
        let Some(last) = path.segments.last() else {
            return Err(format!("{module}: 空の型"));
        };
        let shown = names.join("::");
        if path
            .segments
            .iter()
            .take(names.len() - 1)
            .any(|segment| !segment.arguments.is_none())
        {
            return Err(format!("{module}: `{shown}` の途中に型の引数がある"));
        }
        let mut args = Vec::new();
        match &last.arguments {
            syn::PathArguments::None => {}
            syn::PathArguments::AngleBracketed(list) => {
                for arg in &list.args {
                    match arg {
                        syn::GenericArgument::Lifetime(_) => {}
                        syn::GenericArgument::Type(ty) => args.push(self.ty(module, params, ty)?),
                        _ => return Err(format!("{module}: `{shown}` の引数を扱えない")),
                    }
                }
            }
            syn::PathArguments::Parenthesized(_) => {
                return Err(format!("{module}: `{shown}` を扱えない"));
            }
        }
        let name = last.ident.to_string();
        if names.len() == 1 && params.contains(&name) {
            return if args.is_empty() {
                Ok(Ty::Param(name))
            } else {
                Err(format!("{module}: 型の引数 `{name}` に引数がある"))
            };
        }
        let standard = match names.first().map(String::as_str) {
            Some("std" | "core" | "alloc") => true,
            _ => names.len() == 1 && self.standard(module, &name)?,
        };
        if standard {
            let mut args = args.into_iter();
            return match (name.as_str(), args.next(), args.next()) {
                ("Option", Some(ty), None) => Ok(Ty::Option(Box::new(ty))),
                ("Vec", Some(ty), None) => Ok(Ty::Vec(Box::new(ty))),
                ("Box", Some(ty), None) => Ok(Ty::Box(Box::new(ty))),
                ("Cow", Some(Ty::Prim("str")), None) => Ok(Ty::Text),
                (prim, None, None) => PRIMS
                    .iter()
                    .find(|known| **known == prim)
                    .map(|known| Ty::Prim(known))
                    .ok_or_else(|| format!("{module}: 言語の型 `{shown}` を扱えない")),
                _ => Err(format!("{module}: 言語の型 `{shown}` を扱えない")),
            };
        }
        let (home, name) = self.find(module, &names, 0)?;
        Ok(Ty::Named { name, home, args })
    }

    fn fields(
        &self,
        home: &str,
        params: &[String],
        fields: &syn::FieldsNamed,
        visible: bool,
    ) -> Result<Vec<(String, Ty)>, String> {
        fields
            .named
            .iter()
            .map(|field| {
                let ident = field
                    .ident
                    .as_ref()
                    .map(ToString::to_string)
                    .ok_or_else(|| format!("{home}: 名前のない欄"))?;
                if visible && matches!(field.vis, syn::Visibility::Inherited) {
                    return Err(format!(
                        "{home}: 欄 `{ident}` が外から見えないので、読み書きを作れない"
                    ));
                }
                Ok((ident, self.ty(home, params, &field.ty)?))
            })
            .collect()
    }

    fn unnamed(
        &self,
        home: &str,
        params: &[String],
        fields: &syn::FieldsUnnamed,
    ) -> Result<Vec<Ty>, String> {
        fields
            .unnamed
            .iter()
            .map(|field| self.ty(home, params, &field.ty))
            .collect()
    }

    /// 型の定義を読む
    fn define(&self, home: &str, name: &str) -> Result<Def, String> {
        let item = self
            .module(home)?
            .items
            .get(name)
            .ok_or_else(|| format!("{home}: 型 `{name}` がない"))?;
        let generics = match item {
            syn::Item::Struct(s) => &s.generics,
            syn::Item::Enum(e) => &e.generics,
            _ => return Err(format!("{home}: `{name}` は構造体でも列挙でもない")),
        };
        if generics.where_clause.is_some() {
            return Err(format!("{home}: `{name}` の where を扱えない"));
        }
        let mut params = Vec::new();
        for param in &generics.params {
            match param {
                syn::GenericParam::Type(ty) if ty.bounds.is_empty() => {
                    params.push(ty.ident.to_string())
                }
                syn::GenericParam::Type(_) => {
                    return Err(format!("{home}: `{name}` の型の引数の制約を扱えない"));
                }
                _ => return Err(format!("{home}: `{name}` の寿命・定数の引数を扱えない")),
            }
        }
        // 図面モデルの根の型の欄は、どのモジュールからも見える。外の crate の型と、ほかのモジュールの型の欄は、
        // pub でないと読み書きを作れない（手で書く型を除く）
        let handwritten = HANDWRITTEN.iter().any(|(known, _)| *known == name);
        let visible = home != ROOT && !handwritten;
        let body = match item {
            syn::Item::Struct(s) => match &s.fields {
                syn::Fields::Named(fields) => {
                    Body::Fields(self.fields(home, &params, fields, visible)?)
                }
                syn::Fields::Unnamed(fields) => {
                    if visible
                        && fields
                            .unnamed
                            .iter()
                            .any(|f| matches!(f.vis, syn::Visibility::Inherited))
                    {
                        return Err(format!(
                            "{home}: `{name}` の欄が外から見えないので、読み書きを作れない"
                        ));
                    }
                    Body::Tuple(self.unnamed(home, &params, fields)?)
                }
                syn::Fields::Unit => Body::Tuple(Vec::new()),
            },
            syn::Item::Enum(e) => Body::Variants(
                e.variants
                    .iter()
                    .map(|variant| {
                        let payload = match &variant.fields {
                            syn::Fields::Named(fields) => {
                                Payload::Fields(self.fields(home, &params, fields, false)?)
                            }
                            syn::Fields::Unnamed(fields) => {
                                Payload::Tuple(self.unnamed(home, &params, fields)?)
                            }
                            syn::Fields::Unit => Payload::Unit,
                        };
                        Ok((variant.ident.to_string(), payload))
                    })
                    .collect::<Result<_, String>>()?,
            ),
            _ => return Err(format!("{home}: `{name}` は構造体でも列挙でもない")),
        };
        let path = if home == ROOT {
            format!("crate::{name}")
        } else if Self::external(home) {
            format!("{home}::{name}")
        } else {
            format!("crate::{home}::{name}")
        };
        Ok(Def {
            home: home.to_owned(),
            path,
            params,
            body,
        })
    }

    /// 値を書くと 0 バイトになる型か。一覧の項目の数を、残りのバイト数で抑えられない
    fn zero(&self, ty: &Ty) -> Result<bool, String> {
        Ok(match ty {
            Ty::Prim(_) | Ty::Text | Ty::Option(_) | Ty::Vec(_) | Ty::Param(_) => false,
            Ty::Box(ty) => self.zero(ty)?,
            Ty::Array(ty, len) => *len == 0 || self.zero(ty)?,
            Ty::Tuple(items) => {
                let mut all = true;
                for item in items {
                    all &= self.zero(item)?;
                }
                all
            }
            Ty::Named { name, home, args } => {
                let def = self.define(home, name)?;
                match &def.body {
                    Body::Fields(_) | Body::Variants(_) => false,
                    Body::Tuple(items) => {
                        let subst = def.params.iter().map(String::as_str).zip(args).collect();
                        self.zero(&Ty::Tuple(
                            items.iter().map(|ty| ty.substitute(&subst)).collect(),
                        ))?
                    }
                }
            }
        })
    }

    /// 根からたどれる型を集める。型が自分を含めば、読む深さが決まらないので誤りにする
    fn walk(&self) -> Result<BTreeMap<String, Def>, Vec<String>> {
        let mut walk = Walk::default();
        for path in ROOTS {
            let path: Vec<String> = path.iter().map(|s| (*s).to_owned()).collect();
            match self.find(ROOT, &path, 0) {
                Ok((home, name)) => self.visit(
                    &Ty::Named {
                        name,
                        home,
                        args: Vec::new(),
                    },
                    &mut walk,
                ),
                Err(error) => walk.errors.push(error),
            }
        }
        if walk.errors.is_empty() {
            Ok(walk.defs)
        } else {
            Err(walk.errors)
        }
    }

    fn visit(&self, ty: &Ty, walk: &mut Walk) {
        match ty {
            Ty::Prim(_) | Ty::Text | Ty::Param(_) => {}
            Ty::Option(ty) | Ty::Box(ty) | Ty::Array(ty, _) => self.visit(ty, walk),
            Ty::Vec(item) => {
                match self.zero(item) {
                    Ok(true) => walk
                        .errors
                        .push(format!("`Vec<{item}>` の項目が 0 バイトになる")),
                    Ok(false) => {}
                    Err(error) => walk.errors.push(error),
                }
                self.visit(item, walk);
            }
            Ty::Tuple(items) => {
                for item in items {
                    self.visit(item, walk);
                }
            }
            Ty::Named { name, home, args } => {
                if walk.stack.contains(name) {
                    walk.errors.push(format!(
                        "型 `{name}` が自分を含む（{} → {name}）",
                        walk.stack.join(" → ")
                    ));
                    return;
                }
                if !walk.done.insert(ty.to_string()) {
                    return;
                }
                if !walk.defs.contains_key(name) {
                    match self.define(home, name) {
                        Ok(def) => {
                            walk.defs.insert(name.clone(), def);
                        }
                        Err(error) => {
                            walk.errors.push(error);
                            return;
                        }
                    }
                }
                let Some(def) = walk.defs.get(name) else {
                    return;
                };
                if def.home != *home {
                    walk.errors.push(format!(
                        "同じ名前の型 `{name}` が、{} と {home} にある",
                        def.home
                    ));
                    return;
                }
                if def.params.len() != args.len() {
                    walk.errors
                        .push(format!("`{ty}` の型の引数の数が、定義と違う"));
                    return;
                }
                let subst: BTreeMap<&str, &Ty> =
                    def.params.iter().map(String::as_str).zip(args).collect();
                let types: Vec<Ty> = def.types().iter().map(|ty| ty.substitute(&subst)).collect();
                walk.stack.push(name.clone());
                for ty in &types {
                    self.visit(ty, walk);
                }
                walk.stack.pop();
            }
        }
    }
}

#[derive(Default)]
struct Walk {
    defs: BTreeMap<String, Def>,
    /// たどり終えた型（引数を入れたもの）
    done: BTreeSet<String>,
    /// たどっている途中の型
    stack: Vec<String>,
    errors: Vec<String>,
}

/// 手で書く型の欄が、手で書いた読み書きと合うか
fn check_handwritten(defs: &BTreeMap<String, Def>) -> Result<(), String> {
    for (name, expected) in HANDWRITTEN {
        let Some(def) = defs.get(name) else {
            continue;
        };
        let actual: Vec<String> = match &def.body {
            Body::Fields(fields) => fields
                .iter()
                .map(|(field, ty)| format!("{}: {ty}", unraw(field)))
                .collect(),
            _ => Vec::new(),
        };
        if actual != expected {
            return Err(format!(
                "`{name}` の欄（{}）が、手で書いた読み書き（{}）と違う。zumen-drawing の binary.rs と、xtask の HANDWRITTEN を直す",
                actual.join(", "),
                expected.join(", ")
            ));
        }
    }
    Ok(())
}

/// 型の契約（`schema/drawing.toml`）
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Contract {
    versions: Versions,
    #[serde(default)]
    types: Vec<Entry>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Versions {
    major: u16,
    minor: u16,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    name: String,
    #[serde(default)]
    params: Vec<String>,
    #[serde(default)]
    since: u16,
    fields: Option<Vec<String>>,
    tuple: Option<Vec<String>>,
    variants: Option<Vec<VariantEntry>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct VariantEntry {
    name: String,
    #[serde(default)]
    since: u16,
    fields: Option<Vec<String>>,
    tuple: Option<Vec<String>>,
}

/// 契約に書く型
#[derive(Debug, PartialEq)]
struct Type {
    name: String,
    path: String,
    params: Vec<String>,
    since: u16,
    layout: Layout,
}

#[derive(Debug, PartialEq)]
enum Layout {
    Fields(Vec<Field>),
    Tuple(Vec<String>),
    Variants(Vec<Variant>),
}

#[derive(Debug, PartialEq)]
struct Field {
    /// Rust の識別子
    ident: String,
    ty: String,
    since: u16,
}

#[derive(Debug, PartialEq)]
struct Variant {
    name: String,
    since: u16,
    payload: Shape,
}

#[derive(Debug, PartialEq)]
enum Shape {
    Unit,
    Tuple(Vec<String>),
    Fields(Vec<Field>),
}

fn new_fields(fields: &[(String, Ty)], since: u16) -> Vec<Field> {
    fields
        .iter()
        .map(|(ident, ty)| Field {
            ident: ident.clone(),
            ty: ty.to_string(),
            since,
        })
        .collect()
}

fn new_shape(payload: &Payload, since: u16) -> Shape {
    match payload {
        Payload::Unit => Shape::Unit,
        Payload::Tuple(items) => Shape::Tuple(items.iter().map(ToString::to_string).collect()),
        Payload::Fields(fields) => Shape::Fields(new_fields(fields, since)),
    }
}

fn new_type(name: &str, def: &Def, since: u16) -> Type {
    let layout = match &def.body {
        Body::Fields(fields) => Layout::Fields(new_fields(fields, since)),
        Body::Tuple(items) => Layout::Tuple(items.iter().map(ToString::to_string).collect()),
        Body::Variants(variants) => Layout::Variants(
            variants
                .iter()
                .map(|(name, payload)| Variant {
                    name: name.clone(),
                    since,
                    payload: new_shape(payload, since),
                })
                .collect(),
        ),
    };
    Type {
        name: name.to_owned(),
        path: def.path.clone(),
        params: def.params.clone(),
        since,
        layout,
    }
}

/// 契約の欄の行（`名前: 型` か `名前: 型 @小版`）を読む
fn parse_field(line: &str) -> Result<(&str, &str, u16), String> {
    let (name, rest) = line
        .split_once(": ")
        .ok_or_else(|| format!("欄 `{line}` が `名前: 型` の形でない"))?;
    match rest.rsplit_once(" @") {
        Some((ty, since)) => {
            let since = since
                .parse()
                .map_err(|_| format!("欄 `{line}` の小版が数でない"))?;
            Ok((name, ty, since))
        }
        None => Ok((name, rest, 0)),
    }
}

/// 名前のある欄を突き合わせる
struct Merge {
    /// 足すものに付ける小版
    next: u16,
    /// 今の小版
    minor: u16,
    added: bool,
    errors: Vec<String>,
}

impl Merge {
    fn fields(
        &mut self,
        owner: &str,
        since: u16,
        old: &[String],
        now: &[(String, Ty)],
    ) -> Vec<Field> {
        let mut out = Vec::new();
        let mut kept = BTreeSet::new();
        for line in old {
            let (name, ty, field_since) = match parse_field(line) {
                Ok(parsed) => parsed,
                Err(error) => {
                    self.errors.push(format!("`{owner}`: {error}"));
                    continue;
                }
            };
            if field_since > self.minor || (field_since != 0 && field_since <= since) {
                self.errors.push(format!(
                    "`{owner}` の欄 `{name}` の小版 {field_since} が合わない"
                ));
            }
            match now.iter().find(|(ident, _)| unraw(ident) == name) {
                Some((ident, now_ty)) if now_ty.to_string() == ty => {
                    kept.insert(name.to_owned());
                    out.push(Field {
                        ident: ident.clone(),
                        ty: ty.to_owned(),
                        since: field_since.max(since),
                    });
                }
                Some((_, now_ty)) => self.errors.push(format!(
                    "`{owner}` の欄 `{name}` の型が `{ty}` から `{now_ty}` に変わった。変えるには大版を上げる"
                )),
                None => self.errors.push(format!(
                    "`{owner}` の欄 `{name}` がなくなった。消すには大版を上げる"
                )),
            }
        }
        for (ident, ty) in now {
            if !kept.contains(unraw(ident)) {
                self.added = true;
                out.push(Field {
                    ident: ident.clone(),
                    ty: ty.to_string(),
                    since: self.next,
                });
            }
        }
        out
    }

    fn tuple(&mut self, owner: &str, old: &[String], now: &[Ty]) -> Vec<String> {
        let now: Vec<String> = now.iter().map(ToString::to_string).collect();
        if now != old {
            self.errors.push(format!(
                "`{owner}` の欄（{}）が（{}）に変わった。名前のない欄は、変えるには大版を上げる",
                old.join(", "),
                now.join(", ")
            ));
        }
        now
    }

    fn variants(
        &mut self,
        owner: &str,
        since: u16,
        old: &[VariantEntry],
        now: &[(String, Payload)],
    ) -> Vec<Variant> {
        let mut out = Vec::new();
        let mut kept = BTreeSet::new();
        for entry in old {
            let label = format!("{owner}::{}", entry.name);
            if entry.since > self.minor || (entry.since != 0 && entry.since <= since) {
                self.errors
                    .push(format!("`{label}` の小版 {} が合わない", entry.since));
            }
            let Some((_, payload)) = now.iter().find(|(name, _)| *name == entry.name) else {
                self.errors
                    .push(format!("`{label}` がなくなった。消すには大版を上げる"));
                continue;
            };
            let variant_since = entry.since.max(since);
            let shape = match (payload, &entry.fields, &entry.tuple) {
                (Payload::Unit, None, None) => Shape::Unit,
                (Payload::Tuple(items), None, Some(old)) => {
                    Shape::Tuple(self.tuple(&label, old, items))
                }
                (Payload::Fields(fields), Some(old), None) => {
                    Shape::Fields(self.fields(&label, variant_since, old, fields))
                }
                _ => {
                    self.errors.push(format!(
                        "`{label}` の形（値だけ・組・欄）が変わった。変えるには大版を上げる"
                    ));
                    continue;
                }
            };
            kept.insert(entry.name.clone());
            out.push(Variant {
                name: entry.name.clone(),
                since: variant_since,
                payload: shape,
            });
        }
        for (name, payload) in now {
            if !kept.contains(name) && !out.iter().any(|v: &Variant| v.name == *name) {
                self.added = true;
                out.push(Variant {
                    name: name.clone(),
                    since: self.next,
                    payload: new_shape(payload, self.next),
                });
            }
        }
        out
    }
}

/// 契約と図面モデルの型を突き合わせる。返すのは、大版・小版と、名前の順の型
fn merge(
    old: Option<Contract>,
    defs: &BTreeMap<String, Def>,
) -> Result<(u16, u16, Vec<Type>), Vec<String>> {
    let (major, minor, entries) = match old {
        Some(contract) => (
            contract.versions.major,
            contract.versions.minor,
            contract.types,
        ),
        None => (1, 0, Vec::new()),
    };
    // 型のない契約は、その大版の始まり
    if entries.is_empty() {
        let types = defs
            .iter()
            .map(|(name, def)| new_type(name, def, 0))
            .collect();
        return Ok((major, 0, types));
    }
    let Some(next) = minor.checked_add(1) else {
        return Err(vec!["小版を上げきった。大版を上げる".to_owned()]);
    };
    let mut merge = Merge {
        next,
        minor,
        added: false,
        errors: Vec::new(),
    };
    let mut types = Vec::new();
    let mut seen = BTreeSet::new();
    for entry in &entries {
        if !seen.insert(entry.name.clone()) {
            merge
                .errors
                .push(format!("型 `{}` が 2 つある", entry.name));
            continue;
        }
        if entry.since > minor {
            merge.errors.push(format!(
                "`{}` の小版 {} が合わない",
                entry.name, entry.since
            ));
        }
        let Some(def) = defs.get(&entry.name) else {
            merge.errors.push(format!(
                "契約の型 `{}` が、図面モデルからたどれない。消すには大版を上げる",
                entry.name
            ));
            continue;
        };
        if entry.params != def.params {
            merge.errors.push(format!(
                "`{}` の型の引数が変わった。変えるには大版を上げる",
                entry.name
            ));
            continue;
        }
        let layout = match (&def.body, &entry.fields, &entry.tuple, &entry.variants) {
            (Body::Fields(fields), Some(old), None, None) => {
                Layout::Fields(merge.fields(&entry.name, entry.since, old, fields))
            }
            (Body::Tuple(items), None, Some(old), None) => {
                Layout::Tuple(merge.tuple(&entry.name, old, items))
            }
            (Body::Variants(variants), None, None, Some(old)) => {
                Layout::Variants(merge.variants(&entry.name, entry.since, old, variants))
            }
            _ => {
                merge.errors.push(format!(
                    "`{}` の形（欄・組・列挙）が変わった。変えるには大版を上げる",
                    entry.name
                ));
                continue;
            }
        };
        types.push(Type {
            name: entry.name.clone(),
            path: def.path.clone(),
            params: def.params.clone(),
            since: entry.since,
            layout,
        });
    }
    for (name, def) in defs {
        if !seen.contains(name) {
            merge.added = true;
            types.push(new_type(name, def, next));
        }
    }
    if !merge.errors.is_empty() {
        return Err(merge.errors);
    }
    types.sort_by(|a, b| a.name.cmp(&b.name));
    Ok((major, if merge.added { next } else { minor }, types))
}

fn quoted(items: &[String]) -> String {
    items
        .iter()
        .map(|item| format!("\"{item}\""))
        .collect::<Vec<_>>()
        .join(", ")
}

fn field_line(field: &Field, owner: u16) -> String {
    let name = unraw(&field.ident);
    if field.since > owner {
        format!("{name}: {} @{}", field.ty, field.since)
    } else {
        format!("{name}: {}", field.ty)
    }
}

fn render_contract(major: u16, minor: u16, types: &[Type]) -> String {
    let mut out = String::from(
        "# 図面モデル（L2）のバイナリの型の契約（決定 0246）。\n\
         #\n\
         # `cargo xtask codegen` が、図面モデル（crates/zumen-drawing）の型から書き、読み書き\n\
         # （crates/zumen-drawing/src/binary/generated.rs）を作る。手で直すのは、大版を上げるときだけ。\n\
         #\n\
         # - 名前のある欄は、並べた順に書く。後の小版で足した欄は後ろに足し、`@` の後ろにその小版を書く\n\
         # - 列挙の値の番号は、並べた順（0 から）。後の小版で足した値は後ろに足し、since にその小版を書く\n\
         # - 型の書き方は Rust に合わせる。`Cow<str>` は文字列、`P` などは型の引数\n\
         # - 欄・値を消すか、名前・型を変えるには、大版を上げる。major を上げ、types を消してから codegen を流す\n\
         \n",
    );
    let _ = writeln!(out, "[versions]\nmajor = {major}\nminor = {minor}");
    for ty in types {
        let _ = write!(out, "\n[[types]]\nname = \"{}\"\n", ty.name);
        if !ty.params.is_empty() {
            let _ = writeln!(out, "params = [{}]", quoted(&ty.params));
        }
        if ty.since > 0 {
            let _ = writeln!(out, "since = {}", ty.since);
        }
        match &ty.layout {
            Layout::Fields(fields) => {
                out.push_str("fields = [\n");
                for field in fields {
                    let _ = writeln!(out, "  \"{}\",", field_line(field, ty.since));
                }
                out.push_str("]\n");
            }
            Layout::Tuple(items) => {
                let _ = writeln!(out, "tuple = [{}]", quoted(items));
            }
            Layout::Variants(variants) => {
                out.push_str("variants = [\n");
                for variant in variants {
                    let _ = write!(out, "  {{ name = \"{}\"", variant.name);
                    match &variant.payload {
                        Shape::Unit => {}
                        Shape::Tuple(items) => {
                            let _ = write!(out, ", tuple = [{}]", quoted(items));
                        }
                        Shape::Fields(fields) => {
                            let lines: Vec<String> = fields
                                .iter()
                                .map(|f| field_line(f, variant.since))
                                .collect();
                            let _ = write!(out, ", fields = [{}]", quoted(&lines));
                        }
                    }
                    if variant.since > ty.since {
                        let _ = write!(out, ", since = {}", variant.since);
                    }
                    out.push_str(" },\n");
                }
                out.push_str("]\n");
            }
        }
    }
    out
}

/// 欄を読む式。後の小版で足した欄は、古いバイナリになければ既定の値にする
fn read_field(late: bool) -> &'static str {
    if late {
        "if input.more() { Unpack::unpack(input)? } else { Default::default() }"
    } else {
        "Unpack::unpack(input)?"
    }
}

fn render_type(out: &mut String, ty: &Type) {
    let bounds = |name: &str| {
        if ty.params.is_empty() {
            String::new()
        } else {
            let list: Vec<String> = ty.params.iter().map(|p| format!("{p}: {name}")).collect();
            format!("<{}>", list.join(", "))
        }
    };
    let target = if ty.params.is_empty() {
        ty.path.clone()
    } else {
        format!("{}<{}>", ty.path, ty.params.join(", "))
    };
    let name = &ty.name;
    let counted = name == COUNTED;
    let mut pack = String::new();
    let mut unpack = String::new();
    if counted {
        pack.push_str("out.element();\n");
        unpack.push_str("input.element()?;\n");
    }
    match &ty.layout {
        Layout::Fields(fields) => {
            let idents: Vec<&str> = fields.iter().map(|f| f.ident.as_str()).collect();
            let _ = writeln!(pack, "let Self {{ {} }} = self;", idents.join(", "));
            pack.push_str("out.record(|out| {\n");
            for field in fields {
                let _ = writeln!(pack, "{}.pack(out);", field.ident);
            }
            pack.push_str("});\n");
            let _ = writeln!(unpack, "input.record(\"{name}\", |input| Ok(Self {{");
            for field in fields {
                let _ = writeln!(
                    unpack,
                    "{}: {},",
                    field.ident,
                    read_field(field.since > ty.since)
                );
            }
            unpack.push_str("}))\n");
        }
        Layout::Tuple(items) => {
            let binds: Vec<String> = (0..items.len()).map(|i| format!("f{i}")).collect();
            let _ = writeln!(pack, "let Self({}) = self;", binds.join(", "));
            for bind in &binds {
                let _ = writeln!(pack, "{bind}.pack(out);");
            }
            let reads: Vec<&str> = items.iter().map(|_| read_field(false)).collect();
            let _ = writeln!(unpack, "Ok(Self({}))", reads.join(", "));
        }
        Layout::Variants(variants) => {
            pack.push_str("match self {\n");
            let mut arms = Vec::new();
            for (tag, variant) in variants.iter().enumerate() {
                let v = &variant.name;
                match &variant.payload {
                    Shape::Unit => {
                        let _ = writeln!(pack, "Self::{v} => out.uint({tag}),");
                        arms.push(format!("Self::{v}"));
                    }
                    Shape::Tuple(items) => {
                        let binds: Vec<String> =
                            (0..items.len()).map(|i| format!("f{i}")).collect();
                        let _ = write!(
                            pack,
                            "Self::{v}({}) => {{ out.uint({tag});",
                            binds.join(", ")
                        );
                        for bind in &binds {
                            let _ = write!(pack, " {bind}.pack(out);");
                        }
                        pack.push_str(" }\n");
                        let reads: Vec<&str> = items.iter().map(|_| read_field(false)).collect();
                        arms.push(format!("Self::{v}({})", reads.join(", ")));
                    }
                    Shape::Fields(fields) => {
                        let idents: Vec<&str> = fields.iter().map(|f| f.ident.as_str()).collect();
                        let _ = write!(
                            pack,
                            "Self::{v} {{ {} }} => {{ out.uint({tag}); out.record(|out| {{",
                            idents.join(", ")
                        );
                        for ident in &idents {
                            let _ = write!(pack, " {ident}.pack(out);");
                        }
                        pack.push_str(" }); }\n");
                        let reads: Vec<String> = fields
                            .iter()
                            .map(|f| {
                                format!("{}: {}", f.ident, read_field(f.since > variant.since))
                            })
                            .collect();
                        arms.push(format!(
                            "input.record(\"{name}\", |input| Ok(Self::{v} {{ {} }}))?",
                            reads.join(", ")
                        ));
                    }
                }
            }
            pack.push_str("}\n");
            let count = variants.len();
            if let [arm] = arms.as_slice() {
                let _ = writeln!(unpack, "input.variant(\"{name}\", 1)?;\nOk({arm})");
            } else {
                let _ = writeln!(unpack, "Ok(match input.variant(\"{name}\", {count})? {{");
                let last = arms.len() - 1;
                for (tag, arm) in arms.iter().enumerate() {
                    if tag == last {
                        let _ = writeln!(unpack, "_ => {arm},");
                    } else {
                        let _ = writeln!(unpack, "{tag} => {arm},");
                    }
                }
                unpack.push_str("})\n");
            }
        }
    }
    let _ = write!(
        out,
        "\nimpl{} Pack for {target} {{\nfn pack(&self, out: &mut Writer) {{\n{pack}}}\n}}\n",
        bounds("Pack")
    );
    let _ = write!(
        out,
        "\nimpl{} Unpack for {target} {{\nfn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {{\n{unpack}}}\n}}\n",
        bounds("Unpack")
    );
}

fn render_rust(major: u16, minor: u16, types: &[Type]) -> String {
    let mut out = String::from(
        "// 生成したファイル。手で直さない。`cargo xtask codegen` が、図面モデルの型と schema/drawing.toml から作る（決定 0246）。\n\n\
         use super::{Input, Pack, Unpack, Writer};\n\n",
    );
    let _ = writeln!(
        out,
        "/// 大版。読み手は、同じ大版のバイナリだけを読む\npub const MAJOR: u16 = {major};\n\n\
         /// 小版。型の契約に型・欄・値を足すたびに上げる\npub const MINOR: u16 = {minor};"
    );
    for ty in types {
        if HANDWRITTEN.iter().any(|(name, _)| *name == ty.name) {
            continue;
        }
        render_type(&mut out, ty);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree(lib: &str) -> Tree {
        Tree::parse(&[
            ("lib".to_owned(), lib.to_owned()),
            (
                "binary".to_owned(),
                "pub struct Source { pub format: String }".to_owned(),
            ),
            ("zumen_base".to_owned(), String::new()),
            ("zumen_crs".to_owned(), String::new()),
        ])
        .unwrap()
    }

    fn contract(text: &str) -> Option<Contract> {
        Some(toml::from_str(text).unwrap())
    }

    const LIB: &str = "pub mod binary;\n\
        pub struct Drawing2D { pub layers: Vec<Layer>, pub shape: Shape<f64> }\n\
        pub struct Layer(pub u32);\n\
        pub enum Shape<P> { Empty, Point(P), Line { from: P, to: P } }";

    #[test]
    fn writes_a_fresh_contract_and_appends_what_is_added() {
        let defs = tree(LIB).walk().unwrap();
        let (major, minor, types) = merge(None, &defs).unwrap();
        assert_eq!((major, minor), (1, 0));
        let text = render_contract(major, minor, &types);
        assert!(text.contains("tuple = [\"u32\"]"), "{text}");
        assert!(
            text.contains("{ name = \"Line\", fields = [\"from: P\", \"to: P\"] },"),
            "{text}"
        );
        // 書いた契約を読み直すと、同じになる
        assert_eq!(merge(contract(&text), &defs).unwrap(), (1, 0, types));

        let added = "pub mod binary;\n\
            pub struct Drawing2D { pub layers: Vec<Layer>, pub shape: Shape<f64>, pub name: Option<String> }\n\
            pub struct Layer(pub u32);\n\
            pub enum Shape<P> { Empty, Point(P), Line { from: P, to: P, width: f64 }, Circle(P, f64) }";
        let defs = tree(added).walk().unwrap();
        let (_, minor, types) = merge(contract(&text), &defs).unwrap();
        assert_eq!(minor, 1);
        let text = render_contract(1, minor, &types);
        assert!(text.contains("\"name: Option<String> @1\""), "{text}");
        assert!(text.contains("\"width: f64 @1\""), "{text}");
        assert!(
            text.contains("{ name = \"Circle\", tuple = [\"P\", \"f64\"], since = 1 },"),
            "{text}"
        );
        let rust = render_rust(1, minor, &types);
        assert!(rust.contains("name: if input.more()"), "{rust}");
        assert!(rust.contains("_ => Self::Circle("), "{rust}");
    }

    #[test]
    fn refuses_what_needs_a_major_version() {
        let defs = tree(LIB).walk().unwrap();
        let (_, _, types) = merge(None, &defs).unwrap();
        let text = render_contract(1, 0, &types);
        let changed = "pub mod binary;\n\
            pub struct Drawing2D { pub layers: Vec<Layer>, pub shape: Shape<i64> }\n\
            pub struct Layer(pub u64);\n\
            pub enum Shape<P> { Empty, Line { from: P } }";
        let errors = merge(contract(&text), &tree(changed).walk().unwrap()).unwrap_err();
        let all = errors.join("\n");
        assert!(all.contains("`Layer` の欄"), "{all}");
        assert!(all.contains("`Shape::Point` がなくなった"), "{all}");
        assert!(
            all.contains("`Shape::Line` の欄 `to` がなくなった"),
            "{all}"
        );
    }

    #[test]
    fn refuses_types_that_contain_themselves_or_take_no_bytes() {
        let errors = tree(
            "pub mod binary;\n\
             pub struct Drawing2D { pub group: Group, pub marks: Vec<Mark> }\n\
             pub struct Group { pub inner: Vec<Group> }\n\
             pub struct Mark;",
        )
        .walk()
        .unwrap_err();
        let all = errors.join("\n");
        assert!(all.contains("型 `Group` が自分を含む"), "{all}");
        assert!(all.contains("`Vec<Mark>` の項目が 0 バイトになる"), "{all}");
    }
}
