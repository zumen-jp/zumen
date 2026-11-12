//! DWG の共有オブジェクト表から R2004 DXF を書く（決定 0008・0023）。対応する図形と参照を
//! すべて書けるときだけこの経路を使う。試行中のバイト列と欠落の診断は、成功するまで公開しない。

use std::collections::{BTreeMap, BTreeSet};

use zumen_base::{Cx, Diagnostic, DiagnosticEvents, Kind, Ref, codes};
use zumen_sdk::Error;

use crate::charset::{Charset, charset_of};
use crate::conversion_report::Report;
use crate::db::{AcadDb, Data, Object, Origin, Section, Value};
use crate::dwg::{DwgFile, reencode::DxfSource};
use crate::{ConversionFidelity, ConversionReport};

mod tags;
use tags::Writer;

#[cfg(test)]
mod tests;

/// 同じ系統のオブジェクト表から書いた DXF。
pub struct ReencodedDxf {
    /// ASCII DXF のバイト列。
    pub bytes: Vec<u8>,
    /// モデル空間・用紙・ブロックの定義に書いた図形の合計。
    pub elements: u64,
    /// 書いたレコードと落としたレコードの全件集計と、指定した上限までの明細。
    pub report: ConversionReport,
}

/// R2004 DWG の同系統の変換結果。対象外のときは、既に読んだ表を図面モデルへ渡せる。
pub enum R2004Conversion {
    /// 元のハンドルと参照を使って書いた DXF。
    Dxf(ReencodedDxf),
    /// 同系統で扱えない組み合わせを持つ表。
    DrawingSource(AcadDb),
}

impl DwgFile {
    /// 同系統で扱える図面を DXF にする。必要な参照を解けないときは、図面モデルの変換に渡す。
    pub fn reencode_r2004_dxf(&self, cx: &mut Cx<'_>) -> Result<R2004Conversion, Error> {
        self.reencode_r2004_dxf_with_report(0, cx)
    }

    /// 同系統の値と参照を使って DXF を書き、`limit` 件までの明細を返す。
    ///
    /// # Errors
    ///
    /// [`Self::reencode_r2004_dxf`] と同じ。
    pub fn reencode_r2004_dxf_with_report(
        &self,
        limit: u64,
        cx: &mut Cx<'_>,
    ) -> Result<R2004Conversion, Error> {
        let db = self.objects(cx)?;
        self.reencode_r2004_db(db, limit, cx)
    }

    /// 入力の表を読むときの診断を、書き手の診断とは別に集計する。
    ///
    /// `limit` は入力の診断と書き手のそれぞれの明細の上限。入力側の件数は診断の発生数で、
    /// 図形数ではない。図面モデルへ渡す場合は、返した表の lift の報告を呼び出す側で足す。
    ///
    /// # Errors
    ///
    /// [`Self::reencode_r2004_dxf`] と同じ。診断の記録も予算で数える。
    pub fn reencode_r2004_dxf_with_input_diagnostics(
        &self,
        limit: u64,
        cx: &mut Cx<'_>,
    ) -> Result<(R2004Conversion, DiagnosticEvents), Error> {
        let (db, diagnostics) = cx.with_diagnostic_events(limit, |cx| self.objects(cx))?;
        Ok((self.reencode_r2004_db(db, limit, cx)?, diagnostics))
    }

    fn reencode_r2004_db(
        &self,
        db: AcadDb,
        limit: u64,
        cx: &mut Cx<'_>,
    ) -> Result<R2004Conversion, Error> {
        let encoded = try_reencode(&db, limit, cx, |plan, cx| {
            self.dxf_source(&db, &plan.kept, cx)
        })?;
        Ok(match encoded {
            Some(file) => R2004Conversion::Dxf(file),
            None => R2004Conversion::DrawingSource(db),
        })
    }
}

fn try_reencode<'a>(
    db: &AcadDb,
    limit: u64,
    cx: &mut Cx<'_>,
    source: impl FnOnce(&Plan<'_>, &mut Cx<'_>) -> Result<DxfSource<'a>, Error>,
) -> Result<Option<ReencodedDxf>, Error> {
    let result = (|| {
        let mut plan = Plan::new(db, cx)?;
        let source = source(&plan, cx)?;
        plan.order_blocks(&source, cx)?;
        encode(db, &plan, &source, limit, cx)
    })();
    match result {
        Ok(out) => Ok(Some(out)),
        Err(Error::Unsupported) => Ok(None),
        Err(Error::Stopped(stop)) => {
            cx.report(stop.to_diagnostic());
            Err(Error::Stopped(stop))
        }
        Err(other) => Err(other),
    }
}

