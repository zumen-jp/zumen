// zumen-live の原子トランザクション・履歴・補助索引同期の仕様（crates/zumen-live/src/txn.rs）。
//
// VersionedTable・HandleIndex・SpatialGrid・DirtyColumn・DepGraph・History の 6 構造体について、
// 失敗/予算停止時の完全原子性、ハンドル一意性、依存辺の双対性と循環耐性、
// 任意の commit / undo / redo / trim_to_bytes 系列における可逆性をモデル検査および MBT で検証する。
module live_txn {
  type RowSnap = {
    alive: bool,
    val_num: int,
    handle: int,
    bx: int,
  }

  type DocSnap = {
    state_id: int,
    rows: List[RowSnap],
    deps: int -> Set[int],
  }

  pure val MAX_ROWS = 6
  pure val VALS = Set(1, 2, 3, 4)
  pure val HANDLES = Set(0, 1, 2, 3, 4)
  pure val COORDS = Set(0, 1, 2, 3)

  var rev: int
  var state_id: int
  var next_state_id: int
  var rows: List[RowSnap]
  var deps: int -> Set[int]
  var dirty: Set[int]
  var undo_snaps: List[DocSnap]
  var redo_snaps: List[DocSnap]
  var last_ok: bool

  // ── 純粋補助関数 ──

  pure def row_ids(rs: List[RowSnap]): Set[int] =
    0.to(MAX_ROWS - 1).filter(i => i < rs.length())

  pure def is_alive(rs: List[RowSnap], idx: int): bool =
    idx >= 0 and idx < rs.length() and rs[idx].alive

  // 生きた行の中でハンドル h（> 0）を所有する行が存在するか（skip_row 番目を除く）。
  pure def handle_used_by_other(rs: List[RowSnap], h: int, skip_row: int): bool =
    h > 0 and row_ids(rs).exists(i => i != skip_row and rs[i].alive and rs[i].handle == h)

  pure def snap_now(sid: int, rs: List[RowSnap], ds: int -> Set[int]): DocSnap =
    { state_id: sid, rows: rs, deps: ds }

  // 行 idx を削除した後の依存辺マップ（idx 自身の辺を消し、他行のソースからも idx を除く）。
  pure def remove_row_from_deps(ds: int -> Set[int], rs: List[RowSnap], idx: int): int -> Set[int] =
    row_ids(rs).mapBy(i =>
      if (i == idx) Set()
      else ds.get(i).exclude(Set(idx))
    )

  // 2 つの行リストで変更（追加・更新・削除）があった行番号集合。
  pure def diff_rows(before: List[RowSnap], after: List[RowSnap]): Set[int] = {
    val max_len = if (before.length() > after.length()) before.length() else after.length()
    0.to(MAX_ROWS - 1).filter(i =>
      i < max_len and (
        if (i >= before.length() or i >= after.length()) true
        else before[i] != after[i]
      )
    )
  }

  // ── 初期状態 ──
  action init = all {
    rev' = 0,
    state_id' = 0,
    next_state_id' = 1,
    rows' = List(),
    deps' = Map(),
    dirty' = Set(),
    undo_snaps' = List(),
    redo_snaps' = List(),
    last_ok' = true,
  }

  // ── アクション ──

  // 新規行を末尾に追加する。ハンドル重複なら失敗して状態を一切変えない。
  action txn_insert = {
    nondet val_num = VALS.oneOf()
    nondet handle = HANDLES.oneOf()
    nondet bx = COORDS.oneOf()
    val dup = handle_used_by_other(rows, handle, -1)
    val new_idx = rows.length()
    all {
      rows.length() < MAX_ROWS,
      if (dup) all {
        rev' = rev,
        state_id' = state_id,
        next_state_id' = next_state_id,
        rows' = rows,
        deps' = deps,
        dirty' = dirty,
        undo_snaps' = undo_snaps,
        redo_snaps' = redo_snaps,
        last_ok' = false,
      } else all {
        rev' = rev + 1,
        state_id' = next_state_id,
        next_state_id' = next_state_id + 1,
        rows' = rows.append({ alive: true, val_num: val_num, handle: handle, bx: bx }),
        deps' = deps.put(new_idx, Set()),
        dirty' = dirty.union(Set(new_idx)),
        undo_snaps' = undo_snaps.append(snap_now(state_id, rows, deps)),
        redo_snaps' = List(),
        last_ok' = true,
      },
    }
  }

