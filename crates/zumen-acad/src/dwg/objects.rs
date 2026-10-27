//! DWG のオブジェクトから、系統で共有するオブジェクトの表を作る（決定 0023）。
//!
//! どのオブジェクトにも、地図のハンドル（5。DIMSTYLE は 105）を置く。所有者（330）は、ハンドルの並びの
//! 先頭から読む（空間の図形は、見出しの変数が指す空間のブロックのレコード）。図形は、画層の名前（8）と、共通の欄のうち既定の値でないものを置く。図形ごとの欄は、
//! 今は TEXT・ATTRIB・ATTDEF・TOLERANCE・LEADER・MLINE・VIEWPORT・INSERT・LINE・ARC・CIRCLE・POINT・MTEXT・SOLID・TRACE・3DFACE・
//! SHAPE・ELLIPSE・RAY・XLINE・LWPOLYLINE・SPLINE・HATCH・POLYLINE・VERTEX・DIMENSION を読む。表のレコードは、名前（2）と LAYER・STYLE・LTYPE・APPID・VIEW・UCS・VPORT・DIMSTYLE の欄を読む。
//! オブジェクトは、今は MLINESTYLE・DICTIONARY・ACDBDICTIONARYWDFLT・XRECORD・GROUP・DICTIONARYVAR・SCALE・LAYOUT の欄を読む。
//! 図形が表のレコードをハンドルで指す欄（画層・線種・文字の体裁・ブロック・寸法の体裁）は、レコードの名前に直して置く。
//! 型が 500 以上のオブジェクトは、番号が同じクラスで種類を決める。
//! 読めないオブジェクトは、ハンドルだけを持たせて置き、診断を返す。

use std::collections::BTreeMap;
use std::ops::Range;

use super::bits::Bits;
use super::header::{self, Header};
use super::{COMMON_OBJECT, DwgFile, Entry, Release, stopped_at};
use crate::db::{AcadDb, Data, DataType, Object, Origin, Section, Value};
use zumen_base::{Cx, Diagnostic, Kind, Ref, SpecRef, codes};
use zumen_sdk::Error;

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

const TEXT: u16 = 0x01;
const ATTRIB: u16 = 0x02;
const ATTDEF: u16 = 0x03;
const TOLERANCE: u16 = 0x2E;
const LEADER: u16 = 0x2D;
const APPID: u16 = 0x43;
const MLINE: u16 = 0x2F;
const VIEWPORT: u16 = 0x22;
const ARC: u16 = 0x11;
const CIRCLE: u16 = 0x12;
const LINE: u16 = 0x13;
const INSERT: u16 = 0x07;
const MINSERT: u16 = 0x08;
const POINT: u16 = 0x1B;
const FACE3D: u16 = 0x1C;
const SOLID: u16 = 0x1F;
const TRACE: u16 = 0x20;
const SHAPE: u16 = 0x21;
const ELLIPSE: u16 = 0x23;
const SPLINE: u16 = 0x24;
const RAY: u16 = 0x28;
const XLINE: u16 = 0x29;
const MTEXT: u16 = 0x2C;
const LWPOLYLINE: u16 = 0x4D;
const VERTEX_2D: u16 = 0x0A;
const VERTEX_3D: u16 = 0x0B;
const VERTEX_MESH: u16 = 0x0C;
const VERTEX_PFACE: u16 = 0x0D;
const VERTEX_PFACE_FACE: u16 = 0x0E;
const POLYLINE_2D: u16 = 0x0F;
const POLYLINE_3D: u16 = 0x10;
const POLYLINE_PFACE: u16 = 0x1D;
const POLYLINE_MESH: u16 = 0x1E;
const DIMENSION_ORDINATE: u16 = 0x14;
const DIMENSION_LINEAR: u16 = 0x15;
const DIMENSION_ALIGNED: u16 = 0x16;
const DIMENSION_ANGULAR_3PT: u16 = 0x17;
const DIMENSION_ANGULAR_2LINE: u16 = 0x18;
const DIMENSION_RADIUS: u16 = 0x19;
const DIMENSION_DIAMETER: u16 = 0x1A;
const BLOCK_RECORD: u16 = 0x31;
const LAYER: u16 = 0x33;
const STYLE: u16 = 0x35;
const LTYPE: u16 = 0x39;

/// DIMSTYLE の表のレコードの型。ハンドルのグループ・コードが 105
const VIEW: u16 = 0x3D;
const UCS: u16 = 0x3F;
const VPORT: u16 = 0x41;
const DIMSTYLE: u16 = 0x45;
const MLINESTYLE: u16 = 0x49;
const DICTIONARY: u16 = 0x2A;
const GROUP: u16 = 0x48;

