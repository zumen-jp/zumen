//! 基盤地図情報ダウンロードデータの配布のアーカイブ（ZIP、ファイル仕様書 4.1 節、決定 0251）。
//!
//! アーカイブの中の `FG-GML-` で始まる `.xml` の項目を基盤地図情報の GML として読み、1 つの地理の領域モデルにまとめる。
//! 全項目のアーカイブが提供クラスごとのアーカイブを入れていても読めるよう、`FG-GML-` で始まる `.zip` の項目は 1 段だけ開く。

use zumen_base::{Diagnostic, Kind, codes};
use zumen_geo::GeoModel;
use zumen_sdk::{Codec, Cx, Error, Lift, Signature};
use zumen_zip::Archive;

use crate::Fgd;

/// アーカイブの判定の規則。中の項目の名前（「基盤地図情報データのファイル名について」の `FG-GML-…`）で、ほかの ZIP の形式と
/// 分ける
pub const ARCHIVE: &[Signature] = &[Signature::ZipName {
    prefixes: &[PREFIX.as_bytes()],
    suffixes: &[b".xml", b".zip"],
}];

/// 項目の名前の始まり
const PREFIX: &str = "FG-GML-";

/// アーカイブを読み、中の GML を 1 つの地理の領域モデルにまとめる。地物の ID は、項目の名前と `#` と、GML の中の ID
/// （`FG-GML-533945-RdEdg-20250801-0001.xml#K1_1` など）。入れ子のアーカイブの項目は、外の項目の名前と `/` でつなぐ
pub fn lift_archive(src: &[u8], cx: &mut Cx) -> Result<GeoModel, Error> {
    cx.check(Kind::InputBytes, src.len() as u64)?;
    let archive = zumen_zip::read(src, cx)?;
    let mut parts = Vec::new();
    collect(&archive, "", 0, cx, &mut parts)?;
    let mut model = GeoModel::default();
    let mut placed = false;
    for (name, bytes) in parts {
        // 読めない GML は、XML の読み取りが診断を足してある。ほかの項目は読み進める
        let part = match Fgd::read(&bytes, cx).and_then(|file| file.lift(cx)) {
            Ok(part) => part,
            Err(Error::Stopped(stop)) => return Err(Error::Stopped(stop)),
            Err(_) => continue,
        };
        let mut features = part.features;
        let has_shapes = features
            .iter()
            .any(|feature| !feature.geometries.is_empty());
        if has_shapes {
            if !placed {
                model.crs = part.crs;
                model.height = part.height;
                placed = true;
            } else if (part.crs, part.height) != (model.crs, model.height) {
                // 座標系の違う項目の形は、座標系の違う地物を比べないよう移さない（決定 0233）
                cx.report(Diagnostic::new(codes::FGD_CRS_UNSUPPORTED).with("name", name.as_str()));
                for feature in &mut features {
                    feature.geometries.clear();
                }
            }
        }
        let offset = model.features.len();
        for mut feature in features {
            feature.id = format!("{name}#{}", feature.id);
            feature.parent = feature.parent.map(|parent| parent + offset);
            model.features.push(feature);
        }
    }
    Ok(model)
}

/// アーカイブの中の GML の項目の名前とバイト列。`FG-GML-` で始まらない項目は診断する
fn collect(
    archive: &Archive<'_>,
    outer: &str,
    depth: u32,
    cx: &mut Cx,
    out: &mut Vec<(String, Vec<u8>)>,
) -> Result<(), Error> {
    for entry in archive.entries() {
        if entry.is_directory() {
            continue;
        }
        let name = entry.name();
        let base = name.rsplit('/').next().unwrap_or(name);
        let lower = base.to_ascii_lowercase();
        let full = format!("{outer}{name}");
        let named = lower.starts_with(&PREFIX.to_ascii_lowercase());
        if named && lower.ends_with(".xml") {
            out.push((full, archive.extract(entry, cx)?));
        } else if named && lower.ends_with(".zip") && depth == 0 {
            let inner = archive.open(entry, cx)?;
            collect(&inner, &format!("{full}/"), depth + 1, cx, out)?;
        } else {
            cx.report(Diagnostic::new(codes::FGD_ENTRY_UNSUPPORTED).with("name", full.as_str()));
        }
    }
    Ok(())
}
