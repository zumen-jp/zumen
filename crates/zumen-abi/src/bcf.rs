//! `bcf_read`・`bcf_write` の op（決定 0220）。BCF 3.0 の bcfzip とレビューの文書（決定 0214）を互いに写し、
//! 写したものを blob で返す。

use zumen_base::Cx;
use zumen_bcf::{Bcf, Options, Statuses};
use zumen_review::{Review, Status, Target};

use crate::building::Model;
use crate::messages::{
    BcfReadRequest, BcfReadResult, BcfStatus, BcfWriteRequest, BcfWriteResult, ReviewStatus,
};
use crate::review::Plan;
use crate::session::Failed;

/// 状態の対応の表。書かなければ既定の表
fn statuses(table: Option<&[BcfStatus]>) -> Statuses {
    match table {
        Some(table) => Statuses(
            table
                .iter()
                .map(|row| {
                    let status = match row.status {
                        ReviewStatus::Open => Status::Open,
                        ReviewStatus::Resolved => Status::Resolved,
                        ReviewStatus::Closed => Status::Closed,
                    };
                    (row.text.clone(), status)
                })
                .collect(),
        ),
        None => Statuses::default(),
    }
}

/// bcfzip を読み、建物モデルを対象にしたレビューの文書にする。文書のバイト列も返す
pub(crate) fn read(
    model: &Model,
    bytes: &[u8],
    request: &BcfReadRequest,
    cx: &mut Cx<'_>,
) -> Result<(BcfReadResult, Vec<u8>), Failed> {
    let bcf = Bcf::read(bytes, cx).map_err(|_| Failed)?;
    let plan = Plan {
        hash: model.hash.to_string(),
        elements: model.hashes(cx)?,
    };
    let target = Target {
        id: request.target.clone(),
        format: "ifc".to_string(),
        hash: plan.hash.clone(),
        name: Some(request.doc.clone()),
        extra: Vec::new(),
    };
    let table = statuses(request.statuses.as_deref());
    let review = zumen_bcf::review(&bcf, &target, &table, Some(&plan), cx).map_err(|_| Failed)?;
    Ok((
        BcfReadResult {
            version: "3.0".to_string(),
            topics: review.topics.len() as u64,
        },
        review.to_bytes(),
    ))
}

/// レビューの文書を bcfzip に書く。`original` があれば、変えていない topic を元のまま書く
pub(crate) fn write(
    review: &[u8],
    original: Option<&[u8]>,
    request: &BcfWriteRequest,
    cx: &mut Cx<'_>,
) -> Result<(BcfWriteResult, Vec<u8>), Failed> {
    let review = Review::read(review, cx).map_err(|_| Failed)?;
    let original = match original {
        Some(bytes) => Some(Bcf::read(bytes, cx).map_err(|_| Failed)?),
        None => None,
    };
    let options = Options {
        statuses: statuses(request.statuses.as_deref()),
        target: request.target.clone(),
        kind: request.kind.clone(),
        author: request.author.clone(),
        date: request.date.clone(),
    };
    let bytes = zumen_bcf::write(&review, original.as_ref(), &options, cx).map_err(|_| Failed)?;
    Ok((
        BcfWriteResult {
            topics: review.topics.len() as u64,
        },
        bytes,
    ))
}
