//! セッションの図面の読み取り・編集・表の操作。

use super::*;
use std::borrow::Cow;
use zumen_acad::{Dwg, Dxf};
use zumen_jww::Jww;
use zumen_sdk::Lift;
use zumen_sxf::Sxf;

impl Session {
    pub(super) fn drawing(
        &self,
        request: &DrawingRequest,
        cx: &mut Cx<'_>,
    ) -> Result<DrawingResult, Failed> {
        let (file, hash) = self.drawing_file(&request.doc, cx)?;
        let drawing = file.lift(cx).map_err(|_| Failed)?;
        summary(&drawing, Some(hash.to_string()), cx)
    }

    pub(super) fn entities(
        &self,
        request: &EntitiesRequest,
        cx: &mut Cx<'_>,
    ) -> Result<EntitiesResult, Failed> {
        let fields = match &request.fields {
            None => vec![EntityField::Kind, EntityField::Layer, EntityField::Source],
            Some(fields) => {
                let mut sorted = fields.clone();
                sorted.sort();
                sorted.dedup();
                if fields.is_empty() || sorted.len() != fields.len() {
                    return Err(fail(cx, invalid_value("/fields")));
                }
                sorted
            }
        };
        let region = match request.region.as_deref() {
            None => None,
            Some(&[min_x, min_y, max_x, max_y]) if min_x <= max_x && min_y <= max_y => {
                Some(Extent {
                    min_x,
                    min_y,
                    max_x,
                    max_y,
                })
            }
            Some(_) => return Err(fail(cx, invalid_value("/region"))),
        };
        let drawing = self.lift(&request.doc, cx)?;
        let offset = request.offset.unwrap_or(0);
        let limit = request.limit.unwrap_or(ENTITIES_LIMIT);
        let mut page = Page {
            start: offset,
            end: offset.saturating_add(limit),
            fields,
            region,
            position: 0,
            items: Vec::new(),
            style_ids: None,
        };
        // 空間か用紙を指せば、その要素だけを返す。記号の定義の空間は、指したときだけ返す（決定 0177）
        let wanted = |place: &str| {
            request
                .place
                .as_deref()
                .is_none_or(|wanted| wanted == place)
        };
        for space in &drawing.spaces {
            let place = id::space(space.id);
            if wanted(&place) {
                page.collect(&drawing, &place, &space.elements, cx)?;
            }
        }
        if request.place.is_some() {
            let mut definitions = id::DefinitionNamer::default();
            for definition in &drawing.symbols {
                cx.check(Kind::Work, 1)
                    .map_err(|stop| fail(cx, stop.to_diagnostic()))?;
                let place = definitions.next(definition);
                if wanted(&place) {
                    page.collect(&drawing, &place, &definition.space.elements, cx)?;
                }
            }
        }
        let mut sheets = id::SheetNamer::default();
        for sheet in &drawing.sheets {
            cx.check(Kind::Work, 1)
                .map_err(|stop| fail(cx, stop.to_diagnostic()))?;
            let place = sheets.next(sheet);
            if wanted(&place) {
                page.collect(&drawing, &place, &sheet.elements, cx)?;
            }
        }
        let total = page.position;
        Ok(EntitiesResult {
            items: page.items,
            total,
            next_offset: (page.end < total).then_some(page.end),
        })
    }

