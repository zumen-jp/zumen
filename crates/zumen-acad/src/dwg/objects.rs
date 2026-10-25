//! DWG のオブジェクトから、系統で共有するオブジェクトの表を作る（決定 0023）。
//!
//! どのオブジェクトにも、地図のハンドル（5。DIMSTYLE は 105）を置く。所有者（330）は、ハンドルの並びの
//! 先頭から読む。図形は、共通の欄のうち既定の値でないものを置く。図形ごとの欄は、今は LINE だけを読む。
//! 読めないオブジェクトは、ハンドルだけを持たせて置き、診断を返す。

use super::bits::Bits;
use super::{COMMON_OBJECT, DwgFile, Entry, stopped};
use crate::db::{AcadDb, Data, Object, Origin, Section, Value};
use zumen_base::{Cx, Diagnostic, Kind, Ref, SpecRef, codes};
use zumen_sdk::Error;

const COMMON_ENTITY: SpecRef = SpecRef {
    spec: "ods_dwg_5_4_1",
    section: "20.4.1 Common entity data",
};

/// LINE の型
const LINE: u16 = 0x13;

/// DIMSTYLE の表のレコードの型。ハンドルのグループ・コードが 105
const DIMSTYLE: u16 = 0x45;

/// 型から、DXF の名前と属する所。クラスで決まる型（500 以上）と、知らない型は `None`
pub(super) fn kind(type_code: u16) -> Option<(&'static [u8], Section)> {
    let entity = |name: &'static [u8]| Some((name, Section::Entities));
    let table = |name: &'static [u8]| Some((name, Section::Tables));
    let object = |name: &'static [u8]| Some((name, Section::Objects));
    match type_code {
        0x01 => entity(b"TEXT"),
        0x02 => entity(b"ATTRIB"),
        0x03 => entity(b"ATTDEF"),
        0x04 => Some((b"BLOCK", Section::Blocks)),
        0x05 => Some((b"ENDBLK", Section::Blocks)),
        0x06 => entity(b"SEQEND"),
        0x07 | 0x08 => entity(b"INSERT"),
        0x0A..=0x0E => entity(b"VERTEX"),
        0x0F | 0x10 | 0x1D | 0x1E => entity(b"POLYLINE"),
        0x11 => entity(b"ARC"),
        0x12 => entity(b"CIRCLE"),
        LINE => entity(b"LINE"),
        0x14..=0x1A => entity(b"DIMENSION"),
        0x1B => entity(b"POINT"),
        0x1C => entity(b"3DFACE"),
        0x1F => entity(b"SOLID"),
        0x20 => entity(b"TRACE"),
        0x21 => entity(b"SHAPE"),
        0x22 => entity(b"VIEWPORT"),
        0x23 => entity(b"ELLIPSE"),
        0x24 => entity(b"SPLINE"),
        0x25 => entity(b"REGION"),
        0x26 => entity(b"3DSOLID"),
        0x27 => entity(b"BODY"),
        0x28 => entity(b"RAY"),
        0x29 => entity(b"XLINE"),
        0x2A => object(b"DICTIONARY"),
        0x2B => entity(b"OLEFRAME"),
        0x2C => entity(b"MTEXT"),
        0x2D => entity(b"LEADER"),
        0x2E => entity(b"TOLERANCE"),
        0x2F => entity(b"MLINE"),
        0x30 | 0x32 | 0x34 | 0x38 | 0x3C | 0x3E | 0x40 | 0x42 | 0x44 => table(b"TABLE"),
        0x31 => table(b"BLOCK_RECORD"),
        0x33 => table(b"LAYER"),
        0x35 => table(b"STYLE"),
        0x39 => table(b"LTYPE"),
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
        0x4D => entity(b"LWPOLYLINE"),
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
        let mut db = AcadDb::default();
        for entry in &self.entries {
            cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
            let at = Ref::at_bytes(entry.span.start as u64, entry.span.end as u64);
            let handle_code = if entry.type_code == DIMSTYLE { 105 } else { 5 };
            let mut values = vec![Value {
                code: handle_code,
                data: Data::Handle(entry.handle),
            }];
            let (kind, section) = match kind(entry.type_code) {
                Some((kind, section)) => {
                    let entity = section == Section::Entities;
                    match read(self.data(entry), entry, entity) {
                        Some(read) => values.extend(read),
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

/// 図形の共通の欄のうち、型ごとの欄とハンドルの並びを読むのに要るもの
struct Common {
    /// 所有者のハンドルを持つか（entmode が 0）
    has_owner: bool,
    values: Vec<Value>,
}

/// オブジェクトのデータを読み、ハンドル（5）より後の値を返す。読めなければ `None`
fn read(data: &[u8], entry: &Entry, entity: bool) -> Option<Vec<Value>> {
    let mut bits = Bits::new(data);
    bits.bs()?;
    let handles_at = usize::try_from(bits.rl()?).ok()?;
    bits.handle()?;
    skip_extended_data(&mut bits)?;
    let common = if entity {
        let common = entity_data(&mut bits)?;
        if entry.type_code == LINE && bits.pos() <= handles_at {
            Some((common, line(&mut bits)?))
        } else {
            Some((common, Vec::new()))
        }
    } else {
        bits.bl()?;
        None
    };
    // ハンドルの並びの先頭は、図形なら entmode が 0 のときだけ、オブジェクトならいつも所有者
    bits.seek(handles_at);
    let mut values = Vec::new();
    let has_owner = common.as_ref().is_none_or(|(c, _)| c.has_owner);
    if has_owner {
        let owner = bits.handle()?;
        values.push(Value {
            code: 330,
            data: Data::Handle(owner.resolve(entry.handle)?),
        });
    }
    if let Some((common, own_values)) = common {
        values.extend(common.values);
        values.extend(own_values);
    }
    Some(values)
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
    bits.bl()?;
    bits.bit()?;
    let color = bits.bs()?;
    let ltype_scale = bits.bd()?;
    bits.bb()?;
    bits.bb()?;
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
        values,
    })
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
    // BT: 1 なら厚さは 0
    if !bits.bit()? {
        values.push(real(39, bits.bd()?));
    }
    // BE: 1 なら押し出しの向きは (0, 0, 1)
    if !bits.bit()? {
        let (x, y, z) = (bits.bd()?, bits.bd()?, bits.bd()?);
        values.extend([real(210, x), real(220, y), real(230, z)]);
    }
    Some(values)
}

fn int(code: i16, value: i64) -> Value {
    Value {
        code,
        data: Data::Int(value),
    }
}

fn real(code: i16, value: f64) -> Value {
    Value {
        code,
        data: Data::Real(value),
    }
}
