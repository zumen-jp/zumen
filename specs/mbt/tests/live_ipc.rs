//! `specs/live_ipc.qnt` のトレースを実プロセス `zumen live --stdio` へリプレイする E2E MBT。

use quint_connect::*;
use serde::Deserialize;
use zumen_specs_mbt::{DisplayMirror, LiveClient, MINIMAL_LINE_DXF, unlimited_limits, wire};

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct LiveIpcState {
    opened: bool,
    doc_rev: i64,
    client_rev: i64,
    client_value: i64,
    req_id: i64,
    stored_diag_req_id: i64,
    last_status: String,
    last_display_status: String,
}

struct LiveIpcDriver {
    client: Option<LiveClient>,
    mirror: DisplayMirror,
    opened: bool,
    doc_rev: i64,
    stored_diag_req_id: i64,
    last_status: String,
    last_display_status: String,
}

impl Default for LiveIpcDriver {
    fn default() -> Self {
        Self {
            client: None,
            mirror: DisplayMirror::new(),
            opened: false,
            doc_rev: -1,
            stored_diag_req_id: -1,
            last_status: "ok".to_owned(),
            last_display_status: "ok".to_owned(),
        }
    }
}

impl LiveIpcDriver {
    fn client_mut(&mut self) -> &mut LiveClient {
        self.client.as_mut().expect("init で子プロセスを起動済み")
    }

    fn resolve_ack(&self, mode: i64) -> u64 {
        match mode {
            1 if self.mirror.revision >= 0 => self.mirror.revision as u64,
            2 => 0,
            _ => wire::NONE,
        }
    }

    fn init(&mut self) {
        self.client = Some(LiveClient::spawn());
        self.mirror = DisplayMirror::new();
        self.opened = false;
        self.doc_rev = -1;
        self.stored_diag_req_id = -1;
        self.last_status = "ok".to_owned();
        self.last_display_status = "ok".to_owned();
    }

    fn record_normal_reply(&mut self, reply: &zumen_specs_mbt::LiveReply) {
        self.last_status = wire::status_name(reply.status).to_owned();
        self.last_display_status = wire::status_name(reply.display_status).to_owned();
        if reply.revision != wire::NONE {
            self.doc_rev = reply.revision as i64;
        }
        self.stored_diag_req_id = self.client.as_ref().unwrap().last_req_id() as i64;
        if reply.data_kind == 1 && reply.display_status == wire::STATUS_OK {
            let _ = self
                .mirror
                .apply_packet(&reply.data, reply.revision, reply.base);
        }
    }

    fn open_doc(&mut self) {
        let mut payload = Vec::with_capacity(8 + MINIMAL_LINE_DXF.len());
        payload.extend_from_slice(&1u32.to_le_bytes()); // threads = 1
        payload.extend_from_slice(&0u32.to_le_bytes()); // saf = なし
        payload.extend_from_slice(MINIMAL_LINE_DXF);
        let reply = self.client_mut().send(
            wire::CMD_OPEN,
            0,
            unlimited_limits(),
            wire::NONE,
            wire::NONE,
            &payload,
        );
        if reply.status == wire::STATUS_OK {
            self.opened = true;
        }
        self.record_normal_reply(&reply);
    }

    fn op_before_open(&mut self) {
        let reply = self.client_mut().send(
            wire::CMD_DISPLAY,
            wire::FLAG_WITH_DISPLAY,
            unlimited_limits(),
            0,
            wire::NONE,
            &[],
        );
        self.record_normal_reply(&reply);
    }

    fn transform_payload(delta: i64) -> Vec<u8> {
        let mut payload = Vec::with_capacity(60);
        payload.extend_from_slice(&1u32.to_le_bytes()); // count = 1
        payload.extend_from_slice(&0u32.to_le_bytes()); // reserved = 0
        // 平行移動 [1, 0, delta, 0, 1, 0]
        for v in [1.0_f64, 0.0, delta as f64, 0.0, 1.0, 0.0] {
            payload.extend_from_slice(&v.to_le_bytes());
        }
        payload.extend_from_slice(&0u32.to_le_bytes()); // RowId(0)
        payload
    }

