# BCF

BCF（BIM Collaboration Format）2.1・3.0 の bcfzip を読み書きし、レビューの文書に写す規則は、決定の記録にあります。実装は `crates/zumen-bcf` です。XSD は同梱しません（`formats/ledger.toml`）。

- レビューの文書と、BCF との対応: [決定 0214](../decisions/0214-review-model-and-bcf.md)
- BCF 3.0 の読み書き: [決定 0220](../decisions/0220-bcf-3-0.md)
- BCF 2.1 の読み書き: [決定 0248](../decisions/0248-bcf-2-1.md)

## 確かめた度合い

| 形式 | 向き | 段 |
| --- | --- | --- |
| `bcf` | 読み | `spec` |
| `bcf` | 書き | `spec` |

段の意味は[決定 0253](../decisions/0253-parallel-agents-and-cross-review.md)に、根拠は[形式と権利の一覧](README.md#format-bcf)にあります。BCF の読み書きの規則は、仕様と、試験の中で作ったファイルだけで確かめたものです。公開の試料・外部の実装・実際のソフトとは、まだ照らしていません。実際のファイルでは、ここに書いた扱いと違うことがあります。

- 書いた bcfzip は読み直して確かめたが、BCF を読むソフトでは開いていない。
