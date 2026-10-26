//! 共有のオブジェクトの表の変更を、DWG のオブジェクトに書き戻す（決定 0029）。R2000 だけを扱う。
//!
//! 表と、今のファイルから作り直した表を、元のオブジェクト（[`Origin::Dwg`]）ごとに比べる。値の並びが違う
//! オブジェクトだけを符号化し直し、1 つの [`DwgTxn`](super::DwgTxn) で置き換える（決定 0028）。共通の欄と、
//! いくつかの種類の型ごとの欄（決定 0030〜0043）を符号化する。型ごとの欄を書き直して
//! ハンドルの流れの位置（RL）を直し、その前と、ハンドルの流れから後ろのビットは元のまま写す。
//!
//! オブジェクトの追加と削除と並べ替え、ほかの値と見出しの変数の変更は、まだ書き戻せない。

use std::collections::{BTreeMap, BTreeSet};

use zumen_base::{Cx, Diagnostic, Diagnostics, Severity, codes};
use zumen_sdk::{Error, Transact, Transaction};

use super::bits::{BitWriter, Bits};
use super::objects::{entity_fields, object_fields};
use super::{DwgFile, Release};
use crate::db::{AcadDb, Data, Object, Origin, Section, Value};

/// 図形の共通の欄のうち、表に置くもののグループ・コード
const COMMON: &[i16] = &[62, 48, 60];

/// 型ごとの欄のグループ・コード
const LINE: &[i16] = &[10, 20, 30, 11, 21, 31, 39, 210, 220, 230];
const CIRCLE: &[i16] = &[10, 20, 30, 40, 39, 210, 220, 230];
const ARC: &[i16] = &[10, 20, 30, 40, 39, 210, 220, 230, 50, 51];
const POINT: &[i16] = &[10, 20, 30, 39, 210, 220, 230, 50];
const ELLIPSE: &[i16] = &[10, 20, 30, 11, 21, 31, 210, 220, 230, 40, 41, 42];
const RAY: &[i16] = &[10, 20, 30, 11, 21, 31];
const SOLID: &[i16] = &[
    10, 20, 30, 11, 21, 31, 12, 22, 32, 13, 23, 33, 39, 210, 220, 230,
];
const FACE3D: &[i16] = &[10, 20, 30, 11, 21, 31, 12, 22, 32, 13, 23, 33, 70];
const TEXT: &[i16] = &[
    10, 20, 30, 11, 21, 31, 39, 51, 50, 40, 41, 1, 71, 72, 73, 210, 220, 230,
];
/// ATTRIB は、TEXT の欄の縦の合わせを 74 に置き、タグ（2）・欄の長さ（73）・印（70）を足す。ATTDEF は
/// 問い（3）も足す
const ATTRIB: &[i16] = &[
    10, 20, 30, 11, 21, 31, 39, 51, 50, 40, 41, 1, 71, 72, 74, 210, 220, 230, 2, 73, 70,
];
const ATTDEF: &[i16] = &[
    10, 20, 30, 11, 21, 31, 39, 51, 50, 40, 41, 1, 71, 72, 74, 210, 220, 230, 2, 73, 70, 3,
];
const MTEXT: &[i16] = &[
    10, 20, 30, 210, 220, 230, 11, 21, 31, 41, 40, 71, 72, 3, 1, 73, 44,
];

/// MINSERT の型の番号
const LWPOLYLINE: &[i16] = &[90, 70, 43, 38, 39, 10, 20, 40, 41, 42, 210, 220, 230];
const SPLINE: &[i16] = &[
    71, 72, 73, 74, 44, 12, 22, 32, 13, 23, 33, 42, 43, 40, 10, 20, 30, 41, 11, 21, 31,
];
const HATCH: &[i16] = &[
    10, 20, 30, 210, 220, 230, 2, 70, 71, 91, 92, 93, 72, 73, 74, 11, 21, 40, 42, 50, 51, 94, 95,
    96, 97, 75, 76, 52, 41, 77, 78, 53, 43, 44, 45, 46, 79, 49, 47, 98,
];
const MINSERT_TYPE: i16 = 0x08;
const VERTEX_2D: &[i16] = &[70, 10, 20, 30, 40, 41, 42, 50];
const VERTEX_3D: &[i16] = &[70, 10, 20, 30];
const FACE_RECORD: &[i16] = &[10, 20, 30, 70, 71, 72, 73, 74];
const POLYLINE_2D: &[i16] = &[66, 70, 75, 40, 41, 39, 10, 20, 30, 210, 220, 230];
const POLYLINE_3D: &[i16] = &[66, 10, 20, 30, 70, 75];
const POLYFACE: &[i16] = &[66, 10, 20, 30, 70, 71, 72];
const MESH: &[i16] = &[66, 10, 20, 30, 70, 75, 71, 72, 73, 74];
/// DIMENSION のどの型にもある欄
const DIMENSION: &[i16] = &[
    11, 21, 31, 70, 71, 42, 12, 22, 32, 72, 41, 1, 53, 51, 210, 220, 230,
];
/// DIMENSION の型ごとの点など。DIMENSION の型の順（座標・長さ・平行・3 点の角度・2 線の角度・半径・直径）
const DIMENSION_KINDS: [&[i16]; 7] = [
    &[10, 20, 30, 13, 23, 33, 14, 24, 34],
    &[13, 23, 33, 14, 24, 34, 10, 20, 30, 52, 50],
    &[13, 23, 33, 14, 24, 34, 10, 20, 30, 52],
    &[10, 20, 30, 13, 23, 33, 14, 24, 34, 15, 25, 35],
    &[16, 26, 36, 13, 23, 33, 14, 24, 34, 15, 25, 35, 10, 20, 30],
    &[10, 20, 30, 15, 25, 35, 40],
    &[15, 25, 35, 10, 20, 30, 40],
];
/// DIMENSION の、どれかの型の欄
const DIMENSION_ALL: &[i16] = &[
    11, 21, 31, 70, 71, 42, 12, 22, 32, 72, 41, 1, 53, 51, 210, 220, 230, 10, 20, 30, 13, 23, 33,
    14, 24, 34, 15, 25, 35, 16, 26, 36, 40, 50, 52,
];
const DIMENSION_TYPE: i16 = 0x14;
/// DIMENSION の型ごとの、DXF の寸法の種類（70 の下の 3 ビット）
const DIMENSION_KIND: [i16; 7] = [6, 0, 1, 5, 2, 4, 3];

const INSERT: &[i16] = &[10, 20, 30, 41, 42, 43, 50, 210, 220, 230];
const MINSERT: &[i16] = &[10, 20, 30, 41, 42, 43, 50, 210, 220, 230, 70, 71, 44, 45];
const TOLERANCE: &[i16] = &[10, 20, 30, 11, 21, 31, 210, 220, 230, 1];
const LEADER: &[i16] = &[73, 72, 40, 41, 74, 210, 220, 230, 211, 221, 231];
const LAYER: &[i16] = &[70, 62, 290, 370];
const STYLE: &[i16] = &[70, 40, 41, 50, 71, 42, 3, 4];
const SHAPE: &[i16] = &[10, 20, 30, 40, 50, 41, 51, 39, 210, 220, 230];
const VIEWPORT: &[i16] = &[
    10, 20, 30, 40, 41, 12, 22, 13, 23, 14, 24, 15, 25, 16, 26, 36, 17, 27, 37, 42, 43, 44, 45, 50,
    51, 72, 90, 1, 281, 71, 74, 110, 120, 130, 111, 121, 131, 112, 122, 132, 146, 79,
];
const MLINE: &[i16] = &[
    40, 70, 71, 72, 73, 10, 20, 30, 210, 220, 230, 11, 21, 31, 12, 22, 32, 13, 23, 33, 74, 41, 75,
    42,
];

impl DwgFile {
    /// 表 `db` のオブジェクトの値の変更を、このファイルに書き戻す。`db` は、このファイルから作った表を
    /// 変えたもの。変えていないオブジェクトは、元のバイトのまま残る。1 つでも書き戻せない変更があれば、
    /// 何も反映しない
    pub fn write_back(&mut self, db: &AcadDb, cx: &mut Cx) -> Result<(), Error> {
        // 作り直す表の読みの診断は、表を作ったときに出したので捨てる。止めたときの診断だけを残す
        let mut quiet = Diagnostics::new();
        let base = self.objects(&mut Cx::new(cx.budget, &mut quiet, cx.cancel));
        let Ok(base) = base else {
            for diagnostic in quiet
                .iter()
                .filter(|d| d.code().severity() == Severity::Error)
            {
                cx.report(diagnostic.clone());
            }
            return base.map(|_| ());
        };
        let changes = self.changes(db, &base);
        let mut txn = self.transaction();
        let staged = match changes {
            Ok(changes) => changes
                .iter()
                .try_for_each(|(record, data)| txn.set_data(*record, data)),
            Err(diagnostic) => Err(txn.reject(*diagnostic)),
        };
        // 失敗した変更があれば、確定が最初の失敗を診断として返し、何も反映しない
        let committed = txn.commit(cx);
        staged.and(committed)
    }

