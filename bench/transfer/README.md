# バイナリの受け渡しの測定

[#6](https://github.com/zumen-jp/zumen/issues/6) の試作です。合成した 2D のシーンを、JSON・独自の列指向・FlatBuffers・Arrow IPC で符号化し、Worker からページまでの時間、ページ側の時間、メモリ、wasm の増分を比べます。結果と判断は [決定 0137](../../docs/decisions/0137-columnar-scene-transfer.md) にあります。

核のワークスペースには入れていません。依存（`flatbuffers`・`arrow-*`・`apache-arrow`）は、どれも Apache-2.0 で、この測定のためだけに使います。

## 流し方

```bash
cargo run -q --release --all-features -- out
npm install --no-audit --no-fund --loglevel=error
node run.mjs out
./wasm-size.sh
```

- `encode` は、`out` に形式ごとのファイルを書き、大きさと符号化の時間を出します。2 つめの引数で折れ線の数を変えられます（既定は 22.7 万本）。
- `run.mjs` は、形式ごとに別のプロセスで 7 回測り、中央値を出します。Worker（`worker.mjs`）はファイルを wasm のメモリの代わりに読んでおき、ページ（`page.mjs`）が型付き配列を得るまでを測ります。
- ブラウザーでは、`node serve.mjs out` のあと `http://localhost:24180/browser.html` を開きます。
- `wasm-size.sh` は、形式ごとに wasm を作り、何も符号化しないものとの差を出します。

測定用の図面（旧版の決定 0016 の SFC など）は、Git に入れません。
