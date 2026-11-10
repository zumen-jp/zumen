//! レビュー文書から、対象と起点を照合した別のシーンを作る（決定 0295）。

use std::collections::BTreeSet;

use zumen_base::{Cx, Diagnostic, Kind, Ref, Stop, codes};
use zumen_drawing::{
    Affine, Color, Drawing2D, Element, Height, HeightMeaning, Horizontal, Item, LayerId, Length,
    ModelPoint, Origin, PaperLength, Placement, Shape, SpaceId, Style, Text, Unit as DrawingUnit,
    Vertical, VisitRef, id,
};
use zumen_review::{Markup, MarkupKind, Place, Review, Target};

use crate::{Bounds, Builder, Chunking, FontSet, Options, Palette, Scene, TextItem, layout};

/// 赤線の端に置く矢じり。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReviewArrows {
    /// 置かない。
    None,
    /// 最初の端点に置く。
    Start,
    /// 最後の端点に置く。
    End,
    /// 両端に置く。
    Both,
}

/// 幾何の寸法は起点の単位、線幅だけは用紙の mm。既定値を暗黙に補わない。
#[derive(Clone, Debug, PartialEq)]
pub struct ReviewStyle {
    /// 線・面・文字の RGB。
    pub color: [u8; 3],
    /// 用紙の mm の線幅。0 はもっとも細い線。
    pub line_weight: f64,
    /// 起点の単位での文字枠の高さ。
    pub text_height: f64,
    /// 起点の単位での注記の枠の余白。
    pub text_padding: f64,
    /// 起点の単位での矢じりの長さ。
    pub arrow_length: f64,
    /// 赤線の矢じりを置く端。
    pub arrows: ReviewArrows,
    /// 雲の円弧の弦の長さの上限。起点の単位。
    pub cloud_pitch: f64,
    /// 写真・指摘の位置の円の直径。起点の単位。
    pub marker_size: f64,
    /// 渡した FontSet から引くフォントの名前。
    pub font: Option<String>,
}

/// レビューのシーンの作り方。許容差は、起点の単位で明示する。
#[derive(Clone, Debug, PartialEq)]
pub struct ReviewOptions {
    /// 図面に実在する空間・用紙・用紙のビュー。
    pub origin: Origin,
    /// 起点の単位での正の弦の誤差。
    pub tolerance: f64,
    /// 絞り込む起点の座標の範囲。なければ全体。
    pub region: Option<[f64; 4]>,
    /// 塊の分け方。
    pub chunking: Chunking,
    /// 明示する寸法と色。
    pub style: ReviewStyle,
}

/// 元のレビュー文書での種類と ID。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReviewSourceKind {
    /// レビュー文書の図形。
    Markup {
        /// 元の図形の ID。
        id: String,
    },
    /// 指摘の中の場所。
    Location {
        /// 元の指摘の ID。
        topic: String,
        /// 元の場所の ID。
        id: String,
    },
}

/// シーンの ID から元の図形・指摘の場所へ戻す対応。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReviewSource {
    /// この結果の entity_ids に使う ID。
    pub scene_id: String,
    /// 元の文書での種類と ID。
    pub source: ReviewSourceKind,
}

/// シーン・出所・起点を、同じ結果として使う（決定 0295）。
#[derive(Clone, Debug, PartialEq)]
pub struct ReviewScene {
    /// 元の図面とは別のシーン。
    pub scene: Scene,
    /// 既存の規則による空間・用紙の ID。
    pub from: String,
    /// 用紙のビューの番号。ビューでなければ None。
    pub view: Option<usize>,
    /// シーンの座標の単位。不明な空間を mm にしない。
    pub unit: DrawingUnit,
    /// シーンの ID から元の項目への対応。
    pub sources: Vec<ReviewSource>,
}

/// 詳しい理由は文脈の診断に、場所は戻り値の `Ref` にある。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReviewError {
    /// 文書・対象・起点・引数・選んだ図形が不正。
    Invalid,
    /// 予算か中断の合図で止めた。
    Stopped(Stop),
}

type Result<T> = std::result::Result<T, (ReviewError, Ref)>;

/// 開いた実文書の識別情報を `target` に渡す。レビュー文書・図面を変更しない。
pub fn build_review(
    drawing: &Drawing2D,
    review: &Review,
    target: &Target,
    options: &ReviewOptions,
    cx: &mut Cx<'_>,
) -> Result<ReviewScene> {
    build_review_with(drawing, review, target, options, &FontSet::default(), cx)
}