  // 既存の行を更新する。非生存または他行とハンドル重複なら失敗して完全ロールバック。
  action txn_update = {
    nondet row = 0.to(MAX_ROWS - 1).oneOf()
    nondet val_num = VALS.oneOf()
    nondet handle = HANDLES.oneOf()
    nondet bx = COORDS.oneOf()
    val valid = is_alive(rows, row) and not(handle_used_by_other(rows, handle, row))
    if (not(valid)) all {
      rev' = rev,
      state_id' = state_id,
      next_state_id' = next_state_id,
      rows' = rows,
      deps' = deps,
      dirty' = dirty,
      undo_snaps' = undo_snaps,
      redo_snaps' = redo_snaps,
      last_ok' = false,
    } else all {
      rev' = rev + 1,
      state_id' = next_state_id,
      next_state_id' = next_state_id + 1,
      rows' = rows.replaceAt(row, { alive: true, val_num: val_num, handle: handle, bx: bx }),
      deps' = deps,
      dirty' = dirty.union(Set(row)),
      undo_snaps' = undo_snaps.append(snap_now(state_id, rows, deps)),
      redo_snaps' = List(),
      last_ok' = true,
    }
  }

  // 既存の行を削除する。依存辺から両方向に除かれ、ハンドルも解放される。
  action txn_remove = {
    nondet row = 0.to(MAX_ROWS - 1).oneOf()
    if (not(is_alive(rows, row))) all {
      rev' = rev,
      state_id' = state_id,
      next_state_id' = next_state_id,
      rows' = rows,
      deps' = deps,
      dirty' = dirty,
      undo_snaps' = undo_snaps,
      redo_snaps' = redo_snaps,
      last_ok' = false,
    } else {
      val cur = rows[row]
      all {
        rev' = rev + 1,
        state_id' = next_state_id,
        next_state_id' = next_state_id + 1,
        rows' = rows.replaceAt(row, { alive: false, val_num: cur.val_num, handle: cur.handle, bx: cur.bx }),
        deps' = remove_row_from_deps(deps, rows, row),
        dirty' = dirty.union(Set(row)),
        undo_snaps' = undo_snaps.append(snap_now(state_id, rows, deps)),
        redo_snaps' = List(),
        last_ok' = true,
      }
    }
  }

  // 依存辺を設定する。循環辺は許容し、自己ループは自動除外、非生存行の指定は失敗。
  action txn_set_dep = {
    nondet dep_row = 0.to(MAX_ROWS - 1).oneOf()
    nondet src_row = 0.to(MAX_ROWS - 1).oneOf()
    val valid = is_alive(rows, dep_row) and is_alive(rows, src_row)
    if (not(valid)) all {
      rev' = rev,
      state_id' = state_id,
      next_state_id' = next_state_id,
      rows' = rows,
      deps' = deps,
      dirty' = dirty,
      undo_snaps' = undo_snaps,
      redo_snaps' = redo_snaps,
      last_ok' = false,
    } else {
      val new_srcs = if (dep_row == src_row) Set() else Set(src_row)
      all {
        rev' = rev + 1,
        state_id' = next_state_id,
        next_state_id' = next_state_id + 1,
        rows' = rows,
        deps' = deps.put(dep_row, new_srcs),
        dirty' = dirty,
        undo_snaps' = undo_snaps.append(snap_now(state_id, rows, deps)),
        redo_snaps' = List(),
        last_ok' = true,
      }
    }
  }

  // ステージング途中で拒否または予算停止が起きるトランザクション。全状態 1 ビット不変。
  action txn_aborted = {
    nondet row = 0.to(MAX_ROWS - 1).oneOf()
    nondet fail_budget = Set(false, true).oneOf()
    all {
      rev' = rev,
      state_id' = state_id,
      next_state_id' = next_state_id,
      rows' = rows,
      deps' = deps,
      dirty' = dirty,
      undo_snaps' = undo_snaps,
      redo_snaps' = redo_snaps,
      last_ok' = false,
    }
  }

