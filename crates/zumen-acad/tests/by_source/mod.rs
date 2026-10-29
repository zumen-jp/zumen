//! 出所で要素を指す変更の一覧を、ID と読んだときのハッシュで指す変更の一覧（決定 0139・0156）に写す。
//! 出所で書いた試験を、そのまま新しい形の apply で確かめるための補助。
#![cfg(test)]

use std::collections::BTreeMap;

use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Ref};
use zumen_drawing::{
    Drawing2D, Edge, Element, Hash, ModelPoint, PaperPoint, Placed, Placement, Seen, Shape, id,
};
use zumen_sdk::{Error, Lift};

/// 出所で要素を指す変更の一覧
#[derive(Clone, Debug, Default)]
pub struct Changes {
    pub edits: Vec<Edit>,
}

/// 出所で要素を指す変更
#[derive(Clone, Debug)]
pub enum Edit {
    /// 同じ出所の実寸の空間の要素を置き換える
    Replace(Box<Element<ModelPoint>>),
    /// 同じ出所の用紙の要素を置き換える
    ReplaceOnSheet(Box<Element<PaperPoint>>),
    /// この出所の要素を除く
    Remove(Ref),
    /// 実寸の空間（`space/0`）に足す
    Add(Box<Element<ModelPoint>>),
    /// 要素の点の用紙に足す
    AddOnSheet(Box<Element<PaperPoint>>),
}

/// 出所で指す変更の一覧を当てる
pub trait Apply {
    fn apply(&mut self, changes: &Changes, cx: &mut Cx) -> Result<(), Error>;
}

impl<F> Apply for F
where
    F: zumen_sdk::Apply<zumen_drawing::Changes, Output = Vec<zumen_drawing::Applied>>
        + Lift<Drawing2D>,
{
    fn apply(&mut self, changes: &Changes, cx: &mut Cx) -> Result<(), Error> {
        let changes = convert(self, changes);
        zumen_sdk::Apply::apply(self, &changes, cx).map(|_| ())
    }
}

/// 今のファイルの要素を出所で引き、ID と今のハッシュにする。ない出所は、ID の規則で作った ID と 0 の
/// ハッシュにする（apply は、ないと答える）
pub fn convert<F: Lift<Drawing2D>>(file: &F, changes: &Changes) -> zumen_drawing::Changes {
    let mut budget = Budget::default();
    let mut quiet = Diagnostics::new();
    let drawing = file
        .lift(&mut Cx::new(&mut budget, &mut quiet, &CancelToken::new()))
        .unwrap();
    let mut seen: BTreeMap<Ref, Seen> = BTreeMap::new();
    for space in &drawing.spaces {
        let place = id::space(space.id);
        let mut namer = id::Namer::new(&place);
        for (index, element) in space.elements.iter().enumerate() {
            let id = namer.next(index, element);
            seen.entry(element.source.clone()).or_insert(Seen {
                id,
                hash: Hash::element(element),
            });
        }
    }
    for sheet in &drawing.sheets {
        let place = id::sheet(sheet);
        let mut namer = id::Namer::new(&place);
        for (index, element) in sheet.elements.iter().enumerate() {
            let id = namer.next(index, element);
            seen.entry(element.source.clone()).or_insert(Seen {
                id,
                hash: Hash::element(element),
            });
        }
    }
    let target = |source: &Ref| {
        seen.get(source).cloned().unwrap_or_else(|| Seen {
            id: format!("space/0/{}", id::origin(source, 0)),
            hash: Hash([0; 16]),
        })
    };
    let sheet = |element: &Element<PaperPoint>| {
        let at = anchor(&element.shape).map(|at| at.sheet);
        drawing
            .sheets
            .iter()
            .find(|sheet| Some(sheet.id) == at)
            .map_or_else(|| "sheet/none".to_owned(), id::sheet)
    };
    let edits = changes
        .edits
        .iter()
        .map(|edit| match edit {
            Edit::Replace(element) => zumen_drawing::Edit::Replace {
                target: target(&element.source),
                element: Box::new(Placed::Model((**element).clone())),
            },
            Edit::ReplaceOnSheet(element) => zumen_drawing::Edit::Replace {
                target: target(&element.source),
                element: Box::new(Placed::Paper((**element).clone())),
            },
            Edit::Remove(at) => zumen_drawing::Edit::Remove { target: target(at) },
            Edit::Add(element) => zumen_drawing::Edit::Add {
                place: "space/0".to_owned(),
                element: Box::new(Placed::Model((**element).clone())),
            },
            Edit::AddOnSheet(element) => zumen_drawing::Edit::Add {
                place: sheet(element),
                element: Box::new(Placed::Paper((**element).clone())),
            },
        })
        .collect();
    zumen_drawing::Changes {
        edits,
        document: None,
    }
}

/// 要素の最初の点
fn anchor<P: Copy>(shape: &Shape<P>) -> Option<P> {
    match shape {
        Shape::Line { start, .. } => Some(*start),
        Shape::Circle { center, .. } => Some(*center),
        Shape::Curve(Edge::Arc { center, .. } | Edge::Elliptic { center, .. }) => Some(*center),
        Shape::Polyline(line) => line.vertices.first().map(|v| v.point),
        Shape::Text(text) => match &text.placement {
            Placement::Anchor { point, .. } => Some(*point),
            Placement::Between { first, .. } => Some(*first),
        },
        Shape::Formatted(text) => Some(text.point),
        Shape::Symbol(symbol) => Some(symbol.position),
        _ => None,
    }
}
