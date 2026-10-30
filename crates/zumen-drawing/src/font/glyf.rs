//! `glyf` の字形の輪郭を読む（決定 0188）。OpenType の仕様の glyf の節に従う。
//!
//! 受け取ったファイルは信用しない。複合の字形の入れ子は [`DEPTH`] 段に、1 つの字形で読む点と部品の数は
//! [`WORK`] に抑える。同じ部品を何度も参照しても、作業量は上限で止まる。部品は、位置のずれか、
//! 点の番号で合わせて置く（決定 0189）。

use super::{Contour, Segment, i16_at, slice, u16_at, u32_at};

/// 複合の字形の入れ子の上限
const DEPTH: u32 = 8;
/// 1 つの字形で読む点と部品の数の上限。上限を超えた字形は読めないものとする
const WORK: u32 = 1 << 16;

/// 字形の区間。空の字形は空の区間。番号が `loca` の外か、区間が逆か `glyf` の外なら `None`
fn range<'a>(loca: &[u8], long_loca: bool, glyf: &'a [u8], id: u16) -> Option<&'a [u8]> {
    let (start, end) = if long_loca {
        let at = usize::from(id) * 4;
        (u32_at(loca, at)?, u32_at(loca, at + 4)?)
    } else {
        let at = usize::from(id) * 2;
        let [start, end] = [u16_at(loca, at)?, u16_at(loca, at + 2)?];
        (u32::from(start) * 2, u32::from(end) * 2)
    };
    if start > end {
        return None;
    }
    slice(glyf, u64::from(start), u64::from(end - start))
}

/// 字形の表と、読んだ作業量
struct Reader<'a> {
    loca: &'a [u8],
    long_loca: bool,
    glyf: &'a [u8],
    glyph_count: u16,
    work: u32,
    /// 写した後の点（輪郭の点の番号の順）。複合の字形の点の番号は、この並びの区間で引く
    points: Vec<[f64; 2]>,
}

/// 字形の輪郭。字形が読めなければ `None`、何も描かない字形は空
pub(super) fn outline(
    loca: &[u8],
    long_loca: bool,
    glyf: &[u8],
    glyph_count: u16,
    id: u16,
) -> Option<Vec<Contour>> {
    let mut reader = Reader {
        loca,
        long_loca,
        glyf,
        glyph_count,
        work: 0,
        points: Vec::new(),
    };
    let mut out = Vec::new();
    reader.glyph(id, [1.0, 0.0, 0.0, 1.0, 0.0, 0.0], 0, &mut out)?;
    Some(out)
}

