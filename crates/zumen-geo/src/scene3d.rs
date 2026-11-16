//! 地物の ID・型・名前と、投影した面・辺を既存の ZMS3 の列へ移す。
use zumen_base::{Cx, Diagnostic, Kind, codes};
use zumen_crs::{VerticalCrs, Zone};
use zumen_geom3d::{Failure, ManifoldKernel, Mesh, Tolerance};
use zumen_scene3d::{Bounds, Data, Scene};

use crate::{
    Feature, Form, GeoModel, Selection,
    visual::{self, Projection},
};

mod packing;
mod shape;

/// 地物のシーンの選択と精度。長さと座標はすべてメートル。
#[derive(Clone, Copy, Debug, Default)]
pub struct Options<'a> {
    /// 投影先。省略時は平面座標の元の系か、経緯度の範囲の中心から仮定した系。
    pub zone: Option<Zone>,
    /// 地物と LOD。LOD の省略時は選んだ地物にある最大の LOD を 1 段だけ使う。
    pub selection: Selection<'a>,
    /// 座標を丸める誤差の上限。省略時は選択した範囲の対角線の 1/50000。
    pub tolerance: Option<f64>,
    /// 投影先の東・北・高さの箱と重なる地物を選ぶ。切り抜かない。
    pub region: Option<Bounds>,
}

/// 共通のシーンと、その座標の意味。高さの基準が不明でも数値を変えない。
#[derive(Clone, Debug, PartialEq)]
pub struct Output {
    /// 共通のメッシュ・要素・配置・塊。
    pub scene: Scene,
    /// X は東、Y は北、Z は高さ。平面直角座標の系。
    pub zone: Zone,
    /// 元の標高の基準と成果。
    pub height: Option<VerticalCrs>,
    /// 選択した LOD。LOD のない形だけなら None。
    pub lod: Option<u8>,
}

struct Pending {
    feature: usize,
    element: u32,
    meshes: Vec<Mesh>,
    bounds: Bounds,
}

