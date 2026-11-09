//! ジオイドの格子の補間を、国土地理院のジオイド高計算サービスか内挿計算プログラム（geoidcalc）と照らす検証キット
//! （#438）。手順と期待する結果は `xtask/verify/geoid/README.md` にある。
//!
//! - `cargo xtask geoid points <フォルダー> <ISG>...`: キットの点の一覧（`xtask/verify/geoid/points.txt`）に、渡した
//!   格子の無効の領域の際の点を足し、点の一覧（`points.txt`）・サービスの一括計算の入力（`geoid.in`）・内挿計算
//!   プログラムの入力（`input.txt`）と、格子ごとに `zumen-crs` の `Grid::read`・`Grid::interpolate` で求めた値
//!   （格子のファイルの名前から拡張子を除いて `zumen-JPGEO2024.txt` など）をフォルダーに書く
//! - `cargo xtask geoid compare <フォルダー> <ISG> <結果> [--column <列>]`: サービスか内挿計算プログラムが書いた
//!   結果を、`Grid::interpolate` で求めた値と比べ、点の数と差の最大を示す
//!
//! 国土地理院のモデルとその値、サービスが返した値は、Git・パッケージ・テストに入れない（決定 0019）。フォルダーと
//! ファイルは、リポジトリと Git の作業ツリーの外のものだけを受け付ける。値は画面に出さず、合わない点も差だけを示す。
//! zumen の値は、そのフォルダーのファイルにだけ書く。

use std::fmt;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Value};
use zumen_crs::{Extent, GeoPoint, Grid};

#[cfg(test)]
mod tests;

/// キットの点の一覧（リポジトリの根から）
const KIT_POINTS: &str = "xtask/verify/geoid/points.txt";
/// フォルダーに書く点の一覧。キットの点の一覧と同じ形
const POINTS: &str = "points.txt";
/// サービスの一括計算の入力。名前は英数字・`_`・`.` で、拡張子は `.in`（サービスの入出力ファイル形式の説明）
const SERVICE_INPUT: &str = "geoid.in";
/// 内挿計算プログラムの入力（説明書の 3 章 (3)①）
const GEOIDCALC_INPUT: &str = "input.txt";
/// サービスの一括計算の点の上限（測量計算サイトの注意事項。ジオイド高計算は 100 点）
const SERVICE_LIMIT: usize = 100;
/// 合ったとする差（m）。#438 の完了の条件の 0.1 mm
const TOLERANCE: f64 = 0.0001;
/// 0.1 mm まで確かめられる、結果の値の小数の桁
const DIGITS: usize = 4;
/// 値を比べる点の数。#438 の完了の条件の 10 点以上
const REQUIRED: usize = 10;
/// 浮動小数の計算の違いとして許す差（m）
const NOISE: f64 = 1e-9;
/// 格子のファイルの大きさの上限。JPGEO2024.isg は 2101 行 × 1601 列で、40 MB ほど
const GRID_LIMIT: u64 = 256 * 1024 * 1024;
/// 点の一覧と結果のファイルの大きさの上限
const TEXT_LIMIT: u64 = 16 * 1024 * 1024;
/// 格子点の上と格子の線の上を見分ける幅（格子の間隔に対する比）。`Grid::interpolate` と同じ
const ON_LINE: f64 = 1e-9;
/// 1 度に当たる、度分秒の単位（0.0001 秒）の数
const PER_DEGREE: i64 = 3600 * 10_000;

/// 度分秒を区切らずに書いた角度（`ddmmss.ssss`・`dddmmss.ssss`）。0.0001 秒を 1 とする整数で持つ。サービスの
/// 一括計算と内挿計算プログラムの入力の書き方で、秒の小数は 4 桁まで（説明書の 3 章 (3)①）
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Dms(i64);

impl Dms {
    /// 度分秒を読む。秒の小数が 4 桁より細かければ 0.0001 秒に丸め、丸めずに読めたかを一緒に返す
    fn parse(text: &str) -> Option<(Dms, bool)> {
        let (negative, rest) = match text.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, text.strip_prefix('+').unwrap_or(text)),
        };
        let (whole, fraction) = rest.split_once('.').unwrap_or((rest, ""));
        let digits = |s: &str| s.bytes().all(|b| b.is_ascii_digit());
        if whole.is_empty() || whole.len() > 9 || fraction.len() > 9 {
            return None;
        }
        if !digits(whole) || !digits(fraction) {
            return None;
        }
        let whole: i64 = whole.parse().ok()?;
        let (degrees, minutes, seconds) = (whole / 10_000, whole / 100 % 100, whole % 100);
        if minutes >= 60 || seconds >= 60 {
            return None;
        }
        // 秒の小数を 9 桁にそろえ、0.0001 秒の単位に丸める（半分は切り上げる）
        let nine: i64 = format!("{fraction:0<9}").parse().ok()?;
        let (units, rest) = (nine / 100_000, nine % 100_000);
        let total =
            ((degrees * 60 + minutes) * 60 + seconds) * 10_000 + units + i64::from(rest >= 50_000);
        Some((Dms(if negative { -total } else { total }), rest == 0))
    }

    /// 度を、0.0001 秒に丸めて度分秒にする
    fn from_degrees(degrees: f64) -> Dms {
        Dms((degrees * PER_DEGREE as f64).round() as i64)
    }

    fn degrees(self) -> f64 {
        self.0 as f64 / PER_DEGREE as f64
    }
}