const TABLES: &[(&[u8], &[u8])] = &[
    (b"VPORT", b"AcDbViewportTableRecord"),
    (b"LTYPE", b"AcDbLinetypeTableRecord"),
    (b"LAYER", b"AcDbLayerTableRecord"),
    (b"STYLE", b"AcDbTextStyleTableRecord"),
    (b"APPID", b"AcDbRegAppTableRecord"),
    (b"DIMSTYLE", b"AcDbDimStyleTableRecord"),
    (b"BLOCK_RECORD", b"AcDbBlockTableRecord"),
];

fn text(value: &Value) -> Option<&[u8]> {
    match &value.data {
        Data::Text(bytes) => Some(bytes),
        _ => None,
    }
}

fn string(object: &Object, code: i16) -> Result<&[u8], Error> {
    match object.first(code) {
        Some(Data::Text(s)) if !s.is_empty() => Ok(s),
        _ => Err(Error::Unsupported),
    }
}

fn handle(object: &Object) -> Result<u64, Error> {
    object
        .handle()
        .filter(|id| *id != 0)
        .ok_or(Error::Unsupported)
}

fn owner(object: &Object) -> Result<u64, Error> {
    object.owner().ok_or(Error::Unsupported)
}

const MAX_NAME_CHARS: usize = 255;
const MAX_NAME_BYTES: usize = 2 * MAX_NAME_CHARS;

// DXF Reference「Symbol Table Group Codes」。匿名ブロックと VPORT の先頭の * は予約名に使う。
// R2004 では 1 文字が最大 2 バイト。禁止文字と 255 文字の上限は復号した名前で判定する。
fn valid_name(charset: Charset, table: &[u8], bytes: &[u8]) -> bool {
    if bytes.len() > MAX_NAME_BYTES {
        return false;
    }
    let Some(name) = charset.decode_name(bytes) else {
        return false;
    };
    let value = if matches!(table, b"BLOCK_RECORD" | b"VPORT") {
        name.strip_prefix('*').unwrap_or(&name)
    } else {
        &name
    };
    !value.is_empty()
        && name.chars().count() <= MAX_NAME_CHARS
        && name.trim() == name
        && !value
            .chars()
            .any(|c| c.is_control() || r#"<>/\:;?*|=,"`"#.contains(c))
}

fn entity_class(kind: &[u8]) -> Option<&'static [u8]> {
    Some(match kind {
        b"ARC_DIMENSION" => b"AcDbDimension",
        b"ARC" | b"CIRCLE" => b"AcDbCircle",
        b"LINE" => b"AcDbLine",
        b"SOLID" | b"TRACE" => b"AcDbTrace",
        b"POINT" => b"AcDbPoint",
        b"MTEXT" => b"AcDbMText",
        b"BLOCK" => b"AcDbBlockBegin",
        b"ENDBLK" => b"AcDbBlockEnd",
        _ => return None,
    })
}

#[derive(Default)]
struct Block<'a> {
    begin: Option<&'a Object>,
    end: Option<&'a Object>,
    entities: Vec<&'a Object>,
}

struct Plan<'a> {
    charset: Charset,
    tables: BTreeMap<&'a [u8], Vec<&'a Object>>,
    names: BTreeMap<(&'a [u8], Vec<u8>), &'a Object>,
    blocks: BTreeMap<u64, Block<'a>>,
    layouts: BTreeMap<u64, u64>,
    objects: BTreeSet<u64>,
    kept: BTreeSet<u64>,

    elements: u64,
    next_handle: u64,
}

