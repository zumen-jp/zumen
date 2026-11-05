//! LZW（7.4.4）の出力と辞書を、バイト列を保持せず長さだけで先に数える（#616）。

use hayro_syntax::object::{Array, Dict, Name, Object, Stream};
use zumen_base::{Cx, Kind, Stop};

/// 12 ビットの辞書。長さだけを持つので、確認自身の表は 8 KiB で一定
const ENTRIES: usize = 4096;
const FIRST: usize = 258;
/// hayro-syntax の Option<Vec<u8>> の項目と、表の容量の拡大中の写しを含めた見積もり。
/// 64 ビットの項目は 24 バイト。環境によらず同じ量を数え、文字列の中身は別に足す
const ENTRY_BYTES: u64 = 64;

pub(super) fn before(stream: &Stream<'_>, cx: &mut Cx<'_>) -> Result<(), Stop> {
    let early = early_change(stream, cx)?;
    count(&stream.raw_data(), early, cx)
}

/// hayro-syntax 0.9.0 の Stream::filters_and_params と同じ優先順で、LZW のパラメーターを引く。
/// 名前として読めない項目で止まり、知らない名前もパラメーターの位置を 1 つ進める。
/// 有効なフィルターが単独 LZW（先頭 Crypt を除く）であることは、呼ぶ側で確認済み
fn early_change(stream: &Stream<'_>, cx: &mut Cx<'_>) -> Result<bool, Stop> {
    let dict = stream.dict();
    let is_lzw = |name: &Name<'_>| matches!(name.as_ref(), b"LZWDecode" | b"LZW");
    let early = |params: Option<Dict<'_>>| {
        params
            .and_then(|dict| dict.get::<u8>(b"EarlyChange"))
            .is_none_or(|value| value != 0)
    };
    if dict
        .get::<Name<'_>>(b"F")
        .or_else(|| dict.get::<Name<'_>>(b"Filter"))
        .as_ref()
        .is_some_and(is_lzw)
    {
        return Ok(early(
            dict.get::<Dict<'_>>(b"DP")
                .or_else(|| dict.get::<Dict<'_>>(b"DecodeParms")),
        ));
    }
    let Some(filters) = dict
        .get::<Array<'_>>(b"F")
        .or_else(|| dict.get::<Array<'_>>(b"Filter"))
    else {
        return Ok(true);
    };
    let mut params = dict
        .get::<Array<'_>>(b"DP")
        .or_else(|| dict.get::<Array<'_>>(b"DecodeParms"))
        .map(|array| array.iter::<Object<'_>>());
    for name in filters.iter::<Name<'_>>() {
        cx.check(Kind::Work, 1)?;
        let param = params
            .as_mut()
            .and_then(Iterator::next)
            .and_then(Object::into_dict);
        if is_lzw(&name) {
            return Ok(early(param));
        }
    }
    Ok(true)
}

/// 符号の位置をバイトと残ったビットで持つ。入力長を 8 倍せず、32 ビットでも同じ符号を読む
struct Codes<'a> {
    bytes: std::slice::Iter<'a, u8>,
    pending: u32,
    bits: u8,
}

impl Codes<'_> {
    fn next(&mut self, width: u8) -> Option<usize> {
        while self.bits < width {
            self.pending = (self.pending << 8) | u32::from(*self.bytes.next()?);
            self.bits += 8;
        }
        self.bits -= width;
        let code = self.pending >> self.bits;
        self.pending &= (1 << self.bits) - 1;
        Some(code as usize)
    }
}

/// hayro-syntax 0.9.0 の LZW の展開順を長さでたどる。不正な符号では上流も失敗するが、
/// そこまでに作る分を戻さない。入力が途中で切れた場合は、上流が返す部分の出力まで数える
fn count(data: &[u8], early: bool, cx: &mut Cx<'_>) -> Result<(), Stop> {
    cx.check(Kind::Work, (data.len() as u64).div_ceil(16) + 512)?;
    cx.check(Kind::DecodedBytes, FIRST as u64 * ENTRY_BYTES + 256)?;
    let mut lengths = [0u16; ENTRIES];
    lengths[..256].fill(1);
    let mut size = FIRST;
    let mut previous: Option<u16> = None;
    let mut codes = Codes {
        bytes: data.iter(),
        pending: 0,
        bits: 0,
    };
    loop {
        cx.check(Kind::Work, 1)?;
        let adjusted = size + usize::from(early);
        let width = match adjusted {
            2048.. => 12,
            1024.. => 11,
            512.. => 10,
            _ => 9,
        };
        let Some(code) = codes.next(width) else {
            return Ok(());
        };
        match code {
            256 => {
                size = FIRST;
                previous = None;
            }
            257 => return Ok(()),
            _ if code > size || (code == size && previous.is_none()) => return Ok(()),
            _ => {
                let length = if code < size {
                    let Some(length) = lengths.get(code).copied() else {
                        return Ok(());
                    };
                    length
                } else {
                    // 次の辞書の番号は、前の語とその先頭の 1 バイト（7.4.4.2）
                    let Some(length) = previous else {
                        return Ok(());
                    };
                    length + 1
                };
                cx.check(Kind::Work, u64::from(length).div_ceil(16))?;
                cx.check(Kind::DecodedBytes, u64::from(length))?;
                if let Some(prev) = previous {
                    let added = prev + 1;
                    // 上流は表が満杯でも、前の語を写した後で追加を断るので、その確保も数える。
                    // 表への追加がある場合は、Vec の項目と容量の拡大も数える
                    cx.check(Kind::Work, u64::from(added).div_ceil(16) + 1)?;
                    cx.check(
                        Kind::DecodedBytes,
                        u64::from(added) + if size < ENTRIES { ENTRY_BYTES } else { 0 },
                    )?;
                    if let Some(entry) = lengths.get_mut(size) {
                        *entry = added;
                        size += 1;
                    }
                }
                previous = Some(length);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hayro_syntax::reader::{Reader, ReaderContext, ReaderExt};
    use zumen_base::{Budget, CancelToken, Diagnostics, Limit, Limits};

    mod fixture {
        include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/common/lzw.rs"));
    }

    fn source(filter: &str, data: &[u8]) -> Vec<u8> {
        let mut out = format!("<< /Length {} {filter} >>\nstream\n", data.len()).into_bytes();
        out.extend(data);
        out.extend(b"\nendstream");
        out
    }

    fn stream(source: &[u8]) -> Stream<'_> {
        Reader::new(source)
            .read_with_context::<Stream<'_>>(&ReaderContext::dummy())
            .unwrap()
    }

    fn run(source: &[u8], limits: Limits, cancelled: bool) -> (Result<bool, Stop>, Budget) {
        let mut budget = Budget::new(limits);
        let mut diagnostics = Diagnostics::new();
        let cancel = CancelToken::new();
        if cancelled {
            cancel.cancel();
        }
        let result = crate::inflate::before(
            &stream(source),
            &mut Cx::new(&mut budget, &mut diagnostics, &cancel),
        );
        (result, budget)
    }

    #[test]
    fn counts_dictionary_widths_clear_and_full_table_against_hayro() {
        for early in [false, true] {
            for last in [258, 509, 510, 511, 1022, 1023, 2046, 2047, 4095] {
                let mut codes = vec![256, 65];
                codes.extend(258..=last);
                codes.extend([256, 66, 258, 257]);
                let data = fixture::pack(&codes, early);
                let input = source(
                    &format!(
                        "/Filter /LZWDecode /DecodeParms << /EarlyChange {} >>",
                        u8::from(early)
                    ),
                    &data,
                );
                let decoded = stream(&input).decoded_data().unwrap().into_owned();
                let longest = usize::from(last) - 256;
                assert_eq!(decoded.len(), longest * (longest + 1) / 2 + 3);
                assert!(decoded[..decoded.len() - 3].iter().all(|&b| b == b'A'));
                let (result, budget) = run(&input, Limits::default(), false);
                assert_eq!(result, Ok(true));
                assert!(budget.used(Kind::DecodedBytes) > decoded.len() as u64);
            }
            // 表が満杯の後にも上流は文字列を写す。繰り返し分も予算から落とさない
            let mut codes = vec![256, 65];
            codes.extend(258..=4095);
            let first = source("/Filter /LZWDecode", &fixture::pack(&codes, early));
            codes.extend([4095; 4]);
            codes.push(257);
            let second = source("/Filter /LZWDecode", &fixture::pack(&codes, early));
            let mut first_budget = Budget::default();
            let mut second_budget = Budget::default();
            for (input, budget) in [(&first, &mut first_budget), (&second, &mut second_budget)] {
                count(
                    &stream(input).raw_data(),
                    early,
                    &mut Cx::new(budget, &mut Diagnostics::new(), &CancelToken::new()),
                )
                .unwrap();
            }
            assert!(
                second_budget.used(Kind::DecodedBytes)
                    >= first_budget.used(Kind::DecodedBytes) + 4 * (3839 + 3840)
            );
        }
    }

    #[test]
    fn resolves_early_change_like_the_stream_decoder() {
        let plain = b"q 0 0 1 rg 0 0 50 50 re f Q\n".repeat(1000);
        for early in [0, 1] {
            let data = fixture::encode(&plain, early != 0);
            for filter in [
                format!("/F /LZW /DP << /EarlyChange {early} >>"),
                format!("/Filter [/LZWDecode] /DecodeParms [<< /EarlyChange {early} >>]"),
                format!("/F [/Crypt /LZW] /DP [null << /EarlyChange {early} >>]"),
                format!(
                    "/Filter [/Unknown /LZWDecode] /DecodeParms [null << /EarlyChange {early} >>]"
                ),
                format!("/F /Unknown /Filter [/LZW] /DP [<< /EarlyChange {early} >>]"),
            ] {
                let input = source(&filter, &data);
                assert!(
                    stream(&input).decoded_data().unwrap().as_ref() == plain,
                    "{filter}"
                );
                assert_eq!(run(&input, Limits::default(), false).0, Ok(true));
            }
        }
        // u8 で読めない指定は既定の 1。DP が優先で、読めなければ DecodeParms を使う
        let data = fixture::encode(&plain, true);
        for filter in [
            "/Filter /LZWDecode",
            "/Filter /LZWDecode /DP << /EarlyChange 256 >>",
            "/F /LZW /DP << /EarlyChange 2 >> /DecodeParms << /EarlyChange 0 >>",
            "/F /LZW /DP null /DecodeParms << /EarlyChange -1 >>",
        ] {
            let input = source(filter, &data);
            assert!(
                stream(&input).decoded_data().unwrap().as_ref() == plain,
                "{filter}"
            );
            assert_eq!(run(&input, Limits::default(), false).0, Ok(true));
        }
    }

    #[test]
    fn bounds_damaged_streams_and_preserves_failed_prefix_costs() {
        for early in [false, true] {
            let data = fixture::encode(&b"abcdefabcdefabc".repeat(250), early);
            let filter = format!(
                "/Filter /LZWDecode /DP << /EarlyChange {} >>",
                u8::from(early)
            );
            let check = |data: &[u8]| {
                let input = source(&filter, data);
                let (result, budget) = run(&input, Limits::default(), false);
                assert_eq!(result, Ok(true));
                if let Ok(decoded) = stream(&input).decoded_data() {
                    assert!(decoded.len() as u64 <= budget.used(Kind::DecodedBytes));
                }
            };
            for end in 0..=data.len() {
                check(&data[..end]);
            }
            for at in 0..data.len() {
                for bit in 0..8 {
                    let mut damaged = data.clone();
                    damaged[at] ^= 1 << bit;
                    check(&damaged);
                }
            }
            let prefix = fixture::pack(&[256, 65, 258, 259, 500], early);
            let input = source(&filter, &prefix);
            assert!(stream(&input).decoded_data().is_err());
            assert!(
                run(&input, Limits::default(), false)
                    .1
                    .used(Kind::DecodedBytes)
                    > FIRST as u64 * ENTRY_BYTES + 256
            );
        }
    }

    #[test]
    fn stops_before_large_outputs_at_budget_boundaries_and_on_cancel() {
        let data = fixture::encode(&vec![b' '; 128 * 1024], true);
        assert!(data.len() < 1024);
        let input = source("/Filter /LZWDecode", &data);
        for kind in [Kind::DecodedBytes, Kind::Work] {
            let limit = if kind == Kind::DecodedBytes {
                32 * 1024
            } else {
                600
            };
            let result = run(
                &input,
                Limits::default().with(kind, Limit::Finite(limit)),
                false,
            );
            assert!(matches!(result.0, Err(Stop::Exceeded(e)) if e.kind == kind));
            assert!(result.1.used(Kind::DecodedBytes) < 128 * 1024);
            assert_eq!(
                result,
                run(
                    &input,
                    Limits::default().with(kind, Limit::Finite(limit)),
                    false
                )
            );
        }
        let exact = run(&input, Limits::default(), false).1;
        for kind in [Kind::DecodedBytes, Kind::Work] {
            assert_eq!(
                run(
                    &input,
                    Limits::default().with(kind, Limit::Finite(exact.used(kind))),
                    false
                )
                .0,
                Ok(true)
            );
            assert!(
                matches!(run(&input, Limits::default().with(kind, Limit::Finite(exact.used(kind) - 1)), false).0, Err(Stop::Exceeded(e)) if e.kind == kind)
            );
        }
        let result = run(&input, Limits::default(), true);
        assert_eq!(result.0, Err(Stop::Cancelled));
        assert_eq!(result.1.used(Kind::DecodedBytes), 0);
    }
}
