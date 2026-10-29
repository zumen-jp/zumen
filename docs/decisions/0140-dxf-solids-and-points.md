# 0140: DXF の SOLID・TRACE は塗りのハッチングにし、POINT は点の記号にして図面モデルへ移す

| 項目 | 内容 |
| --- | --- |
| 日付 | 2026-10-28 |
| 状態 | 採用 |

## 背景

[#174](https://github.com/zumen-jp/zumen/issues/174) で、DXF から図面モデルへまだ移さない値を扱う。SOLID・TRACE・POINT は `dxf.lift.dropped` を返して落としていた。日本の図面では、Jw_cad のソリッドと点を DXF に書き出すと、SOLID と POINT になる。

- SOLID と TRACE は、4 つの角（10〜13）を持つ。塗る輪は 1・2・4・3 の順に結んだもので、保存の順と境界の順が違う。3 つの角だけのときは、4 つ目を 3 つ目と同じにする（DXF Reference「SOLID」）。
- POINT は位置（10）と、点を表す記号を回す X 軸の角度（50、度）を持つ。どの記号で表すかと大きさは、図面全体の変数 `$PDMODE` と `$PDSIZE` が決める（DXF Reference「POINT」「HEADER Section Group Codes」）。
- 図面モデルは、JWW のソリッドを、塗り（`Fill::Solid`）の輪 1 つのハッチングにし、JWW の点を、名前空間 `jww.point` の記号にしている（[決定 0134](0134-jww-lift-to-drawing.md)）。対応表は、点を「点と記号」、ソリッドを「塗り」として扱い、頂点の保存順と境界順を区別する（[決定 0022](0022-drawing2d-semantics-and-format-mapping.md)）。

## 選択肢

1. JWW と同じ形にする。SOLID・TRACE は塗りのハッチング、POINT は名前空間 `dxf.point` の記号にする。
2. 図面モデルに、塗った多角形と点の形（`Shape`）を足す。
3. 移さずに、これまでどおり落とす。

2 は、同じ意味の形が 2 つになり、解析と書き出しの両方が両方を扱う必要がある。3 は、日本の図面でよく使う塗りと点を失う。

## 決定

選択肢 1 を採る。

- **SOLID・TRACE。** 角を 1・2・4・3 の順に結んだ、閉じた折れ線の輪 1 つを塗るハッチングにする。3 つ目と 4 つ目の角が同じなら三角形とする。13 がなければ 3 つ目の角を使う。角の Z（30〜33）が 0 でないもの、押し出しの向きが Z でないものは `dxf.non-planar.dropped` を返す。厚さ（39）は、ほかの図形と同じく、まだ移さない。TRACE も SOLID と同じに扱う。
- **POINT。** 名前空間 `dxf.point` の記号にし、`$PDMODE` が 0（なし）なら `point` を、ほかはその番号を名前にする。尺度は 1、回転は 50 とする。DWG は、見出しの変数の区切りから `$PDMODE` を読み、同じ名前で表に置く。`dxf.point` の記号は、ブロックの定義を指さないので、定義の有無を確かめない（[決定 0116](0116-drawing-symbol-references.md)）。
- 用紙の空間・記号の定義の中の SOLID・TRACE・POINT も同じに移す。
- 書き戻し（[決定 0139](0139-drawing-apply.md)）は、今は SOLID・TRACE・POINT の置き換えを扱わない（除去は扱う）。

## 影響

- `$PDSIZE`（点の大きさ）は、まだ移さない。記号の大きさの意味（絶対の大きさ、画面の高さに対する割合）を図面モデルに持たせる方法を決めてから扱う。
- RAY・XLINE は、無限の線を表す形が図面モデルにないので、これまでどおり `dxf.lift.dropped` を返す。
- 塗りのハッチングとして移すので、SOLID と HATCH の塗りは、解析と描画で同じに扱える。元の種類は出所（`Ref`）から辿る。

## 参照

- DXF Reference「SOLID」「TRACE」「POINT」「HEADER Section Group Codes」
- [決定 0022](0022-drawing2d-semantics-and-format-mapping.md)、[決定 0116](0116-drawing-symbol-references.md)、[決定 0134](0134-jww-lift-to-drawing.md)、[決定 0139](0139-drawing-apply.md)
- #174
