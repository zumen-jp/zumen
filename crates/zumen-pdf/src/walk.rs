//! ページを解釈する前の走査（決定 0010・0256）。
//!
//! hayro-interpret の解釈は、予算でも中断の合図でも止まらない。フォームの XObject が次の段のフォームを 2 回ずつ
//! 呼ぶと、解釈する演算子の数は段の数の冪で増える。そこで、ページを渡す前に、内容のストリームを hayro-interpret と
//! 同じ読み手（hayro-syntax の `TypedIter`）でたどり、hayro-interpret が解釈する手間を `Work` に、展開する
//! バイト数を `DecodedBytes` に、先に数える。同じフォームを同じ文脈で呼ぶ所は、1 度だけたどる。hayro-interpret が
//! 作るものの大きさ（写すグラフィックス状態・メッシュ・関数の標本・CID フォントの幅の表・切り抜きにする文字の
//! 輪郭）も、メモリの代わりに `DecodedBytes` に数え、メッシュと切り抜きの輪郭を作る手間と、色を RGB にするときに
//! 関数を当てる手間も `Work` に数える。
//!
//! hayro-interpret がスタックかメモリを使い切るおそれのあるページ（入れ子の深すぎる関数と色空間、範囲の広すぎる
//! CID フォントの幅、行の頂点の数が 0 の格子のメッシュ）と、パニックするおそれのあるページ（大きさの大きすぎる
//! Type 0 の関数）は、解釈しない。
//!
//! あわせて、hayro-interpret が知らせずに捨てるものを診断にする: 展開できないか途中から読めない内容のストリームと、
//! 壊れた XObject、資源にない XObject、入れ子の深すぎるフォーム、資源にないフォントと読めないフォント、代わりの
//! フォントで描かない文字列、縦書きの原点の移動が既定と違うフォント、表示の規則の辞書（OCMD）で隠れる内容、
//! 転送関数。

use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use flate2::{Decompress, FlushDecompress, Status};
use hayro_font::{Font, FontCache, FontGlyph, StandardFont};
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
/// 内容の印（`BDC`、14.6）を始める手間。hayro-interpret は、見えるかの重なりを積み、装置に印と性質の辞書を渡す
const MARKED_WORK: u64 = 4;
/// 切り抜きにする文字（描画モード 4〜7、9.3.6）の輪郭の経路の要素の 1 つを作る手間。hayro-interpret は、字ごとに
/// フォントのプログラムから輪郭を作り、写して切り抜きの経路に足す（65,000 点の字形で、要素の 1 つにおよそ
/// 0.45 µs）
const OUTLINE_WORK: u64 = 3;
/// 切り抜きにする文字の輪郭の経路の要素の 1 つを写す手間。hayro-interpret は、輪郭を `ET` まで貯め、`ET` と、
/// グラフィックス状態を写すときに写す
const CLIP_COPY_WORK: u64 = 1;
/// 経路を塗るか線を描く手間（`f`・`S`・`B` など、8.5.3）。hayro-interpret が経路を装置に渡し、装置が色を RGB に
/// し（DeviceCMYK と ICC の色は、hayro-interpret が moxcms で換算する）、経路を mm に写して要素にする。装置が
/// 経路の要素ごとに数える手間に足す（`0 0 0 1 k 0 0 10 10 re f` で、合わせておよそ 33 倍）
const PAINT_WORK: u64 = 6;
/// メッシュのシェーディングの頂点（型 4・5）かパッチ（型 6・7）の 1 つを読んで、三角形かパッチを作る手間（型 5 の
/// 頂点と、型 6・7 のパッチで、およそ 4〜8 倍）
const MESH_WORK: u64 = 4;
/// Type 0 の関数（7.10.2）の標本の、この数を読む手間を 1 と数える。hayro-interpret は、関数を作るたびに、標本を
/// すべて読んで f32 にする（1 ビットの標本の 100 万個で、およそ 12 ms）
const SAMPLES_PER_WORK: u64 = 8;

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
/// Type 0 の関数（7.10.2）の大きさ（`Size`）の上限。hayro-interpret は標本の位置を f32 で求めるので、これより
/// 大きい大きさでは、丸めた位置が標本の外を指してパニックする（`Type0::eval`）。これより大きい大きさの関数を使う
/// ページは解釈しない
const MAX_SAMPLE_SIZE: u32 = 1 << 24;

// hayro-interpret が作るもののメモリ（`DecodedBytes` に数えるバイト数）。hayro-interpret 0.9.0 の型の大きさか、
// 測ったメモリを、配列を伸ばすときの余りと写しを含めて倍にする。1 つずつ足して伸ばす配列は、伸ばす間に前と
// 後の配列が同時にあるので、要素の型の大きさの 3 倍まで使う（決定 0256）
/// Type 0 の関数の標本の 1 つ（f32）。hayro-interpret は、関数を作るたびに、標本をすべて f32 にして持つ。数の
/// 分からない並びから集めるので、配列を伸ばす間は 3 倍まで使う（測ったメモリは、標本の 1 つでおよそ 12 バイト）
const SAMPLE_BYTES: u64 = 16;
/// 破線の配列の 1 項目（f32。測ったメモリは、写した 1 項目でおよそ 5 バイト）
const DASH_BYTES: u64 = 8;
/// 切り抜きにする文字の輪郭の経路の要素の 1 つ（kurbo の `PathEl` は 56 バイト）。hayro-interpret は、字の並びの
/// 経路と文字の状態の経路の両方に、配列を伸ばしながら足す（測ったメモリは、要素の 1 つでおよそ 224 バイト）
const OUTLINE_BYTES: u64 = 512;
/// 切り抜きに貯めた輪郭を写すときの、経路の要素の 1 つ
const OUTLINE_COPY_BYTES: u64 = 128;
/// CID フォントの幅の表の 1 項目。走査と hayro-interpret が、文書の間、それぞれ表を持つ（`W2` の 1 項目で、2 つの
/// 表を合わせておよそ 76 バイト）
const WIDTH_BYTES: u64 = 160;
/// メッシュのシェーディング（型 4・5）の三角形の 1 つ（216 バイト。配列を伸ばす間は、およそ 650 バイト）
const TRIANGLE_BYTES: u64 = 1024;
/// 格子のメッシュ（型 5）の頂点の 1 つ（48 バイト）。hayro-interpret は、頂点を行ごとの配列に足すので、頂点が
/// 2 つの行では、行の配列の余りを含めておよそ 130 バイト使う
const VERTEX_BYTES: u64 = 256;
/// パッチのメッシュ（型 6・7）のパッチの 1 つ（352 バイトまで。配列を伸ばす間は、およそ 1,060 バイト）
const PATCH_BYTES: u64 = 2048;
/// 色の表（Separation・1 成分の DeviceN・Indexed の色空間と、転送関数）の項目の数。hayro-interpret は、はじめて
/// 使うときに、0〜255 の値のそれぞれに関数を当てて表を作る
const TABLE: u64 = 256;

