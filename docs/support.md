# 対応する環境

zumen が対象にするプラットフォーム・言語の実行環境・ホストアプリと、その下限です。

- ホストアプリのメーカーがサポートするもっとも古い実行環境に合わせます。単独で動く環境はサポート期間内を対象とします。ホストと実行環境の両方のサポートが終了したら、下限を上げる前のリリースで予告し、次のマイナーバージョンで上げます。
- 下限の表の初版は、2026-10-24 に確かめた事情によります。ブラウザーは、下の節に更新時の根拠と検査の範囲を書いています。

## 言語ごとの約束

Tier 1 は言語ごとのパッケージをコアと同じ日に公開し、PR ごとに適合テストを流します。Tier 2 は生成したパッケージを毎晩検査します。Tier 3 は C ABI・wasm・CLI・HTTP の使い方とサンプルを共通のテストで確かめます。Tier を上げる PR では、生成テンプレート、API の関数のラッパー、適合テストの通過を確認します。

## プラットフォーム

| OS | アーキテクチャ | Rust の target | OS の下限 | 確かめ方 |
| --- | --- | --- | --- | --- |
| Windows | x64 | `x86_64-pc-windows-msvc` | Windows 10、Windows Server 2016 | PR ごとにテスト |
| Windows | x86（32 bit の Office） | `i686-pc-windows-msvc` | 同上 | PR ごとにテスト |
| Windows | ARM64 | `aarch64-pc-windows-msvc` | 同上 | 毎晩にテスト |
| macOS | arm64 | `aarch64-apple-darwin` | macOS 11 | PR ごとにテスト |
| macOS | x64 | `x86_64-apple-darwin` | macOS 10.12 | 毎晩に作る |
| Linux | x64 | `x86_64-unknown-linux-gnu` | glibc 2.28（`manylinux_2_28`）、カーネル 3.2 | PR ごとにテスト |
| Linux | arm64 | `aarch64-unknown-linux-gnu` | glibc 2.28、カーネル 4.1 | 毎晩にテスト |
| Linux | x64・arm64（musl、静的） | `x86_64-unknown-linux-musl`・`aarch64-unknown-linux-musl` | glibc に依らない | 毎晩にテスト |
| iOS・iPadOS | arm64（実機・シミュレーター） | `aarch64-apple-ios`・`aarch64-apple-ios-sim` | iOS 15 | 毎晩に作る |
| Android | arm64-v8a・armeabi-v7a・x86_64 | `aarch64-linux-android`・`armv7-linux-androideabi`・`x86_64-linux-android` | Android 7.0（API 24） | 毎晩に作る |
| wasm | wasm32 | `wasm32-unknown-unknown` | 下の「ブラウザー」 | PR ごとにテスト |

