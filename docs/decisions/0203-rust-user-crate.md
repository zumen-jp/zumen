# 0203: Rust の利用者向けの入口は、zumen-abi を包む `zumen` の crate にし、シーンの読み手を持たせる

| 項目 | 内容 |
| --- | --- |
| 日付 | 2026-10-31 |
| 状態 | 採用 |

## 背景

- [0017](0017-format-rights-ledger-and-plugin-sdk.md) は、Rust の利用者が使う配布物を crates.io の `zumen` とし、形式を書くプラグインの SDK を `zumen-sdk` とした。
- `zumen-abi` は `zumen-sdk` に依存するので、`zumen-sdk` から境界を呼べない。
- メッセージの型は `zumen_abi1::messages` に生成済みで、適合試験は `zumen_abi1::Session` を直接呼んでいた（[0011](0011-bindings-through-ffi.md)・[0012](0012-boundary-functions-and-messages.md)）。ほかの階層 1 の言語の包み（[0194](0194-typescript-javascript-wrapper.md)・[0195](0195-python-wrapper.md)・[0196](0196-dotnet-wrapper.md)・[0198](0198-c-and-cpp-wrapper.md)）は、シーンの blob の読み手を持つ。

## 選択肢

1. 新しく `zumen` の crate を作り、`zumen-abi` を包み、シーンの読み手を持たせる。
2. `zumen-sdk` を組み替えて、境界を呼べるようにする。

## 決定

選択肢 1 を採る（利用者の判断）。0017 の名前にそのまま合い、プラグインの SDK の依存を変えない。

- `crates/zumen` に置く。lib の名前は `zumen`。依存は `zumen-abi`（rlib）・`zumen-base`・`zumen-json`・`zumen-scene` だけで、unsafe を使わない。`rust-version` は階層 1 の下限（[support.md](../support.md)）の 1.99。
- **形。**
  - `Session` が境界の関数に 1 つずつ当たる。`open`・`with_options`・`open_bytes` で作り、`push`・`call`・`call_with`（`CancelToken` で別のスレッドから止める）・`call_bytes`・`take` を持つ。落とすと閉じる。
  - `call` は `messages::Request` を受け取り、`Response`（`v`・`result`・`diagnostics`・`blobs`）を返す。`result` は JSON の値で持ち、`result_as` で op の結果の型に読む。応答の知らない欄は読み飛ばす（[0015](0015-add-only-boundary-and-version-floors.md)）。
  - 状態が `ok` でなければ `Error`（`status` と応答のバイト列）を返す。`failed` の応答は `Error::response` で読める。
  - `messages`・`Status`・`CancelToken`・`json`（`zumen_json`）を出し直す。
- **シーンの読み手。** `scene::read_scene` が blob を読み、区画ごとの列（`Column` の `U8`・`U32`・`F32`・`F64`）を返す。区画の番号と定義は `zumen_scene::layout`（`schema/scene.toml` から生成）を `scene::layout` として出し直す。数はリトルエンディアンから読み、blob から写す。知らない区画は読み飛ばし、ない区画は空の列にし、大版が違えば読まない。範囲と掛け算を確かめ、壊れた blob でもパニックしない。
- **適合試験。** `bindings/conformance` の `Wrapper` が、この crate の `Session` で組を流し、`hashes.txt` と一致することと、シーンの blob を読み手で読んで応答の数と合うことを確かめる。

## 影響

- crates.io への公開（名前・時期）は、公開の手順（#317）で扱う。今はほかの crate と同じく `publish = false`。
- 子プロセスのエンジン（`process`）は、`SessionOptions` の `engine` でそのまま選べる（[0200](0200-cli-http-process-engine-and-mcp-resources.md)）。

## 参照

- [0011](0011-bindings-through-ffi.md)、[0012](0012-boundary-functions-and-messages.md)、[0017](0017-format-rights-ledger-and-plugin-sdk.md)、[0137](0137-columnar-scene-transfer.md)、#356
