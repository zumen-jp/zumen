//! 配置をたどって、要素ごとの変換・解いたスタイル・表示と印刷の可否・切り抜きを返す（決定 0159）。
//!
//! 起点は、空間（実寸の空間か記号の定義の空間）・用紙・用紙のビューのどれか。記号の中の要素は、配置ごとに
//! 返す。格子に並べた記号は、列と行の組ごとに 1 つの配置として返す。
//!
//! 実効のレイヤは決定 0143 の規則で決める。配置のレイヤに従うレイヤなら、外側の配置へ進む。色・線種・線幅と、
//! 表示と印刷に同じレイヤを使う。起点まで配置から継承する値は、文書の既定の値（[`Drawing2D::default_color`]
//! など）で解く。
//!
//! 記号の属性は、記号の要素の次に、記号と同じ空間の要素として返す。属性の継承は、記号の配置で解く。
//!
//! [`Drawing2D::walk_parts`] は、寸法・表・引出線つきの注記を描いた部品（表示のための構成要素）の中へも、
//! 記号と同じく進む（決定 0184）。

use std::collections::BTreeMap;
use std::rc::Rc;

use zumen_base::{Cx, Diagnostic, Kind, Ref, Stop, codes};

use crate::id::{self, DefinitionNamer, Namer, SheetNamer};
use crate::transform::Point;
use crate::{
    Affine, Attribute, CalloutContent, Clip, Color, Drawing2D, Element, Inherit, Inheritor,
    LayerId, Length, LineTypeId, LineWeight, ModelPoint, ModelSpace, PaperLength, PaperPoint,
    Shape, Sheet, SheetId, SheetItem, SpaceId, Style, Symbol, SymbolKey, View,
};

/// たどる起点
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    /// 実寸の空間か、記号の定義の空間
    Space(SpaceId),
    /// 用紙。用紙の要素と、ビューを通した実寸の空間の要素を、`Sheet::order` の順に返す
    Sheet(SheetId),
    /// 用紙のビュー。`view` は `Sheet::views` の添字
    View {
        /// 用紙
        sheet: SheetId,
        /// ビューの添字
        view: usize,
    },
}

/// たどって返す要素
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Item<'a> {
    /// 実寸の空間か記号の定義の空間の要素
    Model(&'a Element<ModelPoint>),
    /// 用紙の要素
    Paper(&'a Element<PaperPoint>),
    /// 実寸の空間か記号の定義の空間の記号の属性
    ModelAttribute {
        /// 属性を持つ記号の要素
        symbol: &'a Element<ModelPoint>,
        /// 記号の属性の添字
        index: usize,
    },
    /// 用紙の記号の属性
    PaperAttribute {
        /// 属性を持つ記号の要素
        symbol: &'a Element<PaperPoint>,
        /// 記号の属性の添字
        index: usize,
    },
}

/// 配置の道筋の 1 段
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Step {
    /// 配置の要素の ID
    pub id: String,
    /// 格子に並べた記号の列と行。格子でなければ `[0, 0]`
    pub cell: [u32; 2],
}

/// 切り抜き。要素は、多角形の内側（`inverted` なら外側）だけを描く
#[derive(Clone, Debug, PartialEq)]
pub struct Clipping {
    /// 起点の座標の、閉じた多角形の頂点
    pub polygon: Vec<[f64; 2]>,
    /// 境界の内側を隠す
    pub inverted: bool,
}

/// たどった要素と、解いた値
#[derive(Clone, Debug, PartialEq)]
pub struct Visit<'a> {
    /// 要素
    pub item: Item<'a>,
    /// 要素の ID（決定 0156）。属性なら、属性を持つ記号の要素の ID
    pub id: String,
    /// 配置の道筋。外側から内側へ
    pub path: Vec<Step>,
    /// 要素の空間から起点の座標への変換
    pub transform: Affine,
    /// 解いた色。レイヤが値を持たなければ `None`
    pub color: Option<Color>,
    /// 解いた線種。レイヤが値を持たなければ `None`
    pub line_type: Option<LineTypeId>,
    /// 解いた線幅。元の形式から読めていないか、レイヤが値を持たないか、文書の既定の線幅がなければ `None`
    pub line_weight: Option<PaperLength>,
    /// 実効のレイヤ
    pub layer: LayerId,
    /// 表示する
    pub visible: bool,
    /// 印刷する（推定。決定 0159）
    pub printable: bool,
    /// 切り抜き。外側から
    pub clips: Vec<Clipping>,
}

impl Affine {
    /// 恒等変換
    pub const IDENTITY: Self = Self {
        m: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
    };

    /// `inner` を当ててから `self` を当てる変換
    pub fn after(&self, inner: &Affine) -> Affine {
        let [a, b, c, d, e, f] = self.m;
        let [p, q, r, s, t, u] = inner.m;
        Affine {
            m: [
                a * p + b * s,
                a * q + b * t,
                a * r + b * u + c,
                d * p + e * s,
                d * q + e * t,
                d * r + e * u + f,
            ],
        }
    }
}

