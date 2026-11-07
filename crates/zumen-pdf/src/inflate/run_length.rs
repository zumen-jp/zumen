//! RunLength（7.4.5）の出力を、バイト列を保持せず長さだけで先に数える（#629）。

use zumen_base::{Cx, Kind, Stop};

/// hayro-syntax 0.9.0 の `filter/run_length.rs` と同じ順で符号を読み、出力の長さだけを数える。
/// 128 で終わる。0〜127 は続く 1〜128 バイトを写し、足りなければ写さずに終わる（成功）。129〜255 は次の
/// 1 バイトを 257 − 符号の回数くり返し、そのバイトがなければ失敗する。128 の前に入力が終わっても失敗する。
/// 上流が成功する入力では出力の長さを、失敗する入力では `None` を返す。失敗までに上流が作る分も数え、戻さない
pub(super) fn count(data: &[u8], cx: &mut Cx<'_>) -> Result<Option<u64>, Stop> {
    cx.check(Kind::Work, (data.len() as u64).div_ceil(16))?;
    let mut rest = data;
    let mut length = 0u64;
    loop {
        let Some((&code, tail)) = rest.split_first() else {
            return Ok(None);
        };
        let (added, temporary) = match code {
            128 => return Ok(Some(length)),
            0..=127 => {
                let literal = usize::from(code) + 1;
                let Some(next) = tail.get(literal..) else {
                    return Ok(Some(length));
                };
                rest = next;
                (literal as u64, 0)
            }
            _ => {
                let Some((_, next)) = tail.split_first() else {
                    return Ok(None);
                };
                rest = next;
                let run = 257 - u64::from(code);
                (run, run)
            }
        };
        // 上流は空の Vec に足していき、くり返しのたびに同じ長さの一時の列を作ってから写す
        let bytes = allocated(length + added) - allocated(length) + temporary;
        cx.check(Kind::Work, 1 + bytes.div_ceil(16))?;
        cx.check(Kind::DecodedBytes, bytes)?;
        length += added;
    }
}

