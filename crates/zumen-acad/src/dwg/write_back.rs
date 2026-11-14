//! 共有のオブジェクトの表の変更を、DWG のオブジェクトに書き戻す（決定 0029）。R2000 と R2004 と、R2010 から後の図形・表のレコード・図形でないオブジェクトの変更と、図形の削除と追加を扱う（決定 0085〜0104）。
//! R2004 は、図形の値の変更のうち、共通のフィールドと、単純な種類の型ごとのフィールドだけを書き戻す（決定 0072）。属性を持つ INSERT と
//! MTEXT は、R2004 で後ろに増えたフィールドを元のまま写す（決定 0073）。HATCH は、初めのグラデーションのフィールドを写す（決定 0074）。
//!
//! 表と、今のファイルから作り直した表を、元のオブジェクト（[`Origin::Dwg`]）ごとに比べる。値の並びが違う
//! オブジェクトだけを符号化し直し、1 つの [`DwgTxn`](super::DwgTxn) で置き換える（決定 0028）。共通のフィールドと、
//! いくつかの種類の型ごとのフィールド（決定 0030〜0050）を符号化する。型ごとのフィールドを書き直して
//! ハンドルのストリームの位置（RL）を直し、その前と、ハンドルのストリームから後ろのビットは元のまま写す。
//!
//! 表から除いた図形は、オブジェクトのマップから除き、前後の図形と所有者のブロックのレコードの、図形の並びを
//! たどるハンドルを直す（決定 0051）。
//!
//! 表の終わりに足した図形のうち、いくつかの種類は、一から符号化してオブジェクトのマップに加え、所有者の
//! ブロックのレコードの図形の並びの終わりに繋ぎ、ヘッダーの変数の HANDSEED を進める（決定 0052）。TEXT・ATTDEF・INSERT は、指す文字スタイルとブロックを
//! 名前で探して、そのハンドルを書く（決定 0053）。足した MTEXT は、表にないフィールドを 0 にする（決定 0054）。
//! 属性を持つ INSERT は、すぐ後の ATTRIB と SEQEND を持たせ、そのハンドルを書く（決定 0055）。足した LWPOLYLINE は、
//! 表にないフラグのビットと一定の幅を 0 にする（決定 0056）。足した SPLINE は、閉じている・周期的のフラグを 0 に
//! して書く（決定 0057）。重みを持つものは有理のフラグを立て、フィット点を持つものは場合を 2 にする（決定 0059）。SHAPE は、形の番号と形のファイルを表で示せないので、
//! 足せない（決定 0058）。列と行の数か間隔を持つ INSERT は、MINSERT として書き、属性も INSERT と同じく
//! 続ける（決定 0060）。
//!
//! 表の終わりに足した画層・文字スタイル・線種・アプリケーションの名前・寸法スタイル・ビュー・ビューポートの設定・ブロックの
//! レコードは、一から符号化してオブジェクトのマップに加え、所有者の表のオブジェクトの項目の終わりに足す（決定 0061〜0068）。
//! 足したブロックのレコードは、同じく足した BLOCK・ENDBLK と図形を揃えて書く（決定 0068）。
//!
//! ヘッダーの変数の INSUNITS と、用紙空間の PLIMMIN・PLIMMAX と、PDMODE・PDSIZE の変更は、ヘッダーの変数のセクションの中で
//! 書き直す（決定 0069・0190）。PDMODE・PDSIZE は、書く幅が変わる値ならセクションの大きさを変える（決定 0193）。
//!
//! ほかのオブジェクトの追加と並べ替え、図形でないものの削除、ほかの値と、ほかのヘッダーの変数の変更は、まだ書き戻せない。
//!
//! このファイルは、変更の検出と、オブジェクトのマップ・ヘッダーの変数への反映を持つ。符号化は、子のモジュールに分けて置く。
//!
//! - `groups`: 表に置くグループ・コードの並びと、書き戻せる種類の名前
//! - `restyle`: 図形の画層・線種の参照の付け替え
//! - `links`: 図形の追加と削除で、前後の図形・所有者・リアクターのハンドルをつなぎ直す
//! - `encoding`: オブジェクトを、リリースごとの並びで符号化し直す
//! - `records`: 表のレコードと図形でないオブジェクトの、種類ごとのフィールド
//! - `fields`・`entities`・`curves`: 図形の共通のフィールドと、種類ごとのフィールド

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;

use zumen_base::{Cx, Diagnostic, Diagnostics, Severity, codes};
use zumen_sdk::{Error, Transact, Transaction};

use super::bits::{BitWriter, Bits, Color, modular_char, push_modular_char};
use super::objects::{
    DIM_FIELDS, Dim, block_links, dim_fields, entity_fields, entity_links, entity_refs,
    hatch_gradient, layer_line_type, multileader, object_fields, owned, r2010_entity_fields,
    r2010_object_fields, skip_extended_data, style_handles,
};
use super::{DwgFile, Release, header, kinds};
use crate::db::{AcadDb, Data, DataType, Object, Origin, Section, Value, uncaret};

mod curves;
mod encoding;
mod entities;
mod fields;
mod groups;
mod links;
mod records;
mod restyle;

#[cfg(test)]
mod layer_name_tests;
#[cfg(test)]
mod leader_tests;
#[cfg(test)]
mod mtext_tests;
#[cfg(test)]
mod table_entry_tests;
#[cfg(test)]
mod true_color_layer_tests;

use self::curves::*;
use self::encoding::*;
use self::entities::*;
use self::fields::*;
use self::groups::*;
use self::links::*;
use self::records::*;
use self::restyle::*;

/// 足したオブジェクトのハンドルとデータの並び
type Added = Vec<(u64, Vec<u8>)>;

/// 並びをたどったオブジェクトのハンドルと番号の並び
type Chain = Vec<(u64, usize)>;

