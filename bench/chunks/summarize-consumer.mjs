// 保存した JSON から表を作る。時間・件数を手で転記しない。
import { readFile, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const raw = await readFile(resolve(process.argv[2] ?? resolve(here, "out/consumer-results.json")), "utf8");
const r = JSON.parse(raw);
const date = new Date(Date.parse(r.measured_at) + 9 * 3600_000).toISOString().slice(0, 10);
const snapshot = `consumer-results-${date}.json`;
const ms = v => v.toFixed(3), mib = v => (v / 1048576).toFixed(2);
const table = (headers, rows) => ["| " + headers.join(" | ") + " |", "| " + headers.map(() => "---").join(" | ") + " |", ...rows.map(row => "| " + row.join(" | ") + " |")].join("\n");
const text = `# シーンを使う側の測定結果

${date} JST の 1 台の結果です。[測定手順と境界](CONSUMERS.md)、[機械可読の記録](${snapshot})を参照してください。各項目は 1 回ウォームアップ後、${r.repeats} 回の中央値です。基準のソースは \`${r.source_commit}\` で、計測中のハーネスの SHA-256 も JSON にあります。

${r.environment.cpu.trim()}、${r.environment.logical_cpus} 論理 CPU、${r.environment.os} ${r.environment.release} ${r.environment.arch}、${r.environment.rustc}、Node.js ${r.environment.node}、Playwright ${r.environment.playwright}、headless Chromium ${r.environment.chromium} です。画面は 960 × 640 CSS px、pixel ratio 1 です。GPU・ドライバーの情報は JSON を参照してください。ほかの環境への速さの保証ではありません。

## 正しさの結果

生成側のネイティブ・wasm の全バイトは ${r.validation.producer_native_wasm_exact_pairs} 組で一致しました。既存 Node.js Worker の ${r.validation.node_worker_exact_cases} 入力も、主スレッドの公開 API の応答・シーン全バイトと各回一致しました。ブラウザーではシーンの SHA-256、件数、描いた数と未描画の数、繰り返した画素、転送した全バイトと所有権を検査しました。

**合成入力の範囲選択には不一致があります。** 表の期待数は公開 f32 の列から独立に計算した交差集合です。選択を確認済みとはしません。元の f64 の式との差と、列 1 本だけで再現する点・Canvas の判定も JSON に記録しています。表示部品の修正は [#806](https://github.com/zumen-jp/zumen/issues/806) で追跡し、測定の PR には含めません。この状態で塊の仮値を決めません。

${table(["入力 / 設定", "期待", "実際", "不足", "余分", "全面の実際 / 期待", "f64 式との集合差"], r.browser_results.filter(x => x.independent_selection).map(x => { const q = x.independent_selection; return [x.case + " / " + x.policy, q.expected_count, x.selected_count, q.missing_count, q.actual_only_count, x.full_selected_count + " / " + q.full_expected_count, q.formula_vs_encoded_difference]; }))}

## 描画・選択の中央値

単位は ms です。\`setScene\` は検査と索引作成・描画を含み、\`render\` は準備済みの CPU 側の呼び出しです。rAF と読み戻しは GPU の単独の完了時間ではありません。小さい値は時計の分解能に制限されます。選択の値は不一致が残る現行実装の費用です。

${table(["入力 / 設定", "reader", "setScene", "render", "次 rAF", "描画と読み戻し", "範囲選択"], r.browser_results.map(x => [x.case + " / " + x.policy, ms(x.reader_ms), ms(x.set_scene_ms), ms(x.render_submit_ms), ms(x.render_to_next_raf_ms), ms(x.render_readback_ms), ms(x.select_rect_ms)]))}

合成 100k は既定の \`issues\` 上限で拒否されます。計測時だけ上限を 10 万件にした結果です。未解決の色は代替色で描き、報告の件数もメモリに含まれます。文字を含む公開入力にはブラウザーの代替フォントを使います。

## 転送の中央値

ブラウザーは起動済みの計測用 echo Worker との往復です。転送リストは両方向の所有権移動、clone は両方向のコピーを使います。専有入力の準備は時間の外です。送信呼び出しと往復は、コピー・スケジューリング・受信を別々に取り出した値ではありません。

${table(["入力 / 設定", "blob byte", "転送 post ms", "転送往復 ms", "clone post ms", "clone 往復 ms"], r.browser_results.map(x => [x.case + " / " + x.policy, x.scene_bytes, ms(x.transfer.post_message_ms), ms(x.transfer.roundtrip_ms), ms(x.clone.post_message_ms), ms(x.clone.roundtrip_ms)]))}

Node.js の既存 \`createSceneJob\` は起動から終了までです。読み・wasm の初期化・シーン生成・\`take\` のコピーを含み、既定の許容差を使うので上の blob と分けます。

${table(["入力", "入力 byte", "blob byte", "塊", "許容差", "起動〜終了 ms"], r.node_worker.map(x => [x.case, x.input_bytes, x.scene_bytes, x.counts.chunks, x.tolerance, ms(x.job_to_exit_ms)]))}

## バイト数と保持量の観測

blob と列、順序索引の計算値は確定するバイト数です。reader は今回の整列した blob の列を共有し、列のコピーは 0 byte でした。読み戻す RGBA 配列は 2457600 byte です。GC 後の V8 heap の増分は観測値で、一時的な最大使用量や Canvas・GPU 全体の確保量ではありません。破棄後・embedder・backing storage・Node.js の観測も JSON にあります。

${table(["入力 / 設定", "blob byte", "列 byte", "順序索引 byte", "setScene 後の used heap 増分 MiB"], r.browser_results.map(x => [x.case + " / " + x.policy, x.scene_bytes, x.column_bytes, x.calculated_order_index_bytes, mib(x.retained_heap_observations.after_setup.usedSize - x.retained_heap_observations.before.usedSize)]))}

大規模な実物、用紙・ビュー、フォントを渡す場合、既定の許容差との比較、GPU 完了、最大メモリは未確認です。公開入力は小さく、合成入力は実物の代わりには数えません。残る確認と塊の値の判断は #219 に残します。
`;
await writeFile(resolve(here, snapshot), raw);
await writeFile(resolve(here, "CONSUMER-RESULTS.md"), text);
console.log(JSON.stringify({ snapshot, rows: r.browser_results.length, validation: r.validation }));
