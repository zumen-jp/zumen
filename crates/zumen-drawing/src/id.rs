//! 要素の ID（決定 0117・0156）
//!
//! ID は、空間か用紙（`space/0`・`sheet/h22`・`sheet/1`）と、出所の部分を `/` でつなぐ。出所の部分は、形式が持つ鍵が
//! あれば鍵で作る。ハンドルは `h` と大文字の 16 進、インスタンスの番号は `i` と 10 進で書く。鍵がなければ、
//! レコードの番号（`r`）、バイトの位置（`b`）、要素の順（`e`）で作る。1 つのレコードから作った 2 つ目からの
//! 要素には、`/1`・`/2` を足す。用紙は、元の形式の用紙のレコード（LAYOUT）がハンドルを持てば `sheet/h22`、
//! なければ読んだ順の番号で作る。

use std::collections::BTreeMap;

use zumen_base::{Key, Ref};

use crate::{Element, Sheet, SpaceId};

/// 空間の ID の文字列
pub fn space(id: SpaceId) -> String {
    format!("space/{}", id.0)
}

/// 用紙の ID の文字列
pub fn sheet(sheet: &Sheet) -> String {
    match sheet.source.key() {
        Some(Key::Handle(handle)) => format!("sheet/h{handle:X}"),
        _ => format!("sheet/{}", sheet.id.0),
    }
}

/// 出所の部分。`index` は、空間か用紙の中の要素の順
pub fn origin(source: &Ref, index: usize) -> String {
    match (source.key(), source.record(), source.bytes()) {
        (Some(Key::Handle(handle)), _, _) => format!("h{handle:X}"),
        (Some(Key::Instance(number)), _, _) => format!("i{number}"),
        (None, Some(record), _) => format!("r{record}"),
        (None, None, Some(bytes)) => format!("b{}", bytes.start),
        (None, None, None) => format!("e{index}"),
    }
}

/// 1 つの空間か用紙の要素に、要素の順に ID を振る
#[derive(Debug)]
pub struct Namer<'a> {
    place: &'a str,
    /// 出所ごとの、これまでに出てきた要素の数
    seen: BTreeMap<String, u64>,
}

impl<'a> Namer<'a> {
    /// `place` は [`space`] か [`sheet`] の文字列
    pub fn new(place: &'a str) -> Self {
        Self {
            place,
            seen: BTreeMap::new(),
        }
    }

    /// 次の要素の ID。要素は、空間か用紙の順に、飛ばさずに渡す
    pub fn next<P>(&mut self, index: usize, element: &'a Element<P>) -> String {
        let origin = origin(&element.source, index);
        let repeat = if element.source.key().is_none()
            && element.source.record().is_none()
            && element.source.bytes().is_none()
        {
            0
        } else {
            let count = self.seen.entry(origin.clone()).or_insert(0);
            *count += 1;
            *count - 1
        };
        match repeat {
            0 => format!("{}/{origin}", self.place),
            n => format!("{}/{origin}/{n}", self.place),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LayerId, ModelPoint, Shape, Style};

    fn element(source: Ref) -> Element<ModelPoint> {
        let at = ModelPoint {
            space: SpaceId(0),
            x: 0.0,
            y: 0.0,
        };
        Element {
            source,
            layer: LayerId(0),
            style: Style::BLOCK,
            shape: Shape::Line { start: at, end: at },
        }
    }

    #[test]
    fn keys_come_before_positions() {
        let place = space(SpaceId(0));
        let elements = [
            element(Ref::at_record(3).with_key(Key::Handle(0x2f))),
            element(Ref::at_record(4).with_key(Key::Handle(0x2f))),
            element(Ref::at_bytes(10, 20)),
            element(Ref::at_record(7)),
            element(Ref::default().with_key(Key::Instance(120))),
            element(Ref::default()),
        ];
        let mut namer = Namer::new(&place);
        let ids: Vec<_> = elements
            .iter()
            .enumerate()
            .map(|(index, element)| namer.next(index, element))
            .collect();
        assert_eq!(
            ids,
            [
                "space/0/h2F",
                "space/0/h2F/1",
                "space/0/b10",
                "space/0/r7",
                "space/0/i120",
                "space/0/e5"
            ]
        );
        let mut paper = Sheet {
            id: crate::SheetId(2),
            name: None,
            size: None,
            elements: Vec::new(),
            views: Vec::new(),
            order: Vec::new(),
            source: Ref::at_record(9),
        };
        assert_eq!(sheet(&paper), "sheet/2");
        paper.source = paper.source.with_key(Key::Handle(0x22));
        assert_eq!(sheet(&paper), "sheet/h22");
    }
}
