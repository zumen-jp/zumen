# EXPRESS

利用者が渡す EXPRESS のスキーマ（ISO 10303-11）を読む規則を記録します。読んだスキーマは、IFC をスキーマに照らしてチェックする `validate` の op で使います（[決定 0207](../decisions/0207-building-model-components.md)・[決定 0216](../decisions/0216-express-validate-op.md)）。実装は `crates/zumen-ifc` の `Express` です。

ISO の本文は買わず、読みません。公開された EXPRESS のスキーマ（IFC など）と、寛容なライセンスの実装から読みます。zumen は EXPRESS のスキーマを同梱しません（旧版の決定 0028）。権利の根拠は[形式と権利の一覧](README.md#format-express)にあります。

## 入手

- IFC の EXPRESS のスキーマは、buildingSMART International が IFC のバージョンごとに公開している。利用者が入手し、`zm_push` で渡す。
- IFC のスキーマは、Git・パッケージ・テストに入れない。テストは、テストの中で書いた小さなスキーマを使う。
- 決定 0171 の条件に合う、再配布できる EXPRESS の公開サンプルは、まだ探していない（#663）。

## 読むもの

- ファイルの初めの `SCHEMA` から `END_SCHEMA` までを読み、後ろは読まない。スキーマの名前の後ろのバージョンの識別子は読み飛ばす。
- 注釈（入れ子のある `(* … *)` と、`--` から行の終わりまで）は読まない。名前は、大文字と小文字を区別しない。
- 読むもの、読み飛ばすもの、数だけを数えるものは、[決定 0216](../decisions/0216-express-validate-op.md) の「EXPRESS から読むもの」のとおり。`WHERE`・`UNIQUE`・大域の `RULE` は評価せず、その数を結果と `express.rule.unsupported` で返す。

## 壊れた所と上限

- 読めない書き方、閉じない注釈か文字列、宣言のない名前、上位の型か定義された型の循環は、`express.file.invalid` で失敗する。読めない書き方と、閉じない注釈か文字列には行の番号を付け、読めない書き方には近くの字句も付ける。宣言のない名前と循環には、その名前を付ける。
- 入力のバイト数を予算で数え、字句に分けるときのバイトと、循環を調べる手順を `work` で数える。

## 確かめた度合い

| 形式 | 向き | 段 |
| --- | --- | --- |
| `express` | 読み | `synthetic` |

段の意味は[決定 0254](../decisions/0254-parallel-agents-and-cross-review.md) に、根拠は[形式と権利の一覧](README.md#format-express)にあります。

- 読み: テストの中で書いた小さな EXPRESS（`crates/zumen-ifc/tests/express.rs`）だけで確かめた `synthetic` の段。型・実体・属性・集まりを読むこと、評価しない規則の数、読めない書き方と宣言のない名前の失敗、予算、切ったり変えたりした EXPRESS で落ちないことを確かめる。
- buildingSMART が公開する IFC の EXPRESS のスキーマを読むことと、読んだ値とチェックの結果を外部の実装（IfcOpenShell など）と照らすことは、まだしていない（#663）。
