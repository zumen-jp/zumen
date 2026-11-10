//! Flate・LZW・RunLength と ASCII の系列の展開量を先に数える（決定 0266・0270・0273）。

mod allocation;
mod ascii;
mod chain;
mod lzw;
mod metadata;
mod run_length;
mod source;
mod stages;

pub(crate) use source::Source;

use std::io::Read;

use flate2::read::{DeflateDecoder, ZlibDecoder};
use hayro_syntax::Filter;
use hayro_syntax::object::Stream;
use stages::Stage;
use zumen_base::{Cx, Kind, Stop};

/// 出力を保持しない確かめの、固定長のバッファー。展開の状態と合わせて入力の大きさによらない
const CHUNK: usize = 8192;

/// RFC 1951 の 3.2.5。長さと距離の符号はそれぞれ 1 ビット以上、写す長さは 258 バイト以下。
/// hayro-syntax 0.9.0 の寛容な展開も、この上限を超える長さと 0 ビットの符号を読まないので、
/// 1 入力バイトが出すのは 8 ÷ 2 * 258 = 1,032 バイト以下。非圧縮のブロックとリテラルもこの範囲に収まる
const MAX_EXPANSION: u64 = 1032;

/// 単独 Flate・LZW・RunLength と ASCII を 1 個前置した系列は、全出力の確保前に数え、`true` を返す。
/// hayro-syntax と同じ flate2 のリーダーを、同じ順（zlib、raw deflate）で試す。読めなければ、
/// 寛容な展開の出力の上限を先に数える。失敗した試みの分も戻さない。予測子の出力は、その入力より長くならない。
///
/// その後ろに別のフィルターが続く系列（`[/RL /AHx]`・`[/AHx /Fl /Crypt]` など）も、各段を確保前に数える。
/// hayro-syntax 0.9.0 はフィルターを順に当て、前の段の出力の全量を確保してから次を当てるので、後ろの段が失敗しても
/// 縮めても、前の段の確保は起きる。中間出力は予算内でだけ保持し、次の段は予測子を当てた後のバイト列から数える。
/// ただし、系列のどこかに画像のフィルターがあるか、`F` が外部ファイルを指せば、どの段も当てずに失敗するので数えない。
pub(crate) fn before(
    stream: &Stream<'_>,
    source: Source<'_>,
    cx: &mut Cx<'_>,
) -> Result<bool, Stop> {
    // filters() も全段の引数を解析してから捨てる。同じ解析を 1 回だけ行い、
    // 単独の圧縮でも、元の位置と参照の文脈を保った引数を使う。
    let stages = stages::read(stream.dict(), metadata::Metadata::decoding(source), cx)?;
    // hayro-syntax 0.9.0 の `Stream::decoded_data` は、どこかに画像のフィルターがあれば、どの段も当てずに失敗する
    if stages.iter().any(|stage| {
        matches!(
            stage.filter,
            Filter::DctDecode | Filter::JpxDecode | Filter::Jbig2Decode | Filter::CcittFaxDecode
        )
    }) {
        return Ok(false);
    }
    // hayro-syntax の `Stream::decoded_image` と同じく、先頭の Crypt はフィルターとして適用しない
    let effective = match stages.as_slice() {
        [
            Stage {
                filter: Filter::Crypt,
                ..
            },
            rest @ ..,
        ] => rest,
        other => other,
    };
    // 単独の段と ASCII 1 段の前置は、出力を保持せずに数え、容量の伸長も予約する。
    // それ以外の系列は、中間出力を予算内で保持して次の段を確かめる（#769）。
    if effective.len() > 1
        && !matches!(
            effective,
            [
                Stage {
                    filter: Filter::AsciiHexDecode | Filter::Ascii85Decode,
                    ..
                },
                Stage {
                    filter: Filter::FlateDecode | Filter::LzwDecode | Filter::RunLengthDecode,
                    ..
                }
            ]
        )
    {
        return chain::before(stream, effective, source, cx);
    }
    let (ascii, stage) = match effective {
        [stage]
            if matches!(
                stage.filter,
                Filter::FlateDecode | Filter::LzwDecode | Filter::RunLengthDecode
            ) =>
        {
            (None, *stage)
        }
        [first, next] if first.filter == Filter::AsciiHexDecode => {
            (Some(ascii::Encoding::Hex), *next)
        }
        [first, next] if first.filter == Filter::Ascii85Decode => {
            (Some(ascii::Encoding::Base85), *next)
        }
        _ => return Ok(false),
    };
    match ascii {
        Some(encoding) => sequence(stream, encoding, stage, source, cx),
        None => {
            // 単独の段は、これまでどおり `F` を見分けずに数える。
            compressed(&stream.raw_data(), stage, cx)?;
            Ok(true)
        }
    }
}

