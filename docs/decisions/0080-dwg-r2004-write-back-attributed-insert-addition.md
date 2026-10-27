# 0080: R2004 の DWG に足す属性を持つ INSERT は、ATTRIB の数とすべての ATTRIB のハンドルを持つ

| 項目 | 内容 |
| --- | --- |
| 日付 | 2026-10-27 |
| 状態 | 採用 |

## 背景

R2004 の DWG へは、すでにあるブロックの単純な図形と属性を持たない INSERT を足せる（[決定 0079](0079-dwg-r2004-write-back-entity-addition.md)）。R2000 では、属性を持つ INSERT は最初と最後の ATTRIB と SEQEND のハンドルを持ち、ATTRIB どうしを前後の図形のハンドルで繋ぐ（[決定 0055](0055-dwg-write-back-attributed-insert-addition.md)）。ODS 5.4.1 によれば、R2004 の INSERT は、属性を持つとき型ごとの欄の終わりに持つオブジェクトの数（BL）を持ち、ハンドルの流れにそのオブジェクトのハンドルを並べる。MINSERT は、その数を列と行の数の前に持つ。

## 選択肢

1. 属性を持つ INSERT を、ATTRIB の数とすべての ATTRIB のハンドルを持つ形で書く。MINSERT は、試料で形を確かめられるまで足さない。
2. 属性を持つ INSERT も MINSERT も、まだ足さない。

## 決定

選択肢 1 を採る。試料の R2004 の図面の属性を持つ INSERT で、次の形を確かめた。

- 型ごとの欄の終わりに、ATTRIB の数（BL）が続く。
- ハンドルの流れは、画層・ブロックの後に、ATTRIB のハンドルが数だけ固い所有（符号 3）で並び、SEQEND のハンドル（符号 3）が続く。

足す ATTRIB と SEQEND は、R2004 の共通の欄で、所有者のハンドル（INSERT）を持つ形で書く。ブロックのレコードの持つ図形には、INSERT だけを加える。MINSERT は、試料に属性を持つものがなく、読む側も R2004 で増える数を読まないので、まだ足さない。残件とする。

## 影響

- 試料の R2004 の 2 つの図面のモデル空間に、属性を 2 つ持つ INSERT を足して書き戻し、読み直した表が足した表と同じになった。
- 書いた図面を AutoCAD で開いて確かめてはいない。

## 参照

- [0055: DWG（R2000）への書き戻しで足す属性を持つ INSERT は、すぐ後の ATTRIB と SEQEND を INSERT に持たせる](0055-dwg-write-back-attributed-insert-addition.md)
- [0079: R2004 の DWG への図形の足しは、R2004 の共通の欄で符号化し、ブロックのレコードの持つ図形に加える](0079-dwg-r2004-write-back-entity-addition.md)
- [DWG の扱い](../formats/dwg.md)
