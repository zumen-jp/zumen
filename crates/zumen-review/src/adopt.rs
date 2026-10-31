//! 規約の検査の指摘を、トピックとして取り込む（決定 0218）。

use zumen_base::{Diagnostic, Kind, codes};
use zumen_json::{Value, Writer};
use zumen_sdk::{Cx, Error};

use crate::{Location, Origin, Place, Reference, Review, Status, Target, Topic};

/// 取り込む指摘 1 つ
#[derive(Clone, Debug, PartialEq)]
pub struct Adopted {
    /// 規則の ID
    pub rule: String,
    /// 境界の形の診断（決定 0218）
    pub diagnostic: Value,
    /// 指摘の要素
    pub elements: Vec<Reference>,
}

/// `findings` を、`target` を指す開いたトピックにして足した文書と、足したトピックの ID を返す。
///
/// トピックの ID は、対象のハッシュ・規則・診断と、同じ組の何番目かから決まる UUID（版 8）。同じ
/// ID のトピックが文書にあれば、取り込み済みとして足さない。対象が文書になければ足す。同じ ID の対象の
/// 形式かハッシュが違えば、`review.target.stale` を出して何も足さない
pub fn adopt(
    review: &Review,
    target: &Target,
    findings: &[Adopted],
    cx: &mut Cx,
) -> Result<(Review, Vec<String>), Error> {
    let mut review = review.clone();
    match review.targets.iter().find(|t| t.id == target.id) {
        Some(entry) if entry.format != target.format || entry.hash != target.hash => {
            cx.report(
                Diagnostic::new(codes::REVIEW_TARGET_STALE).with("target", target.id.as_str()),
            );
            return Err(Error::Rejected);
        }
        Some(_) => {}
        None => review.targets.push(target.clone()),
    }
    let mut added = Vec::new();
    let mut seen: Vec<(&str, Vec<u8>)> = Vec::new();
    for finding in findings {
        cx.check(Kind::Work, 1).map_err(|stop| {
            cx.report(stop.to_diagnostic());
            Error::Stopped(stop)
        })?;
        let mut out = Writer::new();
        out.value(&finding.diagnostic);
        let diagnostic = out.into_bytes();
        let ordinal = seen
            .iter()
            .filter(|(rule, bytes)| *rule == finding.rule && *bytes == diagnostic)
            .count() as u64;
        let id = uuid(&target.hash, &finding.rule, &diagnostic, ordinal);
        seen.push((&finding.rule, diagnostic));
        if review.topics.iter().any(|t| t.id == id) {
            continue;
        }
        let title = match finding.diagnostic.pointer("/code") {
            Some(Value::String(code)) => format!("{}: {code}", finding.rule),
            _ => finding.rule.clone(),
        };
        review.topics.push(Topic {
            id: id.clone(),
            title,
            description: None,
            status: Status::Open,
            status_label: None,
            kind: None,
            priority: None,
            labels: Vec::new(),
            assignee: None,
            due: None,
            author: None,
            created: None,
            modified_by: None,
            modified: None,
            related: Vec::new(),
            comments: Vec::new(),
            locations: vec![Location {
                id: format!("{id}/l1"),
                target: target.id.clone(),
                place: Place::Plane {
                    view: None,
                    pin: None,
                    extent: None,
                    elements: finding.elements.clone(),
                },
                carried: None,
                extra: Vec::new(),
            }],
            origin: Some(Origin {
                rule: finding.rule.clone(),
                diagnostic: finding.diagnostic.clone(),
            }),
            extra: Vec::new(),
        });
        added.push(id);
    }
    Ok((review, added))
}

/// 長さを前に置いた欄の BLAKE3 から作る UUID（版 8、RFC 9562）
fn uuid(hash: &str, rule: &str, diagnostic: &[u8], ordinal: u64) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"zumen-review/topic\0");
    for field in [hash.as_bytes(), rule.as_bytes(), diagnostic] {
        hasher.update(&(field.len() as u64).to_le_bytes());
        hasher.update(field);
    }
    hasher.update(&ordinal.to_le_bytes());
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&hasher.finalize().as_bytes()[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x80;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let mut text = String::with_capacity(36);
    for (index, byte) in bytes.iter().enumerate() {
        if matches!(index, 4 | 6 | 8 | 10) {
            text.push('-');
        }
        text.push_str(&format!("{byte:02x}"));
    }
    text
}
