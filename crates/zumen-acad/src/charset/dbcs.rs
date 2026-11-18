//! Windows の 2 バイトのコード・ページ（ANSI_936・ANSI_949・ANSI_950・ANSI_1361、#327）。
//!
//! 文字の表は Unicode の `MAPPINGS/VENDORS/MICSFT/WINDOWS/CP<番号>.TXT` による。この表は、Windows の
//! MultiByteToWideChar が私用領域と C1 制御文字に当てるバイトを除くと、それと同じ。表を写して持たず、WHATWG の
//! GBK・EUC-KR・Big5（encoding_rs）で 2 バイトを戻し、表にない範囲を除き、違う所を直して同じ表にする。
//! WHATWG の表は、CP949 とは同じで、CP936・CP950 の表を含み、私用領域・香港の追加字（HKSCS）・GB 18030 の
//! 追加の文字の分だけ広い。除く範囲と直す所は、Microsoft の表と encoding_rs 0.8.35 の全 2 バイトを比べて決めた
//! （検証キットの `codepage/` が、Windows の API・ezdxf・Python の codec に照らす）。
//!
//! ANSI_1361（Johab）の表は、Windows の MultiByteToWideChar が戻す文字による。ハングルの範囲（先頭のバイトが
//! 0x84〜0xD3）は、2 バイトの下の 15 ビットを 5 ビットずつの初声・中声・終声として、音節（U+AC00〜）か、1 つだけの
//! 字母を結合用の字母（U+1100〜）に戻す（Python の `johab` の codec は互換の字母 U+3131〜 に戻すが、Windows に合わせる）。
//! 記号と漢字（先頭のバイトが 0xD9〜0xDE・0xE0〜0xF9）は、KS X 1001 の行と列に直して、EUC-KR の表で戻す。KS X 1001
//! の 4 行目の字母は、現代の字母をハングルの範囲で表し、古い字母 43 は Windows が戻す結合用の字母を `fixed` に持つ。
//! この方法と Windows の表が全 65,536 の 2 バイトで同じことを、検証キットで確かめた。

use encoding_rs::Encoding;

/// 1 つの 2 バイトのコード・ページ
pub(super) struct Dbcs {
    /// 2 バイトを戻す WHATWG の符号化
    encoding: &'static Encoding,
    /// 先頭のバイトの範囲
    lead: (u8, u8),
    /// 1 バイトで文字を表す 0x80 以上のバイト
    single: Option<(u8, char)>,
    /// WHATWG の表にあり、Microsoft の表にない 2 バイトの範囲（両端を含む）
    excluded: &'static [(u16, u16)],
    /// WHATWG の表と違う、Microsoft の表の 2 バイトと文字。書くときも、この 2 バイトにする
    fixed: &'static [(u16, char)],
    /// Johab の 2 バイトか。ハングルは式で、記号と漢字は KS X 1001 の EUC-KR の 2 バイトに直して戻す
    johab: bool,
}

/// CP936（簡体字中国語、GBK）。WHATWG の GBK の 2 バイトのうち、GB 18030 が足した文字を除く
const CP936: Dbcs = Dbcs {
    encoding: encoding_rs::GBK,
    lead: (0x81, 0xFE),
    single: Some((0x80, '\u{20AC}')),
    excluded: &[
        (0xA2E3, 0xA2E3),
        (0xA3A0, 0xA3A0),
        (0xA6D9, 0xA6DF),
        (0xA6EC, 0xA6ED),
        (0xA6F3, 0xA6F3),
        (0xA8BC, 0xA8BC),
        (0xA8BF, 0xA8BF),
        (0xA989, 0xA995),
        (0xFE50, 0xFE50),
        (0xFE54, 0xFE6B),
        (0xFE6D, 0xFE75),
        (0xFE77, 0xFE7E),
        (0xFE80, 0xFE90),
        (0xFE92, 0xFEA0),
    ],
    fixed: &[],
    johab: false,
};