impl<P: Point> Symbol<P> {
    /// 定義の空間から、配置の空間への変換。`cell` は格子の列と行。基点が分からなければ、定義の原点を基点とする
    pub fn placement(&self, base: Option<ModelPoint>, [column, row]: [u32; 2]) -> Affine {
        let [x, y] = self.position.xy();
        let [sx, sy] = self.scale;
        let (bx, by) = base.map_or((0.0, 0.0), |base| (base.x, base.y));
        let (cx, cy) = self.grid.map_or((0.0, 0.0), |grid| {
            let spacing = |length| match length {
                Length::Model(length) => length.value,
                Length::Paper(PaperLength(value)) => value,
            };
            (
                f64::from(column) * spacing(grid.column_spacing),
                f64::from(row) * spacing(grid.row_spacing),
            )
        });
        // 尺度を掛けてから、回転する前の向きで格子の間隔だけ動かし、回して置く
        let scale = Affine {
            m: [sx, 0.0, cx - sx * bx, 0.0, sy, cy - sy * by],
        };
        Affine::translation(x, y).after(&Affine::rotation(0.0, 0.0, self.rotation).after(&scale))
    }

    /// 格子の列と行の組。格子でなければ `[0, 0]` だけ
    fn cells(&self) -> impl Iterator<Item = [u32; 2]> {
        let (columns, rows) = self
            .grid
            .map_or((1, 1), |grid| (grid.columns.max(1), grid.rows.max(1)));
        (0..rows).flat_map(move |row| (0..columns).map(move |column| [column, row]))
    }
}

impl Drawing2D {
    /// 起点から配置をたどって、要素ごとに解いた値を返す（決定 0159）。起点がなければ空。
    /// 返す要素ごとに `work` を 1 と数え、配置の深さを `depth` で数える。道筋に同じ定義がもう一度現れる配置は
    /// たどらずに `drawing.placement.invalid` を返し、定義のない配置はたどらない
    pub fn walk(&self, origin: Origin, cx: &mut Cx<'_>) -> Result<Vec<Visit<'_>>, (Stop, Ref)> {
        self.walk_with(origin, false, cx)
    }

    /// [`walk`](Self::walk) と同じく配置をたどり、寸法・表・引出線つきの注記を描いた部品の中へも進む
    /// （決定 0184）。部品は、その要素を 1 段の配置とする記号の定義として返す。寸法の部品（DXF の名前のない
    /// ブロック）は定義の空間の座標のまま、表の部品は挿入点へ移して、注記の記号は記号の配置で置く
    pub fn walk_parts(
        &self,
        origin: Origin,
        cx: &mut Cx<'_>,
    ) -> Result<Vec<Visit<'_>>, (Stop, Ref)> {
        self.walk_with(origin, true, cx)
    }

    fn walk_with(
        &self,
        origin: Origin,
        parts: bool,
        cx: &mut Cx<'_>,
    ) -> Result<Vec<Visit<'_>>, (Stop, Ref)> {
        let mut walker = Walker {
            drawing: self,
            parts,
            by_name: BTreeMap::new(),
            names: BTreeMap::new(),
            spaces: BTreeMap::new(),
            keys: BTreeMap::new(),
            out: Vec::new(),
        };
        walker.name_definitions(cx)?;
        match origin {
            Origin::Space(id) => {
                if let Some(space) = self.any_space(id) {
                    let top = Frame::top(Affine::IDENTITY, None, Vec::new());
                    walker.space(space, &top, cx)?;
                }
            }
            Origin::Sheet(id) => {
                let Some((sheet, name)) = walker.sheet(id) else {
                    return Ok(Vec::new());
                };
                let top = Frame::top(Affine::IDENTITY, None, Vec::new());
                let ids = walker.ids(&name, &sheet.elements, cx)?;
                for item in &sheet.order {
                    match *item {
                        SheetItem::Element(index) => {
                            if let (Some(element), Some(id)) =
                                (sheet.elements.get(index), ids.get(index))
                            {
                                walker.element(element, id, &top, Item::Paper, cx)?;
                            }
                        }
                        SheetItem::View(index) => {
                            if let Some(view) = sheet.views.get(index) {
                                walker.view(view, cx)?;
                            }
                        }
                    }
                }
            }
            Origin::View { sheet, view } => {
                if let Some(view) = walker.sheet(sheet).and_then(|(s, _)| s.views.get(view)) {
                    walker.view(view, cx)?;
                }
            }
        }
        Ok(walker.out)
    }

    /// ID の実寸の空間か、記号の定義の空間
    fn any_space(&self, id: SpaceId) -> Option<&ModelSpace> {
        self.spaces
            .iter()
            .chain(self.symbols.iter().map(|symbol| &symbol.space))
            .find(|space| space.id == id)
    }

    /// 配置の並び（内側から外側）を通した値を、文書の既定の値で解く
    fn resolve(
        &self,
        chain: &[Inheritor<'_>],
    ) -> (Option<Color>, Option<LineTypeId>, Option<PaperLength>) {
        let color = match self.color_placed(chain) {
            Inherit::Own(color) => Some(color),
            Inherit::Block => self.default_color,
            Inherit::Layer => None,
        };
        let line_type = match self.line_type_placed(chain) {
            Inherit::Own(line_type) => Some(line_type),
            Inherit::Block => self.default_line_type,
            Inherit::Layer => None,
        };
        let line_weight = match self.line_weight_placed(chain) {
            Some(Inherit::Own(LineWeight::Width(width))) => Some(width),
            Some(Inherit::Own(LineWeight::Default) | Inherit::Block) => self.default_line_weight,
            Some(Inherit::Layer) | None => None,
        };
        (color, line_type, line_weight)
    }

    /// 並び（内側から外側）の実効のレイヤ。配置のレイヤに従うレイヤなら、外側へ進む（決定 0143）
    fn effective_layer(&self, layers: &[LayerId]) -> Option<LayerId> {
        let follows = |id: &&LayerId| {
            self.layer(**id)
                .is_some_and(|layer| layer.follows_placement)
        };
        layers
            .iter()
            .find(|id| !follows(id))
            .or(layers.last())
            .copied()
    }

    /// レイヤが凍結している。定義のないレイヤは凍結しない
    fn frozen(&self, id: LayerId) -> bool {
        self.layer(id)
            .and_then(|layer| layer.state)
            .is_some_and(|state| state.frozen)
    }
}

