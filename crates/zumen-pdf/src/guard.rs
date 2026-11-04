//! hayro-syntax 0.9.0 がパニックするか、環境の大きさの整数で読んで結果が環境で変わる値を、読む前に確かめる
//! （決定 0007・0256）。
//!
//! hayro-syntax は、Flate と LZW の予測子（7.4.4.4）の行の長さを、`Columns` と `Colors` と `BitsPerComponent` の積で
//! 求め、確かめずに掛けて割る。`Predictor` は u8 で読み、1 のほかのどの値でも予測子を当てる。積が 0 か、u8 であふれて
//! 0 になると、0 で割ってパニックし、あふれる値は 32 ビットと 64 ビットで違う。また、展開するたびに 1 行の分を確保する。
//! オブジェクトのストリーム（7.5.7）の `N`・`First` と中の位置、ICCBased の `N`、CCITT の `Columns` は、環境の大きさの
//! 整数（`usize`）で読むので、2 の 32 乗からの値は 32 ビットの環境で読めず、足すとあふれる。
//!
//! hayro-syntax は、表が指すファイルのどの位置の定義も読み（文字列とストリームの中も）、どこに書いたストリームも
//! 読み（オブジェクトのストリームの中も）、値の参照を、オブジェクトのストリームの中の定義にも解く。辞書は、ファイルの
//! バイト列か、オブジェクトのストリームの中身に書いてある（内容のストリームの中の辞書は、インラインの画像のもので、
//! zumen は画素を展開しない）。そこで、辞書の中かを見ずに、ファイルとオブジェクトのストリームの中身のすべての名前を
//! 見て、こうした値を持つ文書は読まない。値の参照は、すべての定義の数を候補にする。オブジェクトのストリームの中の
//! 位置は、hayro-syntax が使う前に確かめる（[`before`]）。暗号化した文書では、オブジェクトのストリームの中身を
//! 読み込みの後でしか展開できないので、表を作り直した後に確かめる（[`after`]）。

use std::collections::{BTreeMap, BTreeSet};

use hayro_syntax::Pdf as Document;
use hayro_syntax::object::{
    Dict, MaybeRef, Name, Number, ObjRef, Object, ObjectIdentifier, Stream,
};
use hayro_syntax::reader::{Readable, Reader, ReaderContext, ReaderExt};
use zumen_base::{Diagnostic, Kind, Ref, Stop};
use zumen_sdk::Cx;

use crate::walk::lookup_work;

/// 環境の大きさの整数で読む値の上限（含まない）。32 ビットの環境でも読め、2 つを足してもあふれない
const LIMIT: i64 = 1 << 31;

/// 予測子の 1 行のバイト数の上限。hayro-syntax は、展開するたびに 1 行の分を確保する。幅 65,536 画素の 16 ビットの
/// RGBA の行（512 KiB）は収まる
const ROW: i64 = 1 << 20;

/// 間接オブジェクトの番号を探す、`obj` の前のバイト数（注釈を含む）
const IDENTIFIER_WINDOW: usize = 256;

/// 参照を探す値の入れ子の深さの上限。深い値は、参照を含むものとして扱う
const VALUE_DEPTH: usize = 16;

/// 読まない理由の場所
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Place {
    /// ファイルのバイトの位置
    Byte(usize),
    /// 読み込んだ後に引いたオブジェクトのストリーム
    Object((u32, i32)),
}

/// 読む前に確かめた結果
pub(crate) enum Verdict {
    /// 読んでよい。読み込みの後に確かめるもの
    Fits(Later),
    /// 読まない
    Refused(Place),
}

/// 読み込みの後に確かめるもの
pub(crate) struct Later {
    seen: Seen,
    /// 暗号化した文書で、オブジェクトのストリームになりうるストリームの番号
    containers: BTreeSet<(u32, i32)>,
}

/// 値を確かめる鍵。hayro-syntax の読み方（予測子は 7.4.4.4 の表 8、オブジェクトのストリームは 7.5.7 の表 16）
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

