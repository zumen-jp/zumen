# 0219: 境界では IFC を open で建物モデルの文書として開き、要約・要素と成分・IDS の照合を別の op で返す

| 項目 | 内容 |
| --- | --- |
| 日付 | 2026-10-31 |
| 状態 | 採用 |

## 背景

#415 は、ブラウザー・Node.js・Python・.NET・CLI・MCP から、IFC を開いて建物モデル（[決定 0207](0207-building-model-components.md)）を使えるようにする。境界の関数は変えず、op とメッセージを足す（[決定 0012](0012-boundary-functions-and-messages.md)・[決定 0015](0015-add-only-boundary-and-version-floors.md)）。作るにあたり、次の 4 つが決まっていなかった。

- IFC の文書を、図面の文書と同じ `open` で開くか、別の op にするか。図面の op を IFC の文書に呼んだら、どうするか。
- 成分を、どの形で返すか。
- IDS の照合（[決定 0217](0217-ids-check.md)）と、レビューの文書への取り込み（[決定 0218](0218-review-document-and-ops.md)）を、境界でどう呼ぶか。
- 0013 は IFC を BIM の wasm のモジュールに置くとしたが、モジュールを分けること（#416）は、まだしていない。

## 選択肢

開き方は 2 つある。

1. `open` が形式を判定し、IFC なら建物モデルの文書にする。
2. `building_open` のような別の op にする。

選択肢 2 は、呼ぶ側が形式を先に判定することになり、`open` が形式を判定するという今の約束（0012）と合わない。

成分の返し方は 2 つある。

1. 要素の一覧の op で、成分の名前を選ばせ、メッセージのスキーマに成分ごとの形を書く。
2. 建物モデルの全体を、blob の JSON で返す。

選択肢 2 は、大きなモデルで一度に全体を返すことになり、ページで区切れない。

## 決定

どちらも選択肢 1 を採る。

### 開く・閉じる

- `open` は、IFC-SPF（FILE_SCHEMA が `IFC` で始まる ISO 10303-21）と ifcZIP を、図面の形式より先に判定し、L1 を読んで建物モデルに変換した文書にする。`format` は `ifc` を返す。ifcZIP の判定は ZIP の署名だけなので、図面の形式のどれにも合わないときに試す。
- 建物モデルの文書は、図面の文書と同じ名前の空間に置く。`close` で捨てられる。
- 図面の op（`drawing`・`entities`・`apply` など）を建物モデルの文書に、建物モデルの op を図面の文書に呼べば、`call.document.unsupported` を出して失敗する。
- 文書のハッシュは、入力のバイト列のハッシュ（0207 の `Hash::document`）にする。

### 建物モデルの op

- `building` は、文書の要約を返す。スキーマ、文書のハッシュ、プロジェクト、単位、要素の数、クラスごとの要素の数。
- `building_elements` は、要素を書いた順に、ページで返す。`ids` で要素を、`class` で IFC のクラスの名前（大文字小文字を区別しない）を選べる。欄は、ID・要素のハッシュ・クラス・名前を既定で返し、`components` で成分を選ぶ。
  - 返せる成分は、`identity`・`class`・`classification`・`spatial`・`parent`・`placement`・`type_of`（型）・`properties`・`quantities`・`material`・`opening`・`group`・`connection`・`level`・`zone`・`style` と、索引から求める `relations`（子・参照・型の実例・開口・埋める要素・構成員）にする。`placement` は、親をたどった世界の変換（3 行 4 列）と、親の要素の ID を返す。
  - `representation`（形状表現）・`grid`・`extension` は、3D のシーンの op（#417）と、それらを使う流れが決まるまで返さない。
  - 要素の参照は、どれも要素の ID（GlobalId か `i` と番号）で返す。値は、種類（`real`・`integer`・`text`・`boolean`・`logical`・`enumeration`・`element`・`list`）と値の組にする。整数は、2^53 − 1 を超えれば、値を変えないように文字列で返す。
- `ids_check` は、push した IDS で照合し、仕様ごとの数と、指摘をページで返す。EXPRESS を push して渡せば、IFC の属性の全体を EXPRESS の名前で照らす（0217）。
- `review_carry` は、建物モデルの文書も新しい版として受ける。要素の参照は、建物モデルの要素の ID とハッシュで引く。
- `review_adopt_ids` は、`ids_check` と同じに照合し、選んだ指摘をレビューの文書に取り込む（0218 の `review_adopt` の IDS 版）。規則の ID は、指摘の仕様の名前にする。要素の参照は、診断の場所の要素の ID と、建物モデルの要素のハッシュにする。

### wasm のモジュール

- モジュールを分けるまで（#416）、建物モデルと IDS の照合は、今の 1 つのモジュールに入れる。分けるときに、モジュールにない op は使えないことを診断で返す（0013）。

- 新しい op と code を足すので、境界の小版を 44 にする。

## 影響

- #415 は、この決定の op を作る。#416 は、建物モデルの op を BIM のモジュールに移す。
- 形状表現は、#417 の 3D のシーンが使う。
- これまで `open` は、FILE_SCHEMA が IFC の入力を SXF のコーデックに渡し、`sxf.schema.unsupported` で失敗していた。今後は IFC として開く。SXF のコーデックが IFC を読まないことは、`zumen-sxf` の試験が確かめ続ける。

## 参照

- [決定 0012](0012-boundary-functions-and-messages.md)、[決定 0013](0013-boundary-in-four-forms.md)、[決定 0015](0015-add-only-boundary-and-version-floors.md)、[決定 0207](0207-building-model-components.md)、[決定 0211](0211-ifc-l1-instance-graph.md)、[決定 0217](0217-ids-check.md)、[決定 0218](0218-review-document-and-ops.md)
- #415・#416・#417
