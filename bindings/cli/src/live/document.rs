//! 常駐 IPC の文書と、表示の受領確認を直列に扱う。

use zumen_acad::AcadLive;
use zumen_base::{Cx, Diagnostic, Error, Kind, codes};
use zumen_drawing::{
    Affine, Applied, Changes, Edge, Edit, Element, Layer, Length, LineType, ModelLength,
    ModelPoint, PaperLength, PaperPoint, Placed, Restyle, Seen, Shape,
};
use zumen_jww::{Jww, JwwLive};
use zumen_live::display::Display;
use zumen_live::{Revision, RowId, display_wire};
use zumen_sdk::{Codec, Signature};
use zumen_sxf::{Mode, Sxf, SxfLive};

// 編集列の固定部分を、環境共通の 1 行 256 バイトで確保前に検査する。
const _: () = assert!(core::mem::size_of::<Edit>() <= 256);

/// 行を指す編集。変換は表示の座標で受ける
#[derive(Clone, Debug)]
pub(super) enum RowEdit {
    /// 画層・色・線種の倍率・線幅を変える
    Restyle(Restyle),
    /// 動かす
    Move(Affine),
    /// 写して、写しを動かす
    Copy(Affine),
    /// 除く
    Remove,
    /// 文字の内容を替える。替えられるのは文書エンジンの `display_retexted` が受ける文字だけ
    Text(String),
}

/// Draw で足す形。値は表示の座標で、有限。半径は正
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum DrawShape {
    /// 始点と終点 `[x0, y0, x1, y1]`
    Line([f64; 4]),
    /// 中心と半径 `[x, y, r]`
    Circle([f64; 3]),
    /// 中心・半径と、始まり・終わりの角度（度、反時計回り）`[x, y, r, start, end]`
    Arc([f64; 5]),
}

/// 表示の座標の形を、`sheet`（行の座標から表示の座標への写し）の逆で、行の座標に写す。
/// 線分はどの写しでも受ける。円・円弧は、軸に沿った正の一様な拡大縮小と平行移動の写しだけを受け、半径を同じ比で直す。
/// 受けない写しか、逆に写せないか、結果が有限でなければ `None`
fn unsheet(shape: DrawShape, sheet: Affine) -> Option<DrawShape> {
    let inverse = invert(sheet)?;
    let [a, b, c, d, e, f] = inverse.m;
    let point = |x: f64, y: f64| {
        let (px, py) = (a * x + b * y + c, d * x + e * y + f);
        (px.is_finite() && py.is_finite()).then_some((px, py))
    };
    let radius = |r: f64| {
        let [sa, sb, _, sd, se, _] = sheet.m;
        let uniform = sb == 0.0 && sd == 0.0 && sa == se && sa > 0.0;
        let value = r * a;
        (uniform && value.is_finite() && value > 0.0).then_some(value)
    };
    Some(match shape {
        DrawShape::Line([x0, y0, x1, y1]) => {
            let ((x0, y0), (x1, y1)) = (point(x0, y0)?, point(x1, y1)?);
            DrawShape::Line([x0, y0, x1, y1])
        }
        DrawShape::Circle([x, y, r]) => {
            let (x, y) = point(x, y)?;
            DrawShape::Circle([x, y, radius(r)?])
        }
        DrawShape::Arc([x, y, r, start, end]) => {
            let (x, y) = point(x, y)?;
            DrawShape::Arc([x, y, radius(r)?, start, end])
        }
    })
}

/// 行の座標の形 `shape` を、点の作り方 `point` と長さの作り方 `length` で、図面モデルの形にする
fn model_shape<P: Copy>(
    shape: DrawShape,
    point: impl Fn(f64, f64) -> P,
    length: impl Fn(f64) -> Length,
) -> Shape<P> {
    match shape {
        DrawShape::Line([x0, y0, x1, y1]) => Shape::Line {
            start: point(x0, y0),
            end: point(x1, y1),
        },
        DrawShape::Circle([x, y, r]) => Shape::Circle {
            radius: length(r),
            center: point(x, y),
        },
        DrawShape::Arc([x, y, r, start, end]) => Shape::Curve(Edge::Arc {
            radius: length(r),
            center: point(x, y),
            start,
            end,
            counterclockwise: true,
        }),
    }
}