impl<'a> Plan<'a> {
    fn new(db: &'a AcadDb, cx: &mut Cx<'_>) -> Result<Self, Error> {
        if !matches!(db.variable(b"$ACADVER"), Some([v]) if text(v)==Some(b"AC1018"))
            || !matches!(db.variable(b"$DWGCODEPAGE"), Some([v]) if v.code == 3 && text(v).is_some_and(|s| !s.is_empty() && s.is_ascii()))
        {
            return Err(Error::Unsupported);
        }
        // この経路が扱う寸法を含まない図面には、参照の索引を作らない。
        let mut has_dimension = false;
        for object in db.objects() {
            cx.check(Kind::Work, 1)?;
            if object.kind == b"ARC_DIMENSION"
                && matches!(object.section, Section::Entities | Section::Blocks)
            {
                has_dimension = true;
                break;
            }
        }
        if !has_dimension {
            return Err(Error::Unsupported);
        }
        let mut plan = Self {
            charset: charset_of(db),
            tables: BTreeMap::new(),
            names: BTreeMap::new(),
            blocks: BTreeMap::new(),
            layouts: BTreeMap::new(),
            objects: BTreeSet::new(),
            kept: BTreeSet::new(),

            elements: 0,
            next_handle: 0,
        };
        let mut root = None;
        for object in db.objects() {
            cx.check(Kind::Work, (object.values.len() as u64).saturating_add(1))?;
            if !matches!(object.origin, Origin::Dwg { .. }) {
                return Err(Error::Unsupported);
            }
            let id = handle(object)?;
            // 重複するハンドルでは参照先を決められない。
            if !db.get(id).is_some_and(|found| std::ptr::eq(found, object)) {
                return Err(Error::Unsupported);
            }
            plan.next_handle = plan
                .next_handle
                .max(id.checked_add(1).ok_or(Error::Unsupported)?);
            match &object.section {
                Section::Tables if object.kind != b"TABLE" => {
                    if !TABLES.iter().any(|(name, _)| *name == object.kind) {
                        return Err(Error::Unsupported);
                    }
                    let table = owner(object)?;
                    if !db.get(table).is_some_and(|o| o.kind == b"TABLE") {
                        return Err(Error::Unsupported);
                    }
                    let name = string(object, 2)?;
                    cx.check(Kind::Work, name.len() as u64)?;
                    if !valid_name(plan.charset, &object.kind, name) {
                        return Err(Error::Unsupported);
                    }
                    if plan
                        .names
                        .insert((&object.kind, plan.charset.fold(name)), object)
                        .is_some()
                    {
                        return Err(Error::Unsupported);
                    }
                    let rows = plan.tables.entry(&object.kind).or_default();
                    if rows
                        .first()
                        .is_some_and(|first| first.owner() != Some(table))
                    {
                        return Err(Error::Unsupported);
                    }
                    rows.push(object);
                    plan.kept.insert(table);
                    plan.kept.insert(id);
                }
                Section::Blocks | Section::Entities => {
                    entity_class(&object.kind).ok_or(Error::Unsupported)?;
                    if matches!(object.kind.as_slice(), b"BLOCK" | b"ENDBLK")
                        && object.section != Section::Blocks
                    {
                        return Err(Error::Unsupported);
                    }
                    let block = plan.blocks.entry(owner(object)?).or_default();
                    match object.kind.as_slice() {
                        b"BLOCK" if block.begin.replace(object).is_none() => (),
                        b"ENDBLK" if block.end.replace(object).is_none() => (),
                        b"BLOCK" | b"ENDBLK" => return Err(Error::Unsupported),
                        _ => {
                            plan.elements += 1;
                            block.entities.push(object);
                        }
                    }
                    plan.kept.insert(id);
                }
                Section::Objects if object.kind == b"DICTIONARY" && object.owner() == Some(0) => {
                    match root {
                        None => root = Some(object),
                        Some(_) => return Err(Error::Unsupported),
                    }
                }
                _ => {}
            }
        }
        let mut controls = BTreeSet::new();
        for rows in plan.tables.values() {
            let control = owner(rows.first().ok_or(Error::Unsupported)?)?;
            if !controls.insert(control) || db.get(control).and_then(Object::owner) != Some(0) {
                return Err(Error::Unsupported);
            }
        }
        plan.layout_objects(db, root.ok_or(Error::Unsupported)?, cx)?;
        for row in plan
            .tables
            .get(b"BLOCK_RECORD".as_slice())
            .ok_or(Error::Unsupported)?
        {
            let block = plan.blocks.get(&handle(row)?).ok_or(Error::Unsupported)?;
            if block.begin.is_none() || block.end.is_none() {
                return Err(Error::Unsupported);
            }
        }
        for id in plan.blocks.keys() {
            if !db.get(*id).is_some_and(|r| r.kind == b"BLOCK_RECORD") {
                return Err(Error::Unsupported);
            }
        }
        for (id, block) in &plan.blocks {
            if !plan.layouts.contains_key(id)
                && block.entities.iter().any(|o| o.kind == b"ARC_DIMENSION")
            {
                return Err(Error::Unsupported);
            }
        }
        for name in [b"*MODEL_SPACE".as_slice(), b"*PAPER_SPACE"] {
            let space = plan.named(b"BLOCK_RECORD", name, cx)?;
            if !plan.layouts.contains_key(&handle(space)?) {
                return Err(Error::Unsupported);
            }
        }
        Ok(plan)
    }

