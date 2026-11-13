//! 保存。変えていないレコードは元のバイト列を写し、変えたレコードと足したレコードは、今の実装の書き出し
//! （`crates/zumen-acad/src/dxf.rs` と `dxf/write_back.rs`）と同じ規則で、タグごとに書く。除いたレコードは書かない。

use crate::store::{Document, OTag};

/// 元のファイルの改行。最初の改行が CR LF なら CR LF、そうでなければ LF
fn newline(src: &[u8]) -> &'static [u8] {
    match src.iter().position(|&byte| byte == b'\n') {
        Some(at) if at > 0 && src.get(at - 1) == Some(&b'\r') => b"\r\n",
        _ => b"\n",
    }
}

/// タグを書く。足したタグは、グループ・コードを 3 桁の右寄せで書く。変えたタグは、値の前後の空白と改行を保つ
fn write_tag(out: &mut Vec<u8>, src: &[u8], tag: &OTag, newline: &[u8]) {
    let part = |start: u32, end: u32| src.get(start as usize..end as usize).unwrap_or_default();
    match (&tag.edited, tag.fresh) {
        (Some(value), true) => {
            out.extend_from_slice(format!("{:>3}", tag.code).as_bytes());
            out.extend_from_slice(newline);
            out.extend_from_slice(value);
            out.extend_from_slice(newline);
        }
        (Some(value), false) => {
            out.extend_from_slice(part(tag.span.start, tag.span.value));
            out.extend_from_slice(value);
            out.extend_from_slice(part(tag.span.value_end, tag.span.end));
        }
        (None, _) => out.extend_from_slice(part(tag.span.start, tag.span.end)),
    }
}

impl Document {
    /// 文書の全体を DXF のバイト列にする。手間は、ファイルの大きさと、変えた・足した・除いたレコードの数による
    pub fn save(&self) -> Vec<u8> {
        let src = &self.src;
        let newline = newline(src);
        let first_added = self.source_records();
        let mut out = Vec::with_capacity(src.len() + self.overlay.len() * 64);
        let mut written = 0;
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
            out.extend_from_slice(src.get(written..range.start).unwrap_or_default());
            written = range.start;
            if end == Some(record) {
                for tag in added().flat_map(|(_, tags)| tags) {
                    write_tag(&mut out, src, tag, newline);
                }
                end = None;
            }
            let tags = changed
                .next_if(|(at, _)| **at == record)
                .map(|(_, tags)| tags);
            if gone.next_if_eq(&record).is_some() {
                written = range.end;
            } else if let Some(tags) = tags {
                for tag in tags {
                    write_tag(&mut out, src, tag, newline);
                }
                written = range.end;
            }
        }
        out.extend_from_slice(src.get(written..).unwrap_or_default());
        out
    }
}