/// 書き戻す変更
#[derive(Debug, Default)]
struct Changes {
    /// 変えたオブジェクトの番号と、新しいデータ
    set: BTreeMap<usize, Vec<u8>>,
    /// 除いたオブジェクトの番号
    removed: Vec<usize>,
    /// 足したオブジェクトのハンドルと、データ
    added: Added,
    /// 新しい HANDSEED
    seed: Option<u64>,
    /// 新しい INSUNITS と、用紙空間の PLIMMIN・PLIMMAX（決定 0069）
    units: Option<i16>,
    limits: Option<[f64; 4]>,
    /// 新しい PDMODE・PDSIZE（決定 0190）
    point_mode: Option<i16>,
    point_size: Option<f64>,
}

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
                .set
                .iter()
                .try_for_each(|(record, data)| txn.set_data(*record, data))
                .and_then(|()| changes.removed.iter().try_for_each(|&r| txn.remove(r)))
                .and_then(|()| {
                    changes
                        .added
                        .iter()
                        .try_for_each(|(handle, data)| txn.add(*handle, data))
                })
                .and_then(|()| {
                    changes
                        .seed
                        .map_or(Ok(()), |seed| txn.set_handle_seed(seed))
                })
                .and_then(|()| changes.units.map_or(Ok(()), |units| txn.set_units(units)))
                .and_then(|()| {
                    changes
                        .limits
                        .map_or(Ok(()), |limits| txn.set_paper_limits(limits))
                })
                .and_then(|()| {
                    changes
                        .point_mode
                        .map_or(Ok(()), |mode| txn.set_point_mode(mode))
                })
                .and_then(|()| {
                    changes
                        .point_size
                        .map_or(Ok(()), |size| txn.set_point_size(size))
                }),
            Err(diagnostic) => Err(txn.reject(*diagnostic)),
        };
        // 失敗した変更があれば、確定が最初の失敗を診断として返し、何も反映しない
        let committed = txn.commit(cx);
        staged.and(committed)
    }

    /// 表 `db` を書き戻す変更
    fn changes(&self, db: &AcadDb, base: &AcadDb) -> Result<Changes, Box<Diagnostic>> {
        if self.release == Release::R2007 {
            let same_vars = base
                .variables()
                .chain(db.variables())
                .map(|(n, _)| n)
                .all(|n| base.variable(n) == db.variable(n));
            if db.objects() == base.objects() && same_vars {
                return Ok(Changes::default());
            }
            return Err(Box::new(
                Diagnostic::new(codes::DWG_EDIT_UNSUPPORTED).with("version", "AC1021"),
            ));
        }
        let r2004 = self.release == Release::R2004;
        let r2010 = self.release >= Release::R2010;
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
        let mut changes = BTreeMap::new();
        let mut added = Vec::new();
        for object in db.objects() {
            let found = match object.origin {
                Origin::Dwg { offset, .. } => before.get(&offset).filter(|(_, old)| {
                    old.kind == object.kind && old.section == object.section && kept.insert(offset)
                }),
                Origin::Dxf { .. } | Origin::New => {
                    added.push(object);
                    continue;
                }
            };
            let Some(&(record, old)) = found else {
                return Err(unsupported(&object.kind));
            };
            // 足した図形は、表の終わりにだけ置ける
            if !added.is_empty() {
                return Err(unsupported(b"ENTITIES"));
            }
            if old.values != object.values {
                // 足したリアクターの組は、ほかのフィールドと分けて、リアクターの数とハンドルのストリームに書く（決定 0191）
                let (reactors, object) = without_reactors(old, object);
                let object = &object;
                // 拡張辞書の組は、読む側が置いたままのものだけを受け付け、型ごとのフィールドからは除く（決定 0148・0152）
                let (old, object) = match (without_xdictionary(old), without_xdictionary(object)) {
                    ((before, old), (after, object)) if before == after => (old, object),
                    _ => return Err(unsupported(&object.kind)),
                };
                let (old, object) = (&old, &object);
                // 図形の画層と線種は、名前を表から引いてハンドルで書く（#723）
                let restyle = match object.section {
                    Section::Entities | Section::Blocks => Restyle::of(db, old, object),
                    Section::Tables if object.kind == b"LAYER" => Restyle::layer(db, old, object),
                    _ => Some(Restyle::default()),
                };
                let data = restyle
                    .and_then(|restyle| Some((restyle, self.entries.get(record)?)))
                    .and_then(|(restyle, entry)| {
                        let data = self.data(entry);
                        if old.values == object.values {
                            return Some(if r2010 {
                                let mut bytes = Vec::new();
                                let bits =
                                    data.len().checked_mul(8)?.checked_sub(entry.handles?)?;
                                push_modular_char(&mut bytes, i64::try_from(bits).ok()?, false);
                                bytes.extend_from_slice(data);
                                bytes
                            } else {
                                data.to_vec()
                            });
                        }
                        // BLOCK と ENDBLK も、DWG では図形として書かれる。R2010 から後は、図形だけを扱う
                        match self.kind(entry.type_code)?.1 {
                            Section::Entities | Section::Blocks
                                if !r2010 && object.kind == b"MULTILEADER" =>
                            {
                                restyle.is_empty().then_some(())?;
                                callout(data, old, object, self.release)
                            }
                            Section::Entities | Section::Blocks if r2010 => encode_r2010(
                                data,
                                entry.handles?,
                                old,
                                object,
                                self.release,
                                restyle,
                            ),
                            Section::Tables if r2010 => encode_record_r2010(
                                data,
                                entry.handles?,
                                old,
                                object,
                                self.release,
                                restyle.ltype,
                            ),
                            Section::Objects if r2010 => {
                                encode_object_r2010(data, entry.handles?, old, object, self.release)
                            }
                            _ if r2010 => None,
                            Section::Entities | Section::Blocks => {
                                encode(data, old, object, self.release, restyle)
                            }
                            Section::Tables => {
                                encode_record(data, old, object, self.release, restyle.ltype)
                            }
                            Section::Objects => encode_object(data, old, object, self.release),
                            _ => None,
                        }
                    })
                    .and_then(|data| {
                        if reactors.is_empty() {
                            return Some(data);
                        }
                        if !matches!(object.section, Section::Entities | Section::Blocks) {
                            return None;
                        }
                        if r2010 {
                            let mut skip = 0;
                            let bits =
                                usize::try_from(modular_char(&data, &mut skip, false)?).ok()?;
                            let data = data.get(skip..)?;
                            let handles_at = (data.len() * 8).checked_sub(bits)?;
                            react(data, Some(handles_at), self.release, &reactors)
                        } else {
                            react(&data, None, self.release, &reactors)
                        }
                    })
                    .ok_or_else(|| unsupported(&object.kind))?;
                changes.insert(record, data);
            }
        }
        let removed: Vec<(usize, &Object)> = before
            .iter()
            .filter(|(offset, _)| !kept.contains(*offset))
            .map(|(_, &found)| found)
            .collect();
        // 並べ替えたオブジェクト
        if !db.objects().iter().map(offset).take(kept.len()).eq(base
            .objects()
            .iter()
            .map(offset)
            .filter(|o| o.is_none_or(|o| kept.contains(&o))))
        {
            return Err(unsupported(b"ENTITIES"));
        }
        let names: BTreeSet<&[u8]> = base
            .variables()
            .chain(db.variables())
            .map(|(n, _)| n)
            .collect();
        // 変えられるのは、このファイルのヘッダーの変数のセクションから読んだ変数だけ
        let (mut units, mut limits) = (None, None);
        let (mut point_mode, mut point_size) = (None, None);
        for name in names
            .into_iter()
            .filter(|n| base.variable(n) != db.variable(n))
        {
            if base.variable(name).is_none() {
                return Err(unsupported(name));
            }
            match name {
                b"$INSUNITS" => {
                    // セクションの長さを変えないよう、BS の幅が変わる値は受け付けない
                    let new = header_units(db)
                        .filter(|&new| header_units(base).map(bs_width) == Some(bs_width(new)))
                        .ok_or_else(|| unsupported(name))?;
                    units = Some(new);
                }
                b"$PLIMMIN" | b"$PLIMMAX" => {
                    limits = Some(paper_limits(db).ok_or_else(|| unsupported(name))?);
                }
                // BS・BD の幅が変わる値は、ヘッダーのセクションの大きさを変えて書く（決定 0190・0193）
                b"$PDMODE" => {
                    point_mode = Some(header_short(db, name).ok_or_else(|| unsupported(name))?);
                }
                b"$PDSIZE" => {
                    point_size = Some(header_real(db, name).ok_or_else(|| unsupported(name))?);
                }
                _ => return Err(unsupported(name)),
            }
        }
        // R2004 は、図形と表のレコードの、R2000 と同じく書けるものの追加だけを書き戻す（決定 0079〜0081）
        if r2004 && let Some(object) = added.iter().find(|object| !r2004_addable(object)) {
            return Err(unsupported(&object.kind));
        }
        // R2010 から後は、文字列のストリームの形が R2000 と同じ図形の追加だけを書き戻す（決定 0092）
        if r2010 && let Some(object) = added.iter().find(|object| !r2010_addable(object)) {
            return Err(unsupported(&object.kind));
        }
        self.unlink(base, &removed, &mut changes)?;
        let (added, seed) = self.append(db, &added, &mut changes)?;
        Ok(Changes {
            set: changes,
            removed: removed.iter().map(|&(record, _)| record).collect(),
            added,
            seed,
            units,
            limits,
            point_mode,
            point_size,
        })
    }

    /// 足した図形 `added` を符号化し、所有者のブロックのレコードの図形の並びの終わりに繋ぐ（決定 0052）。
    /// 前の終わりの図形とレコードの、並びをたどるハンドルを直したデータを `changes` に足し、足した図形の
    /// ハンドルとデータと、新しい HANDSEED を返す。ハンドルは、今の HANDSEED から表の順に大きくなければ
    /// ならない。所有者はこのファイルのブロックのレコード、画層（8）は表にある画層でなければならない
    /// 足した表のレコードは、図形の並びへ繋がず、表のオブジェクトに加える（決定 0061〜0068）。足した
    /// ブロックのレコードは、同じく足した BLOCK・ENDBLK と、それが持つ図形を最後に揃えてから符号化する
    /// （決定 0068）
    fn append(
        &self,
        db: &AcadDb,
        added: &[&Object],
        changes: &mut BTreeMap<usize, Vec<u8>>,
    ) -> Result<(Added, Option<u64>), Box<Diagnostic>> {
        let Some(&last) = added.last() else {
            return Ok((Vec::new(), None));
        };
        let mut seed = self.handle_seed().ok_or_else(|| unsupported(b"HANDSEED"))?;
        let records: BTreeMap<u64, usize> = self
            .entries
            .iter()
            .enumerate()
            .map(|(record, entry)| (entry.handle, record))
            .collect();
        let mut pending: Vec<NewEntity> = Vec::new();
        let mut blocks: BTreeMap<u64, Vec<usize>> = BTreeMap::new();
        // 属性が続く（66）INSERT の番号と、その後の ATTRIB の番号。ATTRIB は INSERT のすぐ後に並べ、
        // SEQEND で閉じる。ATTRIB は ATTRIB どうしで繋ぎ、SEQEND は前後を持たない
        let mut open: Option<(usize, Vec<usize>)> = None;
        let mut inner = BTreeMap::new();
        // 足した表のレコードの、足したものの中の番号とハンドルとデータ。図形の並びへは入れない
        let mut tables: BTreeMap<usize, (u64, Vec<u8>)> = BTreeMap::new();
        // 足したブロックのレコードのハンドルごとの、足したものの中の番号と、BLOCK・ENDBLK・図形
        let mut new_blocks: BTreeMap<u64, (usize, NewBlock)> = added
            .iter()
            .enumerate()
            .filter(|(_, o)| o.kind == b"BLOCK_RECORD" && o.section == Section::Tables)
            .filter_map(|(position, o)| Some((o.handle()?, (position, NewBlock::default()))))
            .collect();
        for (position, &object) in added.iter().enumerate() {
            if object.section == Section::Tables {
                if let Some((insert, _)) = &open {
                    let kind = pending
                        .get(*insert)
                        .map_or(&object.kind, |i| &i.object.kind);
                    return Err(unsupported(kind));
                }
                // ブロックのレコードは、BLOCK・ENDBLK と図形が揃ってから符号化する
                if object.kind == b"BLOCK_RECORD" {
                    let handle = object
                        .handle()
                        .filter(|&h| h >= seed)
                        .ok_or_else(|| unsupported(&object.kind))?;
                    seed = handle + 1;
                    continue;
                }
                let (handle, data) = self
                    .add_record(db, object, &records, seed, changes, None)
                    .ok_or_else(|| unsupported(&object.kind))?;
                seed = handle + 1;
                tables.insert(position, (handle, data));
                continue;
            }
            let parent = open
                .as_ref()
                .and_then(|(i, _)| pending.get(*i))
                .map(|insert| (insert.handle, insert.object.section.clone()));
            let new = self
                .prepare(db, object, &records, &new_blocks, seed, parent)
                .ok_or_else(|| unsupported(&object.kind))?;
            seed = new.handle + 1;
            let index = pending.len();
            if let Some((at, block)) = new_blocks.get_mut(&new.owner) {
                match object.kind.as_slice() {
                    b"ATTDEF" => block.attdef = true,
                    b"BLOCK" | b"ENDBLK" => {
                        // 1 つのブロックに 1 つずつ。BLOCK は基点（10・20・30）を持てる
                        if open.is_some() {
                            return Err(unsupported(&object.kind));
                        }
                        let slot = if object.kind == b"BLOCK" {
                            &mut block.block
                        } else {
                            &mut block.endblk
                        };
                        if slot.replace(new.handle).is_some() {
                            return Err(unsupported(&object.kind));
                        }
                        // BLOCK の名前は、レコードの名前と同じ
                        if object.kind == b"BLOCK" {
                            let name = added.get(*at).and_then(|record| text_of(record, 2));
                            block.base = base_point(object)
                                .filter(|_| name.is_some() && name == text_of(object, 2))
                                .ok_or_else(|| unsupported(&object.kind))?;
                        }
                        pending.push(new);
                        continue;
                    }
                    _ => {}
                }
            }
            // 足したブロックを挿す INSERT は、そのレコードの挿す図形の並びへ入れる
            if let Some((_, block)) = new
                .pointed
                .filter(|_| object.kind == b"INSERT")
                .and_then(|pointed| new_blocks.get_mut(&pointed))
            {
                block.inserts.push(new.handle);
            }
            match (object.kind.as_slice(), open.take()) {
                (b"ATTRIB", Some((insert, mut attribs))) => {
                    attribs.push(index);
                    open = Some((insert, attribs));
                }
                (b"SEQEND", Some((insert, attribs))) => {
                    let handles: Vec<u64> = attribs
                        .iter()
                        .filter_map(|&i| Some(pending.get(i)?.handle))
                        .collect();
                    if handles.is_empty() {
                        return Err(unsupported(&object.kind));
                    }
                    for (n, &i) in attribs.iter().enumerate() {
                        let prev = n.checked_sub(1).and_then(|m| handles.get(m));
                        let next = handles.get(n + 1);
                        inner.insert(i, (prev.copied().unwrap_or(0), next.copied().unwrap_or(0)));
                    }
                    if let Some(insert) = pending.get_mut(insert) {
                        insert.children = Some((handles, new.handle));
                    }
                }
                (b"ATTRIB" | b"SEQEND", None) => return Err(unsupported(&object.kind)),
                (_, Some((insert, _))) => {
                    let kind = pending.get(insert).map_or(&object.kind, |i| &i.object.kind);
                    return Err(unsupported(kind));
                }
                (_, None) => {
                    if small(object, 66)
                        .ok_or_else(|| unsupported(&object.kind))?
                        .is_some()
                    {
                        open = Some((index, Vec::new()));
                    }
                    blocks.entry(new.owner).or_default().push(index);
                }
            }
            pending.push(new);
        }
        // SEQEND で閉じていない INSERT
        if let Some((insert, _)) = open {
            let kind = pending.get(insert).map_or(&last.kind, |i| &i.object.kind);
            return Err(unsupported(kind));
        }
        let mut links: Vec<(u64, u64)> = (0..pending.len())
            .map(|i| inner.get(&i).copied().unwrap_or((0, 0)))
            .collect();
        for (block, list) in blocks {
            let handles: Vec<u64> = list
                .iter()
                .filter_map(|&i| Some(pending.get(i)?.handle))
                .collect();
            // 足したブロックの図形は、足した順に繋ぐ
            if let Some((_, new)) = new_blocks.get_mut(&block) {
                for (n, &i) in list.iter().enumerate() {
                    let prev = n.checked_sub(1).and_then(|m| handles.get(m)).copied();
                    let next = handles.get(n + 1).copied();
                    if let Some(slot) = links.get_mut(i) {
                        *slot = (prev.unwrap_or(0), next.unwrap_or(0));
                    }
                }
                new.entities = handles;
                continue;
            }
            let block_record = *records
                .get(&block)
                .ok_or_else(|| unsupported(b"BLOCK_RECORD"))?;
            // R2010 から後も同じく、ブロックのレコードの持つ図形に加える（決定 0092）
            if self.release >= Release::R2010 {
                let data = self
                    .current_r2010(changes, block_record)
                    .and_then(|(data, at)| adopt_r2010(&data, at, block, &handles, self.release))
                    .ok_or_else(|| unsupported(b"BLOCK_RECORD"))?;
                changes.insert(block_record, data);
                continue;
            }
            // R2004 の図形は前後の図形のハンドルを持たず、ブロックのレコードの持つ図形に加える
            if self.release == Release::R2004 {
                let data = self
                    .current(changes, block_record)
                    .and_then(|data| adopt(&data, block, &handles))
                    .ok_or_else(|| unsupported(b"BLOCK_RECORD"))?;
                changes.insert(block_record, data);
                continue;
            }
            let (first, _, chain) = self
                .chain(block_record, block, &records, changes)
                .ok_or_else(|| unsupported(b"BLOCK_RECORD"))?;
            let joined = self
                .join(block_record, block, first, &chain, &handles, changes)
                .ok_or_else(|| unsupported(b"BLOCK_RECORD"))?;
            for (&i, link) in list.iter().zip(joined) {
                if let Some(slot) = links.get_mut(i) {
                    *slot = link;
                }
            }
        }
        for (handle, (position, block)) in &new_blocks {
            let object = added
                .get(*position)
                .copied()
                .ok_or_else(|| unsupported(b"BLOCK_RECORD"))?;
            let (record, data) = self
                .add_record(db, object, &records, *handle, changes, Some(block))
                .ok_or_else(|| unsupported(&object.kind))?;
            tables.insert(*position, (record, data));
        }
        let mut entities = pending
            .iter()
            .zip(links)
            .map(|(new, (prev, next))| Some((new.handle, new.encode(prev, next, self.release)?)))
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| unsupported(&last.kind))?
            .into_iter();
        let data = (0..added.len())
            .filter_map(|position| tables.remove(&position).or_else(|| entities.next()))
            .collect();
        Ok((data, Some(seed)))
    }

    /// 足した図形 `object` を符号化するのに要るもの。ハンドルが `seed` より小さいか、足せない図形なら `None`
    fn prepare<'a>(
        &self,
        db: &AcadDb,
        object: &'a Object,
        records: &BTreeMap<u64, usize>,
        new_blocks: &BTreeMap<u64, (usize, NewBlock)>,
        seed: u64,
        parent: Option<(u64, Section)>,
    ) -> Option<NewEntity<'a>> {
        let &(_, type_code, own) = ADDABLE.iter().find(|(kind, ..)| *kind == object.kind)?;
        // 列と行の数（70・71）か間隔（44・45）を持つ INSERT は、MINSERT として書く（決定 0060）
        let grid = object.kind == b"INSERT"
            && object
                .values
                .iter()
                .any(|value| matches!(value.code, 70 | 71 | 44 | 45));
        let (type_code, own) = if grid {
            (kinds::MINSERT, MINSERT)
        } else {
            (type_code, own)
        };
        let handle = object.handle().filter(|&h| h >= seed)?;
        // TEXT と ATTDEF は文字スタイル（7）を、INSERT はブロック（2）を指す
        let pointing: &[i16] = match object.kind.as_slice() {
            b"TEXT" | b"ATTDEF" | b"MTEXT" => &[7],
            // LEADER は寸法スタイル（3）を指す（決定 0185）
            b"LEADER" => &[3],
            b"ATTRIB" => &[7],
            // 属性が続くこと（66）は、1 だけ
            b"INSERT" if small(object, 66)?.is_none_or(|v| v == 1) => &[2, 66],
            b"INSERT" => return None,
            _ => &[],
        };
        // R2010 からは、MTEXT は枠の高さ（46）を、ATTRIB は属性のバージョンと位置を固定するか（280）を持つ（決定 0093）
        let r2010: &[i16] = match object.kind.as_slice() {
            b"MTEXT" if self.release >= Release::R2010 => &[46],
            b"ATTRIB" if self.release >= Release::R2010 => &[280],
            _ => &[],
        };
        let codes = [&[5, 330, 8, 6, 67], COMMON, own, pointing, r2010].concat();
        // R2018 からの複数行の ATTRIB は、属性の値の後に、種類（71）・72・埋め込みのオブジェクトのマーカー（101）と
        // MTEXT の値（枠の高さ（46）も）を持つ（決定 0102）
        let (values, embedded) = match multiline_at(object) {
            Some(at) if object.kind == b"ATTRIB" && self.release >= Release::R2018 => {
                object.values.split_at(at)
            }
            _ => (object.values.as_slice(), &[][..]),
        };
        let embedded_codes = [&[71, 72, 101, 46], MTEXT].concat();
        if values.iter().any(|value| !codes.contains(&value.code))
            || embedded
                .iter()
                .any(|value| !embedded_codes.contains(&value.code))
        {
            return None;
        }
        let owner = object.owner()?;
        // BLOCK と ENDBLK は、足したブロックのレコードだけが持つ
        (!matches!(object.kind.as_slice(), b"BLOCK" | b"ENDBLK")
            || new_blocks.contains_key(&owner))
        .then_some(())?;
        // ATTRIB と SEQEND は、すぐ前の INSERT が持ち、同じ所に置く。所有者のハンドルを持つ（entmode が 0）
        let (mode, section) = if matches!(object.kind.as_slice(), b"ATTRIB" | b"SEQEND") {
            let (insert, section) = parent?;
            (owner == insert).then_some((0, section))?
        } else if new_blocks.contains_key(&owner) {
            // 足したブロックの図形は、ほかのブロックと同じく所有者のハンドルを持つ
            (0, Section::Blocks)
        } else {
            let entry = self.entries.get(*records.get(&owner)?)?;
            let block = db.get(owner).filter(|_| {
                self.kind(entry.type_code)
                    .is_some_and(|(k, _)| k == b"BLOCK_RECORD")
            })?;
            // モデル空間と用紙空間の図形は、所有者のハンドルを持たない（entmode が 2・1）
            let name = text_of(block, 2)?;
            let mode = if name.eq_ignore_ascii_case(b"*Model_Space") {
                2
            } else if name.eq_ignore_ascii_case(b"*Paper_Space") {
                1
            } else {
                0
            };
            let section = if mode == 0 {
                Section::Blocks
            } else {
                Section::Entities
            };
            (mode, section)
        };
        // 用紙空間の図形だけが、67 = 1 を持つ
        let paper = small(object, 67)?;
        if object.section != section || paper != (mode == 1).then_some(1) {
            return None;
        }
        let layer = record_handle(db, b"LAYER", text_of(object, 8)?)?;
        let ltype = ltype_ref(db, object)?;
        let pointed = match object.kind.as_slice() {
            // 文字スタイルがなければ Standard
            b"TEXT" | b"ATTRIB" | b"ATTDEF" | b"MTEXT" => {
                match object.values.iter().any(|value| value.code == 7) {
                    false => Some(record_handle(db, b"STYLE", b"Standard")?),
                    true => Some(record_handle(db, b"STYLE", text_of(object, 7)?)?),
                }
            }
            b"LEADER" => {
                // 注記（340）は、図面にある MTEXT だけを指せる
                if let Some(Data::Handle(annotation)) = object.first(340) {
                    db.get(*annotation).filter(|found| found.kind == b"MTEXT")?;
                }
                Some(record_handle(db, b"DIMSTYLE", text_of(object, 3)?)?)
            }
            b"INSERT" => {
                // 空間のブロックと、図形を持つブロックそのものは挿せない
                let name = text_of(object, 2)?;
                let space = [b"*Model_Space".as_slice(), b"*Paper_Space"]
                    .iter()
                    .any(|space| {
                        name.get(..space.len())
                            .is_some_and(|head| head.eq_ignore_ascii_case(space))
                    });
                let block = record_handle(db, b"BLOCK_RECORD", name)?;
                (!space && block != owner).then_some(block)
            }
            _ => None,
        };
        if !pointing.is_empty() && pointed.is_none() {
            return None;
        }
        // R2018 からの複数行の ATTRIB の埋め込みの MTEXT は、ByLayer の線種と材質を指す（決定 0102）
        let embedded = match embedded.is_empty() {
            true => None,
            false => Some((
                record_handle(db, b"LTYPE", b"ByLayer")?,
                dictionary_item(db, b"ACAD_MATERIAL", b"ByLayer")?,
            )),
        };
        Some(NewEntity {
            object,
            handle,
            type_code,
            mode,
            owner,
            layer,
            ltype,
            pointed,
            embedded,
            children: None,
        })
    }

    /// 足した表のレコード `object` のハンドルと、R2000 のレコードのデータ（決定 0061〜0068）。所有者（330）の
    /// 表のオブジェクトの項目の数を増やし、終わりに新しいレコードのハンドルを足したデータを `changes` に入れる。
    /// 画層・文字スタイル・線種・アプリケーションの名前（APPID）・寸法スタイル・ビュー・ビューポートの設定（VPORT）・
    /// ブロックのレコードだけを扱い、名前（2）が表にないことが要る。
    /// 画層は、フラグ（70）・色（62）・線の太さ（370）を持ち、印刷スタイル（390）と線種（6）が表になければ
    /// ならない。印刷スタイルがなければ、画層 0 と同じものにする。文字スタイルは、型ごとのフィールドをみな持たなければ
    /// ならない。線種は、要素がみな形も文字も持たないことが要る。アプリケーションの名前は、フラグ（70）が 0 で
    /// なければならない。寸法スタイルは、表にないフィールドを既定の値で書き、文字スタイルとブロックのハンドル（340〜344）
    /// が図面になければならない。ビューは、型ごとのフィールドをみな持ち、UCS のハンドル（345・346）が図面になければ
    /// ならない。ビューポートの設定は、型ごとのフィールドをみな持ち、UCS のハンドル（345・346）が図面になければ
    /// ならない。ブロックのレコードは、名前が `*` で始まってはならず、`block` の BLOCK・ENDBLK・図形・INSERT と
    /// 基点を指す。ハンドルが `seed` より小さいか、足せないレコードなら `None`
    fn add_record(
        &self,
        db: &AcadDb,
        object: &Object,
        records: &BTreeMap<u64, usize>,
        seed: u64,
        changes: &mut BTreeMap<usize, Vec<u8>>,
        block: Option<&NewBlock>,
    ) -> Option<(u64, Vec<u8>)> {
        // R2010 からは、寸法スタイルは増えたフィールドと、寸法線と補助線の線種（345〜347）を持つ（決定 0095）
        let r2010_dimstyle = self.release >= Release::R2010;
        let dimstyle_codes: Vec<i16> = std::iter::once(70)
            .chain(dim_fields(self.release).iter().map(|field| field.0))
            .chain(340..=344)
            .chain((345..=347).filter(|_| r2010_dimstyle))
            .collect();
        let view_codes = [VIEW, &[345, 346]].concat();
        let vport_codes = [VPORT, &[345, 346]].concat();
        // レコードと表のオブジェクトの型と、型ごとのフィールドのグループ・コード
        let (type_code, control_type, own): (u16, u16, &[i16]) = match object.kind.as_slice() {
            b"LAYER" => (
                kinds::LAYER,
                kinds::LAYER_CONTROL,
                &[70, 62, 290, 370, 390, 347, 6],
            ),
            b"STYLE" => (kinds::STYLE, kinds::STYLE_CONTROL, STYLE),
            b"LTYPE" => (
                kinds::LTYPE,
                kinds::LTYPE_CONTROL,
                &[70, 3, 72, 73, 40, 49, 74],
            ),
            b"APPID" => (kinds::APPID, kinds::APPID_CONTROL, &[70]),
            b"DIMSTYLE" => (kinds::DIMSTYLE, kinds::DIMSTYLE_CONTROL, &dimstyle_codes),
            b"VIEW" => (kinds::VIEW, kinds::VIEW_CONTROL, &view_codes),
            b"VPORT" => (kinds::VPORT, kinds::VPORT_CONTROL, &vport_codes),
            b"BLOCK_RECORD" => (
                kinds::BLOCK_RECORD,
                kinds::BLOCK_RECORD_CONTROL,
                &[70, 280, 281],
            ),
            _ => return None,
        };
        let handle = object.handle().filter(|&h| h >= seed)?;
        // DIMSTYLE のハンドルは、DXF と同じく 105 に置く
        let at = if type_code == kinds::DIMSTYLE { 105 } else { 5 };
        let codes = [&[at, 330, 2], own].concat();
        if object
            .values
            .iter()
            .any(|value| !codes.contains(&value.code))
        {
            return None;
        }
        let name = text_of(object, 2)?;
        let same = db
            .objects()
            .iter()
            .filter(|o| o.kind == object.kind && o.section == Section::Tables)
            .filter(|o| text_of(o, 2).is_some_and(|n| n.eq_ignore_ascii_case(name)))
            .count();
        (!name.is_empty() && same == 1).then_some(())?;
        // 名前のないブロックと空間のブロック（* で始まる）は足せない
        (type_code != kinds::BLOCK_RECORD || !name.starts_with(b"*")).then_some(())?;
        let r2004 = self.release >= Release::R2004;
        // R2010 からは、文字列を文字列のストリームに書き、外部参照の番号を持たない（決定 0094）
        let r2010 = self.release >= Release::R2010;
        let mut fields = match r2010 {
            true => BitWriter::with_strings(),
            false => BitWriter::default(),
        };
        fields.handle(0, handle);
        // 拡張データとリアクターがない。R2004 からは、拡張辞書を持たないフラグを立てる。R2013 からは、DS のデータを
        // 持たないフラグが続く
        fields.bs(0);
        fields.bl(0);
        if r2004 {
            fields.bit(true);
        }
        if self.release >= Release::R2013 {
            fields.bit(false);
        }
        string(&mut fields, object, 2)?;
        // 参照されるフラグ・外部参照の番号・外部参照に依るフラグ
        fields.bit(true);
        if !r2010 {
            fields.bs(0);
        }
        fields.bit(false);
        let control = object.owner()?;
        let mut handles = BitWriter::default();
        handles.handle(4, control);
        if !r2004 {
            handles.handle(3, 0);
        }
        // 外部参照のブロック
        handles.handle(5, 0);
        if type_code == kinds::LAYER {
            let (flags, color) = layer_flags(object, false, 0)?;
            fields.bs(flags);
            cmc(&mut fields, color, self.release)?;
            let plot_style = match object.values.iter().find(|value| value.code == 390) {
                Some(value) => match value.data {
                    Data::Handle(h) => h,
                    _ => return None,
                },
                None => match db
                    .get(record_handle(db, b"LAYER", b"0")?)?
                    .values
                    .iter()
                    .find(|value| value.code == 390)?
                    .data
                {
                    Data::Handle(h) => h,
                    _ => return None,
                },
            };
            db.get(plot_style)?;
            handles.handle(5, plot_style);
            if r2010 {
                // 材質は、指定がなければ画層 0 と同じにする
                let material = |layer: &Object| {
                    layer.values.iter().find_map(|value| match value.data {
                        Data::Handle(h) if value.code == 347 => Some(h),
                        _ => None,
                    })
                };
                let material = match material(object) {
                    Some(h) => h,
                    None => material(db.get(record_handle(db, b"LAYER", b"0")?)?)?,
                };
                db.get(material)?;
                handles.handle(5, material);
            }
            handles.handle(5, record_handle(db, b"LTYPE", text_of(object, 6)?)?);
        } else if type_code == kinds::STYLE {
            // 形のファイル 1 と縦書き 4 のフラグ、高さ・幅の係数・斜めの角度・生成のフラグ・前の高さ・字体の名前
            let flags = small(object, 70)??;
            (flags & !(1 | 4) == 0).then_some(())?;
            fields.bit(flags & 1 != 0);
            fields.bit(flags & 4 != 0);
            fields.bd(real(object, 40)??);
            fields.bd(real(object, 41)??);
            fields.bd(real(object, 50)??.to_radians());
            fields.rc(u8::try_from(small(object, 71)??).ok()?);
            fields.bd(real(object, 42)??);
            string(&mut fields, object, 3)?;
            string(&mut fields, object, 4)?;
        } else if type_code == kinds::LTYPE {
            // 外部参照に依らない線種の、説明・揃え・要素の数・長さと、要素ごとの長さ。形も文字もない要素の
            // 形の番号・ずれ・尺度・回転は、サンプルの図面と同じく 0・0・0・1・0 とし、文字の領域は 0 で埋める
            let mut values = Cursor(
                object
                    .values
                    .iter()
                    .filter(|value| own.contains(&value.code))
                    .collect::<Vec<_>>()
                    .into_iter()
                    .peekable(),
            );
            (values.int(70)? == 0).then_some(())?;
            let Data::Text(_) = values.next(3)? else {
                return None;
            };
            string(&mut fields, object, 3)?;
            let align = u8::try_from(values.int(72)?).ok()?;
            let count = u8::try_from(values.int(73)?).ok()?;
            fields.bd(values.real(40)?);
            fields.rc(align);
            fields.rc(count);
            for _ in 0..count {
                fields.bd(values.real(49)?);
                (values.int(74)? == 0).then_some(())?;
                fields.bs(0);
                fields.rd(0.0);
                fields.rd(0.0);
                fields.bd(1.0);
                fields.bd(0.0);
                fields.bs(0);
                // 形のファイルの文字スタイル
                handles.handle(5, 0);
            }
            values.0.next().is_none().then_some(())?;
            // R2010 からは、文字の要素がなければ文字の領域を持たない（決定 0089）
            if !r2010 {
                for _ in 0..256 {
                    fields.rc(0);
                }
            }
        } else if type_code == kinds::APPID {
            // 使い道の分からない RC は、サンプルの図面と同じく 0 とする
            (small(object, 70)?? == 0).then_some(())?;
            fields.rc(0);
        } else if type_code == kinds::VIEW {
            let (lighting, bits) = default_lighting();
            let associated = view_fields(
                &mut fields,
                object,
                false,
                r2010.then_some((&lighting[..], 0..bits)),
            )?;
            // R2010 からは、カメラを印刷するかの B が続き、サンプルの図面と同じく偽とする。外部参照のブロックの
            // 後に、背景・表示スタイル・太陽を指す。表示スタイルはサンプルの図面と同じく 2dWireframe を指し、
            // 見つからなければ持たない。背景と太陽は持たないものとする（決定 0104）
            if r2010 {
                fields.bit(false);
                handles.handle(4, 0);
                handles.handle(
                    5,
                    dictionary_item(db, b"ACAD_VISUALSTYLE", b"2dWireframe").unwrap_or(0),
                );
                handles.handle(3, 0);
            }
            // UCS を伴わないビューは、UCS のフィールドを持たない
            let ucs = [110, 120, 130, 111, 121, 131, 112, 122, 132, 79, 146];
            (associated || !object.values.iter().any(|value| ucs.contains(&value.code)))
                .then_some(())?;
            // UCS を伴うビューは、基準の UCS と名前の UCS を指す。サンプルの図面と同じく、参照の型は 5
            if associated {
                for code in [346, 345] {
                    handles.handle(5, ucs_handle(db, object, code)?);
                }
            } else {
                (ucs_handle(db, object, 346)? == 0 && ucs_handle(db, object, 345)? == 0)
                    .then_some(())?;
            }
            // R2010 からは、最後に断面の表示を指す
            if r2010 {
                handles.handle(4, 0);
            }
        } else if type_code == kinds::VPORT {
            // 使い道の分からないビットは、サンプルの図面と同じく偽とする
            let (lighting, bits) = default_lighting();
            vport_fields(
                &mut fields,
                object,
                false,
                None,
                false,
                r2010.then_some((&lighting[..], 0..bits)),
            )?;
            // R2010 からは、格子のフラグと主の線の間隔（BS）が続く。サンプルの図面の多くと同じく 3 と 5 とする。外部参照の
            // ブロックの後に、背景・表示スタイル・太陽を指す。どれも持たないものとする（決定 0096）
            if r2010 {
                fields.bs(3);
                fields.bs(5);
                handles.handle(4, 0);
                handles.handle(5, 0);
                handles.handle(3, 0);
            }
            // 名前の UCS と基準の UCS。サンプルの図面と同じく、参照の型は 5
            for code in [345, 346] {
                handles.handle(5, ucs_handle(db, object, code)?);
            }
        } else if type_code == kinds::BLOCK_RECORD {
            let block = block?;
            // 名前のないフラグ・属性を持つフラグ・外部参照のフラグ・重ねる外部参照のフラグ・読み込んだフラグと基点。属性を
            // 持つのは、ATTDEF を持つブロック
            fields.bit(false);
            fields.bit(block.attdef);
            for _ in 0..3 {
                fields.bit(false);
            }
            // R2004 からは、持つ図形の数を置く
            if r2004 {
                fields.bl(i32::try_from(block.entities.len()).ok()?);
            }
            for value in block.base {
                fields.bd(value);
            }
            // 外部参照のパス。挿す INSERT ごとに 1 を書き、0 で閉じる
            fields.text(b"")?;
            for _ in &block.inserts {
                fields.rc(1);
            }
            fields.rc(0);
            // 説明と、絵の大きさ
            fields.text(b"")?;
            fields.bl(0);
            // R2010 からは、挿入単位（BS）・分解できるか（B）・尺度を揃えるか（RC）が続く。レコードの 70・280・281
            // を書き、ないものはサンプルの図面の多くと同じく、単位なし・分解できる・揃えないとする（決定 0095）
            // R2004 までは書くフィールドがないので、単位なし・分解できる・揃えない以外は書けないものとする（推定。docs/formats/dwg.md）
            if r2010 {
                fields.bs(small(object, 70)?.unwrap_or(0));
                fields.bit(small(object, 280)?.unwrap_or(1) != 0);
                fields.rc(u8::try_from(small(object, 281)?.unwrap_or(0)).ok()?);
            } else {
                (small(object, 70)?.unwrap_or(0) == 0
                    && small(object, 280)?.unwrap_or(1) == 1
                    && small(object, 281)?.unwrap_or(0) == 0)
                    .then_some(())?;
            }
            handles.handle(3, block.block?);
            // R2000 は最初と最後の図形を指し、R2004 は持つ図形をみな固い所有で指す
            if r2004 {
                for &entity in &block.entities {
                    handles.handle(3, entity);
                }
            } else {
                handles.handle(4, block.entities.first().copied().unwrap_or(0));
                handles.handle(4, block.entities.last().copied().unwrap_or(0));
            }
            handles.handle(3, block.endblk?);
            for &insert in &block.inserts {
                handles.handle(4, insert);
            }
            // 配置
            handles.handle(5, 0);
        } else {
            (small(object, 70)?? == 0).then_some(())?;
            // R2010 からは、文字の背景の色（DIMTFILLCLR）も 70 に置かれるので、2 つ目の 70 は受け付けない
            (object
                .values
                .iter()
                .filter(|value| value.code == 70)
                .count()
                == 1)
                .then_some(())?;
            for (code, field) in dim_fields(self.release) {
                dim_field(&mut fields, object, code, field, self.release)?;
            }
            // R2010 からは、使い道の分からない B が続く。サンプルの図面と同じく偽とする
            if r2010 {
                fields.bit(false);
            }
            // 文字スタイルと、引出線・矢印・1 つめと 2 つめの矢印のブロック。サンプルの図面と同じく、参照の型は 5
            // R2010 からは、寸法線と 2 つの補助線の線種が続く
            let last = if r2010 { 347 } else { 344 };
            for code in 340..=last {
                let handle = match object.values.iter().find(|value| value.code == code) {
                    Some(value) => match value.data {
                        Data::Handle(h) if code < 345 => db.get(h).map(|_| h)?,
                        Data::Handle(h) => (db.get(h)?.kind == b"LTYPE").then_some(h)?,
                        _ => return None,
                    },
                    None => 0,
                };
                handles.handle(5, handle);
            }
        }
        let record = *records.get(&control)?;
        (self.entries.get(record)?.type_code == control_type).then_some(())?;
        let entry = match r2010 {
            true => {
                let (data, handles_at) = self.current_r2010(changes, record)?;
                add_entry_r2010(
                    &data,
                    handles_at,
                    Entry::Add(handle),
                    type_code,
                    self.release,
                )?
            }
            false => add_entry(
                &self.current(changes, record)?,
                Entry::Add(handle),
                type_code,
                self.release,
            )?,
        };
        changes.insert(record, entry);
        if r2010 {
            let mut out = BitWriter::default();
            out.bot(i16::try_from(type_code).ok()?);
            return Some((handle, r2010_assemble(out, fields, handles)?));
        }
        let mut out = BitWriter::default();
        out.bs(i16::try_from(type_code).ok()?);
        out.rl(i32::try_from(out.len() + 32 + fields.len()).ok()?);
        for part in [fields, handles] {
            let len = part.len();
            out.copy(&part.into_bytes(), 0..len)?;
        }
        Some((handle, out.into_bytes()))
    }

    /// `record` 番目のオブジェクトの、今の変更 `changes` を入れたデータ
    fn current(&self, changes: &BTreeMap<usize, Vec<u8>>, record: usize) -> Option<Vec<u8>> {
        match changes.get(&record) {
            Some(data) => Some(data.clone()),
            None => Some(self.data(self.entries.get(record)?).to_vec()),
        }
    }

    /// R2010 から後の `record` 番目のオブジェクトの、今のデータ（ハンドルのストリームの大きさ（MC）の後）と、
    /// ハンドルのストリームの始まりのビットの位置。`changes` のデータは MC から始まる
    fn current_r2010(
        &self,
        changes: &BTreeMap<usize, Vec<u8>>,
        record: usize,
    ) -> Option<(Vec<u8>, usize)> {
        match changes.get(&record) {
            Some(bytes) => {
                let mut skip = 0;
                let bits = usize::try_from(modular_char(bytes, &mut skip, false)?).ok()?;
                let data = bytes.get(skip..)?.to_vec();
                let handles_at = (data.len() * 8).checked_sub(bits)?;
                Some((data, handles_at))
            }
            None => {
                let entry = self.entries.get(record)?;
                Some((self.data(entry).to_vec(), entry.handles?))
            }
        }
    }

    /// `block_record` 番目のオブジェクトのブロックのレコード `block` の、最初と最後の図形のハンドルと、
    /// 最初の図形から後ろの図形のハンドルをたどった図形のハンドルと番号の並び。たどれなければ `None`
    fn chain(
        &self,
        block_record: usize,
        block: u64,
        records: &BTreeMap<u64, usize>,
        changes: &BTreeMap<usize, Vec<u8>>,
    ) -> Option<(u64, u64, Chain)> {
        let (_, first, last) = block_links(&self.current(changes, block_record)?, block)?;
        // 輪は数えて止める
        let mut chain = Vec::new();
        let mut at = first;
        while at != 0 {
            if chain.len() >= self.entries.len() {
                return None;
            }
            let record = *records.get(&at)?;
            chain.push((at, record));
            if at == last {
                break;
            }
            at = entity_links(&self.current(changes, record)?, at)?.next;
        }
        (chain.last().map_or(0, |&(handle, _)| handle) == last).then_some((first, last, chain))
    }

    /// ブロックのレコード `block` の図形の並び `chain` の終わりに、ハンドル `handles` の図形を繋ぐ。前の
    /// 終わりの図形とレコードの、並びをたどるハンドルを直したデータを `changes` に足し、足す図形ごとの
    /// 前後の図形のハンドルを返す
    fn join(
        &self,
        block_record: usize,
        block: u64,
        first: u64,
        chain: &[(u64, usize)],
        handles: &[u64],
        changes: &mut BTreeMap<usize, Vec<u8>>,
    ) -> Option<Vec<(u64, u64)>> {
        let (&new_first, &new_last) = (handles.first()?, handles.last()?);
        let tail = chain.last().map(|&(handle, _)| handle);
        if let Some(&(handle, record)) = chain.last() {
            let prev = chain
                .len()
                .checked_sub(2)
                .and_then(|i| chain.get(i))
                .map_or(0, |&(h, _)| h);
            let data = relink(&self.current(changes, record)?, handle, prev, new_first)?;
            changes.insert(record, data);
        }
        let data = reblock(
            &self.current(changes, block_record)?,
            block,
            if tail.is_some() { first } else { new_first },
            new_last,
        )?;
        changes.insert(block_record, data);
        Some(
            (0..handles.len())
                .map(|i| {
                    let prev = i.checked_sub(1).and_then(|j| handles.get(j)).copied();
                    let next = handles.get(i + 1).copied();
                    (prev.or(tail).unwrap_or(0), next.unwrap_or(0))
                })
                .collect(),
        )
    }

    /// ヘッダーの変数の HANDSEED。表に足す図形には、これより小さくないハンドルを付ける（決定 0052）。
    /// 読めなければ `None`
    pub fn handle_seed(&self) -> Option<u64> {
        let (src, span) = self.header_section()?;
        Some(header::handle_seed(src, span.start, self.release)?.value)
    }

    /// 除いた図形 `removed` の前後の図形と、所有者のブロックのレコードの、図形の並びをたどるハンドルを直した
    /// データを、`changes` に足す（決定 0051）。残る図形を持つ図形（ATTRIB を持つ INSERT など）と、リアクターを
    /// 持つ図形と、[`REMOVABLE`] にない種類は除けない。拡張辞書を持つ図形は、拡張辞書も一緒に除くとき（ハンドルが
    /// `all` にある）だけ除ける。図形でないオブジェクトは、所有者も除くとき（除くレコードや図形の拡張辞書と、
    /// それが持つオブジェクト。決定 0170・#881）だけ除ける
    fn unlink(
        &self,
        base: &AcadDb,
        removed: &[(usize, &Object)],
        changes: &mut BTreeMap<usize, Vec<u8>>,
    ) -> Result<(), Box<Diagnostic>> {
        let records: BTreeMap<u64, usize> = self
            .entries
            .iter()
            .enumerate()
            .map(|(record, entry)| (entry.handle, record))
            .collect();
        let all: BTreeSet<u64> = removed
            .iter()
            .filter_map(|&(record, _)| self.entries.get(record).map(|entry| entry.handle))
            .collect();
        let kept_owners: BTreeSet<u64> = base
            .objects()
            .iter()
            .filter(|o| o.handle().is_none_or(|h| !all.contains(&h)))
            .filter_map(Object::owner)
            .collect();
        let mut gone = BTreeSet::new();
        let mut blocks = BTreeMap::new();
        for &(record, old) in removed {
            if old.section == Section::Tables {
                self.unlist(record, old, &all, &records, changes)
                    .ok_or_else(|| unsupported(&old.kind))?;
                continue;
            }
            if old.section == Section::Objects {
                // 所有者と一緒に除く。残る子を持つものは除かない
                let handle = self
                    .entries
                    .get(record)
                    .map(|entry| entry.handle)
                    .filter(|handle| !kept_owners.contains(handle));
                handle
                    .and(old.owner())
                    .filter(|owner| all.contains(owner))
                    .ok_or_else(|| unsupported(&old.kind))?;
                continue;
            }
            let (xdictionary, rest) = without_xdictionary(old);
            let xdic_gone = match xdictionary {
                Some(Data::Handle(handle)) => handle != 0 && all.contains(&handle),
                Some(_) => false,
                None => true,
            };
            let no_other_groups = !rest
                .values
                .iter()
                .any(|value| matches!(value.code, 102 | 360));
            let block = self
                .entries
                .get(record)
                .filter(|entry| {
                    matches!(old.section, Section::Entities | Section::Blocks)
                        && REMOVABLE.contains(&old.kind.as_slice())
                        && !kept_owners.contains(&entry.handle)
                        && xdic_gone
                        && no_other_groups
                })
                .and_then(|entry| {
                    let data = self.data(entry);
                    let free = if self.release >= Release::R2004 {
                        let (reactors, has_xdic) = entity_refs(data, self.release)?;
                        reactors == 0 && (!has_xdic || xdictionary.is_some())
                    } else {
                        let links = entity_links(data, entry.handle)?;
                        links.reactors == 0
                            && (links.xdic == 0 || xdictionary == Some(Data::Handle(links.xdic)))
                    };
                    let owner = old.owner()?;
                    let block = self.entries.get(*records.get(&owner)?)?;
                    (free && self.kind(block.type_code)?.0 == b"BLOCK_RECORD")
                        .then_some((entry.handle, owner))
                })
                .ok_or_else(|| unsupported(&old.kind))?;
            gone.insert(block.0);
            *blocks.entry(block.1).or_insert(0usize) += 1;
        }
        for (block, count) in blocks {
            // R2004 の図形は前後の図形のハンドルを持たず、ブロックのレコードが持つ図形を並べる（決定 0077）
            // R2010 から後も同じく、ブロックのレコードが持つ図形を並べる（決定 0091）
            let done = if self.release >= Release::R2010 {
                records.get(&block).and_then(|&record| {
                    let (data, handles_at) = self.current_r2010(changes, record)?;
                    let new = disown_r2010(&data, handles_at, block, count, &gone, self.release)?;
                    changes.insert(record, new);
                    Some(())
                })
            } else if self.release == Release::R2004 {
                records.get(&block).and_then(|&record| {
                    let new = disown(&self.current(changes, record)?, block, count, &gone)?;
                    changes.insert(record, new);
                    Some(())
                })
            } else {
                self.relink_block(block, count, &gone, &records, changes)
            };
            done.ok_or_else(|| unsupported(b"BLOCK_RECORD"))?;
        }
        Ok(())
    }

    /// 除いた画層・文字スタイル・線種・寸法スタイルのレコード `old`（オブジェクトのマップの `record`）を、表のオブジェクトの項目から
    /// 除いたデータを、`changes` に足す（決定 0162〜0165）。リアクターを持つレコードと、ほかの表のレコードは
    /// 除けない。拡張辞書を持つレコードは、拡張辞書も一緒に除くとき（ハンドルが `all` にある。決定 0170）だけ除ける
    fn unlist(
        &self,
        record: usize,
        old: &Object,
        all: &BTreeSet<u64>,
        records: &BTreeMap<u64, usize>,
        changes: &mut BTreeMap<usize, Vec<u8>>,
    ) -> Option<()> {
        let (type_code, control_type) = match old.kind.as_slice() {
            b"LAYER" => (kinds::LAYER, kinds::LAYER_CONTROL),
            b"STYLE" => (kinds::STYLE, kinds::STYLE_CONTROL),
            b"LTYPE" => (kinds::LTYPE, kinds::LTYPE_CONTROL),
            b"DIMSTYLE" => (kinds::DIMSTYLE, kinds::DIMSTYLE_CONTROL),
            _ => return None,
        };
        let (xdictionary, rest) = without_xdictionary(old);
        let gone = match xdictionary {
            Some(Data::Handle(handle)) => all.contains(&handle),
            Some(_) => false,
            None => true,
        };
        (gone
            && !rest
                .values
                .iter()
                .any(|value| matches!(value.code, 102 | 360)))
        .then_some(())?;
        let handle = self.entries.get(record)?.handle;
        let control = *records.get(&old.owner()?)?;
        (self.entries.get(control)?.type_code == control_type).then_some(())?;
        let new = if self.release >= Release::R2010 {
            let (data, handles_at) = self.current_r2010(changes, control)?;
            add_entry_r2010(
                &data,
                handles_at,
                Entry::Remove(handle),
                type_code,
                self.release,
            )?
        } else {
            add_entry(
                &self.current(changes, control)?,
                Entry::Remove(handle),
                type_code,
                self.release,
            )?
        };
        changes.insert(control, new);
        Some(())
    }

    /// ブロックのレコード `block` の図形の並びから、`gone` の `count` 個の図形を除き、隣が変わった図形と
    /// レコードの、並びをたどるハンドルを直す。並びをたどれなければ `None`
    fn relink_block(
        &self,
        block: u64,
        count: usize,
        gone: &BTreeSet<u64>,
        records: &BTreeMap<u64, usize>,
        changes: &mut BTreeMap<usize, Vec<u8>>,
    ) -> Option<()> {
        let data = |changes: &BTreeMap<usize, Vec<u8>>, record| self.current(changes, record);
        let block_record = *records.get(&block)?;
        let (first, last, chain) = self.chain(block_record, block, records, changes)?;
        if chain.iter().filter(|(h, _)| gone.contains(h)).count() != count {
            return None;
        }
        let neighbours = |list: &[(u64, usize)], i: usize| {
            let handle = |j: Option<usize>| j.and_then(|j| list.get(j)).map_or(0, |&(h, _)| h);
            (handle(i.checked_sub(1)), handle(i.checked_add(1)))
        };
        let kept: Vec<_> = chain
            .iter()
            .copied()
            .filter(|(h, _)| !gone.contains(h))
            .collect();
        for (i, &(handle, record)) in kept.iter().enumerate() {
            let was = chain.iter().position(|&(h, _)| h == handle)?;
            let (prev, next) = neighbours(&kept, i);
            if (prev, next) != neighbours(&chain, was) {
                let new = relink(&data(changes, record)?, handle, prev, next)?;
                changes.insert(record, new);
            }
        }
        let ends = (
            kept.first().map_or(0, |&(h, _)| h),
            kept.last().map_or(0, |&(h, _)| h),
        );
        if ends != (first, last) {
            let new = reblock(&data(changes, block_record)?, block, ends.0, ends.1)?;
            changes.insert(block_record, new);
        }
        Some(())
    }
}

