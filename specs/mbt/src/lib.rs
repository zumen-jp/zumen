//! Quint 仕様（`specs/*.qnt`）の実行トレースを `zumen` の実装へリプレイするモデルベーステスト（MBT）共通部品。

use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Limits};
use zumen_live::display_wire::{self, Column};

/// モデル空間に 1 本の線分 `(0, 0) -> (10, 0)` を持つ最小の ASCII DXF（R2000）。
pub const MINIMAL_LINE_DXF: &[u8] = b"\
  0\nSECTION\n  2\nHEADER\n  9\n$ACADVER\n  1\nAC1015\n  9\n$HANDSEED\n  5\n100\n  0\nENDSEC\n\
  0\nSECTION\n  2\nTABLES\n  0\nTABLE\n  2\nLAYER\n 70\n1\n\
  0\nLAYER\n  5\n10\n100\nAcDbSymbolTableRecord\n100\nAcDbLayerTableRecord\n  2\n0\n 70\n0\n 62\n7\n  6\nContinuous\n\
  0\nENDTAB\n  0\nENDSEC\n\
  0\nSECTION\n  2\nENTITIES\n\
  0\nLINE\n  5\n20\n100\nAcDbEntity\n  8\n0\n100\nAcDbLine\n 10\n0.0\n 20\n0.0\n 30\n0.0\n 11\n10.0\n 21\n0.0\n 31\n0.0\n\
  0\nENDSEC\n  0\nEOF\n";

/// ZLIP/ZLIR 通信の定数（`bindings/cli/live.md`）。
pub mod wire {
    pub const CMD_OPEN: u16 = 1;
    pub const CMD_DISPLAY: u16 = 2;
    pub const CMD_TRANSFORM: u16 = 3;
    pub const CMD_UNDO: u16 = 4;
    pub const CMD_REDO: u16 = 5;
    pub const CMD_DIAGNOSTICS: u16 = 8;
    pub const CMD_CLOSE: u16 = 9;
    pub const CMD_SET_TEXT: u16 = 18;

    pub const FLAG_WITH_DISPLAY: u32 = 1;
    pub const FLAG_FULL_DISPLAY: u32 = 2;
    pub const NONE: u64 = u64::MAX;

    pub const STATUS_OK: u32 = 0;
    pub const STATUS_INVALID: u32 = 1;
    pub const STATUS_UNSUPPORTED: u32 = 2;
    pub const STATUS_STALE: u32 = 3;
    pub const STATUS_MISSING: u32 = 4;
    pub const STATUS_REJECTED: u32 = 5;
    pub const STATUS_EXCEEDED: u32 = 6;
    pub const STATUS_CANCELLED: u32 = 7;
    pub const STATUS_TIMED_OUT: u32 = 8;
    pub const STATUS_NO_DOCUMENT: u32 = 9;
    pub const STATUS_ALREADY_OPEN: u32 = 10;
    pub const STATUS_NO_DIAGNOSTICS: u32 = 11;

    pub fn status_name(code: u32) -> &'static str {
        match code {
            STATUS_OK => "ok",
            STATUS_INVALID => "invalid",
            STATUS_UNSUPPORTED => "unsupported",
            STATUS_STALE => "stale",
            STATUS_MISSING => "missing",
            STATUS_REJECTED => "rejected",
            STATUS_EXCEEDED => "exceeded",
            STATUS_CANCELLED => "cancelled",
            STATUS_TIMED_OUT => "timed_out",
            STATUS_NO_DOCUMENT => "no_document",
            STATUS_ALREADY_OPEN => "already_open",
            STATUS_NO_DIAGNOSTICS => "no_diagnostics",
            _ => "unknown",
        }
    }
}

/// ZLIP 応答の解析結果。
#[derive(Debug, Clone)]
pub struct LiveReply {
    pub status: u32,
    pub display_status: u32,
    pub revision: u64,
    pub state: u64,
    pub result: u64,
    pub base: u64,
    pub data_kind: u32,
    pub diagnostics: u32,
    pub data: Vec<u8>,
}

/// ZLDP の 13 列を保持するクライアント側ミラー（線分列の座標を中心に保持）。
#[derive(Debug, Default, Clone)]
pub struct DisplayMirror {
    pub revision: i64,
    /// 各列の要素数（13 列）。
    pub rows: [u32; 13],
    /// 列 0（線分の座標: `f32 × 8` = 32 バイト/要素）の生バイト列。
    pub segments: Vec<u8>,
}

impl DisplayMirror {
    pub fn new() -> Self {
        Self {
            revision: -1,
            rows: [0; 13],
            segments: Vec::new(),
        }
    }

    /// 受信した ZLDP パケットを検証し、契約どおり反映できる場合だけ更新する。
    pub fn apply_packet(&mut self, packet_bytes: &[u8], revision: u64, base: u64) -> bool {
        let mut budget = Budget::new(Limits::default());
        let mut diag = Diagnostics::new();
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        let packet = display_wire::Packet::parse(packet_bytes, &mut cx)
            .expect("サーバーが返した ZLDP パケットは常に契約を満たす");

        let is_full = packet.full();
        if !is_full && (base == wire::NONE || self.revision < 0 || self.revision as u64 != base) {
            return false;
        }
        for (i, col) in packet.columns().iter().enumerate() {
            self.rows[i] = col.rows();
            if col.kind() == Column::Segments {
                let target_bytes = (col.rows() as usize) * (col.stride() as usize);
                if is_full {
                    self.segments.clear();
                }
                self.segments.resize(target_bytes, 0);
                for block in col.blocks() {
                    let offset = (block.start as usize) * (col.stride() as usize);
                    let end = offset + block.bytes.len();
                    self.segments[offset..end].copy_from_slice(block.bytes);
                }
            }
        }
        self.revision = revision as i64;
        true
    }

