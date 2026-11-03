# TypeScript・JavaScript の包み

wasm のモジュール（`zumen_abi1.wasm`）を、ブラウザーと Node.js（22 以降）から呼ぶ薄い包みです（[決定 0011](../../docs/decisions/0011-bindings-through-ffi.md)・[決定 0194](../../docs/decisions/0194-typescript-javascript-wrapper.md)・[決定 0259](../../docs/decisions/0259-javascript-on-node-servers.md)）。

- npm の `@zumen-jp/zumen` で配ります（[決定 0003](../../docs/decisions/0003-names-and-domain.md)・[決定 0205](../../docs/decisions/0205-release-and-publishing.md)）。包みには `zumen_abi1.wasm` を入れ、`@zumen-jp/zumen/zumen_abi1.wasm` で取り出せます。依存のライセンスの文は `THIRD_PARTY_LICENSES` にあります。
- 素の ESM の JavaScript で書き、ビルドも npm の依存もありません。型は `.d.ts` にあります。
- `src/messages.d.ts`・`src/constants.js`・`src/constants.d.ts` は、`schema/` から `cargo xtask codegen` で生成します。手で直さないでください。

## 使い方

Node.js では、`@zumen-jp/zumen/node` の `loadBundled` が、包みに入れた wasm を読み込みます。コンパイルは 1 度だけで、呼ぶたびに新しいインスタンスを返します。ブラウザーでは、`@zumen-jp/zumen` の `load` に、wasm の fetch の応答（`load(fetch("/zumen_abi1.wasm"))`）・バイト列・コンパイルしたモジュールを渡します。

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
| `load`・`loadSync` | モジュールを読み込み、ABI の大版を確かめる。別の realm（Next.js の Edge のランタイムなど）で作ったモジュールやバイト列も受け取る |
| `loadBundled` | 包みに入れた wasm を読み込む。`@zumen-jp/zumen/node` にあり、Node.js だけで使える |
| `Zumen.open` | セッションを作る（`zm_session_new`） |
| `Zumen.broken` | wasm の中で trap が起き、もう使えないか |
| `Session.push`・`call`・`callBytes`・`take`・`cancel`・`close` | 境界の関数（`zm_push`・`zm_call`・`zm_take`・`zm_cancel`・`zm_session_close`）。状態が `ok` でなければ `ZumenError` を投げる |
| `readScene` | シーンの blob を、区画ごとの型付き配列で読む（`schema/scene.toml`） |

### trap のあと

wasm の中のパニックとメモリ不足は trap になり、そのインスタンスはもう使えません（[決定 0010](../../docs/decisions/0010-budget-by-work.md)）。包みは、trap が起きた呼び出しと、それからの呼び出しを、状態が `panicked` の `ZumenError` にし、元の例外（`WebAssembly.RuntimeError` など）を `cause` に入れます。`Zumen.broken` が `true` になったら、モジュールを読み込み直します。`loadModules` の `Modules` は、使えなくなったインスタンスを捨て、次の `get`・`open`・`forOp` で読み込み直します。読み込みに失敗したときも覚えず、次に引いたときにやり直します。`Session.close` と `cancel` は、使えなくなったインスタンスでは何もせず、`finally` の中で呼んでも先に出た例外を隠しません。

### Next.js などのサーバー

- Node.js のランタイムで使います。Next.js の文書は、Edge のランタイムを Proxy で使うものとしています。Vercel では、Next.js 16.3 から route と page の `runtime = 'edge'` が使えず、Node.js で動きます。
- Turbopack（Next.js 16 の既定）で組むときは、設定は要りません。webpack（`next build --webpack`）で組むときは、`next.config` の `serverExternalPackages` に `"@zumen-jp/zumen"` を入れます。webpack は包みの wasm を、ブラウザー向けの URL に書き換えるためです。どちらも、`output: "standalone"` で組んだ出力に wasm が入ります。
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

### 分けたモジュール

図面・BIM・土木地理・点群を分けたモジュール（`zumen_abi1_drawing.wasm`・`zumen_abi1_bim.wasm`・`zumen_abi1_civil.wasm`・`zumen_abi1_pointcloud.wasm`）もあります（[決定 0221](../../docs/decisions/0221-wasm-modules.md)・[決定 0253](../../docs/decisions/0253-wasm-civil-and-pointcloud-modules.md)）。`loadModules` は、要るモジュールだけを、初めて要るときに読み込みます。入っていない op は `call.op.unsupported` の診断で返ります。

