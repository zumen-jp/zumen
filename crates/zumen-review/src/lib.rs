//! レビューの文書（決定 0214・0218）。赤入れと BIM の指摘を、指摘・図形・場所を持つ 1 つの JSON の文書にする。
//!
//! 文書は、対象（`targets`）・指摘（`topics`）・図形（`markups`）を持つ。場所と図形は対象の ID を指し、要素の参照は
//! 要素の ID とハッシュの組にする。知らないキーは、読んで書き直しても保つ。この crate は、図面モデルと建物モデルの
//! ID とハッシュを文字列で持ち、どちらのモデルにも依らない。新しい版の要素は、呼び手が [`Version`] で引く。

mod adopt;
mod carry;
mod read;
mod write;

use zumen_json::Value;

pub use adopt::{Adopted, adopt};
pub use carry::{CarriedReference, Version, carry};

/// 文書の `format`
pub const FORMAT: &str = "zumen-review";
/// 読み書きする文書の大きい版
pub const VERSION: u64 = 1;

/// 知らないキーと値。読んだ順に、書き直すときに知っているキーの後ろへ書く
pub type Extra = Vec<(String, Value)>;

/// レビューの文書
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Review {
    /// 指摘と図形が指す文書
    pub targets: Vec<Target>,
    /// 指摘
    pub topics: Vec<Topic>,
    /// 図形
    pub markups: Vec<Markup>,
    /// 知らないキー
    pub extra: Extra,
}

/// 対象の文書
#[derive(Clone, Debug, PartialEq)]
pub struct Target {
    /// 文書の中で一意の ID
    pub id: String,
    /// 形式の名前（`dxf`・`ifc` など）
    pub format: String,
    /// 元のバイト列のハッシュ（決定 0156 の `v2:`）
    pub hash: String,
    /// 名前
    pub name: Option<String>,
    /// 知らないキー
    pub extra: Extra,
}

/// 指摘の状態
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    /// まだ扱っていない
    Open,
    /// 直したが確かめていない
    Resolved,
    /// 確かめたか取り下げた
    Closed,
}

/// 指摘（BCF の topic にあたる）
#[derive(Clone, Debug, PartialEq)]
pub struct Topic {
    /// UUID
    pub id: String,
    /// 題
    pub title: String,
    /// 説明
    pub description: Option<String>,
    /// 状態
    pub status: Status,
    /// アプリケーションの細かい状態
    pub status_label: Option<String>,
    /// 種類
    pub kind: Option<String>,
    /// 優先度
    pub priority: Option<String>,
    /// ラベル
    pub labels: Vec<String>,
    /// 担当
    pub assignee: Option<String>,
    /// 期限（RFC 3339）
    pub due: Option<String>,
    /// 作者
    pub author: Option<String>,
    /// 作った日時（RFC 3339）
    pub created: Option<String>,
    /// 直した人
    pub modified_by: Option<String>,
    /// 直した日時（RFC 3339）
    pub modified: Option<String>,
    /// 関係する指摘の ID
    pub related: Vec<String>,
    /// コメント
    pub comments: Vec<Comment>,
    /// 場所
    pub locations: Vec<Location>,
    /// 照合の結果から作った指摘の出所
    pub origin: Option<Origin>,
    /// 知らないキー
    pub extra: Extra,
}

/// コメント
#[derive(Clone, Debug, PartialEq)]
pub struct Comment {
    /// ID
    pub id: String,
    /// 作者
    pub author: Option<String>,
    /// 日時（RFC 3339）
    pub date: Option<String>,
    /// 本文
    pub text: String,
    /// 指摘の場所の ID
    pub location: Option<String>,
    /// 知らないキー
    pub extra: Extra,
}

/// 照合の結果の出所。規則の id と、境界の診断の形（決定 0179）のまま持つ
#[derive(Clone, Debug, PartialEq)]
pub struct Origin {
    /// 規則の id（IDS の照合なら仕様の名前）
    pub rule: String,
    /// 診断。境界のメッセージの `diagnostic` の形
    pub diagnostic: Value,
}

/// 要素の参照。場所を作ったときの要素のハッシュと組にする
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reference {
    /// 要素の ID（決定 0156・0207）
    pub id: String,
    /// 要素のハッシュ
    pub hash: Option<String>,
}

