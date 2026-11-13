//! 足すレコードの値と、属性を変えたレコードの値。
//!
//! 今の実装の `apply/add.rs`（足す図形の値）・`apply/transform.rs`（`restyled`）・`drawing/style.rs`（要素のスタイルの読み方）と
//! 同じ規則で作る。今の実装は、画層と線種の名前を、図面の全体の解釈（定義のない名前も含む一覧）から引く。
//! 試作は、開くときに読んだ名前の表だけで決められる場合に限って受け付け、ほかは範囲の外（`Decline::Outside`）とする。

use zumen_acad::{Data, Value};

use crate::edit::{Decline, Outside};
use crate::store::{Charset, Document, table};

/// 色の指定
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorSpec {
    /// 画層に従う
    ByLayer,
    /// ブロックに従う
    ByBlock,
    /// 色番号（1〜255）
    Aci(u8),
    /// RGB
    Rgb([u8; 3]),
    /// 色番号と RGB
    AciRgb(u8, [u8; 3]),
}

/// 線種の指定
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LineTypeSpec {
    /// 画層に従う
    ByLayer,
    /// ブロックに従う
    ByBlock,
    /// 線種の名前
    Name(String),
}

/// 線の太さの指定
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WeightSpec {
    /// 画層に従う
    ByLayer,
    /// ブロックに従う
    ByBlock,
    /// 文書の既定
    Default,
    /// ミリメートル
    Mm(f64),
}

/// 属性を変える変更。`None` の項目は変えない
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Restyle {
    /// 画層の名前
    pub layer: Option<String>,
    /// 色
    pub color: Option<ColorSpec>,
    /// 線種
    pub line_type: Option<LineTypeSpec>,
    /// 線種の尺度
    pub line_type_scale: Option<f64>,
    /// 線の太さ
    pub line_weight: Option<WeightSpec>,
}

/// 足す要素。モデル空間に足す
#[derive(Clone, Debug, PartialEq)]
pub struct Add {
    /// 画層の名前。`None` は画層 0
    pub layer: Option<String>,
    /// 色。`None` は画層に従う
    pub color: Option<ColorSpec>,
    /// 線種。`None` は画層に従う
    pub line_type: Option<LineTypeSpec>,
    /// 線の太さ。`None` は画層に従う
    pub line_weight: Option<WeightSpec>,
    /// 形
    pub shape: NewShape,
}

/// 足す要素の形
#[derive(Clone, Debug, PartialEq)]
pub enum NewShape {
    /// 線
    Line {
        /// 始まり
        start: [f64; 2],
        /// 終わり
        end: [f64; 2],
    },
    /// 円
    Circle {
        /// 中心
        center: [f64; 2],
        /// 半径
        radius: f64,
    },
    /// 反時計回りの円弧
    Arc {
        /// 中心
        center: [f64; 2],
        /// 半径
        radius: f64,
        /// 始まりの角（度）
        start: f64,
        /// 終わりの角（度）
        end: f64,
    },
    /// 折れ線
    Polyline {
        /// 頂点
        points: Vec<[f64; 2]>,
        /// 頂点ごとの膨らみ
        bulges: Option<Vec<f64>>,
        /// 閉じているか
        closed: bool,
    },
    /// 文字
    Text(NewText),
    /// 記号（ブロックの挿入）
    Symbol(NewSymbol),
}

/// 足す文字
#[derive(Clone, Debug, PartialEq)]
pub struct NewText {
    /// 中身
    pub content: String,
    /// 置く点
    pub position: [f64; 2],
    /// 2 点の間に置くときの終わりの点と、幅も合わせるか
    pub end: Option<([f64; 2], bool)>,
    /// 横の位置合わせ（72 の値。0・1・2・4）
    pub horizontal: Option<u8>,
    /// 縦の位置合わせ（73 の値。0〜3）
    pub vertical: Option<u8>,
    /// 高さ
    pub height: f64,
    /// 回転（度）
    pub rotation: f64,
    /// 幅の係数
    pub width_factor: Option<f64>,
    /// 傾き（度）
    pub oblique: Option<f64>,
    /// 左右を反転するか
    pub backward: bool,
    /// 上下を反転するか
    pub upside_down: bool,
    /// 文字スタイルの名前。`None` は Standard
    pub style: Option<String>,
}