/// 外側の配置
#[derive(Clone, Debug)]
struct Frame<'a> {
    /// この空間から起点への変換
    transform: Affine,
    /// 通しているビュー
    view: Option<&'a View>,
    /// 外側からの切り抜き
    clips: Vec<Clipping>,
    /// 配置を外側から
    placements: Vec<Placed<'a>>,
}

/// 道筋の配置
#[derive(Clone, Debug)]
struct Placed<'a> {
    style: &'a Style,
    layer: LayerId,
    step: Step,
    /// 配置した定義の添字。記号の属性のために足した段なら `None`
    definition: Option<usize>,
}

impl<'a> Frame<'a> {
    fn top(transform: Affine, view: Option<&'a View>, clips: Vec<Clipping>) -> Self {
        Self {
            transform,
            view,
            clips,
            placements: Vec::new(),
        }
    }

    /// 要素か配置のレイヤを内側に足した、レイヤの並び（内側から外側）
    fn layers(&self, inner: LayerId) -> Vec<LayerId> {
        std::iter::once(inner)
            .chain(self.placements.iter().rev().map(|placed| placed.layer))
            .collect()
    }

    /// 要素か配置を内側に足した、継承の並び（内側から外側）
    fn chain(&self, inner: Inheritor<'a>) -> Vec<Inheritor<'a>> {
        std::iter::once(inner)
            .chain(self.placements.iter().rev().map(|placed| Inheritor {
                style: placed.style,
                layer: placed.layer,
            }))
            .collect()
    }
}

struct Walker<'a> {
    drawing: &'a Drawing2D,
    /// 部品の中へも進む
    parts: bool,
    /// 名前ごとの、記号の定義の添字。同じ名前があれば先のもの。寸法の部品は名前だけで指す
    by_name: BTreeMap<&'a str, usize>,
    /// 空間と用紙ごとの、要素の ID
    names: BTreeMap<String, Rc<[String]>>,
    /// 記号の定義の空間ごとの、空間の ID
    spaces: BTreeMap<SpaceId, String>,
    /// キーごとの、記号の定義の添字。同じキーがあれば先のもの
    keys: BTreeMap<&'a SymbolKey, usize>,
    out: Vec<Visit<'a>>,
}

impl<'a> Walker<'a> {
    fn name_definitions(&mut self, cx: &mut Cx<'_>) -> Result<(), (Stop, Ref)> {
        let mut namer = DefinitionNamer::default();
        for (index, definition) in self.drawing.symbols.iter().enumerate() {
            cx.check(Kind::Work, 1)
                .map_err(|stop| (stop, definition.source.clone()))?;
            // 名前は、定義の順に飛ばさずに振る
            let name = namer.next(definition);
            self.spaces.entry(definition.space.id).or_insert(name);
            self.keys.entry(&definition.key).or_insert(index);
            self.by_name
                .entry(definition.key.name.as_str())
                .or_insert(index);
        }
        Ok(())
    }

    fn sheet(&self, id: SheetId) -> Option<(&'a Sheet, String)> {
        let mut namer = SheetNamer::default();
        self.drawing
            .sheets
            .iter()
            .map(|sheet| (sheet, namer.next(sheet)))
            .find(|(sheet, _)| sheet.id == id)
    }

    /// 空間か用紙の要素の ID を、要素の順に振る。一度振ったものは覚えておく
    fn ids<P>(
        &mut self,
        place: &str,
        elements: &'a [Element<P>],
        cx: &mut Cx<'_>,
    ) -> Result<Rc<[String]>, (Stop, Ref)> {
        if let Some(ids) = self.names.get(place) {
            return Ok(Rc::clone(ids));
        }
        let mut namer = Namer::new(place);
        let mut ids = Vec::with_capacity(elements.len());
        for (index, element) in elements.iter().enumerate() {
            cx.check(Kind::Work, 1)
                .map_err(|stop| (stop, element.source.clone()))?;
            ids.push(namer.next(index, element));
        }
        let ids: Rc<[String]> = ids.into();
        self.names.insert(place.to_owned(), Rc::clone(&ids));
        Ok(ids)
    }

