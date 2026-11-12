//! レジストリに登録する形式（#398、決定 0017）。capabilities の形式の一覧と、図面の形式の判定は、この一覧から引く。
//! 登録の権利は、台帳（`formats/ledger.toml`）と合うことをテストで確かめる。
//! 形式ごとの判定・読み取りと、セッションに登録する文書の準備もここで行う。

use std::borrow::Cow;
use std::collections::BTreeMap;

use zumen_acad::{Dwg, DwgFile, Dxf, DxfFile};
use zumen_base::{Cx, Diagnostic, Kind, codes, name};
use zumen_drawing::{Drawing2D, Hash, binary};
use zumen_jww::{Jww, JwwFile};
use zumen_sdk::{Codec, Lift, Registration, Rights};
use zumen_sxf::saf::AttributeFile;
use zumen_sxf::{Sxf, SxfFile};

use crate::building;
#[cfg(any(
    feature = "drawing",
    feature = "bim",
    feature = "civil",
    feature = "pointcloud"
))]
use crate::civil;
#[cfg(feature = "civil")]
use crate::geo;
use crate::messages::{OpenRequest, OpenResult};
#[cfg(feature = "drawing")]
use crate::packed;
use crate::packed::Packed;
use crate::plugin;
#[cfg(feature = "pointcloud")]
use crate::pointcloud;
use crate::session::{Failed, fail, input_missing, invalid_value, stopped};

/// 図面の形式。判定はこの順に試す。ID の規則は、AutoCAD の系統はハンドルで、SXF はインスタンスの番号で作る 2
/// （決定 0156）。書けるのは、apply で書き戻せる形式（決定 0139）。SXF は SFC と P21 に書き戻す（決定 0167・0169）
pub(crate) const DRAWING: &[Registration] = &[
    Registration::of::<Dxf>(Rights::Public, "acad", true, 2),
    Registration::of::<Dwg>(Rights::Public, "acad", true, 2),
    Registration::of::<Sxf>(Rights::Public, "sxf", true, 2),
    Registration::of::<Jww>(Rights::Public, "jww", true, 1),
];

/// 図面のモジュールが読む、図面でない形式
pub(crate) const READ_ONLY: &[Registration] = &[
    Registration::of::<zumen_landxml::LandXml>(Rights::Public, "landxml", false, 1),
    Registration::of::<zumen_citygml::CityGml>(Rights::Public, "citygml", false, 1),
    Registration::of::<zumen_fgd::Fgd>(Rights::Public, "fgd", false, 1),
    Registration::of::<zumen_extdm::ExtDm>(Rights::Public, "extdm", false, 1),
    Registration::of::<zumen_las::Las>(Rights::Public, "las", false, 1),
    Registration::of::<zumen_ply::Ply>(Rights::Public, "ply", false, 1),
    Registration::of::<zumen_xyz::Xyz>(Rights::Public, "xyz", false, 1),
    Registration::of::<zumen_e57::E57>(Rights::Public, "e57", false, 1),
];

/// 開いた文書。形式ごとのファイルモデル（L1）を持つ。コアの形式は `drawing` の feature で持ち、プラグインのモジュール
/// （決定 0247）には、コアのコーデックを入れない
#[derive(Debug)]
pub(crate) enum DrawingFile {
    #[cfg(feature = "drawing")]
    Dxf(DxfFile),
    #[cfg(feature = "drawing")]
    Dwg(DwgFile),
    /// SXF。P2Z・SFZ なら、アーカイブの項目も持つ（#385）。属性ファイル（SAF）があれば、読んだものを持つ（決定 0249）
    #[cfg(feature = "drawing")]
    Sxf(SxfFile, Option<Box<Packed>>, Option<Box<AttributeFile>>),
    #[cfg(feature = "drawing")]
    Jww(JwwFile),
    /// プラグインの形式（決定 0243）。読むだけで、図面モデルを持つ
    Plugin(Registration, Drawing2D),
    /// 図面モデルのバイナリ（決定 0247）。lift が書いた図面を、元の文書の形式・系統のまま、読むだけの文書にする
    Lifted(binary::Source, Drawing2D),
}

