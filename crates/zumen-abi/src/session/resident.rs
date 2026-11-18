//! 開いた図面の文書と、そこから作った図面モデルと文書エンジンを常駐させる（`docs/live.md`）。
//!
//! 図面モデルは、最初に要る読み取りで作り、そのときの予算と診断への作用を [`Replay`] に記録して持ち続ける。
//! 次からは、作り直す代わりに記録を当て、使う予算・届く診断・止まる所を、作り直したときとそろえる。
//! 記録を当てられない呼び出し（予算が足りない、中断の合図がある、入力順の収集の中など）は、今までどおり作り直す。
//!
//! DXF・DWG・JWW・SXF の文書は、最初の `apply` で文書エンジン（[`zumen_acad::AcadLive`]・[`zumen_jww::JwwLive`]・
//! [`zumen_sxf::SxfLive`]）も作り、持ち続ける。文書エンジンで当てたら、文書の正は文書エンジンと、それが保存した
//! バイト列（[`Saved`]）になり、ファイルモデルは読み直さない（[`Resident::commit_live`]）。図面モデルは文書エンジンから
//! 組み立て、ファイルモデルは、それが要る op（書き出し・表の詳細・今までの経路の `apply` など）が初めて求めたときに、
//! 保存したバイト列から読み直す。ファイルモデルを書き換えるときは [`Resident::file_mut`] を通し、常駐の図面モデルと
//! 文書エンジンを捨てる。

use std::borrow::Cow;
use std::cell::OnceCell;

use zumen_base::{Cx, Replay};
use zumen_drawing::Drawing2D;

use crate::formats::DrawingFile;
#[cfg(feature = "drawing")]
use crate::packed::Packed;
#[cfg(feature = "drawing")]
use zumen_sxf::saf::AttributeFile;

/// 開いた図面。文書（ファイルモデルか、文書エンジンが保存したバイト列）と、常駐させた図面モデルと、文書エンジン
#[derive(Debug)]
pub(crate) struct Resident {
    model: Model,
    lifted: OnceCell<Box<Lifted>>,
    live: Live,
}

/// 文書の正
#[derive(Debug)]
enum Model {
    /// ファイルモデル
    Held(Box<DrawingFile>),
    /// 文書エンジンで当てたあとの文書。ファイルモデルは、要るときに読み直して `file` に置く
    #[cfg(feature = "drawing")]
    Saved {
        saved: Box<Saved>,
        file: OnceCell<Box<DrawingFile>>,
    },
}

/// 文書エンジンが保存したバイト列。今の文書の内容で、文書エンジンを捨てたときも、ここから読み直せる
#[cfg(feature = "drawing")]
#[derive(Debug)]
pub(crate) struct Saved {
    /// 形式のバイト列。P2Z・SFZ は、アーカイブではなく中の SXF のファイル
    pub(crate) bytes: Vec<u8>,
    pub(crate) kind: SavedKind,
}

/// 保存したバイト列の形式
#[cfg(feature = "drawing")]
#[derive(Debug)]
pub(crate) enum SavedKind {
    Dxf,
    Dwg,
    Jww,
    /// SXF。アーカイブ（P2Z・SFZ）と属性ファイル（SAF）は、開いたときのものを持ち続ける
    Sxf(Option<Box<Packed>>, Option<Box<AttributeFile>>),
}

