//! ページを解釈する前の走査（決定 0010・0256）。
//!
//! hayro-interpret の解釈は、予算でも中断の合図でも止まらない。フォームの XObject が次の段のフォームを 2 回ずつ
//! 呼ぶと、解釈する演算子の数は段の数の冪で増える。そこで、ページを渡す前に、内容のストリームを hayro-interpret と
//! 同じ読み手（hayro-syntax の `TypedIter`）でたどり、hayro-interpret が解釈する手間を `Work` に、展開する
//! バイト数を `DecodedBytes` に、先に数える。同じフォームを同じ文脈で呼ぶ所は、1 度だけたどる。
//!
//! あわせて、hayro-interpret が知らせずに捨てるものを診断にする: 展開できないか途中から読めない内容のストリームと、
//! 壊れた XObject、資源にない XObject、入れ子の深すぎるフォーム、資源にないフォントと読めないフォント、代わりの
//! フォントで描かない文字列、縦書きの原点の移動が既定と違うフォント、表示の規則の辞書（OCMD）で隠れる内容、
//! 転送関数。

use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use flate2::{Decompress, FlushDecompress, Status};
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
use zumen_base::{Code, Diagnostic, Diagnostics, Kind, Ref, Stop, codes};
use zumen_sdk::Cx;

/// hayro-interpret がフォームの XObject を入れ子に解釈する深さの上限（`MAX_NESTED_INTERPRETATION_DEPTH`）
const MAX_DEPTH: u32 = 50;

// hayro-interpret の手間の重み（`Work`）。簡単な演算子（`1 w`）を 1 とする。hayro-interpret 0.9.0 を release で
// build し、同じ演算子を何万回も並べたページを解釈して測った比（2026-11-03、決定 0256）
/// フォームの XObject を 1 回呼ぶ手間。呼ぶたびにストリームを展開し、辞書と資源を読み、切り抜きを足す（空の
/// フォームで、およそ 50 倍）
const FORM_WORK: u64 = 50;
/// 拡張グラフィックス状態を当てる手間（`gs`。およそ 16 倍）
const STATE_WORK: u64 = 16;
/// 文字列の 1 バイトを描く手間。字形を引いて輪郭を作る（Helvetica の 1 字で、およそ 8 倍）
const GLYPH_WORK: u64 = 8;

/// 展開し直すときに、1 度に書き出すバイト数
const INFLATE_CHUNK: usize = 8192;

/// 関数（7.10）と色空間（8.6）の入れ子をたどる深さの上限
const NESTED_DEPTH: u32 = 8;

/// 内容のストリームの後ろに足す、終わりの印の演算子
const END: &[u8] = b"zumen-end";

/// 走査の結果
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Survey {
    /// 転送関数（10.4）を持つ拡張グラフィックス状態を使う
    pub(crate) transfer: bool,
}

/// hayro-interpret がする作業の量。同じ呼ばれ方のフォームの手間を、まとめて数えるために覚える
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

