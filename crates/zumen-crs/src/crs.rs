//! 座標系の名指し（決定 0019 の「座標系の名前」）。
//!
//! EPSG のコードは識別子として使い、原点と縮尺係数は告示から書く。JGD2011 と JGD2024 は同じ定義の名前として、
//! 読むときはどちらも受け付ける。JGD2000 と日本測地系の平面直角座標系は、名前として受け、JGD2011 への変換は
//! [`crate::datum`] で求める（決定 0233）。

use crate::GeoPoint;
use crate::height::Realization;

/// 平面直角座標系の系。平成 14 年国土交通省告示第 9 号の I〜XIX
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Zone(u8);

/// 系の原点の緯度（度）と経度（度と分）。告示の別表。原点の秒はどの系も 0
const ORIGINS: [(u8, u16, u8); 19] = [
    (33, 129, 30),
    (33, 131, 0),
    (36, 132, 10),
    (33, 133, 30),
    (36, 134, 20),
    (36, 136, 0),
    (36, 137, 10),
    (36, 138, 30),
    (36, 139, 50),
    (40, 140, 50),
    (44, 140, 15),
    (44, 142, 15),
    (44, 144, 15),
    (26, 142, 0),
    (26, 127, 30),
    (26, 124, 0),
    (26, 131, 0),
    (20, 136, 0),
    (26, 154, 0),
];

/// 系の番号のローマ数字
const NUMERALS: [&str; 19] = [
    "I", "II", "III", "IV", "V", "VI", "VII", "VIII", "IX", "X", "XI", "XII", "XIII", "XIV", "XV",
    "XVI", "XVII", "XVIII", "XIX",
];

impl Zone {
    /// 番号（1〜19）の系。範囲の外なら `None`
    pub fn new(number: u8) -> Option<Zone> {
        (1..=19).contains(&number).then_some(Zone(number))
    }

    /// 系の番号（1〜19）
    pub fn number(self) -> u8 {
        self.0
    }

    /// 系の番号のローマ数字
    pub fn numeral(self) -> &'static str {
        NUMERALS.get(usize::from(self.0) - 1).copied().unwrap_or("")
    }

    /// 原点の緯度（度）と経度（分）。分で持てば、どの系も整数で表せる
    pub(crate) fn origin_minutes(self) -> (u32, u32) {
        let (lat, lon, min) = ORIGINS
            .get(usize::from(self.0) - 1)
            .copied()
            .unwrap_or((0, 0, 0));
        (u32::from(lat) * 60, u32::from(lon) * 60 + u32::from(min))
    }

    /// 原点の緯度と経度（度）
    pub fn origin(self) -> GeoPoint {
        let (lat, lon) = self.origin_minutes();
        GeoPoint {
            latitude: f64::from(lat) / 60.0,
            longitude: f64::from(lon) / 60.0,
        }
    }

    /// JGD2024（JGD2011）の平面直角座標系の EPSG のコード（6669〜6687）
    pub fn epsg(self) -> u32 {
        6668 + u32::from(self.0)
    }
}

/// 水平の座標系
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Crs {
    /// JGD2024（JGD2011）の経緯度（EPSG:6668）
    Geographic,
    /// JGD2024（JGD2011）の平面直角座標系（EPSG:6669〜6687）
    Plane(Zone),
    /// JGD2000 の平面直角座標系（EPSG:2443〜2461）
    Jgd2000(Zone),
    /// 日本測地系（Tokyo）の平面直角座標系（EPSG:30161〜30179）
    Tokyo(Zone),
}

impl Crs {
    /// EPSG のコードの座標系。扱わないコード（JGD2000 と日本測地系の経緯度など）は `None`
    pub fn from_epsg(code: u32) -> Option<Crs> {
        let zone = |first: u32| u8::try_from(code - first + 1).ok().and_then(Zone::new);
        match code {
            6668 => Some(Crs::Geographic),
            6669..=6687 => zone(6669).map(Crs::Plane),
            2443..=2461 => zone(2443).map(Crs::Jgd2000),
            30161..=30179 => zone(30161).map(Crs::Tokyo),
            _ => None,
        }
    }

    /// EPSG のコード
    pub fn epsg(self) -> u32 {
        match self {
            Crs::Geographic => 6668,
            Crs::Plane(zone) => zone.epsg(),
            Crs::Jgd2000(zone) => 2442 + u32::from(zone.number()),
            Crs::Tokyo(zone) => 30160 + u32::from(zone.number()),
        }
    }

    /// 平面直角座標系の系。経緯度なら `None`
    pub fn zone(self) -> Option<Zone> {
        match self {
            Crs::Geographic => None,
            Crs::Plane(zone) | Crs::Jgd2000(zone) | Crs::Tokyo(zone) => Some(zone),
        }
    }

