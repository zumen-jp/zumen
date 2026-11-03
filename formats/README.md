# 形式の権利の台帳

形式ごとの仕様の入手先・権利・実装の根拠と、コアの実装を確かめた度合いを、`ledger.toml` に置きます（[決定 0017](../docs/decisions/0017-format-rights-ledger-and-plugin-sdk.md)・[決定 0254](../docs/decisions/0254-parallel-agents-and-cross-review.md)）。公開に第三者の許諾が要る形式と、権利が分からない形式は、このリポジトリのコアに入れず、外部のプラグインにします。

## 形式を足す

1. 実装を始める前に、`ledger.toml` に形式の項目を足す。公式の仕様があれば、先に `[specs]` に足す。
2. 仕様の版・入手先・権利の根拠は、公式の文書で確かめ、確かめた日を書く。仕様の本文は写さない。
3. 権利が `public` でない形式は、`place` を `core` にしない。
4. 実装する crate の `Cargo.toml` で形式を名乗る。権利が `public` でない形式の既知の crate は、`deny.toml` の `[bans].deny` に例外・版の制限なく加える。
5. 実装したら、読みと書きの[確かめた度合い](#確かめた度合い)を書き、形式の文書に「確かめた度合い」の節を置く。確かめた度合いを上げたときも、台帳と節を同じ PR で直す。
6. `cargo xtask codegen` で一覧を作り直し、台帳と生成した文書を同じ PR に入れる。

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
| `place` | 要る | `core`（このリポジトリのコア）・`plugin`（外部のリポジトリのプラグイン）・`none`（実装しない） |
| `repository` | `plugin` なら要る | プラグインのリポジトリ |
| `basis` | `none` でなければ要る | 実装の根拠: `spec`（仕様）・`inferred`（推定）・`reference`（参照した実装） |
| `references` | `reference` なら要る | 参照した実装（`name`・`license`・`url`）。GPL などの実装のソースは読まない |
| `verify` | | 照合に使う候補の、外部の実装と基準になるソフト。段の根拠にはしない。照らしたら、[確かめた度合い](#確かめた度合い)の `implementations`・`software` に版と一緒に書く（[決定 0255](../docs/decisions/0255-verification-levels-in-the-ledger.md)） |
| `samples` | | サンプルの出どころ。[決定 0171](../docs/decisions/0171-public-samples-in-git.md) の条件に合う公開サンプルは `samples/` に置ける。置いたら、[公開サンプルの台帳](../samples/ledger.toml)に書く。受け取った図面は Git に入れない |
| `known-crates` | | crates.io にある、この形式を読み書きする crate。権利が `public` でない形式のものは、`deny.toml` で禁じる |
| `decisions`・`legacy-decisions` | | 関わる決定の番号の配列。`["0008"]` のように 4 桁の文字列にする |
| `issues` | | 関わる issue（`zumen-jp/zumen-legacy#140` の形） |
| `docs` | 確かめた度合いがあれば要る | 形式の文書のパスの配列。リポジトリの根から `docs/formats/` 内の実在する Markdown を指す |
| `notes` | | そのほかの補足 |
| `read`・`write` | `core` で実装した向きには要る | 読み・書きの[確かめた度合い](#確かめた度合い)。実装していない向きと、`core` でない形式には書かない |

## 確かめた度合い

コアの実装を、読みと書きに分けて、どこまで確かめたかを `[formats.<キー>.read]`・`[formats.<キー>.write]` に書きます（[決定 0254](../docs/decisions/0254-parallel-agents-and-cross-review.md)・[決定 0255](../docs/decisions/0255-verification-levels-in-the-ledger.md)）。境界の `capabilities` には出しません。

| 段 | 読む向き | 書く向き |
| --- | --- | --- |
| `synthetic` | 仕様と、試験の中で作ったファイルだけで確かめた。公開サンプルや受け取った図面を読んでも、値を照らしていなければこの段にする。根拠は要らない | 書いたファイルを、自前の読み取りだけで確かめた。根拠は要らない |
| `external` | 自前で作っていないファイル（公開サンプル・受け取った図面・外部の実装が書いたもの）を読み、外部の実装が読んだ値か、ファイルに添えられた値と照らした。`implementations` か `attached-values` が要る | 書いたファイルを外部の実装が読み、読んだ値が書いた値と合った。`implementations` が要る |
| `application` | その形式の基準になるソフト（作るソフトか、受け取る側のソフト）で開いた結果と照らした。`software` が要る | 書いたファイルを、基準になるソフトで開いて確かめた。`software` が要る |

段は、根拠のうちもっとも強い種類で決まります。その方法で確かめたのが一部の規則だけでも、その段にし、確かめた範囲を `scope` と形式の文書に書きます。公開サンプルと受け取った図面は、読んだだけでは段を上げません。値を照らしていなければ `synthetic` のままにし、読んだことを `scope` に書きます。試験の中で自前で作ったファイル（zumen か試験のコードが書いたもの）を外部の実装にも読ませて照らしたことは、読みでは `external` に数えません。読んだファイルの出どころは検査では確かめないので、`scope` に書きます。台帳の `verify` に載っていることと、コアが読むのに使う依存の crate が読むことは、根拠に数えません。

| キー | 要るか | 中身 |
| --- | --- | --- |
| `level` | 要る | `synthetic`・`external`・`application` |
| `scope` | 要る | 確かめた範囲と方法。確かめていない所と、段を上げる issue も書く |
| `checked` | 要る | 確かめた日。文字列の `YYYY-MM-DD` にする |
| `public-samples` | | 読みだけ。読んだ公開サンプル。`samples/` の中の道筋（`samples/dwg/` のように `/` で終われば、そのフォルダー）は、[公開サンプルの台帳](../samples/ledger.toml)にあるもの。Git にない公開サンプルは URL で書く |
| `received` | | 読みだけ。読んだ受け取った図面の種類と件数（`kind`・`count`）。ファイル名・顧客の名前・中身は書かない |
| `attached-values` | | 読みだけ。読んだ公開サンプルか受け取った図面に添えられていて、読んだ値と照らした値（作ったソフトが添えた一覧など）。`public-samples` か `received` が要る |
| `implementations` | | 読みでは、自前で作っていないファイルを読み、読んだ値を照らした外部の実装（`name`・`version`）。書きでは、書いたファイルを読み、読んだ値が書いた値と合った実装。コアが読むのに使う依存の crate は数えない |
| `software` | | 開いて確かめた、その形式の基準になるソフト（`name`・`version`） |

形式の文書の「確かめた度合い」の節には、次の形の表を置きます。文書を `docs` に持つ形式の、段を持つ向きを過不足なく並べ、段は台帳と同じにします。表の後に、確かめた範囲と、まだ確かめていないことを書きます。`synthetic` の段の規則は、確かめ済みと取られないように書きます。

```markdown
## 確かめた度合い

| 形式 | 向き | 段 |
| --- | --- | --- |
| `ifc` | 読み | `synthetic` |
| `ifc` | 書き | `external` |
```

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
- 確かめた度合いの段が向きと根拠の種類に合うこと、`public-samples` が公開サンプルの台帳にあること、確かめた度合いを持つ形式の文書に節があること。節の表が台帳と過不足なく合い、形の崩れた行がないこと（段を持たない形式の文書も、節があれば照らす）。
- `cargo metadata --format-version 1 --all-features --locked` に含まれる全パッケージの名乗り。台帳にない形式、`public` の `core` でない形式、確かめた度合いのない形式、不正な名乗りを拒む。通常・build・dev・間接の依存を含み、環境で絞らない。
- 権利が `public` でない形式の既知の crate に、例外・版の制限のない禁止があること。`"stb-parser"` または `{ crate = "stb-parser", reason = "理由" }` の形を使う。古い `name` の形も読めるが、`wrappers` による例外や、複数の版だけを禁じる指定では代わりにならない。既知の crate が Cargo の依存に現れた場合も拒む。

`codegen` は [形式と権利の一覧](../docs/formats/README.md)を生成します。`--check` はファイルを書き換えず、不足や古い生成物を検出します。CI の「書式・lint・生成物」でも同じ条件を検査し、依存のライセンスと禁止は cargo-deny でも検査します。

外部の URL の到達、権利の根拠の正しさ、参照した実装のライセンスは、この検査で再確認しません。形式を名乗らず、既知の crate にも載っていない実装も自動では見つかりません。これらは台帳を変更する際の資料確認とレビューで確かめます。
