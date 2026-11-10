# 配布物とライセンス・SBOM の検査

`licenses.py` と `sbom.py` は crate・target・feature ごとに依存の本文と CycloneDX SBOM を生成します。`build.py` は依存を埋め込んだバイナリを作り、`package.py` は梱包後のファイルから依存を取り出して照合します。Python 3.11 以降の標準ライブラリを使います。

## リリースの条件と周期

- リリースを止めるのは、[タグの前に利用者に示す一覧](../../fuzz/README.md#初回リリース後の記録先)に載る入力の失敗、公開の試運転の失敗や権利・ライセンスの問題、ID の契約を破る変更です。
- CI と `release.yml` の試運転が通った main のコミットで区切り、そのコミットのタグの push を利用者に頼み、同じ日に push します。依頼・コミットの変更・取り下げはバージョンごとのリリースの issue に記録します。依頼後に見つけた対象の失敗は非公開で扱い、そのリリースを止めずにパッチで直します。
- 0.2.0 のあとは 2 週間ごとに main から出します。非公開で扱う入力の失敗を直したら、周期を待たずにパッチを出します。壊す変更や環境の下限を上げる変更ではマイナー、それ以外はパッチを上げ、バージョンの PR をマージしたコミットにタグを打ちます。
- 0.2.0 で外した PDF の配布は [#521](https://github.com/zumen-jp/zumen/issues/521) などの修正後、LAZ の展開は依存の修正版が出たあとにパッチで戻します（[#510](https://github.com/zumen-jp/zumen/issues/510)）。区切りと除外を決めた利用者の記録は [#317](https://github.com/zumen-jp/zumen/issues/317) にあります。

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

## npm の公開を再試行する

公開済みの Release のパッケージを検証し、npm だけに公開する経路です。ワークフローの差分、`release` 環境に `main` ブランチの許可を足す設定変更、手動公開の例外と実際の公開について、利用者の承認を受けてから使います。設定の追加では、既存のタグの許可・承認者・ほかの保護の規則を維持します。

```bash
gh workflow run release.yml --ref main -f npm_retry_tag=v0.2.0
```

このコマンドは、検証後に npm への公開を始めます。今回は、許可リストに固定した `v0.2.0` だけが対象です。元のタグのコミット、Release の状態、asset と取得したファイルの SHA-256、パッケージの名前・バージョン・リポジトリ、元の GitHub attestation を確かめます。ファイルの展開・再ビルド・パッケージのスクリプトの実行はしません。npm に同じバージョンがすでにあれば断り、ほかのレジストリには出し直しません。

npm の provenance は、再試行した main のワークフローとコミットを参照します。元のタグの配布物と同じことは、固定した digest と元の GitHub attestation の検証で、別に確かめます。

`npm_retry_tag` を空にした手動実行は、公開せずにビルドと `cargo publish --dry-run` までを確かめます。手動でタグを指定した場合も、公開しません。通常の公開は、これまでどおり `v*` のタグの push で行います。

## NuGet を作る

7 target の `native/{rid}/` と、生成した `licenses/` を同じディレクトリに集め、.NET 10 SDK で実行します。

```bash
python xtask/release/package.py nuget --artifacts artifacts --out target/release-packages
```

元の C# プロジェクトは変更せず、一時ディレクトリに写して包みます。パッケージの 7 target の本文とライブラリ、リポジトリ自身の本文、アセンブリとメタデータを確かめます。

## 公開せずに検査する

```bash
python -m unittest discover -s xtask/release -p 'test_*.py'
python xtask/release/check_cargo_scope.py
```

このテストは、一時ディレクトリで合成のバイナリを包み、対象・crate の取り違え、本文の変化や抜け、`RECORD` の抜けを拒むことを確かめます。

`check_cargo_scope.py` は、上記の固定版のツールを使い、別の member だけが optional な依存を要求する小さな workspace を実際にビルドします。SBOM と埋込情報が同じ余分な crate を返した場合と、部品の数が合って辺だけが余った場合を検出します。該当する feature を有効にしたビルドは通ります。

[package-check.yml](../../.github/workflows/package-check.yml) は、関連する PR でリリースと同じ関数を使い、7 target のネイティブのライブラリ・CLI・wheel・NuGet と、5 組の wasm・npm を実物で検査します。cargo-about と cargo-cyclonedx はバイナリを作る job と分けます。
