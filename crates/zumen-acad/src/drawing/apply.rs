//! 図面モデルの変更を、共有の表を通して DXF・DWG に書き戻す（決定 0139）。

//!
//! このファイルは、索引の作成・変更の検査と計画・表への反映を持つ。種類ごとの変更の組み立ては、子のモジュールに分けて置く。
//!
//! - `add`: 実寸の空間と用紙への新しい図形の追加
//! - `replace`: 図形の置き換えで変わる形のフィールド
//! - `callout`: 引出線つきの注記（MULTILEADER）の置き換えと変換、および形の基準点
//! - `transform`: スタイルの変更・座標変換・記号の属性の書き換え・動く拡張データ
//! - `layer`・`line_type`・`text_style`・`dimension_style`・`owned`: 表のレコードの追加・置き換え・削除
//! - `new_dxf`: 新しい DXF の画層の拡張データと表の件数
use std::collections::{BTreeMap, BTreeSet};

use super::{BLOCK, Charset, Home, MODEL, PaperFrame, attached, framed, mtext, source, text, work};
use add::{Born, Target, kind_of};

mod add;
mod callout;
mod dimension_style;
mod layer;
mod line_type;
mod new_dxf;
mod owned;
mod replace;
#[cfg(test)]
mod tests;
mod text_style;
mod transform;
use crate::db::{AcadDb, Data, Object, Section, Value};
use crate::{Dwg, DwgFile, Dxf, DxfFile};
use zumen_base::{Cx, Diagnostic, Diagnostics, Key, Ref, codes};
use zumen_drawing::{
    Affine, Applied, Callout, Changes, DimensionStyle, Drawing2D, Edge, Edit, Element,
    FormattedText, Hash, Horizontal, KnotParameterization, Layer, LayerId, Leader, Length,
    LineType, MarkContent, ModelLength, ModelPoint, PaperLength, PaperPoint, Placed, Placement,
    PointMark, PointSize, PointStyle, Polyline, Restyle, Rotation, Seen, Shape, SheetId, SpaceId,
    Spline, Style, Symbol, SymbolDefinition, Text, TextStyle, Vertical, id,
};
use zumen_sdk::{Apply, Codec, Error};

use self::callout::*;
use self::replace::*;
use self::transform::*;

impl Apply<Changes> for DxfFile {
    type Output = Vec<Applied>;

    /// 今のファイルから表を作り、変更を当ててから書き戻す
    fn apply(&mut self, changes: &Changes, cx: &mut Cx) -> Result<Vec<Applied>, Error> {
        let current = document(self, changes, Dxf::write, cx)?;
        let mut db = self.table(cx, false)?;
        let ids = edit(&mut db, changes, Target::Dxf, cx)?;
        if !current {
            return Err(Error::Rejected);
        }
        let result = settled(&db, ids, cx)?;
        self.write_back(&db, cx)?;
        Ok(result)
    }
}

impl Apply<Changes> for DwgFile {
    type Output = Vec<Applied>;

    /// 今のファイルから表を作り、変更を当ててから書き戻す
    fn apply(&mut self, changes: &Changes, cx: &mut Cx) -> Result<Vec<Applied>, Error> {
        if self.version() == b"AC1021" {
            cx.report(Diagnostic::new(codes::DWG_EDIT_UNSUPPORTED).with("version", "AC1021"));
            return Err(Error::Unsupported);
        }
        let current = document(self, changes, Dwg::write, cx)?;
        let mut db = self.objects(cx)?;
        let seed = self.handle_seed();
        let ids = edit(&mut db, changes, Target::Dwg(seed), cx)?;
        if !current {
            return Err(Error::Rejected);
        }
        // 書き戻し後の検査が止まっても、元のファイルは変えない。
        let mut updated = self.snapshot(cx)?;
        updated.write_back(&db, cx)?;
        // DWG は角度をラジアンで持つので、表の度の値は、書いて読み直すと末の桁がずれうる。ハッシュは、
        // 書き戻したファイルから作り直した表で求める
        let mut quiet = Diagnostics::new();
        let written = updated.objects(&mut Cx::new(cx.budget, &mut quiet, cx.cancel))?;
        let result = settled(&written, ids, cx)?;
        *self = updated;
        Ok(result)
    }
}

/// 変更の一覧が文書のハッシュを持てば、今のファイルを書いたバイト列のハッシュと比べる。違えば診断を足して
/// `false` を返す。ほかの変更も確かめるので、ここでは止めない。変えていないファイルは元のバイトのまま書くので、
/// 読んだバイト列のハッシュと同じになる
fn document<F>(
    file: &F,
    changes: &Changes,
    write: fn(&F, &mut Vec<u8>, &mut Cx) -> Result<(), Error>,
    cx: &mut Cx,
) -> Result<bool, Error> {
    let Some(hash) = changes.document else {
        return Ok(true);
    };
    let mut bytes = Vec::new();
    write(file, &mut bytes, cx)?;
    let current = Hash::document_checked(&bytes, cx).map_err(|stop| {
        cx.report(stop.to_diagnostic());
        Error::Stopped(stop)
    })? == hash;
    if !current {
        cx.report(Diagnostic::new(codes::CHANGE_DOCUMENT_STALE));
    }
    Ok(current)
}

/// ID が指す要素
#[derive(Clone, Copy)]
enum Found<'a> {
    Model(&'a Element<ModelPoint>),
    Paper(&'a Element<PaperPoint>),
}

/// 足す先
#[derive(Clone, Copy, PartialEq)]
enum Place {
    Space(SpaceId),
    Sheet(SheetId),
}

/// ID から要素と足す先への索引（決定 0156）。図面モデルを 1 回なめて作る
struct Index<'a> {
    elements: BTreeMap<String, Found<'a>>,
    /// ハンドルから書き戻した後の要素 ID。複数の要素を作ったハンドルは `None`
    element_handles: BTreeMap<u64, Option<String>>,
    /// レイヤ（決定 0162）
    layers: BTreeMap<String, &'a Layer>,
    /// 文字スタイル（決定 0163）
    text_styles: BTreeMap<String, &'a TextStyle>,
    /// 線種（決定 0164）
    line_types: BTreeMap<String, &'a LineType>,
    /// 寸法スタイル（決定 0165）
    dimension_styles: BTreeMap<String, &'a DimensionStyle>,
    places: BTreeMap<String, Place>,
    /// 記号の定義の空間から、その定義（決定 0177）
    definitions: BTreeMap<SpaceId, &'a SymbolDefinition>,
    /// 記号の定義の名前（小文字）から、その中に置く記号の定義の名前（小文字）。定義の入れ子の輪を見つける
    /// （#333）
    nests: Nests,
    /// 2 つ以上の要素を作った出所。1 つの要素だけを書き戻せない
    shared: BTreeSet<&'a Ref>,
}

