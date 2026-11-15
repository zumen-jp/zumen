//! 文書エンジンを動かす場所（同一プロセス・別プロセス IPC・単一スレッド wasm・共有メモリ並列 wasm）の測定。
//!
//! 同じ図面に対して次の 4 通りの構成で、開く時間・初回の表示バッファー受け渡し・
//! 1 件／100 件の移動・追加・取り消し・範囲選択の往復時間を測る。
//! 1. ネイティブ（同一プロセス、1 スレッド／並列スレッド、`open_path` と `fs::read + open_slice` の比較を含む）
//! 2. ネイティブ（別プロセス `live worker`、標準入出力パイプでの 64 要素ブロック差分転送と全体転送の比較）
//! 3. wasm（単一スレッド Worker、`postMessage` での差分転送と全体転送の比較）
//! 4. スレッド付き wasm（`SharedArrayBuffer` ＋ `atomics`、1 スレッド／並列 Worker プール、ゼロコピー差分参照）

use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::Instant;

use zumen_bench_live::born::{Add, ColorSpec, NewShape};
use zumen_bench_live::display::{DIRTY_BLOCK, DirtyColumn, Display};
use zumen_bench_live::edit::Edit;
use zumen_bench_live::paged::Paged;
use zumen_bench_live::store::Document;

/// 動かす場所の測定設定
pub struct Options {
    /// 開く計測の反復回数
    pub runs: usize,
    /// 並列スレッド数
    pub threads: usize,
}

const CMD_OPEN_PATH: u8 = 1;
const CMD_FULL_DISPLAY: u8 = 2;
const CMD_MOVE_1: u8 = 3;
const CMD_MOVE_1_FULL: u8 = 4;
const CMD_MOVE_100: u8 = 5;
const CMD_ADD_1: u8 = 6;
const CMD_UNDO: u8 = 7;
const CMD_SELECT: u8 = 8;
const CMD_QUIT: u8 = 9;

#[allow(clippy::disallowed_methods)]
fn now() -> Instant {
    Instant::now()
}

fn us(since: Instant) -> f64 {
    since.elapsed().as_secs_f64() * 1e6
}

fn ms(since: Instant) -> f64 {
    since.elapsed().as_secs_f64() * 1e3
}

fn median(values: &mut [f64]) -> f64 {
    if values.is_empty() {
        return f64::NAN;
    }
    values.sort_by(f64::total_cmp);
    values.get(values.len() / 2).copied().unwrap_or(f64::NAN)
}

fn p99(values: &mut [f64]) -> f64 {
    if values.is_empty() {
        return f64::NAN;
    }
    values.sort_by(f64::total_cmp);
    let idx = ((values.len() as f64 * 0.99) as usize).min(values.len() - 1);
    values.get(idx).copied().unwrap_or(f64::NAN)
}

