//! ページを解釈する前の走査（決定 0010・0256）。
//!
//! hayro-interpret の解釈は、予算でも中断の合図でも止まらない。フォームの XObject が次の段のフォームを 2 回ずつ
//! 呼ぶと、解釈する演算子の数は段の数の冪で増える。そこで、ページを渡す前に、内容のストリームを hayro-interpret と
//! 同じ読み手（hayro-syntax の `TypedIter`）でたどり、hayro-interpret が解釈する演算子の数を `Work` に、展開する
//! バイト数を `DecodedBytes` に、先に数える。同じフォームを同じ文脈で呼ぶ所は、1 度だけたどる。
//!
//! あわせて、hayro-interpret が知らせずに捨てるものを診断にする: 展開できないか途中から読めない内容のストリームと、
//! 壊れた XObject、資源にないフォントと読めないフォント、代わりのフォントで描かない文字列、表示の規則の辞書（OCMD）
//! で隠れる内容、転送関数。

use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, Read};
use std::rc::Rc;

use flate2::read::{DeflateDecoder, ZlibDecoder};
use hayro_font::FontCache;
use hayro_interpret::InterpreterSettings;
use hayro_syntax::Filter;
use hayro_syntax::content::TypedIter;
use hayro_syntax::content::ops::TypedInstruction;
use hayro_syntax::object::{
    Array, Dict, Name, Object, ObjectIdentifier, Stream, String as PdfString, dict_or_stream,
};
use hayro_syntax::page::{Page, Resources};
use hayro_syntax::xref::XRef;
use zumen_base::{Code, Diagnostic, Kind, Ref, Stop, codes};
use zumen_sdk::Cx;

/// hayro-interpret がフォームの XObject を入れ子に解釈する深さの上限（`MAX_NESTED_INTERPRETATION_DEPTH`）
const MAX_DEPTH: u32 = 50;

/// 内容のストリームの後ろに足す、終わりの印の演算子
const END: &[u8] = b"zumen-end";

/// 走査の結果
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Survey {
    /// 転送関数（10.4）を持つ拡張グラフィックス状態を使う
    pub(crate) transfer: bool,
}

/// hayro-interpret がする作業の量
#[derive(Clone, Copy, Debug, Default)]
struct Cost {
    /// 解釈する演算子の数
    ops: u64,
    /// 展開するバイト数
    bytes: u64,
}

impl Cost {
    fn add(&mut self, other: Cost) {
        self.ops = self.ops.saturating_add(other.ops);
        self.bytes = self.bytes.saturating_add(other.bytes);
    }
}

/// 選んであるフォント（`Tf`、9.3.1）
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Selected {
    /// まだ選んでいない
    Nothing,
    /// hayro-interpret が Helvetica で読む（資源にないフォントと、読めないフォント）
    Fallback,
    /// 読めるフォント
    Loaded,
}

/// 展開したフォーム
#[derive(Clone, Debug)]
struct Form {
    /// 展開したバイト列に、終わりの印を足したもの
    data: Rc<[u8]>,
    /// hayro-syntax が寛容に展開した（[`leniently_inflated`]）
    lenient: bool,
}

/// 資源の辞書の持ち主
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Owner {
    Page,
    /// 間接オブジェクトのフォーム
    Form((u32, i32)),
    /// 間接オブジェクトでないフォーム。呼ばれ方を覚えない
    Direct,
}

/// フォームの呼ばれ方。同じ呼ばれ方なら、hayro-interpret は同じ作業をする
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Call {
    form: (u32, i32),
    owner: Owner,
    /// フォームを解釈する深さ（ページの内容は 0）
    depth: u32,
    /// 呼ぶ所で選んであるフォント
    font: Selected,
}

fn key(id: impl Into<ObjectIdentifier>) -> (u32, i32) {
    let id = id.into();
    (id.obj_number, id.gen_number)
}

