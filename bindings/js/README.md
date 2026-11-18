# TypeScript・JavaScript のラッパー

wasm のモジュール（`zumen_abi1.wasm`）をブラウザーと Node.js（22 以降）から呼び、Node.js ではネイティブの CLI を子プロセスとしても使えるラッパーです（[依存とツールの条件](../../AGENTS.md#範囲と権利)）。

- npm の `@zumen-jp/zumen` で配ります（[コアの設計](../../docs/architecture.md)・[配布の手順](../../xtask/release/README.md)）。パッケージには全体版と 4 つの分割版の wasm を入れます。依存のライセンスの文は `THIRD_PARTY_LICENSES` にあります。
- 素の ESM の JavaScript で書き、ビルドも npm の依存もありません。型は `.d.ts` にあります。
- 各 wasm の依存一覧（CycloneDX SBOM）は `sboms/{wasm のファイル名}.cdx.json` にあります。
- `src/messages.d.ts`・`src/constants.js`・`src/constants.d.ts` は、`schema/` から `cargo xtask codegen` で生成します。手で直さないでください。

## 使い方

サーバーで SVG・PNG を作る `render` op の設定と実行例は、[図面から SVG・PNG を作る](../../docs/render.md)にあります。

Node.js では、`@zumen-jp/zumen/node` の `loadBundled` が、パッケージに入れた wasm を読み込みます。コンパイルは 1 度だけで、呼ぶたびに新しいインスタンスを返します。ブラウザーでは、`@zumen-jp/zumen` の `load` に、wasm の fetch の応答（`load(fetch("/zumen_abi1.wasm"))`）・バイト列・コンパイルしたモジュールを渡します。

```js
import { readFileSync } from "node:fs";
import { ZumenError, loadBundled, readScene } from "@zumen-jp/zumen/node";

const zumen = await loadBundled();
const session = zumen.open();
try {
  session.push("a.dxf", readFileSync("a.dxf"));
  const { result } = session.call({ v: 1, op: "open", input: "a.dxf" });
  const scene = session.call({ v: 1, op: "scene", doc: result.doc, from: "space/0" });
  const columns = readScene(session.take(scene.result.blob)).columns;
  console.log(columns.chunk_origins, columns.line_xy);
} catch (error) {
  if (error instanceof ZumenError) {
    // failed の応答は error.response にあり、診断を持ちます
    console.log(error.status, error.response?.diagnostics);
  }
} finally {
  session.close();
}
```

| 名前 | 中身 |
| --- | --- |
| `load`・`loadSync` | モジュールを読み込み、ABI のメジャーバージョンを確かめる。別の realm（Next.js の Edge のランタイムなど）で作ったモジュールやバイト列も受け取る |
| `loadBundled` | パッケージに入れた wasm を読み込む。`@zumen-jp/zumen/node` にあり、Node.js だけで使える |
| `Zumen.open` | セッションを作る（`zm_session_new`） |
| `Zumen.broken` | wasm の中で trap が起き、もう使えないか |
| `Session.push`・`call`・`callBytes`・`take`・`cancel`・`close` | API の関数（`zm_push`・`zm_call`・`zm_take`・`zm_cancel`・`zm_session_close`）。状態が `ok` でなければ `ZumenError` を投げる |
| `readScene` | シーンの blob を、セクションごとの型付き配列で読む（`schema/scene.toml`） |

### Node.js の子プロセス

`@zumen-jp/zumen/node` の `openProcess` に、別に導入した [CLI](../cli/README.md) の実行ファイルの道筋を渡します。セッションごとに `serve --stdio` で起動し、閉じるまで同じプロセスに文書を保ちます。デスクトップでは 64 bit の CLI を使います。Node.js のメソッドは非同期で、処理中もイベントループは動きます（#358）。

```js
import { openProcess } from "@zumen-jp/zumen/node";

const session = await openProcess("/path/to/zumen", { v: 1 }, { timeoutMs: 60_000 });
try {
  await session.push("input.dxf", bytes);
  const opened = await session.call({ v: 1, op: "open", input: "input.dxf", budget: { input_bytes: 50_000_000 } });
  const response = await session.call({ v: 1, op: "drawing", doc: opened.result.doc });
  console.log(response.result);
} finally {
  await session.close();
}
```

- `push`・`call`・`callBytes`・`take`・`cancel`・`close` は Promise を返します。コアの応答と blob は、そのままのバイト列で返します。返したバッファーは Node.js の共有プールと分けて持ち、転送できます。
- `push`・`call`・`take` は、一度に 1 つだけ呼べます。先の呼び出しや中断の応答を待つ間の重複は、`busy` の `ZumenError` になります。コアが返すほかの失敗も、wasm と同じ `ZumenError` です。
- 処理中の `call` は、別のイベントやタイマーから `await session.cancel()` で中断できます。中断しても 1 秒以内に応答しない場合は子プロセスを終了します。`close` は処理中も呼べ、残る呼び出しを失敗にして、プロセスの終了まで待ちます。
- 起動・通信の失敗や上限の超過は `ProcessError` です。`code` は `spawn`・`io`・`protocol`・`closed`・`timeout`・`limit` です。`broken` になったセッションは、閉じてから開き直します。送信前のサイズ検査で止まった場合は、そのセッションを引き続き使えます。
- 第 3 引数の `timeoutMs` は 1 つの応答を待つ上限で、既定は 30 秒です。`maxMessageBytes` は送受信する JSON-RPC の 1 行の上限で、既定は 64 MiB、最大は 1 GiB です。base64 の分だけ送信する量が増えるため、大きい入力は `push` に分けて渡します。期限や受信上限で止めた場合は、プロセスを終了します。
- 第 2 引数はセッションの設定で、値・JSON の文字列・バイト列を CLI にそのまま渡します。通常は省略するか `engine: "native"` にします。`engine: "process"` を渡すと、その CLI がさらに子プロセスを起こします。
- この入口は CLI を直接起動し、OS のメモリ上限は設定しません。コアの作業量と確保量は要求の `budget` で制限します。OS による制限が必要な場合は、CLI の起動環境で設定します。

全適合ケースと、子プロセスの終了・通信の失敗・中断・同時呼び出しを検査できます。

```sh
cargo build -p zumen-cli
node bindings/js/test/process.mjs target/debug/zumen
node bindings/js/test/process-conformance.mjs target/debug/zumen
```

Windows の実行ファイルは `target/debug/zumen.exe` です。

### Node.js の常駐文書

`openLiveProcess` は `live --stdio` で文書エンジンを保持し、64 要素ブロックの表示差分と移動をバイナリで往復させます。DXF・DWG・JWW・SXF の入口です。JWW・SXF の表示・選択・移動の座標は用紙の座標です。コアの成否は `status`、表示生成の成否は `displayStatus` に分けて返します。

```js
import { openLiveProcess } from "@zumen-jp/zumen/node";

const live = await openLiveProcess("/path/to/zumen");
try {
  const opened = await live.open(bytes, { threads: 4 });
  if (opened.status === "ok" && opened.displayStatus === "ok") {
    // 表示の列を手元へ適用してから、受領した版を確認する。
    applyDisplay(opened);
    live.acknowledge(opened.revision);
    const changed = await live.transform({
      revision: opened.revision, rows: [0], matrix: [1, 0, 5, 0, 1, -2],
    });
    if (changed.status === "ok" && changed.displayStatus === "ok") applyDisplay(changed);
    if (changed.diagnosticCount) {
      const diagnostics = await live.diagnostics(changed.requestId);
      console.log(new TextDecoder().decode(diagnostics.data));
    }
  }
} finally { await live.close(); }
```

`applyDisplay` はアプリ側の処理です。全列の長さと `column.blocks()` の塊を反映します。差分では手元の表示の版と `reply.base` が一致することを確かめ、違う場合は `live.display({ full: true })` を取得します。バイト列は応答を借りるため、使っている間は書き換えません。`transform`・`target`・`undo`・`redo` には対象の版を明示します。

`live.select([minX, minY, maxX, maxY], revision)` は、モデル空間の囲む箱と矩形が交差する生存行を取得します。成功時は `reply.selection.length` で件数を、`reply.selection.rows()` で昇順の行を反復できます。選択した行を編集するときは、この応答の `revision` を渡します。空間索引と応答へのコピーは要求の予算を共有し、停止時は部分結果を返しません。

`live.layers(revision)` は、表示のスタイル `[画層, 色]` の画層の値と、その画層の名前・状態・RGB・色番号を、図面モデルの画層と同じ順で `reply.layers` に返します。画層の値は、DXF・DWG では画層の表の位置、JWW・SXF では画層の ID です。

診断は次の通常要求の前に、明示した予算で別途取得します。表示だけが停止したときの扱い、通信上限、中断・終了は[文書の IPC](../cli/live.md)にあります。残りの形式、描画順・画層・文字・記号の定義、移動以外の編集、Electron の表示への組み込みと性能の検査は[文書エンジンの残件](../../docs/live.md)です。

```sh
node bindings/js/test/live.mjs target/debug/zumen
```

### trap のあと

wasm の中のパニックとメモリ不足は trap になり、そのインスタンスはもう使えません（[予算と中断](../../docs/architecture.md#予算と中断)）。ラッパーは、trap が起きた呼び出しと、それからの呼び出しを、状態が `panicked` の `ZumenError` にし、元の例外（`WebAssembly.RuntimeError` など）を `cause` に入れます。`Zumen.broken` が `true` になったら、モジュールを読み込み直します。`loadModules` の `Modules` は、使えなくなったインスタンスを捨て、次の `get`・`open`・`forOp` で読み込み直します。読み込みに失敗したときも覚えず、次に引いたときにやり直します。`Session.close` と `cancel` は、使えなくなったインスタンスでは何もせず、`finally` の中で呼んでも先に出た例外を隠しません。

### Next.js などのサーバー

- Node.js のランタイムで使います。Next.js の文書は、Edge のランタイムを Proxy で使うものとしています。Vercel では、Next.js 16.3 から route と page の `runtime = 'edge'` が使えず、Node.js で動きます。
- Turbopack（Next.js 16 の既定）で組むときは、設定は要りません。webpack（`next build --webpack`）で組むときは、`next.config` の `serverExternalPackages` に `"@zumen-jp/zumen"` を入れます。webpack はパッケージの wasm を、ブラウザー向けの URL に書き換えるためです。どちらも、`output: "standalone"` で組んだ出力に wasm が入ります。
- 要求ごとに `loadBundled` で新しいインスタンスを使います。壊れたファイルで起きた trap がほかの要求に及ばず、wasm のメモリも要求が終われば回収されます。wasm のメモリは縮まないので、1 つのインスタンスを使い回すと、いちばん大きな入力を読んだときの大きさのままになります。
- 受け取ったファイルを読むときは、要求の `budget` で量に上限を付けます（[予算と中断](../../docs/architecture.md#予算と中断)）。
- 呼び出しは同期で、動いている間は Node.js のイベントループが止まります。大きな図面を受けるサーバーでは、`worker_threads` で動かします。
- Vercel の関数は、要求と応答の本文が 4.5 MB までです。大きな図面は、ブラウザーからストレージに直接上げ、関数はそこから読みます。

```ts
// app/api/drawing/route.ts
import { ZumenError, loadBundled } from "@zumen-jp/zumen/node";

export async function POST(request: Request) {
  const bytes = new Uint8Array(await request.arrayBuffer());
  const zumen = await loadBundled();
  const session = zumen.open();
  try {
    session.push("input", bytes);
    const opened = session.call({ v: 1, op: "open", input: "input", budget: { input_bytes: 100_000_000 } });
    const drawing = session.call({ v: 1, op: "drawing", doc: opened.result!.doc });
    return Response.json(drawing.result);
  } catch (error) {
    if (error instanceof ZumenError && error.status === "failed") {
      return Response.json(error.response?.diagnostics, { status: 422 });
    }
    throw error;
  } finally {
    session.close();
  }
}
```

### Node.js の worker で大きな図面を読む

#532 では、同期 API を保ち、アプリ側で `worker_threads` を使う手順を採りました。非同期のラッパーを新設すると、各メソッドの Promise に加え、セッションの寿命・同時実行の上限・中断の扱いも API として決める必要があります。これらはサーバーの構成で変わるため、まず [worker の実行例](examples/worker/main.mjs)をアプリに写して使います。例の関数は npm の公開 API ではありません。

1. `examples/worker` の `main.mjs`・`job.mjs`・`worker.mjs` を、`@zumen-jp/zumen` を入れたアプリへ写す。
2. 主スレッドで wasm を一度コンパイルし、`WebAssembly.Module` を使い回す。worker にはモジュールを structured clone で渡し、インスタンスとセッションは worker の中で作る。
3. 入力は専有する `ArrayBuffer` にし、転送リストに入れる。転送後は、主スレッドでそのバッファーを使わない。`request.arrayBuffer()` の結果はそのまま使える。`readFile` の `Buffer` は `Uint8Array.from(buffer).buffer` で写す。
4. worker の中で `push`・`open`・`scene`・`take` を呼び、`take` が返した blob のバッファーを親へ転送する。JSON の応答も一緒に返す。最後にセッションと通信ポートを閉じ、worker の終了を待つ。

`WebAssembly.Module` は転送リストに入れません。`Session` や wasm の memory も渡しません。転送は、そのバッファーを使うすべてのビューを無効にします。Node.js の `Buffer` の共有プールをそのまま転送すると、必要な範囲だけを渡せるとは限りません（[Node.js の転送の説明](https://nodejs.org/download/release/latest-v22.x/docs/api/worker_threads.html#considerations-when-transferring-typedarrays-and-buffers)）。

```js
import { readFile } from "node:fs/promises";
import { createSceneJob } from "./job.mjs";

// サーバーの要求の外で一度だけコンパイルする
const module = await WebAssembly.compile(await readFile(new URL(import.meta.resolve("@zumen-jp/zumen/zumen_abi1.wasm"))));

// 要求の中。受信する段階でも、入力の大きさに上限を付ける
const input = await request.arrayBuffer();
const { result } = createSceneJob(module, input, {
  budget: { input_bytes: 64 * 1024 * 1024, decoded_bytes: 512 * 1024 * 1024 },
  signal: AbortSignal.any([request.signal, AbortSignal.timeout(30_000)]),
});
const { response, blob } = await result;
```

- この例は、要求ごとに worker を作り、`result` は worker が終了してから成功か失敗になります。サーバーでは、実行中の worker 数と待ち行列の長さに上限を付けます。頻繁な小さい要求では、worker のプールで起動の費用を減らせます。その場合も、インスタンスは要求ごとに作り直します。
- wasm が同期処理をしている間は、その worker のイベントループも止まります。`cancel` を呼ぶメッセージを送るだけでは処理中に届かないため、この例は AbortSignal で worker 自体を `terminate()` します。中断した要求の入力は転送済みなので、再試行には入力を再び用意します。
- API の失敗は、状態・操作・診断を持つ Error として返します。スレッドを越えるため、`instanceof ZumenError` では判断せず、`error.status` と `error.response` を見ます。worker の異常終了や転送の失敗も Promise の失敗になり、worker を残しません。
- worker は同じプロセスの中で動きます。[`resourceLimits` の対象](https://nodejs.org/download/release/latest-v22.x/docs/api/worker_threads.html#new-workerfilename-options)は JavaScript のエンジンで、ArrayBuffer や wasm のメモリの上限にはなりません。入力の大きさと API の `budget`、同時実行数で量を制限します。ネイティブの CLI を別のプロセスとして動かす場合は、[子プロセスの入口](#nodejs-の子プロセス)を使えます。OS のメモリ制限は、別に設定します。
- この例は素の Node.js で確かめています。Next.js などで組むときは、worker のファイルと、そのファイルから解決できる npm パッケージをサーバーの出力に含めます。上の同期の API ルートの例に対する bundler のテストは、worker の配布までを確認するものではありません。

リポジトリでは、作った wasm のパスを渡して実行できます。

```bash
node bindings/js/examples/worker/main.mjs samples/dxf/text_alignments.dxf target/wasm32-unknown-unknown/release/zumen_abi1.wasm
```

### 入りきらない入力を範囲で読む

点群の LAS・LAZ のように大きな入力は、`openRanged` で初めの部分だけを渡して開けます（決定 0232）。`open` が `needs` で求めた範囲だけを読み、その名前で `push` して同じ要求を呼び直します。読み終えた `done` の入力は `close` で捨てます。範囲で読めない形式では、初めの部分を全体として扱います。そのため、全体の大きさが `head` を超える入力には範囲で読める形式を使います。

```js
import { openRanged } from "@zumen-jp/zumen";

const response = await openRanged(session, { v: 1, input: "a.las" }, file); // File・Blob
await openRanged(session, { v: 1, input: "b.las" }, "https://example.com/b.las"); // Range に 206 で答える URL
await openRanged(session, { v: 1, input: "c.las" }, (start, end) => read(start, end), { size }); // 関数と全体の大きさ
```

`options.head` は 1 回目に渡す大きさで、既定は 64 KiB です。1 回の範囲の合計が要求の `budget.input_bytes` を超えるとき、同じ範囲を求め続けるとき、読み口が求めた長さを返さないときは止めます。止まったら、渡した初めの部分と範囲を捨てます。文書ができたら、初めの部分は普段の `open` の入力と同じく残ります。

### 分けたモジュール

図面・BIM・土木地理・点群を分けたモジュール（`zumen_abi1_drawing.wasm`・`zumen_abi1_bim.wasm`・`zumen_abi1_civil.wasm`・`zumen_abi1_pointcloud.wasm`）もあります（[API のスキーマ](../../schema/README.md)）。`loadModules` は、要るモジュールだけを、初めて要るときに読み込みます。入っていない op は `call.op.unsupported` の診断で返ります。

5 つの wasm は、同じ npm パッケージと GitHub の Release に入っています（[配布の手順](../../xtask/release/README.md)）。ブラウザー向けに Vite などで組むときは、`@zumen-jp/zumen/wasm` の `wasmUrls` を使います。パッケージの中で静的な相対パスを `new URL(..., import.meta.url)` に渡しているため、bundler が wasm を見つけて出力へ写し、URL を書き換えられます。

```js
import { loadModules } from "@zumen-jp/zumen";
import { wasmUrls } from "@zumen-jp/zumen/wasm";

const modules = loadModules({
  drawing: () => fetch(wasmUrls.drawing),
  bim: () => fetch(wasmUrls.bim),
  civil: () => fetch(wasmUrls.civil),
  pointcloud: () => fetch(wasmUrls.pointcloud),
});
const zumen = await modules.forInput(bytes); // 形式を判定し（detect）、それを開けるモジュールを返す。判定できなければ undefined
const viewer = await modules.forOp("scene"); // op を持つモジュール
const opened = await modules.open("a.dxf", bytes); // 形式を持つモジュールの新しいセッションで開く。判定できなければ undefined
opened.session.call({ v: 1, op: "drawing", doc: "a.dxf" });
opened.session.close();
```

`wasmUrls` は URL の文字列の一覧で、import だけでは wasm を取得しません。キーは `all`・`drawing`・`bim`・`civil`・`pointcloud` です。全体版を使う場合は `load(fetch(wasmUrls.all))` と書けます。npm のインストールと bundler の出力には 5 つとも含まれますが、ブラウザーが取得してコンパイルするのは、呼んだ loader のモジュールだけです。

この URL の使い方はブラウザー向けです。Vite の SSR には使わず、Node.js のサーバーでは上の `@zumen-jp/zumen/node` を使います（[Vite の `new URL` の説明](https://vite.dev/guide/assets.html#new-url-url-import-meta-url)）。bundler を使わず配信する場合は、Release の wasm をサーバーに置き、その URL を `fetch` に渡せます。

バイト列を直接取り出すための exports もあります。

| npm の subpath | 内容 |
| --- | --- |
| `@zumen-jp/zumen/zumen_abi1.wasm` | 全体版 |
| `@zumen-jp/zumen/zumen_abi1_drawing.wasm` | 図面 |
| `@zumen-jp/zumen/zumen_abi1_bim.wasm` | BIM |
| `@zumen-jp/zumen/zumen_abi1_civil.wasm` | 土木・地理 |
| `@zumen-jp/zumen/zumen_abi1_pointcloud.wasm` | 点群 |

Node.js で分割版を使うときは、たとえば `readFile(new URL(import.meta.resolve("@zumen-jp/zumen/zumen_abi1_drawing.wasm")))` のバイト列を `load` に渡します。ブラウザーでは、パッケージ名を `new URL` に直接渡さず、上の `wasmUrls` を使います。

| 名前 | 中身 |
| --- | --- |
| `detect` | 入力を、`open` と同じ順と規則で判定し、合った `DETECTION` の行（`format`・`module`）を返す（どれでもなければ `undefined`） |
| `modulesWith` | op を持つモジュールの名前 |
| `loadModules` | `Modules.get`・`forInput`・`forOp` で、モジュールを読み込んで覚えておく。`Modules.open` で、形式を持つモジュールの新しいセッションで入力を開く（[API のスキーマ](../../schema/README.md)） |

外部のプラグインを束ねたモジュール（[プラグインの作り方](../../templates/plugin/README.md)）は、`loadModules` の 2 つ目の引数に、名前・形式ごとの判定の規則（`formats`）・持つ op（`ops`）・読み込む関数（`load`）を渡します。コアのどの形式でもない入力を、その規則で試します。図面の形式のプラグインのモジュールは、プラグインの形式だけを持ちます（[API のスキーマ](../../schema/README.md)）。開いた文書を `lift` の op で図面モデルのバイナリ（blob の種類は `drawing`）にし、図面のモジュールのセッションに `push` して `open` すると、図面を読む op を使えます。`Modules.open` は、この受け渡しをして、図面のモジュールのセッションを返します。`detect` は、図面モデルのバイナリを形式 `drawing` と判定します（`open` は元の文書の形式を返します）。作り方は [templates/plugin](../../templates/plugin/README.md) にあります。

`xml_root` の規則の `name` と `namespace` は Unicode の文字列です。UTF-8・Shift_JIS・UTF-16 の原本を同じ期待名で比べます。Rust の `Signature::XmlRoot` の期待名も UTF-8 に統一します。受理条件と移行方法は [XML の規則](../../docs/formats/xml.md)にあります。

要求と応答の形は [schema/messages.json](../../schema/messages.json) にあり、`call` の型は要求の `op` から結果の型を引きます。

## テスト

```bash
cargo build -p zumen-abi --target wasm32-unknown-unknown --release
node bindings/js/test/conformance.mjs target/wasm32-unknown-unknown/release/zumen_abi1.wasm
```

`test/ranged.mjs` は、点を足した LAS を全体で渡したときと、関数・`Blob`・URL の読み口から範囲で渡したときの `pointcloud` の応答を比べます。止まったときに入力を捨てることも確かめます。

`test/conformance.mjs` は、[適合テスト](../conformance/README.md)の組をこのラッパーで流し、ハッシュの一覧が `hashes.txt` と一致することを確かめます。型は、CI がサイトと同じバージョンの TypeScript で `tsconfig.json`（`test/types.ts` を含む）を確かめます。

ブラウザーの中でも、同じ組を流せます（[適合テスト](../conformance/README.md)）。ツールは `test/browser` にバージョンを固定して置いてあり、ブラウザーは Playwright で取ります。CI は、PR ごとに Chromium、毎晩に Firefox と WebKit で流します。

```bash
npm ci --prefix bindings/js/test/browser --ignore-scripts
npx --prefix bindings/js/test/browser playwright install chromium
node bindings/js/test/browser/run.mjs chromium target/wasm32-unknown-unknown/release/zumen_abi1.wasm
```

分けたモジュールでは、`test/modules.mjs` が、モジュールごとの op と、`detect` が `open` と同じ形式を選ぶことを確かめ、組ごとに合うモジュールで適合テストを流します。`test/browser/run.mjs` に分けたモジュールを渡すと、ブラウザーの中でも同じことをします。

```bash
node bindings/js/test/modules.mjs zumen_abi1.wasm zumen_abi1_drawing.wasm zumen_abi1_bim.wasm zumen_abi1_civil.wasm zumen_abi1_pointcloud.wasm zumen_abi1_plugin.wasm
```

`test/server.mjs` は、Node.js の入口・別の realm で作ったモジュール・trap のあとの扱いを確かめます。CI は PR ごとに流します。

```bash
node bindings/js/test/server.mjs target/wasm32-unknown-unknown/release/zumen_abi1.wasm
```

`Session.push` は、新しい wasm では入力の領域の所有権をコアへ渡します。最初の入力を wasm の中でもう一度写す必要がなくなり、300 MiB の入力を渡す測定では、wasm のメモリが約 602 MiB から 301 MiB に減りました。ホストの入力は引き続き使えます。追記や文書を開く際のメモリは別に必要です。新しい関数がない旧 ABI 1 の wasm では、これまでの入力の渡し方を使います（[API のスキーマ](../../schema/README.md)）。

`test/input.mjs` は、新旧の経路、空の入力と追記、失敗時の解放、同じ wasm のメモリを指す入力、trap の後片付けとメモリの削減を確かめます。CI は PR ごとに 32 MiB の入力で測ります。`measure` を付けると、指定したバイト数を 1 回渡す前後の wasm のメモリを表示します。測定には、ホストの入力と文書の解析は含みません。

```bash
node bindings/js/test/input.mjs target/wasm32-unknown-unknown/release/zumen_abi1.wasm
node bindings/js/test/input.mjs target/wasm32-unknown-unknown/release/zumen_abi1.wasm measure owned 314572800
node bindings/js/test/input.mjs target/wasm32-unknown-unknown/release/zumen_abi1.wasm measure legacy 314572800
```

`test/worker.mjs` は、上の実行例を使い、20 万本の線を持つ合成 DXF の処理中にも主スレッドのタイマーが進むことを確かめます。同期 API と同じ blob、入力の転送、診断、中断・転送失敗・結果を返さない終了の後始末も確かめます。CI は PR ごとに流します。

```bash
node bindings/js/test/worker.mjs target/wasm32-unknown-unknown/release/zumen_abi1.wasm
```

`test/nextjs` は、`npm pack` したパッケージを Next.js の Turbopack（設定なし）と webpack（`serverExternalPackages`）で standalone に組み、組んだサーバーの API ルートで図面を開きます。Next.js と React は、`test/nextjs/package.json` でバージョンを固定しています。CI は毎晩流します。

```bash
npm ci --prefix bindings/js/test/nextjs --ignore-scripts
node bindings/js/test/nextjs/run.mjs target/wasm32-unknown-unknown/release/zumen_abi1.wasm
```

`test/browser/bundle.mjs` は、5 つの wasm を入れて `npm pack` し、Vite の production build を Chromium で開きます。npm の exports と URL から取り出す内容、ルート以外の配信先、図面を開くときの遅延取得、各モジュールの op、2 回目に取得し直さないことを確かめます。Vite と Playwright は `test/browser/package.json` で固定し、CI は PR ごとに流します。

```bash
npm ci --prefix bindings/js/test/browser --ignore-scripts
npm exec --prefix bindings/js/test/browser -- playwright install --only-shell chromium
node bindings/js/test/browser/bundle.mjs target/wasm32-unknown-unknown/release/zumen_abi1.wasm modules/zumen_abi1_drawing.wasm modules/zumen_abi1_bim.wasm modules/zumen_abi1_civil.wasm modules/zumen_abi1_pointcloud.wasm
```

この検査の前に、各 feature を `cargo build -p zumen-abi --target wasm32-unknown-unknown --release --no-default-features --features drawing` などで作り、出力を上の名前で保存します。最後に全体版を既定の feature で作ります。

## 3D のシーン

readScene3D(session.take(response.result.blob)) で ZMS3 を読みます。scene.element(scene.instance(0).element).id から建物の要素 ID を引けます。既定の確保量 256 MiB と作業量 100,000,000 は、第二引数の maxAllocatedBytes・maxWork で指定できます。列は可能なら元の blob を共有し、未整列の場合はコピーします。

同じ blob の 2D 用の読み手との混用はできません。列の配置・座標・単位・色・選択と検査範囲は [3D のシーンの契約](../../schema/README.md#3d-のシーンの契約)に従います。元の blob を共有する読み手の利用中は、そのバイト列を変更しません。