    /// レイヤと線種の表の項目を、ID・名前・ハッシュで返す（決定 0156）
    pub(super) fn tables(
        &self,
        request: &TablesRequest,
        cx: &mut Cx<'_>,
    ) -> Result<TablesResult, Failed> {
        let drawing = self.lift(&request.doc, cx)?;
        // 線種に添える文字が指す文字スタイルの ID（決定 0166）
        let marked = drawing.line_types.iter().any(|line_type| {
            line_type
                .definition
                .as_ref()
                .is_some_and(|d| !d.marks.is_empty())
        });
        let style_ids = match marked {
            true => StyleIds::new(&drawing, cx)?,
            false => StyleIds::default(),
        };
        let mut seen = BTreeMap::new();
        let mut line_types = Vec::with_capacity(drawing.line_types.len());
        let mut line_type_ids = BTreeMap::new();
        for line_type in &drawing.line_types {
            let hash = Hash::line_type_checked(line_type, cx).map_err(|stop| stopped(cx, stop))?;
            let id = table_id(id::line_type(line_type), &mut seen);
            line_type_ids
                .entry(line_type.id)
                .or_insert_with(|| id.clone());
            line_types.push(line_type_item(id, line_type, hash, &style_ids, cx)?);
        }
        seen.clear();
        let mut layers = Vec::with_capacity(drawing.layers.len());
        for layer in &drawing.layers {
            let hash = Hash::layer_checked(layer, cx).map_err(|stop| stopped(cx, stop))?;
            layers.push(layer_item(
                table_id(id::layer(layer), &mut seen),
                layer,
                hash,
                &line_type_ids,
            ));
        }
        seen.clear();
        let mut dimension_styles = Vec::with_capacity(drawing.dimension_styles.len());
        for (index, style) in drawing.dimension_styles.iter().enumerate() {
            let hash =
                Hash::dimension_style_checked(style, cx).map_err(|stop| stopped(cx, stop))?;
            dimension_styles.push(dimension_style_item(
                table_id(id::dimension_style(style, index), &mut seen),
                style,
                hash,
            ));
        }
        seen.clear();
        let mut text_styles = Vec::with_capacity(drawing.text_styles.len());
        for (index, style) in drawing.text_styles.iter().enumerate() {
            let hash = Hash::text_style_checked(style, cx).map_err(|stop| stopped(cx, stop))?;
            text_styles.push(text_style_item(
                table_id(id::text_style(style, index), &mut seen),
                style,
                hash,
            ));
        }
        Ok(TablesResult {
            layers,
            line_types,
            dimension_styles,
            text_styles,
        })
    }

