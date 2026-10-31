# 0216: IFC のスキーマでのチェックは、zm_push で渡した IFC と EXPRESS を `validate` の op で照らし、規則は評価せずに数を返す

| 項目 | 内容 |
| --- | --- |
| 日付 | 2026-10-31 |
| 状態 | 採用 |

## 背景

- [0207](0207-building-model-components.md) は、IFC の L1 をスキーマを知らないインスタンスのグラフにし、スキーマでの検証は、利用者が渡す EXPRESS で行うとした。zumen は EXPRESS を同梱しない（#410）。
- IFC を `open` で文書にする口は、まだない（#415）。`open` は、建物モデル（#409）と IDS（#411）を受けてから足す。
- 境界の入力は `zm_push` で名前を付けて渡す（[0012](0012-boundary-functions-and-messages.md)）。

## 選択肢

1. `validate` の op を足し、IFC と EXPRESS をどちらも push した入力の名前で受ける。
2. `open` で IFC を文書にしてから、文書の名前と EXPRESS で受ける。
   - #415 を待つことになり、#415 は #409 と #411 を待つ。
3. EXPRESS を読んだ結果を、セッションの状態として持つ。
   - 状態と消し方が増える。EXPRESS を読む手間は、IFC を読む手間より小さい。

## 決定

- 選択肢 1 を採る。op を `validate` にし、`input`（IFC-SPF か ifcZIP）・`express`・`offset`・`limit` を受ける。スキーマの小版を 42 にする。
  - ifcZIP は書庫の始まりの判定で見分ける。文書を開いた後の形は、#415 で `doc` を足して受ける。
- 結果は、EXPRESS のスキーマの名前・IFC の `FILE_SCHEMA`・EXPRESS のハッシュ（`v2:`、文書のハッシュと同じ求め方）・評価しなかった規則の数・指摘・数・`next_offset` にする。指摘は診断と同じ形で返し、`check` と同じく、書かなければ 1000 件ずつ区切る。
- `FILE_SCHEMA` の名前がスキーマの名前と違えば `express.schema.mixed` を返し、そのままチェックする。版の違いを、利用者が承知のうえで照らせるようにする。
- EXPRESS から読むもの:
  - スキーマの名前、定義された型、列挙、選択（入れ子の選択もたどる）、集まり（`LIST`・`SET`・`BAG`・`ARRAY`、数で書いた上下の限り、`OPTIONAL` の項目）。
  - 実体の `ABSTRACT`、`SUBTYPE OF`、明示の属性と `OPTIONAL`、上位の属性の宣言し直し（`SELF\上位.属性`）、上位の属性を導く属性にしたもの。
  - `WHERE`・`UNIQUE`・大域の `RULE` は数だけを数えて評価しない。関数・手続き・定数・`USE`・`REFERENCE`・`SUBTYPE_CONSTRAINT`・`INVERSE` は読み飛ばす。式で書いた集まりの限りは、限りなしにする。
  - 読めない書き方、宣言のない名前、上位の型か定義された型の循環は、`express.file.invalid` で失敗する。
- チェックすること:
  - 実体の名前があること（`express.entity.missing`）、抽象の実体を単独のインスタンスにしていないこと（`express.abstract.invalid`）。
  - 属性の数（`express.attribute-count.invalid`）。単独のインスタンスは上位の型の属性から順に、同じ上位の型を 1 度だけ数える。複合のインスタンスは、部分ごとにその実体が宣言した属性だけを数える。
  - 値（`express.value.invalid`・`express.value.missing`）。`*` は導く属性にだけ、`$` は省略できる属性にだけ置ける。参照の先は宣言の実体かその下位の型、選択は受ける実体への参照か、受ける定義された型を付けた値、列挙は値の名前、集まりは数と項目の型を見る。`INTEGER` は小数点と指数のない数にする。`REAL` には整数の書き方も受ける。
  - 参照の先がファイルにないものは、L1 の `ifc.reference.missing` で診断してあるので、指摘しない。
- 作業は、EXPRESS のバイトごと、インスタンスごと、値ごとに work で数える。

## 影響

- 規則を評価しないので、指摘がなくても、EXPRESS の規則に合うとは言えない。結果の数と `express.rule.unsupported` で、確認できなかった項目として示す。
- #415 で `open` が IFC を受けたら、`validate` に `doc` を足す。

## 参照

- [0012](0012-boundary-functions-and-messages.md)・[0013](0013-boundary-in-four-forms.md)・[0207](0207-building-model-components.md)・[0211](0211-ifc-l1-instance-graph.md)
- #410・#415