/// [`build_review`] と同じく作り、文字を渡したフォントの字形で置く。
pub fn build_review_with(
    drawing: &Drawing2D,
    review: &Review,
    target: &Target,
    options: &ReviewOptions,
    fonts: &FontSet<'_>,
    cx: &mut Cx<'_>,
) -> Result<ReviewScene> {
    let at = Ref::default();
    spend(cx, Kind::Work, 1, &at)?;
    check_options(options, cx)?;
    validate(review, cx)?;
    let mut found = None;
    for entry in &review.targets {
        comparison(cx, &target.id, review.targets.len(), &at)?;
        if entry.id == target.id {
            found = Some(entry);
            break;
        }
    }
    let Some(entry) = found else {
        return invalid(
            cx,
            Diagnostic::new(codes::REVIEW_TARGET_MISSING).with("target", target.id.as_str()),
            at,
        );
    };
    spend(
        cx,
        Kind::Work,
        (entry.format.len() + entry.hash.len() + target.format.len() + target.hash.len()) as u64
            + 1,
        &at,
    )?;
    if entry.format != target.format || entry.hash != target.hash {
        return invalid(
            cx,
            Diagnostic::new(codes::REVIEW_TARGET_STALE).with("target", target.id.as_str()),
            at,
        );
    }
    let resolved = resolve(drawing, options.origin, cx)?;
    // 描画の文脈だけを使う。元の図面の要素・スタイル・ID は複製しない。
    let context = Drawing2D::default();
    let ordinary = Options {
        origin: Origin::Space(SpaceId(0)),
        tolerance: Some(options.tolerance),
        print: false,
        region: options.region,
        palette: Palette::default(),
        chunking: options.chunking,
    };
    // 空でも書くセクション表・列の Vec と最終のヘッダーを含める。
    spend(cx, Kind::DecodedBytes, 8192, &at)?;
    let mut emit = Emitter {
        builder: Builder::new(&context, options.tolerance, &ordinary, fonts, cx),
        style: &options.style,
        sources: Vec::new(),
    };
    for markup in &review.markups {
        comparison(emit.builder.cx, &markup.target, 1, &at)?;
        if markup.target != target.id {
            continue;
        }
        let scene_id = scene_id("markup", &[&markup.id], emit.builder.cx)?;
        let source = Ref::default().with_id(scene_id.clone());
        if !selected(
            markup.view.as_deref(),
            &resolved.key,
            &source,
            emit.builder.cx,
        )? {
            continue;
        }
        emit.legacy(&markup.extra, &source)?;
        emit.markup(markup, &scene_id, &source)?;
        emit.add_source(
            scene_id,
            ReviewSourceKind::Markup {
                id: markup.id.clone(),
            },
            &source,
        )?;
    }
    for topic in &review.topics {
        for location in &topic.locations {
            comparison(emit.builder.cx, &location.target, 1, &at)?;
            if location.target != target.id {
                continue;
            }
            let scene_id = scene_id("location", &[&topic.id, &location.id], emit.builder.cx)?;
            let source = Ref::default().with_id(scene_id.clone());
            let Place::Plane {
                view, pin, extent, ..
            } = &location.place
            else {
                dropped(emit.builder.cx, "three_dimensional", &source);
                continue;
            };
            if !selected(view.as_deref(), &resolved.key, &source, emit.builder.cx)? {
                continue;
            }
            if pin.is_none() && extent.is_none() {
                dropped(emit.builder.cx, "geometry_missing", &source);
                continue;
            }
            emit.legacy(&location.extra, &source)?;
            if let Some(pin) = pin {
                emit.marker(*pin, &scene_id, &source)?;
            }
            if let Some(extent) = extent {
                emit.rectangle(*extent, &scene_id, &source)?;
            }
            emit.add_source(
                scene_id,
                ReviewSourceKind::Location {
                    topic: topic.id.clone(),
                    id: location.id.clone(),
                },
                &source,
            )?;
        }
    }
    emit.legacy(&review.extra, &at)?;
    // Builder が内部に持つ形と、それを書き出す列・最終バッファーの複製も、生成量から先に予約している。
    if emit.builder.approximated {
        emit.builder.cx.report(
            Diagnostic::new(codes::SCENE_CURVE_APPROXIMATED).with("tolerance", options.tolerance),
        );
    }
    let sources = emit.sources;
    let scene = emit.builder.finish(options.region);
    Ok(ReviewScene {
        scene,
        from: resolved.from,
        view: resolved.view,
        unit: resolved.unit,
        sources,
    })
}

fn stopped(cx: &mut Cx<'_>, stop: Stop, at: &Ref) -> (ReviewError, Ref) {
    cx.report(stop.to_diagnostic().at(at.clone()));
    (ReviewError::Stopped(stop), at.clone())
}

fn spend(cx: &mut Cx<'_>, kind: Kind, count: u64, at: &Ref) -> Result<()> {
    cx.check(kind, count).map_err(|stop| stopped(cx, stop, at))
}

