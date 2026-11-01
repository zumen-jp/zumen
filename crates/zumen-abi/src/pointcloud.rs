//! 点群（LAS・LAZ）の `open` と `pointcloud` の op（決定 0232）。
//!
//! 入力は範囲で読む。`open` の入力を全体の初めの部分とし、`<入力の名前>#<start>-<end>` の名前で渡された入力を
//! 範囲の表に足す。足りない範囲は `needs` で返す。

use std::collections::BTreeMap;

use zumen_base::{ByteRange, Cx};
use zumen_las::Las;
use zumen_ply::Ply;
use zumen_pointcloud::{PointCloud, Query, select};
use zumen_sdk::ranged::{Missing, Ranged, Source};
use zumen_sdk::{Codec, Lift};
use zumen_xyz::Xyz;

use crate::messages::{
    InputRange, PointcloudBox, PointcloudPoints, PointcloudRequest, PointcloudResult,
};
use crate::session::{Failed, fail, invalid_value, stopped};

/// 開いた結果
pub(crate) enum Opened {
    /// 点群と、読み終えた範囲の入力
    Cloud(Box<PointCloud>, Vec<InputRange>),
    /// 足りない範囲
    Need(Vec<InputRange>),
}

/// 範囲の入力の名前
fn range_name(input: &str, range: ByteRange) -> String {
    format!("{input}#{}-{}", range.start, range.end)
}

/// LAS・LAZ なら、範囲の表から読み、点群の領域モデルに移す。ほかの形式なら `None`
pub(crate) fn read(
    input: &str,
    inputs: &BTreeMap<String, Vec<u8>>,
    size: Option<u64>,
    cx: &mut Cx<'_>,
) -> Option<Result<Opened, Failed>> {
    let first = inputs.get(input)?;
    if !<Las as Ranged>::SIGNATURES
        .iter()
        .any(|rule| rule.matches(first))
    {
        return None;
    }
    let prefix = format!("{input}#");
    let mut parts: Vec<(u64, &[u8])> = vec![(0, first.as_slice())];
    let mut ranges = Vec::new();
    for (name, bytes) in inputs.range(prefix.clone()..) {
        let Some(rest) = name.strip_prefix(&prefix) else {
            break;
        };
        let Some((start, end)) = rest
            .split_once('-')
            .and_then(|(start, end)| Some((start.parse().ok()?, end.parse().ok()?)))
        else {
            continue;
        };
        parts.push((start, bytes.as_slice()));
        ranges.push(InputRange {
            input: name.clone(),
            start,
            end,
        });
    }
    parts.sort_by_key(|(start, _)| *start);
    let source = Source {
        size: size.unwrap_or(0).max(first.len() as u64),
        parts: &parts,
    };
    // 失敗の理由は、コーデックと移す処理が診断として足してある
    Some(match Las::read_ranged(&source, cx) {
        Ok(file) => file
            .lift(cx)
            .map(|cloud| Opened::Cloud(Box::new(cloud), ranges))
            .map_err(|_| Failed),
        Err(Missing::Need(needs)) => Ok(Opened::Need(
            needs
                .into_iter()
                .map(|range| InputRange {
                    input: range_name(input, range),
                    start: range.start,
                    end: range.end,
                })
                .collect(),
        )),
        Err(Missing::Failed(_)) => Err(Failed),
    })
}

/// 全体を渡す点群の形式（PLY と、名指した XYZ）なら読み、点群の領域モデルに移す。ほかの形式なら `None`
pub(crate) fn read_whole(
    bytes: &[u8],
    format: Option<&str>,
    cx: &mut Cx<'_>,
) -> Option<Result<(&'static str, PointCloud), Failed>> {
    fn open<C: Codec>(bytes: &[u8], cx: &mut Cx<'_>) -> Result<(&'static str, PointCloud), Failed>
    where
        C::File: Lift<PointCloud>,
    {
        // 失敗の理由は、コーデックと移す処理が診断として足してある
        C::read(bytes, cx)
            .and_then(|file| file.lift(cx))
            .map(|cloud| (C::FORMAT, cloud))
            .map_err(|_| Failed)
    }
    if format == Some(Xyz::FORMAT) {
        return Some(open::<Xyz>(bytes, cx));
    }
    if Ply::SIGNATURES.iter().any(|rule| rule.matches(bytes)) {
        return Some(open::<Ply>(bytes, cx));
    }
    None
}

