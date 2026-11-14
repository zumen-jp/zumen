# 用紙上の距離校正

`zumen-drawing` の `Drawing2D::calibrations(cx)` は、用紙上の 2 点を測るための校正を、元の拡張の順で返します。呼び出し側が ID を選び、`Calibration::measure(start, end)` へ同じ用紙の `PaperPoint` を渡すと、その校正の単位で距離を得ます。領域が重なる場合も、自動で校正を選びません。

校正は紙上の図形の座標・ID・出所を変えません。実寸の図形やビューを作る操作でもありません。Rust のほか、メッセージのマイナーバージョン 68 からは共通 API の `calibrations`・`measure_distance` で扱えます。

## 拡張の契約

名前空間は `drawing.calibration`、`schema` は 1 です。`name` を文書内で重複しない空でない ID とし、`version` は使いません。`target` と `basis` はともに `Document`、`source` は元のレコードです。`Calibration::into_draft(cx)` で下書きを作り、通常の `settle_extensions` で確定します。既存の図面のバイナリに含めて往復できます。

| フィールド | 型 | 意味 |
| --- | --- | --- |
| `sheet` | Integer | 参照する `SheetId` |
| `bounds_min`・`bounds_max` | Point | 用紙の mm の閉じた矩形。z は 0、幅・高さは正 |
| `matrix_x`・`matrix_y` | Point | 用紙の変位から校正単位の変位への 2×2 行列の各行。z は 0 |
| `unit` | Text | `millimeter`・`centimeter`・`meter`・`inch`・`foot`・`unknown` |
| `unit_label` | Text | 元の単位ラベル。未知の単位でも保持する |
| `name` | Text、任意 | 元の名前。校正 ID とは分ける |

すべてのフィールドの `code` は `None` です。未知・重複・不足のフィールド、不正な型・ヘッダー・領域・行列、参照先のない用紙は `drawing.calibration.invalid` で出所と理由を返し、校正の一覧から除きます。校正 ID が重複する場合はその ID のすべてを、用紙 ID が重複する場合はその用紙を指すすべてを除きます。図面と原値の拡張は変更しません。

## 計算と予算

始点・終点は、校正と同じ用紙の有限な座標で、両方とも閉領域内にある必要があります。行列は有限で非特異でなければなりません。変位を行列で変換し、`libm::hypot` で距離を計算します。非等方の倍率・回転・反転・せん断も扱います。原点は使わず、単位ラベルからの変換も行いません。

違う用紙、非有限の点、領域外の点、不正な領域・行列、計算のあふれは `MeasurementError` で返します。行列の非特異の判定は仮数と指数で正確に行い、行列式の浮動小数点のあふれ・桁落ちで誤判定しません。長さの計算自身があふれる場合は失敗します。

下書きの生成と一覧の読み取りは、渡された `Cx` で作業と確保を数え、中断を確認します。`measure` は選んだ 1 個の校正に対する固定量の計算で、確保しません。検証は [校正のテスト](../crates/zumen-drawing/src/calibration.rs) にあります。

## 原点と差分を分ける

`SplitPaperPoint` は `sheet`・`origin: [f64; 2]`・`offset: [f64; 2]` を持ち、位置を原点と差分の正確な和で表します。`Calibration::measure_split(start, end)` で、大きな原点と小さな差分を保ったまま計測できます。たとえば原点が `1e16`、差分が `0.25` と `0.75` の二点では、校正前の差を `0.5` として扱います。

閉領域の判定は原点・差分・境界の 3 項、距離の差分は両端の 4 項を固定長の二進整数で正確に集計します。境界の比較では丸めず、差分を binary64 に戻すときだけ最近接・偶数丸めを行います。分割の仕方や途中の和のあふれに影響されません。続く行列計算と距離の規則は `measure` と共通で、差分自身が binary64 に収まらない場合は `Overflow` です。有限な値だけを受け付け、ヒープには確保しません。

## 共通 API