/// CP949（韓国語、統合型ハングル）。WHATWG の EUC-KR と同じ
const CP949: Dbcs = Dbcs {
    encoding: encoding_rs::EUC_KR,
    lead: (0x81, 0xFD),
    single: None,
    excluded: &[],
    fixed: &[],
    johab: false,
};

/// CP950（繁体字中国語、Big5）。WHATWG の Big5 のうち、先頭のバイトが A1〜F9 のもの。Microsoft の表にない ETEN の
/// 拡張の一部（制御文字の図形・日本語の仮名・部首など）を除く
const CP950: Dbcs = Dbcs {
    encoding: encoding_rs::BIG5,
    lead: (0xA1, 0xF9),
    single: None,
    excluded: &[
        (0xA3C0, 0xA3E0),
        (0xC6A1, 0xC6FE),
        (0xC740, 0xC77E),
        (0xC7A1, 0xC7FE),
        (0xC840, 0xC87E),
        (0xC8A1, 0xC8A4),
        (0xC8CD, 0xC8F1),
        (0xC8F5, 0xC8FE),
    ],
    fixed: &[(0xF9FE, '\u{2593}'), (0xC969, '\u{4EDD}')],
    johab: false,
};

/// CP1361（韓国語、Johab）。記号と漢字は WHATWG の EUC-KR（CP949 と同じ）の KS X 1001 の範囲で戻す
const CP1361: Dbcs = Dbcs {
    encoding: encoding_rs::EUC_KR,
    lead: (0x84, 0xF9),
    single: None,
    excluded: &[],
    fixed: &[
        (0xDAD4, '\u{115F}'),
        (0xDAD5, '\u{1114}'),
        (0xDAD6, '\u{1115}'),
        (0xDAD7, '\u{11C7}'),
        (0xDAD8, '\u{11C8}'),
        (0xDAD9, '\u{11CC}'),
        (0xDADA, '\u{11CE}'),
        (0xDADB, '\u{11D3}'),
        (0xDADC, '\u{11D7}'),
        (0xDADD, '\u{11D9}'),
        (0xDADE, '\u{111C}'),
        (0xDADF, '\u{11DD}'),
        (0xDAE0, '\u{11DF}'),
        (0xDAE1, '\u{111D}'),
        (0xDAE2, '\u{111E}'),
        (0xDAE3, '\u{1120}'),
        (0xDAE4, '\u{1122}'),
        (0xDAE5, '\u{1123}'),
        (0xDAE6, '\u{1127}'),
        (0xDAE7, '\u{1128}'),
        (0xDAE8, '\u{112B}'),
        (0xDAE9, '\u{112C}'),
        (0xDAEA, '\u{112D}'),
        (0xDAEB, '\u{112E}'),
        (0xDAEC, '\u{112F}'),
        (0xDAED, '\u{1132}'),
        (0xDAEE, '\u{1136}'),
        (0xDAEF, '\u{1140}'),
        (0xDAF0, '\u{1147}'),
        (0xDAF1, '\u{114C}'),
        (0xDAF2, '\u{1145}'),
        (0xDAF3, '\u{1146}'),
        (0xDAF4, '\u{1157}'),
        (0xDAF5, '\u{1158}'),
        (0xDAF6, '\u{1159}'),
        (0xDAF7, '\u{1184}'),
        (0xDAF8, '\u{1185}'),
        (0xDAF9, '\u{1188}'),
        (0xDAFA, '\u{1191}'),
        (0xDAFB, '\u{1192}'),
        (0xDAFC, '\u{1194}'),
        (0xDAFD, '\u{119E}'),
        (0xDAFE, '\u{11A1}'),
    ],
    johab: true,
};

/// コード・ページ `page` の表。2 バイトのコード・ページでない番号は `None`
pub(super) fn table(page: u16) -> Option<&'static Dbcs> {
    match page {
        936 => Some(&CP936),
        949 => Some(&CP949),
        950 => Some(&CP950),
        1361 => Some(&CP1361),
        _ => None,
    }
}

