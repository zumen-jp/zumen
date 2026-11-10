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

use crate::{
    inflate,
    walk::{LOOKUP_BYTES, lookup_work},
};

/// 環境の大きさの整数で読む値の上限（含まない）。32 ビットの環境でも読め、2 つを足してもあふれない
const LIMIT: i64 = 1 << 31;

/// 予測子の 1 行のバイト数の上限。hayro-syntax は、展開するたびに 1 行の分を確保する。幅 65,536 画素の 16 ビットの
/// RGBA の行（512 KiB）は収まる
const ROW: i64 = 1 << 20;

/// LZW と ASCII85 で展開する入力のバイト数の上限（含まない）。hayro-syntax は、LZW の符号を読む位置をビットの数で、
/// ASCII85 の出力の大きさを入力の長さの 4 倍で求めるので、32 ビットの環境で 2 の 29 乗・2 の 30 乗バイトからあふれる
const CODED: u64 = 1 << 28;

/// フィルターの配列の、名前と参照の項目の数の上限（含む）。hayro-syntax は、展開するたびに、フィルターの数だけ
/// `DecodeParms` の配列の項目を辞書として読み、空の辞書でも 1 KiB ほどの表を持つ（`filters_and_params`）
const FILTERS: u64 = 64;

/// 展開のパラメーターの配列（[`listed`]）を見るバイト数の上限。ふつうの文書の配列は、数個の小さな辞書か `null` の
/// 項目で、数百バイトに収まる。上限で、確かめの手間を、見る位置の数の決まった倍までにする
const PARAMETER_BYTES: usize = 1024;

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
    /// F が参照する番号。読み直しの手間を調べるために外部ファイル名を展開しない。
    file_refs: BTreeSet<(u32, i32)>,
    /// 参照先になりうる 1 つの辞書が作る表とエスケープ付きのキーの上限。
    table_bytes: u64,
    numbers: Numbers,
    /// フィルターの値の LZW・ASCII85 の名前のうち、展開する入力が [`CODED`] に届きうるもの
    coded: Option<usize>,
    /// 名前と参照の項目が [`FILTERS`] を超えるフィルターの配列
    long: Option<usize>,
    /// フィルターの配列の、名前と参照の項目の数のいちばん大きいもの（[`FILTERS`] を超えれば、`long` で断る）。
    /// hayro-syntax は、展開のパラメーターの配列の項目を、フィルターの名前の数までしか読まない
    filters: u64,
    /// 展開のパラメーターの配列で、表を重ねて持ちうる項目（[`listed`]）。いちばん前の項目の番号と、配列の場所
    params: Option<(u64, usize)>,
    /// 参照の先になりうる位置の、LZW・ASCII85 の名前か、それを名前の項目に持つか、最初の参照の項目より前の名前の
    /// 項目だけで [`FILTERS`] を超える配列（[`Hit::Coded`]）
    target: Option<usize>,
    /// 参照の先になりうる位置の、参照の項目を含めると名前と参照の項目が [`FILTERS`] を超える配列（[`Hit::Crowded`]）
    crowded: Option<usize>,
    /// 参照の先になりうる位置に、名前がある。参照の項目が名前に解けるのは、このときだけ
    named: bool,
    /// 参照の先になりうる位置の配列の、最初の参照の項目より前の名前の項目の数と、名前と参照の項目の数の、それぞれ
    /// いちばん大きいもの（[`targeted`]）。フィルターの値の参照の先の配列から、hayro-syntax が読むフィルターの数
    leading: u64,
    items: u64,
    /// 参照の先になりうる位置の配列で、展開のパラメーターの配列として表を重ねて持ちうる項目（[`listed`]）
    listed: Option<(u64, usize)>,
    /// フィルターの値に参照がある。hayro-syntax は参照の先の名前と配列も読むので、参照の先になりうる位置の名前を、
    /// 倍率の分からないフィルターとして当てうる（その名前が、ほかの所でフィルターの値として読める名前でも）
    indirect: bool,
    /// 展開のパラメーターの値に参照がある。hayro-syntax は参照の先の配列も読む
    parameters: bool,
}

impl Seen {
    /// フィルターの値の LZW・ASCII85 の名前。入力の長さの上限（バイト列の長さと倍率の積）が [`CODED`] に届けば、
    /// 場所を記録する。同じ名前を、ほかのキーの値としても読めば（配列の中の `/F` の後ろなど。hayro-syntax は、配列の
    /// 中の `F` を知らない名前として飛ばす）、どちらの倍率でも確かめる
    fn bound(&mut self, length: u64, factor: u64, place: usize) {
        if length.saturating_mul(factor) >= CODED {
            self.coded = Some(self.coded.map_or(place, |at| at.min(place)));
        }
    }

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

    /// 参照の先になりうる位置の値（[`targeted`]）。断るかは、参照があるかと合わせて [`Seen::verdict`] で決める
    fn hit(&mut self, hit: Hit, place: usize) {
        match hit {
            Hit::Coded => {
                self.target.get_or_insert(place);
            }
            Hit::Crowded => {
                self.crowded.get_or_insert(place);
            }
            Hit::Name => self.named = true,
            Hit::Nothing => {}
        }
    }

    /// hayro-syntax が、展開のパラメーターの配列の項目を読みうる数。フィルターの値に参照があれば、参照の先の配列の
    /// 名前の項目も読む（名前の定義がなければ、参照の項目で止まる）
    fn reads(&self) -> u64 {
        let target = match (self.indirect, self.named) {
            (false, _) => 0,
            (true, false) => self.leading,
            (true, true) => self.items,
        };
        self.filters.max(target)
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
        // 展開のパラメーターの配列の項目は、フィルターの数より前のものだけを読む
        let filters = self.reads();
        let layered = |slot: Option<(u64, usize)>| {
            slot.filter(|&(index, _)| index < filters)
                .map(|(_, place)| place)
        };
        if let Some(place) = self
            .coded
            .or(self.long)
            .or_else(|| layered(self.params))
            .or_else(|| self.target.filter(|_| self.indirect))
            .or_else(|| self.crowded.filter(|_| self.indirect && self.named))
            .or_else(|| layered(self.listed).filter(|_| self.parameters))
        {
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
    /// 1 つのバイト列のすべての名前が辞書のキーだったときの確保量。
    key_bytes: u64,
    /// `Type` の値が `ObjStm` の名前の、最初のキーの位置
    typed: Option<usize>,
    /// そのうち、値の名前の字が、ここで展開するストリームの型の値の字でない最初のキーの位置（`#xx` で書いた名前は、
    /// バイト列の外に持つので、同じ字にならない）
    unclaimed: Option<usize>,
    /// `Type` の値が参照の位置
    referenced: Option<usize>,
    /// 暗号化の辞書の名前（`Encrypt`、7.6.1）の位置
    encrypted: Option<usize>,
}

pub(crate) enum Checked {
    Refused(usize),
    Ready {
        encrypted: bool,
        largest_container: u64,
        file_refs: BTreeSet<(u32, i32)>,
        table_bytes: u64,
    },
}

/// 読み込む前に確かめ、読まないなら、その理由のファイルのバイトの位置を返す。ファイルのすべての名前と定義を見て、
/// オブジェクトのストリームになりうるストリームを展開して、中の位置と中身を見る
pub(crate) fn before(src: &[u8], cx: &mut Cx<'_>) -> Result<Checked, Stop> {
    let mut seen = Seen::default();
    let containers = objects(src, &mut seen, cx)?;
    // 表を作り直すときに展開する辞書は、型の値が、ここで展開するストリームの上の段の型の値と同じ字か
    let claimed = claims(src, &containers, cx)?;
    let marks = names(src, |at| at, Some(&claimed), &mut seen, cx)?;
    // 暗号化した文書は、オブジェクトのストリームの中身を、読み込む前に hayro-syntax と同じには復号できない。
    // hayro-syntax は、表が指すストリームと、表を作り直すときに型が `ObjStm` の辞書（参照を解いたものも）を展開する
    if marks.encrypted.is_some()
        && let Some(at) = containers
            .first()
            .map(|container| container.start)
            .or(marks.typed)
            .or(marks.referenced)
    {
        return Ok(Checked::Refused(at));
    }
    if let Some(at) = marks.unclaimed {
        return Ok(Checked::Refused(at));
    }
    // フィルターか展開のパラメーターの値に参照があれば、参照の先になりうる位置の値を見る
    let scanned = (seen.indirect || seen.parameters) && seen.coded.is_none();
    if scanned {
        targets(src, &mut seen, cx)?;
    }
    // オブジェクトのストリームを展開する前に、展開のパラメーターを確かめる
    if let Some(place) = seen.verdict() {
        return Ok(Checked::Refused(place));
    }
    let mut largest_container = 0;
    for container in containers.iter().filter(|container| !container.copy) {
        let Some(stream) = container.stream(src, cx)? else {
            continue;
        };
        // 参照の値は、読む前に hayro-syntax と同じには解けない
        if referenced(stream.dict(), cx)? {
            return Ok(Checked::Refused(container.start));
        }
        if let Some(place) = container_fits(
            &stream,
            container.start,
            &mut seen,
            &mut largest_container,
            cx,
        )? {
            return Ok(Checked::Refused(place));
        }
    }
    // フィルターと展開のパラメーターの値の参照は、オブジェクトのストリームの中身にだけあることもある。中の位置は、
    // 参照があるかによらず見たので、ファイルの `obj` の後ろを見る
    if (seen.indirect || seen.parameters) && !scanned {
        targets(src, &mut seen, cx)?;
    }
    Ok(match seen.verdict() {
        Some(at) => Checked::Refused(at),
        None => Checked::Ready {
            encrypted: marks.encrypted.is_some(),
            largest_container,
            file_refs: seen.file_refs,
            table_bytes: seen.table_bytes.saturating_add(2048),
        },
    })
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
/// 見る）。そこで、確かめたストリームの辞書の型の値は、辞書の始まりと同じ行の、始まりより前に `%` がなく、辞書の
/// 始まりから型の値までに `%` も `)` もなければ、同じものとする
fn claims(src: &[u8], containers: &[Container], cx: &mut Cx<'_>) -> Result<BTreeSet<usize>, Stop> {
    let mut claimed = BTreeSet::new();
    // 型の値の字の位置ごとの、最初に見た辞書の始まりから型の値までの、最後の `%` か `)` の位置。同じ型の値を持つ
    // 辞書（同じストリームのコンテナー）は、始まりの順に見るので、後の辞書の始まりから型の値までは、その中にある
    // （#657）
    let mut last_bad = BTreeMap::<usize, Option<usize>>::new();
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
        let bad = match last_bad.get(&value) {
            Some(&bad) => bad,
            None => {
                let head = src.get(container.start..value).unwrap_or_default();
                cx.check(Kind::Work, lookup_work(head))?;
                cx.check(Kind::DecodedBytes, CLAIM_BYTES)?;
                let bad = head
                    .iter()
                    .rposition(|&byte| matches!(byte, b'%' | b')'))
                    .map(|at| container.start + at);
                last_bad.insert(value, bad);
                bad
            }
        };
        if bad.is_none_or(|bad| bad < container.start) {
            claimed.insert(value);
        }
    }
    Ok(claimed)
}

/// [`claims`] の、型の値ごとの覚えの 1 つのメモリ
const CLAIM_BYTES: u64 = 64;

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
    largest: &mut u64,
    cx: &mut Cx<'_>,
) -> Result<Option<usize>, Stop> {
    let dict = stream.dict();
    // hayro-syntax は、展開するときに `F`・`DP`・`DecodeParms` の辞書の表を作る。表は、値のキーの数に比例するので、
    // 値の名前の数で多めに数える（項目の辞書の表は、フィルターの数の上限から、ファイルの大きさによらない）
    for key in [b"F".as_slice(), b"DP", b"DecodeParms"] {
        if let Some(MaybeRef::NotRef(Raw(value))) = dict.get_raw::<Raw<'_>>(key) {
            cx.check(Kind::Work, lookup_work(value))?;
            let names = value.iter().filter(|&&byte| byte == b'/').count() as u64;
            cx.check(Kind::DecodedBytes, names.saturating_mul(KEY_BYTES))?;
        }
    }
    // hayro-syntax は、中の位置を読む前に展開する。`N` と `First` が読めなくても、展開の上限は先に確かめる
    // 参照のあるフィルターと暗号化したコンテナーは、ここへ来る前に断っている。
    let counted = inflate::before(stream, inflate::Source::default(), cx)?;
    let (Some(count), Some(first)) = (dict.get::<i64>(b"N"), dict.get::<i64>(b"First")) else {
        return Ok(None);
    };
    if count < 0 || first < 0 {
        return Ok(None);
    }
    if count >= LIMIT || first >= LIMIT {
        return Ok(Some(place));
    }
    let Ok(data) = stream.decoded_data() else {
        return Ok(None);
    };
    *largest = (*largest).max(data.len() as u64);
    if !counted {
        cx.check(Kind::DecodedBytes, data.len() as u64)?;
    }
    members(&data, count, first, place, seen, cx)
}

