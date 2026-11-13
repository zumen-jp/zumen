//! セッションの図面・地理モデルの変換と事前の見積もり。

use super::*;
#[cfg(feature = "drawing")]
use zumen_acad::Dxf;

impl Session {
    /// 開いた文書を別の形式のファイルに変換し、新しい文書として開く（決定 0206・0281・#876・#374）
    #[cfg(any(feature = "drawing", feature = "civil"))]
    pub(super) fn convert(
        &mut self,
        request: &ConvertRequest,
        cx: &mut Cx<'_>,
    ) -> Result<ConvertResult, Failed> {
        if request.format == zumen_geojson::GeoJson::FORMAT {
            #[cfg(feature = "civil")]
            {
                return self.convert_geojson(request, cx);
            }
            #[cfg(not(feature = "civil"))]
            {
                if !name::is_safe(&request.to) {
                    return Err(fail(cx, invalid_value("/to")));
                }
                self.check_new_document(&request.to, cx)?;
                return Err(fail(
                    cx,
                    Diagnostic::new(codes::CONVERT_FORMAT_UNSUPPORTED)
                        .with("format", request.format.as_str()),
                ));
            }
        }
        if request.format == zumen_sima::Sima::FORMAT {
            return self.convert_sima(request, cx);
        }
        #[cfg(feature = "drawing")]
        {
            let (converted, hash) =
                self.prepare_conversion(&request.doc, &request.format, &request.to, 0, false, cx)?;
            self.docs.insert(
                request.to.clone(),
                Document::Drawing {
                    file: Box::new(DrawingFile::Dxf(converted.file)),
                    hash,
                },
            );
            self.blobs.push(Some(Blob {
                kind: BlobKind::File,
                bytes: converted.bytes,
            }));
            Ok(ConvertResult {
                doc: request.to.clone(),
                format: Dxf::FORMAT.to_string(),
                hash: hash.to_string(),
                elements: converted.elements,
            })
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

    #[cfg(feature = "civil")]
    fn convert_geojson(
        &mut self,
        request: &ConvertRequest,
        cx: &mut Cx<'_>,
    ) -> Result<ConvertResult, Failed> {
        if !name::is_safe(&request.to) {
            return Err(fail(cx, invalid_value("/to")));
        }
        if !name::is_safe(&request.doc) {
            return Err(fail(cx, invalid_value("/doc")));
        }
        self.check_new_document(&request.to, cx)?;
        let doc_ref = self.document(&request.doc, cx)?;
        let lowered = match doc_ref {
            Document::Geo(model) => in_input_order(cx, |cx| {
                zumen_geojson::lower_to_geojson(model, cx).map_err(|_| Failed)
            })?,
            #[cfg(feature = "drawing")]
            Document::Drawing { file, .. } if matches!(file.as_ref(), DrawingFile::Sima(_)) => {
                let DrawingFile::Sima(sima) = file.as_ref() else {
                    return Err(fail(cx, unsupported(&request.doc, "convert")));
                };
                in_input_order(cx, |cx| {
                    let geo_model: zumen_geo::GeoModel =
                        zumen_sdk::Lift::lift(sima, cx).map_err(|_| Failed)?;
                    zumen_geojson::lower_to_geojson(&geo_model, cx).map_err(|_| Failed)
                })?
            }
            #[cfg(all(feature = "civil", not(feature = "drawing")))]
            Document::Sima(sima) => in_input_order(cx, |cx| {
                let geo_model: zumen_geo::GeoModel =
                    zumen_sdk::Lift::lift(sima.as_ref(), cx).map_err(|_| Failed)?;
                zumen_geojson::lower_to_geojson(&geo_model, cx).map_err(|_| Failed)
            })?,
            _ => return Err(fail(cx, unsupported(&request.doc, "convert"))),
        };
        let reopened = {
            let mut reread_diag = zumen_base::Diagnostics::new();
            let mut sub = Cx::new(cx.budget, &mut reread_diag, cx.cancel);
            match <zumen_geojson::GeoJson as zumen_sdk::Codec>::read(&lowered.bytes, &mut sub)
                .and_then(|file| zumen_sdk::Lift::lift(&file, &mut sub))
            {
                Ok(m) => m,
                Err(zumen_sdk::Error::Stopped(stop)) => return Err(stopped(cx, stop)),
                Err(_) => return Err(Failed),
            }
        };
        let hash = Hash::document_checked(&lowered.bytes, cx).map_err(|stop| stopped(cx, stop))?;
        self.docs
            .insert(request.to.clone(), Document::Geo(Box::new(reopened)));
        self.blobs.push(Some(Blob {
            kind: BlobKind::File,
            bytes: lowered.bytes,
        }));
        Ok(ConvertResult {
            doc: request.to.clone(),
            format: zumen_geojson::GeoJson::FORMAT.to_string(),
            hash: hash.to_string(),
            elements: lowered.features,
        })
    }

    #[cfg(any(feature = "drawing", feature = "civil"))]
    fn convert_sima(
        &mut self,
        request: &ConvertRequest,
        cx: &mut Cx<'_>,
    ) -> Result<ConvertResult, Failed> {
        if !name::is_safe(&request.to) {
            return Err(fail(cx, invalid_value("/to")));
        }
        if !name::is_safe(&request.doc) {
            return Err(fail(cx, invalid_value("/doc")));
        }
        self.check_new_document(&request.to, cx)?;
        let doc_ref = self.document(&request.doc, cx)?;
        let lowered = match doc_ref {
            Document::Geo(model) => in_input_order(cx, |cx| {
                zumen_sima::lower_geo_to_sima(model, cx).map_err(|_| Failed)
            })?,
            Document::Civil(model)
            | Document::Building {
                civil: Some(model), ..
            } => in_input_order(cx, |cx| {
                zumen_sima::lower_civil_to_sima(model, cx).map_err(|_| Failed)
            })?,
            #[cfg(feature = "drawing")]
            Document::Drawing { file, .. } => in_input_order(cx, |cx| match file.as_ref() {
                DrawingFile::Sima(sima) => {
                    zumen_sima::lower_sima_to_sima(sima, cx).map_err(|_| Failed)
                }
                _ => {
                    let drawing = file.lift(cx).map_err(|_| Failed)?;
                    zumen_sima::lower_drawing_to_sima(&drawing, cx).map_err(|_| Failed)
                }
            })?,
            #[cfg(all(feature = "civil", not(feature = "drawing")))]
            Document::Sima(sima) => in_input_order(cx, |cx| {
                zumen_sima::lower_sima_to_sima(sima.as_ref(), cx).map_err(|_| Failed)
            })?,
            _ => return Err(fail(cx, unsupported(&request.doc, "convert"))),
        };
        let reopened = {
            let mut reread_diag = zumen_base::Diagnostics::new();
            let mut sub = Cx::new(cx.budget, &mut reread_diag, cx.cancel);
            match <zumen_sima::Sima as zumen_sdk::Codec>::read(&lowered.bytes, &mut sub) {
                Ok(file) => file,
                Err(zumen_sdk::Error::Stopped(stop)) => return Err(stopped(cx, stop)),
                Err(_) => return Err(Failed),
            }
        };
        let hash = Hash::document_checked(&lowered.bytes, cx).map_err(|stop| stopped(cx, stop))?;
        let elements = (lowered.points + lowered.parcels + lowered.routes) as u64;
        #[cfg(feature = "drawing")]
        self.docs.insert(
            request.to.clone(),
            Document::Drawing {
                file: Box::new(DrawingFile::Sima(reopened)),
                hash,
            },
        );
        #[cfg(all(feature = "civil", not(feature = "drawing")))]
        {
            let _ = hash;
            self.docs
                .insert(request.to.clone(), Document::Sima(Box::new(reopened)));
        }
        self.blobs.push(Some(Blob {
            kind: BlobKind::File,
            bytes: lowered.bytes,
        }));
        Ok(ConvertResult {
            doc: request.to.clone(),
            format: zumen_sima::Sima::FORMAT.to_string(),
            hash: hash.to_string(),
            elements,
        })
    }

    /// 書き手を最後まで実行し、文書・blob を登録する前に予定値を返す
    #[cfg(feature = "drawing")]
    pub(super) fn convert_estimate(
        &mut self,
        request: &messages::ConvertEstimateRequest,
        cx: &mut Cx<'_>,
    ) -> Result<messages::ConvertEstimateResult, Failed> {
        let limit = request.limit.unwrap_or(100);
        let (converted, hash) =
            self.prepare_conversion(&request.doc, &request.format, &request.to, limit, true, cx)?;
        let input_diagnostics = conversion_input_diagnostics(&converted.input_diagnostics, cx)?;
        let report = converted.report;
        Ok(messages::ConvertEstimateResult {
            estimate: messages::ConversionEstimate {
                doc: request.to.clone(),
                format: request.format.clone(),
                hash: hash.to_string(),
                elements: converted.elements,
                total: report.total,
                counts: report
                    .counts
                    .into_iter()
                    .map(|row| messages::ConversionCount {
                        kind: row.kind,
                        fidelity: conversion_fidelity(row.fidelity),
                        count: row.count,
                        output_elements: row.output_elements,
                    })
                    .collect(),
                details: report
                    .details
                    .into_iter()
                    .map(|row| messages::ConversionDetail {
                        kind: row.kind,
                        fidelity: conversion_fidelity(row.fidelity),
                        id: row.source.id().map(str::to_string),
                        reasons: row.reasons,
                        source: source_of(&row.source),
                        output_elements: row.output_elements,
                    })
                    .collect(),
                truncated: report.truncated,
                input_diagnostics: Some(input_diagnostics),
            },
        })
    }

    /// 見積もりと実変換で、名前・経路・書き手・ハッシュ計算までを共用する
    #[cfg(feature = "drawing")]
    pub(super) fn prepare_conversion(
        &mut self,
        doc: &str,
        format: &str,
        to: &str,
        limit: u64,
        collect_input: bool,
        cx: &mut Cx<'_>,
    ) -> Result<(convert::Converted, Hash), Failed> {
        if !name::is_safe(to) {
            return Err(fail(cx, invalid_value("/to")));
        }
        self.check_new_document(to, cx)?;
        if format != Dxf::FORMAT {
            return Err(fail(
                cx,
                Diagnostic::new(codes::CONVERT_FORMAT_UNSUPPORTED).with("format", format),
            ));
        }
        let converted = in_input_order(cx, |cx| {
            let (source, _) = self.drawing_file(doc, cx)?;
            if let DrawingFile::Dwg(file) = source
                && file.version() == b"AC1018"
            {
                return convert::from_r2004(file, limit, collect_input, cx);
            }
            if !collect_input {
                let drawing = source.lift(cx).map_err(|_| Failed)?;
                return convert::to_dxf_with_report(&drawing, limit, cx);
            }
            let (drawing, input_diagnostics) = cx
                .with_diagnostic_events(limit, |cx| Ok::<_, Stop>(source.lift(cx)))
                .map_err(|_| Failed)?;
            let drawing = drawing.map_err(|_| Failed)?;
            let mut converted = convert::to_dxf_with_report(&drawing, limit, cx)?;
            converted.input_diagnostics = input_diagnostics;
            Ok(converted)
        })?;
        let hash =
            Hash::document_checked(&converted.bytes, cx).map_err(|stop| stopped(cx, stop))?;
        Ok((converted, hash))
    }
}

#[cfg(feature = "drawing")]
fn conversion_fidelity(value: zumen_acad::ConversionFidelity) -> messages::ConversionFidelity {
    use zumen_acad::ConversionFidelity as Core;
    match value {
        Core::Native => messages::ConversionFidelity::Native,
        Core::Equivalent => messages::ConversionFidelity::Equivalent,
        Core::Approximated => messages::ConversionFidelity::Approximated,
        Core::Drawn => messages::ConversionFidelity::Drawn,
        Core::Dropped => messages::ConversionFidelity::Dropped,
    }
}

#[cfg(feature = "drawing")]
fn source_of(place: &Ref) -> Source {
    Source {
        record: place.record(),
        bytes: place.bytes().map(|range| ByteRange {
            start: range.start,
            end: range.end,
        }),
        handle: match place.key() {
            Some(Key::Handle(handle)) => Some(format!("{handle:X}")),
            _ => None,
        },
        instance: match place.key() {
            Some(Key::Instance(instance)) => Some(instance.to_string()),
            _ => None,
        },
    }
}

/// コアの診断を、メッセージの診断にする
#[cfg(feature = "drawing")]
fn conversion_input_diagnostics(
    events: &zumen_base::DiagnosticEvents,
    cx: &mut Cx<'_>,
) -> Result<messages::ConversionInputDiagnostics, Failed> {
    // 公開の値への複製も、文字列と場所を確保する前に固定の論理量で数える。
    for diagnostic in events.counts.iter().chain(&events.details) {
        let work = 1_u64
            .saturating_add(diagnostic.params().len() as u64)
            .saturating_add(diagnostic.places().len() as u64);
        cx.check(Kind::Work, work)
            .map_err(|stop| stopped(cx, stop))?;
        let mut bytes = 64_u64
            .saturating_add(diagnostic.code().name().len() as u64)
            .saturating_add((diagnostic.params().len() as u64).saturating_mul(48))
            .saturating_add((diagnostic.places().len() as u64).saturating_mul(128));
        if let Some(spec) = diagnostic.spec() {
            bytes = bytes
                .saturating_add(spec.spec.len() as u64)
                .saturating_add(spec.section.len() as u64);
        }
        for (name, value) in diagnostic.params() {
            bytes = bytes.saturating_add(name.len() as u64);
            bytes = bytes.saturating_add(match value {
                zumen_base::Value::Text(text) => text.len() as u64,
                zumen_base::Value::Key(key) => key.len() as u64,
                _ => 32,
            });
        }
        for place in diagnostic.places() {
            bytes = bytes.saturating_add(place.id().map_or(0, |id| id.len() as u64));
        }
        cx.check(Kind::DecodedBytes, bytes)
            .map_err(|stop| stopped(cx, stop))?;
    }
    let result = messages::ConversionInputDiagnostics {
        total: events.total,
        counts: events.counts.iter().map(message_of).collect(),
        details: events
            .details
            .iter()
            .map(|diagnostic| messages::ConversionInputDiagnostic {
                diagnostic: message_of(diagnostic),
                sources: diagnostic.places().iter().map(source_of).collect(),
            })
            .collect(),
        truncated: events.truncated,
    };
    cx.check(Kind::Work, 0).map_err(|stop| stopped(cx, stop))?;
    Ok(result)
}
