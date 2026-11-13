//! 測るための大きい入力を作る。モデル空間の要素を、ずらしながら写して増やす。
//!
//! 写しには新しいハンドルを振り、`$HANDSEED` と `$EXTMAX` を直す。属性の続く記号（INSERT）は、続く ATTRIB と
//! SEQEND も写す。写すのはレコードの値だけなので、拡張ディクショナリやリアクターを持つ図形の写しは、元の図形と
//! 同じものを指す。測るための入力であり、ほかのアプリケーションに渡す図面ではない。

use std::ops::Range;

use crate::lift::Kind;
use crate::scan::{self, Next, Span};
use crate::store::{Document, OpenError};
use crate::value::{format_real, parse_hex, parse_real};

/// 増やせない理由
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GrowError {
    /// 開けない
    Open(OpenError),
    /// ENTITIES のセクションがない
    NoEntities,
    /// 写す要素がない
    NoElements,
}

/// 置き換える値の場所と、新しい値
type Patch = (Range<usize>, Vec<u8>);

fn value_range(tag: &Span) -> Range<usize> {
    tag.value as usize..tag.value_end as usize
}

fn value_of<'a>(src: &'a [u8], tag: &Span) -> &'a [u8] {
    src.get(value_range(tag)).unwrap_or_default()
}

/// ファイルを通して読んで分かること
pub struct Outline {
    /// いちばん大きいハンドル
    pub largest: u64,
    /// ENTITIES のセクションを閉じる ENDSEC の位置
    pub insert_at: Option<usize>,
    /// `$HANDSEED` の値の場所
    pub seed: Option<Range<usize>>,
    /// `$EXTMAX` の x と y の値の場所と、今の値
    pub extmax: [Option<(Range<usize>, f64)>; 2],
}

/// ハンドルのいちばん大きい値と、図形を足す場所、直すヘッダーの値の場所を調べる
pub fn outline(src: &[u8]) -> Outline {
    let mut found = Outline {
        largest: 0,
        insert_at: None,
        seed: None,
        extmax: [None, None],
    };
    let mut in_entities = false;
    let mut section_head = false;
    let mut variable: &[u8] = &[];
    let mut pos = 0;
    while let Next::Tag(tag) = scan::next_tag(src, pos) {
        pos = tag.end as usize;
        let value = value_of(src, &tag).trim_ascii();
        match tag.code {
            0 => {
                if value == b"ENDSEC" && in_entities && found.insert_at.is_none() {
                    found.insert_at = Some(tag.start as usize);
                }
                if value == b"ENDSEC" {
                    in_entities = false;
                }
                section_head = value == b"SECTION";
                variable = &[];
            }
            2 if section_head => {
                in_entities = value == b"ENTITIES";
                section_head = false;
            }
            9 => variable = value,
            5 | 105 => {
                if variable == b"$HANDSEED" && found.seed.is_none() {
                    found.seed = Some(value_range(&tag));
                }
                if let Some(handle) = parse_hex(value) {
                    found.largest = found.largest.max(handle);
                }
            }
            10 | 20 if variable == b"$EXTMAX" => {
                let axis = usize::from(tag.code == 20);
                if let (Some(place), Some(old)) = (found.extmax.get_mut(axis), parse_real(value))
                    && place.is_none()
                {
                    *place = Some((value_range(&tag), old));
                }
            }
            _ => {}
        }
    }
    found
}

/// レコードの写し方
struct Copy {
    /// 点（10・20 と 11・21）をずらす量
    shift: [f64; 2],
    /// ハンドル（5 の、16 進として読める最初の値）を新しく振るか
    rename: bool,
    /// 写す前と後の、親の記号のハンドル。所有者（330）が前の値なら、後の値にする
    owner: Option<(u64, u64)>,
}

