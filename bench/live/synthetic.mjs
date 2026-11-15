// コアの文書エンジンを測るため、決まった格子に線を置く自作 DXF を作る。
// node bench/live/synthetic.mjs <出力フォルダー> [件数 ...]
import { closeSync, mkdirSync, openSync, writeSync } from "node:fs";
import { join } from "node:path";

const [out, ...sizes] = process.argv.slice(2);
if (!out) throw new Error("使い方: node bench/live/synthetic.mjs <出力フォルダー> [件数 ...]");
const counts = sizes.length ? sizes.map(Number) : [10_000, 1_000_000];
for (const count of counts) {
  if (!Number.isSafeInteger(count) || count < 1 || count > 1_000_000) {
    throw new Error("件数は 1 から 1,000,000 の整数にする");
  }
}
mkdirSync(out, { recursive: true });
for (const count of counts) {
  const path = join(out, `lines-${count}.dxf`);
  const file = openSync(path, "w");
  const firstHandle = 0x100;
  try {
    writeSync(file, [
      "0", "SECTION", "2", "HEADER", "9", "$ACADVER", "1", "AC1015",
      "9", "$HANDSEED", "5", (firstHandle + count).toString(16).toUpperCase(),
      "0", "ENDSEC", "0", "SECTION", "2", "TABLES", "0", "TABLE", "2", "LAYER", "70", "1",
      "0", "LAYER", "5", "10", "100", "AcDbSymbolTableRecord", "100", "AcDbLayerTableRecord",
      "2", "0", "70", "0", "62", "7", "6", "CONTINUOUS", "0", "ENDTAB", "0", "ENDSEC",
      "0", "SECTION", "2", "ENTITIES", "",
    ].join("\n"));
    let chunk = "";
    for (let index = 0; index < count; index += 1) {
      const x = index % 1000;
      const y = Math.floor(index / 1000);
      chunk += [
        "0", "LINE", "5", (firstHandle + index).toString(16).toUpperCase(),
        "100", "AcDbEntity", "8", "0", "100", "AcDbLine",
        "10", x, "20", y, "30", "0", "11", x + 0.4, "21", y + 0.4, "31", "0", "",
      ].join("\n");
      if ((index + 1) % 4096 === 0) {
        writeSync(file, chunk);
        chunk = "";
      }
    }
    writeSync(file, chunk + "0\nENDSEC\n0\nEOF\n");
  } finally {
    closeSync(file);
  }
  console.log(`${path}: ${count} 要素`);
}
