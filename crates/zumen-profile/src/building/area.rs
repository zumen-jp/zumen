//! 領域の選択と区分は利用側が渡す。単位・境界・丸めの指定はプロファイルから読む。

use super::{BuildingProfile, Error, reserve, strings, vocabulary_error};
use crate::profile::{GetKey, text};
use crate::{Finding, Identity};
use zumen_base::{Cx, Diagnostic, Kind, Severity, codes};
use zumen_building::{Building, ElementRef, measure};
use zumen_crs::ProjectPoint;
use zumen_geom3d::Tolerance;

/// 面積を合計する区分。階と用途の結果を互いに加算しない。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AreaGroupKind {
    /// 階別。階の識別は key で明示する。
    Storey,
    /// 用途別。同じ階で用途ごとに key を分ける。
    Use,
    /// 建築面積。地階や軒等を除く判断は確認していないと返す。
    Building,
}
impl AreaGroupKind {
    fn name(self) -> &'static str {
        match self {
            Self::Storey => "storey",
            Self::Use => "use",
            Self::Building => "building",
        }
    }
}

/// 呼び出し側が同じ高さで測る領域を、階・用途・建物で分けた入力。
#[derive(Clone, Copy, Debug)]
pub struct AreaGroup<'a> {
    /// 合計する種類。
    pub kind: AreaGroupKind,
    /// 利用側が定める区分の識別。同じ種類では一意。
    pub key: &'a str,
    /// この区分の領域。別の区分との重複は同じ種類の中で拒む。
    pub elements: &'a [ElementRef],
    /// 切る原点。文書単位のプロジェクト座標。
    pub origin: ProjectPoint,
}

/// 1 区分の元の値と、プロファイルで丸めた値。
#[derive(Clone, Debug, PartialEq)]
pub struct GroupArea {
    /// 入力の区分の種類。
    pub kind: AreaGroupKind,
    /// 入力の区分の識別。
    pub key: String,
    /// 丸め前の値・合計・十進の結果・条件・出所。
    pub measurement: measure::Report,
}

/// 区分の順の面積と、確認していない判断。
#[derive(Clone, Debug)]
pub struct AreaReport {
    /// 適用したプロファイルの鎖。
    pub chain: Vec<Identity>,
    /// 区分ごとの結果。階別と用途別を足した合計は作らない。
    pub groups: Vec<GroupArea>,
    /// 建築面積の除外など、確認していない項目。
    pub findings: Vec<Finding>,
}

pub(super) fn measure(
    profile: &BuildingProfile,
    building: &Building,
    groups: &[AreaGroup<'_>],
    tolerance: Tolerance,
    cx: &mut Cx<'_>,
) -> Result<AreaReport, Error> {
    profile.plan_work(cx)?;
    let name = "building-area";
    let invalid = |field| vocabulary_error(name, field);
    let Some(values) = profile
        .resolved
        .vocabularies
        .get(name)
        .and_then(|v| v.get_key("values"))
    else {
        return Err(invalid("values"));
    };
    let zumen_json::Value::Array(values) = values else {
        return Err(invalid("values"));
    };
    let [value] = values.as_slice() else {
        return Err(invalid("values"));
    };
    let field = |key| value.get_key(key).and_then(text);
    if field("boundary") != Some("shape") {
        return Err(invalid("boundary"));
    }
    if field("unit") != Some("m2") {
        return Err(invalid("unit"));
    }
    if field("stage") != Some("total") {
        return Err(invalid("stage"));
    }
    let rounding = match field("rounding") {
        Some("truncate") => measure::RoundingMode::Truncate,
        Some("floor") => measure::RoundingMode::Floor,
        Some("ceiling") => measure::RoundingMode::Ceiling,
        Some("half-away") => measure::RoundingMode::HalfAway,
        Some("half-even") => measure::RoundingMode::HalfEven,
        _ => return Err(invalid("rounding")),
    };
    let digits = value
        .get_key("digits")
        .and_then(crate::number)
        .filter(|v| (-18.0..=18.0).contains(v) && libm::trunc(*v) == *v)
        .ok_or_else(|| invalid("digits"))? as i8;
    let allowed = strings(value.get_key("groups")).ok_or_else(|| invalid("groups"))?;
    let identity = profile.chain().last().ok_or_else(|| invalid("profile"))?;
    if groups.is_empty() {
        return Err(Error::Input("area_groups"));
    }
    let mut out = AreaReport {
        chain: profile.owned_chain(cx)?,
        groups: Vec::new(),
        findings: Vec::new(),
    };
    reserve(&mut out.groups, groups.len(), 512, cx)?;
    for (index, group) in groups.iter().enumerate() {
        profile.plan_work(cx)?;
        cx.check(Kind::Work, 1)?;
        if group.key.is_empty() || !allowed.iter().any(|v| text(v) == Some(group.kind.name())) {
            return Err(Error::Input("area_group"));
        }
        // 区分の重複を計算前に拒む。同じ領域を階別と用途別に使うことは認める。
        for previous in groups.iter().take(index) {
            cx.check(Kind::Work, 1)?;
            if previous.kind != group.kind {
                continue;
            }
            cx.check(
                Kind::Work,
                (previous.key.len() + group.key.len() + 1) as u64,
            )?;
            if previous.key == group.key {
                return Err(Error::Input("area_group_duplicate"));
            }
            for element in group.elements {
                cx.check(Kind::Work, previous.elements.len() as u64)?;
                if previous.elements.contains(element) {
                    return Err(Error::Input("area_element_duplicate"));
                }
            }
        }
        let measurement = measure::areas(
            building,
            group.elements,
            measure::Area {
                origin: group.origin,
                boundary: measure::Boundary::Shape,
                input: measure::InputBoundary::Auto,
                walls: &[],
            },
            measure::Options {
                unit: measure::Unit::Area(measure::LengthUnit::Meter),
                rounding: measure::Rounding {
                    mode: rounding,
                    digits,
                    stage: measure::RoundingStage::Total,
                },
                tolerance,
                profile: Some(measure::Profile {
                    id: &identity.id,
                    version: &identity.edition,
                    hash: profile.hash(),
                }),
            },
            cx,
        )
        .map_err(|error| match error {
            zumen_geom3d::Failure::Stopped(stop) => Error::Stopped(stop),
            error => Error::Geometry(error),
        })?;
        cx.check(Kind::DecodedBytes, group.key.len() as u64)?;
        out.groups.push(GroupArea {
            kind: group.kind,
            key: group.key.into(),
            measurement,
        });
        if group.kind == AreaGroupKind::Building {
            let reasons = profile
                .resolved
                .vocabularies
                .get("building-area-unverified")
                .and_then(|v| strings(v.get_key("values")))
                .filter(|v| !v.is_empty())
                .ok_or_else(|| vocabulary_error("building-area-unverified", "values"))?;
            for reason in reasons {
                let Some(reason) = text(reason).filter(|v| !v.is_empty()) else {
                    return Err(vocabulary_error("building-area-unverified", "values"));
                };
                cx.check(Kind::Work, 1)?;
                cx.check(
                    Kind::DecodedBytes,
                    (group.key.len() + reason.len() + 512) as u64,
                )?;
                reserve(&mut out.findings, 1, 256, cx)?;
                out.findings.push(Finding {
                    rule: "building-area".into(),
                    level: Severity::Info,
                    diagnostic: Diagnostic::new(codes::PROFILE_BUILDING_AREA_UNSUPPORTED)
                        .with("rule", "building-area")
                        .with("group", group.key)
                        .with("reason", reason),
                });
            }
        }
    }
    cx.check(Kind::Work, 0)?;
    Ok(out)
}
