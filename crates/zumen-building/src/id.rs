//! 要素の ID（決定 0207）。GlobalId か、`i` と 10 進のインスタンスの番号。

use core::fmt;

/// GlobalId の 64 の文字。値の小さい順
const ALPHABET: &[u8; 64] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz_$";

/// IFC の GlobalId。128 ビットの値を、22 文字（最初の文字が上位の 2 ビット、残りが 6 ビットずつ）で書く
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GlobalId(u128);

impl GlobalId {
    /// 22 文字の GlobalId を読む。長さか文字が合わないか、最初の文字が `3` より大きければ `None`
    pub fn parse(text: &str) -> Option<Self> {
        let bytes = text.as_bytes();
        if bytes.len() != 22 {
            return None;
        }
        let mut value = 0u128;
        for (at, byte) in bytes.iter().enumerate() {
            let digit = ALPHABET.iter().position(|c| c == byte)?;
            if at == 0 && digit > 3 {
                return None;
            }
            value = (value << 6) | digit as u128;
        }
        Some(Self(value))
    }

    /// UUID の 16 バイト（RFC 9562 の並び）から作る
    pub fn from_uuid(bytes: [u8; 16]) -> Self {
        Self(u128::from_be_bytes(bytes))
    }

    /// UUID の 16 バイト（RFC 9562 の並び）
    pub fn to_uuid(self) -> [u8; 16] {
        self.0.to_be_bytes()
    }

    /// `8-4-4-4-12` の 16 進の UUID を読む。大文字と小文字を問わない
    pub fn parse_uuid(text: &str) -> Option<Self> {
        let groups: Vec<&str> = text.split('-').collect();
        let lengths: Vec<usize> = groups.iter().map(|g| g.len()).collect();
        if lengths != [8, 4, 4, 4, 12] {
            return None;
        }
        let digits: String = groups.concat();
        if !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        u128::from_str_radix(&digits, 16).ok().map(Self)
    }

    /// `8-4-4-4-12` の小文字の 16 進の UUID
    pub fn uuid_string(self) -> String {
        let hex = format!("{:032x}", self.0);
        let part = |range: core::ops::Range<usize>| hex.get(range).unwrap_or_default().to_owned();
        format!(
            "{}-{}-{}-{}-{}",
            part(0..8),
            part(8..12),
            part(12..16),
            part(16..20),
            part(20..32)
        )
    }
}

impl fmt::Display for GlobalId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for at in (0..22u32).rev() {
            let digit = (self.0 >> (6 * at)) & 63;
            let c = ALPHABET.get(digit as usize).copied().unwrap_or(b'0');
            write!(f, "{}", char::from(c))?;
        }
        Ok(())
    }
}

/// 要素の ID
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Id {
    /// GlobalId（22 文字）
    Global(GlobalId),
    /// GlobalId を持たないか使えない要素の、インスタンスの番号（`i120`。決定 0156 の SXF と同じ書き方）
    Instance(u64),
}

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Id::Global(id) => id.fmt(f),
            Id::Instance(number) => write!(f, "i{number}"),
        }
    }
}

impl core::str::FromStr for Id {
    type Err = ();

    /// GlobalId か、`i` と 10 進の番号を読む
    fn from_str(text: &str) -> Result<Self, ()> {
        if let Some(number) = text.strip_prefix('i')
            && !number.is_empty()
            && number.bytes().all(|b| b.is_ascii_digit())
            && (number == "0" || !number.starts_with('0'))
        {
            return number.parse().map(Id::Instance).map_err(|_| ());
        }
        GlobalId::parse(text).map(Id::Global).ok_or(())
    }
}
