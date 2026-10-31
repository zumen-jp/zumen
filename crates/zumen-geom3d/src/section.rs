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

type Key = [[u64; 3]; 2];

/// 辺の交点。端点を座標の順に並べてから求め、どちらの三角形からも同じ値にする
fn crossing(a: (Xyz, f64), b: (Xyz, f64)) -> (Key, Xyz) {
    let (ka, kb) = (a.0.map(f64::to_bits), b.0.map(f64::to_bits));
    let ((p, dp), (q, dq), key) = if ka <= kb {
        (a, b, [ka, kb])
    } else {
        (b, a, [kb, ka])
    };
    let t = dp / (dp - dq);
    (key, add(p, scale(sub(q, p), t)))
}

impl Section for MeshSection {
    fn section(&self, mesh: &Mesh, plane: &Plane, cx: &mut Cx) -> Result<Vec<Vec<Xy>>, Failure> {
        mesh.validate()?;
        let frame = plane.frame().ok_or(Failure::Invalid("plane"))?;
        let normal = frame.column(2);
        let d = |p: Xyz| dot(sub(p, plane.origin), normal);
        // 始まりの交点の辺 → (終わりの交点の辺, 始まりの点, 終わりの点)
        let mut next: BTreeMap<Key, Vec<(Key, Xyz, Xyz)>> = BTreeMap::new();
        let mut count = 0u64;
        for t in &mesh.triangles {
            let Some(points) = mesh.triangle(*t) else {
                continue;
            };
            let side = points.map(|p| (p, d(p)));
            // 外から見て反時計回りの三角形では、表から裏へ渡る辺の交点から、裏から表へ渡る辺の
            // 交点へ向かうと、平面の表から見て中身が左になる。向きを点の符号だけで決めるので、
            // 長さが 0 の辺でも、隣の三角形とつながる
            let (mut start, mut end) = (None, None);
            for (i, j) in [(0, 1), (1, 2), (2, 0)] {
                let (Some(a), Some(b)) = (side.get(i), side.get(j)) else {
                    continue;
                };
                match (a.1 > 0.0, b.1 > 0.0) {
                    (true, false) => start = Some(crossing(*a, *b)),
                    (false, true) => end = Some(crossing(*a, *b)),
                    _ => {}
                }
            }
            let (Some((start, from)), Some((end, to))) = (start, end) else {
                continue;
            };
            next.entry(start).or_default().push((end, from, to));
            count += 1;
        }
        cx.check(Kind::Segments, count)?;
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
            let mut chain = Vec::new();
            let mut at = start;
            let mut last = None;
            while let Some((end, from, to)) = next.get_mut(&at).and_then(Vec::pop) {
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
                loops.push(chain);
            }
        }
        Ok(loops)
    }
}
