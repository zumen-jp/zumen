# TypeScript・JavaScript のラッパー

wasm のモジュール（`zumen_abi1.wasm`）を、ブラウザーと Node.js（22 以降）から呼ぶ薄いラッパーです（[決定 0011](../../docs/decisions/0011-bindings-through-ffi.md)・[決定 0194](../../docs/decisions/0194-typescript-javascript-wrapper.md)・[決定 0259](../../docs/decisions/0259-javascript-on-node-servers.md)）。

- npm の `@zumen-jp/zumen` で配ります（[決定 0003](../../docs/decisions/0003-names-and-domain.md)・[決定 0205](../../docs/decisions/0205-release-and-publishing.md)）。パッケージには全体版と 4 つの分割版の wasm を入れます。依存のライセンスの文は `THIRD_PARTY_LICENSES` にあります。
- 素の ESM の JavaScript で書き、ビルドも npm の依存もありません。型は `.d.ts` にあります。
- `src/messages.d.ts`・`src/constants.js`・`src/constants.d.ts` は、`schema/` から `cargo xtask codegen` で生成します。手で直さないでください。

## 使い方

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

### trap のあと

wasm の中のパニックとメモリ不足は trap になり、そのインスタンスはもう使えません（[決定 0010](../../docs/decisions/0010-budget-by-work.md)）。ラッパーは、trap が起きた呼び出しと、それからの呼び出しを、状態が `panicked` の `ZumenError` にし、元の例外（`WebAssembly.RuntimeError` など）を `cause` に入れます。`Zumen.broken` が `true` になったら、モジュールを読み込み直します。`loadModules` の `Modules` は、使えなくなったインスタンスを捨て、次の `get`・`open`・`forOp` で読み込み直します。読み込みに失敗したときも覚えず、次に引いたときにやり直します。`Session.close` と `cancel` は、使えなくなったインスタンスでは何もせず、`finally` の中で呼んでも先に出た例外を隠しません。

### Next.js などのサーバー