impl<'a> Index<'a> {
    fn new(drawing: &'a Drawing2D, cx: &mut Cx<'_>) -> Result<Self, Error> {
        let mut elements = BTreeMap::new();
        let mut element_handles = BTreeMap::new();
        let mut places = BTreeMap::new();
        let mut seen = BTreeSet::new();
        let mut shared = BTreeSet::new();
        let mut note = |source: &'a Ref| {
            if *source != Ref::default() && !seen.insert(source) {
                shared.insert(source);
            }
        };
        for space in &drawing.spaces {
            work(cx, 1, &Ref::default())?;
            let place = id::space(space.id);
            let mut namer = id::Namer::new(&place);
            for (index, element) in space.elements.iter().enumerate() {
                work(cx, 1, &element.source)?;
                note(&element.source);
                let id = namer.next(index, element);
                remember_handle(&mut element_handles, &element.source, &id);
                elements.insert(id, Found::Model(element));
            }
            places.insert(place, Place::Space(space.id));
        }
        // 記号の定義の中の要素も ID で指せる。定義の空間にも足せる（決定 0177）
        let mut definitions = id::DefinitionNamer::default();
        let mut spaces = BTreeMap::new();
        let mut nests = Nests::new();
        for definition in &drawing.symbols {
            spaces.insert(definition.space.id, definition);
            work(cx, 1, &definition.source)?;
            let place = definitions.next(definition);
            let mut namer = id::Namer::new(&place);
            let mut nested = (definition.key.namespace == BLOCK)
                .then(|| nests.entry(definition.key.name.to_lowercase()).or_default());
            for (index, element) in definition.space.elements.iter().enumerate() {
                work(cx, 1, &element.source)?;
                if let (Some(nested), Shape::Symbol(symbol)) =
                    (nested.as_deref_mut(), &element.shape)
                    && symbol.key.namespace == BLOCK
                {
                    nested.insert(symbol.key.name.to_lowercase());
                }
                note(&element.source);
                let id = namer.next(index, element);
                remember_handle(&mut element_handles, &element.source, &id);
                elements.insert(id, Found::Model(element));
            }
            places.insert(place, Place::Space(definition.space.id));
        }
        let mut sheet_namer = id::SheetNamer::default();
        for sheet in &drawing.sheets {
            work(cx, 1, &sheet.source)?;
            let place = sheet_namer.next(sheet);
            let mut namer = id::Namer::new(&place);
            for (index, element) in sheet.elements.iter().enumerate() {
                work(cx, 1, &element.source)?;
                note(&element.source);
                let id = namer.next(index, element);
                remember_handle(&mut element_handles, &element.source, &id);
                elements.insert(id, Found::Paper(element));
            }
            places.insert(place, Place::Sheet(sheet.id));
        }
        let mut layers = BTreeMap::new();
        for layer in &drawing.layers {
            work(cx, 1, &layer.source)?;
            layers.entry(id::layer(layer)).or_insert(layer);
        }
        let mut text_styles = BTreeMap::new();
        for (index, style) in drawing.text_styles.iter().enumerate() {
            work(cx, 1, &style.source)?;
            text_styles
                .entry(id::text_style(style, index))
                .or_insert(style);
        }
        let mut line_types = BTreeMap::new();
        for line_type in &drawing.line_types {
            work(cx, 1, &line_type.source)?;
            line_types
                .entry(id::line_type(line_type))
                .or_insert(line_type);
        }
        let mut dimension_styles = BTreeMap::new();
        for (index, style) in drawing.dimension_styles.iter().enumerate() {
            work(cx, 1, &style.source)?;
            dimension_styles
                .entry(id::dimension_style(style, index))
                .or_insert(style);
        }
        Ok(Self {
            elements,
            element_handles,
            layers,
            text_styles,
            line_types,
            dimension_styles,
            places,
            definitions: spaces,
            nests,
            shared,
        })
    }

    /// ID の要素の、今のハッシュ
    fn hash(&self, id: &str, cx: &mut Cx<'_>) -> Result<Option<Hash>, Error> {
        let Some(found) = self.elements.get(id) else {
            if let Some(style) = self.text_styles.get(id) {
                work(cx, 1, &style.source)?;
                return Ok(Some(Hash::text_style(style)));
            }
            if let Some(style) = self.dimension_styles.get(id) {
                return Hash::dimension_style_checked(style, cx)
                    .map(Some)
                    .map_err(|stop| {
                        cx.report(stop.to_diagnostic().at(style.source.clone()));
                        Error::Stopped(stop)
                    });
            }
            if let Some(line_type) = self.line_types.get(id) {
                return Hash::line_type_checked(line_type, cx)
                    .map(Some)
                    .map_err(|stop| {
                        cx.report(stop.to_diagnostic().at(line_type.source.clone()));
                        Error::Stopped(stop)
                    });
            }
            let Some(layer) = self.layers.get(id) else {
                return Ok(None);
            };
            return Hash::layer_checked(layer, cx).map(Some).map_err(|stop| {
                cx.report(stop.to_diagnostic().at(layer.source.clone()));
                Error::Stopped(stop)
            });
        };
        let (result, source) = match found {
            Found::Model(element) => (Hash::element_checked(element, cx), &element.source),
            Found::Paper(element) => (Hash::element_checked(element, cx), &element.source),
        };
        result.map(Some).map_err(|stop| {
            cx.report(stop.to_diagnostic().at(source.clone()));
            Error::Stopped(stop)
        })
    }
}

/// 要素のハンドルと ID を控える。同じ出所から複数の要素ができたときは、曖昧なので選ばない
fn remember_handle(handles: &mut BTreeMap<u64, Option<String>>, source: &Ref, id: &str) {
    if let Some(Key::Handle(handle)) = source.key() {
        handles
            .entry(handle)
            .and_modify(|id| *id = None)
            .or_insert_with(|| Some(id.to_owned()));
    }
}

/// 書き戻した後に、変更の結果を探すキー
#[derive(Debug)]
enum ResultTarget {
    /// 図形は用紙の ID が変わっても、ハンドルから探せる
    Element(Option<u64>),
    /// 表の項目（レイヤ・文字スタイル）は表の ID で探す
    Table(Option<String>),
    Removed,
    /// 文書の設定（点の表し方）。ID もハッシュも持たない
    Document,
}

/// 当てた変更の、書き戻した後の ID とハッシュ。キー（ハンドル）を持たない要素の ID は、読み直すとずれうる
/// ので返さない
fn applied(index: &Index, id: Option<String>, cx: &mut Cx<'_>) -> Result<Applied, Error> {
    let keyed = |id: &String| {
        index.elements.get(id).is_some_and(|found| {
            match found {
                Found::Model(element) => &element.source,
                Found::Paper(element) => &element.source,
            }
            .key()
            .is_some()
        }) || index
            .layers
            .get(id)
            .is_some_and(|layer| layer.source.key().is_some())
            || index
                .text_styles
                .get(id)
                .is_some_and(|style| style.source.key().is_some())
            || index
                .line_types
                .get(id)
                .is_some_and(|line_type| line_type.source.key().is_some())
            || index
                .dimension_styles
                .get(id)
                .is_some_and(|style| style.source.key().is_some())
    };
    let Some(id) = id.filter(keyed) else {
        return Ok(Applied {
            id: None,
            hash: None,
        });
    };
    let Some(hash) = index.hash(&id, cx)? else {
        return Ok(Applied {
            id: None,
            hash: None,
        });
    };
    Ok(Applied {
        id: Some(id),
        hash: Some(hash),
    })
}

