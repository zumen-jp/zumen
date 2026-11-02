//! zumen の外部のプラグインのひな形（決定 0017・0243）。
//!
//! 形式は、このひな形のために決めた公開の形式「zumen の線分の一覧」（`zumen_lines`）。1 行目が `ZUMEN-LINES 1`
//! で、続く行に、線分の始点と終点の X・Y（mm）を空白で区切って書く。空の行は飛ばす。
//!
//! ```text
//! ZUMEN-LINES 1
//! 0 0 1000 0
//! 1000 0 1000 500
//! ```
//!
//! プラグインの形式を持つリポジトリは、このひな形を写し、コーデックと lift と台帳を自分の形式に変える。束ね方は
//! 次の 3 つで、どれもこの crate の [`PLUGINS`] を窓口に登録する。
//!
//! - Rust: この crate を依存に足し、[`zumen_abi1::install`] に [`PLUGINS`] を渡してからセッションを作る。
//! - wasm: この crate を `wasm32-unknown-unknown` に向けて作ると、プラグインの形式だけを持つモジュールになる
//!   （決定 0246）。コアのモジュールと同じ境界の関数に `zm_plugin_init` を足して出し、op は capabilities・open・
//!   close・describe・lift だけを持つ。読んだ図面は、lift で図面モデルのバイナリにし、コアの図面のモジュールの
//!   open に渡す。JavaScript の包みは、読み込んだときに `zm_plugin_init` を呼ぶ。
//! - ネイティブで呼ぶ言語: この crate の実行ファイル（`src/main.rs`）が「コアとプラグイン」の CLI になる。各言語
//!   からは、その CLI を `process` の形で使う。

use zumen_abi1::Plugin;
use zumen_base::{ByteRange, Code, Cx, Diagnostic, Kind, Param, ParamType, Ref, Severity};
use zumen_drawing::{
    Axes, Drawing2D, Element, Inherit, Layer, LayerId, ModelPoint, ModelSpace, Shape, SpaceId,
    Style, Unit,
};
use zumen_sdk::{Codec, Error, FileModel, Lift, RecordInfo, Registration, Rights, Signature};

/// 窓口に登録するプラグイン
pub static PLUGINS: &[Plugin] = &[Plugin::of::<Lines>(Registration::of::<Lines>(
    Rights::Public,
    "zumen_lines",
    false,
    1,
))];

/// このプラグインの診断の code。名前は形式のキーで始める（決定 0017）。文言は `catalog.json` にある
pub mod codes {
    use super::*;

    /// 1 行目が `ZUMEN-LINES 1` でない
    pub const FILE_INVALID: Code =
        Code::plugin("zumen_lines.file.invalid", Severity::Error, None, &[], &[]);

    /// 4 つの数として読めない行。その行は飛ばす
    pub const LINE_INVALID: Code = Code::plugin(
        "zumen_lines.line.invalid",
        Severity::Warning,
        None,
        &[Param::plugin("line", ParamType::UInt)],
        &[],
    );
}

const HEADER: &[u8] = b"ZUMEN-LINES 1";

/// コーデック
#[derive(Clone, Copy, Debug)]
pub struct Lines;

/// 読んだファイル。線分の行ごとに 1 つのレコード
#[derive(Clone, Debug, PartialEq)]
pub struct LinesFile {
    /// 線分
    pub lines: Vec<Line>,
}

/// 線分の行
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Line {
    /// 始点と終点の X・Y（mm）
    pub points: [f64; 4],
    /// 1 から数えた行の番号
    pub line: u64,
    /// 行のバイト範囲
    pub bytes: ByteRange,
}

impl Codec for Lines {
    type File = LinesFile;

    const FORMAT: &'static str = "zumen_lines";

    const SIGNATURES: &'static [Signature] = &[Signature::Lines { lines: &[HEADER] }];

