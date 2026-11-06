# 配布物とライセンス・SBOM の検査

`licenses.py` と `sbom.py` は crate・target・feature ごとに依存の本文と CycloneDX SBOM を生成します。`build.py` は依存を埋め込んだバイナリを作り、`package.py` は梱包後のファイルから依存を取り出して照合します（[決定 0269](../../docs/decisions/0269-license-texts-per-artifact.md)・[決定 0279](../../docs/decisions/0279-sbom-per-artifact.md)）。Python 3.11 以降の標準ライブラリを使います。

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
- `licenses/native/{rid}/{abi,cli}/`: 7 target の ABI・CLI 用の本文と、照合用の `license-info.json`
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

## wasm と npm のパッケージを作る

```bash
python xtask/release/build.py --rid wasm --out target/release-built/wasm
python xtask/release/package.py wasm --built target/release-built/wasm --licenses target/release-licenses --out target/release-licenses
python xtask/release/package.py npm --artifacts target/release-licenses --out target/release-packages
```

全体版と 4 つの分割版の wasm に、それぞれ同じ名前で対応する SBOM を `*-wasm-sboms.tar.gz` と npm の `sboms/` に入れます。tgz の中の wasm からも埋込情報を読み取ります。

## NuGet を作る

7 target の `native/{rid}/` と、生成した `licenses/` を同じディレクトリに集め、.NET 10 SDK で実行します。

```bash
python xtask/release/package.py nuget --artifacts artifacts --out target/release-packages
```

元の C# プロジェクトは変更せず、一時ディレクトリに写して包みます。パッケージの 7 target の本文とライブラリ、リポジトリ自身の本文、アセンブリとメタデータを確かめます。

## 公開せずに検査する

```bash
python -m unittest discover -s xtask/release -p 'test_*.py'
```

このテストは、一時ディレクトリで合成のバイナリを包み、対象・crate の取り違え、本文の変化や抜け、`RECORD` の抜けを拒むことを確かめます。

[package-check.yml](../../.github/workflows/package-check.yml) は、関連する PR でリリースと同じ関数を使い、7 target のネイティブのライブラリ・CLI・wheel・NuGet と、5 組の wasm・npm を実物で検査します。cargo-about と cargo-cyclonedx はバイナリを作る job と分けます。
