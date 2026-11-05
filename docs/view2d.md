# 2D の表示部品と、旧版からの置き換え

表示部品は、コアが作る 2D のシーンを利用側の Canvas に描く部品です。方針は[決定 0267](decisions/0267-view2d-on-scene-columns.md)、実装の進み具合と完了条件は [#507](https://github.com/zumen-jp/zumen/issues/507) とその子 issue にあります。この文書は機能の対応表です。描画とカメラの使用案内は [@zumen-jp/view2d](../packages/view2d/README.md) にあります。選択・操作・資源の解決・React・赤入れは後続の issue で扱います。

## 比べたもの

2026-11-06 に、旧版のコミット `7c1c400e9791bd60f4a5bba355ca90362ad607c2` の次の公開ファイルを確認しました。旧版のコードは MIT ライセンスです。この比較では、機能と契約を確認し、コードは移していません。

- [viewer の README](https://github.com/zumen-jp/zumen-legacy/blob/7c1c400e9791bd60f4a5bba355ca90362ad607c2/packages/viewer/README.md)と[詳細](https://github.com/zumen-jp/zumen-legacy/blob/7c1c400e9791bd60f4a5bba355ca90362ad607c2/packages/viewer/README.en.md)、[公開する名前](https://github.com/zumen-jp/zumen-legacy/blob/7c1c400e9791bd60f4a5bba355ca90362ad607c2/packages/viewer/src/index.ts)、[DrawingViewer](https://github.com/zumen-jp/zumen-legacy/blob/7c1c400e9791bd60f4a5bba355ca90362ad607c2/packages/viewer/src/viewer.ts)
- [React のフック](https://github.com/zumen-jp/zumen-legacy/blob/7c1c400e9791bd60f4a5bba355ca90362ad607c2/packages/react/README.md)と[公開する名前](https://github.com/zumen-jp/zumen-legacy/blob/7c1c400e9791bd60f4a5bba355ca90362ad607c2/packages/react/src/index.ts)
- [図面のプレイグラウンド](https://github.com/zumen-jp/zumen-legacy/blob/7c1c400e9791bd60f4a5bba355ca90362ad607c2/playground/drawing-sample.tsx)と[赤入れのプレイグラウンド](https://github.com/zumen-jp/zumen-legacy/blob/7c1c400e9791bd60f4a5bba355ca90362ad607c2/playground/markup-sample.tsx)
- [旧版の決定 0005](https://github.com/zumen-jp/zumen-legacy/blob/7c1c400e9791bd60f4a5bba355ca90362ad607c2/docs/decisions/0005-browser-packages-for-embedding.md)、[ライセンス](https://github.com/zumen-jp/zumen-legacy/blob/7c1c400e9791bd60f4a5bba355ca90362ad607c2/LICENSE)

## 機能の対応

表の追跡先は実装の単位です。完了したかどうかは各 issue で確認できます。

| 旧版の機能・主な API | 新しい部品での形 | 追跡先 |
| --- | --- | --- |
| `renderScene`・`renderPrimitives`。線・面・文字・点の描画 | `readScene` で読んだ列を Canvas 2D へ描きます。字形はコアの輪郭を使います | [#584](https://github.com/zumen-jp/zumen/issues/584) |
| `fit`・`fitBounds`・`zoomBy`・`panBy`・`rotateBy` | 全体と指定範囲の表示、拡大・移動・回転を用意します | [#584](https://github.com/zumen-jp/zumen/issues/584) |
| `getView`・`jumpTo`・`toScreen`・`toPaper` | 表示範囲を保存・復元し、画面とシーンの起点の座標を変換します。空間の座標を用紙の mm とみなしません | [#584](https://github.com/zumen-jp/zumen/issues/584) |
| `setScene`・`destroy`、サイズと画素比への追従 | 利用側の Canvas を使い、シーン・`SceneResult`・文書の識別情報を一緒に差し替え、作った資源を破棄します | [#584](https://github.com/zumen-jp/zumen/issues/584) |
| 背景、線幅、前景色、選択色、名前付き強調色 | 表示設定を引数で受け取ります。CSS とテーマの管理は利用側に置きます | [#584](https://github.com/zumen-jp/zumen/issues/584)・[#585](https://github.com/zumen-jp/zumen/issues/585) |
| `setLayersVisible`・`setHiddenLayers` | シーンの実効のレイヤ ID で表示を切り替えます | [#585](https://github.com/zumen-jp/zumen/issues/585) |
| `select`・`setHovered`・`setHighlight` | L2 の ID と配置の道筋で、選択・hover・名前付き強調を管理します | [#585](https://github.com/zumen-jp/zumen/issues/585) |
| `HitIndex`・`entitiesAt`・`entitiesIn` | 表示と切り抜きを考慮してシーンに当たり判定し、配置ごとの要素を返します | [#585](https://github.com/zumen-jp/zumen/issues/585) |
| `getState`・`subscribe`・`on` | 状態とイベントを購読し、解除できます。出所は既存の `entities` で L1 へたどります | [#585](https://github.com/zumen-jp/zumen/issues/585) |
| 選択・計測・移動、`addTool`・`setTool` | 利用側のツールも同じ入力の入口へ組み込めるようにします | [#586](https://github.com/zumen-jp/zumen/issues/586) |
| ドラッグ・ホイール・ピンチ・ダブルクリック・キーボード | 個別に停止でき、ページのスクロールと両立できる操作を用意します | [#586](https://github.com/zumen-jp/zumen/issues/586) |
| `snapAt`・`setMeasurement` | 表示上の吸着と距離を返します。シーンの単位と実寸を区別します | [#586](https://github.com/zumen-jp/zumen/issues/586) |
| `addOverlay`・`invalidate` | 図面の上の一時の描画と、その更新を利用側から渡せるようにします | [#586](https://github.com/zumen-jp/zumen/issues/586) |
| `imageUnderlayRenderer`・`setUnderlayRenderer`・`refreshUnderlays` | 利用側が画像や PDF を解決します。表示範囲・画素数の上限・中断・失敗・再読込を扱います | [#587](https://github.com/zumen-jp/zumen/issues/587) |
| `useDrawingViewer`・`useViewerState`・`useViewerEvent` | 表示部品と購読の作成・破棄を React のフックで管理します | [#588](https://github.com/zumen-jp/zumen/issues/588) |
| `useDrawing` | 既存の wasm のラッパーを Worker で呼び、古い要求を中断し、診断とシーンを受け取ります | [#588](https://github.com/zumen-jp/zumen/issues/588) |
| ファイル・用紙・レイヤ・要素の一覧を持つプレイグラウンド | 日本語の画面を `playground/` に置き、ファイルを利用側のブラウザーで開きます | [#588](https://github.com/zumen-jp/zumen/issues/588) |
| `MarkupLayer`、赤入れ・変更雲・注記・写真位置・指摘のツール | 決定 0214・0218 のレビューの文書を使います。未実装の図形の生成処理と API をコアに足し、そのシーンを描きます | [#589](https://github.com/zumen-jp/zumen/issues/589) |
| 赤入れの選択・フィルター・変更イベント・取り消し・やり直し | レビュー文書の対象と場所を保ち、利用側へ変更を返します | [#589](https://github.com/zumen-jp/zumen/issues/589) |
| `useMarkupLayer`・`useMarkupState`・`useMarkupEvent` と赤入れのプレイグラウンド | レビューの層と購読のフック、保存・再読込の画面を用意します | [#589](https://github.com/zumen-jp/zumen/issues/589) |

## 置き換えの確認

旧版の部品と同じパッケージ名や入力モデルにはしません。`@zumen-jp/zumen` のシーンのリーダー、`@zumen-jp/view2d` の表示部品、`@zumen-jp/react` のフックを使う形へ移します。各部品の README で、入力・状態・イベント・破棄の移り方を示します。

DXF・JWW・SXF は、既存のコアで開いて作ったシーンを使って確かめます。幾何の表示だけでなく、レイヤ、同じ定義の複数の配置、ID と出所、診断、画像、操作、読み取りの中断までを子 issue で追います。シーンはコアの適合テスト、画面と入力はブラウザーのテストで確かめます。

レビューは、旧版の JSON をそのまま新しい文書とみなしません。現行の `zumen-review` 文書へ移す手順と、移せない値を報告する方法も #589 で確かめます。旧版を archive する判断は、#507 の子 issue と全体計画の残件を確認してから行います。