/// 足す記号
#[derive(Clone, Debug, PartialEq)]
pub struct NewSymbol {
    /// 定義の名前
    pub name: String,
    /// 置く点
    pub position: [f64; 2],
    /// 尺度
    pub scale: [f64; 2],
    /// 回転（度）
    pub rotation: f64,
    /// 格子
    pub grid: Option<NewGrid>,
}

/// 記号の格子
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NewGrid {
    /// 列の数
    pub columns: u32,
    /// 行の数
    pub rows: u32,
    /// 列の間隔
    pub column_spacing: f64,
    /// 行の間隔
    pub row_spacing: f64,
}

/// 色
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Color {
    Layer,
    Block,
    Own {
        aci: Option<u8>,
        rgb: Option<[u8; 3]>,
    },
}

/// 線種。名前は、符号化のまま
#[derive(Clone, Copy, Debug)]
enum Line<'a> {
    Layer,
    Block,
    Own(&'a [u8]),
}

/// 線の太さ
#[derive(Clone, Copy, Debug, PartialEq)]
enum Weight {
    Layer,
    Block,
    Default,
    Width(f64),
}

/// 要素のスタイルのうち、変更で変えられるもの
#[derive(Clone, Copy, Debug)]
struct Look<'a> {
    color: Color,
    line: Line<'a>,
    weight: Weight,
    scale: f64,
}

const UNSUPPORTED: Decline = Decline::Unsupported;

fn text(code: i16, value: &[u8]) -> Value {
    Value {
        code,
        data: Data::Text(value.to_vec()),
    }
}

fn int(code: i16, value: i64) -> Value {
    Value {
        code,
        data: Data::Int(value),
    }
}

fn real(code: i16, value: f64) -> Result<Value, Decline> {
    if value.is_finite() {
        Ok(Value {
            code,
            data: Data::Real(value),
        })
    } else {
        Err(UNSUPPORTED)
    }
}

/// 点。Z は 0 を書く
fn point(values: &mut Vec<Value>, code: i16, [x, y]: [f64; 2]) -> Result<(), Decline> {
    values.extend([real(code, x)?, real(code + 10, y)?, real(code + 20, 0.0)?]);
    Ok(())
}

fn positive(value: f64) -> Result<f64, Decline> {
    if value.is_finite() && value > 0.0 {
        Ok(value)
    } else {
        Err(UNSUPPORTED)
    }
}

fn first(values: &[Value], code: i16) -> Option<&Data> {
    values.iter().find(|v| v.code == code).map(|v| &v.data)
}

/// レコードの今のスタイル（`drawing/style.rs`）。要素として読めたレコードなら、読める
fn look_of(values: &[Value]) -> Option<Look<'_>> {
    let rgb = match first(values, 420) {
        None => None,
        Some(Data::Int(value @ 0..=0xFF_FFFF)) => {
            let [_, r, g, b] = (*value as u32).to_be_bytes();
            Some([r, g, b])
        }
        Some(_) => return None,
    };
    let index = match first(values, 62) {
        None => 256,
        Some(Data::Int(index)) => *index,
        Some(_) => return None,
    };
    let color = match (index, rgb) {
        (1..=255, _) => Color::Own {
            aci: Some(index as u8),
            rgb,
        },
        (_, Some(rgb)) => Color::Own {
            aci: None,
            rgb: Some(rgb),
        },
        (0, None) => Color::Block,
        (256, None) => Color::Layer,
        _ => return None,
    };
    let line = match first(values, 6) {
        None => Line::Layer,
        Some(Data::Text(name)) if name.eq_ignore_ascii_case(b"ByLayer") => Line::Layer,
        Some(Data::Text(name)) if name.eq_ignore_ascii_case(b"ByBlock") => Line::Block,
        Some(Data::Text(name)) => Line::Own(name),
        Some(_) => return None,
    };
    let scale = match first(values, 48) {
        None => 1.0,
        Some(Data::Real(scale)) if *scale > 0.0 => *scale,
        Some(_) => return None,
    };
    let weight = match first(values, 370) {
        None | Some(Data::Int(-1)) => Weight::Layer,
        Some(Data::Int(-2)) => Weight::Block,
        Some(Data::Int(-3)) => Weight::Default,
        Some(Data::Int(width @ 0..=211)) => Weight::Width(*width as f64 / 100.0),
        Some(_) => return None,
    };
    Some(Look {
        color,
        line,
        weight,
        scale,
    })
}

