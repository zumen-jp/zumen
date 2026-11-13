//! DWG のオブジェクトから、系統で共有するオブジェクトの表を作る（決定 0023）。
//!
//! どのオブジェクトにも、マップのハンドル（5。DIMSTYLE は 105）を置く。所有者（330）は、ハンドルの並びの
//! 先頭から読む（空間の図形は、ヘッダーの変数が指す空間のブロックのレコード）。図形は、画層の名前（8）と、共通のフィールドのうち既定の値でないものを置く。図形ごとのフィールドは、
//! 今は TEXT・ATTRIB・ATTDEF・TOLERANCE・LEADER・MLINE・VIEWPORT・INSERT・LINE・ARC・CIRCLE・POINT・MTEXT・SOLID・TRACE・3DFACE・
//! SHAPE・ELLIPSE・RAY・XLINE・LWPOLYLINE・SPLINE・HATCH・POLYLINE・VERTEX・DIMENSION を読む。表のレコードは、名前（2）と LAYER・STYLE・LTYPE・APPID・VIEW・UCS・VPORT・DIMSTYLE のフィールドを読む。
//! オブジェクトは、今は MLINESTYLE・DICTIONARY・ACDBDICTIONARYWDFLT・XRECORD・GROUP・DICTIONARYVAR・SCALE・LAYOUT・SPATIAL_FILTER などのフィールドを読む。
//! 図形が表のレコードをハンドルで指すフィールド（画層・線種・文字スタイル・ブロック・寸法スタイル）は、レコードの名前に直して置く。
//! 型が 500 以上のオブジェクトは、番号が同じクラスで種類を決める。
//! 読めないオブジェクトは、ハンドルだけを持たせて置き、診断を返す。

//!
//! このファイルは、オブジェクトの表の組み立てと、共通フィールド・ハンドルの配置を持つ。種類ごとの読み取りとビット位置の走査は、子のモジュールに分けて置く。
//!
//! - `layout`: 書き戻し・再符号化が参照する共通ヘッダーとハンドルのビット位置
//! - `records`: 表のレコード（LAYER・STYLE・LTYPE・VIEW・UCS・VPORT・DIMSTYLE・BLOCK_RECORD・APPID）
//! - `non_entities`: 図形でないオブジェクト（DICTIONARY・XRECORD・LAYOUT・GROUP・MLINESTYLE・TABLESTYLE など）
//! - `annotations`: 文字・属性・引出線・マルチライン・ビューポート・挿入・寸法
//! - `tables`: IMAGE・MULTILEADER・ACAD_TABLE
//! - `geometry`: 線・円弧・楕円・ポリライン・スプライン・頂点・ハッチング
use std::collections::BTreeMap;
use std::ops::Range;

use super::bits::Bits;
use super::header::{self, Header};
use super::kinds::{
    APPID, APPID_CONTROL, ARC, ATTDEF, ATTRIB, BLOCK, BLOCK_RECORD, BLOCK_RECORD_CONTROL, CIRCLE,
    DICTIONARY, DIMENSION_ALIGNED, DIMENSION_ANGULAR_2LINE, DIMENSION_ANGULAR_3PT,
    DIMENSION_DIAMETER, DIMENSION_LINEAR, DIMENSION_ORDINATE, DIMENSION_RADIUS, DIMSTYLE,
    DIMSTYLE_CONTROL, ELLIPSE, ENDBLK, FACE3D, GROUP, INSERT, LAYER, LAYER_CONTROL, LEADER, LINE,
    LTYPE, LTYPE_CONTROL, LWPOLYLINE, MINSERT, MLINE, MLINESTYLE, MTEXT, POINT, POLYLINE_2D,
    POLYLINE_3D, POLYLINE_MESH, POLYLINE_PFACE, RAY, SEQEND, SHAPE, SOLID, SPLINE, STYLE,
    STYLE_CONTROL, TEXT, TOLERANCE, TRACE, UCS, UCS_CONTROL, VERTEX_2D, VERTEX_3D, VERTEX_MESH,
    VERTEX_PFACE, VERTEX_PFACE_FACE, VIEW, VIEW_CONTROL, VIEWPORT, VP_ENT_HDR, VP_ENT_HDR_CONTROL,
    VPORT, VPORT_CONTROL, XLINE,
};
use super::{COMMON_OBJECT, DwgFile, Entry, Release, stopped_at};
use crate::db::{AcadDb, Data, DataType, Object, Origin, Section, Value, caret};
use zumen_base::{Cx, Diagnostic, Kind, Ref, SpecRef, codes};
use zumen_sdk::Error;

mod annotations;
mod geometry;
mod layout;
mod non_entities;
mod records;
mod tables;

pub(super) use self::annotations::leader;
use self::annotations::*;
pub(super) use self::geometry::hatch_gradient;
use self::geometry::*;
pub(super) use self::layout::*;
use self::non_entities::*;
use self::records::*;
pub(super) use self::records::{DIM_FIELDS, Dim, dim_fields, view, vport};
pub(super) use self::tables::multileader;
use self::tables::*;

const COMMON_ENTITY: SpecRef = SpecRef {
    spec: "ods_dwg_5_4_1",
    section: "20.4.1 Common entity data",
};

const EXTENDED_DATA: SpecRef = SpecRef {
    spec: "ods_dwg_5_4_1",
    section: "28 Extended Entity Data (Extended Object Data)",
};
const COMMON_ENTITY_HANDLES: SpecRef = SpecRef {
    spec: "ods_dwg_5_4_1",
    section: "20.4.2 Common entity handle data",
};

