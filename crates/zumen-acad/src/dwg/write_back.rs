//! 共有のオブジェクトの表の変更を、DWG のオブジェクトに書き戻す（決定 0029）。R2000 だけを扱う。
//!
//! 表と、今のファイルから作り直した表を、元のオブジェクト（[`Origin::Dwg`]）ごとに比べる。値の並びが違う
//! オブジェクトだけを符号化し直し、1 つの [`DwgTxn`](super::DwgTxn) で置き換える（決定 0028）。共通の欄と、
//! いくつかの種類の型ごとの欄（決定 0030〜0032）を符号化する。型ごとの欄を書き直して
//! ハンドルの流れの位置（RL）を直し、その前と、ハンドルの流れから後ろのビットは元のまま写す。
//!
//! オブジェクトの追加と削除と並べ替え、ほかの値と見出しの変数の変更は、まだ書き戻せない。

use std::collections::{BTreeMap, BTreeSet};

use zumen_base::{Cx, Diagnostic, Diagnostics, Severity, codes};
use zumen_sdk::{Error, Transact, Transaction};

use super::bits::{BitWriter, Bits};
use super::objects::entity_fields;
use super::{DwgFile, Release};
use crate::db::{AcadDb, Data, Object, Origin, Section};

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
/// MINSERT の型の番号
const MINSERT_TYPE: i16 = 0x08;

const INSERT: &[i16] = &[10, 20, 30, 41, 42, 43, 50, 210, 220, 230];
const MINSERT: &[i16] = &[10, 20, 30, 41, 42, 43, 50, 210, 220, 230, 70, 71, 44, 45];

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
                    .filter(|entry| {
                        // BLOCK と ENDBLK も、DWG では図形として書かれる
                        self.kind(entry.type_code).is_some_and(|(_, section)| {
                            matches!(section, Section::Entities | Section::Blocks)
                        })
                    })
                    .and_then(|entry| encode(self.data(entry), old, object))
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
    // MINSERT も、表では INSERT とする
    let grid = Bits::with_release(data, Release::R2000).bs()? == MINSERT_TYPE;
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
        b"INSERT" if grid => MINSERT,
        b"INSERT" => INSERT,
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
        own_fields(new, grid)?
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

/// 型ごとの欄。`grid` は MINSERT か。符号化できない種類なら `None`
fn own_fields(new: &Object, grid: bool) -> Option<BitWriter> {
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
        b"TEXT" => text(&mut fields, new)?,
        b"INSERT" => insert(&mut fields, new, grid)?,
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
/// ない欄を表す。標高（30）が 0 なら書かない。合わせの点の Z（31）は、標高と同じでなければならない
fn text(fields: &mut BitWriter, object: &Object) -> Option<()> {
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
    let [generation, horizontal, vertical] = [71, 72, 73].map(|code| small(object, code));
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
    let mut bytes = uncaret(text_of(object, 1)?)?;
    bytes.push(0);
    fields.text(&bytes)?;
    for value in justification.into_iter().flatten() {
        fields.bs(value);
    }
    Some(())
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
