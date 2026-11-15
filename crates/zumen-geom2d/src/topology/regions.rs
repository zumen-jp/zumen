//! 有限の線分からの閉領域と、閉じた輪郭の集合演算。依存へ渡す前に交点を分割する。

use super::{Charge, Error, Failure, MAX_COORDINATE, Point, Segment, SegmentIntersection};
use i_overlay::{
    core::{
        fill_rule::FillRule,
        overlay::{IntOverlayOptions, Overlay, ShapeType},
        overlay_rule::OverlayRule,
        solver::{Precision, Solver, Strategy},
    },
    i_float::int::point::IntPoint,
    string::{overlay::StringOverlay, rule::StringRule},
};

mod prepare;
use prepare::{Edge, Prepared, reserve, work};

/// 入力点・輪郭・線分と、交点で分割した辺のそれぞれの上限。
pub const MAX_REGION_EDGES: usize = 4096;
/// 格子への丸めを伴う交点分割の上限。最後に交点を再検査する。
pub const MAX_REGION_PASSES: usize = 8;

/// 閉領域の演算に渡した入力の組。
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum RegionOperand {
    /// 集合演算の左側。
    Subject,
    /// 集合演算の右側。
    Clip,
    /// 閉領域を検出する線分の集合。
    Lines,
}

/// 元の入力の位置。図面の要素 ID と出所への対応は呼び出し側が保つ。
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct RegionLocation {
    /// 入力の組。
    pub operand: RegionOperand,
    /// 輪郭の位置。線分の集合では 0。
    pub contour: u64,
    /// 辺か線分の位置。輪郭全体が退化した場合は `None`。
    pub edge: Option<u64>,
}

/// 格子へ丸めた交点によって曲げた元の辺と、分割した点。
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct RegionAdjustment {
    /// 元の入力の位置。
    pub source: RegionLocation,
    /// 分割した格子点。元の線分からの距離は格子の 1 間隔以内。
    pub point: Point,
}

/// 閉領域を作らなかった入力。呼び出し側は種類・件数・出所を診断へ結び付ける。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegionDiscard {
    /// 点または一直線に縮んだ輪郭、または長さ 0 の辺。
    Degenerate(RegionLocation),
    /// 検出した閉領域の境界に含まれない線分の部分。橋や開いた枝も含む。
    Open {
        /// 元の線分の位置。
        source: RegionLocation,
        /// 使われなかった部分。元の向きと分割点を保つ。
        segment: Segment,
    },
}

/// 閉じた輪郭の内部を決める規則。自己交差も同じ規則で扱う。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegionFill {
    /// 境界を横切る回数が奇数の部分。
    EvenOdd,
    /// 符号を付けて数えた周回数が 0 でない部分。
    NonZero,
}

/// 左右の閉領域の集合演算。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegionOperation {
    /// 和。
    Union,
    /// 積。点や線だけの接触は面積を持つ結果に含めない。
    Intersection,
    /// 左側から右側を除く差。
    Difference,
    /// 片方だけに含まれる部分。
    Xor,
}

/// 外周と穴で表した閉領域。最後の点から最初の点へ閉じる。
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Region {
    /// 反時計回りの外周。辞書順で最小の点から始める。
    pub outer: Vec<Point>,
    /// 時計回りの穴。各輪郭の始点をそろえ、輪郭の辞書順に並べる。
    pub holes: Vec<Vec<Point>>,
}

/// 全結果か失敗を返し、入力を変更しない。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RegionResult {
    /// 外周、穴の順の辞書順にそろえた領域。
    pub regions: Vec<Region>,
    /// 元の辺と点の順で並べた、重複のない丸めの記録。
    pub adjustments: Vec<RegionAdjustment>,
    /// 退化した入力、続いて閉境界に使わなかった部分を元の入力順で返す。
    pub discarded: Vec<RegionDiscard>,
}

fn solver() -> Solver {
    Solver {
        strategy: Strategy::List,
        precision: Precision::ABSOLUTE,
        multithreading: None,
    }
}

fn int(point: Point) -> IntPoint<i64> {
    let [x, y] = point.coordinates();
    IntPoint::new(x, y)
}

