//! 点群の領域モデル（L2、決定 0232）。
//!
//! 点は、地図の空間（`northing`・`easting`、メートル。決定 0019）と標高に置き、属性ごとの列で持つ。形式が
//! 持たない属性は持たない。索引は八分木で、節は、段が 1 つ深くなるごとに半分の間隔に間引いた点を持つ。

use std::collections::BTreeSet;

use zumen_base::{Cx, Kind, Ref, Stop};
use zumen_crs::{Crs, VerticalCrs};

/// 点群
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PointCloud {
    /// 水平の座標系。分からなければ `None`
    pub crs: Option<Crs>,
    /// 高さの座標系。分からなければ `None`
    pub vertical: Option<VerticalCrs>,
    /// 点
    pub points: Points,
    /// 八分木の索引
    pub octree: Octree,
    /// 出所
    pub source: Ref,
}

/// 属性ごとの点の列。どの列も、点の数と同じ長さにする
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Points {
    /// 北の座標（メートル）
    pub northing: Vec<f64>,
    /// 東の座標（メートル）
    pub easting: Vec<f64>,
    /// 標高（メートル）
    pub height: Vec<f64>,
    /// 強度
    pub intensity: Option<Vec<u16>>,
    /// 分類
    pub classification: Option<Vec<u8>>,
    /// 色（赤・緑・青。16 ビット）
    pub color: Option<Vec<[u16; 3]>>,
    /// 反射の番号と、反射の数
    pub returns: Option<Vec<[u8; 2]>>,
    /// 時刻（GPS の時刻、秒）
    pub time: Option<Vec<f64>>,
}

impl Points {
    /// 点の数
    pub fn len(&self) -> usize {
        self.northing.len()
    }

    /// 点がない
    pub fn is_empty(&self) -> bool {
        self.northing.is_empty()
    }

    /// 持つ属性の名前。位置は必ず持つ
    pub fn attributes(&self) -> Vec<&'static str> {
        let mut names = vec!["position"];
        for (name, present) in [
            ("intensity", self.intensity.is_some()),
            ("classification", self.classification.is_some()),
            ("color", self.color.is_some()),
            ("returns", self.returns.is_some()),
            ("time", self.time.is_some()),
        ] {
            if present {
                names.push(name);
            }
        }
        names
    }

    /// 点の範囲（北・東・標高の最小と最大）。点がなければ `None`
    pub fn bounds(&self) -> Option<[[f64; 3]; 2]> {
        let mut out: Option<[[f64; 3]; 2]> = None;
        for ((n, e), h) in self.northing.iter().zip(&self.easting).zip(&self.height) {
            let p = [*n, *e, *h];
            out = Some(match out {
                None => [p, p],
                Some([low, high]) => [
                    [low[0].min(p[0]), low[1].min(p[1]), low[2].min(p[2])],
                    [high[0].max(p[0]), high[1].max(p[1]), high[2].max(p[2])],
                ],
            });
        }
        out
    }
}

/// 八分木の節の番号
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Key {
    /// 段。根は 0
    pub level: u8,
    /// 北の向きの位置
    pub x: u32,
    /// 東の向きの位置
    pub y: u32,
    /// 標高の向きの位置
    pub z: u32,
}

/// 八分木の節
#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    /// 番号
    pub key: Key,
    /// 節の点の番号
    pub points: Vec<u32>,
}

/// 八分木の索引
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Octree {
    /// 根の立方体の、最小の角（北・東・標高）
    pub origin: [f64; 3],
    /// 根の立方体の辺の長さ（メートル）
    pub size: f64,
    /// 1 つの節の 1 辺を分ける数。節の点の間隔は `size / 2^段 / span`
    pub span: u32,
    /// 節。番号の順に並べる
    pub nodes: Vec<Node>,
}

/// 節を分ける数の既定
pub const SPAN: u32 = 64;
/// 八分木の深さの上限。これより深い点は、いちばん深い段の節に入れる
pub const MAX_LEVEL: u8 = 20;

impl Octree {
    /// 段 `level` の節の点の間隔
    pub fn spacing(&self, level: u8) -> f64 {
        self.size / libm::ldexp(1.0, i32::from(level)) / f64::from(self.span.max(1))
    }