    /// 空間の ID の文字列。記号の定義の空間なら、定義の ID
    fn space_name(&self, space: &ModelSpace) -> String {
        self.spaces
            .get(&space.id)
            .cloned()
            .unwrap_or_else(|| id::space(space.id))
    }

    fn view(&mut self, view: &'a View, cx: &mut Cx<'_>) -> Result<(), (Stop, Ref)> {
        let Some(space) = self.drawing.spaces.iter().find(|s| s.id == view.space) else {
            return Ok(());
        };
        let clips = if view.clip.is_empty() {
            Vec::new()
        } else {
            vec![Clipping {
                polygon: view.clip.iter().map(|p| [p.x, p.y]).collect(),
                inverted: false,
            }]
        };
        let top = Frame::top(view.transform, Some(view), clips);
        self.space(space, &top, cx)
    }

    fn space(
        &mut self,
        space: &'a ModelSpace,
        frame: &Frame<'a>,
        cx: &mut Cx<'_>,
    ) -> Result<(), (Stop, Ref)> {
        let name = self.space_name(space);
        let ids = self.ids(&name, &space.elements, cx)?;
        for &index in &space.order {
            if let (Some(element), Some(id)) = (space.elements.get(index), ids.get(index)) {
                self.element(element, id, frame, Item::Model, cx)?;
            }
        }
        Ok(())
    }

    /// 要素を返し、記号なら属性を返して、定義の中へ進む
    fn element<P: Point>(
        &mut self,
        element: &'a Element<P>,
        id: &str,
        frame: &Frame<'a>,
        item: fn(&'a Element<P>) -> Item<'a>,
        cx: &mut Cx<'_>,
    ) -> Result<(), (Stop, Ref)>
    where
        Item<'a>: Attributed<'a, P>,
    {
        let at = || element.source.clone();
        cx.check(Kind::Work, 1).map_err(|stop| (stop, at()))?;
        let mut visit = self.resolved(item(element), id, element.layer, &element.style, frame);
        // 作図の補助の点は印刷しない（決定 0160）
        if let Shape::Point(point) = &element.shape {
            visit.printable &= !point.construction;
        }
        self.out.push(visit);
        if self.parts
            && let Some((definition, placement)) = self.part(&element.shape)
        {
            return self.inside(
                element,
                id,
                frame,
                definition,
                vec![([0, 0], frame.transform.after(&placement))],
                None,
                cx,
            );
        }
        let Shape::Symbol(symbol) = &element.shape else {
            return Ok(());
        };
        for (index, attribute) in symbol.attributes.iter().enumerate() {
            cx.check(Kind::Work, 1).map_err(|stop| (stop, at()))?;
            self.attribute(element, index, attribute, id, frame);
        }
        let Some(&definition) = self.keys.get(&symbol.key) else {
            return Ok(());
        };
        let base = self.drawing.symbols.get(definition).and_then(|d| d.base);
        let cells = symbol
            .cells()
            .map(|cell| (cell, frame.transform.after(&symbol.placement(base, cell))))
            .collect();
        self.inside(
            element,
            id,
            frame,
            definition,
            cells,
            symbol.clip.as_ref(),
            cx,
        )
    }

    /// 寸法・表・引出線つきの注記を描いた部品の、記号の定義の添字と、定義の空間から要素の空間への変換
    fn part<P: Point>(&self, shape: &Shape<P>) -> Option<(usize, Affine)> {
        match shape {
            Shape::Dimension(dimension) => {
                let name = dimension.block.as_deref()?;
                Some((*self.by_name.get(name)?, Affine::IDENTITY))
            }
            Shape::Table(table) => {
                let definition = *self.keys.get(table.drawn.as_ref()?)?;
                let [x, y] = table.position.xy();
                Some((definition, Affine::translation(x, y)))
            }
            Shape::Callout(callout) => match &callout.content {
                CalloutContent::Symbol { symbol, .. } => {
                    let definition = *self.keys.get(&symbol.key)?;
                    let base = self.drawing.symbols.get(definition)?.base;
                    Some((definition, symbol.placement(base, [0, 0])))
                }
                CalloutContent::None | CalloutContent::Text(_) => None,
            },
            _ => None,
        }
    }

    /// 記号の定義の中へ、配置ごとに進む。`cells` は格子の列と行と、定義の空間から起点への変換
    #[allow(clippy::too_many_arguments)]
    fn inside<P: Point>(
        &mut self,
        element: &'a Element<P>,
        id: &str,
        frame: &Frame<'a>,
        definition: usize,
        cells: Vec<([u32; 2], Affine)>,
        clip: Option<&'a Clip>,
        cx: &mut Cx<'_>,
    ) -> Result<(), (Stop, Ref)> {
        let at = || element.source.clone();
        if frame
            .placements
            .iter()
            .any(|placed| placed.definition == Some(definition))
        {
            cx.report(
                Diagnostic::new(codes::DRAWING_PLACEMENT_INVALID)
                    .with("id", id.to_owned())
                    .at(at()),
            );
            return Ok(());
        }
        let Some(symbols) = self.drawing.symbols.get(definition) else {
            return Ok(());
        };
        cx.enter().map_err(|stop| (stop, at()))?;
        for (cell, placement) in cells {
            let mut clips = frame.clips.clone();
            if let Some(clip) = clip {
                let corners = match clip.vertices.as_slice() {
                    &[[x0, y0], [x1, y1]] => vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1]],
                    vertices => vertices.to_vec(),
                };
                clips.push(Clipping {
                    polygon: corners
                        .into_iter()
                        .map(|[x, y]| {
                            let (x, y) = placement.apply(x, y);
                            [x, y]
                        })
                        .collect(),
                    inverted: clip.inverted,
                });
            }
            let mut placements = frame.placements.clone();
            placements.push(Placed {
                style: &element.style,
                layer: element.layer,
                step: Step {
                    id: id.to_owned(),
                    cell,
                },
                definition: Some(definition),
            });
            let inner = Frame {
                transform: placement,
                view: frame.view,
                clips,
                placements,
            };
            if let Err(error) = self.space(&symbols.space, &inner, cx) {
                cx.leave();
                return Err(error);
            }
        }
        cx.leave();
        Ok(())
    }

    fn attribute<P: Point>(
        &mut self,
        element: &'a Element<P>,
        index: usize,
        attribute: &'a Attribute<P>,
        id: &str,
        frame: &Frame<'a>,
    ) where
        Item<'a>: Attributed<'a, P>,
    {
        // 属性は記号の配置から継承する。記号を一段の配置として、外側に足す
        let mut placements = frame.placements.clone();
        placements.push(Placed {
            style: &element.style,
            layer: element.layer,
            step: Step {
                id: id.to_owned(),
                cell: [0, 0],
            },
            definition: None,
        });
        let inner = Frame {
            placements,
            ..frame.clone()
        };
        let mut visit = self.resolved(
            Item::attribute(element, index),
            id,
            attribute.layer,
            &attribute.style,
            &inner,
        );
        // 道筋は、属性を持つ記号の外側の配置まで
        visit.path.pop();
        visit.visible &= !attribute.hidden;
        visit.printable &= !attribute.hidden;
        self.out.push(visit);
    }

    fn resolved(
        &self,
        item: Item<'a>,
        id: &str,
        layer: LayerId,
        style: &'a Style,
        frame: &Frame<'a>,
    ) -> Visit<'a> {
        let drawing = self.drawing;
        let layers = frame.layers(layer);
        let effective = drawing.effective_layer(&layers).unwrap_or(layer);
        let chain = frame.chain(Inheritor { style, layer });
        let (color, line_type, line_weight) = drawing.resolve(&chain);
        // 配置ごとの実効のレイヤ。外側の配置から順に、そこより外側の並びで決める
        let count = frame.placements.len();
        let placement_layers: Vec<LayerId> = (0..count)
            .filter_map(|at| drawing.effective_layer(layers.get(count - at..)?))
            .collect();
        let hidden = |id: &LayerId| {
            frame
                .view
                .is_some_and(|view| view.hidden_layers.contains(id))
        };
        let state = drawing.layer(effective).and_then(|layer| layer.state);
        let visible = placement_layers
            .iter()
            .all(|id| !drawing.frozen(*id) && !hidden(id))
            && state.is_none_or(|state| state.visible && !state.frozen)
            && !hidden(&effective);
        let printable = visible && state.is_none_or(|state| state.printable);
        Visit {
            item,
            id: id.to_owned(),
            path: frame
                .placements
                .iter()
                .map(|placed| placed.step.clone())
                .collect(),
            transform: frame.transform,
            color,
            line_type,
            line_weight,
            layer: effective,
            visible,
            printable,
            clips: frame.clips.clone(),
        }
    }
}

