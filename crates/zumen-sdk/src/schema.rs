//! ISO 10303-21 の交換構造の先頭から、見出しの `FILE_SCHEMA` の最初のスキーマの名前を読む
//! （[`crate::Signature::StepSchema`]）。
//!
//! 判定は先頭のバイト列だけを見るので、見出しの節の文を、文字列とコメントを飛ばしながら読む。`FILE_SCHEMA` の名前が
//! 先頭のバイト列の中で終わらないか、`DATA;` の節が先に始まれば、合わないとする。

/// 最初のスキーマの名前（引用符の内側）
pub(crate) fn schema(head: &[u8]) -> Option<&[u8]> {
    let mut rest = head.trim_ascii_start().strip_prefix(b"ISO-10303-21;")?;
    loop {
        rest = skip(rest)?;
        let (&byte, after) = rest.split_first()?;
        if byte == b'\'' {
            rest = after_string(after)?;
        } else if byte.is_ascii_alphabetic() || byte == b'_' {
            let end = rest
                .iter()
                .position(|b| !(b.is_ascii_alphanumeric() || *b == b'_' || *b == b'-'))
                .unwrap_or(rest.len());
            let word = rest.get(..end)?;
            rest = rest.get(end..)?;
            if word.eq_ignore_ascii_case(b"FILE_SCHEMA") {
                return first_name(rest);
            }
            if word.eq_ignore_ascii_case(b"DATA") || word.eq_ignore_ascii_case(b"ENDSEC") {
                return None;
            }
        } else {
            rest = after;
        }
    }
}

/// 空白とコメントを飛ばす。コメントが終わらなければ `None`
fn skip(mut rest: &[u8]) -> Option<&[u8]> {
    loop {
        rest = rest.trim_ascii_start();
        let Some(after) = rest.strip_prefix(b"/*") else {
            return Some(rest);
        };
        let end = after.windows(2).position(|window| window == b"*/")?;
        rest = after.get(end + 2..)?;
    }
}

/// 文字列の残りを飛ばす。`''` は引用符 1 つ
fn after_string(mut rest: &[u8]) -> Option<&[u8]> {
    loop {
        let end = rest.iter().position(|b| *b == b'\'')?;
        rest = rest.get(end + 1..)?;
        match rest.strip_prefix(b"'") {
            Some(after) => rest = after,
            None => return Some(rest),
        }
    }
}

/// `(('NAME' …` の `NAME`
fn first_name(rest: &[u8]) -> Option<&[u8]> {
    let rest = skip(rest)?.strip_prefix(b"(")?;
    let rest = skip(rest)?.strip_prefix(b"(")?;
    let rest = skip(rest)?.strip_prefix(b"'")?;
    let end = rest.iter().position(|b| *b == b'\'')?;
    rest.get(..end)
}
