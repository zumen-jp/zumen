//! 点の共有・退化・T 字の継ぎ目と、描く稜線を決まった順で処理する。

use std::collections::BTreeMap;

use zumen_base::Cx;

use super::{
    MeshChanges,
    budget::{self, push, reserve, work},
};
use crate::{Failure, Mesh, Xyz, cross, dot, length, normalize, sub};

fn key(a: u32, b: u32) -> (u32, u32) {
    if a < b { (a, b) } else { (b, a) }
}

pub(super) fn snapped(
    mesh: &mut Mesh,
    grid: f64,
    changes: &mut MeshChanges,
    cx: &mut Cx,
) -> Result<(), Failure> {
    work(cx, mesh.positions.len() as u64)?;
    for p in &mut mesh.positions {
        let old = *p;
        for v in p.iter_mut() {
            let q = *v / grid;
            if !q.is_finite() || q.abs() > 4_503_599_627_370_495.0 {
                return Err(Failure::Invalid("grid_range"));
            }
            *v = libm::round(q) * grid;
            if *v == 0.0 {
                *v = 0.0;
            }
        }
        changes.snapped_vertices += u64::from(old != *p);
    }
    Ok(())
}

pub(super) fn clean(
    mesh: &mut Mesh,
    changes: &mut MeshChanges,
    cx: &mut Cx,
) -> Result<(), Failure> {
    if mesh.positions.len() > budget::MAX_POINTS || mesh.triangles.len() > budget::MAX_TRIANGLES {
        return Err(Failure::Invalid("mesh_count"));
    }
    work(
        cx,
        (mesh.positions.len() + mesh.triangles.len() + mesh.edges.len()) as u64,
    )?;
    mesh.validate()?;
    let mut index = BTreeMap::new();
    budget::bytes(cx, mesh.positions.len() as u64 * 80)?;
    let mut positions = Vec::new();
    let mut remap = Vec::new();
    reserve(&mut positions, mesh.positions.len(), 24, cx)?;
    reserve(&mut remap, mesh.positions.len(), 4, cx)?;
    for p in &mesh.positions {
        // -0 と +0 は同じ点。入力順で番号を付け、ハッシュの反復順は使わない。
        let p = p.map(|v| if v == 0.0 { 0.0 } else { v });
        let next = positions.len() as u32;
        let i = *index.entry(p.map(f64::to_bits)).or_insert_with(|| {
            positions.push(p);
            next
        });
        remap.push(i);
    }
    let at = |i: u32| {
        remap
            .get(i as usize)
            .copied()
            .ok_or(Failure::Invalid("index"))
    };
    for t in &mut mesh.triangles {
        *t = [at(t[0])?, at(t[1])?, at(t[2])?];
    }
    for e in &mut mesh.edges {
        *e = [at(e[0])?, at(e[1])?];
    }
    mesh.positions = positions;
    let positions = &mesh.positions;
    mesh.triangles.retain(|t| {
        let a = positions.get(t[0] as usize).copied().unwrap_or_default();
        let b = positions.get(t[1] as usize).copied().unwrap_or_default();
        let c = positions.get(t[2] as usize).copied().unwrap_or_default();
        let n = cross(sub(b, a), sub(c, a));
        let keep = n != [0.0; 3];
        if !keep {
            changes.dropped_faces += 1;
        }
        keep
    });
    Ok(())
}

fn interior(p: Xyz, a: Xyz, b: Xyz) -> bool {
    let ab = sub(b, a);
    let ap = sub(p, a);
    let length2 = dot(ab, ab);
    if !length2.is_finite() || length2 <= 0.0 {
        return false;
    }
    let t = dot(ap, ab) / length2;
    let d = cross(ap, ab);
    t > 0.0 && t < 1.0 && dot(d, d) <= 1e-24 * length2 * length2
}

fn edge_counts(mesh: &Mesh, cx: &mut Cx) -> Result<BTreeMap<(u32, u32), u32>, Failure> {
    work(cx, mesh.triangles.len() as u64 * 3)?;
    budget::bytes(cx, mesh.triangles.len() as u64 * 3 * 64)?;
    let mut edges = BTreeMap::new();
    for t in &mesh.triangles {
        for edge in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            *edges.entry(edge).or_insert(0) += 1;
        }
    }
    Ok(edges)
}

