//! 複数のフィルターを、各段の確保前に数える（#769）。

use std::borrow::Cow;
use std::io::Write;

use hayro_syntax::Filter;
use hayro_syntax::object::Stream;
use hayro_syntax::reader::{Reader, ReaderContext, ReaderExt};
use zumen_base::{Cx, Kind, Stop};

use super::{Source, allocation, ascii, lzw, run_length, stages::Stage};

/// 合成する辞書・stream と endstream の最大長。整数はすべて 20 桁以下で、
/// 名前も固定なので、この長さを超えない。入力の長さと合わせて、確保前に数える。
const ENVELOPE: usize = 512;

/// 元のストリームは最後に hayro が展開する。確認用の中間出力も予算に数えてから
/// 保持し、次の段だけを同じ hayro で展開する。失敗した段から先へは進まない。
pub(super) fn before(
    stream: &Stream<'_>,
    stages: &[Stage],
    source: Source<'_>,
    cx: &mut Cx<'_>,
) -> Result<bool, Stop> {
    if source.external(stream.dict(), cx)? {
        return Ok(false);
    }
    source.reserve_decryption(stream, cx)?;
    let mut data = stream.raw_data();
    for (index, &stage) in stages.iter().enumerate() {
        cx.check(Kind::Work, 1)?;
        match stage.filter {
            Filter::Crypt => return Ok(true),
            Filter::AsciiHexDecode | Filter::Ascii85Decode => {
                let encoding = if stage.filter == Filter::AsciiHexDecode {
                    ascii::Encoding::Hex
                } else {
                    ascii::Encoding::Base85
                };
                let Some(decoded) = ascii::decode(&data, encoding, cx)? else {
                    return Ok(true);
                };
                data = Cow::Owned(decoded);
            }
            Filter::FlateDecode | Filter::LzwDecode | Filter::RunLengthDecode => {
                let bytes_before = cx.budget.used(Kind::DecodedBytes);
                let work_before = cx.budget.used(Kind::Work);
                let mut fallback = false;
                let output_bound = match stage.filter {
                    Filter::FlateDecode => {
                        fallback = super::count_fallback(&data, cx)?;
                        // 厳しい展開と寛容な展開の試行は、どれも出力量以上を数える。
                        cx.budget.used(Kind::DecodedBytes) - bytes_before
                    }
                    Filter::LzwDecode => lzw::count_output(&data, stage.early, cx)?,
                    _ => {
                        let Some(length) = run_length::count(&data, cx)? else {
                            return Ok(true);
                        };
                        length
                    }
                };
                // 失敗した試みと辞書を含むので、実際の出力長以上になる。
                let bytes = cx.budget.used(Kind::DecodedBytes) - bytes_before;
                let work = cx.budget.used(Kind::Work) - work_before;
                // RunLength は既存の count が伸長も数える。Flate は最大 3 回の試み、
                // LZW は 1 回の出力の Vec の、全確保の合計を数える。
                let (growth_work, growth_bytes) = match stage.filter {
                    Filter::FlateDecode => allocation::flate(data.len(), output_bound, fallback),
                    Filter::LzwDecode => allocation::lzw(output_bound),
                    _ => (0, 0),
                };
                let (predictor_work, predictor_bytes) = stage.predictor_cost(output_bound);
                let extra_bytes = growth_bytes.saturating_add(predictor_bytes);
                let extra_work = growth_work.saturating_add(predictor_work);
                reserve(extra_work, extra_bytes, cx)?;
                if index + 1 == stages.len() {
                    return Ok(true);
                }
                // 確認用の展開をもう 1 回行う分。最終的な hayro の展開の予約とは分ける。
                reserve(
                    work.saturating_add(extra_work),
                    bytes.saturating_add(extra_bytes),
                    cx,
                )?;
                let Some(decoded) = decode(stage, &data, cx)? else {
                    return Ok(true);
                };
                data = Cow::Owned(decoded);
            }
            _ => return Ok(false),
        }
    }
    Ok(true)
}

fn reserve(work: u64, bytes: u64, cx: &mut Cx<'_>) -> Result<(), Stop> {
    cx.check(Kind::Work, work)?;
    cx.check(Kind::DecodedBytes, bytes)
}

/// 元の暗号化と参照を持ち込まない、解決済みの整数だけの 1 段のストリーム。
/// 確保の予約は呼ぶ側で済ませ、辞書と入力の写しはここで確保前に数える。
fn decode(stage: Stage, data: &[u8], cx: &mut Cx<'_>) -> Result<Option<Vec<u8>>, Stop> {
    decode_encoded(stage, data, data.len() as u64 > u64::from(u32::MAX), cx)
}

