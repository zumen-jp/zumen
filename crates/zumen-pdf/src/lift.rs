//! PDF から図面モデル（L2）への移し（決定 0255）。
//!
//! ページを用紙に、表示グループ（optional content group、8.11）をレイヤにする。ページの内容は hayro-interpret で
//! 解釈し、塗った経路・字形の並び・画像を要素にする（[`crate::device`]）。文書の情報と版は、図面の拡張
//! `pdf.document` に持つ。注釈と縮尺の辞書は、まだ移さずに診断する。

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use hayro_interpret::hayro_syntax::object::{
    Array, Dict, Name, ObjectIdentifier, Stream, String as PdfString,
};
use hayro_interpret::hayro_syntax::page::Page;
use hayro_interpret::hayro_syntax::{LoadPdfError, Pdf as Document, PdfData, PdfVersion};
use hayro_interpret::kurbo::{Affine, Rect};
use hayro_interpret::{
    Context, InterpreterCache, InterpreterSettings, InterpreterWarning, RenderContent,
    interpret_page,
};
use zumen_base::{Diagnostic, Kind, Ref, Stop, codes};
use zumen_drawing::{
    Draft, Drawing2D, Field, FieldValue, Layer, LayerId, LayerState, PaperLength, Position, Sheet,
    SheetId, SheetItem,
};
use zumen_sdk::{Cx, Error, Lift};

use crate::device::{Collector, DEVICE_PER_MM, PAGE_LAYER, Styles};
use crate::page::{self, Geometry};
use crate::{PdfFile, RecordKind, strings};

/// 形式固有の情報の値の型の契約の版（決定 0115）
const SCHEMA: u32 = 1;

/// 表示グループの外の内容を置くレイヤの名前
const PAGE_LAYER_NAME: &str = "0";

impl Lift<Drawing2D> for PdfFile {
    fn lift(&self, cx: &mut Cx) -> Result<Drawing2D, Error> {
        let header = self
            .records
            .iter()
            .position(|record| record.kind == RecordKind::Header)
            .map_or_else(Ref::default, |index| record_ref(self, index));
        // 相互参照・ページの木を読む。壊れた相互参照は、オブジェクトを探し直して読む（hayro-syntax、7.5）
        let document = Document::new(PdfData::from(Arc::clone(&self.src))).map_err(|error| {
            let (code, error) = match error {
                LoadPdfError::Decryption(_) => {
                    (codes::PDF_ENCRYPTION_UNSUPPORTED, Error::Unsupported)
                }
                LoadPdfError::Invalid => (codes::PDF_DOCUMENT_INVALID, Error::Invalid),
            };
            cx.report(Diagnostic::new(code).at(header.clone()));
            error
        })?;
        let objects = objects(self);
        Lifter {
            document: &document,
            objects: &objects,
            header,
            styles: Styles::default(),
            drafts: Vec::new(),
        }
        .run(cx)
    }
}

/// L1 のレコードを指す出所。番号とバイト範囲
fn record_ref(file: &PdfFile, index: usize) -> Ref {
    let record = file.records.get(index);
    let reference = Ref::at_record(index as u64);
    match record {
        Some(record) => reference.with_bytes(record.start, record.end),
        None => reference,
    }
}

/// 間接オブジェクトの番号と世代番号 → L1 のレコードの出所。増分の更新で同じ番号が何度も現れれば、後ろのものを使う
/// （推定。相互参照が指すのは、ふつう最後に書いたもの）。オブジェクトのストリームの中のオブジェクトはない
fn objects(file: &PdfFile) -> BTreeMap<(u32, i32), Ref> {
    let mut out = BTreeMap::new();
    for (index, record) in file.records.iter().enumerate() {
        if let Some((number, generation)) = record.object {
            out.insert((number, i32::from(generation)), record_ref(file, index));
        }
    }
    out
}

/// 表示グループのオブジェクト → レイヤ
type Groups = BTreeMap<(u32, i32), LayerId>;

fn key(id: impl Into<ObjectIdentifier>) -> (u32, i32) {
    let id = id.into();
    (id.obj_number, id.gen_number)
}

