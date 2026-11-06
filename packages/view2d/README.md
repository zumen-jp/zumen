# @zumen-jp/view2d

zumen のシーンの列を、Canvas 2D に描く表示部品です。線・面・文字・字形・点と、利用側が解決した画像・PDF を描き、表示範囲・レイヤ・選択・計測を操作します。入力は必要なときだけ接続し、React・CSS・ラベル・操作用のボタンは利用側に置きます。[決定 0267](../../docs/decisions/0267-view2d-on-scene-columns.md) に従い、実行時の外部描画ライブラリは使いません。

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
| 画像・PDF | 利用側の解決・描画関数から受け取り、元の順序・画像枠・切り抜きで描画。関数がなければ `resource-unresolved` として未描画 |
| DWF・DGN など | `resource-kind-unsupported` として未描画 |

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

破棄した後のほかのメソッドは `Error` を返します。DOM の入力は `attachInput` で明示して接続します。`destroy()` は状態の購読・DOM のイベント・ポインターの捕捉を解除し、独自ツールも中断します。返した解除関数は破棄後も呼べます。React と Worker は [#588](https://github.com/zumen-jp/zumen/issues/588) で扱います。旧版との対応は [機能の比較](../../docs/view2d.md) にあります。

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

線幅・面の穴・字形の塗り規則・点の形・切り抜きは描画と共通です。点の許容差は、問い合わせの円と可視の形の交差で判定します。円の中心が切り抜きの外でも、円が残された形に届けば当たります。範囲は一部でも交わる配置を返し、境界への接触も含みます。画像・PDF の画素と、未対応の文字は当たり判定に含めません。

判定には共通の座標と形を使い、画素のアンチエイリアスは比べません。相殺された輪や、複数の切り抜きが辺で接するだけで描かれない部分は除きます。RGBA と、Canvas が正規化した色に明示された alpha が 0 の形も除きます。未知の CSS 色の構文を独自には解釈せず、透明だと確かめられない色は選択対象に残します。不透明な選択・強調色に変えれば、当たり判定にも戻ります。

ブラウザーのフォントで代替した文字は、ブラウザーが返す外形の四角を同じ変換で写す近似です。文字間の空白や字形の穴を正確には区別しません。コアから字形の列を渡すと、輪郭と穴で判定できます。精密な幾何の検査は L2 の API を使ってください。

```js
const hits = viewer.hitTest({ x: 120, y: 80 });
viewer.select(hits.slice(0, 1));
viewer.setHovered(hits[0] ?? null);
viewer.setHighlight("search", viewer.hitTestRect({ min_x: 0, min_y: 0, max_x: 300, max_y: 200 }));
```

## 状態の購読と出所

`getState()` は `scene`・`view`・`size`・`layers`・`hiddenLayers`・`selection`・`hovered`・`highlights` と、ツール名 `tool`・入力設定 `input`・進行中の操作 `gesture`・計測 `measurement`・画像の状態 `underlays` を返します。凍結した状態で、変わるまで同じ参照を返すため、状態を直接変更しないでください。`subscribe(listener)` はすべての変化で `listener(state)`、`on(type, listener)` は種類を絞って `listener({ type, state })` を呼びます。種類は `scene`・`view`・`size`・`layers`・`selection`・`hover`・`highlight`・`tool`・`input`・`measurement`・`underlay` です。

登録だけでは通知せず、描画と状態の更新後に同期で通知します。通知の中から状態を変えると `RangeError` になります。通知する関数が例外を投げても残りへ通知し、最初の例外を返します。この場合、状態の更新は済んでいます。通知中に解除した購読は呼ばず、追加した購読は次の変更から呼びます。

```js
const stop = viewer.subscribe((state) => { selectionList.textContent = JSON.stringify(state.selection); });
const stopSelection = viewer.on("selection", ({ state }) => { console.log(state.selection); });
// 利用側の部品を片づけるとき
stop();
stopSelection();
```

選択の参照には、出所のレコードを複製しません。[出所へたどる例](example/source.js) は、表示した文書のハッシュを確かめ、選択した ID の空間か用紙を `entities` の `place` に渡します。空間・用紙と要素のそれぞれに重複を区別する番号があっても、分けて読みます。定義内の要素も、この `place` で読めます。一覧を上限付きでたどり、応答にある `source.record` と `source.bytes`（`start` 以上 `end` 未満）を返します。現在の DXF・JWW・SXF はレコード番号を返し、バイト範囲は返しません。出所のない要素には番号や範囲を推測しません。[最小の例](example/index.html) では、クリックした配置の道筋と出所を画面で読めます。

## 入力とツール

`attachInput(element = canvas, options = {})` で DOM の入力を接続し、返す関数で解除します。同時に接続する先は 1 つで、付け替えると前の接続を解除します。カメラのメソッドと描画だけを使う場合は、接続する必要はありません。`OffscreenCanvas` でも `handleInput(input)` に正規化した入力を渡せます。座標とホイール量は CSS px です。型は [ToolInput](src/index.d.ts) を参照してください。

```js
const detach = viewer.attachInput(canvas, { wheelModifier: "ctrlOrMeta" });
viewer.setTool("pan"); // select・pan・measure、または独自ツールの名前
viewer.setInputOptions({ touch: false, keyboard: false });
// 利用側の部品を片づけるとき
detach();
```

`pointer`（マウス・ペン）・`touch`・`drag`・`pinch`・`wheel`・`keyboard`・`doubleClick` は、個別に無効にできます。いずれも既定は `true` です。`setInputOptions` は省略した設定を保ち、実際に変わると操作を中断します。

| 入力 | 既定の動作 |
| --- | --- |
| 選択ツールのクリック・タップ | 当たり判定の先頭の配置を選択。Shift は追加、Ctrl・Command は切り替え |
| 選択ツールのドラッグ | 始点から 3 CSS px 以上離したら、四角と交差する配置を選択 |
| 移動ツールのドラッグ・組み込みツールの中ボタン | カメラを移動 |
| 計測ツールのドラッグ | 始点と終点を吸着し、シーン上の距離を更新 |
| 2 つのタッチ | 重心で移動し、間隔で拡大・縮小。片方を離すと両方の操作を終了 |
| Ctrl・Command とホイール | ポインターを固定して拡大・縮小。`wheelModifier: "none"` なら修飾キーは不要 |
| ダブルクリック | その位置を固定して 2 倍に拡大。Shift と一緒なら半分に縮小 |
| 矢印キー | 40 CSS px 移動。Shift と一緒なら 200 CSS px |
| `+`・`=`・`-` | 中心を固定して拡大・縮小 |
| Home・`0` | 全体を表示 |
| Escape | 進行中の操作と計測を中断 |

処理した入力だけ `preventDefault` し、修飾キーのないホイールや未対応のキーはページへ残します。ホイールの `deltaMode` が行なら 1 行を表示用の 16 CSS px、ページなら Canvas の高さとして扱います。入力先の子にあるフォーム・編集可能な要素と、文字を変換中のキーは処理しません。

`touch-action`・`tabindex`・フォーカス・アクセシブルな名前は利用側で指定します。たとえば Canvas 内のタッチを図面の操作に使うなら、開始前に `touch-action: none` を指定します。入力設定を無効にするだけでは、利用側が禁止したページのタッチ操作を戻せません。例では `tabindex="0"` を設定し、ポインターを押したときに Canvas へフォーカスを移します。部品は CSS を変更しません。

DOM の座標は border・padding と、軸に沿う正の拡縮を除いて Canvas の CSS px に直します。CSS の回転・傾斜・反転や、独自の入力装置を使う場合は、利用側で座標を直して `handleInput` を呼んでください。カメラの回転はそのまま使えます。

## 吸着と計測

`snapAt(point, tolerance = 8)` は、許容円の中でもっとも近い可視の頂点を返します。距離が等しければ描画の逆順を優先します。対象はシーンの折れ線と面の頂点、点の中心です。非表示レイヤ・切り抜き・未描画・透明色は当たり判定と同じ扱いです。点の囲みだけを描いて中心が空白なら、中心へは吸着しません。元の円弧の中心、交点、文字や画像の点をシーンの列から作り直しません。

結果は `{ position, screen, kind, distance, tolerance }` です。`distance` と `tolerance` は CSS px、`kind` は `vertex` か `point` です。候補がなければ `null` です。`position` は `{ origin, offset, element? }` として大きな原点を相対座標から分け、吸着した位置には ID と配置の道筋を残します。`positionAt(point)` は吸着せず、この形の位置を返します。`toScreen(position)` で画面へ戻せるため、利用側で先に原点を足す必要はありません。

```js
const start = viewer.snapAt({ x: 100, y: 80 })?.position
  ?? viewer.positionAt({ x: 100, y: 80 });
const end = viewer.snapAt({ x: 200, y: 120 })?.position
  ?? viewer.positionAt({ x: 200, y: 120 });
viewer.setMeasurement({ start, end });
console.log(viewer.getState().measurement);
```

`setMeasurement` は両端の位置と、あれば現在のシーンの要素参照を検査します。結果には始点・終点・差 `delta`・シーン上の `distance` と、応答の `from`・`view`・`unit`・`tolerance` を保ちます。原点どうしと相対座標どうしの差を先に取り、大きな原点の丸めを減らします。`tolerance` はコアが曲線を展開したときの許容差で、計測の誤差保証ではありません。

実寸の扱いは `real` で区別します。起点が空間で `view` がなく、単位が既知で、両端に配置の道筋が空の要素参照がある場合だけ、`{ status: "known", distance, unit }` を返します。ほかは `{ status: "unknown", reason }` です。理由は次の表の上から優先します。利用側から要素参照を渡す場合も、その位置が参照する要素上にあることは利用側で確かめてください。

| `reason` | 意味 |
| --- | --- |
| `paper` | 用紙または表示枠を起点とする。図形や枠ごとの実寸の尺度を推測しない |
| `definition` | 空間以外の定義を起点とする |
| `unit-unknown` | 単位が不明 |
| `placement-scale` | 少なくとも片方が配置を経ている。異なる尺度や非一様な尺度を列から戻さない |
| `unbound` | 少なくとも片方が要素への吸着のない自由な位置 |

計測は表示の補助です。曲線を含む精密な距離や、配置前の実寸には L2 の API を使います。`setMeasurement(null)` で消せます。部品は計測の線と選択範囲だけを `overlayColor`（既定 `#cc0066`）で描き、数値・単位名・不明な理由のラベルは利用側へ残します。

## 独自ツールと重ね描きの寿命

`addTool(name, { onActivate, onInput, onCancel })` で独自ツールを登録し、返す関数で除きます。組み込みの名前と重複する名前は使えません。使用中のツールを除くと `select` に戻ります。`setTool` で選んだ独自ツールは、最初の入力で有効になります。各関数は同期で呼びます。`onInput(input, context)` が `true` を返した入力を消費し、未処理のホイール・キー・ダブルクリックは共通のカメラ操作へ渡します。無効にした入力は独自ツールにも渡しません。Escape とピンチは共通で扱います。

`context.viewer` は表示部品、`context.signal` は有効期間の中断の合図です。`context.subscribe`・`context.on`・`context.addOverlay` で作った購読と重ね描きは、この期間の終わりにまとめて解除します。`onActivate(context)` が返した関数も、このときに呼びます。`onCancel(reason, context)` より先に signal を中断し、後始末の 1 つが例外を投げても残りを実行します。最初の例外は呼び出し側へ返します。DOM のイベントから起きた例外はブラウザーへ伝わります。

ツール名・入力設定・入力の接続の変更、`cancelInput(reason)`、Escape、捕捉の喪失、`pointercancel`、フォーカスの喪失、ページの非表示、成功したシーンの差し替え、破棄で、捕捉したポインター・進行中の操作・計測・独自ツールの有効期間を終了します。シーンの差し替えではツールの登録・名前・DOM の接続と、表示部品の購読を保ちます。次の入力で新しい期間を始めます。破棄では登録と DOM のイベントもすべて除きます。

操作の状態は選択・計測・カメラの同期通知より先に更新します。通知中の変更の禁止は従来どおりです。独自ツールの関数から選択・カメラ・計測を更新できますが、ツール・入力の構成変更、シーンの差し替え、再度の入力、破棄はできません。これらは関数を抜けてから行います。入力の関数か同期通知が失敗した場合も、進行中の操作を片づけてから例外を返します。

`addOverlay(draw, { scope: "scene" })` は図面と標準の計測線の上に描きます。`draw(context, state)` には CSS px の座標系の Canvas 2D と現在の状態を渡し、登録順に呼びます。前後で Canvas の状態を保存・復元します。既定の `scope: "scene"` はシーンの差し替えで除き、`scope: "viewer"` は解除か破棄まで保ちます。返す関数で除けます。外部の表示データを変えたら `invalidate()` で描き直します。描画する関数の中から状態を変更したり再び描画したりしないでください。

[最小の例](example/index.html) の「位置にしるし」は、入力・吸着・重ね描き・後始末を使う独自ツールです。しるしの形・色・ラベルはすべて例の側で指定しています。API と寿命の判断は[決定 0276](../../docs/decisions/0276-view2d-input-tools-and-measurement.md) にあります。

## 画像と PDF のアンダーレイ

`underlayRenderer` または `setUnderlayRenderer(fn)` で、資源を解決する関数を渡します。表示部品は参照名を URL として開かず、入力や結果を外へ送りません。関数には `document`、元の `name` と `page`、`element`（ID と配置の道筋）、画像列の `index`、`kind`、`signal` と `limits` を渡します。ページ名を番号へ推測せず、利用側が自分の入力と結んでください。

関数は `{ width, height, render, dispose }` を返します。幅・高さは内容の単位です。シーンの `corners`（全体の枠）は内容全体を辺に収め、`unit` は内容の幅・高さを辺に掛けます。内容は左下原点で Y が上です。PDF のページ回転と内容の単位は、利用側で決めます。解決後も `SceneResult.extent` は変えず、`fit()` は元の範囲を使います。`unit` の全体を収めたい場合は、利用側から `fitBounds` へ範囲を渡します。

`render(request)` には、内容の単位の `region` と、画素比を反映済みの整数の `width`・`height` を渡します。画面と内側の切り抜きの外形まで絞った長方形です。多角形の正確な共通部分や反転した切り抜きは合成時に処理するため、要求の長方形に最後には描かない部分も含みます。利用側は、その範囲を表す左上原点の画像を `{ source, dispose }` で返します。`source` は ImageBitmap・Canvas・OffscreenCanvas のいずれかで、寸法を要求と一致させます。表示部品は画像を元の描画順に合成し、前後の図形や切り抜きを保ちます。画像自身の色と透明度を使い、図形の選択色では染めません。画像の透明部分を読む当たり判定や、画像内の点への吸着は行いません。

[画像のアダプター](example/image.js) は、利用側が復号した画像から部分画像を作ります。次の `openKnownImage` は利用側の関数です。参照名を自動で取得する関数にはせず、上限を確保前に数えて、所有権を渡せる画像とその `dispose` を返してください。

```js
import { imageUnderlayRenderer } from "./image.js";

const createCanvas = (width, height) => {
  const canvas = document.createElement("canvas");
  canvas.width = width;
  canvas.height = height;
  return canvas;
};
viewer.setUnderlayRenderer(imageUnderlayRenderer(openKnownImage, createCanvas));
const off = viewer.on("underlay", ({ state }) => {
  showUnderlayStatus(state.underlays); // 文言と画面は利用側で用意する。
});
viewer.refreshUnderlays();
// 画面を片づけるとき
off();
viewer.destroy();
```

[PDF のアダプター](example/pdf.js) は、利用側が選んだ pdf.js、PDF を読む関数、ページ名を番号へ結ぶ関数、`contentUnitsPerPoint` と Canvas を作る関数を受け取ります。アダプターは入力バイト数の上限を確かめ、渡された配列の範囲を複製して pdf.js の Worker へ渡します。利用側は同じ `Uint8Array` を再読込や別の配置にも使えます。pdf.js は表示部品の依存ではありません。入力バイト数と出力 Canvas の上限は確認しますが、pdf.js 内部の全確保量を制限する例ではありません。利用側のデコーダーに必要な実行環境と、内部の上限・中断も利用側で決めてください。

解決と描画は同期通知の外で始めます。利用側の関数や、返却値のプロパティを読む getter から同期で表示部品を変更すると `RangeError` になります。返却値の検査中に getter が投げた例外は `resource-invalid` として報告し、返却値に解放関数があれば呼びます。画面や画素比の変更で不要になった描画、関数の変更、シーンの差し替え、非表示、画面外、破棄では `signal` を中断します。古い結果と破棄後の結果は描かず、`dispose` を 1 回呼びます。戻した資源と画像は所有権を渡すため、解放されるまで変更・転送・close しないでください。同じ内容を複数の配置で共有する場合は、利用側で参照数を管理します。

`dispose` は Promise も返せます。画像を解放し、未完了の描画を待ってから資源を解放します。中断した処理や解放が未完了の間も、上限の枠を保持します。`cancelUnderlays()` は処理を止め、`refreshUnderlays()`、シーンか関数の差し替えまで再開しません。失敗も自動で繰り返さず、`refreshUnderlays()` で解決し直します。`setUnderlayRenderer(null)` は関数を除きます。

`getState().underlays` と `on("underlay", ...)` は、配置ごとの状態を返します。シーンの差し替え、レイヤやカメラの変更で画像の状態が変わると、その操作の購読と `underlay` の購読に同じ状態を渡します。`subscribe` の各購読には 1 回だけ通知します。未描画の理由は `getReport().issues` にも含めます。同じシーン・表示レイヤ・画像の状態で再描画すると、同じ報告を返します。

| 状態・理由 | 内容 |
| --- | --- |
| `ready` | 現在の要求の画素を保持し、描画可能 |
| `resource-unresolved`・`resource-pending` | 関数なし、または解決・描画・空き枠を待っている |
| `resource-cancelled`・`resource-outside-view` | 明示した中断、または非表示・画面外 |
| `resource-limit` | サイズ・総画素数・保持資源数の上限 |
| `resource-resolve-failed`・`resource-render-failed` | 解決か描画の関数、または合成が失敗 |
| `resource-invalid`・`resource-frame-invalid` | 返った寸法・関数・画像、または画像枠が不正 |
| `resource-release-failed` | 現行シーンの解放が失敗。ほかの資源の解放は続ける |
| `resource-kind-unsupported`・`clip-unsupported` | 資源の種類か切り抜きが未対応 |

非同期の結果も、描画と状態を更新してから通知します。例外の本文は状態へ写しません。解放の失敗は、その資源や画素の要求に対応させます。再表示、再読込、表示範囲の変更で次の要求へ移った後に古い解放が失敗しても、新しい画像は表示を続けます。寿命と上限の判断は[決定 0278](../../docs/decisions/0278-view2d-underlay-resources.md) にあります。

## 入力の検査と上限

列の型・行数・単調な始まり・参照先・文字列の範囲・座標の有限性を確かめます。座標などの数値の絶対値は、変換の計算がオーバーフローしないよう `1e150` 以下に制限します。`text_second` の未指定の組 `[NaN, NaN]` は正規の値として扱います。応答の件数も列と照らします。未知のメジャー・マイナーバージョンは、知らない描画内容を黙って落とさず拒否します。不正な値や上限の超過は `RangeError` になり、`setScene` の検査に失敗しても現在の組と画面を保ちます。

`limits` はすべて正の整数です。列の値は合計 16,000,000、作業量は 32,000,000、報告は 10,000、1 つの文字は UTF-8 で 16,384 バイト、Canvas は 16,777,216 画素が既定の上限です。`stateItems` は選択・hover・強調の参照とグループ・非表示レイヤの合計、購読・当たり判定の結果・独自ツール・重ね描きの各件数を、それぞれ 100,000 に制限します。作業量は列の検査・並べ替え・共有された切り抜きと字形を描く回数を含み、描画前に確認します。当たり判定と吸着にも問い合わせごとに作業量の上限を使い、超過したら途中の結果を返さず `RangeError` にします。同時に扱うポインターは最大 2 つです。利用側の関数の実行量は利用側で制限してください。画素の一辺は 16,384、画素比は 8、拡大率は `1e-12`〜`1e12` に制限します。入力を読むコアの予算も別に設定してください。

アンダーレイの既定の上限は次のとおりです。画像の件数は `stateItems`、参照名とページ名は `textLength` にも数えます。

| 設定 | 既定 | 数えるもの |
| --- | --- | --- |
| `underlayBytes` | 64 MiB | 1 資源の入力と復号。利用側へ渡し、利用側が確保前に数える |
| `underlayPixels` | 16,777,216 | 保持した画像と未完了の描画要求の総画素数 |
| `underlaySide` | 8192 | 要求する画像の一辺 |
| `underlayJobs` | 4 | 未完了の解決と描画 |
| `underlayResources` | 64 | 解決中・保持中・解放中の資源 |

中断しても完了しない利用側の処理は数え続けるため、新しい要求が止まることがあります。表示部品は利用側のデコーダー内部を強制停止できません。戻った寸法の検査だけで、復号前の上限を守れたとは扱わないでください。

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
npm ci --ignore-scripts --no-audit --no-fund --prefix xtask/verify/pdfjs
node bindings/js/test/browser/view2d.mjs chromium target/wasm32-unknown-unknown/release/zumen_abi1.wasm
```

合成シーンで穴・切り抜き・順序・大きい原点・色と幅・字形と代替文字・カメラの逆変換・差し替え・不正入力・上限・破棄を検査します。レイヤ・強調の優先順・状態の購読と解除・点と範囲の当たり判定も検査します。入力の個別の無効化、回転中のドラッグ、ピンチ、修飾キーとホイール、単位と尺度が異なる計測、吸着の可視性、独自ツールの後始末も確かめます。実際のポインター捕捉、Canvas の外へのドラッグ、タッチのタップ、利用例の独自ツールをブラウザーで動かします。同じ定義を格子と別の INSERT に配置した合成 DXF で、選択の区別と L1 の出所を確認します。既存の適合テストの合成 DXF・JWW・SXF も、コアの wasm で読んで描きます。PR では Chromium、毎晩は Firefox・WebKit で流します。受け取った図面や外部のフォントは使いません。

画像は合成の色分け、PDF は[台帳](../../samples/ledger.toml)にある PDFium の `pdfium-smask-blend.pdf`（BSD-3-Clause）と合成の 2 ページを使います。ページ・回転・切り抜き・画素比、古い非同期結果、例外、中断、再読込、非同期の解放と上限を検査します。公開 PDF は pdf.js で直接描いた画素とも照らします。pdf.js がなければ PDF の検査だけを飛ばします。CI は `ZUMEN_PDFJS_REQUIRED=1` を指定して必須にします。pdf.js の現在のブラウザー向け API を利用側の例に使うことは、表示部品自身のブラウザーの下限を変えません。