/// 雛形 `template`（足す先の原点の線分）の画層・スタイルと足す先で、行の座標の形 `shape` の要素を作る。
/// 雛形が線分でなければ `None`
fn drawn(template: Placed, shape: DrawShape) -> Option<Placed> {
    Some(match template {
        Placed::Model(element) => {
            let Shape::Line { start, .. } = element.shape else {
                return None;
            };
            let space = start.space;
            Placed::Model(Element {
                shape: model_shape(
                    shape,
                    |x, y| ModelPoint { space, x, y },
                    |value| Length::Model(ModelLength { space, value }),
                ),
                ..element
            })
        }
        Placed::Paper(element) => {
            let Shape::Line { start, .. } = element.shape else {
                return None;
            };
            let sheet = start.sheet;
            Placed::Paper(Element {
                shape: model_shape(
                    shape,
                    |x, y| PaperPoint { sheet, x, y },
                    |value| Length::Paper(PaperLength(value)),
                ),
                ..element
            })
        }
    })
}

/// 記号の定義の表示
pub(super) struct Symbols {
    /// 文書の記号の定義の数。表示の挿入の定義の値は `0..` この数
    pub total: u32,
    /// 要求した定義ごとの、`display` での先頭の行と行の数
    pub ranges: Vec<[u32; 2]>,
    /// 要求した定義の要素を、要求の順に 1 要素 1 行で並べた表示
    pub display: Display,
}

/// 実行入口の状態エラー。コアの診断とは別に応答する。
#[derive(Debug, PartialEq)]
pub(super) enum Problem {
    /// 表示の行が属する版から文書が進んでいる。
    Stale {
        expected: Revision,
        actual: Revision,
    },
    /// 指定した行が生きていない。
    Missing(RowId),
    /// コアが編集または取得を断った。
    Core(Error),
}

impl From<Error> for Problem {
    fn from(value: Error) -> Self {
        Self::Core(value)
    }
}

/// 文書エンジン。表示の座標は、DXF・DWG はモデル空間、JWW・SXF は用紙の座標
enum Engine {
    Acad(Box<AcadLive>),
    Jww(Box<JwwLive>),
    Sxf(Box<SxfLive>),
}

impl Engine {
    /// 形式の署名で文書エンジンを選んで開く。JWW・SXF のどちらでもなければ、DXF・DWG として開く
    fn open(bytes: &[u8], threads: usize, cx: &mut Cx<'_>) -> Result<Self, Error> {
        let signed =
            |signatures: &[Signature]| signatures.iter().any(|signature| signature.matches(bytes));
        if signed(Jww::SIGNATURES) {
            JwwLive::open_with_threads(bytes, threads, cx).map(|live| Self::Jww(Box::new(live)))
        } else if signed(Sxf::SIGNATURES) {
            SxfLive::open_with_threads(bytes, threads, cx).map(|live| Self::Sxf(Box::new(live)))
        } else {
            AcadLive::open_with_threads(bytes, threads, cx).map(|live| Self::Acad(Box::new(live)))
        }
    }

    fn revision(&self) -> Revision {
        match self {
            Self::Acad(live) => live.revision(),
            Self::Jww(live) => live.revision(),
            Self::Sxf(live) => live.revision(),
        }
    }

    fn state_id(&self) -> u64 {
        match self {
            Self::Acad(live) => live.state_id(),
            Self::Jww(live) => live.state_id(),
            Self::Sxf(live) => live.state_id(),
        }
    }

    fn live_count(&self) -> u32 {
        match self {
            Self::Acad(live) => live.live_count(),
            Self::Jww(live) => live.live_count(),
            Self::Sxf(live) => live.live_count(),
        }
    }

    fn display(&self) -> &Display {
        match self {
            Self::Acad(live) => live.display(),
            Self::Jww(live) => live.display(),
            Self::Sxf(live) => live.display(),
        }
    }

    fn clear_dirty(&mut self) {
        match self {
            Self::Acad(live) => live.display_mut().clear_dirty(),
            Self::Jww(live) => live.clear_display_dirty(),
            Self::Sxf(live) => live.clear_display_dirty(),
        }
    }

    /// 表の行の数（削除した行を含む）。足した行は末尾に並ぶ
    fn row_count(&self) -> usize {
        match self {
            Self::Acad(live) => live.table().len(),
            Self::Jww(live) => live.row_count(),
            Self::Sxf(live) => live.row_count(),
        }
    }

