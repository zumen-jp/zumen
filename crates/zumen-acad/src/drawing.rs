//! 共有のオブジェクトの表から図面モデル（L2）への変換（決定 0022・0023・0105〜0116）。いまは ENTITIES の
//! LINE・CIRCLE・ARC・ELLIPSE・LWPOLYLINE・POLYLINE（と、その VERTEX）・SPLINE・TEXT・MTEXT・DIMENSION・LEADER・MULTILEADER・INSERT（と、その ATTRIB）・HATCH・ACAD_TABLE・IMAGE・PDFUNDERLAY など・VIEWPORT と、画層・線種・寸法スタイルの表と、
//! BLOCKS の BLOCK（と、その中身と ATTDEF）を記号の定義にし、図形の色・線種・線幅と、空間の描く順を移す。
//! 図形の XDATA は、読んだときの図形の値とともに形式固有の情報にする。
//! DXF も DWG も、表を作ってから同じ変換を通す。
//!
//! モデル空間の図形は実寸の空間に、ペーパー空間（67 = 1）の図形とビューポートはレイアウトごとの用紙に置く。
//! 実寸の単位は見出しの `$INSUNITS` で決め、分からなければ不明とする。用紙の点は `$PLIMMIN` を原点にした mm で、
//! 単位が分からなければ mm に直せないので、ペーパー空間の図形を移さない。
//! 画層は表の順に置き、表にない名前を図形が指せば、定義のない画層として足す。描く順は、モデル空間・
//! ペーパー空間・記号の定義のブロックのレコードを指す SORTENTSTABLE で決め、なければ入力の順とする。
//! 移さない図形と値は、元のレコードを指す診断で返す。診断の code は、表がグループ・コードで値を名付けるので、
//! DWG でも `dxf.` で始まるものを使う。DWG の読み手がまだ読まない値（R2010 からの表の書式）だけは、`dwg.` で
//! 始まるものを使う。何をどう読んだかの推定は `docs/formats/dxf.md` に記録した。

mod acad_table;
mod annotation;
mod apply;
mod block;
mod extension;
mod geometry;
mod hatch;
mod image;
mod layout;
mod lettering;
mod mtext;
mod style;

use std::collections::{BTreeMap, BTreeSet};

use crate::db::{AcadDb, Data, Object, Origin, Section, Value};
use crate::{DwgFile, DxfFile};
use acad_table::report_cell_values;
use extension::extension_work;
use zumen_base::{Cx, Diagnostic, Key, Kind, Ref, codes};
use zumen_drawing::{
    Arrowhead, Basis, CalloutContent, CellContent, Color, ColorCode, ColorSystem, DimensionStyle,
    Drawing2D, Edge, Element, Extension, Layer, LayerId, Length, LineType, LineTypeId, ModelLength,
    ModelPoint, ModelSpace, PaperLength, PaperPoint, Shape, Sheet, SheetId, SheetItem, SpaceId,
    Spline, SymbolDefinition, SymbolKey, Target, TextStyle, Unit, View,
};
use zumen_sdk::{Error, Lift};

/// UTF-8 で書く最初の版（2007）
const UTF8: &[u8] = b"AC1021";

/// HATCH の SPLINE の辺が当てはめる点を持つ最初の版（2010）
const FIT_DATA: &[u8] = b"AC1024";

/// DWG の ACAD_TABLE が内容を TABLECONTENT の形で持つ最初の版（2010。ODS 5.4.1 の 20.4.96.2）
const TABLE_CONTENT: &[u8] = b"AC1024";

/// 移さない理由。診断は大きいので箱に入れる
type Lifted<T> = Result<T, Box<Diagnostic>>;

const MODEL: SpaceId = SpaceId(0);
const SHEET: SheetId = SheetId(0);
/// まだ ID を付けていない、中身のない用紙
const EMPTY: SheetId = SheetId(u32::MAX);

/// ブロックの定義を指す記号のキーの名前空間
const BLOCK: &str = "dxf.block";

/// XDATA の形式固有の情報の名前空間と、値の型の契約の版（決定 0115・0157）
const XDATA: &str = "dxf.xdata";

const POINT: &str = "dxf.point";
const XDATA_SCHEMA: u32 = 2;
/// 図面モデルの値にない図形の欄の拡張の名前空間と、値の型の契約の版（決定 0142・0157）
const ENTITY: &str = "dxf.entity";
const ENTITY_SCHEMA: u32 = 2;
/// 図形の拡張辞書の XRECORD の形式固有の情報の名前空間と、値の型の契約の版（決定 0148・0157）
const XRECORD: &str = "dxf.xrecord";
const XRECORD_SCHEMA: u32 = 2;

impl Lift<Drawing2D> for DxfFile {
    /// 表を作ってから変換する。型どおりに読めない値は、変換がその値を使うときに診断を返す
    fn lift(&self, cx: &mut Cx) -> Result<Drawing2D, Error> {
        lift(&self.table(cx, false)?, cx)
    }
}

impl Lift<Drawing2D> for DwgFile {
    /// 表を作ってから変換する。表を作るときの診断も返す
    fn lift(&self, cx: &mut Cx) -> Result<Drawing2D, Error> {
        lift(&self.objects(cx)?, cx)
    }
}

fn lift(db: &AcadDb, cx: &mut Cx) -> Result<Drawing2D, Error> {
    framed(db, cx).map(|(drawing, _)| drawing)
}

/// MTEXT の内容を、図面モデルの書式の区間に分ける。読むときと書き戻すときに区間を確かめるのと同じ解釈で、
/// `length` は図面の単位の値を長さにする。境界から書式つきの文字を足し、置き換えるときに使う
pub fn mtext_spans(content: &str, length: &impl Fn(f64) -> Length) -> Vec<zumen_drawing::Span> {
    mtext::spans(content, length)
}

/// 寸法値の文字の内容を、図面モデルの書式の区間に分ける。測定値に置き換える符号（`<>`）は測定値の区間にする。
/// 読むときと同じ解釈で、境界から寸法の形の値を受けるときに使う
pub fn dimension_spans(content: &str, length: &impl Fn(f64) -> Length) -> Vec<zumen_drawing::Span> {
    mtext::dimension_spans(content, length)
}

/// 用紙の座標と、元の図形の座標の対応。用紙の点は、元の点から原点（図面の単位）を引き、1 単位の mm の数を掛ける
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct PaperFrame {
    pub(crate) origin: [f64; 2],
    pub(crate) mm: f64,
    /// 用紙の図形の置き場所。用紙に図形を足すときに、同じ所に置く
    pub(crate) home: Home,
}

/// 図形の置き場所。所有者のブロックのレコードと、BLOCKS の所（ほかのレイアウトのペーパー空間のブロック）に
/// あるかと、ペーパー空間の印（67 = 1）を持つか
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Home {
    pub(crate) owner: Option<u64>,
    pub(crate) blocks: bool,
    pub(crate) paper: bool,
}

