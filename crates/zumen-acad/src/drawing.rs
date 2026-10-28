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
//! DWG でも `dxf.` で始まるものを使う。何をどう読んだかの推定は `docs/formats/dxf.md` に記録した。

mod apply;
mod mtext;

use std::collections::{BTreeMap, BTreeSet};

use crate::db::{AcadDb, Data, Object, Origin, Section, Value};
use crate::{DwgFile, DxfFile};
use zumen_base::{Cx, Diagnostic, Kind, Ref, codes};
use zumen_drawing::{
    Affine, Arrowhead, Attribute, AttributeDefinition, AttributeValue, Axis, Background,
    BackgroundFill, Basis, Border, Borders, Boundary, Branch, Callout, CalloutContent, Cell,
    CellContent, CellData, CellFormat, CellValue, Clip, Color, ColorCode, ColorSystem, DateTime,
    Dimension, DimensionKind, DimensionStyle, DimensionText, Dogleg, Drawing2D, Edge, Element,
    Extension, Field, FieldValue, Fill, Fit, Flow, Font, FontKind, FormattedText, Gradient, Grid,
    Hatch, HatchLoop, Height, HeightMeaning, Horizontal, Inherit, Islands, Label, Layer, LayerId,
    LayerState, LayerStyle, Leader, Length, LineSpacing, LineType, LineTypeDefinition, LineTypeId,
    LineTypeMark, LineWeight, MarkContent, MarkRotation, Measured, ModelLength, ModelPoint,
    ModelSpace, PaperLength, PaperPoint, PathKind, Pattern, PatternBasis, PatternLine,
    PatternOrigin, Placement, Polyline, Resource, Rotation, Shape, Sheet, SheetId, SheetItem,
    SpaceId, Spline, Style, Symbol, SymbolDefinition, SymbolKey, Table, TableFormats, Target, Text,
    Underlay, UnderlayFrame, UnderlayKind, Unit, ValueUnit, Vertex, Vertical, View,
};
use zumen_sdk::{Error, Lift};

/// UTF-8 で書く最初の版（2007）
const UTF8: &[u8] = b"AC1021";

/// HATCH の SPLINE の辺が当てはめる点を持つ最初の版（2010）
const FIT_DATA: &[u8] = b"AC1024";

/// 移さない理由。診断は大きいので箱に入れる
type Lifted<T> = Result<T, Box<Diagnostic>>;

const MODEL: SpaceId = SpaceId(0);
const SHEET: SheetId = SheetId(0);

/// ブロックの定義を指す記号のキーの名前空間
const BLOCK: &str = "dxf.block";

/// XDATA の形式固有の情報の名前空間と、値の型の契約の版（決定 0115）
const XDATA: &str = "dxf.xdata";

