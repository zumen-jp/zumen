//! hayro-syntax 0.9.0 が現実に使っている相互参照表の決定的な復元。
//!
//! [`crate::walk::settle`] が終わった後の文書では、`XRef` が元の相互参照（表・`Prev`・`XRefStm`・相互参照ストリーム）を
//! 保っているか、ファイルの字句走査（`fallback_xref_map_inner`）へ作り直したかが確定している。公開 API は表の項目や
//! オフセットを返さないため、同じ規則でアクティブな対応表（オブジェクト番号・世代 → 元バイト位置または `ObjStm` と
//! 添字）を `BTreeMap` に組み、重複定義や `ObjStm` を含む文書でも出所と数値字句の参照先を一意に特定する。

use std::collections::{BTreeMap, BTreeSet};

use hayro_syntax::object::Object;
use hayro_syntax::object::{Array, Dict, Name, ObjectIdentifier, Stream};
use hayro_syntax::reader::{Reader, ReaderContext, ReaderExt};
use zumen_base::{Cx, Kind, Ref, Stop};

use crate::{PdfFile, inflate};

const XREF_ENTRY_LEN: usize = 20;
const MAX_XREF_CHAIN_DEPTH: usize = 256;
/// `BTreeMap<(u32, i32), ActiveEntry>` の 1 項目あたりの論理量。
const ENTRY_BYTES: u64 = 64;
/// 展開済み `ObjStm` のオフセット表 1 項目あたりの論理量。
const MEMBER_OFFSET_BYTES: u64 = 32;
/// `Dict` の初期表と、キー 1 つあたりの論理量。
const DICT_BASE_BYTES: u64 = 256;
const DICT_KEY_BYTES: u64 = 192;
/// バイト列を走査する 4 KiB ごとの `Work` の単位。
const SCAN_BLOCK_SHIFT: u32 = 12;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ActiveEntry {
    /// 元のファイルバイト列の先頭からのオフセット。
    Normal(usize),
    /// オブジェクトストリーム（世代は常に 0）のオブジェクト番号と、その中の添字。
    ObjStream { container: u32, index: u32 },
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ActiveXref {
    entries: BTreeMap<(u32, i32), ActiveEntry>,
}

type MemberOffsets = Vec<(u32, Option<usize>)>;

pub(crate) struct DecodedObjectStream {
    pub(crate) data: Vec<u8>,
    pub(crate) offsets: MemberOffsets,
}

impl ActiveXref {
    /// `settle` 後の `followed_table` 状態に合わせて、アクティブな相互参照表を組み立てる。
    ///
    /// `ObjStm` を含まず、各 `(番号, 世代)` の定義が L1 レコードに高々 1 回しか現れない文書では、L1 レコードだけで
    /// 出所も一意の直接オブジェクトのオフセットも求まるため、表の再構築を省く。
    pub(crate) fn build(
        file: &PdfFile,
        followed_table: bool,
        has_object_streams: bool,
        streams: inflate::Source<'_>,
        cx: &mut Cx<'_>,
    ) -> Result<Self, Stop> {
        cx.check(Kind::Work, 0)?;
        if !has_object_streams && !has_duplicate_object_records(file) {
            return Ok(Self::default());
        }
        let source = &file.src;
        let mut entries = BTreeMap::new();
        if followed_table
            && let Some(pos) = find_last_xref_pos(source, cx)?
            && populate_xref(source, pos, streams, &mut entries, cx)?.is_some()
        {
            return Ok(Self { entries });
        }
        entries.clear();
        populate_fallback(source, has_object_streams, streams, &mut entries, cx)?;
        Ok(Self { entries })
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub(crate) fn get(&self, id: ObjectIdentifier) -> Option<ActiveEntry> {
        self.entries.get(&(id.obj_number, id.gen_number)).copied()
    }

    /// 間接オブジェクトの番号・世代 → L1 レコードの出所。
    ///
    /// アクティブな相互参照表に載る項目は、そのオフセット（または格納先の `ObjStm` のオフセット）が属する L1 レコードを
    /// 指す。表に載らない番号だけ、L1 レコードの定義順の最後の位置で補う。
    pub(crate) fn objects(
        &self,
        file: &PdfFile,
        header: &Ref,
        place_at: impl Fn(&PdfFile, usize, &Ref) -> Ref,
        record_at: impl Fn(&PdfFile, usize) -> Ref,
        cx: &mut Cx<'_>,
    ) -> Result<BTreeMap<(u32, i32), Ref>, Stop> {
        let mut out = BTreeMap::new();
        for (index, record) in file.records().iter().enumerate() {
            if let Some((number, generation)) = record.object {
                out.insert((number, i32::from(generation)), record_at(file, index));
            }
        }
        if self.entries.is_empty() {
            return Ok(out);
        }
        for (&(number, generation), &entry) in &self.entries {
            cx.check(Kind::Work, 1)?;
            let offset = match entry {
                ActiveEntry::Normal(offset) => Some(offset),
                ActiveEntry::ObjStream { container, .. }
                    if generation == 0 && container != number =>
                {
                    match self.entries.get(&(container, 0)) {
                        Some(&ActiveEntry::Normal(offset)) => Some(offset),
                        _ => None,
                    }
                }
                ActiveEntry::ObjStream { .. } => None,
            };
            let Some(offset) = offset else {
                continue;
            };
            if offset >= file.src.len() {
                continue;
            }
            if !out.contains_key(&(number, generation)) {
                cx.check(Kind::DecodedBytes, ENTRY_BYTES)?;
            }
            out.insert((number, generation), place_at(file, offset, header));
        }
        Ok(out)
    }
}

/// 指定オフセットの間接ストリームを `ObjStm` として展開し、メンバーのオフセット表を作る。
pub(crate) fn decode_object_stream_at(
    source: &[u8],
    container: u32,
    offset: usize,
    streams: inflate::Source<'_>,
    cx: &mut Cx<'_>,
) -> Result<Option<DecodedObjectStream>, Stop> {
    let Some(tail) = source.get(offset..) else {
        return Ok(None);
    };
    cx.check(Kind::Work, 1)?;
    let mut reader = Reader::new(tail);
    let Some(id) = reader.read_without_context::<ObjectIdentifier>() else {
        return Ok(None);
    };
    if id != ObjectIdentifier::new(container, 0) {
        return Ok(None);
    }
    reader.skip_white_spaces_and_comments();
    let Some(stream) = read_stream(&mut reader, cx)? else {
        return Ok(None);
    };
    decode_object_stream(&stream, streams, cx)
}

fn decode_object_stream(
    stream: &Stream<'_>,
    streams: inflate::Source<'_>,
    cx: &mut Cx<'_>,
) -> Result<Option<DecodedObjectStream>, Stop> {
    reserve_dict_slice(stream.dict().data(), 2, cx)?;
    if !inflate::before(stream, streams, cx)? {
        let raw_len = stream.raw_data().len() as u64;
        cx.check(Kind::Work, (raw_len >> SCAN_BLOCK_SHIFT).saturating_add(1))?;
        cx.check(Kind::DecodedBytes, raw_len.saturating_mul(2))?;
    }
    let Ok(decoded) = stream.decoded_data() else {
        return Ok(None);
    };
    let data = decoded.into_owned();
    let offsets = read_object_stream_offsets(stream, &data, cx)?.unwrap_or_default();
    Ok(Some(DecodedObjectStream { data, offsets }))
}

fn read_object_stream_offsets(
    stream: &Stream<'_>,
    data: &[u8],
    cx: &mut Cx<'_>,
) -> Result<Option<MemberOffsets>, Stop> {
    let Some(num_objects) = stream.dict().get::<usize>(b"N") else {
        return Ok(None);
    };
    let Some(first_offset) = stream.dict().get::<usize>(b"First") else {
        return Ok(None);
    };
    cx.check(
        Kind::Work,
        (num_objects as u64)
            .saturating_add((data.len() as u64) >> SCAN_BLOCK_SHIFT)
            .saturating_add(1),
    )?;
    cx.check(
        Kind::DecodedBytes,
        (num_objects as u64).saturating_mul(MEMBER_OFFSET_BYTES),
    )?;
    let mut r = Reader::new(data);
    let mut offsets = Vec::with_capacity(num_objects);
    let mut last_offset = None;
    for _ in 0..num_objects {
        r.skip_white_spaces_and_comments();
        let Some(obj_num) = r.read_without_context::<u32>() else {
            return Ok(None);
        };
        r.skip_white_spaces_and_comments();
        let Some(relative_offset) = r.read_without_context::<usize>() else {
            return Ok(None);
        };
        let Some(offset) = first_offset.checked_add(relative_offset) else {
            return Ok(None);
        };
        if offset >= data.len() || last_offset.is_some_and(|last| offset <= last) {
            offsets.push((obj_num, None));
        } else {
            let mut member = Reader::new(data);
            member.jump(offset);
            member.skip_white_spaces_and_comments();
            last_offset = Some(member.offset());
            offsets.push((obj_num, Some(member.offset())));
        }
    }
    Ok(Some(offsets))
}

fn find_last_xref_pos(source: &[u8], cx: &mut Cx<'_>) -> Result<Option<usize>, Stop> {
    let needle = b"startxref";
    if source.len() < needle.len() {
        cx.check(Kind::Work, 1)?;
        return Ok(None);
    }
    let step = 1_usize << SCAN_BLOCK_SHIFT;
    let mut end = source.len();
    let mut found = None;
    while end >= needle.len() {
        cx.check(Kind::Work, 1)?;
        let chunk_start = end.saturating_sub(step);
        let window_start = chunk_start.saturating_sub(needle.len() - 1);
        if let Some(slice) = source.get(window_start..end)
            && let Some(rel) = slice.windows(needle.len()).rposition(|w| w == needle)
        {
            found = Some(window_start + rel);
            break;
        }
        if chunk_start == 0 {
            break;
        }
        end = chunk_start;
    }
    let Some(pos) = found else {
        return Ok(None);
    };
    let mut finder = Reader::new(source);
    finder.jump(pos);
    if finder.forward_tag(needle).is_none() {
        return Ok(None);
    }
    finder.skip_white_spaces_and_comments();
    Ok(finder
        .read_without_context::<i32>()
        .and_then(|value| usize::try_from(value).ok()))
}

fn populate_xref<'a>(
    source: &'a [u8],
    pos: usize,
    streams: inflate::Source<'_>,
    entries: &mut BTreeMap<(u32, i32), ActiveEntry>,
    cx: &mut Cx<'_>,
) -> Result<Option<&'a [u8]>, Stop> {
    let mut visited = BTreeSet::new();
    populate_xref_inner(source, pos, streams, entries, &mut visited, cx)
}

