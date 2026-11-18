# @zumen-jp/react

React で図面ファイルを開き、Canvas に表示するフックです。描画・レイヤ・選択には `@zumen-jp/view2d` を使います。組み込んだ例は[プレイグラウンド](../../playground/README.md)にあります。

## 表示と購読

`useDrawingViewer(canvas, options?)` は、Canvas の接続後に `View2D` を返し、Canvas・設定の差し替えとアンマウントで破棄します。接続前は `null` です。Canvas はコールバック ref で渡せます。設定には同じ参照を渡し、変更する場合だけ作り直してください。

`useViewerState(viewer)` は `useSyncExternalStore` で表示部品の状態を購読します。選択は要素の ID と配置の道筋を持ちます。`useViewerEvent(viewer, type, listener)` ではイベントの種類を絞れます。再描画後のコールバックを使い、表示部品か種類が変わると前の購読を解除します。

Suspense・Activity が表示を再開すると、同じ Canvas でも新しい表示部品を作り、その状態・イベントを購読します。

```jsx
const [canvas, setCanvas] = useState(null);
const viewer = useDrawingViewer(canvas);
const state = useViewerState(viewer);
useViewerEvent(viewer, "selection", ({ state }) => onSelection(state.selection));

return <canvas ref={setCanvas} aria-label="図面" />;
```

旧版から移すときは、シーンと文書識別を `setScene` に一緒に渡してください。`viewer.attachInput()` とサイズの監視は利用側で接続し、effect の後始末で解除します。操作の文言・ラベル・CSS は利用側で用意します。

## ファイルを開く

`useDrawing(file, { loadModule, from?, catalog? })` は `idle`・`loading`・`ready`・`error` の順に読み込みの状態を返します。`file` には利用側が選んだ `File` か `Blob` を渡し、`null` で閉じます。`loadModule(signal)` は、利用側が用意した wasm のコンパイル済み `WebAssembly.Module` を返します。関数の参照は固定し、非同期の取得では `signal` を使ってください。

```jsx
const opened = useDrawing(file, { loadModule, from, catalog });
useLayoutEffect(() => {
  if (!viewer) return;
  viewer.setScene(opened.status === "ready" ? opened.input : null);
  if (opened.status === "ready") viewer.fit();
}, [viewer, opened]);
```

`from` は `drawing` の用紙・空間の ID です。省略時は最初の空間、空間がなければ最初の用紙を選びます。`ready` の `input` には同じ文書・起点のシーン、応答、文書識別が入ります。`drawing` には用紙・空間、`diagnostics` には処理の段階、コアの診断、`describe` で作った文が入ります。言語のカタログを JSON の文字列で渡せます。読み込み失敗の `error` でも、コアが返した診断を読めます。

選択した要素の出所は `await opened.getSource(element)` で問い合わせます。見つかれば ID・配置の道筋と `source`、要素がなければ `null` を返します。出所にあるレコード・ハンドル・インスタンス・バイト範囲をそのまま使います。選択が変わったときは、前の要素の応答を画面へ反映しないようにします。

`opened.canEstimate` は、その wasm の `capabilities` が `convert_estimate` を提供するかを示します。対応する wasm なら `await opened.estimate({ limit: 100 })` で、DXF に保存する前の忠実度と欠落を確認できます。`result.estimate` に予定の要素数、種類と忠実度ごとの全件集計、上限付きの明細と元の場所を返します。`limit` は明細の件数で、省略時は 100 件、最大は 1000 件です。文書や配布するファイルは登録しません。診断の段階は `convert_estimate` です。

確認した見積もりを `await opened.convert(preview)` に渡すと、その予定文書名で変換し、見積もりと出力のハッシュ・要素数を比べてから DXF のバイト列を返します。渡せるのは同じ読み込みで最後に得た見積もりです。もう一度見積もるか、入力・起点を切り替えると、前の見積もりでは保存できません。

```js
const preview = await opened.estimate();
// 利用側で集計と診断を表示し、保存の操作を受けてから呼ぶ。
const { bytes, result, diagnostics } = await opened.convert(preview);
```

見積もりを渡さず `await opened.convert()` で直接 DXF に変換する使い方も維持します（`convert`、[DXF の読み書き](../../docs/formats/dxf.md)）。同じ Worker のセッションで変換し、書いた DXF のバイト列（`bytes`）、応答の `result`（書いた要素の数など）、変換の診断（`diagnostics`、段階は `convert`）を返します。変換した文書はセッションに残さず、元の文書とシーンは変えません。変換できなければ、`Error` の `diagnostics` に変換の診断を入れて拒みます。保存は利用側が行います。

入力は 64 MiB までです。非同期の `FileReader` で読み、専有バッファーを module Worker に転送します。Worker は自分の wasm インスタンス・セッションで `open`・`drawing`・`scene` を呼び、シーンのバッファーを転送します。表示中は同じセッションで出所を問い合わせ、DXF に変換します。同時に応答を待つ要求の上限は 32 件です。

入力・起点・読み込み設定を差し替えると、前の読み込みを中断し、Worker を終了します。実行中の wasm とセッションも Worker のメモリとともに解放します。未完了の問い合わせは `AbortError` になり、アンマウントでも同じ後始末をします。そのため用紙の切り替えではファイルを読み直します。

