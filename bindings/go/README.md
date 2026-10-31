# Go の包み

wasm のモジュール（`zumen_abi1.wasm`）を [wazero](https://wazero.io/) で動かす薄い包みです（[決定 0011](../../docs/decisions/0011-bindings-through-ffi.md)・[決定 0226](../../docs/decisions/0226-go-wrapper.md)）。cgo を使いません。下限は、Go がサポートする 2 つの大版（今は 1.26・1.27）です。

- モジュールは `zumen.dev/go`、パッケージ名は `zumen` です。まだ出していません。
- 依存は wazero 1.12.0（Apache-2.0）と、それが使う `golang.org/x/sys`（BSD-3-Clause）だけです。
- すべての形式を入れた `zumen_abi1.wasm` を `go:embed` で埋め込みます（[決定 0221](../../docs/decisions/0221-wasm-modules.md)）。モジュールはコミットしないので、使う前に作って、このフォルダーに写します。
- `messages.go`・`constants.go` は、`schema/` から `cargo xtask codegen` で生成します。手で直さないでください。

## 使い方

```go
import (
	"context"
	"os"

	zumen "zumen.dev/go"
)

ctx := context.Background()
core, err := zumen.New(ctx)
if err != nil {
	return err
}
defer core.Close(ctx)
session, err := core.Open(ctx, nil)
if err != nil {
	return err
}
defer session.Close(ctx)
data, _ := os.ReadFile("a.dxf")
if err := session.Push(ctx, "a.dxf", data); err != nil {
	return err
}
v := zumen.SchemaMajor
response, err := session.Call(ctx, zumen.OpenRequest{V: &v, Input: new("a.dxf")})
if err != nil {
	// failed の応答は (*zumen.ZumenError).Response() にあり、診断を持ちます
	return err
}
opened, _ := zumen.ResultOf[zumen.OpenResult](response)
scene, _ := session.Call(ctx, zumen.SceneRequest{V: &v, Doc: opened.Doc, From: new("space/0")})
blob, _ := session.Take(ctx, uint32(*scene.Blobs[0].Id))
columns, _ := zumen.ReadScene(blob)
origins, _ := columns.F64("chunk_origins")
```

| 名前 | 中身 |
| --- | --- |
| `New`・`NewFromWasm` | 埋め込んだモジュールか、渡したバイト列を読み込み、関数と ABI の大版を確かめる。`Core` は、ほかのゴルーチンから同時に使える |
| `Core.Open`・`OpenBytes` | セッションを作る（`zm_session_new`）。セッションごとにモジュールのインスタンスを作る |
| `Session.Push`・`Call`・`CallBytes`・`CallText`・`Take`・`Cancel`・`Close` | 境界の関数（`zm_push`・`zm_call`・`zm_take`・`zm_session_close`）。状態が `ok` でなければ `*ZumenError` を返す |
| `ReadScene` | シーンの blob を読む。区画は `U8`・`Buffer`（写さない）か `U32`・`F32`・`F64`（写す）で引く（`schema/scene.toml`） |

要求と応答の形は [schema/messages.json](../../schema/messages.json) にあります。`messages.go` の型は `json` のタグを持つ構造体で、欄はポインターで持ち、ない欄は `nil` です。知らない欄は読み落とすので、要るときは `CallBytes` で JSON のまま受け取ります。列挙は文字列の型で持ち、値を定数で出すので、知らない値も受け取れます。共用体は `AsXxx` で形ごとの型に移します。op の名前は `OpXxx` の定数です。

1 つのセッションは、一度に 1 つのゴルーチンで使います。呼び出しは、渡したコンテキストか `Cancel`（どのゴルーチンからも呼べる）で止められます。止めた呼び出しは `failed` の `ZumenError` を返し、セッションは閉じます。子プロセスのエンジン（`engine: "process"`）は、まだありません。CLI の `zumen serve --stdio` を `os/exec` で起こして使えます。

## 試験

```bash
cargo build -p zumen-abi --target wasm32-unknown-unknown --release
cp target/wasm32-unknown-unknown/release/zumen_abi1.wasm bindings/go/
cd bindings/go && go test ./...
```

`conformance_test.go` は、[適合試験](../conformance/README.md)の組をこの包みで流し、ハッシュの一覧が `hashes.txt` と一致することを確かめます。毎晩（`nightly.yml`）、Linux で Go 1.26 と 1.27 を流します。