fn populate_xref_inner<'a>(
    source: &'a [u8],
    pos: usize,
    streams: inflate::Source<'_>,
    entries: &mut BTreeMap<(u32, i32), ActiveEntry>,
    visited: &mut BTreeSet<usize>,
    cx: &mut Cx<'_>,
) -> Result<Option<&'a [u8]>, Stop> {
    cx.check(Kind::Work, 1)?;
    if pos > source.len() || !visited.insert(pos) || visited.len() > MAX_XREF_CHAIN_DEPTH {
        return Ok(None);
    }
    let mut reader = Reader::new(source);
    reader.jump(pos);
    reader.skip_white_spaces_and_comments();
    let mut r2 = reader.clone();
    if reader
        .clone()
        .read_without_context::<ObjectIdentifier>()
        .is_some()
    {
        populate_from_xref_stream(source, &mut r2, streams, entries, visited, cx)
    } else {
        populate_from_xref_table(source, &mut r2, streams, entries, visited, cx)
    }
}

fn populate_from_xref_table<'a>(
    source: &'a [u8],
    reader: &mut Reader<'a>,
    streams: inflate::Source<'_>,
    entries: &mut BTreeMap<(u32, i32), ActiveEntry>,
    visited: &mut BTreeSet<usize>,
    cx: &mut Cx<'_>,
) -> Result<Option<&'a [u8]>, Stop> {
    let Some(trailer) = read_xref_table_trailer(&mut reader.clone(), cx)? else {
        return Ok(None);
    };
    reader.skip_white_spaces();
    if reader.forward_tag(b"xref").is_none() {
        return Ok(None);
    }
    reader.skip_white_spaces();

    if let Some(prev) = trailer.get::<i32>(b"Prev") {
        let Ok(prev_pos) = usize::try_from(prev) else {
            return Ok(None);
        };
        if populate_xref_inner(source, prev_pos, streams, entries, visited, cx)?.is_none() {
            return Ok(None);
        }
    }
    if let Some(xref_stm) = trailer.get::<i32>(b"XRefStm") {
        let Ok(stm_pos) = usize::try_from(xref_stm) else {
            return Ok(None);
        };
        if populate_xref_inner(source, stm_pos, streams, entries, visited, cx)?.is_none() {
            return Ok(None);
        }
    }

    while let Some((start, num_entries)) = read_subsection_header(reader) {
        reader.skip_white_spaces();
        let Some(end) = start.checked_add(num_entries) else {
            return Ok(None);
        };
        cx.check(Kind::Work, u64::from(num_entries).saturating_add(1))?;
        for obj_number in start..end {
            let Some(bytes) = reader.read_bytes(XREF_ENTRY_LEN) else {
                return Ok(None);
            };
            let Some((offset, gen_number, used)) = parse_xref_entry(bytes) else {
                return Ok(None);
            };
            if used {
                insert_entry(
                    entries,
                    (obj_number, gen_number),
                    ActiveEntry::Normal(offset),
                    cx,
                )?;
            }
        }
    }
    Ok(Some(trailer.data()))
}

