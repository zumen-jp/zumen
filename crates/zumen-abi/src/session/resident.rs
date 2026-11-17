//! 開いた図面のファイルモデルと、そこから作った図面モデルを常駐させる（`docs/live.md`）。
//!
//! 図面モデルは、最初に要る読み取りで作り、そのときの予算と診断への作用を [`Replay`] に記録して持ち続ける。
//! 次からは、作り直す代わりに記録を当て、使う予算・届く診断・止まる所を、作り直したときとそろえる。
//! 記録を当てられない呼び出し（予算が足りない、中断の合図がある、入力順の収集の中など）は、今までどおり作り直す。
//! ファイルモデルを書き換えるときは [`Resident::file_mut`] を通し、常駐の図面モデルと文書エンジンを捨てる。
//!
//! DXF・DWG・JWW・SXF の文書は、最初の `apply` で文書エンジン（[`zumen_acad::AcadLive`]・[`zumen_jww::JwwLive`]・
//! [`zumen_sxf::SxfLive`]）も作り、持ち続ける。正はファイルモデルで、文書エンジンはその写しにする。文書エンジンで
//! 当てたら、保存したバイト列からファイルモデルを読み直して置き換える（[`Resident::commit_live`]）。写しが食い違う
//! おそれのある経路では捨てる。

use std::borrow::Cow;
use std::cell::OnceCell;
use std::ops::Deref;

use zumen_base::{Cx, Replay};
use zumen_drawing::Drawing2D;

use crate::formats::DrawingFile;

/// 開いた図面。ファイルモデルと、常駐させた図面モデルと、文書エンジン
#[derive(Debug)]
pub(crate) struct Resident {
    file: Box<DrawingFile>,
    lifted: OnceCell<Box<Lifted>>,
    live: Live,
}

/// 文書エンジンの状態。ファイルモデルと同じ内容を持つ
#[derive(Default)]
pub(crate) enum Live {
    /// まだ作っていない
    #[default]
    Absent,
    /// 作れなかった。ファイルモデルを書き換えるまで作り直さない
    #[cfg(feature = "drawing")]
    Refused,
    /// DXF（ASCII）・DWG の文書エンジン
    #[cfg(feature = "drawing")]
    Acad(Box<zumen_acad::AcadLive>),
    /// JWW の文書エンジン
    #[cfg(feature = "drawing")]
    Jww(Box<zumen_jww::JwwLive>),
    /// SXF（SFC・P21）の文書エンジン
    #[cfg(feature = "drawing")]
    Sxf(Box<zumen_sxf::SxfLive>),
}

impl std::fmt::Debug for Live {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Absent => "Absent",
            #[cfg(feature = "drawing")]
            Self::Refused => "Refused",
            #[cfg(feature = "drawing")]
            Self::Acad(_) => "Acad",
            #[cfg(feature = "drawing")]
            Self::Jww(_) => "Jww",
            #[cfg(feature = "drawing")]
            Self::Sxf(_) => "Sxf",
        })
    }
}

/// 常駐させた図面モデルと、それを作ったときの作用
#[derive(Debug)]
struct Lifted {
    drawing: Drawing2D,
    replay: Replay,
}

impl Resident {
    pub(crate) fn new(file: Box<DrawingFile>) -> Self {
        Self {
            file,
            lifted: OnceCell::new(),
            live: Live::Absent,
        }
    }

    /// ファイルモデルを書き換えるために借りる。常駐の図面モデルと文書エンジンは捨てる
    pub(crate) fn file_mut(&mut self) -> &mut DrawingFile {
        self.lifted = OnceCell::new();
        self.live = Live::Absent;
        &mut self.file
    }

    /// 文書エンジンを受け取る。当てたあとは [`Self::commit_live`]、文書を変えなかったときは [`Self::put_live`] で戻す。
    /// 戻さなければ、次の `apply` で作り直す
    #[cfg(feature = "drawing")]
    pub(crate) fn take_live(&mut self) -> Live {
        std::mem::take(&mut self.live)
    }

    /// 文書を変えなかったときに、[`Self::take_live`] で受け取った文書エンジンを戻す
    #[cfg(feature = "drawing")]
    pub(crate) fn put_live(&mut self, live: Live) {
        self.live = live;
    }