impl fmt::Display for Dms {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sign = if self.0 < 0 { "-" } else { "" };
        let total = self.0.unsigned_abs();
        let (seconds, fraction) = (total / 10_000, total % 10_000);
        write!(
            f,
            "{sign}{}{:02}{:02}.{fraction:04}",
            seconds / 3600,
            seconds / 60 % 60,
            seconds % 60
        )
    }
}

/// 照らす点
#[derive(Clone, Debug, PartialEq)]
struct Point {
    id: String,
    lat: Dms,
    lon: Dms,
    label: String,
}

impl Point {
    fn at(&self) -> GeoPoint {
        GeoPoint {
            latitude: self.lat.degrees(),
            longitude: self.lon.degrees(),
        }
    }
}

/// 点の一覧を読む。行は「ID 緯度 経度 説明」で、空白で区切る。`#` で始まる行と空の行は飛ばす
fn parse_points(text: &str) -> Result<Vec<Point>, String> {
    let mut points: Vec<Point> = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let number = index + 1;
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut fields = line.split_whitespace();
        let (Some(id), Some(lat), Some(lon)) = (fields.next(), fields.next(), fields.next()) else {
            return Err(format!(
                "点の一覧の {number} 行目に、ID・緯度・経度がありません"
            ));
        };
        if id.is_empty() || !id.bytes().all(|b| b.is_ascii_alphanumeric()) {
            return Err(format!(
                "点の一覧の {number} 行目の ID は、英数字だけにしてください"
            ));
        }
        let angle = |text: &str, limit: i64| {
            Dms::parse(text).filter(|(dms, exact)| *exact && dms.0.abs() <= limit * PER_DEGREE)
        };
        let (Some((lat, _)), Some((lon, _))) = (angle(lat, 90), angle(lon, 360)) else {
            return Err(format!(
                "点の一覧の {number} 行目の緯度・経度は、度分秒を区切らずに、秒の小数を 4 桁までで書いてください"
            ));
        };
        if points.iter().any(|p| p.id == id) {
            return Err(format!(
                "点の一覧の {number} 行目の ID {id} は、前の行と同じです"
            ));
        }
        if points.iter().any(|p| p.lat == lat && p.lon == lon) {
            return Err(format!("点の一覧の {number} 行目の点は、前の行と同じです"));
        }
        points.push(Point {
            id: id.to_owned(),
            lat,
            lon,
            label: fields.collect::<Vec<_>>().join(" "),
        });
    }
    Ok(points)
}

/// 点が格子のどこにあるか
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Place {
    /// 格子点の上
    Node,
    /// 東西に隣り合う格子点を結ぶ線の上
    EastWest,
    /// 南北に隣り合う格子点を結ぶ線の上
    NorthSouth,
    /// 四隅の格子点の間
    Cell,
    /// 範囲の外
    Outside,
}

impl Place {
    fn label(self) -> &'static str {
        match self {
            Place::Node => "格子点の上",
            Place::EastWest => "東西の格子の線の上",
            Place::NorthSouth => "南北の格子の線の上",
            Place::Cell => "格子の中",
            Place::Outside => "範囲の外",
        }
    }
}

/// 点の場所と、範囲の縁（いちばん外の格子点を結ぶ線）の上か。`Grid::interpolate` と同じく、経度は、格子が 180 度を
/// 越えて持つなら 360 度を足して合わせ、格子点の上か線の上かは、格子の間隔の 1e-9 倍の幅で見る
fn locate(extent: &Extent, at: GeoPoint) -> (Place, bool) {
    let latitude = at.latitude;
    let mut longitude = at.longitude;
    if longitude < extent.lon_min && longitude + 360.0 <= extent.lon_max {
        longitude += 360.0;
    }
    if !(extent.lat_min..=extent.lat_max).contains(&latitude)
        || !(extent.lon_min..=extent.lon_max).contains(&longitude)
    {
        return (Place::Outside, false);
    }
    // 格子の線の上なら、縁の線かを返す
    let line = |offset: f64, span: f64, n: usize| {
        let p = offset / span * (n - 1) as f64;
        let rounded = p.round();
        ((p - rounded).abs() < ON_LINE).then_some(rounded == 0.0 || rounded == (n - 1) as f64)
    };
    let row = line(
        extent.lat_max - latitude,
        extent.lat_max - extent.lat_min,
        extent.rows,
    );
    let col = line(
        longitude - extent.lon_min,
        extent.lon_max - extent.lon_min,
        extent.cols,
    );
    let place = match (row.is_some(), col.is_some()) {
        (true, true) => Place::Node,
        (true, false) => Place::EastWest,
        (false, true) => Place::NorthSouth,
        (false, false) => Place::Cell,
    };
    (place, row == Some(true) || col == Some(true))
}

/// 北から `row`、西から `col` の位置（格子の間隔を 1 とする。小数でもよい）の経緯度
fn position(extent: &Extent, row: f64, col: f64) -> GeoPoint {
    GeoPoint {
        latitude: extent.lat_max
            - (extent.lat_max - extent.lat_min) * row / (extent.rows - 1) as f64,
        longitude: extent.lon_min
            + (extent.lon_max - extent.lon_min) * col / (extent.cols - 1) as f64,
    }
}