fn name_text(name: &[u8]) -> String {
    String::from_utf8_lossy(name).into_owned()
}

/// 内容のストリームのバイト列に、終わりの印を足したもの。最後の行のコメント（7.2.4）に入らないよう、改行を挟む
fn with_end(data: &[u8]) -> Rc<[u8]> {
    let mut bytes = Vec::with_capacity(data.len() + END.len() + 2);
    bytes.extend_from_slice(data);
    bytes.push(b'\n');
    bytes.extend_from_slice(END);
    bytes.push(b'\n');
    Rc::from(bytes)
}

/// 終わりの印の演算子か
fn is_end(op: &TypedInstruction<'_, '_>) -> bool {
    match op {
        TypedInstruction::Fallback(operator) => {
            let name: &[u8] = operator;
            name == END
        }
        _ => false,
    }
}

/// 拡張グラフィックス状態が、転送関数を持つか。名前（`Identity`・`Default`）は、関数を外す（10.4、8.4.5 の表 57）
fn transfer(state: &Dict<'_>) -> bool {
    state
        .get::<Object<'_>>(b"TR2")
        .or_else(|| state.get::<Object<'_>>(b"TR"))
        .is_some_and(|function| !matches!(function, Object::Name(_)))
}

/// ストリームを、hayro-syntax が寛容に展開したか（7.4.4）。hayro-syntax は Flate を、zlib の形、見出しのない
/// deflate の形の順に厳しく展開し、どちらも壊れていれば、展開できた所までを返す。フィルターが Flate だけで予測子の
/// ないストリームを、同じ順に厳しく展開し直し、hayro-syntax が返した長さ `length` になるかを見る。展開し直すのは、
/// その長さより 1 バイト多い所まで
fn leniently_inflated(stream: &Stream<'_>, length: usize) -> bool {
    let dict = stream.dict();
    let predicted = dict
        .get::<Dict<'_>>(b"DecodeParms")
        .or_else(|| dict.get::<Dict<'_>>(b"DP"))
        .and_then(|params| params.get::<i64>(b"Predictor"))
        .is_some_and(|predictor| predictor > 1);
    if predicted || !matches!(stream.filters().as_slice(), [Filter::FlateDecode]) {
        return false;
    }
    let raw = stream.raw_data();
    !(inflates_to(ZlibDecoder::new(raw.as_ref()), length)
        || inflates_to(DeflateDecoder::new(raw.as_ref()), length))
}

/// 厳しく展開して、ちょうど `length` バイトになるか
fn inflates_to(decoder: impl Read, length: usize) -> bool {
    let length = u64::try_from(length).unwrap_or(u64::MAX);
    let copied = io::copy(&mut decoder.take(length.saturating_add(1)), &mut io::sink());
    matches!(copied, Ok(copied) if copied == length)
}

/// 画像の辞書が、画素の数を持つか。hayro-interpret は、どちらかがないか 0 の画像を描かない（8.9.5.1）
fn has_size(dict: &Dict<'_>) -> bool {
    let size = |short: &[u8], long: &[u8]| {
        dict.get::<u32>(short)
            .or_else(|| dict.get::<u32>(long))
            .is_some_and(|value| value > 0)
    };
    size(b"W", b"Width") && size(b"H", b"Height")
}

pub(crate) struct Walker<'a, 'o> {
    xref: &'a XRef,
    /// 間接オブジェクト → L1 のレコードの出所
    objects: &'o BTreeMap<(u32, i32), Ref>,
    /// 既定の設定で隠れる表示グループ（hayro-interpret の `OcgState` と同じ決め方）
    inactive: &'o BTreeSet<(u32, i32)>,
    settings: &'o InterpreterSettings,
    /// hayro-interpret と同じ読み方でフォントを読む。読めるかを見るだけに使う
    fonts: FontCache<'a>,
    /// ページの最初の内容のストリーム。ほかに場所のない診断の場所
    place: Ref,
    /// フォームの呼ばれ方 → hayro-interpret がする作業の量（ページごと）
    calls: BTreeMap<Call, Cost>,
    /// 展開したフォーム。展開できなければ `None`（ページごと）
    forms: BTreeMap<(u32, i32), Option<Form>>,
    /// ソフトマスクのグループとシェーディングのストリームの、展開したバイト数（ページごと）
    sizes: BTreeMap<(u32, i32), u64>,
    /// 印つきの内容の入れ子の、見えるか（hayro-interpret の `OcgState` の重なりと同じ）
    visible: Vec<bool>,
    /// ページで 1 度だけ返す診断
    reported: BTreeSet<(&'static str, String)>,
    transfer: bool,
}

impl<'a, 'o> Walker<'a, 'o> {
    pub(crate) fn new(
        xref: &'a XRef,
        objects: &'o BTreeMap<(u32, i32), Ref>,
        inactive: &'o BTreeSet<(u32, i32)>,
        settings: &'o InterpreterSettings,
    ) -> Self {
        Self {
            xref,
            objects,
            inactive,
            settings,
            fonts: FontCache::new(),
            place: Ref::default(),
            calls: BTreeMap::new(),
            forms: BTreeMap::new(),
            sizes: BTreeMap::new(),
            visible: Vec::new(),
            reported: BTreeSet::new(),
            transfer: false,
        }
    }

    /// ページを走査し、hayro-interpret がする作業を予算に数える。`place` は、ページの最初の内容のストリーム
    pub(crate) fn page(
        &mut self,
        page: &Page<'a>,
        place: &Ref,
        cx: &mut Cx<'_>,
    ) -> Result<Survey, Stop> {
        self.place = place.clone();
        self.calls.clear();
        self.forms.clear();
        self.sizes.clear();
        self.visible.clear();
        self.reported.clear();
        self.transfer = false;

        let raw = page.raw();
        // 内容のストリームを確かめる（7.7.3.3）。hayro-syntax は、展開できない配列の項目を飛ばしてつなぎ、壊れた
        // Flate は展開できた所までを返す。配列は、項目ごとに展開し直す
        if let Some(items) = raw.get::<Array<'_>>(b"Contents") {
            for item in items.raw_iter() {
                cx.check(Kind::Work, 1)?;
                let id = item.as_obj_ref().map(ObjectIdentifier::from);
                let stream = id.and_then(|id| self.xref.get::<Stream<'_>>(id));
                let length = stream
                    .as_ref()
                    .and_then(|stream| stream.decoded_data().ok())
                    .map(|data| data.len());
                if let Some(length) = length {
                    cx.check(Kind::DecodedBytes, length as u64)?;
                }
                let readable = match (&stream, length) {
                    (Some(stream), Some(length)) => !leniently_inflated(stream, length),
                    _ => false,
                };
                if !readable {
                    let place = self.record(id);
                    self.report(codes::PDF_CONTENT_INVALID, place, cx);
                }
            }
        }
        let content = page.page_stream();
        if let Some(data) = content {
            cx.check(Kind::DecodedBytes, data.len() as u64)?;
        }
        let broken = match raw.get::<Stream<'_>>(b"Contents") {
            Some(stream) => content.is_none_or(|data| leniently_inflated(&stream, data.len())),
            None => content.is_none() && raw.contains_key(b"Contents"),
        };
        if broken {
            self.report(codes::PDF_CONTENT_INVALID, place.clone(), cx);
        }

        let data = with_end(content.unwrap_or_default());
        let cost = self.content(
            &data,
            page.resources(),
            Owner::Page,
            0,
            Selected::Nothing,
            place,
            cx,
        )?;
        // hayro-interpret が解釈する演算子と、フォームなどを展開するバイト数を、解釈の前に数える
        cx.check(Kind::Work, cost.ops)?;
        cx.check(Kind::DecodedBytes, cost.bytes)?;
        Ok(Survey {
            transfer: self.transfer,
        })
    }

    /// 間接オブジェクトのレコードの出所。なければページの内容のストリーム
    fn record(&self, id: Option<ObjectIdentifier>) -> Ref {
        id.and_then(|id| self.objects.get(&key(id)).cloned())
            .unwrap_or_else(|| self.place.clone())
    }

    fn report(&mut self, code: Code, place: Ref, cx: &mut Cx<'_>) {
        cx.report(Diagnostic::new(code).at(place));
    }

    /// ページで 1 度だけ返す診断。`value` は、同じものか見分ける値
    fn report_once(&mut self, diagnostic: Diagnostic, value: String, cx: &mut Cx<'_>) {
        if self.reported.insert((diagnostic.code().name(), value)) {
            cx.report(diagnostic);
        }
    }

    fn is_visible(&self) -> bool {
        self.visible.last().copied().unwrap_or(true)
    }

    /// 内容のストリーム（終わりの印を足したもの）をたどる。`font` は、呼ぶ所で選んであるフォント
    #[allow(clippy::too_many_arguments)]
    fn content(
        &mut self,
        data: &[u8],
        resources: &Resources<'a>,
        owner: Owner,
        depth: u32,
        font: Selected,
        place: &Ref,
        cx: &mut Cx<'_>,
    ) -> Result<Cost, Stop> {
        let mut cost = Cost::default();
        // q・Q で保存する、選んであるフォント（9.3.1）
        let mut fonts = vec![font];
        let mut ended = false;
        let mut ops = TypedIter::new(data);
        while let Some(op) = ops.next() {
            cx.check(Kind::Work, 1)?;
            if is_end(&op) {
                ended = true;
                break;
            }
            cost.ops = cost.ops.saturating_add(1);
            let font = fonts.last().copied().unwrap_or(Selected::Nothing);
            match op {
                TypedInstruction::SaveState(_) => fonts.push(font),
                TypedInstruction::RestoreState(_) => {
                    if fonts.len() > 1 {
                        fonts.pop();
                    }
                }
                TypedInstruction::SetGraphicsState(state) => {
                    if let Some(state) = resources.get_ext_g_state(state.0) {
                        cost.bytes = cost.bytes.saturating_add(self.state(&state, cx)?);
                    }
                }
                TypedInstruction::TextFont(selected) => {
                    let selected = self.font(resources, selected.0, cx);
                    if let Some(last) = fonts.last_mut() {
                        *last = selected;
                    }
                }
                TypedInstruction::ShowText(text) => {
                    self.show(&mut fonts, true, &[text.0.as_bytes()], place, cx);
                }
                TypedInstruction::ShowTexts(texts) => {
                    let strings: Vec<PdfString<'_>> = texts
                        .0
                        .iter::<Object<'_>>()
                        .filter_map(|item| item.into_string())
                        .collect();
                    let strings: Vec<&[u8]> = strings.iter().map(PdfString::as_bytes).collect();
                    self.show(&mut fonts, true, &strings, place, cx);
                }
                TypedInstruction::NextLineAndShowText(text) => {
                    self.show(&mut fonts, false, &[text.0.as_bytes()], place, cx);
                }
                TypedInstruction::ShowTextWithParameters(text) => {
                    self.show(&mut fonts, false, &[text.2.as_bytes()], place, cx);
                }
                TypedInstruction::BeginMarkedContentWithProperties(marked) => {
                    self.begin_marked(resources, marked.1, place, cx);
                }
                TypedInstruction::BeginMarkedContent(_) => {
                    let visible = self.is_visible();
                    self.visible.push(visible);
                }
                TypedInstruction::EndMarkedContent(_) => {
                    self.visible.pop();
                }
                TypedInstruction::XObject(x_object) => {
                    if let Some(stream) = resources.get_x_object(x_object.0) {
                        let called = self.x_object(&stream, resources, owner, depth, font, cx)?;
                        cost.add(called);
                    }
                }
                TypedInstruction::InlineImage(image) => {
                    if self.is_visible() && !has_size(image.0.dict()) {
                        self.report(codes::PDF_CONTENT_INVALID, place.clone(), cx);
                    }
                }
                TypedInstruction::Shading(shading) => {
                    if let Some(Object::Stream(stream)) = resources.get_shading(shading.0) {
                        cost.bytes = cost.bytes.saturating_add(self.size(&stream, cx)?);
                    }
                }
                TypedInstruction::NonStrokeColorNamed(color) => {
                    if let Some(name) = color.1 {
                        cost.bytes = cost
                            .bytes
                            .saturating_add(self.pattern(resources, name, cx)?);
                    }
                }
                TypedInstruction::StrokeColorNamed(color) => {
                    if let Some(name) = color.1 {
                        cost.bytes = cost
                            .bytes
                            .saturating_add(self.pattern(resources, name, cx)?);
                    }
                }
                _ => {}
            }
        }
        // 終わりの印まで読めなければ、そこから後ろを hayro-interpret も読まない（引数の足りない経路の演算子・
        // 閉じない文字列など）
        if !ended {
            self.report(codes::PDF_CONTENT_INVALID, place.clone(), cx);
        }
        Ok(cost)
    }

    /// 拡張グラフィックス状態（8.4.5）。転送関数を見つけ、ソフトマスクのグループを展開するバイト数を返す
    /// （hayro-interpret は、状態を当てるたびにグループを展開する）
    fn state(&mut self, state: &Dict<'a>, cx: &mut Cx<'_>) -> Result<u64, Stop> {
        self.transfer |= transfer(state);
        match state
            .get::<Dict<'_>>(b"SMask")
            .and_then(|mask| mask.get::<Stream<'_>>(b"G"))
        {
            Some(group) => self.size(&group, cx),
            None => Ok(0),
        }
    }

    /// シェーディングのパターン（8.7.4.3）のストリームを展開するバイト数。hayro-interpret は、パターンを選ぶたびに
    /// シェーディングを読む
    fn pattern(
        &mut self,
        resources: &Resources<'a>,
        name: &Name<'_>,
        cx: &mut Cx<'_>,
    ) -> Result<u64, Stop> {
        match resources.get_pattern(name) {
            Some(Object::Dict(pattern)) => match pattern.get::<Object<'_>>(b"Shading") {
                Some(Object::Stream(shading)) => self.size(&shading, cx),
                _ => Ok(0),
            },
            _ => Ok(0),
        }
    }

    /// ストリームを展開したバイト数。ページの中では 1 度だけ展開する
    fn size(&mut self, stream: &Stream<'a>, cx: &mut Cx<'_>) -> Result<u64, Stop> {
        let id = stream.dict().obj_id().map(key);
        if let Some(size) = id.and_then(|id| self.sizes.get(&id)) {
            return Ok(*size);
        }
        let size = stream.decoded().map_or(0, |data| data.len() as u64);
        cx.check(Kind::DecodedBytes, size)?;
        if let Some(id) = id {
            self.sizes.insert(id, size);
        }
        Ok(size)
    }

    /// 文字を描く（`Tj`・`TJ`・`'`・`"`、9.4.2）。フォントを選ばずに描く文字を、hayro-interpret は `Tj`・`TJ` なら
    /// Helvetica で読み、`'`・`"` なら描かない。Helvetica で読む文字列は、ASCII のバイトだけのものしか描かない。
    /// `fallback` は、フォントを選んでいなければ Helvetica を選ぶ演算子か
    fn show(
        &mut self,
        fonts: &mut [Selected],
        fallback: bool,
        strings: &[&[u8]],
        place: &Ref,
        cx: &mut Cx<'_>,
    ) {
        let visible = self.is_visible();
        let Some(font) = fonts.last_mut() else {
            return;
        };
        if *font == Selected::Nothing && fallback {
            *font = Selected::Fallback;
            if visible {
                self.report_once(
                    Diagnostic::new(codes::PDF_FONT_MISSING)
                        .with("name", "")
                        .at(place.clone()),
                    String::new(),
                    cx,
                );
            }
        }
        if !visible {
            return;
        }
        let dropped = match *font {
            Selected::Loaded => 0,
            Selected::Fallback => strings.iter().filter(|bytes| !bytes.is_ascii()).count(),
            Selected::Nothing => strings.iter().filter(|bytes| !bytes.is_empty()).count(),
        };
        for _ in 0..dropped {
            self.report(codes::PDF_TEXT_DROPPED, place.clone(), cx);
        }
    }

    /// フォントを選ぶ（`Tf`、9.3.1）。資源にないフォントと、hayro-interpret が読めないフォントは、Helvetica で読む
    fn font(&mut self, resources: &Resources<'a>, name: &Name<'_>, cx: &mut Cx<'_>) -> Selected {
        let Some(dict) = resources.get_font(name) else {
            let name = name_text(name.as_ref());
            self.report_once(
                Diagnostic::new(codes::PDF_FONT_MISSING)
                    .with("name", name.as_str())
                    .at(self.place.clone()),
                name,
                cx,
            );
            return Selected::Fallback;
        };
        let settings = self.settings;
        if self
            .fonts
            .font(&dict, &settings.font_resolver, &settings.cmap_resolver)
            .is_some()
        {
            return Selected::Loaded;
        }
        let id = dict.obj_id();
        let base = dict
            .get::<Name<'_>>(b"BaseFont")
            .map_or_else(|| name_text(name.as_ref()), |base| name_text(base.as_ref()));
        let place = self.record(id);
        let value = id.map_or_else(
            || base.clone(),
            |id| format!("{} {}", id.obj_number, id.gen_number),
        );
        self.report_once(
            Diagnostic::new(codes::PDF_FONT_UNSUPPORTED)
                .with("name", base)
                .at(place),
            value,
            cx,
        );
        Selected::Fallback
    }

    /// 印つきの内容の始まり（`BDC`、14.6）。表示グループ（8.11.3.2）の見え方を、hayro-interpret と同じく決める。
    /// 表示グループの名前が資源の `Properties` で間接参照を指すか、内容の中の辞書が `OC` に間接参照を持てば、
    /// 表示グループか表示の規則の辞書として扱う
    fn begin_marked(
        &mut self,
        resources: &Resources<'a>,
        properties: &Object<'_>,
        place: &Ref,
        cx: &mut Cx<'_>,
    ) {
        let named = properties.clone().into_name().and_then(|name| {
            let reference = resources.properties.get_ref(name.as_ref())?;
            let dict = resources
                .properties
                .get::<Dict<'_>>(name.as_ref())
                .unwrap_or_default();
            Some((dict, key(reference)))
        });
        let membership = named.or_else(|| {
            let (inline, _) = dict_or_stream(properties)?;
            let reference = inline.get_ref(b"OC")?;
            let dict = inline.get::<Dict<'_>>(b"OC").unwrap_or_default();
            Some((dict, key(reference)))
        });
        let parent = self.is_visible();
        let visible = match membership {
            Some((dict, reference)) => {
                let (shown, ocmd) = self.membership(&dict, reference);
                if parent && ocmd {
                    self.membership_report(shown, place, cx);
                }
                parent && shown
            }
            None => parent,
        };
        self.visible.push(visible);
    }

    /// 表示の規則の辞書（OCMD）の内容の診断。見える内容は最初の表示グループのレイヤに置き、隠れる内容は
    /// hayro-interpret が描かない
    fn membership_report(&mut self, shown: bool, place: &Ref, cx: &mut Cx<'_>) {
        let code = if shown {
            codes::PDF_OPTIONAL_CONTENT_APPROXIMATED
        } else {
            codes::PDF_MEMBERSHIP_DROPPED
        };
        self.report(code, place.clone(), cx);
    }

    /// 表示グループか表示の規則の辞書で、内容が見えるか。2 つ目は、表示の規則の辞書か（hayro-interpret の
    /// `OcgState::begin_ocg`）
    fn membership(&self, properties: &Dict<'_>, reference: (u32, i32)) -> (bool, bool) {
        if properties.get::<Name<'_>>(b"Type").as_deref() == Some(b"OCMD".as_slice()) {
            (self.ocmd(properties), true)
        } else {
            (!self.inactive.contains(&reference), false)
        }
    }

    /// 表示の規則の辞書（8.11.2.2）で、内容が見えるか。`P` の規則だけを見る。hayro-interpret は `VE` を読まない
    fn ocmd(&self, ocmd: &Dict<'_>) -> bool {
        let groups: Vec<(u32, i32)> = match ocmd.get::<Array<'_>>(b"OCGs") {
            Some(items) => items
                .raw_iter()
                .filter_map(|item| item.as_obj_ref())
                .map(key)
                .collect(),
            None => ocmd.get_ref(b"OCGs").map(key).into_iter().collect(),
        };
        if groups.is_empty() {
            return true;
        }
        let on = |group: &(u32, i32)| !self.inactive.contains(group);
        match ocmd.get::<Name<'_>>(b"P").as_deref() {
            Some(b"AllOn") => groups.iter().all(on),
            Some(b"AnyOff") => groups.iter().any(|group| !on(group)),
            Some(b"AllOff") => groups.iter().all(|group| !on(group)),
            _ => groups.iter().any(on),
        }
    }

    /// XObject の表示グループ（`OC`、8.11.3.3）で、XObject が見えるか。見える内容は、XObject の表示グループの
    /// レイヤに分けない
    fn optional(&mut self, dict: &Dict<'_>, place: &Ref, cx: &mut Cx<'_>) -> bool {
        let Some(properties) = dict.get::<Dict<'_>>(b"OC") else {
            return true;
        };
        let (shown, ocmd) = match dict.get_ref(b"OC") {
            Some(reference) => self.membership(&properties, key(reference)),
            None => (self.ocmd(&properties), true),
        };
        if shown {
            self.report(codes::PDF_OPTIONAL_CONTENT_APPROXIMATED, place.clone(), cx);
        } else if ocmd {
            self.report(codes::PDF_MEMBERSHIP_DROPPED, place.clone(), cx);
        }
        shown
    }

    /// XObject を描く（`Do`、8.8）
    fn x_object(
        &mut self,
        stream: &Stream<'a>,
        resources: &Resources<'a>,
        owner: Owner,
        depth: u32,
        font: Selected,
        cx: &mut Cx<'_>,
    ) -> Result<Cost, Stop> {
        let dict = stream.dict();
        let id = dict.obj_id();
        let place = self.record(id);
        let visible = self.is_visible();
        match dict.get::<Name<'_>>(b"Subtype").as_deref() {
            Some(b"Form") => self.form(stream, id, &place, resources, owner, depth, font, cx),
            Some(b"Image") => {
                if visible {
                    if has_size(dict) {
                        self.optional(dict, &place, cx);
                    } else {
                        self.report(codes::PDF_CONTENT_INVALID, place, cx);
                    }
                }
                Ok(Cost::default())
            }
            // PostScript の XObject は描かない（8.8.2）
            Some(b"PS") => Ok(Cost::default()),
            _ => {
                if visible {
                    self.report(codes::PDF_CONTENT_INVALID, place, cx);
                }
                Ok(Cost::default())
            }
        }
    }

    /// フォームの XObject（8.10）。hayro-interpret は、呼ぶたびに展開し、見えれば中身を解釈する。`BBox` のない
    /// フォームと、展開できないフォームは描かない
    #[allow(clippy::too_many_arguments)]
    fn form(
        &mut self,
        stream: &Stream<'a>,
        id: Option<ObjectIdentifier>,
        place: &Ref,
        resources: &Resources<'a>,
        owner: Owner,
        depth: u32,
        font: Selected,
        cx: &mut Cx<'_>,
    ) -> Result<Cost, Stop> {
        let data = self.form_data(stream, id, cx)?;
        let bytes = data.as_ref().map_or(0, |form| {
            form.data.len().saturating_sub(END.len() + 2) as u64
        });
        let mut cost = Cost { ops: 0, bytes };
        let dict = stream.dict();
        let visible = self.is_visible();
        let Some(Form { data, lenient }) = data.filter(|_| dict.get::<[f32; 4]>(b"BBox").is_some())
        else {
            if visible {
                self.report(codes::PDF_CONTENT_INVALID, place.clone(), cx);
            }
            return Ok(cost);
        };
        // 壊れた Flate のフォームは、展開できた所までを描く
        if visible && lenient {
            self.report(codes::PDF_CONTENT_INVALID, place.clone(), cx);
        }
        if !visible || depth >= MAX_DEPTH || !self.optional(dict, place, cx) {
            return Ok(cost);
        }
        let own = dict.get::<Dict<'_>>(b"Resources").map(Resources::new);
        let owner = match (&own, id) {
            (None, _) => owner,
            (Some(_), Some(id)) => Owner::Form(key(id)),
            (Some(_), None) => Owner::Direct,
        };
        let call = match (id, owner) {
            (Some(id), Owner::Page | Owner::Form(_)) => Some(Call {
                form: key(id),
                owner,
                depth: depth + 1,
                font,
            }),
            _ => None,
        };
        if let Some(known) = call.and_then(|call| self.calls.get(&call)) {
            cost.add(*known);
            return Ok(cost);
        }
        // 印つきの内容の入れ子は、フォームの中だけのものにする（hayro-interpret はフォームの外へ持ち越すが、
        // 同じ呼ばれ方を 1 度だけたどるので、そろえる）
        let saved = self.visible.clone();
        let inner = self.content(
            &data,
            own.as_ref().unwrap_or(resources),
            owner,
            depth + 1,
            font,
            place,
            cx,
        )?;
        self.visible = saved;
        if let Some(call) = call {
            self.calls.insert(call, inner);
        }
        cost.add(inner);
        Ok(cost)
    }

    /// 展開したフォーム。ページの中では 1 度だけ展開する
    fn form_data(
        &mut self,
        stream: &Stream<'a>,
        id: Option<ObjectIdentifier>,
        cx: &mut Cx<'_>,
    ) -> Result<Option<Form>, Stop> {
        let id = id.map(key);
        if let Some(data) = id.and_then(|id| self.forms.get(&id)) {
            return Ok(data.clone());
        }
        let data = stream.decoded_data().ok().map(|data| Form {
            data: with_end(&data),
            lenient: leniently_inflated(stream, data.len()),
        });
        cx.check(
            Kind::DecodedBytes,
            data.as_ref().map_or(0, |form| form.data.len() as u64),
        )?;
        if let Some(id) = id {
            self.forms.insert(id, data.clone());
        }
        Ok(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 終わりの印まで読めるか
    fn reaches_the_end(data: &[u8]) -> bool {
        let data = with_end(data);
        let mut ops = TypedIter::new(&data);
        let mut ended = false;
        while let Some(op) = ops.next() {
            ended |= is_end(&op);
        }
        ended
    }

    #[test]
    fn finds_where_content_stops() {
        // 最後の行が PDF のコメントでも、終わりの印は読める
        assert!(reaches_the_end(b"BT (a) Tj ET % note"));
        assert!(reaches_the_end(b""));
        // 引数の足りない経路の演算子と、閉じない文字列から後ろは読まない
        assert!(!reaches_the_end(b"0 0 m 1 l 2 2 l S"));
        assert!(!reaches_the_end(b"BT (a Tj ET"));
    }
}
