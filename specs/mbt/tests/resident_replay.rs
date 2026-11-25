//! `specs/resident_replay.qnt` のトレースを `zumen-base::Cx`（record/replay）と
//! `zumen-abi::Session`（常駐キャッシュ＋文書エンジン切替）へリプレイする MBT。

use quint_connect::*;
use serde::Deserialize;
use zumen_abi1::{Session, Status};
use zumen_base::{
    Budget, CancelToken, Cx, Diagnostic, Diagnostics, Kind, Limit, Limits, Ref, Replay, Stop, codes,
};
use zumen_json::Value;
use zumen_specs_mbt::MINIMAL_LINE_DXF;

const LIFT_WORK: u64 = 10;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct ResidentReplayState {
    doc_ver: i64,
    model_kind: String,
    file_loaded: bool,
    lifted_cached: bool,
    lifted_ver: i64,
    live_kind: String,
    live_ver: i64,
    last_used_replay: bool,
    last_ok: bool,
}

/// 決まった作業量（`LIFT_WORK = 10`）と深さ（2）を使い、診断を 2 件報告する決定的変換処理。
fn synthetic_lift(doc_ver: i64, cx: &mut Cx<'_>) -> std::result::Result<i64, Stop> {
    cx.enter()?;
    cx.enter()?;
    cx.check(Kind::Work, LIFT_WORK)?;
    cx.report(Diagnostic::new(codes::SXF_CODE_MISSING).at(Ref::at_record(1)));
    cx.report(Diagnostic::new(codes::DXF_UNITS_MISSING).at(Ref::at_record(0)));
    cx.leave();
    cx.leave();
    Ok(doc_ver)
}

fn call_json(session: &mut Session, req: &str) -> (Status, Value) {
    let cancel = CancelToken::new();
    let reply = session.call(req.as_bytes(), &cancel);
    let val = zumen_json::parse(&reply.response).expect("セッション応答は常に有効な JSON");
    (reply.status, val)
}

fn json_str<'a>(v: &'a Value, ptr: &str) -> Option<&'a str> {
    match v.pointer(ptr)? {
        Value::String(s) => Some(s.as_str()),
        _ => None,
    }
}

fn json_f64(v: &Value, ptr: &str) -> Option<f64> {
    match v.pointer(ptr)? {
        Value::Number(n) => n.to_f64(),
        _ => None,
    }
}

struct ResidentReplayDriver {
    // 1. Cx::record / Cx::replay を実運用と同じ条件で駆動するキャッシュ層
    doc_ver: i64,
    model_kind: String,
    file_loaded: bool,
    cached: Option<(i64, Replay)>,
    live_kind: String,
    live_ver: i64,
    last_used_replay: bool,
    last_ok: bool,
    // 2. 実 Session（文書エンジンと常駐図面モデルを持つ実機）
    session: Session,
    current_id: String,
    current_hash: String,
}

impl Default for ResidentReplayDriver {
    fn default() -> Self {
        let mut driver = Self {
            doc_ver: 0,
            model_kind: "Held".to_owned(),
            file_loaded: true,
            cached: None,
            live_kind: "Absent".to_owned(),
            live_ver: -1,
            last_used_replay: false,
            last_ok: true,
            session: Session::new(b"").expect("session"),
            current_id: String::new(),
            current_hash: String::new(),
        };
        driver.reset_session();
        driver
    }
}

impl ResidentReplayDriver {
    fn reset_session(&mut self) {
        self.session = Session::new(b"").expect("session");
        self.session
            .push(b"mbt.dxf", MINIMAL_LINE_DXF)
            .expect("push");
        let (status, open_res) = call_json(
            &mut self.session,
            r#"{"v":1,"op":"open","input":"mbt.dxf"}"#,
        );
        assert_eq!(status, Status::Ok, "open 失敗: {open_res:?}");
        self.refresh_target_from_entities();
    }

    fn refresh_target_from_entities(&mut self) {
        let (status, ent) = call_json(
            &mut self.session,
            r#"{"v":1,"op":"entities","doc":"mbt.dxf","fields":["hash","bbox"]}"#,
        );
        assert_eq!(status, Status::Ok, "entities 失敗: {ent:?}");
        self.current_id = json_str(&ent, "/result/items/0/id")
            .expect("線分 1 件が存在する")
            .to_owned();
        self.current_hash = json_str(&ent, "/result/items/0/hash").unwrap().to_owned();
        let x0 = json_f64(&ent, "/result/items/0/bbox/min_x")
            .unwrap_or_else(|| panic!("予期しない entities 応答: {ent:?}"));
        assert_eq!(
            x0.round() as i64,
            self.doc_ver,
            "Session の常駐読み取り座標がモデル版 doc_ver と一致する"
        );
    }

