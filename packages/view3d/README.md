# @zumen-jp/view3d

zumen の 3D のシーン（ZMS3）を、利用側の Canvas に WebGPU で描く表示部品です。メッシュ・稜線・複数の配置・透明度・未解決の色を描き、要素の ID でビューごとに選択・非表示・色を指定し、切断面で切り、画面の点から要素を当てます。レビュー文書の 3D の場所と往復します。設計は [3D の表示部品](../../docs/view3d.md) に従います。

`@zumen-jp/zumen` と同じ版数で npm の配布物を組みます（初回のレジストリ公開は [#909](https://github.com/zumen-jp/zumen/issues/909) で準備中）。手元では `packages/view3d/src/index.js` を ESM として読み込み、`@zumen-jp/zumen` を `bindings/js/src/index.js` に、`three`・`three/webgpu`・`three/tsl` を利用側の three.js に解決します。

## 依存

[three.js](https://github.com/mrdoob/three.js) は必須の peer dependency で、許容する範囲は `>=0.187.0 <0.188.0` です。表示部品は three.js を同梱せず、利用側と同じ ESM の実体を使います。読み込んだ three.js の `REVISION` が `187` でなければ、互換を推測せずに `three-version` の例外を返します。検査には `bindings/js/test/browser` の lockfile で固定した 0.187.0 を使います。

## 使い方

```js
import { View3D } from "@zumen-jp/view3d";

const viewer = await View3D.create(canvas, { width: 800, height: 600, pixelRatio: window.devicePixelRatio, unresolvedColor: "#b4b4b4" });
// session は @zumen-jp/zumen のセッション。IFC は open 済み。
const { hash } = session.call({ v: 1, op: "building", doc }).result;
const result = session.call({ v: 1, op: "scene3d", doc }).result;
const report = viewer.setScene({ blob: session.take(result.blob), result, document: { doc, format: "ifc", hash } });
viewer.setViews([{
  id: "main",
  rect: { x: 0, y: 0, width: 800, height: 600 },
  camera: { projection: "perspective", position: [0, 0, 0], direction: [1, 1, -1], up: [0, 0, 1], fov: 45 },
}]);
viewer.fit("main");
viewer.render();

const { hits } = viewer.hitTest("main", { x: 400, y: 300 });
if (hits.length > 0) viewer.select({ doc, format: "ifc", hash }, [hits[0].id]);
```

- 位置とカメラはプロジェクト空間・文書の長さの単位・Z が上のままです。GPU へ渡す座標は `(塊の原点 - カメラの位置) + 変換 × 点` で、先に絶対座標を f32 にしません。three.js の `Object3D.DEFAULT_UP` は変えません。
- 相似（反転なし・一様な尺度）の配置はメッシュ・塊・不透明度ごとにまとめ描きし、反転や非一様な尺度の配置は個別の Mesh の変換で描きます。面の法線は画面上で求めます。
- 透明度 0 の配置と非表示の要素は、描画と当たり判定の両方から除きます。未解決の色は `unresolvedColor` で描き、要素ごとに `color-unresolved` を報告します。点群の点はまだ描かず、件数を `points-unsupported` で報告します。
- 当たりは光線の f64 の原点（カメラの位置）と相対位置、文書単位の距離を返し、近い順に並べます。同じ要素の当たりはもっとも近いものにまとめます。
- 新しいシーンの検査・資源の作成に失敗したら、前のシーンと状態を保ちます。上限は `limits` で下げられ、入力に比例する GPU のバイト数と作業量は資源を作る前に数えます。
- デバイスを失うと `contextlost` を通知して描画を止め、`snapshot` は `context-lost` の例外を返します。`destroy` は予約した描画・購読・通知と、部品が作った資源を解放します。Canvas は取り除きません。

## ビューごとの状態と切断面

- 選択・非表示・色はビューごとに持ちます。`setViews` の各ビューに `selected`・`visible`・`exceptions`・`colored` を渡すか、`select`・`setHidden`・`setColors` に `{ view }` を渡します。`view` を省くとすべてのビューに当てます。色の優先は選択、`colored`、要素の色の順です。同じ要素に 2 つの色を当てると `invalid-input` です。
- `clips` は `{ point, normal }` の平面で、法線の向きの側を除きます。面の上の点は残します。`box` は `{ min, max }` の箱で、外側の 6 面の切断面として数えます。切断面の数は `limits.clips`（既定 16）までです。描画と当たり判定は同じ条件で除きます。
- `getState().views` の各ビューは、カメラ・切断面・箱・要素の状態を持ちます。`await snapshot(viewId)` はそのビューの枠を device px の RGBA（上から下）で返します。

## レビュー文書との往復

`toReviewPlace(viewId, { frame, hashes })` はビューを zumen-review の 3D の場所（カメラ・切断面・選択・表示・例外・色）にし、`applyReviewPlace(viewId, document, place, { hashes })` は場所をビューに当てます。BCF への書き出しと読み込みはコアの `bcf_write`・`bcf_read` で行い、作成者・時刻・書き出し先は利用側が決めます。

- 厳密に往復するのは、長さの単位が m（接頭辞と尺度なし）、ビューの枠が 1:1、位置がプロジェクト空間の場合です。m でない文書は `unit-not-metre` を報告します。
- 1:1 でない枠は、既定（`frame: "exact"`）では `aspect-ratio` の例外です。`frame: "contain"` を渡すと枠を含む正方形へ広げ、調整した値と理由を `adjustments` に返します。当てるときは正方形を枠に収め、視野角か幅を調整して返します。
- 見つからない要素（`element-missing`）、ハッシュの違う要素（`element-changed`）、ハッシュを確かめられない要素（`hash-unchecked`）、上書きした色（`color-overridden`）は `issues` に返し、採れる要素だけ当てます。別文書の場所・不正なカメラ・3D でない場所は例外です。

## 追加描画

利用側の物は `addGeometry(items, options)` か `addObject(root, options)` で登録します。`options` に利用側の `id`、f64 の `origin`、列優先 16 値の `matrix`、シーンと同じ `unit`、`scope`（`scene` か `viewer`）、同じ距離での順を決める `priority`、`pickable`、線・点の当たりの許容差 `pickTolerance`（CSS px）を渡します。戻り値の `remove()` は何回呼んでも後始末を重ねず、`signal` は理由（`removed`・`scene`・`unit`・`destroy`）付きで abort します。

- `addGeometry` は線・範囲・円・寸法線・ラベルの記述から表示部品が資源を作り、解除で解放します。寸法の長さは文書単位で `parts` に返します。ラベルは利用側が描いた画像を CSS px の大きさで渡し、部品は写しを持ちます。文字の組版・フォント・URL の取得は行いません。
- `addObject` は親のない root を借用し、部品の wrapper に付けます。利用側の位置・材質・`userData` を書き換えず、資源を dispose しません。解除で `dispose` を渡していれば一度だけ呼びます。表示部品と同じ three.js の実体の Object3D に限ります。
- `scope: "scene"` はシーンの差し替えで外し、`viewer` は残しますが単位の違うシーンでは外します。破棄では両方を外します。後始末の 1 つが失敗しても残りを実行し、最初の例外を投げます。
- `hitTest` は要素と追加描画の当たりを距離、優先度、種類、ID の順に並べて返します。追加描画を常に手前とは扱いません。`clipping: false` の物は切断面のあるビューで描かず選ばず、`render` の通知の `excluded` に返します。
- 利用側の独自の shader や raycast は信頼したアプリのコードとして動かし、予算で中断できるとはしません。線と点の許容差は物の外接球の中心の深さで画素から換算します。

## 入力

`attachInput(element, options)` は DOM の入力を接続し、解除関数を返します。DOM を使わない場合は正規化した入力を `handleInput(input)` に渡します。標準の割り当ては、クリック・タップで選択、中ボタンで移動、Shift と中ボタンで回転、Ctrl・Command とホイールで拡大、2 本指で移動・拡大です。`setInputOptions` で個別に止められ、扱わない入力は DOM の既定の動作に残します。

- 操作は始めたビューに固定します。Escape・`pointercancel`・捕捉の喪失・フォーカスの喪失・ビューやシーンや入力設定の変更・例外・破棄で操作を止め、移動と回転は元のカメラに戻します。`cancelInput(reason)` でも止められます。
- 選択の結果は `pick`、操作の始まり・終わり・中断は `input` で通知します。通知の中から `handleInput` を呼ぶと `reentrant` の例外です。
- `touch-action`・フォーカスの扱い・ポインターの見た目は利用側が CSS で決めます。

## 地面の 2D

`@zumen-jp/view3d/ground2d` の `attachGround2D(viewer, options)` は、利用側が作った View2D と、その Canvas を 3D の平面に敷きます。表示部品の入口はこのアダプターと View2D を読み込みません。2D の線・塗り・文字・切り抜きは View2D が描いたものを texture にし、列や描画を作り直しません。

```js
import { attachGround2D } from "@zumen-jp/view3d/ground2d";

const ground = attachGround2D(viewer, {
  id: "plan", view2d, canvas: canvas2d,
  plane: { origin: [x, y, z], xAxis: [1, 0, 0], yAxis: [0, 1, 0], anchor: { x: 0, y: 0 } },
});
```

- `plane` は 2D の起点の座標 `anchor` を 3D の `origin` に置き、2D の +x・+y を直交する `xAxis`・`yAxis` に写します。尺度は 2D の単位（millimeter・centimeter・meter・inch・foot）と 3D の長さの単位から決めます。`scale` を渡すと 2D の 1 単位あたりの 3D の文書単位として使い、単位から決まる値と食い違えば `unit-mismatch` で重ねません。
- 用紙とビューの 2D（`paper`）、記号定義の中（`definition`）、単位の分からない組（`unit-unknown`）は重ねず、`getReport()` と当たり判定で理由を返します。2D・3D のシーンの差し替えで判定し直します。
- `hitTest(viewId, point)` は光線と平面の交点を、2D の原点と相対座標・View2D の CSS px に戻し、View2D の `hitTest` へ渡します。手前の 3D の要素に隠れれば `occluded`、Canvas の枠の外・切断面の外・`side: "front"` の裏は `miss` と理由を返します。背景が透明な View2D では、透明な部分の奥の 3D の要素が見えます。
- `getReport()` は texture の画素数、1 画素の 2D と 3D での長さ（解像度による近似の目安）、元の View2D の `issues` を返します。texture は Canvas の画素をそのまま使い、`maxPixels`（既定 16,777,216）・`maxSide`（既定 4096）を超えれば `limit` で重ねません。View2D の状態の変化ごとに次の 3D の描画で 1 回だけ転送します。状態を変えずに View2D の `render()` を呼んだときは `update()` を呼びます。
- `link` に `{ document, to3d, to2d, view? }` を渡すと、2D と 3D の選択を連動します。同じ建物から切り出した図面の要素の対応は利用側が渡し、同じ名前の ID を自動では結びません。2D にない参照は数えて捨てます。
- 解除・3D のシーンの差し替え（`scope: "scene"`）・単位の違うシーン・破棄で外し、購読を外して texture・geometry・material を解放します。View2D と Canvas は利用側が持ち続けます。

## 地物と点群

地物は #472 の共通の ZMS3 をそのまま `setScene` に渡します。`getElement(document, id)` は要素の ID・クラス（地物では型）・名前・所属階の ID を返し、選択・色・非表示は建物と同じく元の地物 ID で行います。`getState().scene` の `map` と `heightCrs` は応答の地図の変換と高さの基準です。応答の恒等変換を再投影の要求とは扱いません。

`@zumen-jp/view3d/pointcloud` の `attachPointCloud(viewer, options)` は、#475 の点の区画を持つ ZMS3 1.1 を、表示中の建物・地物と同じビューに重ねます。表示部品の入口はこのアダプターを読み込みません。

```js
import { attachPointCloud } from "@zumen-jp/view3d/pointcloud";

const cloud = attachPointCloud(viewer, { id: "scan", input: { blob, result, document }, scope: "viewer" });
const hit = cloud.hitTest("main", { x, y });
```

- 点群の地図の座標（メートル）を、表示中のシーンの地図の変換の逆（尺度・X 軸の回転・東・北・高さ）で文書の座標に写します。塊の行列を f64 で組み、大きな座標でも塊の原点からの差で描きます。
- 単位が m でない（`unit-unknown`・`unit-not-metre`）、座標系の名前がない・違う（`crs-unknown`・`crs-mismatch`、EPSG の 3 つの書き方はそろえて比べます）、表示中のシーンが地図の変換を持たない（`map-unknown`）、点群の地図の変換が恒等でない（`map-unsupported`）、高さの基準が両方にあって違う（`height-mismatch`）ときは重ねず、`getReport()` と当たり判定で理由を返します。高さの基準が片方にしかなければ重ね、`height: "unverified"` と返します。推測で別の基準に合わせません。
- 八分木の節（塊）ごとにインスタンシングした `THREE.Sprite`（`PointsNodeMaterial`）を作り、位置と色の列は blob から借用します。強度（点群の最大で正規化）と分類の表示色だけ 4 バイト境界の 8 ビット RGBA 色を作り、その大きさを `displayBytes` に返します。描く塊は表示範囲の中から粗い段、近い順、塊の順に `maxPoints`（既定 2,000,000）まで選び、ビューと大きさの変化で選び直します。
- `hitTest(viewId, point)` は描いている点のうち、光線から許容差（CSS px）の内の点を距離・塊・行の順に返します。切断面と切断の箱の外の点と、手前の要素（`occluder`）より奥の点は返しません。当たりは塊と塊の中の行・節・元の属性で、点ごとの永続 ID ではありません。調べる点の数は `maxWork`（既定 10,000,000）で切り、`truncated` を返します。
- `load` を渡すと `request(query)` で読み直します。新しい要求・`cancel()`・解除で前の要求の `signal` を abort し、始まる前なら `load` を呼びません。結果は `applied`・`stale`（中断後に届いた）・`cancelled`・`failed` で、失敗しても前の表示を保ちます。
- 解除・シーンの差し替え（`scope: "scene"`）・単位の違うシーン・破棄で外し、geometry と material を解放します。blob は利用側が持ち続けます。

## 検査

```bash
node bindings/js/test/browser/view3d.mjs chromium target/wasm32-unknown-unknown/release/zumen_abi1.wasm
```

合成の ZMS3 で画素・ID・位置・当たり・ビューごとの状態・切断面・pixel ratio・レビューの場所の往復・追加描画の位置と当たりの順と後始末・入力の操作と中断・地面の 2D の座標と ID と画素と寿命・地物の属性と点群の位置と色と当たりと予算と読み直しと寿命・不正な入力・上限・デバイス喪失・破棄を確かめ、コアが作った IFC のシーンで列の数・要素 ID・描画と当たりの一致を確かめ、m の IFC で BCF 2.1・3.0 の透視・平行投影の視点が `bcf_write`・`bcf_read` を通って同じ場所に戻ることを確かめます。外部の BCF の実装で view3d の出力を読むことは確かめていません。CPU の時間、`snapshot` までの時間、確定したバイト数、取れる環境での JS ヒープを別々に記録します。点群は、コアが自作の LAS から作ったシーンと、テストで組んだ 100 万点・64 塊の合成の点群で、読み込み・描画・当たり判定の CPU の時間を記録します。大きな実データの点群と GPU のメモリの上限は確かめていません。GPU に依存する画素はコアのビット一致の保証に含めません。

Playwright の Chromium 等で検査を通します。CI は Chromium で流します。[対応するブラウザー](../../docs/support.md)の下限の製品そのものでは確かめていません。

## 第三者のソフトウェア

three.js は MIT License です（Copyright © 2010-2026 three.js authors）。本文は [licenses/threejs-MIT.txt](../../licenses/threejs-MIT.txt) にあります。
