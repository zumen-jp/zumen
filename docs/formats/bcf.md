# BCF

BCF（BIM Collaboration Format）2.1・3.0 の bcfzip を読み書きし、レビューの文書に写す規則は、決定の記録にあります。実装は `crates/zumen-bcf` です。XSD は同梱しません（`formats/ledger.toml`）。

- レビューの文書と、BCF との対応: [決定 0214](../decisions/0214-review-model-and-bcf.md)
- BCF 3.0 の読み書き: [決定 0220](../decisions/0220-bcf-3-0.md)
- BCF 2.1 の読み書き: [決定 0248](../decisions/0248-bcf-2-1.md)

## 確かめた度合い

| 形式 | 向き | 段 |
| --- | --- | --- |
| `bcf` | 読み | `synthetic` |
| `bcf` | 書き | `synthetic` |

段の意味は[決定 0254](../decisions/0254-parallel-agents-and-cross-review.md) に、根拠は[形式と権利の一覧](README.md#format-bcf)にあります。BCF の読み書きの規則は、仕様と、テストの中で作ったファイルだけで確かめたものです。公開サンプル・受け取った図面・外部の実装・基準になるソフトとは、まだ照らしていません。実際のファイルでは、ここに書いた扱いと違うことがあります。

- 書いた bcfzip は読み直して確かめたが、外部の実装や BCF を読むソフトでは読ませていない。
- 公開サンプルと外部の実装での照合は #522 で行う。
- Autodesk Build の取り込み・書き出しには、[オフラインの検証キット](../../xtask/verify/acc/README.md)を使う（#686・#787）。公開 IFC の要素に合う BCF 2.1 を作り、題・状態・コメントを書き戻して、選択要素・カメラ・画像の保持を zumen で読み直す。製品での確認と wasm との照合は、このキットを作っただけでは確認済みとしない。