    fn edit_ok_with_display(&mut self, ack_mode: i64, full_flag: bool, delta: i64) {
        let ack = self.resolve_ack(ack_mode);
        let flags = wire::FLAG_WITH_DISPLAY
            | if full_flag {
                wire::FLAG_FULL_DISPLAY
            } else {
                0
            };
        let payload = Self::transform_payload(delta);
        let rev = self.doc_rev as u64;
        let reply = self.client_mut().send(
            wire::CMD_TRANSFORM,
            flags,
            unlimited_limits(),
            rev,
            ack,
            &payload,
        );
        self.record_normal_reply(&reply);
    }

    fn edit_ok_display_exceeded(&mut self, ack_mode: i64, delta: i64) {
        let ack = self.resolve_ack(ack_mode);
        let payload = Self::transform_payload(delta);
        let rev = self.doc_rev as u64;
        // Transform 自体は Records を消費しない一方、直後の ZLDP 符号化がブロック 1 件で
        // Records(1) を消費するため、Records = 0 で「編集確定 ok＋表示取得 exceeded」を決定的に再現する。
        let mut limits = unlimited_limits();
        limits[2] = 0; // Kind::Records = 0
        let reply = self.client_mut().send(
            wire::CMD_TRANSFORM,
            wire::FLAG_WITH_DISPLAY | wire::FLAG_FULL_DISPLAY,
            limits,
            rev,
            ack,
            &payload,
        );
        self.record_normal_reply(&reply);
    }

    fn edit_rejected(&mut self, ack_mode: i64) {
        let ack = self.resolve_ack(ack_mode);
        let rev = self.doc_rev as u64;
        // 線分（RowId(0)）に対する SetText は文字でない行として必ず Status::Rejected になる。
        let mut payload = Vec::new();
        payload.extend_from_slice(&1u32.to_le_bytes()); // count = 1
        payload.extend_from_slice(&1u32.to_le_bytes()); // text len = 1
        payload.extend_from_slice(b"x\0\0\0"); // 4 バイト境界まで 0 埋め
        payload.extend_from_slice(&0u32.to_le_bytes()); // RowId(0)
        let reply = self.client_mut().send(
            wire::CMD_SET_TEXT,
            0,
            unlimited_limits(),
            rev,
            ack,
            &payload,
        );
        self.record_normal_reply(&reply);
    }

    fn edit_stale(&mut self) {
        let stale_rev = (self.doc_rev + 1) as u64;
        let payload = Self::transform_payload(1);
        let reply = self.client_mut().send(
            wire::CMD_TRANSFORM,
            wire::FLAG_WITH_DISPLAY,
            unlimited_limits(),
            stale_rev,
            wire::NONE,
            &payload,
        );
        self.record_normal_reply(&reply);
    }

    fn undo_with_display(&mut self, ack_mode: i64, full_flag: bool) {
        let ack = self.resolve_ack(ack_mode);
        let flags = wire::FLAG_WITH_DISPLAY
            | if full_flag {
                wire::FLAG_FULL_DISPLAY
            } else {
                0
            };
        let rev = self.doc_rev as u64;
        let reply =
            self.client_mut()
                .send(wire::CMD_UNDO, flags, unlimited_limits(), rev, ack, &[]);
        self.record_normal_reply(&reply);
    }

    fn redo_with_display(&mut self, ack_mode: i64, full_flag: bool) {
        let ack = self.resolve_ack(ack_mode);
        let flags = wire::FLAG_WITH_DISPLAY
            | if full_flag {
                wire::FLAG_FULL_DISPLAY
            } else {
                0
            };
        let rev = self.doc_rev as u64;
        let reply =
            self.client_mut()
                .send(wire::CMD_REDO, flags, unlimited_limits(), rev, ack, &[]);
        self.record_normal_reply(&reply);
    }

    fn display_sync(&mut self, ack_mode: i64, full_flag: bool) {
        let ack = self.resolve_ack(ack_mode);
        let flags = wire::FLAG_WITH_DISPLAY
            | if full_flag {
                wire::FLAG_FULL_DISPLAY
            } else {
                0
            };
        let rev = self.doc_rev as u64;
        let reply =
            self.client_mut()
                .send(wire::CMD_DISPLAY, flags, unlimited_limits(), rev, ack, &[]);
        self.record_normal_reply(&reply);
    }

