//! 行とタグの読み。今の実装の `Lines` と `DxfFile::read` と同じ切り方をする（`crates/zumen-acad/src/dxf.rs`）。
//!
//! 行は LF で切り、直前の CR を 1 つ除く。グループ・コードの行と値の行の 2 行で 1 つのタグになる。

use std::thread;

/// タグの場所。位置は、入力の先頭からのバイト数
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Span {
    /// グループ・コード
    pub code: i16,
    /// グループ・コードの行の先頭
    pub start: u32,
    /// 値の行の先頭
    pub value: u32,
    /// 値の終わり（改行を除く）
    pub value_end: u32,
    /// 次のタグの先頭
    pub end: u32,
}

/// 次のタグを読んだ結果
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Next {
    /// タグ
    Tag(Span),
    /// 入力の終わり
    End,
    /// 値の行がないか、グループ・コードを読めない
    Invalid,
}

/// 行の中身の終わりと、次の行の先頭。`pos` は入力の中にあること
fn line(src: &[u8], pos: usize) -> (usize, usize) {
    let rest = src.get(pos..).unwrap_or_default();
    let (mut end, next) = match rest.iter().position(|&b| b == b'\n') {
        Some(n) => (pos + n, pos + n + 1),
        None => (src.len(), src.len()),
    };
    if end > pos && src.get(end - 1) == Some(&b'\r') {
        end -= 1;
    }
    (end, next)
}

/// グループ・コード。前後の空白と符号を受け付ける
pub fn parse_code(line: &[u8]) -> Option<i16> {
    let text = line.trim_ascii();
    // 1〜4 桁の数字だけなら、そのまま数える（`str::parse` と同じ値になる）
    if (1..=4).contains(&text.len()) && text.iter().all(u8::is_ascii_digit) {
        return Some(text.iter().fold(0i16, |n, &b| n * 10 + i16::from(b - b'0')));
    }
    core::str::from_utf8(text).ok()?.parse().ok()
}

/// `pos` から始まるタグを読む。入力は 4 GiB 未満であること（呼ぶ側が確かめる）
pub fn next_tag(src: &[u8], pos: usize) -> Next {
    if pos >= src.len() {
        return Next::End;
    }
    let (code_end, value) = line(src, pos);
    if value >= src.len() {
        return Next::Invalid;
    }
    let Some(code) = parse_code(src.get(pos..code_end).unwrap_or_default()) else {
        return Next::Invalid;
    };
    let (value_end, end) = line(src, value);
    Next::Tag(Span {
        code,
        start: pos as u32,
        value: value as u32,
        value_end: value_end as u32,
        end: end as u32,
    })
}

/// 入力を、レコードの先頭（グループ・コードが 0 のタグの先頭）で `parts` 個までに分ける位置。
/// 最初は 0、最後は入力の長さ。行の偶奇は、手前の改行の数から決める
pub fn split_points(src: &[u8], parts: usize) -> Vec<usize> {
    let n = src.len();
    let parts = parts.max(1);
    if parts == 1 || n < parts * 4096 {
        return vec![0, n];
    }
    let raw: Vec<usize> = (0..=parts)
        .map(|k| if k == parts { n } else { n / parts * k })
        .collect();
    // 区間ごとの改行の数
    let counts: Vec<usize> = thread::scope(|scope| {
        let handles: Vec<_> = raw
            .windows(2)
            .map(|pair| {
                let part = src.get(pair[0]..pair[1]).unwrap_or_default();
                scope.spawn(move || part.iter().filter(|&&b| b == b'\n').count())
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().unwrap_or(0))
            .collect()
    });
    let mut out = vec![0usize];
    let mut before = 0usize;
    for k in 1..parts {
        before += counts.get(k - 1).copied().unwrap_or(0);
        let at = raw.get(k).copied().unwrap_or(n);
        // `at` より後の最初の行の先頭と、その行の番号（0 始まり）
        let (mut pos, mut number) = if at > 0 && src.get(at - 1) == Some(&b'\n') {
            (at, before)
        } else {
            (line(src, at).1, before + 1)
        };
        // 値の行なら、次のグループ・コードの行へ進む
        if number % 2 == 1 && pos < n {
            pos = line(src, pos).1;
            number += 1;
        }
        let _ = number;
        // グループ・コードが 0 のタグまで進む
        let found = loop {
            match next_tag(src, pos) {
                Next::Tag(tag) if tag.code == 0 => break Some(pos),
                Next::Tag(tag) => pos = tag.end as usize,
                Next::End | Next::Invalid => break None,
            }
        };
        match found {
            Some(pos) if out.last().is_some_and(|&last| pos > last) => out.push(pos),
            _ => {}
        }
    }
    out.push(n);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn group_codes_allow_padding_and_signs() {
        assert_eq!(parse_code(b"  0"), Some(0));
        assert_eq!(parse_code(b"10 "), Some(10));
        assert_eq!(parse_code(b"-1"), Some(-1));
        assert_eq!(parse_code(b"+5"), Some(5));
        assert_eq!(parse_code(b""), None);
        assert_eq!(parse_code(b"1 0"), None);
        assert_eq!(parse_code(b"99999"), None);
        assert_eq!(parse_code(b"9999"), Some(9999));
    }

    #[test]
    fn tags_strip_cr_lf_and_keep_the_last_line() {
        let src = b"  0\r\nLINE\r\n 10\n1.5";
        let Next::Tag(first) = next_tag(src, 0) else {
            panic!("最初のタグ");
        };
        assert_eq!(
            (
                first.code,
                first.start,
                first.value,
                first.value_end,
                first.end
            ),
            (0, 0, 5, 9, 11)
        );
        let Next::Tag(second) = next_tag(src, 11) else {
            panic!("次のタグ");
        };
        assert_eq!(
            (second.code, second.value, second.value_end, second.end),
            (10, 15, 18, 18)
        );
        assert_eq!(next_tag(src, 18), Next::End);
        assert_eq!(next_tag(b"  0\r\n", 0), Next::Invalid);
        assert_eq!(next_tag(b"x\r\nLINE\r\n", 0), Next::Invalid);
    }

    #[test]
    fn split_points_are_record_starts() {
        let mut src = Vec::new();
        for i in 0..4000 {
            // 値の行が「0」のタグ（画層 0）を、レコードの先頭と取り違えない
            src.extend_from_slice(
                format!("  0\r\nLINE\r\n  8\r\n0\r\n 10\r\n{i}.0\r\n").as_bytes(),
            );
        }
        let cuts = split_points(&src, 7);
        assert_eq!(cuts.first(), Some(&0));
        assert_eq!(cuts.last(), Some(&src.len()));
        assert!(cuts.len() > 2);
        for pair in cuts.windows(2) {
            assert!(pair[0] < pair[1]);
        }
        for &cut in cuts.iter().take(cuts.len() - 1) {
            assert!(src[cut..].starts_with(b"  0\r\nLINE\r\n"), "{cut}");
        }
    }
}
