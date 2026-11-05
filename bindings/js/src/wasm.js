// パッケージに入れた wasm の URL（決定 0268）。読み込みは利用者の fetch・loadModules に任せる。
// bundler がファイルを見つけられるように、new URL の第 1 引数は静的な相対パスにする。
export const wasmUrls = Object.freeze({
  all: new URL("../zumen_abi1.wasm", import.meta.url).href,
  drawing: new URL("../zumen_abi1_drawing.wasm", import.meta.url).href,
  bim: new URL("../zumen_abi1_bim.wasm", import.meta.url).href,
  civil: new URL("../zumen_abi1_civil.wasm", import.meta.url).href,
  pointcloud: new URL("../zumen_abi1_pointcloud.wasm", import.meta.url).href,
});