/// 図面モデルと、用紙ごとの座標の対応。中身のない用紙は、LAYOUT のブロックのレコードがあり、単位が
/// 分かるものだけが持つ
fn framed(db: &AcadDb, cx: &mut Cx) -> Result<(Drawing2D, BTreeMap<SheetId, PaperFrame>), Error> {
    let unit = unit(db, cx);
    let limits = |name: &[u8]| {
        db.variable(name)
            .and_then(|values| Some([header_real(values, 10)?, header_real(values, 20)?]))
    };
    let version =
        db.variable(b"$ACADVER")
            .and_then(|values| match values.first().map(|v| &v.data) {
                Some(Data::Text(text)) => Some(text.trim_ascii()),
                _ => None,
            });
    let layouts = layout_index(db, cx)?;
    let lifter = Lifter {
        layouts: &layouts,
        sheet: SHEET,
        unicode: version.is_some_and(|v| v >= UTF8),
        fit_data: version.is_some_and(|v| v >= FIT_DATA),
        table_content: version.is_some_and(|v| v >= TABLE_CONTENT),
        version: version.map(|v| String::from_utf8_lossy(v).into_owned()),
        mm: unit.millimeters(),
        unit,
        origin: limits(b"$PLIMMIN").unwrap_or_default(),
        point_mode: db
            .variable(b"$PDMODE")
            .and_then(|values| match values.first().map(|v| &v.data) {
                Some(Data::Int(mode)) => Some(*mode),
                _ => None,
            })
            .unwrap_or(0),
    };
    let mut out = Out::default();
    // 画層が線種を指すので、線種の表を先に移す
    for kind in [&b"LTYPE"[..], b"LAYER", b"DIMSTYLE", b"STYLE"] {
        for object in db.objects() {
            if object.section != Section::Tables || object.kind != kind {
                continue;
            }
            let source = source(object);
            work(cx, 1, &source)?;
            let lifted = match kind {
                b"LTYPE" => lifter.line_type_record(db, object, source.clone(), &mut out, cx),
                b"LAYER" => lifter.layer_record(object, source.clone(), &mut out),
                b"DIMSTYLE" => lifter.dimension_style_record(db, object, source.clone(), &mut out),
                _ => lifter.text_style_record(object, source.clone(), &mut out),
            };
            if let Err(diagnostic) = lifted {
                cx.report((*diagnostic).at(source));
                continue;
            }
            // 画層と線種の拡張辞書の XRECORD は、移したレコードに付ける（決定 0152）
            let (target, basis) = match kind {
                b"LTYPE" => match out.line_types.last() {
                    Some(line_type) => (
                        Target::LineType(line_type.id),
                        Basis::LineType(line_type.clone()),
                    ),
                    None => continue,
                },
                b"LAYER" => match out.layers.last() {
                    Some(layer) => (Target::Layer(layer.id), Basis::Layer(layer.clone())),
                    None => continue,
                },
                _ => continue,
            };
            extension_work(db, object, cx)?;
            lifter.dictionary(db, object, target, &|| basis.clone(), &mut out, cx);
        }
    }
    let entities: Vec<&Object> = db
        .objects()
        .iter()
        .filter(|object| object.section == Section::Entities)
        .collect();
    let (attributes, owned) = attached(&entities);
    for (index, object) in entities.iter().enumerate() {
        let source = source(object);
        work(cx, amount(object), &source)?;
        if owned.contains(&index) {
            continue;
        }
        extension_work(db, object, cx)?;
        let attributes: Vec<&Object> = attributes
            .get(&index)
            .map_or(&[][..], Vec::as_slice)
            .iter()
            .filter_map(|at| entities.get(*at).copied())
            .collect();
        if let Err(diagnostic) =
            lifter.entity(db, object, source.clone(), &attributes, &mut out, cx)
        {
            cx.report((*diagnostic).at(source));
        }
    }
    let ([model_keys, paper_keys], block_keys) = lifter.sort_keys(db, cx)?;
    let symbols = lifter.definitions(db, &block_keys, &mut out, cx)?;
    check_symbols(&symbols, &out, cx)?;
    // 見出しの $LTSCALE と、1/100 mm の $LWDEFAULT（DXF Reference「HEADER Section Group Codes」）
    let line_type_scale = db
        .variable(b"$LTSCALE")
        .and_then(|values| header_real(values, 40))
        .filter(|scale| scale.is_finite() && *scale > 0.0);
    let default_line_weight = db.variable(b"$LWDEFAULT").and_then(|values| {
        match values.iter().find(|v| v.code == 370)?.data {
            Data::Int(width @ 0..=211) => Some(PaperLength(width as f64 / 100.0)),
            _ => None,
        }
    });
    // 起点まで BYBLOCK のまま残る色と線種は、色 7 と実線とする（推定。決定 0159）
    let default_color = Some(Color {
        code: Some(ColorCode {
            system: ColorSystem::Aci,
            value: 7,
        }),
        rgb: None,
    });
    let default_line_type = out
        .line_types
        .iter()
        .find(|line_type| line_type.name.eq_ignore_ascii_case("Continuous"))
        .map(|line_type| line_type.id);
    let mut drawing = Drawing2D {
        layers: out.layers,
        line_types: out.line_types,
        line_type_scale,
        default_line_weight,
        default_color,
        default_line_type,
        dimension_styles: out.dimension_styles,
        text_styles: out.text_styles,
        symbols,
        extensions: out.extensions,
        ..Drawing2D::default()
    };
    drawing.spaces.push(ModelSpace {
        id: MODEL,
        unit,
        elements: out.model,
        order: arrange(
            model_keys.as_ref(),
            out.model_handles.into_iter().enumerate().collect(),
            cx,
        ),
    });
    // 今のレイアウトの LAYOUT は、ペーパー空間の図形の所有者（今のレイアウトのブロックのレコード）を指す
    // LAYOUT とする。DWG のほかのレイアウトのブロックのレコードには、共有の表を作るときに名前の番号を付ける
    let current = entities
        .iter()
        .find(|object| matches!(object.first(67), Some(Data::Int(1))))
        .and_then(|object| object.owner())
        .and_then(|handle| lifter.layout_of(db, handle));
    let has_paper = !out.paper.is_empty() || !out.views.is_empty();
    if has_paper {
        let size = match (limits(b"$PLIMMAX"), lifter.mm) {
            (Some([x, y]), Some(mm)) => {
                let [ox, oy] = lifter.origin;
                let size = [(x - ox) * mm, (y - oy) * mm];
                size.iter().all(|s| *s > 0.0).then(|| size.map(PaperLength))
            }
            _ => None,
        };
        let order = arrange(paper_keys.as_ref(), out.sheet, cx);
        let name = current.and_then(|(_, layout)| lifter.layout_info(layout).name);
        drawing.sheets.push(Sheet {
            id: SHEET,
            name,
            size,
            elements: out.paper,
            views: out.views,
            order,
            source: current.map_or_else(Ref::default, |(_, layout)| source(layout)),
        });
    }
    // ほかのレイアウトの用紙は、今のレイアウトの後に、タブの順で並べる。今のレイアウトのペーパー空間の
    // ブロックは中身を持たないので、今のレイアウトの用紙を作ったなら、そのブロックからの空の用紙は除く。
    // 中身のない用紙の ID は、中身のある用紙の後から、タブの順に付ける
    let current = current.map(|(index, _)| index);
    out.layouts
        .retain(|(_, layout, _, _)| !(has_paper && layout.is_some() && *layout == current));
    out.layouts.sort_by_key(|(tab, _, _, _)| *tab);
    let filled = out
        .layouts
        .iter()
        .filter(|(_, _, sheet, _)| sheet.id != EMPTY)
        .count();
    let mut next = u32::try_from(filled).unwrap_or(u32::MAX).saturating_add(1);
    for (_, _, sheet, frame) in &mut out.layouts {
        if sheet.id == EMPTY {
            sheet.id = SheetId(next);
            next = next.saturating_add(1);
            if let Some(frame) = frame {
                out.frames.insert(sheet.id, *frame);
            }
        }
    }
    drawing
        .sheets
        .extend(out.layouts.into_iter().map(|(_, _, sheet, _)| sheet));
    drawing.add_named_text_styles(cx).map_err(|(stop, place)| {
        cx.report(stop.to_diagnostic().at(place));
        Error::Stopped(stop)
    })?;
    Ok((drawing, out.frames))
}

