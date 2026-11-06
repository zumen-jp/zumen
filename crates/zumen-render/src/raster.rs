//! 固定の 4×4 サンプルを走査する（決定 0277）。確保・辺・画素の前に予算を数える。

use std::ops::Range;

use zumen_base::{Cx, Kind};

use crate::{Error, MAX_PNG_PIXELS, Options};

pub(crate) type Point = [f64; 2];
const COORDINATE_LIMIT: f64 = 2_147_483_648.0;

pub(crate) fn point(p: Point) -> Result<Point, Error> {
    if p.iter()
        .any(|v| !v.is_finite() || v.abs() > COORDINATE_LIMIT)
    {
        return Err(Error::Options);
    }
    Ok(p)
}

/// 論理上の容量を別に持ち、allocator の余分な容量や usize の幅で予算を変えない。
pub(crate) struct Buffer<T> {
    pub items: Vec<T>,
    capacity: usize,
    item_bytes: u64,
}

impl<T> Buffer<T> {
    pub fn new(item_bytes: u64) -> Self {
        Self {
            items: Vec::new(),
            capacity: 0,
            item_bytes,
        }
    }

    pub fn push(&mut self, item: T, cx: &mut Cx<'_>) -> Result<(), Error> {
        cx.check(Kind::Work, 1)?;
        if self.items.len() == self.capacity {
            let capacity = self.capacity.saturating_mul(2).max(16);
            let bytes = (capacity as u64)
                .checked_mul(self.item_bytes)
                .ok_or(Error::Allocation)?;
            cx.check(Kind::DecodedBytes, bytes)?;
            cx.check(Kind::Work, self.items.len() as u64)?;
            if bytes > i32::MAX as u64 {
                return Err(Error::Allocation);
            }
            self.items
                .try_reserve_exact(capacity - self.items.len())
                .map_err(|_| Error::Allocation)?;
            self.capacity = capacity;
        }
        self.items.push(item);
        Ok(())
    }
}

#[derive(Clone, Copy)]
struct Edge {
    low: Point,
    high: Point,
    winding: i64,
}

pub(crate) struct Path {
    edges: Buffer<Edge>,
    pub evenodd: bool,
    pub inverse: bool,
    // [xmin, ymin, xmax, ymax]。水平な辺だけの輪の範囲は空。
    bounds: [f64; 4],
}

impl Path {
    pub fn new(evenodd: bool) -> Self {
        Self {
            edges: Buffer::new(40),
            evenodd,
            inverse: false,
            bounds: [
                f64::INFINITY,
                f64::INFINITY,
                f64::NEG_INFINITY,
                f64::NEG_INFINITY,
            ],
        }
    }

    pub fn edge(&mut self, a: Point, b: Point, cx: &mut Cx<'_>) -> Result<(), Error> {
        let (a, b) = (point(a)?, point(b)?);
        cx.check(Kind::Segments, 1)?;
        if a[1] != b[1] {
            let (low, high, winding) = if a[1] < b[1] { (a, b, 1) } else { (b, a, -1) };
            self.bounds[0] = self.bounds[0].min(a[0]).min(b[0]);
            self.bounds[1] = self.bounds[1].min(low[1]);
            self.bounds[2] = self.bounds[2].max(a[0]).max(b[0]);
            self.bounds[3] = self.bounds[3].max(high[1]);
            self.edges.push(Edge { low, high, winding }, cx)?;
        }
        Ok(())
    }

    pub fn polygon(&mut self, points: &[Point], cx: &mut Cx<'_>) -> Result<(), Error> {
        if let Some((&first, rest)) = points.split_first() {
            let mut previous = first;
            for &next in rest {
                self.edge(previous, next, cx)?;
                previous = next;
            }
            self.edge(previous, first, cx)?;
        }
        Ok(())
    }

