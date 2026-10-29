//! 除く表のレコードが持つ拡張辞書と、拡張辞書が持つオブジェクト（決定 0170）。
//!
//! レコードを除くとき、その拡張辞書（`{ACAD_XDICTIONARY` の組の 360）と、拡張辞書から所有を辿れる
//! オブジェクトも一緒に除く。辿るのは、辞書の項目（350・360）と、組の外の 360 と、辿ったオブジェクトの
//! 拡張辞書。辿ったオブジェクトの所有者（330）が辿った元でなければ、共有されうるので除かない

use std::collections::{BTreeMap, BTreeSet};

use super::{count_transform_values, work};
use crate::db::{AcadDb, Data, Object, Section};
use zumen_base::{Cx, Ref};
use zumen_sdk::Error;

/// レコード `record` と一緒に除くオブジェクトの位置とハンドル。拡張辞書がなければ空。
/// レコードが拡張辞書のほかの 102 の組（反応子など）か組の外の 360 を持つとき、辿れないオブジェクトか
/// 所有者の合わないオブジェクトがあるときは `None`
pub(super) fn owned(
    db: &AcadDb,
    record: &Object,
    cx: &mut Cx,
    place: &Ref,
) -> Result<Option<BTreeMap<usize, u64>>, Error> {
    let Some(handle) = record.handle() else {
        return Ok(None);
    };
    let mut found = BTreeMap::new();
    let Some(mut next) = children(record, true) else {
        return Ok(None);
    };
    let mut parents: Vec<u64> = vec![handle; next.len()];
    let mut seen = BTreeSet::from([handle]);
    while let (Some(child), Some(parent)) = (next.pop(), parents.pop()) {
        work(cx, 1, place)?;
        if !seen.insert(child) {
            return Ok(None);
        }
        let Some(position) = db.position(child) else {
            return Ok(None);
        };
        let Some(object) = db.objects().get(position) else {
            return Ok(None);
        };
        count_transform_values(object, place, cx)?;
        if object.section != Section::Objects || object.owner() != Some(parent) {
            return Ok(None);
        }
        let Some(more) = children(object, false) else {
            return Ok(None);
        };
        parents.extend(std::iter::repeat_n(child, more.len()));
        next.extend(more);
        found.insert(position, child);
    }
    Ok(Some(found))
}

/// オブジェクトが所有を示すハンドル。辞書の項目（350・360）、組の外の 360、拡張辞書の組の 360。
/// `record` なら、拡張辞書の組のほかの組か、組の外の 360 があれば `None`
fn children(object: &Object, record: bool) -> Option<Vec<u64>> {
    let dictionary = object.kind == b"DICTIONARY";
    let mut out = Vec::new();
    // 開いている組が、拡張辞書の組か
    let mut groups: Vec<bool> = Vec::new();
    for value in &object.values {
        match (value.code, &value.data) {
            (102, Data::Text(text)) if text.starts_with(b"{") => {
                let xdictionary = text.as_slice() == b"{ACAD_XDICTIONARY";
                if record && !xdictionary {
                    return None;
                }
                groups.push(xdictionary);
            }
            (102, Data::Text(text)) if text.starts_with(b"}") => {
                groups.pop();
            }
            (360, Data::Handle(handle)) if groups.last().is_some_and(|&x| x) => out.push(*handle),
            (360, _) if record && groups.is_empty() => return None,
            (360, Data::Handle(handle)) if groups.is_empty() => out.push(*handle),
            (350, Data::Handle(handle)) if dictionary && groups.is_empty() => out.push(*handle),
            _ => {}
        }
    }
    Some(out)
}

/// 位置 `skip` のほかのオブジェクトか見出しの変数が、ハンドル `handles` のどれかを指すか
pub(super) fn pointed(
    db: &AcadDb,
    skip: &BTreeSet<usize>,
    handles: &BTreeSet<u64>,
    cx: &mut Cx,
    place: &Ref,
) -> Result<bool, Error> {
    if handles.is_empty() {
        return Ok(false);
    }
    let points = |data: &Data| matches!(data, Data::Handle(found) if handles.contains(found));
    for (at, object) in db.objects().iter().enumerate() {
        work(cx, 1, place)?;
        if skip.contains(&at) {
            continue;
        }
        count_transform_values(object, place, cx)?;
        if object.values.iter().any(|value| points(&value.data)) {
            return Ok(true);
        }
    }
    for (_, values) in db.variables() {
        work(cx, 1 + values.len() as u64, place)?;
        if values.iter().any(|value| points(&value.data)) {
            return Ok(true);
        }
    }
    Ok(false)
}
