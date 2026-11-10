//! 評価前の曲線の定義。座標と長さは文書の単位、円の媒介変数は文書の平面角の単位で持つ。
//!
//! 曲線を折れ線化せず、共有する基の曲線を参照で持つ。参照はこのグラフ内だけで有効で、要素の ID やハッシュではない。
//! 出所は意味の値から分ける。建物のコンポーネントと要素ハッシュへの統合は #419 の残件。

use crate::Unit;

/// 2 次元か 3 次元の座標、または向きの比
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Coordinates {
    /// 2 次元。z を補わない
    Two([f64; 2]),
    /// 3 次元
    Three([f64; 3]),
}

impl Coordinates {
    /// 次元の数
    pub fn dimension(self) -> u8 {
        match self {
            Self::Two(_) => 2,
            Self::Three(_) => 3,
        }
    }
}

/// 同じ [`CurveGraph`] の中の曲線への参照。グラフ間では使い回さない
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct CurveRef(pub u32);

/// 曲線の出所。意味の値とハッシュには含めない
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CurveSource {
    /// 曲線を定義する L1 のインスタンスの番号
    pub instance: Option<u64>,
    /// この曲線が直接使う点・向き・配置・区間などのインスタンス。読んだ順。同じものも残す
    /// 合成の曲線では、`CurveKind::Composite::segments` と同じ順の区間の出所
    pub records: Vec<u64>,
}

/// 円の局所の座標空間。軸の比を正規化せず保持する
#[derive(Clone, Debug, PartialEq)]
pub struct CurvePosition {
    /// 中心。2 次元か 3 次元
    pub location: Coordinates,
    /// 局所の z 軸。3 次元だけ。参照方向とともに指定するか、ともに省略する。なければ [0, 0, 1]
    pub axis: Option<[f64; 3]>,
    /// 局所の x 軸の参照方向。3 次元では z 軸に垂直な面へ射影して使う。
    /// 3 次元では z 軸とともに指定するか、ともに省略する。なければ X 軸、2 次元では [1, 0] を使う
    pub reference_direction: Option<Coordinates>,
}

/// 添字で定義する区間。点の番号は元形式と同じ 1 から
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IndexedSegment {
    /// 順に結ぶ折れ線。2 点以上
    Line(Vec<u32>),
    /// 始点・弧上の点・終点の 3 点で定義する弧
    Arc([u32; 3]),
}

/// 合成の曲線の区間のつなぎ方
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CurveTransition {
    /// 不連続
    Discontinuous,
    /// 位置が連続
    Continuous,
    /// 位置と接線が連続
    SameGradient,
    /// 位置・接線・曲率が連続
    SameGradientSameCurvature,
}

/// 合成の曲線の 1 区間
#[derive(Clone, Debug, PartialEq)]
pub struct CurveSegment {
    /// 基の曲線
    pub curve: CurveRef,
    /// 基の曲線と同じ向きか。点の順は変えず、この値で向きを残す
    pub same_sense: bool,
    /// 次の区間とのつなぎ方
    pub transition: CurveTransition,
}

/// 曲線を切る位置。点と媒介変数の両方があれば両方を保持する
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CurveTrim {
    /// 基の曲線の座標空間の点
    Point(Coordinates),
    /// 基の曲線の媒介変数。円では [`CurveGraph::angle_unit`]、直線では方向の大きさに掛ける無次元の値
    Parameter(f64),
}

/// 曲線を切る位置に点と媒介変数の両方があるときの優先する表現
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrimmingPreference {
    /// 点
    Cartesian,
    /// 媒介変数
    Parameter,
    /// 指定なし
    Unspecified,
}

/// 評価前の曲線の意味の値。近似・向きの反転・閉じる点の追加は行わない
#[derive(Clone, Debug, PartialEq)]
pub enum CurveKind {
    /// 点を順に結ぶ。最初と最後が同じ点なら閉じる
    Polyline(Vec<Coordinates>),
    /// 無限の直線。媒介変数に、方向を正規化したものと大きさを掛けて点へ加える
    Line {
        /// 基点
        point: Coordinates,
        /// 方向の比
        direction: Coordinates,
        /// 方向の大きさ。文書の長さの単位
        magnitude: f64,
    },
    /// 局所の XY 平面の円。始点は局所の x 軸、正の向きは z 軸に右ねじ
    Circle {
        /// 円の局所の座標空間
        position: CurvePosition,
        /// 半径。文書の長さの単位
        radius: f64,
    },
    /// 添字の折れ線と弧。区間の省略時は点を順に直線で結ぶ
    Indexed {
        /// 点
        points: Vec<Coordinates>,
        /// 区間。省略と空の列は区別する
        segments: Option<Vec<IndexedSegment>>,
        /// 自己交差の申告値。幾何の検査の結果ではない
        self_intersect: Option<bool>,
    },
    /// 合成の曲線。区間の順と向きをそのまま持つ
    Composite {
        /// 区間
        segments: Vec<CurveSegment>,
        /// 自己交差の申告値。`None` は不明
        self_intersect: Option<bool>,
    },
    /// 基の曲線の一部
    Trimmed {
        /// 基の曲線
        basis: CurveRef,
        /// 初めの位置
        trim1: Vec<CurveTrim>,
        /// 終わりの位置
        trim2: Vec<CurveTrim>,
        /// 基の曲線と同じ向きか
        sense_agreement: bool,
        /// 優先する表現
        preference: TrimmingPreference,
    },
    /// 型を持たない種類。定義は L1 に残す
    Unsupported(String),
}

/// 1 つの曲線。意味の値と出所を分けて持つ
#[derive(Clone, Debug, PartialEq)]
pub struct CurveItem {
    /// 意味の値
    pub kind: CurveKind,
    /// 元のインスタンスと直接使ったレコード
    pub source: CurveSource,
}

/// 必要な曲線だけを写したグラフ。子の曲線の共有を保ち、再帰した所有権を持たない
#[derive(Clone, Debug, PartialEq)]
pub struct CurveGraph {
    /// 求めた曲線
    pub root: CurveRef,
    /// 曲線。参照の先を初めて訪れた順
    pub items: Vec<CurveItem>,
    /// 座標・長さの単位。ないときは不明
    pub length_unit: Option<Unit>,
    /// 円の媒介変数の平面角の単位。ないときは不明で、度やラジアンと決めない
    pub angle_unit: Option<Unit>,
}

impl CurveGraph {
    /// このグラフ内の曲線。範囲の外の参照は `None`
    pub fn item(&self, reference: CurveRef) -> Option<&CurveItem> {
        self.items.get(reference.0 as usize)
    }
}