    /// 変更の一覧を文書に当てて書き戻し、書き戻したファイルを blob で返す（決定 0139）
    #[cfg(feature = "drawing")]
    pub(super) fn apply(
        &mut self,
        request: &ApplyRequest,
        cx: &mut Cx<'_>,
    ) -> Result<ApplyResult, Failed> {
        let drawing = self.lift(&request.doc, cx)?;
        // 移せなかった ATTDEF は図面モデルの定義に現れない。どの定義の属性かを API で判定できないため、
        // 安全に属性を作れない記号の追加は拒否する。
        let mut symbol_add = None;
        for (index, edit) in request.edits.iter().enumerate() {
            cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
            if matches!(edit, messages::Edit::Add(edit) if matches!(edit.geometry, messages::Geometry::Symbol(_)))
            {
                symbol_add = Some(index);
                break;
            }
        }
        if let Some(index) = symbol_add {
            let diagnostics = u64::try_from(cx.diag.len()).unwrap_or(u64::MAX);
            cx.check(Kind::Work, diagnostics)
                .map_err(|stop| stopped(cx, stop))?;
            let invalid_definitions = cx.diag.iter().any(|diagnostic| {
                matches!(diagnostic.param("kind"), Some(zumen_base::Value::Text(kind)) if kind == "ATTDEF")
            });
            if invalid_definitions {
                return Err(fail(
                    cx,
                    invalid_value(format!("/edits/{index}/geometry/attributes")),
                ));
            }
        }
        let changes = changes(request, &drawing, cx)?;
        let Some(Document::Drawing {
            file,
            hash: current_hash,
        }) = self.docs.get_mut(&request.doc)
        else {
            return Err(fail(
                cx,
                Diagnostic::new(codes::CALL_DOCUMENT_MISSING).with("name", request.doc.as_str()),
            ));
        };
        // 書き出しが予算や中断で止まっても、元の文書は変更しない。
        let mut bytes = Vec::new();
        let mut ids_shifted = false;
        let (applied, hash) = match file.as_mut() {
            // プラグインの文書・図面モデルのバイナリ・SIMA・TIFF は、apply では書き戻せない（決定 0243・0247・#374・#877）
            DrawingFile::Plugin(..)
            | DrawingFile::Lifted(..)
            | DrawingFile::Sima(..)
            | DrawingFile::Tiff(..) => {
                return Err(fail(cx, unsupported(&request.doc, self.op)));
            }
            DrawingFile::Dxf(file) => {
                let mut updated = file.snapshot(cx).map_err(|_| Failed)?;
                let applied = updated.apply(&changes, cx).map_err(|_| Failed)?;
                Dxf::write(&updated, &mut bytes, cx).map_err(|_| Failed)?;
                let hash = Hash::document_checked(&bytes, cx).map_err(|stop| stopped(cx, stop))?;
                // ハンドルを持たない DXF（R12 など）の要素の ID は、レコードの番号で決まる（決定 0156、#722）
                if keyless(&drawing, cx)? {
                    // 作り直しの診断は、書き戻しの診断と重なるので返さない。失敗したときだけ返す
                    let mut quiet = Diagnostics::new();
                    let lifted = updated.lift(&mut Cx::new(cx.budget, &mut quiet, cx.cancel));
                    let after = lifted.map_err(|error| match error {
                        zumen_sdk::Error::Stopped(stop) => stopped(cx, stop),
                        _ => {
                            for diagnostic in quiet.iter() {
                                cx.report(diagnostic.clone());
                            }
                            Failed
                        }
                    })?;
                    ids_shifted = record_ids_shifted(&drawing, &after, &changes, cx)?;
                }
                *file = updated;
                (applied, hash)
            }
            DrawingFile::Dwg(file) => {
                let mut updated = file.snapshot(cx).map_err(|_| Failed)?;
                let applied = updated.apply(&changes, cx).map_err(|_| Failed)?;
                Dwg::write(&updated, &mut bytes, cx).map_err(|_| Failed)?;
                let hash = Hash::document_checked(&bytes, cx).map_err(|stop| stopped(cx, stop))?;
                *file = updated;
                (applied, hash)
            }
            DrawingFile::Sxf(file, archive, _) => {
                let mut updated = file.clone();
                let applied = updated.apply(&changes, cx).map_err(|_| Failed)?;
                Sxf::write(&updated, &mut bytes, cx).map_err(|_| Failed)?;
                if let Some(archive) = archive {
                    bytes = archive
                        .write(&bytes)
                        .ok_or_else(|| fail(cx, Diagnostic::new(codes::SXF_ARCHIVE_UNSUPPORTED)))?;
                }
                let hash = Hash::document_checked(&bytes, cx).map_err(|stop| stopped(cx, stop))?;
                *file = updated;
                (applied, hash)
            }
            DrawingFile::Jww(file) => {
                let mut updated = file.clone();
                let (applied, shifted) = updated.apply(&changes, cx).map_err(|_| Failed)?;
                Jww::write(&updated, &mut bytes, cx).map_err(|_| Failed)?;
                let hash = Hash::document_checked(&bytes, cx).map_err(|stop| stopped(cx, stop))?;
                *file = updated;
                ids_shifted = shifted;
                (applied, hash)
            }
        };
        *current_hash = hash;
        self.blobs.push(Some(Blob {
            kind: BlobKind::File,
            bytes,
        }));
        Ok(ApplyResult {
            applied: applied
                .into_iter()
                .map(|Applied { id, hash }| messages::Applied {
                    id,
                    hash: hash.map(|hash| hash.to_string()),
                })
                .collect(),
            hash: hash.to_string(),
            ids_shifted: ids_shifted.then_some(true),
        })
    }

