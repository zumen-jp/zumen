# 0295: レビューの図形は zumen-scene で別のシーンにし、対象と起点を照合し、元の図形と場所へ戻れる ID を付ける

| 項目 | 内容 |
| --- | --- |
| 日付 | 2026-11-10 |
| 状態 | 採用 |

## 背景

[0214](0214-review-model-and-bcf.md) は、レビュー文書の図形をコアで 2D のシーンにするとした。[0267](0267-view2d-on-scene-columns.md) は、そのシーンを表示部品で描き、形を JavaScript で再実装しないとした。図形の生成入口と出所の ID は、まだない（#589）。

`zumen-review` の型は対象・指摘・図形と場所を持ち、図面モデルと建物モデルに依らない。`zumen-scene` は、図面の配置をたどり、線・面・文字などを ZMSC の列にする。既存の `build` を一時の図面へそのまま使うと、生成した要素に図面の ID（`space/0/e0` など）が付き、元の図形の ID を表さない。

レビュー文書の `view` は文字列である。既存の図面 API は、空間・用紙の ID と、用紙のビューの番号を別に受ける。用紙のビューをレビュー文書の文字列で区別する規則も要る。

## 選択肢

### 置き場所と出力

1. `zumen-review` が `zumen-scene` に依り、シーンを作る。
2. `zumen-scene` が `zumen-review` に依り、図面とは別のレビュー用のシーンを作る。
3. 表示部品が、レビュー文書から形を作る。

1 は、領域の文書（L2）から表示の派生（L3）への依存になる。3 は、0214・0267 で決めたコアの形の生成と重なる。2 なら、元の図面のシーンと ID を保ち、既存の列・曲線の分割・文字の処理を使える。

### 失敗の型

1. `zumen-sdk::Error` を使う。
2. `zumen-scene` に、入力の不備と予算・中断の停止を表す `ReviewError` を足す。

SDK の `Error` は `Invalid`・`Unsupported`・`Rejected`・`Stopped` を持ち、L1 の読み書きと編集にも使う。SDK の依存は `zumen-base` だけなので循環は起きないが、今回の L3 の入口に必要なのは入力の不備と停止だけである。`zumen-review` がすでに SDK に依るので、2 は依存の大きさを減らす選択ではない。L3 の公開 API を、形式の読み書き用のエラーの分類に結び付けないために選ぶ。

### 出所の ID

1. 元の図形・場所の ID を、そのままシーンの ID にする。
2. 図形と場所を区別する名前空間を付け、元の ID への対応も返す。
3. 一時の図面が作る ID を返し、元の ID を別の表だけで管理する。

1 は、異なる種類で同じ ID を持てるレビュー文書で衝突する。3 は、描く部品の追加や並べ替えで ID が変わる。2 なら、元の ID を変えず、同名の図形と場所を区別できる。

## 決定

置き場所は 2、失敗の型は 2、出所の ID は 2 を採る。

### 入口と依存

`zumen-scene` の `review` モジュールに、次の Rust の入口と型を置く。`zumen-review` への依存は、すでにワークスペースで使う MIT OR Apache-2.0 の内部 crate であり、外部の依存を増やさない。文書の型と決定的な処理だけを使い、wasm でも同じコードを動かす。

```rust
pub fn build_review(
    drawing: &Drawing2D,
    review: &zumen_review::Review,
    target: &zumen_review::Target,
    options: &ReviewOptions,
    cx: &mut Cx<'_>,
) -> Result<ReviewScene, (ReviewError, Ref)>;

pub fn build_review_with(
    drawing: &Drawing2D,
    review: &zumen_review::Review,
    target: &zumen_review::Target,
    options: &ReviewOptions,
    fonts: &FontSet<'_>,
    cx: &mut Cx<'_>,
) -> Result<ReviewScene, (ReviewError, Ref)>;

pub enum ReviewError {
    Invalid,
    Stopped(Stop),
}
```

- `build_review` は空の `FontSet` を使う。`build_review_with` は、通常のシーンと同じフォントと高さの解釈を使う。
- `ReviewOptions` は `origin`（既存の `Origin`）・正の許容差 `tolerance`・任意の `region`・`chunking`・明示的な `ReviewStyle` を持つ。スタイルの値と図形の作り方は、[レビューの形式の文書](../formats/review.md)に置く。
- `ReviewScene` は `scene: Scene`・`from: String`・`view: Option<usize>`・`unit: Unit` と、元の ID へ戻す `sources: Vec<ReviewSource>` を持つ。
- `ReviewSource` は `scene_id` と、`Markup { id }` または `Location { topic, id }` の区別を持つ。同じ図形や場所の線・面・文字には同じ `scene_id` を付け、対応は 1 件だけ返す。
- 詳しい失敗は診断に記録し、戻り値の `Ref` でも場所を返す。入力の不備と停止では、部分的なシーンを返さない。