    /// 変えたオブジェクトの番号と、新しいデータ
    fn changes(
        &self,
        db: &AcadDb,
        base: &AcadDb,
    ) -> Result<Vec<(usize, Vec<u8>)>, Box<Diagnostic>> {
        if db != base
            && let Some(diagnostic) = self.edit_unsupported()
        {
            return Err(Box::new(diagnostic));
        }
        let records: BTreeMap<usize, usize> = self
            .entries
            .iter()
            .enumerate()
            .map(|(record, entry)| (entry.span.start, record))
            .collect();
        let before: BTreeMap<usize, (usize, &Object)> = base
            .objects()
            .iter()
            .filter_map(|o| match o.origin {
                Origin::Dwg { offset, .. } => Some((offset, (*records.get(&offset)?, o))),
                Origin::Dxf { .. } | Origin::New => None,
            })
            .collect();
        let mut kept = BTreeSet::new();
        let mut changes = Vec::new();
        for object in db.objects() {
            let found = match object.origin {
                Origin::Dwg { offset, .. } => before.get(&offset).filter(|(_, old)| {
                    old.kind == object.kind && old.section == object.section && kept.insert(offset)
                }),
                Origin::Dxf { .. } | Origin::New => None,
            };
            let Some(&(record, old)) = found else {
                return Err(unsupported(&object.kind));
            };
            if old.values != object.values {
                let data = self
                    .entries
                    .get(record)
                    .and_then(|entry| {
                        let data = self.data(entry);
                        // BLOCK と ENDBLK も、DWG では図形として書かれる
                        match self.kind(entry.type_code)?.1 {
                            Section::Entities | Section::Blocks => encode(data, old, object),
                            Section::Tables => encode_record(data, old, object),
                            _ => None,
                        }
                    })
                    .ok_or_else(|| unsupported(&object.kind))?;
                changes.push((record, data));
            }
        }
        // 表から除いたか、並べ替えたオブジェクト
        if let Some((_, (_, old))) = before.iter().find(|(offset, _)| !kept.contains(*offset)) {
            return Err(unsupported(&old.kind));
        }
        if !db
            .objects()
            .iter()
            .map(offset)
            .eq(base.objects().iter().map(offset))
        {
            return Err(unsupported(b"ENTITIES"));
        }
        let names: BTreeSet<&[u8]> = base
            .variables()
            .chain(db.variables())
            .map(|(n, _)| n)
            .collect();
        if let Some(name) = names
            .into_iter()
            .find(|n| base.variable(n) != db.variable(n))
        {
            return Err(unsupported(name));
        }
        Ok(changes)
    }
}

fn offset(object: &Object) -> Option<usize> {
    match object.origin {
        Origin::Dwg { offset, .. } => Some(offset),
        Origin::Dxf { .. } | Origin::New => None,
    }
}

fn unsupported(kind: &[u8]) -> Box<Diagnostic> {
    let kind = String::from_utf8_lossy(kind).into_owned();
    Box::new(Diagnostic::new(codes::DWG_CHANGE_UNSUPPORTED).with("kind", kind))
}

/// `old` から `new` に値を変えた図形のデータ `data` を、符号化し直す。共通の欄と型ごとの欄のほかの値が
/// 変わったか、符号化できない値なら `None`
fn encode(data: &[u8], old: &Object, new: &Object) -> Option<Vec<u8>> {
    // MINSERT も、表では INSERT とする。VERTEX と POLYLINE も、型で欄が分かれる
    let type_code = Bits::with_release(data, Release::R2000).bs()?;
    let grid = type_code == MINSERT_TYPE;
    let own: &[i16] = match new.kind.as_slice() {
        b"LINE" => LINE,
        b"CIRCLE" => CIRCLE,
        b"ARC" => ARC,
        b"POINT" => POINT,
        b"ELLIPSE" => ELLIPSE,
        b"RAY" | b"XLINE" => RAY,
        b"SOLID" | b"TRACE" => SOLID,
        b"3DFACE" => FACE3D,
        b"TEXT" => TEXT,
        b"ATTRIB" => ATTRIB,
        b"ATTDEF" => ATTDEF,
        b"MTEXT" => MTEXT,
        b"LWPOLYLINE" => LWPOLYLINE,
        b"SPLINE" => SPLINE,
        b"HATCH" => HATCH,
        b"VERTEX" => match type_code {
            0x0A => VERTEX_2D,
            0x0B..=0x0D => VERTEX_3D,
            0x0E => FACE_RECORD,
            _ => &[],
        },
        b"POLYLINE" => match type_code {
            0x0F => POLYLINE_2D,
            0x10 => POLYLINE_3D,
            0x1D => POLYFACE,
            0x1E => MESH,
            _ => &[],
        },
        b"DIMENSION" => DIMENSION_ALL,
        b"INSERT" if grid => MINSERT,
        b"INSERT" => INSERT,
        b"TOLERANCE" => TOLERANCE,
        b"LEADER" => LEADER,
        b"MLINE" => MLINE,
        b"VIEWPORT" => VIEWPORT,
        b"SHAPE" => SHAPE,
        _ => &[],
    };
    let only = |codes: &[i16], object: &Object, keep: bool| -> Vec<_> {
        object
            .values
            .iter()
            .filter(|value| codes.contains(&value.code) == keep)
            .cloned()
            .collect()
    };
    let rest = |object| only(&[COMMON, own].concat(), object, false);
    if rest(old) != rest(new) {
        return None;
    }
    let [size_at, color_at, start, handles_at] = entity_fields(data)?;
    let common = if only(COMMON, old, true) == only(COMMON, new, true) {
        let mut common = BitWriter::default();
        common.copy(data, color_at..start)?;
        common
    } else {
        common_fields(data, color_at, start, new)?
    };
    let fields = if only(own, old, true) == only(own, new, true) {
        let mut fields = BitWriter::default();
        fields.copy(data, start..handles_at)?;
        fields
    } else {
        own_fields(old, new, type_code, data, start)?
    };
    let mut out = BitWriter::default();
    out.copy(data, 0..size_at)?;
    out.rl(i32::try_from(color_at + common.len() + fields.len()).ok()?);
    out.copy(data, size_at + 32..color_at)?;
    for part in [common, fields] {
        let len = part.len();
        out.copy(&part.into_bytes(), 0..len)?;
    }
    // ハンドルの流れの後ろの、バイトの終わりまでの埋めのビットも写す
    out.copy(data, handles_at..data.len() * 8)?;
    Some(out.into_bytes())
}

/// 表のレコードの型ごとの欄を符号化し直したデータ（決定 0043）。LAYER と STYLE だけを扱う。名前（2）などの
/// ほかの値が変わったものは `None`
fn encode_record(data: &[u8], old: &Object, new: &Object) -> Option<Vec<u8>> {
    let own: &[i16] = match new.kind.as_slice() {
        b"LAYER" => LAYER,
        b"STYLE" => STYLE,
        _ => return None,
    };
    let rest = |object: &Object| -> Vec<_> {
        object
            .values
            .iter()
            .filter(|value| !own.contains(&value.code))
            .cloned()
            .collect()
    };
    if rest(old) != rest(new) {
        return None;
    }
    let [size_at, start, handles_at] = object_fields(data)?;
    // 名前・参照される印・外部参照の番号・外部参照に依る印
    let mut bits = Bits::with_release(data, Release::R2000);
    bits.seek(start);
    bits.text()?;
    bits.bit()?;
    bits.bs()?;
    let xdep = bits.bit()?;
    let own_at = bits.pos();
    let mut fields = BitWriter::default();
    fields.copy(data, start..own_at)?;
    match new.kind.as_slice() {
        b"LAYER" => layer(&mut fields, new, xdep, data, own_at)?,
        _ => style(&mut fields, old, new, xdep, data, own_at)?,
    }
    fields.copy(
        data,
        own_at + fields_end(&new.kind, data, own_at)?..handles_at,
    )?;
    let mut out = BitWriter::default();
    out.copy(data, 0..size_at)?;
    out.rl(i32::try_from(start + fields.len()).ok()?);
    out.copy(data, size_at + 32..start)?;
    let len = fields.len();
    out.copy(&fields.into_bytes(), 0..len)?;
    out.copy(data, handles_at..data.len() * 8)?;
    Some(out.into_bytes())
}

/// `own_at` から始まる LAYER・STYLE の欄の長さ（ビット）
fn fields_end(kind: &[u8], data: &[u8], own_at: usize) -> Option<usize> {
    let mut bits = Bits::with_release(data, Release::R2000);
    bits.seek(own_at);
    if kind == b"LAYER" {
        bits.bs()?;
        bits.bs()?;
    } else {
        bits.bit()?;
        bits.bit()?;
        for _ in 0..3 {
            bits.bd()?;
        }
        bits.rc()?;
        bits.bd()?;
        bits.text()?;
        bits.text()?;
    }
    Some(bits.pos() - own_at)
}

