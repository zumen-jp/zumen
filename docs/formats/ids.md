# IDS

IDS（Information Delivery Specification）1.0 を読み、建物モデルで照合する規則は、[決定 0217](../decisions/0217-ids-check.md) にあります。実装は `crates/zumen-ids` です。XSD と、そこから生成したコードは同梱しません（`formats/ledger.toml`）。

## 確かめた度合い

| 形式 | 向き | 段 |
| --- | --- | --- |
| `ids` | 読み | `synthetic` |

段の意味は[決定 0254](../decisions/0254-parallel-agents-and-cross-review.md)に、根拠は[形式と権利の一覧](README.md#format-ids)にあります。IDS の読み取りと照合の規則は、仕様と、試験の中で作ったファイルだけで確かめたものです。公開サンプル・受け取った図面・外部の実装・基準になるソフトとは、まだ照らしていません。実際のファイルでは、ここに書いた扱いと違うことがあります。