fn stopped(cx: &mut Cx, stop: Stop, place: &Ref) -> Error {
    cx.report(stop.to_diagnostic().at(place.clone()));
    Error::Stopped(stop)
}

struct Lifter<'d, 'o> {
    document: &'d Document,
    /// 間接オブジェクト → L1 のレコードの出所
    objects: &'o BTreeMap<(u32, i32), Ref>,
    /// 見出しのレコード
    header: Ref,
    styles: Styles,
    /// 付く先を位置で指す拡張。要素がそろってから ID とハッシュにする（決定 0156）
    drafts: Vec<Draft>,
}

impl Lifter<'_, '_> {
    fn run(mut self, cx: &mut Cx) -> Result<Drawing2D, Error> {
        let document = self.document;
        let xref = document.xref();
        let catalog = xref.get::<Dict<'_>>(xref.root_id());
        let (layers, ocgs) = self.layers(catalog.as_ref(), cx)?;
        let pages = document.pages();
        let labels = page::labels(catalog.as_ref(), pages.len(), cx)
            .map_err(|stop| stopped(cx, stop, &self.header))?;

        // hayro-interpret が読めなかったフォントの数。警告は解釈の途中に届くので、数えてから診断にする
        let unsupported_fonts = Arc::new(AtomicU64::new(0));
        let counter = Arc::clone(&unsupported_fonts);
        let settings = InterpreterSettings {
            warning_sink: Arc::new(move |warning| {
                if matches!(warning, InterpreterWarning::UnsupportedFont) {
                    counter.fetch_add(1, Ordering::Relaxed);
                }
            }),
            // 注釈の見た目は描かない（数を診断する）
            render_content: RenderContent::PAGE,
            ..InterpreterSettings::default()
        };
        let cache = InterpreterCache::new();
        let mut sheets = Vec::with_capacity(pages.len());
        for (index, page) in pages.iter().enumerate() {
            let label = labels.get(index).cloned().flatten();
            let source = page
                .raw()
                .obj_id()
                .and_then(|id| self.objects.get(&key(id)).cloned())
                .unwrap_or_default();
            cx.check(Kind::Work, 1)
                .map_err(|stop| stopped(cx, stop, &source))?;
            let sheet = SheetId(u32::try_from(index).unwrap_or(u32::MAX));
            let geometry = Geometry::of(page);
            let elements = {
                let content = self.content_source(page).unwrap_or_else(|| source.clone());
                if let Some(stream) = page.page_stream() {
                    cx.check(Kind::DecodedBytes, stream.len() as u64)
                        .map_err(|stop| stopped(cx, stop, &content))?;
                } else if page.raw().contains_key(b"Contents") {
                    cx.report(Diagnostic::new(codes::PDF_CONTENT_INVALID).at(content.clone()));
                }
                // 用紙の 1/1024 mm を、hayro-interpret の装置の単位にする
                let device = Affine::scale(DEVICE_PER_MM) * geometry.user_space;
                let paper = Rect::new(0.0, 0.0, geometry.width, geometry.height);
                let bbox = Rect::new(
                    0.0,
                    0.0,
                    geometry.width * DEVICE_PER_MM,
                    geometry.height * DEVICE_PER_MM,
                );
                let mut context = Context::new(device, bbox, &cache, xref, settings.clone());
                let mut collector = Collector::new(
                    cx,
                    content,
                    sheet,
                    paper,
                    &ocgs,
                    self.objects,
                    &mut self.styles,
                );
                interpret_page(page, &mut context, &mut collector);
                if let Some(stop) = collector.stopped {
                    return Err(Error::Stopped(stop));
                }
                collector.elements
            };
            self.unread(page, &source, cx)?;
            let order = (0..elements.len()).map(SheetItem::Element).collect();
            sheets.push(Sheet {
                id: sheet,
                name: label,
                size: Some([PaperLength(geometry.width), PaperLength(geometry.height)]),
                elements,
                views: Vec::new(),
                order,
                source,
            });
        }
        for _ in 0..unsupported_fonts.load(Ordering::Relaxed) {
            cx.report(Diagnostic::new(codes::PDF_FONT_UNSUPPORTED));
        }
        self.document_draft(pages.len());

        let drafts = core::mem::take(&mut self.drafts);
        let mut drawing = Drawing2D {
            sheets,
            layers,
            line_types: self.styles.line_type_list,
            text_styles: self.styles.text_style_list,
            ..Drawing2D::default()
        };
        drawing
            .settle_extensions(drafts, cx)
            .map_err(|stop| stopped(cx, stop, &self.header))?;
        Ok(drawing)
    }

