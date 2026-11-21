# 対応する環境

zumen が対象にするプラットフォーム・言語の実行環境・ホストアプリと、その下限です。

- 現行の 64 bit 環境・最新の言語ランタイム・WebGPU・wasm の固定幅 SIMD（`simd128`）を基準にし、32 bit 環境や古い実行環境に設計を合わせません。
- 下限を上げる変更は、マイナーバージョンのリリースノートに書きます。
- 公開済み v0.3.0 の配布物は引き上げ前の条件（glibc 2.28、`manylinux_2_28`、Windows x86、macOS x64 など）で作られたもので、再公開による置き換えは行いません。以下は `main`（次のマイナーバージョン以降）の対象です。

## 言語ごとの約束

Tier 1 は言語ごとのパッケージをコアと同じ日に公開し、PR ごとに適合テストを流します。Tier 2 はラッパーとテストを毎晩検査します。Tier を上げる PR では、生成テンプレート、API の関数のラッパー、適合テストの通過を確認します。

Tier 3 では、公式のラッパーを使わずに図面を開く方法を、呼び出し方ごとに 1 例ずつ保守します。Ruby で C ABI、PHP でネイティブと WASI の CLI、PowerShell で HTTP を確かめます。採用する例には、その言語で公開図面を開く手順と、出力を共通の期待値と照らす毎晩の CI が必要です。[サンプルと実行手順](tier3.md)

## プラットフォーム

| OS | アーキテクチャ | Rust の target | OS の下限 | 確かめ方 |
| --- | --- | --- | --- | --- |
| Windows | x64 | `x86_64-pc-windows-msvc` | Windows 11、Windows Server 2022 | PR ごとにテスト |
| Windows | ARM64 | `aarch64-pc-windows-msvc` | 同上 | 毎晩に作る |
| macOS | arm64 | `aarch64-apple-darwin` | macOS 14 | PR ごとにテスト |
| Linux | x64 | `x86_64-unknown-linux-gnu` | glibc 2.34（`manylinux_2_34`）、カーネル 5.14 | PR ごとにテスト |
| Linux | arm64 | `aarch64-unknown-linux-gnu` | 同上 | 毎晩にテスト |
| iOS・iPadOS | arm64（実機・シミュレーター） | `aarch64-apple-ios`・`aarch64-apple-ios-sim` | iOS 17 | 毎晩に作る |
| Android | arm64-v8a・x86_64 | `aarch64-linux-android`・`x86_64-linux-android` | Android 12（API 31） | 毎晩に作る |
| wasm | wasm32 | `wasm32-unknown-unknown` | 下の「ブラウザー」 | PR ごとにテスト |

- ネイティブの対象はすべて 64 bit です。32 bit の Windows（`i686-pc-windows-msvc`）・32 bit の Android（`armv7-linux-androideabi`）・Intel の macOS（`x86_64-apple-darwin`）・musl の配布物は作りません。
- GNU Linux のライブラリ・CLI・Python wheel は glibc 2.34（`manylinux_2_34`、AlmaLinux 9 相当）で作り、要求するシンボルのバージョンと wheel のタグを検査します。
- ツールチェーンは Rust 1.99.1 です。1.99.0 は使いません（`bool` を返す `extern "C"` の関数を、x64 で誤ってコンパイルする）。

## 言語と実行環境