fn sequence(
    stream: &Stream<'_>,
    ascii: ascii::Encoding,
    next: Stage,
    source: Source<'_>,
    cx: &mut Cx<'_>,
) -> Result<bool, Stop> {
    // F は短縮した Filter のほかに外部ファイルを指す（7.3.8.2）。後者では filters() が
    // Filter の配列を返しても、hayro-syntax の decoded_image は適用前に失敗する。
    if source.external(stream.dict(), cx)? {
        return Ok(false);
    }
    source.reserve_decryption(stream, cx)?;
    // 中間の量を先に数え、予算内でだけ保持する。失敗までの確保は戻さず、後段へは進まない。
    let Some(data) = ascii::decode(&stream.raw_data(), ascii, cx)? else {
        return Ok(true);
    };
    compressed(&data, next, cx)?;
    Ok(true)
}

fn compressed(data: &[u8], stage: Stage, cx: &mut Cx<'_>) -> Result<(), Stop> {
    let (bound, growth) = match stage.filter {
        Filter::FlateDecode => {
            let before = cx.budget.used(Kind::DecodedBytes);
            let fallback = count_fallback(data, cx)?;
            // 容量を予約する前の量。失敗した試みを含み、予測子への入力以上になる。
            let bound = cx.budget.used(Kind::DecodedBytes) - before;
            (bound, allocation::flate(data.len(), bound, fallback))
        }
        Filter::LzwDecode => {
            // 辞書の確保量を混ぜず、実際に出すバイト数から予測子を見積もる。
            let bound = lzw::count_output(data, stage.early, cx)?;
            (bound, allocation::lzw(bound))
        }
        _ => {
            run_length::count(data, cx)?;
            return Ok(());
        }
    };
    allocation::reserve(growth, cx)?;
    allocation::reserve(stage.predictor_cost(bound), cx)
}

#[cfg(test)]
fn count(data: &[u8], cx: &mut Cx<'_>) -> Result<(), Stop> {
    compressed(data, Stage::new(Filter::FlateDecode, None), cx)
}

