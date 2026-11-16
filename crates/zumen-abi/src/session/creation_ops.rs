//! 入力を用意せずに、空の図面を作る。

use super::*;
use crate::messages::{CreateRequest, CreateResult};
use zumen_acad::Dxf;
use zumen_drawing::{Axes, ModelSpace, SpaceId};

impl Session {
    pub(super) fn create(
        &mut self,
        request: &CreateRequest,
        cx: &mut Cx<'_>,
    ) -> Result<CreateResult, Failed> {
        if !name::is_safe(&request.to) {
            return Err(fail(cx, invalid_value("/to")));
        }
        self.check_new_document(&request.to, cx)?;
        if request.format != Dxf::FORMAT {
            return Err(fail(
                cx,
                Diagnostic::new(codes::CREATE_FORMAT_UNSUPPORTED)
                    .with("format", request.format.as_str()),
            ));
        }
        // 要素や記号を持たない固定のひな形とモデル、登録する名前を確保前に数える。
        // ひな形を読んでからの処理は、convert と同じ書き手の共有予算で数える。
        cx.check(Kind::Work, 1_u64.saturating_add(request.to.len() as u64))
            .map_err(|stop| stopped(cx, stop))?;
        cx.check(
            Kind::DecodedBytes,
            16_384_u64.saturating_add((request.to.len() as u64).saturating_mul(2)),
        )
        .map_err(|stop| stopped(cx, stop))?;
        let unit = match request.unit.as_ref().unwrap_or(&messages::Unit::Millimeter) {
            messages::Unit::Millimeter => Unit::Millimeter,
            messages::Unit::Centimeter => Unit::Centimeter,
            messages::Unit::Meter => Unit::Meter,
            messages::Unit::Inch => Unit::Inch,
            messages::Unit::Foot => Unit::Foot,
            messages::Unit::Unknown => Unit::Unknown,
        };
        let drawing = Drawing2D {
            spaces: vec![ModelSpace {
                id: SpaceId(0),
                unit,
                axes: Axes::EastNorth,
                elements: Vec::new(),
                order: Vec::new(),
            }],
            ..Drawing2D::default()
        };
        let converted = in_input_order(cx, |cx| convert::to_dxf(&drawing, cx))?;
        let hash =
            Hash::document_checked(&converted.bytes, cx).map_err(|stop| stopped(cx, stop))?;
        self.docs.insert(
            request.to.clone(),
            Document::Drawing {
                file: Box::new(converted.file),
                hash,
            },
        );
        self.blobs.push(Some(Blob {
            kind: BlobKind::File,
            bytes: converted.bytes,
        }));
        Ok(CreateResult {
            doc: request.to.clone(),
            format: Dxf::FORMAT.to_string(),
            hash: hash.to_string(),
            elements: converted.elements,
        })
    }
}
