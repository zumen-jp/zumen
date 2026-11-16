//! 元の八分木の節から ZMS3 の点の列を作る。キーと点の順を保ち、座標だけ東・北・高さにする。
use crate::{MAX_LEVEL, Node, PointCloud, Query};
use zumen_base::{Cx, Diagnostic, Kind, codes};
use zumen_scene3d::{Bounds, Chunk, Data, Error, PointChunk, PointData};

/// 点群のシーン。点の行番号は、この結果の中だけで有効。
#[derive(Clone, Debug, PartialEq)]
pub struct Scene {
    /// 共通の塊。メッシュと要素の列は空
    pub data: Data,
    /// 点と元の節への参照
    pub points: PointData,
    /// 件数の上限で省いた点がある
    pub truncated: bool,
    /// 返したもっとも深い段。空なら 0
    pub level: u8,
    /// 返した座標の東・北・高さの範囲
    pub extent: Option<Bounds>,
}

fn reserve<T>(count: usize, slot: u64, cx: &mut Cx) -> Result<Vec<T>, Error> {
    let bytes = (count as u64).saturating_mul(slot);
    if bytes > zumen_scene3d::MAX_BYTES {
        return Err(Error::Invalid("size"));
    }
    cx.check(Kind::DecodedBytes, bytes)?;
    let mut out = Vec::new();
    out.try_reserve_exact(count)
        .map_err(|_| Error::Invalid("allocation"))?;
    Ok(out)
}

fn position(cloud: &PointCloud, index: u32) -> Result<[f64; 3], Error> {
    let points = &cloud.points;
    let i = index as usize;
    Ok([
        *points
            .easting
            .get(i)
            .ok_or(Error::Invalid("point_reference"))?,
        *points
            .northing
            .get(i)
            .ok_or(Error::Invalid("point_reference"))?,
        *points
            .height
            .get(i)
            .ok_or(Error::Invalid("point_reference"))?,
    ])
}

fn union(bounds: &mut Option<Bounds>, point: [f64; 3]) {
    match bounds {
        None => *bounds = Some([point, point]),
        Some([low, high]) => {
            for i in 0..3 {
                if let (Some(l), Some(h), Some(p)) = (low.get_mut(i), high.get_mut(i), point.get(i))
                {
                    *l = l.min(*p);
                    *h = h.max(*p);
                }
            }
        }
    }
}

fn inside(point: [f64; 3], bounds: Option<Bounds>) -> bool {
    // Query は北・東・高さ、scene3d の列は東・北・高さ。
    bounds.is_none_or(|[low, high]| {
        let [e, n, h] = point;
        [n, e, h]
            .into_iter()
            .zip(low.into_iter().zip(high))
            .all(|(p, (l, h))| l <= p && p <= h)
    })
}

/// 同じ Query と元の節で選ぶ。座標系と高さ基準を変換せず、メートルの元値を使う。
pub fn build(cloud: &PointCloud, query: &Query, cx: &mut Cx) -> Result<Scene, Error> {
    build_colored(cloud, query, None, cx)
}

