//! 要素の形を、使い手が渡すアフィン変換で動かす（決定 0139 の `Transform`）。
//!
//! 線分と、膨らみと幅を持たない折れ線は、どのアフィン変換でも写せる。円・円弧・楕円・幅か膨らみを持つ折れ線は、
//! 相似（等方の尺度・回転・鏡映・平行移動）でだけ、元の種類のまま写せる。
//!
//! 記号と文字（TEXT）は、相似で写す。鏡映は、文字の枠の Y を反転して表す（`M R(θ) = R(φ − θ) F`。`M` は X 軸で
//! 鏡映してから `φ` 回し `s` 倍する相似、`F` は X 軸の鏡映）。記号は Y の尺度の符号を変え
//! （`M R(θ) diag(sx, sy) = R(φ − θ) diag(s sx, −s sy)`）、文字は上下の反転（`upside_down`）を入れ替える。
//! 文字の反転は、斜体と幅の倍率を当てた字形に、文字の基点のまわりで当てるものとして扱う（DXF の 71 の
//! 2・4。推定）ので、斜体の角度は変えない。書式つきの文字（MTEXT）は反転の印を持たないので、鏡映を含まない
//! 相似でだけ写し、書式つきの属性を持つ記号も鏡映しない。格子の間隔は尺度を掛け、鏡映では、記号の Y の向きに
//! 並ぶ行の間隔の符号を変える。

use crate::{
    Affine, Attribute, Edge, FormattedText, Height, Length, ModelLength, ModelPoint, PaperLength,
    PaperPoint, Piece, Placement, Polyline, Rotation, Scaled, Shape, Symbol, Text, Vertex,
};

/// 変換で写せる点
pub trait Point: Copy {
    /// 座標
    fn xy(self) -> [f64; 2];
    /// 同じ空間（用紙）の、座標が `xy` の点
    fn at(self, xy: [f64; 2]) -> Self;
}

impl Point for ModelPoint {
    fn xy(self) -> [f64; 2] {
        [self.x, self.y]
    }

    fn at(self, [x, y]: [f64; 2]) -> Self {
        Self { x, y, ..self }
    }
}

impl Point for PaperPoint {
    fn xy(self) -> [f64; 2] {
        [self.x, self.y]
    }

    fn at(self, [x, y]: [f64; 2]) -> Self {
        Self { x, y, ..self }
    }
}

/// 相似変換の、尺度・回転・鏡映
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Similarity {
    /// 等方の尺度。0 より大きい
    pub scale: f64,
    /// 回転の角度（度）。鏡映なら、X 軸で鏡映してから回す
    pub rotation: f64,
    /// 鏡映を含む
    pub mirrored: bool,
}

/// 相似とみなす、行列の成分の相対の許容差
const TOLERANCE: f64 = 1e-9;

impl Affine {
    /// 平行移動
    pub fn translation(dx: f64, dy: f64) -> Self {
        Self {
            m: [1.0, 0.0, dx, 0.0, 1.0, dy],
        }
    }

    /// 点 `(x, y)` を中心に、角度 `degrees`（度、反時計回り）だけ回す
    pub fn rotation(x: f64, y: f64, degrees: f64) -> Self {
        let radians = degrees.to_radians();
        let (sin, cos) = (libm::sin(radians), libm::cos(radians));
        Self {
            m: [
                cos,
                -sin,
                x - cos * x + sin * y,
                sin,
                cos,
                y - sin * x - cos * y,
            ],
        }
    }

    /// 向き `[x, y]` を写す（平行移動を含めない）
    pub fn vector(&self, [x, y]: [f64; 2]) -> [f64; 2] {
        let [a, b, _, d, e, _] = self.m;
        [a * x + b * y, d * x + e * y]
    }

    /// 向き `[x, y]` を写してから、元の長さにそろえる。写した向きの長さが 0 なら元のまま
    pub fn direction(&self, [x, y]: [f64; 2]) -> [f64; 2] {
        let [dx, dy] = self.vector([x, y]);
        let (was, now) = (libm::hypot(x, y), libm::hypot(dx, dy));
        if now == 0.0 {
            [x, y]
        } else {
            [dx / now * was, dy / now * was]
        }
    }

    /// 点を写す
    pub fn point<P: Point>(&self, point: P) -> P {
        let [x, y] = point.xy();
        let (x, y) = self.apply(x, y);
        point.at([x, y])
    }

    /// 相似なら、その尺度・回転・鏡映。成分が有限でないか、尺度が 0 なら `None`
    pub fn similarity(&self) -> Option<Similarity> {
        let [a, b, _, d, e, _] = self.m;
        if !self.m.iter().all(|v| v.is_finite()) {
            return None;
        }
        let scale = libm::hypot(a, d);
        if scale == 0.0 || !scale.is_finite() {
            return None;
        }
        let near = |x: f64, y: f64| (x - y).abs() <= TOLERANCE * scale;
        let mirrored = if near(a, e) && near(b, -d) {
            false
        } else if near(a, -e) && near(b, d) {
            true
        } else {
            return None;
        };
        Some(Similarity {
            scale,
            rotation: libm::atan2(d, a).to_degrees(),
            mirrored,
        })
    }