    /// 文書エンジンで当てた結果に置き換える。`update` は、文書エンジンが保存したバイト列から読み直したファイルモデルを
    /// 置く（SXF のアーカイブと属性ファイルは残す）。`lifted` は、それを変換した図面モデルと記録（変換したときだけ）
    #[cfg(feature = "drawing")]
    pub(crate) fn commit_live(
        &mut self,
        update: impl FnOnce(&mut DrawingFile),
        lifted: Option<(Drawing2D, Replay)>,
        live: Live,
    ) {
        update(&mut self.file);
        self.lifted = OnceCell::new();
        if let Some((drawing, replay)) = lifted {
            self.restore(drawing, replay);
        }
        self.live = live;
    }

    /// 変換して図面モデルを作る形式か。図面モデルを持つ文書（プラグイン・図面モデルのバイナリ）は、変換せずに貸す。
    /// 常駐させるのは、変換が予算の残りや収集先の中身を読まないと確かめた形式に限る
    fn resident(&self) -> bool {
        match self.file.as_ref() {
            #[cfg(feature = "drawing")]
            DrawingFile::Dxf(_)
            | DrawingFile::Dwg(_)
            | DrawingFile::Sxf(..)
            | DrawingFile::Jww(_) => true,
            _ => false,
        }
    }

    /// 図面モデル（L2）を借りる。[`DrawingFile::lift`] と同じ予算を使い、同じ診断を足し、同じ所で止まる。
    ///
    /// 常駐させた図面モデルがあり、その記録を当てられれば、それを貸す。なければ変換し、最後まで通せたら常駐させる。
    pub(crate) fn lift(&self, cx: &mut Cx<'_>) -> Result<Cow<'_, Drawing2D>, zumen_sdk::Error> {
        if !self.resident() {
            return self.file.lift(cx);
        }
        if let Some(lifted) = self.lifted.get() {
            if cx.replay(&lifted.replay) {
                return Ok(Cow::Borrowed(&lifted.drawing));
            }
            return self.file.lift(cx);
        }
        match cx.record(|cx| self.file.lift(cx)) {
            (Ok(drawing), Some(replay)) => {
                let lifted = self.lifted.get_or_init(|| {
                    Box::new(Lifted {
                        drawing: drawing.into_owned(),
                        replay,
                    })
                });
                Ok(Cow::Borrowed(&lifted.drawing))
            }
            (result, _) => result,
        }
    }

    /// 書き換えに使う図面モデルを、[`Self::lift`] と同じ作用で受け取る。常駐の図面モデルは複製せずに移す。
    ///
    /// 記録があれば一緒に返す。書き換えが失敗してファイルモデルが変わらなかったときは、[`Self::restore`] で戻せる
    pub(crate) fn take_lift(
        &mut self,
        cx: &mut Cx<'_>,
    ) -> Result<(Drawing2D, Option<Replay>), zumen_sdk::Error> {
        let owned = |drawing: Cow<'_, Drawing2D>| (drawing.into_owned(), None);
        if !self.resident() {
            return self.file.lift(cx).map(owned);
        }
        if let Some(lifted) = self.lifted.take() {
            if cx.replay(&lifted.replay) {
                let Lifted { drawing, replay } = *lifted;
                return Ok((drawing, Some(replay)));
            }
            self.lifted = OnceCell::from(lifted);
            return self.file.lift(cx).map(owned);
        }
        match cx.record(|cx| self.file.lift(cx)) {
            (Ok(drawing), Some(replay)) => Ok((drawing.into_owned(), Some(replay))),
            (result, _) => result.map(owned),
        }
    }

    /// [`Self::take_lift`] で受け取った図面モデルを戻す。ファイルモデルを書き換えなかったときだけ呼ぶ
    pub(crate) fn restore(&mut self, drawing: Drawing2D, replay: Replay) {
        let _ = self.lifted.set(Box::new(Lifted { drawing, replay }));
    }
}

impl Deref for Resident {
    type Target = DrawingFile;

    fn deref(&self) -> &DrawingFile {
        &self.file
    }
}

impl AsRef<DrawingFile> for Resident {
    fn as_ref(&self) -> &DrawingFile {
        &self.file
    }
}

#[cfg(test)]
#[cfg(feature = "drawing")]
mod tests;