/// 倍に伸ばす Vec の全確保の合計は、最終の長さ（最小容量 8）を 4 倍した量未満（ASCII85 と同じ見積もり）
fn allocated(length: u64) -> u64 {
    if length == 0 { 0 } else { length.max(8) * 4 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hayro_syntax::object::Stream;
    use hayro_syntax::reader::{Reader, ReaderContext, ReaderExt};
    use zumen_base::{Budget, CancelToken, Diagnostics, Limit, Limits};

    fn run(data: &[u8], limits: Limits, cancelled: bool) -> (Result<Option<u64>, Stop>, Budget) {
        let mut budget = Budget::new(limits);
        let mut diagnostics = Diagnostics::new();
        let cancel = CancelToken::new();
        if cancelled {
            cancel.cancel();
        }
        let result = count(data, &mut Cx::new(&mut budget, &mut diagnostics, &cancel));
        (result, budget)
    }

    /// hayro-syntax の展開の長さ。失敗は `None`
    fn upstream(data: &[u8]) -> Option<u64> {
        let mut source = format!(
            "<< /Filter /RunLengthDecode /Length {} >>\nstream\n",
            data.len()
        )
        .into_bytes();
        source.extend(data);
        source.extend(b"\nendstream");
        let stream = Reader::new(&source)
            .read_with_context::<Stream<'_>>(&ReaderContext::dummy())
            .unwrap();
        stream.decoded_data().ok().map(|data| data.len() as u64)
    }

    /// 上流と同じ足し方で、Vec の容量が変わるたびの確保と、くり返しの一時の列の合計を求める
    fn simulated(data: &[u8]) -> u64 {
        let mut decoded: Vec<u8> = Vec::new();
        let mut total = 0;
        let mut rest = data;
        while let Some((&code, tail)) = rest.split_first() {
            let capacity = decoded.capacity();
            match code {
                128 => break,
                0..=127 => {
                    let Some(bytes) = tail.get(..usize::from(code) + 1) else {
                        break;
                    };
                    decoded.extend(bytes);
                    rest = &tail[bytes.len()..];
                }
                _ => {
                    let Some((&byte, next)) = tail.split_first() else {
                        break;
                    };
                    let run = [byte].repeat(257 - usize::from(code));
                    total += run.capacity() as u64;
                    decoded.extend(run);
                    rest = next;
                }
            }
            if decoded.capacity() != capacity {
                total += decoded.capacity() as u64;
            }
        }
        total
    }

    #[test]
    fn matches_hayro_for_literals_runs_terminators_and_truncation() {
        let literal: Vec<u8> = [127].into_iter().chain(0..128).chain([128]).collect();
        // 入力、上流の結果、DecodedBytes。リテラルは出力の Vec、くり返しは一時の列も数える
        let cases: [(&[u8], Option<u64>, u64); 11] = [
            (&[128], Some(0), 0),
            (&[], None, 0),
            (&[4, 10, 11, 12, 13, 14, 253, 3, 128], Some(9), 36 + 4),
            (&[0, 7, 128, 0, 9], Some(1), 32),
            (&[1, 5, 6, 3, 1, 2], Some(2), 32),
            (&[3, 1, 2], Some(0), 0),
            (&[1, 5, 6, 255], None, 32),
            (&[1, 5, 6], None, 32),
            (&literal, Some(128), 512),
            (&[129, 7, 128], Some(128), 512 + 128),
            (&[255, 7, 128], Some(2), 32 + 2),
        ];
        for (data, expected, bytes) in cases {
            assert_eq!(upstream(data), expected, "{data:?}");
            let (result, budget) = run(data, Limits::default(), false);
            assert_eq!(result, Ok(expected), "{data:?}");
            assert_eq!(budget.used(Kind::DecodedBytes), bytes, "{data:?}");
        }
    }

    #[test]
    fn matches_hayro_and_bounds_its_allocations_for_generated_inputs() {
        let alphabet = [0, 1, 2, 126, 127, 128, 129, 130, 254, 255];
        let mut inputs: Vec<Vec<u8>> = vec![Vec::new()];
        let mut last = inputs.clone();
        for _ in 0..4 {
            last = last
                .iter()
                .flat_map(|data| {
                    alphabet.iter().map(move |&byte| {
                        let mut next = data.clone();
                        next.push(byte);
                        next
                    })
                })
                .collect();
            inputs.extend(last.iter().cloned());
        }
        // 長いリテラルとくり返しが混ざる入力。固定の線形合同法で作る
        let mut state = 1u32;
        for length in 0..600 {
            let data = (0..length)
                .map(|_| {
                    state = state.wrapping_mul(1_103_515_245).wrapping_add(12_345);
                    (state >> 16) as u8
                })
                .collect();
            inputs.push(data);
        }
        for data in &inputs {
            let (result, budget) = run(data, Limits::default(), false);
            assert_eq!(result, Ok(upstream(data)), "{data:?}");
            assert!(
                simulated(data) <= budget.used(Kind::DecodedBytes),
                "{data:?}: {} > {}",
                simulated(data),
                budget.used(Kind::DecodedBytes)
            );
            // 失敗した入力は、失敗した所に終端を置いて成功する入力と同じ量を数える。
            // 入力が尽きたなら末尾に足し、くり返すバイトがないならその符号を置き換える
            if result == Ok(None) {
                let mut appended = data.clone();
                appended.push(128);
                let mut replaced = data[..data.len().saturating_sub(1)].to_vec();
                replaced.push(128);
                assert!(
                    [appended, replaced].iter().any(|ended| {
                        let (result, ended) = run(ended, Limits::default(), false);
                        matches!(result, Ok(Some(_)))
                            && ended.used(Kind::DecodedBytes) == budget.used(Kind::DecodedBytes)
                    }),
                    "{data:?}"
                );
            }
        }
    }

    #[test]
    fn stops_small_inputs_before_large_outputs() {
        let mut data = [129, b' '].repeat(512);
        data.push(128);
        assert_eq!(data.len(), 1025);
        assert_eq!(upstream(&data), Some(64 * 1024));
        let limits = Limits::default().with(Kind::DecodedBytes, Limit::Finite(32 * 1024));
        let (result, budget) = run(&data, limits, false);
        assert!(matches!(result, Err(Stop::Exceeded(e)) if e.kind == Kind::DecodedBytes));
        // くり返し 1 回は、出力の Vec の 4 × 128 と一時の列の 128
        assert_eq!(budget.used(Kind::DecodedBytes), 51 * 640);
        assert_eq!(budget.exceeded().unwrap().requested, 52 * 640);
        assert_eq!((result, budget), run(&data, limits, false));
        let (result, budget) = run(&data, Limits::default(), false);
        assert_eq!(result, Ok(Some(64 * 1024)));
        assert_eq!(budget.used(Kind::DecodedBytes), 4 * 64 * 1024 + 64 * 1024);
        assert_eq!(budget.used(Kind::Work), 65 + 512 * 41);
    }

    #[test]
    fn stops_at_exact_budgets_and_on_cancel() {
        let mut data = [0, b'a', 4, 1, 2, 3, 4, 5, 200, b'b'].repeat(64);
        data.push(128);
        let (expected, budget) = run(&data, Limits::default(), false);
        assert!(matches!(expected, Ok(Some(_))));
        for kind in [Kind::DecodedBytes, Kind::Work] {
            let exact = Limits::default().with(kind, Limit::Finite(budget.used(kind)));
            let again = run(&data, exact, false);
            assert_eq!(again.0, expected);
            assert_eq!(again.1.used(kind), budget.used(kind));
            let short = Limits::default().with(kind, Limit::Finite(budget.used(kind) - 1));
            let stopped = run(&data, short, false);
            assert!(matches!(stopped.0, Err(Stop::Exceeded(e)) if e.kind == kind));
            assert!(stopped.1.used(Kind::DecodedBytes) < budget.used(Kind::DecodedBytes));
            assert_eq!(stopped, run(&data, short, false));
        }
        let cancelled = run(&data, Limits::default(), true);
        assert_eq!(cancelled.0, Err(Stop::Cancelled));
        assert_eq!(cancelled.1.used(Kind::DecodedBytes), 0);
    }
}
