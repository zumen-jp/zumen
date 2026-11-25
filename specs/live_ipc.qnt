// 常駐子プロセス IPC（ZLIP/ZLIR）の版・表示差分受領確認・診断寿命の仕様（bindings/cli/live.md）。
//
// 編集確定と表示取得の分離、古い確認や編集失敗で表示変更フラグを消さない規則、
// 診断取得失敗時の元診断保持、Open の 1 回限り性をモデル検査および MBT で検証する。
module live_ipc {
  // ── 状態変数（サーバー側とクライアントミラー） ──
  var opened: bool
  var doc_rev: int
  var doc_value: int
  var undo_stack: List[int]
  var redo_stack: List[int]
  var dirty: bool
  var sent: int
  var confirmed: int
  var client_rev: int
  var client_value: int
  var req_id: int
  var stored_diag_req_id: int
  var last_status: str
  var last_display_status: str

  // ── 純粋補助関数 ──

  // 現在の版を送った直後の同じ版の確認だけを受け入れる（bindings/cli/src/live/document.rs）。
  pure def can_confirm(ack: int, s: int, rev: int): bool =
    ack >= 0 and s == ack and rev == ack

  // クライアント側が ack に載せる版の候補を解決する（0: なし、1: 手元の版、2: 古い版 0）。
  pure def resolve_ack(mode: int, c_rev: int): int =
    if (mode == 1) c_rev
    else if (mode == 2) 0
    else -1

  // 表示パケット生成後にクライアントミラーへ反映した後の (client_rev, client_value)。
  // 差分パケットは dirty が偽なら空になるため、誤って dirty を消すと client_value が更新されず
  // 不変条件 inv_client_mirror_consistent でドリフトが検出される。
  pure def apply_display_to_client(
    full_req: bool,
    cur_confirmed: int,
    cur_rev: int,
    cur_val: int,
    cur_dirty: bool,
    c_rev: int,
    c_val: int
  ): (int, int) = {
    val effective_full = full_req or cur_confirmed == -1
    val base = if (effective_full) -1 else cur_confirmed
    if (effective_full) {
      (cur_rev, cur_val)
    } else if (c_rev == base) {
      val next_val = if (cur_dirty) cur_val else c_val
      (cur_rev, next_val)
    } else {
      (c_rev, c_val)
    }
  }

  // ── 初期状態 ──
  action init = all {
    opened' = false,
    doc_rev' = -1,
    doc_value' = 0,
    undo_stack' = List(),
    redo_stack' = List(),
    dirty' = false,
    sent' = -1,
    confirmed' = -1,
    client_rev' = -1,
    client_value' = 0,
    req_id' = 0,
    stored_diag_req_id' = -1,
    last_status' = "ok",
    last_display_status' = "ok",
  }

  // ── アクション ──

  // 文書を開く。既に開いている場合は AlreadyOpen で文書を変えない。
  action open_doc = {
    val next_req = req_id + 1
    if (not(opened)) all {
      opened' = true,
      doc_rev' = 0,
      doc_value' = 0,
      undo_stack' = List(),
      redo_stack' = List(),
      dirty' = true,
      sent' = -1,
      confirmed' = -1,
      client_rev' = -1,
      client_value' = 0,
      req_id' = next_req,
      stored_diag_req_id' = next_req,
      last_status' = "ok",
      last_display_status' = "ok",
    } else all {
      opened' = opened,
      doc_rev' = doc_rev,
      doc_value' = doc_value,
      undo_stack' = undo_stack,
      redo_stack' = redo_stack,
      dirty' = dirty,
      sent' = sent,
      confirmed' = confirmed,
      client_rev' = client_rev,
      client_value' = client_value,
      req_id' = next_req,
      stored_diag_req_id' = next_req,
      last_status' = "already_open",
      last_display_status' = "ok",
    }
  }

  // 未開の状態で編集や表示を呼ぶと NoDocument を返す。
  action op_before_open = all {
    not(opened),
    opened' = false,
    doc_rev' = doc_rev,
    doc_value' = doc_value,
    undo_stack' = undo_stack,
    redo_stack' = redo_stack,
    dirty' = dirty,
    sent' = sent,
    confirmed' = confirmed,
    client_rev' = client_rev,
    client_value' = client_value,
    req_id' = req_id + 1,
    stored_diag_req_id' = req_id + 1,
    last_status' = "no_document",
    last_display_status' = "ok",
  }

