//! 測地系の間の変換（決定 0019・0233・0237・0286）。日本測地系と JGD2000 の平面直角座標を、JGD2011 の平面直角座標に
//! 直す。
//!
//! 変換のパラメーターは、国土地理院の TKY2JGD（日本測地系から JGD2000）と PatchJGD（JGD2000 から JGD2011 と、地震の
//! 後の成果の改定）の形の格子で、3 次メッシュのコードの格子点ごとに、緯度と経度の補正量（秒）を持つ。格子点の間隔は、
//! 格子点の並びから見分ける。3 次メッシュの間隔（緯度 30 秒・経度 45 秒。TKY2JGD の論文の表 2）か、その 5 倍の間隔
//! （緯度 150 秒・経度 225 秒。公共測量成果改定マニュアルの第 3 条の解説）のどちらか。求める点を囲む四隅の格子点の
//! 値で双一次に補間し、経緯度に足す。格子は利用者が入手して渡し、zumen は同梱しない。
//!
//! - 日本測地系: ベッセル楕円体で経緯度に直し、TKY2JGD で JGD2000 にし、PatchJGD で JGD2011 にする。
//! - JGD2000: GRS80 で経緯度に直し、PatchJGD で JGD2011 にする。
//!
//! PatchJGD のファイルは、地震などごとに、地殻変動のあった地域だけを持つ。呼ぶ側が古い順に並べたファイルを、その順に
//! 当てる。点を囲む格子点が 1 つもないファイルは、補正量を 0 とする。JGD2011 の座標には当てない（#678）。

use std::collections::BTreeMap;

use zumen_base::{Cx, Diagnostic, Kind, Stop, codes};
use zumen_sdk::Error;

use crate::projection::{Ellipsoid, to_geographic_on, to_plane};
use crate::{Crs, GeoPoint, MapPoint};

/// 格子点の数の上限。3 次メッシュで日本の全域を覆っても、これより少ない
const MAX_NODES: usize = 1 << 22;

/// 3 次メッシュのコードで表せる格子点の番号の上限（緯度・経度とも、1 次メッシュの 2 桁 100 区画の 80 倍）
const MESH_LIMIT: u32 = 100 * 80;

/// 公共測量成果改定マニュアルの第 3 条の解説の、補正パラメーターの格子点の間隔（3 次メッシュの数）。格子点は、3 次
/// メッシュのコードの末位 2 桁が 00・05・50・55 のメッシュの南西の隅（緯度 150 秒・経度 225 秒の倍数）
const FIVE: u32 = 5;

/// 変換のパラメーターの格子（TKY2JGD・PatchJGD の形）。格子点の間隔は、格子点の並びから見分ける（決定 0286）。
/// 格子点がすべて 3 次メッシュの 5 倍の間隔の所にあれば、その間隔（緯度 150 秒・経度 225 秒）で補間する。そうでなく、
/// 3 次メッシュの間隔で隣り合う格子点があれば、3 次メッシュの間隔（緯度 30 秒・経度 45 秒）で補間する。どちらでも
/// なければ、`crs.datum-grid.unsupported` で読まない
#[derive(Clone, Debug, PartialEq)]
pub struct ShiftGrid {
    /// 格子点（緯度の 30 秒ごとの番号、経度 100 度から 45 秒ごとの番号）と、緯度・経度の補正量（秒）
    nodes: BTreeMap<(u32, u32), (f64, f64)>,
    /// 格子点の間隔（3 次メッシュの数）。1 か [`FIVE`]
    step: u32,
}

/// 変換に使う格子
#[derive(Clone, Copy, Debug, Default)]
pub struct Shifts<'a> {
    /// 日本測地系から JGD2000（TKY2JGD）
    pub tky2jgd: Option<&'a ShiftGrid>,
    /// JGD2000 から JGD2011 と、その後の地震などによる成果の改定（PatchJGD）。地震などごとのファイルを古い順に
    /// 並べ、この順に当てる。どのファイルを当てるかは、直したい座標を得た時期で呼ぶ側が選ぶ（公共測量成果改定
    /// マニュアルの第 3 条の解説、決定 0286）。日本測地系と JGD2000 の座標にだけ当て、JGD2000 から JGD2011 にする
    /// ファイルから並べる。JGD2011 の座標には当てないので、2011 年より後の改定のファイルを、その前に得た JGD2011 の
    /// 座標には当てられない（#678）
    pub patch: &'a [&'a ShiftGrid],
}

fn stopped(cx: &mut Cx, stop: Stop) -> Error {
    cx.report(stop.to_diagnostic());
    Error::Stopped(stop)
}

/// 3 次メッシュのコード（8 桁）の南西の隅の、格子点の番号
fn node(code: &str) -> Option<(u32, u32)> {
    let digits: Vec<u32> = code
        .chars()
        .map(|c| c.to_digit(10))
        .collect::<Option<_>>()?;
    let [p, q, r, s, t, u, v, w] = digits.as_slice() else {
        return None;
    };
    if *t > 7 || *u > 7 {
        return None;
    }
    Some((
        (p * 10 + q) * 80 + t * 10 + v,
        (r * 10 + s) * 80 + u * 10 + w,
    ))
}

