import { load, loadModules } from "@zumen-jp/zumen";
import { wasmUrls } from "@zumen-jp/zumen/wasm";

const modules = loadModules({
  drawing: () => fetch(wasmUrls.drawing),
  bim: () => fetch(wasmUrls.bim),
  civil: () => fetch(wasmUrls.civil),
  pointcloud: () => fetch(wasmUrls.pointcloud),
});

window.bundled = {
  urls: wasmUrls,
  async openDrawing() {
    const bytes = new TextEncoder().encode("0\nSECTION\n2\nENTITIES\n0\nLINE\n10\n0\n20\n0\n11\n1\n21\n1\n0\nENDSEC\n0\nEOF\n");
    const opened = await modules.open("line.dxf", bytes);
    try {
      return opened.response.result.format;
    } finally {
      opened.session.close();
    }
  },
  async ops(name) {
    const zumen = name === "all" ? await load(fetch(wasmUrls.all)) : await modules.get(name);
    const session = zumen.open();
    try {
      return session.call({ v: 1, op: "capabilities" }).result.ops;
    } finally {
      session.close();
    }
  },
};