/// オブジェクトのストリームの中の位置と中身。hayro-syntax の `ObjectStream::read_offsets` と同じく、番号と位置の組を
/// `N` 個まで読み、前の位置より後ろで、展開したバイト列の中を指す位置だけを使う。中の位置が 2 の 31 乗以上なら、
/// 32 ビットの環境で読めないか、`First` と足すとあふれるので、場所を返す。使う位置の数は、参照の先の候補にする。
/// 位置が数の途中を指せば、数の終わりまでを位置ごとに読むので、読む字の数を `Work` に数える。中の位置は、参照の先に
/// なりうる位置として見る（[`targeted`]・[`listed`]）。フィルターと展開のパラメーターの値の参照は、後のコンテナーの
/// 中身にだけあることもあるので、参照があるかによらず見て、断るかは最後に決める
fn members(
    data: &[u8],
    count: i64,
    first: i64,
    place: usize,
    seen: &mut Seen,
    cx: &mut Cx<'_>,
) -> Result<Option<usize>, Stop> {
    // 展開から戻った直後に中断と、名前を探す 1 回の走査（16 バイトごと）の手間を確かめる。
    cx.check(Kind::Work, (data.len() as u64).div_ceil(16))?;
    let all_keys = names(data, |_| place, None, seen, cx)?.key_bytes;
    let mut table_end = 0;
    let mut table_scan = false;
    let mut header = Reader::new(data);
    let mut skip = Skip::default();
    let mut lists = Lists::default();
    let mut params = Lists::default();
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
        if seen.table_bytes < all_keys && data.get(at..).is_some_and(|tail| tail.starts_with(b"<<"))
        {
            // 次の項目が前の辞書の途中を指す場合は、同じ範囲を読み直さず、コンテナー全体の名前で上限を取る。
            if at < table_end {
                seen.table_bytes = seen.table_bytes.max(all_keys);
            } else {
                if !table_scan {
                    // ここから後の辞書は重ならない範囲だけ読む。初めて必要になった所からの上限を 1 回だけ先に数える。
                    cx.check(Kind::Work, lookup_work(data.get(at..).unwrap_or_default()))?;
                    table_scan = true;
                }
                let mut bytes = 0_u64;
                let mut gaps = Skip::default();
                let mut tokens = Tokens::default();
                let (walked, _) = entries(
                    data,
                    at,
                    &mut gaps,
                    &mut tokens,
                    data.len(),
                    cx,
                    |event, _| {
                        if let Event::Entry(_, cost, _) = event {
                            bytes = bytes.saturating_add(cost);
                        }
                        Ok(false)
                    },
                )?;
                table_end = match walked {
                    Walked::Closed(end) => end,
                    Walked::Broken(..) | Walked::Stopped(_) => data.len(),
                };
                seen.table_bytes = seen.table_bytes.max(bytes);
            }
        }
        targeted(data, at, place, &mut lists, seen, cx)?;
        if let Some(index) = listed(data, at, &mut params, cx)? {
            earliest(&mut seen.listed, index, place);
        }
    }
    Ok(None)
}

