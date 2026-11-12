//! 計測用の数値参照を、アクティブな相互参照表の元字句へ対応づける。
//!
//! [`crate::xref::ActiveXref`] が指す通常のオフセット、または展開済みオブジェクトストリーム（`ObjStm`）内の
//! メンバー位置から、丸める前の十進数の元字句を取り出す。重複定義を含む文書でもアクティブな世代・位置だけを採用し、
//! 後続の余分な字句を含む壊れた項目は採用しない。

use std::borrow::Cow;
use std::collections::BTreeMap;

use hayro_syntax::object::{Object, ObjectIdentifier};
use hayro_syntax::reader::{Reader, ReaderExt};
use zumen_base::{Cx, Kind, Stop};

use crate::xref::{ActiveEntry, ActiveXref, DecodedObjectStream, decode_object_stream_at};
use crate::{PdfFile, Record, RecordKind, inflate};

const CACHE_ENTRY_BYTES: u64 = 64;

pub(crate) struct Source<'a> {
    file: &'a PdfFile,
    active_xref: &'a ActiveXref,
    streams: inflate::Source<'a>,
    decoded_streams: BTreeMap<u32, Option<DecodedObjectStream>>,
}

impl<'a> Source<'a> {
    pub(crate) fn new(
        file: &'a PdfFile,
        active_xref: &'a ActiveXref,
        streams: inflate::Source<'a>,
    ) -> Self {
        Self {
            file,
            active_xref,
            streams,
            decoded_streams: BTreeMap::new(),
        }
    }

    /// アクティブな間接オブジェクトの、空白・コメントを含まない十進数の元字句。
    pub(crate) fn raw(
        &mut self,
        id: ObjectIdentifier,
        cx: &mut Cx<'_>,
    ) -> Result<Option<Cow<'a, [u8]>>, Stop> {
        cx.check(Kind::Work, 1)?;
        if self.active_xref.is_empty() {
            return self.raw_unique_record(id, cx);
        }
        let Some(entry) = self.active_xref.get(id) else {
            return Ok(None);
        };
        match entry {
            ActiveEntry::Normal(offset) => self.raw_normal(id, offset, cx),
            ActiveEntry::ObjStream { container, index }
                if id.gen_number == 0 && container != id.obj_number =>
            {
                self.raw_obj_stream(id.obj_number, container, index, cx)
            }
            ActiveEntry::ObjStream { .. } => Ok(None),
        }
    }

    fn raw_unique_record(
        &self,
        id: ObjectIdentifier,
        cx: &mut Cx<'_>,
    ) -> Result<Option<Cow<'a, [u8]>>, Stop> {
        let Ok(expected_gen) = u16::try_from(id.gen_number) else {
            return Ok(None);
        };
        let mut found = None;
        for record in self.file.records() {
            if record.kind == RecordKind::Object
                && record.object == Some((id.obj_number, expected_gen))
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
        let Ok(offset) = usize::try_from(record.start) else {
            return Ok(None);
        };
        self.raw_normal(id, offset, cx)
    }

    fn raw_normal(
        &self,
        id: ObjectIdentifier,
        offset: usize,
        cx: &mut Cx<'_>,
    ) -> Result<Option<Cow<'a, [u8]>>, Stop> {
        let Some(record) = record_containing(self.file, offset) else {
            return Ok(None);
        };
        let Ok(expected_gen) = u16::try_from(id.gen_number) else {
            return Ok(None);
        };
        if record.kind != RecordKind::Object || record.object != Some((id.obj_number, expected_gen))
        {
            return Ok(None);
        }
        let end = usize::try_from(record.end).unwrap_or(self.file.src.len());
        let Some(bytes) = self.file.src.get(offset..end) else {
            return Ok(None);
        };
        // ヘッダー・空白・コメント・値・末尾を読む前に払う。数値字句の確認でもう一度
        // たどる分を含み、辞書・配列・文字列は展開も再帰した読み飛ばしもしない。
        cx.check(
            Kind::Work,
            (bytes.len() as u64).saturating_mul(3).saturating_add(1),
        )?;
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
        Ok(reader.at_end().then_some(Cow::Borrowed(raw)))
    }

    fn raw_obj_stream(
        &mut self,
        obj_number: u32,
        container: u32,
        index: u32,
        cx: &mut Cx<'_>,
    ) -> Result<Option<Cow<'a, [u8]>>, Stop> {
        if !self.decoded_streams.contains_key(&container) {
            let Some(ActiveEntry::Normal(container_offset)) =
                self.active_xref.get(ObjectIdentifier::new(container, 0))
            else {
                return Ok(None);
            };
            let decoded = decode_object_stream_at(
                &self.file.src,
                container,
                container_offset,
                self.streams,
                cx,
            )?;
            cx.check(Kind::DecodedBytes, CACHE_ENTRY_BYTES)?;
            self.decoded_streams.insert(container, decoded);
        }
        let Some(Some(stream)) = self.decoded_streams.get(&container) else {
            return Ok(None);
        };
        let Ok(slot) = usize::try_from(index) else {
            return Ok(None);
        };
        let Some(&(member_num, Some(start))) = stream.offsets.get(slot) else {
            return Ok(None);
        };
        if member_num != obj_number {
            return Ok(None);
        }
        let end = stream
            .offsets
            .get(slot + 1..)
            .and_then(|rest| rest.iter().find_map(|&(_, next)| next))
            .unwrap_or(stream.data.len());
        let Some(slice) = stream.data.get(start..end) else {
            return Ok(None);
        };
        cx.check(
            Kind::Work,
            (slice.len() as u64).saturating_mul(3).saturating_add(1),
        )?;
        let mut reader = Reader::new(slice);
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
        if !reader.at_end() {
            return Ok(None);
        }
        cx.check(Kind::DecodedBytes, (raw.len() as u64).saturating_add(16))?;
        cx.check(Kind::Work, 0)?;
        Ok(Some(Cow::Owned(raw.to_vec())))
    }
}

