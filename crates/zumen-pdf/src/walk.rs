//! ページを解釈する前の走査（決定 0010・0256）。
//!
//! hayro-interpret の解釈は、予算でも中断の合図でも止まらない。フォームの XObject が次の段のフォームを 2 回ずつ
//! 呼ぶと、解釈する演算子の数は段の数の冪で増える。そこで、ページを渡す前に、内容のストリームを hayro-interpret と
//! 同じ読み手（hayro-syntax の `TypedIter`）でたどり、hayro-interpret が解釈する手間を `Work` に、展開する
//! バイト数を `DecodedBytes` に、先に数える。同じフォームを同じ文脈で呼ぶ所は、1 度だけたどる。
//!
//! hayro-interpret がスタックかメモリを使い切るおそれのあるページ（入れ子の深すぎる関数と色空間、範囲の広すぎる
//! CID フォントの幅）は、解釈しない。
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
/// 切り抜きにする文字（描画モード 4〜7、9.3.6）の 1 バイトの手間。字形の輪郭を切り抜きの経路にする
const CLIP_GLYPH_WORK: u64 = 40;
/// 画像（`Do` と、内容の中の画像）を 1 回描く手間。画像を作り、下絵の要素にする（1 画素の画像で、およそ 16 倍）
const IMAGE_WORK: u64 = 16;
/// シェーディング（`sh`）を 1 回塗る手間。関数と色空間を作り、塗る範囲を求める（軸のシェーディングで、
/// およそ 16 倍）
const SHADING_WORK: u64 = 16;
/// 引いたオブジェクトの、この数のバイトを 1 と数える。hayro-syntax は、資源を引くたびに辞書を読み直し、
/// hayro-interpret は、フォームを呼ぶたびに資源の辞書を読み直す
const LOOKUP_BYTES: u64 = 4;
/// CID フォントの幅（`W`・`W2`、9.7.4.3）の 1 項目を表に入れる手間。hayro-font は、範囲の CID を 1 つずつ入れる
const WIDTH_WORK: u64 = 2;

/// 展開し直すときに、1 度に書き出すバイト数
const INFLATE_CHUNK: usize = 8192;

/// 関数（7.10）と色空間（8.6）の入れ子をたどる深さの上限。hayro-interpret は、どちらも深さの上限なしに再帰して
/// 作り、スタックを使い切るとプロセスが落ちるので、これより深い入れ子を使うページは解釈しない
const NESTED_DEPTH: u32 = 16;
/// Type 4 の関数の手続き（`{` `}`、7.10.5）の入れ子の深さの上限。hayro-interpret は、手続きを深さの上限なしに
/// 再帰して読み、当てるので、これより深い手続きを使うページは解釈しない。`if`・`ifelse` を重ねた手続きは、関数を
/// つなぐ入れ子より深く書かれうるので、[`NESTED_DEPTH`] より多めにする
const PROCEDURE_DEPTH: u32 = 64;
/// CID フォントの幅の項目の数の上限。これより多い項目（範囲の CID の数）を持つフォントを使うページは解釈しない。
/// 2 バイトの符号で選ぶ CID は 65,536 個までなので、その 4 倍とする
const MAX_WIDTHS: u64 = 1 << 18;
/// メッシュのシェーディング（型 4〜7、8.7.4.5.5〜8.7.4.5.8）の展開したバイト数に掛ける数。hayro-interpret は、
/// 使うたびに三角形とパッチをすべて作り、展開したバイト数のおよそ 90〜120 倍のメモリを使う
const MESH_FACTOR: u64 = 128;

/// 内容のストリームの後ろに足す、終わりの印の演算子
const END: &[u8] = b"zumen-end";

/// 走査の結果
#[derive(Clone, Debug, Default)]
pub(crate) struct Survey {
    /// 転送関数（10.4）を持つ拡張グラフィックス状態を使う
    pub(crate) transfer: bool,
    /// hayro-interpret がスタックかメモリを使い切るおそれがあるので、ページを解釈しない。値は、理由の
    /// オブジェクト（入れ子の深すぎる関数か色空間、範囲の広すぎる幅を持つフォント）の場所
    pub(crate) refused: Option<Ref>,
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
    /// 選ばずに文字を描いたので、hayro-interpret が Helvetica を選んだ
    Assumed,
    /// hayro-interpret が Helvetica で読む（資源にないフォントと、読めないフォント）
    Fallback,
    /// 読めるフォント
    Loaded,
    /// 読めるフォントのうち、原点の移動が既定と違いうる縦書きのフォント（間接オブジェクト）
    Shifted((u32, i32)),
}

/// 走査が追う、グラフィックス状態の一部（`q`・`Q` で保存する、8.4.2）。hayro-interpret の手間を変える
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Graphics {
    font: Selected,
    /// 破線の配列（`d`、8.4.3.6）の長さ。hayro-interpret は、線と文字を描くたびに写す
    dash: u64,
    /// 文字を切り抜きにする（描画モード 4〜7、9.3.6）
    clip: bool,
}

