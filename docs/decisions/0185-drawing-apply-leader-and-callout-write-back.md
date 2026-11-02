# 0185: 引出線の書き戻しを、注記つきの追加・DWG・変換と MULTILEADER の枝の置き換えへ広げる

| 項目 | 内容 |
| --- | --- |
| 日付 | 2026-10-31 |
| 状態 | 採用 |

## 背景

[決定 0177](0177-drawing-apply-leaders-and-definitions.md) で、LEADER の経路・曲線か・矢印の置き換えと、注記のない LEADER の DXF への追加を書き戻した。ただし、注記を持つ追加、DWG への追加、R2010 以降の DWG の置き換え、矢印（71）の DWG への書き戻し、`Edit::Transform` は断っていた。引出線つきの注記（MULTILEADER）は、境界の形の値を持たず、書き戻しもしなかった（#332）。

## 選択肢

1. 今のまま断る。
2. 形式が欄を持つものから書き、欄の組を決め直せないもの（MULTILEADER の注記や文脈など）は `change.edit.unsupported` か `dwg.change.unsupported` で断る。
3. MULTILEADER も含め、足すことと注記の書き直しまで、元の形式の欄を推定して書く。

## 決定

選択肢 2 を採る。3 は、MULTILEADER の文脈（注記の位置・向き・大きさの組と、スタイルの上書きの印）を推定で埋めることになり、AutoCAD が読み直したときの見え方を確かめられない。新しい診断は作らない。

- **注記つきの LEADER の追加。** 境界の `annotation` は、図面にある要素の ID で指し、変更を作るときに要素の出所へ直す。書き戻す側は、指す先が同じ空間か用紙（定義の中なら同じ定義）の MTEXT であることを確かめ、73 = 0、注記の大きさ（40・41）にその MTEXT の高さと幅、340 にそのハンドルを書く。ほかの要素を指す追加は `change.edit.unsupported` で断る。
- **DWG の LEADER。** 読むときに、鉤の線の印の後のビットを矢印（71）として読む。置き換えは R2010 以降の DWG（ハンドルの流れの大きさと文字列の流れを持つ版）でも書き、矢印を書く。追加は R2000 から R2018 までの DWG に新しく符号化して書く。DWG は矢印の印を必ず持つので、矢印を寸法スタイルに従わせる（`arrow` がない）引出線は、置き換えでも追加でも `dwg.change.unsupported` か `change.edit.unsupported` で断る。
- **変換。** `Edit::Transform` は経路の頂点を写し、X 軸の向き（211）を同じ変換の向きで直す。曲線の引出線は、相似でない変換では曲線の形が変わるので断る。ずれ（212・213）は元のまま残す。
- **MULTILEADER。** 境界の `geometry` に `callout_geometry`（枝・線の引き方・ランディング・端部記号の大きさ・倍率・注記の種類と中身）を足し、`entities` が返すようにする。置き換えは DXF だけで、枝の線の頂点・集まる点・折れ曲がりの向きと長さを書き直す。ほかの欄の変更と、枝と線の数、集まる点と折れ曲がりの有無の変更は断る。線の切れ目（線の 11・12、枝の 12・13）を持つ線か枝は、切れ目を頂点から決め直せないので変えない。MULTILEADER の追加、DWG の MULTILEADER の書き戻し、変換は断る。
- **境界。** `callout_geometry` と、枝 `callout_branch`・折れ曲がり `callout_dogleg`・線の引き方 `callout_path`・注記の種類 `callout_content` を足し、小版を 36 に上げる。足す引出線の `annotation` を受けるようにする。

## 影響

- 足した注記つきの LEADER の注記の MTEXT には、反応子（330）を書かない。AutoCAD が注記を動かしても、引出線は追わないことがある。
- 変換は 212・213（注記からのずれ）を元のまま残すので、回す変換の後は、注記との組の見え方が変わることがある。
- DWG に新しく足す LEADER の注記の大きさ（40・41）は、注記がなければ 0 と 0 にする。AutoCAD 2027 が書いた注記のない LEADER がこの値を持つ（#345）。R2010 からの DWG の LEADER は、この欄を持たない。
- MULTILEADER の注記の位置や中身の書き直し、DWG の MULTILEADER、追加は残る。
- 反応子・212・213・MULTILEADER の一部は [決定 0191](0191-drawing-apply-multileaders-and-reactors.md) で足した。
- 書き戻した LEADER と MULTILEADER は、AutoCAD 2027 で DXF と DWG（R2000・R2004・R2010・R2018）を開いて確かめた。足した・置き換えた・変換した引出線の形と矢印と注記が合い、注記を動かすと引出線がついてくる。監査の誤りは 0 だった（#345）。

## 参照

- [決定 0110](0110-drawing-callouts-and-dimension-text.md)
- [決定 0139](0139-drawing-apply.md)
- [決定 0155](0155-drawing-apply-additions.md)
- [決定 0177](0177-drawing-apply-leaders-and-definitions.md)
- #332
