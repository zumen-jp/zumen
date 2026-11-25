# 形式検証とモデルベーステスト

このディレクトリには、zumen の状態機械・プロトコルを記述した [Quint](https://quint-lang.org/) 仕様と、仕様から生成したトレースを実コードへリプレイするモデルベーステスト（MBT）を置きます。あわせて、リポジトリ全体で使っている Miri・Kani・Quint の 3 層の検証の役割と動かし方をまとめます。

## 3 層の検証の役割

| 層 | ツール | 対象 | 確かめる性質 |
| --- | --- | --- | --- |
| 動的 UB 検査 | Miri（Tree Borrows・Strict Provenance・Isolation） | `zumen-abi` の FFI 境界、`zumen-base`・`zumen-json`・`zumen-geom2d`・`zumen-live` の純 Rust 部品（64 bit および 32 bit `i686`） | 生ポインターの借用規則・アライメント・ポインター出所・並列中断と閉じる操作のデータ競合なし |
| 静的境界モデル検査 | Kani（CBMC・Kissat・関数契約） | `zumen-base`・`zumen-geom2d`・`zumen-live`・`zumen-building`（`measure`）・`zumen-zip`・`zumen-las`・`zumen-acad` | 全入力域での桁あふれなし・パニックなし・境界検査・決定性・予算超過時の無変更・データ構造の参照モデル同値 |
| 形式仕様・MBT | Quint・Apalache・`quint-connect` | `specs/*.qnt` と `specs/mbt/`（実 `zumen live --stdio`・`zumen-live`・`zumen-base::Cx`・`zumen-abi1::Session`） | プロトコル不変条件・記号モデル検査・生成トレースと実装のステップごとの状態一致 |

Miri・Kani・Quint・Apalache は Cargo の外で動き、配布する成果物に何も残しません（[依存と照合ツール](../CONTRIBUTING.md#依存と照合ツール)）。`specs/mbt/` は `fuzz/` と同じくルートワークスペースの外に独立したパッケージとして置き、公開 crate の依存を汚しません。

## ファイルの構成

| パス | 内容 |
| --- | --- |
| `specs/live_ipc.qnt` | `zumen live --stdio` の 2 進フレーム（ZLIP / ZLIR / ZLDP）プロトコル仕様（開閉・版の単調増加・ACK と再同期・編集確定後の表示予算超過からの復旧・診断保持） |
| `specs/live_txn.qnt` | `zumen-live` のトランザクションと履歴の仕様（`VersionedTable`・`SpatialGrid`・`DirtyColumn`・`DepGraph`・`History` の原子的反映・ロールバック・循環依存拒否・undo / redo・履歴切り詰め） |
| `specs/resident_replay.qnt` | 常駐セッションの `Resident` 状態遷移と `Cx::record` / `Cx::replay` の仕様（`Held` / `Saved` / 遅延再読・文書エンジン障害時の復旧・非 plain スコープでの記録抑止） |
| `specs/mbt/` | `quint-connect` を使い、3 仕様のトレースを実コードと子プロセスへリプレイして状態を照合する Rust テストパッケージ |

## 手元での動かし方

Linux（x86_64）または Windows 11 上の WSL2（Ubuntu）で実行します。Kani は Linux と macOS に対応しているため、Windows では WSL2 内で実行してください。

### 1. Miri

nightly の Rust（バージョンは `.github/workflows/verify.yml` に固定）に `miri`・`rust-src` と 32 bit ターゲット `i686-unknown-linux-gnu` を入れます。

```sh
NIGHTLY=nightly-2026-10-30
rustup toolchain install "$NIGHTLY" --profile minimal --component rust-src,miri
rustup target add i686-unknown-linux-gnu --toolchain "$NIGHTLY"

# FFI 境界（Tree Borrows + Strict Provenance + Symbolic Alignment + Isolation）
MIRIFLAGS="-Zmiri-tree-borrows -Zmiri-strict-provenance -Zmiri-symbolic-alignment-check" \
  cargo "+$NIGHTLY" miri test -p zumen-abi --lib ffi::tests --locked

# 並列 cancel / busy close のデータ競合検査（複数シードのプリエンプション）
MIRIFLAGS="-Zmiri-tree-borrows -Zmiri-strict-provenance -Zmiri-many-seeds=0..8 -Zmiri-preemption-rate=0.1" \
  cargo "+$NIGHTLY" miri test -p zumen-abi --lib ffi::tests::concurrent_cancel_and_busy_close_keep_session_sound --locked

# 純 Rust コア（64 bit と 32 bit クロス検証）
MIRIFLAGS="-Zmiri-tree-borrows -Zmiri-strict-provenance -Zmiri-symbolic-alignment-check" \
  cargo "+$NIGHTLY" miri test -p zumen-base --lib --locked
MIRIFLAGS="-Zmiri-tree-borrows -Zmiri-strict-provenance -Zmiri-symbolic-alignment-check" \
  cargo "+$NIGHTLY" miri test --target i686-unknown-linux-gnu -p zumen-live --lib --locked -- dirty::tests handle::tests cow::tests
```

### 2. Kani

バージョンを固定した `kani-verifier` を入れます。

```sh
cargo install --locked kani-verifier --version 0.68.0
cargo kani setup

cargo kani -p zumen-base --output-format terse
cargo kani -Z function-contracts -Z stubbing -p zumen-geom2d --output-format terse
cargo kani -p zumen-live --output-format terse
cargo kani -p zumen-building --features measure --output-format terse
cargo kani -p zumen-zip --output-format terse
cargo kani -p zumen-las --output-format terse
cargo kani -p zumen-acad --output-format terse
```

### 3. Quint・Apalache・`quint-connect` MBT

Node.js 24 以降と Quint CLI を入れます。`quint verify`（Apalache による記号モデル検査）を動かす場合は Java 17 以降も必要です。

```sh
npm install -g @informalsystems/quint@0.34.0

# 型検査・単体テスト・ランダムシミュレーション
for spec in specs/live_ipc.qnt specs/live_txn.qnt specs/resident_replay.qnt; do
  quint typecheck "$spec"
  quint test "$spec"
  quint run --max-samples=2000 --max-steps=40 --invariant=inv_all "$spec"
done

# Apalache による記号モデル検査（_apalache-out を一時ディレクトリへ逃がす）
tmp_apalache="${TMPDIR:-/tmp}/zumen-apalache"
mkdir -p "$tmp_apalache"
root="$PWD"
(cd "$tmp_apalache" && quint verify --max-steps=6 --invariant=inv_all "$root/specs/live_ipc.qnt")
(cd "$tmp_apalache" && quint verify --max-steps=4 --invariant=inv_all "$root/specs/live_txn.qnt")
(cd "$tmp_apalache" && quint verify --max-steps=6 --invariant=inv_all "$root/specs/resident_replay.qnt")

# 実装へのリプレイ MBT（zumen live --stdio 用の CLI を先にビルドする）
cargo build --locked -p zumen-cli
ZUMEN_BIN="$PWD/target/debug/zumen" cargo test --manifest-path specs/mbt/Cargo.toml
```

失敗したトレースを再現するときは、出力に表示されたシードを `QUINT_SEED=...` に渡して再実行してください。

## CI での実行

[`.github/workflows/verify.yml`](../.github/workflows/verify.yml) で毎週定期実行し、対象の crate・CLI・`specs/`・ワークフロー本体を変えた PR でも実行します。権限は `contents: read` のみに限ります。
