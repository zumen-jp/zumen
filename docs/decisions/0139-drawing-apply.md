# 0139: 図面モデルの変更は、出所で要素を指す変更の一覧にし、共有の表を通して元の形式へ書き戻す

| 項目 | 内容 |
| --- | --- |
| 日付 | 2026-10-28 |
| 状態 | 提案 |

## 背景

[#170](https://github.com/zumen-jp/zumen/issues/170) で、図面モデル（Drawing2D）の変更を、元の系統の L1 へ書き戻す（apply）。これまでの決定は次のとおり。

- L2 から L1 への向きは `lower`（別の形式への変換と新規作成）と `apply`（その場の編集）に分け、形は AutoCAD の系統の試作で決める（[決定 0008](0008-model-layers-and-codecs.md)）。
- 図面モデルの要素は、元のレコードを指す出所（`Ref`）を持つ（[決定 0022](0022-drawing2d-semantics-and-format-mapping.md)）。DXF ではレコードの番号、DWG ではバイトの範囲で指す。
- DXF と DWG は、共有のオブジェクトの表 `AcadDb` を持つ。表の変更は `write_back` が元のレコードと比べて、違うレコードだけを 1 つのトランザクションで書き戻す。書き戻せない変更が 1 つでもあれば、何も反映しない（[決定 0024](0024-acad-table-write-back.md)・[決定 0025](0025-acad-object-add-remove-write-back.md)）。

## 選択肢

1. 変更の一覧。要素を出所で指し、置き換え・除去を並べる。apply は一覧の変更だけを書き戻す。
2. 編集した図面モデル全体を渡し、apply が元の図面モデルと比べて変更を見つける。
3. 図面モデルの型ごとに、L1 を直接変える関数（線の端点を動かす、など）を置く。

2 は、比べる手間が図面の大きさに比例し、要素の順の入れ替えや、移せなかった要素との区別が難しい。3 は、関数が形式の系統と要素の種類の組ごとに増え、境界のメッセージにも写しにくい。

## 決定

選択肢 1 を提案する。

- **trait。** `zumen_sdk::Apply<C>` を置く。`apply(&mut self, changes: &C, cx)` は、すべての変更を確かめてから反映し、1 つでも書き戻せなければ何も反映せずに `Error::Rejected` を返す。0008 の素案の `Lower` は、別の形式への変換を作るときに足す。
- **変更の型。** `zumen_drawing::Changes` は、`Edit` の一覧を持つ。`Edit::Replace` は出所を持つ要素で、同じ出所の要素を置き換える。`Edit::Remove` は出所で要素を除く。足すことは、新しい出所の決め方と合わせて次に決める。
- **AutoCAD の系統。** DXF と DWG は、今のファイルから表を作り、変更を表のオブジェクトに当ててから `write_back` で書く。どちらの符号化でも同じ当て方を使う。
- **確かめること。** 置き換えは、同じ出所の要素が今の図面モデルにあり、レイヤ・色・線種・線幅が同じで、形の種類が元のレコードの種類で表せるときだけ当てる。当てられない変更は、要素の出所を指す診断（`dxf.element.missing`・`dxf.change.unsupported`）を返す。
- **最初の範囲。** 実寸の空間の LINE と CIRCLE の形（端点、中心と半径）の置き換えと、続くレコード（VERTEX・ATTRIB・SEQEND）を持たない図形の除去から始める。

## 影響

- 置き換えられる種類・欄、用紙の要素、要素の追加は、試料で確かめながら広げる。
  - 用紙の要素は、`Edit::ReplaceOnSheet` で置き換える。除去は、`Edit::Remove` が実寸の空間と用紙の要素の両方を指す。
  - 実寸の空間の要素は、`Edit::Add` で足す。出所は、書き戻したファイルを読み直して決める（[決定 0155](0155-drawing-apply-additions.md)）。
- 対応表の E1・E4・E5・E10（[対応表](../formats/drawing2d-mapping.md)）と、書けない拡張の診断は、置き換えの範囲を広げるときに扱う。形式固有の情報（[決定 0115](0115-drawing-format-extensions.md)）は、今は書き戻さない。
- 境界の op として変更を受けるときは、`Changes` の形をメッセージのスキーマに写す。

## 参照

- [決定 0008](0008-model-layers-and-codecs.md)、[決定 0022](0022-drawing2d-semantics-and-format-mapping.md)、[決定 0024](0024-acad-table-write-back.md)、[決定 0025](0025-acad-object-add-remove-write-back.md)、[決定 0115](0115-drawing-format-extensions.md)
- #170