/// 無効の領域の際に選ぶ点の形。説明書の注 6 のとおり、格子点の上と格子の線の上では、重みが 0 の格子点は無効でも見ない。
/// `Grid::interpolate` は南と東の隣の格子点を重み 0 で持つ。説明書の 3 章 (2) の式は、どちらの側の格子を使うかを
/// 書いておらず、(i, j) を南西の格子点にとれば、北と東の隣の格子点が重み 0 になる。そこで、両方の側の際を選ぶ
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Edge {
    /// 東・南・南東の隣に無効の格子点を持つ、有効の格子点。値を返す
    Node,
    /// 北か北東の隣に無効の格子点を持つ、有効の格子点。値を返す
    NodeNorth,
    /// 両端が有効で、南の隣の格子点に無効のものがある、東西の線の上。値を返す
    EastWest,
    /// 両端が有効で、北の隣の格子点に無効のものがある、東西の線の上。値を返す
    EastWestNorth,
    /// 両端が有効で、東の隣の格子点に無効のものがある、南北の線の上。値を返す
    NorthSouth,
    /// 片方の端だけが無効の、東西の線の上。値を返さない
    EastWestNoData,
    /// 片方の端だけが無効の、南北の線の上。値を返さない
    NorthSouthNoData,
    /// 四隅に有効と無効の格子点を持つ格子の中。値を返さない
    Cell,
}

impl Edge {
    const ALL: [Edge; 8] = [
        Edge::Node,
        Edge::NodeNorth,
        Edge::EastWest,
        Edge::EastWestNorth,
        Edge::NorthSouth,
        Edge::EastWestNoData,
        Edge::NorthSouthNoData,
        Edge::Cell,
    ];

    /// 値を返すか
    fn has_value(self) -> bool {
        matches!(
            self,
            Edge::Node | Edge::NodeNorth | Edge::EastWest | Edge::EastWestNorth | Edge::NorthSouth
        )
    }

    /// 点の場所
    fn place(self) -> Place {
        match self {
            Edge::Node | Edge::NodeNorth => Place::Node,
            Edge::EastWest | Edge::EastWestNorth | Edge::EastWestNoData => Place::EastWest,
            Edge::NorthSouth | Edge::NorthSouthNoData => Place::NorthSouth,
            Edge::Cell => Place::Cell,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Edge::Node => "東・南・南東の隣に無効の格子点がある格子点",
            Edge::NodeNorth => "北・北東の隣に無効の格子点がある格子点",
            Edge::EastWest => "両端が有効で南の隣が無効の、東西の線の上",
            Edge::EastWestNorth => "両端が有効で北の隣が無効の、東西の線の上",
            Edge::NorthSouth => "両端が有効で東の隣が無効の、南北の線の上",
            Edge::EastWestNoData => "片方の端が無効の、東西の線の上",
            Edge::NorthSouthNoData => "片方の端が無効の、南北の線の上",
            Edge::Cell => "隅に無効の格子点を持つ格子の中",
        }
    }
}

/// 格子点の値。無効なら NaN
struct Nodes {
    extent: Extent,
    values: Vec<f64>,
}

impl Nodes {
    /// 格子点ごとに `Grid::interpolate` で値を求める。格子点の上では、その格子点だけが重みを持つ
    fn new(grid: &Grid) -> Nodes {
        let extent = grid.extent();
        let mut values = Vec::with_capacity(extent.rows.saturating_mul(extent.cols));
        for row in 0..extent.rows {
            for col in 0..extent.cols {
                let at = position(&extent, row as f64, col as f64);
                values.push(grid.interpolate(at).map_or(f64::NAN, |u| u.value));
            }
        }
        Nodes { extent, values }
    }

    /// 格子点の値。範囲の外の位置なら `None`、無効の格子点なら `Some(None)`
    fn at(&self, row: isize, col: isize) -> Option<Option<f64>> {
        let (row, col) = (usize::try_from(row).ok()?, usize::try_from(col).ok()?);
        if row >= self.extent.rows || col >= self.extent.cols {
            return None;
        }
        let value = *self.values.get(row * self.extent.cols + col)?;
        Some((!value.is_nan()).then_some(value))
    }

    fn invalid(&self, row: isize, col: isize) -> bool {
        self.at(row, col) == Some(None)
    }
}