- Node.js のランタイムで使います。Next.js の文書は、Edge のランタイムを Proxy で使うものとしています。Vercel では、Next.js 16.3 から route と page の `runtime = 'edge'` が使えず、Node.js で動きます。
- Turbopack（Next.js 16 の既定）で組むときは、設定は要りません。webpack（`next build --webpack`）で組むときは、`next.config` の `serverExternalPackages` に `"@zumen-jp/zumen"` を入れます。webpack はパッケージの wasm を、ブラウザー向けの URL に書き換えるためです。どちらも、`output: "standalone"` で組んだ出力に wasm が入ります。
- 要求ごとに `loadBundled` で新しいインスタンスを使います。壊れたファイルで起きた trap がほかの要求に及ばず、wasm のメモリも要求が終われば回収されます。wasm のメモリは縮まないので、1 つのインスタンスを使い回すと、いちばん大きな入力を読んだときの大きさのままになります。
- 受け取ったファイルを読むときは、要求の `budget` で量に上限を付けます（[決定 0010](../../docs/decisions/0010-budget-by-work.md)）。
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
- worker は同じプロセスの中で動きます。[`resourceLimits` の対象](https://nodejs.org/download/release/latest-v22.x/docs/api/worker_threads.html#new-workerfilename-options)は JavaScript のエンジンで、ArrayBuffer や wasm のメモリの上限にはなりません。入力の大きさと API の `budget`、同時実行数で量を制限します。OS でメモリを制限したい場合や、ネイティブの CLI を別のプロセスとして動かしたい場合は、子プロセスのエンジン（#358）の用途になります。#358 の Node.js 用ラッパーはまだありません。
- この例は素の Node.js で確かめています。Next.js などで組むときは、worker のファイルと、そのファイルから解決できる npm パッケージをサーバーの出力に含めます。上の同期の API ルートの例に対する bundler のテストは、worker の配布までを確認するものではありません。

リポジトリでは、作った wasm のパスを渡して実行できます。

```bash
node bindings/js/examples/worker/main.mjs samples/dxf/text_alignments.dxf target/wasm32-unknown-unknown/release/zumen_abi1.wasm
```

### 分けたモジュール

図面・BIM・土木地理・点群を分けたモジュール（`zumen_abi1_drawing.wasm`・`zumen_abi1_bim.wasm`・`zumen_abi1_civil.wasm`・`zumen_abi1_pointcloud.wasm`）もあります（[決定 0221](../../docs/decisions/0221-wasm-modules.md)・[決定 0253](../../docs/decisions/0253-wasm-civil-and-pointcloud-modules.md)）。`loadModules` は、要るモジュールだけを、初めて要るときに読み込みます。入っていない op は `call.op.unsupported` の診断で返ります。

5 つの wasm は、同じ npm パッケージと GitHub の Release に入っています（[決定 0268](../../docs/decisions/0268-distribute-wasm-modules.md)）。ブラウザー向けに Vite などで組むときは、`@zumen-jp/zumen/wasm` の `wasmUrls` を使います。パッケージの中で静的な相対パスを `new URL(..., import.meta.url)` に渡しているため、bundler が wasm を見つけて出力へ写し、URL を書き換えられます。

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
| `loadModules` | `Modules.get`・`forInput`・`forOp` で、モジュールを読み込んで覚えておく。`Modules.open` で、形式を持つモジュールの新しいセッションで入力を開く（[決定 0250](../../docs/decisions/0250-open-plugin-drawings-in-the-drawing-module.md)） |

外部のプラグインを束ねたモジュール（[決定 0243](../../docs/decisions/0243-plugin-bundling.md)）は、`loadModules` の 2 つ目の引数に、名前・形式ごとの判定の規則（`formats`）・持つ op（`ops`）・読み込む関数（`load`）を渡します。コアのどの形式でもない入力を、その規則で試します。図面の形式のプラグインのモジュールは、プラグインの形式だけを持ちます（[決定 0247](../../docs/decisions/0247-drawing-model-binary.md)）。開いた文書を `lift` の op で図面モデルのバイナリ（blob の種類は `drawing`）にし、図面のモジュールのセッションに `push` して `open` すると、図面を読む op を使えます。`Modules.open` は、この受け渡しをして、図面のモジュールのセッションを返します。`detect` は、図面モデルのバイナリを形式 `drawing` と判定します（`open` は元の文書の形式を返します）。作り方は [templates/plugin](../../templates/plugin/README.md) にあります。

要求と応答の形は [schema/messages.json](../../schema/messages.json) にあり、`call` の型は要求の `op` から結果の型を引きます。

## テスト

```bash
cargo build -p zumen-abi --target wasm32-unknown-unknown --release
node bindings/js/test/conformance.mjs target/wasm32-unknown-unknown/release/zumen_abi1.wasm
```

`test/conformance.mjs` は、[適合テスト](../conformance/README.md)の組をこのラッパーで流し、ハッシュの一覧が `hashes.txt` と一致することを確かめます。型は、CI がサイトと同じバージョンの TypeScript で `tsconfig.json`（`test/types.ts` を含む）を確かめます。

ブラウザーの中でも、同じ組を流せます（[決定 0199](../../docs/decisions/0199-browser-conformance-with-playwright.md)）。ツールは `test/browser` にバージョンを固定して置いてあり、ブラウザーは Playwright で取ります。CI は、PR ごとに Chromium、毎晩に Firefox と WebKit で流します。

```bash
npm ci --prefix bindings/js/test/browser --ignore-scripts
npx --prefix bindings/js/test/browser playwright install chromium
node bindings/js/test/browser/run.mjs chromium target/wasm32-unknown-unknown/release/zumen_abi1.wasm
```

分けたモジュールでは、`test/modules.mjs` が、モジュールごとの op と、`detect` が `open` と同じ形式を選ぶことを確かめ、組ごとに合うモジュールで適合テストを流します。`test/browser/run.mjs` に分けたモジュールを渡すと、ブラウザーの中でも同じことをします。

```bash
node bindings/js/test/modules.mjs zumen_abi1.wasm zumen_abi1_drawing.wasm zumen_abi1_bim.wasm zumen_abi1_civil.wasm zumen_abi1_pointcloud.wasm zumen_abi1_plugin.wasm
```

`test/server.mjs` は、Node.js の入口・別の realm で作ったモジュール・trap のあとの扱いを確かめます（[決定 0259](../../docs/decisions/0259-javascript-on-node-servers.md)）。CI は PR ごとに流します。

```bash
node bindings/js/test/server.mjs target/wasm32-unknown-unknown/release/zumen_abi1.wasm
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
