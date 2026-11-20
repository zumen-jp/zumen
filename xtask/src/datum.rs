//! 手元の TKY2JGD・PatchJGD の格子から、測量計算サイトと照らす点を選ぶ（#664）。

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::fs::File;
use std::io::Write as _;
use std::path::Path;

use zumen_base::{Budget, CancelToken, Cx, Diagnostics};
use zumen_crs::{GeoPoint, ShiftGrid};

use crate::geoid::{output_folder, outside_repository, read_limited};

type Node = (i32, i32);

/// 3 次メッシュのコードを、格子の南北・東西の番号に戻す。
fn node(code: u32) -> Node {
    let i = code / 1_000_000 * 80 + code / 1_000 % 10 * 10 + code / 10 % 10;
    let j = code / 10_000 % 100 * 80 + code / 100 % 10 * 10 + code % 10;
    (i as i32, j as i32)
}

/// 秒の小数 5 桁の整数に丸めてから度分秒に分け、分や度に繰り上がる結果も同じ桁で出す。
fn dms(seconds: f64) -> String {
    let units = (seconds * 100_000.0).round() as i64;
    let sign = if units < 0 { "-" } else { "" };
    let seconds = units.unsigned_abs() / 100_000;
    format!(
        "{sign}{}{:02}{:02}.{:05}",
        seconds / 3600,
        seconds / 60 % 60,
        seconds % 60,
        units.unsigned_abs() % 100_000
    )
}

struct Mesh {
    nodes: BTreeSet<Node>,
    ordered: Vec<Node>,
    step: i32,
}

impl Mesh {
    fn find(&self, start: usize, accept: impl Fn(Node) -> bool) -> Option<Node> {
        let (before, after) = self.ordered.split_at(start);
        after.iter().chain(before).copied().find(|&n| accept(n))
    }

    fn has(&self, (i, j): Node, di: i32, dj: i32) -> bool {
        self.nodes
            .contains(&(i + di * self.step, j + dj * self.step))
    }

    fn full(&self, n: Node) -> bool {
        (-1..=1).all(|i| (-1..=1).all(|j| self.has(n, i, j)))
    }

    fn points(&self) -> Vec<(String, f64, f64)> {
        let mut points = Vec::new();
        // 1 区画を 3 等分する。緯度 30 秒・経度 45 秒も、5 倍の間隔も整数秒に収まる。
        let mut push = |id: String, (i, j): Node, dy: i32, dx: i32| {
            points.push((
                id,
                f64::from(i * 30 + dy * self.step * 10),
                f64::from(360_000 + j * 45 + dx * self.step * 15),
            ));
        };
        let mut chosen = BTreeSet::new();
        for k in 0..3 {
            let start = self.ordered.len() * (2 * k + 1) / 16;
            if let Some(n) = self.find(start, |n| !chosen.contains(&n) && self.full(n)) {
                chosen.insert(n);
                for (kind, dy, dx) in [
                    ("node", 0, 0),
                    ("ew-line", 0, 1),
                    ("ns-line", 1, 0),
                    ("cell", 1, 1),
                ] {
                    push(format!("N{k}-{kind}"), n, dy, dx);
                }
            }
        }
        let mid = self.ordered.len() / 2;
        // E1 は北東隅が欠ける区画の中。残りは格子線上で、どちら側の格子点が欠けるかを変える。
        if let Some(n) = self.find(mid, |n| {
            self.has(n, 0, 1) && self.has(n, 1, 0) && !self.has(n, 1, 1)
        }) {
            push("E1-cell-missing-ne".into(), n, 1, 1);
        }
        for (id, east_west, side) in [
            ("E2-ew-line-north-missing", true, 1),
            ("E4-ns-line-east-missing", false, 1),
            ("E5-ew-line-south-missing", true, -1),
            ("E6-ns-line-west-missing", false, -1),
        ] {
            let parallel = |n, offset, along| {
                if east_west {
                    self.has(n, offset, along)
                } else {
                    self.has(n, along, offset)
                }
            };
            if let Some(n) = self.find(mid, |n| {
                parallel(n, 0, 1)
                    && [0, 1]
                        .into_iter()
                        .all(|along| !parallel(n, side, along) && parallel(n, -side, along))
            }) {
                push(id.into(), n, i32::from(!east_west), i32::from(east_west));
                if east_west && side == 1 {
                    push("E3-node-north-missing".into(), n, 0, 0);
                }
            }
        }
        points
    }

