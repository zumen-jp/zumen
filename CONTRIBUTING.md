# 開発への参加

zumen は作り直しを始めたところです。設計は [#1](https://github.com/zumen-jp/zumen/issues/1) とその子 issue で進めています。

- 大きな変更は、着手する前に issue で相談してください。
- issue・PR・コメントは日本語を基本にします。
- 参加する人には、[行動規範](CODE_OF_CONDUCT.md)が適用されます。

## 図面の取り扱い

図面は、顧客のものであることがあります。

- 機密の図面を issue・PR に添付したり、Git に入れたりしないでください。
- 不具合を報告するときは、再現する最小の図面を作るか、形式・版・作成ソフトと、表示された診断を書いてください。

## 寄稿のライセンス

明示しない限り、寄稿は Apache License 2.0 の定める「寄稿」として、追加の条件なしに MIT と Apache-2.0 の 2 つのライセンスで提供されたものとします（[決定 0002](docs/decisions/0002-dual-license.md)）。

次のものは入れないでください。

- 仕様の本文・図・表を写したもの
- GPL の実装を読んで書いたもの
- 権利のはっきりしないもの

## 開発の準備

Rust は [rustup](https://rustup.rs/) で入れます。版は `rust-toolchain.toml` で決めてあり、リポジトリの中で `rustup toolchain install` を実行すると入ります。

PR を出す前に、CI と同じ検査を手元で通してください。

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo xtask codegen --check
```

境界の crate を変えたときは、wasm32 でも確かめてください。適合試験には Node.js 22 以降が要ります（[bindings/conformance/README.md](bindings/conformance/README.md)）。

```bash
cargo clippy -p zumen-abi --target wasm32-unknown-unknown -- -D warnings
cargo build -p zumen-abi --target wasm32-unknown-unknown --release
node bindings/conformance/wasm.mjs target/wasm32-unknown-unknown/release/zumen_abi1.wasm
```

診断の code を足すときは [catalog/README.md](catalog/README.md)、境界の関数・op・欄を変えるときは [schema/README.md](schema/README.md) の手順に従ってください。

## 手順

- 作業を選ぶときは、[次の作業と進め方](docs/work.md)で優先順と開始条件を確認してください。issue に範囲と完了条件を書き、PR で残る作業と一覧も更新してください。
- 最新の `main` からブランチを作り、PR を出してください。
- コミットと PR のタイトルは Conventional Commits にします（例: `feat(dxf): 外部参照を読む`）。種別とスコープは英語、説明は日本語です。
- 設計の判断を含む変更は、[docs/decisions](docs/decisions/README.md) に記録してください。