impl Default for Graphics {
    fn default() -> Self {
        Self {
            font: Selected::Nothing,
            dash: 0,
            clip: false,
        }
    }
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
    /// 呼ぶ所のグラフィックス状態
    state: Graphics,
}

/// 名前で引く色空間（8.6.3）。hayro-interpret は、文書の中で 1 度だけ作る
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Space {
    /// 間接オブジェクトの色空間
    Object((u32, i32)),
    /// 資源の辞書に直接書いた色空間。資源の持ち主と名前
    Entry(Owner, Vec<u8>),
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
    // 空白だけのデータは、hayro-syntax が空のバイト列にするので、何も失わない
    if length == 0 && raw.iter().all(u8::is_ascii_whitespace) {
        return false;
    }
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

/// 文字列を描く手間（[`GLYPH_WORK`]・[`CLIP_GLYPH_WORK`]）。1 バイトを 1 字と数える（2 バイトの CMap では多めに
/// なる）。hayro-interpret は、文字を描くたびに破線の配列も写す
fn glyph_work(strings: &[&[u8]], state: Graphics) -> u64 {
    let bytes = strings.iter().map(|bytes| bytes.len() as u64).sum::<u64>();
    let weight = if state.clip {
        CLIP_GLYPH_WORK
    } else {
        GLYPH_WORK
    };
    bytes.saturating_mul(weight).saturating_add(state.dash)
}

/// 引いたオブジェクトを読み直す手間（[`LOOKUP_BYTES`]）
fn lookup_work(data: &[u8]) -> u64 {
    (data.len() as u64).div_ceil(LOOKUP_BYTES)
}

/// オブジェクトの、辞書か配列のバイト列。ストリームは辞書だけ
fn object_data<'x>(object: &Object<'x>) -> &'x [u8] {
    match object {
        Object::Dict(dict) => dict.data(),
        Object::Stream(stream) => stream.dict().data(),
        Object::Array(array) => array.data(),
        _ => &[],
    }
}

/// 資源の辞書（7.8.3）を読み直す手間。hayro-syntax の `Resources::new` は、7 つの種類の辞書を引く
fn resources_work(resources: &Resources<'_>) -> u64 {
    [
        &resources.ext_g_states,
        &resources.fonts,
        &resources.properties,
        &resources.color_spaces,
        &resources.x_objects,
        &resources.patterns,
        &resources.shadings,
    ]
    .iter()
    .map(|dict| lookup_work(dict.data()))
    .sum()
}

/// 破線の配列（`d` の 1 つ目の引数か、拡張グラフィックス状態の `D` の 1 つ目の項目）の長さ
fn dash_length(array: &Array<'_>) -> u64 {
    array.raw_iter().count() as u64
}

