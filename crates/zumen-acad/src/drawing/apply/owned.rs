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
    while let (Some((child, dictionary)), Some(parent)) = (next.pop(), parents.pop()) {
        work(cx, 1, place)?;
        if !seen.insert(child) {
            return Ok(None);
        }
        // 編集の計画中は索引を組んだ後に表を変えない。存在しないハンドルでも全体を走査しない。
        let Some(position) = db.indexed_position(child) else {
            return Ok(None);
        };
        let Some(object) = db.objects().get(position) else {
            return Ok(None);
        };
        count_transform_values(object, place, cx)?;
        if object.handle() != Some(child)
            || object.section != Section::Objects
            || object.owner() != Some(parent)
            || (dictionary && object.kind != b"DICTIONARY")
        {
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
fn children(object: &Object, record: bool) -> Option<Vec<(u64, bool)>> {
    let dictionary = object.kind == b"DICTIONARY";
    let mut out = Vec::new();
    // 開いている組が、拡張辞書の組か
    let mut groups: Vec<(bool, usize)> = Vec::new();
    let mut has_xdictionary = false;
    for value in &object.values {
        match (value.code, &value.data) {
            (102, Data::Text(text)) if text.starts_with(b"{") => {
                let xdictionary = text.as_slice() == b"{ACAD_XDICTIONARY";
                if record && !xdictionary {
                    return None;
                }
                if xdictionary && (has_xdictionary || !groups.is_empty()) {
                    return None;
                }
                has_xdictionary |= xdictionary;
                groups.push((xdictionary, 0));
            }
            (102, Data::Text(text)) if text.starts_with(b"}") => {
                let (xdictionary, count) = groups.pop()?;
                if xdictionary && count != 1 {
                    return None;
                }
            }
            (360, data) if groups.last().is_some_and(|&(x, _)| x) => {
                let Data::Handle(handle) = data else {
                    return None;
                };
                if *handle == 0 {
                    return None;
                }
                groups.last_mut()?.1 += 1;
                out.push((*handle, true));
            }
            (360, _) if record && groups.is_empty() => return None,
            (360, Data::Handle(handle)) if groups.is_empty() => out.push((*handle, false)),
            (350, Data::Handle(handle)) if dictionary && groups.is_empty() => {
                out.push((*handle, false))
            }
            (360, _) if groups.is_empty() => return None,
            (350, _) if dictionary && groups.is_empty() => return None,
            _ => {}
        }
    }
    groups.is_empty().then_some(out)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{Origin, Value};
    use zumen_base::{Budget, CancelToken, Diagnostics, Kind};

    #[test]
    fn missing_child_does_not_search_unrelated_objects() {
        let layer = Object {
            kind: b"LAYER".to_vec(),
            section: Section::Tables,
            values: vec![
                Value {
                    code: 5,
                    data: Data::Handle(1),
                },
                Value {
                    code: 102,
                    data: Data::Text(b"{ACAD_XDICTIONARY".to_vec()),
                },
                Value {
                    code: 360,
                    data: Data::Handle(2),
                },
                Value {
                    code: 102,
                    data: Data::Text(b"}".to_vec()),
                },
            ],
            origin: Origin::New,
        };
        let run = |extra| {
            let mut db = AcadDb::default();
            db.push(layer.clone());
            for _ in 0..extra {
                db.push(Object {
                    kind: b"XRECORD".to_vec(),
                    section: Section::Objects,
                    values: Vec::new(),
                    origin: Origin::New,
                });
            }
            let mut budget = Budget::default();
            let mut diagnostics = Diagnostics::new();
            let cancel = CancelToken::new();
            let result = owned(
                &db,
                &layer,
                &mut Cx::new(&mut budget, &mut diagnostics, &cancel),
                &Ref::default(),
            )
            .unwrap();
            assert!(result.is_none());
            budget.used(Kind::Work)
        };
        assert_eq!(run(0), run(10_000));
    }

    #[test]
    fn stale_index_does_not_fall_back_to_search() {
        let layer = Object {
            kind: b"LAYER".to_vec(),
            section: Section::Tables,
            values: vec![
                Value {
                    code: 5,
                    data: Data::Handle(1),
                },
                Value {
                    code: 102,
                    data: Data::Text(b"{ACAD_XDICTIONARY".to_vec()),
                },
                Value {
                    code: 360,
                    data: Data::Handle(2),
                },
                Value {
                    code: 102,
                    data: Data::Text(b"}".to_vec()),
                },
            ],
            origin: Origin::New,
        };
        let dictionary = Object {
            kind: b"DICTIONARY".to_vec(),
            section: Section::Objects,
            values: vec![
                Value {
                    code: 5,
                    data: Data::Handle(3),
                },
                Value {
                    code: 330,
                    data: Data::Handle(1),
                },
            ],
            origin: Origin::New,
        };
        let mut db = AcadDb::default();
        db.push(layer.clone());
        db.push(dictionary);
        db.object_mut(1).unwrap().values[0].data = Data::Handle(2);
        assert_eq!(db.position(2), Some(1));
        let mut budget = Budget::default();
        let mut diagnostics = Diagnostics::new();
        let cancel = CancelToken::new();
        assert!(
            owned(
                &db,
                &layer,
                &mut Cx::new(&mut budget, &mut diagnostics, &cancel),
                &Ref::default()
            )
            .unwrap()
            .is_none()
        );
        let mut old_reference = layer;
        old_reference.values[2].data = Data::Handle(3);
        assert!(
            owned(
                &db,
                &old_reference,
                &mut Cx::new(&mut budget, &mut diagnostics, &cancel),
                &Ref::default()
            )
            .unwrap()
            .is_none()
        );
    }
}