/// LAYER の欄（決定 0043）。DXF の 70 の凍結 1・新しいビューポートで凍結 2・ロック 4 と、負の色（62）の
/// 表示しない印、印刷しない印（290）、線の太さ（370）から、DWG の印（BS）を作る。印のほかのビットは元のまま
/// 写す。外部参照に依る印（70 の 16）は、元のものと同じでなければ受け付けない
fn layer(
    fields: &mut BitWriter,
    object: &Object,
    xdep: bool,
    data: &[u8],
    own_at: usize,
) -> Option<()> {
    let mut bits = Bits::with_release(data, Release::R2000);
    bits.seek(own_at);
    let old = bits.bs()?;
    let dxf = small(object, 70)??;
    (dxf & !(1 | 2 | 4 | 16) == 0 && (dxf & 16 != 0) == xdep).then_some(())?;
    let color = small(object, 62)??;
    let mut flags = old & !0x03FF;
    for (dxf_bit, dwg_bit) in [(1, 1), (2, 4), (4, 8)] {
        if dxf & dxf_bit != 0 {
            flags |= dwg_bit;
        }
    }
    if color < 0 {
        flags |= 2;
    }
    if new_plot(object)? {
        flags |= 16;
    }
    flags |= lineweight_index(integer(object, 370)??)? << 5;
    fields.bs(flags);
    fields.bs(color.checked_abs()?);
    Some(())
}

/// 印刷する画層か。290 がなければ印刷し、偽なら印刷しない
fn new_plot(object: &Object) -> Option<bool> {
    let mut found = object.values.iter().filter(|value| value.code == 290);
    match (found.next().map(|value| &value.data), found.next()) {
        (None, _) => Some(true),
        (Some(Data::Bool(plot)), None) => Some(*plot),
        _ => None,
    }
}

/// DXF の線の太さ（370）の、DWG の番号
fn lineweight_index(width: i64) -> Option<i16> {
    const WIDTHS: [i64; 24] = [
        0, 5, 9, 13, 15, 18, 20, 25, 30, 35, 40, 50, 53, 60, 70, 80, 90, 100, 106, 120, 140, 158,
        200, 211,
    ];
    match width {
        -1 => Some(29),
        -2 => Some(30),
        -3 => Some(31),
        _ => WIDTHS
            .iter()
            .position(|&w| w == width)
            .and_then(|index| i16::try_from(index).ok()),
    }
}

/// STYLE の欄（決定 0043）。70 の形のファイル 1 と縦書き 4 を印に書き、角度（50）は度からラジアンに直す。
/// 外部参照に依る印（70 の 16）は、元のものと同じでなければ受け付けない。文字列（3・4）が変わらなければ、
/// 元のビットを写す
fn style(
    fields: &mut BitWriter,
    old: &Object,
    new: &Object,
    xdep: bool,
    data: &[u8],
    own_at: usize,
) -> Option<()> {
    let mut bits = Bits::with_release(data, Release::R2000);
    bits.seek(own_at);
    bits.bit()?;
    bits.bit()?;
    for _ in 0..3 {
        bits.bd()?;
    }
    bits.rc()?;
    bits.bd()?;
    let font_at = bits.pos();
    bits.text()?;
    let big_font_at = bits.pos();
    bits.text()?;
    let texts = [(3, font_at..big_font_at), (4, big_font_at..bits.pos())];
    let flags = small(new, 70)??;
    (flags & !(1 | 4 | 16) == 0 && (flags & 16 != 0) == xdep).then_some(())?;
    fields.bit(flags & 1 != 0);
    fields.bit(flags & 4 != 0);
    fields.bd(real(new, 40)??);
    fields.bd(real(new, 41)??);
    fields.bd(real(new, 50)??.to_radians());
    fields.rc(u8::try_from(small(new, 71)??).ok()?);
    fields.bd(real(new, 42)??);
    for (code, range) in texts {
        if text_of(old, code) == text_of(new, code) {
            fields.copy(data, range)?;
        } else {
            string(fields, new, code)?;
        }
    }
    Some(())
}

/// 共通の欄の、色から型ごとの欄の前まで（R2000）。色（62）がなければ ByLayer、線種の尺度（48）がなければ 1、
/// 非表示（60）がなければ 0 を書く。線種と印刷スタイルの印と、線の太さは元のまま写す
fn common_fields(data: &[u8], color_at: usize, start: usize, object: &Object) -> Option<BitWriter> {
    let mut bits = Bits::with_release(data, Release::R2000);
    bits.seek(color_at);
    bits.bs()?;
    bits.bd()?;
    let flags_at = bits.pos();
    bits.seek(flags_at + 4);
    bits.bs()?;
    let lineweight_at = bits.pos();
    let mut fields = BitWriter::default();
    fields.bs(small(object, 62)?.unwrap_or(256));
    fields.bd(real(object, 48)?.unwrap_or(1.0));
    fields.copy(data, flags_at..flags_at + 4)?;
    fields.bs(small(object, 60)?.unwrap_or(0));
    fields.copy(data, lineweight_at..start)?;
    Some(fields)
}

/// 型ごとの欄。`grid` は MINSERT か。表にない欄は、元のデータ `data` の `start` からの型ごとの欄から写す。
/// 符号化できない種類なら `None`
fn own_fields(
    old: &Object,
    new: &Object,
    type_code: i16,
    data: &[u8],
    start: usize,
) -> Option<BitWriter> {
    let mut fields = BitWriter::default();
    match new.kind.as_slice() {
        b"LINE" => line(&mut fields, new)?,
        b"CIRCLE" | b"ARC" => {
            for code in [10, 20, 30, 40] {
                fields.bd(real(new, code)??);
            }
            thickness_and_extrusion(&mut fields, new)?;
            if new.kind == b"ARC" {
                // DXF は度、DWG はラジアンで書く
                for code in [50, 51] {
                    fields.bd(real(new, code)??.to_radians());
                }
            }
        }
        b"POINT" => {
            point3(&mut fields, new, 10)?;
            thickness_and_extrusion(&mut fields, new)?;
            // X 軸の角度は、なければ 0
            fields.bd(real(new, 50)?.unwrap_or(0.0).to_radians());
        }
        b"ELLIPSE" => {
            point3(&mut fields, new, 10)?;
            point3(&mut fields, new, 11)?;
            // 押し出しの向きは、なければ (0, 0, 1)
            match [210, 220, 230].map(|code| real(new, code)) {
                [Some(None), Some(None), Some(None)] => [0.0, 0.0, 1.0].map(|v| fields.bd(v)),
                [Some(Some(x)), Some(Some(y)), Some(Some(z))] => [x, y, z].map(|v| fields.bd(v)),
                _ => return None,
            };
            for code in [40, 41, 42] {
                fields.bd(real(new, code)??);
            }
        }
        b"RAY" | b"XLINE" => {
            point3(&mut fields, new, 10)?;
            point3(&mut fields, new, 11)?;
        }
        b"SOLID" | b"TRACE" => solid(&mut fields, new)?,
        b"3DFACE" => face3d(&mut fields, new)?,
        b"TEXT" => text(&mut fields, new, 73)?,
        b"ATTRIB" | b"ATTDEF" => attribute(&mut fields, new)?,
        b"MTEXT" => mtext(&mut fields, new, data, start)?,
        b"LWPOLYLINE" => lwpolyline(&mut fields, new, data, start)?,
        b"SPLINE" => spline(&mut fields, new, data, start)?,
        b"HATCH" => hatch(&mut fields, old, new)?,
        b"VERTEX" => vertex(&mut fields, new, type_code)?,
        b"POLYLINE" => polyline(&mut fields, new, type_code, data, start)?,
        b"DIMENSION" => dimension(&mut fields, old, new, type_code, data, start)?,
        b"INSERT" => insert(&mut fields, new, type_code == MINSERT_TYPE)?,
        b"TOLERANCE" => tolerance(&mut fields, old, new, data, start)?,
        b"LEADER" => leader(&mut fields, new, data, start)?,
        b"MLINE" => mline(&mut fields, new)?,
        b"VIEWPORT" => viewport(&mut fields, old, new, data, start)?,
        b"SHAPE" => shape(&mut fields, new, data, start)?,
        _ => return None,
    }
    Some(fields)
}