    /// 線分の四角形と外側の join を、同じ向きの輪として一度だけ合成する。
    pub fn stroke(
        &mut self,
        points: &[Point],
        width: f64,
        closed: bool,
        cx: &mut Cx<'_>,
    ) -> Result<(), Error> {
        point([width, width])?;
        if width <= 0.0 {
            return Err(Error::Options);
        }
        let mut first = None;
        let mut previous = None;
        let count = points.len().saturating_sub(1) + usize::from(closed && !points.is_empty());
        for index in 0..count {
            cx.check(Kind::Work, 1)?;
            let next = if index + 1 == points.len() {
                0
            } else {
                index + 1
            };
            let a = *points.get(index).ok_or(Error::Scene)?;
            let b = *points.get(next).ok_or(Error::Scene)?;
            point(a)?;
            point(b)?;
            let delta = [b[0] - a[0], b[1] - a[1]];
            let length = libm::hypot(delta[0], delta[1]);
            if length == 0.0 {
                continue;
            }
            let normal = [
                -delta[1] / length * (width / 2.0),
                delta[0] / length * (width / 2.0),
            ];
            self.polygon(
                &[
                    add(a, normal),
                    sub(a, normal),
                    sub(b, normal),
                    add(b, normal),
                ],
                cx,
            )?;
            if let Some((_, prev_normal)) = previous {
                self.join(a, prev_normal, normal, cx)?;
            } else {
                first = Some((a, normal));
            }
            previous = Some((b, normal));
        }
        if closed && let (Some((at, first)), Some((_, last))) = (first, previous) {
            self.join(at, last, first, cx)?;
        }
        Ok(())
    }

    fn join(&mut self, at: Point, a: Point, b: Point, cx: &mut Cx<'_>) -> Result<(), Error> {
        // SVG 1.1 11.4 の miterlimit=4。平行・反転の辺は四角形だけで覆える。
        let cross = a[0] * b[1] - a[1] * b[0];
        if cross == 0.0 {
            return Ok(());
        }
        let sign = if cross > 0.0 { -1.0 } else { 1.0 };
        let a = [a[0] * sign, a[1] * sign];
        let b = [b[0] * sign, b[1] * sign];
        let radius2 = a[0] * a[0] + a[1] * a[1];
        let sum = add(a, b);
        let divisor = sum[0] * a[0] + sum[1] * a[1];
        let miter = [sum[0] * (radius2 / divisor), sum[1] * (radius2 / divisor)];
        let mut ring = [at, add(at, a), add(at, b), at];
        let count = if divisor > 0.0
            && miter.iter().all(|n| n.is_finite())
            && miter[0] * miter[0] + miter[1] * miter[1] <= 16.0 * radius2
        {
            ring[2] = add(at, miter);
            ring[3] = add(at, b);
            4
        } else {
            3
        };
        // 四角形と同じ向きにする。別の線分との重なりも nonzero で一度だけ塗る。
        if cross < 0.0 {
            ring.get_mut(..count).ok_or(Error::Scene)?.reverse();
        }
        self.polygon(ring.get(..count).ok_or(Error::Scene)?, cx)
    }
}

fn add(a: Point, b: Point) -> Point {
    [a[0] + b[0], a[1] + b[1]]
}
fn sub(a: Point, b: Point) -> Point {
    [a[0] - b[0], a[1] - b[1]]
}

pub(crate) enum Shape<'a> {
    Path(&'a Path),
    Circle { at: Point, outer: f64, inner: f64 },
}

// 交点そのものではなく、交点以上の最初のサンプルの位置。横方向の範囲外は端へそろえる。
fn sample(x: f64, range: &Range<u64>) -> u64 {
    libm::ceil(x * 4.0 - 0.5).clamp(range.start as f64, range.end as f64) as u64
}

