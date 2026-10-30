# 0198: C・C++ はヘッダーだけの RAII の包みとシーンの読み手を足し、メッセージは JSON の文字列のまま渡す

| 項目 | 内容 |
| --- | --- |
| 日付 | 2026-10-31 |
| 状態 | 採用 |

## 背景

- C・C++ は階層 1 の言語で、C ABI を生成したヘッダーで呼び、C++ には RAII の薄い包みを付ける（[0006](0006-language-tiers.md)・[0011](0011-bindings-through-ffi.md)）。GitHub Releases で、ヘッダー・ライブラリ・CMake の設定を配る（[0014](0014-platforms-signing-and-distribution.md)）。公開の時期は、利用者が決める（#317）。
- いまは、`schema/boundary.toml` から生成した `bindings/c/include/zumen.h` と、それを C と C++ から呼ぶ小さな試験だけがある。シーンの blob の配置（`schema/scene.toml`）は、C・C++ の定数を生成していなかった（[0137](0137-columnar-scene-transfer.md)・[0178](0178-scene-layout-columns.md)）。
- 下限は C99 と C++17（[docs/support.md](../support.md)）。C++17 には `std::span` と `std::endian` がない。C と C++ の標準ライブラリには、JSON を読むものがない。
- 生成器は xtask に自前で書き、境界の関数の包みは言語ごとに手で書く（[0011](0011-bindings-through-ffi.md)）。手本は Python・.NET の包み（[0195](0195-python-wrapper.md)・[0196](0196-dotnet-wrapper.md)）。

## 選択肢

### メッセージの型

1. **スキーマから C++ の構造体と、JSON の読み書きを生成する。** JSON のライブラリ（nlohmann/json など）に依存するか、読み書きを自前で生成する。利用者が使う JSON のライブラリとぶつかり、ヘッダーが大きくなる。
2. **要求と応答は JSON の文字列のまま渡す。** 利用者が好きな JSON のライブラリで読み書きする。依存がなく、知らない欄・列挙の値で失敗しない（[0015](0015-add-only-boundary-and-version-floors.md)）。

### シーンの読み手

1. **C の関数で読み手を生成する。** C の利用者にも読み手が届くが、手で書く部分を生成器に入れることになる（[0011](0011-bindings-through-ffi.md)）。
2. **区画の番号・型・幅と値の組を C のヘッダーに生成し、読み手は C++ の包みに手で書く。** C の利用者は定数で blob を読める。

### 適合試験の流し方

1. **組の手順の読み方と JSON Pointer の確かめ方を、C と C++ で書き直す。** JSON の読み手を試験に持つことになる。
2. **組の手順は Rust の試験の道具（`zumen-conformance`）が読み、境界の関数は C と C++ の側から呼ぶ。** 状態・応答・バイナリの結果は、C・C++ の側が返したバイト列を、ほかの形と同じ道具で確かめる。

## 決定

メッセージの型は 2、シーンの読み手は 2、適合試験の流し方は 2 を選ぶ。

- `bindings/c/include` に置く。名前は仮とし、配る形（GitHub Releases の zip と CMake の設定、[0014](0014-platforms-signing-and-distribution.md)）は、公開の手順を決めるときに足す。
- **生成するもの**（`cargo xtask codegen`）
  - `zumen.h`: 境界の関数・状態コード・版（いまのまま）。
  - `zumen_scene.h`: シーンの見出しの目印と版、区画の数、区画ごとの番号（`ZM_SCENE_SECTION_*`）、値の型（`ZM_SCENE_TYPE_*`）、列に入れる値（`ZM_SCENE_VALUE_*`）と、番号から区画の名前・型・幅を引く `zm_scene_section_at`。関数は `static inline` にし、ライブラリに関数を足さない。
- **C++ の包み**（`zumen.hpp`、手で書く、ヘッダーだけ）
  - `zumen::Session` がセッションを、`zumen::Bytes` がコアの返したバッファーを持ち、壊すときに `zm_session_close`・`zm_free` で返す。どちらも写せず、動かせる。
  - `Session` は `open`・`push`・`call`・`take`・`cancel`・`close` を持ち、境界の関数（[0012](0012-boundary-functions-and-messages.md)）に 1 つずつ当たる。状態が `ok` でなければ `zumen::Error` を投げ、`failed` の応答は `Error::response` で読める。例外を使わない人のために、状態を返す `try_open`・`try_push`・`try_call`・`try_take` も置く。閉じたセッションへの呼び出しは NULL のまま境界に渡し、境界が `invalid` を返す。長さ 0 の入力にも NULL でない番地を渡す。
  - シーンの blob は `zumen::Scene::read` で読み、区画を `Column<T>` で見せる。blob を写さず、値はリトルエンディアンのバイト列から、環境のバイト順によらず直して返す。番地の揃いも問わない。知らない区画は読み飛ばし、ない区画は空の列にし、大版が違えば `zumen::SceneError` を投げる（[0137](0137-columnar-scene-transfer.md)）。区画と違う型の列を引くと投げる。
- **適合試験**: `bindings/c/tests/conformance.rs` が `bindings/conformance` の組を、C の側（`conformance.c`、ヘッダーだけ）と C++ の側（`conformance.cpp`、包み）を通して流し、状態・応答の値・ハッシュの一覧が `hashes.txt` と一致することを確かめる。C++ の側では、シーンの blob を `zumen::Scene` で読み、応答の数と合うことも確かめる。試験の C と C++ は、警告を失敗にして C99（MSVC は C11）と C++17 で組み立てる。CI の試験の行列（Linux x64・macOS arm64・Windows x64・Windows x86）の `cargo test --workspace` で流れる。

## 影響

- C・C++ について、階層 1 に上げる 3 つの条件（生成のテンプレート・境界の関数の包み・適合試験）を満たす。
- op ごとのメソッドと、メッセージの型は持たない。要るようになったら、JSON のライブラリの選び方と合わせて、この記録を置き換えて決める。
- `zm_take_arrow`（Arrow の C Stream Interface）は `zumen.h` の宣言のまま使い、包みのメソッドにしない。
- CMake の設定と、ヘッダーをライブラリと一緒に配る手順は、公開の手順を決めるときに足す。
- 包みの API は、公開するまで変えてよい。公開してからは、境界と同じく足すだけにする。
- 新しい区画・値を足すと、`zumen_scene.h` の差分が PR に並ぶ。

## 参照

- [#353](https://github.com/zumen-jp/zumen/issues/353)、[#317](https://github.com/zumen-jp/zumen/issues/317)
- [0006](0006-language-tiers.md)、[0011](0011-bindings-through-ffi.md)、[0012](0012-boundary-functions-and-messages.md)、[0014](0014-platforms-signing-and-distribution.md)、[0015](0015-add-only-boundary-and-version-floors.md)、[0137](0137-columnar-scene-transfer.md)、[0178](0178-scene-layout-columns.md)、[0195](0195-python-wrapper.md)、[0196](0196-dotnet-wrapper.md)