/// 図形 1 つの作業量。書式つきの文字は、内容の 250 バイトの区切り（3）ごとにも数える。曲線は、頂点・制御点・
/// 当てはめる点・節点（10・11・40）ごとにも数える。線種は、列の要素（49）ごとにも数える
fn amount(object: &Object) -> u64 {
    let count = |codes: &[i16]| {
        object
            .values
            .iter()
            .filter(|value| codes.contains(&value.code))
            .count() as u64
    };
    1 + match object.kind.as_slice() {
        b"MTEXT" | b"ATTRIB" | b"ATTDEF" => count(&[3]),
        b"LWPOLYLINE" | b"SPLINE" => count(&[10, 11, 40]),
        b"ACAD_TABLE" => count(&[141, 142, 171]),
        b"LTYPE" => count(&[49]),
        _ => 0,
    } + object
        .values
        .iter()
        .filter(|value| value.code >= 1000)
        .count() as u64
}

/// 用紙空間のブロックのレコードから LAYOUT の位置を引く索引。レイアウトごとに表全体を探し直さない。
/// LAYOUT の欄の 330 で指す。初めの 330 は所有者なので見ない（DXF Reference「LAYOUT」）
fn layout_index(db: &AcadDb, cx: &mut Cx) -> Result<BTreeMap<u64, usize>, Error> {
    let mut layouts = BTreeMap::new();
    for (index, object) in db.objects().iter().enumerate() {
        if object.kind != b"LAYOUT" {
            continue;
        }
        work(cx, 1, &source(object))?;
        for value in object
            .values
            .iter()
            .filter(|value| value.code == 330)
            .skip(1)
        {
            if let Data::Handle(record) = value.data {
                layouts.entry(record).or_insert(index);
            }
        }
    }
    Ok(layouts)
}

/// LAYOUT の名前（2 つ目の 1。初めの 1 はページ設定の名前）・タブの順（71）・範囲（10・20 と 11・21）
struct LayoutInfo {
    name: Option<String>,
    tab: i64,
    limits: Option<[[f64; 2]; 2]>,
}

/// 決まった作業量を数え、上限を超えたら止める（決定 0010）
/// 配置（INSERT）と、表・セル・寸法・引出線つきの注記・寸法スタイルが指す定義がないものと、自分を直接か
/// 間接に配置する定義を診断する（決定 0116）。配置は展開しないので、どちらも配置と定義は移したまま持つ。
/// 輪は配置だけでたどる
fn check_symbols(symbols: &[SymbolDefinition], out: &Out, cx: &mut Cx) -> Result<(), Error> {
    fn placed<P>(elements: &[Element<P>]) -> impl Iterator<Item = (&SymbolKey, &Ref)> {
        elements.iter().filter_map(|element| match &element.shape {
            Shape::Symbol(symbol) => Some((&symbol.key, &element.source)),
            _ => None,
        })
    }
    /// 配置のほかに図形が指す定義のキー
    fn referenced<P>(elements: &[Element<P>]) -> impl Iterator<Item = (SymbolKey, &Ref)> {
        elements.iter().flat_map(|element| {
            let keys: Vec<SymbolKey> = match &element.shape {
                Shape::Table(table) => table
                    .drawn
                    .iter()
                    .cloned()
                    .chain(table.cells.iter().filter_map(|cell| match &cell.content {
                        CellContent::Symbol(key) => Some(key.clone()),
                        _ => None,
                    }))
                    .collect(),
                Shape::Dimension(dimension) => dimension
                    .block
                    .iter()
                    .map(|name| SymbolKey {
                        namespace: BLOCK,
                        name: name.clone(),
                    })
                    .collect(),
                Shape::Callout(callout) => {
                    let mut keys: Vec<SymbolKey> = arrowheads([&callout.arrowhead]).collect();
                    if let CalloutContent::Symbol { symbol, .. } = &callout.content {
                        keys.push(symbol.key.clone());
                    }
                    keys
                }
                _ => Vec::new(),
            };
            keys.into_iter().map(|key| (key, &element.source))
        })
    }
    fn arrowheads<'a>(
        arrowheads: impl IntoIterator<Item = &'a Arrowhead>,
    ) -> impl Iterator<Item = SymbolKey> {
        arrowheads
            .into_iter()
            .filter_map(|arrowhead| match arrowhead {
                Arrowhead::Symbol(key) => Some(key.clone()),
                Arrowhead::Default => None,
            })
    }
    let mut by_name = BTreeMap::new();
    for (index, symbol) in symbols.iter().enumerate() {
        by_name.entry(&symbol.key).or_insert(index);
    }
    let inner = symbols
        .iter()
        .flat_map(|symbol| placed(&symbol.space.elements));
    let layouts = || {
        out.layouts
            .iter()
            .flat_map(|(_, _, sheet, _)| &sheet.elements)
    };
    let others = referenced(&out.model)
        .chain(referenced(&out.paper))
        .chain(layouts().flat_map(|element| referenced(std::slice::from_ref(element))))
        .chain(
            symbols
                .iter()
                .flat_map(|symbol| referenced(&symbol.space.elements)),
        )
        .chain(out.dimension_styles.iter().flat_map(|style| {
            // 両端と引出線が同じブロックを指すときは、寸法スタイルごとに 1 つと数える
            let keys: BTreeSet<SymbolKey> =
                arrowheads(style.ends.iter().chain([&style.leader])).collect();
            keys.into_iter().map(|key| (key, &style.source))
        }));
    let placements = placed(&out.model)
        .chain(placed(&out.paper))
        .chain(layouts().flat_map(|element| placed(std::slice::from_ref(element))))
        .chain(inner)
        .map(|(key, at)| (key.clone(), at));
    for (key, at) in placements.chain(others) {
        work(cx, 1, at)?;
        // 点の記号（`dxf.point`）は、ブロックの定義を指さない
        if key.namespace == BLOCK && !by_name.contains_key(&key) {
            cx.report(
                Diagnostic::new(codes::DXF_BLOCK_MISSING)
                    .with("name", key.name.clone())
                    .at(at.clone()),
            );
        }
    }
    let edges: Vec<Vec<usize>> = symbols
        .iter()
        .map(|symbol| {
            placed(&symbol.space.elements)
                .filter_map(|(key, _)| by_name.get(key).copied())
                .collect()
        })
        .collect();
    for (index, symbol) in symbols.iter().enumerate() {
        let mut seen = vec![false; symbols.len()];
        let mut next = edges.get(index).cloned().unwrap_or_default();
        while let Some(at) = next.pop() {
            work(cx, 1, &symbol.source)?;
            if at == index {
                cx.report(
                    Diagnostic::new(codes::DXF_BLOCK_CYCLE_INVALID)
                        .with("name", symbol.key.name.clone())
                        .at(symbol.source.clone()),
                );
                break;
            }
            if let Some(visited @ false) = seen.get_mut(at) {
                *visited = true;
                next.extend(edges.get(at).into_iter().flatten());
            }
        }
    }
    Ok(())
}

