//! 計測用の数値参照を、単一の元レコードの字句へ対応づける。
//!
//! 相互参照表は元の位置を公開しないため、オブジェクトストリームを含む文書と
//! 同じ番号・世代の重複定義は採用しない。全 xref の解決や数値の丸めは行わない。

use hayro_syntax::object::{Object, ObjectIdentifier};
use hayro_syntax::reader::{Reader, ReaderExt};
use zumen_base::{Cx, Kind, Stop};

use crate::{PdfFile, RecordKind};

#[derive(Clone, Copy)]
pub(crate) struct Source<'a> {
    file: &'a PdfFile,
    has_object_streams: bool,
}

impl<'a> Source<'a> {
    pub(crate) fn new(file: &'a PdfFile, has_object_streams: bool) -> Self {
        Self {
            file,
            has_object_streams,
        }
    }

    /// 単一の間接オブジェクトの、空白・コメントを含まない十進数の元字句。
    pub(crate) fn raw(
        self,
        id: ObjectIdentifier,
        cx: &mut Cx<'_>,
    ) -> Result<Option<&'a [u8]>, Stop> {
        cx.check(Kind::Work, 0)?;
        if self.has_object_streams {
            return Ok(None);
        }
        let mut found = None;
        for record in self.file.records() {
            cx.check(Kind::Work, 1)?;
            if record.kind == RecordKind::Object
                && record.object.is_some_and(|(number, generation)| {
                    number == id.obj_number && i32::from(generation) == id.gen_number
                })
            {
                if found.is_some() {
                    return Ok(None);
                }
                found = Some(record);
            }
        }
        let Some(record) = found else {
            return Ok(None);
        };
        let bytes = self.file.bytes(record.start, record.end);
        // ヘッダー・空白・コメント・値・末尾を読む前に払う。数値字句の確認でもう一度
        // たどる分を含み、辞書・配列・文字列は展開も再帰した読み飛ばしもしない。
        cx.check(Kind::Work, (bytes.len() as u64).saturating_mul(3))?;
        let mut reader = Reader::new(bytes);
        reader.skip_white_spaces_and_comments();
        if reader.read_without_context::<ObjectIdentifier>() != Some(id)
            || !reader.peek_byte().is_some_and(separator)
        {
            return Ok(None);
        }
        reader.skip_white_spaces_and_comments();
        if !matches!(reader.peek_byte(), Some(b'.' | b'+' | b'-' | b'0'..=b'9')) {
            return Ok(None);
        }
        let Some(raw) = reader.skip::<Object<'_>>(false) else {
            return Ok(None);
        };
        if !decimal(raw) {
            return Ok(None);
        }
        reader.skip_white_spaces_and_comments();
        if reader.forward_tag(b"endobj").is_none() {
            return Ok(None);
        }
        reader.skip_white_spaces_and_comments();
        cx.check(Kind::Work, 0)?;
        Ok(reader.at_end().then_some(raw))
    }
}

/// ObjectIdentifier の obj は prefix 一致なので、直後の字句境界も確認する。
fn separator(byte: u8) -> bool {
    matches!(
        byte,
        0 | b'\t'
            | b'\n'
            | 0x0c
            | b'\r'
            | b' '
            | b'('
            | b')'
            | b'<'
            | b'>'
            | b'['
            | b']'
            | b'{'
            | b'}'
            | b'/'
            | b'%'
    )
}

/// hayro の寛容な Number::skip が認める単独の符号や末尾の - を採用しない。
fn decimal(raw: &[u8]) -> bool {
    let raw = match raw.first() {
        Some(b'+' | b'-') => raw.get(1..).unwrap_or_default(),
        _ => raw,
    };
    let mut dot = false;
    let mut digit = false;
    for byte in raw {
        match byte {
            b'0'..=b'9' => digit = true,
            b'.' if !dot => dot = true,
            _ => return false,
        }
    }
    digit
}

#[cfg(test)]
mod tests {
    use super::*;
    use zumen_base::{Budget, CancelToken, Diagnostics, Limit, Limits};
    use zumen_sdk::testkit;

    fn file(source: &[u8]) -> PdfFile {
        testkit::read::<crate::Pdf>(source).0.unwrap()
    }

    fn get(source: Source<'_>, id: ObjectIdentifier) -> Option<&[u8]> {
        let mut budget = Budget::default();
        let mut diagnostics = Diagnostics::new();
        let cancel = CancelToken::new();
        source
            .raw(id, &mut Cx::new(&mut budget, &mut diagnostics, &cancel))
            .unwrap()
    }