Activity・Suspense が表示を切り離すときも、Worker を終了してその結果を消します。同じ入力で表示を再開すると、新しい読み込みの間は `loading` を返します。利用側の effect は、再び `ready` を受け取ってから新しいセッションへ出所を問い合わせられます。

## レビューを重ねる

`opened.canReview` は、その wasm の `capabilities` が `review_scene` を提供するかを示します。対応する wasm なら `await opened.reviewScene(text, { target, style, tolerance? })` で、レビューの文書（JSON の文字列）の対象を、開いた起点のシーンにします（[#589](https://github.com/zumen-jp/zumen/issues/589)）。文書は呼ぶたびに渡し、Worker は描いた後に登録を捨てます。`input` は `ReviewLayer` の `setResult` にそのまま渡せます。文書の検査と診断はコアが行い、描けなければ `Error` の `diagnostics` にレビューの診断を入れて拒みます。診断の段階は `review_scene` です。

`useReviewLayer(base, overlay, store?)` は、図面の表示部品と、背景を透明にして重ねた表示部品が揃う間だけ `ReviewLayer` を返します。`useReviewState(store)` と `useReviewEvent(store, type, listener)` は、`@zumen-jp/view2d` の `ReviewStore` を `useViewerState`・`useViewerEvent` と同じ方法で購読します。

```jsx
const store = useMemo(() => new ReviewStore(text), [text]);
const state = useReviewState(store);
const layer = useReviewLayer(viewer, overlay, store);
useEffect(() => {
  if (!layer || !state || opened.status !== "ready" || !opened.canReview) return;
  let current = true;
  opened.reviewScene(store.filteredJSON(), { target: "plan", style })
    .then(({ input }) => current && layer.setResult(input), () => current && layer.setResult(null));
  return () => { current = false; };
}, [layer, opened, store, state?.document, state?.filter]);
```

## 3D の表示

3D のフックは `@zumen-jp/react/view3d` から読み込みます（[#818](https://github.com/zumen-jp/zumen/issues/818)）。表示部品は `@zumen-jp/view3d` と three.js 0.187 で、どちらも 3D を使うときだけ要る peer dependency です。2D だけを使う利用側は、この入口を読まない限り three.js を読みません。

`useView3D(canvas, options?)` は Canvas の接続後に `View3D` を返し、Canvas・設定の差し替えとアンマウントで破棄します。GPU の資源と RAF も一緒に放します。`useView3DInput(viewer, element, options?)` は `attachInput` を要素につなぎ、差し替えとアンマウントで外します。`useView3DState` と `useView3DEvent` は 2D の `useViewerState`・`useViewerEvent` と同じ購読です。

`useScene3D(file, { loadModule, catalog?, tolerance? })` は、入力の先頭のバイト列で形式を判定し、その形式の `scene3d` を持つモジュール（`bim`・`civil`・`pointcloud`）の名前で `loadModule(name, signal)` を呼びます。どのモジュールにもない形式は `NotSupportedError`（`scene3d.format.unsupported`、`format` 付き）で拒みます。入力は 256 MiB までで、入力ごとに module Worker を専有し、`open`・`scene3d` を呼んでシーンのバッファーを転送します。

```jsx
const opened = useScene3D(file, { loadModule });
const viewer = useView3D(canvas, options);
useLayoutEffect(() => {
  if (!viewer) return;
  viewer.setScene(opened.status === "ready" ? opened.input : null);
  if (opened.status === "ready") viewer.fit();
}, [viewer, opened]);
```

`ready` の `kind` は `building`（IFC）・`feature`（CityGML など）・`pointcloud` です。`elements` はシーンの行の順で最初の 1000 件の ID・クラス・名前、`total` は総数、`bounds` は塊の原点と範囲から求めた文書の座標の範囲です。範囲は視点や追加描画の位置の目安で、要素の厳密な範囲ではありません。点群は `@zumen-jp/view3d/pointcloud` の `attachPointCloud` に同じ `input` を渡して描きます。

IFC では、`await opened.getElement(id)` が `building_elements` の名前・クラス・空間・親・プロパティを返します。`opened.writeBcf({ title, place, author, date, version })` は `toReviewPlace` の場所を開いたモデルへの指摘にした BCF 2.1・3.0 のバイト列を返し、`opened.readBcf(bytes)` は BCF を開いたモデルに当てて読み、`applyReviewPlace` に渡せる場所を返します。保存とファイル選択は利用側の明示操作で行います。IFC 以外ではどれも `null` です。

入力・設定の差し替え、Activity・Suspense の切り離し、アンマウントでは、前の読み込みを中断して Worker を終了します。未完了の問い合わせは `AbortError` になります。

## 依存と確認

React は利用側が渡す peer dependency です。プレイグラウンドでは既存の Next.js の検査と同じ React・React DOM 19.3.1（MIT）を固定します。React DOM の scheduler も MIT です。コアと同じ図面用 wasm を使い、ネイティブと共通のシーンを既存のリーダーで読みます。

PR の Chromium と毎晩の Firefox・WebKit のブラウザー検査には、[適合テスト](../../bindings/conformance/README.md) の Playwright を使います。製品の下限バージョンは別の検査です。とくに module Worker は Firefox 114 以降を必要とします（[対応する環境](../../docs/support.md)）。パッケージは未公開です。[2D の表示部品](../../docs/view2d.md)