/// Johab の 2 バイトが表すもの
enum Johab {
    /// ハングルの範囲の音節か字母
    Hangul(char),
    /// 記号と漢字の、KS X 1001 の EUC-KR の 2 バイト
    Ks([u8; 2]),
}

/// Johab の 2 バイト `code` を、ハングルの文字か KS X 1001 の 2 バイトにする。どちらでもないものは `None`
fn johab(code: u16) -> Option<Johab> {
    let [lead, trail] = code.to_be_bytes();
    if lead <= 0xD3 {
        return hangul(code).map(Johab::Hangul);
    }
    let row = match lead {
        0xD9..=0xDE => 2 * (lead - 0xD9) + 0x21,
        0xE0..=0xF9 => 2 * (lead - 0xE0) + 0x4A,
        _ => return None,
    };
    let column = match trail {
        0x31..=0x7E => trail - 0x31,
        0x91..=0xFE => trail - 0x43,
        _ => return None,
    };
    // 1 つの先頭のバイトが、KS X 1001 の 2 つの行を持つ
    let (row, column) = if column < 0x5E {
        (row, column)
    } else {
        (row + 1, column - 0x5E)
    };
    // 4 行目の字母は、ハングルの範囲と `fixed` で表す
    (row != 0x24).then_some(Johab::Ks([row | 0x80, (column + 0x21) | 0x80]))
}

/// KS X 1001 の EUC-KR の 2 バイト `pair` を、Johab の記号と漢字の 2 バイトにする
fn johab_of_ks([first, second]: [u8; 2]) -> Option<[u8; 2]> {
    let row = first.checked_sub(0xA1)?;
    let column = second.checked_sub(0xA1)?;
    let (lead, odd) = match row {
        0..=11 => (0xD9 + row / 2, row % 2),
        0x29..=0x5C => (0xE0 + (row - 0x29) / 2, (row - 0x29) % 2),
        _ => return None,
    };
    if column >= 0x5E {
        return None;
    }
    let index = column + odd * 0x5E;
    Some([
        lead,
        if index < 0x4E {
            index + 0x31
        } else {
            index + 0x43
        },
    ])
}

/// Johab のハングルの範囲の 2 バイト。下の 15 ビットが、5 ビットずつの初声（1 は詰め物、2〜20）・中声（2 は詰め物、
/// 3〜7・10〜15・18〜23・26〜29）・終声（1 は詰め物、2〜17・19〜29）。3 つがそろうか終声だけがないものは音節に、
/// 1 つだけのものは結合用の字母に戻し、ほかの組み合わせは `None`
fn hangul(code: u16) -> Option<char> {
    let (first, middle, last) = ((code >> 10) & 0x1F, (code >> 5) & 0x1F, code & 0x1F);
    let initial = match first {
        1 => None,
        2..=20 => Some(first - 2),
        _ => return None,
    };
    let vowel = match middle {
        2 => None,
        3..=7 => Some(middle - 3),
        10..=15 => Some(middle - 5),
        18..=23 => Some(middle - 7),
        26..=29 => Some(middle - 9),
        _ => return None,
    };
    let last = match last {
        1 => 0,
        2..=17 => last - 1,
        19..=29 => last - 2,
        _ => return None,
    };
    let scalar = match (initial, vowel, last) {
        (Some(initial), Some(vowel), last) => 0xAC00 + (initial * 21 + vowel) * 28 + last,
        (Some(initial), None, 0) => 0x1100 + initial,
        (None, Some(vowel), 0) => 0x1161 + vowel,
        (None, None, 1..) => 0x11A7 + last,
        _ => return None,
    };
    char::from_u32(u32::from(scalar))
}

