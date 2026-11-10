// 決定 0296。公開済みの wasm と、既存の表示部品・プレイグラウンドを組む。Rust は起動しない。
import { createRequire } from "node:module";
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { build, normalizePath } from "vite";
import { samplesPlugin } from "../../playground/samples.js";

const site = fileURLToPath(new URL("../", import.meta.url));
const root = resolve(site, "..");
const require = createRequire(new URL("../package.json", import.meta.url));
const core = dirname(dirname(require.resolve("@zumen-jp/zumen")));

// ログイン名を含む 2 件の DWG はサイトには入れない（#737 の軽微 6）。出所とライセンスは共通の台帳から出す。
export const samplePaths = ["dxf/text_alignments.dxf", "dxf/opendxfviewer-latin1.dxf", "dwg/zumen-layout-r2000.dwg"];

await build({
  configFile: false,
  root: resolve(root, "playground"),
  base: "/viewer/",
  plugins: [samplesPlugin(root, samplePaths), {
    name: "zumen-runtime-notices",
    generateBundle() {
      // wasm に入る依存の本文と、その実物の SBOM も公開済みのパッケージから保つ（決定 0269・0279）。
      for (const name of ["LICENSE-MIT", "LICENSE-APACHE", "THIRD_PARTY_LICENSES", "sboms/zumen_abi1_drawing.wasm.cdx.json"]) {
        this.emitFile({ type: "asset", fileName: name, source: readFileSync(resolve(core, name)) });
      }
    },
  }],
  define: { "import.meta.env.VITE_ZUMEN_CORE_VERSION": JSON.stringify("0.2.0") },
  resolve: {
    dedupe: ["react", "react-dom", "@zumen-jp/view2d", "@zumen-jp/zumen"],
    alias: [
      { find: /^drawing-module\?url$/, replacement: normalizePath(resolve(core, "zumen_abi1_drawing.wasm")) + "?url" },
      { find: /^@zumen-jp\/zumen$/, replacement: require.resolve("@zumen-jp/zumen") },
      { find: /^@zumen-jp\/react$/, replacement: require.resolve("@zumen-jp/react") },
      { find: /^@zumen-jp\/view2d$/, replacement: require.resolve("@zumen-jp/view2d") },
      { find: "react-dom", replacement: dirname(require.resolve("react-dom/package.json")) },
      { find: "react", replacement: dirname(require.resolve("react/package.json")) },
    ],
  },
  build: { outDir: resolve(site, "dist/viewer"), emptyOutDir: true, assetsInlineLimit: 0, cssMinify: false, license: { fileName: "licenses.md" } },
  worker: { format: "es" },
});
