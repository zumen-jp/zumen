# @zumen-jp/view2d

zumen のシーンの列を、Canvas 2D に描く表示部品です。線・面・文字・字形・点を描き、表示範囲を操作します。React・CSS・操作用のボタン・ポインターイベントは持ちません。[決定 0267](../../docs/decisions/0267-view2d-on-scene-columns.md) に従い、実行時の外部描画ライブラリは使いません。

このパッケージは未公開です。名前の確保と公開は別の作業です。手元では `packages/view2d/src/index.js` を ESM として読み込み、`@zumen-jp/zumen` を `bindings/js/src/index.js` に解決します。バンドラーの alias か、[最小の例](example/index.html) の import map を使えます。例を開くサーバーはリポジトリのルートを配り、開発用のポートは 24100〜24199 にします。

最小の例の [import map](https://github.com/mdn/browser-compat-data/blob/188707c73f5a93701429eadcdaf35d6e31715c67/html/elements/script.json) は、Firefox 108・Safari 16.4 以降で使えます。それより前の環境ではバンドラーで解決します。表示部品の本体は import map を作らず、[ラッパーの下限](../../docs/support.md)で使えない `Object.hasOwn`・`Array.at` にも依存しません。`OffscreenCanvas` がない環境では通常の Canvas を使えます。下限の製品そのものでの検査はまだ行っていません。

## 使い方

```js
import { readScene } from "@zumen-jp/zumen";
import { View2D } from "@zumen-jp/view2d";

const viewer = new View2D(canvas, {
  width: 800,
  height: 600,
  pixelRatio: window.devicePixelRatio,
  background: "#ffffff",
  foreground: "#111111",
});

// session は @zumen-jp/zumen のセッション。文書は open 済み。
const drawing = session.call({ v: 1, op: "drawing", doc }).result;
const result = session.call({ v: 1, op: "scene", doc, from: "space/0" }).result;
const scene = readScene(session.take(result.blob));
const report = viewer.setScene({
  scene,
  result,
  document: { doc, format: "dxf", hash: drawing.hash },
});
viewer.fit();

// 代替と未描画は、要素 ID・配置の道筋・理由と一緒に返る。
console.log(report.issues);
const saved = viewer.getView();
viewer.zoomBy(2, { x: 400, y: 300 });
viewer.panBy(20, 10);
viewer.rotateBy(Math.PI / 6);
viewer.jumpTo(saved);
viewer.destroy();
```

Canvas の CSS の幅・高さは利用側で指定します。`resize(width, height, pixelRatio)` の幅・高さと、カメラの画面座標は CSS px です。画素比の既定は 1 です。`ResizeObserver` や画素比の変化の監視も利用側で行います。同じ Canvas の描画コンテキストは共有せず、表示部品を使う間はほかのコードから描かないでください。

## シーンと文書

`setScene` は `Scene`・対応する `SceneResult`・文書識別を一組で受け取り、検査が成功したときだけ差し替えます。応答の `from`・`view`・`unit`・`tolerance` と文書の `doc`・`format`・`hash` は一緒に保持します。文書の名前だけで入力や版を区別しません。`getScene()` で現在の組を読めます。メタデータは写して凍結し、シーンの列は写さずに使います。

同じ版の `drawing` のハッシュと、同じ要求の応答・blob を渡してください。列にハッシュは入っていないため、表示部品がその対応を証明できません。非同期で読む側は、古い要求の結果を渡さないようにします。保持中は列とそのバッファーを変更・転送しないでください。`setScene(null)` はシーンと報告を消去します。差し替えではカメラを保ち、必要なら続けて `fit()` を呼びます。

単位は応答の値をそのまま保ちます。`unknown` を mm とみなさず、用紙から実寸への縮尺も推測しません。`toScreen({ x, y })` と `toScene({ x, y })` は、シーンの起点の座標と Canvas の左上からの CSS px を相互に変換します。画面の Y 軸は下向き、起点の Y 軸は上向きです。精密な測定は L2 の API を使います。

## 描画

塊の順に進み、塊の中は描く単位の番号の順に描きます。同じ単位では面・線・文字・点・画像の順と、各列の順を保ちます。線種や曲線を解き直さず、既存の頂点を使います。大きい原点はカメラの中心を引いてから相対座標を足し、f32 に丸めません。

| 対象 | 描き方 |
| --- | --- |
| 線・面 | 折れ線と、偶奇規則で穴を持つ塗り |
| 色 | `color_rgba`。前景のフラグは `foreground`。未解決は `unresolvedColor`（既定 `#777777`）で代替し、報告 |
| 線幅 | 用紙 mm に `pixelsPerMillimeter`（既定 `96 / 25.4`）を掛けた CSS px。ズームに連動しない。0 と細い線は 1 画素 |
| 切り抜き | 組の多角形すべての共通部分。外側を残すフラグも適用 |
| 字形のある文字 | `glyph_axes`・`glyph_xy` で輪郭を写し、非零規則で塗る。元の文字列を重ねない |
| 字形のない単純な文字 | ブラウザーの汎用フォントで代替。高さ・幅・回転・斜体・反転・基本の位置合わせを使う。文字の計量・高さの基準・字形の一致は保証せず、`browser-text` を報告 |
| 書式つき・縦書き・積み上げ・折り返し・2 点間の文字 | 字形がなければ `text-layout-unsupported` として未描画。コアへフォントを渡して字形を得る |
| 点 | 文書の点の形・囲み・回転と、起点の長さか画面の高さに対する大きさを使う。文書に形がなければ小さな点 |
| 既定義のマーカー | 小さな点で代替し、`marker-unresolved` を報告 |
| 画像・PDF など | `resource-unresolved` として未描画。参照名を取得せず、外へ送信しない。解決関数は [#587](https://github.com/zumen-jp/zumen/issues/587) で追加予定 |

代替文字の `horizontal.middle` は、横を送り幅の中央、縦をブラウザーが返す字形外形の中央に置きます。垂直の指定には依存せず、位置合わせは斜体を当てる前に決めます（[決定 0246](../../docs/decisions/0246-dxf-text-height-and-alignment-against-autocad.md)）。

`render()` と描画を変えるメソッドは `RenderReport` を返します。`drawn` は描画を発行した数で、画面外や切り抜かれたものも含みます。`issues` は代替（`fallback`）と未描画（`omitted`）を区別し、元の要素 ID と、配置の ID・格子の列・行を保ちます。コアの診断は呼び出し側で別に保持してください。ブラウザーの画素は、コアのシーンと異なり環境間でビット単位の一致を約束しません。

## 表示範囲と破棄

| メソッド | 内容 |
| --- | --- |
| `getView()`・`jumpTo(view)` | 起点の座標の `center`、CSS px / 起点の長さの `scale`、反時計回りのラジアンの `rotation` を保存・復元 |
| `fit(padding = 16)` | `SceneResult.extent` に収める。文字は基点だけの範囲で、代替文字の外形は含まない。範囲がなければ変更なし |
| `fitBounds(extent, padding = 16)` | 指定した起点の範囲を、回転を保って収める。余白は CSS px |
| `zoomBy(factor, anchor?)`・`rotateBy(radians, anchor?)` | 画面上の固定点を保って拡大・回転。固定点の既定は中心 |
| `panBy(dx, dy)` | 描画が画面上で右・下へ動く向きの CSS px |
| `resize(width, height, pixelRatio?)` | カメラを保って Canvas の画素数を変え、描き直す |
| `getSize()`・`getReport()`・`render()` | 現在の大きさ・報告の取得と、明示的な再描画 |
| `destroy()` | 画面を消し、シーンと Canvas への参照を解放。繰り返して呼べる。Canvas 自体は利用側に残る |

破棄した後のほかのメソッドは `Error` を返します。この実装はイベントの購読や非同期の資源を作りません。選択とレイヤは [#585](https://github.com/zumen-jp/zumen/issues/585)、ポインター操作は [#586](https://github.com/zumen-jp/zumen/issues/586)、React と Worker は [#588](https://github.com/zumen-jp/zumen/issues/588) で扱います。旧版との対応は [機能の比較](../../docs/view2d.md) にあります。

## 入力の検査と上限

列の型・行数・単調な始まり・参照先・文字列の範囲・座標の有限性を確かめます。座標などの数値の絶対値は、変換の計算がオーバーフローしないよう `1e150` 以下に制限します。`text_second` の未指定の組 `[NaN, NaN]` は正規の値として扱います。応答の件数も列と照らします。未知のメジャー・マイナーバージョンは、知らない描画内容を黙って落とさず拒否します。不正な値や上限の超過は `RangeError` になり、`setScene` の検査に失敗しても現在の組と画面を保ちます。

`limits` はすべて正の整数です。列の値は合計 16,000,000、作業量は 32,000,000、報告は 10,000、1 つの文字は UTF-8 で 16,384 バイト、Canvas は 16,777,216 画素が既定の上限です。作業量は列の検査・並べ替え・共有された切り抜きと字形を描く回数を含み、描画前に確認します。画素の一辺は 16,384、画素比は 8、拡大率は `1e-12`〜`1e12` に制限します。入力を読むコアの予算も別に設定してください。

## 検査

型はサイトの固定した TypeScript、描画は既存の固定した Playwright を使います。

```bash
# site で npm ci --ignore-scripts を実行してから
cd site
npx --no-install tsc -p ../packages/view2d/tsconfig.json
cd ..
cargo build -p zumen-abi --target wasm32-unknown-unknown --release --locked
cd bindings/js/test/browser
npm ci --ignore-scripts
npx --no-install playwright install --only-shell chromium
cd ../../../..
node bindings/js/test/browser/view2d.mjs chromium target/wasm32-unknown-unknown/release/zumen_abi1.wasm
```

合成シーンで穴・切り抜き・順序・大きい原点・色と幅・字形と代替文字・カメラの逆変換・差し替え・不正入力・上限・破棄を検査します。既存の適合テストの合成 DXF・JWW・SXF も、コアの wasm で読んで描きます。PR では Chromium、毎晩は Firefox・WebKit で流します。受け取った図面や外部のフォントは使いません。
