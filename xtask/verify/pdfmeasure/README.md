# PDF の計測辞書を外部と照らす

計測辞書の原値は pdf-lib 1.17.1、校正距離の算術は PDF4QT の固定コミットで照らします（#520）。どちらも Cargo の依存や配布物には入れません。Node.js 24 と、Windows では既存の MSVC、Linux・macOS では C++20 のコンパイラーを使います。Java・Qt の追加は不要です。

```sh
npm ci --ignore-scripts --no-audit --no-fund --prefix xtask/verify/pdfmeasure
node xtask/verify/pdfmeasure/setup.mjs
node xtask/verify/pdfmeasure/read.mjs --check
node xtask/verify/pdfmeasure/run.mjs --check
```

Windows の準備は `vswhere.exe` で Visual Studio を探し、公式の DevShell モジュールから `cl` を呼びます。Linux・macOS は `CXX`、または `c++`・`g++`・`clang++` の順で探します。`CXX` はコンパイラーの実行ファイルだけを指定します。グローバルな環境設定や runtime は変更しません。

## 原値を読む

`read.mjs` は標準入力の PDF を読み、標準出力へ JSON を 1 個返します。Node.js の API で渡す場合は次の形です。

```js
spawnSync(process.execPath, ["xtask/verify/pdfmeasure/read.mjs"], { input: pdfBytes });
```

出力の外側は次の形です。ページの `index` は 0 から始まります。

```json
{"oracle":"pdf-lib","version":"1.17.1","pages":[{"index":0,"UserUnit":{"type":"absent"},"VP":{"type":"array","value":[]}}]}
```

各値は `type` 付きです。`dict.value` はキーごとの値、`array.value` は配列です。`number.value` は JavaScript の Number をそのまま返し、float32 へ丸めません。文字列は `value` に復号した文字列、`bytes` に元の内容の 16 進表記、`representation` に `literal` または `hex` を持ちます。名前も `value` と `bytes` を返します。

| 入力 | 出力 |
| --- | --- |
| キーがない | `{"type":"absent"}` |
| 明示した null | `{"type":"null"}` |
| 間接参照 | `{"type":"ref","object":4,"generation":0,"value":...}` |
| 未定義の参照先 | 参照の `value` が `{"type":"unresolved"}` |
| 循環参照 | 参照の `value` が `{"type":"cycle"}` |

Viewport は `Type`・`BBox`・`Name`・`Measure`、Measure は `Type`・`Subtype`・`R`・`X`・`Y`・`D`・`A`・`T`・`S`・`O`・`CYX` を、欠落も含めて返します。NumberFormat は `Type`・`U`・`C`・`F`・`D`・`FD`・`RT`・`RD`・`PS`・`SS`・`O` を返します。存在する追加キーも返し、数値への変換や既定値の補充は行いません。

原値は PDF の型と数値・文字列の値です。元の数の字面や、2 の 53 乗を超える整数の厳密値を保存するものではありません。ページの `MediaBox`・`CropBox`・`Rotate`・`UserUnit` も直接書いた値を返します。ページ寸法・回転・継承・計測の意味を判定するツールではありません。入力 64 MiB、10 万項目、深さ 32、文字列 1 MiB を上限にします。

## 校正距離を計算する

`run.mjs` は標準入力の JSON を、コンパイルした外部の算術関数へ渡します。`vector` は既定です。`delta` は元の関数へ渡す既定のユーザー空間の差分で、用紙の mm への変換・回転・`UserUnit` の乗算は行いません。

```json
{"mode":"vector","delta":[3,4],"x":[0.5],"y":[0.25],"distance":[100],"cyx":2}
```

`x`・`y`・`distance` は、それぞれ X・Y・D の NumberFormat の C 値の列です。単数の number も指定できます。省略すると空の列です。`cyx` の省略時は 1 です。`scalar` は `delta` の代わりに `length` を渡します。

```json
{"mode":"scalar","length":5.25,"x":[1],"distance":[1,12]}
```

入力をトップレベルの配列にすると、1024 件までまとめて計算できます。書式は各列 256 個まで、数は有限の double、入力 JSON は 1 MiB までです。

結果は `formatValue` に実際に渡された値の列です。`array` は使った X または D、`index` はその書式の位置、`last` は末尾かを表します。最初の例の結果は次の形です。

```json
{"oracle":"pdf4qt-arithmetic","pin":"0cfea2f280c40d5f5e0d065496704654468390b5","mode":"vector","samples":[{"array":"D","index":0,"value":250,"last":true}]}
```

2 番目の例では値が 5.25・3 の 2 項目になります。D の各係数を掛け、末尾以外は小数部を次へ渡す処理も、元の未改変の関数を動かします。バッチの結果は `results` 配列に入ります。標準出力は JSON だけで、エラーは標準エラー出力へ返します。`--check` は依存や準備がなければ終了コード 77 を返します。

## 外部のコードと照合範囲

- 原値は [pdf-lib 1.17.1](https://github.com/Hopding/pdf-lib/tree/4beebbef7a261ed0be089bb1d63a5b6e28cc488a) の公開 `PDFContext`・`PDFDict`・`PDFNumber` などで読みます。[MIT](https://github.com/Hopding/pdf-lib/blob/4beebbef7a261ed0be089bb1d63a5b6e28cc488a/LICENSE.md) で、npm の版と推移依存はこのフォルダーの lockfile で固定します。wasm へはリンクせず、キーを並べ、結果の順を固定します。
- 算術は [PDF4QT の pdfmeasure.cpp](https://github.com/JakubMelka/PDF4QT/blob/0cfea2f280c40d5f5e0d065496704654468390b5/Pdf4QtLibCore/sources/pdfmeasure.cpp) を使います。Git blob は `f2319cb468706d11da5d2eccc681b97bebe2b138`、SHA-256 は `89f9c96b9cee4b56697ae9d37549edfe440ed6ea0362b4ab8c55c7b6d1c58fa7` です。取得時に両方を確かめます。
- `setup.mjs` は `PDFNumberFormat::format`、`PDFMeasure` の X・Y の係数取得、`formatDistance` の 2 形式を byte span のまま抽出します。算術の本体は変更しません。原文の MIT ヘッダー、固定ソース、抽出した定義、各 span の範囲と hash、コンパイル結果は `target/pdfmeasure/` にだけ置きます。
- 型の代替は QPointF の座標、QString 系の結果収集、係数の受け渡しだけです。`formatValue` は double と末尾の旗を収集する処理に置き換えます。`qFuzzyIsNull(double)` は [Qt 6.11 の公開 API 文書](https://doc.qt.io/qt-6.11/qtnumeric.html#qFuzzyIsNull) に示す境界を使います。Qt のソースは使用しません。

このツールが実行するのは PDF4QT の算術関数です。PDF4QT 全体のパーサーや Qt の文字整形は含みません。単位の文字、桁数・分数・丸め・区切り・前後の文字・ロケールが Qt と同じになるとは扱いません。数値の入力を渡した後の、元の距離算術と書式係数の連鎖だけを照らします。外部算術の浮動小数点の末尾の差は、コアのネイティブ・wasm のバイト一致とは分けて評価します。
