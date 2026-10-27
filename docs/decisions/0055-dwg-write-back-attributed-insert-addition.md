# 0055: DWG（R2000）への書き戻しで足す属性を持つ INSERT は、すぐ後の ATTRIB と SEQEND を INSERT に持たせる

| 項目 | 内容 |
| --- | --- |
| 日付 | 2026-10-27 |
| 状態 | 採用 |

## 背景

[決定 0053](0053-dwg-write-back-pointing-entity-addition.md) で足した INSERT を R2000 の DWG に書き戻せるようにしたが、属性が続く印（66）を持つものは受け付けなかった。DXF では、属性を持つ INSERT の後に ATTRIB が並び、SEQEND で閉じる。共有の表も、この並びで読む。

試料の R2000 の図面の、属性を持つ INSERT を調べると、次の形だった。

- INSERT は、ハンドルの欄で、ブロックのハンドルに続けて、最初の ATTRIB・最後の ATTRIB（どちらもコード 4）と SEQEND（コード 3）を指す。
- ATTRIB と SEQEND は、entmode が 0 で、所有者のハンドルに INSERT を書く。
- ATTRIB は ATTRIB どうしで前後の図形のハンドルを繋ぎ、最初の前と最後の後は 0。SEQEND の前後は 0。
- ブロックのレコードの図形の並びへ入るのは INSERT だけで、ATTRIB と SEQEND は入らない。

## 選択肢

1. 66 を持つ INSERT のすぐ後に、その INSERT を所有者とする ATTRIB を 1 つ以上並べ、SEQEND で閉じたものだけを受け付ける。
2. 表のどこにある ATTRIB と SEQEND でも、所有者の INSERT を探して受け付ける。
3. 属性を持つ INSERT は、まだ受け付けない。

1 は DXF と共有の表の並びと同じで、足した図形を順に符号化できる。2 は並びが崩れた表を受け付けることになり、確かめる手間が増える。

## 決定

選択肢 1 を採る。

- ATTRIB（型 0x02）と SEQEND（型 0x06）を足せるようにする。ATTRIB の値は、値を変えたときと同じ書き方で書き、文字の体裁（7）は TEXT と同じく探す。SEQEND は型ごとの欄を持たない。
- ATTRIB と SEQEND は、すぐ前の INSERT を所有者（330）とし、その INSERT と同じ所に置かなければならない。用紙空間の印（67）は持たない。
- INSERT のハンドルの欄に、最初と最後の ATTRIB と SEQEND を書く。前後の図形のハンドルは、上の試料と同じ形に繋ぐ。ブロックのレコードの図形の並びへ繋ぐのは INSERT だけにする。
- SEQEND で閉じない INSERT、ATTRIB のない SEQEND、INSERT のすぐ後にない ATTRIB と SEQEND は受け付けない。66 は 1 だけを受け付ける。

## 影響

- 手組みのファイルに、属性を 2 つ持つ INSERT を足して書き戻し、読み直した表が足した表と同じになることを確かめた。上の受け付けない並びは、書き戻さずに拒む。
- 試料の R2000 の図面のうち、名前のあるブロックを持つもののモデル空間に、属性を持つ INSERT を足して書き戻し、読み直した表が同じになり、CRC の診断が出ないことを確かめた。
- ATTRIB の並びの順は、ハンドルの順と同じにする。試料には、SEQEND のハンドルが ATTRIB の間にあるものもあったが、そうした並びは作らない。
- 属性を持つ MINSERT、LWPOLYLINE・SPLINE・SHAPE など表にない欄を元のデータから写すほかの図形、図形でないオブジェクトの追加は、まだ書き戻せない。

## 参照

- [0053: DWG（R2000）への書き戻しで足す TEXT・ATTDEF・INSERT は、指す文字の体裁とブロックを名前で探してハンドルを書く](0053-dwg-write-back-pointing-entity-addition.md)
- [0052: DWG（R2000）への書き戻しで、表の終わりに足した単純な図形を符号化する](0052-dwg-write-back-entity-addition.md)
- [DWG の扱い](../formats/dwg.md)