/// 無効の領域の際の点を、形ごとに 1 つ選ぶ。北の行から、各行は西から東へ探し、使う有効の格子点の値がどれも 0 でない
/// ものを先に選ぶ（基準面補正量は、日本水準原点の範囲では 0 なので、離島の際を選ぶため）。範囲の縁の線の上の点は
/// 選ばない。相手が縁の線の上を範囲の外とすると（説明書の 3 章 (4)①）、境の規則の違いと、注 6 の扱いの違いを
/// 分けられないため。無効の格子点がなければ空
fn edges(grid: &Grid) -> Vec<(Edge, GeoPoint)> {
    let nodes = Nodes::new(grid);
    let extent = nodes.extent;
    let (last_row, last_col) = ((extent.rows - 1) as f64, (extent.cols - 1) as f64);
    let mut first: [Option<(f64, f64)>; Edge::ALL.len()] = [None; Edge::ALL.len()];
    let mut nonzero: [Option<(f64, f64)>; Edge::ALL.len()] = [None; Edge::ALL.len()];
    let mut offer = |edge: Edge, row: f64, col: f64, values: &[f64]| {
        if row == 0.0 || row == last_row || col == 0.0 || col == last_col {
            return;
        }
        let slot = edge as usize;
        if let Some(first) = first.get_mut(slot).filter(|f| f.is_none()) {
            *first = Some((row, col));
        }
        if values.iter().all(|v| *v != 0.0)
            && let Some(nonzero) = nonzero.get_mut(slot).filter(|f| f.is_none())
        {
            *nonzero = Some((row, col));
        }
    };
    let third = 1.0 / 3.0;
    for r in 0..extent.rows as isize {
        for c in 0..extent.cols as isize {
            let (row, col) = (r as f64, c as f64);
            let here = nodes.at(r, c);
            if let Some(Some(v)) = here {
                if nodes.invalid(r, c + 1) || nodes.invalid(r + 1, c) || nodes.invalid(r + 1, c + 1)
                {
                    offer(Edge::Node, row, col, &[v]);
                }
                if nodes.invalid(r - 1, c) || nodes.invalid(r - 1, c + 1) {
                    offer(Edge::NodeNorth, row, col, &[v]);
                }
            }
            match (here, nodes.at(r, c + 1)) {
                (Some(Some(a)), Some(Some(b))) => {
                    if nodes.invalid(r + 1, c) || nodes.invalid(r + 1, c + 1) {
                        offer(Edge::EastWest, row, col + third, &[a, b]);
                    }
                    if nodes.invalid(r - 1, c) || nodes.invalid(r - 1, c + 1) {
                        offer(Edge::EastWestNorth, row, col + third, &[a, b]);
                    }
                }
                (Some(Some(a)), Some(None)) => offer(Edge::EastWestNoData, row, col + third, &[a]),
                (Some(None), Some(Some(b))) => {
                    offer(Edge::EastWestNoData, row, col + 1.0 - third, &[b]);
                }
                _ => {}
            }
            match (here, nodes.at(r + 1, c)) {
                (Some(Some(a)), Some(Some(b))) => {
                    if nodes.invalid(r, c + 1) || nodes.invalid(r + 1, c + 1) {
                        offer(Edge::NorthSouth, row + third, col, &[a, b]);
                    }
                }
                (Some(Some(a)), Some(None)) => {
                    offer(Edge::NorthSouthNoData, row + third, col, &[a]);
                }
                (Some(None), Some(Some(b))) => {
                    offer(Edge::NorthSouthNoData, row + 1.0 - third, col, &[b]);
                }
                _ => {}
            }
            let corners = [
                here,
                nodes.at(r, c + 1),
                nodes.at(r + 1, c),
                nodes.at(r + 1, c + 1),
            ];
            if corners.iter().all(Option::is_some) {
                let valid: Vec<f64> = corners.iter().flatten().flatten().copied().collect();
                if !valid.is_empty() && valid.len() < corners.len() {
                    offer(Edge::Cell, row + third, col + third, &valid);
                }
            }
        }
    }
    Edge::ALL
        .iter()
        .filter_map(|edge| {
            let slot = *edge as usize;
            let chosen = nonzero.get(slot).copied().flatten();
            let (row, col) = chosen.or(first.get(slot).copied().flatten())?;
            Some((*edge, position(&extent, row, col)))
        })
        .collect()
}

/// リポジトリの中と、Git の作業ツリーの中のパスを断る。国土地理院のモデルと、そこから求めた値は Git に入れない
/// （決定 0019）。作業ツリーがほかの checkout の中にあるとき（`.claude/worktrees/` の下など）も、外の checkout を守る
fn outside_repository(path: &Path, label: &str) -> Result<PathBuf, String> {
    let root = fs::canonicalize(crate::root())
        .map_err(|e| format!("リポジトリの根を開けませんでした（{}）", e.kind()))?;
    let full = fs::canonicalize(path).map_err(|e| {
        format!(
            "{label}（{}）を開けませんでした（{}）",
            path.display(),
            e.kind()
        )
    })?;
    if full.starts_with(&root) {
        return Err(format!(
            "{label}（{}）は、リポジトリの外に置いてください。国土地理院のモデルとその値は Git に入れません（決定 0019）",
            path.display()
        ));
    }
    if let Some(tree) = full.ancestors().find(|a| a.join(".git").exists()) {
        // Windows の正規化したパスの前に付く `\\?\` は、表示しない
        let tree = tree.display().to_string();
        return Err(format!(
            "{label}（{}）は、Git の作業ツリー（{}）の中にあります。リポジトリの外の、Git の外に置いてください。国土地理院のモデルとその値は Git に入れません（決定 0019）",
            path.display(),
            tree.strip_prefix(r"\\?\").unwrap_or(&tree)
        ));
    }
    Ok(full)
}

/// 書く先のフォルダー。なければ、親のフォルダーがリポジトリの外にあることを確かめてから作る
fn output_folder(path: &Path) -> Result<PathBuf, String> {
    if path.exists() {
        return outside_repository(path, "フォルダー");
    }
    let (Some(parent), Some(name)) = (path.parent(), path.file_name()) else {
        return Err(format!(
            "フォルダー（{}）の名前を読めません",
            path.display()
        ));
    };
    let parent = if parent.as_os_str().is_empty() {
        Path::new(".")
    } else {
        parent
    };
    let folder = outside_repository(parent, "フォルダーの親")?.join(name);
    fs::create_dir(&folder).map_err(|e| {
        format!(
            "フォルダー（{}）を作れませんでした（{}）",
            path.display(),
            e.kind()
        )
    })?;
    Ok(folder)
}

fn read_limited(path: &Path, label: &str, limit: u64) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    File::open(path)
        .and_then(|f| f.take(limit + 1).read_to_end(&mut bytes))
        .map_err(|e| {
            format!(
                "{label}（{}）を読めませんでした（{}）",
                path.display(),
                e.kind()
            )
        })?;
    if bytes.len() as u64 > limit {
        return Err(format!("{label}が {limit} バイトの上限を超えました"));
    }
    Ok(bytes)
}

