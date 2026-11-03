# 0195: Python のラッパーは標準の ctypes と生成した TypedDict で書き、名前を仮にして公開しない

| 項目 | 内容 |
| --- | --- |
| 日付 | 2026-10-31 |
| 状態 | 採用 |

## 背景

- Python は Tier 1 の言語で、C ABI を標準ライブラリの ctypes で呼び、呼び出しの間は GIL を手放す（[0006](0006-language-tiers.md)・[0011](0011-bindings-through-ffi.md)）。旧版の PyO3 のバインディングは移さない。
- 生成器は xtask に自前で書き、生成したコードはコミットして `cargo xtask codegen --check` で更新漏れを調べる（[0011](0011-bindings-through-ffi.md)）。API は足すだけで、ラッパーは知らない列挙の値も受け取る（[0015](0015-add-only-boundary-and-version-floors.md)）。
- PyPI では、言語のバージョンによらないプラットフォームごとの wheel に、ctypes のラッパーとネイティブのライブラリを入れて配る（[0014](0014-platforms-signing-and-distribution.md)）。公開の時期とパッケージの名前は、利用者が決める（#317）。
- Python の下限は 3.9 で、PR ごとに 3.9 と最新で確かめる（[docs/support.md](../support.md)）。32 bit の Windows では、API の関数を stdcall で出す。
- 手本は TypeScript・JavaScript のラッパー（[0194](0194-typescript-javascript-wrapper.md)）。

## 選択肢

### ネイティブの呼び方

1. **cffi。** 宣言を C のヘッダーから読めるが、依存が増え、ABI のモードでも cffi の wheel が要る。
2. **標準ライブラリの ctypes。** 依存がなく、Rhino・Dynamo などの組み込みの Python でもそのまま動く。関数の形は、生成した表で宣言する。

### 型の書き方

1. **dataclass などのクラスに読み直す。** 使いやすいが、応答をすべて写し、知らないフィールドを落とす。生成するコードが大きくなる。
2. **`TypedDict`・`Literal`・`Union` で、JSON の dict にそのまま型を付ける。** 写さず、知らないフィールドも残る。Python 3.9 では `NotRequired` がないので、欠かせないフィールドとなくてよいフィールドを 2 つの `TypedDict` に分けて継ぐ。フィールドの名前に `from` などの予約語があるので、関数の形で書く。

## 決定

ネイティブの呼び方は 2、型の書き方は 2 を選ぶ。

- `bindings/python` にラッパーを置く。名前は仮（配る名前 `zumen-provisional`、モジュール `zumen_provisional`）とし、`pyproject.toml` に PyPI が受け付けない分類 `Private :: Do Not Upload` を付ける。build-system と wheel の作り方は、公開の手順（[0014](0014-platforms-signing-and-distribution.md)）を決めるときに足す。バージョンはコアと同じにし、xtask が確かめる（[0015](0015-add-only-boundary-and-version-floors.md)）。
- 実行時と開発の依存を持たない。
- **形**
  - `load` で、利用者が渡した道筋からライブラリを読み込み、関数がそろうことと ABI のメジャーバージョンを確かめる。32 bit の Windows では `WinDLL`（stdcall）で読む。
  - `Zumen.open` でセッションを作る。`Session` は `push`・`call`・`call_bytes`・`take`・`cancel`・`close` を持ち、API の関数（[0012](0012-boundary-functions-and-messages.md)）に 1 つずつ当たる。`with` の文で閉じる。コアが返したバッファーは、写したあとにラッパーの中で `zm_free` で返す。
  - 状態が `ok` でなければ `ZumenError` を投げる。`failed` の応答も、エラーの `response` で読める。閉じたセッションへの呼び出しは NULL のまま API に渡し、API が `invalid` を返す。
  - シーンの blob は `read_scene` で読み、セクションごとに 1 次元の `memoryview` を返す。環境がリトルエンディアンなら blob のバイト列を写さずに共有し、そうでなければ `array` に写して並びを直す。知らないセクションは読み飛ばし、ないセクションは空の列にし、メジャーバージョンが違えば読まない（[0137](0137-columnar-scene-transfer.md)）。
- **生成するもの**（`cargo xtask codegen`）
  - `zumen_provisional/messages.py`: `schema/messages.json` の定義ごとの型。列挙は `Union[Literal[...], str]` にして、知らない値も受け取る。応答の `result` は `Dict[str, Any]` とし、`OPERATIONS` で op から要求と結果の型の名前を引く。3.9 には総称の `TypedDict` がないため。
  - `zumen_provisional/constants.py`: バージョン、状態コードの名前、ネイティブのライブラリが出す関数の ctypes の形、シーンのセクションの定義と値の組。
- **適合テスト**: `bindings/conformance` の組をラッパーのメソッドで流し、状態・応答の値・ハッシュの一覧が `hashes.txt` と一致することと、シーンの blob をリーダーで読んで応答の数と合うことを、CI のテストの行列（Linux x64・macOS arm64・Windows x64・Windows x86）で確かめる。Python は、Linux x64 と Windows x86 で 3.9、macOS arm64 と Windows x64 で最新を使う。

## 影響

- Tier 1 に上げる 3 つの条件（生成のテンプレート・API の関数のラッパー・適合テスト）を、この PR で満たす。
- 型の静的な検査（mypy・pyright）は CI で走らせていない。ツールを入れるときに足す。
- `zm_take_arrow`（Arrow の C Stream Interface）は宣言だけを持ち、ラッパーのメソッドにしない。pyarrow などとつなぐときに足す。
- ラッパーの API と生成した型は、公開するまで変えてよい。公開してからは、API と同じく足すだけにする。
- 新しい op・フィールド・セクションを足すと、`bindings/python/zumen_provisional` の生成したファイルの差分が PR に並ぶ。

## 参照

- [#350](https://github.com/zumen-jp/zumen/issues/350)、[#317](https://github.com/zumen-jp/zumen/issues/317)
- [0006](0006-language-tiers.md)、[0007](0007-single-rust-implementation.md)、[0011](0011-bindings-through-ffi.md)、[0012](0012-boundary-functions-and-messages.md)、[0014](0014-platforms-signing-and-distribution.md)、[0015](0015-add-only-boundary-and-version-floors.md)、[0137](0137-columnar-scene-transfer.md)、[0194](0194-typescript-javascript-wrapper.md)
- 名前と公開: [0205: バージョン 0.2.0 から、タグを push してワークフローで crates.io・npm・PyPI・NuGet と GitHub の Release に出す](0205-release-and-publishing.md)