生成した形は、`review` モジュールから内部の Builder へ、出所の ID を明示した `VisitRef` で渡す。一時の `Drawing2D` は生成処理の内部の文脈にだけ使える。通常の `build`・`build_with` と図面の `Namer` は変えない。

### 対象と起点

- 呼び手は、開いた実文書の ID・形式・元のバイト列のハッシュを `target` に渡す。レビュー文書の同じ ID の対象と、形式・ハッシュが一致するときだけ作る。渡した `drawing` がその実文書に対応することは、呼び手の責務とする。
- `Review` は Rust で直接作れるので、JSON の読み取りで検査済みとは仮定しない。ID の重複と文書内の参照は文書全体で、有限でない座標と図形の形は選んだ対象・起点の項目で検査する。引数の形も検査し、いずれも予算と中断を確かめる。選ばなかった項目の座標や未知のビューを、選んだ項目の失敗理由にはしない。
- `origin` が存在するかを図面で確かめる。既存の `walk_to` が存在しない起点を空の結果にすることには依らない。
- 空間と用紙の文字列の ID は、既存の `DefinitionNamer`・`SheetNamer` を含む規則で解く。用紙のビューの、レビュー用の ID は、**確定した用紙 ID 全体の後ろに `/view/` と 0 からのビューの番号を足す**。例は `sheet/h22/view/0`。用紙の ID が重なって付いた接尾辞も残す。
- レビューの `view` は、起点から決まる文字列と厳密に照合する。用紙とその中のビュー、実寸の空間と用紙を、同じ起点に混ぜない。選んでいない対象・起点の項目は、指定による絞り込みとして描かない。
- `ReviewScene.from` と `view` は既存の `scene` の結果と同じ分け方にする。上の例の `from` は `sheet/h22`、`view` は `Some(0)` になる。
- この解決規則は、新しいシーンの生成入口で使う。既存の JSON の読み書きと `carry` が、任意の `view` の文字列を保つ挙動は変えない。

用紙と用紙のビューの座標は mm、空間の座標は文書の単位、Y は上にする（0214・0282）。空間の `Unit::Unknown` を mm にしない。スタイルの幾何の寸法と許容差は、明示された起点の長さで受け、線幅は既存のシーンと同じく用紙の mm で受ける。未知の単位でも、座標の単位を保ったまま、明示された寸法を使える。

### シーンの ID

`hex` は、元の ID の UTF-8 の各バイトを、小文字の 2 桁の 16 進で並べるものとする。スラッシュ・NUL・日本語なども、この方法で区別する。

- 図形: `review/markup/<hex(id)>`
- 指摘の場所: `review/location/<hex(topic)>/<hex(location)>`

元の図形・場所・指摘の ID は書き換えない。シーンの ID は、描く部品の数、点、色、配列の位置から作らない。図形の並べ替えや、同じ図形の編集でも、元の ID が同じなら同じ値にする。別のレビュー文書の同名の項目は、レビュー文書と対象の識別情報で区別する。

ZMSC の既存の列を使う。`entity_ids` に上の文字列を入れ、種類は実際に描く形、配置の道筋は空にする。シーンと `sources`、対象と起点の識別情報は、1 回の結果として組にして使う。図面の `entities` にレビューのシーンの ID を渡して出所を引かない。

### 予算と検査

検査の走査と文字列の比較を `Work`、作る要素を `Elements`、作る点を `Segments`、一時の形・文字列・ID・索引・結果のバッファーを `DecodedBytes` で、確保する前に数える。点を作る回数は整数で決め、浮動小数の足し算を終わりの条件にしない。既存 Builder の数え方と、レビュー側で先に作る量を分けて数える。

図形の種類、同名の図形と場所、対象・起点の違い、ハッシュの不一致、未知の単位、不正な Rust の値、予算の境目と中断を、合成入力で検査する。従来の図面のシーンのバイトと ID が変わらないことも確かめる。

## 影響

- 形式を読む実装とレビューの文書の型は変わらず、L3 の入口と出所の対応が増える。
- ZMSC の列配置・既存の図面の ID・文書のハッシュは変えない。新しい名前空間の ID は、この記録の規則を契約にする。
- シーンの生成は、ファイルの編集・変換の書き出しの経路ではなく、レビューの文書から表示を作る L3 の処理である。入力の図面とレビュー文書は変更しない。
- API のメッセージ、JS のラッパー、表示部品とプレイグラウンドへの接続は #589 の後続の作業で行う。既存の領域の `scene` の op で表せるかを確認してから公開する。

## 参照

- [0214](0214-review-model-and-bcf.md)・[0218](0218-review-document-and-ops.md)・[0267](0267-view2d-on-scene-columns.md)・[0282](0282-legacy-markup-review-identity.md)
- [0007](0007-single-rust-implementation.md)・[0010](0010-budget-by-work.md)・[0156](0156-drawing-stable-ids-and-preconditions.md)・[0172](0172-scene-sections-and-op.md)
- #589、[レビュー文書の移行手順](../review-migration.md)
