# 0226: Go の包みは wasm のモジュールを wazero で動かし、セッションごとにインスタンスを作り、取り消しはコンテキストで止め、process はまだ作らない

| 項目 | 内容 |
| --- | --- |
| 日付 | 2026-10-31 |
| 状態 | 採用 |

## 背景

- Go は、階層 2 の言語である。生成のテンプレートと境界の関数の包みを持ち、適合試験を毎晩流す（[0006](0006-language-tiers.md)・[0011](0011-bindings-through-ffi.md)）。Go は wasm のモジュールを wazero で動かし、cgo を使わない（[0013](0013-boundary-in-four-forms.md)・[0014](0014-platforms-signing-and-distribution.md)）。
- 下限は、Go がサポートする 2 つの大版（今は 1.26・1.27）にする（[docs/support.md](../support.md)）。
- [0221](0221-wasm-modules.md) は、モジュールを形式ごとに分け、Go にはすべてを入れたモジュールを埋め込むとした。release のモジュールは約 6 MiB で、Go のモジュールの zip の上限（500 MiB）に十分収まる。
- wasm のモジュールは 1 つのスレッドで動く。コアの `zm_cancel` は、ほかのスレッドから同時に呼ぶことを前提にしており、wasm の中では同時に呼べない。
- 手本は Python・.NET・Java・Swift の包み（[0195](0195-python-wrapper.md)・[0196](0196-dotnet-wrapper.md)・[0224](0224-java-kotlin-wrapper.md)・[0225](0225-swift-wrapper.md)）。境界は足すだけで、包みは知らない欄と列挙の値も受け取る（[0015](0015-add-only-boundary-and-version-floors.md)）。
- `zumen.dev/go`（鏡のリポジトリ `zumen-go`）に出すことは、段階 2 の別の子 issue で扱う。

## 選択肢

### 取り消しの形

1. **1 つのインスタンスを全セッションで分け、`zm_cancel` を呼ぶ。** 動いている呼び出しと同時には呼べないので、取り消しが効かない。
2. **セッションごとにインスタンスを作り、動いている呼び出しをコンテキストで止める。** wazero の `WithCloseOnContextDone` で実行を止める。止めたインスタンスの中の状態は信用できないので、セッションごと閉じる。

### 型の書き方

1. **`json` のタグを持つ構造体を生成する。** スカラーとオブジェクトの欄はポインターにして、ない欄と区別する。知らない欄は読み落とす。
2. **読んだ JSON の表を包む型を生成する。** Go では、欄を読むたびに型を確かめる書き方になり、Go の書き方から離れる。

### `process`（子プロセスのエンジン）

1. **包みの中で、CLI の `serve --stdio` を起こして使う。** 公開の API に、子プロセスの場所と寿命を足す。
2. **まだ作らない。** [0201](0201-process-engine-memory-limit-and-host-safety.md) の Node.js と同じく、使い方が出てから決める。

## 決定

取り消しの形は 2、型の書き方は 1、`process` は 2 を選ぶ。

- **置き場所。** 包みを `bindings/go` に置き、Go のモジュール `zumen.dev/go` にする。パッケージ名は `zumen`。依存は wazero 1.12.0（Apache-2.0）と、それが使う `golang.org/x/sys`（BSD-3-Clause）だけ。
- **モジュールの埋め込み。** すべての形式を入れた `zumen_abi1.wasm` を `go:embed` で埋め込む。モジュールはコミットせず、作ってから `bindings/go` に写す。鏡のリポジトリに出す形は、その issue で決める。
- **形**
  - `New`（埋め込んだモジュール）か `NewFromWasm`（バイト列）で `Core` を作る。関数がそろうこと、import がないこと、ABI の大版を確かめる。`Core` は、ほかのゴルーチンから同時に使える。
  - `Core.Open`（`SessionOptions` の値）か `Core.OpenBytes`（JSON のバイト列）で、セッションごとにモジュールのインスタンスを作る。
  - `Session` が `Push`・`PushBytes`・`Call`・`CallBytes`・`CallText`・`Take`・`Cancel`・`Close` を持ち、境界の関数（[0012](0012-boundary-functions-and-messages.md)）に 1 つずつ当たる。どれも `context.Context` を受け取る。
  - 状態が `ok` でなければ `*ZumenError` を返す。`failed` の応答も `Response()` で読める。閉じたセッションへの呼び出しは `invalid` にする。
  - 取り消しは、呼び出しのコンテキストか `Cancel`（どのゴルーチンからも呼べる）で止める。止めた呼び出しは `failed` の `ZumenError` を返し、`Err` に取り消しの理由（`context.Canceled` など）を持つ。wasm ではコアのパニックが trap になるので、trap は `panicked`（コアの不具合で止まった）にする。どちらも、セッションのインスタンスを閉じる。
  - シーンの blob は `ReadScene` で読む。区画は `U8`・`Buffer`（写さない）か `U32`・`F32`・`F64`（写す）で引く。知らない区画は読み飛ばし、ない区画は空の列にし、大版が違えば読まない（[0137](0137-columnar-scene-transfer.md)）。
