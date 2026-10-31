//! 3D のカーネルの候補を、合成の形で比べる（#414）。`compare` と `size.sh` が使う

pub mod kernels;
mod primitives;

use zumen_base::{Budget, CancelToken, Cx, Diagnostics};
use zumen_geom3d::cases;
use zumen_geom3d::{Failure, Kernel, Mesh, Tolerance};

use kernels::Candidate;

/// 入れた候補（名前とカーネル）
pub fn candidates() -> Vec<(&'static str, Box<dyn Kernel>)> {
    let mut out: Vec<(&'static str, Box<dyn Kernel>)> = Vec::new();
    #[cfg(feature = "own")]
    out.push(("own", Box::new(Candidate(kernels::Own))));
    #[cfg(feature = "manifold")]
    out.push(("manifold-rust", Box::new(Candidate(kernels::ManifoldRust))));
    #[cfg(feature = "csgrs")]
    out.push(("csgrs", Box::new(Candidate(kernels::Csgrs))));
    if out.is_empty() {
        out.push(("none", Box::new(Candidate(kernels::Nothing))));
    }
    out
}

/// 比べるときの許容差
pub const TOLERANCE: Tolerance = Tolerance {
    chord: cases::CHORD,
    length: 1e-9,
};

/// 形を評価する
pub fn evaluate(kernel: &dyn Kernel, shape: &zumen_geom3d::Shape) -> Result<Mesh, Failure> {
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
    kernel.evaluate(shape, TOLERANCE, &mut cx)
}

/// 出力のメッシュの指紋（FNV-1a、点の座標のビットと三角形の番号）。環境で結果が変わらないかを比べる
pub fn fingerprint(mesh: &Mesh) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |v: u64| {
        for b in v.to_le_bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    };
    for p in &mesh.positions {
        p.iter().for_each(|v| eat(v.to_bits()));
    }
    for t in &mesh.triangles {
        t.iter().for_each(|i| eat(u64::from(*i)));
    }
    h
}

/// wasm の大きさを測るための入口。候補と組の番号で評価し、指紋を返す（失敗は 0）
#[unsafe(no_mangle)]
pub extern "C" fn zm_bench(candidate: u32, case: u32) -> u64 {
    let candidates = candidates();
    let (Some((_, kernel)), Some(case)) = (
        candidates.get(candidate as usize),
        cases::all().get(case as usize).cloned(),
    ) else {
        return 0;
    };
    evaluate(kernel.as_ref(), &case.shape)
        .map(|m| fingerprint(&m))
        .unwrap_or(0)
}
