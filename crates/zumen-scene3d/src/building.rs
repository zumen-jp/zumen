//! 建物の選択、許容差の決定と列へ渡す形の生成。

use crate::{Data, Error, packing, selection};
use zumen_base::{Cx, Diagnostic, Diagnostics, Kind, codes};
use zumen_building::{Building, Id, evaluation};

/// プロジェクト空間の文書単位の最小点と最大点。
pub type Bounds = [[f64; 3]; 2];

/// 建物のシーンを作る条件。各選択条件の中は和、条件どうしは積。
#[derive(Clone, Copy, Debug)]
pub struct Options<'a> {
    /// 階の ID。空の配列は何も選ばない
    pub storeys: Option<&'a [Id]>,
    /// クラス名。大文字・小文字を区別する
    pub classes: Option<&'a [String]>,
    /// 要素の ID
    pub ids: Option<&'a [Id]>,
    /// 重なりで選ぶ箱。形の切り抜きは読み手が行う
    pub region: Option<Bounds>,
    /// 文書単位の弦の誤差。省略時は予備評価した対象範囲の対角線の 1/50000
    pub tolerance: Option<f64>,
    /// 塊のインスタンス数の目安。1 要素は分割せず、精度のため早く区切る場合もある
    pub chunk_instances: u32,
}

impl Default for Options<'_> {
    fn default() -> Self {
        Self {
            storeys: None,
            classes: None,
            ids: None,
            region: None,
            tolerance: None,
            chunk_instances: 4096,
        }
    }
}

/// 建物から作ったシーン。単位と地図への変換は元の文書の値を併せて渡す。
#[derive(Clone, Debug, PartialEq)]
pub struct Scene {
    /// 符号化する列の値
    pub data: Data,
    /// 曲線の評価へ渡した文書単位の弦の誤差
    pub tolerance: f64,
    /// 描く形を囲む文書単位の箱。形がなければ None
    pub extent: Option<Bounds>,
}

pub(crate) fn reserve<T>(
    out: &mut Vec<T>,
    count: usize,
    slot: u64,
    cx: &mut Cx,
) -> Result<(), Error> {
    let length = out.len().checked_add(count).ok_or(Error::Invalid("size"))?;
    let limit = (crate::MAX_BYTES / slot.max(1)) as usize;
    if length > limit {
        return Err(Error::Invalid("size"));
    }
    if length > out.capacity() {
        let capacity = length.max(out.capacity().saturating_mul(2)).clamp(4, limit);
        cx.check(
            Kind::DecodedBytes,
            (capacity - out.capacity()) as u64 * slot,
        )?;
        out.try_reserve_exact(capacity - out.len())
            .map_err(|_| Error::Invalid("allocation"))?;
    }
    Ok(())
}

pub(crate) fn union(a: &mut Option<Bounds>, b: Bounds) {
    match a {
        None => *a = Some(b),
        Some([min, max]) => {
            for ((min, max), (lo, hi)) in min.iter_mut().zip(max).zip(b[0].into_iter().zip(b[1])) {
                *min = min.min(lo);
                *max = max.max(hi);
            }
        }
    }
}

pub(crate) fn point(bounds: &mut Option<Bounds>, value: [f64; 3]) -> Result<(), Error> {
    if !value.iter().all(|v| v.is_finite()) {
        return Err(Error::Invalid("coordinate"));
    }
    union(bounds, [value, value]);
    Ok(())
}

pub(crate) fn overlaps(a: Bounds, b: Bounds) -> bool {
    a[0].into_iter().zip(b[1]).all(|(min, max)| min <= max)
        && a[1].into_iter().zip(b[0]).all(|(max, min)| max >= min)
}

pub(crate) fn mesh_bounds(
    element: &evaluation::ElementMesh,
    scale: f64,
    cx: &mut Cx,
) -> Result<Option<Bounds>, Error> {
    let mut extent = None;
    for part in &element.parts {
        cx.check(Kind::Work, part.mesh.positions.len() as u64)?;
        for &p in &part.mesh.positions {
            point(&mut extent, part.placement.point(p).map(|v| v / scale))?;
        }
    }
    Ok(extent)
}

fn validate(options: Options) -> Result<(), Error> {
    if options.chunk_instances == 0 || options.chunk_instances > 65536 {
        return Err(Error::Invalid("chunk_instances"));
    }
    if options
        .tolerance
        .is_some_and(|v| !v.is_finite() || v <= 0.0)
    {
        return Err(Error::Invalid("tolerance"));
    }
    if let Some([min, max]) = options.region
        && min
            .into_iter()
            .zip(max)
            .any(|(a, b)| !a.is_finite() || !b.is_finite() || a > b)
    {
        return Err(Error::Invalid("box"));
    }
    Ok(())
}

/// 建物の元値と ID を変えず、開口を引いた形をシーンへ詰める。
///
/// 開口の切削形は表示せず、その ID は要素表に保つ。選択対象の所属階も表へ含める。
/// 形のない要素は箱との重なりを判定できないため残す。開口と分解の関係を保つため、
/// 形の評価は建物全体に行う。許容差を省略すると、1 mm の予備評価と本評価で予算を共有する。
pub fn build(building: &Building, options: Options<'_>, cx: &mut Cx) -> Result<Scene, Error> {
    cx.check(Kind::Work, 1)?;
    validate(options)?;
    let selected = selection::select(building, options, cx)?;
    let unit = building
        .document
        .units
        .iter()
        .find(|u| u.kind == "LENGTHUNIT");
    // 空の建物でも単位不明の座標を推測して返さない。
    let scale = unit
        .map(evaluation::length_scale)
        .transpose()?
        .ok_or(Error::Invalid("unit_missing"))?;
    let tolerance = if let Some(value) = options.tolerance {
        value
    } else {
        let mut diagnostics = Diagnostics::new();
        let mut probe_cx = Cx::new(cx.budget, &mut diagnostics, cx.cancel);
        let probe = evaluation::evaluate(building, evaluation::Options::default(), &mut probe_cx)?;
        let mut extent = None;
        for &at in &selected.elements {
            if selection::is_void(building, at) {
                continue;
            }
            let Some(element) = probe.get(at.0 as usize) else {
                return Err(Error::Invalid("element"));
            };
            // 表示不能な座標は本評価で要素の場所とともに診断する。
            let bounds = match mesh_bounds(element, scale, &mut probe_cx) {
                Ok(value) => value,
                Err(Error::Invalid(_)) => None,
                Err(error) => return Err(error),
            };
            if let Some(bounds) = bounds
                && options.region.is_none_or(|region| overlaps(bounds, region))
            {
                union(&mut extent, bounds);
            }
        }
        // 空か点だけなら、予備評価と同じ実寸の 1 mm を使う。
        let diagonal = extent
            .map(|[a, b]| libm::hypot(libm::hypot(b[0] - a[0], b[1] - a[1]), b[2] - a[2]))
            .unwrap_or(0.0);
        if diagonal > 0.0 {
            diagonal / 50000.0
        } else {
            0.001 / scale
        }
    };
    let chord = tolerance * scale;
    if !chord.is_finite() || chord <= 0.0 {
        return Err(Error::Invalid("tolerance"));
    }
    cx.report(Diagnostic::new(codes::SCENE3D_TOLERANCE_APPROXIMATED).with("tolerance", tolerance));
    let meshes = evaluation::evaluate(
        building,
        evaluation::Options {
            chord,
            openings: true,
        },
        cx,
    )?;
    packing::pack(building, &meshes, selected, options, tolerance, scale, cx)
}
