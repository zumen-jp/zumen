# 0225: Swift のラッパーは静的ライブラリの XCFramework を module map で呼び、型は JSON の表を包む構造体として生成し、毎晩 macOS と iOS のシミュレーターで流す

| 項目 | 内容 |
| --- | --- |
| 日付 | 2026-10-31 |
| 状態 | 採用 |

## 背景

- Swift は、Tier 2 の言語である。生成のテンプレートと API の関数のラッパーを持ち、適合テストを毎晩流す（[0006](0006-language-tiers.md)・[0011](0011-bindings-through-ffi.md)）。
- 下限は iOS・iPadOS 15 と macOS 12 にする（[docs/support.md](../support.md)）。SwiftPM の `binaryTarget` は、Apple のプラットフォームでだけ使える。
- iOS のアプリは、自前の動的ライブラリを `dlopen` で読めない。ライブラリはアプリに静的にリンクするか、フレームワークに入れて署名する。
- 手本は Python・.NET・Java のラッパー（[0195](0195-python-wrapper.md)・[0196](0196-dotnet-wrapper.md)・[0224](0224-java-kotlin-wrapper.md)）。API は足すだけで、ラッパーは知らないフィールドと列挙の値も受け取る（[0015](0015-add-only-boundary-and-version-floors.md)）。
- SwiftPM のミラーのリポジトリ（`zumen-swift`）に出すことは、段階 2 の別の子 issue で扱う。

## 選択肢

### ネイティブのライブラリの形

1. **動的ライブラリを XCFramework に入れる。** iOS では、フレームワークの形と署名が要る。
2. **静的ライブラリ（`staticlib`）を XCFramework に入れる。** ヘッダーと module map を添えると、Swift から `import CZumen` で呼べる。アプリに静的にリンクされ、署名はアプリの署名で済む。

### 型の書き方

1. **`Codable` の構造体に読み直す。** 知らないフィールドを落とす。
2. **読んだ JSON のオブジェクト（`[String: JSON]`）を包む構造体を生成する。** フィールドごとに読み書きするプロパティを持つ。知らないフィールドも残る。

### シミュレーターでの流し方

1. **XCTest のテストの対象を、Xcode のプロジェクトで流す。** プロジェクトの設定が増える。
2. **`swiftc` でシミュレーターに向けて実行ファイルを作り、`xcrun simctl spawn` で流す。** macOS と同じテストの実行ファイルを、そのまま流せる。

## 決定

ネイティブのライブラリの形は 2、型の書き方は 2、シミュレーターでの流し方は 2 を選ぶ。

- **置き場所。** ラッパーを `bindings/swift` に置き、SwiftPM のパッケージにする。ライブラリは `Zumen`、ネイティブの部分は `CZumen` の `binaryTarget`。
- **XCFramework**（`build-xcframework.sh`）。コアを `staticlib` で、macOS（arm64・x86_64）・iOS（arm64）・iOS のシミュレーター（arm64・x86_64）に向けて作り、プラットフォームごとに `lipo` で 1 つにして、`xcodebuild -create-xcframework` でまとめる。ヘッダーは `bindings/c/include` の `zumen.h` と `zumen_scene.h` を写し、`module.modulemap` を添える。
- **依存。** Foundation と、テストの SHA-256 に使う CryptoKit だけを使う。JSON の小さな読み書き（`JSON`）を、ラッパーの中に持つ。Foundation の `JSONSerialization` は真偽値と数の区別があいまいで、64 bit の整数を写せないため使わない。
- **形**
  - `Session` が `push`・`call`・`callData`・`callText`・`take`・`cancel`・`close` を持ち、API の関数（[0012](0012-boundary-functions-and-messages.md)）に 1 つずつ当たる。作るときに ABI のメジャーバージョンを確かめ、`deinit` で閉じる。
  - コアが返したバッファーは、写したあとにラッパーの中で `zm_free` で返す。長さ 0 の入力にも NULL でない番地を渡す。
  - 状態が `ok` でなければ `ZumenError` を投げる。`failed` の応答も、`response` で読める。閉じたセッションへの呼び出しは NULL のまま API に渡し、API が `invalid` を返す。`cancel` は、ほかのスレッドから呼べる。`cancel` と `close` はロックで順に並べる。
  - シーンの blob は `Scene` で読む。セクションは `u8`・`u32`・`f32`・`f64`・`buffer`（どれも写す）で引く。知らないセクションは読み飛ばし、ないセクションは空の列にし、メジャーバージョンが違えば読まない（[0137](0137-columnar-scene-transfer.md)）。
- **生成するもの**（`cargo xtask codegen`）
  - `Messages.swift`: `schema/messages.json` の定義ごとの構造体（`Message` に従う）。フィールドはプロパティで読み書きし（ないフィールドと形の違う値は `nil`、`nil` を書くとフィールドを除く）、名前はキャメルケースにする。Swift の語と重なる名前はバッククォートで囲み、`Message` のメンバーと重なる名前には `Field` を付ける。タグのフィールド（`op` など）は、作るときに入れる。列挙は文字列の定数、共用体は形ごとの型に移す `asXxx`、op の名前は `Operations` に置く。名前が重なれば、生成を止める。
  - `Constants.swift`: バージョン、状態コード（`Status` と名前）、シーンのセクションの定義と値の組。
- **適合テスト**
  - `ZumenConformance` が、`bindings/conformance` の組をラッパーのメソッドで流す。状態・応答の値・ハッシュの一覧が `hashes.txt` と一致することと、シーンの blob をリーダーで読んで応答の数と合うことを確かめる。
  - 毎晩（`nightly.yml`）、macOS の arm64 で、配るのと同じ対象の XCFramework を作り、`swift run` で流す。続けて `test-simulator.sh` が、iOS 15 に向けて作ったテストを iPhone のシミュレーターで流す。

## 影響

- Tier 2 の 3 つの条件（生成のテンプレート・API の関数のラッパー・毎晩の適合テスト）を、この PR で満たす。
- `Messages.swift` は大きい（約 7,600 行）。新しい op・フィールドを足すと、その差分が PR に並ぶ。
- XCFramework はコミットせず、作るたびに作る。ミラーのリポジトリに出す形（URL の `binaryTarget` とチェックサム）は、ミラーのリポジトリの issue で決める。
- iOS の実機、x86_64 の Mac、Mac Catalyst・visionOS・watchOS・tvOS は、テストしていない。Mac Catalyst などは、対象を足すときに決める。
- `zm_take_arrow`（Arrow の C Stream Interface）は、ラッパーのメソッドにしない。
- ラッパーの API と生成した型は、公開するまで変えてよい。公開してからは、API と同じく足すだけにする。

## 参照

- [0006](0006-language-tiers.md)、[0011](0011-bindings-through-ffi.md)、[0012](0012-boundary-functions-and-messages.md)、[0014](0014-platforms-signing-and-distribution.md)、[0015](0015-add-only-boundary-and-version-floors.md)、[0137](0137-columnar-scene-transfer.md)、[0195](0195-python-wrapper.md)、[0196](0196-dotnet-wrapper.md)、[0224](0224-java-kotlin-wrapper.md)
- [docs/support.md](../support.md)、[bindings/swift](../../bindings/swift/README.md)
- #431