impl DrawingFile {
    /// 形式の台帳のキー
    pub(crate) fn format(&self) -> &str {
        match self {
            #[cfg(feature = "drawing")]
            DrawingFile::Dxf(_) => Dxf::FORMAT,
            #[cfg(feature = "drawing")]
            DrawingFile::Dwg(_) => Dwg::FORMAT,
            #[cfg(feature = "drawing")]
            DrawingFile::Sxf(..) => Sxf::FORMAT,
            #[cfg(feature = "drawing")]
            DrawingFile::Jww(_) => Jww::FORMAT,
            DrawingFile::Plugin(registration, _) => registration.format,
            DrawingFile::Lifted(source, _) => &source.format,
        }
    }

    /// 形式の系統（capabilities の family）
    pub(crate) fn family(&self) -> &str {
        match self {
            #[cfg(feature = "drawing")]
            DrawingFile::Dxf(_) | DrawingFile::Dwg(_) => "acad",
            #[cfg(feature = "drawing")]
            DrawingFile::Sxf(..) => "sxf",
            #[cfg(feature = "drawing")]
            DrawingFile::Jww(_) => "jww",
            DrawingFile::Plugin(registration, _) => registration.family,
            DrawingFile::Lifted(source, _) => &source.family,
        }
    }

    /// 図面モデル（L2）に変換する。図面モデルを持つ文書は、複製せずに貸す
    pub(crate) fn lift(&self, cx: &mut Cx<'_>) -> Result<Cow<'_, Drawing2D>, zumen_sdk::Error> {
        match self {
            #[cfg(feature = "drawing")]
            DrawingFile::Dxf(file) => file.lift(cx).map(Cow::Owned),
            #[cfg(feature = "drawing")]
            DrawingFile::Dwg(file) => file.lift(cx).map(Cow::Owned),
            #[cfg(feature = "drawing")]
            DrawingFile::Sxf(file, _, attributes) => {
                file.lift_with(attributes.as_deref(), cx).map(Cow::Owned)
            }
            #[cfg(feature = "drawing")]
            DrawingFile::Jww(file) => file.lift(cx).map(Cow::Owned),
            DrawingFile::Plugin(_, drawing) | DrawingFile::Lifted(_, drawing) => {
                Ok(Cow::Borrowed(drawing))
            }
        }
    }
}

/// 読み終えた文書。形式の読み取りと派生モデルの準備を終えてから、セッションに登録する。
pub(crate) enum ReadDocument {
    Drawing {
        file: Box<DrawingFile>,
        hash: Hash,
    },
    Building {
        model: Box<building::Model>,
        civil: Option<Box<zumen_civil::CivilModel>>,
    },
    Civil(Box<zumen_civil::CivilModel>),
    Geo(Box<zumen_geo::GeoModel>),
    Cloud(Box<zumen_pointcloud::PointCloud>),
}

impl ReadDocument {
    /// IFC から読んだ線形を、その建物と一緒に登録・更新する。
    #[cfg(feature = "bim")]
    pub(crate) fn building(model: building::Model, cx: &mut Cx<'_>) -> Result<Self, Failed> {
        let civil = match &model.file {
            Some(file) => civil::from_ifc(file, cx)?,
            None => None,
        };
        Ok(Self::Building {
            model: Box::new(model),
            civil: civil.map(Box::new),
        })
    }
}

/// 読み取りの結果。範囲の入力が足りない場合は、応答だけを返し、文書を登録しない。
pub(crate) struct Opened {
    pub(crate) result: OpenResult,
    pub(crate) document: Option<ReadDocument>,
}

impl Opened {
    fn document(name: &str, format: impl Into<String>, document: ReadDocument) -> Self {
        Self {
            result: OpenResult {
                doc: name.to_string(),
                format: format.into(),
                needs: None,
                done: None,
                entries: None,
            },
            document: Some(document),
        }
    }

