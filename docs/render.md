# 図面から SVG・PNG を作る

`render` は、`open` で開いた図面を `scene` と同じ規則で描き、SVG 1.1 か PNG を返します。図面のモジュールと全体版で使えます。ブラウザーや OS のフォントを使わず、同じ入力・指定から同じバイト列と診断を返します。SVG はメッセージのスキーマ 1.64、PNG は 1.65 から使えます。

`drawing` か `entities` で起点の ID を調べ、次のように呼びます。範囲は起点の座標です。用紙からなら mm、空間からならその空間の単位を使います。

```json
{
  "v": 1, "op": "render", "doc": "a.dxf", "from": "space/0",
  "format": "svg", "width": 640, "height": 480,
  "region": [0, 0, 100, 100], "dpi": 96,
  "background": [255, 255, 255, 255], "foreground": [0, 0, 0, 255],
  "tolerance": 0.01
}
```

`format` は `svg` か `png` です。結果の `blob` を `take` で取り出すと、SVG の UTF-8 か PNG のバイト列を得ます。`blobs` の種類は `image`、MIME type はそれぞれ `image/svg+xml`・`image/png` です。結果には形式とピクセル数、起点・ビュー・座標の単位・使った許容差も入ります。縦横同率で中央に収め、表示範囲の外を切ります。余白にも背景を塗ります。DPI は用紙 mm の線幅の換算だけに使い、線幅 0 は 1 ピクセルです。

`view`・`print`・`palette`・`fonts`・`heights` は `scene` と同じ指定です。文字を描くには、利用者が用意したフォントを `push` して `fonts: [{ "input": "font.ttf" }]` のように指定します。名前を合わせるフォントと、名前のない既定のフォントを使えます。フォント名・コレクションの番号・宣言値の高さの意味も `scene` と同じです。OS のフォントや外部画像を自動で取得しません。

字形のない空でない文字、外部画像、名前付きの点記号は描かず、診断を返します。診断の元の要素 ID と `unit` は変えません。配置の道筋が必要なら、同じ起点・範囲・フォントなどを指定した `scene` から `unit_entities`・`entity_paths`・`step_cells` をたどれます。

予算と中断は図面の変換・シーン生成・描画を通して数えます。内部のシーンを読む量も `input_bytes` に含みます。停止と不正な指定では画像を返しません。`failed` の応答の診断を確認してください。元の文書はそのままなので、指定を直して再実行できます。

PNG は RGBA 8 bit・非インターレースで、圧縮しない DEFLATE ブロックを使います。出力はおおむね幅×高さ×4 バイトです。最大 16,777,216 画素、変換後の座標・線幅・半径は絶対値 2^31 ピクセル以内に制限します。大きすぎる寸法はシーンを作る前に、描画中の範囲外は画像全体を失敗にして診断します。

PNG のアンチエイリアスは 4×4 の固定サンプル、合成は sRGB の source-over です。各画素のサンプル位置は縦横とも `(1/8, 3/8, 5/8, 7/8)` です。透明な背景の RGB は 0 にそろえます。SVG のブラウザーごとのアンチエイリアスと同じ画素になることは保証しません。

## Node.js から実行する例

[例のスクリプト](../examples/render-svg.mjs)は、リポジトリの JavaScript バインディングと、指定した wasm を使います。次の設定を `render-options.json` として用意します。サイズ・範囲・色は対象の図面に合わせます。

```json
{
  "from": "space/0", "width": 640, "height": 480,
  "region": [0, 0, 100, 100], "dpi": 96,
  "background": [255, 255, 255, 255], "foreground": [0, 0, 0, 255]
}
```

```sh
cargo build -p zumen-abi --target wasm32-unknown-unknown --release
node examples/render-svg.mjs target/wasm32-unknown-unknown/release/zumen_abi1.wasm a.dxf drawing.svg render-options.json
```

末尾にフォントのファイルを指定すると、名前のない既定のフォントとして渡します。各言語のバインディングでも、同じ JSON の要求と `take` を使います。PNG は [PNG の例](../examples/render-png.mjs)を同じ引数で呼びます。

```sh
node examples/render-png.mjs target/wasm32-unknown-unknown/release/zumen_abi1.wasm a.dxf drawing.png render-options.json
```

判断は[決定 0275](decisions/0275-render-op-from-drawing-scene.md)・[0277](decisions/0277-bounded-png-rendering.md)、対応と検証は [SVG](formats/svg.md)・[PNG](formats/png.md) の文書にあります。