  // 編集成功＋表示取得成功（受領確認は編集前に独立処理し、編集確定後に表示を生成する）。
  action edit_ok_with_display = {
    nondet ack_mode = Set(0, 1, 2).oneOf()
    nondet full_flag = Set(false, true).oneOf()
    nondet delta = Set(1, 2).oneOf()
    val ack = resolve_ack(ack_mode, client_rev)
    val confirmed_after_ack = if (can_confirm(ack, sent, doc_rev)) ack else confirmed
    val next_rev = doc_rev + 1
    val next_val = doc_value + delta
    val applied = apply_display_to_client(
      full_flag, confirmed_after_ack, next_rev, next_val, true, client_rev, client_value
    )
    all {
      opened,
      doc_rev < 16,
      opened' = true,
      doc_rev' = next_rev,
      doc_value' = next_val,
      undo_stack' = undo_stack.append(doc_value),
      redo_stack' = List(),
      dirty' = true,
      sent' = next_rev,
      confirmed' = confirmed_after_ack,
      client_rev' = applied._1,
      client_value' = applied._2,
      req_id' = req_id + 1,
      stored_diag_req_id' = req_id + 1,
      last_status' = "ok",
      last_display_status' = "ok",
    }
  }

  // 編集確定は成功し、続く表示生成だけ予算停止する。編集は巻き戻さず dirty を保つ。
  action edit_ok_display_exceeded = {
    nondet ack_mode = Set(0, 1, 2).oneOf()
    nondet delta = Set(1, 2).oneOf()
    val ack = resolve_ack(ack_mode, client_rev)
    val confirmed_after_ack = if (can_confirm(ack, sent, doc_rev)) ack else confirmed
    val next_rev = doc_rev + 1
    val next_val = doc_value + delta
    all {
      opened,
      doc_rev < 16,
      opened' = true,
      doc_rev' = next_rev,
      doc_value' = next_val,
      undo_stack' = undo_stack.append(doc_value),
      redo_stack' = List(),
      dirty' = true,
      sent' = sent,
      confirmed' = confirmed_after_ack,
      client_rev' = client_rev,
      client_value' = client_value,
      req_id' = req_id + 1,
      stored_diag_req_id' = req_id + 1,
      last_status' = "ok",
      last_display_status' = "exceeded",
    }
  }

  // 編集が拒否される。前提の版が一致しているため有効な受領確認は独立して保持される。
  action edit_rejected = {
    nondet ack_mode = Set(0, 1, 2).oneOf()
    val ack = resolve_ack(ack_mode, client_rev)
    val ok_ack = can_confirm(ack, sent, doc_rev)
    all {
      opened,
      opened' = true,
      doc_rev' = doc_rev,
      doc_value' = doc_value,
      undo_stack' = undo_stack,
      redo_stack' = redo_stack,
      dirty' = if (ok_ack) false else dirty,
      sent' = sent,
      confirmed' = if (ok_ack) ack else confirmed,
      client_rev' = client_rev,
      client_value' = client_value,
      req_id' = req_id + 1,
      stored_diag_req_id' = req_id + 1,
      last_status' = "rejected",
      last_display_status' = "ok",
    }
  }

  // 前提の版が違う要求。受領確認より前に Stale で断り、状態を一切変えない。
  action edit_stale = all {
    opened,
    opened' = true,
    doc_rev' = doc_rev,
    doc_value' = doc_value,
    undo_stack' = undo_stack,
    redo_stack' = redo_stack,
    dirty' = dirty,
    sent' = sent,
    confirmed' = confirmed,
    client_rev' = client_rev,
    client_value' = client_value,
    req_id' = req_id + 1,
    stored_diag_req_id' = req_id + 1,
    last_status' = "stale",
    last_display_status' = "ok",
  }

