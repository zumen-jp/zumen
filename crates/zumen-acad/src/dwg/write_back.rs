//! 共有のオブジェクトの表の変更を、DWG のオブジェクトに書き戻す（決定 0029）。R2000 と R2004 と、R2010 から後の図形・表のレコード・図形でないオブジェクトの変更と、図形の除きと足しを扱う（決定 0085〜0104）。
//! R2004 は、図形の値の変更のうち、共通の欄と、単純な種類の型ごとの欄だけを書き戻す（決定 0072）。属性を持つ INSERT と
//! MTEXT は、R2004 で後ろに増えた欄を元のまま写す（決定 0073）。HATCH は、初めのグラデーションの欄を写す（決定 0074）。
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
//! 表の終わりに足した画層・文字の体裁・線種・アプリケーションの名前・寸法の体裁・ビュー・ビューポートの設定・ブロックの
//! レコードは、一から符号化してオブジェクトの地図に加え、所有者の表のオブジェクトの項目の終わりに足す（決定 0061〜0068）。
//! 足したブロックのレコードは、同じく足した BLOCK・ENDBLK と図形を揃えて書く（決定 0068）。
//!
//! 見出しの変数の INSUNITS と、用紙空間の PLIMMIN・PLIMMAX の変更は、見出しの変数の区切りの中で書き直す
//! （決定 0069）。
//!
//! ほかのオブジェクトの追加と並べ替え、図形でないものの削除、ほかの値と、ほかの見出しの変数の変更は、まだ書き戻せない。

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;

use zumen_base::{Cx, Diagnostic, Diagnostics, Severity, codes};
use zumen_sdk::{Error, Transact, Transaction};

use super::bits::{BitWriter, Bits, Color, modular_char, push_modular_char};
use super::objects::{
    DIM_FIELDS, Dim, block_links, dim_fields, entity_fields, entity_links, entity_refs,
    hatch_gradient, object_fields, owned, r2010_entity_fields, r2010_object_fields,
    skip_extended_data,
};
use super::{DwgFile, Release, header};
use crate::db::{AcadDb, Data, DataType, Object, Origin, Section, Value};

/// 図形の共通の欄のうち、表に置くもののグループ・コード
const COMMON: &[i16] = &[62, 48, 60];

/// R2004 で、型ごとの欄を R2000 と同じに符号化する図形の種類（決定 0072・0073）
/// R2010 から後に書き戻す図形の種類（決定 0085〜0088）
const R2010_KINDS: &[&[u8]] = &[
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
    b"ATTRIB",
    b"ATTDEF",
    b"LWPOLYLINE",
    b"SPLINE",
    b"INSERT",
    b"MLINE",
    b"MTEXT",
    b"VERTEX",
    b"HATCH",
    b"DIMENSION",
    b"TOLERANCE",
    b"VIEWPORT",
    b"SHAPE",
];

