# 開発への参加

zumen の計画は、[#1](https://github.com/zumen-jp/zumen/issues/1) の下の issue で進めています。決まったことは [docs/decisions](docs/decisions/README.md) に、リポジトリのどこに何があるかは [AGENTS.md](AGENTS.md) の「リポジトリの地図」にあります。

- 大きな変更は、着手する前に issue で相談してください。
- issue・PR・コメントは日本語を基本にします。
- 参加する人には、[行動規範](CODE_OF_CONDUCT.md)が適用されます。

## 図面の取り扱い

図面は、顧客のものであることがあります。

- 機密の図面を issue・PR に添付したり、Git に入れたりしないでください。
- 公開されている図面は、再配布を認めるライセンスのものだけを `samples/` に入れられます。出所とライセンスを `samples/ledger.toml` に書いてください（[決定 0171](docs/decisions/0171-public-samples-in-git.md)）。`samples/` のファイルは、リポジトリのライセンスではなく、台帳に書いたそれぞれのライセンスに従います。
- 不具合を報告するときは、再現する最小の図面を作るか、形式・バージョン・作成ソフトと、表示された診断を書いてください。

## 貢献のライセンス

明示しない限り、貢献は Apache License 2.0 の定める「貢献」（Contribution）として、追加の条件なしに MIT と Apache-2.0 の 2 つのライセンスで提供されたものとします（[決定 0002](docs/decisions/0002-dual-license.md)）。

次のものは入れないでください。

- 仕様の本文・図・表を写したもの
- GPL の実装を読んで書いたもの
- 権利のはっきりしないもの

## 開発の準備

Rust は [rustup](https://rustup.rs/) で入れます。バージョンは `rust-toolchain.toml` で決めてあり、リポジトリの中で `rustup toolchain install` を実行すると入ります。

PR を出す前に、CI と同じ検査を手元で通してください。

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo xtask codegen --check
```

API の crate を変えたときは、wasm32 でも確かめてください。適合テストには Node.js 22 以降が要ります（[bindings/conformance/README.md](bindings/conformance/README.md)）。

```bash
cargo clippy -p zumen-abi --target wasm32-unknown-unknown -- -D warnings
cargo build -p zumen-abi --target wasm32-unknown-unknown --release
node bindings/conformance/wasm.mjs target/wasm32-unknown-unknown/release/zumen_abi1.wasm
```

文書・コメント・診断の文を変えたときは、表記の検査も通してください。Node.js 22 以降が要ります。表記と用語の規則は [docs/style.md](docs/style.md) にあります。

```bash
npm ci
npm run lint:text
```

辞書（`prh.yml`）の指摘の多くは、`npm run fix:text` で直せます（Markdown・ソースコード・設定）。

診断の code を足すときは [catalog/README.md](catalog/README.md)、API の関数・op・フィールドを変えるときは [schema/README.md](schema/README.md) の手順に従ってください。

形式を足すときは、実装する前に [権利の台帳](formats/README.md)を更新してください。`cargo xtask codegen` は台帳と依存を検査し、形式の一覧も生成します。台帳と生成した文書を同じ PR に入れてください。

## 手順

- 作業を選ぶときは、[作業の選び方](docs/work.md)に沿って、[#1](https://github.com/zumen-jp/zumen/issues/1) の下の issue の並び・ラベル・依存で、優先順と開始条件を確認してください。issue に範囲と完了条件を書き、PR に残る作業を書いてください。
- 最新の `main` からブランチを作り、PR を出してください。
- PR は、作った人と別の人か、作った側の会話を引き継がないエージェントが確かめてからマージします。確かめた結果は、PR のレビューのコメントに書きます（[決定 0254](docs/decisions/0254-parallel-agents-and-cross-review.md)）。
- コミットと PR のタイトルは Conventional Commits にします（例: `feat(dxf): 外部参照を読む`）。種別とスコープは英語、説明は日本語です。
- API・ID・層とモデル・依存・権利・形式をまたぐ振る舞い・作業の進め方に関わる設計の判断は、[docs/decisions](docs/decisions/README.md) に記録してください。形式ごとのフィールドや符号化の細部は、[docs/formats](docs/formats/README.md) に書きます。
