# 配布物とライセンスの検査

`licenses.py` は crate・target ごとの依存の本文を生成し、`package.py` は配布物に入れて、包んだ中身を確かめます（[決定 0269](../../docs/decisions/0269-license-texts-per-artifact.md)）。どちらも公開はしません。Python 3.11 以降の標準ライブラリを使います。

## ライセンス本文を作る

```bash
cargo install --locked cargo-about@0.9.2 --features cli
python xtask/release/licenses.py --output target/release-licenses
```

`--cargo-about` で実行ファイルの場所を指定できます。生成した生の JSON は一時ディレクトリだけに置きます。出力には、次のものが入ります。

- `THIRD_PARTY_LICENSES`: 既存の npm と wasm 向けの本文
- `licenses/wasm/abi/`: wasm の ABI 用の本文と、照合用の `license-info.json`
- `licenses/native/{rid}/{abi,cli}/`: 7 target の ABI・CLI 用の本文と、照合用の `license-info.json`

cargo-about の警告、SPDX の定型の文への置換、本文に第三者の crate が載っていないときは止まります。crate の更新後に止まったら、`about.toml` の clarify と、更新先の本文を確認してください。生成に使う feature は、配布時と同じ default です。

## ネイティブのライブラリ・CLI・wheel を作る

次は Windows x64 の例です。ほかの組は [targets.json](targets.json) の `rid` と `target` を使います。

```bash
cargo build --release --locked --target x86_64-pc-windows-msvc -p zumen-abi -p zumen-cli
python xtask/release/package.py native --rid win-x64 --built target/x86_64-pc-windows-msvc/release --licenses target/release-licenses --out target/release-packages
```

`out` の下に、ライセンス本文を添えた `native/{rid}/`、CLI の zip か tar.gz、`wheels/*.whl` ができます。対象の違う本文、本文のハッシュの不一致、梱包後の抜けやバイナリの変化があれば止まります。wheel は `License-File` と `RECORD` も確かめます。

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

[package-check.yml](../../.github/workflows/package-check.yml) は、関連する PR でリリースと同じ関数を使い、全 target の実バイナリからネイティブのライブラリ・CLI・wheel・NuGet を作ります。cargo-about はバイナリを作る job と分け、各配布物の中身まで確かめます。公開の権限と secrets は使いません。

SBOM と cargo-auditable の埋め込みは、[決定 0014](../../docs/decisions/0014-platforms-signing-and-distribution.md) に沿って [#593](https://github.com/zumen-jp/zumen/issues/593) で扱います。