    /// 文書を図面モデルに変換し、図面モデルのバイナリ（決定 0247）を blob で返す。元の文書の形式・系統・ハッシュも書く。
    /// 図面モデルを持つ文書は、複製せずに書く。書くときは、要素ごとに作業量を使い、中断の合図を確かめる
    pub(super) fn lift_binary(
        &mut self,
        request: &LiftRequest,
        cx: &mut Cx<'_>,
    ) -> Result<LiftResult, Failed> {
        let bytes = {
            let (doc, hash) = self.drawing_file(&request.doc, cx)?;
            let drawing = doc.lift(cx).map_err(|_| Failed)?;
            let source = binary::Source {
                format: doc.format().to_string(),
                family: doc.family().to_string(),
                hash,
            };
            binary::write(&source, &drawing, cx).map_err(|error| match error {
                binary::WriteError::Stopped(stop) => stopped(cx, stop),
                binary::WriteError::TooLarge => {
                    fail(cx, Diagnostic::new(codes::DRAWING_BINARY_SIZE_EXCEEDED))
                }
            })?
        };
        let blob = self.blobs.len() as u64;
        self.blobs.push(Some(Blob {
            kind: BlobKind::Drawing,
            bytes,
        }));
        Ok(LiftResult { blob })
    }

    /// 名前の文書を、図面モデルに変換する
    pub(super) fn lift(&self, doc: &str, cx: &mut Cx<'_>) -> Result<Drawing2D, Failed> {
        let (file, _) = self.drawing_file(doc, cx)?;
        file.lift(cx).map(Cow::into_owned).map_err(|_| Failed)
    }

    pub(super) fn drawing_file(
        &self,
        doc: &str,
        cx: &mut Cx<'_>,
    ) -> Result<(&DrawingFile, Hash), Failed> {
        if !name::is_safe(doc) {
            return Err(fail(cx, invalid_value("/doc")));
        }
        match self.document(doc, cx)? {
            Document::Drawing { file, hash } => Ok((file, *hash)),
            _ => Err(fail(cx, unsupported(doc, self.op))),
        }
    }
}

/// 図面モデルの要約。空間と用紙は、モデルの順に並べる
fn summary(
    drawing: &Drawing2D,
    hash: Option<String>,
    cx: &mut Cx<'_>,
) -> Result<DrawingResult, Failed> {
    let mut sheet_names = id::SheetNamer::default();
    Ok(DrawingResult {
        hash,
        spaces: drawing
            .spaces
            .iter()
            .map(|space| SpaceSummary {
                id: Some(id::space(space.id)),
                unit: unit_of(space.unit),
                elements: counts(&space.elements),
                extent: extent(&space.elements, |p| (p.x, p.y)),
            })
            .collect(),
        sheets: drawing
            .sheets
            .iter()
            .map(|sheet| {
                cx.check(Kind::Work, 1)
                    .map_err(|stop| fail(cx, stop.to_diagnostic()))?;
                Ok(SheetSummary {
                    id: Some(sheet_names.next(sheet)),
                    name: sheet.name.clone(),
                    size: sheet.size.map(|size| size.iter().map(|l| l.0).collect()),
                    elements: counts(&sheet.elements),
                    views: sheet.views.len() as u64,
                    extent: extent(&sheet.elements, |p| (p.x, p.y)),
                })
            })
            .collect::<Result<_, Failed>>()?,
        point_style: drawing.point_style.map(point_style_summary),
    })
}

/// 文書の点の表し方の要約（決定 0193）
fn point_style_summary(style: PointStyle) -> PointStyleSummary {
    let (size, size_kind) = match style.size {
        PointSize::Length(size) => (size, messages::PointSizeKind::Length),
        PointSize::Screen(size) => (size, messages::PointSizeKind::Screen),
    };
    PointStyleSummary {
        mark: match style.mark {
            PointMark::Dot => messages::PointMark::Dot,
            PointMark::None => messages::PointMark::None,
            PointMark::Plus => messages::PointMark::Plus,
            PointMark::Cross => messages::PointMark::Cross,
            PointMark::Tick => messages::PointMark::Tick,
        },
        circle: style.circle,
        square: style.square,
        size,
        size_kind,
    }
}