/// 作業を数える。止めたら、扱っていた項目か図形の出所を場所に入れる（決定 0119）
fn work(cx: &mut Cx, amount: u64, place: &Ref) -> Result<(), Error> {
    cx.check(Kind::Work, amount).map_err(|stop| {
        cx.report(stop.to_diagnostic().at(place.clone()));
        Error::Stopped(stop)
    })
}

/// 出所。ハンドルがあれば、変わらない鍵として足す（決定 0156）
fn source(object: &Object) -> Ref {
    let at = match object.origin {
        Origin::Dxf { record } => Ref::at_record(record as u64),
        Origin::Dwg { offset, end } => Ref::at_bytes(offset as u64, end as u64),
        Origin::New => Ref::default(),
    };
    match object.handle() {
        Some(handle) => at.with_key(Key::Handle(handle)),
        None => at,
    }
}

/// 空間の SORTENTSTABLE の出所と、図形のハンドルから並べ替えのハンドルへの対応
type SortKeys = (Ref, BTreeMap<u64, u64>);

/// モデル空間とペーパー空間の SortKeys と、ほかのブロックのレコードのハンドルごとの SortKeys
type SpaceKeys = ([Option<SortKeys>; 2], BTreeMap<u64, SortKeys>);

/// 描く順に並べたもの。`items` は、入力の順のものとハンドルの組。並べ替えのハンドルの小さいものから描き、
/// 表にない図形は自分のハンドルで並べる（DXF Reference「SORTENTSTABLE」）。同じハンドルなら入力の順。
/// 表がなければ入力の順のまま。ハンドルのない図形があれば並べ替えられないので、入力の順のままにして診断する
fn arrange<T>(keys: Option<&SortKeys>, items: Vec<(T, Option<u64>)>, cx: &mut Cx) -> Vec<T> {
    let Some((table, sort)) = keys else {
        return items.into_iter().map(|(item, _)| item).collect();
    };
    if items.iter().any(|(_, handle)| handle.is_none()) {
        cx.report((*invalid(b"SORTENTSTABLE", 331)).at(table.clone()));
        return items.into_iter().map(|(item, _)| item).collect();
    }
    let mut keyed: Vec<(u64, T)> = items
        .into_iter()
        .map(|(item, handle)| {
            let handle = handle.unwrap_or_default();
            (sort.get(&handle).copied().unwrap_or(handle), item)
        })
        .collect();
    keyed.sort_by_key(|(key, _)| *key);
    keyed.into_iter().map(|(_, item)| item).collect()
}

/// INSERT の属性（ATTRIB）と POLYLINE の頂点（VERTEX）と、終わりの印（SEQEND）。所有者（330）が INSERT か
/// POLYLINE を指すもの、所有者がないか図形でないものは、続く印（66 = 1）を持つ INSERT か POLYLINE の後に並ぶ
/// ものを、そのもののとする（DXF Reference「INSERT」「POLYLINE」）。INSERT と POLYLINE の添字から ATTRIB と
/// VERTEX の添字への対応と、どれかのものとした添字を返す
fn attached(entities: &[&Object]) -> (BTreeMap<usize, Vec<usize>>, BTreeSet<usize>) {
    let by_handle: BTreeMap<u64, usize> = entities
        .iter()
        .enumerate()
        .filter_map(|(index, object)| Some((object.handle()?, index)))
        .collect();
    let mut attributes: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    let mut owned = BTreeSet::new();
    let mut open = None;
    for (index, object) in entities.iter().enumerate() {
        match object.kind.as_slice() {
            b"INSERT" | b"POLYLINE" => {
                open = matches!(object.first(66), Some(Data::Int(1) | Data::Bool(true)))
                    .then_some(index);
            }
            kind @ (b"ATTRIB" | b"VERTEX" | b"SEQEND") => {
                let expected = |parent: &[u8]| match kind {
                    b"ATTRIB" => parent == b"INSERT",
                    b"VERTEX" => parent == b"POLYLINE",
                    _ => parent == b"INSERT" || parent == b"POLYLINE",
                };
                let parent = object
                    .owner()
                    .and_then(|owner| by_handle.get(&owner).copied())
                    .or(open)
                    .filter(|parent| entities.get(*parent).is_some_and(|o| expected(&o.kind)));
                if let Some(parent) = parent {
                    owned.insert(index);
                    if kind != b"SEQEND" {
                        attributes.entry(parent).or_default().push(index);
                    }
                }
                if kind == b"SEQEND" {
                    open = None;
                }
            }
            _ => open = None,
        }
    }
    (attributes, owned)
}

/// 見出しの `$INSUNITS`（DXF Reference「HEADER Section Group Codes」）。長さの単位でなければ不明
fn unit(db: &AcadDb, cx: &mut Cx) -> Unit {
    let code = db
        .variable(b"$INSUNITS")
        .and_then(|values| values.iter().find(|v| v.code == 70))
        .and_then(|value| match value.data {
            Data::Int(code) => Some(code),
            _ => None,
        });
    let unit = code.map_or(Unit::Unknown, unit_of);
    if unit == Unit::Unknown {
        cx.report(Diagnostic::new(codes::DXF_UNITS_MISSING));
    }
    unit
}

