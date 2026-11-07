//! ASCIIHex・ASCII85（7.4.2・7.4.3）の中間出力を、確保前に数える（#623）。

use zumen_base::{Cx, Kind, Stop};

#[derive(Clone, Copy)]
pub(super) enum Encoding {
    Hex,
    Base85,
}

/// 最初は長さだけを数える。上流の確保と、自身が保持する中間出力を先に数えてから、同じ入力を展開する。
/// 予算内の元のストリームは引き続き hayro が展開し、この出力は圧縮の事前確認だけに使う。
pub(super) fn decode(
    data: &[u8],
    encoding: Encoding,
    cx: &mut Cx<'_>,
) -> Result<Option<Vec<u8>>, Stop> {
    // hayro-syntax 0.9.0 は、失敗する入力でも先に容量を確保する。Hex は終端以後も含めて多めに数える。
    // ASCII85 の途中の z は 4 バイトを出し、初期の容量を超えて Vec を伸ばす場合がある。
    let initial = match encoding {
        Encoding::Hex => (data.len() as u64).div_ceil(2),
        Encoding::Base85 => data.len() as u64 * 4 / 5,
    };
    cx.check(Kind::Work, (data.len() as u64).div_ceil(16))?;
    cx.check(
        Kind::DecodedBytes,
        initial
            + if matches!(encoding, Encoding::Base85) {
                5
            } else {
                0
            },
    )?;
    let mut length = 0usize;
    let start = cx.budget.used(Kind::Work);
    let valid = visit(data, encoding, cx, 1, |bytes| length += bytes.len())?;
    let scan_work = cx.budget.used(Kind::Work) - start;
    // 倍に伸ばす Vec の全確保の合計は、最終の長さ（最小容量 8）を 4 倍した量未満。
    // 初期の容量も別に残し、不正な文字や不正な末尾に至るまでの部分も数える。
    let growth = if matches!(encoding, Encoding::Base85) && length as u64 > initial {
        (length as u64).max(8) * 4
    } else {
        0
    };
    cx.check(Kind::Work, (length as u64 + growth).div_ceil(16))?;
    cx.check(Kind::DecodedBytes, growth)?;
    if !valid {
        return Ok(None);
    }
    // 同じ位置まで読む手間をまとめて予約し、残量がなければ中間出力を確保しない。
    // 2 回目も 16 バイトごとに中断を見る。終端より後の入力は再走査しない。
    cx.check(Kind::Work, scan_work)?;
    cx.check(Kind::DecodedBytes, length as u64)?;
    let mut output = Vec::with_capacity(length);
    visit(data, encoding, cx, 0, |bytes| {
        output.extend_from_slice(bytes)
    })?;
    Ok(Some(output))
}

/// 文字を 16 個読むたびに予算と中断を確認する。空白だけの長い入力にも同じ間隔を使う。
/// hayro の終端省略、奇数の Hex、ASCII85 の部分群の後の z と、u32 のあふれの扱いに合わせる。
fn visit(
    data: &[u8],
    encoding: Encoding,
    cx: &mut Cx<'_>,
    work_per_chunk: u64,
    mut emit: impl FnMut(&[u8]),
) -> Result<bool, Stop> {
    let mut group = [0u8; 5];
    let mut used = 0;
    for (index, &byte) in data.iter().enumerate() {
        if index % 16 == 0 {
            cx.check(Kind::Work, work_per_chunk)?;
        }
        if matches!(byte, 0 | 9 | 10 | 12 | 13 | 32) {
            continue;
        }
        match encoding {
            Encoding::Hex => {
                if byte == b'>' {
                    break;
                }
                let digit = match byte {
                    b'0'..=b'9' => byte - b'0',
                    b'A'..=b'F' => byte - b'A' + 10,
                    b'a'..=b'f' => byte - b'a' + 10,
                    _ => return Ok(false),
                };
                if used == 0 {
                    group[0] = digit << 4;
                    used = 1;
                } else {
                    emit(&[group[0] | digit]);
                    used = 0;
                }
            }
            Encoding::Base85 => match byte {
                b'!'..=b'u' => {
                    let Some(digit) = group.get_mut(used) else {
                        return Ok(false);
                    };
                    *digit = byte - b'!';
                    used += 1;
                    if used == 5 {
                        if !flush(&group, used, &mut emit) {
                            return Ok(false);
                        }
                        used = 0;
                    }
                }
                b'z' => {
                    if !flush(&group, used, &mut emit) {
                        return Ok(false);
                    }
                    used = 0;
                    emit(&[0; 4]);
                }
                b'~' => break,
                _ => return Ok(false),
            },
        }
    }
    cx.check(Kind::Work, 0)?;
    Ok(match encoding {
        Encoding::Hex => {
            if used == 1 {
                emit(&group[..1]);
            }
            true
        }
        Encoding::Base85 => flush(&group, used, &mut emit),
    })
}

