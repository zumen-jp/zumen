//! PLY（台帳の `ply`）の点群を読み、点群の領域モデル（決定 0232）に移す。
//!
//! L1 は、見出しの要素と性質の表と、要素ごとの値の列で持つ。ASCII・リトルエンディアン・ビッグエンディアンの本体を
//! 読む。点群の領域モデルには、`vertex` の要素だけを移す。持ち主の決めた仕様はないので、広く使われる書き方
//! （`docs/formats/ply.md`）に従う。

use zumen_base::{Diagnostic, Kind, Ref, codes};
use zumen_pointcloud::{Octree, PointCloud, Points};
use zumen_sdk::{Codec, Cx, Error, FileModel, Lift, RecordInfo, Signature};

/// PLY のコーデック
pub struct Ply;

/// 本体の書き方
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Encoding {
    /// 空白で区切った文字
    Ascii,
    /// リトルエンディアンのバイナリ
    Little,
    /// ビッグエンディアンのバイナリ
    Big,
}

/// 値の型
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scalar {
    /// 符号つき 8 ビット
    I8,
    /// 符号なし 8 ビット
    U8,
    /// 符号つき 16 ビット
    I16,
    /// 符号なし 16 ビット
    U16,
    /// 符号つき 32 ビット
    I32,
    /// 符号なし 32 ビット
    U32,
    /// 32 ビットの浮動小数
    F32,
    /// 64 ビットの浮動小数
    F64,
}

impl Scalar {
    fn parse(name: &str) -> Option<Scalar> {
        Some(match name {
            "char" | "int8" => Scalar::I8,
            "uchar" | "uint8" => Scalar::U8,
            "short" | "int16" => Scalar::I16,
            "ushort" | "uint16" => Scalar::U16,
            "int" | "int32" => Scalar::I32,
            "uint" | "uint32" => Scalar::U32,
            "float" | "float32" => Scalar::F32,
            "double" | "float64" => Scalar::F64,
            _ => return None,
        })
    }

    fn size(self) -> usize {
        match self {
            Scalar::I8 | Scalar::U8 => 1,
            Scalar::I16 | Scalar::U16 => 2,
            Scalar::I32 | Scalar::U32 | Scalar::F32 => 4,
            Scalar::F64 => 8,
        }
    }

    /// 浮動小数の型
    pub fn is_float(self) -> bool {
        matches!(self, Scalar::F32 | Scalar::F64)
    }
}

/// 要素の性質
#[derive(Clone, Debug, PartialEq)]
pub struct Property {
    /// 名前
    pub name: String,
    /// 値の型
    pub scalar: Scalar,
    /// 列（`list`）なら、長さの型
    pub list: Option<Scalar>,
}

/// 要素
#[derive(Clone, Debug, PartialEq)]
pub struct Element {
    /// 名前（`vertex`・`face` など）
    pub name: String,
    /// 数
    pub count: u64,
    /// 性質
    pub properties: Vec<Property>,
    /// 値。列でない性質だけを、要素ごとに性質の順で並べる
    pub values: Vec<f64>,
    /// 元のバイト範囲
    pub span: (u64, u64),
}

/// 読んだ PLY
#[derive(Clone, Debug, PartialEq)]
pub struct PlyFile {
    /// 本体の書き方
    pub encoding: Encoding,
    /// 要素。見出しの順
    pub elements: Vec<Element>,
}

impl FileModel for PlyFile {
    fn record_count(&self) -> usize {
        self.elements.len()
    }

    fn record(&self, index: usize) -> Option<RecordInfo<'_>> {
        let element = self.elements.get(index)?;
        Some(RecordInfo {
            kind: element.name.as_bytes(),
            bytes: Some(zumen_base::ByteRange {
                start: element.span.0,
                end: element.span.1,
            }),
        })
    }
}

impl Codec for Ply {
    type File = PlyFile;

    const FORMAT: &'static str = "ply";

    const SIGNATURES: &'static [Signature] = &[Signature::Lines { lines: &[b"ply"] }];

    fn read(src: &[u8], cx: &mut Cx) -> Result<PlyFile, Error> {
        cx.check(Kind::InputBytes, src.len() as u64)?;
        let invalid = |cx: &mut Cx<'_>, at: usize| {
            cx.report(
                Diagnostic::new(codes::PLY_FILE_INVALID)
                    .at(Ref::at_bytes(at as u64, src.len() as u64)),
            );
            Error::Invalid
        };
        let Some((encoding, mut elements, body)) = header(src) else {
            return Err(invalid(cx, 0));
        };
        let mut at = body;
        for element in &mut elements {
            let start = at;
            cx.check(Kind::Records, element.count)?;
            let read = match encoding {
                Encoding::Ascii => ascii(src, &mut at, element, cx)?,
                Encoding::Little | Encoding::Big => {
                    binary(src, &mut at, element, encoding == Encoding::Big, cx)?
                }
            };
            if !read {
                return Err(invalid(cx, start));
            }
            element.span = (start as u64, at as u64);
        }
        Ok(PlyFile { encoding, elements })
    }

    /// 書き出しはまだしない
    fn write(_file: &PlyFile, _out: &mut Vec<u8>, _cx: &mut Cx) -> Result<(), Error> {
        Err(Error::Unsupported)
    }
}