/// LINE の欄。両方の Z が 0 なら、Z を書かない
fn line(fields: &mut BitWriter, object: &Object) -> Option<()> {
    let [x0, y0, z0, x1, y1, z1] = [10, 20, 30, 11, 21, 31].map(|code| real(object, code));
    let (x0, y0, z0, x1, y1, z1) = (x0??, y0??, z0??, x1??, y1??, z1??);
    let flat = z0.to_bits() == 0 && z1.to_bits() == 0;
    fields.bit(flat);
    fields.rd(x0);
    fields.dd(x1, x0);
    fields.rd(y0);
    fields.dd(y1, y0);
    if !flat {
        fields.rd(z0);
        fields.dd(z1, z0);
    }
    thickness_and_extrusion(fields, object)
}

/// SOLID・TRACE の欄。4 つの角の Z（30〜33）は、どれも同じ標高でなければならない
fn solid(fields: &mut BitWriter, object: &Object) -> Option<()> {
    let elevation = real(object, 30)??;
    let mut corners = Vec::new();
    for code in 10..14 {
        let z = real(object, code + 20)??;
        if z.to_bits() != elevation.to_bits() {
            return None;
        }
        corners.extend([real(object, code)??, real(object, code + 10)??]);
    }
    thickness(fields, object)?;
    fields.bd(elevation);
    for value in corners {
        fields.rd(value);
    }
    extrusion(fields, object)
}

/// 3DFACE の欄。見えない辺の印（70）がなければ、印を書かない
fn face3d(fields: &mut BitWriter, object: &Object) -> Option<()> {
    let mut corners = [[0.0; 3]; 4];
    for (corner, code) in corners.iter_mut().zip(10..) {
        *corner = [
            real(object, code)??,
            real(object, code + 10)??,
            real(object, code + 20)??,
        ];
    }
    let invisible = small(object, 70)?;
    let [first, ..] = corners;
    let z_is_zero = first[2].to_bits() == 0;
    fields.bit(invisible.is_none());
    fields.bit(z_is_zero);
    fields.rd(first[0]);
    fields.rd(first[1]);
    if !z_is_zero {
        fields.rd(first[2]);
    }
    for (previous, corner) in corners.iter().zip(&corners[1..]) {
        for (value, default) in corner.iter().zip(previous) {
            fields.dd(*value, *default);
        }
    }
    if let Some(flags) = invisible {
        fields.bs(flags);
    }
    Some(())
}

/// TEXT の欄（R2000）。初めの印（RC）は、標高・合わせの点・斜めの角度・回転・幅の比・合わせの向きの、
/// ない欄を表す。標高（30）が 0 なら書かない。合わせの点の Z（31）は、標高と同じでなければならない。
/// `vertical` は縦の合わせのグループ・コード
fn text(fields: &mut BitWriter, object: &Object, vertical: i16) -> Option<()> {
    let [x, y, elevation, height] = [10, 20, 30, 40].map(|code| real(object, code));
    let (x, y, elevation, height) = (x??, y??, elevation??, height??);
    let alignment = match [11, 21, 31].map(|code| real(object, code)) {
        [Some(None), Some(None), Some(None)] => None,
        [Some(Some(x)), Some(Some(y)), Some(Some(z))] if z.to_bits() == elevation.to_bits() => {
            Some((x, y))
        }
        _ => return None,
    };
    let [oblique, rotation, width] = [51, 50, 41].map(|code| real(object, code));
    let (oblique, rotation, width) = (oblique?, rotation?, width?);
    let [generation, horizontal, vertical] = [71, 72, vertical].map(|code| small(object, code));
    let justification = [generation?, horizontal?, vertical?];
    let mut flags = 0;
    for (absent, bit) in [
        (elevation.to_bits() == 0, 0x01),
        (alignment.is_none(), 0x02),
        (oblique.is_none(), 0x04),
        (rotation.is_none(), 0x08),
        (width.is_none(), 0x10),
        (justification[0].is_none(), 0x20),
        (justification[1].is_none(), 0x40),
        (justification[2].is_none(), 0x80),
    ] {
        if absent {
            flags |= bit;
        }
    }
    fields.rc(flags);
    if elevation.to_bits() != 0 {
        fields.rd(elevation);
    }
    fields.rd(x);
    fields.rd(y);
    if let Some((ax, ay)) = alignment {
        fields.dd(ax, x);
        fields.dd(ay, y);
    }
    // TEXT は、押し出しの向きが厚さの前にある
    extrusion(fields, object)?;
    thickness(fields, object)?;
    // DXF は度、DWG はラジアンで書く
    for angle in [oblique, rotation].into_iter().flatten() {
        fields.rd(angle.to_radians());
    }
    fields.rd(height);
    if let Some(width) = width {
        fields.rd(width);
    }
    string(fields, object, 1)?;
    for value in justification.into_iter().flatten() {
        fields.bs(value);
    }
    Some(())
}

/// ATTRIB・ATTDEF の欄（R2000）。TEXT の欄に続けて、タグ・欄の長さ（なければ 0）・印を書き、ATTDEF は
/// 問いも書く
fn attribute(fields: &mut BitWriter, object: &Object) -> Option<()> {
    text(fields, object, 74)?;
    string(fields, object, 2)?;
    fields.bs(small(object, 73)?.unwrap_or(0));
    fields.rc(u8::try_from(small(object, 70)??).ok()?);
    if object.kind == b"ATTDEF" {
        string(fields, object, 3)?;
    }
    Some(())
}

/// MTEXT の欄（R2000）。押し出しの向きと X 軸の向きは、なければ (0, 0, 1)・(1, 0, 0) を書く。文字列は、
/// 250 バイトずつの 3 と、残りの 1 をつないで書く。表にない文字の範囲の大きさ（2 つの BD）と、行の間隔の後の
/// B は、元のデータから写す
fn mtext(fields: &mut BitWriter, object: &Object, data: &[u8], start: usize) -> Option<()> {
    let mut bits = Bits::with_release(data, Release::R2000);
    bits.seek(start);
    for _ in 0..11 {
        bits.bd()?;
    }
    bits.bs()?;
    bits.bs()?;
    let extents_at = bits.pos();
    bits.bd()?;
    bits.bd()?;
    let extents_end = bits.pos();
    bits.text()?;
    bits.bs()?;
    bits.bd()?;
    let flag_at = bits.pos();
    point3(fields, object, 10)?;
    for (code, default) in [(210, [0.0, 0.0, 1.0]), (11, [1.0, 0.0, 0.0])] {
        match [code, code + 10, code + 20].map(|code| real(object, code)) {
            [Some(None), Some(None), Some(None)] => default.map(|v| fields.bd(v)),
            [Some(Some(x)), Some(Some(y)), Some(Some(z))] => [x, y, z].map(|v| fields.bd(v)),
            _ => return None,
        };
    }
    for code in [41, 40] {
        fields.bd(real(object, code)??);
    }
    for code in [71, 72] {
        fields.bs(small(object, code)??);
    }
    fields.copy(data, extents_at..extents_end)?;
    let mut text = Vec::new();
    for value in object.values.iter().filter(|value| value.code == 3) {
        let Data::Text(chunk) = &value.data else {
            return None;
        };
        text.extend(uncaret(chunk)?);
    }
    text.extend(uncaret(text_of(object, 1)?)?);
    text.push(0);
    fields.text(&text)?;
    fields.bs(small(object, 73)??);
    fields.bd(real(object, 44)??);
    fields.copy(data, flag_at..flag_at + 1)
}