/// 文書エンジンの状態。文書と同じ内容を持つ
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
            model: Model::Held(file),
            lifted: OnceCell::new(),
            live: Live::Absent,
        }
    }

    /// 読み直さずに持っているファイルモデル。文書エンジンで当てたあと、まだ読み直していなければ `None`。
    /// SIMA・TIFF・プラグイン・図面モデルのバイナリの文書は、文書エンジンを持たないので、いつも返す
    pub(crate) fn held(&self) -> Option<&DrawingFile> {
        match &self.model {
            Model::Held(file) => Some(file),
            #[cfg(feature = "drawing")]
            Model::Saved { file, .. } => file.get().map(|file| &**file),
        }
    }

    /// 形式の台帳のキー。ファイルモデルを読み直さずに答える
    pub(crate) fn format(&self) -> &str {
        match &self.model {
            Model::Held(file) => file.format(),
            #[cfg(feature = "drawing")]
            Model::Saved { saved, .. } => saved.kind.format(),
        }
    }

    /// 形式の系統。ファイルモデルを読み直さずに答える
    pub(crate) fn family(&self) -> &str {
        match &self.model {
            Model::Held(file) => file.family(),
            #[cfg(feature = "drawing")]
            Model::Saved { saved, .. } => saved.kind.family(),
        }
    }

    /// ファイルモデルを借りる。文書エンジンで当てたあとなら、保存したバイト列から読み直す。
    ///
    /// 読み直しの予算は呼び出し元と共有し、診断は返さない（開いたときに返した診断と、当てたときに返した診断に
    /// 含まれる）。止まったときと読めなかったときは、その理由を返す
    pub(crate) fn file(&self, cx: &mut Cx<'_>) -> Result<&DrawingFile, zumen_sdk::Error> {
        match &self.model {
            Model::Held(file) => Ok(file),
            #[cfg(feature = "drawing")]
            Model::Saved { saved, file } => {
                if let Some(file) = file.get() {
                    return Ok(file);
                }
                let read = saved.read(cx)?;
                Ok(file.get_or_init(|| Box::new(read)))
            }
        }
    }

    /// ファイルモデルを書き換えるために借りる。常駐の図面モデルと文書エンジンは捨てる。
    /// 文書エンジンで当てたあとなら、保存したバイト列から読み直す。読み直せなければ、何も捨てない
    pub(crate) fn file_mut(
        &mut self,
        cx: &mut Cx<'_>,
    ) -> Result<&mut DrawingFile, zumen_sdk::Error> {
        #[cfg(feature = "drawing")]
        if let Model::Saved { saved, file } = &mut self.model {
            let read = match file.take() {
                Some(read) => read,
                None => Box::new(saved.read(cx)?),
            };
            self.model = Model::Held(read);
        }
        #[cfg(not(feature = "drawing"))]
        let _ = cx;
        self.lifted = OnceCell::new();
        self.live = Live::Absent;
        match &mut self.model {
            Model::Held(file) => Ok(file),
            #[cfg(feature = "drawing")]
            Model::Saved { .. } => Err(zumen_sdk::Error::Invalid),
        }
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

    /// 文書エンジンで当てた結果に置き換える。文書の正は、文書エンジンと、それが保存したバイト列 `saved` になる。
    /// `file` は、当てる中で読み直したファイルモデル（読み直したときだけ）。`lifted` は、それを変換した図面モデルと記録
    #[cfg(feature = "drawing")]
    pub(crate) fn commit_live(
        &mut self,
        saved: Saved,
        file: Option<DrawingFile>,
        lifted: Option<(Drawing2D, Replay)>,
        live: Live,
    ) {
        let cell = OnceCell::new();
        if let Some(file) = file {
            let _ = cell.set(Box::new(file));
        }
        self.model = Model::Saved {
            saved: Box::new(saved),
            file: cell,
        };
        self.lifted = OnceCell::new();
        if let Some((drawing, replay)) = lifted {
            self.restore(drawing, replay);
        }
        self.live = live;
    }

    /// SXF の文書のアーカイブ（P2Z・SFZ）と属性ファイル（SAF）を移して、文書エンジンが保存したバイト列の形式にする。
    /// 続けて [`Self::commit_live`] で文書を置き換えるときだけ呼ぶ
    #[cfg(feature = "drawing")]
    pub(crate) fn take_sxf_parts(&mut self) -> SavedKind {
        let (archive, attributes) = match &mut self.model {
            Model::Held(file) => match &mut **file {
                DrawingFile::Sxf(_, archive, attributes) => (archive.take(), attributes.take()),
                _ => (None, None),
            },
            Model::Saved { saved, file } => {
                file.take();
                match &mut saved.kind {
                    SavedKind::Sxf(archive, attributes) => (archive.take(), attributes.take()),
                    _ => (None, None),
                }
            }
        };
        SavedKind::Sxf(archive, attributes)
    }

    /// SXF のアーカイブ（P2Z・SFZ）。ファイルモデルを読み直さずに答える
    #[cfg(feature = "drawing")]
    pub(crate) fn sxf_archive(&self) -> Option<&Packed> {
        match &self.model {
            Model::Held(file) => match &**file {
                DrawingFile::Sxf(_, archive, _) => archive.as_deref(),
                _ => None,
            },
            Model::Saved { saved, .. } => match &saved.kind {
                SavedKind::Sxf(archive, _) => archive.as_deref(),
                _ => None,
            },
        }
    }

    /// 変換して図面モデルを作る形式か。図面モデルを持つ文書（プラグイン・図面モデルのバイナリ）は、変換せずに貸す。
    /// 常駐させるのは、変換が予算の残りや収集先の中身を読まないと確かめた形式に限る
    fn resident(&self) -> bool {
        match &self.model {
            Model::Held(file) => match &**file {
                #[cfg(feature = "drawing")]
                DrawingFile::Dxf(_)
                | DrawingFile::Dwg(_)
                | DrawingFile::Sxf(..)
                | DrawingFile::Jww(_) => true,
                _ => false,
            },
            #[cfg(feature = "drawing")]
            Model::Saved { .. } => true,
        }
    }

    /// 図面モデルを作る。文書エンジンで当てたあとは、文書エンジンから組み立てる（`docs/live.md` の
    /// 「セッションの文書エンジン」）。組み立てられない文書は、ファイルモデルを変換する
    fn convert(&self, cx: &mut Cx<'_>) -> Result<Cow<'_, Drawing2D>, zumen_sdk::Error> {
        #[cfg(feature = "drawing")]
        if let Model::Saved { saved, .. } = &self.model
            && let Some(drawing) = self.engine_drawing(saved, cx)
        {
            return drawing.map(Cow::Owned);
        }
        self.file(cx)?.lift(cx)
    }

    /// 文書エンジンから図面モデルを組み立てる。保存したバイト列を読み直して変換したときと同じ図面モデルと
    /// 変換の診断を返す文書エンジンだけを使う。DWG の文書エンジンは DXF として変換するので診断が違い、使わない。
    /// SXF は、属性ファイル（SAF）を結ばない文書エンジンなので、属性ファイルを持つ文書では使わない
    #[cfg(feature = "drawing")]
    fn engine_drawing(
        &self,
        saved: &Saved,
        cx: &mut Cx<'_>,
    ) -> Option<Result<Drawing2D, zumen_sdk::Error>> {
        match (&self.live, &saved.kind) {
            (Live::Acad(engine), SavedKind::Dxf) => Some(engine.drawing(cx)),
            (Live::Jww(engine), SavedKind::Jww) => Some(engine.drawing(cx)),
            (Live::Sxf(engine), SavedKind::Sxf(_, None)) => Some(engine.drawing(cx)),
            _ => None,
        }
    }

    /// 変換にファイルモデルが要るなら、記録の外で先に読む。読む仕事を常駐の図面モデルの記録に入れると、
    /// 記録を当てるたびに、もう読んだファイルモデルの分まで予算を使ってしまう
    fn prepare(&self, cx: &mut Cx<'_>) -> Result<(), zumen_sdk::Error> {
        #[cfg(feature = "drawing")]
        if let Model::Saved { saved, file } = &self.model
            && file.get().is_none()
            && !matches!(
                (&self.live, &saved.kind),
                (Live::Acad(_), SavedKind::Dxf)
                    | (Live::Jww(_), SavedKind::Jww)
                    | (Live::Sxf(_), SavedKind::Sxf(_, None))
            )
        {
            self.file(cx)?;
        }
        Ok(())
    }

    /// 図面モデル（L2）を借りる。[`DrawingFile::lift`] と同じ予算を使い、同じ診断を足し、同じ所で止まる。
    /// 文書エンジンで当てたあとは、予算の使い方と止まる所は文書エンジンの仕事に従う。
    ///
    /// 常駐させた図面モデルがあり、その記録を当てられれば、それを貸す。なければ作り、最後まで通せたら常駐させる。
    pub(crate) fn lift(&self, cx: &mut Cx<'_>) -> Result<Cow<'_, Drawing2D>, zumen_sdk::Error> {
        if !self.resident() {
            return self.file(cx)?.lift(cx);
        }
        self.prepare(cx)?;
        if let Some(lifted) = self.lifted.get() {
            if cx.replay(&lifted.replay) {
                return Ok(Cow::Borrowed(&lifted.drawing));
            }
            return self.convert(cx);
        }
        match cx.record(|cx| self.convert(cx).map(Cow::into_owned)) {
            (Ok(drawing), Some(replay)) => {
                let lifted = self
                    .lifted
                    .get_or_init(|| Box::new(Lifted { drawing, replay }));
                Ok(Cow::Borrowed(&lifted.drawing))
            }
            (result, _) => result.map(Cow::Owned),
        }
    }

    /// 書き換えに使う図面モデルを、[`Self::lift`] と同じ作用で受け取る。常駐の図面モデルは複製せずに移す。
    ///
    /// 記録があれば一緒に返す。書き換えが失敗して文書が変わらなかったときは、[`Self::restore`] で戻せる
    pub(crate) fn take_lift(
        &mut self,
        cx: &mut Cx<'_>,
    ) -> Result<(Drawing2D, Option<Replay>), zumen_sdk::Error> {
        let owned = |drawing: Cow<'_, Drawing2D>| (drawing.into_owned(), None);
        if !self.resident() {
            return self.file(cx)?.lift(cx).map(owned);
        }
        self.prepare(cx)?;
        if let Some(lifted) = self.lifted.take() {
            if cx.replay(&lifted.replay) {
                let Lifted { drawing, replay } = *lifted;
                return Ok((drawing, Some(replay)));
            }
            self.lifted = OnceCell::from(lifted);
            return self.convert(cx).map(owned);
        }
        match cx.record(|cx| self.convert(cx).map(Cow::into_owned)) {
            (Ok(drawing), Some(replay)) => Ok((drawing, Some(replay))),
            (result, _) => result.map(|drawing| (drawing, None)),
        }
    }

    /// [`Self::take_lift`] で受け取った図面モデルを戻す。文書を書き換えなかったときだけ呼ぶ
    pub(crate) fn restore(&mut self, drawing: Drawing2D, replay: Replay) {
        let _ = self.lifted.set(Box::new(Lifted { drawing, replay }));
    }
}

#[cfg(feature = "drawing")]
impl SavedKind {
    fn format(&self) -> &'static str {
        use zumen_sdk::Codec;
        match self {
            Self::Dxf => zumen_acad::Dxf::FORMAT,
            Self::Dwg => zumen_acad::Dwg::FORMAT,
            Self::Jww => zumen_jww::Jww::FORMAT,
            Self::Sxf(..) => zumen_sxf::Sxf::FORMAT,
        }
    }

    fn family(&self) -> &'static str {
        match self {
            Self::Dxf | Self::Dwg => "acad",
            Self::Jww => "jww",
            Self::Sxf(..) => "sxf",
        }
    }
}

