//! 関係の索引（決定 0207）。成分が持つ向きと逆から引く表を、問い合わせるときに作る。

use std::collections::BTreeMap;

use zumen_base::{Cx, Kind, Stop};

use crate::{Building, ElementRef, Opening};

/// 逆から引く関係の表。どの列も要素の番号の順
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Relations {
    children: BTreeMap<ElementRef, Vec<ElementRef>>,
    referenced: BTreeMap<ElementRef, Vec<ElementRef>>,
    instances: BTreeMap<ElementRef, Vec<ElementRef>>,
    openings: BTreeMap<ElementRef, Vec<ElementRef>>,
    fillings: BTreeMap<ElementRef, Vec<ElementRef>>,
    members: BTreeMap<ElementRef, Vec<ElementRef>>,
}

fn get(map: &BTreeMap<ElementRef, Vec<ElementRef>>, key: ElementRef) -> &[ElementRef] {
    map.get(&key).map(Vec::as_slice).unwrap_or_default()
}

impl Relations {
    /// 所属の先が `element` の要素（集約・入れ子・空間への包含）
    pub fn children(&self, element: ElementRef) -> &[ElementRef] {
        get(&self.children, element)
    }

    /// 空間 `element` を参照する、複数の階にわたる要素
    pub fn referenced(&self, element: ElementRef) -> &[ElementRef] {
        get(&self.referenced, element)
    }

    /// 型 `element` の要素
    pub fn instances(&self, element: ElementRef) -> &[ElementRef] {
        get(&self.instances, element)
    }

    /// 要素 `element` に穴をあける開口
    pub fn openings(&self, element: ElementRef) -> &[ElementRef] {
        get(&self.openings, element)
    }

    /// 開口 `element` を埋める建具
    pub fn fillings(&self, element: ElementRef) -> &[ElementRef] {
        get(&self.fillings, element)
    }

    /// グループ・系統・区域 `element` に所属する要素
    pub fn members(&self, element: ElementRef) -> &[ElementRef] {
        get(&self.members, element)
    }
}

impl Building {
    /// 関係の索引を作る。成分の 1 つと、そこから引く参照の 1 つごとに `work` を 1 つ数える
    pub fn relations(&self, cx: &mut Cx<'_>) -> Result<Relations, Stop> {
        let mut out = Relations::default();
        let add = |map: &mut BTreeMap<ElementRef, Vec<ElementRef>>, key, value| {
            map.entry(key).or_default().push(value);
        };
        let c = &self.components;
        for (element, parent) in c.parent.iter() {
            cx.check(Kind::Work, 1 + parent.referenced_in.len() as u64)?;
            if let Some(of) = &parent.of {
                add(&mut out.children, of.target, element);
            }
            for space in &parent.referenced_in {
                add(&mut out.referenced, *space, element);
            }
        }
        for (element, of) in c.r#type.iter() {
            cx.check(Kind::Work, 1)?;
            add(&mut out.instances, of.target, element);
        }
        for (element, opening) in c.opening.iter() {
            cx.check(Kind::Work, 1)?;
            match opening {
                Opening::Voids(host) => add(&mut out.openings, *host, element),
                Opening::Fills(opening) => add(&mut out.fillings, *opening, element),
            }
        }
        for (element, group) in c.group.iter() {
            cx.check(Kind::Work, 1 + group.groups.len() as u64)?;
            for target in &group.groups {
                add(&mut out.members, *target, element);
            }
        }
        Ok(out)
    }
}