/// entities の op で返す範囲と、そこまでに数えた要素
pub(super) struct Page {
    /// 返す最初の要素の位置
    pub(super) start: u64,
    /// 返す範囲の終わり（含まない）
    pub(super) end: u64,
    /// 返すフィールド。`id` のほかに返すもの
    pub(super) fields: Vec<EntityField>,
    /// 範囲がこの長方形と重なる要素だけを数える
    pub(super) region: Option<Extent>,
    /// 次に数える要素の位置
    pub(super) position: u64,
    pub(super) items: Vec<Entity>,
    /// 形の値に書くスタイルの ID。形の値を返すときに作る
    pub(super) style_ids: Option<StyleIds>,
}

impl Page {
    /// 空間か用紙の要素を数え、返す範囲にあるものを足す
    pub(super) fn collect<P: geometry::Spot + Encode>(
        &mut self,
        drawing: &Drawing2D,
        place: &str,
        elements: &[Element<P>],
        cx: &mut Cx<'_>,
    ) -> Result<(), Failed> {
        let mut namer = id::Namer::new(place);
        // 引出線の注記の出所から、同じ並びの要素の ID。注記を持つ引出線の形の値を返すときに作る。同じ出所の
        // 要素が複数あれば、どれか決まらないので `None`
        let mut annotations: Option<BTreeMap<&Ref, Option<String>>> = None;
        for (index, element) in elements.iter().enumerate() {
            let id = namer.next(index, element);
            let mut bbox = None;
            // 止めたら、扱っていた要素の出所を場所に入れる（決定 0119）
            let stop_at = |cx: &mut Cx<'_>, stop: Stop| {
                let mut diagnostic = stop.to_diagnostic();
                if element.source != Ref::default() {
                    diagnostic = diagnostic.at(element.source.clone());
                }
                fail(cx, diagnostic)
            };
            if let Some(region) = &self.region {
                cx.check(Kind::Work, 1).map_err(|stop| stop_at(cx, stop))?;
                bbox = bounds::bbox(&element.shape);
                if !bbox
                    .as_ref()
                    .is_some_and(|extent| bounds::overlaps(extent, region))
                {
                    continue;
                }
            }
            let position = self.position;
            self.position += 1;
            if position < self.start || position >= self.end {
                continue;
            }
            cx.check(Kind::Work, 1).map_err(|stop| stop_at(cx, stop))?;
            let has = |field| self.fields.contains(&field);
            if has(EntityField::Bbox) && self.region.is_none() {
                bbox = bounds::bbox(&element.shape);
            }
            let hash = if has(EntityField::Hash) {
                Some(
                    Hash::element_checked(element, cx)
                        .map_err(|stop| stop_at(cx, stop))?
                        .to_string(),
                )
            } else {
                None
            };
            // 形の値に複製する頂点と文字列の大きさを数える
            let geometry = if has(EntityField::Geometry) {
                let size = match &element.shape {
                    Shape::Polyline(line) => line.vertices.len(),
                    Shape::Curve(zumen_drawing::Edge::Spline(spline)) => spline
                        .control
                        .len()
                        .saturating_add(spline.knots.len())
                        .saturating_add(spline.weights.len())
                        .saturating_add(spline.fit.len()),
                    Shape::Text(text) => text.content.len().saturating_add(text.style.len()),
                    Shape::Formatted(text) => text.content.len().saturating_add(text.style.len()),
                    Shape::Symbol(symbol) => symbol.key.name.len(),
                    _ => 0,
                };
                cx.check(Kind::Work, u64::try_from(size).unwrap_or(u64::MAX))
                    .map_err(|stop| stop_at(cx, stop))?;
                if let Shape::Symbol(symbol) = &element.shape {
                    for attribute in &symbol.attributes {
                        let size = 1_usize
                            .saturating_add(attribute.tag.len())
                            .saturating_add(attribute.text.content.len());
                        cx.check(Kind::Work, u64::try_from(size).unwrap_or(u64::MAX))
                            .map_err(|stop| stop_at(cx, stop))?;
                    }
                }
                if self.style_ids.is_none() {
                    self.style_ids = Some(StyleIds::new(drawing, cx)?);
                }
                let annotation = match &element.shape {
                    Shape::Leader(Leader {
                        annotation: Some(annotation),
                        ..
                    }) => {
                        if annotations.is_none() {
                            let mut found = BTreeMap::new();
                            let mut namer = id::Namer::new(place);
                            for (index, element) in elements.iter().enumerate() {
                                cx.check(Kind::Work, 1).map_err(|stop| stop_at(cx, stop))?;
                                let id = namer.next(index, element);
                                found
                                    .entry(&element.source)
                                    .and_modify(|id: &mut Option<String>| *id = None)
                                    .or_insert(Some(id));
                            }
                            annotations = Some(found);
                        }
                        annotations
                            .as_ref()
                            .and_then(|found| found.get(annotation).cloned().flatten())
                    }
                    _ => None,
                };
                geometry::geometry_of(&element.shape).map(|mut geometry| {
                    if let Some(ids) = &self.style_ids {
                        ids.fill(&mut geometry);
                    }
                    if let messages::Geometry::Leader(leader) = &mut geometry {
                        leader.annotation = annotation;
                    }
                    geometry
                })
            } else {
                None
            };
            self.items.push(Entity {
                id,
                kind: has(EntityField::Kind).then(|| kind_of(&element.shape)),
                layer: drawing
                    .layer(element.layer)
                    .filter(|_| has(EntityField::Layer))
                    .map(|layer| layer.name.clone()),
                source: has(EntityField::Source).then(|| Source {
                    record: element.source.record(),
                    bytes: element.source.bytes().map(|bytes| ByteRange {
                        start: bytes.start,
                        end: bytes.end,
                    }),
                    handle: match element.source.key() {
                        Some(Key::Handle(handle)) => Some(format!("{handle:X}")),
                        _ => None,
                    },
                    instance: match element.source.key() {
                        Some(Key::Instance(number)) => Some(number.to_string()),
                        _ => None,
                    },
                }),
                bbox: bbox.filter(|_| has(EntityField::Bbox)),
                hash,
                geometry,
            });
        }
        Ok(())
    }
}

