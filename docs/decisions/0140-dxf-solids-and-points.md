# 0140: DXF の SOLID・TRACE は塗りのハッチングにし、POINT は点の形にして図面モデルへ移す

| 項目 | 内容 |
| --- | --- |
| 日付 | 2026-10-28 |
| 状態 | 提案 |

## 背景

[#174](https://github.com/zumen-jp/zumen/issues/174) で、DXF から図面モデルへまだ移さない値を扱う。SOLID・TRACE・POINT は `dxf.lift.dropped` を返して落としていた。日本の図面では、Jw_cad のソリッドと点を DXF に書き出すと、SOLID と POINT になる。

- SOLID と TRACE は、4 つの角（10〜13）を持つ。塗る輪は 1・2・4・3 の順に結んだもので、保存の順と境界の順が違う。3 つの角だけのときは、4 つ目を 3 つ目と同じにする（DXF Reference「SOLID」）。
- POINT は位置（10）と、点を表す印を回す X 軸の角度（50、度）を持つ。どの印で表すかと大きさは、図面全体の変数 `$PDMODE` と `$PDSIZE` が決める（DXF Reference「POINT」「HEADER Section Group Codes」）。
- 図面モデルは、JWW のソリッドを、塗り（`Fill::Solid`）の輪 1 つのハッチングにしている（[決定 0134](0134-jww-lift-to-drawing.md)）。対応表は、点を「点と記号」、ソリッドを「塗り」として扱い、頂点の保存順と境界順を区別する（[決定 0022](0022-drawing2d-semantics-and-format-mapping.md)）。
- 点は、図面モデルの点の形にする（[決定 0160](0160-drawing-points.md)、提案）。

## 選択肢

1. SOLID・TRACE は塗りのハッチングにし、POINT は点の形（0160）にする。
2. SOLID・TRACE は塗りのハッチングにし、POINT は名前空間 `dxf.point` の記号にする（この決定の最初の案）。
3. 図面モデルに、塗った多角形の形を足す。
4. 移さずに、これまでどおり落とす。

2 は、点を描く側と分析が形式ごとの名前空間を知る必要があり、`$PDSIZE` を表せない（0160）。3 は、塗りのハッチングと同じ意味の形が 2 つになり、解析と書き出しの両方が両方を扱う必要がある。4 は、日本の図面でよく使う塗りと点を失う。

## 決定

選択肢 1 を提案する。

- **SOLID・TRACE。** 角を 1・2・4・3 の順に結んだ、閉じた折れ線の輪 1 つを塗るハッチングにする。3 つ目と 4 つ目の角が同じなら三角形とする。13 がなければ 3 つ目の角を使う。角の Z（30〜33）が 0 でないもの、押し出しの向きが Z でないものは `dxf.non-planar.dropped` を返す。厚さ（39）は、ほかの図形と同じく `dxf.entity` に持つ（[決定 0142](0142-dxf-entity-extension.md)）。TRACE も SOLID と同じに扱う。
- **POINT。** 文書の点の表し方に従う点の形にする。位置は 10、回転は 50 とする。`$PDMODE` と `$PDSIZE` は、文書の点の表し方にする（0160）。DWG は、見出しの変数の区切りから `$PDMODE` と `$PDSIZE` を読み、同じ名前で表に置く。
- 用紙の空間・記号の定義の中の SOLID・TRACE・POINT も同じに移す。
- 書き戻し（[決定 0139](0139-drawing-apply.md)）は、今は SOLID・TRACE・POINT の置き換えを扱わない（除去は扱う）。

## 影響

- RAY・XLINE は、無限の線を表す形が図面モデルにないので、これまでどおり `dxf.lift.dropped` を返す。
- 塗りのハッチングとして移すので、SOLID と HATCH の塗りは、解析と描画で同じに扱える。元の種類は出所（`Ref`）から辿る。
- 今の main は、POINT を `dxf.point` の記号にしている。この決定を採ったら、点の形に移す。

## 参照

- DXF Reference「SOLID」「TRACE」「POINT」「HEADER Section Group Codes」
- [決定 0022](0022-drawing2d-semantics-and-format-mapping.md)、[決定 0134](0134-jww-lift-to-drawing.md)、[決定 0139](0139-drawing-apply.md)、[決定 0142](0142-dxf-entity-extension.md)、[決定 0160](0160-drawing-points.md)
- #174
