# 0197: CLI と MCP のサーバーは依存を足さずに書き、MCP は 2026-07-28 と前のバージョンの両方に応える

| 項目 | 内容 |
| --- | --- |
| 日付 | 2026-10-31 |
| 状態 | 採用 |

## 背景

- CLI・MCP は Tier 1 で、Rust のバイナリとして出す（[0006](0006-language-tiers.md)・[0011](0011-bindings-through-ffi.md)）。CLI は実行ファイル `zumen` にし、サブコマンド、標準入出力の JSON-RPC（`serve --stdio`）、MCP（`mcp`）を持つ（[0013](0013-boundary-in-four-forms.md)）。配る名前は `zumen`（[0014](0014-platforms-signing-and-distribution.md)）だが、公開の時期と名前は利用者が決める（#317）。
- [0013](0013-boundary-in-four-forms.md) は、MCP に公式の Rust SDK（rmcp）を使うと決めた。rmcp は tokio などの依存が多く、2026 年にメジャーバージョンを 3 回上げた。
- MCP の仕様を、2026-10-31 に公開の文書で確かめた。
  - 最新は 2026-07-28 のバージョン。初期化（`initialize`）をなくし、要求ごとに `_meta` の `io.modelcontextprotocol/protocolVersion` でバージョンを伝える。サーバーは `server/discover` を持たなければならない。バージョンが合わなければ `-32022`（UnsupportedProtocolVersionError）を返す。
  - 結果には `resultType` が要り、`tools/list` には `ttlMs` と `cacheScope` が要る。ツールの一覧は決まった順に返す。状態はつながりに持たせず、サーバーが渡す名前（handle）をツールの引数で受け取る。
  - 前のバージョン（2025-11-25 まで）のクライアントは初期化から始める。両方に応えるサーバーは、`initialize` を受けたら前のバージョンで応えてよい。
- API は足すだけで、どの形態でも応答を書き換えずに運び、適合テストのハッシュが一致しなければならない（[0012](0012-boundary-functions-and-messages.md)・[0015](0015-add-only-boundary-and-version-floors.md)）。
- 手本は Python のラッパー（[0195](0195-python-wrapper.md)）。

## 選択肢

### MCP と JSON-RPC の書き方

1. **rmcp を使う（[0013](0013-boundary-in-four-forms.md) の決定）。** バージョンの変化に追いつきやすい。tokio などの依存と、メジャーバージョンを上げるたびの直しが要る。
2. **自前で書く。** 標準入出力の 1 行に 1 つの JSON-RPC は、`serve --stdio` でどうせ自前で書く。MCP で使うのは、バージョンの確かめ・`server/discover`・`initialize`・`tools/list`・`tools/call` だけで、JSON は `zumen-json` で読み書きできる。バージョンの変化には自分で追いつく。

### 引数の読み方

1. **clap などの crate を使う。** 助けの文や補完を作れる。依存が増える。
2. **標準ライブラリで読む。** サブコマンドと `--フィールド 値` だけなら足りる。

### MCP のセッション

1. **プロセスに 1 つのセッションを持ち、ツールはそれを暗に使う。** 2026-07-28 の、つながりに状態を持たせない考えに合わない。
2. **`session_new` がセッションの名前を返し、ほかのツールは `session` の引数で指す。** 2026-07-28 の handle の形に合う。

## 決定

MCP と JSON-RPC は 2、引数は 2、セッションは 2 を選ぶ。依存は、ワークスペースの crate（`zumen-abi`・`zumen-base`・`zumen-json`）だけにする。[0013](0013-boundary-in-four-forms.md) の「MCP は rmcp を使う」は、この記録で改める。HTTP（hyper・tokio）は、`serve --http` を作るときに決める。

- `bindings/cli` に crate `zumen-cli`（仮の名前、`publish = false`）を置き、実行ファイル `zumen` を作る。コアは同じプロセスで動かす。コアがパニックしたら、`zm_call` と同じく `panicked` と `internal.panic.caught` の応答を返し、そのセッションを閉じる。
- **サブコマンド**
  - `zumen call [--input 名前=ファイル]... [--options 設定] [--blobs フォルダー] <要求のファイル | ->`: 要求をそのまま渡し、応答を標準出力に書く。
  - `zumen <op> [ファイル] [--フィールド 値]...`: よく使う形。ファイルを名前で push して open し、その文書を `doc` に入れて op を呼ぶ。`--フィールド` の値は JSON として読み、読めなければ文字列にする。op の名前は CLI が持たず、コアが判断する。
  - 終了コードは API の状態の値（`ok` が 0、`failed` が 1 など）にし、引数とファイルの誤りは 64 にする。`--blobs` で、応答の blobs を `<id>.<kind>` に書き出す。
