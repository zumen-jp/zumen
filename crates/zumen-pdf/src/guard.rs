//! hayro-syntax 0.9.0 がパニックするか、環境の大きさの整数で読んで結果が環境で変わる値を、読む前に確かめる
//! （決定 0007・0256）。
//!
//! hayro-syntax は、Flate と LZW の予測子（7.4.4.4）の行の長さを、`Columns` と `Colors` と `BitsPerComponent` の積で
//! 求め、確かめずに掛けて割る。`Predictor` は u8 で読み、1 のほかのどの値でも予測子を当てる。積が 0 か、u8 であふれて
//! 0 になると、0 で割ってパニックし、あふれる値は 32 ビットと 64 ビットで違う。また、展開するたびに 1 行の分を確保する。
//! オブジェクトのストリーム（7.5.7）の `N`・`First` と中の位置、ICCBased の `N`、CCITT の `Columns` は、環境の大きさの
//! 整数（`usize`）で読むので、2 の 32 乗からの値は 32 ビットの環境で読めず、足すとあふれる。LZW（7.4.4）は符号を読む
//! 位置をビットの数で、ASCII85（7.4.3）は入力の長さの 4 倍を、同じく環境の大きさの整数で求める。
//!
//! hayro-syntax は、表が指すファイルのどの位置の定義も読み（文字列とストリームの中も）、どこに書いたストリームも
//! 読み（オブジェクトのストリームの中も）、値の参照を、オブジェクトのストリームの中の定義にも解く。表を作り直すときは、
//! 空白で区切ったどの字句の辞書も、型がオブジェクトのストリームなら展開する。辞書は、ファイルのバイト列か、オブジェクトの
//! ストリームの中身に書いてある（内容のストリームの中の辞書は、インラインの画像のもので、zumen は画素を展開しない）。
//! そこで、辞書の中かを見ずに、ファイルとオブジェクトのストリームの中身のすべての名前を見て、こうした値を持つ文書は
//! 読まない。値の参照は、すべての定義の数を候補にする。オブジェクトのストリームは、hayro-syntax と同じく展開して、
//! 中の位置を使う前に確かめる。暗号化した文書は、読み込む前にオブジェクトのストリームの中身を復号できないので、
//! オブジェクトのストリームになりうるものを持てば読まない。

use std::collections::{BTreeMap, BTreeSet};

use hayro_syntax::object::{Dict, MaybeRef, Name, Number, ObjRef, Object, Stream};
use hayro_syntax::reader::{Readable, Reader, ReaderContext, ReaderExt};
use zumen_base::{Diagnostic, Kind, Ref, Stop};
use zumen_sdk::Cx;

use crate::walk::lookup_work;

/// 環境の大きさの整数で読む値の上限（含まない）。32 ビットの環境でも読め、2 つを足してもあふれない
const LIMIT: i64 = 1 << 31;

/// 予測子の 1 行のバイト数の上限。hayro-syntax は、展開するたびに 1 行の分を確保する。幅 65,536 画素の 16 ビットの
/// RGBA の行（512 KiB）は収まる
const ROW: i64 = 1 << 20;

/// LZW と ASCII85 で展開する入力のバイト数の上限（含まない）。hayro-syntax は、LZW の符号を読む位置をビットの数で、
/// ASCII85 の出力の大きさを入力の長さの 4 倍で求めるので、32 ビットの環境で 2 の 29 乗・2 の 30 乗バイトからあふれる
const CODED: u64 = 1 << 28;

/// 参照を探す値の入れ子の深さの上限。深い値は、参照を含むものとして扱う
const VALUE_DEPTH: usize = 16;

/// 値を確かめるキー。hayro-syntax の読み方（予測子は 7.4.4.4 の表 8、オブジェクトのストリームは 7.5.7 の表 16）
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Key {
    /// `Predictor`。u8 で読む
    Predictor,
    /// `Colors`。u8 で読み、読めなければ 1
    Colors,
    /// `BitsPerComponent`。u8 で読み、読めなければ 8
    Bits,
    /// `Columns`。`usize` で読み、読めなければ 1
    Columns,
    /// `N`。`usize` で読む（オブジェクトのストリーム、ICCBased）
    Count,
    /// `First`。`usize` で読む
    First,
}

impl Key {
    fn of(name: &[u8]) -> Option<Self> {
        Some(match name {
            b"Predictor" => Self::Predictor,
            b"Colors" => Self::Colors,
            b"BitsPerComponent" => Self::Bits,
            b"Columns" => Self::Columns,
            b"N" => Self::Count,
            b"First" => Self::First,
            _ => return None,
        })
    }
}

