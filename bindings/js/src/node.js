// Node.js から、包みに入れた wasm のモジュール（zumen_abi1.wasm）を読み込む入口（決定 0259）。
//
// wasm は new URL(…, import.meta.url) の形で指す。Node.js はこのまま読み、Next.js の Turbopack はこの形を辿って、
// 組んだ出力に wasm を入れる。webpack で組むときは、包みを serverExternalPackages に入れ、組まずに Node.js に読ませる。

import { readFile } from "node:fs/promises";

import { load } from "./index.js";

export * from "./index.js";

/** コンパイルしたモジュール。失敗したら捨て、次に呼んだときにやり直す */
let compiled;

/** 包みに入れた wasm を読み込む。コンパイルは 1 度だけにし、呼ぶたびに新しいインスタンスを返す */
export function loadBundled() {
  compiled ??= readFile(new URL("../zumen_abi1.wasm", import.meta.url))
    .then((bytes) => WebAssembly.compile(bytes))
    .catch((error) => {
      compiled = undefined;
      throw error;
    });
  return load(compiled);
}
