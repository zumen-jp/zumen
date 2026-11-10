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
