import { fileURLToPath } from "node:url";
import { resolve } from "node:path";
import { defineConfig, normalizePath } from "vite";

const root = fileURLToPath(new URL(".", import.meta.url));
const wasm = process.env.ZUMEN_DRAWING_WASM ?? resolve(root, "../target/wasm32-unknown-unknown/release/zumen_abi1.wasm");
export default defineConfig({
  root,
  base: "./",
  resolve: {
    dedupe: ["react", "react-dom", "@zumen-jp/view2d", "@zumen-jp/zumen"],
    alias: [{ find: /^drawing-module\?url$/, replacement: normalizePath(resolve(wasm)) + "?url" }],
  },
  // CSS は public からそのまま写す（決定 0268）。JS に組み込んだ依存のライセンスも出力する。
  build: { assetsInlineLimit: 0, cssMinify: false, license: { fileName: "licenses.md" } },
  worker: { format: "es" },
});