/// LWPOLYLINE の欄（R2000）。続く欄があるかの印は、値があるかから選ぶ。膨らみ（42）が 1 つでもあれば、
/// すべての頂点の膨らみを書く（ないものは 0）。頂点ごとの幅（40・41）は、初めの頂点から続けて置く。
/// 頂点ごとの幅があるときは、表にない一定の幅を元のまま残す。ほかの印のビットも元のまま写す
fn lwpolyline(fields: &mut BitWriter, object: &Object, data: &[u8], start: usize) -> Option<()> {
    let mut bits = Bits::with_release(data, Release::R2000);
    bits.seek(start);
    let old = bits.bs()?;
    let old_width = if old & 4 != 0 { bits.bd()? } else { 0.0 };
    // 頂点ごとの x・y・始まりの幅・終わりの幅・膨らみ
    let mut vertices: Vec<[Option<f64>; 5]> = Vec::new();
    for value in &object.values {
        let slot = match value.code {
            10 => {
                vertices.push([None; 5]);
                0
            }
            20 => 1,
            40 => 2,
            41 => 3,
            42 => 4,
            _ => continue,
        };
        let Data::Real(v) = value.data else {
            return None;
        };
        let slot = vertices.last_mut()?.get_mut(slot)?;
        if slot.is_some() {
            return None;
        }
        *slot = Some(v);
    }
    let count = object.values.iter().filter(|value| value.code == 90);
    match count.map(|value| &value.data).collect::<Vec<_>>()[..] {
        [Data::Int(count)] if usize::try_from(*count).ok()? == vertices.len() => {}
        _ => return None,
    }
    let closed = match small(object, 70)? {
        None | Some(0) => false,
        Some(1) => true,
        _ => return None,
    };
    let widths = vertices.iter().take_while(|v| v[2].is_some()).count();
    let (with, without) = vertices.split_at_checked(widths)?;
    if without.iter().any(|v| v[2].is_some() || v[3].is_some())
        || with.iter().any(|v| v[3].is_none())
    {
        return None;
    }
    let width = match (real(object, 43)?, widths) {
        (Some(_), 1..) => return None,
        (width, 0) => width.unwrap_or(0.0),
        (None, _) => old_width,
    };
    let elevation = real(object, 38)?;
    let thickness = real(object, 39)?;
    let extrusion = match [210, 220, 230].map(|code| real(object, code)) {
        [Some(None), Some(None), Some(None)] => None,
        [Some(Some(x)), Some(Some(y)), Some(Some(z))] => Some([x, y, z]),
        _ => return None,
    };
    let bulges = vertices.iter().any(|v| v[4].is_some());
    let mut flags = old & !(0x3F | 0x200);
    for (on, bit) in [
        (extrusion.is_some(), 1),
        (thickness.is_some(), 2),
        (width.to_bits() != 0, 4),
        (elevation.is_some(), 8),
        (bulges, 16),
        (widths > 0, 32),
        (closed, 0x200),
    ] {
        if on {
            flags |= bit;
        }
    }
    fields.bs(flags);
    if flags & 4 != 0 {
        fields.bd(width);
    }
    for value in [elevation, thickness].into_iter().flatten() {
        fields.bd(value);
    }
    for value in extrusion.into_iter().flatten() {
        fields.bd(value);
    }
    let count = i32::try_from(vertices.len()).ok()?;
    fields.bl(count);
    if bulges {
        fields.bl(count);
    }
    if widths > 0 {
        fields.bl(i32::try_from(widths).ok()?);
    }
    let mut last: Option<(f64, f64)> = None;
    for vertex in &vertices {
        let (x, y) = (vertex[0]?, vertex[1]?);
        match last {
            None => {
                fields.rd(x);
                fields.rd(y);
            }
            Some((last_x, last_y)) => {
                fields.dd(x, last_x);
                fields.dd(y, last_y);
            }
        }
        last = Some((x, y));
    }
    if bulges {
        for vertex in &vertices {
            fields.bd(vertex[4].unwrap_or(0.0));
        }
    }
    for vertex in with {
        fields.bd(vertex[2]?);
        fields.bd(vertex[3]?);
    }
    Some(())
}

/// SPLINE の欄（R2000）。場合（scenario）は元のままとし、1 なら節点と制御点、2 なら当てはめる点を書く。
/// 場合に合わない値があれば `None`。有理・閉じている・周期的の印は表にないので元のビットを写し、重みが
/// あるかの印は 41 があるかから選ぶ。接線（12・13）がなければ 0 を書く
fn spline(fields: &mut BitWriter, object: &Object, data: &[u8], start: usize) -> Option<()> {
    let mut bits = Bits::with_release(data, Release::R2000);
    bits.seek(start);
    let scenario = bits.bl()?;
    bits.bl()?;
    let flags_at = bits.pos();
    // 制御点（x・y・z・重み）と、当てはめる点（x・y・z）
    let mut controls: Vec<[Option<f64>; 4]> = Vec::new();
    let mut fit: Vec<[Option<f64>; 4]> = Vec::new();
    let mut knots = Vec::new();
    for value in &object.values {
        let (list, slot) = match value.code {
            10 => {
                controls.push([None; 4]);
                (&mut controls, 0)
            }
            20 => (&mut controls, 1),
            30 => (&mut controls, 2),
            41 => (&mut controls, 3),
            11 => {
                fit.push([None; 4]);
                (&mut fit, 0)
            }
            21 => (&mut fit, 1),
            31 => (&mut fit, 2),
            40 => {
                let Data::Real(knot) = value.data else {
                    return None;
                };
                knots.push(knot);
                continue;
            }
            _ => continue,
        };
        let Data::Real(v) = value.data else {
            return None;
        };
        let slot = list.last_mut()?.get_mut(slot)?;
        if slot.is_some() {
            return None;
        }
        *slot = Some(v);
    }
    for (code, len) in [(72, knots.len()), (73, controls.len()), (74, fit.len())] {
        if integer(object, code)?.unwrap_or(0) != i64::try_from(len).ok()? {
            return None;
        }
    }
    let present = |codes: &[i16]| {
        codes
            .iter()
            .any(|code| object.values.iter().any(|v| v.code == *code))
    };
    fields.bl(scenario);
    fields.bl(i32::try_from(integer(object, 71)??).ok()?);
    match scenario {
        1 => {
            if !fit.is_empty() || present(&[44, 12, 22, 32, 13, 23, 33]) {
                return None;
            }
            let weighted = controls.first().is_some_and(|c| c[3].is_some());
            if controls.iter().any(|c| c[3].is_some() != weighted) {
                return None;
            }
            fields.copy(data, flags_at..flags_at + 3)?;
            for code in [42, 43] {
                fields.bd(real(object, code)??);
            }
            fields.bl(i32::try_from(knots.len()).ok()?);
            fields.bl(i32::try_from(controls.len()).ok()?);
            fields.bit(weighted);
            for knot in knots {
                fields.bd(knot);
            }
            for control in &controls {
                for v in &control[..3] {
                    fields.bd((*v)?);
                }
                if weighted {
                    fields.bd(control[3]?);
                }
            }
        }
        2 => {
            if !controls.is_empty() || !knots.is_empty() || present(&[42, 43]) {
                return None;
            }
            fields.bd(real(object, 44)??);
            for code in [12, 13] {
                match [code, code + 10, code + 20].map(|code| real(object, code)) {
                    [Some(None), Some(None), Some(None)] => [0.0; 3].map(|v| fields.bd(v)),
                    [Some(Some(x)), Some(Some(y)), Some(Some(z))] => {
                        [x, y, z].map(|v| fields.bd(v))
                    }
                    _ => return None,
                };
            }
            fields.bl(i32::try_from(fit.len()).ok()?);
            for point in &fit {
                for v in &point[..3] {
                    fields.bd((*v)?);
                }
            }
        }
        _ => return None,
    }
    Some(())
}

/// HATCH の欄（R2000）。境界の辺では同じグループ・コードが何度も出るので、表の値を読む側が置く順に
/// 先頭から読み進めて書く。順が合わないか、境界の図形の数（97）が変わったら `None`。10・20 は 0 で
/// なければならず、押し出しの向きがなければ (0, 0, 1) を書く。角度は度からラジアンに直す
fn hatch(fields: &mut BitWriter, old: &Object, new: &Object) -> Option<()> {
    let sources = |object: &Object| -> Vec<_> {
        object
            .values
            .iter()
            .filter(|value| value.code == 97)
            .cloned()
            .collect()
    };
    if sources(old) != sources(new) {
        return None;
    }
    let mut values = Cursor(
        new.values
            .iter()
            .filter(|value| HATCH.contains(&value.code))
            .collect::<Vec<_>>()
            .into_iter()
            .peekable(),
    );
    if values.real(10)?.to_bits() != 0 || values.real(20)?.to_bits() != 0 {
        return None;
    }
    fields.bd(values.real(30)?);
    let extrusion = if values.next_is(210) {
        [values.real(210)?, values.real(220)?, values.real(230)?]
    } else {
        [0.0, 0.0, 1.0]
    };
    for v in extrusion {
        fields.bd(v);
    }
    let Data::Text(name) = values.next(2)? else {
        return None;
    };
    let mut name = uncaret(name)?;
    name.push(0);
    fields.text(&name)?;
    let solid = values.bit(70)?;
    fields.bit(solid);
    fields.bit(values.bit(71)?);
    let paths = values.int(91)?;
    fields.bl(paths);
    let mut pixel_size = false;
    for _ in 0..paths {
        let flags = values.int(92)?;
        fields.bl(flags);
        pixel_size |= flags & 4 != 0;
        if flags & 2 == 0 {
            let edges = values.int(93)?;
            fields.bl(edges);
            for _ in 0..edges {
                hatch_edge(fields, &mut values)?;
            }
        } else {
            let bulges = values.bit(72)?;
            fields.bit(bulges);
            fields.bit(values.bit(73)?);
            let count = values.int(93)?;
            fields.bl(count);
            for _ in 0..count {
                fields.rd(values.real(10)?);
                fields.rd(values.real(20)?);
                if bulges {
                    fields.bd(values.real(42)?);
                }
            }
        }
        fields.bl(values.int(97)?);
    }
    for code in [75, 76] {
        fields.bs(i16::try_from(values.int(code)?).ok()?);
    }
    if !solid {
        fields.bd(values.real(52)?.to_radians());
        fields.bd(values.real(41)?);
        fields.bit(values.bit(77)?);
        let lines = i16::try_from(values.int(78)?).ok()?;
        fields.bs(lines);
        for _ in 0..lines {
            fields.bd(values.real(53)?.to_radians());
            for code in [43, 44, 45, 46] {
                fields.bd(values.real(code)?);
            }
            let dashes = i16::try_from(values.int(79)?).ok()?;
            fields.bs(dashes);
            for _ in 0..dashes {
                fields.bd(values.real(49)?);
            }
        }
    }
    if pixel_size {
        fields.bd(values.real(47)?);
    }
    let seeds = values.int(98)?;
    fields.bl(seeds);
    for _ in 0..seeds {
        fields.rd(values.real(10)?);
        fields.rd(values.real(20)?);
    }
    values.0.next().is_none().then_some(())
}

