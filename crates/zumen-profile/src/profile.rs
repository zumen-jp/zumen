//! プロファイルの JSON（`profile.json`）を読み、`extends` を解く（決定 0138・0173）。
//!
//! 読み手として、知らない欄は読み飛ばす。規則と語彙は JSON の値のまま持ち、照合の種類ごとに読む。

use std::collections::BTreeMap;

use zumen_base::{Cx, Diagnostic, Kind, codes};
use zumen_json::Value;

use crate::Error;

/// 読める書式の大版。小版が新しくても読む
pub const FORMAT_MAJOR: &str = "0";

/// プロファイルの識別
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Identity {
    /// 基準の系統
    pub id: String,
    /// 基準の版
    pub edition: String,
    /// このデータの直し
    pub revision: u64,
}

/// 読んだプロファイル
#[derive(Clone, Debug)]
struct Profile {
    identity: Identity,
    extends: Option<(String, String)>,
    vocabularies: Vec<(String, Value)>,
    rules: Vec<(String, Value)>,
}

/// `extends` を解いたプロファイル
#[derive(Clone, Debug)]
pub(crate) struct Resolved {
    /// 基から順の鎖
    pub chain: Vec<Identity>,
    /// 名前ごとの語彙。子が全体を置き換える
    pub vocabularies: BTreeMap<String, Value>,
    /// 規則の id と、規則の値。同じ id は子が置き換え、新しい id は後ろに足す
    pub rules: Vec<(String, Value)>,
}

/// 入力の名前とバイト列から、`id` と `edition` のプロファイルを選び、`extends` を解く。
/// 鎖の深さを予算の `depth` で数える（決定 0010）
pub(crate) fn resolve(
    inputs: &[(&str, &[u8])],
    id: &str,
    edition: &str,
    cx: &mut Cx<'_>,
) -> Result<Resolved, Error> {
    let mut profiles = Vec::with_capacity(inputs.len());
    for (name, bytes) in inputs {
        cx.check(Kind::InputBytes, bytes.len() as u64)
            .map_err(|stop| Error::Stop(stop, Box::default()))?;
        let Some(profile) = read(bytes) else {
            return Err(Error::Profile(Box::new(
                Diagnostic::new(codes::PROFILE_FORMAT_UNSUPPORTED).with("name", *name),
            )));
        };
        profiles.push(profile);
    }
    let find = |id: &str, edition: &str| {
        profiles
            .iter()
            .position(|p| p.identity.id == id && p.identity.edition == edition)
    };
    let Some(mut at) = find(id, edition) else {
        return Err(Error::NotFound);
    };
    // 子から基へ
    let mut chain: Vec<usize> = Vec::new();
    let result = loop {
        if let Err(stop) = cx.enter() {
            break Err(Error::Stop(stop, Box::default()));
        }
        chain.push(at);
        let Some(profile) = profiles.get(at) else {
            break Err(Error::NotFound);
        };
        let Some((id, edition)) = &profile.extends else {
            break Ok(());
        };
        match find(id, edition) {
            Some(next) if chain.contains(&next) => {
                break Err(Error::Profile(Box::new(
                    Diagnostic::new(codes::PROFILE_EXTENDS_INVALID)
                        .with("id", id.as_str())
                        .with("edition", edition.as_str()),
                )));
            }
            Some(next) => at = next,
            None => {
                break Err(Error::Profile(Box::new(
                    Diagnostic::new(codes::PROFILE_EXTENDS_MISSING)
                        .with("id", id.as_str())
                        .with("edition", edition.as_str()),
                )));
            }
        }
    };
    for _ in 0..chain.len() {
        cx.leave();
    }
    result?;
    let mut resolved = Resolved {
        chain: Vec::new(),
        vocabularies: BTreeMap::new(),
        rules: Vec::new(),
    };
    for &index in chain.iter().rev() {
        let Some(profile) = profiles.get(index) else {
            continue;
        };
        resolved.chain.push(profile.identity.clone());
        for (name, value) in &profile.vocabularies {
            resolved.vocabularies.insert(name.clone(), value.clone());
        }
        for (id, rule) in &profile.rules {
            match resolved.rules.iter_mut().find(|(known, _)| known == id) {
                Some(slot) => slot.1 = rule.clone(),
                None => resolved.rules.push((id.clone(), rule.clone())),
            }
        }
    }
    Ok(resolved)
}

/// プロファイルの JSON を読む。書式の大版が違うか、必ず要る欄がなければ `None`
fn read(bytes: &[u8]) -> Option<Profile> {
    let value = zumen_json::parse(bytes).ok()?;
    let format = text(value.get_key("format")?)?;
    if format.split('.').next() != Some(FORMAT_MAJOR) {
        return None;
    }
    let revision = match value.get_key("revision")? {
        Value::Number(number) => u64::try_from(number.to_safe_integer()?).ok()?,
        _ => return None,
    };
    let identity = Identity {
        id: text(value.get_key("id")?)?.to_string(),
        edition: text(value.get_key("edition")?)?.to_string(),
        revision,
    };
    let extends = match value.get_key("extends") {
        None => None,
        Some(extends) => Some((
            text(extends.get_key("id")?)?.to_string(),
            text(extends.get_key("edition")?)?.to_string(),
        )),
    };
    let vocabularies = match value.get_key("vocabularies") {
        None => Vec::new(),
        Some(Value::Object(object)) => object
            .iter()
            .map(|(name, value)| (name.to_string(), value.clone()))
            .collect(),
        Some(_) => return None,
    };
    let rules = match value.get_key("rules") {
        None => Vec::new(),
        Some(Value::Array(rules)) => rules
            .iter()
            .map(|rule| Some((text(rule.get_key("id")?)?.to_string(), rule.clone())))
            .collect::<Option<Vec<_>>>()?,
        Some(_) => return None,
    };
    Some(Profile {
        identity,
        extends,
        vocabularies,
        rules,
    })
}

/// 文字列の値
pub(crate) fn text(value: &Value) -> Option<&str> {
    match value {
        Value::String(text) => Some(text),
        _ => None,
    }
}

/// オブジェクトの欄を引く
pub(crate) trait GetKey {
    fn get_key(&self, key: &str) -> Option<&Value>;
}

impl GetKey for Value {
    fn get_key(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Object(object) => object.get(key),
            _ => None,
        }
    }
}
