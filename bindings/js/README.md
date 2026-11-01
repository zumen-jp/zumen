# TypeScript・JavaScript の包み

wasm のモジュール（`zumen_abi1.wasm`）を、ブラウザーと Node.js（22 以降）から呼ぶ薄い包みです（[決定 0011](../../docs/decisions/0011-bindings-through-ffi.md)・[決定 0194](../../docs/decisions/0194-typescript-javascript-wrapper.md)）。

- npm の `@zumen-jp/zumen` で配ります（[決定 0003](../../docs/decisions/0003-names-and-domain.md)・[決定 0205](../../docs/decisions/0205-release-and-publishing.md)）。包みには `zumen_abi1.wasm` を入れ、`@zumen-jp/zumen/zumen_abi1.wasm` で取り出せます。
- 素の ESM の JavaScript で書き、ビルドも npm の依存もありません。型は `.d.ts` にあります。
- `src/messages.d.ts`・`src/constants.js`・`src/constants.d.ts` は、`schema/` から `cargo xtask codegen` で生成します。手で直さないでください。

## 使い方

```js
import { readFileSync } from "node:fs";
import { ZumenError, load, readScene } from "./src/index.js";

// ブラウザーでは load(fetch(wasm の URL)) と書けます
const zumen = await load(readFileSync(new URL(import.meta.resolve("@zumen-jp/zumen/zumen_abi1.wasm"))));
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
| `load`・`loadSync` | モジュールを読み込み、ABI の大版を確かめる |
| `Zumen.open` | セッションを作る（`zm_session_new`） |
| `Session.push`・`call`・`callBytes`・`take`・`cancel`・`close` | 境界の関数（`zm_push`・`zm_call`・`zm_take`・`zm_cancel`・`zm_session_close`）。状態が `ok` でなければ `ZumenError` を投げる |
| `readScene` | シーンの blob を、区画ごとの型付き配列で読む（`schema/scene.toml`） |

### 分けたモジュール

図面と BIM を分けたモジュール（`zumen_abi1_drawing.wasm`・`zumen_abi1_bim.wasm`）もあります（[決定 0221](../../docs/decisions/0221-wasm-modules.md)）。`loadModules` は、要るモジュールだけを、初めて要るときに読み込みます。入っていない op は `call.op.unsupported` の診断で返ります。

```js
import { detect, loadModules } from "./src/index.js";

const modules = loadModules({
  drawing: () => fetch("/zumen_abi1_drawing.wasm"),
  bim: () => fetch("/zumen_abi1_bim.wasm"),
});
const zumen = await modules.forInput(bytes); // 形式を判定し（detect）、それを開けるモジュールを返す。判定できなければ undefined
const viewer = await modules.forOp("scene"); // op を持つモジュール
```

| 名前 | 中身 |
| --- | --- |
| `detect` | 入力を、`open` と同じ順と規則で判定し、合った `DETECTION` の行（`format`・`module`）を返す（どれでもなければ `undefined`） |
| `modulesWith` | op を持つモジュールの名前 |
| `loadModules` | `Modules.get`・`forInput`・`forOp` で、モジュールを読み込んで覚えておく |

外部のプラグインを束ねたモジュール（[決定 0243](../../docs/decisions/0243-plugin-bundling.md)）は、`loadModules` の 2 つ目の引数に、名前・形式ごとの判定の規則（`formats`）・持つ op（`ops`）・読み込む関数（`load`）を渡します。コアのどの形式でもない入力を、その規則で試します。作り方は [templates/plugin](../../templates/plugin/README.md) にあります。

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
node bindings/js/test/modules.mjs zumen_abi1.wasm zumen_abi1_drawing.wasm zumen_abi1_bim.wasm
```
