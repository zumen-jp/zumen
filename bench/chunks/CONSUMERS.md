# シーンを使う側の測定

[#219](https://github.com/zumen-jp/zumen/issues/219) の追加測定です。[生成側](README.md)と同じ公開 3 入力・合成 2 入力を使います。表示部品の現行の経路を呼び、描画・範囲選択・転送・メモリを分けて記録します。結果は [CONSUMER-RESULTS.md](CONSUMER-RESULTS.md) にあります。値を決めるための参考で、仮値の変更や issue 全体の完了ではありません。

## 再現する手順

リポジトリで固定した Rust と Node.js 22 以降を使います。ブラウザー用の依存は既存の lockfile に従います。新しい依存はありません。

```powershell
npm ci --prefix bindings/js/test/browser --ignore-scripts
# この環境でまだ入れていない場合だけ、既存の検査と同じ Chromium を入れる。
npm exec --prefix bindings/js/test/browser -- playwright install chromium
cargo run --manifest-path bench/chunks/Cargo.toml --bin measure --release --locked -- bench/chunks/out 7
cargo build --manifest-path bench/chunks/Cargo.toml --lib --target wasm32-unknown-unknown --release --locked
node bench/chunks/run.mjs bench/chunks/out bench/chunks/target/wasm32-unknown-unknown/release/zumen_bench_chunks.wasm 7
cargo build -p zumen-abi --target wasm32-unknown-unknown --release --locked --no-default-features --features drawing
node bench/chunks/consumer.mjs bench/chunks/out target/wasm32-unknown-unknown/release/zumen_abi1.wasm 7
node bench/chunks/summarize-consumer.mjs bench/chunks/out/consumer-results.json
```

3〜99 の奇数を指定でき、生成側と同じ回数でなければ停止します。各項目を 1 回ウォームアップし、指定回数の中央値を取ります。既存の Worker は各回新しく作り、ブラウザーの Worker 往復は起動済みのものを使います。結果は `out/consumer-results.json` です。最後のコマンドはその JSON を日付付きの結果と Markdown に写すため、記録を更新するときに使います。

専用の headless Chromium を起動し、各設定に新しい browser context を作ります。個人のプロファイルや受け取った図面は使いません。ローカルサーバーは `127.0.0.1:24192` にだけ bind し、固定したモジュールと 9 個の生成済みシーンだけを返します。外の origin への要求は遮断します。図面を外へ送りません。

## 入力と比較設定

公開入力は仮値だけを測ります。合成 10k は仮値と件数 1024・16384、合成 100k は仮値と範囲 16384・262144 倍を測り、計 9 組に絞ります。入力・シーンのサイズ、要素数・塊数、SHA-256 は生成側の結果と照合します。入力の生成物も `out/` だけに保存します。

表示には `readScene()` と `View2D` を使います。Rust の監査済み件数と列から、表示部品に渡す `result`・`document` のラッパーを bench 内で作ります。単位は `unknown`、文書ハッシュはシーンの SHA-256 を表す測定用の値です。この部分は API の応答の測定ではありません。描画と選択は既存の表示部品そのものです。空間、許容差 0.01、フォントなしのシーンを、960 × 640 CSS px、pixel ratio 1、回転 0、余白 32 px で収めます。文字にはブラウザーの代替フォントを使い、報告する代替・未描画の理由と件数を保存します。

合成 100k は色の未解決を報告する 10 万件が、既定の `issues` 上限 1 万件に当たります。その拒否を記録したうえで、計測時だけ公開オプションの `issues` を 10 万件にします。ほかの上限は既定のままです。既定の上限で表示できたとは扱いません。

## 時間の境界

| キー | 始まりと終わり・含むもの |
| --- | --- |
| `reader_ms` | 所有済みの整列した ArrayBuffer を `readScene()` に渡して返るまで。通信と準備のコピーを除く |
| `set_scene_ms` | 同じカメラで `setScene()` を呼んで返るまで。検査・索引・報告・最初の描画を含む |
| `render_submit_ms` | 準備済みの表示部品で `render()` を呼んで返るまで。JavaScript と Canvas の呼び出しを含む CPU 側の費用 |
| `render_to_next_raf_ms` | rAF の中で描画を呼ぶ直前から、次の rAF のコールバックまで。表示周期とスケジューリングを含む |
| `render_readback_ms` | `render()` と全画素の `getImageData()` が返るまで。描画と同期した CPU 側への読み戻しを含む |
| `select_rect_ms` | 中央の CSS px の四角（240・160）〜（720・480）を `hitTestRect()` に渡して返るまで |
| `transfer.post_message_ms`・`clone.post_message_ms` | 主スレッドの `postMessage()` が返るまで。受信や Worker の処理を待たない |
| `transfer.roundtrip_ms`・`clone.roundtrip_ms` | 起動済み Worker へ送る直前から、同じバッファーの返信の受信まで。往復のスケジューリングを含む |
| `job_to_exit_ms` | Node.js の既存 `createSceneJob()` を呼ぶ直前から、`result` が成功するまで。Worker の起動・入力転送・wasm の初期化・読み・シーン生成・`take` のコピー・返信転送・終了を含む |

rAF は表示完了の通知ではありません。Canvas の呼び出しが戻る時刻と、GPU の完了を分けて測れる API は使っていません。読み戻しも GPU の単独の処理時間ではなく、ソフトウェア描画ならその費用を含みます。headless の GPU 情報を保存します。小さい値はブラウザーの時計の分解能に制限されます。

生成済み blob の Worker 往復は、標準の structured clone と転送リストを使う測定専用の echo です。既存の scene job の内訳を測ったものではありません。転送は両方向に所有権を移し、clone は両方向でコピーします。専有バッファーを用意する `slice` は時計の外です。送信元の detach と返信の SHA-256・バイト数を検査します。`post_message_ms` と往復時間の差を、片道のコピー時間とはみなしません。

既存の scene job には塊や許容差を追加指定せず、既定のまま使います。同じ公開 API を主スレッドで呼んだ結果と、返信の全バイト・応答・件数・Worker の終了を各回比較します。既定の許容差のため、生成側の許容差 0.01 の blob と同じとは限りません。wasm モジュールのコンパイルと送る入力の専有コピーは時間の外です。

## 正しさとメモリ

生成側のネイティブ・wasm の全バイト一致を通した blob を使います。ブラウザーでは SHA-256・件数・描いたものと未描画の件数を確認します。同じ表示を繰り返した画素の SHA-256 も比べます。ブラウザーや環境をまたぐ画素の一致は保証しません。

範囲選択は ID と配置の道筋をキーにし、繰り返し・選択状態を検査します。設定間の集合も比較します。合成入力では、幅 1 CSS px・butt の線の四角と問い合わせの交差を独立した分離軸の判定で調べます。公開された f32 の列から戻した端点を期待値に使い、元の f64 の生成式による集合との差も別に記録します。丸めによる境界の差を、選択の誤りと混ぜません。全面の問い合わせは全線分を期待します。不一致は `exact: false` と不足・余分の件数に残します。ハーネスが終わったことを選択の正しさの成功とは扱いません。最初に不足した線の列だけでシーンを作り、線の中点・Canvas の stroke・点選択・範囲選択も記録します。

| 記録 | 意味 |
| --- | --- |
| `scene_bytes`・`column_bytes`・`header_and_padding_bytes` | blob と各列の確定バイト数。整列した入力を reader が共有し、列をコピーしていないことを確認 |
| `calculated_order_index_bytes` | 描くものの数 × 4。既存の `prepare()` が保持する Uint32Array の順序索引の計算値。JS のオブジェクトや一時配列を含まない |
| `readback_rgba_bytes` | 読み戻す RGBA 配列の 960 × 640 × 4 バイト。Canvas・GPU 全体の確保量ではない |
| `selection_json_utf8_bytes` | 比較に使った選択キーを JSON にした UTF-8 のバイト数。選択オブジェクトのメモリ使用量ではない |
| `retained_heap_observations` | CDP で GC を要求したあとの `Runtime.getHeapUsage`。読み込み前・`setScene` 後・計測後・破棄後を記録。V8 と embedder・backing storage の観測値で、最大や純増の保証ではない |
| `memory_before`・`memory_after` | Node.js プロセスの `process.memoryUsage()`。Worker の実行前と終了後で、Worker 内の最大使用量は含まない |

最大メモリ、GPU の確保量、wasm の最大ページ数は測っていません。GC やドライバー、他プロセス、フォントとブラウザーの版によって観測値は変わります。今回の測定は、大規模な実物、用紙・ビュー、既定の許容差・フォントの比較を代えるものではありません。
