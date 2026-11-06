//! DWG の共有オブジェクト表から R2004 DXF を書く（決定 0008・0023）。対応する図形と参照を
//! すべて書けるときだけこの経路を使う。試行中のバイト列と欠落の診断は、成功するまで公開しない。

use std::collections::{BTreeMap, BTreeSet};

use zumen_base::{Cx, Diagnostic, Kind, Ref, codes};
use zumen_sdk::Error;

use crate::db::{AcadDb, Data, Object, Origin, Section, Value};

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
}

impl AcadDb {
    /// R2004 DWG の弧長寸法とその描画ブロックを、同じ版の DXF に再符号化する。
    /// 対象外の図形や解けない参照があれば `None` を返し、呼び出し元が既存の変換経路を選べる。
    pub fn reencode_r2004_dxf(&self, cx: &mut Cx<'_>) -> Result<Option<ReencodedDxf>, Error> {
        match encode(self, cx) {
            Ok(out) => Ok(Some(out)),
            Err(Error::Unsupported) => Ok(None),
            Err(Error::Stopped(stop)) => {
                cx.report(stop.to_diagnostic());
                Err(Error::Stopped(stop))
            }
            Err(other) => Err(other),
        }
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

// DXF Reference「Symbol Table Group Codes」。匿名ブロックと VPORT の先頭の * は予約名に使う。
// 表の名前の照合は ASCII から扱う。複数バイトの名前はコード・ページに対応する既存の経路に渡す。
fn valid_name(table: &[u8], name: &[u8]) -> bool {
    let value = if matches!(table, b"BLOCK_RECORD" | b"VPORT") {
        name.strip_prefix(b"*").unwrap_or(name)
    } else {
        name
    };
    name.is_ascii()
        && !value.is_empty()
        && name.len() <= 255
        && name.trim_ascii() == name
        && !value
            .iter()
            .any(|b| b.is_ascii_control() || br#"<>/\:;?*|=,"`"#.contains(b))
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
    tables: BTreeMap<&'a [u8], Vec<&'a Object>>,
    names: BTreeMap<(&'a [u8], Vec<u8>), &'a Object>,
    blocks: BTreeMap<u64, Block<'a>>,
    layouts: BTreeMap<u64, u64>,
    objects: BTreeSet<u64>,
    kept: BTreeSet<u64>,
    dimensions: u64,
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
            if object.kind == b"ARC_DIMENSION" {
                has_dimension = true;
                break;
            }
        }
        if !has_dimension {
            return Err(Error::Unsupported);
        }
        let mut plan = Self {
            tables: BTreeMap::new(),
            names: BTreeMap::new(),
            blocks: BTreeMap::new(),
            layouts: BTreeMap::new(),
            objects: BTreeSet::new(),
            kept: BTreeSet::new(),
            dimensions: 0,
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
                    if !valid_name(&object.kind, name) {
                        return Err(Error::Unsupported);
                    }
                    cx.check(Kind::Work, name.len() as u64)?;
                    if plan
                        .names
                        .insert((&object.kind, name.to_ascii_uppercase()), object)
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
                        || object.kind == b"ARC_DIMENSION" && object.section != Section::Entities
                    {
                        return Err(Error::Unsupported);
                    }
                    let block = plan.blocks.entry(owner(object)?).or_default();
                    match object.kind.as_slice() {
                        b"BLOCK" if block.begin.replace(object).is_none() => (),
                        b"ENDBLK" if block.end.replace(object).is_none() => (),
                        b"BLOCK" | b"ENDBLK" => return Err(Error::Unsupported),
                        kind => {
                            plan.elements += 1;
                            plan.dimensions += u64::from(kind == b"ARC_DIMENSION");
                            if object.section == Section::Blocks {
                                block.entities.push(object);
                            }
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
        for name in [b"*MODEL_SPACE".as_slice(), b"*PAPER_SPACE"] {
            let space = plan.named(b"BLOCK_RECORD", name, cx)?;
            if !plan.layouts.contains_key(&handle(space)?) {
                return Err(Error::Unsupported);
            }
        }
        Ok(plan)
    }

    fn named(&self, table: &'static [u8], name: &[u8], cx: &mut Cx<'_>) -> Result<&Object, Error> {
        cx.check(Kind::Work, (name.len() as u64).saturating_add(1))?;
        self.names
            .get(&(table, name.to_ascii_uppercase()))
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

fn encode(db: &AcadDb, cx: &mut Cx<'_>) -> Result<ReencodedDxf, Error> {
    let plan = Plan::new(db, cx)?;
    let mut w = Writer::new(&plan, cx);
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
    // DXF Reference「CLASS」。R2004 の弧長寸法のクラス名・アプリケーション・フラグは
    // AutoCAD で保存した公開 DXF の CLASS と照合した（docs/formats/dxf.md）。
    w.section(b"CLASSES")?;
    w.tag(0, b"CLASS")?;
    w.tag(1, b"ARC_DIMENSION")?;
    w.tag(2, b"AcDbArcDimension")?;
    w.tag(3, b"ObjectDBX Classes")?;
    w.tag(90, b"1025")?;
    w.integer(91, plan.dimensions)?;
    w.tag(280, b"0")?;
    w.tag(281, b"1")?;
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
        for object in &block.entities {
            w.entity(object, None)?;
        }
        w.entity(block.end.ok_or(Error::Unsupported)?, None)?;
    }
    w.end()?;
    w.section(b"ENTITIES")?;
    for obj in db
        .objects()
        .iter()
        .filter(|r| r.section == Section::Entities)
    {
        if !plan.layouts.contains_key(&owner(obj)?) {
            return Err(Error::Unsupported);
        }
        w.entity(obj, None)?;
    }
    w.end()?;
    w.section(b"OBJECTS")?;
    // NOD は OBJECTS の最初に置く（DXF Reference「OBJECTS Section」）。
    let root = db
        .objects()
        .iter()
        .find(|o| o.kind == b"DICTIONARY" && o.owner() == Some(0))
        .ok_or(Error::Unsupported)?;
    w.metadata(root)?;
    for r in db.objects() {
        if !std::ptr::eq(r, root)
            && r.section == Section::Objects
            && r.handle().is_some_and(|h| plan.objects.contains(&h))
        {
            w.metadata(r)?;
        }
    }
    w.end()?;
    w.tag(0, b"EOF")?;
    let (bytes, references) = w.finish();
    // ここからは対象外への切り替えをしない。書けた範囲だけについて減ったものを報告する。
    for obj in db.objects() {
        cx.check(Kind::Work, 1)?;
        // 種類の名前を持たないレコードは、DWG のリーダーが元の型番号・場所で既に診断した。
        if !obj.kind.is_empty() && !obj.handle().is_some_and(|h| plan.kept.contains(&h)) {
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
    Ok(ReencodedDxf {
        bytes,
        elements: plan.elements,
    })
}

fn source(object: &Object) -> Ref {
    match object.origin {
        Origin::Dwg { offset, end } => Ref::at_bytes(offset as u64, end as u64),
        _ => Ref::default(),
    }
}