/// 見出しを読む。本体の書き方と要素と、本体の始まり
fn header(src: &[u8]) -> Option<(Encoding, Vec<Element>, usize)> {
    let mut at = 0;
    let mut encoding = None;
    let mut elements: Vec<Element> = Vec::new();
    let mut first = true;
    loop {
        let rest = src.get(at..)?;
        let end = rest.iter().position(|b| *b == b'\n')?;
        let line = std::str::from_utf8(rest.get(..end)?).ok()?.trim();
        at += end + 1;
        let words: Vec<&str> = line.split_ascii_whitespace().collect();
        if first {
            if words != ["ply"] {
                return None;
            }
            first = false;
            continue;
        }
        match words.as_slice() {
            ["format", name, _version] => {
                encoding = Some(match *name {
                    "ascii" => Encoding::Ascii,
                    "binary_little_endian" => Encoding::Little,
                    "binary_big_endian" => Encoding::Big,
                    _ => return None,
                });
            }
            ["comment", ..] | ["obj_info", ..] | [] => {}
            ["element", name, count] => elements.push(Element {
                name: (*name).to_string(),
                count: count.parse().ok()?,
                properties: Vec::new(),
                values: Vec::new(),
                span: (0, 0),
            }),
            ["property", "list", length, scalar, name] => {
                elements.last_mut()?.properties.push(Property {
                    name: (*name).to_string(),
                    scalar: Scalar::parse(scalar)?,
                    list: Some(Scalar::parse(length)?),
                });
            }
            ["property", scalar, name] => elements.last_mut()?.properties.push(Property {
                name: (*name).to_string(),
                scalar: Scalar::parse(scalar)?,
                list: None,
            }),
            ["end_header"] => return Some((encoding?, elements, at)),
            _ => return None,
        }
    }
}

/// ASCII の本体の要素を読む。読めなければ `false`
fn ascii(
    src: &[u8],
    at: &mut usize,
    element: &mut Element,
    cx: &mut Cx<'_>,
) -> Result<bool, Error> {
    let width = element
        .properties
        .iter()
        .filter(|p| p.list.is_none())
        .count();
    for _ in 0..element.count {
        cx.check(Kind::Work, 1)?;
        let Some(rest) = src.get(*at..) else {
            return Ok(false);
        };
        let end = rest.iter().position(|b| *b == b'\n').unwrap_or(rest.len());
        let Some(line) = rest
            .get(..end)
            .and_then(|line| std::str::from_utf8(line).ok())
        else {
            return Ok(false);
        };
        *at += (end + 1).min(rest.len());
        let mut words = line.split_ascii_whitespace();
        let mut row = Vec::with_capacity(width);
        for property in &element.properties {
            let Some(value) = words.next().and_then(|word| word.parse::<f64>().ok()) else {
                return Ok(false);
            };
            match property.list {
                None => row.push(value),
                // 列は、長さの分の値を飛ばす
                Some(_) => {
                    for _ in 0..value as u64 {
                        if words.next().is_none() {
                            return Ok(false);
                        }
                    }
                }
            }
        }
        element.values.extend(row);
    }
    Ok(true)
}

/// バイナリの値を 1 つ読む
fn value(src: &[u8], at: &mut usize, scalar: Scalar, big: bool) -> Option<f64> {
    let size = scalar.size();
    let bytes = src.get(*at..at.checked_add(size)?)?;
    *at += size;
    let mut buf = [0u8; 8];
    buf.get_mut(..size)?.copy_from_slice(bytes);
    if big {
        buf.get_mut(..size)?.reverse();
    }
    let [a, b, c, d, e, f, g, h] = buf;
    Some(match scalar {
        Scalar::I8 => f64::from(a as i8),
        Scalar::U8 => f64::from(a),
        Scalar::I16 => f64::from(i16::from_le_bytes([a, b])),
        Scalar::U16 => f64::from(u16::from_le_bytes([a, b])),
        Scalar::I32 => f64::from(i32::from_le_bytes([a, b, c, d])),
        Scalar::U32 => f64::from(u32::from_le_bytes([a, b, c, d])),
        Scalar::F32 => f64::from(f32::from_le_bytes([a, b, c, d])),
        Scalar::F64 => f64::from_le_bytes([a, b, c, d, e, f, g, h]),
    })
}