fn read_text(path: &Path, label: &str) -> Result<String, String> {
    String::from_utf8(read_limited(path, label, TEXT_LIMIT)?)
        .map_err(|_| format!("{label}（{}）が UTF-8 ではありません", path.display()))
}

fn file_name(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    )
}

/// 格子を `Grid::read` で読む。予算は既定のもの
fn read_grid(path: &Path) -> Result<Grid, String> {
    let bytes = read_limited(path, "格子のファイル", GRID_LIMIT)?;
    let mut budget = Budget::default();
    let mut diagnostics = Diagnostics::new();
    let cancel = CancelToken::new();
    Grid::read(&bytes, &mut Cx::new(&mut budget, &mut diagnostics, &cancel)).map_err(|_| {
        let found: Vec<String> = diagnostics
            .iter()
            .map(|d| {
                let params: Vec<String> = d
                    .params()
                    .iter()
                    .map(|(name, value)| format!("{name}={}", param_text(value)))
                    .collect();
                format!("{} {}", d.code().name(), params.join(" "))
            })
            .collect();
        format!(
            "{} を Grid::read で読めませんでした（{}）",
            file_name(path),
            found.join("・")
        )
    })
}

fn param_text(value: &Value) -> String {
    match value {
        Value::UInt(v) => v.to_string(),
        Value::Int(v) => v.to_string(),
        Value::Float(v) => v.to_string(),
        Value::Text(v) => v.clone(),
        Value::Key(v) => (*v).to_owned(),
    }
}

/// 格子の説明。モデルの名前と年、範囲、行と列の数。値は含めない
fn describe(name: &str, grid: &Grid) -> String {
    let model = grid.model();
    let extent = grid.extent();
    format!(
        "{name}: モデル {}（{}）、緯度 {}〜{} 度・経度 {}〜{} 度、{} 行 × {} 列",
        model.name,
        model.year.as_deref().unwrap_or("年なし"),
        extent.lat_min,
        extent.lat_max,
        extent.lon_min,
        extent.lon_max,
        extent.rows,
        extent.cols
    )
}

/// 点の一覧を、キットの点の一覧と同じ形で書く
fn points_text(points: &[Point]) -> String {
    let mut out = String::from(
        "# cargo xtask geoid points が書いた点の一覧（xtask/verify/geoid/README.md）。行は「ID 緯度 経度 説明」。\n\
         # 緯度と経度は度分秒を区切らずに書く。国土地理院のモデルから選んだ点を含むので、Git に入れない（決定 0019）\n",
    );
    for p in points {
        out.push_str(&format!(
            "{:<4} {:>13} {:>14} {}\n",
            p.id, p.lat, p.lon, p.label
        ));
    }
    out
}

/// サービスの一括計算の入力。`#` で始まるヘッダーの行と、1 行に緯度・経度を半角の空白で区切って書く
fn service_input(points: &[Point]) -> String {
    let mut out = String::from("# latitude(dms) longitude(dms)\n");
    for p in points {
        out.push_str(&format!("{} {}\n", p.lat, p.lon));
    }
    out
}

/// 内挿計算プログラムの入力。1 行目の `Unit:DMS` と、1 行に緯度・経度を半角の空白で区切って書く
fn geoidcalc_input(points: &[Point]) -> String {
    let mut out = String::from("Unit:DMS\n");
    for p in points {
        out.push_str(&format!("{} {}\n", p.lat, p.lon));
    }
    out
}

/// zumen の値のファイルの名前。格子のファイルの名前から拡張子を除いて付ける
fn values_file(name: &str) -> String {
    let stem = Path::new(name)
        .file_stem()
        .map_or_else(|| name.to_owned(), |s| s.to_string_lossy().into_owned());
    format!("zumen-{stem}.txt")
}