#[cfg(feature = "drawing")]
impl Saved {
    /// 保存したバイト列からファイルモデルを読み直す。読み直しの診断は返さず、止まった理由と読めなかった理由だけを返す
    fn read(&self, cx: &mut Cx<'_>) -> Result<DrawingFile, zumen_sdk::Error> {
        use zumen_base::Diagnostics;
        use zumen_sdk::Codec;
        let mut quiet = Diagnostics::with_max_places(cx.diag.max_places());
        let read = {
            let cx = &mut Cx::new(cx.budget, &mut quiet, cx.cancel);
            match &self.kind {
                SavedKind::Dxf => zumen_acad::Dxf::read(&self.bytes, cx).map(DrawingFile::Dxf),
                SavedKind::Dwg => zumen_acad::Dwg::read(&self.bytes, cx).map(DrawingFile::Dwg),
                SavedKind::Jww => zumen_jww::Jww::read(&self.bytes, cx).map(DrawingFile::Jww),
                SavedKind::Sxf(archive, attributes) => zumen_sxf::Sxf::read(&self.bytes, cx)
                    .map(|file| DrawingFile::Sxf(file, archive.clone(), attributes.clone())),
            }
        };
        match read {
            Ok(file) => Ok(file),
            Err(zumen_sdk::Error::Stopped(stop)) => {
                crate::session::stopped(cx, stop);
                Err(zumen_sdk::Error::Stopped(stop))
            }
            Err(error) => {
                for diagnostic in quiet.iter() {
                    cx.report(diagnostic.clone());
                }
                Err(error)
            }
        }
    }
}

#[cfg(test)]
#[cfg(feature = "drawing")]
mod tests;