/// 記号の属性を指す要素
trait Attributed<'a, P> {
    fn attribute(symbol: &'a Element<P>, index: usize) -> Item<'a>;
}

impl<'a> Attributed<'a, ModelPoint> for Item<'a> {
    fn attribute(symbol: &'a Element<ModelPoint>, index: usize) -> Item<'a> {
        Item::ModelAttribute { symbol, index }
    }
}

impl<'a> Attributed<'a, PaperPoint> for Item<'a> {
    fn attribute(symbol: &'a Element<PaperPoint>, index: usize) -> Item<'a> {
        Item::PaperAttribute { symbol, index }
    }
}

#[cfg(test)]
mod tests {
    use zumen_base::{Budget, CancelToken, Diagnostics, Limit, Limits};

    use super::*;
    use crate::{
        Clip, ColorCode, ColorSystem, Grid, Height, HeightMeaning, Horizontal, Layer, LayerState,
        LayerStyle, ModelLength, Placement, SymbolDefinition, Text, Vertical,
    };

    const MODEL: SpaceId = SpaceId(0);
    const INNER: SpaceId = SpaceId(1);
    const OUTER: SpaceId = SpaceId(2);
    const SHEET: SheetId = SheetId(0);

    fn aci(value: u32) -> Color {
        Color {
            code: Some(ColorCode {
                system: ColorSystem::Aci,
                value,
            }),
            rgb: None,
        }
    }

    fn at(space: SpaceId, x: f64, y: f64) -> ModelPoint {
        ModelPoint { space, x, y }
    }

    fn block() -> Style {
        Style::BLOCK
    }

    fn by_layer() -> Style {
        Style {
            color: Inherit::Layer,
            line_type: Inherit::Layer,
            line_type_scale: 1.0,
            line_weight: Some(Inherit::Layer),
            transparency: Inherit::Layer,
        }
    }