pub(super) fn repair(
    mesh: &mut Mesh,
    changes: &mut MeshChanges,
    cx: &mut Cx,
) -> Result<(), Failure> {
    let mut edges = edge_counts(mesh, cx)?;
    let mut at = 0;
    while let Some(&t) = mesh.triangles.get(at) {
        work(cx, 3)?;
        let mut split = false;
        for (a, b, c) in [(t[0], t[1], t[2]), (t[1], t[2], t[0]), (t[2], t[0], t[1])] {
            if edges.get(&(a, b)) == edges.get(&(b, a)) {
                continue;
            }
            let pa = mesh
                .positions
                .get(a as usize)
                .copied()
                .ok_or(Failure::Invalid("index"))?;
            let pb = mesh
                .positions
                .get(b as usize)
                .copied()
                .ok_or(Failure::Invalid("index"))?;
            let mut found = None;
            for (k, &p) in mesh.positions.iter().enumerate() {
                work(cx, 1)?;
                let k = k as u32;
                if !t.contains(&k) && interior(p, pa, pb) {
                    found = Some(k);
                    break;
                }
            }
            if let Some(k) = found {
                // 2 面へ分けても面積と向きは保つ。追加した面も同じ走査で確かめる。
                if let Some(t) = mesh.triangles.get_mut(at) {
                    *t = [a, k, c];
                }
                budget::triangle(mesh, [k, b, c], cx)?;
                for edge in [(a, b), (b, c), (c, a)] {
                    if let Some(n) = edges.get_mut(&edge) {
                        *n -= 1;
                        if *n == 0 {
                            edges.remove(&edge);
                        }
                    }
                }
                budget::bytes(cx, 6 * 64)?;
                for edge in [(a, k), (k, c), (c, a), (k, b), (b, c), (c, k)] {
                    *edges.entry(edge).or_insert(0) += 1;
                }
                changes.split_edges += 1;
                // 同じ面の別の辺も再検査する。分割ごとに triangle が上限を検査する。
                split = true;
                break;
            }
        }
        if !split {
            at += 1;
        }
    }
    Ok(())
}

/// 辺で接する複数の閉じた殻も許す。正逆の回数が同じでなければ閉じていない。
pub(super) fn watertight(mesh: &Mesh, cx: &mut Cx) -> Result<bool, Failure> {
    let edges = edge_counts(mesh, cx)?;
    Ok(!edges.is_empty()
        && edges
            .iter()
            .all(|(&(a, b), n)| edges.get(&(b, a)) == Some(n)))
}

/// 体積が丸め誤差の上限を超えるか。両面の平面の微小な残差を立体とみなさない。
pub(super) fn positive_volume(mesh: &Mesh, cx: &mut Cx) -> Result<bool, Failure> {
    work(cx, mesh.triangles.len() as u64)?;
    let anchor = mesh.positions.first().copied().unwrap_or_default();
    let mut sum = 0.0;
    let mut absolute = 0.0;
    for &t in &mesh.triangles {
        let [a, b, c] = mesh.triangle(t).ok_or(Failure::Invalid("index"))?;
        let (a, b, c) = (sub(a, anchor), sub(b, anchor), sub(c, anchor));
        let terms = [
            a[0] * b[1] * c[2],
            -a[0] * b[2] * c[1],
            a[1] * b[2] * c[0],
            -a[1] * b[0] * c[2],
            a[2] * b[0] * c[1],
            -a[2] * b[1] * c[0],
        ];
        for term in terms {
            sum += term;
            absolute += term.abs();
        }
    }
    let error = 32.0 * f64::EPSILON * mesh.triangles.len() as f64 * absolute;
    Ok(sum.is_finite() && error.is_finite() && sum > error)
}

pub(super) fn edges(mesh: &mut Mesh, cx: &mut Cx) -> Result<(), Failure> {
    work(cx, mesh.triangles.len() as u64 * 3)?;
    budget::bytes(cx, mesh.triangles.len() as u64 * 3 * 96)?;
    let mut edges: BTreeMap<(u32, u32), (Xyz, u32, bool)> = BTreeMap::new();
    for &t in &mesh.triangles {
        let [a, b, c] = mesh.triangle(t).ok_or(Failure::Invalid("index"))?;
        let normal = normalize(cross(sub(b, a), sub(c, a))).ok_or(Failure::Invalid("normal"))?;
        for edge in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            let entry = edges
                .entry(key(edge.0, edge.1))
                .or_insert((normal, 0, false));
            entry.1 += 1;
            entry.2 |= length(cross(entry.0, normal)) > 1e-8 || dot(entry.0, normal) < 0.0;
        }
    }
    mesh.edges.clear();
    for ((a, b), (_, count, crease)) in edges {
        if count != 2 || crease {
            push(&mut mesh.edges, [a, b], 8, cx)?;
        }
    }
    Ok(())
}
