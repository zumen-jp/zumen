# 用紙上の距離校正

`zumen-drawing` の `Drawing2D::calibrations(cx)` は、用紙上の 2 点を測るための校正を、元の拡張の順で返します。呼び出し側が ID を選び、`Calibration::measure(start, end)` へ同じ用紙の `PaperPoint` を渡すと、その校正の単位で距離を得ます。領域が重なる場合も、自動で校正を選びません。

校正は紙上の図形の座標・ID・出所を変えません。実寸の図形やビューを作る操作でもありません。現在の入口は Rust で、ABI・各言語・view2d への計測の接続は未実装です。

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

下書きの生成と一覧の読み取りは、渡された `Cx` で作業と確保を数え、中断を確認します。`measure` は選んだ 1 個の校正に対する固定量の計算で、確保しません。検証は [校正のテスト](../crates/zumen-drawing/src/calibration.rs) にあります。PDF の係数を移す範囲と外部照合は [PDF の計測辞書](formats/pdf.md#計測辞書) にあります。
