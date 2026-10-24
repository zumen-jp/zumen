# 形式の権利の台帳

形式ごとの仕様の入手先・権利・実装の根拠を、`ledger.toml` に置きます（[決定 0017](../docs/decisions/0017-format-rights-ledger-and-plugin-sdk.md)）。公開に第三者の許諾が要る形式と、権利が分からない形式は、このリポジトリの核に入れず、外部のプラグインにします。

## 形式を足す

1. 実装を始める前に、`ledger.toml` に形式の項目を足す。公式の仕様があれば、先に `[specs]` に足す。
2. 仕様の版・入手先・権利の根拠は、公式の文書で確かめ、確かめた日を書く。仕様の本文は写さない。
3. 権利が `public` でない形式は、`place` を `core` にしない。

## 仕様（`[specs.<ID>]`）

ID は英小文字・数字・`_` にし、版を含めます（`sxf_3_1` など）。診断の `cite`（[決定 0009](../docs/decisions/0009-diagnostics-and-catalog.md)）は、この ID で仕様を指します。

| キー | 要るか | 中身 |
| --- | --- | --- |
| `title` | 要る | 仕様の名前 |
| `publisher` | 要る | 仕様を出している者 |
| `version` | | 版と日付 |
| `access` | 要る | `public`（誰でも無償で入手できる）・`members`（会員などにだけ配る）・`paid`（買う） |
| `url` | 要る | 入手先 |
| `checked` | 要る | 確かめた日 |
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
| `decisions`・`legacy-decisions` | | 関わる決定の記録と、旧版の決定の番号 |
| `issues` | | 関わる issue（`zumen-jp/zumen-legacy#140` の形） |
| `docs` | | 形式の文書（`docs/formats/` のファイル） |
| `notes` | | そのほかの補足 |

## 権利

| 値 | 意味 | 置ける所 |
| --- | --- | --- |
| `public` | 実装を公開するのに、第三者の許諾が要らない。持ち主が仕様を公開して実装を制限していないもの、または持ち主の仕様がなく、公開された資料と推定で読むもの | `core`・`plugin` |
| `permission` | 持ち主が、仕様の入手か実装の公開に、契約・会員・許諾を求めている | `plugin`・`none` |
| `unknown` | 持ち主か、その条件を確かめられていない | `plugin`・`none` |

## 検査

crate は、実装する形式を `Cargo.toml` の `[package.metadata.zumen]` の `formats` で名乗ります。`cargo xtask` が、ワークスペースと依存のすべての crate の名乗りを台帳と照らし、`place` が `core` でない形式があれば失敗にします。この検査と、台帳から形式の一覧を作る生成は、次の PR で入れます（[決定 0017](../docs/decisions/0017-format-rights-ledger-and-plugin-sdk.md) の影響）。それまでは、レビューで台帳を確かめます。
