//! メッシュを平面で切る

use std::collections::BTreeMap;

use zumen_base::{Cx, Kind};

use crate::{Failure, Mesh, Plane, Section, Xy, Xyz, add, dot, scale, sub};

/// どのカーネルのメッシュにも使える、平面での切断。
///
/// 点の符号は、平面からの距離が正なら表、0 以下なら裏とする。平面の上の点は裏に寄せるので、
/// 切り口の辺はいつも三角形の辺の上の交点を結び、座標の同じ辺の交点は同じ値になる。交点は座標で
/// つなぐので、点の番号が分かれたメッシュでも輪になる。
#[derive(Clone, Copy, Debug, Default)]
pub struct MeshSection;

/// 切断の輪郭。開いた面の切り口は、終点を始点へ結ばない。
#[derive(Clone, Debug, PartialEq)]
pub struct SectionPath {
    /// 平面の局所座標の点。閉じた輪郭でも始点を末尾に重ねない。
    pub points: Vec<Xy>,
    /// 始点から終点までたどって、元の始点に戻ったか。
    pub closed: bool,
}

type Key = [[u64; 3]; 2];

/// 辺の交点。端点を座標の順に並べてから求め、どちらの三角形からも同じ値にする
fn crossing(a: (Xyz, f64), b: (Xyz, f64)) -> Result<(Key, Xyz), Failure> {
    let (ka, kb) = (a.0.map(f64::to_bits), b.0.map(f64::to_bits));
    let ((p, dp), (q, dq), key) = if ka <= kb {
        (a, b, [ka, kb])
    } else {
        (b, a, [kb, ka])
    };
    let denominator = dp - dq;
    if !denominator.is_finite() {
        return Err(Failure::Invalid("section_coordinate"));
    }
    let t = dp / denominator;
    Ok((key, add(p, scale(sub(q, p), t))))
}

impl Section for MeshSection {
    fn section(&self, mesh: &Mesh, plane: &Plane, cx: &mut Cx) -> Result<Vec<Vec<Xy>>, Failure> {
        Ok(self
            .paths(mesh, plane, cx)?
            .into_iter()
            .map(|p| p.points)
            .collect())
    }
}

impl MeshSection {
    /// 開いた鎖と閉じた輪郭を区別して切る。点の順は [`Section::section`] と同じ。
    pub fn paths(
        &self,
        mesh: &Mesh,
        plane: &Plane,
        cx: &mut Cx,
    ) -> Result<Vec<SectionPath>, Failure> {
        let n = mesh.triangles.len() as u64;
        // 交点の索引 2 つ、始点、鎖と出力を、確保前に固定の論理量で予約する。
        // 各三角形は最大 1 辺を作る。木の探索と走査も入力の上限から先に数える。
        let bytes = n
            .checked_mul(1536)
            .ok_or(Failure::Invalid("section_size"))?;
        if bytes > i32::MAX as u64 {
            return Err(Failure::Invalid("section_size"));
        }
        let levels = 65 - n.leading_zeros() as u64;
        cx.check(
            Kind::Work,
            n.saturating_mul(64 * levels)
                .saturating_add(mesh.positions.len() as u64)
                .saturating_add(mesh.edges.len() as u64),
        )?;
        cx.check(Kind::DecodedBytes, bytes)?;
        mesh.validate()?;
        let frame = plane.frame().ok_or(Failure::Invalid("plane"))?;
        let normal = frame.column(2);
        let d = |p: Xyz| dot(sub(p, plane.origin), normal);
        // 始まりの交点の辺 → (終わりの交点の辺, 始まりの点, 終わりの点)
        let mut next: BTreeMap<Key, Vec<(Key, Xyz, Xyz)>> = BTreeMap::new();
        for t in &mesh.triangles {
            cx.check(Kind::Work, 0)?;
            let Some(points) = mesh.triangle(*t) else {
                continue;
            };
            let side = points.map(|p| (p, d(p)));
            if side.iter().any(|(_, distance)| !distance.is_finite()) {
                return Err(Failure::Invalid("section_coordinate"));
            }
            // 外から見て反時計回りの三角形では、表から裏へ渡る辺の交点から、裏から表へ渡る辺の
            // 交点へ向かうと、平面の表から見て中身が左になる。向きを点の符号だけで決めるので、
            // 長さが 0 の辺でも、隣の三角形とつながる
            let (mut start, mut end) = (None, None);
            for (i, j) in [(0, 1), (1, 2), (2, 0)] {
                let (Some(a), Some(b)) = (side.get(i), side.get(j)) else {
                    continue;
                };
                match (a.1 > 0.0, b.1 > 0.0) {
                    (true, false) => start = Some(crossing(*a, *b)?),
                    (false, true) => end = Some(crossing(*a, *b)?),
                    _ => {}
                }
            }
            let (Some((start, from)), Some((end, to))) = (start, end) else {
                continue;
            };
            if !from.into_iter().chain(to).all(f64::is_finite) {
                return Err(Failure::Invalid("section_coordinate"));
            }
            cx.check(Kind::Segments, 1)?;
            next.entry(start).or_default().push((end, from, to));
        }
        let ends: BTreeMap<Key, ()> = next
            .values()
            .flatten()
            .map(|(end, _, _)| (*end, ()))
            .collect();
        // 開いた鎖（どの辺の終わりでもない始まり）から先にたどり、残りを輪にする
        let starts: Vec<Key> = next
            .keys()
            .filter(|k| !ends.contains_key(*k))
            .chain(next.keys())
            .copied()
            .collect();
        let mut loops = Vec::new();
        for start in starts {
            cx.check(Kind::Work, 0)?;
            let mut chain = Vec::new();
            let mut at = start;
            let mut last = None;
            while let Some((end, from, to)) = next.get_mut(&at).and_then(Vec::pop) {
                cx.check(Kind::Work, 0)?;
                chain.push(Plane::project(&frame, from));
                last = Some(to);
                at = end;
                if at == start {
                    break;
                }
            }
            if let Some(to) = last {
                if at != start {
                    // 開いた鎖は、最後の辺の終わりの点も持つ
                    chain.push(Plane::project(&frame, to));
                }
                // 頂点を通る所では、長さが 0 の辺ができる。同じ点を続けない
                chain.dedup();
                if at == start && chain.len() > 1 && chain.first() == chain.last() {
                    chain.pop();
                }
                if chain.iter().flatten().any(|v| !v.is_finite()) {
                    return Err(Failure::Invalid("section_coordinate"));
                }
                loops.push(SectionPath {
                    points: chain,
                    closed: at == start,
                });
            }
        }
        Ok(loops)
    }
}