    /// 点の列から八分木を作る。点は、浅い段の節の格子の空いた所から順に入れる
    pub fn build(points: &Points, cx: &mut Cx<'_>) -> Result<Octree, Stop> {
        let Some([low, high]) = points.bounds() else {
            return Ok(Octree {
                span: SPAN,
                ..Octree::default()
            });
        };
        let size = (high[0] - low[0])
            .max(high[1] - low[1])
            .max(high[2] - low[2])
            .max(f64::MIN_POSITIVE);
        let mut tree = Octree {
            origin: low,
            size,
            span: SPAN,
            nodes: Vec::new(),
        };
        let mut taken: BTreeSet<(Key, [u32; 3])> = BTreeSet::new();
        let mut members: std::collections::BTreeMap<Key, Vec<u32>> = Default::default();
        for (index, ((n, e), h)) in points
            .northing
            .iter()
            .zip(&points.easting)
            .zip(&points.height)
            .enumerate()
        {
            cx.check(Kind::Work, u64::from(MAX_LEVEL) + 1)?;
            let p = [*n, *e, *h];
            let mut level = 0;
            let key = loop {
                let (key, cell) = tree.locate(p, level);
                if level == MAX_LEVEL || taken.insert((key, cell)) {
                    break key;
                }
                level += 1;
            };
            members
                .entry(key)
                .or_default()
                .push(u32::try_from(index).unwrap_or(u32::MAX));
        }
        tree.nodes = members
            .into_iter()
            .map(|(key, points)| Node { key, points })
            .collect();
        Ok(tree)
    }

    /// 点の、段 `level` の節の番号と、節の中の格子の位置
    fn locate(&self, p: [f64; 3], level: u8) -> (Key, [u32; 3]) {
        let cells = libm::ldexp(f64::from(self.span), i32::from(level));
        let side = self.size / cells;
        let at = |axis: usize| {
            let value = (p.get(axis).copied().unwrap_or(0.0)
                - self.origin.get(axis).copied().unwrap_or(0.0))
                / side;
            let value = libm::floor(value).clamp(0.0, cells - 1.0);
            if value.is_finite() { value as u64 } else { 0 }
        };
        let [x, y, z] = [at(0), at(1), at(2)];
        let span = u64::from(self.span.max(1));
        let key = Key {
            level,
            x: u32::try_from(x / span).unwrap_or(u32::MAX),
            y: u32::try_from(y / span).unwrap_or(u32::MAX),
            z: u32::try_from(z / span).unwrap_or(u32::MAX),
        };
        let cell = [x % span, y % span, z % span].map(|v| u32::try_from(v).unwrap_or(0));
        (key, cell)
    }

    /// 節の立方体（最小の角と最大の角）
    pub fn cube(&self, key: Key) -> [[f64; 3]; 2] {
        let side = self.size / libm::ldexp(1.0, i32::from(key.level));
        let low = [
            self.origin[0] + f64::from(key.x) * side,
            self.origin[1] + f64::from(key.y) * side,
            self.origin[2] + f64::from(key.z) * side,
        ];
        [low, [low[0] + side, low[1] + side, low[2] + side]]
    }

    /// いちばん深い段
    pub fn depth(&self) -> u8 {
        self.nodes
            .iter()
            .map(|node| node.key.level)
            .max()
            .unwrap_or(0)
    }
}

/// 点を選ぶ条件
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Query {
    /// 範囲（北・東・標高の最小と最大）。なければ全体
    pub bounds: Option<[[f64; 3]; 2]>,
    /// 許容差（点の間隔、メートル）。これより細かい段を読まない。なければすべての段
    pub spacing: Option<f64>,
    /// 点の数の上限
    pub limit: u64,
}

/// 選んだ点
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Selection {
    /// 点の番号。浅い段の節から順に並べる
    pub points: Vec<u32>,
    /// 上限のために読まなかった点がある
    pub truncated: bool,
    /// 読んだいちばん深い段
    pub level: u8,
}

/// 条件に合う点を、浅い段の節から順に選ぶ
pub fn select(cloud: &PointCloud, query: &Query, cx: &mut Cx<'_>) -> Result<Selection, Stop> {
    let tree = &cloud.octree;
    let mut nodes: Vec<&Node> = tree
        .nodes
        .iter()
        .filter(|node| {
            let fine_enough = query.spacing.is_none_or(|spacing| {
                node.key.level == 0 || tree.spacing(node.key.level - 1) > spacing
            });
            let overlaps = query.bounds.is_none_or(|[low, high]| {
                let [a, b] = tree.cube(node.key);
                (0..3).all(|i| a.get(i) <= high.get(i) && low.get(i) <= b.get(i))
            });
            fine_enough && overlaps
        })
        .collect();
    nodes.sort_by_key(|node| node.key);
    let mut out = Selection::default();
    let inside = |index: u32| {
        query.bounds.is_none_or(|[low, high]| {
            let at = index as usize;
            let p = [
                cloud.points.northing.get(at),
                cloud.points.easting.get(at),
                cloud.points.height.get(at),
            ];
            p.iter()
                .zip(low.iter().zip(&high))
                .all(|(v, (l, h))| v.is_some_and(|v| l <= v && v <= h))
        })
    };
    for node in nodes {
        cx.check(Kind::Work, node.points.len() as u64)?;
        for index in &node.points {
            if !inside(*index) {
                continue;
            }
            if out.points.len() as u64 >= query.limit {
                out.truncated = true;
                return Ok(out);
            }
            out.points.push(*index);
            out.level = out.level.max(node.key.level);
        }
    }
    cx.check(Kind::Elements, out.points.len() as u64)?;
    Ok(out)
}