/// 当てられない変更の診断
fn unsupported(edit: &str, kind: &str, target: Target) -> Diagnostic {
    let format = match target {
        Target::Dxf => "DXF",
        Target::Dwg(_) => "DWG",
    };
    Diagnostic::new(codes::CHANGE_EDIT_UNSUPPORTED)
        .with("edit", edit)
        .with("kind", kind)
        .with("format", format)
}

/// 変更を表に当てる。すべての変更を確かめ、当てられない変更が 1 つでもあれば、表を変えずに失敗する。
/// `target` は書き戻す先。当てたら、変更ごとの新しい ID を返す（除いた変更は `None`）
fn edit(
    db: &mut AcadDb,
    changes: &Changes,
    target: Target,
    cx: &mut Cx,
) -> Result<Vec<ResultTarget>, Error> {
    unique_handles(db, target, cx)?;
    let positions = object_positions(db, cx)?;
    // 今の図面モデル。変換の診断は、書き戻しの診断に混ぜない
    let mut quiet = Diagnostics::new();
    let (drawing, frames) = framed(db, &mut Cx::new(cx.budget, &mut quiet, cx.cancel))?;
    let index = Index::new(&drawing, cx)?;
    // 2007 からのバージョンは文字を UTF-8 で持つ。それより前は、ヘッダーのコード・ページで符号化する（読む側と同じ、
    // 決定 0192）
    let charset = super::charset_of(db);
    let born = Born::new(db, &drawing, charset, target);
    let mut plan = Plan::new(db, target);
    let current = Current {
        db,
        drawing: &drawing,
        index: &index,
        born: &born,
        positions: &positions,
        frames: &frames,
        edits: &changes.edits,
        charset,
        encoder: born.encoder(),
        target,
    };
    for change in &changes.edits {
        let rejected = std::mem::take(&mut plan.rejected);
        plan.check(&current, change, cx)?;
        // コード・ページで表せない文字のために拒んだなら、その文字を返す（決定 0192）
        if let Some(character) = born.encoder().take()
            && plan.rejected
        {
            cx.report(
                Diagnostic::new(codes::CHANGE_CHARACTER_UNSUPPORTED)
                    .with("character", format!("U+{:04X}", u32::from(character))),
            );
        }
        plan.rejected |= rejected;
    }
    if plan.rejected {
        return Err(Error::Rejected);
    }
    let Plan {
        replaced,
        mut removed,
        added,
        layers,
        text_styles,
        line_types,
        dimension_styles,
        variables,
        ..
    } = plan;
    count_removals(db, &replaced, &removed, cx)?;
    let insertion_work = born.insertion_work(&added, db.objects().len());
    if insertion_work > 0 {
        work(cx, insertion_work, &Ref::default())?;
    }
    for (position, values) in replaced {
        db.object_mut(position).ok_or(Error::Rejected)?.values = values;
    }
    removed.sort_unstable();
    removed.dedup();
    for position in removed.into_iter().rev() {
        db.remove(position);
    }
    let handles = born.push(db, added);
    let layer_handles = layers.write_back(db, target, cx)?;
    let style_handles = text_styles.write_back(db, target, cx)?;
    let line_type_handles = line_types.write_back(db, target, cx)?;
    let dimension_style_handles = dimension_styles.write_back(db, target, cx)?;
    for (name, values) in variables {
        db.set_variable(name, values);
    }
    Ok(results(
        changes,
        &index,
        handles,
        layer_handles,
        style_handles,
        line_type_handles,
        dimension_style_handles,
    ))
}

/// ハンドルが重複すると、参照や ID が別のオブジェクトを指しうる。書き戻しは拒否する。
fn unique_handles(db: &AcadDb, target: Target, cx: &mut Cx) -> Result<(), Error> {
    let mut handles = BTreeSet::new();
    for object in db.objects() {
        work(cx, 1, &source(object))?;
        if let Some(handle) = object.handle()
            && handle != 0
            && !handles.insert(handle)
        {
            cx.report(unsupported("apply", "duplicate_handle", target).at(source(object)));
            return Err(Error::Rejected);
        }
    }
    Ok(())
}

/// 書き戻す文字の符号化。2007 より前のバージョンのコード・ページで表せない文字は書かず、最初のものを覚えておく
/// （決定 0192）
pub(super) struct Encoder {
    charset: Charset,
    unencodable: std::cell::Cell<Option<char>>,
}

impl Encoder {
    pub(super) fn new(charset: Charset) -> Self {
        Self {
            charset,
            unencodable: std::cell::Cell::new(None),
        }
    }

    /// 文字の値を符号化する。表せない文字があれば `None`
    pub(super) fn encode(&self, value: &str) -> Option<Vec<u8>> {
        match self.charset.encode(value) {
            Ok(bytes) => Some(bytes),
            Err(character) => {
                if self.unencodable.get().is_none() {
                    self.unencodable.set(Some(character));
                }
                None
            }
        }
    }

    /// 覚えておいた、表せない文字を取り出す
    fn take(&self) -> Option<char> {
        self.unencodable.take()
    }
}

/// 変更を確かめる間に読む、今の表と図面モデル
struct Current<'a> {
    db: &'a AcadDb,
    drawing: &'a Drawing2D,
    index: &'a Index<'a>,
    born: &'a Born,
    positions: &'a BTreeMap<Ref, usize>,
    frames: &'a BTreeMap<SheetId, PaperFrame>,
    /// 変更の一覧。除く項目を、同じ一覧のほかの変更が指していないかを調べる
    edits: &'a [Edit],
    /// 文字の値の符号化。名前を比べるときに使う
    charset: Charset,
    /// 書き戻す文字の符号化
    encoder: &'a Encoder,
    target: Target,
}

/// 確かめた変更から作る、表への書き込みの控え。拒んだ変更が 1 つでもあれば、表を変えない
struct Plan<'e> {
    replaced: Vec<(usize, Vec<Value>)>,
    removed: Vec<usize>,
    added: Vec<add::Record>,
    added_records: u64,
    rejected: bool,
    /// 変えた項目の ID。同じ項目は一度だけ変えられる
    targets: BTreeSet<&'e str>,
    attribute_counts: Option<BTreeMap<usize, usize>>,
    layers: layer::Layers,
    text_styles: text_style::TextStyles,
    line_types: line_type::LineTypes,
    dimension_styles: dimension_style::DimensionStyles,
    /// この変更の一覧で記号の定義の中に足した記号。[`Index::nests`] と同じ形（#333）
    nests: Nests,
    /// 書き直すヘッダーの変数（`$PDMODE`・`$PDSIZE`、決定 0190）
    variables: Vec<(&'static [u8], Vec<Value>)>,
}

/// 記号の定義の名前（小文字）から、その中に置く記号の定義の名前（小文字）
type Nests = BTreeMap<String, BTreeSet<String>>;