fn intersect(a: [f64; 4], b: [f64; 4]) -> [f64; 4] {
    [
        a[0].max(b[0]),
        a[1].max(b[1]),
        a[2].min(b[2]),
        a[3].min(b[3]),
    ]
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Event {
    column: u64,
    winding: i64,
}

pub(crate) struct Surface {
    pub pixels: Vec<[u8; 4]>,
    width: usize,
    height: usize,
    viewport: [f64; 4],
    mask: Vec<u16>,
    clip_mask: Vec<u16>,
    events: Buffer<Event>,
}

impl Surface {
    pub fn new(o: &Options, camera: &crate::svg::Camera, cx: &mut Cx<'_>) -> Result<Self, Error> {
        let count = u64::from(o.width) * u64::from(o.height);
        if count == 0 || count > MAX_PNG_PIXELS {
            return Err(Error::Options);
        }
        let width = o.width as usize;
        let [x, y] = camera.offset;
        let [w, h] = camera.size;
        // RGBA と 2 行分のサンプルマスク。大きな初期化は 1024 要素ずつに分ける。
        cx.check(Kind::DecodedBytes, count * 4 + u64::from(o.width) * 4)?;
        let background = if o.background[3] == 0 {
            [0; 4]
        } else {
            o.background
        };
        Ok(Self {
            pixels: initialized(count as usize, background, cx)?,
            width,
            height: o.height as usize,
            viewport: [x, y, x + w, y + h],
            mask: initialized(width, 0, cx)?,
            clip_mask: initialized(width, 0, cx)?,
            events: Buffer::new(16),
        })
    }

    pub fn paint(
        &mut self,
        shape: Shape<'_>,
        clips: &[Path],
        rgba: [u8; 4],
        cx: &mut Cx<'_>,
    ) -> Result<(), Error> {
        cx.check(Kind::Work, 1)?;
        let bounds = match shape {
            Shape::Path(path) => path.bounds,
            Shape::Circle { at, outer, inner } => {
                point(at)?;
                point([outer, inner])?;
                if inner < 0.0 || outer < inner {
                    return Err(Error::Options);
                }
                if outer == 0.0 {
                    return Ok(());
                }
                [at[0] - outer, at[1] - outer, at[0] + outer, at[1] + outer]
            }
        };
        let mut bounds = intersect(bounds, self.viewport);
        for clip in clips {
            cx.check(Kind::Work, 1)?;
            if !clip.inverse {
                bounds = intersect(bounds, clip.bounds);
            }
        }
        // 図形・表示範囲・通常のクリップが重なるサンプルだけを走査する。
        // 反転クリップは輪の外側も通すので、範囲を狭めずマスクで判定する。
        let x_samples = 0..self.width as u64 * 4;
        let y_samples = 0..self.height as u64 * 4;
        let columns = sample(bounds[0], &x_samples)..sample(bounds[2], &x_samples);
        let rows = sample(bounds[1], &y_samples)..sample(bounds[3], &y_samples);
        if columns.is_empty() || rows.is_empty() {
            return Ok(());
        }
        let left = (columns.start / 4) as usize;
        let right = columns.end.div_ceil(4) as usize;
        let mask = self.mask.get_mut(left..right).ok_or(Error::Scene)?;
        let clip_mask = self.clip_mask.get_mut(left..right).ok_or(Error::Scene)?;
        for row in rows.start / 4..rows.end.div_ceil(4) {
            cx.check(Kind::Work, 1)?;
            let subrows = rows.start.max(row * 4)..rows.end.min((row + 1) * 4);
            row_mask(
                &shape,
                &columns,
                subrows.clone(),
                mask,
                &mut self.events,
                cx,
            )?;
            for clip in clips {
                row_mask(
                    &Shape::Path(clip),
                    &columns,
                    subrows.clone(),
                    clip_mask,
                    &mut self.events,
                    cx,
                )?;
                for (mask, clip) in mask.iter_mut().zip(clip_mask.iter()) {
                    cx.check(Kind::Work, 1)?;
                    *mask &= clip;
                }
            }
            let from = row as usize * self.width;
            for (pixel, &mask) in self
                .pixels
                .get_mut(from + left..from + right)
                .ok_or(Error::Scene)?
                .iter_mut()
                .zip(mask.iter())
            {
                cx.check(Kind::Work, 1)?;
                composite(pixel, rgba, mask.count_ones());
            }
        }
        Ok(())
    }
}

pub(crate) fn initialized<T: Copy>(
    count: usize,
    value: T,
    cx: &mut Cx<'_>,
) -> Result<Vec<T>, Error> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| Error::Allocation)?;
    let block = [value; 1024];
    while values.len() < count {
        let length = (count - values.len()).min(block.len());
        cx.check(Kind::Work, length as u64)?;
        values.extend_from_slice(block.get(..length).ok_or(Error::Scene)?);
    }
    Ok(values)
}

