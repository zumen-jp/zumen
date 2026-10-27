//! 共有のオブジェクトの表の変更を、DWG のオブジェクトに書き戻す（決定 0029）。R2000 だけを扱う。
//!
//! 表と、今のファイルから作り直した表を、元のオブジェクト（[`Origin::Dwg`]）ごとに比べる。値の並びが違う
//! オブジェクトだけを符号化し直し、1 つの [`DwgTxn`](super::DwgTxn) で置き換える（決定 0028）。共通の欄と、
//! いくつかの種類の型ごとの欄（決定 0030〜0050）を符号化する。型ごとの欄を書き直して
//! ハンドルの流れの位置（RL）を直し、その前と、ハンドルの流れから後ろのビットは元のまま写す。
//!
//! 表から除いた図形は、オブジェクトの地図から除き、前後の図形と所有者のブロックのレコードの、図形の並びを
//! たどるハンドルを直す（決定 0051）。
//!
//! 表の終わりに足した図形のうち、いくつかの種類は、一から符号化してオブジェクトの地図に加え、所有者の
//! ブロックのレコードの図形の並びの終わりに繋ぎ、見出しの変数の HANDSEED を進める（決定 0052）。TEXT・ATTDEF・INSERT は、指す文字の体裁とブロックを
//! 名前で探して、そのハンドルを書く（決定 0053）。足した MTEXT は、表にない欄を 0 にする（決定 0054）。
//! 属性を持つ INSERT は、すぐ後の ATTRIB と SEQEND を持たせ、そのハンドルを書く（決定 0055）。足した LWPOLYLINE は、
//! 表にない印のビットと一定の幅を 0 にする（決定 0056）。足した SPLINE は、閉じている・周期的の印を 0 に
//! して書く（決定 0057）。重みを持つものは有理の印を立て、当てはめる点を持つものは場合を 2 にする（決定 0059）。SHAPE は、形の番号と形のファイルを表で示せないので、
//! 足せない（決定 0058）。列と行の数か間隔を持つ INSERT は、MINSERT として書き、属性も INSERT と同じく
//! 続ける（決定 0060）。
//!
//! 表の終わりに足した画層・文字の体裁・線種・アプリケーションの名前・寸法の体裁・ビュー・ビューポートの設定は、一から符号化して
//! オブジェクトの地図に加え、所有者の表のオブジェクトの項目の終わりに足す（決定 0061〜0067）。
//!
//! ほかのオブジェクトの追加と並べ替え、図形でないものの削除、ほかの値と見出しの変数の変更は、まだ書き戻せない。

use std::collections::{BTreeMap, BTreeSet};

use zumen_base::{Cx, Diagnostic, Diagnostics, Severity, codes};
use zumen_sdk::{Error, Transact, Transaction};

