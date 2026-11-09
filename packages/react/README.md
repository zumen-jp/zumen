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

開いた図面は `await opened.convert()` で DXF に変換できます（`convert`、[決定 0206](../../docs/decisions/0206-convert-to-new-dxf.md)）。同じ Worker のセッションで変換し、書いた DXF のバイト列（`bytes`）、応答の `result`（書いた要素の数など）、変換の診断（`diagnostics`、段階は `convert`）を返します。変換した文書はセッションに残さず、元の文書とシーンは変えません。変換できなければ、`Error` の `diagnostics` に変換の診断を入れて拒みます。保存は利用側が行います。

入力は 64 MiB までです。非同期の `FileReader` で読み、専有バッファーを module Worker に転送します。Worker は自分の wasm インスタンス・セッションで `open`・`drawing`・`scene` を呼び、シーンのバッファーを転送します。表示中は同じセッションで出所を問い合わせ、DXF に変換します。同時に応答を待つ要求の上限は 32 件です。

入力・起点・読み込み設定を差し替えると、前の読み込みを中断し、Worker を終了します。実行中の wasm とセッションも Worker のメモリとともに解放します。未完了の問い合わせは `AbortError` になり、アンマウントでも同じ後始末をします。そのため用紙の切り替えではファイルを読み直します。

Activity・Suspense が表示を切り離すときも、Worker を終了してその結果を消します。同じ入力で表示を再開すると、新しい読み込みの間は `loading` を返します。利用側の effect は、再び `ready` を受け取ってから新しいセッションへ出所を問い合わせられます。

## 依存と確認

React は利用側が渡す peer dependency です。プレイグラウンドでは既存の Next.js の検査と同じ React・React DOM 19.3.1（MIT）を固定します。React DOM の scheduler も MIT です。コアと同じ図面用 wasm を使い、ネイティブと共通のシーンを既存のリーダーで読みます。

PR の Chromium と毎晩の Firefox・WebKit のブラウザー検査には、決定 0199 の Playwright を使います。製品の下限バージョンは別の検査です。とくに module Worker は Firefox 114 以降を必要とします（[対応する環境](../../docs/support.md)）。パッケージは未公開です。[決定 0267](../../docs/decisions/0267-view2d-on-scene-columns.md)