  // Undo＋表示取得。履歴があれば 1 段戻して版を進め、空なら文書を変えずに表示を返す。
  action undo_with_display = {
    nondet ack_mode = Set(0, 1, 2).oneOf()
    nondet full_flag = Set(false, true).oneOf()
    val ack = resolve_ack(ack_mode, client_rev)
    val ok_ack = can_confirm(ack, sent, doc_rev)
    val confirmed_after_ack = if (ok_ack) ack else confirmed
    val dirty_after_ack = if (ok_ack) false else dirty
    val has_undo = undo_stack.length() > 0
    val next_rev = if (has_undo) doc_rev + 1 else doc_rev
    val next_val = if (has_undo) undo_stack[undo_stack.length() - 1] else doc_value
    val next_undo = if (has_undo) undo_stack.slice(0, undo_stack.length() - 1) else undo_stack
    val next_redo = if (has_undo) redo_stack.append(doc_value) else redo_stack
    val next_dirty = if (has_undo) true else dirty_after_ack
    val applied = apply_display_to_client(
      full_flag, confirmed_after_ack, next_rev, next_val, next_dirty, client_rev, client_value
    )
    all {
      opened,
      doc_rev < 16,
      opened' = true,
      doc_rev' = next_rev,
      doc_value' = next_val,
      undo_stack' = next_undo,
      redo_stack' = next_redo,
      dirty' = next_dirty,
      sent' = next_rev,
      confirmed' = confirmed_after_ack,
      client_rev' = applied._1,
      client_value' = applied._2,
      req_id' = req_id + 1,
      stored_diag_req_id' = req_id + 1,
      last_status' = "ok",
      last_display_status' = "ok",
    }
  }

  // Redo＋表示取得。やり直し履歴があれば 1 段進めて版を進め、空なら文書を変えない。
  action redo_with_display = {
    nondet ack_mode = Set(0, 1, 2).oneOf()
    nondet full_flag = Set(false, true).oneOf()
    val ack = resolve_ack(ack_mode, client_rev)
    val ok_ack = can_confirm(ack, sent, doc_rev)
    val confirmed_after_ack = if (ok_ack) ack else confirmed
    val dirty_after_ack = if (ok_ack) false else dirty
    val has_redo = redo_stack.length() > 0
    val next_rev = if (has_redo) doc_rev + 1 else doc_rev
    val next_val = if (has_redo) redo_stack[redo_stack.length() - 1] else doc_value
    val next_redo = if (has_redo) redo_stack.slice(0, redo_stack.length() - 1) else redo_stack
    val next_undo = if (has_redo) undo_stack.append(doc_value) else undo_stack
    val next_dirty = if (has_redo) true else dirty_after_ack
    val applied = apply_display_to_client(
      full_flag, confirmed_after_ack, next_rev, next_val, next_dirty, client_rev, client_value
    )
    all {
      opened,
      doc_rev < 16,
      opened' = true,
      doc_rev' = next_rev,
      doc_value' = next_val,
      undo_stack' = next_undo,
      redo_stack' = next_redo,
      dirty' = next_dirty,
      sent' = next_rev,
      confirmed' = confirmed_after_ack,
      client_rev' = applied._1,
      client_value' = applied._2,
      req_id' = req_id + 1,
      stored_diag_req_id' = req_id + 1,
      last_status' = "ok",
      last_display_status' = "ok",
    }
  }

  // Display コマンドで表示だけを取得してクライアントへ反映する。
  action display_sync = {
    nondet ack_mode = Set(0, 1, 2).oneOf()
    nondet full_flag = Set(false, true).oneOf()
    val ack = resolve_ack(ack_mode, client_rev)
    val ok_ack = can_confirm(ack, sent, doc_rev)
    val confirmed_after_ack = if (ok_ack) ack else confirmed
    val dirty_after_ack = if (ok_ack) false else dirty
    val applied = apply_display_to_client(
      full_flag, confirmed_after_ack, doc_rev, doc_value, dirty_after_ack, client_rev, client_value
    )
    all {
      opened,
      opened' = true,
      doc_rev' = doc_rev,
      doc_value' = doc_value,
      undo_stack' = undo_stack,
      redo_stack' = redo_stack,
      dirty' = dirty_after_ack,
      sent' = doc_rev,
      confirmed' = confirmed_after_ack,
      client_rev' = applied._1,
      client_value' = applied._2,
      req_id' = req_id + 1,
      stored_diag_req_id' = req_id + 1,
      last_status' = "ok",
      last_display_status' = "ok",
    }
  }