fn record_containing(file: &PdfFile, offset: usize) -> Option<&Record> {
    let at = offset as u64;
    let records = file.records();
    let idx = records.partition_point(|record| record.end <= at);
    records.get(idx).filter(|record| record.start <= at)
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

    fn build_xref(file: &PdfFile, followed_table: bool, has_object_streams: bool) -> ActiveXref {
        let mut budget = Budget::default();
        let mut diagnostics = Diagnostics::new();
        let cancel = CancelToken::new();
        let streams = inflate::Source::new(&file.src, false, file.src.len() as u64);
        ActiveXref::build(
            file,
            followed_table,
            has_object_streams,
            streams,
            &mut Cx::new(&mut budget, &mut diagnostics, &cancel),
        )
        .unwrap()
    }

    fn get<'a>(source: &mut Source<'a>, id: ObjectIdentifier) -> Option<Cow<'a, [u8]>> {
        let mut budget = Budget::default();
        let mut diagnostics = Diagnostics::new();
        let cancel = CancelToken::new();
        source
            .raw(id, &mut Cx::new(&mut budget, &mut diagnostics, &cancel))
            .unwrap()
    }

    #[test]
    fn borrows_the_complete_decimal_from_a_unique_matching_generation() {
        let file = file(
            b"%PDF-1.7\n5 0 obj 0.001390000000000000000000000000 endobj\n5 1 obj -.25 endobj\n%%EOF",
        );
        let xref = build_xref(&file, false, false);
        let streams = inflate::Source::new(&file.src, false, file.src.len() as u64);
        let mut source = Source::new(&file, &xref, streams);
        let raw = get(&mut source, ObjectIdentifier::new(5, 0)).unwrap();
        assert_eq!(raw.as_ref(), b"0.001390000000000000000000000000");
        assert!(
            matches!(raw, Cow::Borrowed(slice) if file.src.as_ptr_range().contains(&slice.as_ptr()))
        );
        assert_eq!(
            get(&mut source, ObjectIdentifier::new(5, 1)).as_deref(),
            Some(&b"-.25"[..])
        );
        assert_eq!(get(&mut source, ObjectIdentifier::new(5, 2)), None);
        assert_eq!(get(&mut source, ObjectIdentifier::new(99, 0)), None);
        assert_eq!(get(&mut source, ObjectIdentifier::new(5, -1)), None);
    }

    #[test]
    fn resolves_active_duplicate_definition_and_compressed_object_stream_members() {
        let file_dup =
            file(b"%PDF-1.7\n5 0 obj 1.25 endobj\n5 0 obj 3.75 endobj\n5 1 obj 2.5 endobj\n%%EOF");
        let xref_dup = build_xref(&file_dup, false, false);
        let streams_dup = inflate::Source::new(&file_dup.src, false, file_dup.src.len() as u64);
        let mut source_dup = Source::new(&file_dup, &xref_dup, streams_dup);
        assert_eq!(
            get(&mut source_dup, ObjectIdentifier::new(5, 0)).as_deref(),
            Some(&b"3.75"[..])
        );
        assert_eq!(
            get(&mut source_dup, ObjectIdentifier::new(5, 1)).as_deref(),
            Some(&b"2.5"[..])
        );

        let objstm_body = b"5 0 6 33 0.001390000000000000000000000000 1.25 extra";
        let pdf_objstm = format!(
            "%PDF-1.7\n10 0 obj\n<< /Type /ObjStm /N 2 /First 9 /Length {} >>\nstream\n",
            objstm_body.len()
        );
        let mut raw_pdf = pdf_objstm.into_bytes();
        raw_pdf.extend_from_slice(objstm_body);
        raw_pdf.extend_from_slice(b"\nendstream\nendobj\n%%EOF");
        let file_stm = file(&raw_pdf);
        let xref_stm = build_xref(&file_stm, false, true);
        let streams_stm = inflate::Source::new(&file_stm.src, false, 256);
        let mut source_stm = Source::new(&file_stm, &xref_stm, streams_stm);
        assert_eq!(
            get(&mut source_stm, ObjectIdentifier::new(5, 0)).as_deref(),
            Some(&b"0.001390000000000000000000000000"[..])
        );
        assert_eq!(get(&mut source_stm, ObjectIdentifier::new(6, 0)), None);
    }

    #[test]
    fn separates_comments_and_whitespace_from_the_number() {
        let file =
            file(b"%PDF-1.7\n5 0 obj % before\r\n% next\n\t+000.00139% after\nendobj\n%%EOF");
        let xref = build_xref(&file, false, false);
        let streams = inflate::Source::new(&file.src, false, file.src.len() as u64);
        let mut source = Source::new(&file, &xref, streams);
        assert_eq!(
            get(&mut source, ObjectIdentifier::new(5, 0)).as_deref(),
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
            let xref = build_xref(&file, false, false);
            let streams = inflate::Source::new(&file.src, false, file.src.len() as u64);
            let mut source = Source::new(&file, &xref, streams);
            assert_eq!(
                get(&mut source, ObjectIdentifier::new(5, 0)),
                None,
                "{value}"
            );
        }
        let mut file = file(b"%PDF-1.7\n5 0 obj 1.25 endobj\n%%EOF");
        let xref = build_xref(&file, false, false);
        let record = file
            .records
            .iter_mut()
            .find(|r| r.object.is_some())
            .unwrap();
        record.object = Some((6, 0));
        let streams = inflate::Source::new(&file.src, false, file.src.len() as u64);
        let mut source = Source::new(&file, &xref, streams);
        assert_eq!(get(&mut source, ObjectIdentifier::new(5, 0)), None);
        assert_eq!(get(&mut source, ObjectIdentifier::new(6, 0)), None);
    }

    #[test]
    fn reserves_work_before_scanning_and_skipping_and_observes_cancellation() {
        let file = file(
            b"%PDF-1.7\n1 0 obj (unrelated) endobj\n5 0 obj 0.001390000000000000000000000000 endobj\n%%EOF",
        );
        let xref = build_xref(&file, false, false);
        let streams = inflate::Source::new(&file.src, false, file.src.len() as u64);
        let id = ObjectIdentifier::new(5, 0);
        let cancel = CancelToken::new();
        let mut budget = Budget::default();
        let mut diagnostics = Diagnostics::new();
        let mut source = Source::new(&file, &xref, streams);
        let expected = source
            .raw(id, &mut Cx::new(&mut budget, &mut diagnostics, &cancel))
            .unwrap();
        assert!(expected.is_some());
        assert_eq!(budget.used(Kind::DecodedBytes), 0);
        let used = budget.used(Kind::Work);
        for limit in [0, 1, used - 1, used] {
            let mut budget = Budget::new(Limits::default().with(Kind::Work, Limit::Finite(limit)));
            let mut source = Source::new(&file, &xref, streams);
            let result = source.raw(id, &mut Cx::new(&mut budget, &mut diagnostics, &cancel));
            if limit == used {
                assert_eq!(result.unwrap(), expected);
            } else {
                assert!(result.is_err(), "{limit}");
            }
        }
        cancel.cancel();
        let mut budget = Budget::default();
        let mut source = Source::new(&file, &xref, streams);
        assert!(
            source
                .raw(id, &mut Cx::new(&mut budget, &mut diagnostics, &cancel))
                .is_err()
        );
        assert_eq!(budget.used(Kind::Work), 0);
        assert_eq!(budget.used(Kind::DecodedBytes), 0);
    }
}
