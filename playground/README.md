# 図面のプレイグラウンド

DXF・DWG・JWW・SXF を手元のブラウザーで開き、描画した要素を元のレコードへたどれます。図面はファイル選択で読み込むか、同梱した公開サンプルから選びます。画像・PDF・フォントの参照名を URL として取得せず、図面や結果も送信しません。

## 起動

リポジトリのルートで次を実行します。Node.js 24 以降と、リポジトリで指定した Rust を使います。

```sh
rustup target add wasm32-unknown-unknown
cargo build -p zumen-abi --release --target wasm32-unknown-unknown --locked --no-default-features --features drawing
npm ci --ignore-scripts --prefix playground
npm run build --prefix playground
npm start --prefix playground
```

[図面ビューアー](http://127.0.0.1:24188/)を開きます。図面用 wasm は Vite（8.3.5、MIT）がローカルの配布物に含めます。別の場所にビルドした場合は `ZUMEN_DRAWING_WASM` にそのファイルのパスを指定してから `npm run build` します。

公開サンプルは、`samples.js` の `SAMPLE_PATHS` に挙げたものを配布物に入れます。サンプルは、プレイグラウンドのライセンス（MIT OR Apache-2.0）ではなく、それぞれのライセンスに従います（[公開サンプルの条件](../samples/README.md)）。[公開サンプルの条件](../samples/README.md) は、配る crate とパッケージに `samples/` を入れないとしています。プレイグラウンドは `private` で、パッケージとしては配りません。そのため、配布物に入れても、この決定に反しないと読みます。サイトに載せるとき（#691）も、`samples.md` を一緒に置きます。出所とライセンスは `samples/ledger.toml` から読み、画面と `dist/samples.md` に出します。`samples.md` には、本文の同梱を求めるライセンス（MIT）の本文も入れます。JWW は線・円・寸法を試せる図面を、DXF は内容の異なる 2 件を、DWG は R2000・R2004・R2018 を収録しています。自前で作った小さな図面も、公開サンプルの台帳（[公開サンプルの条件](../samples/README.md)）に載せる手間に比べて、デモで見せられることが少ないので入れません（#689）。

CSS は作成したファイルをそのままコピーし、ビルド後のバイト列も検査します。Vite の開発用の依存にある Lightning CSS（MPL-2.0）は、CSS の生成・圧縮や実行時のコードに使いません（[配布の手順](../xtask/release/README.md)）。ビルドした JS に含めた依存のライセンスは `dist/licenses.md` に出力します。

## 操作

1. 「開く」か図面のドロップで DXF・DWG・JWW・SXF（SFC・P21・SFZ・P2Z）を選びます。「公開サンプル」から選ぶこともできます。サンプルは配布物の中のファイルで、「案内」で提供元とライセンスを確認できます。
2. 「用紙・空間」で表示の起点を選びます。読み込み中も矢印キーで続けて切り替えられます。「全体を表示」と「拡大」「縮小」で表示範囲を調整できます。図面にフォーカスを置くと、矢印で移動し、Home で全体を表示します。
3. 「距離計測」で始点から終点までドラッグすると距離が出ます。「選択」「移動」でツールを切り替えます。「レイヤ・要素」を押すと詳細パネルが開き、チェックボックスでレイヤの表示を切り替えられます。図形をクリックするか、要素のボタンに Tab で移動して Enter または Space で選びます。
4. 「出所」で要素 ID・配置の道筋と、元のレコードなどを読めます。続けて選ぶと、最後に選んだ要素の出所を表示します。「診断」にはコアの日本語の説明・種類・件数・場所と、表示の代替・未描画が出ます。同じ要素を格子に配置した場合は、配置の ID・列・行で区別できます。
5. 「DXF 書き出し」を開き、「保存前の見積もりを確認」で、保持・等価・近似・見た目だけ・欠落の件数と、元の場所を確認します（`convert_estimate`）。全件の集計と先頭 100 件の明細を表示し、明細は 25 件ずつ切り替えます。見積もりだけではダウンロードしません。確認後に「DXF で保存」を押すと、開いた図面を DXF に変換し、見積もりとハッシュ・要素数を比べてから保存します（`convert`、[DXF の読み書き](../docs/formats/dxf.md)）。変換はこのブラウザーの中で行い、図面を送りません。書けない値は診断に種類・件数・場所を出します。DXF を開いたときは、元のファイルと分かるよう `-zumen.dxf` を付けた名前で保存します。入力や用紙を切り替えると、前の見積もりは消えます。

図面上でホイールを回すと、ポインターの位置を中心に拡大・縮小します。中ボタンのドラッグで移動し、中ボタンのダブルクリックで全体を表示します。左右のボタンを同時に押してドラッグすると、右下で指定範囲を拡大、右上で全体表示、左上で縮小、左下で前の表示に戻ります。

要素・レイヤ・診断の一覧は 25 件ずつ表示します。ファイルを選び直すか「閉じる」で、その図面の処理を終了します。読み込みと選択の状態は支援技術にも通知します。

保存の操作は、使っている wasm が `convert_estimate` に対応するときだけ表示します。公開サイトが使うコアのバージョンは「案内」で確認できます。

## 3D のページ

`view3d.html` は IFC・CityGML・点群（LAS など）の、`scene3d` を持つ形式を 3D で表示します（[#818](https://github.com/zumen-jp/zumen/issues/818)）。3D の形式は図面用 wasm にないため、全部入りの wasm（`--no-default-features --features drawing` を付けないビルド）を `ZUMEN_SCENE3D_WASM` に指定してから `npm run build` します。指定がなければ図面用と同じ wasm を使います。

1. 「3D ファイル」でモデルを選びます。要素の一覧のボタンで選び、「選んだ要素」で IFC の属性（`building_elements`）を読めます。
2. 「重ねる点群」で点群を同じ座標に重ね、「地面の図面」で DXF などの 2D の図面を地面に置きます。地図の変換が確かめられないときは通知に理由を出します。
3. 図面にフォーカスを置くと、矢印で回し、`+`・`-` で寄せ引きし、Home で全体を表示します。「幅の寸法を表示」で利用側の追加描画を試せます。
4. IFC では「BCF 3.0 で保存」「BCF 2.1 で保存」で今の視点を指摘にして保存し、「BCF を読む」で視点を戻します。
5. 「表示の資源」で GPU の資源の数を見られます。「閉じる」で、開く前と同じ数に戻ります。

## ブラウザーでの検査

`bindings/js/test/browser` で `npm ci --ignore-scripts` と `npx --no-install playwright install --only-shell chromium` を実行します。上の手順で wasm と `playground` の依存を用意してから、ルートで次を実行します。

```sh
node bindings/js/test/browser/react.mjs chromium target/wasm32-unknown-unknown/release/zumen_abi1.wasm
```

合成した DXF・JWW・SXF と、公開サンプルの DWG（R2000・R2004・R2018）と本番用ビルドで、ファイル選択・用紙・レイヤ・キーボード・出所・診断を操作します。「DXF で保存」は、適合テストの `convert_playground.json` と同じ合成の DWG・JWW・SFC・P21 と公開サンプルの DWG で保存し、保存したバイト列の SHA-256 が、ネイティブの `convert` が書いたもの（`hashes.txt` の blob の行）と、Node.js で同じ wasm が書いたものに一致することを確かめます。見積もりと実変換のハッシュ・要素数・診断、画面の忠実度ごとの件数を照らし、見積もりだけではダウンロードせず、図面の切り替えで前の見積もりが消えることも確かめます。変換の診断の件数と code は、画面と応答で比べます。保存した DXF は開き直してもう一度変換し、同じ数の要素を書き、新しい診断の code が出ないことを確かめます。ネットワークではローカル配布物を取得する GET だけを認めます。本番用 React と開発用 StrictMode の両方で Activity・Suspense の表示を再開し、読み込みを保留した間と新しい Worker の準備後に、利用側の effect が受け取る状態と出所を確かめます。遅い読み込みの差し替え、アンマウント後の Worker・購読と未完了の問い合わせも検査します。`chromium` の代わりに `firefox`・`webkit`、インストール済みの製品版を使う `chrome`・`msedge` も指定できます。

3D のページは、全部入りの wasm で次を実行します。

```sh
node bindings/js/test/browser/react3d.mjs chromium target/wasm32-unknown-unknown/release/zumen_abi1.wasm
```

適合テストの IFC・CityGML・LAS で、描いた画素・要素の一覧・属性・キーボードの視点・寸法の追加描画、点群と地面の図面の重ね、BCF 2.1・3.0 の保存と読み戻しを操作します。3D に対応しない形式の理由、「閉じる」で GPU の資源と Worker が開く前に戻ること、本番用 React と StrictMode での中断・アンマウントの後始末も確かめます。

ResizeObserver の通知中に Canvas の寸法を書き換えず、次のフレームにまとめることも検査します。本番用 React と StrictMode で、詳細パネルが並んだ初回の表示範囲を確かめ、viewport の変更後は 8 フレーム以内に CSS/backing 寸法・描画・通知が収束することを検査します。リサイズ後の移動と全体表示、通知直後のアンマウントで予約したフレーム・observer・入力・Worker・購読を片づけること、再マウント後の後始末も確かめます。未処理エラーの収集は維持し、失敗時だけ直前の操作と寸法をログに残します。

React から組み込む手順は[フックの README](../packages/react/README.md)にあります。赤入れは [#589](https://github.com/zumen-jp/zumen/issues/589) で進めます。