/// HATCH の境界の 1 つの辺。種類（72）が 1 なら線分、2 なら円弧、3 なら楕円弧、4 なら SPLINE
fn hatch_edge(fields: &mut BitWriter, values: &mut Cursor<'_>) -> Option<()> {
    let kind = u8::try_from(values.int(72)?).ok()?;
    fields.rc(kind);
    let point = |fields: &mut BitWriter, values: &mut Cursor<'_>, code| -> Option<()> {
        fields.rd(values.real(code)?);
        fields.rd(values.real(code + 10)?);
        Some(())
    };
    match kind {
        1 => {
            point(fields, values, 10)?;
            point(fields, values, 11)?;
        }
        2 | 3 => {
            point(fields, values, 10)?;
            if kind == 3 {
                point(fields, values, 11)?;
            }
            fields.bd(values.real(40)?);
            for code in [50, 51] {
                fields.bd(values.real(code)?.to_radians());
            }
            fields.bit(values.bit(73)?);
        }
        4 => {
            fields.bl(values.int(94)?);
            let rational = values.bit(73)?;
            fields.bit(rational);
            fields.bit(values.bit(74)?);
            let knots = values.int(95)?;
            let controls = values.int(96)?;
            fields.bl(knots);
            fields.bl(controls);
            for _ in 0..knots {
                fields.bd(values.real(40)?);
            }
            for _ in 0..controls {
                point(fields, values, 10)?;
                if rational {
                    fields.bd(values.real(42)?);
                }
            }
        }
        _ => return None,
    }
    Some(())
}

/// 表の値を、先頭から 1 つずつ読む
struct Cursor<'a>(std::iter::Peekable<std::vec::IntoIter<&'a Value>>);

impl<'a> Cursor<'a> {
    /// 次の値が `code` なら、その値
    fn next(&mut self, code: i16) -> Option<&'a Data> {
        self.0
            .next_if(|value| value.code == code)
            .map(|value| &value.data)
    }

    fn next_is(&mut self, code: i16) -> bool {
        self.0.peek().is_some_and(|value| value.code == code)
    }

    fn real(&mut self, code: i16) -> Option<f64> {
        match self.next(code)? {
            Data::Real(value) => Some(*value),
            _ => None,
        }
    }

    /// BL に収まる整数
    fn int(&mut self, code: i16) -> Option<i32> {
        match self.next(code)? {
            Data::Int(value) => i32::try_from(*value).ok(),
            _ => None,
        }
    }

    /// 0 か 1 の整数
    fn bit(&mut self, code: i16) -> Option<bool> {
        match self.int(code)? {
            0 => Some(false),
            1 => Some(true),
            _ => None,
        }
    }
}

/// `code` の文字列を、DXF の `^` の書き方を戻し、終わりの NUL を付けて T で書く
fn string(fields: &mut BitWriter, object: &Object, code: i16) -> Option<()> {
    let mut bytes = uncaret(text_of(object, code)?)?;
    bytes.push(0);
    fields.text(&bytes)
}

/// VERTEX の欄（決定 0038）。2D の頂点は、始まりと終わりの幅が同じで 0 でなければ、負の始まりの幅だけを
/// 書く。角度は度からラジアンに直す。多面の面は、点が 0 で印が 128 のものだけを受け付ける
fn vertex(fields: &mut BitWriter, object: &Object, type_code: i16) -> Option<()> {
    let flags = |object| small(object, 70).map(|flags| flags.unwrap_or(0));
    match type_code {
        0x0A..=0x0D => {
            fields.rc(u8::try_from(flags(object)?).ok()?);
            point3(fields, object, 10)?;
            if type_code != 0x0A {
                return Some(());
            }
            let [start, end, bulge, tangent] =
                [40, 41, 42, 50].map(|code| real(object, code).map(|v| v.unwrap_or(0.0)));
            let (start, end) = (start?, end?);
            if start.to_bits() == end.to_bits() && start > 0.0 {
                fields.bd(-start);
            } else if start < 0.0 || end < 0.0 {
                return None;
            } else {
                fields.bd(start);
                fields.bd(end);
            }
            fields.bd(bulge?);
            fields.bd(tangent?.to_radians());
        }
        0x0E => {
            zero_point(object)?;
            (flags(object)? == 128).then_some(())?;
            for code in 71..75 {
                fields.bs(small(object, code)?.unwrap_or(0));
            }
        }
        _ => return None,
    }
    Some(())
}

/// POLYLINE の欄（決定 0038）。頂点が続く印（66）は 1、点（10・20）は 0 だけを受け付ける。3D の
/// POLYLINE の 2 つの印は、70・75 から読む値が変わらなければ元のバイトを写し、変われば読まれる
/// ビットだけを書き換える
fn polyline(
    fields: &mut BitWriter,
    object: &Object,
    type_code: i16,
    data: &[u8],
    start: usize,
) -> Option<()> {
    (small(object, 66)? == Some(1)).then_some(())?;
    let flags = small(object, 70)?;
    let surface = small(object, 75)?.unwrap_or(0);
    match type_code {
        0x0F => {
            for code in [10, 20] {
                (real(object, code)? == Some(0.0)).then_some(())?;
            }
            fields.bs(flags.unwrap_or(0));
            fields.bs(surface);
            for code in [40, 41] {
                fields.bd(real(object, code)?.unwrap_or(0.0));
            }
            thickness(fields, object)?;
            fields.bd(real(object, 30)??);
            extrusion(fields, object)?;
        }
        0x10 => {
            zero_point(object)?;
            let flags = flags?;
            let curve = match surface {
                0 => 0,
                5 => 1,
                6 => 2,
                _ => return None,
            };
            // 8 は 3D、4 は曲線の印。読む側が作るのは 8・4・1 だけ
            let splined = if curve != 0 { 4 } else { 0 };
            (flags & !1 == 8 | splined).then_some(())?;
            let mut bits = Bits::with_release(data, Release::R2000);
            bits.seek(start);
            let (old_curve, old_closed) = (bits.rc()?, bits.rc()?);
            let read = |curve: u8| match curve {
                c if c & 1 != 0 => 1,
                c if c & 2 != 0 => 2,
                _ => 0,
            };
            fields.rc(if read(old_curve) == curve {
                old_curve
            } else {
                old_curve & !3 | curve
            });
            fields.rc(old_closed & !1 | u8::try_from(flags & 1).ok()?);
        }
        0x1D => {
            zero_point(object)?;
            (flags == Some(64)).then_some(())?;
            for code in [71, 72] {
                fields.bs(small(object, code)??);
            }
        }
        0x1E => {
            zero_point(object)?;
            fields.bs(flags?);
            fields.bs(surface);
            for code in 71..75 {
                fields.bs(small(object, code)??);
            }
        }
        _ => return None,
    }
    Some(())
}

