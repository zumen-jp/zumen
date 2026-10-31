//! 配置の連鎖（決定 0207 の「配置」）。親の配置と局所の変換をたどり、プロジェクトの空間への変換を求める。

use std::collections::BTreeSet;

use zumen_base::{Cx, Diagnostic, Stop, codes};

use crate::{Building, ElementRef, Local};

/// 3 次元のアフィン変換。行ごとに、回転の 3 つの成分と移動を持つ（`p' = M p`）
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform(pub [[f64; 4]; 3]);

impl Transform {
    /// 恒等の変換
    pub const IDENTITY: Self = Self([
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
    ]);

    /// 移動だけの変換
    pub fn translation(x: f64, y: f64, z: f64) -> Self {
        Self([[1.0, 0.0, 0.0, x], [0.0, 1.0, 0.0, y], [0.0, 0.0, 1.0, z]])
    }

    /// `self` の後に `other` を掛けた変換（先に `other`、次に `self`）
    pub fn then(&self, other: &Self) -> Self {
        let [r0, r1, r2] = self.0;
        let [o0, o1, o2] = other.0;
        let row = |r: [f64; 4]| {
            let [a, b, c, t] = r;
            let col = |k: usize| {
                let pick = |row: [f64; 4]| row.get(k).copied().unwrap_or(0.0);
                a * pick(o0) + b * pick(o1) + c * pick(o2)
            };
            [col(0), col(1), col(2), col(3) + t]
        };
        Self([row(r0), row(r1), row(r2)])
    }

    /// 点を写す
    pub fn apply(&self, point: [f64; 3]) -> [f64; 3] {
        let [x, y, z] = point;
        let row = |r: [f64; 4]| {
            let [a, b, c, t] = r;
            a * x + b * y + c * z + t
        };
        let [r0, r1, r2] = self.0;
        [row(r0), row(r1), row(r2)]
    }
}

impl Building {
    /// 要素の、プロジェクトの空間への変換。連鎖をたどる深さを `depth` の予算で数える。
    ///
    /// 配置を持たない要素は `None`。連鎖が循環するか、親が配置を持たないか、解けない配置を含めば、`None` にして
    /// 診断を返す。
    pub fn world_placement(
        &self,
        element: ElementRef,
        cx: &mut Cx<'_>,
    ) -> Result<Option<Transform>, Stop> {
        let mut entered = 0;
        let result = self.walk(element, cx, &mut entered);
        for _ in 0..entered {
            cx.leave();
        }
        result
    }

    fn walk(
        &self,
        element: ElementRef,
        cx: &mut Cx<'_>,
        entered: &mut u32,
    ) -> Result<Option<Transform>, Stop> {
        let mut chain = Vec::new();
        let mut seen = BTreeSet::new();
        let mut current = element;
        loop {
            cx.enter()?;
            *entered += 1;
            if !seen.insert(current) {
                cx.report(
                    Diagnostic::new(codes::BUILDING_PLACEMENT_INVALID).at(self.place(element)),
                );
                return Ok(None);
            }
            let Some(placement) = self.components.placement.get(current) else {
                if current != element {
                    cx.report(
                        Diagnostic::new(codes::BUILDING_PLACEMENT_MISSING)
                            .with("id", self.label(current))
                            .at(self.place(element)),
                    );
                }
                return Ok(None);
            };
            match &placement.local {
                Local::Transform(local) => chain.push(*local),
                Local::Unsupported(kind) => {
                    cx.report(
                        Diagnostic::new(codes::BUILDING_PLACEMENT_UNSUPPORTED)
                            .with("kind", kind.as_str())
                            .at(self.place(element)),
                    );
                    return Ok(None);
                }
            }
            match placement.relative_to {
                Some(parent) => current = parent,
                None => break,
            }
        }
        // 外側の配置から順に掛ける
        let world = chain
            .iter()
            .rev()
            .fold(Transform::IDENTITY, |outer, local| outer.then(local));
        Ok(Some(world))
    }

    pub(crate) fn label(&self, element: ElementRef) -> String {
        match self.id(element) {
            Some(id) => id.to_string(),
            None => format!("#{}", element.0),
        }
    }
}
