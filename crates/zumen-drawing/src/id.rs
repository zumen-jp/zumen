//! 要素の ID（決定 0117・0156）
//!
//! ID は、空間か用紙（`space/0`・`sheet/h22`・`sheet/1`）と、出所の部分を `/` でつなぐ。出所の部分は、形式が持つ鍵が
//! あれば鍵で作る。ハンドルは `h` と大文字の 16 進、インスタンスの番号は `i` と 10 進で書く。鍵がなければ、
//! レコードの番号（`r`）、バイトの位置（`b`）、要素の順（`e`）で作る。1 つのレコードから作った 2 つ目からの
//! 要素には、`/1`・`/2` を足す。用紙は、元の形式の用紙のレコード（LAYOUT）がハンドルを持てば `sheet/h22`、
//! なければ読んだ順の番号で作る。同じ用紙 ID が重複するときは、2 つ目から `/1`・`/2` を足す。
//!
//! 記号の定義の空間は、定義（BLOCK）がハンドルを持てば `space/h1F`、なければ `space/` と空間の番号で作る。
//! 同じ定義の ID が重なれば、2 つ目から `/1`・`/2` を足す。
//! レイヤと線種の表の項目は、表のレコードがハンドルを持てば `layer/h10`・`line-type/h14`、なければ `layer/` と
//! `line-type/` に読んだ順の番号を足して作る。寸法スタイルも同じく、`dimension-style/h1A` か、図面の寸法スタイルの
//! 並びの中の順で `dimension-style/0` と作る。

use std::collections::BTreeMap;

use zumen_base::{Cx, Key, Ref, Stop};

use crate::{
    DimensionStyle, Drawing2D, Element, Hash, Layer, LineType, ModelPoint, PaperPoint, Position,
    Sheet, SpaceId, SymbolDefinition, TextStyle,
};

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

/// 用紙に順に、重複しない ID を振る
#[derive(Debug, Default)]
pub struct SheetNamer {
    seen: BTreeMap<String, u64>,
}

impl SheetNamer {
    /// 次の用紙の ID。用紙は図面モデルの順に、飛ばさずに渡す
    pub fn next(&mut self, value: &Sheet) -> String {
        let id = sheet(value);
        let count = self.seen.entry(id.clone()).or_insert(0);
        let repeat = *count;
        *count += 1;
        if repeat == 0 {
            id
        } else {
            format!("{id}/{repeat}")
        }
    }
}

/// 記号の定義の空間の ID の基底。同じハンドルが重なるときは、[`DefinitionNamer`] で区別する
pub fn definition(definition: &SymbolDefinition) -> String {
    match definition.source.key() {
        Some(Key::Handle(handle)) => format!("space/h{handle:X}"),
        _ => space(definition.space.id),
    }
}

/// 記号の定義に順に、重複しない空間の ID を振る
#[derive(Debug, Default)]
pub struct DefinitionNamer {
    seen: BTreeMap<String, u64>,
}

impl DefinitionNamer {
    /// 次の定義の ID。定義は図面モデルの順に、飛ばさずに渡す
    pub fn next(&mut self, value: &SymbolDefinition) -> String {
        let id = definition(value);
        let count = self.seen.entry(id.clone()).or_insert(0);
        let repeat = *count;
        *count += 1;
        if repeat == 0 {
            id
        } else {
            format!("{id}/{repeat}")
        }
    }
}

/// レイヤの ID の文字列
pub fn layer(layer: &Layer) -> String {
    keyed("layer", &layer.source, layer.id.0)
}

/// 線種の ID の文字列
pub fn line_type(line_type: &LineType) -> String {
    keyed("line-type", &line_type.source, line_type.id.0)
}

/// 寸法スタイルの ID の文字列。`index` は、図面の寸法スタイルの並びの中の順
pub fn dimension_style(style: &DimensionStyle, index: usize) -> String {
    match style.source.key() {
        Some(Key::Handle(handle)) => format!("dimension-style/h{handle:X}"),
        _ => format!("dimension-style/{index}"),
    }
}

/// 文字スタイルの ID の文字列（決定 0158）。ハンドルがなければ、表の中の順の番号で作る
pub fn text_style(style: &TextStyle, index: usize) -> String {
    match style.source.key() {
        Some(Key::Handle(handle)) => format!("text-style/h{handle:X}"),
        _ => format!("text-style/{index}"),
    }
}

