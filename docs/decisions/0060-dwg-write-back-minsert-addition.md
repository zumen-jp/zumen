# 0060: DWG（R2000）への書き戻しで足す INSERT は、列と行の数か間隔を持てば MINSERT として書く

| 項目 | 内容 |
| --- | --- |
| 日付 | 2026-10-27 |
| 状態 | 採用 |

## 背景

[決定 0055](0055-dwg-write-back-attributed-insert-addition.md) までで、表の終わりに足した INSERT（属性を持つものも）を、R2000 の DWG に書き戻せるようにした。足した INSERT は、いつも INSERT の型（0x07）で書き、列と行の数（70・71）と間隔（44・45）を持つものは拒んでいた。

表では、MINSERT も INSERT として置く。読む側は、MINSERT の列と行の数と間隔を、既定の値（1・1・0・0）でなければ置く。R2000 の MINSERT の型ごとの欄は、INSERT の欄の後に列と行の数（BS）と間隔（BD）を持つ。ハンドルの流れは INSERT と同じで、ブロックに続けて、属性を持てば最初と最後の ATTRIB と SEQEND を指す。値を変えたときの書き方（MINSERT の欄も書く）は、すでにある。

## 選択肢

1. 足した INSERT が 70・71・44・45 のどれかを持てば、MINSERT の型（0x08）で書く。持たなければ INSERT の型で書く。
2. 足した INSERT は、これまでどおり INSERT の型だけで書く。

1 は、読み直した表が足した表と同じになる。既定の値だけの MINSERT は INSERT として書くが、表では区別できないので同じになる。

## 決定

選択肢 1 を採る。

- 足した INSERT が列と行の数か間隔を持てば、MINSERT の型で書く。表にない列と行の数は 1、間隔は 0 にする。
- 属性が続く（66）MINSERT も、INSERT と同じく、すぐ後の ATTRIB と SEQEND を持たせる。

## 影響

- 手組みのファイルで、属性を 1 つ持つ MINSERT と、属性のない MINSERT を足して書き戻した。読み直した表は、足した表と同じになった。
- 試料の R2000 の図面のモデル空間にも、属性を 1 つ持つ MINSERT を足して書き戻した。読み直した表は同じになり、CRC の診断も出なかった。
- 足した MINSERT を、AutoCAD で開いて確かめてはいない。
- 図形でないオブジェクトの追加は、まだ書き戻せない。

## 参照

- [0055: DWG（R2000）への書き戻しで足す属性を持つ INSERT は、すぐ後の ATTRIB と SEQEND を INSERT に持たせる](0055-dwg-write-back-attributed-insert-addition.md)
- [DWG の扱い](../formats/dwg.md)
