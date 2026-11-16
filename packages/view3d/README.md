# @zumen-jp/view3d

zumen の 3D のシーン（ZMS3）を、利用側の Canvas に WebGL2 で描く表示部品です。メッシュ・稜線・複数の配置・透明度・未解決の色を描き、要素の ID でビューごとに選択・非表示・色を指定し、切断面で切り、画面の点から要素を当てます。レビュー文書の 3D の場所と往復します。設計は [3D の表示部品](../../docs/view3d.md) に従います。

このパッケージは未公開です。手元では `packages/view3d/src/index.js` を ESM として読み込み、`@zumen-jp/zumen` を `bindings/js/src/index.js` に、`three` を利用側の three.js に解決します。

## 依存

[three.js](https://github.com/mrdoob/three.js) は必須の peer dependency で、許容する範囲は `>=0.187.0 <0.188.0` です。表示部品は three.js を同梱せず、利用側と同じ ESM の実体を使います。読み込んだ three.js の `REVISION` が `187` でなければ、互換を推測せずに `three-version` の例外を返します。検査には `bindings/js/test/browser` の lockfile で固定した 0.187.0 を使います。

## 使い方

```js
import { View3D } from "@zumen-jp/view3d";

const viewer = new View3D(canvas, { width: 800, height: 600, pixelRatio: window.devicePixelRatio, unresolvedColor: "#b4b4b4" });
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
- context を失うと描画を止め、戻れば描き直します。`destroy` は予約した描画・購読・通知と、部品が作った資源を解放します。Canvas は取り除きません。

## ビューごとの状態と切断面

- 選択・非表示・色はビューごとに持ちます。`setViews` の各ビューに `selected`・`visible`・`exceptions`・`colored` を渡すか、`select`・`setHidden`・`setColors` に `{ view }` を渡します。`view` を省くとすべてのビューに当てます。色の優先は選択、`colored`、要素の色の順です。同じ要素に 2 つの色を当てると `invalid-input` です。
- `clips` は `{ point, normal }` の平面で、法線の向きの側を除きます。面の上の点は残します。`box` は `{ min, max }` の箱で、外側の 6 面の切断面として数えます。切断面の数は `limits.clips`（既定 16）までです。描画と当たり判定は同じ条件で除きます。
- `getState().views` の各ビューは、カメラ・切断面・箱・要素の状態を持ちます。`snapshot(viewId)` はそのビューの枠を device px の RGBA（上から下）で返します。

## レビュー文書との往復

`toReviewPlace(viewId, { frame, hashes })` はビューを zumen-review の 3D の場所（カメラ・切断面・選択・表示・例外・色）にし、`applyReviewPlace(viewId, document, place, { hashes })` は場所をビューに当てます。BCF への書き出しと読み込みはコアの `bcf_write`・`bcf_read` で行い、作成者・時刻・書き出し先は利用側が決めます。

- 厳密に往復するのは、長さの単位が m（接頭辞と尺度なし）、ビューの枠が 1:1、位置がプロジェクト空間の場合です。m でない文書は `unit-not-metre` を報告します。
- 1:1 でない枠は、既定（`frame: "exact"`）では `aspect-ratio` の例外です。`frame: "contain"` を渡すと枠を含む正方形へ広げ、調整した値と理由を `adjustments` に返します。当てるときは正方形を枠に収め、視野角か幅を調整して返します。
- 見つからない要素（`element-missing`）、ハッシュの違う要素（`element-changed`）、ハッシュを確かめられない要素（`hash-unchecked`）、上書きした色（`color-overridden`）は `issues` に返し、採れる要素だけ当てます。別文書の場所・不正なカメラ・3D でない場所は例外です。

利用側の追加描画と入力の接続（#815）、地面の 2D（#816）、地物と点群（#817）は後続の issue で足します。

## 検査

```bash
node bindings/js/test/browser/view3d.mjs chromium target/wasm32-unknown-unknown/release/zumen_abi1.wasm
```

合成の ZMS3 で画素・ID・位置・当たり・ビューごとの状態・切断面・pixel ratio・レビューの場所の往復・不正な入力・上限・context loss・破棄を確かめ、コアが作った IFC のシーンで列の数・要素 ID・描画と当たりの一致を確かめ、m の IFC で BCF 2.1・3.0 の透視・平行投影の視点が `bcf_write`・`bcf_read` を通って同じ場所に戻ることを確かめます。外部の BCF の実装で view3d の出力を読むことは確かめていません。CPU の時間、`readPixels` までの時間、確定したバイト数、取れる環境での JS ヒープを別々に記録します。GPU に依存する画素はコアのビット一致の保証に含めません。

2026-11-17 に Playwright 1.64.1 の Chromium 156・Firefox 157・WebKit 27.2 で検査を通しました。CI は Chromium で流します。[対応するブラウザー](../../docs/support.md)の下限の製品そのものでは確かめていません。表示部品の本体は `Object.hasOwn`・`Array.at` に依存しないことだけを、これらを外して確かめます。

## 第三者のソフトウェア

three.js は MIT License です（Copyright © 2010-2026 three.js authors）。本文は [licenses/threejs-MIT.txt](../../licenses/threejs-MIT.txt) にあります。
