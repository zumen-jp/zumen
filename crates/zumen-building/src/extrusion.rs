//! 評価前の押し出しと断面の値。元のレコードの番号・配置や写像を適用した値は含めない。

use crate::CurveRef;

/// 3 次元の局所の配置。軸の比を正規化せず、省略もそのまま保持する
#[derive(Clone, Debug, PartialEq)]
pub struct AxisPlacement3 {
    /// 文書の長さの単位で持つ原点
    pub location: [f64; 3],
    /// 局所の z 軸。参照方向と同時に指定か省略。省略時の既定値は適用しない
    pub axis: Option<[f64; 3]>,
    /// 局所の x 軸の参照方向。元の比を持ち、z 軸への射影は行わない
    pub reference_direction: Option<[f64; 3]>,
}

/// 閉じた外輪だけの面の断面。穴・パラメーターで定義する断面は別の対象
#[derive(Clone, Debug, PartialEq)]
pub struct ClosedProfile {
    /// 元の断面の名前。図面の文字のまま持つ
    pub name: Option<String>,
    /// 同じ取得結果が持つ CurveGraph の中の外輪。source の番号ではない
    pub boundary: CurveRef,
}

/// 押し出しの定義。評価・近似・配置の適用は行わない。
///
/// 深さと配置の座標の単位は、併せて取得した CurveGraph の length_unit に従う。
/// この値だけでは曲線の値と単位を含まない。意味の比較には曲線の定義と単位も使い、CurveSource は含めない。
#[derive(Clone, Debug, PartialEq)]
pub struct Extrusion {
    /// 閉じた外輪の断面
    pub profile: ClosedProfile,
    /// 立体の局所の配置。省略なら None のまま保持する
    pub position: Option<AxisPlacement3>,
    /// 局所の押し出す向きの比。正規化せず、z 成分は 0 以外
    pub direction: [f64; 3],
    /// 文書の長さの単位で持つ正の有限な深さ
    pub depth: f64,
}

/// 2 次元の局所の配置。既定の軸や正規化を適用せず元の値を持つ
#[derive(Clone, Debug, PartialEq)]
pub struct AxisPlacement2 {
    /// 文書の長さの単位で持つ原点
    pub location: [f64; 2],
    /// 局所の x 軸の元の比。省略は None のまま持つ
    pub reference_direction: Option<[f64; 2]>,
}

/// 評価前の面の断面。輪郭の生成、穴の補完や配置の適用は行わない
#[derive(Clone, Debug, PartialEq)]
pub struct AreaProfile {
    /// 元の断面の名前
    pub name: Option<String>,
    /// パラメーター付き断面の局所の配置。省略と任意断面は None
    pub position: Option<AxisPlacement2>,
    /// 元の寸法、または同じ取得結果の外輪への参照
    pub kind: AreaProfileKind,
}

/// 断面の定義。長さは併せて取得した単位に従う
#[derive(Clone, Debug, PartialEq)]
pub enum AreaProfileKind {
    /// 閉じた外輪。元の STEP 番号ではなく同じ CurveGraph の参照
    ArbitraryClosed {
        /// 同じ取得結果の外輪
        boundary: CurveRef,
    },
    /// 中心を原点とする矩形の全幅と全高。正の有限な元寸法
    Rectangle {
        /// 局所 x 方向の全幅
        x_dim: f64,
        /// 局所 y 方向の全高
        y_dim: f64,
    },
    /// 中心を原点とする円の正の有限な元半径
    Circle {
        /// 元の半径
        radius: f64,
    },
}

/// 断面の種類を共通の経路で取得する押し出し。形の評価や単位換算は行わない
#[derive(Clone, Debug, PartialEq)]
pub struct ProfileExtrusion {
    /// 面の断面の定義
    pub profile: AreaProfile,
    /// 立体の局所の配置。既定値を適用しない
    pub position: Option<AxisPlacement3>,
    /// 局所の押し出す向きの元の比
    pub direction: [f64; 3],
    /// 文書の単位で持つ正の有限な深さ
    pub depth: f64,
}