    fn line(space: SpaceId, layer: u32, style: Style) -> Element<ModelPoint> {
        Element {
            source: Ref::default(),
            layer: LayerId(layer),
            style,
            shape: Shape::Line {
                start: at(space, 0.0, 0.0),
                end: at(space, 1.0, 0.0),
            },
        }
    }

    fn insert(space: SpaceId, name: &str, layer: u32, style: Style, x: f64) -> Element<ModelPoint> {
        Element {
            source: Ref::default(),
            layer: LayerId(layer),
            style,
            shape: Shape::Symbol(Symbol {
                key: SymbolKey {
                    namespace: "dxf.block".into(),
                    name: name.to_owned(),
                },
                position: at(space, x, 0.0),
                scale: [2.0, 2.0],
                rotation: 0.0,
                grid: None,
                attributes: Vec::new(),
                clip: None,
            }),
        }
    }

    fn space(id: SpaceId, elements: Vec<Element<ModelPoint>>) -> ModelSpace {
        ModelSpace {
            id,
            unit: crate::Unit::Millimeter,
            axes: crate::Axes::EastNorth,
            order: (0..elements.len()).collect(),
            elements,
        }
    }

    fn definition(name: &str, space: ModelSpace) -> SymbolDefinition {
        SymbolDefinition {
            key: SymbolKey {
                namespace: "dxf.block".into(),
                name: name.to_owned(),
            },
            base: Some(at(space.id, 0.0, 0.0)),
            space,
            attributes: Vec::new(),
            anonymous: false,
            external: None,
            source: Ref::default(),
        }
    }

    /// レイヤ 0 は配置のレイヤに従い、色 7。レイヤ 1 は色 1、2 は色 2、3 は値を持たない
    fn layer(id: u32, color: Option<u32>, state: LayerState) -> Layer {
        Layer {
            id: LayerId(id),
            name: id.to_string(),
            state: Some(state),
            style: Some(LayerStyle {
                color: color.map(aci),
                line_type: Some(LineTypeId(id)),
                line_weight: Some(LineWeight::Width(PaperLength(0.1 * f64::from(id + 1)))),
                transparency: None,
            }),
            follows_placement: id == 0,
            source: Ref::default(),
            description: None,
        }
    }

    const ON: LayerState = LayerState {
        visible: true,
        printable: true,
        editable: true,
        frozen: false,
    };

    /// 実寸の空間に OUTER の配置（レイヤ 1、色はレイヤ）、OUTER の中に INNER の配置（レイヤ 0、色は配置）、
    /// INNER の中に、配置から継承する線（レイヤ 0）と、色 2 のレイヤ 2 の線
    fn nested() -> Drawing2D {
        Drawing2D {
            spaces: vec![space(
                MODEL,
                vec![insert(MODEL, "OUTER", 1, by_layer(), 10.0)],
            )],
            layers: vec![
                layer(0, Some(7), ON),
                layer(1, Some(1), ON),
                layer(2, Some(2), ON),
                layer(3, None, ON),
            ],
            symbols: vec![
                definition(
                    "INNER",
                    space(
                        INNER,
                        vec![line(INNER, 0, block()), line(INNER, 2, by_layer())],
                    ),
                ),
                definition(
                    "OUTER",
                    space(OUTER, vec![insert(OUTER, "INNER", 0, block(), 1.0)]),
                ),
            ],
            default_color: Some(aci(7)),
            default_line_type: Some(LineTypeId(9)),
            default_line_weight: Some(PaperLength(0.25)),
            ..Drawing2D::default()
        }
    }

    fn walk(drawing: &Drawing2D, origin: Origin) -> (Vec<Visit<'_>>, Diagnostics) {
        let mut budget = Budget::default();
        let mut diag = Diagnostics::new();
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        let visits = drawing.walk(origin, &mut cx).unwrap();
        (visits, diag)
    }

    fn lines<'v>(visits: &'v [Visit<'_>]) -> Vec<&'v Visit<'v>> {
        visits
            .iter()
            .filter(|visit| {
                matches!(
                    visit.item,
                    Item::Model(Element {
                        shape: Shape::Line { .. },
                        ..
                    })
                )
            })
            .collect()
    }

    #[test]
    fn nested_placements_compose_and_inherit_from_the_outer_placement() {
        let drawing = nested();
        let (visits, diag) = walk(&drawing, Origin::Space(MODEL));
        assert_eq!(diag.iter().count(), 0);
        // 外の配置・中の配置・2 本の線
        assert_eq!(visits.len(), 4);
        let found = lines(&visits);
        let [byblock, own] = found.as_slice() else {
            panic!("{visits:?}");
        };
        // 外の配置は (10,0) に 2 倍、中の配置は外の定義の (1,0) に 2 倍。定義の (1,0) は (10 + 2 (1 + 2), 0)
        assert_eq!(byblock.transform.apply(1.0, 0.0), (16.0, 0.0));
        assert_eq!(
            byblock
                .path
                .iter()
                .map(|s| s.id.as_str())
                .collect::<Vec<_>>(),
            ["space/0/e0", "space/2/e0"]
        );
        assert_eq!(byblock.id, "space/1/e0");
        // 配置から継承する線は、中の配置（BYBLOCK）を通して外の配置のレイヤ 1 の色と線種と線幅になる。
        // レイヤ 0 は配置のレイヤに従うので、実効のレイヤも 1
        assert_eq!(byblock.color, Some(aci(1)));
        assert_eq!(byblock.line_type, Some(LineTypeId(1)));
        assert_eq!(byblock.line_weight, Some(PaperLength(0.2)));
        assert_eq!(byblock.layer, LayerId(1));
        // レイヤ 2 の線は、自分のレイヤで解く
        assert_eq!(own.color, Some(aci(2)));
        assert_eq!(own.layer, LayerId(2));
        assert!(own.visible && own.printable);
    }

