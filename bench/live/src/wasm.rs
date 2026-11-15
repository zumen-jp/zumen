//! `wasm32-unknown-unknown` で文書エンジンを動かすための C ABI 入口。
//!
//! - 単一スレッド Worker（通常の `WebAssembly.Memory` ＋ `postMessage` での差分転送）
//! - 共有メモリ並列 Worker プール（`SharedArrayBuffer` ＋ `atomics` での並列スキャン・ゼロコピー差分参照）
//! の両方を同じモジュールから測れるようにする。

use std::alloc::{Layout, alloc_zeroed, dealloc};
use std::ops::Range;
use std::sync::Mutex;

use zumen_acad::Charset;
use zumen_acad::dwg::DwgFile;
use zumen_acad::dwg::single::DwgSingle;
use zumen_sdk::FileModel;

use crate::born::{Add, ColorSpec, NewShape};
use crate::display::{DirtyColumn, Display};
use crate::dwg::{DwgPart, ScannedDwg, is_dwg_bytes, prepare_dwg, scan_dwg_range};
use crate::edit::Edit;
use crate::paged::{CompressedPart, Paged, SourceBytes, compress_pages_slice};
use crate::store::{
    Document, OpenError, Part, Scanned, ScannedInner, pre_scan_header, scan_source_part,
    split_points_source,
};

struct WasmSession {
    doc: Document,
    handles: Vec<u64>,
    extent: [f64; 4],
    span: f64,
    first_layer: Option<String>,
    /// ゼロコピー参照用の `[col_id, dst_byte_offset, wasm_slice_ptr, byte_len]` 列
    dirty_table: Vec<u32>,
    /// `postMessage` 転送用の連結済み差分バイト列
    dirty_packed: Vec<u8>,
    dirty_total_bytes: usize,
    /// 比較用の表示バッファー全体バイト列
    full_packed: Vec<u8>,
    select_scratch: Vec<u32>,
}

enum MtStage {
    Dwg {
        raw_len: usize,
        file: DwgFile,
        single: DwgSingle,
        charset: Charset,
        ranges: Vec<Range<usize>>,
        parts: Mutex<Vec<Option<DwgPart>>>,
    },
    DxfCompress {
        ptr: usize,
        len: usize,
        ranges: Vec<Range<usize>>,
        parts: Mutex<Vec<Option<CompressedPart>>>,
    },
    DxfScan {
        source: SourceBytes,
        charset: Charset,
        ranges: Vec<Range<usize>>,
        parts: Mutex<Vec<Option<Result<Part, OpenError>>>>,
    },
}

static SESSION: Mutex<Option<WasmSession>> = Mutex::new(None);
static MT_STAGE: Mutex<Option<MtStage>> = Mutex::new(None);

fn compute_extent(doc: &Document) -> [f64; 4] {
    let mut extent = [
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    ];
    for item in doc.boxes.iter() {
        if item[0] <= item[2] && item[1] <= item[3] && item.iter().all(|v| v.is_finite()) {
            extent = [
                extent[0].min(item[0]),
                extent[1].min(item[1]),
                extent[2].max(item[2]),
                extent[3].max(item[3]),
            ];
        }
    }
    if extent.iter().any(|v| !v.is_finite()) {
        extent = [0.0, 0.0, 100.0, 100.0];
    }
    extent
}

fn finish_session(doc: Document) -> u32 {
    let count = doc.counts.elements;
    let handles: Vec<u64> = doc
        .rows
        .iter()
        .filter(|r| r.alive && !doc.duplicate_handles.contains(&r.handle().unwrap_or(0)))
        .filter_map(|r| r.handle())
        .collect();
    let extent = compute_extent(&doc);
    let span = (extent[2] - extent[0]).max(extent[3] - extent[1]).max(1.0);
    let first_layer = doc.layers.first().map(|l| l.name.clone());
    let sess = WasmSession {
        doc,
        handles,
        extent,
        span,
        first_layer,
        dirty_table: Vec::with_capacity(64),
        dirty_packed: Vec::with_capacity(16384),
        dirty_total_bytes: 0,
        full_packed: Vec::new(),
        select_scratch: Vec::with_capacity(4096),
    };
    if let Ok(mut guard) = SESSION.lock() {
        *guard = Some(sess);
    }
    count
}

