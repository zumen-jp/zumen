# プラグインのひな形

zumen の外部のリポジトリに置く形式（プラグイン）のひな形（[決定 0017](../../docs/decisions/0017-format-rights-ledger-and-plugin-sdk.md)・[決定 0243](../../docs/decisions/0243-plugin-bundling.md)・[決定 0247](../../docs/decisions/0247-drawing-model-binary.md)）。このリポジトリでは、試験のための形式「zumen の線分の一覧」（`zumen_lines`）で、3 つの束ね方を確かめる。

| ファイル | 中身 |
| --- | --- |
| `src/lib.rs` | コーデック・図面モデルへの lift・診断の code・窓口に登録する `PLUGINS`・wasm の `zm_plugin_init` |
| `src/main.rs` | 「コアとプラグイン」の CLI |
| `ledger.toml` | プラグインの形式の台帳 |
| `catalog.ja.json` | プラグインの診断の文 |
| `plugin.json` | JavaScript の包みに渡す、判定の規則 |
| `tests/forms.rs` | SDK の契約、Rust で束ねる形、`process` の形、図面モデルのバイナリで渡す形の試験 |
| `tests/wasm.mjs` | wasm の別のモジュールの形と、コアの図面のモジュールに渡す形の試験 |

## 新しいプラグインを作る手順

1. このフォルダーを、新しいリポジトリに写す。`Cargo.toml` の `version.workspace` などを値にし、zumen の crate の `path` を外して `version` だけにする。
2. 台帳を書く。`ledger.toml` に形式の項目を書き、`place` を `plugin`、`repository` をそのリポジトリにする。キーは、コアの台帳（`formats/ledger.toml`）のキーとぶつからないものにする。権利が `public` でなければ、許諾の根拠を `rights-note` に書く。
3. コーデックを書く。`zumen_sdk::Codec` と、`FileModel`・`Lift<Drawing2D>` を実装する。判定の規則（`SIGNATURES`）は、コアの形式とぶつからないものにする。コアの形式は、プラグインの前に試す。
4. 診断の code を書く。名前は形式のキーと `.` で始め（`<キー>.file.invalid` など）、`zumen_base::Code::plugin` で作る。文は `catalog.<言語>.json` に書く。describe の op には、コアのカタログ（`catalog/<言語>.json`）の `codes` にプラグインの `codes` を足したものを渡す。
5. 試験を書く。`zumen_sdk::testkit` の `read`・`stops_at_same_place`・`survives` で、予算で止める位置と、切れた入力でパニックしないことを確かめる。試験のデータは、試験の中で作る合成のものにする。
6. 束ねる。
   - Rust: プラグインの crate を依存に足し、`zumen_abi1::install(PLUGINS)` を呼んでからセッションを作る。登録はプロセスで 1 度だけで、登録の後に開いた文書から、コアの形式でない入力をプラグインで試す。
   - wasm: `cargo build --lib --target wasm32-unknown-unknown --release` で、プラグインの形式だけを持つモジュールを作る（`zumen-abi` の `plugin` の feature）。JavaScript の包みの `loadModules` に、`plugin.json` の中身に `ops`（`MODULES.plugin.ops`）と `load` を足したものを渡す。包みは、読み込んだときに `zm_plugin_init` を呼ぶ。プラグインのモジュールで開いた文書を `lift` の op で図面モデルのバイナリにし、`take` で取り出したバイト列を、コアの図面のモジュールのセッションに `push` して `open` すると、元の形式とハッシュの文書として、図面を読む op（drawing・entities・scene など）を使える。
   - ネイティブで呼ぶ言語: `cargo build --release` で作る実行ファイルが、「コアとプラグイン」の CLI になる。各言語の包みは、設定の `engine` を `process` にし、`process_path` にその実行ファイルを渡す。

## 今の制約

- プラグインの形式は、図面の形式（`Lift<Drawing2D>`、`Plugin::of`）か建物の形式（`Lift<Building>`、`Plugin::building`、決定 0245）で、読むだけ。apply・building_apply では書き戻せない。建物の形式の wasm のモジュールは、`zumen-abi` の `bim` の feature と一緒に作る。
- wasm のモジュールから図面のモジュールへの受け渡しは、使う側が書く。包みの `loadModules` は、まだ自動で渡さない（[#483](https://github.com/zumen-jp/zumen/issues/483)）。
- 建物の形式のプラグインの wasm のモジュールは、建物モデルのバイナリがまだないので、コアの建物のモジュールを中に持つ（`bim` の feature、[#484](https://github.com/zumen-jp/zumen/issues/484)）。
- コアのワークスペースで試すため、このフォルダーの crate はワークスペースの一員で、zumen の crate を `path` で指す。