use super::bits::{BitWriter, Bits};
use super::objects::{
    DIM_FIELDS, Dim, block_links, entity_fields, entity_links, object_fields, skip_extended_data,
};
use super::{DwgFile, Release, header};
use crate::db::{AcadDb, Data, DataType, Object, Origin, Section, Value};

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
const LAYER_TYPE: i16 = 0x33;
const LAYER_CONTROL_TYPE: u16 = 0x32;
const STYLE_TYPE: i16 = 0x35;
const STYLE_CONTROL_TYPE: u16 = 0x34;
const LTYPE_TYPE: i16 = 0x39;
const LTYPE_CONTROL_TYPE: u16 = 0x38;
const APPID_TYPE: i16 = 0x43;
const APPID_CONTROL_TYPE: u16 = 0x42;
const DIMSTYLE_TYPE: i16 = 0x45;
const DIMSTYLE_CONTROL_TYPE: u16 = 0x44;
const VIEW_TYPE: i16 = 0x3D;
const VIEW_CONTROL_TYPE: u16 = 0x3C;
const VPORT_TYPE: i16 = 0x41;
const VPORT_CONTROL_TYPE: u16 = 0x40;
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
const LTYPE: &[i16] = &[70, 3, 72, 73, 40, 49, 74, 75, 46, 50, 44, 45, 9];
const VIEW: &[i16] = &[
    70, 40, 10, 20, 41, 11, 21, 31, 12, 22, 32, 42, 43, 44, 50, 71, 281, 72, 110, 120, 130, 111,
    121, 131, 112, 122, 132, 79, 146,
];
const UCS: &[i16] = &[70, 10, 20, 30, 11, 21, 31, 12, 22, 32, 79, 146];
const VPORT: &[i16] = &[
    70, 10, 20, 11, 21, 12, 22, 13, 23, 14, 24, 15, 25, 16, 26, 36, 17, 27, 37, 40, 41, 42, 43, 44,
    50, 51, 71, 72, 73, 74, 75, 76, 77, 78, 281, 65, 110, 120, 130, 111, 121, 131, 112, 122, 132,
    79, 146,
];
const SHAPE: &[i16] = &[10, 20, 30, 40, 50, 41, 51, 39, 210, 220, 230];
const VIEWPORT: &[i16] = &[
    10, 20, 30, 40, 41, 12, 22, 13, 23, 14, 24, 15, 25, 16, 26, 36, 17, 27, 37, 42, 43, 44, 45, 50,
    51, 72, 90, 1, 281, 71, 74, 110, 120, 130, 111, 121, 131, 112, 122, 132, 146, 79,
];
const MLINE: &[i16] = &[
    40, 70, 71, 72, 73, 10, 20, 30, 210, 220, 230, 11, 21, 31, 12, 22, 32, 13, 23, 33, 74, 41, 75,
    42,
];

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
                }),
            Err(diagnostic) => Err(txn.reject(*diagnostic)),
        };
        // 失敗した変更があれば、確定が最初の失敗を診断として返し、何も反映しない
        let committed = txn.commit(cx);
        staged.and(committed)
    }

    /// 表 `db` を書き戻す変更
    fn changes(&self, db: &AcadDb, base: &AcadDb) -> Result<Changes, Box<Diagnostic>> {
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
                let data = self
                    .entries
                    .get(record)
                    .and_then(|entry| {
                        let data = self.data(entry);
                        // BLOCK と ENDBLK も、DWG では図形として書かれる
                        match self.kind(entry.type_code)?.1 {
                            Section::Entities | Section::Blocks => encode(data, old, object),
                            Section::Tables => encode_record(data, old, object),
                            Section::Objects => encode_object(data, old, object),
                            _ => None,
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
        if let Some(name) = names
            .into_iter()
            .find(|n| base.variable(n) != db.variable(n))
        {
            return Err(unsupported(name));
        }
        self.unlink(base, &removed, &mut changes)?;
        let (added, seed) = self.append(db, &added, &mut changes)?;
        Ok(Changes {
            set: changes,
            removed: removed.iter().map(|&(record, _)| record).collect(),
            added,
            seed,
        })
    }

    /// 足した図形 `added` を符号化し、所有者のブロックのレコードの図形の並びの終わりに繋ぐ（決定 0052）。
    /// 前の終わりの図形とレコードの、並びをたどるハンドルを直したデータを `changes` に足し、足した図形の
    /// ハンドルとデータと、新しい HANDSEED を返す。ハンドルは、今の HANDSEED から表の順に大きくなければ
    /// ならない。所有者はこのファイルのブロックのレコード、画層（8）は表にある画層でなければならない
    /// 足した表のレコードは、図形の並びへ繋がず、表のオブジェクトに加える（決定 0061〜0067）
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
        for (position, &object) in added.iter().enumerate() {
            if object.section == Section::Tables {
                if let Some((insert, _)) = &open {
                    let kind = pending
                        .get(*insert)
                        .map_or(&object.kind, |i| &i.object.kind);
                    return Err(unsupported(kind));
                }
                let (handle, data) = self
                    .add_record(db, object, &records, seed, changes)
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
                .prepare(db, object, &records, seed, parent)
                .ok_or_else(|| unsupported(&object.kind))?;
            seed = new.handle + 1;
            let index = pending.len();
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
                    let (&first, &last) = handles
                        .first()
                        .zip(handles.last())
                        .ok_or_else(|| unsupported(&object.kind))?;
                    for (n, &i) in attribs.iter().enumerate() {
                        let prev = n.checked_sub(1).and_then(|m| handles.get(m));
                        let next = handles.get(n + 1);
                        inner.insert(i, (prev.copied().unwrap_or(0), next.copied().unwrap_or(0)));
                    }
                    if let Some(insert) = pending.get_mut(insert) {
                        insert.children = Some((first, last, new.handle));
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
            let block_record = *records
                .get(&block)
                .ok_or_else(|| unsupported(b"BLOCK_RECORD"))?;
            let (first, _, chain) = self
                .chain(block_record, block, &records, changes)
                .ok_or_else(|| unsupported(b"BLOCK_RECORD"))?;
            let handles: Vec<u64> = list
                .iter()
                .filter_map(|&i| Some(pending.get(i)?.handle))
                .collect();
            let joined = self
                .join(block_record, block, first, &chain, &handles, changes)
                .ok_or_else(|| unsupported(b"BLOCK_RECORD"))?;
            for (&i, link) in list.iter().zip(joined) {
                if let Some(slot) = links.get_mut(i) {
                    *slot = link;
                }
            }
        }
        let mut entities = pending
            .iter()
            .zip(links)
            .map(|(new, (prev, next))| Some((new.handle, new.encode(prev, next)?)))
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
            (MINSERT_TYPE, MINSERT)
        } else {
            (type_code, own)
        };
        let handle = object.handle().filter(|&h| h >= seed)?;
        // TEXT と ATTDEF は文字の体裁（7）を、INSERT はブロック（2）を指す
        let pointing: &[i16] = match object.kind.as_slice() {
            b"TEXT" | b"ATTDEF" | b"MTEXT" => &[7],
            b"ATTRIB" => &[7],
            // 属性が続くこと（66）は、1 だけ
            b"INSERT" if small(object, 66)?.is_none_or(|v| v == 1) => &[2, 66],
            b"INSERT" => return None,
            _ => &[],
        };
        let codes = [&[5, 330, 8, 6, 67], COMMON, own, pointing].concat();
        if object
            .values
            .iter()
            .any(|value| !codes.contains(&value.code))
        {
            return None;
        }
        let owner = object.owner()?;
        // ATTRIB と SEQEND は、すぐ前の INSERT が持ち、同じ所に置く。所有者のハンドルを持つ（entmode が 0）
        let (mode, section) = if matches!(object.kind.as_slice(), b"ATTRIB" | b"SEQEND") {
            let (insert, section) = parent?;
            (owner == insert).then_some((0, section))?
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
        let ltype = match object.values.iter().any(|value| value.code == 6) {
            false => (0, 0),
            true => match text_of(object, 6)? {
                name if name.eq_ignore_ascii_case(b"ByLayer") => (0, 0),
                name if name.eq_ignore_ascii_case(b"ByBlock") => (1, 0),
                name if name.eq_ignore_ascii_case(b"Continuous") => (2, 0),
                name => (3, record_handle(db, b"LTYPE", name)?),
            },
        };
        let pointed = match object.kind.as_slice() {
            // 文字の体裁がなければ Standard
            b"TEXT" | b"ATTRIB" | b"ATTDEF" | b"MTEXT" => {
                match object.values.iter().any(|value| value.code == 7) {
                    false => Some(record_handle(db, b"STYLE", b"Standard")?),
                    true => Some(record_handle(db, b"STYLE", text_of(object, 7)?)?),
                }
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
        Some(NewEntity {
            object,
            handle,
            type_code,
            mode,
            owner,
            layer,
            ltype,
            pointed,
            children: None,
        })
    }

    /// 足した表のレコード `object` のハンドルと、R2000 のレコードのデータ（決定 0061〜0067）。所有者（330）の
    /// 表のオブジェクトの項目の数を増やし、終わりに新しいレコードのハンドルを足したデータを `changes` に入れる。
    /// 画層・文字の体裁・線種・アプリケーションの名前（APPID）・寸法の体裁・ビュー・ビューポートの設定（VPORT）
    /// だけを扱い、名前（2）が表にない
    /// ことが要る。
    /// 画層は、印（70）・色（62）・線の太さ（370）を持ち、印刷スタイル（390）と線種（6）が表になければ
    /// ならない。印刷スタイルがなければ、画層 0 と同じものにする。文字の体裁は、型ごとの欄をみな持たなければ
    /// ならない。線種は、要素がみな形も文字も持たないことが要る。アプリケーションの名前は、印（70）が 0 で
    /// なければならない。寸法の体裁は、表にない欄を既定の値で書き、文字の体裁とブロックのハンドル（340〜344）
    /// が図面になければならない。ビューは、型ごとの欄をみな持ち、UCS のハンドル（345・346）が図面になければ
    /// ならない。ビューポートの設定は、型ごとの欄をみな持ち、UCS のハンドル（345・346）が図面になければ
    /// ならない。ハンドルが `seed` より小さいか、足せないレコードなら `None`
    fn add_record(
        &self,
        db: &AcadDb,
        object: &Object,
        records: &BTreeMap<u64, usize>,
        seed: u64,
        changes: &mut BTreeMap<usize, Vec<u8>>,
    ) -> Option<(u64, Vec<u8>)> {
        let dimstyle_codes: Vec<i16> = std::iter::once(70)
            .chain(DIM_FIELDS.iter().map(|field| field.0))
            .chain(340..=344)
            .collect();
        let view_codes = [VIEW, &[345, 346]].concat();
        let vport_codes = [VPORT, &[345, 346]].concat();
        // レコードと表のオブジェクトの型と、型ごとの欄のグループ・コード
        let (type_code, control_type, own): (i16, u16, &[i16]) = match object.kind.as_slice() {
            b"LAYER" => (LAYER_TYPE, LAYER_CONTROL_TYPE, &[70, 62, 290, 370, 390, 6]),
            b"STYLE" => (STYLE_TYPE, STYLE_CONTROL_TYPE, STYLE),
            b"LTYPE" => (LTYPE_TYPE, LTYPE_CONTROL_TYPE, &[70, 3, 72, 73, 40, 49, 74]),
            b"APPID" => (APPID_TYPE, APPID_CONTROL_TYPE, &[70]),
            b"DIMSTYLE" => (DIMSTYLE_TYPE, DIMSTYLE_CONTROL_TYPE, &dimstyle_codes),
            b"VIEW" => (VIEW_TYPE, VIEW_CONTROL_TYPE, &view_codes),
            b"VPORT" => (VPORT_TYPE, VPORT_CONTROL_TYPE, &vport_codes),
            _ => return None,
        };
        let handle = object.handle().filter(|&h| h >= seed)?;
        // DIMSTYLE のハンドルは、DXF と同じく 105 に置く
        let at = if type_code == DIMSTYLE_TYPE { 105 } else { 5 };
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
        let mut fields = BitWriter::default();
        fields.handle(0, handle);
        // 拡張データと反応子がない
        fields.bs(0);
        fields.bl(0);
        string(&mut fields, object, 2)?;
        // 参照される印・外部参照の番号・外部参照に依る印
        fields.bit(true);
        fields.bs(0);
        fields.bit(false);
        let control = object.owner()?;
        let mut handles = BitWriter::default();
        handles.handle(4, control);
        handles.handle(3, 0);
        // 外部参照のブロック
        handles.handle(5, 0);
        if type_code == LAYER_TYPE {
            let (flags, color) = layer_flags(object, false, 0)?;
            fields.bs(flags);
            fields.bs(color);
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
            handles.handle(5, record_handle(db, b"LTYPE", text_of(object, 6)?)?);
        } else if type_code == STYLE_TYPE {
            // 形のファイル 1 と縦書き 4 の印、高さ・幅の係数・斜めの角度・生成の印・前の高さ・字体の名前
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
        } else if type_code == LTYPE_TYPE {
            // 外部参照に依らない線種の、説明・揃え・要素の数・長さと、要素ごとの長さ。形も文字もない要素の
            // 形の番号・ずれ・尺度・回転は、試料の図面と同じく 0・0・0・1・0 とし、文字の領域は 0 で埋める
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
                // 形のファイルの文字の体裁
                handles.handle(5, 0);
            }
            values.0.next().is_none().then_some(())?;
            for _ in 0..256 {
                fields.rc(0);
            }
        } else if type_code == APPID_TYPE {
            // 使い道の分からない RC は、試料の図面と同じく 0 とする
            (small(object, 70)?? == 0).then_some(())?;
            fields.rc(0);
        } else if type_code == VIEW_TYPE {
            let associated = view_fields(&mut fields, object, false)?;
            // UCS を伴わないビューは、UCS の欄を持たない
            let ucs = [110, 120, 130, 111, 121, 131, 112, 122, 132, 79, 146];
            (associated || !object.values.iter().any(|value| ucs.contains(&value.code)))
                .then_some(())?;
            // UCS を伴うビューは、基準の UCS と名前の UCS を指す。試料の図面と同じく、参照の型は 5
            if associated {
                for code in [346, 345] {
                    handles.handle(5, ucs_handle(db, object, code)?);
                }
            } else {
                (ucs_handle(db, object, 346)? == 0 && ucs_handle(db, object, 345)? == 0)
                    .then_some(())?;
            }
        } else if type_code == VPORT_TYPE {
            // 使い道の分からないビットは、試料の図面と同じく偽とする
            vport_fields(&mut fields, object, false, None, false)?;
            // 名前の UCS と基準の UCS。試料の図面と同じく、参照の型は 5
            for code in [345, 346] {
                handles.handle(5, ucs_handle(db, object, code)?);
            }
        } else {
            (small(object, 70)?? == 0).then_some(())?;
            for &(code, field) in &DIM_FIELDS {
                dim_field(&mut fields, object, code, field)?;
            }
            // 文字の体裁と、引出線・矢印・1 つめと 2 つめの矢印のブロック。試料の図面と同じく、参照の型は 5
            for code in 340..=344 {
                let handle = match object.values.iter().find(|value| value.code == code) {
                    Some(value) => match value.data {
                        Data::Handle(h) => db.get(h).map(|_| h)?,
                        _ => return None,
                    },
                    None => 0,
                };
                handles.handle(5, handle);
            }
        }
        let record = *records.get(&control)?;
        (self.entries.get(record)?.type_code == control_type).then_some(())?;
        changes.insert(
            record,
            add_entry(
                &self.current(changes, record)?,
                handle,
                type_code == DIMSTYLE_TYPE,
            )?,
        );
        let mut out = BitWriter::default();
        out.bs(type_code);
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

    /// 見出しの変数の HANDSEED。表に足す図形には、これより小さくないハンドルを付ける（決定 0052）。
    /// 読めなければ `None`
    pub fn handle_seed(&self) -> Option<u64> {
        let (src, span) = self.header_section()?;
        Some(header::handle_seed(src, span.start)?.value)
    }

    /// 除いた図形 `removed` の前後の図形と、所有者のブロックのレコードの、図形の並びをたどるハンドルを直した
    /// データを、`changes` に足す（決定 0051）。図形を持つ図形（ATTRIB を持つ INSERT など）と、反応子か
    /// 拡張辞書を持つ図形と、[`REMOVABLE`] にない種類は除けない
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
        let owners: BTreeSet<u64> = base.objects().iter().filter_map(Object::owner).collect();
        let mut gone = BTreeSet::new();
        let mut blocks = BTreeMap::new();
        for &(record, old) in removed {
            let block = self
                .entries
                .get(record)
                .filter(|entry| {
                    matches!(old.section, Section::Entities | Section::Blocks)
                        && REMOVABLE.contains(&old.kind.as_slice())
                        && !owners.contains(&entry.handle)
                })
                .and_then(|entry| {
                    let links = entity_links(self.data(entry), entry.handle)?;
                    let owner = old.owner()?;
                    let block = self.entries.get(*records.get(&owner)?)?;
                    (links.reactors == 0
                        && links.xdic == 0
                        && self.kind(block.type_code)?.0 == b"BLOCK_RECORD")
                        .then_some((entry.handle, owner))
                })
                .ok_or_else(|| unsupported(&old.kind))?;
            gone.insert(block.0);
            *blocks.entry(block.1).or_insert(0usize) += 1;
        }
        for (block, count) in blocks {
            self.relink_block(block, count, &gone, &records, changes)
                .ok_or_else(|| unsupported(b"BLOCK_RECORD"))?;
        }
        Ok(())
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

/// 表から除ける図形の種類（決定 0051）
const REMOVABLE: &[&[u8]] = &[
    b"LINE",
    b"CIRCLE",
    b"ARC",
    b"POINT",
    b"ELLIPSE",
    b"RAY",
    b"XLINE",
    b"SOLID",
    b"TRACE",
    b"3DFACE",
    b"TEXT",
    b"MTEXT",
    b"ATTDEF",
    b"LWPOLYLINE",
    b"SPLINE",
    b"INSERT",
    b"SHAPE",
];

/// 表に足せる図形の種類と型と、型ごとの欄のグループ・コード（決定 0052〜0060）。型ごとの欄を元のデータ
/// なしで符号化でき、型ごとのハンドルが表のレコードか、足した図形を指すものだけか、ないもの
const ADDABLE: &[(&[u8], i16, &[i16])] = &[
    (b"LINE", 0x13, LINE),
    (b"CIRCLE", 0x12, CIRCLE),
    (b"ARC", 0x11, ARC),
    (b"POINT", 0x1B, POINT),
    (b"ELLIPSE", 0x23, ELLIPSE),
    (b"RAY", 0x28, RAY),
    (b"XLINE", 0x29, RAY),
    (b"SOLID", 0x1F, SOLID),
    (b"TRACE", 0x20, SOLID),
    (b"3DFACE", 0x1C, FACE3D),
    (b"TEXT", 0x01, TEXT),
    (b"ATTDEF", 0x03, ATTDEF),
    (b"INSERT", 0x07, INSERT),
    (b"MTEXT", 0x2C, MTEXT),
    (b"LWPOLYLINE", 0x4D, LWPOLYLINE),
    (b"SPLINE", 0x24, SPLINE),
    (b"ATTRIB", 0x02, ATTRIB),
    (b"SEQEND", 0x06, &[]),
];

/// 足す図形を符号化するのに要るもの
struct NewEntity<'a> {
    object: &'a Object,
    handle: u64,
    type_code: i16,
    /// entmode。2 はモデル空間、1 は用紙空間、0 はほかのブロック
    mode: u8,
    /// 所有者のブロックのレコードのハンドル
    owner: u64,
    /// 画層のハンドル
    layer: u64,
    /// 線種の印と、印が 3 のときの線種のハンドル
    ltype: (u8, u64),
    /// 型ごとに指す表のレコード（TEXT・ATTDEF の文字の体裁、INSERT のブロック）のハンドル
    pointed: Option<u64>,
    /// INSERT が持つ最初と最後の ATTRIB と、SEQEND のハンドル
    children: Option<(u64, u64, u64)>,
}

impl NewEntity<'_> {
    /// 前後の図形のハンドルを `prev` と `next` にした、R2000 の図形のデータ。共通の欄は、色（62）が
    /// なければ ByLayer、線種の尺度（48）がなければ 1、非表示（60）がなければ 0 にし、反応子と拡張辞書と
    /// 図形の絵を持たず、前後の図形のハンドルを持ち、印刷スタイルと線の太さは ByLayer にする
    fn encode(&self, prev: u64, next: u64) -> Option<Vec<u8>> {
        let object = self.object;
        let mut common = BitWriter::default();
        common.handle(0, self.handle);
        // 拡張データがない
        common.bs(0);
        common.bit(false);
        common.bb(self.mode);
        common.bl(0);
        common.bit(false);
        common.bs(small(object, 62)?.unwrap_or(256));
        common.bd(real(object, 48)?.unwrap_or(1.0));
        common.bb(self.ltype.0);
        common.bb(0);
        common.bs(small(object, 60)?.unwrap_or(0));
        common.rc(lineweight_index(-1)?.try_into().ok()?);
        let fields = own_fields(object, object, self.type_code, &[], 0)?;
        let mut handles = BitWriter::default();
        if self.mode == 0 {
            handles.handle(4, self.owner);
        }
        handles.handle(3, 0);
        handles.handle(4, prev);
        handles.handle(4, next);
        handles.handle(5, self.layer);
        if self.ltype.0 == 3 {
            handles.handle(5, self.ltype.1);
        }
        if let Some(pointed) = self.pointed {
            handles.handle(5, pointed);
        }
        // INSERT は、ブロックに続けて、最初と最後の ATTRIB と SEQEND を指す
        if let Some((first, last, seqend)) = self.children {
            handles.handle(4, first);
            handles.handle(4, last);
            handles.handle(3, seqend);
        }
        let mut out = BitWriter::default();
        out.bs(self.type_code);
        out.rl(i32::try_from(out.len() + 32 + common.len() + fields.len()).ok()?);
        for part in [common, fields, handles] {
            let len = part.len();
            out.copy(&part.into_bytes(), 0..len)?;
        }
        Some(out.into_bytes())
    }
}

/// 種類 `kind` の表のレコードのうち、名前（2）が `name` のもののハンドル。大文字と小文字は区別しない
fn record_handle(db: &AcadDb, kind: &[u8], name: &[u8]) -> Option<u64> {
    db.objects()
        .iter()
        .filter(|o| o.kind == kind && o.section == Section::Tables)
        .find(|o| text_of(o, 2).is_some_and(|n| n.eq_ignore_ascii_case(name)))?
        .handle()
}

/// ハンドル `handle` の図形のデータ `data` の、前後の図形のハンドルを `prev` と `next` にしたもの。
/// 前後の図形のハンドルを持たない図形は、持つようにする。ハンドルの流れの位置は変わらない
fn relink(data: &[u8], handle: u64, prev: u64, next: u64) -> Option<Vec<u8>> {
    let links = entity_links(data, handle)?;
    let mut out = BitWriter::default();
    out.copy(data, 0..links.no_links_at)?;
    out.bit(false);
    out.copy(data, links.no_links_at + 1..links.at.start)?;
    out.handle(4, prev);
    out.handle(4, next);
    out.copy(data, links.at.end..data.len() * 8)?;
    Some(out.into_bytes())
}

/// ハンドル `handle` のブロックのレコードのデータ `data` の、最初と最後の図形のハンドルを、`first` と
/// `last` にしたもの
fn reblock(data: &[u8], handle: u64, first: u64, last: u64) -> Option<Vec<u8>> {
    let (at, _, _) = block_links(data, handle)?;
    let mut out = BitWriter::default();
    out.copy(data, 0..at.start)?;
    out.handle(4, first);
    out.handle(4, last);
    out.copy(data, at.end..data.len() * 8)?;
    Some(out.into_bytes())
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

/// 表のレコードの型ごとの欄を符号化し直したデータ（決定 0043〜0046）。LAYER・STYLE・LTYPE・VIEW・UCS・VPORT・DIMSTYLE だけを扱う。名前（2）などの
/// ほかの値が変わったものは `None`
fn encode_record(data: &[u8], old: &Object, new: &Object) -> Option<Vec<u8>> {
    // DIMSTYLE は、70 と DWG の並びの欄
    let dimstyle_codes: Vec<i16> = std::iter::once(70)
        .chain(DIM_FIELDS.iter().map(|field| field.0))
        .collect();
    let own: &[i16] = match new.kind.as_slice() {
        b"LAYER" => LAYER,
        b"STYLE" => STYLE,
        b"LTYPE" => LTYPE,
        b"VIEW" => VIEW,
        b"UCS" => UCS,
        b"VPORT" => VPORT,
        b"DIMSTYLE" => &dimstyle_codes,
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
    let end = match new.kind.as_slice() {
        b"LAYER" => layer(&mut fields, new, xdep, data, own_at)?,
        b"STYLE" => style(&mut fields, old, new, xdep, data, own_at)?,
        b"LTYPE" => ltype(&mut fields, old, new, xdep, data, own_at)?,
        b"VIEW" => view_record(&mut fields, new, xdep, data, own_at)?,
        b"UCS" => ucs_record(&mut fields, new, xdep, data, own_at)?,
        b"VPORT" => vport_record(&mut fields, old, new, xdep, data, own_at)?,
        _ => dimstyle(&mut fields, old, new, xdep, data, own_at)?,
    };
    fields.copy(data, end..handles_at)?;
    let mut out = BitWriter::default();
    out.copy(data, 0..size_at)?;
    out.rl(i32::try_from(start + fields.len()).ok()?);
    out.copy(data, size_at + 32..start)?;
    let len = fields.len();
    out.copy(&fields.into_bytes(), 0..len)?;
    out.copy(data, handles_at..data.len() * 8)?;
    Some(out.into_bytes())
}

/// OBJECTS のオブジェクトの型ごとの欄を符号化し直したデータ（決定 0047〜0050）。GROUP・DICTIONARYVAR・
/// DICTIONARY・ACDBDICTIONARYWDFLT・MLINESTYLE・LAYOUT・XRECORD だけを扱う。ハンドルの並びから作る値などのほかの値が
/// 変わったものは `None`
fn encode_object(data: &[u8], old: &Object, new: &Object) -> Option<Vec<u8>> {
    let own: &[i16] = match new.kind.as_slice() {
        b"GROUP" => &[300, 70, 71],
        b"DICTIONARYVAR" => &[280, 1],
        b"DICTIONARY" | b"ACDBDICTIONARYWDFLT" => &[280, 281, 3],
        b"MLINESTYLE" => &[2, 70, 3, 62, 51, 52, 71, 49],
        b"LAYOUT" => return encode_layout(data, old, new),
        b"XRECORD" => return encode_xrecord(data, old, new),
        _ => return None,
    };
    // 辞書の項目の数と、多重線の体裁の線の数と名前は変えない
    let codes = |object: &Object| -> Vec<i16> { object.values.iter().map(|v| v.code).collect() };
    if new.kind != b"GROUP" && new.kind != b"DICTIONARYVAR" && codes(old) != codes(new) {
        return None;
    }
    if new.kind == b"MLINESTYLE" && text_of(old, 2) != text_of(new, 2) {
        return None;
    }
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
    let mut bits = Bits::with_release(data, Release::R2000);
    bits.seek(start);
    let mut fields = BitWriter::default();
    if new.kind == b"GROUP" {
        // 説明・名前のない印・選べる印。図形の数は元のまま写す
        bits.text()?;
        bits.bs()?;
        bits.bs()?;
        string(&mut fields, new, 300)?;
        for code in [70, 71] {
            fields.bs(i16::from(flag(new, code)?));
        }
    } else if new.kind == b"DICTIONARYVAR" {
        // 型と値
        bits.rc()?;
        bits.text()?;
        fields.rc(u8::try_from(small(new, 280)??).ok()?);
        string(&mut fields, new, 1)?;
    } else if new.kind == b"MLINESTYLE" {
        mlinestyle(&mut fields, &mut bits, new, data)?;
    } else {
        dictionary(&mut fields, &mut bits, new, data)?;
    }
    let end = bits.pos();
    fields.copy(data, end..handles_at)?;
    let mut out = BitWriter::default();
    out.copy(data, 0..size_at)?;
    out.rl(i32::try_from(start + fields.len()).ok()?);
    out.copy(data, size_at + 32..start)?;
    let len = fields.len();
    out.copy(&fields.into_bytes(), 0..len)?;
    out.copy(data, handles_at..data.len() * 8)?;
    Some(out.into_bytes())
}

/// LAYER の欄（決定 0043）。DXF の 70 の凍結 1・新しいビューポートで凍結 2・ロック 4 と、負の色（62）の
/// 表示しない印、印刷しない印（290）、線の太さ（370）から、DWG の印（BS）を作る。印のほかのビットは元のまま
/// 写す。外部参照に依る印（70 の 16）は、元のものと同じでなければ受け付けない。元の欄の終わりの位置を返す
fn layer(
    fields: &mut BitWriter,
    object: &Object,
    xdep: bool,
    data: &[u8],
    own_at: usize,
) -> Option<usize> {
    let mut bits = Bits::with_release(data, Release::R2000);
    bits.seek(own_at);
    let old = bits.bs()?;
    bits.bs()?;
    let (flags, color) = layer_flags(object, xdep, old)?;
    fields.bs(flags);
    fields.bs(color);
    Some(bits.pos())
}

/// LAYER の DWG の印（BS）と色の番号。印のほかのビットは `old` から写す（決定 0043）
fn layer_flags(object: &Object, xdep: bool, old: i16) -> Option<(i16, i16)> {
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
    let color = color.checked_abs()?;
    (1..=255).contains(&color).then_some((flags, color))
}

/// 表のオブジェクトのデータ `data` の、項目の数（BL）を 1 増やし、項目のハンドルの終わりに `entry` を
/// 足したもの（決定 0061）。ハンドルの流れの後の埋めるビットは写さない
fn add_entry(data: &[u8], entry: u64, more: bool) -> Option<Vec<u8>> {
    let [size_at, start, handles_at] = object_fields(data)?;
    let mut bits = Bits::with_release(data, Release::R2000);
    bits.seek(size_at + 32);
    bits.handle()?;
    skip_extended_data(&mut bits)?;
    let reactors = bits.bl()?;
    let count = bits.bl()?;
    let mut fields = BitWriter::default();
    fields.bl(count.checked_add(1)?);
    // 寸法の体裁の表は、項目の数の後に RC を持ち、項目の後にその数だけハンドルが続く
    let extra = if more { bits.rc()? } else { 0 };
    if more {
        fields.rc(extra);
    }
    (bits.pos() == handles_at).then_some(())?;
    let mut handles = BitWriter::default();
    // 所有者・反応子・拡張辞書・項目
    for _ in 0..(1 + i64::from(reactors) + 1 + i64::from(count)) {
        let found = bits.handle()?;
        handles.handle(found.code, found.value);
    }
    handles.handle(2, entry);
    for _ in 0..extra {
        let found = bits.handle()?;
        handles.handle(found.code, found.value);
    }
    let mut out = BitWriter::default();
    out.copy(data, 0..size_at)?;
    out.rl(i32::try_from(start + fields.len()).ok()?);
    out.copy(data, size_at + 32..start)?;
    for part in [fields, handles] {
        let len = part.len();
        out.copy(&part.into_bytes(), 0..len)?;
    }
    Some(out.into_bytes())
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
/// 元のビットを写す。元の欄の終わりの位置を返す
fn style(
    fields: &mut BitWriter,
    old: &Object,
    new: &Object,
    xdep: bool,
    data: &[u8],
    own_at: usize,
) -> Option<usize> {
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
    Some(bits.pos())
}

/// LTYPE の欄（決定 0044）。要素の数（73）と、要素ごとの形と文字の印（74 の 2・4）は、元のものと同じでなければ
/// 受け付けない。要素の印の 7 より上のビットと、形も文字もない要素の形の番号・ずれ・尺度・回転は、元のまま写す。
/// 説明（3）と文字の要素の文字列（9）が変わらなければ元のビットを写し、文字列が変われば、256 バイトの領域に
/// 初めから順に詰め直す。元の欄の終わりの位置を返す
fn ltype(
    fields: &mut BitWriter,
    old: &Object,
    new: &Object,
    xdep: bool,
    data: &[u8],
    own_at: usize,
) -> Option<usize> {
    let mut bits = Bits::with_release(data, Release::R2000);
    bits.seek(own_at);
    bits.text()?;
    let description = own_at..bits.pos();
    bits.bd()?;
    bits.rc()?;
    let count = bits.rc()?;
    let mut dashes = Vec::new();
    for _ in 0..count {
        bits.bd()?;
        let shape = bits.bs()?;
        let (x, y) = (bits.rd()?, bits.rd()?);
        let (scale, rotation) = (bits.bd()?, bits.bd()?);
        dashes.push((shape, x, y, scale, rotation, bits.bs()?));
    }
    let area_at = bits.pos();
    bits.skip_bytes(256)?;
    let texts = |object: &Object| -> Vec<_> {
        object
            .values
            .iter()
            .filter(|value| value.code == 9)
            .cloned()
            .collect()
    };
    let same_texts = texts(old) == texts(new);
    let mut values = Cursor(
        new.values
            .iter()
            .filter(|value| LTYPE.contains(&value.code))
            .collect::<Vec<_>>()
            .into_iter()
            .peekable(),
    );
    (values.int(70)? == if xdep { 16 } else { 0 }).then_some(())?;
    match values.next(3)? {
        Data::Text(_) if text_of(old, 3) == text_of(new, 3) => {
            fields.copy(data, description)?;
        }
        Data::Text(_) => string(fields, new, 3)?,
        _ => return None,
    }
    let align = u8::try_from(values.int(72)?).ok()?;
    (values.int(73)? == i32::from(count)).then_some(())?;
    fields.bd(values.real(40)?);
    fields.rc(align);
    fields.rc(count);
    let mut area = Vec::new();
    for (shape, x, y, scale, rotation, flags) in dashes {
        fields.bd(values.real(49)?);
        let dxf = i16::try_from(values.int(74)?).ok()?;
        (dxf & !7 == 0 && dxf & 6 == flags & 6).then_some(())?;
        if flags & 6 == 0 {
            fields.bs(shape);
            fields.rd(x);
            fields.rd(y);
            fields.bd(scale);
            fields.bd(rotation);
        } else {
            let number = i16::try_from(values.int(75)?).ok()?;
            let (scale, rotation) = (values.real(46)?, values.real(50)?.to_radians());
            let (x, y) = (values.real(44)?, values.real(45)?);
            // 文字の要素の 75 は 0 と決まっている
            let shape = if flags & 2 == 0 {
                number
            } else if number != 0 {
                return None;
            } else if same_texts {
                values.next(9)?;
                shape
            } else {
                let Data::Text(text) = values.next(9)? else {
                    return None;
                };
                let at = i16::try_from(area.len()).ok()?;
                area.extend(uncaret(text)?);
                area.push(0);
                at
            };
            fields.bs(shape);
            fields.rd(x);
            fields.rd(y);
            fields.bd(scale);
            fields.bd(rotation);
        }
        fields.bs(flags & !7 | dxf);
    }
    values.0.next().is_none().then_some(())?;
    if same_texts {
        fields.copy(data, area_at..bits.pos())?;
    } else {
        (area.len() <= 256).then_some(())?;
        area.resize(256, 0);
        for byte in area {
            fields.rc(byte);
        }
    }
    Some(bits.pos())
}

/// DIMSTYLE の欄（決定 0046）。値が変わらない欄は元のビットを写し、変わった欄は書き直す。表にない欄は、
/// 読む側と同じ既定の値で書く。色は R2000 の CMC の番号（BS）で書く。元の欄の終わりの位置を返す
fn dimstyle(
    fields: &mut BitWriter,
    old: &Object,
    new: &Object,
    xdep: bool,
    data: &[u8],
    own_at: usize,
) -> Option<usize> {
    record_flags(new, xdep, 0)?;
    let of = |object: &Object, code: i16| -> Vec<_> {
        object
            .values
            .iter()
            .filter(|value| value.code == code)
            .cloned()
            .collect()
    };
    let mut bits = Bits::with_release(data, Release::R2000);
    bits.seek(own_at);
    for &(code, field) in &DIM_FIELDS {
        let at = bits.pos();
        match field {
            Dim::Text => bits.text().map(drop)?,
            Dim::Real(_) => bits.bd().map(drop)?,
            Dim::Bit(_) => bits.bit().map(drop)?,
            Dim::Short(_) | Dim::Color(_) => bits.bs().map(drop)?,
        }
        if of(old, code) == of(new, code) {
            fields.copy(data, at..bits.pos())?;
            continue;
        }
        dim_field(fields, new, code, field)?;
    }
    Some(bits.pos())
}

/// DIMSTYLE の `code` の欄を、`new` の値で書く。値がなければ、読む側と同じ既定の値で書く
fn dim_field(fields: &mut BitWriter, new: &Object, code: i16, field: Dim) -> Option<()> {
    match field {
        Dim::Text if text_of(new, code).is_some() => string(fields, new, code)?,
        Dim::Text => {
            new.values
                .iter()
                .all(|value| value.code != code)
                .then_some(())?;
            fields.text(&[0])?;
        }
        Dim::Real(default) => fields.bd(real(new, code)?.unwrap_or(default)),
        Dim::Bit(default) => match small(new, code)? {
            None => fields.bit(default),
            Some(0) => fields.bit(false),
            Some(1) => fields.bit(true),
            Some(_) => return None,
        },
        Dim::Short(default) | Dim::Color(default) => {
            fields.bs(small(new, code)?.unwrap_or(default));
        }
    }
    Some(())
}

/// LAYOUT の欄の形。読む側は、すべての欄を DWG の並びで置く
#[derive(Clone, Copy)]
enum Field {
    Text,
    Short,
    Bd,
    Rd,
}

/// LAYOUT の欄の形の並び（印刷の設定の 28 の欄と、レイアウトの 26 の欄）
const LAYOUT_FIELDS: [Field; 54] = {
    use Field::{Bd, Rd, Short, Text};
    [
        Text, Text, Short, Bd, Bd, Bd, Bd, Bd, Bd, Text, Bd, Bd, Short, Short, Short, Bd, Bd, Bd,
        Bd, Text, Bd, Bd, Text, Short, Bd, Bd, Bd, Text, Short, Short, Bd, Bd, Bd, Rd, Rd, Rd, Rd,
        Bd, Bd, Bd, Bd, Bd, Bd, Bd, Bd, Bd, Bd, Short, Bd, Bd, Bd, Bd, Bd, Bd,
    ]
};

/// LAYOUT を符号化し直したデータ（決定 0049）。値の番号の並びが元と同じで、レイアウトの名前（2 つめの 1）
/// とハンドルの並びから作る値が変わらないものだけを扱う。欄は DWG の並びのまま、すべて書き直す
fn encode_layout(data: &[u8], old: &Object, new: &Object) -> Option<Vec<u8>> {
    let codes = |object: &Object| -> Vec<i16> { object.values.iter().map(|v| v.code).collect() };
    if codes(old) != codes(new) {
        return None;
    }
    // 欄は、ハンドル（5）と所有者（330）の後に、ハンドルの並びから作る値の前に並ぶ
    let at = new.values.iter().position(|value| value.code == 1)?;
    let range = at..at + LAYOUT_FIELDS.len();
    let own = new.values.get(range.clone())?;
    let rest = |object: &Object| -> Option<Vec<Value>> {
        let mut values = object.values.clone();
        values.drain(range.clone());
        // レイアウトの名前
        values.push(object.values.get(at + 27)?.clone());
        Some(values)
    };
    if rest(old)? != rest(new)? {
        return None;
    }
    let [size_at, start, handles_at] = object_fields(data)?;
    let mut bits = Bits::with_release(data, Release::R2000);
    bits.seek(start);
    let mut fields = BitWriter::default();
    for (field, value) in LAYOUT_FIELDS.iter().zip(own) {
        match (field, &value.data) {
            (Field::Text, Data::Text(text)) => {
                bits.text()?;
                let mut text = uncaret(text)?;
                text.push(0);
                fields.text(&text)?;
            }
            (Field::Short, Data::Int(v)) => {
                bits.bs()?;
                fields.bs(i16::try_from(*v).ok()?);
            }
            (Field::Bd, Data::Real(v)) => {
                bits.bd()?;
                fields.bd(*v);
            }
            (Field::Rd, Data::Real(v)) => {
                bits.rd()?;
                fields.rd(*v);
            }
            _ => return None,
        }
    }
    let end = bits.pos();
    fields.copy(data, end..handles_at)?;
    let mut out = BitWriter::default();
    out.copy(data, 0..size_at)?;
    out.rl(i32::try_from(start + fields.len()).ok()?);
    out.copy(data, size_at + 32..start)?;
    let len = fields.len();
    out.copy(&fields.into_bytes(), 0..len)?;
    out.copy(data, handles_at..data.len() * 8)?;
    Some(out.into_bytes())
}

/// XRECORD を符号化し直したデータ（決定 0050）。値の番号の並びが元と同じものだけを扱う。データは、
/// 元のデータを読みながら値を書き直し、文字列のコード・ページのバイトは元の値のものを写す
fn encode_xrecord(data: &[u8], old: &Object, new: &Object) -> Option<Vec<u8>> {
    let codes = |object: &Object| -> Vec<i16> { object.values.iter().map(|v| v.code).collect() };
    if codes(old) != codes(new) {
        return None;
    }
    // 複製の扱い（280）の前は、ハンドル（5）と所有者（330）
    let at = new.values.iter().position(|value| value.code == 280)?;
    if old.values.get(..at) != new.values.get(..at) {
        return None;
    }
    let Data::Int(cloning) = new.values.get(at)?.data else {
        return None;
    };
    let mut values = new.values.get(at + 1..)?.iter();
    let mut next = |code: i16| -> Option<&Data> {
        let value = values.next()?;
        (value.code == code).then_some(&value.data)
    };
    let [size_at, start, handles_at] = object_fields(data)?;
    let mut bits = Bits::with_release(data, Release::R2000);
    bits.seek(start);
    let len = usize::try_from(bits.bl()?).ok()?;
    let end = bits.pos().checked_add(len.checked_mul(8)?)?;
    let mut items = BitWriter::default();
    while bits.pos() < end {
        let code = bits.rs()?;
        items.rs(code);
        match DataType::of(code) {
            DataType::Real if matches!(code, 10..=19 | 1010..=1019) => {
                for code in [code, code + 10, code + 20] {
                    bits.rd()?;
                    let &Data::Real(v) = next(code)? else {
                        return None;
                    };
                    items.rd(v);
                }
            }
            DataType::Real => {
                bits.rd()?;
                let &Data::Real(v) = next(code)? else {
                    return None;
                };
                items.rd(v);
            }
            DataType::Int => {
                let &Data::Int(v) = next(code)? else {
                    return None;
                };
                match code {
                    280..=289 => {
                        bits.rc()?;
                        items.rc(u8::try_from(v).ok()?);
                    }
                    90..=99 | 420..=429 | 440..=459 | 1071 => {
                        bits.rl()?;
                        items.rl(i32::try_from(v).ok()?);
                    }
                    160..=169 => {
                        bits.skip_bytes(8)?;
                        items.raw(&v.to_le_bytes());
                    }
                    _ => {
                        bits.rs()?;
                        items.rs(i16::try_from(v).ok()?);
                    }
                }
            }
            DataType::Bool => {
                bits.rc()?;
                let &Data::Bool(v) = next(code)? else {
                    return None;
                };
                items.rc(u8::from(v));
            }
            DataType::Handle => {
                bits.skip_bytes(8)?;
                let &Data::Handle(v) = next(code)? else {
                    return None;
                };
                items.raw(&v.to_le_bytes());
            }
            DataType::Binary => {
                let len = usize::from(bits.rc()?);
                bits.skip_bytes(len)?;
                let Data::Binary(v) = next(code)? else {
                    return None;
                };
                items.rc(u8::try_from(v.len()).ok()?);
                items.raw(v);
            }
            DataType::Text => {
                let len = usize::try_from(bits.rs()?).ok()?;
                let code_page = bits.rc()?;
                bits.skip_bytes(len)?;
                let Data::Text(v) = next(code)? else {
                    return None;
                };
                let text = uncaret(v)?;
                items.rs(i16::try_from(text.len()).ok()?);
                items.rc(code_page);
                items.raw(&text);
            }
        }
    }
    if bits.pos() != end || values.next().is_some() {
        return None;
    }
    bits.bs()?;
    let mut fields = BitWriter::default();
    fields.bl(i32::try_from(items.len() / 8).ok()?);
    let len = items.len();
    fields.copy(&items.into_bytes(), 0..len)?;
    fields.bs(i16::try_from(cloning).ok()?);
    let end = bits.pos();
    fields.copy(data, end..handles_at)?;
    let mut out = BitWriter::default();
    out.copy(data, 0..size_at)?;
    out.rl(i32::try_from(start + fields.len()).ok()?);
    out.copy(data, size_at + 32..start)?;
    let len = fields.len();
    out.copy(&fields.into_bytes(), 0..len)?;
    out.copy(data, handles_at..data.len() * 8)?;
    Some(out.into_bytes())
}

/// DICTIONARY の欄（決定 0048）。項目の数は元のまま写し、複製の扱い（281）・所有する印（280）・項目の
/// 名前（3）を書く。読む側は 0 の所有する印を置かないので、置いた印は 1〜255 だけを受け付ける
fn dictionary(fields: &mut BitWriter, bits: &mut Bits, new: &Object, data: &[u8]) -> Option<()> {
    let start = bits.pos();
    bits.bl()?;
    fields.copy(data, start..bits.pos())?;
    bits.bs()?;
    bits.rc()?;
    fields.bs(small(new, 281)??);
    let owner = small(new, 280)?.map_or(Some(0), |owner| {
        u8::try_from(owner).ok().filter(|o| *o != 0)
    })?;
    fields.rc(owner);
    for value in new.values.iter().filter(|value| value.code == 3) {
        bits.text()?;
        let Data::Text(name) = &value.data else {
            return None;
        };
        let mut name = uncaret(name)?;
        name.push(0);
        fields.text(&name)?;
    }
    Some(())
}

/// MLINESTYLE の欄（決定 0048）。名前は元のまま写す。印（70）は DXF のビットを DWG のビットに並べ直し、
/// DXF にない DWG のビットは元のまま写す。角度は度からラジアンに直す。線の線種の番号は元のまま写す
fn mlinestyle(fields: &mut BitWriter, bits: &mut Bits, new: &Object, data: &[u8]) -> Option<()> {
    // DWG のビットと、DXF のビット（読む側と同じ）
    const MOVED: [(i16, i16); 8] = [
        (1, 2),
        (2, 1),
        (16, 16),
        (32, 64),
        (64, 32),
        (256, 256),
        (512, 1024),
        (1024, 512),
    ];
    let start = bits.pos();
    bits.text()?;
    fields.copy(data, start..bits.pos())?;
    bits.text()?;
    let flags = bits.bs()?;
    bits.bs()?;
    for _ in 0..2 {
        bits.bd()?;
    }
    let lines = bits.rc()?;
    // 名前の後の、型ごとの値の並び（印・説明・色・2 つの角度・線の数・線ごとの位置と色）
    let at = new.values.iter().position(|value| value.code == 2)?;
    let own = new.values.get(at + 1..)?;
    let int = |value: &Value| match value.data {
        Data::Int(value) => i16::try_from(value).ok(),
        _ => None,
    };
    let float = |value: &Value| match value.data {
        Data::Real(value) => Some(value),
        _ => None,
    };
    let [dxf, about, color, begin, end, count, rest @ ..] = own else {
        return None;
    };
    let dxf = int(dxf)?;
    if dxf & !MOVED.iter().fold(0, |sum, (_, bit)| sum | bit) != 0
        || int(count)? != i16::from(lines)
    {
        return None;
    }
    let mut dwg = MOVED.iter().fold(flags, |sum, (bit, _)| sum & !bit);
    for (bit, dxf_bit) in MOVED {
        if dxf & dxf_bit != 0 {
            dwg |= bit;
        }
    }
    fields.text(&{
        let Data::Text(about) = &about.data else {
            return None;
        };
        let mut about = uncaret(about)?;
        about.push(0);
        about
    })?;
    fields.bs(dwg);
    fields.bs(int(color)?);
    fields.bd(float(begin)?.to_radians());
    fields.bd(float(end)?.to_radians());
    fields.rc(lines);
    for line in rest.chunks(2) {
        let [offset, color] = line else {
            return None;
        };
        bits.bd()?;
        bits.bs()?;
        let ltype = bits.pos();
        bits.bs()?;
        fields.bd(float(offset)?);
        fields.bs(int(color)?);
        fields.copy(data, ltype..bits.pos())?;
    }
    Some(())
}

/// 外部参照に依る印（70 の 16）が元のものと同じで、ほかに `allowed` のビットだけを持つ 70
fn record_flags(object: &Object, xdep: bool, allowed: i16) -> Option<i16> {
    let flags = small(object, 70)??;
    (flags & !(allowed | 16) == 0 && (flags & 16 != 0) == xdep).then_some(flags)
}

/// 0 か 1 の整数の値
fn flag(object: &Object, code: i16) -> Option<bool> {
    match small(object, code)?? {
        0 => Some(false),
        1 => Some(true),
        _ => None,
    }
}

/// 2RD の点（`code`・`code + 10`）
fn point2(fields: &mut BitWriter, object: &Object, code: i16) -> Option<()> {
    for code in [code, code + 10] {
        fields.rd(real(object, code)??);
    }
    Some(())
}

/// VIEW・VPORT の見え方の印（71）の、透視・前の切り取り・後ろの切り取りと、前の切り取りが目の位置にあること
/// （DXF の 16 の逆）の 4 つのビット
fn view_mode(fields: &mut BitWriter, mode: i16) {
    for flag in [1, 2, 4] {
        fields.bit(mode & flag != 0);
    }
    fields.bit(mode & 16 == 0);
}

/// UCS の原点と軸（110〜112）と、高さ（146）・正投影の種類（79）
fn ucs_axes(fields: &mut BitWriter, object: &Object) -> Option<()> {
    for code in [110, 111, 112] {
        point3(fields, object, code)?;
    }
    fields.bd(real(object, 146)??);
    fields.bs(small(object, 79)??);
    Some(())
}

/// VIEW の欄（決定 0045）。ねじれ（50）は度からラジアンに直し、紙空間の印は 70 の 1 から書く。UCS を伴う印
/// （72）は、ハンドルの流れの数が変わるので、元のものと同じでなければ受け付けない。元の欄の終わりの位置を返す
fn view_record(
    fields: &mut BitWriter,
    new: &Object,
    xdep: bool,
    data: &[u8],
    own_at: usize,
) -> Option<usize> {
    let mut bits = Bits::with_release(data, Release::R2000);
    bits.seek(own_at);
    let old = super::objects::view(&mut bits, 0, &mut Vec::new())?;
    let was = old.iter().find(|value| value.code == 72)?;
    (was.data == Data::Int(i64::from(flag(new, 72)?))).then_some(())?;
    view_fields(fields, new, xdep)?;
    Some(bits.pos())
}

/// 表のレコードの `code` の UCS のハンドル。なければ 0。図面にある UCS を指さなければ `None`
fn ucs_handle(db: &AcadDb, object: &Object, code: i16) -> Option<u64> {
    match object.values.iter().find(|value| value.code == code) {
        Some(value) => match value.data {
            Data::Handle(h) => (db.get(h)?.kind == b"UCS").then_some(h),
            _ => None,
        },
        None => Some(0),
    }
}

/// VIEW の型ごとの欄。UCS を伴うかを返す
fn view_fields(fields: &mut BitWriter, new: &Object, xdep: bool) -> Option<bool> {
    let associated = flag(new, 72)?;
    let paper = record_flags(new, xdep, 1)? & 1 != 0;
    let mode = small(new, 71)??;
    (mode & !(1 | 2 | 4 | 16) == 0).then_some(())?;
    fields.bd(real(new, 40)??);
    fields.bd(real(new, 41)??);
    point2(fields, new, 10)?;
    point3(fields, new, 12)?;
    point3(fields, new, 11)?;
    fields.bd(real(new, 50)??.to_radians());
    for code in [42, 43, 44] {
        fields.bd(real(new, code)??);
    }
    view_mode(fields, mode);
    fields.rc(u8::try_from(small(new, 281)??).ok()?);
    fields.bit(paper);
    fields.bit(associated);
    if associated {
        ucs_axes(fields, new)?;
    }
    Some(associated)
}

/// UCS の欄（決定 0045）。後に続く正投影の型は、表にないので元のまま写す。元の欄の終わりの位置を返す
fn ucs_record(
    fields: &mut BitWriter,
    new: &Object,
    xdep: bool,
    data: &[u8],
    own_at: usize,
) -> Option<usize> {
    let mut bits = Bits::with_release(data, Release::R2000);
    bits.seek(own_at);
    for _ in 0..10 {
        bits.bd()?;
    }
    bits.bs()?;
    record_flags(new, xdep, 0)?;
    for code in [10, 11, 12] {
        point3(fields, new, code)?;
    }
    fields.bd(real(new, 146)??);
    fields.bs(small(new, 79)??);
    Some(bits.pos())
}

/// VPORT の欄（決定 0045）。幅は、高さ（40）と比（41）の積で書く。高さと比が変わらなければ、元の幅を写す。
/// 角度（50・51）は度からラジアンに直し、UCS に従う印は 71 の 8 から書く。使い道の分からないビットは、元のまま
/// 写す。元の欄の終わりの位置を返す
fn vport_record(
    fields: &mut BitWriter,
    old: &Object,
    new: &Object,
    xdep: bool,
    data: &[u8],
    own_at: usize,
) -> Option<usize> {
    let mut bits = Bits::with_release(data, Release::R2000);
    bits.seek(own_at);
    bits.bd()?;
    let width = bits.bd()?;
    // 中心・注視点・向き・ねじれ・レンズ・切り取りから、スナップの基点と間隔まで
    bits.rd()?;
    bits.rd()?;
    for _ in 0..10 {
        bits.bd()?;
    }
    for _ in 0..4 {
        bits.bit()?;
    }
    bits.rc()?;
    for _ in 0..4 {
        bits.rd()?;
    }
    bits.bit()?;
    bits.bs()?;
    bits.bit()?;
    bits.bb()?;
    bits.bit()?;
    bits.rd()?;
    bits.rd()?;
    bits.bit()?;
    bits.bit()?;
    bits.bs()?;
    bits.bd()?;
    for _ in 0..4 {
        bits.rd()?;
    }
    let unknown = bits.bit()?;
    bits.seek(own_at);
    super::objects::vport(&mut bits)?;
    let same = real(old, 40)? == real(new, 40)? && real(old, 41)? == real(new, 41)?;
    vport_fields(fields, new, xdep, same.then_some(width), unknown)?;
    Some(bits.pos())
}

/// VPORT の型ごとの欄。幅は、`width` があればそれを、なければ高さ（40）と比（41）の積を書く。
/// `unknown` は、使い道の分からないビット
fn vport_fields(
    fields: &mut BitWriter,
    new: &Object,
    xdep: bool,
    width: Option<f64>,
    unknown: bool,
) -> Option<()> {
    record_flags(new, xdep, 0)?;
    let mode = small(new, 71)??;
    (mode & !(1 | 2 | 4 | 8 | 16) == 0).then_some(())?;
    let height = real(new, 40)??;
    let ratio = real(new, 41)??;
    let width = match width {
        Some(width) => width,
        None if height == 0.0 => ratio,
        None => ratio * height,
    };
    fields.bd(height);
    fields.bd(width);
    point2(fields, new, 12)?;
    point3(fields, new, 17)?;
    point3(fields, new, 16)?;
    fields.bd(real(new, 51)??.to_radians());
    for code in [42, 43, 44] {
        fields.bd(real(new, code)??);
    }
    view_mode(fields, mode);
    fields.rc(u8::try_from(small(new, 281)??).ok()?);
    point2(fields, new, 10)?;
    point2(fields, new, 11)?;
    fields.bit(mode & 8 != 0);
    fields.bs(small(new, 72)??);
    fields.bit(flag(new, 73)?);
    let icon = u8::try_from(small(new, 74)??).ok()?;
    (icon < 4).then_some(())?;
    fields.bb(icon);
    fields.bit(flag(new, 76)?);
    point2(fields, new, 15)?;
    fields.bit(flag(new, 75)?);
    fields.bit(flag(new, 77)?);
    fields.bs(small(new, 78)??);
    fields.bd(real(new, 50)??.to_radians());
    point2(fields, new, 13)?;
    point2(fields, new, 14)?;
    fields.bit(unknown);
    fields.bit(flag(new, 65)?);
    ucs_axes(fields, new)
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
        // SEQEND は型ごとの欄を持たない
        b"SEQEND" => {}
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
/// B は、元のデータから写す。足した MTEXT では、どちらも 0 にする（決定 0054）
fn mtext(fields: &mut BitWriter, object: &Object, data: &[u8], start: usize) -> Option<()> {
    // 足した MTEXT は元のデータを持たない（`data` が空）。範囲の大きさを 0、B を 0 にする
    let copied = match data.is_empty() {
        true => None,
        false => {
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
            Some((extents_at..extents_end, bits.pos()))
        }
    };
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
    match &copied {
        Some((extents, _)) => fields.copy(data, extents.clone())?,
        None => {
            fields.bd(0.0);
            fields.bd(0.0);
        }
    }
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
    match copied {
        Some((_, flag_at)) => fields.copy(data, flag_at..flag_at + 1),
        None => {
            fields.bit(false);
            Some(())
        }
    }
}

/// LWPOLYLINE の欄（R2000）。続く欄があるかの印は、値があるかから選ぶ。膨らみ（42）が 1 つでもあれば、
/// すべての頂点の膨らみを書く（ないものは 0）。頂点ごとの幅（40・41）は、初めの頂点から続けて置く。
/// 頂点ごとの幅があるときは、表にない一定の幅を元のまま残す。ほかの印のビットも元のまま写す。足した
/// LWPOLYLINE（`data` が空）では、どちらも 0 にする（決定 0056）
fn lwpolyline(fields: &mut BitWriter, object: &Object, data: &[u8], start: usize) -> Option<()> {
    let (old, old_width) = if data.is_empty() {
        (0, 0.0)
    } else {
        let mut bits = Bits::with_release(data, Release::R2000);
        bits.seek(start);
        let old = bits.bs()?;
        (old, if old & 4 != 0 { bits.bd()? } else { 0.0 })
    };
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
/// あるかの印は 41 があるかから選ぶ。接線（12・13）がなければ 0 を書く。足した SPLINE（`data` が空）は、
/// 当てはめる点（11）を持てば場合を 2、持たなければ 1 とし、閉じている・周期的の印を 0、有理の印を
/// 重みがあるかにする（決定 0057・0059）
fn spline(fields: &mut BitWriter, object: &Object, data: &[u8], start: usize) -> Option<()> {
    let added = data.is_empty();
    let (scenario, flags_at) = if added {
        let fitted = object.values.iter().any(|value| value.code == 11);
        (if fitted { 2 } else { 1 }, 0)
    } else {
        let mut bits = Bits::with_release(data, Release::R2000);
        bits.seek(start);
        let scenario = bits.bl()?;
        bits.bl()?;
        (scenario, bits.pos())
    };
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
            if added {
                for on in [weighted, false, false] {
                    fields.bit(on);
                }
            } else {
                fields.copy(data, flags_at..flags_at + 3)?;
            }
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
