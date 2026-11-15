//! 1 つの DWG オブジェクトの読み方と差分書き戻しの共通入口。
//!
//! `AcadDb`（全オブジェクトの `Vec<Object>`）を常駐させずに、表のレコード・クラス・ヘッダーだけを軽く索引化し、
//! 個々のレコードを並列に復号したり、1 図形の変更・追加・削除を差分で符号化したりするために使う。

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;

use zumen_base::{Cx, Diagnostic, Kind, Ref, codes};
use zumen_sdk::{Codec, Error, Transact, Transaction};

use super::bits::{Bits, modular_char, push_modular_char};
use super::header::{self, Header};
use super::kinds::{
    BLOCK, BLOCK_RECORD, DICTIONARY, DIMSTYLE, LAYER, LTYPE, MLINESTYLE, STYLE, VIEWPORT,
};
use super::objects::{
    BLOCK_FIELDS, COMMON_ENTITY, Fields, code_page_name, is_table_record, kind as builtin_kind,
    number_anonymous, read as read_fields,
};
use super::objects::{entity_data_entmode, skip_extended_data};
use super::write_back::{can_unlink_single, encode_born_entities, encode_single_entity};
use super::{COMMON_OBJECT, Dwg, DwgFile, Entry, Release, stopped, stopped_at};
use crate::db::{Data, Object, Origin, Section, Value};

/// 符号化した追加レコードの列と次の `HANDSEED`
pub type EncodedBorn = (Vec<(u64, Vec<u8>)>, u64);

/// 差分書き戻しで更新する 1 レコードの `(handle, section, kind, old_values, new_values)`
pub type UpdatedDwgRecord = (u64, Section, Vec<u8>, Vec<Value>, Vec<Value>);

/// 図形レコードの所属空間の分類
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntitySpace {
    /// モデル空間の最上位の図形
    Model,
    /// ペーパー空間の最上位の図形（`entmode == 1`）
    Paper,
    /// 親の図形（INSERT や POLYLINE）に付く子レコード（モデル空間側）
    ModelFollower {
        /// 直接の所有者のハンドル
        owner: u64,
    },
    /// ブロック定義の中に属する図形（またはその子レコード）
    BlockDefinition,
}

/// 1 つのレコードの軽い見出し情報
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EntryMeta<'a> {
    /// オブジェクトのハンドル
    pub handle: u64,
    /// ファイル（または `AcDb:AcDbObjects`）内のバイト範囲
    pub span: Range<usize>,
    /// DXF の種類名（未知の型は空）
    pub kind: &'a [u8],
    /// 所属するセクション（図形がブロック定義に移る前の初期区分）
    pub section: Section,
}

/// `AcadDb` を常駐させずに DWG の個別レコードの復号と差分書き戻しを行う索引
#[derive(Clone, Debug)]
pub struct DwgSingle {
    release: Release,
    version: Vec<u8>,
    code_page: Option<&'static [u8]>,
    header: Option<Header>,
    handle_seed: Option<u64>,
    classes: BTreeMap<u16, (Vec<u8>, Section)>,
    names: BTreeMap<u64, (u16, Vec<u8>)>,
    by_table_name: BTreeMap<u16, BTreeMap<Vec<u8>, u64>>,
    books: BTreeMap<u64, Vec<Value>>,
    block_fields: BTreeMap<u64, Vec<Value>>,
    viewport_numbers: BTreeMap<usize, i64>,
    material_bylayer: Option<u64>,
    mtext_handles: BTreeSet<u64>,
    /// ハンドルからレコード番号を引くソート済み配列（重複時は最後のレコード番号）
    handle_to_record: Vec<(u64, u32)>,
    has_duplicate_handle: bool,
    /// 各レコードを直接の所有者（330）として指すオブジェクトの件数（255 以上は `owner_counts_large`）
    owner_counts: Vec<u8>,
    owner_counts_large: BTreeMap<usize, u32>,
    owner_counts_by_handle: BTreeMap<u64, u32>,
    /// モデル空間の `BLOCK_RECORD` の `(record, handle)`（追加・削除の書き戻しが可能なとき `Some`）
    model_block: Option<(usize, u64)>,
    /// モデル空間の `BLOCK_RECORD` の図形列に 1 度だけ現れるレコード番号のビット集合
    model_owned_bits: Vec<u64>,
}

fn bit_get(bits: &[u64], index: usize) -> bool {
    let word = index / 64;
    let bit = index % 64;
    bits.get(word).is_some_and(|w| (w >> bit) & 1 != 0)
}

fn bit_set(bits: &mut [u64], index: usize) {
    let word = index / 64;
    let bit = index % 64;
    if let Some(w) = bits.get_mut(word) {
        *w |= 1u64 << bit;
    }
}

fn bit_clear(bits: &mut [u64], index: usize) {
    let word = index / 64;
    let bit = index % 64;
    if let Some(w) = bits.get_mut(word) {
        *w &= !(1u64 << bit);
    }
}

fn table_code_of(name: &[u8]) -> Option<u16> {
    match name {
        b"LAYER" => Some(LAYER),
        b"LTYPE" => Some(LTYPE),
        b"STYLE" => Some(STYLE),
        b"DIMSTYLE" => Some(DIMSTYLE),
        b"BLOCK_RECORD" => Some(BLOCK_RECORD),
        _ => None,
    }
}

/// オブジェクトのデータから直接の所有者ハンドルだけを高速に読む
fn fast_direct_owner(
    data: &[u8],
    release: Release,
    entry: &Entry,
    entity: bool,
    header: Option<Header>,
) -> Option<u64> {
    let mut bits = Bits::with_release(data, release);
    let handles_at = match entry.handles {
        Some(at) => {
            bits.bot()?;
            bits.split_strings(at)?;
            at
        }
        None => {
            bits.bs()?;
            let at = usize::try_from(bits.rl()?).ok()?;
            if release == Release::R2007 {
                bits.split_strings(at)?;
            }
            at
        }
    };
    bits.handle()?;
    skip_extended_data(&mut bits)?;
    if entity {
        let entmode = entity_data_entmode(&mut bits, release)?;
        match entmode {
            0 => {
                bits.seek(handles_at);
                bits.handle()?.resolve(entry.handle)
            }
            1 => header.map(|h| h.paper),
            2 => header.map(|h| h.model),
            _ => None,
        }
    } else {
        bits.seek(handles_at);
        bits.handle()?.resolve(entry.handle)
    }
}

