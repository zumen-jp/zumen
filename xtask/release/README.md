# 配布物とライセンス・SBOM の検査

`licenses.py` と `sbom.py` は crate・target・feature ごとに依存の本文と CycloneDX SBOM を生成します。`build.py` は依存を埋め込んだバイナリを作り、`package.py` は梱包後のファイルから依存を取り出して照合します。Python 3.11 以降の標準ライブラリを使います。

## リリースの条件と周期

- リリースを止めるのは、[タグの前に利用者に示す一覧](../../fuzz/README.md#初回リリース後の記録先)に載る入力の失敗、公開の試運転の失敗や権利・ライセンスの問題、ID の契約を破る変更です。
- CI と `release.yml` の試運転が通った main のコミットで区切り、そのコミットのタグの push を利用者に頼み、同じ日に push します。依頼・コミットの変更・取り下げはバージョンごとのリリースの issue に記録します。依頼後に見つけた対象の失敗は非公開で扱い、そのリリースを止めずにパッチで直します。
- 0.2.0 のあとは 2 週間ごとに main から出します。非公開で扱う入力の失敗を直したら、周期を待たずにパッチを出します。壊す変更や環境の下限を上げる変更ではマイナー、それ以外はパッチを上げます。タグを打つ対象は、バージョンを更新した main のコミット（PR を使ったときはマージしたコミット）です。
- v0.4.0 の配布先・タグを打つコミット・公開状況は [#932](https://github.com/zumen-jp/zumen/issues/932) に記録します。v0.3.0 の実績は [#795](https://github.com/zumen-jp/zumen/issues/795) にあります。

## ライセンス本文と SBOM を作る

```bash
cargo install --locked cargo-about@0.9.2 --features cli
cargo install --locked cargo-cyclonedx@0.5.9
python xtask/release/licenses.py --output target/release-licenses
python xtask/release/sbom.py --output target/release-licenses
```

`--cargo-about` で実行ファイルの場所を指定できます。生成した生の JSON は一時ディレクトリだけに置きます。出力には、次のものが入ります。

- `THIRD_PARTY_LICENSES`: 既存の npm と wasm 向けの本文
- `licenses/wasm/abi/`: wasm の ABI 用の本文と、照合用の `license-info.json`
- `licenses/wasm/{drawing,bim,civil,pointcloud}/`: 分割版の feature ごとの本文と照合用の一覧
- `licenses/native/{rid}/{abi,cli}/`: 5 target の ABI・CLI 用の本文と、照合用の `license-info.json`
- 各組の `cargo.cdx.json`: 梱包する前の SBOM。梱包時に起点へバイナリの SHA-256 を入れます。

cargo-about の警告、SPDX の定型の文への置換、本文に第三者の crate が載っていないときは止まります。crate の更新後に止まったら、`about.toml` の clarify と、更新先の本文を確認してください。分割版は `--no-default-features` と対象の feature を使います。SBOM の生成とビルドは、Rust の配布対象の入力をコミットした作業ディレクトリで行います。

## ネイティブのライブラリ・CLI・wheel を作る

次は Windows x64 の例です。ほかの組は [targets.json](targets.json) の `rid` と `target` を使います。

```bash
cargo install --locked cargo-auditable@0.7.7 rust-audit-info@0.5.4
python xtask/release/build.py --rid win-x64 --out target/release-built/win-x64
python xtask/release/package.py native --rid win-x64 --built target/release-built/win-x64 --licenses target/release-licenses --out target/release-packages
```

`out` の下に、ライセンス本文と SBOM を添えた `native/{rid}/`、CLI の zip か tar.gz、`wheels/*.whl` ができます。依存の区分や辺、ビルド時の記録、梱包後のバイナリや本文が違えば止まります。wheel は `.dist-info/sboms/` と `RECORD`、`License-File` も確かめます。

`win-x64` では [VBA 用](../../bindings/vba/README.md)の `zumen-vba-<版>-win-x64.zip` も作ります。VBE に取り込む 6 ファイルを ASCII・CRLF で収録し、同じビルドの DLL、ABI 用のライセンス本文・SBOM、導入手順を同梱します。ZIP を読み戻して中身と DLL の依存情報を照合します。タグ公開時はこの ZIP も GitHub Release の添付に含まれます。

### GNU Linux の下限を確かめる

Linux の x64 と arm64 は、[対応するプラットフォーム](../../docs/support.md#プラットフォーム)に従い glibc 2.34 で作ります。`targets.json` は、[PyPA の manylinux_2_34](https://github.com/pypa/manylinux#manylinux_2_34-almalinux-9-based) の公式イメージを digest で固定します。Linux のビルドだけをコンテナーに置き、x64 と arm64 はそれぞれのアーキテクチャの runner で動かします。`build.py` は実環境の glibc とアーキテクチャも確認するので、通常の Ubuntu 上でそのまま配布用の GNU Linux バイナリを作ることはできません。

manylinux と、Rust を入れる [dtolnay/rust-toolchain](https://github.com/dtolnay/rust-toolchain/tree/683b588c487a69d01f6c6ddf40f036ff682cc2c8) は MIT のビルドツールとして使います。Rust のバージョンは `rust-toolchain.toml` から読みます。コンテナーとその GCC などのソース・イメージは成果物に同梱せず、実行時の依存は追加しません。ELF の検査は Python の標準ライブラリだけで行います。

コンテナーでは、checkout の一時 HOME にある Git の許可に依存せず、CI の workspace の正確なパスだけを `safe.directory` に指定します。設定は `GIT_CONFIG_COUNT` の command scope でこの job に渡し、既存の項目も保ちます。個人のグローバル設定は変更せず、`*` や親フォルダー全体は指定しません。ビルドの前に、同じ Git でリポジトリの場所と状態を読めることを確かめます。

`linux_abi.py` は ELF64 のアーキテクチャ、GNU のバージョン要求、wheel のタグを確かめます。ELF のバージョン要求が壊れている場合と、glibc 2.34 を超える要求がある場合は拒みます。`build.py` のコピー前、ネイティブ・CLI・wheel・NuGet の梱包で同じ検査を使います。単独でも、バイナリを実行せずに確認できます。

```bash
python xtask/release/linux_abi.py --rid linux-x64 built/libzumen_abi1.so built/zumen
```

`package-check.yml` と `release.yml` の Linux のビルドは、梱包後に同じコンテナー内で次を実行します。

```bash
python xtask/release/check_linux_runtime.py --rid linux-x64 --built built --packages out
```

- 梱包対象の共有ライブラリを、Python 3.12・3.14 のソースのラッパーから開き、既存の適合テストのハッシュと比べる。
- wheel を通信なしで一時フォルダーへインストールし、wheel 自体の import と同梱ライブラリの読み込みを確かめる。
- `ZUMEN_CONFORMANCE_CLI` に梱包対象の CLI を指定し、既存の stdio・HTTP・process・MCP の適合テストを流す。テストで作る別の CLI に置き換えて確かめたことにはしない。
- .NET 10 のソースのラッパーから、梱包対象の共有ライブラリの適合テストを流す。.NET の実行だけに使う ICU・Kerberos のライブラリはコンテナーに入れ、配布物には加えない。

コンテナーで実際の glibc 2.34 を使いますが、カーネルは runner と共有するので、古いカーネルの下限で動かしたことにはなりません。静的な検査の通過と、x64・arm64 の実行結果は分けて記録します。公開済み v0.3.0 は引き上げ前の glibc 2.28 で公開された記録です（[#789](https://github.com/zumen-jp/zumen/issues/789)）。

ELF の参照先は [gABI のヘッダーと節](https://gabi.xinuos.com/elf/02-eheader.html)、[LSB の Symbol Versioning](https://refspecs.linuxfoundation.org/LSB_3.1.1/LSB-Core-generic/LSB-Core-generic/symversion.html)、[GNU ld のバージョンの照合](https://sourceware.org/binutils/docs/ld/VERSION.html)です。wheel の下限の意味は [Python Packaging の manylinux](https://packaging.python.org/en/latest/specifications/platform-compatibility-tags/#manylinux) に従います。

## wasm と npm のパッケージを作る

```bash
python xtask/release/build.py --rid wasm --out target/release-built/wasm
python xtask/release/package.py wasm --built target/release-built/wasm --licenses target/release-licenses --out target/release-licenses
python xtask/release/package.py npm --artifacts target/release-licenses --out target/release-packages
```

全体版と 4 つの分割版の wasm に、それぞれ同じ名前で対応する SBOM を `*-wasm-sboms.tar.gz` と `@zumen-jp/zumen` の `sboms/` に入れます。tgz の中の wasm からも埋込情報を読み取ります。あわせて表示部品と React フック（`@zumen-jp/view2d`・`@zumen-jp/view3d`・`@zumen-jp/react`）も、ライセンス本文を添えて同じ版数の `.tgz` に包み、収録ファイル・メタデータ・peer dependency のみを確かめます（[#909](https://github.com/zumen-jp/zumen/issues/909)）。

## Go の鏡に配るファイルを作る

`python xtask/release/go_module.py --artifacts artifacts --out out` に、同じコミットで作った wasm・ライセンス・SBOM を渡します。全形式の wasm、ソース、`go.mod` と `go.sum`、ライセンス本文と SBOM を `zumen-go-<版>.tar.gz` にまとめ、展開したモジュールを共通の適合テストにかけます。

初回は GitHub Release のアーカイブのビルド証明を `gh attestation verify <ファイル> --repo zumen-jp/zumen` で確かめ、配布用のリポジトリ `zumen-jp/zumen-go` のルートに展開します。展開したファイルをコミットしてコアと同じ版のタグを付けたら、空の利用側のモジュールで `go get zumen.dev/go@v<版>` が成功することを確かめます。

## npm の公開を再試行する

公開済みの Release のパッケージを検証し、npm だけに公開する経路です。ワークフローの差分、`release` 環境に `main` ブランチの許可を足す設定変更、手動公開の例外と実際の公開について、利用者の承認を受けてから使います。設定の追加では、既存のタグの許可・承認者・ほかの保護の規則を維持します。

```bash
gh workflow run release.yml --ref main -f npm_retry_tag=v0.2.0
```

このコマンドは、検証後に npm への公開を始めます。今回は、許可リストに固定した `v0.2.0` だけが対象です。元のタグのコミット、Release の状態、asset と取得したファイルの SHA-256、パッケージの名前・バージョン・リポジトリ、元の GitHub attestation を確かめます。ファイルの展開・再ビルド・パッケージのスクリプトの実行はしません。npm に同じバージョンがすでにあれば断り、ほかのレジストリには出し直しません。

npm の provenance は、再試行した main のワークフローとコミットを参照します。元のタグの配布物と同じことは、固定した digest と元の GitHub attestation の検証で、別に確かめます。

`npm_retry_tag` を空にした手動実行は、公開せずにビルドと `cargo publish --dry-run` までを確かめます。手動でタグを指定した場合も、公開しません。通常の公開は、これまでどおり `v*` のタグの push で行います。

## SwiftPM の配布物を作る

macOS の Xcode と Rust の `aarch64-apple-darwin`・`aarch64-apple-ios`・`aarch64-apple-ios-sim` が必要です。`rustup component add llvm-tools-preview` で同じ toolchain の `llvm-ar` を用意します。独立したライセンスの工程で作った `licenses/swift/<対象>/` を使います。

```bash
python xtask/release/swift_package.py --licenses target/release-licenses --out target/release-packages
```

XCFramework の ZIP は 3 つの静的ライブラリとヘッダー、対象別の第三者のライセンス・SBOM、リポジトリのライセンスを収めます。各アーカイブのコンパイル記録と Cargo の依存の辺を SBOM に照合します。固定版の cargo-auditable が同じコンパイルで作る依存情報のオブジェクトを静的ライブラリに加え、XCFramework のアーカイブから読み戻して依存の区分と辺を照合します。SBOM の SHA-256 はそのアーカイブ全体の値です。

`zumen-swift-<版>-source.tar.gz` は鏡に置く Swift ソースと `Package.swift` を収めます。マニフェストは同じ版の GitHub Release の ZIP と、そのバイト列から SwiftPM が計算したチェックサムを参照します。試運転ではマニフェストを検査し、コピーで ZIP の展開先を指定して SwiftPM のビルドを行い、macOS・iOS シミュレーターで適合検査を行います。

## Swift と Go の鏡に出す

初回は手動でも進められます。元のリポジトリの GitHub Release から対象のアーカイブを取り、`gh attestation verify <ファイル> --repo zumen-jp/zumen --signer-workflow zumen-jp/zumen/.github/workflows/release.yml --source-ref refs/tags/v<版> --source-digest <タグのコミット>` で照合します。Go はアーカイブの中身、Swift は `zumen-swift-<版>-source/` の中身をそれぞれの公開鏡のルートに置き、コミットと同じ版のタグを送ります。Swift のバイナリは元の GitHub Release が配ります。

タグ公開で自動化するときは、本人がリポジトリ変数 `PUBLISH_MIRRORS` に `go`、`swift`、`both` のいずれかを設定します。未設定の間は手動経路を使えます。GitHub の `release` 環境の secret `MIRROR_PUBLISH_TOKEN` には、配布先の公開リポジトリ `zumen-jp/zumen-go`・`zumen-jp/zumen-swift` のうち使うものだけにContents の読み書きを許した fine-grained personal access token を登録します。この設定と公開先の作成は利用者が決めます。トークンを使うのは計画済みの Git push のステップだけです。

公開ジョブは同じ Actions 実行が検査したアーカイブを使います。初回は空の鏡に `main` を作り、既定のブランチがあればその履歴を引き継ぎます。前の版で配ったファイルを元の Release の証明と照らし、新版で外したものを除きます。鏡に加えた運用ファイルは保ち、配布対象への手元の変更や既存タグとの食い違いは停止して報告します。ブランチとタグは一度の atomic push で送ります。同じ中身を手動でタグ付けした場合や片方の公開後に再実行した場合は、そのタグを照合して完了します。

## NuGet を作る

5 target の `native/{rid}/` と、生成した `licenses/` を同じディレクトリに集め、.NET 10 SDK で実行します。

```bash
python xtask/release/package.py nuget --artifacts artifacts --out target/release-packages
```

元の C# プロジェクトは変更せず、一時ディレクトリに写して包みます。パッケージの 5 target の本文とライブラリ、リポジトリ自身の本文、アセンブリとメタデータを確かめます。

## 公開せずに検査する

```bash
python -m unittest discover -s xtask/release -p 'test_*.py'
python xtask/release/check_cargo_scope.py
```

このテストは、一時ディレクトリで合成のバイナリを包み、対象・crate の取り違え、本文の変化や抜け、`RECORD` の抜けを拒むことを確かめます。

`check_cargo_scope.py` は、上記の固定版のツールを使い、別の member だけが optional な依存を要求する小さな workspace を実際にビルドします。SBOM と埋込情報が同じ余分な crate を返した場合と、部品の数が合って辺だけが余った場合を検出します。該当する feature を有効にしたビルドは通ります。

[package-check.yml](../../.github/workflows/package-check.yml) は、関連する PR でリリースと同じ関数を使い、5 target のネイティブのライブラリ・CLI・wheel・NuGet と、5 組の wasm・npm を実物で検査します。ドラフトはレビュー可能にしたときに検査します。cargo-about と cargo-cyclonedx はバイナリを作る job と分けます。