/// `Grid::interpolate` で求めた値。内挿計算プログラムの結果と同じく「緯度 経度 値」と書き、値なしは `NaN` と書く。
/// 国土地理院のモデルから求めた値なので、リポジトリの外のフォルダーにだけ書く（決定 0019）
fn values_text(name: &str, grid: &Grid, points: &[Point]) -> String {
    let mut out = format!(
        "# cargo xtask geoid points が {name} を Grid::read・Grid::interpolate で補間した値（m）。\n\
         # 国土地理院のモデルから求めた値なので、Git・issue・PR に書かない（決定 0019）\n\
         # Lat(dms) Lon(dms) Value(m)\n"
    );
    for p in points {
        match grid.interpolate(p.at()) {
            Some(u) => out.push_str(&format!("{} {} {:.6}\n", p.lat, p.lon, u.value)),
            None => out.push_str(&format!("{} {} NaN\n", p.lat, p.lon)),
        }
    }
    out
}

/// キットの点に、格子ごとの無効の領域の際の点を足す。秒の小数 4 桁に丸めて場所が変わる点は使わない。点がサービスの
/// 一括計算の上限を超えたら止める
fn gather(mut points: Vec<Point>, grids: &[(String, Grid)]) -> Result<Vec<Point>, String> {
    let fixed = points.len();
    for (name, grid) in grids {
        let found = edges(grid);
        if found.is_empty() {
            println!("{name}: 無効の格子点がないので、無効の領域の際の点はない");
        }
        for (edge, at) in found {
            let point = Point {
                id: format!("E{:02}", points.len() - fixed + 1),
                lat: Dms::from_degrees(at.latitude),
                lon: Dms::from_degrees(at.longitude),
                label: format!("{}（{name} の無効の領域の際）", edge.label()),
            };
            // 秒の小数 4 桁に丸めて、格子点の上・格子の線の上から外れたら使わない
            if locate(&grid.extent(), point.at()).0 != edge.place() {
                println!(
                    "{name}: {} の点は、秒の小数 4 桁で表せないので使わない",
                    edge.label()
                );
                continue;
            }
            if grid.interpolate(point.at()).is_some() != edge.has_value() {
                return Err(format!(
                    "{name} の {} の点で、zumen の補間が、無効の格子点の扱い（説明書の注 6）と合いません",
                    edge.label()
                ));
            }
            if points
                .iter()
                .any(|p| p.lat == point.lat && p.lon == point.lon)
            {
                continue;
            }
            points.push(point);
        }
    }
    if points.len() > SERVICE_LIMIT {
        return Err(format!(
            "点が {} あり、サービスの一括計算の上限（{SERVICE_LIMIT} 点）を超えます",
            points.len()
        ));
    }
    Ok(points)
}

/// `cargo xtask geoid points`。キットの点に、格子ごとの無効の領域の際の点を足して、フォルダーに書く
pub(crate) fn points(folder: &str, grids: &[&str]) -> Result<(), String> {
    let folder = output_folder(Path::new(folder))?;
    let kit = read_text(&crate::root().join(KIT_POINTS), "キットの点の一覧")?;
    let kit = parse_points(&kit)?;
    let fixed = kit.len();
    let mut loaded: Vec<(String, Grid)> = Vec::new();
    for path in grids {
        let path = outside_repository(Path::new(path), "格子のファイル")?;
        let name = file_name(&path);
        // Windows と macOS のファイルシステムは大文字と小文字を分けないので、そろえて比べる
        let file = values_file(&name).to_lowercase();
        if loaded
            .iter()
            .any(|(n, _)| values_file(n).to_lowercase() == file)
        {
            return Err(format!(
                "格子のファイルの名前（{name}）が、前に渡したファイルと重なります。名前の違うファイルを渡してください"
            ));
        }
        let grid = read_grid(&path)?;
        println!("{}", describe(&name, &grid));
        loaded.push((name, grid));
    }
    let points = gather(kit, &loaded)?;
    let mut files = vec![
        (POINTS.to_owned(), points_text(&points)),
        (SERVICE_INPUT.to_owned(), service_input(&points)),
        (GEOIDCALC_INPUT.to_owned(), geoidcalc_input(&points)),
    ];
    for (name, grid) in &loaded {
        files.push((values_file(name), values_text(name, grid, &points)));
    }
    for (file, contents) in &files {
        fs::write(folder.join(file), contents)
            .map_err(|e| format!("{file} を書けませんでした（{}）", e.kind()))?;
    }
    println!();
    for p in &points {
        let mut places = Vec::new();
        for (name, grid) in &loaded {
            let (place, edge) = locate(&grid.extent(), p.at());
            let value = if grid.interpolate(p.at()).is_some() {
                "値あり"
            } else {
                "値なし"
            };
            let edge = if edge { "（範囲の縁）" } else { "" };
            places.push(format!("{name}: {}{edge}・{value}", place.label()));
        }
        println!(
            "{} {} {} {} | {}",
            p.id,
            p.lat,
            p.lon,
            p.label,
            places.join(" | ")
        );
    }
    println!();
    println!(
        "点 {}（キットの点 {fixed}・無効の領域の際の点 {}）を、{POINTS}・{SERVICE_INPUT}・{GEOIDCALC_INPUT} に書いた",
        points.len(),
        points.len() - fixed
    );
    let written: Vec<&str> = files
        .iter()
        .skip(3)
        .map(|(file, _)| file.as_str())
        .collect();
    println!(
        "zumen の値を {} に書いた。国土地理院のモデルから求めた値なので、Git・issue・PR に書かない（決定 0019）",
        written.join("・")
    );
    Ok(())
}

