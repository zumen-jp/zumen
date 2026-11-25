// Session の常駐キャッシュ（Resident）と Cx::record / Cx::replay の整合性仕様
// （crates/zumen-abi/src/session/resident.rs・drawing_ops/live.rs・crates/zumen-base/src/cx.rs）。
//
// Held / Saved・lifted（常駐図面モデル）・Live（文書エンジン）・Fault（Kept/Lost/Retry）と、
// Cx::record / Cx::replay の 5 条件および入れ子収集スコープの健全性をモデル検査および MBT で検証する。
module resident_replay {
  // 抽象的な変換 1 回が使う作業量と深さ
  pure val LIFT_WORK = 10
  pure val LIFT_DEPTH = 2

  var doc_ver: int
  var model_kind: str        // "Held" | "Saved"
  var file_loaded: bool      // Held は常に true、Saved は遅延再読で true になる
  var lifted_cached: bool
  var lifted_ver: int
  var live_kind: str         // "Absent" | "Active" | "Refused"
  var live_ver: int
  var replay_depth_room: int // -1 は Unlimited 記録
  var replay_max_places: int
  var last_used_replay: bool
  var last_ok: bool

  pure def depth_ok(recorded_room: int, now_room: int): bool =
    if (now_room == -1) true
    else if (recorded_room == -1) false
    else now_room >= recorded_room

  action init = all {
    doc_ver' = 0,
    model_kind' = "Held",
    file_loaded' = true,
    lifted_cached' = false,
    lifted_ver' = -1,
    live_kind' = "Absent",
    live_ver' = -1,
    replay_depth_room' = -1,
    replay_max_places' = 16,
    last_used_replay' = false,
    last_ok' = true,
  }

  // 通常の読み取り（plain スコープ）。条件を満たせば Replay を当て、そうでなければ再変換＋記録。
  action read_plain = {
    nondet work_limit = Set(5, 10, 20).oneOf()
    nondet depth_room = Set(-1, 1, 2, 4).oneOf()
    nondet max_places = Set(2, 16).oneOf()
    nondet cancelled = Set(false, true).oneOf()
    val can_replay =
      lifted_cached
        and not(cancelled)
        and max_places == replay_max_places
        and work_limit >= LIFT_WORK
        and depth_ok(replay_depth_room, depth_room)
    val lift_succeeds =
      not(cancelled)
        and work_limit >= LIFT_WORK
        and (depth_room == -1 or depth_room >= LIFT_DEPTH)
    if (can_replay) all {
      doc_ver' = doc_ver,
      model_kind' = model_kind,
      file_loaded' = file_loaded,
      lifted_cached' = true,
      lifted_ver' = lifted_ver,
      live_kind' = live_kind,
      live_ver' = live_ver,
      replay_depth_room' = replay_depth_room,
      replay_max_places' = replay_max_places,
      last_used_replay' = true,
      last_ok' = true,
    } else if (lift_succeeds) all {
      doc_ver' = doc_ver,
      model_kind' = model_kind,
      file_loaded' = file_loaded,
      lifted_cached' = true,
      lifted_ver' = doc_ver,
      live_kind' = live_kind,
      live_ver' = live_ver,
      replay_depth_room' = depth_room,
      replay_max_places' = max_places,
      last_used_replay' = false,
      last_ok' = true,
    } else all {
      doc_ver' = doc_ver,
      model_kind' = model_kind,
      file_loaded' = file_loaded,
      lifted_cached' = lifted_cached,
      lifted_ver' = lifted_ver,
      live_kind' = live_kind,
      live_ver' = live_ver,
      replay_depth_room' = replay_depth_room,
      replay_max_places' = replay_max_places,
      last_used_replay' = false,
      last_ok' = false,
    }
  }

  // 入力順収集または明細捕捉（非 plain スコープ）の読み取り。Replay を当てず記録も上書きしない。
  action read_non_plain = {
    nondet work_limit = Set(5, 20).oneOf()
    nondet depth_room = Set(1, 4).oneOf()
    val ok = work_limit >= LIFT_WORK and depth_room >= LIFT_DEPTH
    all {
      doc_ver' = doc_ver,
      model_kind' = model_kind,
      file_loaded' = file_loaded,
      lifted_cached' = lifted_cached,
      lifted_ver' = lifted_ver,
      live_kind' = live_kind,
      live_ver' = live_ver,
      replay_depth_room' = replay_depth_room,
      replay_max_places' = replay_max_places,
      last_used_replay' = false,
      last_ok' = ok,
    }
  }