    /// 写した向きが 0 にならない（行列式が 0 でない）か
    pub fn invertible(&self) -> bool {
        let [a, b, _, d, e, _] = self.m;
        let det = a * e - b * d;
        det.is_finite() && det != 0.0
    }
}

/// 長さを `scale` 倍にする
fn scaled(length: Length, scale: f64) -> Length {
    match length {
        Length::Model(ModelLength { space, value }) => Length::Model(ModelLength {
            space,
            value: value * scale,
        }),
        Length::Paper(PaperLength(value)) => Length::Paper(PaperLength(value * scale)),
    }
}

/// 角度（度）を 0 以上 360 未満にそろえる
fn degrees(value: f64) -> f64 {
    let turned = value.rem_euclid(360.0);
    if turned >= 360.0 { 0.0 } else { turned }
}

/// 全周は、向きを保ったまま始まりと終わりに一回転の差を残す
fn full_turn(start: f64, end: f64, counterclockwise: bool) -> Option<(f64, f64)> {
    ((end - start).abs() >= 360.0).then_some(if counterclockwise {
        (0.0, 360.0)
    } else {
        (360.0, 0.0)
    })
}

/// 形を `transform` で写す。元の種類のまま写せなければ `None`
pub fn transform<P: Point>(shape: &Shape<P>, transform: &Affine) -> Option<Shape<P>> {
    if !transform.invertible() {
        return None;
    }
    let similar = transform.similarity();
    let point = |p: &P| transform.point(*p);
    Some(match shape {
        Shape::Line { start, end } => Shape::Line {
            start: point(start),
            end: point(end),
        },
        Shape::Curve(Edge::Line { start, end }) => Shape::Curve(Edge::Line {
            start: point(start),
            end: point(end),
        }),
        Shape::Circle { center, radius } => Shape::Circle {
            center: point(center),
            radius: scaled(*radius, similar?.scale),
        },
        Shape::Curve(Edge::Arc {
            center,
            radius,
            start,
            end,
            counterclockwise,
        }) => {
            let s = similar?;
            // 鏡映は角度 φ を -φ に写し、たどる向きを逆にする
            let turn = |angle: f64| match s.mirrored {
                false => degrees(angle + s.rotation),
                true => degrees(s.rotation - angle),
            };
            let counterclockwise = *counterclockwise != s.mirrored;
            let (start, end) = full_turn(*start, *end, counterclockwise)
                .unwrap_or_else(|| (turn(*start), turn(*end)));
            Shape::Curve(Edge::Arc {
                center: point(center),
                radius: scaled(*radius, s.scale),
                start,
                end,
                counterclockwise,
            })
        }
        Shape::Curve(Edge::Elliptic {
            center,
            major,
            ratio,
            start,
            end,
            counterclockwise,
        }) => {
            let s = similar?;
            // パラメーターは長軸から測るので、回しても変わらない。鏡映は短軸の向きを逆にする
            let turn = |angle: f64| match s.mirrored {
                false => angle,
                true => degrees(-angle),
            };
            let counterclockwise = *counterclockwise != s.mirrored;
            let (start, end) = full_turn(*start, *end, counterclockwise)
                .unwrap_or_else(|| (turn(*start), turn(*end)));
            Shape::Curve(Edge::Elliptic {
                center: point(center),
                major: point(major),
                ratio: *ratio,
                start,
                end,
                counterclockwise,
            })
        }
        Shape::Polyline(line) => Shape::Polyline(polyline(line, transform, similar)?),
        Shape::Text(text) => Shape::Text(lettering(text, transform, similar?)?),
        Shape::Formatted(text) => Shape::Formatted(formatting(text, transform, similar?)?),
        Shape::Symbol(symbol) => Shape::Symbol(match similar? {
            s if s.mirrored => mirroring(symbol, transform, s)?,
            s => placing(symbol, transform, s)?,
        }),
        _ => return None,
    })
}

fn polyline<P: Point>(
    line: &Polyline<P>,
    transform: &Affine,
    similar: Option<Similarity>,
) -> Option<Polyline<P>> {
    let curved = line.vertices.iter().any(|v| v.bulge != 0.0)
        || line.width.is_some()
        || !line.vertex_widths.is_empty();
    let (scale, mirrored) = match (curved, similar) {
        (false, _) => (1.0, false),
        (true, Some(s)) => (s.scale, s.mirrored),
        (true, None) => return None,
    };
    Some(Polyline {
        vertices: line
            .vertices
            .iter()
            .map(|v| Vertex {
                point: transform.point(v.point),
                // 鏡映は、弧の曲がる向きを逆にする
                bulge: if mirrored { -v.bulge } else { v.bulge },
            })
            .collect(),
        closed: line.closed,
        width: line.width.map(|w| scaled(w, scale)),
        vertex_widths: line
            .vertex_widths
            .iter()
            .map(|[a, b]| [scaled(*a, scale), scaled(*b, scale)])
            .collect(),
    })
}

