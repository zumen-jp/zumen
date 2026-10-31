//! 指摘を、対象の新しい版へ引き継ぐ（決定 0218）。

use zumen_base::{Diagnostic, Kind, codes};
use zumen_sdk::{Cx, Error};

use crate::{Carried, Change, Place, Reference, Review};

/// 対象の新しい版。要素を ID で引き、要素のハッシュを返す
pub trait Version {
    /// 版の全体のハッシュ
    fn hash(&self) -> String;
    /// ID の要素のハッシュ。要素がなければ `None`
    fn element(&self, id: &str) -> Option<String>;
}

/// 引き継いだ要素の参照 1 つ
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CarriedReference {
    /// トピックの ID
    pub topic: String,
    /// 場所の ID
    pub location: String,
    /// 要素の ID
    pub element: String,
    /// 状態
    pub state: Change,
}

/// `target` の ID の対象を `version` へ移した文書と、要素の参照ごとの状態を返す。
///
/// 要素が新しい版になければ「なくなった」、ハッシュが違うか元のハッシュがなければ「変わった」、
/// 同じなら「変わっていない」とする。変わった参照は新しいハッシュに書き換え、なくなった参照は元の
/// ハッシュのまま残す。場所と図形には、元の版のハッシュと、まとめた状態を `carried` に書く。対象が
/// 文書になければ `review.target.missing` を出して失敗する
pub fn carry(
    review: &Review,
    target: &str,
    version: &dyn Version,
    cx: &mut Cx,
) -> Result<(Review, Vec<CarriedReference>), Error> {
    let mut review = review.clone();
    let Some(entry) = review.targets.iter_mut().find(|t| t.id == target) else {
        cx.report(Diagnostic::new(codes::REVIEW_TARGET_MISSING).with("target", target));
        return Err(Error::Invalid);
    };
    let from = core::mem::replace(&mut entry.hash, version.hash());
    let mut carried = Vec::new();
    for topic in &mut review.topics {
        for location in topic.locations.iter_mut().filter(|l| l.target == target) {
            let mut states = Vec::new();
            for reference in references(&mut location.place) {
                cx.check(Kind::Work, 1).map_err(|stop| {
                    cx.report(stop.to_diagnostic());
                    Error::Stopped(stop)
                })?;
                let state = match version.element(&reference.id) {
                    None => Change::Removed,
                    Some(hash) if reference.hash.as_ref() == Some(&hash) => Change::Unchanged,
                    Some(hash) => {
                        reference.hash = Some(hash);
                        Change::Changed
                    }
                };
                states.push(state);
                carried.push(CarriedReference {
                    topic: topic.id.clone(),
                    location: location.id.clone(),
                    element: reference.id.clone(),
                    state,
                });
            }
            location.carried = Some(Carried {
                from: from.clone(),
                state: summary(&states),
            });
        }
    }
    // 図形は要素を指さないので、元の版から変わっていないとする
    for markup in review.markups.iter_mut().filter(|m| m.target == target) {
        markup.carried = Some(Carried {
            from: from.clone(),
            state: Change::Unchanged,
        });
    }
    Ok((review, carried))
}

/// すべてなくなったら「なくなった」、1 つでも変わるかなくなったら「変わった」
fn summary(states: &[Change]) -> Change {
    if !states.is_empty() && states.iter().all(|s| *s == Change::Removed) {
        Change::Removed
    } else if states.iter().any(|s| *s != Change::Unchanged) {
        Change::Changed
    } else {
        Change::Unchanged
    }
}

fn references(place: &mut Place) -> Vec<&mut Reference> {
    match place {
        Place::Plane { elements, .. } => elements.iter_mut().collect(),
        Place::Space {
            selected,
            exceptions,
            colored,
            ..
        } => selected
            .iter_mut()
            .chain(exceptions.iter_mut())
            .chain(colored.iter_mut().flat_map(|c| c.elements.iter_mut()))
            .collect(),
    }
}