/// 1 つのキーの値。場所は、ファイルの順で最初のもの
#[derive(Clone, Copy, Debug, Default)]
struct Values {
    /// 2 の 31 乗以上の値
    large: Option<usize>,
    /// hayro-syntax の読みで 0 になる値
    zero: Option<usize>,
    /// hayro-syntax の読みでいちばん大きい値
    most: i64,
    /// 予測子を当てる値（`Predictor`）
    active: Option<usize>,
    /// 参照の値
    referenced: Option<usize>,
}

/// 間接オブジェクトの定義の数（参照の先の候補）
#[derive(Clone, Copy, Debug, Default)]
struct Numbers {
    /// 2 の 31 乗以上の数がある
    large: bool,
    /// 0 がある
    zero: bool,
    /// 0〜255 の数でいちばん大きいもの
    byte: i64,
    /// 2 の 31 乗より小さい数でいちばん大きいもの
    small: i64,
    /// u8 で読めて 1 でない数がある
    active: bool,
}

/// 見た値。場所は、ファイルのバイトの位置
#[derive(Debug, Default)]
struct Seen {
    values: BTreeMap<Key, Values>,
    numbers: Numbers,
    /// フィルターの値の LZW・ASCII85 の名前のうち、展開する入力が [`CODED`] に届きうるもの
    coded: Option<usize>,
    /// LZW・ASCII85 の名前のうち、フィルターの値として読めない所のもの
    stray: Option<usize>,
    /// フィルターの値に参照がある。hayro-syntax は参照の先の名前と配列も読むので、フィルターの値として読めない所の
    /// 名前も、フィルターとして当てうる
    indirect: bool,
}

impl Seen {
    /// 直接書いた値。hayro-syntax と同じく、数は切り捨てた整数で読む
    fn value(&mut self, key: Key, value: i64, place: usize) {
        let values = self.values.entry(key).or_default();
        match key {
            Key::Predictor => {
                if (0..=255).contains(&value) && value != 1 {
                    values.active.get_or_insert(place);
                }
            }
            Key::Colors | Key::Bits => {
                if (0..=255).contains(&value) {
                    if value == 0 {
                        values.zero.get_or_insert(place);
                    }
                    values.most = values.most.max(value);
                }
            }
            // 負の値は読めず、既定の値になる
            Key::Columns => {
                if value >= LIMIT {
                    values.large.get_or_insert(place);
                } else if value == 0 {
                    values.zero.get_or_insert(place);
                } else {
                    values.most = values.most.max(value);
                }
            }
            Key::Count | Key::First => {
                if value >= LIMIT {
                    values.large.get_or_insert(place);
                }
            }
        }
    }

    fn reference(&mut self, key: Key, place: usize) {
        self.values
            .entry(key)
            .or_default()
            .referenced
            .get_or_insert(place);
    }

    fn number(&mut self, value: i64) {
        let numbers = &mut self.numbers;
        numbers.large |= value >= LIMIT;
        numbers.zero |= value == 0;
        if (0..=255).contains(&value) {
            numbers.byte = numbers.byte.max(value);
            numbers.active |= value != 1;
        }
        if (0..LIMIT).contains(&value) {
            numbers.small = numbers.small.max(value);
        }
    }

    /// 読まない理由の場所。参照の値は、すべての定義の数を候補にする。予測子は、どこかに予測子を当てる値があれば、
    /// どの `Colors`・`BitsPerComponent`・`Columns`（と既定の値）の組でも行の長さが 0 にならず、`Colors` と
    /// `BitsPerComponent` の積が u8 に収まり、1 行が [`ROW`] に収まるかを見る
    fn verdict(&self) -> Option<usize> {
        if let Some(place) = self.coded.or_else(|| self.stray.filter(|_| self.indirect)) {
            return Some(place);
        }
        let numbers = self.numbers;
        let get = |key: Key| self.values.get(&key).copied().unwrap_or_default();
        for key in [Key::Count, Key::First, Key::Columns] {
            let values = get(key);
            if let Some(place) = values.large.or(values.referenced.filter(|_| numbers.large)) {
                return Some(place);
            }
        }
        let predictor = get(Key::Predictor);
        let at = predictor
            .active
            .or(predictor.referenced.filter(|_| numbers.active))?;
        for key in [Key::Colors, Key::Bits, Key::Columns] {
            let values = get(key);
            if let Some(place) = values.zero.or(values.referenced.filter(|_| numbers.zero)) {
                return Some(place);
            }
        }
        let most = |key: Key, default: i64| {
            let values = get(key);
            let referenced = match (values.referenced, key) {
                (None, _) => 0,
                (Some(_), Key::Columns) => numbers.small,
                (Some(_), _) => numbers.byte,
            };
            values.most.max(default).max(referenced)
        };
        let pixel = most(Key::Colors, 1) * most(Key::Bits, 8);
        // どれも 0 より大きい
        let row = most(Key::Columns, 1)
            .saturating_mul(pixel)
            .saturating_add(7)
            / 8;
        (pixel > 255 || row > ROW).then_some(at)
    }
}

