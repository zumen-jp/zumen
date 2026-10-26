//! DWG のオブジェクトから、系統で共有するオブジェクトの表を作る（決定 0023）。
//!
//! どのオブジェクトにも、地図のハンドル（5。DIMSTYLE は 105）を置く。所有者（330）は、ハンドルの並びの
//! 先頭から読む。図形は、画層の名前（8）と、共通の欄のうち既定の値でないものを置く。図形ごとの欄は、
//! 今は TEXT・ATTRIB・ATTDEF・TOLERANCE・LEADER・MLINE・VIEWPORT・INSERT・LINE・ARC・CIRCLE・POINT・MTEXT・SOLID・TRACE・3DFACE・
//! ELLIPSE・RAY・XLINE・LWPOLYLINE・SPLINE・HATCH・POLYLINE・VERTEX・DIMENSION を読む。表のレコードは、今は名前（2）だけを読む。
//! オブジェクトは、今は MLINESTYLE の欄を読む。
//! 図形が表のレコードをハンドルで指す欄（画層・線種・文字の体裁・ブロック・寸法の体裁）は、レコードの名前に直して置く。
//! 型が 500 以上のオブジェクトは、番号が同じクラスで種類を決める。
//! 読めないオブジェクトは、ハンドルだけを持たせて置き、診断を返す。

use std::collections::BTreeMap;

use super::bits::Bits;
use super::{COMMON_OBJECT, DwgFile, Entry, stopped};
use crate::db::{AcadDb, Data, Object, Origin, Section, Value};
use zumen_base::{Cx, Diagnostic, Kind, Ref, SpecRef, codes};
use zumen_sdk::Error;