/// ハングルの音節と現代の結合用の字母 `c` を、Johab のハングルの範囲の 2 バイトにする
fn johab_of_hangul(c: char) -> Option<[u8; 2]> {
    let scalar = u32::from(c);
    let (initial, vowel, last) = match scalar {
        0xAC00..=0xD7A3 => {
            let index = scalar - 0xAC00;
            (Some(index / 588), Some(index / 28 % 21), index % 28)
        }
        0x1100..=0x1112 => (Some(scalar - 0x1100), None, 0),
        0x1161..=0x1175 => (None, Some(scalar - 0x1161), 0),
        0x11A8..=0x11C2 => (None, None, scalar - 0x11A7),
        _ => return None,
    };
    let first = initial.map_or(1, |initial| initial + 2);
    let middle = vowel.map_or(2, |vowel| match vowel {
        0..=4 => vowel + 3,
        5..=10 => vowel + 5,
        11..=16 => vowel + 7,
        _ => vowel + 9,
    });
    let last = if last <= 16 { last + 1 } else { last + 2 };
    let code = u16::try_from(0x8000 | first << 10 | middle << 5 | last).ok()?;
    Some(code.to_be_bytes())
}

impl Dbcs {
    /// `byte` が 2 バイトの文字の先頭のバイトか
    pub(super) fn leads(&self, byte: u8) -> bool {
        (self.lead.0..=self.lead.1).contains(&byte)
    }

    /// 2 バイト `lead`・`trail` が表す文字。表にない 2 バイトは `None`
    fn pair(&self, lead: u8, trail: u8) -> Option<char> {
        if !self.leads(lead) {
            return None;
        }
        let code = u16::from_be_bytes([lead, trail]);
        if let Some((_, c)) = self.fixed.iter().find(|(fixed, _)| *fixed == code) {
            return Some(*c);
        }
        if self
            .excluded
            .iter()
            .any(|(first, last)| (*first..=*last).contains(&code))
        {
            return None;
        }
        let pair = if self.johab {
            match johab(code)? {
                Johab::Hangul(c) => return Some(c),
                Johab::Ks(pair) => pair,
            }
        } else {
            [lead, trail]
        };
        let text = self
            .encoding
            .decode_without_bom_handling_and_without_replacement(&pair)?;
        let mut chars = text.chars();
        let c = chars.next()?;
        // 私用領域（外字）は、表に割り当てがない
        (chars.next().is_none() && !c.is_ascii() && !('\u{E000}'..='\u{F8FF}').contains(&c))
            .then_some(c)
    }

    /// バイト列を戻す。表にないバイトか、途中で切れた 2 バイトがあれば `None`
    pub(super) fn decode(&self, value: &[u8]) -> Option<String> {
        let mut text = String::with_capacity(value.len());
        let mut bytes = value.iter().copied();
        while let Some(byte) = bytes.next() {
            if byte.is_ascii() {
                text.push(char::from(byte));
            } else if let Some((_, c)) = self.single.filter(|(single, _)| *single == byte) {
                text.push(c);
            } else {
                text.push(self.pair(byte, bytes.next()?)?);
            }
        }
        Some(text)
    }

    /// 文字列を符号化する。表にない文字があれば `None`
    pub(super) fn encode(&self, text: &str) -> Option<Vec<u8>> {
        let mut bytes = Vec::with_capacity(text.len());
        for c in text.chars() {
            if let Ok(byte) = u8::try_from(c)
                && byte.is_ascii()
            {
                bytes.push(byte);
            } else if let Some((byte, _)) = self.single.filter(|(_, single)| *single == c) {
                bytes.push(byte);
            } else if let Some((code, _)) = self.fixed.iter().find(|(_, fixed)| *fixed == c) {
                bytes.extend(code.to_be_bytes());
            } else if self.johab
                && let Some(pair) = johab_of_hangul(c)
            {
                bytes.extend(pair);
            } else {
                let mut buffer = [0; 4];
                let (encoded, _, unmappable) = self.encoding.encode(c.encode_utf8(&mut buffer));
                // WHATWG の表だけにある 2 バイトにした文字は、表せない
                let [lead, trail] = *encoded else {
                    return None;
                };
                let [lead, trail] = if self.johab {
                    johab_of_ks([lead, trail])?
                } else {
                    [lead, trail]
                };
                if unmappable || self.pair(lead, trail) != Some(c) {
                    return None;
                }
                bytes.extend([lead, trail]);
            }
        }
        Some(bytes)
    }
}