impl<'e> Plan<'e> {
    fn new(db: &AcadDb, target: Target) -> Self {
        Self {
            replaced: Vec::new(),
            removed: Vec::new(),
            added: Vec::new(),
            added_records: 0,
            rejected: false,
            targets: BTreeSet::new(),
            attribute_counts: None,
            layers: layer::Layers::default(),
            text_styles: text_style::TextStyles::default(),
            line_types: line_type::LineTypes::default(),
            dimension_styles: dimension_style::DimensionStyles::new(db, target),
            nests: Nests::new(),
            variables: Vec::new(),
        }
    }

    /// 変更を 1 つ確かめて控える。当てられない変更は、診断を足して拒む
    fn check(&mut self, current: &Current<'_>, change: &'e Edit, cx: &mut Cx) -> Result<(), Error> {
        match change {
            Edit::ReplaceLayer {
                target: target_of,
                layer: new,
            } => layer::replace(current, self, target_of, new, cx),
            Edit::RemoveLayer { target: target_of } => layer::remove(current, self, target_of, cx),
            Edit::AddLayer { layer: new } => layer::add(current, self, new, cx),
            Edit::ReplaceLineType {
                target: target_of,
                line_type: new,
            } => line_type::replace(current, self, target_of, new, cx),
            Edit::RemoveLineType { target: target_of } => {
                line_type::remove(current, self, target_of, cx)
            }
            Edit::AddLineType { line_type: new } => line_type::add(current, self, new, cx),
            Edit::ReplaceDimensionStyle {
                target: target_of,
                style: new,
            } => dimension_style::replace(current, self, target_of, new, cx),
            Edit::RemoveDimensionStyle { target: target_of } => {
                dimension_style::remove(current, self, target_of, cx)
            }
            Edit::AddDimensionStyle { style: new } => dimension_style::add(current, self, new, cx),
            Edit::ReplaceTextStyle {
                target: target_of,
                style: new,
            } => text_style::replace(current, self, target_of, new, cx),
            Edit::RemoveTextStyle { target: target_of } => {
                text_style::remove(current, self, target_of, cx)
            }
            Edit::AddTextStyle { style: new } => text_style::add(current, self, new, cx),
            Edit::Add { place, element } => add_element(current, self, place, element, cx),
            Edit::Replace {
                target: target_of, ..
            } => change_element(current, self, change, "replace", target_of, cx),
            Edit::Restyle {
                target: target_of, ..
            } => change_element(current, self, change, "restyle", target_of, cx),
            Edit::Transform {
                target: target_of, ..
            } => change_element(current, self, change, "transform", target_of, cx),
            Edit::Remove { target: target_of } => {
                change_element(current, self, change, "remove", target_of, cx)
            }
            Edit::SetPointStyle { style } => {
                set_point_style(current, self, style, cx);
                Ok(())
            }
        }
    }

    /// 当てられない変更の診断を足し、表を変えないようにする
    fn reject(&mut self, diagnostic: Diagnostic, cx: &mut Cx) {
        cx.report(diagnostic);
        self.rejected = true;
    }

    /// 変更が指す項目を `items` から引き、読んだときのハッシュと今のハッシュを比べる。項目がない・ハッシュが
    /// 違うときは、診断を足して拒み、`None` を返す
    fn seen<V: Copy>(
        &mut self,
        items: &BTreeMap<String, V>,
        target_of: &Seen,
        index: &Index<'_>,
        cx: &mut Cx,
    ) -> Result<Option<V>, Error> {
        let Some(&found) = items.get(&target_of.id) else {
            missing(&target_of.id, cx);
            self.rejected = true;
            return Ok(None);
        };
        if index.hash(&target_of.id, cx)? != Some(target_of.hash) {
            self.reject(
                Diagnostic::new(codes::CHANGE_ELEMENT_STALE).with("id", target_of.id.as_str()),
                cx,
            );
            return Ok(None);
        }
        Ok(Some(found))
    }
}

/// 変更が指す ID がない診断
fn missing(id: &str, cx: &mut Cx) {
    cx.report(Diagnostic::new(codes::CHANGE_ELEMENT_MISSING).with("id", id));
}

/// 文書の点の表し方を、ヘッダーの `$PDMODE`（70）と `$PDSIZE`（40）に書く（`Edit::SetPointStyle`、決定 0190）。
/// 値の決まりは読む側（DXF Reference「HEADER Section Group Codes」）と同じ。形は 0〜4 に、円で囲むなら 32、
/// 四角で囲むなら 64 を足す。大きさは、長さなら正、画面の高さに対する百分率なら負の値にする。読んで同じ意味になる
/// 変数は書き直さない。大きさが正で有限でなければ拒む。一覧の中で一度だけ変えられる
fn set_point_style(current: &Current<'_>, plan: &mut Plan<'_>, style: &PointStyle, cx: &mut Cx) {
    let size = match style.size {
        PointSize::Length(length) if length > 0.0 && length.is_finite() => length,
        PointSize::Screen(percent) if percent > 0.0 && percent.is_finite() => -percent,
        _ => 0.0,
    };
    if size == 0.0 || !plan.targets.insert("point_style") {
        plan.reject(
            unsupported("point_style", "point_style", current.target),
            cx,
        );
        return;
    }
    let mark = match style.mark {
        PointMark::Dot => 0,
        PointMark::None => 1,
        PointMark::Plus => 2,
        PointMark::Cross => 3,
        PointMark::Tick => 4,
    };
    let mode = mark | if style.circle { 32 } else { 0 } | if style.square { 64 } else { 0 };
    let mode = vec![Value {
        code: 70,
        data: Data::Int(mode),
    }];
    let same_mode = match current.db.variable(b"$PDMODE") {
        None => mark == 0 && !style.circle && !style.square,
        Some(values) => *values == mode,
    };
    // 0 は画面の高さの 5%
    let meaning = |size: f64| match size {
        size if size > 0.0 => PointSize::Length(size),
        size if size < 0.0 => PointSize::Screen(-size),
        _ => PointSize::Screen(5.0),
    };
    let same_size = match current.db.variable(b"$PDSIZE") {
        None => meaning(0.0) == style.size,
        Some(
            [
                Value {
                    code: 40,
                    data: Data::Real(was),
                },
            ],
        ) if was.is_finite() => meaning(*was) == style.size,
        Some(_) => false,
    };
    if !same_mode {
        plan.variables.push((b"$PDMODE", mode));
    }
    if !same_size {
        plan.variables.push((
            b"$PDSIZE",
            vec![Value {
                code: 40,
                data: Data::Real(size),
            }],
        ));
    }
}

