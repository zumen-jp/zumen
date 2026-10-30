//! P21 の実体を、SFC と同じ形のフィーチャのレコードに組み直す（決定 0132）。
//!
//! 表（色・線種・線幅・文字フォント・レイヤ）を番号の順に、用紙の要素を並びの順に組み直し、用紙を最後に置く。
//! 組み直したフィーチャの出どころは、元の実体のレコードにする。まだ組み直さない要素は、実体の名前を種類にして、
//! 変換に「まだ移さないもの」として渡す。

use std::collections::BTreeMap;

use zumen_base::{Diagnostic, Ref, codes};
use zumen_sdk::{Cx, Error};

use crate::drawing::{Arg, FeatureRecord, LINE_TYPES, WIDTHS, work};
use crate::{Part, SxfFile, Value};

/// 既定義色のコード 1〜16 の名前（SXF の共通既定義要素の表）
pub(crate) const COLORS: [&str; 16] = [
    "black",
    "red",
    "green",
    "blue",
    "yellow",
    "magenta",
    "cyan",
    "white",
    "deeppink",
    "brown",
    "orange",
    "lightgreen",
    "lightblue",
    "lavender",
    "lightgray",
    "darkgray",
];

/// 曲線の見え方がないときの色・線種・線幅のコード
const DEFAULT_STYLE: [i64; 3] = [8, 1, 1];

/// 複合図形の名前の接頭語と、SFC の複合図形の種類のコード。接頭語がなければ作図グループ（3）とする
const SUBFIGURES: [(&str, i64); 4] = [
    ("$$SXF_FM_", 1),
    ("$$SXF_FG_", 2),
    ("$$SXF_G_", 3),
    ("$$SXF_P_", 4),
];

/// 複合図形の定義の入れ子の上限
const MAX_DEPTH: usize = 64;

/// ハッチングのフィーチャの名前。最後の 3 つの引数が、外形と中抜きの複合曲線の番号
const FILLS: [&str; 4] = [
    "fill_area_style_colour",
    "fill_area_style_hatching",
    "fill_area_style_tiles",
    "externally_defined_hatch",
];

/// 矢印（端末記号）のコード 1〜11 の名前
const ARROWS: [&str; 11] = [
    "blanked arrow",
    "blanked box",
    "blanked dot",
    "dimension origin",
    "filled box",
    "filled arrow",
    "filled dot",
    "integral symbol",
    "open arrow",
    "slash",
    "unfilled arrow",
];

/// 名前のない端末記号の矢印のコード（open arrow）
const DEFAULT_ARROW: usize = 9;

/// 点マーカーのコード 1〜7 の名前
pub(crate) const MARKERS: [&str; 7] = [
    "asterisk", "circle", "dot", "plus", "square", "triangle", "x",
];

/// 文字の配置の行（下から）と列（左から）の名前。`$$SXF_` の後に、行と列を空白で区切って書く
const ROWS: [&str; 3] = ["baseline", "middleline", "topline"];
const COLUMNS: [&[&str]; 3] = [&["left"], &["centre", "center"], &["right"]];

/// P21 の実体を、フィーチャのレコードに組み直す
///
/// SFC の変換は、複合曲線と複合図形の定義に、その前にある要素をまとめる。複合図形の定義を先に、複合曲線を
/// その図形の並びの先に書き、ほかの要素がまとめられないようにする
pub(crate) fn records(file: &SxfFile, cx: &mut Cx) -> Result<Vec<FeatureRecord>, Error> {
    let mut builder = Builder::new(file);
    builder.check_references(cx)?;
    builder.tables(cx)?;
    let mut out = core::mem::take(&mut builder.out);
    builder.sheet(cx)?;
    out.append(&mut builder.definitions);
    out.append(&mut builder.front);
    out.append(&mut builder.out);
    number_composites(&mut out);
    Ok(out)
}

/// 表のコードを持つ実体。書き戻しが、コードから実体を引くのに使う
pub(crate) struct Codes {
    /// レイヤのコード（1 から）の順の、PRESENTATION_LAYER_ASSIGNMENT の番号
    pub(crate) layers: Vec<u64>,
    /// 色の実体の番号から、色のコード
    pub(crate) colors: BTreeMap<u64, i64>,
    /// 線種の実体の番号から、線種のコード
    pub(crate) line_types: BTreeMap<u64, i64>,
}

/// 変換と同じ順で、表のコードを数える
pub(crate) fn codes(file: &SxfFile, cx: &mut Cx) -> Result<Codes, Error> {
    let mut builder = Builder::new(file);
    builder.tables(cx)?;
    Ok(Codes {
        layers: builder.layers,
        colors: builder.colors,
        line_types: builder.line_types,
    })
}

/// ハッチングの外形と中抜きに書いた複合曲線のレコードを、複合曲線の番号（書いた順に 1 から）にする
fn number_composites(out: &mut [FeatureRecord]) {
    let composites: Vec<usize> = out
        .iter()
        .filter(|record| record.name == "composite_curve_org")
        .map(|record| record.record)
        .collect();
    let number = |arg: &mut Arg| {
        if let Arg::Text(text) = arg
            && let Ok(record) = text.parse::<usize>()
        {
            let at = composites.iter().position(|known| *known == record);
            *text = at.map_or(0, |at| at + 1).to_string();
        }
    };
    for record in out
        .iter_mut()
        .filter(|record| FILLS.contains(&record.name.as_str()))
    {
        let at = record.args.len().saturating_sub(3);
        let mut region = record.args.iter_mut().skip(at);
        if let Some(outer) = region.next() {
            number(outer);
        }
        if let Some(Arg::List(inner)) = region.nth(1) {
            inner.iter_mut().for_each(number);
        }
    }
}

/// 番号で引く実体
#[derive(Clone, Copy)]
struct Instance<'f> {
    record: usize,
    parts: &'f [Part],
}

/// 軸の置き方（AXIS2_PLACEMENT_2D）。角度は度
struct Placement {
    x: f64,
    y: f64,
    angle: f64,
    /// X 軸の向きの単位の値
    axis: [f64; 2],
}

/// 端末記号。X 軸は矢印の指す向き
struct Terminator {
    code: usize,
    x: f64,
    y: f64,
    axis: [f64; 2],
    scale: f64,
}

/// トリムの値。媒介変数（PARAMETER_VALUE）か点
#[derive(Default)]
struct Trim {
    parameter: Option<f64>,
    point: Option<[f64; 2]>,
}

struct Builder<'f> {
    file: &'f SxfFile,
    instances: BTreeMap<u64, Instance<'f>>,
    out: Vec<FeatureRecord>,
    colors: BTreeMap<u64, i64>,
    line_types: BTreeMap<u64, i64>,
    fonts: BTreeMap<u64, i64>,
    /// 利用者定義線幅（mm）。コードは 11 から
    widths: Vec<f64>,
    /// 要素（表記の実体）からレイヤのコードへ
    layer_of: BTreeMap<u64, i64>,
    /// レイヤのコードの順の、PRESENTATION_LAYER_ASSIGNMENT の番号
    layers: Vec<u64>,
    /// 複合図形の定義のレコード。用紙の要素より前に書く
    definitions: Vec<FeatureRecord>,
    /// いま組み直している図形の並びの、複合曲線のレコード。並びの先に書く
    front: Vec<FeatureRecord>,
    /// 組み直した複合図形の定義（DRAUGHTING_SUBFIGURE_REPRESENTATION）の名前
    subfigures: BTreeMap<u64, String>,
    /// 組み直した複合曲線（COMPOSITE_CURVE）
    composites: Vec<u64>,
    /// 非表示にする要素（INVISIBILITY）
    invisible: Vec<u64>,
    depth: usize,
}

impl<'f> Builder<'f> {
    fn new(file: &'f SxfFile) -> Self {
        let mut instances = BTreeMap::new();
        for (record, entry) in file.records().iter().enumerate() {
            if let Some(id) = entry.id() {
                // 同じ番号の 2 つ目は使わない
                instances.entry(id).or_insert(Instance {
                    record,
                    parts: entry.parts(),
                });
            }
        }
        Self {
            file,
            instances,
            out: Vec::new(),
            colors: BTreeMap::new(),
            line_types: BTreeMap::new(),
            fonts: BTreeMap::new(),
            widths: Vec::new(),
            layer_of: BTreeMap::new(),
            layers: Vec::new(),
            definitions: Vec::new(),
            front: Vec::new(),
            subfigures: BTreeMap::new(),
            composites: Vec::new(),
            invisible: Vec::new(),
            depth: 0,
        }
    }