/// [`build`] と同じに選び、点の色を `colors`（点群の点の順、16 ビットの赤・緑・青・不透明度）にする。
/// `colors` が `None` なら元の色を使う
pub fn build_colored(
    cloud: &PointCloud,
    query: &Query,
    colors: Option<&[[u16; 4]]>,
    cx: &mut Cx,
) -> Result<Scene, Error> {
    if query.spacing.is_some_and(|v| !v.is_finite() || v <= 0.0)
        || query.bounds.is_some_and(|[low, high]| {
            low.into_iter()
                .zip(high)
                .any(|(l, h)| !l.is_finite() || !h.is_finite() || l > h)
        })
    {
        return Err(Error::Invalid("selection"));
    }
    let n = cloud.points.len();
    let p = &cloud.points;
    if n > u32::MAX as usize
        || p.easting.len() != n
        || p.height.len() != n
        || p.color.as_ref().is_some_and(|v| v.len() != n)
        || colors.is_some_and(|v| v.len() != n)
        || p.intensity.as_ref().is_some_and(|v| v.len() != n)
        || p.classification.as_ref().is_some_and(|v| v.len() != n)
        || p.returns.as_ref().is_some_and(|v| v.len() != n)
        || p.time.as_ref().is_some_and(|v| v.len() != n)
    {
        return Err(Error::Invalid("point_columns"));
    }
    let tree = &cloud.octree;
    cx.check(
        Kind::Work,
        (n as u64)
            .saturating_mul(12)
            .saturating_add(tree.nodes.len() as u64 * 32 + 1),
    )?;
    let mut seen = reserve(n, 1, cx)?;
    seen.resize(n, false);
    let mut nodes = reserve(tree.nodes.len(), 16, cx)?;
    nodes.extend(tree.nodes.iter());
    nodes.sort_unstable_by_key(|node: &&Node| node.key);
    if n != 0
        && (tree.span == 0
            || !tree.size.is_finite()
            || tree.size <= 0.0
            || tree.origin.iter().any(|v| !v.is_finite()))
    {
        return Err(Error::Invalid("octree"));
    }
    let mut previous = None;
    let mut members = 0_u64;
    for node in &nodes {
        cx.check(Kind::Work, node.points.len() as u64 + 1)?;
        let key = node.key;
        if key.level > MAX_LEVEL || previous == Some(key) || node.points.is_empty() {
            return Err(Error::Invalid("point_node"));
        }
        let width = 1_u32 << key.level;
        if [key.x, key.y, key.z].into_iter().any(|v| v >= width) {
            return Err(Error::Invalid("point_node"));
        }
        previous = Some(key);
        members = members.saturating_add(node.points.len() as u64);
        if members > n as u64 {
            return Err(Error::Invalid("point_reference"));
        }
        for &index in &node.points {
            let used = seen
                .get_mut(index as usize)
                .ok_or(Error::Invalid("point_reference"))?;
            if *used {
                return Err(Error::Invalid("point_reference"));
            }
            *used = true;
            if position(cloud, index)?.iter().any(|v| !v.is_finite()) {
                return Err(Error::Invalid("coordinate"));
            }
        }
    }
    if members != n as u64 {
        return Err(Error::Invalid("point_reference"));
    }
    let capacity =
        usize::try_from(query.limit.min(n as u64)).map_err(|_| Error::Invalid("size"))?;
    let mut out = Scene {
        data: Data {
            chunks: reserve(nodes.len().min(capacity), 64, cx)?,
            ..Data::default()
        },
        points: PointData {
            chunks: reserve(nodes.len().min(capacity), 32, cx)?,
            positions: reserve(capacity, 12, cx)?,
            rgba: (p.color.is_some() || colors.is_some())
                .then(|| reserve(capacity, 8, cx))
                .transpose()?,
            intensity: p
                .intensity
                .as_ref()
                .map(|_| reserve(capacity, 2, cx))
                .transpose()?,
            classification: p
                .classification
                .as_ref()
                .map(|_| reserve(capacity, 1, cx))
                .transpose()?,
        },
        truncated: false,
        level: 0,
        extent: None,
    };
    let mut error = 0.0_f64;
    for node in nodes {
        cx.check(Kind::Work, node.points.len() as u64 * 2 + 1)?;
        if query.spacing.is_some_and(|spacing| {
            node.key.level != 0 && tree.spacing(node.key.level - 1) <= spacing
        }) {
            continue;
        }
        let start = out.points.positions.len();
        let mut count = 0;
        let mut bounds = None;
        for &index in &node.points {
            let point = position(cloud, index)?;
            if !inside(point, query.bounds) {
                continue;
            }
            if start + count >= capacity {
                out.truncated = true;
                break;
            }
            union(&mut bounds, point);
            count += 1;
        }
        let Some([low, high]) = bounds else {
            continue;
        };
        let origin = std::array::from_fn(|i| {
            low.get(i).copied().unwrap_or(0.0) * 0.5 + high.get(i).copied().unwrap_or(0.0) * 0.5
        });
        let mut local_bounds = None;
        for &index in &node.points {
            let point = position(cloud, index)?;
            if !inside(point, query.bounds) {
                continue;
            }
            if out.points.positions.len() - start >= count {
                break;
            }
            cx.check(Kind::Elements, 1)?;
            let mut local = [0.0_f32; 3];
            for ((value, &v), &o) in local.iter_mut().zip(&point).zip(&origin) {
                *value = (v - o) as f32;
                if !value.is_finite() {
                    return Err(Error::Invalid("f32_range"));
                }
                if *value == 0.0 {
                    *value = 0.0;
                }
                error = error.max((o + f64::from(*value) - v).abs());
            }
            union(&mut local_bounds, local.map(f64::from));
            union(
                &mut out.extent,
                std::array::from_fn(|i| {
                    origin.get(i).copied().unwrap_or(0.0)
                        + local.get(i).copied().map(f64::from).unwrap_or(0.0)
                }),
            );
            out.points.positions.push(local);
            if let Some(target) = &mut out.points.rgba {
                let rgba = match (colors, &p.color) {
                    (Some(colors), _) => colors.get(index as usize).copied(),
                    (None, Some(source)) => source
                        .get(index as usize)
                        .map(|&[r, g, b]| [r, g, b, u16::MAX]),
                    (None, None) => None,
                };
                target.push(rgba.ok_or(Error::Invalid("point_reference"))?);
            }
            if let (Some(source), Some(target)) = (&p.intensity, &mut out.points.intensity) {
                target.push(
                    *source
                        .get(index as usize)
                        .ok_or(Error::Invalid("point_reference"))?,
                );
            }
            if let (Some(source), Some(target)) =
                (&p.classification, &mut out.points.classification)
            {
                target.push(
                    *source
                        .get(index as usize)
                        .ok_or(Error::Invalid("point_reference"))?,
                );
            }
        }
        let [low, high] = local_bounds.ok_or(Error::Invalid("bounds"))?;
        let [x, y, z] = low;
        let [u, v, w] = high;
        out.data.chunks.push(Chunk {
            origin,
            bounds: [x as f32, y as f32, z as f32, u as f32, v as f32, w as f32],
            instances: 0,
        });
        let key = node.key;
        out.points.chunks.push(PointChunk {
            start: start as u32,
            node: Some([u32::from(key.level), key.x, key.y, key.z]),
        });
        out.level = out.level.max(key.level);
    }
    if error > 0.0 {
        cx.report(
            Diagnostic::new(codes::SCENE3D_COORDINATE_APPROXIMATED)
                .with("error", error)
                .at(cloud.source.clone()),
        );
    }
    cx.check(Kind::Work, 0)?;
    Ok(out)
}