/// 1 つの鍵の値。場所は、ファイルの順で最初のもの
#[derive(Clone, Copy, Debug, Default)]
struct Values {
    /// 2 の 31 乗以上の値
    large: Option<Place>,
    /// hayro-syntax の読みで 0 になる値
    zero: Option<Place>,
    /// hayro-syntax の読みでいちばん大きい値
    most: i64,
    /// 予測子を当てる値（`Predictor`）
    active: Option<Place>,
    /// 参照の値
    referenced: Option<Place>,
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

/// 見た値
#[derive(Debug, Default)]
struct Seen {
    values: BTreeMap<Key, Values>,
    numbers: Numbers,
    /// 暗号化の辞書の名前（`Encrypt`、7.6.1）がある
    encrypted: bool,
}

impl Seen {
    /// 直接書いた値。hayro-syntax と同じく、数は切り捨てた整数で読む
    fn value(&mut self, key: Key, value: i64, place: Place) {
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

    fn reference(&mut self, key: Key, place: Place) {
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
    fn verdict(&self) -> Option<Place> {
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

/// 読み込む前に確かめる。ファイルのすべての名前と定義を見て、暗号化していなければ、オブジェクトのストリームになり
/// うるストリームを展開して、中の位置と中身を見る
pub(crate) fn before(src: &[u8], cx: &mut Cx<'_>) -> Result<Verdict, Stop> {
    let mut seen = Seen::default();
    let types = names(src, Place::Byte, &mut seen);
    let (dicts, containers) = objects(src, &mut seen, cx)?;
    // オブジェクトのストリームの型を、間接オブジェクトの辞書の外に書いたもの。hayro-syntax は、表を作り直すときに、
    // 文字列とストリームの中の辞書も、オブジェクトのストリームとして展開する
    for at in types {
        let index = dicts.partition_point(|&(start, _)| start < at);
        let inside = index
            .checked_sub(1)
            .and_then(|index| dicts.get(index))
            .is_some_and(|&(_, end)| at < end);
        if !inside {
            return Ok(Verdict::Refused(Place::Byte(at)));
        }
    }
    // オブジェクトのストリームを展開する前に、展開のパラメーターを確かめる
    if let Some(place) = seen.verdict() {
        return Ok(Verdict::Refused(place));
    }
    let mut later = BTreeSet::new();
    for container in &containers {
        let place = Place::Byte(container.start);
        // 参照の値は、読む前に hayro-syntax と同じには解けない
        if referenced(container.stream.dict(), cx)? {
            return Ok(Verdict::Refused(place));
        }
        if seen.encrypted {
            let ids = identifiers(src, container.keyword, cx)?;
            if ids.is_empty() {
                return Ok(Verdict::Refused(place));
            }
            later.extend(ids);
            continue;
        }
        if let Some(place) = container_fits(&container.stream, place, &mut seen, cx)? {
            return Ok(Verdict::Refused(place));
        }
    }
    if let Some(place) = seen.verdict() {
        return Ok(Verdict::Refused(place));
    }
    Ok(Verdict::Fits(Later {
        seen,
        containers: later,
    }))
}

/// 暗号化した文書で、hayro-syntax がオブジェクトのストリームとして使うストリームを確かめる。表を作り直した後
/// （[`crate::walk::settle`]）に呼ぶ。中身は、hayro-syntax が復号して展開したものを見る
pub(crate) fn after(
    document: &Document,
    later: Later,
    cx: &mut Cx<'_>,
) -> Result<Option<Place>, Stop> {
    let Later {
        mut seen,
        containers,
    } = later;
    if containers.is_empty() {
        return Ok(None);
    }
    let xref = document.xref();
    for (number, generation) in containers {
        cx.check(Kind::Work, 1)?;
        let Some(stream) = xref.get::<Stream<'_>>(ObjectIdentifier::new(number, generation)) else {
            continue;
        };
        let place = Place::Object((number, generation));
        if let Some(place) = container_fits(&stream, place, &mut seen, cx)? {
            return Ok(Some(place));
        }
    }
    Ok(seen.verdict())
}

/// オブジェクトのストリームを展開して、中の位置と中身を見る
fn container_fits(
    stream: &Stream<'_>,
    place: Place,
    seen: &mut Seen,
    cx: &mut Cx<'_>,
) -> Result<Option<Place>, Stop> {
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
/// 32 ビットの環境で読めないか、`First` と足すとあふれるので、場所を返す。使う位置の数は、参照の先の候補にする
fn members(
    data: &[u8],
    count: i64,
    first: i64,
    place: Place,
    seen: &mut Seen,
    cx: &mut Cx<'_>,
) -> Result<Option<Place>, Stop> {
    let mut header = Reader::new(data);
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
        let mut member = Reader::new(data);
        member.jump(offset);
        member.skip_white_spaces_and_comments();
        last = Some(member.offset());
        if let Some(number) = member.read_without_context::<Number>() {
            seen.number(number.as_i64());
        }
    }
    names(data, |_| place, seen);
    Ok(None)
}

/// バイト列のすべての名前（7.3.5）のうち、値を確かめる鍵の後ろの値を見る。hayro-syntax の辞書と同じく、名前の後ろの
/// 空白と注釈を飛ばし、参照（`n g R`）か数として読む。辞書の中かを見ないので、文字列とストリームの中の名前も見る
/// （多めに断るだけ）。名前の中に `/` はないので、名前の字は 1 度だけ読み、手間はバイト列の長さに比例する。
/// オブジェクトのストリームの型（`/Type /ObjStm`）を書いた位置を返す
fn names(data: &[u8], place: impl Fn(usize) -> Place, seen: &mut Seen) -> Vec<usize> {
    let mut types = Vec::new();
    let mut at = 0;
    while let Some(found) = data
        .get(at..)
        .and_then(|rest| rest.iter().position(|&byte| byte == b'/'))
    {
        let start = at + found;
        let mut reader = Reader::new(data);
        reader.jump(start);
        let name = reader.read_without_context::<Name<'_>>();
        at = reader.offset().max(start + 1);
        let Some(name) = name else {
            continue;
        };
        let name: &[u8] = &name;
        if name == b"Encrypt" {
            seen.encrypted = true;
            continue;
        }
        reader.skip_white_spaces_and_comments();
        if name == b"Type" {
            if reader
                .read_without_context::<Name<'_>>()
                .is_some_and(|value| &*value == b"ObjStm")
            {
                types.push(start);
            }
            continue;
        }
        let Some(key) = Key::of(name) else {
            continue;
        };
        if reader.clone().read_without_context::<ObjRef>().is_some() {
            seen.reference(key, place(start));
        } else if let Some(number) = reader.read_without_context::<Number>() {
            seen.value(key, number.as_i64(), place(start));
        }
    }
    types
}

/// オブジェクトのストリームになりうるストリーム（`N` と `First` を持つ）
struct Container<'a> {
    /// 辞書の始まり
    start: usize,
    /// 前の `obj` の位置
    keyword: usize,
    stream: Stream<'a>,
}

/// 間接オブジェクトの辞書の範囲（始まりの順）と、オブジェクトのストリームになりうるストリーム
type Objects<'a> = (Vec<(usize, usize)>, Vec<Container<'a>>);

/// ファイルのすべての `obj` の後ろ（7.3.10）を読む。hayro-syntax は、表が指す位置の定義を読むので、レコードの始まりに
/// ない定義（文字列とストリームの中）も読みうる。数は、参照の先の候補にする。辞書は、手間を数えずに 1 度ずつ読む
/// ので、ほかの辞書と重ならなければ、手間は `InputBytes` に比例する。重なって読み直す分と、読めなかった辞書の後ろの
/// バイト列は、`Work` に数える
fn objects<'a>(src: &'a [u8], seen: &mut Seen, cx: &mut Cx<'_>) -> Result<Objects<'a>, Stop> {
    let context = ReaderContext::dummy();
    let mut dicts = Vec::new();
    let mut containers = Vec::new();
    let mut covered = 0;
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
        let mut reader = Reader::new(src);
        reader.jump(keyword + 3);
        reader.skip_white_spaces_and_comments();
        let start = reader.offset();
        if reader.peek_tag(b"<<").is_none() {
            if let Some(number) = reader.read_without_context::<Number>() {
                seen.number(number.as_i64());
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
        if start < covered {
            cx.check(
                Kind::Work,
                lookup_work(src.get(start..end).unwrap_or_default()),
            )?;
        }
        covered = covered.max(end);
        dicts.push((start, end));
        if !(dict.contains_key(b"N") && dict.contains_key(b"First")) {
            continue;
        }
        let mut reader = Reader::new(src);
        reader.jump(start);
        let Some(stream) = reader.read_with_context::<Stream<'_>>(&context) else {
            continue;
        };
        covered = covered.max(reader.offset());
        containers.push(Container {
            start,
            keyword,
            stream,
        });
    }
    // 始まりは増えるので、並びは始まりの順
    Ok((dicts, containers))
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

/// `obj` の前に書いた番号と世代番号（7.3.10）。hayro-syntax は表の位置から番号を読むので、数字の並びの途中から読み
/// 始めた番号も候補にする。読み始めうる位置から `obj` まで読めたものだけを使う
fn identifiers(src: &[u8], keyword: usize, cx: &mut Cx<'_>) -> Result<Vec<(u32, i32)>, Stop> {
    let context = ReaderContext::dummy();
    let mut out = Vec::new();
    for start in keyword.saturating_sub(IDENTIFIER_WINDOW)..keyword {
        if !src
            .get(start)
            .is_some_and(|&byte| byte.is_ascii_digit() || matches!(byte, b'+' | b'-' | b'.'))
        {
            continue;
        }
        let mut reader = Reader::new(src);
        reader.jump(start);
        let id = <ObjectIdentifier as Readable>::read(&mut reader, &context);
        cx.check(
            Kind::Work,
            lookup_work(src.get(start..reader.offset()).unwrap_or_default()).max(1),
        )?;
        if let Some(id) = id
            && reader.offset() == keyword + 3
        {
            out.push((id.obj_number, id.gen_number));
        }
    }
    Ok(out)
}

/// 読まない文書の診断
pub(crate) fn refused(place: Ref) -> Diagnostic {
    Diagnostic::new(zumen_base::codes::PDF_DOCUMENT_UNSUPPORTED).at(place)
}