/// 型から、DXF の名前と属する所。クラスで決まる型（500 以上）と、知らない型は `None`
fn kind(type_code: u16) -> Option<(&'static [u8], Section)> {
    let entity = |name: &'static [u8]| Some((name, Section::Entities));
    let table = |name: &'static [u8]| Some((name, Section::Tables));
    let object = |name: &'static [u8]| Some((name, Section::Objects));
    match type_code {
        TEXT => entity(b"TEXT"),
        ATTRIB => entity(b"ATTRIB"),
        ATTDEF => entity(b"ATTDEF"),
        BLOCK => Some((b"BLOCK", Section::Blocks)),
        ENDBLK => Some((b"ENDBLK", Section::Blocks)),
        SEQEND => entity(b"SEQEND"),
        INSERT | MINSERT => entity(b"INSERT"),
        VERTEX_2D..=VERTEX_PFACE_FACE => entity(b"VERTEX"),
        POLYLINE_2D | POLYLINE_3D | POLYLINE_PFACE | POLYLINE_MESH => entity(b"POLYLINE"),
        ARC => entity(b"ARC"),
        CIRCLE => entity(b"CIRCLE"),
        LINE => entity(b"LINE"),
        DIMENSION_ORDINATE..=DIMENSION_DIAMETER => entity(b"DIMENSION"),
        POINT => entity(b"POINT"),
        FACE3D => entity(b"3DFACE"),
        SOLID => entity(b"SOLID"),
        TRACE => entity(b"TRACE"),
        SHAPE => entity(b"SHAPE"),
        VIEWPORT => entity(b"VIEWPORT"),
        ELLIPSE => entity(b"ELLIPSE"),
        SPLINE => entity(b"SPLINE"),
        0x25 => entity(b"REGION"),
        0x26 => entity(b"3DSOLID"),
        0x27 => entity(b"BODY"),
        RAY => entity(b"RAY"),
        XLINE => entity(b"XLINE"),
        DICTIONARY => object(b"DICTIONARY"),
        0x2B => entity(b"OLEFRAME"),
        MTEXT => entity(b"MTEXT"),
        TOLERANCE => entity(b"TOLERANCE"),
        LEADER => entity(b"LEADER"),
        MLINE => entity(b"MLINE"),
        BLOCK_RECORD_CONTROL | LAYER_CONTROL | STYLE_CONTROL | LTYPE_CONTROL | VIEW_CONTROL
        | UCS_CONTROL | VPORT_CONTROL | APPID_CONTROL | DIMSTYLE_CONTROL => table(b"TABLE"),
        BLOCK_RECORD => table(b"BLOCK_RECORD"),
        LAYER => table(b"LAYER"),
        STYLE => table(b"STYLE"),
        LTYPE => table(b"LTYPE"),
        VIEW => table(b"VIEW"),
        UCS => table(b"UCS"),
        VPORT => table(b"VPORT"),
        APPID => table(b"APPID"),
        DIMSTYLE => table(b"DIMSTYLE"),
        // DXF にない表。名前は DWG のもの
        VP_ENT_HDR_CONTROL => table(b"VP_ENT_HDR_CONTROL"),
        VP_ENT_HDR => table(b"VP_ENT_HDR"),
        GROUP => object(b"GROUP"),
        MLINESTYLE => object(b"MLINESTYLE"),
        0x4A => entity(b"OLE2FRAME"),
        0x4B => object(b"DUMMY"),
        0x4C => object(b"LONG_TRANSACTION"),
        LWPOLYLINE => entity(b"LWPOLYLINE"),
        0x4E => entity(b"HATCH"),
        0x4F => object(b"XRECORD"),
        0x50 => object(b"ACDBPLACEHOLDER"),
        0x51 => object(b"VBA_PROJECT"),
        0x52 => object(b"LAYOUT"),
        0x1F2 => entity(b"ACAD_PROXY_ENTITY"),
        0x1F3 => object(b"ACAD_PROXY_OBJECT"),
        _ => None,
    }
}

impl super::Class {
    fn kind(&self) -> (&[u8], Section) {
        (
            self.name.as_slice(),
            if self.entity {
                Section::Entities
            } else {
                Section::Objects
            },
        )
    }
}

impl DwgFile {
    /// 型から、DXF の名前と属する所。500 以上の型は、番号が同じクラスで決める。知らない型は `None`
    pub(super) fn kind(&self, type_code: u16) -> Option<(&[u8], Section)> {
        if let Some(found) = kind(type_code) {
            return Some(found);
        }
        self.classes
            .iter()
            .find(|c| c.number == type_code)
            .map(super::Class::kind)
    }

    /// オブジェクトの表を作る。予算の上限か中断の合図で止めたときだけ失敗する
    pub fn objects(&self, cx: &mut Cx) -> Result<AcadDb, Error> {
        let header = self
            .header_section()
            .and_then(|(src, span)| header::header(src, span, self.release, cx));
        // オブジェクトごとに全クラスを探索しない。重複した番号を読むときは従来通り最初の宣言を使う。
        let mut classes = BTreeMap::new();
        for class in &self.classes {
            cx.check(Kind::Work, 1)
                .map_err(|stop| super::stopped(cx, stop))?;
            classes.entry(class.number).or_insert_with(|| class.kind());
        }
        let mut found = Vec::with_capacity(self.entries.len());
        for entry in &self.entries {
            let place = Ref::at_bytes(entry.span.start as u64, entry.span.end as u64);
            cx.check(Kind::Work, 1)
                .map_err(|stop| stopped_at(cx, stop, place))?;
            let kind = kind(entry.type_code).or_else(|| classes.get(&entry.type_code).cloned());
            let fields = kind.as_ref().and_then(|(name, section)| {
                // BLOCK と ENDBLK は、DXF では BLOCKS のセクションに属するが、DWG では図形として書かれる
                let entity = matches!(section, Section::Entities | Section::Blocks);
                read(self.data(entry), self.release, entry, name, entity, header)
            });
            found.push((kind, fields));
        }
        if let Some(header) = header {
            let records = self
                .entries
                .iter()
                .zip(found.iter_mut())
                .filter(|(entry, _)| entry.type_code == BLOCK_RECORD)
                .filter(|(entry, _)| entry.handle != header.model && entry.handle != header.paper)
                .map(|(_, (_, fields))| fields.as_mut());
            number_anonymous(records);
        }
        // 図形は表のレコードとマルチラインスタイルをハンドルで指すので、先にその型と名前を集める
        let names: BTreeMap<u64, (u16, Vec<u8>)> = self
            .entries
            .iter()
            .zip(&found)
            .filter(|(entry, _)| is_table_record(entry.type_code) || entry.type_code == MLINESTYLE)
            .filter_map(|(entry, (_, fields))| {
                let name = fields.as_ref()?.values.iter().find(|v| v.code == 2)?;
                match &name.data {
                    Data::Text(name) => Some((entry.handle, (entry.type_code, name.clone()))),
                    _ => None,
                }
            })
            .collect();
        // DBCOLOR ごとの、RGB の値（420）と名前（430）
        let books: BTreeMap<u64, Vec<Value>> = self
            .entries
            .iter()
            .zip(&found)
            .filter(|(_, (kind, _))| kind.as_ref().is_some_and(|(kind, _)| *kind == b"DBCOLOR"))
            .filter_map(|(entry, (_, fields))| {
                let values = &fields.as_ref()?.values;
                let book: Vec<Value> = values
                    .iter()
                    .filter(|v| matches!(v.code, 420 | 430))
                    .cloned()
                    .collect();
                Some((entry.handle, book))
            })
            .collect();
        let mut objects = Vec::with_capacity(self.entries.len());
        // 所有者ごとの、これまでのビューポートの数
        let mut viewports: BTreeMap<u64, i64> = BTreeMap::new();
        // オブジェクトごとの拡張データ。ほかの値を移した後で、終わりに置く
        let mut extended = Vec::with_capacity(self.entries.len());
        for (entry, (kind, fields)) in self.entries.iter().zip(found) {
            let at = Ref::at_bytes(entry.span.start as u64, entry.span.end as u64);
            let handle_code = if entry.type_code == DIMSTYLE { 105 } else { 5 };
            let mut extra = Vec::new();
            let mut values = vec![Value {
                code: handle_code,
                data: Data::Handle(entry.handle),
            }];
            let (kind, section) = match kind {
                Some((kind, section)) => {
                    let entity = matches!(section, Section::Entities | Section::Blocks);
                    match fields {
                        Some(fields) => {
                            let (own, xdata) = fields.place(&names, &books, entry, cx, &at);
                            values.extend(own);
                            extra = xdata;
                        }
                        None => cx.report(
                            Diagnostic::new(codes::DWG_OBJECT_INVALID)
                                .with("type", i64::from(entry.type_code))
                                .at(at)
                                .cite(if entity { COMMON_ENTITY } else { COMMON_OBJECT }),
                        ),
                    }
                    (kind.to_vec(), section)
                }
                None => {
                    cx.report(
                        Diagnostic::new(codes::DWG_TYPE_UNSUPPORTED)
                            .with("type", i64::from(entry.type_code))
                            .at(at),
                    );
                    (Vec::new(), Section::Other(Vec::new()))
                }
            };
            let mut object = Object {
                kind,
                section,
                values,
                origin: Origin::Dwg {
                    offset: entry.span.start,
                    end: entry.span.end,
                },
            };
            // ビューポートの番号（69）は DWG に書かれない。所有者ごとにオブジェクトのマップの順（ハンドルの順）で
            // 1 から振る。1 は用紙そのものを表すビューポート（推定。docs/formats/dwg.md）
            if let (b"VIEWPORT", Some(owner)) = (object.kind.as_slice(), object.owner()) {
                let number = viewports.entry(owner).or_default();
                *number += 1;
                object.values.push(int(69, *number));
            }
            objects.push(object);
            extended.push(extra);
        }
        to_blocks(&mut objects);
        if let Some(header) = header {
            in_blocks(&mut objects, header);
        }
        for (object, xdata) in objects.iter_mut().zip(extended) {
            object.values.extend(xdata);
        }
        let mut db = AcadDb::default();
        for object in objects {
            db.push(object);
        }
        db.set_variable(b"$ACADVER", vec![text_value(1, self.version())]);
        if let Some(name) = code_page_name(self.code_page) {
            db.set_variable(b"$DWGCODEPAGE", vec![text_value(3, name)]);
        }
        if let Some(header) = header {
            let [x0, y0, x1, y1] = header.limits;
            db.set_variable(b"$INSUNITS", vec![int(70, i64::from(header.units))]);
            db.set_variable(b"$PLIMMIN", vec![real(10, x0), real(20, y0)]);
            db.set_variable(b"$PLIMMAX", vec![real(10, x1), real(20, y1)]);
            db.set_variable(b"$LTSCALE", vec![real(40, header.line_type_scale)]);
            db.set_variable(b"$PDMODE", vec![int(70, i64::from(header.point_mode))]);
            db.set_variable(b"$PDSIZE", vec![real(40, header.point_size)]);
        }
        Ok(db)
    }
}

