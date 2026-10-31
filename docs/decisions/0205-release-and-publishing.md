# 0205: 版 0.2.0 から、タグを push してワークフローで crates.io・npm・PyPI・NuGet と GitHub の Release に出す

| 項目 | 内容 |
| --- | --- |
| 日付 | 2026-10-31 |
| 状態 | 採用 |

## 背景

- 利用者は、ある程度使えるものができたら公開すると決めた（#317）。
- 名前は [0003](0003-names-and-domain.md) で決めてある。crates.io の `zumen`・`zumen-*`、npm の `@zumen-jp/*`、PyPI の `zumen`、NuGet の `Zumen`、CLI の `zumen`。どれも空いている。PyPI の `zumen` には別のリポジトリの 0.1.1 があるので、版を 0.2.0 から始める。
- 配る形は [0014](0014-platforms-signing-and-distribution.md) で決めてある。言語の包みには、対象ごとのネイティブのライブラリを入れる。
- Python と .NET の包みは、仮の名前（`zumen_provisional`・`Zumen.Provisional`）で、公開を止める印（分類・`IsPackable`・`private`）を持っていた。
- CLI は、MCP のリソースとして `schema/` と `catalog/` を crate の外から読んでいた。crates.io の crate は、crate のフォルダーの外を持てない。

## 選択肢

1. タグの push で、GitHub Actions が対象ごとに作り、Release に置き、各レジストリに trusted publishing（OIDC）で出す。
2. 手元で作って、手で出す。
   - 対象が 7 つあり、手元では作れないものがある。来歴の証明も付かない。

## 決定

- 選択肢 1 を採る。`.github/workflows/release.yml` に置く。
- 版は 0.2.0 にし、すべての言語で同じ版にする（[0015](0015-add-only-boundary-and-version-floors.md)）。仮の名前をやめ、`zumen`（Python）・`Zumen`（.NET）・`@zumen-jp/zumen`（npm）にする。
- 対象は、linux-x64・linux-arm64・osx-arm64・osx-x64・win-x64・win-x86・win-arm64 と wasm にする。
- 出すもの:
  - crates.io: `zumen` と、それが使う `zumen-*` の crate と、`zumen-cli`。依存される側から順に出す。試験だけの crate（`zumen-c`・`zumen-conformance`・`xtask`）は出さない。
  - npm: `@zumen-jp/zumen`。`zumen_abi1.wasm` を入れ、`@zumen-jp/zumen/zumen_abi1.wasm` で取り出せるようにする。
  - PyPI: 対象ごとの wheel（`py3-none-<プラットフォーム>`）。ライブラリを `zumen/_native` に入れ、`load()` が道筋なしで読む。wheel は標準ライブラリだけの `bindings/python/build_wheel.py` で作る。
  - NuGet: `Zumen`。ライブラリを `runtimes/{rid}/native` に入れ、`ZumenLibrary.Load()` が道筋なしで読む。
  - GitHub の Release: CLI の圧縮、ライブラリ、wasm、wheel、npm と NuGet の包み。来歴の証明（attestation）を付ける。
- CLI の MCP のリソースは、`cargo xtask codegen` で `bindings/cli/resources/` に写し、`--check` が更新漏れを調べる。
- 手で走らせたワークフローは、作るところと `cargo publish --dry-run` までを確かめ、出さない。
- Windows と Apple のコード署名（[0014](0014-platforms-signing-and-distribution.md)）は、まだしない。証明書と鍵の用意が要る。

## 影響

- 出す前に、利用者が各レジストリで trusted publisher を登録する。リポジトリ `zumen-jp/zumen`、ワークフロー `release.yml`、環境 `release`。NuGet はアカウントの名前を、リポジトリの変数 `NUGET_USER` に置く。
- crates.io と npm は、まだない名前に trusted publisher を登録できない。最初の 1 回は、トークンで出すか、空の版で名前を取る必要がある。
- 版を上げるときは、Cargo.toml・`package.json`・`pyproject.toml`・csproj をそろえ、`v<版>` のタグを push する。

## 参照

- [0003](0003-names-and-domain.md)・[0014](0014-platforms-signing-and-distribution.md)・[0015](0015-add-only-boundary-and-version-floors.md)
- #317