    #[test]
    fn borrows_the_complete_decimal_from_a_unique_matching_generation() {
        let file = file(b"%PDF-1.7\n5 0 obj 0.001390000000000000000000000000 endobj\n5 1 obj -.25 endobj\n%%EOF");
        let source = Source::new(&file, false);
        let raw = get(source, ObjectIdentifier::new(5, 0)).unwrap();
        assert_eq!(raw, b"0.001390000000000000000000000000");
        assert!(file.src.as_ptr_range().contains(&raw.as_ptr()));
        assert_eq!(get(source, ObjectIdentifier::new(5, 1)), Some(&b"-.25"[..]));
        assert_eq!(get(source, ObjectIdentifier::new(5, 2)), None);
        assert_eq!(get(source, ObjectIdentifier::new(99, 0)), None);
        assert_eq!(get(source, ObjectIdentifier::new(5, -1)), None);
    }

    #[test]
    fn declines_every_duplicate_definition_and_object_stream_files() {
        let file =
            file(b"%PDF-1.7\n5 0 obj 1.25 endobj\n5 0 obj 1.25 endobj\n5 1 obj 2.5 endobj\n%%EOF");
        assert_eq!(
            get(Source::new(&file, false), ObjectIdentifier::new(5, 0)),
            None
        );
        assert_eq!(
            get(Source::new(&file, false), ObjectIdentifier::new(5, 1)),
            Some(&b"2.5"[..])
        );
        assert_eq!(
            get(Source::new(&file, true), ObjectIdentifier::new(5, 1)),
            None
        );
    }

    #[test]
    fn separates_comments_and_whitespace_from_the_number() {
        let file =
            file(b"%PDF-1.7\n5 0 obj % before\r\n% next\n\t+000.00139% after\nendobj\n%%EOF");
        assert_eq!(
            get(Source::new(&file, false), ObjectIdentifier::new(5, 0)),
            Some(&b"+000.00139"[..])
        );
    }

    #[test]
    fn declines_non_numbers_malformed_tokens_and_mismatched_headers() {
        for value in [
            "null",
            "true",
            "/Name",
            "(1.25)",
            "<3132>",
            "[1.25]",
            "<< /C 1.25 >>",
            "4 0 R",
            "1.25 extra",
            "1.25x",
            "-.",
            "+",
            ".",
            "1-",
            "1.2.3",
            "",
        ] {
            let file = file(format!("%PDF-1.7\n5 0 obj {value} endobj\n%%EOF").as_bytes());
            assert_eq!(
                get(Source::new(&file, false), ObjectIdentifier::new(5, 0)),
                None,
                "{value}"
            );
        }
        let mut file = file(b"%PDF-1.7\n5 0 obj 1.25 endobj\n%%EOF");
        let record = file
            .records
            .iter_mut()
            .find(|r| r.object.is_some())
            .unwrap();
        record.object = Some((6, 0));
        assert_eq!(
            get(Source::new(&file, false), ObjectIdentifier::new(6, 0)),
            None
        );
    }

    #[test]
    fn reserves_work_before_scanning_and_skipping_and_observes_cancellation() {
        let file = file(b"%PDF-1.7\n1 0 obj (unrelated) endobj\n5 0 obj 0.001390000000000000000000000000 endobj\n%%EOF");
        let source = Source::new(&file, false);
        let id = ObjectIdentifier::new(5, 0);
        let cancel = CancelToken::new();
        let mut budget = Budget::default();
        let mut diagnostics = Diagnostics::new();
        let expected = source
            .raw(id, &mut Cx::new(&mut budget, &mut diagnostics, &cancel))
            .unwrap();
        assert!(expected.is_some());
        assert_eq!(budget.used(Kind::DecodedBytes), 0);
        let used = budget.used(Kind::Work);
        for limit in [0, file.records().len() as u64, used - 1, used] {
            let mut budget = Budget::new(Limits::default().with(Kind::Work, Limit::Finite(limit)));
            let result = source.raw(id, &mut Cx::new(&mut budget, &mut diagnostics, &cancel));
            if limit == used {
                assert_eq!(result.unwrap(), expected);
            } else {
                assert!(result.is_err(), "{limit}");
            }
        }
        cancel.cancel();
        for has_object_streams in [false, true] {
            let mut budget = Budget::default();
            assert!(
                Source::new(&file, has_object_streams)
                    .raw(id, &mut Cx::new(&mut budget, &mut diagnostics, &cancel))
                    .is_err()
            );
        }
        assert_eq!(get(source, id), expected);
    }
}