/// 表の項目の ID の文字列。ハンドルがなければ、読んだ順の番号で作る
fn keyed(table: &str, source: &Ref, number: u32) -> String {
    match source.key() {
        Some(Key::Handle(handle)) => format!("{table}/h{handle:X}"),
        _ => format!("{table}/{number}"),
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

/// 図面の中の要素
#[derive(Clone, Copy, Debug)]
pub enum Found<'a> {
    /// 実寸の空間か記号の定義の空間の要素
    Model(&'a Element<ModelPoint>),
    /// 用紙の要素
    Paper(&'a Element<PaperPoint>),
}

impl Found<'_> {
    /// 要素のハッシュ
    pub fn hash(self) -> Hash {
        match self {
            Self::Model(element) => Hash::element(element),
            Self::Paper(element) => Hash::element(element),
        }
    }

    /// 予算と中断の合図を確かめながら要素をハッシュする
    pub fn hash_checked(self, cx: &mut Cx<'_>) -> Result<Hash, Stop> {
        match self {
            Self::Model(element) => Hash::element_checked(element, cx),
            Self::Paper(element) => Hash::element_checked(element, cx),
        }
    }
}

/// 図面全体の要素に ID を振り、要素の位置（空間か用紙と、その中の添字）・ID・要素を `visit` に渡す。
/// 実寸の空間、記号の定義の空間、用紙の順に、それぞれの要素の順にたどる。書き戻しが ID から要素を引く索引と
/// 同じ振り方にする
pub fn each_element<'a, E>(
    drawing: &'a Drawing2D,
    mut visit: impl FnMut(Position, String, Found<'a>) -> Result<(), E>,
) -> Result<(), E> {
    for model in &drawing.spaces {
        let place = space(model.id);
        let mut namer = Namer::new(&place);
        for (index, element) in model.elements.iter().enumerate() {
            let at = Position::Model {
                space: model.id,
                index,
            };
            visit(at, namer.next(index, element), Found::Model(element))?;
        }
    }
    let mut definitions = DefinitionNamer::default();
    for symbol in &drawing.symbols {
        let place = definitions.next(symbol);
        let mut namer = Namer::new(&place);
        for (index, element) in symbol.space.elements.iter().enumerate() {
            let at = Position::Model {
                space: symbol.space.id,
                index,
            };
            visit(at, namer.next(index, element), Found::Model(element))?;
        }
    }
    let mut sheets = SheetNamer::default();
    for paper in &drawing.sheets {
        let place = sheets.next(paper);
        let mut namer = Namer::new(&place);
        for (index, element) in paper.elements.iter().enumerate() {
            let at = Position::Paper {
                sheet: paper.id,
                index,
            };
            visit(at, namer.next(index, element), Found::Paper(element))?;
        }
    }
    Ok(())
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

    #[test]
    fn definitions_and_table_items_are_named_by_handles() {
        use crate::{LayerId, LineTypeId, ModelSpace, SymbolKey, Unit};
        let mut symbol = SymbolDefinition {
            key: SymbolKey {
                namespace: "dxf.block".into(),
                name: "B".to_owned(),
            },
            space: ModelSpace {
                id: SpaceId(3),
                unit: Unit::Millimeter,
                axes: crate::Axes::EastNorth,
                elements: Vec::new(),
                order: Vec::new(),
            },
            base: None,
            attributes: Vec::new(),
            anonymous: false,
            external: None,
            source: Ref::at_record(4),
        };
        assert_eq!(definition(&symbol), "space/3");
        symbol.source = symbol.source.with_key(Key::Handle(0x1f));
        assert_eq!(definition(&symbol), "space/h1F");
        let mut other = symbol.clone();
        other.key.name = "C".to_owned();
        other.space.id = SpaceId(4);
        other.source = Ref::at_record(9).with_key(Key::Handle(0x1f));
        let mut namer = DefinitionNamer::default();
        assert_eq!(namer.next(&symbol), "space/h1F");
        assert_eq!(namer.next(&other), "space/h1F/1");
        assert_eq!(namer.next(&other), "space/h1F/2");
        other.source = Ref::at_record(9);
        assert_eq!(namer.next(&other), "space/4");
        let mut wall = Layer {
            id: LayerId(2),
            name: "Wall".to_owned(),
            state: None,
            style: None,
            follows_placement: false,
            source: Ref::default(),
            description: None,
        };
        assert_eq!(layer(&wall), "layer/2");
        wall.source = Ref::at_record(8).with_key(Key::Handle(0x10));
        assert_eq!(layer(&wall), "layer/h10");
        let dashed = LineType {
            id: LineTypeId(1),
            name: "Dashed".to_owned(),
            definition: None,
            source: Ref::default().with_key(Key::Handle(0x14)),
        };
        assert_eq!(line_type(&dashed), "line-type/h14");
        let mut gothic = TextStyle {
            name: "Gothic".to_owned(),
            definition: None,
            source: Ref::default(),
        };
        assert_eq!(text_style(&gothic, 2), "text-style/2");
        gothic.source = Ref::default().with_key(Key::Handle(0x11));
        assert_eq!(text_style(&gothic, 2), "text-style/h11");
        let mut standard = DimensionStyle {
            name: "Standard".to_owned(),
            ends: [crate::Arrowhead::Default, crate::Arrowhead::Default],
            leader: crate::Arrowhead::Default,
            size: 2.5,
            tick: None,
            scale: 1.0,
            source: Ref::at_record(12),
        };
        assert_eq!(dimension_style(&standard, 1), "dimension-style/1");
        #[cfg(target_pointer_width = "64")]
        assert_eq!(
            dimension_style(&standard, u32::MAX as usize + 1),
            "dimension-style/4294967296"
        );
        standard.source = standard.source.with_key(Key::Handle(0x1a));
        assert_eq!(dimension_style(&standard, 1), "dimension-style/h1A");
    }
}
