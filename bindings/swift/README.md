# Swift の包み

zumen の C ABI を、Swift から呼ぶ包みです（[決定 0225](../../docs/decisions/0225-swift-wrapper.md)）。下限は iOS・iPadOS 15 と macOS 12 です（[docs/support.md](../../docs/support.md)）。まだ公開していません。SwiftPM の鏡のリポジトリに出すことは、別の issue で扱います。

## 使い方

コアを静的ライブラリで作り、XCFramework（`build/CZumen.xcframework`）にまとめてから、このフォルダーを SwiftPM のパッケージとして加えます。

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
| `Session` | 境界の関数の包み。`push`・`call`・`callData`・`callText`・`take`・`cancel`・`close`。`cancel` だけは、ほかのスレッドから呼べる |
| `ZumenError` | 状態が `ok` でなかったときに投げる。`status`・`code`・`data`・`response`（`failed` の応答と診断） |
| `Scene` | シーンの blob の読み手。区画を `u8`・`u32`・`f32`・`f64`・`buffer` で引き、文字列を `text` で引く |
| `JSON` | JSON の値。`parse`・`text`・`data` |
| `Messages.swift` の型 | 要求と結果の型（生成）。JSON のオブジェクトを包む構造体で、知らない欄も持つ。欄はプロパティで読み書きし、`with` で書き換えた写しを作る |
| `Constants` | 版・状態コード・シーンの区画（生成） |

`Messages.swift` と `Constants.swift` は、`cargo xtask codegen` で作ります。手で直しません。

## 試験

適合試験（[bindings/conformance](../conformance/README.md)）を、macOS と iOS のシミュレーターで流します。毎晩の `nightly.yml` でも流します。

```bash
sh bindings/swift/build-xcframework.sh debug aarch64-apple-darwin aarch64-apple-ios-sim
```

```bash
cd bindings/swift && swift run ZumenConformance ../..
```

```bash
sh bindings/swift/test-simulator.sh
```

`test-simulator.sh` は、包みと試験を `swiftc` で iOS 15 のシミュレーターに向けて作り、使える iPhone のシミュレーターの 1 つめで `xcrun simctl spawn` で流します。シミュレーターの名前か UDID を引数で選べます。