/// 表 `db` の INSUNITS。BS に収まる 70 の整数だけを持たなければ `None`（決定 0069）
fn header_units(db: &AcadDb) -> Option<i16> {
    header_short(db, b"$INSUNITS")
}

/// 表 `db` の変数 `name`。BS に収まる 70 の整数だけを持たなければ `None`
fn header_short(db: &AcadDb, name: &[u8]) -> Option<i16> {
    match db.variable(name)? {
        [
            Value {
                code: 70,
                data: Data::Int(value),
            },
        ] => i16::try_from(*value).ok(),
        _ => None,
    }
}

/// 表 `db` の変数 `name`。有限の 40 の実数だけを持たなければ `None`
fn header_real(db: &AcadDb, name: &[u8]) -> Option<f64> {
    match db.variable(name)? {
        [
            Value {
                code: 40,
                data: Data::Real(value),
            },
        ] if value.is_finite() => Some(*value),
        _ => None,
    }
}

/// BS で `value` を書くときのビットの数
fn bs_width(value: i16) -> u32 {
    match value {
        0 | 256 => 2,
        1..=255 => 10,
        _ => 18,
    }
}

/// 表 `db` の用紙空間の PLIMMIN と PLIMMAX（X・Y の順）。どちらも、有限の 10 と 20 の実数だけを持たなければ
/// `None`（決定 0069）
fn paper_limits(db: &AcadDb) -> Option<[f64; 4]> {
    let point = |name: &[u8]| match db.variable(name)? {
        [
            Value {
                code: 10,
                data: Data::Real(x),
            },
            Value {
                code: 20,
                data: Data::Real(y),
            },
        ] if x.is_finite() && y.is_finite() => Some([*x, *y]),
        _ => None,
    };
    let ([x0, y0], [x1, y1]) = (point(b"$PLIMMIN")?, point(b"$PLIMMAX")?);
    Some([x0, y0, x1, y1])
}
