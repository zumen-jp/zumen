# 0208: 記号の反転した切り抜きは、SPATIAL_FILTER の拡張辞書の XRECORD にある元の境界から読む

| 項目 | 内容 |
| --- | --- |
| 日付 | 2026-10-31 |
| 状態 | 採用 |

## 背景

[決定 0149](0149-drawing-symbol-clips.md) では、INSERT の切り抜き（XCLIP）を SPATIAL_FILTER の境界から作る。境界の内側を隠す切り抜き（AutoCAD 2015 からの反転）は、試料の図面になく、どこに持つかも確かめていなかった。そのため、内側を隠さないものとしていた。

AutoCAD 2027 で作った試料の図面（`samples/dwg/zumen-xclip-*`、#316）では、AutoCAD は反転した切り抜きを次のように書いた。R2013 と R2018 の DWG・DXF で同じだった。

- SPATIAL_FILTER の境界には、記号の外枠から元の境界を切り欠いた 11 点の多角形を書く。反転を知らない版でも、ほぼ同じ見た目になる。
- SPATIAL_FILTER の拡張辞書に `ACAD_XREC_ROUNDTRIP` の XRECORD を置く。102 の `ACAD_INVERTEDCLIP_ROUNDTRIP` の後に元の境界の点（10・20・30）を、`ACAD_INVERTEDCLIP_ROUNDTRIP_COMPARE` の後に、そのときの 11 点の多角形の写しを置く。

## 選択肢

1. XRECORD の元の境界を、内側を隠す切り抜きにする。多角形の写しが SPATIAL_FILTER の境界と合わなければ、境界をそのまま、内側を隠さない切り抜きにする。
2. これまでどおり、SPATIAL_FILTER の境界を内側を隠さない切り抜きにする。

2 では見た目はほぼ合う。しかし、切り欠きの細い隙間が残り、元の長方形を表す 2 点も失われる。

## 決定

選択肢 1 を採る。

- `ACAD_XREC_ROUNDTRIP` の XRECORD に、2 点以上の元の境界があり、多角形の写しが SPATIAL_FILTER の境界と同じ値なら、元の境界を `inverted: true` の切り抜きにする。元の境界は、SPATIAL_FILTER の境界と同じ変換で記号の定義の空間に写す。2 点なら、向かい合う角の長方形とする。
- 写しがないか、境界と合わないか、点が平面の外にあるときは、反転を知らないソフトが境界を変えたものとし、SPATIAL_FILTER の境界をそのまま使う。
- XRECORD の値も、配置ごとに作業量として数える。

## 影響

- 反転した切り抜きを持つ記号は、元の境界の内側を隠して描かれる。
- 切り抜きを書き戻すことは、これまでどおりしない。

## 参照

- [決定 0149](0149-drawing-symbol-clips.md)
- [DXF の扱い](../formats/dxf.md)
- #316
