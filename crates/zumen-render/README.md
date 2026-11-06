# シーンからの SVG・PNG

`zumen_render::svg` は、ZMSC のバイト列と `Options`・`Cx` を受け取り、SVG 1.1 全体のバイト列を返します。各言語の API からは [render op](../../docs/render.md) を使えます。`zumen_render::png` は同じ引数から PNG のバイト列を返します（決定 0277）。

`Options` の `region` はシーンの起点と同じ座標系です。縦横同率で中央に収め、その範囲の外を切ります。縦横比の違いによる余白にも背景を塗ります。`width`・`height` はピクセル、`dpi` は用紙 mm の線幅の換算にだけ使います。背景・前景は RGBA で明示します。座標の単位を mm と推測しません。

文字はシーンの字形の輪郭を使います。字形のない空でない文字・外部画像・名前で指定された点記号は描かず、要素 ID と描く単位の番号を診断に残します。診断の `unit` からシーンの `unit_entities` と配置の道筋をたどれます。入力の参照名を取得したり、XML に埋め込んだりしません。

種類ごとの列で unit の番号が前後しても、塊・unit・種類・元の行の順に描きます。シーンの列と ID は変えません。塊ごとの順序索引は、確保前に要素数を `elements`、1 要素につき 12 バイトを `decoded_bytes` で数えます。索引の並べ替えは追加の確保や再帰を使わず、比較の順と `work` の数え方を native/wasm でそろえます。

SVG の出力の容量は 4096 バイトから倍に増やし、増やす前に `decoded_bytes` で数えます。診断は 1 回につき固定の 1024 バイトと ID の長さを数えます。出力と索引それぞれの容量は wasm32 に合わせて `i32::MAX` 以下にし、途中の結果は返しません。

PNG は最大 16,777,216 画素です。RGBA の面、2 行分のサンプルマスク、PNG の完成時の容量を確保前に数えます。辺・交点・クリップのベクターも容量を増やす前に数えます。固定した 4×4 サンプルの走査・並べ替え・合成・符号化のループで `work` と中断を確かめます。PNG は非圧縮 DEFLATE を使い、圧縮器の内部の確保や処理を持ちません。

## 確かめる

```sh
cargo test -p zumen-render
cargo clippy -p zumen-render --all-targets -- -D warnings
cargo clippy -p zumen-render --target wasm32-unknown-unknown -- -D warnings
cargo run -p zumen-render --example export -- target/svg-check
cargo build -p zumen-render --example parity --target wasm32-unknown-unknown
node crates/zumen-render/tests/wasm.mjs target/wasm32-unknown-unknown/debug/examples/parity.wasm target/svg-check/transcript.bin
```

`export` は合成シーン・SVG・PNG・RGBA と、成功・不正な入力・予算超過・中断の記録を作ります。`parity` は同じ記録を wasm で作るテスト用のモジュールです。製品の ABI ではありません。

製品の wasm で合成 DXF の三角形・面積 0 の輪・多数の縦線を描き、図形から求めた全画素を照らすには、次を実行します。

```sh
cargo build -p zumen-abi --target wasm32-unknown-unknown --release --locked
node crates/zumen-render/tests/api.mjs target/wasm32-unknown-unknown/release/zumen_abi1.wasm
```

外部との照合は既存のブラウザー検査の依存を使います。インストール済みなら、最後のコマンドだけで実行できます。

```sh
npm ci --ignore-scripts --prefix bindings/js/test/browser
npm exec --prefix bindings/js/test/browser -- playwright install --only-shell chromium
node crates/zumen-render/tests/browser.mjs target/svg-check/drawing.svg target/svg-check/region.svg
```

PR の CI と、既存の `nightly.yml` の読み取り専用の `svg` job で同じ手順を使います。毎晩と手動実行でも SVG・PNG を生成し、Chromium で照合します。PNG のチャンクの CRC と zlib の Adler-32・復号後の RGBA も Node.js の zlib で照らします。検査には Node.js 22.2 以降を使います。

`fuzz` の `svg`・`png` target は ZMSC を入力にし、それぞれの画像を 2 回作って出力・診断・停止・予算が一致することを確かめます。`cargo xtask fuzz-seeds` が、線と列の参照を持つ合成シーンを種に足します。

PNG の仕様と外部照合は [PNG の文書](../../docs/formats/png.md)にあります。SVG の仕様・対応の範囲は [SVG の文書](../../docs/formats/svg.md)、判断は[決定 0272](../../docs/decisions/0272-scene-svg-rendering.md)にあります。
