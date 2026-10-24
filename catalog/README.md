# 診断の code の台帳とカタログ

核は人向けの文を返さず、診断の code と型付きの値を返します。code の性質と人向けの文は、ここに置きます（[決定 0009](../docs/decisions/0009-diagnostics-and-catalog.md)）。

| ファイル | 中身 |
| --- | --- |
| `codes.toml` | code の台帳。code ごとの `severity`・`fidelity`・値の名前と型・まとめ方・説明・足した版・状態と、出来事の語 |
| `ja.toml` | 診断の文と、数の書き方（日本語）。こちらを正にする |
| `en.toml` | 診断の文と、数の書き方（英語）。`ja.toml` の訳 |
| `ja.json`・`en.json` | `ja.toml`・`en.toml` から生成したカタログ。核に `zm_push` で渡し、`describe` の op で文を組み立てる。手で直さない |

カタログの JSON の形は、[schema/messages.json](../schema/messages.json) の `catalog` です。核と同じ版で出し、核とは別のファイルとして配ります。文言が要らないアプリは、読み込まなくて構いません。

## code を足す

1. `codes.toml` に code を足す。名前は `<範囲>.<対象>.<出来事>` にし、出来事は `[events]` の語から選ぶ。
2. `ja.toml` と `en.toml` に文を書く。`{名前}` には台帳の値が、`{count}` にはまとめた件数が入る。
3. `cargo xtask codegen` を実行し、生成した `crates/zumen-base/src/codes.rs` と `ja.json`・`en.json` も一緒にコミットする。

CI の `cargo xtask codegen --check` が、台帳と文の食い違いと、生成したファイルが古いことを見つけます。

一度使った code を、別の意味に使わないでください。使わなくなった code は、`status = "retired"` にして残します。

## 数の書き方

`[number]` の表で決めます。`describe` は、OS のロケールを使わず、この規則で数を文に入れます。

| キー | 中身 |
| --- | --- |
| `group-separator` | 整数部を 3 桁ごとに区切る文字。空なら区切らない |
| `decimal-separator` | 小数点の文字 |

数は、読み直すと同じ値に戻る最短の表記（ECMAScript の `Number::toString` と同じ）にしてから区切ります。指数のある表記は区切りません。