    fn init(&mut self) {
        self.doc_ver = 0;
        self.model_kind = "Held".to_owned();
        self.file_loaded = true;
        self.cached = None;
        self.live_kind = "Absent".to_owned();
        self.live_ver = -1;
        self.last_used_replay = false;
        self.last_ok = true;
        self.reset_session();
    }

    fn build_limits(work_limit: i64, depth_room: i64) -> Limits {
        let depth = if depth_room < 0 {
            Limit::Unlimited
        } else {
            Limit::Finite(depth_room as u64)
        };
        Limits::default()
            .with(Kind::Work, Limit::Finite(work_limit as u64))
            .with(Kind::Depth, depth)
    }

    fn read_plain(&mut self, work_limit: i64, depth_room: i64, max_places: i64, cancelled: bool) {
        let limits = Self::build_limits(work_limit, depth_room);
        let cancel = CancelToken::new();
        if cancelled {
            cancel.cancel();
        }
        let mp = max_places as usize;

        // 常に「直接再実行」をオラクルとして計算し、キャッシュ経路と予算・診断・成否を比較する。
        let mut oracle_budget = Budget::new(limits);
        let mut oracle_diag = Diagnostics::with_max_places(mp);
        let oracle_res = synthetic_lift(
            self.doc_ver,
            &mut Cx::new(&mut oracle_budget, &mut oracle_diag, &cancel),
        );

        let mut budget = Budget::new(limits);
        let mut diag = Diagnostics::with_max_places(mp);
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);

        if let Some((ver, ref replay)) = self.cached
            && cx.replay(replay)
        {
            assert_eq!(ver, self.doc_ver);
            assert_eq!(oracle_res, Ok(self.doc_ver));
            assert_eq!(budget, oracle_budget);
            assert_eq!(diag.into_vec(), oracle_diag.into_vec());
            self.last_used_replay = true;
            self.last_ok = true;
            self.refresh_target_from_entities();
            return;
        }

