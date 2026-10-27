# 0076: R2004 の DWG への図形でないオブジェクトの書き戻しは、LAYOUT を除いて R2000 と同じに符号化する

| 項目 | 内容 |
| --- | --- |
| 日付 | 2026-10-27 |
| 状態 | 採用 |

## 背景

R2004 の DWG へは、図形と表のレコードの欄を書き戻す（[決定 0072](0072-dwg-r2004-write-back.md)〜[0075](0075-dwg-r2004-write-back-records.md)）。OBJECTS の GROUP・DICTIONARYVAR・DICTIONARY・ACDBDICTIONARYWDFLT・MLINESTYLE・LAYOUT・XRECORD は、R2000 では書き戻す（[決定 0047](0047-dwg-write-back-group-dictionaryvar.md)〜[0050](0050-dwg-write-back-xrecord.md)）が、R2004 ではまだ受け付けていなかった。R2004 では、反応子の数の後に拡張辞書を持たない印（B）が続き、MLINESTYLE の色は CMC になる。LAYOUT は、R2004 から印刷の設定の欄が増え、読む側がまだ R2004 の LAYOUT の欄を読まない（試料の図面では、ハンドルだけが表に置かれる）。

## 選択肢

1. LAYOUT を除く種類を、R2000 と同じに符号化し、MLINESTYLE の色は表のレコードと同じく CMC の形で書く。LAYOUT は、読む側を直すまで受け付けない。
2. LAYOUT の読む側も直してから、すべての種類を書き戻す。

## 決定

選択肢 1 を採る。LAYOUT の読む側を直すには、R2004 で増えた欄を図面で確かめる必要があり、ほかの種類の書き戻しとは分けて扱う。

- 型ごとの欄の始まりは、表のレコードと同じく、反応子の数の後の B を読み飛ばした位置とする。
- MLINESTYLE の色と線ごとの色は、変わらなければ元のビットを写し、変わったものは番号の形の CMC で書く（[決定 0075](0075-dwg-r2004-write-back-records.md)）。
- R2004 の LAYOUT の変更は、これまでどおり書き戻さない。

## 影響

- 試料の R2004 の 2 つの図面で、辞書の複製の扱い、図面の変数の値、多重線の体裁の線の位置と色、XRECORD の実数と文字列を変えて書き戻し、読み直した表が変えた表と同じになった。試料に GROUP はなく、GROUP の書き戻しは試料では確かめていない。
- R2004 の LAYOUT の読み書きは、残件とする。
- 書いた図面を AutoCAD で開いて確かめてはいない。

## 参照

- [0075: R2004 の DWG への表のレコードの書き戻しは、色を CMC の形で書く](0075-dwg-r2004-write-back-records.md)
- [DWG の扱い](../formats/dwg.md)