fn row_mask(
    shape: &Shape<'_>,
    columns: &Range<u64>,
    subrows: Range<u64>,
    mask: &mut [u16],
    events: &mut Buffer<Event>,
    cx: &mut Cx<'_>,
) -> Result<(), Error> {
    for value in mask.iter_mut() {
        cx.check(Kind::Work, 1)?;
        *value = 0;
    }
    let (evenodd, inverse) = match shape {
        Shape::Path(p) => (p.evenodd, p.inverse),
        _ => (true, false),
    };
    // マスクの先頭は画素境界。表示範囲はその画素の途中からでも始められる。
    let origin = columns.start / 4 * 4;
    for sample_y in subrows {
        let y = (sample_y as f64 + 0.5) / 4.0;
        let subrow = (sample_y % 4) as u32;
        events.items.clear();
        match shape {
            Shape::Path(path) => {
                for &Edge { low, high, winding } in &path.edges.items {
                    cx.check(Kind::Work, 1)?;
                    if low[1] <= y && y < high[1] {
                        // 先に傾きを求め、走査線に近い端点から交点までを測る。
                        // 長い辺の両端が遠くても、頂点順にかかわらず同じ演算になる。
                        let slope = (high[0] - low[0]) / (high[1] - low[1]);
                        let from_low = y - low[1];
                        let from_high = high[1] - y;
                        let x = if from_low <= from_high {
                            low[0] + from_low * slope
                        } else {
                            high[0] - from_high * slope
                        };
                        events.push(
                            Event {
                                column: sample(x, columns) - origin,
                                winding,
                            },
                            cx,
                        )?;
                    }
                }
            }
            Shape::Circle { at, outer, inner } => {
                for radius in [*outer, *inner] {
                    cx.check(Kind::Work, 1)?;
                    let dy = (y - at[1]).abs();
                    if radius > 0.0 && dy < radius {
                        // 接線の近くで大きい二乗値を引くと、サンプルの内外が変わる。
                        let dx = libm::sqrt((radius - dy) * (radius + dy));
                        for x in [at[0] - dx, at[0] + dx] {
                            events.push(
                                Event {
                                    column: sample(x, columns) - origin,
                                    winding: 1,
                                },
                                cx,
                            )?;
                        }
                    }
                }
            }
        }
        sort(&mut events.items, cx)?;
        let mut winding = 0_i64;
        let mut previous = columns.start - origin;
        for event in &events.items {
            cx.check(Kind::Work, 1)?;
            if (if evenodd {
                winding % 2 != 0
            } else {
                winding != 0
            }) ^ inverse
            {
                span(mask, previous, event.column, subrow, cx)?;
            }
            winding += event.winding;
            previous = event.column;
        }
        if inverse {
            span(mask, previous, columns.end - origin, subrow, cx)?;
        }
    }
    Ok(())
}

fn span(mask: &mut [u16], start: u64, end: u64, subrow: u32, cx: &mut Cx<'_>) -> Result<(), Error> {
    for column in start..end {
        cx.check(Kind::Work, 1)?;
        *mask.get_mut((column / 4) as usize).ok_or(Error::Scene)? |=
            1 << (subrow * 4 + (column % 4) as u32);
    }
    Ok(())
}

fn sort(events: &mut [Event], cx: &mut Cx<'_>) -> Result<(), Error> {
    for root in (0..events.len() / 2).rev() {
        sift(events, root, events.len(), cx)?;
    }
    for end in (1..events.len()).rev() {
        cx.check(Kind::Work, 1)?;
        events.swap(0, end);
        sift(events, 0, end, cx)?;
    }
    Ok(())
}