    #[test]
    fn construction_points_are_shown_but_not_printed() {
        let point = |construction| Element {
            source: Ref::default(),
            layer: LayerId(0),
            style: by_layer(),
            shape: Shape::Point(crate::Point {
                position: at(MODEL, 0.0, 0.0),
                rotation: 0.0,
                look: crate::PointLook::Document,
                construction,
            }),
        };
        let drawing = Drawing2D {
            spaces: vec![space(MODEL, vec![point(false), point(true)])],
            ..Drawing2D::default()
        };
        let (visits, _) = walk(&drawing, Origin::Space(MODEL));
        let shown: Vec<_> = visits.iter().map(|v| (v.visible, v.printable)).collect();
        assert_eq!(shown, [(true, true), (true, false)]);
    }

    #[test]
    fn inheritance_left_at_the_origin_uses_the_document_defaults() {
        let mut drawing = nested();
        // 定義の空間から直接たどると、配置から継承する値は文書の既定の値になる
        let (visits, _) = walk(&drawing, Origin::Space(INNER));
        let byblock = lines(&visits)[0];
        assert_eq!(byblock.color, Some(aci(7)));
        assert_eq!(byblock.line_type, Some(LineTypeId(9)));
        assert_eq!(byblock.line_weight, Some(PaperLength(0.25)));
        assert_eq!(byblock.layer, LayerId(0));
        assert!(byblock.path.is_empty());
        // 値を持たないレイヤから継承する色は解かない
        drawing.spaces[0].elements.push(line(MODEL, 3, by_layer()));
        drawing.spaces[0].order.push(1);
        let (visits, _) = walk(&drawing, Origin::Space(MODEL));
        let last = visits.last().unwrap();
        assert_eq!(last.color, None);
        assert_eq!(last.line_type, Some(LineTypeId(3)));
    }

    #[test]
    fn a_frozen_placement_hides_its_contents_and_an_off_one_hides_only_followers() {
        let mut drawing = nested();
        drawing.layers[1].state = Some(LayerState {
            visible: false,
            ..ON
        });
        let (visits, _) = walk(&drawing, Origin::Space(MODEL));
        let found = lines(&visits);
        // オフのレイヤの配置は、配置のレイヤに従う線だけを隠す
        assert!(!found[0].visible && !found[0].printable);
        assert!(found[1].visible);
        drawing.layers[1].state = Some(LayerState {
            visible: false,
            frozen: true,
            ..ON
        });
        let (visits, _) = walk(&drawing, Origin::Space(MODEL));
        assert!(lines(&visits).iter().all(|visit| !visit.visible));
        // 印刷しないレイヤの要素は、表示しても印刷しない
        drawing.layers[1].state = Some(ON);
        drawing.layers[2].state = Some(LayerState {
            printable: false,
            ..ON
        });
        let (visits, _) = walk(&drawing, Origin::Space(MODEL));
        let found = lines(&visits);
        assert!(found[1].visible && !found[1].printable);
    }

    #[test]
    fn a_sheet_walks_its_elements_and_views_with_clips_and_hidden_layers() {
        let mut drawing = nested();
        let corner = |x, y| PaperPoint { sheet: SHEET, x, y };
        drawing.sheets.push(Sheet {
            id: SHEET,
            name: None,
            size: None,
            elements: vec![Element {
                source: Ref::default(),
                layer: LayerId(2),
                style: by_layer(),
                shape: Shape::Line {
                    start: corner(0.0, 0.0),
                    end: corner(1.0, 0.0),
                },
            }],
            views: vec![View {
                space: MODEL,
                sheet: SHEET,
                transform: Affine {
                    m: [0.5, 0.0, 100.0, 0.0, 0.5, 50.0],
                },
                clip: vec![corner(0.0, 0.0), corner(10.0, 0.0), corner(10.0, 10.0)],
                hidden_layers: vec![LayerId(2)],
                source: Ref::default(),
            }],
            order: vec![SheetItem::Element(0), SheetItem::View(0)],
            source: Ref::default(),
        });
        // 外の配置を、定義の (0,0)〜(1,1) で切り抜く
        let Shape::Symbol(symbol) = &mut drawing.spaces[0].elements[0].shape else {
            panic!("記号の要素")
        };
        symbol.clip = Some(Clip {
            vertices: vec![[0.0, 0.0], [1.0, 1.0]],
            inverted: false,
        });
        let (visits, _) = walk(&drawing, Origin::Sheet(SHEET));
        assert!(matches!(visits[0].item, Item::Paper(_)));
        assert_eq!(visits[0].id, "sheet/0/e0");
        assert!(visits[0].visible);
        let found = lines(&visits);
        // ビューの変換を通す。定義の (1,0) は実寸の (16,0)、用紙の (108,50)
        assert_eq!(found[0].transform.apply(1.0, 0.0), (108.0, 50.0));
        // ビューの切り抜きと、外の配置の切り抜き（用紙の座標の長方形）を外側から
        assert_eq!(found[0].clips.len(), 2);
        assert_eq!(found[0].clips[0].polygon.len(), 3);
        assert_eq!(
            found[0].clips[1].polygon,
            [[105.0, 50.0], [106.0, 50.0], [106.0, 51.0], [105.0, 51.0]]
        );
        // ビューで隠すレイヤ 2 の線は表示しない。レイヤ 1 に従う線は表示する
        assert!(found[0].visible);
        assert!(!found[1].visible);
        // ビューだけを起点にすると、用紙の要素は返さない
        let (visits, _) = walk(
            &drawing,
            Origin::View {
                sheet: SHEET,
                view: 0,
            },
        );
        assert!(
            visits
                .iter()
                .all(|visit| matches!(visit.item, Item::Model(_)))
        );
        // 起点がなければ空
        let (visits, _) = walk(
            &drawing,
            Origin::View {
                sheet: SHEET,
                view: 1,
            },
        );
        assert!(visits.is_empty());
    }