        let (res, recorded) = cx.record(|cx| synthetic_lift(self.doc_ver, cx));
        assert_eq!(res, oracle_res);
        assert_eq!(budget, oracle_budget);
        assert_eq!(diag.into_vec(), oracle_diag.into_vec());
        self.last_used_replay = false;
        self.last_ok = res.is_ok();
        if let (Ok(ver), Some(replay)) = (res, recorded) {
            self.cached = Some((ver, replay));
        }
        if self.last_ok {
            self.refresh_target_from_entities();
        }
    }

    fn read_non_plain(&mut self, work_limit: i64, depth_room: i64) {
        let limits = Self::build_limits(work_limit, depth_room);
        let cancel = CancelToken::new();
        let mut budget = Budget::new(limits);
        let mut diag = Diagnostics::new();
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        let before_cached = self.cached.as_ref().map(|(v, _)| *v);

        let res: std::result::Result<i64, Stop> = cx.with_diagnostics_in_record_order(|cx| {
            // 非 plain スコープでは replay は必ず false、record は None を返す契約
            if let Some((_, ref r)) = self.cached {
                assert!(!cx.replay(r));
            }
            let (out, rec) = cx.record(|cx| synthetic_lift(self.doc_ver, cx));
            assert!(rec.is_none());
            out
        });
        assert_eq!(self.cached.as_ref().map(|(v, _)| *v), before_cached);
        self.last_used_replay = false;
        self.last_ok = res.is_ok();
    }

    fn apply_live_ok(&mut self) {
        // 実 Session に文書エンジン経由の平行移動（X += 1）を当てる
        let req = format!(
            r#"{{"v":1,"op":"apply","doc":"mbt.dxf","edits":[{{"kind":"transform","id":"{}","hash":"{}","matrix":[1,0,1,0,1,0]}}]}}"#,
            self.current_id, self.current_hash
        );
        let (status, res) = call_json(&mut self.session, &req);
        assert_eq!(status, Status::Ok, "apply 失敗: {res:?}");

        self.doc_ver += 1;
        self.model_kind = "Saved".to_owned();
        self.file_loaded = false;
        self.cached = None;
        self.live_kind = "Active".to_owned();
        self.live_ver = self.doc_ver;
        self.last_used_replay = false;
        self.last_ok = true;
        self.refresh_target_from_entities();
    }

    fn apply_live_fault_kept(&mut self) {
        // 存在しない要素の削除要求で拒否させ、Session の文書とハッシュが変わらないことを確認
        let req = r#"{"v":1,"op":"apply","doc":"mbt.dxf","edits":[{"kind":"remove","id":"space/0/hDEAD","hash":"v2:00000000000000000000000000000000"}]}"#;
        let (status, _res) = call_json(&mut self.session, req);
        assert_eq!(status, Status::Failed);
        self.live_kind = "Active".to_owned();
        self.live_ver = self.doc_ver;
        self.last_used_replay = false;
        self.last_ok = false;
        self.refresh_target_from_entities();
    }

    fn apply_live_fault_lost(&mut self) {
        // 極小予算（work=1）での apply 停止で文書が変わらず安全に復旧できることを確認
        let req = format!(
            r#"{{"v":1,"op":"apply","doc":"mbt.dxf","budget":{{"work":1}},"edits":[{{"kind":"transform","id":"{}","hash":"{}","matrix":[1,0,1,0,1,0]}}]}}"#,
            self.current_id, self.current_hash
        );
        let (status, _res) = call_json(&mut self.session, &req);
        assert_eq!(status, Status::Failed);
        self.live_kind = "Absent".to_owned();
        self.live_ver = -1;
        self.last_used_replay = false;
        self.last_ok = false;
        self.refresh_target_from_entities();
    }

    fn apply_fallback_ok(&mut self) {
        // 従来経路フォールバック（通常 apply）で版を進めキャッシュ破棄
        let req = format!(
            r#"{{"v":1,"op":"apply","doc":"mbt.dxf","edits":[{{"kind":"transform","id":"{}","hash":"{}","matrix":[1,0,1,0,1,0]}}]}}"#,
            self.current_id, self.current_hash
        );
        let (status, res) = call_json(&mut self.session, &req);
        assert_eq!(status, Status::Ok, "fallback apply 失敗: {res:?}");

        self.doc_ver += 1;
        self.model_kind = "Held".to_owned();
        self.file_loaded = true;
        self.cached = None;
        self.live_kind = "Absent".to_owned();
        self.live_ver = -1;
        self.last_used_replay = false;
        self.last_ok = true;
        self.refresh_target_from_entities();
    }

    fn load_saved_file(&mut self) {
        // ファイルモデルが必要な `tables` 読み取りで遅延再読が正常に動くことを検証
        let (status, res) = call_json(
            &mut self.session,
            r#"{"v":1,"op":"tables","doc":"mbt.dxf"}"#,
        );
        assert_eq!(status, Status::Ok, "tables 失敗: {res:?}");
        self.file_loaded = true;
        self.last_used_replay = false;
        self.last_ok = true;
    }
}

impl State<ResidentReplayDriver> for ResidentReplayState {
    fn from_driver(driver: &ResidentReplayDriver) -> Result<Self> {
        Ok(Self {
            doc_ver: driver.doc_ver,
            model_kind: driver.model_kind.clone(),
            file_loaded: driver.file_loaded,
            lifted_cached: driver.cached.is_some(),
            lifted_ver: driver.cached.as_ref().map_or(-1, |(v, _)| *v),
            live_kind: driver.live_kind.clone(),
            live_ver: driver.live_ver,
            last_used_replay: driver.last_used_replay,
            last_ok: driver.last_ok,
        })
    }
}

impl Driver for ResidentReplayDriver {
    type State = ResidentReplayState;

    fn step(&mut self, step: &Step) -> Result {
        switch!(step {
            init => self.init(),
            read_plain(work_limit, depth_room, max_places, cancelled) => {
                self.read_plain(work_limit, depth_room, max_places, cancelled);
            }
            read_non_plain(work_limit, depth_room) => {
                self.read_non_plain(work_limit, depth_room);
            }
            apply_live_ok => self.apply_live_ok(),
            apply_live_fault_kept => self.apply_live_fault_kept(),
            apply_live_fault_lost => self.apply_live_fault_lost(),
            apply_fallback_ok => self.apply_fallback_ok(),
            load_saved_file => self.load_saved_file(),
        })
    }
}

#[quint_run(
    spec = "../resident_replay.qnt",
    max_samples = 30,
    max_steps = 25,
    seed = "20261124"
)]
fn mbt_resident_replay_random_traces() -> impl Driver {
    ResidentReplayDriver::default()
}

#[quint_run(
    spec = "../resident_replay.qnt",
    max_samples = 20,
    max_steps = 40,
    seed = "20261125"
)]
fn mbt_resident_replay_deep_traces() -> impl Driver {
    ResidentReplayDriver::default()
}