/// 結果の値
#[derive(Clone, Copy, Debug, PartialEq)]
enum Reading {
    /// 値と、書かれた小数の桁
    Value { value: f64, decimals: usize },
    /// 値なし（内挿計算プログラムの `NaN`、サービスの `-9999.`）
    None,
}

/// 結果の値を読む。`NaN` と、-9999 は値なしとする
fn reading(text: &str) -> Option<Reading> {
    if text.eq_ignore_ascii_case("nan") {
        return Some(Reading::None);
    }
    let value: f64 = text.parse().ok().filter(|v: &f64| v.is_finite())?;
    if (value + 9999.0).abs() < 0.5 {
        return Some(Reading::None);
    }
    let decimals = text.split_once('.').map_or(0, |(_, f)| f.len());
    Some(Reading::Value { value, decimals })
}

/// 結果の行
#[derive(Clone, Debug, PartialEq)]
struct Row {
    line: usize,
    lat: Dms,
    lon: Dms,
    value: Reading,
}

/// 読んだ結果
#[derive(Debug, Default)]
struct Parsed {
    rows: Vec<Row>,
    /// 値の行でない行（`#` で始まるコメントの行など）。使ったパラメーターやモデルのファイル、プログラムのバージョンを
    /// 書く
    comments: Vec<String>,
    /// UTF-8 でないコメントの行の数（サービスの結果は Shift_JIS のことがある）
    unreadable: usize,
}

/// サービスか内挿計算プログラムの結果を読む。数字（符号を含む）で始まる行を値の行とし、「緯度 経度 値...」の
/// `column` 番目（1 から）を値とする。ほかの行はコメントの行とする。値の行をコメントの行と取り違えても、`check` が
/// 行の数と緯度・経度で見つける。値の後ろの列（点の名前など）は、どの文字コードでもよい
fn parse_result(bytes: &[u8], column: usize) -> Result<Parsed, String> {
    let mut result = Parsed::default();
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    for (index, line) in bytes.split(|b| *b == b'\n').enumerate() {
        let number = index + 1;
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        let text = String::from_utf8_lossy(line);
        let text = text.trim();
        if text.is_empty() {
            continue;
        }
        let mut chars = text.chars();
        let data = match chars.next() {
            Some('+' | '-') => chars.next().is_some_and(|c| c.is_ascii_digit()),
            Some(c) => c.is_ascii_digit(),
            None => false,
        };
        if !data {
            match std::str::from_utf8(line) {
                Ok(_) => result.comments.push(text.to_owned()),
                Err(_) => result.unreadable += 1,
            }
            continue;
        }
        let tokens: Vec<&str> = text.split_whitespace().collect();
        let (Some(lat), Some(lon)) = (tokens.first(), tokens.get(1)) else {
            return Err(format!("結果の {number} 行目に、緯度と経度がありません"));
        };
        let Some(value) = column.checked_sub(1).and_then(|i| tokens.get(i)) else {
            return Err(format!(
                "結果の {number} 行目に、{column} 列目の値がありません"
            ));
        };
        let (Some((lat, _)), Some((lon, _))) = (Dms::parse(lat), Dms::parse(lon)) else {
            return Err(format!(
                "結果の {number} 行目の緯度・経度を、度分秒として読めません"
            ));
        };
        let Some(value) = reading(value) else {
            return Err(format!(
                "結果の {number} 行目の {column} 列目を、数として読めません"
            ));
        };
        result.rows.push(Row {
            line: number,
            lat,
            lon,
            value,
        });
    }
    Ok(result)
}

/// 比べた結果。値は持たない
#[derive(Debug, Default, PartialEq)]
struct Outcome {
    /// 値を比べた点の数
    with_value: usize,
    /// 両方とも値なしだった点の数
    without_value: usize,
    /// 差の最大（m）と、その点の ID
    largest: Option<(f64, String)>,
    /// 結果の値の小数の桁のうち、もっとも少ないもの
    decimals: Option<usize>,
    /// 合わない点。ID・場所・差だけを書き、値は書かない
    mismatches: Vec<String>,
    /// 範囲の縁の上で、zumen は値を返し、結果は値なしだった点（ID と場所）。範囲の外とする境の規則の違いで、
    /// 補間の照合には数えない（説明書の 3 章 (4)①。`xtask/verify/geoid/README.md` の「範囲の縁」）
    border: Vec<String>,
}

impl Outcome {
    /// 0.1 mm の照合として合ったか
    fn passed(&self) -> bool {
        self.mismatches.is_empty() && self.decimals.is_none_or(|d| d >= DIGITS)
    }
}