fn invalid<T>(cx: &mut Cx<'_>, diagnostic: Diagnostic, at: Ref) -> Result<T> {
    cx.report(diagnostic.at(at.clone()));
    Err((ReviewError::Invalid, at))
}

fn geometry<T>(cx: &mut Cx<'_>, reason: &'static str, at: &Ref) -> Result<T> {
    invalid(
        cx,
        Diagnostic::new(codes::REVIEW_SCENE_GEOMETRY_INVALID).with("reason", reason),
        at.clone(),
    )
}

fn comparison(cx: &mut Cx<'_>, text: &str, items: usize, at: &Ref) -> Result<()> {
    let levels = u64::from(items.max(1).ilog2()) + 1;
    spend(
        cx,
        Kind::Work,
        (text.len() as u64 + 1).saturating_mul(levels),
        at,
    )
}

fn check_options(options: &ReviewOptions, cx: &mut Cx<'_>) -> Result<()> {
    let valid = |value: f64, positive: bool| {
        value.is_finite()
            && value <= f64::from(f32::MAX)
            && if positive { value > 0.0 } else { value >= 0.0 }
    };
    let style = &options.style;
    for (pointer, value, positive) in [
        ("/tolerance", options.tolerance, true),
        ("/chunking/span", options.chunking.span, true),
        ("/style/line_weight", style.line_weight, false),
        ("/style/text_height", style.text_height, true),
        ("/style/text_padding", style.text_padding, false),
        ("/style/arrow_length", style.arrow_length, true),
        ("/style/cloud_pitch", style.cloud_pitch, true),
        ("/style/marker_size", style.marker_size, true),
    ] {
        spend(cx, Kind::Work, 1, &Ref::default())?;
        if !valid(value, positive) {
            return invalid(
                cx,
                Diagnostic::new(codes::REVIEW_SCENE_OPTIONS_INVALID).with("pointer", pointer),
                Ref::default(),
            );
        }
    }
    if options.chunking.items == 0 || !valid(options.tolerance * options.chunking.span, true) {
        return invalid(
            cx,
            Diagnostic::new(codes::REVIEW_SCENE_OPTIONS_INVALID).with("pointer", "/chunking"),
            Ref::default(),
        );
    }
    if let Some([x0, y0, x1, y1]) = options.region
        && (!(x0.is_finite() && y0.is_finite() && x1.is_finite() && y1.is_finite())
            || x0 > x1
            || y0 > y1)
    {
        return invalid(
            cx,
            Diagnostic::new(codes::REVIEW_SCENE_OPTIONS_INVALID).with("pointer", "/region"),
            Ref::default(),
        );
    }
    Ok(())
}

fn add_id<'a>(set: &mut BTreeSet<&'a str>, value: &'a str, cx: &mut Cx<'_>) -> Result<()> {
    let at = Ref::default();
    comparison(cx, value, set.len(), &at)?;
    spend(cx, Kind::DecodedBytes, 192 + value.len() as u64 * 2, &at)?;
    let at = at.with_id(value);
    if value.is_empty() {
        return invalid(
            cx,
            Diagnostic::new(codes::REVIEW_FILE_INVALID).with("pointer", "/id"),
            at,
        );
    }
    if !set.insert(value) {
        return invalid(
            cx,
            Diagnostic::new(codes::REVIEW_ID_EXISTS).with("id", value),
            at,
        );
    }
    Ok(())
}

fn reference(set: &BTreeSet<&str>, value: &str, cx: &mut Cx<'_>, owner: &str) -> Result<()> {
    comparison(cx, value, set.len(), &Ref::default())?;
    if !set.contains(value) {
        spend(
            cx,
            Kind::DecodedBytes,
            128 + owner.len() as u64 * 2 + value.len() as u64,
            &Ref::default(),
        )?;
        return invalid(
            cx,
            Diagnostic::new(codes::REVIEW_REFERENCE_MISSING).with("id", value),
            Ref::default().with_id(owner),
        );
    }
    Ok(())
}

fn validate(review: &Review, cx: &mut Cx<'_>) -> Result<()> {
    let mut targets = BTreeSet::new();
    let mut topics = BTreeSet::new();
    let mut locations = BTreeSet::new();
    let mut comments = BTreeSet::new();
    let mut markups = BTreeSet::new();
    for target in &review.targets {
        add_id(&mut targets, &target.id, cx)?;
    }
    for topic in &review.topics {
        add_id(&mut topics, &topic.id, cx)?;
        for location in &topic.locations {
            add_id(&mut locations, &location.id, cx)?;
        }
        for comment in &topic.comments {
            add_id(&mut comments, &comment.id, cx)?;
        }
    }
    for markup in &review.markups {
        add_id(&mut markups, &markup.id, cx)?;
    }
    for topic in &review.topics {
        let mut local = BTreeSet::new();
        for location in &topic.locations {
            reference(&targets, &location.target, cx, &location.id)?;
            add_id(&mut local, &location.id, cx)?;
        }
        for comment in &topic.comments {
            if let Some(location) = &comment.location {
                reference(&local, location, cx, &comment.id)?;
            }
        }
        for related in &topic.related {
            reference(&topics, related, cx, &topic.id)?;
        }
    }
    for markup in &review.markups {
        reference(&targets, &markup.target, cx, &markup.id)?;
        if let Some(topic) = &markup.topic {
            reference(&topics, topic, cx, &markup.id)?;
        }
    }
    Ok(())
}