    // ODS 5.4.1 の BLOCK_HEADER。ハンドルの値と、ブロックが持つ図形の表示順は別に決まる。
    fn order_blocks(&mut self, source: &DxfSource<'_>, cx: &mut Cx<'_>) -> Result<(), Error> {
        for (id, block) in &mut self.blocks {
            let order = source.order.get(id).ok_or(Error::Unsupported)?;
            if order.len() != block.entities.len() {
                return Err(Error::Unsupported);
            }
            cx.check(Kind::Work, order.len() as u64)?;
            let mut members = BTreeMap::new();
            for entity in block.entities.drain(..) {
                members.insert(handle(entity)?, entity);
            }
            for key in order {
                block
                    .entities
                    .push(members.remove(key).ok_or(Error::Unsupported)?);
            }
        }
        Ok(())
    }

    fn named(&self, table: &'static [u8], name: &[u8], cx: &mut Cx<'_>) -> Result<&Object, Error> {
        cx.check(Kind::Work, (name.len() as u64).saturating_add(1))?;
        if name.len() > MAX_NAME_BYTES {
            return Err(Error::Unsupported);
        }
        self.names
            .get(&(table, self.charset.fold(name)))
            .copied()
            .ok_or(Error::Unsupported)
    }

    // DXF Reference「DICTIONARY」「LAYOUT」「ACDBDICTIONARYWDFLT」。用紙と画層の印刷スタイルが
    // 指す辞書だけを選ぶ。ほかの辞書への項目は実在しないハンドルとして書かない。
    fn layout_objects(
        &mut self,
        db: &'a AcadDb,
        root: &Object,
        cx: &mut Cx<'_>,
    ) -> Result<(), Error> {
        let root_id = handle(root)?;
        self.objects.insert(root_id);
        for [name, target] in root.values.array_windows() {
            cx.check(Kind::Work, 1)?;
            if name.code != 3 {
                continue;
            }
            let layout = text(name) == Some(b"ACAD_LAYOUT");
            if !layout && text(name) != Some(b"ACAD_PLOTSTYLENAME") {
                continue;
            }
            if !matches!(target.code, 350 | 360) {
                return Err(Error::Unsupported);
            }
            let Data::Handle(id) = target.data else {
                return Err(Error::Unsupported);
            };
            let dict = db.get(id).ok_or(Error::Unsupported)?;
            if dict.owner() != Some(root_id)
                || !(if layout {
                    dict.kind == b"DICTIONARY"
                } else {
                    dict.kind == b"ACDBDICTIONARYWDFLT"
                })
            {
                return Err(Error::Unsupported);
            }
            if !self.objects.insert(id) {
                return Err(Error::Unsupported);
            }
            for [_, target] in dict
                .values
                .array_windows()
                .filter(|[name, _]| name.code == 3)
            {
                cx.check(Kind::Work, 1)?;
                if !matches!(target.code, 350 | 360) {
                    return Err(Error::Unsupported);
                }
                let Data::Handle(child) = target.data else {
                    return Err(Error::Unsupported);
                };
                let leaf = db.get(child).ok_or(Error::Unsupported)?;
                if leaf.owner() != Some(id) {
                    return Err(Error::Unsupported);
                }
                if layout {
                    if leaf.kind != b"LAYOUT" {
                        return Err(Error::Unsupported);
                    }
                    let block = leaf
                        .values
                        .iter()
                        .rev()
                        .find_map(|v| match (v.code, &v.data) {
                            (330, Data::Handle(h)) => Some(*h),
                            _ => None,
                        })
                        .ok_or(Error::Unsupported)?;
                    if !db.get(block).is_some_and(|b| b.kind == b"BLOCK_RECORD")
                        || self.layouts.insert(block, child).is_some()
                    {
                        return Err(Error::Unsupported);
                    }
                } else if leaf.kind != b"ACDBPLACEHOLDER" {
                    return Err(Error::Unsupported);
                }
                if !self.objects.insert(child) {
                    return Err(Error::Unsupported);
                }
            }
        }
        self.kept.extend(self.objects.iter().copied());
        Ok(())
    }
}