/// 型から、DXF の名前と属する所。クラスで決まる型（500 以上）と、知らない型は `None`
fn kind(type_code: u16) -> Option<(&'static [u8], Section)> {
    let entity = |name: &'static [u8]| Some((name, Section::Entities));
    let table = |name: &'static [u8]| Some((name, Section::Tables));
    let object = |name: &'static [u8]| Some((name, Section::Objects));
    match type_code {
        TEXT => entity(b"TEXT"),
        ATTRIB => entity(b"ATTRIB"),
        ATTDEF => entity(b"ATTDEF"),
        0x04 => Some((b"BLOCK", Section::Blocks)),
        0x05 => Some((b"ENDBLK", Section::Blocks)),
        0x06 => entity(b"SEQEND"),
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
        0x30 | 0x32 | 0x34 | 0x38 | 0x3C | 0x3E | 0x40 | 0x42 | 0x44 => table(b"TABLE"),
        BLOCK_RECORD => table(b"BLOCK_RECORD"),
        LAYER => table(b"LAYER"),
        STYLE => table(b"STYLE"),
        LTYPE => table(b"LTYPE"),
        0x3D => table(b"VIEW"),
        0x3F => table(b"UCS"),
        0x41 => table(b"VPORT"),
        APPID => table(b"APPID"),
        DIMSTYLE => table(b"DIMSTYLE"),
        // DXF にない表。名前は DWG のもの
        0x46 => table(b"VP_ENT_HDR_CONTROL"),
        0x47 => table(b"VP_ENT_HDR"),
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

impl DwgFile {
    /// 型から、DXF の名前と属する所。500 以上の型は、番号が同じクラスで決める。知らない型は `None`
    pub(super) fn kind(&self, type_code: u16) -> Option<(&[u8], Section)> {
        if let Some(found) = kind(type_code) {
            return Some(found);
        }
        let class = self.classes.iter().find(|c| c.number == type_code)?;
        let section = if class.entity {
            Section::Entities
        } else {
            Section::Objects
        };
        Some((class.name.as_slice(), section))
    }

    /// オブジェクトの表を作る。予算の上限か中断の合図で止めたときだけ失敗する
    pub fn objects(&self, cx: &mut Cx) -> Result<AcadDb, Error> {
        let header = self
            .header_section()
            .and_then(|(src, span)| header::header(src, span, self.release, cx));
        let mut found = Vec::with_capacity(self.entries.len());
        for entry in &self.entries {
            let place = Ref::at_bytes(entry.span.start as u64, entry.span.end as u64);
            cx.check(Kind::Work, 1)
                .map_err(|stop| stopped_at(cx, stop, place))?;
            let kind = self.kind(entry.type_code);
            let fields = kind.as_ref().and_then(|(name, section)| {
                // BLOCK と ENDBLK は、DXF では BLOCKS の節に属するが、DWG では図形として書かれる
                let entity = matches!(section, Section::Entities | Section::Blocks);
                read(self.data(entry), self.release, entry, name, entity, header)
            });
            found.push((kind, fields));
        }
        // 図形は表のレコードと多重線の体裁をハンドルで指すので、先にその型と名前を集める
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
                            let (own, xdata) = fields.place(&names, entry, cx, &at);
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
            // ビューポートの番号（69）は DWG に書かれない。所有者ごとにオブジェクトの地図の順（ハンドルの順）で
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
        if let Some(header) = header {
            let [x0, y0, x1, y1] = header.limits;
            db.set_variable(b"$INSUNITS", vec![int(70, i64::from(header.units))]);
            db.set_variable(b"$PLIMMIN", vec![real(10, x0), real(20, y0)]);
            db.set_variable(b"$PLIMMAX", vec![real(10, x1), real(20, y1)]);
        }
        Ok(db)
    }
}

/// BLOCK_RECORD から読んだ印・基点などを、そのレコードを所有者に持つ BLOCK の名前の後ろへ移す
fn to_blocks(objects: &mut [Object]) {
    let mut moved: BTreeMap<u64, Vec<Value>> = BTreeMap::new();
    for object in objects.iter_mut() {
        if object.kind != b"BLOCK_RECORD" {
            continue;
        }
        let Some(handle) = object.handle() else {
            continue;
        };
        let (block, own) = object
            .values
            .drain(..)
            .partition(|value| BLOCK_FIELDS.contains(&value.code));
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

/// オブジェクトから読んだ欄
struct Fields {
    /// 所有者（330）
    owner: Option<u64>,
    /// 図形の画層のハンドル
    layer: Option<u64>,
    /// 所有者と画層より後の値
    values: Vec<Value>,
    /// 値の後に、名前に直して置く表のレコード
    records: Vec<RecordRef>,
    /// 登録アプリケーションのハンドルごとの拡張データ。読めなければ `None`
    xdata: Vec<(u64, Option<Vec<Xdata>>)>,
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

/// BLOCK_RECORD から読み、DXF と同じく BLOCK に置く欄
const BLOCK_FIELDS: &[i16] = &[70, 10, 20, 30, 1, 4];

/// 名前（2）を持つ表のレコードの型か
fn is_table_record(type_code: u16) -> bool {
    matches!(
        type_code,
        BLOCK_RECORD | LAYER | STYLE | LTYPE | VIEW | UCS | VPORT | APPID | DIMSTYLE | 0x47
    )
}

impl Fields {
    /// ハンドル（5）より後の値と、拡張データ（1001 から）を並べる。画層などは名前に直し、見つからなければ
    /// 診断を返す
    fn place(
        self,
        names: &BTreeMap<u64, (u16, Vec<u8>)>,
        entry: &Entry,
        cx: &mut Cx,
        at: &Ref,
    ) -> (Vec<Value>, Vec<Value>) {
        let mut values = Vec::new();
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
        values.extend(self.values);
        for r in self.records {
            match names
                .get(&r.handle)
                .filter(|(record, _)| *record == r.record)
            {
                // 線種が ByLayer、文字の体裁が Standard の既定の値なら、DXF と同じく置かない
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

/// 図形の共通の欄のうち、型ごとの欄とハンドルの並びを読むのに要るもの
struct Common {
    /// 所有者の決まり方。0 は所有者のハンドルを持ち、1 は用紙空間、2 はモデル空間の図形
    entmode: u8,
    /// 反応子の数
    reactors: u32,
    /// 拡張辞書のハンドルを持つか
    xdic: bool,
    /// 前後の図形のハンドルを持たないか
    no_links: bool,
    /// 色を DBCOLOR のハンドルで指すか（R2004 から）
    color_handle: bool,
    /// 色（ENC）の始まりのビットの位置
    color_at: usize,
    /// 印刷スタイルの後のハンドルの数。材質（R2007 から、印が 3 のとき）と、表示スタイル
    /// （R2010 から、3 つの B のそれぞれが立つとき）
    styles: usize,
    /// 線種の印。0 は ByLayer、1 は ByBlock、2 は Continuous、3 はハンドルで指す
    ltype_flags: u8,
    /// 印刷スタイルのハンドルを持つか（印刷スタイルの印が 3）
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
    // R2010 からは、型は BOT で、ハンドルの流れの位置は区切りの大きさで決まる。文字列の流れは
    // ハンドルの流れの前にある
    let handles_at = match entry.handles {
        Some(at) => {
            bits.bot()?;
            bits.split_strings(at)?;
            at
        }
        None => {
            bits.bs()?;
            usize::try_from(bits.rl()?).ok()?
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
                MTEXT => mtext(&mut bits)?,
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
                // HATCH は型が固定でなく、クラスで決まる
                _ if kind == b"HATCH" => hatch(&mut bits)?,
                _ if kind == b"IMAGE" => image(&mut bits, release)?,
                _ => Vec::new(),
            }
        } else {
            Vec::new()
        };
        // 図形のハンドルの並び: 所有者（entmode が 0 のとき。1・2 のときは見出しの変数が指す空間）・反応子・拡張辞書・前後の図形
        // （nolinks が 0 のとき）・色（R2004 で、ENC の 0x4000 が立つとき）・画層・線種と印刷スタイル（印が 3 のとき）・型ごとのハンドル
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
        if common.xdic {
            bits.handle()?;
        }
        if !common.no_links {
            bits.handle()?;
            bits.handle()?;
        }
        if common.color_handle {
            bits.handle()?;
        }
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
        // IMAGE は、画像の定義（340）と、その反応子（360）のハンドルを持つ。DXF と同じく、表示の印（70）と
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
        // 寸法は、寸法の体裁（3）とブロック（2）の順に指す
        let pointed: &[(i16, u16)] = match entry.type_code {
            TEXT | MTEXT | ATTRIB | ATTDEF => &[(7, STYLE)],
            INSERT | MINSERT => &[(2, BLOCK_RECORD)],
            DIMENSION_ORDINATE..=DIMENSION_DIAMETER => &[(3, DIMSTYLE), (2, BLOCK_RECORD)],
            TOLERANCE => &[(3, DIMSTYLE)],
            LEADER => &[(3, DIMSTYLE)],
            _ => &[],
        };
        // LEADER は寸法の体裁の前に注釈のハンドルを、MLINE は多重線の体裁のハンドルを持つ（340）
        if matches!(entry.type_code, LEADER | MLINE) {
            let annotation = bits.handle()?.resolve(entry.handle)?;
            if annotation != 0 {
                values.push(Value {
                    code: 340,
                    data: Data::Handle(annotation),
                });
            }
            // MLINE は、体裁の名前も 2 に置く
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
            layer: Some(layer),
            values,
            records,
            xdata,
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
        // SORTENTSTABLE の並べ替えのハンドル。データの流れにあり、図形のハンドルはハンドルの流れにある
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
        } else if entry.type_code == GROUP {
            group(&mut bits, &mut handles)?
        } else if kind == b"DICTIONARYVAR" {
            vec![
                int(280, i64::from(bits.rc()?)),
                text_value(1, &bits.text()?),
            ]
        } else if kind == b"SCALE" {
            scale(&mut bits)?
        } else if kind == b"IMAGEDEF" {
            imagedef(&mut bits)?
        } else if kind == b"IMAGEDEF_REACTOR" {
            vec![int(90, i64::from(bits.bl()?))]
        } else if kind == b"LAYOUT" {
            layout(&mut bits, release, &mut handles)?
        } else if sortents {
            for _ in 0..bits.bl()? {
                sorts.push(bits.handle()?.resolve(entry.handle)?);
            }
            Vec::new()
        } else {
            Vec::new()
        };
        // オブジェクトのハンドルの並び: 所有者・反応子・拡張辞書・型ごとのハンドル
        bits.seek(handles_at);
        let owner = bits.handle()?.resolve(entry.handle)?;
        if !entries.is_empty() || !handles.is_empty() || sortents {
            for _ in 0..reactors {
                bits.handle()?;
            }
            if xdic {
                bits.handle()?;
            }
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
                match code {
                    0 => {}
                    6 if handle != 0 => records.push(RecordRef {
                        code,
                        record: LTYPE,
                        handle,
                    }),
                    _ if handle != 0 => values.push(Value {
                        code,
                        data: Data::Handle(handle),
                    }),
                    _ => {}
                }
            }
        }
        Some(Fields {
            owner: Some(owner),
            layer: None,
            values,
            records,
            xdata,
        })
    }
}

/// 版 `release`（R2004 まで）の図形でないオブジェクトのデータの、ハンドルの流れの位置（RL）を書く所と、
/// 型ごとの欄の始まりと、ハンドルの流れの始まりの、ビットの位置
pub(super) fn object_fields(data: &[u8], release: Release) -> Option<[usize; 3]> {
    let mut bits = Bits::with_release(data, release);
    bits.bs()?;
    let size_at = bits.pos();
    let handles_at = usize::try_from(bits.rl()?).ok()?;
    bits.handle()?;
    skip_extended_data(&mut bits)?;
    bits.bl()?;
    // R2004 からは、反応子の数の後に、拡張辞書を持たない印（B）が続く
    if release >= Release::R2004 {
        bits.bit()?;
    }
    let start = bits.pos();
    (start <= handles_at).then_some([size_at, start, handles_at])
}

/// 版 `release`（R2010 から）の図形でないオブジェクトのデータ（ハンドルの流れの大きさ（MC）の後）の、型ごとの
/// 欄の始まりと、主のデータの終わりの、ビットの位置。`handles_at` は、ハンドルの流れの始まり（決定 0089）
pub(super) fn r2010_object_fields(
    data: &[u8],
    handles_at: usize,
    release: Release,
) -> Option<[usize; 2]> {
    let mut bits = Bits::with_release(data, release);
    bits.bot()?;
    let end = bits.split_strings(handles_at)?;
    bits.handle()?;
    skip_extended_data(&mut bits)?;
    bits.bl()?;
    bits.bit()?;
    // R2013 からは、DS のデータを持つかの B が続く
    if release >= Release::R2013 {
        bits.bit()?;
    }
    let start = bits.pos();
    (start <= end).then_some([start, end])
}

/// 版 `release`（R2010 から）の図形のデータ（ハンドルの流れの大きさ（MC）の後）の、共通の欄の色の始まりと、
/// 型ごとの欄の始まりと、主のデータの終わり（文字列の流れの始まりか、その有無の印）の、ビットの位置。
/// `handles_at` は、ハンドルの流れの始まり（決定 0085）
pub(super) fn r2010_entity_fields(
    data: &[u8],
    handles_at: usize,
    release: Release,
) -> Option<[usize; 3]> {
    let mut bits = Bits::with_release(data, release);
    bits.bot()?;
    let end = bits.split_strings(handles_at)?;
    bits.handle()?;
    skip_extended_data(&mut bits)?;
    let common = entity_data(&mut bits)?;
    let start = bits.pos();
    (start <= end).then_some([common.color_at, start, end])
}

/// 版 `release`（R2004 まで）の図形のデータの、ハンドルの流れの位置（RL）を書く所と、共通の欄の色の
/// 始まりと、型ごとの欄の始まりと、ハンドルの流れの始まりの、ビットの位置
pub(super) fn entity_fields(data: &[u8], release: Release) -> Option<[usize; 4]> {
    let mut bits = Bits::with_release(data, release);
    bits.bs()?;
    let size_at = bits.pos();
    let handles_at = usize::try_from(bits.rl()?).ok()?;
    bits.handle()?;
    skip_extended_data(&mut bits)?;
    let common = entity_data(&mut bits)?;
    let start = bits.pos();
    (start <= handles_at).then_some([size_at, common.color_at, start, handles_at])
}

/// R2000 の図形の、図形の並びをたどるのに要るもの（決定 0051）
pub(super) struct Links {
    /// 前後の図形のハンドルを持たないことを表す B のビットの位置
    pub(super) no_links_at: usize,
    /// 反応子の数
    pub(super) reactors: u32,
    /// 拡張辞書のハンドル。なければ 0
    pub(super) xdic: u64,
    /// 前後の図形のハンドルのビットの範囲。持たなければ、拡張辞書の後ろの空の範囲
    pub(super) at: Range<usize>,
    /// 後ろの図形のハンドル。持たなければ、ハンドルの 1 つ後ろ（図面で確かめた）。最後の図形は 0
    pub(super) next: u64,
}

/// ハンドル `handle` の R2000 の図形のデータの、図形の並びをたどるのに要るもの
pub(super) fn entity_links(data: &[u8], handle: u64) -> Option<Links> {
    let mut bits = Bits::with_release(data, Release::R2000);
    bits.bs()?;
    let handles_at = usize::try_from(bits.rl()?).ok()?;
    bits.handle()?;
    skip_extended_data(&mut bits)?;
    if bits.bit()? {
        let size = usize::try_from(bits.rl()?).ok()?;
        bits.skip_bytes(size)?;
    }
    let entmode = bits.bb()?;
    let reactors = u32::try_from(bits.bl()?).ok()?;
    let no_links_at = bits.pos();
    let no_links = bits.bit()?;
    bits.seek(handles_at);
    if entmode == 0 {
        bits.handle()?;
    }
    for _ in 0..reactors {
        bits.handle()?;
    }
    let xdic = bits.handle()?.resolve(handle)?;
    let start = bits.pos();
    let next = if no_links {
        handle.checked_add(1)?
    } else {
        bits.handle()?;
        bits.handle()?.resolve(handle)?
    };
    Some(Links {
        no_links_at,
        reactors,
        xdic,
        at: start..bits.pos(),
        next,
    })
}

/// ハンドル `handle` の R2000 のブロックのレコードのデータの、最初と最後の図形のハンドルのビットの範囲と、
/// その 2 つのハンドル（決定 0051）。ハンドルの並びは、所有者・反応子・拡張辞書・外部参照のブロック・
/// BLOCK・最初の図形・最後の図形・ENDBLK・配置（図面で確かめた）。図形がなければ、どちらも 0
pub(super) fn block_links(data: &[u8], handle: u64) -> Option<(Range<usize>, u64, u64)> {
    let mut bits = Bits::with_release(data, Release::R2000);
    bits.bs()?;
    let handles_at = usize::try_from(bits.rl()?).ok()?;
    bits.handle()?;
    skip_extended_data(&mut bits)?;
    let reactors = u32::try_from(bits.bl()?).ok()?;
    bits.seek(handles_at);
    for _ in 0..reactors + 4 {
        bits.handle()?;
    }
    let start = bits.pos();
    let first = bits.handle()?.resolve(handle)?;
    let last = bits.handle()?.resolve(handle)?;
    Some((start..bits.pos(), first, last))
}

/// 版 `release`（R2004 から）の図形のデータの、反応子の数と、拡張辞書のハンドルを持つか。R2010 からは、
/// データはハンドルの流れの大きさ（MC）の後（決定 0091）
pub(super) fn entity_refs(data: &[u8], release: Release) -> Option<(u32, bool)> {
    let mut bits = Bits::with_release(data, release);
    if release >= Release::R2010 {
        bits.bot()?;
    } else {
        bits.bs()?;
        bits.rl()?;
    }
    bits.handle()?;
    skip_extended_data(&mut bits)?;
    let common = entity_data(&mut bits)?;
    Some((common.reactors, common.xdic))
}

/// R2004 から後のブロックのレコードが持つ図形（決定 0077・0091）
pub(super) struct Owned {
    /// ハンドルの流れの位置（RL）を書く所のビットの位置。R2010 からは使わない
    pub(super) size_at: usize,
    /// 持つ図形の数（BL）のビットの範囲
    pub(super) count: Range<usize>,
    /// ハンドルの流れの始まりのビットの位置
    pub(super) handles_at: usize,
    /// 持つ図形のハンドルの、ビットの範囲とハンドルの並び
    pub(super) handles: Vec<(Range<usize>, u64)>,
    /// 持つ図形のハンドルの終わり（ENDBLK のハンドルの始まり）のビットの位置
    pub(super) end: usize,
}

/// ハンドル `handle` の版 `release`（R2004 から）のブロックのレコードのデータの、持つ図形（決定 0077）。外部参照の
/// ブロックは `None`。持つ図形の数は、読み込まれた印（B）の後に BL で置かれ、ハンドルは、所有者・反応子・拡張辞書・
/// 外部参照のブロック・BLOCK の後に、その数だけ並ぶ（図面で確かめた）。R2010 からは、データはハンドルの流れの
/// 大きさ（MC）の後で、ハンドルの流れの始まり `handles` を受け取る。外部参照の番号はない（決定 0091）
pub(super) fn owned(
    data: &[u8],
    handle: u64,
    release: Release,
    handles: Option<usize>,
) -> Option<Owned> {
    let mut bits = Bits::with_release(data, release);
    let r2010 = release >= Release::R2010;
    let (size_at, handles_at) = if r2010 {
        bits.bot()?;
        let at = handles?;
        bits.split_strings(at)?;
        (0, at)
    } else {
        bits.bs()?;
        let size_at = bits.pos();
        (size_at, usize::try_from(bits.rl()?).ok()?)
    };
    bits.handle()?;
    skip_extended_data(&mut bits)?;
    let reactors = u32::try_from(bits.bl()?).ok()?;
    let xdic = !bits.bit()?;
    // R2013 からは、DS のデータを持つかの B が続く
    if release >= Release::R2013 {
        bits.bit()?;
    }
    // 名前・参照される印・外部参照の番号・外部参照に依る印
    bits.text()?;
    bits.bit()?;
    if !r2010 {
        bits.bs()?;
    }
    bits.bit()?;
    // 名前のない印・属性を持つ印・外部参照の印・重ねる外部参照の印・読み込まれた印
    let flags: Vec<bool> = (0..5).map(|_| bits.bit()).collect::<Option<_>>()?;
    if flags.get(2..4)?.contains(&true) {
        return None;
    }
    let start = bits.pos();
    let count = usize::try_from(bits.bl()?).ok()?;
    let count_range = start..bits.pos();
    bits.seek(handles_at);
    for _ in 0..reactors + u32::from(xdic) + 3 {
        bits.handle()?;
    }
    let mut handles = Vec::with_capacity(count.min(data.len()));
    for _ in 0..count {
        let at = bits.pos();
        let owned = bits.handle()?.resolve(handle)?;
        handles.push((at..bits.pos(), owned));
    }
    Some(Owned {
        size_at,
        count: count_range,
        handles_at,
        handles,
        end: bits.pos(),
    })
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
/// 置く。文字列は、R2007 から（読める版では R2010 から）は RS の長さと UTF-16LE、それより前は RC の長さと RS のコード・ページと
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
        // ハンドルは上位のバイトから並ぶ（試料の DWG と DXF で確かめた）
        let handle = |bytes: &[u8]| {
            bytes
                .iter()
                .fold(0u64, |sum, byte| sum << 8 | u64::from(*byte))
        };
        match kind {
            0 if release >= Release::R2010 => {
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

/// 拡張データ（EED）を読み飛ばす（ODS 5.4.1 の 28）
pub(super) fn skip_extended_data(bits: &mut Bits) -> Option<()> {
    loop {
        let size = bits.bs()?;
        if size == 0 {
            return Some(());
        }
        bits.handle()?;
        bits.skip_bytes(usize::try_from(size).ok()?)?;
    }
}

/// 図形の共通の欄（ODS 5.4.1 の 20.4.1）。既定の値でない色（62）・線種の尺度（48）・
/// 非表示（60）を値にする。R2004 からは、反応子の数の後に拡張辞書がないことを表す B があり、
/// 前後の図形のハンドルはない
fn entity_data(bits: &mut Bits) -> Option<Common> {
    // 図形の絵の大きさは、R2010 からは BLL
    if bits.bit()? {
        let size = if bits.release() >= Release::R2010 {
            bits.bll()?
        } else {
            u64::try_from(bits.rl()?).ok()?
        };
        bits.skip_bytes(usize::try_from(size).ok()?)?;
    }
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
    // R2007 からは、材質の印（BB）と影の印（RC）、R2010 からは、表示スタイルを持つかの 3 つの B が続く（図面で確かめた）
    let mut styles = 0;
    if release >= Release::R2010 {
        if bits.bb()? == 3 {
            styles += 1;
        }
        bits.rc()?;
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
    // 線の太さ（370）。DXF と同じく、ByLayer なら置かない
    if let Some(weight) = lineweight(u16::from(weight)).filter(|weight| *weight != -1) {
        values.push(int(370, weight));
    }
    Some(Common {
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

/// TEXT の欄（ODS 5.4.1 の 20.4.3、R2000）。DataFlags のビットが立つ欄は記録されず、既定の値なので
/// 置かない。高さ（40）は DD で書かれないのでいつも置く
fn text(bits: &mut Bits) -> Option<Vec<Value>> {
    let flags = bits.rc()?;
    let absent = |bit: u8| flags & bit != 0;
    let elevation = if absent(0x01) { 0.0 } else { bits.rd()? };
    let (x, y) = (bits.rd()?, bits.rd()?);
    let mut values = vec![real(10, x), real(20, y), real(30, elevation)];
    let alignment = if absent(0x02) {
        None
    } else {
        Some((bits.dd(x)?, bits.dd(y)?))
    };
    // BE の押し出しの向きと、BT の厚さ。LINE などとは並びが逆
    let extrusion = if bits.bit()? {
        None
    } else {
        Some(point3(bits, 210)?)
    };
    if !bits.bit()? {
        values.push(real(39, bits.bd()?));
    }
    if !absent(0x04) {
        values.push(real(51, bits.rd()?.to_degrees()));
    }
    if !absent(0x08) {
        values.push(real(50, bits.rd()?.to_degrees()));
    }
    values.push(real(40, bits.rd()?));
    if !absent(0x10) {
        values.push(real(41, bits.rd()?));
    }
    values.push(text_value(1, &bits.text()?));
    for (bit, code) in [(0x20, 71), (0x40, 72), (0x80, 73)] {
        if !absent(bit) {
            values.push(int(code, i64::from(bits.bs()?)));
        }
    }
    if let Some((x, y)) = alignment {
        values.extend([real(11, x), real(21, y), real(31, elevation)]);
    }
    values.extend(extrusion.into_iter().flatten());
    Some(values)
}

/// ATTRIB・ATTDEF の欄（ODS 5.4.1 の 20.4.4・20.4.5、R2000）。TEXT の欄に続けて、タグ（2）・欄の長さ
/// （73）・印（70）を、ATTDEF は問い（3）も置く。DXF は縦の合わせを 74 に書くので、TEXT の 73 を 74 に
/// 移す。欄の長さは 0 でなければ置く
fn attribute(bits: &mut Bits, definition: bool) -> Option<Vec<Value>> {
    let mut values = text(bits)?;
    for value in &mut values {
        if value.code == 73 {
            value.code = 74;
        }
    }
    // R2010 からは、名前の前に属性の版（RC、DXF の 280）が、印の後に位置を固定するか（B、DXF の
    // 2 つ目の 280）が、定義の問いの前にもう 1 つの版（RC）がある
    let r2010 = bits.release() >= Release::R2010;
    if r2010 {
        values.push(int(280, i64::from(bits.rc()?)));
    }
    // R2018 からは、版の後に属性の種類（RC、DXF の 71）が続く。1 は 1 行の属性で、2（ATTRIB）と
    // 4（ATTDEF）は複数行の属性。複数行の属性は、意味の分からない 37 ビット（図面の 2 つの属性で
    // 同じ値）の後に、埋め込みの MTEXT の欄と、DXF の 72 にあたる BS を持つ（図面で確かめた）。
    // DXF と同じく、属性の値の後に、種類（71）と 72 と、埋め込みのオブジェクトの印（101）と MTEXT の値を
    // 置く（決定 0102）
    let mut embedded = Vec::new();
    if bits.release() >= Release::R2018 {
        let kind = bits.rc()?;
        if kind != 1 {
            bits.seek(bits.pos() + 37);
            let mtext = mtext(bits)?;
            mtext_columns(bits)?;
            embedded.push(int(71, i64::from(kind)));
            embedded.push(int(72, i64::from(bits.bs()?)));
            embedded.push(text_value(101, b"Embedded Object"));
            embedded.extend(mtext);
        }
    }
    values.push(text_value(2, &bits.text()?));
    let length = bits.bs()?;
    if length != 0 {
        values.push(int(73, i64::from(length)));
    }
    values.push(int(70, i64::from(bits.rc()?)));
    if r2010 {
        values.push(int(280, i64::from(bits.bit()?)));
    }
    if definition {
        if r2010 {
            bits.rc()?;
        }
        values.push(text_value(3, &bits.text()?));
    }
    values.extend(embedded);
    Some(values)
}

/// TOLERANCE の欄（ODS 5.4.1 の 20.4.49、R2000）。挿入点（10）と文字（1）を置き、X 軸の向き（11）と
/// 押し出しの向きは既定の値でなければ置く
fn tolerance(bits: &mut Bits) -> Option<Vec<Value>> {
    let mut values = point3(bits, 10)?;
    let direction = point3(bits, 11)?;
    let extrusion = point3(bits, 210)?;
    values.push(text_value(1, &bits.text()?));
    if reals(&direction) != [Some(1.0), Some(0.0), Some(0.0)] {
        values.extend(direction);
    }
    if !is_up(&extrusion) {
        values.extend(extrusion);
    }
    Some(values)
}

/// LEADER の欄（ODS 5.4.1 の 20.4.47、R2000）。注釈の種類（73）・注釈の高さと幅（40・41）・鉤の線の
/// 向き（74）・頂点の数（76）と頂点（10）を置き、押し出しの向き（210）と X 軸の向き（211）は既定の値で
/// なければ置く。DXF は寸法の体裁から鉤の線の点を足して書くことがあり、そのときは頂点が 1 つ多い
fn leader(bits: &mut Bits) -> Option<Vec<Value>> {
    bits.bit()?;
    let annotation = bits.bs()?;
    let path = bits.bs()?;
    let count = bits.bl()?;
    let mut vertices = Vec::new();
    for _ in 0..count {
        vertices.extend(point3(bits, 10)?);
    }
    // 終わりの点の射影。DXF には書かれない
    point3(bits, 10)?;
    let extrusion = point3(bits, 210)?;
    let direction = point3(bits, 211)?;
    let mut values = vec![int(73, i64::from(annotation))];
    if path != 0 {
        values.push(int(72, i64::from(path)));
    }
    // 鉤の線の点の差と、注釈への差。どちらも DXF で確かめていない
    point3(bits, 212)?;
    point3(bits, 213)?;
    // 注釈の枠の高さと幅。R2010 からは、DXF は注釈から求めた値を書き、DWG の値（図面では 1 と 0）とは
    // 違うので置かない
    let (height, width) = (bits.bd()?, bits.bd()?);
    if bits.release() < Release::R2010 {
        values.push(real(40, height));
        values.push(real(41, width));
    }
    values.push(int(74, i64::from(bits.bit()?)));
    values.push(int(76, i64::from(count)));
    values.extend(vertices);
    if reals(&direction) != [Some(1.0), Some(0.0), Some(0.0)] {
        values.extend(direction);
    }
    if !is_up(&extrusion) {
        values.extend(extrusion);
    }
    Some(values)
}

/// MLINE の欄（ODS 5.4.1 の 20.4.50、R2000）。尺度（40）・合わせ（70）・開閉の印（71）・頂点の数（72）・
/// 線の数（73）・基点（10）・押し出しの向き（210）を置き、頂点ごとに点（11）・向き（12）・留めの向き
/// （13）と、線ごとの区切り（74・41）と塗りの区切り（75・42）を置く
fn mline(bits: &mut Bits) -> Option<Vec<Value>> {
    let mut values = vec![real(40, bits.bd()?)];
    let justification = bits.rc()?;
    let base = point3(bits, 10)?;
    let extrusion = point3(bits, 210)?;
    let flags = bits.bs()?;
    let lines = bits.rc()?;
    let count = bits.bs()?;
    values.extend([
        int(70, i64::from(justification)),
        int(71, i64::from(flags)),
        int(72, i64::from(count)),
        int(73, i64::from(lines)),
    ]);
    values.extend(base);
    values.extend(extrusion);
    for _ in 0..count {
        values.extend(point3(bits, 11)?);
        values.extend(point3(bits, 12)?);
        values.extend(point3(bits, 13)?);
        for _ in 0..lines {
            for code in [74, 75] {
                let params = bits.bs()?;
                values.push(int(code, i64::from(params)));
                for _ in 0..params {
                    values.push(real(code - 33, bits.bd()?));
                }
            }
        }
    }
    Some(values)
}

/// VIEWPORT の欄（ODS 5.4.1 の 20.4.38、R2000）。仕様の欄を、DXF の並び（中心・大きさ・ビューの中心・
/// スナップ・グリッド・ビューの向きと目標・ビュー・状態・UCS）で置く。角度は度に直す。ビューポートの
/// 番号（69）と状態（68）は DWG に記録されていないので置かない。凍結した画層の数を `frozen` に返す
fn viewport(bits: &mut Bits, frozen: &mut u32) -> Option<Vec<Value>> {
    let mut values = point3(bits, 10)?;
    values.push(real(40, bits.bd()?));
    values.push(real(41, bits.bd()?));
    let target = point3(bits, 17)?;
    let direction = point3(bits, 16)?;
    let twist = bits.bd()?;
    let height = bits.bd()?;
    let lens = bits.bd()?;
    let front = bits.bd()?;
    let back = bits.bd()?;
    let snap = bits.bd()?;
    for code in [12, 13, 14, 15] {
        values.push(real(code, bits.rd()?));
        values.push(real(code + 10, bits.rd()?));
    }
    values.extend(direction);
    values.extend(target);
    values.extend([
        real(42, lens),
        real(43, front),
        real(44, back),
        real(45, height),
        real(50, snap.to_degrees()),
        real(51, twist.to_degrees()),
        int(72, i64::from(bits.bs()?)),
    ]);
    // R2010 からは、主なグリッドの線の間隔（BS、DXF の 61）が続く
    if bits.release() >= Release::R2010 {
        values.push(int(61, i64::from(bits.bs()?)));
    }
    *frozen = u32::try_from(bits.bl()?).ok()?;
    values.push(int(90, i64::from(bits.bl()?)));
    values.push(text_value(1, &bits.text()?));
    values.push(int(281, i64::from(bits.rc()?)));
    let at_origin = bits.bit()?;
    let per_viewport = bits.bit()?;
    values.extend([
        int(71, i64::from(per_viewport)),
        int(74, i64::from(at_origin)),
    ]);
    for code in [110, 111, 112] {
        values.extend(point3(bits, code)?);
    }
    let elevation = bits.bd()?;
    values.push(int(79, i64::from(bits.bs()?)));
    values.push(real(146, elevation));
    Some(values)
}

/// MLINESTYLE の欄（ODS 5.4.1 の 20.4.73、R2000）。名前（2）・印（70）・説明（3）・塗りの色（62）・
/// 始めと終わりの角度（51・52、度に直す）・線の数（71）と、線ごとの位置（49）と色（62）を置く。印は、
/// DWG と DXF でビットの並びが違うので並べ直す。線種は番号で記録されているので、まだ置かない
fn mlinestyle(bits: &mut Bits) -> Option<Vec<Value>> {
    let name = bits.text()?;
    let description = bits.text()?;
    let flags = bits.bs()?;
    // DWG のビットと、DXF のビット
    let moved = [
        (1, 2),
        (2, 1),
        (16, 16),
        (32, 64),
        (64, 32),
        (256, 256),
        (512, 1024),
        (1024, 512),
    ];
    let flags = moved
        .iter()
        .filter(|(dwg, _)| flags & dwg != 0)
        .fold(0, |sum, (_, dxf)| sum | dxf);
    let mut values = vec![
        Value {
            code: 2,
            data: Data::Text(name),
        },
        int(70, i64::from(flags)),
        text_value(3, &description),
        int(62, i64::from(bits.cmc()?.index)),
        real(51, bits.bd()?.to_degrees()),
        real(52, bits.bd()?.to_degrees()),
    ];
    let lines = bits.rc()?;
    values.push(int(71, i64::from(lines)));
    for _ in 0..lines {
        values.push(real(49, bits.bd()?));
        values.push(int(62, i64::from(bits.cmc()?.index)));
        // R2018 からは、線種を番号でなくハンドルで指す
        if bits.release() < Release::R2018 {
            bits.bs()?;
        }
    }
    Some(values)
}

/// DICTIONARY の欄（ODS 5.4.1 の 20.4.44、R2000）。所有する印（280）は 1 なら、複製の扱い（281）は
/// いつも置く。項目の名前は `entries` に集め、ハンドルの並びの項目と組にして後で置く
fn dictionary(bits: &mut Bits, entries: &mut Vec<Vec<u8>>) -> Option<Vec<Value>> {
    let count = bits.bl()?;
    let cloning = bits.bs()?;
    let hard_owner = bits.rc()?;
    let mut values = Vec::new();
    if hard_owner != 0 {
        values.push(int(280, i64::from(hard_owner)));
    }
    values.push(int(281, i64::from(cloning)));
    for _ in 0..count {
        entries.push(bits.text()?);
    }
    Some(values)
}

/// XRECORD の欄（ODS 5.4.1 の 20.4.89、R2000）。複製の扱い（280）を DXF と同じく先に置き、
/// 続けてデータの値を置く。データは、グループ・コード（RS）と、コードの型で決まる値の並びで、
/// 記録されたバイト数をちょうど読み切らなければ読めないとする
fn xrecord(bits: &mut Bits) -> Option<Vec<Value>> {
    let len = usize::try_from(bits.bl()?).ok()?;
    let end = bits.pos().checked_add(len.checked_mul(8)?)?;
    let mut data = Vec::new();
    while bits.pos() < end {
        let code = bits.rs()?;
        match DataType::of(code) {
            // 点は、X のコードに 3RD で記録され、DXF では Y と Z が別のコードになる
            DataType::Real if matches!(code, 10..=19 | 1010..=1019) => {
                data.extend([
                    real(code, bits.rd()?),
                    real(code + 10, bits.rd()?),
                    real(code + 20, bits.rd()?),
                ]);
            }
            DataType::Real => data.push(real(code, bits.rd()?)),
            DataType::Int => {
                let value = match code {
                    280..=289 => i64::from(bits.rc()?),
                    90..=99 | 420..=429 | 440..=459 | 1071 => i64::from(bits.rl()?),
                    160..=169 => i64::from_le_bytes(raw_bytes(bits, 8)?.try_into().ok()?),
                    _ => i64::from(bits.rs()?),
                };
                data.push(int(code, value));
            }
            DataType::Bool => data.push(Value {
                code,
                data: Data::Bool(bits.rc()? != 0),
            }),
            DataType::Handle => data.push(Value {
                code,
                data: Data::Handle(u64::from_le_bytes(raw_bytes(bits, 8)?.try_into().ok()?)),
            }),
            DataType::Binary => {
                let len = usize::from(bits.rc()?);
                data.push(Value {
                    code,
                    data: Data::Binary(raw_bytes(bits, len)?),
                });
            }
            DataType::Text => {
                let len = usize::try_from(bits.rs()?).ok()?;
                // コード・ページ
                bits.rc()?;
                data.push(text_value(code, &raw_bytes(bits, len)?));
            }
        }
    }
    if bits.pos() != end {
        return None;
    }
    let mut values = vec![int(280, i64::from(bits.bs()?))];
    values.extend(data);
    Some(values)
}

/// 表のレコードの共通の欄（名前 2・参照される印・外部参照の番号・外部参照に依る印）と、
/// LAYER・STYLE・LTYPE の欄（ODS 5.4.1、R2000）。外部参照に依る印は、DXF と同じく 70 の 16 とする。
/// ハンドルの並びは、外部参照のブロックの後に型ごとのハンドルが続く
fn table_record(bits: &mut Bits, type_code: u16, handles: &mut Vec<i16>) -> Option<Vec<Value>> {
    let name = bits.text()?;
    // 参照される印（64）は、DXF に書かれないので置かない。外部参照の番号は、R2010 の図面には
    // ない（図面で確かめた）
    bits.bit()?;
    if bits.release() < Release::R2010 {
        bits.bs()?;
    }
    let xdep = if bits.bit()? { 16 } else { 0 };
    handles.push(0);
    let mut values = vec![Value {
        code: 2,
        data: Data::Text(name),
    }];
    match type_code {
        LAYER => {
            // 凍結 1・表示しない 2・新しいビューポートで凍結 4・ロック 8・印刷 16 と、線の太さの番号
            let flags = bits.bs()?;
            let dxf = [(1, 1), (4, 2), (8, 4)]
                .iter()
                .filter(|(dwg, _)| flags & dwg != 0)
                .fold(xdep, |sum, (_, dxf)| sum | dxf);
            values.push(int(70, dxf));
            // 表示しない画層は、DXF と同じく色を負にする
            // 真の色は、図形と同じく 420 だけを置く
            let color = bits.cmc()?;
            match color.rgb {
                Some(rgb) => values.push(int(420, i64::from(rgb))),
                None => {
                    let index = i64::from(color.index);
                    values.push(int(62, if flags & 2 != 0 { -index } else { index }));
                }
            }
            if flags & 16 == 0 {
                values.push(Value {
                    code: 290,
                    data: Data::Bool(false),
                });
            }
            values.push(int(
                370,
                lineweight(u16::try_from((flags & 0x03E0) >> 5).ok()?)?,
            ));
            // 印刷スタイル（390）と線種（6）。R2010 からは、その間に材質（347）がある（決定 0094）
            if bits.release() >= Release::R2010 {
                handles.extend([390, 347, 6]);
            } else {
                handles.extend([390, 6]);
            }
        }
        STYLE => {
            // 形のファイルの印（70 の 1）と、縦書きの印（70 の 4）
            let shape = bits.bit()?;
            let vertical = bits.bit()?;
            let flags = xdep | if shape { 1 } else { 0 } | if vertical { 4 } else { 0 };
            values.push(int(70, flags));
            values.push(real(40, bits.bd()?));
            values.push(real(41, bits.bd()?));
            values.push(real(50, bits.bd()?.to_degrees()));
            values.push(int(71, i64::from(bits.rc()?)));
            values.push(real(42, bits.bd()?));
            values.push(text_value(3, &bits.text()?));
            values.push(text_value(4, &bits.text()?));
        }
        LTYPE => {
            values.push(int(70, xdep));
            values.push(text_value(3, &bits.text()?));
            let length = bits.bd()?;
            values.push(int(72, i64::from(bits.rc()?)));
            let count = bits.rc()?;
            values.push(int(73, i64::from(count)));
            values.push(real(40, length));
            let mut dashes = Vec::new();
            for _ in 0..count {
                let dash = bits.bd()?;
                let shape = bits.bs()?;
                let (x, y) = (bits.rd()?, bits.rd()?);
                let (scale, rotation) = (bits.bd()?, bits.bd()?);
                let flags = bits.bs()?;
                dashes.push((dash, shape, x, y, scale, rotation, flags));
            }
            // 文字の要素の文字列は、256 バイトの領域に、形の番号の位置から 0 で終わる。R2010 からは、
            // 文字の要素があるときだけ 512 バイトの領域があり、文字列は UTF-16LE
            let unicode = bits.release() >= Release::R2010;
            let area = if !unicode {
                raw_bytes(bits, 256)?
            } else if dashes.iter().any(|dash| dash.6 & 2 != 0) {
                raw_bytes(bits, 512)?
            } else {
                Vec::new()
            };
            // DWG の 8 は文字の要素に付く印で、R2013 より前の DXF には書かれない
            let mask = if bits.release() >= Release::R2013 {
                15
            } else {
                7
            };
            for (dash, shape, x, y, scale, rotation, flags) in dashes {
                values.push(real(49, dash));
                values.push(int(74, i64::from(flags & mask)));
                // 要素ごとに、形のファイルの文字の体裁のハンドル（340）を持つ。形も文字もない要素の
                // ハンドルは置かない
                if flags & 6 == 0 {
                    handles.push(0);
                    continue;
                }
                handles.push(340);
                let text = flags & 2 != 0;
                values.push(int(75, if text { 0 } else { i64::from(shape) }));
                values.extend([
                    real(46, scale),
                    real(50, rotation.to_degrees()),
                    real(44, x),
                    real(45, y),
                ]);
                if text {
                    let rest = area.get(usize::try_from(shape).ok()?..)?;
                    let text = if unicode {
                        let units: Vec<u16> = rest
                            .as_chunks::<2>()
                            .0
                            .iter()
                            .map(|&pair| u16::from_le_bytes(pair))
                            .take_while(|&unit| unit != 0)
                            .collect();
                        String::from_utf16_lossy(&units).into_bytes()
                    } else {
                        let end = rest.iter().position(|&b| b == 0)?;
                        rest.get(..end)?.to_vec()
                    };
                    values.push(text_value(9, &text));
                }
            }
        }
        VIEW => values.extend(view(bits, xdep, handles)?),
        UCS => {
            values.push(int(70, xdep));
            values.extend(ucs(bits)?);
            handles.extend([346, 0]);
        }
        VPORT => {
            values.push(int(70, xdep));
            values.extend(vport(bits)?);
            // R2010 からは、外部参照のブロックの後に、背景・表示スタイル・太陽のハンドルが続く
            if bits.release() >= Release::R2010 {
                handles.extend([0, 0, 0]);
            }
            handles.extend([345, 346]);
        }
        DIMSTYLE => {
            values.push(int(70, xdep));
            values.extend(dimstyle(bits)?);
            handles.extend([340, 341, 342, 343, 344]);
            // R2010 からは、寸法線と 2 つの補助線の線種のハンドルが続く
            if bits.release() >= Release::R2010 {
                handles.extend([345, 346, 347]);
            }
        }
        // BLOCK_RECORD の印・基点・外部参照のパス・説明は、DXF では BLOCK に書かれる。ここで読んで、
        // 表を作るときに BLOCK へ移す（BLOCK_FIELDS）
        BLOCK_RECORD => {
            // 名前のない 1・属性の定義を持つ 2・外部参照 4・重ねる外部参照 8 と、読み込んだ外部参照（32）
            let mut flags = xdep;
            for bit in [1, 2, 4, 8] {
                if bits.bit()? {
                    flags |= bit;
                }
            }
            if bits.bit()? && flags & 4 != 0 {
                flags |= 32;
            }
            if bits.release() >= Release::R2004 {
                bits.bl()?;
            }
            values.push(int(70, flags));
            values.extend(point3(bits, 10)?);
            let path = bits.text()?;
            if !path.is_empty() {
                values.push(text_value(1, &path));
            }
            // 参照する INSERT の数だけ 0 でない RC が並び、0 で終わる
            while bits.rc()? != 0 {}
            let description = bits.text()?;
            if !description.is_empty() {
                values.push(text_value(4, &description));
            }
        }
        _ => {
            values.push(int(70, xdep));
            // APPID は、使い道の分からない RC を持つ
            if type_code == APPID {
                bits.rc()?;
            }
        }
    }
    Some(values)
}

/// VIEW・VPORT の見え方の印（71）。4 つのビットは、透視（1）・前の切り取り（2）・後ろの切り取り（4）・
/// 前の切り取りが目の位置にあること（DXF の 16 の逆）の順
fn view_mode(bits: &mut Bits) -> Option<i64> {
    let mut mode = 0;
    for flag in [1, 2, 4] {
        if bits.bit()? {
            mode |= flag;
        }
    }
    if !bits.bit()? {
        mode |= 16;
    }
    Some(mode)
}

/// VIEW の欄（ODS 5.4.1 の VIEW の節、R2000）。高さ（40）・中心（10）・幅（41）・向き（11）・
/// 注視点（12）・レンズ（42）・切り取り（43・44）・ねじれ（50、度に直す）・見え方の印（71）・
/// 描き方（281）と、UCS を伴う印（72）を置く。UCS を伴うときは、UCS の原点と軸（110〜112）・
/// 正投影の種類（79）・高さ（146）も置き、ハンドルの並びでは基準の UCS（346）と名前の UCS（345）が続く。
/// 紙空間の印は、70 の 1 とする
pub(super) fn view(bits: &mut Bits, xdep: i64, handles: &mut Vec<i16>) -> Option<Vec<Value>> {
    let height = bits.bd()?;
    let width = bits.bd()?;
    let center = point2(bits, 10)?;
    let target = point3(bits, 12)?;
    let direction = point3(bits, 11)?;
    let twist = bits.bd()?;
    let (lens, front, back) = (bits.bd()?, bits.bd()?, bits.bd()?);
    let mode = view_mode(bits)?;
    let render = bits.rc()?;
    // R2010 からは、既定の照明を使うか（B）・照明の種類（RC）・明るさと対比（BD）・環境の色（CMC）が
    // 続く（図面で確かめた）
    if bits.release() >= Release::R2010 {
        bits.bit()?;
        bits.rc()?;
        bits.bd()?;
        bits.bd()?;
        bits.cmc()?;
    }
    let paper = bits.bit()?;
    let mut values = vec![int(70, xdep | i64::from(paper)), real(40, height)];
    values.extend(center);
    values.push(real(41, width));
    values.extend(direction);
    values.extend(target);
    values.extend([
        real(42, lens),
        real(43, front),
        real(44, back),
        real(50, twist.to_degrees()),
        int(71, mode),
        int(281, i64::from(render)),
    ]);
    let associated = bits.bit()?;
    values.push(int(72, i64::from(associated)));
    // R2010 からは、外部参照のブロックの後に、背景・表示スタイル・太陽のハンドルが続く
    if bits.release() >= Release::R2010 {
        handles.extend([0, 0, 0]);
    }
    if associated {
        for code in [110, 111, 112] {
            values.extend(point3(bits, code)?);
        }
        let elevation = bits.bd()?;
        values.push(int(79, i64::from(bits.bs()?)));
        values.push(real(146, elevation));
        handles.extend([346, 345]);
    }
    Some(values)
}

/// UCS の欄（ODS 5.4.1 の UCS の節、R2000）。原点（10）・X 軸（11）・Y 軸（12）・正投影の種類（79）・
/// 高さ（146）を置く。後に続く正投影の型（BS）は、DXF の 71 と 13 の組と並びが合わないので置かない
fn ucs(bits: &mut Bits) -> Option<Vec<Value>> {
    let mut values = point3(bits, 10)?;
    values.extend(point3(bits, 11)?);
    values.extend(point3(bits, 12)?);
    let elevation = bits.bd()?;
    values.push(int(79, i64::from(bits.bs()?)));
    values.push(real(146, elevation));
    bits.bs()?;
    Some(values)
}

/// VPORT の欄（ODS 5.4.1 の VPORT の節、R2000）。DWG は幅を高さとの積で持つので、DXF と同じく
/// 高さとの比（41）に直す。UCS に従う印は、見え方の印（71）の 8 とする
pub(super) fn vport(bits: &mut Bits) -> Option<Vec<Value>> {
    let height = bits.bd()?;
    let width = bits.bd()?;
    let center = point2(bits, 12)?;
    let target = point3(bits, 17)?;
    let direction = point3(bits, 16)?;
    let twist = bits.bd()?;
    let (lens, front, back) = (bits.bd()?, bits.bd()?, bits.bd()?);
    let mode = view_mode(bits)?;
    let render = bits.rc()?;
    // R2010 からは、VIEW と同じく照明の欄が続く（図面で確かめた、決定 0096）
    if bits.release() >= Release::R2010 {
        bits.bit()?;
        bits.rc()?;
        bits.bd()?;
        bits.bd()?;
        bits.cmc()?;
    }
    let mut values = point2(bits, 10)?;
    values.extend(point2(bits, 11)?);
    values.extend(center);
    let follow = if bits.bit()? { 8 } else { 0 };
    let zoom = bits.bs()?;
    let fast = bits.bit()?;
    let icon = bits.bb()?;
    let grid = bits.bit()?;
    let grid_spacing = point2(bits, 15)?;
    let snap = bits.bit()?;
    let snap_style = bits.bit()?;
    let isopair = bits.bs()?;
    let snap_rotation = bits.bd()?;
    values.extend(point2(bits, 13)?);
    values.extend(point2(bits, 14)?);
    values.extend(grid_spacing);
    values.extend(direction);
    values.extend(target);
    values.extend([
        real(40, height),
        real(41, if height == 0.0 { width } else { width / height }),
        real(42, lens),
        real(43, front),
        real(44, back),
        real(50, snap_rotation.to_degrees()),
        real(51, twist.to_degrees()),
        int(71, mode | follow),
        int(72, i64::from(zoom)),
        int(73, i64::from(fast)),
        int(74, i64::from(icon)),
        int(75, i64::from(snap)),
        int(76, i64::from(grid)),
        int(77, i64::from(snap_style)),
        int(78, i64::from(isopair)),
        int(281, i64::from(render)),
    ]);
    // 使い道の分からない B の後に、ビューポートごとの UCS の印が続く
    bits.bit()?;
    values.push(int(65, i64::from(bits.bit()?)));
    for code in [110, 111, 112] {
        values.extend(point3(bits, code)?);
    }
    let elevation = bits.bd()?;
    values.push(int(79, i64::from(bits.bs()?)));
    values.push(real(146, elevation));
    Some(values)
}

/// DIMSTYLE の欄の読み方
#[derive(Clone, Copy)]
pub(super) enum Dim {
    Text,
    Real(f64),
    Bit(bool),
    Short(i16),
    /// CMC の色
    Color(i16),
}

/// DIMSTYLE の欄（ODS 5.4.1 の DIMSTYLE の節、R2000）の、DWG での並びと、グループ・コードと、
/// DXF が書かない既定の値
pub(super) const DIM_FIELDS: [(i16, Dim); 59] = {
    use Dim::{Bit, Color, Real, Short, Text};
    [
        (3, Text),
        (4, Text),
        (40, Real(1.0)),
        (41, Real(0.18)),
        (42, Real(0.0625)),
        (43, Real(0.38)),
        (44, Real(0.18)),
        (45, Real(0.0)),
        (46, Real(0.0)),
        (47, Real(0.0)),
        (48, Real(0.0)),
        (71, Bit(false)),
        (72, Bit(false)),
        (73, Bit(true)),
        (74, Bit(true)),
        (75, Bit(false)),
        (76, Bit(false)),
        (77, Short(0)),
        (78, Short(0)),
        (79, Short(0)),
        (140, Real(0.18)),
        (141, Real(0.09)),
        (142, Real(0.0)),
        (143, Real(25.4)),
        (144, Real(1.0)),
        (145, Real(0.0)),
        (146, Real(1.0)),
        (147, Real(0.09)),
        (148, Real(0.0)),
        (170, Bit(false)),
        (171, Short(2)),
        (172, Bit(false)),
        (173, Bit(false)),
        (174, Bit(false)),
        (175, Bit(false)),
        (176, Color(0)),
        (177, Color(0)),
        (178, Color(0)),
        (179, Short(0)),
        (271, Short(4)),
        (272, Short(4)),
        (273, Short(2)),
        (274, Short(2)),
        (275, Short(0)),
        (276, Short(0)),
        (277, Short(2)),
        (278, Short(46)),
        (279, Short(0)),
        (280, Short(0)),
        (281, Bit(false)),
        (282, Bit(false)),
        (283, Short(1)),
        (284, Short(0)),
        (285, Short(0)),
        (286, Short(0)),
        (288, Bit(false)),
        (289, Short(3)),
        (371, Short(-2)),
        (372, Short(-2)),
    ]
};

/// 版 `release` の DIMSTYLE の欄の並び。コードが 0 の欄は、DXF に書かれない
pub(super) fn dim_fields(release: Release) -> Vec<(i16, Dim)> {
    // R2010 からは、見出しの変数と同じく、DIMTM（48）の後に DIMFXL・DIMJOGANG・DIMTFILL・
    // DIMTFILLCLR、DIMAZIN（79）の後に DIMARCSYM、DIMATFIT（289）の後に DIMFXLON・
    // DIMTXTDIRECTION・DIMALTMZF・DIMALTMZS・DIMMZF・DIMMZS が入る（図面で確かめた）
    let added: &[(i16, &[(i16, Dim)])] = if release >= Release::R2010 {
        &[
            (
                48,
                &[
                    (49, Dim::Real(1.0)),
                    (50, Dim::Real(std::f64::consts::FRAC_PI_4)),
                    (69, Dim::Short(0)),
                    (70, Dim::Color(0)),
                ],
            ),
            (79, &[(90, Dim::Short(0))]),
            (
                289,
                &[
                    (290, Dim::Bit(false)),
                    (294, Dim::Bit(false)),
                    (0, Dim::Real(0.0)),
                    (0, Dim::Text),
                    (0, Dim::Real(0.0)),
                    (0, Dim::Text),
                ],
            ),
        ]
    } else {
        &[]
    };
    DIM_FIELDS
        .iter()
        .flat_map(|field| {
            let extra = added
                .iter()
                .find(|(after, _)| *after == field.0)
                .map_or(&[][..], |(_, extra)| *extra);
            std::iter::once(field).chain(extra)
        })
        .copied()
        .collect()
}

/// DIMSTYLE の欄。DXF と同じく、既定の値と同じ欄は置かない
fn dimstyle(bits: &mut Bits) -> Option<Vec<Value>> {
    let mut values = Vec::new();
    for (code, field) in dim_fields(bits.release()) {
        // コードが 0 の欄は、DXF に書かれないので読み飛ばす
        if code == 0 {
            match field {
                Dim::Text => bits.text().map(drop)?,
                _ => bits.bd().map(drop)?,
            }
            continue;
        }
        match field {
            Dim::Text => {
                let text = bits.text()?;
                if !text.is_empty() {
                    values.push(text_value(code, &text));
                }
            }
            Dim::Real(default) => {
                let value = bits.bd()?;
                if value != default {
                    values.push(real(code, value));
                }
            }
            Dim::Bit(default) => {
                let value = bits.bit()?;
                if value != default {
                    values.push(int(code, i64::from(value)));
                }
            }
            Dim::Short(default) => {
                let value = bits.bs()?;
                if value != default {
                    values.push(int(code, i64::from(value)));
                }
            }
            Dim::Color(default) => {
                let value = bits.cmc()?.index;
                if value != default {
                    values.push(int(code, i64::from(value)));
                }
            }
        }
    }
    Some(values)
}

/// DWG の線の太さの番号を、DXF の値（1/100 mm。-1 は ByLayer、-2 は ByBlock、-3 は既定）に直す
fn lineweight(index: u16) -> Option<i64> {
    const WIDTHS: [i64; 24] = [
        0, 5, 9, 13, 15, 18, 20, 25, 30, 35, 40, 50, 53, 60, 70, 80, 90, 100, 106, 120, 140, 158,
        200, 211,
    ];
    match index {
        29 => Some(-1),
        30 => Some(-2),
        31 => Some(-3),
        _ => WIDTHS.get(usize::from(index)).copied(),
    }
}

/// GROUP の欄（ODS 5.4.1 の GROUP の節、R2000）。説明（300）・名前のない印（70）・選べる印（71）を置く。
/// 図形の数だけ、ハンドルの並びの図形（340）を後で置く
fn group(bits: &mut Bits, handles: &mut Vec<i16>) -> Option<Vec<Value>> {
    let values = vec![
        text_value(300, &bits.text()?),
        int(70, i64::from(bits.bs()?)),
        int(71, i64::from(bits.bs()?)),
    ];
    let count = bits.bl()?;
    for _ in 0..count {
        handles.push(340);
    }
    Some(values)
}

/// SCALE の欄（クラスで決まる型）。名前（300）・紙の単位（140）・図面の単位（141）・単位の尺度の印（290）
fn scale(bits: &mut Bits) -> Option<Vec<Value>> {
    Some(vec![
        int(70, i64::from(bits.bs()?)),
        text_value(300, &bits.text()?),
        real(140, bits.bd()?),
        real(141, bits.bd()?),
        Value {
            code: 290,
            data: Data::Bool(bits.bit()?),
        },
    ])
}

/// IMAGE の欄（ODS 5.4.1 の 20.4.80。クラスで決まる型）。DXF と同じく、切り抜きの境界が矩形（71 = 1）でも、
/// 頂点の数（91 = 2）を置く。画像の定義と反応子のハンドルは、ハンドルの流れで読む
fn image(bits: &mut Bits, release: Release) -> Option<Vec<Value>> {
    let mut values = vec![int(90, i64::from(bits.bl()?))];
    values.extend(point3(bits, 10)?);
    values.extend(point3(bits, 11)?);
    values.extend(point3(bits, 12)?);
    values.extend(point2(bits, 13)?);
    values.push(int(70, i64::from(bits.bs()?)));
    values.push(int(280, i64::from(bits.bit()?)));
    for code in [281, 282, 283] {
        values.push(int(code, i64::from(bits.rc()?)));
    }
    if release >= Release::R2010 {
        values.push(Value {
            code: 290,
            data: Data::Bool(bits.bit()?),
        });
    }
    let kind = bits.bs()?;
    values.push(int(71, i64::from(kind)));
    let count = if kind == 1 { 2 } else { bits.bl()? };
    values.push(int(91, i64::from(count)));
    for _ in 0..count {
        values.extend(point2(bits, 14)?);
    }
    Some(values)
}

/// IMAGEDEF の欄（ODS 5.4.1 の 20.4.81。クラスで決まる型）。DXF の順に、版（90）・ファイルの場所（1）・
/// 画素の数（10）・1 画素の大きさ（11）・読み込んだ印（280）・解像度の単位（281）を置く
fn imagedef(bits: &mut Bits) -> Option<Vec<Value>> {
    let version = int(90, i64::from(bits.bl()?));
    let size = point2(bits, 10)?;
    let path = text_value(1, &bits.text()?);
    let loaded = int(280, i64::from(bits.bit()?));
    let units = int(281, i64::from(bits.rc()?));
    let pixel = point2(bits, 11)?;
    let mut values = vec![version, path];
    values.extend(size);
    values.extend(pixel);
    values.extend([loaded, units]);
    Some(values)
}

/// LAYOUT の欄（ODS 5.4.1 の LAYOUT の節）。印刷の設定の欄に続けて、レイアウトの欄を置く。R2004 からは、
/// 印刷の設定に陰影の印刷の欄（76〜78）が加わり、レイアウトの欄の後にビューポートの数が続く（決定 0082）
fn layout(bits: &mut Bits, release: Release, handles: &mut Vec<i16>) -> Option<Vec<Value>> {
    let r2004 = release >= Release::R2004;
    // R2004 からは、印刷するビューの名前（6）の代わりに、そのハンドルを持つ。値には置かない
    if r2004 {
        handles.push(0);
    }
    // R2007 からは、表示スタイルのハンドルも持つ（読むのは R2010 から）
    if release >= Release::R2010 {
        handles.push(0);
    }
    // 紙空間のブロックのレコード・最後に使ったビューポート・基準の UCS・名前の UCS
    handles.extend([330, 331, 346, 345]);
    let mut values = vec![
        text_value(1, &bits.text()?),
        text_value(2, &bits.text()?),
        int(70, i64::from(bits.bs()?)),
    ];
    // 余白（左・下・右・上）と紙の幅・高さ
    for code in 40..=45 {
        values.push(real(code, bits.bd()?));
    }
    values.push(text_value(4, &bits.text()?));
    values.extend([real(46, bits.bd()?), real(47, bits.bd()?)]);
    for code in [72, 73, 74] {
        values.push(int(code, i64::from(bits.bs()?)));
    }
    // 印刷する窓の隅
    values.extend([real(48, bits.bd()?), real(49, bits.bd()?)]);
    values.extend([real(140, bits.bd()?), real(141, bits.bd()?)]);
    if !r2004 {
        values.push(text_value(6, &bits.text()?));
    }
    values.extend([real(142, bits.bd()?), real(143, bits.bd()?)]);
    values.push(text_value(7, &bits.text()?));
    values.push(int(75, i64::from(bits.bs()?)));
    values.push(real(147, bits.bd()?));
    values.extend([real(148, bits.bd()?), real(149, bits.bd()?)]);
    // 陰影の印刷の方法・解像度・DPI
    if r2004 {
        for code in [76, 77, 78] {
            values.push(int(code, i64::from(bits.bs()?)));
        }
    }
    // レイアウトの欄
    values.push(text_value(1, &bits.text()?));
    values.push(int(71, i64::from(bits.bs()?)));
    values.push(int(70, i64::from(bits.bs()?)));
    values.extend(point3(bits, 13)?);
    values.extend(point2(bits, 10)?);
    values.extend(point2(bits, 11)?);
    values.extend(point3(bits, 12)?);
    values.extend(point3(bits, 16)?);
    values.extend(point3(bits, 17)?);
    values.push(real(146, bits.bd()?));
    values.push(int(76, i64::from(bits.bs()?)));
    values.extend(point3(bits, 14)?);
    values.extend(point3(bits, 15)?);
    if r2004 {
        // ビューポートの数。ODS は RL とするが、試料の図面では BL だった。ビューポートのハンドルは置かない
        for _ in 0..bits.bl()? {
            handles.push(0);
        }
    }
    Some(values)
}

/// RC を `n` 個読む
fn raw_bytes(bits: &mut Bits, n: usize) -> Option<Vec<u8>> {
    (0..n).map(|_| bits.rc()).collect()
}

/// INSERT の欄（ODS 5.4.1 の 20.4.9、R2000）。挿入点はいつも置き、尺度（41・42・43）・回転（50）・
/// 押し出しの向き（210・220・230）は既定の値でなければ、属性が続くこと（66）は続くときだけ置く。
/// MINSERT（`grid`）は、列と行の数（70・71）と間隔（44・45）も、既定の値（1・1・0・0）でなければ置く
fn insert(bits: &mut Bits, grid: bool) -> Option<Vec<Value>> {
    let mut values = point3(bits, 10)?;
    let (x, y, z) = match bits.bb()? {
        3 => (1.0, 1.0, 1.0),
        1 => (1.0, bits.dd(1.0)?, bits.dd(1.0)?),
        2 => {
            let x = bits.rd()?;
            (x, x, x)
        }
        _ => {
            let x = bits.rd()?;
            (x, bits.dd(x)?, bits.dd(x)?)
        }
    };
    for (code, scale) in [(41, x), (42, y), (43, z)] {
        if scale != 1.0 {
            values.push(real(code, scale));
        }
    }
    let rotation = bits.bd()?;
    if rotation != 0.0 {
        values.push(real(50, rotation.to_degrees()));
    }
    let extrusion = point3(bits, 210)?;
    if !is_up(&extrusion) {
        values.extend(extrusion);
    }
    if bits.bit()? {
        values.push(int(66, 1));
    }
    if grid {
        for code in [70, 71] {
            let count = bits.bs()?;
            if count != 1 {
                values.push(int(code, i64::from(count)));
            }
        }
        for code in [44, 45] {
            let spacing = bits.bd()?;
            if spacing != 0.0 {
                values.push(real(code, spacing));
            }
        }
    }
    Some(values)
}

/// MTEXT の欄（ODS 5.4.1 の 20.4.46、R2000）。文字列は DXF と同じく、250 バイトずつの 3 と、残りの 1 に
/// 分けて置く。X 軸の向き（11・21・31）と押し出しの向きは既定の値でなければ置く。文字の範囲の大きさは
/// DXF にないので置かない
fn mtext(bits: &mut Bits) -> Option<Vec<Value>> {
    let mut values = point3(bits, 10)?;
    let extrusion = point3(bits, 210)?;
    let x_axis = point3(bits, 11)?;
    values.push(real(41, bits.bd()?));
    // R2010 からは、枠の高さ（46）が続く
    if bits.release() >= Release::R2010 {
        values.push(real(46, bits.bd()?));
    }
    values.push(real(40, bits.bd()?));
    values.push(int(71, i64::from(bits.bs()?)));
    values.push(int(72, i64::from(bits.bs()?)));
    bits.bd()?;
    bits.bd()?;
    // DXF は、制御文字を ^ の形に直す前の文字列を 250 バイトずつに分ける
    let text = bits.text()?;
    let mut rest = text.as_slice();
    while rest.len() > 250 {
        let (chunk, tail) = rest.split_at(250);
        values.push(Value {
            code: 3,
            data: Data::Text(caret(chunk)),
        });
        rest = tail;
    }
    values.push(Value {
        code: 1,
        data: Data::Text(caret(rest)),
    });
    values.push(int(73, i64::from(bits.bs()?)));
    values.push(real(44, bits.bd()?));
    bits.bit()?;
    // R2004 からは背景の欄が続く。印（90）の 1 は、背景の大きさの倍率（45）・色（63）・透明度（441）が
    // 続くことを表す。DXF と同じく、印が 0 でなければ置く
    if bits.release() >= Release::R2004 {
        let background = bits.bl()?;
        if background != 0 {
            values.push(int(90, i64::from(background)));
        }
        if background & 1 != 0 {
            values.push(real(45, bits.bd()?));
            // 真の色は、図形と同じく 421 だけを置く
            let color = bits.cmc()?;
            match color.rgb {
                Some(rgb) => values.push(int(421, i64::from(rgb))),
                None => values.push(int(63, i64::from(color.index))),
            }
            values.push(int(441, i64::from(bits.bl()?)));
        }
    }
    if reals(&x_axis) != [Some(1.0), Some(0.0), Some(0.0)] {
        values.extend(x_axis);
    }
    if !is_up(&extrusion) {
        values.extend(extrusion);
    }
    Some(values)
}

/// R2018 からの MTEXT の、背景の欄の後の欄を読み飛ばす。注釈でないか（B）・版（BS）・既定か（B）・
/// 寄せ方（BL）・X 軸の向き・挿入点・幅と高さと範囲の幅と高さ（BD）・段組みの種類（BL）の順（図面で
/// 確かめた）。段組みのある欄は、まだ確かめていないので読まない
fn mtext_columns(bits: &mut Bits) -> Option<()> {
    bits.bit()?;
    bits.bs()?;
    bits.bit()?;
    bits.bl()?;
    point3(bits, 10)?;
    point3(bits, 11)?;
    for _ in 0..4 {
        bits.bd()?;
    }
    (bits.bl()? == 0).then_some(())
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

/// LINE の欄（ODS 5.4.1 の 20.4.17）。端点はいつも置き、厚さと押し出しの向きは既定でなければ置く
fn line(bits: &mut Bits) -> Option<Vec<Value>> {
    let flat = bits.bit()?;
    let start_x = bits.rd()?;
    let end_x = bits.dd(start_x)?;
    let start_y = bits.rd()?;
    let end_y = bits.dd(start_y)?;
    let (start_z, end_z) = if flat {
        (0.0, 0.0)
    } else {
        let start_z = bits.rd()?;
        (start_z, bits.dd(start_z)?)
    };
    let mut values = vec![
        real(10, start_x),
        real(20, start_y),
        real(30, start_z),
        real(11, end_x),
        real(21, end_y),
        real(31, end_z),
    ];
    thickness_and_extrusion(bits, &mut values)?;
    Some(values)
}

/// CIRCLE の欄（ODS 5.4.1 の 20.4.20）。中心と半径
fn circle(bits: &mut Bits) -> Option<Vec<Value>> {
    let mut values = point3(bits, 10)?;
    values.push(real(40, bits.bd()?));
    thickness_and_extrusion(bits, &mut values)?;
    Some(values)
}

/// ARC の欄（ODS 5.4.1 の 20.4.18）。CIRCLE の欄と、始まりと終わりの角度。DWG はラジアン、
/// DXF は度で書くので、度に直す
fn arc(bits: &mut Bits) -> Option<Vec<Value>> {
    let mut values = circle(bits)?;
    values.push(real(50, bits.bd()?.to_degrees()));
    values.push(real(51, bits.bd()?.to_degrees()));
    Some(values)
}

/// POINT の欄（ODS 5.4.1 の 20.4.31）。X 軸の角度（50）は 0 でなければ度に直して置く
fn point(bits: &mut Bits) -> Option<Vec<Value>> {
    let mut values = point3(bits, 10)?;
    thickness_and_extrusion(bits, &mut values)?;
    let angle = bits.bd()?;
    if angle != 0.0 {
        values.push(real(50, angle.to_degrees()));
    }
    Some(values)
}

/// SOLID・TRACE の欄（ODS 5.4.1 の 20.4.35・20.4.36）。4 つの角の Z（30〜33）は標高とする
fn solid(bits: &mut Bits) -> Option<Vec<Value>> {
    let thickness = if bits.bit()? { None } else { Some(bits.bd()?) };
    let elevation = bits.bd()?;
    let mut values = Vec::new();
    for code in 10..14 {
        values.push(real(code, bits.rd()?));
        values.push(real(code + 10, bits.rd()?));
        values.push(real(code + 20, elevation));
    }
    if let Some(thickness) = thickness {
        values.push(real(39, thickness));
    }
    if !bits.bit()? {
        values.extend(point3(bits, 210)?);
    }
    Some(values)
}

/// 3DFACE の欄（ODS 5.4.1 の 20.4.32、R2000）。2 つ目からの角は、前の角を既定の値とする DD。
/// 見えない辺の印（70）は、記録されていて 0 でなければ置く
fn face3d(bits: &mut Bits) -> Option<Vec<Value>> {
    let no_flags = bits.bit()?;
    let z_is_zero = bits.bit()?;
    let x = bits.rd()?;
    let y = bits.rd()?;
    let z = if z_is_zero { 0.0 } else { bits.rd()? };
    let mut corner = [x, y, z];
    let mut values = Vec::new();
    for code in 10..14 {
        if code > 10 {
            for c in &mut corner {
                *c = bits.dd(*c)?;
            }
        }
        let [x, y, z] = corner;
        values.extend([real(code, x), real(code + 10, y), real(code + 20, z)]);
    }
    if !no_flags {
        let invisible = bits.bs()?;
        if invisible != 0 {
            values.push(int(70, i64::from(invisible)));
        }
    }
    Some(values)
}

/// SHAPE の欄（ODS 5.4.1 の 20.4.33）。挿入点（10）と大きさ（40）を置き、回転（50）・幅の係数（41）・
/// 傾き（51）・厚さ（39）・押し出しの向きは既定の値でなければ置く。角度は度に直す。DXF の形の名前（2）は、
/// DWG では形の番号と形のファイル（文字の体裁）で記録され、名前はファイルの中にあるので、まだ置かない
fn shape(bits: &mut Bits) -> Option<Vec<Value>> {
    let mut values = point3(bits, 10)?;
    values.push(real(40, bits.bd()?));
    for (code, default, angle) in [(50, 0.0, true), (41, 1.0, false), (51, 0.0, true)] {
        let value = bits.bd()?;
        if value != default {
            values.push(real(code, if angle { value.to_degrees() } else { value }));
        }
    }
    let thickness = bits.bd()?;
    if thickness != 0.0 {
        values.push(real(39, thickness));
    }
    // 形の番号
    bits.bs()?;
    let extrusion = point3(bits, 210)?;
    if !is_up(&extrusion) {
        values.extend(extrusion);
    }
    Some(values)
}

/// ELLIPSE の欄（ODS 5.4.1 の 20.4.39）。始まりと終わりの角度（41・42）は、DXF も
/// ラジアンで書くのでそのまま置く。押し出しの向きは既定の値でなければ置く
fn ellipse(bits: &mut Bits) -> Option<Vec<Value>> {
    let mut values = point3(bits, 10)?;
    values.extend(point3(bits, 11)?);
    let extrusion = point3(bits, 210)?;
    values.push(real(40, bits.bd()?));
    values.push(real(41, bits.bd()?));
    values.push(real(42, bits.bd()?));
    if !is_up(&extrusion) {
        values.extend(extrusion);
    }
    Some(values)
}

/// RAY・XLINE の欄（ODS 5.4.1 の 20.4.42・20.4.43）。始まりの点と向き
fn ray(bits: &mut Bits) -> Option<Vec<Value>> {
    let mut values = point3(bits, 10)?;
    values.extend(point3(bits, 11)?);
    Some(values)
}

/// LWPOLYLINE の欄（ODS 5.4.1 の 20.4.85、R2000）。DWG の印のうち、閉じていること（0x200）だけを
/// DXF の 70 の 1 に直す。ほかの印は、続く欄があるかを表す。DXF と同じく、頂点ごとの幅がなければ
/// 一定の幅（43）を置き、膨らみ（42）は 0 でなければ置く
fn lwpolyline(bits: &mut Bits) -> Option<Vec<Value>> {
    let flags = bits.bs()?;
    let has = |bit: i16| flags & bit != 0;
    let width = if has(4) { bits.bd()? } else { 0.0 };
    let elevation = if has(8) { Some(bits.bd()?) } else { None };
    let thickness = if has(2) { Some(bits.bd()?) } else { None };
    let extrusion = if has(1) {
        Some(point3(bits, 210)?)
    } else {
        None
    };
    let count = usize::try_from(bits.bl()?).ok()?;
    let bulges = if has(16) {
        usize::try_from(bits.bl()?).ok()?
    } else {
        0
    };
    let widths = if has(32) {
        usize::try_from(bits.bl()?).ok()?
    } else {
        0
    };
    let mut points = Vec::new();
    if count > 0 {
        let mut x = bits.rd()?;
        let mut y = bits.rd()?;
        points.push((x, y));
        for _ in 1..count {
            x = bits.dd(x)?;
            y = bits.dd(y)?;
            points.push((x, y));
        }
    }
    let bulges: Vec<f64> = (0..bulges).map(|_| bits.bd()).collect::<Option<_>>()?;
    let widths: Vec<(f64, f64)> = (0..widths)
        .map(|_| Some((bits.bd()?, bits.bd()?)))
        .collect::<Option<_>>()?;
    let mut values = vec![
        int(90, i64::try_from(count).ok()?),
        int(70, i64::from(has(0x200))),
    ];
    if widths.is_empty() {
        values.push(real(43, width));
    }
    if let Some(elevation) = elevation {
        values.push(real(38, elevation));
    }
    if let Some(thickness) = thickness {
        values.push(real(39, thickness));
    }
    for (i, (x, y)) in points.into_iter().enumerate() {
        values.push(real(10, x));
        values.push(real(20, y));
        if let Some((start, end)) = widths.get(i) {
            values.push(real(40, *start));
            values.push(real(41, *end));
        }
        if let Some(bulge) = bulges.get(i).filter(|b| **b != 0.0) {
            values.push(real(42, *bulge));
        }
    }
    if let Some(extrusion) = extrusion {
        values.extend(extrusion);
    }
    Some(values)
}

/// SPLINE の欄（ODS 5.4.1 の 20.4.40、R2000）。場合（scenario）が 1 なら節点と制御点、2 なら
/// 当てはめる点を読む。DXF の印（70）と法線（210）は、DXF を書くときに平らかどうかから決まるので
/// 置かない。始まりと終わりの接線（12・13）は、0 でなければ置く
fn spline(bits: &mut Bits) -> Option<Vec<Value>> {
    let scenario = bits.bl()?;
    // R2013 からは、スプラインの印と、節点の取り方（どちらも BL）が続く。印の 1 が立つときは、
    // 場合が 1 でも、当てはめる点の欄だけがあり、制御点の欄はない（図面で確かめた）
    let mut fitted = scenario == 2;
    let mut fit_only = false;
    if bits.release() >= Release::R2013 {
        fit_only = bits.bl()? & 1 != 0;
        fitted |= fit_only;
        bits.bl()?;
    }
    let degree = bits.bl()?;
    let mut values = vec![int(71, i64::from(degree))];
    let mut fit = 0;
    let mut knots = 0;
    let mut controls = 0;
    let mut weighted = false;
    let mut tail = Vec::new();
    if !matches!(scenario, 1 | 2) {
        return None;
    }
    if fitted {
        let tolerance = bits.bd()?;
        let start = point3(bits, 12)?;
        let end = point3(bits, 13)?;
        fit = bits.bl()?;
        if fit_only && scenario == 1 {
            return spline_from_fit(bits, degree, fit, &start, &end);
        }
        tail.push(real(44, tolerance));
        for tangent in [start, end] {
            if reals(&tangent).iter().any(|r| *r != Some(0.0)) {
                tail.extend(tangent);
            }
        }
    }
    if scenario == 1 && !fit_only {
        // 印（70）の 4 は有理、1 は閉じている、2 は周期的。DXF の 70 は平らか（8）なども持つが、DWG に
        // 書かれないので置かない
        let mut flags = 0;
        for bit in [4, 1, 2] {
            if bits.bit()? {
                flags |= bit;
            }
        }
        values.insert(0, int(70, flags));
        tail.push(real(42, bits.bd()?));
        tail.push(real(43, bits.bd()?));
        knots = bits.bl()?;
        controls = bits.bl()?;
        weighted = bits.bit()?;
    }
    values.push(int(72, i64::from(knots)));
    values.push(int(73, i64::from(controls)));
    values.push(int(74, i64::from(fit)));
    values.extend(tail);
    for _ in 0..knots {
        values.push(real(40, bits.bd()?));
    }
    for _ in 0..controls {
        values.extend(point3(bits, 10)?);
        if weighted {
            values.push(real(41, bits.bd()?));
        }
    }
    for _ in 0..fit {
        values.extend(point3(bits, 11)?);
    }
    Some(values)
}

/// R2013 からの、制御点を書かないスプライン（場合が 1 で、当てはめる点だけを持つ）。DXF は制御点で
/// 書くので、3 次で当てはめる点が 2 つのときは、両端の点と接線から Bézier の制御点を求める（接線の
/// 3 分の 1 だけ両端から離す。図面で確かめた）。節点と制御点の許容差は書かれないので、DXF が置く
/// 値（1e-9 と 1e-10）を置く。それ以外は、当てはめる点の欄として置く
fn spline_from_fit(
    bits: &mut Bits,
    degree: i32,
    fit: i32,
    start: &[Value],
    end: &[Value],
) -> Option<Vec<Value>> {
    let mut points = Vec::new();
    for _ in 0..fit {
        points.push(point3(bits, 11)?);
    }
    let xyz = |values: &[Value]| -> Option<[f64; 3]> {
        let r = reals(values);
        Some([
            r.first().copied()??,
            r.get(1).copied()??,
            r.get(2).copied()??,
        ])
    };
    let mut values = vec![int(71, i64::from(degree))];
    if degree == 3 && fit == 2 {
        let (p0, p3) = (xyz(points.first()?)?, xyz(points.get(1)?)?);
        let ([x0, y0, z0], [x3, y3, z3]) = (p0, p3);
        let ([a, b, c], [d, e, f]) = (xyz(start)?, xyz(end)?);
        let p1 = [x0 + a / 3.0, y0 + b / 3.0, z0 + c / 3.0];
        let p2 = [x3 - d / 3.0, y3 - e / 3.0, z3 - f / 3.0];
        values.extend([
            int(72, 8),
            int(73, 4),
            int(74, 0),
            real(42, 1e-9),
            real(43, 1e-10),
        ]);
        for knot in [0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0] {
            values.push(real(40, knot));
        }
        for [x, y, z] in [p0, p1, p2, p3] {
            values.extend([real(10, x), real(20, y), real(30, z)]);
        }
        return Some(values);
    }
    values.extend([int(72, 0), int(73, 0), int(74, i64::from(fit))]);
    for tangent in [start, end] {
        if reals(tangent).iter().any(|r| *r != Some(0.0)) {
            values.extend(tangent.iter().cloned());
        }
    }
    values.extend(points.into_iter().flatten());
    Some(values)
}

/// 2D の VERTEX の欄（ODS 5.4.1 の 20.4.11、R2000）。始まりの幅が負なら、終わりの幅は書かれず、
/// 始まりと同じ絶対値とする。幅・膨らみ・接線の向きは 0 でなければ置き、接線の向きは度に直す
fn vertex2d(bits: &mut Bits) -> Option<Vec<Value>> {
    let mut values = vec![int(70, i64::from(bits.rc()?))];
    values.extend(point3(bits, 10)?);
    let start = bits.bd()?;
    let (start, end) = if start < 0.0 {
        (start.abs(), start.abs())
    } else {
        (start, bits.bd()?)
    };
    let bulge = bits.bd()?;
    // R2010 からは、頂点の番号（BL、DXF の 91）が続く。表には置かない
    if bits.release() >= Release::R2010 {
        bits.bl()?;
    }
    let tangent = bits.bd()?.to_degrees();
    for (code, value) in [(40, start), (41, end), (42, bulge), (50, tangent)] {
        if value != 0.0 {
            values.push(real(code, value));
        }
    }
    Some(values)
}

/// 3D・網・多面の頂点の VERTEX の欄（ODS 5.4.1 の 20.4.12〜20.4.14）。印（70）と点
fn vertex3d(bits: &mut Bits) -> Option<Vec<Value>> {
    let mut values = vec![int(70, i64::from(bits.rc()?))];
    values.extend(point3(bits, 10)?);
    Some(values)
}

/// 多面の面の VERTEX の欄（ODS 5.4.1 の 20.4.15）。DXF と同じく、点を 0、印（70）を 128 とする。
/// 頂点の番号（71〜74）は 0 でなければ置く。負の番号は、その辺が見えないこと
fn face_record(bits: &mut Bits) -> Option<Vec<Value>> {
    let mut values = vec![real(10, 0.0), real(20, 0.0), real(30, 0.0), int(70, 128)];
    for code in 71..75 {
        let index = bits.bs()?;
        if index != 0 {
            values.push(int(code, i64::from(index)));
        }
    }
    Some(values)
}

/// 2D の POLYLINE の欄（ODS 5.4.1 の 20.4.16、R2000）。POLYLINE には、DXF と同じく、頂点が続く印（66）を置く。DXF と同じく、標高を 30 に置き、10・20 は
/// 0 とする。曲線の種類（75）と既定の幅（40・41）は 0 でなければ置く
fn polyline2d(bits: &mut Bits) -> Option<Vec<Value>> {
    let mut values = vec![int(66, 1), int(70, i64::from(bits.bs()?))];
    let curve = bits.bs()?;
    if curve != 0 {
        values.push(int(75, i64::from(curve)));
    }
    for code in [40, 41] {
        let width = bits.bd()?;
        if width != 0.0 {
            values.push(real(code, width));
        }
    }
    // BT: 1 なら厚さは 0
    if !bits.bit()? {
        values.push(real(39, bits.bd()?));
    }
    values.extend([real(10, 0.0), real(20, 0.0), real(30, bits.bd()?)]);
    // BE: 1 なら押し出しの向きは (0, 0, 1)
    if !bits.bit()? {
        values.extend(point3(bits, 210)?);
    }
    Some(values)
}

/// 3D の POLYLINE の欄（ODS 5.4.1 の 20.4.17）。2 つの印から、DXF の 70（3D の 8、閉じていれば
/// 1、曲線なら 4）と曲線の種類（75）を作る。点（10）は 0
fn polyline3d(bits: &mut Bits) -> Option<Vec<Value>> {
    let curve = bits.rc()?;
    let closed = bits.rc()? & 1;
    let splined = if curve & 3 != 0 { 4 } else { 0 };
    let mut values = vec![
        int(66, 1),
        real(10, 0.0),
        real(20, 0.0),
        real(30, 0.0),
        int(70, i64::from(8 | splined | closed)),
    ];
    if curve & 1 != 0 {
        values.push(int(75, 5));
    } else if curve & 2 != 0 {
        values.push(int(75, 6));
    }
    Some(values)
}

/// 多面の POLYLINE の欄（ODS 5.4.1 の 20.4.33）。DXF と同じく、点（10）を 0、印（70）を 64 とし、
/// 頂点の数（71）と面の数（72）を置く
fn polyface(bits: &mut Bits) -> Option<Vec<Value>> {
    Some(vec![
        int(66, 1),
        real(10, 0.0),
        real(20, 0.0),
        real(30, 0.0),
        int(70, 64),
        int(71, i64::from(bits.bs()?)),
        int(72, i64::from(bits.bs()?)),
    ])
}

/// 網の POLYLINE の欄（ODS 5.4.1 の 20.4.34）。点（10）は 0。印（70）・曲面の種類（75）・頂点の数
/// （71・72）・密度（73・74）を置き、曲面の種類は 0 でなければ置く
fn mesh(bits: &mut Bits) -> Option<Vec<Value>> {
    let mut values = vec![
        int(66, 1),
        real(10, 0.0),
        real(20, 0.0),
        real(30, 0.0),
        int(70, i64::from(bits.bs()?)),
    ];
    let surface = bits.bs()?;
    if surface != 0 {
        values.push(int(75, i64::from(surface)));
    }
    for code in 71..75 {
        values.push(int(code, i64::from(bits.bs()?)));
    }
    Some(values)
}

/// DIMENSION の欄（ODS 5.4.1 の 20.4.22〜20.4.29、R2000）。DXF の 70 は、型で決まる寸法の種類に、
/// 共通の印の 1 が 0 なら 128、2 なら 32、座標寸法の印の 1 なら 64 を足す。文字の中点（11）と
/// 12 の点の Z は標高とし、12 の点は 0 でなければ置く。ブロックの挿入の尺度と回転は、DXF の 41・42 が
/// 別の欄に使われるので置かない
fn dimension(bits: &mut Bits, type_code: u16) -> Option<Vec<Value>> {
    // R2010 からは、寸法の版（RC、DXF の 280）が先にある
    let r2010 = bits.release() >= Release::R2010;
    let version = if r2010 { Some(bits.rc()?) } else { None };
    let extrusion = point3(bits, 210)?;
    let text_x = bits.rd()?;
    let text_y = bits.rd()?;
    let elevation = bits.bd()?;
    let flags = bits.rc()?;
    let user_text = bits.text()?;
    let text_rotation = bits.bd()?;
    let horizontal = bits.bd()?;
    for _ in 0..4 {
        bits.bd()?;
    }
    let attachment = bits.bs()?;
    let spacing_style = bits.bs()?;
    let spacing = bits.bd()?;
    let measurement = bits.bd()?;
    // R2010 からは、3 つの B（DXF の 73 と、矢印を裏返すかの 74・75）が続く
    let arrows = if r2010 {
        Some([bits.bit()?, bits.bit()?, bits.bit()?])
    } else {
        None
    };
    let point12 = (bits.rd()?, bits.rd()?);
    let (kind, mut values) = match type_code {
        DIMENSION_ORDINATE => {
            let mut values = point3(bits, 10)?;
            values.extend(point3(bits, 13)?);
            values.extend(point3(bits, 14)?);
            let kind = if bits.rc()? & 1 != 0 { 6 | 64 } else { 6 };
            (kind, values)
        }
        DIMENSION_LINEAR | DIMENSION_ALIGNED => {
            let mut values = point3(bits, 13)?;
            values.extend(point3(bits, 14)?);
            values.extend(point3(bits, 10)?);
            let oblique = bits.bd()?.to_degrees();
            if oblique != 0.0 {
                values.push(real(52, oblique));
            }
            if type_code == DIMENSION_LINEAR {
                let rotation = bits.bd()?.to_degrees();
                if rotation != 0.0 {
                    values.push(real(50, rotation));
                }
                (0, values)
            } else {
                (1, values)
            }
        }
        DIMENSION_ANGULAR_3PT => {
            let mut values = point3(bits, 10)?;
            for code in [13, 14, 15] {
                values.extend(point3(bits, code)?);
            }
            (5, values)
        }
        DIMENSION_ANGULAR_2LINE => {
            let mut values = point2(bits, 16)?;
            values.push(real(36, elevation));
            for code in [13, 14, 15, 10] {
                values.extend(point3(bits, code)?);
            }
            (2, values)
        }
        DIMENSION_RADIUS => {
            let mut values = point3(bits, 10)?;
            values.extend(point3(bits, 15)?);
            values.push(real(40, bits.bd()?));
            (4, values)
        }
        _ => {
            let mut values = point3(bits, 15)?;
            values.extend(point3(bits, 10)?);
            values.push(real(40, bits.bd()?));
            (3, values)
        }
    };
    let user_location = if flags & 1 == 0 { 128 } else { 0 };
    let block_only = if flags & 2 != 0 { 32 } else { 0 };
    values.extend([
        real(11, text_x),
        real(21, text_y),
        real(31, elevation),
        int(70, kind | user_location | block_only),
        int(71, i64::from(attachment)),
        real(42, measurement),
    ]);
    if let Some(version) = version {
        values.push(int(280, i64::from(version)));
    }
    if let Some(arrows) = arrows {
        for (code, flag) in [73, 74, 75].into_iter().zip(arrows) {
            values.push(int(code, i64::from(flag)));
        }
    }
    if point12 != (0.0, 0.0) {
        values.extend([
            real(12, point12.0),
            real(22, point12.1),
            real(32, elevation),
        ]);
    }
    if spacing_style != 1 {
        values.push(int(72, i64::from(spacing_style)));
    }
    if spacing != 1.0 {
        values.push(real(41, spacing));
    }
    if !user_text.is_empty() {
        values.push(text_value(1, &user_text));
    }
    for (code, angle) in [(53, text_rotation), (51, horizontal)] {
        if angle != 0.0 {
            values.push(real(code, angle.to_degrees()));
        }
    }
    if !is_up(&extrusion) {
        values.extend(extrusion);
    }
    Some(values)
}

/// HATCH の欄（ODS 5.4.1 の 20.4.75、R2000）。DXF と同じく、標高を 30 に置き、10・20 は 0 とする。
/// 角度は度に直す。境界の図形のハンドル（330）は、ハンドルの流れから読んで輪ごとの数（97）の後に置く
fn hatch(bits: &mut Bits) -> Option<Vec<Value>> {
    // R2004 からは、先頭にグラデーションの欄が来る。DXF と同じく、グラデーションのときだけ置く
    let gradient = if bits.release() >= Release::R2004 {
        hatch_gradient(bits)?
    } else {
        Vec::new()
    };
    let elevation = bits.bd()?;
    let mut values = vec![real(10, 0.0), real(20, 0.0), real(30, elevation)];
    let extrusion = point3(bits, 210)?;
    if !is_up(&extrusion) {
        values.extend(extrusion);
    }
    values.push(text_value(2, &bits.text()?));
    let solid = bits.bit()?;
    values.push(int(70, i64::from(solid)));
    values.push(int(71, i64::from(bits.bit()?)));
    let paths = bits.bl()?;
    values.push(int(91, i64::from(paths)));
    let mut pixel_size = false;
    for _ in 0..paths {
        let flags = bits.bl()?;
        values.push(int(92, i64::from(flags)));
        pixel_size |= flags & 4 != 0;
        if flags & 2 == 0 {
            let edges = bits.bl()?;
            values.push(int(93, i64::from(edges)));
            for _ in 0..edges {
                hatch_edge(bits, &mut values)?;
            }
        } else {
            // 多角形の境界。DXF は頂点の数を 93 に書く
            let bulges = bits.bit()?;
            values.push(int(72, i64::from(bulges)));
            values.push(int(73, i64::from(bits.bit()?)));
            let count = bits.bl()?;
            values.push(int(93, i64::from(count)));
            for _ in 0..count {
                values.extend(point2(bits, 10)?);
                if bulges {
                    values.push(real(42, bits.bd()?));
                }
            }
        }
        values.push(int(97, i64::from(bits.bl()?)));
    }
    values.push(int(75, i64::from(bits.bs()?)));
    values.push(int(76, i64::from(bits.bs()?)));
    if !solid {
        values.push(real(52, bits.bd()?.to_degrees()));
        values.push(real(41, bits.bd()?));
        values.push(int(77, i64::from(bits.bit()?)));
        let lines = bits.bs()?;
        values.push(int(78, i64::from(lines)));
        for _ in 0..lines {
            values.push(real(53, bits.bd()?.to_degrees()));
            values.push(real(43, bits.bd()?));
            values.push(real(44, bits.bd()?));
            values.push(real(45, bits.bd()?));
            values.push(real(46, bits.bd()?));
            let dashes = bits.bs()?;
            values.push(int(79, i64::from(dashes)));
            for _ in 0..dashes {
                values.push(real(49, bits.bd()?));
            }
        }
    }
    if pixel_size {
        values.push(real(47, bits.bd()?));
    }
    let seeds = bits.bl()?;
    values.push(int(98, i64::from(seeds)));
    for _ in 0..seeds {
        values.extend(point2(bits, 10)?);
    }
    values.extend(gradient);
    Some(values)
}

/// HATCH のグラデーションの欄（ODS 5.4.1 の 20.4.75、R2004 から）。グラデーションでなければ空
pub(super) fn hatch_gradient(bits: &mut Bits) -> Option<Vec<Value>> {
    let kind = bits.bl()?;
    let reserved = bits.bl()?;
    let angle = bits.bd()?;
    let shift = bits.bd()?;
    let single = bits.bl()?;
    let tint = bits.bd()?;
    let count = bits.bl()?;
    let mut colors = Vec::new();
    for _ in 0..count {
        colors.push(real(463, bits.bd()?));
        bits.bs()?;
        colors.push(int(421, i64::from(bits.bl()? as u32 & 0x00FF_FFFF)));
        bits.rc()?;
    }
    let name = bits.text()?;
    if kind == 0 {
        return Some(Vec::new());
    }
    let mut values = vec![
        int(450, i64::from(kind)),
        int(451, i64::from(reserved)),
        int(452, i64::from(single)),
        int(453, i64::from(count)),
    ];
    values.extend(colors);
    values.extend([
        real(460, angle),
        real(461, shift),
        real(462, tint),
        text_value(470, &name),
    ]);
    Some(values)
}

/// HATCH の境界の 1 つの辺。種類（72）が 1 なら線分、2 なら円弧、3 なら楕円弧、4 なら SPLINE
fn hatch_edge(bits: &mut Bits, values: &mut Vec<Value>) -> Option<()> {
    let kind = bits.rc()?;
    values.push(int(72, i64::from(kind)));
    match kind {
        1 => {
            values.extend(point2(bits, 10)?);
            values.extend(point2(bits, 11)?);
        }
        2 => {
            values.extend(point2(bits, 10)?);
            values.push(real(40, bits.bd()?));
            values.push(real(50, bits.bd()?.to_degrees()));
            values.push(real(51, bits.bd()?.to_degrees()));
            values.push(int(73, i64::from(bits.bit()?)));
        }
        3 => {
            values.extend(point2(bits, 10)?);
            values.extend(point2(bits, 11)?);
            values.push(real(40, bits.bd()?));
            values.push(real(50, bits.bd()?.to_degrees()));
            values.push(real(51, bits.bd()?.to_degrees()));
            values.push(int(73, i64::from(bits.bit()?)));
        }
        4 => {
            values.push(int(94, i64::from(bits.bl()?)));
            let rational = bits.bit()?;
            values.push(int(73, i64::from(rational)));
            values.push(int(74, i64::from(bits.bit()?)));
            let knots = bits.bl()?;
            let controls = bits.bl()?;
            values.push(int(95, i64::from(knots)));
            values.push(int(96, i64::from(controls)));
            for _ in 0..knots {
                values.push(real(40, bits.bd()?));
            }
            for _ in 0..controls {
                values.extend(point2(bits, 10)?);
                // DXF は重みを 42 に書く
                if rational {
                    values.push(real(42, bits.bd()?));
                }
            }
        }
        _ => return None,
    }
    Some(())
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

/// 制御文字を DXF の書き方（0x0A なら `^J`）に直す。`^` は、後に空白を付けて `^ ` とする
fn caret(text: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len());
    for &byte in text {
        match byte {
            0..0x20 => out.extend([b'^', byte + 0x40]),
            b'^' => out.extend(*b"^ "),
            _ => out.push(byte),
        }
    }
    out
}

fn real(code: i16, value: f64) -> Value {
    Value {
        code,
        data: Data::Real(value),
    }
}
