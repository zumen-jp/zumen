//! DWG のオブジェクトから、系統で共有するオブジェクトの表を作る（決定 0023）。
//!
//! どのオブジェクトにも、地図のハンドル（5。DIMSTYLE は 105）を置く。所有者（330）は、ハンドルの並びの
//! 先頭から読む。図形は、画層の名前（8）と、共通の欄のうち既定の値でないものを置く。図形ごとの欄は、
//! 今は TEXT・INSERT・LINE・ARC・CIRCLE・POINT・MTEXT・SOLID・TRACE・3DFACE・
//! ELLIPSE・RAY・XLINE・LWPOLYLINE を読む。表のレコードは、今は名前（2）だけを読む。
//! 図形が表のレコードをハンドルで指す欄（画層・線種・文字の体裁・ブロック）は、レコードの名前に直して置く。
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
const ARC: u16 = 0x11;
const CIRCLE: u16 = 0x12;
const LINE: u16 = 0x13;
const INSERT: u16 = 0x07;
const POINT: u16 = 0x1B;
const FACE3D: u16 = 0x1C;
const SOLID: u16 = 0x1F;
const TRACE: u16 = 0x20;
const ELLIPSE: u16 = 0x23;
const RAY: u16 = 0x28;
const XLINE: u16 = 0x29;
const MTEXT: u16 = 0x2C;
const LWPOLYLINE: u16 = 0x4D;
const BLOCK_RECORD: u16 = 0x31;
const LAYER: u16 = 0x33;
const STYLE: u16 = 0x35;
const LTYPE: u16 = 0x39;

/// DIMSTYLE の表のレコードの型。ハンドルのグループ・コードが 105
const DIMSTYLE: u16 = 0x45;

/// 型から、DXF の名前と属する所。クラスで決まる型（500 以上）と、知らない型は `None`
pub(super) fn kind(type_code: u16) -> Option<(&'static [u8], Section)> {
    let entity = |name: &'static [u8]| Some((name, Section::Entities));
    let table = |name: &'static [u8]| Some((name, Section::Tables));
    let object = |name: &'static [u8]| Some((name, Section::Objects));
    match type_code {
        TEXT => entity(b"TEXT"),
        0x02 => entity(b"ATTRIB"),
        0x03 => entity(b"ATTDEF"),
        0x04 => Some((b"BLOCK", Section::Blocks)),
        0x05 => Some((b"ENDBLK", Section::Blocks)),
        0x06 => entity(b"SEQEND"),
        INSERT | 0x08 => entity(b"INSERT"),
        0x0A..=0x0E => entity(b"VERTEX"),
        0x0F | 0x10 | 0x1D | 0x1E => entity(b"POLYLINE"),
        ARC => entity(b"ARC"),
        CIRCLE => entity(b"CIRCLE"),
        LINE => entity(b"LINE"),
        0x14..=0x1A => entity(b"DIMENSION"),
        POINT => entity(b"POINT"),
        FACE3D => entity(b"3DFACE"),
        SOLID => entity(b"SOLID"),
        TRACE => entity(b"TRACE"),
        0x21 => entity(b"SHAPE"),
        0x22 => entity(b"VIEWPORT"),
        ELLIPSE => entity(b"ELLIPSE"),
        0x24 => entity(b"SPLINE"),
        0x25 => entity(b"REGION"),
        0x26 => entity(b"3DSOLID"),
        0x27 => entity(b"BODY"),
        RAY => entity(b"RAY"),
        XLINE => entity(b"XLINE"),
        0x2A => object(b"DICTIONARY"),
        0x2B => entity(b"OLEFRAME"),
        MTEXT => entity(b"MTEXT"),
        0x2D => entity(b"LEADER"),
        0x2E => entity(b"TOLERANCE"),
        0x2F => entity(b"MLINE"),
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
        0x49 => object(b"MLINESTYLE"),
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
    /// オブジェクトの表を作る。予算の上限か中断の合図で止めたときだけ失敗する
    pub fn objects(&self, cx: &mut Cx) -> Result<AcadDb, Error> {
        let mut found = Vec::with_capacity(self.entries.len());
        for entry in &self.entries {
            cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
            let kind = kind(entry.type_code);
            let fields = kind.as_ref().and_then(|(_, section)| {
                // BLOCK と ENDBLK は、DXF では BLOCKS の節に属するが、DWG では図形として書かれる
                let entity = matches!(section, Section::Entities | Section::Blocks);
                read(self.data(entry), entry, entity)
            });
            found.push((kind, fields));
        }
        // 図形は表のレコードをハンドルで指すので、先にレコードの型と名前を集める
        let names: BTreeMap<u64, (u16, Vec<u8>)> = self
            .entries
            .iter()
            .zip(&found)
            .filter(|(entry, _)| is_table_record(entry.type_code))
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
fn read(data: &[u8], entry: &Entry, entity: bool) -> Option<Fields> {
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
        let record = match entry.type_code {
            TEXT | MTEXT => Some((7, STYLE)),
            INSERT => Some((2, BLOCK_RECORD)),
            _ => None,
        };
        if let Some((code, record)) = record {
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
    values.push(Value {
        code: 1,
        data: Data::Text(bits.text()?),
    });
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
    let text = bits.text()?;
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

fn text_value(code: i16, text: &[u8]) -> Value {
    Value {
        code,
        data: Data::Text(text.to_vec()),
    }
}

fn real(code: i16, value: f64) -> Value {
    Value {
        code,
        data: Data::Real(value),
    }
}