  // 保持中の診断を取得する（成功）。元の診断スロットは置き換えない。
  action diagnostics_ok = all {
    stored_diag_req_id > 0,
    opened' = opened,
    doc_rev' = doc_rev,
    doc_value' = doc_value,
    undo_stack' = undo_stack,
    redo_stack' = redo_stack,
    dirty' = dirty,
    sent' = sent,
    confirmed' = confirmed,
    client_rev' = client_rev,
    client_value' = client_value,
    req_id' = req_id + 1,
    stored_diag_req_id' = stored_diag_req_id,
    last_status' = "ok",
    last_display_status' = "ok",
  }

  // 診断取得が予算超過で止まる。保持元の診断スロットは消さずに再試行可能を保つ。
  action diagnostics_exceeded = all {
    stored_diag_req_id > 0,
    opened' = opened,
    doc_rev' = doc_rev,
    doc_value' = doc_value,
    undo_stack' = undo_stack,
    redo_stack' = redo_stack,
    dirty' = dirty,
    sent' = sent,
    confirmed' = confirmed,
    client_rev' = client_rev,
    client_value' = client_value,
    req_id' = req_id + 1,
    stored_diag_req_id' = stored_diag_req_id,
    last_status' = "exceeded",
    last_display_status' = "ok",
  }

  // 一致しない要求番号で診断を引くと NoDiagnostics となり保持元は変わらない。
  action diagnostics_mismatch = all {
    opened' = opened,
    doc_rev' = doc_rev,
    doc_value' = doc_value,
    undo_stack' = undo_stack,
    redo_stack' = redo_stack,
    dirty' = dirty,
    sent' = sent,
    confirmed' = confirmed,
    client_rev' = client_rev,
    client_value' = client_value,
    req_id' = req_id + 1,
    stored_diag_req_id' = stored_diag_req_id,
    last_status' = "no_diagnostics",
    last_display_status' = "ok",
  }

  action step = any {
    open_doc,
    op_before_open,
    edit_ok_with_display,
    edit_ok_display_exceeded,
    edit_rejected,
    edit_stale,
    undo_with_display,
    redo_with_display,
    display_sync,
    diagnostics_ok,
    diagnostics_exceeded,
    diagnostics_mismatch,
  }

  // ── 安全性不変条件 ──

  // クライアントが最新の版を反映済みなら、ミラーの内容は必ずサーバーと一致する。
  val inv_client_mirror_consistent: bool =
    (opened and client_rev == doc_rev) implies (client_value == doc_value)

  // 確認済みの版が現在の版と一致するなら、dirty は偽かつクライアントは最新内容を持つ。
  val inv_confirmed_implies_clean_and_consistent: bool =
    (opened and confirmed == doc_rev) implies (not(dirty) and client_rev == doc_rev and client_value == doc_value)

  // クライアントの内容がサーバーと食い違っているのに dirty が消えて差分を取りこぼすことは起きない。
  val inv_no_lost_diff: bool =
    (opened and confirmed >= 0 and client_value != doc_value) implies dirty

  // 版番号の単調順序：confirmed <= sent <= doc_rev、かつ client_rev <= doc_rev。
  val inv_revision_order: bool =
    confirmed <= sent and sent <= doc_rev and client_rev <= doc_rev

  // 編集確定と表示停止の分離：表示だけが予算停止したときは操作自体は必ず ok。
  val inv_display_stop_preserves_ok: bool =
    last_display_status == "exceeded" implies last_status == "ok"

  // 診断スロットは常に直近までの要求番号を指す。
  val inv_diag_slot_bound: bool =
    stored_diag_req_id <= req_id

  val inv_all: bool = all {
    inv_client_mirror_consistent,
    inv_confirmed_implies_clean_and_consistent,
    inv_no_lost_diff,
    inv_revision_order,
    inv_display_stop_preserves_ok,
    inv_diag_slot_bound,
  }

  // 代表シナリオのユニットテスト（quint test）
  run recoveryAfterDisplayStopTest =
    init
      .then(open_doc)
      .then(display_sync)
      .then(edit_ok_display_exceeded)
      .then(diagnostics_exceeded)
      .then(diagnostics_ok)
      .then(display_sync)
      .then(undo_with_display)
      .then(redo_with_display)
}