    /// ページの最初の内容のストリーム（`Contents`、7.7.3.3）の出所。要素の ID はこのレコードから作る（決定 0156）
    fn content_source(&self, page: &Page<'_>) -> Option<Ref> {
        let raw = page.raw();
        let first = raw.get_ref(b"Contents").or_else(|| {
            raw.get::<Array<'_>>(b"Contents")
                .and_then(|streams| streams.raw_iter().find_map(|item| item.as_obj_ref()))
        });
        first.and_then(|id| self.objects.get(&key(id)).cloned())
    }

    /// 表示グループ（8.11.2）のレイヤ。表示の状態は既定の設定（`OCProperties` の `D`、8.11.4.3）から決める。
    /// hayro-interpret は既定で隠れる表示グループの内容を塗らないので、その内容は読まずに診断する
    fn layers(
        &mut self,
        catalog: Option<&Dict<'_>>,
        cx: &mut Cx,
    ) -> Result<(Vec<Layer>, Groups), Error> {
        let mut layers = vec![Layer {
            id: PAGE_LAYER,
            name: PAGE_LAYER_NAME.to_owned(),
            state: None,
            style: None,
            follows_placement: false,
            description: None,
            source: Ref::default(),
        }];
        let mut ocgs = BTreeMap::new();
        let Some(properties) = catalog.and_then(|catalog| catalog.get::<Dict<'_>>(b"OCProperties"))
        else {
            return Ok((layers, ocgs));
        };
        let config = properties.get::<Dict<'_>>(b"D").unwrap_or_default();
        let groups = |name: &[u8]| -> BTreeSet<(u32, i32)> {
            config
                .get::<Array<'_>>(name)
                .map(|items| {
                    items
                        .raw_iter()
                        .filter_map(|item| item.as_obj_ref())
                        .map(key)
                        .collect()
                })
                .unwrap_or_default()
        };
        let base_off = config.get::<Name<'_>>(b"BaseState").as_deref() == Some(b"OFF".as_slice());
        let (on, off, locked) = (groups(b"ON"), groups(b"OFF"), groups(b"Locked"));
        let xref = self.document.xref();
        let items = properties
            .get::<Array<'_>>(b"OCGs")
            .map(|items| {
                items
                    .raw_iter()
                    .filter_map(|item| item.as_obj_ref())
                    .collect()
            })
            .unwrap_or_else(Vec::new);
        for id in items {
            let source = self.objects.get(&key(id)).cloned().unwrap_or_default();
            cx.check(Kind::Work, 1)
                .map_err(|stop| stopped(cx, stop, &source))?;
            let group = key(id);
            if ocgs.contains_key(&group) {
                continue;
            }
            let name = xref
                .get::<Dict<'_>>(id.into())
                .and_then(|dict| dict.get::<PdfString<'_>>(b"Name"))
                .map(|name| strings::text(name.as_bytes()))
                .unwrap_or_default();
            // hayro-interpret と同じ順に決める: BaseState が OFF ならすべて隠し、ON で見せ、OFF で隠す
            let hidden = off.contains(&group) || (base_off && !on.contains(&group));
            if hidden {
                cx.report(
                    Diagnostic::new(codes::PDF_OPTIONAL_CONTENT_DROPPED)
                        .with("name", name.as_str())
                        .at(source.clone()),
                );
            }
            let layer = LayerId(u32::try_from(layers.len()).unwrap_or(u32::MAX));
            layers.push(Layer {
                id: layer,
                name,
                // 印刷の可否は、表示と同じとみなす（推定。用途の辞書 Usage は読まない）
                state: Some(LayerState {
                    visible: !hidden,
                    printable: !hidden,
                    editable: !locked.contains(&group),
                    frozen: false,
                }),
                style: None,
                follows_placement: false,
                description: None,
                source,
            });
            ocgs.insert(group, layer);
        }
        Ok((layers, ocgs))
    }

    /// まだ移さないものを診断する: 注釈（12.5）、縮尺の辞書を持つビューポート（12.9）、表示グループを持つ
    /// XObject（8.11.3.3。中の要素はレイヤに分けない）
    fn unread(&self, page: &Page<'_>, source: &Ref, cx: &mut Cx) -> Result<(), Error> {
        let raw = page.raw();
        if let Some(annotations) = raw.get::<Array<'_>>(b"Annots") {
            for annotation in annotations.iter::<Dict<'_>>() {
                cx.check(Kind::Work, 1)
                    .map_err(|stop| stopped(cx, stop, source))?;
                let kind = annotation
                    .get::<Name<'_>>(b"Subtype")
                    .map(|name| String::from_utf8_lossy(name.as_ref()).into_owned())
                    .unwrap_or_default();
                cx.report(
                    Diagnostic::new(codes::PDF_ANNOTATION_UNSUPPORTED)
                        .with("kind", kind)
                        .at(source.clone()),
                );
            }
        }
        if let Some(viewports) = raw.get::<Array<'_>>(b"VP") {
            let mut measured = false;
            for viewport in viewports.iter::<Dict<'_>>() {
                cx.check(Kind::Work, 1)
                    .map_err(|stop| stopped(cx, stop, source))?;
                measured |= viewport.contains_key(b"Measure");
            }
            if measured {
                cx.report(Diagnostic::new(codes::PDF_MEASURE_UNSUPPORTED).at(source.clone()));
            }
        }
        let x_objects = &page.resources().x_objects;
        let mut grouped = false;
        for name in x_objects.keys() {
            cx.check(Kind::Work, 1)
                .map_err(|stop| stopped(cx, stop, source))?;
            grouped |= x_objects
                .get::<Stream<'_>>(name.as_ref())
                .is_some_and(|stream| stream.dict().contains_key(b"OC"));
        }
        if grouped {
            cx.report(Diagnostic::new(codes::PDF_OPTIONAL_CONTENT_APPROXIMATED).at(source.clone()));
        }
        Ok(())
    }

    /// 文書の情報の辞書（14.3.3）とページの数を、図面の拡張 `pdf.document` にする。版は拡張の版に持つ
    fn document_draft(&mut self, pages: usize) {
        let info = self.document.metadata();
        let mut fields = Vec::new();
        let texts = [
            ("title", &info.title),
            ("author", &info.author),
            ("subject", &info.subject),
            ("keywords", &info.keywords),
            ("creator", &info.creator),
            ("producer", &info.producer),
        ];
        for (name, value) in texts {
            if let Some(bytes) = value {
                fields.push(Field {
                    key: name.into(),
                    code: None,
                    value: FieldValue::Text(strings::text(bytes)),
                });
            }
        }
        let dates = [
            ("created", &info.creation_date),
            ("modified", &info.modification_date),
        ];
        for (name, value) in dates {
            if let Some(date) = value {
                fields.push(Field {
                    key: name.into(),
                    code: None,
                    value: FieldValue::Text(strings::date(date)),
                });
            }
        }
        fields.push(Field {
            key: "pages".into(),
            code: None,
            value: FieldValue::Integer(i64::try_from(pages).unwrap_or(i64::MAX)),
        });
        self.drafts.push(Draft {
            namespace: "pdf.document".into(),
            name: String::new(),
            version: Some(version_name(self.document.version()).to_owned()),
            schema: SCHEMA,
            at: Position::Document,
            fields,
            source: self.header.clone(),
        });
    }
}

/// 版の名前（見出しの `%PDF-` の後ろ。カタログの `Version` が新しければ、そちら。7.2.2）
fn version_name(version: PdfVersion) -> &'static str {
    match version {
        PdfVersion::Pdf10 => "1.0",
        PdfVersion::Pdf11 => "1.1",
        PdfVersion::Pdf12 => "1.2",
        PdfVersion::Pdf13 => "1.3",
        PdfVersion::Pdf14 => "1.4",
        PdfVersion::Pdf15 => "1.5",
        PdfVersion::Pdf16 => "1.6",
        PdfVersion::Pdf17 => "1.7",
        PdfVersion::Pdf20 => "2.0",
    }
}