/// ファイルのバイト列の名前のうち、表の作り直しと暗号化に関わるもの
#[derive(Debug, Default)]
struct Marks {
    /// `Type` の値が `ObjStm` の名前の、キーの位置と、値の名前の字の位置（`#xx` で書いた名前は、位置を持たない）
    types: Vec<(usize, Option<usize>)>,
    /// `Type` の値が参照の位置
    referenced: Option<usize>,
    /// 暗号化の辞書の名前（`Encrypt`、7.6.1）の位置
    encrypted: Option<usize>,
}

/// 読み込む前に確かめ、読まないなら、その理由のファイルのバイトの位置を返す。ファイルのすべての名前と定義を見て、
/// オブジェクトのストリームになりうるストリームを展開して、中の位置と中身を見る
pub(crate) fn before(src: &[u8], cx: &mut Cx<'_>) -> Result<Option<usize>, Stop> {
    let mut seen = Seen::default();
    let marks = names(src, |at| at, &mut seen, cx)?;
    let containers = objects(src, &mut seen, cx)?;
    // 暗号化した文書は、オブジェクトのストリームの中身を、読み込む前に hayro-syntax と同じには復号できない。
    // hayro-syntax は、表が指すストリームと、表を作り直すときに型が `ObjStm` の辞書（参照を解いたものも）を展開する
    if marks.encrypted.is_some()
        && let Some(at) = containers
            .first()
            .map(|container| container.start)
            .or(marks.types.first().map(|&(at, _)| at))
            .or(marks.referenced)
    {
        return Ok(Some(at));
    }
    // 表を作り直すときに展開する辞書は、型の値が、ここで展開するストリームの上の段の型の値と同じ字か
    let claimed = claims(src, &containers, cx)?;
    if let Some(&(at, _)) = marks
        .types
        .iter()
        .find(|(_, value)| !value.is_some_and(|value| claimed.contains(&value)))
    {
        return Ok(Some(at));
    }
    // オブジェクトのストリームを展開する前に、展開のパラメーターを確かめる
    if let Some(place) = seen.verdict() {
        return Ok(Some(place));
    }
    for container in &containers {
        // 参照の値は、読む前に hayro-syntax と同じには解けない
        if referenced(container.stream.dict(), cx)? {
            return Ok(Some(container.start));
        }
        if let Some(place) = container_fits(&container.stream, container.start, &mut seen, cx)? {
            return Ok(Some(place));
        }
    }
    Ok(seen.verdict())
}

