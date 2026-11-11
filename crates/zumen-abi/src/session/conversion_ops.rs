//! セッションの図面の変換と事前の見積もり。

use super::*;
use zumen_acad::Dxf;

impl Session {
    /// 開いた図面を別の形式のファイルに変換し、新しい文書として開く（決定 0206・0281）
    #[cfg(feature = "drawing")]
    pub(super) fn convert(
        &mut self,
        request: &ConvertRequest,
        cx: &mut Cx<'_>,
    ) -> Result<ConvertResult, Failed> {
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
