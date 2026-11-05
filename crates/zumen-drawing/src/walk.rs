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

use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use zumen_base::{Cx, Diagnostic, Kind, Ref, Stop, codes};

use crate::id::{self, DefinitionNamer, Namer, SheetNamer};
use crate::transform::Point;
use crate::{
    Affine, Attribute, CalloutContent, Clip, Color, Drawing2D, Element, Inherit, Layer, LayerId,
    Length, LineTypeId, LineWeight, ModelPoint, ModelSpace, PaperLength, PaperPoint, Shape, Sheet,
    SheetId, SheetItem, SpaceId, Style, Symbol, SymbolKey, View,
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

/// 逐次処理で借りて受け取る要素。道筋と切り抜きを要素ごとに複製しない（決定 0264）。
#[derive(Clone, Copy, Debug)]
pub struct VisitRef<'a, 'v> {
    /// 要素
    pub item: Item<'a>,
    /// 要素の ID
    pub id: &'v str,
    /// 配置の道筋。外側から内側へ
    pub path: &'v [Step],
    /// 起点への変換
    pub transform: Affine,
    /// 解いた色
    pub color: Option<Color>,
    /// 解いた線種
    pub line_type: Option<LineTypeId>,
    /// 解いた線幅
    pub line_weight: Option<PaperLength>,
    /// 実効のレイヤ
    pub layer: LayerId,
    /// 実効のレイヤの定義。ID が重なれば先のもの
    pub layer_definition: Option<&'a Layer>,
    /// 表示する
    pub visible: bool,
    /// 印刷する
    pub printable: bool,
    /// 起点の座標の切り抜き
    pub clips: &'v [Clipping],
}

impl<'a> VisitRef<'a, '_> {
    fn owned(&self) -> Visit<'a> {
        Visit {
            item: self.item,
            id: self.id.to_owned(),
            path: self.path.to_vec(),
            transform: self.transform,
            color: self.color,
            line_type: self.line_type,
            line_weight: self.line_weight,
            layer: self.layer,
            visible: self.visible,
            printable: self.printable,
            clips: self.clips.to_vec(),
        }
    }
}

/// 配置をたどるときの受け取り側。走査と結果を作る処理が、同じ予算と診断を使う。
pub trait WalkVisitor<'a> {
    /// 予算・診断・中断の合図を借りる
    fn context(&mut self) -> Cx<'_>;
    /// 走査の診断。複数回たどる受け取り側は、最初の走査だけで記録してよい
    fn diagnostic(&mut self, diagnostic: Diagnostic) {
        self.context().report(diagnostic);
    }
    /// 要素を 1 つ受け取る。結果に残す量を、受け取り側で予算に数える
    fn visit(&mut self, visit: VisitRef<'a, '_>) -> Result<(), (Stop, Ref)>;
}