fn flush(group: &[u8; 5], used: usize, emit: &mut impl FnMut(&[u8])) -> bool {
    if used == 0 {
        return true;
    }
    if used == 1 {
        return false;
    }
    let mut value = 0u32;
    for (index, &digit) in group.iter().enumerate() {
        let Some(next) = value
            .checked_mul(85)
            .and_then(|n| n.checked_add(u32::from(if index < used { digit } else { 84 })))
        else {
            return false;
        };
        value = next;
    }
    let bytes = value.to_be_bytes();
    let Some(bytes) = bytes.get(..used - 1) else {
        return false;
    };
    emit(bytes);
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use hayro_syntax::object::Stream;
    use hayro_syntax::reader::{Reader, ReaderContext, ReaderExt};
    use zumen_base::{Budget, CancelToken, Diagnostics, Limit, Limits};

    mod fixture {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/common/ascii.rs"
        ));
    }

    fn run(
        data: &[u8],
        encoding: Encoding,
        limits: Limits,
        cancelled: bool,
    ) -> (Result<Option<Vec<u8>>, Stop>, Budget) {
        let mut budget = Budget::new(limits);
        let mut diagnostics = Diagnostics::new();
        let cancel = CancelToken::new();
        if cancelled {
            cancel.cancel();
        }
        let result = decode(
            data,
            encoding,
            &mut Cx::new(&mut budget, &mut diagnostics, &cancel),
        );
        (result, budget)
    }

    fn compare(data: &[u8], encoding: Encoding) {
        let name = match encoding {
            Encoding::Hex => "ASCIIHexDecode",
            Encoding::Base85 => "ASCII85Decode",
        };
        let mut source =
            format!("<< /Filter /{name} /Length {} >>\nstream\n", data.len()).into_bytes();
        source.extend(data);
        source.extend(b"\nendstream");
        let stream = Reader::new(&source)
            .read_with_context::<Stream<'_>>(&ReaderContext::dummy())
            .unwrap();
        let expected = stream.decoded_data().ok().map(|data| data.into_owned());
        let actual = run(data, encoding, Limits::default(), false);
        assert_eq!(actual.0, Ok(expected), "{name}, {data:?}");
    }

    #[test]
    fn matches_hayro_for_padding_terminators_whitespace_and_invalid_groups() {
        for encoding in [Encoding::Hex, Encoding::Base85] {
            for data in [
                b"".as_slice(),
                b" ",
                b">tail",
                b"~tail",
                b"0",
                b"fFf>",
                b"123",
                b"z",
                b"zz",
                b"!z",
                b"!!z",
                b"!!!z",
                b"!!!!z",
                b"!!!!!z",
                b"uuuuu",
                b"s8W-!",
                b"s8W-\"",
                b"!!",
                b"!!!",
                b"!!!!",
                b"!!!!!",
                b"!!!!!!",
                b"!!~notgt",
                b"\0\t\n\x0c\r 12\0\t\n\x0c\r 34 ",
                b"12\x0b34",
                b"12\xff34",
                b"!!>!!",
                b"ffffffg",
                b"z!!!!!v",
            ] {
                compare(data, encoding);
            }
            for length in 0..=128 {
                let plain: Vec<_> = (0..length).map(|i| (i * 37) as u8).collect();
                let encoded = fixture::encode(&plain, matches!(encoding, Encoding::Base85));
                compare(&encoded, encoding);
                for end in 0..=encoded.len() {
                    compare(&encoded[..end], encoding);
                }
                for index in 0..encoded.len() {
                    let mut damaged = encoded.clone();
                    damaged[index] ^= 0x55;
                    compare(&damaged, encoding);
                }
            }
        }
    }

    #[test]
    fn reserves_initial_capacity_and_growth_even_when_decoding_fails() {
        for length in [0, 1, 2, 3, 5, 8, 16, 31, 128, 1024] {
            for invalid in [false, true] {
                let mut data = vec![b'z'; length];
                if invalid {
                    data.push(b'v');
                }
                let (_, budget) = run(&data, Encoding::Base85, Limits::default(), false);
                // 上流と同じ初期容量から、z ごとに 4 バイト足す場合の全確保と比べる。
                let mut upstream = Vec::with_capacity(data.len() * 4 / 5);
                let mut allocated = upstream.capacity() as u64 + 5;
                for _ in 0..length {
                    let previous = upstream.capacity();
                    upstream.extend_from_slice(&[0u8; 4]);
                    if previous != upstream.capacity() {
                        allocated += upstream.capacity() as u64;
                    }
                }
                if !invalid {
                    allocated += (length * 4) as u64;
                }
                assert!(
                    budget.used(Kind::DecodedBytes) >= allocated,
                    "{length}, {invalid}"
                );
                compare(&data, Encoding::Base85);
            }
        }
        for encoding in [Encoding::Hex, Encoding::Base85] {
            let mut data = vec![b'v'];
            data.extend(std::iter::repeat_n(b' ', 4096));
            let result = run(&data, encoding, Limits::default(), false);
            assert_eq!(result.0, Ok(None));
            assert!(result.1.used(Kind::DecodedBytes) >= 2048);
            let limited = Limits::default().with(Kind::DecodedBytes, Limit::Finite(1024));
            assert!(
                matches!(run(&data, encoding, limited, false).0, Err(Stop::Exceeded(e)) if e.kind == Kind::DecodedBytes)
            );
        }
    }

    #[test]
    fn reserves_second_scan_work_before_the_intermediate_output() {
        for (encoding, byte, end) in [(Encoding::Hex, b'0', b'>'), (Encoding::Base85, b'!', b'~')] {
            let mut data = vec![byte; 524_280];
            data.push(end);
            data.extend(std::iter::repeat_n(b' ', 8192));
            let (decoded, budget) = run(&data, encoding, Limits::default(), false);
            let length = decoded.unwrap().unwrap().len() as u64;
            let total = budget.used(Kind::Work);
            let scan = 524_281u64.div_ceil(16);
            assert_eq!(
                total,
                (data.len() as u64).div_ceil(16) + 2 * scan + length.div_ceil(16)
            );
            for remaining in [0, 1, scan - 1] {
                let limits =
                    Limits::default().with(Kind::Work, Limit::Finite(total - scan + remaining));
                let (result, stopped) = run(&data, encoding, limits, false);
                assert!(matches!(result, Err(Stop::Exceeded(e)) if e.kind == Kind::Work));
                assert_eq!(
                    stopped.used(Kind::DecodedBytes),
                    budget.used(Kind::DecodedBytes) - length
                );
            }
        }
    }

    #[test]
    fn checks_exact_work_bytes_and_cancel_before_materializing_the_middle() {
        for encoding in [Encoding::Hex, Encoding::Base85] {
            let data = [
                vec![b' '; 4096],
                fixture::encode(&[1, 2, 3, 4], matches!(encoding, Encoding::Base85)),
            ]
            .concat();
            let result = run(&data, encoding, Limits::default(), false);
            assert_eq!(result.0, Ok(Some(vec![1, 2, 3, 4])));
            for kind in [Kind::DecodedBytes, Kind::Work] {
                let amount = result.1.used(kind);
                assert_eq!(
                    run(
                        &data,
                        encoding,
                        Limits::default().with(kind, Limit::Finite(amount)),
                        false
                    )
                    .0,
                    result.0
                );
                let short = Limits::default().with(kind, Limit::Finite(amount - 1));
                let stopped = run(&data, encoding, short, false);
                assert!(matches!(stopped.0, Err(Stop::Exceeded(e)) if e.kind == kind));
                assert_eq!(stopped, run(&data, encoding, short, false));
            }
            let cancelled = run(&data, encoding, Limits::default(), true);
            assert_eq!(cancelled.0, Err(Stop::Cancelled));
            assert_eq!(cancelled.1.used(Kind::DecodedBytes), 0);
        }
    }
}