fn kind_of<P>(shape: &Shape<P>) -> EntityKind {
    match shape {
        Shape::Line { .. } => EntityKind::Line,
        Shape::Circle { .. } => EntityKind::Circle,
        Shape::Curve(_) => EntityKind::Curve,
        Shape::Polyline(_) => EntityKind::Polyline,
        Shape::Text(_) => EntityKind::Text,
        Shape::Formatted(_) => EntityKind::FormattedText,
        Shape::Dimension(_) => EntityKind::Dimension,
        Shape::Leader(_) => EntityKind::Leader,
        Shape::Callout(_) => EntityKind::Callout,
        Shape::Table(_) => EntityKind::Table,
        Shape::Underlay(_) => EntityKind::Underlay,
        Shape::Symbol(_) => EntityKind::Symbol,
        Shape::Hatch(_) => EntityKind::Hatch,
        Shape::Point(_) => EntityKind::Point,
    }
}

fn counts<P>(elements: &[Element<P>]) -> ElementCounts {
    // 書式つきの文字・寸法・引出線・引出線つきの注記・記号・ハッチングは、要約のフィールドがまだないので数えない（#66）
    let count = |is: fn(&Shape<P>) -> bool| elements.iter().filter(|e| is(&e.shape)).count() as u64;
    ElementCounts {
        lines: count(|shape| matches!(shape, Shape::Line { .. })),
        texts: count(|shape| matches!(shape, Shape::Text(_))),
    }
}

