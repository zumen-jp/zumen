//! XML の文書の先頭から、最初の要素の名前と名前空間を読む（[`crate::Signature::XmlRoot`]）。
//!
//! 判定は先頭のバイト列だけを見るので、XML 宣言・注記・処理命令・文書型宣言を飛ばし、最初の開始タグの名前と、
//! そのタグが宣言する名前空間だけを読む。タグが先頭のバイト列の中で終わらなければ、合わないとする。

/// 最初の要素の、接頭辞を除いた名前と、その名前空間の名前（宣言の値のバイト列）
pub(crate) fn root(head: &[u8]) -> Option<(&[u8], Option<&[u8]>)> {
    let mut rest = head.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(head);
    loop {
        rest = rest.trim_ascii_start();
        if let Some(after) = rest.strip_prefix(b"<?") {
            rest = after_pattern(after, b"?>")?;
        } else if let Some(after) = rest.strip_prefix(b"<!--") {
            rest = after_pattern(after, b"-->")?;
        } else if let Some(after) = rest.strip_prefix(b"<!DOCTYPE") {
            rest = after_doctype(after)?;
        } else {
            return start_tag(rest.strip_prefix(b"<")?);
        }
    }
}

fn after_pattern<'h>(rest: &'h [u8], pattern: &[u8]) -> Option<&'h [u8]> {
    let at = rest
        .windows(pattern.len())
        .position(|window| window == pattern)?;
    rest.get(at + pattern.len()..)
}

fn after_doctype(mut rest: &[u8]) -> Option<&[u8]> {
    let mut depth = 0usize;
    loop {
        let (&byte, after) = rest.split_first()?;
        rest = match byte {
            b'"' | b'\'' => {
                let end = after.iter().position(|b| *b == byte)?;
                after.get(end + 1..)?
            }
            b'[' => {
                depth += 1;
                after
            }
            b']' => {
                depth = depth.checked_sub(1)?;
                after
            }
            b'>' if depth == 0 => return Some(after),
            _ => after,
        };
    }
}

fn is_name(byte: u8) -> bool {
    !byte.is_ascii_whitespace()
        && !matches!(
            byte,
            b'<' | b'>' | b'/' | b'=' | b'!' | b'?' | b'"' | b'\'' | b'&' | b';' | b'[' | b']'
        )
}

fn start_tag(rest: &[u8]) -> Option<(&[u8], Option<&[u8]>)> {
    let end = rest.iter().position(|b| !is_name(*b))?;
    let (name, mut rest) = rest.split_at(end);
    let (prefix, local) = match name.iter().position(|b| *b == b':') {
        Some(colon) => (name.get(..colon), name.get(colon + 1..)?),
        None => (None, name),
    };
    if local.is_empty() {
        return None;
    }
    let mut namespace = None;
    loop {
        rest = rest.trim_ascii_start();
        if rest.starts_with(b">") || rest.starts_with(b"/>") {
            return Some((local, namespace));
        }
        let end = rest.iter().position(|b| !is_name(*b))?;
        let (attribute, after) = rest.split_at(end);
        let after = after
            .trim_ascii_start()
            .strip_prefix(b"=")?
            .trim_ascii_start();
        let (&quote, after) = after.split_first()?;
        if !matches!(quote, b'"' | b'\'') {
            return None;
        }
        let end = after.iter().position(|b| *b == quote)?;
        let (value, after) = after.split_at(end);
        let declares = match prefix {
            None => attribute == b"xmlns",
            Some(prefix) => attribute.strip_prefix(b"xmlns:") == Some(prefix),
        };
        if declares && !value.is_empty() {
            namespace = Some(value);
        }
        rest = after.get(1..)?;
    }
}
