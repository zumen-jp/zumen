# Go のラッパー

wasm のモジュール（`zumen_abi1.wasm`）を [wazero](https://wazero.io/) で動かす薄いラッパーです（[依存とツールの条件](../../AGENTS.md#範囲と権利)）。cgo を使いません。下限は、Go がサポートする 2 つのメジャーバージョン（今は 1.26・1.27）です。

- モジュールは `zumen.dev/go`、パッケージ名は `zumen` です。まだ出していません。
- 依存は wazero 1.12.0（Apache-2.0）と、それが使う `golang.org/x/sys`（BSD-3-Clause）だけです。
- すべての形式を入れた `zumen_abi1.wasm` を `go:embed` で埋め込みます（[API のスキーマ](../../schema/README.md)）。モジュールはコミットしないので、使う前に作って、このフォルダーに写します。
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
| `New`・`NewFromWasm` | 埋め込んだモジュールか、渡したバイト列を読み込み、関数と ABI のメジャーバージョンを確かめる。`Core` は、ほかのゴルーチンから同時に使える |
| `Core.Open`・`OpenBytes` | セッションを作る（`zm_session_new`）。セッションごとにモジュールのインスタンスを作る |
| `Session.Push`・`Call`・`CallBytes`・`CallText`・`Take`・`Cancel`・`Close` | API の関数（`zm_push`・`zm_call`・`zm_take`・`zm_session_close`）。状態が `ok` でなければ `*ZumenError` を返す |
| `ReadScene` | シーンの blob を読む。セクションは `U8`・`Buffer`（写さない）か `U32`・`F32`・`F64`（写す）で引く（`schema/scene.toml`） |

要求と応答の形は [schema/messages.json](../../schema/messages.json) にあります。`messages.go` の型は `json` のタグを持つ構造体で、フィールドはポインターで持ち、ないフィールドは `nil` です。知らないフィールドは読み落とすので、要るときは `CallBytes` で JSON のまま受け取ります。列挙は文字列の型で持ち、値を定数で出すので、知らない値も受け取れます。共用体は `AsXxx` で形ごとの型に移します。op の名前は `OpXxx` の定数です。

1 つのセッションは、一度に 1 つのゴルーチンで使います。呼び出しは、渡したコンテキストか `Cancel`（どのゴルーチンからも呼べる）で止められます。止めた呼び出しは `failed` の `ZumenError` を返し、セッションは閉じます。

`Push`・`PushBytes` は、新しい wasm では入力の領域の所有権をコアへ渡し、最初の入力を wasm の中でもう一度写すことを避けます。渡した Go のバイト列は引き続き使えます。新しい関数がない旧 ABI 1 の wasm では、従来の `zm_push` を使います。追記と文書の解析には別にメモリが必要です（[API のスキーマ](../../schema/README.md)）。

## 子プロセスのエンジン

`OpenProcess`・`OpenProcessBytes` は、CLI を `zumen serve --stdio` で起こし、標準入出力の JSON-RPC でネイティブのコアを呼びます（決定 0201）。wasm の 4 GiB の上限を超える入力や、ネイティブの速さが要るときに使います。子プロセスはセッションごとに 1 つで、シェルを介さず、標準エラー出力は捨てます。

```go
session, err := zumen.OpenProcess(ctx, "/usr/local/bin/zumen", nil, &zumen.ProcessOptions{Timeout: 2 * time.Minute})
if err != nil {
	return err
}
defer session.Close(ctx)
```

`ProcessSession` のメソッドは `Session` と同じ名前・引数で、コアの状態も同じ `*ZumenError` で返します。両方を使い分けるときは、使うメソッドだけのインターフェースを書きます。違いは次のとおりです。

- 起動・通信・上限による失敗は `*ProcessError` で返します。`Code` は `ProcessSpawn`・`ProcessIO`・`ProcessLimit`・`ProcessProtocol`・`ProcessTimeout`・`ProcessClosed`（`spawn`・`io`・`limit`・`protocol`・`timeout`・`closed`）のどれかで、JavaScript の `ProcessError` と同じです。`limit` で要求を送らなかったときのほかは、子プロセスを止め、`Broken` が真になります。後の呼び出しは同じ失敗を返すので、新しいセッションを作ります。コアの `panicked` も同じく子プロセスを止めます。
- `Call` の最中にコンテキストが終わるか `Cancel` を呼ぶと、中断の合図を送ります。コアが中断した呼び出しは `failed` の `ZumenError` を返し（`Err` は、コンテキストの理由か `context.Canceled`）、セッションは使い続けられます。要求を書き終えてから 1 秒で戻らなければ、子プロセスを止めます。コアは `Push`・`Take` を中断しないので、これらのコンテキストは送る前にだけ確かめます。
- `ProcessOptions` の `Timeout` は 1 つの要求の応答を待つ時間（既定 30 秒、負の値は期限なし）、`MaxMessageBytes` は要求と応答の改行を含む 1 行の上限（既定 64 MiB、1024 バイトから 1 GiB）です。入力と応答は base64 で送るので、1 行は元の約 4/3 倍です。
- `Close` は標準入力を閉じて終了を 1 秒待ち、終わらなければ止めます。処理中の呼び出しは `closed` の `ProcessError` を返します。

## テスト

```bash
cargo build -p zumen-abi --target wasm32-unknown-unknown --release
cp target/wasm32-unknown-unknown/release/zumen_abi1.wasm bindings/go/
cd bindings/go && go test ./...
cargo build -p zumen-cli
ZUMEN_CLI="$PWD/../../target/debug/zumen" go test ./...
```

`conformance_test.go` は、[適合テスト](../conformance/README.md)の組をこのラッパーで流し、ハッシュの一覧が `hashes.txt` と一致することを確かめます。`input_test.go` は、新旧の関数の経路、空の入力と追記、失敗時の解放、32 MiB の入力を渡したときのメモリの削減と中断を確かめます。`process_test.go` は、子プロセスのエンジンの中断・上限・通信の失敗を、試験の実行ファイル自身を JSON-RPC の応答器として起こして確かめます。`ZUMEN_CLI` に CLI を渡すと、適合テストの組も子プロセスのエンジンで流します。PR ごとに Go 1.26、毎晩（`nightly.yml`）に Go 1.26 と 1.27 を Linux で流します。

## 3D のシーン

ReadScene3D(blob) で ZMS3 を読みます。InstanceElement と ElementID で建物の要素 ID を引けます。U8 は元の blob を共有し、U32・F32・F64 は配列へコピーします。

同じ blob の 2D 用の読み手との混用はできません。列の配置・座標・単位・色・選択と検査範囲は [3D のシーンの契約](../../schema/README.md#3d-のシーンの契約)に従います。元の blob を共有する読み手の利用中は、そのバイト列を変更しません。
