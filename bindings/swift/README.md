# Swift のラッパー

zumen の C ABI を、Swift から呼ぶラッパーです。下限は iOS・iPadOS 17 と macOS 14 です。実機、シミュレーター、Mac は arm64 に対応します。[対応する環境](https://github.com/zumen-jp/zumen/blob/main/docs/support.md)

SwiftPM の鏡 `zumen-swift` に置くソースには、同じ版の GitHub Release にある XCFramework の URL と SHA-256 が入ります。鏡への初回公開は [#435](https://github.com/zumen-jp/zumen/issues/435) で扱います。

## 使い方

開発用に元のリポジトリから使うときは、コアを静的ライブラリで作り、XCFramework（`build/CZumen.xcframework`）にまとめてから、このフォルダーを SwiftPM のパッケージとして加えます。

```bash
sh bindings/swift/build-xcframework.sh release
```

```swift
import Foundation
import Zumen

let session = try Session()
try session.push("a.dxf", try Data(contentsOf: URL(fileURLWithPath: "a.dxf")))
let response = try session.call(OpenRequest().with {
    $0.v = Constants.schemaMajor
    $0.input = "a.dxf"
})
if let result = response.result(as: OpenResult.self) {
    print(result.format ?? "")
}
session.close()
```

| 型 | 中身 |
| --- | --- |
| `Session` | API の関数のラッパー。`push`・`call`・`callData`・`callText`・`take`・`cancel`・`close`。`cancel` だけは、ほかのスレッドから呼べる |
| `ZumenError` | 状態が `ok` でなかったときに投げる。`status`・`code`・`data`・`response`（`failed` の応答と診断） |
| `Scene` | シーンの blob のリーダー。セクションを `u8`・`u32`・`f32`・`f64`・`buffer` で引き、文字列を `text` で引く |
| `JSON` | JSON の値。`parse`・`text`・`data` |
| `Messages.swift` の型 | 要求と結果の型（生成）。JSON のオブジェクトを包む構造体で、知らないフィールドも持つ。フィールドはプロパティで読み書きし、`with` で書き換えた写しを作る |
| `Constants` | バージョン・状態コード・シーンのセクション（生成） |

`Messages.swift` と `Constants.swift` は、`cargo xtask codegen` で作ります。手で直しません。

## テスト

元のリポジトリの[適合テスト](https://github.com/zumen-jp/zumen/tree/main/bindings/conformance)を、macOS と iOS のシミュレーターで流します。毎晩の `nightly.yml` でも流します。

```bash
sh bindings/swift/build-xcframework.sh debug aarch64-apple-darwin aarch64-apple-ios-sim
```

```bash
cd bindings/swift && swift run ZumenConformance ../..
```

```bash
sh bindings/swift/test-simulator.sh
```

`test-simulator.sh` は、ラッパーとテストを `swiftc` で iOS 17 のシミュレーターに向けて作り、使える iPhone のシミュレーターの 1 つめで `xcrun simctl spawn` で流します。シミュレーターの名前か UDID を引数で選べます。

## 3D のシーン

Scene3D(blob) で ZMS3 を読みます。instanceElement と elementID で建物の要素 ID を引けます。列の取得は Swift の配列へコピーします。

同じ blob の 2D 用の読み手との混用はできません。列の配置・座標・単位・色・選択と検査範囲は [3D のシーンの契約](https://github.com/zumen-jp/zumen/blob/main/schema/README.md#3d-のシーンの契約)に従います。元の blob を共有する読み手の利用中は、そのバイト列を変更しません。