/// バイト列のすべての名前（7.3.5）のうち、値を確かめるキーと、型・フィルターのキーの後ろの値を見る。hayro-syntax の
/// 辞書と同じく、キーの後ろの空白とコメントを飛ばし、参照（`n g R`）か数として読む。辞書の中かを見ないので、文字列と
/// ストリームの中の名前も見る（多めに断るだけ）。名前の中に `/` はないので、名前の字は 1 度だけ読む。値を読む手間は
/// `Work` に数える。型の値は、`claimed`（ここで展開するストリームの型の値の字の位置）があれば、そこにないものを
/// 記録する。
///
/// 2 つのキーが同じ値の位置に着くのは、後のキーが、前のキーの飛ばした空白とコメントの中（コメントの中）にあり、
/// 飛ばした範囲の終わりに飛ぶときだけ。範囲の中で止まる値の前には空白しかないので、ほかのキーは着かない。そこで、
/// 範囲の終わりの値を読み方ごとに 1 つずつ覚え、値は、位置と読み方ごとに 1 度だけ読む（違う位置の値は、違う字句から
/// 読む）
fn names(
    data: &[u8],
    place: impl Fn(usize) -> usize,
    claimed: Option<&BTreeSet<usize>>,
    seen: &mut Seen,
    cx: &mut Cx<'_>,
) -> Result<Marks, Stop> {
    let mut marks = Marks::default();
    let mut skip = Skip::default();
    // 範囲の終わりの値として、型とフィルターの値に読んだ位置、キーの値に読んだ位置と読んだもの、フィルターの配列が
    // 読んだ項目の並び
    let mut typed = None;
    let mut filtered = None;
    let mut parameters = None;
    let mut read = None;
    let mut items = Items::default();
    let mut params = Lists::default();
    let mut at = 0;
    while let Some(start) = next_slash(data, at, cx)? {
        let mut reader = Reader::new(data);
        reader.jump(start);
        let name = scanned_name(data, &mut reader, cx)?;
        let end = reader.offset();
        at = end.max(start + 1);
        let Some((name, cost)) = name else {
            continue;
        };
        marks.key_bytes = marks.key_bytes.saturating_add(cost);
        let name: &[u8] = &name;
        let key = Key::of(name);
        match name {
            b"Encrypt" => {
                marks.encrypted.get_or_insert(start);
                continue;
            }
            b"Type" | b"Filter" | b"F" | b"DecodeParms" | b"DP" => {}
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
                if reference(data, value, cx)?.is_some() {
                    marks.referenced.get_or_insert(start);
                } else if let Some((name, next)) = read_name(data, value, cx)? {
                    cx.check(
                        Kind::Work,
                        lookup_work(data.get(value..next).unwrap_or_default()),
                    )?;
                    if &*name == b"ObjStm" {
                        marks.typed.get_or_insert(start);
                        if claimed.is_some_and(|claimed| {
                            !position(data, &name).is_some_and(|value| claimed.contains(&value))
                        }) {
                            marks.unclaimed.get_or_insert(start);
                        }
                    }
                }
            }
            (b"Filter" | b"F", _) => {
                if name == b"F"
                    && let Some((id, _)) = read_reference(data, value, cx)?
                {
                    let id = hayro_syntax::object::ObjectIdentifier::from(id);
                    let key = (id.obj_number, id.gen_number);
                    if !seen.file_refs.contains(&key) {
                        cx.check(Kind::DecodedBytes, crate::walk::REFERENCE_BYTES)?;
                        seen.file_refs.insert(key);
                    }
                }
                if !(shared && filtered.replace(value) == Some(value))
                    && filters(data, value, &place, &mut skip, &mut items, seen, cx)?
                {
                    seen.indirect = true;
                }
            }
            (b"DecodeParms" | b"DP", _) => {
                if shared && parameters.replace(value) == Some(value) {
                    continue;
                }
                if reference(data, value, cx)?.is_some() {
                    seen.parameters = true;
                } else if let Some(index) = listed(data, value, &mut params, cx)? {
                    earliest(&mut seen.params, index, place(start));
                }
            }
            (_, Some(key)) => {
                let found = match read {
                    Some((at, found)) if shared && at == value => found,
                    _ => {
                        let found = if reference(data, value, cx)?.is_some() {
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

/// フィルターの値（7.3.8.2、表 5 の `Filter`。hayro-syntax は `F` も読む）の LZW と ASCII85 の名前の、入力の長さの
/// 上限を確かめ（[`Seen::bound`]）、値か配列の項目が参照かを返す。hayro-syntax と同じく、名前か、名前の配列を読む。
/// 配列の参照の項目は、hayro-syntax が名前に解けば当てて、後ろの項目も読むので、後ろの項目は倍率の上限なしで確かめる
/// （名前に解けなければ hayro-syntax はそこで止まるので、多めに断るだけ）。名前でも参照でもない項目の後ろは、
/// hayro-syntax が当てないので、確かめない（参照の先の名前は、[`targeted`] で見る）。名前と参照の項目が [`FILTERS`] を
/// 超える配列は、場所を記録する。
///
/// 項目の間の空白とコメントは、キーの後ろを飛ばす覚え（`skip`）で飛ばすが、キーの側がまだ読む範囲は覚え直さない
/// （[`Skip::keeping`]）。コメントの中の配列は、どれも、覚えた範囲の終わりから同じ項目の並びを読みうる。そこで、範囲の
/// 終わりから読んだ並びは、初めのフィルターか（`first`）ごとに、読み始めの倍率のいちばん大きいものと、参照の項目が
/// あるかと、項目の数を覚える。後ろの項目の倍率は、読み始めの倍率が大きいほど大きいので、覚えた倍率より大きくなければ、
/// 同じ名前を前より小さい倍率で確かめるだけになり、読まない
fn filters(
    data: &[u8],
    at: usize,
    place: &impl Fn(usize) -> usize,
    skip: &mut Skip,
    items: &mut Items,
    seen: &mut Seen,
    cx: &mut Cx<'_>,
) -> Result<bool, Stop> {
    let length = data.len() as u64;
    if let Some((name, next)) = read_name(data, at, cx)? {
        cx.check(
            Kind::Work,
            lookup_work(data.get(at..next).unwrap_or_default()),
        )?;
        if coded(&name) {
            seen.bound(length, 1, place(at));
        }
        return Ok(false);
    }
    if data.get(at) != Some(&b'[') {
        return Ok(reference(data, at, cx)?.is_some());
    }
    let mut item = at + 1;
    let mut factor = 1u64;
    let mut first = true;
    // 範囲の終わりから読み始めた倍率と、初めのフィルターかと、そこまでの項目の数
    let mut start = None;
    let mut found = false;
    let mut count = 0u64;
    loop {
        item = skip.keeping(data, item, at, cx)?;
        if item == skip.to && start.is_none() {
            if items.at != item {
                *items = Items {
                    at: item,
                    ..Items::default()
                };
            }
            // 覚えた並びの参照と、この配列が前の項目に読んだ参照を合わせて返す
            if let Some((most, remembered)) = *items.slot(first)
                && factor <= most
            {
                let total = count.saturating_add(items.rest);
                seen.filters = seen.filters.max(total);
                if total > FILTERS {
                    seen.long.get_or_insert(place(at));
                }
                return Ok(found || remembered);
            }
            start = Some((factor, first, count));
        }
        let Some((name, next)) = read_name(data, item, cx)? else {
            // 参照の項目の先の名前の倍率は分からない
            let Some(end) = reference(data, item, cx)? else {
                break;
            };
            if counted(&mut count, seen, place(at)) {
                return Ok(found);
            }
            found = true;
            factor = u64::MAX;
            item = end;
            continue;
        };
        if counted(&mut count, seen, place(at)) {
            return Ok(found);
        }
        cx.check(
            Kind::Work,
            lookup_work(data.get(item..next).unwrap_or_default()),
        )?;
        if coded(&name) {
            seen.bound(length, factor, place(item));
        }
        let (known, ratio) = expansion(&name, first);
        factor = factor.saturating_mul(ratio);
        first &= !known;
        item = next;
    }
    if let Some((factor, first, before)) = start {
        *items.slot(first) = Some((factor, found));
        items.rest = count - before;
    }
    Ok(found)
}

/// フィルターの配列の名前か参照の項目を 1 つ数え（[`Seen::filters`]）、[`FILTERS`] を超えたら、配列の場所を記録して
/// 真を返す
fn counted(count: &mut u64, seen: &mut Seen, place: usize) -> bool {
    *count += 1;
    seen.filters = seen.filters.max(*count);
    let long = *count > FILTERS;
    if long {
        seen.long.get_or_insert(place);
    }
    long
}

/// フィルターの配列が、空白とコメントを飛ばした範囲の終わりから読んだ項目の並び（[`filters`]）。初めのフィルターか
/// （`first`）ごとに、読み始めの倍率のいちばん大きいものと、参照の項目があるかを持ち、並びの名前と参照の項目の数も
/// 持つ
#[derive(Debug, Default)]
struct Items {
    /// 範囲の終わり
    at: usize,
    first: Option<(u64, bool)>,
    later: Option<(u64, bool)>,
    /// 範囲の終わりからの、名前と参照の項目の数
    rest: u64,
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

/// 位置の名前と、その後ろの位置
fn read_name<'a>(
    data: &'a [u8],
    at: usize,
    cx: &mut Cx<'_>,
) -> Result<Option<(Name<'a>, usize)>, Stop> {
    let mut reader = Reader::new(data);
    reader.jump(at);
    Ok(scanned_name(data, &mut reader, cx)?.map(|(name, _)| (name, reader.offset())))
}

const NAME_CHUNK: usize = 1024;

fn next_slash(data: &[u8], mut at: usize, cx: &mut Cx<'_>) -> Result<Option<usize>, Stop> {
    for chunk in data.get(at..).unwrap_or_default().chunks(NAME_CHUNK) {
        cx.check(Kind::Work, 0)?;
        if let Some(found) = chunk.iter().position(|&b| b == b'/') {
            return Ok(Some(at + found));
        }
        at += chunk.len();
    }
    Ok(None)
}

/// 名前（7.3.5）の綴りを見て、判定用の名前と、上流の辞書にこのキーを置く確保量を返す。
/// hayro-syntax 0.9.0 の Name は # があれば不正なエスケープでも名前を所有する。正しい #xx だけを 1 字と数える。
/// 判定する最長の名前は BitsPerComponent（16 字）。これを超えるエスケープつきの名前は長さだけで判別できる。
/// 短い名前は上流の 23 バイトのインライン領域に収まる。長いキーのバッファーをここでは確保しない。
fn scanned_name<'a>(
    data: &'a [u8],
    reader: &mut Reader<'a>,
    cx: &mut Cx<'_>,
) -> Result<Option<(Name<'a>, u64)>, Stop> {
    if reader.forward_tag(b"/").is_none() {
        return Ok(None);
    }
    let start = reader.offset();
    let mut end = start;
    let mut owned = false;
    let mut escapes = 0;
    'name: for chunk in data.get(start..).unwrap_or_default().chunks(NAME_CHUNK) {
        cx.check(Kind::Work, 0)?;
        for &byte in chunk {
            if !regular(byte) {
                break 'name;
            }
            if byte == b'#' {
                owned = true;
                if data
                    .get(end + 1..end + 3)
                    .is_some_and(|pair| pair.iter().all(u8::is_ascii_hexdigit))
                {
                    escapes += 1;
                }
            }
            end += 1;
        }
    }
    reader.jump(end);
    let raw = data.get(start..end).unwrap_or_default();
    if !owned {
        return Ok(Some((Name::new_unescaped(raw), KEY_BYTES)));
    }
    let len = raw.len() - 2 * escapes;
    let name = if len <= b"BitsPerComponent".len() {
        Name::new(raw)
    } else {
        Some(Name::new_unescaped(b""))
    };
    Ok(name.map(|name| {
        (
            name,
            KEY_BYTES.saturating_add((len as u64).saturating_mul(4)),
        )
    }))
}

/// 位置が参照（7.3.10）なら、その後ろの位置。hayro-syntax の辞書と同じく、数の間の空白だけを飛ばす。読んだバイト数を
/// `Work` に数える（読めなければ hayro-syntax は位置を戻すので、位置の字句の長さを数える）
fn reference(data: &[u8], at: usize, cx: &mut Cx<'_>) -> Result<Option<usize>, Stop> {
    Ok(read_reference(data, at, cx)?.map(|(_, end)| end))
}

fn read_reference(
    data: &[u8],
    at: usize,
    cx: &mut Cx<'_>,
) -> Result<Option<(ObjRef, usize)>, Stop> {
    let mut reader = Reader::new(data);
    reader.jump(at);
    let found = <ObjRef as Readable>::read(&mut reader, &ReaderContext::dummy());
    let rest = data.get(at..).unwrap_or_default();
    let token = rest.iter().take_while(|&&byte| regular(byte)).count();
    let read = reader.offset().saturating_sub(at).max(token);
    cx.check(
        Kind::Work,
        lookup_work(rest.get(..read).unwrap_or_default()),
    )?;
    Ok(found.map(|id| (id, reader.offset())))
}

/// 位置の数を、hayro-syntax と同じく切り捨てた整数で読む。hayro-syntax の `Number::read` が読む字（符号・数字・
/// 小数点と、続く `-`）の並びと、その後ろの 1 字を `Work` に数える。字句の残り（`0obj0obj…` の `obj` から後ろなど）は
/// 見ない
fn number(data: &[u8], at: usize, cx: &mut Cx<'_>) -> Result<Option<i64>, Stop> {
    let rest = data.get(at..).unwrap_or_default();
    let digits = rest
        .iter()
        .take_while(|&&byte| matches!(byte, b'+' | b'-' | b'.' | b'0'..=b'9'))
        .count();
    cx.check(Kind::Work, lookup_work(rest.get(..=digits).unwrap_or(rest)))?;
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
/// 範囲の終わりより先には進まない。そこで、位置が増える順に飛ばせば、手間はバイト列の長さに比例する。[`Skip::over`] と
/// [`Skip::keeping`] は、読んだバイト数を `Work` に数え、[`Skip::holding`] は、読んだバイト数を返す
#[derive(Debug, Default)]
struct Skip {
    from: usize,
    to: usize,
    /// 範囲が行末か `%` を含む。含まなければ、範囲は空白だけで、名前を含まない
    lines: bool,
}

impl Skip {
    fn over(&mut self, data: &[u8], start: usize, cx: &mut Cx<'_>) -> Result<usize, Stop> {
        let (end, at) = self.walk(data, start, false);
        cx.check(
            Kind::Work,
            lookup_work(data.get(start..at).unwrap_or_default()),
        )?;
        Ok(end)
    }

    /// 配列の項目の間を飛ばす。覚えた範囲が行末か `%` を含み、キーの値（`cursor`）が範囲の終わりより後ろになければ、
    /// 範囲の外から飛ばしても覚え直さない。範囲の中の後ろのコメントの行のキーは、まだその範囲を使う（項目は、キーの
    /// 値より後ろにある）。キーの値が範囲の終わりにあるときも同じ。空白だけの範囲は、キーを含まないので覚え直す
    fn keeping(
        &mut self,
        data: &[u8],
        start: usize,
        cursor: usize,
        cx: &mut Cx<'_>,
    ) -> Result<usize, Stop> {
        let keep = self.lines && cursor <= self.to;
        let (end, at) = self.walk(data, start, keep);
        cx.check(
            Kind::Work,
            lookup_work(data.get(start..at).unwrap_or_default()),
        )?;
        Ok(end)
    }

    /// 辞書の項目の間を飛ばし（[`entries`]）、飛ばした先と、読んだバイト数を返す。覚えた範囲が行末か `%` を含み、次の
    /// `obj`（`hold`）を含めば、範囲の外から飛ばしても覚え直さない。次の `obj` の辞書は、範囲の中のコメントの行から
    /// 読み始め、項目の間で範囲の中の行末に来る。読んだバイト数は、呼ぶ側が、重なって読む辞書の分だけ `Work` に数える
    fn holding(
        &mut self,
        data: &[u8],
        start: usize,
        hold: usize,
        cx: &mut Cx<'_>,
    ) -> Result<(usize, usize), Stop> {
        cx.check(Kind::Work, 0)?;
        let keep = self.lines && (self.from..self.to).contains(&hold);
        let (end, at) = self.walk(data, start, keep);
        Ok((end, at.saturating_sub(start)))
    }

    /// 飛ばした先と、読み終えた位置（範囲の終わりに飛んだときは、飛ぶ前の位置）を返す。範囲の外から飛ばしたら、
    /// `keep` が偽なら、飛ばした範囲を覚え直す
    fn walk(&mut self, data: &[u8], start: usize, keep: bool) -> (usize, usize) {
        let range = self.from..self.to;
        let mut at = start;
        let mut end = None;
        let mut lines = false;
        while let Some(&byte) = data.get(at) {
            let line = matches!(byte, b'\n' | b'\r' | b'%');
            lines |= line;
            if line && range.contains(&at) {
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
        let end = end.unwrap_or(at);
        if !range.contains(&start) && !keep {
            *self = Self {
                from: start,
                to: end,
                lines,
            };
        }
        (end, at)
    }
}

/// 辞書の上の段で読む文字列と配列の、読み終えた先の覚え（[`entries`]、#657）。コメントの行ごとの `obj<<` から読む
/// 辞書の最初の字句が、後ろの行をまたぐ文字列か配列なら、辞書ごとにその字句を終わりまで読み直さないようにする。
///
/// - 文字列（7.3.4.2）: hayro-syntax の `skip_literal` と同じく、`\` の次の字を飛ばし、括弧の深さを数える。読んだ文字列の
///   中のエスケープしない `(` は、そこから読んでも、エスケープの組み方が同じで、深さが一定の差になるので、同じ `)` で
///   閉じる。そこで、読むたびに括弧の対応を覚え、閉じない括弧は終わりまで閉じないと覚える。
/// - 配列（7.3.6）と辞書（7.3.7）: hayro-syntax の `Array::skip_with_depth` と `Dict::skip_with_depth` と同じく、
///   項目の間の空白とコメントを飛ばし、配列の項目と辞書の値を `MaybeRef<Object>` として、辞書のキーでない字句を
///   `Object` として、1 つ深く飛ばす。項目の始まりから後ろは、位置と深さで決まるので（深さの上限で読めなくなる所が
///   変わる）、どの配列か辞書も、同じ項目と終わりを読む。そこで、項目の始まりと深さごとに、終わりを覚える。入れ子の
///   配列と辞書も同じく読み、覚える（#747）。
///
/// 覚えは、1 つを [`TOKEN_BYTES`] として `DecodedBytes` に、読んだバイト列を `Work` に数える
#[derive(Default)]
struct Tokens {
    /// エスケープしない `(` の位置と、閉じた `)` の後ろ。閉じなければ `None`
    strings: BTreeMap<usize, Option<usize>>,
    /// 配列か辞書（真）の項目の始まりと、配列か辞書の深さから、その `]` か `>>` の後ろ。読めなければ `None`
    items: BTreeMap<(usize, usize, bool), Option<usize>>,
    /// 読んだバイト数の合計と、`Work` に数えた分（[`Tokens::count`]）
    scanned: u64,
    counted: u64,
}

/// 文字列と配列と辞書の覚えの 1 つのメモリ（位置の組と、表の木の節）
const TOKEN_BYTES: u64 = 48;

/// 配列と辞書の覚えを 1 つ足す手間を、読んだバイト数として数える量（`Work` の 20）
const INSERT_BYTES: usize = 20 * LOOKUP_BYTES as usize;

/// hayro-syntax 0.9.0 の、配列と辞書の入れ子の深さの上限（`MAX_OBJECT_NESTING_DEPTH`）
const NESTING: usize = 512;

impl Tokens {
    /// 読んだバイト数を足し、合計の [`LOOKUP_BYTES`] ごとに `Work` に数える。読み方の分け方によらず、合計で決まる
    fn count(&mut self, bytes: usize, cx: &mut Cx<'_>) -> Result<(), Stop> {
        self.scanned = self.scanned.saturating_add(bytes as u64);
        let work = self.scanned / LOOKUP_BYTES;
        cx.check(Kind::Work, work.saturating_sub(self.counted))?;
        self.counted = work;
        Ok(())
    }

    /// 辞書の上の段（深さ 1）の字句を、`reader` の位置から飛ばす。`Some` は飛ばせたか。文字列と配列は覚えから飛ばす。
    /// ほかは hayro-syntax で飛ばす（`value` なら `MaybeRef<Object>`、でなければ `Object`）。文字列と配列で読んだ
    /// バイト数（覚えから飛ばした所を除く）は、ここで `Work` に数える。返す数は、hayro-syntax で読んだバイト数（呼ぶ側
    /// が、重なる辞書の分だけ `Work` に数える）
    fn skip(
        &mut self,
        src: &[u8],
        reader: &mut Reader<'_>,
        value: bool,
        cx: &mut Cx<'_>,
    ) -> Result<(Option<()>, usize), Stop> {
        let at = reader.offset();
        if !matches!(src.get(at..at + 1), Some(b"(" | b"[")) && src.get(at..at + 2) != Some(b"<<") {
            let skipped = if value {
                reader.skip_with_depth::<MaybeRef<Object<'_>>>(false, 1)
            } else {
                reader.skip_with_depth::<Object<'_>>(false, 1)
            };
            return Ok((skipped.map(|_| ()), reader.offset().saturating_sub(at)));
        }
        let mut read = 0;
        let skipped = self.value(src, reader, 1, false, &mut read, cx)?;
        if src.get(at) == Some(&b'<') {
            // 辞書は、hayro-syntax で飛ばすほかの字句と同じく、呼ぶ側が、重なる辞書の分だけ数える
            return Ok((skipped, read));
        }
        // 文字列と配列の読みは、重なるかによらず数える（覚えに着かずに読み直す形も、予算で止める）
        self.count(read, cx)?;
        Ok((skipped, 0))
    }

    /// `reader` の位置の値を、hayro-syntax の `skip_with_depth` と同じく、深さ `depth` で飛ばす（`reference` なら
    /// `MaybeRef<Object>`、でなければ `Object`）。文字列と配列と辞書は覚えから飛ばす。読んだバイト数（覚えから
    /// 飛ばした所を除く）を `read` に足す。飛ばせなければ、位置を戻して `None`
    fn value(
        &mut self,
        src: &[u8],
        reader: &mut Reader<'_>,
        depth: usize,
        reference: bool,
        read: &mut usize,
        cx: &mut Cx<'_>,
    ) -> Result<Option<()>, Stop> {
        let at = reader.offset();
        if reference && reader.skip::<ObjRef>(false).is_some() {
            *read += reader.offset().saturating_sub(at);
            return Ok(Some(()));
        }
        if src.get(at) == Some(&b'(') {
            let (end, scanned) = self.string(src, at, cx)?;
            *read += scanned;
            return Ok(end.map(|end| reader.jump(end)));
        }
        let dict = src.get(at..at + 2) == Some(b"<<");
        if dict || src.get(at) == Some(&b'[') {
            return self.container(src, reader, depth, dict, read, cx);
        }
        let skipped = reader
            .skip_with_depth::<Object<'_>>(false, depth)
            .map(|_| ());
        // 飛ばせなければ hayro-syntax は位置を戻すので、終わりまで読みえたとして数える
        *read += match skipped {
            Some(()) => reader.offset().saturating_sub(at),
            None => src.len().saturating_sub(at),
        };
        Ok(skipped)
    }

    /// `at` の `(` からの文字列の終わりと、読んだバイト数
    fn string(
        &mut self,
        src: &[u8],
        at: usize,
        cx: &mut Cx<'_>,
    ) -> Result<(Option<usize>, usize), Stop> {
        if let Some(&end) = self.strings.get(&at) {
            return Ok((end, 0));
        }
        let mut open = vec![at];
        let mut next = at + 1;
        let mut scanned = 1;
        while let Some(&byte) = src.get(next) {
            next += 1;
            scanned += 1;
            match byte {
                b'\\' => {
                    if src.get(next).is_none() {
                        break;
                    }
                    next += 1;
                    scanned += 1;
                }
                // 括弧ごとに覚えを引くので、配列の項目と同じく、1 つを 2 として `Work` に数える
                b'(' => {
                    scanned += 2 * LOOKUP_BYTES as usize;
                    match self.strings.get(&(next - 1)) {
                        // 前に読んだ括弧は、覚えた `)` まで飛ばす（深さは変わらない）。閉じない括弧なら、外の括弧も閉じない
                        Some(&Some(end)) => next = end,
                        Some(None) => break,
                        None => {
                            cx.check(Kind::DecodedBytes, TOKEN_BYTES)?;
                            open.push(next - 1);
                        }
                    }
                }
                b')' => {
                    if let Some(start) = open.pop() {
                        self.strings.insert(start, Some(next));
                    }
                    if open.is_empty() {
                        break;
                    }
                }
                _ => {}
            }
        }
        cx.check(Kind::DecodedBytes, TOKEN_BYTES)?;
        // 終わりまで閉じない括弧は、どこから読んでも閉じない
        for start in open {
            self.strings.insert(start, None);
        }
        Ok((self.strings.get(&at).copied().flatten(), scanned))
    }

    /// `reader` の位置の `[` か `<<`（`dict`）からの、深さ `depth` の配列か辞書を飛ばす。覚えるのは、前の項目との間に
    /// 行末か `%` がある項目だけにする。ほかの配列か辞書が同じ項目の始まりに着くのは、その `[` か `<<` が、この配列か
    /// 辞書のコメントの中にあり、コメントの行末の後で同じ項目を読むとき
    fn container(
        &mut self,
        src: &[u8],
        reader: &mut Reader<'_>,
        depth: usize,
        dict: bool,
        read: &mut usize,
        cx: &mut Cx<'_>,
    ) -> Result<Option<()>, Stop> {
        // hayro-syntax の深さの上限（`MAX_OBJECT_NESTING_DEPTH`）
        if depth >= NESTING {
            return Ok(None);
        }
        let at = reader.offset();
        let (open, close): (usize, &[u8]) = if dict { (2, b">>") } else { (1, b"]") };
        reader.jump(at + open);
        *read += open;
        let mut items = Vec::new();
        let end = loop {
            cx.check(Kind::Work, 0)?;
            let gap = reader.offset();
            reader.skip_white_spaces_and_comments();
            let item = reader.offset();
            *read += item - gap;
            if reader.forward_tag(close).is_some() {
                *read += close.len();
                break Some(reader.offset());
            }
            if let Some(&end) = self.items.get(&(item, depth, dict)) {
                break end;
            }
            let lines = src
                .get(gap..item)
                .is_some_and(|gap| gap.iter().any(|&byte| matches!(byte, b'\n' | b'\r' | b'%')));
            if lines {
                cx.check(Kind::DecodedBytes, TOKEN_BYTES)?;
                // 覚えを足す手間（大きな表で 1 つ数百 ns）も数える。深さの違う辞書が同じ項目に着く形では、辞書ごとに
                // 深さの上限まで覚えを足す（PR #750 の確かめ）
                *read += INSERT_BYTES;
                items.push(item);
            }
            // 項目ごとに覚えを引くので、覚えから飛ばした項目も、1 つを 2 として `Work` に数える
            *read += 2 * LOOKUP_BYTES as usize;
            let skipped = if dict && reader.skip::<Name<'_>>(false).is_some() {
                // キーの後ろの値。キーでない字句は、値と同じ深さの `Object` として飛ばす
                reader.skip_white_spaces_and_comments();
                *read += reader.offset() - item;
                self.value(src, reader, depth + 1, true, read, cx)?
            } else {
                self.value(src, reader, depth + 1, !dict, read, cx)?
            };
            if skipped.is_none() {
                break None;
            }
        };
        for item in items {
            self.items.insert((item, depth, dict), end);
        }
        Ok(match end {
            Some(end) => {
                reader.jump(end);
                Some(())
            }
            None => {
                reader.jump(at);
                None
            }
        })
    }
}

/// オブジェクトのストリームになりうるストリーム（`N` と `First` を持つ）。辞書の表を持ち続けないように、位置だけを
/// 持ち、確かめるときに読み直す
struct Container {
    /// 辞書の始まり
    start: usize,
    /// 上の段の `Type` が `ObjStm` の名前なら、その字の位置
    typed: Option<usize>,
    /// 前に読んだ辞書かストリームと重なる
    overlapped: bool,
    /// 前のコンテナーと同じストリーム（終わりと、[`STREAM_KEYS`] の値の位置が同じ辞書）。展開して確かめるのは、前の
    /// コンテナーだけにする（#657）
    copy: bool,
}

/// 同じストリームを持つ辞書を見分けるもの（[`Container::copy`]）
type StreamKey = (usize, [Option<usize>; STREAM_KEYS.len()]);

/// 読んだストリームの覚えの 1 つのメモリ（[`StreamKey`] と、読み終えた所と型の値の位置と、表の木の節）
const STREAM_BYTES: u64 = 160;

/// コンテナーの 1 つのメモリ（[`Container`] と、型の値の字の位置の表の木の節）
const CONTAINER_BYTES: u64 = 64;

impl Container {
    /// ストリームを読み直す。ほかと重ならないものは、読み直しても手間が `InputBytes` に比例するので、重なるものの
    /// 読み直すバイト列だけを `Work` に数える
    fn stream<'a>(&self, src: &'a [u8], cx: &mut Cx<'_>) -> Result<Option<Stream<'a>>, Stop> {
        let mut reader = Reader::new(src);
        reader.jump(self.start);
        let stream = reader.read_with_context::<Stream<'_>>(&ReaderContext::dummy());
        if self.overlapped {
            cx.check(
                Kind::Work,
                lookup_work(src.get(self.start..reader.offset()).unwrap_or_default()),
            )?;
        }
        Ok(stream)
    }
}

/// 辞書の項目を読む歩み。項目の始まり（`value` が偽）か、キーの後ろの値（`value` が真）の位置。hayro-syntax の辞書
/// は、同じ歩みからは、同じ項目と終わりを読む
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Step {
    at: usize,
    value: bool,
}

/// 辞書を読み終えた形（[`entries`]）
#[derive(Debug, Clone, Copy)]
enum Walked {
    /// `>>` で閉じた。終わりの位置
    Closed(usize),
    /// 字句を読めなかった。その字句の位置と、hayro-syntax が字句を終わりまで読みうるか（文字列と配列は [`Tokens`] が
    /// 読んで数えたので偽）
    Broken(usize, bool),
    /// 呼ぶ側が止めた歩み
    Stopped(Step),
}

/// 辞書を読む途中で、呼ぶ側に渡すもの（[`entries`]）
enum Event<'a> {
    /// 歩みに着いた
    Step(Step),
    /// 項目のキーと、キーの表の 1 つのメモリと、値の位置
    Entry(Name<'a>, u64, usize),
}

/// コンテナーを読むときに hayro-syntax と確かめが引くキー（[`referenced`]・[`container_fits`]・`inflate`）。どのキーも
/// 最後の値の位置が同じで、終わりが同じ辞書は、同じストリームを同じく展開する
const STREAM_KEYS: [&[u8]; 8] = [
    b"Length",
    b"N",
    b"First",
    b"Filter",
    b"F",
    b"DecodeParms",
    b"DP",
    b"Type",
];
const KEY_N: usize = 1;
const KEY_FIRST: usize = 2;
const KEY_TYPE: usize = 7;

/// 覚えた歩みから後ろの項目（[`objects`]）
#[derive(Debug, Clone, Copy, Default)]
struct Suffix {
    /// 辞書の終わり。読めない辞書は `None`
    end: Option<usize>,
    /// [`STREAM_KEYS`] のキーの、最後の値の位置
    keys: [Option<usize>; STREAM_KEYS.len()],
    /// キーの表のメモリ
    bytes: u64,
}

/// 読んでいる辞書の項目（[`objects`]）
#[derive(Default)]
struct Own {
    /// キーの表のメモリ
    bytes: u64,
    /// [`STREAM_KEYS`] のキーの、最後の値の位置
    keys: [Option<usize>; STREAM_KEYS.len()],
    /// 覚える歩みと、そこまでのキーの表のメモリ
    steps: Vec<(Step, u64)>,
}

impl Own {
    fn entry(&mut self, name: &[u8], cost: u64, value: usize) {
        self.bytes = self.bytes.saturating_add(cost);
        if let Some(index) = STREAM_KEYS.iter().position(|key| *key == name)
            && let Some(slot) = self.keys.get_mut(index)
        {
            *slot = Some(value);
        }
    }

    /// `at` から後ろの項目。`prefix` は `at` までのキーの表のメモリ、`tail` は読むのをやめた所から後ろの項目。`at` から
    /// 後ろの項目は、値の位置が `at` より後ろにある
    fn after(&self, at: usize, prefix: u64, tail: &Suffix) -> Suffix {
        let mut keys = tail.keys;
        for (slot, own) in keys.iter_mut().zip(self.keys) {
            *slot = slot.or(own.filter(|&value| value > at));
        }
        Suffix {
            end: tail.end,
            keys,
            bytes: self.bytes.saturating_sub(prefix).saturating_add(tail.bytes),
        }
    }
}

/// 覚えた歩みの 1 つのメモリ（[`Step`] と [`Suffix`] と、表の木の節と、歩みの並びの分）
const MARK_BYTES: u64 = 128;

/// ファイルのすべての `obj` の後ろ（7.3.10）を読み、オブジェクトのストリームになりうるストリームを始まりの順に返す。
/// hayro-syntax は、表が指す位置の定義を読むので、レコードの始まりにない定義（文字列とストリームの中）も読みうる。
/// 数は、参照の先の候補にする。辞書は、キーの表を作らずに（[`entries`]）、手間を数えずに 1 度ずつ読むので、ほかの
/// 辞書と重ならなければ、手間は `InputBytes` に比例する。重なる辞書が読んだバイト数と、読めなかった字句とストリームの
/// 後ろのバイト列は、`Work` に数える。コンテナーの辞書を 2 回読む前に、キーごとの [`KEY_BYTES`] とエスケープした名前の
/// 確保を 2 回分、コンテナー 1 つを [`CONTAINER_BYTES`] として `DecodedBytes` に数える。
///
/// 2 つの `obj` が同じ位置に着くのは、後の `obj` が、前の `obj` の後ろで飛ばした空白とコメントの中（コメントの中）に
/// あり、飛ばした範囲の終わりに飛ぶときだけ。`obj` の位置は増える一方なので、範囲の終わりも増える一方で、範囲の中で
/// 止まる位置の前には空白しかなく、ほかの `obj` は着かない。そこで、範囲の終わりの位置を 1 つ覚え、位置ごとに 1 度
/// だけ読む。辞書の後ろは、`obj` の側の範囲を覚え直さないように、別の覚えで飛ばす。
///
/// hayro-syntax の辞書は、`<<` の後ろの空白とコメントを飛ばしてから項目を読むので、始まりの違う辞書も、そこで同じ
/// 位置に着けば、同じ項目と終わりとストリームを持つ。後の辞書の始まりは、前の辞書の `<<` の後ろのコメントの中にあり、
/// 同じ行の前に `%` があるので、型の値を確かめたものにしない（[`claims`]）。そこで、`<<` の後ろも同じく範囲の終わりの
/// 位置を 1 つ覚え、最初の辞書だけを読む（コメントの行ごとの `obj<<` から、後ろの同じ大きな辞書を読み直さない）。
/// 前の辞書より前から始まる辞書は、前の辞書の始まりの `<<` で止まるので、同じ位置に着かない。
///
/// `<<` の後ろで違う位置に着いた辞書も、前の辞書が読んだ歩み（[`Step`]）に着けば、そこから後ろは前の辞書と同じ項目を
/// 読む（コメントの行ごとの `obj<<` が、`<<` の後ろで、それぞれ次の行のキーに着くなど）。そこで、次の `obj` より後ろの
/// 歩みごとに、そこから後ろの項目（[`Suffix`]）を覚え、覚えた歩みに着いたら読むのをやめて、覚えた項目を足す。後ろの
/// `obj` の辞書は、その `obj` より後ろから読むので、次の `obj` より前の歩みには着かない。どの歩みからも、読み続ける辞書は
/// 1 つだけになる（歩みの間の 1 つの字句が、後ろの行をまたぐ文字列か配列なら、その字句は辞書ごとに読む）。重なる辞書は、
/// 読んだバイト数を `Work` に数え、覚えた歩みは、1 つを [`MARK_BYTES`] として `DecodedBytes` に数える。項目の間の空白と
/// コメントは、辞書をまたぐ 1 つの覚えで飛ばす（[`Skip::holding`]）
fn objects(src: &[u8], seen: &mut Seen, cx: &mut Cx<'_>) -> Result<Vec<Container>, Stop> {
    let context = ReaderContext::dummy();
    let mut containers = Vec::<Container>::new();
    // 範囲の終わりとして読んだ位置と、辞書の `<<` の後ろを飛ばす覚えの範囲の終わりとして読んだ位置
    let mut last = None;
    let mut entered = None;
    let mut covered = 0;
    let mut skip = Skip::default();
    let mut inner = Skip::default();
    let mut trailing = Skip::default();
    // 辞書の項目の間を飛ばす覚えと、前の辞書が読んだ歩みごとの、そこから後ろの項目
    let mut gaps = Skip::default();
    let mut tokens = Tokens::default();
    let mut marks = BTreeMap::<Step, Suffix>::new();
    // 読んだストリームの、終わりとキーの値の位置ごとの、読み終えた所と型の値の字の位置と、展開して確かめるコンテナーの
    // 番号（並べ替える前の `containers` の位置）。読めなければ `None`
    let mut streams = BTreeMap::<StreamKey, Option<(usize, Option<usize>, usize)>>::new();
    let mut keywords = (0..src.len().saturating_sub(2))
        .filter(|&keyword| definition(src, keyword))
        .peekable();
    while let Some(keyword) = keywords.next() {
        // ここから後ろの辞書は、この `obj` より後ろから読む
        while marks
            .first_key_value()
            .is_some_and(|(step, _)| step.at <= keyword)
        {
            marks.pop_first();
        }
        let start = skip.over(src, keyword + 3, cx)?;
        if start == skip.to && last.replace(start) == Some(start) {
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
        let first = inner.over(src, start + 2, cx)?;
        if first == inner.to && entered.replace(first) == Some(first) {
            continue;
        }
        // 辞書の表を作らずに、キーの数と、`N`・`First` と最後の型の値の位置を見る。次の `obj` より後ろの歩みを覚え、前の
        // 辞書が覚えた歩みに着いたら、そこで読むのをやめる
        let hold = keywords.peek().copied().unwrap_or(src.len());
        let mut own = Own::default();
        // 前の歩みの位置
        let mut last = start;
        let (walked, read) = entries(src, start, &mut gaps, &mut tokens, hold, cx, |event, cx| {
            match event {
                Event::Entry(name, cost, value) => own.entry(&name, cost, value),
                Event::Step(step) if marks.contains_key(&step) => return Ok(true),
                Event::Step(step) => {
                    // 後ろの `obj` の辞書が着きうる歩みだけを覚える。後ろの `obj` の辞書は、この辞書がコメントか文字列
                    // などの中に読んだ `obj` から始まり、そこから出た後の歩みで、この辞書と同じ所を読みうる。そこで、前の
                    // 歩みからこの歩みまでに、次の `obj` から後ろの `obj` があるときだけ覚える（#657）。閉じない辞書が
                    // 後ろのオブジェクトの字句をすべて読んでも、覚える数は `obj` の数を超えない
                    let from = last.max(hold);
                    let reached =
                        step.at > hold && (from..step.at).any(|keyword| definition(src, keyword));
                    // 探す所は、この辞書が読んだ所の中にあるので、読んだバイト数と同じに扱う
                    last = step.at;
                    if reached {
                        cx.check(Kind::DecodedBytes, MARK_BYTES)?;
                        own.steps.push((step, own.bytes));
                    }
                }
            }
            Ok(false)
        })?;
        let tail = match walked {
            Walked::Closed(end) => Suffix {
                end: Some(end),
                ..Suffix::default()
            },
            Walked::Broken(..) => Suffix::default(),
            Walked::Stopped(step) => marks.get(&step).copied().unwrap_or_default(),
        };
        for &(step, prefix) in &own.steps {
            marks.insert(step, own.after(step.at, prefix, &tail));
        }
        let whole = own.after(start, 0, &tail);
        // 途中で壊れた辞書でも、その前のキーの表は作られる。
        seen.table_bytes = seen.table_bytes.max(whole.bytes);
        let prior = covered;
        if start < prior || matches!(walked, Walked::Stopped(_)) {
            cx.check(Kind::Work, (read as u64).div_ceil(LOOKUP_BYTES))?;
        }
        match walked {
            Walked::Closed(end) => covered = covered.max(end),
            // 読めない字句は、終わりまで読みうる。文字列と配列と辞書は、読んだ分を数えた
            Walked::Broken(at, tail) => {
                if tail {
                    cx.check(Kind::Work, lookup_work(src.get(at..).unwrap_or_default()))?;
                    covered = covered.max(at);
                } else {
                    // 読めない文字列と配列と辞書は、終わりまで読みうる（[`Tokens`] が読んだ分は `read` にある）。後ろの辞書は、
                    // どれも重なる辞書として、読んだバイト数を `Work` に数える（覚えに着かずに読み直す形も、予算で止める）
                    covered = src.len();
                }
            }
            Walked::Stopped(step) => covered = covered.max(step.at),
        }
        let Some(end) = whole.end else {
            continue;
        };
        let has = |index: usize| whole.keys.get(index).copied().flatten().is_some();
        if !(has(KEY_N) && has(KEY_FIRST)) {
            continue;
        }
        // ストリームでない辞書は、hayro-syntax も展開しない
        let after = trailing.over(src, end, cx)?;
        if src
            .get(after..)
            .is_none_or(|rest| !rest.starts_with(b"stream"))
        {
            continue;
        }
        // 終わりと、ストリームを読むキーの値の位置が前の辞書と同じなら、同じストリームなので読み直さない（#657）
        let same = (end, whole.keys);
        if let Some(read) = streams.get_mut(&same) {
            if let Some((reached, typed, first)) = read {
                covered = covered.max(*reached);
                cx.check(Kind::DecodedBytes, CONTAINER_BYTES)?;
                // 展開して確かめるのは、始まりのいちばん前の辞書にする（断るときに返す位置を、読み直す形と同じにする）
                let earlier = containers
                    .get(*first)
                    .is_some_and(|container| start < container.start);
                if earlier && let Some(container) = containers.get_mut(*first) {
                    container.copy = true;
                    *first = containers.len();
                }
                containers.push(Container {
                    start,
                    typed: *typed,
                    overlapped: start < prior,
                    copy: !earlier,
                });
            }
            continue;
        }
        cx.check(Kind::DecodedBytes, STREAM_BYTES)?;
        // ストリームをここで読み、確認時にもう一度読む。どちらの辞書の表と名前の確保も先に数える。
        cx.check(Kind::DecodedBytes, whole.bytes.saturating_mul(2))?;
        let mut reader = Reader::new(src);
        reader.jump(start);
        if reader.read_with_context::<Stream<'_>>(&context).is_none() {
            // `Length` が合わなければ、hayro-syntax は `endstream` を終わりまで探す
            cx.check(
                Kind::Work,
                lookup_work(src.get(start..).unwrap_or_default()),
            )?;
            streams.insert(same, None);
            continue;
        }
        let reached = reader.offset();
        // ストリームを読むときは、辞書も読み直す
        if start < prior {
            cx.check(
                Kind::Work,
                lookup_work(src.get(start..reached).unwrap_or_default()),
            )?;
        }
        covered = covered.max(reached);
        // hayro-syntax の `Dict::get` と同じく、最後の型の値を、参照でない名前として読む
        let typed = match whole.keys.get(KEY_TYPE).copied().flatten() {
            Some(at) => read_name(src, at, cx)?
                .filter(|(name, _)| &**name == b"ObjStm")
                .and_then(|(name, _)| position(src, &name)),
            None => None,
        };
        cx.check(Kind::DecodedBytes, CONTAINER_BYTES)?;
        streams.insert(same, Some((reached, typed, containers.len())));
        containers.push(Container {
            start,
            typed,
            overlapped: start < prior,
            copy: false,
        });
    }
    // コメントの中の `obj` から読んだ辞書は、後ろの `obj` の辞書より前に始まりうる
    containers.sort_by_key(|container| container.start);
    Ok(containers)
}

/// `start` の `<<` からの辞書（7.3.7）を、hayro-syntax の `Dict::read`（`parse_dict_with`）と同じく読み、歩みごとと
/// 項目ごとに `visit` を呼んで、読み終えた形と、読んだバイト数を返す。`visit` が歩みで真を返したら、そこで読むのを
/// やめる。hayro-syntax と違い、キーの表を作らない。キーでない字句は値として飛ばし、キーの後ろは空白とコメントを
/// 飛ばした位置を値の位置にする。同じキーが何度もあれば、どれでも呼ぶ（hayro-syntax は最後のものを使う）。空白と
/// コメントは `gaps` で飛ばす（[`Skip::holding`]）
fn entries<'a>(
    src: &'a [u8],
    start: usize,
    gaps: &mut Skip,
    tokens: &mut Tokens,
    hold: usize,
    cx: &mut Cx<'_>,
    mut visit: impl FnMut(Event<'a>, &mut Cx<'_>) -> Result<bool, Stop>,
) -> Result<(Walked, usize), Stop> {
    let mut reader = Reader::new(src);
    reader.jump(start);
    if reader.forward_tag(b"<<").is_none() {
        return Ok((Walked::Broken(start, true), 0));
    }
    let mut read = 2;
    loop {
        let (at, gap) = gaps.holding(src, reader.offset(), hold, cx)?;
        read += gap;
        reader.jump(at);
        let step = Step { at, value: false };
        if visit(Event::Step(step), cx)? {
            return Ok((Walked::Stopped(step), read));
        }
        if reader.forward_tag(b">>").is_some() {
            return Ok((Walked::Closed(reader.offset()), read + 2));
        }
        let Some((name, cost)) = scanned_name(src, &mut reader, cx)? else {
            let (skipped, bytes) = tokens.skip(src, &mut reader, false, cx)?;
            read += bytes;
            if skipped.is_none() {
                return Ok((Walked::Broken(at, !tokened(src, at)), read));
            }
            continue;
        };
        read += reader.offset().saturating_sub(at);
        let (value, gap) = gaps.holding(src, reader.offset(), hold, cx)?;
        read += gap;
        reader.jump(value);
        // 項目では読むのをやめない
        visit(Event::Entry(name, cost, value), cx)?;
        let step = Step {
            at: value,
            value: true,
        };
        if visit(Event::Step(step), cx)? {
            return Ok((Walked::Stopped(step), read));
        }
        let (skipped, bytes) = tokens.skip(src, &mut reader, true, cx)?;
        read += bytes;
        if skipped.is_none() {
            return Ok((Walked::Broken(value, !tokened(src, value)), read));
        }
    }
}

/// `at` の字句を [`Tokens`] が読むか（文字列・配列・辞書）。読めなければ、[`Tokens`] が読んだ分を数えたので、
/// 終わりまで読みうるものとしない
fn tokened(src: &[u8], at: usize) -> bool {
    matches!(src.get(at), Some(b'(' | b'[')) || src.get(at..at + 2) == Some(b"<<")
}

/// hayro-syntax の辞書の表の、キーの 1 つのメモリ。キーと位置の組（48 バイトまで）と制御の 1 バイトを、表が大きく
/// なる途中で新旧の表が持つ分（キーの数の 3.5 倍まで）
const KEY_BYTES: u64 = 176;

/// 位置が、定義の `obj`（7.3.10）になりうるか。`endobj` などの語の一部は、定義にならない
fn definition(src: &[u8], keyword: usize) -> bool {
    src.get(keyword..keyword + 3) == Some(b"obj".as_slice())
        && !keyword
            .checked_sub(1)
            .and_then(|at| src.get(at))
            .is_some_and(u8::is_ascii_alphabetic)
}

/// フィルターか展開のパラメーターの値に参照があるときに、ファイルのすべての `obj` の後ろ（参照の先になりうる位置）を
/// 見る（[`targeted`]・[`listed`]）。[`objects`] と同じく、範囲の終わりの位置を 1 つ覚えて、位置ごとに 1 度だけ見る
fn targets(src: &[u8], seen: &mut Seen, cx: &mut Cx<'_>) -> Result<(), Stop> {
    let mut skip = Skip::default();
    let mut lists = Lists::default();
    let mut params = Lists::default();
    let mut last = None;
    for keyword in 0..src.len().saturating_sub(2) {
        if !definition(src, keyword) {
            continue;
        }
        let start = skip.over(src, keyword + 3, cx)?;
        if start == skip.to && last.replace(start) == Some(start) {
            continue;
        }
        targeted(src, start, start, &mut lists, seen, cx)?;
        if let Some(index) = listed(src, start, &mut params, cx)? {
            earliest(&mut seen.listed, index, start);
        }
    }
    Ok(())
}

/// 参照の先になりうる位置の値（[`targeted`]）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Hit {
    /// 当てるものがない
    Nothing,
    /// LZW・ASCII85 でない名前。ほかの配列の参照の項目を、名前に解きうる
    Name,
    /// LZW・ASCII85 の名前か、それを名前の項目に持つか、最初の参照の項目より前の名前の項目だけで [`FILTERS`] を超える
    /// 配列
    Coded,
    /// 参照の項目も数えると、名前と参照の項目が [`FILTERS`] を超える配列。hayro-syntax は、名前に解けない参照の項目で
    /// 止まるので、参照の先になりうる位置に名前があるときだけ当たる
    Crowded,
}

/// 参照の先になりうる位置（`obj` の後ろと、オブジェクトのストリームの中の位置。hayro-syntax の `IndirectObject::read`
/// と `ObjectStream::read_offsets` と同じく、空白とコメントを飛ばした位置）の値を見て、当たり（[`Hit`]）と配列の項目の
/// 数（[`Seen::leading`]・[`Seen::items`]）を記録する。hayro-syntax は、フィルターの値の参照を解いて名前か名前の配列を
/// 読み、配列の項目の参照も名前に解く（`FlexArrayIter::next`。参照の先の参照は読まない）。配列は、hayro-syntax と
/// 同じく、名前でも参照でもない最初の項目まで見る（参照の項目の後ろも見るので、多めに当てる）。読んだ名前の字と、
/// 項目の間の空白とコメントを `Work` に数える。
///
/// 配列は、名前と参照の項目を [`FILTERS`] を超えるまでしか読まない。コメントの行ごとの `obj [` は、どれも `[` の後ろの
/// 空白とコメントを飛ばした範囲の終わりの同じ位置に着き、同じ項目の並びを読むので、2 つ目からは読まない（[`Lists`]）
fn targeted(
    data: &[u8],
    at: usize,
    place: usize,
    lists: &mut Lists,
    seen: &mut Seen,
    cx: &mut Cx<'_>,
) -> Result<(), Stop> {
    if let Some((name, next)) = read_name(data, at, cx)? {
        cx.check(
            Kind::Work,
            lookup_work(data.get(at..next).unwrap_or_default()),
        )?;
        seen.hit(if coded(&name) { Hit::Coded } else { Hit::Name }, place);
        return Ok(());
    }
    if data.get(at) != Some(&b'[') {
        return Ok(());
    }
    let mut item = lists.skip.over(data, at + 1, cx)?;
    if item == lists.skip.to && lists.opened.replace(item) == Some(item) {
        return Ok(());
    }
    let mut count = 0u64;
    // 最初の参照の項目より前の名前の項目の数
    let mut leading = None;
    let hit = loop {
        let next = match read_name(data, item, cx)? {
            Some((name, next)) => {
                cx.check(
                    Kind::Work,
                    lookup_work(data.get(item..next).unwrap_or_default()),
                )?;
                if coded(&name) {
                    break Hit::Coded;
                }
                next
            }
            // 参照の項目は、名前に解ければ当てて、後ろの項目も読む
            None => match reference(data, item, cx)? {
                Some(end) => {
                    leading.get_or_insert(count);
                    end
                }
                None => break Hit::Nothing,
            },
        };
        count += 1;
        if count > FILTERS {
            break if leading.is_some() {
                Hit::Crowded
            } else {
                Hit::Coded
            };
        }
        item = lists.skip.over(data, next, cx)?;
    };
    seen.leading = seen.leading.max(leading.unwrap_or(count));
    seen.items = seen.items.max(count);
    seen.hit(hit, place);
    Ok(())
}

/// 位置の値が展開のパラメーター（`DecodeParms`・`DP`）の配列なら、hayro-syntax が同じ辞書の表を重ねて持ちうる、
/// いちばん前の項目の番号を返す。hayro-syntax は、配列の項目を、フィルターの名前の数（[`Seen::reads`]）だけ読み
/// （`iter::<Object>`）、参照の項目は、解くたびに辞書を読み直して表を作るので、同じ大きな辞書を指す参照を並べると、
/// その表を項目の数だけ持つ。そこで、上の段の 2 つ目の参照の項目を返す（参照が 1 つなら、表も 1 つ。辞書の中の参照は、
/// 項目の数だけの表を作らないので見ない）。
///
/// 項目は、hayro-syntax（`FlexArrayIter::next`）と同じく、参照（`ObjRef`）として読めなければ、ファイルの文脈の読み方
/// （`Object::read`）で区切る。読み方は、飛ばし方（`Skippable`。配列の終わりを決める）と、次の所で区切りが変わる。
/// - 数字のない数の字句: 読み方は、数字の前の符号と `.` までを読むことがある（`.-0 0 R` は、数 `.` と参照 `-0 0 R`）。
/// - 辞書の後ろ（空白とコメントを飛ばして）に `stream` がある項目: 読み方はストリームとして読み、中身を `Length` で
///   飛ばす。後ろの項目が分からないので、その番号を返す（ストリームは間接オブジェクトに限るので、ふつうの文書にはない。
///   7.3.8.1）。辞書は表を作らないように飛ばし方で読み、後ろの `stream` を見る（辞書の終わりは、どちらも同じ）。
///
/// 項目は [`FILTERS`] 個とその後ろの 2 つまで、バイト列は [`PARAMETER_BYTES`] まで見る。項目の数の上限まで読めば、
/// 上の項目のほかは返さない。バイト列の上限で切れたら、後ろの項目が分からないので、切れた所の 2 つ前の項目の番号を
/// 返す。上限の終わりにかかる参照（`5 0 R` の `R` が外）は、数 2 つに読むからで、後ろの 2 つは、[`FILTERS`] 個目
/// までの項目にかかる参照のために読む。読めずに切れた項目が `+` で始まれば、前の項目から始まる参照の世代番号の符号
/// （`4 +0 R` の `+`）のこともあるので、1 つ前の番号を返す。`[` の後ろから、ファイルかオブジェクトのストリームの中身の終わりまでが
/// 上限より長ければ（ふつうのファイルは当たる）、hayro-syntax も読めない項目（`5-5.5` など）も、切れた項目と見分け
/// ないので、同じく扱う（多めに断るだけ）。読んだバイト列を `Work` に数える（読めずに位置が戻った項目は、見たバイト
/// 列の終わりまで）。[`targeted`] と同じく、`[` の後ろの範囲の終わりの同じ位置からは、1 度だけ読む（[`Lists`]）
fn listed(data: &[u8], at: usize, lists: &mut Lists, cx: &mut Cx<'_>) -> Result<Option<u64>, Stop> {
    if data.get(at) != Some(&b'[') {
        return Ok(None);
    }
    let first = lists.skip.over(data, at + 1, cx)?;
    if first == lists.skip.to && lists.opened.replace(first) == Some(first) {
        return Ok(None);
    }
    let end = first.saturating_add(PARAMETER_BYTES).min(data.len());
    let cut = end < data.len();
    let mut reader = Reader::new(data.get(..end).unwrap_or(data));
    reader.jump(first);
    // ファイルの文脈で読む（hayro-syntax と同じく、入れ子の配列をファイルの飛ばし方で区切る）
    let context = ReaderContext::dummy();
    let mut references = 0;
    let mut reached = first;
    let mut layered = None;
    // 終わりにかかる参照を 2 つ前の番号にするため、[`FILTERS`] 個目の項目の後ろの 2 つまで読む（その番号の参照と
    // ストリームは、フィルターの数より後ろなので、結果を変えない）
    for index in 0..FILTERS + 2 {
        reader.skip_white_spaces_and_comments();
        if reader.peek_tag(b"]").is_some() {
            break;
        }
        if reader.at_end() {
            // 上限で切れた。終わりにかかる参照は、数 2 つに読む
            layered = cut.then(|| index.saturating_sub(2));
            break;
        }
        if reader.read_with_context::<ObjRef>(&context).is_some() {
            references += 1;
            if references >= 2 {
                layered = Some(index);
                break;
            }
            continue;
        }
        let dict = reader.peek_tag(b"<<").is_some();
        let read = if dict {
            reader.skip::<Object<'_>>(false).is_some()
        } else {
            reader.read_with_context::<Object<'_>>(&context).is_some()
        };
        if !read {
            // 上限で切れたか、hayro-syntax も項目を読めず、後ろを読まない。`+` で始まる切れた項目は、前の項目から
            // 始まる参照の世代番号のことがある（世代番号の字句のうち、数として読めないのは `+` だけ）。読めないと
            // 位置が戻るので、項目の初めを見る
            reached = end;
            let sign = data.get(reader.offset()) == Some(&b'+');
            layered = cut.then(|| if sign { index.saturating_sub(1) } else { index });
            break;
        }
        if dict {
            let mut after = reader.clone();
            after.skip_white_spaces_and_comments();
            reached = after.offset();
            if after.peek_tag(b"stream").is_some() {
                layered = Some(index);
                break;
            }
        }
    }
    let reached = reached.max(reader.offset());
    cx.check(
        Kind::Work,
        lookup_work(data.get(first..reached).unwrap_or_default()),
    )?;
    Ok(layered)
}

/// 表を重ねて持ちうる項目（[`listed`]）の、いちばん前の番号と場所を覚える
fn earliest(slot: &mut Option<(u64, usize)>, index: u64, place: usize) {
    if slot.is_none_or(|(least, _)| index < least) {
        *slot = Some((index, place));
    }
}

/// 配列の項目を飛ばす覚えと、`[` の後ろの範囲の終わりとして読んだ位置（[`targeted`]・[`listed`]）
#[derive(Debug, Default)]
struct Lists {
    skip: Skip,
    opened: Option<usize>,
}

/// オブジェクトのストリームの辞書の、長さ・数・展開の値（`Length`・`N`・`First`・`Filter`・`F`・`DecodeParms`・
/// `DP`）が参照を含むか。値は、hayro-syntax の飛ばし方でバイト列として読み（[`Raw`]。中の辞書の表を作らない）、
/// 参照の `R` になりうる字を探す（[`mentions`]）。飛ばせない値も、参照を含むものとして扱う
fn referenced(dict: &Dict<'_>, cx: &mut Cx<'_>) -> Result<bool, Stop> {
    for key in [
        b"Length".as_slice(),
        b"N",
        b"First",
        b"Filter",
        b"F",
        b"DecodeParms",
        b"DP",
    ] {
        if !dict.contains_key(key) {
            continue;
        }
        let Some(MaybeRef::NotRef(Raw(value))) = dict.get_raw::<Raw<'_>>(key) else {
            return Ok(true);
        };
        cx.check(Kind::Work, lookup_work(value))?;
        if mentions(value) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// 値のバイト列。hayro-syntax の `Skippable` で飛ばすので、辞書と配列の中を読んでも、辞書の表を作らない
struct Raw<'a>(&'a [u8]);

impl<'a> Readable<'a> for Raw<'a> {
    fn read(r: &mut Reader<'a>, _: &ReaderContext<'a>) -> Option<Self> {
        r.skip::<Object<'_>>(false).map(Raw)
    }
}

/// バイト列に、参照（7.3.10）の `R` になりうる字があるか。hayro-syntax は、参照を数・空白・数・空白・`R` と読み
/// （`ObjRef::read`）、名前の字が後ろに続く数は数として読まないので、`R` の前は空白か、符号か小数点だけの数の字
/// （`9 -R` など）になる。前が `/` か英字の `R` だけを除くので、ほかの名前の中の `R`（`/K_R` など）と、文字列と
/// コメントの中の `R` も、参照として扱う（多めに断るだけ）
fn mentions(value: &[u8]) -> bool {
    value.iter().enumerate().any(|(at, &byte)| {
        byte == b'R'
            && !at
                .checked_sub(1)
                .and_then(|before| value.get(before))
                .is_some_and(|&before| before == b'/' || before.is_ascii_alphabetic())
    })
}

/// 読まない文書の診断
pub(crate) fn refused(place: Ref) -> Diagnostic {
    Diagnostic::new(zumen_base::codes::PDF_DOCUMENT_UNSUPPORTED).at(place)
}

#[cfg(test)]
mod table_tests {
    use super::*;
    use zumen_base::{Budget, CancelToken, Diagnostics};

    #[test]
    fn counts_long_escaped_keys_without_materializing_them() {
        let mut budget = Budget::default();
        let mut diag = Diagnostics::new();
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        let raw = format!("/{}#G0#1 ", "#41".repeat(262_144));
        let mut reader = Reader::new(raw.as_bytes());
        let (word, cost) = scanned_name(raw.as_bytes(), &mut reader, &mut cx)
            .unwrap()
            .unwrap();
        assert!(word.is_empty());
        assert_eq!(cost, KEY_BYTES + 4 * (262_144 + 5));
        assert_eq!(reader.offset(), raw.len() - 1);
        // 空の名前、16 字の既知の名前、不正な # と区切りも上流と同じ範囲・所有の扱いになる。
        for raw in [
            b"/".as_slice(),
            b"/A}",
            b"/#42itsPerComponent]",
            b"/#23#G1#",
        ] {
            let mut upstream = Reader::new(raw);
            let full = upstream.read_without_context::<Name<'_>>().unwrap();
            let mut reader = Reader::new(raw);
            let (word, cost) = scanned_name(raw, &mut reader, &mut cx).unwrap().unwrap();
            assert_eq!(&*word, &*full);
            assert_eq!(reader.offset(), upstream.offset());
            let owned = position(raw, &full).is_none();
            assert_eq!(position(raw, &word), position(raw, &full));
            assert_eq!(
                cost,
                KEY_BYTES + if owned { 4 * full.len() as u64 } else { 0 }
            );
        }
    }

    #[test]
    fn bounds_separate_and_overlapping_object_stream_dictionaries() {
        let table = format!(
            "<< {}>>",
            (0..4000).map(|i| format!("/K{i} 1 ")).collect::<String>()
        );
        // 並んだ 2 辞書では最大の 1 表分で足りる。途中を指す項目は、外側の小さい表で見積もらない。
        for (body, offset, keys) in [
            (format!("{table} {table}"), table.len() + 1, 4000),
            (format!("<< /A {table} >>"), 6, 4001),
        ] {
            let header = format!("30 0 31 {offset} ");
            let data = format!("{header}{body}");
            let mut budget = Budget::default();
            let mut diag = Diagnostics::new();
            let cancel = CancelToken::new();
            let mut seen = Seen::default();
            let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
            assert_eq!(
                members(
                    data.as_bytes(),
                    2,
                    header.len() as i64,
                    0,
                    &mut seen,
                    &mut cx
                ),
                Ok(None)
            );
            assert_eq!(seen.table_bytes, keys * KEY_BYTES);
        }
    }
}

#[cfg(test)]
mod tests {
    use zumen_base::{Budget, CancelToken, Diagnostics};

    use super::*;

    /// `starts` の位置から順に、同じ覚えを使って [`Tokens::value`] で飛ばした終わりと、hayro-syntax で飛ばした終わり
    fn skips(src: &[u8], starts: &[(usize, usize, bool)]) -> Vec<(Option<usize>, Option<usize>)> {
        let mut budget = Budget::default();
        let mut diag = Diagnostics::new();
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        let mut tokens = Tokens::default();
        starts
            .iter()
            .map(|&(at, depth, reference)| {
                let mut reader = Reader::new(src);
                reader.jump(at);
                let mut read = 0;
                let ours = tokens
                    .value(src, &mut reader, depth, reference, &mut read, &mut cx)
                    .ok()
                    .flatten()
                    .map(|()| reader.offset());
                let mut reader = Reader::new(src);
                reader.jump(at);
                let theirs = if reference {
                    reader
                        .skip_with_depth::<MaybeRef<Object<'_>>>(false, depth)
                        .map(|_| reader.offset())
                } else {
                    reader
                        .skip_with_depth::<Object<'_>>(false, depth)
                        .map(|_| reader.offset())
                };
                (ours, theirs)
            })
            .collect()
    }

    /// 同じ項目の始まりに違う深さで着いても、深さの上限で読めるかが変わるので、覚えを深さで分ける。外すと、浅く読んだ
    /// 覚えを深く読むときに使い、hayro-syntax と違う終わりを返す（#752）
    #[test]
    fn memos_depend_on_depth() {
        // 行末の後ろに、505 段の配列を持つ配列。深さ 1 なら上限（512）の内、深さ 10 なら外
        let nested = format!("{}{}", "[".repeat(505), "]".repeat(505));
        let src = format!("[\n{nested}]");
        let results = skips(
            src.as_bytes(),
            &[(0, 1, false), (0, 10, false), (0, 1, false)],
        );
        for (ours, theirs) in &results {
            assert_eq!(ours, theirs);
        }
        assert_eq!(results[0].0, Some(src.len()));
        assert_eq!(results[1].0, None);
    }

    /// 同じ項目の始まりに、配列と辞書から着いても、項目の読み方（配列の項目か、辞書のキーと値か）が違うので、覚えを
    /// 種類で分ける（#752）
    #[test]
    fn memos_depend_on_the_kind_of_container() {
        // 配列の中のコメントの `<<` から読む辞書は、配列と同じ `/a` に着く。配列は閉じ、辞書は `]` で読めない
        let src = b"[ %<<\n/a 1 0 R]";
        let dict = 3;
        assert_eq!(&src[dict..dict + 2], b"<<");
        for order in [
            [(0, 1, false), (dict, 1, false)],
            [(dict, 1, false), (0, 1, false)],
        ] {
            let results = skips(src, &order);
            for (ours, theirs) in &results {
                assert_eq!(ours, theirs, "{order:?}");
            }
        }
        let results = skips(src, &[(0, 1, false), (dict, 1, false)]);
        assert_eq!(results[0].0, Some(src.len()));
        assert_eq!(results[1].0, None);
    }
}
