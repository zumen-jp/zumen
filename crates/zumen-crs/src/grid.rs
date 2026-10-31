//! ジオイド高と基準面補正量の格子（決定 0019 の「換算を自前で持つ」）。
//!
//! 国土地理院の「ジオイド2024日本とその周辺」の説明書（2026 年 4 月）のとおり、ISG format 2.0 の格子を読み、
//! 求める点を囲む四隅の格子点の値で双一次に補間する。重みが 0 でない格子点のどれかが無効の値（`nodata`）なら、
//! 求めない。格子は利用者が国土地理院から入手して渡し、zumen は同梱しない。使ったモデルの名前と年を結果に返す。
//!
//! 読むのは、ISG format 2.0 の、経緯度（`deg`・`dms`）の格子で、北から南へ行を、西から東へ列を並べ、値が m の
//! もの。ほかの形（点の並び・投影の座標・フィートの値・ほかの並び）は扱わない。

use zumen_base::{Cx, Diagnostic, Kind, Stop, codes};
use zumen_sdk::Error;

use crate::GeoPoint;

/// 格子のモデル。見出しの `model name` と `model year`。翻訳しない
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Model {
    /// モデルの名前（`model name`）
    pub name: String,
    /// モデルを計算した年（`model year`）。書かれていなければ `None`
    pub year: Option<String>,
}

/// 補間した値と、使ったモデル
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Undulation<'a> {
    /// 値（m）。ジオイド高、基準面補正量、またはその和。どれかは渡した格子で決まる
    pub value: f64,
    /// 使ったモデル
    pub model: &'a Model,
}

/// ISG format 2.0 の格子
#[derive(Clone, Debug, PartialEq)]
pub struct Grid {
    model: Model,
    lat_min: f64,
    lat_max: f64,
    lon_min: f64,
    lon_max: f64,
    rows: usize,
    cols: usize,
    nodata: f64,
    /// 北の行から、各行は西から東へ
    values: Vec<f64>,
}

/// 見出しの値の、読み終えたもの
#[derive(Default)]
struct Header<'a> {
    entries: Vec<(&'a str, &'a str)>,
}

impl<'a> Header<'a> {
    fn get(&self, key: &str) -> Option<&'a str> {
        self.entries
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, v)| *v)
            .filter(|v| *v != "---")
    }
}

fn invalid(cx: &mut Cx, key: &str) -> Error {
    cx.report(Diagnostic::new(codes::CRS_GEOID_INVALID).with("key", key));
    Error::Invalid
}

fn unsupported(cx: &mut Cx, key: &str, value: &str) -> Error {
    cx.report(
        Diagnostic::new(codes::CRS_GEOID_UNSUPPORTED)
            .with("key", key)
            .with("value", value),
    );
    Error::Unsupported
}

fn stopped(cx: &mut Cx, stop: Stop) -> Error {
    cx.report(stop.to_diagnostic());
    Error::Stopped(stop)
}

/// 度か、度分秒（`39°50'00"`）の値。度分秒は、数字と小数点の並びを区切りで分けて、度・分・秒とする
fn angle(text: &str, dms: bool) -> Option<f64> {
    let text = text.trim();
    if !dms {
        return number(text);
    }
    let (sign, rest) = match text.strip_prefix('-') {
        Some(rest) => (-1.0, rest),
        None => (1.0, text.strip_prefix('+').unwrap_or(text)),
    };
    let parts: Vec<&str> = rest
        .split(|c: char| !(c.is_ascii_digit() || c == '.'))
        .filter(|part| !part.is_empty())
        .collect();
    if parts.is_empty() || parts.len() > 3 {
        return None;
    }
    let mut value = 0.0;
    for (part, unit) in parts.iter().zip([1.0, 60.0, 3600.0]) {
        value += number(part)? / unit;
    }
    Some(sign * value)
}

fn number(text: &str) -> Option<f64> {
    text.trim().parse::<f64>().ok().filter(|v| v.is_finite())
}