/// DWG は、名前のないブロック（`*D`・`*U`・`*T` など）と、今の配置でない用紙空間（`*Paper_Space`）の
/// 名前に番号を持たない。DXF と同じく、モデル空間と今の用紙空間を除いたブロックのレコードの、
/// オブジェクトのマップの順（ハンドルの順）の位置を番号として名前の後ろに付ける（推定。サンプルの DWG と DXF の
/// 比較で確かめた。docs/formats/dwg.md）。`records` は、その順のレコードのフィールド
fn number_anonymous<'a>(records: impl Iterator<Item = Option<&'a mut Fields>>) {
    for (number, fields) in records.enumerate() {
        let Some(name) = fields.and_then(|fields| fields.values.iter_mut().find(|v| v.code == 2))
        else {
            continue;
        };
        if let Data::Text(name) = &mut name.data
            && (name.eq_ignore_ascii_case(b"*Paper_Space")
                || name.split_first().is_some_and(|(first, rest)| {
                    *first == b'*' && !rest.is_empty() && rest.iter().all(u8::is_ascii_alphabetic)
                }))
        {
            name.extend_from_slice(number.to_string().as_bytes());
        }
    }
}

/// BLOCK_RECORD から読んだフラグ・基点などを、そのレコードを所有者に持つ BLOCK の名前の後ろへ移す
fn to_blocks(objects: &mut [Object]) {
    let mut moved: BTreeMap<u64, Vec<Value>> = BTreeMap::new();
    for object in objects.iter_mut() {
        if object.kind != b"BLOCK_RECORD" {
            continue;
        }
        let Some(handle) = object.handle() else {
            continue;
        };
        // 70 は、最初のもの（フラグ）だけを移す。後の 70 は挿入の単位で、BLOCK_RECORD に残す
        let mut flags = false;
        let (block, own) = object.values.drain(..).partition(|value| {
            let first = value.code != 70 || !flags;
            flags |= value.code == 70;
            first && BLOCK_FIELDS.contains(&value.code)
        });
        object.values = own;
        moved.insert(handle, block);
    }
    for object in objects.iter_mut() {
        if object.kind != b"BLOCK" {
            continue;
        }
        if let Some(values) = object.owner().and_then(|owner| moved.remove(&owner)) {
            object.values.extend(values);
        }
    }
}

/// DWG は図形の所属を書かないので、DXF の ENTITIES と同じく、所有者をたどってモデル空間か用紙空間の
/// ブロックのレコードに着く図形だけを図形の所に置き、ほかのブロックのレコードに着く図形はブロックの所に移す。
/// 所有者が分からない図形は、図形の所に残す
fn in_blocks(objects: &mut [Object], header: Header) {
    let owners: BTreeMap<u64, Option<u64>> = objects
        .iter()
        .filter(|object| object.section == Section::Entities)
        .filter_map(|object| Some((object.handle()?, object.owner())))
        .collect();
    let moved: Vec<bool> = objects
        .iter()
        .map(|object| {
            if object.section != Section::Entities {
                return false;
            }
            let mut owner = object.owner();
            // 図形の所有者が図形（VERTEX の POLYLINE、ATTRIB の INSERT など）なら、その所有者へ。輪は数えて止める
            for _ in 0..owners.len() {
                match owner.and_then(|o| owners.get(&o)) {
                    Some(next) => owner = *next,
                    None => break,
                }
            }
            owner.is_some_and(|o| o != header.model && o != header.paper)
        })
        .collect();
    for (object, moved) in objects.iter_mut().zip(moved) {
        if moved {
            object.section = Section::Blocks;
        }
    }
}