/// 内容のストリームの後ろに足す、終わりの印の演算子
const END: &[u8] = b"zumen-end";

/// 走査の結果
#[derive(Clone, Debug, Default)]
pub(crate) struct Survey {
    /// 転送関数（10.4）を持つ拡張グラフィックス状態を使う
    pub(crate) transfer: bool,
    /// hayro-interpret がスタックかメモリを使い切るか、パニックするおそれがあるので、ページを解釈しない。値は、理由の
    /// オブジェクト（入れ子の深すぎる関数か色空間、範囲の広すぎる幅を持つフォント、大きさの大きすぎる Type 0 の
    /// 関数、行の頂点の数が 0 の格子のメッシュ）の場所。直接書いたオブジェクトは、それを含む間接オブジェクトの場所
    /// （hayro-syntax の `obj_id`）。色空間の入れ子は、ページの内容のストリーム
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
    /// 読めるフォント。値は、走査が読んだフォントの表（[`Walker::faces`]）の番号
    Loaded(u32),
    /// 読めるフォントのうち、原点の移動が既定と違いうる縦書きのフォント（間接オブジェクト）と、表の番号
    Shifted((u32, i32), u32),
    /// 読める Type 3 のフォント（9.6.4）。hayro-interpret は、字形ごとにグラフィックス状態を写す
    Type3,
}

/// 走査が追う、グラフィックス状態の一部（`q`・`Q` で保存する、8.4.2）。hayro-interpret の手間を変える
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Graphics {
    font: Selected,
    /// 破線の配列（`d`、8.4.3.6）の長さ。hayro-interpret は、線と文字を描くたびに写す
    dash: u64,
    /// 文字を切り抜きにする（描画モード 4〜7、9.3.6）
    clip: bool,
    /// 切り抜きに貯めた文字の輪郭の、経路の要素の数（`ET` で空にする）
    clipped: u64,
    /// 塗りの色を 1 度 RGB にする手間（[`Conversion::each`]）。装置は、塗るたびに色を RGB にする
    fill: u64,
    /// 線の色を 1 度 RGB にする手間
    stroke: u64,
}

impl Default for Graphics {
    fn default() -> Self {
        Self {
            font: Selected::Nothing,
            dash: 0,
            clip: false,
            clipped: 0,
            fill: 0,
            stroke: 0,
        }
    }
}

