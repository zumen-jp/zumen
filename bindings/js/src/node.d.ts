// Node.js から、パッケージに入れた wasm のモジュールを読み込む入口の型（決定 0259）。

import type { Zumen } from "./index.js";

export * from "./index.js";

/** パッケージに入れた wasm（zumen_abi1.wasm）を読み込む。コンパイルは 1 度だけにし、呼ぶたびに新しいインスタンスを返す */
export declare function loadBundled(): Promise<Zumen>;
