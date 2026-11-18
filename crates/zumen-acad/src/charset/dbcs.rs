//! Windows の 2 バイトのコード・ページ（ANSI_936・ANSI_949・ANSI_950、#327）。
//!
//! 文字の表は Unicode の `MAPPINGS/VENDORS/MICSFT/WINDOWS/CP<番号>.TXT` による。この表は、Windows の
//! MultiByteToWideChar が私用領域と C1 制御文字に当てるバイトを除くと、それと同じ。表を写して持たず、WHATWG の
//! GBK・EUC-KR・Big5（encoding_rs）で 2 バイトを戻し、表にない範囲を除き、違う所を直して同じ表にする。
//! WHATWG の表は、CP949 とは同じで、CP936・CP950 の表を含み、私用領域・香港の追加字（HKSCS）・GB 18030 の
//! 追加の文字の分だけ広い。除く範囲と直す所は、Microsoft の表と encoding_rs 0.8.35 の全 2 バイトを比べて決めた
//! （検証キットの `codepage/` が、Windows の API・ezdxf・Python の codec に照らす）。

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
};

/// CP949（韓国語、統合型ハングル）。WHATWG の EUC-KR と同じ
const CP949: Dbcs = Dbcs {
    encoding: encoding_rs::EUC_KR,
    lead: (0x81, 0xFD),
    single: None,
    excluded: &[],
    fixed: &[],
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
};

/// コード・ページ `page` の表。2 バイトのコード・ページでない番号は `None`
pub(super) fn table(page: u16) -> Option<&'static Dbcs> {
    match page {
        936 => Some(&CP936),
        949 => Some(&CP949),
        950 => Some(&CP950),
        _ => None,
    }
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
        let pair = [lead, trail];
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
            } else {
                let mut buffer = [0; 4];
                let (encoded, _, unmappable) = self.encoding.encode(c.encode_utf8(&mut buffer));
                // WHATWG の表だけにある 2 バイトにした文字は、表せない
                let [lead, trail] = *encoded else {
                    return None;
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