- **`zumen serve --stdio`**: API の関数に 1 対 1 で当たるメソッド（`session_new`・`push`・`call`・`take`・`cancel`）を出す。セッションは 1 つで、既定の設定で作ってから始める。バイト列は、文字列・base64・JSON の値（`request`・`options`）・ファイルの道筋（`push` の `path`）で渡せる。応答の JSON は、書き換えないよう文字列のまま `response` に入れる。`cancel` は読む側のスレッドが受け、実行中の `call` を止める。
- **`zumen mcp [--root フォルダー]`**
  - バージョンは 2026-07-28 と、初期化で受ける 2025-11-25・2025-06-18・2025-03-26。`_meta` のバージョンが知らないものなら `-32022` を返す。`_meta` にバージョンがない要求（前のバージョン）は、そのまま受ける。`server/discover`・`initialize`・`ping`・`tools/list`・`tools/call` に応え、`notifications/cancelled` で実行中のツールを止める。
  - ツールは、セッションのツール（`session_new`・`session_close`・`push`・`take`）と、op ごとのツール。op のツールの入力の型は、`schema/messages.json` の `<op>_request` から `op` を除き、共通のフィールドを広げ、`session` を足し、指す定義を `$defs` に入れて閉じたもの。`cargo xtask codegen` が `bindings/cli/src/tools.json` に作り、`--check` が更新漏れを調べる。一覧はスキーマの順に返す。
  - op のツールは、コアの応答の JSON を書き換えずに文字の内容で返し、API の状態を結果の `_meta` の `io.github.zumen-jp/status` に入れる。`ok` でなければ `isError` にする。知らないツールは `-32602` にする。
  - ファイルは `--root` の中だけを、安全な相対の名前（`zumen_base::name::is_safe`）で読み書きする。`--root` がなければ、ファイルを使わない。`take` の書き出しは、まだないファイルに限る。
  - 上限: セッションは 16、1 回の push は 256 MiB、`take` の埋め込みは 16 MiB。図面の文字・名前・属性はデータで指示ではないことを、案内（`instructions`）とツールの説明に書く。
  - セッションの名前は `s1` からの連番にする。標準入出力のサーバーを使うのは、起こしたクライアント 1 つだけだから。
- **適合テスト**: `bindings/cli/tests/conformance.rs` が、実行ファイルを子プロセスで起こして流す。`serve --stdio` はすべての組を、MCP はツールで表せる組（設定を JSON の値で渡し、要求が知っている op のオブジェクトで、入力の名前が文字列のもの）を流し、その行が `hashes.txt` と一致することを確かめる。`cargo test --workspace` に入るので、CI のテストの行列で流れる。

## 影響

- Tier 1 の 3 つの条件（生成・API のラッパー・適合テスト）を、CLI・MCP で満たす。シーンの blob は CLI では読まず、ファイルか base64 のまま渡す。
- MCP のバージョンが変わったら、自分で追う。バージョンの一覧を足し、`server/discover` とツールの結果の形を見直す。
- `serve --http`、子プロセスの形態（`engine` の `process`）、画像の大きさの上限、MCP の `resources` と進み具合の通知は、まだない。要るものと一緒に足す。
- `serve --http`・`process`・MCP の `resources` と進み具合の通知・シーンの blob を読むことは、[0200](0200-cli-http-process-engine-and-mcp-resources.md) で作った。
- 公開するまで、サブコマンド・メソッド・ツールの形は変えてよい。公開してからは、API と同じく足すだけにする。
- 新しい op を足すと、`bindings/cli/src/tools.json` の差分が PR に並ぶ。

## 参照

- [#352](https://github.com/zumen-jp/zumen/issues/352)、[#317](https://github.com/zumen-jp/zumen/issues/317)
- [0006](0006-language-tiers.md)、[0011](0011-bindings-through-ffi.md)、[0012](0012-boundary-functions-and-messages.md)、[0013](0013-boundary-in-four-forms.md)、[0014](0014-platforms-signing-and-distribution.md)、[0015](0015-add-only-boundary-and-version-floors.md)、[0195](0195-python-wrapper.md)
- [JSON-RPC 2.0](https://www.jsonrpc.org/specification)、[MCP 2026-07-28 の変更点](https://modelcontextprotocol.io/specification/2026-07-28/changelog)、[版と互換](https://modelcontextprotocol.io/specification/2026-07-28/basic/lifecycle)、[server/discover](https://modelcontextprotocol.io/specification/2026-07-28/server/discover)、[ツール](https://modelcontextprotocol.io/specification/2026-07-28/server/tools)
- 名前と公開: [0205: バージョン 0.2.0 から、タグを push してワークフローで crates.io・npm・PyPI・NuGet と GitHub の Release に出す](0205-release-and-publishing.md)