    fn seen_of_row(&self, row: RowId, cx: &mut Cx<'_>) -> Result<Option<Seen>, Error> {
        match self {
            Self::Acad(live) => live.seen_of_row(row, cx),
            Self::Jww(live) => live.seen_of_row(row, cx),
            Self::Sxf(live) => live.seen_of_row(row, cx),
        }
    }

    /// 行を雛形にして足すときの、足す先の場所の ID と、雛形の画層・スタイルを持つ要素
    fn template(&self, row: RowId, cx: &mut Cx<'_>) -> Result<Option<(String, Placed)>, Error> {
        match self {
            Self::Acad(live) => live.display_template(row, cx),
            Self::Jww(live) => live.display_template(row, cx),
            Self::Sxf(live) => live.display_template(row, cx),
        }
    }

    /// 行の座標から表示の座標への写し。DXF・DWG は恒等。1 つに決まらなければ `None`
    fn sheet(&self, row: RowId) -> Option<Affine> {
        match self {
            Self::Acad(_) => Some(Affine {
                m: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
            }),
            Self::Jww(live) => live.sheet_of_row(row),
            Self::Sxf(live) => live.sheet_of_row(row),
        }
    }

    /// 表示の座標の変換を、行の座標の変換に直す。DXF・DWG は表示の座標が行の座標なので、そのまま返す。
    /// 行の座標から表示の座標への変換が 1 つに決まらないか、逆に写せないときは `None`
    fn row_transform(&self, row: RowId, transform: Affine) -> Option<Affine> {
        let sheet = match self {
            Self::Acad(_) => return Some(transform),
            Self::Jww(live) => live.sheet_of_row(row)?,
            Self::Sxf(live) => live.sheet_of_row(row)?,
        };
        conjugate(sheet, transform)
    }

    /// 診断に書く形式の名前
    fn format(&self) -> &'static str {
        match self {
            Self::Acad(_) => "DXF",
            Self::Jww(_) => "JWW",
            Self::Sxf(live) => match live.mode() {
                Mode::Sfc => "SFC",
                Mode::P21 => "P21",
            },
        }
    }

    fn apply(&mut self, changes: &Changes, cx: &mut Cx<'_>) -> Result<Vec<Applied>, Error> {
        match self {
            Self::Acad(live) => live.apply(changes, cx),
            // 移動はレコードを足し引きしないので、ほかの要素の ID はずれない
            Self::Jww(live) => live.apply(changes, cx).map(|(applied, _)| applied),
            Self::Sxf(live) => live.apply(changes, cx),
        }
    }

    fn undo(&mut self, cx: &mut Cx<'_>) -> Result<bool, Error> {
        match self {
            Self::Acad(live) => live.undo(cx),
            Self::Jww(live) => live.undo(cx),
            Self::Sxf(live) => live.undo(cx),
        }
    }

    fn redo(&mut self, cx: &mut Cx<'_>) -> Result<bool, Error> {
        match self {
            Self::Acad(live) => live.redo(cx),
            Self::Jww(live) => live.redo(cx),
            Self::Sxf(live) => live.redo(cx),
        }
    }

    fn save(&self, cx: &mut Cx<'_>) -> Result<Vec<u8>, Error> {
        match self {
            Self::Acad(live) => live.save(cx),
            Self::Jww(live) => live.save(cx),
            Self::Sxf(live) => live.save(cx),
        }
    }

    fn layers(&self, cx: &mut Cx<'_>) -> Result<Vec<(u32, Layer)>, Error> {
        match self {
            Self::Acad(live) => live.display_layers(cx),
            Self::Jww(live) => live.display_layers(cx),
            Self::Sxf(live) => live.display_layers(cx),
        }
    }

    fn retexted(
        &self,
        row: RowId,
        content: &str,
        cx: &mut Cx<'_>,
    ) -> Result<Option<Placed>, Error> {
        match self {
            Self::Acad(live) => live.display_retexted(row, content, cx),
            Self::Jww(live) => live.display_retexted(row, content, cx),
            Self::Sxf(live) => live.display_retexted(row, content, cx),
        }
    }

    fn text(&self, row: RowId, cx: &mut Cx<'_>) -> Result<Option<(u32, String)>, Error> {
        match self {
            Self::Acad(live) => live.display_text(row, cx),
            Self::Jww(live) => live.display_text(row, cx),
            Self::Sxf(live) => live.display_text(row, cx),
        }
    }

    fn line_types(&self, cx: &mut Cx<'_>) -> Result<Vec<LineType>, Error> {
        match self {
            Self::Acad(live) => live.display_line_types(cx),
            Self::Jww(live) => live.display_line_types(cx),
            Self::Sxf(live) => live.display_line_types(cx),
        }
    }

    fn order(&self, cx: &mut Cx<'_>) -> Result<Vec<RowId>, Error> {
        match self {
            Self::Acad(live) => live.display_order(cx),
            Self::Jww(live) => live.display_order(cx),
            Self::Sxf(live) => live.display_order(cx),
        }
    }

    fn select(&self, rect: [f64; 4], out: &mut Vec<RowId>, cx: &mut Cx<'_>) -> Result<(), Error> {
        match self {
            Self::Acad(live) => live.select_rect_with_cx(rect, out, cx),
            Self::Jww(live) => live.select_rect_with_cx(rect, out, cx),
            Self::Sxf(live) => live.select_rect_with_cx(rect, out, cx),
        }
    }
}