/// 同じ線種か。線種は、名前の ASCII の大小を区別せずに引く
fn same_line(a: Line<'_>, b: Line<'_>) -> bool {
    match (a, b) {
        (Line::Layer, Line::Layer) | (Line::Block, Line::Block) => true,
        (Line::Own(a), Line::Own(b)) => a.eq_ignore_ascii_case(b),
        _ => false,
    }
}

fn color_from(spec: ColorSpec) -> Result<Color, Decline> {
    Ok(match spec {
        ColorSpec::ByLayer => Color::Layer,
        ColorSpec::ByBlock => Color::Block,
        ColorSpec::Aci(0) | ColorSpec::AciRgb(0, _) => return Err(UNSUPPORTED),
        ColorSpec::Aci(aci) => Color::Own {
            aci: Some(aci),
            rgb: None,
        },
        ColorSpec::Rgb(rgb) => Color::Own {
            aci: None,
            rgb: Some(rgb),
        },
        ColorSpec::AciRgb(aci, rgb) => Color::Own {
            aci: Some(aci),
            rgb: Some(rgb),
        },
    })
}

fn weight_from(spec: WeightSpec) -> Result<Weight, Decline> {
    Ok(match spec {
        WeightSpec::ByLayer => Weight::Layer,
        WeightSpec::ByBlock => Weight::Block,
        WeightSpec::Default => Weight::Default,
        WeightSpec::Mm(mm) if mm.is_finite() && mm >= 0.0 => Weight::Width(mm),
        WeightSpec::Mm(_) => return Err(UNSUPPORTED),
    })
}

/// 線の太さ（370）。-1 は画層、-2 はブロック、-3 は文書の既定、0〜211 は 1/100 mm
fn weight_code(weight: Weight) -> Result<i64, Decline> {
    match weight {
        Weight::Layer => Ok(-1),
        Weight::Block => Ok(-2),
        Weight::Default => Ok(-3),
        Weight::Width(width) => {
            let hundredths = width * 100.0;
            let rounded = hundredths.round();
            if (hundredths - rounded).abs() < 1e-9 && (0.0..=211.0).contains(&rounded) {
                Ok(rounded as i64)
            } else {
                Err(UNSUPPORTED)
            }
        }
    }
}

impl Document {
    /// 図面のバージョンが `release` 以後か
    fn since(&self, release: &[u8]) -> bool {
        self.version.as_slice() >= release
    }