    fn outside(&self, specified: Option<(&str, &str)>) -> Result<(f64, f64), String> {
        let Some(&(i, j)) = self.ordered.first() else {
            return Err("格子点がありません".into());
        };
        let Some((lat, lon)) = specified else {
            return Ok((
                f64::from((i - 2 * self.step) * 30),
                f64::from(360_000 + j * 45),
            ));
        };
        let seconds = |text: &str, max: f64| {
            text.parse::<f64>()
                .ok()
                .filter(|v| v.abs() <= max)
                .map(|v| (v * 3600.0 * 100_000.0).round() / 100_000.0)
        };
        let (Some(lat), Some(lon)) = (seconds(lat, 90.0), seconds(lon, 180.0)) else {
            return Err("範囲外の点を十進の度で指定してください".into());
        };
        let step = f64::from(self.step);
        let base = (
            (lat / (30.0 * step)).floor() as i32 * self.step,
            ((lon - 360_000.0) / (45.0 * step)).floor() as i32 * self.step,
        );
        if [(0, 0), (1, 0), (0, 1), (1, 1)]
            .into_iter()
            .any(|(di, dj)| self.has(base, di, dj))
        {
            return Err("範囲外の点には、囲む格子点が 1 つもない点を指定してください".into());
        }
        Ok((lat, lon))
    }
}

pub(super) fn points(
    folder: &str,
    grid: &str,
    outside: Option<(&str, &str)>,
) -> Result<(), String> {
    let grid = outside_repository(Path::new(grid), "格子")?;
    let bytes = read_limited(&grid, "格子", 256 * 1024 * 1024)?;
    let mut budget = Budget::default();
    let mut diagnostics = Diagnostics::new();
    let cancel = CancelToken::new();
    let grid = ShiftGrid::read(&bytes, &mut Cx::new(&mut budget, &mut diagnostics, &cancel))
        .map_err(|e| format!("格子を読めませんでした（{e:?}）"))?;
    let ordered: Vec<_> = grid.nodes().map(|(code, _, _)| node(code)).collect();
    let mesh = Mesh {
        nodes: ordered.iter().copied().collect(),
        ordered,
        step: (grid.spacing().0 / 30) as i32,
    };
    let mut points = mesh.points();
    let (lat, lon) = mesh.outside(outside)?;
    points.push(("OUT".into(), lat, lon));
    let mut output = String::new();
    for (id, lat, lon) in &points {
        let at = GeoPoint {
            latitude: lat / 3600.0,
            longitude: lon / 3600.0,
        };
        let (b, l) = match grid.correction(at) {
            Some((db, dl))
                if (lat + db).abs() <= 90.0 * 3600.0 && (lon + dl).abs() <= 180.0 * 3600.0 =>
            {
                (dms(lat + db), dms(lon + dl))
            }
            Some(_) => return Err(format!("{id} の補正結果が経緯度の範囲外です")),
            None => ("-".into(), "-".into()),
        };
        writeln!(output, "{id}\t{}\t{}\t{b}\t{l}", dms(*lat), dms(*lon)).expect("String");
    }
    let path = output_folder(Path::new(folder))?.join("zumen.tsv");
    let mut file = File::create_new(&path)
        .map_err(|e| format!("zumen.tsv を新規作成できませんでした（{}）", e.kind()))?;
    if let Err(error) = file.write_all(output.as_bytes()) {
        drop(file);
        let _ = std::fs::remove_file(path);
        return Err(format!("zumen.tsv を書けませんでした（{}）", error.kind()));
    }
    println!(
        "{} 点の入力と計算結果を zumen.tsv に書きました",
        points.len()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selects_the_same_cells_at_both_mesh_spacings() {
        let mut reference = Vec::new();
        for step in [1, 5] {
            let nodes: BTreeSet<_> = (1..17)
                .flat_map(|i| (1..14).map(move |j| (i, j)))
                .filter(|&n| n != (8, 8))
                .map(|(i, j)| (i * step, j * step))
                .collect();
            let mesh = Mesh {
                ordered: nodes.iter().copied().collect(),
                nodes,
                step,
            };
            let points = mesh.points();
            assert_eq!(points.len(), 18);
            let normalized: Vec<_> = points
                .into_iter()
                .map(|(id, lat, lon)| {
                    (
                        id,
                        lat / f64::from(step),
                        (lon - 360_000.0) / f64::from(step),
                    )
                })
                .collect();
            if step == 1 {
                reference = normalized;
            } else {
                assert_eq!(normalized, reference);
            }
            let (lat, _) = mesh.outside(None).unwrap();
            assert!(lat < 0.0);
            assert!(mesh.outside(Some(("NaN", "0"))).is_err());
        }
    }

    #[test]
    fn rounds_before_splitting_degrees_and_minutes() {
        assert_eq!(dms(59.999_999), "00100.00000");
        assert_eq!(dms(-3_599.999_999), "-10000.00000");
    }
}
