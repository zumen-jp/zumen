//! セッションの新しい IFC の作成・変換と見積もり（#395）。
//!
//! `create`・`convert`・`convert_estimate` の format が `ifc` なら、建物のモジュールのここで受ける。ほかの
//! 形式は今までの図面・土木の経路へ渡す。書き手は `zumen-ifc` の [`zumen_ifc::create`] と
//! [`zumen_ifc::convert`] で、見積もりは実変換と同じ書き手で最後まで書き、文書と blob を登録しない。

#[cfg(feature = "bim")]
use super::conversion_ops::{conversion_estimate, conversion_input_diagnostics};
use super::*;
#[cfg(feature = "bim")]
use zumen_base::{DiagnosticEvents, Stop};
#[cfg(feature = "bim")]
use zumen_building::Building;
#[cfg(feature = "bim")]
use zumen_ifc::{ConvertOptions, Header, LengthUnit, OwnerHistory, Template, Version};

/// IFC の形式の名前
#[cfg(feature = "bim")]
const IFC: &str = "ifc";

impl Session {
    /// `create` の要求を、形式ごとの経路へ渡す
    #[cfg(any(feature = "drawing", feature = "bim"))]
    pub(super) fn create_op(
        &mut self,
        request: &messages::CreateRequest,
        cx: &mut Cx<'_>,
    ) -> Result<messages::CreateResult, Failed> {
        #[cfg(feature = "bim")]
        if request.format == IFC {
            return self.create_ifc(request, cx);
        }
        if request.ifc.is_some() {
            return Err(fail(cx, invalid_value("/ifc")));
        }
        #[cfg(feature = "drawing")]
        {
            self.create(request, cx)
        }
        #[cfg(not(feature = "drawing"))]
        {
            if !name::is_safe(&request.to) {
                return Err(fail(cx, invalid_value("/to")));
            }
            self.check_new_document(&request.to, cx)?;
            Err(fail(
                cx,
                Diagnostic::new(codes::CREATE_FORMAT_UNSUPPORTED)
                    .with("format", request.format.as_str()),
            ))
        }
    }

    /// `convert` の要求を、形式ごとの経路へ渡す
    #[cfg(any(feature = "drawing", feature = "civil", feature = "bim"))]
    pub(super) fn convert_op(
        &mut self,
        request: &ConvertRequest,
        cx: &mut Cx<'_>,
    ) -> Result<ConvertResult, Failed> {
        #[cfg(feature = "bim")]
        if request.format == IFC {
            return self.convert_ifc(request, cx);
        }
        if request.ifc.is_some() {
            return Err(fail(cx, invalid_value("/ifc")));
        }
        #[cfg(any(feature = "drawing", feature = "civil"))]
        {
            self.convert(request, cx)
        }
        #[cfg(not(any(feature = "drawing", feature = "civil")))]
        {
            if !name::is_safe(&request.to) {
                return Err(fail(cx, invalid_value("/to")));
            }
            self.check_new_document(&request.to, cx)?;
            Err(fail(
                cx,
                Diagnostic::new(codes::CONVERT_FORMAT_UNSUPPORTED)
                    .with("format", request.format.as_str()),
            ))
        }
    }

    /// `convert_estimate` の要求を、形式ごとの経路へ渡す
    #[cfg(any(feature = "drawing", feature = "bim"))]
    pub(super) fn convert_estimate_op(
        &mut self,
        request: &messages::ConvertEstimateRequest,
        cx: &mut Cx<'_>,
    ) -> Result<messages::ConvertEstimateResult, Failed> {
        #[cfg(feature = "bim")]
        if request.format == IFC {
            return self.convert_estimate_ifc(request, cx);
        }
        if request.ifc.is_some() {
            return Err(fail(cx, invalid_value("/ifc")));
        }
        #[cfg(feature = "drawing")]
        {
            self.convert_estimate(request, cx)
        }
        #[cfg(not(feature = "drawing"))]
        {
            if !name::is_safe(&request.to) {
                return Err(fail(cx, invalid_value("/to")));
            }
            self.check_new_document(&request.to, cx)?;
            Err(fail(
                cx,
                Diagnostic::new(codes::CONVERT_FORMAT_UNSUPPORTED)
                    .with("format", request.format.as_str()),
            ))
        }
    }