/// バイナリの本体の要素を読む。読めなければ `false`
fn binary(
    src: &[u8],
    at: &mut usize,
    element: &mut Element,
    big: bool,
    cx: &mut Cx<'_>,
) -> Result<bool, Error> {
    for _ in 0..element.count {
        cx.check(Kind::Work, 1)?;
        for property in &element.properties {
            match property.list {
                None => {
                    let Some(found) = value(src, at, property.scalar, big) else {
                        return Ok(false);
                    };
                    element.values.push(found);
                }
                Some(length) => {
                    let Some(length) = value(src, at, length, big) else {
                        return Ok(false);
                    };
                    let skip = (length.max(0.0) as usize).saturating_mul(property.scalar.size());
                    match at.checked_add(skip).filter(|end| *end <= src.len()) {
                        Some(end) => *at = end,
                        None => return Ok(false),
                    }
                }
            }
        }
    }
    Ok(true)
}

impl Lift<PointCloud> for PlyFile {
    fn lift(&self, cx: &mut Cx) -> Result<PointCloud, Error> {
        let mut points = Points::default();
        let mut span = (0, 0);
        for element in &self.elements {
            if element.name != "vertex" {
                cx.report(
                    Diagnostic::new(codes::PLY_ELEMENT_DROPPED)
                        .with("element", element.name.as_str())
                        .with("found", element.count),
                );
                continue;
            }
            span = element.span;
            points = vertices(element, cx)?;
        }
        cx.report(Diagnostic::new(codes::PLY_CRS_MISSING));
        let octree = Octree::build(&points, cx)?;
        Ok(PointCloud {
            crs: None,
            vertical: None,
            points,
            octree,
            source: Ref::at_bytes(span.0, span.1),
        })
    }
}

/// 性質の名前の別名。CloudCompare は、スカラーの場に `scalar_` をつける
fn role(name: &str) -> Option<&'static str> {
    let name = name.to_ascii_lowercase();
    let name = name.strip_prefix("scalar_").unwrap_or(&name);
    Some(match name {
        "x" => "x",
        "y" => "y",
        "z" => "z",
        "red" | "diffuse_red" | "r" => "red",
        "green" | "diffuse_green" | "g" => "green",
        "blue" | "diffuse_blue" | "b" => "blue",
        "intensity" => "intensity",
        "classification" => "classification",
        _ => return None,
    })
}

/// `vertex` の要素を、点の列にする
fn vertices(element: &Element, cx: &mut Cx<'_>) -> Result<Points, Error> {
    let columns: Vec<&Property> = element
        .properties
        .iter()
        .filter(|p| p.list.is_none())
        .collect();
    let width = columns.len().max(1);
    let find = |wanted: &str| {
        columns
            .iter()
            .position(|p| role(&p.name) == Some(wanted))
            .map(|index| (index, columns.get(index).map(|p| p.scalar)))
    };
    let (Some((x, _)), Some((y, _)), Some((z, _))) = (find("x"), find("y"), find("z")) else {
        cx.report(Diagnostic::new(codes::PLY_FILE_INVALID));
        return Err(Error::Invalid);
    };
    for property in &element.properties {
        if property.list.is_some() || role(&property.name).is_none() {
            cx.report(
                Diagnostic::new(codes::PLY_PROPERTY_DROPPED)
                    .with("property", property.name.as_str()),
            );
        }
    }
    let count = element.values.len() / width;
    cx.check(Kind::Elements, count as u64)?;
    let rows = element.values.chunks_exact(width);
    let get = |row: &[f64], index: usize| row.get(index).copied().unwrap_or(0.0);
    let mut points = Points {
        easting: rows.clone().map(|row| get(row, x)).collect(),
        northing: rows.clone().map(|row| get(row, y)).collect(),
        height: rows.clone().map(|row| get(row, z)).collect(),
        ..Points::default()
    };
    if let (Some((r, scalar)), Some((g, _)), Some((b, _))) =
        (find("red"), find("green"), find("blue"))
    {
        // 8 ビットの色は、16 ビットに広げる
        let scale = if scalar == Some(Scalar::U8) {
            257.0
        } else {
            1.0
        };
        let channel =
            |row: &[f64], index: usize| (get(row, index) * scale).clamp(0.0, 65535.0) as u16;
        points.color = Some(
            rows.clone()
                .map(|row| [channel(row, r), channel(row, g), channel(row, b)])
                .collect(),
        );
    }
    if let Some((index, scalar)) = find("intensity") {
        let values: Vec<f64> = rows.clone().map(|row| get(row, index)).collect();
        // 浮動小数の強度が 0〜1 に収まれば、16 ビットの幅に広げる
        let unit =
            scalar.is_some_and(Scalar::is_float) && values.iter().all(|v| (0.0..=1.0).contains(v));
        let scale = if unit { 65535.0 } else { 1.0 };
        points.intensity = Some(
            values
                .iter()
                .map(|v| libm::round(v * scale).clamp(0.0, 65535.0) as u16)
                .collect(),
        );
    }
    if let Some((index, _)) = find("classification") {
        points.classification = Some(
            rows.map(|row| get(row, index).clamp(0.0, 255.0) as u8)
                .collect(),
        );
    }
    Ok(points)
}
