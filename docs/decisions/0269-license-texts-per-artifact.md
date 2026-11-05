# 0269: 依存のライセンス本文を crate・target ごとに作り、wheel・NuGet・CLI・ネイティブの配布物に入れる

| 項目 | 内容 |
| --- | --- |
| 日付 | 2026-11-06 |
| 状態 | 採用 |

## 背景

[0014](0014-platforms-signing-and-distribution.md) は、すべての成果物に依存のライセンス本文を入れると決めた。[0259](0259-javascript-on-node-servers.md) で wasm と npm には入れたが、wheel・NuGet・CLI の圧縮・ネイティブのライブラリにはまだない（#536）。wasm とネイティブ、ABI と CLI は依存が違うので、wasm 用の本文だけを使い回せない。

wheel の Core metadata 2.4 は、ライセンス文を `License-File` に列挙し、`.dist-info/licenses/` に入れる。`License-Expression` は、その配布物全体のライセンスを表す。NuGet も、パッケージのライセンスを SPDX の式で表せる。

## 選択肢

1. **ワークスペースの依存を全部まとめ、すべての配布物に入れる。** 作る回数は少ないが、開発用の依存や、別の target の依存も入る。
2. **crate・target ごとに本文を作り、梱包する前後に確かめる。** 配布物との対応が分かり、取り違えを止められる。生成を複数回行う。
3. **ライセンス本文と、SBOM・埋め込む依存一覧を一度にそろえる。** [0014](0014-platforms-signing-and-distribution.md) の成果物をまとめられるが、依存の範囲と、端末の絶対パスの除去を先に確かめる必要がある。cargo-cyclonedx 0.5.9 は、ワークスペース内の manifest を指定しても複数の crate を出し、`bom-ref` と依存の参照に絶対パスを含めた。

## 決定

- 選択肢 2 を採る。既存の cargo-about 0.9.2・`about.toml`・`about.hbs` を使い、`zumen-abi` と `zumen-cli` を、配る 7 target ごとに生成する。既存の wasm の ABI 用も残す。feature は配布時と同じ default とし、build・dev の依存を除く。
- `xtask/release/targets.json` を、リリースと配布物の検査が共通で読む。Windows x86 は、32 bit の Office の VBA 向けに残す（[0011](0011-bindings-through-ffi.md)・[0014](0014-platforms-signing-and-distribution.md)）。
- cargo-about は、配るバイナリを作る job とは別の job で動かす。警告・SPDX の定型の文・第三者の crate の抜けを拒む。本文の先頭に対象の crate と target を書く。
- 生成時に、crate・target・依存の名前と版・選んだライセンス・本文の SHA-256 を `license-info.json` に残す。梱包時の照合に使い、端末のパスを含む cargo-about の生の JSON は配布物に入れない。
- 本文の置き方を次のようにする。リポジトリ自身の `LICENSE-MIT` と `LICENSE-APACHE` も添える。

  | 配布物 | 第三者の本文 | リポジトリ自身の本文 |
  | --- | --- | --- |
  | wheel | `.dist-info/licenses/THIRD_PARTY_LICENSES`（ABI 用） | 同じディレクトリ |
  | NuGet | `runtimes/{rid}/native/THIRD_PARTY_LICENSES`（ABI 用） | パッケージの直下 |
  | CLI の圧縮 | 実行ファイルと同じディレクトリの `THIRD_PARTY_LICENSES`（CLI 用） | 同じディレクトリ |
  | ネイティブ | ライブラリと同じディレクトリの `THIRD_PARTY_LICENSES`（ABI 用） | 同じディレクトリ |

- wheel は 3 つの本文を `License-File` と `RECORD` に載せる。wheel と NuGet のライセンスの式は、リポジトリの `MIT OR Apache-2.0` に、同梱する第三者の選ばれたライセンスを `AND` で併記する。リポジトリ自身のライセンスは変えない。
- 梱包には Python 3.11 以降の標準ライブラリを使う。梱包後に本文とバイナリの一致、対象の抜け、wheel の `RECORD`、wheel と NuGet のメタデータを確かめる。CLI の圧縮には、端末の時刻と利用者の名前を入れない。
- 公開しない `package-check.yml` を PR で流し、リリースと同じ生成・梱包の関数で、7 target の実バイナリから配布物を作って確かめる。
- SBOM と cargo-auditable は [0014](0014-platforms-signing-and-distribution.md) の採用を保ち、実装を #593 に分ける。crate・target・feature ごとの依存と埋め込んだ一覧の照合、参照関係を保った絶対パスの除去を先に確かめる。

## 影響

- リリースの権限を持たない生成・梱包の job で必要なファイルを用意する。公開の job は、既存のコピーと公開の手順でこれらを扱える。
- 依存が変わって本文を見分けられなくなったら、梱包の前に止まる。本文を確認して `about.toml` の clarify を直す。
- SBOM と埋め込んだ一覧は、#593 の実装と検査がそろうまで配布物に付かない。

## 参照

- #536・#593
- [Core metadata: License-Expression・License-File](https://packaging.python.org/en/latest/specifications/core-metadata/)
- [Wheel distribution format](https://packaging.python.org/en/latest/specifications/binary-distribution-format/)
- [NuGet の license](https://learn.microsoft.com/en-us/nuget/reference/nuspec#license)
- [cargo-cyclonedx 0.5.9](https://github.com/CycloneDX/cyclonedx-rust-cargo/tree/cargo-cyclonedx-0.5.9/cargo-cyclonedx)
