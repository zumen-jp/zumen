# 0124: 表題欄は、項目のキー・元の項目名・値・表示する要素を持つ、図面の一覧にする

| 項目 | 内容 |
| --- | --- |
| 日付 | 2026-10-28 |
| 状態 | 採用 |

## 背景

[決定 0022](0022-drawing2d-semantics-and-format-mapping.md) は、表題欄が項目のキーと値・元の項目名・対応する表示要素を持ち、罫線の表と区別し、線と文字から推定して作らないことを決めた。[決定 0113](0113-drawing-tables-and-title-blocks.md) は、表題欄の要素を、SXF の表題欄を読むときに足すとした。SFC の `drawing_attribute_feature`（図面表題欄）は、事業名・工事名・図面名などの決まった 12 の項目の値を、引数の順に持つ。位置は持たず、表示する罫線と文字との対応も持たない（#171）。

## 選択肢

1. 図形の要素（`Shape`）にする。
2. 図面（`Drawing2D`）に、表題欄の一覧を足す。項目は、表示する要素を対象（`Target`）で指す。
3. 拡張（`Extension`）に残す。

## 決定

選択肢 2 を採る。表題欄の項目は位置を持たないので、図形の要素にすると、位置を決めなければならない。拡張にすると、形式によらない項目のキーで引けない。

- `Drawing2D::title_blocks` は、元の形式の順に表題欄を持つ。`TitleBlock` は項目と出所を持つ。
- `TitleItem` は、形式によらない項目のキー（共通の項目に当たらなければ `None`）・元の項目名・値・値を表示する要素を持つ。表示する要素は、元の形式が対応を持つときだけ入れる。
- SFC の図面表題欄の 12 の項目に、キーを付ける。`business_name`・`construction_name`・`contract_type`・`drawing_name`・`drawing_number`・`drawing_type`・`scale`・`year`・`month`・`day`・`contractor`・`client` の順とする（推定）。値は前後の空白を除き、足りない項目は空の値にする。値の読めないものは `sxf.lift.invalid` で落とす。
- DXF の属性を持つ記号は、表題欄にしない（決定 0113 のまま）。

## 影響

- 項目名と表示する文字が異なる表題欄（対応表の E10）は、表示する要素との対応を持つ形式を読むときに試す。SFC の図面表題欄は対応を持たない。
- 境界（`drawing` の op）は、表題欄をまだ出さない。

## 参照

- [決定 0022](0022-drawing2d-semantics-and-format-mapping.md)、[決定 0113](0113-drawing-tables-and-title-blocks.md)、[決定 0121](0121-sxf-sfc-lift-to-drawing.md)
- [docs/formats/sxf.md](../formats/sxf.md)
- #171
