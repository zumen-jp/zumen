//! 計算中の SI の値と、利用者が明示した出力単位の対応。

/// 長さの基準単位。計算の単位と表示の単位を分ける。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LengthUnit {
    /// ミリメートル。
    Millimeter,
    /// センチメートル。
    Centimeter,
    /// メートル。
    Meter,
    /// 国際インチ。
    Inch,
    /// 国際フィート。
    Foot,
}

impl LengthUnit {
    /// 1 単位の長さをメートルで返す。
    pub fn meters(self) -> f64 {
        match self {
            Self::Millimeter => 0.001,
            Self::Centimeter => 0.01,
            Self::Meter => 1.0,
            Self::Inch => 0.0254,
            Self::Foot => 0.3048,
        }
    }
}

/// 次元を含む出力単位。面積に長さの単位を指定することを許さない。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unit {
    /// 長さ。
    Length(LengthUnit),
    /// 平方した単位。
    Area(LengthUnit),
    /// 立方した単位。
    Volume(LengthUnit),
}

impl Unit {
    /// 長さの次元の指数。
    pub fn dimension(self) -> u8 {
        match self {
            Self::Length(_) => 1,
            Self::Area(_) => 2,
            Self::Volume(_) => 3,
        }
    }

    /// 1 単位を SI の長さ・面積・体積で表す倍率。
    pub fn si_scale(self) -> f64 {
        match self {
            Self::Length(u) => u.meters(),
            Self::Area(u) => u.meters() * u.meters(),
            Self::Volume(u) => u.meters() * u.meters() * u.meters(),
        }
    }
}
