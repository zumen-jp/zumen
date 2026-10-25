# 形式の権利の台帳

形式ごとの仕様の入手先・権利・実装の根拠を、`ledger.toml` に置きます（[決定 0017](../docs/decisions/0017-format-rights-ledger-and-plugin-sdk.md)）。公開に第三者の許諾が要る形式と、権利が分からない形式は、このリポジトリの核に入れず、外部のプラグインにします。

## 形式を足す

1. 実装を始める前に、`ledger.toml` に形式の項目を足す。公式の仕様があれば、先に `[specs]` に足す。
2. 仕様の版・入手先・権利の根拠は、公式の文書で確かめ、確かめた日を書く。仕様の本文は写さない。
3. 権利が `public` でない形式は、`place` を `core` にしない。
4. 実装する crate の `Cargo.toml` で形式を名乗る。権利が `public` でない形式の既知の crate は、`deny.toml` の `[bans].deny` に例外・版の制限なく加える。
5. `cargo xtask codegen` で一覧を作り直し、台帳と生成した文書を同じ PR に入れる。

## 仕様（`[specs.<ID>]`）

ID は英小文字・数字・`_` にし、版を含めます（`sxf_3_1` など）。診断の `cite`（[決定 0009](../docs/decisions/0009-diagnostics-and-catalog.md)）は、この ID で仕様を指します。

| キー | 要るか | 中身 |
| --- | --- | --- |
| `title` | 要る | 仕様の名前 |
| `publisher` | 要る | 仕様を出している者 |
| `version` | | 版と日付 |
| `access` | 要る | `public`（誰でも無償で入手できる）・`members`（会員などにだけ配る）・`paid`（買う） |
| `url` | 要る | 入手先 |
| `checked` | 要る | 確かめた日。文字列の `YYYY-MM-DD` にする |
| `note` | | 補足 |

## 形式（`[formats.<キー>]`）

キーは英小文字と数字だけにします（`sxf`・`stbridge` など）。境界の列挙の値（[決定 0012](../docs/decisions/0012-boundary-functions-and-messages.md)）にも、診断の code の範囲にも、そのまま使えるようにするためです。

| キー | 要るか | 中身 |
| --- | --- | --- |
| `name` | 要る | 形式の名前 |
| `family` | 要る | L1 の系統（[決定 0008](../docs/decisions/0008-model-layers-and-codecs.md)）。同じ系統の形式は、1 つのファイルモデルを共有する |
| `domains` | 要る | L2 の領域: `drawing`・`building`・`civil`・`geo`・`pointcloud`・`delivery`・`review` |
| `specs` | 要る | 仕様の ID の並び。公式の仕様がなければ空にする |
| `rights` | 要る | 下の 3 つの値 |
| `rights-note` | 要る | 権利の根拠の要約 |
| `evidence` | | 仕様のほかに、権利の根拠にした文書（`title`・`url`・`checked`・`note`） |
| `place` | 要る | `core`（このリポジトリの核）・`plugin`（外部のリポジトリのプラグイン）・`none`（実装しない） |
| `repository` | `plugin` なら要る | プラグインのリポジトリ |
| `basis` | `none` でなければ要る | 実装の根拠: `spec`（仕様）・`inferred`（推定）・`reference`（参照した実装） |
| `references` | `reference` なら要る | 参照した実装（`name`・`license`・`url`）。GPL などの実装のソースは読まない |
| `verify` | | 照合に使うソフト |
| `samples` | | サンプルの出どころ。サンプルは Git に入れない |
| `known-crates` | | crates.io にある、この形式を読み書きする crate。権利が `public` でない形式のものは、`deny.toml` で禁じる |
| `decisions`・`legacy-decisions` | | 関わる決定の番号の配列。`["0008"]` のように 4 桁の文字列にする |
| `issues` | | 関わる issue（`zumen-jp/zumen-legacy#140` の形） |
| `docs` | | 形式の文書のパスの配列。リポジトリの根から `docs/formats/` 内の実在する Markdown を指す |
| `notes` | | そのほかの補足 |

## 権利

| 値 | 意味 | 置ける所 |
| --- | --- | --- |
| `public` | 実装を公開するのに、第三者の許諾が要らない。持ち主が仕様を公開して実装を制限していないもの、または持ち主の仕様がなく、公開された資料と推定で読むもの | `core`・`plugin` |
| `permission` | 持ち主が、仕様の入手か実装の公開に、契約・会員・許諾を求めている | `plugin`・`none` |
| `unknown` | 持ち主か、その条件を確かめられていない | `plugin`・`none` |

## 検査

crate は、実装する形式を `Cargo.toml` の `[package.metadata.zumen]` の `formats` で名乗ります。

```toml
[package.metadata.zumen]
formats = ["dxf", "dwg"]
```

`cargo xtask codegen` と `cargo xtask codegen --check` は、次を検査します。

- 要る項目・値の語・キー・空欄・重複、仕様の参照、HTTP または HTTPS の入手先、実在する日付。
- `core` の権利が `public` であること、`plugin` のリポジトリ、実装の根拠に必要な仕様や参照した実装。
- 決定の記録と形式の文書があること。旧版の決定は番号の形、issue は `所有者/リポジトリ#番号` の形を確かめる。
- `cargo metadata --format-version 1 --all-features --locked` に含まれる全パッケージの名乗り。台帳にない形式、`public` の `core` でない形式、不正な名乗りを拒む。通常・build・dev・間接の依存を含み、環境で絞らない。
- 権利が `public` でない形式の既知の crate に、例外・版の制限のない禁止があること。`"stb-parser"` または `{ crate = "stb-parser", reason = "理由" }` の形を使う。古い `name` の形も読めるが、`wrappers` による例外や、複数の版だけを禁じる指定では代わりにならない。既知の crate が Cargo の依存に現れた場合も拒む。

`codegen` は [形式と権利の一覧](../docs/formats/README.md)を生成します。`--check` はファイルを書き換えず、不足や古い生成物を検出します。CI の「書式・lint・生成物」でも同じ条件を検査し、依存のライセンスと禁止は cargo-deny でも検査します。

外部の URL の到達、権利の根拠の正しさ、参照した実装のライセンスは、この検査で再確認しません。形式を名乗らず、既知の crate にも載っていない実装も自動では見つかりません。これらは台帳を変更する際の資料確認とレビューで確かめます。