    /// 名前の座標系。EPSG のコードの書き方（`EPSG:6677`・`urn:ogc:def:crs:EPSG::6677`・
    /// `http://www.opengis.net/def/crs/EPSG/0/6677`）と、EPSG の名前（`JGD2024 / Japan Plane Rectangular CS IX`・
    /// `JGD2024`・`JGD2000 / Japan Plane Rectangular CS IX`・`Tokyo / Japan Plane Rectangular CS IX`）を受け付ける。
    /// 名前の `JGD2011` は `JGD2024` と同じに読む
    pub fn from_name(name: &str) -> Option<Crs> {
        if let Some(code) = epsg_code(name) {
            return Crs::from_epsg(code);
        }
        let name = name.trim();
        let (rest, plane): (&str, fn(Zone) -> Crs) = if let Some(rest) = name
            .strip_prefix("JGD2024")
            .or_else(|| name.strip_prefix("JGD2011"))
        {
            if rest.is_empty() {
                return Some(Crs::Geographic);
            }
            (rest, Crs::Plane)
        } else if let Some(rest) = name.strip_prefix("JGD2000") {
            (rest, Crs::Jgd2000)
        } else {
            (name.strip_prefix("Tokyo")?, Crs::Tokyo)
        };
        let numeral = rest
            .trim_start()
            .strip_prefix('/')?
            .trim_start()
            .strip_prefix("Japan Plane Rectangular CS")?
            .trim();
        let index = NUMERALS.iter().position(|n| *n == numeral)?;
        u8::try_from(index + 1).ok().and_then(Zone::new).map(plane)
    }
}

/// 高さの座標系
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum VerticalCrs {
    /// JGD2011 (vertical) height（EPSG:6695）。測地成果2011 の標高
    Jgd2011,
    /// JGD2024 (vertical) height（EPSG:11317）。測地成果2024 の標高
    Jgd2024,
}

impl VerticalCrs {
    /// EPSG のコードの座標系。扱わないコードは `None`
    pub fn from_epsg(code: u32) -> Option<VerticalCrs> {
        match code {
            6695 => Some(VerticalCrs::Jgd2011),
            11317 => Some(VerticalCrs::Jgd2024),
            _ => None,
        }
    }

    /// EPSG のコード
    pub fn epsg(self) -> u32 {
        match self {
            VerticalCrs::Jgd2011 => 6695,
            VerticalCrs::Jgd2024 => 11317,
        }
    }

    /// 名前の座標系。EPSG のコードの書き方と、EPSG の名前（`JGD2024 (vertical) height`）を受け付ける。
    /// 水平の座標系と違い、2 つの名前は別の成果を指す
    pub fn from_name(name: &str) -> Option<VerticalCrs> {
        if let Some(code) = epsg_code(name) {
            return VerticalCrs::from_epsg(code);
        }
        match name.trim() {
            "JGD2011 (vertical) height" => Some(VerticalCrs::Jgd2011),
            "JGD2024 (vertical) height" => Some(VerticalCrs::Jgd2024),
            _ => None,
        }
    }

    /// 標高の成果
    pub fn realization(self) -> Realization {
        match self {
            VerticalCrs::Jgd2011 => Realization::Survey2011,
            VerticalCrs::Jgd2024 => Realization::Survey2024,
        }
    }
}

/// EPSG のコードの書き方から、コードを取り出す。書き方が違えば `None`
fn epsg_code(name: &str) -> Option<u32> {
    let name = name.trim();
    let digits = ["EPSG:", "epsg:", "urn:ogc:def:crs:EPSG::"]
        .iter()
        .find_map(|prefix| name.strip_prefix(prefix))
        .or_else(|| {
            [
                "http://www.opengis.net/def/crs/EPSG/0/",
                "https://www.opengis.net/def/crs/EPSG/0/",
            ]
            .iter()
            .find_map(|prefix| name.strip_prefix(prefix))
        })?;
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}

/// WKT（LAS・E57 の座標系）から、EPSG のコード（WKT1 の `AUTHORITY["EPSG","6677"]`、WKT2 の `ID["EPSG",6677]`）を読む。高さの
/// 座標系（`VERT_CS`・`VERTCRS`）の中のコードは高さに、ほかは水平に分ける
pub fn wkt_codes(text: &str) -> (Vec<u32>, Vec<u32>) {
    let (mut horizontal, mut vertical) = (Vec::new(), Vec::new());
    // 高さの座標系の中にいる間は、その括弧の深さを持つ
    let mut depth = 0usize;
    let mut vertical_from: Option<usize> = None;
    let mut word = String::new();
    let chars = text.char_indices();
    for (at, c) in chars {
        match c {
            '[' | '(' => {
                depth += 1;
                let name = word.trim().to_ascii_uppercase();
                if vertical_from.is_none()
                    && matches!(name.as_str(), "VERT_CS" | "VERTCRS" | "VERTICALCRS")
                {
                    vertical_from = Some(depth);
                }
                if matches!(name.as_str(), "AUTHORITY" | "ID") {
                    let rest = text.get(at + 1..).unwrap_or_default();
                    if let Some(code) = epsg(rest) {
                        if vertical_from.is_some() {
                            vertical.push(code);
                        } else {
                            horizontal.push(code);
                        }
                    }
                }
                word.clear();
            }
            ']' | ')' => {
                if vertical_from == Some(depth) {
                    vertical_from = None;
                }
                depth = depth.saturating_sub(1);
                word.clear();
            }
            ',' => word.clear(),
            _ => word.push(c),
        }
    }
    // 外側の座標系のコードは後ろに書くので、投影の座標系が先に来るよう、後ろから並べる
    horizontal.reverse();
    vertical.reverse();
    horizontal.sort_by_key(|code| Crs::from_epsg(*code) == Some(Crs::Geographic));
    (horizontal, vertical)
}

/// `"EPSG","6677"]` か `"EPSG",6677]` の続きから、コード
fn epsg(rest: &str) -> Option<u32> {
    let (authority, rest) = rest.split_once(',')?;
    if !authority
        .trim()
        .trim_matches('"')
        .eq_ignore_ascii_case("EPSG")
    {
        return None;
    }
    let end = rest.find([']', ')', ','])?;
    rest.get(..end)?.trim().trim_matches('"').parse().ok()
}
