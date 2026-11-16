//! 常駐 IPC の文書と、表示の受領確認を直列に扱う。

use zumen_acad::AcadLive;
use zumen_base::{Cx, Error, Kind};
use zumen_drawing::{Affine, Applied, Changes, Edit, Seen};
use zumen_live::{Revision, RowId, display_wire};

// 編集列の固定部分を、環境共通の 1 行 256 バイトで確保前に検査する。
const _: () = assert!(core::mem::size_of::<Edit>() <= 256);

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

/// 行番号の意味と表示の差分は、この文書の現在の版に結び付く。
pub(super) struct Document {
    live: AcadLive,
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
            live: AcadLive::open_with_threads(bytes, threads, cx)?,
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
        self.live.display_mut().clear_dirty();
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

    /// 1 要求中の移動を既存 Changes の 1 トランザクションとして確定する。
    pub fn transform(
        &mut self,
        expected: Revision,
        rows: &[RowId],
        transform: Affine,
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
            edits.push(Edit::Transform { target, transform });
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
        self.live.select_rect_with_cx(rect, &mut rows, cx)?;
        Ok(rows)
    }

    pub fn live_count(&self) -> u32 {
        self.live.live_count()
    }
}
