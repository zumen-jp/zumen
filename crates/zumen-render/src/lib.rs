//! シーンの列から SVG 1.1 を書く（決定 0272）。外部のフォント・画像は取得しない。

mod scene;
mod svg;
mod writer;

use zumen_base::{Cx, Diagnostic, Stop, codes};

/// SVG の出力設定。座標はシーンの起点と同じ座標系で渡す
#[derive(Clone, Copy, Debug)]
pub struct Options {
    /// 幅（ピクセル）。0 は不可
    pub width: u32,
    /// 高さ（ピクセル）。0 は不可
    pub height: u32,
    /// 表示範囲 `[xmin, ymin, xmax, ymax]`。縦横同率で中央に収める
    pub region: [f64; 4],
    /// 用紙 mm の線幅をピクセルにする DPI。正の有限値
    pub dpi: f64,
    /// 背景の sRGB と alpha
    pub background: [u8; 4],
    /// 背景で変わる色と未解決の色に使う sRGB と alpha
    pub foreground: [u8; 4],
}

/// SVG を作れなかった理由
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// シーンの列・参照・数値が契約に合わない
    Scene,
    /// 設定か、座標を出力へ移す計算が有限の範囲に収まらない
    Options,
    /// 出力のメモリを確保できない
    Allocation,
    /// 予算か中断の合図で止まった
    Stopped(Stop),
}

impl From<Stop> for Error {
    fn from(value: Stop) -> Self {
        Self::Stopped(value)
    }
}

impl Error {
    fn diagnostic(self) -> Diagnostic {
        match self {
            Self::Scene => Diagnostic::new(codes::RENDER_SCENE_INVALID),
            Self::Options => Diagnostic::new(codes::RENDER_OPTIONS_INVALID),
            Self::Allocation => Diagnostic::new(codes::RENDER_MEMORY_EXCEEDED),
            Self::Stopped(stop) => stop.to_diagnostic(),
        }
    }
}

/// シーンのバイト列から SVG を作る。成功したときだけ全体のバイト列を返す。
///
/// シーンを別のモデルに写さず、借りた列を検査して読む。描かないもの・色の代替と、失敗の理由は `cx.diag` に残す。
/// 文字はシーンの字形の輪郭だけを書く。出力の字形と ID の規則は変えない。
pub fn svg(bytes: &[u8], options: &Options, cx: &mut Cx<'_>) -> Result<Vec<u8>, Error> {
    let result = svg::render(bytes, options, cx);
    if let Err(error) = result {
        cx.report(error.diagnostic());
    }
    result
}
