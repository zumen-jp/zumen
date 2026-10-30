# CLI と MCP のサーバー

zumen のコアを同じプロセスで動かす実行ファイル `zumen` です（[決定 0013](../../docs/decisions/0013-boundary-in-four-forms.md)・[決定 0197](../../docs/decisions/0197-cli-and-mcp-server.md)）。境界と同じ要求と応答（[schema/messages.json](../../schema/messages.json)）を、書き換えずに運びます。

- まだ公開しません。crate の名前（`zumen-cli`）は仮で、公開の時期と名前は利用者が決めます（#317）。
- 依存はワークスペースの crate だけです。引数は標準ライブラリで読み、JSON-RPC と MCP も自前で書きます。
- `src/tools.json`（MCP の op の道具の一覧）は、`schema/messages.json` から `cargo xtask codegen` で生成します。手で直さないでください。

## 使い方

```bash
cargo build -p zumen-cli
target/debug/zumen capabilities
target/debug/zumen entities samples/dxf/text_alignments.dxf --limit 2 --fields '["bbox"]'
target/debug/zumen call --input a.dxf=samples/dxf/text_alignments.dxf request.json
```

| コマンド | 中身 |
| --- | --- |
| `zumen call [--input 名前=ファイル]... [--options 設定] [--blobs フォルダー] <要求のファイル \| ->` | 要求の JSON をそのまま渡す |
| `zumen <op> [ファイル] [--欄 値]... [--blobs フォルダー]` | ファイルを open し、その文書を `doc` に入れて op を呼ぶ。`open` と `capabilities` は、それだけを呼ぶ。値は JSON として読み、読めなければ文字列にする |
| `zumen serve --stdio` | 境界の関数を、標準入出力の JSON-RPC 2.0 で出す |
| `zumen mcp [--root フォルダー]` | MCP のサーバー（標準入出力） |

応答の JSON を標準出力に書きます。終了コードは境界の状態の値（0 `ok`・1 `failed`・2 `invalid`・3 `busy`・4 `panicked`）で、引数とファイルの誤りは 64 です。`--blobs` を書くと、応答の blobs を `<フォルダー>/<id>.<kind>` に書き出します。

## 標準入出力の JSON-RPC

1 行に 1 つのメッセージを書きます。メソッドは境界の関数に 1 対 1 で当たり、セッションは 1 つです（既定の設定で作ってから始めます）。

| メソッド | 引数 | 結果 |
| --- | --- | --- |
| `session_new` | `options`（JSON の値）・`options_text`・`options_base64` のどれか。なければ既定 | `status` |
| `push` | `name` か `name_base64` と、`text`・`base64`・`path` のどれか | `status` |
| `call` | `request`（JSON の値）・`request_text`・`request_base64` のどれか | `status` と、応答の JSON を文字列のまま入れた `response` |
| `take` | `blob` | `status` と `base64` |
| `cancel` | なし | 実行中の `call` を止める |

## MCP

MCP 2026-07-28（状態を持たない形）と、初期化から始める前の版（2025-11-25・2025-06-18・2025-03-26）に応えます。

- `session_new` でセッションを作り、返った名前をほかの道具の `session` に渡します。`push` で入力を足し、`open` で文書にしてから、`drawing`・`entities`・`scene` などの op の道具を呼びます。`take` は blob を base64 で返すか、`--root` の中の新しいファイルに書きます。
- op の道具は、コアの応答の JSON を文字の内容で返し、境界の状態を結果の `_meta` の `io.github.zumen-jp/status` に入れます。
- ファイルは `--root` の中だけを読み書きします。`--root` がなければ、ファイルを使いません。

クライアントの設定の例です。

```json
{ "mcpServers": { "zumen": { "command": "/path/to/zumen", "args": ["mcp", "--root", "/path/to/drawings"] } } }
```

## 試験

```bash
cargo test -p zumen-cli
```

`tests/conformance.rs` は、[適合試験](../conformance/README.md)の組を `zumen serve --stdio` と `zumen mcp` を通して流し、ハッシュの一覧が `hashes.txt` と一致することを確かめます。MCP は、道具で表せる組だけを流します。