fn populate_from_xref_stream<'a>(
    source: &'a [u8],
    reader: &mut Reader<'a>,
    streams: inflate::Source<'_>,
    entries: &mut BTreeMap<(u32, i32), ActiveEntry>,
    visited: &mut BTreeSet<usize>,
    cx: &mut Cx<'_>,
) -> Result<Option<&'a [u8]>, Stop> {
    cx.check(Kind::Work, 1)?;
    if reader.read_without_context::<ObjectIdentifier>().is_none() {
        return Ok(None);
    }
    reader.skip_white_spaces_and_comments();
    let Some(stream) = read_stream(reader, cx)? else {
        return Ok(None);
    };
    if let Some(prev) = stream.dict().get::<i32>(b"Prev") {
        let Ok(prev_pos) = usize::try_from(prev) else {
            return Ok(None);
        };
        if populate_xref_inner(source, prev_pos, streams, entries, visited, cx)?.is_none() {
            return Ok(None);
        }
    }
    let Some(size) = stream.dict().get::<u32>(b"Size") else {
        return Ok(None);
    };
    let Some([f1_len, f2_len, f3_len]) = stream.dict().get::<[u8; 3]>(b"W") else {
        return Ok(None);
    };
    if f2_len == 0 || usize::from(f2_len) > size_of::<u64>() {
        return Ok(None);
    }
    if !inflate::before(&stream, streams, cx)? {
        let raw_len = stream.raw_data().len() as u64;
        cx.check(Kind::Work, (raw_len >> SCAN_BLOCK_SHIFT).saturating_add(1))?;
        cx.check(Kind::DecodedBytes, raw_len.saturating_mul(2))?;
    }
    let Ok(xref_data) = stream.decoded_data() else {
        return Ok(None);
    };
    let mut xref_reader = Reader::new(xref_data.as_ref());
    if let Some(arr) = stream.dict().get::<Array<'_>>(b"Index") {
        cx.check(
            Kind::Work,
            ((arr.data().len() as u64) >> SCAN_BLOCK_SHIFT).saturating_add(1),
        )?;
        for (start, num_elements) in arr.iter::<(u32, u32)>() {
            if !xref_stream_subsection(
                &mut xref_reader,
                start,
                num_elements,
                f1_len,
                f2_len,
                f3_len,
                entries,
                cx,
            )? {
                return Ok(None);
            }
        }
    } else if !xref_stream_subsection(
        &mut xref_reader,
        0,
        size,
        f1_len,
        f2_len,
        f3_len,
        entries,
        cx,
    )? {
        return Ok(None);
    }
    Ok(Some(stream.dict().data()))
}