struct Resolved {
    from: String,
    key: String,
    view: Option<usize>,
    unit: DrawingUnit,
}

fn resolve(drawing: &Drawing2D, origin: Origin, cx: &mut Cx<'_>) -> Result<Resolved> {
    let at = Ref::default();
    let mut found = None;
    let mut matches = 0;
    // Namer の木・文字列・返す ID を、元の要素をたどらずに数える。
    let count =
        drawing.spaces.len() as u64 + drawing.symbols.len() as u64 + drawing.sheets.len() as u64;
    spend(
        cx,
        Kind::DecodedBytes,
        count.saturating_mul(512).saturating_add(256),
        &at,
    )?;
    let levels = u64::from(count.max(1).ilog2()) + 1;
    for space in &drawing.spaces {
        spend(cx, Kind::Work, levels * 64, &at)?;
        if origin == Origin::Space(space.id) {
            matches += 1;
            found = Some((id::space(space.id), space.unit));
        }
    }
    let mut definitions = id::DefinitionNamer::default();
    for definition in &drawing.symbols {
        spend(cx, Kind::Work, levels * 64, &at)?;
        let name = definitions.next(definition);
        if origin == Origin::Space(definition.space.id) {
            matches += 1;
            found = Some((name, definition.space.unit));
        }
    }
    let mut sheets = id::SheetNamer::default();
    let mut selected_view = None;
    for sheet in &drawing.sheets {
        spend(cx, Kind::Work, levels * 64, &at)?;
        let name = sheets.next(sheet);
        let selected = match origin {
            Origin::Sheet(selected) => selected == sheet.id,
            Origin::View {
                sheet: selected,
                view,
            } if selected == sheet.id => {
                if sheet.views.get(view).is_none() {
                    return invalid(
                        cx,
                        Diagnostic::new(codes::REVIEW_SCENE_ORIGIN_INVALID).with("origin", name),
                        at,
                    );
                }
                selected_view = Some(view);
                true
            }
            _ => false,
        };
        if selected {
            matches += 1;
            found = Some((name, DrawingUnit::Millimeter));
        }
    }
    if matches != 1 {
        return invalid(
            cx,
            Diagnostic::new(codes::REVIEW_SCENE_ORIGIN_INVALID)
                .with("origin", format!("{origin:?}")),
            at,
        );
    }
    let Some((from, unit)) = found else {
        return unreachable_origin(cx, origin);
    };
    let key = selected_view.map_or_else(|| from.clone(), |view| format!("{from}/view/{view}"));
    Ok(Resolved {
        from,
        key,
        view: selected_view,
        unit,
    })
}

fn unreachable_origin<T>(cx: &mut Cx<'_>, origin: Origin) -> Result<T> {
    invalid(
        cx,
        Diagnostic::new(codes::REVIEW_SCENE_ORIGIN_INVALID).with("origin", format!("{origin:?}")),
        Ref::default(),
    )
}

fn scene_id(kind: &str, ids: &[&str], cx: &mut Cx<'_>) -> Result<String> {
    let length = ids.iter().fold(32u64, |n, s| {
        n.saturating_add((s.len() as u64).saturating_mul(2).saturating_add(1))
    });
    spend(cx, Kind::Work, length, &Ref::default())?;
    spend(
        cx,
        Kind::DecodedBytes,
        length.saturating_mul(8),
        &Ref::default(),
    )?;
    let mut out = format!("review/{kind}");
    for id in ids {
        out.push('/');
        for byte in id.bytes() {
            for digit in [byte >> 4, byte & 15] {
                out.push(char::from(if digit < 10 {
                    b'0' + digit
                } else {
                    b'a' + digit - 10
                }));
            }
        }
    }
    Ok(out)
}

fn dropped(cx: &mut Cx<'_>, reason: &'static str, at: &Ref) {
    cx.report(
        Diagnostic::new(codes::REVIEW_SCENE_PLACEMENT_DROPPED)
            .with("reason", reason)
            .at(at.clone()),
    );
}