/// Type 4 の関数の手続き（`{` `}`、7.10.5.2）の入れ子が、[`PROCEDURE_DEPTH`] 段に収まるか。注釈（`%` から行の
/// 終わりまで）の中の括弧は、hayro-interpret と同じく数えない
fn procedure_fits(data: &[u8]) -> bool {
    let mut depth = 0u32;
    let mut comment = false;
    for &byte in data {
        match byte {
            b'\r' | b'\n' => comment = false,
            _ if comment => {}
            b'%' => comment = true,
            b'{' => {
                depth += 1;
                if depth > PROCEDURE_DEPTH {
                    return false;
                }
            }
            b'}' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    true
}

/// 幅の配列（`W`・`W2`、9.7.4.3）が表に入れる項目の数を、`count` に足す。並びは `c [値 …]` か
/// `c_first c_last 値`。`values` は、1 つの CID の値の数（`W` は 1、`W2` は 3）。[`MAX_WIDTHS`] を超えたら、そこで
/// 数えるのをやめる
fn width_entries(
    widths: &Array<'_>,
    values: usize,
    mut count: u64,
    cx: &mut Cx<'_>,
) -> Result<u64, Stop> {
    let mut numbers: Vec<f64> = Vec::with_capacity(values + 2);
    for item in widths.iter::<Object<'_>>() {
        cx.check(Kind::Work, 1)?;
        match item {
            Object::Array(group) => {
                let listed = group.raw_iter().count() / values;
                count = count.saturating_add(listed as u64);
                numbers.clear();
            }
            Object::Number(number) => {
                numbers.push(number.as_f64());
                if numbers.len() == values + 2 {
                    // 範囲の CID の数。u64 に収まらない数は、u64 の最大にする
                    if let [first, last, ..] = numbers[..]
                        && last >= first
                    {
                        count = count.saturating_add(((last - first) as u64).saturating_add(1));
                    }
                    numbers.clear();
                }
            }
            _ => numbers.clear(),
        }
        if count > MAX_WIDTHS {
            break;
        }
    }
    Ok(count)
}

/// 幅の配列の値が、すべて既定（`W` は字形の幅 1,000、`W2` は位置のベクトル `(500, 880)`）か。並びは
/// [`width_entries`] と同じ
fn default_widths(widths: &Array<'_>, values: usize, cx: &mut Cx<'_>) -> Result<bool, Stop> {
    let normal = |entry: &[f32]| match entry {
        [width] => *width == 1000.0,
        [_, x, y] => *x == 500.0 && *y == 880.0,
        _ => true,
    };
    let mut numbers: Vec<f32> = Vec::with_capacity(values + 2);
    for item in widths.iter::<Object<'_>>() {
        cx.check(Kind::Work, 1)?;
        match item {
            Object::Array(group) => {
                let group: Vec<f32> = group.iter::<f32>().collect();
                if !group.chunks_exact(values).all(normal) {
                    return Ok(false);
                }
                numbers.clear();
            }
            Object::Number(number) => {
                numbers.push(number.as_f32());
                if numbers.len() == values + 2 {
                    if !numbers.get(2..).is_none_or(normal) {
                        return Ok(false);
                    }
                    numbers.clear();
                }
            }
            _ => numbers.clear(),
        }
    }
    Ok(true)
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
    /// 原点の移動が既定と違いうる縦書きのフォント → 診断の名前（`BaseFont`）と場所（ページごと）
    origins: BTreeMap<(u32, i32), (String, Ref)>,
    /// CID フォント → 幅の項目の数（ページごと）
    widths: BTreeMap<(u32, i32), u64>,
    /// 作る手間を数えた、名前で引く色空間（ページごと）
    spaces: BTreeSet<Space>,
    /// ページを解釈しない理由のオブジェクトの場所（[`Survey::refused`]）
    refused: Option<Ref>,
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
            origins: BTreeMap::new(),
            widths: BTreeMap::new(),
            spaces: BTreeSet::new(),
            refused: None,
            recording: Vec::new(),
            forms: BTreeMap::new(),
            sizes: BTreeMap::new(),
            visible: Vec::new(),
            closed: usize::MAX,
            reported: BTreeSet::new(),
            transfer: false,
        }
    }

    /// ページを走査し、hayro-interpret がする作業を予算に数える。`place` は、ページの最初の内容のストリーム。
    /// ページを解釈しないと決めたら（[`Survey::refused`]）、そこでたどるのをやめ、そのページで見つけた診断は
    /// 返さない（中身を移さないことだけを、呼ぶ側が返す）
    pub(crate) fn page(
        &mut self,
        page: &Page<'a>,
        place: &Ref,
        cx: &mut Cx<'_>,
    ) -> Result<Survey, Stop> {
        let mut found = Diagnostics::with_max_places(cx.diag.max_places());
        let surveyed = self.survey(page, place, &mut Cx::new(cx.budget, &mut found, cx.cancel));
        if surveyed.is_err() || self.refused.is_none() {
            for diagnostic in found.into_vec() {
                cx.report(diagnostic);
            }
        }
        surveyed?;
        Ok(Survey {
            transfer: self.transfer,
            refused: self.refused.take(),
        })
    }

    fn survey(&mut self, page: &Page<'a>, place: &Ref, cx: &mut Cx<'_>) -> Result<(), Stop> {
        self.place = place.clone();
        self.calls.clear();
        self.costs.clear();
        self.vertical.clear();
        self.origins.clear();
        self.widths.clear();
        self.spaces.clear();
        self.refused = None;
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
            Graphics::default(),
            place,
            cx,
        )?;
        Ok(())
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

    /// ページを解釈しないことにする（[`Survey::refused`]）。場所は、最初に見つけた理由のオブジェクト
    fn refuse(&mut self, place: Ref) {
        if self.refused.is_none() {
            self.refused = Some(place);
        }
    }

    /// 内容のストリーム（終わりの印を足したもの）をたどる。`state` は、呼ぶ所のグラフィックス状態。2 つ目は、
    /// 印つきの内容の入れ子が釣り合うか。釣り合うのは、始めた所より外の入れ子を閉じず、始めた所に戻って終わる
    /// とき。このとき、見えるかの重なりは、たどる前と同じになり、たどった中身は、呼ぶ所が見えることだけで決まる
    #[allow(clippy::too_many_arguments)]
    fn content(
        &mut self,
        data: &[u8],
        resources: &Resources<'a>,
        owner: Owner,
        depth: u32,
        state: Graphics,
        place: &Ref,
        cx: &mut Cx<'_>,
    ) -> Result<(Cost, bool), Stop> {
        let mut cost = Cost::default();
        // q・Q で保存する、グラフィックス状態の一部
        let mut states = vec![state];
        let start = self.visible.len();
        let outer = std::mem::replace(&mut self.closed, usize::MAX);
        let mut ended = false;
        let mut ops = TypedIter::new(data);
        while let Some(op) = ops.next() {
            // ページを解釈しないと決めたら、たどるのをやめる
            if is_end(&op) || self.refused.is_some() {
                ended = true;
                break;
            }
            spend(&mut cost, 1, 0, cx)?;
            let current = states.last().copied().unwrap_or_default();
            match op {
                TypedInstruction::SaveState(_) => states.push(current),
                TypedInstruction::RestoreState(_) => {
                    if states.len() > 1 {
                        states.pop();
                    }
                }
                TypedInstruction::DashPattern(dash) => {
                    let length = dash_length(dash.0);
                    spend(&mut cost, length, 0, cx)?;
                    if let Some(last) = states.last_mut() {
                        last.dash = length;
                    }
                }
                TypedInstruction::TextRenderingMode(mode) => {
                    if let Some(last) = states.last_mut() {
                        last.clip = (4.0..=7.0).contains(&mode.0.as_f64());
                    }
                }
                TypedInstruction::SetGraphicsState(name) => {
                    spend(&mut cost, STATE_WORK, 0, cx)?;
                    if let Some(state) = resources.get_ext_g_state(name.0) {
                        let extra = self.state(&state, cx)?;
                        spend(
                            &mut cost,
                            lookup_work(state.data()).saturating_add(extra.ops),
                            extra.bytes,
                            cx,
                        )?;
                        self.apply_state(&state, &mut states, &mut cost, cx)?;
                    }
                }
                TypedInstruction::TextFont(selected) => {
                    let selected = self.font(resources, selected.0, &mut cost, cx)?;
                    if let Some(last) = states.last_mut() {
                        last.font = selected;
                    }
                }
                TypedInstruction::ShowText(text) => {
                    let strings = [text.0.as_bytes()];
                    self.show(&mut states, true, &strings, &mut cost, place, cx)?;
                }
                TypedInstruction::ShowTexts(texts) => {
                    let strings: Vec<PdfString<'_>> = texts
                        .0
                        .iter::<Object<'_>>()
                        .filter_map(|item| item.into_string())
                        .collect();
                    let strings: Vec<&[u8]> = strings.iter().map(PdfString::as_bytes).collect();
                    self.show(&mut states, true, &strings, &mut cost, place, cx)?;
                }
                TypedInstruction::NextLineAndShowText(text) => {
                    let strings = [text.0.as_bytes()];
                    self.show(&mut states, false, &strings, &mut cost, place, cx)?;
                }
                TypedInstruction::ShowTextWithParameters(text) => {
                    let strings = [text.2.as_bytes()];
                    self.show(&mut states, false, &strings, &mut cost, place, cx)?;
                }
                // hayro-interpret は、線を描くたびに破線の配列を写す
                TypedInstruction::StrokePath(_)
                | TypedInstruction::CloseAndStrokePath(_)
                | TypedInstruction::FillAndStrokeNonZero(_)
                | TypedInstruction::FillAndStrokeEvenOdd(_)
                | TypedInstruction::CloseFillAndStrokeNonZero(_)
                | TypedInstruction::CloseFillAndStrokeEvenOdd(_) => {
                    spend(&mut cost, current.dash, 0, cx)?;
                }
                TypedInstruction::BeginMarkedContentWithProperties(marked) => {
                    self.begin_marked(resources, marked.1, &mut cost, place, cx)?;
                }
                TypedInstruction::BeginMarkedContent(_) => {
                    let visible = self.is_visible();
                    self.visible.push(visible);
                }
                TypedInstruction::EndMarkedContent(_) => self.end_marked(),
                TypedInstruction::XObject(x_object) => match resources.get_x_object(x_object.0) {
                    Some(stream) => {
                        let called =
                            self.x_object(&stream, resources, owner, depth, current, cx)?;
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
                    spend(&mut cost, IMAGE_WORK, 0, cx)?;
                    let dict = image.0.dict();
                    if self.is_visible() && !has_size(dict) {
                        self.report(codes::PDF_CONTENT_INVALID, place.clone(), cx);
                    }
                    let space = dict
                        .get::<Object<'_>>(b"CS")
                        .or_else(|| dict.get::<Object<'_>>(b"ColorSpace"));
                    if let Some(space) = space {
                        let extra = self.image_space(&space, resources, owner, cx)?;
                        spend(&mut cost, extra.ops, extra.bytes, cx)?;
                    }
                }
                TypedInstruction::Shading(shading) => {
                    if let Some(shading) = resources.get_shading(shading.0) {
                        let extra = self.shading(&shading, cx)?;
                        spend(
                            &mut cost,
                            SHADING_WORK
                                .saturating_add(lookup_work(object_data(&shading)))
                                .saturating_add(extra.ops),
                            extra.bytes,
                            cx,
                        )?;
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
                TypedInstruction::ColorSpaceStroke(space) => {
                    let extra = self.named_space(resources, owner, space.0, cx)?;
                    spend(&mut cost, extra.ops, extra.bytes, cx)?;
                }
                TypedInstruction::ColorSpaceNonStroke(space) => {
                    let extra = self.named_space(resources, owner, space.0, cx)?;
                    spend(&mut cost, extra.ops, extra.bytes, cx)?;
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

    /// 拡張グラフィックス状態が変えるもののうち、走査が追うもの: 破線（`D`）とフォント（`Font`、8.4.5 の
    /// 表 57）。hayro-interpret は、状態を当てるたびに破線の配列を読み、フォントを引く
    fn apply_state(
        &mut self,
        state: &Dict<'a>,
        states: &mut [Graphics],
        cost: &mut Cost,
        cx: &mut Cx<'_>,
    ) -> Result<(), Stop> {
        let dash = state
            .get::<Array<'_>>(b"D")
            .and_then(|dash| dash.iter::<Object<'_>>().next())
            .and_then(|array| array.into_array());
        if let Some(dash) = dash {
            let length = dash_length(&dash);
            spend(cost, length, 0, cx)?;
            if let Some(last) = states.last_mut() {
                last.dash = length;
            }
        }
        let font = state
            .get::<Array<'_>>(b"Font")
            .and_then(|font| font.iter::<Object<'_>>().next())
            .and_then(|font| font.into_dict());
        if let Some(font) = font {
            let base = font
                .get::<Name<'_>>(b"BaseFont")
                .map(|base| name_text(base.as_ref()))
                .unwrap_or_default();
            let selected = self.font_dict(&font, base, cost, cx)?;
            if let Some(last) = states.last_mut() {
                last.font = selected;
            }
        }
        Ok(())
    }

    /// 拡張グラフィックス状態（8.4.5）を当てる手間。転送関数を見つける。hayro-interpret は、状態を当てるたびに、
    /// 転送関数（`TR`・`TR2`、10.4）を作り、ソフトマスク（11.6.5.2）のグループを展開し、その色空間と転送関数を作る。
    /// 転送関数は、`TR` と `TR2` の鍵のそれぞれで作り直す
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
            let made = self.function(&function, 0, cx)?;
            for key in [b"TR".as_slice(), b"TR2"] {
                if state.contains_key(key) {
                    cost.add(made);
                }
            }
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

    /// パターン（8.7.3）を選ぶ手間。hayro-interpret は、選ぶたびにパターンを引き直し、シェーディングのパターンなら
    /// シェーディングを作る。タイルのパターンは、塗るときまで中身を展開しない
    fn pattern(
        &mut self,
        resources: &Resources<'a>,
        name: &Name<'_>,
        cx: &mut Cx<'_>,
    ) -> Result<Cost, Stop> {
        let Some(pattern) = resources.get_pattern(name) else {
            return Ok(Cost::default());
        };
        let mut cost = Cost {
            ops: lookup_work(object_data(&pattern)),
            bytes: 0,
        };
        if let Object::Dict(pattern) = &pattern
            && let Some(shading) = pattern.get::<Object<'_>>(b"Shading")
        {
            cost.ops = cost.ops.saturating_add(lookup_work(object_data(&shading)));
            cost.add(self.shading(&shading, cx)?);
        }
        Ok(cost)
    }

    /// シェーディング（8.7.4.3）を作る手間。hayro-interpret は、使うたびに作り直し、ストリームと関数を展開し、
    /// 色空間を作る。メッシュ（型 4〜7）は、三角形とパッチをすべて作るので、展開したバイト数の [`MESH_FACTOR`] 倍を
    /// 数える
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
            let size = self.size(stream, cx)?;
            let mesh = dict
                .get::<u8>(b"ShadingType")
                .is_some_and(|kind| (4..=7).contains(&kind));
            let size = if mesh {
                size.saturating_mul(MESH_FACTOR)
            } else {
                size
            };
            cost.bytes = cost.bytes.saturating_add(size);
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

    /// 画像の XObject を描く手間。hayro-interpret は、描くたびに画像の色空間を作る。名前の色空間は、呼ぶ所の資源で
    /// 引く。画素は、描くときまで展開しない
    fn image(
        &mut self,
        dict: &Dict<'_>,
        resources: &Resources<'a>,
        owner: Owner,
        cx: &mut Cx<'_>,
    ) -> Result<Cost, Stop> {
        let space = dict
            .get::<Object<'_>>(b"CS")
            .or_else(|| dict.get::<Object<'_>>(b"ColorSpace"));
        if let Some(Object::Name(name)) = &space {
            return self.named_space(resources, owner, name, cx);
        }
        let id = dict.obj_id().map(key);
        if let Some(cost) = id.and_then(|id| self.costs.get(&(Role::Image, id))) {
            return Ok(*cost);
        }
        let cost = match space {
            Some(space) => self.color_space(&space, 0, cx)?,
            None => Cost::default(),
        };
        if let Some(id) = id {
            self.costs.insert((Role::Image, id), cost);
        }
        Ok(cost)
    }

    /// 内容の中の画像（8.9.7）の色空間を作る手間。配列は使うたびに作り、名前は資源で引く
    fn image_space(
        &mut self,
        space: &Object<'_>,
        resources: &Resources<'a>,
        owner: Owner,
        cx: &mut Cx<'_>,
    ) -> Result<Cost, Stop> {
        match space {
            Object::Name(name) => self.named_space(resources, owner, name, cx),
            _ => self.color_space(space, 0, cx),
        }
    }

    /// 名前で引く色空間（`cs`・`CS`・画像の名前の色空間、8.6.3）を使う手間。hayro-interpret は、使うたびに資源を
    /// 引いて読み直し、そのバイト列から表の鍵を作る。色空間そのものは、文書の中で 1 度だけ作るので、作る手間は
    /// ページで 1 度だけ数える（使う手間には入れない）。資源にない名前（装置の色空間など）は、何もしない
    fn named_space(
        &mut self,
        resources: &Resources<'a>,
        owner: Owner,
        name: &Name<'_>,
        cx: &mut Cx<'_>,
    ) -> Result<Cost, Stop> {
        let Some(space) = resources.get_color_space(name) else {
            return Ok(Cost::default());
        };
        let cost = Cost {
            ops: lookup_work(object_data(&space)),
            bytes: 0,
        };
        let entry = match resources.color_spaces.get_ref(name.as_ref()) {
            Some(reference) => Some(Space::Object(key(reference))),
            None if owner != Owner::Direct => Some(Space::Entry(owner, name.as_ref().to_vec())),
            None => None,
        };
        if entry
            .as_ref()
            .is_none_or(|entry| !self.spaces.contains(entry))
        {
            let made = self.color_space(&space, 0, cx)?;
            cx.check(Kind::Work, made.ops)?;
            cx.check(Kind::DecodedBytes, made.bytes)?;
            if let Some(entry) = entry {
                self.spaces.insert(entry);
            }
        }
        Ok(cost)
    }

    /// 関数（7.10）を作る手間。hayro-interpret は、関数を使う演算子ごとに関数を作り直し、ストリームの関数（Type 0・
    /// Type 4）を展開する。つなぐ関数（Type 3）の `Functions` と、関数の配列の中もたどる。[`NESTED_DEPTH`] より深い
    /// 入れ子は、ページを解釈しないことにする
    fn function(
        &mut self,
        function: &Object<'_>,
        depth: u32,
        cx: &mut Cx<'_>,
    ) -> Result<Cost, Stop> {
        cx.check(Kind::Work, 1)?;
        let mut cost = Cost { ops: 1, bytes: 0 };
        if depth > NESTED_DEPTH {
            let id = match function {
                Object::Dict(dict) => dict.obj_id(),
                Object::Stream(stream) => Some(stream.obj_id()),
                _ => None,
            };
            let place = self.record(id);
            self.refuse(place);
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

    /// 直接書いた色空間（8.6）を作る手間。hayro-interpret は、配列の色空間を使うたびに作り直す。分版（Separation・
    /// DeviceN、8.6.6.4・8.6.6.5）の代わりの色空間と色変換の関数、Indexed の基の色空間と表（8.6.6.3）、パターンの
    /// 基の色空間（8.6.6.2）、ICCBased の代わりの色空間（`Alternate`、8.6.5.5）をたどる。[`NESTED_DEPTH`] より深い
    /// 入れ子は、ページを解釈しないことにする
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
        if depth > NESTED_DEPTH {
            let place = self.place.clone();
            self.refuse(place);
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
            Some(b"ICCBased") => {
                let alternate = items
                    .next()
                    .and_then(|profile| profile.into_stream())
                    .and_then(|profile| profile.dict().get::<Object<'_>>(b"Alternate"));
                if let Some(alternate) = alternate {
                    cost.add(self.color_space(&alternate, depth + 1, cx)?);
                }
            }
            _ => {}
        }
        Ok(cost)
    }

    /// ストリームを展開したバイト数。ページの中では 1 度だけ展開する。hayro-interpret と同じく、画像のフィルター
    /// （DCT など）は展開しない。手続きの入れ子が [`PROCEDURE_DEPTH`] 段より深い Type 4 の関数（7.10.5）は、ページを
    /// 解釈しないことにする
    fn size(&mut self, stream: &Stream<'_>, cx: &mut Cx<'_>) -> Result<u64, Stop> {
        let id = stream.dict().obj_id().map(key);
        if let Some(size) = id.and_then(|id| self.sizes.get(&id)) {
            return Ok(*size);
        }
        let data = stream.decoded_data().ok();
        let size = data.as_ref().map_or(0, |data| data.len() as u64);
        cx.check(Kind::DecodedBytes, size)?;
        let program = stream.dict().get::<u8>(b"FunctionType") == Some(4);
        if program && data.is_some_and(|data| !procedure_fits(&data)) {
            let place = self.record(stream.dict().obj_id());
            self.refuse(place);
        }
        if let Some(id) = id {
            self.sizes.insert(id, size);
        }
        Ok(size)
    }

    /// 文字を描く（`Tj`・`TJ`・`'`・`"`、9.4.2）。フォントを選ばずに描く文字を、hayro-interpret は `Tj`・`TJ` なら
    /// Helvetica で読み、`'`・`"` なら描かない。Helvetica で読む文字列は、ASCII のバイトだけのものしか描かない。
    /// `fallback` は、フォントを選んでいなければ Helvetica を選ぶ演算子か。フォントの診断は、見える所で文字を
    /// 描くときに返す
    fn show(
        &mut self,
        states: &mut [Graphics],
        fallback: bool,
        strings: &[&[u8]],
        cost: &mut Cost,
        place: &Ref,
        cx: &mut Cx<'_>,
    ) -> Result<(), Stop> {
        let Some(state) = states.last_mut() else {
            return Ok(());
        };
        spend(cost, glyph_work(strings, *state), 0, cx)?;
        // hayro-interpret は、見えるかによらず Helvetica を選ぶ
        if state.font == Selected::Nothing && fallback {
            state.font = Selected::Assumed;
        }
        let font = state.font;
        if !self.is_visible() || strings.iter().all(|bytes| bytes.is_empty()) {
            return Ok(());
        }
        match font {
            Selected::Assumed => self.report_once(
                Diagnostic::new(codes::PDF_FONT_ASSUMED).at(place.clone()),
                String::new(),
                cx,
            ),
            Selected::Shifted(id) => {
                if let Some((name, at)) = self.origins.get(&id).cloned() {
                    self.report_once(
                        Diagnostic::new(codes::PDF_VERTICAL_ORIGIN_APPROXIMATED)
                            .with("name", name)
                            .at(at),
                        format!("{} {}", id.0, id.1),
                        cx,
                    );
                }
            }
            _ => {}
        }
        let dropped = match font {
            Selected::Loaded | Selected::Shifted(_) => 0,
            Selected::Assumed | Selected::Fallback => {
                strings.iter().filter(|bytes| !bytes.is_ascii()).count()
            }
            Selected::Nothing => strings.iter().filter(|bytes| !bytes.is_empty()).count(),
        };
        for _ in 0..dropped {
            self.report(codes::PDF_TEXT_DROPPED, place.clone(), cx);
        }
        Ok(())
    }

    /// フォントを選ぶ（`Tf`、9.3.1）。資源にないフォントと、hayro-interpret が読めないフォントは、Helvetica で読む
    fn font(
        &mut self,
        resources: &Resources<'a>,
        name: &Name<'_>,
        cost: &mut Cost,
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
        let base = dict
            .get::<Name<'_>>(b"BaseFont")
            .map_or_else(|| name_text(name.as_ref()), |base| name_text(base.as_ref()));
        self.font_dict(&dict, base, cost, cx)
    }

    /// フォントの辞書（`Tf` で選ぶものと、拡張グラフィックス状態の `Font`）を選ぶ。hayro-interpret は、選ぶたびに
    /// 辞書のバイト列から表の鍵を作り、文書の中で 1 度だけフォントを読む。`base` は、診断の名前
    fn font_dict(
        &mut self,
        dict: &Dict<'a>,
        base: String,
        cost: &mut Cost,
        cx: &mut Cx<'_>,
    ) -> Result<Selected, Stop> {
        spend(cost, lookup_work(dict.data()), 0, cx)?;
        let id = dict.obj_id();
        // 幅の項目の多すぎるフォントは、読む前に、ページを解釈しないことにする
        if !self.widths_fit(dict, cx)? {
            let place = self.record(id);
            self.refuse(place);
            return Ok(Selected::Fallback);
        }
        let settings = self.settings;
        let font = self
            .fonts
            .font(dict, &settings.font_resolver, &settings.cmap_resolver);
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
        if font.is_horizontal() || !self.shifted(dict, cx)? {
            return Ok(Selected::Loaded);
        }
        let place = self.record(id);
        match id {
            Some(id) => {
                self.origins.insert(key(id), (base, place));
                Ok(Selected::Shifted(key(id)))
            }
            // 間接オブジェクトでないフォントは、選ぶ所で診断する
            None => {
                if self.is_visible() {
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
        }
    }

    /// CID フォント（9.7）の幅（`W`・`W2`、9.7.4.3）の項目が [`MAX_WIDTHS`] に収まるか。hayro-font が範囲の CID を
    /// 1 つずつ表に入れる手間を数える（ページの中で 1 度だけ）
    fn widths_fit(&mut self, font: &Dict<'_>, cx: &mut Cx<'_>) -> Result<bool, Stop> {
        if font.get::<Name<'_>>(b"Subtype").as_deref() != Some(b"Type0".as_slice()) {
            return Ok(true);
        }
        let id = font.obj_id().map(key);
        if let Some(count) = id.and_then(|id| self.widths.get(&id)) {
            return Ok(*count <= MAX_WIDTHS);
        }
        let descendant = font
            .get::<Array<'_>>(b"DescendantFonts")
            .and_then(|fonts| fonts.iter::<Dict<'_>>().next())
            .unwrap_or_default();
        let mut count = 0;
        for (key, values) in [(b"W".as_slice(), 1), (b"W2", 3)] {
            if let Some(widths) = descendant.get::<Array<'_>>(key) {
                count = width_entries(&widths, values, count, cx)?;
            }
        }
        cx.check(Kind::Work, count.min(MAX_WIDTHS).saturating_mul(WIDTH_WORK))?;
        if let Some(id) = id {
            self.widths.insert(id, count);
        }
        Ok(count <= MAX_WIDTHS)
    }

    /// 縦書きのフォントの原点の移動（9.7.4.3）が、図面モデルに移すときに仮定する既定（字形の幅 1,000、位置の
    /// ベクトル `(500, 880)`）と違いうるか。子孫のフォントの `DW`・`W`・`DW2`・`W2` を見る（ページの中で 1 度だけ）
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
        let mut shifted = width || origin;
        // W は `c [w1 w2 …]` か `c_first c_last w`、W2 は `c [w1y v1x v1y …]` か `c_first c_last w1y v1x v1y` の
        // 並び。字形の幅が 1,000 で、位置のベクトルが (500, 880) なら、既定と同じ
        for (key, values) in [(b"W".as_slice(), 1), (b"W2", 3)] {
            if shifted {
                break;
            }
            if let Some(widths) = descendant.get::<Array<'_>>(key) {
                shifted = !default_widths(&widths, values, cx)?;
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
        cost: &mut Cost,
        place: &Ref,
        cx: &mut Cx<'_>,
    ) -> Result<(), Stop> {
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
                // hayro-interpret は、内容の印のたびに表示グループの辞書を引き直す
                spend(cost, lookup_work(dict.data()), 0, cx)?;
                let (shown, ocmd) = self.membership(&dict, reference);
                if parent && ocmd {
                    self.membership_report(shown, place, cx);
                }
                parent && shown
            }
            None => parent,
        };
        self.visible.push(visible);
        Ok(())
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

    /// XObject を描く（`Do`、8.8）。hayro-interpret は、描くたびに XObject の辞書を読み直す
    fn x_object(
        &mut self,
        stream: &Stream<'a>,
        resources: &Resources<'a>,
        owner: Owner,
        depth: u32,
        state: Graphics,
        cx: &mut Cx<'_>,
    ) -> Result<Cost, Stop> {
        let dict = stream.dict();
        let id = dict.obj_id();
        let place = self.record(id);
        let visible = self.is_visible();
        let mut cost = Cost::default();
        spend(&mut cost, lookup_work(dict.data()), 0, cx)?;
        match dict.get::<Name<'_>>(b"Subtype").as_deref() {
            Some(b"Form") => {
                let called = self.form(stream, id, &place, resources, owner, depth, state, cx)?;
                cost.add(called);
            }
            Some(b"Image") => {
                if visible {
                    if has_size(dict) {
                        self.optional(dict, &place, cx);
                    } else {
                        self.report(codes::PDF_CONTENT_INVALID, place, cx);
                    }
                }
                let extra = self.image(dict, resources, owner, cx)?;
                spend(
                    &mut cost,
                    IMAGE_WORK.saturating_add(extra.ops),
                    extra.bytes,
                    cx,
                )?;
            }
            // PostScript の XObject は描かない（8.8.2）
            Some(b"PS") => {}
            _ => {
                if visible {
                    self.report(codes::PDF_CONTENT_INVALID, place, cx);
                }
            }
        }
        Ok(cost)
    }

    /// フォームの XObject（8.10）。hayro-interpret は、呼ぶたびに展開し、資源の辞書を読み直し、見えれば中身を
    /// 解釈する。`BBox` のないフォームと、展開できないフォームと、入れ子が [`MAX_DEPTH`] 段より深いフォームは描かない
    #[allow(clippy::too_many_arguments)]
    fn form(
        &mut self,
        stream: &Stream<'a>,
        id: Option<ObjectIdentifier>,
        place: &Ref,
        resources: &Resources<'a>,
        owner: Owner,
        depth: u32,
        state: Graphics,
        cx: &mut Cx<'_>,
    ) -> Result<Cost, Stop> {
        let data = self.form_data(stream, id, cx)?;
        let bytes = data.as_ref().map_or(0, |form| {
            form.data.len().saturating_sub(END.len() + 2) as u64
        });
        let dict = stream.dict();
        let own_dict = dict.get::<Dict<'_>>(b"Resources");
        let reread = own_dict.as_ref().map_or(0, |own| lookup_work(own.data()));
        let own = own_dict.map(Resources::new);
        let reread = reread.saturating_add(own.as_ref().map_or(0, resources_work));
        let mut cost = Cost::default();
        spend(&mut cost, FORM_WORK.saturating_add(reread), bytes, cx)?;
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
                state,
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
                state,
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
