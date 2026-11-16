//! 申請前の建物のチェックと、プロファイルに明記された面積の計測。
//!
//! 法令への適合、図書と IFC の一致、領域の境界を選んだ設計者の判断は判定しない。

mod alignment;
mod area;
mod check;

use zumen_base::{Cx, Diagnostic, Kind, Stop, codes};
use zumen_building::Building;
use zumen_json::{Value, Writer};

use crate::profile::{self, Resolved, text};
use crate::{Identity, Report};

pub use area::{AreaGroup, AreaGroupKind, AreaReport, GroupArea};

/// 建物のチェック・計測を始められなかった理由。部分結果を成功として返さない。
#[derive(Clone, Debug, PartialEq)]
pub enum Error {
    /// プロファイルを読めないか、継承を解けない。
    Profile(crate::Error),
    /// 要求の区分や要素などが使えない。
    Input(&'static str),
    /// 明示された予算または中断で止まった。
    Stopped(Stop),
    /// 形の評価または計測を始められない。
    Geometry(zumen_geom3d::Failure),
}

impl From<Stop> for Error {
    fn from(value: Stop) -> Self {
        Self::Stopped(value)
    }
}

/// 継承を解いたプロファイル。入力を借用せず、採用した鎖と規則のハッシュを保つ。
#[derive(Clone, Debug)]
pub struct BuildingProfile {
    resolved: Resolved,
    hash: [u8; 32],
    size: u64,
}

impl BuildingProfile {
    /// JSON を読み、既存の extends の置換規則で解く。配列・解析・継承の探索を先に数える。
    pub fn load(
        inputs: &[(&str, &[u8])],
        id: &str,
        edition: &str,
        cx: &mut Cx<'_>,
    ) -> Result<Self, Error> {
        cx.check(Kind::Work, inputs.len() as u64)?;
        let bytes = inputs
            .iter()
            .try_fold(
                id.len() as u64 + edition.len() as u64,
                |sum, (name, bytes)| sum.checked_add(bytes.len() as u64 + name.len() as u64 + 512),
            )
            .ok_or(Error::Input("profile_size"))?;
        // 汎用の JSON・継承の処理の前に、その木・複製・直列化と線形探索の上界を確保する。
        // 入力バイトそのものは resolve が数える。時計や実際の allocator の差を使わない。
        if bytes > 1024 * 1024 {
            return Err(Error::Input("profile_size"));
        }
        cx.check(Kind::DecodedBytes, bytes * 256)?;
        cx.check(Kind::Work, bytes * bytes + bytes)?;
        for (_, bytes) in inputs {
            json_depth(bytes, cx)?;
        }
        let resolved = profile::resolve(inputs, id, edition, cx).map_err(Error::Profile)?;
        let mut writer = Writer::new();
        writer.begin_array();
        writer.string("zumen-profile-building-1");
        for identity in &resolved.chain {
            writer.begin_array();
            writer.string(&identity.id);
            writer.string(&identity.edition);
            writer.uint(identity.revision);
            writer.end_array();
        }
        writer.map(&resolved.vocabularies, Writer::value);
        writer.begin_array();
        for (_, rule) in &resolved.rules {
            writer.value(rule);
        }
        writer.end_array();
        writer.end_array();
        let hash = *blake3::hash(&writer.into_bytes()).as_bytes();
        cx.check(Kind::Work, 0)?;
        Ok(Self {
            resolved,
            hash,
            size: bytes,
        })
    }

    /// 基から子への、適用した識別と版の鎖。
    pub fn chain(&self) -> &[Identity] {
        &self.resolved.chain
    }

    /// 継承後の規則・語彙と、識別・版・revision の BLAKE3。
    pub fn hash(&self) -> [u8; 32] {
        self.hash
    }

    /// 規則の順、モデルの順、要素の順にチェックする。
    /// 診断のモデル番号は入力の 1 始まりの位置。0 はモデルに限らない規則の設定を指す。
    pub fn check(&self, models: &[&Building], cx: &mut Cx<'_>) -> Result<Report, Error> {
        check::check(self, models, cx)
    }

    /// 呼び出し側が選んだ階・用途・建物の区分を、それぞれ合計してから丸める。
    /// 切る高さと領域の選択は呼び出し側が指定し、属性の値から推測しない。
    pub fn areas(
        &self,
        building: &Building,
        groups: &[AreaGroup<'_>],
        tolerance: zumen_geom3d::Tolerance,
        cx: &mut Cx<'_>,
    ) -> Result<AreaReport, Error> {
        area::measure(self, building, groups, tolerance, cx)
    }

    fn owned_chain(&self, cx: &mut Cx<'_>) -> Result<Vec<Identity>, Error> {
        let mut out = Vec::new();
        reserve(&mut out, self.chain().len(), 64, cx)?;
        for identity in self.chain() {
            cx.check(Kind::Work, 1)?;
            cx.check(
                Kind::DecodedBytes,
                (identity.id.len() + identity.edition.len()) as u64,
            )?;
            out.push(identity.clone());
        }
        Ok(out)
    }

    fn plan_work(&self, cx: &mut Cx<'_>) -> Result<(), Error> {
        // 語彙・規則のフィールド探索と、文字列の配列の検査を入力長から先に数える。
        cx.check(Kind::Work, self.size * 64)?;
        Ok(())
    }
}

fn reserve<T>(
    values: &mut Vec<T>,
    additional: usize,
    slot: u64,
    cx: &mut Cx<'_>,
) -> Result<(), Error> {
    let count = values
        .len()
        .checked_add(additional)
        .ok_or(Error::Input("profile_size"))?;
    let max = i32::MAX as usize / slot as usize;
    if count > max {
        return Err(Error::Input("profile_size"));
    }
    if count > values.capacity() {
        let capacity = count.max(values.capacity().saturating_mul(2)).clamp(4, max);
        cx.check(
            Kind::DecodedBytes,
            (capacity - values.capacity()) as u64 * slot,
        )?;
        values
            .try_reserve_exact(capacity - values.len())
            .map_err(|_| Error::Input("allocation"))?;
    }
    Ok(())
}

fn strings(value: Option<&Value>) -> Option<&[Value]> {
    match value? {
        Value::Array(values) if values.iter().all(|v| text(v).is_some()) => Some(values),
        _ => None,
    }
}

// 汎用の JSON パーサーへ渡す前に、文字列の外側の入れ子を同じ Cx で数える。
// 文法と引用符の妥当性は後のパーサーが確かめる。走査の work は load で確保済み。
fn json_depth(bytes: &[u8], cx: &mut Cx<'_>) -> Result<(), Error> {
    let (mut depth, mut string, mut escape) = (0u64, false, false);
    let result = (|| {
        for byte in bytes {
            if string {
                if escape {
                    escape = false;
                } else if *byte == b'\\' {
                    escape = true;
                } else if *byte == b'"' {
                    string = false;
                }
            } else {
                match byte {
                    b'"' => string = true,
                    b'[' | b'{' => {
                        cx.enter()?;
                        depth += 1;
                    }
                    b']' | b'}' if depth > 0 => {
                        cx.leave();
                        depth -= 1;
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    })();
    for _ in 0..depth {
        cx.leave();
    }
    result
}

fn vocabulary_error(vocabulary: &str, field: &str) -> Error {
    Error::Profile(crate::Error::Profile(Box::new(
        Diagnostic::new(codes::PROFILE_VOCABULARY_INVALID)
            .with("vocabulary", vocabulary)
            .with("field", field),
    )))
}