fn encode<'a>(
    db: &'a AcadDb,
    plan: &Plan<'a>,
    input: &DxfSource<'_>,
    limit: u64,
    cx: &mut Cx<'_>,
) -> Result<ReencodedDxf, Error> {
    let mut w = Writer::new(plan, input, cx);
    w.section(b"HEADER")?;
    for (name, values) in db.variables() {
        if name == b"$HANDSEED" {
            continue;
        }
        w.tag(9, name)?;
        for v in values {
            w.value(v)?;
        }
    }
    w.tag(9, b"$HANDSEED")?;
    w.hex(5, plan.next_handle)?;
    w.end()?;
    // DXF Reference「CLASS」。使うクラスの宣言は DWG から移し、個数は実際に書く対象から数える。
    w.section(b"CLASSES")?;
    for &(class, count) in &input.classes {
        w.tag(0, b"CLASS")?;
        w.tag(1, &class.name)?;
        w.tag(2, &class.cpp)?;
        w.tag(3, &class.app)?;
        w.integer(90, u64::from(class.flags))?;
        w.integer(91, count)?;
        w.integer(280, u64::from(class.proxy))?;
        w.integer(281, u64::from(class.entity))?;
    }
    w.end()?;
    w.section(b"TABLES")?;
    for &(name, class) in TABLES {
        let Some(rows) = plan.tables.get(name) else {
            continue;
        };
        let control = db
            .get(owner(rows.first().ok_or(Error::Unsupported)?)?)
            .ok_or(Error::Unsupported)?;
        w.table(control, name, rows.len())?;
        for object in rows {
            w.table_record(object, class, db)?;
        }
        w.tag(0, b"ENDTAB")?;
    }
    w.end()?;
    w.section(b"BLOCKS")?;
    for r in plan
        .tables
        .get(b"BLOCK_RECORD".as_slice())
        .ok_or(Error::Unsupported)?
    {
        let name = string(r, 2)?;
        let block = plan.blocks.get(&handle(r)?).ok_or(Error::Unsupported)?;
        w.entity(block.begin.ok_or(Error::Unsupported)?, Some(name))?;
        for &object in block
            .entities
            .iter()
            .filter(|o| o.section == Section::Blocks)
        {
            w.entity(object, None)?;
        }
        w.entity(block.end.ok_or(Error::Unsupported)?, None)?;
    }
    w.end()?;
    w.section(b"ENTITIES")?;
    for (id, block) in &plan.blocks {
        for &entity in block
            .entities
            .iter()
            .filter(|o| o.section == Section::Entities)
        {
            if !plan.layouts.contains_key(id) {
                return Err(Error::Unsupported);
            }
            w.entity(entity, None)?;
        }
    }
    w.end()?;
    w.section(b"OBJECTS")?;
    // NOD は OBJECTS の最初に置く（DXF Reference「OBJECTS Section」）。
    let root = db
        .objects()
        .iter()
        .find(|o| o.kind == b"DICTIONARY" && o.owner() == Some(0))
        .ok_or(Error::Unsupported)?;
    w.metadata(root, db)?;
    for r in db.objects() {
        if !std::ptr::eq(r, root)
            && r.section == Section::Objects
            && r.handle().is_some_and(|h| plan.objects.contains(&h))
        {
            w.metadata(r, db)?;
        }
    }
    w.end()?;
    w.tag(0, b"EOF")?;
    let (bytes, references) = w.finish();
    let mut report = Report::new(limit);
    let lost_references = lost_reference_handles(&references, cx)?;
    // ここからは対象外への切り替えをしない。書けた範囲だけについて減ったものを報告する。
    for obj in db.objects() {
        cx.check(Kind::Work, 1)?;
        // 種類の名前を持たないレコードは、DWG のリーダーが元の型番号・場所で既に診断した。
        let kept = obj.handle().is_some_and(|h| plan.kept.contains(&h));
        let changed = obj.handle().is_some_and(|h| lost_references.contains(&h));
        let fidelity = if !kept {
            ConversionFidelity::Dropped
        } else if changed {
            ConversionFidelity::Drawn
        } else {
            ConversionFidelity::Native
        };
        let geometry = kept
            && matches!(obj.section, Section::Blocks | Section::Entities)
            && !matches!(obj.kind.as_slice(), b"BLOCK" | b"ENDBLK");
        let mut report_source = source(obj);
        if let Some(handle) = obj.handle() {
            report_source = report_source.with_key(zumen_base::Key::Handle(handle));
        }
        report.record_with_reasons(
            report_kind(&obj.kind),
            fidelity,
            &report_source,
            u64::from(geometry),
            if changed { &["reference_dropped"] } else { &[] },
            cx,
        )?;
        if !obj.kind.is_empty() && !kept {
            cx.report(
                Diagnostic::new(codes::DWG_DXF_RECORD_DROPPED)
                    .with("kind", String::from_utf8_lossy(&obj.kind).as_ref())
                    .at(source(obj)),
            );
        }
    }
    for (object, code) in references {
        cx.check(Kind::Work, 1)?;
        cx.report(
            Diagnostic::new(codes::DWG_DXF_REFERENCE_DROPPED)
                .with("code", i64::from(code))
                .at(source(object)),
        );
    }
    cx.report(Diagnostic::new(codes::DWG_DXF_HEADER_DROPPED));
    report.record_with_reasons(
        "header",
        ConversionFidelity::Dropped,
        &Ref::default(),
        0,
        &["unmapped_header"],
        cx,
    )?;
    Ok(ReencodedDxf {
        bytes,
        elements: plan.elements,
        report: report.finish(cx)?,
    })
}

