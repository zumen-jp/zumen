//! 平面直角座標系の GeoTIFF の DEM を、地形の面（セル代表点を頂点にした三角形）の ZMS3 で返す（#896）。
use super::*;
use zumen_scene3d::{Chunk, Color, Data, Element, Instance, Mesh};
use zumen_tiff::DemGrid;

/// 1 つの区画の格子の間隔の数。区画ごとに要素と塊を 1 つ作る
const TILE: usize = 128;

/// 恒等の変換（行優先の 3 行 4 列）
const IDENTITY: [f32; 12] = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0];

/// `stride` おきの格子の行か列。最後の行か列を必ず含める
fn samples(count: u32, stride: u32) -> Vec<u32> {
    let mut out: Vec<u32> = (0..count).step_by(stride.max(1) as usize).collect();
    if let Some(last) = count.checked_sub(1)
        && out.last() != Some(&last)
    {
        out.push(last);
    }
    out
}

/// 要求の間隔に合う、セルの間隔の整数倍
fn stride_for(grid: &DemGrid, tolerance: Option<f64>) -> u32 {
    let Some(tolerance) = tolerance else {
        return 1;
    };
    let cells = libm::ceil(tolerance / grid.spacing);
    let largest = grid.columns.max(grid.rows).max(1);
    if cells >= f64::from(largest) {
        largest
    } else {
        (cells as u32).max(1)
    }
}

/// xy 平面で反時計回りになるように並べた三角形
fn counter_clockwise(points: [[f64; 3]; 3], ids: [u32; 3]) -> [u32; 3] {
    let [a, b, c] = points;
    let cross = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
    if cross < 0.0 {
        [ids[0], ids[2], ids[1]]
    } else {
        ids
    }
}

/// 区画の原点（東・北・標高の最小）、最大、メッシュ
type Tile = ([f64; 3], [f64; 3], Mesh);

/// 1 つの区画のメッシュ。三角形がなければ `None`
fn tile(
    grid: &DemGrid,
    rows: &[u32],
    columns: &[u32],
    cx: &mut Cx<'_>,
) -> Result<Option<Tile>, Stop> {
    let width = columns.len();
    cx.check(Kind::Work, (rows.len() * width) as u64)?;
    let node =
        |i: usize, j: usize| -> Option<[f64; 3]> { grid.cell(*columns.get(j)?, *rows.get(i)?) };
    // 格子点の番号から、メッシュの頂点の番号へ
    let mut index: Vec<Option<u32>> = vec![None; rows.len() * width];
    let mut used: Vec<[f64; 3]> = Vec::new();
    let mut triangles: Vec<[u32; 3]> = Vec::new();
    let mut vertex = |i: usize, j: usize, used: &mut Vec<[f64; 3]>| -> Option<u32> {
        let point = node(i, j)?;
        let slot = index.get_mut(i * width + j)?;
        if slot.is_none() {
            *slot = Some(used.len() as u32);
            used.push(point);
        }
        *slot
    };
    for i in 0..rows.len().saturating_sub(1) {
        for j in 0..width.saturating_sub(1) {
            // a b
            // c d（a から d へ対角線を引く）
            let corners = [(i, j), (i, j + 1), (i + 1, j), (i + 1, j + 1)];
            let valid: Vec<bool> = corners.iter().map(|&(p, q)| node(p, q).is_some()).collect();
            let picks: &[[usize; 3]] = match valid.as_slice() {
                [true, true, true, true] => &[[0, 2, 3], [0, 3, 1]],
                [false, true, true, true] => &[[1, 2, 3]],
                [true, false, true, true] => &[[0, 2, 3]],
                [true, true, false, true] => &[[0, 3, 1]],
                [true, true, true, false] => &[[0, 2, 1]],
                _ => &[],
            };
            for pick in picks {
                let mut ids = [0u32; 3];
                let mut points = [[0.0; 3]; 3];
                for (k, &corner) in pick.iter().enumerate() {
                    let Some(&(p, q)) = corners.get(corner) else {
                        continue;
                    };
                    let (Some(id), Some(point)) = (vertex(p, q, &mut used), node(p, q)) else {
                        continue;
                    };
                    if let (Some(slot), Some(at)) = (ids.get_mut(k), points.get_mut(k)) {
                        *slot = id;
                        *at = point;
                    }
                }
                triangles.push(counter_clockwise(points, ids));
            }
        }
    }
    if triangles.is_empty() {
        return Ok(None);
    }
    cx.check(
        Kind::DecodedBytes,
        (used.len() * 12 + triangles.len() * 12) as u64,
    )?;
    let min = used.iter().fold([f64::INFINITY; 3], |a, b| lower(a, *b));
    let max = used
        .iter()
        .fold([f64::NEG_INFINITY; 3], |a, b| upper(a, *b));
    let [mx, my, mz] = min;
    let vertices = used
        .iter()
        .map(|[x, y, z]| [(x - mx) as f32, (y - my) as f32, (z - mz) as f32])
        .collect();
    Ok(Some((
        min,
        max,
        Mesh {
            vertices,
            triangles,
            edges: Vec::new(),
        },
    )))
}