/// 要素を足す（`Edit::Add`）
fn add_element(
    current: &Current<'_>,
    plan: &mut Plan<'_>,
    place: &str,
    element: &Placed,
    cx: &mut Cx,
) -> Result<(), Error> {
    let Current {
        index,
        born,
        frames,
        target,
        ..
    } = *current;
    let Some(&place) = index.places.get(place) else {
        missing(place, cx);
        plan.rejected = true;
        return Ok(());
    };
    let (record, kind) = match (place, element) {
        (Place::Space(space), Placed::Model(element)) => {
            add::work(&element.shape, cx, &Ref::default())?;
            let definition = index.definitions.get(&space).copied();
            if let (Some(definition), Shape::Symbol(symbol)) = (definition, &element.shape)
                && definition.key.namespace == BLOCK
                && symbol.key.namespace == BLOCK
            {
                let outer = definition.key.name.to_lowercase();
                let inner = symbol.key.name.to_lowercase();
                if nests_into(&index.nests, &plan.nests, &inner, &outer, cx)? {
                    plan.reject(
                        Diagnostic::new(codes::CHANGE_SYMBOL_CYCLE_INVALID)
                            .with("id", id::space(space))
                            .with("name", symbol.key.name.as_str()),
                        cx,
                    );
                    return Ok(());
                }
                plan.nests.entry(outer).or_default().insert(inner);
            }
            let home = match definition {
                None if space == MODEL => Some(born.model()),
                None => None,
                Some(definition) => {
                    definition_home(current.db, current.positions, definition, &element.shape)
                }
            };
            let record = home
                .and_then(|home| born.record(element, &InSpace(space), home, plan.added_records));
            (record, kind_of(&element.shape))
        }
        (Place::Sheet(sheet), Placed::Paper(element)) => {
            add::work(&element.shape, cx, &Ref::default())?;
            let record = frames.get(&sheet).and_then(|&frame| {
                born.record(
                    element,
                    &OnSheet(sheet, frame),
                    frame.home,
                    plan.added_records,
                )
            });
            (record, kind_of(&element.shape))
        }
        (_, Placed::Model(element)) => (None, kind_of(&element.shape)),
        (_, Placed::Paper(element)) => (None, kind_of(&element.shape)),
    };
    match record {
        Some(record) => {
            plan.added_records += record.count();
            plan.added.push(record);
        }
        None => plan.reject(unsupported("add", kind, target), cx),
    }
    Ok(())
}

/// 記号の定義 `from` を置くと、その中を直接か間接にたどって定義 `to` に着くか（#333）。`to` の中に `from` を
/// 足すと、定義の入れ子が輪になる。名前は小文字。図面の入れ子（`nests`）と、この変更の一覧で足した入れ子
/// （`added`）をたどり、たどった定義ごとに作業量を 1 数える
fn nests_into(
    nests: &Nests,
    added: &Nests,
    from: &str,
    to: &str,
    cx: &mut Cx,
) -> Result<bool, Error> {
    let mut seen = BTreeSet::from([from]);
    let mut next = vec![from];
    while let Some(name) = next.pop() {
        work(cx, 1, &Ref::default())?;
        if name == to {
            return Ok(true);
        }
        for inner in [nests.get(name), added.get(name)]
            .into_iter()
            .flatten()
            .flatten()
        {
            if seen.insert(inner) {
                next.push(inner);
            }
        }
    }
    Ok(false)
}

/// 記号の定義の空間に足す図形の置き場所（決定 0177）。BLOCK の所有者のブロックのレコードを所有者とし、BLOCKS の
/// 所の、その ENDBLK の前に置く。名前のない定義（寸法・表などを描いた部品）は AutoCAD が描き直して足した図形が
/// 消えうるので、外部参照の定義は中身が外のファイルにあるので、足さない。記号は、定義の入れ子が輪にならない
/// ことを先に確かめる（#333）。BLOCK のレコードかブロックのレコードがなければ `None`
fn definition_home<P>(
    db: &AcadDb,
    positions: &BTreeMap<Ref, usize>,
    definition: &SymbolDefinition,
    shape: &Shape<P>,
) -> Option<Home> {
    if definition.anonymous || definition.external.is_some() {
        return None;
    }
    if let Shape::Symbol(symbol) = shape
        && (definition.key.namespace != BLOCK || symbol.key.namespace != BLOCK)
    {
        return None;
    }
    let block = db.objects().get(*positions.get(&definition.source)?)?;
    if block.kind != b"BLOCK" {
        return None;
    }
    let owner = block.owner().filter(|&owner| {
        db.get(owner)
            .is_some_and(|record| record.kind == b"BLOCK_RECORD")
    })?;
    Some(Home {
        owner: Some(owner),
        blocks: true,
        paper: false,
    })
}