const COMMON_ENTITY: SpecRef = SpecRef {
    spec: "ods_dwg_5_4_1",
    section: "20.4.1 Common entity data",
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
const MLINE: u16 = 0x2F;
const VIEWPORT: u16 = 0x22;
const ARC: u16 = 0x11;
const CIRCLE: u16 = 0x12;
const LINE: u16 = 0x13;
const INSERT: u16 = 0x07;
const POINT: u16 = 0x1B;
const FACE3D: u16 = 0x1C;
const SOLID: u16 = 0x1F;
const TRACE: u16 = 0x20;
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
const DIMSTYLE: u16 = 0x45;
const MLINESTYLE: u16 = 0x49;

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
        INSERT | 0x08 => entity(b"INSERT"),
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
        0x21 => entity(b"SHAPE"),
        VIEWPORT => entity(b"VIEWPORT"),
        ELLIPSE => entity(b"ELLIPSE"),
        SPLINE => entity(b"SPLINE"),
        0x25 => entity(b"REGION"),
        0x26 => entity(b"3DSOLID"),
        0x27 => entity(b"BODY"),
        RAY => entity(b"RAY"),
        XLINE => entity(b"XLINE"),
        0x2A => object(b"DICTIONARY"),
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
        0x43 => table(b"APPID"),
        DIMSTYLE => table(b"DIMSTYLE"),
        // DXF にない表。名前は DWG のもの
        0x46 => table(b"VP_ENT_HDR_CONTROL"),
        0x47 => table(b"VP_ENT_HDR"),
        0x48 => object(b"GROUP"),
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
        let mut found = Vec::with_capacity(self.entries.len());
        for entry in &self.entries {
            cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
            let kind = self.kind(entry.type_code);
            let fields = kind.as_ref().and_then(|(name, section)| {
                // BLOCK と ENDBLK は、DXF では BLOCKS の節に属するが、DWG では図形として書かれる
                let entity = matches!(section, Section::Entities | Section::Blocks);
                read(self.data(entry), entry, name, entity)
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
        let mut db = AcadDb::default();
        for (entry, (kind, fields)) in self.entries.iter().zip(found) {
            let at = Ref::at_bytes(entry.span.start as u64, entry.span.end as u64);
            let handle_code = if entry.type_code == DIMSTYLE { 105 } else { 5 };
            let mut values = vec![Value {
                code: handle_code,
                data: Data::Handle(entry.handle),
            }];
            let (kind, section) = match kind {
                Some((kind, section)) => {
                    let entity = matches!(section, Section::Entities | Section::Blocks);
                    match fields {
                        Some(fields) => values.extend(fields.place(&names, entry, cx, &at)),
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
            db.push(Object {
                kind,
                section,
                values,
                origin: Origin::Dwg {
                    offset: entry.span.start,
                },
            });
        }
        Ok(db)
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
}

/// 表のレコードを指すハンドル。`code` の値として、`record` の型のレコードの名前を置く
struct RecordRef {
    code: i16,
    record: u16,
    handle: u64,
}

/// 名前（2）を持つ表のレコードの型か
fn is_table_record(type_code: u16) -> bool {
    matches!(
        type_code,
        BLOCK_RECORD | LAYER | STYLE | LTYPE | 0x3D | 0x3F | 0x41 | 0x43 | DIMSTYLE | 0x47
    )
}

impl Fields {
    /// ハンドル（5）より後の値を並べる。画層などは名前に直し、見つからなければ診断を返す
    fn place(
        self,
        names: &BTreeMap<u64, (u16, Vec<u8>)>,
        entry: &Entry,
        cx: &mut Cx,
        at: &Ref,
    ) -> Vec<Value> {
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
        values
    }
}

/// 図形の共通の欄のうち、型ごとの欄とハンドルの並びを読むのに要るもの
struct Common {
    /// 所有者のハンドルを持つか（entmode が 0）
    has_owner: bool,
    /// 反応子の数
    reactors: u32,
    /// 前後の図形のハンドルを持たないか
    no_links: bool,
    /// 線種の印。0 は ByLayer、1 は ByBlock、2 は Continuous、3 はハンドルで指す
    ltype_flags: u8,
    /// 印刷スタイルのハンドルを持つか（印刷スタイルの印が 3）
    plotstyle_handle: bool,
    values: Vec<Value>,
}

/// オブジェクトのデータを読む。読めなければ `None`
fn read(data: &[u8], entry: &Entry, kind: &[u8], entity: bool) -> Option<Fields> {
    let mut bits = Bits::new(data);
    bits.bs()?;
    let handles_at = usize::try_from(bits.rl()?).ok()?;
    bits.handle()?;
    skip_extended_data(&mut bits)?;
    if entity {
        let common = entity_data(&mut bits)?;
        let own = if bits.pos() <= handles_at {
            match entry.type_code {
                TEXT => text(&mut bits)?,
                ATTRIB | ATTDEF => attribute(&mut bits, entry.type_code == ATTDEF)?,
                TOLERANCE => tolerance(&mut bits)?,
                LEADER => leader(&mut bits)?,
                MLINE => mline(&mut bits)?,
                VIEWPORT => viewport(&mut bits)?,
                INSERT => insert(&mut bits)?,
                MTEXT => mtext(&mut bits)?,
                LINE => line(&mut bits)?,
                ARC => arc(&mut bits)?,
                CIRCLE => circle(&mut bits)?,
                POINT => point(&mut bits)?,
                SOLID | TRACE => solid(&mut bits)?,
                FACE3D => face3d(&mut bits)?,
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
                _ => Vec::new(),
            }
        } else {
            Vec::new()
        };
        // 図形のハンドルの並び: 所有者（entmode が 0 のとき）・反応子・拡張辞書・前後の図形
        // （nolinks が 0 のとき）・画層・線種と印刷スタイル（印が 3 のとき）・型ごとのハンドル
        bits.seek(handles_at);
        let owner = if common.has_owner {
            Some(bits.handle()?.resolve(entry.handle)?)
        } else {
            None
        };
        for _ in 0..common.reactors {
            bits.handle()?;
        }
        bits.handle()?;
        if !common.no_links {
            bits.handle()?;
            bits.handle()?;
        }
        let layer = bits.handle()?.resolve(entry.handle)?;
        let mut records = Vec::new();
        let mut values = common.values;
        values.extend(own);
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
        // 寸法は、寸法の体裁（3）とブロック（2）の順に指す
        let pointed: &[(i16, u16)] = match entry.type_code {
            TEXT | MTEXT | ATTRIB | ATTDEF => &[(7, STYLE)],
            INSERT => &[(2, BLOCK_RECORD)],
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
        })
    } else {
        bits.bl()?;
        let values = if is_table_record(entry.type_code) {
            vec![Value {
                code: 2,
                data: Data::Text(bits.text()?),
            }]
        } else if entry.type_code == MLINESTYLE {
            mlinestyle(&mut bits)?
        } else {
            Vec::new()
        };
        // オブジェクトのハンドルの並びの先頭は、いつも所有者
        bits.seek(handles_at);
        let owner = bits.handle()?.resolve(entry.handle)?;
        Some(Fields {
            owner: Some(owner),
            layer: None,
            values,
            records: Vec::new(),
        })
    }
}

/// 拡張データ（EED）を読み飛ばす（ODS 5.4.1 の 28）
fn skip_extended_data(bits: &mut Bits) -> Option<()> {
    loop {
        let size = bits.bs()?;
        if size == 0 {
            return Some(());
        }
        bits.handle()?;
        bits.skip_bytes(usize::try_from(size).ok()?)?;
    }
}

/// R2000 の図形の共通の欄（ODS 5.4.1 の 20.4.1）。既定の値でない色（62）・線種の尺度（48）・
/// 非表示（60）を値にする
fn entity_data(bits: &mut Bits) -> Option<Common> {
    if bits.bit()? {
        let size = bits.rl()?;
        bits.skip_bytes(usize::try_from(size).ok()?)?;
    }
    let entmode = bits.bb()?;
    let reactors = u32::try_from(bits.bl()?).ok()?;
    let no_links = bits.bit()?;
    let color = bits.bs()?;
    let ltype_scale = bits.bd()?;
    let ltype_flags = bits.bb()?;
    let plotstyle_flags = bits.bb()?;
    let invisible = bits.bs()?;
    bits.rc()?;
    let mut values = Vec::new();
    if color != 256 {
        values.push(int(62, i64::from(color)));
    }
    if ltype_scale != 1.0 {
        values.push(real(48, ltype_scale));
    }
    if invisible != 0 {
        values.push(int(60, i64::from(invisible)));
    }
    Some(Common {
        has_owner: entmode == 0,
        reactors,
        no_links,
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
    values.push(text_value(2, &bits.text()?));
    let length = bits.bs()?;
    if length != 0 {
        values.push(int(73, i64::from(length)));
    }
    values.push(int(70, i64::from(bits.rc()?)));
    if definition {
        values.push(text_value(3, &bits.text()?));
    }
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
/// 向き（74）を置き、押し出しの向き（210）と X 軸の向き（211）は既定の値でなければ置く。DXF は寸法の
/// 体裁から鉤の線の点を足して書くので、点（10）とその数（76）はまだ置かない
fn leader(bits: &mut Bits) -> Option<Vec<Value>> {
    bits.bit()?;
    let annotation = bits.bs()?;
    let path = bits.bs()?;
    let count = bits.bl()?;
    for _ in 0..count {
        point3(bits, 10)?;
    }
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
    values.push(real(40, bits.bd()?));
    values.push(real(41, bits.bd()?));
    values.push(int(74, i64::from(bits.bit()?)));
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
/// 番号（69）と状態（68）は DWG に記録されていないので置かない
fn viewport(bits: &mut Bits) -> Option<Vec<Value>> {
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
    bits.bl()?;
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
        int(62, i64::from(bits.bs()?)),
        real(51, bits.bd()?.to_degrees()),
        real(52, bits.bd()?.to_degrees()),
    ];
    let lines = bits.rc()?;
    values.push(int(71, i64::from(lines)));
    for _ in 0..lines {
        values.push(real(49, bits.bd()?));
        values.push(int(62, i64::from(bits.bs()?)));
        bits.bs()?;
    }
    Some(values)
}

/// INSERT の欄（ODS 5.4.1 の 20.4.9、R2000）。挿入点はいつも置き、尺度（41・42・43）・回転（50）・
/// 押し出しの向き（210・220・230）は既定の値でなければ、属性が続くこと（66）は続くときだけ置く
fn insert(bits: &mut Bits) -> Option<Vec<Value>> {
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
    values.push(real(40, bits.bd()?));
    values.push(int(71, i64::from(bits.bs()?)));
    values.push(int(72, i64::from(bits.bs()?)));
    bits.bd()?;
    bits.bd()?;
    let text = caret(&bits.text()?);
    let mut rest = text.as_slice();
    while rest.len() > 250 {
        let (chunk, tail) = rest.split_at(250);
        values.push(Value {
            code: 3,
            data: Data::Text(chunk.to_vec()),
        });
        rest = tail;
    }
    values.push(Value {
        code: 1,
        data: Data::Text(rest.to_vec()),
    });
    values.push(int(73, i64::from(bits.bs()?)));
    values.push(real(44, bits.bd()?));
    bits.bit()?;
    if reals(&x_axis) != [Some(1.0), Some(0.0), Some(0.0)] {
        values.extend(x_axis);
    }
    if !is_up(&extrusion) {
        values.extend(extrusion);
    }
    Some(values)
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
    let mut values = vec![int(71, i64::from(bits.bl()?))];
    let mut fit = 0;
    let mut knots = 0;
    let mut controls = 0;
    let mut weighted = false;
    let mut tail = Vec::new();
    match scenario {
        2 => {
            let tolerance = bits.bd()?;
            let start = point3(bits, 12)?;
            let end = point3(bits, 13)?;
            fit = bits.bl()?;
            tail.push(real(44, tolerance));
            for tangent in [start, end] {
                if reals(&tangent).iter().any(|r| *r != Some(0.0)) {
                    tail.extend(tangent);
                }
            }
        }
        1 => {
            bits.bit()?; // 有理
            bits.bit()?; // 閉じている
            bits.bit()?; // 周期的
            tail.push(real(42, bits.bd()?));
            tail.push(real(43, bits.bd()?));
            knots = bits.bl()?;
            controls = bits.bl()?;
            weighted = bits.bit()?;
        }
        _ => return None,
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
/// 角度は度に直す。境界の図形のハンドル（330）は、所有者と見分けられないので、まだ置かない
fn hatch(bits: &mut Bits) -> Option<Vec<Value>> {
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
