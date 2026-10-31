# CLI と MCP のサーバー

zumen のコアを同じプロセスで動かす実行ファイル `zumen` です（[決定 0013](../../docs/decisions/0013-boundary-in-four-forms.md)・[決定 0197](../../docs/decisions/0197-cli-and-mcp-server.md)・[決定 0200](../../docs/decisions/0200-cli-http-process-engine-and-mcp-resources.md)）。境界と同じ要求と応答（[schema/messages.json](../../schema/messages.json)）を、書き換えずに運びます。

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
| `zumen serve --http [--listen アドレス:ポート]` | 同じ JSON-RPC を HTTP/1.1 で受ける。既定は `127.0.0.1:24180` |
| `zumen mcp [--root フォルダー]` | MCP のサーバー（標準入出力） |
| `zumen scene-info <ファイル \| ->` | シーンの blob の区画ごとの行の数を JSON で書く |

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

## HTTP

`zumen serve --http` は、待ち受けたアドレスを標準出力の 1 行目に書きます。TLS と認証を持たないので、ループバックでないアドレスで待ち受けるときは、認証のある前段の後ろに置きます。

| 要求 | 中身 |
| --- | --- |
| `POST /`（`application/json`） | JSON-RPC 2.0 の 1 つのメッセージ。メソッドは標準入出力と同じで、`session_new` が返す `session` でセッションを指す。`session_close` で閉じる |
| `POST /sessions/<id>/inputs/<名前>`（`application/octet-stream`） | 本文のバイト列を入力に足す |
| `GET /sessions/<id>/blobs/<番号>` | バイナリの結果を本文で返す |

- サーバーのファイルは読みません（`push` の `path` は使えません）。
- ループバックで待ち受けるときは、`Host` がループバックの名前の要求だけを受けます。`Origin` のある要求（ブラウザー）は受けません。
- 上限: セッションは 16、入力の合計は 1 GiB、本文は 256 MiB、同時のつながりは 32。使われない時間が 10 分続いたセッションは捨てます。

## 子プロセスの形

ネイティブのライブラリは、セッションの設定を `{"v":1,"engine":"process"}` にすると、この CLI を `serve --stdio` で子プロセスとして起こし、同じメッセージを送ります。実行ファイルは `process_path` で渡すか、ライブラリと同じフォルダーに置きます。子プロセスのメモリには 8 GiB の上限を付けます（Linux と Windows。macOS では付けられません、[決定 0201](../../docs/decisions/0201-process-engine-memory-limit-and-host-safety.md)）。

## MCP

MCP 2026-07-28（状態を持たない形）と、初期化から始める前の版（2025-11-25・2025-06-18・2025-03-26）に応えます。

- `session_new` でセッションを作り、返った名前をほかの道具の `session` に渡します。`push` で入力を足し、`open` で文書にしてから、`drawing`・`entities`・`scene` などの op の道具を呼びます。`take` は blob を base64 で返すか、`--root` の中の新しいファイルに書きます。
- op の道具は、コアの応答の JSON を文字の内容で返し、境界の状態を結果の `_meta` の `io.github.zumen-jp/status` に入れます。
- ファイルは `--root` の中だけを読み書きします。`--root` がなければ、ファイルを使いません。
- リソースとして、メッセージのスキーマ（`zumen://schema/messages.json`）とカタログ（`zumen://catalog/ja.json`・`en.json`）を出します。blob は `zumen://<セッション>/blobs/<番号>` で読め、次の op までは何回でも読めます。
- 要求の `_meta` に `progressToken` があれば、道具が終わるまで 5 秒ごとに進み具合を通知します。

クライアントの設定の例です。

```json
{ "mcpServers": { "zumen": { "command": "/path/to/zumen", "args": ["mcp", "--root", "/path/to/drawings"] } } }
```

## 試験

```bash
cargo test -p zumen-cli
```

`tests/conformance.rs` は、[適合試験](../conformance/README.md)の組を `zumen serve --stdio`・`zumen serve --http`・`zumen mcp`・子プロセスの形を通して流し、ハッシュの一覧が `hashes.txt` と一致することを確かめます。MCP は、道具で表せる組だけを流します。