/// オブジェクトから読んだフィールド
struct Fields {
    /// 所有者（330）
    owner: Option<u64>,
    /// 図形の拡張辞書（`{ACAD_XDICTIONARY` の組の 360）
    xdic: Option<u64>,
    /// 図形の画層のハンドル
    layer: Option<u64>,
    /// 所有者と画層より後の値
    values: Vec<Value>,
    /// 値の後に、名前に直して置く表のレコード
    records: Vec<RecordRef>,
    /// 登録アプリケーションのハンドルごとの拡張データ。読めなければ `None`
    xdata: Vec<(u64, Option<Vec<Xdata>>)>,
    /// 図形の色が指す DBCOLOR のハンドル
    book: Option<u64>,
}

/// 拡張データの値。画層は名前に直して 1003 に置く
enum Xdata {
    Value(Value),
    Layer(u64),
}

/// 表のレコードを指すハンドル。`code` の値として、`record` の型のレコードの名前を置く
struct RecordRef {
    code: i16,
    record: u16,
    handle: u64,
}

/// BLOCK_RECORD から読み、DXF と同じく BLOCK に置くフィールド
const BLOCK_FIELDS: &[i16] = &[70, 10, 20, 30, 1, 4];

/// 名前（2）を持つ表のレコードの型か
fn is_table_record(type_code: u16) -> bool {
    matches!(
        type_code,
        BLOCK_RECORD | LAYER | STYLE | LTYPE | VIEW | UCS | VPORT | APPID | DIMSTYLE | VP_ENT_HDR
    )
}

impl Fields {
    /// ハンドル（5）より後の値と、拡張データ（1001 から）を並べる。画層などは名前に直し、見つからなければ
    /// 診断を返す
    fn place(
        self,
        names: &BTreeMap<u64, (u16, Vec<u8>)>,
        books: &BTreeMap<u64, Vec<Value>>,
        entry: &Entry,
        cx: &mut Cx,
        at: &Ref,
    ) -> (Vec<Value>, Vec<Value>) {
        let mut values = Vec::new();
        // 拡張辞書は、DXF と同じく所有者の前に組で置く
        if let Some(xdic) = self.xdic {
            values.push(text_value(102, b"{ACAD_XDICTIONARY"));
            values.push(Value {
                code: 360,
                data: Data::Handle(xdic),
            });
            values.push(text_value(102, b"}"));
        }
        if let Some(owner) = self.owner {
            values.push(Value {
                code: 330,
                data: Data::Handle(owner),
            });
        }
        if let Some(layer) = self.layer {
            match names.get(&layer).filter(|(record, _)| *record == LAYER) {
                Some((_, name)) => values.push(Value {
                    code: 8,
                    data: Data::Text(name.clone()),
                }),
                None => cx.report(
                    Diagnostic::new(codes::DWG_LAYER_MISSING)
                        .with("type", i64::from(entry.type_code))
                        .at(at.clone())
                        .cite(COMMON_ENTITY_HANDLES),
                ),
            }
        }
        let mut own = self.values;
        // 色見本帳の色は、DXF と同じく DBCOLOR の RGB の値（420）と名前（430）を色の番号の後に置く
        if let Some(book) = self.book.and_then(|handle| books.get(&handle)) {
            let at = own.iter().position(|v| v.code == 62).map_or(0, |at| at + 1);
            own.splice(at..at, book.iter().cloned());
        }
        values.extend(own);
        for r in self.records {
            match names
                .get(&r.handle)
                .filter(|(record, _)| *record == r.record)
            {
                // 線種が ByLayer、文字スタイルが Standard の既定の値なら、DXF と同じく置かない
                Some((_, name))
                    if (r.code == 6 && name.eq_ignore_ascii_case(b"ByLayer"))
                        || (r.code == 7 && name.eq_ignore_ascii_case(b"Standard")) => {}
                Some((_, name)) => values.push(Value {
                    code: r.code,
                    data: Data::Text(name.clone()),
                }),
                None => cx.report(
                    Diagnostic::new(codes::DWG_RECORD_MISSING)
                        .with("type", i64::from(entry.type_code))
                        .with("code", i64::from(r.code))
                        .at(at.clone())
                        .cite(COMMON_ENTITY_HANDLES),
                ),
            }
        }
        // 拡張データ。登録アプリケーションの名前（1001）か画層の名前（1003）がなければ、そのアプリケーションの
        // ものを置かない
        let mut xdata = Vec::new();
        for (app, items) in self.xdata {
            let Some(items) = items else {
                cx.report(
                    Diagnostic::new(codes::DWG_XDATA_INVALID)
                        .with("type", i64::from(entry.type_code))
                        .at(at.clone())
                        .cite(EXTENDED_DATA),
                );
                continue;
            };
            let name = |handle: u64, record: u16| {
                names
                    .get(&handle)
                    .filter(|(found, _)| *found == record)
                    .map(|(_, name)| name.clone())
            };
            let mut group = Vec::with_capacity(items.len() + 1);
            let mut missing = None;
            match name(app, APPID) {
                Some(name) => group.push(Value {
                    code: 1001,
                    data: Data::Text(name),
                }),
                None => missing = Some(1001),
            }
            for item in items {
                match item {
                    Xdata::Value(value) => group.push(value),
                    Xdata::Layer(handle) => match name(handle, LAYER) {
                        Some(name) => group.push(Value {
                            code: 1003,
                            data: Data::Text(name),
                        }),
                        None => missing = missing.or(Some(1003)),
                    },
                }
            }
            match missing {
                None => xdata.extend(group),
                Some(code) => cx.report(
                    Diagnostic::new(codes::DWG_RECORD_MISSING)
                        .with("type", i64::from(entry.type_code))
                        .with("code", i64::from(code))
                        .at(at.clone())
                        .cite(EXTENDED_DATA),
                ),
            }
        }
        (values, xdata)
    }
}

/// 図形の共通のフィールドのうち、型ごとのフィールドとハンドルの並びを読むのに要るもの
struct Common {
    /// プロキシ画像の開始ビットとバイト数（ODS 5.4.1 の 21 共通の図形）。
    picture: Option<(usize, usize)>,
    /// 所有者の決まり方。0 は所有者のハンドルを持ち、1 は用紙空間、2 はモデル空間の図形
    entmode: u8,
    /// リアクターの数
    reactors: u32,
    /// 拡張辞書のハンドルを持つか
    xdic: bool,
    /// 前後の図形のハンドルを持たないか
    no_links: bool,
    /// 色を DBCOLOR のハンドルで指すか（R2004 から）
    color_handle: bool,
    /// 色（ENC）の始まりのビットの位置
    color_at: usize,
    /// 印刷スタイルの後のハンドルの数。材質（R2007 から、フラグが 3 のとき）と、表示スタイル
    /// （R2010 から、3 つの B のそれぞれが立つとき）
    styles: usize,
    /// 線種のフラグ。0 は ByLayer、1 は ByBlock、2 は Continuous、3 はハンドルで指す
    ltype_flags: u8,
    /// 印刷スタイルのハンドルを持つか（印刷スタイルのフラグが 3）
    plotstyle_handle: bool,
    values: Vec<Value>,
}

