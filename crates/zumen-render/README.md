# シーンからの SVG

`zumen_render::svg` は、ZMSC のバイト列と `Options`・`Cx` を受け取り、SVG 1.1 全体のバイト列を返します。領域の API への接続は [#603](https://github.com/zumen-jp/zumen/issues/603)、PNG は [#604](https://github.com/zumen-jp/zumen/issues/604) で追跡しています。

`Options` の `region` はシーンの起点と同じ座標系です。`width`・`height` はピクセル、`dpi` は用紙 mm の線幅の換算にだけ使います。背景・前景は RGBA で明示します。座標の単位を mm と推測しません。

文字はシーンの字形の輪郭を使います。字形のない空でない文字・外部画像・名前で指定された点記号は描かず、要素 ID と描く単位の番号を診断に残します。診断の `unit` からシーンの `unit_entities` と配置の道筋をたどれます。入力の参照名を取得したり、XML に埋め込んだりしません。

出力の容量は 4096 バイトから倍に増やし、増やす前に `decoded_bytes` で数えます。診断は 1 回につき固定の 1024 バイトと ID の長さを数えます。予算の数え方は native/wasm で同じです。出力の容量は wasm32 に合わせて `i32::MAX` 以下にし、途中の結果は返しません。

## 確かめる

```sh
cargo test -p zumen-render
cargo clippy -p zumen-render --all-targets -- -D warnings
cargo clippy -p zumen-render --target wasm32-unknown-unknown -- -D warnings
cargo run -p zumen-render --example export -- target/svg-check
cargo build -p zumen-render --example parity --target wasm32-unknown-unknown
node crates/zumen-render/tests/wasm.mjs target/wasm32-unknown-unknown/debug/examples/parity.wasm target/svg-check/transcript.bin
```

`export` は合成シーン・SVG と、成功・不正な入力・予算超過・中断の記録を作ります。`parity` は同じ記録を wasm で作るテスト用のモジュールです。製品の ABI ではありません。

外部との照合は既存のブラウザー検査の依存を使います。インストール済みなら、最後のコマンドだけで実行できます。

```sh
npm ci --ignore-scripts --prefix bindings/js/test/browser
npm exec --prefix bindings/js/test/browser -- playwright install --only-shell chromium
node crates/zumen-render/tests/browser.mjs target/svg-check/drawing.svg
```

CI でも同じ方法で照合します。`fuzz` の `svg` target は ZMSC を入力にし、SVG を 2 回作って出力・診断・停止・予算が一致することを確かめます。`cargo xtask fuzz-seeds` が、線と列の参照を持つ合成シーンを種に足します。

仕様・対応の範囲は [SVG の文書](../../docs/formats/svg.md)、判断は[決定 0272](../../docs/decisions/0272-scene-svg-rendering.md)にあります。
