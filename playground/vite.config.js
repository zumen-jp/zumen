import { fileURLToPath } from "node:url";
import { resolve } from "node:path";
import { defineConfig, normalizePath } from "vite";
import { samplesPlugin } from "./samples.js";

const root = fileURLToPath(new URL(".", import.meta.url));
const wasm = process.env.ZUMEN_DRAWING_WASM ?? resolve(root, "../target/wasm32-unknown-unknown/release/zumen_abi1.wasm");
// 3D のページ（view3d.html）は、IFC・地物・点群の scene3d と図面を持つ全部入りの wasm を使う。
const scene3d = process.env.ZUMEN_SCENE3D_WASM ?? wasm;
export default defineConfig({
  root,
  base: "./",
  plugins: [samplesPlugin(resolve(root, ".."))],
  resolve: {
    dedupe: ["react", "react-dom", "three", "@zumen-jp/view2d", "@zumen-jp/view3d", "@zumen-jp/zumen"],
    alias: [
      { find: /^drawing-module\?url$/, replacement: normalizePath(resolve(wasm)) + "?url" },
      { find: /^scene3d-module\?url$/, replacement: normalizePath(resolve(scene3d)) + "?url" },
    ],
  },
  // CSS は public からそのまま写す（決定 0268）。JS に組み込んだ依存のライセンスも出力する。同梱した公開サンプルの
  // 出所とライセンスは samples.md に出す（#689）。
  build: {
    assetsInlineLimit: 0, cssMinify: false, license: { fileName: "licenses.md" },
    rolldownOptions: { input: { index: resolve(root, "index.html"), view3d: resolve(root, "view3d.html") } },
  },
  worker: { format: "es" },
});
