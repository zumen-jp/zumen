//! 文書エンジンと今の実装に、同じ変更を順に流して比べる。
//!
//! 変更は、種を決めた乱数で作る。動かす変更（平行移動・回転・拡大と縮小・鏡映・相似でない変換・恒等・逆のない変換、
//! 1 件から 100 件）、除く変更、属性を変える変更（画層・色・線種・線種の尺度・線の太さ）、足す変更（線・円・円弧・
//! 折れ線・文字・記号）と、種類の違う変更を 1 つの要求に混ぜたもの。ない ID、同じ要素を 2 度、受け付けない値も混ぜる。
//! 1 つ流すごとに、受け付けるかどうかと、保存したバイト列を比べる。足した要素は、今の実装が返した ID と、
//! 文書エンジンが返したハンドルも比べる。
//! 文書エンジンの中では、取り消しとやり直しのバイト列、開き直した文書との一致、範囲で選ぶ結果も確かめる。

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use zumen_bench_live::born::{
    Add, ColorSpec, LineTypeSpec, NewGrid, NewShape, NewSymbol, NewText, Restyle, WeightSpec,
};
use zumen_bench_live::edit::{Decline, Edit};
use zumen_bench_live::lift::Kind;
use zumen_bench_live::store::{self, Charset, Document, table};
use zumen_drawing::Affine;

use crate::random::Random;
use crate::today::{Today, transform_json};

/// 比べ方
pub struct Options {
    /// 流す変更の数。なければ、要素の数から決める
    pub steps: Option<usize>,
    /// 乱数の種
    pub seed: u64,
    /// 文書エンジンが開くときのスレッドの数
    pub threads: usize,
    /// 違いの前後のバイト列を出す（図面の中身が出る。公開できる入力だけで使う）
    pub show: bool,
    /// 今の実装の要求に足す予算（JSON のオブジェクト）
    pub budget: Option<String>,
    /// 今の実装が受け付けた変更のたびに、すべての要素のハッシュを取り直して、変更が指していない要素のハッシュが
    /// 変わっていないかを見る
    pub hashes: bool,
}

/// ハッシュを知らない要素に付ける値
const FALLBACK: &str = "v2:00000000000000000000000000000000";

/// `a` の前に `b` を行う変換
fn compose(a: [f64; 6], b: [f64; 6]) -> [f64; 6] {
    [
        a[0] * b[0] + a[1] * b[3],
        a[0] * b[1] + a[1] * b[4],
        a[0] * b[2] + a[1] * b[5] + a[2],
        a[3] * b[0] + a[4] * b[3],
        a[3] * b[1] + a[4] * b[4],
        a[3] * b[2] + a[4] * b[5] + a[5],
    ]
}

fn id_of(handle: u64) -> String {
    format!("space/0/h{handle:X}")
}

/// モデル空間の、ハンドルで指す要素の ID からハンドルを読む
fn handle_of(id: &str) -> Option<u64> {
    let hex = id.strip_prefix("space/0/h")?;
    let handle = u64::from_str_radix(hex, 16).ok()?;
    (id_of(handle) == id).then_some(handle)
}

fn kind_name(kind: Kind) -> &'static str {
    match kind {
        Kind::Line => "LINE",
        Kind::Circle => "CIRCLE",
        Kind::Arc => "ARC",
        Kind::Polyline => "LWPOLYLINE",
        Kind::Text => "TEXT",
        Kind::Insert => "INSERT",
    }
}

/// JSON の文字列
fn quoted(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for letter in text.chars() {
        match letter {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            letter if u32::from(letter) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", u32::from(letter));
            }
            letter => out.push(letter),
        }
    }
    out.push('"');
    out
}

/// 2 つのバイト列の、最初の違い
fn difference(ours: &[u8], theirs: &[u8], show: bool) -> String {
    let at = ours
        .iter()
        .zip(theirs)
        .position(|(a, b)| a != b)
        .unwrap_or(ours.len().min(theirs.len()));
    let mut text = format!(
        "最初の違いは {at} バイト目（文書エンジン {} バイト、今の実装 {} バイト）",
        ours.len(),
        theirs.len()
    );
    if show {
        let window = |bytes: &[u8]| {
            let part = bytes
                .get(at.saturating_sub(200)..(at + 200).min(bytes.len()))
                .unwrap_or_default();
            String::from_utf8_lossy(part)
                .replace("\r\n", "|")
                .replace('\n', "|")
        };
        text += &format!(
            "\n    文書エンジン: {}\n    今の実装    : {}",
            window(ours),
            window(theirs)
        );
    }
    text
}

/// 変更の種類
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum What {
    Move,
    Remove,
    Restyle,
    Add,
}

impl What {
    fn done(self) -> &'static str {
        match self {
            What::Move => "動かした",
            What::Remove => "除いた",
            What::Restyle => "属性を変えた",
            What::Add => "足した",
        }
    }
}

/// 流す変更の 1 つ。今の実装に渡す JSON と、文書エンジンに渡す値
struct Change {
    json: String,
    edit: Edit,
    /// 指す要素のハンドル。足す変更は持たない
    target: Option<u64>,
    what: What,
}

