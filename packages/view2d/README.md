# @zumen-jp/view2d

zumen のシーンの列を、Canvas 2D に描く表示部品です。線・面・文字・字形・点を描き、表示範囲・レイヤ・選択を操作します。React・CSS・操作用のボタン・ポインターイベントは持ちません。[決定 0267](../../docs/decisions/0267-view2d-on-scene-columns.md) に従い、実行時の外部描画ライブラリは使いません。

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

同じ版の `drawing` のハッシュと、同じ要求の応答・blob を渡してください。列にハッシュは入っていないため、表示部品がその対応を証明できません。非同期で読む側は、古い要求の結果を渡さないようにします。保持中は列とそのバッファーを変更・転送しないでください。`setScene(null)` はシーンと報告を消去します。成功した差し替えは、同じ組の再設定でも、非表示レイヤ・選択・hover・名前付き強調を消します。カメラと表示部品の購読は保ち、必要なら続けて `fit()` を呼びます。

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

破棄した後のほかのメソッドは `Error` を返します。`destroy()` は状態の購読も解除し、返した解除関数は破棄後も呼べます。DOM のポインターイベントや非同期の資源は作りません。ポインター操作は [#586](https://github.com/zumen-jp/zumen/issues/586)、React と Worker は [#588](https://github.com/zumen-jp/zumen/issues/588) で扱います。旧版との対応は [機能の比較](../../docs/view2d.md) にあります。

## レイヤと選択

選択する参照は `{ id, path: [{ id, column, row }, ...] }` です。同じ定義の要素でも配置の道筋が違えば別々に扱います。ID を作り直さず、格子の列・行も保ちます。参照は現在のシーンにあるものだけを受け取り、知らない ID や道筋は `RangeError` にします。[決定 0274](../../docs/decisions/0274-view2d-selection-and-state.md) に契約を記録しています。

| メソッド | 内容 |
| --- | --- |
| `getLayers()` | シーンの実効のレイヤ ID の一覧。元のレイヤ名とは区別 |
| `setHiddenLayers(ids)` | 非表示のレイヤの集合を置き換え |
| `setLayersVisible(ids, visible)` | 指定したレイヤだけ表示・非表示を切り替え |
| `select(elements, mode = "replace")` | 選択を置き換え。`add`・`remove`・`toggle` も指定可能 |
| `setHovered(element)` | hover を 1 つの配置に設定。`null` で消去 |
| `setHighlight(name, elements, color = "#cc00cc")` | 名前付き強調。同じ名前を置き換え、空の一覧で除去 |

名前付き強調は最後に設定したもの、その上に選択、その上に hover の色を使います。選択色は作成時の `selectionColor`（既定 `#ff8800`）、hover は `hoverColor`（既定 `#00aacc`）です。非表示レイヤは強調していても描かず、当たり判定と描画の報告からも除きます。非表示にした配置の選択は保ち、再表示すると強調も戻ります。

## 当たり判定

`hitTest({ x, y }, tolerance = 3)` は、Canvas 左上からの CSS px と許容半径を受けます。`hitTestRect({ min_x, min_y, max_x, max_y })` は、同じ CSS px の四角を受けます。どちらも、描く順の逆順で配置の参照を返します。同じ配置の複数の線や字形は 1 件にまとめます。

線幅・面の穴・字形の塗り規則・点の形・切り抜きは描画と共通です。点の許容差は、問い合わせの円と可視の形の交差で判定します。円の中心が切り抜きの外でも、円が残された形に届けば当たります。範囲は一部でも交わる配置を返し、境界への接触も含みます。未描画の画像や未対応の文字は当たりません。

判定には共通の座標と形を使い、画素のアンチエイリアスは比べません。相殺された輪や、複数の切り抜きが辺で接するだけで描かれない部分は除きます。RGBA と、Canvas が正規化した色に明示された alpha が 0 の形も除きます。未知の CSS 色の構文を独自には解釈せず、透明だと確かめられない色は選択対象に残します。不透明な選択・強調色に変えれば、当たり判定にも戻ります。

ブラウザーのフォントで代替した文字は、ブラウザーが返す外形の四角を同じ変換で写す近似です。文字間の空白や字形の穴を正確には区別しません。コアから字形の列を渡すと、輪郭と穴で判定できます。精密な幾何の検査は L2 の API を使ってください。

```js
const hits = viewer.hitTest({ x: 120, y: 80 });
viewer.select(hits.slice(0, 1));
viewer.setHovered(hits[0] ?? null);
viewer.setHighlight("search", viewer.hitTestRect({ min_x: 0, min_y: 0, max_x: 300, max_y: 200 }));
```

## 状態の購読と出所

`getState()` は `scene`・`view`・`size`・`layers`・`hiddenLayers`・`selection`・`hovered`・`highlights` を返します。凍結した状態で、変わるまで同じ参照を返すため、状態を直接変更しないでください。`subscribe(listener)` はすべての変化で `listener(state)`、`on(type, listener)` は種類を絞って `listener({ type, state })` を呼びます。種類は `scene`・`view`・`size`・`layers`・`selection`・`hover`・`highlight` です。

登録だけでは通知せず、描画と状態の更新後に同期で通知します。通知の中から状態を変えると `RangeError` になります。通知する関数が例外を投げても残りへ通知し、最初の例外を返します。この場合、状態の更新は済んでいます。通知中に解除した購読は呼ばず、追加した購読は次の変更から呼びます。

```js
const stop = viewer.subscribe((state) => { selectionList.textContent = JSON.stringify(state.selection); });
const stopSelection = viewer.on("selection", ({ state }) => { console.log(state.selection); });
// 利用側の部品を片づけるとき
stop();
stopSelection();
```

選択の参照には、出所のレコードを複製しません。[出所へたどる例](example/source.js) は、表示した文書のハッシュを確かめ、選択した ID の空間か用紙を `entities` の `place` に渡します。定義内の要素も、この `place` で読めます。一覧を上限付きでたどり、応答にある `source.record` と `source.bytes`（`start` 以上 `end` 未満）を返します。現在の DXF・JWW・SXF はレコード番号を返し、バイト範囲は返しません。出所のない要素には番号や範囲を推測しません。[最小の例](example/index.html) では、クリックした配置の道筋と出所を画面で読めます。

## 入力の検査と上限

列の型・行数・単調な始まり・参照先・文字列の範囲・座標の有限性を確かめます。座標などの数値の絶対値は、変換の計算がオーバーフローしないよう `1e150` 以下に制限します。`text_second` の未指定の組 `[NaN, NaN]` は正規の値として扱います。応答の件数も列と照らします。未知のメジャー・マイナーバージョンは、知らない描画内容を黙って落とさず拒否します。不正な値や上限の超過は `RangeError` になり、`setScene` の検査に失敗しても現在の組と画面を保ちます。

`limits` はすべて正の整数です。列の値は合計 16,000,000、作業量は 32,000,000、報告は 10,000、1 つの文字は UTF-8 で 16,384 バイト、Canvas は 16,777,216 画素が既定の上限です。`stateItems` は選択・hover・強調の参照とグループ・非表示レイヤの合計、購読の数、当たり判定の結果の数を、それぞれ 100,000 に制限します。作業量は列の検査・並べ替え・共有された切り抜きと字形を描く回数を含み、描画前に確認します。当たり判定にも問い合わせごとに作業量の上限を使い、超過したら途中の結果を返さず `RangeError` にします。画素の一辺は 16,384、画素比は 8、拡大率は `1e-12`〜`1e12` に制限します。入力を読むコアの予算も別に設定してください。

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

合成シーンで穴・切り抜き・順序・大きい原点・色と幅・字形と代替文字・カメラの逆変換・差し替え・不正入力・上限・破棄を検査します。レイヤ・強調の優先順・状態の購読と解除・点と範囲の当たり判定も検査します。同じ定義を格子と別の INSERT に配置した合成 DXF で、選択の区別と L1 の出所を確認します。既存の適合テストの合成 DXF・JWW・SXF も、コアの wasm で読んで描きます。PR では Chromium、毎晩は Firefox・WebKit で流します。受け取った図面や外部のフォントは使いません。
