# 診断の code の台帳とカタログ

核は人向けの文を返さず、診断の code と型付きの値を返します。code の性質と人向けの文は、ここに置きます（[決定 0009](../docs/decisions/0009-diagnostics-and-catalog.md)）。

| ファイル | 中身 |
| --- | --- |
| `codes.toml` | code の台帳。code ごとの `severity`・`fidelity`・値の名前と型・まとめ方・説明・足した版・状態と、出来事の語 |
| `ja.toml` | 診断の文（日本語）。こちらを正にする |
| `en.toml` | 診断の文（英語）。`ja.toml` の訳 |

## code を足す

1. `codes.toml` に code を足す。名前は `<範囲>.<対象>.<出来事>` にし、出来事は `[events]` の語から選ぶ。
2. `ja.toml` と `en.toml` に文を書く。`{名前}` には台帳の値が、`{count}` にはまとめた件数が入る。
3. `cargo xtask codegen` を実行し、生成した `crates/zumen-base/src/codes.rs` も一緒にコミットする。

CI の `cargo xtask codegen --check` が、台帳と文の食い違いと、生成したファイルが古いことを見つけます。

一度使った code を、別の意味に使わないでください。使わなくなった code は、`status = "retired"` にして残します。