/// 壊れた内容の診断で読む引数も、展開の予約とは別に数える。
/// 既存の診断は最初の引数の i64 の Predictor を見るため、u8 の Stage の値で代えない。
pub(crate) fn plain_flate(
    stream: &Stream<'_>,
    source: Source<'_>,
    cx: &mut Cx<'_>,
) -> Result<bool, Stop> {
    use hayro_syntax::object::Object;
    let dict = stream.dict();
    let metadata = metadata::Metadata::new(source, 1);
    let name = match metadata.name(dict, b"F", cx)? {
        Some(name) => Some(name),
        None => metadata.name(dict, b"Filter", cx)?,
    };
    let params = if name.is_some() {
        match metadata.dict(dict, b"DP", cx)? {
            Some(params) => Some(params),
            None => metadata.dict(dict, b"DecodeParms", cx)?,
        }
    } else {
        let array = match metadata.array(dict, b"DP", cx)? {
            Some(params) => Some(params),
            None => metadata.array(dict, b"DecodeParms", cx)?,
        };
        if let Some(array) = &array {
            metadata.objects(array, 1, cx)?;
        }
        array
            .and_then(|array| array.iter::<Object<'_>>().next())
            .and_then(Object::into_dict)
    };
    if params
        .and_then(|params| params.get::<i64>(b"Predictor"))
        .is_some_and(|p| p > 1)
    {
        return Ok(false);
    }
    // raw_data が Crypt Name を読む分も含める。暗号化していなければ追加の解析はない。
    let copies = if source.metadata_encrypted() { 2 } else { 1 };
    let stages = stages::read(dict, metadata::Metadata::new(source, copies), cx)?;
    Ok(matches!(
        stages.as_slice(),
        [Stage {
            filter: Filter::FlateDecode,
            ..
        }]
    ))
}

/// 伸長の予約前の出力量を数え、寛容な展開が必要かも返す。系列は、この量を次の段にも使う。
fn count_fallback(data: &[u8], cx: &mut Cx<'_>) -> Result<bool, Stop> {
    if strict(
        ZlibDecoder::new(data),
        ZlibDecoder::total_out,
        data.len(),
        cx,
    )? || strict(
        DeflateDecoder::new(data),
        DeflateDecoder::total_out,
        data.len(),
        cx,
    )? {
        return Ok(false);
    }
    let bound = (data.len() as u64).saturating_mul(MAX_EXPANSION);
    cx.check(Kind::Work, bound.div_ceil(16))?;
    cx.check(Kind::DecodedBytes, bound)?;
    Ok(true)
}

/// 入力を読む量は先に、出力を作る手間は 1 回の読みの前に数える。読んだ出力は固定長のバッファーにしか持たず、
/// バイト数を数えてから次を読む。`read_to_end` と同じく、途中で切れていてもリーダーがエラーを返さなければ成功
fn strict<R: Read>(
    mut reader: R,
    total_out: impl Fn(&R) -> u64,
    input: usize,
    cx: &mut Cx<'_>,
) -> Result<bool, Stop> {
    cx.check(Kind::Work, (input as u64).div_ceil(16))?;
    let mut buffer = [0; CHUNK];
    let mut counted = 0u64;
    loop {
        cx.check(Kind::Work, 1 + CHUNK as u64 / 16)?;
        match reader.read(&mut buffer) {
            Ok(0) => return Ok(true),
            Ok(length) => {
                cx.check(Kind::DecodedBytes, length as u64)?;
                counted = counted.saturating_add(length as u64);
            }
            Err(_) => {
                // flate2 は、出力を書いた後でも Read に Err を返す。read_to_end の小刻みな
                // 読みでは保持済みになる分を、公開の総出力量から数える。上流と同じ backend を使う。
                cx.check(
                    Kind::DecodedBytes,
                    total_out(&reader).saturating_sub(counted),
                )?;
                return Ok(false);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zumen_base::{Budget, CancelToken, Diagnostics, Limit, Limits};

    fn encoded(data: &[u8]) -> Vec<u8> {
        let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::best());
        encoder.write_all(data).unwrap();
        encoder.finish().unwrap()
    }

    fn run(data: &[u8], limits: Limits) -> (Result<(), Stop>, Budget) {
        let mut budget = Budget::new(limits);
        let mut diagnostics = Diagnostics::new();
        let cancel = CancelToken::new();
        let result = count(data, &mut Cx::new(&mut budget, &mut diagnostics, &cancel));
        (result, budget)
    }

    #[test]
    fn stops_small_compressed_inputs_without_retaining_the_output() {
        let data = encoded(&vec![b' '; 2 * 1024 * 1024]);
        assert!(data.len() < 4096);
        let limits = Limits::default().with(Kind::DecodedBytes, Limit::Finite(32 * 1024));
        let (result, budget) = run(&data, limits);
        assert!(matches!(result, Err(Stop::Exceeded(e)) if e.kind == Kind::DecodedBytes));
        assert_eq!(budget.used(Kind::DecodedBytes), 32 * 1024);
        assert_eq!(
            budget.exceeded().unwrap().requested,
            32 * 1024 + CHUNK as u64
        );
        assert_eq!((result, budget), run(&data, limits));
    }

    #[test]
    fn counts_exact_boundaries_and_raw_deflate() {
        for length in [0, 1, CHUNK - 1, CHUNK, CHUNK + 1, CHUNK * 2] {
            let encoded = encoded(&vec![b' '; length]);
            for data in [&encoded[..], &encoded[2..encoded.len() - 4]] {
                let (result, budget) = run(data, Limits::default());
                assert_eq!(result, Ok(()));
                let used = budget.used(Kind::DecodedBytes);
                assert!(used > length as u64);
                let limits = Limits::default().with(Kind::DecodedBytes, Limit::Finite(used));
                assert_eq!(run(data, limits).0, Ok(()));
                if used != 0 {
                    let limits =
                        Limits::default().with(Kind::DecodedBytes, Limit::Finite(used - 1));
                    assert!(matches!(run(data, limits).0,
                        Err(Stop::Exceeded(e)) if e.kind == Kind::DecodedBytes));
                }
            }
        }
    }

    #[test]
    fn failed_reads_count_bytes_kept_by_smaller_read_to_end_calls() {
        let mut saw_partial = false;
        for length in [35usize, 97, CHUNK, CHUNK + 1, CHUNK * 2, CHUNK * 8] {
            let mut data = encoded(&vec![b'A'; length]);
            *data.last_mut().unwrap() ^= 1;
            let mut reader = ZlibDecoder::new(data.as_slice());
            let mut retained = Vec::new();
            assert!(reader.read_to_end(&mut retained).is_err());
            let mut large_reads = ZlibDecoder::new(data.as_slice());
            let mut buffer = [0; CHUNK];
            let mut reported = 0;
            while let Ok(read) = large_reads.read(&mut buffer) {
                if read == 0 {
                    break;
                }
                reported += read;
            }
            saw_partial |= retained.len() > reported;
            let mut budget = Budget::default();
            let mut diagnostics = Diagnostics::new();
            let cancel = CancelToken::new();
            assert_eq!(
                strict(
                    ZlibDecoder::new(data.as_slice()),
                    ZlibDecoder::total_out,
                    data.len(),
                    &mut Cx::new(&mut budget, &mut diagnostics, &cancel),
                ),
                Ok(false),
            );
            assert!(budget.used(Kind::DecodedBytes) >= retained.len() as u64);
            let limit = budget.used(Kind::DecodedBytes);
            if limit > 0 {
                let mut small = Budget::new(
                    Limits::default().with(Kind::DecodedBytes, Limit::Finite(limit - 1)),
                );
                assert!(matches!(
                    strict(
                        ZlibDecoder::new(data.as_slice()),
                        ZlibDecoder::total_out,
                        data.len(),
                        &mut Cx::new(&mut small, &mut diagnostics, &cancel),
                    ),
                    Err(Stop::Exceeded(e)) if e.kind == Kind::DecodedBytes
                ));
            }
        }
        assert!(saw_partial);
    }

    #[test]
    fn reserves_the_fallback_before_it_runs() {
        let mut data = encoded(&vec![b' '; CHUNK]);
        *data.last_mut().unwrap() ^= 0xff;
        let (result, budget) = run(&data, Limits::default());
        assert_eq!(result, Ok(()));
        let bound = data.len() as u64 * MAX_EXPANSION;
        assert!(budget.used(Kind::DecodedBytes) >= bound);
        let limits = Limits::default().with(Kind::DecodedBytes, Limit::Finite(bound - 1));
        assert!(matches!(run(&data, limits).0,
            Err(Stop::Exceeded(e)) if e.kind == Kind::DecodedBytes));
    }

    #[test]
    fn bounds_hayro_outputs_of_damaged_streams() {
        use hayro_syntax::reader::{Reader, ReaderContext, ReaderExt};

        let check = |data: &[u8]| {
            let mut source = format!(
                "<< /Filter /FlateDecode /Length {} >>\nstream\n",
                data.len()
            )
            .into_bytes();
            source.extend(data);
            source.extend(b"\nendstream");
            let stream = Reader::new(&source)
                .read_with_context::<Stream<'_>>(&ReaderContext::dummy())
                .unwrap();
            // 確保上限を比べる検査では、損傷時の内部表の保守的な予約も通す。
            let limits = Limits::default()
                .with(Kind::Work, Limit::Finite(u64::MAX))
                .with(Kind::DecodedBytes, Limit::Finite(u64::MAX));
            let (result, budget) = run(data, limits);
            assert_eq!(result, Ok(()));
            let actual = stream.decoded_data().unwrap().len() as u64;
            assert!(
                actual <= budget.used(Kind::DecodedBytes),
                "{data:?}: {actual} > {}",
                budget.used(Kind::DecodedBytes)
            );
        };
        // 小さな入力の切断とビットの変更で、厳しい展開と寛容な展開の両方を通す
        for plain in [vec![0; 512], (0..=255).collect(), b"30 0 null ".repeat(8)] {
            let data = encoded(&plain);
            for end in 0..=data.len() {
                check(&data[..end]);
            }
            for index in 0..data.len() {
                for bit in 0..8 {
                    let mut damaged = data.clone();
                    damaged[index] ^= 1 << bit;
                    check(&damaged);
                }
            }
        }
    }

    #[test]
    fn stops_for_work_and_cancellation() {
        let data = encoded(&vec![b' '; 2 * CHUNK]);
        let limits = Limits::default().with(Kind::Work, Limit::Finite(0));
        let (result, budget) = run(&data, limits);
        assert!(matches!(result, Err(Stop::Exceeded(e)) if e.kind == Kind::Work));
        assert_eq!(budget.used(Kind::DecodedBytes), 0);
        let work = (data.len() as u64).div_ceil(16) + 1 + CHUNK as u64 / 16;
        let limits = Limits::default().with(Kind::Work, Limit::Finite(work));
        let (result, budget) = run(&data, limits);
        assert!(matches!(result, Err(Stop::Exceeded(e)) if e.kind == Kind::Work));
        assert_eq!(budget.used(Kind::DecodedBytes), CHUNK as u64);
        let mut budget = Budget::default();
        let mut diagnostics = Diagnostics::new();
        let cancel = CancelToken::new();
        cancel.cancel();
        assert_eq!(
            count(&data, &mut Cx::new(&mut budget, &mut diagnostics, &cancel)),
            Err(Stop::Cancelled)
        );
        assert_eq!(budget.used(Kind::DecodedBytes), 0);
    }
}
