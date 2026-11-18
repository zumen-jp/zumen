# 2D の表示部品

`@zumen-jp/zumen` でシーンを読み、利用側の Canvas に `@zumen-jp/view2d` で描きます。ファイルを開く画面は[プレイグラウンド](../playground/README.md#操作)にあります。

## 機能の一覧

| 機能・API | 説明 |
| --- | --- |
| `readScene`、背景・線幅・前景色 | 列を読み、線・面・字形・点を[描画](../packages/view2d/README.md#描画)します。CSS とテーマは利用側で管理します |
| `setScene`・`destroy`、サイズ・画素比への追従 | [シーン・応答・文書識別](../packages/view2d/README.md#シーンと文書)を一緒に渡します。サイズの監視と Canvas の所有は利用側です |
| `fit`・`fitBounds`・`zoomBy`・`panBy`・`rotateBy`・`getView`・`jumpTo`・`toScreen` | [カメラ](../packages/view2d/README.md#表示範囲と破棄)を操作します。画面からは `toScene` で元の空間・用紙の座標に戻します |
| `setLayersVisible`・`setHiddenLayers`・`select`・`setHovered`・`setHighlight` | [レイヤと選択](../packages/view2d/README.md#レイヤと選択)には実効レイヤ ID、要素 ID と配置の道筋を渡します。選択色・名前付き強調色を指定できます |
| `hitTest`・`hitTestRect` | [当たり判定](../packages/view2d/README.md#当たり判定)が、表示と切り抜きを考慮して配置を返します |
| `getState`・`subscribe`・`on` | [状態・イベントの購読](../packages/view2d/README.md#状態の購読と出所)は解除できます。選んだ配置から `findSource` で元のレコードをたどれます |
| 選択・移動・計測、`addTool`・`setTool` | [DOM の入力](../packages/view2d/README.md#入力とツール)は `attachInput` で接続し、種類ごとに停止できます。独自ツールも `handleInput` を使います |
| `snapAt`・`setMeasurement`、`addOverlay`・`invalidate` | [可視の頂点への吸着と距離の単位](../packages/view2d/README.md#吸着と計測)、[重ね描きの解除と寿命](../packages/view2d/README.md#独自ツールと重ね描きの寿命)を参照してください |
| `underlayRenderer`・`setUnderlayRenderer`・`refreshUnderlays` | 利用側が寸法と部分画像を返します。中断・非同期の解放・上限は[画像と PDF](../packages/view2d/README.md#画像と-pdf-のアンダーレイ)にあります |
| `useDrawingViewer`・`useViewerState`・`useViewerEvent`・`useDrawing` | [React のフック](../packages/react/README.md)が表示部品と購読を管理します。ファイルは Worker で開き、古い要求を中断します |
| `ReviewStore`・`ReviewLayer`、`useReviewLayer`・`useReviewState`・`useReviewEvent` | [`ReviewStore`・`ReviewLayer`](../packages/view2d/README.md#レビューの図形と指摘)が赤入れ・指摘の編集、購読・取り消し、図面への重ね描きと選択を担い、[React のフック](../packages/react/README.md#レビューを重ねる)から使えます。保存と再読込は利用側が文書の JSON で行います |

## 確認

DXF・JWW・SXF のシーンを[コアの適合テスト](../bindings/conformance/README.md)で、レイヤ・複数配置の ID と出所・診断・画像・入力・中断を[ブラウザーの検査](../packages/view2d/README.md#検査)で確かめます。
