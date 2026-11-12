# 開発への参加

zumen の計画は [#1 の子 issue](https://github.com/zumen-jp/zumen/issues/1)、各 crate の担当は[コアの設計](docs/architecture.md#モデルと依存)にあります。

- 誰が担当するか決まっていない大きな変更は、issue で範囲を相談してください。
- issue・PR・コメントは日本語を基本にします。
- 参加する人には、[行動規範](CODE_OF_CONDUCT.md)が適用されます。

## 図面の取り扱い

図面は、顧客のものであることがあります。

- 機密の図面を issue・PR に添付したり、Git に入れたりしないでください。
- 公開されている図面は、再配布を認めるライセンスのものだけを `samples/` に入れられます。出所とライセンスを `samples/ledger.toml` に書いてください（[公開サンプルの条件](samples/README.md)）。`samples/` のファイルは、リポジトリのライセンスではなく、台帳に書いたそれぞれのライセンスに従います。
- 不具合を報告するときは、再現する最小の図面を作るか、形式・バージョン・作成ソフトと、表示された診断を書いてください。読み取りが終わらない、落ちる、メモリや CPU を使いすぎる入力は、公開の issue でなく、[SECURITY.md](SECURITY.md) のとおり非公開で報告してください。

## 貢献のライセンス

明示しない限り、貢献は Apache License 2.0 の定める「貢献」（Contribution）として、追加の条件なしに MIT と Apache-2.0 の 2 つのライセンスで提供されたものとします（[ライセンス](README.md#ライセンス)）。

旧版のコードを移すときは、元の著作者が MIT から MIT OR Apache-2.0 にライセンスを付け替えます。ほかの人の貢献を含む部分は、その人の同意を確かめてから移してください。

次のものは入れないでください。

- 仕様の本文・図・表を写したもの
- GPL の実装を読んで書いたもの
- 権利のはっきりしないもの

## 依存と照合ツール

実行時・リンク・Cargo の全依存（build・dev を含む）と同梱コードの生成ツールには、[許可した寛容なライセンス](deny.toml)を使います。ルートの Cargo ワークスペースの外部依存は crates.io に限ります。追加前にライセンス・wasm・決定性を確認し、採用理由を記録してください。

Cargo の外で動き、成果物に何も残さないツールはライセンスを問いません。照合用の外部実装は同梱せず、別に入れたプログラムか寛容なライセンスの dev 依存を使います。外部プログラムは指定時と毎晩の CI で実行します。ソースを読めるのは寛容なライセンスの実装に限ります。

## 開発の手順

書き込み権限のある担当者は[直接 push の手順](docs/work.md#直接-push)を使えます。外部からの貢献は [PR](docs/work.md#pr-とレビュー) で受け付けます。

コミットと PR の題は `feat(dxf): 外部参照を読む` のように、種別とスコープを英語、説明を日本語にします。設計を変えるときは仕様・テストも更新し、理由はコミットか PR に残してください。

## 変更に応じた検査

手元では次の表から変更に応じて検査を選びます。Rust は `rust-toolchain.toml` のバージョン、Node.js は 22 以降を使います。Rust を上げるときは CI の cargo-deny のバージョンも合わせます。

| 変更したもの | 手元で確かめること |
| --- | --- |
| 説明だけの Markdown | 記述とリンクを確認し、`npx --no-install textlint <ファイル>` を実行。依存が未導入か lockfile が変わったときは先に `npm ci` |
| 表記辞書・表記の設定、コードのコメント・カタログの文 | `npm run lint:text`。[表記の基準](docs/style.md) を参照 |
| Rust の `tests/` のテストだけ | `cargo test -p <crate> --test <テスト名>`。共通の入力を変えたときは、その入力を使うテストも実行 |
| Rust の実装 | `cargo fmt --all --check`、`cargo clippy -p <crate> --all-targets -- -D warnings`、`cargo test -p <crate>`。呼び出す側に影響すればそのテストも実行 |
| ワークスペース全体に及ぶ変更 | `cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace` |
| スキーマ・台帳・生成器・生成物 | 元を更新して `cargo xtask codegen`、続けて `cargo xtask codegen --check`。元と生成物を一緒にコミット |
| API・バインディング | 対象言語のテスト。[適合テスト](bindings/conformance/README.md#流し方)で応答を照合。`zumen-abi` を変えたら wasm32 の clippy と適合テストも実行 |
| サイト・表示部品・CLI | 変えたパッケージのビルドと、その操作を通るテスト |

文書でもコード生成・ビルド・配布に使うものは、その処理を確かめます。応答が変わったら `cargo run -p zumen-conformance -- --write` でハッシュを更新し、理由を残します。

[通常の CI](.github/workflows/ci.yml) は push・PR の[変更パスから検査を選びます](.github/ci/checks.py)。形式の文書は台帳と照合し、サイトの入力はビルドとブラウザーテストでも確かめます。

`crates/` の実装を変えたときは、Linux でその crate と、それを直接・間接に使う crate を lint し、テストします。利用側は `Cargo.lock` の開発用・任意の依存も含めて選びます。テストや例だけの変更は原則としてその crate で確かめます。たとえば PDF の読み取りだけを変えた push では、`zumen-pdf` を既定の機能と領域モデルなしの両方で検査します。

ABI に変更が及ぶ push とすべての PR では、[対応する環境](docs/support.md)の Linux x64・Windows x86・Windows x64・macOS arm64 と wasm で適合テストを実行します。PR ごとに台帳・生成物・依存も検査します。週に 1 回と CI の手動実行では、4 環境でワークスペース全体をテストし、wasm の圧縮後のサイズも記録します。

変更に [fuzz](fuzz/README.md#動かし方) や [配布物の検査](xtask/release/README.md#公開せずに検査する) が必要なら、対応する対象を手元で実行するか、PR のワークフローを使います。