    fn read(src: &[u8], cx: &mut Cx) -> Result<LinesFile, Error> {
        cx.check(Kind::InputBytes, src.len() as u64)?;
        let mut rows = src.split(|byte| *byte == b'\n');
        if rows.next().map(|row| row.trim_ascii()) != Some(HEADER) {
            cx.report(Diagnostic::new(codes::FILE_INVALID));
            return Err(Error::Invalid);
        }
        let mut start = HEADER.len() as u64 + 1;
        let mut lines = Vec::new();
        for (index, row) in rows.enumerate() {
            let line = index as u64 + 2;
            let bytes = ByteRange {
                start,
                end: start + row.len() as u64,
            };
            start = bytes.end + 1;
            if row.trim_ascii().is_empty() {
                continue;
            }
            cx.check(Kind::Records, 1)?;
            match numbers(row) {
                Some(points) => lines.push(Line {
                    points,
                    line,
                    bytes,
                }),
                None => cx.report(
                    Diagnostic::new(codes::LINE_INVALID)
                        .with("line", line)
                        .at(Ref::at_bytes(bytes.start, bytes.end)),
                ),
            }
        }
        Ok(LinesFile { lines })
    }

    fn write(_: &LinesFile, _: &mut Vec<u8>, _: &mut Cx) -> Result<(), Error> {
        // このひな形は読むだけ
        Err(Error::Unsupported)
    }
}

/// 空白で区切った 4 つの有限の数
fn numbers(row: &[u8]) -> Option<[f64; 4]> {
    let text = core::str::from_utf8(row).ok()?;
    let mut values = text.split_ascii_whitespace().map(str::parse::<f64>);
    let mut out = [0.0; 4];
    for slot in &mut out {
        *slot = values.next()?.ok().filter(|value| value.is_finite())?;
    }
    values.next().is_none().then_some(out)
}

impl FileModel for LinesFile {
    fn record_count(&self) -> usize {
        self.lines.len()
    }

    fn record(&self, index: usize) -> Option<RecordInfo<'_>> {
        self.lines.get(index).map(|line| RecordInfo {
            kind: b"LINE",
            bytes: Some(line.bytes),
        })
    }
}

impl Lift<Drawing2D> for LinesFile {
    fn lift(&self, cx: &mut Cx) -> Result<Drawing2D, Error> {
        cx.check(Kind::Elements, self.lines.len() as u64)?;
        let space = SpaceId(0);
        let point = |x, y| ModelPoint { space, x, y };
        let elements: Vec<Element<ModelPoint>> = self
            .lines
            .iter()
            .enumerate()
            .map(|(index, line)| {
                let [x1, y1, x2, y2] = line.points;
                Element {
                    // 図形の ID は、窓口がレコードの番号で作る（`space/0/r<番号>`。ID の規則の版 1）
                    source: Ref::at_record(index as u64),
                    layer: LayerId(0),
                    style: Style {
                        color: Inherit::Layer,
                        line_type: Inherit::Layer,
                        line_type_scale: 1.0,
                        line_weight: Some(Inherit::Layer),
                        transparency: Inherit::Layer,
                    },
                    shape: Shape::Line {
                        start: point(x1, y1),
                        end: point(x2, y2),
                    },
                }
            })
            .collect();
        Ok(Drawing2D {
            spaces: vec![ModelSpace {
                id: space,
                unit: Unit::Millimeter,
                axes: Axes::EastNorth,
                order: (0..elements.len()).collect(),
                elements,
            }],
            layers: vec![Layer {
                id: LayerId(0),
                name: "0".to_string(),
                state: None,
                style: None,
                follows_placement: false,
                source: Ref::default(),
                description: None,
            }],
            ..Drawing2D::default()
        })
    }
}

/// wasm のモジュールが出す、プラグインを登録する関数。JavaScript の包みが、読み込んだときに呼ぶ
// 境界の関数の名前で出すために no_mangle を使う。ワークスペースで禁じた unsafe を、この関数でだけ許す
#[cfg(target_family = "wasm")]
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn zm_plugin_init() {
    let _ = zumen_abi1::install(PLUGINS);
}
