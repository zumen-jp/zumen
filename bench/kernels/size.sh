#!/bin/sh
# 候補を 1 つずつ入れて wasm32-unknown-unknown の cdylib を作り、ブーリアンのない基準との差を出す。
# brotli の大きさは、Node.js の zlib（品質 11）で測る
set -eu
target=target/wasm32-unknown-unknown/release/zumen_bench_kernels.wasm
size() {
  node -e 'const z=require("node:zlib"),b=require("node:fs").readFileSync(process.argv[1]);console.log(b.length+"\t"+z.brotliCompressSync(b,{params:{[z.constants.BROTLI_PARAM_QUALITY]:11}}).length)' "$1"
}
echo "candidate	bytes	brotli"
for feature in none own manifold csgrs; do
  if [ "$feature" = none ]; then flags="--no-default-features"; else flags="--no-default-features --features $feature"; fi
  if cargo build -q --release --lib --target wasm32-unknown-unknown $flags 2>/dev/null; then
    echo "$feature	$(size $target)"
  else
    echo "$feature	build failed"
  fi
done