#[allow(clippy::too_many_arguments)]
fn xref_stream_subsection(
    xref_reader: &mut Reader<'_>,
    start: u32,
    num_elements: u32,
    f1_len: u8,
    f2_len: u8,
    f3_len: u8,
    entries: &mut BTreeMap<(u32, i32), ActiveEntry>,
    cx: &mut Cx<'_>,
) -> Result<bool, Stop> {
    for i in 0..num_elements {
        cx.check(Kind::Work, 1)?;
        let f_type = if f1_len == 0 {
            1
        } else {
            let Some(first) = xref_reader
                .read_bytes(1)
                .and_then(|bytes| bytes.first().copied())
            else {
                return Ok(false);
            };
            first
        };
        let Some(obj_number) = start.checked_add(i) else {
            return Ok(false);
        };
        match f_type {
            0 => {
                if xref_reader
                    .skip_bytes(usize::from(f2_len) + usize::from(f3_len))
                    .is_none()
                {
                    return Ok(false);
                }
            }
            1 => {
                let offset = if f2_len > 0 {
                    let Some(data) = xref_reader.read_bytes(usize::from(f2_len)) else {
                        return Ok(false);
                    };
                    let Some(num) = xref_stream_num(data) else {
                        return Ok(false);
                    };
                    num
                } else {
                    0
                };
                let gen_number = if f3_len > 0 {
                    let Some(data) = xref_reader.read_bytes(usize::from(f3_len)) else {
                        return Ok(false);
                    };
                    let Some(num) = xref_stream_num(data) else {
                        return Ok(false);
                    };
                    num
                } else {
                    0
                };
                insert_entry(
                    entries,
                    (obj_number, gen_number.cast_signed()),
                    ActiveEntry::Normal(offset as usize),
                    cx,
                )?;
            }
            2 => {
                let Some(data) = xref_reader.read_bytes(usize::from(f2_len)) else {
                    return Ok(false);
                };
                let Some(container) = xref_stream_num(data) else {
                    return Ok(false);
                };
                let index = if f3_len > 0 {
                    let Some(data) = xref_reader.read_bytes(usize::from(f3_len)) else {
                        return Ok(false);
                    };
                    let Some(num) = xref_stream_num(data) else {
                        return Ok(false);
                    };
                    num
                } else {
                    0
                };
                insert_entry(
                    entries,
                    (obj_number, 0),
                    ActiveEntry::ObjStream { container, index },
                    cx,
                )?;
            }
            _ => return Ok(false),
        }
    }
    Ok(true)
}