fn height(height: &Height, scale: f64) -> Height {
    Height {
        value: scaled(height.value, scale),
        meaning: height.meaning.clone(),
    }
}

fn lettering<P: Point>(text: &Text<P>, transform: &Affine, s: Similarity) -> Option<Text<P>> {
    let placement = match &text.placement {
        Placement::Anchor {
            point,
            horizontal,
            vertical,
        } => Placement::Anchor {
            point: transform.point(*point),
            horizontal: *horizontal,
            vertical: *vertical,
        },
        Placement::Between { first, second, fit } => Placement::Between {
            first: transform.point(*first),
            second: transform.point(*second),
            fit: *fit,
        },
    };
    Some(Text {
        placement,
        height: height(&text.height, s.scale),
        rotation: if s.mirrored {
            degrees(s.rotation - text.rotation)
        } else {
            degrees(text.rotation + s.rotation)
        },
        upside_down: text.upside_down != s.mirrored,
        ..text.clone()
    })
}

/// 書式つきの文字を、鏡映を含まない相似で写す
fn formatting<P: Point>(
    text: &FormattedText<P>,
    transform: &Affine,
    s: Similarity,
) -> Option<FormattedText<P>> {
    if s.mirrored {
        return None;
    }
    // 内容の絶対高さや解釈できない書式は、元の符号を書き直さずに尺度を変えられない。
    if s.scale != 1.0
        && text.spans.iter().any(|span| {
            matches!(span.format.height, Some(Scaled::Value(_)))
                || matches!(span.piece, Piece::Unparsed(_))
        })
    {
        return None;
    }
    let rotation = match text.rotation {
        Rotation::Angle(angle) => Rotation::Angle(degrees(angle + s.rotation)),
        Rotation::Direction(direction) => {
            let [x, y] = transform.vector(direction);
            Rotation::Direction([x / s.scale, y / s.scale])
        }
    };
    Some(FormattedText {
        point: transform.point(text.point),
        height: height(&text.height, s.scale),
        rotation,
        width: text.width.map(|w| scaled(w, s.scale)),
        frame_height: text.frame_height.map(|h| scaled(h, s.scale)),
        ..text.clone()
    })
}

/// 属性も同じ変換で写す。書式つきの属性は、鏡映しない
fn attributes<P: Point>(
    symbol: &Symbol<P>,
    transform: &Affine,
    s: Similarity,
) -> Option<Vec<Attribute<P>>> {
    symbol
        .attributes
        .iter()
        .map(|attribute| {
            Some(Attribute {
                text: lettering(&attribute.text, transform, s)?,
                formatted: match &attribute.formatted {
                    Some(text) => Some(formatting(text, transform, s)?),
                    None => None,
                },
                ..attribute.clone()
            })
        })
        .collect()
}

fn placing<P: Point>(symbol: &Symbol<P>, transform: &Affine, s: Similarity) -> Option<Symbol<P>> {
    let attributes = attributes(symbol, transform, s)?;
    let grid = symbol.grid.as_ref().map(|grid| crate::Grid {
        column_spacing: scaled(grid.column_spacing, s.scale),
        row_spacing: scaled(grid.row_spacing, s.scale),
        ..*grid
    });
    Some(Symbol {
        position: transform.point(symbol.position),
        scale: symbol.scale.map(|value| value * s.scale),
        rotation: degrees(symbol.rotation + s.rotation),
        grid,
        attributes,
        ..symbol.clone()
    })
}

/// 記号を、鏡映を含む相似で写す。書式つきの属性を持つものは写さない。格子の行は、記号の Y の向きに並ぶので、
/// 鏡映すると行の間隔の符号が変わる
fn mirroring<P: Point>(symbol: &Symbol<P>, transform: &Affine, s: Similarity) -> Option<Symbol<P>> {
    let grid = symbol.grid.as_ref().map(|grid| crate::Grid {
        column_spacing: scaled(grid.column_spacing, s.scale),
        row_spacing: scaled(grid.row_spacing, -s.scale),
        ..*grid
    });
    let [sx, sy] = symbol.scale;
    Some(Symbol {
        position: transform.point(symbol.position),
        scale: [sx * s.scale, -sy * s.scale],
        rotation: degrees(s.rotation - symbol.rotation),
        grid,
        attributes: attributes(symbol, transform, s)?,
        ..symbol.clone()
    })
}