/// 表を作り直すときに、ここで展開するストリームと同じものとして展開される、型の値の名前の字の位置。hayro-syntax は、
/// 表を作り直すとき（`fallback_xref_map`）、空白で区切った字句を順に見て、`<<` から読める辞書の型（上の段の
/// `Type`。同じキーが何度もあれば最後のもの）が `ObjStm` の名前で、後ろに `stream` があれば、オブジェクトのストリーム
/// として展開する。字句の区切りはコメントと文字列を見ないので、どの位置の辞書も展開しうる。
///
/// 型の値の字が同じ 2 つの辞書は、そこから後ろを同じく読むので、同じ所で終わり、同じストリームを持つ。ただし、
/// 型の前の値（`Filter`・`Length` など）は、片方が相手の始まりをコメントか文字列の中に読み、型の前でそこから出れば
/// 違う。コメントは行末で、文字列は `)` で終わる（`\` は名前の字だが、文字列の中では次の字をエスケープするので、同じ `)` で
/// 片方だけが文字列を出られる）。16 進の文字列は `<<` を含められない。始まりが同じでなければ、片方は相手の辞書を
/// 入れ子として読み、型の値を上の段に読まない（hayro-syntax は、読み飛ばす辞書も、読む辞書と同じく文字列と入れ子を
/// 見る）。そこで、確かめたストリームの辞書の型の値は、辞書の始まりの行の前に `%` がなく、辞書の始まりから型の値までに
/// `%` も `)` もなければ、同じものとする
fn claims(
    src: &[u8],
    containers: &[Container<'_>],
    cx: &mut Cx<'_>,
) -> Result<BTreeSet<usize>, Stop> {
    let mut claimed = BTreeSet::new();
    // 行の始まりから、いまの位置の前までに `%` がある
    let mut comment = false;
    let mut at = 0;
    for container in containers {
        while at < container.start {
            match src.get(at) {
                Some(b'\n' | b'\r') => comment = false,
                Some(b'%') => comment = true,
                _ => {}
            }
            at += 1;
        }
        let Some(value) = container.typed else {
            continue;
        };
        if comment {
            continue;
        }
        let head = src.get(container.start..value).unwrap_or_default();
        cx.check(Kind::Work, lookup_work(head))?;
        if !head.iter().any(|&byte| matches!(byte, b'%' | b')')) {
            claimed.insert(value);
        }
    }
    Ok(claimed)
}

/// 名前の字の、バイト列の中の位置。`#xx` を解いた名前は、バイト列の外に持つので、位置を持たない
fn position(data: &[u8], name: &Name<'_>) -> Option<usize> {
    let bytes: &[u8] = name;
    let at = bytes.as_ptr().addr().checked_sub(data.as_ptr().addr())?;
    (at.checked_add(bytes.len())? <= data.len()).then_some(at)
}

/// オブジェクトのストリームを展開して、中の位置と中身を見る
fn container_fits(
    stream: &Stream<'_>,
    place: usize,
    seen: &mut Seen,
    cx: &mut Cx<'_>,
) -> Result<Option<usize>, Stop> {
    let dict = stream.dict();
    let (Some(count), Some(first)) = (dict.get::<i64>(b"N"), dict.get::<i64>(b"First")) else {
        return Ok(None);
    };
    // 負の値は、どの環境でも読めず、中のオブジェクトを使わない
    if count < 0 || first < 0 {
        return Ok(None);
    }
    if count >= LIMIT || first >= LIMIT {
        return Ok(Some(place));
    }
    let Ok(data) = stream.decoded_data() else {
        return Ok(None);
    };
    cx.check(Kind::DecodedBytes, data.len() as u64)?;
    members(&data, count, first, place, seen, cx)
}

/// オブジェクトのストリームの中の位置と中身。hayro-syntax の `ObjectStream::read_offsets` と同じく、番号と位置の組を
/// `N` 個まで読み、前の位置より後ろで、展開したバイト列の中を指す位置だけを使う。中の位置が 2 の 31 乗以上なら、
/// 32 ビットの環境で読めないか、`First` と足すとあふれるので、場所を返す。使う位置の数は、参照の先の候補にする。
/// 位置が数の途中を指せば、数の終わりまでを位置ごとに読むので、読む字の数を `Work` に数える
fn members(
    data: &[u8],
    count: i64,
    first: i64,
    place: usize,
    seen: &mut Seen,
    cx: &mut Cx<'_>,
) -> Result<Option<usize>, Stop> {
    let mut header = Reader::new(data);
    let mut skip = Skip::default();
    let mut last = None;
    for _ in 0..count {
        cx.check(Kind::Work, 1)?;
        header.skip_white_spaces_and_comments();
        if header.read_without_context::<u32>().is_none() {
            break;
        }
        header.skip_white_spaces_and_comments();
        let Some(relative) = header.read_without_context::<i64>() else {
            break;
        };
        // 負の位置は、どの環境でも読めず、ストリームの全体を使わない
        if relative < 0 {
            break;
        }
        if relative >= LIMIT {
            return Ok(Some(place));
        }
        let offset = usize::try_from(first + relative).unwrap_or(usize::MAX);
        if offset >= data.len() || last.is_some_and(|last| offset <= last) {
            continue;
        }
        let at = skip.over(data, offset, cx)?;
        last = Some(at);
        if let Some(number) = number(data, at, cx)? {
            seen.number(number);
        }
    }
    names(data, |_| place, seen, cx)?;
    Ok(None)
}

/// バイト列のすべての名前（7.3.5）のうち、値を確かめるキーと、型・フィルターのキーの後ろの値を見る。hayro-syntax の
/// 辞書と同じく、キーの後ろの空白とコメントを飛ばし、参照（`n g R`）か数として読む。辞書の中かを見ないので、文字列と
/// ストリームの中の名前も見る（多めに断るだけ）。名前の中に `/` はないので、名前の字は 1 度だけ読む。値を読む手間は
/// `Work` に数える。
///
/// 2 つのキーが同じ値の位置に着くのは、後のキーが、前のキーの飛ばした空白とコメントの中（コメントの中）にあり、
/// 飛ばした範囲の終わりに飛ぶときだけ。範囲の中で止まる値の前には空白しかないので、ほかのキーは着かない。そこで、
/// 範囲の終わりの値を読み方ごとに 1 つずつ覚え、値は、位置と読み方ごとに 1 度だけ読む（違う位置の値は、違う字句から
/// 読む）
fn names(
    data: &[u8],
    place: impl Fn(usize) -> usize,
    seen: &mut Seen,
    cx: &mut Cx<'_>,
) -> Result<Marks, Stop> {
    let mut marks = Marks::default();
    let mut skip = Skip::default();
    // フィルターの値の LZW・ASCII85 の名前の位置 → 入力の長さの、バイト列の長さに対する倍率の上限
    let mut bounded = BTreeMap::new();
    // 範囲の終わりの値として、型とフィルターの値に読んだ位置、キーの値に読んだ位置と読んだもの、フィルターの配列が
    // 読んだ項目の並び
    let mut typed = None;
    let mut filtered = None;
    let mut read = None;
    let mut items = Items::default();
    let length = data.len() as u64;
    let mut at = 0;
    while let Some(found) = data
        .get(at..)
        .and_then(|rest| rest.iter().position(|&byte| byte == b'/'))
    {
        let start = at + found;
        let mut reader = Reader::new(data);
        reader.jump(start);
        let name = reader.read_without_context::<Name<'_>>();
        let end = reader.offset();
        at = end.max(start + 1);
        let Some(name) = name else {
            continue;
        };
        let name: &[u8] = &name;
        if coded(name) {
            match bounded.remove(&start) {
                Some(factor) if length.saturating_mul(factor) < CODED => {}
                Some(_) => {
                    seen.coded.get_or_insert(place(start));
                }
                None => {
                    seen.stray.get_or_insert(place(start));
                }
            }
            continue;
        }
        let key = Key::of(name);
        match name {
            b"Encrypt" => {
                marks.encrypted.get_or_insert(start);
                continue;
            }
            b"Type" | b"Filter" | b"F" => {}
            _ if key.is_some() => {}
            _ => continue,
        }
        let value = skip.over(data, end, cx)?;
        let shared = value == skip.to;
        match (name, key) {
            (b"Type", _) => {
                if shared && typed.replace(value) == Some(value) {
                    continue;
                }
                if reference(data, value, cx)? {
                    marks.referenced.get_or_insert(start);
                } else if let Some((name, next)) = read_name(data, value) {
                    cx.check(
                        Kind::Work,
                        lookup_work(data.get(value..next).unwrap_or_default()),
                    )?;
                    if &*name == b"ObjStm" {
                        marks.types.push((start, position(data, &name)));
                    }
                }
            }
            (b"Filter" | b"F", _) => {
                if !(shared && filtered.replace(value) == Some(value))
                    && filters(data, value, &mut skip, &mut items, &mut bounded, cx)?
                {
                    seen.indirect = true;
                }
            }
            (_, Some(key)) => {
                let found = match read {
                    Some((at, found)) if shared && at == value => found,
                    _ => {
                        let found = if reference(data, value, cx)? {
                            Found::Reference
                        } else {
                            number(data, value, cx)?.map_or(Found::Other, Found::Number)
                        };
                        if shared {
                            read = Some((value, found));
                        }
                        found
                    }
                };
                match found {
                    Found::Reference => seen.reference(key, place(start)),
                    Found::Number(number) => seen.value(key, number, place(start)),
                    Found::Other => {}
                }
            }
            _ => {}
        }
    }
    Ok(marks)
}

/// キーの値として読んだもの
#[derive(Debug, Clone, Copy)]
enum Found {
    Reference,
    Number(i64),
    Other,
}

/// LZW か ASCII85 のフィルターの名前（7.4、表 6）。hayro-syntax の `Filter::from_name` と同じく、短い名前も読む
fn coded(name: &[u8]) -> bool {
    matches!(name, b"LZWDecode" | b"LZW" | b"ASCII85Decode" | b"A85")
}

/// フィルターの名前 → hayro-syntax が当てるか、出力の長さの、入力の長さに対する倍率の上限。hayro-syntax は、知らない
/// 名前を飛ばし、最初に当てるフィルターが `Crypt` なら当てない。ASCIIHex は短くし、ASCII85 は 1 字（`z`）を 4 バイトに
/// する。ほかのフィルターは、どこまでも長くしうる
fn expansion(name: &[u8], first: bool) -> (bool, u64) {
    match name {
        b"ASCIIHexDecode" | b"AHx" => (true, 1),
        b"ASCII85Decode" | b"A85" => (true, 4),
        b"Crypt" if first => (true, 1),
        b"LZWDecode" | b"LZW" | b"FlateDecode" | b"Fl" | b"RunLengthDecode" | b"RL"
        | b"CCITTFaxDecode" | b"CCF" | b"JBIG2Decode" | b"DCTDecode" | b"DCT" | b"JPXDecode"
        | b"Crypt" => (true, u64::MAX),
        _ => (false, 1),
    }
}

/// フィルターの値（7.3.8.2、表 5 の `Filter`。hayro-syntax は `F` も読む）の LZW と ASCII85 の名前に、入力の長さの
/// 倍率の上限を記録し、値か、配列の名前でない最初の項目が参照かを返す。hayro-syntax と同じく、名前か、名前の配列を
/// 読む。配列の名前でない項目の後ろは、hayro-syntax が当てないので、記録しない（項目が参照なら、後ろの名前は、読めない
/// ものとして断る）。
///
/// 項目の間の空白とコメントは、キーの後ろを飛ばす覚え（`skip`）で飛ばすが、キーの側がまだ読む範囲は覚え直さない
/// （[`Skip::keeping`]）。コメントの中の配列は、どれも、覚えた範囲の終わりから同じ項目の並びを読みうる。そこで、範囲の
/// 終わりから読んだ並びは、初めのフィルターか（`first`）ごとに、読み始めの倍率のいちばん大きいものと、止まった項目が
/// 参照かを覚える。後ろの項目の倍率は、読み始めの倍率が大きいほど大きく、[`bound`] は大きいほうを使うので、覚えた
/// 倍率より大きくなければ読まない
fn filters(
    data: &[u8],
    at: usize,
    skip: &mut Skip,
    items: &mut Items,
    bounded: &mut BTreeMap<usize, u64>,
    cx: &mut Cx<'_>,
) -> Result<bool, Stop> {
    if let Some((name, next)) = read_name(data, at) {
        cx.check(
            Kind::Work,
            lookup_work(data.get(at..next).unwrap_or_default()),
        )?;
        if coded(&name) {
            bound(bounded, at, 1);
        }
        return Ok(false);
    }
    if data.get(at) != Some(&b'[') {
        return reference(data, at, cx);
    }
    let mut item = at + 1;
    let mut factor = 1u64;
    let mut first = true;
    // 範囲の終わりから読み始めた倍率と、初めのフィルターか
    let mut start = None;
    let found = loop {
        item = skip.keeping(data, item, at, cx)?;
        if item == skip.to && start.is_none() {
            if items.at != item {
                *items = Items {
                    at: item,
                    ..Items::default()
                };
            }
            if let Some((most, found)) = *items.slot(first)
                && factor <= most
            {
                return Ok(found);
            }
            start = Some((factor, first));
        }
        let Some((name, next)) = read_name(data, item) else {
            break reference(data, item, cx)?;
        };
        cx.check(
            Kind::Work,
            lookup_work(data.get(item..next).unwrap_or_default()),
        )?;
        if coded(&name) {
            bound(bounded, item, factor);
        }
        let (known, ratio) = expansion(&name, first);
        factor = factor.saturating_mul(ratio);
        first &= !known;
        item = next;
    };
    if let Some((factor, first)) = start {
        *items.slot(first) = Some((factor, found));
    }
    Ok(found)
}

/// フィルターの配列が、空白とコメントを飛ばした範囲の終わりから読んだ項目の並び（[`filters`]）。初めのフィルターか
/// （`first`）ごとに、読み始めの倍率のいちばん大きいものと、止まった項目が参照かを持つ
#[derive(Debug, Default)]
struct Items {
    /// 範囲の終わり
    at: usize,
    first: Option<(u64, bool)>,
    later: Option<(u64, bool)>,
}

impl Items {
    fn slot(&mut self, first: bool) -> &mut Option<(u64, bool)> {
        if first {
            &mut self.first
        } else {
            &mut self.later
        }
    }
}

/// 位置の名前に、入力の長さの倍率の上限を記録する。同じ位置を、ほかのキーの値としても読めば（配列の中の `/F` の
/// 後ろなど。hayro-syntax は、配列の中の `F` を知らない名前として飛ばす）、大きいほうを使う
fn bound(bounded: &mut BTreeMap<usize, u64>, at: usize, factor: u64) {
    let kept = bounded.entry(at).or_insert(factor);
    *kept = (*kept).max(factor);
}

/// 位置の名前と、その後ろの位置
fn read_name(data: &[u8], at: usize) -> Option<(Name<'_>, usize)> {
    let mut reader = Reader::new(data);
    reader.jump(at);
    let name = reader.read_without_context::<Name<'_>>()?;
    Some((name, reader.offset()))
}

/// 位置が参照（7.3.10）か。hayro-syntax の辞書と同じく、数の間の空白だけを飛ばす。読んだバイト数を `Work` に数える
/// （読めなければ hayro-syntax は位置を戻すので、位置の字句の長さを数える）
fn reference(data: &[u8], at: usize, cx: &mut Cx<'_>) -> Result<bool, Stop> {
    let mut reader = Reader::new(data);
    reader.jump(at);
    let found = <ObjRef as Readable>::read(&mut reader, &ReaderContext::dummy()).is_some();
    let rest = data.get(at..).unwrap_or_default();
    let token = rest.iter().take_while(|&&byte| regular(byte)).count();
    let read = reader.offset().saturating_sub(at).max(token);
    cx.check(
        Kind::Work,
        lookup_work(rest.get(..read).unwrap_or_default()),
    )?;
    Ok(found)
}

/// 位置の数を、hayro-syntax と同じく切り捨てた整数で読む。数の字句（空白と区切りの文字の前まで）の長さを `Work` に
/// 数える
fn number(data: &[u8], at: usize, cx: &mut Cx<'_>) -> Result<Option<i64>, Stop> {
    let rest = data.get(at..).unwrap_or_default();
    let token = rest.iter().take_while(|&&byte| regular(byte)).count();
    cx.check(
        Kind::Work,
        lookup_work(rest.get(..token).unwrap_or_default()),
    )?;
    let mut reader = Reader::new(data);
    reader.jump(at);
    Ok(reader
        .read_without_context::<Number>()
        .map(|number| number.as_i64()))
}

/// 空白でも区切りの文字でもない字（7.2.3）
fn regular(byte: u8) -> bool {
    !white(byte) && !b"()<>[]{}/%".contains(&byte)
}

/// 空白の字（7.2.3、表 1）
fn white(byte: u8) -> bool {
    matches!(byte, 0x00 | 0x09 | 0x0a | 0x0c | 0x0d | 0x20)
}

/// 空白とコメント（7.2.3・7.2.4）を、hayro-syntax の `skip_white_spaces_and_comments` と同じく飛ばす。コメントは行末で
/// 終わるので、前に飛ばした範囲の中の行末か `%` に来たら、前と同じ所まで飛ぶ。範囲の中のほかの位置から飛ばしても、
/// 範囲の終わりより先には進まない。そこで、位置が増える順に飛ばせば、手間はバイト列の長さに比例する。読んだバイト数は
/// `Work` に数える
#[derive(Debug, Default)]
struct Skip {
    from: usize,
    to: usize,
}

impl Skip {
    fn over(&mut self, data: &[u8], start: usize, cx: &mut Cx<'_>) -> Result<usize, Stop> {
        self.walk(data, start, None, cx)
    }

    /// 配列の項目の間を飛ばす。覚えた範囲が空でなく、キーの値（`cursor`）より後ろに届けば、範囲の外から飛ばしても
    /// 覚え直さない。キーの側は、まだその範囲の中の位置を読む（項目は、キーの値より後ろにある）
    fn keeping(
        &mut self,
        data: &[u8],
        start: usize,
        cursor: usize,
        cx: &mut Cx<'_>,
    ) -> Result<usize, Stop> {
        self.walk(data, start, Some(cursor), cx)
    }

    fn walk(
        &mut self,
        data: &[u8],
        start: usize,
        cursor: Option<usize>,
        cx: &mut Cx<'_>,
    ) -> Result<usize, Stop> {
        let range = self.from..self.to;
        let mut at = start;
        let mut end = None;
        while let Some(&byte) = data.get(at) {
            if range.contains(&at) && matches!(byte, b'\n' | b'\r' | b'%') {
                end = Some(self.to);
                break;
            }
            if white(byte) {
                at += 1;
            } else if byte == b'%' {
                at += data
                    .get(at..)
                    .unwrap_or_default()
                    .iter()
                    .position(|&byte| matches!(byte, b'\n' | b'\r'))
                    .unwrap_or(data.len() - at);
            } else {
                break;
            }
        }
        cx.check(
            Kind::Work,
            lookup_work(data.get(start..at).unwrap_or_default()),
        )?;
        let end = end.unwrap_or(at);
        let ahead = cursor.is_some_and(|cursor| !range.is_empty() && cursor < range.end);
        if !range.contains(&start) && !ahead {
            *self = Self {
                from: start,
                to: end,
            };
        }
        Ok(end)
    }
}

/// オブジェクトのストリームになりうるストリーム（`N` と `First` を持つ）
struct Container<'a> {
    /// 辞書の始まり
    start: usize,
    /// 上の段の `Type` が `ObjStm` の名前なら、その字の位置
    typed: Option<usize>,
    stream: Stream<'a>,
}