    fn diagnostics_ok(&mut self) {
        let target = self.stored_diag_req_id as u64;
        let reply = self.client_mut().send(
            wire::CMD_DIAGNOSTICS,
            0,
            unlimited_limits(),
            wire::NONE,
            wire::NONE,
            &target.to_le_bytes(),
        );
        self.last_status = wire::status_name(reply.status).to_owned();
        self.last_display_status = wire::status_name(reply.display_status).to_owned();
        // Diagnostics は stored_diag_req_id を書き換えない
    }

    fn diagnostics_exceeded(&mut self) {
        let target = self.stored_diag_req_id as u64;
        let mut limits = unlimited_limits();
        limits[6] = 0; // Kind::Work = 0（入口チェックで確実に Exceeded）
        let reply = self.client_mut().send(
            wire::CMD_DIAGNOSTICS,
            0,
            limits,
            wire::NONE,
            wire::NONE,
            &target.to_le_bytes(),
        );
        self.last_status = wire::status_name(reply.status).to_owned();
        self.last_display_status = wire::status_name(reply.display_status).to_owned();
    }

    fn diagnostics_mismatch(&mut self) {
        let bogus = 999_999_u64;
        let reply = self.client_mut().send(
            wire::CMD_DIAGNOSTICS,
            0,
            unlimited_limits(),
            wire::NONE,
            wire::NONE,
            &bogus.to_le_bytes(),
        );
        self.last_status = wire::status_name(reply.status).to_owned();
        self.last_display_status = wire::status_name(reply.display_status).to_owned();
    }
}

impl State<LiveIpcDriver> for LiveIpcState {
    fn from_driver(driver: &LiveIpcDriver) -> Result<Self> {
        let req_id = driver.client.as_ref().map_or(0, |c| c.last_req_id() as i64);
        Ok(Self {
            opened: driver.opened,
            doc_rev: driver.doc_rev,
            client_rev: driver.mirror.revision,
            client_value: driver.mirror.first_line_x0(),
            req_id,
            stored_diag_req_id: driver.stored_diag_req_id,
            last_status: driver.last_status.clone(),
            last_display_status: driver.last_display_status.clone(),
        })
    }
}

impl Driver for LiveIpcDriver {
    type State = LiveIpcState;

    fn step(&mut self, step: &Step) -> Result {
        switch!(step {
            init => self.init(),
            open_doc => self.open_doc(),
            op_before_open => self.op_before_open(),
            edit_ok_with_display(ack_mode, full_flag, delta) => {
                self.edit_ok_with_display(ack_mode, full_flag, delta);
            }
            edit_ok_display_exceeded(ack_mode, delta) => {
                self.edit_ok_display_exceeded(ack_mode, delta);
            }
            edit_rejected(ack_mode) => self.edit_rejected(ack_mode),
            edit_stale => self.edit_stale(),
            undo_with_display(ack_mode, full_flag) => {
                self.undo_with_display(ack_mode, full_flag);
            }
            redo_with_display(ack_mode, full_flag) => {
                self.redo_with_display(ack_mode, full_flag);
            }
            display_sync(ack_mode, full_flag) => self.display_sync(ack_mode, full_flag),
            diagnostics_ok => self.diagnostics_ok(),
            diagnostics_exceeded => self.diagnostics_exceeded(),
            diagnostics_mismatch => self.diagnostics_mismatch(),
        })
    }
}

#[quint_run(
    spec = "../live_ipc.qnt",
    max_samples = 20,
    max_steps = 25,
    seed = "20261124"
)]
fn mbt_live_ipc_random_traces() -> impl Driver {
    LiveIpcDriver::default()
}

#[quint_run(
    spec = "../live_ipc.qnt",
    max_samples = 15,
    max_steps = 40,
    seed = "20261125"
)]
fn mbt_live_ipc_deep_traces() -> impl Driver {
    LiveIpcDriver::default()
}