fn xref_stream_num(data: &[u8]) -> Option<u32> {
    match data {
        [b0] => Some(u32::from(*b0)),
        [b0, b1] => Some(u32::from(u16::from_be_bytes([*b0, *b1]))),
        [b0, b1, b2] => Some(u32::from_be_bytes([0, *b0, *b1, *b2])),
        [b0, b1, b2, b3] => Some(u32::from_be_bytes([*b0, *b1, *b2, *b3])),
        [b0, b1, b2, b3, b4, b5, b6, b7] => {
            u32::try_from(u64::from_be_bytes([*b0, *b1, *b2, *b3, *b4, *b5, *b6, *b7])).ok()
        }
        _ => None,
    }
}

fn read_subsection_header(reader: &mut Reader<'_>) -> Option<(u32, u32)> {
    let mut probe = reader.clone();
    probe.skip_white_spaces();
    let start = probe.read_without_context::<u32>()?;
    probe.skip_white_spaces();
    let num_entries = probe.read_without_context::<u32>()?;
    probe.skip_white_spaces();
    *reader = probe;
    Some((start, num_entries))
}

fn parse_xref_entry(bytes: &[u8]) -> Option<(usize, i32, bool)> {
    fn parse_u32(slice: &[u8]) -> Option<u32> {
        let mut accum = 0_u32;
        for &byte in slice {
            accum = accum.checked_mul(10)?;
            match byte {
                b'0'..=b'9' => accum = accum.checked_add(u32::from(byte - b'0'))?,
                _ => return None,
            }
        }
        Some(accum)
    }
    let offset = parse_u32(bytes.get(0..10)?)? as usize;
    let gen_number = i32::try_from(parse_u32(bytes.get(11..16)?)?).ok()?;
    let used = *bytes.get(17)? == b'n';
    Some((offset, gen_number, used))
}