  // 文書エンジンで apply が成功（commit_live）。Saved へ移行し、lifted は破棄、file は未読込になる。
  action apply_live_ok = all {
    live_kind != "Refused",
    doc_ver < 16,
    doc_ver' = doc_ver + 1,
    model_kind' = "Saved",
    file_loaded' = false,
    lifted_cached' = false,
    lifted_ver' = -1,
    live_kind' = "Active",
    live_ver' = doc_ver + 1,
    replay_depth_room' = replay_depth_room,
    replay_max_places' = replay_max_places,
    last_used_replay' = false,
    last_ok' = true,
  }

  // 文書エンジンで apply が拒否または予算停止（Fault::Kept）。文書も常駐キャッシュも変えず engine を戻す。
  action apply_live_fault_kept = all {
    live_kind != "Refused",
    doc_ver' = doc_ver,
    model_kind' = model_kind,
    file_loaded' = file_loaded,
    lifted_cached' = lifted_cached,
    lifted_ver' = lifted_ver,
    live_kind' = "Active",
    live_ver' = doc_ver,
    replay_depth_room' = replay_depth_room,
    replay_max_places' = replay_max_places,
    last_used_replay' = false,
    last_ok' = false,
  }

  // 当てた後の保存・ハッシュで停止（Fault::Lost）。文書は変えず、進んだ engine を破棄して Absent にする。
  action apply_live_fault_lost = all {
    live_kind == "Active",
    doc_ver' = doc_ver,
    model_kind' = model_kind,
    file_loaded' = file_loaded,
    lifted_cached' = lifted_cached,
    lifted_ver' = lifted_ver,
    live_kind' = "Absent",
    live_ver' = -1,
    replay_depth_room' = replay_depth_room,
    replay_max_places' = replay_max_places,
    last_used_replay' = false,
    last_ok' = false,
  }

  // 従来経路での apply 成功（file_mut 経由）。Held に戻り、lifted と live を必ず破棄する。
  action apply_fallback_ok = all {
    doc_ver < 16,
    doc_ver' = doc_ver + 1,
    model_kind' = "Held",
    file_loaded' = true,
    lifted_cached' = false,
    lifted_ver' = -1,
    live_kind' = "Absent",
    live_ver' = -1,
    replay_depth_room' = replay_depth_room,
    replay_max_places' = replay_max_places,
    last_used_replay' = false,
    last_ok' = true,
  }

  // Saved 状態でファイルモデルが初めて必要になったときの遅延再読（OnceCell）。
  action load_saved_file = all {
    model_kind == "Saved",
    not(file_loaded),
    doc_ver' = doc_ver,
    model_kind' = "Saved",
    file_loaded' = true,
    lifted_cached' = lifted_cached,
    lifted_ver' = lifted_ver,
    live_kind' = live_kind,
    live_ver' = live_ver,
    replay_depth_room' = replay_depth_room,
    replay_max_places' = replay_max_places,
    last_used_replay' = false,
    last_ok' = true,
  }

  action step = any {
    read_plain,
    read_non_plain,
    apply_live_ok,
    apply_live_fault_kept,
    apply_live_fault_lost,
    apply_fallback_ok,
    load_saved_file,
  }

  // ── 安全性不変条件 ──

  // 常駐図面モデルが保持されているなら、必ず正の文書と同じ版である。
  val inv_lifted_consistent: bool =
    lifted_cached implies (lifted_ver == doc_ver)

  // 文書エンジンが Active なら、必ず正の文書と同じ版である。
  val inv_live_consistent: bool =
    live_kind == "Active" implies (live_ver == doc_ver)

  // Held 状態ではファイルモデルが必ず存在する。
  val inv_held_has_file: bool =
    model_kind == "Held" implies file_loaded

  // Replay を当てた読み取りは途中で失敗せず必ず成功する。
  val inv_replay_never_fails: bool =
    last_used_replay implies last_ok

  val inv_all: bool = all {
    inv_lifted_consistent,
    inv_live_consistent,
    inv_held_has_file,
    inv_replay_never_fails,
  }

  run residentLifecycleTest =
    init
      .then(read_plain)
      .then(apply_live_ok)
      .then(read_plain)
      .then(apply_live_fault_lost)
      .then(load_saved_file)
      .then(apply_fallback_ok)
      .then(read_plain)
}