/// 格子点の番号の、3 次メッシュのコード（8 桁の数）。[`node`] の逆
fn code((i, j): (u32, u32)) -> u32 {
    let first = (i / 80) * 100 + j / 80;
    let second = (i % 80) / 10 * 10 + (j % 80) / 10;
    let third = (i % 10) * 10 + j % 10;
    (first * 100 + second) * 100 + third
}

/// 格子点の並びから、格子点の間隔（3 次メッシュの数）を見分ける（決定 0286）。格子点がすべて 3 次メッシュの 5 倍
/// の間隔の所にあれば [`FIVE`]、そうでなく 3 次メッシュの間隔で隣り合う格子点があれば 1、どちらでもなければ `None`
fn spacing(nodes: &BTreeMap<(u32, u32), (f64, f64)>) -> Option<u32> {
    if nodes.keys().all(|(i, j)| i % FIVE == 0 && j % FIVE == 0) {
        return Some(FIVE);
    }
    nodes
        .keys()
        .any(|&(i, j)| nodes.contains_key(&(i, j + 1)) || nodes.contains_key(&(i + 1, j)))
        .then_some(1)
}

impl ShiftGrid {
    /// TKY2JGD・PatchJGD の形の格子を読む。初めの数字の行の前はヘッダーとして飛ばす。数字の行は、3 次メッシュの
    /// コードと、緯度・経度の補正量（秒）。高さの補正量の列があっても読まない。初めの数字の行の後は、空白だけの
    /// 行のほかに読めない行（UTF-8 でない行を含む）があれば、`crs.datum-grid.invalid` で読まない（決定 0237）
    pub fn read(src: &[u8], cx: &mut Cx) -> Result<ShiftGrid, Error> {
        cx.check(Kind::InputBytes, src.len() as u64)
            .map_err(|stop| stopped(cx, stop))?;
        let mut nodes = BTreeMap::new();
        let mut started = false;
        for (index, line) in src.split(|b| *b == b'\n').enumerate() {
            let invalid = |cx: &mut Cx| {
                cx.report(
                    Diagnostic::new(codes::CRS_DATUM_GRID_INVALID).with("line", index as u64 + 1),
                );
                Error::Invalid
            };
            let text = core::str::from_utf8(line).ok();
            let fields: Vec<&str> = text
                .map(|t| t.split_whitespace().collect())
                .unwrap_or_default();
            let parsed = match fields.as_slice() {
                [code, b, l] | [code, b, l, _] => node(code).zip(
                    b.parse::<f64>()
                        .ok()
                        .zip(l.parse::<f64>().ok())
                        .filter(|(b, l)| b.is_finite() && l.is_finite()),
                ),
                _ => None,
            };
            match parsed {
                Some((at, shift)) => {
                    started = true;
                    cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
                    if nodes.len() >= MAX_NODES || nodes.insert(at, shift).is_some() {
                        return Err(invalid(cx));
                    }
                }
                // ヘッダーは文字コードを問わず飛ばす。その後は、空白だけの行のほかは読めない行にする
                None if !started || (text.is_some() && fields.is_empty()) => {}
                None => return Err(invalid(cx)),
            }
        }
        if nodes.is_empty() {
            cx.report(Diagnostic::new(codes::CRS_DATUM_GRID_INVALID).with("line", 1_u64));
            return Err(Error::Invalid);
        }
        let Some(step) = spacing(&nodes) else {
            cx.report(Diagnostic::new(codes::CRS_DATUM_GRID_UNSUPPORTED));
            return Err(Error::Unsupported);
        };
        Ok(ShiftGrid { nodes, step })
    }

    /// 格子点の緯度と経度の間隔（秒）。3 次メッシュの間隔なら `(30, 45)`、その 5 倍の間隔なら `(150, 225)`
    pub fn spacing(&self) -> (u32, u32) {
        (30 * self.step, 45 * self.step)
    }

