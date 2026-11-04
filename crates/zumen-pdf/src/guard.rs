//! hayro-syntax 0.9.0 がパニックするか、環境の大きさの整数で読んで結果が環境で変わる値を、読む前に確かめる
//! （決定 0007・0256）。
//!
//! hayro-syntax は、Flate と LZW の予測子（7.4.4.4）の行の長さを、`Columns` と `Colors` と `BitsPerComponent` の積で
//! 求め、確かめずに掛けて割る。積が 0 か、あふれて 0 になると、0 で割ってパニックし、あふれる値は 32 ビットと 64 ビットで
//! 違う。オブジェクトのストリーム（7.5.7）の `N`・`First` と中の位置、ICCBased の `N`、CCITT の `Columns` は、環境の
//! 大きさの整数（`usize`）で読むので、2 の 32 乗からの値は 32 ビットの環境で読めず、足すとあふれる。
//!
//! そこで、`Pdf::new` の前に、L1 のストリームのレコードの辞書を見て、こうした値を持つ文書は読まない（[`records`]）。
//! 値の参照は、L1 の同じ番号のすべての定義を見て、L1 にない番号（オブジェクトのストリームの中など）を指す文書は読まない。
//! 読み込んだ後に、hayro-syntax がオブジェクトのストリームとして使うストリームを展開し、中の位置と、中に書いた
//! ストリーム（7.5.7 に反するが、hayro-syntax は読む）を見る（[`object_streams`]）。中に書いたストリームの値は、L1 では
//! 見えないので、そうした文書も読まない。

use std::collections::{BTreeMap, BTreeSet};

use hayro_syntax::Pdf as Document;
use hayro_syntax::object::{Dict, MaybeRef, Object, ObjectIdentifier, Stream};
use hayro_syntax::reader::{Reader, ReaderContext, ReaderExt};
use hayro_syntax::xref::XRef;
use zumen_base::{Diagnostic, Kind, Ref, Stop};
use zumen_sdk::Cx;

use crate::walk::lookup_work;
use crate::{PdfFile, RecordKind};

/// 環境の大きさの整数で読む値の上限（含まない）。32 ビットの環境でも読め、2 つを足してもあふれない
const LIMIT: i64 = 1 << 31;

/// 予測子の `Columns` の上限。`Colors` と `BitsPerComponent` の積（255 まで）を掛けても、32 ビットであふれない
const COLUMNS: i64 = 1 << 24;

/// 確かめた結果
pub(crate) enum Verdict {
    /// 読んでよい。オブジェクトのストリームになりうるストリーム（`N` と `First` を持つ）の番号
    Fits(BTreeSet<(u32, i32)>),
    /// 読まない。理由の値を持つ L1 のレコード
    Refused(usize),
}

/// L1 のストリームのレコードの辞書を確かめる。どのレコードも 1 度ずつ読み、値の参照の先は、引くたびに読んで
/// `Work` に数える
pub(crate) fn records(file: &PdfFile, cx: &mut Cx<'_>) -> Result<Verdict, Stop> {
    let definitions = definitions(file);
    let values = Definitions {
        file,
        definitions: &definitions,
    };
    let mut containers = BTreeSet::new();
    for (index, record) in file.records().iter().enumerate() {
        if record.kind != RecordKind::Object {
            continue;
        }
        // どのレコードも 1 度だけ読むので、手間はファイルの大きさ（`InputBytes`）に比例する
        let bytes = file.bytes(record.start, record.end);
        if !bytes.windows(STREAM.len()).any(|window| window == STREAM) {
            continue;
        }
        let Some(Object::Stream(stream)) = body(bytes) else {
            continue;
        };
        if !stream_fits(stream.dict(), &values, cx)? {
            return Ok(Verdict::Refused(index));
        }
        let dict = stream.dict();
        if let Some((number, generation)) = record.object
            && dict.get_raw::<Object<'_>>(b"N").is_some()
            && dict.get_raw::<Object<'_>>(b"First").is_some()
        {
            containers.insert((number, i32::from(generation)));
        }
    }
    Ok(Verdict::Fits(containers))
}

/// hayro-syntax がオブジェクトのストリームとして使うストリーム（`containers` の番号で表が指すもの）を確かめる。
/// 表を作り直した後（[`crate::walk::settle`]）に呼ぶ。読まない文書なら、理由のストリームの番号を返す
pub(crate) fn object_streams(
    document: &Document,
    containers: &BTreeSet<(u32, i32)>,
    cx: &mut Cx<'_>,
) -> Result<Option<(u32, i32)>, Stop> {
    let xref = document.xref();
    for &(number, generation) in containers {
        cx.check(Kind::Work, 1)?;
        let Some(stream) = xref.get::<Stream<'_>>(ObjectIdentifier::new(number, generation)) else {
            continue;
        };
        // 表が指す定義は、L1 で確かめた定義と違いうる（作り直した表は、ファイルを走査して選ぶ）
        if !stream_fits(stream.dict(), &Resolved(xref), cx)? || !members_fit(&stream, cx)? {
            return Ok(Some((number, generation)));
        }
    }
    Ok(None)
}

