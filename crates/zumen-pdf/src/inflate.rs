//! Flate・LZW・RunLength と ASCII の系列の展開量を先に数える（決定 0266・0270・0273）。

mod allocation;
mod ascii;
mod chain;
mod lzw;
mod metadata;
mod run_length;
mod source;
mod stages;

pub(crate) use allocation::{FLATE_DECODER_BYTES, FLATE_DECODER_WORK};
pub(crate) use source::Source;

use std::io::Read;

use flate2::bufread::{DeflateDecoder, ZlibDecoder};
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
    // 画像のときは decoded_data が F・raw_data に進まない。それ以外は、単独圧縮や
    // 未対応の段でも decoded_image が F を Object として読むので先に予約する。
    let external = source.external(stream.dict(), cx)?;
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
        if external {
            return Ok(false);
        }
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
        _ => {
            if !external {
                source.reserve_raw(stream, 1, cx)?;
            }
            return Ok(false);
        }
    };
    match ascii {
        Some(_) if external => Ok(false),
        Some(encoding) => sequence(stream, encoding, stage, source, cx),
        None => {
            // 外部 F の単独圧縮も、従来どおり確認を 1 回行う。通常は本来の展開も合わせて 2 回。
            source.reserve_raw(stream, 1 + u64::from(!external), cx)?;
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
    source.reserve_raw(stream, 2, cx)?;
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
            let (bound, strict_attempts, fallback) = count_fallback(data, cx)?;
            allocation::reserve(allocation::flate_readers(strict_attempts), cx)?;
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

/// 伸長の予約前の出力量、厳しい展開の試行回数（1 または 2）、寛容な展開が必要かを返す。
/// 出力量には固定のデコーダーの予約量を混ぜず、予測子や系列の次の段にも使う。
fn count_fallback(data: &[u8], cx: &mut Cx<'_>) -> Result<(u64, u64, bool), Stop> {
    let (zlib_ok, zlib_out) = strict(
        || ZlibDecoder::new(data),
        ZlibDecoder::total_out,
        data.len(),
        cx,
    )?;
    if zlib_ok {
        return Ok((zlib_out, 1, false));
    }
    let (raw_ok, raw_out) = strict(
        || DeflateDecoder::new(data),
        DeflateDecoder::total_out,
        data.len(),
        cx,
    )?;
    let produced = zlib_out.saturating_add(raw_out);
    if raw_ok {
        return Ok((produced, 2, false));
    }
    let bound = (data.len() as u64).saturating_mul(MAX_EXPANSION);
    cx.check(Kind::Work, bound.div_ceil(16))?;
    cx.check(Kind::DecodedBytes, bound)?;
    Ok((produced.saturating_add(bound), 2, true))
}

/// 固定のデコーダーの確保・初期化の手間と入力を読む量は、リーダーを作る前に数える。
/// 出力を作る手間は 1 回の読みの前に数え、読んだ出力は固定長のスタック上のバッファーにしか持たず、
/// バイト数を数えてから次を読む。`read_to_end` と同じく、途中で切れていてもリーダーがエラーを返さなければ成功。
fn strict<R: Read>(
    make: impl FnOnce() -> R,
    total_out: impl Fn(&R) -> u64,
    input: usize,
    cx: &mut Cx<'_>,
) -> Result<(bool, u64), Stop> {
    cx.check(
        Kind::Work,
        FLATE_DECODER_WORK.saturating_add((input as u64).div_ceil(16)),
    )?;
    cx.check(Kind::DecodedBytes, FLATE_DECODER_BYTES)?;
    let mut reader = make();
    let mut buffer = [0; CHUNK];
    let mut counted = 0u64;
    loop {
        cx.check(Kind::Work, 1 + CHUNK as u64 / 16)?;
        match reader.read(&mut buffer) {
            Ok(0) => return Ok((true, counted)),
            Ok(length) => {
                cx.check(Kind::DecodedBytes, length as u64)?;
                counted = counted.saturating_add(length as u64);
            }
            Err(_) => {
                // flate2 は、出力を書いた後でも Read に Err を返す。read_to_end の小刻みな
                // 読みでは保持済みになる分を、公開の総出力量から数える。上流と同じ backend を使う。
                let extra = total_out(&reader).saturating_sub(counted);
                cx.check(Kind::DecodedBytes, extra)?;
                return Ok((false, counted.saturating_add(extra)));
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
        let before_init =
            Limits::default().with(Kind::DecodedBytes, Limit::Finite(FLATE_DECODER_BYTES - 1));
        let (result, budget) = run(&data, before_init);
        assert!(matches!(result, Err(Stop::Exceeded(e)) if e.kind == Kind::DecodedBytes));
        assert_eq!(budget.used(Kind::DecodedBytes), 0);
        assert_eq!(budget.exceeded().unwrap().requested, FLATE_DECODER_BYTES);

        let mid_stream = FLATE_DECODER_BYTES + 32 * 1024;
        let limits = Limits::default().with(Kind::DecodedBytes, Limit::Finite(mid_stream));
        let (result, budget) = run(&data, limits);
        assert!(matches!(result, Err(Stop::Exceeded(e)) if e.kind == Kind::DecodedBytes));
        assert_eq!(budget.used(Kind::DecodedBytes), mid_stream);
        assert_eq!(
            budget.exceeded().unwrap().requested,
            mid_stream + CHUNK as u64
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
            let result = strict(
                || ZlibDecoder::new(data.as_slice()),
                ZlibDecoder::total_out,
                data.len(),
                &mut Cx::new(&mut budget, &mut diagnostics, &cancel),
            );
            assert!(matches!(result, Ok((false, out)) if out >= retained.len() as u64));
            assert!(budget.used(Kind::DecodedBytes) >= FLATE_DECODER_BYTES + retained.len() as u64);
            let limit = budget.used(Kind::DecodedBytes);
            if limit > 0 {
                let mut small = Budget::new(
                    Limits::default().with(Kind::DecodedBytes, Limit::Finite(limit - 1)),
                );
                assert!(matches!(
                    strict(
                        || ZlibDecoder::new(data.as_slice()),
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
        let work = FLATE_DECODER_WORK + (data.len() as u64).div_ceil(16) + 1 + CHUNK as u64 / 16;
        let limits = Limits::default().with(Kind::Work, Limit::Finite(work));
        let (result, budget) = run(&data, limits);
        assert!(matches!(result, Err(Stop::Exceeded(e)) if e.kind == Kind::Work));
        assert_eq!(
            budget.used(Kind::DecodedBytes),
            FLATE_DECODER_BYTES + CHUNK as u64
        );
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

    #[test]
    fn cumulative_non_refunded_allocations_across_preflight_decoded_data_and_reinflate_stay_within_reservation()
     {
        use hayro_syntax::Pdf as HayroPdf;
        use hayro_syntax::object::ObjectIdentifier;
        use std::collections::BTreeSet;
        use std::sync::Arc;

        fn deflate_encoded(data: &[u8]) -> Vec<u8> {
            let mut encoder =
                flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::best());
            encoder.write_all(data).unwrap();
            encoder.finish().unwrap()
        }

        fn hex_encoded(data: &[u8]) -> Vec<u8> {
            let mut out: Vec<u8> = data
                .iter()
                .flat_map(|b| format!("{b:02x}").into_bytes())
                .collect();
            out.push(b'>');
            out
        }

        fn a85_encoded(data: &[u8]) -> Vec<u8> {
            let mut out = Vec::new();
            for chunk in data.chunks(4) {
                let mut padded = [0u8; 4];
                padded[..chunk.len()].copy_from_slice(chunk);
                let mut value = u32::from_be_bytes(padded);
                let mut digits = [0u8; 5];
                for slot in digits.iter_mut().rev() {
                    *slot = (value % 85) as u8 + b'!';
                    value /= 85;
                }
                out.extend_from_slice(&digits[..chunk.len() + 1]);
            }
            out.extend_from_slice(b"~>");
            out
        }

        fn rle_encoded(data: &[u8]) -> Vec<u8> {
            let mut out = Vec::new();
            for chunk in data.chunks(128) {
                out.push((chunk.len() - 1) as u8);
                out.extend_from_slice(chunk);
            }
            out.push(128);
            out
        }

        fn unhex(text: &str) -> Vec<u8> {
            text.as_bytes()
                .as_chunks::<2>()
                .0
                .iter()
                .map(|s| u8::from_str_radix(core::str::from_utf8(s).unwrap(), 16).unwrap())
                .collect()
        }

        fn build_pdf(
            stream_dict: &str,
            stored: &[u8],
            encrypt_obj: Option<&str>,
            form: bool,
        ) -> Vec<u8> {
            let mut objects: Vec<Vec<u8>> = Vec::new();
            objects.push(b"<< /Type /Catalog /Pages 2 0 R >>".to_vec());
            objects.push(b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec());
            if form {
                objects.push(
                    b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] /Resources << /XObject << /Fm 4 0 R >> >> /Contents 6 0 R >>".to_vec(),
                );
            } else {
                objects.push(
                    b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] /Contents 4 0 R >>"
                        .to_vec(),
                );
            }
            let kind = if form {
                "/Type /XObject /Subtype /Form /BBox [0 0 100 100]"
            } else {
                ""
            };
            let mut stm = format!(
                "<< {kind} {stream_dict} /Length {} >>\nstream\n",
                stored.len()
            )
            .into_bytes();
            stm.extend_from_slice(stored);
            stm.extend_from_slice(b"\nendstream");
            objects.push(stm);
            objects.push(encrypt_obj.unwrap_or("null").as_bytes().to_vec());
            if form {
                let ops = b"/Fm Do\n";
                let mut c = format!("<< /Length {} >>\nstream\n", ops.len()).into_bytes();
                c.extend_from_slice(ops);
                c.extend_from_slice(b"\nendstream");
                objects.push(c);
            }

            let mut pdf = b"%PDF-1.7\n".to_vec();
            let mut offsets = vec![0usize];
            for (idx, body) in objects.iter().enumerate() {
                offsets.push(pdf.len());
                pdf.extend_from_slice(format!("{} 0 obj\n", idx + 1).as_bytes());
                pdf.extend_from_slice(body);
                pdf.extend_from_slice(b"\nendobj\n");
            }
            let xref_pos = pdf.len();
            pdf.extend_from_slice(format!("xref\n0 {}\n", offsets.len()).as_bytes());
            pdf.extend_from_slice(b"0000000000 65535 f \n");
            for off in &offsets[1..] {
                pdf.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
            }
            let enc = if encrypt_obj.is_some() {
                "/Encrypt 5 0 R /ID [<30303030303030303030303030303030> <30303030303030303030303030303030>]"
            } else {
                ""
            };
            pdf.extend_from_slice(
                format!(
                    "trailer\n<< /Size {} /Root 1 0 R {enc} >>\nstartxref\n{xref_pos}\n%%EOF\n",
                    offsets.len()
                )
                .as_bytes(),
            );
            pdf
        }

        let line = b"0 0 m 1 1 l S\n";
        let good_zlib = encoded(line);
        let good_raw = deflate_encoded(line);
        let truncated_zlib = good_zlib[..good_zlib.len() - 5].to_vec();
        let mut bad_adler = good_zlib.clone();
        *bad_adler.last_mut().unwrap() ^= 1;
        let mut corrupt = good_zlib.clone();
        corrupt[0] = 0xff;
        corrupt[1] = 0xff;

        let mut cases: Vec<(&str, Vec<u8>, bool, bool)> = vec![
            (
                "valid_zlib_content",
                build_pdf("/Filter /FlateDecode", &good_zlib, None, false),
                false,
                true,
            ),
            (
                "valid_raw_content",
                build_pdf("/Filter /FlateDecode", &good_raw, None, false),
                false,
                true,
            ),
            (
                "truncated_zlib_content",
                build_pdf("/Filter /FlateDecode", &truncated_zlib, None, false),
                false,
                true,
            ),
            (
                "bad_adler_content",
                build_pdf("/Filter /FlateDecode", &bad_adler, None, false),
                false,
                true,
            ),
            (
                "corrupt_fallback_content",
                build_pdf("/Filter /FlateDecode", &corrupt, None, false),
                false,
                true,
            ),
            (
                "whitespace_empty_content",
                build_pdf("/Filter /FlateDecode", b" \t\r\n", None, false),
                false,
                true,
            ),
            (
                "ahx_zlib",
                build_pdf("/Filter [/AHx /Fl]", &hex_encoded(&good_zlib), None, false),
                false,
                false,
            ),
            (
                "a85_raw",
                build_pdf("/Filter [/A85 /Fl]", &a85_encoded(&good_raw), None, false),
                false,
                false,
            ),
            (
                "chain_fl_rl",
                build_pdf(
                    "/Filter [/Fl /RL]",
                    &encoded(&rle_encoded(line)),
                    None,
                    false,
                ),
                false,
                false,
            ),
            (
                "chain_rl_fl",
                build_pdf("/Filter [/RL /Fl]", &rle_encoded(&good_zlib), None, false),
                false,
                false,
            ),
            (
                "chain_fallback_fl_rl",
                {
                    let mut d = encoded(&rle_encoded(line));
                    *d.last_mut().unwrap() ^= 1;
                    build_pdf("/Filter [/Fl /RL]", &d, None, false)
                },
                false,
                false,
            ),
            (
                "resource_only_zlib",
                build_pdf("/Filter /FlateDecode", &good_zlib, None, false),
                false,
                false,
            ),
            (
                "resource_only_raw",
                build_pdf("/Filter /FlateDecode", &good_raw, None, false),
                false,
                false,
            ),
        ];

        // #778 で厳しい Flate の固定確保を含めると 8 文書中 7 文書で超過した平文・RC4・AESV2 ×
        // 単独 Flate / ASCIIHex+Flate × 内容 / フォームの組も直接確かめる。
        const RC4_DICT: &str = "<< /Filter /Standard /V 1 /R 2 /P -4 /O <2055c756c72e1ad702608e8196acad447ad32d17cff583235f6dd15fed7dab67> /U <92c034164f1ef00c9edf3ba7595c16b11c50ff99a474b82f495eabb623e302f8> >>";
        const AES_DICT: &str = "<< /Filter /Standard /V 4 /R 4 /Length 128 /P -4 /O <36451bd39d753b7c1d10922c28e6665aa4f3353fb0348b536893e3b1db5c579b> /U <acf99d8614017dd73cbe2bc3c79b668c00000000000000000000000000000000> /EncryptMetadata true /StmF /StdCF /StrF /StdCF /CF << /StdCF << /CFM /AESV2 /Length 16 >> >> >>";
        const MASK4: &str = "0e2030bbbfaf55ac6d99bd277e57b9511bf2c89378aa7cd909f60049d3695daa5690faced25007a6a2e844038f8fa960c5b8a298cdaa1c334229d50fa2ba2fcd56263a401271adba4c3cc853dd98b63b287fa73943bdfd521784d2ed15fb7623b992270f3c2a59b81b72ca9fa0cdd557d38a0512d1b763d9f8add1abf6fecb6bbe1a9ca8529f262134a8db396f9b602f5933250ba2e13f7b2c476407d0782cace24d1fc8695cdea2062d018416eee9fed264bbce0992c9d295d66fcd8bf45b0201515a9dfd6fee217e6a2939c47db985b7d50630b57e8db14a7b53506c86ac4e38fdf398cf036e99324147fc190ae11e";
        const AES_FLATE: &str = "0405060708090a0b0c0d0e0f10111213870d7a396736892523f4600752838846a7c9d0ae30e8b1bd8f83d2000b75b59c";
        const AES_AHX_FLATE: &str = "0405060708090a0b0c0d0e0f10111213e98f74943257b7034ccaac38f51af952e69f0ababc9a75975faae6299e1165c356fe6da7de95587a81fa2c9653f70741";
        let clear_flate = unhex("78da33503050c85530543054c85108e60200143e02b9");
        let clear_ahx_flate = unhex(
            "37386461333335303330353063383535333035343330353463383531303865363032303031343365303262393e",
        );
        let mask4 = unhex(MASK4);
        for (tag, filter, clear, aes, reinflate) in [
            (
                "enc_fl",
                "/Filter /Fl",
                clear_flate.as_slice(),
                AES_FLATE,
                true,
            ),
            (
                "enc_ahx_fl",
                "/Filter [/AHx /Fl] /DP [null null]",
                clear_ahx_flate.as_slice(),
                AES_AHX_FLATE,
                false,
            ),
        ] {
            for (mode_name, mode, dict) in [
                ("plain", 0, None),
                ("rc4", 1, Some(RC4_DICT)),
                ("aes", 2, Some(AES_DICT)),
            ] {
                for form in [false, true] {
                    let stored: Vec<u8> = match mode {
                        0 => clear.to_vec(),
                        1 => clear.iter().zip(&mask4).map(|(&a, &b)| a ^ b).collect(),
                        _ => unhex(aes),
                    };
                    let label: &'static str = Box::leak(
                        format!(
                            "{tag}_{mode_name}_{}",
                            if form { "form" } else { "content" }
                        )
                        .into_boxed_str(),
                    );
                    cases.push((
                        label,
                        build_pdf(filter, &stored, dict, form),
                        mode != 0,
                        reinflate,
                    ));
                }
            }
        }

        let empty_refs = BTreeSet::new();
        for (label, pdf_bytes, encrypted, check_reinflate) in &cases {
            let pdf = HayroPdf::new(Arc::new(pdf_bytes.clone())).unwrap();
            let stream = pdf
                .xref()
                .get::<Stream<'_>>(ObjectIdentifier::new(4, 0))
                .unwrap();
            let source = Source::new(pdf_bytes, *encrypted, 0).with_file_refs(&empty_refs, 2048);

            // Work=0・DecodedBytes=0・開始前の中断では確保 0 バイトで止まる。
            for (limits, cancelled) in [
                (Limits::default().with(Kind::Work, Limit::Finite(0)), false),
                (
                    Limits::default().with(Kind::DecodedBytes, Limit::Finite(0)),
                    false,
                ),
                (Limits::default(), true),
            ] {
                let mut budget = Budget::new(limits);
                let mut diagnostics = Diagnostics::new();
                let cancel = CancelToken::new();
                if cancelled {
                    cancel.cancel();
                }
                let (res, alloc_bytes, alloc_calls) = crate::test_alloc::measure(|| {
                    before(
                        &stream,
                        source,
                        &mut Cx::new(&mut budget, &mut diagnostics, &cancel),
                    )
                });
                assert!(res.is_err(), "{label}");
                assert_eq!(
                    (alloc_bytes, alloc_calls),
                    (0, 0),
                    "{label}: allocated before first Cx check"
                );
            }

            // 実際の before + decoded_data (+ 内容・フォームの leniently_inflated) の
            // 解放を戻さない累積確保量が DecodedBytes 予約内にあることを確かめる。
            let limits = Limits::default()
                .with(Kind::Work, Limit::Unlimited)
                .with(Kind::DecodedBytes, Limit::Unlimited);
            let mut budget = Budget::new(limits);
            let mut diagnostics = Diagnostics::new();
            let cancel = CancelToken::new();
            let (res, alloc_bytes, _) = crate::test_alloc::measure(|| -> Result<(), Stop> {
                let counted = before(
                    &stream,
                    source,
                    &mut Cx::new(&mut budget, &mut diagnostics, &cancel),
                )?;
                assert!(counted, "{label}");
                let decoded = stream.decoded_data();
                if *check_reinflate && let Ok(bytes) = &decoded {
                    let _ = crate::walk::leniently_inflated(
                        &stream,
                        bytes.len(),
                        source,
                        &mut Cx::new(&mut budget, &mut diagnostics, &cancel),
                    )?;
                }
                Ok(())
            });
            assert_eq!(res, Ok(()), "{label}");
            let reserved = budget.used(Kind::DecodedBytes);
            assert!(
                alloc_bytes <= reserved,
                "{label}: cumulative non-refunded {alloc_bytes} > reserved {reserved}"
            );
        }
    }
}