impl Grid {
    /// ISG format 2.0 の格子を読む
    pub fn read(src: &[u8], cx: &mut Cx) -> Result<Grid, Error> {
        cx.check(Kind::InputBytes, src.len() as u64)
            .map_err(|stop| stopped(cx, stop))?;
        // 見出しの前の注記は、どの符号化でもよい。見出しと値は ASCII なので、行ごとに UTF-8 として読む
        let mut lines = src.split(|b| *b == b'\n').map(|line| {
            let line = line.strip_suffix(b"\r").unwrap_or(line);
            core::str::from_utf8(line).ok()
        });
        if !lines
            .by_ref()
            .any(|line| line.is_some_and(|l| l.trim_start().starts_with("begin_of_head")))
        {
            return Err(invalid(cx, "begin_of_head"));
        }
        let mut header = Header::default();
        let mut ended = false;
        for line in lines.by_ref() {
            cx.check(Kind::Records, 1)
                .map_err(|stop| stopped(cx, stop))?;
            let Some(line) = line else {
                return Err(invalid(cx, "header"));
            };
            if line.trim_start().starts_with("end_of_head") {
                ended = true;
                break;
            }
            if line.trim().is_empty() {
                continue;
            }
            let Some(at) = line.find([':', '=']) else {
                return Err(invalid(cx, "header"));
            };
            let (key, value) = (line.get(..at), line.get(at + 1..));
            let (Some(key), Some(value)) = (key, value) else {
                return Err(invalid(cx, "header"));
            };
            header.entries.push((key.trim(), value.trim()));
        }
        if !ended {
            return Err(invalid(cx, "end_of_head"));
        }
        let expect = |cx: &mut Cx, key: &str, allowed: &[&str]| -> Result<(), Error> {
            match header.get(key) {
                Some(value) if !allowed.contains(&value) => Err(unsupported(cx, key, value)),
                _ => Ok(()),
            }
        };
        match header.get("ISG format") {
            Some("2.0") => {}
            Some(value) => return Err(unsupported(cx, "ISG format", value)),
            None => return Err(invalid(cx, "ISG format")),
        }
        // 書かれていなければ、ISG format 2.0 の既定（北から南、西から東）とする
        expect(cx, "data format", &["grid"])?;
        expect(cx, "coord type", &["geodetic"])?;
        expect(cx, "data units", &["meters"])?;
        expect(cx, "data ordering", &["N-to-S, W-to-E", "N-S, W-E"])?;
        let dms = match header.get("coord units") {
            Some("deg") => false,
            Some("dms") => true,
            Some(value) => return Err(unsupported(cx, "coord units", value)),
            None => return Err(invalid(cx, "coord units")),
        };
        let read_angle = |cx: &mut Cx, key: &str| -> Result<f64, Error> {
            header
                .get(key)
                .and_then(|v| angle(v, dms))
                .ok_or_else(|| invalid(cx, key))
        };
        let lat_min = read_angle(cx, "lat min")?;
        let lat_max = read_angle(cx, "lat max")?;
        let lon_min = read_angle(cx, "lon min")?;
        let lon_max = read_angle(cx, "lon max")?;
        let count = |cx: &mut Cx, key: &str| -> Result<usize, Error> {
            header
                .get(key)
                .and_then(|v| v.parse::<usize>().ok())
                .filter(|n| *n >= 2)
                .ok_or_else(|| invalid(cx, key))
        };
        let rows = count(cx, "nrows")?;
        let cols = count(cx, "ncols")?;
        if !(lat_min < lat_max && (-90.0..=90.0).contains(&lat_min) && lat_max <= 90.0) {
            return Err(invalid(cx, "lat max"));
        }
        if !(lon_min < lon_max && lon_min >= -180.0 && lon_max <= 360.0) {
            return Err(invalid(cx, "lon max"));
        }
        // 間隔は、範囲と行と列の数から求める。書かれた間隔（`deg` の 0.333333 など）は丸めてあるので、
        // 半分の間隔より違うときだけ壊れているとする
        for (key, span, n) in [
            ("delta lat", lat_max - lat_min, rows),
            ("delta lon", lon_max - lon_min, cols),
        ] {
            let step = span / (n - 1) as f64;
            if let Some(delta) = header.get(key) {
                let delta = angle(delta, dms).ok_or_else(|| invalid(cx, key))?;
                if (delta - step).abs() > step / 2.0 {
                    return Err(invalid(cx, key));
                }
            }
        }
        let nodata = match header.get("nodata") {
            Some(value) => number(value).ok_or_else(|| invalid(cx, "nodata"))?,
            None => f64::NAN,
        };
        let total = rows.checked_mul(cols).ok_or_else(|| invalid(cx, "nrows"))?;
        // 見出しの数で場所を取らない。値は入力にあるだけ読む
        let mut values = Vec::new();
        for line in lines {
            let Some(line) = line else {
                return Err(invalid(cx, "data"));
            };
            for token in line.split_ascii_whitespace() {
                cx.check(Kind::Records, 1)
                    .map_err(|stop| stopped(cx, stop))?;
                if values.len() == total {
                    return Err(invalid(cx, "data"));
                }
                values.push(number(token).ok_or_else(|| invalid(cx, "data"))?);
            }
        }
        if values.len() != total {
            return Err(invalid(cx, "data"));
        }
        let model = Model {
            name: header.get("model name").unwrap_or("").to_owned(),
            year: header.get("model year").map(str::to_owned),
        };
        Ok(Grid {
            model,
            lat_min,
            lat_max,
            lon_min,
            lon_max,
            rows,
            cols,
            nodata,
            values,
        })
    }