/// ブロックのレコードの挿す単位の番号。R2007 からは 70 に、R2004 までは拡張データの ACAD の
/// `DesignCenter Data` の組（1002 の `{`、版の 1070、単位の 1070、1002 の `}`）にある（試料の図面で確かめた）
fn insertion_units(record: &Object) -> Option<i64> {
    if let Some(Data::Int(code)) = record.first(70) {
        return Some(*code);
    }
    let values = &record.values;
    let at = values.windows(2).position(|pair| {
        matches!(pair, [app, name]
            if app.code == 1001
                && app.data == Data::Text(b"ACAD".to_vec())
                && name.code == 1000
                && name.data == Data::Text(b"DesignCenter Data".to_vec()))
    })?;
    match values.get(at + 2..at + 5)? {
        [open, version, units]
            if open.code == 1002 && version.code == 1070 && units.code == 1070 =>
        {
            match units.data {
                Data::Int(code) => Some(code),
                _ => None,
            }
        }
        _ => None,
    }
}

/// 単位の番号（`$INSUNITS` と、ブロックのレコードの挿す単位で共通）。長さの単位でなければ不明
fn unit_of(code: i64) -> Unit {
    match code {
        1 => Unit::Inch,
        2 => Unit::Foot,
        4 => Unit::Millimeter,
        5 => Unit::Centimeter,
        6 => Unit::Meter,
        _ => Unit::Unknown,
    }
}

fn header_real(values: &[crate::Value], code: i16) -> Option<f64> {
    match values.iter().find(|v| v.code == code)?.data {
        Data::Real(value) => Some(value),
        _ => None,
    }
}

#[derive(Default)]
struct Out {
    model: Vec<Element<ModelPoint>>,
    /// `model` の図形のハンドル
    model_handles: Vec<Option<u64>>,
    paper: Vec<Element<PaperPoint>>,
    views: Vec<View>,
    /// 用紙の上に描くものと、そのハンドル。順序は入力の順
    sheet: Vec<(SheetItem, Option<u64>)>,
    /// ほかのレイアウトの用紙と、そのタブの順と LAYOUT の位置（オブジェクトの並びの中）。中身のない用紙の ID は、
    /// 最後に付ける。中身のない用紙は、要素を足すための座標の対応も持つ
    layouts: Vec<(i64, Option<usize>, Sheet, Option<PaperFrame>)>,
    /// 中身のあるほかのレイアウトの用紙の数。ID を決めるために毎回並びを探し直さない
    filled_layouts: u32,
    layers: Vec<Layer>,
    /// ASCII の大文字を小文字にした画層の名前。DXF の名前は大文字と小文字を区別しない（推定。docs/formats/dxf.md）
    by_name: BTreeMap<String, LayerId>,
    /// 画層の表のレコードのハンドル
    by_handle: BTreeMap<u64, LayerId>,
    line_types: Vec<LineType>,
    /// ASCII の大文字を小文字にした線種の名前
    line_type_names: BTreeMap<String, LineTypeId>,
    /// 線種の表のレコードのハンドル
    line_type_handles: BTreeMap<u64, LineTypeId>,
    dimension_styles: Vec<DimensionStyle>,
    text_styles: Vec<TextStyle>,
    extensions: Vec<Extension>,
    /// 要素を置いた用紙ごとの座標の対応。中身のない用紙のものは、ID を付けるときに足す
    frames: BTreeMap<SheetId, PaperFrame>,
}

#[derive(Clone)]
struct Lifter<'a> {
    /// ペーパー空間のブロックのレコードのハンドルから、LAYOUT の位置を引く索引
    layouts: &'a BTreeMap<u64, usize>,
    /// ペーパー空間の図形を置く用紙
    sheet: SheetId,
    /// 2007 以降の版なら、文字の値を UTF-8 として読む
    unicode: bool,
    /// 2010 以降の版なら、DXF の HATCH の SPLINE の辺は当てはめる点（97）を持つ
    fit_data: bool,
    /// 2010 以降の版なら、DWG の表は内容を TABLECONTENT の形で持つ。DWG の読み手は、その書式と TABLESTYLE を
    /// まだ読まない
    table_content: bool,
    /// 見出しの `$ACADVER`。形式固有の情報の版にする
    version: Option<String>,
    /// 文書の長さの単位。記号の定義の空間も、この単位とする
    unit: Unit,
    /// 1 単位の mm の数。不明なら `None`
    mm: Option<f64>,
    /// 用紙の原点（`$PLIMMIN`、図面の単位）
    origin: [f64; 2],
    /// 点の表し方（`$PDMODE`）。なければ 0
    point_mode: i64,
}