fn decode_encoded(
    stage: Stage,
    data: &[u8],
    hex: bool,
    cx: &mut Cx<'_>,
) -> Result<Option<Vec<u8>>, Stop> {
    // Stream の /Length は u32。超える中間出力だけは ASCIIHex で包み、/Length を
    // 省略する。fallback の endstream 検索が 16 進文字のデータに一致することはなく、
    // 終端の > で元のバイト列が過不足なく戻る。2 GiB の既定予算ではこの経路に至らない。
    let payload = (data.len() as u64).saturating_mul(if hex { 2 } else { 1 });
    let bytes = payload.saturating_add(ENVELOPE as u64);
    let ascii_bytes = if hex {
        (data.len() as u64).saturating_add(2)
    } else {
        0
    };
    // 固定キーの辞書の表、16 進にするときの出力も含める。
    // 入力を写す手間と辞書・終端を読む手間を先に数える。
    reserve(
        bytes.div_ceil(8).saturating_add(128),
        bytes.saturating_add(ascii_bytes).saturating_add(4096),
        cx,
    )?;
    let Some(capacity) = usize::try_from(bytes).ok() else {
        return Ok(None);
    };
    let mut encoded = Vec::with_capacity(capacity);
    encoded.extend_from_slice(b"<< ");
    if hex {
        let _ = write!(
            encoded,
            "/Filter [/ASCIIHexDecode /{}] /DecodeParms [null ",
            stage.name()
        );
    } else {
        let _ = write!(
            encoded,
            "/Length {} /Filter /{} /DecodeParms ",
            data.len(),
            stage.name()
        );
    }
    let _ = write!(
        encoded,
        "<< /Predictor {} /Colors {} /BitsPerComponent {} /Columns {} /EarlyChange {} >>",
        stage.predictor,
        stage.colors,
        stage.bits,
        stage.columns,
        u8::from(stage.early),
    );
    if hex {
        encoded.push(b']');
    }
    encoded.extend_from_slice(b" >>\nstream\n");
    if hex {
        const DIGITS: &[u8; 16] = b"0123456789abcdef";
        for chunk in data.chunks(4096) {
            cx.check(Kind::Work, 0)?;
            for &byte in chunk {
                encoded.push(*DIGITS.get(usize::from(byte >> 4)).unwrap_or(&b'0'));
                encoded.push(*DIGITS.get(usize::from(byte & 15)).unwrap_or(&b'0'));
            }
        }
        encoded.push(b'>');
    } else {
        encoded.extend_from_slice(data);
    }
    encoded.extend_from_slice(b"\nendstream");
    let Some(stream) =
        Reader::new(&encoded).read_with_context::<Stream<'_>>(&ReaderContext::dummy())
    else {
        return Ok(None);
    };
    let output = stream.decoded_data().ok().map(Cow::into_owned);
    cx.check(Kind::Work, 0)?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inflate::stages::read as stages;
    use zumen_base::{Budget, CancelToken, Diagnostics, Limit, Limits};

    mod lzw_fixture {
        include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/common/lzw.rs"));
    }

    fn source(filters: &str, bytes: &[u8]) -> Vec<u8> {
        let mut source = format!("<< /Length {} {filters} >>\nstream\n", bytes.len()).into_bytes();
        source.extend_from_slice(bytes);
        source.extend_from_slice(b"\nendstream");
        source
    }

    fn stream(source: &[u8]) -> Stream<'_> {
        Reader::new(source)
            .read_with_context(&ReaderContext::dummy())
            .unwrap()
    }

    #[test]
    fn aligns_parameters_with_unknown_names_and_stops_on_other_objects() {
        for (filter, expected) in [
            (
                "/Filter [/Unknown /Fl /LZW] /DP [<< /Columns 90 >> << /Columns 3 >> << /EarlyChange 0 >>]",
                vec![
                    (Filter::FlateDecode, 3, true),
                    (Filter::LzwDecode, 1, false),
                ],
            ),
            (
                "/Filter [/Fl 12 /LZW] /DP [<< /Columns 5 >> null << /EarlyChange 0 >>]",
                vec![(Filter::FlateDecode, 5, true)],
            ),
            (
                "/F /Unknown /Filter [/LZW] /DP [<< /EarlyChange 0 >>]",
                vec![(Filter::LzwDecode, 1, false)],
            ),
        ] {
            let source = source(filter, b"");
            let mut budget = Budget::default();
            let mut diagnostics = Diagnostics::new();
            let cancel = CancelToken::new();
            let stages = stages(
                stream(&source).dict(),
                &mut Cx::new(&mut budget, &mut diagnostics, &cancel),
            )
            .unwrap();
            assert_eq!(
                stages
                    .iter()
                    .map(|s| (s.filter, s.columns, s.early))
                    .collect::<Vec<_>>(),
                expected
            );
        }
    }

    #[test]
    fn synthetic_stages_preserve_predictors_and_both_lzw_width_rules() {
        for params in [
            "/Predictor 1",
            "/Predictor 2 /Colors 1 /Columns 8 /BitsPerComponent 8",
            "/Predictor 12 /Colors 3 /Columns 2 /BitsPerComponent 8",
            "/Predictor 2 /Colors 5 /Columns 13 /BitsPerComponent 1",
            "/Predictor 15 /Colors 1 /Columns 7 /BitsPerComponent 4",
            "/Predictor 12 /Colors 3 /Columns 2 /BitsPerComponent 16",
        ] {
            for early in [false, true] {
                let plain = b"endstream\0\xff >\n".repeat(8);
                let mut encoder =
                    flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::best());
                encoder.write_all(&plain).unwrap();
                for (name, encoded) in [
                    ("FlateDecode", encoder.finish().unwrap()),
                    ("LZWDecode", lzw_fixture::encode(&plain, early)),
                ] {
                    let filter = format!(
                        "/Filter /{name} /DP << {params} /EarlyChange {} >>",
                        u8::from(early)
                    );
                    let source = source(&filter, &encoded);
                    let stream = stream(&source);
                    let mut budget = Budget::default();
                    let mut diagnostics = Diagnostics::new();
                    let cancel = CancelToken::new();
                    let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
                    let stage = stages(stream.dict(), &mut cx).unwrap()[0];
                    let expected = stream.decoded_data().ok().map(Cow::into_owned);
                    assert_eq!(
                        decode_encoded(stage, &encoded, false, &mut cx).unwrap(),
                        expected
                    );
                    assert_eq!(
                        decode_encoded(stage, &encoded, true, &mut cx).unwrap(),
                        expected
                    );
                }
            }
        }
    }

    #[test]
    fn hex_envelope_keeps_embedded_endstream_bytes() {
        let mut data = vec![8];
        data.extend_from_slice(b"endstream");
        data.push(128);
        let mut budget = Budget::default();
        let mut diagnostics = Diagnostics::new();
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
        let stage = Stage::new(Filter::RunLengthDecode, None);
        assert_eq!(
            decode_encoded(stage, &data, true, &mut cx).unwrap(),
            Some(b"endstream".to_vec())
        );
    }

    #[test]
    fn standard_eight_bit_predictors_do_not_reserve_generic_bit_chunks() {
        for (predictor, colors, bits, columns) in
            [(2, 3, 8, 100), (12, 3, 8, 100), (15, 3, 16, 100)]
        {
            let mut stage = Stage::new(Filter::FlateDecode, None);
            stage.predictor = predictor;
            stage.colors = colors;
            stage.bits = bits;
            stage.columns = columns;
            let (work, allocated) = stage.predictor_cost(64 * 1024);
            assert!(work >= 64 * 1024);
            assert_eq!(allocated, 0);
        }
    }

    #[test]
    fn reserves_fallback_tables_before_materializing_small_outputs() {
        let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::best());
        encoder.write_all(&[0, b'A', 128]).unwrap();
        let mut data = encoder.finish().unwrap();
        // Adler-32 だけを変え、厳しいリーダーの後に寛容な展開を使わせる。
        *data.last_mut().unwrap() ^= 1;
        let input = source("/Filter [/Fl /RL]", &data);
        let stream = stream(&input);
        assert_eq!(stream.decoded_data().unwrap().as_ref(), b"A");
        for kind in [Kind::DecodedBytes, Kind::Work] {
            let limit = 512 * 1024;
            let mut budget = Budget::new(Limits::default().with(kind, Limit::Finite(limit)));
            let mut diagnostics = Diagnostics::new();
            let cancel = CancelToken::new();
            let result = super::super::before(
                &stream,
                Source::default(),
                &mut Cx::new(&mut budget, &mut diagnostics, &cancel),
            );
            assert!(matches!(result, Err(Stop::Exceeded(e)) if e.kind == kind));
            assert!(budget.exceeded().unwrap().requested > limit);
        }
        let mut budget = Budget::default();
        let mut diagnostics = Diagnostics::new();
        let cancel = CancelToken::new();
        assert_eq!(
            super::super::before(
                &stream,
                Source::default(),
                &mut Cx::new(&mut budget, &mut diagnostics, &cancel),
            ),
            Ok(true)
        );
        assert!(budget.used(Kind::DecodedBytes) >= allocation::FLATE_BLOCK_BYTES);
    }
}
