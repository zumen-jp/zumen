//! UTF-16 の文字を UTF-8 にし、原本との位置の対応を持つ（XML 1.0 4.3.3）。
//!
//! 同じ文字幅が続く所を一つの区間にする。ASCII の多い文書に、文字ごとの表を確保しない。

use core::ops::Range;

use zumen_base::{Cx, Diagnostic, Error, Kind, Ref, codes};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Order {
    Little,
    Big,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Detection {
    pub(crate) order: Order,
    pub(crate) bom: bool,
}

pub(crate) fn is_utf32(src: &[u8]) -> bool {
    matches!(
        src,
        [0, 0, 0xfe, 0xff, ..]
            | [0xff, 0xfe, 0, 0, ..]
            | [0, 0, 0xff, 0xfe, ..]
            | [0xfe, 0xff, 0, 0, ..]
            | [0, 0, 0, b'<', ..]
            | [b'<', 0, 0, 0, ..]
            | [0, 0, b'<', 0, ..]
            | [0, b'<', 0, 0, ..]
    )
}

pub(crate) fn detect(src: &[u8]) -> Option<Detection> {
    if is_utf32(src) {
        return None;
    }
    let (order, bom) = match src {
        [0xff, 0xfe, ..] => (Order::Little, true),
        [0xfe, 0xff, ..] => (Order::Big, true),
        [b'<', 0, b'?', 0, ..] => (Order::Little, false),
        [0, b'<', 0, b'?', ..] => (Order::Big, false),
        _ => return None,
    };
    Some(Detection { order, bom })
}

/// XML 1.0 の NameStartChar / NameChar と、名前空間 1.0 の QName。
pub(crate) fn valid_name(bytes: &[u8]) -> bool {
    crate::name::valid_utf8(bytes)
}

/// 幅が等しい文字の並び。予算ではプラットフォームによらず 1 区間を 32 バイトと数える。
#[derive(Clone, Debug, PartialEq, Eq)]
struct Run {
    decoded: u64,
    original: u64,
    decoded_width: u8,
    original_width: u8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Decoded {
    pub(crate) bytes: Vec<u8>,
    runs: Vec<Run>,
    start: usize,
    original_len: usize,
}

fn invalid(cx: &mut Cx, range: Range<usize>) -> Error {
    cx.report(Diagnostic::new(codes::XML_DOCUMENT_INVALID).at(place(range)));
    Error::Invalid
}

fn place(range: Range<usize>) -> Ref {
    Ref::at_bytes(range.start as u64, range.end as u64)
}

fn charge(cx: &mut Cx, kind: Kind, amount: u64, range: Range<usize>) -> Result<(), Error> {
    cx.check(kind, amount).map_err(|stop| {
        cx.report(stop.to_diagnostic().at(place(range)));
        Error::Stopped(stop)
    })
}

fn unit(src: &[u8], offset: usize, order: Order) -> Option<u16> {
    let &[a, b] = src.get(offset..offset.checked_add(2)?)? else {
        return None;
    };
    Some(match order {
        Order::Little => u16::from_le_bytes([a, b]),
        Order::Big => u16::from_be_bytes([a, b]),
    })
}

/// 文字ごとに渡すが、手間は入力の UTF-16 コード単位ごとに数える。
fn walk(
    src: &[u8],
    detection: Detection,
    cx: &mut Cx,
    mut character: impl FnMut(char, Range<usize>),
) -> Result<(), Error> {
    let mut offset = if detection.bom { 2 } else { 0 };
    while offset < src.len() {
        let start = offset;
        charge(cx, Kind::Work, 1, offset..(offset + 2).min(src.len()))?;
        let Some(first) = unit(src, offset, detection.order) else {
            return Err(invalid(cx, offset..src.len()));
        };
        offset += 2;
        let value = match first {
            0xd800..=0xdbff => {
                charge(cx, Kind::Work, 1, offset..(offset + 2).min(src.len()))?;
                let Some(second @ 0xdc00..=0xdfff) = unit(src, offset, detection.order) else {
                    return Err(invalid(cx, start..(offset + 2).min(src.len())));
                };
                offset += 2;
                0x10000 + ((u32::from(first) - 0xd800) << 10) + u32::from(second) - 0xdc00
            }
            0xdc00..=0xdfff => return Err(invalid(cx, start..offset)),
            _ => u32::from(first),
        };
        // XML 1.0 の Char。符号化の不正を置換文字へ黙って変えない。
        if !matches!(value, 0x9 | 0xa | 0xd | 0x20..=0xd7ff | 0xe000..=0xfffd | 0x10000..=0x10ffff)
        {
            return Err(invalid(cx, start..offset));
        }
        let Some(value) = char::from_u32(value) else {
            return Err(invalid(cx, start..offset));
        };
        character(value, start..offset);
    }
    Ok(())
}

impl Decoded {
    pub(crate) fn read(src: &[u8], detection: Detection, cx: &mut Cx) -> Result<Self, Error> {
        let mut length = 0_usize;
        let mut count = 0_usize;
        let mut previous = None;
        let mut overflow = false;
        walk(src, detection, cx, |value, raw| {
            if let Some(next) = length.checked_add(value.len_utf8()) {
                length = next;
            } else {
                overflow = true;
            }
            let widths = (value.len_utf8(), raw.len());
            if previous != Some(widths) {
                if let Some(next) = count.checked_add(1) {
                    count = next;
                } else {
                    overflow = true;
                }
                previous = Some(widths);
            }
        })?;
        if overflow {
            return Err(invalid(cx, 0..src.len()));
        }
        // 本文と位置の表を確保する前に、固定の量で予約する。usize の幅を予算へ使わない。
        let amount = (length as u64).saturating_add((count as u64).saturating_mul(32));
        charge(cx, Kind::DecodedBytes, amount, 0..src.len())?;
        let mut decoded = Self {
            bytes: Vec::with_capacity(length),
            runs: Vec::with_capacity(count),
            start: if detection.bom { 2 } else { 0 },
            original_len: src.len(),
        };
        walk(src, detection, cx, |value, raw| {
            let widths = (value.len_utf8() as u8, raw.len() as u8);
            if !decoded
                .runs
                .last()
                .is_some_and(|run| (run.decoded_width, run.original_width) == widths)
            {
                decoded.runs.push(Run {
                    decoded: decoded.bytes.len() as u64,
                    original: raw.start as u64,
                    decoded_width: widths.0,
                    original_width: widths.1,
                });
            }
            let mut bytes = [0_u8; 4];
            decoded
                .bytes
                .extend_from_slice(value.encode_utf8(&mut bytes).as_bytes());
        })?;
        Ok(decoded)
    }

    fn original_offset(&self, offset: usize, end: bool) -> usize {
        if offset >= self.bytes.len() {
            return self.original_len;
        }
        let at = self
            .runs
            .partition_point(|run| run.decoded <= offset as u64);
        let Some(run) = at.checked_sub(1).and_then(|at| self.runs.get(at)) else {
            return self.start;
        };
        let delta = offset as u64 - run.decoded;
        let width = u64::from(run.decoded_width);
        let characters = delta / width + u64::from(end && !delta.is_multiple_of(width));
        usize::try_from(run.original + characters * u64::from(run.original_width))
            .unwrap_or(self.original_len)
    }

    /// 途中のバイトを指す診断は、原本の文字全体へ広げる。
    pub(crate) fn original_range(&self, range: Range<usize>) -> Range<usize> {
        self.original_offset(range.start, false)..self.original_offset(range.end, true)
    }

    fn decoded_offset(&self, offset: usize, end: bool) -> usize {
        if offset >= self.original_len {
            return self.bytes.len();
        }
        let at = self
            .runs
            .partition_point(|run| run.original <= offset as u64);
        let Some(run) = at.checked_sub(1).and_then(|at| self.runs.get(at)) else {
            return 0;
        };
        let delta = offset as u64 - run.original;
        let width = u64::from(run.original_width);
        let characters = delta / width + u64::from(end && !delta.is_multiple_of(width));
        usize::try_from(run.decoded + characters * u64::from(run.decoded_width))
            .unwrap_or(self.bytes.len())
    }

    pub(crate) fn decoded_range(&self, range: Range<usize>) -> Range<usize> {
        self.decoded_offset(range.start, false)..self.decoded_offset(range.end, true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zumen_base::{Budget, CancelToken, Diagnostics};

    fn encoded(value: &str, order: Order) -> Vec<u8> {
        let mut bytes = match order {
            Order::Little => vec![0xff, 0xfe],
            Order::Big => vec![0xfe, 0xff],
        };
        for value in value.encode_utf16() {
            bytes.extend_from_slice(&match order {
                Order::Little => value.to_le_bytes(),
                Order::Big => value.to_be_bytes(),
            });
        }
        bytes
    }

    #[test]
    fn maps_runs_and_ranges_in_both_byte_orders() {
        for order in [Order::Little, Order::Big] {
            let src = encoded("A日本😀B", order);
            let mut budget = Budget::default();
            let mut diag = Diagnostics::new();
            let cancel = CancelToken::new();
            let decoded = Decoded::read(
                &src,
                detect(&src).unwrap(),
                &mut Cx::new(&mut budget, &mut diag, &cancel),
            )
            .unwrap();
            assert_eq!(decoded.bytes, "A日本😀B".as_bytes());
            assert_eq!(decoded.runs.len(), 4);
            for (norm, raw) in [(0, 2), (1, 4), (4, 6), (7, 8), (11, 12), (12, 14)] {
                assert_eq!(decoded.original_range(norm..norm), raw..raw);
                assert_eq!(decoded.decoded_range(raw..raw), norm..norm);
            }
            assert_eq!(decoded.original_range(8..9), 8..12);
            assert_eq!(decoded.decoded_range(9..10), 7..11);
            assert_eq!(budget.used(Kind::Work), 12);
            assert_eq!(budget.used(Kind::DecodedBytes), 12 + 4 * 32);
            assert!(diag.is_empty());
        }
    }
}