// iOverlay 9.2.0 の整数 API に限る。交点を先に分割し、内部で反復する丸めを発生させない。
// 走査・抽出・穴の所属判定の二乗の上限と、辺ごとの確保の余裕を先に数える。
// 依存の更新時はこの上限、prepare、入出力の契約記録を合わせて再監査する。
fn library_budget<E>(
    edges: usize,
    check: &mut impl FnMut(Charge) -> Result<(), E>,
) -> Result<(), Failure<E>> {
    let n = (edges + 4) as u64;
    work(64 * n * n, check)?;
    check(Charge::Bytes(4096 * n)).map_err(Failure::Stopped)
}

/// 輪郭の和・積・差・排他的論理和を返す。輪郭は暗黙に閉じ、末尾の始点の重複も受け付ける。
///
/// 交点を格子へ丸めて分割し、元の辺からの距離を格子の 1 間隔以内に制限する。
/// 分割は最大 [`MAX_REGION_PASSES`] 回、辺は最大 [`MAX_REGION_EDGES`] 本。
/// 範囲・順序を保てないときや分割が収束しないときは `Precision`、個数超過は `Range`。
/// 同じ予算の検査列を全環境で使い、停止時に部分結果を返さない。
pub fn boolean_regions<E>(
    subject: &[&[Point]],
    clip: &[&[Point]],
    operation: RegionOperation,
    fill: RegionFill,
    mut check: impl FnMut(Charge) -> Result<(), E>,
) -> Result<RegionResult, Failure<E>> {
    let mut input = Prepared::contours(subject, clip, &mut check)?;
    input.split(&mut check)?;
    library_budget(input.edges.len(), &mut check)?;
    let mut options = IntOverlayOptions::keep_all_points();
    options.ogc = true;
    let mut overlay = Overlay::<i64>::new_custom(input.edges.len(), options, solver());
    for path in input.edges.chunk_by(|a, b| {
        (a.source.operand, a.source.contour) == (b.source.operand, b.source.contour)
    }) {
        work(0, &mut check)?;
        let shape = if path
            .first()
            .is_some_and(|edge| edge.source.operand == RegionOperand::Subject)
        {
            ShapeType::Subject
        } else {
            ShapeType::Clip
        };
        overlay.add_path_iter(path.iter().map(|edge| int(edge.segment.start)), shape);
    }
    let rule = match operation {
        RegionOperation::Union => OverlayRule::Union,
        RegionOperation::Intersection => OverlayRule::Intersect,
        RegionOperation::Difference => OverlayRule::Difference,
        RegionOperation::Xor => OverlayRule::Xor,
    };
    let fill = match fill {
        RegionFill::EvenOdd => FillRule::EvenOdd,
        RegionFill::NonZero => FillRule::NonZero,
    };
    let shapes = overlay.overlay(rule, fill);
    work(0, &mut check)?;
    input.result.regions = canonical(shapes, false, input.edges.len(), &mut check)?;
    Ok(input.result)
}

/// 線分の集合が囲むすべての有限の面を返す。穴の内側の面も別の領域として返す。
/// 線の向き・重複に依存せず、開いた枝や橋は `discarded` に元の入力位置付きで返す。
/// 交点の丸め、個数と予算の制限は [`boolean_regions`] と同じ。
pub fn detect_regions<E>(
    lines: &[Segment],
    mut check: impl FnMut(Charge) -> Result<(), E>,
) -> Result<RegionResult, Failure<E>> {
    let mut input = Prepared::lines(lines, &mut check)?;
    input.split(&mut check)?;
    library_budget(input.edges.len(), &mut check)?;
    let options = IntOverlayOptions::keep_all_points();
    let mut overlay = StringOverlay::<i64>::with_options(input.edges.len() + 4, options);
    // 全入力の外側に固定の枠を置く。枠を含む外側の面だけを取り除く。
    let m = MAX_COORDINATE + 1;
    overlay.add_shape_contour_iter(
        [[-m, -m], [m, -m], [m, m], [-m, m]]
            .into_iter()
            .map(|[x, y]| IntPoint::new(x, y)),
    );
    for edge in &input.edges {
        work(0, &mut check)?;
        overlay.add_string_line([int(edge.segment.start), int(edge.segment.end)]);
    }
    let shapes = overlay
        .build_graph_view_with_solver(FillRule::NonZero, solver())
        .map_or_else(Vec::new, |graph| {
            graph.extract_shapes_custom(StringRule::Slice, options)
        });
    work(0, &mut check)?;
    input.result.regions = canonical(shapes, true, input.edges.len(), &mut check)?;
    find_open(&input.edges, &mut input.result, &mut check)?;
    Ok(input.result)
}

