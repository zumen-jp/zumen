//! 投影した面の箱を、元の番号で同順位を決めて二分する。

use super::budget::{push, reserve, work};
use zumen_base::Cx;
use zumen_geom3d::Failure;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Bounds {
    pub min: [i64; 2],
    pub max: [i64; 2],
}

impl Bounds {
    pub fn points(points: impl IntoIterator<Item = [i64; 2]>) -> Option<Self> {
        let mut points = points.into_iter();
        let first = points.next()?;
        Some(points.fold(
            Self {
                min: first,
                max: first,
            },
            |mut out, p| {
                out.min = [out.min[0].min(p[0]), out.min[1].min(p[1])];
                out.max = [out.max[0].max(p[0]), out.max[1].max(p[1])];
                out
            },
        ))
    }
    fn union(self, other: Self) -> Self {
        Self {
            min: [self.min[0].min(other.min[0]), self.min[1].min(other.min[1])],
            max: [self.max[0].max(other.max[0]), self.max[1].max(other.max[1])],
        }
    }
    fn intersects(self, other: Self) -> bool {
        self.min[0] <= other.max[0]
            && other.min[0] <= self.max[0]
            && self.min[1] <= other.max[1]
            && other.min[1] <= self.max[1]
    }
}

#[derive(Clone, Copy)]
enum Children {
    Leaf(usize),
    Pair(usize, usize),
}
struct Node {
    bounds: Bounds,
    children: Children,
}

pub(super) struct Bvh {
    nodes: Vec<Node>,
    root: Option<usize>,
}

impl Bvh {
    pub fn new(bounds: &[Bounds], cx: &mut Cx<'_>) -> Result<Self, Failure> {
        let n = bounds.len() as u64;
        let levels = 65 - n.leading_zeros() as u64;
        work(cx, n.saturating_mul(levels * levels * 4))?;
        let mut order = Vec::new();
        reserve(&mut order, bounds.len(), 40, cx)?;
        order.extend(bounds.iter().copied().enumerate());
        let mut result = Self {
            nodes: Vec::new(),
            root: None,
        };
        let count = bounds
            .len()
            .checked_mul(2)
            .ok_or(Failure::Invalid("drawing_size"))?;
        reserve(&mut result.nodes, count, 64, cx)?;
        if !order.is_empty() {
            result.root = Some(result.build(&mut order, cx)?);
        }
        Ok(result)
    }

    fn build(&mut self, order: &mut [(usize, Bounds)], cx: &mut Cx<'_>) -> Result<usize, Failure> {
        work(cx, 0)?;
        let bounds = order
            .iter()
            .map(|entry| entry.1)
            .reduce(Bounds::union)
            .ok_or(Failure::Invalid("drawing_index"))?;
        let children = if let [one] = order {
            Children::Leaf(one.0)
        } else {
            let vertical = bounds.max[1] - bounds.min[1] > bounds.max[0] - bounds.min[0];
            order.sort_unstable_by_key(|&(i, b)| {
                (
                    if vertical {
                        b.min[1] + b.max[1]
                    } else {
                        b.min[0] + b.max[0]
                    },
                    i,
                )
            });
            let middle = order.len() / 2;
            let (first, second) = order.split_at_mut(middle);
            cx.enter()?;
            let pair = (|| Ok::<_, Failure>((self.build(first, cx)?, self.build(second, cx)?)))();
            cx.leave();
            let (a, b) = pair?;
            Children::Pair(a, b)
        };
        let index = self.nodes.len();
        self.nodes.push(Node { bounds, children });
        Ok(index)
    }

    pub fn visit(
        &self,
        bounds: Bounds,
        cx: &mut Cx<'_>,
        mut visit: impl FnMut(usize, &mut Cx<'_>) -> Result<(), Failure>,
    ) -> Result<(), Failure> {
        let Some(root) = self.root else {
            return Ok(());
        };
        let mut stack = Vec::new();
        push(&mut stack, root, 8, cx)?;
        while let Some(at) = stack.pop() {
            work(cx, 1)?;
            let node = self
                .nodes
                .get(at)
                .ok_or(Failure::Invalid("drawing_index"))?;
            if !node.bounds.intersects(bounds) {
                continue;
            }
            match node.children {
                Children::Leaf(index) => visit(index, cx)?,
                Children::Pair(a, b) => {
                    push(&mut stack, b, 8, cx)?;
                    push(&mut stack, a, 8, cx)?;
                }
            }
        }
        Ok(())
    }
}