fn hash_step(seed: u64) -> u64 {
    let mut x = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

fn collect_dirty_into(
    display: &mut Display,
    pack_bytes: bool,
    table: &mut Vec<u32>,
    packed: &mut Vec<u8>,
) -> usize {
    table.clear();
    packed.clear();
    let mut total_bytes = 0usize;

    fn visit_slice<T: Copy>(
        col_id: u32,
        col: &Paged<T>,
        dirty: &DirtyColumn,
        pack_bytes: bool,
        table: &mut Vec<u32>,
        packed: &mut Vec<u8>,
        total_bytes: &mut usize,
    ) {
        let stride = core::mem::size_of::<T>();
        for span in dirty.spans(col.len()) {
            if let Some(slice) = col.page_slice(span.start, span.end) {
                let byte_len = slice.len() * stride;
                if byte_len == 0 {
                    continue;
                }
                let ptr = slice.as_ptr() as usize as u32;
                let dst_off = (span.start * stride) as u32;
                table.extend_from_slice(&[col_id, dst_off, ptr, byte_len as u32]);
                *total_bytes += byte_len;
                if pack_bytes {
                    // Safety: `T` は数値配列（`f32` または `u32`）でパディングを持たない
                    let raw = unsafe {
                        core::slice::from_raw_parts(slice.as_ptr().cast::<u8>(), byte_len)
                    };
                    packed.extend_from_slice(&col_id.to_le_bytes());
                    packed.extend_from_slice(&dst_off.to_le_bytes());
                    packed.extend_from_slice(&(byte_len as u32).to_le_bytes());
                    packed.extend_from_slice(raw);
                }
            }
        }
    }

    visit_slice(
        0,
        &display.segments,
        &display.dirty.segments,
        pack_bytes,
        table,
        packed,
        &mut total_bytes,
    );
    visit_slice(
        1,
        &display.segment_owner,
        &display.dirty.segment_owner,
        pack_bytes,
        table,
        packed,
        &mut total_bytes,
    );
    visit_slice(
        2,
        &display.segment_style,
        &display.dirty.segment_style,
        pack_bytes,
        table,
        packed,
        &mut total_bytes,
    );
    visit_slice(
        3,
        &display.arcs,
        &display.dirty.arcs,
        pack_bytes,
        table,
        packed,
        &mut total_bytes,
    );
    visit_slice(
        4,
        &display.arc_owner,
        &display.dirty.arc_owner,
        pack_bytes,
        table,
        packed,
        &mut total_bytes,
    );
    visit_slice(
        5,
        &display.arc_style,
        &display.dirty.arc_style,
        pack_bytes,
        table,
        packed,
        &mut total_bytes,
    );
    visit_slice(
        6,
        &display.texts,
        &display.dirty.texts,
        pack_bytes,
        table,
        packed,
        &mut total_bytes,
    );
    visit_slice(
        7,
        &display.text_owner,
        &display.dirty.text_owner,
        pack_bytes,
        table,
        packed,
        &mut total_bytes,
    );
    visit_slice(
        8,
        &display.text_style,
        &display.dirty.text_style,
        pack_bytes,
        table,
        packed,
        &mut total_bytes,
    );
    visit_slice(
        9,
        &display.instances,
        &display.dirty.instances,
        pack_bytes,
        table,
        packed,
        &mut total_bytes,
    );
    visit_slice(
        10,
        &display.instance_ref,
        &display.dirty.instance_ref,
        pack_bytes,
        table,
        packed,
        &mut total_bytes,
    );
    visit_slice(
        11,
        &display.instance_owner,
        &display.dirty.instance_owner,
        pack_bytes,
        table,
        packed,
        &mut total_bytes,
    );
    visit_slice(
        12,
        &display.instance_style,
        &display.dirty.instance_style,
        pack_bytes,
        table,
        packed,
        &mut total_bytes,
    );

    display.clear_dirty();
    total_bytes
}

/// 指定サイズ・アライメントのゼロ初期化領域を確保する（入力バイト列・Worker スタック・TLS 用）
#[unsafe(no_mangle)]
pub extern "C" fn wasm_alloc(size: usize, align: usize) -> usize {
    let Ok(layout) = Layout::from_size_align(size.max(1), align.max(1)) else {
        return 0;
    };
    // Safety: `size >= 1` かつ `align` は `from_size_align` で検証済み
    unsafe { alloc_zeroed(layout) as usize }
}

/// [`wasm_alloc`] で確保した領域を解放する
#[unsafe(no_mangle)]
pub extern "C" fn wasm_free(ptr: usize, size: usize, align: usize) {
    if ptr == 0 {
        return;
    }
    let Ok(layout) = Layout::from_size_align(size.max(1), align.max(1)) else {
        return;
    };
    // Safety: 呼び出し側が `wasm_alloc` と同じ引数で渡す
    unsafe { dealloc(ptr as *mut u8, layout) };
}

/// 単一スレッドでスライスを開き、要素数を返す（失敗時は `u32::MAX`）
#[unsafe(no_mangle)]
pub extern "C" fn wasm_open_1t(ptr: usize, len: usize) -> u32 {
    // Safety: 呼び出し側が `wasm_alloc` で確保して書き込んだ入力領域を渡す
    let bytes = unsafe { core::slice::from_raw_parts(ptr as *const u8, len) };
    let Ok(doc) = Document::open_slice(bytes, 1) else {
        return u32::MAX;
    };
    finish_session(doc)
}

/// 共有メモリ並列オープンの第 1 段階を準備する。上位 16 ビットに種類（1=DWG、2=DXF）、下位 16 ビットにタスク数を返す
#[unsafe(no_mangle)]
pub extern "C" fn wasm_mt_begin(ptr: usize, len: usize, workers: usize) -> u32 {
    // Safety: 共有メモリ上の入力スライスを読む
    let bytes = unsafe { core::slice::from_raw_parts(ptr as *const u8, len) };
    let workers = workers.max(1);
    if is_dwg_bytes(bytes) {
        let Ok((file, single, charset)) = prepare_dwg(bytes) else {
            return u32::MAX;
        };
        let n = file.record_count();
        let w = workers.min(n.max(1));
        let mut ranges = Vec::with_capacity(w);
        if w <= 1 || n < 256 {
            ranges.push(0..n);
        } else {
            for k in 0..w {
                let s = n * k / w;
                let e = n * (k + 1) / w;
                if s < e {
                    ranges.push(s..e);
                }
            }
        }
        let count = ranges.len();
        let mut slots = Vec::with_capacity(count);
        slots.resize_with(count, || None);
        if let Ok(mut guard) = MT_STAGE.lock() {
            *guard = Some(MtStage::Dwg {
                raw_len: len,
                file,
                single,
                charset,
                ranges,
                parts: Mutex::new(slots),
            });
        }
        (1u32 << 16) | (count as u32)
    } else {
        let total_pages = len.div_ceil(SourceBytes::PAGE_SIZE);
        let w = workers.min(total_pages.div_ceil(4).max(1));
        let pages_per = total_pages.div_ceil(w);
        let mut ranges = Vec::with_capacity(w);
        if len == 0 {
            ranges.push(0..0);
        } else {
            for k in 0..w {
                let s = (k * pages_per) * SourceBytes::PAGE_SIZE;
                if s >= len {
                    break;
                }
                let e = ((k + 1) * pages_per * SourceBytes::PAGE_SIZE).min(len);
                ranges.push(s..e);
            }
        }
        let count = ranges.len();
        let mut slots = Vec::with_capacity(count);
        slots.resize_with(count, || None);
        if let Ok(mut guard) = MT_STAGE.lock() {
            *guard = Some(MtStage::DxfCompress {
                ptr,
                len,
                ranges,
                parts: Mutex::new(slots),
            });
        }
        (2u32 << 16) | (count as u32)
    }
}

/// 第 1 段階の `task_idx` 番めを Worker スレッドで実行する
#[unsafe(no_mangle)]
pub extern "C" fn wasm_mt_work_a(task_idx: usize) {
    let Ok(guard) = MT_STAGE.lock() else {
        return;
    };
    let Some(stage) = guard.as_ref() else {
        return;
    };
    match stage {
        MtStage::Dwg {
            file,
            single,
            charset,
            ranges,
            parts,
            ..
        } => {
            let Some(range) = ranges.get(task_idx).cloned() else {
                return;
            };
            // ロックを離さずに共有参照だけ取るため、ポインター経由ではなくステージ参照のまま並列実行できるよう
            // `MT_STAGE` 自体は変更しない設計だが、Mutex ガードを保持したままだと直列化してしまう。
            // 共有参照を生ポインターへ取り出してロックをすぐ手放す。
            let file_ptr = core::ptr::from_ref(file);
            let single_ptr = core::ptr::from_ref(single);
            let cs = *charset;
            let parts_ptr = core::ptr::from_ref(parts);
            drop(guard);
            // Safety: `wasm_mt_work_a` の実行完了までメインスレッドは `MT_STAGE` を書き換えない
            let part = unsafe { scan_dwg_range(&*file_ptr, &*single_ptr, cs, range) };
            // Safety: 同上
            if let Ok(mut p_guard) = unsafe { (*parts_ptr).lock() }
                && let Some(slot) = p_guard.get_mut(task_idx)
            {
                *slot = Some(part);
            }
        }
        MtStage::DxfCompress {
            ptr,
            len,
            ranges,
            parts,
        } => {
            let Some(range) = ranges.get(task_idx).cloned() else {
                return;
            };
            let base_ptr = *ptr;
            let total_len = *len;
            let parts_ptr = core::ptr::from_ref(parts);
            drop(guard);
            // Safety: 入力バイト列は `wasm_mt_dxf_stage_b` 完了まで有効
            let all = unsafe { core::slice::from_raw_parts(base_ptr as *const u8, total_len) };
            let sub = all.get(range).unwrap_or_default();
            let comp = compress_pages_slice(sub);
            // Safety: `wasm_mt_work_a` 完了まで `MT_STAGE` は不変
            if let Ok(mut p_guard) = unsafe { (*parts_ptr).lock() }
                && let Some(slot) = p_guard.get_mut(task_idx)
            {
                *slot = Some(comp);
            }
        }
        MtStage::DxfScan { .. } => {}
    }
}

/// DXF の第 2 段階（圧縮済みページの並列スキャン）を準備し、タスク数を返す
#[unsafe(no_mangle)]
pub extern "C" fn wasm_mt_dxf_stage_b(workers: usize) -> u32 {
    let prev = MT_STAGE.lock().ok().and_then(|mut g| g.take());
    let Some(MtStage::DxfCompress { len, parts, .. }) = prev else {
        return u32::MAX;
    };
    let Ok(slots) = parts.into_inner() else {
        return u32::MAX;
    };
    let comp_parts: Vec<CompressedPart> = slots.into_iter().flatten().collect();
    let source = SourceBytes::assemble_parts(len, comp_parts);
    let head_len = source.len().min(256 * 1024);
    let charset = source.with_range(0..head_len, |rs| pre_scan_header(rs.bytes));
    let cuts = split_points_source(&source, workers.max(1));
    let mut ranges = Vec::with_capacity(cuts.len().saturating_sub(1).max(1));
    if cuts.len() <= 2 {
        ranges.push(0..source.len());
    } else {
        for pair in cuts.windows(2) {
            if pair[0] < pair[1] {
                ranges.push(pair[0]..pair[1]);
            }
        }
    }
    let count = ranges.len();
    let mut scan_slots = Vec::with_capacity(count);
    scan_slots.resize_with(count, || None);
    if let Ok(mut guard) = MT_STAGE.lock() {
        *guard = Some(MtStage::DxfScan {
            source,
            charset,
            ranges,
            parts: Mutex::new(scan_slots),
        });
    }
    count as u32
}

/// DXF の第 2 段階の `task_idx` 番めを Worker スレッドで実行する
#[unsafe(no_mangle)]
pub extern "C" fn wasm_mt_work_b(task_idx: usize) {
    let Ok(guard) = MT_STAGE.lock() else {
        return;
    };
    let Some(MtStage::DxfScan {
        source,
        charset,
        ranges,
        parts,
    }) = guard.as_ref()
    else {
        return;
    };
    let Some(range) = ranges.get(task_idx).cloned() else {
        return;
    };
    let src_ptr = core::ptr::from_ref(source);
    let cs = *charset;
    let parts_ptr = core::ptr::from_ref(parts);
    drop(guard);
    // Safety: `wasm_mt_work_b` 完了までメインスレッドは `MT_STAGE` を書き換えない
    let res = unsafe { scan_source_part(&*src_ptr, range, cs) };
    // Safety: 同上
    if let Ok(mut p_guard) = unsafe { (*parts_ptr).lock() }
        && let Some(slot) = p_guard.get_mut(task_idx)
    {
        *slot = Some(res);
    }
}

/// 並列スキャン結果から `Document`・表示バッファー・格子索引を組み上げ、要素数を返す
#[unsafe(no_mangle)]
pub extern "C" fn wasm_mt_finish() -> u32 {
    let prev = MT_STAGE.lock().ok().and_then(|mut g| g.take());
    let scanned = match prev {
        Some(MtStage::Dwg {
            raw_len,
            file,
            single,
            charset,
            parts,
            ..
        }) => {
            let Ok(slots) = parts.into_inner() else {
                return u32::MAX;
            };
            let collected: Vec<DwgPart> = slots.into_iter().flatten().collect();
            Scanned {
                inner: ScannedInner::Dwg(Box::new(ScannedDwg {
                    raw_len,
                    file,
                    single,
                    charset,
                    parts: collected,
                })),
            }
        }
        Some(MtStage::DxfScan {
            source,
            charset,
            parts,
            ..
        }) => {
            let Ok(slots) = parts.into_inner() else {
                return u32::MAX;
            };
            let mut collected = Vec::with_capacity(slots.len());
            for item in slots.into_iter().flatten() {
                let Ok(part) = item else {
                    return u32::MAX;
                };
                collected.push(part);
            }
            Scanned {
                inner: ScannedInner::Dxf {
                    source,
                    parts: collected,
                    guessed_charset: charset,
                },
            }
        }
        _ => return u32::MAX,
    };
    let Ok(mut doc) = Document::assemble_scanned(scanned) else {
        return u32::MAX;
    };
    doc.build_display();
    doc.build_index();
    finish_session(doc)
}

/// 常駐している文書（列・表示・索引・圧縮済み元バイト列）の合計バイト数
#[unsafe(no_mangle)]
pub extern "C" fn wasm_resident_bytes() -> usize {
    SESSION
        .lock()
        .ok()
        .and_then(|g| {
            g.as_ref()
                .map(|s| s.doc.resident_bytes() + s.doc.source_stored_bytes())
        })
        .unwrap_or(0)
}

/// 表示バッファー全体のバイト数
#[unsafe(no_mangle)]
pub extern "C" fn wasm_display_bytes() -> usize {
    SESSION
        .lock()
        .ok()
        .and_then(|g| g.as_ref().map(|s| s.doc.display.bytes()))
        .unwrap_or(0)
}

/// 表示バッファー全体を連続領域へ詰め、そのバイト数を返す（`postMessage` 全体転送の比較用）
#[unsafe(no_mangle)]
pub extern "C" fn wasm_pack_full_display() -> usize {
    let Ok(mut guard) = SESSION.lock() else {
        return 0;
    };
    let Some(s) = guard.as_mut() else {
        return 0;
    };
    s.full_packed.clear();
    s.full_packed.reserve(s.doc.display.bytes());
    fn append_col<T: Copy>(out: &mut Vec<u8>, col: &Paged<T>) {
        let stride = core::mem::size_of::<T>();
        for page in col.pages() {
            let byte_len = page.len() * stride;
            // Safety: 表示列の要素は数値配列でパディングを持たない
            let raw = unsafe { core::slice::from_raw_parts(page.as_ptr().cast::<u8>(), byte_len) };
            out.extend_from_slice(raw);
        }
    }
    append_col(&mut s.full_packed, &s.doc.display.segments);
    append_col(&mut s.full_packed, &s.doc.display.segment_owner);
    append_col(&mut s.full_packed, &s.doc.display.segment_style);
    append_col(&mut s.full_packed, &s.doc.display.arcs);
    append_col(&mut s.full_packed, &s.doc.display.arc_owner);
    append_col(&mut s.full_packed, &s.doc.display.arc_style);
    append_col(&mut s.full_packed, &s.doc.display.texts);
    append_col(&mut s.full_packed, &s.doc.display.text_owner);
    append_col(&mut s.full_packed, &s.doc.display.text_style);
    append_col(&mut s.full_packed, &s.doc.display.instances);
    append_col(&mut s.full_packed, &s.doc.display.instance_ref);
    append_col(&mut s.full_packed, &s.doc.display.instance_owner);
    append_col(&mut s.full_packed, &s.doc.display.instance_style);
    s.full_packed.len()
}

/// [`wasm_pack_full_display`] の先頭ポインター
#[unsafe(no_mangle)]
pub extern "C" fn wasm_full_packed_ptr() -> usize {
    SESSION
        .lock()
        .ok()
        .and_then(|g| g.as_ref().map(|s| s.full_packed.as_ptr() as usize))
        .unwrap_or(0)
}

/// 直前の変更で集めた差分テーブル（`u32 * 4 * N`）の先頭ポインター
#[unsafe(no_mangle)]
pub extern "C" fn wasm_dirty_table_ptr() -> usize {
    SESSION
        .lock()
        .ok()
        .and_then(|g| g.as_ref().map(|s| s.dirty_table.as_ptr() as usize))
        .unwrap_or(0)
}

/// 直前の変更で連結した `postMessage` 用差分バッファーの先頭ポインター
#[unsafe(no_mangle)]
pub extern "C" fn wasm_dirty_packed_ptr() -> usize {
    SESSION
        .lock()
        .ok()
        .and_then(|g| g.as_ref().map(|s| s.dirty_packed.as_ptr() as usize))
        .unwrap_or(0)
}

/// 直前の変更で連結した `postMessage` 用差分バッファーのバイト数
#[unsafe(no_mangle)]
pub extern "C" fn wasm_dirty_packed_len() -> usize {
    SESSION
        .lock()
        .ok()
        .and_then(|g| g.as_ref().map(|s| s.dirty_packed.len()))
        .unwrap_or(0)
}

/// 直前の変更で汚れた表示データの合計バイト数
#[unsafe(no_mangle)]
pub extern "C" fn wasm_dirty_bytes() -> usize {
    SESSION
        .lock()
        .ok()
        .and_then(|g| g.as_ref().map(|s| s.dirty_total_bytes))
        .unwrap_or(0)
}

/// 1 件を動かし、汚れたブロック区間数を返す（`pack_bytes != 0` なら `dirty_packed` も作る）
#[unsafe(no_mangle)]
pub extern "C" fn wasm_move_1(step: u32, pack_bytes: u32) -> u32 {
    let Ok(mut guard) = SESSION.lock() else {
        return 0;
    };
    let Some(s) = guard.as_mut() else {
        return 0;
    };
    if s.handles.is_empty() {
        return 0;
    }
    let h_rand = hash_step(u64::from(step));
    let handle = s.handles[(h_rand as usize) % s.handles.len()];
    let dx = (((h_rand >> 16) & 0xFFFF) as f64 / 65535.0 - 0.5) * (s.span / 64.0);
    let dy = (((h_rand >> 32) & 0xFFFF) as f64 / 65535.0 - 0.5) * (s.span / 64.0);
    let _ = s.doc.apply(&[Edit::Transform {
        handle,
        matrix: [1.0, 0.0, dx, 0.0, 1.0, dy],
    }]);
    s.dirty_total_bytes = collect_dirty_into(
        &mut s.doc.display,
        pack_bytes != 0,
        &mut s.dirty_table,
        &mut s.dirty_packed,
    );
    (s.dirty_table.len() / 4) as u32
}

/// 連続する 100 件を 1 回で動かし、汚れたブロック区間数を返す
#[unsafe(no_mangle)]
pub extern "C" fn wasm_move_100(step: u32, pack_bytes: u32) -> u32 {
    let Ok(mut guard) = SESSION.lock() else {
        return 0;
    };
    let Some(s) = guard.as_mut() else {
        return 0;
    };
    let group = s.handles.len().min(100);
    if group == 0 {
        return 0;
    }
    let h_rand = hash_step(u64::from(step) ^ 0xA5A5_5A5A);
    let first = (h_rand as usize) % (s.handles.len() - group + 1);
    let dx = (((h_rand >> 16) & 0xFFFF) as f64 / 65535.0 - 0.5) * (s.span / 64.0);
    let dy = (((h_rand >> 32) & 0xFFFF) as f64 / 65535.0 - 0.5) * (s.span / 64.0);
    let edits: Vec<Edit> = s.handles[first..first + group]
        .iter()
        .map(|&handle| Edit::Transform {
            handle,
            matrix: [1.0, 0.0, dx, 0.0, 1.0, dy],
        })
        .collect();
    let _ = s.doc.apply(&edits);
    s.dirty_total_bytes = collect_dirty_into(
        &mut s.doc.display,
        pack_bytes != 0,
        &mut s.dirty_table,
        &mut s.dirty_packed,
    );
    (s.dirty_table.len() / 4) as u32
}

/// 線を 1 件足し、汚れたブロック区間数を返す
#[unsafe(no_mangle)]
pub extern "C" fn wasm_add_1(step: u32, pack_bytes: u32) -> u32 {
    let Ok(mut guard) = SESSION.lock() else {
        return 0;
    };
    let Some(s) = guard.as_mut() else {
        return 0;
    };
    let h_rand = hash_step(u64::from(step) ^ 0x3C3C_C3C3);
    let x = s.extent[0] + ((h_rand & 0xFFFF) as f64 / 65535.0) * s.span;
    let y = s.extent[1] + (((h_rand >> 16) & 0xFFFF) as f64 / 65535.0) * s.span;
    let d = (s.span / 256.0).max(1.0);
    let add = Edit::Add(Box::new(Add {
        layer: s.first_layer.clone(),
        color: Some(ColorSpec::Aci(2)),
        line_type: None,
        line_weight: None,
        shape: NewShape::Line {
            start: [x, y],
            end: [x + d, y + d],
        },
    }));
    let _ = s.doc.apply(&[add]);
    s.dirty_total_bytes = collect_dirty_into(
        &mut s.doc.display,
        pack_bytes != 0,
        &mut s.dirty_table,
        &mut s.dirty_packed,
    );
    (s.dirty_table.len() / 4) as u32
}

/// 直前の変更を取り消し、汚れたブロック区間数を返す
#[unsafe(no_mangle)]
pub extern "C" fn wasm_undo(pack_bytes: u32) -> u32 {
    let Ok(mut guard) = SESSION.lock() else {
        return 0;
    };
    let Some(s) = guard.as_mut() else {
        return 0;
    };
    let _ = s.doc.undo();
    s.dirty_total_bytes = collect_dirty_into(
        &mut s.doc.display,
        pack_bytes != 0,
        &mut s.dirty_table,
        &mut s.dirty_packed,
    );
    (s.dirty_table.len() / 4) as u32
}

/// 全体の `1/10 × 1/10` の矩形で範囲選択を行い、選ばれた件数を返す
#[unsafe(no_mangle)]
pub extern "C" fn wasm_select(step: u32) -> u32 {
    let Ok(mut guard) = SESSION.lock() else {
        return 0;
    };
    let Some(s) = guard.as_mut() else {
        return 0;
    };
    let h_rand = hash_step(u64::from(step) ^ 0x55AA_AA55);
    let w = (s.extent[2] - s.extent[0]).max(1.0) * 0.1;
    let h = (s.extent[3] - s.extent[1]).max(1.0) * 0.1;
    let x0 = s.extent[0]
        + ((h_rand & 0xFFFF) as f64 / 65535.0) * (s.extent[2] - s.extent[0] - w).max(0.0);
    let y0 = s.extent[1]
        + (((h_rand >> 16) & 0xFFFF) as f64 / 65535.0) * (s.extent[3] - s.extent[1] - h).max(0.0);
    let mut rows = core::mem::take(&mut s.select_scratch);
    rows.clear();
    s.doc.select([x0, y0, x0 + w, y0 + h], &mut rows);
    let n = rows.len() as u32;
    s.select_scratch = rows;
    n
}