/// 要素を置き換える・スタイルを変える・動かす・除く（`Edit::{Replace,Restyle,Transform,Remove}`）。`edit` は
/// 診断に書く変更の名前
fn change_element<'e>(
    current: &Current<'_>,
    plan: &mut Plan<'e>,
    change: &Edit,
    edit: &str,
    target_of: &'e Seen,
    cx: &mut Cx,
) -> Result<(), Error> {
    let Current {
        db,
        index,
        born,
        positions,
        frames,
        encoder,
        target,
        ..
    } = *current;
    let Some(found) = plan.seen(&index.elements, target_of, index, cx)? else {
        return Ok(());
    };
    let at = match found {
        Found::Model(element) => &element.source,
        Found::Paper(element) => &element.source,
    };
    if !plan.targets.insert(target_of.id.as_str()) {
        let kind = match found {
            Found::Model(element) => kind_of(&element.shape),
            Found::Paper(element) => kind_of(&element.shape),
        };
        plan.reject(unsupported(edit, kind, target).at(at.clone()), cx);
        return Ok(());
    }
    let object = (!index.shared.contains(at))
        .then(|| positions.get(at).copied())
        .flatten()
        .and_then(|position| Some((position, db.objects().get(position)?)));
    let Some((position, object)) = object else {
        let kind = match found {
            Found::Model(element) => kind_of(&element.shape),
            Found::Paper(element) => kind_of(&element.shape),
        };
        plan.reject(unsupported(edit, kind, target).at(at.clone()), cx);
        return Ok(());
    };
    // 置き換え値の比較と、元のレコードの複製に先立って作業量を数える。
    if let Edit::Replace { element, .. } = change {
        match element.as_ref() {
            Placed::Model(element) => add::work(&element.shape, cx, at)?,
            Placed::Paper(element) => add::work(&element.shape, cx, at)?,
        }
        // 属性の値を置き換えるときは、続く ATTRIB の値も複製する。
        // その走査と複製を、実行する前に予算に含める。
        match found {
            Found::Model(was) => transform_work(db, positions, was, object, cx)?,
            Found::Paper(was) => transform_work(db, positions, was, object, cx)?,
        }
    }
    let reshaped = match (change, found) {
        (Edit::Replace { element, .. }, Found::Model(was)) => {
            matches!(element.as_ref(), Placed::Model(element) if element.shape != was.shape)
        }
        (Edit::Replace { element, .. }, Found::Paper(was)) => {
            matches!(element.as_ref(), Placed::Paper(element) if element.shape != was.shape)
        }
        _ => false,
    };
    if reshaped && let Some(code) = moving_xdata(object) {
        plan.reject(
            Diagnostic::new(codes::CHANGE_XDATA_UNSUPPORTED)
                .with("kind", text(&object.kind))
                .with("code", i64::from(code))
                .at(at.clone()),
            cx,
        );
        return Ok(());
    }
    // 図面モデルに移せなかった ATTRIB も元のファイルには残る。INSERT だけを動かさない。
    let moved_symbol = match (change, found) {
        (Edit::Replace { element, .. }, Found::Model(was)) => {
            matches!(element.as_ref(), Placed::Model(element) if symbol_moved(&was.shape, &element.shape))
        }
        (Edit::Replace { element, .. }, Found::Paper(was)) => {
            matches!(element.as_ref(), Placed::Paper(element) if symbol_moved(&was.shape, &element.shape))
        }
        _ => false,
    };
    let lifted_count = match (change, found) {
        (Edit::Transform { .. }, Found::Model(was)) => match &was.shape {
            Shape::Symbol(symbol) => Some(symbol.attributes.len()),
            _ => None,
        },
        (Edit::Transform { .. }, Found::Paper(was)) => match &was.shape {
            Shape::Symbol(symbol) => Some(symbol.attributes.len()),
            _ => None,
        },
        _ => None,
    };
    let count = if moved_symbol || lifted_count.is_some() {
        if plan.attribute_counts.is_none() {
            plan.attribute_counts = Some(attribute_counts_for(db, at, cx)?);
        }
        plan.attribute_counts
            .as_ref()
            .and_then(|counts| counts.get(&position))
            .copied()
            .unwrap_or_default()
    } else {
        0
    };
    if lifted_count.is_some_and(|lifted| count != lifted) || (moved_symbol && count > 0) {
        plan.reject(
            unsupported(edit, &text(&object.kind), target).at(at.clone()),
            cx,
        );
        return Ok(());
    }
    let replaced = &mut plan.replaced;
    let planned = match (change, found) {
        (Edit::Replace { element, .. }, Found::Model(was)) => match element.as_ref() {
            Placed::Model(element) => anchor(&was.shape)
                .and_then(|anchor| {
                    let frame = InSpace(anchor.space);
                    let (written, followers) =
                        revalued(db, positions, was, element, encoder, &frame)?;
                    let values = replacement(was, &written, object, encoder, &frame)?;
                    Some((values, followers))
                })
                .map(|(values, followers)| {
                    replaced.push((position, values));
                    replaced.extend(followers);
                }),
            Placed::Paper(_) => None,
        },
        (Edit::Replace { element, .. }, Found::Paper(was)) => match element.as_ref() {
            Placed::Paper(element) => anchor(&was.shape)
                .and_then(|anchor| {
                    let frame = OnSheet(anchor.sheet, *frames.get(&anchor.sheet)?);
                    let (written, followers) =
                        revalued(db, positions, was, element, encoder, &frame)?;
                    let values = replacement(was, &written, object, encoder, &frame)?;
                    Some((values, followers))
                })
                .map(|(values, followers)| {
                    replaced.push((position, values));
                    replaced.extend(followers);
                }),
            Placed::Model(_) => None,
        },
        (Edit::Restyle { style, .. }, found) => {
            let (layer, was) = match found {
                Found::Model(element) => (element.layer, &element.style),
                Found::Paper(element) => (element.layer, &element.style),
            };
            restyled(born, object, layer, was, style, cx)?
                .map(|values| replaced.push((position, values)))
        }
        (Edit::Transform { transform, .. }, Found::Model(was)) => {
            transform_work(db, positions, was, object, cx)?;
            anchor(&was.shape)
                .and_then(|anchor| {
                    let frame = InSpace(anchor.space);
                    moved(db, positions, was, object, transform, encoder, &frame, true)
                })
                .map(|values| {
                    replaced.push((position, values.object));
                    replaced.extend(values.followers);
                })
        }
        (Edit::Transform { transform, .. }, Found::Paper(was)) => {
            transform_work(db, positions, was, object, cx)?;
            anchor(&was.shape)
                .and_then(|anchor| {
                    let frame = OnSheet(anchor.sheet, *frames.get(&anchor.sheet)?);
                    moved(
                        db, positions, was, object, transform, encoder, &frame, false,
                    )
                })
                .map(|values| {
                    replaced.push((position, values.object));
                    replaced.extend(values.followers);
                })
        }
        (Edit::Remove { .. }, _) => entity_removal(db, position, object, target, cx, at)?
            .map(|skip| plan.removed.extend(skip)),
        (
            Edit::Add { .. }
            | Edit::ReplaceLayer { .. }
            | Edit::AddLayer { .. }
            | Edit::RemoveLayer { .. }
            | Edit::ReplaceTextStyle { .. }
            | Edit::AddTextStyle { .. }
            | Edit::RemoveTextStyle { .. }
            | Edit::ReplaceLineType { .. }
            | Edit::AddLineType { .. }
            | Edit::RemoveLineType { .. }
            | Edit::ReplaceDimensionStyle { .. }
            | Edit::AddDimensionStyle { .. }
            | Edit::RemoveDimensionStyle { .. }
            | Edit::SetPointStyle { .. },
            _,
        ) => None,
    };
    if planned.is_none() {
        plan.reject(
            unsupported(edit, &text(&object.kind), target).at(at.clone()),
            cx,
        );
    }
    Ok(())
}

/// remove は毎回、残る全オブジェクトを再索引し、値から参照を除く。
/// 拡張辞書の子をまとめて除く場合も、変更する前に全回数分の作業を数える。
fn count_removals(
    db: &AcadDb,
    replaced: &[(usize, Vec<Value>)],
    removed: &[usize],
    cx: &mut Cx,
) -> Result<(), Error> {
    if !removed.is_empty() {
        let mut values = 0u64;
        for object in db.objects() {
            work(cx, 1, &Ref::default())?;
            values = values.saturating_add(object.values.len() as u64);
        }
        for (_, replacement) in replaced {
            values = values.saturating_add(replacement.len() as u64);
        }
        let per_remove = (db.objects().len() as u64)
            .saturating_add(values)
            .saturating_mul(2);
        work(
            cx,
            per_remove.saturating_mul(removed.len() as u64),
            &Ref::default(),
        )?;
    }
    Ok(())
}

/// 足す表のレコード（LAYER・STYLE・LTYPE・DIMSTYLE）を、DXF は表の終わりに、DWG は後ろに置き、ハンドルを返す
fn insert_records(
    db: &mut AcadDb,
    kind: &[u8],
    added: Vec<Vec<Value>>,
    target: Target,
    cx: &mut Cx,
) -> Result<Vec<Option<u64>>, Error> {
    let mut handles = Vec::with_capacity(added.len());
    for values in added {
        for object in db.objects() {
            work(cx, 1, &Ref::default())?;
            count_transform_values(object, &Ref::default(), cx)?;
        }
        let handle = add::next_free_handle(db, target);
        let mut record = layer::record(kind, values, handle);
        // 寸法スタイルのハンドルは 105 に置く（5 は DIMBLK の名前）
        if kind == b"DIMSTYLE"
            && handle.is_some()
            && let Some(first) = record.values.first_mut()
        {
            first.code = 105;
        }
        match target {
            Target::Dxf => {
                let at = layer::table_end(db, kind).ok_or(Error::Rejected)?;
                db.insert(at, record);
            }
            Target::Dwg(_) => db.push(record),
        }
        handles.push(handle);
    }
    Ok(handles)
}