- **生成するもの**（`cargo xtask codegen`）
  - `messages.go`: `schema/messages.json` の定義ごとの型。オブジェクトは構造体で、欄は `omitempty` のタグとポインターで持ち、配列・表・任意の値はポインターにしない。目印の欄（`op` など）は、`MarshalJSON` が書く。列挙は文字列の型と値の定数、共用体は JSON のまま持つ型と、形ごとの型に移す `AsXxx`、op の名前は `OpXxx` の定数にする。名前が Go の名前・手書きの型と重なれば、生成を止める。
  - `constants.go`: 版、状態コード（`Status` で始まる定数と `StatusName`）、シーンの区画の定義と値の組。
- **適合試験**
  - `conformance_test.go` が、`bindings/conformance` の組を包みのメソッドで流す。状態・応答の値・ハッシュの一覧が `hashes.txt` と一致することと、シーンの blob を読み手で読んで応答の数と合うことを確かめる。型を渡す `Call` と、取り消しも確かめる。
  - 毎晩（`nightly.yml`）、Linux で Go 1.26 と 1.27 のジョブを流す。
- **`process`。** 包みはまだ作らない。`SessionOptions` の `engine` に `process` を渡すと、Node.js の包みと同じく、wasm のモジュールが `failed` を返す。子プロセスに分けたい使い方が出るまでは、CLI の `zumen serve --stdio` を `os/exec` で起こし、同じ JSON-RPC を送れば同じ結果になる。使い方を待つ issue は #462。

## 影響

- 階層 2 の 3 つの条件（生成のテンプレート・境界の関数の包み・毎晩の適合試験）を、この PR で満たす。
- セッションごとにインスタンスを作るので、セッションを開くたびに、モジュールの線形メモリを新しく確保する。多くのセッションを同時に開くと、その分メモリを使う。
- 取り消したセッションは使えなくなる。続けるには、セッションを開き直して入力を渡し直す。
- `messages.go` は大きい（約 4,100 行）。新しい op・欄を足すと、その差分が PR に並ぶ。
- 構造体は知らない欄を読み落とす。知らない欄も要るときは、`CallBytes` で JSON のまま受け取る。
- 試験は Linux の amd64 だけで流す。wazero は macOS・Windows と arm64 でも動くが、毎晩の行には入れていない。
- 包みの API と生成した型は、公開するまで変えてよい。公開してからは、境界と同じく足すだけにする。

## 参照

- [0006](0006-language-tiers.md)、[0011](0011-bindings-through-ffi.md)、[0012](0012-boundary-functions-and-messages.md)、[0013](0013-boundary-in-four-forms.md)、[0014](0014-platforms-signing-and-distribution.md)、[0015](0015-add-only-boundary-and-version-floors.md)、[0137](0137-columnar-scene-transfer.md)、[0201](0201-process-engine-memory-limit-and-host-safety.md)、[0221](0221-wasm-modules.md)、[0224](0224-java-kotlin-wrapper.md)、[0225](0225-swift-wrapper.md)
- [docs/support.md](../support.md)、[bindings/go](../../bindings/go/README.md)
- #432、#462
