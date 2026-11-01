//! ZIP に入れた SXF（P2Z・SFZ、#385）。書庫の最初の `.p21`・`.sfc` の項目を SXF として読み、ほかの項目
//! （属性ファイル・画像）は書庫の中の名前で返す。書き戻すときは、SXF の項目だけを置き換え、ほかの項目は展開した
//! バイトのまま写す。書庫は `zumen-zip` の書き出し（stored、日時を固定）で作る。

#[cfg(feature = "drawing")]
use zumen_base::{Cx, Diagnostics};

#[cfg(feature = "drawing")]
use crate::session::{Failed, stopped};

/// 読んだ書庫の項目
#[derive(Clone, Debug)]
pub(crate) struct Packed {
    /// 項目の名前とバイト列。書庫の順。SXF の項目のバイト列は持たない
    entries: Vec<(String, Vec<u8>)>,
    /// SXF の項目の位置
    index: usize,
}

/// ZIP の署名（ローカルファイルの見出し）
#[cfg(feature = "drawing")]
const SIGNATURE: &[u8] = b"PK\x03\x04";

#[cfg(feature = "drawing")]
fn is_sxf(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".p21") || lower.ends_with(".sfc")
}

/// ZIP で、`.p21`・`.sfc` の項目があれば、その項目のバイト列と、書庫の項目を返す。ZIP でないか、SXF の項目が
/// なければ `None`（ifcZIP などは、ほかの読み取りが試す）。そのときは、書庫を読んだ診断を足さない
#[cfg(feature = "drawing")]
pub(crate) fn read(bytes: &[u8], cx: &mut Cx<'_>) -> Result<Option<(Vec<u8>, Packed)>, Failed> {
    if !bytes.starts_with(SIGNATURE) {
        return Ok(None);
    }
    let mut quiet = Diagnostics::new();
    let archive = {
        let mut inner = Cx::new(cx.budget, &mut quiet, cx.cancel);
        zumen_zip::read(bytes, &mut inner)
    };
    let Ok(archive) = archive else {
        return Ok(None);
    };
    let Some(index) = archive
        .entries()
        .iter()
        .position(|entry| !entry.is_directory() && is_sxf(entry.name()))
    else {
        return Ok(None);
    };
    for diagnostic in quiet.iter() {
        cx.report(diagnostic.clone());
    }
    let mut entries = Vec::with_capacity(archive.entries().len());
    let mut sxf = Vec::new();
    for (at, entry) in archive.entries().iter().enumerate() {
        let data = if entry.is_directory() {
            Vec::new()
        } else {
            archive.extract(entry, cx).map_err(|error| match error {
                zumen_sdk::Error::Stopped(stop) => stopped(cx, stop),
                // ほかの理由は、展開する処理が診断として足してある
                _ => Failed,
            })?
        };
        if at == index {
            sxf = data;
            entries.push((entry.name().to_string(), Vec::new()));
        } else {
            entries.push((entry.name().to_string(), data));
        }
    }
    Ok(Some((sxf, Packed { entries, index })))
}

impl Packed {
    /// SXF の項目のほかの項目の名前。書庫の順
    #[cfg(feature = "drawing")]
    pub(crate) fn names(&self) -> Vec<String> {
        self.entries
            .iter()
            .enumerate()
            .filter(|(at, _)| *at != self.index)
            .map(|(_, (name, _))| name.clone())
            .collect()
    }

    /// SXF の項目を `sxf` に置き換えた書庫を書く。ZIP64 が要る大きさなら `None`
    pub(crate) fn write(&self, sxf: &[u8]) -> Option<Vec<u8>> {
        let entries: Vec<(&str, &[u8])> = self
            .entries
            .iter()
            .enumerate()
            .map(|(at, (name, data))| {
                let data = if at == self.index { sxf } else { data };
                (name.as_str(), data)
            })
            .collect();
        zumen_zip::write(&entries)
    }
}
