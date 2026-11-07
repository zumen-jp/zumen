# 図面のプレイグラウンド

DXF・JWW・SXF を手元のブラウザーで開き、描画した要素を元のレコードへたどれます。図面はファイル選択で読み込みます。画像・PDF・フォントの参照名を URL として取得せず、図面や結果も送信しません。

## 起動

リポジトリのルートで次を実行します。Node.js 22.12 以降と、リポジトリで指定した Rust を使います。

```sh
rustup target add wasm32-unknown-unknown
cargo build -p zumen-abi --release --target wasm32-unknown-unknown --locked --no-default-features --features drawing
npm ci --ignore-scripts --prefix playground
npm run build --prefix playground
npm start --prefix playground
```

[図面ビューア](http://127.0.0.1:24188/)を開きます。図面用 wasm は Vite（8.3.5、MIT）がローカルの配布物に含めます。別の場所にビルドした場合は `ZUMEN_DRAWING_WASM` にそのファイルのパスを指定してから `npm run build` します。

CSS は作成したファイルをそのままコピーし、ビルド後のバイト列も検査します。Vite の開発用の依存にある Lightning CSS（MPL-2.0）は、CSS の生成・圧縮や実行時のコードに使いません（[決定 0268](../docs/decisions/0268-distribute-wasm-modules.md)）。ビルドした JS に含めた依存のライセンスは `dist/licenses.md` に出力します。

## 操作

1. 「図面ファイル」で DXF・JWW・SXF（SFC・P21・SFZ・P2Z）を選びます。
2. 「用紙・空間」で表示の起点を選びます。読み込み中も矢印キーで続けて切り替えられます。「全体を表示」と「拡大」「縮小」で表示範囲を調整できます。図面にフォーカスを置くと、矢印で移動し、Home で全体を表示します。
3. シーンの実効レイヤ ID を示すチェックボックスで表示を切り替えます。図形をクリックするか、要素のボタンに Tab で移動して Enter または Space で選びます。
4. 「出所」で要素 ID・配置の道筋と、元のレコードなどを読めます。続けて選ぶと、最後に選んだ要素の出所を表示します。「診断」にはコアの日本語の説明・種類・件数・場所と、表示の代替・未描画が出ます。同じ要素を格子に配置した場合は、配置の ID・列・行で区別できます。

要素・レイヤ・診断の一覧は 25 件ずつ表示します。ファイルを選び直すか「閉じる」で、その図面の処理を終了します。読み込みと選択の状態は支援技術にも通知します。

## ブラウザーでの検査

`bindings/js/test/browser` で `npm ci --ignore-scripts` と `npx --no-install playwright install --only-shell chromium` を実行します。上の手順で wasm と `playground` の依存を用意してから、ルートで次を実行します。

```sh
node bindings/js/test/browser/react.mjs chromium target/wasm32-unknown-unknown/release/zumen_abi1.wasm
```

合成した DXF・JWW・SXF と本番用ビルドで、ファイル選択・用紙・レイヤ・キーボード・出所・診断を操作します。ネットワークではローカル配布物を取得する GET だけを認めます。本番用 React と開発用 StrictMode の両方で Activity・Suspense の表示を再開し、読み込みを保留した間と新しい Worker の準備後に、利用側の effect が受け取る状態と出所を確かめます。遅い読み込みの差し替え、アンマウント後の Worker・購読と未完了の問い合わせも検査します。`chromium` の代わりに `firefox`・`webkit`、インストール済みの製品版を使う `chrome`・`msedge` も指定できます。

React から組み込む手順は[フックの README](../packages/react/README.md)にあります。赤入れは [#589](https://github.com/zumen-jp/zumen/issues/589) で進めます。