/// `sheet` で写した座標での `transform` を、写す前の座標の変換にする（`sheet` の逆 ∘ `transform` ∘ `sheet`）。
/// `sheet` が逆に写せないか、結果が有限でなければ `None`
pub(super) fn conjugate(sheet: Affine, transform: Affine) -> Option<Affine> {
    let inverse = invert(sheet)?;
    let result = compose(inverse, compose(transform, sheet));
    result
        .m
        .iter()
        .all(|value| value.is_finite())
        .then_some(result)
}

/// `sheet` の逆の写し。逆に写せないか、値が有限でなければ `None`
fn invert(sheet: Affine) -> Option<Affine> {
    let [a, b, c, d, e, f] = sheet.m;
    let det = a * e - b * d;
    if !(det.is_finite() && det != 0.0) {
        return None;
    }
    let inverse = Affine {
        m: [
            e / det,
            -b / det,
            (b * f - c * e) / det,
            -d / det,
            a / det,
            (c * d - a * f) / det,
        ],
    };
    inverse
        .m
        .iter()
        .all(|value| value.is_finite())
        .then_some(inverse)
}

/// `outer ∘ inner`（`inner` を先に当てる）
fn compose(outer: Affine, inner: Affine) -> Affine {
    let [a, b, c, d, e, f] = outer.m;
    let [p, q, r, s, t, u] = inner.m;
    Affine {
        m: [
            a * p + b * s,
            a * q + b * t,
            a * r + b * u + c,
            d * p + e * s,
            d * q + e * t,
            d * r + e * u + f,
        ],
    }
}

/// 行番号の意味と表示の差分は、この文書の現在の版に結び付く。
pub(super) struct Document {
    live: Engine,
    sent: Option<Revision>,
    confirmed: Option<Revision>,
}

pub(super) struct DisplayReply {
    pub revision: Revision,
    pub base: Option<Revision>,
    pub bytes: Vec<u8>,
}

impl Document {
    pub fn open(bytes: &[u8], threads: usize, cx: &mut Cx<'_>) -> Result<Self, Error> {
        Ok(Self {
            live: Engine::open(bytes, threads, cx)?,
            sent: None,
            confirmed: None,
        })
    }

    pub fn revision(&self) -> Revision {
        self.live.revision()
    }

    pub fn state_id(&self) -> u64 {
        self.live.state_id()
    }

    pub fn require_revision(&self, expected: Revision) -> Result<(), Problem> {
        let actual = self.revision();
        if expected == actual {
            Ok(())
        } else {
            Err(Problem::Stale { expected, actual })
        }
    }

    /// 現在の版を送った場合だけ、同じ版の確認で表示フラグを消す。
    pub fn confirm(&mut self, revision: Revision) -> bool {
        if self.sent != Some(revision) || self.revision() != revision {
            return false;
        }
        self.live.clear_dirty();
        self.confirmed = Some(revision);
        true
    }