    /// IFC のひな形を作って開く
    #[cfg(feature = "bim")]
    fn create_ifc(
        &mut self,
        request: &messages::CreateRequest,
        cx: &mut Cx<'_>,
    ) -> Result<messages::CreateResult, Failed> {
        if !name::is_safe(&request.to) {
            return Err(fail(cx, invalid_value("/to")));
        }
        self.check_new_document(&request.to, cx)?;
        let Some(output) = &request.ifc else {
            return Err(fail(cx, invalid_value("/ifc")));
        };
        let unit = match request.unit.as_ref().unwrap_or(&messages::Unit::Millimeter) {
            messages::Unit::Millimeter => LengthUnit::Millimeter,
            messages::Unit::Centimeter => LengthUnit::Centimeter,
            messages::Unit::Meter => LengthUnit::Meter,
            messages::Unit::Inch => LengthUnit::Inch,
            messages::Unit::Foot => LengthUnit::Foot,
            messages::Unit::Unknown => return Err(fail(cx, invalid_value("/unit"))),
        };
        let namespace = output
            .namespace
            .as_deref()
            .unwrap_or(request.to.as_str())
            .as_bytes();
        let template = Template {
            version: version(output.version),
            unit,
            project: zumen_ifc::derive_global_id(namespace, b"project"),
            name: None,
            header: header(output.header.as_ref()),
            owner: output.owner.as_ref().map(owner),
        };
        let bytes = in_input_order(cx, |cx| {
            zumen_ifc::create(&template, cx).map_err(|_| Failed)
        })?;
        let (model, hash) = reopened(&bytes, cx)?;
        self.docs
            .insert(request.to.clone(), Document::from_building(model, cx)?);
        self.blobs.push(Some(Blob {
            kind: BlobKind::File,
            bytes,
        }));
        Ok(messages::CreateResult {
            doc: request.to.clone(),
            format: IFC.to_owned(),
            hash: hash.to_string(),
            elements: 0,
        })
    }

    /// 建物モデルの文書を新しい IFC にして開く
    #[cfg(feature = "bim")]
    fn convert_ifc(
        &mut self,
        request: &ConvertRequest,
        cx: &mut Cx<'_>,
    ) -> Result<ConvertResult, Failed> {
        let prepared = self.prepare_ifc(
            &request.doc,
            &request.to,
            request.ifc.as_ref(),
            0,
            false,
            cx,
        )?;
        let (model, hash) = reopened(&prepared.bytes, cx)?;
        self.docs
            .insert(request.to.clone(), Document::from_building(model, cx)?);
        self.blobs.push(Some(Blob {
            kind: BlobKind::File,
            bytes: prepared.bytes,
        }));
        Ok(ConvertResult {
            doc: request.to.clone(),
            format: IFC.to_owned(),
            hash: hash.to_string(),
            elements: prepared.elements,
        })
    }

    /// 建物モデルの文書を新しい IFC にしたときの忠実度と欠落を、convert と同じ書き手で見積もる
    #[cfg(feature = "bim")]
    fn convert_estimate_ifc(
        &mut self,
        request: &messages::ConvertEstimateRequest,
        cx: &mut Cx<'_>,
    ) -> Result<messages::ConvertEstimateResult, Failed> {
        let limit = request.limit.unwrap_or(100);
        let prepared = self.prepare_ifc(
            &request.doc,
            &request.to,
            request.ifc.as_ref(),
            limit,
            true,
            cx,
        )?;
        let hash = Hash::document_checked(&prepared.bytes, cx).map_err(|stop| stopped(cx, stop))?;
        let input_diagnostics = conversion_input_diagnostics(&prepared.input_diagnostics, cx)?;
        Ok(messages::ConvertEstimateResult {
            estimate: conversion_estimate(
                &request.to,
                IFC,
                hash,
                prepared.elements,
                prepared.report,
                Some(input_diagnostics),
            ),
        })
    }

