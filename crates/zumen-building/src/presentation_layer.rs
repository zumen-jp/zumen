//! レイヤの指定値。表示・編集状態を既定値で補わない。

/// レイヤの表示と編集の指定。None は不明であり、省略ではない
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LayerState {
    /// 表示する指定
    pub on: Option<bool>,
    /// 固定する指定
    pub frozen: Option<bool>,
    /// 編集を禁止する指定
    pub blocked: Option<bool>,
}

/// レイヤの意味値。割り当て先と面の表示、出所は別に持つ
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PresentationLayer {
    /// 元の名前。空文字列も保持する
    pub name: String,
    /// 説明
    pub description: Option<String>,
    /// 文書内の識別子
    pub identifier: Option<String>,
    /// 表示を持つレイヤの指定。通常のレイヤでは None
    pub state: Option<LayerState>,
}