struct Collect<'a, 'c, 'x> {
    cx: &'c mut Cx<'x>,
    out: Vec<Visit<'a>>,
}
impl<'a> WalkVisitor<'a> for Collect<'a, '_, '_> {
    fn context(&mut self) -> Cx<'_> {
        Cx::new(self.cx.budget, self.cx.diag, self.cx.cancel)
    }
    fn visit(&mut self, visit: VisitRef<'a, '_>) -> Result<(), (Stop, Ref)> {
        // ポインターの幅によらない見積もり。道筋・切り抜きも、写す前に数える。
        let bytes = 256_u64
            .saturating_add(visit.id.len() as u64)
            .saturating_add(
                visit
                    .path
                    .iter()
                    .map(|s| 64 + s.id.len() as u64)
                    .sum::<u64>(),
            )
            .saturating_add(
                visit
                    .clips
                    .iter()
                    .map(|c| 32 + c.polygon.len() as u64 * 16)
                    .sum::<u64>(),
            );
        self.cx
            .check(Kind::DecodedBytes, bytes)
            .map_err(|s| (s, Ref::default().with_id(visit.id)))?;
        self.out.push(visit.owned());
        Ok(())
    }
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
    /// 返す要素は `elements` と `work`、配置の深さは `depth`、集める結果は `decoded_bytes` で数える。
    /// ID・描く順・レイヤ・ビューは走査中に覚え、配置の継承は親から渡す（決定 0264）。
    /// 道筋に同じ定義がもう一度現れる配置はたどらず、`drawing.placement.invalid` を返す。
    /// 定義のない配置と、返す要素のない定義の格子はたどらない。
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
        let mut visitor = Collect {
            cx,
            out: Vec::new(),
        };
        self.walk_to(origin, parts, &mut visitor)?;
        Ok(visitor.out)
    }

    /// 要素を集めず、順に受け取り側へ渡す（決定 0264）。`parts` なら、描いた部品の中もたどる。
    /// 既定の予算を変えず、走査の記憶は入力と配置の深さで決まり、返した要素の総数には依存しない。返す要素と ID は `walk` と同じ。
    pub fn walk_to<'a>(
        &'a self,
        origin: Origin,
        parts: bool,
        visitor: &mut dyn WalkVisitor<'a>,
    ) -> Result<(), (Stop, Ref)> {
        let mut walker = Walker {
            drawing: self,
            parts,
            by_name: BTreeMap::new(),
            names: BTreeMap::new(),
            orders: BTreeMap::new(),
            spaces: BTreeMap::new(),
            keys: BTreeMap::new(),
            layers: BTreeMap::new(),
            model_spaces: BTreeMap::new(),
            views: BTreeMap::new(),
            visitor,
        };
        walker.name_definitions()?;
        for layer in &self.layers {
            walker
                .visitor
                .context()
                .check(Kind::Work, 1)
                .map_err(|s| (s, layer.source.clone()))?;
            walker.layers.entry(layer.id).or_insert(layer);
        }
        for space in &self.spaces {
            walker
                .visitor
                .context()
                .check(Kind::Work, 1)
                .map_err(|s| (s, Ref::default()))?;
            walker.model_spaces.entry(space.id).or_insert(space);
        }
        match origin {
            Origin::Space(id) => {
                let space = walker.model_spaces.get(&id).copied().or_else(|| {
                    self.symbols
                        .iter()
                        .map(|symbol| &symbol.space)
                        .find(|space| space.id == id)
                });
                if let Some(space) = space {
                    walker.space(space, &Frame::top(Affine::IDENTITY))?;
                }
            }
            Origin::Sheet(id) => {
                let Some((sheet, name)) = walker.sheet(id)? else {
                    return Ok(());
                };
                let top = Frame::top(Affine::IDENTITY);
                let ids = walker.ids(&name, &sheet.elements)?;
                for item in &sheet.order {
                    walker
                        .visitor
                        .context()
                        .check(Kind::Work, 1)
                        .map_err(|s| (s, sheet.source.clone()))?;
                    match *item {
                        SheetItem::Element(index) => {
                            if let (Some(element), Some(id)) =
                                (sheet.elements.get(index), ids.get(index))
                            {
                                walker.element(element, id, &top, Item::Paper)?;
                            }
                        }
                        SheetItem::View(index) => {
                            if let Some(view) = sheet.views.get(index) {
                                walker.view(view)?;
                            }
                        }
                    }
                }
            }
            Origin::View { sheet, view } => {
                if let Some(view) = walker.sheet(sheet)?.and_then(|(s, _)| s.views.get(view)) {
                    walker.view(view)?;
                }
            }
        }
        Ok(())
    }
}

/// 外側の配置。継承と表示の状態は、配置ごとに一度だけ求める。
#[derive(Clone, Debug)]
struct Frame {
    transform: Affine,
    clips: Rc<[Clipping]>,
    path: Rc<[Step]>,
    definitions: Rc<[usize]>,
    hidden: Rc<BTreeSet<LayerId>>,
    layer: Option<LayerId>,
    color: Option<Color>,
    line_type: Option<LineTypeId>,
    line_weight: Option<PaperLength>,
    visible: bool,
}
impl Frame {
    fn top(transform: Affine) -> Self {
        Self {
            transform,
            clips: Rc::from([]),
            path: Rc::from([]),
            definitions: Rc::from([]),
            hidden: Rc::new(BTreeSet::new()),
            layer: None,
            color: None,
            line_type: None,
            line_weight: None,
            visible: true,
        }
    }
}

struct Walker<'a, 'v> {
    drawing: &'a Drawing2D,
    parts: bool,
    by_name: BTreeMap<&'a str, usize>,
    names: BTreeMap<String, Rc<[String]>>,
    orders: BTreeMap<*const ModelSpace, Rc<[usize]>>,
    spaces: BTreeMap<SpaceId, String>,
    keys: BTreeMap<&'a SymbolKey, usize>,
    layers: BTreeMap<LayerId, &'a Layer>,
    model_spaces: BTreeMap<SpaceId, &'a ModelSpace>,
    views: BTreeMap<*const View, Frame>,
    visitor: &'v mut dyn WalkVisitor<'a>,
}