fn read_xref_table_trailer<'a>(
    reader: &mut Reader<'a>,
    cx: &mut Cx<'_>,
) -> Result<Option<Dict<'a>>, Stop> {
    reader.skip_white_spaces();
    if reader.forward_tag(b"xref").is_none() {
        return Ok(None);
    }
    reader.skip_white_spaces();
    while let Some((_, num_entries)) = read_subsection_header(reader) {
        cx.check(Kind::Work, 1)?;
        let Some(len) = XREF_ENTRY_LEN.checked_mul(num_entries as usize) else {
            return Ok(None);
        };
        let Some(next) = reader.offset().checked_add(len) else {
            return Ok(None);
        };
        if reader.range(0..next).is_none() {
            return Ok(None);
        }
        reader.jump(next);
    }
    reader.skip_white_spaces();
    if reader.forward_tag(b"trailer").is_none() {
        return Ok(None);
    }
    reader.skip_white_spaces();
    let mut probe = reader.clone();
    let Some(dict_bytes) = probe.skip::<Dict<'_>>(false) else {
        return Ok(None);
    };
    reserve_dict_slice(dict_bytes, 1, cx)?;
    Ok(reader.read_with_context::<Dict<'_>>(&ReaderContext::dummy()))
}

fn read_stream<'a>(reader: &mut Reader<'a>, cx: &mut Cx<'_>) -> Result<Option<Stream<'a>>, Stop> {
    let mut probe = reader.clone();
    let Some(dict_bytes) = probe.skip::<Dict<'_>>(false) else {
        return Ok(None);
    };
    reserve_dict_slice(dict_bytes, 2, cx)?;
    let tail_len = reader.tail().map_or(0, <[u8]>::len);
    cx.check(Kind::Work, (tail_len as u64) >> SCAN_BLOCK_SHIFT)?;
    Ok(reader.read_with_context::<Stream<'_>>(&ReaderContext::dummy()))
}

fn reserve_dict_slice(dict_bytes: &[u8], copies: u64, cx: &mut Cx<'_>) -> Result<(), Stop> {
    let mut keys = 0_u64;
    for chunk in dict_bytes.chunks(1_usize << SCAN_BLOCK_SHIFT) {
        cx.check(Kind::Work, 1)?;
        keys = keys.saturating_add(chunk.iter().filter(|&&b| b == b'/').count() as u64);
    }
    if dict_bytes.is_empty() {
        cx.check(Kind::Work, 1)?;
    }
    if keys == 0 {
        return Ok(());
    }
    let bytes = keys
        .saturating_mul(DICT_KEY_BYTES)
        .saturating_add(DICT_BASE_BYTES)
        .saturating_mul(copies);
    cx.check(Kind::DecodedBytes, bytes)
}

