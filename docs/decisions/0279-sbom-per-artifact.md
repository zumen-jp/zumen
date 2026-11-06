# 0279: 配布物ごとに SBOM を作り、バイナリに埋め込んだ依存と照合する

| 項目 | 内容 |
| --- | --- |
| 日付 | 2026-11-06 |
| 状態 | 採用 |

## 背景

[0014](0014-platforms-signing-and-distribution.md) で採用した CycloneDX と cargo-auditable を、[0205](0205-release-and-publishing.md) の配布物に使う（#593）。[0259](0259-javascript-on-node-servers.md)・[0269](0269-license-texts-per-artifact.md) の cargo-about は、通常の依存にある第三者の crate のライセンス本文を配る。build・dev の依存を除く。SBOM は build の依存も扱う。

cargo-cyclonedx 0.5.9 は、manifest を指定しても workspace の各 member に出力し、参照 ID にローカルの絶対パスを使う。cargo-auditable 0.7.7 の埋込形式 1 は、proc-macro を build の依存として扱う。cargo-cyclonedx と cargo-about は、Cargo の通常の依存に宣言した proc-macro をその区分のまま扱う。

## 選択肢

1. **埋込情報から SBOM を生成する。** ビルドと同じ一覧になるが、梱包したバイナリを独立した一覧と照らせない。
2. **Cargo の対象ごとに SBOM を作り、梱包したバイナリの埋込情報と照合する。** 正規化と照合が要るが、別の crate・target・feature の取り違えを検査できる。

## 決定

選択肢 2 を採る。7 target のネイティブ ABI と CLI（14 組）、wasm の全体版と分割版（図面・BIM・土木地理・点群の 5 組）を対象とする。

- cargo-about と cargo-cyclonedx はバイナリを作る job から分ける。cargo-cyclonedx 0.5.9 を、追跡ファイルを写した一時ディレクトリで動かす。Cargo の manifest・target・既定または分割版の feature を指定する。対象バイナリの CycloneDX 1.5 JSON だけを選ぶ。前に `cargo metadata --locked` を通し、生成後に lockfile の不変と crates.io の各 crate の SHA-256 を確かめる。
- 部品とすべての依存の辺を Cargo のグラフと照らす。dev だけの辺は除く。cargo-about の選んだライセンスは、その対象の通常の辺だけで到達する第三者の crate と照合する。SBOM の区分では、build の辺か proc-macro を経由する依存を `excluded`、実行時にも使うものを `required` とする。推移する依存と、複数の経路から使う crate にも適用する。
- `bom-ref` は `urn:zumen:cargo:{source}:{name}@{version}` とし、起点・各部品・依存元・依存先をまとめて書き換える。入手先は workspace の `local` と `crates.io` を区別する。新しい入手先には、一意性を保つ識別規則を決めてから対応する。PURL は、workspace の crate にリポジトリの URL・コミットとリポジトリ内の相対パスを付ける。生成した文書に絶対パスがあれば止める。
- 配布時のコミットの時刻を `SOURCE_DATE_EPOCH` に渡し、その時刻を検査する。乱数の `serialNumber` は省略する。部品・辺・JSON のキーの順と UTF-8 の改行を固定する。同じコミット・lockfile・target・feature・ツール版・バイナリなら、作業ディレクトリによらず同じ SBOM のバイト列になる。バイナリの再ビルドでバイト列が違えば、そのハッシュも変わる。
- 実物は `cargo auditable build --release --locked` で作る。Cargo が返した対象ファイルと有効な feature を確かめ、要求した feature・target・crate・コミット・lockfile と実物の SHA-256 を記録する。梱包時にこの記録を照合する。SBOM の起点のハッシュは配るバイナリの SHA-256 とする。
- 梱包後の実物を rust-audit-info 0.5.4 で読み、起点、入手先・名前・版、build と実行時の区分、依存の辺の全体を照合する。埋込形式 1 は Cargo の解決した依存を表し、最適化で除かれたコードの区別はしない。
- ネイティブのライブラリと CLI の圧縮では、バイナリの隣に `{バイナリ名}.cdx.json` を置く。wheel は `.dist-info/sboms/` と `RECORD`、NuGet は各 `runtimes/{rid}/native/`、npm は `sboms/` に入れる。個別に配る wasm には、同名で対応付けた 5 つの文書を `zumen-{版}-wasm-sboms.tar.gz` にまとめる。Release の既存のアーカイブ収集と来歴の証明を使える。wasm の全体版に添える本文が分割版の依存も含むことを確かめる。

## 影響

公開しない配布物の検査で、リリースと同じ生成・梱包を 19 組で行う。梱包時の Python は標準ライブラリを使い、上記のツールは固定版の外部 CLI として呼ぶ。Rust コアや各ラッパーの実行時の依存には足さない。cargo-cyclonedx は Apache-2.0、cargo-auditable と rust-audit-info は MIT OR Apache-2.0 である。

## 参照

- [cargo-cyclonedx 0.5.9](https://github.com/CycloneDX/cyclonedx-rust-cargo/tree/cargo-cyclonedx-0.5.9/cargo-cyclonedx)・[CycloneDX 1.5](https://cyclonedx.org/docs/1.5/json/)
- [cargo-auditable 0.7.7 の埋込形式](https://github.com/rust-secure-code/cargo-auditable/blob/v0.7.7/cargo-auditable.schema.json)・[rust-audit-info](https://crates.io/crates/rust-audit-info/0.5.4)
- [Python の sboms ディレクトリ](https://packaging.python.org/en/latest/specifications/recording-installed-packages/#the-sboms-subdirectory)