fn sift(events: &mut [Event], mut root: usize, end: usize, cx: &mut Cx<'_>) -> Result<(), Error> {
    loop {
        cx.check(Kind::Work, 1)?;
        let mut child = root * 2 + 1; // 容量は i32::MAX / 16 以下。
        if child >= end {
            return Ok(());
        }
        if child + 1 < end && events.get(child) < events.get(child + 1) {
            child += 1;
        }
        if events.get(root) >= events.get(child) {
            return Ok(());
        }
        events.swap(root, child);
        root = child;
    }
}

fn composite(target: &mut [u8; 4], source: [u8; 4], samples: u32) {
    // sRGB の source-over。coverage の分母を 16 に保ち、最後に最も近い整数へ丸める。
    let alpha = u64::from(source[3]) * u64::from(samples);
    let rest = u64::from(target[3]) * (4080 - alpha);
    let total = alpha * 255 + rest;
    if alpha == 0 {
        return;
    }
    for (target, source) in target.iter_mut().zip(source).take(3) {
        *target = ((u64::from(source) * alpha * 255 + u64::from(*target) * rest + total / 2)
            / total) as u8;
    }
    target[3] = ((total + 2040) / 4080) as u8;
}

#[cfg(test)]
mod tests {
    use super::*;
    use zumen_base::{Budget, CancelToken, Diagnostics, Stop};

    fn options() -> Options {
        Options {
            width: 5,
            height: 5,
            region: [0.0, 0.0, 5.0, 5.0],
            dpi: 25.4,
            background: [0; 4],
            foreground: [0, 0, 0, 255],
        }
    }

    fn test_surface(cx: &mut Cx<'_>) -> Surface {
        let o = options();
        Surface::new(&o, &crate::svg::Camera::new(&o).unwrap(), cx).unwrap()
    }

    #[test]
    fn collinear_edges_of_different_lengths_cover_no_samples() {
        let (mut budget, mut diag, cancel) =
            (Budget::default(), Diagnostics::new(), CancelToken::new());
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        let mut surface = test_surface(&mut cx);
        let mut path = Path::new(true);
        // 同じ直線 x=y を 1 辺で進んで、長さの違う 2 辺で戻る。
        path.polygon(&[[-987_650_560.0; 2], [3.0; 2], [0.0; 2]], &mut cx)
            .unwrap();
        surface
            .paint(Shape::Path(&path), &[], [255; 4], &mut cx)
            .unwrap();
        assert!(surface.pixels.iter().all(|p| *p == [0; 4]));
    }

    #[test]
    fn fractional_viewport_and_clips_use_the_same_subsamples() {
        let (mut budget, mut diag, cancel) =
            (Budget::default(), Diagnostics::new(), CancelToken::new());
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        let o = Options {
            region: [0.0, 0.0, 2.0, 5.0],
            ..options()
        };
        let mut surface = Surface::new(&o, &crate::svg::Camera::new(&o).unwrap(), &mut cx).unwrap();
        let mut path = Path::new(true);
        path.polygon(&[[0.0, 0.5], [5.0, 0.5], [5.0, 1.75], [0.0, 1.75]], &mut cx)
            .unwrap();
        let mut clip = Path::new(true);
        clip.polygon(
            &[[1.75, 0.0], [3.75, 0.0], [3.75, 5.0], [1.75, 5.0]],
            &mut cx,
        )
        .unwrap();
        let mut hole = Path::new(true);
        hole.inverse = true;
        hole.polygon(
            &[[2.25, 0.75], [2.75, 0.75], [2.75, 1.25], [2.25, 1.25]],
            &mut cx,
        )
        .unwrap();
        let clips = [clip, hole];
        surface
            .paint(Shape::Path(&path), &clips, [255; 4], &mut cx)
            .unwrap();
        // 表示範囲 x=[1.5,3.5)、クリップ後 x=[1.75,3.5)。穴は中央の上下 2 サンプルずつを除く。
        let alpha: Vec<_> = surface.pixels.iter().map(|p| p[3]).collect();
        assert_eq!(&alpha[..10], &[0, 32, 96, 64, 0, 0, 48, 159, 96, 0]);
        assert!(alpha[10..].iter().all(|&a| a == 0));
        cancel.cancel();
        assert_eq!(
            surface.paint(Shape::Path(&Path::new(true)), &clips, [255; 4], &mut cx),
            Err(Error::Stopped(Stop::Cancelled))
        );
    }