/// ストリームの本体の前の語
const STREAM: &[u8] = b"stream";

/// オブジェクトのストリームの中の位置と、位置にあるもの。hayro-syntax の `ObjectStream::read_offsets` と同じく、
/// 番号と位置の組を `N` 個まで読み、前の位置より後ろで、展開したバイト列の中を指す位置だけを使う
fn members_fit(stream: &Stream<'_>, cx: &mut Cx<'_>) -> Result<bool, Stop> {
    let dict = stream.dict();
    let (Some(count), Some(first)) = (dict.get::<i64>(b"N"), dict.get::<i64>(b"First")) else {
        return Ok(true);
    };
    // 負の値は、どの環境でも読めず、中のオブジェクトを使わない
    if count < 0 || first < 0 {
        return Ok(true);
    }
    let Ok(data) = stream.decoded_data() else {
        return Ok(true);
    };
    cx.check(Kind::DecodedBytes, data.len() as u64)?;
    let context = ReaderContext::dummy();
    let mut header = Reader::new(&data);
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
            return Ok(false);
        }
        let offset = usize::try_from(first.saturating_add(relative)).unwrap_or(usize::MAX);
        if offset >= data.len() || last.is_some_and(|last| offset <= last) {
            continue;
        }
        let mut member = Reader::new(&data);
        member.jump(offset);
        member.skip_white_spaces_and_comments();
        let start = member.offset();
        last = Some(start);
        if member.peek_tag(b"<<").is_none() {
            continue;
        }
        let read = member.read_with_context::<Dict<'_>>(&context).is_some();
        cx.check(
            Kind::Work,
            lookup_work(data.get(start..member.offset()).unwrap_or_default()),
        )?;
        if read {
            member.skip_white_spaces_and_comments();
            if member.peek_tag(STREAM).is_some() {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

/// ストリームの辞書の値（`N`・`First`・展開のパラメーター）が、どの環境でも同じに読め、パニックしないか
fn stream_fits<'a>(
    dict: &Dict<'a>,
    values: &impl Values<'a>,
    cx: &mut Cx<'_>,
) -> Result<bool, Stop> {
    for key in [b"N".as_slice(), b"First"] {
        match values.of(dict, key, cx)? {
            Some(found) if found.iter().filter_map(integer).all(|value| value < LIMIT) => {}
            _ => return Ok(false),
        }
    }
    for key in [b"DecodeParms".as_slice(), b"DP"] {
        let Some(found) = values.of(dict, key, cx)? else {
            return Ok(false);
        };
        for params in found {
            let fits = match params {
                Object::Array(items) => {
                    let mut fits = true;
                    for item in items.raw_iter() {
                        cx.check(Kind::Work, 1)?;
                        let Some(found) = values.item(item, cx)? else {
                            return Ok(false);
                        };
                        for params in found {
                            fits &= params_fit(&params, values, cx)?;
                        }
                    }
                    fits
                }
                params => params_fit(&params, values, cx)?,
            };
            if !fits {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

/// 展開のパラメーターの辞書（7.4.4.4 の表 8、7.4.6 の表 11）。辞書でなければ、hayro-syntax は既定の値で展開する。
/// 予測子（`Predictor` が 2〜255）は、行の長さが 0 でなく、32 ビットであふれないか（hayro-syntax は `Colors` と
/// `BitsPerComponent` を u8 で読み、範囲の外は既定の値にし、`Columns` の負の値も既定の値にする）。`Columns` は、
/// 予測子によらず、32 ビットの環境で読めるか
fn params_fit<'a>(
    params: &Object<'a>,
    values: &impl Values<'a>,
    cx: &mut Cx<'_>,
) -> Result<bool, Stop> {
    let params = match params {
        Object::Dict(dict) => dict.clone(),
        Object::Stream(stream) => stream.dict().clone(),
        _ => return Ok(true),
    };
    let mut read = |key: &[u8], default: i64| -> Result<Option<Vec<i64>>, Stop> {
        Ok(values.of(&params, key, cx)?.map(|found| {
            let found: Vec<_> = found.iter().filter_map(integer).collect();
            if found.is_empty() {
                vec![default]
            } else {
                found
            }
        }))
    };
    let (Some(predictors), Some(colors), Some(bits), Some(columns)) = (
        read(b"Predictor", 1)?,
        read(b"Colors", 1)?,
        read(b"BitsPerComponent", 8)?,
        read(b"Columns", 1)?,
    ) else {
        return Ok(false);
    };
    if columns.iter().any(|&value| value >= LIMIT) {
        return Ok(false);
    }
    if !predictors.iter().any(|&value| (2..=255).contains(&value)) {
        return Ok(true);
    }
    let byte = |value: i64, default: i64| {
        if (0..=255).contains(&value) {
            value
        } else {
            default
        }
    };
    let colors: Vec<_> = colors.iter().map(|&value| byte(value, 1)).collect();
    let bits: Vec<_> = bits.iter().map(|&value| byte(value, 8)).collect();
    let columns: Vec<_> = columns
        .iter()
        .map(|&value| if value < 0 { 1 } else { value })
        .collect();
    let most = |values: &[i64]| values.iter().copied().max().unwrap_or_default();
    Ok(!colors.contains(&0)
        && !bits.contains(&0)
        && !columns.contains(&0)
        && most(&colors) * most(&bits) <= 255
        && most(&columns) <= COLUMNS)
}

/// 数の値。hayro-syntax と同じく、実数は切り捨てた整数にする
fn integer(value: &Object<'_>) -> Option<i64> {
    match value {
        Object::Number(number) => Some(number.as_i64()),
        _ => None,
    }
}

/// 値を引く方法
trait Values<'a> {
    /// 辞書の値の候補。値がなければ空、引けない参照なら `None`
    fn of(
        &self,
        dict: &Dict<'a>,
        key: &[u8],
        cx: &mut Cx<'_>,
    ) -> Result<Option<Vec<Object<'a>>>, Stop>;
    /// 配列の項目の候補
    fn item(
        &self,
        item: MaybeRef<Object<'a>>,
        cx: &mut Cx<'_>,
    ) -> Result<Option<Vec<Object<'a>>>, Stop>;
}

/// L1 のレコードから引く。参照は、同じ番号のすべての定義を候補にする
struct Definitions<'f> {
    file: &'f PdfFile,
    definitions: &'f BTreeMap<(u32, i32), Vec<usize>>,
}

impl<'f> Values<'f> for Definitions<'f> {
    fn of(
        &self,
        dict: &Dict<'f>,
        key: &[u8],
        cx: &mut Cx<'_>,
    ) -> Result<Option<Vec<Object<'f>>>, Stop> {
        match dict.get_raw::<Object<'f>>(key) {
            None => Ok(Some(Vec::new())),
            Some(item) => self.item(item, cx),
        }
    }

    fn item(
        &self,
        item: MaybeRef<Object<'f>>,
        cx: &mut Cx<'_>,
    ) -> Result<Option<Vec<Object<'f>>>, Stop> {
        let reference = match item {
            MaybeRef::NotRef(object) => return Ok(Some(vec![object])),
            MaybeRef::Ref(reference) => reference,
        };
        let id = ObjectIdentifier::from(reference);
        let Some(indices) = self.definitions.get(&(id.obj_number, id.gen_number)) else {
            return Ok(None);
        };
        let mut found = Vec::with_capacity(indices.len());
        for record in indices
            .iter()
            .filter_map(|&index| self.file.records().get(index))
        {
            let bytes = self.file.bytes(record.start, record.end);
            cx.check(Kind::Work, lookup_work(bytes))?;
            found.extend(body(bytes));
        }
        Ok(Some(found))
    }
}