/// オブジェクトのデータを読む。読めなければ `None`
fn read(
    data: &[u8],
    release: Release,
    entry: &Entry,
    kind: &[u8],
    entity: bool,
    header: Option<Header>,
) -> Option<Fields> {
    let mut bits = Bits::with_release(data, release);
    // R2010 からは、型は BOT で、ハンドルのストリームの位置はセクションの大きさで決まる。文字列のストリームは
    // ハンドルのストリームの前にある
    let handles_at = match entry.handles {
        Some(at) => {
            bits.bot()?;
            bits.split_strings(at)?;
            at
        }
        None => {
            bits.bs()?;
            let at = usize::try_from(bits.rl()?).ok()?;
            if release == Release::R2007 {
                bits.split_strings(at)?;
            }
            at
        }
    };
    bits.handle()?;
    let xdata = extended_data(&mut bits)?
        .into_iter()
        .map(|(app, bytes)| (app, xdata(&bytes, release)))
        .collect::<Vec<_>>();
    if entity {
        let common = entity_data(&mut bits)?;
        let mut frozen = 0;
        // データの中で、ハンドルのストリームから読むハンドルの数（MULTILEADER）
        let mut slots = 0;
        let own = if bits.pos() <= handles_at {
            match entry.type_code {
                TEXT => text(&mut bits)?,
                ATTRIB | ATTDEF => attribute(&mut bits, entry.type_code == ATTDEF)?,
                TOLERANCE => tolerance(&mut bits)?,
                LEADER => leader(&mut bits)?,
                MLINE => mline(&mut bits)?,
                VIEWPORT => viewport(&mut bits, &mut frozen)?,
                INSERT => insert(&mut bits, false)?,
                MINSERT => insert(&mut bits, true)?,
                MTEXT => {
                    let mut values = mtext(&mut bits)?;
                    if bits.release() >= Release::R2018 {
                        values.extend(mtext_embedded(&mut bits)?);
                    }
                    values
                }
                LINE => line(&mut bits)?,
                ARC => arc(&mut bits)?,
                CIRCLE => circle(&mut bits)?,
                POINT => point(&mut bits)?,
                SOLID | TRACE => solid(&mut bits)?,
                FACE3D => face3d(&mut bits)?,
                SHAPE => shape(&mut bits)?,
                ELLIPSE => ellipse(&mut bits)?,
                RAY | XLINE => ray(&mut bits)?,
                LWPOLYLINE => lwpolyline(&mut bits)?,
                SPLINE => spline(&mut bits)?,
                VERTEX_2D => vertex2d(&mut bits)?,
                VERTEX_3D | VERTEX_MESH | VERTEX_PFACE => vertex3d(&mut bits)?,
                VERTEX_PFACE_FACE => face_record(&mut bits)?,
                POLYLINE_2D => polyline2d(&mut bits)?,
                POLYLINE_3D => polyline3d(&mut bits)?,
                POLYLINE_PFACE => polyface(&mut bits)?,
                POLYLINE_MESH => mesh(&mut bits)?,
                DIMENSION_ORDINATE..=DIMENSION_DIAMETER => dimension(&mut bits, entry.type_code)?,
                // ARC_DIMENSION は型が固定でなく、クラスで決まる
                _ if kind == b"ARC_DIMENSION" => dimension(&mut bits, ARC_DIMENSION)?,
                // HATCH は型が固定でなく、クラスで決まる
                _ if kind == b"HATCH" => hatch(&mut bits)?,
                _ if kind == b"IMAGE" => image(&mut bits, release)?,
                _ if kind == b"MULTILEADER" => multileader(&mut bits, release, &mut slots, None)?,
                _ if kind == b"ACAD_TABLE" && release < Release::R2010 => {
                    table(&mut bits, release, &mut slots)?
                }
                // R2010 からの ACAD_TABLE は、内容を TABLECONTENT の形で持つ（ODS 5.4.1 の 20.4.96.2）
                _ if kind == b"ACAD_TABLE" => table_content(&mut bits, release, &mut slots)?,
                _ => Vec::new(),
            }
        } else {
            Vec::new()
        };
        // 図形のハンドルの並び: 所有者（entmode が 0 のとき。1・2 のときはヘッダーの変数が指す空間）・リアクター・拡張辞書・前後の図形
        // （nolinks が 0 のとき）・色（R2004 で、ENC の 0x4000 が立つとき）・画層・線種と印刷スタイル（フラグが 3 のとき）・型ごとのハンドル
        bits.seek(handles_at);
        let owner = match common.entmode {
            0 => Some(bits.handle()?.resolve(entry.handle)?),
            1 => header.map(|h| h.paper),
            2 => header.map(|h| h.model),
            _ => None,
        };
        for _ in 0..common.reactors {
            bits.handle()?;
        }
        let xdic = if common.xdic {
            Some(bits.handle()?.resolve(entry.handle)?).filter(|handle| *handle != 0)
        } else {
            None
        };
        if !common.no_links {
            bits.handle()?;
            bits.handle()?;
        }
        let book = if common.color_handle {
            Some(bits.handle()?.resolve(entry.handle)?)
        } else {
            None
        };
        let layer = bits.handle()?.resolve(entry.handle)?;
        let mut records = Vec::new();
        let mut values = common.values;
        values.extend(own);
        // 用紙空間の図形は、DXF と同じく 67 = 1 を置く
        if common.entmode == 1 {
            values.push(int(67, 1));
        }
        match common.ltype_flags {
            1 => values.push(text_value(6, b"ByBlock")),
            2 => values.push(text_value(6, b"Continuous")),
            3 => records.push(RecordRef {
                code: 6,
                record: LTYPE,
                handle: bits.handle()?.resolve(entry.handle)?,
            }),
            _ => {}
        }
        if common.plotstyle_handle {
            bits.handle()?;
        }
        for _ in 0..common.styles {
            bits.handle()?;
        }
        // MULTILEADER は、データの中のハンドルを、データの順にハンドルのストリームから読む。仮の値を読んだ値で
        // 置き換え、0 のハンドルと置かないハンドルは除く
        if slots > 0 {
            let mut read = Vec::with_capacity(usize::try_from(slots).ok()?.min(data.len()));
            for _ in 0..slots {
                read.push(bits.handle()?.resolve(entry.handle)?);
            }
            for value in &mut values {
                if let Data::Handle(handle) = &mut value.data
                    && let Some(slot) = handle.checked_sub(SLOT)
                {
                    *handle = *read.get(usize::try_from(slot).ok()?)?;
                }
            }
            // ACAD_TABLE の文字のセルのフィールド（344）は、0 のときだけ文字を持つ。文字を読んだので、0 でなければ
            // 読めないとする。ブロック（2）と文字スタイル（7）は、名前に直す
            if kind == b"ACAD_TABLE" {
                if values
                    .iter()
                    .any(|v| v.code == 344 && v.data != Data::Handle(0))
                {
                    return None;
                }
                let mut kept = Vec::with_capacity(values.len());
                for value in values {
                    match (value.code, &value.data) {
                        (2, Data::Handle(handle)) => records.push(RecordRef {
                            code: 2,
                            record: BLOCK_RECORD,
                            handle: *handle,
                        }),
                        (7, Data::Handle(handle)) => records.push(RecordRef {
                            code: 7,
                            record: STYLE,
                            handle: *handle,
                        }),
                        _ => kept.push(value),
                    }
                }
                values = kept;
            }
            values.retain(|v| !matches!(v.data, Data::Handle(0)));
        }
        // IMAGE は、画像の定義（340）と、そのリアクター（360）のハンドルを持つ。DXF と同じく、表示のフラグ（70）と
        // 切り抜きの境界の種類（71）の前に置く
        if kind == b"IMAGE" {
            for (code, before) in [(340, 70), (360, 71)] {
                let handle = Value {
                    code,
                    data: Data::Handle(bits.handle()?.resolve(entry.handle)?),
                };
                let at = values
                    .iter()
                    .position(|v| v.code == before)
                    .unwrap_or(values.len());
                values.insert(at, handle);
            }
        }
        // HATCH は、輪ごとの境界の図形の数（97）だけ、境界の図形のハンドル（330）を持つ。数の後に置く
        if kind == b"HATCH" {
            let mut placed = Vec::with_capacity(values.len());
            for value in values {
                let count = match (value.code, &value.data) {
                    (97, Data::Int(count)) => *count,
                    _ => 0,
                };
                placed.push(value);
                for _ in 0..count {
                    placed.push(Value {
                        code: 330,
                        data: Data::Handle(bits.handle()?.resolve(entry.handle)?),
                    });
                }
            }
            values = placed;
        }
        // VIEWPORT は、型ごとのハンドルの初めに、ビューポートで凍結した画層（341）を持つ
        for _ in 0..frozen {
            values.push(Value {
                code: 341,
                data: Data::Handle(bits.handle()?.resolve(entry.handle)?),
            });
        }
        // 寸法は、寸法スタイル（3）とブロック（2）の順に指す
        let pointed: &[(i16, u16)] = match entry.type_code {
            TEXT | MTEXT | ATTRIB | ATTDEF => &[(7, STYLE)],
            INSERT | MINSERT => &[(2, BLOCK_RECORD)],
            DIMENSION_ORDINATE..=DIMENSION_DIAMETER => &[(3, DIMSTYLE), (2, BLOCK_RECORD)],
            _ if kind == b"ARC_DIMENSION" => &[(3, DIMSTYLE), (2, BLOCK_RECORD)],
            TOLERANCE => &[(3, DIMSTYLE)],
            LEADER => &[(3, DIMSTYLE)],
            _ => &[],
        };
        // LEADER は寸法スタイルの前に注釈のハンドルを、MLINE はマルチラインスタイルのハンドルを持つ（340）
        if matches!(entry.type_code, LEADER | MLINE) {
            let annotation = bits.handle()?.resolve(entry.handle)?;
            if annotation != 0 {
                values.push(Value {
                    code: 340,
                    data: Data::Handle(annotation),
                });
            }
            // MLINE は、スタイルの名前も 2 に置く
            if entry.type_code == MLINE {
                records.push(RecordRef {
                    code: 2,
                    record: MLINESTYLE,
                    handle: annotation,
                });
            }
        }
        for &(code, record) in pointed {
            let handle = bits.handle()?.resolve(entry.handle)?;
            records.push(RecordRef {
                code,
                record,
                handle,
            });
        }
        Some(Fields {
            owner,
            xdic,
            layer: Some(layer),
            values,
            records,
            xdata,
            book,
        })
    } else {
        let reactors = bits.bl()?;
        // R2004 からは、拡張辞書のハンドルがないことを表す B が続く。R2013 からは、さらに
        // DS のデータを持つかの B が続く
        let xdic = release == Release::R2000 || !bits.bit()?;
        if release >= Release::R2013 {
            bits.bit()?;
        }
        let mut entries = Vec::new();
        // 型ごとのハンドルを置くコード。0 のハンドルは置かない。コードが 0 のハンドルは読み飛ばし、
        // 6 のハンドルは線種の名前に直す
        let mut handles = Vec::new();
        let mut records = Vec::new();
        // SORTENTSTABLE の並べ替えのハンドル。データのストリームにあり、図形のハンドルはハンドルのストリームにある
        let sortents = kind == b"SORTENTSTABLE";
        let mut sorts = Vec::new();
        let mut values = if is_table_record(entry.type_code) {
            table_record(&mut bits, entry.type_code, &mut handles)?
        } else if entry.type_code == MLINESTYLE {
            mlinestyle(&mut bits)?
        } else if entry.type_code == DICTIONARY || kind == b"ACDBDICTIONARYWDFLT" {
            // 既定の項目を持つ辞書は、項目の後に既定の項目のハンドル（340）を持つ
            if kind == b"ACDBDICTIONARYWDFLT" {
                handles.push(340);
            }
            dictionary(&mut bits, &mut entries)?
        } else if kind == b"XRECORD" {
            xrecord(&mut bits)?
        } else if kind == b"DBCOLOR" && release < Release::R2004 {
            // R2000 の DXF は、DBCOLOR に色の値を書かない
            Vec::new()
        } else if kind == b"DBCOLOR" {
            // 色（CMC）だけを持つ。DXF と同じく、番号（62）・RGB の値（420）・名前（430）を置く。真の色は、
            // 図形と同じく番号を置かない（DWG の番号は 0 で、DXF は近い番号を書く）
            let (color, name) = bits.cmc_named()?;
            let mut values = Vec::new();
            match color.rgb {
                Some(rgb) => values.push(int(420, i64::from(rgb))),
                None => values.push(int(62, i64::from(color.index))),
            }
            if let Some(name) = name {
                values.push(text_value(430, &name));
            }
            values
        } else if entry.type_code == GROUP {
            group(&mut bits, &mut handles)?
        } else if kind == b"DICTIONARYVAR" {
            vec![
                int(280, i64::from(bits.rc()?)),
                text_value(1, &bits.text()?),
            ]
        } else if kind == b"SCALE" {
            scale(&mut bits)?
        } else if kind == b"SPATIAL_FILTER" {
            spatial_filter(&mut bits)?
        } else if kind == b"IMAGEDEF" {
            imagedef(&mut bits)?
        } else if kind == b"IMAGEDEF_REACTOR" {
            vec![int(90, i64::from(bits.bl()?))]
        } else if kind == b"LAYOUT" {
            layout(&mut bits, release, &mut handles)?
        } else if kind == b"TABLESTYLE" && release < Release::R2010 {
            // 行ごとの文字スタイル（7）のハンドルは、ハンドルのストリームで仮の値を埋める
            handles.extend([7; 3]);
            tablestyle(&mut bits)?
        } else if sortents {
            for _ in 0..bits.bl()? {
                sorts.push(bits.handle()?.resolve(entry.handle)?);
            }
            Vec::new()
        } else {
            Vec::new()
        };
        // オブジェクトのハンドルの並び: 所有者・リアクター・拡張辞書・型ごとのハンドル
        bits.seek(handles_at);
        let owner = bits.handle()?.resolve(entry.handle)?;
        // 拡張辞書は、図形と同じく DXF の組で置く。0 のハンドルは、ないものとする。型ごとのハンドルを
        // 持たないオブジェクトは、ハンドルのストリームがリアクターと拡張辞書を持たないことがあるので、読めなければ
        // 拡張辞書を持たないとする
        let xdic = (|| {
            for _ in 0..reactors {
                bits.handle()?;
            }
            if !xdic {
                return None;
            }
            bits.handle()?.resolve(entry.handle)
        })()
        .filter(|handle| *handle != 0);
        if !entries.is_empty() || !handles.is_empty() || sortents {
            // SORTENTSTABLE は、ブロックのレコード（330）と、図形（331）と並べ替えのハンドル（5）の組を持つ
            if sortents {
                values.push(Value {
                    code: 330,
                    data: Data::Handle(bits.handle()?.resolve(entry.handle)?),
                });
                for sort in sorts {
                    values.push(Value {
                        code: 331,
                        data: Data::Handle(bits.handle()?.resolve(entry.handle)?),
                    });
                    values.push(Value {
                        code: 5,
                        data: Data::Handle(sort),
                    });
                }
            }
            // 辞書の項目は、名前（3）と指すオブジェクトの組。所有するなら 360、しないなら 350
            let code = if values.iter().any(|v| v.code == 280) {
                360
            } else {
                350
            };
            for name in entries {
                let item = bits.handle()?.resolve(entry.handle)?;
                values.push(text_value(3, &name));
                values.push(Value {
                    code,
                    data: Data::Handle(item),
                });
            }
            for code in handles {
                let handle = bits.handle()?.resolve(entry.handle)?;
                let slot = values
                    .iter_mut()
                    .find(|value| value.code == code && value.data == Data::Handle(SLOT));
                match (code, slot) {
                    (0, _) => {}
                    // TABLESTYLE の行の 7 と LTYPE の要素の 340 は、0 でも区切りとして残す
                    (_, Some(slot)) => slot.data = Data::Handle(handle),
                    (6, None) if handle != 0 => records.push(RecordRef {
                        code,
                        record: LTYPE,
                        handle,
                    }),
                    (_, None) if handle != 0 => values.push(Value {
                        code,
                        data: Data::Handle(handle),
                    }),
                    _ => {}
                }
            }
        }
        Some(Fields {
            owner: Some(owner),
            xdic,
            layer: None,
            values,
            records,
            xdata,
            book: None,
        })
    }
}