impl Copy {
    /// レコードを写して `out` に足す。新しく振ったハンドルを返す
    fn write(
        &self,
        src: &[u8],
        range: Range<usize>,
        next: &mut u64,
        out: &mut Vec<u8>,
    ) -> Option<u64> {
        let mut pos = range.start;
        let mut handle = None;
        while pos < range.end {
            let Next::Tag(tag) = scan::next_tag(src, pos) else {
                break;
            };
            pos = tag.end as usize;
            let value = value_of(src, &tag);
            let replaced = match tag.code {
                5 if self.rename && handle.is_none() && parse_hex(value.trim_ascii()).is_some() => {
                    *next += 1;
                    handle = Some(*next);
                    Some(format!("{next:X}").into_bytes())
                }
                330 => self
                    .owner
                    .filter(|(old, _)| parse_hex(value.trim_ascii()) == Some(*old))
                    .map(|(_, new)| format!("{new:X}").into_bytes()),
                10 | 11 => parse_real(value).map(|x| format_real(x + self.shift[0])),
                20 | 21 => parse_real(value).map(|y| format_real(y + self.shift[1])),
                _ => None,
            };
            match replaced {
                Some(value) => {
                    out.extend_from_slice(
                        src.get(tag.start as usize..tag.value as usize)
                            .unwrap_or_default(),
                    );
                    out.extend_from_slice(&value);
                    out.extend_from_slice(
                        src.get(tag.value_end as usize..tag.end as usize)
                            .unwrap_or_default(),
                    );
                }
                None => out.extend_from_slice(
                    src.get(tag.start as usize..tag.end as usize)
                        .unwrap_or_default(),
                ),
            }
        }
        handle
    }
}