    /// 見積もりと実変換で、名前・設定の検査と形を含む lift と書き手までを共用する
    #[cfg(feature = "bim")]
    fn prepare_ifc(
        &self,
        doc: &str,
        to: &str,
        output: Option<&messages::IfcOutput>,
        limit: u64,
        collect_input: bool,
        cx: &mut Cx<'_>,
    ) -> Result<PreparedIfc, Failed> {
        if !name::is_safe(to) {
            return Err(fail(cx, invalid_value("/to")));
        }
        self.check_new_document(to, cx)?;
        let model = self.model(doc, "convert", cx)?;
        let Some(output) = output else {
            return Err(fail(cx, invalid_value("/ifc")));
        };
        let options = ConvertOptions {
            version: version(output.version),
            header: header(output.header.as_ref()),
            owner: output.owner.as_ref().map(owner),
            namespace: output.namespace.as_ref().map(|n| n.as_bytes().to_vec()),
            limit,
        };
        in_input_order(cx, |cx| {
            // 開いたときの建物モデルは形を持たないので、IFC は形を含めて lift し直す。ST-Bridge とプラグインの
            // 建物モデルは、開いたときのものを使う
            let lift = |cx: &mut Cx<'_>| -> Result<Option<Building>, Stop> {
                match &model.file {
                    Some(file) => match file.lift_geometry(cx) {
                        Ok(building) => Ok(Some(building)),
                        Err(zumen_sdk::Error::Stopped(stop)) => Err(stop),
                        Err(_) => Ok(None),
                    },
                    None => Ok(Some(model.building.clone())),
                }
            };
            let (building, input_diagnostics) = if collect_input {
                cx.with_diagnostic_events(limit, |cx| lift(cx).map(Ok::<_, Stop>))
                    .map_err(|_| Failed)?
            } else {
                (
                    lift(cx).map(Ok).map_err(|_| Failed)?,
                    DiagnosticEvents::default(),
                )
            };
            let Some(building) = building.map_err(|_| Failed)? else {
                return Err(Failed);
            };
            let converted =
                zumen_ifc::convert(&building, &model.hash.0, &options, cx).map_err(|_| Failed)?;
            let elements = converted
                .report
                .counts
                .iter()
                .fold(0_u64, |sum, row| sum.saturating_add(row.output_elements));
            Ok(PreparedIfc {
                bytes: converted.bytes,
                report: converted.report,
                elements,
                input_diagnostics,
            })
        })
    }
}

/// 書いた IFC と忠実度の集計
#[cfg(feature = "bim")]
struct PreparedIfc {
    bytes: Vec<u8>,
    report: zumen_sdk::conversion::ConversionReport,
    /// 書いた要素の数
    elements: u64,
    /// 形を含む lift の診断の発生記録。見積もりのときだけ集める
    input_diagnostics: DiagnosticEvents,
}

/// 書いた IFC を読み直して、新しい文書の建物モデルと、見積もりと同じ計算のハッシュにする
#[cfg(feature = "bim")]
fn reopened(bytes: &[u8], cx: &mut Cx<'_>) -> Result<(building::Model, Hash), Failed> {
    let model = building::read_spf(bytes, None, cx).ok_or(Failed)??;
    let hash = Hash::document_checked(bytes, cx).map_err(|stop| stopped(cx, stop))?;
    Ok((model, hash))
}

#[cfg(feature = "bim")]
fn version(version: messages::IfcVersion) -> Version {
    match version {
        messages::IfcVersion::Ifc2x3 => Version::Ifc2x3,
        messages::IfcVersion::Ifc4 => Version::Ifc4,
        messages::IfcVersion::Ifc4x3 => Version::Ifc4x3,
    }
}

#[cfg(feature = "bim")]
fn header(header: Option<&messages::IfcHeader>) -> Header {
    let Some(header) = header else {
        return Header::default();
    };
    let text = |value: &Option<String>| value.clone().unwrap_or_default();
    Header {
        view_definition: header.view_definition.clone(),
        name: text(&header.name),
        time_stamp: text(&header.time_stamp),
        author: header.author.clone().unwrap_or_default(),
        organization: header.organization.clone().unwrap_or_default(),
        preprocessor_version: text(&header.preprocessor_version),
        originating_system: text(&header.originating_system),
        authorization: text(&header.authorization),
    }
}

#[cfg(feature = "bim")]
fn owner(owner: &messages::IfcOwner) -> OwnerHistory {
    OwnerHistory {
        person: owner.person.clone(),
        organization: owner.organization.clone(),
        developer: owner.developer.clone(),
        application: owner.application.clone(),
        version: owner.version.clone(),
        identifier: owner.identifier.clone(),
        created: owner.created,
    }
}