/// 拡張データ（EED）を、登録アプリケーションのハンドルとデータのバイト列の組で読む（ODS 5.4.1 の 28）
fn extended_data(bits: &mut Bits) -> Option<Vec<(u64, Vec<u8>)>> {
    let mut found = Vec::new();
    loop {
        let size = usize::try_from(bits.bs()?).ok()?;
        if size == 0 {
            return Some(found);
        }
        let app = bits.handle()?.value;
        let bytes = (0..size).map(|_| bits.rc()).collect::<Option<Vec<_>>>()?;
        found.push((app, bytes));
    }
}

/// 拡張データのバイト列の値（ODS 5.4.1 の 28）。DXF と同じく、1 バイトの種類に 1000 を足したグループ・コードで
/// 置く。文字列は、R2007 から（読めるバージョンでは R2010 から）は RS の長さと UTF-16LE、それより前は RC の長さと RS のコード・ページと
/// バイト列。画層と図形のハンドルは 8 バイト。知らない種類か、バイト列が足りなければ `None`
fn xdata(bytes: &[u8], release: Release) -> Option<Vec<Xdata>> {
    let mut rest = bytes;
    let mut take = |n: usize| -> Option<&[u8]> {
        let (head, tail) = rest.split_at_checked(n)?;
        rest = tail;
        Some(head)
    };
    let mut items = Vec::new();
    loop {
        let Some(&[kind]) = take(1) else {
            return Some(items);
        };
        let code = 1000 + i16::from(kind);
        let value = |data| Xdata::Value(Value { code, data });
        // ハンドルは上位のバイトから並ぶ（サンプルの DWG と DXF で確かめた）
        let handle = |bytes: &[u8]| {
            bytes
                .iter()
                .fold(0u64, |sum, byte| sum << 8 | u64::from(*byte))
        };
        match kind {
            0 if release >= Release::R2007 => {
                let len = usize::from(u16::from_le_bytes(take(2)?.try_into().ok()?));
                let units: Vec<u16> = take(len * 2)?
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|unit| u16::from_le_bytes(*unit))
                    .collect();
                items.push(value(Data::Text(
                    String::from_utf16_lossy(&units).into_bytes(),
                )));
            }
            0 => {
                let len = usize::from(*take(1)?.first()?);
                take(2)?;
                items.push(value(Data::Text(take(len)?.to_vec())));
            }
            2 => items.push(value(Data::Text(match take(1)? {
                [0] => b"{".to_vec(),
                [1] => b"}".to_vec(),
                _ => return None,
            }))),
            3 => items.push(Xdata::Layer(handle(take(8)?))),
            4 => {
                let len = usize::from(*take(1)?.first()?);
                items.push(value(Data::Binary(take(len)?.to_vec())));
            }
            5 => items.push(value(Data::Handle(handle(take(8)?)))),
            10..=13 => {
                for axis in 0..3 {
                    let real = f64::from_le_bytes(take(8)?.try_into().ok()?);
                    items.push(Xdata::Value(Value {
                        code: code + axis * 10,
                        data: Data::Real(real),
                    }));
                }
            }
            40..=42 => items.push(value(Data::Real(f64::from_le_bytes(
                take(8)?.try_into().ok()?,
            )))),
            70 => items.push(value(Data::Int(i64::from(i16::from_le_bytes(
                take(2)?.try_into().ok()?,
            ))))),
            71 => items.push(value(Data::Int(i64::from(i32::from_le_bytes(
                take(4)?.try_into().ok()?,
            ))))),
            _ => return None,
        }
    }
}