impl Lifter<'_> {
    fn entity(
        &self,
        db: &AcadDb,
        object: &Object,
        source: Ref,
        attributes: &[&Object],
        out: &mut Out,
        cx: &mut Cx,
    ) -> Lifted<()> {
        let paper = self.int(object, 67, 0)? == 1;
        self.place(db, object, source, attributes, paper, out, cx)
    }

    /// 図形を、`paper` ならペーパー空間の用紙（`self.sheet`）に、でなければモデル空間に置く
    #[allow(clippy::too_many_arguments)]
    fn place(
        &self,
        db: &AcadDb,
        object: &Object,
        source: Ref,
        attributes: &[&Object],
        paper: bool,
        out: &mut Out,
        cx: &mut Cx,
    ) -> Lifted<()> {
        let kind = object.kind.as_slice();
        let mut paper_mm = None;
        match kind {
            // 69 = 1 は用紙そのものを表すビューポート
            b"VIEWPORT" if paper && self.int(object, 69, 0)? == 1 => {}
            b"VIEWPORT" if paper => {
                let mm = self.paper_mm(kind)?;
                let mut view = self.view(object, mm, source.clone())?;
                // 341 はビューポートで凍結した画層。指す画層がなければ、その指定だけを除く
                for value in object.values.iter().filter(|value| value.code == 341) {
                    let layer = match value.data {
                        Data::Handle(handle) => out.by_handle.get(&handle).copied(),
                        _ => None,
                    };
                    match layer {
                        Some(layer) => view.hidden_layers.push(layer),
                        None => cx.report((*invalid(kind, 341)).at(source.clone())),
                    }
                }
                out.sheet
                    .push((SheetItem::View(out.views.len()), object.handle()));
                out.views.push(view);
                paper_mm = Some(mm);
            }
            b"LINE" | b"CIRCLE" | b"ARC" | b"ELLIPSE" | b"LWPOLYLINE" | b"SPLINE" | b"TEXT"
            | b"MTEXT" | b"DIMENSION" | b"LEADER" | b"MULTILEADER" | b"INSERT" | b"HATCH"
            | b"ACAD_TABLE" | b"IMAGE" | b"PDFUNDERLAY" | b"DWFUNDERLAY" | b"DGNUNDERLAY"
            | b"POLYLINE" | b"SOLID" | b"TRACE" | b"POINT"
                if paper =>
            {
                let mm = self.paper_mm(kind)?;
                let [ox, oy] = self.origin;
                let point = |x: f64, y: f64| PaperPoint {
                    sheet: self.sheet,
                    x: (x - ox) * mm,
                    y: (y - oy) * mm,
                };
                let length = |value: f64| Length::Paper(PaperLength(value * mm));
                let element =
                    self.element(db, object, source, attributes, point, length, out, cx)?;
                paper_mm = Some(mm);
                let target = Target::Paper {
                    sheet: self.sheet,
                    index: out.paper.len(),
                };
                self.extend(
                    db,
                    object,
                    target,
                    || Basis::Paper(element.clone()),
                    out,
                    cx,
                );
                out.sheet
                    .push((SheetItem::Element(out.paper.len()), object.handle()));
                out.paper.push(element);
            }
            b"LINE" | b"CIRCLE" | b"ARC" | b"ELLIPSE" | b"LWPOLYLINE" | b"SPLINE" | b"TEXT"
            | b"MTEXT" | b"DIMENSION" | b"LEADER" | b"MULTILEADER" | b"INSERT" | b"HATCH"
            | b"ACAD_TABLE" | b"IMAGE" | b"PDFUNDERLAY" | b"DWFUNDERLAY" | b"DGNUNDERLAY"
            | b"POLYLINE" | b"SOLID" | b"TRACE" | b"POINT" => {
                let point = |x: f64, y: f64| ModelPoint { space: MODEL, x, y };
                let length = |value: f64| {
                    Length::Model(ModelLength {
                        space: MODEL,
                        value,
                    })
                };
                let element =
                    self.element(db, object, source, attributes, point, length, out, cx)?;
                let target = Target::Model {
                    space: MODEL,
                    index: out.model.len(),
                };
                self.extend(
                    db,
                    object,
                    target,
                    || Basis::Model(element.clone()),
                    out,
                    cx,
                );
                out.model_handles.push(object.handle());
                out.model.push(element);
            }
            _ => {
                return Err(Box::new(
                    Diagnostic::new(codes::DXF_LIFT_DROPPED).with("kind", text(kind)),
                ));
            }
        }
        if let Some(mm) = paper_mm {
            out.frames.insert(
                self.sheet,
                PaperFrame {
                    origin: self.origin,
                    mm,
                    home: Home {
                        owner: object.owner(),
                        blocks: object.section == Section::Blocks,
                        paper: matches!(object.first(67), Some(Data::Int(1))),
                    },
                },
            );
        }
        Ok(())
    }

    /// 図形を要素にする。INSERT の属性のうち移せないものは、属性の出所を指す診断を返し、INSERT は移す。
    /// R2010 からの DWG の表は、書式を移さないことを表の出所を指す診断で返し、表は移す
    #[allow(clippy::too_many_arguments)]
    fn element<P: Copy>(
        &self,
        db: &AcadDb,
        object: &Object,
        source: Ref,
        attributes: &[&Object],
        point: impl Fn(f64, f64) -> P,
        length: impl Fn(f64) -> Length,
        out: &mut Out,
        cx: &mut Cx,
    ) -> Lifted<Element<P>> {
        // POLYLINE は、続く VERTEX（`attributes`）を頂点にする
        let mut shape = if object.kind == b"POLYLINE" {
            self.planar(object)?;
            Shape::Polyline(self.polyline(object, attributes, &point, &length)?)
        } else {
            self.shape(db, object, &point, &length)?
        };
        let style = self.style(object, out)?;
        let layer = self.layer(object, out)?;
        if let Shape::Callout(callout) = &mut shape {
            let symbol = matches!(callout.content, CalloutContent::Symbol { .. });
            (callout.line_style, callout.content_color) =
                self.callout_style(db, object, symbol, out, cx)?;
        }
        if let Shape::Table(_) = &shape {
            report_cell_values(object, cx);
            // R2010 からの DWG の読み手は、表のスタイル・セルのスタイル・内容の書式を読まない（docs/formats/dwg.md）
            if self.table_content && matches!(object.origin, Origin::Dwg { .. }) {
                cx.report(Diagnostic::new(codes::DWG_TABLE_FORMAT_UNSUPPORTED).at(source.clone()));
            }
        }
        if let Shape::Symbol(symbol) = &mut shape {
            // 切り抜きを読めなければ、切り抜かずに移す
            match self.symbol_clip(db, object) {
                Ok(clip) => symbol.clip = clip,
                Err(diagnostic) => cx.report((*diagnostic).at(source.clone())),
            }
            for attribute in attributes {
                match self.attribute(attribute, &point, &length, out) {
                    Ok(lifted) => symbol.attributes.push(lifted),
                    Err(diagnostic) => {
                        cx.report((*diagnostic).at(crate::drawing::source(attribute)))
                    }
                }
            }
        }
        Ok(Element {
            source,
            layer,
            style,
            shape,
        })
    }

    fn shape<P: Copy>(
        &self,
        db: &AcadDb,
        object: &Object,
        point: &impl Fn(f64, f64) -> P,
        length: &impl Fn(f64) -> Length,
    ) -> Lifted<Shape<P>> {
        let kind = object.kind.as_slice();
        self.planar(object)?;
        match kind {
            b"DIMENSION" => Ok(Shape::Dimension(self.dimension(object, point, length)?)),
            b"LEADER" => Ok(Shape::Leader(self.leader(db, object, point)?)),
            b"MULTILEADER" => Ok(Shape::Callout(self.callout(db, object, point, length)?)),
            b"ACAD_TABLE" => Ok(Shape::Table(self.table(db, object, point, length)?)),
            b"IMAGE" => Ok(Shape::Underlay(self.image(db, object, point)?)),
            b"PDFUNDERLAY" | b"DWFUNDERLAY" | b"DGNUNDERLAY" => {
                Ok(Shape::Underlay(self.underlay(db, object, point, length)?))
            }
            b"INSERT" => Ok(Shape::Symbol(self.symbol(object, point, length)?)),
            b"HATCH" => Ok(Shape::Hatch(self.hatch(db, object, point, length)?)),
            b"LINE" => self.line(object, point),
            b"SOLID" | b"TRACE" => Ok(Shape::Hatch(self.solid(object, point)?)),
            b"POINT" => Ok(Shape::Symbol(self.point(object, point)?)),
            b"CIRCLE" => Ok(Shape::Circle {
                center: self.planar_point(object, 10, point)?,
                radius: length(self.radius(object)?),
            }),
            // ARC（DXF Reference「ARC」）。角度は度で、反時計回り
            b"ARC" => Ok(Shape::Curve(Edge::Arc {
                center: self.planar_point(object, 10, point)?,
                radius: length(self.radius(object)?),
                start: self.real(object, 50, None)?,
                end: self.real(object, 51, None)?,
                counterclockwise: true,
            })),
            b"ELLIPSE" => Ok(Shape::Curve(self.ellipse(object, point)?)),
            b"LWPOLYLINE" => Ok(Shape::Polyline(self.lwpolyline(object, point, length)?)),
            b"SPLINE" => Ok(Shape::Curve(Edge::Spline(self.spline(object, point)?))),
            // 段組みの欄は、R2018 からは埋め込みのオブジェクト（101）に置く（推定。docs/formats/dxf.md）
            b"MTEXT" if object.first(101).is_some() => Err(columns()),
            b"MTEXT" => Ok(Shape::Formatted(self.formatted(object, point, length)?)),
            // TEXT（DXF Reference「TEXT」）
            _ => Ok(Shape::Text(self.text(
                object,
                point,
                length,
                73,
                "dxf.TEXT.40",
            )?)),
        }
    }

    /// 真偽の値。なければ `default`
    fn flag(&self, object: &Object, code: i16, default: bool) -> Lifted<bool> {
        match object.first(code) {
            None => Ok(default),
            Some(Data::Bool(flag)) => Ok(*flag),
            Some(Data::Int(flag @ (0 | 1))) => Ok(*flag == 1),
            Some(_) => Err(invalid(&object.kind, code)),
        }
    }

    fn planar_point<P>(
        &self,
        object: &Object,
        x: i16,
        point: &impl Fn(f64, f64) -> P,
    ) -> Lifted<P> {
        if self.real(object, x + 20, Some(0.0))? != 0.0 {
            return Err(non_planar(&object.kind));
        }
        Ok(point(
            self.real(object, x, None)?,
            self.real(object, x + 10, None)?,
        ))
    }

    /// 押し出しの向き（210・220・230）が Z でなければ、平面に移さない
    fn planar(&self, object: &Object) -> Lifted<()> {
        let extrusion = [
            self.real(object, 210, Some(0.0))?,
            self.real(object, 220, Some(0.0))?,
            self.real(object, 230, Some(1.0))?,
        ];
        if extrusion == [0.0, 0.0, 1.0] {
            Ok(())
        } else {
            Err(non_planar(&object.kind))
        }
    }

    /// 有限の実数の値。なければ `default`、それもなければ読めないとする
    fn real(&self, object: &Object, code: i16, default: Option<f64>) -> Lifted<f64> {
        match object.first(code) {
            Some(Data::Real(value)) => Some(*value).filter(|v| v.is_finite()),
            Some(_) => None,
            None => default,
        }
        .ok_or_else(|| invalid(&object.kind, code))
    }

    /// 整数の値。なければ `default`
    fn int(&self, object: &Object, code: i16, default: i64) -> Lifted<i64> {
        match object.first(code) {
            Some(Data::Int(value)) => Ok(*value),
            Some(_) => Err(invalid(&object.kind, code)),
            None => Ok(default),
        }
    }

    /// 文字の値。2007 より前の版の ASCII でない文字は、コード・ページを読まないので受け付けない
    fn string(&self, object: &Object, code: i16) -> Lifted<Option<String>> {
        match object.first(code) {
            Some(data) => self.decode(&object.kind, code, data).map(Some),
            None => Ok(None),
        }
    }

    /// 文字の値を読む
    fn decode(&self, kind: &[u8], code: i16, data: &Data) -> Lifted<String> {
        let Data::Text(value) = data else {
            return Err(invalid(kind, code));
        };
        if self.unicode || value.is_ascii() {
            match core::str::from_utf8(value) {
                Ok(value) => Ok(value.to_owned()),
                Err(_) => Err(invalid(kind, code)),
            }
        } else {
            Err(Box::new(
                Diagnostic::new(codes::DXF_CODE_PAGE_UNSUPPORTED).with("kind", text(kind)),
            ))
        }
    }
}