/// 成分ごとの小さいほう
fn lower([a, b, c]: [f64; 3], [x, y, z]: [f64; 3]) -> [f64; 3] {
    [a.min(x), b.min(y), c.min(z)]
}

/// 成分ごとの大きいほう
fn upper([a, b, c]: [f64; 3], [x, y, z]: [f64; 3]) -> [f64; 3] {
    [a.max(x), b.max(y), c.max(z)]
}

/// 箱と重なるか。箱は東・北・高さ
fn overlaps(min: [f64; 3], max: [f64; 3], region: Option<Bounds>) -> bool {
    region.is_none_or(|[lo, hi]| {
        min.iter().zip(&hi).all(|(a, b)| a <= b) && lo.iter().zip(&max).all(|(a, b)| a <= b)
    })
}

/// DEM の面のシーンの結果と blob のバイト列。結果の `blob` は 0 のままで、blob を足す側が書く
pub(in crate::session) fn terrain_scene(
    doc: &str,
    grid: &DemGrid,
    region: Option<Bounds>,
    tolerance: Option<f64>,
    limit: Option<u64>,
    cx: &mut Cx<'_>,
) -> Result<(Scene3dResult, Vec<u8>), Failed> {
    let largest = grid.columns.max(grid.rows).max(1);
    let mut stride = stride_for(grid, tolerance);
    let mut rows = samples(grid.rows, stride);
    let mut columns = samples(grid.columns, stride);
    let nodes = |rows: &[u32], columns: &[u32]| (rows.len() as u64) * (columns.len() as u64);
    while let Some(limit) = limit
        && nodes(&rows, &columns) > limit
        && stride < largest
    {
        stride = stride.saturating_mul(2).min(largest);
        rows = samples(grid.rows, stride);
        columns = samples(grid.columns, stride);
    }
    let truncated = limit.is_some_and(|limit| nodes(&rows, &columns) > limit);
    let mut data = Data::default();
    let mut extent: Option<Bounds> = None;
    if !truncated {
        data.colors.push(Color {
            rgba: [0.0, 0.0, 0.0, 1.0],
            unresolved: true,
        });
        let row_tiles = rows.len().saturating_sub(1).div_ceil(TILE);
        let column_tiles = columns.len().saturating_sub(1).div_ceil(TILE);
        for ti in 0..row_tiles {
            for tj in 0..column_tiles {
                let r = rows
                    .get(ti * TILE..=((ti + 1) * TILE).min(rows.len().saturating_sub(1)))
                    .unwrap_or_default();
                let c = columns
                    .get(tj * TILE..=((tj + 1) * TILE).min(columns.len().saturating_sub(1)))
                    .unwrap_or_default();
                let Some((min, max, mesh)) = tile(grid, r, c, cx).map_err(|s| stopped(cx, s))?
                else {
                    continue;
                };
                if !overlaps(min, max, region) {
                    continue;
                }
                cx.check(Kind::Elements, 1).map_err(|s| stopped(cx, s))?;
                let (row, column) = (r.first().copied(), c.first().copied());
                let id = data.elements.len() as u32;
                data.elements.push(Element {
                    id: format!("r{}c{}", row.unwrap_or(0), column.unwrap_or(0)),
                    class: "dem".into(),
                    name: String::new(),
                    storey: None,
                });
                data.chunks.push(Chunk {
                    origin: min,
                    bounds: {
                        let ([x0, y0, z0], [x1, y1, z1]) = (min, max);
                        [
                            0.0,
                            0.0,
                            0.0,
                            (x1 - x0) as f32,
                            (y1 - y0) as f32,
                            (z1 - z0) as f32,
                        ]
                    },
                    instances: data.instances.len() as u32,
                });
                data.instances.push(Instance {
                    mesh: data.meshes.len() as u32,
                    element: id,
                    color: 0,
                    transform: IDENTITY,
                });
                data.meshes.push(mesh);
                extent = Some(match extent {
                    None => [min, max],
                    Some([lo, hi]) => [lower(lo, min), upper(hi, max)],
                });
            }
        }
        if data.elements.is_empty() {
            data.colors.clear();
        }
    }
    let counts = Scene3dCounts {
        chunks: data.chunks.len() as u64,
        meshes: data.meshes.len() as u64,
        instances: data.instances.len() as u64,
        elements: data.elements.len() as u64,
        colors: data.colors.len() as u64,
        vertices: data.meshes.iter().map(|m| m.vertices.len() as u64).sum(),
        triangles: data.meshes.iter().map(|m| m.triangles.len() as u64).sum(),
        edges: 0,
        points: None,
    };
    let bytes = zumen_scene3d::encode(&data, cx).map_err(|e| scene_error(cx, e))?;
    let result = Scene3dResult {
        doc: doc.to_string(),
        unit: BuildingUnit {
            kind: "LENGTHUNIT".into(),
            name: "METRE".into(),
            prefix: None,
            scale: None,
        },
        map: Some(Scene3dMap {
            crs_name: Some(format!("EPSG:{}", grid.crs.epsg())),
            eastings: 0.0,
            northings: 0.0,
            height: 0.0,
            x_axis_abscissa: Some(1.0),
            x_axis_ordinate: Some(0.0),
            scale: Some(1.0),
        }),
        height_crs: grid.vertical.map(|v| u64::from(v.epsg())),
        lod: None,
        tolerance: grid.spacing * f64::from(stride),
        truncated: Some(truncated),
        r#box: region.map(output_box),
        extent: extent.map(output_box),
        counts,
        blob: 0,
    };
    Ok((result, bytes))
}

impl Session {
    /// 開いた GeoTIFF の DEM の標高格子。DEM でなければ `None`
    pub(super) fn dem_grid(&self, doc: &str, cx: &mut Cx<'_>) -> Result<Option<DemGrid>, Failed> {
        let tiff = match self.document(doc, cx)? {
            #[cfg(feature = "drawing")]
            Document::Drawing { file, .. } => match file.held() {
                Some(DrawingFile::Tiff(tiff)) => tiff,
                _ => return Ok(None),
            },
            #[cfg(not(feature = "drawing"))]
            Document::Tiff(tiff) => tiff.as_ref(),
            _ => return Ok(None),
        };
        match zumen_sdk::Lift::lift(tiff, cx) {
            Ok(grid) => Ok(Some(grid)),
            Err(zumen_sdk::Error::Unsupported) => Err(fail(cx, unsupported(doc, "scene3d"))),
            Err(_) => Err(Failed),
        }
    }
}