fn selected(view: Option<&str>, wanted: &str, at: &Ref, cx: &mut Cx<'_>) -> Result<bool> {
    let Some(view) = view else {
        dropped(cx, "view_missing", at);
        return Ok(false);
    };
    comparison(cx, view, 1, at)?;
    Ok(view == wanted)
}

struct Emitter<'d, 'c, 'x> {
    builder: Builder<'d, 'c, 'x>,
    style: &'d ReviewStyle,
    sources: Vec<ReviewSource>,
}

impl Emitter<'_, '_, '_> {
    fn add_source(&mut self, scene_id: String, source: ReviewSourceKind, at: &Ref) -> Result<()> {
        spend(
            self.builder.cx,
            Kind::DecodedBytes,
            512 + scene_id.len() as u64 * 8,
            at,
        )?;
        self.sources.push(ReviewSource { scene_id, source });
        Ok(())
    }

    fn legacy(&mut self, extra: &zumen_review::Extra, at: &Ref) -> Result<()> {
        for (key, _) in extra {
            comparison(self.builder.cx, key, 1, at)?;
            if key.starts_with("legacy.") || key == "legacy_item_order" {
                spend(
                    self.builder.cx,
                    Kind::DecodedBytes,
                    192 + key.len() as u64 * 2,
                    at,
                )?;
                self.builder.cx.report(
                    Diagnostic::new(codes::REVIEW_SCENE_PROPERTY_DROPPED)
                        .with("property", key.as_str())
                        .at(at.clone()),
                );
            }
        }
        Ok(())
    }

    fn reserve(&mut self, points: u64, at: &Ref) -> Result<()> {
        spend(self.builder.cx, Kind::Segments, points, at)?;
        spend(
            self.builder.cx,
            Kind::Work,
            points.saturating_mul(64).saturating_add(1),
            at,
        )?;
        // 入力の点、Builder の保持、列と最終バッファー、Vec の容量の増加・索引を合わせた上界。
        spend(
            self.builder.cx,
            Kind::DecodedBytes,
            points.saturating_mul(256).saturating_add(1024),
            at,
        )
    }

    fn unit(&mut self, scene_id: &str, kind: u8, at: &Ref) -> Result<u32> {
        spend(self.builder.cx, Kind::Elements, 1, at)?;
        comparison(
            self.builder.cx,
            scene_id,
            self.builder.strings.seen.len(),
            at,
        )?;
        spend(
            self.builder.cx,
            Kind::DecodedBytes,
            1024 + (scene_id.len() as u64).saturating_mul(4),
            at,
        )?;
        let element = dummy(at);
        let visit = visit(&element, scene_id, self.style);
        self.builder
            .unit(&visit, kind, &mut None)
            .map_err(|(stop, at)| stopped(self.builder.cx, stop, &at))
    }

    fn check_points(&mut self, points: &[[f64; 2]], at: &Ref) -> Result<Bounds> {
        let mut bounds = Bounds::default();
        for &point in points {
            spend(self.builder.cx, Kind::Work, 1, at)?;
            if point.iter().any(|v| !v.is_finite()) {
                return geometry(self.builder.cx, "non_finite", at);
            }
            bounds.add(point);
        }
        if let Some([x0, y0, x1, y1]) = bounds.get()
            && (!(x1 - x0).is_finite()
                || !(y1 - y0).is_finite()
                || x1 - x0 > f64::from(f32::MAX)
                || y1 - y0 > f64::from(f32::MAX))
        {
            return geometry(self.builder.cx, "coordinate_range", at);
        }
        Ok(bounds)
    }

    fn line(&mut self, points: &[[f64; 2]], closed: bool, scene_id: &str, at: &Ref) -> Result<()> {
        self.check_points(points, at)?;
        self.reserve(points.len() as u64 + u64::from(closed), at)?;
        let mut copied = points.to_vec();
        if closed && let Some(first) = points.first() {
            copied.push(*first);
        }
        let unit = self.unit(scene_id, layout::entity_kind::POLYLINE, at)?;
        self.builder.line(copied, unit);
        Ok(())
    }

    fn rectangle(&mut self, [x0, y0, x1, y1]: [f64; 4], scene_id: &str, at: &Ref) -> Result<()> {
        if x0 > x1 || y0 > y1 {
            return geometry(self.builder.cx, "extent_order", at);
        }
        self.line(
            &[[x0, y0], [x1, y0], [x1, y1], [x0, y1]],
            true,
            scene_id,
            at,
        )
    }

    fn arrow(&mut self, tip: [f64; 2], from: [f64; 2], scene_id: &str, at: &Ref) -> Result<()> {
        let length = libm::hypot(tip[0] - from[0], tip[1] - from[1]);
        if !(length.is_finite() && length > 0.0) {
            return geometry(self.builder.cx, "zero_length", at);
        }
        let d = [(tip[0] - from[0]) / length, (tip[1] - from[1]) / length];
        let size = self.style.arrow_length;
        let back = [tip[0] - d[0] * size, tip[1] - d[1] * size];
        let n = [-d[1] * size / 6.0, d[0] * size / 6.0];
        let points = [
            tip,
            [back[0] + n[0], back[1] + n[1]],
            [back[0] - n[0], back[1] - n[1]],
        ];
        self.check_points(&points, at)?;
        self.reserve(3, at)?;
        let unit = self.unit(scene_id, layout::entity_kind::POLYLINE, at)?;
        self.builder.face(vec![points.to_vec()], unit);
        Ok(())
    }

    fn arc(
        &mut self,
        center: [f64; 2],
        radius: f64,
        start: f64,
        sweep: f64,
        scene_id: &str,
        at: &Ref,
    ) -> Result<()> {
        if !(radius.is_finite() && radius > 0.0) {
            return geometry(self.builder.cx, "radius", at);
        }
        self.check_points(
            &[
                [center[0] - radius, center[1] - radius],
                [center[0] + radius, center[1] + radius],
            ],
            at,
        )?;
        // 既存の分割と同じ点数を、固定長の 2 頂点の膨らみで求めて、確保前に予約する。
        // 半円・一周を 90 度ずつに分けるので、終わりまでの数は整数で決まる。
        let pieces = if sweep.abs() > 180.0 { 4 } else { 2 };
        let step = sweep / f64::from(pieces);
        for i in 0..pieces {
            let angle = start + step * f64::from(i);
            let radians = |degrees: f64| degrees * std::f64::consts::PI / 180.0;
            let point = |degrees| {
                [
                    center[0] + radius * libm::cos(radians(degrees)),
                    center[1] + radius * libm::sin(radians(degrees)),
                ]
            };
            let vertices = [
                (point(angle), libm::tan(radians(step) / 4.0)),
                (point(angle + step), 0.0),
            ];
            let count = zumen_geom2d::polyline_points(&vertices, false, self.builder.tolerance);
            self.reserve(count, at)?;
            let points = zumen_geom2d::polyline(&vertices, false, self.builder.tolerance);
            self.check_points(&points, at)?;
            let unit = self.unit(scene_id, layout::entity_kind::CURVE, at)?;
            self.builder.line(points, unit);
        }
        self.builder.approximated = true;
        Ok(())
    }

    fn marker(&mut self, point: [f64; 2], scene_id: &str, at: &Ref) -> Result<()> {
        self.check_points(&[point], at)?;
        let radius = self.style.marker_size / 2.0;
        self.arc(point, radius, 0.0, 360.0, scene_id, at)?;
        self.line(
            &[[point[0] - radius, point[1]], [point[0] + radius, point[1]]],
            false,
            scene_id,
            at,
        )?;
        self.line(
            &[[point[0], point[1] - radius], [point[0], point[1] + radius]],
            false,
            scene_id,
            at,
        )
    }

    fn markup(&mut self, markup: &Markup, scene_id: &str, at: &Ref) -> Result<()> {
        match markup.kind {
            MarkupKind::Redline => {
                if markup.points.len() < 2 {
                    return geometry(self.builder.cx, "points_missing", at);
                }
                self.line(&markup.points, false, scene_id, at)?;
                let first = markup.points.first().copied().unwrap_or([0.0; 2]);
                let last = markup.points.last().copied().unwrap_or([0.0; 2]);
                let mut start = None;
                let mut end = None;
                for pair in markup.points.windows(2) {
                    spend(self.builder.cx, Kind::Work, 1, at)?;
                    if let [a, b] = pair
                        && a != b
                    {
                        start.get_or_insert(*b);
                        end = Some(*a);
                    }
                }
                let (Some(start), Some(end)) = (start, end) else {
                    return geometry(self.builder.cx, "zero_length", at);
                };
                if matches!(self.style.arrows, ReviewArrows::Start | ReviewArrows::Both) {
                    self.arrow(first, start, scene_id, at)?;
                }
                if matches!(self.style.arrows, ReviewArrows::End | ReviewArrows::Both) {
                    self.arrow(last, end, scene_id, at)?;
                }
                self.caption(markup, scene_id, at)?;
            }
            MarkupKind::Cloud => {
                self.cloud(&markup.points, scene_id, at)?;
                self.caption(markup, scene_id, at)?;
            }
            MarkupKind::Text | MarkupKind::Photo => {
                let Some(&point) = markup.points.first() else {
                    return geometry(self.builder.cx, "points_missing", at);
                };
                if markup.points.len() > 1 {
                    self.builder.cx.report(
                        Diagnostic::new(codes::REVIEW_SCENE_POINTS_DROPPED)
                            .with("points", (markup.points.len() - 1) as u64)
                            .at(at.clone()),
                    );
                }
                if markup.kind == MarkupKind::Photo {
                    self.marker(point, scene_id, at)?;
                } else {
                    let Some(text) = markup.text.as_deref().filter(|t| !t.is_empty()) else {
                        return geometry(self.builder.cx, "text_missing", at);
                    };
                    self.text(text, point, true, scene_id, at)?;
                }
            }
        }
        Ok(())
    }

    fn caption(&mut self, markup: &Markup, scene_id: &str, at: &Ref) -> Result<()> {
        if let Some(text) = markup.text.as_deref().filter(|s| !s.is_empty()) {
            let bounds = self.check_points(&markup.points, at)?;
            if let Some([x0, _, _, y1]) = bounds.get() {
                self.text(
                    text,
                    [x0, y1 + self.style.text_padding + self.style.text_height],
                    false,
                    scene_id,
                    at,
                )?;
            }
        }
        Ok(())
    }

    fn cloud(&mut self, points: &[[f64; 2]], scene_id: &str, at: &Ref) -> Result<()> {
        self.check_points(points, at)?;
        let points = if points.len() > 1 && points.first() == points.last() {
            points.get(..points.len() - 1).unwrap_or_default()
        } else {
            points
        };
        if points.len() < 3 {
            return geometry(self.builder.cx, "points_missing", at);
        }
        let base = points.first().copied().unwrap_or([0.0; 2]);
        let mut area = 0.0;
        let mut total = 0u64;
        for i in 0..points.len() {
            spend(self.builder.cx, Kind::Work, 1, at)?;
            let (Some(a), Some(b)) = (points.get(i), points.get((i + 1) % points.len())) else {
                continue;
            };
            area += (a[0] - base[0]) * (b[1] - base[1]) - (b[0] - base[0]) * (a[1] - base[1]);
            let count = libm::ceil(libm::hypot(b[0] - a[0], b[1] - a[1]) / self.style.cloud_pitch);
            if !count.is_finite() || count >= u64::MAX as f64 {
                return geometry(self.builder.cx, "cloud_count", at);
            }
            total = total.saturating_add(count as u64);
        }
        if !(area.is_finite() && area != 0.0) {
            return geometry(self.builder.cx, "cloud_area", at);
        }
        // 膨らみを置く前に円弧数を数える。細かい刻みでも、生成を始める前に止まる。
        spend(self.builder.cx, Kind::Work, total.saturating_mul(64), at)?;
        spend(self.builder.cx, Kind::Elements, total, at)?;
        for i in 0..points.len() {
            let (Some(a), Some(b)) = (points.get(i), points.get((i + 1) % points.len())) else {
                continue;
            };
            let distance = libm::hypot(b[0] - a[0], b[1] - a[1]);
            if distance == 0.0 {
                continue;
            }
            let count = libm::ceil(distance / self.style.cloud_pitch) as u64;
            for part in 0..count {
                let t0 = part as f64 / count as f64;
                let t1 = (part + 1) as f64 / count as f64;
                let p = [a[0] + (b[0] - a[0]) * t0, a[1] + (b[1] - a[1]) * t0];
                let q = [a[0] + (b[0] - a[0]) * t1, a[1] + (b[1] - a[1]) * t1];
                let center = [p[0] + (q[0] - p[0]) / 2.0, p[1] + (q[1] - p[1]) / 2.0];
                let start = libm::atan2(p[1] - center[1], p[0] - center[0]).to_degrees();
                self.arc(
                    center,
                    distance / count as f64 / 2.0,
                    start,
                    if area > 0.0 { 180.0 } else { -180.0 },
                    scene_id,
                    at,
                )?;
            }
        }
        Ok(())
    }

    fn text(
        &mut self,
        content: &str,
        origin: [f64; 2],
        frame: bool,
        scene_id: &str,
        at: &Ref,
    ) -> Result<()> {
        self.check_points(&[origin], at)?;
        spend(self.builder.cx, Kind::Work, content.len() as u64 + 1, at)?;
        spend(
            self.builder.cx,
            Kind::DecodedBytes,
            (content.len() as u64)
                .saturating_mul(256)
                .saturating_add(1024),
            at,
        )?;
        let mut bounds = Bounds::default();
        let mut approximated = false;
        for (index, line) in content.split('\n').enumerate() {
            let line = line.strip_suffix('\r').unwrap_or(line);
            let point = [
                origin[0],
                origin[1] - index as f64 * self.style.text_height * 1.2,
            ];
            self.check_points(&[point], at)?;
            let count = line.chars().count();
            if count == 0 {
                bounds.add(point);
                bounds.add([point[0], point[1] - self.style.text_height]);
                continue;
            }
            let element = dummy(at);
            let visit = visit(&element, scene_id, self.style);
            let text = Text {
                content: line.to_owned(),
                height: Height {
                    value: Length::Paper(PaperLength(self.style.text_height)),
                    meaning: HeightMeaning::Frame,
                },
                placement: Placement::Anchor {
                    point: ModelPoint {
                        space: SpaceId(0),
                        x: point[0],
                        y: point[1],
                    },
                    horizontal: Horizontal::Left,
                    vertical: Vertical::Top,
                },
                rotation: 0.0,
                width_factor: 1.0,
                oblique: 0.0,
                backward: false,
                upside_down: false,
                style: String::new(),
            };
            let unit = self.unit(scene_id, layout::entity_kind::TEXT, at)?;
            let string = self.builder.strings.add(line);
            if let Some(name) = self.style.font.as_deref().filter(|name| !name.is_empty()) {
                // font は既存の行でも検索の前に名前を複製するので、行ごとに数える。
                // 表の二つの名前、文字列表と出力、その Vec の伸長を 8 倍に含める。
                spend(
                    self.builder.cx,
                    Kind::DecodedBytes,
                    (name.len() as u64).saturating_mul(8).saturating_add(1024),
                    at,
                )?;
                comparison(self.builder.cx, name, self.builder.font_rows.len(), at)?;
                // 最後の文字列表の検索・追加も、64 段 × 32 比較で先払いする。
                // 32/64 bit で同じ量とし、木の各節点での比較も含める。
                spend(
                    self.builder.cx,
                    Kind::Work,
                    (name.len() as u64 + 1).saturating_mul(2048),
                    at,
                )?;
            }
            let font = self.builder.font(self.style.font.as_deref());
            let mut item = TextItem {
                string,
                at: point,
                second: None,
                height: self.style.text_height,
                width: 1.0,
                rotation: 0.0,
                oblique: 0.0,
                box_width: 0.0,
                align: [layout::horizontal::LEFT, layout::vertical::TOP],
                flags: layout::text_flag::HEIGHT_FRAME,
                line: 0,
                font,
                unit,
                bounds: Bounds::default(),
                glyphs: Vec::new(),
            };
            self.builder
                .lay_review_text(&visit, &text, self.style.font.as_deref(), &mut item)
                .map_err(|(stop, at)| stopped(self.builder.cx, stop, &at))?;
            let mut line_bounds = Bounds::default();
            for glyph in &item.glyphs {
                spend(self.builder.cx, Kind::Work, 1, at)?;
                if let Some(rings) = self.builder.shapes.get(glyph.shape as usize) {
                    for ring in rings {
                        for &[x, y] in ring {
                            spend(self.builder.cx, Kind::Work, 1, at)?;
                            let [a, b, c, d, e, f] = glyph.transform;
                            let point = [a * x + c * y + e, b * x + d * y + f];
                            if point.iter().any(|v| !v.is_finite()) {
                                return geometry(self.builder.cx, "text_bounds", at);
                            }
                            line_bounds.add(point);
                        }
                    }
                }
            }
            if line_bounds.get().is_none() {
                approximated = true;
                let end = [
                    point[0] + self.style.text_height * count as f64,
                    point[1] - self.style.text_height,
                ];
                self.check_points(&[point, end], at)?;
                line_bounds.add(point);
                line_bounds.add(end);
            }
            bounds = bounds.union(line_bounds);
            self.builder.put_text(item);
        }
        if frame {
            if approximated {
                self.builder.cx.report(
                    Diagnostic::new(codes::REVIEW_SCENE_TEXT_FRAME_APPROXIMATED).at(at.clone()),
                );
            }
            if let Some([x0, y0, x1, y1]) = bounds.get() {
                let pad = self.style.text_padding;
                self.rectangle([x0 - pad, y0 - pad, x1 + pad, y1 + pad], scene_id, at)?;
            }
        }
        Ok(())
    }
}

fn dummy(at: &Ref) -> Element<ModelPoint> {
    let point = ModelPoint {
        space: SpaceId(0),
        x: 0.0,
        y: 0.0,
    };
    Element {
        source: at.clone(),
        layer: LayerId(0),
        style: Style::BLOCK,
        shape: Shape::Line {
            start: point,
            end: point,
        },
    }
}

fn visit<'a, 'v>(
    element: &'a Element<ModelPoint>,
    scene_id: &'v str,
    style: &ReviewStyle,
) -> VisitRef<'a, 'v> {
    VisitRef {
        item: Item::Model(element),
        id: scene_id,
        path: &[],
        transform: Affine {
            m: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
        },
        color: Some(Color {
            rgb: Some(style.color),
            code: None,
        }),
        line_type: None,
        line_weight: Some(PaperLength(style.line_weight)),
        layer: LayerId(0),
        layer_definition: None,
        visible: true,
        printable: true,
        clips: &[],
    }
}