/// モデル空間の要素を `tiles` 枚（元を含む）に増やした DXF を返す
pub fn grow(src: &[u8], tiles: usize, threads: usize) -> Result<Vec<u8>, GrowError> {
    let document = Document::open(src.to_vec(), threads).map_err(GrowError::Open)?;
    let found = outline(src);
    let insert_at = found.insert_at.ok_or(GrowError::NoEntities)?;
    if document.rows.is_empty() {
        return Err(GrowError::NoElements);
    }
    // 元の図面の範囲。写しは、この大きさの升目に並べる
    let mut extent = [
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    ];
    for item in &document.boxes {
        if item.iter().all(|value| value.is_finite()) {
            extent = [
                extent[0].min(item[0]),
                extent[1].min(item[1]),
                extent[2].max(item[2]),
                extent[3].max(item[3]),
            ];
        }
    }
    if extent.iter().any(|value| !value.is_finite()) {
        extent = [0.0, 0.0, 1.0, 1.0];
    }
    let pitch = [
        ((extent[2] - extent[0]) * 1.0625).max(1.0),
        ((extent[3] - extent[1]) * 1.0625).max(1.0),
    ];
    let tiles = tiles.max(1);
    let mut columns = 1;
    while columns * columns < tiles {
        columns += 1;
    }

    let mut added = Vec::with_capacity(src.len().saturating_mul(tiles - 1));
    let mut next = found.largest;
    for tile in 1..tiles {
        let shift = [
            (tile % columns) as f64 * pitch[0],
            (tile / columns) as f64 * pitch[1],
        ];
        for row in &document.rows {
            let copy = Copy {
                shift,
                rename: row.handle != 0,
                owner: None,
            };
            let handle = copy.write(
                src,
                document.record_range(row.record),
                &mut next,
                &mut added,
            );
            // 属性の続く記号は、続く ATTRIB と SEQEND も写す。所有者が元の記号を指していれば、写しの記号に向ける。
            // 付き方がレコードの並びだけで決まらない図面では、記号のレコードだけを写す
            let followed = row.kind == Kind::Insert
                && document
                    .shapes
                    .inserts
                    .get(row.geom as usize)
                    .is_some_and(|insert| insert.followed);
            if !(followed && document.plain) {
                continue;
            }
            let copy = Copy {
                shift,
                rename: true,
                owner: (row.handle != 0).then_some(row.handle).zip(handle),
            };
            for record in row.record + 1..document.source_records() {
                let name = document.kind_name(record);
                let closes = name == b"SEQEND";
                if !document.in_entities(record) || !(closes || name == b"ATTRIB") {
                    break;
                }
                copy.write(src, document.record_range(record), &mut next, &mut added);
                if closes {
                    break;
                }
            }
        }
    }

    // ヘッダーの値を直す。どれも、足す場所より前にある
    let last = [
        ((tiles - 1).min(columns - 1)) as f64 * pitch[0],
        ((tiles - 1) / columns) as f64 * pitch[1],
    ];
    let mut patches: Vec<Patch> = Vec::new();
    if let Some(seed) = found.seed {
        patches.push((seed, format!("{:X}", next + 1).into_bytes()));
    }
    for (axis, place) in found.extmax.into_iter().enumerate() {
        if let Some((range, old)) = place {
            let reach = extent[axis + 2] + last[axis];
            if reach > old {
                patches.push((range, format_real(reach)));
            }
        }
    }
    patches.retain(|(range, _)| range.end <= insert_at);
    patches.sort_by_key(|(range, _)| range.start);

    let mut out = Vec::with_capacity(src.len() + added.len() + 64);
    let mut written = 0;
    for (range, value) in patches {
        out.extend_from_slice(src.get(written..range.start).unwrap_or_default());
        out.extend_from_slice(&value);
        written = range.end;
    }
    out.extend_from_slice(src.get(written..insert_at).unwrap_or_default());
    out.extend_from_slice(&added);
    out.extend_from_slice(src.get(insert_at..).unwrap_or_default());
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copies_get_new_handles_and_shifted_points() {
        let src = b"  0\nSECTION\n  2\nHEADER\n  9\n$HANDSEED\n  5\n30\n  9\n$EXTMAX\n 10\n4.0\n 20\n2.0\n 30\n0.0\n  0\nENDSEC\n\
                    \x20 0\nSECTION\n  2\nENTITIES\n  0\nLINE\n  5\n2A\n  8\n0\n 10\n0.0\n 20\n0.0\n 11\n4.0\n 21\n2.0\n\
                    \x20 0\nCIRCLE\n  5\n2B\n  8\n0\n 10\n1.0\n 20\n1.0\n 40\n0.5\n  0\nENDSEC\n  0\nEOF\n";
        let grown = grow(src, 4, 1).expect("増やす");
        let document = Document::open(grown.clone(), 1).expect("開く");
        assert_eq!(document.rows.len(), 8);
        assert_eq!(document.locked, None);
        // 新しいハンドルは、元のいちばん大きい値（$HANDSEED の 30）の次から
        let handles: Vec<u64> = document.rows.iter().map(|row| row.handle).collect();
        assert_eq!(handles, [0x2A, 0x2B, 0x31, 0x32, 0x33, 0x34, 0x35, 0x36]);
        let text = String::from_utf8(grown).expect("ASCII");
        assert!(text.contains("$HANDSEED\n  5\n37\n"), "{text}");
        // 2 列に並ぶ。升目は、範囲（4 × 2）より少し大きい
        assert!(text.contains("$EXTMAX\n 10\n8.25\n 20\n4.125\n"), "{text}");
        assert_eq!(document.boxes[2], [4.25, 0.0, 8.25, 2.0]);
        assert_eq!(document.boxes[7], [4.75, 2.625, 5.75, 3.625]);
    }

    #[test]
    fn copies_of_a_symbol_take_their_attributes() {
        let src = b"  0\nSECTION\n  2\nHEADER\n  9\n$HANDSEED\n  5\n30\n  0\nENDSEC\n\
                    \x20 0\nSECTION\n  2\nENTITIES\n\
                    \x20 0\nINSERT\n  5\n2A\n  8\n0\n 66\n     1\n  2\nMARK\n 10\n1.0\n 20\n1.0\n\
                    \x20 0\nATTRIB\n  5\n2B\n330\n2A\n  8\n0\n 10\n1.5\n 20\n1.5\n 40\n0.25\n  1\nA\n  2\nTAG\n 70\n     0\n\
                    \x20 0\nSEQEND\n  5\n2C\n330\n2A\n  8\n0\n\
                    \x20 0\nLINE\n  5\n2D\n  8\n0\n 10\n0.0\n 20\n0.0\n 11\n4.0\n 21\n2.0\n  0\nENDSEC\n  0\nEOF\n";
        let grown = grow(src, 3, 1).expect("増やす");
        let document = Document::open(grown.clone(), 1).expect("開く");
        // 記号と線が 3 組。属性と SEQEND は、要素にならない
        assert_eq!(document.rows.len(), 6);
        assert!(document.plain);
        let handles: Vec<u64> = document.rows.iter().map(|row| row.handle).collect();
        assert_eq!(handles, [0x2A, 0x2D, 0x31, 0x34, 0x35, 0x38]);
        for insert in &document.shapes.inserts {
            assert!(insert.followed);
            assert_eq!(insert.attached, 1);
            assert_eq!(insert.attribs.1, 1);
        }
        let text = String::from_utf8(grown).expect("ASCII");
        // 写しの属性と SEQEND は、写しの記号を所有者にする
        assert!(text.contains("ATTRIB\n  5\n32\n330\n31\n"), "{text}");
        assert!(text.contains("SEQEND\n  5\n33\n330\n31\n"), "{text}");
        assert!(text.contains("ATTRIB\n  5\n36\n330\n35\n"), "{text}");
        assert!(text.contains("$HANDSEED\n  5\n39\n"), "{text}");
    }
}
