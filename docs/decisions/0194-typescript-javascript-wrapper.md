# 0194: TypeScript・JavaScript のラッパーは素の ESM と生成した .d.ts で書き、名前を仮にして公開しない

| 項目 | 内容 |
| --- | --- |
| 日付 | 2026-10-31 |
| 状態 | 採用 |

## 背景

- TS・JS は Tier 1 の言語で、wasm のモジュールを、ブラウザーと Node.js の標準の WebAssembly の API で呼ぶ（[0006](0006-language-tiers.md)・[0011](0011-bindings-through-ffi.md)）。上の Tier へ上げる条件は、生成のテンプレート・API の関数のラッパー・適合テストに通ることの 3 つ。
- 生成器は xtask に自前で書き、生成したコードはコミットして `cargo xtask codegen --check` で更新漏れを調べる（[0011](0011-bindings-through-ffi.md)）。生成器とテンプレートは寛容なライセンスに限る。
- API は足すだけで、生成したラッパーは知らない列挙の値も受け取る（[0015](0015-add-only-boundary-and-version-floors.md)）。
- シーンの blob の配置は `schema/scene.toml` にあり、今は Rust の定数だけを生成していた（[0137](0137-columnar-scene-transfer.md)）。
- 公開の時期とパッケージの名前は、利用者が決める（#317）。npm の配る場所は `@zumen-jp/*` とした（[0014](0014-platforms-signing-and-distribution.md)）。
- Node.js の下限は 22、ブラウザーの下限は Safari 15 など（[docs/support.md](../support.md)）。

## 選択肢

### ラッパーの書き方

1. **TypeScript で書き、JS と .d.ts に変換して配る。** 変換のツール（TypeScript か bundler）が、ラッパーのビルドと CI の依存になる。
2. **素の ESM の JS を手で書き、型は .d.ts で別に書く。** ビルドがなく、そのまま Node.js とブラウザーで読み込める。型と実装の食い違いは、型のテストとテンプレートのレビューで防ぐ。

### 型の生成

1. **JSON Schema から型を作る既存のツールを使う。** npm の依存が増え、生成器が Cargo の外に出る。`schema/README.md` の書き方の制限と、列挙を開く規則を、ツールの設定で再現する必要がある。
2. **xtask に、Rust の型と同じスキーマの読み方で .d.ts を書く生成器を足す。** 生成器は 1 つのリーダーを共有し、書き方の制限も同じになる。

## 決定

ラッパーの書き方は 2、型の生成は 2 を選ぶ。

- `bindings/js` にラッパーを置く。`package.json` は `"private": true` にし、名前は仮（`@zumen-jp/zumen-provisional`）とする。公開の時期と本当の名前は利用者が決め、決めたときに名前を変えて公開の手順（[0014](0014-platforms-signing-and-distribution.md)）を足す。バージョンはコアと同じにし、xtask が確かめる（[0015](0015-add-only-boundary-and-version-floors.md)）。
- 実行時と開発の npm の依存を持たない。型は、サイトと同じバージョンの TypeScript で CI が確かめる。
- **形**
  - `load`（非同期。コンパイルしたモジュール・バイト列・fetch の応答を受ける）と `loadSync` でモジュールを読み込み、ABI のメジャーバージョンと、出す関数を確かめる。wasm のファイルはラッパーに入れず、利用者が渡す。配り方は公開の手順で決める。
  - `Zumen.open` でセッションを作る。`Session` は `push`・`call`・`callBytes`・`take`・`cancel`・`close` を持ち、API の関数（[0012](0012-boundary-functions-and-messages.md)）に 1 つずつ当たる。メモリの確保と解放はラッパーの中に隠す。
  - 状態が `ok` でなければ `ZumenError` を投げる。`failed` の応答も、エラーの `response` で読める。閉じたセッションへの呼び出しはそのまま API に渡し、API が `invalid` を返す。
  - シーンの blob は `readScene` で読み、セクションごとに型付き配列を返す。境界がそろい、環境がリトルエンディアンなら、blob のバイト列を写さずに共有する。知らないセクションは読み飛ばし、ないセクションは空の列にし、メジャーバージョンが違えば読まない（[0137](0137-columnar-scene-transfer.md)）。
- **生成するもの**（`cargo xtask codegen`）
  - `src/messages.d.ts`: `schema/messages.json` の定義ごとの型。列挙は `(string & {})` を加えて、知らない値も受け取る。応答は結果の型を引数に持ち、`Operations` で op から要求と結果の型を引く。
  - `src/constants.js`・`src/constants.d.ts`: バージョン、状態コードの名前、wasm が出す関数、シーンのセクションの定義と値の組と、セクションごとの型付き配列の型。
- **適合テスト**: `bindings/conformance` の組をラッパーのメソッドで流し、状態・応答の値・ハッシュの一覧が `hashes.txt` と一致することと、シーンの blob をリーダーで読んで応答の数と合うことを、CI の wasm32 の job で確かめる。手順の読み方は、`wasm.mjs` と `cases.mjs` で共有する。

## 影響

- Tier 1 に上げる 3 つの条件のうち、Node.js での適合テストはこの PR で満たす。ブラウザーでの適合テスト（PR ごとに Chromium、毎晩に Firefox・WebKit、[docs/support.md](../support.md)）は、ブラウザーを動かすツールを選ぶときに足す。
- ブラウザーでの適合テストは、[0199](0199-browser-conformance-with-playwright.md) で足した。
- ラッパーの API と生成した型は、公開するまで変えてよい。公開してからは、API と同じく足すだけにする。
- 新しい op・フィールド・セクションを足すと、`bindings/js/src` の生成したファイルの差分が PR に並ぶ。
- Node.js でネイティブの速さかスレッドが要るときは、napi-rs のアドオンを、記録を書いてから足す（[0011](0011-bindings-through-ffi.md)）。

## 参照

- [#349](https://github.com/zumen-jp/zumen/issues/349)、[#317](https://github.com/zumen-jp/zumen/issues/317)
- [0006](0006-language-tiers.md)、[0007](0007-single-rust-implementation.md)、[0011](0011-bindings-through-ffi.md)、[0012](0012-boundary-functions-and-messages.md)、[0014](0014-platforms-signing-and-distribution.md)、[0015](0015-add-only-boundary-and-version-floors.md)、[0137](0137-columnar-scene-transfer.md)
- 名前と公開: [0205: バージョン 0.2.0 から、タグを push してワークフローで crates.io・npm・PyPI・NuGet と GitHub の Release に出す](0205-release-and-publishing.md)
- Node.js の入口と trap のあと: [0259: npm のパッケージに Node.js の入口を足し、trap のあとのインスタンスを使えなくし、ライセンスの文を入れる](0259-javascript-on-node-servers.md)