const R2004_KINDS: &[&[u8]] = &[
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
    b"ATTRIB",
    b"ATTDEF",
    b"LWPOLYLINE",
    b"SPLINE",
    b"DIMENSION",
    b"TOLERANCE",
    b"LEADER",
    b"MLINE",
    b"SHAPE",
    b"VERTEX",
    b"INSERT",
    b"MTEXT",
    b"HATCH",
    b"VIEWPORT",
];

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
    70, 71, 72, 73, 74, 44, 12, 22, 32, 13, 23, 33, 42, 43, 40, 10, 20, 30, 41, 11, 21, 31,
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
const BLOCK_RECORD_TYPE: i16 = 0x31;
const BLOCK_RECORD_CONTROL_TYPE: u16 = 0x30;
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
const LEADER: &[i16] = &[
    73, 72, 40, 41, 74, 76, 10, 20, 30, 210, 220, 230, 211, 221, 231,
];
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
    /// 新しい INSUNITS と、用紙空間の PLIMMIN・PLIMMAX（決定 0069）
    units: Option<i16>,
    limits: Option<[f64; 4]>,
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
                }),
            Err(diagnostic) => Err(txn.reject(*diagnostic)),
        };
        // 失敗した変更があれば、確定が最初の失敗を診断として返し、何も反映しない
        let committed = txn.commit(cx);
        staged.and(committed)
    }

    /// 表 `db` を書き戻す変更
    fn changes(&self, db: &AcadDb, base: &AcadDb) -> Result<Changes, Box<Diagnostic>> {
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
                let data = self
                    .entries
                    .get(record)
                    .and_then(|entry| {
                        let data = self.data(entry);
                        // BLOCK と ENDBLK も、DWG では図形として書かれる。R2010 から後は、図形だけを扱う
                        match self.kind(entry.type_code)?.1 {
                            Section::Entities | Section::Blocks if r2010 => {
                                encode_r2010(data, entry.handles?, old, object, self.release)
                            }
                            Section::Tables if r2010 => {
                                encode_record_r2010(data, entry.handles?, old, object, self.release)
                            }
                            Section::Objects if r2010 => {
                                encode_object_r2010(data, entry.handles?, old, object, self.release)
                            }
                            _ if r2010 => None,
                            Section::Entities | Section::Blocks => {
                                encode(data, old, object, self.release)
                            }
                            Section::Tables => encode_record(data, old, object, self.release),
                            Section::Objects => encode_object(data, old, object, self.release),
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
        // 変えられるのは、このファイルの見出しの変数の区切りから読んだ変数だけ
        let (mut units, mut limits) = (None, None);
        for name in names
            .into_iter()
            .filter(|n| base.variable(n) != db.variable(n))
        {
            if base.variable(name).is_none() {
                return Err(unsupported(name));
            }
            match name {
                b"$INSUNITS" => {
                    // 区切りの長さを変えないよう、BS の幅が変わる値は受け付けない
                    let new = header_units(db)
                        .filter(|&new| header_units(base).map(bs_width) == Some(bs_width(new)))
                        .ok_or_else(|| unsupported(name))?;
                    units = Some(new);
                }
                b"$PLIMMIN" | b"$PLIMMAX" => {
                    limits = Some(paper_limits(db).ok_or_else(|| unsupported(name))?);
                }
                _ => return Err(unsupported(name)),
            }
        }
        // R2004 は、図形と表のレコードの、R2000 と同じく書けるものの追加だけを書き戻す（決定 0079〜0081）
        if r2004 && let Some(object) = added.iter().find(|object| !r2004_addable(object)) {
            return Err(unsupported(&object.kind));
        }
        // R2010 から後は、文字列の流れの形が R2000 と同じ図形の足しだけを書き戻す（決定 0092）
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
        // R2010 からは、MTEXT は枠の高さ（46）を、ATTRIB は属性の版と位置を固定するか（280）を持つ（決定 0093）
        let r2010: &[i16] = match object.kind.as_slice() {
            b"MTEXT" if self.release >= Release::R2010 => &[46],
            b"ATTRIB" if self.release >= Release::R2010 => &[280],
            _ => &[],
        };
        let codes = [&[5, 330, 8, 6, 67], COMMON, own, pointing, r2010].concat();
        // R2018 からの複数行の ATTRIB は、属性の値の後に、種類（71）・72・埋め込みのオブジェクトの印（101）と
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
    /// 画層・文字の体裁・線種・アプリケーションの名前（APPID）・寸法の体裁・ビュー・ビューポートの設定（VPORT）・
    /// ブロックのレコードだけを扱い、名前（2）が表にないことが要る。
    /// 画層は、印（70）・色（62）・線の太さ（370）を持ち、印刷スタイル（390）と線種（6）が表になければ
    /// ならない。印刷スタイルがなければ、画層 0 と同じものにする。文字の体裁は、型ごとの欄をみな持たなければ
    /// ならない。線種は、要素がみな形も文字も持たないことが要る。アプリケーションの名前は、印（70）が 0 で
    /// なければならない。寸法の体裁は、表にない欄を既定の値で書き、文字の体裁とブロックのハンドル（340〜344）
    /// が図面になければならない。ビューは、型ごとの欄をみな持ち、UCS のハンドル（345・346）が図面になければ
    /// ならない。ビューポートの設定は、型ごとの欄をみな持ち、UCS のハンドル（345・346）が図面になければ
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
        // R2010 からは、寸法の体裁は増えた欄と、寸法線と補助線の線種（345〜347）を持つ（決定 0095）
        let r2010_dimstyle = self.release >= Release::R2010;
        let dimstyle_codes: Vec<i16> = std::iter::once(70)
            .chain(dim_fields(self.release).iter().map(|field| field.0))
            .chain(340..=344)
            .chain((345..=347).filter(|_| r2010_dimstyle))
            .collect();
        let view_codes = [VIEW, &[345, 346]].concat();
        let vport_codes = [VPORT, &[345, 346]].concat();
        // レコードと表のオブジェクトの型と、型ごとの欄のグループ・コード
        let (type_code, control_type, own): (i16, u16, &[i16]) = match object.kind.as_slice() {
            b"LAYER" => (
                LAYER_TYPE,
                LAYER_CONTROL_TYPE,
                &[70, 62, 290, 370, 390, 347, 6],
            ),
            b"STYLE" => (STYLE_TYPE, STYLE_CONTROL_TYPE, STYLE),
            b"LTYPE" => (LTYPE_TYPE, LTYPE_CONTROL_TYPE, &[70, 3, 72, 73, 40, 49, 74]),
            b"APPID" => (APPID_TYPE, APPID_CONTROL_TYPE, &[70]),
            b"DIMSTYLE" => (DIMSTYLE_TYPE, DIMSTYLE_CONTROL_TYPE, &dimstyle_codes),
            b"VIEW" => (VIEW_TYPE, VIEW_CONTROL_TYPE, &view_codes),
            b"VPORT" => (VPORT_TYPE, VPORT_CONTROL_TYPE, &vport_codes),
            b"BLOCK_RECORD" => (BLOCK_RECORD_TYPE, BLOCK_RECORD_CONTROL_TYPE, &[]),
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
        // 名前のないブロックと空間のブロック（* で始まる）は足せない
        (type_code != BLOCK_RECORD_TYPE || !name.starts_with(b"*")).then_some(())?;
        let r2004 = self.release >= Release::R2004;
        // R2010 からは、文字列を文字列の流れに書き、外部参照の番号を持たない（決定 0094）
        let r2010 = self.release >= Release::R2010;
        let mut fields = match r2010 {
            true => BitWriter::with_strings(),
            false => BitWriter::default(),
        };
        fields.handle(0, handle);
        // 拡張データと反応子がない。R2004 からは、拡張辞書を持たない印を立てる。R2013 からは、DS のデータを
        // 持たない印が続く
        fields.bs(0);
        fields.bl(0);
        if r2004 {
            fields.bit(true);
        }
        if self.release >= Release::R2013 {
            fields.bit(false);
        }
        string(&mut fields, object, 2)?;
        // 参照される印・外部参照の番号・外部参照に依る印
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
        if type_code == LAYER_TYPE {
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
            // R2010 からは、文字の要素がなければ文字の領域を持たない（決定 0089）
            if !r2010 {
                for _ in 0..256 {
                    fields.rc(0);
                }
            }
        } else if type_code == APPID_TYPE {
            // 使い道の分からない RC は、試料の図面と同じく 0 とする
            (small(object, 70)?? == 0).then_some(())?;
            fields.rc(0);
        } else if type_code == VIEW_TYPE {
            let (lighting, bits) = default_lighting();
            let associated = view_fields(
                &mut fields,
                object,
                false,
                r2010.then_some((&lighting[..], 0..bits)),
            )?;
            // R2010 からは、カメラを印刷するかの B が続き、試料の図面と同じく偽とする。外部参照のブロックの
            // 後に、背景・表示スタイル・太陽を指す。表示スタイルは試料の図面と同じく 2dWireframe を指し、
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
            // R2010 からは、最後に断面の表示を指す
            if r2010 {
                handles.handle(4, 0);
            }
        } else if type_code == VPORT_TYPE {
            // 使い道の分からないビットは、試料の図面と同じく偽とする
            let (lighting, bits) = default_lighting();
            vport_fields(
                &mut fields,
                object,
                false,
                None,
                false,
                r2010.then_some((&lighting[..], 0..bits)),
            )?;
            // R2010 からは、格子の印と主の線の間隔（BS）が続く。試料の図面の多くと同じく 3 と 5 とする。外部参照の
            // ブロックの後に、背景・表示スタイル・太陽を指す。どれも持たないものとする（決定 0096）
            if r2010 {
                fields.bs(3);
                fields.bs(5);
                handles.handle(4, 0);
                handles.handle(5, 0);
                handles.handle(3, 0);
            }
            // 名前の UCS と基準の UCS。試料の図面と同じく、参照の型は 5
            for code in [345, 346] {
                handles.handle(5, ucs_handle(db, object, code)?);
            }
        } else if type_code == BLOCK_RECORD_TYPE {
            let block = block?;
            // 名前のない印・属性を持つ印・外部参照の印・重ねる外部参照の印・読み込んだ印と基点。属性を
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
            // R2010 からは、挿す単位（BS）・分解できるか（B）・尺度を揃えるか（RC）が続く。試料の図面の
            // 多くと同じく、単位なし・分解できる・揃えないとする（決定 0095）
            if r2010 {
                fields.bs(0);
                fields.bit(true);
                fields.rc(0);
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
            // R2010 からは、使い道の分からない B が続く。試料の図面と同じく偽とする
            if r2010 {
                fields.bit(false);
            }
            // 文字の体裁と、引出線・矢印・1 つめと 2 つめの矢印のブロック。試料の図面と同じく、参照の型は 5
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
                add_entry_r2010(&data, handles_at, handle, type_code, self.release)?
            }
            false => add_entry(
                &self.current(changes, record)?,
                handle,
                type_code,
                self.release,
            )?,
        };
        changes.insert(record, entry);
        if r2010 {
            let mut out = BitWriter::default();
            out.bot(type_code);
            return Some((handle, r2010_assemble(out, fields, handles)?));
        }
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

    /// R2010 から後の `record` 番目のオブジェクトの、今のデータ（ハンドルの流れの大きさ（MC）の後）と、
    /// ハンドルの流れの始まりのビットの位置。`changes` のデータは MC から始まる
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

    /// 見出しの変数の HANDSEED。表に足す図形には、これより小さくないハンドルを付ける（決定 0052）。
    /// 読めなければ `None`
    pub fn handle_seed(&self) -> Option<u64> {
        let (src, span) = self.header_section()?;
        Some(header::handle_seed(src, span.start, self.release)?.value)
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
                    let data = self.data(entry);
                    let free = if self.release >= Release::R2004 {
                        entity_refs(data, self.release)? == (0, false)
                    } else {
                        let links = entity_links(data, entry.handle)?;
                        links.reactors == 0 && links.xdic == 0
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
    match db.variable(b"$INSUNITS")? {
        [
            Value {
                code: 70,
                data: Data::Int(units),
            },
        ] => i16::try_from(*units).ok(),
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
    // 足したブロックのレコードの BLOCK と ENDBLK（決定 0068）。BLOCK の名前（2）と基点は、レコードに合わせる
    (b"BLOCK", 0x04, &[2, 10, 20, 30]),
    (b"ENDBLK", 0x05, &[]),
];

/// 足したブロックのレコードが持つもの（決定 0068）
#[derive(Default)]
struct NewBlock {
    /// BLOCK と ENDBLK のハンドル
    block: Option<u64>,
    endblk: Option<u64>,
    /// BLOCK の基点
    base: [f64; 3],
    /// 図形のハンドル。足した順
    entities: Vec<u64>,
    /// このブロックを挿す、足した INSERT のハンドル
    inserts: Vec<u64>,
    /// ATTDEF を持つか
    attdef: bool,
}

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
    /// R2018 からの複数行の ATTRIB の、埋め込みの MTEXT が指す ByLayer の線種と材質のハンドル
    embedded: Option<(u64, u64)>,
    /// INSERT が持つ ATTRIB と、SEQEND のハンドル
    children: Option<(Vec<u64>, u64)>,
}

impl NewEntity<'_> {
    /// 前後の図形のハンドルを `prev` と `next` にした、版 `release` の図形のデータ。共通の欄は、色（62）が
    /// なければ ByLayer、線種の尺度（48）がなければ 1、非表示（60）がなければ 0 にし、反応子と拡張辞書と
    /// 図形の絵を持たず、印刷スタイルと線の太さは ByLayer にする。R2000 では前後の図形のハンドルを持ち、
    /// R2004 では持たず、拡張辞書を持たない印を書き、MTEXT の背景の欄（BL）を 0 にする（決定 0079）
    fn encode(&self, prev: u64, next: u64, release: Release) -> Option<Vec<u8>> {
        if release >= Release::R2010 {
            return self.encode_r2010(release);
        }
        let object = self.object;
        let r2004 = release == Release::R2004;
        let mut common = BitWriter::default();
        common.handle(0, self.handle);
        // 拡張データがない
        common.bs(0);
        common.bit(false);
        common.bb(self.mode);
        common.bl(0);
        // R2000 は前後の図形のハンドルを持つ印、R2004 は拡張辞書を持たない印
        common.bit(r2004);
        common.bs(small(object, 62)?.unwrap_or(256));
        common.bd(real(object, 48)?.unwrap_or(1.0));
        common.bb(self.ltype.0);
        common.bb(0);
        common.bs(small(object, 60)?.unwrap_or(0));
        common.rc(lineweight_index(-1)?.try_into().ok()?);
        let mut fields = own_fields(BitWriter::default(), object, object, self.type_code, &[], 0)?;
        if r2004 && object.kind == b"MTEXT" {
            fields.bl(0);
        }
        // R2004 の属性を持つ INSERT は、型ごとの欄の終わりに ATTRIB の数（BL）を持つ（決定 0080）
        if r2004 && let Some((attribs, _)) = &self.children {
            fields.bl(i32::try_from(attribs.len()).ok()?);
        }
        let mut handles = BitWriter::default();
        if self.mode == 0 {
            handles.handle(4, self.owner);
        }
        if !r2004 {
            handles.handle(3, 0);
            handles.handle(4, prev);
            handles.handle(4, next);
        }
        handles.handle(5, self.layer);
        if self.ltype.0 == 3 {
            handles.handle(5, self.ltype.1);
        }
        if let Some(pointed) = self.pointed {
            handles.handle(5, pointed);
        }
        // INSERT は、ブロックに続けて、R2000 では最初と最後の ATTRIB を、R2004 ではすべての ATTRIB を固い
        // 所有で指し、SEQEND を指す（決定 0080）
        if let Some((attribs, seqend)) = &self.children {
            if r2004 {
                for &attrib in attribs {
                    handles.handle(3, attrib);
                }
            } else {
                handles.handle(4, *attribs.first()?);
                handles.handle(4, *attribs.last()?);
            }
            handles.handle(3, *seqend);
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
    /// R2010 から後の形で符号化したデータ（決定 0092）。ハンドルの流れの大きさ（MC）から始まる。材質は画層の
    /// もので、影の印は 0、表示スタイルは持たない
    fn encode_r2010(&self, release: Release) -> Option<Vec<u8>> {
        let object = self.object;
        let mut out = BitWriter::default();
        out.bot(self.type_code);
        out.handle(0, self.handle);
        // 拡張データと図形の絵がない
        out.bs(0);
        out.bit(false);
        out.bb(self.mode);
        out.bl(0);
        // 拡張辞書を持たない印。R2013 からは、DS のデータを持たない印
        out.bit(true);
        if release >= Release::R2013 {
            out.bit(false);
        }
        out.bs(small(object, 62)?.unwrap_or(256));
        out.bd(real(object, 48)?.unwrap_or(1.0));
        out.bb(self.ltype.0);
        out.bb(0);
        // 材質の印（BB）・影の印（RC）・表示スタイルを持つかの 3 つの B
        out.bb(0);
        out.rc(0);
        for _ in 0..3 {
            out.bit(false);
        }
        out.bs(small(object, 60)?.unwrap_or(0));
        out.rc(lineweight_index(-1)?.try_into().ok()?);
        let mut fields = BitWriter::with_strings();
        match object.kind.as_slice() {
            b"ATTRIB" => r2010_attribute(&mut fields, object, &[], [0, 0], release)?,
            b"MTEXT" => r2010_mtext(&mut fields, object, &[], 0, 0, release)?,
            b"SPLINE" => spline(&mut fields, object, &[], 0, release)?,
            _ => fields = own_fields(fields, object, object, self.type_code, &[], 0)?,
        }
        // 属性を持つ INSERT は、R2004 と同じく型ごとの欄の終わりに ATTRIB の数（BL）を持つ（決定 0093）
        if let Some((attribs, _)) = &self.children {
            fields.bl(i32::try_from(attribs.len()).ok()?);
        }
        let mut handles = BitWriter::default();
        if self.mode == 0 {
            handles.handle(4, self.owner);
        }
        handles.handle(5, self.layer);
        if self.ltype.0 == 3 {
            handles.handle(5, self.ltype.1);
        }
        if let Some(pointed) = self.pointed {
            handles.handle(5, pointed);
        }
        // R2018 からの MTEXT は、アプリケーションの名前を指す。試料の図面と同じく 0 とする（決定 0098）
        if object.kind == b"MTEXT" && release >= Release::R2018 {
            handles.handle(5, 0);
        }
        // 複数行の ATTRIB の埋め込みの MTEXT は、所有者（0）・画層・線種・材質・文字の体裁・アプリケーションの
        // 名前（0）を指す。試料の図面と同じ並び（決定 0102）
        if let Some((ltype, material)) = self.embedded {
            handles.handle(4, 0);
            for handle in [self.layer, ltype, material, self.pointed?, 0] {
                handles.handle(5, handle);
            }
        }
        if let Some((attribs, seqend)) = &self.children {
            for &attrib in attribs {
                handles.handle(3, attrib);
            }
            handles.handle(3, *seqend);
        }
        r2010_assemble(out, fields, handles)
    }
}

/// BLOCK の基点（10・20・30）。どれもなければ原点。一部だけなら `None`
fn base_point(object: &Object) -> Option<[f64; 3]> {
    let point = [real(object, 10)?, real(object, 20)?, real(object, 30)?];
    if point.iter().all(Option::is_none) {
        return Some([0.0; 3]);
    }
    Some([point[0]?, point[1]?, point[2]?])
}

/// 名前のオブジェクトの辞書の項目 `group`（ACAD_MATERIAL など）が指す辞書の、項目 `name` が指すオブジェクトの
/// ハンドル
fn dictionary_item(db: &AcadDb, group: &[u8], name: &[u8]) -> Option<u64> {
    let item = |dictionary: &Object, name: &[u8]| {
        dictionary.values.windows(2).find_map(|pair| match pair {
            [
                Value {
                    code: 3,
                    data: Data::Text(found),
                },
                Value {
                    code: 350 | 360,
                    data: Data::Handle(handle),
                },
            ] if found.eq_ignore_ascii_case(name) => Some(*handle),
            _ => None,
        })
    };
    let mut dictionaries = db.objects().iter().filter(|o| o.kind == b"DICTIONARY");
    let group = dictionaries.clone().find_map(|o| item(o, group))?;
    item(dictionaries.find(|o| o.handle() == Some(group))?, name)
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

/// ハンドル `block` の R2004 のブロックのレコードのデータ `data` から、持つ図形のうち `gone` の `count` 個を
/// 除いたもの（決定 0077）。持つ図形の数（BL）を減らし、そのハンドルを除く
fn disown(data: &[u8], block: u64, count: usize, gone: &BTreeSet<u64>) -> Option<Vec<u8>> {
    let owned = owned(data, block, Release::R2004, None)?;
    let kept: Vec<_> = owned
        .handles
        .iter()
        .filter(|(_, handle)| !gone.contains(handle))
        .collect();
    (owned.handles.len().checked_sub(kept.len())? == count).then_some(())?;
    let first = owned.handles.first()?.0.start;
    let last = owned.handles.last()?.0.end;
    let mut fields = BitWriter::default();
    fields.copy(data, owned.size_at + 32..owned.count.start)?;
    fields.bl(i32::try_from(kept.len()).ok()?);
    fields.copy(data, owned.count.end..owned.handles_at)?;
    let mut out = BitWriter::default();
    out.copy(data, 0..owned.size_at)?;
    out.rl(i32::try_from(owned.size_at + 32 + fields.len()).ok()?);
    let len = fields.len();
    out.copy(&fields.into_bytes(), 0..len)?;
    out.copy(data, owned.handles_at..first)?;
    for (at, _) in kept {
        out.copy(data, at.clone())?;
    }
    out.copy(data, last..data.len() * 8)?;
    Some(out.into_bytes())
}

/// ハンドル `block` の R2010 から後のブロックのレコードのデータ `data`（ハンドルの流れの大きさ（MC）の後）から、
/// `gone` の `count` 個の図形を除いたもの（決定 0091）。持つ図形の数（BL）を減らし、文字列の流れは元のまま写して、
/// MC を付け直す。`handles_at` はハンドルの流れの始まり
fn disown_r2010(
    data: &[u8],
    handles_at: usize,
    block: u64,
    count: usize,
    gone: &BTreeSet<u64>,
    release: Release,
) -> Option<Vec<u8>> {
    let owned = owned(data, block, release, Some(handles_at))?;
    let kept: Vec<_> = owned
        .handles
        .iter()
        .filter(|(_, handle)| !gone.contains(handle))
        .collect();
    (owned.handles.len().checked_sub(kept.len())? == count).then_some(())?;
    let first = owned.handles.first()?.0.start;
    let last = owned.handles.last()?.0.end;
    let mut out = BitWriter::default();
    out.copy(data, 0..owned.count.start)?;
    out.bl(i32::try_from(kept.len()).ok()?);
    out.copy(data, owned.count.end..handles_at)?;
    let handles = out.len();
    out.copy(data, handles_at..first)?;
    for (at, _) in kept {
        out.copy(data, at.clone())?;
    }
    out.copy(data, last..data.len() * 8)?;
    with_modular_size(out, handles)
}

/// R2004 の DWG に足せるものか（決定 0079〜0081）。型ごとの欄が R2000 と同じ形の図形か、MTEXT か、
/// 属性を持つ INSERT とその ATTRIB・SEQEND か、表のレコードと、足したブロックの BLOCK・ENDBLK だけを
/// 足せる。MINSERT は、まだ足せない
fn r2004_addable(object: &Object) -> bool {
    const RECORDS: [&[u8]; 8] = [
        b"LAYER",
        b"STYLE",
        b"LTYPE",
        b"APPID",
        b"DIMSTYLE",
        b"VIEW",
        b"VPORT",
        b"BLOCK_RECORD",
    ];
    if object.section == Section::Tables {
        return RECORDS.contains(&object.kind.as_slice());
    }
    const KINDS: [&[u8]; 20] = [
        b"BLOCK",
        b"ENDBLK",
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
        b"ATTDEF",
        b"MTEXT",
        b"LWPOLYLINE",
        b"SPLINE",
        b"INSERT",
        b"ATTRIB",
        b"SEQEND",
    ];
    matches!(object.section, Section::Entities | Section::Blocks)
        && KINDS.contains(&object.kind.as_slice())
        && (object.kind != b"INSERT"
            || object
                .values
                .iter()
                .all(|value| !matches!(value.code, 70 | 71 | 44 | 45)))
}

/// ハンドル `block` の R2004 のブロックのレコードのデータ `data` の、持つ図形の終わりに `handles` を加えた
/// もの（決定 0079）。持つ図形の数（BL）を増やし、ハンドルは固い所有（符号 3）で書く（図面で確かめた）
fn adopt(data: &[u8], block: u64, handles: &[u64]) -> Option<Vec<u8>> {
    let owned = owned(data, block, Release::R2004, None)?;
    let mut fields = BitWriter::default();
    fields.copy(data, owned.size_at + 32..owned.count.start)?;
    fields.bl(i32::try_from(owned.handles.len().checked_add(handles.len())?).ok()?);
    fields.copy(data, owned.count.end..owned.handles_at)?;
    let mut out = BitWriter::default();
    out.copy(data, 0..owned.size_at)?;
    out.rl(i32::try_from(owned.size_at + 32 + fields.len()).ok()?);
    let len = fields.len();
    out.copy(&fields.into_bytes(), 0..len)?;
    out.copy(data, owned.handles_at..owned.end)?;
    for &handle in handles {
        out.handle(3, handle);
    }
    out.copy(data, owned.end..data.len() * 8)?;
    Some(out.into_bytes())
}

/// ハンドル `block` の R2010 から後のブロックのレコードのデータ `data`（ハンドルの流れの大きさ（MC）の後）の、
/// 持つ図形の終わりに `handles` を加えたもの（決定 0092）。`handles_at` はハンドルの流れの始まり
fn adopt_r2010(
    data: &[u8],
    handles_at: usize,
    block: u64,
    handles: &[u64],
    release: Release,
) -> Option<Vec<u8>> {
    let owned = owned(data, block, release, Some(handles_at))?;
    let mut out = BitWriter::default();
    out.copy(data, 0..owned.count.start)?;
    out.bl(i32::try_from(owned.handles.len().checked_add(handles.len())?).ok()?);
    out.copy(data, owned.count.end..handles_at)?;
    let at = out.len();
    out.copy(data, handles_at..owned.end)?;
    for &handle in handles {
        out.handle(3, handle);
    }
    out.copy(data, owned.end..data.len() * 8)?;
    with_modular_size(out, at)
}

/// R2010 から後の DWG に足せるものか（決定 0092〜0102）。型ごとの欄と文字列の流れが R2000 と同じ形に
/// 書ける図形と、MTEXT と、属性を持つ INSERT とその ATTRIB・SEQEND と、SPLINE（R2013 からは
/// 制御点のものだけ）と、
/// UCS のほかの表のレコードと、足したブロックを足せる。MINSERT は、まだ足せない
fn r2010_addable(object: &Object) -> bool {
    // 表のレコードは、LAYER・STYLE・LTYPE・APPID（決定 0094）と、DIMSTYLE・VIEW・ブロックのレコード
    // （決定 0095）を足せる
    if object.section == Section::Tables {
        return matches!(
            object.kind.as_slice(),
            b"LAYER"
                | b"STYLE"
                | b"LTYPE"
                | b"APPID"
                | b"DIMSTYLE"
                | b"VIEW"
                | b"VPORT"
                | b"BLOCK_RECORD"
        );
    }
    // 足したブロックの BLOCK・ENDBLK と、ブロックの持つ図形も足せる（決定 0095）
    let kind = object.kind.as_slice();
    if matches!(kind, b"BLOCK" | b"ENDBLK") {
        return object.section == Section::Blocks;
    }
    const KINDS: [&[u8]; 17] = [
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
        b"LWPOLYLINE",
        b"INSERT",
        b"MTEXT",
        b"SPLINE",
        b"ATTRIB",
        b"SEQEND",
    ];
    matches!(object.section, Section::Entities | Section::Blocks)
        && KINDS.contains(&kind)
        && (kind != b"INSERT"
            || object
                .values
                .iter()
                .all(|value| !matches!(value.code, 70 | 71 | 44 | 45)))
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

/// `old` から `new` に値を変えた、版 `release` の図形のデータ `data` を、符号化し直す。共通の欄と型ごとの欄の
/// ほかの値が変わったか、符号化できない値なら `None`。R2004 は、R2000 と型ごとの欄が同じ種類だけを
/// 扱う（決定 0072）
fn encode(data: &[u8], old: &Object, new: &Object, release: Release) -> Option<Vec<u8>> {
    // MINSERT も、表では INSERT とする。VERTEX と POLYLINE も、型で欄が分かれる
    let type_code = Bits::with_release(data, Release::R2000).bs()?;
    let own = own_codes(&new.kind, type_code);
    if release == Release::R2004 && !R2004_KINDS.contains(&new.kind.as_slice()) {
        return None;
    }
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
    let [size_at, color_at, start, handles_at] = entity_fields(data, release)?;
    let common = if only(COMMON, old, true) == only(COMMON, new, true) {
        let mut common = BitWriter::default();
        common.copy(data, color_at..start)?;
        common
    } else {
        let color = only(&[62], old, true) != only(&[62], new, true);
        common_fields(data, color_at, start, new, release, color)?
    };
    let fields = if only(own, old, true) == only(own, new, true) {
        let mut fields = BitWriter::default();
        fields.copy(data, start..handles_at)?;
        fields
    } else if release == Release::R2004 && new.kind == b"HATCH" {
        // R2004 の初めのグラデーションの欄（表では 450〜470）は、元のビットを写す（決定 0074）
        let mut bits = Bits::with_release(data, release);
        bits.seek(start);
        hatch_gradient(&mut bits)?;
        let mut fields = BitWriter::default();
        fields.copy(data, start..bits.pos())?;
        hatch(&mut fields, old, new)?;
        fields
    } else if release == Release::R2004 && matches!(new.kind.as_slice(), b"INSERT" | b"MTEXT") {
        // MINSERT は、持つオブジェクトの数が列と行の欄の前に入るので、まだ扱わない
        (type_code != MINSERT_TYPE).then_some(())?;
        r2004_own_fields(
            BitWriter::default,
            old,
            new,
            type_code,
            data,
            start,
            handles_at,
        )?
    } else {
        own_fields(BitWriter::default(), old, new, type_code, data, start)?
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

/// 種類 `kind`（型 `type_code`）の図形の、型ごとの欄の組の符号。扱わない種類は空
fn own_codes(kind: &[u8], type_code: i16) -> &'static [i16] {
    match kind {
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
        b"INSERT" if type_code == MINSERT_TYPE => MINSERT,
        b"INSERT" => INSERT,
        b"TOLERANCE" => TOLERANCE,
        b"LEADER" => LEADER,
        b"MLINE" => MLINE,
        b"VIEWPORT" => VIEWPORT,
        b"SHAPE" => SHAPE,
        _ => &[],
    }
}

/// R2010 から後の図形を符号化し直したデータ（決定 0085・0086）。`data` はハンドルの流れの大きさ（MC）の後の
/// データ、`handles_at` はハンドルの流れの始まり。型ごとの欄を変えたときは、主のデータの後の文字列の流れも
/// 書き直す。ハンドルの流れは写し、MC を付けて返す
fn encode_r2010(
    data: &[u8],
    handles_at: usize,
    old: &Object,
    new: &Object,
    release: Release,
) -> Option<Vec<u8>> {
    let type_code = Bits::with_release(data, release).bot()?;
    let own = own_codes(&new.kind, type_code);
    if !R2010_KINDS.contains(&new.kind.as_slice()) {
        return None;
    }
    // R2018 からの段組みの MTEXT は、埋め込みのオブジェクトの印（101）の後に段組みの欄を持つ。欄は背景の欄の
    // 後ろにあり、元のビットを写すので、元と同じときだけ受け付け、印から後を除いて比べる
    let (old, new) = (&columns_removed(old)?, &columns_removed(new)?);
    if old.1 != new.1 {
        return None;
    }
    let (old, new) = (&old.0, &new.0);
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
    let [color_at, start, end] = r2010_entity_fields(data, handles_at, release)?;
    let common = if only(COMMON, old, true) == only(COMMON, new, true) {
        let mut common = BitWriter::default();
        common.copy(data, color_at..start)?;
        common
    } else {
        let color = only(&[62], old, true) != only(&[62], new, true);
        common_fields(data, color_at, start, new, release, color)?
    };
    let mut out = BitWriter::default();
    out.copy(data, 0..color_at)?;
    let common_len = common.len();
    out.copy(&common.into_bytes(), 0..common_len)?;
    let handles = if only(own, old, true) == only(own, new, true) {
        out.copy(data, start..end)?;
        // ハンドルの流れは、主のデータの終わりと同じだけずれる
        let handles = (out.len() + handles_at).checked_sub(end)?;
        out.copy(data, end..data.len() * 8)?;
        handles
    } else {
        // 元の値を符号化した欄が、主のデータの終わりまでの長さを持ち、文字列の流れが元と同じことを確かめる。
        // 角度は度から直すので、主のデータのビットが元と同じとは限らない
        let mut same = r2010_own_fields(old, old, type_code, data, start, end, release)?;
        (start + same.len() == end).then_some(())?;
        let strings_end = r2010_strings_end(data, end, handles_at)?;
        let mut found = BitWriter::default();
        found.copy(data, end..strings_end)?;
        (found.into_bytes() == same.take_strings().into_bytes()).then_some(())?;
        let fields = r2010_own_fields(old, new, type_code, data, start, end, release)?;
        return r2010_finish(out, fields, data, handles_at);
    };
    with_modular_size(out, handles)
}

/// MTEXT `object` から、埋め込みのオブジェクトの印（101）と後の値を除いたものと、除いた値。MTEXT でなければ
/// そのまま
fn columns_removed(object: &Object) -> Option<(Object, Vec<Value>)> {
    let at = match object.kind.as_slice() {
        b"MTEXT" => object.values.iter().position(|value| value.code == 101),
        _ => None,
    };
    let Some(at) = at else {
        return Some((object.clone(), Vec::new()));
    };
    let (values, columns) = object.values.split_at_checked(at)?;
    let kept = Object {
        values: values.to_vec(),
        ..object.clone()
    };
    Some((kept, columns.to_vec()))
}

/// R2010 から後のオブジェクトのデータを、`out` に続けて、型ごとの欄 `fields` の主のデータと文字列の流れ、
/// 流れのビットの数と有無の印、元のハンドルの流れの順に書き、MC を付けて返す
fn r2010_finish(
    out: BitWriter,
    fields: BitWriter,
    data: &[u8],
    handles_at: usize,
) -> Option<Vec<u8>> {
    let mut handles = BitWriter::default();
    handles.copy(data, handles_at..data.len() * 8)?;
    r2010_assemble(out, fields, handles)
}

/// `out` に、主のデータと文字列の流れを持つ `fields` と、その大きさと有無の印と、ハンドルの流れ `handles` を続け、
/// ハンドルの流れの大きさ（MC）を前に付けたデータ
fn r2010_assemble(
    mut out: BitWriter,
    mut fields: BitWriter,
    handles: BitWriter,
) -> Option<Vec<u8>> {
    let strings = fields.take_strings();
    let fields_len = fields.len();
    out.copy(&fields.into_bytes(), 0..fields_len)?;
    let size = strings.len();
    out.copy(&strings.into_bytes(), 0..size)?;
    if size > 0 {
        // 流れのビットの数（RS）。0x8000 に収まらなければ、上位の RS を前に置き、下位に 0x8000 を立てる
        if size >= 0x8000 {
            out.rs(i16::try_from(size >> 15).ok()?);
            out.rs(i16::from_ne_bytes(
                (u16::try_from(size & 0x7FFF).ok()? | 0x8000).to_ne_bytes(),
            ));
        } else {
            out.rs(i16::try_from(size).ok()?);
        }
    }
    out.bit(size > 0);
    let at = out.len();
    let len = handles.len();
    out.copy(&handles.into_bytes(), 0..len)?;
    with_modular_size(out, at)
}

/// `out` の前に、ビット `handles` から終わりまでのビットの数（MC）を付ける
fn with_modular_size(out: BitWriter, handles: usize) -> Option<Vec<u8>> {
    let out = out.into_bytes();
    let mut bytes = Vec::new();
    push_modular_char(
        &mut bytes,
        i64::try_from((out.len() * 8).checked_sub(handles)?).ok()?,
        false,
    );
    bytes.extend(out);
    Some(bytes)
}

/// R2010 から後の型ごとの欄を、文字列の流れを分けて符号化する。R2004 で後ろに欄が増えた種類は、増えた欄を
/// 主のデータの終わり `end` まで写す
fn r2010_own_fields(
    old: &Object,
    new: &Object,
    type_code: i16,
    data: &[u8],
    start: usize,
    end: usize,
    release: Release,
) -> Option<BitWriter> {
    if matches!(new.kind.as_slice(), b"ATTRIB" | b"ATTDEF") {
        let mut fields = BitWriter::with_strings();
        r2010_attribute(&mut fields, new, data, [start, end], release)?;
        Some(fields)
    } else if new.kind == b"VIEWPORT" {
        let mut fields = BitWriter::with_strings();
        viewport(&mut fields, old, new, data, [start, end], release)?;
        Some(fields)
    } else if new.kind == b"TOLERANCE" {
        let mut fields = BitWriter::with_strings();
        tolerance(&mut fields, old, new, data, [start, end], release)?;
        Some(fields)
    } else if new.kind == b"SHAPE" {
        let mut fields = BitWriter::with_strings();
        shape(&mut fields, new, data, [start, end], release)?;
        Some(fields)
    } else if new.kind == b"DIMENSION" {
        let mut fields = BitWriter::with_strings();
        dimension(&mut fields, old, new, type_code, data, start, release)?;
        Some(fields)
    } else if new.kind == b"MTEXT" {
        let mut fields = BitWriter::with_strings();
        r2010_mtext(&mut fields, new, data, start, end, release)?;
        Some(fields)
    } else if new.kind == b"VERTEX" && type_code == 0x0A {
        // 2D の頂点は、膨らみの後に頂点の番号（BL）を持つ。元のビットを写し、接線の向きを後に書く
        let mut bits = Bits::with_release(data, release);
        bits.seek(start);
        bits.rc()?;
        for _ in 0..3 {
            bits.bd()?;
        }
        if bits.bd()? >= 0.0 {
            bits.bd()?;
        }
        bits.bd()?;
        let id_at = bits.pos();
        bits.bl()?;
        let id = id_at..bits.pos();
        let mut whole = BitWriter::with_strings();
        vertex(&mut whole, new, type_code)?;
        // 接線の向き（BD）の前までを写し、番号と接線の向きを続ける
        let tangent = real(new, 50)?.unwrap_or(0.0).to_radians();
        let mut last = BitWriter::default();
        last.bd(tangent);
        let head = whole.len().checked_sub(last.len())?;
        let mut fields = BitWriter::with_strings();
        fields.copy(&whole.into_bytes(), 0..head)?;
        fields.copy(data, id)?;
        fields.bd(tangent);
        Some(fields)
    } else if new.kind == b"HATCH" {
        // 初めのグラデーションの欄は、元のビットを写す（決定 0074）。その名前は文字列の流れの初めにあるので、
        // 流れの初めの TU も写す（決定 0088）
        let mut bits = Bits::with_release(data, release);
        bits.seek(start);
        hatch_gradient(&mut bits)?;
        let mut fields = BitWriter::with_strings();
        fields.copy(data, start..bits.pos())?;
        bits.seek(end);
        let units = usize::try_from(bits.bs()?).ok()?;
        fields.copy_strings(data, end..bits.pos() + units * 16)?;
        hatch(&mut fields, old, new)?;
        Some(fields)
    } else if new.kind == b"SPLINE" {
        let mut fields = BitWriter::with_strings();
        spline(&mut fields, new, data, start, release)?;
        Some(fields)
    } else if new.kind == b"INSERT" {
        (type_code != MINSERT_TYPE).then_some(())?;
        r2004_own_fields(
            BitWriter::with_strings,
            old,
            new,
            type_code,
            data,
            start,
            end,
        )
    } else {
        own_fields(BitWriter::with_strings(), old, new, type_code, data, start)
    }
}

/// R2010 から後の MTEXT の欄（決定 0087）。幅（41）の後に枠の高さ（46）を書く。範囲の大きさと、文字列の後の
/// 印から主のデータの終わり `end` まで（背景の欄など）は、元のビットを写す
fn r2010_mtext(
    fields: &mut BitWriter,
    object: &Object,
    data: &[u8],
    start: usize,
    end: usize,
    release: Release,
) -> Option<()> {
    // 足した MTEXT は元のデータを持たない（`data` が空）。範囲の大きさを 0 にし、後の B と背景の印（BL）を 0 に
    // する（決定 0093）
    let copied = match data.is_empty() {
        true => None,
        false => {
            // 文字列は文字列の流れにあるので、主のデータの位置を求めるときは読まない
            let mut bits = Bits::with_release(data, release);
            bits.seek(start);
            for _ in 0..12 {
                bits.bd()?;
            }
            bits.bs()?;
            bits.bs()?;
            let extents_at = bits.pos();
            bits.bd()?;
            bits.bd()?;
            let extents_end = bits.pos();
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
    for code in [41, 46, 40] {
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
    fields.text(&text)?;
    fields.bs(small(object, 73)??);
    fields.bd(real(object, 44)??);
    match copied {
        Some((_, flag_at)) => fields.copy(data, flag_at..end),
        None => {
            fields.bit(false);
            fields.bl(0);
            // R2018 からは、注釈的でない印（B）と、版（BS）・既定の印（B）・揃え・X 軸・挿入点・枠の幅と高さ・
            // 範囲の幅と高さ・段の種類（BS）が続く。試料の図面と同じく、注釈的でなく、版は 4、既定の印は真、
            // 段はないとし、ほかは型ごとの欄と同じ値を書く（決定 0098）
            if release >= Release::R2018 {
                fields.bit(true);
                fields.bs(4);
                fields.bit(true);
                fields.bl(i32::from(small(object, 71)??));
                for (code, default) in [(11, 1.0), (21, 0.0), (31, 0.0)] {
                    fields.bd(real(object, code)?.unwrap_or(default));
                }
                point3(fields, object, 10)?;
                for code in [41, 46] {
                    fields.bd(real(object, code)??);
                }
                fields.bd(0.0);
                fields.bd(0.0);
                fields.bs(0);
            }
            Some(())
        }
    }
}

/// R2018 からの複数行の属性の、意味の分からない 37 ビット。試料の図面の属性はどれもこの値を持つ（決定 0102）
const EMBEDDED_BITS: &[u8; 37] = b"0010101101111100000000000001000011101";

/// 複数行の属性の種類（71）の位置。属性の版（280）の後の 71 で、版の前の 71 は文字の向き。属性の値は、その前に
/// 並ぶ（決定 0102）
fn multiline_at(object: &Object) -> Option<usize> {
    let version = object.values.iter().position(|value| value.code == 280)?;
    object
        .values
        .iter()
        .skip(version)
        .position(|value| value.code == 71)
        .map(|at| at + version)
}

/// R2018 からの複数行の属性の元の欄の、種類の後の 37 ビットの始まりと、埋め込みの MTEXT の欄の始まりと、
/// 段の種類の後の終わりのビットの位置。型ごとの欄は `start` から始まる。文字列は文字列の流れにあるので読まない
fn embedded_span(data: &[u8], start: usize, release: Release) -> Option<[usize; 3]> {
    let mut bits = Bits::with_release(data, release);
    bits.seek(start);
    // TEXT の欄（R2000）。DataFlags のビットが立つ欄はない
    let flags = bits.rc()?;
    if flags & 0x01 == 0 {
        bits.rd()?;
    }
    bits.rd()?;
    bits.rd()?;
    if flags & 0x02 == 0 {
        bits.dd(0.0)?;
        bits.dd(0.0)?;
    }
    if !bits.bit()? {
        for _ in 0..3 {
            bits.bd()?;
        }
    }
    if !bits.bit()? {
        bits.bd()?;
    }
    for bit in [0x04, 0x08] {
        if flags & bit == 0 {
            bits.rd()?;
        }
    }
    bits.rd()?;
    if flags & 0x10 == 0 {
        bits.rd()?;
    }
    for bit in [0x20, 0x40, 0x80] {
        if flags & bit == 0 {
            bits.bs()?;
        }
    }
    // 属性の版と種類
    bits.rc()?;
    (bits.rc()? != 1).then_some(())?;
    let at = bits.pos();
    let mtext_at = at.checked_add(EMBEDDED_BITS.len())?;
    bits.seek(mtext_at);
    // 埋め込みの MTEXT の欄。挿入点・押し出しと X 軸の向き・幅・枠の高さ・文字の高さ、揃えと向き、範囲の大きさ、
    // 行の間隔の取り方と倍率、B と背景の欄
    for _ in 0..12 {
        bits.bd()?;
    }
    bits.bs()?;
    bits.bs()?;
    bits.bd()?;
    bits.bd()?;
    bits.bs()?;
    bits.bd()?;
    bits.bit()?;
    if bits.bl()? & 1 != 0 {
        bits.bd()?;
        bits.cmc()?;
        bits.bl()?;
    }
    // R2018 から増えた欄（決定 0098）。段のあるものは、まだ扱わない
    bits.bit()?;
    bits.bs()?;
    bits.bit()?;
    bits.bl()?;
    for _ in 0..10 {
        bits.bd()?;
    }
    (bits.bl()? == 0).then_some(())?;
    Some([at, mtext_at, bits.pos()])
}

/// R2010 から後の ATTRIB・ATTDEF の欄。名前の前に属性の版（RC、1 つ目の 280）を、印の後に位置を固定するか
/// （B、2 つ目の 280）を書く。ATTDEF の問いの前の版（RC）は、主のデータの終わり `end` の前の元のビットを写す。
/// 型ごとの欄は `start` から始まり、足したものは元のデータを持たない（`data` が空）
fn r2010_attribute(
    fields: &mut BitWriter,
    object: &Object,
    data: &[u8],
    [start, end]: [usize; 2],
    release: Release,
) -> Option<()> {
    // 複数行の属性は、属性の値の後に、種類（71）・72・埋め込みのオブジェクトの印（101）と MTEXT の値を持つ
    let mut own = object.clone();
    let rest = match multiline_at(object) {
        Some(at) => own.values.split_off(at),
        None => Vec::new(),
    };
    let object = &own;
    let mut versions = object.values.iter().filter(|value| value.code == 280);
    let mut next = || match versions.next().map(|value| &value.data) {
        Some(Data::Int(value)) => u8::try_from(*value).ok(),
        _ => None,
    };
    let (version, locked) = (next()?, next()?);
    text(fields, object, 74)?;
    fields.rc(version);
    // R2018 からは、版の後に属性の種類（RC）が続く。複数行の属性は、37 ビットの後に、埋め込みの MTEXT の欄と
    // DXF の 72 にあたる BS を持つ。37 ビットは元のものを写し、足したものは試料の図面と同じ値を書く。
    // MTEXT の欄は、MTEXT と同じく書く（決定 0098・0102）
    match rest.as_slice() {
        [] if release >= Release::R2018 => fields.rc(1),
        [] => {}
        [kind, after, marker, mtext @ ..]
            if release >= Release::R2018
                && (kind.code, after.code, marker.code) == (71, 72, 101) =>
        {
            let int = |value: &Value| match value.data {
                Data::Int(value) => Some(value),
                _ => None,
            };
            let kind = u8::try_from(int(kind)?).ok().filter(|&kind| kind != 1)?;
            fields.rc(kind);
            let embedded = Object {
                values: mtext.to_vec(),
                ..object.clone()
            };
            if data.is_empty() {
                for &bit in EMBEDDED_BITS {
                    fields.bit(bit == b'1');
                }
                r2010_mtext(fields, &embedded, &[], 0, 0, release)?;
            } else {
                let [at, mtext_at, mtext_end] = embedded_span(data, start, release)?;
                fields.copy(data, at..mtext_at)?;
                r2010_mtext(fields, &embedded, data, mtext_at, mtext_end, release)?;
            }
            fields.bs(i16::try_from(int(after)?).ok()?);
        }
        _ => return None,
    }
    string(fields, object, 2)?;
    fields.bs(small(object, 73)?.unwrap_or(0));
    fields.rc(u8::try_from(small(object, 70)??).ok()?);
    fields.bit(locked != 0);
    if object.kind == b"ATTDEF" {
        fields.copy(data, end.checked_sub(8)?..end)?;
        string(fields, object, 3)?;
    }
    Some(())
}

/// 主のデータの終わり `end` から始まる文字列の流れの終わり（流れのビットの数（RS）の始まり）のビットの位置。
/// 流れがなければ `end`
fn r2010_strings_end(data: &[u8], end: usize, handles_at: usize) -> Option<usize> {
    let mut bits = Bits::new(data);
    bits.seek(handles_at.checked_sub(1)?);
    if !bits.bit()? {
        return Some(end);
    }
    let at = handles_at.checked_sub(17)?;
    bits.seek(at);
    let low = bits.rs()?;
    if low < 0 {
        at.checked_sub(16)
    } else {
        Some(at)
    }
}

/// 表のレコードの、型ごとの欄でない値が変わっていなければ `Some`（決定 0043〜0046）
fn same_record_rest(old: &Object, new: &Object) -> Option<()> {
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
    (rest(old) == rest(new)).then_some(())
}

/// 表のレコードの、ビット `start` から始まる欄を `fields` に書き、元のデータの型ごとの欄の終わりを返す。
/// 名前・参照される印・外部参照の番号・外部参照に依る印は、元のビットを写す
fn record_fields(
    fields: &mut BitWriter,
    old: &Object,
    new: &Object,
    data: &[u8],
    start: usize,
    release: Release,
) -> Option<usize> {
    let mut bits = Bits::with_release(data, release);
    bits.seek(start);
    bits.text()?;
    bits.bit()?;
    // R2010 からは、外部参照の番号（BS）がない
    if release < Release::R2010 {
        bits.bs()?;
    }
    let xdep = bits.bit()?;
    let own_at = bits.pos();
    fields.copy(data, start..own_at)?;
    match new.kind.as_slice() {
        b"LAYER" => layer(fields, new, xdep, data, own_at, release),
        b"STYLE" => style(fields, old, new, xdep, data, own_at, release),
        b"LTYPE" => ltype(fields, old, new, xdep, data, own_at, release),
        b"VIEW" => view_record(fields, new, xdep, data, own_at, release),
        b"UCS" => ucs_record(fields, new, xdep, data, own_at, release),
        b"VPORT" => vport_record(fields, old, new, xdep, data, own_at, release),
        _ => dimstyle(fields, old, new, xdep, data, own_at, release),
    }
}

/// R2010 から後の表のレコードを符号化し直したデータ（決定 0089）。`data` はハンドルの流れの大きさ（MC）の後の
/// データ、`handles_at` はハンドルの流れの始まり。文字列の流れは、初めの名前を写し、型ごとの欄の文字列を続ける
fn encode_record_r2010(
    data: &[u8],
    handles_at: usize,
    old: &Object,
    new: &Object,
    release: Release,
) -> Option<Vec<u8>> {
    same_record_rest(old, new)?;
    // UCS のレコードは、試料の図面になく確かめていない
    (new.kind != b"UCS").then_some(())?;
    let [start, end] = r2010_object_fields(data, handles_at, release)?;
    let encode = |object: &Object| -> Option<BitWriter> {
        let mut fields = BitWriter::with_strings();
        let mut bits = Bits::with_release(data, release);
        bits.seek(end);
        let units = usize::try_from(bits.bs()?).ok()?;
        fields.copy_strings(data, end..bits.pos() + units * 16)?;
        let own_end = record_fields(&mut fields, old, object, data, start, release)?;
        fields.copy(data, own_end..end)?;
        Some(fields)
    };
    r2010_checked(data, [start, end, handles_at], encode(old)?, encode(new)?)
}

/// 型ごとの欄がビット `start` から `end` まで、ハンドルの流れが `handles_at` からの R2010 のオブジェクトに、
/// 欄 `fields` を置いたデータ。元の値を符号化した `same` が、元の主のデータの長さと文字列の流れを持つことを確かめる
fn r2010_checked(
    data: &[u8],
    [start, end, handles_at]: [usize; 3],
    mut same: BitWriter,
    fields: BitWriter,
) -> Option<Vec<u8>> {
    (start + same.len() == end).then_some(())?;
    let strings_end = r2010_strings_end(data, end, handles_at)?;
    let mut found = BitWriter::default();
    found.copy(data, end..strings_end)?;
    (found.into_bytes() == same.take_strings().into_bytes()).then_some(())?;
    let mut out = BitWriter::default();
    out.copy(data, 0..start)?;
    r2010_finish(out, fields, data, handles_at)
}

/// 表のレコードの型ごとの欄を符号化し直したデータ（決定 0043〜0046）。LAYER・STYLE・LTYPE・VIEW・UCS・VPORT・DIMSTYLE だけを扱う。名前（2）などの
/// ほかの値が変わったものは `None`
fn encode_record(data: &[u8], old: &Object, new: &Object, release: Release) -> Option<Vec<u8>> {
    same_record_rest(old, new)?;
    let [size_at, start, handles_at] = object_fields(data, release)?;
    let mut fields = BitWriter::default();
    let end = record_fields(&mut fields, old, new, data, start, release)?;
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
fn encode_object(data: &[u8], old: &Object, new: &Object, release: Release) -> Option<Vec<u8>> {
    let [size_at, start, handles_at] = object_fields(data, release)?;
    let mut fields = BitWriter::default();
    let end = object_own_fields(&mut fields, old, new, data, start, release)?;
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

/// R2010 から後の OBJECTS のオブジェクトを符号化し直したデータ（決定 0090）。`data` はハンドルの流れの
/// 大きさ（MC）の後のデータ、`handles_at` はハンドルの流れの始まり。文字列の流れは、型ごとの欄の文字列だけを持つ。
/// XRECORD の文字列の値は、試料の図面になく確かめていないので受け付けない
fn encode_object_r2010(
    data: &[u8],
    handles_at: usize,
    old: &Object,
    new: &Object,
    release: Release,
) -> Option<Vec<u8>> {
    let text = |object: &Object| {
        object
            .values
            .iter()
            .any(|v| matches!(v.data, Data::Text(_)))
    };
    (new.kind != b"XRECORD" || !text(old) && !text(new)).then_some(())?;
    let [start, end] = r2010_object_fields(data, handles_at, release)?;
    let encode = |object: &Object| -> Option<BitWriter> {
        let mut fields = BitWriter::with_strings();
        let own_end = object_own_fields(&mut fields, old, object, data, start, release)?;
        fields.copy(data, own_end..end)?;
        Some(fields)
    };
    r2010_checked(data, [start, end, handles_at], encode(old)?, encode(new)?)
}

/// OBJECTS のオブジェクトの型ごとの欄を、ビット `start` から読みながら `fields` に書き、元の欄の終わりの位置を返す
fn object_own_fields(
    fields: &mut BitWriter,
    old: &Object,
    new: &Object,
    data: &[u8],
    start: usize,
    release: Release,
) -> Option<usize> {
    let own: &[i16] = match new.kind.as_slice() {
        b"GROUP" => &[300, 70, 71],
        b"DICTIONARYVAR" => &[280, 1],
        b"DICTIONARY" | b"ACDBDICTIONARYWDFLT" => &[280, 281, 3],
        b"MLINESTYLE" => &[2, 70, 3, 62, 51, 52, 71, 49],
        b"LAYOUT" => return layout(fields, old, new, data, start, release),
        b"XRECORD" => return xrecord(fields, old, new, data, start, release),
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
    let mut bits = Bits::with_release(data, release);
    bits.seek(start);
    if new.kind == b"GROUP" {
        // 説明・名前のない印・選べる印。図形の数は元のまま写す
        bits.text()?;
        bits.bs()?;
        bits.bs()?;
        string(fields, new, 300)?;
        for code in [70, 71] {
            fields.bs(i16::from(flag(new, code)?));
        }
    } else if new.kind == b"DICTIONARYVAR" {
        // 型と値
        bits.rc()?;
        bits.text()?;
        fields.rc(u8::try_from(small(new, 280)??).ok()?);
        string(fields, new, 1)?;
    } else if new.kind == b"MLINESTYLE" {
        mlinestyle(fields, &mut bits, new, data, release)?;
    } else {
        dictionary(fields, &mut bits, new, data)?;
    }
    Some(bits.pos())
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
    release: Release,
) -> Option<usize> {
    let mut bits = Bits::with_release(data, release);
    bits.seek(own_at);
    let old = bits.bs()?;
    let color_at = bits.pos();
    let old_color = bits.cmc()?;
    let (flags, color) = layer_flags(object, xdep, old)?;
    fields.bs(flags);
    same_cmc(
        fields,
        data,
        color_at..bits.pos(),
        old_color,
        color,
        release,
    )?;
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

/// 型 `type_code` のレコードの表のオブジェクトのデータ `data` の、項目の数（BL）を 1 増やし、項目の
/// ハンドルの終わりに `entry` を足したもの（決定 0061）。ハンドルの流れの後の埋めるビットは写さない
fn add_entry(data: &[u8], entry: u64, type_code: i16, release: Release) -> Option<Vec<u8>> {
    let [size_at, start, handles_at] = object_fields(data, release)?;
    let mut bits = Bits::with_release(data, release);
    bits.seek(size_at + 32);
    bits.handle()?;
    skip_extended_data(&mut bits)?;
    let reactors = bits.bl()?;
    // R2004 からは、拡張辞書を持たなければ、そのハンドルもない
    let xdic = release < Release::R2004 || !bits.bit()?;
    let count = bits.bl()?;
    let mut fields = BitWriter::default();
    fields.bl(count.checked_add(1)?);
    // 寸法の体裁の表は、項目の数の後に RC を持ち、項目の後にその数だけハンドルが続く。ブロックの表は、
    // 項目の後にモデル空間と用紙空間のハンドルが続く（決定 0068）
    let extra = match type_code {
        DIMSTYLE_TYPE => bits.rc()?,
        BLOCK_RECORD_TYPE => 2,
        _ => 0,
    };
    if type_code == DIMSTYLE_TYPE {
        fields.rc(extra);
    }
    (bits.pos() == handles_at).then_some(())?;
    let mut handles = BitWriter::default();
    // 所有者・反応子・拡張辞書・項目
    for _ in 0..(1 + i64::from(reactors) + i64::from(xdic) + i64::from(count)) {
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

/// 型 `type_code` のレコードの、R2010 から後の表のオブジェクトのデータ `data`（ハンドルの流れの大きさ（MC）の
/// 後）の、項目の数（BL）を 1 増やし、項目のハンドルの終わりに `entry` を足したもの（決定 0094）。`handles_at` は
/// ハンドルの流れの始まり。ハンドルの流れの後の埋めるビットは写さない
fn add_entry_r2010(
    data: &[u8],
    handles_at: usize,
    entry: u64,
    type_code: i16,
    release: Release,
) -> Option<Vec<u8>> {
    let mut bits = Bits::with_release(data, release);
    bits.bot()?;
    let end = bits.split_strings(handles_at)?;
    bits.handle()?;
    skip_extended_data(&mut bits)?;
    let reactors = bits.bl()?;
    let xdic = !bits.bit()?;
    if release >= Release::R2013 {
        bits.bit()?;
    }
    let count_at = bits.pos();
    let count = bits.bl()?;
    let count_end = bits.pos();
    let extra = match type_code {
        DIMSTYLE_TYPE => bits.rc()?,
        BLOCK_RECORD_TYPE => 2,
        _ => 0,
    };
    (bits.pos() == end).then_some(())?;
    let mut out = BitWriter::default();
    out.copy(data, 0..count_at)?;
    out.bl(count.checked_add(1)?);
    out.copy(data, count_end..handles_at)?;
    let at = out.len();
    bits.seek(handles_at);
    // 所有者・反応子・拡張辞書・項目
    for _ in 0..(1 + i64::from(reactors) + i64::from(xdic) + i64::from(count)) {
        let found = bits.handle()?;
        out.handle(found.code, found.value);
    }
    out.handle(2, entry);
    for _ in 0..extra {
        let found = bits.handle()?;
        out.handle(found.code, found.value);
    }
    with_modular_size(out, at)
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
    release: Release,
) -> Option<usize> {
    let mut bits = Bits::with_release(data, release);
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
        // R2010 からは、文字列は文字列の流れに書く（決定 0089）
        if text_of(old, code) == text_of(new, code) && release < Release::R2010 {
            fields.copy(data, range)?;
        } else {
            string(fields, new, code)?;
        }
    }
    Some(bits.pos())
}

/// LTYPE の欄（決定 0044）。要素の数（73）と、要素ごとの形と文字の印（74 の 2・4）は、元のものと同じでなければ
/// 受け付けない。要素の印の 7 より上のビットと、形も文字もない要素の形の番号・ずれ・尺度・回転は、元のまま写す。
/// 説明（3）と文字の要素の文字列（9）が変わらなければ元のビットを写し、文字列が変われば、256 バイト（R2010 からは
/// 512 バイト）の領域に初めから順に詰め直す。元の欄の終わりの位置を返す
fn ltype(
    fields: &mut BitWriter,
    old: &Object,
    new: &Object,
    xdep: bool,
    data: &[u8],
    own_at: usize,
    release: Release,
) -> Option<usize> {
    let mut bits = Bits::with_release(data, release);
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
    // R2010 からは、文字の要素があるときだけ、UTF-16LE の 512 バイトの領域がある（決定 0089）
    let r2010 = release >= Release::R2010;
    let area_at = bits.pos();
    if !r2010 {
        bits.skip_bytes(256)?;
    } else if dashes.iter().any(|dash| dash.5 & 2 != 0) {
        bits.skip_bytes(512)?;
    }
    let texts = |object: &Object| -> Vec<_> {
        object
            .values
            .iter()
            .filter(|value| value.code == 9)
            .cloned()
            .collect()
    };
    let same_texts = texts(old) == texts(new);
    // R2013 からは、DXF の 74 も文字の要素の印（8）を持つ
    let mask = if release >= Release::R2013 { 15 } else { 7 };
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
        Data::Text(_) if text_of(old, 3) == text_of(new, 3) && !r2010 => {
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
        (dxf & !mask == 0 && dxf & 6 == flags & 6).then_some(())?;
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
                // R2010 からは、UTF-16LE で詰め、形の番号はバイトの位置とする（決定 0100）
                let at = i16::try_from(area.len()).ok()?;
                let text = uncaret(text)?;
                if r2010 {
                    for unit in core::str::from_utf8(&text).ok()?.encode_utf16().chain([0]) {
                        area.extend(unit.to_le_bytes());
                    }
                } else {
                    area.extend(text);
                    area.push(0);
                }
                at
            };
            fields.bs(shape);
            fields.rd(x);
            fields.rd(y);
            fields.bd(scale);
            fields.bd(rotation);
        }
        fields.bs(flags & !mask | dxf);
    }
    values.0.next().is_none().then_some(())?;
    if same_texts {
        fields.copy(data, area_at..bits.pos())?;
    } else {
        let size = if r2010 { 512 } else { 256 };
        (area.len() <= size).then_some(())?;
        area.resize(size, 0);
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
    release: Release,
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
    let mut bits = Bits::with_release(data, release);
    bits.seek(own_at);
    // R2010 からは、増えた欄も並べ、文字列は文字列の流れに書く（決定 0089）
    let r2010 = release >= Release::R2010;
    for (code, field) in dim_fields(release) {
        let at = bits.pos();
        match field {
            Dim::Text => bits.text().map(drop)?,
            Dim::Real(_) => bits.bd().map(drop)?,
            Dim::Bit(_) => bits.bit().map(drop)?,
            Dim::Short(_) => bits.bs().map(drop)?,
            Dim::Color(_) => bits.cmc().map(drop)?,
        }
        if of(old, code) == of(new, code) && !(r2010 && matches!(field, Dim::Text)) {
            fields.copy(data, at..bits.pos())?;
            continue;
        }
        dim_field(fields, new, code, field, release)?;
    }
    Some(bits.pos())
}

/// DIMSTYLE の `code` の欄を、`new` の値で書く。値がなければ、読む側と同じ既定の値で書く
fn dim_field(
    fields: &mut BitWriter,
    new: &Object,
    code: i16,
    field: Dim,
    release: Release,
) -> Option<()> {
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
        Dim::Short(default) => fields.bs(small(new, code)?.unwrap_or(default)),
        Dim::Color(default) => cmc(fields, small(new, code)?.unwrap_or(default), release)?,
    }
    Some(())
}

/// 色の番号 `index` の CMC。元の CMC `old`（`data` の `at` のビット）が同じ番号の色なら、元のビットを写す
fn same_cmc(
    fields: &mut BitWriter,
    data: &[u8],
    at: Range<usize>,
    old: Color,
    index: i16,
    release: Release,
) -> Option<()> {
    if old.rgb.is_none() && old.index == index {
        fields.copy(data, at)
    } else {
        cmc(fields, index, release)
    }
}

/// 色の番号 `index` の CMC（決定 0075）。R2004 からは、番号（BS）の後に、色の決め方（上位の
/// バイト）と番号（下位のバイト）の BL と、名前を持たない印（RC の 0）を書く。決め方は、ByLayer（256）が
/// 0xC0、ByBlock（0）が 0xC1、ほかの番号が 0xC3
fn cmc(fields: &mut BitWriter, index: i16, release: Release) -> Option<()> {
    fields.bs(index);
    if release >= Release::R2004 {
        let method: u32 = match index {
            256 => 0xC0 << 24,
            0 => 0xC1 << 24,
            1..=255 => (0xC3 << 24) | u32::from(index.unsigned_abs()),
            _ => return None,
        };
        fields.bl(i32::from_ne_bytes(method.to_ne_bytes()));
        fields.rc(0);
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

/// 版 `release` の LAYOUT の欄の形の並び。R2004 からは、印刷するビューの名前がハンドルの流れに移り、
/// 印刷の設定の終わりに陰影の印刷の 3 つの欄が加わる（決定 0082）
fn layout_fields(release: Release) -> Vec<Field> {
    let mut fields = LAYOUT_FIELDS.to_vec();
    if release >= Release::R2004 {
        fields.remove(19);
        fields.splice(26..26, [Field::Short; 3]);
    }
    fields
}

/// LAYOUT の欄を `fields` に符号化し直し、元の欄の終わりの位置を返す（決定 0049・0082）。値の番号の並びが元と同じで、レイアウトの名前（2 つめの 1）
/// とハンドルの並びから作る値が変わらないものだけを扱う。欄は DWG の並びのまま、すべて書き直す
fn layout(
    fields: &mut BitWriter,
    old: &Object,
    new: &Object,
    data: &[u8],
    start: usize,
    release: Release,
) -> Option<usize> {
    let codes = |object: &Object| -> Vec<i16> { object.values.iter().map(|v| v.code).collect() };
    if codes(old) != codes(new) {
        return None;
    }
    // 欄は、ハンドル（5）と所有者（330）の後に、ハンドルの並びから作る値の前に並ぶ
    let at = new.values.iter().position(|value| value.code == 1)?;
    let layout = layout_fields(release);
    let range = at..at + layout.len();
    // レイアウトの名前は、印刷の設定の欄の次
    let name = at + layout.len() - 27;
    let own = new.values.get(range.clone())?;
    let rest = |object: &Object| -> Option<Vec<Value>> {
        let mut values = object.values.clone();
        values.drain(range.clone());
        // レイアウトの名前
        values.push(object.values.get(name)?.clone());
        Some(values)
    };
    if rest(old)? != rest(new)? {
        return None;
    }
    let mut bits = Bits::with_release(data, release);
    bits.seek(start);
    for (field, value) in layout.iter().zip(own) {
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
    Some(bits.pos())
}

/// XRECORD の欄を `fields` に符号化し直し、元の欄の終わりの位置を返す（決定 0050）。値の番号の並びが元と同じものだけを扱う。データは、
/// 元のデータを読みながら値を書き直し、文字列のコード・ページのバイトは元の値のものを写す
fn xrecord(
    fields: &mut BitWriter,
    old: &Object,
    new: &Object,
    data: &[u8],
    start: usize,
    release: Release,
) -> Option<usize> {
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
    let mut bits = Bits::with_release(data, release);
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
    fields.bl(i32::try_from(items.len() / 8).ok()?);
    let len = items.len();
    fields.copy(&items.into_bytes(), 0..len)?;
    fields.bs(i16::try_from(cloning).ok()?);
    Some(bits.pos())
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

/// MLINESTYLE の欄（決定 0048）。名前は元のまま写す（R2010 からは書き直す）。印（70）は DXF のビットを DWG のビットに並べ直し、
/// DXF にない DWG のビットは元のまま写す。角度は度からラジアンに直す。線の線種の番号（R2018 からはハンドル）は
/// 元のまま写す
fn mlinestyle(
    fields: &mut BitWriter,
    bits: &mut Bits,
    new: &Object,
    data: &[u8],
    release: Release,
) -> Option<()> {
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
    // R2010 からは、名前を文字列の流れに書く
    let start = bits.pos();
    bits.text()?;
    if release >= Release::R2010 {
        string(fields, new, 2)?;
    } else {
        fields.copy(data, start..bits.pos())?;
    }
    bits.text()?;
    let flags = bits.bs()?;
    let color_at = bits.pos();
    let old_color = bits.cmc()?;
    let color_end = bits.pos();
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
    same_cmc(
        fields,
        data,
        color_at..color_end,
        old_color,
        int(color)?,
        release,
    )?;
    fields.bd(float(begin)?.to_radians());
    fields.bd(float(end)?.to_radians());
    fields.rc(lines);
    for line in rest.chunks(2) {
        let [offset, color] = line else {
            return None;
        };
        bits.bd()?;
        let color_at = bits.pos();
        let old_color = bits.cmc()?;
        let ltype = bits.pos();
        // R2018 からは、線種を番号でなくハンドルで指す。ハンドルは元のまま写す（決定 0099）
        if release < Release::R2018 {
            bits.bs()?;
        }
        fields.bd(float(offset)?);
        same_cmc(
            fields,
            data,
            color_at..ltype,
            old_color,
            int(color)?,
            release,
        )?;
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
    release: Release,
) -> Option<usize> {
    let mut bits = Bits::with_release(data, release);
    bits.seek(own_at);
    let old = super::objects::view(&mut bits, 0, &mut Vec::new())?;
    let was = old.iter().find(|value| value.code == 72)?;
    (was.data == Data::Int(i64::from(flag(new, 72)?))).then_some(())?;
    // R2010 からは、描画の種類（281）の後の照明の欄を、元のビットから写す（決定 0089）
    let lighting = if release >= Release::R2010 {
        let mut bits = Bits::with_release(data, release);
        bits.seek(own_at);
        for _ in 0..2 {
            bits.bd()?;
        }
        for _ in 0..2 {
            bits.rd()?;
        }
        for _ in 0..10 {
            bits.bd()?;
        }
        for _ in 0..4 {
            bits.bit()?;
        }
        bits.rc()?;
        let at = bits.pos();
        bits.bit()?;
        bits.rc()?;
        bits.bd()?;
        bits.bd()?;
        bits.cmc()?;
        Some((data, at..bits.pos()))
    } else {
        None
    };
    view_fields(fields, new, xdep, lighting)?;
    Some(bits.pos())
}

/// R2010 から後の VIEW・VPORT の、描画の種類の後の照明の欄の既定の値と、そのビットの長さ。試料の図面と同じく、
/// 既定の照明を使い、種類は 1、明るさと対比は 0、環境の色は真の色の 0x333333 とする（決定 0095・0096）
fn default_lighting() -> (Vec<u8>, usize) {
    let mut lighting = BitWriter::default();
    lighting.bit(true);
    lighting.rc(1);
    lighting.bd(0.0);
    lighting.bd(0.0);
    lighting.bs(0);
    lighting.bl(i32::from_ne_bytes(0xC233_3333_u32.to_ne_bytes()));
    lighting.rc(0);
    let bits = lighting.len();
    (lighting.into_bytes(), bits)
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
fn view_fields(
    fields: &mut BitWriter,
    new: &Object,
    xdep: bool,
    lighting: Option<(&[u8], Range<usize>)>,
) -> Option<bool> {
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
    if let Some((data, range)) = lighting {
        fields.copy(data, range)?;
    }
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
    release: Release,
) -> Option<usize> {
    let mut bits = Bits::with_release(data, release);
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
    release: Release,
) -> Option<usize> {
    let mut bits = Bits::with_release(data, release);
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
    // R2010 からは、描画の種類の後の照明の欄を、元のビットから写す（決定 0096）
    let lighting = if release >= Release::R2010 {
        let at = bits.pos();
        bits.bit()?;
        bits.rc()?;
        bits.bd()?;
        bits.bd()?;
        bits.cmc()?;
        Some((data, at..bits.pos()))
    } else {
        None
    };
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
    vport_fields(fields, new, xdep, same.then_some(width), unknown, lighting)?;
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
    lighting: Option<(&[u8], Range<usize>)>,
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
    if let Some((data, range)) = lighting {
        fields.copy(data, range)?;
    }
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

/// 版 `release` の共通の欄の、色から型ごとの欄の前まで。`color` なら色を書き、色（62）がなければ ByLayer に
/// する。線種の尺度（48）がなければ 1、非表示（60）がなければ 0 を書く。線種と印刷スタイルの印と、線の太さは
/// 元のまま写す
fn common_fields(
    data: &[u8],
    color_at: usize,
    start: usize,
    object: &Object,
    release: Release,
    color: bool,
) -> Option<BitWriter> {
    let mut bits = Bits::with_release(data, release);
    bits.seek(color_at);
    let flags = bits.bs()? as u16;
    let index_end = bits.pos();
    bits.seek(color_at);
    bits.enc()?;
    let scale_at = bits.pos();
    bits.bd()?;
    let flags_at = bits.pos();
    // 線種と印刷スタイルの印。R2010 からは、材質と影の印と、表示スタイルを持つかの 3 つの B が続く
    bits.seek(flags_at + 4);
    if release >= Release::R2010 {
        bits.bb()?;
        bits.rc()?;
        for _ in 0..3 {
            bits.bit()?;
        }
    }
    let invisible_at = bits.pos();
    bits.bs()?;
    let lineweight_at = bits.pos();
    let mut fields = BitWriter::default();
    if !color {
        fields.copy(data, color_at..scale_at)?;
    } else if release == Release::R2000 {
        fields.bs(small(object, 62)?.unwrap_or(256));
    } else {
        // R2004 の番号は、印の下の 12 ビットに収まらなければならない。RGB の値と DBCOLOR を持つ色は、
        // まだ変えない。透明度（0x2000 と BL）は元のまま写す
        let index = small(object, 62)?.unwrap_or(256);
        if flags & 0xC000 != 0 || !(0..=0x0FFF).contains(&index) {
            return None;
        }
        fields.bs(index | (flags & 0x2000) as i16);
        fields.copy(data, index_end..scale_at)?;
    }
    fields.bd(real(object, 48)?.unwrap_or(1.0));
    fields.copy(data, flags_at..invisible_at)?;
    fields.bs(small(object, 60)?.unwrap_or(0));
    fields.copy(data, lineweight_at..start)?;
    Some(fields)
}

/// 型ごとの欄。`grid` は MINSERT か。表にない欄は、元のデータ `data` の `start` からの型ごとの欄から写す。
/// 符号化できない種類なら `None`
fn own_fields(
    mut fields: BitWriter,
    old: &Object,
    new: &Object,
    type_code: i16,
    data: &[u8],
    start: usize,
) -> Option<BitWriter> {
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
        b"SPLINE" => spline(&mut fields, new, data, start, Release::R2000)?,
        b"HATCH" => hatch(&mut fields, old, new)?,
        b"VERTEX" => vertex(&mut fields, new, type_code)?,
        b"POLYLINE" => polyline(&mut fields, new, type_code, data, start)?,
        b"DIMENSION" => dimension(
            &mut fields,
            old,
            new,
            type_code,
            data,
            start,
            Release::R2000,
        )?,
        b"INSERT" => {
            // 回転が変わらなければ、元のラジアンを書く。度から直すと、元のビットと同じになるとは限らない
            // （決定 0101）
            let kept = (real(old, 50) == real(new, 50))
                .then(|| insert_rotation(data, start))
                .flatten();
            insert(&mut fields, new, type_code == MINSERT_TYPE, kept)?
        }
        b"TOLERANCE" => {
            let end = entity_fields(data, Release::R2000)?[3];
            tolerance(&mut fields, old, new, data, [start, end], Release::R2000)?;
        }
        b"LEADER" => leader(&mut fields, new, data, start)?,
        b"MLINE" => mline(&mut fields, new)?,
        b"VIEWPORT" => {
            let end = entity_fields(data, Release::R2000)?[3];
            viewport(&mut fields, old, new, data, [start, end], Release::R2000)?;
        }
        b"SHAPE" => {
            let end = entity_fields(data, Release::R2000)?[3];
            shape(&mut fields, new, data, [start, end], Release::R2000)?;
        }
        // SEQEND は型ごとの欄を持たない
        b"SEQEND" | b"ENDBLK" => {}
        b"BLOCK" => string(&mut fields, new, 2)?,
        _ => return None,
    }
    Some(fields)
}

/// R2004 で後ろに欄が増えた種類（属性を持つ INSERT の、持つオブジェクトの数（BL）と、MTEXT の背景の欄）の
/// 型ごとの欄。R2000 の形で符号化し、増えた欄は元のビットを写す。元の値を R2000 の形で符号化した欄が、元の
/// ビットの初めと同じでなければ `None`（決定 0073）
fn r2004_own_fields(
    writer: fn() -> BitWriter,
    old: &Object,
    new: &Object,
    type_code: i16,
    data: &[u8],
    start: usize,
    handles_at: usize,
) -> Option<BitWriter> {
    let mut fields = own_fields(writer(), old, new, type_code, data, start)?;
    let same = own_fields(writer(), old, old, type_code, data, start)?;
    let end = start
        .checked_add(same.len())
        .filter(|&end| end <= handles_at)?;
    let mut found = BitWriter::default();
    found.copy(data, start..end)?;
    (found.into_bytes() == same.into_bytes()).then_some(())?;
    fields.copy(data, end..handles_at)?;
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

/// R2013 からの、制御点を書かないスプライン（場合が 1 で、印の 1 が立つ）の欄（決定 0103）。読む側は、3 次で
/// 当てはめる点が 2 つのものを、両端の点と接線から Bézier の制御点に直して置く。その形（3 次・節点が
/// 0 と 1 の 4 つずつ・重みのない 4 つの制御点・許容差が 1e-9 と 1e-10）の値だけを受け付け、両端の制御点を
/// 当てはめる点に、両端の制御点との差の 3 倍を接線に戻して書く。両端の 2 つの制御点が変わらなければ、元の
/// 接線のビットを写す。印・節点の取り方・当てはめる許容差は元のビットを写す
fn fit_bezier(
    fields: &mut BitWriter,
    object: &Object,
    data: &[u8],
    start: usize,
    release: Release,
) -> Option<()> {
    let mut bits = Bits::with_release(data, release);
    bits.seek(start);
    bits.bl()?;
    let method_at = bits.pos();
    bits.bl()?;
    bits.bl()?;
    let method_end = bits.pos();
    (bits.bl()? == 3).then_some(())?;
    let tolerance_at = bits.pos();
    bits.bd()?;
    let tangents_at = bits.pos();
    let point =
        |bits: &mut Bits| -> Option<[f64; 3]> { Some([bits.bd()?, bits.bd()?, bits.bd()?]) };
    let (start_tangent, end_tangent) = (point(&mut bits)?, point(&mut bits)?);
    (bits.bl()? == 2).then_some(())?;
    let (first, last) = (point(&mut bits)?, point(&mut bits)?);
    // 値は、読む側が置く形でなければならない
    let codes: Vec<_> = object
        .values
        .iter()
        .filter(|value| matches!(value.code, 10..=13 | 20..=23 | 30..=33 | 40..=44 | 71..=74))
        .map(|value| (value.code, &value.data))
        .collect();
    let int = |data: &Data| match data {
        Data::Int(value) => Some(*value),
        _ => None,
    };
    let float = |data: &Data| match data {
        Data::Real(value) => Some(*value),
        _ => None,
    };
    let [
        (71, degree),
        (72, knots),
        (73, controls),
        (74, fit),
        (42, knot_tolerance),
        (43, control_tolerance),
        rest @ ..,
    ] = codes.as_slice()
    else {
        return None;
    };
    ([int(degree)?, int(knots)?, int(controls)?, int(fit)?] == [3, 8, 4, 0]
        && float(knot_tolerance)? == 1e-9
        && float(control_tolerance)? == 1e-10)
        .then_some(())?;
    let [
        knots @ ..,
        p0x,
        p0y,
        p0z,
        p1x,
        p1y,
        p1z,
        p2x,
        p2y,
        p2z,
        p3x,
        p3y,
        p3z,
    ] = rest
    else {
        return None;
    };
    (knots.len() == 8
        && knots
            .iter()
            .zip([0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0])
            .all(|((code, data), knot)| *code == 40 && float(data) == Some(knot)))
    .then_some(())?;
    let mut points = Vec::new();
    for xyz in [
        [p0x, p0y, p0z],
        [p1x, p1y, p1z],
        [p2x, p2y, p2z],
        [p3x, p3y, p3z],
    ] {
        let [(10, x), (20, y), (30, z)] = xyz.map(|(code, data)| (*code, *data)) else {
            return None;
        };
        points.push([float(x)?, float(y)?, float(z)?]);
    }
    let [p0, p1, p2, p3] = points.as_slice() else {
        return None;
    };
    // 読む側と同じく、元の接線から求めた制御点
    let near = |end: [f64; 3], tangent: [f64; 3], sign: f64| -> [f64; 3] {
        let mut out = end;
        for (v, t) in out.iter_mut().zip(tangent) {
            *v += sign * t / 3.0;
        }
        out
    };
    let same =
        |a: &[f64; 3], b: &[f64; 3]| a.iter().zip(b).all(|(a, b)| a.to_bits() == b.to_bits());
    fields.bl(1);
    fields.copy(data, method_at..method_end)?;
    fields.bl(3);
    fields.copy(data, tolerance_at..tangents_at)?;
    for (end, inner, tangent, sign) in [(p0, p1, start_tangent, 1.0), (p3, p2, end_tangent, -1.0)] {
        let original = if sign > 0.0 { first } else { last };
        if same(end, &original) && same(inner, &near(original, tangent, sign)) {
            for v in tangent {
                fields.bd(v);
            }
        } else {
            for (inner, end) in inner.iter().zip(end) {
                fields.bd(sign * (inner - end) * 3.0);
            }
        }
    }
    fields.bl(2);
    for v in p0.iter().chain(p3) {
        fields.bd(*v);
    }
    Some(())
}

/// SPLINE の欄（R2000）。場合（scenario）は元のままとし、1 なら節点と制御点、2 なら当てはめる点を書く。
/// 場合に合わない値があれば `None`。有理・閉じている・周期的の印は、印（70）があればその 4・1・2 を書き、
/// なければ元のビットを写す。重みがあるかの印は 41 があるかから選ぶ。接線（12・13）がなければ 0 を書く。足した SPLINE（`data` が空）は、
/// 当てはめる点（11）を持てば場合を 2、持たなければ 1 とし、閉じている・周期的の印を 0、有理の印を
/// 重みがあるかにする（決定 0057・0059）
fn spline(
    fields: &mut BitWriter,
    object: &Object,
    data: &[u8],
    start: usize,
    release: Release,
) -> Option<()> {
    let added = data.is_empty();
    let r2013 = release >= Release::R2013;
    let (scenario, flags_at, method) = if added {
        let fitted = object.values.iter().any(|value| value.code == 11);
        (if fitted { 2 } else { 1 }, 0, None)
    } else {
        let mut bits = Bits::with_release(data, release);
        bits.seek(start);
        let scenario = bits.bl()?;
        // R2013 からは、場合の後にスプラインの印と節点の取り方（BL）が続く。印の 1 が立つ制御点の
        // スプラインは、読む側が当てはめる点から制御点を求めて置くので、当てはめる点に戻して書く（決定 0103）
        let method = if r2013 {
            let at = bits.pos();
            let flags = bits.bl()?;
            bits.bl()?;
            if scenario == 1 && flags & 1 != 0 {
                return fit_bezier(fields, object, data, start, release);
            }
            Some(at..bits.pos())
        } else {
            None
        };
        bits.bl()?;
        (scenario, bits.pos(), method)
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
    match method {
        Some(range) => fields.copy(data, range)?,
        // 足すのは制御点のスプラインだけとし、試料の図面と同じく印を 0、節点の取り方を 15 とする
        None if r2013 => {
            (scenario == 1).then_some(())?;
            fields.bl(0);
            fields.bl(15);
        }
        None => {}
    }
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
            match integer(object, 70)? {
                Some(flags) => {
                    for bit in [4, 1, 2] {
                        fields.bit(flags & bit != 0);
                    }
                }
                None if added => {
                    for on in [weighted, false, false] {
                        fields.bit(on);
                    }
                }
                None => fields.copy(data, flags_at..flags_at + 3)?,
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
    release: Release,
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
    // R2010 からは、初めの寸法の版（RC）と、測った値の後の 3 つの B を写し、文字列は文字列の流れに書く
    // （決定 0088）
    let r2010 = release >= Release::R2010;
    let mut bits = Bits::with_release(data, release);
    bits.seek(start);
    let version_at = bits.pos();
    if r2010 {
        bits.rc()?;
    }
    let version = version_at..bits.pos();
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
    let arrows_at = bits.pos();
    if r2010 {
        for _ in 0..3 {
            bits.bit()?;
        }
    }
    let arrows = arrows_at..bits.pos();
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
    fields.copy(data, version)?;
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
    if texts(old) == texts(new) && !r2010 {
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
    fields.copy(data, arrows)?;
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

/// 元の INSERT・MINSERT の欄の回転（ラジアン）。元のデータがなければ `None`
fn insert_rotation(data: &[u8], start: usize) -> Option<f64> {
    let mut bits = Bits::new(data);
    bits.seek(start);
    for _ in 0..3 {
        bits.bd()?;
    }
    match bits.bb()? {
        3 => {}
        1 => {
            bits.dd(1.0)?;
            bits.dd(1.0)?;
        }
        2 => {
            bits.rd()?;
        }
        _ => {
            let x = bits.rd()?;
            bits.dd(x)?;
            bits.dd(x)?;
        }
    }
    bits.bd()
}

/// INSERT・MINSERT の欄（R2000）。尺度（41〜43）がなければ 1、回転（50）がなければ 0、押し出しの向きが
/// なければ (0, 0, 1) を書く。回転が変わらなければ、元のラジアン `kept` を書く。尺度の印（BB）は、尺度の値から
/// 選ぶ。属性が続くかの印は、66 から書く
fn insert(fields: &mut BitWriter, object: &Object, grid: bool, kept: Option<f64>) -> Option<()> {
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
    match kept {
        Some(rotation) => fields.bd(rotation),
        None => fields.bd(real(object, 50)?.unwrap_or(0.0).to_radians()),
    }
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
    [start, end]: [usize; 2],
    release: Release,
) -> Option<()> {
    let mut bits = Bits::with_release(data, release);
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
    // R2010 からは、文字列は文字列の流れに書く（決定 0088）
    if text_of(old, 1) == text_of(new, 1) && release < Release::R2010 {
        fields.copy(data, text_at..text_end)?;
    } else {
        string(fields, new, 1)?;
    }
    fields.copy(data, text_end..end)
}

/// LEADER の欄（決定 0040）。注釈の種類（73）・経路の種類（72、なければ 0）・頂点の数（76）と頂点（10）・
/// 押し出しの向き（なければ (0, 0, 1)）・X 軸の向き（211、なければ (1, 0, 0)）・注釈の高さと幅（40・41）・
/// 鉤の線の向き（74）を書く。頂点が変わったら、表にない終わりの点の射影は終わりの頂点にする。表にない
/// 鉤の線の点と差の欄と、後ろの欄は、元のビットを写す
fn leader(fields: &mut BitWriter, object: &Object, data: &[u8], start: usize) -> Option<()> {
    let mut bits = Bits::with_release(data, Release::R2000);
    bits.seek(start);
    bits.bit()?;
    let kinds_at = bits.pos();
    bits.bs()?;
    bits.bs()?;
    let points_at = bits.pos();
    let count = bits.bl()?;
    let mut old = Vec::new();
    for _ in 0..count {
        old.push([bits.bd()?, bits.bd()?, bits.bd()?]);
    }
    for _ in 0..3 {
        bits.bd()?;
    }
    let points_end = bits.pos();
    let mut values = Cursor(
        object
            .values
            .iter()
            .filter(|value| matches!(value.code, 76 | 10 | 20 | 30))
            .collect::<Vec<_>>()
            .into_iter()
            .peekable(),
    );
    let mut vertices = Vec::new();
    for _ in 0..values.int(76)? {
        vertices.push([values.real(10)?, values.real(20)?, values.real(30)?]);
    }
    if values.0.next().is_some() {
        return None;
    }
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
    if vertices == old {
        fields.copy(data, points_at..points_end)?;
    } else {
        let last = *vertices.last()?;
        fields.bl(i32::try_from(vertices.len()).ok()?);
        for point in vertices.iter().chain([&last]) {
            for value in point {
                fields.bd(*value);
            }
        }
    }
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
    fields.copy(data, rest_at..entity_fields(data, Release::R2000)?[3])
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
    [start, end]: [usize; 2],
    release: Release,
) -> Option<()> {
    // R2010 からは、グリッドの向き（72）の後に主なグリッドの線の間隔（BS、61）を書き、文字列は文字列の
    // 流れに書く（決定 0088）
    let r2010 = release >= Release::R2010;
    let mut bits = Bits::with_release(data, release);
    bits.seek(start);
    for _ in 0..17 {
        bits.bd()?;
    }
    for _ in 0..8 {
        bits.rd()?;
    }
    bits.bs()?;
    if r2010 {
        bits.bs()?;
    }
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
    if r2010 {
        fields.bs(small(new, 61)??);
    }
    fields.copy(data, frozen_at..frozen_end)?;
    fields.bl(i32::try_from(integer(new, 90)??).ok()?);
    if text_of(old, 1) == text_of(new, 1) && !r2010 {
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
    fields.copy(data, rest_at..end)
}

/// SHAPE の欄（決定 0042）。角度（50・51）がなければ 0、幅の尺度（41）がなければ 1、厚さ（39）がなければ 0、
/// 押し出しの向きがなければ (0, 0, 1) を書く。角度は度からラジアンに直す。表にない形の番号と後ろの欄は、
/// 元のビットを写す
fn shape(
    fields: &mut BitWriter,
    object: &Object,
    data: &[u8],
    [start, end]: [usize; 2],
    release: Release,
) -> Option<()> {
    let mut bits = Bits::with_release(data, release);
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
    fields.copy(data, rest_at..end)
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