    /// 先頭線分の始点 X 座標（四捨五入した整数値）。線分がなければ 0。
    pub fn first_line_x0(&self) -> i64 {
        if self.segments.len() < 4 {
            return 0;
        }
        let x0 = f32::from_le_bytes([
            self.segments[0],
            self.segments[1],
            self.segments[2],
            self.segments[3],
        ]);
        x0.round() as i64
    }
}

/// `zumen live --stdio` 子プロセスと ZLIP/ZLIR で通信するクライアント。
pub struct LiveClient {
    child: Child,
    stdin: ChildStdin,
    stdout: ChildStdout,
    next_id: u64,
}

impl LiveClient {
    pub fn spawn() -> Self {
        let bin = resolve_zumen_bin();
        let mut child = Command::new(&bin)
            .args(["live", "--stdio"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap_or_else(|e| panic!("zumen 実行ファイル ({}) の起動に失敗: {e}", bin.display()));
        let stdin = child.stdin.take().expect("stdin pipe");
        let stdout = child.stdout.take().expect("stdout pipe");
        Self {
            child,
            stdin,
            stdout,
            next_id: 1,
        }
    }

    pub fn last_req_id(&self) -> u64 {
        self.next_id - 1
    }

    pub fn send(
        &mut self,
        command: u16,
        flags: u32,
        limits: [u64; 7],
        revision: u64,
        ack: u64,
        payload: &[u8],
    ) -> LiveReply {
        let id = self.next_id;
        self.next_id += 1;

        let mut body = Vec::new();
        if command != wire::CMD_CLOSE {
            for lim in limits {
                body.extend_from_slice(&lim.to_le_bytes());
            }
            body.extend_from_slice(&revision.to_le_bytes());
            body.extend_from_slice(&ack.to_le_bytes());
            body.extend_from_slice(payload);
        }

        let mut header = [0u8; 24];
        header[0..4].copy_from_slice(b"ZLIP");
        header[4..6].copy_from_slice(&1u16.to_le_bytes());
        header[6..8].copy_from_slice(&command.to_le_bytes());
        header[8..16].copy_from_slice(&id.to_le_bytes());
        header[16..20].copy_from_slice(&(body.len() as u32).to_le_bytes());
        header[20..24].copy_from_slice(&flags.to_le_bytes());

        self.stdin.write_all(&header).expect("write header");
        self.stdin.write_all(&body).expect("write body");
        self.stdin.flush().expect("flush");

        let mut resp_hdr = [0u8; 24];
        self.stdout
            .read_exact(&mut resp_hdr)
            .expect("read reply header");
        assert_eq!(&resp_hdr[0..4], b"ZLIR");
        assert_eq!(u16::from_le_bytes([resp_hdr[4], resp_hdr[5]]), 1);
        assert_eq!(u16::from_le_bytes([resp_hdr[6], resp_hdr[7]]), command);
        assert_eq!(u64::from_le_bytes(resp_hdr[8..16].try_into().unwrap()), id);
        let resp_len = u32::from_le_bytes(resp_hdr[16..20].try_into().unwrap()) as usize;
        assert!(resp_len >= 136);
        let mut resp_body = vec![0u8; resp_len];
        self.stdout
            .read_exact(&mut resp_body)
            .expect("read reply body");

        let u32_at = |off: usize| u32::from_le_bytes(resp_body[off..off + 4].try_into().unwrap());
        let u64_at = |off: usize| u64::from_le_bytes(resp_body[off..off + 8].try_into().unwrap());

        let data_len = u32_at(48) as usize;
        assert_eq!(resp_len, 136 + data_len);

        LiveReply {
            status: u32_at(0),
            display_status: u32_at(4),
            revision: u64_at(8),
            state: u64_at(16),
            result: u64_at(24),
            base: u64_at(32),
            data_kind: u32_at(40),
            diagnostics: u32_at(44),
            data: resp_body[136..].to_vec(),
        }
    }
}

impl Drop for LiveClient {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// 既定の 7 種予算（`u64::MAX` は上限なし）。
pub fn unlimited_limits() -> [u64; 7] {
    [wire::NONE; 7]
}

#[allow(clippy::disallowed_methods)]
fn resolve_zumen_bin() -> PathBuf {
    if let Ok(path) = std::env::var("ZUMEN_BIN") {
        return PathBuf::from(path);
    }
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root");
    let exe = if cfg!(windows) { "zumen.exe" } else { "zumen" };
    let candidate = root.join("target").join("debug").join(exe);
    if candidate.is_file() {
        return candidate;
    }
    panic!(
        "zumen 実行ファイルが見つかりません。先に `cargo build -p zumen-cli` を実行するか環境変数 ZUMEN_BIN を指定してください（探索先: {}）",
        candidate.display()
    );
}