/// hayro-syntax の表で引く（参照を解く）
struct Resolved<'a>(&'a XRef);

impl<'a> Values<'a> for Resolved<'a> {
    fn of(
        &self,
        dict: &Dict<'a>,
        key: &[u8],
        cx: &mut Cx<'_>,
    ) -> Result<Option<Vec<Object<'a>>>, Stop> {
        cx.check(Kind::Work, 1)?;
        Ok(Some(dict.get::<Object<'a>>(key).into_iter().collect()))
    }

    fn item(
        &self,
        item: MaybeRef<Object<'a>>,
        cx: &mut Cx<'_>,
    ) -> Result<Option<Vec<Object<'a>>>, Stop> {
        cx.check(Kind::Work, 1)?;
        Ok(Some(match item {
            MaybeRef::NotRef(object) => vec![object],
            MaybeRef::Ref(reference) => self
                .0
                .get::<Object<'a>>(reference.into())
                .into_iter()
                .collect(),
        }))
    }
}

/// 間接オブジェクトのレコードの本体（`N G obj` の後ろ）。参照は解かない
fn body(bytes: &[u8]) -> Option<Object<'_>> {
    let mut reader = Reader::new(bytes);
    reader.read_without_context::<ObjectIdentifier>()?;
    reader.skip_white_spaces_and_comments();
    reader.read_with_context::<Object<'_>>(&ReaderContext::dummy())
}

/// 番号と世代番号 → L1 のレコードの位置。同じ番号の定義をすべて持つ
fn definitions(file: &PdfFile) -> BTreeMap<(u32, i32), Vec<usize>> {
    let mut out: BTreeMap<_, Vec<_>> = BTreeMap::new();
    for (index, record) in file.records().iter().enumerate() {
        if let Some((number, generation)) = record.object {
            out.entry((number, i32::from(generation)))
                .or_default()
                .push(index);
        }
    }
    out
}

/// 読まない文書の診断
pub(crate) fn refused(place: Ref) -> Diagnostic {
    Diagnostic::new(zumen_base::codes::PDF_DOCUMENT_UNSUPPORTED).at(place)
}
