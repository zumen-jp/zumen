# 0225: Swift の包みは静的ライブラリの XCFramework を module map で呼び、型は JSON の表を包む構造体として生成し、毎晩 macOS と iOS のシミュレーターで流す

| 項目 | 内容 |
| --- | --- |
| 日付 | 2026-10-31 |
| 状態 | 採用 |

## 背景

- Swift は、階層 2 の言語である。生成のテンプレートと境界の関数の包みを持ち、適合試験を毎晩流す（[0006](0006-language-tiers.md)・[0011](0011-bindings-through-ffi.md)）。
- 下限は iOS・iPadOS 15 と macOS 12 にする（[docs/support.md](../support.md)）。SwiftPM の `binaryTarget` は、Apple のプラットフォームでだけ使える。
- iOS のアプリは、自前の動的ライブラリを `dlopen` で読めない。ライブラリはアプリに静的にリンクするか、フレームワークに入れて署名する。
- 手本は Python・.NET・Java の包み（[0195](0195-python-wrapper.md)・[0196](0196-dotnet-wrapper.md)・[0224](0224-java-kotlin-wrapper.md)）。境界は足すだけで、包みは知らない欄と列挙の値も受け取る（[0015](0015-add-only-boundary-and-version-floors.md)）。
- SwiftPM の鏡のリポジトリ（`zumen-swift`）に出すことは、段階 2 の別の子 issue で扱う。

## 選択肢

### ネイティブのライブラリの形

1. **動的ライブラリを XCFramework に入れる。** iOS では、フレームワークの形と署名が要る。
2. **静的ライブラリ（`staticlib`）を XCFramework に入れる。** ヘッダーと module map を添えると、Swift から `import CZumen` で呼べる。アプリに静的にリンクされ、署名はアプリの署名で済む。

### 型の書き方

1. **`Codable` の構造体に読み直す。** 知らない欄を落とす。
2. **読んだ JSON のオブジェクト（`[String: JSON]`）を包む構造体を生成する。** 欄ごとに読み書きするプロパティを持つ。知らない欄も残る。

### シミュレーターでの流し方

1. **XCTest のテストの対象を、Xcode のプロジェクトで流す。** プロジェクトの設定が増える。
2. **`swiftc` でシミュレーターに向けて実行ファイルを作り、`xcrun simctl spawn` で流す。** macOS と同じ試験の実行ファイルを、そのまま流せる。

## 決定

ネイティブのライブラリの形は 2、型の書き方は 2、シミュレーターでの流し方は 2 を選ぶ。

- **置き場所。** 包みを `bindings/swift` に置き、SwiftPM のパッケージにする。ライブラリは `Zumen`、ネイティブの部分は `CZumen` の `binaryTarget`。
- **XCFramework**（`build-xcframework.sh`）。コアを `staticlib` で、macOS（arm64・x86_64）・iOS（arm64）・iOS のシミュレーター（arm64・x86_64）に向けて作り、プラットフォームごとに `lipo` で 1 つにして、`xcodebuild -create-xcframework` でまとめる。ヘッダーは `bindings/c/include` の `zumen.h` と `zumen_scene.h` を写し、`module.modulemap` を添える。
- **依存。** Foundation と、試験の SHA-256 に使う CryptoKit だけを使う。JSON の小さな読み書き（`JSON`）を、包みの中に持つ。Foundation の `JSONSerialization` は真偽値と数の区別があいまいで、64 bit の整数を写せないため使わない。
- **形**
  - `Session` が `push`・`call`・`callData`・`callText`・`take`・`cancel`・`close` を持ち、境界の関数（[0012](0012-boundary-functions-and-messages.md)）に 1 つずつ当たる。作るときに ABI の大版を確かめ、`deinit` で閉じる。
  - コアが返したバッファーは、写したあとに包みの中で `zm_free` で返す。長さ 0 の入力にも NULL でない番地を渡す。
  - 状態が `ok` でなければ `ZumenError` を投げる。`failed` の応答も、`response` で読める。閉じたセッションへの呼び出しは NULL のまま境界に渡し、境界が `invalid` を返す。`cancel` は、ほかのスレッドから呼べる。`cancel` と `close` はロックで順に並べる。
  - シーンの blob は `Scene` で読む。区画は `u8`・`u32`・`f32`・`f64`・`buffer`（どれも写す）で引く。知らない区画は読み飛ばし、ない区画は空の列にし、大版が違えば読まない（[0137](0137-columnar-scene-transfer.md)）。
- **生成するもの**（`cargo xtask codegen`）
  - `Messages.swift`: `schema/messages.json` の定義ごとの構造体（`Message` に従う）。欄はプロパティで読み書きし（ない欄と形の違う値は `nil`、`nil` を書くと欄を除く）、名前はキャメルケースにする。Swift の語と重なる名前はバッククォートで囲み、`Message` のメンバーと重なる名前には `Field` を付ける。目印の欄（`op` など）は、作るときに入れる。列挙は文字列の定数、共用体は形ごとの型に移す `asXxx`、op の名前は `Operations` に置く。名前が重なれば、生成を止める。
  - `Constants.swift`: 版、状態コード（`Status` と名前）、シーンの区画の定義と値の組。
- **適合試験**
  - `ZumenConformance` が、`bindings/conformance` の組を包みのメソッドで流す。状態・応答の値・ハッシュの一覧が `hashes.txt` と一致することと、シーンの blob を読み手で読んで応答の数と合うことを確かめる。
  - 毎晩（`nightly.yml`）、macOS の arm64 で、配るのと同じ対象の XCFramework を作り、`swift run` で流す。続けて `test-simulator.sh` が、iOS 15 に向けて作った試験を iPhone のシミュレーターで流す。

## 影響

- 階層 2 の 3 つの条件（生成のテンプレート・境界の関数の包み・毎晩の適合試験）を、この PR で満たす。
- `Messages.swift` は大きい（約 7,600 行）。新しい op・欄を足すと、その差分が PR に並ぶ。
- XCFramework はコミットせず、作るたびに作る。鏡のリポジトリに出す形（URL の `binaryTarget` とチェックサム）は、鏡のリポジトリの issue で決める。
- iOS の実機、x86_64 の Mac、Mac Catalyst・visionOS・watchOS・tvOS は、試験していない。Mac Catalyst などは、対象を足すときに決める。
- `zm_take_arrow`（Arrow の C Stream Interface）は、包みのメソッドにしない。
- 包みの API と生成した型は、公開するまで変えてよい。公開してからは、境界と同じく足すだけにする。

## 参照

- [0006](0006-language-tiers.md)、[0011](0011-bindings-through-ffi.md)、[0012](0012-boundary-functions-and-messages.md)、[0014](0014-platforms-signing-and-distribution.md)、[0015](0015-add-only-boundary-and-version-floors.md)、[0137](0137-columnar-scene-transfer.md)、[0195](0195-python-wrapper.md)、[0196](0196-dotnet-wrapper.md)、[0224](0224-java-kotlin-wrapper.md)
- [docs/support.md](../support.md)、[bindings/swift](../../bindings/swift/README.md)
- #431
