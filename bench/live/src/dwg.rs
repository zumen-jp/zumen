//! DWG（R2000〜R2018。R2007 は読み取りのみ）の並列の持ち上げと、レコード単位の差分書き戻し。
//!
//! `zumen_acad::dwg::single::DwgSingle` を使い、全レコードの `Vec<Object>` を常駐させずに 6 種類の図形
//! （LINE・CIRCLE・ARC・LWPOLYLINE・TEXT・INSERT）と ATTRIB を直接ページ列に持ち上げる。
//! 変更（移動・スタイル変更・追加・削除）は、変えたレコードだけを DWG のビット列に符号化し直して保持し、
//! 保存するときに元の `DwgFile` の複製へ `DwgTxn` で 1 回だけ当てる。

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::thread;

use zumen_acad::dwg::single::DwgSingle;
use zumen_acad::single::{self, BornRecords, CatalogShell, Followed, SingleCatalog, SingleTables};
use zumen_acad::{
    Charset, Data, Dwg, DwgFile, Origin, Section, Value, charset as charset_from_header,
};
use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Key, Ref};
use zumen_drawing::{
    Affine, Inherit, LayerId, LineTypeId, Unit, transform::transform as transform_shape,
};
use zumen_sdk::{Codec, FileModel};

use crate::display::{Display, DisplayGeomExt};
use crate::edit::History;
use crate::grid::Grid;
use crate::grow::GrowError;
use crate::lift::{
    self, AttribLift, AttribRow, BLOCK, DropReason, Geom, INSERT_FOLLOWED, Kind, Lift, LineTypeRef,
    NO_DEFINITION, Pools,
};
use crate::paged::{Paged, SourceBytes, TextSpan};
use crate::store::{
    Counts, Document, HandleIndex, HeaderState, NonSliceUsage, OpenDiagnostic, OpenDiagnosticKind,
    OpenError, PlainSummary, Row, Shapes, TABLE_KINDS, attach_family, bounds,
    combine_plain_summaries, merge_pools, shift_geom_pools,
};

/// バイト列が DWG の先頭印（`AC1012`〜`AC1032`）で始まるか
pub fn is_dwg_bytes(bytes: &[u8]) -> bool {
    bytes.get(..6).is_some_and(|head| {
        matches!(
            head,
            b"AC1012"
                | b"AC1014"
                | b"AC1015"
                | b"AC1018"
                | b"AC1021"
                | b"AC1024"
                | b"AC1027"
                | b"AC1032"
        )
    })
}

/// 2 つの DWG バイト列が、同じバージョン・コードページ・`HANDSEED` と、全レコードの同じビット列を持つか確かめる
pub fn same_dwg_state(a_bytes: &[u8], b_bytes: &[u8]) -> Result<(), String> {
    if a_bytes == b_bytes {
        return Ok(());
    }
    let read_one = |bytes: &[u8], label: &str| -> Result<DwgFile, String> {
        let mut budget = Budget::default();
        let mut diag = Diagnostics::new();
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        Dwg::read(bytes, &mut cx).map_err(|e| format!("{label} を DWG として読めない: {e:?}"))
    };
    let a = read_one(a_bytes, "文書エンジンの保存結果")?;
    let b = read_one(b_bytes, "今の実装の保存結果")?;
    if a.version() != b.version() {
        return Err(format!(
            "DWG のバージョンが違う（{:?} と {:?}）",
            a.version(),
            b.version()
        ));
    }
    if a.code_page() != b.code_page() {
        return Err(format!(
            "DWG のコードページが違う（{} と {}）",
            a.code_page(),
            b.code_page()
        ));
    }
    if a.handle_seed() != b.handle_seed() {
        return Err(format!(
            "DWG の HANDSEED が違う（{:?} と {:?}）",
            a.handle_seed(),
            b.handle_seed()
        ));
    }
    if a.entry_count() != b.entry_count() {
        return Err(format!(
            "DWG のオブジェクト数が違う（文書エンジン {} 件、今の実装 {} 件）",
            a.entry_count(),
            b.entry_count()
        ));
    }
    for idx in 0..a.entry_count() {
        let Some((ha, ta, hsa, _)) = a.entry_summary(idx) else {
            break;
        };
        let Some((hb, tb, hsb, _)) = b.entry_summary(idx) else {
            break;
        };
        if ha != hb || ta != tb || hsa != hsb {
            return Err(format!(
                "{idx} 番目のオブジェクトの見出しが違う（文書エンジン handle={ha:X} type={ta} handles={hsa:?}、今の実装 handle={hb:X} type={tb} handles={hsb:?}）",
            ));
        }
        let da = a.entry_raw_data(idx).unwrap_or_default();
        let db = b.entry_raw_data(idx).unwrap_or_default();
        if da != db {
            return Err(format!(
                "{idx} 番目（handle {ha:X}、type {ta}）のオブジェクトのビット列が違う（{} B と {} B）",
                da.len(),
                db.len()
            ));
        }
    }
    Ok(())
}

/// 変更された既存 DWG レコードの差分保持
#[derive(Clone, Debug)]
pub struct DwgRecordPatch {
    /// `DwgTxn::set_data` と同じ形の符号化済みバイト列
    pub encoded: Vec<u8>,
    /// レコードの種類名
    pub kind: Vec<u8>,
    /// 所属セクション
    pub section: Section,
    /// ラジアン往復後の値列
    pub values: Vec<Value>,
}

impl DwgRecordPatch {
    /// ヒープの保持バイト数
    pub fn heap_bytes(&self) -> usize {
        self.encoded.capacity() + self.kind.capacity() + self.values.capacity() * size_of::<Value>()
    }
}

/// 追加された DWG レコードの保持
#[derive(Clone, Debug)]
pub struct DwgBornRecord {
    /// 新しく振ったハンドル
    pub handle: u64,
    /// モデル空間の直接の子（`BLOCK_RECORD` に繋ぐ最上位図形）か
    pub is_top_model: bool,
    /// `DwgTxn::add` と同じ形の符号化済みバイト列
    pub encoded: Vec<u8>,
    /// レコードの種類名
    pub kind: Vec<u8>,
    /// ラジアン往復後の値列
    pub values: Vec<Value>,
}

impl DwgBornRecord {
    /// ヒープの保持バイト数
    pub fn heap_bytes(&self) -> usize {
        self.encoded.capacity() + self.kind.capacity() + self.values.capacity() * size_of::<Value>()
    }
}

/// 常駐する DWG の元データとレコード差分
pub struct DwgBacking {
    /// 元の DWG ファイルのバイト数
    pub raw_len: usize,
    /// 展開済みの `DwgFile`（コンテナーまたは元のバイト列とオブジェクトマップ）
    pub file: DwgFile,
    /// 単一レコード復号・再符号化用の索引
    pub single: DwgSingle,
    /// 今の `HANDSEED`
    pub handseed: Option<u64>,
    /// 変更された元のレコード（`record < counts.records`）
    pub patched: BTreeMap<u32, DwgRecordPatch>,
    /// 追加されたレコード（`record >= counts.records`）
    pub born: BTreeMap<u32, DwgBornRecord>,
}

