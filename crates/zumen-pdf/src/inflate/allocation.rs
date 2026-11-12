//! 展開する出力の Vec と、Flate の固定のデコーダー・バッファー・一時の表を、確保前に予約する（#772・#779）。

use zumen_base::{Cx, Kind, Stop};

/// flate2 1.1.10（zlib-rs 0.6.8）の `Decompress::new` が初期化で確保する展開の状態と 32 KiB 窓の組
/// （64 ビット環境で 47,552 バイト、32 ビット環境でそれ以下）。固定の初期化の手間と合わせて予約する。
pub(crate) const FLATE_DECODER_BYTES: u64 = 49_152;
pub(crate) const FLATE_DECODER_WORK: u64 = FLATE_DECODER_BYTES.div_ceil(16);

/// `flate2::read::{ZlibDecoder, DeflateDecoder}::new` が初期化で確保する `Decompress` と 32 KiB の
/// `BufReader` の組（47,552 + 32,768 = 80,320 バイト）。hayro-syntax 0.9.0 の厳しい展開が 1 試行ごとに作る。
pub(super) const FLATE_READER_BYTES: u64 = 81_920;
const FLATE_READER_WORK: u64 = FLATE_READER_BYTES.div_ceil(16);

/// hayro の寛容な Flate がブロックごとに作る表と一時の列。動的な表は最大 2^15 個の u32 が 2 つ、
/// 符号長の表は最大 128 個の u32、長さの列は 320 + 19 バイト。非圧縮ブロックの写し（65,535 バイト）と、
/// 固定の表の写しも含めて 512 KiB で上から数える。出力の Vec は別に数える。
pub(super) const FLATE_BLOCK_BYTES: u64 = 1 << 19;
/// 表の生成は最大 320 個の符号について 2^15 個の項目を埋め、15 回の長さの走査と初期化も行う。
/// 非圧縮ブロックの写しを含め、ブロックごとに 2^24 の作業で上から数える。
const FLATE_BLOCK_WORK: u64 = 1 << 24;

/// 単独と系列で同じ予約を使う。bound は予測子前の出力量で、失敗した試行の量も含む。
pub(super) fn flate(input: usize, bound: u64, fallback: bool) -> (u64, u64) {
    // https://github.com/rust-lang/rust/blob/1.99.1/library/alloc/src/io/read.rs#L836
    // Rust 1.99.1 の read_to_end は、空きが 32 バイト未満になると EOF の確認前にも
    // try_reserve(32) する。最終容量は 2 * (出力長 + 32) 未満で、それまでの容量の合計は
    // その 2 倍未満。zlib・raw の最大 2 試行に各 128 バイトを足し、寛容な展開の
    // extend_from_slice/push の最小容量 8 には、別に 4 * 8 バイトを足す。
    let growth = bound
        .saturating_mul(4)
        .saturating_add(256)
        .saturating_add(if fallback { 32 } else { 0 });
    // RFC 1951 の 3.2.3: ブロックは最低 3 ビットのヘッダーを使う。
    // 表は出力が空でも作るため、出力量では数えない。
    let blocks = if fallback {
        (input as u64).saturating_mul(8).div_ceil(3)
    } else {
        0
    };
    (
        growth
            .div_ceil(16)
            .saturating_add(blocks.saturating_mul(FLATE_BLOCK_WORK)),
        growth.saturating_add(blocks.saturating_mul(FLATE_BLOCK_BYTES)),
    )
}

/// 上流の `flate::decode` が厳しい `flate2::read` のデコーダーを初期化する分（1 または 2 試行）。
/// 出力の Vec の容量予約とは分けて予約する。
pub(super) fn flate_readers(strict_attempts: u64) -> (u64, u64) {
    (
        strict_attempts.saturating_mul(FLATE_READER_WORK),
        strict_attempts.saturating_mul(FLATE_READER_BYTES),
    )
}

/// LZW の出力 Vec。辞書のセルと各語の確保は lzw::count_output が別に数える。
pub(super) fn lzw(output: u64) -> (u64, u64) {
    let growth = output.saturating_mul(4).saturating_add(32);
    (growth.div_ceil(16), growth)
}