/// hayro-interpret がする作業を、予算に数えて `cost` に足す。走査の手間は、hayro-interpret の手間より小さいので、
/// 別に数えない
fn spend(cost: &mut Cost, ops: u64, bytes: u64, cx: &mut Cx<'_>) -> Result<(), Stop> {
    cx.check(Kind::Work, ops)?;
    cx.check(Kind::DecodedBytes, bytes)?;
    cost.add(Cost { ops, bytes });
    Ok(())
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

/// 手間を覚えたオブジェクトの役割。同じオブジェクトでも、役割で手間が違う
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Role {
    /// 拡張グラフィックス状態
    State,
    /// シェーディング
    Shading,
    /// 画像の XObject
    Image,
}

/// 覚えたフォームの呼ばれ方: hayro-interpret がする作業と、走査が返した診断（件数を含む）
#[derive(Clone, Debug)]
struct Known {
    cost: Cost,
    diagnostics: Rc<[Diagnostic]>,
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
/// deflate の形の順に厳しく展開し、どちらも壊れていれば、展開できた所までを返す。途中で切れた zlib の形は、
/// 厳しい展開でも、展開できた所までを返す。フィルターが Flate だけで予測子のないストリームを、同じ順と、zlib の
/// 見出しを除いた deflate の形（Adler-32 が違うか、ないもの）で厳しく展開し直し、deflate の終わりまで読めて、
/// hayro-syntax が返した長さ `length` になるかを見る
fn leniently_inflated(stream: &Stream<'_>, length: usize) -> bool {
    if predicted(stream.dict()) || !matches!(stream.filters().as_slice(), [Filter::FlateDecode]) {
        return false;
    }
    let raw = stream.raw_data();
    let raw = raw.as_ref();
    !(inflates_to(raw, true, length)
        || inflates_to(raw, false, length)
        || raw
            .get(2..)
            .is_some_and(|data| inflates_to(data, false, length)))
}

/// Flate の予測子（7.4.4.4）を使うか。hayro-syntax と同じく、フィルターが名前なら辞書を、配列なら配列の最初の
/// 項目を読む
fn predicted(dict: &Dict<'_>) -> bool {
    let single = dict
        .get::<Name<'_>>(b"F")
        .or_else(|| dict.get::<Name<'_>>(b"Filter"))
        .is_some();
    let params = if single {
        dict.get::<Dict<'_>>(b"DP")
            .or_else(|| dict.get::<Dict<'_>>(b"DecodeParms"))
    } else {
        dict.get::<Array<'_>>(b"DP")
            .or_else(|| dict.get::<Array<'_>>(b"DecodeParms"))
            .and_then(|params| params.iter::<Object<'_>>().next())
            .and_then(|params| params.into_dict())
    };
    params
        .and_then(|params| params.get::<i64>(b"Predictor"))
        .is_some_and(|predictor| predictor > 1)
}

/// 厳しく展開して、deflate の終わりまで読め、ちょうど `length` バイトになるか。`zlib` は、zlib の形（見出しと
/// Adler-32）で読むか。書き出すのは、その長さより 1 バイト多い所まで
fn inflates_to(data: &[u8], zlib: bool, length: usize) -> bool {
    let length = u64::try_from(length).unwrap_or(u64::MAX);
    let limit = length.saturating_add(1);
    let mut inflater = Decompress::new(zlib);
    let mut buffer = vec![0; INFLATE_CHUNK];
    loop {
        let consumed = usize::try_from(inflater.total_in()).unwrap_or(usize::MAX);
        let room = usize::try_from(limit.saturating_sub(inflater.total_out()))
            .unwrap_or(usize::MAX)
            .min(INFLATE_CHUNK);
        let (Some(input), Some(output)) = (data.get(consumed..), buffer.get_mut(..room)) else {
            return false;
        };
        let before = (inflater.total_in(), inflater.total_out());
        match inflater.decompress(input, output, FlushDecompress::None) {
            Ok(Status::StreamEnd) => return inflater.total_out() == length,
            // 書き出す所がなくなったか、入力が尽きて進まない（途中で切れた）
            Ok(Status::Ok | Status::BufError)
                if (inflater.total_in(), inflater.total_out()) == before =>
            {
                return false;
            }
            Ok(_) => {}
            Err(_) => return false,
        }
    }
}

/// 文字列を描く手間（[`GLYPH_WORK`]）。1 バイトを 1 字と数える（2 バイトの CMap では多めになる）
fn glyph_work(strings: &[&[u8]]) -> u64 {
    let bytes = strings.iter().map(|bytes| bytes.len() as u64).sum::<u64>();
    bytes.saturating_mul(GLYPH_WORK)
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
    /// フォームの呼ばれ方 → hayro-interpret がする作業と、走査が返した診断（ページごと）
    calls: BTreeMap<Call, Known>,
    /// 拡張グラフィックス状態・シェーディング・画像 → hayro-interpret が使うたびにする作業（ページごと）
    costs: BTreeMap<(Role, (u32, i32)), Cost>,
    /// 縦書きのフォント → 原点の移動が既定と違いうるか（ページごと）
    vertical: BTreeMap<(u32, i32), bool>,
    /// 呼ばれ方を覚えるフォームの中で返した診断。たどっているフォームごとに 1 つ
    recording: Vec<Diagnostics>,
    /// 展開したフォーム。展開できなければ `None`（ページごと）
    forms: BTreeMap<(u32, i32), Option<Form>>,
    /// ソフトマスクのグループとシェーディングのストリームの、展開したバイト数（ページごと）
    sizes: BTreeMap<(u32, i32), u64>,
    /// 印つきの内容の入れ子の、見えるか。hayro-interpret の `OcgState` の重なりと同じく、ページの中で 1 つだけ持ち、
    /// フォームの中と外で分けない
    visible: Vec<bool>,
    /// 入れ子を閉じる前の重なりの数のうち、いちばん小さいもの。たどっている内容のストリームが、始めた所より外の
    /// 入れ子を閉じたかを見る（[`Walker::content`]）
    closed: usize,
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
            costs: BTreeMap::new(),
            vertical: BTreeMap::new(),
            recording: Vec::new(),
            forms: BTreeMap::new(),
            sizes: BTreeMap::new(),
            visible: Vec::new(),
            closed: usize::MAX,
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
        self.costs.clear();
        self.vertical.clear();
        self.recording.clear();
        self.forms.clear();
        self.sizes.clear();
        self.visible.clear();
        self.closed = usize::MAX;
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
        // hayro-interpret が解釈する手間と、フォームなどを展開するバイト数は、たどりながら数える
        self.content(
            &data,
            page.resources(),
            Owner::Page,
            0,
            Selected::Nothing,
            place,
            cx,
        )?;
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
        self.emit(Diagnostic::new(code).at(place), cx);
    }

    /// 診断を返す。呼ばれ方を覚えるフォームの中なら、書き留め、同じ呼ばれ方の 2 回目からも数える
    fn emit(&mut self, diagnostic: Diagnostic, cx: &mut Cx<'_>) {
        if let Some(recording) = self.recording.last_mut() {
            recording.report(diagnostic.clone());
        }
        cx.report(diagnostic);
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

    /// 印つきの内容の入れ子を閉じる（hayro-interpret の `end_marked_content`。空なら何もしない）
    fn end_marked(&mut self) {
        self.closed = self.closed.min(self.visible.len());
        self.visible.pop();
    }

    /// 内容のストリーム（終わりの印を足したもの）をたどる。`font` は、呼ぶ所で選んであるフォント。2 つ目は、
    /// 印つきの内容の入れ子が釣り合うか。釣り合うのは、始めた所より外の入れ子を閉じず、始めた所に戻って終わる
    /// とき。このとき、見えるかの重なりは、たどる前と同じになり、たどった中身は、呼ぶ所が見えることだけで決まる
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
    ) -> Result<(Cost, bool), Stop> {
        let mut cost = Cost::default();
        // q・Q で保存する、選んであるフォント（9.3.1）
        let mut fonts = vec![font];
        let start = self.visible.len();
        let outer = std::mem::replace(&mut self.closed, usize::MAX);
        let mut ended = false;
        let mut ops = TypedIter::new(data);
        while let Some(op) = ops.next() {
            if is_end(&op) {
                ended = true;
                break;
            }
            spend(&mut cost, 1, 0, cx)?;
            let font = fonts.last().copied().unwrap_or(Selected::Nothing);
            match op {
                TypedInstruction::SaveState(_) => fonts.push(font),
                TypedInstruction::RestoreState(_) => {
                    if fonts.len() > 1 {
                        fonts.pop();
                    }
                }
                TypedInstruction::SetGraphicsState(state) => {
                    let extra = match resources.get_ext_g_state(state.0) {
                        Some(state) => self.state(&state, cx)?,
                        None => Cost::default(),
                    };
                    spend(
                        &mut cost,
                        STATE_WORK.saturating_add(extra.ops),
                        extra.bytes,
                        cx,
                    )?;
                }
                TypedInstruction::TextFont(selected) => {
                    let selected = self.font(resources, selected.0, cx)?;
                    if let Some(last) = fonts.last_mut() {
                        *last = selected;
                    }
                }
                TypedInstruction::ShowText(text) => {
                    spend(&mut cost, glyph_work(&[text.0.as_bytes()]), 0, cx)?;
                    self.show(&mut fonts, true, &[text.0.as_bytes()], place, cx);
                }
                TypedInstruction::ShowTexts(texts) => {
                    let strings: Vec<PdfString<'_>> = texts
                        .0
                        .iter::<Object<'_>>()
                        .filter_map(|item| item.into_string())
                        .collect();
                    let strings: Vec<&[u8]> = strings.iter().map(PdfString::as_bytes).collect();
                    spend(&mut cost, glyph_work(&strings), 0, cx)?;
                    self.show(&mut fonts, true, &strings, place, cx);
                }
                TypedInstruction::NextLineAndShowText(text) => {
                    spend(&mut cost, glyph_work(&[text.0.as_bytes()]), 0, cx)?;
                    self.show(&mut fonts, false, &[text.0.as_bytes()], place, cx);
                }
                TypedInstruction::ShowTextWithParameters(text) => {
                    spend(&mut cost, glyph_work(&[text.2.as_bytes()]), 0, cx)?;
                    self.show(&mut fonts, false, &[text.2.as_bytes()], place, cx);
                }
                TypedInstruction::BeginMarkedContentWithProperties(marked) => {
                    self.begin_marked(resources, marked.1, place, cx);
                }
                TypedInstruction::BeginMarkedContent(_) => {
                    let visible = self.is_visible();
                    self.visible.push(visible);
                }
                TypedInstruction::EndMarkedContent(_) => self.end_marked(),
                TypedInstruction::XObject(x_object) => match resources.get_x_object(x_object.0) {
                    Some(stream) => {
                        let called = self.x_object(&stream, resources, owner, depth, font, cx)?;
                        cost.add(called);
                    }
                    // hayro-interpret は、資源にない XObject（循環する参照を含む）を描かない
                    None => {
                        if self.is_visible() {
                            let name = name_text(x_object.0.as_ref());
                            self.report_once(
                                Diagnostic::new(codes::PDF_XOBJECT_MISSING)
                                    .with("name", name.as_str())
                                    .at(place.clone()),
                                name,
                                cx,
                            );
                        }
                    }
                },
                TypedInstruction::InlineImage(image) => {
                    let dict = image.0.dict();
                    if self.is_visible() && !has_size(dict) {
                        self.report(codes::PDF_CONTENT_INVALID, place.clone(), cx);
                    }
                    let space = dict
                        .get::<Object<'_>>(b"CS")
                        .or_else(|| dict.get::<Object<'_>>(b"ColorSpace"));
                    if let Some(space) = space {
                        let extra = self.color_space(&space, 0, cx)?;
                        spend(&mut cost, extra.ops, extra.bytes, cx)?;
                    }
                }
                TypedInstruction::Shading(shading) => {
                    if let Some(shading) = resources.get_shading(shading.0) {
                        let extra = self.shading(&shading, cx)?;
                        spend(&mut cost, extra.ops, extra.bytes, cx)?;
                    }
                }
                TypedInstruction::NonStrokeColorNamed(color) => {
                    if let Some(name) = color.1 {
                        let extra = self.pattern(resources, name, cx)?;
                        spend(&mut cost, extra.ops, extra.bytes, cx)?;
                    }
                }
                TypedInstruction::StrokeColorNamed(color) => {
                    if let Some(name) = color.1 {
                        let extra = self.pattern(resources, name, cx)?;
                        spend(&mut cost, extra.ops, extra.bytes, cx)?;
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
        let balanced = self.closed > start && self.visible.len() == start;
        self.closed = self.closed.min(outer);
        Ok((cost, balanced))
    }

    /// 拡張グラフィックス状態（8.4.5）を当てる手間。転送関数を見つける。hayro-interpret は、状態を当てるたびに、
    /// 転送関数（`TR`・`TR2`、10.4）を作り、ソフトマスク（11.6.5.2）のグループを展開し、その色空間と転送関数を作る
    fn state(&mut self, state: &Dict<'a>, cx: &mut Cx<'_>) -> Result<Cost, Stop> {
        self.transfer |= transfer(state);
        let id = state.obj_id().map(key);
        if let Some(cost) = id.and_then(|id| self.costs.get(&(Role::State, id))) {
            return Ok(*cost);
        }
        let mut cost = Cost::default();
        let function = state
            .get::<Object<'_>>(b"TR2")
            .or_else(|| state.get::<Object<'_>>(b"TR"));
        if let Some(function) = function {
            cost.add(self.function(&function, 0, cx)?);
        }
        if let Some(mask) = state.get::<Dict<'_>>(b"SMask") {
            if let Some(group) = mask.get::<Stream<'_>>(b"G") {
                cost.bytes = cost.bytes.saturating_add(self.size(&group, cx)?);
                let space = group
                    .dict()
                    .get::<Dict<'_>>(b"Group")
                    .and_then(|group| group.get::<Object<'_>>(b"CS"));
                if let Some(space) = space {
                    cost.add(self.color_space(&space, 0, cx)?);
                }
            }
            if let Some(function) = mask.get::<Object<'_>>(b"TR") {
                cost.add(self.function(&function, 0, cx)?);
            }
        }
        if let Some(id) = id {
            self.costs.insert((Role::State, id), cost);
        }
        Ok(cost)
    }

    /// シェーディングのパターン（8.7.3.3）を選ぶ手間。hayro-interpret は、選ぶたびにシェーディングを作る。タイルの
    /// パターンは、塗るときまで中身を展開しない
    fn pattern(
        &mut self,
        resources: &Resources<'a>,
        name: &Name<'_>,
        cx: &mut Cx<'_>,
    ) -> Result<Cost, Stop> {
        match resources.get_pattern(name) {
            Some(Object::Dict(pattern)) => match pattern.get::<Object<'_>>(b"Shading") {
                Some(shading) => self.shading(&shading, cx),
                None => Ok(Cost::default()),
            },
            _ => Ok(Cost::default()),
        }
    }

    /// シェーディング（8.7.4.3）を作る手間。hayro-interpret は、使うたびに作り直し、ストリームと関数を展開し、
    /// 色空間を作る
    fn shading(&mut self, shading: &Object<'_>, cx: &mut Cx<'_>) -> Result<Cost, Stop> {
        let Some((dict, stream)) = dict_or_stream(shading) else {
            return Ok(Cost::default());
        };
        let id = dict.obj_id().map(key);
        if let Some(cost) = id.and_then(|id| self.costs.get(&(Role::Shading, id))) {
            return Ok(*cost);
        }
        let mut cost = Cost::default();
        if let Some(stream) = stream {
            cost.bytes = cost.bytes.saturating_add(self.size(stream, cx)?);
        }
        if let Some(function) = dict.get::<Object<'_>>(b"Function") {
            cost.add(self.function(&function, 0, cx)?);
        }
        if let Some(space) = dict.get::<Object<'_>>(b"ColorSpace") {
            cost.add(self.color_space(&space, 0, cx)?);
        }
        if let Some(id) = id {
            self.costs.insert((Role::Shading, id), cost);
        }
        Ok(cost)
    }

    /// 画像の XObject を描く手間。hayro-interpret は、描くたびに画像の色空間を作る。画素は、描くときまで展開しない
    fn image(&mut self, dict: &Dict<'_>, cx: &mut Cx<'_>) -> Result<Cost, Stop> {
        let id = dict.obj_id().map(key);
        if let Some(cost) = id.and_then(|id| self.costs.get(&(Role::Image, id))) {
            return Ok(*cost);
        }
        let cost = match dict.get::<Object<'_>>(b"ColorSpace") {
            Some(space) => self.color_space(&space, 0, cx)?,
            None => Cost::default(),
        };
        if let Some(id) = id {
            self.costs.insert((Role::Image, id), cost);
        }
        Ok(cost)
    }

    /// 関数（7.10）を作る手間。hayro-interpret は、関数を使う演算子ごとに関数を作り直し、ストリームの関数（Type 0・
    /// Type 4）を展開する。つなぐ関数（Type 3）の `Functions` と、関数の配列の中もたどる
    fn function(
        &mut self,
        function: &Object<'_>,
        depth: u32,
        cx: &mut Cx<'_>,
    ) -> Result<Cost, Stop> {
        cx.check(Kind::Work, 1)?;
        let mut cost = Cost { ops: 1, bytes: 0 };
        if depth >= NESTED_DEPTH {
            return Ok(cost);
        }
        let functions = match function {
            Object::Stream(stream) => {
                cost.bytes = self.size(stream, cx)?;
                return Ok(cost);
            }
            Object::Dict(dict) => dict.get::<Array<'_>>(b"Functions"),
            Object::Array(functions) => Some(functions.clone()),
            _ => None,
        };
        for function in functions
            .iter()
            .flat_map(|functions| functions.iter::<Object<'_>>())
        {
            cost.add(self.function(&function, depth + 1, cx)?);
        }
        Ok(cost)
    }

    /// 直接書いた色空間（8.6）を作る手間。hayro-interpret は、名前で引く色空間は覚え、直接書いた配列は使うたびに
    /// 作り直す。分版（Separation・DeviceN、8.6.6.4・8.6.6.5）の色変換の関数と、Indexed の表（8.6.6.3）を数える
    fn color_space(
        &mut self,
        space: &Object<'_>,
        depth: u32,
        cx: &mut Cx<'_>,
    ) -> Result<Cost, Stop> {
        cx.check(Kind::Work, 1)?;
        let mut cost = Cost { ops: 1, bytes: 0 };
        let Object::Array(items) = space else {
            return Ok(cost);
        };
        if depth >= NESTED_DEPTH {
            return Ok(cost);
        }
        let mut items = items.iter::<Object<'_>>();
        let family = items.next().and_then(|family| family.into_name());
        match family.as_deref() {
            Some(b"Separation" | b"DeviceN") => {
                let _names = items.next();
                if let Some(alternate) = items.next() {
                    cost.add(self.color_space(&alternate, depth + 1, cx)?);
                }
                if let Some(tint) = items.next() {
                    cost.add(self.function(&tint, depth + 1, cx)?);
                }
            }
            Some(b"Indexed" | b"I") => {
                if let Some(base) = items.next() {
                    cost.add(self.color_space(&base, depth + 1, cx)?);
                }
                let _high = items.next();
                if let Some(Object::Stream(table)) = items.next() {
                    cost.bytes = cost.bytes.saturating_add(self.size(&table, cx)?);
                }
            }
            Some(b"Pattern") => {
                if let Some(base) = items.next() {
                    cost.add(self.color_space(&base, depth + 1, cx)?);
                }
            }
            _ => {}
        }
        Ok(cost)
    }

    /// ストリームを展開したバイト数。ページの中では 1 度だけ展開する。hayro-interpret と同じく、画像のフィルター
    /// （DCT など）は展開しない
    fn size(&mut self, stream: &Stream<'_>, cx: &mut Cx<'_>) -> Result<u64, Stop> {
        let id = stream.dict().obj_id().map(key);
        if let Some(size) = id.and_then(|id| self.sizes.get(&id)) {
            return Ok(*size);
        }
        let size = stream.decoded_data().map_or(0, |data| data.len() as u64);
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
                    Diagnostic::new(codes::PDF_FONT_ASSUMED).at(place.clone()),
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
    fn font(
        &mut self,
        resources: &Resources<'a>,
        name: &Name<'_>,
        cx: &mut Cx<'_>,
    ) -> Result<Selected, Stop> {
        let Some(dict) = resources.get_font(name) else {
            let name = name_text(name.as_ref());
            self.report_once(
                Diagnostic::new(codes::PDF_FONT_MISSING)
                    .with("name", name.as_str())
                    .at(self.place.clone()),
                name,
                cx,
            );
            return Ok(Selected::Fallback);
        };
        let settings = self.settings;
        let font = self
            .fonts
            .font(&dict, &settings.font_resolver, &settings.cmap_resolver);
        let id = dict.obj_id();
        let base = dict
            .get::<Name<'_>>(b"BaseFont")
            .map_or_else(|| name_text(name.as_ref()), |base| name_text(base.as_ref()));
        let value = id.map_or_else(
            || base.clone(),
            |id| format!("{} {}", id.obj_number, id.gen_number),
        );
        let Some(font) = font else {
            let place = self.record(id);
            self.report_once(
                Diagnostic::new(codes::PDF_FONT_UNSUPPORTED)
                    .with("name", base)
                    .at(place),
                value,
                cx,
            );
            return Ok(Selected::Fallback);
        };
        if !font.is_horizontal() && self.is_visible() && self.shifted(&dict, cx)? {
            let place = self.record(id);
            self.report_once(
                Diagnostic::new(codes::PDF_VERTICAL_ORIGIN_APPROXIMATED)
                    .with("name", base)
                    .at(place),
                value,
                cx,
            );
        }
        Ok(Selected::Loaded)
    }

    /// 縦書きのフォントの原点の移動（9.7.4.3）が、図面モデルに移すときに仮定する既定（字形の幅 1,000、`DW2` の
    /// `[880 −1000]`）と違いうるか。子孫のフォントの `DW`・`W`・`DW2`・`W2` を見る（ページの中で 1 度だけ）
    fn shifted(&mut self, font: &Dict<'_>, cx: &mut Cx<'_>) -> Result<bool, Stop> {
        let id = font.obj_id().map(key);
        if let Some(shifted) = id.and_then(|id| self.vertical.get(&id)) {
            return Ok(*shifted);
        }
        let descendant = font
            .get::<Array<'_>>(b"DescendantFonts")
            .and_then(|fonts| fonts.iter::<Dict<'_>>().next())
            .unwrap_or_default();
        let width = descendant
            .get::<f32>(b"DW")
            .is_some_and(|width| width != 1000.0);
        let origin = descendant
            .get::<[f32; 2]>(b"DW2")
            .is_some_and(|[y, _]| y != 880.0);
        let mut shifted = width || origin || descendant.contains_key(b"W2");
        // W は、`c [w1 w2 …]` か `c_first c_last w` の並び（9.7.4.3）。配列の外では、数の 3 つ目ごとに幅が来る
        if let Some(widths) = descendant.get::<Array<'_>>(b"W").filter(|_| !shifted) {
            let mut numbers = 0;
            for item in widths.iter::<Object<'_>>() {
                cx.check(Kind::Work, 1)?;
                match item {
                    Object::Array(group) => {
                        numbers = 0;
                        shifted = group.iter::<f32>().any(|width| width != 1000.0);
                    }
                    Object::Number(width) => {
                        numbers += 1;
                        if numbers == 3 {
                            numbers = 0;
                            shifted = width.as_f64() != 1000.0;
                        }
                    }
                    _ => {}
                }
                if shifted {
                    break;
                }
            }
        }
        if let Some(id) = id {
            self.vertical.insert(id, shifted);
        }
        Ok(shifted)
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

    /// XObject の表示グループ（`OC`、8.11.3.3）で、XObject が見えるか。表示グループがなければ `None`。見える内容は、
    /// XObject の表示グループのレイヤに分けない
    fn optional(&mut self, dict: &Dict<'_>, place: &Ref, cx: &mut Cx<'_>) -> Option<bool> {
        let properties = dict.get::<Dict<'_>>(b"OC")?;
        let (shown, ocmd) = match dict.get_ref(b"OC") {
            Some(reference) => self.membership(&properties, key(reference)),
            None => (self.ocmd(&properties), true),
        };
        if shown {
            self.report(codes::PDF_OPTIONAL_CONTENT_APPROXIMATED, place.clone(), cx);
        } else if ocmd {
            self.report(codes::PDF_MEMBERSHIP_DROPPED, place.clone(), cx);
        }
        Some(shown)
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
                let mut cost = Cost::default();
                let extra = self.image(dict, cx)?;
                spend(&mut cost, extra.ops, extra.bytes, cx)?;
                Ok(cost)
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
    /// フォームと、展開できないフォームと、入れ子が [`MAX_DEPTH`] 段より深いフォームは描かない
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
        let mut cost = Cost::default();
        spend(&mut cost, FORM_WORK, bytes, cx)?;
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
        if !visible {
            return Ok(cost);
        }
        if depth >= MAX_DEPTH {
            self.report(codes::PDF_NESTING_DROPPED, place.clone(), cx);
            return Ok(cost);
        }
        // hayro-interpret は、表示グループを持つフォームの中身を、その表示グループの入れ子の中で解釈する
        let optional = self.optional(dict, place, cx);
        if let Some(shown) = optional {
            self.visible.push(shown);
            if !shown {
                self.end_marked();
                return Ok(cost);
            }
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
        if let Some(known) = call.and_then(|call| self.calls.get(&call)).cloned() {
            spend(&mut cost, known.cost.ops, known.cost.bytes, cx)?;
            // 中の診断も、呼んだ回数だけ数える
            for diagnostic in known.diagnostics.iter() {
                self.emit(diagnostic.clone(), cx);
            }
        } else {
            self.recording.push(Diagnostics::new());
            let walked = self.content(
                &data,
                own.as_ref().unwrap_or(resources),
                owner,
                depth + 1,
                font,
                place,
                cx,
            );
            let recorded = self.recording.pop().unwrap_or_default();
            let (inner, balanced) = walked?;
            if let Some(outer) = self.recording.last_mut() {
                for diagnostic in recorded.iter() {
                    outer.report(diagnostic.clone());
                }
            }
            cost.add(inner);
            // 印つきの内容の入れ子が釣り合わないフォームは、呼ぶ所の入れ子で中身の見え方が変わるので、覚えない
            if let Some(call) = call.filter(|_| balanced) {
                let known = Known {
                    cost: inner,
                    diagnostics: recorded.into_vec().into(),
                };
                self.calls.insert(call, known);
            }
        }
        if optional.is_some() {
            self.end_marked();
        }
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
