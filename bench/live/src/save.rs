//! 保存。変えていないレコードは元のバイト列を写し、変えたレコードと足したレコードは、今の実装の書き出し
//! （`crates/zumen-acad/src/dxf.rs` と `dxf/write_back.rs`）と同じ規則で、タグごとに書く。除いたレコードは書かない。

use std::io::Write;

use crate::paged::RecordSlice;
use crate::store::{Document, OTag};

/// タグを書く。足したタグは、グループ・コードを 3 桁の右寄せで書く。変えたタグは、値の前後の空白と改行を保つ
fn write_tag(out: &mut Vec<u8>, slice: &RecordSlice<'_>, tag: &OTag, newline: &[u8]) {
    match (&tag.edited, tag.fresh) {
        (Some(value), true) => {
            let _ = write!(out, "{:>3}", tag.code);
            out.extend_from_slice(newline);
            out.extend_from_slice(value);
            out.extend_from_slice(newline);
        }
        (Some(value), false) => {
            out.extend_from_slice(slice.slice(tag.span.start, tag.span.value));
            out.extend_from_slice(value);
            out.extend_from_slice(slice.slice(tag.span.value_end, tag.span.end));
        }
        (None, _) => out.extend_from_slice(slice.slice(tag.span.start, tag.span.end)),
    }
}

impl Document {
    /// 文書の全体を DXF のバイト列にする。手間は、ファイルの大きさと、変えた・足した・除いたレコードの数による
    pub fn save(&self) -> Vec<u8> {
        let src = &self.src;
        let mut reader = src.page_reader();
        let newline = src.newline();
        let first_added = self.source_records();
        let mut out = Vec::with_capacity(src.len() + self.overlay.len() * 64);
        let mut written = 0;
        let empty_slice = RecordSlice {
            base: 0,
            bytes: &[],
        };
        // 元のレコードのうち、変えたものと除いたものを、ファイルの順に見る。足したレコードは、今の実装と同じく、
        // ENTITIES のセクションを閉じる ENDSEC の前に、足した順に書く
        let mut changed = self.overlay.range(..first_added).peekable();
        let mut gone = self.gone.range(..first_added).copied().peekable();
        let added = || {
            self.overlay
                .range(first_added..)
                .filter(|(record, _)| !self.gone.contains(record))
        };
        let mut end = self.entities_end.filter(|_| added().next().is_some());
        loop {
            let stops = [
                changed.peek().map(|(record, _)| **record),
                gone.peek().copied(),
                end,
            ];
            let Some(record) = stops.into_iter().flatten().min() else {
                break;
            };
            let range = self.record_range(record);
            reader.append_range(&mut out, written..range.start);
            written = range.start;
            if end == Some(record) {
                for tag in added().flat_map(|(_, ov)| &ov.tags) {
                    write_tag(&mut out, &empty_slice, tag, newline);
                }
                end = None;
            }
            let changed_ov = changed.next_if(|(at, _)| **at == record).map(|(_, ov)| ov);
            if gone.next_if_eq(&record).is_some() {
                written = range.end;
            } else if let Some(ov) = changed_ov {
                if !ov.raw.is_empty() {
                    let slice = RecordSlice {
                        base: ov.base as usize,
                        bytes: &ov.raw,
                    };
                    for tag in &ov.tags {
                        write_tag(&mut out, &slice, tag, newline);
                    }
                } else {
                    reader.with_range(range.clone(), |slice| {
                        for tag in &ov.tags {
                            write_tag(&mut out, &slice, tag, newline);
                        }
                    });
                }
                written = range.end;
            }
        }
        reader.append_range(&mut out, written..src.len());
        out
    }
}