impl DwgBacking {
    /// 差分のヒープバイト数
    pub fn edit_bytes(&self) -> usize {
        self.patched
            .values()
            .map(|p| size_of::<(u32, DwgRecordPatch)>() + p.heap_bytes())
            .sum::<usize>()
            + self
                .born
                .values()
                .map(|b| size_of::<(u32, DwgBornRecord)>() + b.heap_bytes())
                .sum::<usize>()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DwgOtherKind {
    Seqend,
    Lifted,
    Other,
}

enum DwgEntityItem {
    Lifted {
        rec: u32,
        lifted: lift::Lifted,
        has_xdict: bool,
    },
    Attrib {
        rec: u32,
        handle: Option<u64>,
        lift: AttribLift,
    },
    Paper {
        rec: u32,
        handle: Option<u64>,
        owner: Option<u64>,
        kind: Vec<u8>,
        values: Vec<Value>,
        origin: Origin,
    },
    Dropped {
        rec: u32,
        handle: Option<u64>,
        reason: DropReason,
    },
    Other {
        rec: u32,
        handle: Option<u64>,
        other_kind: DwgOtherKind,
        payload: Option<(Vec<u8>, Vec<Value>, Origin)>,
    },
}

type DwgCatalogItem = (u8, Vec<u8>, Vec<Value>, Origin, u32);

pub(crate) struct DwgPart {
    pools: Pools,
    cat_items: Vec<DwgCatalogItem>,
    table_raw_names: Vec<(&'static [u8], Vec<u8>)>,
    non_ent_handles: Vec<(u32, u64)>,
    object_handles: Vec<(u64, u32)>,
    forget_refs: Vec<(u64, u32)>,
    ent_family: Vec<(u32, Followed)>,
    plain: PlainSummary,
    has_spatial_filter: bool,
    entities: Vec<DwgEntityItem>,
}

/// 段階計測のために DWG の並列スキャン結果をまとめた中間状態
pub struct ScannedDwg {
    pub(crate) raw_len: usize,
    pub(crate) file: DwgFile,
    pub(crate) single: DwgSingle,
    pub(crate) charset: Charset,
    pub(crate) parts: Vec<DwgPart>,
}

impl ScannedDwg {
    #[cfg(test)]
    pub(crate) fn parts_len(&self) -> usize {
        self.parts.len()
    }
}

fn collect_forget_refs_in_values(
    kind: &[u8],
    values: &[Value],
    rec: u32,
    out: &mut Vec<(u64, u32)>,
) {
    let is_dict = kind == b"DICTIONARY";
    let mut in_group = false;
    for v in values {
        match (v.code, &v.data) {
            (102, Data::Text(t)) if t.starts_with(b"{") => in_group = true,
            (102, Data::Text(t)) if t.starts_with(b"}") => in_group = false,
            (330 | 340 | 350 | 360, Data::Handle(h)) if in_group => out.push((*h, rec)),
            (350 | 360, Data::Handle(h)) if !in_group && is_dict => out.push((*h, rec)),
            _ => {}
        }
    }
}

fn first_owner_in_values(values: &[Value]) -> Option<u64> {
    let mut depth = 0usize;
    for v in values {
        match (v.code, &v.data) {
            (102, Data::Text(t)) if t.starts_with(b"{") => depth += 1,
            (102, Data::Text(t)) if t.starts_with(b"}") => depth = depth.saturating_sub(1),
            (330, Data::Handle(h)) if depth == 0 => return Some(*h),
            _ => {}
        }
    }
    None
}

fn is_paper_values(values: &[Value]) -> bool {
    values.iter().find_map(|v| match (v.code, &v.data) {
        (67, Data::Int(n)) => Some(*n),
        _ => None,
    }) == Some(1)
}

pub(crate) fn scan_dwg_range(
    file: &DwgFile,
    single: &DwgSingle,
    charset: Charset,
    range: core::ops::Range<usize>,
) -> DwgPart {
    let mut pools = Pools::default();
    let mut cat_items = Vec::new();
    let mut table_raw_names = Vec::new();
    let mut non_ent_handles = Vec::new();
    let mut object_handles = Vec::new();
    let mut forget_refs = Vec::new();
    let mut ent_family = Vec::new();
    let mut plain = PlainSummary::default();
    let mut has_spatial_filter = false;
    let mut entities = Vec::new();

    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diag, &cancel);

    for rec in range {
        let Some(obj) = single.decode_record(file, rec, &mut cx) else {
            continue;
        };
        let rec32 = rec as u32;
        let obj_handle = obj.handle();

        if matches!(
            obj.section,
            Section::Tables | Section::Blocks | Section::Entities | Section::Objects
        ) {
            let has_102 = obj.values.iter().any(|v| v.code == 102);
            if has_102 || obj.kind == b"DICTIONARY" {
                collect_forget_refs_in_values(&obj.kind, &obj.values, rec32, &mut forget_refs);
            }
        }

        if let Some(&table_kind) = TABLE_KINDS.iter().find(|&&tk| tk == obj.kind.as_slice())
            && let Some(Data::Text(raw_name)) = obj.first(2)
        {
            table_raw_names.push((table_kind, raw_name.clone()));
        }

        match obj.section {
            Section::Tables => {
                if let Some(h) = obj_handle {
                    non_ent_handles.push((rec32, h));
                }
                cat_items.push((0, obj.kind, obj.values, obj.origin, rec32));
            }
            Section::Blocks => {
                if let Some(h) = obj_handle {
                    non_ent_handles.push((rec32, h));
                }
                cat_items.push((1, obj.kind, obj.values, obj.origin, rec32));
            }
            Section::Objects => {
                if let Some(h) = obj_handle {
                    non_ent_handles.push((rec32, h));
                    object_handles.push((h, rec32));
                }
                if obj.kind == b"SPATIAL_FILTER" {
                    has_spatial_filter = true;
                }
                if single::is_catalog_object(&obj.kind) || obj.kind == b"XRECORD" {
                    cat_items.push((2, obj.kind, obj.values, obj.origin, rec32));
                }
            }
            Section::Entities => {
                if let Some(role) = single::followed_object(&obj) {
                    plain.feed(role);
                    ent_family.push((rec32, role));
                } else {
                    plain.feed_non_family();
                }

                if let Some(slice_kind) = Kind::of(&obj.kind) {
                    match lift::lift_values(
                        charset,
                        slice_kind,
                        obj_handle,
                        &obj.values,
                        &mut pools,
                    ) {
                        Lift::Element(lifted) => {
                            let has_xdict = slice_kind == Kind::Insert
                                && obj.values.iter().any(|v| v.code == 102)
                                && obj.values.iter().any(|v| v.code == 360);
                            entities.push(DwgEntityItem::Lifted {
                                rec: rec32,
                                lifted,
                                has_xdict,
                            });
                        }
                        Lift::Paper => {
                            let owner = first_owner_in_values(&obj.values);
                            entities.push(DwgEntityItem::Paper {
                                rec: rec32,
                                handle: obj_handle,
                                owner,
                                kind: obj.kind,
                                values: obj.values,
                                origin: obj.origin,
                            });
                        }
                        Lift::Not(reason) => {
                            entities.push(DwgEntityItem::Dropped {
                                rec: rec32,
                                handle: obj_handle,
                                reason,
                            });
                        }
                    }
                } else if obj.kind == b"ATTRIB" {
                    if is_paper_values(&obj.values) {
                        let owner = first_owner_in_values(&obj.values);
                        entities.push(DwgEntityItem::Paper {
                            rec: rec32,
                            handle: obj_handle,
                            owner,
                            kind: obj.kind,
                            values: obj.values,
                            origin: obj.origin,
                        });
                    } else {
                        let lifted = lift::attrib_values(
                            charset,
                            obj_handle,
                            &obj.values,
                            rec32,
                            &mut pools,
                        );
                        entities.push(DwgEntityItem::Attrib {
                            rec: rec32,
                            handle: obj_handle,
                            lift: lifted,
                        });
                    }
                } else if is_paper_values(&obj.values) {
                    let owner = first_owner_in_values(&obj.values);
                    entities.push(DwgEntityItem::Paper {
                        rec: rec32,
                        handle: obj_handle,
                        owner,
                        kind: obj.kind,
                        values: obj.values,
                        origin: obj.origin,
                    });
                } else {
                    let other_kind = if obj.kind == b"SEQEND" {
                        DwgOtherKind::Seqend
                    } else if single::is_other_lifted_entity(&obj.kind) {
                        DwgOtherKind::Lifted
                    } else {
                        DwgOtherKind::Other
                    };
                    let payload = (other_kind == DwgOtherKind::Lifted)
                        .then_some((obj.kind, obj.values, obj.origin));
                    entities.push(DwgEntityItem::Other {
                        rec: rec32,
                        handle: obj_handle,
                        other_kind,
                        payload,
                    });
                }
            }
            Section::Classes | Section::Other(_) => {
                if let Some(h) = obj_handle {
                    non_ent_handles.push((rec32, h));
                }
            }
        }
    }

    DwgPart {
        pools,
        cat_items,
        table_raw_names,
        non_ent_handles,
        object_handles,
        forget_refs,
        ent_family,
        plain,
        has_spatial_filter,
        entities,
    }
}

pub(crate) fn prepare_dwg(bytes: &[u8]) -> Result<(DwgFile, DwgSingle, Charset), OpenError> {
    if bytes.len() >= u32::MAX as usize {
        return Err(OpenError::TooLarge);
    }
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
    let file = Dwg::read(bytes, &mut cx).map_err(|_| OpenError::InvalidDwg)?;
    if file.record_count() >= u32::MAX as usize {
        return Err(OpenError::TooLarge);
    }
    let single = DwgSingle::from_file(&file, &mut cx).map_err(|_| OpenError::InvalidDwg)?;
    let charset = charset_from_header(Some(single.version()), single.code_page_name());
    Ok((file, single, charset))
}

/// DWG のバイト列を `threads` 本のスレッドで読んで列にする
pub fn scan_dwg(bytes: &[u8], threads: usize) -> Result<ScannedDwg, OpenError> {
    let (file, single, charset) = prepare_dwg(bytes)?;

    let n = file.record_count();
    let workers = threads.max(1).min(n.max(1));
    let parts = if cfg!(target_arch = "wasm32") || workers <= 1 || n < 256 {
        vec![scan_dwg_range(&file, &single, charset, 0..n)]
    } else {
        let mut ranges = Vec::with_capacity(workers);
        for k in 0..workers {
            let start = n * k / workers;
            let end = n * (k + 1) / workers;
            if start < end {
                ranges.push(start..end);
            }
        }
        thread::scope(|scope| {
            let handles: Vec<_> = ranges
                .into_iter()
                .map(|r| scope.spawn(|| scan_dwg_range(&file, &single, charset, r)))
                .collect();
            handles
                .into_iter()
                .map(|h| h.join().map_err(|_| OpenError::InvalidDwg))
                .collect::<Result<Vec<_>, _>>()
        })?
    };

    Ok(ScannedDwg {
        raw_len: bytes.len(),
        file,
        single,
        charset,
        parts,
    })
}

/// スキャン済みの DWG から `Document` を組み上げる
pub fn assemble_dwg(scanned: ScannedDwg) -> Result<Document, OpenError> {
    let ScannedDwg {
        raw_len,
        file,
        single,
        charset,
        mut parts,
    } = scanned;
    let total_records = file.record_count() as u32;
    let version = single.version().to_vec();
    let insunits = single.insunits();
    let plimmin = single.plimmin();

    let mut has_duplicate_object_handle = single.has_duplicate_handle();
    let mut non_ent_handles: Vec<(u32, u64)> = Vec::new();
    let mut non_ent_seen: BTreeSet<u64> = BTreeSet::new();
    let mut objects_by_handle: BTreeMap<u64, u32> = BTreeMap::new();
    let mut dup_object_handles: BTreeSet<u64> = BTreeSet::new();
    let mut forget_refs: Vec<(u64, u32)> = Vec::new();
    let mut plain_summaries: Vec<PlainSummary> = Vec::with_capacity(parts.len());
    let mut ent_family: Vec<(u32, Followed)> = Vec::new();
    let mut has_spatial_filter = false;
    let mut has_non_plain_item = false;
    let mut first_paper_owner: Option<u64> = None;

    let mut ent_seen_handles: BTreeSet<u64> = BTreeSet::new();
    let mut duplicate_handles: BTreeSet<u64> = BTreeSet::new();
    let mut min_ent_h: Option<u64> = None;
    let mut max_ent_h: Option<u64> = None;
    let mut est_lifted_rows = 0usize;

    let mut names_map: BTreeMap<&'static [u8], BTreeMap<String, String>> = BTreeMap::new();
    for tk in TABLE_KINDS {
        names_map.insert(tk, BTreeMap::new());
    }
    let mut cat_items_with_origin: Vec<(u8, Vec<u8>, Vec<Value>, Origin)> = Vec::new();

    for part in &mut parts {
        for &(tk, ref raw_name) in &part.table_raw_names {
            if let Some(decoded) = charset.decode(raw_name)
                && let Some(map) = names_map.get_mut(tk)
            {
                map.insert(decoded.to_lowercase(), decoded);
            }
        }
        for &(rec, h) in &part.non_ent_handles {
            non_ent_handles.push((rec, h));
            if h != 0 && !non_ent_seen.insert(h) {
                has_duplicate_object_handle = true;
            }
        }
        for &(h, rec) in &part.object_handles {
            if objects_by_handle.insert(h, rec).is_some() {
                dup_object_handles.insert(h);
                if h != 0 {
                    has_duplicate_object_handle = true;
                }
            }
        }
        forget_refs.extend_from_slice(&part.forget_refs);
        plain_summaries.push(part.plain);
        ent_family.extend_from_slice(&part.ent_family);
        has_spatial_filter |= part.has_spatial_filter;

        for (sec_code, k, v, orig, _) in core::mem::take(&mut part.cat_items) {
            cat_items_with_origin.push((sec_code, k, v, orig));
        }

        for item in &part.entities {
            let h_opt = match item {
                DwgEntityItem::Lifted { lifted, .. } => {
                    est_lifted_rows += 1;
                    if let Some(h) = lifted.handle {
                        min_ent_h = Some(min_ent_h.map_or(h, |m| m.min(h)));
                        max_ent_h = Some(max_ent_h.map_or(h, |m| m.max(h)));
                    }
                    lifted.handle
                }
                DwgEntityItem::Attrib { handle, lift, .. } => {
                    if matches!(lift, AttribLift::Multiline | AttribLift::Not(_)) {
                        has_non_plain_item = true;
                    }
                    *handle
                }
                DwgEntityItem::Paper { handle, owner, .. } => {
                    if first_paper_owner.is_none() {
                        first_paper_owner = *owner;
                    }
                    *handle
                }
                DwgEntityItem::Dropped { handle, .. } | DwgEntityItem::Other { handle, .. } => {
                    *handle
                }
            };
            if let Some(h) = h_opt {
                if non_ent_seen.contains(&h) && h != 0 {
                    has_duplicate_object_handle = true;
                }
                if !ent_seen_handles.insert(h) {
                    duplicate_handles.insert(h);
                    if h != 0 {
                        has_duplicate_object_handle = true;
                    }
                }
            }
        }
    }
    for dup_h in &dup_object_handles {
        objects_by_handle.remove(dup_h);
    }
    forget_refs.sort_unstable();
    forget_refs.dedup();

    let live_object_handles: BTreeSet<u64> = objects_by_handle.keys().copied().collect();
    let mut fixed_max_handle: Option<u64> = None;
    let mut fold_fixed = |h: u64| {
        fixed_max_handle = Some(fixed_max_handle.map_or(h, |m| m.max(h)));
    };
    let object_rec_set: BTreeSet<u32> = objects_by_handle.values().copied().collect();
    for &(rec, h) in &non_ent_handles {
        if !object_rec_set.contains(&rec) || !live_object_handles.contains(&h) {
            fold_fixed(h);
        }
    }

    let mut catalog = SingleCatalog::new(charset, &version, insunits, plimmin);
    catalog.register_and_read_tables_with_origin(cat_items_with_origin, first_paper_owner);

    let mut open_diagnostics = Vec::new();
    if catalog.unit() == Unit::Unknown {
        open_diagnostics.push(OpenDiagnostic {
            kind: OpenDiagnosticKind::UnitsMissing,
            record: None,
            handle: None,
        });
    }

    let fast_plain = duplicate_handles.is_empty()
        && !has_non_plain_item
        && combine_plain_summaries(plain_summaries);
    let (attached_map, owned_set, plain, all_plain_attribs) = if fast_plain {
        ent_family = Vec::new();
        (BTreeMap::new(), BTreeSet::new(), true, true)
    } else {
        let (m, o, p) = attach_family(&ent_family);
        (m, o, p, false)
    };

    let mut shapes = Shapes::default();
    let mut rows: Paged<Row> = Paged::new();
    let mut boxes: Paged<[f64; 4]> = Paged::new();
    let mut by_handle = HandleIndex::with_range(min_ent_h, max_ent_h, est_lifted_rows);
    let mut non_slice_usages: Vec<NonSliceUsage> = Vec::new();
    let mut counts = Counts {
        records: total_records,
        ..Counts::default()
    };

    let mut part_shifts = Vec::with_capacity(parts.len());
    for part in &mut parts {
        part_shifts.push(merge_pools(
            &mut shapes.pools,
            core::mem::take(&mut part.pools),
        ));
    }

    let mut general_attribs: BTreeMap<u32, (AttribLift, u32, u32, u32)> = BTreeMap::new();
    if !all_plain_attribs {
        for (part, &(_, text_base, text_extra_base, _, extra_style_base)) in
            parts.iter().zip(&part_shifts)
        {
            for item in &part.entities {
                if let DwgEntityItem::Attrib { rec, ref lift, .. } = *item {
                    general_attribs
                        .insert(rec, (*lift, text_base, text_extra_base, extra_style_base));
                }
            }
        }
    }

    let intern_extra_lt = |extra_style: u32, pools: &mut Pools, cat: &mut SingleCatalog| {
        if let Some(es_idx) = extra_style.checked_sub(1)
            && let Some(LineTypeRef::Named(span)) =
                pools.extra_styles.get(es_idx as usize).map(|e| e.line_type)
        {
            let name = pools.str_of(span);
            let lt_id = cat.intern_line_type(name).0;
            if let Some(es) = pools.extra_styles.get_mut(es_idx as usize) {
                es.line_type_id = lt_id;
            }
        }
    };
    let mut zero_layer_id: Option<u32> = None;
    let mut last_layer: Option<(TextSpan, u32)> = None;
    let mut resolve_layer = |span: TextSpan, pools: &Pools, cat: &mut SingleCatalog| -> u32 {
        if span == TextSpan::ZERO {
            if let Some(id) = zero_layer_id {
                return id;
            }
            let id = cat.intern_layer("0").0;
            zero_layer_id = Some(id);
            return id;
        }
        if let Some((prev_span, prev_id)) = last_layer
            && prev_span == span
        {
            return prev_id;
        }
        let id = cat.intern_layer(pools.str_of(span)).0;
        last_layer = Some((span, id));
        id
    };

    let decode_one = |rec: u32| -> Option<(Vec<u8>, Vec<Value>, Origin)> {
        let mut b = Budget::default();
        let mut d = Diagnostics::new();
        let c = CancelToken::new();
        let mut cx = Cx::new(&mut b, &mut d, &c);
        let obj = single.decode_record(&file, rec as usize, &mut cx)?;
        Some((obj.kind, obj.values, obj.origin))
    };

    let feed_non_slice = |rec: u32,
                          kind: Vec<u8>,
                          values: Vec<Value>,
                          origin: Origin,
                          cat: &mut SingleCatalog,
                          usages: &mut Vec<NonSliceUsage>| {
        let mut followers = Vec::new();
        if let Some(f_recs) = attached_map.get(&rec) {
            for &f_rec in f_recs {
                if let Some((fk, fv, fo)) = decode_one(f_rec) {
                    followers.push((fk, fv, fo));
                }
            }
        }
        let layers_before = cat.layers_len();
        let lts_before = cat.line_types_len();
        cat.read_entity_for_catalog_with_origin(&kind, values, origin, followers);
        let mut used_layers = Vec::new();
        let mut used_lts = Vec::new();
        for lid in layers_before..cat.layers_len() {
            used_layers.push(LayerId(lid as u32));
        }
        for ltid in lts_before..cat.line_types_len() {
            used_lts.push(LineTypeId(ltid as u32));
        }
        if !used_layers.is_empty() || !used_lts.is_empty() {
            usages.push(NonSliceUsage {
                record: rec,
                layers: used_layers,
                line_types: used_lts,
            });
        }
    };

    let mut open_fast_insert: Option<u32> = None;

    for (part, &(vertex_base, text_base, text_extra_base, insert_extra_base, extra_style_base)) in
        parts.into_iter().zip(&part_shifts)
    {
        for item in part.entities {
            match item {
                DwgEntityItem::Lifted {
                    rec,
                    lifted,
                    has_xdict,
                } => {
                    counts.entities += 1;
                    if !all_plain_attribs && owned_set.contains(&rec) {
                        if let Some(h) = lifted.handle {
                            fold_fixed(h);
                        }
                        continue;
                    }
                    let mut geom = lifted.geom;
                    shift_geom_pools(
                        &mut geom,
                        vertex_base,
                        text_base,
                        text_extra_base,
                        insert_extra_base,
                    );
                    let extra_style = if lifted.extra_style == 0 {
                        0
                    } else {
                        lifted.extra_style + extra_style_base
                    };
                    let layer_span = lifted.layer.shifted(text_base);

                    intern_extra_lt(extra_style, &mut shapes.pools, &mut catalog);
                    let layer_id = resolve_layer(layer_span, &shapes.pools, &mut catalog);

                    if let Geom::Insert(ref mut ins) = geom {
                        if has_xdict
                            && has_spatial_filter
                            && let Some((_, ins_vals, _)) = decode_one(rec)
                        {
                            let res = single::symbol_clip(&ins_vals, |h| {
                                let obj_rec = *objects_by_handle.get(&h)?;
                                let (k, v, _) = decode_one(obj_rec)?;
                                Some((Cow::Owned(k), Cow::Owned(v)))
                            });
                            match res {
                                Ok(Some(clip)) => {
                                    let _ = shapes.pools.set_insert_clip(ins, clip);
                                }
                                Ok(None) => {}
                                Err(_) => {
                                    open_diagnostics.push(OpenDiagnostic {
                                        kind: OpenDiagnosticKind::ClipDropped,
                                        record: Some(rec),
                                        handle: lifted.handle,
                                    });
                                }
                            }
                        }

                        if all_plain_attribs {
                            ins.attrib_first = shapes.pools.attribs.len() as u32;
                            ins.attrib_count = 0;
                            ins.attached = 0;
                        } else if let Some(att_recs) = attached_map.get(&rec) {
                            ins.attached = att_recs.len() as u32;
                            ins.attrib_first = shapes.pools.attribs.len() as u32;
                            let mut ok_count = 0u32;
                            for &att_rec in att_recs {
                                if let Some((att_lift, tb, teb, esb)) =
                                    general_attribs.get(&att_rec).copied()
                                {
                                    match att_lift {
                                        AttribLift::Row(mut arow, lspan) => {
                                            arow.record = att_rec;
                                            arow.text = arow.text.shifted_spans(tb, teb);
                                            arow.tag = arow.tag.shifted(tb);
                                            let lspan = lspan.shifted(tb);
                                            if arow.extra_style != 0 {
                                                arow.extra_style += esb;
                                            }
                                            intern_extra_lt(
                                                arow.extra_style,
                                                &mut shapes.pools,
                                                &mut catalog,
                                            );
                                            arow.layer =
                                                resolve_layer(lspan, &shapes.pools, &mut catalog);
                                            shapes.pools.attribs.push(arow);
                                            ok_count += 1;
                                        }
                                        AttribLift::Multiline => {
                                            if let Some((_, vals, orig)) = decode_one(att_rec) {
                                                match catalog
                                                    .read_multiline_attrib_with_origin(vals, orig)
                                                {
                                                    Ok(attr) => {
                                                        let lid = attr.layer;
                                                        let lt_id_opt = match attr.style.line_type {
                                                            Inherit::Own(id) => Some(id),
                                                            _ => None,
                                                        };
                                                        let ltname = lt_id_opt.and_then(|id| {
                                                            catalog
                                                                .line_type_name(id)
                                                                .map(str::to_owned)
                                                        });
                                                        if let Some(mut arow) =
                                                            lift::push_lifted_attribute(
                                                                attr,
                                                                att_rec,
                                                                &mut shapes.pools,
                                                            )
                                                        {
                                                            if let (Some(lt_id), Some(ltname)) =
                                                                (lt_id_opt, ltname)
                                                                && let Some(span) =
                                                                    shapes.pools.push_utf8(&ltname)
                                                                && let Some(es_idx) =
                                                                    arow.extra_style.checked_sub(1)
                                                                && let Some(es) = shapes
                                                                    .pools
                                                                    .extra_styles
                                                                    .get_mut(es_idx as usize)
                                                            {
                                                                es.line_type =
                                                                    LineTypeRef::Named(span);
                                                                es.line_type_id = lt_id.0;
                                                            }
                                                            arow.layer = lid.0;
                                                            shapes.pools.attribs.push(arow);
                                                            ok_count += 1;
                                                        }
                                                    }
                                                    Err(code) => {
                                                        let kind = match code {
                                                            "dxf.non-planar.dropped" => {
                                                                Some(OpenDiagnosticKind::NonPlanar)
                                                            }
                                                            "dxf.code-page.unsupported" => {
                                                                Some(OpenDiagnosticKind::CodePage)
                                                            }
                                                            "dxf.lift.invalid" => Some(
                                                                OpenDiagnosticKind::LiftInvalid,
                                                            ),
                                                            _ => None,
                                                        };
                                                        if let Some(kind) = kind {
                                                            open_diagnostics.push(OpenDiagnostic {
                                                                kind,
                                                                record: Some(att_rec),
                                                                handle: None,
                                                            });
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        AttribLift::Not(reason) => {
                                            open_diagnostics.push(OpenDiagnostic {
                                                kind: match reason {
                                                    DropReason::Invalid => {
                                                        OpenDiagnosticKind::LiftInvalid
                                                    }
                                                    DropReason::NonPlanar => {
                                                        OpenDiagnosticKind::NonPlanar
                                                    }
                                                    DropReason::CodePage => {
                                                        OpenDiagnosticKind::CodePage
                                                    }
                                                },
                                                record: Some(rec),
                                                handle: lifted.handle,
                                            });
                                        }
                                    }
                                }
                            }
                            ins.attrib_count = ok_count;
                        }
                    }

                    let (kind, shape_idx) = shapes.push(geom);
                    let row_idx = rows.len() as u32;
                    rows.push(Row::new(
                        rec,
                        lifted.handle,
                        kind,
                        shape_idx,
                        layer_id,
                        lifted.color,
                        extra_style,
                        true,
                    ));
                    if all_plain_attribs {
                        open_fast_insert = match geom {
                            Geom::Insert(ins) if ins.flags & INSERT_FOLLOWED != 0 => Some(row_idx),
                            _ => None,
                        };
                    }
                    let bx = bounds(&shapes.pools, &geom);
                    boxes.push(bx);
                    match lifted.handle {
                        Some(h) if !duplicate_handles.contains(&h) => {
                            match by_handle.push_base_raw(h, row_idx) {
                                Some(0) | None => {}
                                Some(n) => {
                                    duplicate_handles.insert(h);
                                    fold_fixed(h);
                                    counts.unaddressable += n;
                                }
                            }
                        }
                        Some(h) => {
                            fold_fixed(h);
                            counts.unaddressable += 1;
                        }
                        None => {
                            counts.unaddressable += 1;
                        }
                    }
                    counts.elements += 1;
                }
                DwgEntityItem::Attrib { rec, handle, lift } => {
                    if let Some(h) = handle {
                        fold_fixed(h);
                    }
                    counts.entities += 1;
                    if all_plain_attribs {
                        if let (Some(parent_row_idx), AttribLift::Row(mut arow, lspan)) =
                            (open_fast_insert, lift)
                        {
                            arow.record = rec;
                            arow.text = arow.text.shifted_spans(text_base, text_extra_base);
                            arow.tag = arow.tag.shifted(text_base);
                            let lspan = lspan.shifted(text_base);
                            if arow.extra_style != 0 {
                                arow.extra_style += extra_style_base;
                            }
                            intern_extra_lt(arow.extra_style, &mut shapes.pools, &mut catalog);
                            arow.layer = resolve_layer(lspan, &shapes.pools, &mut catalog);
                            shapes.pools.attribs.push(arow);
                            if let Some(pr) = rows.get(parent_row_idx as usize).copied()
                                && let Some(ins) = shapes.inserts.get_mut(pr.shape as usize)
                            {
                                ins.attrib_count += 1;
                                ins.attached += 1;
                                let ins_copy = *ins;
                                let new_bx = bounds(&shapes.pools, &Geom::Insert(ins_copy));
                                if let Some(b_slot) = boxes.get_mut(parent_row_idx as usize) {
                                    *b_slot = new_bx;
                                }
                            }
                        }
                    } else if !owned_set.contains(&rec) {
                        counts.unlifted_entities += 1;
                    }
                }
                DwgEntityItem::Paper {
                    rec,
                    handle,
                    kind,
                    values,
                    origin,
                    ..
                } => {
                    if let Some(h) = handle {
                        fold_fixed(h);
                    }
                    counts.entities += 1;
                    open_fast_insert = None;
                    counts.paper += 1;
                    if catalog.has_paper_mm() {
                        feed_non_slice(
                            rec,
                            kind,
                            values,
                            origin,
                            &mut catalog,
                            &mut non_slice_usages,
                        );
                    }
                }
                DwgEntityItem::Dropped {
                    rec,
                    handle,
                    reason,
                } => {
                    if let Some(h) = handle {
                        fold_fixed(h);
                    }
                    counts.entities += 1;
                    open_fast_insert = None;
                    counts.dropped += 1;
                    open_diagnostics.push(OpenDiagnostic {
                        kind: match reason {
                            DropReason::Invalid => OpenDiagnosticKind::LiftInvalid,
                            DropReason::NonPlanar => OpenDiagnosticKind::NonPlanar,
                            DropReason::CodePage => OpenDiagnosticKind::CodePage,
                        },
                        record: Some(rec),
                        handle,
                    });
                }
                DwgEntityItem::Other {
                    rec,
                    handle,
                    other_kind,
                    payload,
                } => {
                    if let Some(h) = handle {
                        fold_fixed(h);
                    }
                    open_fast_insert = None;
                    counts.entities += 1;
                    if !all_plain_attribs && owned_set.contains(&rec) {
                        continue;
                    }
                    match other_kind {
                        DwgOtherKind::Seqend => {}
                        DwgOtherKind::Other => {
                            counts.unlifted_entities += 1;
                        }
                        DwgOtherKind::Lifted => {
                            counts.unlifted_entities += 1;
                            if let Some((kind, values, origin)) = payload {
                                feed_non_slice(
                                    rec,
                                    kind,
                                    values,
                                    origin,
                                    &mut catalog,
                                    &mut non_slice_usages,
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    counts.unaddressable += by_handle.finish_base(&mut duplicate_handles, &mut fold_fixed);
    let base_row_count = rows.len() as u32;

    let CatalogShell {
        unit,
        layers,
        by_layer_name,
        line_types,
        by_line_type_name,
        dimension_styles,
        text_styles,
        symbols,
        model_sort,
    } = catalog.finish();

    let mut def_by_name: BTreeMap<String, u32> = BTreeMap::new();
    let mut blocks_for_born: BTreeMap<String, String> = BTreeMap::new();
    for (idx, sym) in symbols.iter().enumerate() {
        if sym.key.namespace == BLOCK {
            def_by_name
                .entry(sym.key.name.clone())
                .or_insert(idx as u32);
            blocks_for_born.insert(sym.key.name.to_lowercase(), sym.key.name.clone());
        }
    }

    let mut layer_uses = vec![0u32; layers.len()];
    let mut line_type_uses = vec![0u32; line_types.len()];
    let mut bump_layer = |id: u32| {
        let idx = id as usize;
        if idx >= layer_uses.len() {
            layer_uses.resize(idx + 1, 0);
        }
        if let Some(s) = layer_uses.get_mut(idx) {
            *s += 1;
        }
    };
    let mut bump_lt = |extra_style: u32, pools: &Pools| {
        if let Some(es_idx) = extra_style.checked_sub(1)
            && let Some(es) = pools.extra_styles.get(es_idx as usize)
            && matches!(es.line_type, LineTypeRef::Named(_))
        {
            let idx = es.line_type_id as usize;
            if idx >= line_type_uses.len() {
                line_type_uses.resize(idx + 1, 0);
            }
            if let Some(s) = line_type_uses.get_mut(idx) {
                *s += 1;
            }
        }
    };

    let mut last_def: Option<(TextSpan, Option<u32>)> = None;
    for row_idx in 0..rows.len() {
        let Some(r) = rows.get(row_idx).copied() else {
            continue;
        };
        bump_layer(r.layer);
        bump_lt(r.extra_style, &shapes.pools);
        if r.kind == Kind::Insert
            && let Some(ins) = shapes.inserts.get_mut(r.shape as usize)
        {
            let found = if let Some((prev_span, prev_def)) = last_def
                && prev_span == ins.name
            {
                prev_def
            } else {
                let name = shapes.pools.str_of(ins.name);
                let def = def_by_name.get(name).copied();
                last_def = Some((ins.name, def));
                def
            };
            match found {
                Some(def_id) => ins.definition = def_id,
                None => {
                    ins.definition = NO_DEFINITION;
                    open_diagnostics.push(OpenDiagnostic {
                        kind: OpenDiagnosticKind::BlockMissing,
                        record: Some(r.record),
                        handle: r.handle(),
                    });
                }
            }
            for idx in ins.attrib_first..ins.attrib_first.saturating_add(ins.attrib_count) {
                if let Some(&attr) = shapes.pools.attribs.get(idx as usize) {
                    bump_layer(attr.layer);
                    bump_lt(attr.extra_style, &shapes.pools);
                }
            }
        }
    }

    for u in &non_slice_usages {
        for &lid in &u.layers {
            if let Some(s) = layer_uses.get_mut(lid.0 as usize) {
                *s += 1;
            }
        }
        for &ltid in &u.line_types {
            if let Some(s) = line_type_uses.get_mut(ltid.0 as usize) {
                *s += 1;
            }
        }
    }
    for sym in &symbols {
        for el in &sym.space.elements {
            if let Some(s) = layer_uses.get_mut(el.layer.0 as usize) {
                *s += 1;
            }
            if let Inherit::Own(ltid) = el.style.line_type
                && let Some(s) = line_type_uses.get_mut(ltid.0 as usize)
            {
                *s += 1;
            }
        }
        for ad in &sym.attributes {
            if let Some(s) = layer_uses.get_mut(ad.attribute.layer.0 as usize) {
                *s += 1;
            }
            if let Inherit::Own(ltid) = ad.attribute.style.line_type
                && let Some(s) = line_type_uses.get_mut(ltid.0 as usize)
            {
                *s += 1;
            }
        }
    }

    let model_owner = single.model_owner();
    let handseed = single.handle_seed();
    let tables_cache = SingleTables {
        model_owner,
        markers: false,
        version: version.clone(),
        charset,
        dwg_seed: Some(handseed),
        names: Arc::new(names_map),
        blocks: Arc::new(blocks_for_born),
    };

    let header = HeaderState {
        insunits,
        plimmin,
        ..HeaderState::default()
    };

    let backing = Box::new(DwgBacking {
        raw_len,
        file,
        single,
        handseed,
        patched: BTreeMap::new(),
        born: BTreeMap::new(),
    });

    Ok(Document {
        src: SourceBytes::compress_slice(&[], 1),
        dwg: Some(backing),
        record_starts: Vec::new(),
        counts,
        unit,
        version,
        charset,
        layers,
        by_layer_name,
        line_types,
        by_line_type_name,
        dimension_styles,
        text_styles,
        symbols,
        def_by_name,
        model_sort,
        rows,
        by_handle,
        duplicate_handles,
        base_row_count,
        added_by_record: BTreeMap::new(),
        shapes,
        boxes,
        grid: Grid::default(),
        display: Display::default(),
        open_diagnostics,
        overlay: BTreeMap::new(),
        gone: BTreeSet::new(),
        history: History::new(),
        next_added_record: total_records,
        entities: Vec::new(),
        entities_end: None,
        sections: Vec::new(),
        header,
        tables_cache,
        has_duplicate_object_handle,
        objects_by_handle,
        fixed_max_handle,
        live_object_handles,
        added_follower_handles: BTreeSet::new(),
        forget_refs,
        plain,
        ent_family,
        layer_uses,
        line_type_uses,
        non_slice_usages,
    })
}

impl Document {
    /// DWG レコードの今の種類・セクション・値列を返す
    pub(crate) fn dwg_record_kind_section_values(
        &self,
        record: u32,
    ) -> Option<(Vec<u8>, Section, Vec<Value>)> {
        let dwg = self.dwg.as_ref()?;
        if let Some(patch) = dwg.patched.get(&record) {
            return Some((
                patch.kind.clone(),
                patch.section.clone(),
                patch.values.clone(),
            ));
        }
        if let Some(born) = dwg.born.get(&record) {
            return Some((born.kind.clone(), Section::Entities, born.values.clone()));
        }
        if record >= self.counts.records {
            return None;
        }
        let mut budget = Budget::default();
        let mut diag = Diagnostics::new();
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        let obj = dwg
            .single
            .decode_record(&dwg.file, record as usize, &mut cx)?;
        Some((obj.kind, obj.section, obj.values))
    }

    /// DWG レコードの今の符号化済みバイト列を返す
    pub(crate) fn dwg_record_encoded(&self, record: u32) -> Option<Vec<u8>> {
        let dwg = self.dwg.as_ref()?;
        if let Some(patch) = dwg.patched.get(&record) {
            return Some(patch.encoded.clone());
        }
        if let Some(born) = dwg.born.get(&record) {
            return Some(born.encoded.clone());
        }
        if record >= self.counts.records {
            return None;
        }
        dwg.single.base_encoded(&dwg.file, record as usize)
    }

    /// DWG レコードのハンドルを返す
    pub(crate) fn dwg_record_handle(&self, record: u32) -> Option<u64> {
        let dwg = self.dwg.as_ref()?;
        if let Some(born) = dwg.born.get(&record) {
            return Some(born.handle);
        }
        dwg.file.entry_summary(record as usize).map(|(h, ..)| h)
    }

    /// DWG の元のレコードの出所 `Ref` を返す
    pub(crate) fn dwg_base_source(&self, record: u32, handle: Option<u64>) -> Option<Ref> {
        let dwg = self.dwg.as_ref()?;
        let (_, _, _, span) = dwg.file.entry_summary(record as usize)?;
        let at = Ref::at_bytes(span.start as u64, span.end as u64);
        Some(match handle {
            Some(h) => at.with_key(Key::Handle(h)),
            None => at,
        })
    }

    /// DWG の 1 つの ATTRIB レコードを持ち上げる
    pub(crate) fn dwg_lift_single_attrib(&mut self, rec: u32) -> Option<AttribRow> {
        let (_, _, values) = self.dwg_record_kind_section_values(rec)?;
        let handle = self.dwg_record_handle(rec);
        let lifted =
            lift::attrib_values(self.charset, handle, &values, rec, &mut self.shapes.pools);
        match lifted {
            AttribLift::Row(mut row, layer_span) => {
                self.bind_attrib_ids(&mut row, layer_span);
                return Some(row);
            }
            AttribLift::Multiline => {}
            AttribLift::Not(_) => return None,
        }
        let orig = match self
            .dwg
            .as_ref()
            .and_then(|d| d.file.entry_summary(rec as usize))
        {
            Some((_, _, _, span)) if rec < self.counts.records => Origin::Dwg {
                offset: span.start,
                end: span.end,
            },
            _ => Origin::New,
        };
        let mut cat = SingleCatalog::new(self.charset, &self.version, self.header.insunits, None);
        let attr = cat.read_multiline_attrib_with_origin(values, orig).ok()?;
        let layer_name = cat.layer_name(attr.layer).to_owned();
        let lt_name = match attr.style.line_type {
            Inherit::Own(id) => cat.line_type_name(id).map(str::to_owned),
            _ => None,
        };
        let mut row = lift::push_lifted_attribute(attr, rec, &mut self.shapes.pools)?;
        if let Some(lt_name) = lt_name {
            let lt_id = self.ensure_line_type_id(&lt_name);
            let span = self.shapes.pools.push_utf8(&lt_name)?;
            if let Some(es_idx) = row.extra_style.checked_sub(1)
                && let Some(es) = self.shapes.pools.extra_styles.get_mut(es_idx as usize)
            {
                es.line_type = LineTypeRef::Named(span);
                es.line_type_id = lt_id.0;
            }
        }
        row.layer = self.ensure_layer_id(&layer_name).0;
        Some(row)
    }

    /// DWG レコードの今の内容から既存の行を持ち上げ直す
    pub(crate) fn dwg_refresh_row(
        &mut self,
        row_index: u32,
        attrib_records: Option<&[u32]>,
    ) -> bool {
        let Some(old_row) = self.rows.get(row_index as usize).copied() else {
            return false;
        };
        if old_row.alive {
            self.adjust_row_uses(old_row, -1);
        }
        let Some((_, _, values)) = self.dwg_record_kind_section_values(old_row.record) else {
            if old_row.alive {
                self.adjust_row_uses(old_row, 1);
            }
            return false;
        };
        let handle = self.dwg_record_handle(old_row.record);
        let Lift::Element(mut lifted) = lift::lift_values(
            self.charset,
            old_row.kind,
            handle,
            &values,
            &mut self.shapes.pools,
        ) else {
            if old_row.alive {
                self.adjust_row_uses(old_row, 1);
            }
            return false;
        };
        let layer_id = self.bind_lifted_ids(&mut lifted);

        if let Geom::Insert(ref mut ins) = lifted.geom {
            if let Some(prev_ins) = self.shapes.inserts.get(old_row.shape as usize).copied()
                && let Some(clip) = prev_ins.clip(&self.shapes.pools).cloned()
            {
                let _ = self.shapes.pools.set_insert_clip(ins, clip);
            }
            let records_vec: Vec<u32> = match attrib_records {
                Some(list) => list.to_vec(),
                None => {
                    if let Some(prev_ins) = self.shapes.inserts.get(old_row.shape as usize) {
                        self.shapes
                            .pools
                            .attribs_of(prev_ins)
                            .map(|a| a.record)
                            .collect()
                    } else {
                        Vec::new()
                    }
                }
            };
            ins.attached = records_vec.len() as u32;
            ins.attrib_first = self.shapes.pools.attribs.len() as u32;
            let mut lifted_count = 0u32;
            for rec in records_vec {
                if let Some(attr_row) = self.dwg_lift_single_attrib(rec) {
                    self.shapes.pools.attribs.push(attr_row);
                    lifted_count += 1;
                }
            }
            ins.attrib_count = lifted_count;
        }

        let new_box = bounds(&self.shapes.pools, &lifted.geom);
        let geom_copy = lifted.geom;
        self.shapes.set(old_row.kind, old_row.shape, lifted.geom);
        let new_row = Row::new(
            old_row.record,
            lifted.handle,
            old_row.kind,
            old_row.shape,
            layer_id,
            lifted.color,
            lifted.extra_style,
            old_row.alive,
        );
        if let Some(slot) = self.rows.get_mut(row_index as usize) {
            *slot = new_row;
        }
        if let Some(slot) = self.boxes.get_mut(row_index as usize) {
            *slot = new_box;
        }
        if new_row.alive {
            self.adjust_row_uses(new_row, 1);
            self.grid.update(row_index, new_box);
            self.display.update(
                row_index,
                &geom_copy,
                &self.shapes.pools,
                [new_row.layer, new_row.color],
            );
        }
        true
    }

    /// `!self.plain` の DWG 図面で追加・削除があったとき、生きている親子関係を再計算して `INSERT` 行を更新する
    pub(crate) fn dwg_reattach_live(&mut self) {
        let mut live_family: Vec<(u32, Followed)> = Vec::with_capacity(self.ent_family.len() + 8);
        for &(rec, role) in &self.ent_family {
            if !self.gone.contains(&rec) {
                live_family.push((rec, role));
            }
        }
        if let Some(dwg) = &self.dwg {
            for (&rec, born) in &dwg.born {
                if self.gone.contains(&rec) {
                    continue;
                }
                let obj = zumen_acad::Object {
                    kind: born.kind.clone(),
                    section: Section::Entities,
                    values: born.values.clone(),
                    origin: Origin::New,
                };
                if let Some(role) = single::followed_object(&obj) {
                    live_family.push((rec, role));
                }
            }
        }
        let (attached_map, _, _) = attach_family(&live_family);
        let insert_rows: Vec<(u32, u32, u32)> = (0..self.rows.len() as u32)
            .filter_map(|idx| {
                let r = self.rows.get(idx as usize)?;
                (r.alive && r.kind == Kind::Insert).then_some((idx, r.record, r.shape))
            })
            .collect();
        for (row_idx, rec, shape_idx) in insert_rows {
            let want: &[u32] = attached_map.get(&rec).map_or(&[], Vec::as_slice);
            let same = self
                .shapes
                .inserts
                .get(shape_idx as usize)
                .is_some_and(|ins| {
                    ins.attached as usize == want.len()
                        && ins.attrib_count == ins.attached
                        && self
                            .shapes
                            .pools
                            .attribs_of(ins)
                            .map(|a| a.record)
                            .eq(want.iter().copied())
                });
            if !same {
                let _ = self.dwg_refresh_row(row_idx, Some(want));
            }
        }
    }

    /// DWG で `skip` 以外の生きているレコードが `handles` のどれかを指すか調べる
    pub(crate) fn dwg_is_any_handle_pointed(
        &self,
        skip: &BTreeSet<u32>,
        handles: &BTreeSet<u64>,
    ) -> bool {
        let Some(dwg) = &self.dwg else {
            return false;
        };
        let mut budget = Budget::default();
        let mut diag = Diagnostics::new();
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        for rec in 0..self.counts.records {
            if self.gone.contains(&rec) || skip.contains(&rec) {
                continue;
            }
            if let Some(patch) = dwg.patched.get(&rec) {
                if patch
                    .values
                    .iter()
                    .any(|v| matches!(v.data, Data::Handle(h) if handles.contains(&h)))
                {
                    return true;
                }
            } else if let Some(obj) = dwg.single.decode_record(&dwg.file, rec as usize, &mut cx)
                && obj
                    .values
                    .iter()
                    .any(|v| matches!(v.data, Data::Handle(h) if handles.contains(&h)))
            {
                return true;
            }
        }
        for (&rec, born) in &dwg.born {
            if self.gone.contains(&rec) || skip.contains(&rec) {
                continue;
            }
            if born
                .values
                .iter()
                .any(|v| matches!(v.data, Data::Handle(h) if handles.contains(&h)))
            {
                return true;
            }
        }
        false
    }

    /// DWG で削除対象以外にハンドル `0` のレコードが残っているか
    pub(crate) fn dwg_has_surviving_zero_handle(&self, removing_records: &BTreeSet<u32>) -> bool {
        let Some(dwg) = &self.dwg else {
            return false;
        };
        for idx in 0..dwg.file.entry_count() {
            let rec = idx as u32;
            if !self.gone.contains(&rec)
                && !removing_records.contains(&rec)
                && dwg.file.entry_summary(idx).is_some_and(|(h, ..)| h == 0)
            {
                return true;
            }
        }
        false
    }

    /// DWG の差分変更を反映してバイト列に書き出す
    pub(crate) fn save_dwg(&self) -> Vec<u8> {
        let Some(dwg) = &self.dwg else {
            return Vec::new();
        };
        let mut staged: BTreeMap<usize, Vec<u8>> = BTreeMap::new();
        for (&rec, patch) in &dwg.patched {
            if rec < self.counts.records && !self.gone.contains(&rec) {
                staged.insert(rec as usize, patch.encoded.clone());
            }
        }
        let mut removed_records: BTreeSet<usize> = BTreeSet::new();
        let mut removed_model_handles: BTreeSet<u64> = BTreeSet::new();
        for &rec in self.gone.range(..self.counts.records) {
            removed_records.insert(rec as usize);
            if let Some((h, ..)) = dwg.file.entry_summary(rec as usize)
                && dwg
                    .single
                    .entry_meta(&dwg.file, rec as usize)
                    .is_some_and(|m| matches!(m.section, Section::Entities | Section::Blocks))
            {
                removed_model_handles.insert(h);
            }
        }
        let mut added: Vec<(u64, Vec<u8>, bool)> = Vec::new();
        for (&rec, born) in &dwg.born {
            if !self.gone.contains(&rec) {
                let bytes = dwg
                    .patched
                    .get(&rec)
                    .map_or_else(|| born.encoded.clone(), |p| p.encoded.clone());
                added.push((born.handle, bytes, born.is_top_model));
            }
        }
        let new_seed = (dwg.handseed != dwg.single.handle_seed())
            .then_some(dwg.handseed)
            .flatten();
        let mut budget = Budget::default();
        let mut diag = Diagnostics::new();
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        dwg.single
            .save_file(
                &dwg.file,
                &staged,
                &removed_records,
                &removed_model_handles,
                &added,
                new_seed,
                &mut cx,
            )
            .unwrap_or_default()
    }
}

struct DwgTileTemplate {
    el: zumen_drawing::Element<zumen_drawing::ModelPoint>,
    layer_name: String,
    lt_name: Option<String>,
    attrib_layers: Vec<(LayerId, String)>,
    attrib_lts: Vec<(LineTypeId, String)>,
    record_count: u64,
}

/// モデル空間の要素を `tiles` 枚（元を含む）に増やした DWG を返す
pub fn grow_dwg(src: &[u8], tiles: usize, threads: usize) -> Result<Vec<u8>, GrowError> {
    let document = Document::open_slice(src, threads).map_err(GrowError::Open)?;
    let dwg = document.dwg.as_ref().ok_or(GrowError::NoEntities)?;
    if dwg.single.model_owner().is_none() || dwg.single.handle_seed().is_none() {
        return Err(GrowError::NoEntities);
    }
    if document.rows.is_empty() {
        return Err(GrowError::NoElements);
    }
    if tiles <= 1 {
        return Ok(src.to_vec());
    }

    let mut extent = [
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    ];
    for item in document.boxes.iter() {
        if item.iter().all(|value| value.is_finite()) {
            extent = [
                extent[0].min(item[0]),
                extent[1].min(item[1]),
                extent[2].max(item[2]),
                extent[3].max(item[3]),
            ];
        }
    }
    if extent.iter().any(|value| !value.is_finite()) {
        extent = [0.0, 0.0, 1.0, 1.0];
    }
    let pitch = [
        ((extent[2] - extent[0]) * 1.0625).max(1.0),
        ((extent[3] - extent[1]) * 1.0625).max(1.0),
    ];
    let mut columns = 1usize;
    while columns * columns < tiles {
        columns += 1;
    }

    let mut base_seed = single::next_handle(document.live_max_handle(), dwg.handseed)
        .ok_or(GrowError::NoEntities)?;
    let mut tables = document.tables_cache.clone();
    tables.dwg_seed = Some(Some(base_seed));

    let mut templates: Vec<DwgTileTemplate> = Vec::with_capacity(document.rows.len());
    for row_idx in 0..document.rows.len() as u32 {
        let Some(mut el) = document.element(row_idx) else {
            continue;
        };
        if let zumen_drawing::Shape::Symbol(ref mut sym) = el.shape {
            sym.clip = None;
            if sym.attributes.iter().any(|a| a.formatted.is_some()) {
                sym.attributes.clear();
            }
        }
        // DWG の `Add` は RGB 直指定や太さ直指定を受け付けない（決定 0079）ため、増やす写しでは画層継承に揃える
        if let Inherit::Own(ref mut c) = el.style.color {
            c.rgb = None;
            if c.code.is_none() {
                el.style.color = Inherit::Layer;
            }
        }
        if matches!(el.style.line_weight, Some(Inherit::Own(_))) {
            el.style.line_weight = Some(Inherit::Layer);
        }
        let Some(layer_name) = document
            .layers
            .get(el.layer.0 as usize)
            .map(|l| l.name.clone())
        else {
            continue;
        };
        let lt_name = match el.style.line_type {
            Inherit::Own(ltid) => document
                .line_types
                .get(ltid.0 as usize)
                .map(|lt| lt.name.clone()),
            _ => None,
        };
        let mut attrib_layers = Vec::new();
        let mut attrib_lts = Vec::new();
        if matches!(el.shape, zumen_drawing::Shape::Symbol(_)) {
            for attr in el.shape_symbol_attribs_mut_or_empty() {
                if let Inherit::Own(ref mut c) = attr.style.color {
                    c.rgb = None;
                    if c.code.is_none() {
                        attr.style.color = Inherit::Layer;
                    }
                }
                if matches!(attr.style.line_weight, Some(Inherit::Own(_))) {
                    attr.style.line_weight = Some(Inherit::Layer);
                }
            }
            if let zumen_drawing::Shape::Symbol(ref sym2) = el.shape {
                for attr in &sym2.attributes {
                    if let Some(al) = document.layers.get(attr.layer.0 as usize) {
                        attrib_layers.push((attr.layer, al.name.clone()));
                    }
                    if let Inherit::Own(ltid) = attr.style.line_type
                        && let Some(alt) = document.line_types.get(ltid.0 as usize)
                    {
                        attrib_lts.push((ltid, alt.name.clone()));
                    }
                }
            }
        }

        let probe: BornRecords = match single::born_model(
            &tables,
            &el,
            &layer_name,
            lt_name.as_deref(),
            &attrib_layers,
            &attrib_lts,
            base_seed,
        ) {
            Some(r) => r,
            None => continue,
        };
        if dwg
            .single
            .encode_born_model(&probe, base_seed, |_| false)
            .is_none()
        {
            continue;
        }
        let record_count = probe.len() as u64;
        templates.push(DwgTileTemplate {
            el,
            layer_name,
            lt_name,
            attrib_layers,
            attrib_lts,
            record_count,
        });
    }

    if templates.is_empty() {
        return Err(GrowError::NoElements);
    }

    let handles_per_tile: u64 = templates.iter().map(|t| t.record_count).sum();
    let copy_tiles = tiles - 1;
    let workers = threads.max(1).min(copy_tiles.max(1));

    let encode_tile_range =
        |tile_range: core::ops::Range<usize>| -> Option<Vec<(u64, Vec<u8>, bool)>> {
            let mut out = Vec::with_capacity(tile_range.len() * templates.len());
            for tile in tile_range {
                let shift = [
                    (tile % columns) as f64 * pitch[0],
                    (tile / columns) as f64 * pitch[1],
                ];
                let affine = Affine {
                    m: [1.0, 0.0, shift[0], 0.0, 1.0, shift[1]],
                };
                let mut cur_handle = base_seed + (tile - 1) as u64 * handles_per_tile;
                for tmpl in &templates {
                    let mut moved_el = tmpl.el.clone();
                    moved_el.shape = transform_shape(&tmpl.el.shape, &affine)?;
                    let born = single::born_model(
                        &tables,
                        &moved_el,
                        &tmpl.layer_name,
                        tmpl.lt_name.as_deref(),
                        &tmpl.attrib_layers,
                        &tmpl.attrib_lts,
                        cur_handle,
                    )?;
                    let (encoded, next_h) =
                        dwg.single.encode_born_model(&born, cur_handle, |_| false)?;
                    for ((kind, _), (h, bytes)) in born.iter().zip(encoded) {
                        let is_top = !matches!(*kind, b"ATTRIB" | b"SEQEND");
                        out.push((h, bytes, is_top));
                    }
                    cur_handle = next_h;
                }
            }
            Some(out)
        };

    let added_chunks: Vec<Vec<(u64, Vec<u8>, bool)>> = if workers <= 1 || copy_tiles < 16 {
        vec![encode_tile_range(1..tiles).ok_or(GrowError::NoElements)?]
    } else {
        let mut ranges = Vec::with_capacity(workers);
        for k in 0..workers {
            let s = 1 + copy_tiles * k / workers;
            let e = 1 + copy_tiles * (k + 1) / workers;
            if s < e {
                ranges.push(s..e);
            }
        }
        thread::scope(|scope| {
            let handles: Vec<_> = ranges
                .into_iter()
                .map(|r| scope.spawn(move || encode_tile_range(r)))
                .collect();
            handles
                .into_iter()
                .map(|h| h.join().ok().flatten().ok_or(GrowError::NoElements))
                .collect::<Result<Vec<_>, _>>()
        })?
    };

    let total_added: usize = added_chunks.iter().map(Vec::len).sum();
    let mut added = Vec::with_capacity(total_added);
    for mut chunk in added_chunks {
        added.append(&mut chunk);
    }
    base_seed += copy_tiles as u64 * handles_per_tile;

    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
    dwg.single
        .save_file(
            &dwg.file,
            &BTreeMap::new(),
            &BTreeSet::new(),
            &BTreeSet::new(),
            &added,
            Some(base_seed),
            &mut cx,
        )
        .map_err(|_| GrowError::NoEntities)
}

trait ElementSymbolMutExt {
    fn shape_symbol_attribs_mut_or_empty(
        &mut self,
    ) -> &mut [zumen_drawing::Attribute<zumen_drawing::ModelPoint>];
}

impl ElementSymbolMutExt for zumen_drawing::Element<zumen_drawing::ModelPoint> {
    fn shape_symbol_attribs_mut_or_empty(
        &mut self,
    ) -> &mut [zumen_drawing::Attribute<zumen_drawing::ModelPoint>] {
        match &mut self.shape {
            zumen_drawing::Shape::Symbol(sym) => &mut sym.attributes,
            _ => &mut [],
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::born::{Add, ColorSpec, NewShape, NewSymbol, NewText, Restyle};
    use crate::dwg::grow_dwg;
    use crate::edit::Edit;
    use crate::inspect::live_elements;
    use crate::store::Document;

    #[test]
    fn open_dwg_parallel_matches_single_thread_and_roundtrips_edits() {
        let samples = [
            "../../samples/dwg/zumen-layout-r2000.dwg",
            "../../samples/dwg/zumen-minsert-r2000.dwg",
            "../../samples/dwg/zumen-layout-r2018.dwg",
            "../../samples/dwg/zumen-minsert-r2018.dwg",
            "../../samples/dwg/zumen-xclip-r2018.dwg",
        ];
        for rel in samples {
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(rel);
            let bytes = std::fs::read(&path).expect("sample should exist");
            let doc1 = Document::open_slice(&bytes, 1).expect("1T open");
            let doc4 = Document::open_slice(&bytes, 4).expect("4T open");
            assert_eq!(doc1.counts, doc4.counts, "counts mismatch on {rel}");
            assert_eq!(doc1.rows.len(), doc4.rows.len(), "rows mismatch on {rel}");
            assert_eq!(doc1.display, doc4.display, "display mismatch on {rel}");
            assert_eq!(
                live_elements(&doc1),
                live_elements(&doc4),
                "inspected elements mismatch on {rel}"
            );
        }
    }

    #[test]
    fn dwg_r2000_and_r2018_edit_undo_redo_and_grow() {
        for rel in [
            "../../samples/dwg/zumen-minsert-r2000.dwg",
            "../../samples/dwg/zumen-minsert-r2018.dwg",
        ] {
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(rel);
            let bytes = std::fs::read(&path).expect("sample should exist");
            let mut doc = Document::open_slice(&bytes, 2).expect("open");
            let before_bytes = doc.save();
            let before_elements = live_elements(&doc);

            let first_handle = doc
                .rows
                .iter()
                .find_map(|r| r.alive.then(|| r.handle()).flatten())
                .expect("at least one element");
            doc.apply(&[Edit::Transform {
                handle: first_handle,
                matrix: [1.0, 0.0, 12.5, 0.0, 1.0, -7.25],
            }])
            .expect("transform");
            doc.apply(&[Edit::Restyle {
                handle: first_handle,
                style: Restyle {
                    color: Some(ColorSpec::Aci(3)),
                    ..Restyle::default()
                },
            }])
            .expect("restyle");

            let sym_name = doc
                .symbols
                .iter()
                .find(|s| !s.key.name.is_empty() && !s.key.name.starts_with('*'))
                .map(|s| s.key.name.clone())
                .expect("named block definition");

            let added = doc
                .apply(&[
                    Edit::Add(Box::new(Add {
                        layer: None,
                        color: None,
                        line_type: None,
                        line_weight: None,
                        shape: NewShape::Line {
                            start: [0.0, 0.0],
                            end: [20.0, 10.0],
                        },
                    })),
                    Edit::Add(Box::new(Add {
                        layer: None,
                        color: None,
                        line_type: None,
                        line_weight: None,
                        shape: NewShape::Circle {
                            center: [5.0, 5.0],
                            radius: 3.0,
                        },
                    })),
                    Edit::Add(Box::new(Add {
                        layer: None,
                        color: None,
                        line_type: None,
                        line_weight: None,
                        shape: NewShape::Arc {
                            center: [10.0, 10.0],
                            radius: 4.0,
                            start: 0.0,
                            end: 90.0,
                        },
                    })),
                    Edit::Add(Box::new(Add {
                        layer: None,
                        color: None,
                        line_type: None,
                        line_weight: None,
                        shape: NewShape::Text(NewText {
                            content: "R-TEST".to_owned(),
                            position: [2.0, 3.0],
                            end: None,
                            horizontal: None,
                            vertical: None,
                            height: 2.5,
                            rotation: 0.0,
                            width_factor: None,
                            oblique: None,
                            backward: false,
                            upside_down: false,
                            style: None,
                        }),
                    })),
                    Edit::Add(Box::new(Add {
                        layer: None,
                        color: None,
                        line_type: None,
                        line_weight: None,
                        shape: NewShape::Symbol(NewSymbol {
                            name: sym_name,
                            position: [30.0, 40.0],
                            scale: [1.0, 1.0],
                            rotation: 0.0,
                            grid: None,
                        }),
                    })),
                ])
                .expect("add shapes");
            let added_line = added[0].expect("added line handle");
            doc.apply(&[Edit::Remove { handle: added_line }])
                .expect("remove added line");

            let edited_saved = doc.save();
            let reopened = Document::open_slice(&edited_saved, 1).expect("reopen edited dwg");
            let public_view = |d: &Document| -> Vec<_> {
                live_elements(d)
                    .into_iter()
                    .map(|e| (e.id, e.kind, e.layer, e.hash, e.bounds))
                    .collect()
            };
            assert_eq!(
                public_view(&doc),
                public_view(&reopened),
                "reopened elements mismatch on {rel}"
            );

            while doc.undo() {}
            assert_eq!(doc.save(), before_bytes, "undo bytes mismatch on {rel}");
            assert_eq!(
                live_elements(&doc),
                before_elements,
                "undo elements mismatch on {rel}"
            );
            while doc.redo() {}
            assert_eq!(doc.save(), edited_saved, "redo bytes mismatch on {rel}");

            let grown = grow_dwg(&bytes, 4, 2).expect("grow dwg");
            let grown_doc = Document::open_slice(&grown, 2).expect("open grown dwg");
            assert!(grown_doc.counts.elements > doc.counts.elements);
        }
    }
}
