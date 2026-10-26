//! 共有のオブジェクトの表の変更を、DWG のオブジェクトに書き戻す（決定 0029）。R2000 だけを扱う。
//!
//! 表と、今のファイルから作り直した表を、元のオブジェクト（[`Origin::Dwg`]）ごとに比べる。値の並びが違う
//! オブジェクトだけを符号化し直し、1 つの [`DwgTxn`](super::DwgTxn) で置き換える（決定 0028）。今は、LINE・
//! CIRCLE・ARC の型ごとの欄（点・半径・角度・厚さ・押し出しの向き）だけを符号化する。型ごとの欄を書き直して
//! ハンドルの流れの位置（RL）を直し、その前と、ハンドルの流れから後ろのビットは元のまま写す。
//!
//! オブジェクトの追加と削除と並べ替え、ほかの値と見出しの変数の変更は、まだ書き戻せない。

use std::collections::{BTreeMap, BTreeSet};

use zumen_base::{Cx, Diagnostic, Diagnostics, Severity, codes};
use zumen_sdk::{Error, Transact, Transaction};

use super::DwgFile;
use super::bits::BitWriter;
use super::objects::entity_fields;
use crate::db::{AcadDb, Data, Object, Origin};

/// 型ごとの欄のグループ・コード
const LINE: &[i16] = &[10, 20, 30, 11, 21, 31, 39, 210, 220, 230];
const CIRCLE: &[i16] = &[10, 20, 30, 40, 39, 210, 220, 230];
const ARC: &[i16] = &[10, 20, 30, 40, 39, 210, 220, 230, 50, 51];

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

/// `old` から `new` に値を変えたオブジェクトのデータ `data` を、符号化し直す。型ごとの欄のほかの値が
/// 変わったか、符号化できない値なら `None`
fn encode(data: &[u8], old: &Object, new: &Object) -> Option<Vec<u8>> {
    let own = match new.kind.as_slice() {
        b"LINE" => LINE,
        b"CIRCLE" => CIRCLE,
        b"ARC" => ARC,
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
    let mut fields = BitWriter::default();
    if new.kind == b"LINE" {
        let [x0, y0, z0, x1, y1, z1] = [10, 20, 30, 11, 21, 31].map(|code| real(new, code));
        let (x0, y0, z0, x1, y1, z1) = (x0??, y0??, z0??, x1??, y1??, z1??);
        // 両方の Z が 0 なら、Z を書かない
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
        thickness_and_extrusion(&mut fields, new)?;
    } else {
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
    splice(data, fields)
}

/// BT の厚さ（39）と BE の押し出しの向き（210・220・230）。値がなければ既定の値の印を書く
fn thickness_and_extrusion(fields: &mut BitWriter, object: &Object) -> Option<()> {
    match real(object, 39)? {
        None => fields.bit(true),
        Some(thickness) => {
            fields.bit(false);
            fields.bd(thickness);
        }
    }
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

/// `data` の型ごとの欄を `fields` に置き換え、ハンドルの流れの位置を直す
fn splice(data: &[u8], fields: BitWriter) -> Option<Vec<u8>> {
    let [size_at, start, handles_at] = entity_fields(data)?;
    let mut out = BitWriter::default();
    out.copy(data, 0..size_at)?;
    out.rl(i32::try_from(start + fields.len()).ok()?);
    out.copy(data, size_at + 32..start)?;
    let len = fields.len();
    out.copy(&fields.into_bytes(), 0..len)?;
    // ハンドルの流れの後ろの、バイトの終わりまでの埋めのビットも写す
    out.copy(data, handles_at..data.len() * 8)?;
    Some(out.into_bytes())
}
