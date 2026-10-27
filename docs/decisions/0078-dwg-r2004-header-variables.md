# 0078: R2004 の DWG の見出しの変数は、展開した見出しの区切りの中で書き直し、区切りを置き換える

| 項目 | 内容 |
| --- | --- |
| 日付 | 2026-10-27 |
| 状態 | 採用 |

## 背景

R2000 の DWG の見出しの変数の HANDSEED（[決定 0052](0052-dwg-write-back-entity-addition.md)）と、INSUNITS・PLIMMIN・PLIMMAX（[決定 0069](0069-dwg-write-back-header-variables.md)）は、見出しの変数の区切りの中でその場で書き直す。R2004 では、見出しの変数は入れ物の `AcDb:Header` の区切りにあり、区切りの中の形（始まりの印・大きさ・データ・CRC・終わりの印）は R2000 と同じで、変数の並びが一部違う。読む側は、R2004 の並びで HANDSEED から INSUNITS までを読む。R2004 の DWG に図形を足すには、HANDSEED を進める必要がある。

## 選択肢

1. 展開した `AcDb:Header` の区切りを、R2000 と同じ方法で R2004 の変数の順に沿って書き直し、入れ物の区切りを置き換える（[決定 0070](0070-dwg-r2004-container-write.md)）。
2. R2004 の見出しの変数は書き戻さない。

## 決定

選択肢 1 を採る。

- HANDSEED・INSUNITS・PLIMMIN・PLIMMAX の位置は、読む側と同じ R2004 の変数の並びで求める。
- 書き直すときは、R2000 と同じく区切りのデータの大きさ（バイト）を変えず、CRC を計算し直す。
- 見出しの変数が変わったときだけ、入れ物の `AcDb:Header` を置き換える。
- R2010 から後は、ハンドルが別の流れにあるので、まだ書き換えない。

## 影響

- 試料の R2004 の 2 つの図面で、INSUNITS・PLIMMIN・PLIMMAX を変えて書き戻し、読み直した表が変えた表と同じになり、CRC の診断も出なかった。HANDSEED を 1 進めて書き、読み直した値が進めた値になった。
- R2004 の DWG に図形を足す準備ができた。
- 書いた図面を AutoCAD で開いて確かめてはいない。

## 参照

- [0052: DWG（R2000）への書き戻しは、表の終わりに足した単純な図形を一から符号化し、ブロックの並びの終わりに繋いで HANDSEED を進める](0052-dwg-write-back-entity-addition.md)
- [0069: DWG（R2000）への書き戻しで変えた INSUNITS・PLIMMIN・PLIMMAX は、見出しの変数の区切りの長さを変えずにその場で書き直す](0069-dwg-write-back-header-variables.md)
- [0070: R2004 の入れ物は、置き換えた区切りだけを圧縮し直し、ページを並べ直して地図と見出しを作り直して書く](0070-dwg-r2004-container-write.md)
- [DWG の扱い](../formats/dwg.md)
