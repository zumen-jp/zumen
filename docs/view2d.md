# 2D の表示部品と、旧版からの置き換え

`@zumen-jp/zumen` でシーンを読み、利用側の Canvas に `@zumen-jp/view2d` で描きます。ファイルを開く画面は[プレイグラウンド](../playground/README.md#操作)、置き換えの進捗は [#507](https://github.com/zumen-jp/zumen/issues/507) で確認できます。

## 機能の対応

| 旧版の機能・API | 移行先 |
| --- | --- |
| `renderScene`・`renderPrimitives`、背景・線幅・前景色 | `readScene` で列を読み、線・面・字形・点を[描画](../packages/view2d/README.md#描画)します。CSS とテーマは利用側で管理します |
| `setScene`・`destroy`、サイズ・画素比への追従 | [シーン・応答・文書識別](../packages/view2d/README.md#シーンと文書)を一緒に渡します。サイズの監視と Canvas の所有は利用側です |
| `fit`・`fitBounds`・`zoomBy`・`panBy`・`rotateBy`・`getView`・`jumpTo`・`toScreen`・`toPaper` | [カメラ](../packages/view2d/README.md#表示範囲と破棄)を操作します。画面からは `toScene` で元の空間・用紙の座標に戻します |
| `setLayersVisible`・`setHiddenLayers`・`select`・`setHovered`・`setHighlight` | [レイヤと選択](../packages/view2d/README.md#レイヤと選択)には実効レイヤ ID、要素 ID と配置の道筋を渡します。選択色・名前付き強調色を指定できます |
| `HitIndex`・`entitiesAt`・`entitiesIn` | [`hitTest`・`hitTestRect`](../packages/view2d/README.md#当たり判定)が、表示と切り抜きを考慮して配置を返します |
| `getState`・`subscribe`・`on` | [状態・イベントの購読](../packages/view2d/README.md#状態の購読と出所)は解除できます。選んだ配置から `findSource` で元のレコードをたどれます |
| 選択・移動・計測、`addTool`・`setTool` | [DOM の入力](../packages/view2d/README.md#入力とツール)は `attachInput` で接続し、種類ごとに停止できます。独自ツールも `handleInput` を使います |
| `snapAt`・`setMeasurement`、`addOverlay`・`invalidate` | [可視の頂点への吸着と距離の単位](../packages/view2d/README.md#吸着と計測)、[重ね描きの解除と寿命](../packages/view2d/README.md#独自ツールと重ね描きの寿命)を参照してください |
| `imageUnderlayRenderer`・`setUnderlayRenderer`・`refreshUnderlays` | 利用側が寸法と部分画像を返します。中断・非同期の解放・上限は[画像と PDF](../packages/view2d/README.md#画像と-pdf-のアンダーレイ)にあります |
| `useDrawingViewer`・`useViewerState`・`useViewerEvent`・`useDrawing` | [React のフック](../packages/react/README.md)が表示部品と購読を管理します。ファイルは Worker で開き、古い要求を中断します |
| `MarkupLayer`・`useMarkupLayer`・`useMarkupState`・`useMarkupEvent`、赤入れの画面 | 旧版 JSON は[レビュー文書へ移行](review-migration.md)できます。赤入れ・指摘の表示と編集、購読・取り消し・保存と再読込は [#589](https://github.com/zumen-jp/zumen/issues/589) で追います |

## 置き換えの確認

DXF・JWW・SXF のシーンを[コアの適合テスト](../bindings/conformance/README.md)で、レイヤ・複数配置の ID と出所・診断・画像・入力・中断を[ブラウザーの検査](../packages/view2d/README.md#検査)で確かめます。旧版のアーカイブは #507 の子 issue と[全体計画](https://github.com/zumen-jp/zumen/issues/1)の残件を確認してから判断します。

## 比べたもの

2026-11-06 に旧版の [viewer](https://github.com/zumen-jp/zumen-legacy/tree/7c1c400e9791bd60f4a5bba355ca90362ad607c2/packages/viewer)・[React](https://github.com/zumen-jp/zumen-legacy/tree/7c1c400e9791bd60f4a5bba355ca90362ad607c2/packages/react)・[プレイグラウンド](https://github.com/zumen-jp/zumen-legacy/tree/7c1c400e9791bd60f4a5bba355ca90362ad607c2/playground)の機能と契約を比較しました。旧版は [MIT](https://github.com/zumen-jp/zumen-legacy/blob/7c1c400e9791bd60f4a5bba355ca90362ad607c2/LICENSE) で、コードの移植はありません。組み込み部品の方針は[旧版の決定 0005](https://github.com/zumen-jp/zumen-legacy/blob/7c1c400e9791bd60f4a5bba355ca90362ad607c2/docs/decisions/0005-browser-packages-for-embedding.md)にあります。