/// ファイルのすべての `obj` の後ろ（7.3.10）を読み、オブジェクトのストリームになりうるストリームを始まりの順に返す。
/// hayro-syntax は、表が指す位置の定義を読むので、レコードの始まりにない定義（文字列とストリームの中）も読みうる。
/// 数は、参照の先の候補にする。コメントの中の `obj` は、空白とコメントを飛ばす覚えで同じ位置に飛ぶので、位置ごとに
/// 1 度だけ読む。辞書は、手間を数えずに 1 度ずつ読むので、ほかの辞書と重ならなければ、手間は `InputBytes` に比例する。
/// 重なって読み直す分と、読めなかった辞書とストリームの後ろのバイト列は、`Work` に数える
fn objects<'a>(
    src: &'a [u8],
    seen: &mut Seen,
    cx: &mut Cx<'_>,
) -> Result<Vec<Container<'a>>, Stop> {
    let context = ReaderContext::dummy();
    let mut containers: Vec<Container<'a>> = Vec::new();
    // 読んだ位置。コメントの中の `obj` は、後ろの定義と同じ位置を指しうる
    let mut starts = BTreeSet::new();
    let mut covered = 0;
    let mut skip = Skip::default();
    for keyword in 0..src.len().saturating_sub(2) {
        if src.get(keyword..keyword + 3) != Some(b"obj".as_slice()) {
            continue;
        }
        // `endobj` などの語の一部は、定義にならない
        if keyword
            .checked_sub(1)
            .and_then(|at| src.get(at))
            .is_some_and(u8::is_ascii_alphabetic)
        {
            continue;
        }
        let start = skip.over(src, keyword + 3, cx)?;
        if !starts.insert(start) {
            continue;
        }
        let mut reader = Reader::new(src);
        reader.jump(start);
        if reader.peek_tag(b"<<").is_none() {
            if let Some(number) = number(src, start, cx)? {
                seen.number(number);
            }
            continue;
        }
        let Some(dict) = <Dict<'_> as Readable>::read(&mut reader, &context) else {
            cx.check(
                Kind::Work,
                lookup_work(src.get(start..).unwrap_or_default()),
            )?;
            continue;
        };
        let end = reader.offset();
        let prior = covered;
        if start < prior {
            cx.check(
                Kind::Work,
                lookup_work(src.get(start..end).unwrap_or_default()),
            )?;
        }
        covered = covered.max(end);
        if !(dict.contains_key(b"N") && dict.contains_key(b"First")) {
            continue;
        }
        // ストリームでない辞書は、hayro-syntax も展開しない
        let after = skip.over(src, end, cx)?;
        if src
            .get(after..)
            .is_none_or(|rest| !rest.starts_with(b"stream"))
        {
            continue;
        }
        let mut reader = Reader::new(src);
        reader.jump(start);
        let Some(stream) = reader.read_with_context::<Stream<'_>>(&context) else {
            // `Length` が合わなければ、hayro-syntax は `endstream` を終わりまで探す
            cx.check(
                Kind::Work,
                lookup_work(src.get(start..).unwrap_or_default()),
            )?;
            continue;
        };
        let reached = reader.offset();
        if start < prior {
            cx.check(
                Kind::Work,
                lookup_work(src.get(end..reached).unwrap_or_default()),
            )?;
        }
        covered = covered.max(reached);
        let typed = dict
            .get::<Name<'_>>(b"Type")
            .filter(|name| &**name == b"ObjStm")
            .and_then(|name| position(src, &name));
        containers.push(Container {
            start,
            typed,
            stream,
        });
    }
    // コメントの中の `obj` から読んだ辞書は、後ろの `obj` の辞書より前に始まりうる
    containers.sort_by_key(|container| container.start);
    Ok(containers)
}

