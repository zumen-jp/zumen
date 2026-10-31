//! `review_carry`・`review_adopt`・`review_adopt_ids` の op（決定 0214・0218・0219）。レビューの文書を入力で
//! 受け、図面か建物モデルの要素の ID とハッシュで新しい版へ移すか、照合の指摘を取り込んで、文書を blob で返す。

use std::collections::BTreeMap;

use zumen_base::{Cx, Stop};
use zumen_drawing::Drawing2D;
use zumen_json::{ToJson, Writer};
use zumen_review::{Adopted, Change, Reference, Review, Target, Version};

use crate::messages::{
    self, CarriedState, ReviewAdoptResult, ReviewCarryRequest, ReviewCarryResult,
};
use crate::session::{Failed, fail, invalid_value, stopped};

/// limit を書かなかったときに返す参照の数
const LIMIT: u64 = 1000;

/// 文書の版。文書のハッシュと、要素の ID とハッシュ（決定 0156・0207）
pub(crate) struct Plan {
    pub(crate) hash: String,
    pub(crate) elements: BTreeMap<String, String>,
}

impl Plan {
    /// 図面の文書の版
    pub(crate) fn drawing(
        drawing: &Drawing2D,
        hash: String,
        cx: &mut Cx<'_>,
    ) -> Result<Self, Failed> {
        let mut elements = BTreeMap::new();
        zumen_drawing::id::each_element(drawing, |_, id, found| {
            let hash = found.hash_checked(cx)?;
            elements.insert(id, hash.to_string());
            Ok(())
        })
        .map_err(|stop: Stop| stopped(cx, stop))?;
        Ok(Plan { hash, elements })
    }
}

impl Version for Plan {
    fn hash(&self) -> String {
        self.hash.clone()
    }

    fn element(&self, id: &str) -> Option<String> {
        self.elements.get(id).cloned()
    }
}

fn state(change: Change) -> CarriedState {
    match change {
        Change::Unchanged => CarriedState::Unchanged,
        Change::Changed => CarriedState::Changed,
        Change::Removed => CarriedState::Removed,
    }
}

/// レビューの文書の対象を、文書の版へ移す。移した文書のバイト列も返す
pub(crate) fn carry(
    plan: Plan,
    review: &[u8],
    request: &ReviewCarryRequest,
    cx: &mut Cx<'_>,
) -> Result<(ReviewCarryResult, Vec<u8>), Failed> {
    let review = Review::read(review, cx).map_err(|_| Failed)?;
    let from = review
        .targets
        .iter()
        .find(|t| t.id == request.target)
        .map(|t| t.hash.clone())
        .unwrap_or_default();
    let (carried, references) =
        zumen_review::carry(&review, &request.target, &plan, cx).map_err(|_| Failed)?;
    let total = references.len() as u64;
    let start = request.offset.unwrap_or(0);
    let end = start.saturating_add(request.limit.unwrap_or(LIMIT));
    let page = references
        .into_iter()
        .skip(usize::try_from(start).unwrap_or(usize::MAX))
        .take(usize::try_from(end - start).unwrap_or(usize::MAX))
        .map(|reference| messages::CarriedReference {
            topic: reference.topic,
            location: reference.location,
            element: reference.element,
            state: state(reference.state),
        })
        .collect();
    Ok((
        ReviewCarryResult {
            from,
            hash: plan.hash,
            references: page,
            total,
            next_offset: (end < total).then_some(end),
        },
        carried.to_bytes(),
    ))
}

/// 照合の指摘（規則の ID と診断）のうち `chosen` の番号のものを、レビューの文書に取り込む。足した文書の
/// バイト列も返す
pub(crate) fn adopt(
    plan: &Plan,
    target: Target,
    review: Option<&[u8]>,
    findings: &[(String, &messages::Diagnostic)],
    chosen: &[u64],
    cx: &mut Cx<'_>,
) -> Result<(ReviewAdoptResult, Vec<u8>), Failed> {
    let review = match review {
        Some(bytes) => Review::read(bytes, cx).map_err(|_| Failed)?,
        None => Review::default(),
    };
    let mut adopted = Vec::with_capacity(chosen.len());
    for (index, number) in chosen.iter().enumerate() {
        let Some((rule, finding)) = usize::try_from(*number).ok().and_then(|n| findings.get(n))
        else {
            return Err(fail(cx, invalid_value(format!("/findings/{index}"))));
        };
        let mut out = Writer::new();
        finding.to_json(&mut out);
        let Ok(diagnostic) = zumen_json::parse(&out.into_bytes()) else {
            return Err(fail(cx, invalid_value(format!("/findings/{index}"))));
        };
        // 診断の場所のうち、要素の ID だけを参照にする。レイヤや用紙の ID は要素ではない
        let elements = finding
            .at
            .iter()
            .filter_map(|place| place.id.as_ref())
            .filter_map(|id| {
                plan.element(id).map(|hash| Reference {
                    id: id.clone(),
                    hash: Some(hash),
                })
            })
            .collect();
        adopted.push(Adopted {
            rule: rule.clone(),
            diagnostic,
            elements,
        });
    }
    let (review, added) =
        zumen_review::adopt(&review, &target, &adopted, cx).map_err(|_| Failed)?;
    Ok((
        ReviewAdoptResult {
            added,
            topics: review.topics.len() as u64,
        },
        review.to_bytes(),
    ))
}