    /// 符号化が成功しても、確認を受けるまでは再送できる状態を保つ。
    pub fn display(
        &mut self,
        full: bool,
        cx: &mut Cx<'_>,
    ) -> Result<DisplayReply, display_wire::Error> {
        let full = full || self.confirmed.is_none();
        let bytes = display_wire::encode(self.live.display(), full, cx)?;
        let revision = self.revision();
        self.sent = Some(revision);
        Ok(DisplayReply {
            revision,
            base: if full { None } else { self.confirmed },
            bytes,
        })
    }

    /// 1 要求中の行の編集を既存 Changes の 1 トランザクションとして確定する。
    pub fn edit(
        &mut self,
        expected: Revision,
        rows: &[RowId],
        edit: &RowEdit,
        cx: &mut Cx<'_>,
    ) -> Result<Vec<Applied>, Problem> {
        self.require_revision(expected)?;
        // 環境に依らない論理サイズで、編集列のコピー前に検査する。
        cx.check(Kind::DecodedBytes, (rows.len() as u64).saturating_mul(256))
            .map_err(|stop| {
                cx.report(stop.to_diagnostic());
                Problem::Core(Error::Stopped(stop))
            })?;
        let mut edits = Vec::with_capacity(rows.len());
        for &row in rows {
            let target = self
                .live
                .seen_of_row(row, cx)?
                .ok_or(Problem::Missing(row))?;
            let (name, transform) = match edit {
                RowEdit::Remove => {
                    edits.push(Edit::Remove { target });
                    continue;
                }
                RowEdit::Restyle(style) => {
                    edits.push(Edit::Restyle {
                        target,
                        style: style.clone(),
                    });
                    continue;
                }
                RowEdit::Text(content) => {
                    let Some(element) = self.live.retexted(row, content, cx)? else {
                        // 文字でない行と、区間が書式や区切りを持つ書式つきの文字は、内容だけを替えられない
                        cx.report(
                            Diagnostic::new(codes::CHANGE_EDIT_UNSUPPORTED)
                                .with("edit", "text")
                                .with("kind", "shape")
                                .with("format", self.live.format()),
                        );
                        return Err(Problem::Core(Error::Rejected));
                    };
                    edits.push(Edit::Replace {
                        target,
                        element: Box::new(element),
                    });
                    continue;
                }
                &RowEdit::Move(transform) => ("transform", transform),
                &RowEdit::Copy(transform) => ("copy", transform),
            };
            let Some(transform) = self.live.row_transform(row, transform) else {
                // 表示の座標が 1 つに決まらない行（ビューに置いていない、または 2 つ以上のビューに置いた SXF の空間の要素）は動かせない
                cx.report(
                    Diagnostic::new(codes::CHANGE_EDIT_UNSUPPORTED)
                        .with("edit", name)
                        .with("kind", "view")
                        .with("format", self.live.format()),
                );
                return Err(Problem::Core(Error::Rejected));
            };
            edits.push(match edit {
                RowEdit::Copy(_) => Edit::Copy { target, transform },
                _ => Edit::Transform { target, transform },
            });
        }
        self.live
            .apply(
                &Changes {
                    document: None,
                    edits,
                },
                cx,
            )
            .map_err(Problem::Core)
    }