/// DIMENSION の欄（決定 0039）。表にないブロックの挿入の尺度と回転は、元のビットを写す。印は、70 から
/// 読まれるビットだけを書き換える。利用者の文字（1）が変わらなければ、元のビットを写す。角度は度から
/// ラジアンに直す。標高（31）と、12 の点の Z・2 線の角度の 36 は同じ値だけを受け付ける
fn dimension(
    fields: &mut BitWriter,
    old: &Object,
    new: &Object,
    type_code: i16,
    data: &[u8],
    start: usize,
) -> Option<()> {
    let at = usize::try_from(type_code.checked_sub(DIMENSION_TYPE)?).ok()?;
    let (kind, own) = (*DIMENSION_KIND.get(at)?, *DIMENSION_KINDS.get(at)?);
    // ほかの型の欄があるものは受け付けない
    let allowed = |code| DIMENSION.contains(&code) || own.contains(&code);
    if new
        .values
        .iter()
        .any(|value| DIMENSION_ALL.contains(&value.code) && !allowed(value.code))
    {
        return None;
    }
    let mut bits = Bits::with_release(data, Release::R2000);
    bits.seek(start);
    for _ in 0..3 {
        bits.bd()?;
    }
    bits.rd()?;
    bits.rd()?;
    bits.bd()?;
    let old_flags = bits.rc()?;
    let text_at = bits.pos();
    bits.text()?;
    let text_end = bits.pos();
    bits.bd()?;
    bits.bd()?;
    let scale_at = bits.pos();
    for _ in 0..4 {
        bits.bd()?;
    }
    let scale_end = bits.pos();
    bits.bs()?;
    bits.bs()?;
    bits.bd()?;
    bits.bd()?;
    bits.rd()?;
    bits.rd()?;
    let old_ordinate = if type_code == DIMENSION_TYPE {
        for _ in 0..9 {
            bits.bd()?;
        }
        bits.rc()?
    } else {
        0
    };
    let flags = small(new, 70)??;
    let ordinate = if type_code == DIMENSION_TYPE { 64 } else { 0 };
    (flags & !(32 | ordinate | 128) == kind).then_some(())?;
    let elevation = real(new, 31)??;
    let same = |code| real(new, code).map(|z| z.is_none_or(|z| z.to_bits() == elevation.to_bits()));
    (same(32)? && same(36)?).then_some(())?;
    match [210, 220, 230].map(|code| real(new, code)) {
        [Some(None), Some(None), Some(None)] => [0.0, 0.0, 1.0].map(|v| fields.bd(v)),
        [Some(Some(x)), Some(Some(y)), Some(Some(z))] => [x, y, z].map(|v| fields.bd(v)),
        _ => return None,
    };
    fields.rd(real(new, 11)??);
    fields.rd(real(new, 21)??);
    fields.bd(elevation);
    let user_location = if flags & 128 == 0 { 1 } else { 0 };
    let block_only = if flags & 32 != 0 { 2 } else { 0 };
    fields.rc(old_flags & !3 | user_location | block_only);
    let texts = |object: &Object| -> Vec<Value> {
        object
            .values
            .iter()
            .filter(|v| v.code == 1)
            .cloned()
            .collect()
    };
    if texts(old) == texts(new) {
        fields.copy(data, text_at..text_end)?;
    } else if texts(new).is_empty() {
        fields.text(&[])?;
    } else {
        string(fields, new, 1)?;
    }
    for code in [53, 51] {
        fields.bd(real(new, code)?.unwrap_or(0.0).to_radians());
    }
    fields.copy(data, scale_at..scale_end)?;
    fields.bs(small(new, 71)??);
    fields.bs(small(new, 72)?.unwrap_or(1));
    fields.bd(real(new, 41)?.unwrap_or(1.0));
    fields.bd(real(new, 42)??);
    let [x, y, z] = [12, 22, 32].map(|code| real(new, code));
    match [x?, y?, z?] {
        [Some(x), Some(y), Some(_)] => {
            fields.rd(x);
            fields.rd(y);
        }
        [None, None, None] => {
            fields.rd(0.0);
            fields.rd(0.0);
        }
        _ => return None,
    }
    for &code in own {
        match code {
            10 | 13 | 14 | 15 => point3(fields, new, code)?,
            16 => {
                fields.rd(real(new, 16)??);
                fields.rd(real(new, 26)??);
            }
            40 => fields.bd(real(new, 40)??),
            50 | 52 => fields.bd(real(new, code)?.unwrap_or(0.0).to_radians()),
            _ => {}
        }
    }
    if type_code == DIMENSION_TYPE {
        fields.rc(old_ordinate & !1 | u8::from(flags & 64 != 0));
    }
    Some(())
}

/// 点（10・20・30）がどれも 0 なら `Some`
fn zero_point(object: &Object) -> Option<()> {
    [10, 20, 30]
        .iter()
        .all(|&code| real(object, code) == Some(Some(0.0)))
        .then_some(())
}

/// INSERT・MINSERT の欄（R2000）。尺度（41〜43）がなければ 1、回転（50）がなければ 0、押し出しの向きが
/// なければ (0, 0, 1) を書く。尺度の印（BB）は、尺度の値から選ぶ。属性が続くかの印は、66 から書く
fn insert(fields: &mut BitWriter, object: &Object, grid: bool) -> Option<()> {
    point3(fields, object, 10)?;
    let [x, y, z] = [41, 42, 43].map(|code| real(object, code));
    let [x, y, z] = [x?, y?, z?].map(|scale| scale.unwrap_or(1.0));
    let one = 1.0f64.to_bits();
    if [x, y, z].iter().all(|scale| scale.to_bits() == one) {
        fields.bb(3);
    } else if x.to_bits() == one {
        fields.bb(1);
        fields.dd(y, 1.0);
        fields.dd(z, 1.0);
    } else if y.to_bits() == x.to_bits() && z.to_bits() == x.to_bits() {
        fields.bb(2);
        fields.rd(x);
    } else {
        fields.bb(0);
        fields.rd(x);
        fields.dd(y, x);
        fields.dd(z, x);
    }
    fields.bd(real(object, 50)?.unwrap_or(0.0).to_radians());
    match [210, 220, 230].map(|code| real(object, code)) {
        [Some(None), Some(None), Some(None)] => [0.0, 0.0, 1.0].map(|v| fields.bd(v)),
        [Some(Some(x)), Some(Some(y)), Some(Some(z))] => [x, y, z].map(|v| fields.bd(v)),
        _ => return None,
    };
    fields.bit(small(object, 66)?.is_some());
    if grid {
        for code in [70, 71] {
            fields.bs(small(object, code)?.unwrap_or(1));
        }
        for code in [44, 45] {
            fields.bd(real(object, code)?.unwrap_or(0.0));
        }
    }
    Some(())
}

/// TOLERANCE の欄（決定 0040）。X 軸の向き（11）がなければ (1, 0, 0)、押し出しの向きがなければ (0, 0, 1) を
/// 書く。文字（1）が変わっていなければ、元のビットを写す
fn tolerance(
    fields: &mut BitWriter,
    old: &Object,
    new: &Object,
    data: &[u8],
    start: usize,
) -> Option<()> {
    let mut bits = Bits::with_release(data, Release::R2000);
    bits.seek(start);
    for _ in 0..9 {
        bits.bd()?;
    }
    let text_at = bits.pos();
    bits.text()?;
    let text_end = bits.pos();
    point3(fields, new, 10)?;
    point3_or(fields, new, 11, [1.0, 0.0, 0.0])?;
    point3_or(fields, new, 210, [0.0, 0.0, 1.0])?;
    if text_of(old, 1) == text_of(new, 1) {
        fields.copy(data, text_at..text_end)?;
    } else {
        string(fields, new, 1)?;
    }
    fields.copy(data, text_end..entity_fields(data)?[3])
}

/// LEADER の欄（決定 0040）。注釈の種類（73）・経路の種類（72、なければ 0）・押し出しの向き（なければ
/// (0, 0, 1)）・X 軸の向き（211、なければ (1, 0, 0)）・注釈の高さと幅（40・41）・鉤の線の向き（74）を
/// 書く。表にない鉤の線の点と差の欄と、後ろの欄は、元のビットを写す
fn leader(fields: &mut BitWriter, object: &Object, data: &[u8], start: usize) -> Option<()> {
    let mut bits = Bits::with_release(data, Release::R2000);
    bits.seek(start);
    bits.bit()?;
    let kinds_at = bits.pos();
    bits.bs()?;
    bits.bs()?;
    let points_at = bits.pos();
    let count = bits.bl()?;
    for _ in 0..=count {
        for _ in 0..3 {
            bits.bd()?;
        }
    }
    let points_end = bits.pos();
    for _ in 0..6 {
        bits.bd()?;
    }
    let offsets_at = bits.pos();
    for _ in 0..6 {
        bits.bd()?;
    }
    let offsets_end = bits.pos();
    bits.bd()?;
    bits.bd()?;
    bits.bit()?;
    let rest_at = bits.pos();
    fields.copy(data, start..kinds_at)?;
    fields.bs(small(object, 73)??);
    fields.bs(small(object, 72)?.unwrap_or(0));
    fields.copy(data, points_at..points_end)?;
    point3_or(fields, object, 210, [0.0, 0.0, 1.0])?;
    point3_or(fields, object, 211, [1.0, 0.0, 0.0])?;
    fields.copy(data, offsets_at..offsets_end)?;
    fields.bd(real(object, 40)??);
    fields.bd(real(object, 41)??);
    fields.bit(match small(object, 74)?? {
        0 => false,
        1 => true,
        _ => return None,
    });
    fields.copy(data, rest_at..entity_fields(data)?[3])
}