/// オブジェクトのストリームの辞書の、長さ・数・展開の値（`Length`・`N`・`First`・`Filter`・`F`・`DecodeParms`・
/// `DP`）が参照を含むか。辞書と配列の中も、[`VALUE_DEPTH`] 段まで見る
fn referenced(dict: &Dict<'_>, cx: &mut Cx<'_>) -> Result<bool, Stop> {
    let mut pending: Vec<(MaybeRef<Object<'_>>, usize)> = Vec::new();
    for key in [
        b"Length".as_slice(),
        b"N",
        b"First",
        b"Filter",
        b"F",
        b"DecodeParms",
        b"DP",
    ] {
        pending.extend(dict.get_raw::<Object<'_>>(key).map(|value| (value, 0)));
    }
    while let Some((value, depth)) = pending.pop() {
        cx.check(Kind::Work, 1)?;
        let value = match value {
            MaybeRef::Ref(_) => return Ok(true),
            MaybeRef::NotRef(value) => value,
        };
        let inner: Vec<_> = match &value {
            Object::Array(items) => items.raw_iter().collect(),
            Object::Dict(dict) => dict.entries().map(|(_, value)| value).collect(),
            _ => continue,
        };
        if depth >= VALUE_DEPTH {
            return Ok(true);
        }
        pending.extend(inner.into_iter().map(|value| (value, depth + 1)));
    }
    Ok(false)
}

/// 読まない文書の診断
pub(crate) fn refused(place: Ref) -> Diagnostic {
    Diagnostic::new(zumen_base::codes::PDF_DOCUMENT_UNSUPPORTED).at(place)
}