/// 線分の端点と文字の基点（2 点の間に置く文字は 2 点とも）を囲む範囲。要素がなければ `None`
fn extent<P: Copy>(elements: &[Element<P>], xy: impl Fn(P) -> (f64, f64)) -> Option<Extent> {
    let points = elements.iter().flat_map(|e| match &e.shape {
        Shape::Line { start, end } => [Some(*start), Some(*end)],
        Shape::Text(text) => match text.placement {
            Placement::Anchor { point, .. } => [Some(point), None],
            Placement::Between { first, second, .. } => [Some(first), Some(second)],
        },
        Shape::Circle { .. }
        | Shape::Curve(_)
        | Shape::Polyline(_)
        | Shape::Formatted(_)
        | Shape::Dimension(_)
        | Shape::Leader(_)
        | Shape::Callout(_)
        | Shape::Table(_)
        | Shape::Underlay(_)
        | Shape::Symbol(_)
        | Shape::Hatch(_)
        | Shape::Point(_) => [None, None],
    });
    points.flatten().map(xy).fold(None, |extent, (x, y)| {
        Some(match extent {
            None => Extent {
                min_x: x,
                min_y: y,
                max_x: x,
                max_y: y,
            },
            Some(e) => Extent {
                min_x: e.min_x.min(x),
                min_y: e.min_y.min(y),
                max_x: e.max_x.max(x),
                max_y: e.max_y.max(y),
            },
        })
    })
}

/// キーを持たず、レコードの番号で ID が決まる要素があるか（決定 0156）。見つけたあとは、数えずに抜ける
#[cfg(feature = "drawing")]
fn keyless(drawing: &Drawing2D, cx: &mut Cx<'_>) -> Result<bool, Failed> {
    let mut found = false;
    id::each_element(drawing, |_, _, element| {
        if found {
            return Ok(());
        }
        cx.check(Kind::Work, 1)?;
        found = numbered(element);
        Ok::<(), Stop>(())
    })
    .map_err(|stop| stopped(cx, stop))?;
    Ok(found)
}

/// キーを持たず、レコードの番号で ID が決まる要素か
#[cfg(feature = "drawing")]
fn numbered(element: id::Found<'_>) -> bool {
    let source = match element {
        id::Found::Model(element) => &element.source,
        id::Found::Paper(element) => &element.source,
    };
    source.key().is_none() && source.record().is_some()
}

/// 残した要素の ID が、apply の前と後でずれたか（決定 0156、#722）。キーを持たずレコードの番号で ID が決まる要素の
/// ID を、図面の順に並べて比べる。前の並びから除いた要素を抜いたものが、後の並びの頭と同じなら、ずれていない
/// （終わりに足した要素は、後ろに並ぶ）。前の要素を除くか足すか、置き換えでレコードの数が変わると、後ろの要素の
/// ID が変わるので、変えた要素も含めて並びが食い違う。値は比べないので、表の変更で要素の値が変わっても、ずれたとしない
#[cfg(feature = "drawing")]
fn record_ids_shifted(
    before: &Drawing2D,
    after: &Drawing2D,
    changes: &zumen_drawing::Changes,
    cx: &mut Cx<'_>,
) -> Result<bool, Failed> {
    use std::collections::BTreeSet;
    use zumen_drawing::Edit;
    let removed: BTreeSet<&str> = changes
        .edits
        .iter()
        .filter_map(|edit| match edit {
            Edit::Remove { target } => Some(target.id.as_str()),
            _ => None,
        })
        .collect();
    let kept = numbered_ids(before, &removed, cx)?;
    let now = numbered_ids(after, &BTreeSet::new(), cx)?;
    Ok(!now.starts_with(&kept))
}

/// レコードの番号で決まる要素の ID を、図面の順に並べる。`skip` の ID は抜く
#[cfg(feature = "drawing")]
fn numbered_ids(
    drawing: &Drawing2D,
    skip: &std::collections::BTreeSet<&str>,
    cx: &mut Cx<'_>,
) -> Result<Vec<String>, Failed> {
    let mut found = Vec::new();
    id::each_element(drawing, |_, id, element| {
        cx.check(Kind::Work, 1)?;
        if numbered(element) && !skip.contains(id.as_str()) {
            found.push(id);
        }
        Ok::<(), Stop>(())
    })
    .map_err(|stop| stopped(cx, stop))?;
    Ok(found)
}
