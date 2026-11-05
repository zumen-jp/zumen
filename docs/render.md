# 図面から SVG を作る

`render` は、`open` で開いた図面を `scene` と同じ規則で描き、SVG 1.1 を返します。図面のモジュールと全体版で使えます。ブラウザーや OS のフォントを使わず、同じ入力・指定から同じバイト列と診断を返します。メッセージのスキーマのマイナーバージョン 64 からの機能です。

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

結果の `blob` を `take` で取り出すと SVG の UTF-8 のバイト列を得ます。`blobs` の種類は `image`、MIME type は `image/svg+xml` です。結果には形式とピクセル数、起点・ビュー・座標の単位・使った許容差も入ります。縦横同率で中央に収め、表示範囲の外を切ります。余白にも背景を塗ります。DPI は用紙 mm の線幅の換算だけに使い、線幅 0 は 1 ピクセルです。

`view`・`print`・`palette`・`fonts`・`heights` は `scene` と同じ指定です。文字を描くには、利用者が用意したフォントを `push` して `fonts: [{ "input": "font.ttf" }]` のように指定します。名前を合わせるフォントと、名前のない既定のフォントを使えます。フォント名・コレクションの番号・宣言値の高さの意味も `scene` と同じです。OS のフォントや外部画像を自動で取得しません。

字形のない空でない文字、外部画像、名前付きの点記号は描かず、診断を返します。診断の元の要素 ID と `unit` は変えません。配置の道筋が必要なら、同じ起点・範囲・フォントなどを指定した `scene` から `unit_entities`・`entity_paths`・`step_cells` をたどれます。

予算と中断は図面の変換・シーン生成・描画を通して数えます。内部のシーンを読む量も `input_bytes` に含みます。停止と不正な指定では画像を返しません。`failed` の応答の診断を確認してください。元の文書はそのままなので、指定を直して再実行できます。

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

末尾にフォントのファイルを指定すると、名前のない既定のフォントとして渡します。各言語のバインディングでも、同じ JSON の要求と `take` を使います。PNG の出力は [#604](https://github.com/zumen-jp/zumen/issues/604) で追跡しています。判断は[決定 0275](decisions/0275-render-op-from-drawing-scene.md)、SVG の範囲と検証は [SVG の文書](formats/svg.md)にあります。