impl Reader<'_> {
    fn spend(&mut self, amount: u32) -> Option<()> {
        self.work = self.work.saturating_add(amount);
        (self.work <= WORK).then_some(())
    }

    /// 字形を変換 `[a, b, c, d, e, f]`（`x' = a·x + c·y + e`、`y' = b·x + d·y + f`）で写して足す
    fn glyph(
        &mut self,
        id: u16,
        transform: [f64; 6],
        depth: u32,
        out: &mut Vec<Contour>,
    ) -> Option<()> {
        if id >= self.glyph_count || depth > DEPTH {
            return None;
        }
        let data = range(self.loca, self.long_loca, self.glyf, id)?;
        if data.is_empty() {
            return Some(());
        }
        let contours = i16_at(data, 0)?;
        if contours >= 0 {
            self.simple(data, contours.unsigned_abs(), transform, out)
        } else {
            self.composite(data, transform, depth, out)
        }
    }

    /// 単純な字形（glyf の節の Simple glyph description）
    fn simple(
        &mut self,
        data: &[u8],
        contours: u16,
        transform: [f64; 6],
        out: &mut Vec<Contour>,
    ) -> Option<()> {
        if contours == 0 {
            return Some(());
        }
        // 見出し 10 バイトの後に endPtsOfContours・instructionLength・命令・旗・X・Y
        let mut ends = Vec::with_capacity(usize::from(contours));
        let mut previous: Option<u16> = None;
        for i in 0..usize::from(contours) {
            let end = u16_at(data, 10 + 2 * i)?;
            if previous.is_some_and(|p| end <= p) {
                return None;
            }
            previous = Some(end);
            ends.push(end);
        }
        let count = usize::from(*ends.last()?) + 1;
        self.spend(u32::try_from(count).ok()?)?;
        let instructions = u16_at(data, 10 + 2 * usize::from(contours))?;
        let mut at = 12 + 2 * usize::from(contours) + usize::from(instructions);

        let mut flags = Vec::with_capacity(count);
        while flags.len() < count {
            let flag = *data.get(at)?;
            at += 1;
            let mut repeat = 0;
            if flag & 0x08 != 0 {
                repeat = *data.get(at)?;
                at += 1;
            }
            for _ in 0..=repeat {
                if flags.len() == count {
                    return None;
                }
                flags.push(flag);
            }
        }
        let mut read = |short: u8, same: u8| -> Option<Vec<f64>> {
            let mut values = Vec::with_capacity(count);
            let mut value = 0i32;
            for &flag in &flags {
                if flag & short != 0 {
                    let delta = i32::from(*data.get(at)?);
                    at += 1;
                    value += if flag & same != 0 { delta } else { -delta };
                } else if flag & same == 0 {
                    value += i32::from(i16_at(data, at)?);
                    at += 2;
                }
                values.push(f64::from(value));
            }
            Some(values)
        };
        let xs = read(0x02, 0x10)?;
        let ys = read(0x04, 0x20)?;

        let [a, b, c, d, e, f] = transform;
        let points: Vec<([f64; 2], bool)> = xs
            .iter()
            .zip(&ys)
            .zip(&flags)
            .map(|((&x, &y), &flag)| ([a * x + c * y + e, b * x + d * y + f], flag & 0x01 != 0))
            .collect();
        self.points.extend(points.iter().map(|(p, _)| *p));
        let mut start = 0;
        for end in ends {
            let end = usize::from(end) + 1;
            if let Some(contour) = quadratic(points.get(start..end)?) {
                out.push(contour);
            }
            start = end;
        }
        Some(())
    }

    /// 複合の字形（glyf の節の Composite glyph description）
    fn composite(
        &mut self,
        data: &[u8],
        transform: [f64; 6],
        depth: u32,
        out: &mut Vec<Contour>,
    ) -> Option<()> {
        const WORDS: u16 = 0x0001;
        const XY: u16 = 0x0002;
        const SCALE: u16 = 0x0008;
        const MORE: u16 = 0x0020;
        const XY_SCALE: u16 = 0x0040;
        const TWO_BY_TWO: u16 = 0x0080;
        const SCALED_OFFSET: u16 = 0x0800;
        const UNSCALED_OFFSET: u16 = 0x1000;
        // この字形の点の番号は、最初の部品の点から数える
        let base = self.points.len();
        let mut at = 10;
        loop {
            self.spend(1)?;
            let flags = u16_at(data, at)?;
            let id = u16_at(data, at + 2)?;
            at += 4;
            // 引数は、ARGS_ARE_XY_VALUES なら符号つきの位置のずれ、でなければ符号なしの点の番号
            let words = flags & WORDS != 0;
            let args: [i32; 2] = match (words, flags & XY != 0) {
                (true, true) => [i16_at(data, at)?.into(), i16_at(data, at + 2)?.into()],
                (true, false) => [u16_at(data, at)?.into(), u16_at(data, at + 2)?.into()],
                (false, true) => [
                    i32::from(*data.get(at)? as i8),
                    i32::from(*data.get(at + 1)? as i8),
                ],
                (false, false) => [(*data.get(at)?).into(), (*data.get(at + 1)?).into()],
            };
            at += if words { 4 } else { 2 };
            let f2dot14 = |at: usize| i16_at(data, at).map(|v| f64::from(v) / 16384.0);
            let [sa, sb, sc, sd] = if flags & SCALE != 0 {
                let s = f2dot14(at)?;
                at += 2;
                [s, 0.0, 0.0, s]
            } else if flags & XY_SCALE != 0 {
                let v = [f2dot14(at)?, f2dot14(at + 2)?];
                at += 4;
                [v[0], 0.0, 0.0, v[1]]
            } else if flags & TWO_BY_TWO != 0 {
                let v = [
                    f2dot14(at)?,
                    f2dot14(at + 2)?,
                    f2dot14(at + 4)?,
                    f2dot14(at + 6)?,
                ];
                at += 8;
                v
            } else {
                [1.0, 0.0, 0.0, 1.0]
            };
            // 位置のずれは、SCALED_COMPONENT_OFFSET だけが立てば部品の変換で写し、それ以外（どちらも
            // 立たないか、UNSCALED_COMPONENT_OFFSET が立つ）は写さない。点の番号で合わせるときは、
            // まずずれなしで置き、後で点が重なるように動かす
            let (dx, dy) = if flags & XY == 0 {
                (0.0, 0.0)
            } else {
                let (dx, dy) = (f64::from(args[0]), f64::from(args[1]));
                if flags & (SCALED_OFFSET | UNSCALED_OFFSET) == SCALED_OFFSET {
                    (sa * dx + sc * dy, sb * dx + sd * dy)
                } else {
                    (dx, dy)
                }
            };
            // 部品の変換を、親の変換に重ねる
            let [a, b, c, d, e, f] = transform;
            let child = [
                a * sa + c * sb,
                b * sa + d * sb,
                a * sc + c * sd,
                b * sc + d * sd,
                a * dx + c * dy + e,
                b * dx + d * dy + f,
            ];
            let (first_point, first_contour) = (self.points.len(), out.len());
            self.glyph(id, child, depth + 1, out)?;
            if flags & XY == 0 {
                // 親の点（前の部品までの点）の番号 args[0] と、部品の点の番号 args[1] を重ねる
                let parent = base.checked_add(usize::try_from(args[0]).ok()?)?;
                let own = first_point.checked_add(usize::try_from(args[1]).ok()?)?;
                if parent >= first_point {
                    return None;
                }
                let [px, py] = *self.points.get(parent)?;
                let [qx, qy] = *self.points.get(own)?;
                let shift = [px - qx, py - qy];
                for point in self.points.get_mut(first_point..)? {
                    *point = moved(*point, shift);
                }
                for contour in out.get_mut(first_contour..)? {
                    translate(contour, shift);
                }
            }
            if flags & MORE == 0 {
                return Some(());
            }
        }
    }
}