fn canonical<E>(
    shapes: Vec<Vec<Vec<IntPoint<i64>>>>,
    remove_frame: bool,
    edges: usize,
    check: &mut impl FnMut(Charge) -> Result<(), E>,
) -> Result<Vec<Region>, Failure<E>> {
    let count = shapes.iter().flatten().map(Vec::len).sum::<usize>();
    // 分割済みの無向辺を両側からたどった点数まで。枠の 4 辺も含む。
    if count > 2 * (edges + 4) {
        return Err(Failure::Geometry(Error::Precision));
    }
    work((4 * count * count + count) as u64, check)?;
    let mut regions = Vec::new();
    reserve(&mut regions, shapes.len(), 64, check)?;
    for shape in shapes {
        work(0, check)?;
        if shape.is_empty() {
            return Err(Failure::Geometry(Error::Precision));
        }
        if remove_frame
            && shape.first().is_some_and(|path| {
                path.iter()
                    .any(|p| p.x.abs() > MAX_COORDINATE || p.y.abs() > MAX_COORDINATE)
            })
        {
            continue;
        }
        let mut contours = Vec::new();
        reserve(&mut contours, shape.len(), 32, check)?;
        for (index, path) in shape.into_iter().enumerate() {
            let mut points = Vec::new();
            reserve(&mut points, path.len(), 16, check)?;
            check(Charge::Points(path.len() as u64)).map_err(Failure::Stopped)?;
            for point in path {
                points.push(Point::new(point.x, point.y).map_err(Failure::Geometry)?);
            }
            if points.len() < 3 {
                return Err(Failure::Geometry(Error::Precision));
            }
            let area = area_twice(&points);
            if area == 0 {
                return Err(Failure::Geometry(Error::Precision));
            }
            if (area > 0) != (index == 0) {
                points.reverse();
            }
            let mut first = 0;
            for i in 1..points.len() {
                if points
                    .iter()
                    .cycle()
                    .skip(i)
                    .take(points.len())
                    .cmp(points.iter().cycle().skip(first).take(points.len()))
                    .is_lt()
                {
                    first = i;
                }
            }
            points.rotate_left(first);
            contours.push(points);
        }
        let outer = contours.remove(0);
        contours.sort_unstable();
        regions.push(Region {
            outer,
            holes: contours,
        });
    }
    regions.sort_unstable();
    Ok(regions)
}

fn area_twice(points: &[Point]) -> i128 {
    points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .map(|(&a, &b)| {
            let [x, y] = a.coordinates().map(i128::from);
            let [u, v] = b.coordinates().map(i128::from);
            x * v - y * u
        })
        .sum()
}

fn find_open<E>(
    edges: &[Edge],
    result: &mut RegionResult,
    check: &mut impl FnMut(Charge) -> Result<(), E>,
) -> Result<(), Failure<E>> {
    let boundary_count = result
        .regions
        .iter()
        .map(|r| r.outer.len() + r.holes.iter().map(Vec::len).sum::<usize>())
        .sum::<usize>();
    work((edges.len() * boundary_count) as u64, check)?;
    reserve(&mut result.discarded, edges.len(), 80, check)?;
    for edge in edges {
        work(0, check)?;
        let mut used = false;
        for contour in result
            .regions
            .iter()
            .flat_map(|region| std::iter::once(&region.outer).chain(&region.holes))
        {
            for (&start, &end) in contour.iter().zip(contour.iter().cycle().skip(1)) {
                if let SegmentIntersection::Overlap(overlap) =
                    super::intersect(edge.segment, Segment { start, end })
                        .map_err(Failure::Geometry)?
                {
                    if super::ends(overlap) != super::ends(edge.segment) {
                        // 分割済みの辺の途中に、依存が別の頂点を作った場合は受け付けない。
                        return Err(Failure::Geometry(Error::Precision));
                    }
                    used = true;
                }
            }
        }
        if !used {
            check(Charge::Points(2)).map_err(Failure::Stopped)?;
            result.discarded.push(RegionDiscard::Open {
                source: edge.source,
                segment: edge.segment,
            });
        }
    }
    Ok(())
}
