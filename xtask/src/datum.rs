//! 手元の TKY2JGD・PatchJGD の格子から、測量計算サイトと照らす点を選ぶ（#664）。

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::fs::File;
use std::io::Write as _;
use std::path::Path;

use zumen_base::{Budget, CancelToken, Cx, Diagnostics};
use zumen_crs::{
    Crs, Ellipsoid, GeoPoint, MapPoint, ShiftGrid, Shifts, Zone, to_geographic_on, to_jgd2011,
    to_plane, to_plane_on,
};

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
    fn from_grid(grid: &ShiftGrid) -> Self {
        let ordered: Vec<_> = grid.nodes().map(|(code, _, _)| node(code)).collect();
        Self {
            nodes: ordered.iter().copied().collect(),
            ordered,
            step: (grid.spacing().0 / 30) as i32,
        }
    }

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

fn read_grid(grid: &str) -> Result<ShiftGrid, String> {
    let grid = outside_repository(Path::new(grid), "格子")?;
    let bytes = read_limited(&grid, "格子", 256 * 1024 * 1024)?;
    let mut budget = Budget::default();
    let mut diagnostics = Diagnostics::new();
    let cancel = CancelToken::new();
    ShiftGrid::read(&bytes, &mut Cx::new(&mut budget, &mut diagnostics, &cancel))
        .map_err(|e| format!("格子を読めませんでした（{e:?}）"))
}

pub(super) fn points(
    folder: &str,
    grid: &str,
    outside: Option<(&str, &str)>,
) -> Result<(), String> {
    let grid = read_grid(grid)?;
    let mesh = Mesh::from_grid(&grid);
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
    write_new_files(folder, &[("zumen.tsv", &output)])?;
    println!(
        "{} 点の入力と計算結果を zumen.tsv に書きました",
        points.len()
    );
    Ok(())
}

/// 途中で失敗したら、この呼び出しが新規作成したものだけを戻す。
fn write_new_files(folder: &str, contents: &[(&str, &str)]) -> Result<(), String> {
    let folder = output_folder(Path::new(folder))?;
    let mut created = Vec::new();
    for &(name, data) in contents {
        let path = folder.join(name);
        let result = File::create_new(&path).and_then(|mut file| {
            created.push(path);
            file.write_all(data.as_bytes())
        });
        if let Err(error) = result {
            for path in created {
                let _ = std::fs::remove_file(path);
            }
            return Err(format!(
                "{name} を新規作成できませんでした（{}）",
                error.kind()
            ));
        }
    }
    Ok(())
}

fn site_xy(point: MapPoint) -> MapPoint {
    let round = |value: f64| (value * 10_000.0).round() / 10_000.0;
    MapPoint {
        northing: round(point.northing),
        easting: round(point.easting),
    }
}

fn plane_stages(
    zone: Zone,
    at: GeoPoint,
    tky: &ShiftGrid,
    patch: &ShiftGrid,
    previous: Option<&ShiftGrid>,
) -> Option<[MapPoint; 4]> {
    let tokyo = site_xy(to_plane_on(Ellipsoid::Bessel, zone, at)?.point);
    let geographic = to_geographic_on(Ellipsoid::Bessel, zone, tokyo)?.point;
    let (b, l) = tky.correction(geographic)?;
    if b == 0.0 && l == 0.0 {
        return None;
    }
    let jgd = GeoPoint {
        latitude: geographic.latitude + b / 3600.0,
        longitude: geographic.longitude + l / 3600.0,
    };
    let middle = site_xy(to_plane(zone, jgd)?.point);
    // 改定のファイルを照らす場合、先に渡す JGD2000 用格子の対象外の点に限る。
    // 四隅の一部だけがある点は、連続変換でも診断されて選ばれない。
    if previous.is_some_and(|grid| grid.correction(jgd).is_some()) {
        return None;
    }
    let revision = [patch];
    let shifts = Shifts {
        tky2jgd: Some(tky),
        patch: &[previous.unwrap_or(patch)],
        revision: if previous.is_some() { &revision } else { &[] },
    };
    let mut budget = Budget::default();
    let mut diagnostics = Diagnostics::new();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
    let (_, chain) = to_jgd2011(Crs::Tokyo(zone), tokyo, &shifts, &mut cx)?;
    let datum = if previous.is_some() {
        Crs::Plane(zone)
    } else {
        Crs::Jgd2000(zone)
    };
    let (_, single) = to_jgd2011(datum, middle, &shifts, &mut cx)?;
    let single = site_xy(single);
    (single != middle && middle != tokyo).then_some([tokyo, middle, single, site_xy(chain)])
}