/// `hayro_syntax::xref::fallback_xref_map_inner` と同じ字句走査で相互参照表を作る。
fn populate_fallback(
    source: &[u8],
    has_object_streams: bool,
    streams: inflate::Source<'_>,
    entries: &mut BTreeMap<(u32, i32), ActiveEntry>,
    cx: &mut Cx<'_>,
) -> Result<(), Stop> {
    cx.check(Kind::Work, 1)?;
    let dummy = ReaderContext::dummy();
    let mut r = Reader::new(source);
    let mut last_obj_num: Option<ObjectIdentifier> = None;
    let mut checked_block = 0_usize;

    while !r.at_end() {
        let cur_pos = r.offset();
        let cur_block = cur_pos >> SCAN_BLOCK_SHIFT;
        if cur_block > checked_block {
            cx.check(Kind::Work, (cur_block - checked_block) as u64)?;
            checked_block = cur_block;
        }
        let mut old_r = r.clone();

        if r.peek_byte().is_some_and(|b| b.is_ascii_digit()) {
            if let Some(obj_id) = r.read_with_context::<ObjectIdentifier>(&dummy) {
                let mut cloned = r.clone();
                cloned.skip_white_spaces_and_comments();
                let tail_len = cloned.tail().map_or(0, <[u8]>::len);
                if cloned.skip::<Object<'_>>(false).is_some() {
                    let skipped = tail_len.saturating_sub(cloned.tail().map_or(0, <[u8]>::len));
                    cx.check(
                        Kind::Work,
                        ((skipped as u64) >> SCAN_BLOCK_SHIFT).saturating_add(1),
                    )?;
                    insert_entry(
                        entries,
                        (obj_id.obj_number, obj_id.gen_number),
                        ActiveEntry::Normal(cur_pos),
                        cx,
                    )?;
                    last_obj_num = Some(obj_id);
                }
            } else {
                r.forward_while(|b| !is_pdf_white(b));
            }
        } else if r.peek_bytes(2) == Some(b"<<") {
            let mut probe = r.clone();
            if let Some(dict_bytes) = probe.skip::<Dict<'_>>(false) {
                cx.check(Kind::Work, (dict_bytes.len() as u64) >> SCAN_BLOCK_SHIFT)?;
                r = probe;
                if has_object_streams
                    && (dict_bytes.windows(5).any(|w| w == b"/Type")
                        || dict_bytes.windows(2).any(|w| w == b"/#"))
                    && let Some(last_id) = last_obj_num
                {
                    let mut after_dict = r.clone();
                    after_dict.skip_white_spaces_and_comments();
                    if after_dict.forward_tag(b"stream").is_some() {
                        reserve_dict_slice(dict_bytes, 1, cx)?;
                        if let Some(dict) =
                            Reader::new(dict_bytes).read_with_context::<Dict<'_>>(&dummy)
                            && dict.get::<Name<'_>>(b"Type").as_deref()
                                == Some(b"ObjStm".as_slice())
                            && let Some(stream) = read_stream(&mut old_r, cx)?
                            && let Some(obj_stream) = decode_object_stream(&stream, streams, cx)?
                        {
                            for (idx, &(obj_num, offset)) in obj_stream.offsets.iter().enumerate() {
                                if offset.is_none() {
                                    continue;
                                }
                                let Ok(idx) = u32::try_from(idx) else {
                                    continue;
                                };
                                let key = (obj_num, 0);
                                if entries
                                    .get(&key)
                                    .is_none_or(|e| !matches!(e, ActiveEntry::Normal(_)))
                                {
                                    cx.check(Kind::Work, 1)?;
                                    insert_entry(
                                        entries,
                                        key,
                                        ActiveEntry::ObjStream {
                                            container: last_id.obj_number,
                                            index: idx,
                                        },
                                        cx,
                                    )?;
                                }
                            }
                        }
                    }
                }
            } else {
                step_non_white(&mut r);
            }
        } else {
            step_non_white(&mut r);
        }
    }
    Ok(())
}

fn has_duplicate_object_records(file: &PdfFile) -> bool {
    let mut seen = BTreeSet::new();
    for record in file.records() {
        if let Some(key) = record.object
            && !seen.insert(key)
        {
            return true;
        }
    }
    false
}

fn insert_entry(
    entries: &mut BTreeMap<(u32, i32), ActiveEntry>,
    key: (u32, i32),
    entry: ActiveEntry,
    cx: &mut Cx<'_>,
) -> Result<(), Stop> {
    if !entries.contains_key(&key) {
        cx.check(Kind::DecodedBytes, ENTRY_BYTES)?;
    }
    entries.insert(key, entry);
    Ok(())
}

fn step_non_white(reader: &mut Reader<'_>) {
    let before = reader.offset();
    reader.forward_while(|byte| !is_pdf_white(byte));
    if reader.offset() == before {
        let _ = reader.read_byte();
    }
}

fn is_pdf_white(byte: u8) -> bool {
    matches!(byte, 0x00 | 0x09 | 0x0a | 0x0c | 0x0d | 0x20)
}
