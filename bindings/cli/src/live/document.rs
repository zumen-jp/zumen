//! 常駐 IPC の文書と、表示の受領確認を直列に扱う。

use zumen_acad::AcadLive;
use zumen_base::{Cx, Diagnostic, Error, Kind, codes};
use zumen_drawing::{Affine, Applied, Changes, Edit, Layer, Restyle, Seen};
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
    let result = compose(inverse, compose(transform, sheet));
    result
        .m
        .iter()
        .all(|value| value.is_finite())
        .then_some(result)
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

    /// 表示に出る生きた行を、先に描くものから並べたもの
    pub fn order(&self, cx: &mut Cx<'_>) -> Result<Vec<RowId>, Error> {
        self.live.order(cx)
    }
}