/// 図形の共通のフィールド（ODS 5.4.1 の 20.4.1）。既定の値でない色（62）・線種の尺度（48）・
/// 非表示（60）・透過性（440）を値にする。R2004 からは、リアクターの数の後に拡張辞書がないことを表す B があり、
/// 前後の図形のハンドルはない
fn entity_data(bits: &mut Bits) -> Option<Common> {
    // 図形の絵の大きさは、R2010 からは BLL
    let picture = if bits.bit()? {
        let size = if bits.release() >= Release::R2010 {
            bits.bll()?
        } else {
            u64::try_from(bits.rl()?).ok()?
        };
        let size = usize::try_from(size).ok()?;
        let start = bits.pos();
        bits.skip_bytes(size)?;
        Some((start, size))
    } else {
        None
    };
    let entmode = bits.bb()?;
    let reactors = u32::try_from(bits.bl()?).ok()?;
    let release = bits.release();
    let r2004 = release >= Release::R2004;
    let xdic = !r2004 || !bits.bit()?;
    if release >= Release::R2013 {
        bits.bit()?;
    }
    let no_links = r2004 || bits.bit()?;
    let color_at = bits.pos();
    let color = bits.enc()?;
    let ltype_scale = bits.bd()?;
    let ltype_flags = bits.bb()?;
    let plotstyle_flags = bits.bb()?;
    // R2007 からは、材質のフラグ（BB）と影のフラグ（RC）、R2010 からは、表示スタイルを持つかの 3 つの B が続く（図面で確かめた）
    let mut styles = 0;
    if release >= Release::R2007 {
        if bits.bb()? == 3 {
            styles += 1;
        }
        bits.rc()?;
    }
    if release >= Release::R2010 {
        for _ in 0..3 {
            if bits.bit()? {
                styles += 1;
            }
        }
    }
    let invisible = bits.bs()?;
    let weight = bits.rc()?;
    let mut values = Vec::new();
    // 真の色は、DXF が近い番号を 62 に置くが、近さの決まりが分からないので 420 だけを置く
    match color.rgb {
        Some(rgb) => values.push(int(420, i64::from(rgb))),
        None if color.index != 256 => values.push(int(62, i64::from(color.index))),
        None => {}
    }
    if ltype_scale != 1.0 {
        values.push(real(48, ltype_scale));
    }
    if invisible != 0 {
        values.push(int(60, i64::from(invisible)));
    }
    // 透過性（440）。DXF と同じく、レイヤから継承する値（上位のバイトが 0）なら置かない
    if let Some(transparency) = color.transparency.filter(|value| value >> 24 != 0) {
        values.push(int(440, i64::from(transparency)));
    }
    // 線の太さ（370）。DXF と同じく、ByLayer なら置かない
    if let Some(weight) = lineweight(u16::from(weight)).filter(|weight| *weight != -1) {
        values.push(int(370, weight));
    }
    Some(Common {
        picture,
        entmode,
        reactors,
        xdic,
        no_links,
        color_handle: color.book,
        color_at,
        styles,
        ltype_flags,
        plotstyle_handle: plotstyle_flags == 3,
        values,
    })
}