fn lost_reference_handles(
    references: &[(&Object, i16)],
    cx: &mut Cx<'_>,
) -> Result<BTreeSet<u64>, Error> {
    let mut handles = BTreeSet::new();
    for (object, _) in references {
        let search = u64::from(handles.len().saturating_add(1).ilog2()).saturating_add(1);
        cx.check(Kind::Work, search)?;
        if let Some(handle) = object.handle()
            && !handles.contains(&handle)
        {
            // ハンドルの索引は、環境によらない固定の論理量で確保前に数える。
            cx.check(Kind::DecodedBytes, 128)?;
            handles.insert(handle);
        }
    }
    Ok(handles)
}

fn source(object: &Object) -> Ref {
    match object.origin {
        Origin::Dwg { offset, end } => Ref::at_bytes(offset as u64, end as u64),
        _ => Ref::default(),
    }
}

// 集計の種類は固定する。未知の型名は既存の欠落の診断で返し、入力ごとに集計のキーを増やさない。
fn report_kind(kind: &[u8]) -> &'static str {
    match kind {
        b"TABLE" => "TABLE",
        b"VPORT" => "VPORT",
        b"LTYPE" => "LTYPE",
        b"LAYER" => "LAYER",
        b"STYLE" => "STYLE",
        b"APPID" => "APPID",
        b"DIMSTYLE" => "DIMSTYLE",
        b"BLOCK_RECORD" => "BLOCK_RECORD",
        b"BLOCK" => "BLOCK",
        b"ENDBLK" => "ENDBLK",
        b"ARC_DIMENSION" => "ARC_DIMENSION",
        b"ARC" => "ARC",
        b"CIRCLE" => "CIRCLE",
        b"LINE" => "LINE",
        b"SOLID" => "SOLID",
        b"TRACE" => "TRACE",
        b"POINT" => "POINT",
        b"MTEXT" => "MTEXT",
        b"DICTIONARY" => "DICTIONARY",
        b"ACDBDICTIONARYWDFLT" => "ACDBDICTIONARYWDFLT",
        b"LAYOUT" => "LAYOUT",
        b"ACDBPLACEHOLDER" => "ACDBPLACEHOLDER",
        _ => "record",
    }
}