fn hash_step(seed: u64) -> u64 {
    let mut x = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

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

struct SessionState {
    doc: Document,
    handles: Vec<u64>,
    extent: [f64; 4],
    span: f64,
    first_layer: Option<String>,
    select_scratch: Vec<u32>,
}

impl SessionState {
    fn new(doc: Document) -> Self {
        let handles: Vec<u64> = doc
            .rows
            .iter()
            .filter(|r| r.alive && !doc.duplicate_handles.contains(&r.handle().unwrap_or(0)))
            .filter_map(|r| r.handle())
            .collect();
        let extent = compute_extent(&doc);
        let span = (extent[2] - extent[0]).max(extent[3] - extent[1]).max(1.0);
        let first_layer = doc.layers.first().map(|l| l.name.clone());
        Self {
            doc,
            handles,
            extent,
            span,
            first_layer,
            select_scratch: Vec::with_capacity(4096),
        }
    }

    fn apply_move_1(&mut self, step: u32) {
        if self.handles.is_empty() {
            return;
        }
        let h_rand = hash_step(u64::from(step));
        let handle = self.handles[(h_rand as usize) % self.handles.len()];
        let dx = (((h_rand >> 16) & 0xFFFF) as f64 / 65535.0 - 0.5) * (self.span / 64.0);
        let dy = (((h_rand >> 32) & 0xFFFF) as f64 / 65535.0 - 0.5) * (self.span / 64.0);
        let _ = self.doc.apply(&[Edit::Transform {
            handle,
            matrix: [1.0, 0.0, dx, 0.0, 1.0, dy],
        }]);
    }

    fn apply_move_100(&mut self, step: u32) {
        let group = self.handles.len().min(100);
        if group == 0 {
            return;
        }
        let h_rand = hash_step(u64::from(step) ^ 0xA5A5_5A5A);
        let first = (h_rand as usize) % (self.handles.len() - group + 1);
        let dx = (((h_rand >> 16) & 0xFFFF) as f64 / 65535.0 - 0.5) * (self.span / 64.0);
        let dy = (((h_rand >> 32) & 0xFFFF) as f64 / 65535.0 - 0.5) * (self.span / 64.0);
        let edits: Vec<Edit> = self.handles[first..first + group]
            .iter()
            .map(|&handle| Edit::Transform {
                handle,
                matrix: [1.0, 0.0, dx, 0.0, 1.0, dy],
            })
            .collect();
        let _ = self.doc.apply(&edits);
    }

    fn apply_add_1(&mut self, step: u32) {
        let h_rand = hash_step(u64::from(step) ^ 0x3C3C_C3C3);
        let x = self.extent[0] + ((h_rand & 0xFFFF) as f64 / 65535.0) * self.span;
        let y = self.extent[1] + (((h_rand >> 16) & 0xFFFF) as f64 / 65535.0) * self.span;
        let d = (self.span / 256.0).max(1.0);
        let add = Edit::Add(Box::new(Add {
            layer: self.first_layer.clone(),
            color: Some(ColorSpec::Aci(2)),
            line_type: None,
            line_weight: None,
            shape: NewShape::Line {
                start: [x, y],
                end: [x + d, y + d],
            },
        }));
        let _ = self.doc.apply(&[add]);
    }

    fn apply_undo(&mut self) {
        let _ = self.doc.undo();
    }

    fn run_select(&mut self, step: u32) -> &[u32] {
        let h_rand = hash_step(u64::from(step) ^ 0x55AA_AA55);
        let w = (self.extent[2] - self.extent[0]).max(1.0) * 0.1;
        let h = (self.extent[3] - self.extent[1]).max(1.0) * 0.1;
        let x0 = self.extent[0]
            + ((h_rand & 0xFFFF) as f64 / 65535.0) * (self.extent[2] - self.extent[0] - w).max(0.0);
        let y0 = self.extent[1]
            + (((h_rand >> 16) & 0xFFFF) as f64 / 65535.0)
                * (self.extent[3] - self.extent[1] - h).max(0.0);
        self.select_scratch.clear();
        self.doc
            .select([x0, y0, x0 + w, y0 + h], &mut self.select_scratch);
        &self.select_scratch
    }
}

fn write_f32_slice<const N: usize>(
    w: &mut impl Write,
    slice: &[[f32; N]],
    scratch: &mut Vec<u8>,
) -> std::io::Result<()> {
    scratch.clear();
    scratch.reserve(slice.len() * N * 4);
    for item in slice {
        for &v in item {
            scratch.extend_from_slice(&v.to_le_bytes());
        }
    }
    w.write_all(scratch)
}

fn write_u32_slice(
    w: &mut impl Write,
    slice: &[u32],
    scratch: &mut Vec<u8>,
) -> std::io::Result<()> {
    scratch.clear();
    scratch.reserve(slice.len() * 4);
    for &v in slice {
        scratch.extend_from_slice(&v.to_le_bytes());
    }
    w.write_all(scratch)
}

fn write_u32_arr_slice<const N: usize>(
    w: &mut impl Write,
    slice: &[[u32; N]],
    scratch: &mut Vec<u8>,
) -> std::io::Result<()> {
    scratch.clear();
    scratch.reserve(slice.len() * N * 4);
    for item in slice {
        for &v in item {
            scratch.extend_from_slice(&v.to_le_bytes());
        }
    }
    w.write_all(scratch)
}

fn col_bytes<T>(col: &Paged<T>) -> usize {
    col.len() * core::mem::size_of::<T>()
}

fn display_payload_bytes(display: &Display) -> usize {
    col_bytes(&display.segments)
        + col_bytes(&display.segment_owner)
        + col_bytes(&display.segment_style)
        + col_bytes(&display.arcs)
        + col_bytes(&display.arc_owner)
        + col_bytes(&display.arc_style)
        + col_bytes(&display.texts)
        + col_bytes(&display.text_owner)
        + col_bytes(&display.text_style)
        + col_bytes(&display.instances)
        + col_bytes(&display.instance_ref)
        + col_bytes(&display.instance_owner)
        + col_bytes(&display.instance_style)
}

fn stream_full_display(
    display: &Display,
    w: &mut impl Write,
    scratch: &mut Vec<u8>,
) -> std::io::Result<()> {
    let total = display_payload_bytes(display) as u64;
    w.write_all(&total.to_le_bytes())?;
    for page in display.segments.pages() {
        write_f32_slice(w, page, scratch)?;
    }
    for page in display.segment_owner.pages() {
        write_u32_slice(w, page, scratch)?;
    }
    for page in display.segment_style.pages() {
        write_u32_arr_slice(w, page, scratch)?;
    }
    for page in display.arcs.pages() {
        write_f32_slice(w, page, scratch)?;
    }
    for page in display.arc_owner.pages() {
        write_u32_slice(w, page, scratch)?;
    }
    for page in display.arc_style.pages() {
        write_u32_arr_slice(w, page, scratch)?;
    }
    for page in display.texts.pages() {
        write_f32_slice(w, page, scratch)?;
    }
    for page in display.text_owner.pages() {
        write_u32_slice(w, page, scratch)?;
    }
    for page in display.text_style.pages() {
        write_u32_arr_slice(w, page, scratch)?;
    }
    for page in display.instances.pages() {
        write_f32_slice(w, page, scratch)?;
    }
    for page in display.instance_ref.pages() {
        write_u32_arr_slice(w, page, scratch)?;
    }
    for page in display.instance_owner.pages() {
        write_u32_slice(w, page, scratch)?;
    }
    for page in display.instance_style.pages() {
        write_u32_arr_slice(w, page, scratch)?;
    }
    Ok(())
}

fn pack_dirty_frames(display: &mut Display, out: &mut Vec<u8>) -> (u32, u32) {
    out.clear();
    out.extend_from_slice(&[0u8; 8]); // span_count, dirty_bytes の予約領域
    let mut span_count = 0u32;
    let mut dirty_bytes = 0u32;

    fn add_f32<const N: usize>(
        col_id: u32,
        col: &Paged<[f32; N]>,
        dirty: &DirtyColumn,
        out: &mut Vec<u8>,
        span_count: &mut u32,
        dirty_bytes: &mut u32,
    ) {
        let stride = N * 4;
        for span in dirty.spans(col.len()) {
            if let Some(slice) = col.page_slice(span.start, span.end) {
                let b_len = (slice.len() * stride) as u32;
                if b_len == 0 {
                    continue;
                }
                *span_count += 1;
                *dirty_bytes += b_len;
                out.extend_from_slice(&col_id.to_le_bytes());
                out.extend_from_slice(&((span.start * stride) as u32).to_le_bytes());
                out.extend_from_slice(&b_len.to_le_bytes());
                out.reserve(b_len as usize);
                for item in slice {
                    for &v in item {
                        out.extend_from_slice(&v.to_le_bytes());
                    }
                }
            }
        }
    }

    fn add_u32(
        col_id: u32,
        col: &Paged<u32>,
        dirty: &DirtyColumn,
        out: &mut Vec<u8>,
        span_count: &mut u32,
        dirty_bytes: &mut u32,
    ) {
        for span in dirty.spans(col.len()) {
            if let Some(slice) = col.page_slice(span.start, span.end) {
                let b_len = (slice.len() * 4) as u32;
                if b_len == 0 {
                    continue;
                }
                *span_count += 1;
                *dirty_bytes += b_len;
                out.extend_from_slice(&col_id.to_le_bytes());
                out.extend_from_slice(&((span.start * 4) as u32).to_le_bytes());
                out.extend_from_slice(&b_len.to_le_bytes());
                out.reserve(b_len as usize);
                for &v in slice {
                    out.extend_from_slice(&v.to_le_bytes());
                }
            }
        }
    }

    fn add_u32_arr<const N: usize>(
        col_id: u32,
        col: &Paged<[u32; N]>,
        dirty: &DirtyColumn,
        out: &mut Vec<u8>,
        span_count: &mut u32,
        dirty_bytes: &mut u32,
    ) {
        let stride = N * 4;
        for span in dirty.spans(col.len()) {
            if let Some(slice) = col.page_slice(span.start, span.end) {
                let b_len = (slice.len() * stride) as u32;
                if b_len == 0 {
                    continue;
                }
                *span_count += 1;
                *dirty_bytes += b_len;
                out.extend_from_slice(&col_id.to_le_bytes());
                out.extend_from_slice(&((span.start * stride) as u32).to_le_bytes());
                out.extend_from_slice(&b_len.to_le_bytes());
                out.reserve(b_len as usize);
                for item in slice {
                    for &v in item {
                        out.extend_from_slice(&v.to_le_bytes());
                    }
                }
            }
        }
    }

    add_f32(
        0,
        &display.segments,
        &display.dirty.segments,
        out,
        &mut span_count,
        &mut dirty_bytes,
    );
    add_u32(
        1,
        &display.segment_owner,
        &display.dirty.segment_owner,
        out,
        &mut span_count,
        &mut dirty_bytes,
    );
    add_u32_arr(
        2,
        &display.segment_style,
        &display.dirty.segment_style,
        out,
        &mut span_count,
        &mut dirty_bytes,
    );
    add_f32(
        3,
        &display.arcs,
        &display.dirty.arcs,
        out,
        &mut span_count,
        &mut dirty_bytes,
    );
    add_u32(
        4,
        &display.arc_owner,
        &display.dirty.arc_owner,
        out,
        &mut span_count,
        &mut dirty_bytes,
    );
    add_u32_arr(
        5,
        &display.arc_style,
        &display.dirty.arc_style,
        out,
        &mut span_count,
        &mut dirty_bytes,
    );
    add_f32(
        6,
        &display.texts,
        &display.dirty.texts,
        out,
        &mut span_count,
        &mut dirty_bytes,
    );
    add_u32(
        7,
        &display.text_owner,
        &display.dirty.text_owner,
        out,
        &mut span_count,
        &mut dirty_bytes,
    );
    add_u32_arr(
        8,
        &display.text_style,
        &display.dirty.text_style,
        out,
        &mut span_count,
        &mut dirty_bytes,
    );
    add_f32(
        9,
        &display.instances,
        &display.dirty.instances,
        out,
        &mut span_count,
        &mut dirty_bytes,
    );
    add_u32_arr(
        10,
        &display.instance_ref,
        &display.dirty.instance_ref,
        out,
        &mut span_count,
        &mut dirty_bytes,
    );
    add_u32(
        11,
        &display.instance_owner,
        &display.dirty.instance_owner,
        out,
        &mut span_count,
        &mut dirty_bytes,
    );
    add_u32_arr(
        12,
        &display.instance_style,
        &display.dirty.instance_style,
        out,
        &mut span_count,
        &mut dirty_bytes,
    );

    out[0..4].copy_from_slice(&span_count.to_le_bytes());
    out[4..8].copy_from_slice(&dirty_bytes.to_le_bytes());
    display.clear_dirty();
    (span_count, dirty_bytes)
}

/// 別プロセスとして起動されるバイナリ IPC ワーカー
pub fn worker(threads: usize) -> Result<(), String> {
    let mut reader = BufReader::with_capacity(65536, std::io::stdin().lock());
    let mut writer = BufWriter::with_capacity(262_144, std::io::stdout().lock());
    let mut session: Option<SessionState> = None;
    let mut scratch = Vec::with_capacity(65536);
    let mut dirty_buf = Vec::with_capacity(16384);

    loop {
        let mut cmd = [0u8; 1];
        if reader.read_exact(&mut cmd).is_err() {
            break;
        }
        match cmd[0] {
            CMD_OPEN_PATH => {
                let mut len_b = [0u8; 4];
                reader
                    .read_exact(&mut len_b)
                    .map_err(|e| format!("IPC read: {e}"))?;
                let len = u32::from_le_bytes(len_b) as usize;
                let mut path_b = vec![0u8; len];
                reader
                    .read_exact(&mut path_b)
                    .map_err(|e| format!("IPC read path: {e}"))?;
                let path_str = String::from_utf8(path_b).map_err(|e| format!("UTF-8 path: {e}"))?;
                let t0 = now();
                match Document::open_path(Path::new(&path_str), threads) {
                    Ok(doc) => {
                        let open_us = us(t0);
                        let elem_count = doc.counts.elements;
                        let disp_bytes = doc.display.bytes() as u64;
                        session = Some(SessionState::new(doc));
                        writer.write_all(&[1u8]).map_err(|e| e.to_string())?;
                        writer
                            .write_all(&elem_count.to_le_bytes())
                            .map_err(|e| e.to_string())?;
                        writer
                            .write_all(&disp_bytes.to_le_bytes())
                            .map_err(|e| e.to_string())?;
                        writer
                            .write_all(&open_us.to_le_bytes())
                            .map_err(|e| e.to_string())?;
                    }
                    Err(_) => {
                        writer.write_all(&[0u8]).map_err(|e| e.to_string())?;
                    }
                }
                writer.flush().map_err(|e| e.to_string())?;
            }
            CMD_FULL_DISPLAY => {
                if let Some(s) = session.as_ref() {
                    stream_full_display(&s.doc.display, &mut writer, &mut scratch)
                        .map_err(|e| e.to_string())?;
                } else {
                    writer
                        .write_all(&0u64.to_le_bytes())
                        .map_err(|e| e.to_string())?;
                }
                writer.flush().map_err(|e| e.to_string())?;
            }
            CMD_MOVE_1 => {
                let mut b = [0u8; 4];
                reader.read_exact(&mut b).map_err(|e| e.to_string())?;
                let step = u32::from_le_bytes(b);
                if let Some(s) = session.as_mut() {
                    s.apply_move_1(step);
                    pack_dirty_frames(&mut s.doc.display, &mut dirty_buf);
                    writer.write_all(&dirty_buf).map_err(|e| e.to_string())?;
                }
                writer.flush().map_err(|e| e.to_string())?;
            }
            CMD_MOVE_1_FULL => {
                let mut b = [0u8; 4];
                reader.read_exact(&mut b).map_err(|e| e.to_string())?;
                let step = u32::from_le_bytes(b);
                if let Some(s) = session.as_mut() {
                    s.apply_move_1(step);
                    s.doc.display.clear_dirty();
                    stream_full_display(&s.doc.display, &mut writer, &mut scratch)
                        .map_err(|e| e.to_string())?;
                }
                writer.flush().map_err(|e| e.to_string())?;
            }
            CMD_MOVE_100 => {
                let mut b = [0u8; 4];
                reader.read_exact(&mut b).map_err(|e| e.to_string())?;
                let step = u32::from_le_bytes(b);
                if let Some(s) = session.as_mut() {
                    s.apply_move_100(step);
                    pack_dirty_frames(&mut s.doc.display, &mut dirty_buf);
                    writer.write_all(&dirty_buf).map_err(|e| e.to_string())?;
                }
                writer.flush().map_err(|e| e.to_string())?;
            }
            CMD_ADD_1 => {
                let mut b = [0u8; 4];
                reader.read_exact(&mut b).map_err(|e| e.to_string())?;
                let step = u32::from_le_bytes(b);
                if let Some(s) = session.as_mut() {
                    s.apply_add_1(step);
                    pack_dirty_frames(&mut s.doc.display, &mut dirty_buf);
                    writer.write_all(&dirty_buf).map_err(|e| e.to_string())?;
                }
                writer.flush().map_err(|e| e.to_string())?;
            }
            CMD_UNDO => {
                if let Some(s) = session.as_mut() {
                    s.apply_undo();
                    pack_dirty_frames(&mut s.doc.display, &mut dirty_buf);
                    writer.write_all(&dirty_buf).map_err(|e| e.to_string())?;
                }
                writer.flush().map_err(|e| e.to_string())?;
            }
            CMD_SELECT => {
                let mut b = [0u8; 4];
                reader.read_exact(&mut b).map_err(|e| e.to_string())?;
                let step = u32::from_le_bytes(b);
                if let Some(s) = session.as_mut() {
                    let rows = s.run_select(step);
                    writer
                        .write_all(&(rows.len() as u32).to_le_bytes())
                        .map_err(|e| e.to_string())?;
                    write_u32_slice(&mut writer, rows, &mut scratch).map_err(|e| e.to_string())?;
                }
                writer.flush().map_err(|e| e.to_string())?;
            }
            CMD_QUIT => break,
            _ => return Err(format!("未知の IPC コマンド: {}", cmd[0])),
        }
    }
    Ok(())
}

struct IpcClient {
    child: Child,
    stdin: BufWriter<ChildStdin>,
    stdout: BufReader<ChildStdout>,
    mirror: Vec<u8>,
}

impl IpcClient {
    fn spawn(threads: usize) -> Result<Self, String> {
        let exe = std::env::current_exe().map_err(|e| format!("current_exe: {e}"))?;
        let mut child = Command::new(exe)
            .arg("worker")
            .arg("--threads")
            .arg(threads.to_string())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .map_err(|e| format!("worker を起動できない: {e}"))?;
        let stdin = BufWriter::new(child.stdin.take().ok_or("stdin なし")?);
        let stdout = BufReader::with_capacity(262_144, child.stdout.take().ok_or("stdout なし")?);
        Ok(Self {
            child,
            stdin,
            stdout,
            mirror: Vec::new(),
        })
    }

    fn open_path(&mut self, path: &str) -> Result<(u32, u64, f64), String> {
        let b = path.as_bytes();
        self.stdin
            .write_all(&[CMD_OPEN_PATH])
            .map_err(|e| e.to_string())?;
        self.stdin
            .write_all(&(b.len() as u32).to_le_bytes())
            .map_err(|e| e.to_string())?;
        self.stdin.write_all(b).map_err(|e| e.to_string())?;
        self.stdin.flush().map_err(|e| e.to_string())?;

        let mut ok = [0u8; 1];
        self.stdout
            .read_exact(&mut ok)
            .map_err(|e| format!("IPC open 応答失敗: {e}"))?;
        if ok[0] != 1 {
            return Err(format!("{path} を worker で開けない"));
        }
        let mut buf = [0u8; 20];
        self.stdout
            .read_exact(&mut buf)
            .map_err(|e| e.to_string())?;
        let elems = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
        let disp_bytes = u64::from_le_bytes(buf[4..12].try_into().unwrap_or([0; 8]));
        let open_us = f64::from_le_bytes(buf[12..20].try_into().unwrap_or([0; 8]));
        Ok((elems, disp_bytes, open_us))
    }

    fn recv_full_display(&mut self) -> Result<usize, String> {
        let mut len_b = [0u8; 8];
        self.stdout
            .read_exact(&mut len_b)
            .map_err(|e| e.to_string())?;
        let total = u64::from_le_bytes(len_b) as usize;
        if self.mirror.len() < total {
            self.mirror.resize(total, 0);
        }
        self.stdout
            .read_exact(&mut self.mirror[..total])
            .map_err(|e| e.to_string())?;
        Ok(total)
    }

    fn pull_full_display(&mut self) -> Result<usize, String> {
        self.stdin
            .write_all(&[CMD_FULL_DISPLAY])
            .map_err(|e| e.to_string())?;
        self.stdin.flush().map_err(|e| e.to_string())?;
        self.recv_full_display()
    }

    fn recv_dirty(&mut self) -> Result<(u32, u32), String> {
        let mut head = [0u8; 8];
        self.stdout
            .read_exact(&mut head)
            .map_err(|e| e.to_string())?;
        let spans = u32::from_le_bytes([head[0], head[1], head[2], head[3]]);
        let dirty_b = u32::from_le_bytes([head[4], head[5], head[6], head[7]]);
        for _ in 0..spans {
            let mut sh = [0u8; 12];
            self.stdout.read_exact(&mut sh).map_err(|e| e.to_string())?;
            let _col_id = u32::from_le_bytes([sh[0], sh[1], sh[2], sh[3]]);
            let dst_off = u32::from_le_bytes([sh[4], sh[5], sh[6], sh[7]]) as usize;
            let b_len = u32::from_le_bytes([sh[8], sh[9], sh[10], sh[11]]) as usize;
            let need = dst_off.saturating_add(b_len);
            if self.mirror.len() < need {
                self.mirror.resize(need, 0);
            }
            self.stdout
                .read_exact(&mut self.mirror[dst_off..dst_off + b_len])
                .map_err(|e| e.to_string())?;
        }
        Ok((spans, dirty_b))
    }

    fn step_dirty(&mut self, cmd: u8, step: Option<u32>) -> Result<(u32, u32), String> {
        self.stdin.write_all(&[cmd]).map_err(|e| e.to_string())?;
        if let Some(s) = step {
            self.stdin
                .write_all(&s.to_le_bytes())
                .map_err(|e| e.to_string())?;
        }
        self.stdin.flush().map_err(|e| e.to_string())?;
        self.recv_dirty()
    }

    fn move_1_full(&mut self, step: u32) -> Result<usize, String> {
        self.stdin
            .write_all(&[CMD_MOVE_1_FULL])
            .map_err(|e| e.to_string())?;
        self.stdin
            .write_all(&step.to_le_bytes())
            .map_err(|e| e.to_string())?;
        self.stdin.flush().map_err(|e| e.to_string())?;
        self.recv_full_display()
    }

    fn select(&mut self, step: u32, out: &mut Vec<u8>) -> Result<u32, String> {
        self.stdin
            .write_all(&[CMD_SELECT])
            .map_err(|e| e.to_string())?;
        self.stdin
            .write_all(&step.to_le_bytes())
            .map_err(|e| e.to_string())?;
        self.stdin.flush().map_err(|e| e.to_string())?;
        let mut c = [0u8; 4];
        self.stdout.read_exact(&mut c).map_err(|e| e.to_string())?;
        let count = u32::from_le_bytes(c);
        let byte_len = count as usize * 4;
        if out.len() < byte_len {
            out.resize(byte_len, 0);
        }
        self.stdout
            .read_exact(&mut out[..byte_len])
            .map_err(|e| e.to_string())?;
        Ok(count)
    }

    fn quit(mut self) {
        let _ = self.stdin.write_all(&[CMD_QUIT]);
        let _ = self.stdin.flush();
        let _ = self.child.wait();
    }
}

fn measure_in_process(path: &str, options: &Options) -> Result<(), String> {
    let p = Path::new(path);
    // 1. open_path（1T・並列）と fs::read + open_slice（1T・並列）を測る
    let mut open_path_1t = Vec::with_capacity(options.runs);
    let mut open_path_mt = Vec::with_capacity(options.runs);
    let mut read_only_ms = Vec::with_capacity(options.runs);
    let mut open_slice_1t = Vec::with_capacity(options.runs);
    let mut open_slice_mt = Vec::with_capacity(options.runs);

    // ファイルシステムキャッシュのウォームアップ
    let warm_bytes = std::fs::read(p).map_err(|e| format!("{path} を読めない: {e}"))?;
    let _ = Document::open_slice(&warm_bytes, options.threads)
        .map_err(|e| format!("open_slice: {e:?}"))?;
    drop(warm_bytes);

    for _ in 0..options.runs {
        let t = now();
        let d = Document::open_path(p, 1)?;
        open_path_1t.push(ms(t));
        drop(d);

        let t = now();
        let d = Document::open_path(p, options.threads)?;
        open_path_mt.push(ms(t));
        drop(d);

        let t_read = now();
        let raw = std::fs::read(p).map_err(|e| e.to_string())?;
        read_only_ms.push(ms(t_read));

        let t = now();
        let d1 = Document::open_slice(&raw, 1).map_err(|e| format!("{e:?}"))?;
        open_slice_1t.push(ms(t));
        drop(d1);

        let t = now();
        let dm = Document::open_slice(&raw, options.threads).map_err(|e| format!("{e:?}"))?;
        open_slice_mt.push(ms(t));
        drop(dm);
    }

    let doc = Document::open_path(p, options.threads)?;
    let elem_count = doc.counts.elements;
    let disp_mib = display_payload_bytes(&doc.display) as f64 / (1024.0 * 1024.0);
    let mut sess = SessionState::new(doc);
    let mut dirty_buf = Vec::with_capacity(16384);

    let steps = 60usize;
    let mut t_move1 = Vec::with_capacity(steps);
    let mut b_move1 = Vec::with_capacity(steps);
    for s in 0..steps as u32 {
        let t = now();
        sess.apply_move_1(s);
        let (_, b) = pack_dirty_frames(&mut sess.doc.display, &mut dirty_buf);
        t_move1.push(us(t));
        b_move1.push(f64::from(b));
    }

    let mut t_move100 = Vec::with_capacity(steps);
    let mut b_move100 = Vec::with_capacity(steps);
    for s in 0..steps as u32 {
        let t = now();
        sess.apply_move_100(s);
        let (_, b) = pack_dirty_frames(&mut sess.doc.display, &mut dirty_buf);
        t_move100.push(us(t));
        b_move100.push(f64::from(b));
    }

    let mut t_add1 = Vec::with_capacity(30);
    let mut t_undo = Vec::with_capacity(30);
    for s in 0..30u32 {
        let t = now();
        sess.apply_add_1(s);
        let _ = pack_dirty_frames(&mut sess.doc.display, &mut dirty_buf);
        t_add1.push(us(t));

        let t = now();
        sess.apply_undo();
        let _ = pack_dirty_frames(&mut sess.doc.display, &mut dirty_buf);
        t_undo.push(us(t));
    }

    let mut t_select = Vec::with_capacity(steps);
    for s in 0..steps as u32 {
        let t = now();
        let _ = sess.run_select(s);
        t_select.push(us(t));
    }

    println!(
        "== {path}（要素 {elem_count}、表示バッファー {disp_mib:.2} MiB、差分ブロック {DIRTY_BLOCK} 要素）"
    );
    println!("  [1] ネイティブ（同一プロセス）");
    println!(
        "    開く（open_path ストリーミング圧縮）: 1T {:.2} ms ／ {}T {:.2} ms",
        median(&mut open_path_1t),
        options.threads,
        median(&mut open_path_mt),
    );
    println!(
        "    開く（fs::read {:.2} ms ＋ open_slice 連続メモリ）: 1T {:.2} ms ／ {}T {:.2} ms",
        median(&mut read_only_ms),
        median(&mut open_slice_1t),
        options.threads,
        median(&mut open_slice_mt),
    );
    println!(
        "    操作（差分抽出込み）: 1 件移動 {:.1} µs（99% {:.1} µs、差分 {:.0} B）、100 件移動 {:.1} µs（差分 {:.0} B）、1 件追加 {:.1} µs、取り消し {:.1} µs、範囲選択 {:.1} µs",
        median(&mut t_move1),
        p99(&mut t_move1),
        median(&mut b_move1),
        median(&mut t_move100),
        median(&mut b_move100),
        median(&mut t_add1),
        median(&mut t_undo),
        median(&mut t_select),
    );
    Ok(())
}

fn measure_ipc(path: &str, options: &Options) -> Result<(), String> {
    let mut ipc_1t = IpcClient::spawn(1)?;
    let _ = ipc_1t.open_path(path)?;
    let mut open_1t = Vec::with_capacity(options.runs);
    for _ in 0..options.runs {
        let t = now();
        let _ = ipc_1t.open_path(path)?;
        open_1t.push(ms(t));
    }
    ipc_1t.quit();

    let mut ipc = IpcClient::spawn(options.threads)?;
    let _ = ipc.open_path(path)?;
    let _ = ipc.pull_full_display()?;

    let mut open_mt = Vec::with_capacity(options.runs);
    let mut open_plus_full = Vec::with_capacity(options.runs);
    for _ in 0..options.runs {
        let t = now();
        let _ = ipc.open_path(path)?;
        open_mt.push(ms(t));
        let _ = ipc.pull_full_display()?;
        open_plus_full.push(ms(t));
    }

    let steps = 60usize;
    let mut t_move1 = Vec::with_capacity(steps);
    let mut b_move1 = Vec::with_capacity(steps);
    for s in 0..steps as u32 {
        let t = now();
        let (_, b) = ipc.step_dirty(CMD_MOVE_1, Some(s))?;
        t_move1.push(us(t));
        b_move1.push(f64::from(b));
    }

    let mut t_move1_full = Vec::with_capacity(7);
    for s in 0..7u32 {
        let t = now();
        let _ = ipc.move_1_full(s)?;
        t_move1_full.push(ms(t));
    }

    let mut t_move100 = Vec::with_capacity(steps);
    for s in 0..steps as u32 {
        let t = now();
        let _ = ipc.step_dirty(CMD_MOVE_100, Some(s))?;
        t_move100.push(us(t));
    }

    let mut t_add1 = Vec::with_capacity(30);
    let mut t_undo = Vec::with_capacity(30);
    for s in 0..30u32 {
        let t = now();
        let _ = ipc.step_dirty(CMD_ADD_1, Some(s))?;
        t_add1.push(us(t));

        let t = now();
        let _ = ipc.step_dirty(CMD_UNDO, None)?;
        t_undo.push(us(t));
    }

    let mut sel_buf = Vec::new();
    let mut t_select = Vec::with_capacity(steps);
    for s in 0..steps as u32 {
        let t = now();
        let _ = ipc.select(s, &mut sel_buf)?;
        t_select.push(us(t));
    }
    ipc.quit();

    println!("  [2] ネイティブ（別プロセス、標準入出力パイプ IPC）");
    println!(
        "    開く（パス通知のみ）: 1T {:.2} ms ／ {}T {:.2} ms（＋ 初回表示バッファー全体パイプ転送込み: {:.2} ms）",
        median(&mut open_1t),
        options.threads,
        median(&mut open_mt),
        median(&mut open_plus_full),
    );
    println!(
        "    操作（IPC 往復 ＋ 差分パイプ転送）: 1 件移動 {:.1} µs（99% {:.1} µs、差分 {:.0} B）、100 件移動 {:.1} µs、1 件追加 {:.1} µs、取り消し {:.1} µs、範囲選択 {:.1} µs",
        median(&mut t_move1),
        p99(&mut t_move1),
        median(&mut b_move1),
        median(&mut t_move100),
        median(&mut t_add1),
        median(&mut t_undo),
        median(&mut t_select),
    );
    println!(
        "    （比較）1 件移動で表示バッファー全体をパイプ転送した場合: {:.2} ms",
        median(&mut t_move1_full),
    );
    Ok(())
}

const NODE_WASM_RUNNER: &str = r#"import fs from "node:fs";
import { Worker, isMainThread, parentPort, workerData } from "node:worker_threads";
import { performance } from "node:perf_hooks";
import { fileURLToPath } from "node:url";

const SELF_PATH = fileURLToPath(import.meta.url);

function stats(arr) {
  if (!arr.length) return { median: 0, p99: 0 };
  const s = [...arr].sort((a, b) => a - b);
  return {
    median: s[Math.floor(s.length / 2)],
    p99: s[Math.min(Math.floor(s.length * 0.99), s.length - 1)],
  };
}

if (!isMainThread) {
  const mode = workerData.mode;
  if (mode === "1t-worker") {
    const wasmBytes = fs.readFileSync(workerData.wasmPath);
    const { instance } = await WebAssembly.instantiate(wasmBytes, {});
    const ex = instance.exports;
    parentPort.on("message", (msg) => {
      if (msg.op === "open") {
        const u8 = new Uint8Array(msg.ab);
        const ptr = ex.wasm_alloc(u8.byteLength, 16);
        new Uint8Array(ex.memory.buffer, ptr, u8.byteLength).set(u8);
        const t0 = performance.now();
        const elems = ex.wasm_open_1t(ptr, u8.byteLength);
        const coreMs = performance.now() - t0;
        ex.wasm_free(ptr, u8.byteLength, 16);
        let initAb = null;
        if (msg.withDisplay) {
          const fullLen = ex.wasm_pack_full_display();
          const fullPtr = ex.wasm_full_packed_ptr();
          initAb = ex.memory.buffer.slice(fullPtr, fullPtr + fullLen);
        }
        parentPort.postMessage(
          { elems, coreMs, initAb },
          initAb ? [initAb] : []
        );
      } else if (msg.op === "move1") {
        ex.wasm_move_1(msg.step, 1);
        const pLen = ex.wasm_dirty_packed_len();
        const pPtr = ex.wasm_dirty_packed_ptr();
        const dBytes = ex.wasm_dirty_bytes();
        const ab = ex.memory.buffer.slice(pPtr, pPtr + pLen);
        parentPort.postMessage({ ab, dBytes }, [ab]);
      } else if (msg.op === "move1Full") {
        ex.wasm_move_1(msg.step, 0);
        const fullLen = ex.wasm_pack_full_display();
        const fullPtr = ex.wasm_full_packed_ptr();
        const ab = ex.memory.buffer.slice(fullPtr, fullPtr + fullLen);
        parentPort.postMessage({ ab }, [ab]);
      } else if (msg.op === "move100") {
        ex.wasm_move_100(msg.step, 1);
        const pLen = ex.wasm_dirty_packed_len();
        const pPtr = ex.wasm_dirty_packed_ptr();
        const ab = ex.memory.buffer.slice(pPtr, pPtr + pLen);
        parentPort.postMessage({ ab }, [ab]);
      } else if (msg.op === "add1") {
        ex.wasm_add_1(msg.step, 1);
        const pLen = ex.wasm_dirty_packed_len();
        const pPtr = ex.wasm_dirty_packed_ptr();
        const ab = ex.memory.buffer.slice(pPtr, pPtr + pLen);
        parentPort.postMessage({ ab }, [ab]);
      } else if (msg.op === "undo") {
        ex.wasm_undo(1);
        const pLen = ex.wasm_dirty_packed_len();
        const pPtr = ex.wasm_dirty_packed_ptr();
        const ab = ex.memory.buffer.slice(pPtr, pPtr + pLen);
        parentPort.postMessage({ ab }, [ab]);
      } else if (msg.op === "select") {
        const count = ex.wasm_select(msg.step);
        parentPort.postMessage({ count });
      }
    });
  } else if (mode === "mt-helper") {
    const { mod, memory, ctrlSab, helperId, stackTop, tlsPtr } = workerData;
    const inst = new WebAssembly.Instance(mod, { env: { memory } });
    const ex = inst.exports;
    ex.__stack_pointer.value = stackTop;
    ex.__wasm_init_tls(tlsPtr);
    const ctrl = new Int32Array(ctrlSab);
    parentPort.postMessage({ ready: true });
    let seenEpoch = 0;
    while (true) {
      Atomics.wait(ctrl, 0, seenEpoch);
      const curEpoch = Atomics.load(ctrl, 0);
      if (curEpoch < 0) break;
      if (curEpoch === seenEpoch) continue;
      seenEpoch = curEpoch;
      const limit = Atomics.load(ctrl, 4);
      if (helperId >= limit) continue;
      const phase = Atomics.load(ctrl, 1);
      const total = Atomics.load(ctrl, 3);
      while (true) {
        const task = Atomics.add(ctrl, 2, 1);
        if (task >= total) break;
        if (phase === 1) {
          ex.wasm_mt_work_a(task);
        } else {
          ex.wasm_mt_work_b(task);
        }
        if (Atomics.sub(ctrl, 5, 1) === 1) {
          Atomics.notify(ctrl, 5, 1);
        }
      }
    }
  } else if (mode === "mt-lead") {
    const wasmBytes = fs.readFileSync(workerData.wasmMtPath);
    const maxThreads = workerData.maxThreads;
    const mod = await WebAssembly.compile(wasmBytes);
    const memory = new WebAssembly.Memory({ initial: 512, maximum: 32768, shared: true });
    const inst = new WebAssembly.Instance(mod, { env: { memory } });
    const ex = inst.exports;
    const tlsSize = Math.max(ex.__tls_size ? ex.__tls_size.value : 64, 64);
    const tlsAlign = Math.max(ex.__tls_align ? ex.__tls_align.value : 16, 16);
    const leadTls = ex.wasm_alloc(tlsSize, tlsAlign);
    if (ex.__wasm_init_tls) ex.__wasm_init_tls(leadTls);

    const ctrlSab = new SharedArrayBuffer(64);
    const ctrl = new Int32Array(ctrlSab);
    const helperCount = Math.max(0, maxThreads - 1);
    const helpers = [];
    for (let h = 0; h < helperCount; h++) {
      const stackSize = 2 * 1024 * 1024;
      const stackBase = ex.wasm_alloc(stackSize, 16);
      const stackTop = stackBase + stackSize;
      const hTls = ex.wasm_alloc(tlsSize, tlsAlign);
      const w = new Worker(SELF_PATH, {
        workerData: { mode: "mt-helper", mod, memory, ctrlSab, helperId: h, stackTop, tlsPtr: hTls },
      });
      w.on("error", (err) => { throw err; });
      await new Promise((res, rej) => { w.once("error", rej); w.once("message", res); });
      helpers.push(w);
    }

    const dispatchPhase = (phase, taskCount, threads) => {
      if (threads <= 1 || taskCount <= 1 || helperCount === 0) {
        for (let t = 0; t < taskCount; t++) {
          if (phase === 1) ex.wasm_mt_work_a(t);
          else ex.wasm_mt_work_b(t);
        }
        return;
      }
      const activeHelpers = Math.min(helperCount, threads - 1, taskCount - 1);
      Atomics.store(ctrl, 1, phase);
      Atomics.store(ctrl, 2, 0);
      Atomics.store(ctrl, 3, taskCount);
      Atomics.store(ctrl, 4, activeHelpers);
      Atomics.store(ctrl, 5, taskCount);
      Atomics.add(ctrl, 0, 1);
      Atomics.notify(ctrl, 0, activeHelpers);

      while (true) {
        const task = Atomics.add(ctrl, 2, 1);
        if (task >= taskCount) break;
        if (phase === 1) ex.wasm_mt_work_a(task);
        else ex.wasm_mt_work_b(task);
        if (Atomics.sub(ctrl, 5, 1) === 1) {
          Atomics.notify(ctrl, 5, 1);
        }
      }
      while (true) {
        const rem = Atomics.load(ctrl, 5);
        if (rem <= 0) break;
        Atomics.wait(ctrl, 5, rem, 5);
      }
    };

    parentPort.postMessage({ ready: true, memory });

    parentPort.on("message", async (msg) => {
      if (msg.op === "shutdown") {
        Atomics.store(ctrl, 0, -1);
        Atomics.notify(ctrl, 0, helperCount);
        await Promise.all(helpers.map((w) => w.terminate()));
        parentPort.postMessage({ done: true });
      } else if (msg.op === "alloc") {
        const ptr = ex.wasm_alloc(msg.len, 16);
        parentPort.postMessage({ ptr });
      } else if (msg.op === "open") {
        const { ptr, len, threads } = msg;
        const t0 = performance.now();
        let elems = 0;
        if (threads <= 1) {
          elems = ex.wasm_open_1t(ptr, len);
        } else {
          const info = ex.wasm_mt_begin(ptr, len, threads);
          const kind = info >>> 16;
          const countA = info & 0xffff;
          dispatchPhase(1, countA, threads);
          if (kind === 2) {
            const countB = ex.wasm_mt_dxf_stage_b(threads);
            dispatchPhase(2, countB, threads);
          }
          elems = ex.wasm_mt_finish();
        }
        const coreMs = performance.now() - t0;
        ex.wasm_free(ptr, len, 16);
        const dispBytes = ex.wasm_display_bytes();
        parentPort.postMessage({ elems, coreMs, dispBytes });
      } else if (msg.op === "move1") {
        const spans = ex.wasm_move_1(msg.step, 0);
        const tablePtr = ex.wasm_dirty_table_ptr();
        const dBytes = ex.wasm_dirty_bytes();
        parentPort.postMessage({ spans, tablePtr, dBytes });
      } else if (msg.op === "move100") {
        const spans = ex.wasm_move_100(msg.step, 0);
        const tablePtr = ex.wasm_dirty_table_ptr();
        parentPort.postMessage({ spans, tablePtr });
      } else if (msg.op === "add1") {
        const spans = ex.wasm_add_1(msg.step, 0);
        const tablePtr = ex.wasm_dirty_table_ptr();
        parentPort.postMessage({ spans, tablePtr });
      } else if (msg.op === "undo") {
        const spans = ex.wasm_undo(0);
        const tablePtr = ex.wasm_dirty_table_ptr();
        parentPort.postMessage({ spans, tablePtr });
      } else if (msg.op === "select") {
        const count = ex.wasm_select(msg.step);
        parentPort.postMessage({ count });
      }
    });
  }
} else {
  const [filePath, wasm1tPath, wasmMtPath, runsStr, threadsStr] = process.argv.slice(2);
  const runs = Number(runsStr) || 5;
  const threads = Number(threadsStr) || 12;
  const rawBytes = fs.readFileSync(filePath);
  const result = {};

  // 1. 単一スレッド wasm Worker（postMessage）
  if (wasm1tPath && fs.existsSync(wasm1tPath)) {
    const w = new Worker(SELF_PATH, { workerData: { mode: "1t-worker", wasmPath: wasm1tPath } });
    w.on("error", (err) => { console.error(err); process.exit(1); });
    const call = (msg, transfer = []) =>
      new Promise((res) => {
        w.once("message", res);
        w.postMessage(msg, transfer);
      });

    // ウォームアップ
    {
      const copy = rawBytes.buffer.slice(rawBytes.byteOffset, rawBytes.byteOffset + rawBytes.byteLength);
      await call({ op: "open", ab: copy, withDisplay: true }, [copy]);
    }

    const openCore = [];
    const openRoundtrip = [];
    let mirror = new Uint8Array(1024 * 1024);
    for (let r = 0; r < runs; r++) {
      const copy = rawBytes.buffer.slice(rawBytes.byteOffset, rawBytes.byteOffset + rawBytes.byteLength);
      const t0 = performance.now();
      const rep = await call({ op: "open", ab: copy, withDisplay: true }, [copy]);
      openRoundtrip.push(performance.now() - t0);
      openCore.push(rep.coreMs);
      if (rep.initAb && mirror.byteLength < rep.initAb.byteLength) {
        mirror = new Uint8Array(rep.initAb.byteLength + 65536);
      }
    }

    const applyPacked = (ab) => {
      const dv = new DataView(ab);
      const u8 = new Uint8Array(ab);
      let off = 0;
      while (off + 12 <= ab.byteLength) {
        const _col = dv.getUint32(off, true);
        const dstOff = dv.getUint32(off + 4, true);
        const bLen = dv.getUint32(off + 8, true);
        off += 12;
        if (dstOff + bLen <= mirror.byteLength) {
          mirror.set(u8.subarray(off, off + bLen), dstOff);
        }
        off += bLen;
      }
    };

    const move1Us = [];
    const move1Bytes = [];
    for (let s = 0; s < 60; s++) {
      const t0 = performance.now();
      const rep = await call({ op: "move1", step: s });
      applyPacked(rep.ab);
      move1Us.push((performance.now() - t0) * 1000);
      move1Bytes.push(rep.dBytes);
    }

    const move1FullMs = [];
    for (let s = 0; s < 7; s++) {
      const t0 = performance.now();
      const rep = await call({ op: "move1Full", step: s });
      if (rep.ab.byteLength <= mirror.byteLength) {
        mirror.set(new Uint8Array(rep.ab), 0);
      }
      move1FullMs.push(performance.now() - t0);
    }

    const move100Us = [];
    for (let s = 0; s < 60; s++) {
      const t0 = performance.now();
      const rep = await call({ op: "move100", step: s });
      applyPacked(rep.ab);
      move100Us.push((performance.now() - t0) * 1000);
    }

    const add1Us = [];
    const undoUs = [];
    for (let s = 0; s < 30; s++) {
      const t0 = performance.now();
      const r1 = await call({ op: "add1", step: s });
      applyPacked(r1.ab);
      add1Us.push((performance.now() - t0) * 1000);

      const t1 = performance.now();
      const r2 = await call({ op: "undo" });
      applyPacked(r2.ab);
      undoUs.push((performance.now() - t1) * 1000);
    }

    const selectUs = [];
    for (let s = 0; s < 60; s++) {
      const t0 = performance.now();
      await call({ op: "select", step: s });
      selectUs.push((performance.now() - t0) * 1000);
    }

    await w.terminate();
    result.wasm1t = {
      openCore: stats(openCore),
      openRoundtrip: stats(openRoundtrip),
      move1Us: stats(move1Us),
      move1Bytes: stats(move1Bytes).median,
      move1FullMs: stats(move1FullMs),
      move100Us: stats(move100Us),
      add1Us: stats(add1Us),
      undoUs: stats(undoUs),
      selectUs: stats(selectUs),
    };
  }

  // 2. スレッド付き共有メモリ wasm（SharedArrayBuffer + atomics）
  if (wasmMtPath && fs.existsSync(wasmMtPath)) {
    const lead = new Worker(SELF_PATH, {
      workerData: { mode: "mt-lead", wasmMtPath, maxThreads: threads },
    });
    lead.on("error", (err) => { console.error(err); process.exit(1); });
    const initMsg = await new Promise((res) => lead.once("message", res));
    const sharedMemory = initMsg.memory;
    const callLead = (msg) =>
      new Promise((res) => {
        lead.once("message", res);
        lead.postMessage(msg);
      });

    const runOpenBench = async (kThreads) => {
      // ウォームアップ
      {
        const { ptr } = await callLead({ op: "alloc", len: rawBytes.byteLength });
        new Uint8Array(sharedMemory.buffer, ptr, rawBytes.byteLength).set(rawBytes);
        await callLead({ op: "open", ptr, len: rawBytes.byteLength, threads: kThreads });
      }
      const coreArr = [];
      const totalArr = [];
      let dispBytes = 0;
      for (let r = 0; r < runs; r++) {
        const t0 = performance.now();
        const { ptr } = await callLead({ op: "alloc", len: rawBytes.byteLength });
        new Uint8Array(sharedMemory.buffer, ptr, rawBytes.byteLength).set(rawBytes);
        const rep = await callLead({ op: "open", ptr, len: rawBytes.byteLength, threads: kThreads });
        totalArr.push(performance.now() - t0);
        coreArr.push(rep.coreMs);
        dispBytes = rep.dispBytes;
      }
      return { core: stats(coreArr), total: stats(totalArr), dispBytes };
    };

    const open1 = await runOpenBench(1);
    const openMt = await runOpenBench(threads);

    let mirror = new Uint8Array(Math.max(openMt.dispBytes + 65536, 1024 * 1024));
    const syncZeroCopy = (spans, tablePtr) => {
      const buf = sharedMemory.buffer;
      const table = new Uint32Array(buf, tablePtr, spans * 4);
      for (let i = 0; i < spans; i++) {
        const dstOff = table[i * 4 + 1];
        const slicePtr = table[i * 4 + 2];
        const bLen = table[i * 4 + 3];
        if (dstOff + bLen <= mirror.byteLength) {
          mirror.set(new Uint8Array(buf, slicePtr, bLen), dstOff);
        }
      }
    };

    const move1Us = [];
    const move1Bytes = [];
    for (let s = 0; s < 60; s++) {
      const t0 = performance.now();
      const rep = await callLead({ op: "move1", step: s });
      syncZeroCopy(rep.spans, rep.tablePtr);
      move1Us.push((performance.now() - t0) * 1000);
      move1Bytes.push(rep.dBytes);
    }

    const move100Us = [];
    for (let s = 0; s < 60; s++) {
      const t0 = performance.now();
      const rep = await callLead({ op: "move100", step: s });
      syncZeroCopy(rep.spans, rep.tablePtr);
      move100Us.push((performance.now() - t0) * 1000);
    }

    const add1Us = [];
    const undoUs = [];
    for (let s = 0; s < 30; s++) {
      const t0 = performance.now();
      const r1 = await callLead({ op: "add1", step: s });
      syncZeroCopy(r1.spans, r1.tablePtr);
      add1Us.push((performance.now() - t0) * 1000);

      const t1 = performance.now();
      const r2 = await callLead({ op: "undo" });
      syncZeroCopy(r2.spans, r2.tablePtr);
      undoUs.push((performance.now() - t1) * 1000);
    }

    const selectUs = [];
    for (let s = 0; s < 60; s++) {
      const t0 = performance.now();
      await callLead({ op: "select", step: s });
      selectUs.push((performance.now() - t0) * 1000);
    }

    await callLead({ op: "shutdown" });
    await lead.terminate();

    result.wasmMt = {
      open1Core: open1.core,
      open1Total: open1.total,
      openMtCore: openMt.core,
      openMtTotal: openMt.total,
      move1Us: stats(move1Us),
      move1Bytes: stats(move1Bytes).median,
      move100Us: stats(move100Us),
      add1Us: stats(add1Us),
      undoUs: stats(undoUs),
      selectUs: stats(selectUs),
    };
  }

  process.stdout.write(JSON.stringify(result));
}
"#;

fn find_wasm_paths() -> (PathBuf, PathBuf) {
    let p1 = PathBuf::from("target/wasm32-unknown-unknown/release/zumen_bench_live.wasm");
    let p2 = PathBuf::from("target/wasm-mt/wasm32-unknown-unknown/release/zumen_bench_live.wasm");
    (p1, p2)
}

fn measure_wasm(path: &str, options: &Options) -> Result<(), String> {
    let (wasm_1t, wasm_mt) = find_wasm_paths();
    if !wasm_1t.exists() && !wasm_mt.exists() {
        println!(
            "  [3/4] wasm バイナリが未生成のためスキップ（{} / {}）",
            wasm_1t.display(),
            wasm_mt.display()
        );
        return Ok(());
    }

    let script_path = std::env::temp_dir().join("zumen-live-place-wasm.mjs");
    std::fs::write(&script_path, NODE_WASM_RUNNER)
        .map_err(|e| format!("一時スクリプトを書けない: {e}"))?;

    let output = Command::new("node")
        .arg(&script_path)
        .arg(path)
        .arg(&wasm_1t)
        .arg(&wasm_mt)
        .arg(options.runs.to_string())
        .arg(options.threads.to_string())
        .output()
        .map_err(|e| format!("node を起動できない: {e}"))?;
    let _ = std::fs::remove_file(&script_path);

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("node wasm 測定失敗: {stderr}"));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed =
        zumen::json::parse(stdout.as_bytes()).map_err(|_| format!("JSON を読めない: {stdout}"))?;
    let num = |ptr: &str| -> f64 {
        match parsed.pointer(ptr) {
            Some(zumen::json::Value::Number(n)) => n.to_f64().unwrap_or(f64::NAN),
            _ => f64::NAN,
        }
    };

    if parsed.pointer("/wasm1t").is_some() {
        println!("  [3] wasm（単一スレッド Worker ＋ postMessage 差分転送）");
        println!(
            "    開く: Worker 内 1T {:.2} ms（入力 ArrayBuffer 転送 ＋ 初回表示バッファー返送込み: {:.2} ms）",
            num("/wasm1t/openCore/median"),
            num("/wasm1t/openRoundtrip/median"),
        );
        println!(
            "    操作（Worker 往復 ＋ postMessage 差分コピー）: 1 件移動 {:.1} µs（99% {:.1} µs、差分 {:.0} B）、100 件移動 {:.1} µs、1 件追加 {:.1} µs、取り消し {:.1} µs、範囲選択 {:.1} µs",
            num("/wasm1t/move1Us/median"),
            num("/wasm1t/move1Us/p99"),
            num("/wasm1t/move1Bytes"),
            num("/wasm1t/move100Us/median"),
            num("/wasm1t/add1Us/median"),
            num("/wasm1t/undoUs/median"),
            num("/wasm1t/selectUs/median"),
        );
        println!(
            "    （比較）1 件移動で表示バッファー全体を postMessage 転送した場合: {:.2} ms",
            num("/wasm1t/move1FullMs/median"),
        );
    }

    if parsed.pointer("/wasmMt").is_some() {
        println!("  [4] スレッド付き wasm（SharedArrayBuffer ＋ atomics、ゼロコピー差分参照）");
        println!(
            "    開く: 1T {:.2} ms（入力コピー込み {:.2} ms） ／ {}T {:.2} ms（入力コピー込み {:.2} ms）",
            num("/wasmMt/open1Core/median"),
            num("/wasmMt/open1Total/median"),
            options.threads,
            num("/wasmMt/openMtCore/median"),
            num("/wasmMt/openMtTotal/median"),
        );
        println!(
            "    操作（Worker 通知 ＋ 共有メモリ直接参照）: 1 件移動 {:.1} µs（99% {:.1} µs、差分 {:.0} B）、100 件移動 {:.1} µs、1 件追加 {:.1} µs、取り消し {:.1} µs、範囲選択 {:.1} µs",
            num("/wasmMt/move1Us/median"),
            num("/wasmMt/move1Us/p99"),
            num("/wasmMt/move1Bytes"),
            num("/wasmMt/move100Us/median"),
            num("/wasmMt/add1Us/median"),
            num("/wasmMt/undoUs/median"),
            num("/wasmMt/selectUs/median"),
        );
    }
    Ok(())
}

/// 指定した図面列について、文書エンジンを動かす場所ごとの時間を測る
pub fn run(paths: &[String], options: &Options) -> Result<(), String> {
    for path in paths {
        measure_in_process(path, options)?;
        measure_ipc(path, options)?;
        measure_wasm(path, options)?;
    }
    Ok(())
}