    #[test]
    fn subpixel_clip_intersects_samples_before_alpha() {
        let (mut budget, mut diag, cancel) =
            (Budget::default(), Diagnostics::new(), CancelToken::new());
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        let mut surface = test_surface(&mut cx);
        let mut half = Path::new(true);
        half.polygon(&[[0.0, 0.0], [0.5, 0.0], [0.5, 1.0], [0.0, 1.0]], &mut cx)
            .unwrap();
        surface
            .paint(Shape::Path(&half), &[], [255, 0, 0, 255], &mut cx)
            .unwrap();
        assert_eq!(surface.pixels[0], [255, 0, 0, 128]);
        let mut other = Path::new(true);
        other
            .polygon(&[[0.5, 0.0], [1.0, 0.0], [1.0, 1.0], [0.5, 1.0]], &mut cx)
            .unwrap();
        surface.pixels[0] = [0; 4];
        surface
            .paint(Shape::Path(&half), &[other], [255, 0, 0, 255], &mut cx)
            .unwrap();
        assert_eq!(surface.pixels[0], [0; 4]);
    }

    #[test]
    fn miter_joins_and_overlapping_stroke_parts_blend_once() {
        let (mut budget, mut diag, cancel) =
            (Budget::default(), Diagnostics::new(), CancelToken::new());
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        for points in [
            [[1.0, 1.0], [3.0, 1.0], [3.0, 3.0]],
            [[3.0, 3.0], [3.0, 1.0], [1.0, 1.0]],
        ] {
            let mut path = Path::new(false);
            path.stroke(&points, 2.0, false, &mut cx).unwrap();
            let mut surface = test_surface(&mut cx);
            surface
                .paint(Shape::Path(&path), &[], [255, 0, 0, 128], &mut cx)
                .unwrap();
            assert_eq!(surface.pixels[3], [255, 0, 0, 128]); // 外側の miter。
            assert_eq!(surface.pixels[7], [255, 0, 0, 128]); // 内側の四角形の重なり。
            assert_eq!(surface.pixels[4], [0; 4]);
        }
    }

    #[test]
    fn circle_annulus_and_source_over_have_fixed_samples() {
        let (mut budget, mut diag, cancel) =
            (Budget::default(), Diagnostics::new(), CancelToken::new());
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        let mut surface = test_surface(&mut cx);
        surface
            .paint(
                Shape::Circle {
                    at: [0.5, 0.5],
                    outer: 0.5,
                    inner: 0.0,
                },
                &[],
                [10, 20, 30, 255],
                &mut cx,
            )
            .unwrap();
        assert_eq!(surface.pixels[0], [10, 20, 30, 191]);
        surface
            .paint(
                Shape::Circle {
                    at: [2.5, 2.5],
                    outer: 2.0,
                    inner: 1.0,
                },
                &[],
                [255, 0, 0, 255],
                &mut cx,
            )
            .unwrap();
        assert_eq!(surface.pixels[12], [0; 4]);
        let mut pixel = [255, 0, 0, 128];
        composite(&mut pixel, [0, 0, 255, 128], 16);
        assert_eq!(pixel, [85, 0, 170, 192]);
    }

