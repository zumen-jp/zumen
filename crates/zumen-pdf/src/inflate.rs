//! Flate・LZW と、それに ASCII を前置した系列の展開量を先に数える（決定 0266・0270・0273）。

mod ascii;
mod lzw;

use std::io::Read;

use flate2::read::{DeflateDecoder, ZlibDecoder};
use hayro_syntax::Filter;
use hayro_syntax::object::{Object, Stream};
use zumen_base::{Cx, Kind, Stop};

/// 出力を保持しない確かめの、固定長のバッファー。展開の状態と合わせて入力の大きさによらない
const CHUNK: usize = 8192;

/// RFC 1951 の 3.2.5。長さと距離の符号はそれぞれ 1 ビット以上、写す長さは 258 バイト以下。
/// hayro-syntax 0.9.0 の寛容な展開も、この上限を超える長さと 0 ビットの符号を読まないので、
/// 1 入力バイトが出すのは 8 ÷ 2 * 258 = 1,032 バイト以下。非圧縮のブロックとリテラルもこの範囲に収まる
const MAX_EXPANSION: u64 = 1032;

/// 単独 Flate・LZW と ASCII を 1 個前置した系列は、全出力の確保前に数え、`true` を返す。
/// hayro-syntax と同じ flate2 のリーダーを、同じ順（zlib、raw deflate）で試す。読めなければ、
/// 寛容な展開の出力の上限を先に数える。失敗した試みの分も戻さない。予測子の出力は、その入力より長くならない
pub(crate) fn before(stream: &Stream<'_>, cx: &mut Cx<'_>) -> Result<bool, Stop> {
    let filters = stream.filters();
    // hayro-syntax の `Stream::decoded_image` と同じく、先頭の Crypt はフィルターとして適用しない
    let effective = filters.strip_prefix(&[Filter::Crypt]).unwrap_or(&filters);
    match effective {
        [Filter::FlateDecode] => count(&stream.raw_data(), cx)?,
        [Filter::LzwDecode] => lzw::before(stream, cx)?,
        [
            Filter::AsciiHexDecode,
            next @ (Filter::FlateDecode | Filter::LzwDecode),
        ] => {
            return sequence(stream, ascii::Encoding::Hex, *next, cx);
        }
        [
            Filter::Ascii85Decode,
            next @ (Filter::FlateDecode | Filter::LzwDecode),
        ] => {
            return sequence(stream, ascii::Encoding::Base85, *next, cx);
        }
        _ => return Ok(false),
    }
    Ok(true)
}

fn sequence(
    stream: &Stream<'_>,
    ascii: ascii::Encoding,
    next: Filter,
    cx: &mut Cx<'_>,
) -> Result<bool, Stop> {
    // F は短縮した Filter のほかに外部ファイルを指す（7.3.8.2）。後者では filters() が
    // Filter の配列を返しても、hayro-syntax の decoded_image は適用前に失敗する。
    cx.check(Kind::Work, 1)?;
    if matches!(
        stream.dict().get::<Object<'_>>(b"F"),
        Some(Object::String(_) | Object::Dict(_))
    ) {
        return Ok(false);
    }
    // 中間の量を先に数え、予算内でだけ保持する。失敗までの確保は戻さず、後段へは進まない。
    let Some(data) = ascii::decode(&stream.raw_data(), ascii, cx)? else {
        return Ok(true);
    };
    if next == Filter::FlateDecode {
        count(&data, cx)?;
    } else {
        let early = lzw::early_change(stream, cx)?;
        lzw::count(&data, early, cx)?;
    }
    Ok(true)
}

fn count(data: &[u8], cx: &mut Cx<'_>) -> Result<(), Stop> {
    if strict(ZlibDecoder::new(data), data.len(), cx)?
        || strict(DeflateDecoder::new(data), data.len(), cx)?
    {
        return Ok(());
    }
    let bound = (data.len() as u64).saturating_mul(MAX_EXPANSION);
    cx.check(Kind::Work, bound.div_ceil(16))?;
    cx.check(Kind::DecodedBytes, bound)
}

/// 入力を読む量は先に、出力を作る手間は 1 回の読みの前に数える。読んだ出力は固定長のバッファーにしか持たず、
/// バイト数を数えてから次を読む。`read_to_end` と同じく、途中で切れていてもリーダーがエラーを返さなければ成功
fn strict(mut reader: impl Read, input: usize, cx: &mut Cx<'_>) -> Result<bool, Stop> {
    cx.check(Kind::Work, (input as u64).div_ceil(16))?;
    let mut buffer = [0; CHUNK];
    loop {
        cx.check(Kind::Work, 1 + CHUNK as u64 / 16)?;
        match reader.read(&mut buffer) {
            Ok(0) => return Ok(true),
            Ok(length) => cx.check(Kind::DecodedBytes, length as u64)?,
            Err(_) => return Ok(false),
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
                assert_eq!(budget.used(Kind::DecodedBytes), length as u64);
                let limits =
                    Limits::default().with(Kind::DecodedBytes, Limit::Finite(length as u64));
                assert_eq!(run(data, limits).0, Ok(()));
                if length != 0 {
                    let limits = Limits::default()
                        .with(Kind::DecodedBytes, Limit::Finite(length as u64 - 1));
                    assert!(matches!(run(data, limits).0,
                        Err(Stop::Exceeded(e)) if e.kind == Kind::DecodedBytes));
                }
            }
        }
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
            let (result, budget) = run(data, Limits::default());
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
