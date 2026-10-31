//! BCF（BIM Collaboration Format）の bcfzip を読み書きし、レビューの文書（決定 0214・0218）に写す（決定 0220）。
//!
//! 読むのは BCF 3.0 の bcfzip。topic は指摘に、comment はコメントに、viewpoint は 3D の場所に写す。XSD は同梱せず、
//! BCF-XML 3.0 の文書が書く要素と属性だけを読む。書くときは、元の bcfzip を渡せば、変えていない topic を元の
//! バイト列のまま書き、変えた topic でも、レビューの文書に写さない要素と知らない要素・属性を保つ。

mod read;
mod write;
mod xml;

use zumen_review::Status;

pub use read::review;
pub use write::write;

/// 読んだ bcfzip。topic ごとのフォルダーと、ほかの項目を、元のバイト列のまま持つ
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Bcf {
    /// `bcf.version` の版（`3.0`）
    pub version: String,
    /// topic のフォルダー。書庫の順
    pub topics: Vec<Folder>,
    /// `bcf.version` と topic のフォルダーのほかの項目（`extensions.xml`・`project.bcfp`・`documents.xml`・
    /// `Documents/` の中など）。名前とデータ
    pub files: Vec<(String, Vec<u8>)>,
}

/// topic のフォルダー
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Folder {
    /// フォルダーの名前（topic の Guid）
    pub name: String,
    /// `markup.bcf`
    pub markup: Vec<u8>,
    /// ほかの項目（viewpoint・スナップショットなど）。フォルダーの中の名前とデータ
    pub files: Vec<(String, Vec<u8>)>,
}

/// 指摘の状態と、BCF の `TopicStatus` の文字列との対応の表
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Statuses(pub Vec<(String, Status)>);

impl Default for Statuses {
    /// `Open`・`Resolved`・`Closed`
    fn default() -> Self {
        Self(vec![
            ("Open".to_string(), Status::Open),
            ("Resolved".to_string(), Status::Resolved),
            ("Closed".to_string(), Status::Closed),
        ])
    }
}

impl Statuses {
    /// 文字列の状態。表になければ `None`
    pub fn status(&self, text: &str) -> Option<Status> {
        self.0
            .iter()
            .find(|(name, _)| name == text)
            .map(|(_, status)| *status)
    }

    /// 状態の文字列。表の初めのもの
    pub fn text(&self, status: Status) -> Option<&str> {
        self.0
            .iter()
            .find(|(_, s)| *s == status)
            .map(|(name, _)| name.as_str())
    }
}

/// 書くときの引数
#[derive(Clone, Debug, PartialEq)]
pub struct Options {
    /// 状態の対応の表
    pub statuses: Statuses,
    /// 書く場所の対象の ID。この対象の 3D の場所を viewpoint にする
    pub target: String,
    /// 種類のない指摘に書く `TopicType`
    pub kind: Option<String>,
    /// 作者のない指摘とコメントに書く作者
    pub author: Option<String>,
    /// 日時のない指摘とコメントに書く日時（RFC 3339）
    pub date: Option<String>,
}

impl Bcf {
    /// bcfzip を読む。ZIP として読めないか、`bcf.version` がないか、topic のフォルダーに `markup.bcf` がなければ
    /// 失敗する。3.0 でない版は `bcf.version.unsupported` を報告して失敗する
    pub fn read(src: &[u8], cx: &mut zumen_sdk::Cx) -> Result<Self, zumen_sdk::Error> {
        read::archive(src, cx)
    }
}