    /// 格子点を、南から北へ、同じ緯度は西から東へ、3 次メッシュのコード（8 桁の数）と緯度・経度の補正量（秒）で返す
    pub fn nodes(&self) -> impl Iterator<Item = (u32, f64, f64)> + '_ {
        self.nodes.iter().map(|(at, (b, l))| (code(*at), *b, *l))
    }

    /// 点 `at` の緯度・経度の補正量（秒）を、囲む四隅の格子点から双一次に補間する。重みが 0 でない格子点の
    /// どれかがなければ `None`。PatchJGD の範囲の外の点を補正量 0 とするのは、[`to_jgd2011`] が決める
    pub fn correction(&self, at: GeoPoint) -> Option<(f64, f64)> {
        self.interpolate(at).1
    }

    /// 点 `at` の緯度・経度の補正量（秒）を、囲む四隅の格子点から双一次に補間する。重みが 0 でない格子点の
    /// どれかがなければ `None`。囲む格子点が 1 つもなければ `Some(None)` にせず、呼ぶ側が決められるよう
    /// 見つかった数を返す
    fn interpolate(&self, at: GeoPoint) -> (usize, Option<(f64, f64)>) {
        // 格子点の間隔を 1 とする番号。3 次メッシュの間隔では、1 で割っても値は変わらない
        let step = f64::from(self.step);
        let y = at.latitude * 120.0 / step;
        let x = (at.longitude - 100.0) * 80.0 / step;
        // 3 次メッシュのコードで表せない所（NaN と無限を含む）には、格子点がない。番号が u32 であふれないようにする
        let limit = 0.0..f64::from(MESH_LIMIT / self.step);
        if !(limit.contains(&y) && limit.contains(&x)) {
            return (0, None);
        }
        let (i, j) = (libm::floor(y), libm::floor(x));
        let (t, u) = (y - i, x - j);
        let s = self.step;
        let (i, j) = (i as u32 * s, j as u32 * s);
        let corners = [
            (i, j, (1.0 - t) * (1.0 - u)),
            (i, j + s, (1.0 - t) * u),
            (i + s, j, t * (1.0 - u)),
            (i + s, j + s, t * u),
        ];
        let found = corners
            .iter()
            .filter(|(i, j, _)| self.nodes.contains_key(&(*i, *j)))
            .count();
        let mut shift = (0.0, 0.0);
        for (i, j, weight) in corners {
            if weight == 0.0 {
                continue;
            }
            let Some((b, l)) = self.nodes.get(&(i, j)) else {
                return (found, None);
            };
            shift.0 += weight * b;
            shift.1 += weight * l;
        }
        (found, Some(shift))
    }
}

/// 格子で経緯度を直す。`partial` が真なら、囲む格子点が 1 つもない点の補正量を 0 とする
fn shift(
    grid: &ShiftGrid,
    name: &str,
    point: GeoPoint,
    partial: bool,
    cx: &mut Cx,
) -> Option<GeoPoint> {
    let (found, value) = grid.interpolate(point);
    let (b, l) = match value {
        Some(value) => value,
        None if partial && found == 0 => (0.0, 0.0),
        None => {
            cx.report(
                Diagnostic::new(codes::CRS_DATUM_SHIFT_MISSING)
                    .with("grid", name)
                    .with("latitude", point.latitude)
                    .with("longitude", point.longitude),
            );
            return None;
        }
    };
    Some(GeoPoint {
        latitude: point.latitude + b / 3600.0,
        longitude: point.longitude + l / 3600.0,
    })
}

/// 座標系 `crs` の点を、同じ系の JGD2011 の平面直角座標に直す。JGD2011 の平面直角座標ならそのまま返し、PatchJGD の
/// ファイルも当てない（#678）。PatchJGD のファイルは、渡した順（古い順）に、前のファイルで直した経緯度に当てる
/// （決定 0286）。要る格子がないか、点が格子の範囲の外なら、診断して `None` を返す。診断の `grid` は、PatchJGD の
/// ファイルが 1 つなら `PatchJGD`、複数なら渡した順の番号を付けた `PatchJGD 1`・`PatchJGD 2` など。平面直角座標と
/// 経緯度の換算ができない点（補正量の極端な格子で直した点など）は、まだ診断せずに `None` を返す（#679）。呼ぶ側は、
/// 座標系の違う点を比べない
pub fn to_jgd2011(
    crs: Crs,
    point: MapPoint,
    shifts: &Shifts<'_>,
    cx: &mut Cx,
) -> Option<(Crs, MapPoint)> {
    let missing = |grid: &str, cx: &mut Cx| {
        cx.report(
            Diagnostic::new(codes::CRS_DATUM_GRID_MISSING)
                .with("crs", u64::from(crs.epsg()))
                .with("grid", grid),
        );
    };
    let (zone, geographic) = match crs {
        Crs::Geographic => return None,
        Crs::Plane(_) => return Some((crs, point)),
        Crs::Jgd2000(zone) => (zone, to_geographic_on(Ellipsoid::Grs80, zone, point)?.point),
        Crs::Tokyo(zone) => {
            let Some(grid) = shifts.tky2jgd else {
                missing("TKY2JGD", cx);
                return None;
            };
            let tokyo = to_geographic_on(Ellipsoid::Bessel, zone, point)?.point;
            (zone, shift(grid, "TKY2JGD", tokyo, false, cx)?)
        }
    };
    if shifts.patch.is_empty() {
        missing("PatchJGD", cx);
        return None;
    }
    let mut jgd2011 = geographic;
    for (index, patch) in shifts.patch.iter().enumerate() {
        let name = if shifts.patch.len() == 1 {
            String::from("PatchJGD")
        } else {
            format!("PatchJGD {}", index + 1)
        };
        jgd2011 = shift(patch, &name, jgd2011, true, cx)?;
    }
    Some((Crs::Plane(zone), to_plane(zone, jgd2011)?.point))
}