/// 指摘の場所
#[derive(Clone, Debug, PartialEq)]
pub struct Location {
    /// 文書の中で一意の ID
    pub id: String,
    /// 対象の ID
    pub target: String,
    /// 2D か 3D の場所
    pub place: Place,
    /// 新しい版へ移したときの、元の文書のハッシュと状態
    pub carried: Option<Carried>,
    /// 知らないキー
    pub extra: Extra,
}

/// 2D か 3D の場所
#[derive(Clone, Debug, PartialEq)]
pub enum Place {
    /// 用紙かビューの上の場所。位置は用紙の mm で、Y が上
    Plane {
        /// 用紙かビューの ID（決定 0156）
        view: Option<String>,
        /// ピンの位置
        pin: Option<[f64; 2]>,
        /// 範囲（左下と右上）
        extent: Option<[f64; 4]>,
        /// 要素の参照
        elements: Vec<Reference>,
    },
    /// 3D の場所。座標はプロジェクトの空間
    Space {
        /// カメラ
        camera: Option<Camera>,
        /// 選んだ要素
        selected: Vec<Reference>,
        /// 既定で要素を見せるか
        visible: Option<bool>,
        /// 既定と逆に見せる・隠す要素
        exceptions: Vec<Reference>,
        /// 色を変えた要素
        colored: Vec<Colored>,
        /// 切断面
        clips: Vec<Clip>,
    },
}

/// カメラの投影
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Projection {
    /// 透視。視野角（度）
    Perspective(f64),
    /// 平行。見える幅
    Orthographic(f64),
}

/// カメラ
#[derive(Clone, Debug, PartialEq)]
pub struct Camera {
    /// 投影
    pub projection: Projection,
    /// 位置
    pub position: [f64; 3],
    /// 向き
    pub direction: [f64; 3],
    /// 上の向き
    pub up: [f64; 3],
}

/// 色を変えた要素
#[derive(Clone, Debug, PartialEq)]
pub struct Colored {
    /// 色（`#RRGGBB` か `#AARRGGBB`）
    pub color: String,
    /// 要素
    pub elements: Vec<Reference>,
}

/// 切断面。法線の向きを切り取る
#[derive(Clone, Debug, PartialEq)]
pub struct Clip {
    /// 面の上の点
    pub point: [f64; 3],
    /// 法線
    pub normal: [f64; 3],
}

/// 新しい版へ移したときの状態
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
    /// 変わっていない
    Unchanged,
    /// 変わった
    Changed,
    /// なくなった
    Removed,
}

/// 移したときの、元の文書と状態
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Carried {
    /// 元の文書のハッシュ
    pub from: String,
    /// 状態
    pub state: Change,
}

/// 図形の種類
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarkupKind {
    /// 赤線
    Redline,
    /// 変更雲
    Cloud,
    /// 注記
    Text,
    /// 写真
    Photo,
}

/// 図形（赤入れ）
#[derive(Clone, Debug, PartialEq)]
pub struct Markup {
    /// 文書の中で一意の ID
    pub id: String,
    /// 種類
    pub kind: MarkupKind,
    /// 対象の ID
    pub target: String,
    /// 用紙かビューの ID
    pub view: Option<String>,
    /// 点（用紙の mm、Y が上）
    pub points: Vec<[f64; 2]>,
    /// 注記の文字か、写真の名前
    pub text: Option<String>,
    /// 指摘の ID。持たなければ、指摘によらない赤入れ
    pub topic: Option<String>,
    /// 移したときの、元の文書と状態
    pub carried: Option<Carried>,
    /// 知らないキー
    pub extra: Extra,
}

impl Status {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Resolved => "resolved",
            Self::Closed => "closed",
        }
    }
}

impl Change {
    /// 文書に書く名前
    pub fn name(self) -> &'static str {
        match self {
            Self::Unchanged => "unchanged",
            Self::Changed => "changed",
            Self::Removed => "removed",
        }
    }
}

impl MarkupKind {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Redline => "redline",
            Self::Cloud => "cloud",
            Self::Text => "text",
            Self::Photo => "photo",
        }
    }
}