impl DwgSingle {
    /// `DwgFile` から軽い索引を組み立てる
    pub fn from_file(file: &DwgFile, cx: &mut Cx<'_>) -> Result<Self, Error> {
        let header = file
            .header_section()
            .and_then(|(src, span)| header::header(src, span, file.release, cx));
        let mut classes: BTreeMap<u16, (Vec<u8>, Section)> = BTreeMap::new();
        for class in &file.classes {
            cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
            classes.entry(class.number).or_insert_with(|| {
                let (name, sec) = class.kind();
                (name.to_vec(), sec)
            });
        }

        let n = file.entries.len();
        let mut handle_to_record: Vec<(u64, u32)> = Vec::with_capacity(n);
        let mut sorted = true;
        let mut prev_h = 0u64;
        for (idx, entry) in file.entries.iter().enumerate() {
            if idx > 0 && entry.handle <= prev_h {
                sorted = false;
            }
            prev_h = entry.handle;
            handle_to_record.push((entry.handle, u32::try_from(idx).unwrap_or(u32::MAX)));
        }
        let mut has_duplicate_handle = false;
        if !sorted {
            handle_to_record.sort_by_key(|&(h, _)| h);
            let mut deduped: Vec<(u64, u32)> = Vec::with_capacity(handle_to_record.len());
            for (h, rec) in handle_to_record {
                if let Some(last) = deduped.last_mut()
                    && last.0 == h
                {
                    last.1 = rec;
                    has_duplicate_handle = true;
                } else {
                    deduped.push((h, rec));
                }
            }
            handle_to_record = deduped;
        }

        // 表のレコード・DBCOLOR・DICTIONARY・VIEWPORT だけを先に読む
        let mut table_indices: Vec<usize> = Vec::new();
        let mut table_fields: Vec<Option<Fields>> = Vec::new();
        let mut dbcolor_entries: Vec<(usize, Fields)> = Vec::new();
        let mut dict_entries: Vec<(usize, Fields)> = Vec::new();
        let mut viewport_records: Vec<usize> = Vec::new();
        let mut mtext_handles: BTreeSet<u64> = BTreeSet::new();

        for (idx, entry) in file.entries.iter().enumerate() {
            let place = Ref::at_bytes(entry.span.start as u64, entry.span.end as u64);
            cx.check(Kind::Work, 1)
                .map_err(|stop| stopped_at(cx, stop, place))?;
            let resolved = builtin_kind(entry.type_code).or_else(|| {
                classes
                    .get(&entry.type_code)
                    .map(|(name, sec)| (name.as_slice(), sec.clone()))
            });
            let Some((kname, sec)) = resolved else {
                continue;
            };
            if kname == b"MTEXT" {
                mtext_handles.insert(entry.handle);
            }
            if entry.type_code == VIEWPORT {
                viewport_records.push(idx);
            }
            if is_table_record(entry.type_code) || entry.type_code == MLINESTYLE {
                let entity = matches!(sec, Section::Entities | Section::Blocks);
                let fields =
                    read_fields(file.data(entry), file.release, entry, kname, entity, header);
                table_indices.push(idx);
                table_fields.push(fields);
            } else if kname == b"DBCOLOR" {
                if let Some(fields) =
                    read_fields(file.data(entry), file.release, entry, kname, false, header)
                {
                    dbcolor_entries.push((idx, fields));
                }
            } else if (entry.type_code == DICTIONARY || kname == b"ACDBDICTIONARYWDFLT")
                && let Some(fields) =
                    read_fields(file.data(entry), file.release, entry, kname, false, header)
            {
                dict_entries.push((idx, fields));
            }
        }

        if let Some(hdr) = header {
            let anon_iter = table_indices
                .iter()
                .zip(table_fields.iter_mut())
                .filter_map(|(&idx, fields)| {
                    let entry = file.entries.get(idx)?;
                    (entry.type_code == BLOCK_RECORD
                        && entry.handle != hdr.model
                        && entry.handle != hdr.paper)
                        .then_some(fields.as_mut())
                });
            number_anonymous(anon_iter);
        }

        let mut names: BTreeMap<u64, (u16, Vec<u8>)> = BTreeMap::new();
        let mut by_table_name: BTreeMap<u16, BTreeMap<Vec<u8>, u64>> = BTreeMap::new();
        for (&idx, fields_opt) in table_indices.iter().zip(&table_fields) {
            let Some(entry) = file.entries.get(idx) else {
                continue;
            };
            let Some(fields) = fields_opt.as_ref() else {
                continue;
            };
            let Some(name_val) = fields.values.iter().find(|v| v.code == 2) else {
                continue;
            };
            if let Data::Text(name_bytes) = &name_val.data {
                names.insert(entry.handle, (entry.type_code, name_bytes.clone()));
                if is_table_record(entry.type_code) {
                    by_table_name
                        .entry(entry.type_code)
                        .or_default()
                        .entry(name_bytes.to_ascii_lowercase())
                        .or_insert(entry.handle);
                }
            }
        }

        let mut books: BTreeMap<u64, Vec<Value>> = BTreeMap::new();
        for (idx, fields) in dbcolor_entries {
            if let Some(entry) = file.entries.get(idx) {
                let book: Vec<Value> = fields
                    .values
                    .iter()
                    .filter(|v| matches!(v.code, 420 | 430))
                    .cloned()
                    .collect();
                books.insert(entry.handle, book);
            }
        }

        // BLOCK_RECORD から BLOCK へ移すフィールドを先に分けておく
        let mut block_fields: BTreeMap<u64, Vec<Value>> = BTreeMap::new();
        let mut quiet_diag = zumen_base::Diagnostics::new();
        let mut quiet_cx = Cx::new(cx.budget, &mut quiet_diag, cx.cancel);
        for (&idx, fields_opt) in table_indices.iter().zip(table_fields) {
            let Some(entry) = file.entries.get(idx) else {
                continue;
            };
            if entry.type_code != BLOCK_RECORD {
                continue;
            }
            let Some(fields) = fields_opt else {
                continue;
            };
            let at = Ref::at_bytes(entry.span.start as u64, entry.span.end as u64);
            let (own, _) = fields.place(&names, &books, entry, &mut quiet_cx, &at);
            let mut seen_70 = false;
            let mut blk = Vec::new();
            for val in own {
                let first_70 = val.code != 70 || !seen_70;
                seen_70 |= val.code == 70;
                if first_70 && BLOCK_FIELDS.contains(&val.code) {
                    blk.push(val);
                }
            }
            block_fields.insert(entry.handle, blk);
        }

        // DICTIONARY から ACAD_MATERIAL -> ByLayer を引く
        let mut dicts: Vec<(u64, Vec<Value>)> = Vec::with_capacity(dict_entries.len());
        for (idx, fields) in dict_entries {
            if let Some(entry) = file.entries.get(idx) {
                let at = Ref::at_bytes(entry.span.start as u64, entry.span.end as u64);
                let (own, _) = fields.place(&names, &books, entry, &mut quiet_cx, &at);
                dicts.push((entry.handle, own));
            }
        }
        let dict_item = |vals: &[Value], target: &[u8]| -> Option<u64> {
            vals.windows(2).find_map(|pair| match pair {
                [
                    Value {
                        code: 3,
                        data: Data::Text(found),
                    },
                    Value {
                        code: 350 | 360,
                        data: Data::Handle(h),
                    },
                ] if found.eq_ignore_ascii_case(target) => Some(*h),
                _ => None,
            })
        };
        let material_bylayer = dicts
            .iter()
            .find_map(|(_, vals)| dict_item(vals, b"ACAD_MATERIAL"))
            .and_then(|grp| dicts.iter().find(|(h, _)| *h == grp))
            .and_then(|(_, vals)| dict_item(vals, b"ByLayer"));

        // VIEWPORT の連番（69）
        let mut viewport_numbers: BTreeMap<usize, i64> = BTreeMap::new();
        let mut vp_by_owner: BTreeMap<u64, i64> = BTreeMap::new();
        for idx in viewport_records {
            if let Some(entry) = file.entries.get(idx)
                && let Some(owner) =
                    fast_direct_owner(file.data(entry), file.release, entry, true, header)
            {
                let num = vp_by_owner.entry(owner).or_default();
                *num += 1;
                viewport_numbers.insert(idx, *num);
            }
        }

        // 所有者の参照カウント（entmode == 1・2 は空間の BLOCK_RECORD を指すので飛ばす）
        let mut owner_counts = vec![0u8; n];
        let mut owner_counts_large: BTreeMap<usize, u32> = BTreeMap::new();
        let mut owner_counts_by_handle: BTreeMap<u64, u32> = BTreeMap::new();
        let lookup_rec = |h: u64| -> Option<usize> {
            handle_to_record
                .binary_search_by_key(&h, |&(k, _)| k)
                .ok()
                .and_then(|i| handle_to_record.get(i))
                .map(|&(_, r)| r as usize)
        };
        let space_handles = (header.map(|h| h.model), header.map(|h| h.paper));
        for entry in &file.entries {
            let sec_opt = builtin_kind(entry.type_code)
                .map(|(_, s)| s)
                .or_else(|| classes.get(&entry.type_code).map(|(_, s)| s.clone()));
            let Some(sec) = sec_opt else {
                continue;
            };
            let entity = matches!(sec, Section::Entities | Section::Blocks);
            if entity {
                let mut bits = Bits::with_release(file.data(entry), file.release);
                let handles_at = match entry.handles {
                    Some(at) => {
                        if bits.bot().is_none() || bits.split_strings(at).is_none() {
                            continue;
                        }
                        at
                    }
                    None => {
                        if bits.bs().is_none() {
                            continue;
                        }
                        let Some(at) = bits.rl().and_then(|v| usize::try_from(v).ok()) else {
                            continue;
                        };
                        if file.release == Release::R2007 && bits.split_strings(at).is_none() {
                            continue;
                        }
                        at
                    }
                };
                if bits.handle().is_none() || skip_extended_data(&mut bits).is_none() {
                    continue;
                }
                let Some(entmode) = entity_data_entmode(&mut bits, file.release) else {
                    continue;
                };
                if entmode != 0 {
                    continue;
                }
                bits.seek(handles_at);
                let Some(owner) = bits.handle().and_then(|h| h.resolve(entry.handle)) else {
                    continue;
                };
                if Some(owner) == space_handles.0 || Some(owner) == space_handles.1 {
                    continue;
                }
                if has_duplicate_handle {
                    *owner_counts_by_handle.entry(owner).or_default() += 1;
                } else if let Some(parent_rec) = lookup_rec(owner)
                    && let Some(slot) = owner_counts.get_mut(parent_rec)
                {
                    if *slot < u8::MAX {
                        *slot += 1;
                    } else {
                        let large = owner_counts_large.entry(parent_rec).or_insert(255);
                        *large = large.saturating_add(1);
                    }
                }
            } else if let Some(owner) =
                fast_direct_owner(file.data(entry), file.release, entry, false, header)
            {
                if Some(owner) == space_handles.0 || Some(owner) == space_handles.1 {
                    continue;
                }
                if has_duplicate_handle {
                    *owner_counts_by_handle.entry(owner).or_default() += 1;
                } else if let Some(parent_rec) = lookup_rec(owner)
                    && let Some(slot) = owner_counts.get_mut(parent_rec)
                {
                    if *slot < u8::MAX {
                        *slot += 1;
                    } else {
                        let large = owner_counts_large.entry(parent_rec).or_insert(255);
                        *large = large.saturating_add(1);
                    }
                }
            }
        }

        // モデル空間の BLOCK_RECORD の図形列を検証する
        let word_len = n.div_ceil(64);
        let mut model_owned_bits = vec![0u64; word_len];
        let mut model_owned_seen = vec![0u64; word_len];
        let model_block = header.and_then(|hdr| {
            let rec = lookup_rec(hdr.model)?;
            let entry = file.entries.get(rec)?;
            if entry.type_code != BLOCK_RECORD {
                return None;
            }
            let (_, name) = names.get(&hdr.model)?;
            if !name.eq_ignore_ascii_case(b"*Model_Space") {
                return None;
            }
            let owned_handles = file.model_block_owned_handles(rec, hdr.model, lookup_rec)?;
            for h in owned_handles {
                if let Some(ent_rec) = lookup_rec(h) {
                    if bit_get(&model_owned_seen, ent_rec) {
                        bit_clear(&mut model_owned_bits, ent_rec);
                    } else {
                        bit_set(&mut model_owned_seen, ent_rec);
                        bit_set(&mut model_owned_bits, ent_rec);
                    }
                }
            }
            Some((rec, hdr.model))
        });

        Ok(Self {
            release: file.release,
            version: file.version().to_vec(),
            code_page: code_page_name(file.code_page),
            header,
            handle_seed: file.handle_seed(),
            classes,
            names,
            by_table_name,
            books,
            block_fields,
            viewport_numbers,
            material_bylayer,
            mtext_handles,
            handle_to_record,
            has_duplicate_handle,
            owner_counts,
            owner_counts_large,
            owner_counts_by_handle,
            model_block,
            model_owned_bits,
        })
    }

