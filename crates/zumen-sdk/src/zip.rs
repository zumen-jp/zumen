//! ZIP の書庫の中央ディレクトリから、項目の名前を読む（[`crate::Signature::ZipEntry`]）。
//!
//! 判定は入力の全体を受けるときに限って働く。終わりの記録が見つからないか、中央ディレクトリが入力の中に収まらなければ、
//! 名前はないとする。ZIP64 の書庫は読まない。

/// 終わりの記録（end of central directory）の署名
const END: &[u8] = b"PK\x05\x06";
/// 中央ディレクトリの項目の署名
const ENTRY: &[u8] = b"PK\x01\x02";
/// 終わりの記録の、注記を除いた大きさ
const END_SIZE: usize = 22;
/// 注記の最大の大きさ
const COMMENT_MAX: usize = 0xFFFF;

fn u16_at(bytes: &[u8], at: usize) -> Option<usize> {
    let b = bytes.get(at..at.checked_add(2)?)?;
    Some(usize::from(u16::from_le_bytes(b.try_into().ok()?)))
}

fn u32_at(bytes: &[u8], at: usize) -> Option<usize> {
    let b = bytes.get(at..at.checked_add(4)?)?;
    usize::try_from(u32::from_le_bytes(b.try_into().ok()?)).ok()
}

/// 項目の名前のどれかが `pred` に合うか
pub(crate) fn any_name(bytes: &[u8], mut pred: impl FnMut(&[u8]) -> bool) -> bool {
    let Some(last) = bytes.len().checked_sub(END_SIZE) else {
        return false;
    };
    let first = last.saturating_sub(COMMENT_MAX);
    let Some(end) = (first..=last)
        .rev()
        .find(|&at| bytes.get(at..at + END.len()) == Some(END))
    else {
        return false;
    };
    let (Some(count), Some(mut at)) = (u16_at(bytes, end + 10), u32_at(bytes, end + 16)) else {
        return false;
    };
    for _ in 0..count {
        if bytes.get(at..at.saturating_add(ENTRY.len())) != Some(ENTRY) {
            return false;
        }
        let (Some(name), Some(extra), Some(comment)) = (
            u16_at(bytes, at + 28),
            u16_at(bytes, at + 30),
            u16_at(bytes, at + 32),
        ) else {
            return false;
        };
        let start = at + 46;
        let Some(found) = bytes.get(start..start + name) else {
            return false;
        };
        if pred(found) {
            return true;
        }
        at = start + name + extra + comment;
    }
    false
}
