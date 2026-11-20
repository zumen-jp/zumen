# Tier 3 の言語から使う

[Tier 3](support.md#言語ごとの約束) では、公式のラッパーを使わずに図面を開く 4 つの経路を、Ruby・PHP・PowerShell のサンプルで確かめます。図面に対する要求と応答には、どの経路でも[同じ JSON](../schema/messages.json) を使います。

| 形態 | 向く場面 | サンプル |
| --- | --- | --- |
| C ABI | 同じプロセスで共有ライブラリを呼ぶ | [Ruby の Fiddle](../examples/tier3/ruby/c_abi.rb) |
| CLI | 子プロセスの標準入出力で要求と応答をやり取りする | [PHP の proc_open](../examples/tier3/php/stdio.php) |
| wasm（WASI） | WASI のランタイムで CLI を動かす | 同じ PHP のサンプルで [Node.js の WASI ランタイム](../bindings/conformance/wasi.mjs)を起動 |
| HTTP | 別プロセスのサーバーに HTTP で要求を送る | [PowerShell の Invoke-RestMethod](../examples/tier3/powershell/http.ps1) |

[毎晩の CI](../.github/workflows/nightly.yml) では、各経路で[同じ DXF](../samples/dwg/zumen-layout-r2000_ascii.dxf) を開き、形式と図形の数を[共通の期待値](../examples/tier3/expected.txt) と照らします。CLI・WASI・HTTP の応答とバイナリの結果は、[適合テスト](../bindings/conformance/cli.mjs)で確かめます。

CLI と C ABI の共有ライブラリは [GitHub Release](https://github.com/zumen-jp/zumen/releases) でも配っています。下のサンプルは、このリポジトリでビルドしたもの（`target/release`）を使う手順です。

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
- `zm_buf` は、ポインターと `size_t` の 2 つのフィールドの構造体です。FFI では、この大きさの領域を確保して渡し、読んだあとに `zm_free` に渡します。
- 関数は各環境の 64 bit の標準 C 呼び出し規約です。
- 1 つのセッションを、同時に 2 つのスレッドから呼ばないでください。呼ぶと busy を返します。

## CLI と `process`

`cargo build -p zumen-cli --release` で、`target/release/zumen` ができます（[bindings/cli/README.md](../bindings/cli/README.md)）。

- 1 回だけ呼ぶなら、`zumen call --input <名前>=<ファイル> <要求の JSON のファイル>`。応答の JSON を標準出力に書き、終了コードが状態コードです。1 回が 1 つのセッションです。
- 続けて呼ぶなら、`zumen serve --stdio` を子プロセスで起こし、1 行に 1 つの JSON-RPC 2.0 のメッセージを送ります。メソッドは `session_new`・`push`（`name` と `base64` か `text`）・`call`（`request` に要求の JSON の値）・`take`（`blob`）・`cancel` です。答えは `{"status": "ok", "response": "<応答の JSON の文字列>"}` の形で、id のない行は進み具合の通知です。
- ネイティブのライブラリを読み込める言語は、セッションの設定を `{"v": 1, "engine": "process"}` にすると、同じ CLI を子プロセスで使えます（[CLI の使い方](../bindings/cli/README.md)）。

## wasm（WASI）

`cargo build -p zumen-cli --release --target wasm32-wasip1`（`rustup target add wasm32-wasip1` が要る）で、`target/wasm32-wasip1/release/zumen.wasm` ができます。WASI のランタイムで `serve --stdio` を動かせば、CLI と同じ JSON-RPC で使えます。

```bash
wasmtime run target/wasm32-wasip1/release/zumen.wasm serve --stdio
```

- フォルダーを渡さなくても使えます。入力は push で、バイト列のまま送ります。
- WASI にはスレッドがないので、要求は 1 行ずつ順に処理し、処理の途中の `cancel` は受けません。予算（`budget`）で止めてください。
- Node.js では、[bindings/conformance/wasi.mjs](../bindings/conformance/wasi.mjs) が node:wasi で動かします。毎晩の CI は、これで適合テストとサンプルを流します。
- ブラウザーや、wasm のモジュールを直接読み込める環境では、`wasm32-unknown-unknown` のモジュール（`zumen_abi1.wasm`）を使えます。API の関数は C ABI と同じ名前で、メモリは `zm_alloc`・`zm_dealloc` で確保します（[API のスキーマ](../schema/README.md)）。

## HTTP

`zumen serve --http` は、既定で `127.0.0.1:24180` で待ち受け、待ち受けたアドレスを 1 行目に書きます（`--listen 127.0.0.1:0` で空いたポート）。

| 要求 | 中身 |
| --- | --- |
| `POST /`（`application/json`） | JSON-RPC 2.0 のメッセージ。メソッドは `serve --stdio` と同じで、`session_new` の答えの `session` で、セッションを指す |
| `POST /sessions/<id>/inputs/<名前>`（`application/octet-stream`） | 本文のバイト列を、そのまま入力に足す |
| `GET /sessions/<id>/blobs/<番号>` | バイナリの結果を、そのまま本文で返す |

- TLS と認証は持ちません。ループバックの外で待ち受けるときは、前に TLS と認証を持つサーバーを置いてください。
- ブラウザーが付ける `Origin` のある要求は受けません。ブラウザーからは wasm のモジュールを使います。

## 製品で確かめた利用例

### AutoLISP

[AutoLISP のサンプル](../examples/tier3/autolisp/cli.lsp)は、Windows 版 AutoCAD の `WScript.Shell` から `zumen open` と `zumen entities` を呼び、`jq.exe` で形式と図形の総数を取り出します。`jq.exe` を PATH に置き、リポジトリのルートで次を実行すると、Core Console が公開 DXF の一時コピーを開きます。

```powershell
py -3 examples/tier3/autolisp/verify.py target/release/zumen.exe 'C:\Program Files\Autodesk\AutoCAD 2027\accoreconsole.exe'
```

Windows x64 の AutoCAD 2027（26.0.118.0.0）と jq 1.8.2 で、改行を LF にそろえた出力が `expected.txt` と一致しました。

### MATLAB

ターミナルで `zumen serve --http` を起動します。MATLAB の作業フォルダーをリポジトリのルートにして、[HTTP のサンプル](../examples/tier3/matlab/zumen_http.m)を実行します。

```matlab
addpath('examples/tier3/matlab')
zumen_http('http://127.0.0.1:24180/', 'samples/dwg/zumen-layout-r2000_ascii.dxf')
```

Linux x64 の MATLAB R2025b Update 7 で、出力が `expected.txt` と一致しました。[MATLAB の検証](../.github/workflows/verify-matlab.yml)は、サンプル・入力図面・期待値・検証手順を変える PR と手動実行で、出力を照合します。