pub(super) fn reserve((work, bytes): (u64, u64), cx: &mut Cx<'_>) -> Result<(), Stop> {
    cx.check(Kind::Work, work)?;
    cx.check(Kind::DecodedBytes, bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 上流の extend_from_slice / push が要求した容量を、新旧の確保を含めて足す。
    fn extend(output: &mut Vec<u8>, bytes: &[u8], allocated: &mut u64) {
        let before = output.capacity();
        output.extend_from_slice(bytes);
        if output.capacity() != before {
            *allocated += output.capacity() as u64;
        }
    }

    #[test]
    fn lzw_output_bound_covers_actual_vec_capacities() {
        for chunk in [1usize, 2, 7, 8, 9, 31, 32, 33, 255, 3839] {
            let word = vec![0; chunk];
            let mut output = Vec::new();
            let mut allocated = 0;
            for _ in 0..40 {
                extend(&mut output, &word, &mut allocated);
                let (_, bound) = lzw(output.len() as u64);
                assert!(allocated <= bound, "{chunk}: {allocated} > {bound}");
            }
        }
    }

    /// 固定した Rust の read_to_end と同じ 32 バイトの先読み・予約を、公開の Vec 操作でたどる。
    /// 入力を小さく刻み、実際の容量を観測する。予約量の式自体は使わない。
    fn read_to_end_capacity(length: usize, chunk: usize) -> (u64, usize) {
        let data = vec![0; length];
        let mut output = Vec::new();
        let mut allocated = 0;
        let first = length.min(chunk).min(32);
        extend(&mut output, &data[..first], &mut allocated);
        if first == 0 {
            return (allocated, output.capacity());
        }
        loop {
            if output.capacity() - output.len() < 32 {
                let before = output.capacity();
                output.reserve(32);
                if output.capacity() != before {
                    allocated += output.capacity() as u64;
                }
            }
            let read = (length - output.len())
                .min(chunk)
                .min(output.capacity() - output.len());
            if read == 0 {
                return (allocated, output.capacity());
            }
            let start = output.len();
            let end = start + read;
            extend(&mut output, &data[start..end], &mut allocated);
        }
    }

    #[test]
    fn flate_output_bound_includes_capacity_reserved_before_eof() {
        use std::io::Read;

        struct ShortReads {
            left: usize,
            chunk: usize,
        }

        impl Read for ShortReads {
            fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
                let read = self.left.min(self.chunk).min(buffer.len());
                buffer[..read].fill(0);
                self.left -= read;
                Ok(read)
            }
        }

        // 32 バイトを最後に足すだけでは足りない。35 バイトを 1 バイトずつ返すと、
        // 8・33・66・132 の容量を順に要求する。
        assert_eq!(read_to_end_capacity(35, 1).0, 239);
        assert!(read_to_end_capacity(35, 1).0 > 4 * 35 + 96);
        for length in [
            0usize, 1, 7, 8, 9, 31, 32, 33, 35, 63, 65, 97, 8191, 8192, 8193, 32769,
        ] {
            for chunk in [1, 2, 3, 7, 31, 32, 33, 8192] {
                let (allocated, capacity) = read_to_end_capacity(length, chunk);
                let mut actual = Vec::new();
                ShortReads {
                    left: length,
                    chunk,
                }
                .read_to_end(&mut actual)
                .unwrap();
                assert_eq!(actual.len(), length);
                assert_eq!(actual.capacity(), capacity);
                let (_, bound) = flate(0, 2 * length as u64, false);
                assert!(2 * allocated <= bound, "{length}/{chunk}: {allocated}");
            }
        }
    }

    #[test]
    fn maximum_lengths_saturate_and_still_stop_before_reserving() {
        use zumen_base::{Budget, CancelToken, Diagnostics, Limits};

        for cost in [
            flate(usize::MAX, u64::MAX, true),
            flate_readers(u64::MAX),
            lzw(u64::MAX),
        ] {
            let mut budget = Budget::new(Limits::default());
            let mut diagnostics = Diagnostics::new();
            let cancel = CancelToken::new();
            assert!(reserve(cost, &mut Cx::new(&mut budget, &mut diagnostics, &cancel)).is_err());
            assert_eq!(budget.used(Kind::DecodedBytes), 0);
        }
    }
}
