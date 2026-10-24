# 対応する環境

zumen が対象にするプラットフォーム・言語の実行環境・ホストアプリと、その下限です。

- 対象のプラットフォームと確かめ方は [決定 0014](decisions/0014-platforms-signing-and-distribution.md)、下限の決め方と上げ方は [決定 0015](decisions/0015-add-only-boundary-and-version-floors.md) にあります。この表は、その規則で直します。
- 最初の版です（2026-10-24 に確かめた事情による）。まだ成果物がないので、「確かめ方」は予定です。

## プラットフォーム

| OS | アーキテクチャ | Rust の target | OS の下限 | 確かめ方 |
| --- | --- | --- | --- | --- |
| Windows | x64 | `x86_64-pc-windows-msvc` | Windows 10、Windows Server 2016 | PR ごとに試験 |
| Windows | x86（32 bit の Office） | `i686-pc-windows-msvc` | 同上 | PR ごとに試験 |
| Windows | ARM64 | `aarch64-pc-windows-msvc` | 同上 | 毎晩に試験 |
| macOS | arm64 | `aarch64-apple-darwin` | macOS 11 | PR ごとに試験 |
| macOS | x64 | `x86_64-apple-darwin` | macOS 10.12 | 毎晩に作る |
| Linux | x64 | `x86_64-unknown-linux-gnu` | glibc 2.28（`manylinux_2_28`）、カーネル 3.2 | PR ごとに試験 |
| Linux | arm64 | `aarch64-unknown-linux-gnu` | glibc 2.28、カーネル 4.1 | 毎晩に試験 |
| Linux | x64・arm64（musl、静的） | `x86_64-unknown-linux-musl`・`aarch64-unknown-linux-musl` | glibc に依らない | 毎晩に試験 |
| iOS・iPadOS | arm64（実機・シミュレーター） | `aarch64-apple-ios`・`aarch64-apple-ios-sim` | iOS 15 | 毎晩に作る |
| Android | arm64-v8a・armeabi-v7a・x86_64 | `aarch64-linux-android`・`armv7-linux-androideabi`・`x86_64-linux-android` | Android 7.0（API 24） | 毎晩に作る |
| wasm | wasm32 | `wasm32-unknown-unknown` | 下の「ブラウザ」 | PR ごとに試験 |

- OS の下限は、Rust の対象の下限（Windows 10、macOS 11・10.12、glibc 2.17）と、配り方の下限（`manylinux_2_28`、Flutter の iOS 15・Android 7.0）の高いほうです。
- glibc 2.28 より古い Linux では、musl で静的にリンクした CLI を、プロセスの外か HTTP の形（[決定 0013](decisions/0013-boundary-in-four-forms.md)）で使います。glibc のプロセスには、musl のライブラリを読み込めません。
- ツールチェーンは Rust 1.99.1 です。1.99.0 は使いません（`bool` を返す `extern "C"` の関数を、x64 で誤ってコンパイルする）。

## 言語と実行環境

| 段 | 言語 | 下限 | 理由 | 確かめ方 |
| --- | --- | --- | --- | --- |
| 1 | Rust | 1.99（crate の `rust-version`） | 固定したツールチェーン | PR ごと |
| 1 | TS・JS（ブラウザ） | Chrome・Edge 96、Firefox 79、Safari 15 | Rust の wasm32 が既定で使う wasm の機能（multivalue・mutable-globals・reference-types・sign-ext・bulk-memory・nontrapping-fptoint） | PR ごと（Chromium）、毎晩（Firefox・WebKit） |
| 1 | Node.js | 22 | 20 のサポートは 2026-04-30 に終わった | PR ごと |
| 1 | Python | 3.9 | Rhino 8 と Dynamo 3.x の CPython3（どちらも 3.9） | PR ごと（3.9 と最新） |
| 1 | .NET | .NET Framework 4.8、.NET 8 以降 | Revit・AutoCAD 2024 は .NET Framework 4.8。netstandard2.0 の 1 つのアセンブリで両方に出す | PR ごと（.NET Framework 4.8 と .NET 10） |
| 1 | C・C++ | C99、C++17 | | PR ごと |
| 2 | Java | 8 | 業務システム。Oracle の延長サポートは 2030 年 12 月まで。JNA 5 の下限も 8 | 毎晩 |
| 2 | Kotlin（Android） | Android 7.0（API 24） | Dart（Flutter）の下限に合わせる | 毎晩（エミュレーター） |
| 2 | Swift | iOS・iPadOS 15、macOS 12 | Dart（Flutter）の下限に合わせる。SwiftPM の `binaryTarget` は Apple のプラットフォームだけ | 毎晩（シミュレーター） |
| 2 | Go | Go がサポートする 2 つの大版（今は 1.26・1.27） | Go のサポートの方針。wazero も同じ方針 | 毎晩 |
| 2 | Dart | Dart 3（Flutter 3） | | 毎晩 |
| 2 | VBA | VBA7（Office 2010 以降）、32 bit・64 bit | `Declare PtrSafe` が使える版。確かめるのは、サポートのある Microsoft 365 と Office LTSC 2024 | Windows の実機（`verify`） |

## ホストアプリ

| ホストアプリ | 対象の版 | 言語と実行環境 |
| --- | --- | --- |
| Revit・AutoCAD・Civil 3D | 2024〜2027（現行と過去 3 版） | .NET Framework 4.8（2024）、.NET 8（2025・2026。2026 年 8〜9 月の更新から .NET 10）、.NET 10（2027） |
| Dynamo（Revit の中） | 3.x・4.x | CPython 3.9（3.x の既定）、CPython 3.11（PythonNet3。4.0 の既定） |
| pyRevit | 7 | CPython 3.12 |
| Rhino・Grasshopper | 8 | CPython 3.9 |
| Blender | 4.5 LTS 以降 | Python 3.11（5.1 から 3.13） |
| QGIS | 3.44 LTR 以降 | Python 3.12（Windows） |
| ArcGIS Pro | 3.3 以降 | Python 3.11（3.6 から 3.13） |
| FreeCAD | 1.0 以降 | Python 3.11 |
| Excel | Microsoft 365、Office LTSC 2024 | VBA7（32 bit・64 bit）、.NET（Excel-DNA） |
| SketchUp | 2024 以降 | Ruby 3.2（段 3） |
| Unity | 6 の LTS | .NET Standard 2.1。IL2CPP は、iOS と Web では静的ライブラリをリンクする |

- 32 bit の Excel では、64 bit の子プロセス（[決定 0013](decisions/0013-boundary-in-four-forms.md) の `process`）を使うと、大きな図面を読めます。

## 見直す予定

| 対象 | 見直すとき |
| --- | --- |
| Python 3.9 | Rhino 8 と、Dynamo 3.x を載せた Revit のサポートが、どちらも終わったとき |
| .NET Framework 4.8 | Revit・AutoCAD 2024 のサポートが終わったとき（2028 の版が出たとき） |
| Intel の macOS | Homebrew が対応を終える 2027 年 9 月。Rust の対応も見る |
| 32 bit の Windows | Rust 1.100 から、ホストの道具を持たない Tier 1 になる（64 bit の Windows で作る）。Rust の対応を見る |