`drawing` モジュールの `calibrations` は、開いた図面の校正一覧を返します。`doc` を必須とし、任意の `from` は `drawing` が返す公開用紙 ID で絞ります。ハンドルと重複の接尾辞を含む ID を使い、内部の `SheetId` の数字から推定しません。未知の用紙 ID は失敗にします。

`offset` は 0 からの位置、`limit` は 1〜10,000（既定 1,000）です。返す `items` は元の拡張の順で、各項目は `id`・`name`（任意）・`from`・`bounds`・`matrix`・`unit`・`unit_label`・`source`（任意）を持ちます。`total` は用紙で絞った後の総数、`next_offset` は続きがある場合だけ返します。不正・重複校正の除外と診断は Rust と共通です。

`measure_distance` は用紙と校正 ID、原点と差分に分けた二点を受け取り、`measure_split` を呼びます。単位とラベルを距離とともに返し、重なる校正を自動選択しません。

```json
{
  "v": 1,
  "op": "measure_distance",
  "doc": "plan",
  "from": "sheet/0",
  "calibration": "sheet/0/vp/0",
  "start": { "origin": { "x": 0, "y": 0 }, "offset": { "x": 10, "y": 10 } },
  "end": { "origin": { "x": 0, "y": 0 }, "offset": { "x": 13, "y": 14 } }
}
```

両操作は `hash` に元の文書のハッシュを返します。任意の `expected_hash` を渡すと、異なる場合は `/expected_hash` の `call.value.invalid` で失敗します。図面バイナリから開いた場合、この値は埋め込まれた `Source.hash` です。入力バイナリ自身のハッシュや、元のファイルの正しさを証明する値ではありません。

未知の校正 ID・用紙 ID は `call.value.invalid` です。校正と異なる用紙、領域外、演算のあふれなどは `drawing.measurement.failed` に校正 ID と `reason` を返します。失敗と予算停止では部分的な結果を返さず、文書・図形・校正を変更しません。一覧と返却値の確保を予算へ数え、固定長の計測も呼び出す前に作業量を数えます。

## 図面バイナリを渡す

Rust で作った校正は、`binary::write` で既存の図面バイナリに含め、各言語の `Session.push` と `open` で読めます。`open` の `format` は省略します。

PDF 単独の閲覧・読み取りは、[形式台帳](formats/README.md#format-pdf)のとおりアプリの側で扱います。コアは PDF を直接 `open` せず、PDF の計測辞書から校正を作る処理も持ちません。ここで扱うのは、呼び出し側が図面バイナリに含めた校正です。

## 表示部品で選ぶ

view2d は利用側が選んだ ID と同期の resolver を受け取り、コアの結果を計測に添えます。`measureDistance` は分離した二点と文書のハッシュを共通 API へ渡す helper です。

```js
import { measureDistance } from "@zumen-jp/view2d";

const id = "sheet/0/vp/0";
viewer.setMeasurementCalibration({
  id,
  resolve: (measurement, document) => measureDistance(session, document, id, measurement),
});
```

`Measurement.calibrated` は成功時に `status: "measured"` と距離・単位・元ラベルを持ちます。未知の単位も表示できます。失敗・停止・領域外では `status: "unavailable"` と理由を持ち、従来の紙上の距離・計測線と `real` は保ちます。未選択では `calibrated` を持ちません。校正距離を JavaScript の行列計算で作り直しません。

helper は成功応答をそのまま返します。失敗時は通常の `Session.call` と同じく、例外の `response` に診断と予算が残ります。必要なら resolver 内でそれを利用側へ保存してください。表示部品は応答全体を状態に保持せず、ID・用紙・文書のハッシュ・単位・有限な距離を確かめて、凍結した計測値だけを添えます。校正の ID と単位ラベルは各 4,096 文字までです。

`setMeasurementCalibration(null)` は校正の選択を消します。シーンの交換・消去と `destroy` でも解除します。`setMeasurement(null)` は計測だけを消し、同じシーンの校正選択は残します。Promise を返す resolver は採用せず、呼び出し中は表示部品の状態を変更できません。