pub(super) fn plane(
    folder: &str,
    tky: &str,
    patch: &str,
    zone: &str,
    previous: Option<&str>,
) -> Result<(), String> {
    let zone = zone
        .parse()
        .ok()
        .and_then(Zone::new)
        .ok_or("系は 1〜19 で指定してください")?;
    let tky = read_grid(tky)?;
    let patch = read_grid(patch)?;
    let previous = previous.map(read_grid).transpose()?;
    let mesh = Mesh::from_grid(&patch);
    let mut selected = BTreeSet::new();
    let mut inputs = [String::new(), String::new()];
    let mut expected =
        String::from("id\ttokyo_x\ttokyo_y\ttky_x\ttky_y\tpatch_x\tpatch_y\tchain_x\tchain_y\n");
    for k in 0..2 {
        let start = mesh.ordered.len() * (2 * k + 1) / 16;
        let (before, after) = mesh.ordered.split_at(start);
        let (chosen, stages) = after
            .iter()
            .chain(before)
            .find_map(|&(i, j)| {
                if selected.contains(&(i, j)) || !mesh.full((i, j)) {
                    return None;
                }
                let at = GeoPoint {
                    latitude: f64::from(i * 30 + mesh.step * 10) / 3600.0,
                    longitude: f64::from(360_000 + j * 45 + mesh.step * 15) / 3600.0,
                };
                Some((
                    (i, j),
                    plane_stages(zone, at, &tky, &patch, previous.as_ref())?,
                ))
            })
            .ok_or("両段で補正される格子内の異なる点を 2 点選べませんでした")?;
        selected.insert(chosen);
        let id = format!("P{}", k + 1);
        for (text, xy) in inputs.iter_mut().zip(&stages) {
            write!(text, "{:.4} {:.4} {id}\r\n", xy.northing, xy.easting).expect("String");
        }
        expected.push_str(&id);
        for xy in stages {
            write!(expected, "\t{:.4}\t{:.4}", xy.northing, xy.easting).expect("String");
        }
        expected.push('\n');
    }
    write_new_files(
        folder,
        &[
            ("tky.in", &inputs[0]),
            ("patch.in", &inputs[1]),
            ("expected.tsv", &expected),
        ],
    )?;
    println!(
        "第 {} 系の 2 点を tky.in、patch.in、expected.tsv に書きました",
        zone.number()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plane_uses_the_previous_grid_only_outside_its_region() {
        let grid = |prefix, b, l| {
            let mut text = String::new();
            for i in 0..4 {
                for j in 0..4 {
                    writeln!(text, "{prefix}{i}{j} {b} {l}").unwrap();
                }
            }
            let mut budget = Budget::default();
            let mut diagnostics = Diagnostics::new();
            let cancel = CancelToken::new();
            ShiftGrid::read(
                text.as_bytes(),
                &mut Cx::new(&mut budget, &mut diagnostics, &cancel),
            )
            .unwrap()
        };
        let zone = Zone::new(9).unwrap();
        let at = GeoPoint {
            latitude: 4001.5 / 120.0,
            longitude: 100.0 + 3201.5 / 80.0,
        };
        let tky = grid("504000", 1, 2);
        let patch = grid("504000", -1, 1);
        let outside = grid("603000", 3, 4);
        let expected = plane_stages(zone, at, &tky, &patch, None).unwrap();
        assert_eq!(
            plane_stages(zone, at, &tky, &patch, Some(&outside)),
            Some(expected)
        );
        assert_eq!(plane_stages(zone, at, &tky, &patch, Some(&patch)), None);
        assert_eq!(plane_stages(zone, at, &tky, &outside, None), None);
    }

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