/// 図形の値の並びを、先頭から順に読む
struct Values<'a> {
    /// 図形の種類
    kind: &'a [u8],
    /// まだ読んでいない値
    values: &'a [Value],
}

impl<'a> Values<'a> {
    /// 次の値のグループ・コード
    fn next(&self) -> Option<i16> {
        self.values.first().map(|value| value.code)
    }

    /// 次の値。グループ・コードが `code` でなければ読めないとする
    fn take(&mut self, code: i16) -> Lifted<&'a Data> {
        match self.values.split_first() {
            Some((value, rest)) if value.code == code => {
                self.values = rest;
                Ok(&value.data)
            }
            _ => Err(invalid(self.kind, code)),
        }
    }

    /// 次の有限の実数
    fn real(&mut self, code: i16) -> Lifted<f64> {
        match self.take(code)? {
            Data::Real(value) if value.is_finite() => Ok(*value),
            _ => Err(invalid(self.kind, code)),
        }
    }

    /// 次の整数
    fn int(&mut self, code: i16) -> Lifted<i64> {
        match self.take(code)? {
            Data::Int(value) => Ok(*value),
            _ => Err(invalid(self.kind, code)),
        }
    }

    /// 次の 0 以上の数
    fn count(&mut self, code: i16) -> Lifted<usize> {
        usize::try_from(self.int(code)?).map_err(|_| invalid(self.kind, code))
    }
}

