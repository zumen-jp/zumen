import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";

export default defineConfig({
  root: fileURLToPath(new URL(".", import.meta.url)),
  resolve: {
    alias: [
      { find: "@zumen-jp/zumen", replacement: fileURLToPath(new URL("../../../../../bindings/js/src/index.js", import.meta.url)) },
      ...["three", "three/tsl", "three/webgpu"].map((name) => ({
        find: new RegExp(`^${name}$`),
        replacement: fileURLToPath(import.meta.resolve(name)),
      })),
    ],
  },
  build: { target: "esnext", outDir: fileURLToPath(new URL("../../../../../target/browser-webgpu/", import.meta.url)) },
});