/// 変更ごとに、書き戻した後の結果を探すキー。要素の場所は追加後に変わりうる。ハンドルで読み直した要素の ID を引く
fn results(
    changes: &Changes,
    index: &Index<'_>,
    handles: Vec<Option<u64>>,
    layer_handles: Vec<Option<u64>>,
    style_handles: Vec<Option<u64>>,
    line_type_handles: Vec<Option<u64>>,
    dimension_style_handles: Vec<Option<u64>>,
) -> Vec<ResultTarget> {
    let mut handles = handles.into_iter();
    let mut dimension_style_handles = dimension_style_handles.into_iter();
    let mut style_handles = style_handles.into_iter();
    let mut line_type_handles = line_type_handles.into_iter();
    let mut layer_handles = layer_handles.into_iter();
    changes
        .edits
        .iter()
        .map(|change| match change {
            Edit::Replace { target, .. }
            | Edit::Restyle { target, .. }
            | Edit::Transform { target, .. } => {
                let handle = index.elements.get(&target.id).and_then(|found| {
                    let source = match found {
                        Found::Model(element) => &element.source,
                        Found::Paper(element) => &element.source,
                    };
                    match source.key() {
                        Some(Key::Handle(handle)) => Some(handle),
                        _ => None,
                    }
                });
                ResultTarget::Element(handle)
            }
            Edit::Add { .. } => ResultTarget::Element(handles.next().flatten()),
            Edit::Remove { .. }
            | Edit::RemoveLayer { .. }
            | Edit::RemoveTextStyle { .. }
            | Edit::RemoveLineType { .. }
            | Edit::RemoveDimensionStyle { .. } => ResultTarget::Removed,
            Edit::ReplaceDimensionStyle { target, .. } => {
                ResultTarget::Table(Some(target.id.clone()))
            }
            Edit::AddDimensionStyle { .. } => ResultTarget::Table(
                dimension_style_handles
                    .next()
                    .flatten()
                    .map(|handle| format!("dimension-style/h{handle:X}")),
            ),
            Edit::ReplaceLineType { target, .. } => ResultTarget::Table(Some(target.id.clone())),
            Edit::AddLineType { .. } => ResultTarget::Table(
                line_type_handles
                    .next()
                    .flatten()
                    .map(|handle| format!("line-type/h{handle:X}")),
            ),
            Edit::ReplaceLayer { target, .. } => ResultTarget::Table(Some(target.id.clone())),
            Edit::AddLayer { .. } => ResultTarget::Table(
                layer_handles
                    .next()
                    .flatten()
                    .map(|handle| format!("layer/h{handle:X}")),
            ),
            Edit::SetPointStyle { .. } => ResultTarget::Document,
            Edit::ReplaceTextStyle { target, .. } => ResultTarget::Table(Some(target.id.clone())),
            Edit::AddTextStyle { .. } => ResultTarget::Table(
                style_handles
                    .next()
                    .flatten()
                    .map(|handle| format!("text-style/h{handle:X}")),
            ),
        })
        .collect()
}

/// 表のレコード（画層・文字スタイル）を探す前に、名前と出所の探索に掛かる作業を数える。
fn count_table_records(db: &AcadDb, records: &[usize], at: &Ref, cx: &mut Cx) -> Result<(), Error> {
    for &position in records {
        work(cx, 1, at)?;
        if let Some(object) = db.objects().get(position) {
            count_transform_values(object, at, cx)?;
        }
    }
    Ok(())
}

/// 表の終わりを探す前に、表の名前を調べる作業を数える。
fn count_table_search(db: &AcadDb, cx: &mut Cx) -> Result<(), Error> {
    let at = Ref::default();
    for object in db.objects() {
        work(cx, 1, &at)?;
        if object.kind == b"TABLE" {
            count_transform_values(object, &at, cx)?;
        }
    }
    Ok(())
}

/// 表のレコードの位置 `records` のうち、出所が `source` のもの
fn record_at<'a>(db: &'a AcadDb, records: &[usize], source: &Ref) -> Option<(usize, &'a Object)> {
    records.iter().find_map(|&position| {
        db.objects()
            .get(position)
            .filter(|object| super::source(object) == *source)
            .map(|object| (position, object))
    })
}

fn symbol_moved<P: PartialEq>(before: &Shape<P>, after: &Shape<P>) -> bool {
    matches!((before, after), (Shape::Symbol(a), Shape::Symbol(b))
        if a.position != b.position || a.scale != b.scale || a.rotation != b.rotation)
}

/// 元の ATTRIB の数を、読み込みと同じ所有者・並びの規則でまとめて調べる。
/// 添字は元の表の位置。変更が多くても表の走査は一度だけ。
fn attribute_counts_for(
    db: &AcadDb,
    at: &Ref,
    cx: &mut Cx<'_>,
) -> Result<BTreeMap<usize, usize>, Error> {
    let mut entities = Vec::new();
    let mut blocks = Vec::new();
    for (position, candidate) in db.objects().iter().enumerate() {
        work(cx, 1, at)?;
        if matches!(candidate.section, Section::Entities | Section::Blocks) {
            for _ in &candidate.values {
                work(cx, 1, at)?;
            }
            let group = if candidate.section == Section::Entities {
                &mut entities
            } else {
                &mut blocks
            };
            group.push((position, candidate));
        }
    }
    let mut counts = BTreeMap::new();
    for group in [entities, blocks] {
        let objects: Vec<_> = group.iter().map(|(_, object)| *object).collect();
        let (attributes, _) = attached(&objects);
        for (index, items) in attributes {
            if let Some((position, _)) = group.get(index) {
                counts.insert(*position, items.len());
            }
        }
    }
    Ok(counts)
}

/// 書き戻した表から図面モデルを作り直し、置き換えた要素と足した要素の ID とハッシュを求める
fn settled(db: &AcadDb, targets: Vec<ResultTarget>, cx: &mut Cx) -> Result<Vec<Applied>, Error> {
    let mut quiet = Diagnostics::new();
    let (after, _) = framed(db, &mut Cx::new(cx.budget, &mut quiet, cx.cancel))?;
    let after = Index::new(&after, cx)?;
    targets
        .into_iter()
        .map(|target| {
            let id = match target {
                ResultTarget::Element(handle) => handle
                    .and_then(|handle| after.element_handles.get(&handle))
                    .cloned()
                    .flatten(),
                ResultTarget::Table(id) => id,
                ResultTarget::Removed | ResultTarget::Document => None,
            };
            applied(&after, id, cx)
        })
        .collect()
}

/// 出所から元の表の位置を引く。重複する出所は、従来の走査と同じく最初のものを返す。
fn object_positions(db: &AcadDb, cx: &mut Cx<'_>) -> Result<BTreeMap<Ref, usize>, Error> {
    let mut positions = BTreeMap::new();
    let at = Ref::default();
    for (position, object) in db.objects().iter().enumerate() {
        work(cx, 1, &at)?;
        if matches!(object.section, Section::Entities | Section::Blocks) {
            for _ in &object.values {
                work(cx, 1, &at)?;
            }
            positions.entry(source(object)).or_insert(position);
        }
    }
    Ok(positions)
}