/// 押し出しの向きが既定の (0, 0, 1) か
fn is_up(extrusion: &[Value]) -> bool {
    reals(extrusion) == [Some(0.0), Some(0.0), Some(1.0)]
}

/// 値の実数の並び
fn reals(values: &[Value]) -> Vec<Option<f64>> {
    values
        .iter()
        .map(|v| match v.data {
            Data::Real(r) => Some(r),
            _ => None,
        })
        .collect()
}

/// 2RD の点を、`code`・`code + 10` に置く
fn point2(bits: &mut Bits, code: i16) -> Option<Vec<Value>> {
    Some(vec![real(code, bits.rd()?), real(code + 10, bits.rd()?)])
}

/// 3BD の点を、`code`・`code + 10`・`code + 20` に置く
fn point3(bits: &mut Bits, code: i16) -> Option<Vec<Value>> {
    Some(vec![
        real(code, bits.bd()?),
        real(code + 10, bits.bd()?),
        real(code + 20, bits.bd()?),
    ])
}

/// BT の厚さ（39）と BE の押し出しの向き（210・220・230）を、既定の値でなければ置く
fn thickness_and_extrusion(bits: &mut Bits, values: &mut Vec<Value>) -> Option<()> {
    // BT: 1 なら厚さは 0
    if !bits.bit()? {
        values.push(real(39, bits.bd()?));
    }
    // BE: 1 なら押し出しの向きは (0, 0, 1)
    if !bits.bit()? {
        values.extend(point3(bits, 210)?);
    }
    Some(())
}

fn int(code: i16, value: i64) -> Value {
    Value {
        code,
        data: Data::Int(value),
    }
}

/// 図形の文字列の値。DXF と同じく、制御文字を `^` と文字で、`^` を `^ ` で書く
fn text_value(code: i16, text: &[u8]) -> Value {
    Value {
        code,
        data: Data::Text(caret(text)),
    }
}

/// ヘッダーのコード・ページの番号（ODS 5.4.1 の 3）を、DXF の `$DWGCODEPAGE` の名前にする。番号の順は ODA の
/// コード・ページの列挙による（推定。30 が ANSI_1252 であることをサンプルの DWG と DXF で確かめた。docs/formats/dwg.md）。
/// 0（未定義）と表にない番号は `None`
fn code_page_name(number: u16) -> Option<&'static [u8]> {
    const NAMES: [&[u8]; 44] = [
        b"ASCII",
        b"ISO8859-1",
        b"ISO8859-2",
        b"ISO8859-3",
        b"ISO8859-4",
        b"ISO8859-5",
        b"ISO8859-6",
        b"ISO8859-7",
        b"ISO8859-8",
        b"ISO8859-9",
        b"DOS437",
        b"DOS850",
        b"DOS852",
        b"DOS855",
        b"DOS857",
        b"DOS860",
        b"DOS861",
        b"DOS863",
        b"DOS864",
        b"DOS865",
        b"DOS869",
        b"DOS932",
        b"MAC-ROMAN",
        b"BIG5",
        b"KSC5601",
        b"JOHAB",
        b"DOS866",
        b"ANSI_1250",
        b"ANSI_1251",
        b"ANSI_1252",
        b"GB2312",
        b"ANSI_1253",
        b"ANSI_1254",
        b"ANSI_1255",
        b"ANSI_1256",
        b"ANSI_1257",
        b"ANSI_874",
        b"ANSI_932",
        b"ANSI_936",
        b"ANSI_949",
        b"ANSI_950",
        b"ANSI_1361",
        b"ANSI_1200",
        b"ANSI_1258",
    ];
    NAMES.get(usize::from(number).checked_sub(1)?).copied()
}

fn real(code: i16, value: f64) -> Value {
    Value {
        code,
        data: Data::Real(value),
    }
}