/// 箱の欄を、北・東・標高の最小と最大にする
fn corners(region: &PointcloudBox, cx: &mut Cx<'_>) -> Result<[[f64; 3]; 2], Failed> {
    let corner = |values: &[f64]| match values {
        [n, e, h] if [n, e, h].iter().all(|v| v.is_finite()) => Some([*n, *e, *h]),
        _ => None,
    };
    let Some(low) = corner(&region.min) else {
        return Err(fail(cx, invalid_value("/region/min")));
    };
    let Some(high) = corner(&region.max) else {
        return Err(fail(cx, invalid_value("/region/max")));
    };
    Ok([low, high])
}

fn to_box([low, high]: [[f64; 3]; 2]) -> PointcloudBox {
    PointcloudBox {
        min: low.to_vec(),
        max: high.to_vec(),
    }
}

/// 点群の要約と、条件に合う点
pub(crate) fn pointcloud(
    request: &PointcloudRequest,
    cloud: &PointCloud,
    cx: &mut Cx<'_>,
) -> Result<PointcloudResult, Failed> {
    let bounds = match &request.region {
        Some(region) => Some(corners(region, cx)?),
        None => None,
    };
    if request
        .tolerance
        .is_some_and(|tolerance| !(tolerance.is_finite() && tolerance > 0.0))
    {
        return Err(fail(cx, invalid_value("/tolerance")));
    }
    let points = &cloud.points;
    let selected = match request.limit {
        None => None,
        Some(limit) => {
            let query = Query {
                bounds,
                spacing: request.tolerance,
                limit,
            };
            let chosen = select(cloud, &query, cx).map_err(|stop| stopped(cx, stop))?;
            let at = |index: &u32| *index as usize;
            Some(PointcloudPoints {
                positions: chosen
                    .points
                    .iter()
                    .filter_map(|i| {
                        let i = at(i);
                        Some(vec![
                            *points.northing.get(i)?,
                            *points.easting.get(i)?,
                            *points.height.get(i)?,
                        ])
                    })
                    .collect(),
                intensity: points.intensity.as_ref().map(|values| {
                    chosen
                        .points
                        .iter()
                        .filter_map(|i| values.get(at(i)).map(|v| u64::from(*v)))
                        .collect()
                }),
                classification: points.classification.as_ref().map(|values| {
                    chosen
                        .points
                        .iter()
                        .filter_map(|i| values.get(at(i)).map(|v| u64::from(*v)))
                        .collect()
                }),
                color: points.color.as_ref().map(|colors| {
                    chosen
                        .points
                        .iter()
                        .filter_map(|i| colors.get(at(i)))
                        .map(|rgb| rgb.iter().map(|v| u64::from(*v)).collect())
                        .collect()
                }),
                truncated: chosen.truncated,
                level: u64::from(chosen.level),
            })
        }
    };
    Ok(PointcloudResult {
        points: points.len() as u64,
        bounds: points.bounds().map(to_box),
        crs: cloud.crs.map(|crs| u64::from(crs.epsg())),
        vertical: cloud.vertical.map(|vertical| u64::from(vertical.epsg())),
        attributes: points
            .attributes()
            .into_iter()
            .map(str::to_string)
            .collect(),
        depth: u64::from(cloud.octree.depth()),
        nodes: cloud.octree.nodes.len() as u64,
        selected,
    })
}