    /// 行 `template` の空間か用紙に、その画層とスタイルで形 `shape` の要素を 1 つ足す。1 トランザクションで当てる。
    /// 生きていない行は `Problem::Missing`。雛形にならない行（記号の定義の中の要素）と、表示の座標を
    /// 行の座標に写せない形は `change.edit.unsupported`（`edit` は `add`）を診断して Rejected
    pub fn draw(
        &mut self,
        expected: Revision,
        template: RowId,
        shape: DrawShape,
        cx: &mut Cx<'_>,
    ) -> Result<Vec<Applied>, Problem> {
        self.require_revision(expected)?;
        let reject = |kind: &str, format: &str, cx: &mut Cx<'_>| {
            cx.report(
                Diagnostic::new(codes::CHANGE_EDIT_UNSUPPORTED)
                    .with("edit", "add")
                    .with("kind", kind)
                    .with("format", format),
            );
            Problem::Core(Error::Rejected)
        };
        let Some((place, template_element)) = self.live.template(template, cx)? else {
            if self.live.seen_of_row(template, cx)?.is_none() {
                return Err(Problem::Missing(template));
            }
            return Err(reject("place", self.live.format(), cx));
        };
        let Some(element) = self
            .live
            .sheet(template)
            .and_then(|sheet| unsheet(shape, sheet))
            .and_then(|shape| drawn(template_element, shape))
        else {
            return Err(reject("view", self.live.format(), cx));
        };
        self.live
            .apply(
                &Changes {
                    document: None,
                    edits: vec![Edit::Add {
                        place,
                        element: Box::new(element),
                    }],
                },
                cx,
            )
            .map_err(Problem::Core)
    }

    /// 表の行の数（削除した行を含む）。写しの行は、当てる前のこの数から順に足される
    pub fn row_count(&self) -> usize {
        self.live.row_count()
    }

    pub fn undo(&mut self, expected: Revision, cx: &mut Cx<'_>) -> Result<bool, Problem> {
        self.require_revision(expected)?;
        self.live.undo(cx).map_err(Problem::Core)
    }

    pub fn redo(&mut self, expected: Revision, cx: &mut Cx<'_>) -> Result<bool, Problem> {
        self.require_revision(expected)?;
        self.live.redo(cx).map_err(Problem::Core)
    }

    pub fn save(&self, cx: &mut Cx<'_>) -> Result<Vec<u8>, Error> {
        self.live.save(cx)
    }

    pub fn target(&self, row: RowId, cx: &mut Cx<'_>) -> Result<Seen, Problem> {
        self.live.seen_of_row(row, cx)?.ok_or(Problem::Missing(row))
    }

    pub fn select(&self, rect: [f64; 4], cx: &mut Cx<'_>) -> Result<Vec<RowId>, Error> {
        let mut rows = Vec::new();
        self.live.select(rect, &mut rows, cx)?;
        Ok(rows)
    }

    pub fn live_count(&self) -> u32 {
        self.live.live_count()
    }

    /// 表示のスタイルの画層の値と、その画層の組
    pub fn layers(&self, cx: &mut Cx<'_>) -> Result<Vec<(u32, Layer)>, Error> {
        self.live.layers(cx)
    }

    /// 行の文字の種類（0 文字でない・1 文字・2 書式つきの文字）と、元の符号のままの内容。要求の順に返す。
    /// 生きていない行があれば `Problem::Missing`
    pub fn texts(&self, rows: &[RowId], cx: &mut Cx<'_>) -> Result<Vec<(u32, String)>, Problem> {
        let mut out = Vec::with_capacity(rows.len());
        for &row in rows {
            out.push(self.live.text(row, cx)?.ok_or(Problem::Missing(row))?);
        }
        Ok(out)
    }

    /// `Restyle` の線種に使える線種の表。値は図面モデルの `LineTypeId`
    pub fn line_types(&self, cx: &mut Cx<'_>) -> Result<Vec<LineType>, Error> {
        self.live.line_types(cx)
    }

    /// 記号の定義 `indices` の要素を、定義の座標のまま要求の順に並べる。範囲外の定義は Invalid
    pub fn symbols(&self, indices: &[u32], cx: &mut Cx<'_>) -> Result<Symbols, Problem> {
        let total = match &self.live {
            Engine::Acad(live) => live.display_symbol_count(),
            Engine::Sxf(live) => live.display_symbol_count(),
            Engine::Jww(live) => live.display_symbol_count(),
        };
        let mut display = Display::with_rows(0);
        let mut ranges = Vec::with_capacity(indices.len());
        let mut first = 0_u32;
        for &index in indices {
            let rows = match &self.live {
                Engine::Acad(live) => live.display_symbol(index, &mut display, cx)?,
                Engine::Sxf(live) => live.display_symbol(index, &mut display, cx)?,
                Engine::Jww(live) => live.display_symbol(index, &mut display, cx)?,
            }
            .ok_or(Problem::Core(Error::Invalid))?;
            ranges.push([first, rows]);
            first = first
                .checked_add(rows)
                .ok_or(Problem::Core(Error::Invalid))?;
        }
        Ok(Symbols {
            total,
            ranges,
            display,
        })
    }

    /// 表示に出る生きた行を、先に描くものから並べたもの
    pub fn order(&self, cx: &mut Cx<'_>) -> Result<Vec<RowId>, Error> {
        self.live.order(cx)
    }
}