/// AutoCAD の色の番号。1〜255 のものだけを渡す
fn aci(index: i64) -> ColorCode {
    ColorCode {
        system: ColorSystem::Aci,
        value: index as u32,
    }
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn invalid(kind: &[u8], code: i16) -> Box<Diagnostic> {
    Box::new(
        Diagnostic::new(codes::DXF_LIFT_INVALID)
            .with("kind", text(kind))
            .with("code", i64::from(code)),
    )
}

/// スプラインの定義の数が合うか。制御点があれば、次数より多く、重みは空か同じ数で正、周期的でなければ
/// 節点は制御点の数と次数と 1 の和。制御点も当てはめる点もなければ読めないとする
fn check_spline<P>(kind: &[u8], spline: &Spline<P>) -> Lifted<()> {
    let degree = spline.degree as usize;
    let controls = spline.control.len();
    if spline.degree == 0 {
        return Err(invalid(kind, 71));
    }
    if controls == 0 && spline.fit.is_empty() {
        return Err(invalid(kind, 10));
    }
    if controls > 0 && controls <= degree {
        return Err(invalid(kind, 10));
    }
    if !(spline.weights.is_empty() || spline.weights.len() == controls)
        || spline.weights.iter().any(|weight| *weight <= 0.0)
    {
        return Err(invalid(kind, 41));
    }
    if controls > 0 && !spline.periodic && spline.knots.len() != controls + degree + 1 {
        return Err(invalid(kind, 40));
    }
    Ok(())
}

fn columns() -> Box<Diagnostic> {
    Box::new(Diagnostic::new(codes::DXF_MTEXT_COLUMNS_UNSUPPORTED))
}

fn non_planar(kind: &[u8]) -> Box<Diagnostic> {
    Box::new(Diagnostic::new(codes::DXF_NON_PLANAR_DROPPED).with("kind", text(kind)))
}

/// 表のレコードの XDATA の ACAD の、TrueType の書体の名前（1000）と印（1071）。名前が空か、なければ `None`
fn true_type_face(object: &Object) -> Option<(&Data, Option<i64>)> {
    let start = object.values.iter().position(|value| {
        value.code == 1001 && matches!(&value.data, Data::Text(name) if name.as_slice() == b"ACAD")
    })?;
    let group = object.values.get(start + 1..)?;
    let group = group.split(|value| value.code == 1001).next()?;
    let face = group
        .iter()
        .find_map(|value| match (&value.code, &value.data) {
            (1000, face @ Data::Text(name)) if !name.is_empty() => Some(face),
            _ => None,
        })?;
    let bits = group
        .iter()
        .find_map(|value| match (&value.code, &value.data) {
            (1071, Data::Int(bits)) => Some(*bits),
            _ => None,
        });
    Some((face, bits))
}

#[cfg(test)]
mod tests {
    use super::*;
    use zumen_drawing::{Color, Inherit};

    #[test]
    fn table_style_rows_without_text_styles_keep_their_formats() {
        let mut db = AcadDb::default();
        db.push(Object {
            kind: b"STYLE".to_vec(),
            section: Section::Tables,
            values: vec![
                Value {
                    code: 5,
                    data: Data::Handle(0x11),
                },
                Value {
                    code: 2,
                    data: Data::Text(b"Standard".to_vec()),
                },
            ],
            origin: Origin::New,
        });
        let mut values = Vec::new();
        for (handle, height, color) in [(0, 1.0, 1), (0x11, 2.0, 2), (0, 3.0, 3)] {
            values.extend([
                Value {
                    code: 7,
                    data: Data::Handle(handle),
                },
                Value {
                    code: 140,
                    data: Data::Real(height),
                },
                Value {
                    code: 62,
                    data: Data::Int(color),
                },
            ]);
        }
        let style = Object {
            kind: b"TABLESTYLE".to_vec(),
            section: Section::Objects,
            values,
            origin: Origin::New,
        };
        let layouts = BTreeMap::new();
        let lifter = Lifter {
            layouts: &layouts,
            sheet: SHEET,
            unicode: true,
            fit_data: true,
            table_content: true,
            version: None,
            unit: Unit::Millimeter,
            mm: Some(1.0),
            origin: [0.0; 2],
            point_mode: 0,
        };
        let formats = lifter
            .table_formats(&db, &style, &|value| Length::Paper(PaperLength(value)))
            .unwrap()
            .unwrap();
        for (row, height, color, text_style) in [
            (&formats.data_format, 1.0, 1, None),
            (&formats.title_format, 2.0, 2, Some("Standard")),
            (&formats.header_format, 3.0, 3, None),
        ] {
            assert_eq!(row.text_height, Some(Length::Paper(PaperLength(height))));
            assert_eq!(
                row.text_color,
                Some(Inherit::Own(Color {
                    code: Some(aci(color)),
                    rgb: None
                }))
            );
            assert_eq!(row.text_style.as_deref(), text_style);
        }
    }

    /// 版が `version` の DWG（`dwg` が偽なら DXF）の、1 行 1 列の表と、欄のない TABLESTYLE を持つ表。DWG の
    /// 読み手は、R2010 からの TABLESTYLE の欄を読まず、ACAD_TABLE を AcDbTable（100）なしで置き、表を描いた
    /// ブロック（2）を値の終わりに置く
    fn table_db(version: &str, dwg: bool) -> AcadDb {
        let origin = |at: usize| {
            if dwg {
                Origin::Dwg {
                    offset: at,
                    end: at + 1,
                }
            } else {
                Origin::Dxf { record: at }
            }
        };
        let value = |code: i16, data: Data| Value { code, data };
        let text = |code: i16, text: &str| value(code, Data::Text(text.as_bytes().to_vec()));
        let mut db = AcadDb::default();
        db.set_variable(b"$ACADVER", vec![text(1, version)]);
        db.push(Object {
            kind: b"TABLESTYLE".to_vec(),
            section: Section::Objects,
            values: vec![value(5, Data::Handle(0x40))],
            origin: origin(0),
        });
        let mut values = vec![
            value(5, Data::Handle(0x41)),
            text(8, "0"),
            value(10, Data::Real(0.0)),
            value(20, Data::Real(0.0)),
            value(30, Data::Real(0.0)),
        ];
        if !dwg {
            values.extend([text(2, "*T1"), text(100, "AcDbTable")]);
        }
        values.extend([
            value(342, Data::Handle(0x40)),
            value(91, Data::Int(1)),
            value(92, Data::Int(1)),
            value(141, Data::Real(1.0)),
            value(142, Data::Real(2.0)),
            value(171, Data::Int(1)),
            text(1, "A"),
        ]);
        if dwg {
            values.push(text(2, "*T1"));
        }
        db.push(Object {
            kind: b"ACAD_TABLE".to_vec(),
            section: Section::Entities,
            values,
            origin: origin(1),
        });
        db
    }

    /// R2010 からの DWG の表は、書式を読まないことを、表ごとに表の出所を指す `dwg.table-format.unsupported`
    /// で返し、表は移す。R2004 までの DWG と、DXF の表は返さない
    #[test]
    fn dwg_tables_from_2010_report_that_their_formats_are_not_read() {
        use zumen_base::{Budget, CancelToken, Diagnostics};
        for (version, dwg, reported) in [
            ("AC1024", true, true),
            ("AC1027", true, true),
            ("AC1032", true, true),
            ("AC1018", true, false),
            ("AC1024", false, false),
            ("AC1032", false, false),
        ] {
            let db = table_db(version, dwg);
            let mut diag = Diagnostics::new();
            let mut budget = Budget::default();
            let cancel = CancelToken::new();
            let drawing = lift(&db, &mut Cx::new(&mut budget, &mut diag, &cancel)).unwrap();
            let [element] = drawing.spaces[0].elements.as_slice() else {
                panic!("{version} {dwg}: {:?}", drawing.spaces[0].elements);
            };
            let Shape::Table(table) = &element.shape else {
                panic!("{version} {dwg}: {:?}", element.shape);
            };
            assert_eq!(table.formats, None, "{version} {dwg}");
            let found: Vec<_> = diag
                .iter()
                .filter(|d| d.code() == codes::DWG_TABLE_FORMAT_UNSUPPORTED)
                .map(|d| (d.count(), d.places().to_vec()))
                .collect();
            let expected = if reported {
                vec![(1, vec![source(&db.objects()[1])])]
            } else {
                Vec::new()
            };
            assert_eq!(found, expected, "{version} {dwg}");
            assert_eq!(element.source, source(&db.objects()[1]), "{version} {dwg}");
        }
    }
}