/// 頂点・面の順と元の地物 ID を保って、シーンを作る。
pub fn build(model: &GeoModel, options: Options<'_>, cx: &mut Cx) -> Result<Output, Failure> {
    visual::validate(model, options.selection, cx)?;
    if options
        .tolerance
        .is_some_and(|v| !v.is_finite() || v <= 0.0)
    {
        return Err(Failure::Invalid("tolerance"));
    }
    if let Some([lo, hi]) = options.region
        && lo
            .into_iter()
            .zip(hi)
            .any(|(a, b)| !a.is_finite() || !b.is_finite() || a > b)
    {
        return Err(Failure::Invalid("box"));
    }
    let projection = Projection::new(model, options.zone, cx)?;
    let mut lod = options.selection.lod;
    if lod.is_none() {
        for feature in &model.features {
            if !options.selection.includes(feature, cx)? {
                continue;
            }
            for geometry in &feature.geometries {
                cx.check(Kind::Work, 1)?;
                if geometry.lod.is_some_and(|n| n > 4) {
                    return Err(Failure::Invalid("lod"));
                }
                lod = lod.max(geometry.lod);
            }
        }
    }
    let selection = Selection {
        lod,
        ..options.selection
    };
    let mut data = Data::default();
    let mut pending = Vec::new();
    let mut extent = None;
    for (feature_index, feature) in model.features.iter().enumerate() {
        if !selection.includes(feature, cx)? {
            continue;
        }
        let mut parts = Vec::new();
        let mut bounds = None;
        for geometry in &feature.geometries {
            cx.check(Kind::Work, 1)?;
            if !selection.geometry(geometry) {
                continue;
            }
            let result = shape::evaluate(&geometry.form, projection, cx);
            match result {
                Ok((mesh, changes)) => {
                    projection.report(feature, &geometry.form, cx)?;
                    let place = visual::source(feature, cx)?;
                    changes.report(
                        Some(&place),
                        Tolerance {
                            chord: 0.001,
                            length: 1e-9,
                        },
                        cx,
                    )?;
                    if let Some(part_bounds) = mesh_bounds(&mesh, cx)? {
                        union(&mut bounds, part_bounds);
                        visual::reserve(&mut parts, 1, 160, cx)?;
                        parts.push(mesh);
                    } else {
                        visual::omitted(feature, visual::form_name(&geometry.form), "empty", cx)?;
                    }
                }
                Err(error) => failed(feature, &geometry.form, error, cx)?,
            }
        }
        if let (Some(region), Some(bounds)) = (options.region, bounds)
            && !overlaps(region, bounds)
        {
            continue;
        }
        let element = u32::try_from(data.elements.len()).map_err(|_| Failure::Invalid("size"))?;
        visual::reserve(&mut data.elements, 1, 128, cx)?;
        data.elements.push(zumen_scene3d::Element {
            id: visual::text(&feature.id, cx)?,
            class: visual::text(&feature.kind, cx)?,
            name: visual::text(feature.name.as_deref().unwrap_or(""), cx)?,
            storey: None,
        });
        if let Some(bounds) = bounds {
            union(&mut extent, bounds);
            visual::reserve(&mut pending, 1, 192, cx)?;
            pending.push(Pending {
                feature: feature_index,
                element,
                meshes: parts,
                bounds,
            });
        }
    }
    let tolerance = options.tolerance.unwrap_or_else(|| {
        extent
            .map(|[lo, hi]| zumen_geom3d::length(zumen_geom3d::sub(hi, lo)) / 50_000.0)
            .filter(|n| n.is_finite() && *n > 0.0)
            .unwrap_or(0.001)
    });
    let mut extent = None;
    for part in pending {
        let feature = model
            .features
            .get(part.feature)
            .ok_or(Failure::Invalid("feature"))?;
        match packing::append(part, tolerance, &mut data, cx) {
            Ok((bounds, error)) => {
                union(&mut extent, bounds);
                let place = visual::source(feature, cx)?;
                cx.report(Diagnostic::new(codes::SCENE3D_COLOR_MISSING).at(place));
                if error > 0.0 {
                    let place = visual::source(feature, cx)?;
                    cx.report(
                        Diagnostic::new(codes::SCENE3D_COORDINATE_APPROXIMATED)
                            .with("error", error)
                            .at(place),
                    );
                }
            }
            Err(Failure::Stopped(stop)) => return Err(stop.into()),
            Err(error) => visual::omitted(feature, "mesh", reason(&error), cx)?,
        }
    }
    cx.check(Kind::Work, 0)?;
    Ok(Output {
        scene: Scene {
            data,
            tolerance,
            extent,
        },
        zone: projection.zone,
        height: model.height,
        lod,
    })
}

fn reason(error: &Failure) -> &str {
    match error {
        Failure::Unsupported(reason) | Failure::Invalid(reason) => reason,
        _ => "geometry",
    }
}
fn failed(feature: &Feature, form: &Form, error: Failure, cx: &mut Cx) -> Result<(), Failure> {
    match error {
        Failure::Stopped(stop) => Err(stop.into()),
        error => visual::omitted(feature, visual::form_name(form), reason(&error), cx),
    }
}
fn union(extent: &mut Option<Bounds>, bounds: Bounds) {
    match extent {
        Some([lo, hi]) => {
            for ((lo, hi), (a, b)) in lo
                .iter_mut()
                .zip(hi)
                .zip(bounds[0].into_iter().zip(bounds[1]))
            {
                *lo = lo.min(a);
                *hi = hi.max(b);
            }
        }
        None => *extent = Some(bounds),
    }
}
fn overlaps(a: Bounds, b: Bounds) -> bool {
    a[0].into_iter().zip(b[1]).all(|(lo, hi)| lo <= hi)
        && a[1].into_iter().zip(b[0]).all(|(hi, lo)| hi >= lo)
}
fn mesh_bounds(mesh: &Mesh, cx: &mut Cx) -> Result<Option<Bounds>, Failure> {
    let mut bounds = None;
    for &point in &mesh.positions {
        cx.check(Kind::Work, 1)?;
        if !point.iter().all(|n| n.is_finite()) {
            return Err(Failure::Invalid("coordinate"));
        }
        union(&mut bounds, [point, point]);
    }
    Ok(bounds)
}