    #[test]
    fn cancellation_after_surface_allocation_stops_the_first_scan() {
        let (mut budget, mut diag, cancel) =
            (Budget::default(), Diagnostics::new(), CancelToken::new());
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        let mut surface = test_surface(&mut cx);
        cancel.cancel();
        assert_eq!(
            surface.paint(
                Shape::Circle {
                    at: [2.5, 2.5],
                    outer: 2.0,
                    inner: 0.0
                },
                &[],
                [255; 4],
                &mut cx
            ),
            Err(Error::Stopped(Stop::Cancelled))
        );
        assert!(surface.pixels.iter().all(|p| *p == [0; 4]));
    }

    #[test]
    fn large_circle_keeps_samples_near_the_tangent() {
        let (mut budget, mut diag, cancel) =
            (Budget::default(), Diagnostics::new(), CancelToken::new());
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        let mut surface = test_surface(&mut cx);
        surface
            .paint(
                Shape::Circle {
                    at: [-15811.263301780698, 1_000_000_000.3],
                    outer: 1_000_000_000.3,
                    inner: 0.0,
                },
                &[],
                [255; 4],
                &mut cx,
            )
            .unwrap();
        // 60 桁の十進計算では y=0.125 の右端は 0.125000938800259…。
        // 1 行目の最初のサンプルと、ほかの 3 行の全サンプルが入り、13/16 になる。
        assert_eq!(surface.pixels[0], [255, 255, 255, 207]);
    }

    #[test]
    fn winding_rule_distinguishes_faces_and_glyphs() {
        let (mut budget, mut diag, cancel) =
            (Budget::default(), Diagnostics::new(), CancelToken::new());
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        for (evenodd, reverse, covered) in [
            (true, false, false),
            (false, false, true),
            (false, true, false),
        ] {
            let mut path = Path::new(evenodd);
            path.polygon(&[[0.0, 0.0], [5.0, 0.0], [5.0, 5.0], [0.0, 5.0]], &mut cx)
                .unwrap();
            let mut inner = [[1.0, 1.0], [4.0, 1.0], [4.0, 4.0], [1.0, 4.0]];
            if reverse {
                inner.reverse();
            }
            path.polygon(&inner, &mut cx).unwrap();
            let mut surface = test_surface(&mut cx);
            surface
                .paint(Shape::Path(&path), &[], [255; 4], &mut cx)
                .unwrap();
            assert_eq!(surface.pixels[12], if covered { [255; 4] } else { [0; 4] });
        }
    }

    #[test]
    fn butt_caps_closed_join_and_miter_limit_keep_the_stroke_contract() {
        let (mut budget, mut diag, cancel) =
            (Budget::default(), Diagnostics::new(), CancelToken::new());
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        let mut path = Path::new(false);
        path.stroke(&[[1.0, 2.0], [4.0, 2.0]], 1.0, false, &mut cx)
            .unwrap();
        let mut surface = test_surface(&mut cx);
        surface
            .paint(Shape::Path(&path), &[], [255; 4], &mut cx)
            .unwrap();
        assert_eq!(surface.pixels[10], [0; 4]);
        let mut path = Path::new(false);
        path.stroke(
            &[[1.0, 1.0], [4.0, 1.0], [4.0, 4.0], [1.0, 4.0]],
            1.0,
            true,
            &mut cx,
        )
        .unwrap();
        let mut surface = test_surface(&mut cx);
        surface
            .paint(Shape::Path(&path), &[], [255; 4], &mut cx)
            .unwrap();
        assert_eq!(surface.pixels[0], [255, 255, 255, 64]);
        assert_eq!(surface.pixels[12], [0; 4]);
        let mut path = Path::new(false);
        path.stroke(&[[1.0, 5.0], [2.0, 2.0], [2.1, 5.0]], 1.0, false, &mut cx)
            .unwrap();
        let mut surface = test_surface(&mut cx);
        surface
            .paint(Shape::Path(&path), &[], [255; 4], &mut cx)
            .unwrap();
        assert!(surface.pixels[..5].iter().all(|p| *p == [0; 4]));
        assert_ne!(surface.pixels[12], [0; 4]);
    }
}