    /// 参照の先がない実体を報告する
    fn check_references(&self, cx: &mut Cx) -> Result<(), Error> {
        for instance in self.instances.values() {
            let source = Ref::at_record(instance.record as u64);
            work(cx, 1, &source)?;
            let mut missing = Vec::new();
            for part in instance.parts {
                self.dangling(part.args(), &mut missing);
            }
            for id in missing {
                cx.report(
                    Diagnostic::new(codes::SXF_P21_REFERENCE_MISSING)
                        .with("id", id)
                        .at(source.clone()),
                );
            }
        }
        Ok(())
    }

    fn dangling(&self, values: &[Value], missing: &mut Vec<u64>) {
        for value in values {
            match value {
                Value::Ref(id) if !self.instances.contains_key(id) => {
                    if !missing.contains(id) {
                        missing.push(*id);
                    }
                }
                Value::List(items) => self.dangling(items, missing),
                Value::Typed(part) => self.dangling(part.args(), missing),
                _ => {}
            }
        }
    }

    /// 番号の順に並べた、名前の部分を持つ実体の番号とその引数
    fn all(&self, name: &str) -> Vec<(u64, Instance<'f>, &'f [Value])> {
        self.instances
            .iter()
            .filter_map(|(id, instance)| Some((*id, *instance, self.part(*instance, name)?)))
            .collect()
    }

    fn part(&self, instance: Instance<'f>, name: &str) -> Option<&'f [Value]> {
        instance
            .parts
            .iter()
            .find(|part| {
                self.file
                    .bytes(part.name())
                    .eq_ignore_ascii_case(name.as_bytes())
            })
            .map(Part::args)
    }

    fn args(&self, id: Option<u64>, name: &str) -> Option<&'f [Value]> {
        self.part(*self.instances.get(&id?)?, name)
    }

    fn has(&self, id: Option<u64>, name: &str) -> bool {
        self.args(id, name).is_some()
    }

    fn push(&mut self, record: usize, name: &str, kind: &str, args: Vec<Arg>) {
        self.out.push(FeatureRecord {
            record,
            name: name.to_owned(),
            kind: kind.to_owned(),
            args,
        });
    }

    fn text(&self, value: Option<&Value>) -> Option<String> {
        match value? {
            Value::Text { raw, .. } => Some(decode(self.file.bytes(raw.clone()))),
            Value::Typed(part) => self.text(part.args().first()),
            _ => None,
        }
    }

    fn number(&self, value: Option<&Value>) -> Option<f64> {
        match value? {
            Value::Number(raw) => core::str::from_utf8(self.file.bytes(raw.clone()))
                .ok()?
                .parse::<f64>()
                .ok()
                .filter(|value| value.is_finite()),
            Value::Typed(part) => self.number(part.args().first()),
            _ => None,
        }
    }