| Tier | 言語 | 下限 | 理由 | 確かめ方 |
| --- | --- | --- | --- | --- |
| 1 | Rust | 1.99（crate の `rust-version`） | 固定したツールチェーン | PR ごと |
| 1 | TS・JS（ブラウザー） | Chrome・Edge 113、Firefox 141、Safari 26 | WebGPU、wasm の `simd128`、現行の標準 API（下の根拠） | PR ごと（Chromium）、毎晩（Firefox・WebKit） |
| 1 | Node.js | 24 | 現行の LTS | PR ごと。Next.js（Turbopack・webpack）で組んで使うテストは毎晩（[JavaScript の使い方](../bindings/js/README.md)） |
| 1 | Python | 3.12 | 現行のホスト環境と標準ライブラリ | PR ごと（3.12 と最新） |
| 1 | .NET | .NET 8、.NET 10 | Revit・AutoCAD 2025 以降。`net8.0` と `net10.0` を出す | PR ごと（.NET 8 と .NET 10） |
| 1 | C・C++ | C11、C++17 | 64 bit の標準の呼び出し規約 | PR ごと |
| 2 | Java | 21 | 現行の LTS | 毎晩 |
| 2 | Kotlin（Android） | Android 12（API 31） | 64 bit の Android | 毎晩（エミュレーター） |
| 2 | Swift | iOS・iPadOS 17、macOS 14 | Apple Silicon と 64 bit シミュレーター | 毎晩（シミュレーター） |
| 2 | Go | Go がサポートする 2 つのメジャーバージョン（今は 1.26・1.27） | Go と wazero のサポート方針 | 毎晩 |
| 2 | Dart | Dart 3.5（Flutter 3.24） | 現行の Flutter 3 世代 | 毎晩 |
| 2 | VBA | VBA7（Office 2021 以降、64 bit） | 64 bit の Microsoft 365 と Office LTSC 2024 | DLL は PR ごと。Microsoft 365 の Excel 64 bit で [VBE のコンパイルと全手順を確認](../bindings/vba/README.md#実機の検証結果) |

## ブラウザーの下限の根拠

ブラウザー向けのパッケージ（`@zumen-jp/zumen`・`@zumen-jp/view2d`・`@zumen-jp/view3d`・`@zumen-jp/react`）とサイトのビューアーは、次の 2 つを前提にします。

1. **WebGPU（`navigator.gpu`）**: 2D と 3D の表示部品は描画を WebGPU に一本化し、Canvas 2D や WebGL2 へのフォールバックを持ちません。WebGPU が標準で有効になった Chrome・Edge 113、Firefox 141（Windows。macOS は 145 以降）、Safari 26 を下限とします。WebGPU は Secure Context（`https:`・`http://localhost`・`http://127.0.0.1` など）でのみ利用できます。
2. **wasm の固定幅 SIMD（`simd128`）**: 配布用の `.cargo/config.toml` は、`wasm32-unknown-unknown` と `wasm32-wasip1` で `simd128` を有効にします（決定性を崩す `relaxed-simd` は使いません）。あわせて Rust の既定機能（multivalue・mutable-globals・reference-types・sign-ext・bulk-memory・nontrapping-fptoint）を使います。これらは上の WebGPU 対応下限のすべてのブラウザーに含まれます。

JavaScript のラッパーと表示部品は、`Object.hasOwn`・`Array.prototype.at`・`Symbol.dispose`・`OffscreenCanvas`・module Worker・import map など、この下限で標準搭載されている構文と API をそのまま使います。

[適合テスト](../bindings/conformance/README.md)に従い、CI では Playwright の Chromium で WebGPU 表示を含む全テストを PR ごとに流し、毎晩のジョブで Firefox と WebKit も検査します（Linux 版 WebKit には WebGPU のバックエンドがないため、Linux 上ではコアの wasm 適合テストと `webgpu-unavailable` の扱いを確認します）。

製品版の Firefox と Safari では、[合成シーンの検査](../bindings/js/test/browser/desktop/)で同じ位置の画素と選択を照合できます。`bindings/js/test/browser` で `npm ci`、`node node_modules/vite/bin/vite.js build --config desktop/vite.config.mjs` の順に実行します。Windows は `uv run desktop/check.py firefox`、macOS は Safari のリモートオートメーションを有効にして `uv run desktop/check.py safari` で検査します。[Safari の手動 CI](../.github/workflows/verify-webgpu.yml) も同じページを使います。

200 × 200 CSS px のビューで、2D は（100, 70）の青い線 `space/0/h1`、3D は上面ビューの（30, 70）の赤い面 `red` を選びます。Windows 11（ビルド 26300）と Firefox 159.0 では、既定の WebGPU 設定で両方の画素と ID が一致しました。機種別の実行記録は [#918](https://github.com/zumen-jp/zumen/issues/918) にあります。

## ホストアプリ

| ホストアプリ | 対象のバージョン | 言語と実行環境 |
| --- | --- | --- |
| Revit・AutoCAD・Civil 3D | 2025〜2027 | .NET 8（2025・2026）、.NET 10（2026 更新後・2027） |
| Dynamo（Revit の中） | 4.x | CPython 3.12 以降（PythonNet3） |
| pyRevit | 7 | CPython 3.12 |
| Blender | 5.1 以降 | Python 3.13 |
| QGIS | 3.44 LTR 以降 | Python 3.12（Windows） |
| ArcGIS Pro | 3.6 以降 | Python 3.13 |
| Excel | Microsoft 365、Office LTSC 2024 | VBA7（64 bit）、.NET（64 bit） |
| SketchUp | 2024 以降 | Ruby 3.2（Tier 3） |
| Unity | 6 の LTS | .NET Standard 2.1 / .NET（64 bit プラットフォーム） |

- 古いホストアプリ（.NET Framework 4.8 の AutoCAD・Revit 2024、Python 3.9 の Rhino 8・Dynamo 3.x、32 bit の Excel など）から使う場合は、同じ PC の 64 bit の CLI（`zumen serve --stdio` または `zumen serve --http`）を別プロセスとして呼びます（[Tier 3 の言語から使う](tier3.md)）。