```js
import { detect, loadModules } from "./src/index.js";

const modules = loadModules({
  drawing: () => fetch("/zumen_abi1_drawing.wasm"),
  bim: () => fetch("/zumen_abi1_bim.wasm"),
  civil: () => fetch("/zumen_abi1_civil.wasm"),
  pointcloud: () => fetch("/zumen_abi1_pointcloud.wasm"),
});
const zumen = await modules.forInput(bytes); // 形式を判定し（detect）、それを開けるモジュールを返す。判定できなければ undefined
const viewer = await modules.forOp("scene"); // op を持つモジュール
const opened = await modules.open("a.dxf", bytes); // 形式を持つモジュールの新しいセッションで開く。判定できなければ undefined
opened.session.call({ v: 1, op: "drawing", doc: "a.dxf" });
opened.session.close();
```

| 名前 | 中身 |
| --- | --- |
| `detect` | 入力を、`open` と同じ順と規則で判定し、合った `DETECTION` の行（`format`・`module`）を返す（どれでもなければ `undefined`） |
| `modulesWith` | op を持つモジュールの名前 |
| `loadModules` | `Modules.get`・`forInput`・`forOp` で、モジュールを読み込んで覚えておく。`Modules.open` で、形式を持つモジュールの新しいセッションで入力を開く（[決定 0250](../../docs/decisions/0250-open-plugin-drawings-in-the-drawing-module.md)） |

外部のプラグインを束ねたモジュール（[決定 0243](../../docs/decisions/0243-plugin-bundling.md)）は、`loadModules` の 2 つ目の引数に、名前・形式ごとの判定の規則（`formats`）・持つ op（`ops`）・読み込む関数（`load`）を渡します。コアのどの形式でもない入力を、その規則で試します。図面の形式のプラグインのモジュールは、プラグインの形式だけを持ちます（[決定 0247](../../docs/decisions/0247-drawing-model-binary.md)）。開いた文書を `lift` の op で図面モデルのバイナリ（blob の種類は `drawing`）にし、図面のモジュールのセッションに `push` して `open` すると、図面を読む op を使えます。`Modules.open` は、この受け渡しをして、図面のモジュールのセッションを返します。`detect` は、図面モデルのバイナリを形式 `drawing` と判定します（`open` は元の文書の形式を返します）。作り方は [templates/plugin](../../templates/plugin/README.md) にあります。

要求と応答の形は [schema/messages.json](../../schema/messages.json) にあり、`call` の型は要求の `op` から結果の型を引きます。

## 試験

```bash
cargo build -p zumen-abi --target wasm32-unknown-unknown --release
node bindings/js/test/conformance.mjs target/wasm32-unknown-unknown/release/zumen_abi1.wasm
```

`test/conformance.mjs` は、[適合試験](../conformance/README.md)の組をこの包みで流し、ハッシュの一覧が `hashes.txt` と一致することを確かめます。型は、CI がサイトと同じ版の TypeScript で `tsconfig.json`（`test/types.ts` を含む）を確かめます。

ブラウザーの中でも、同じ組を流せます（[決定 0199](../../docs/decisions/0199-browser-conformance-with-playwright.md)）。道具は `test/browser` に版を固定して置いてあり、ブラウザーは Playwright で取ります。CI は、PR ごとに Chromium、毎晩に Firefox と WebKit で流します。

```bash
npm ci --prefix bindings/js/test/browser --ignore-scripts
npx --prefix bindings/js/test/browser playwright install chromium
node bindings/js/test/browser/run.mjs chromium target/wasm32-unknown-unknown/release/zumen_abi1.wasm
```

分けたモジュールでは、`test/modules.mjs` が、モジュールごとの op と、`detect` が `open` と同じ形式を選ぶことを確かめ、組ごとに合うモジュールで適合試験を流します。`test/browser/run.mjs` に分けたモジュールを渡すと、ブラウザーの中でも同じことをします。

```bash
node bindings/js/test/modules.mjs zumen_abi1.wasm zumen_abi1_drawing.wasm zumen_abi1_bim.wasm zumen_abi1_civil.wasm zumen_abi1_pointcloud.wasm zumen_abi1_plugin.wasm
```

`test/server.mjs` は、Node.js の入口・別の realm で作ったモジュール・trap のあとの扱いを確かめます（[決定 0259](../../docs/decisions/0259-javascript-on-node-servers.md)）。CI は PR ごとに流します。

```bash
node bindings/js/test/server.mjs target/wasm32-unknown-unknown/release/zumen_abi1.wasm
```

`test/nextjs` は、`npm pack` した包みを Next.js の Turbopack（設定なし）と webpack（`serverExternalPackages`）で standalone に組み、組んだサーバーの API ルートで図面を開きます。Next.js と React は、`test/nextjs/package.json` で版を固定しています。CI は毎晩流します。

```bash
npm ci --prefix bindings/js/test/nextjs --ignore-scripts
node bindings/js/test/nextjs/run.mjs target/wasm32-unknown-unknown/release/zumen_abi1.wasm
```