const POINT: &str = "dxf.point";
const XDATA_SCHEMA: u32 = 1;
/// 図面モデルの値にない図形の欄の拡張の名前空間と、値の型の契約の版（決定 0142）
const ENTITY: &str = "dxf.entity";
const ENTITY_SCHEMA: u32 = 1;
/// 図形の拡張辞書の XRECORD の形式固有の情報の名前空間と、値の型の契約の版（決定 0148）
const XRECORD: &str = "dxf.xrecord";
const XRECORD_SCHEMA: u32 = 1;

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
    let lifter = Lifter {
        sheet: SHEET,
        unicode: version.is_some_and(|v| v >= UTF8),
        fit_data: version.is_some_and(|v| v >= FIT_DATA),
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
    for kind in [&b"LTYPE"[..], b"LAYER", b"DIMSTYLE"] {
        for object in db.objects() {
            if object.section != Section::Tables || object.kind != kind {
                continue;
            }
            let source = source(object);
            work(cx, 1, &source)?;
            let lifted = match kind {
                b"LTYPE" => lifter.line_type_record(db, object, source.clone(), &mut out, cx),
                b"LAYER" => lifter.layer_record(object, source.clone(), &mut out),
                _ => lifter.dimension_style_record(db, object, source.clone(), &mut out),
            };
            if let Err(diagnostic) = lifted {
                cx.report((*diagnostic).at(source));
            }
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
    let mut drawing = Drawing2D {
        layers: out.layers,
        line_types: out.line_types,
        line_type_scale,
        default_line_weight,
        dimension_styles: out.dimension_styles,
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
    if !out.paper.is_empty() || !out.views.is_empty() {
        let size = match (limits(b"$PLIMMAX"), lifter.mm) {
            (Some([x, y]), Some(mm)) => {
                let [ox, oy] = lifter.origin;
                let size = [(x - ox) * mm, (y - oy) * mm];
                size.iter().all(|s| *s > 0.0).then(|| size.map(PaperLength))
            }
            _ => None,
        };
        let order = arrange(paper_keys.as_ref(), out.sheet, cx);
        // 今のレイアウトの名前は、ペーパー空間の図形の所有者（今のレイアウトのブロックのレコード）を指す LAYOUT
        // から読む。DWG は、どのレイアウトのブロックのレコードも `*Paper_Space` と名付ける
        let name = entities
            .iter()
            .find(|object| matches!(object.first(67), Some(Data::Int(1))))
            .and_then(|object| object.owner())
            .and_then(|handle| layout_of(db, handle))
            .and_then(|layout| lifter.layout_info(layout).name);
        drawing.sheets.push(Sheet {
            id: SHEET,
            name,
            size,
            elements: out.paper,
            views: out.views,
            order,
        });
    }
    // ほかのレイアウトの用紙は、今のレイアウトの後に、タブの順で並べる
    out.layouts.sort_by_key(|(tab, _)| *tab);
    drawing
        .sheets
        .extend(out.layouts.into_iter().map(|(_, sheet)| sheet));
    Ok(drawing)
}

/// 図形 1 つの作業量。書式つきの文字は、内容の 250 バイトの区切り（3）ごとにも数える。曲線は、頂点・制御点・
/// 当てはめる点・節点（10・11・40）ごとにも数える
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
        _ => 0,
    } + object
        .values
        .iter()
        .filter(|value| value.code >= 1000)
        .count() as u64
}

/// 拡張辞書の項目と XRECORD の値の複製を、移す前に数える。同じ辞書を複数の図形が指しても、
/// 図形ごとに数える。形式固有の情報に控える図形の値の複製も、図形の作業量として加える。
fn extension_work(db: &AcadDb, object: &Object, cx: &mut Cx) -> Result<(), Error> {
    let Some(dictionary) = xdictionary(object)
        .and_then(|handle| db.get(handle))
        .filter(|found| found.kind == b"DICTIONARY")
    else {
        return Ok(());
    };
    let source = source(object);
    for pair in dictionary.values.windows(2) {
        work(cx, 1, &source)?;
        if let [
            Value {
                code: 3,
                data: name,
            },
            Value {
                code: 350 | 360,
                data: Data::Handle(handle),
            },
        ] = pair
            && let Some(item) = db.get(*handle)
        {
            if item.kind == b"XRECORD" {
                work(
                    cx,
                    (item.values.len() as u64).saturating_add(amount(object)),
                    &source,
                )?;
            } else if object.kind == b"INSERT"
                && item.kind == b"DICTIONARY"
                && matches!(name, Data::Text(name) if name == b"ACAD_FILTER")
            {
                // 共有する切り抜きも、配置ごとに辞書の探索と境界の値の複製・変換を数える。
                work(cx, item.values.len() as u64, &source)?;
                if let Some(filter) = entry(db, *handle, b"SPATIAL")
                    .and_then(|handle| db.get(handle))
                    .filter(|filter| filter.kind == b"SPATIAL_FILTER")
                {
                    work(cx, filter.values.len() as u64, &source)?;
                }
            }
        }
    }
    Ok(())
}

/// 用紙とモデルの空間のブロック。中身は空間の図形として移すので、記号の定義にしない
fn layout_block(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    name.starts_with("*model_space") || name.starts_with("*paper_space")
}

/// ペーパー空間のブロック（`*Paper_Space`・`*Paper_Space0` など）か。今のレイアウトの図形は ENTITIES に
/// あるので、BLOCKS にあるペーパー空間のブロックの中身は、ほかのレイアウトのもの。DWG は名前に番号を持たない
fn paper_layout(name: &str) -> bool {
    name.to_ascii_lowercase().starts_with("*paper_space")
}

/// ブロックのレコード `record` を用紙空間に持つ LAYOUT（DXF Reference「LAYOUT」）。レイアウトの欄の
/// 330 で指す。初めの 330 は所有者なので見ない
fn layout_of(db: &AcadDb, record: u64) -> Option<&Object> {
    db.objects().iter().find(|object| {
        object.kind == b"LAYOUT"
            && object
                .values
                .iter()
                .filter(|value| value.code == 330)
                .skip(1)
                .any(|value| value.data == Data::Handle(record))
    })
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
    let layouts = || out.layouts.iter().flat_map(|(_, sheet)| &sheet.elements);
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

fn source(object: &Object) -> Ref {
    match object.origin {
        Origin::Dxf { record } => Ref::at_record(record as u64),
        Origin::Dwg { offset, end } => Ref::at_bytes(offset as u64, end as u64),
        Origin::New => Ref::default(),
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
    /// ほかのレイアウトの用紙と、そのタブの順
    layouts: Vec<(i64, Sheet)>,
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
    extensions: Vec<Extension>,
}

#[derive(Clone)]
struct Lifter {
    /// ペーパー空間の図形を置く用紙
    sheet: SheetId,
    /// 2007 以降の版なら、文字の値を UTF-8 として読む
    unicode: bool,
    /// 2010 以降の版なら、DXF の HATCH の SPLINE の辺は当てはめる点（97）を持つ
    fit_data: bool,
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

impl Lifter {
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
        Ok(())
    }

    /// 図形の XDATA（DXF Reference「Extended Data」、決定 0115）を、登録アプリケーション名（1001）ごとの
    /// 形式固有の情報にして、図形の値とともに控える。最初の 1001 より前の XDATA の値と、読めない値を持つ
    /// 名前の分は移さず、診断する。図面モデルの値にない図形の欄は、`dxf.entity` の拡張に控える（決定 0142）
    fn extend(
        &self,
        db: &AcadDb,
        object: &Object,
        target: Target,
        basis: impl Fn() -> Basis,
        out: &mut Out,
        cx: &mut Cx,
    ) {
        match entity_fields(object) {
            Ok(fields) if !fields.is_empty() => out.extensions.push(Extension {
                namespace: ENTITY,
                name: String::new(),
                version: self.version.clone(),
                schema: ENTITY_SCHEMA,
                target,
                basis: basis(),
                fields,
                source: source(object),
            }),
            Ok(_) => {}
            Err(diagnostic) => cx.report((*diagnostic).at(source(object))),
        }
        let start = object
            .values
            .iter()
            .position(|value| value.code == 1001)
            .unwrap_or(object.values.len());
        let (head, xdata) = object.values.split_at(start);
        if let Some(stray) = head.iter().find(|value| value.code >= 1000) {
            cx.report((*invalid(&object.kind, stray.code)).at(source(object)));
        }
        for group in xdata.chunk_by(|_, next| next.code != 1001) {
            let Some((name, values)) = group.split_first() else {
                continue;
            };
            match self.xdata(&object.kind, name, values) {
                Ok((name, fields)) => out.extensions.push(Extension {
                    namespace: XDATA,
                    name,
                    version: self.version.clone(),
                    schema: XDATA_SCHEMA,
                    target,
                    basis: basis(),
                    fields,
                    source: source(object),
                }),
                Err(diagnostic) => cx.report((*diagnostic).at(source(object))),
            }
        }
        self.dictionary(db, object, target, &basis, out, cx);
    }

    /// 図形の拡張辞書（`{ACAD_XDICTIONARY` の組の 360 が指す DICTIONARY。DXF Reference「Extension
    /// Dictionary and Persistent Reactors」、決定 0148）の XRECORD の項目を、項目の名前ごとの形式固有の
    /// 情報にして、図形の値とともに控える。値は、グループ・コード（`code`）と、その型の値の組を順に並べる。
    /// XRECORD でない項目と、読めない値を持つ XRECORD は移さず、診断する
    fn dictionary(
        &self,
        db: &AcadDb,
        object: &Object,
        target: Target,
        basis: &impl Fn() -> Basis,
        out: &mut Out,
        cx: &mut Cx,
    ) {
        let Some(dictionary) = xdictionary(object)
            .and_then(|handle| db.get(handle))
            .filter(|found| found.kind == b"DICTIONARY")
        else {
            return;
        };
        let mut values = dictionary.values.iter().peekable();
        while let Some(value) = values.next() {
            let (
                Value {
                    code: 3,
                    data: name,
                },
                Some(Value {
                    code: 350 | 360,
                    data: Data::Handle(handle),
                }),
            ) = (value, values.peek())
            else {
                continue;
            };
            values.next();
            // 記号の切り抜きは、記号が持つ（決定 0149）
            if object.kind == b"INSERT"
                && matches!(name, Data::Text(text) if text == b"ACAD_FILTER")
            {
                continue;
            }
            // 項目のオブジェクトがなければ、移すものはない
            let Some(item) = db.get(*handle) else {
                continue;
            };
            let named =
                self.decode(b"DICTIONARY", 3, name)
                    .and_then(|name| match item.kind.as_slice() {
                        b"XRECORD" => Ok((name.clone(), self.xrecord(item)?)),
                        kind => Err(Box::new(
                            Diagnostic::new(codes::DXF_EXTENSION_DICTIONARY_DROPPED)
                                .with("key", text(name.as_bytes()))
                                .with("kind", text(kind)),
                        )),
                    });
            match named {
                Ok((name, fields)) => out.extensions.push(Extension {
                    namespace: XRECORD,
                    name,
                    version: self.version.clone(),
                    schema: XRECORD_SCHEMA,
                    target,
                    basis: basis(),
                    fields,
                    source: source(object),
                }),
                Err(diagnostic) => cx.report((*diagnostic).at(source(object))),
            }
        }
    }

    /// INSERT の切り抜き（XCLIP、決定 0149）。拡張辞書の `ACAD_FILTER` の辞書の `SPATIAL` が指す SPATIAL_FILTER の、
    /// 境界の点（10・20）を、境界の変換と挿入の逆の変換（40 の 12 ずつ。3 行 4 列を行の順）で、記号の定義の空間に
    /// 写す（推定。試料の図面で、挿入の逆の変換が挿入点と尺度の逆になることを確かめた）。境界の向き（210〜230）が
    /// Z でないもの、境界の原点（11〜31）が 0 でないもの、点が 2 つより少ないものは読めないとする。前と後ろで切るか
    /// （72・73）は平面の形を変えず、境界を表示するか（71）は移さない
    fn symbol_clip(&self, db: &AcadDb, object: &Object) -> Lifted<Option<Clip>> {
        let Some(filter) = xdictionary(object)
            .and_then(|handle| entry(db, handle, b"ACAD_FILTER"))
            .and_then(|handle| entry(db, handle, b"SPATIAL"))
            .and_then(|handle| db.get(handle))
            .filter(|filter| filter.kind == b"SPATIAL_FILTER")
        else {
            return Ok(None);
        };
        let dropped = |code: i16| {
            Box::new(Diagnostic::new(codes::DXF_CLIP_DROPPED).with("code", i64::from(code)))
        };
        let start = filter
            .values
            .iter()
            .position(|value| value.code == 70)
            .ok_or_else(|| dropped(70))?;
        let mut values = filter
            .values
            .get(start..)
            .unwrap_or_default()
            .iter()
            .peekable();
        let mut take = |code: i16| -> Lifted<&Data> {
            values
                .next_if(|value| value.code == code)
                .map(|value| &value.data)
                .ok_or_else(|| dropped(code))
        };
        let int = |data: &Data, code: i16| match data {
            Data::Int(value) => Ok(*value),
            _ => Err(dropped(code)),
        };
        let real = |data: &Data, code: i16| match data {
            Data::Real(value) if value.is_finite() => Ok(*value),
            _ => Err(dropped(code)),
        };
        let count = int(take(70)?, 70)?;
        if count < 2 {
            return Err(dropped(70));
        }
        let mut points = Vec::new();
        for _ in 0..count {
            points.push([real(take(10)?, 10)?, real(take(20)?, 20)?]);
        }
        let normal = [
            real(take(210)?, 210)?,
            real(take(220)?, 220)?,
            real(take(230)?, 230)?,
        ];
        if !matches!(normal, [x, y, z] if x.abs() < 1e-9 && y.abs() < 1e-9 && z > 0.0) {
            return Err(dropped(210));
        }
        for code in [11, 21, 31] {
            if real(take(code)?, code)? != 0.0 {
                return Err(dropped(code));
            }
        }
        int(take(71)?, 71)?;
        for (flag, distance) in [(72, 40), (73, 41)] {
            if int(take(flag)?, flag)? != 0 {
                real(take(distance)?, distance)?;
            }
        }
        let mut matrix = || -> Lifted<[[f64; 4]; 3]> {
            let mut rows = [[0.0; 4]; 3];
            for slot in rows.iter_mut().flatten() {
                *slot = real(take(40)?, 40)?;
            }
            Ok(rows)
        };
        let inverse = matrix()?;
        let bound = matrix()?;
        let apply = |m: &[[f64; 4]; 3], [x, y, z]: [f64; 3]| {
            m.map(|[a, b, c, d]| a * x + b * y + c * z + d)
        };
        // 2 点の長方形も、回転やせん断の後は軸に平行とは限らないので、四隅を写す。
        let rectangular = points.len() == 2;
        if let [[x0, y0], [x1, y1]] = points.as_slice() {
            points = vec![[*x0, *y0], [*x1, *y0], [*x1, *y1], [*x0, *y1]];
        }
        let mut vertices = points
            .into_iter()
            .map(|[x, y]| {
                let [x, y, z] = apply(&inverse, apply(&bound, [x, y, 0.0]));
                if ![x, y, z].iter().all(|value| value.is_finite()) || z != 0.0 {
                    return Err(dropped(40));
                }
                Ok([x, y])
            })
            .collect::<Lifted<Vec<_>>>()?;
        // 軸に平行な長方形なら、元と同じく向かい合う 2 点で持つ。
        if rectangular {
            let [a, b, c, d] = vertices.as_slice() else {
                return Err(dropped(70));
            };
            if (a[1] == b[1] && b[0] == c[0] && c[1] == d[1] && d[0] == a[0])
                || (a[0] == b[0] && b[1] == c[1] && c[0] == d[0] && d[1] == a[1])
            {
                vertices = vec![*a, *c];
            }
        }
        Ok(Some(Clip {
            vertices,
            inverted: false,
        }))
    }

    /// XRECORD のデータの値。所有者などの共通の値と、複製の扱い（280）の後に並ぶ
    fn xrecord(&self, item: &Object) -> Lifted<Vec<Field>> {
        let mut depth = 0usize;
        let mut values = item.values.iter().skip_while(|value| {
            match (value.code, &value.data) {
                (102, Data::Text(text)) if text.starts_with(b"{") => depth += 1,
                (102, Data::Text(text)) if text.starts_with(b"}") => {
                    depth = depth.saturating_sub(1);
                    return true;
                }
                _ => {}
            }
            depth > 0 || matches!(value.code, 5 | 100 | 330)
        });
        let mut fields = Vec::new();
        let mut first = true;
        for Value { code, data } in values.by_ref() {
            if core::mem::take(&mut first) && *code == 280 {
                continue;
            }
            let value = match data {
                Data::Text(_) => FieldValue::Text(self.decode(&item.kind, *code, data)?),
                Data::Int(value) => FieldValue::Integer(*value),
                Data::Real(value) => FieldValue::Real(*value),
                Data::Bool(value) => FieldValue::Integer(i64::from(*value)),
                Data::Handle(handle) => FieldValue::Handle(*handle),
                Data::Binary(bytes) => FieldValue::Bytes(bytes.clone()),
            };
            let key = match value {
                FieldValue::Text(_) => "text",
                FieldValue::Integer(_) => "integer",
                FieldValue::Real(_) => "real",
                FieldValue::Handle(_) => "handle",
                _ => "binary",
            };
            fields.push(Field {
                key: "code",
                value: FieldValue::Integer(i64::from(*code)),
            });
            fields.push(Field { key, value });
        }
        Ok(fields)
    }

    /// 1 つの登録アプリケーション名の XDATA。点（1010〜1013）は、続く Y（+10）と Z（+20）と合わせる
    fn xdata(&self, kind: &[u8], name: &Value, values: &[Value]) -> Lifted<(String, Vec<Field>)> {
        let name = self.decode(kind, 1001, &name.data)?;
        let mut fields = Vec::new();
        let mut values = values.iter();
        while let Some(Value { code, data }) = values.next() {
            let code = *code;
            let key = match code {
                1000 => "string",
                1002 => "control",
                1003 => "layer",
                1004 => "binary",
                1005 => "handle",
                1010 => "point",
                1011 => "position",
                1012 => "displacement",
                1013 => "direction",
                1040 => "real",
                1041 => "distance",
                1042 => "scale",
                1070 => "integer",
                1071 => "long",
                _ => return Err(invalid(kind, code)),
            };
            let value = match (code, data) {
                (1002, data) => match self.decode(kind, code, data)?.as_str() {
                    text @ ("{" | "}") => FieldValue::Text(text.to_owned()),
                    _ => return Err(invalid(kind, code)),
                },
                (1000 | 1003, data) => FieldValue::Text(self.decode(kind, code, data)?),
                (1004, Data::Binary(bytes)) => FieldValue::Bytes(bytes.clone()),
                (1005, Data::Handle(handle)) => FieldValue::Handle(*handle),
                (1010..=1013, Data::Real(x)) => {
                    let mut axis = |offset: i16| match values.next() {
                        Some(Value {
                            code: next,
                            data: Data::Real(value),
                        }) if *next == code + offset => Ok(*value),
                        _ => Err(invalid(kind, code + offset)),
                    };
                    FieldValue::Point([*x, axis(10)?, axis(20)?])
                }
                (1040..=1042, Data::Real(value)) => FieldValue::Real(*value),
                (1070 | 1071, Data::Int(value)) => FieldValue::Integer(*value),
                _ => return Err(invalid(kind, code)),
            };
            fields.push(Field { key, value });
        }
        Ok((name, fields))
    }

    /// BLOCKS の節の BLOCK を、記号の定義にする（決定 0112）。中身は、所有者（330）をたどって着くブロックの
    /// レコードの BLOCK のもの、所有者が分からなければ、並びの中で前の BLOCK から ENDBLK までのものとする。
    /// 名前は BLOCK の 2、なければブロックのレコードの 2。基点は 10、印（70）の 1 は名前のない定義、4 は外部参照
    /// で、そのパス（1）を持つ。名前が `*` で始まるものも名前のない定義とする（推定）。
    /// 空間のブロック（`*Model_Space`・`*Paper_Space`）は移さない。中身の描く順は、ブロックのレコードを指す
    /// SORTENTSTABLE（`keys`。ブロックのレコードのハンドルごと）で決め、なければ入力の順とする
    fn definitions(
        &self,
        db: &AcadDb,
        keys: &BTreeMap<u64, SortKeys>,
        out: &mut Out,
        cx: &mut Cx,
    ) -> Result<Vec<SymbolDefinition>, Error> {
        let objects: Vec<&Object> = db
            .objects()
            .iter()
            .filter(|object| object.section == Section::Blocks)
            .collect();
        let starts: Vec<usize> = objects
            .iter()
            .enumerate()
            .filter(|(_, object)| object.kind == b"BLOCK")
            .map(|(index, _)| index)
            .collect();
        let by_record: BTreeMap<u64, usize> = starts
            .iter()
            .enumerate()
            .filter_map(|(at, index)| Some((objects.get(*index)?.owner()?, at)))
            .collect();
        let owners: BTreeMap<u64, Option<u64>> = objects
            .iter()
            .filter_map(|object| Some((object.handle()?, object.owner())))
            .collect();
        let mut members: Vec<Vec<&Object>> = vec![Vec::new(); starts.len()];
        let mut open = None;
        for object in &objects {
            work(cx, 1, &source(object))?;
            match object.kind.as_slice() {
                b"BLOCK" => {
                    open = starts.iter().position(|index| {
                        objects
                            .get(*index)
                            .is_some_and(|o| std::ptr::eq(*o, *object))
                    });
                    continue;
                }
                b"ENDBLK" => {
                    open = None;
                    continue;
                }
                _ => {}
            }
            // 所有者が図形（ATTRIB の INSERT など）なら、その所有者へ。輪は数えて止める
            let mut owner = object.owner();
            let mut at = None;
            for _ in 0..=owners.len() {
                let Some(handle) = owner else { break };
                if let Some(found) = by_record.get(&handle) {
                    at = Some(*found);
                    break;
                }
                owner = owners.get(&handle).copied().flatten();
            }
            match at.or(open).and_then(|at| members.get_mut(at)) {
                Some(group) => group.push(object),
                None => cx.report((*invalid(&object.kind, 330)).at(source(object))),
            }
        }
        let mut symbols = Vec::new();
        for (at, (index, members)) in starts.iter().zip(members).enumerate() {
            let Some(block) = objects.get(*index).copied() else {
                continue;
            };
            let space = SpaceId(u32::try_from(at).unwrap_or(u32::MAX).saturating_add(1));
            if let Some(symbol) = self.definition(db, block, space, &members, keys, out, cx)? {
                symbols.push(symbol);
            }
        }
        Ok(symbols)
    }

    /// LAYOUT の名前・タブの順・範囲。読めない値は、ないものとする
    fn layout_info(&self, layout: &Object) -> LayoutInfo {
        let name = layout
            .values
            .iter()
            .rfind(|value| value.code == 1)
            .and_then(|value| self.decode(&layout.kind, 1, &value.data).ok());
        let real = |code| self.real(layout, code, None).ok();
        let limits = (|| Some([[real(10)?, real(20)?], [real(11)?, real(21)?]]))();
        LayoutInfo {
            name,
            tab: self.int(layout, 71, 0).unwrap_or(0),
            limits,
        }
    }

    /// ほかのレイアウトのペーパー空間のブロックの中身を、1 枚の用紙にする。用紙の原点と大きさは LAYOUT の
    /// 範囲で決め、なければ原点を 0 とし、大きさを決めない（推定。docs/formats/dxf.md）。中身がなければ
    /// 用紙を作らない
    fn layout(
        &self,
        db: &AcadDb,
        record: Option<&Object>,
        members: &[&Object],
        keys: &BTreeMap<u64, SortKeys>,
        out: &mut Out,
        cx: &mut Cx,
    ) -> Result<(), Error> {
        let handle = record.and_then(Object::handle);
        let info = handle
            .and_then(|handle| layout_of(db, handle))
            .map(|layout| self.layout_info(layout));
        let limits = info.as_ref().and_then(|info| info.limits);
        let id = SheetId(
            u32::try_from(out.layouts.len())
                .unwrap_or(u32::MAX)
                .saturating_add(1),
        );
        let lifter = Lifter {
            sheet: id,
            origin: limits.map_or([0.0, 0.0], |[min, _]| min),
            ..self.clone()
        };
        let paper = std::mem::take(&mut out.paper);
        let views = std::mem::take(&mut out.views);
        let sheet = std::mem::take(&mut out.sheet);
        let (attached, owned) = attached(members);
        for (index, object) in members.iter().enumerate() {
            let source = source(object);
            work(cx, amount(object), &source)?;
            if owned.contains(&index) {
                continue;
            }
            let inserted: Vec<&Object> = attached
                .get(&index)
                .map_or(&[][..], Vec::as_slice)
                .iter()
                .filter_map(|at| members.get(*at).copied())
                .collect();
            if let Err(diagnostic) =
                lifter.place(db, object, source.clone(), &inserted, true, out, cx)
            {
                cx.report((*diagnostic).at(source));
            }
        }
        let elements = std::mem::replace(&mut out.paper, paper);
        let views = std::mem::replace(&mut out.views, views);
        let items = std::mem::replace(&mut out.sheet, sheet);
        if elements.is_empty() && views.is_empty() {
            return Ok(());
        }
        let size = match (limits, self.mm) {
            (Some([[x0, y0], [x1, y1]]), Some(mm)) => {
                let size = [(x1 - x0) * mm, (y1 - y0) * mm];
                size.iter().all(|s| *s > 0.0).then(|| size.map(PaperLength))
            }
            _ => None,
        };
        let order = arrange(handle.and_then(|handle| keys.get(&handle)), items, cx);
        let (name, tab) = info.map_or((None, 0), |info| (info.name, info.tab));
        out.layouts.push((
            tab,
            Sheet {
                id,
                name,
                size,
                elements,
                views,
                order,
            },
        ));
        Ok(())
    }

    /// BLOCK と中身の図形を、空間 `space` の記号の定義にする。空間のブロックなら `None`。中身の図形のうち
    /// 移せないものは、その図形を指す診断を返し、定義は移す
    #[allow(clippy::too_many_arguments)]
    fn definition(
        &self,
        db: &AcadDb,
        block: &Object,
        space: SpaceId,
        members: &[&Object],
        keys: &BTreeMap<u64, SortKeys>,
        out: &mut Out,
        cx: &mut Cx,
    ) -> Result<Option<SymbolDefinition>, Error> {
        let kind = block.kind.as_slice();
        let record = block
            .owner()
            .and_then(|owner| db.get(owner))
            .filter(|record| record.kind == b"BLOCK_RECORD");
        let name = match self.string(block, 2) {
            Ok(Some(name)) => Some(name),
            Ok(None) => record.and_then(|record| self.string(record, 2).ok().flatten()),
            Err(diagnostic) => {
                cx.report((*diagnostic).at(source(block)));
                return Ok(None);
            }
        };
        let Some(name) = name else {
            cx.report((*invalid(kind, 2)).at(source(block)));
            return Ok(None);
        };
        if layout_block(&name) {
            if paper_layout(&name) {
                self.layout(db, record, members, keys, out, cx)?;
            }
            return Ok(None);
        }
        let point = |x: f64, y: f64| ModelPoint { space, x, y };
        let length = |value: f64| Length::Model(ModelLength { space, value });
        let head = (|| -> Lifted<_> {
            let base = match block.first(10) {
                Some(_) => Some(self.planar_point(block, 10, &point)?),
                None => None,
            };
            let flags = self.int(block, 70, 0)?;
            let external = if flags & 4 != 0 {
                Some(self.string(block, 1)?.unwrap_or_default())
            } else {
                None
            };
            Ok((base, flags & 1 != 0 || name.starts_with('*'), external))
        })();
        let (base, anonymous, external) = match head {
            Ok(head) => head,
            Err(diagnostic) => {
                cx.report((*diagnostic).at(source(block)));
                return Ok(None);
            }
        };
        let (attached, owned) = attached(members);
        let mut elements = Vec::new();
        let mut handles = Vec::new();
        let mut attributes = Vec::new();
        for (index, object) in members.iter().enumerate() {
            let source = source(object);
            work(cx, amount(object), &source)?;
            if owned.contains(&index) {
                continue;
            }
            extension_work(db, object, cx)?;
            let lifted = match object.kind.as_slice() {
                b"ATTDEF" => self
                    .attribute_definition(object, &point, &length, out)
                    .map(|definition| attributes.push(definition)),
                b"LINE" | b"CIRCLE" | b"ARC" | b"ELLIPSE" | b"LWPOLYLINE" | b"SPLINE" | b"TEXT"
                | b"MTEXT" | b"DIMENSION" | b"LEADER" | b"MULTILEADER" | b"INSERT" | b"HATCH"
                | b"ACAD_TABLE" | b"IMAGE" | b"PDFUNDERLAY" | b"DWFUNDERLAY" | b"DGNUNDERLAY"
                | b"POLYLINE" | b"SOLID" | b"TRACE" | b"POINT" => {
                    let inserted: Vec<&Object> = attached
                        .get(&index)
                        .map_or(&[][..], Vec::as_slice)
                        .iter()
                        .filter_map(|at| members.get(*at).copied())
                        .collect();
                    self.element(
                        db,
                        object,
                        source.clone(),
                        &inserted,
                        point,
                        length,
                        out,
                        cx,
                    )
                    .map(|element| {
                        let target = Target::Model {
                            space,
                            index: elements.len(),
                        };
                        self.extend(
                            db,
                            object,
                            target,
                            || Basis::Model(element.clone()),
                            out,
                            cx,
                        );
                        elements.push(element);
                        handles.push(object.handle());
                    })
                }
                kind => Err(Box::new(
                    Diagnostic::new(codes::DXF_LIFT_DROPPED).with("kind", text(kind)),
                )),
            };
            if let Err(diagnostic) = lifted {
                cx.report((*diagnostic).at(source));
            }
        }
        let keys = record
            .and_then(Object::handle)
            .and_then(|handle| keys.get(&handle));
        let order = arrange(keys, handles.into_iter().enumerate().collect(), cx);
        // ブロックのレコードの挿す単位。0（単位なし）か、なければ文書の単位とする。配置の尺度は、
        // 単位の違いを掛けた値を持つ（推定。決定 0144）
        let unit = match record.and_then(insertion_units) {
            Some(code) if code != 0 => unit_of(code),
            _ => self.unit,
        };
        Ok(Some(SymbolDefinition {
            key: SymbolKey {
                namespace: BLOCK,
                name,
            },
            space: ModelSpace {
                id: space,
                unit,
                order,
                elements,
            },
            base,
            attributes,
            anonymous,
            external,
            source: source(block),
        }))
    }

    /// ACAD_TABLE（DXF Reference「TABLE」、決定 0113）。AcDbTable（100）より前の挿入点（10）と、表を描いた
    /// ブロック（2）を読む。後の値は、行と列の数（91・92）・横の向き（11・21・31）・表のスタイル（342）・行の高さ
    /// （141）・列の幅（142）と、種類（171）で始まるセルの並び。セルは、覆われる（173）・広げる列と行の数（175・
    /// 176）を持ち、種類が 1 なら文字（2 の区切りと 1）、2 ならブロックのレコード（340）を内容とする。文字がなく、
    /// 2007 からの値（301 から）の文字（302）があれば、それを内容とする（docs/formats/dxf.md）。数が合わない
    /// ものは読めないとする。セルの値の型と単位と表示の書式は `cell_value` で読む。セルの書式と枠は、まだ移さない
    fn table<P>(
        &self,
        db: &AcadDb,
        object: &Object,
        point: &impl Fn(f64, f64) -> P,
        length: &impl Fn(f64) -> Length,
    ) -> Lifted<Table<P>> {
        let kind = object.kind.as_slice();
        // DWG は AcDbTable（100）を持たないので、表の欄の初め（342・11・90）で分ける。表を描いたブロック（2）は、
        // DWG では値の終わりにある（docs/formats/dwg.md）
        let dwg = matches!(object.origin, Origin::Dwg { .. });
        let marker = object
            .values
            .iter()
            .position(|value| match (value.code, &value.data) {
                (342 | 11 | 90, _) => dwg,
                (100, Data::Text(text)) => !dwg && text == b"AcDbTable",
                _ => false,
            })
            .ok_or_else(|| invalid(kind, 100))?;
        let (reference, rest) = object
            .values
            .split_at_checked(marker)
            .ok_or_else(|| invalid(kind, 100))?;
        let part = |values: &[Value]| Object {
            values: values.to_vec(),
            ..object.clone()
        };
        let mut reference = part(reference);
        let mut rest = rest.to_vec();
        if dwg && let Some(at) = rest.iter().rposition(|value| value.code == 2) {
            reference.values.push(rest.remove(at));
        }
        let rest = rest.as_slice();
        let first = rest
            .iter()
            .position(|value| value.code == 171)
            .unwrap_or(rest.len());
        let (head, body) = rest.split_at_checked(first).unwrap_or((rest, &[]));
        let head = part(head);
        let count = |code: i16| -> Lifted<usize> {
            usize::try_from(self.int(&head, code, 0)?)
                .ok()
                .filter(|count| *count > 0)
                .ok_or_else(|| invalid(kind, code))
        };
        let [rows, columns] = [count(91)?, count(92)?];
        if self.real(&head, 31, Some(0.0))? != 0.0 {
            return Err(non_planar(kind));
        }
        let sizes = |code: i16, expected: usize| -> Lifted<Vec<Length>> {
            let sizes: Vec<Length> = head
                .values
                .iter()
                .filter(|value| value.code == code)
                .map(|value| match value.data {
                    Data::Real(size) if size.is_finite() && size > 0.0 => Ok(length(size)),
                    _ => Err(invalid(kind, code)),
                })
                .collect::<Lifted<_>>()?;
            if sizes.len() == expected {
                Ok(sizes)
            } else {
                Err(invalid(kind, code))
            }
        };
        let row_heights = sizes(141, rows)?;
        let column_widths = sizes(142, columns)?;
        let mut cells = Vec::new();
        for chunk in body.chunk_by(|_, next| next.code != 171) {
            cells.push(self.cell(db, &part(chunk), length)?);
        }
        if Some(cells.len()) != rows.checked_mul(columns) {
            return Err(invalid(kind, 171));
        }
        let style = match head.first(342) {
            None | Some(Data::Handle(0)) => None,
            Some(Data::Handle(handle)) => {
                Some(source(db.get(*handle).ok_or_else(|| invalid(kind, 342))?))
            }
            Some(_) => return Err(invalid(kind, 342)),
        };
        let formats = match head.first(342) {
            Some(Data::Handle(handle)) => db
                .get(*handle)
                .filter(|style| style.kind == b"TABLESTYLE")
                .map(|style| self.table_formats(db, style, length))
                .transpose()?
                .flatten()
                .map(Box::new),
            _ => None,
        };
        Ok(Table {
            position: self.planar_point(&reference, 10, point)?,
            direction: [
                self.real(&head, 11, Some(1.0))?,
                self.real(&head, 21, Some(0.0))?,
            ],
            row_heights,
            column_widths,
            cells,
            style,
            formats,
            drawn: self.string(&reference, 2)?.map(|name| SymbolKey {
                namespace: BLOCK,
                name,
            }),
        })
    }

    /// 下敷きが指す定義（340）の、参照名（1）。定義が `definition` の種類でなければ読めないとする。
    /// 定義と出所を返す
    fn resource<'a>(
        &self,
        db: &'a AcadDb,
        object: &Object,
        definition: &[u8],
    ) -> Lifted<(&'a Object, Resource)> {
        let kind = object.kind.as_slice();
        let found = match object.first(340) {
            Some(Data::Handle(handle)) => db.get(*handle),
            _ => None,
        }
        .filter(|found| found.kind == definition)
        .ok_or_else(|| invalid(kind, 340))?;
        let path = self
            .string(found, 1)?
            .ok_or_else(|| invalid(&found.kind, 1))?;
        Ok((
            found,
            Resource {
                path,
                definition: source(found),
            },
        ))
    }

    /// IMAGE（DXF Reference「IMAGE」「IMAGEDEF」、決定 0114）。挿入点（10）は左下の角、1 画素の横と縦の向きと
    /// 大きさ（11・21・31 と 12・22・32）、画素の数（13・23）から、右下と左上の角を求める。表示の印（70）の 1 は
    /// 表示する。切り抜く（280 が 1）なら、境界（71 が 1 なら長方形、2 なら多角形）の頂点（14・24）を、数（91）と
    /// 合わせて読み、290 が 1 なら内側を隠す。明るさ・コントラスト・薄さ（281〜283）は、まだ移さない
    fn image<P>(
        &self,
        db: &AcadDb,
        object: &Object,
        point: &impl Fn(f64, f64) -> P,
    ) -> Lifted<Underlay<P>> {
        let kind = object.kind.as_slice();
        let (_, resource) = self.resource(db, object, b"IMAGEDEF")?;
        if self.real(object, 30, Some(0.0))? != 0.0
            || self.real(object, 31, Some(0.0))? != 0.0
            || self.real(object, 32, Some(0.0))? != 0.0
        {
            return Err(non_planar(kind));
        }
        let [x, y] = [self.real(object, 10, None)?, self.real(object, 20, None)?];
        let u = [self.real(object, 11, None)?, self.real(object, 21, None)?];
        let v = [self.real(object, 12, None)?, self.real(object, 22, None)?];
        let size = [self.real(object, 13, None)?, self.real(object, 23, None)?];
        if size.iter().any(|pixels| *pixels <= 0.0) {
            return Err(invalid(kind, 13));
        }
        let [width, height] = size;
        let clip = if self.int(object, 280, 0)? == 1 {
            let vertices: Vec<[f64; 2]> = self.points(kind, &object.values, 14, &|x, y| [x, y])?;
            let expected = match self.int(object, 71, 1)? {
                1 => 2,
                2 => vertices.len().max(3),
                _ => return Err(invalid(kind, 71)),
            };
            if usize::try_from(self.int(object, 91, -1)?).ok() != Some(vertices.len())
                || vertices.len() != expected
            {
                return Err(invalid(kind, 91));
            }
            Some(Clip {
                vertices,
                inverted: self.flag(object, 290, false)?,
            })
        } else {
            None
        };
        Ok(Underlay {
            kind: UnderlayKind::Image,
            resource,
            page: None,
            frame: UnderlayFrame::Pixels {
                size,
                origin: point(x, y),
                right: point(x + u[0] * width, y + u[1] * width),
                top: point(x + v[0] * height, y + v[1] * height),
            },
            clip,
            visible: self.int(object, 70, 1)? & 1 != 0,
        })
    }

    /// PDFUNDERLAY・DWFUNDERLAY・DGNUNDERLAY（DXF Reference「UNDERLAY」「UNDERLAYDEFINITION」、決定 0114）。
    /// 定義の 2 はページなどの名前。挿入点（10）・尺度（41・42）・回転（50）を持つ。印（280）の 2 は表示する、1 は
    /// 切り抜く、16 は内側を切り抜く（内側を隠す）。切り抜くなら、境界の頂点（11・21）を読む。コントラスト・薄さ・
    /// 単色（281・282 と 280 の 4・8）は、まだ移さない
    fn underlay<P>(
        &self,
        db: &AcadDb,
        object: &Object,
        point: &impl Fn(f64, f64) -> P,
        length: &impl Fn(f64) -> Length,
    ) -> Lifted<Underlay<P>> {
        let kind = object.kind.as_slice();
        let (underlay, definition): (UnderlayKind, &[u8]) = match kind {
            b"PDFUNDERLAY" => (UnderlayKind::Pdf, b"PDFDEFINITION"),
            b"DWFUNDERLAY" => (UnderlayKind::Dwf, b"DWFDEFINITION"),
            _ => (UnderlayKind::Dgn, b"DGNDEFINITION"),
        };
        let (found, resource) = self.resource(db, object, definition)?;
        let scale = |code: i16| -> Lifted<Length> {
            match self.real(object, code, Some(1.0))? {
                scale if scale > 0.0 => Ok(length(scale)),
                _ => Err(invalid(kind, code)),
            }
        };
        let flags = self.int(object, 280, 2)?;
        let clip = if flags & 1 != 0 {
            let vertices: Vec<[f64; 2]> = self.points(kind, &object.values, 11, &|x, y| [x, y])?;
            if vertices.len() < 2 {
                return Err(invalid(kind, 11));
            }
            Some(Clip {
                vertices,
                inverted: flags & 16 != 0,
            })
        } else {
            None
        };
        Ok(Underlay {
            kind: underlay,
            resource,
            page: self.string(found, 2)?,
            frame: UnderlayFrame::Scaled {
                origin: self.planar_point(object, 10, point)?,
                scale: [scale(41)?, scale(42)?],
                rotation: self.real(object, 50, Some(0.0))?,
            },
            clip,
            visible: flags & 2 != 0,
        })
    }

    /// 表のセル 1 つの値。2007 からの値（301〜304）の中のコードは、セルの欄として読まない
    fn cell(&self, db: &AcadDb, cell: &Object, length: &impl Fn(f64) -> Length) -> Lifted<Cell> {
        let kind = cell.kind.as_slice();
        let end = cell
            .values
            .iter()
            .position(|value| value.code == 301)
            .unwrap_or(cell.values.len());
        let (fields, value) = cell
            .values
            .split_at_checked(end)
            .unwrap_or((&cell.values, &[]));
        let fields = Object {
            values: fields.to_vec(),
            ..cell.clone()
        };
        let span = |code: i16| -> Lifted<u32> {
            u32::try_from(self.int(&fields, code, 1)?)
                .ok()
                .filter(|span| *span > 0)
                .ok_or_else(|| invalid(kind, code))
        };
        let content = match self.int(&fields, 171, 1)? {
            1 => {
                let mut content = String::new();
                for value in fields
                    .values
                    .iter()
                    .filter(|value| matches!(value.code, 1 | 2))
                {
                    content.push_str(&self.decode(kind, value.code, &value.data)?);
                }
                if fields.first(1).is_none()
                    && let Some(value) = value.iter().find(|value| value.code == 302)
                {
                    content = self.decode(kind, 302, &value.data)?;
                }
                if content.is_empty() {
                    CellContent::Empty
                } else {
                    let spans = mtext::spans(&content, length);
                    CellContent::Text { content, spans }
                }
            }
            2 => CellContent::Symbol(
                self.block_key(db, &fields, 340)?
                    .ok_or_else(|| invalid(kind, 340))?,
            ),
            _ => return Err(invalid(kind, 171)),
        };
        Ok(Cell {
            content,
            span: [span(175)?, span(176)?],
            covered: self.int(&fields, 173, 0)? != 0,
            value: self.cell_value(cell, value)?,
            rotation: self.real(&fields, 145, Some(0.0))?,
            format: self.cell_format(db, &fields, length, &CELL_BORDERS)?,
        })
    }

    /// 表のスタイル（DXF Reference「TABLESTYLE」）の、行の種類ごとの書式。説明（3）の後の 280 が 1 ならタイトルの
    /// 行を、281 が 1 なら見出しの行を持たない。文字スタイル（7）から始まる組が、データ・タイトル・見出しの順に
    /// 続く（試料の表を描いたブロックの文字の高さと位置合わせで確かめた）。組の枠は、上・内側の横・下・左・
    /// 内側の縦・右の順に、線幅（274〜279）・表示（284〜289）・色（64〜69）を持つ（推定）。組がなければ `None`
    /// （R2010 からの DWG の読み手は、TABLESTYLE の欄をまだ読まない）
    fn table_formats(
        &self,
        db: &AcadDb,
        style: &Object,
        length: &impl Fn(f64) -> Length,
    ) -> Lifted<Option<TableFormats>> {
        let kind = style.kind.as_slice();
        let first = style
            .values
            .iter()
            .position(|value| value.code == 7)
            .unwrap_or(style.values.len());
        let (head, rows) = style.values.split_at_checked(first).unwrap_or_default();
        let after = head
            .iter()
            .position(|value| value.code == 3)
            .map_or(head, |at| head.get(at..).unwrap_or_default());
        let head = Object {
            values: after.to_vec(),
            ..style.clone()
        };
        if rows.is_empty() {
            return Ok(None);
        }
        let mut formats = Vec::new();
        for row in rows.chunk_by(|_, next| next.code != 7) {
            let row = Object {
                values: row.to_vec(),
                ..style.clone()
            };
            formats.push(self.cell_format(db, &row, length, &STYLE_BORDERS)?);
        }
        let [data_format, title_format, header_format] =
            <[CellFormat; 3]>::try_from(formats).map_err(|_| invalid(kind, 7))?;
        Ok(Some(TableFormats {
            title: self.int(&head, 280, 0)? == 0,
            header: self.int(&head, 281, 0)? == 0,
            title_format,
            header_format,
            data_format,
        }))
    }

    /// セルか、表のスタイルの組の書式。文字スタイル（7。名前か STYLE のハンドル）・高さ（140）・位置合わせ
    /// （170。1〜9）・文字の色（62 か、セルでは 64）・背景を塗るか（283）と色（63）・枠（`borders` の欄）を読む。
    /// 色は ACI の符号（0 は配置から、256 はレイヤから継承）、線幅は -1 と -2 を継承、-3 を文書の既定、0〜211 を
    /// 1/100 mm とする
    fn cell_format(
        &self,
        db: &AcadDb,
        object: &Object,
        length: &impl Fn(f64) -> Length,
        borders: &BorderCodes,
    ) -> Lifted<CellFormat> {
        let kind = object.kind.as_slice();
        let color = |code: i16| -> Lifted<Option<Inherit<Color>>> {
            match object.first(code) {
                None => Ok(None),
                Some(Data::Int(0)) => Ok(Some(Inherit::Block)),
                Some(Data::Int(256)) => Ok(Some(Inherit::Layer)),
                Some(Data::Int(index @ 1..=255)) => Ok(Some(Inherit::Own(Color {
                    code: Some(aci(*index)),
                    rgb: None,
                }))),
                Some(_) => Err(invalid(kind, code)),
            }
        };
        let weight = |code: i16| -> Lifted<Option<Inherit<LineWeight>>> {
            match object.first(code) {
                None => Ok(None),
                Some(Data::Int(-1)) => Ok(Some(Inherit::Layer)),
                Some(Data::Int(-2)) => Ok(Some(Inherit::Block)),
                Some(Data::Int(-3)) => Ok(Some(Inherit::Own(LineWeight::Default))),
                Some(Data::Int(width @ 0..=211)) => Ok(Some(Inherit::Own(LineWeight::Width(
                    PaperLength(*width as f64 / 100.0),
                )))),
                Some(_) => Err(invalid(kind, code)),
            }
        };
        let flag = |code: i16| -> Lifted<Option<bool>> {
            match object.first(code) {
                None => Ok(None),
                Some(Data::Int(value)) => Ok(Some(*value != 0)),
                Some(Data::Bool(value)) => Ok(Some(*value)),
                Some(_) => Err(invalid(kind, code)),
            }
        };
        let border = |[color_code, weight_code, visible]: [i16; 3]| -> Lifted<Border> {
            Ok(Border {
                color: color(color_code)?,
                weight: weight(weight_code)?,
                visible: flag(visible)?,
            })
        };
        let text_style = match object.first(7) {
            None => None,
            Some(Data::Handle(handle)) => Some(
                db.get(*handle)
                    .filter(|record| record.kind == b"STYLE")
                    .map(|record| self.string(record, 2))
                    .transpose()?
                    .flatten()
                    .ok_or_else(|| invalid(kind, 7))?,
            ),
            Some(data) => Some(self.decode(kind, 7, data)?),
        };
        let text_height = match object.first(140) {
            None => None,
            Some(Data::Real(height)) if height.is_finite() && *height > 0.0 => {
                Some(length(*height))
            }
            Some(_) => return Err(invalid(kind, 140)),
        };
        let alignment = match object.first(170) {
            None => None,
            Some(Data::Int(value)) => Some(attachment(kind, *value, 170)?),
            Some(_) => return Err(invalid(kind, 170)),
        };
        Ok(CellFormat {
            text_style,
            text_height,
            alignment,
            text_color: color(borders.text_color)?,
            fill: flag(283)?,
            fill_color: color(63)?,
            borders: Borders {
                top: border(borders.top)?,
                right: border(borders.right)?,
                bottom: border(borders.bottom)?,
                left: border(borders.left)?,
                horizontal: borders
                    .horizontal
                    .map(border)
                    .transpose()?
                    .unwrap_or_default(),
                vertical: borders
                    .vertical
                    .map(border)
                    .transpose()?
                    .unwrap_or_default(),
            },
        })
    }

    /// セルの 2007 からの値（301 `CELL_VALUE` から 304 `ACVALUE_END` まで）の、型のある値。印（93）の 1 が立つか、
    /// 型（90）が 0 なら `None`。整数（1）は 91、実数（2）は 140、文字列（4）は 2 の並びと 1、日時（8）は 310 の
    /// SYSTEMTIME（年・月・曜日・日・時・分・秒・ミリ秒の 16 バイト。推定）、点（16・32）は 11・21・31 から読む。
    /// ほかの型（64・128・256・512）は移さない（`element` で診断する）。単位（94）は 0・1・2・4・8・16・32 を
    /// なし・長さ・角度・面積・体積・通貨・百分率とし、書式（300）は空なら `None` とする（docs/formats/dxf.md）
    fn cell_value(&self, cell: &Object, values: &[Value]) -> Lifted<Option<CellValue>> {
        let kind = cell.kind.as_slice();
        let end = values
            .iter()
            .position(|value| value.code == 304)
            .unwrap_or(values.len());
        let object = Object {
            values: values.get(..end).unwrap_or_default().to_vec(),
            ..cell.clone()
        };
        if object.values.is_empty() || self.int(&object, 93, 0)? & 1 != 0 {
            return Ok(None);
        }
        let data = match self.int(&object, 90, 0)? {
            0 | 64 | 128 | 256 | 512 => return Ok(None),
            1 => match object.first(91) {
                Some(Data::Int(value)) => CellData::Integer(*value),
                _ => return Err(invalid(kind, 91)),
            },
            2 => CellData::Real(self.real(&object, 140, None)?),
            4 => {
                let mut text = String::new();
                for value in object
                    .values
                    .iter()
                    .filter(|value| matches!(value.code, 1 | 2))
                {
                    text.push_str(&self.decode(kind, value.code, &value.data)?);
                }
                CellData::Text(text)
            }
            8 => match object.first(310) {
                Some(Data::Binary(bytes)) => {
                    CellData::Date(system_time(bytes).ok_or_else(|| invalid(kind, 310))?)
                }
                _ => return Err(invalid(kind, 310)),
            },
            16 | 32 => CellData::Point([
                self.real(&object, 11, None)?,
                self.real(&object, 21, None)?,
                self.real(&object, 31, Some(0.0))?,
            ]),
            _ => return Err(invalid(kind, 90)),
        };
        let unit = match self.int(&object, 94, 0)? {
            0 => ValueUnit::None,
            1 => ValueUnit::Distance,
            2 => ValueUnit::Angle,
            4 => ValueUnit::Area,
            8 => ValueUnit::Volume,
            16 => ValueUnit::Currency,
            32 => ValueUnit::Percentage,
            _ => return Err(invalid(kind, 94)),
        };
        Ok(Some(CellValue {
            data,
            unit,
            format: self
                .string(&object, 300)?
                .filter(|format| !format.is_empty()),
        }))
    }

    /// ATTDEF（DXF Reference「ATTDEF」）。ATTRIB と同じ欄に、値を尋ねる文（3）を持つ。印（70）の 1 は表示しない、
    /// 2 は値が決まっている、4 は確かめる、8 は尋ねない
    fn attribute_definition(
        &self,
        object: &Object,
        point: &impl Fn(f64, f64) -> ModelPoint,
        length: &impl Fn(f64) -> Length,
        out: &mut Out,
    ) -> Lifted<AttributeDefinition> {
        let attribute = self.attribute(object, point, length, out)?;
        let flags = self.int(object, 70, 0)?;
        Ok(AttributeDefinition {
            attribute,
            prompt: self.string(object, 3)?.unwrap_or_default(),
            constant: flags & 2 != 0,
            verify: flags & 4 != 0,
            preset: flags & 8 != 0,
        })
    }

    /// 図形を要素にする。INSERT の属性のうち移せないものは、属性の出所を指す診断を返し、INSERT は移す
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

    /// SOLID と TRACE（DXF Reference「SOLID」「TRACE」）。4 つの角（10〜13）を 1・2・4・3 の順に結んだ輪を
    /// 塗る。3 つ目と 4 つ目の角が同じなら三角形とする。厚さ（39）は、まだ移さない（決定 0140）
    fn solid<P>(&self, object: &Object, point: &impl Fn(f64, f64) -> P) -> Lifted<Hatch<P>> {
        let corner = |x: i16| self.planar_point(object, x, &|x, y| [x, y]);
        let [first, second, third] = [corner(10)?, corner(11)?, corner(12)?];
        let fourth = if object.first(13).is_some() {
            corner(13)?
        } else {
            third
        };
        let corners = if fourth == third {
            vec![first, second, third]
        } else {
            vec![first, second, fourth, third]
        };
        Ok(Hatch {
            loops: vec![HatchLoop {
                boundary: Boundary::Polyline {
                    vertices: corners
                        .into_iter()
                        .map(|[x, y]| Vertex {
                            point: point(x, y),
                            bulge: 0.0,
                        })
                        .collect(),
                    closed: true,
                },
                external: true,
                outermost: true,
                derived: false,
                text_box: false,
                sources: Vec::new(),
            }],
            islands: Islands::Nested,
            fill: Fill::Solid,
            associative: false,
            seeds: Vec::new(),
        })
    }

    /// POINT（DXF Reference「POINT」）。名前空間 `dxf.point` の記号として置き、点の表し方（`$PDMODE`）が 0 なら
    /// `point` を、ほかはその番号を名前にする。角度（50）は X 軸の角度（度）で、記号の回転にする。点の大きさ
    /// （`$PDSIZE`）は、まだ移さない（決定 0140）
    fn point<P>(&self, object: &Object, point: &impl Fn(f64, f64) -> P) -> Lifted<Symbol<P>> {
        Ok(Symbol {
            key: SymbolKey {
                namespace: POINT,
                name: match self.point_mode {
                    0 => "point".to_owned(),
                    mode => mode.to_string(),
                },
            },
            position: self.planar_point(object, 10, point)?,
            scale: [1.0; 2],
            rotation: self.real(object, 50, Some(0.0))?,
            grid: None,
            attributes: Vec::new(),
        })
    }

    /// LINE（DXF Reference「LINE」）。Z のないものは 0 とみなす（推定。docs/formats/dxf.md）
    fn line<P>(&self, object: &Object, point: &impl Fn(f64, f64) -> P) -> Lifted<Shape<P>> {
        let end = |x: i16| -> Lifted<[f64; 3]> {
            let z = self.real(object, x + 20, Some(0.0))?;
            Ok([
                self.real(object, x, None)?,
                self.real(object, x + 10, None)?,
                z,
            ])
        };
        let (start, end) = (end(10)?, end(11)?);
        if start[2] != 0.0 || end[2] != 0.0 {
            return Err(non_planar(&object.kind));
        }
        Ok(Shape::Line {
            start: point(start[0], start[1]),
            end: point(end[0], end[1]),
        })
    }

    /// TEXT と ATTRIB の文字。縦の位置合わせは、TEXT では 73、ATTRIB では 74 にある。`key` は高さの欄
    fn text<P: Copy>(
        &self,
        object: &Object,
        point: &impl Fn(f64, f64) -> P,
        length: &impl Fn(f64) -> Length,
        vertical: i16,
        key: &'static str,
    ) -> Lifted<Text<P>> {
        let kind = object.kind.as_slice();
        let vertical_code = vertical;
        let at = |x: i16| -> Lifted<P> {
            if self.real(object, x + 20, Some(0.0))? != 0.0 {
                return Err(non_planar(kind));
            }
            Ok(point(
                self.real(object, x, None)?,
                self.real(object, x + 10, None)?,
            ))
        };
        let first = at(10)?;
        let horizontal = self.int(object, 72, 0)?;
        let vertical = self.int(object, vertical_code, 0)?;
        let placement = match (horizontal, vertical) {
            (0, 0) => Placement::Anchor {
                point: first,
                horizontal: Horizontal::Left,
                vertical: Vertical::Baseline,
            },
            (3, _) | (5, _) => Placement::Between {
                first,
                second: at(11)?,
                fit: if horizontal == 3 {
                    Fit::Aligned
                } else {
                    Fit::Fit
                },
            },
            _ => Placement::Anchor {
                point: at(11)?,
                horizontal: match horizontal {
                    0 => Horizontal::Left,
                    1 => Horizontal::Center,
                    2 => Horizontal::Right,
                    4 => Horizontal::Middle,
                    _ => return Err(invalid(kind, 72)),
                },
                vertical: match vertical {
                    0 => Vertical::Baseline,
                    1 => Vertical::Bottom,
                    2 => Vertical::Middle,
                    3 => Vertical::Top,
                    _ => return Err(invalid(kind, vertical_code)),
                },
            },
        };
        let flags = self.int(object, 71, 0)?;
        Ok(Text {
            content: self.string(object, 1)?.ok_or_else(|| invalid(kind, 1))?,
            height: Height {
                value: length(self.real(object, 40, None)?),
                meaning: HeightMeaning::Declared(key),
            },
            placement,
            rotation: self.real(object, 50, Some(0.0))?,
            width_factor: self.real(object, 41, Some(1.0))?,
            oblique: self.real(object, 51, Some(0.0))?,
            backward: flags & 2 != 0,
            upside_down: flags & 4 != 0,
            style: self
                .string(object, 7)?
                .unwrap_or_else(|| "STANDARD".to_owned()),
        })
    }

    /// MTEXT（DXF Reference「MTEXT」）。内容は、250 バイトずつの 3 の後に、残りの 1 を続けたもの。向きは X 軸の
    /// 向き（11・21）があればそれを、なければ回転（50、ラジアン）を移す。枠の幅（41）と高さ（46）は 0 なら
    /// 持たない。文字の範囲の大きさ（42・43）は内容から決まる値なので移さない。段組み（75）は、まだ移さない
    fn formatted<P: Copy>(
        &self,
        object: &Object,
        point: &impl Fn(f64, f64) -> P,
        length: &impl Fn(f64) -> Length,
    ) -> Lifted<FormattedText<P>> {
        let kind = object.kind.as_slice();
        if self.int(object, 75, 0)? != 0 {
            return Err(columns());
        }
        let mut content = String::new();
        let mut ended = false;
        for value in object.values.iter().filter(|v| matches!(v.code, 1 | 3)) {
            if ended {
                return Err(invalid(kind, value.code));
            }
            content.push_str(&self.decode(kind, value.code, &value.data)?);
            ended = value.code == 1;
        }
        if !ended {
            return Err(invalid(kind, 1));
        }
        let (horizontal, vertical) = attachment(kind, self.int(object, 71, 1)?, 71)?;
        let rotation = if object.first(11).is_some() {
            if self.real(object, 31, Some(0.0))? != 0.0 {
                return Err(non_planar(kind));
            }
            let direction = [
                self.real(object, 11, None)?,
                self.real(object, 21, Some(0.0))?,
            ];
            if direction == [0.0, 0.0] {
                return Err(invalid(kind, 11));
            }
            Rotation::Direction(direction)
        } else {
            Rotation::Angle(self.real(object, 50, Some(0.0))?.to_degrees())
        };
        let frame = |code: i16| -> Lifted<Option<Length>> {
            match self.real(object, code, Some(0.0))? {
                0.0 => Ok(None),
                size if size > 0.0 => Ok(Some(length(size))),
                _ => Err(invalid(kind, code)),
            }
        };
        let flow = flow(kind, self.int(object, 72, 1)?, 72)?;
        let exact = match self.int(object, 73, 1)? {
            1 => false,
            2 => true,
            _ => return Err(invalid(kind, 73)),
        };
        let factor = self.real(object, 44, Some(1.0))?;
        if factor <= 0.0 {
            return Err(invalid(kind, 44));
        }
        Ok(FormattedText {
            spans: mtext::spans(&content, length),
            content,
            height: Height {
                value: length(self.real(object, 40, None)?),
                meaning: HeightMeaning::Declared("dxf.MTEXT.40"),
            },
            point: self.planar_point(object, 10, point)?,
            horizontal,
            vertical,
            rotation,
            width: frame(41)?,
            frame_height: frame(46)?,
            flow,
            line_spacing: LineSpacing { factor, exact },
            background: self.background(object)?,
            style: self
                .string(object, 7)?
                .unwrap_or_else(|| "STANDARD".to_owned()),
        })
    }

    /// MTEXT の背景（90）。1 は色（63・421）で、2 は表示の背景の色で塗り、16 は枠の線を描く。倍率（45）の
    /// 既定は 1.5 とする（推定。docs/formats/dxf.md）
    fn background(&self, object: &Object) -> Lifted<Option<Background>> {
        let kind = object.kind.as_slice();
        let flags = self.int(object, 90, 0)?;
        if flags & !(1 | 2 | 16) != 0 {
            return Err(invalid(kind, 90));
        }
        if flags == 0 {
            return Ok(None);
        }
        let fill = if flags & 2 != 0 {
            Some(BackgroundFill::Screen)
        } else if flags & 1 != 0 {
            let code = match self.int(object, 63, 0)? {
                0 => None,
                index @ 1..=255 => Some(aci(index)),
                _ => return Err(invalid(kind, 63)),
            };
            let rgb = self.rgb(object, 421)?;
            if code.is_none() && rgb.is_none() {
                return Err(invalid(kind, 63));
            }
            Some(BackgroundFill::Color(Color { code, rgb }))
        } else {
            None
        };
        let scale = self.real(object, 45, Some(1.5))?;
        if scale <= 0.0 {
            return Err(invalid(kind, 45));
        }
        let transparency = match object.first(441) {
            None => None,
            Some(Data::Int(value)) => Some(u32::try_from(*value).map_err(|_| invalid(kind, 441))?),
            Some(_) => return Err(invalid(kind, 441)),
        };
        Ok(Some(Background {
            fill,
            frame: flags & 16 != 0,
            scale,
            transparency,
        }))
    }

    /// INSERT（DXF Reference「INSERT」）。尺度（41・42）が 0 なら読めないとする。Z の尺度（43）は平面の形を
    /// 変えないので移さない。列と行（70・71）が 1 より多ければ、格子に並べる（MINSERT）。属性は後で足す
    fn symbol<P: Copy>(
        &self,
        object: &Object,
        point: &impl Fn(f64, f64) -> P,
        length: &impl Fn(f64) -> Length,
    ) -> Lifted<Symbol<P>> {
        let kind = object.kind.as_slice();
        let name = self.string(object, 2)?.ok_or_else(|| invalid(kind, 2))?;
        let mut scale = [1.0; 2];
        for (code, slot) in [41, 42].into_iter().zip(&mut scale) {
            *slot = self.real(object, code, Some(1.0))?;
            if *slot == 0.0 {
                return Err(invalid(kind, code));
            }
        }
        let count = |code: i16| -> Lifted<u32> {
            u32::try_from(self.int(object, code, 1)?)
                .ok()
                .filter(|count| *count > 0)
                .ok_or_else(|| invalid(kind, code))
        };
        let (columns, rows) = (count(70)?, count(71)?);
        let grid = if (columns, rows) == (1, 1) {
            None
        } else {
            Some(Grid {
                columns,
                rows,
                column_spacing: length(self.real(object, 44, Some(0.0))?),
                row_spacing: length(self.real(object, 45, Some(0.0))?),
            })
        };
        Ok(Symbol {
            key: SymbolKey {
                namespace: BLOCK,
                name,
            },
            position: self.planar_point(object, 10, point)?,
            scale,
            rotation: self.real(object, 50, Some(0.0))?,
            grid,
            attributes: Vec::new(),
            clip: None,
        })
    }

    /// ATTRIB（DXF Reference「ATTRIB」）。タグ（2）と、TEXT と同じ欄の値を移し、70 の 1 なら表示しないとする。
    /// 複数行の属性は、埋め込みのオブジェクトの印（101）の後の MTEXT の値も移す。印の前の、最後の 71（属性の
    /// 種類）と 72 の組は、TEXT の欄と取り違えないよう、1 行の値から除く（決定 0102）
    fn attribute<P: Copy>(
        &self,
        object: &Object,
        point: &impl Fn(f64, f64) -> P,
        length: &impl Fn(f64) -> Length,
        out: &mut Out,
    ) -> Lifted<Attribute<P>> {
        let kind = object.kind.as_slice();
        self.planar(object)?;
        let embedded = object
            .values
            .iter()
            .position(|v| v.code == 101)
            .and_then(|at| object.values.split_at_checked(at));
        let (single, formatted) = match embedded {
            None => (None, None),
            Some((before, marked)) => {
                // DXF は 71 と 72 の後に、もう一度位置合わせの点（11・21・31）を書く
                let mut values = before.to_vec();
                let kind_at = values
                    .windows(2)
                    .rposition(|pair| matches!(pair, [a, b] if a.code == 71 && b.code == 72));
                if let Some(at) = kind_at {
                    values.drain(at..at + 2);
                }
                let single = Object {
                    values,
                    ..object.clone()
                };
                let mut embedded = Object {
                    values: marked.get(1..).unwrap_or_default().to_vec(),
                    ..object.clone()
                };
                // 文字スタイルは、埋め込みの値になければ属性のものとする
                if embedded.first(7).is_none()
                    && let Some(style) = before.iter().find(|v| v.code == 7)
                {
                    embedded.values.push(style.clone());
                }
                let formatted = self.formatted(&embedded, point, length)?;
                (Some(single), Some(formatted))
            }
        };
        let object = single.as_ref().unwrap_or(object);
        let text = self.text(object, point, length, 74, "dxf.ATTRIB.40")?;
        let tag = self.string(object, 2)?.ok_or_else(|| invalid(kind, 2))?;
        let hidden = self.int(object, 70, 0)? & 1 != 0;
        let style = self.style(object, out)?;
        let layer = self.layer(object, out)?;
        Ok(Attribute {
            tag,
            text,
            formatted,
            hidden,
            layer,
            style,
            source: source(object),
        })
    }

    /// HATCH（DXF Reference「HATCH」「Boundary Path Data」「Pattern Data」）。境界の輪の数（91）からは、値を
    /// 並びの順に読む。標高（30）が 0 でなければ平面に移さない。SPLINE の辺を持つものは、まだ移さない。
    /// 境界を作った図形（330）は、輪ごとの数（97）と合わなければ読めないとする。密度の画素の大きさ（47）は
    /// 表示のための値なので移さない。グラデーション（450 が
    /// 1）は、色の RGB（421）だけを移し、色の番号（63）は移さない（推定。docs/formats/dxf.md）
    fn hatch<P: Copy>(
        &self,
        db: &AcadDb,
        object: &Object,
        point: &impl Fn(f64, f64) -> P,
        length: &impl Fn(f64) -> Length,
    ) -> Lifted<Hatch<P>> {
        // DWG の読み手は、SPLINE の辺の当てはめる点を置かない
        let fit_data = self.fit_data && !matches!(object.origin, Origin::Dwg { .. });
        let kind = object.kind.as_slice();
        if self.real(object, 30, Some(0.0))? != 0.0 {
            return Err(non_planar(kind));
        }
        let solid = match self.int(object, 70, 0)? {
            0 => false,
            1 => true,
            _ => return Err(invalid(kind, 70)),
        };
        let associative = self.int(object, 71, 0)? != 0;
        let start = object
            .values
            .iter()
            .position(|value| value.code == 91)
            .ok_or_else(|| invalid(kind, 91))?;
        let mut values = Values {
            kind,
            values: object.values.get(start..).unwrap_or_default(),
        };
        let mut loops = Vec::new();
        for _ in 0..values.count(91)? {
            let flags = values.int(92)?;
            let boundary = if flags & 2 != 0 {
                let bulges = values.int(72)? != 0;
                let closed = values.int(73)? != 0;
                let mut vertices = Vec::new();
                for _ in 0..values.count(93)? {
                    let at = point(values.real(10)?, values.real(20)?);
                    let bulge = if bulges && values.next() == Some(42) {
                        values.real(42)?
                    } else {
                        0.0
                    };
                    vertices.push(Vertex { point: at, bulge });
                }
                Boundary::Polyline { vertices, closed }
            } else {
                let mut edges = Vec::new();
                for _ in 0..values.count(93)? {
                    edges.push(self.hatch_edge(&mut values, fit_data, point, length)?);
                }
                Boundary::Edges(edges)
            };
            let count = values.count(97)?;
            let mut sources = Vec::new();
            while sources.len() < count && values.next() == Some(330) {
                let Data::Handle(handle) = values.take(330)? else {
                    return Err(invalid(kind, 330));
                };
                let found = db.get(*handle).ok_or_else(|| invalid(kind, 330))?;
                sources.push(source(found));
            }
            if sources.len() != count {
                return Err(invalid(kind, 97));
            }
            loops.push(HatchLoop {
                boundary,
                external: flags & 1 != 0,
                outermost: flags & 16 != 0,
                derived: flags & 4 != 0,
                text_box: flags & 8 != 0,
                sources,
            });
        }
        let islands = match values.int(75)? {
            0 => Islands::Nested,
            1 => Islands::Outer,
            2 => Islands::Ignore,
            _ => return Err(invalid(kind, 75)),
        };
        let origin = match values.int(76)? {
            0 => PatternOrigin::UserDefined,
            1 => PatternOrigin::Predefined,
            2 => PatternOrigin::Custom,
            _ => return Err(invalid(kind, 76)),
        };
        // 塗りでも、パターンの欄を書く形式がある。塗りなら読むだけにする
        let pattern = if values.next() == Some(52) {
            let angle = values.real(52)?;
            let scale = values.real(41)?;
            let double = values.int(77)? != 0;
            let mut lines = Vec::new();
            for _ in 0..values.count(78)? {
                let angle = values.real(53)?;
                let base = [values.real(43)?, values.real(44)?];
                let offset = [values.real(45)?, values.real(46)?];
                let mut dashes = Vec::new();
                for _ in 0..values.count(79)? {
                    dashes.push(values.real(49)?);
                }
                lines.push(PatternLine {
                    angle,
                    base,
                    offset,
                    dashes,
                });
            }
            Some(Pattern {
                name: self.string(object, 2)?.unwrap_or_default(),
                origin,
                angle,
                scale,
                double,
                lines,
            })
        } else {
            None
        };
        if values.next() == Some(47) {
            values.real(47)?;
        }
        let mut seeds = Vec::new();
        if values.next() == Some(98) {
            for _ in 0..values.count(98)? {
                seeds.push(point(values.real(10)?, values.real(20)?));
            }
        }
        let fill = match (self.int(object, 450, 0)?, solid, pattern) {
            (0, true, _) => Fill::Solid,
            (0, false, Some(pattern)) => Fill::Pattern(pattern),
            (0, false, None) => return Err(invalid(kind, 52)),
            (1, _, _) => Fill::Gradient(self.gradient(object)?),
            _ => return Err(invalid(kind, 450)),
        };
        Ok(Hatch {
            loops,
            islands,
            fill,
            associative,
            seeds,
        })
    }

    /// HATCH の境界の辺。種類（72）が 1 なら線分、2 なら円弧、3 なら楕円弧（長軸の端は中心からの向き）、4 なら
    /// SPLINE
    fn hatch_edge<P: Copy>(
        &self,
        values: &mut Values,
        fit_data: bool,
        point: &impl Fn(f64, f64) -> P,
        length: &impl Fn(f64) -> Length,
    ) -> Lifted<Edge<P>> {
        let kind = values.kind;
        let edge = match values.int(72)? {
            1 => Edge::Line {
                start: point(values.real(10)?, values.real(20)?),
                end: point(values.real(11)?, values.real(21)?),
            },
            2 => {
                let center = point(values.real(10)?, values.real(20)?);
                let radius = values.real(40)?;
                if radius <= 0.0 {
                    return Err(invalid(kind, 40));
                }
                Edge::Arc {
                    center,
                    radius: length(radius),
                    start: values.real(50)?,
                    end: values.real(51)?,
                    counterclockwise: values.int(73)? != 0,
                }
            }
            3 => {
                let [x, y] = [values.real(10)?, values.real(20)?];
                let [dx, dy] = [values.real(11)?, values.real(21)?];
                let ratio = values.real(40)?;
                if !(ratio > 0.0 && ratio <= 1.0) {
                    return Err(invalid(kind, 40));
                }
                Edge::Elliptic {
                    center: point(x, y),
                    major: point(x + dx, y + dy),
                    ratio,
                    start: values.real(50)?,
                    end: values.real(51)?,
                    counterclockwise: values.int(73)? != 0,
                }
            }
            // 次数（94）・有理（73）・周期的（74）・節点（95・40）・制御点（96・10・20）と、有理なら重み（42）。
            // 2010 以降の DXF は、当てはめる点（97・11・21）と、あれば接線（12・22・13・23）が続く
            4 => {
                let degree = u32::try_from(values.int(94)?).map_err(|_| invalid(kind, 94))?;
                let rational = values.int(73)? != 0;
                let periodic = values.int(74)? != 0;
                let knots = values.count(95)?;
                let controls = values.count(96)?;
                let mut spline = Spline {
                    degree,
                    knots: Vec::new(),
                    control: Vec::new(),
                    weights: Vec::new(),
                    fit: Vec::new(),
                    start_tangent: None,
                    end_tangent: None,
                    closed: false,
                    periodic,
                };
                for _ in 0..knots {
                    spline.knots.push(values.real(40)?);
                }
                for _ in 0..controls {
                    spline
                        .control
                        .push(point(values.real(10)?, values.real(20)?));
                    if rational {
                        spline.weights.push(values.real(42)?);
                    }
                }
                if fit_data {
                    let fit = values.count(97)?;
                    for _ in 0..fit {
                        spline.fit.push(point(values.real(11)?, values.real(21)?));
                    }
                    if values.next() == Some(12) {
                        spline.start_tangent = Some([values.real(12)?, values.real(22)?]);
                    }
                    if values.next() == Some(13) {
                        spline.end_tangent = Some([values.real(13)?, values.real(23)?]);
                    }
                }
                check_spline(kind, &spline)?;
                Edge::Spline(spline)
            }
            _ => return Err(invalid(kind, 72)),
        };
        Ok(edge)
    }

    /// HATCH のグラデーション（450〜470）。回転（460）はラジアンなので度に直す。1 色（452 が 1）なら、色の
    /// 濃さ（462）を持つ
    fn gradient(&self, object: &Object) -> Lifted<Gradient> {
        let kind = object.kind.as_slice();
        let start = object
            .values
            .iter()
            .position(|value| value.code == 450)
            .unwrap_or_default();
        let mut colors = Vec::new();
        for value in object.values.iter().skip(start) {
            match (value.code, &value.data) {
                (421, Data::Int(rgb @ 0..=0xFF_FFFF)) => {
                    let [_, r, g, b] = (*rgb as u32).to_be_bytes();
                    colors.push([r, g, b]);
                }
                (421, _) => return Err(invalid(kind, 421)),
                _ => {}
            }
        }
        let tint = match self.int(object, 452, 0)? {
            0 => None,
            _ => Some(self.real(object, 462, None)?),
        };
        Ok(Gradient {
            name: self.string(object, 470)?.unwrap_or_default(),
            angle: self.real(object, 460, Some(0.0))?.to_degrees(),
            shift: self.real(object, 461, Some(0.0))?,
            colors,
            tint,
        })
    }

    /// DIMENSION（DXF Reference「DIMENSION」「Common Dimension Group Codes」）。70 の下の 5 ビットが種類で、
    /// 64 は座標寸法の X、128 は利用者が動かした文字。利用者の文字（1）は、ないか空か `<>` なら測定値を、
    /// 空白 1 つなら書かないことを表す。複製の挿入点（12）・行間（41・72）・水平の向き（51）は移さない
    /// （推定。docs/formats/dxf.md）
    fn dimension<P: Copy>(
        &self,
        object: &Object,
        point: &impl Fn(f64, f64) -> P,
        length: &impl Fn(f64) -> Length,
    ) -> Lifted<Dimension<P>> {
        let kind = object.kind.as_slice();
        let at = |x: i16| self.planar_point(object, x, point);
        let real = |code: i16, default: Option<f64>| self.real(object, code, default);
        let flags = self.int(object, 70, 0)?;
        let dimension = match flags & 0b1_1111 {
            0 => DimensionKind::Linear {
                first: at(13)?,
                second: at(14)?,
                line: at(10)?,
                rotation: real(50, Some(0.0))?,
                oblique: real(52, Some(0.0))?,
            },
            1 => DimensionKind::Aligned {
                first: at(13)?,
                second: at(14)?,
                line: at(10)?,
                oblique: real(52, Some(0.0))?,
            },
            2 => DimensionKind::AngularLines {
                first: [at(13)?, at(14)?],
                second: [at(15)?, at(10)?],
                arc: at(16)?,
            },
            3 => DimensionKind::Diameter {
                chord: at(15)?,
                opposite: at(10)?,
                leader: length(real(40, None)?),
            },
            4 => DimensionKind::Radius {
                center: at(10)?,
                chord: at(15)?,
                leader: length(real(40, None)?),
            },
            5 => DimensionKind::Angular {
                vertex: at(15)?,
                first: at(13)?,
                second: at(14)?,
                arc: at(10)?,
            },
            6 => DimensionKind::Ordinate {
                origin: at(10)?,
                feature: at(13)?,
                leader_end: at(14)?,
                axis: if flags & 64 != 0 { Axis::X } else { Axis::Y },
            },
            _ => return Err(invalid(kind, 70)),
        };
        let label = match self.string(object, 1)?.as_deref() {
            None | Some("") | Some("<>") => Label::Measurement,
            Some(" ") => Label::Hidden,
            Some(text) => Label::Override {
                spans: mtext::dimension_spans(text, length),
                content: text.to_owned(),
            },
        };
        // 71 は 1〜9 で、上・中・下の段ごとに左・中央・右
        let attachment = match object.first(71) {
            None => None,
            Some(_) => match self.int(object, 71, 0)? {
                at @ 1..=9 => Some((
                    match (at - 1) % 3 {
                        0 => Horizontal::Left,
                        1 => Horizontal::Center,
                        _ => Horizontal::Right,
                    },
                    match (at - 1) / 3 {
                        0 => Vertical::Top,
                        1 => Vertical::Middle,
                        _ => Vertical::Bottom,
                    },
                )),
                _ => return Err(invalid(kind, 71)),
            },
        };
        let position = match object.first(11) {
            Some(_) => Some(at(11)?),
            None => None,
        };
        let rotation = match object.first(53) {
            Some(_) => Some(real(53, None)?),
            None => None,
        };
        Ok(Dimension {
            kind: dimension,
            measurement: match object.first(42) {
                Some(_) => Some(Measured {
                    value: real(42, None)?,
                    key: "dxf.DIMENSION.42",
                }),
                None => None,
            },
            label,
            text: DimensionText {
                position,
                moved: flags & 128 != 0,
                rotation,
                attachment,
            },
            style: self
                .string(object, 3)?
                .unwrap_or_else(|| "STANDARD".to_owned()),
            block: self.string(object, 2)?,
        })
    }

    /// LEADER（DXF Reference「LEADER」）。頂点（10）は数（76）と合わなければ読めないとする。矢印（71）がなければ
    /// 寸法スタイルに従うとして `None` にする。注記（340）は、指すオブジェクトの出所にする。鉤の線（74・75）・
    /// 注記の種類と大きさ（73・40・41）・ずれ（212・213）は移さない（推定。docs/formats/dxf.md）
    fn leader<P: Copy>(
        &self,
        db: &AcadDb,
        object: &Object,
        point: &impl Fn(f64, f64) -> P,
    ) -> Lifted<Leader<P>> {
        let kind = object.kind.as_slice();
        let mut path = Vec::new();
        let mut current: Option<[Option<f64>; 3]> = None;
        let finish = |current: Option<[Option<f64>; 3]>, path: &mut Vec<P>| -> Lifted<()> {
            if let Some([x, y, z]) = current {
                let (Some(x), Some(y)) = (x, y) else {
                    return Err(invalid(kind, 20));
                };
                if z.unwrap_or(0.0) != 0.0 {
                    return Err(non_planar(kind));
                }
                path.push(point(x, y));
            }
            Ok(())
        };
        for value in &object.values {
            let slot = match value.code {
                10 => {
                    finish(current.take(), &mut path)?;
                    0
                }
                20 => 1,
                30 => 2,
                _ => continue,
            };
            let Data::Real(number) = value.data else {
                return Err(invalid(kind, value.code));
            };
            let slot = current.get_or_insert([None; 3]).get_mut(slot);
            match slot {
                Some(slot @ None) if number.is_finite() => *slot = Some(number),
                _ => return Err(invalid(kind, value.code)),
            }
        }
        finish(current, &mut path)?;
        let count = self.int(object, 76, -1)?;
        if path.len() < 2 || usize::try_from(count).ok() != Some(path.len()) {
            return Err(invalid(kind, 76));
        }
        let annotation = match object.first(340) {
            Some(Data::Handle(handle)) => {
                Some(source(db.get(*handle).ok_or_else(|| invalid(kind, 340))?))
            }
            Some(_) => return Err(invalid(kind, 340)),
            None => None,
        };
        let arrow = match object.first(71) {
            Some(_) => Some(self.int(object, 71, 0)? != 0),
            None => None,
        };
        Ok(Leader {
            path,
            spline: match self.int(object, 72, 0)? {
                0 => false,
                1 => true,
                _ => return Err(invalid(kind, 72)),
            },
            arrow,
            annotation,
            style: self
                .string(object, 3)?
                .unwrap_or_else(|| "STANDARD".to_owned()),
        })
    }

    /// 平面の点（`x` と `x + 10`）。Z（`x + 20`）がなければ 0 とし、0 でなければ移さない
    /// MULTILEADER（DXF Reference「MLEADER」）。値を、注記の文脈（300〜301）・枝（302〜303）・線（304〜305）の
    /// 区切りで分けてから読む。注記は、文脈の 290 が真なら書式つきの文字、296 が真なら記号とする。公差の注記
    /// （172 = 3）と段組み（173）は、まだ移さない。線と注記の色・線種・線幅は `callout_style` で読む。
    /// 線ごとの上書きと、寄せ方の欄は、まだ移さない（docs/formats/dxf.md）
    fn callout<P: Copy>(
        &self,
        db: &AcadDb,
        object: &Object,
        point: &impl Fn(f64, f64) -> P,
        length: &impl Fn(f64) -> Length,
    ) -> Lifted<Callout<P>> {
        let kind = object.kind.as_slice();
        let parts = split_callout(object)?;
        let part = |values: Vec<Value>| Object {
            values,
            ..object.clone()
        };
        let entity = part(parts.entity);
        let context = part(parts.context);
        if self.int(&entity, 172, 0)? == 3 {
            return Err(invalid(kind, 172));
        }
        let content = match (
            self.flag(&context, 290, false)?,
            self.flag(&context, 296, false)?,
        ) {
            (false, false) => CalloutContent::None,
            (true, false) => {
                CalloutContent::Text(Box::new(self.callout_text(db, &context, point, length)?))
            }
            (false, true) => CalloutContent::Symbol {
                symbol: Box::new(self.callout_symbol(db, &context, point)?),
                values: self.attribute_values(db, &entity)?,
            },
            (true, true) => return Err(invalid(kind, 296)),
        };
        let doglegs = self.flag(&entity, 291, true)?;
        let mut branches = Vec::new();
        for (leader, lines) in parts.leaders {
            let leader = part(leader);
            let end = if self.flag(&leader, 290, false)? {
                Some(self.planar_point(&leader, 10, point)?)
            } else {
                None
            };
            let dogleg = if doglegs && self.flag(&leader, 291, false)? {
                let direction = [self.real(&leader, 11, None)?, self.real(&leader, 21, None)?];
                if direction == [0.0, 0.0] || self.real(&leader, 31, Some(0.0))? != 0.0 {
                    return Err(invalid(kind, 11));
                }
                Some(Dogleg {
                    direction,
                    length: length(self.real(&leader, 40, None)?),
                })
            } else {
                None
            };
            let lines = lines
                .iter()
                .map(|line| self.vertices(kind, line, point))
                .collect::<Lifted<Vec<_>>>()?;
            branches.push(Branch { lines, end, dogleg });
        }
        let path = match self.int(&entity, 170, 1)? {
            0 => PathKind::Hidden,
            1 => PathKind::Straight,
            2 => PathKind::Spline,
            _ => return Err(invalid(kind, 170)),
        };
        // 端部記号の大きさは図形の 42 にあり、なければ文脈の 140 とする
        let arrow_size = match entity.first(42) {
            Some(_) => self.real(&entity, 42, None)?,
            None => self.real(&context, 140, None)?,
        };
        let scale = self.real(&context, 40, Some(1.0))?;
        if scale <= 0.0 || arrow_size < 0.0 {
            return Err(invalid(kind, if scale <= 0.0 { 40 } else { 42 }));
        }
        let style = match entity.first(340) {
            None => None,
            Some(Data::Handle(handle)) => {
                Some(source(db.get(*handle).ok_or_else(|| invalid(kind, 340))?))
            }
            Some(_) => return Err(invalid(kind, 340)),
        };
        Ok(Callout {
            branches,
            content,
            path,
            arrowhead: self
                .block_key(db, &entity, 342)?
                .map_or(Arrowhead::Default, Arrowhead::Symbol),
            arrow_size: length(arrow_size),
            landing: self.flag(&entity, 290, true)?,
            scale,
            // 線と注記の色・線種・線幅は、表の線種を引くので `element` で読む
            line_style: Style::BLOCK,
            content_color: Inherit::Block,
            style,
        })
    }

    /// MULTILEADER の線の色（91）・線種（341）・線幅（171）と、注記の色（文字は 92、記号は 93）（DXF
    /// Reference「MLEADER」）。値がなければ、注記の要素に従う（推定。docs/formats/dxf.md）。線ごとの上書き
    /// （線の 93 が 0 でないか、線の色（92）が注記の要素に従わない）は移さず、診断する
    fn callout_style(
        &self,
        db: &AcadDb,
        object: &Object,
        symbol: bool,
        out: &mut Out,
        cx: &mut Cx,
    ) -> Lifted<(Style, Inherit<Color>)> {
        let kind = object.kind.as_slice();
        let parts = split_callout(object)?;
        // DXF の AcDbEntity の欄は、代わりの図形の大きさ（92）を持つ。MULTILEADER の欄は、最後の区切り（100）の後から読む
        let start = parts
            .entity
            .iter()
            .rposition(|value| value.code == 100)
            .map_or(0, |at| at + 1);
        let entity = Object {
            values: parts.entity.get(start..).unwrap_or_default().to_vec(),
            ..object.clone()
        };
        let line_type = match entity.first(341) {
            None | Some(Data::Handle(0)) => Inherit::Block,
            Some(Data::Handle(handle)) => {
                let record = db
                    .get(*handle)
                    .filter(|record| record.kind == b"LTYPE")
                    .ok_or_else(|| invalid(kind, 341))?;
                match self.string(record, 2)? {
                    Some(name) if name.eq_ignore_ascii_case("ByLayer") => Inherit::Layer,
                    Some(name) if name.eq_ignore_ascii_case("ByBlock") => Inherit::Block,
                    _ => Inherit::Own(
                        *out.line_type_handles
                            .get(handle)
                            .ok_or_else(|| invalid(kind, 341))?,
                    ),
                }
            }
            Some(_) => return Err(invalid(kind, 341)),
        };
        let line_weight = match entity.first(171) {
            None | Some(Data::Int(-2)) => Inherit::Block,
            Some(Data::Int(-1)) => Inherit::Layer,
            Some(Data::Int(-3)) => Inherit::Own(LineWeight::Default),
            Some(Data::Int(width @ 0..=211)) => {
                Inherit::Own(LineWeight::Width(PaperLength(*width as f64 / 100.0)))
            }
            Some(_) => return Err(invalid(kind, 171)),
        };
        let line_style = Style {
            color: self.inherited_color(&entity, 91)?,
            line_type,
            line_type_scale: 1.0,
            line_weight: Some(line_weight),
        };
        let content_color = self.inherited_color(&entity, if symbol { 93 } else { 92 })?;
        for line in parts.leaders.iter().flat_map(|(_, lines)| lines) {
            let line = Object {
                values: line.clone(),
                ..object.clone()
            };
            let overridden = match line.first(93) {
                None | Some(Data::Int(0)) => false,
                Some(Data::Int(_)) => true,
                Some(_) => return Err(invalid(kind, 93)),
            };
            if overridden
                || line.first(92).is_some() && self.inherited_color(&line, 92)? != Inherit::Block
            {
                cx.report(Diagnostic::new(codes::DXF_LEADER_LINE_STYLE_DROPPED).at(source(object)));
            }
        }
        Ok((line_style, content_color))
    }

    /// 継承を含む色の値（AcCmColor）。上の 1 バイトが 0xC0 ならレイヤ、0xC1 なら配置から継承する（推定。
    /// docs/formats/dxf.md）。ほかは `raw_color` で読む。なければ配置から継承する
    fn inherited_color(&self, object: &Object, code: i16) -> Lifted<Inherit<Color>> {
        match object.first(code) {
            None => Ok(Inherit::Block),
            Some(Data::Int(value)) => match (*value as u32).to_be_bytes() {
                [0xC0, ..] => Ok(Inherit::Layer),
                [0xC1, ..] => Ok(Inherit::Block),
                _ => Ok(Inherit::Own(self.raw_color(object, code)?)),
            },
            Some(_) => Err(invalid(&object.kind, code)),
        }
    }

    /// MULTILEADER の文脈の書式つきの文字。欄は MTEXT と同じ意味で、符号が違う（DXF Reference「MLEADER」）。
    /// 背景の色（91）は、ほかの色と同じく元の形式の色の値とする
    fn callout_text<P: Copy>(
        &self,
        db: &AcadDb,
        context: &Object,
        point: &impl Fn(f64, f64) -> P,
        length: &impl Fn(f64) -> Length,
    ) -> Lifted<FormattedText<P>> {
        let kind = context.kind.as_slice();
        if self.int(context, 173, 0)? != 0 {
            return Err(columns());
        }
        let content = self
            .string(context, 304)?
            .ok_or_else(|| invalid(kind, 304))?;
        let (horizontal, vertical) = attachment(kind, self.int(context, 171, 1)?, 171)?;
        let direction = [
            self.real(context, 13, Some(0.0))?,
            self.real(context, 23, Some(0.0))?,
        ];
        if self.real(context, 33, Some(0.0))? != 0.0 {
            return Err(non_planar(kind));
        }
        let rotation = if direction == [0.0, 0.0] {
            Rotation::Angle(self.real(context, 42, Some(0.0))?.to_degrees())
        } else {
            Rotation::Direction(direction)
        };
        let frame = |code: i16| -> Lifted<Option<Length>> {
            match self.real(context, code, Some(0.0))? {
                0.0 => Ok(None),
                size if size > 0.0 => Ok(Some(length(size))),
                _ => Err(invalid(kind, code)),
            }
        };
        let background = if self.flag(context, 292, false)? {
            let fill = if self.flag(context, 291, false)? {
                BackgroundFill::Screen
            } else {
                BackgroundFill::Color(self.raw_color(context, 91)?)
            };
            let scale = self.real(context, 141, Some(1.5))?;
            if scale <= 0.0 {
                return Err(invalid(kind, 141));
            }
            let transparency = match context.first(92) {
                None => None,
                Some(Data::Int(value)) => {
                    Some(u32::try_from(*value).map_err(|_| invalid(kind, 92))?)
                }
                Some(_) => return Err(invalid(kind, 92)),
            };
            Some(Background {
                fill: Some(fill),
                frame: false,
                scale,
                transparency,
            })
        } else {
            None
        };
        let style = match context.first(340) {
            None => "STANDARD".to_owned(),
            Some(Data::Handle(handle)) => db
                .get(*handle)
                .filter(|record| record.kind == b"STYLE")
                .map(|record| self.string(record, 2))
                .transpose()?
                .flatten()
                .ok_or_else(|| invalid(kind, 340))?,
            Some(_) => return Err(invalid(kind, 340)),
        };
        let factor = self.real(context, 45, Some(1.0))?;
        if factor <= 0.0 {
            return Err(invalid(kind, 45));
        }
        Ok(FormattedText {
            spans: mtext::spans(&content, length),
            content,
            height: Height {
                value: length(self.real(context, 41, None)?),
                meaning: HeightMeaning::Declared("dxf.MULTILEADER.41"),
            },
            point: self.planar_point(context, 12, point)?,
            horizontal,
            vertical,
            rotation,
            width: frame(43)?,
            frame_height: frame(44)?,
            flow: flow(kind, self.int(context, 172, 1)?, 172)?,
            line_spacing: LineSpacing {
                factor,
                exact: match self.int(context, 170, 1)? {
                    1 => false,
                    2 => true,
                    _ => return Err(invalid(kind, 170)),
                },
            },
            background,
            style,
        })
    }

    /// MULTILEADER の文脈の記号。ブロックのレコード（341）を指し、位置（15）・尺度（16・26）・回転（46、
    /// ラジアン）を持つ。変換の行列（47）は、これらから決まる値なので移さない
    fn callout_symbol<P: Copy>(
        &self,
        db: &AcadDb,
        context: &Object,
        point: &impl Fn(f64, f64) -> P,
    ) -> Lifted<Symbol<P>> {
        let kind = context.kind.as_slice();
        let key = self
            .block_key(db, context, 341)?
            .ok_or_else(|| invalid(kind, 341))?;
        let mut scale = [1.0; 2];
        for (code, slot) in [16, 26].into_iter().zip(&mut scale) {
            *slot = self.real(context, code, Some(1.0))?;
            if *slot == 0.0 {
                return Err(invalid(kind, code));
            }
        }
        Ok(Symbol {
            key,
            position: self.planar_point(context, 15, point)?,
            scale,
            rotation: self.real(context, 46, Some(0.0))?.to_degrees(),
            grid: None,
            attributes: Vec::new(),
            clip: None,
        })
    }

    /// MULTILEADER の記号の属性の値。属性の定義（330）・番号（177）・幅（44）・値（302）の組を、入力の順に
    /// 読む。番号と幅は、まだ移さない
    fn attribute_values(&self, db: &AcadDb, entity: &Object) -> Lifted<Vec<AttributeValue>> {
        let kind = entity.kind.as_slice();
        let mut values = Vec::new();
        let mut definition = None;
        // 属性の組は、記号のブロック（344）の後にある。前の 330 は図形の所有者なので読まない
        let start = entity
            .values
            .iter()
            .position(|value| value.code == 344)
            .map_or(0, |at| at + 1);
        for value in entity.values.iter().skip(start) {
            match (value.code, &value.data) {
                (330, Data::Handle(handle)) => {
                    let found = db
                        .get(*handle)
                        .filter(|record| record.kind == b"ATTDEF")
                        .ok_or_else(|| invalid(kind, 330))?;
                    definition = Some(source(found));
                }
                (302, data) => values.push(AttributeValue {
                    definition: definition.take().ok_or_else(|| invalid(kind, 330))?,
                    text: self.decode(kind, 302, data)?,
                }),
                _ => {}
            }
        }
        Ok(values)
    }

    /// 線の頂点（10・20・30）。1 つもなければ読めないとする
    fn vertices<P>(
        &self,
        kind: &[u8],
        values: &[Value],
        point: &impl Fn(f64, f64) -> P,
    ) -> Lifted<Vec<P>> {
        let vertices = self.points(kind, values, 10, point)?;
        if vertices.is_empty() {
            return Err(invalid(kind, 10));
        }
        Ok(vertices)
    }

    /// `code` で始まる点（X・Y と、あれば Z）の並び。Z が 0 でなければ平面に移さない
    fn points<P>(
        &self,
        kind: &[u8],
        values: &[Value],
        code: i16,
        point: &impl Fn(f64, f64) -> P,
    ) -> Lifted<Vec<P>> {
        let mut values = Values { kind, values };
        let mut points = Vec::new();
        while let Some(next) = values.next() {
            if next != code {
                values.values = values.values.get(1..).unwrap_or_default();
                continue;
            }
            let (x, y) = (values.real(code)?, values.real(code + 10)?);
            if values.next() == Some(code + 20) && values.real(code + 20)? != 0.0 {
                return Err(non_planar(kind));
            }
            points.push(point(x, y));
        }
        Ok(points)
    }

    /// 元の形式の色の値（AcCmColor）。上の 1 バイトが 0xC2 なら下の 3 バイトが RGB、0xC3 なら下の 1 バイトが
    /// 色の番号（推定。docs/formats/dxf.md）。ほかは読めないとする
    fn raw_color(&self, object: &Object, code: i16) -> Lifted<Color> {
        let value = match object.first(code) {
            Some(Data::Int(value)) => *value as u32,
            _ => return Err(invalid(&object.kind, code)),
        };
        let [method, r, g, b] = value.to_be_bytes();
        match (method, b) {
            (0xC2, _) => Ok(Color {
                code: None,
                rgb: Some([r, g, b]),
            }),
            (0xC3, index @ 1..=255) => Ok(Color {
                code: Some(aci(i64::from(index))),
                rgb: None,
            }),
            _ => Err(invalid(&object.kind, code)),
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

    /// ブロックのレコードを指すハンドルを、記号のキーにする。ないか 0 なら `None`
    fn block_key(&self, db: &AcadDb, object: &Object, code: i16) -> Lifted<Option<SymbolKey>> {
        let kind = object.kind.as_slice();
        match object.first(code) {
            None | Some(Data::Handle(0)) => Ok(None),
            Some(Data::Handle(handle)) => {
                let record = db
                    .get(*handle)
                    .filter(|record| record.kind == b"BLOCK_RECORD")
                    .ok_or_else(|| invalid(kind, code))?;
                let name = self.string(record, 2)?.ok_or_else(|| invalid(kind, code))?;
                Ok(Some(SymbolKey {
                    namespace: BLOCK,
                    name,
                }))
            }
            Some(_) => Err(invalid(kind, code)),
        }
    }

    /// 半径（40）。正でなければ読めないとする
    fn radius(&self, object: &Object) -> Lifted<f64> {
        let radius = self.real(object, 40, None)?;
        if radius > 0.0 {
            Ok(radius)
        } else {
            Err(invalid(&object.kind, 40))
        }
    }

    /// ELLIPSE（DXF Reference「ELLIPSE」）。長軸の端（11・21）は中心からの向きで、比（40）は 0 より大きく 1 以下。
    /// 始まりと終わりのパラメーター（41・42）はラジアンなので、度に直す
    fn ellipse<P>(&self, object: &Object, point: &impl Fn(f64, f64) -> P) -> Lifted<Edge<P>> {
        let kind = object.kind.as_slice();
        if self.real(object, 30, Some(0.0))? != 0.0 || self.real(object, 31, Some(0.0))? != 0.0 {
            return Err(non_planar(kind));
        }
        let [x, y] = [self.real(object, 10, None)?, self.real(object, 20, None)?];
        let [dx, dy] = [self.real(object, 11, None)?, self.real(object, 21, None)?];
        if [dx, dy] == [0.0, 0.0] {
            return Err(invalid(kind, 11));
        }
        let ratio = self.real(object, 40, None)?;
        if !(ratio > 0.0 && ratio <= 1.0) {
            return Err(invalid(kind, 40));
        }
        Ok(Edge::Elliptic {
            center: point(x, y),
            major: point(x + dx, y + dy),
            ratio,
            start: self.real(object, 41, Some(0.0))?.to_degrees(),
            end: self
                .real(object, 42, Some(core::f64::consts::TAU))?
                .to_degrees(),
            counterclockwise: true,
        })
    }

    /// LWPOLYLINE（DXF Reference「LWPOLYLINE」）。頂点（10・20）ごとに、続く始まりと終わりの幅（40・41）と
    /// 膨らみ（42）を読む。頂点の数（90）と合わなければ読めないとする。標高（38）が 0 でなければ平面に移さない。
    /// 線種の生成の印（70 の 128）と厚さ（39）は、`dxf.entity` の拡張に控える（決定 0142）
    fn lwpolyline<P: Copy>(
        &self,
        object: &Object,
        point: &impl Fn(f64, f64) -> P,
        length: &impl Fn(f64) -> Length,
    ) -> Lifted<Polyline<P>> {
        let kind = object.kind.as_slice();
        if self.real(object, 38, Some(0.0))? != 0.0 {
            return Err(non_planar(kind));
        }
        let mut corners: Vec<([f64; 2], [Option<f64>; 2], f64)> = Vec::new();
        let mut values = Values {
            kind,
            values: &object.values,
        };
        while let Some(code) = values.next() {
            match code {
                10 => {
                    let at = [values.real(10)?, values.real(20)?];
                    corners.push((at, [None; 2], 0.0));
                }
                40..=42 => {
                    let value = values.real(code)?;
                    let (_, widths, bulge) =
                        corners.last_mut().ok_or_else(|| invalid(kind, code))?;
                    match code {
                        40 => widths[0] = Some(value),
                        41 => widths[1] = Some(value),
                        _ => *bulge = value,
                    }
                }
                _ => values.values = values.values.get(1..).unwrap_or_default(),
            }
        }
        if usize::try_from(self.int(object, 90, -1)?).ok() != Some(corners.len()) {
            return Err(invalid(kind, 90));
        }
        let any_width = corners
            .iter()
            .any(|(_, widths, _)| widths.iter().any(Option::is_some));
        let mut vertex_widths = Vec::new();
        let mut vertices = Vec::new();
        for ([x, y], [start, end], bulge) in corners {
            if any_width {
                vertex_widths.push([length(start.unwrap_or(0.0)), length(end.unwrap_or(0.0))]);
            }
            vertices.push(Vertex {
                point: point(x, y),
                bulge,
            });
        }
        let width = match self.real(object, 43, Some(0.0))? {
            0.0 => None,
            width if width > 0.0 => Some(length(width)),
            _ => return Err(invalid(kind, 43)),
        };
        Ok(Polyline {
            vertices,
            closed: self.int(object, 70, 0)? & 1 != 0,
            width,
            vertex_widths,
        })
    }

    /// 古い POLYLINE と、その VERTEX（DXF Reference「POLYLINE」「VERTEX」）。印（70）の 1 は閉じている、8 は 3D
    /// の折れ線。曲線（2）かスプライン（4）で当てはめたものと、網（16）・多面（64）は、まだ移さない。頂点は VERTEX
    /// の点（10・20）と膨らみ（42）と、始まりと終わりの幅（40・41。なければ 0）。どの頂点の幅も同じなら一定の
    /// 幅にし、0 なら幅を持たない。POLYLINE の既定の幅（40・41）は、新しい頂点のためのものなので使わない。
    /// 標高（POLYLINE の 30）か頂点の Z（30）が 0 でなければ平面に移さない。厚さ（39）と線種の生成（70 の 128）は、`dxf.entity` の拡張に控える（決定 0142）
    fn polyline<P: Copy>(
        &self,
        object: &Object,
        vertices: &[&Object],
        point: &impl Fn(f64, f64) -> P,
        length: &impl Fn(f64) -> Length,
    ) -> Lifted<Polyline<P>> {
        let kind = object.kind.as_slice();
        let flags = self.int(object, 70, 0)?;
        if flags & (2 | 4 | 16 | 64) != 0 {
            return Err(Box::new(
                Diagnostic::new(codes::DXF_LIFT_DROPPED).with("kind", text(kind)),
            ));
        }
        if self.real(object, 30, Some(0.0))? != 0.0 {
            return Err(non_planar(kind));
        }
        let mut corners = Vec::new();
        for vertex in vertices {
            let widths = [
                self.real(vertex, 40, Some(0.0))?,
                self.real(vertex, 41, Some(0.0))?,
            ];
            if widths.iter().any(|width| *width < 0.0) {
                return Err(invalid(&vertex.kind, 40));
            }
            if self.real(vertex, 30, Some(0.0))? != 0.0 {
                return Err(non_planar(kind));
            }
            let at = point(self.real(vertex, 10, None)?, self.real(vertex, 20, None)?);
            corners.push((at, widths, self.real(vertex, 42, Some(0.0))?));
        }
        let first = corners.first().map(|(_, widths, _)| *widths);
        let uniform = first.filter(|[start, end]| {
            start == end
                && corners
                    .iter()
                    .all(|(_, widths, _)| *widths == [*start, *end])
        });
        let (width, vertex_widths) = match uniform {
            Some([width, _]) => ((width > 0.0).then(|| length(width)), Vec::new()),
            None => (
                None,
                corners
                    .iter()
                    .map(|(_, [start, end], _)| [length(*start), length(*end)])
                    .collect(),
            ),
        };
        Ok(Polyline {
            vertices: corners
                .into_iter()
                .map(|(point, _, bulge)| Vertex { point, bulge })
                .collect(),
            closed: flags & 1 != 0,
            width,
            vertex_widths,
        })
    }

    /// SPLINE（DXF Reference「SPLINE」）。印（70）の 1 は閉じている、2 は周期的、4 は有理。次数（71）・節点
    /// （40）・制御点（10・20・30）・重み（41）・当てはめる点（11・21・31）・接線（12・13）を持つ。節点・制御点・
    /// 当てはめる点の数（72〜74）と合わなければ読めないとする。許容差（42〜44）は、まだ移さない
    fn spline<P>(&self, object: &Object, point: &impl Fn(f64, f64) -> P) -> Lifted<Spline<P>> {
        let kind = object.kind.as_slice();
        let flags = self.int(object, 70, 0)?;
        let degree = u32::try_from(self.int(object, 71, -1)?).map_err(|_| invalid(kind, 71))?;
        let reals = |code: i16| -> Lifted<Vec<f64>> {
            object
                .values
                .iter()
                .filter(|value| value.code == code)
                .map(|value| match value.data {
                    Data::Real(value) if value.is_finite() => Ok(value),
                    _ => Err(invalid(kind, code)),
                })
                .collect()
        };
        let tangent = |code: i16| -> Lifted<Option<[f64; 2]>> {
            if object.first(code).is_none() {
                return Ok(None);
            }
            if self.real(object, code + 20, Some(0.0))? != 0.0 {
                return Err(non_planar(kind));
            }
            Ok(Some([
                self.real(object, code, None)?,
                self.real(object, code + 10, None)?,
            ]))
        };
        let spline = Spline {
            degree,
            knots: reals(40)?,
            control: self.points(kind, &object.values, 10, point)?,
            weights: reals(41)?,
            fit: self.points(kind, &object.values, 11, point)?,
            start_tangent: tangent(12)?,
            end_tangent: tangent(13)?,
            closed: flags & 1 != 0,
            periodic: flags & 2 != 0,
        };
        for (code, count) in [
            (72, spline.knots.len()),
            (73, spline.control.len()),
            (74, spline.fit.len()),
        ] {
            if object.first(code).is_some()
                && usize::try_from(self.int(object, code, -1)?).ok() != Some(count)
            {
                return Err(invalid(kind, code));
            }
        }
        check_spline(kind, &spline)?;
        Ok(spline)
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

    /// ペーパー空間のビューポート（DXF Reference「VIEWPORT」）。ねじれ・視点の向き・注視点のないものだけを扱う
    fn view(&self, object: &Object, mm: f64, source: Ref) -> Lifted<View> {
        let kind = object.kind.as_slice();
        let real = |code: i16, default: Option<f64>| self.real(object, code, default);
        let unsupported = |code: i16| {
            Box::new(Diagnostic::new(codes::DXF_VIEWPORT_UNSUPPORTED).with("code", i64::from(code)))
        };
        if [
            real(16, Some(0.0))?,
            real(26, Some(0.0))?,
            real(36, Some(1.0))?,
        ] != [0.0, 0.0, 1.0]
        {
            return Err(unsupported(16));
        }
        if [
            real(17, Some(0.0))?,
            real(27, Some(0.0))?,
            real(37, Some(0.0))?,
        ] != [0.0; 3]
        {
            return Err(unsupported(17));
        }
        if real(51, Some(0.0))? != 0.0 {
            return Err(unsupported(51));
        }
        let [px, py] = [real(10, None)?, real(20, None)?];
        let [width, height] = [real(40, None)?, real(41, None)?];
        let [vx, vy] = [real(12, Some(0.0))?, real(22, Some(0.0))?];
        let view_height = real(45, None)?;
        for (code, value) in [(40, width), (41, height), (45, view_height)] {
            if value <= 0.0 {
                return Err(invalid(kind, code));
            }
        }
        // 用紙の mm = mm × (用紙の中心 + 縮尺 × (実寸の点 − ビューの中心) − 用紙の原点)
        let [ox, oy] = self.origin;
        let scale = height / view_height * mm;
        let transform = Affine {
            m: [
                scale,
                0.0,
                (px - ox) * mm - scale * vx,
                0.0,
                scale,
                (py - oy) * mm - scale * vy,
            ],
        };
        let corner = |dx: f64, dy: f64| PaperPoint {
            sheet: self.sheet,
            x: (px + dx * width / 2.0 - ox) * mm,
            y: (py + dy * height / 2.0 - oy) * mm,
        };
        Ok(View {
            space: MODEL,
            transform,
            sheet: self.sheet,
            clip: vec![
                corner(-1.0, -1.0),
                corner(1.0, -1.0),
                corner(1.0, 1.0),
                corner(-1.0, 1.0),
            ],
            hidden_layers: Vec::new(),
            source,
        })
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

    fn paper_mm(&self, kind: &[u8]) -> Lifted<f64> {
        self.mm.ok_or_else(|| {
            Box::new(Diagnostic::new(codes::DXF_PAPER_SPACE_DROPPED).with("kind", text(kind)))
        })
    }

    /// 図形の画層（8）。表にない名前なら、定義のない画層として足す
    fn layer(&self, object: &Object, out: &mut Out) -> Lifted<LayerId> {
        let name = self
            .string(object, 8)?
            .ok_or_else(|| invalid(&object.kind, 8))?;
        if let Some(id) = out.by_name.get(&name.to_ascii_lowercase()) {
            return Ok(*id);
        }
        self.add_layer(&object.kind, name, None, Ref::default(), out)
    }

    /// 図形の色（62・420）・線種（6）・線種の倍率（48）・線幅（370）（DXF Reference「Common Group Codes for
    /// Entities」）
    fn style(&self, object: &Object, out: &mut Out) -> Lifted<Style> {
        let kind = object.kind.as_slice();
        let rgb = self.rgb(object, 420)?;
        let color = match (self.int(object, 62, 256)?, rgb) {
            (index @ 1..=255, _) => Inherit::Own(Color {
                code: Some(aci(index)),
                rgb,
            }),
            (_, Some(rgb)) => Inherit::Own(Color {
                code: None,
                rgb: Some(rgb),
            }),
            (0, None) => Inherit::Block,
            (256, None) => Inherit::Layer,
            _ => return Err(invalid(kind, 62)),
        };
        let line_type = match self.string(object, 6)? {
            None => Inherit::Layer,
            Some(name) if name.eq_ignore_ascii_case("ByLayer") => Inherit::Layer,
            Some(name) if name.eq_ignore_ascii_case("ByBlock") => Inherit::Block,
            Some(name) => Inherit::Own(self.line_type(kind, name, out)?),
        };
        let line_type_scale = self.real(object, 48, Some(1.0))?;
        if line_type_scale <= 0.0 {
            return Err(invalid(kind, 48));
        }
        let line_weight = match object.first(370) {
            None | Some(Data::Int(-1)) => Some(Inherit::Layer),
            Some(Data::Int(-2)) => Some(Inherit::Block),
            Some(_) => Some(Inherit::Own(self.line_weight(object)?)),
        };
        Ok(Style {
            color,
            line_type,
            line_type_scale,
            line_weight,
        })
    }

    /// 真の色（420、背景は 421）。0x00RRGGBB
    fn rgb(&self, object: &Object, code: i16) -> Lifted<Option<[u8; 3]>> {
        match object.first(code) {
            None => Ok(None),
            Some(Data::Int(value @ 0..=0xFF_FFFF)) => {
                let [_, r, g, b] = (*value as u32).to_be_bytes();
                Ok(Some([r, g, b]))
            }
            Some(_) => Err(invalid(&object.kind, code)),
        }
    }

    /// 線幅（370）。-3 は文書の既定、0〜211 は 1/100 mm
    fn line_weight(&self, object: &Object) -> Lifted<LineWeight> {
        match object.first(370) {
            Some(Data::Int(-3)) => Ok(LineWeight::Default),
            Some(Data::Int(width @ 0..=211)) => {
                Ok(LineWeight::Width(PaperLength(*width as f64 / 100.0)))
            }
            _ => Err(invalid(&object.kind, 370)),
        }
    }

    /// 名前の線種。表になければ、定義のない線種として足す
    fn line_type(&self, kind: &[u8], name: String, out: &mut Out) -> Lifted<LineTypeId> {
        match out.line_type_names.get(&name.to_ascii_lowercase()) {
            Some(id) => Ok(*id),
            None => self.add_line_type(kind, name, None, Ref::default(), out),
        }
    }

    /// 線種の表のレコード（DXF Reference「LTYPE」）。線と間の長さ（49）の列を移す。形か文字を持つ要素
    /// （74 が 0 でない）は、長さだけを移して形と文字を落とし、診断する
    fn line_type_record(
        &self,
        db: &AcadDb,
        object: &Object,
        source: Ref,
        out: &mut Out,
        cx: &mut Cx,
    ) -> Lifted<()> {
        let kind = object.kind.as_slice();
        let name = self.string(object, 2)?.ok_or_else(|| invalid(kind, 2))?;
        let description = self.string(object, 3)?.unwrap_or_default();
        let mut pattern = Vec::new();
        let mut dashes: Vec<Dash> = Vec::new();
        for value in &object.values {
            match (value.code, &value.data, dashes.last_mut()) {
                (49, Data::Real(length), _) if length.is_finite() => {
                    pattern.push(*length);
                    dashes.push(Dash::default());
                }
                (49, _, _) => return Err(invalid(kind, 49)),
                (74, Data::Int(flags), Some(element)) => element.flags = *flags,
                (75, Data::Int(number), Some(element)) => element.number = *number,
                (340, Data::Handle(handle), Some(element)) => element.style = *handle,
                (46, Data::Real(scale), Some(element)) => element.scale = *scale,
                (50, Data::Real(rotation), Some(element)) => element.rotation = *rotation,
                (44, Data::Real(x), Some(element)) => element.offset[0] = *x,
                (45, Data::Real(y), Some(element)) => element.offset[1] = *y,
                (9, data, Some(element)) => element.text = Some(self.decode(kind, 9, data)?),
                (74 | 75 | 340 | 46 | 50 | 44 | 45 | 9, _, _) => {
                    return Err(invalid(kind, value.code));
                }
                _ => {}
            }
        }
        let mut marks = Vec::new();
        let mut unread = false;
        for (index, dash) in dashes.into_iter().enumerate() {
            match self.line_type_mark(db, index, dash) {
                Ok(Some(mark)) => marks.push(mark),
                Ok(None) => {}
                Err(()) => unread = true,
            }
        }
        if unread {
            cx.report(
                Diagnostic::new(codes::DXF_LINE_TYPE_ELEMENT_DROPPED)
                    .with("name", name.clone())
                    .at(source.clone()),
            );
        }
        let definition = LineTypeDefinition {
            description,
            pattern,
            basis: PatternBasis::Element,
            marks,
        };
        let id = self.add_line_type(kind, name, Some(definition), source, out)?;
        if let Some(handle) = object.handle() {
            out.line_type_handles.insert(handle, id);
        }
        Ok(())
    }

    /// 線種の列の要素に添える形か文字（DXF Reference「LTYPE」）。74 の 2 は文字、4 は形、1 は回転を文書の
    /// x 軸から測ること、8 は文字を逆さにしないことを表す（8 は推定。R2013 からの DXF だけが書く）。形も
    /// 文字もなければ `None`。形の番号（75）が 0 の形は、形がないとみなす（推定）。形と文字を兼ねるもの、
    /// 番号が範囲の外のもの、知らない印のもの、有限でない値のものは読めないとする
    fn line_type_mark(
        &self,
        db: &AcadDb,
        index: usize,
        dash: Dash,
    ) -> Result<Option<LineTypeMark>, ()> {
        let Dash {
            flags,
            number,
            style,
            scale,
            rotation,
            offset,
            text,
        } = dash;
        let style = db
            .get(style)
            .filter(|record| style != 0 && record.kind == b"STYLE");
        let content = match flags & 6 {
            0 => return Ok(None),
            2 => MarkContent::Text {
                content: text.ok_or(())?,
                style: style
                    .map(|record| self.string(record, 2))
                    .transpose()
                    .map_err(|_| ())?
                    .flatten(),
            },
            4 if number == 0 => return Ok(None),
            4 => MarkContent::Shape {
                font: style
                    .map(|record| self.string(record, 3))
                    .transpose()
                    .map_err(|_| ())?
                    .flatten()
                    .map(|name| Font {
                        name,
                        kind: FontKind::Shape,
                    }),
                number: u16::try_from(number).map_err(|_| ())?,
            },
            _ => return Err(()),
        };
        if flags & !15 != 0
            || ![scale, rotation, offset[0], offset[1]]
                .iter()
                .all(|v| v.is_finite())
        {
            return Err(());
        }
        Ok(Some(LineTypeMark {
            index,
            content,
            scale,
            rotation,
            rotation_basis: match (flags & 1, flags & 8) {
                (1, _) => MarkRotation::Absolute,
                (_, 8) => MarkRotation::Upright,
                _ => MarkRotation::Relative,
            },
            offset,
        }))
    }

    /// 線種を足す。同じ名前がすでにあれば、名前で引くのは先のものにする
    fn add_line_type(
        &self,
        kind: &[u8],
        name: String,
        definition: Option<LineTypeDefinition>,
        source: Ref,
        out: &mut Out,
    ) -> Lifted<LineTypeId> {
        let id = LineTypeId(u32::try_from(out.line_types.len()).map_err(|_| invalid(kind, 6))?);
        out.line_type_names
            .entry(name.to_ascii_lowercase())
            .or_insert(id);
        out.line_types.push(LineType {
            id,
            name,
            definition,
            source,
        });
        Ok(id)
    }

    /// 画層の表のレコード（DXF Reference「LAYER」）。凍結（70 の 1）か色（62）が負なら表示せず、290 が偽なら
    /// 印刷せず、ロック（70 の 4）なら編集できないとする。凍結は、表示しないこととは別にも持つ（決定 0141）。
    /// 新しいビューポートで凍結（70 の 2）は、これから作るビューのための値なので移さない
    fn layer_record(&self, object: &Object, source: Ref, out: &mut Out) -> Lifted<()> {
        let kind = object.kind.as_slice();
        let name = self.string(object, 2)?.ok_or_else(|| invalid(kind, 2))?;
        let flags = self.int(object, 70, 0)?;
        let state = LayerState {
            visible: flags & 1 == 0 && self.int(object, 62, 7)? >= 0,
            frozen: flags & 1 != 0,
            printable: match object.first(290) {
                Some(Data::Bool(printable)) => *printable,
                Some(_) => return Err(invalid(kind, 290)),
                None => true,
            },
            editable: flags & 4 == 0,
        };
        // 色は 62 の絶対値か 420。線種は、DXF では名前、DWG ではレコードのハンドル
        let rgb = self.rgb(object, 420)?;
        let color = match self.int(object, 62, 0)?.unsigned_abs() {
            index @ 1..=255 => Some(Color {
                code: Some(aci(index as i64)),
                rgb,
            }),
            0 => rgb.map(|rgb| Color {
                code: None,
                rgb: Some(rgb),
            }),
            _ => return Err(invalid(kind, 62)),
        };
        let line_type = match object.first(6) {
            None => None,
            Some(Data::Text(_)) => {
                let name = self.string(object, 6)?.unwrap_or_default();
                Some(self.line_type(kind, name, out)?)
            }
            Some(Data::Handle(handle)) => Some(
                *out.line_type_handles
                    .get(handle)
                    .ok_or_else(|| invalid(kind, 6))?,
            ),
            Some(_) => return Err(invalid(kind, 6)),
        };
        let line_weight = match object.first(370) {
            None => None,
            Some(_) => Some(self.line_weight(object)?),
        };
        let style = LayerStyle {
            color,
            line_type,
            line_weight,
        };
        let id = self.add_layer(kind, name, Some((state, style)), source, out)?;
        if let Some(handle) = object.handle() {
            out.by_handle.insert(handle, id);
        }
        Ok(())
    }

    /// 寸法スタイルの表のレコード（DXF Reference「DIMSTYLE」）。端部記号は、ブロックのレコードのハンドル
    /// （DIMBLK の 342、DIMBLK1・DIMBLK2 の 343・344、DIMLDRBLK の 341）を名前にし、0 かなければ既定の矢印と
    /// する。両端を別にする（DIMSAH の 173）のでなければ、両端とも 342 にする。値がなければ DXF の既定の値
    /// （DIMSCALE の 40 は 1、DIMASZ の 41 は 0.18、DIMTSZ の 142 は 0）とする（推定。docs/formats/dxf.md）
    fn dimension_style_record(
        &self,
        db: &AcadDb,
        object: &Object,
        source: Ref,
        out: &mut Out,
    ) -> Lifted<()> {
        let kind = object.kind.as_slice();
        let name = self.string(object, 2)?.ok_or_else(|| invalid(kind, 2))?;
        let arrow = |code: i16| -> Lifted<Arrowhead> {
            Ok(self
                .block_key(db, object, code)?
                .map_or(Arrowhead::Default, Arrowhead::Symbol))
        };
        let separate = match object.first(173) {
            None => false,
            Some(Data::Int(flag)) => *flag != 0,
            Some(Data::Bool(flag)) => *flag,
            Some(_) => return Err(invalid(kind, 173)),
        };
        let ends = if separate {
            [arrow(343)?, arrow(344)?]
        } else {
            let both = arrow(342)?;
            [both.clone(), both]
        };
        let non_negative = |code: i16, default: f64| -> Lifted<f64> {
            Some(self.real(object, code, Some(default))?)
                .filter(|value| *value >= 0.0)
                .ok_or_else(|| invalid(kind, code))
        };
        let tick = non_negative(142, 0.0)?;
        out.dimension_styles.push(DimensionStyle {
            name,
            ends,
            leader: arrow(341)?,
            size: non_negative(41, 0.18)?,
            tick: (tick > 0.0).then_some(tick),
            scale: non_negative(40, 1.0)?,
            source,
        });
        Ok(())
    }

    /// 画層を足す。同じ名前がすでにあれば、名前で引くのは先のものにする
    fn add_layer(
        &self,
        kind: &[u8],
        name: String,
        defined: Option<(LayerState, LayerStyle)>,
        source: Ref,
        out: &mut Out,
    ) -> Lifted<LayerId> {
        let id = LayerId(u32::try_from(out.layers.len()).map_err(|_| invalid(kind, 8))?);
        out.by_name.entry(name.to_ascii_lowercase()).or_insert(id);
        out.layers.push(Layer {
            id,
            // ブロックの中のレイヤ 0 の図形は、配置のレイヤに従う（決定 0143）
            follows_placement: name == "0",
            name,
            state: defined.map(|(state, _)| state),
            style: defined.map(|(_, style)| style),
            source,
        });
        Ok(id)
    }

    /// モデル空間とペーパー空間の SORTENTSTABLE と、ほかのブロックのレコードのハンドルごとの SORTENTSTABLE
    fn sort_keys(&self, db: &AcadDb, cx: &mut Cx) -> Result<SpaceKeys, Error> {
        let mut keys = [None, None];
        let mut blocks: BTreeMap<u64, SortKeys> = BTreeMap::new();
        for object in db.objects() {
            if object.section != Section::Objects || object.kind != b"SORTENTSTABLE" {
                continue;
            }
            let source = source(object);
            work(cx, 1, &source)?;
            match self.sortents(db, object) {
                Ok((record, space, sort)) => {
                    work(cx, sort.len() as u64, &source)?;
                    let all = match space {
                        Some(index) => keys.get_mut(index).map(|slot| {
                            &mut slot.get_or_insert_with(|| (source, BTreeMap::new())).1
                        }),
                        None => Some(
                            &mut blocks
                                .entry(record)
                                .or_insert_with(|| (source, BTreeMap::new()))
                                .1,
                        ),
                    };
                    if let Some(all) = all {
                        all.extend(sort);
                    }
                }
                Err(diagnostic) => cx.report((*diagnostic).at(source)),
            }
        }
        Ok((keys, blocks))
    }

    /// SORTENTSTABLE の、指すブロックのレコードのハンドルと空間（0 はモデル空間、1 はペーパー空間、ほかは
    /// `None`）と、図形のハンドル（331）から並べ替えのハンドル（5）への対応（DXF Reference「SORTENTSTABLE」）
    fn sortents(
        &self,
        db: &AcadDb,
        object: &Object,
    ) -> Lifted<(u64, Option<usize>, BTreeMap<u64, u64>)> {
        let kind = object.kind.as_slice();
        // DWG の表は区切りの 100 を持たないので、所有者（初めの 330）の後から読む
        let start = object
            .values
            .iter()
            .position(|value| {
                matches!((value.code, &value.data), (100, Data::Text(text)) if text == b"AcDbSortentsTable")
            })
            .or_else(|| match object.origin {
                Origin::Dwg { .. } => object.values.iter().position(|value| value.code == 330),
                _ => None,
            })
            .ok_or_else(|| invalid(kind, 100))?;
        let mut rest = object.values.iter().skip(start + 1);
        let (record, block) = match rest.next() {
            Some(Value {
                code: 330,
                data: Data::Handle(handle),
            }) => (
                *handle,
                db.get(*handle)
                    .filter(|record| record.kind == b"BLOCK_RECORD")
                    .ok_or_else(|| invalid(kind, 330))?,
            ),
            _ => return Err(invalid(kind, 330)),
        };
        let name = self.string(block, 2)?.unwrap_or_default();
        let space = ["*Model_Space", "*Paper_Space"]
            .iter()
            .position(|space| name.eq_ignore_ascii_case(space));
        let mut sort = BTreeMap::new();
        while let Some(value) = rest.next() {
            let (331, Data::Handle(entity)) = (value.code, &value.data) else {
                return Err(invalid(kind, value.code));
            };
            let Some(Value {
                code: 5,
                data: Data::Handle(key),
            }) = rest.next()
            else {
                return Err(invalid(kind, 5));
            };
            sort.insert(*entity, *key);
        }
        Ok((record, space, sort))
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

/// 線種の列の 1 つの要素の、形か文字の値（DXF Reference「LTYPE」の 74〜9）
#[derive(Default)]
struct Dash {
    flags: i64,
    number: i64,
    style: u64,
    scale: f64,
    rotation: f64,
    offset: [f64; 2],
    text: Option<String>,
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

/// 図面モデルの値にない図形の欄（決定 0142）。厚さ（39）は、平面の図形を Z の向きに押し出す長さで、上から
/// 見た形を変えないので、0 でなければ `thickness` に控える。LWPOLYLINE・POLYLINE の線種の生成（70 の 128）は、
/// 線種の模様を頂点ごとに切らずに続けることで、立っていれば `line_type_generation` を 1 とする
fn entity_fields(object: &Object) -> Lifted<Vec<Field>> {
    let mut fields = Vec::new();
    let real = |code: i16| match object.first(code) {
        Some(Data::Real(value)) if value.is_finite() => Ok(Some(*value)),
        None => Ok(None),
        Some(_) => Err(invalid(&object.kind, code)),
    };
    let thick = matches!(
        object.kind.as_slice(),
        b"LINE"
            | b"CIRCLE"
            | b"ARC"
            | b"TEXT"
            | b"LWPOLYLINE"
            | b"POLYLINE"
            | b"SOLID"
            | b"TRACE"
            | b"POINT"
            | b"SHAPE"
    );
    if thick
        && let Some(thickness) = real(39)?
        && thickness != 0.0
    {
        fields.push(Field {
            key: "thickness",
            value: FieldValue::Real(thickness),
        });
    }
    if matches!(object.kind.as_slice(), b"LWPOLYLINE" | b"POLYLINE")
        && matches!(object.first(70), Some(Data::Int(flags)) if flags & 128 != 0)
    {
        fields.push(Field {
            key: "line_type_generation",
            value: FieldValue::Integer(1),
        });
    }
    if object.kind == b"SPLINE" {
        let tolerances = [
            (42, "knot_tolerance"),
            (43, "control_tolerance"),
            (44, "fit_tolerance"),
        ];
        for (code, key) in tolerances {
            if let Some(tolerance) = real(code)? {
                fields.push(Field {
                    key,
                    value: FieldValue::Real(tolerance),
                });
            }
        }
    }
    Ok(fields)
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// DICTIONARY（`handle`）の、名前が `name` の項目のハンドル
fn entry(db: &AcadDb, handle: u64, name: &[u8]) -> Option<u64> {
    let dictionary = db.get(handle).filter(|found| found.kind == b"DICTIONARY")?;
    dictionary.values.windows(2).find_map(|pair| match pair {
        [
            Value {
                code: 3,
                data: Data::Text(text),
            },
            Value {
                code: 350 | 360,
                data: Data::Handle(item),
            },
        ] if text == name => Some(*item),
        _ => None,
    })
}

/// 図形の拡張辞書のハンドル（`{ACAD_XDICTIONARY` の組の 360）
fn xdictionary(object: &Object) -> Option<u64> {
    let mut inside = false;
    for value in &object.values {
        match (value.code, &value.data) {
            (102, Data::Text(text)) if text == b"{ACAD_XDICTIONARY" => inside = true,
            (102, Data::Text(text)) if text.starts_with(b"}") => inside = false,
            (360, Data::Handle(handle)) if inside => return Some(*handle),
            _ => {}
        }
    }
    None
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

/// MULTILEADER の値の区切り
struct CalloutParts {
    /// 図形の値
    entity: Vec<Value>,
    /// 注記の文脈の値
    context: Vec<Value>,
    /// 枝ごとの、枝の値と線ごとの値
    leaders: Vec<(Vec<Value>, Vec<Vec<Value>>)>,
}

/// セルか表のスタイルの組の、文字の色と枠の辺ごとの色・線幅・表示のグループ・コード
struct BorderCodes {
    text_color: i16,
    top: [i16; 3],
    right: [i16; 3],
    bottom: [i16; 3],
    left: [i16; 3],
    horizontal: Option<[i16; 3]>,
    vertical: Option<[i16; 3]>,
}

/// セルの枠（DXF Reference「ACAD_TABLE」）。上・右・下・左の枠を持つ。左と上と右は、試料の表を描いたブロックの
/// 線の色と線幅で確かめた。下は推定。表示（285〜289）は、DXF が書かない
const CELL_BORDERS: BorderCodes = BorderCodes {
    text_color: 64,
    top: [69, 279, 289],
    right: [65, 275, 285],
    bottom: [66, 276, 286],
    left: [68, 278, 288],
    horizontal: None,
    vertical: None,
};

/// 表のスタイルの組の枠（DXF Reference「TABLESTYLE」。推定）
const STYLE_BORDERS: BorderCodes = BorderCodes {
    text_color: 62,
    top: [64, 274, 284],
    right: [69, 279, 289],
    bottom: [66, 276, 286],
    left: [67, 277, 287],
    horizontal: Some([65, 275, 285]),
    vertical: Some([68, 278, 288]),
};

/// 表のセルの値（301 から 304 まで）のうち、`Lifter::cell_value` が移さない型（64・128・256・512）で、値を持つ
/// （印の 93 の 1 が立たない）ものを診断する
fn report_cell_values(object: &Object, cx: &mut Cx) {
    let mut inside = false;
    let (mut flags, mut kind) = (0, 0);
    for value in &object.values {
        match (value.code, &value.data) {
            (301, _) => (inside, flags, kind) = (true, 0, 0),
            (93, Data::Int(value)) if inside => flags = *value,
            (90, Data::Int(value)) if inside => kind = *value,
            (304, _) if inside => {
                inside = false;
                if flags & 1 == 0 && matches!(kind, 64 | 128 | 256 | 512) {
                    cx.report(
                        Diagnostic::new(codes::DXF_CELL_VALUE_DROPPED)
                            .with("type", kind)
                            .at(source(object)),
                    );
                }
            }
            _ => {}
        }
    }
}

/// SYSTEMTIME（年・月・曜日・日・時・分・秒・ミリ秒の、リトル・エンディアンの 16 ビットの並び）。範囲の外なら `None`
fn system_time(bytes: &[u8]) -> Option<DateTime> {
    let ([year, month, _, day, hour, minute, second, millisecond], []) = bytes.as_chunks::<2>()
    else {
        return None;
    };
    let small = |pair: &[u8; 2], range: core::ops::RangeInclusive<u8>| {
        u8::try_from(u16::from_le_bytes(*pair))
            .ok()
            .filter(|value| range.contains(value))
    };
    Some(DateTime {
        year: u16::from_le_bytes(*year),
        month: small(month, 1..=12)?,
        day: small(day, 1..=31)?,
        hour: small(hour, 0..=23)?,
        minute: small(minute, 0..=59)?,
        second: small(second, 0..=59)?,
        millisecond: Some(u16::from_le_bytes(*millisecond)).filter(|ms| *ms < 1000)?,
    })
}

/// MULTILEADER の値を、区切り（300 `CONTEXT_DATA{`・301・302 `LEADER{`・303・304 `LEADER_LINE{`・305）で
/// 分ける。文脈の中の 304 は、`LEADER_LINE{` でなければ文字の内容とする。区切りが閉じなければ読めないとする
fn split_callout(object: &Object) -> Lifted<CalloutParts> {
    let kind = object.kind.as_slice();
    let mut parts = CalloutParts {
        entity: Vec::new(),
        context: Vec::new(),
        leaders: Vec::new(),
    };
    // 0 は図形、1 は文脈、2 は枝、3 は線の中
    let mut depth = 0;
    let mut seen = false;
    for value in &object.values {
        let marker = |text: &[u8]| matches!(&value.data, Data::Text(t) if t.as_slice() == text);
        match (depth, value.code) {
            (0, 300) if !seen => {
                depth = 1;
                seen = true;
            }
            (1, 301) => depth = 0,
            (1, 302) if marker(b"LEADER{") => {
                parts.leaders.push((Vec::new(), Vec::new()));
                depth = 2;
            }
            (2, 303) => depth = 1,
            (2, 304) if marker(b"LEADER_LINE{") => {
                if let Some((_, lines)) = parts.leaders.last_mut() {
                    lines.push(Vec::new());
                }
                depth = 3;
            }
            (3, 305) => depth = 2,
            (0, _) => parts.entity.push(value.clone()),
            (1, _) => parts.context.push(value.clone()),
            (2, _) => {
                if let Some((leader, _)) = parts.leaders.last_mut() {
                    leader.push(value.clone());
                }
            }
            _ => {
                if let Some(line) = parts.leaders.last_mut().and_then(|(_, l)| l.last_mut()) {
                    line.push(value.clone());
                }
            }
        }
    }
    match (seen, depth) {
        (false, _) => Err(invalid(kind, 300)),
        (true, 0) => Ok(parts),
        (true, _) => Err(invalid(kind, 301)),
    }
}

/// 枠への位置合わせ。1〜9 は、上・中央・下の行の、左・中央・右
fn attachment(kind: &[u8], value: i64, code: i16) -> Lifted<(Horizontal, Vertical)> {
    if !(1..=9).contains(&value) {
        return Err(invalid(kind, code));
    }
    let vertical = match (value - 1) / 3 {
        0 => Vertical::Top,
        1 => Vertical::Middle,
        _ => Vertical::Bottom,
    };
    let horizontal = match (value - 1) % 3 {
        0 => Horizontal::Left,
        1 => Horizontal::Center,
        _ => Horizontal::Right,
    };
    Ok((horizontal, vertical))
}

/// 書き進める向き。1 は左から右、3 は上から下、5 は文字スタイルに従う
fn flow(kind: &[u8], value: i64, code: i16) -> Lifted<Flow> {
    match value {
        1 => Ok(Flow::LeftToRight),
        3 => Ok(Flow::TopToBottom),
        5 => Ok(Flow::ByStyle),
        _ => Err(invalid(kind, code)),
    }
}

fn columns() -> Box<Diagnostic> {
    Box::new(Diagnostic::new(codes::DXF_MTEXT_COLUMNS_UNSUPPORTED))
}

fn non_planar(kind: &[u8]) -> Box<Diagnostic> {
    Box::new(Diagnostic::new(codes::DXF_NON_PLANAR_DROPPED).with("kind", text(kind)))
}