/// 点の一覧と結果の行を、並びの順に結び、`Grid::interpolate` の値と比べる
fn check(points: &[Point], grid: &Grid, rows: &[Row]) -> Result<Outcome, String> {
    if rows.len() != points.len() {
        return Err(format!(
            "結果の値の行は {} 行で、点の一覧の {} 点と数が違います",
            rows.len(),
            points.len()
        ));
    }
    let extent = grid.extent();
    let mut outcome = Outcome::default();
    for (point, row) in points.iter().zip(rows) {
        if (row.lat, row.lon) != (point.lat, point.lon) {
            return Err(format!(
                "結果の {} 行目の緯度・経度（{} {}）が、点 {}（{} {}）と違います",
                row.line, row.lat, row.lon, point.id, point.lat, point.lon
            ));
        }
        let (spot, border) = locate(&extent, point.at());
        let place = format!(
            "{}{}",
            spot.label(),
            if border { "（範囲の縁）" } else { "" }
        );
        let zumen = grid.interpolate(point.at()).map(|u| u.value);
        match (zumen, row.value) {
            (Some(z), Reading::Value { value, decimals }) => {
                outcome.with_value += 1;
                outcome.decimals = Some(outcome.decimals.map_or(decimals, |d| d.min(decimals)));
                let difference = (z - value).abs();
                if outcome
                    .largest
                    .as_ref()
                    .is_none_or(|(d, _)| difference > *d)
                {
                    outcome.largest = Some((difference, point.id.clone()));
                }
                // 小数の桁が 4 より少なければ、表示の丸めの幅（最後の桁の半分）まで許す。その結果は 0.1 mm の照合に
                // しない
                let allowed = if decimals >= DIGITS {
                    TOLERANCE
                } else {
                    (0..decimals).fold(0.5, |half, _| half / 10.0)
                };
                if difference > allowed + NOISE {
                    outcome.mismatches.push(format!(
                        "{}（{place}）: 差 {:.4} mm",
                        point.id,
                        difference * 1000.0
                    ));
                }
            }
            (None, Reading::None) => outcome.without_value += 1,
            (Some(_), Reading::None) if border => {
                outcome.border.push(format!("{}（{place}）", point.id));
            }
            (Some(_), Reading::None) => outcome.mismatches.push(format!(
                "{}（{place}）: zumen は値を返し、結果は値なし",
                point.id
            )),
            (None, Reading::Value { .. }) => outcome.mismatches.push(format!(
                "{}（{place}）: zumen は値を返さず、結果は値あり",
                point.id
            )),
        }
    }
    Ok(outcome)
}

/// `cargo xtask geoid compare`。フォルダーの点の一覧の順に、結果の値と `Grid::interpolate` の値を比べる
pub(crate) fn compare(folder: &str, grid: &str, result: &str, column: usize) -> Result<(), String> {
    let folder = outside_repository(Path::new(folder), "フォルダー")?;
    let points = parse_points(&read_text(&folder.join(POINTS), "点の一覧")?)?;
    let grid_path = outside_repository(Path::new(grid), "格子のファイル")?;
    let grid_name = file_name(&grid_path);
    let grid = read_grid(&grid_path)?;
    let result_path = outside_repository(Path::new(result), "結果のファイル")?;
    let parsed = parse_result(
        &read_limited(&result_path, "結果のファイル", TEXT_LIMIT)?,
        column,
    )?;
    let outcome = check(&points, &grid, &parsed.rows)?;
    println!("{}", describe(&grid_name, &grid));
    println!("結果: {}（{column} 列目）", file_name(&result_path));
    for comment in &parsed.comments {
        println!("  {comment}");
    }
    if parsed.unreadable > 0 {
        println!("  （UTF-8 でないコメントの行 {}）", parsed.unreadable);
    }
    println!(
        "点: {}（値を比べた点 {}・両方とも値なしの点 {}・範囲の縁で結果だけが値なしの点 {}）",
        points.len(),
        outcome.with_value,
        outcome.without_value,
        outcome.border.len()
    );
    if let Some((difference, id)) = &outcome.largest {
        println!("差の最大: {:.4} mm（{id}）", difference * 1000.0);
    }
    if let Some(decimals) = outcome.decimals {
        println!("結果の値の小数の桁: {decimals}");
    }
    for border in &outcome.border {
        println!(
            "範囲の縁: {border}: zumen は値を返し、結果は値なし。境の規則の違いで、補間の照合には数えない"
        );
    }
    for mismatch in &outcome.mismatches {
        println!("合わない: {mismatch}");
    }
    if !outcome.mismatches.is_empty() {
        return Err(format!("{} 点が合いませんでした", outcome.mismatches.len()));
    }
    if !outcome.passed() {
        return Err(format!(
            "結果の値の小数が {DIGITS} 桁より少なく、0.1 mm の照合になりません。表示の丸めの中では合いました"
        ));
    }
    for line in passed_lines(&outcome) {
        println!("{line}");
    }
    Ok(())
}

/// 照合が通ったときに示す行。「合った」の行に、値を比べた点と、両方とも値なしの点の数を分けて示し、その後の行に、
/// 範囲の縁の点と、値を比べた点が #438 の完了の条件に足りないことを示す
fn passed_lines(outcome: &Outcome) -> Vec<String> {
    let mut lines = vec![format!(
        "合った: 値を比べた点 {}（差は 0.1 mm 以内）・両方とも値なしの点 {}",
        outcome.with_value, outcome.without_value
    )];
    if !outcome.border.is_empty() {
        lines.push(format!(
            "範囲の縁の {} 点は、境の規則の違いとして記録してください（xtask/verify/geoid/README.md の「範囲の縁」）",
            outcome.border.len()
        ));
    }
    if outcome.with_value < REQUIRED {
        lines.push(format!(
            "値を比べた点が {REQUIRED} 点より少なく、この結果だけでは #438 の完了の条件（{REQUIRED} 点以上）に足りません"
        ));
    }
    lines
}