/// 点を `shift` だけ動かす
fn moved([x, y]: [f64; 2], [dx, dy]: [f64; 2]) -> [f64; 2] {
    [x + dx, y + dy]
}

/// 輪郭を `shift` だけ動かす
fn translate(contour: &mut Contour, shift: [f64; 2]) {
    contour.start = moved(contour.start, shift);
    for segment in &mut contour.segments {
        *segment = match *segment {
            Segment::Line(p) => Segment::Line(moved(p, shift)),
            Segment::Quad(c, p) => Segment::Quad(moved(c, shift), moved(p, shift)),
            Segment::Cubic(c, d, p) => {
                Segment::Cubic(moved(c, shift), moved(d, shift), moved(p, shift))
            }
        };
    }
}

/// 2 次の輪郭の点（曲線の上の点かどうか）を、直線と 2 次曲線の区間にする。続く 2 つの曲線の外の点の間には、
/// その中点を曲線の上の点として補う
fn quadratic(points: &[([f64; 2], bool)]) -> Option<Contour> {
    let n = points.len();
    let first = points.first()?;
    let mid = |p: [f64; 2], q: [f64; 2]| [(p[0] + q[0]) / 2.0, (p[1] + q[1]) / 2.0];
    // 始点は、最初の曲線の上の点。なければ最初と最後の外の点の中点
    let (start, offset) = match points.iter().position(|(_, on)| *on) {
        Some(i) => (points.get(i)?.0, i + 1),
        None => (mid(points.get(n - 1)?.0, first.0), 0),
    };
    let mut segments = Vec::new();
    let mut control: Option<[f64; 2]> = None;
    for k in 0..n {
        let (p, on) = *points.get((offset + k) % n)?;
        match (on, control) {
            (true, None) => segments.push(Segment::Line(p)),
            (true, Some(c)) => {
                segments.push(Segment::Quad(c, p));
                control = None;
            }
            (false, None) => control = Some(p),
            (false, Some(c)) => {
                segments.push(Segment::Quad(c, mid(c, p)));
                control = Some(p);
            }
        }
    }
    match control {
        Some(c) => segments.push(Segment::Quad(c, start)),
        None => {
            // 最後の区間が始点に戻る直線なら、閉じるので要らない
            if matches!(segments.last(), Some(Segment::Line(p)) if *p == start) {
                segments.pop();
            }
        }
    }
    Some(Contour { start, segments })
}