    #[cfg(feature = "bim")]
    fn building(name: &str, model: building::Model, cx: &mut Cx<'_>) -> Result<Self, Failed> {
        let format = model.format.to_string();
        Ok(Self::document(
            name,
            format,
            ReadDocument::building(model, cx)?,
        ))
    }
}

/// 入力から形式を判定し、文書を読む。判定順・予算・診断は、持つモジュールによって決まる。
pub(crate) fn read(
    request: &OpenRequest,
    bytes: &[u8],
    inputs: &BTreeMap<String, Vec<u8>>,
    cx: &mut Cx<'_>,
) -> Result<Opened, Failed> {
    let name = request.doc.as_deref().unwrap_or(&request.input);
    // IFC-SPF は SXF と同じ始まりなので先に、ifcZIP は ZIP なので最後に試す（決定 0219）。モジュールには、
    // 持つ形式の判定だけを入れる（決定 0221）
    #[cfg(feature = "bim")]
    if let Some(model) = building::read_spf(bytes, cx) {
        return Opened::building(name, model?, cx);
    }
    // 名指せる形式は、見分けるマーカーのない xyz（点群のモジュール）と extdm（土木・地理のモジュール）だけ。ほかの形式は
    // 入力から判定する
    if request.format.as_deref().is_some_and(|format| {
        !((format == zumen_xyz::Xyz::FORMAT && cfg!(feature = "pointcloud"))
            || (format == zumen_extdm::ExtDm::FORMAT && cfg!(feature = "civil")))
    }) {
        return Err(fail(cx, invalid_value("/format")));
    }
    // 属性ファイルは、SXF（P2Z・SFZ を含む）にだけ使う（決定 0249）
    if request.saf.is_some()
        && !(cfg!(feature = "drawing")
            && (Sxf::SIGNATURES.iter().any(|rule| rule.matches(bytes))
                || bytes.starts_with(b"PK\x03\x04")))
    {
        return Err(fail(cx, invalid_value("/saf")));
    }
    // 名指した拡張 DM（#378）
    #[cfg(feature = "civil")]
    if request.format.as_deref() == Some(zumen_extdm::ExtDm::FORMAT) {
        let model = geo::read_extdm(bytes, cx)?;
        return Ok(Opened::document(
            name,
            zumen_extdm::ExtDm::FORMAT,
            ReadDocument::Geo(Box::new(model)),
        ));
    }
    // LAS は範囲で読む（決定 0232）。LAZ は、ヘッダーを読んで断る（決定 0289）
    #[cfg(feature = "pointcloud")]
    if let Some(opened) = pointcloud::read(&request.input, inputs, request.size, cx) {
        let format = zumen_las::Las::FORMAT.to_string();
        return Ok(match opened? {
            pointcloud::Opened::Cloud(cloud, done) => {
                let mut opened = Opened::document(name, format, ReadDocument::Cloud(cloud));
                opened.result.done = Some(done).filter(|done| !done.is_empty());
                opened
            }
            pointcloud::Opened::Need(needs) => Opened {
                result: OpenResult {
                    doc: name.to_string(),
                    format,
                    needs: Some(needs),
                    done: None,
                    entries: None,
                },
                document: None,
            },
        });
    }
    // PLY・E57 と、名指した XYZ
    #[cfg(feature = "pointcloud")]
    if let Some(opened) = pointcloud::read_whole(bytes, request.format.as_deref(), cx) {
        let (format, cloud) = opened?;
        return Ok(Opened::document(
            name,
            format,
            ReadDocument::Cloud(Box::new(cloud)),
        ));
    }
    #[cfg(feature = "drawing")]
    let doc = with_attributes(read_drawing(bytes, cx)?, request.saf.as_deref(), inputs, cx)?;
    // プラグインのモジュールは、プラグインの形式だけを読む（決定 0247）
    #[cfg(all(feature = "plugin", not(feature = "drawing")))]
    let doc = read_plugin(bytes, cx)?;
    #[cfg(any(feature = "drawing", feature = "plugin"))]
    if let Some(doc) = doc {
        let result = OpenResult {
            doc: name.to_string(),
            format: doc.format().to_string(),
            needs: None,
            done: None,
            entries: match &doc {
                #[cfg(feature = "drawing")]
                DrawingFile::Sxf(_, Some(archive), _) => Some(archive.names()),
                _ => None,
            },
        };
        let hash = match &doc {
            // 元の文書のハッシュ。書いた側が付けたもので、ここでは確かめられない
            DrawingFile::Lifted(source, _) => source.hash,
            _ => Hash::document_checked(bytes, cx).map_err(|stop| stopped(cx, stop))?,
        };
        return Ok(Opened {
            result,
            document: Some(ReadDocument::Drawing {
                file: Box::new(doc),
                hash,
            }),
        });
    }
    // J-LandXML は XML の根で見分ける（決定 0230）。図面・土木地理・点群のどのモジュールも読む（決定 0253）
    #[cfg(any(feature = "drawing", feature = "civil", feature = "pointcloud"))]
    if let Some(model) = civil::read(bytes, cx) {
        return Ok(Opened::document(
            name,
            zumen_landxml::LandXml::FORMAT,
            ReadDocument::Civil(Box::new(model?)),
        ));
    }
    // CityGML と基盤地図情報も XML の根で見分ける（決定 0233）
    #[cfg(feature = "civil")]
    if let Some((format, model)) = geo::read(bytes, cx) {
        return Ok(Opened::document(
            name,
            format,
            ReadDocument::Geo(Box::new(model?)),
        ));
    }
    #[cfg(feature = "bim")]
    if let Some(model) = building::read_zip(bytes, cx) {
        return Opened::building(name, model?, cx);
    }
    // コアのどの形式でもなければ、登録した建物の形式のプラグインを順に試す
    #[cfg(feature = "bim")]
    if let Some(model) = building::read_plugin(bytes, cx) {
        return Opened::building(name, model?, cx);
    }
    cx.check(Kind::InputBytes, bytes.len() as u64)
        .map_err(|stop| stopped(cx, stop))?;
    Err(fail(
        cx,
        Diagnostic::new(codes::CALL_FORMAT_UNSUPPORTED).with("name", request.input.as_str()),
    ))
}

/// SXF の文書に、属性ファイル（SAF）を読んで持たせる（決定 0249）。名指した入力がなければ、P2Z・SFZ の中の、図面と
/// 同じ名前の SAF を使う。SAF が読めなければ、診断を返して、属性ファイルなしで開く。SXF でない文書に名指せば失敗する
#[cfg(feature = "drawing")]
fn with_attributes(
    doc: Option<DrawingFile>,
    saf: Option<&str>,
    inputs: &BTreeMap<String, Vec<u8>>,
    cx: &mut Cx<'_>,
) -> Result<Option<DrawingFile>, Failed> {
    let (file, archive) = match doc {
        Some(DrawingFile::Sxf(file, archive, _)) => (file, archive),
        other if saf.is_none() => return Ok(other),
        _ => return Err(fail(cx, invalid_value("/saf"))),
    };
    let bytes = match saf {
        Some(name) => {
            if !name::is_safe(name) {
                return Err(fail(cx, invalid_value("/saf")));
            }
            let Some(bytes) = inputs.get(name) else {
                return Err(fail(cx, input_missing(name)));
            };
            Some(bytes.as_slice())
        }
        None => archive.as_deref().and_then(Packed::attribute_file),
    };
    let attributes = match bytes.map(|bytes| zumen_sxf::saf::read(bytes, cx)) {
        None => None,
        Some(Ok(file)) => Some(Box::new(file)),
        // 失敗の理由は、読み取りが診断として足してある
        Some(Err(zumen_sdk::Error::Stopped(_))) => return Err(Failed),
        Some(Err(_)) => None,
    };
    Ok(Some(DrawingFile::Sxf(file, archive, attributes)))
}

/// 図面の形式なら読む。どの図面の形式でもなければ `None`。形式は、コーデックが持つ判定の規則で決める
/// （決定 0007・0008）。入力の大きさは、コーデックが確かめる
#[cfg(feature = "drawing")]
fn read_drawing(bytes: &[u8], cx: &mut Cx<'_>) -> Result<Option<DrawingFile>, Failed> {
    // 図面モデルのバイナリ（決定 0247）。始まりの 4 バイトは、コアのどの形式とも重ならない
    if binary::matches(bytes) {
        let (source, drawing) = binary::read(bytes, cx).map_err(|_| Failed)?;
        return Ok(Some(DrawingFile::Lifted(source, drawing)));
    }
    // 失敗の理由は、コーデックが診断として足してある
    let format = zumen_sdk::detect(DRAWING, bytes).map(|registration| registration.format);
    let doc = if format == Some(Dxf::FORMAT) {
        DrawingFile::Dxf(Dxf::read(bytes, cx).map_err(|_| Failed)?)
    } else if format == Some(Dwg::FORMAT) {
        // 読めるのは R2000（AC1015）だけ。ほかのバージョンは、コーデックが診断を足して失敗する
        DrawingFile::Dwg(Dwg::read(bytes, cx).map_err(|_| Failed)?)
    } else if format == Some(Sxf::FORMAT) {
        // 同じ始まりの IFC などは、コーデックがスキーマを確かめて、診断を足して失敗する
        DrawingFile::Sxf(Sxf::read(bytes, cx).map_err(|_| Failed)?, None, None)
    } else if format == Some(Jww::FORMAT) {
        DrawingFile::Jww(Jww::read(bytes, cx).map_err(|_| Failed)?)
    } else if let Some((sxf, archive)) = packed::read(bytes, cx)? {
        // P2Z・SFZ（#385）。SXF の項目のない ZIP は、ifcZIP などとして後で試す
        let file = Sxf::read(&sxf, cx).map_err(|_| Failed)?;
        DrawingFile::Sxf(file, Some(Box::new(archive)), None)
    } else if let Some(doc) = read_plugin(bytes, cx)? {
        // コアのどの形式でもなければ、登録した図面の形式のプラグインを順に試す（決定 0243）
        doc
    } else {
        return Ok(None);
    };
    Ok(Some(doc))
}

/// 登録した図面の形式のプラグインを、順に試す。どのプラグインの形式でもなければ `None`
#[cfg(any(feature = "drawing", feature = "plugin"))]
fn read_plugin(bytes: &[u8], cx: &mut Cx<'_>) -> Result<Option<DrawingFile>, Failed> {
    let Some((registration, read)) =
        plugin::installed()
            .iter()
            .find_map(|plugin| match plugin.read {
                plugin::Reader::Drawing(read) if plugin.registration.matches(bytes) => {
                    Some((plugin.registration, read))
                }
                #[allow(unreachable_patterns)]
                _ => None,
            })
    else {
        return Ok(None);
    };
    // 失敗の理由は、プラグインが診断として足してある
    Ok(Some(DrawingFile::Plugin(
        registration,
        read(bytes, cx).map_err(|_| Failed)?,
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    const LEDGER: &str = include_str!("../../../formats/ledger.toml");

    /// 台帳の `[formats.<key>]` の表の行
    fn table(key: &str) -> impl Iterator<Item = &'static str> {
        let header = format!("[formats.{key}]");
        LEDGER
            .lines()
            .skip_while(move |line| line.trim() != header)
            .skip(1)
            .take_while(|line| !line.starts_with('['))
    }

    /// 台帳の `[formats.<key>]` の `name = "…"` の値
    fn field(key: &str, name: &str) -> Option<&'static str> {
        let prefix = format!("{name} = \"");
        table(key)
            .find_map(|line| line.strip_prefix(prefix.as_str()))
            .and_then(|rest| rest.strip_suffix('"'))
    }

    /// 台帳に、読みか書きの確かめた度合いがあるか（決定 0254）。表の見出し `[formats.<key>.<direction>]` と、
    /// `[formats.<key>]` の中のインラインの表（`read = { … }`）・ドットのキー（`read.level = …`）を数える
    fn verified(key: &str, direction: &str) -> bool {
        let header = format!("[formats.{key}.{direction}]");
        let inline = format!("{direction} = {{");
        let dotted = format!("{direction}.");
        LEDGER.lines().any(|line| line.trim() == header)
            || table(key).any(|line| line.starts_with(&inline) || line.starts_with(&dotted))
    }

    #[test]
    fn registrations_match_the_ledger() {
        for registration in DRAWING.iter().chain(READ_ONLY) {
            let format = registration.format;
            assert_eq!(
                field(format, "rights"),
                Some(registration.rights.key()),
                "{format}"
            );
            assert_eq!(field(format, "place"), Some("core"), "{format}");
            // capabilities の read・write と同じ向きに、確かめた度合いがある
            assert!(
                verified(format, "read"),
                "{format}: 読みの確かめた度合いがない"
            );
            assert_eq!(
                verified(format, "write"),
                registration.write,
                "{format}: 書きの確かめた度合いが、書けるかと合わない"
            );
        }
    }
}