    fn enumeration(&self, value: Option<&Value>) -> Option<&'f [u8]> {
        match value? {
            Value::Enum(raw) => Some(self.file.bytes(raw.clone())),
            _ => None,
        }
    }

    fn point(&self, id: Option<u64>) -> Option<[f64; 2]> {
        match self.args(id, "CARTESIAN_POINT")?.get(1)? {
            Value::List(values) => {
                Some([self.number(values.first())?, self.number(values.get(1))?])
            }
            _ => None,
        }
    }

    fn direction(&self, id: Option<u64>) -> Option<[f64; 2]> {
        match self.args(id, "DIRECTION")?.get(1)? {
            Value::List(values) => {
                Some([self.number(values.first())?, self.number(values.get(1))?])
            }
            _ => None,
        }
    }

    /// 原点と X 軸の向き。向きを省けば X 軸のまま
    fn placement(&self, id: Option<u64>) -> Option<Placement> {
        let args = self.args(id, "AXIS2_PLACEMENT_2D")?;
        let [x, y] = self.point(reference(args.get(1)))?;
        let [dx, dy] = match reference(args.get(2)) {
            None => [1.0, 0.0],
            Some(direction) => self.direction(Some(direction))?,
        };
        let length = libm::hypot(dx, dy);
        if length == 0.0 {
            return None;
        }
        Some(Placement {
            x,
            y,
            angle: degrees(libm::atan2(dy, dx)),
            axis: [dx / length, dy / length],
        })
    }

    /// 色・線種・文字フォント・レイヤを、番号の順にフィーチャにする。コードは SFC と同じに、書いた順で決まる
    fn tables(&mut self, cx: &mut Cx) -> Result<(), Error> {
        let mut users = 0;
        for (id, instance, args) in self.all("COLOUR_RGB") {
            work(cx, 1, &Ref::at_record(instance.record as u64))?;
            let name = self.text(args.first()).unwrap_or_default();
            // 既定義色の RGB を、名前に $$SXF_ を付けて書くことがある
            if let Some(code) = name.trim().strip_prefix("$$SXF_").and_then(color_code) {
                self.colors.insert(id, code);
                continue;
            }
            let channel = |at: usize| {
                self.number(args.get(at))
                    .map(|value| Arg::Text(format!("{}", libm::round(value * 255.0))))
                    .unwrap_or(Arg::Other)
            };
            let rgb = vec![channel(1), channel(2), channel(3)];
            self.push(instance.record, "user_defined_colour", "colour_rgb", rgb);
            users += 1;
            self.colors.insert(id, 16 + users);
        }
        for (id, _, args) in self.all("DRAUGHTING_PRE_DEFINED_COLOUR") {
            let name = self.text(args.first()).unwrap_or_default();
            self.colors
                .insert(id, color_code(&name).unwrap_or_default());
        }
        for (id, _, args) in self.all("DRAUGHTING_PRE_DEFINED_CURVE_FONT") {
            let name = self.text(args.first()).unwrap_or_default();
            let code = LINE_TYPES
                .iter()
                .position(|known| known.eq_ignore_ascii_case(name.trim()))
                .map_or(1, |at| at as i64 + 1);
            self.line_types.insert(id, code);
        }
        let mut users = 0;
        for (id, instance, args) in self.all("CURVE_STYLE_FONT") {
            let mut pitch = Vec::new();
            for pattern in references(args.get(1)) {
                let pattern = self
                    .args(Some(pattern), "CURVE_STYLE_FONT_PATTERN")
                    .unwrap_or_default();
                pitch.push(num(self.number(pattern.first())));
                pitch.push(num(self.number(pattern.get(1))));
            }
            let name = self.text(args.first()).map_or(Arg::Other, Arg::Text);
            let segments = Arg::Text(pitch.len().to_string());
            let font = vec![name, segments, Arg::List(pitch)];
            self.push(
                instance.record,
                "user_defined_font",
                "curve_style_font",
                font,
            );
            users += 1;
            self.line_types.insert(id, 16 + users);
        }
        for (id, instance, args) in self.all("EXTERNALLY_DEFINED_TEXT_FONT") {
            let name = self.text(args.first()).map_or(Arg::Other, Arg::Text);
            let code = self.fonts.len() as i64 + 1;
            self.fonts.insert(id, code);
            self.push(
                instance.record,
                "text_font",
                "externally_defined_text_font",
                vec![name],
            );
        }
        let invisible: Vec<u64> = self
            .all("INVISIBILITY")
            .into_iter()
            .flat_map(|(_, _, args)| references(args.first()))
            .collect();
        self.invisible.clone_from(&invisible);
        let mut code = 0;
        for (id, instance, args) in self.all("PRESENTATION_LAYER_ASSIGNMENT") {
            let name = self.text(args.first()).unwrap_or_default();
            // 複合図形を入れるための仮のレイヤは、レイヤにしない（推定）
            if name.starts_with("$$SXF_dummy") {
                continue;
            }
            let visible = if invisible.contains(&id) { "0" } else { "1" };
            let layer = vec![Arg::Text(name), Arg::Text(visible.to_owned())];
            self.push(
                instance.record,
                "layer",
                "presentation_layer_assignment",
                layer,
            );
            code += 1;
            self.layers.push(id);
            for item in references(args.get(2)) {
                self.layer_of.entry(item).or_insert(code);
            }
        }
        Ok(())
    }

    /// 用紙の要素を並びの順にフィーチャにし、用紙を最後に置く
    fn sheet(&mut self, cx: &mut Cx) -> Result<(), Error> {
        let Some((id, instance, args)) = self.all("DRAWING_SHEET_REVISION").into_iter().next()
        else {
            cx.report(Diagnostic::new(codes::SXF_P21_SHEET_MISSING));
            return Ok(());
        };
        for item in references(args.get(1)) {
            let Some(occurrence) = self.instances.get(&item).copied() else {
                continue;
            };
            work(cx, 1, &Ref::at_record(occurrence.record as u64))?;
            self.item(item, occurrence);
        }
        let size = self.text(args.first()).unwrap_or_default();
        let (kind, orientation) = sheet_kind(size.trim()).unwrap_or((9, 1));
        let mut extent = [Arg::Other, Arg::Other];
        for (_, _, size) in self.all("PRESENTATION_SIZE") {
            if reference(size.first()) == Some(id)
                && let Some(planar) = self.args(reference(size.get(1)), "PLANAR_BOX")
            {
                extent = [
                    num(self.number(planar.get(1))),
                    num(self.number(planar.get(2))),
                ];
            }
        }
        let name = self
            .all("DRAUGHTING_TITLE")
            .into_iter()
            .find_map(|(_, _, title)| self.text(title.get(2)))
            .unwrap_or_default();
        let [width, height] = extent;
        let sheet = vec![
            Arg::Text(name.trim().to_owned()),
            Arg::Text(kind.to_string()),
            Arg::Text(orientation.to_string()),
            width,
            height,
        ];
        self.push(
            instance.record,
            "drawing_sheet",
            "drawing_sheet_revision",
            sheet,
        );
        Ok(())
    }

    /// 用紙の要素の 1 つ
    fn item(&mut self, id: u64, occurrence: Instance<'f>) {
        let curve = self
            .part(occurrence, "ANNOTATION_CURVE_OCCURRENCE")
            .is_some();
        let (style, geometry) = self.styled(occurrence);
        if self
            .part(occurrence, "ANNOTATION_SUBFIGURE_OCCURRENCE")
            .is_some()
        {
            let args = self.subfigure(id, geometry);
            self.finish(occurrence, geometry, "sfig_locate", args);
            return;
        }
        if self
            .part(occurrence, "ANNOTATION_FILL_AREA_OCCURRENCE")
            .is_some()
        {
            match self.fill(id, style, geometry) {
                Some((name, args)) => self.push(
                    occurrence.record,
                    name,
                    "annotation_fill_area_occurrence",
                    args,
                ),
                None => self.push(
                    occurrence.record,
                    "",
                    "annotation_fill_area_occurrence",
                    Vec::new(),
                ),
            }
            return;
        }
        if self.part(occurrence, "DRAUGHTING_CALLOUT").is_some() {
            self.callout(occurrence);
            return;
        }
        if let Some(callout) = geometry
            .and_then(|target| self.instances.get(&target).copied())
            .filter(|target| self.part(*target, "DRAUGHTING_CALLOUT").is_some())
        {
            self.callout(callout);
            return;
        }
        if self
            .part(occurrence, "ANNOTATION_TEXT_OCCURRENCE")
            .is_some()
        {
            let args = self.text_string(id, style, geometry);
            self.finish(occurrence, geometry, "text_string", args);
            return;
        }
        if self
            .part(occurrence, "ANNOTATION_SYMBOL_OCCURRENCE")
            .is_some()
        {
            let symbol = self.symbol(id, style, geometry);
            match symbol {
                Some((name, args)) => self.finish(occurrence, geometry, name, Some(args)),
                None => self.finish(occurrence, geometry, "", None),
            }
            return;
        }
        if curve
            && let Some(composite) = geometry.filter(|id| self.has(Some(*id), "COMPOSITE_CURVE"))
        {
            // 複合曲線は、並びの先に置く
            self.composite(composite, self.layer(id));
            return;
        }
        if !curve {
            // 見え方を持つほかの要素は、まだ移さない
            let kind = self.styled_args(occurrence).and_then(|_| {
                let first = occurrence.parts.first().map(Part::name)?;
                Some(String::from_utf8_lossy(self.file.bytes(first)).to_ascii_lowercase())
            });
            // 見え方を持たない要素（用紙の枠の PLANAR_BOX など）は、図形にしない
            if let Some(kind) = kind {
                self.push(occurrence.record, "", &kind, Vec::new());
            }
            return;
        }
        let mut args = vec![self.layer(id)];
        args.extend(
            self.curve_style(style)
                .map(|code| Arg::Text(code.to_string())),
        );
        match self.curve(geometry, &mut args) {
            Some((name, kind)) => self.push(occurrence.record, name, &kind, args),
            None => self.finish(occurrence, geometry, "", None),
        }
    }

    /// 複合図形の配置（SFC の sfig_locate と同じ順）
    fn subfigure(&mut self, id: u64, mapped: Option<u64>) -> Option<Vec<Arg>> {
        let mapped = self.args(mapped, "MAPPED_ITEM")?;
        // 単純な実体は名前を先に持つ
        let at = usize::from(mapped.len() >= 3);
        let map = self.args(reference(mapped.get(at)), "SYMBOL_REPRESENTATION_MAP")?;
        let target = self.args(reference(mapped.get(at + 1)), "SYMBOL_TARGET")?;
        let name = self.definition(reference(map.get(1))?)?;
        let placement = self.placement(reference(target.get(1)))?;
        Some(vec![
            self.layer(id),
            Arg::Text(name),
            num(Some(placement.x)),
            num(Some(placement.y)),
            num(Some(placement.angle)),
            num(self.number(target.get(2))),
            num(self.number(target.get(3))),
        ])
    }

    /// 複合図形の定義を組み直して、名前を返す。要素と複合曲線をまとめて、定義のレコードに書く
    fn definition(&mut self, id: u64) -> Option<String> {
        if let Some(name) = self.subfigures.get(&id) {
            return Some(name.clone());
        }
        let instance = *self.instances.get(&id)?;
        let args = self.part(instance, "DRAUGHTING_SUBFIGURE_REPRESENTATION")?;
        if self.depth >= MAX_DEPTH {
            return None;
        }
        let full = self.text(args.first()).unwrap_or_default();
        let (name, flag) = SUBFIGURES
            .iter()
            .find_map(|(prefix, flag)| Some((full.strip_prefix(prefix)?.to_owned(), *flag)))
            .unwrap_or((full, 3));
        // 輪になる配置は、名前だけで指す
        self.subfigures.insert(id, name.clone());
        let out = core::mem::take(&mut self.out);
        let front = core::mem::take(&mut self.front);
        self.depth += 1;
        for item in references(args.get(1)) {
            if let Some(occurrence) = self.instances.get(&item).copied() {
                self.item(item, occurrence);
            }
        }
        self.depth -= 1;
        let mut members = core::mem::replace(&mut self.front, front);
        members.append(&mut core::mem::replace(&mut self.out, out));
        self.definitions.append(&mut members);
        self.definitions.push(FeatureRecord {
            record: instance.record,
            name: "sfig_org".to_owned(),
            kind: "draughting_subfigure_representation".to_owned(),
            args: vec![Arg::Text(name.clone()), Arg::Text(flag.to_string())],
        });
        Some(name)
    }

    /// 複合曲線を、いまの並びの先に書く。曲線の見え方とレイヤは、複合曲線を指す曲線の要素から読み、
    /// なければ `layer` にする
    fn composite(&mut self, id: u64, layer: Arg) {
        if self.composites.contains(&id) {
            return;
        }
        self.composites.push(id);
        let Some(instance) = self.instances.get(&id).copied() else {
            return;
        };
        let occurrence = self
            .instances
            .iter()
            .filter(|(_, candidate)| {
                self.part(**candidate, "ANNOTATION_CURVE_OCCURRENCE")
                    .is_some()
            })
            .find(|(_, candidate)| self.styled(**candidate).1 == Some(id))
            .map(|(occurrence, candidate)| (*occurrence, *candidate));
        let (layer, style, visible) = match occurrence {
            Some((occurrence, candidate)) => (
                self.layer(occurrence),
                self.curve_style(self.styled(candidate).0),
                !self.invisible.contains(&occurrence),
            ),
            None => (layer, DEFAULT_STYLE, true),
        };
        let segments = references(
            self.part(instance, "COMPOSITE_CURVE")
                .and_then(|args| args.get(1)),
        );
        for segment in segments {
            let parent = self
                .args(Some(segment), "COMPOSITE_CURVE_SEGMENT")
                .and_then(|args| reference(args.get(2)));
            let mut args = vec![layer.clone()];
            args.extend(style.map(|code| Arg::Text(code.to_string())));
            let record = parent
                .and_then(|parent| self.instances.get(&parent))
                .map_or(instance.record, |parent| parent.record);
            let (name, kind, args) = match self.curve(parent, &mut args) {
                Some((name, kind)) => (name.to_owned(), kind, args),
                None => (
                    String::new(),
                    "composite_curve_segment".to_owned(),
                    Vec::new(),
                ),
            };
            self.front.push(FeatureRecord {
                record,
                name,
                kind,
                args,
            });
        }
        let mut args: Vec<Arg> = style
            .iter()
            .map(|code| Arg::Text(code.to_string()))
            .collect();
        args.push(Arg::Text(if visible { "1" } else { "0" }.to_owned()));
        self.front.push(FeatureRecord {
            record: instance.record,
            name: "composite_curve_org".to_owned(),
            kind: "composite_curve".to_owned(),
            args,
        });
    }

    /// ハッチングの、フィーチャの名前と引数（SFC と同じ順）。外形と中抜きは、複合曲線のレコードで書き、
    /// 最後に複合曲線の番号にする
    fn fill(
        &mut self,
        id: u64,
        style: Option<u64>,
        area: Option<u64>,
    ) -> Option<(&'static str, Vec<Arg>)> {
        let boundaries: Vec<u64> = references(self.args(area, "ANNOTATION_FILL_AREA")?.get(1))
            .into_iter()
            .filter(|boundary| self.has(Some(*boundary), "COMPOSITE_CURVE"))
            .collect();
        let (outer, inner) = boundaries.split_first()?;
        let layer = self.layer(id);
        for boundary in &boundaries {
            self.composite(*boundary, layer.clone());
        }
        let record = |id: &u64| {
            Arg::Text(
                self.instances
                    .get(id)
                    .map_or(0, |instance| instance.record)
                    .to_string(),
            )
        };
        let region = [
            record(outer),
            Arg::Text(inner.len().to_string()),
            Arg::List(inner.iter().map(record).collect()),
        ];
        let fills = references(
            self.args(style, "PRESENTATION_STYLE_ASSIGNMENT")
                .and_then(<[Value]>::first),
        )
        .into_iter()
        .filter_map(|style| self.args(Some(style), "FILL_AREA_STYLE"))
        .flat_map(|args| references(args.get(1)))
        .collect::<Vec<u64>>();
        let mut patterns = Vec::new();
        let mut args = vec![layer];
        for fill in fills {
            if let Some(colour) = self.args(Some(fill), "FILL_AREA_STYLE_COLOUR") {
                args.push(Arg::Text(self.color(reference(colour.get(1))).to_string()));
                args.extend(region);
                return Some(("fill_area_style_colour", args));
            }
            if let Some(external) = self.args(Some(fill), "EXTERNALLY_DEFINED_HATCH_STYLE") {
                args.push(self.text(external.first()).map_or(Arg::Other, Arg::Text));
                args.extend(region);
                return Some(("externally_defined_hatch", args));
            }
            if let Some(tiles) = self.args(Some(fill), "FILL_AREA_STYLE_TILES") {
                args.extend(self.tiles(tiles)?);
                args.extend(region);
                return Some(("fill_area_style_tiles", args));
            }
            if let Some(hatching) = self.args(Some(fill), "FILL_AREA_STYLE_HATCHING") {
                patterns.push(self.hatching(hatching)?);
            }
        }
        if patterns.is_empty() {
            return None;
        }
        args.push(Arg::Text(patterns.len().to_string()));
        args.extend(patterns);
        args.extend(region);
        Some(("fill_area_style_hatching", args))
    }

    /// ハッチングの線の 1 本。色・線種・線幅のコード、通る点、間隔、角度（度）
    fn hatching(&mut self, hatching: &'f [Value]) -> Option<Arg> {
        let style = self.args(reference(hatching.get(1)), "CURVE_STYLE")?;
        let line_type = reference(style.get(1)).map_or(1, |id| {
            self.line_types.get(&id).copied().unwrap_or_default()
        });
        let width = self.width(style.get(2));
        let color = self.color(reference(style.get(3)));
        let repeat = self.args(reference(hatching.get(2)), "ONE_DIRECTION_REPEAT_FACTOR")?;
        let vector = self.args(reference(repeat.get(1)), "VECTOR")?;
        let [x, y] = self.point(reference(hatching.get(3)))?;
        let mut values: Vec<Arg> = [color, line_type, width]
            .iter()
            .map(|code| Arg::Text(code.to_string()))
            .collect();
        values.extend([
            num(Some(x)),
            num(Some(y)),
            num(self.number(vector.get(2))),
            num(self.number(hatching.get(5)).map(degrees)),
        ]);
        Some(Arg::List(values))
    }

    /// タイルのハッチングの、名前から角度までの引数。タイルの記号は、最初の外部定義の記号とする
    fn tiles(&self, tiles: &'f [Value]) -> Option<Vec<Arg>> {
        let repeat = self.args(reference(tiles.get(1)), "TWO_DIRECTION_REPEAT_FACTOR")?;
        let vector = |at: usize| -> Option<[Arg; 2]> {
            let vector = self.args(reference(repeat.get(at)), "VECTOR")?;
            let [dx, dy] = self.direction(reference(vector.get(1)))?;
            Some([
                num(self.number(vector.get(2))),
                num(Some(degrees(libm::atan2(dy, dx)))),
            ])
        };
        let [first, first_angle] = vector(1)?;
        let [second, second_angle] = vector(2)?;
        let tile = references(tiles.get(2))
            .into_iter()
            .find_map(|tile| self.args(Some(tile), "FILL_AREA_STYLE_TILE_SYMBOL_WITH_STYLE"))?;
        let occurrence = *self.instances.get(&reference(tile.get(1))?)?;
        let (style, defined) = self.styled(occurrence);
        let target = self
            .args(defined, "DEFINED_SYMBOL")
            .and_then(|symbol| self.args(reference(symbol.get(2)), "SYMBOL_TARGET"))?;
        let (_, symbol) = self.symbol(0, style, defined)?;
        // 外部定義の記号の引数は、レイヤ・色の有無・色・名前・X・Y・角度・倍率
        let [_, _, color, name, x, y, angle, _] = <[Arg; 8]>::try_from(symbol).ok()?;
        Some(vec![
            name,
            color,
            x,
            y,
            first,
            first_angle,
            second,
            second_angle,
            num(self.number(target.get(2))),
            num(self.number(target.get(3))),
            angle,
        ])
    }

    /// 組み直した要素を足す。引数がなければ、対象の実体の名前を種類にして、まだ移さないものにする
    fn finish(
        &mut self,
        occurrence: Instance<'f>,
        target: Option<u64>,
        name: &str,
        args: Option<Vec<Arg>>,
    ) {
        let kind = target
            .and_then(|target| self.instances.get(&target))
            .and_then(|instance| instance.parts.first())
            .map(|part| String::from_utf8_lossy(self.file.bytes(part.name())).to_ascii_lowercase())
            .unwrap_or_else(|| {
                let first = occurrence.parts.first().map(Part::name).unwrap_or_default();
                String::from_utf8_lossy(self.file.bytes(first)).to_ascii_lowercase()
            });
        match args {
            Some(args) => self.push(occurrence.record, name, &kind, args),
            None => self.push(occurrence.record, "", &kind, Vec::new()),
        }
    }

    /// 寸法・引出線・バルーン（DRAUGHTING_CALLOUT）。レイヤと線の見え方は、寸法線か引出線の要素から読む
    fn callout(&mut self, callout: Instance<'f>) {
        // 寸法値だけをまとめた callout は、寸法の callout が同じ文字を持つので、要素にしない
        if self.part(callout, "STRUCTURED_DIMENSION_CALLOUT").is_some() {
            return;
        }
        let contents: Vec<(u64, Instance<'f>)> = references(
            self.part(callout, "DRAUGHTING_CALLOUT")
                .and_then(<[Value]>::first),
        )
        .into_iter()
        .filter_map(|id| Some((id, *self.instances.get(&id)?)))
        .collect();
        let text = contents
            .iter()
            .find(|(_, item)| self.part(*item, "ANNOTATION_TEXT_OCCURRENCE").is_some())
            .and_then(|(_, item)| {
                let (style, literal) = self.styled(*item);
                self.annotation(style, literal)
            })
            .map(|(_, annotation)| annotation);
        let mut text_args = vec![Arg::Text(if text.is_some() { "1" } else { "0" }.to_owned())];
        text_args.extend(text.into_iter().flatten());
        let kind = [
            "LINEAR_DIMENSION",
            "CURVE_DIMENSION",
            "ANGULAR_DIMENSION",
            "RADIUS_DIMENSION",
            "DIAMETER_DIMENSION",
            "LEADER_DIRECTED_CALLOUT",
        ]
        .into_iter()
        .find(|name| self.part(callout, name).is_some());
        let read = match kind {
            Some("LEADER_DIRECTED_CALLOUT") => self.leader(&contents, text_args),
            Some(kind) => self.dimension(kind, &contents, text_args),
            None => None,
        };
        let kind = kind.map_or_else(|| "draughting_callout".to_owned(), str::to_ascii_lowercase);
        match read {
            Some((name, args)) => self.push(callout.record, name, &kind, args),
            None => self.push(callout.record, "", &kind, Vec::new()),
        }
    }

    /// 寸法線か引出線の要素の、レイヤと線の見え方の引数と、線の幾何
    fn callout_curve(
        &mut self,
        contents: &[(u64, Instance<'f>)],
        role: &str,
    ) -> Option<(Vec<Arg>, Option<u64>)> {
        let (id, curve) = contents
            .iter()
            .find(|(_, item)| self.part(*item, role).is_some())
            .copied()?;
        let (style, geometry) = self.styled(curve);
        let mut args = vec![self.layer(id)];
        args.extend(
            self.curve_style(style)
                .map(|code| Arg::Text(code.to_string())),
        );
        Some((args, geometry))
    }

    /// 曲線の幾何の、フィーチャの名前と数の値
    fn curve_values(&self, geometry: Option<u64>) -> Option<(&'static str, Vec<f64>)> {
        let mut args = Vec::new();
        let (name, _) = self.curve(geometry, &mut args)?;
        let values = args
            .iter()
            .map(|arg| match arg {
                Arg::Text(text) => text.parse::<f64>().ok(),
                _ => None,
            })
            .collect::<Option<Vec<f64>>>()?;
        Some((name, values))
    }

    /// 寸法の、フィーチャの名前と引数（SFC と同じ順）
    fn dimension(
        &mut self,
        kind: &str,
        contents: &[(u64, Instance<'f>)],
        text: Vec<Arg>,
    ) -> Option<(&'static str, Vec<Arg>)> {
        let (mut args, geometry) = self.callout_curve(contents, "DIMENSION_CURVE")?;
        let (shape, values) = self.curve_values(geometry)?;
        // 寸法補助線は、名前（$$SXF_prj_1・$$SXF_prj_2）の順
        let mut projections: Vec<(String, Instance<'f>)> = contents
            .iter()
            .filter(|(_, item)| self.part(*item, "PROJECTION_CURVE").is_some())
            .map(|(_, item)| {
                let name = self
                    .part(*item, "REPRESENTATION_ITEM")
                    .and_then(|args| self.text(args.first()))
                    .unwrap_or_default();
                (name, *item)
            })
            .collect();
        projections.sort_by(|a, b| a.0.cmp(&b.0));
        let extensions: Vec<Arg> = [0, 1]
            .into_iter()
            .flat_map(|at| self.extension(projections.get(at).map(|(_, item)| *item)))
            .collect();
        let terminators: Vec<(&'f [u8], Instance<'f>)> = contents
            .iter()
            .filter_map(|(_, item)| {
                let role = self.part(*item, "DIMENSION_CURVE_TERMINATOR")?;
                Some((self.enumeration(role.first()).unwrap_or_default(), *item))
            })
            .collect();
        let arrow = |role: &[u8]| -> Vec<Arg> {
            let terminator = terminators
                .iter()
                .find(|(candidate, _)| candidate.eq_ignore_ascii_case(role))
                .and_then(|(_, item)| self.terminator(*item));
            let Some(terminator) = terminator else {
                return ["0", "0", "0", "0", "1"]
                    .map(|v| Arg::Text(v.to_owned()))
                    .to_vec();
            };
            // 端末記号の X 軸は矢印の指す向き。SFC の向きは、1 が寸法線の外向き、2 が内向き
            let [x, y] = [terminator.x, terminator.y];
            let outward = match (shape, values.as_slice()) {
                ("line", &[x1, y1, x2, y2]) => {
                    let near = libm::hypot(x - x1, y - y1) <= libm::hypot(x - x2, y - y2);
                    if near {
                        [x1 - x2, y1 - y2]
                    } else {
                        [x2 - x1, y2 - y1]
                    }
                }
                ("arc", &[cx, cy, ..]) => {
                    let angle = libm::atan2(y - cy, x - cx);
                    let (sin, cos) = (libm::sin(angle), libm::cos(angle));
                    if role.eq_ignore_ascii_case(b"ORIGIN") {
                        [sin, -cos]
                    } else {
                        [-sin, cos]
                    }
                }
                _ => terminator.axis,
            };
            let dot = outward[0] * terminator.axis[0] + outward[1] * terminator.axis[1];
            vec![
                Arg::Text(terminator.code.to_string()),
                Arg::Text(if dot >= 0.0 { "1" } else { "2" }.to_owned()),
                num(Some(x)),
                num(Some(y)),
                num(Some(terminator.scale)),
            ]
        };
        let (origin, target) = (arrow(b"ORIGIN"), arrow(b"TARGET"));
        let name = match (kind, shape, values.as_slice()) {
            ("LINEAR_DIMENSION", "line", line) => {
                args.extend(line.iter().map(|v| num(Some(*v))));
                args.extend(extensions);
                args.extend(origin);
                args.extend(target);
                "linear_dim"
            }
            ("CURVE_DIMENSION" | "ANGULAR_DIMENSION", "arc", &[cx, cy, r, _, start, end]) => {
                args.extend([cx, cy, r, start, end].map(|v| num(Some(v))));
                args.extend(extensions);
                args.extend(origin);
                args.extend(target);
                if kind == "CURVE_DIMENSION" {
                    "curve_dim"
                } else {
                    "angular_dim"
                }
            }
            ("RADIUS_DIMENSION", "line", line) => {
                args.extend(line.iter().map(|v| num(Some(*v))));
                let present = |arrow: &[Arg]| arrow.get(1) != Some(&Arg::Text("0".to_owned()));
                args.extend(if present(&origin) { origin } else { target });
                "radius_dim"
            }
            ("DIAMETER_DIMENSION", "line", line) => {
                args.extend(line.iter().map(|v| num(Some(*v))));
                args.extend(origin);
                args.extend(target);
                "diameter_dim"
            }
            _ => return None,
        };
        args.extend(text);
        Some((name, args))
    }

    /// 寸法補助線の引数。有無・基点（線の点）・始点・終点
    fn extension(&self, occurrence: Option<Instance<'f>>) -> Vec<Arg> {
        let read = occurrence.and_then(|occurrence| {
            let (_, geometry) = self.styled(occurrence);
            let trimmed = self.args(geometry, "TRIMMED_CURVE")?;
            let line = self.args(reference(trimmed.get(1)), "LINE")?;
            let base = self.point(reference(line.get(1)))?;
            let ("line", ends) = self.curve_values(geometry)? else {
                return None;
            };
            let mut values = base.to_vec();
            values.extend(ends);
            Some(values)
        });
        match read {
            Some(values) => {
                let mut args = vec![Arg::Text("1".to_owned())];
                args.extend(values.into_iter().map(|v| num(Some(v))));
                args
            }
            None => (0..7).map(|_| Arg::Text("0".to_owned())).collect(),
        }
    }

    /// 端末記号（矢印）。コードは、名前のないものを open arrow とする（推定）
    fn terminator(&self, occurrence: Instance<'f>) -> Option<Terminator> {
        let (_, target) = self.styled(occurrence);
        let symbol = self.args(target, "DEFINED_SYMBOL")?;
        let name = self
            .args(reference(symbol.get(1)), "PRE_DEFINED_TERMINATOR_SYMBOL")
            .and_then(|args| self.text(args.first()))
            .unwrap_or_default();
        let code = ARROWS
            .iter()
            .position(|known| known.eq_ignore_ascii_case(name.trim()))
            .map_or(DEFAULT_ARROW, |at| at + 1);
        let target = self.args(reference(symbol.get(2)), "SYMBOL_TARGET")?;
        let placement = self.placement(reference(target.get(1)))?;
        Some(Terminator {
            code,
            x: placement.x,
            y: placement.y,
            axis: placement.axis,
            scale: self
                .number(target.get(2))
                .filter(|scale| *scale != 0.0)
                .unwrap_or(1.0),
        })
    }

    /// 引出線かバルーンの、フィーチャの名前と引数（SFC と同じ順）。バルーンの円は、引出線でない曲線の要素の円
    fn leader(
        &mut self,
        contents: &[(u64, Instance<'f>)],
        text: Vec<Arg>,
    ) -> Option<(&'static str, Vec<Arg>)> {
        let (mut args, geometry) = self.callout_curve(contents, "LEADER_CURVE")?;
        let mut polyline = Vec::new();
        if self.curve(geometry, &mut polyline)?.0 != "polyline" {
            return None;
        }
        args.extend(polyline);
        let terminator = contents
            .iter()
            .find(|(_, item)| self.part(*item, "LEADER_TERMINATOR").is_some())
            .and_then(|(_, item)| self.terminator(*item));
        let balloon = contents
            .iter()
            .filter(|(_, item)| {
                self.part(*item, "ANNOTATION_CURVE_OCCURRENCE").is_some()
                    && self.part(*item, "LEADER_CURVE").is_none()
            })
            .find_map(|(_, item)| match self.curve_values(self.styled(*item).1)? {
                ("circle", circle) => Some(circle),
                _ => None,
            });
        let name = match balloon {
            Some(circle) => {
                args.extend(circle.into_iter().map(|v| num(Some(v))));
                "balloon"
            }
            None => "label",
        };
        args.push(Arg::Text(
            terminator.as_ref().map_or(0, |t| t.code).to_string(),
        ));
        args.push(num(Some(terminator.map_or(1.0, |t| t.scale))));
        args.extend(text);
        Some((name, args))
    }

    fn layer(&self, id: u64) -> Arg {
        Arg::Text(
            self.layer_of
                .get(&id)
                .copied()
                .unwrap_or_default()
                .to_string(),
        )
    }

    fn color(&self, id: Option<u64>) -> i64 {
        id.map_or(0, |id| self.colors.get(&id).copied().unwrap_or_default())
    }

    /// 文字の引数（SFC の text_string と同じ順）
    fn text_string(&self, id: u64, style: Option<u64>, literal: Option<u64>) -> Option<Vec<Arg>> {
        let (color, annotation) = self.annotation(style, literal)?;
        let mut args = vec![self.layer(id), Arg::Text(color.to_string())];
        args.extend(annotation);
        Some(args)
    }

    /// 文字の色のコードと、フォントのコードから書く向きまでの引数（SFC の文字の値と同じ順）。配置基点は、
    /// 行ごとにずらして書いた点を戻す
    fn annotation(&self, style: Option<u64>, literal: Option<u64>) -> Option<(i64, Vec<Arg>)> {
        let args = self
            .args(literal, "TEXT_LITERAL_WITH_EXTENT")
            .or_else(|| self.args(literal, "TEXT_LITERAL"))?;
        let alignment = self.text(args.first()).unwrap_or_default();
        let alignment = alignment.trim();
        let mut names = alignment
            .strip_prefix("$$SXF_")
            .unwrap_or(alignment)
            .split_whitespace();
        let row = names
            .next()
            .and_then(|name| {
                ROWS.iter()
                    .position(|known| known.eq_ignore_ascii_case(name))
            })
            .unwrap_or_default();
        let column = names
            .next()
            .and_then(|name| {
                COLUMNS
                    .iter()
                    .position(|known| known.iter().any(|known| known.eq_ignore_ascii_case(name)))
            })
            .unwrap_or_default();
        let placement = self.placement(reference(args.get(2)));
        let extent = self.args(reference(args.get(6)), "PLANAR_EXTENT");
        let width = extent.and_then(|extent| self.number(extent.get(1)));
        let height = extent.and_then(|extent| self.number(extent.get(2)));
        let (mut color, mut slant, mut spacing) = (8, 0.0, 0.0);
        let styles = references(
            self.args(style, "PRESENTATION_STYLE_ASSIGNMENT")
                .and_then(<[Value]>::first),
        );
        for style in styles {
            let appearance = self
                .args(Some(style), "TEXT_STYLE")
                .and_then(|text| self.args(reference(text.get(1)), "TEXT_STYLE_FOR_DEFINED_FONT"));
            if let Some(appearance) = appearance {
                color = self.color(reference(appearance.first()));
            }
            let characteristics = self
                .args(Some(style), "TEXT_STYLE_WITH_BOX_CHARACTERISTICS")
                .and_then(<[Value]>::first);
            if let Some(Value::List(characteristics)) = characteristics {
                for characteristic in characteristics {
                    if let Value::Typed(part) = characteristic
                        && self
                            .file
                            .bytes(part.name())
                            .eq_ignore_ascii_case(b"BOX_SLANT_ANGLE")
                    {
                        slant = self.number(part.args().first()).map_or(slant, degrees);
                    }
                }
            }
            if let Some(value) = self
                .args(Some(style), "TEXT_STYLE_WITH_SPACING")
                .and_then(<[Value]>::first)
            {
                spacing = self.number(Some(value)).unwrap_or(spacing);
            }
        }
        // 書き手は配置基点を、下・中・上の行ごとに、高さの 1/2・-1/2・-1 だけ文字の上の向きへずらす（推定。
        // 旧版が同じ図面の SFC と P21 で確かめた）
        let point = placement.as_ref().zip(height).map(|(p, height)| {
            let offset = match row {
                0 => height / 2.0,
                1 => -height / 2.0,
                _ => -height,
            };
            let [dx, dy] = p.axis;
            [p.x + dy * offset, p.y - dx * offset]
        });
        let font = reference(args.get(5))
            .map_or(0, |font| self.fonts.get(&font).copied().unwrap_or_default());
        let down = self.enumeration(args.get(4)) == Some(b"DOWN".as_slice());
        let annotation = vec![
            Arg::Text(font.to_string()),
            self.text(args.get(1)).map_or(Arg::Other, Arg::Text),
            num(point.map(|[x, _]| x)),
            num(point.map(|[_, y]| y)),
            num(height),
            num(width),
            num(Some(spacing)),
            num(placement.as_ref().map(|p| p.angle)),
            num(Some(slant)),
            Arg::Text((row * 3 + column + 1).to_string()),
            Arg::Text(if down { "2" } else { "1" }.to_owned()),
        ];
        Some((color, annotation))
    }

    /// 点マーカーか既定義シンボルの、フィーチャの名前と引数（SFC と同じ順）
    fn symbol(
        &self,
        id: u64,
        style: Option<u64>,
        symbol: Option<u64>,
    ) -> Option<(&'static str, Vec<Arg>)> {
        let symbol = self.args(symbol, "DEFINED_SYMBOL")?;
        let target = self.args(reference(symbol.get(2)), "SYMBOL_TARGET");
        let placement = self.placement(target.and_then(|target| reference(target.get(1))));
        let scale = target
            .and_then(|target| self.number(target.get(2)))
            .filter(|scale| *scale != 0.0)
            .unwrap_or(1.0);
        let mut color = None;
        let styles = references(
            self.args(style, "PRESENTATION_STYLE_ASSIGNMENT")
                .and_then(<[Value]>::first),
        );
        for style in styles {
            let colour = self
                .args(Some(style), "SYMBOL_STYLE")
                .and_then(|symbol| self.args(reference(symbol.get(1)), "SYMBOL_COLOUR"));
            if let Some(colour) = colour {
                color = Some(self.color(reference(colour.first())));
            }
        }
        let definition = reference(symbol.get(1));
        let point = [
            num(placement.as_ref().map(|p| p.x)),
            num(placement.as_ref().map(|p| p.y)),
        ];
        let angle = num(placement.as_ref().map(|p| p.angle));
        if let Some(marker) = self.args(definition, "PRE_DEFINED_POINT_MARKER_SYMBOL") {
            let name = self.text(marker.first()).unwrap_or_default();
            let code = MARKERS
                .iter()
                .position(|known| known.eq_ignore_ascii_case(name.trim()))
                .map_or(Arg::Other, |at| Arg::Text((at + 1).to_string()));
            let [x, y] = point;
            let args = vec![
                self.layer(id),
                Arg::Text(color.unwrap_or(8).to_string()),
                x,
                y,
                code,
                angle,
                num(Some(scale)),
            ];
            return Some(("point_marker", args));
        }
        if let Some(external) = self.args(definition, "EXTERNALLY_DEFINED_SYMBOL") {
            let [x, y] = point;
            let args = vec![
                self.layer(id),
                Arg::Text(if color.is_some() { "1" } else { "0" }.to_owned()),
                Arg::Text(color.unwrap_or_default().to_string()),
                self.text(external.first()).map_or(Arg::Other, Arg::Text),
                x,
                y,
                angle,
                num(Some(scale)),
            ];
            return Some(("externally_defined_symbol", args));
        }
        None
    }

    /// STYLED_ITEM の見え方と対象。複合の実体では、名前を REPRESENTATION_ITEM が持つ
    fn styled(&self, instance: Instance<'f>) -> (Option<u64>, Option<u64>) {
        let args = self.styled_args(instance).unwrap_or_default();
        let offset = usize::from(args.len() >= 3);
        (
            references(args.get(offset)).first().copied(),
            reference(args.get(offset + 1)),
        )
    }

    /// STYLED_ITEM の引数。単純な実体では、表記の実体（`…_OCCURRENCE`）の引数
    fn styled_args(&self, instance: Instance<'f>) -> Option<&'f [Value]> {
        self.part(instance, "STYLED_ITEM")
            .or_else(|| match instance.parts {
                [part] if self.file.bytes(part.name()).ends_with(b"_OCCURRENCE") => {
                    Some(part.args())
                }
                _ => None,
            })
    }

    /// PRESENTATION_STYLE_ASSIGNMENT の中の CURVE_STYLE の、色・線種・線幅のコード
    fn curve_style(&mut self, assignment: Option<u64>) -> [i64; 3] {
        let styles = references(
            self.args(assignment, "PRESENTATION_STYLE_ASSIGNMENT")
                .and_then(<[Value]>::first),
        );
        for style in styles {
            let Some(args) = self.args(Some(style), "CURVE_STYLE") else {
                continue;
            };
            let line_type = reference(args.get(1)).map_or(1, |id| {
                self.line_types.get(&id).copied().unwrap_or_default()
            });
            let width = self.width(args.get(2));
            let color = self.color(reference(args.get(3)));
            return [color, line_type, width];
        }
        DEFAULT_STYLE
    }

    /// 線幅（LENGTH_MEASURE_WITH_UNIT か数）のコード。既定義なら 1〜9、ほかは 11 から
    fn width(&mut self, value: Option<&Value>) -> i64 {
        let measure = reference(value);
        let width = match measure {
            None => self.number(value),
            Some(_) => self.number(
                self.args(measure, "LENGTH_MEASURE_WITH_UNIT")
                    .and_then(<[Value]>::first),
            ),
        };
        let Some(width) = width else {
            return 1;
        };
        if let Some(at) = WIDTHS.iter().position(|known| (known - width).abs() < 1e-6) {
            return at as i64 + 1;
        }
        if let Some(at) = self.widths.iter().position(|known| *known == width) {
            return at as i64 + 11;
        }
        let record = measure
            .and_then(|id| self.instances.get(&id))
            .map_or(0, |instance| instance.record);
        self.widths.push(width);
        self.push(
            record,
            "width",
            "length_measure_with_unit",
            vec![Arg::Text(format!("{width}"))],
        );
        self.widths.len() as i64 + 10
    }

    /// 曲線の幾何を、フィーチャの名前と図形の値にする。値は args に足す。種類は幾何の実体の名前
    fn curve(&self, id: Option<u64>, args: &mut Vec<Arg>) -> Option<(&'static str, String)> {
        if let Some(trimmed) = self.args(id, "TRIMMED_CURVE") {
            return self.trimmed(trimmed, args);
        }
        if let Some(polyline) = self.args(id, "POLYLINE") {
            let points = self.points(polyline.get(1));
            args.push(Arg::Text(points.len().to_string()));
            args.extend(split(&points));
            return Some(("polyline", "polyline".to_owned()));
        }
        if let Some(circle) = self.args(id, "CIRCLE") {
            let placement = self.placement(reference(circle.get(1)));
            args.extend(center(placement.as_ref()));
            args.push(num(self.number(circle.get(2))));
            return Some(("circle", "circle".to_owned()));
        }
        if let Some(ellipse) = self.args(id, "ELLIPSE") {
            let placement = self.placement(reference(ellipse.get(1)));
            args.extend(center(placement.as_ref()));
            args.push(num(self.number(ellipse.get(2))));
            args.push(num(self.number(ellipse.get(3))));
            args.push(num(placement.map(|p| p.angle)));
            return Some(("ellipse", "ellipse".to_owned()));
        }
        if let Some(bezier) = self.args(id, "BEZIER_CURVE") {
            let points = self.points(bezier.get(2));
            // SFC の開閉のフラグは 0 が閉じている
            let closed = self.enumeration(bezier.get(4)) == Some(b"T".as_slice());
            args.push(Arg::Text(if closed { "0" } else { "1" }.to_owned()));
            args.push(Arg::Text(points.len().to_string()));
            args.extend(split(&points));
            return Some(("spline", "bezier_curve".to_owned()));
        }
        None
    }

    fn points(&self, value: Option<&Value>) -> Vec<Option<[f64; 2]>> {
        references(value)
            .into_iter()
            .map(|point| self.point(Some(point)))
            .collect()
    }

    fn trim(&self, value: Option<&Value>) -> Trim {
        let items = match value {
            Some(Value::List(items)) => items.iter().collect(),
            Some(other) => vec![other],
            None => Vec::new(),
        };
        let mut trim = Trim::default();
        for item in items {
            match item {
                Value::Typed(part)
                    if self
                        .file
                        .bytes(part.name())
                        .eq_ignore_ascii_case(b"PARAMETER_VALUE") =>
                {
                    trim.parameter = self.number(part.args().first());
                }
                Value::Ref(id) => {
                    if let Some(point) = self.point(Some(*id)) {
                        trim.point = Some(point);
                    }
                }
                _ => {}
            }
        }
        trim
    }

    /// トリムした線・円・楕円・クロソイド。媒介変数か点で端を決める
    fn trimmed(&self, trimmed: &[Value], args: &mut Vec<Arg>) -> Option<(&'static str, String)> {
        let basis = reference(trimmed.get(1));
        let first = self.trim(trimmed.get(2));
        let second = self.trim(trimmed.get(3));
        // 向きが合っていれば反時計回り。SFC の向きのフラグは 0 が反時計回り
        let sense = self.enumeration(trimmed.get(4)) != Some(b"F".as_slice());
        let flag = Arg::Text(if sense { "0" } else { "1" }.to_owned());
        let kind = "trimmed_curve".to_owned();
        if let Some(line) = self.args(basis, "LINE") {
            let origin = self.point(reference(line.get(1)));
            let vector = self.args(reference(line.get(2)), "VECTOR");
            let direction = vector
                .and_then(|vector| self.direction(reference(vector.get(1))))
                .unwrap_or([1.0, 0.0]);
            let magnitude = vector
                .and_then(|vector| self.number(vector.get(2)))
                .unwrap_or(1.0);
            let length = libm::hypot(direction[0], direction[1]);
            let at = |trim: &Trim| match (trim.point, trim.parameter, origin) {
                (Some(point), _, _) => Some(point),
                (None, Some(t), Some([x, y])) if length > 0.0 => Some([
                    x + direction[0] / length * magnitude * t,
                    y + direction[1] / length * magnitude * t,
                ]),
                _ => None,
            };
            for end in [at(&first), at(&second)] {
                args.extend([num(end.map(|[x, _]| x)), num(end.map(|[_, y]| y))]);
            }
            return Some(("line", kind));
        }
        let circle = self.args(basis, "CIRCLE");
        let ellipse = self.args(basis, "ELLIPSE");
        if let Some(conic) = circle.or(ellipse) {
            let placement = self.placement(reference(conic.get(1)));
            let angle = |trim: &Trim| match (trim.parameter, trim.point, placement.as_ref()) {
                (Some(t), _, _) => Some(degrees(t)),
                (None, Some([x, y]), Some(p)) => {
                    Some(degrees(libm::atan2(y - p.y, x - p.x)) - p.angle)
                }
                _ => None,
            };
            let (start, end) = (angle(&first), angle(&second));
            args.extend(center(placement.as_ref()));
            let rotation = placement.as_ref().map(|p| p.angle);
            if let Some(circle) = circle {
                args.push(num(self.number(circle.get(2))));
                args.push(flag);
                let turn = |value: Option<f64>| Some(value? + rotation?);
                args.extend([num(turn(start)), num(turn(end))]);
                return Some(("arc", kind));
            }
            let ellipse = ellipse.unwrap_or_default();
            args.push(num(self.number(ellipse.get(2))));
            args.push(num(self.number(ellipse.get(3))));
            args.extend([flag, num(rotation), num(start), num(end)]);
            return Some(("ellipse_arc", kind));
        }
        if let Some(clothoid) = self.args(basis, "CLOTHOID") {
            let placement = self.placement(reference(clothoid.get(1)));
            args.extend(center(placement.as_ref()));
            args.push(num(self.number(clothoid.get(2))));
            // 曲がる向きのフラグは名前に書く（0 が反時計回り）
            let name = self.text(clothoid.first()).unwrap_or_default();
            args.push(Arg::Text(
                if name.trim() == "1" { "1" } else { "0" }.to_owned(),
            ));
            args.push(num(placement.map(|p| p.angle)));
            args.push(num(Some(first.parameter.unwrap_or_default())));
            args.push(num(Some(second.parameter.unwrap_or_default())));
            return Some(("clothoid", kind));
        }
        None
    }
}

fn reference(value: Option<&Value>) -> Option<u64> {
    match value? {
        Value::Ref(id) => Some(*id),
        _ => None,
    }
}

fn references(value: Option<&Value>) -> Vec<u64> {
    match value {
        Some(Value::List(items)) => items
            .iter()
            .filter_map(|item| reference(Some(item)))
            .collect(),
        _ => Vec::new(),
    }
}

fn degrees(radians: f64) -> f64 {
    radians * 180.0 / core::f64::consts::PI
}

fn num(value: Option<f64>) -> Arg {
    value.map_or(Arg::Other, |value| Arg::Text(format!("{value}")))
}

fn center(placement: Option<&Placement>) -> [Arg; 2] {
    [num(placement.map(|p| p.x)), num(placement.map(|p| p.y))]
}

/// 点の並びから、X の値の並びと Y の値の並びを作る
fn split(points: &[Option<[f64; 2]>]) -> impl Iterator<Item = Arg> {
    let axis = |at: usize| {
        Arg::List(
            points
                .iter()
                .map(|point| num(point.and_then(|point| point.get(at).copied())))
                .collect(),
        )
    };
    [axis(0), axis(1)].into_iter()
}

fn color_code(name: &str) -> Option<i64> {
    COLORS
        .iter()
        .position(|known| known.eq_ignore_ascii_case(name.trim()))
        .map(|at| at as i64 + 1)
}

/// 用紙の名前（`A3_horizontal` など）の、用紙サイズ種別と向き（1 が横）
fn sheet_kind(name: &str) -> Option<(i64, i64)> {
    let (size, orientation) = name.split_once('_')?;
    let kind = size
        .strip_prefix(['A', 'a'])?
        .parse::<i64>()
        .ok()
        .filter(|kind| (0..=4).contains(kind))?;
    let orientation = if orientation.eq_ignore_ascii_case("horizontal") {
        1
    } else if orientation.eq_ignore_ascii_case("vertical") {
        0
    } else {
        return None;
    };
    Some((kind, orientation))
}

/// ISO 10303-21 の文字列のエスケープ（`''`・`\X2\…\X0\`・`\X4\…\X0\`・`\X\hh`・`\S\c`・`\P?\`・`\\`）を戻す。
/// 規格の外の 8 ビットのバイトは、Shift_JIS として読む（推定。旧版の扱いを引き継ぐ）
fn decode(raw: &[u8]) -> String {
    let mut out: Vec<u16> = Vec::with_capacity(raw.len());
    let mut rest = raw;
    while let Some((&byte, tail)) = rest.split_first() {
        if byte == b'\'' && tail.first() == Some(&b'\'') {
            out.push(u16::from(b'\''));
            rest = tail.get(1..).unwrap_or_default();
            continue;
        }
        if byte == b'\\' {
            if let Some((units, after)) = hex_run(rest, b"\\X2\\", 4) {
                out.extend(units.iter().map(|unit| *unit as u16));
                rest = after;
                continue;
            }
            if let Some((units, after)) = hex_run(rest, b"\\X4\\", 8) {
                for unit in units {
                    let c = char::from_u32(unit).unwrap_or(char::REPLACEMENT_CHARACTER);
                    out.extend(c.encode_utf16(&mut [0; 2]).iter());
                }
                rest = after;
                continue;
            }
            if let Some(after) = rest.strip_prefix(b"\\X\\")
                && let Some(hex) = after
                    .get(..2)
                    .filter(|hex| hex.iter().all(u8::is_ascii_hexdigit))
            {
                out.push(hex_value(hex) as u16);
                rest = after.get(2..).unwrap_or_default();
                continue;
            }
            if let Some(after) = rest.strip_prefix(b"\\S\\")
                && let Some((&c, after)) = after.split_first()
            {
                out.push(u16::from(c.wrapping_add(128)));
                rest = after;
                continue;
            }
            if let [b'\\', b'P', page, b'\\', after @ ..] = rest
                && (b'A'..=b'I').contains(page)
            {
                rest = after;
                continue;
            }
            if tail.first() == Some(&b'\\') {
                out.push(u16::from(b'\\'));
                rest = tail.get(1..).unwrap_or_default();
                continue;
            }
        }
        if byte >= 0x80 {
            // 2 バイトの文字の 2 バイト目は 0x40〜0x7E（\ を含む）のこともあるので、1 バイト目の次も含める
            let mut len = 0;
            while let Some(&b) = rest.get(len) {
                if b < 0x80 {
                    break;
                }
                len += if matches!(b, 0x81..=0x9f | 0xe0..=0xfc) {
                    2
                } else {
                    1
                };
            }
            let (run, after) = rest.split_at(len.min(rest.len()));
            let (text, _) = encoding_rs::SHIFT_JIS.decode_without_bom_handling(run);
            out.extend(text.encode_utf16());
            rest = after;
            continue;
        }
        out.push(u16::from(byte));
        rest = tail;
    }
    String::from_utf16_lossy(&out)
}

/// `\X2\` などで始まり、`\X0\` で閉じる 16 進の並び。width 桁ずつの値と、閉じの後
fn hex_run<'a>(rest: &'a [u8], open: &[u8], width: usize) -> Option<(Vec<u32>, &'a [u8])> {
    let after = rest.strip_prefix(open)?;
    let run = after
        .iter()
        .take_while(|byte| byte.is_ascii_hexdigit())
        .count();
    if !run.is_multiple_of(width) {
        return None;
    }
    let (hex, close) = after.split_at(run);
    let close = close.strip_prefix(b"\\X0\\")?;
    Some((hex.chunks(width).map(hex_value).collect(), close))
}

fn hex_value(hex: &[u8]) -> u32 {
    hex.iter().fold(0, |value, byte| {
        value * 16 + char::from(*byte).to_digit(16).unwrap_or_default()
    })
}