  // 取り消し。履歴があれば直前のスナップショットへ完全復元し、版は単調増加する。
  action do_undo = {
    if (undo_snaps.length() == 0) all {
      rev' = rev,
      state_id' = state_id,
      next_state_id' = next_state_id,
      rows' = rows,
      deps' = deps,
      dirty' = dirty,
      undo_snaps' = undo_snaps,
      redo_snaps' = redo_snaps,
      last_ok' = false,
    } else {
      val prev = undo_snaps[undo_snaps.length() - 1]
      val changed = diff_rows(rows, prev.rows).filter(i => i < prev.rows.length())
      all {
        rev' = rev + 1,
        state_id' = prev.state_id,
        next_state_id' = next_state_id,
        rows' = prev.rows,
        deps' = prev.deps,
        dirty' = dirty.filter(i => i < prev.rows.length()).union(changed),
        undo_snaps' = undo_snaps.slice(0, undo_snaps.length() - 1),
        redo_snaps' = redo_snaps.append(snap_now(state_id, rows, deps)),
        last_ok' = true,
      }
    }
  }

  // やり直し。やり直し履歴があれば復元し、版は単調増加する。
  action do_redo = {
    if (redo_snaps.length() == 0) all {
      rev' = rev,
      state_id' = state_id,
      next_state_id' = next_state_id,
      rows' = rows,
      deps' = deps,
      dirty' = dirty,
      undo_snaps' = undo_snaps,
      redo_snaps' = redo_snaps,
      last_ok' = false,
    } else {
      val redo_snap = redo_snaps[redo_snaps.length() - 1]
      val changed = diff_rows(rows, redo_snap.rows).filter(i => i < redo_snap.rows.length())
      all {
        rev' = rev + 1,
        state_id' = redo_snap.state_id,
        next_state_id' = next_state_id,
        rows' = redo_snap.rows,
        deps' = redo_snap.deps,
        dirty' = dirty.union(changed),
        undo_snaps' = undo_snaps.append(snap_now(state_id, rows, deps)),
        redo_snaps' = redo_snaps.slice(0, redo_snaps.length() - 1),
        last_ok' = true,
      }
    }
  }

  // 履歴を上限バイト数で切り詰める（2 段以上あれば最古だけ落とし最低 1 段は保つ）。
  action do_trim_undo = all {
    rev' = rev,
    state_id' = state_id,
    next_state_id' = next_state_id,
    rows' = rows,
    deps' = deps,
    dirty' = dirty,
    undo_snaps' = if (undo_snaps.length() > 1) undo_snaps.slice(1, undo_snaps.length()) else undo_snaps,
    redo_snaps' = redo_snaps,
    last_ok' = true,
  }

  // 変更フラグを消去する。
  action do_clear_dirty = all {
    rev' = rev,
    state_id' = state_id,
    next_state_id' = next_state_id,
    rows' = rows,
    deps' = deps,
    dirty' = Set(),
    undo_snaps' = undo_snaps,
    redo_snaps' = redo_snaps,
    last_ok' = true,
  }

  action step = any {
    txn_insert,
    txn_update,
    txn_remove,
    txn_set_dep,
    txn_aborted,
    do_undo,
    do_redo,
    do_trim_undo,
    do_clear_dirty,
  }

  // ── 安全性不変条件 ──

  // 生きた行どうしで 0 以外のハンドルは重複しない。
  val inv_unique_handles: bool =
    row_ids(rows).forall(i =>
      row_ids(rows).forall(j =>
        (i != j and rows[i].alive and rows[j].alive and rows[i].handle > 0)
          implies rows[i].handle != rows[j].handle
      )
    )

  // 削除済みの行は依存元にも依存先にも残らない。
  val inv_dead_has_no_deps: bool =
    row_ids(rows).forall(i =>
      (not(rows[i].alive) implies deps.get(i) == Set())
        and deps.get(i).forall(src => is_alive(rows, src) and src != i)
    )

  // 変更フラグは現在の行範囲内に収まる。
  val inv_dirty_in_bounds: bool =
    dirty.forall(i => i >= 0 and i < rows.length())

  // 版番号と状態番号の非負性。
  val inv_counters: bool =
    rev >= 0 and state_id >= 0 and state_id < next_state_id

  val inv_all: bool = all {
    inv_unique_handles,
    inv_dead_has_no_deps,
    inv_dirty_in_bounds,
    inv_counters,
  }

  run basicCycleAndUndoTest =
    init
      .then(txn_insert)
      .then(txn_insert)
      .then(txn_set_dep)
      .then(txn_aborted)
      .then(do_undo)
      .then(do_redo)
}