impl<'a> Walker<'a, '_> {
    fn name_definitions(&mut self) -> Result<(), (Stop, Ref)> {
        let mut namer = DefinitionNamer::default();
        for (index, definition) in self.drawing.symbols.iter().enumerate() {
            self.visitor
                .context()
                .check(Kind::Work, 1)
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

    fn sheet(&mut self, id: SheetId) -> Result<Option<(&'a Sheet, String)>, (Stop, Ref)> {
        let mut namer = SheetNamer::default();
        for sheet in &self.drawing.sheets {
            self.visitor
                .context()
                .check(Kind::Work, 1)
                .map_err(|s| (s, sheet.source.clone()))?;
            let name = namer.next(sheet);
            if sheet.id == id {
                return Ok(Some((sheet, name)));
            }
        }
        Ok(None)
    }

    /// 空間か用紙の要素の ID を、要素の順に振る。一度振ったものは覚えておく
    fn ids<P>(
        &mut self,
        place: &str,
        elements: &'a [Element<P>],
    ) -> Result<Rc<[String]>, (Stop, Ref)> {
        if let Some(ids) = self.names.get(place) {
            return Ok(Rc::clone(ids));
        }
        let mut namer = Namer::new(place);
        let mut ids = Vec::with_capacity(elements.len());
        for (index, element) in elements.iter().enumerate() {
            self.visitor
                .context()
                .check(Kind::Work, 1)
                .map_err(|stop| (stop, element.source.clone()))?;
            ids.push(namer.next(index, element));
        }
        let ids: Rc<[String]> = ids.into();
        self.names.insert(place.to_owned(), Rc::clone(&ids));
        Ok(ids)
    }

    /// 空間の描く順のうち、要素を指す添字。空間ごとに一度求めたものを覚えておく。図面モデルのバイナリは描く順の添字を
    /// 確かめずに読むので、要素の外を指す添字を、配置の升ごと・ビューごとに見直さずに済ませる（#517）。要素の外を指す
    /// 添字は、1 つごとに `work` を 1 と数える
    fn order(&mut self, space: &'a ModelSpace) -> Result<Rc<[usize]>, Stop> {
        let key = std::ptr::from_ref(space);
        if let Some(order) = self.orders.get(&key) {
            return Ok(Rc::clone(order));
        }
        let elements = space.elements.len();
        let mut order = Vec::new();
        for &index in &space.order {
            if index < elements {
                order.push(index);
            } else {
                self.visitor.context().check(Kind::Work, 1)?;
            }
        }
        let order: Rc<[usize]> = order.into();
        self.orders.insert(key, Rc::clone(&order));
        Ok(order)
    }

    /// 空間の ID の文字列。記号の定義の空間なら、定義の ID
    fn space_name(&self, space: &ModelSpace) -> String {
        self.spaces
            .get(&space.id)
            .cloned()
            .unwrap_or_else(|| id::space(space.id))
    }

    fn view(&mut self, view: &'a View) -> Result<(), (Stop, Ref)> {
        let Some(space) = self.model_spaces.get(&view.space).copied() else {
            return Ok(());
        };
        let key = std::ptr::from_ref(view);
        let frame = if let Some(frame) = self.views.get(&key) {
            frame.clone()
        } else {
            self.visitor
                .context()
                .check(
                    Kind::Work,
                    view.clip.len() as u64 + view.hidden_layers.len() as u64,
                )
                .map_err(|s| (s, view.source.clone()))?;
            let mut frame = Frame::top(view.transform);
            if !view.clip.is_empty() {
                frame.clips = Rc::from([Clipping {
                    polygon: view.clip.iter().map(|p| [p.x, p.y]).collect(),
                    inverted: false,
                }]);
            }
            frame.hidden = Rc::new(view.hidden_layers.iter().copied().collect());
            self.views.insert(key, frame.clone());
            frame
        };
        self.space(space, &frame)
    }

    fn space(&mut self, space: &'a ModelSpace, frame: &Frame) -> Result<(), (Stop, Ref)> {
        let name = self.space_name(space);
        let ids = self.ids(&name, &space.elements)?;
        let order = self
            .order(space)
            .map_err(|stop| (stop, Ref::default().with_id(name.as_str())))?;
        for &index in order.iter() {
            if let (Some(element), Some(id)) = (space.elements.get(index), ids.get(index)) {
                self.element(element, id, frame, Item::Model)?;
            } else {
                // Rust から直接渡された不正なモデルでも、ID を引けない添字を無制限に繰り返さない。
                // バイナリの読み取りでは、空間の ID の重複を拒否する（#567）。
                self.visitor
                    .context()
                    .check(Kind::Work, 1)
                    .map_err(|stop| (stop, Ref::default().with_id(name.as_str())))?;
            }
        }
        Ok(())
    }

    /// 要素を返し、記号なら属性を返して、定義の中へ進む
    fn element<P: Point>(
        &mut self,
        element: &'a Element<P>,
        id: &str,
        frame: &Frame,
        item: fn(&'a Element<P>) -> Item<'a>,
    ) -> Result<(), (Stop, Ref)>
    where
        Item<'a>: Attributed<'a, P>,
    {
        let at = || element.source.clone();
        self.visitor
            .context()
            .check(Kind::Work, 1)
            .map_err(|stop| (stop, at()))?;
        self.visitor
            .context()
            .check(Kind::Elements, 1)
            .map_err(|stop| (stop, at()))?;
        let mut visit = self.resolved(item(element), id, element.layer, &element.style, frame);
        // 作図の補助の点は印刷しない（決定 0160）
        if let Shape::Point(point) = &element.shape {
            visit.printable &= !point.construction;
        }
        self.visitor.visit(visit)?;
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
                visit,
            );
        }
        let Shape::Symbol(symbol) = &element.shape else {
            return Ok(());
        };
        for (index, attribute) in symbol.attributes.iter().enumerate() {
            self.visitor
                .context()
                .check(Kind::Work, 1)
                .map_err(|stop| (stop, at()))?;
            self.visitor
                .context()
                .check(Kind::Elements, 1)
                .map_err(|stop| (stop, at()))?;
            self.attribute(element, index, attribute, id, frame)?;
        }
        let Some(&definition) = self.keys.get(&symbol.key) else {
            return Ok(());
        };
        let base = self.drawing.symbols.get(definition).and_then(|d| d.base);
        let cells = symbol
            .cells()
            .map(|cell| (cell, frame.transform.after(&symbol.placement(base, cell))));
        self.inside(
            element,
            id,
            frame,
            definition,
            cells,
            symbol.clip.as_ref(),
            visit,
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

    /// 記号の定義の中へ、配置ごとに進む。`cells` は格子の列と行と、定義の空間から起点への変換。格子の升は
    /// 多くなりうるので、集めずに順に進み、升ごとに `work` を 1 と数える
    #[allow(clippy::too_many_arguments)]
    fn inside<P: Point>(
        &mut self,
        element: &'a Element<P>,
        id: &str,
        frame: &Frame,
        definition: usize,
        cells: impl IntoIterator<Item = ([u32; 2], Affine)>,
        clip: Option<&'a Clip>,
        inherited: VisitRef<'a, '_>,
    ) -> Result<(), (Stop, Ref)> {
        let at = || element.source.clone();
        if frame.definitions.contains(&definition) {
            self.visitor.diagnostic(
                Diagnostic::new(codes::DRAWING_PLACEMENT_INVALID)
                    .with("id", id.to_owned())
                    .at(at()),
            );
            return Ok(());
        }
        let Some(symbols) = self.drawing.symbols.get(definition) else {
            return Ok(());
        };
        // 返す要素のない定義（描く順が要素を指さない）は、どの升でも何も返さない。升の多い格子でも、升をたどらずに
        // 済ませる（#517）。描く順は、定義ごとに一度だけ確かめる
        if self
            .order(&symbols.space)
            .map_err(|stop| (stop, at()))?
            .is_empty()
        {
            return Ok(());
        }
        self.visitor
            .context()
            .enter()
            .map_err(|stop| (stop, at()))?;
        let result = (|| {
            let layer = inherited.layer;
            let color = inherited.color;
            let line_type = inherited.line_type;
            let line_weight = inherited.line_weight;
            let visible = frame.visible
                && !frame.hidden.contains(&layer)
                && self
                    .layers
                    .get(&layer)
                    .and_then(|l| l.state)
                    .is_none_or(|s| !s.frozen);
            for (cell, placement) in cells {
                self.visitor
                    .context()
                    .check(Kind::Work, 1 + frame.path.len() as u64)
                    .map_err(|s| (s, at()))?;
                let clips = if let Some(clip) = clip {
                    let points = if clip.vertices.len() == 2 {
                        4
                    } else {
                        clip.vertices.len()
                    };
                    let copied: usize = frame.clips.iter().map(|c| c.polygon.len()).sum();
                    self.visitor
                        .context()
                        .check(Kind::Work, (points + copied) as u64)
                        .map_err(|s| (s, at()))?;
                    let corners = match clip.vertices.as_slice() {
                        &[[x0, y0], [x1, y1]] => vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1]],
                        vertices => vertices.to_vec(),
                    };
                    let mut clips = frame.clips.to_vec();
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
                    Rc::from(clips)
                } else {
                    Rc::clone(&frame.clips)
                };
                let mut path = frame.path.to_vec();
                path.push(Step {
                    id: id.to_owned(),
                    cell,
                });
                let mut definitions = frame.definitions.to_vec();
                definitions.push(definition);
                let inner = Frame {
                    transform: placement,
                    clips,
                    path: path.into(),
                    definitions: definitions.into(),
                    hidden: Rc::clone(&frame.hidden),
                    layer: Some(layer),
                    color,
                    line_type,
                    line_weight,
                    visible,
                };
                self.space(&symbols.space, &inner)?;
            }
            Ok(())
        })();
        self.visitor.context().leave();
        result
    }

    fn attribute<P: Point>(
        &mut self,
        element: &'a Element<P>,
        index: usize,
        attribute: &'a Attribute<P>,
        id: &str,
        frame: &Frame,
    ) -> Result<(), (Stop, Ref)>
    where
        Item<'a>: Attributed<'a, P>,
    {
        let parent = self.resolved(
            Item::attribute(element, index),
            id,
            element.layer,
            &element.style,
            frame,
        );
        let layer = parent.layer;
        let inner = Frame {
            layer: Some(layer),
            color: parent.color,
            line_type: parent.line_type,
            line_weight: parent.line_weight,
            visible: frame.visible
                && !frame.hidden.contains(&layer)
                && self
                    .layers
                    .get(&layer)
                    .and_then(|l| l.state)
                    .is_none_or(|s| !s.frozen),
            ..frame.clone()
        };
        let mut visit = self.resolved(
            Item::attribute(element, index),
            id,
            attribute.layer,
            &attribute.style,
            &inner,
        );
        visit.visible &= !attribute.hidden;
        visit.printable &= !attribute.hidden;
        self.visitor.visit(visit)
    }

    fn resolved<'v>(
        &self,
        item: Item<'a>,
        id: &'v str,
        layer: LayerId,
        style: &Style,
        frame: &'v Frame,
    ) -> VisitRef<'a, 'v> {
        let definition = self.layers.get(&layer).copied();
        let effective = if definition.is_some_and(|l| l.follows_placement) {
            frame.layer.unwrap_or(layer)
        } else {
            layer
        };
        let definition = self.layers.get(&effective).copied();
        let layer_style = definition.and_then(|l| l.style.as_ref());
        let color = match style.color {
            Inherit::Own(v) => Some(v),
            Inherit::Layer => layer_style.and_then(|s| s.color),
            Inherit::Block => {
                if frame.layer.is_some() {
                    frame.color
                } else {
                    self.drawing.default_color
                }
            }
        };
        let line_type = match style.line_type {
            Inherit::Own(v) => Some(v),
            Inherit::Layer => layer_style.and_then(|s| s.line_type),
            Inherit::Block => {
                if frame.layer.is_some() {
                    frame.line_type
                } else {
                    self.drawing.default_line_type
                }
            }
        };
        let width = |weight| match weight {
            LineWeight::Width(width) => Some(width),
            LineWeight::Default => self.drawing.default_line_weight,
        };
        let line_weight = match style.line_weight {
            Some(Inherit::Own(v)) => width(v),
            Some(Inherit::Layer) => layer_style.and_then(|s| s.line_weight).and_then(width),
            Some(Inherit::Block) => {
                if frame.layer.is_some() {
                    frame.line_weight
                } else {
                    self.drawing.default_line_weight
                }
            }
            None => None,
        };
        let state = definition.and_then(|l| l.state);
        let visible = frame.visible
            && !frame.hidden.contains(&effective)
            && state.is_none_or(|s| s.visible && !s.frozen);
        VisitRef {
            item,
            id,
            path: &frame.path,
            transform: frame.transform,
            color,
            line_type,
            line_weight,
            layer: effective,
            layer_definition: definition,
            visible,
            printable: visible && state.is_none_or(|s| s.printable),
            clips: &frame.clips,
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

    /// 結果は保持せず、ビューの切り抜きが同じ領域を借りることも確かめる。
    struct Counter<'c, 'x> {
        cx: &'c mut Cx<'x>,
        count: usize,
        clip: Option<*const [f64; 2]>,
    }
    impl<'a> WalkVisitor<'a> for Counter<'_, '_> {
        fn context(&mut self) -> Cx<'_> {
            Cx::new(self.cx.budget, self.cx.diag, self.cx.cancel)
        }
        fn visit(&mut self, visit: VisitRef<'a, '_>) -> Result<(), (Stop, Ref)> {
            self.count += 1;
            if let Some(clip) = visit.clips.first() {
                let pointer = clip.polygon.as_ptr();
                if let Some(previous) = self.clip {
                    assert_eq!(previous, pointer);
                }
                self.clip = Some(pointer);
            }
            Ok(())
        }
    }

    fn viewed(
        elements: Vec<Element<ModelPoint>>,
        clip_points: usize,
        order: Vec<SheetItem>,
    ) -> Drawing2D {
        Drawing2D {
            spaces: vec![space(MODEL, elements)],
            sheets: vec![Sheet {
                id: SHEET,
                name: None,
                size: None,
                elements: Vec::new(),
                source: Ref::default(),
                order,
                views: vec![View {
                    space: MODEL,
                    sheet: SHEET,
                    transform: Affine::IDENTITY,
                    clip: (0..clip_points)
                        .map(|x| PaperPoint {
                            sheet: SHEET,
                            x: x as f64,
                            y: 0.0,
                        })
                        .collect(),
                    hidden_layers: Vec::new(),
                    source: Ref::default(),
                }],
            }],
            ..Drawing2D::default()
        }
    }

    #[test]
    fn a_repeated_empty_view_prepares_its_polygon_once_and_counts_sheet_order() {
        let drawing = viewed(Vec::new(), 10_000, vec![SheetItem::View(0); 10_000]);
        let mut budget = Budget::new(Limits::default().with(Kind::Work, Limit::Finite(20_002)));
        let mut diag = Diagnostics::new();
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        let mut counter = Counter {
            cx: &mut cx,
            count: 0,
            clip: None,
        };
        drawing
            .walk_to(Origin::Sheet(SHEET), false, &mut counter)
            .unwrap();
        assert_eq!(counter.count, 0);
        assert_eq!(budget.used(Kind::Work), 20_002);
        let mut budget = Budget::new(Limits::default().with(Kind::Work, Limit::Finite(10_000)));
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        assert!(drawing.walk(Origin::Sheet(SHEET), &mut cx).is_err());
        // 範囲の外の要素とビューも、順の項目ごとに数える。
        let drawing = viewed(
            Vec::new(),
            0,
            vec![SheetItem::Element(99), SheetItem::View(99)],
        );
        let mut budget = Budget::new(Limits::default().with(Kind::Work, Limit::Finite(3)));
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        assert!(
            matches!(drawing.walk(Origin::Sheet(SHEET), &mut cx), Err((Stop::Exceeded(e), _)) if e.kind == Kind::Work)
        );
    }

    #[test]
    fn streamed_visits_share_view_clips_and_do_not_charge_for_collected_results() {
        let drawing = viewed(
            vec![line(MODEL, 0, block()); 1000],
            1000,
            vec![SheetItem::View(0); 10],
        );
        let mut budget = Budget::new(Limits::default().with(Kind::DecodedBytes, Limit::Finite(1)));
        let mut diag = Diagnostics::new();
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        let mut counter = Counter {
            cx: &mut cx,
            count: 0,
            clip: None,
        };
        drawing
            .walk_to(Origin::Sheet(SHEET), false, &mut counter)
            .unwrap();
        assert_eq!(counter.count, 10_000);
        assert!(counter.clip.is_some());
        // 所有する API は、切り抜きを含む複製量の予算が足りなければ止まる。
        assert!(
            matches!(drawing.walk(Origin::Sheet(SHEET), &mut cx), Err((Stop::Exceeded(e), _)) if e.kind == Kind::DecodedBytes)
        );
    }

    #[test]
    fn indexed_layers_keep_the_first_duplicate_and_resolve_a_deep_chain() {
        let mut drawing = nested();
        drawing
            .layers
            .extend((4..10_000).map(|id| layer(id, Some(2), ON)));
        drawing.layers.push(layer(1, Some(9), ON));
        let depth = 100;
        drawing.spaces[0].elements = vec![insert(MODEL, "chain-0", 1, by_layer(), 0.0)];
        drawing.symbols = (0..depth)
            .map(|n| {
                let id = SpaceId(n + 1);
                let element = if n + 1 == depth {
                    line(id, 0, block())
                } else {
                    insert(id, &format!("chain-{}", n + 1), 0, block(), 0.0)
                };
                definition(&format!("chain-{n}"), space(id, vec![element]))
            })
            .collect();
        let (visits, _) = walk(&drawing, Origin::Space(MODEL));
        let last = visits.last().unwrap();
        assert_eq!(last.path.len(), depth as usize);
        assert_eq!(last.layer, LayerId(1));
        assert_eq!(last.color, Some(aci(1)));
        assert_eq!(last.line_type, Some(LineTypeId(1)));
        assert_eq!(last.line_weight, Some(PaperLength(0.2)));
        assert!(last.visible && last.printable);
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

    #[test]
    fn returned_elements_are_counted_as_elements() {
        let drawing = nested();
        let cancel = CancelToken::new();
        let mut budget = Budget::default();
        let mut diag = Diagnostics::new();
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        let visits = drawing.walk(Origin::Space(MODEL), &mut cx).unwrap().len();
        assert_eq!(budget.used(Kind::Elements), visits as u64);
        let limit = visits as u64 - 1;
        let mut budget = Budget::new(Limits::default().with(Kind::Elements, Limit::Finite(limit)));
        let mut diag = Diagnostics::new();
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        let (stop, _) = drawing.walk(Origin::Space(MODEL), &mut cx).unwrap_err();
        assert!(
            matches!(stop, Stop::Exceeded(exceeded) if exceeded.kind == Kind::Elements),
            "{stop:?}"
        );
    }

    #[test]
    fn grids_of_cycles_stop_at_the_elements_budget() {
        // #517 の fuzz で見つけた形を縮めたもの。格子の配置の升ごとに、中の配置が循環する定義へ進み、要素を返して
        // 診断を足す。返す要素を数えないと、既定の `work` の上限（1e10）まで要素を集め、メモリを使い果たす
        let mut drawing = nested();
        drawing.symbols[0]
            .space
            .elements
            .push(insert(INNER, "OUTER", 0, block(), 0.0));
        drawing.symbols[0].space.order.push(2);
        let Shape::Symbol(symbol) = &mut drawing.spaces[0].elements[0].shape else {
            panic!("記号の要素")
        };
        symbol.grid = Some(Grid {
            columns: 60_000,
            rows: 60_000,
            column_spacing: Length::Model(ModelLength {
                space: MODEL,
                value: 1.0,
            }),
            row_spacing: Length::Model(ModelLength {
                space: MODEL,
                value: 1.0,
            }),
        });
        let cancel = CancelToken::new();
        // `work` も有限にして、要素を数えなくなったときに、終わらずにメモリを使い果たすのではなく、`work` で止まって
        // 落ちるようにする
        let limits = Limits::default()
            .with(Kind::Elements, Limit::Finite(1000))
            .with(Kind::Work, Limit::Finite(1 << 20));
        let mut budget = Budget::new(limits);
        let mut diag = Diagnostics::new();
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        let (stop, _) = drawing.walk(Origin::Space(MODEL), &mut cx).unwrap_err();
        assert!(
            matches!(stop, Stop::Exceeded(exceeded) if exceeded.kind == Kind::Elements),
            "{stop:?}"
        );
        // 3.6e9 の升を歩かずに止まる
        assert!(
            budget.used(Kind::Work) < 10_000,
            "{}",
            budget.used(Kind::Work)
        );
    }

    #[test]
    fn grids_of_empty_definitions_are_not_walked() {
        // #517 の fuzz で見つけた形を縮めたもの。中身の配置を落とした定義を、升の多い格子で置く。升ごとにたどると、
        // 何も返さないまま `work` の上限まで回る
        // 要素のない定義と、描く順が要素の外だけを指す定義（図面モデルのバイナリは、描く順の添字を確かめずに読む）
        let mut outside = space(OUTER, Vec::new());
        outside.order = vec![0, 7];
        for definition in [space(OUTER, Vec::new()), outside] {
            let mut drawing = nested();
            drawing.symbols[1].space = definition;
            let Shape::Symbol(symbol) = &mut drawing.spaces[0].elements[0].shape else {
                panic!("記号の要素")
            };
            symbol.grid = Some(Grid {
                columns: u32::MAX,
                rows: u32::MAX,
                column_spacing: Length::Model(ModelLength {
                    space: MODEL,
                    value: 1.0,
                }),
                row_spacing: Length::Model(ModelLength {
                    space: MODEL,
                    value: 1.0,
                }),
            });
            let cancel = CancelToken::new();
            let mut budget = Budget::new(Limits::default().with(Kind::Work, Limit::Finite(1000)));
            let mut diag = Diagnostics::new();
            let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
            let visits = drawing.walk(Origin::Space(MODEL), &mut cx).unwrap();
            // 配置だけを返し、升をたどらない
            assert_eq!(visits.len(), 1);
            assert!(budget.used(Kind::Work) < 100, "{}", budget.used(Kind::Work));
        }
    }

    #[test]
    fn spaces_with_the_same_id_keep_their_own_orders() {
        // #565 の確かめで見つけた形。図面モデルのバイナリは、空間の ID の重なりを確かめずに読む。同じ ID の 2 つの
        // 定義は、それぞれの描く順で返す。描く順を ID や名前で覚えると、先の定義の描く順で後の定義を返す
        let mut drawing = nested();
        let lines_on = || vec![line(OUTER, 2, by_layer()), line(OUTER, 3, by_layer())];
        let mut first = space(OUTER, lines_on());
        first.order = vec![1, 0];
        let second = space(OUTER, lines_on());
        drawing.symbols = vec![definition("FIRST", first), definition("OUTER", second)];
        drawing.spaces[0]
            .elements
            .push(insert(MODEL, "FIRST", 1, by_layer(), 0.0));
        drawing.spaces[0].order = vec![1, 0];
        let (visits, _) = walk(&drawing, Origin::Space(MODEL));
        // 先に描く配置（FIRST）は描く順 [1, 0]、後の配置（OUTER）は [0, 1] で、線のレイヤが 3・2 と 2・3 の順になる
        let layers: Vec<u32> = lines(&visits).iter().map(|visit| visit.layer.0).collect();
        assert_eq!(layers, vec![3, 2, 2, 3]);
    }

    #[test]
    fn indices_without_ids_are_counted() {
        // #565 の確かめで見つけた形。空間の ID が重なった図面モデルのバイナリでは、定義の空間が、先にたどった空間の
        // 要素の ID を使う（#567）。ID を引けない添字を数えずに飛ばすと、升ごとに、添字の数だけ数えずに回る
        const SIDE: u32 = 64;
        let mut drawing = nested();
        // 外の定義の空間を、実寸の空間と同じ ID にし、線を 64 本置く。要素の ID は、実寸の空間の 1 つだけになる
        drawing.symbols[1].space =
            space(MODEL, (0..64).map(|_| line(MODEL, 2, by_layer())).collect());
        let Shape::Symbol(symbol) = &mut drawing.spaces[0].elements[0].shape else {
            panic!("記号の要素")
        };
        symbol.grid = Some(Grid {
            columns: SIDE,
            rows: SIDE,
            column_spacing: Length::Model(ModelLength {
                space: MODEL,
                value: 1.0,
            }),
            row_spacing: Length::Model(ModelLength {
                space: MODEL,
                value: 1.0,
            }),
        });
        let cancel = CancelToken::new();
        // 升ごとに、ID を引けない 63 の添字を数えると、この上限を超えて止まる。数えなければ、上限の中で終わる
        let mut budget = Budget::new(Limits::default().with(Kind::Work, Limit::Finite(1 << 16)));
        let mut diag = Diagnostics::new();
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        let (stop, _) = drawing.walk(Origin::Space(MODEL), &mut cx).unwrap_err();
        assert!(
            matches!(stop, Stop::Exceeded(exceeded) if exceeded.kind == Kind::Work),
            "{stop:?}"
        );
    }

    #[test]
    fn orders_outside_the_elements_are_counted_once_per_definition() {
        // #565 の確かめで見つけた形。描く順が、要素の外を指す添字を数多く持ち、要素を 1 つだけ指す定義を、格子で
        // 置く。升ごとに描く順を見直すと、`work` を数えずに、升の数 × 添字の数だけ回る
        const OUTSIDE: usize = 1 << 16;
        const SIDE: u32 = 64;
        let mut drawing = nested();
        let mut definition = space(OUTER, vec![line(OUTER, 0, by_layer())]);
        definition
            .order
            .extend((1..=OUTSIDE).map(|offset| offset + 7));
        drawing.symbols[1].space = definition;
        let Shape::Symbol(symbol) = &mut drawing.spaces[0].elements[0].shape else {
            panic!("記号の要素")
        };
        symbol.grid = Some(Grid {
            columns: SIDE,
            rows: SIDE,
            column_spacing: Length::Model(ModelLength {
                space: MODEL,
                value: 1.0,
            }),
            row_spacing: Length::Model(ModelLength {
                space: MODEL,
                value: 1.0,
            }),
        });
        let cancel = CancelToken::new();
        // 升ごとに外を指す添字を数えると、この上限を超えて止まる
        let mut budget = Budget::new(Limits::default().with(Kind::Work, Limit::Finite(1 << 18)));
        let mut diag = Diagnostics::new();
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        let visits = drawing.walk(Origin::Space(MODEL), &mut cx).unwrap();
        // 配置と、升ごとの線
        let cells = (SIDE * SIDE) as usize;
        assert_eq!(visits.len(), 1 + cells);
        // 外を指す添字は、定義ごとに一度だけ数える
        let used = budget.used(Kind::Work);
        assert!(
            (OUTSIDE as u64..OUTSIDE as u64 + 4 * cells as u64).contains(&used),
            "{used}"
        );
    }

    #[test]
    fn huge_grids_stop_at_the_work_budget_without_collecting_the_cells() {
        let mut drawing = nested();
        let Shape::Symbol(symbol) = &mut drawing.spaces[0].elements[0].shape else {
            panic!("記号の要素")
        };
        // 升を集めてから歩くと、約 1.8e19 の升で記憶を食い尽くす
        symbol.grid = Some(Grid {
            columns: u32::MAX,
            rows: u32::MAX,
            column_spacing: Length::Model(ModelLength {
                space: MODEL,
                value: 1.0,
            }),
            row_spacing: Length::Model(ModelLength {
                space: MODEL,
                value: 1.0,
            }),
        });
        let cancel = CancelToken::new();
        let mut budget = Budget::new(Limits::default().with(Kind::Work, Limit::Finite(1000)));
        let mut diag = Diagnostics::new();
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        let (stop, _) = drawing.walk(Origin::Space(MODEL), &mut cx).unwrap_err();
        assert!(matches!(stop, Stop::Exceeded(_)));
    }
}