/// 図面モデルの点と長さを、元の図形のレコードの値に戻す
trait Frame<P> {
    /// 点のレコードの X と Y。元の要素と同じ空間（用紙）の点でなければ `None`
    fn point(&self, point: &P) -> Option<[f64; 2]>;
    /// 長さのレコードの値。元の要素と同じ空間（用紙）の長さでなければ `None`
    fn length(&self, length: &Length) -> Option<f64>;
    /// レコードの長さを、読む側と同じく図面モデルの長さにする
    fn lifted(&self, value: f64) -> Length;
    /// 図面モデルの点を写す変換 `transform` を、レコードの座標を写す変換にする
    fn affine(&self, transform: &Affine) -> Affine;
}

/// 実寸の空間。点と長さは、レコードの値のまま
struct InSpace(SpaceId);

impl Frame<ModelPoint> for InSpace {
    fn point(&self, point: &ModelPoint) -> Option<[f64; 2]> {
        (point.space == self.0).then_some([point.x, point.y])
    }

    fn length(&self, length: &Length) -> Option<f64> {
        match length {
            Length::Model(length) if length.space == self.0 => Some(length.value),
            _ => None,
        }
    }

    fn lifted(&self, value: f64) -> Length {
        Length::Model(ModelLength {
            space: self.0,
            value,
        })
    }

    fn affine(&self, transform: &Affine) -> Affine {
        *transform
    }
}

/// 用紙。用紙の mm を、読むときと逆に、図面の単位に戻して原点を足す
struct OnSheet(SheetId, PaperFrame);

impl Frame<PaperPoint> for OnSheet {
    fn point(&self, point: &PaperPoint) -> Option<[f64; 2]> {
        let PaperFrame {
            origin: [x, y], mm, ..
        } = self.1;
        (point.sheet == self.0).then_some([point.x / mm + x, point.y / mm + y])
    }

    fn length(&self, length: &Length) -> Option<f64> {
        match length {
            Length::Paper(PaperLength(value)) => Some(value / self.1.mm),
            _ => None,
        }
    }

    fn lifted(&self, value: f64) -> Length {
        Length::Paper(PaperLength(value * self.1.mm))
    }

    /// レコードの点 `q` は用紙の点 `mm (q − o)` なので、`L p + t` は `L q + t / mm + o − L o` になる
    fn affine(&self, transform: &Affine) -> Affine {
        let PaperFrame {
            origin: [x, y], mm, ..
        } = self.1;
        let [a, b, c, d, e, f] = transform.m;
        Affine {
            m: [
                a,
                b,
                c / mm + x - (a * x + b * y),
                d,
                e,
                f / mm + y - (d * x + e * y),
            ],
        }
    }
}

/// 点を書く。変わっていなければ、元の値を残す（戻した値が最後の桁で違って、変えていない点を書き換えないため）
fn place<P: PartialEq>(
    values: &mut Vec<Value>,
    frame: &impl Frame<P>,
    [x, y]: [i16; 2],
    point: &P,
    was: &P,
) -> Option<()> {
    let at = frame.point(point)?;
    if point != was {
        put(values, x, at[0], &[])?;
        put(values, y, at[1], &[])?;
    }
    Some(())
}

/// 図形とともに動く XDATA の最初のグループ・コード。AutoCAD は、図形を動かし・回し・尺度を変えると、
/// 1011（位置）・1012（変位）・1013（向き）・1041（距離）・1042（倍率）を同じく直す（DXF Reference
/// 「Group Codes That Apply to Extended Data」）。置き換えから同じ変換を決められないので、書き戻さない（決定 0154）
fn moving_xdata(object: &Object) -> Option<i16> {
    let start = object.values.iter().position(|v| v.code == 1001)?;
    object
        .values
        .iter()
        .skip(start)
        .map(|v| v.code)
        .find(|code| matches!(code, 1011..=1013 | 1041 | 1042))
}

/// 除く表のレコードを、一緒に除く位置のほかが指すかを答える関数（`layer::referenced` など）
type Referenced = fn(&AcadDb, &BTreeSet<usize>, &Object, &mut Cx, &Ref) -> Result<bool, Error>;

/// 除く表のレコード `object`（表の `position`）と、一緒に除く拡張辞書とそれが持つオブジェクトの位置
/// （決定 0170）。一緒に除くものを辿れないとき、レコードか一緒に除くものをほかが指すとき（`referenced`）は `None`
fn removal(
    db: &AcadDb,
    position: usize,
    object: &Object,
    referenced: Referenced,
    cx: &mut Cx,
    place: &Ref,
) -> Result<Option<BTreeSet<usize>>, Error> {
    let Some(owned) = owned::owned(db, object, cx, place)? else {
        return Ok(None);
    };
    let handles = owned.values().copied().collect();
    let skip: BTreeSet<usize> = owned.into_keys().chain([position]).collect();
    let free = !referenced(db, &skip, object, cx, place)?
        && !owned::pointed(db, &skip, &handles, cx, place)?;
    Ok(free.then_some(skip))
}

/// 除く図形（表の `position`）と、DXF で一緒に除く拡張辞書とそれが持つオブジェクトの位置（#881）。
/// 続くレコードを持つ図形は除かない。DWG は図形の拡張辞書とリアクターを書き戻し側（`DwgFile::write_back`）で
/// 検査する。DXF は、102 の組も組の外の 360 もなければ図形だけを除き、あれば表のレコードと同じ条件
/// （[`owned::owned`]・[`owned::pointed`]）で拡張辞書の木ごと除く
fn entity_removal(
    db: &AcadDb,
    position: usize,
    object: &Object,
    target: Target,
    cx: &mut Cx,
    place: &Ref,
) -> Result<Option<BTreeSet<usize>>, Error> {
    if followed(object) {
        return Ok(None);
    }
    if matches!(target, Target::Dwg(_)) {
        return Ok(Some(BTreeSet::from([position])));
    }
    count_transform_values(object, place, cx)?;
    if !object
        .values
        .iter()
        .any(|value| matches!(value.code, 102 | 360))
    {
        return Ok(Some(BTreeSet::from([position])));
    }
    let Some(owned) = owned::owned(db, object, cx, place)? else {
        return Ok(None);
    };
    let handles = owned.values().copied().collect();
    let skip: BTreeSet<usize> = owned.into_keys().chain([position]).collect();
    let free = !owned::pointed(db, &skip, &handles, cx, place)?;
    Ok(free.then_some(skip))
}

/// 続くレコード（POLYLINE の VERTEX、属性を持つ INSERT の ATTRIB、SEQEND）を持つ図形か
fn followed(object: &Object) -> bool {
    match object.kind.as_slice() {
        b"POLYLINE" => true,
        b"INSERT" => matches!(object.first(66), Some(Data::Int(n)) if *n != 0),
        _ => false,
    }
}