- OS の下限は、Rust の対象の下限（Windows 10、macOS 11・10.12、glibc 2.17）と、配り方の下限（`manylinux_2_28`、Flutter の iOS 15・Android 7.0）の高いほうです。
- glibc 2.28 より古い Linux では、musl で静的にリンクした CLI を、プロセスの外か HTTP の形態（[API のスキーマ](../schema/README.md)）で使います。glibc のプロセスには、musl のライブラリを読み込めません。
- ツールチェーンは Rust 1.99.1 です。1.99.0 は使いません（`bool` を返す `extern "C"` の関数を、x64 で誤ってコンパイルする）。
- 公開済み v0.2.0 の GNU Linux のライブラリと CLI は `GLIBC_2.34` を要求し、この表の glibc 2.28 の下限を満たしていません。PyPI の wheel のインストールの条件は `manylinux_2_35`（glibc 2.35 以降）です。これは公開物の静的解析とタグの条件であり、glibc 2.34 での動作確認ではありません。以後のビルドと検査の修正、新しい版の公開は [#789](https://github.com/zumen-jp/zumen/issues/789) で追跡します。v0.2.0 には musl の CLI もありません。

## 言語と実行環境

| Tier | 言語 | 下限 | 理由 | 確かめ方 |
| --- | --- | --- | --- | --- |
| 1 | Rust | 1.99（crate の `rust-version`） | 固定したツールチェーン | PR ごと |
| 1 | TS・JS（ブラウザー） | Chrome・Edge 96、Firefox 91、Safari 15.1 | wasm の機能と、ラッパーの構文・API の両方（下の根拠） | PR ごと（Chromium）、毎晩（Firefox・WebKit）。下限の実機は未確認 |
| 1 | Node.js | 22 | 20 のサポートは 2026-04-30 に終わった | PR ごと。Next.js（Turbopack・webpack）で組んで使うテストは毎晩（[JavaScript の使い方](../bindings/js/README.md)） |
| 1 | Python | 3.9 | Rhino 8 と Dynamo 3.x の CPython3（どちらも 3.9） | PR ごと（3.9 と最新） |
| 1 | .NET | .NET Framework 4.8、.NET 8 以降 | Revit・AutoCAD 2024 は .NET Framework 4.8。netstandard2.0 の 1 つのアセンブリで両方に出す | PR ごと（.NET Framework 4.8 と .NET 10） |
| 1 | C・C++ | C99、C++17 | | PR ごと |
| 2 | Java | 8 | 業務システム。Oracle の延長サポートは 2030 年 12 月まで。JNA 5 の下限も 8 | 毎晩 |
| 2 | Kotlin（Android） | Android 7.0（API 24） | Dart（Flutter）の下限に合わせる | 毎晩（エミュレーター） |
| 2 | Swift | iOS・iPadOS 15、macOS 12 | Dart（Flutter）の下限に合わせる。SwiftPM の `binaryTarget` は Apple のプラットフォームだけ | 毎晩（シミュレーター） |
| 2 | Go | Go がサポートする 2 つのメジャーバージョン（今は 1.26・1.27） | Go のサポートの方針。wazero も同じ方針 | 毎晩 |
| 2 | Dart | Dart 3（Flutter 3） | | 毎晩 |
| 2 | VBA | VBA7（Office 2010 以降）、32 bit・64 bit | `Declare PtrSafe` が使えるバージョン。確かめるのは、サポートのある Microsoft 365 と Office LTSC 2024 | DLL は PR ごと。Office の実機は [#800](https://github.com/zumen-jp/zumen/issues/800) で未確認 |

## ブラウザーの下限の根拠

2026-11-06 に、ブラウザーの入口の `bindings/js/src/index.js` と、その入口が読む `modules.js`・`scene.js`・生成した `constants.js`、分割 wasm の URL を返す入口の `wasm.js` を確認しました。下表は、[MDN Browser Compatibility Data の固定したコミット](https://github.com/mdn/browser-compat-data/tree/188707c73f5a93701429eadcdaf35d6e31715c67)で、その構文・API を使える最初のバージョンを調べたものです。Node.js 専用の `node.js` と型の宣言は含めません。

| ラッパーが使う機能 | Chrome | Edge | Firefox | Safari |
| --- | --- | --- | --- | --- |
| [ESM の import・export](https://github.com/mdn/browser-compat-data/blob/188707c73f5a93701429eadcdaf35d6e31715c67/javascript/statements.json) | 61 | 16 | 60 | 10.1 |
| [`import.meta`](https://github.com/mdn/browser-compat-data/blob/188707c73f5a93701429eadcdaf35d6e31715c67/javascript/operators/import_meta.json) | 64 | 79 | 62 | 11.1 |
| [`URL` のコンストラクター](https://github.com/mdn/browser-compat-data/blob/188707c73f5a93701429eadcdaf35d6e31715c67/api/URL.json) | 19 | 12 | 26 | 14.1 |
| [async・await](https://github.com/mdn/browser-compat-data/blob/188707c73f5a93701429eadcdaf35d6e31715c67/javascript/statements.json) | 55 | 15 | 52 | 10.1 |
| [オブジェクトのスプレッド](https://github.com/mdn/browser-compat-data/blob/188707c73f5a93701429eadcdaf35d6e31715c67/javascript/operators/spread.json) | 60 | 79 | 55 | 11.1 |
| [optional chaining（`?.`）](https://github.com/mdn/browser-compat-data/blob/188707c73f5a93701429eadcdaf35d6e31715c67/javascript/operators/optional_chaining.json) | 80 | 80 | 74 | 13.1 |
| [nullish coalescing（`??`）](https://github.com/mdn/browser-compat-data/blob/188707c73f5a93701429eadcdaf35d6e31715c67/javascript/operators/nullish_coalescing.json) | 80 | 80 | 72 | 13.1 |
| [private のフィールド](https://github.com/mdn/browser-compat-data/blob/188707c73f5a93701429eadcdaf35d6e31715c67/javascript/classes.json) | 74 | 79 | 90 | 14.1 |
| [private のメソッド](https://github.com/mdn/browser-compat-data/blob/188707c73f5a93701429eadcdaf35d6e31715c67/javascript/classes.json) | 84 | 84 | 90 | 15 |
| [`Error` の第 2 引数の `cause`](https://github.com/mdn/browser-compat-data/blob/188707c73f5a93701429eadcdaf35d6e31715c67/javascript/builtins/Error.json) | 93 | 93 | 91 | 15 |
| [`Object.entries`](https://github.com/mdn/browser-compat-data/blob/188707c73f5a93701429eadcdaf35d6e31715c67/javascript/builtins/Object.json) | 54 | 14 | 47 | 10.1 |
| [`TextEncoder`](https://github.com/mdn/browser-compat-data/blob/188707c73f5a93701429eadcdaf35d6e31715c67/api/TextEncoder.json) | 38 | 79 | 18 | 10.1 |
| [`TextDecoder`](https://github.com/mdn/browser-compat-data/blob/188707c73f5a93701429eadcdaf35d6e31715c67/api/TextDecoder.json) | 38 | 79 | 19 | 10.1 |
| [`Response.arrayBuffer`](https://github.com/mdn/browser-compat-data/blob/188707c73f5a93701429eadcdaf35d6e31715c67/api/Response.json) | 42 | 14 | 39 | 10.1 |
| [WebAssembly の API](https://github.com/mdn/browser-compat-data/blob/188707c73f5a93701429eadcdaf35d6e31715c67/webassembly/api.json)（compile・instantiate・Module・Instance・Memory） | 57 | 16 | 52 | 11 |

- private のメソッドは、`loadModules` を呼ばなくても import のときに読みます。Firefox 79 は、その時点で構文エラーになります。Firefox 90 でも、`ZumenError` が元の例外を `cause` に保つ契約を満たせません。
- class・分割代入・テンプレート文字列・アロー関数・rest・配列のスプレッド・for-of と、Array・TypedArray・ArrayBuffer・DataView・Map・Promise・Symbol・Object・JSON・Math の使用箇所も確認しました。これらは上の下限を上げません。`Symbol.dispose` は存在するときだけ使い、`using` の構文は使いません。
- Edge の 79 は、Chromium を使う Edge の最初のバージョンです。BCD の `mirror` を、その開始バージョンも含めて読みました。

Rust の [`wasm32-unknown-unknown` が既定で使う機能](https://doc.rust-lang.org/stable/rustc/platform-support/wasm32-unknown-unknown.html)は、multivalue・mutable-globals・reference-types・sign-ext・bulk-memory・nontrapping-fptoint です。配布用の `.cargo/config.toml` では、これらを無効にしていません。うち、下限を左右する機能は次のとおりです。

| wasm の機能 | Chrome | Edge | Firefox | Safari |
| --- | --- | --- | --- | --- |
| [reference-types](https://github.com/mdn/browser-compat-data/blob/188707c73f5a93701429eadcdaf35d6e31715c67/webassembly/reference-types.json) | 96 | 96 | 79 | 15.1 |
| [bulk-memory](https://github.com/mdn/browser-compat-data/blob/188707c73f5a93701429eadcdaf35d6e31715c67/webassembly/bulk-memory-operations.json) | 75 | 79 | 79 | 15.1 |
| [nontrapping-fptoint](https://github.com/mdn/browser-compat-data/blob/188707c73f5a93701429eadcdaf35d6e31715c67/webassembly/non-trapping-float-to-int-conversions.json) | 75 | 18 | 64 | 15.1 |

両方の高いほうから、Chrome・Edge 96、Firefox 91、Safari 15.1 としています。これは互換性の資料から求めた下限で、下限の製品そのもので動かした結果ではありません。[適合テスト](../bindings/conformance/README.md)に従い、CI では固定した Playwright が用意する Chromium・Firefox・WebKit で適合テストを流します。

利用側のコードには、別の要件が加わることがあります。たとえば、[転送の測定ページ](../bench/transfer/browser.html)が使う [module Worker](https://github.com/mdn/browser-compat-data/blob/188707c73f5a93701429eadcdaf35d6e31715c67/api/Worker.json)は Firefox 114 以降です。ラッパー自身は Worker を作りません。`bindings/js/examples/worker` は Node.js の `worker_threads` の例です。

## ホストアプリ

| ホストアプリ | 対象のバージョン | 言語と実行環境 |
| --- | --- | --- |
| Revit・AutoCAD・Civil 3D | 2024〜2027（現行と過去 3 バージョン） | .NET Framework 4.8（2024）、.NET 8（2025・2026。2026 年 8〜9 月の更新から .NET 10）、.NET 10（2027） |
| Dynamo（Revit の中） | 3.x・4.x | CPython 3.9（3.x の既定）、CPython 3.11（PythonNet3。4.0 の既定） |
| pyRevit | 7 | CPython 3.12 |
| Rhino・Grasshopper | 8 | CPython 3.9 |
| Blender | 4.5 LTS 以降 | Python 3.11（5.1 から 3.13） |
| QGIS | 3.44 LTR 以降 | Python 3.12（Windows） |
| ArcGIS Pro | 3.3 以降 | Python 3.11（3.6 から 3.13） |
| FreeCAD | 1.0 以降 | Python 3.11 |
| Excel | Microsoft 365、Office LTSC 2024 | VBA7（32 bit・64 bit）、.NET（Excel-DNA） |
| SketchUp | 2024 以降 | Ruby 3.2（Tier 3） |
| Unity | 6 の LTS | .NET Standard 2.1。IL2CPP は、iOS と Web では静的ライブラリをリンクする |

- 32 bit の Excel では、64 bit の子プロセス（[API のスキーマ](../schema/README.md) の `process`）を使うと、大きな図面を読めます。

## 見直す予定

| 対象 | 見直すとき |
| --- | --- |
| Python 3.9 | Rhino 8 と、Dynamo 3.x を載せた Revit のサポートが、どちらも終わったとき |
| .NET Framework 4.8 | Revit・AutoCAD 2024 のサポートが終わったとき（2028 のバージョンが出たとき） |
| Intel の macOS | Homebrew が対応を終える 2027 年 9 月。Rust の対応も見る |
| 32 bit の Windows | Rust 1.100 から、ホストのツールを持たない Tier 1 になる（64 bit の Windows で作る）。Rust の対応を見る |