    /// モデルの名前と年
    pub fn model(&self) -> &Model {
        &self.model
    }

    /// 格子点の値。北から `row` 行目、西から `col` 列目。無効の値なら `None`
    fn value(&self, row: usize, col: usize) -> Option<f64> {
        let value = *self
            .values
            .get(row.checked_mul(self.cols)?.checked_add(col)?)?;
        (value != self.nodata).then_some(value)
    }

    /// 点 `at` の値を、囲む四隅の格子点から双一次に補間する。格子の範囲の外か、重みが 0 でない格子点のどれかが
    /// 無効の値なら `None`。経度は、格子が 180 度を越えて持つなら、360 度を足して合わせる
    pub fn interpolate(&self, at: GeoPoint) -> Option<Undulation<'_>> {
        let GeoPoint {
            latitude,
            mut longitude,
        } = at;
        if !latitude.is_finite() || !longitude.is_finite() {
            return None;
        }
        if longitude < self.lon_min && longitude + 360.0 <= self.lon_max {
            longitude += 360.0;
        }
        if !(self.lat_min..=self.lat_max).contains(&latitude)
            || !(self.lon_min..=self.lon_max).contains(&longitude)
        {
            return None;
        }
        // 北の行からの位置と、西の列からの位置。格子点の上にあるときに隣の点を使わないよう、ごく近ければ丸める
        let position = |offset: f64, span: f64, n: usize| {
            let p = offset / span * (n - 1) as f64;
            let rounded = libm::round(p);
            let p = if (p - rounded).abs() < 1e-9 {
                rounded
            } else {
                p
            };
            let cell = (libm::floor(p) as usize).min(n - 2);
            (cell, p - cell as f64)
        };
        let (row, t) = position(
            self.lat_max - latitude,
            self.lat_max - self.lat_min,
            self.rows,
        );
        let (col, u) = position(
            longitude - self.lon_min,
            self.lon_max - self.lon_min,
            self.cols,
        );
        let corners = [
            (row, col, (1.0 - t) * (1.0 - u)),
            (row, col + 1, (1.0 - t) * u),
            (row + 1, col, t * (1.0 - u)),
            (row + 1, col + 1, t * u),
        ];
        let mut value = 0.0;
        for (r, c, weight) in corners {
            if weight != 0.0 {
                value += weight * self.value(r, c)?;
            }
        }
        Some(Undulation {
            value,
            model: &self.model,
        })
    }
}