    /// 文字列を、この図面の符号化で書く。試作は、UTF-8 の図面のほかでは ASCII だけを書く
    fn encoded<'a>(&self, value: &'a str) -> Result<&'a [u8], Decline> {
        if value.is_ascii() || self.charset == Charset::Utf8 {
            Ok(value.as_bytes())
        } else {
            Err(Decline::Outside(Outside::Charset))
        }
    }

    /// 画層か線種の名前を、表の綴りで返す。受け付けるのは、その名前（小文字にして同じもの）のレコードが 1 つだけで、
    /// TABLES にあり、綴りも同じとき。今の実装は、定義のない名前や、綴りの違う定義がいくつかある名前も、
    /// 図面の全体の解釈から決める
    fn defined(&self, table: usize, name: &str) -> Result<&[u8], Decline> {
        let outside = Decline::Outside(Outside::Names);
        if !self.names_exact {
            return Err(outside);
        }
        let spelling = self
            .names
            .get(table)
            .and_then(Option::as_ref)
            .and_then(|names| names.get(&name.to_lowercase()))
            .ok_or(outside)?;
        if spelling.count == 1 && spelling.tables == 1 && *spelling.name == *name.as_bytes() {
            Ok(&spelling.name)
        } else {
            Err(outside)
        }
    }

    /// 文字スタイルの名前（今の実装の `Born::named`）。表があれば、表の綴り。表がなければ、Standard だけを書ける
    fn text_style(&self, name: &str) -> Result<Vec<u8>, Decline> {
        if !self.names_exact {
            return Err(Decline::Outside(Outside::Names));
        }
        match self.names.get(table::STYLE).and_then(Option::as_ref) {
            Some(names) => names
                .get(&name.to_lowercase())
                .map(|spelling| spelling.name.to_vec())
                .ok_or(UNSUPPORTED),
            None if name.eq_ignore_ascii_case("Standard") => Ok(self.encoded(name)?.to_vec()),
            None => Err(UNSUPPORTED),
        }
    }

    /// 記号の定義の名前（今の実装の `Born::placing`）。指せるのは、名前で 1 つに決まり、属性の定義を持たない定義だけ
    fn block_name(&self, name: &str) -> Result<Vec<u8>, Decline> {
        let lower = name.to_lowercase();
        if lower.starts_with("*model_space") || lower.starts_with("*paper_space") {
            return Err(UNSUPPORTED);
        }
        if !self.names_exact {
            return Err(Decline::Outside(Outside::Names));
        }
        let definition = self
            .usable_definitions
            .get(&lower)
            .and_then(|at| self.definitions.get(*at as usize))
            .ok_or(Decline::Outside(Outside::Definition))?;
        match self.names.get(table::BLOCK_RECORD).and_then(Option::as_ref) {
            Some(names) => {
                let key = self
                    .decode(&definition.name)
                    .ok_or(Decline::Outside(Outside::Names))?
                    .to_lowercase();
                names
                    .get(&key)
                    .map(|spelling| spelling.name.to_vec())
                    .ok_or(UNSUPPORTED)
            }
            None => Ok(definition.name.to_vec()),
        }
    }

    /// 線種の指定を読む
    fn line_from<'a>(&'a self, spec: &'a LineTypeSpec) -> Result<Line<'a>, Decline> {
        Ok(match spec {
            LineTypeSpec::ByLayer => Line::Layer,
            LineTypeSpec::ByBlock => Line::Block,
            // 今の実装は、表に ByLayer・ByBlock という名前の線種があれば、それを指す。試作は扱わない
            LineTypeSpec::Name(name)
                if name.eq_ignore_ascii_case("ByLayer") || name.eq_ignore_ascii_case("ByBlock") =>
            {
                return Err(Decline::Outside(Outside::Names));
            }
            LineTypeSpec::Name(name) => Line::Own(self.defined(table::LTYPE, name)?),
        })
    }

    /// スタイルの値（今の実装の `Born::style_values`）。`changed` にあるグループ・コードの分だけを、
    /// 線種（6）・色（62）・線の太さ（370）・線種の尺度（48）・RGB（420）の順に並べる
    fn style_values(&self, look: &Look<'_>, changed: &[i16]) -> Result<Vec<Value>, Decline> {
        let mut values = Vec::new();
        let mut last = None;
        if changed.contains(&6) {
            match look.line {
                Line::Layer => {}
                Line::Block => values.push(text(6, b"ByBlock")),
                Line::Own(name) => values.push(text(6, name)),
            }
        }
        if changed.contains(&62) {
            let mut rgb = None;
            match look.color {
                Color::Layer => {}
                Color::Block => values.push(int(62, 0)),
                Color::Own { aci, rgb: own } => {
                    match aci {
                        Some(aci) => values.push(int(62, i64::from(aci))),
                        None if own.is_none() => return Err(UNSUPPORTED),
                        None => {}
                    }
                    rgb = own;
                }
            }
            if let Some([r, g, b]) = rgb {
                if !self.since(b"AC1018") {
                    return Err(UNSUPPORTED);
                }
                last = Some(int(420, i64::from(u32::from_be_bytes([0, r, g, b]))));
            }
        }
        if changed.contains(&370) {
            match look.weight {
                Weight::Layer => {}
                weight if self.since(b"AC1015") => values.push(int(370, weight_code(weight)?)),
                _ => return Err(UNSUPPORTED),
            }
        }
        if changed.contains(&48) && look.scale != 1.0 {
            values.push(real(48, positive(look.scale)?)?);
        }
        values.extend(last);
        Ok(values)
    }

    /// 属性を変えた後のレコードの値（今の実装の `restyled`）。`layer` は、今の画層の名前。
    /// 何も変わらなければ、元の値のまま
    pub(crate) fn restyled(
        &self,
        layer: &[u8],
        old: &[Value],
        change: &Restyle,
    ) -> Result<Vec<Value>, Decline> {
        let was = look_of(old).ok_or(UNSUPPORTED)?;
        let color = match change.color {
            Some(spec) => color_from(spec)?,
            None => was.color,
        };
        let line = match &change.line_type {
            Some(spec) => self.line_from(spec)?,
            None => was.line,
        };
        let weight = match change.line_weight {
            Some(spec) => weight_from(spec)?,
            None => was.weight,
        };
        let scale = match change.line_type_scale {
            Some(scale) => positive(scale)?,
            None => was.scale,
        };
        let new_layer = match &change.layer {
            Some(name) => Some(self.defined(table::LAYER, name)?),
            None => None,
        };
        let mut changed: Vec<i16> = Vec::new();
        if new_layer.is_some_and(|name| !name.eq_ignore_ascii_case(layer)) {
            changed.push(8);
        }
        if !same_line(line, was.line) {
            changed.push(6);
        }
        if color != was.color {
            changed.extend([62, 420, 430]);
        }
        if weight != was.weight {
            changed.push(370);
        }
        if scale != was.scale {
            changed.push(48);
        }
        if changed.is_empty() {
            return Ok(old.to_vec());
        }
        let mut written = Vec::new();
        if changed.contains(&8)
            && let Some(name) = new_layer
        {
            written.push(text(8, name));
        }
        let look = Look {
            color,
            line,
            weight,
            scale,
        };
        written.extend(self.style_values(&look, &changed)?);

        // 図形に共通の値の範囲。サブクラスの印（100 AcDbEntity）の次から、次の印か XDATA の前まで。
        // 印のないレコードは、最初から XDATA の前まで
        let marker = old.iter().position(|v| {
            v.code == 100 && matches!(&v.data, Data::Text(name) if name == b"AcDbEntity")
        });
        let start = marker.map_or(0, |index| index + 1);
        let end = old
            .iter()
            .enumerate()
            .skip(start)
            .find(|(_, v)| v.code == 1001 || (marker.is_some() && v.code == 100))
            .map_or(old.len(), |(index, _)| index);
        let common = start..end;
        let mut values = Vec::with_capacity(old.len() + written.len());
        let mut insert_at = None;
        for (index, value) in old.iter().enumerate() {
            let inside = common.contains(&index);
            if inside && value.code == 8 && insert_at.is_none() {
                if !changed.contains(&8) {
                    values.push(value.clone());
                }
                insert_at = Some(values.len());
            } else if !(inside && changed.contains(&value.code)) {
                values.push(value.clone());
            }
        }
        let insert_at = insert_at.unwrap_or(common.start);
        values.splice(insert_at..insert_at, written);
        Ok(values)
    }

    /// 足す要素のレコードの種類と値（今の実装の `Born::record` と `Born::push`）。値は、ハンドル（5）から並ぶ
    pub(crate) fn born(
        &self,
        add: &Add,
        handle: u64,
    ) -> Result<(&'static [u8], Vec<Value>), Decline> {
        let layer = self.defined(table::LAYER, add.layer.as_deref().unwrap_or("0"))?;
        let look = Look {
            color: match add.color {
                Some(spec) => color_from(spec)?,
                None => Color::Layer,
            },
            line: match &add.line_type {
                Some(spec) => self.line_from(spec)?,
                None => Line::Layer,
            },
            weight: match add.line_weight {
                Some(spec) => weight_from(spec)?,
                None => Weight::Layer,
            },
            scale: 1.0,
        };
        let mut values = vec![Value {
            code: 5,
            data: Data::Handle(handle),
        }];
        if let Some(owner) = self.model_owner {
            values.push(Value {
                code: 330,
                data: Data::Handle(owner),
            });
        }
        values.push(text(100, b"AcDbEntity"));
        values.push(text(8, layer));
        values.extend(self.style_values(&look, &[6, 62, 420, 430, 370, 48, 440])?);
        let kind: &'static [u8] = match &add.shape {
            NewShape::Line { start, end } => {
                values.push(text(100, b"AcDbLine"));
                point(&mut values, 10, *start)?;
                point(&mut values, 11, *end)?;
                b"LINE"
            }
            NewShape::Circle { center, radius } => {
                values.push(text(100, b"AcDbCircle"));
                point(&mut values, 10, *center)?;
                values.push(real(40, positive(*radius)?)?);
                b"CIRCLE"
            }
            NewShape::Arc {
                center,
                radius,
                start,
                end,
            } => {
                values.push(text(100, b"AcDbCircle"));
                point(&mut values, 10, *center)?;
                values.push(real(40, positive(*radius)?)?);
                values.push(text(100, b"AcDbArc"));
                values.push(real(50, *start)?);
                values.push(real(51, *end)?);
                b"ARC"
            }
            NewShape::Polyline {
                points,
                bulges,
                closed,
            } => {
                if !self.version.is_empty() && !self.since(b"AC1014") {
                    return Err(UNSUPPORTED);
                }
                if points.len() < 2
                    || bulges.as_ref().is_some_and(|bulges| {
                        bulges.len() != points.len() || bulges.iter().any(|b| !b.is_finite())
                    })
                {
                    return Err(UNSUPPORTED);
                }
                values.push(text(100, b"AcDbPolyline"));
                values.push(int(
                    90,
                    i64::try_from(points.len()).map_err(|_| UNSUPPORTED)?,
                ));
                values.push(int(70, i64::from(*closed)));
                for (index, [x, y]) in points.iter().enumerate() {
                    values.extend([real(10, *x)?, real(20, *y)?]);
                    let bulge = bulges
                        .as_ref()
                        .and_then(|bulges| bulges.get(index))
                        .copied()
                        .unwrap_or(0.0);
                    if bulge != 0.0 {
                        values.push(real(42, bulge)?);
                    }
                }
                b"LWPOLYLINE"
            }
            NewShape::Text(new) => {
                self.lettering(&mut values, new)?;
                b"TEXT"
            }
            NewShape::Symbol(new) => {
                self.placing(&mut values, new)?;
                b"INSERT"
            }
        };
        if !self.markers {
            values.retain(|value| value.code != 100);
        }
        Ok((kind, values))
    }

    /// TEXT の値（今の実装の `Born::lettering`）
    fn lettering(&self, values: &mut Vec<Value>, new: &NewText) -> Result<(), Decline> {
        if new.content.contains(['\n', '\r']) {
            return Err(UNSUPPORTED);
        }
        let height = positive(new.height)?;
        let width = match new.width_factor {
            Some(width) => positive(width)?,
            None => 1.0,
        };
        let oblique = new.oblique.unwrap_or(0.0);
        let (second, horizontal, vertical) = match new.end {
            Some((end, fit)) => {
                if new.horizontal.is_some() || new.vertical.is_some() || end == new.position {
                    return Err(UNSUPPORTED);
                }
                // 2 点の間に置く文字の回転は、2 点の向きと同じでなければならない
                let angle =
                    libm::atan2(end[1] - new.position[1], end[0] - new.position[0]).to_degrees();
                let turn = (new.rotation - angle).rem_euclid(360.0);
                if turn.min(360.0 - turn) > 1e-6 {
                    return Err(UNSUPPORTED);
                }
                (Some(end), if fit { 5 } else { 3 }, 0)
            }
            None => {
                let horizontal = new.horizontal.unwrap_or(0);
                let vertical = new.vertical.unwrap_or(0);
                if !matches!(horizontal, 0 | 1 | 2 | 4) || vertical > 3 {
                    return Err(UNSUPPORTED);
                }
                let aligned = (horizontal, vertical) != (0, 0);
                (
                    aligned.then_some(new.position),
                    i64::from(horizontal),
                    i64::from(vertical),
                )
            }
        };
        let style = self.text_style(new.style.as_deref().unwrap_or("Standard"))?;
        let content = self.encoded(&new.content)?;
        values.push(text(100, b"AcDbText"));
        point(values, 10, new.position)?;
        values.push(real(40, height)?);
        values.push(text(1, content));
        if new.rotation != 0.0 {
            values.push(real(50, new.rotation)?);
        }
        if width != 1.0 {
            values.push(real(41, width)?);
        }
        if oblique != 0.0 {
            values.push(real(51, oblique)?);
        }
        values.push(text(7, &style));
        let flags = i64::from(new.backward) * 2 + i64::from(new.upside_down) * 4;
        if flags != 0 {
            values.push(int(71, flags));
        }
        if horizontal != 0 {
            values.push(int(72, horizontal));
        }
        if let Some(second) = second {
            point(values, 11, second)?;
        }
        values.push(text(100, b"AcDbText"));
        if vertical != 0 {
            values.push(int(73, vertical));
        }
        Ok(())
    }

    /// INSERT の値（今の実装の `Born::placing`）。属性の定義を持つ定義は指せないので、ATTRIB と SEQEND は書かない
    fn placing(&self, values: &mut Vec<Value>, new: &NewSymbol) -> Result<(), Decline> {
        let name = self.block_name(&new.name)?;
        let class: &[u8] = if new.grid.is_some() {
            b"AcDbMInsertBlock"
        } else {
            b"AcDbBlockReference"
        };
        values.push(text(100, class));
        values.push(text(2, &name));
        point(values, 10, new.position)?;
        for (code, scale) in [(41, new.scale[0]), (42, new.scale[1])] {
            if !(scale.is_finite() && scale != 0.0) {
                return Err(UNSUPPORTED);
            }
            if scale != 1.0 {
                values.push(real(code, scale)?);
            }
        }
        if new.rotation != 0.0 {
            values.push(real(50, new.rotation)?);
        }
        if let Some(grid) = &new.grid {
            if grid.columns == 0 || grid.rows == 0 || (grid.columns, grid.rows) == (1, 1) {
                return Err(UNSUPPORTED);
            }
            for (code, count) in [(70, grid.columns), (71, grid.rows)] {
                let count = i16::try_from(count).map_err(|_| UNSUPPORTED)?;
                values.push(int(code, i64::from(count)));
            }
            for (code, spacing) in [(44, grid.column_spacing), (45, grid.row_spacing)] {
                values.push(real(code, spacing)?);
            }
        }
        Ok(())
    }
}
