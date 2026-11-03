# PLY

PLY（台帳の `ply`）の点群を読み、点群の領域モデル（[決定 0232](../decisions/0232-point-clouds.md)）に移す規則を記録します。実装は `crates/zumen-ply` です。持ち主の決めた仕様はないので、広く使われる書き方（台帳の根拠）に従います。

## 境界

- `open` は、入力の最初の行が `ply` なら PLY として開く。`pointcloud` の op で、要約と点を返す（[LAS・LAZ](las.md) と同じ）。
- 書き出しはしない。

## 見出しと本体

- 見出しは `format`（`ascii`・`binary_little_endian`・`binary_big_endian`）・`element`・`property`（`list` を含む）を `end_header` まで読む。`comment` と `obj_info` は飛ばす。
- 値の型は `char`・`uchar`・`short`・`ushort`・`int`・`uint`・`float`・`double` と、`int8`〜`float64` の別名を読む。
- 本体は、見出しの要素の順に、要素の数だけ読む。列の性質（`list`）は、長さの分を飛ばす。本体が見出しの数より短いか、値を読めなければ、`ply.file.invalid` を返して開かない。

## 点

- `vertex` の要素を点にする。ほかの要素（`face` など）は移さず、`ply.element.dropped` を返す。
- `x` を東、`y` を北、`z` を標高にする。`x`・`y`・`z` のどれかがなければ、`ply.file.invalid` を返す。
- 色は `red`・`green`・`blue`（`diffuse_` の別名を含む）を読み、8 ビットの値は 257 倍して 16 ビットにする。強度は `intensity`、分類は `classification` を読む。CloudCompare がつける `scalar_` の接頭辞は除いて読む。浮動小数の強度が 0〜1 に収まれば、65535 倍する。
- ほかの性質（法線など）と列の性質は移さず、性質ごとに `ply.property.dropped` を返す。
- PLY は座標系を書かないので、座標系を不明にして `ply.crs.missing` を返す。

## 確かめた度合い

| 形式 | 向き | 段 |
| --- | --- | --- |
| `ply` | 読み | `spec` |

段の意味は[決定 0253](../decisions/0253-parallel-agents-and-cross-review.md)に、根拠は[形式と権利の一覧](README.md#format-ply)にあります。持ち主の公式の仕様はないので、この文書の規則は、公開の解説と、試験の中で作ったファイルだけで確かめたものです。公開の試料・外部の実装・実際のソフトとは、まだ照らしていません。実際のファイルでは、ここに書いた扱いと違うことがあります。

- CloudCompare・MeshLab が書いた PLY との照合は #504 で行う。