    #[test]
    fn grids_return_each_cell_and_attributes_inherit_from_their_placement() {
        let mut drawing = nested();
        let Shape::Symbol(symbol) = &mut drawing.spaces[0].elements[0].shape else {
            panic!("記号の要素")
        };
        symbol.grid = Some(Grid {
            columns: 2,
            rows: 1,
            column_spacing: Length::Model(ModelLength {
                space: MODEL,
                value: 100.0,
            }),
            row_spacing: Length::Model(ModelLength {
                space: MODEL,
                value: 0.0,
            }),
        });
        let text = |hidden| Attribute {
            tag: "TAG".to_owned(),
            text: Text {
                content: "A".to_owned(),
                height: Height {
                    value: Length::Model(ModelLength {
                        space: MODEL,
                        value: 1.0,
                    }),
                    meaning: HeightMeaning::Frame,
                },
                placement: Placement::Anchor {
                    point: at(MODEL, 0.0, 0.0),
                    horizontal: Horizontal::Left,
                    vertical: Vertical::Baseline,
                },
                rotation: 0.0,
                width_factor: 1.0,
                oblique: 0.0,
                backward: false,
                upside_down: false,
                style: String::new(),
            },
            formatted: None,
            hidden,
            layer: LayerId(0),
            style: block(),
            source: Ref::default(),
        };
        symbol.attributes = vec![text(false), text(true)];
        let (visits, _) = walk(&drawing, Origin::Space(MODEL));
        let found = lines(&visits);
        assert_eq!(found.len(), 4);
        // 2 つ目の列は、回転する前の向きに 100 だけずれる
        assert_eq!(found[2].transform.apply(1.0, 0.0), (116.0, 0.0));
        assert_eq!(found[2].path[0].cell, [1, 0]);
        let attributes: Vec<_> = visits
            .iter()
            .filter(|visit| matches!(visit.item, Item::ModelAttribute { .. }))
            .collect();
        assert_eq!(attributes.len(), 2);
        // 属性は記号と同じ空間にあり、記号の配置（レイヤ 1）から継承する
        assert_eq!(attributes[0].id, "space/0/e0");
        assert!(attributes[0].path.is_empty());
        assert_eq!(attributes[0].transform, Affine::IDENTITY);
        assert_eq!(attributes[0].color, Some(aci(1)));
        assert_eq!(attributes[0].layer, LayerId(1));
        assert!(attributes[0].visible);
        assert!(!attributes[1].visible);
    }

    #[test]
    fn cycles_are_reported_and_not_walked() {
        let mut drawing = nested();
        drawing.symbols[0]
            .space
            .elements
            .push(insert(INNER, "OUTER", 0, block(), 0.0));
        drawing.symbols[0].space.order.push(2);
        let (visits, diag) = walk(&drawing, Origin::Space(MODEL));
        let codes: Vec<_> = diag.iter().map(|d| d.code().name()).collect();
        assert_eq!(codes, ["drawing.placement.invalid"]);
        // 外の配置・中の配置・2 本の線・循環する配置。循環する配置の中はたどらない
        assert_eq!(visits.len(), 5);
    }

    #[test]
    fn work_and_depth_are_limited() {
        let drawing = nested();
        let cancel = CancelToken::new();
        for (kind, limit) in [(Kind::Work, 3), (Kind::Depth, 1)] {
            let mut budget = Budget::new(Limits::default().with(kind, Limit::Finite(limit)));
            let mut diag = Diagnostics::new();
            let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
            let (stop, _) = drawing.walk(Origin::Space(MODEL), &mut cx).unwrap_err();
            assert!(matches!(stop, Stop::Exceeded(_)), "{kind:?}");
            assert_eq!(budget.used(Kind::Depth), 0, "{kind:?}");
        }
    }
}
