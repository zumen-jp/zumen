# 0292: SXF の追加と、ブロックに従う色・線種・線幅の追加も、書くフィールドの分かる診断で断る

| 項目 | 内容 |
| --- | --- |
| 日付 | 2026-11-10 |
| 状態 | 採用 |

## 背景

[決定 0290](0290-jww-add-inherited-style.md) で、JWW の追加で色か線種がレイヤに従うものを、新しい診断 `change.style.unsupported`（`field` と `format`）で断ることにした。そのとき、ブロックに従う値などのほかの断り方は、今までどおり `change.edit.unsupported` で断るとし、SXF をそろえるかは別に決めるとした（#715）。

SXF（SFC・P21）のフィーチャも、色・線種・線幅をコードで持ち、レイヤかブロックに従う値を持たない（[決定 0167](0167-sxf-sfc-apply.md)・[決定 0169](0169-sxf-p21-apply.md)）。API の `add` は、書かない色・線種・線幅をレイヤに従う値として渡す（[決定 0139](0139-drawing-apply.md)）ので、SXF に足すときは 3 つとも書く必要がある（点マーカーは色だけ）。書かないと、足す図形の種類を `kind` にした `change.edit.unsupported` で断るので、何を書けばよいかが分からない。API の `inherit` で「ブロックに従う」と書いた値も、JWW と SXF では同じく受けられない。

## 選択肢

1. SXF とブロックに従う値は、`change.edit.unsupported` のままにする。
2. SXF のレイヤに従う値だけを `change.style.unsupported` にする。ブロックに従う値は `change.edit.unsupported` のまま。
3. SXF と JWW の、レイヤかブロックに従う値（JWW の線幅はブロックに従うものだけ）を、すべて `change.style.unsupported` にする。診断の文を「レイヤかブロックに従う」にする。
4. 3 に加え、ブロックに従う値には別の新しい診断を作る。

1 は、#715 が直したい分かりにくさが残る。2 と 4 は、どちらも「図形が自分で持つ値を書かなかった」という同じ理由に、別の診断を返す。利用者がすることは、どちらも「そのフィールドを書く」で変わらない。

## 決定

選択肢 3 を採る。

- **SXF。** 追加で、色（点マーカーと線分・折線・円・円弧）か、線種か線幅（線分・折線・円・円弧）がレイヤかブロックに従う（線幅は持たないものも）なら、`change.style.unsupported` で断り、何も変えない。`field` は `color`・`line_type`・`line_weight`、`format` は `SFC` か `P21`。点マーカーは線種と線幅を持たないので、今までどおりそれらがレイヤに従うものだけを受ける。
- **JWW。** 色と線種は、レイヤに従うものに加え、ブロックに従うものも `change.style.unsupported` で断る。線幅は、持たないかレイヤに従うものを 0（ペンの印刷の線幅）で書くまま（決定 0290）とし、ブロックに従うものを `change.style.unsupported`（`field` は `line_weight`）で断る。
- 書いた値を受けられないとき（定義のない色、文書の既定の線幅など）と、足せない形は、今までどおり `change.edit.unsupported` で断る。
- `change.style.unsupported` の説明と文を、「レイヤかブロックに従う値を持たない」に直す。code の意味（図形が自分で持つ値を書かずに足そうとした）は変えない。`field` の値に `line_weight` を足す。

決定 0290 の「ブロックに従う値などは、今までどおり `change.edit.unsupported` で断る」は、この決定で置き換える。

## 影響

- API で SXF に要素を足すとき、色・線種・線幅のどれを書かなかったかが、診断の `field` で分かる。
- JWW と SXF の追加で、ブロックに従う値を断る診断の code が変わる。どちらも 0.2.0 で足した振る舞いで、まだ公開していない。
- `schema/messages.json` の `add_edit` の説明、`docs/formats/sxf.md`・`docs/formats/jww.md` を直した。SFC の合成の試験（色・線種・線幅のそれぞれと、レイヤとブロック、点マーカー）、JWW の試験（ブロックに従う色・線種・線幅）、適合テストの SFC の組で、code と `field` を確かめた。

## 参照

- [決定 0139](0139-drawing-apply.md)
- [決定 0167](0167-sxf-sfc-apply.md)
- [決定 0169](0169-sxf-p21-apply.md)
- [決定 0290](0290-jww-add-inherited-style.md)
- #715
