# 階層 3 の言語から使う

公式の包みのない言語（Ruby・PHP・R・Julia・PowerShell・MATLAB・Delphi など、[決定 0006](decisions/0006-language-tiers.md) の階層 3）から zumen を使う手順です。境界は 4 つの形（[決定 0013](decisions/0013-boundary-in-four-forms.md)）で、どれも同じ要求と応答（[schema/messages.json](../schema/messages.json) の JSON）を運びます。言語の FFI・子プロセス・HTTP のどれが使いやすいかで、形を選びます。

| 形 | 向く場面 | サンプル |
| --- | --- | --- |
| C ABI | 言語に FFI がある。同じプロセスで速く呼びたい | [Ruby（Fiddle）](../examples/tier3/ruby/c_abi.rb) |
| CLI と `process` | 子プロセスを起こせる。ライブラリを読み込みたくない | [PHP（proc_open）](../examples/tier3/php/stdio.php) |
| wasm（WASI） | ネイティブのライブラリを置けない。WASI のランタイムがある | PHP のサンプルを、WASI のランタイムの上の CLI で動かす |
| HTTP | 言語から HTTP だけを使える。別のマシンやコンテナーに置きたい | [PowerShell（Invoke-RestMethod）](../examples/tier3/powershell/http.ps1) |

サンプルは、同じ図面（[samples/dwg/zumen-layout-r2000_ascii.dxf](../samples/dwg/zumen-layout-r2000_ascii.dxf)）を開き、形式と図形の数（[examples/tier3/expected.txt](../examples/tier3/expected.txt)）を書きます。毎晩の CI（`nightly.yml` の `tier3`）が流し、書いたものが `expected.txt` と同じかを確かめます。言語に依らない適合試験は、[bindings/conformance/cli.mjs](../bindings/conformance/cli.mjs) が同じ形で流します。

成果物はまだ配っていないので、どの形も、このリポジトリで作ったものを使います（配ったあとの手順は、公開のときに直します）。

## 共通の流れ

1. セッションを作る。
2. 入力（図面のバイト列）に名前をつけて足す（push）。コアはファイルを読まないので、ホストが読んで渡します。
3. 要求の JSON（`{"v": 1, "op": "open", "input": "<名前>"}` など）を送り、応答の JSON を受ける。応答の `diagnostics` に、読めなかったものや近似したものが入ります。
4. シーンなどのバイナリの結果は、応答の `blobs` の番号で取り出す（take）。
5. セッションを閉じる。

## C ABI

`cargo build -p zumen-abi --release` で、`target/release` に `libzumen_abi1.so`（Linux）・`libzumen_abi1.dylib`（macOS）・`zumen_abi1.dll`（Windows）ができます。関数の形は [bindings/c/include/zumen.h](../bindings/c/include/zumen.h) にあります。

| 関数 | 中身 |
| --- | --- |
| `zm_session_new(options, len, &session)` | セッションを作る。設定が要らなければ `NULL` と 0 |
| `zm_push(session, name, name_len, data, data_len)` | 入力を足す |
| `zm_call(session, request, len, &buf)` | 要求を送る。応答の JSON が `buf`（`ptr` と `len`）に入る |
| `zm_take(session, blob, &buf)` | バイナリの結果を取り出す |
| `zm_free(&buf)` | コアが返したバッファーを返す |
| `zm_session_close(session)` | セッションを閉じる |

- 状態コードは 0 が ok、1 が failed、2 が invalid、3 が busy、4 が panicked です。failed と panicked でも、`buf` に応答があります。
- `zm_buf` は、ポインターと `size_t` の 2 つの欄の構造体です。FFI では、この大きさの領域を確保して渡し、読んだあとに `zm_free` に渡します。
- 32 bit の Windows（32 bit の Office の VBA など）では、関数は stdcall です。ほかの環境は、その環境の C の呼び出し規約です。
- 1 つのセッションを、同時に 2 つのスレッドから呼ばないでください。呼ぶと busy を返します。

## CLI と `process`

`cargo build -p zumen-cli --release` で、`target/release/zumen` ができます（[bindings/cli/README.md](../bindings/cli/README.md)）。

- 1 回だけ呼ぶなら、`zumen call --input <名前>=<ファイル> <要求の JSON のファイル>`。応答の JSON を標準出力に書き、終了コードが状態コードです。1 回が 1 つのセッションです。
- 続けて呼ぶなら、`zumen serve --stdio` を子プロセスで起こし、1 行に 1 つの JSON-RPC 2.0 のメッセージを送ります。メソッドは `session_new`・`push`（`name` と `base64` か `text`）・`call`（`request` に要求の JSON の値）・`take`（`blob`）・`cancel` です。答えは `{"status": "ok", "response": "<応答の JSON の文字列>"}` の形で、id のない行は進み具合の通知です。
- ネイティブのライブラリを読み込める言語は、セッションの設定を `{"v": 1, "engine": "process"}` にすると、同じ CLI を子プロセスで使えます（[決定 0200](decisions/0200-cli-http-process-engine-and-mcp-resources.md)）。

## wasm（WASI）

`cargo build -p zumen-cli --release --target wasm32-wasip1`（`rustup target add wasm32-wasip1` が要る）で、`target/wasm32-wasip1/release/zumen.wasm` ができます。WASI のランタイムで `serve --stdio` を動かせば、CLI と同じ JSON-RPC で使えます。

```bash
wasmtime run target/wasm32-wasip1/release/zumen.wasm serve --stdio
```

- フォルダーを渡さなくても使えます。入力は push で、バイト列のまま送ります。
- WASI にはスレッドがないので、要求は 1 行ずつ順に処理し、処理の途中の `cancel` は受けません。予算（`budget`）で止めてください。
- Node.js では、[bindings/conformance/wasi.mjs](../bindings/conformance/wasi.mjs) が node:wasi で動かします。毎晩の CI は、これで適合試験とサンプルを流します。
- ブラウザーや、wasm のモジュールを直接読み込める環境では、`wasm32-unknown-unknown` のモジュール（`zumen_abi1.wasm`）を使えます。境界の関数は C ABI と同じ名前で、メモリは `zm_alloc`・`zm_dealloc` で確保します（[決定 0013](decisions/0013-boundary-in-four-forms.md)）。

## HTTP

`zumen serve --http` は、既定で `127.0.0.1:24180` で待ち受け、待ち受けたアドレスを 1 行目に書きます（`--listen 127.0.0.1:0` で空いたポート）。

| 要求 | 中身 |
| --- | --- |
| `POST /`（`application/json`） | JSON-RPC 2.0 のメッセージ。メソッドは `serve --stdio` と同じで、`session_new` の答えの `session` で、セッションを指す |
| `POST /sessions/<id>/inputs/<名前>`（`application/octet-stream`） | 本文のバイト列を、そのまま入力に足す |
| `GET /sessions/<id>/blobs/<番号>` | バイナリの結果を、そのまま本文で返す |

- TLS と認証は持ちません。ループバックの外で待ち受けるときは、前に TLS と認証を持つサーバーを置いてください。
- ブラウザーが付ける `Origin` のある要求は受けません。ブラウザーからは wasm のモジュールを使います。