/// MLINE の欄（決定 0040）。値を表の並びで読み、DWG の並びで書く。頂点と線と区切りの数は、表の値の数に
/// 合わなければ受け付けない
fn mline(fields: &mut BitWriter, object: &Object) -> Option<()> {
    let mut values = Cursor(
        object
            .values
            .iter()
            .filter(|value| MLINE.contains(&value.code))
            .collect::<Vec<_>>()
            .into_iter()
            .peekable(),
    );
    let scale = values.real(40)?;
    let justification = u8::try_from(values.int(70)?).ok()?;
    let flags = i16::try_from(values.int(71)?).ok()?;
    let count = i16::try_from(values.int(72)?).ok()?;
    let lines = u8::try_from(values.int(73)?).ok()?;
    let point = |fields: &mut BitWriter, values: &mut Cursor<'_>, code| {
        for code in [code, code + 10, code + 20] {
            fields.bd(values.real(code)?);
        }
        Some(())
    };
    fields.bd(scale);
    fields.rc(justification);
    point(fields, &mut values, 10)?;
    point(fields, &mut values, 210)?;
    fields.bs(flags);
    fields.rc(lines);
    fields.bs(count);
    for _ in 0..count {
        for code in [11, 12, 13] {
            point(fields, &mut values, code)?;
        }
        for _ in 0..lines {
            for code in [74, 75] {
                let params = values.int(code)?;
                fields.bs(i16::try_from(params).ok()?);
                for _ in 0..params {
                    fields.bd(values.real(code - 33)?);
                }
            }
        }
    }
    values.0.next().is_none().then_some(())
}

/// VIEWPORT の欄（決定 0041）。表の値を DWG の並びで書き、角度（50・51）は度からラジアンに直す。表にない
/// 凍結した画層の数と後ろの欄は元のビットを写し、印刷スタイルの名前（1）が変わらなければ元のビットを写す
fn viewport(
    fields: &mut BitWriter,
    old: &Object,
    new: &Object,
    data: &[u8],
    start: usize,
) -> Option<()> {
    let mut bits = Bits::with_release(data, Release::R2000);
    bits.seek(start);
    for _ in 0..17 {
        bits.bd()?;
    }
    for _ in 0..8 {
        bits.rd()?;
    }
    bits.bs()?;
    let frozen_at = bits.pos();
    bits.bl()?;
    let frozen_end = bits.pos();
    bits.bl()?;
    let text_at = bits.pos();
    bits.text()?;
    let text_end = bits.pos();
    bits.rc()?;
    bits.bit()?;
    bits.bit()?;
    for _ in 0..10 {
        bits.bd()?;
    }
    bits.bs()?;
    let rest_at = bits.pos();
    point3(fields, new, 10)?;
    for code in [40, 41] {
        fields.bd(real(new, code)??);
    }
    point3(fields, new, 17)?;
    point3(fields, new, 16)?;
    fields.bd(real(new, 51)??.to_radians());
    for code in [45, 42, 43, 44] {
        fields.bd(real(new, code)??);
    }
    fields.bd(real(new, 50)??.to_radians());
    for code in [12, 13, 14, 15] {
        fields.rd(real(new, code)??);
        fields.rd(real(new, code + 10)??);
    }
    fields.bs(small(new, 72)??);
    fields.copy(data, frozen_at..frozen_end)?;
    fields.bl(i32::try_from(integer(new, 90)??).ok()?);
    if text_of(old, 1) == text_of(new, 1) {
        fields.copy(data, text_at..text_end)?;
    } else {
        string(fields, new, 1)?;
    }
    fields.rc(u8::try_from(small(new, 281)??).ok()?);
    for code in [74, 71] {
        fields.bit(match small(new, code)?? {
            0 => false,
            1 => true,
            _ => return None,
        });
    }
    for code in [110, 111, 112] {
        point3(fields, new, code)?;
    }
    fields.bd(real(new, 146)??);
    fields.bs(small(new, 79)??);
    fields.copy(data, rest_at..entity_fields(data)?[3])
}

/// SHAPE の欄（決定 0042）。角度（50・51）がなければ 0、幅の尺度（41）がなければ 1、厚さ（39）がなければ 0、
/// 押し出しの向きがなければ (0, 0, 1) を書く。角度は度からラジアンに直す。表にない形の番号と後ろの欄は、
/// 元のビットを写す
fn shape(fields: &mut BitWriter, object: &Object, data: &[u8], start: usize) -> Option<()> {
    let mut bits = Bits::with_release(data, Release::R2000);
    bits.seek(start);
    for _ in 0..8 {
        bits.bd()?;
    }
    let number_at = bits.pos();
    bits.bs()?;
    let number_end = bits.pos();
    for _ in 0..3 {
        bits.bd()?;
    }
    let rest_at = bits.pos();
    point3(fields, object, 10)?;
    fields.bd(real(object, 40)??);
    fields.bd(real(object, 50)?.unwrap_or(0.0).to_radians());
    fields.bd(real(object, 41)?.unwrap_or(1.0));
    fields.bd(real(object, 51)?.unwrap_or(0.0).to_radians());
    fields.bd(real(object, 39)?.unwrap_or(0.0));
    fields.copy(data, number_at..number_end)?;
    point3_or(fields, object, 210, [0.0, 0.0, 1.0])?;
    fields.copy(data, rest_at..entity_fields(data)?[3])
}

/// 3BD の点（`code`・`code + 10`・`code + 20`）。どれもなければ `default` を書く
fn point3_or(fields: &mut BitWriter, object: &Object, code: i16, default: [f64; 3]) -> Option<()> {
    match [code, code + 10, code + 20].map(|code| real(object, code)) {
        [Some(None), Some(None), Some(None)] => default.map(|v| fields.bd(v)),
        [Some(Some(x)), Some(Some(y)), Some(Some(z))] => [x, y, z].map(|v| fields.bd(v)),
        _ => return None,
    };
    Some(())
}

/// DXF の書き方の文字列を、元のバイト列に戻す。`^` と文字は制御文字に、`^ ` は `^` にする。ほかの `^` の
/// 並びは、読みが作らないので受け付けない
fn uncaret(text: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(text.len());
    let mut bytes = text.iter();
    while let Some(&byte) = bytes.next() {
        if byte != b'^' {
            out.push(byte);
            continue;
        }
        match bytes.next()? {
            b' ' => out.push(b'^'),
            &next @ 0x40..0x60 => out.push(next - 0x40),
            _ => return None,
        }
    }
    Some(out)
}

/// `code` の文字列の値。ちょうど 1 度なければ `None`
fn text_of(object: &Object, code: i16) -> Option<&[u8]> {
    let mut found = object.values.iter().filter(|value| value.code == code);
    match (found.next().map(|value| &value.data), found.next()) {
        (Some(Data::Text(text)), None) => Some(text),
        _ => None,
    }
}

/// 3BD の点（`code`・`code + 10`・`code + 20`）
fn point3(fields: &mut BitWriter, object: &Object, code: i16) -> Option<()> {
    for code in [code, code + 10, code + 20] {
        fields.bd(real(object, code)??);
    }
    Some(())
}

/// BT の厚さ（39）と BE の押し出しの向き（210・220・230）。値がなければ既定の値の印を書く
fn thickness_and_extrusion(fields: &mut BitWriter, object: &Object) -> Option<()> {
    thickness(fields, object)?;
    extrusion(fields, object)
}

/// BT の厚さ（39）
fn thickness(fields: &mut BitWriter, object: &Object) -> Option<()> {
    match real(object, 39)? {
        None => fields.bit(true),
        Some(thickness) => {
            fields.bit(false);
            fields.bd(thickness);
        }
    }
    Some(())
}

/// BE の押し出しの向き（210・220・230）
fn extrusion(fields: &mut BitWriter, object: &Object) -> Option<()> {
    match [210, 220, 230].map(|code| real(object, code)) {
        [Some(None), Some(None), Some(None)] => fields.bit(true),
        [Some(Some(x)), Some(Some(y)), Some(Some(z))] => {
            fields.bit(false);
            fields.bd(x);
            fields.bd(y);
            fields.bd(z);
        }
        _ => return None,
    }
    Some(())
}

/// `code` の値。ないなら `Some(None)`、2 度あるか実数でなければ `None`
fn real(object: &Object, code: i16) -> Option<Option<f64>> {
    let mut found = object.values.iter().filter(|value| value.code == code);
    let value = match found.next() {
        None => return Some(None),
        Some(value) => value,
    };
    match (&value.data, found.next()) {
        (Data::Real(value), None) => Some(Some(*value)),
        _ => None,
    }
}

/// `code` の整数の値。ないなら `Some(None)`、2 度あるか BS に収まる整数でなければ `None`
fn small(object: &Object, code: i16) -> Option<Option<i16>> {
    let mut found = object.values.iter().filter(|value| value.code == code);
    let value = match found.next() {
        None => return Some(None),
        Some(value) => value,
    };
    match (&value.data, found.next()) {
        (Data::Int(value), None) => i16::try_from(*value).ok().map(Some),
        _ => None,
    }
}

/// `code` の整数の値。ないなら `Some(None)`、2 度あるか整数でなければ `None`
fn integer(object: &Object, code: i16) -> Option<Option<i64>> {
    let mut found = object.values.iter().filter(|value| value.code == code);
    let value = match found.next() {
        None => return Some(None),
        Some(value) => value,
    };
    match (&value.data, found.next()) {
        (Data::Int(value), None) => Some(Some(*value)),
        _ => None,
    }
}