/// 色空間で色を RGB にする手間（`Work`）
#[derive(Clone, Copy, Debug, Default)]
struct Conversion {
    /// はじめて色を RGB にするときに作る表（[`TABLE`]）の手間
    table: u64,
    /// 色を 1 つ RGB にするたびの手間（多成分の DeviceN の色変換の関数）
    each: u64,
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
    /// 間接オブジェクトでないフォーム（直接書いたストリーム、[`own_stream`]）。呼ばれ方を覚えない
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

/// ストリームが、それ自身の間接オブジェクトなら、その番号。hayro-syntax 0.9.0 は、辞書の値や配列の項目に直接書いた
/// ストリーム（7.3.8.1 に反する）も読み、その `obj_id` には、直接書いた辞書と同じく、それを含む間接オブジェクトの
/// 番号を返す。そこで、その番号で xref から引いたストリームの辞書が、同じバイト列かを確かめる。入れ物のストリームの
/// 辞書は、直接書いたストリームの辞書を真に含むので、同じバイト列にならない
pub(crate) fn own_stream(xref: &XRef, stream: &Stream<'_>) -> Option<ObjectIdentifier> {
    let id = stream.dict().obj_id()?;
    let own = xref.get::<Stream<'_>>(id)?;
    (own.dict().data() == stream.dict().data()).then_some(id)
}

/// 辞書が、それ自身の間接オブジェクトなら、その番号。見分け方は [`own_stream`] と同じ
pub(crate) fn own_dict(xref: &XRef, dict: &Dict<'_>) -> Option<ObjectIdentifier> {
    let id = dict.obj_id()?;
    let own = xref.get::<Dict<'_>>(id)?;
    (own.data() == dict.data()).then_some(id)
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

/// 関数の辞書（7.10）の、数の配列の項目の数。hayro-interpret は、関数を作るたびに、`Domain`・`Range` と型ごとの
/// 配列を読んで数にするので、項目の数に比例する手間がかかる（短い数の並びでは、バイト数で数える手間より重い）
fn numbers(dict: &Dict<'_>) -> u64 {
    [
        b"Domain".as_slice(),
        b"Range",
        b"Size",
        b"Encode",
        b"Decode",
        b"C0",
        b"C1",
        b"Bounds",
    ]
    .iter()
    .filter_map(|key| dict.get::<Array<'_>>(key))
    .map(|array| array.raw_iter().count() as u64)
    .sum()
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

/// グラフィックス状態を 1 度写す手間（`Work`）とメモリ（バイト数）。hayro-interpret は、`q` の後で状態を変える
/// とき・フォームを呼ぶとき（8.10.1）・Type 3 の字形ごと（9.6.4）に状態を写す。大きさが決まっていないのは、破線の
/// 配列と、切り抜きに貯めた文字の輪郭。`q` は、後で状態を変えるかによらず数える
fn copy(state: Graphics) -> (u64, u64) {
    let work = state
        .dash
        .saturating_add(state.clipped.saturating_mul(CLIP_COPY_WORK));
    let bytes = state
        .dash
        .saturating_mul(DASH_BYTES)
        .saturating_add(state.clipped.saturating_mul(OUTLINE_COPY_BYTES));
    (work, bytes)
}

/// メッシュのシェーディング（型 4〜7、8.7.4.5.5〜8.7.4.5.8）で、hayro-interpret が三角形とパッチを作る手間
/// （[`MESH_WORK`]）とメモリ（バイト数）の上限。`size` は、展開したバイト数。hayro-interpret は、使うたびに、
/// 型 4・5 では頂点をバイトの境から読んで三角形をすべて作り（型 4 は頂点ごとに 1 つ、型 5 は 2 つまで）、型 6・7
/// ではパッチをすべて作る（前のパッチと辺を分けるものは、点を 8 つか 12 読む）。頂点とパッチの数の上限は、色を
/// 読まないとした最小のビット数で求める。hayro-interpret は、`Function` がなければ、色の成分の数を `Decode` の
/// 座標より後ろの項目の数の半分とするので、`Decode` が座標の分だけなら、色を読まない（`read_colors`）
fn mesh_cost(dict: &Dict<'_>, kind: u8, size: u64) -> Cost {
    let bits = |key: &[u8]| u64::from(dict.get::<u8>(key).unwrap_or(0));
    let coordinate = bits(b"BitsPerCoordinate");
    let flag = bits(b"BitsPerFlag");
    let (unit, memory) = match kind {
        4 => ((flag + 2 * coordinate).div_ceil(8) * 8, TRIANGLE_BYTES),
        5 => (
            (2 * coordinate).div_ceil(8) * 8,
            2 * TRIANGLE_BYTES + VERTEX_BYTES,
        ),
        6 => (flag + 16 * coordinate, PATCH_BYTES),
        7 => (flag + 24 * coordinate, PATCH_BYTES),
        _ => return Cost::default(),
    };
    let count = size.saturating_mul(8) / unit.max(1);
    Cost {
        ops: count.saturating_mul(MESH_WORK),
        bytes: count.saturating_mul(memory),
    }
}

/// Type 4 の関数の手続き（`{` `}`、7.10.5.2）の入れ子が、[`PROCEDURE_DEPTH`] 段に収まるか。コメント（`%` から
/// 行の終わりまで、7.2.4）の中の括弧は、hayro-interpret と同じく数えない
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
    /// hayro-interpret と同じ読み方でフォントを読む。読めるかを見て、切り抜きにする文字の輪郭を作る
    fonts: FontCache<'a>,
    /// 読めたフォントの表（文書ごと）。[`Selected::Loaded`] の番号で引く
    faces: Vec<Font<'a>>,
    /// フォントの辞書のバイト列 → 表の番号（文書ごと）。hayro-interpret も、辞書のバイト列でフォントを見分ける
    face_ids: BTreeMap<Vec<u8>, u32>,
    /// hayro-interpret が代わりに使う Helvetica の、表の番号（文書ごと。読めなければ `None`）
    helvetica: Option<Option<u32>>,
    /// フォントの表の番号と文字の符号 → 切り抜きにする字形の輪郭の、経路の要素の数（文書ごと）
    outlines: BTreeMap<(u32, u32), u64>,
    /// ストリームの番号 → xref から引いたストリームの辞書のバイト列（文書ごと、[`own_stream`]）
    streams: BTreeMap<(u32, i32), Option<&'a [u8]>>,
    /// ページの最初の内容のストリーム。ほかに場所のない診断の場所
    place: Ref,
    /// フォームの呼ばれ方 → hayro-interpret がする作業と、走査が返した診断（ページごと）
    calls: BTreeMap<Call, Known>,
    /// 拡張グラフィックス状態・シェーディング・画像 → hayro-interpret が使うたびにする作業（ページごと）。間接
    /// オブジェクトだけを覚える。hayro-syntax の `obj_id` は、直接書いた辞書とストリームには、それを含む間接
    /// オブジェクトの番号を返すので、鍵には、資源の辞書の項目が指す参照か、それ自身の間接オブジェクトのストリームの
    /// 番号（[`own_stream`]）を使う
    costs: BTreeMap<(Role, (u32, i32)), Cost>,
    /// 縦書きのフォント（間接オブジェクト）→ 原点の移動が既定と違いうるか（ページごと）
    vertical: BTreeMap<(u32, i32), bool>,
    /// 原点の移動が既定と違いうる縦書きのフォント → 診断の名前（`BaseFont`）と場所（ページごと）
    origins: BTreeMap<(u32, i32), (String, Ref)>,
    /// CID フォント（間接オブジェクト）→ 幅の項目の数（ページごと）
    widths: BTreeMap<(u32, i32), u64>,
    /// 幅の表のメモリを数えた CID フォントの辞書のバイト列（文書ごと）。走査と hayro-interpret は、フォントを
    /// 辞書のバイト列で見分け、文書の中で 1 度だけ読む
    tables: BTreeSet<Vec<u8>>,
    /// 作る手間を数えた、名前で引く色空間 → 色を 1 つ RGB にするたびの手間（ページごと）
    spaces: BTreeMap<Space, u64>,
    /// ページを解釈しない理由のオブジェクトの場所（[`Survey::refused`]）
    refused: Option<Ref>,
    /// 呼ばれ方を覚えるフォームの中で返した診断。たどっているフォームごとに 1 つ
    recording: Vec<Diagnostics>,
    /// 展開したフォーム。展開できなければ `None`（ページごと）。鍵は、それ自身の間接オブジェクトのストリームの番号
    /// （[`own_stream`]）
    forms: BTreeMap<(u32, i32), Option<Form>>,
    /// ストリーム（ソフトマスクのグループ・シェーディング・関数・Indexed の表）の、展開したバイト数（ページごと）。
    /// 鍵は、フォームと同じ
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
            faces: Vec::new(),
            face_ids: BTreeMap::new(),
            helvetica: None,
            outlines: BTreeMap::new(),
            streams: BTreeMap::new(),
            place: Ref::default(),
            calls: BTreeMap::new(),
            costs: BTreeMap::new(),
            vertical: BTreeMap::new(),
            origins: BTreeMap::new(),
            widths: BTreeMap::new(),
            tables: BTreeSet::new(),
            spaces: BTreeMap::new(),
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

    /// ストリームが、それ自身の間接オブジェクトなら、その番号（[`own_stream`]）。番号ごとに、xref から引いた
    /// ストリームの辞書のバイト列を覚える。直接書いたストリームは、手間と展開した中身を覚えず、使うたびにたどる
    fn own(&mut self, stream: &Stream<'_>) -> Option<(u32, i32)> {
        let id = stream.dict().obj_id()?;
        let xref = self.xref;
        let own = *self
            .streams
            .entry(key(id))
            .or_insert_with(|| xref.get::<Stream<'a>>(id).map(|own| own.dict().data()));
        (own == Some(stream.dict().data())).then(|| key(id))
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
                TypedInstruction::SaveState(_) => {
                    let (work, bytes) = copy(current);
                    spend(&mut cost, work, bytes, cx)?;
                    states.push(current);
                }
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
                // hayro-interpret は、切り抜きに貯めた輪郭を写して切り抜きにし、空にする
                TypedInstruction::EndText(_) => {
                    spend(
                        &mut cost,
                        current.clipped.saturating_mul(CLIP_COPY_WORK),
                        current.clipped.saturating_mul(OUTLINE_COPY_BYTES),
                        cx,
                    )?;
                    if let Some(last) = states.last_mut() {
                        last.clipped = 0;
                    }
                }
                TypedInstruction::SetGraphicsState(name) => {
                    spend(&mut cost, STATE_WORK, 0, cx)?;
                    if let Some(state) = resources.get_ext_g_state(name.0) {
                        let reference = resources.ext_g_states.get_ref(name.0.as_ref()).map(key);
                        let extra = self.state(&state, reference, cx)?;
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
                // 装置は、塗るたびに塗りの色を RGB にする
                TypedInstruction::FillPathNonZero(_)
                | TypedInstruction::FillPathNonZeroCompatibility(_)
                | TypedInstruction::FillPathEvenOdd(_) => {
                    spend(&mut cost, PAINT_WORK.saturating_add(current.fill), 0, cx)?;
                }
                // hayro-interpret は、線を描くたびに破線の配列を写し、装置は、線の色を RGB にする
                TypedInstruction::StrokePath(_) | TypedInstruction::CloseAndStrokePath(_) => {
                    let work = PAINT_WORK
                        .saturating_add(current.dash)
                        .saturating_add(current.stroke);
                    spend(&mut cost, work, 0, cx)?;
                }
                TypedInstruction::FillAndStrokeNonZero(_)
                | TypedInstruction::FillAndStrokeEvenOdd(_)
                | TypedInstruction::CloseFillAndStrokeNonZero(_)
                | TypedInstruction::CloseFillAndStrokeEvenOdd(_) => {
                    // 塗りと線を、別に装置に渡す
                    let work = PAINT_WORK
                        .saturating_mul(2)
                        .saturating_add(current.dash)
                        .saturating_add(current.fill)
                        .saturating_add(current.stroke);
                    spend(&mut cost, work, 0, cx)?;
                }
                // 装置の色空間の色は、関数を当てずに RGB にする
                TypedInstruction::NonStrokeColorDeviceGray(_)
                | TypedInstruction::NonStrokeColorDeviceRgb(_)
                | TypedInstruction::NonStrokeColorCmyk(_) => {
                    if let Some(last) = states.last_mut() {
                        last.fill = 0;
                    }
                }
                TypedInstruction::StrokeColorDeviceGray(_)
                | TypedInstruction::StrokeColorDeviceRgb(_)
                | TypedInstruction::StrokeColorCmyk(_) => {
                    if let Some(last) = states.last_mut() {
                        last.stroke = 0;
                    }
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
                TypedInstruction::Shading(name) => {
                    if let Some(shading) = resources.get_shading(name.0) {
                        let reference = resources.shadings.get_ref(name.0.as_ref()).map(key);
                        let extra = self.shading(&shading, reference, cx)?;
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
                    let (extra, each) = self.named_space(resources, owner, space.0, cx)?;
                    spend(&mut cost, extra.ops, extra.bytes, cx)?;
                    if let Some(last) = states.last_mut() {
                        last.stroke = each;
                    }
                }
                TypedInstruction::ColorSpaceNonStroke(space) => {
                    let (extra, each) = self.named_space(resources, owner, space.0, cx)?;
                    spend(&mut cost, extra.ops, extra.bytes, cx)?;
                    if let Some(last) = states.last_mut() {
                        last.fill = each;
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
        let entry = state.get::<Array<'_>>(b"Font");
        let font = entry
            .as_ref()
            .and_then(|font| font.iter::<Object<'_>>().next())
            .and_then(|font| font.into_dict());
        if let Some(font) = font {
            let reference = entry
                .as_ref()
                .and_then(|entry| entry.raw_iter().next())
                .and_then(|item| item.as_obj_ref())
                .map(key);
            let base = font
                .get::<Name<'_>>(b"BaseFont")
                .map(|base| name_text(base.as_ref()))
                .unwrap_or_default();
            let selected = self.font_dict(&font, reference, base, cost, cx)?;
            if let Some(last) = states.last_mut() {
                last.font = selected;
            }
        }
        Ok(())
    }

    /// 拡張グラフィックス状態（8.4.5）を当てる手間。転送関数を見つける。hayro-interpret は、状態を当てるたびに、
    /// 転送関数（`TR`・`TR2`、10.4）を作り、ソフトマスク（11.6.5.2）のグループを展開し、その色空間と転送関数を作る。
    /// 転送関数は、`TR` と `TR2` の鍵のそれぞれで作り直し、はじめて塗るときに [`TABLE`] 個の値に当てて表を作る。
    /// `reference` は、資源の辞書の項目が指す参照（直接書いた状態なら `None` で、手間を覚えない）
    fn state(
        &mut self,
        state: &Dict<'a>,
        reference: Option<(u32, i32)>,
        cx: &mut Cx<'_>,
    ) -> Result<Cost, Stop> {
        self.transfer |= transfer(state);
        if let Some(cost) = reference.and_then(|id| self.costs.get(&(Role::State, id))) {
            return Ok(*cost);
        }
        let mut cost = Cost::default();
        let function = state
            .get::<Object<'_>>(b"TR2")
            .or_else(|| state.get::<Object<'_>>(b"TR"));
        if let Some(function) = function {
            let (mut made, eval) = self.function(&function, 0, cx)?;
            made.ops = made.ops.saturating_add(TABLE.saturating_mul(eval));
            for key in [b"TR".as_slice(), b"TR2"] {
                if state.contains_key(key) {
                    cost.add(made);
                }
            }
        }
        if let Some(mask) = state.get::<Dict<'_>>(b"SMask") {
            cost.ops = cost.ops.saturating_add(lookup_work(mask.data()));
            if let Some(group) = mask.get::<Stream<'_>>(b"G") {
                cost.ops = cost.ops.saturating_add(lookup_work(group.dict().data()));
                cost.bytes = cost.bytes.saturating_add(self.size(&group, cx)?);
                let space = group
                    .dict()
                    .get::<Dict<'_>>(b"Group")
                    .and_then(|group| group.get::<Object<'_>>(b"CS"));
                if let Some(space) = space {
                    cost.add(self.color_space(&space, 0, cx)?.0);
                }
            }
            if let Some(function) = mask.get::<Object<'_>>(b"TR") {
                cost.add(self.function(&function, 0, cx)?.0);
            }
        }
        if let Some(id) = reference {
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
            let reference = pattern.get_ref(b"Shading").map(key);
            cost.add(self.shading(&shading, reference, cx)?);
        }
        Ok(cost)
    }

    /// シェーディング（8.7.4.3）を作る手間。hayro-interpret は、使うたびに作り直し、ストリームと関数を展開し、
    /// 色空間を作る。メッシュ（型 4〜7）は、三角形とパッチをすべて作るので、その手間とメモリ（[`mesh_cost`]）も
    /// 数える。`reference` は、資源の辞書かパターンの項目が指す参照（直接書いた辞書なら `None` で、手間を覚えない）。
    /// ストリームは、それ自身の間接オブジェクトのときだけ覚える
    fn shading(
        &mut self,
        shading: &Object<'_>,
        reference: Option<(u32, i32)>,
        cx: &mut Cx<'_>,
    ) -> Result<Cost, Stop> {
        let Some((dict, stream)) = dict_or_stream(shading) else {
            return Ok(Cost::default());
        };
        let id = match stream {
            Some(stream) => self.own(stream),
            None => reference,
        };
        if let Some(cost) = id.and_then(|id| self.costs.get(&(Role::Shading, id))) {
            return Ok(*cost);
        }
        let mut cost = Cost::default();
        if let Some(stream) = stream {
            let size = self.size(stream, cx)?;
            cost.bytes = cost.bytes.saturating_add(size);
            let kind = dict.get::<u8>(b"ShadingType");
            if let Some(kind) = kind {
                cost.add(mesh_cost(dict, kind, size));
            }
            // hayro-interpret は、格子のメッシュ（型 5、8.7.4.5.6）の行の頂点の数が 0 だと、頂点を読まずに行を足し
            // 続け、メモリを使い切る（`read_lattice_triangles`）。大きさは hayro-interpret と同じく u32 で読む
            if kind == Some(5) && dict.get::<u32>(b"VerticesPerRow") == Some(0) {
                let place = self.record(Some(stream.obj_id()));
                self.refuse(place);
            }
        }
        if let Some(function) = dict.get::<Object<'_>>(b"Function") {
            cost.add(self.function(&function, 0, cx)?.0);
        }
        if let Some(space) = dict.get::<Object<'_>>(b"ColorSpace") {
            cost.add(self.color_space(&space, 0, cx)?.0);
        }
        if let Some(id) = id {
            self.costs.insert((Role::Shading, id), cost);
        }
        Ok(cost)
    }

    /// 画像の XObject を描く手間。hayro-interpret は、描くたびに画像の色空間を作る。名前の色空間は、呼ぶ所の資源で
    /// 引く。画素は、描くときまで展開しない。手間は、それ自身の間接オブジェクトの画像だけ覚える
    fn image(
        &mut self,
        stream: &Stream<'_>,
        resources: &Resources<'a>,
        owner: Owner,
        cx: &mut Cx<'_>,
    ) -> Result<Cost, Stop> {
        let dict = stream.dict();
        let space = dict
            .get::<Object<'_>>(b"CS")
            .or_else(|| dict.get::<Object<'_>>(b"ColorSpace"));
        if let Some(Object::Name(name)) = &space {
            return Ok(self.named_space(resources, owner, name, cx)?.0);
        }
        let id = self.own(stream);
        if let Some(cost) = id.and_then(|id| self.costs.get(&(Role::Image, id))) {
            return Ok(*cost);
        }
        let cost = match space {
            Some(space) => self.color_space(&space, 0, cx)?.0,
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
            Object::Name(name) => Ok(self.named_space(resources, owner, name, cx)?.0),
            _ => Ok(self.color_space(space, 0, cx)?.0),
        }
    }

    /// 名前で引く色空間（`cs`・`CS`・画像の名前の色空間、8.6.3）を使う手間と、その色空間で色を 1 つ RGB にするたびの
    /// 手間（[`Conversion::each`]）。hayro-interpret は、使うたびに資源を引いて読み直し、そのバイト列から表の鍵を
    /// 作る。色空間そのものと色の表（[`Conversion::table`]）は、文書の中で 1 度だけ作るので、作る手間はページで
    /// 1 度だけ数える（使う手間には入れない）。資源にない名前（装置の色空間など）は、何もしない
    fn named_space(
        &mut self,
        resources: &Resources<'a>,
        owner: Owner,
        name: &Name<'_>,
        cx: &mut Cx<'_>,
    ) -> Result<(Cost, u64), Stop> {
        let Some(space) = resources.get_color_space(name) else {
            return Ok((Cost::default(), 0));
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
        if let Some(each) = entry.as_ref().and_then(|entry| self.spaces.get(entry)) {
            return Ok((cost, *each));
        }
        let (made, conversion) = self.color_space(&space, 0, cx)?;
        cx.check(Kind::Work, made.ops.saturating_add(conversion.table))?;
        cx.check(Kind::DecodedBytes, made.bytes)?;
        if let Some(entry) = entry {
            self.spaces.insert(entry, conversion.each);
        }
        Ok((cost, conversion.each))
    }

    /// 関数（7.10）を作る手間と、1 度当てる手間（`Work`）。hayro-interpret は、関数を使う演算子ごとに関数を作り直し、
    /// 辞書を読み直し、ストリームの関数（Type 0・Type 4）を展開する。つなぐ関数（Type 3）の `Functions` は、辞書でも
    /// ストリームでもたどる（hayro-interpret は `dict_or_stream` で読む）。関数の配列の中もたどる。[`NESTED_DEPTH`]
    /// より深い入れ子は、ページを解釈しないことにする。当てる手間は、hayro-interpret 0.9.0 の当て方に合わせて
    /// 多めに見積もる: Type 0 は 2 の（大きさが 1 より大きい入力の数）乗の端の標本を足し（`Type0::eval`）、Type 2 は
    /// 出力ごとに冪を求め、Type 3 は区間を探して 1 つの関数を当て、Type 4 は手続きを読む（繰り返しはない、7.10.5）
    fn function(
        &mut self,
        function: &Object<'_>,
        depth: u32,
        cx: &mut Cx<'_>,
    ) -> Result<(Cost, u64), Stop> {
        cx.check(Kind::Work, 1)?;
        let mut cost = Cost {
            ops: 1u64.saturating_add(lookup_work(object_data(function))),
            bytes: 0,
        };
        if depth > NESTED_DEPTH {
            let id = match function {
                Object::Dict(dict) => dict.obj_id(),
                Object::Stream(stream) => Some(stream.obj_id()),
                _ => None,
            };
            let place = self.record(id);
            self.refuse(place);
            return Ok((cost, 1));
        }
        let (dict, stream) = match function {
            // 出力が 1 つずつの関数の配列。当てる手間は、それぞれの和
            Object::Array(functions) => {
                let mut eval = 0u64;
                for function in functions.iter::<Object<'_>>() {
                    let (made, each) = self.function(&function, depth + 1, cx)?;
                    cost.add(made);
                    eval = eval.saturating_add(each);
                }
                return Ok((cost, eval));
            }
            other => match dict_or_stream(other) {
                Some(pair) => pair,
                None => return Ok((cost, 1)),
            },
        };
        cost.ops = cost.ops.saturating_add(numbers(dict));
        let mut eval = 1u64;
        if let Some(stream) = stream {
            let size = self.size(stream, cx)?;
            cost.bytes = size;
            match dict.get::<u8>(b"FunctionType") {
                Some(0) => {
                    let (made, each) = self.sampled(dict, stream, size, cx)?;
                    cost.add(made);
                    eval = each;
                }
                // hayro-interpret は、作るたびに手続きを読む。引いたオブジェクトと同じく、4 バイトを 1 と数える
                Some(4) => {
                    cost.ops = cost.ops.saturating_add(size.div_ceil(LOOKUP_BYTES));
                    eval = size.max(1);
                }
                _ => {}
            }
        } else if dict.get::<u8>(b"FunctionType") == Some(2) {
            let outputs = dict
                .get::<Array<'_>>(b"C0")
                .map_or(1, |values| values.raw_iter().count() as u64);
            eval = outputs.saturating_add(1);
        }
        if let Some(functions) = dict.get::<Array<'_>>(b"Functions") {
            let count = functions.raw_iter().count() as u64;
            let mut inner = 0u64;
            for function in functions.iter::<Object<'_>>() {
                let (made, each) = self.function(&function, depth + 1, cx)?;
                cost.add(made);
                inner = inner.max(each);
            }
            eval = eval.saturating_add(count).saturating_add(inner);
        }
        Ok((cost, eval))
    }

    /// Type 0 の関数（7.10.2）の標本を読む手間（[`SAMPLES_PER_WORK`]）とメモリ（バイト数）と、1 度当てる手間。
    /// `size` は、展開したバイト数。hayro-interpret は、関数を作るたびに、標本（出力の数 × 大きさの積。展開した
    /// バイト列に収まる分まで）を読んで f32 にして持つ。大きさのどれかが [`MAX_SAMPLE_SIZE`] を超える関数は、ページを
    /// 解釈しないことにする
    fn sampled(
        &mut self,
        dict: &Dict<'_>,
        stream: &Stream<'_>,
        size: u64,
        cx: &mut Cx<'_>,
    ) -> Result<(Cost, u64), Stop> {
        // hayro-interpret と同じく、大きさは u32 で読む
        let sizes: Vec<u32> = dict
            .get::<Array<'_>>(b"Size")
            .map(|sizes| sizes.iter::<u32>().collect())
            .unwrap_or_default();
        cx.check(Kind::Work, sizes.len() as u64)?;
        if sizes.iter().any(|&size| size > MAX_SAMPLE_SIZE) {
            let place = self.record(Some(stream.obj_id()));
            self.refuse(place);
        }
        let outputs = dict
            .get::<Array<'_>>(b"Range")
            .map_or(0, |range| range.raw_iter().count() / 2) as u64;
        let wanted = sizes.iter().fold(outputs, |count, &size| {
            count.saturating_mul(u64::from(size))
        });
        let bits = u64::from(dict.get::<u8>(b"BitsPerSample").unwrap_or(0)).max(1);
        let samples = wanted.min(size.saturating_mul(8) / bits);
        let inputs = sizes.iter().filter(|&&size| size > 1).count() as u32;
        let corners = 1u64.checked_shl(inputs).unwrap_or(u64::MAX);
        let made = Cost {
            ops: samples.div_ceil(SAMPLES_PER_WORK),
            bytes: samples.saturating_mul(SAMPLE_BYTES),
        };
        Ok((made, corners.saturating_mul(outputs.max(1))))
    }

    /// 直接書いた色空間（8.6）を作る手間。hayro-interpret は、配列の色空間を使うたびに作り直す。分版（Separation・
    /// DeviceN、8.6.6.4・8.6.6.5）の代わりの色空間と色変換の関数、Indexed の基の色空間と表（8.6.6.3）、パターンの
    /// 基の色空間（8.6.6.2）、ICCBased の代わりの色空間（`Alternate`、8.6.5.5）をたどる。[`NESTED_DEPTH`] より深い
    /// 入れ子は、ページを解釈しないことにする。2 つ目は、この色空間で色を RGB にする手間。hayro-interpret は、
    /// Separation・1 成分の DeviceN・Indexed では、はじめて色を RGB にするときに [`TABLE`] 色の表を作り、多成分の
    /// DeviceN では、色を RGB にするたびに色変換の関数を当てる。ICCBased は、プロファイルを読めなければ代わりの
    /// 色空間で RGB にするので、代わりの色空間の手間とする
    fn color_space(
        &mut self,
        space: &Object<'_>,
        depth: u32,
        cx: &mut Cx<'_>,
    ) -> Result<(Cost, Conversion), Stop> {
        cx.check(Kind::Work, 1)?;
        // hayro-interpret は、色空間を作るたびに配列を読み直す（入れ子の配列の中の参照の先も）
        let mut cost = Cost {
            ops: 1u64.saturating_add(lookup_work(object_data(space))),
            bytes: 0,
        };
        let mut conversion = Conversion::default();
        let Object::Array(items) = space else {
            return Ok((cost, conversion));
        };
        if depth > NESTED_DEPTH {
            let place = self.place.clone();
            self.refuse(place);
            return Ok((cost, conversion));
        }
        let mut items = items.iter::<Object<'_>>();
        let family = items.next().and_then(|family| family.into_name());
        match family.as_deref() {
            Some(b"Separation" | b"DeviceN") => {
                let separation = family.as_deref() == Some(b"Separation".as_slice());
                // DeviceN の成分の数（色材の名前の数）。Separation は 1 つ
                let components = items
                    .next()
                    .and_then(|names| names.into_array())
                    .map_or(1, |names| names.raw_iter().count());
                let mut alternate = Conversion::default();
                if let Some(space) = items.next() {
                    let (made, inner) = self.color_space(&space, depth + 1, cx)?;
                    cost.add(made);
                    alternate = inner;
                }
                let mut tint = 1;
                if let Some(function) = items.next() {
                    let (made, eval) = self.function(&function, depth + 1, cx)?;
                    cost.add(made);
                    tint = eval;
                }
                let each = tint.saturating_add(alternate.each);
                conversion.table = alternate.table;
                if separation || components == 1 {
                    conversion.table = conversion.table.saturating_add(TABLE.saturating_mul(each));
                } else {
                    conversion.each = each;
                }
            }
            Some(b"Indexed" | b"I") => {
                if let Some(base) = items.next() {
                    let (made, inner) = self.color_space(&base, depth + 1, cx)?;
                    cost.add(made);
                    conversion.table = TABLE.saturating_mul(inner.each).saturating_add(inner.table);
                }
                let _high = items.next();
                if let Some(Object::Stream(table)) = items.next() {
                    cost.bytes = cost.bytes.saturating_add(self.size(&table, cx)?);
                }
            }
            Some(b"Pattern") => {
                if let Some(base) = items.next() {
                    let (made, inner) = self.color_space(&base, depth + 1, cx)?;
                    cost.add(made);
                    conversion = inner;
                }
            }
            Some(b"ICCBased") => {
                let profile = items.next().and_then(|profile| profile.into_stream());
                if let Some(profile) = &profile {
                    cost.ops = cost.ops.saturating_add(lookup_work(profile.dict().data()));
                }
                let alternate =
                    profile.and_then(|profile| profile.dict().get::<Object<'_>>(b"Alternate"));
                if let Some(alternate) = alternate {
                    let (made, inner) = self.color_space(&alternate, depth + 1, cx)?;
                    cost.add(made);
                    conversion = inner;
                }
            }
            _ => {}
        }
        Ok((cost, conversion))
    }

    /// ストリームを展開したバイト数。それ自身の間接オブジェクトのストリームは、ページの中で 1 度だけ展開し、直接
    /// 書いたストリームは、使うたびに展開する。hayro-interpret と同じく、画像のフィルター（DCT など）は展開しない。
    /// 手続きの入れ子が [`PROCEDURE_DEPTH`] 段より深い Type 4 の関数（7.10.5）は、ページを解釈しないことにする
    fn size(&mut self, stream: &Stream<'_>, cx: &mut Cx<'_>) -> Result<u64, Stop> {
        let id = self.own(stream);
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
    /// 描くときに返す。装置は、文字列ごとに塗りと線の色を RGB にする。Type 3 のフォントでは、hayro-interpret が字形
    /// ごとにグラフィックス状態を写し（[`copy`]）、切り抜きにする文字では、見える字の輪郭を作って `ET` まで貯める
    /// （[`Walker::outlines_of`]）
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
        // hayro-interpret は、見えるかによらず Helvetica を選ぶ
        if state.font == Selected::Nothing && fallback {
            state.font = Selected::Assumed;
        }
        spend(cost, glyph_work(strings, *state), 0, cx)?;
        let colors = (strings.len() as u64).saturating_mul(state.fill.saturating_add(state.stroke));
        spend(cost, colors, 0, cx)?;
        let bytes = strings.iter().map(|bytes| bytes.len() as u64).sum::<u64>();
        if state.font == Selected::Type3 {
            let (work, copied) = copy(*state);
            spend(
                cost,
                work.saturating_mul(bytes),
                copied.saturating_mul(bytes),
                cx,
            )?;
        }
        if state.clip && self.is_visible() {
            let elements = self.outlines_of(state.font, strings, cost, cx)?;
            state.clipped = state.clipped.saturating_add(elements);
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
            Selected::Shifted(id, _) => {
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
            Selected::Loaded(_) | Selected::Shifted(..) | Selected::Type3 => 0,
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

    /// 切り抜きにする文字（描画モード 4〜7、9.3.6）の輪郭を作る手間（[`OUTLINE_WORK`]）とメモリ（[`OUTLINE_BYTES`]）を
    /// 字ごとに数え、輪郭の経路の要素の数を返す。hayro-interpret は、選んだフォントで文字列から符号を読み
    /// （`read_code`）、字形を引き（`map_code`・`glyph`）、見える字ごとにフォントのプログラムから輪郭を作る
    /// （`clip_glyph`）。輪郭の大きさは、フォントのプログラムで決まり、上限は hayro-font が使う skrifa と read-fonts の
    /// 制限（TrueType は 65,536 点、CFF と Type 1 は 100 万の演算）だけなので、走査も同じ API で輪郭を作って数える。
    /// 要素の数は、フォントと符号ごとに 1 度だけ作って覚える。Helvetica で読む文字列は、ASCII のバイトだけのものしか
    /// 描かない。Type 3 の字形は、輪郭を足さない
    fn outlines_of(
        &mut self,
        font: Selected,
        strings: &[&[u8]],
        cost: &mut Cost,
        cx: &mut Cx<'_>,
    ) -> Result<u64, Stop> {
        let (face, fallback) = match font {
            Selected::Loaded(face) | Selected::Shifted(_, face) => (Some(face), false),
            Selected::Assumed | Selected::Fallback => (self.helvetica(), true),
            Selected::Nothing | Selected::Type3 => (None, false),
        };
        let Some(font) = face.and_then(|face| self.faces.get(face as usize).cloned()) else {
            return Ok(0);
        };
        let face = face.unwrap_or_default();
        let mut total = 0u64;
        for bytes in strings {
            if fallback && !bytes.is_ascii() {
                continue;
            }
            let mut at = 0;
            while at < bytes.len() {
                let (code, advance) = font.read_code(bytes, at);
                at = at.saturating_add(advance.max(1));
                let elements = match self.outlines.get(&(face, code)) {
                    Some(elements) => *elements,
                    None => {
                        let elements = match font.glyph(font.map_code(code), code) {
                            FontGlyph::Outline(glyph) => glyph.outline().elements().len() as u64,
                            FontGlyph::Type3(_) => 0,
                        };
                        self.outlines.insert((face, code), elements);
                        elements
                    }
                };
                spend(
                    cost,
                    elements.saturating_mul(OUTLINE_WORK),
                    elements.saturating_mul(OUTLINE_BYTES),
                    cx,
                )?;
                total = total.saturating_add(elements);
            }
        }
        Ok(total)
    }

    /// 読めたフォントの、表（[`Walker::faces`]）の番号。辞書のバイト列が同じフォントは、同じ番号にする
    fn face(&mut self, dict: &Dict<'_>, font: &Font<'a>) -> u32 {
        if let Some(face) = self.face_ids.get(dict.data()) {
            return *face;
        }
        let face = u32::try_from(self.faces.len()).unwrap_or(u32::MAX);
        self.faces.push(font.clone());
        self.face_ids.insert(dict.data().to_vec(), face);
        face
    }

    /// hayro-interpret が代わりに使う Helvetica（`fallback_font`）の、表の番号。読めなければ `None`
    fn helvetica(&mut self) -> Option<u32> {
        if let Some(face) = self.helvetica {
            return face;
        }
        let settings = self.settings;
        let font = self
            .fonts
            .standard_font(StandardFont::Helvetica, &settings.font_resolver);
        let face = font.map(|font| {
            let face = u32::try_from(self.faces.len()).unwrap_or(u32::MAX);
            self.faces.push(font);
            face
        });
        self.helvetica = Some(face);
        face
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
        let reference = resources.fonts.get_ref(name.as_ref()).map(key);
        self.font_dict(&dict, reference, base, cost, cx)
    }

    /// フォントの辞書（`Tf` で選ぶものと、拡張グラフィックス状態の `Font`）を選ぶ。hayro-interpret は、選ぶたびに
    /// 辞書のバイト列から表の鍵を作り、文書の中で 1 度だけフォントを読む。`reference` は、フォントの辞書を指す参照
    /// （直接書いた辞書なら `None`）。`base` は、診断の名前
    fn font_dict(
        &mut self,
        dict: &Dict<'a>,
        reference: Option<(u32, i32)>,
        base: String,
        cost: &mut Cost,
        cx: &mut Cx<'_>,
    ) -> Result<Selected, Stop> {
        spend(cost, lookup_work(dict.data()), 0, cx)?;
        let id = dict.obj_id();
        // 幅の項目の多すぎるフォントは、読む前に、ページを解釈しないことにする
        if !self.widths_fit(dict, reference, cx)? {
            let place = self.record(id);
            self.refuse(place);
            return Ok(Selected::Fallback);
        }
        let settings = self.settings;
        let font = self
            .fonts
            .font(dict, &settings.font_resolver, &settings.cmap_resolver);
        let value = reference.map_or_else(
            || base.clone(),
            |(number, generation)| format!("{number} {generation}"),
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
        if dict.get::<Name<'_>>(b"Subtype").as_deref() == Some(b"Type3".as_slice()) {
            return Ok(Selected::Type3);
        }
        let face = self.face(dict, &font);
        if font.is_horizontal() || !self.shifted(dict, reference, cx)? {
            return Ok(Selected::Loaded(face));
        }
        let place = self.record(id);
        match reference {
            Some(reference) => {
                self.origins.insert(reference, (base, place));
                Ok(Selected::Shifted(reference, face))
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
                Ok(Selected::Loaded(face))
            }
        }
    }

    /// CID フォント（9.7）の幅（`W`・`W2`、9.7.4.3）の項目が [`MAX_WIDTHS`] に収まるか。hayro-font が範囲の CID を
    /// 1 つずつ表に入れる手間を数える（間接オブジェクトのフォントは、ページの中で 1 度だけ）。収まれば、表のメモリを
    /// 文書の中で 1 度だけ数える
    fn widths_fit(
        &mut self,
        font: &Dict<'_>,
        reference: Option<(u32, i32)>,
        cx: &mut Cx<'_>,
    ) -> Result<bool, Stop> {
        if font.get::<Name<'_>>(b"Subtype").as_deref() != Some(b"Type0".as_slice()) {
            return Ok(true);
        }
        if let Some(count) = reference.and_then(|id| self.widths.get(&id)) {
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
        if let Some(id) = reference {
            self.widths.insert(id, count);
        }
        let fits = count <= MAX_WIDTHS;
        if fits && self.tables.insert(font.data().to_vec()) {
            cx.check(Kind::DecodedBytes, count.saturating_mul(WIDTH_BYTES))?;
        }
        Ok(fits)
    }

    /// 縦書きのフォントの原点の移動（9.7.4.3）が、図面モデルに移すときに仮定する既定（字形の幅 1,000、位置の
    /// ベクトル `(500, 880)`）と違いうるか。子孫のフォントの `DW`・`W`・`DW2`・`W2` を見る（間接オブジェクトの
    /// フォントは、ページの中で 1 度だけ）
    fn shifted(
        &mut self,
        font: &Dict<'_>,
        reference: Option<(u32, i32)>,
        cx: &mut Cx<'_>,
    ) -> Result<bool, Stop> {
        if let Some(shifted) = reference.and_then(|id| self.vertical.get(&id)) {
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
        if let Some(id) = reference {
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
        spend(cost, MARKED_WORK, 0, cx)?;
        // hayro-interpret は、名前の性質の辞書を、印の種類によらず読み直す
        if let Some(name) = properties.clone().into_name()
            && let Some(dict) = resources.properties.get::<Dict<'_>>(name.as_ref())
        {
            spend(cost, lookup_work(dict.data()), 0, cx)?;
        }
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
                // hayro-interpret は、内容の印のたびに表示グループか表示の規則の辞書を引き直す。表示の規則の辞書なら、
                // hayro-interpret・走査・装置が、それぞれ `OCGs` の配列を読む
                let groups = dict
                    .get::<Array<'_>>(b"OCGs")
                    .map_or(0, |groups| lookup_work(groups.data()));
                spend(
                    cost,
                    lookup_work(dict.data()).saturating_add(groups.saturating_mul(3)),
                    0,
                    cx,
                )?;
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
        let place = self.record(dict.obj_id());
        let id = self.own(stream);
        let visible = self.is_visible();
        let mut cost = Cost::default();
        spend(&mut cost, lookup_work(dict.data()), 0, cx)?;
        // hayro-interpret は、描くたびに表示グループ（`OC`、8.11.3.3）の辞書と、表示の規則の辞書の `OCGs` を読み直す。
        // 走査も、見えるかを決めるために読む
        if let Some(group) = dict.get::<Dict<'_>>(b"OC") {
            let groups = group
                .get::<Array<'_>>(b"OCGs")
                .map_or(0, |groups| lookup_work(groups.data()));
            spend(
                &mut cost,
                lookup_work(group.data()).saturating_add(groups.saturating_mul(2)),
                0,
                cx,
            )?;
        }
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
                let extra = self.image(stream, resources, owner, cx)?;
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
    /// 解釈する。`BBox` のないフォームと、展開できないフォームと、入れ子が [`MAX_DEPTH`] 段より深いフォームは描かない。
    /// `id` は、それ自身の間接オブジェクトのフォームの番号（[`own_stream`]。直接書いたフォームなら `None` で、展開した
    /// 中身と呼ばれ方を覚えない）
    #[allow(clippy::too_many_arguments)]
    fn form(
        &mut self,
        stream: &Stream<'a>,
        id: Option<(u32, i32)>,
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
        // hayro-interpret は、フォームを呼ぶたびにグラフィックス状態を積み、中で変えるときに写す
        let (work, copied) = copy(state);
        spend(&mut cost, work, copied, cx)?;
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
            (Some(_), Some(id)) => Owner::Form(id),
            (Some(_), None) => Owner::Direct,
        };
        let call = match (id, owner) {
            (Some(id), Owner::Page | Owner::Form(_)) => Some(Call {
                form: id,
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

    /// 展開したフォーム。それ自身の間接オブジェクトのフォーム（番号 `id`）は、ページの中で 1 度だけ展開する
    fn form_data(
        &mut self,
        stream: &Stream<'a>,
        id: Option<(u32, i32)>,
        cx: &mut Cx<'_>,
    ) -> Result<Option<Form>, Stop> {
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
