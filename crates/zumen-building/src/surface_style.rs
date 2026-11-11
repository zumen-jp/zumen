//! 面に明示された表示の値。材質・レイヤ・写像による継承や反射の式は評価しない。

/// 色の名前と、0〜1 の RGB。文書の明示値を持つ
#[derive(Clone, Debug, PartialEq)]
pub struct SurfaceColour {
    /// 元の色の名前
    pub name: Option<String>,
    /// 赤・緑・青
    pub rgb: [f64; 3],
}

/// 色を明示した値か、基の色へ掛ける係数。掛け算は行わない
#[derive(Clone, Debug, PartialEq)]
pub enum ColourOrFactor {
    /// 明示した色
    Colour(SurfaceColour),
    /// 0〜1 の係数
    Factor(f64),
}

/// 鏡面反射の鋭さの指定。反射を計算した値ではない
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SpecularHighlight {
    /// 指数。元の有限な実数を保つ
    Exponent(f64),
    /// 0〜1 の粗さ
    Roughness(f64),
}

/// 指定された反射モデル。モデルの式と既定値は適用しない
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReflectanceMethod {
    /// BLINN
    Blinn,
    /// FLAT
    Flat,
    /// GLASS
    Glass,
    /// MATT
    Matt,
    /// METAL
    Metal,
    /// MIRROR
    Mirror,
    /// PHONG
    Phong,
    /// PHYSICAL
    Physical,
    /// PLASTIC
    Plastic,
    /// STRAUSS
    Strauss,
    /// NOTDEFINED
    NotDefined,
}

/// 面のどちら側に指定した値か。法線や面の向きは評価しない
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SurfaceSide {
    /// 正側
    Positive,
    /// 負側
    Negative,
    /// 両側
    Both,
}

/// 反射モデルへ渡す元の値。色の係数や既定値を適用しない
#[derive(Clone, Debug, PartialEq)]
pub struct SurfaceRendering {
    /// 拡散反射
    pub diffuse: Option<ColourOrFactor>,
    /// 透過
    pub transmission: Option<ColourOrFactor>,
    /// 拡散透過
    pub diffuse_transmission: Option<ColourOrFactor>,
    /// 鏡面の反射
    pub reflection: Option<ColourOrFactor>,
    /// 鏡面反射の色か係数
    pub specular: Option<ColourOrFactor>,
    /// 鏡面反射の鋭さ
    pub highlight: Option<SpecularHighlight>,
    /// 指定された反射モデル
    pub method: ReflectanceMethod,
}

/// 面の色と透明度、任意の反射の指定。最終的な表示色ではない
#[derive(Clone, Debug, PartialEq)]
pub struct SurfaceShading {
    /// 面に明示された RGB。反射モデルによって使い方が異なる
    pub colour: SurfaceColour,
    /// 0 は不透明、1 は透明。省略は None のまま保持し、既定値を補わない
    pub transparency: Option<f64>,
    /// 反射の指定。単純な色の指定では None
    pub rendering: Option<SurfaceRendering>,
}

/// 形に指定する面の表示の定義。出所と、適用後の表示は含めない
#[derive(Clone, Debug, PartialEq)]
pub struct SurfaceStyle {
    /// 元のスタイルの名前
    pub name: Option<String>,
    /// 指定する面の側
    pub side: SurfaceSide,
    /// 色・透明度・反射の元の値
    pub shading: SurfaceShading,
}