    /// バージョンのバイト列（`AC1015`・`AC1032` など）
    pub fn version(&self) -> &[u8] {
        &self.version
    }

    /// DXF の `$DWGCODEPAGE` に対応するコード・ページ名
    pub fn code_page_name(&self) -> Option<&'static [u8]> {
        self.code_page
    }

    /// ヘッダーの `HANDSEED`
    pub fn handle_seed(&self) -> Option<u64> {
        self.handle_seed
    }

    /// モデル空間の `BLOCK_RECORD` のハンドル
    pub fn model_owner(&self) -> Option<u64> {
        self.header.map(|h| h.model)
    }

    /// 現在のペーパー空間の `BLOCK_RECORD` のハンドル
    pub fn paper_owner(&self) -> Option<u64> {
        self.header.map(|h| h.paper)
    }

    /// ヘッダーの `$INSUNITS`
    pub fn insunits(&self) -> Option<i64> {
        self.header.map(|h| i64::from(h.units))
    }

    /// ヘッダーの `$PLIMMIN`
    pub fn plimmin(&self) -> Option<[f64; 2]> {
        self.header.map(|h| [h.limits[0], h.limits[1]])
    }

    /// ヘッダーの `PDMODE`
    pub fn point_mode(&self) -> Option<i16> {
        self.header.map(|h| h.point_mode)
    }

    /// ヘッダーの `PDSIZE`
    pub fn point_size(&self) -> Option<f64> {
        self.header.map(|h| h.point_size)
    }

    /// `DwgFile::objects` と同じ規則でヘッダー変数を `db` に設定する
    pub fn populate_header_variables(&self, db: &mut crate::db::AcadDb) {
        db.set_variable(
            b"$ACADVER",
            vec![Value {
                code: 1,
                data: Data::Text(self.version.clone()),
            }],
        );
        if let Some(name) = self.code_page {
            db.set_variable(
                b"$DWGCODEPAGE",
                vec![Value {
                    code: 3,
                    data: Data::Text(name.to_vec()),
                }],
            );
        }
        if let Some(header) = self.header {
            let [x0, y0, x1, y1] = header.limits;
            db.set_variable(
                b"$INSUNITS",
                vec![Value {
                    code: 70,
                    data: Data::Int(i64::from(header.units)),
                }],
            );
            db.set_variable(
                b"$PLIMMIN",
                vec![
                    Value {
                        code: 10,
                        data: Data::Real(x0),
                    },
                    Value {
                        code: 20,
                        data: Data::Real(y0),
                    },
                ],
            );
            db.set_variable(
                b"$PLIMMAX",
                vec![
                    Value {
                        code: 10,
                        data: Data::Real(x1),
                    },
                    Value {
                        code: 20,
                        data: Data::Real(y1),
                    },
                ],
            );
            db.set_variable(
                b"$LTSCALE",
                vec![Value {
                    code: 40,
                    data: Data::Real(header.line_type_scale),
                }],
            );
            db.set_variable(
                b"$PDMODE",
                vec![Value {
                    code: 70,
                    data: Data::Int(i64::from(header.point_mode)),
                }],
            );
            db.set_variable(
                b"$PDSIZE",
                vec![Value {
                    code: 40,
                    data: Data::Real(header.point_size),
                }],
            );
        }
    }

    /// ハンドル重複が元のオブジェクト・マップにあったか
    pub fn has_duplicate_handle(&self) -> bool {
        self.has_duplicate_handle
    }

    /// `DwgFile` とこの索引が保持しているおおよそのヒープ・バイト数
    pub fn backing_bytes(&self, file: &DwgFile) -> usize {
        file.heap_bytes()
            + self.handle_to_record.capacity() * core::mem::size_of::<(u64, u32)>()
            + self.owner_counts.capacity()
            + self.model_owned_bits.capacity() * core::mem::size_of::<u64>()
    }

    /// ハンドルからレコード番号を引く
    pub fn record_of_handle(&self, handle: u64) -> Option<usize> {
        self.handle_to_record
            .binary_search_by_key(&handle, |&(k, _)| k)
            .ok()
            .and_then(|i| self.handle_to_record.get(i))
            .map(|&(_, r)| r as usize)
    }

    /// ハンドルから表レコードの名前（生のバイト列）と型を引く
    pub fn table_names(&self) -> impl Iterator<Item = (&'static [u8], &[u8])> + '_ {
        self.names.values().filter_map(|(code, name)| {
            let table: &'static [u8] = match *code {
                LAYER => b"LAYER",
                LTYPE => b"LTYPE",
                STYLE => b"STYLE",
                BLOCK_RECORD => b"BLOCK_RECORD",
                DIMSTYLE => b"DIMSTYLE",
                _ => return None,
            };
            Some((table, name.as_slice()))
        })
    }

    /// 表の種類（`b"LAYER"` など）と名前（ASCII 大文字・小文字無視）から最初のハンドルを引く
    pub fn lookup_table_handle(&self, table: &[u8], name: &[u8]) -> Option<u64> {
        let code = table_code_of(table)?;
        let map = self.by_table_name.get(&code)?;
        if !name.iter().any(u8::is_ascii_uppercase) {
            return map.get(name).copied();
        }
        map.get(&name.to_ascii_lowercase()).copied()
    }

    /// `record` 番目のレコードの見出し情報を返す
    pub fn entry_meta<'a>(&'a self, file: &'a DwgFile, record: usize) -> Option<EntryMeta<'a>> {
        let entry = file.entries.get(record)?;
        let (kind, section) = match builtin_kind(entry.type_code) {
            Some((k, s)) => (k, s),
            None => match self.classes.get(&entry.type_code) {
                Some((k, s)) => (k.as_slice(), s.clone()),
                None => (&b""[..], Section::Other(Vec::new())),
            },
        };
        Some(EntryMeta {
            handle: entry.handle,
            span: entry.span.clone(),
            kind,
            section,
        })
    }

    /// `Section::Entities` のレコードの所属空間（モデル・ペーパー・モデルの子・ブロック定義）を高速に判定する
    pub fn classify_entity(&self, file: &DwgFile, record: usize) -> Option<EntitySpace> {
        let entry = file.entries.get(record)?;
        let mut bits = Bits::with_release(file.data(entry), self.release);
        let handles_at = match entry.handles {
            Some(at) => {
                bits.bot()?;
                bits.split_strings(at)?;
                at
            }
            None => {
                bits.bs()?;
                let at = usize::try_from(bits.rl()?).ok()?;
                if self.release == Release::R2007 {
                    bits.split_strings(at)?;
                }
                at
            }
        };
        bits.handle()?;
        skip_extended_data(&mut bits)?;
        let entmode = entity_data_entmode(&mut bits, self.release)?;
        match entmode {
            2 => Some(EntitySpace::Model),
            1 => Some(EntitySpace::Paper),
            0 => {
                bits.seek(handles_at);
                let direct = bits.handle()?.resolve(entry.handle)?;
                let Some(hdr) = self.header else {
                    return Some(EntitySpace::Model);
                };
                if direct == hdr.model {
                    return Some(EntitySpace::Model);
                }
                if direct == hdr.paper {
                    // entmode == 0 で paper を直接指す図形は 67 = 1 を持たないためモデル空間に残る
                    return Some(EntitySpace::Model);
                }
                if self.in_block_definition(file, Some(direct)) {
                    Some(EntitySpace::BlockDefinition)
                } else {
                    Some(EntitySpace::ModelFollower { owner: direct })
                }
            }
            _ => None,
        }
    }

    fn in_block_definition(&self, file: &DwgFile, start_owner: Option<u64>) -> bool {
        let Some(hdr) = self.header else {
            return false;
        };
        let mut owner = start_owner;
        for _ in 0..file.entries.len() {
            let Some(o) = owner else {
                break;
            };
            let Some(rec) = self.record_of_handle(o) else {
                break;
            };
            let Some(entry) = file.entries.get(rec) else {
                break;
            };
            let sec = builtin_kind(entry.type_code)
                .map(|(_, s)| s)
                .or_else(|| self.classes.get(&entry.type_code).map(|(_, s)| s.clone()));
            if sec != Some(Section::Entities) {
                break;
            }
            owner = fast_direct_owner(file.data(entry), self.release, entry, true, self.header);
        }
        owner.is_some_and(|o| o != hdr.model && o != hdr.paper)
    }

    /// `record` 番目のレコードを `DwgFile::objects` と同じ規則で 1 件だけ復号する（スレッド安全）
    pub fn decode_record(&self, file: &DwgFile, record: usize, cx: &mut Cx<'_>) -> Option<Object> {
        let entry = file.entries.get(record)?;
        let at = Ref::at_bytes(entry.span.start as u64, entry.span.end as u64);
        let resolved = builtin_kind(entry.type_code).or_else(|| {
            self.classes
                .get(&entry.type_code)
                .map(|(k, s)| (k.as_slice(), s.clone()))
        });
        let handle_code = if entry.type_code == DIMSTYLE { 105 } else { 5 };
        let mut values = vec![Value {
            code: handle_code,
            data: Data::Handle(entry.handle),
        }];
        let mut extra = Vec::new();
        let (kind_vec, mut section) = match resolved {
            Some((kname, sec)) => {
                let entity = matches!(sec, Section::Entities | Section::Blocks);
                let mut fields = read_fields(
                    file.data(entry),
                    self.release,
                    entry,
                    kname,
                    entity,
                    self.header,
                );
                if let (BLOCK_RECORD, Some(hdr), Some(f)) =
                    (entry.type_code, self.header, fields.as_mut())
                    && entry.handle != hdr.model
                    && entry.handle != hdr.paper
                    && let Some((_, numbered)) = self.names.get(&entry.handle)
                    && let Some(v2) = f.values.iter_mut().find(|v| v.code == 2)
                {
                    v2.data = Data::Text(numbered.clone());
                }
                match fields {
                    Some(f) => {
                        let direct_owner = f.owner;
                        let (mut own, xdata) = f.place(&self.names, &self.books, entry, cx, &at);
                        if entry.type_code == BLOCK_RECORD {
                            let mut seen_70 = false;
                            own.retain(|val| {
                                let first_70 = val.code != 70 || !seen_70;
                                seen_70 |= val.code == 70;
                                !(first_70 && BLOCK_FIELDS.contains(&val.code))
                            });
                        }
                        values.extend(own);
                        if entry.type_code == BLOCK
                            && let Some(owner) = direct_owner
                            && let Some(moved) = self.block_fields.get(&owner)
                        {
                            values.extend(moved.iter().cloned());
                        }
                        extra = xdata;
                    }
                    None => cx.report(
                        Diagnostic::new(codes::DWG_OBJECT_INVALID)
                            .with("type", i64::from(entry.type_code))
                            .at(at)
                            .cite(if entity { COMMON_ENTITY } else { COMMON_OBJECT }),
                    ),
                }
                (kname.to_vec(), sec)
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
        if let Some(&vp_num) = self.viewport_numbers.get(&record) {
            values.push(Value {
                code: 69,
                data: Data::Int(vp_num),
            });
        }
        if section == Section::Entities {
            let owner = values.iter().find_map(|v| match (v.code, &v.data) {
                (330, Data::Handle(h)) => Some(*h),
                _ => None,
            });
            if self.in_block_definition(file, owner) {
                section = Section::Blocks;
            }
        }
        values.extend(extra);
        Some(Object {
            kind: kind_vec,
            section,
            values,
            origin: Origin::Dwg {
                offset: entry.span.start,
                end: entry.span.end,
            },
        })
    }

    /// 元のファイル `file` の `record` 番目のオブジェクトを `DwgTxn::set_data` と同じバイト列で取り出す
    pub fn base_encoded(&self, file: &DwgFile, record: usize) -> Option<Vec<u8>> {
        let entry = file.entries.get(record)?;
        let data = file.data(entry);
        if self.release >= Release::R2010 {
            let bits =
                i64::try_from(data.len().checked_mul(8)?.checked_sub(entry.handles?)?).ok()?;
            let mut out = Vec::with_capacity(data.len() + 6);
            push_modular_char(&mut out, bits, false);
            out.extend_from_slice(data);
            Some(out)
        } else {
            Some(data.to_vec())
        }
    }

    /// `DwgTxn::set_data` / `DwgTxn::add` と同じ形の符号化済みバイト列を 1 件だけ復号する
    pub fn decode_encoded(
        &self,
        handle: u64,
        encoded: &[u8],
    ) -> Option<(Vec<u8>, Section, Vec<Value>)> {
        let (raw_data, handles, type_code) = if self.release >= Release::R2010 {
            let mut skip = 0;
            let bits = usize::try_from(modular_char(encoded, &mut skip, false)?).ok()?;
            let data = encoded.get(skip..)?;
            let at = data.len().checked_mul(8)?.checked_sub(bits)?;
            let code = u16::try_from(Bits::new(data).bot()?).ok()?;
            (data, Some(at), code)
        } else {
            let code = u16::try_from(Bits::new(encoded).bs()?).ok()?;
            (encoded, None, code)
        };
        let (kname, sec) = builtin_kind(type_code).or_else(|| {
            self.classes
                .get(&type_code)
                .map(|(k, s)| (k.as_slice(), s.clone()))
        })?;
        let entity = matches!(sec, Section::Entities | Section::Blocks);
        let entry = Entry {
            handle,
            span: 0..0,
            data: 0..raw_data.len(),
            handles,
            type_code,
        };
        let fields = read_fields(raw_data, self.release, &entry, kname, entity, self.header)?;
        let mut budget = zumen_base::Budget::default();
        let mut diag = zumen_base::Diagnostics::new();
        let cancel = zumen_base::CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        let at = Ref::default();
        let (own, xdata) = fields.place(&self.names, &self.books, &entry, &mut cx, &at);
        let handle_code = if type_code == DIMSTYLE { 105 } else { 5 };
        let mut values = vec![Value {
            code: handle_code,
            data: Data::Handle(handle),
        }];
        values.extend(own);
        values.extend(xdata);
        Some((kname.to_vec(), sec, values))
    }

    /// 1 図形の変更を符号化し直す
    pub fn encode_entity_edit(
        &self,
        current_encoded: &[u8],
        kind: &[u8],
        old_values: &[Value],
        new_values: &[Value],
    ) -> Option<Vec<u8>> {
        self.encode_record_edit(
            current_encoded,
            Section::Entities,
            kind,
            old_values,
            new_values,
        )
    }

    /// 1 レコード（図形またはオブジェクト）の変更を符号化し直す
    pub fn encode_record_edit(
        &self,
        current_encoded: &[u8],
        section: Section,
        kind: &[u8],
        old_values: &[Value],
        new_values: &[Value],
    ) -> Option<Vec<u8>> {
        encode_single_entity(
            self.release,
            current_encoded,
            section,
            kind,
            old_values,
            new_values,
            &|table, name| self.lookup_table_handle(table, name),
        )
    }

    /// モデル空間に足す 1 図形（と続く ATTRIB・SEQEND）を符号化する
    pub fn encode_born_model(
        &self,
        records: &[(&[u8], Vec<Value>)],
        seed: u64,
        is_removed: impl Fn(usize) -> bool,
    ) -> Option<EncodedBorn> {
        self.handle_seed?;
        let (_, model_owner) = self.model_block?;
        let objects: Vec<Object> = records
            .iter()
            .map(|(kind, values)| Object {
                kind: kind.to_vec(),
                section: Section::Entities,
                values: values.clone(),
                origin: Origin::New,
            })
            .collect();
        let (encoded, next_seed) = encode_born_entities(
            self.release,
            &objects,
            seed,
            model_owner,
            &|table, name| self.lookup_table_handle(table, name),
            self.material_bylayer,
            &|h| self.mtext_handles.contains(&h),
        )?;
        // 生きている既存のハンドルと衝突しないことを確かめる
        if encoded
            .iter()
            .any(|&(h, _)| self.record_of_handle(h).is_some_and(|r| !is_removed(r)))
        {
            return None;
        }
        Some((encoded, next_seed))
    }

    /// レコード `record` が持つ直接の子オブジェクトの総数
    fn total_children_of(&self, record: usize, handle: u64) -> u32 {
        if self.has_duplicate_handle {
            return self
                .owner_counts_by_handle
                .get(&handle)
                .copied()
                .unwrap_or(0);
        }
        match self.owner_counts.get(record).copied() {
            Some(u8::MAX) => self.owner_counts_large.get(&record).copied().unwrap_or(255),
            Some(c) => u32::from(c),
            None => 0,
        }
    }

    /// 既存のモデル空間図形 `record`（および一緒に除く所有オブジェクト `group_records`）を削除できるか
    pub fn can_unlink_model_entity(
        &self,
        file: &DwgFile,
        record: usize,
        current_encoded: &[u8],
        kind: &[u8],
        values: &[Value],
        group_records: &[(usize, u64, Option<u64>)],
    ) -> bool {
        let Some((_, model_owner)) = self.model_block else {
            return false;
        };
        if !bit_get(&self.model_owned_bits, record) {
            return false;
        }
        let Some(entry) = file.entries.get(record) else {
            return false;
        };
        let ent_owner = values.iter().find_map(|v| match (v.code, &v.data) {
            (330, Data::Handle(h)) => Some(*h),
            _ => None,
        });
        if ent_owner != Some(model_owner) {
            return false;
        }
        let mut group_handles = BTreeSet::new();
        for &(_, h, _) in group_records {
            group_handles.insert(h);
        }
        // `group_records` 内の各ハンドルについて、残る子オブジェクトが 1 件もないことを確かめる
        for &(rec, h, parent_opt) in group_records {
            if rec != record {
                let Some(p) = parent_opt else {
                    return false;
                };
                if !group_handles.contains(&p) {
                    return false;
                }
            }
            let total_children = self.total_children_of(rec, h);
            if total_children > 0 {
                let removed_children = group_records
                    .iter()
                    .filter(|&&(other_rec, _, other_parent)| {
                        other_rec != rec && other_parent == Some(h)
                    })
                    .count() as u32;
                if total_children > removed_children {
                    return false;
                }
            }
        }
        can_unlink_single(
            self.release,
            current_encoded,
            entry.handle,
            kind,
            values,
            |h| group_handles.contains(&h),
        )
    }

    /// 差分の変更を `DwgFile` の複製に反映して書き出したバイト列を返す
    #[allow(clippy::too_many_arguments)]
    pub fn save_file(
        &self,
        file: &DwgFile,
        staged: &BTreeMap<usize, Vec<u8>>,
        removed_records: &BTreeSet<usize>,
        removed_model_handles: &BTreeSet<u64>,
        added: &[(u64, Vec<u8>, bool)],
        new_seed: Option<u64>,
        cx: &mut Cx<'_>,
    ) -> Result<Vec<u8>, Error> {
        self.save_file_with_headers(
            file,
            staged,
            removed_records,
            removed_model_handles,
            added,
            new_seed,
            None,
            None,
            cx,
        )
    }

    /// ヘッダー変数（`PDMODE`・`PDSIZE`）の変更も含め、差分を 1 つの `DwgTxn` で反映して書き出す
    #[allow(clippy::too_many_arguments)]
    pub fn save_file_with_headers(
        &self,
        file: &DwgFile,
        staged: &BTreeMap<usize, Vec<u8>>,
        removed_records: &BTreeSet<usize>,
        removed_model_handles: &BTreeSet<u64>,
        added: &[(u64, Vec<u8>, bool)],
        new_seed: Option<u64>,
        point_mode: Option<i16>,
        point_size: Option<f64>,
        cx: &mut Cx<'_>,
    ) -> Result<Vec<u8>, Error> {
        if staged.is_empty()
            && removed_records.is_empty()
            && added.is_empty()
            && new_seed.is_none()
            && point_mode.is_none()
            && point_size.is_none()
        {
            let mut out = Vec::new();
            Dwg::write(file, &mut out, cx)?;
            return Ok(out);
        }
        let mut changes = staged.clone();
        let mut added_copy: Vec<(u64, Vec<u8>, bool)> = added.to_vec();
        let reuses_removed_handle = added_copy.iter().any(|(h, _, _)| {
            self.record_of_handle(*h)
                .is_some_and(|r| removed_records.contains(&r))
        });
        if reuses_removed_handle {
            if !removed_model_handles.is_empty() {
                let (block_record, block_handle) = self.model_block.ok_or(Error::Rejected)?;
                let mut no_added = Vec::new();
                file.patch_model_block_and_added(
                    block_record,
                    block_handle,
                    |h| self.record_of_handle(h),
                    &mut changes,
                    removed_model_handles,
                    &mut no_added,
                )
                .ok_or(Error::Rejected)?;
            }
            let mut updated = file.snapshot(cx)?;
            {
                let mut txn = updated.transaction();
                for (&rec, data) in &changes {
                    if !removed_records.contains(&rec) {
                        txn.set_data(rec, data)?;
                    }
                }
                for &rec in removed_records {
                    txn.remove(rec)?;
                }
                txn.commit(cx)?;
            }
            let mut add_changes = BTreeMap::new();
            if !added_copy.is_empty() {
                let (block_record, block_handle) = self.model_block.ok_or(Error::Rejected)?;
                let shifted_block = block_record - removed_records.range(..block_record).count();
                let handle_map: BTreeMap<u64, usize> = updated
                    .entries
                    .iter()
                    .enumerate()
                    .map(|(i, e)| (e.handle, i))
                    .collect();
                updated
                    .patch_model_block_and_added(
                        shifted_block,
                        block_handle,
                        |h| handle_map.get(&h).copied(),
                        &mut add_changes,
                        &BTreeSet::new(),
                        &mut added_copy,
                    )
                    .ok_or(Error::Rejected)?;
            }
            {
                let mut txn = updated.transaction();
                for (&rec, data) in &add_changes {
                    txn.set_data(rec, data)?;
                }
                for (handle, data, _) in &added_copy {
                    txn.add(*handle, data)?;
                }
                if let Some(seed) = new_seed {
                    txn.set_handle_seed(seed)?;
                }
                if let Some(mode) = point_mode {
                    txn.set_point_mode(mode)?;
                }
                if let Some(size) = point_size {
                    txn.set_point_size(size)?;
                }
                txn.commit(cx)?;
            }
            let mut out = Vec::new();
            Dwg::write(&updated, &mut out, cx)?;
            return Ok(out);
        }
        if !removed_model_handles.is_empty() || !added_copy.is_empty() {
            let (block_record, block_handle) = self.model_block.ok_or(Error::Rejected)?;
            file.patch_model_block_and_added(
                block_record,
                block_handle,
                |h| self.record_of_handle(h),
                &mut changes,
                removed_model_handles,
                &mut added_copy,
            )
            .ok_or(Error::Rejected)?;
        }
        let mut updated = file.snapshot(cx)?;
        let mut txn = updated.transaction();
        for (&rec, data) in &changes {
            if !removed_records.contains(&rec) {
                txn.set_data(rec, data)?;
            }
        }
        for &rec in removed_records {
            txn.remove(rec)?;
        }
        for (handle, data, _) in &added_copy {
            txn.add(*handle, data)?;
        }
        if let Some(seed) = new_seed {
            txn.set_handle_seed(seed)?;
        }
        if let Some(mode) = point_mode {
            txn.set_point_mode(mode)?;
        }
        if let Some(size) = point_size {
            txn.set_point_size(size)?;
        }
        txn.commit(cx)?;
        let mut out = Vec::new();
        Dwg::write(&updated, &mut out, cx)?;
        Ok(out)
    }

    /// 1 回の `apply` に相当する差分を `file` に確定させる（全体を書く保存で変更列を順に再現するのに使う）
    #[allow(clippy::too_many_arguments)]
    pub fn apply_fast_step(
        &self,
        file: &mut DwgFile,
        updated_records: &[UpdatedDwgRecord],
        removed_handles: &BTreeSet<u64>,
        removed_model_handles: &BTreeSet<u64>,
        added: &[(u64, Vec<u8>, bool)],
        new_seed: Option<u64>,
        point_mode: Option<i16>,
        point_size: Option<f64>,
        cx: &mut Cx<'_>,
    ) -> Result<(), Error> {
        if updated_records.is_empty()
            && removed_handles.is_empty()
            && added.is_empty()
            && new_seed.is_none()
            && point_mode.is_none()
            && point_size.is_none()
        {
            return Ok(());
        }
        let mut handle_map: BTreeMap<u64, usize> = BTreeMap::new();
        for (idx, entry) in file.entries.iter().enumerate() {
            handle_map.entry(entry.handle).or_insert(idx);
        }
        let mut changes: BTreeMap<usize, Vec<u8>> = BTreeMap::new();
        for (handle, section, kind, old_values, new_values) in updated_records {
            if old_values == new_values {
                continue;
            }
            let rec = *handle_map.get(handle).ok_or(Error::Rejected)?;
            let current_encoded = self.base_encoded(file, rec).ok_or(Error::Rejected)?;
            let bytes = self
                .encode_record_edit(
                    &current_encoded,
                    section.clone(),
                    kind,
                    old_values,
                    new_values,
                )
                .ok_or(Error::Rejected)?;
            changes.insert(rec, bytes);
        }
        let mut removed_records: BTreeSet<usize> = BTreeSet::new();
        for handle in removed_handles {
            let rec = *handle_map.get(handle).ok_or(Error::Rejected)?;
            removed_records.insert(rec);
        }
        let mut added_copy: Vec<(u64, Vec<u8>, bool)> = added.to_vec();
        if added_copy
            .iter()
            .any(|(h, _, _)| removed_handles.contains(h))
        {
            if !removed_model_handles.is_empty() {
                let (_, block_handle) = self.model_block.ok_or(Error::Rejected)?;
                let block_record = *handle_map.get(&block_handle).ok_or(Error::Rejected)?;
                let mut no_added = Vec::new();
                file.patch_model_block_and_added(
                    block_record,
                    block_handle,
                    |h| handle_map.get(&h).copied(),
                    &mut changes,
                    removed_model_handles,
                    &mut no_added,
                )
                .ok_or(Error::Rejected)?;
            }
            {
                let mut txn = file.transaction();
                for (&rec, data) in &changes {
                    if !removed_records.contains(&rec) {
                        txn.set_data(rec, data)?;
                    }
                }
                for &rec in &removed_records {
                    txn.remove(rec)?;
                }
                txn.commit(cx)?;
            }
            let mut post_map: BTreeMap<u64, usize> = BTreeMap::new();
            for (idx, entry) in file.entries.iter().enumerate() {
                post_map.entry(entry.handle).or_insert(idx);
            }
            let mut add_changes = BTreeMap::new();
            if !added_copy.is_empty() {
                let (_, block_handle) = self.model_block.ok_or(Error::Rejected)?;
                let block_record = *post_map.get(&block_handle).ok_or(Error::Rejected)?;
                file.patch_model_block_and_added(
                    block_record,
                    block_handle,
                    |h| post_map.get(&h).copied(),
                    &mut add_changes,
                    &BTreeSet::new(),
                    &mut added_copy,
                )
                .ok_or(Error::Rejected)?;
            }
            let mut txn = file.transaction();
            for (&rec, data) in &add_changes {
                txn.set_data(rec, data)?;
            }
            for (handle, data, _) in &added_copy {
                txn.add(*handle, data)?;
            }
            if let Some(seed) = new_seed {
                txn.set_handle_seed(seed)?;
            }
            if let Some(mode) = point_mode {
                txn.set_point_mode(mode)?;
            }
            if let Some(size) = point_size {
                txn.set_point_size(size)?;
            }
            return txn.commit(cx);
        }
        if !removed_model_handles.is_empty() || !added_copy.is_empty() {
            let (_, block_handle) = self.model_block.ok_or(Error::Rejected)?;
            let block_record = *handle_map.get(&block_handle).ok_or(Error::Rejected)?;
            file.patch_model_block_and_added(
                block_record,
                block_handle,
                |h| handle_map.get(&h).copied(),
                &mut changes,
                removed_model_handles,
                &mut added_copy,
            )
            .ok_or(Error::Rejected)?;
        }
        let mut txn = file.transaction();
        for (&rec, data) in &changes {
            if !removed_records.contains(&rec) {
                txn.set_data(rec, data)?;
            }
        }
        for &rec in &removed_records {
            txn.remove(rec)?;
        }
        for (handle, data, _) in &added_copy {
            txn.add(*handle, data)?;
        }
        if let Some(seed) = new_seed {
            txn.set_handle_seed(seed)?;
        }
        if let Some(mode) = point_mode {
            txn.set_point_mode(mode)?;
        }
        if let Some(size) = point_size {
            txn.set_point_size(size)?;
        }
        txn.commit(cx)
    }
}

#[cfg(test)]
mod tests {
    use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Limits};
    use zumen_sdk::{Codec, FileModel};

    use super::DwgSingle;
    use crate::Dwg;

    #[test]
    fn decode_record_matches_full_objects_across_releases() {
        let samples = [
            "../../samples/dwg/zumen-layout-r2000.dwg",
            "../../samples/dwg/zumen-minsert-r2000.dwg",
            "../../samples/dwg/zumen-points-a-r2000.dwg",
            "../../samples/dwg/zumen-layout-r2004.dwg",
            "../../samples/dwg/zumen-xclip-r2013.dwg",
            "../../samples/dwg/zumen-arcdim-r2018.dwg",
            "../../samples/dwg/zumen-layout-r2018.dwg",
            "../../samples/dwg/zumen-minsert-r2018.dwg",
            "../../samples/dwg/zumen-xclip-r2018.dwg",
        ];
        for rel in samples {
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(rel);
            let Ok(bytes) = std::fs::read(&path) else {
                continue;
            };
            let mut budget = Budget::new(Limits::default());
            let mut diag = Diagnostics::new();
            let cancel = CancelToken::new();
            let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
            let Ok(file) = Dwg::read(&bytes, &mut cx) else {
                continue;
            };

            let mut d1 = Diagnostics::new();
            let Ok(full) = file.objects(&mut Cx::new(&mut budget, &mut d1, &cancel)) else {
                continue;
            };
            let mut d2 = Diagnostics::new();
            let Ok(single) =
                DwgSingle::from_file(&file, &mut Cx::new(&mut budget, &mut d2, &cancel))
            else {
                continue;
            };
            assert_eq!(full.objects().len(), file.record_count());
            for rec in 0..file.record_count() {
                let decoded =
                    single.decode_record(&file, rec, &mut Cx::new(&mut budget, &mut d2, &cancel));
                assert_eq!(
                    decoded.as_ref(),
                    full.objects().get(rec),
                    "record {rec} mismatch in {rel}"
                );
            }
            assert_eq!(d1.len(), d2.len(), "diagnostics mismatch in {rel}");
        }
    }
}