/// スタイルの項目を JSON に足す
fn style_json(
    out: &mut String,
    layer: Option<&str>,
    color: Option<ColorSpec>,
    line_type: Option<&LineTypeSpec>,
    weight: Option<WeightSpec>,
) {
    if let Some(layer) = layer {
        let _ = write!(out, r#","layer":{}"#, quoted(layer));
    }
    let _ = match color {
        None => Ok(()),
        Some(ColorSpec::ByLayer) => write!(out, r#","color":{{"inherit":"layer"}}"#),
        Some(ColorSpec::ByBlock) => write!(out, r#","color":{{"inherit":"block"}}"#),
        Some(ColorSpec::Aci(aci)) => write!(out, r#","color":{{"aci":{aci}}}"#),
        Some(ColorSpec::Rgb([r, g, b])) => write!(out, r#","color":{{"rgb":[{r},{g},{b}]}}"#),
        Some(ColorSpec::AciRgb(aci, [r, g, b])) => {
            write!(out, r#","color":{{"aci":{aci},"rgb":[{r},{g},{b}]}}"#)
        }
    };
    let _ = match line_type {
        None => Ok(()),
        Some(LineTypeSpec::ByLayer) => write!(out, r#","line_type":{{"inherit":"layer"}}"#),
        Some(LineTypeSpec::ByBlock) => write!(out, r#","line_type":{{"inherit":"block"}}"#),
        Some(LineTypeSpec::Name(name)) => {
            write!(out, r#","line_type":{{"name":{}}}"#, quoted(name))
        }
    };
    let _ = match weight {
        None => Ok(()),
        Some(WeightSpec::ByLayer) => write!(out, r#","line_weight":{{"inherit":"layer"}}"#),
        Some(WeightSpec::ByBlock) => write!(out, r#","line_weight":{{"inherit":"block"}}"#),
        Some(WeightSpec::Default) => write!(out, r#","line_weight":{{"default":true}}"#),
        Some(WeightSpec::Mm(mm)) => write!(out, r#","line_weight":{{"mm":{mm}}}"#),
    };
}

/// 点の項目を JSON に足す
fn pair(out: &mut String, key: &str, [x, y]: [f64; 2]) {
    let _ = write!(out, r#","{key}":[{x},{y}]"#);
}

/// 足す形の JSON
fn shape_json(out: &mut String, shape: &NewShape) {
    match shape {
        NewShape::Line { start, end } => {
            out.push_str(r#"{"shape":"line""#);
            pair(out, "start", *start);
            pair(out, "end", *end);
        }
        NewShape::Circle { center, radius } => {
            out.push_str(r#"{"shape":"circle""#);
            pair(out, "center", *center);
            let _ = write!(out, r#","radius":{radius}"#);
        }
        NewShape::Arc {
            center,
            radius,
            start,
            end,
        } => {
            out.push_str(r#"{"shape":"arc""#);
            pair(out, "center", *center);
            let _ = write!(
                out,
                r#","radius":{radius},"start_angle":{start},"end_angle":{end}"#
            );
        }
        NewShape::Polyline {
            points,
            bulges,
            closed,
        } => {
            out.push_str(r#"{"shape":"polyline","points":["#);
            for (index, [x, y]) in points.iter().enumerate() {
                let comma = if index > 0 { "," } else { "" };
                let _ = write!(out, "{comma}[{x},{y}]");
            }
            out.push(']');
            if let Some(bulges) = bulges {
                out.push_str(r#","bulges":["#);
                for (index, bulge) in bulges.iter().enumerate() {
                    let comma = if index > 0 { "," } else { "" };
                    let _ = write!(out, "{comma}{bulge}");
                }
                out.push(']');
            }
            let _ = write!(out, r#","closed":{closed}"#);
        }
        NewShape::Text(text) => {
            let _ = write!(
                out,
                r#"{{"shape":"text","content":{}"#,
                quoted(&text.content)
            );
            pair(out, "position", text.position);
            if let Some((end, fit)) = text.end {
                pair(out, "end", end);
                let fit = if fit { "fit" } else { "aligned" };
                let _ = write!(out, r#","fit":"{fit}""#);
            }
            if let Some(horizontal) = text.horizontal {
                let name = match horizontal {
                    0 => "left",
                    1 => "center",
                    2 => "right",
                    _ => "middle",
                };
                let _ = write!(out, r#","horizontal":"{name}""#);
            }
            if let Some(vertical) = text.vertical {
                let name = match vertical {
                    0 => "baseline",
                    1 => "bottom",
                    2 => "middle",
                    _ => "top",
                };
                let _ = write!(out, r#","vertical":"{name}""#);
            }
            let _ = write!(
                out,
                r#","height":{},"rotation":{}"#,
                text.height, text.rotation
            );
            if let Some(factor) = text.width_factor {
                let _ = write!(out, r#","width_factor":{factor}"#);
            }
            if let Some(oblique) = text.oblique {
                let _ = write!(out, r#","oblique":{oblique}"#);
            }
            if text.backward {
                out.push_str(r#","backward":true"#);
            }
            if text.upside_down {
                out.push_str(r#","upside_down":true"#);
            }
            if let Some(style) = &text.style {
                let _ = write!(out, r#","style":{}"#, quoted(style));
            }
        }
        NewShape::Symbol(symbol) => {
            let _ = write!(out, r#"{{"shape":"symbol","name":{}"#, quoted(&symbol.name));
            pair(out, "position", symbol.position);
            pair(out, "scale", symbol.scale);
            let _ = write!(out, r#","rotation":{}"#, symbol.rotation);
            if let Some(grid) = &symbol.grid {
                let _ = write!(
                    out,
                    r#","grid":{{"columns":{},"rows":{},"column_spacing":{},"row_spacing":{}}}"#,
                    grid.columns, grid.rows, grid.column_spacing, grid.row_spacing
                );
            }
        }
    }
    out.push('}');
}

/// 足す変更
fn adding(add: Add) -> Change {
    let mut json = String::from(r#"{"kind":"add","place":"space/0""#);
    style_json(
        &mut json,
        add.layer.as_deref(),
        add.color,
        add.line_type.as_ref(),
        add.line_weight,
    );
    json.push_str(r#","geometry":"#);
    shape_json(&mut json, &add.shape);
    json.push('}');
    Change {
        json,
        edit: Edit::Add(Box::new(add)),
        target: None,
        what: What::Add,
    }
}

/// 変更で指す名前。開いたときの図面から集める
#[derive(Default)]
struct Names {
    /// 表に定義のある画層
    layers: Vec<String>,
    /// 図形が指すだけで、表に定義のない画層
    loose_layers: Vec<String>,
    /// 表にある線種
    line_types: Vec<String>,
    /// 表にある文字スタイル
    text_styles: Vec<String>,
    /// ブロックの定義
    blocks: Vec<String>,
}

impl Names {
    fn of(document: &Document) -> Names {
        let text = |raw: &[u8]| String::from_utf8_lossy(raw).into_owned();
        let mut names = Names {
            line_types: document.table_names(table::LTYPE),
            text_styles: document.table_names(table::STYLE),
            ..Names::default()
        };
        for layer in &document.layers.list {
            if layer.record.is_some() {
                names.layers.push(text(&layer.name));
            } else {
                names.loose_layers.push(text(&layer.name));
            }
        }
        for definition in &document.definitions {
            if !definition.name.is_empty() {
                names.blocks.push(text(&definition.name));
            }
        }
        names
    }
}

/// 数えたもの
#[derive(Default)]
struct Tally {
    steps: usize,
    /// 両方が受け付け、バイト列が同じ
    equal: usize,
    /// 両方が受け付けない。文書エンジンの理由と、今の実装の診断ごと
    declined: BTreeMap<String, usize>,
    /// 今の実装だけが受け付けた。この試作が扱わないと決めているもの。理由と、変更の種類ごと
    gaps: BTreeMap<String, usize>,
    /// 違い
    bugs: usize,
    /// 流した変更の種類
    labels: BTreeMap<&'static str, [usize; 2]>,
    /// 変えた要素の数。変更の種類と、要素の種類ごと
    kinds: BTreeMap<(What, &'static str), usize>,
    /// 足した要素の ID を、今の実装のものと比べた回数
    id_checks: usize,
    undo_checks: usize,
    deep_checks: usize,
    /// 開き直した文書と、表示のバッファーが、並びまで同じだった回数
    same_buffers: usize,
    select_checks: usize,
    resyncs: usize,
    /// 今の実装が、覚えているハッシュを古いとして受け付けなかった回数
    stale: usize,
    /// 変更が指していないのに、ハッシュが変わった要素の数
    drifted: usize,
}

struct Run<'a> {
    options: &'a Options,
    today: Today,
    document: Document,
    /// 今の文書のバイト列（今の実装が返したもの）
    current: Vec<u8>,
    /// 文書エンジンの取り消しの列の、始まりのバイト列
    base: Vec<u8>,
    /// 要素のハッシュ（今の実装のもの）
    hashes: BTreeMap<String, String>,
    /// 両方が同じ ID で持つ要素のハンドル
    covered: Vec<u64>,
    /// どちらにもないハンドル
    unknown: u64,
    extent: [f64; 4],
    names: Names,
    /// 直前の平行移動
    last_move: Option<(Vec<u64>, [f64; 2])>,
    /// 終わりに近い変更か。UTF-8 でない図面に ASCII でない文字を足すのは、ここからにする
    /// （今の実装が足した後は、開き直した文書の文字コードが変わり、後ろの変更の多くが範囲の外になる）
    late: bool,
    random: Random,
    tally: Tally,
}

impl Run<'_> {
    fn bug(&mut self, step: usize, label: &str, text: String) {
        self.tally.bugs += 1;
        if self.tally.bugs <= 12 {
            println!("  違い（{step} 番目・{label}）: {text}");
        }
    }

    /// 文書エンジンを、今の実装のバイト列から開き直す
    fn resync(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.document = Document::open(bytes.to_vec(), self.options.threads)
            .map_err(|error| format!("今の実装が書いたファイルを開けない: {error:?}"))?;
        self.base = bytes.to_vec();
        let document = &self.document;
        self.covered
            .retain(|handle| document.row_of(*handle).is_some());
        self.tally.resyncs += 1;
        Ok(())
    }

    /// 今の実装が受け付けた変更から、要素のハッシュと、指せる要素を直す
    fn learn(&mut self, changes: &[Change], hashes: Vec<(String, String)>) {
        let mut removed = BTreeSet::new();
        for (change, (id, hash)) in changes.iter().zip(hashes) {
            match change.what {
                What::Remove => {
                    if let Some(handle) = change.target {
                        removed.insert(handle);
                        self.hashes.remove(&id_of(handle));
                    }
                }
                What::Add => {
                    if let Some(handle) = handle_of(&id)
                        && self.document.row_of(handle).is_some()
                    {
                        self.covered.push(handle);
                    }
                    if !id.is_empty() {
                        self.hashes.insert(id, hash);
                    }
                }
                What::Move | What::Restyle => {
                    if !id.is_empty() {
                        self.hashes.insert(id, hash);
                    }
                }
            }
        }
        if !removed.is_empty() {
            self.covered.retain(|handle| !removed.contains(handle));
        }
    }

    fn hash_of(&self, id: &str) -> &str {
        self.hashes.get(id).map_or(FALLBACK, String::as_str)
    }

    /// 今の実装の要素のハッシュを取り直す。覚えていた値と違う要素（変更が指していないのに変わったもの）の数を返す
    fn refresh_hashes(&mut self, step: usize, label: &str) -> Result<usize, String> {
        let elements = self
            .today
            .elements()
            .map_err(|failure| format!("今の実装が要素を返さない: {}", failure.describe()))?;
        let mut drifted = Vec::new();
        for element in elements {
            if self.hashes.get(&element.id) != Some(&element.hash) {
                if self.hashes.contains_key(&element.id) {
                    drifted.push(element.id.clone());
                }
                self.hashes.insert(element.id, element.hash);
            }
        }
        if !drifted.is_empty() {
            self.tally.drifted += drifted.len();
            if self.options.show {
                println!(
                    "  指していない要素のハッシュが変わった（{step} 番目・{label}）: {} 件（最初は {}）",
                    drifted.len(),
                    drifted.first().map_or("", String::as_str)
                );
            }
        }
        Ok(drifted.len())
    }

    /// 動かす変更
    fn moving(&self, handle: u64, matrix: [f64; 6]) -> Change {
        let id = id_of(handle);
        Change {
            json: transform_json(&id, self.hash_of(&id), &matrix),
            edit: Edit::Transform { handle, matrix },
            target: Some(handle),
            what: What::Move,
        }
    }

    /// 除く変更
    fn removing(&self, handle: u64) -> Change {
        let id = id_of(handle);
        Change {
            json: format!(
                r#"{{"kind":"remove","id":"{id}","hash":"{}"}}"#,
                self.hash_of(&id)
            ),
            edit: Edit::Remove { handle },
            target: Some(handle),
            what: What::Remove,
        }
    }

    /// 属性を変える変更
    fn restyling(&self, handle: u64, style: &Restyle) -> Change {
        let id = id_of(handle);
        let mut json = format!(
            r#"{{"kind":"restyle","id":"{id}","hash":"{}""#,
            self.hash_of(&id)
        );
        style_json(
            &mut json,
            style.layer.as_deref(),
            style.color,
            style.line_type.as_ref(),
            style.line_weight,
        );
        if let Some(scale) = style.line_type_scale {
            let _ = write!(json, r#","line_type_scale":{scale}"#);
        }
        json.push('}');
        Change {
            json,
            edit: Edit::Restyle {
                handle,
                style: style.clone(),
            },
            target: Some(handle),
            what: What::Restyle,
        }
    }

    fn span(&self) -> f64 {
        let [x0, y0, x1, y1] = self.extent;
        (x1 - x0).max(y1 - y0).max(1.0)
    }

    /// 図面の範囲の中の点
    fn point(&mut self) -> [f64; 2] {
        let [x0, y0, x1, y1] = self.extent;
        let x = self.random.between(x0, x1);
        let y = self.random.between(y0, y1);
        [self.random.rounded(x), self.random.rounded(y)]
    }

    /// 変換を 1 つ作る
    fn matrix(&mut self) -> (&'static str, [f64; 6]) {
        let [x0, y0, x1, y1] = self.extent;
        let span = self.span();
        let random = &mut self.random;
        let x = random.between(x0, x1);
        let y = random.between(y0, y1);
        let (x, y) = (random.rounded(x), random.rounded(y));
        let about = |m: [f64; 4]| {
            [
                m[0],
                m[1],
                x - m[0] * x - m[1] * y,
                m[2],
                m[3],
                y - m[2] * x - m[3] * y,
            ]
        };
        match random.below(100) {
            0..=36 => {
                let dx = random.between(-span, span) / 8.0;
                let dy = random.between(-span, span) / 8.0;
                let (dx, dy) = (random.rounded(dx), random.rounded(dy));
                ("平行移動", [1.0, 0.0, dx, 0.0, 1.0, dy])
            }
            37..=39 => {
                let dx = random.between(-1.0, 1.0) * 1e-7;
                let dy = random.between(-1.0, 1.0) * 1e9;
                ("平行移動", [1.0, 0.0, dx, 0.0, 1.0, dy])
            }
            40..=54 => {
                let turn = random.between(0.0, 360.0);
                let degrees = random.pick(&[90.0, 180.0, 270.0, -90.0, 45.0, 30.0, turn]);
                ("回転", Affine::rotation(x, y, degrees).m)
            }
            55..=64 => {
                let free = random.between(0.25, 4.0);
                let scale = random.pick(&[2.0, 0.5, 1.5, 0.75, 3.0, 1.0 / 3.0, free]);
                ("拡大と縮小", about([scale, 0.0, 0.0, scale]))
            }
            65..=72 => match random.below(3) {
                0 => ("鏡映", about([-1.0, 0.0, 0.0, 1.0])),
                1 => ("鏡映", about([1.0, 0.0, 0.0, -1.0])),
                _ => ("鏡映", about([0.0, 1.0, 1.0, 0.0])),
            },
            73..=79 => {
                let degrees = random.between(0.0, 360.0);
                let scale = random.between(0.5, 2.0);
                let dx = random.between(-span, span) / 8.0;
                let turn = Affine::rotation(x, y, degrees).m;
                let grow = about([scale, 0.0, 0.0, scale]);
                let shift = [1.0, 0.0, dx, 0.0, 1.0, -dx];
                ("相似", compose(shift, compose(turn, grow)))
            }
            80..=86 => {
                let sx = random.pick(&[2.0, 0.5, 1.0, 1.25]);
                let sy = random.pick(&[3.0, 0.25, 1.5, 0.8]);
                ("縦横で違う拡大", about([sx, 0.0, 0.0, sy]))
            }
            87..=90 => {
                let slope = random.pick(&[0.5, -0.25, 1.0]);
                ("せん断", about([1.0, slope, 0.0, 1.0]))
            }
            91..=95 => ("恒等", [1.0, 0.0, 0.0, 0.0, 1.0, 0.0]),
            _ => match random.below(3) {
                0 => ("逆のない変換", [1.0, 0.0, 0.0, 0.0, 0.0, 0.0]),
                1 => ("逆のない変換", [0.0; 6]),
                _ => ("逆のない変換", [1.0, 2.0, 3.0, 2.0, 4.0, 5.0]),
            },
        }
    }

    /// 指せる要素から、重ならないように `count` 件までを選ぶ
    fn targets(&mut self, count: usize) -> Vec<u64> {
        let count = count.min(self.covered.len());
        let mut chosen = BTreeSet::new();
        let mut targets = Vec::with_capacity(count + 1);
        while targets.len() < count {
            let at = self.random.below(self.covered.len());
            if chosen.insert(at)
                && let Some(handle) = self.covered.get(at)
            {
                targets.push(*handle);
            }
        }
        targets
    }

    /// 画層の名前。`None` は、書かない
    fn layer(&mut self) -> Option<String> {
        let names = &self.names;
        match self.random.below(100) {
            0..=34 => None,
            35..=82 => self.random.choose(&names.layers).cloned(),
            83..=88 => self.random.choose(&names.loose_layers).cloned(),
            89..=92 => Some("0".to_owned()),
            93..=95 => Some("no such layer".to_owned()),
            // 大文字と小文字の違う綴り
            _ => self.random.choose(&names.layers).map(|name| {
                if name.to_uppercase() == *name {
                    name.to_lowercase()
                } else {
                    name.to_uppercase()
                }
            }),
        }
    }

    fn rgb(&mut self) -> [u8; 3] {
        let bits = self.random.next();
        [bits as u8, (bits >> 8) as u8, (bits >> 16) as u8]
    }

    /// 色。`None` は、書かない
    fn color(&mut self) -> Option<ColorSpec> {
        match self.random.below(100) {
            0..=39 => None,
            40..=49 => Some(ColorSpec::ByLayer),
            50..=54 => Some(ColorSpec::ByBlock),
            55..=79 => Some(ColorSpec::Aci(1 + self.random.below(255) as u8)),
            80..=87 => Some(ColorSpec::Rgb(self.rgb())),
            88..=95 => {
                let aci = 1 + self.random.below(255) as u8;
                Some(ColorSpec::AciRgb(aci, self.rgb()))
            }
            96..=97 => Some(ColorSpec::Aci(0)),
            _ => Some(ColorSpec::AciRgb(0, self.rgb())),
        }
    }

    /// 線種。`None` は、書かない
    fn line_type(&mut self) -> Option<LineTypeSpec> {
        let name = |name: &str| Some(LineTypeSpec::Name(name.to_owned()));
        match self.random.below(100) {
            0..=49 => None,
            50..=59 => Some(LineTypeSpec::ByLayer),
            60..=64 => Some(LineTypeSpec::ByBlock),
            65..=89 => self
                .random
                .choose(&self.names.line_types)
                .cloned()
                .map(LineTypeSpec::Name),
            90..=93 => name("Continuous"),
            94..=95 => name("ByLayer"),
            96..=97 => name("BYBLOCK"),
            _ => name("no such line type"),
        }
    }

    /// 線の太さ。`None` は、書かない
    fn weight(&mut self) -> Option<WeightSpec> {
        match self.random.below(100) {
            0..=54 => None,
            55..=64 => Some(WeightSpec::ByLayer),
            65..=69 => Some(WeightSpec::ByBlock),
            70..=77 => Some(WeightSpec::Default),
            78..=93 => Some(WeightSpec::Mm(self.random.pick(&[
                0.0, 0.05, 0.09, 0.13, 0.15, 0.18, 0.2, 0.25, 0.3, 0.35, 0.4, 0.5, 0.53, 0.6, 0.7,
                0.8, 0.9, 1.0, 1.06, 1.2, 1.4, 1.58, 2.0, 2.11,
            ]))),
            // DXF の太さの表にない値
            94..=96 => Some(WeightSpec::Mm(
                self.random.pick(&[0.123, 0.07, 2.5, 3.0, 0.255]),
            )),
            _ => Some(WeightSpec::Mm(-0.25)),
        }
    }

    /// 属性を変える変更の中身
    fn restyle(&mut self) -> Restyle {
        let line_type_scale = match self.random.below(100) {
            0..=69 => None,
            70..=79 => Some(1.0),
            80..=93 => Some(self.random.pick(&[0.5, 2.0, 0.25, 10.0, 1.5, 0.001])),
            94..=96 => Some(self.random.between(0.01, 5.0)),
            97 => Some(0.0),
            _ => Some(-1.0),
        };
        Restyle {
            layer: self.layer(),
            color: self.color(),
            line_type: self.line_type(),
            line_type_scale,
            line_weight: self.weight(),
        }
    }

    /// 足す文字
    fn new_text(&mut self) -> NewText {
        let span = self.span();
        let position = self.point();
        let content = match self.random.below(12) {
            0 => "A".to_owned(),
            1 if self.late || self.document.charset == Charset::Utf8 => "図面 1".to_owned(),
            1 => "PLAN 1".to_owned(),
            2 => "%%c30 \\U+00B1".to_owned(),
            3 => "  spaced  ".to_owned(),
            4 => "quote\"back\\slash".to_owned(),
            5 => "caret ^ here".to_owned(),
            6 => String::new(),
            _ => format!("TEXT {}", self.random.below(1000)),
        };
        let free = self.random.between(0.1, span / 20.0 + 0.2);
        let height = self.random.pick(&[2.5, 3.5, 5.0, 0.18, 10.0, free]);
        let turn = self.random.between(0.0, 360.0);
        let mut rotation = self
            .random
            .pick(&[0.0, 0.0, 0.0, 90.0, 45.0, 180.0, -30.0, 360.0, turn]);
        let (mut end, mut horizontal, mut vertical) = (None, None, None);
        match self.random.below(10) {
            0..=1 => {
                let reach = self.random.pick(&[10.0, 25.0, 0.5, span / 7.0]);
                let [dx, dy] = match self.random.below(4) {
                    0 => [reach, 0.0],
                    1 => [0.0, reach],
                    2 => [-reach, 0.0],
                    _ => [reach * 0.6, reach * 0.8],
                };
                rotation = libm::atan2(dy, dx).to_degrees();
                end = Some((
                    [position[0] + dx, position[1] + dy],
                    self.random.below(2) == 0,
                ));
            }
            2..=4 => {
                horizontal = Some(self.random.pick(&[0.0, 1.0, 2.0, 4.0]) as u8);
                vertical = Some(self.random.below(4) as u8);
            }
            5 => horizontal = Some(self.random.pick(&[1.0, 2.0, 4.0]) as u8),
            6 => vertical = Some(1 + self.random.below(3) as u8),
            _ => {}
        }
        let width_factor =
            (self.random.below(5) == 0).then(|| self.random.pick(&[1.0, 0.8, 1.5, 0.5]));
        let oblique = (self.random.below(6) == 0).then(|| self.random.pick(&[0.0, 15.0, -10.0]));
        let style = match self.random.below(100) {
            0..=29 => self.random.choose(&self.names.text_styles).cloned(),
            30..=35 => Some("Standard".to_owned()),
            36..=37 => Some("no such style".to_owned()),
            _ => None,
        };
        NewText {
            content,
            position,
            end,
            horizontal,
            vertical,
            height,
            rotation,
            width_factor,
            oblique,
            backward: self.random.below(20) == 0,
            upside_down: self.random.below(20) == 0,
            style,
        }
    }

    /// 足す記号
    fn new_symbol(&mut self) -> NewSymbol {
        // レイアウトと無名のブロック（`*` で始まる名前）は、少なめに選ぶ
        let named: Vec<&String> = self
            .names
            .blocks
            .iter()
            .filter(|name| !name.starts_with('*'))
            .collect();
        let name = match self.random.below(100) {
            0..=74 if !named.is_empty() => named
                .get(self.random.below(named.len()))
                .map(|name| (*name).clone()),
            0..=89 => self.random.choose(&self.names.blocks).cloned(),
            90..=94 => Some("*Model_Space".to_owned()),
            _ => None,
        }
        .unwrap_or_else(|| "no such block".to_owned());
        let scale = match self.random.below(10) {
            0..=3 => [1.0, 1.0],
            4 => [2.0, 2.0],
            5 => [0.5, 0.5],
            6 => [-1.0, 1.0],
            7 => [1.0, -1.0],
            8 => [2.0, 3.0],
            _ => [-2.0, -2.0],
        };
        let turn = self.random.between(0.0, 360.0);
        let rotation = self.random.pick(&[0.0, 0.0, 90.0, 180.0, -45.0, turn]);
        let grid = (self.random.below(6) == 0).then(|| {
            let columns = 1 + self.random.below(4) as u32;
            let rows = if columns == 1 {
                2 + self.random.below(3) as u32
            } else {
                1 + self.random.below(3) as u32
            };
            NewGrid {
                columns,
                rows,
                column_spacing: self.random.pick(&[10.0, 2.5, 0.0, -4.0]),
                row_spacing: self.random.pick(&[10.0, 7.5, 0.0]),
            }
        });
        NewSymbol {
            name,
            position: self.point(),
            scale,
            rotation,
            grid,
        }
    }

    /// 今の実装が受け付けないはずの形
    fn bad_shape(&mut self) -> NewShape {
        let center = self.point();
        match self.random.below(14) {
            0 => NewShape::Circle {
                center,
                radius: 0.0,
            },
            1 => NewShape::Circle {
                center,
                radius: -2.0,
            },
            2 => NewShape::Arc {
                center,
                radius: 0.0,
                start: 0.0,
                end: 90.0,
            },
            3 => NewShape::Polyline {
                points: vec![center],
                bulges: None,
                closed: false,
            },
            4 => NewShape::Polyline {
                points: vec![center, self.point(), self.point()],
                bulges: Some(vec![0.5]),
                closed: true,
            },
            5 => NewShape::Text(NewText {
                content: "two\nlines".to_owned(),
                ..self.new_text()
            }),
            6 => NewShape::Text(NewText {
                height: 0.0,
                ..self.new_text()
            }),
            7 => NewShape::Text(NewText {
                width_factor: Some(0.0),
                ..self.new_text()
            }),
            // 2 点の間に置く文字の 2 点が同じ
            8 => NewShape::Text(NewText {
                end: Some((center, false)),
                position: center,
                horizontal: None,
                vertical: None,
                rotation: 0.0,
                ..self.new_text()
            }),
            // 2 点の間に置く文字の向きが、2 点の向きと違う
            9 => NewShape::Text(NewText {
                end: Some(([center[0] + 10.0, center[1]], true)),
                position: center,
                horizontal: None,
                vertical: None,
                rotation: 30.0,
                ..self.new_text()
            }),
            // 2 点の間に置く文字に、位置合わせを書く
            10 => NewShape::Text(NewText {
                end: Some(([center[0] + 10.0, center[1]], true)),
                position: center,
                horizontal: Some(1),
                vertical: None,
                rotation: 0.0,
                ..self.new_text()
            }),
            11 => NewShape::Symbol(NewSymbol {
                scale: [0.0, 1.0],
                ..self.new_symbol()
            }),
            12 => NewShape::Symbol(NewSymbol {
                grid: Some(NewGrid {
                    columns: 1,
                    rows: 1,
                    column_spacing: 5.0,
                    row_spacing: 5.0,
                }),
                ..self.new_symbol()
            }),
            _ => NewShape::Symbol(NewSymbol {
                name: "no such block".to_owned(),
                ..self.new_symbol()
            }),
        }
    }

    /// 足す形
    fn shape(&mut self) -> (&'static str, NewShape) {
        let span = self.span();
        let size = {
            let free = self.random.between(span / 400.0, span / 12.0);
            let rounded = self.random.rounded(free);
            if rounded > 0.0 { rounded } else { free }
        };
        match self.random.below(100) {
            0..=13 => (
                "線を足す",
                NewShape::Line {
                    start: self.point(),
                    end: self.point(),
                },
            ),
            14..=25 => (
                "円を足す",
                NewShape::Circle {
                    center: self.point(),
                    radius: size,
                },
            ),
            26..=37 => {
                let from = self.random.between(0.0, 360.0);
                let to = self.random.between(0.0, 360.0);
                let start = self
                    .random
                    .pick(&[0.0, 90.0, 180.0, 270.0, -90.0, 45.0, from]);
                let end = self
                    .random
                    .pick(&[360.0, 90.0, 180.0, 0.0, 450.0, 30.0, to]);
                (
                    "円弧を足す",
                    NewShape::Arc {
                        center: self.point(),
                        radius: size,
                        start,
                        end,
                    },
                )
            }
            38..=51 => {
                let count = 2 + self.random.below(6);
                let points: Vec<[f64; 2]> = (0..count).map(|_| self.point()).collect();
                let bulges = (self.random.below(2) == 0).then(|| {
                    (0..count)
                        .map(|_| self.random.pick(&[0.0, 0.0, 0.5, -1.0, 1.0, 0.25]))
                        .collect()
                });
                (
                    "折れ線を足す",
                    NewShape::Polyline {
                        points,
                        bulges,
                        closed: self.random.below(3) == 0,
                    },
                )
            }
            52..=67 => ("文字を足す", NewShape::Text(self.new_text())),
            68..=94 if !self.names.blocks.is_empty() => {
                ("記号を足す", NewShape::Symbol(self.new_symbol()))
            }
            68..=94 => (
                "線を足す",
                NewShape::Line {
                    start: self.point(),
                    end: self.point(),
                },
            ),
            _ => ("受け付けない値で足す", self.bad_shape()),
        }
    }

    /// 足す要素
    fn add(&mut self) -> (&'static str, Add) {
        let (label, shape) = self.shape();
        let add = Add {
            layer: self.layer(),
            color: self.color(),
            line_type: self.line_type(),
            line_weight: self.weight(),
            shape,
        };
        (label, add)
    }

    /// 動かす変更の要求
    fn draw_moves(&mut self) -> (&'static str, Vec<Change>) {
        let count = match self.random.below(100) {
            0..=77 => 1,
            78..=92 => 2 + self.random.below(7),
            _ => 9 + self.random.below(92),
        };
        let mut targets = self.targets(count);
        let (mut label, first) = self.matrix();
        let mut matrices = vec![first; targets.len()];
        if targets.len() > 1 && self.random.below(100) < 30 {
            for matrix in &mut matrices {
                *matrix = self.matrix().1;
            }
            label = "要素ごとに違う変換";
        }
        match self.random.below(100) {
            0..=1 => {
                targets.push(self.unknown);
                matrices.push(first);
                label = "ない ID";
            }
            2..=3 => {
                if let Some(again) = targets.first().copied() {
                    targets.push(again);
                    matrices.push(first);
                    label = "同じ要素を 2 度";
                }
            }
            _ => {}
        }
        if label == "平行移動" {
            self.last_move = Some((targets.clone(), [first[2], first[5]]));
        }
        let changes = targets
            .iter()
            .zip(&matrices)
            .map(|(handle, matrix)| self.moving(*handle, *matrix))
            .collect();
        (label, changes)
    }

    /// 除く変更の要求
    fn draw_removes(&mut self) -> (&'static str, Vec<Change>) {
        let count = match self.random.below(100) {
            0..=71 => 1,
            72..=94 => 2 + self.random.below(4),
            _ => 6 + self.random.below(self.covered.len() / 20 + 1).min(40),
        };
        let mut targets = self.targets(count);
        let mut label = "除く";
        match self.random.below(100) {
            0..=2 => {
                targets.push(self.unknown);
                label = "ない ID を除く";
            }
            3..=5 => {
                if let Some(again) = targets.first().copied() {
                    targets.push(again);
                    label = "同じ要素を 2 度除く";
                }
            }
            _ => {}
        }
        let changes = targets
            .iter()
            .map(|handle| self.removing(*handle))
            .collect();
        (label, changes)
    }

    /// 属性を変える変更の要求
    fn draw_restyles(&mut self) -> (&'static str, Vec<Change>) {
        let count = match self.random.below(100) {
            0..=74 => 1,
            _ => 2 + self.random.below(7),
        };
        let mut targets = self.targets(count);
        let mut label = "属性を変える";
        let shared = self.restyle();
        let mut styles = vec![shared.clone(); targets.len()];
        if targets.len() > 1 && self.random.below(2) == 0 {
            for style in &mut styles {
                *style = self.restyle();
            }
        }
        match self.random.below(100) {
            0..=2 => {
                targets.push(self.unknown);
                styles.push(shared);
                label = "ない ID の属性を変える";
            }
            3..=5 => {
                if let Some(again) = targets.first().copied() {
                    targets.push(again);
                    styles.push(shared);
                    label = "同じ要素の属性を 2 度変える";
                }
            }
            _ => {}
        }
        let changes = targets
            .iter()
            .zip(&styles)
            .map(|(handle, style)| self.restyling(*handle, style))
            .collect();
        (label, changes)
    }

    /// 足す変更の要求
    fn draw_adds(&mut self) -> (&'static str, Vec<Change>) {
        let count = match self.random.below(100) {
            0..=77 => 1,
            _ => 2 + self.random.below(5),
        };
        let mut label = "いくつかを足す";
        let mut changes = Vec::with_capacity(count);
        for _ in 0..count {
            let (one, add) = self.add();
            if count == 1 {
                label = one;
            }
            changes.push(adding(add));
        }
        (label, changes)
    }

    /// 種類の違う変更を混ぜた要求
    fn draw_mixed(&mut self) -> (&'static str, Vec<Change>) {
        let count = 2 + self.random.below(5);
        let targets = self.targets(count);
        let mut label = "種類の違う変更を混ぜる";
        let mut changes = Vec::with_capacity(count + 1);
        for at in 0..count {
            let target = targets.get(at).copied();
            let change = match (self.random.below(100), target) {
                (0..=37, Some(handle)) => {
                    let matrix = self.matrix().1;
                    self.moving(handle, matrix)
                }
                (38..=61, Some(handle)) => {
                    let style = self.restyle();
                    self.restyling(handle, &style)
                }
                (62..=77, Some(handle)) => self.removing(handle),
                _ => adding(self.add().1),
            };
            changes.push(change);
        }
        if self.random.below(100) < 8
            && let Some(handle) = targets.first().copied()
        {
            // すでに指した要素を、もう 1 度、別の種類の変更で指す
            let change = match self.random.below(3) {
                0 => self.removing(handle),
                1 => self.moving(handle, [1.0, 0.0, 1.0, 0.0, 1.0, 1.0]),
                _ => {
                    let style = self.restyle();
                    self.restyling(handle, &style)
                }
            };
            changes.push(change);
            label = "同じ要素に種類の違う変更";
        }
        (label, changes)
    }

    /// 変更の要求を 1 つ作る
    fn draw(&mut self) -> (&'static str, Vec<Change>) {
        if let Some((targets, [dx, dy])) = self.last_move.take()
            && self.random.below(100) < 30
        {
            let back = [1.0, 0.0, -dx, 0.0, 1.0, -dy];
            let changes = targets
                .iter()
                .map(|handle| self.moving(*handle, back))
                .collect();
            return ("平行移動を戻す", changes);
        }
        match self.random.below(100) {
            _ if self.covered.is_empty() => self.draw_adds(),
            0..=15 => self.draw_adds(),
            16..=27 => self.draw_removes(),
            28..=41 => self.draw_restyles(),
            42..=47 => self.draw_mixed(),
            _ => self.draw_moves(),
        }
    }

    /// 取り消しとやり直しのバイト列を確かめる。`expected` は、変更の後のバイト列。
    /// `deep` なら、取り消した所で、開き直した文書とも比べる
    fn check_undo(
        &mut self,
        step: usize,
        label: &str,
        expected: &[u8],
        deep: bool,
    ) -> Result<(), String> {
        self.tally.undo_checks += 1;
        if !self.document.undo() {
            self.bug(step, label, "取り消せない".to_owned());
            return Ok(());
        }
        let back = self.document.save();
        if back != self.current {
            let text = difference(&back, &self.current, self.options.show);
            self.bug(
                step,
                label,
                format!("取り消した後のバイト列が、変更の前と違う。{text}"),
            );
        }
        if deep {
            self.check_deep(step, "取り消した後")?;
        }
        if !self.document.redo() {
            self.bug(step, label, "やり直せない".to_owned());
            return Ok(());
        }
        let again = self.document.save();
        if again != expected {
            let text = difference(&again, expected, self.options.show);
            self.bug(
                step,
                label,
                format!("やり直した後のバイト列が、変更の後と違う。{text}"),
            );
        }
        Ok(())
    }

    /// 文書エンジンの中の値を、保存したバイト列から開き直した文書と比べ、範囲で選ぶ結果を総当たりと比べる
    fn check_deep(&mut self, step: usize, label: &str) -> Result<(), String> {
        self.tally.deep_checks += 1;
        let reopened = Document::open(self.document.save(), self.options.threads)
            .map_err(|error| format!("保存したファイルを開けない: {error:?}"))?;
        if !self.document.same_content(&reopened) {
            self.bug(step, label, "開き直した文書と、要素の列が違う".to_owned());
        }
        if self.document.live_display() != reopened.live_display() {
            self.bug(
                step,
                label,
                "開き直した文書と、表示のバッファーが違う".to_owned(),
            );
        }
        if self.document.display == reopened.display {
            self.tally.same_buffers += 1;
        }
        let bits = |boxes: Vec<[f64; 4]>| -> Vec<[u64; 4]> {
            boxes.iter().map(|item| item.map(f64::to_bits)).collect()
        };
        if bits(self.document.live_boxes()) != bits(reopened.live_boxes()) {
            self.bug(step, label, "開き直した文書と、要素の箱が違う".to_owned());
        }
        let handles = |document: &Document, rows: &[u32]| -> Vec<u64> {
            rows.iter()
                .filter_map(|row| document.rows.get(*row as usize))
                .map(|row| row.handle)
                .collect()
        };
        let [x0, y0, x1, y1] = self.extent;
        let span = self.span();
        let mut found = Vec::new();
        let mut other = Vec::new();
        for _ in 0..24 {
            let x = self.random.between(x0 - span * 0.1, x1 + span * 0.1);
            let y = self.random.between(y0 - span * 0.1, y1 + span * 0.1);
            let size = span * self.random.pick(&[0.001, 0.01, 0.1, 0.5]);
            let rect = [
                x,
                y,
                x + size * self.random.unit(),
                y + size * self.random.unit(),
            ];
            found.clear();
            self.document.select(rect, &mut found);
            let brute: Vec<u32> = (0u32..)
                .zip(&self.document.boxes)
                .filter(|(_, item)| {
                    item[0] <= item[2]
                        && item[1] <= item[3]
                        && item[0] <= rect[2]
                        && item[2] >= rect[0]
                        && item[1] <= rect[3]
                        && item[3] >= rect[1]
                })
                .map(|(row, _)| row)
                .collect();
            self.tally.select_checks += 1;
            if found != brute {
                self.bug(
                    step,
                    label,
                    format!(
                        "範囲で選ぶ結果が総当たりと違う（{} 件と {} 件）",
                        found.len(),
                        brute.len()
                    ),
                );
            }
            other.clear();
            reopened.select(rect, &mut other);
            if handles(&self.document, &found) != handles(&reopened, &other) {
                self.bug(
                    step,
                    label,
                    format!(
                        "範囲で選ぶ結果が、開き直した文書と違う（{} 件と {} 件）",
                        found.len(),
                        other.len()
                    ),
                );
            }
        }
        Ok(())
    }

    /// 要素の種類
    fn kind_of(&self, handle: u64) -> Option<&'static str> {
        let row = self.document.row_of(handle)?;
        Some(kind_name(self.document.rows.get(row as usize)?.kind))
    }

    /// 足した要素の、今の実装が返した ID と、文書エンジンが返したハンドルを比べる
    fn check_ids(
        &mut self,
        step: usize,
        label: &str,
        changes: &[Change],
        theirs: &[(String, String)],
        ours: &[Option<u64>],
    ) {
        if theirs.len() != changes.len() || ours.len() != changes.len() {
            self.bug(
                step,
                label,
                format!(
                    "変更 {} 個に、今の実装の結果 {} 個、文書エンジンの結果 {} 個",
                    changes.len(),
                    theirs.len(),
                    ours.len()
                ),
            );
            return;
        }
        for ((change, (id, _)), handle) in changes.iter().zip(theirs).zip(ours) {
            let same = match (change.what, handle) {
                (What::Add, Some(handle)) => {
                    self.tally.id_checks += 1;
                    id_of(*handle) == *id
                }
                (What::Add, None) => false,
                (What::Remove, None) => id.is_empty(),
                (_, None) => change.target.map(id_of).as_deref() == Some(id.as_str()),
                (_, Some(_)) => false,
            };
            if !same {
                self.bug(
                    step,
                    label,
                    format!(
                        "変えた要素の ID が違う（今の実装 {id:?}、文書エンジン {:?}）",
                        handle.map(id_of)
                    ),
                );
            }
        }
    }

    fn step(&mut self, step: usize, deep: bool) -> Result<(), String> {
        let (label, changes) = self.draw();
        let requests: Vec<String> = changes.iter().map(|change| change.json.clone()).collect();
        let edits: Vec<Edit> = changes.iter().map(|change| change.edit.clone()).collect();
        // 変える前の、指す要素の種類
        let before: Vec<Option<&'static str>> = changes
            .iter()
            .map(|change| change.target.and_then(|handle| self.kind_of(handle)))
            .collect();
        let theirs = self.today.edit(&requests);
        let ours = self.document.apply(&edits);
        self.tally.steps += 1;
        let accepted = usize::from(theirs.is_ok());
        if let Some(count) = self
            .tally
            .labels
            .entry(label)
            .or_default()
            .get_mut(accepted)
        {
            *count += 1;
        }
        match (theirs, ours) {
            (Ok(applied), Ok(handles)) => {
                let expected = applied.file.unwrap_or_else(|| self.current.clone());
                let saved = self.document.save();
                if saved == expected {
                    self.tally.equal += 1;
                    self.check_ids(step, label, &changes, &applied.hashes, &handles);
                    self.check_undo(step, label, &expected, deep)?;
                    for ((change, kind), handle) in changes.iter().zip(&before).zip(&handles) {
                        let kind = match change.what {
                            What::Add => handle.and_then(|handle| self.kind_of(handle)),
                            _ => *kind,
                        };
                        if let Some(kind) = kind {
                            *self.tally.kinds.entry((change.what, kind)).or_default() += 1;
                        }
                    }
                } else {
                    let text = difference(&saved, &expected, self.options.show);
                    self.bug(step, label, format!("保存したバイト列が違う。{text}"));
                    if self.options.show {
                        println!("    要求: {}", requests.join(","));
                    }
                    self.resync(&expected)?;
                }
                self.learn(&changes, applied.hashes);
                self.current = expected;
                if self.options.hashes && self.refresh_hashes(step, label)? > 0 && self.options.show
                {
                    println!("    要求: {}", requests.join(","));
                }
            }
            (Err(failure), Err(decline)) => {
                // `--show` では、変更の種類ごとに分け、最初の要求を出す
                let key = if self.options.show {
                    format!("{decline:?} ／ {}（{label}）", failure.reason())
                } else {
                    format!("{decline:?} ／ {}", failure.reason())
                };
                let count = self.tally.declined.entry(key.clone()).or_default();
                *count += 1;
                if self.options.show && *count <= 2 {
                    println!("  両方が受け付けない（{step} 番目）: {key}");
                    println!("    要求: {}", requests.join(","));
                }
            }
            (Ok(applied), Err(decline)) => {
                let expected = applied.file.unwrap_or_else(|| self.current.clone());
                let outside = matches!(decline, Decline::Unreadable | Decline::Outside(_));
                if outside {
                    let key = format!("{decline:?}（{label}）");
                    *self.tally.gaps.entry(key).or_default() += 1;
                } else {
                    self.bug(
                        step,
                        label,
                        format!(
                            "今の実装は受け付けたが、文書エンジンは受け付けない（{decline:?}）"
                        ),
                    );
                    if self.options.show {
                        println!("    要求: {}", requests.join(","));
                    }
                }
                self.resync(&expected)?;
                self.learn(&changes, applied.hashes);
                self.current = expected;
                if self.options.hashes && self.refresh_hashes(step, label)? > 0 && self.options.show
                {
                    println!("    要求: {}", requests.join(","));
                }
            }
            (Err(failure), Ok(_)) if failure.reason().contains("change.element.stale") => {
                // 覚えているハッシュが古い。今の実装が、前の変更で、指していない要素のハッシュを変えた。
                // 同じ変更を比べたことにならないので、ハッシュを取り直して、次へ進む
                self.tally.stale += 1;
                self.document.undo();
                if self.document.save() != self.current {
                    let current = self.current.clone();
                    self.resync(&current)?;
                }
                if self.refresh_hashes(step, label)? == 0 {
                    self.bug(
                        step,
                        label,
                        "今の実装はハッシュが古いとしたが、取り直したハッシュは同じ".to_owned(),
                    );
                }
            }
            (Err(failure), Ok(_)) => {
                self.bug(
                    step,
                    label,
                    format!(
                        "今の実装は受け付けない（{}）が、文書エンジンは受け付けた",
                        failure.describe()
                    ),
                );
                if self.options.show {
                    println!("    要求: {}", requests.join(","));
                }
                self.document.undo();
                if self.document.save() != self.current {
                    let current = self.current.clone();
                    self.resync(&current)?;
                }
            }
        }
        Ok(())
    }

    /// すべて取り消して始まりのバイト列と比べ、すべてやり直して今のバイト列と比べる
    fn finish(&mut self, steps: usize) -> Result<(), String> {
        let label = "終わり";
        let mut undone = 0;
        while self.document.undo() {
            undone += 1;
        }
        let back = self.document.save();
        if back != self.base {
            let text = difference(&back, &self.base, self.options.show);
            self.bug(
                steps,
                label,
                format!("{undone} 回取り消したバイト列が、始まりと違う。{text}"),
            );
        }
        if self.document.edited_records() != 0 {
            self.bug(
                steps,
                label,
                "すべて取り消したのに、変えたレコードが残る".to_owned(),
            );
        }
        if self.document.gone_rows() != 0 {
            self.bug(
                steps,
                label,
                "すべて取り消したのに、除いた行が残る".to_owned(),
            );
        }
        self.check_deep(steps, "すべて取り消した後")?;
        while self.document.redo() {}
        let again = self.document.save();
        if again != self.current {
            let text = difference(&again, &self.current, self.options.show);
            self.bug(
                steps,
                label,
                format!("すべてやり直したバイト列が、今と違う。{text}"),
            );
        }
        self.check_deep(steps, "すべてやり直した後")
    }
}

/// 1 つの入力を比べる。見つけた違いの数を返す
pub fn run(path: &str, options: &Options) -> Result<usize, String> {
    let bytes = std::fs::read(path).map_err(|error| format!("{path} を読めない: {error}"))?;
    println!("== {path}（{} バイト）", bytes.len());
    let mut today = Today::opened(&bytes, options.budget.as_deref())
        .map_err(|failure| format!("今の実装が開けない: {}", failure.describe()))?;
    let document = Document::open(bytes.clone(), options.threads)
        .map_err(|error| format!("文書エンジンが開けない: {error:?}"))?;
    let elements = today
        .elements()
        .map_err(|failure| format!("今の実装が要素を返さない: {}", failure.describe()))?;

    // ID を比べる
    let counts = document.counts;
    println!(
        "  文書エンジン: レコード {}、ENTITIES {}、要素 {}（指せない {}）、ペーパー空間 {}、要素にならない {}、Shift_JIS {}、文字を読めない {}",
        counts.records,
        counts.entities,
        counts.elements,
        counts.unaddressable,
        counts.paper,
        counts.dropped,
        counts.shift_jis,
        counts.unreadable
    );
    if let Some(locked) = document.locked {
        println!("  文書エンジンは、この文書の変更を受け付けない: {locked:?}");
    }
    let mut hashes = BTreeMap::new();
    let mut covered = Vec::new();
    let mut pairs: BTreeMap<(&'static str, &str), usize> = BTreeMap::new();
    let mut outside: BTreeMap<String, usize> = BTreeMap::new();
    let mut known = BTreeSet::new();
    for element in &elements {
        known.insert(element.id.as_str());
        hashes.insert(element.id.clone(), element.hash.clone());
        let row = handle_of(&element.id).and_then(|handle| {
            Some((
                handle,
                document.rows.get(document.row_of(handle)? as usize)?,
            ))
        });
        match row {
            Some((handle, row)) => {
                covered.push(handle);
                *pairs
                    .entry((kind_name(row.kind), element.kind.as_str()))
                    .or_default() += 1;
            }
            None => {
                let place = element.id.rsplit_once('/').map_or("", |(place, _)| place);
                *outside
                    .entry(format!("{place} の {}", element.kind))
                    .or_default() += 1;
            }
        }
    }
    let mut bugs = 0;
    let extra: Vec<String> = document
        .rows
        .iter()
        .filter(|row| row.handle != 0)
        .map(|row| id_of(row.handle))
        .filter(|id| !known.contains(id.as_str()))
        .collect();
    println!(
        "  今の実装の要素 {} 件のうち、文書エンジンが同じ ID で持つもの {} 件",
        elements.len(),
        covered.len()
    );
    for ((ours, theirs), count) in &pairs {
        println!("    {ours} ＝ {theirs}: {count}");
    }
    for (what, count) in &outside {
        println!("    文書エンジンが持たない: {what}: {count}");
    }
    if !extra.is_empty() {
        bugs += extra.len();
        println!(
            "  違い: 文書エンジンだけが持つ要素が {} 件（最初は {}）",
            extra.len(),
            extra.first().map_or("", String::as_str)
        );
    }
    // 開いた結果は、分けて読む区間の数に依らない
    for parts in [1, 2, 3, 7, 16] {
        let split = store::scan_split(&bytes, parts)
            .and_then(|scanned| Document::assemble(bytes.clone(), scanned))
            .map_err(|error| format!("文書エンジンが {parts} 個に分けて開けない: {error:?}"))?;
        let mut split = split;
        split.build_display();
        split.build_index();
        if !(document.same_content(&split)
            && document.display == split.display
            && document
                .live_boxes()
                .iter()
                .map(|item| item.map(f64::to_bits))
                .eq(split.live_boxes().iter().map(|item| item.map(f64::to_bits)))
            && document.grid == split.grid
            && document.counts == split.counts
            && document.locked == split.locked)
        {
            bugs += 1;
            println!("  違い: {parts} 個に分けて開いた文書が、ふつうに開いた文書と違う");
        }
    }

    let mut extent = [
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    ];
    for item in &document.boxes {
        if item.iter().all(|value| value.is_finite()) {
            extent = [
                extent[0].min(item[0]),
                extent[1].min(item[1]),
                extent[2].max(item[2]),
                extent[3].max(item[3]),
            ];
        }
    }
    if extent.iter().any(|value| !value.is_finite()) {
        extent = [0.0, 0.0, 100.0, 100.0];
    }
    let largest = document
        .rows
        .iter()
        .map(|row| row.handle)
        .max()
        .unwrap_or(0);
    let mut unknown = largest + 0x4000_0000_0000;
    while known.contains(id_of(unknown).as_str()) || document.row_of(unknown).is_some() {
        unknown += 1;
    }
    let steps = options
        .steps
        .unwrap_or_else(|| (4_000_000 / covered.len().max(1)).clamp(80, 4000));
    let names = Names::of(&document);
    println!(
        "  変更で指す名前: 表にある画層 {}、図形が指すだけの画層 {}、線種 {}、文字スタイル {}、ブロックの定義 {}",
        names.layers.len(),
        names.loose_layers.len(),
        names.line_types.len(),
        names.text_styles.len(),
        names.blocks.len()
    );
    let mut run = Run {
        options,
        today,
        document,
        current: bytes.clone(),
        base: bytes,
        hashes,
        covered,
        unknown,
        extent,
        names,
        last_move: None,
        late: false,
        random: Random(options.seed),
        tally: Tally::default(),
    };
    let deep_every = (steps / 16).max(1);
    for step in 0..steps {
        let deep = (step + 1) % deep_every == 0;
        run.late = step * 20 >= steps * 19;
        run.step(step, deep)?;
        if deep {
            run.check_deep(step, "途中")?;
        }
    }
    let gone = run.document.gone_rows();
    let rows = run.document.rows.len();
    run.finish(steps)?;

    let tally = &run.tally;
    let declined: usize = tally.declined.values().sum();
    let gaps: usize = tally.gaps.values().sum();
    println!(
        "  変更 {} 回: 両方が受け付けてバイト列が同じ {}、両方が受け付けない {}、今の実装だけが受け付ける（試作の範囲の外） {}、違い {}",
        tally.steps, tally.equal, declined, gaps, tally.bugs
    );
    for (label, [no, yes]) in &tally.labels {
        println!("    {label}: 今の実装が受け付けた {yes}、受け付けない {no}");
    }
    for ((what, kind), count) in &tally.kinds {
        println!("    {} {kind}: {count}", what.done());
    }
    for (key, count) in &tally.declined {
        println!("    両方が受け付けない: {key}: {count}");
    }
    for (key, count) in &tally.gaps {
        println!("    試作の範囲の外: {key}: {count}");
    }
    println!(
        "  文書エンジンの中の確かめ: 取り消しとやり直し {} 回、足した要素の ID {} 回、開き直した文書との一致 {} 回（うち、表示のバッファーが並びまで同じ {} 回）、範囲で選ぶ {} 回、開き直し {} 回",
        tally.undo_checks,
        tally.id_checks,
        tally.deep_checks,
        tally.same_buffers,
        tally.select_checks,
        tally.resyncs,
    );
    if tally.stale > 0 || tally.drifted > 0 {
        println!(
            "  今の実装のハッシュ: 覚えている値が古いとして受け付けなかった {} 回（比べた数に入れない）、変更が指していないのに変わった要素 {} 件",
            tally.stale, tally.drifted
        );
    }
    println!(
        "  終わりの文書: {} バイト、指せる要素 {} 件、文書エンジンの行 {}（うち、除いて場所を残したもの {}）",
        run.current.len(),
        run.covered.len(),
        rows,
        gone
    );
    Ok(bugs + tally.bugs)
}
