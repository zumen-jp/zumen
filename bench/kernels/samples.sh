#!/bin/sh
# 公開サンプルの IFC（samples/ifc）を、候補と試料ごとに別のプロセスで流し、15 分で打ち切る（#5）。
# パニックやアボートでも、ほかの組を流す。行は <出力のフォルダー>/<試料>.<候補>.tsv に書く。
# 使い方: ./samples.sh <出力のフォルダー> ./target/release/samples
#         ./samples.sh <出力のフォルダー> node --no-warnings wasi-samples.mjs target/wasm32-wasip1/release/samples.wasm
set -u
out=$1
shift
mkdir -p "$out"
: > "$out/aborts.tsv"
for file in $(find ../../samples/ifc -name '*.ifc' | sort); do
  name=$(echo "$file" | sed 's#^\.\./\.\./samples/ifc/##; s#/#__#g; s#\.ifc$##')
  for candidate in own manifold-rust csgrs; do
    perl -e 'alarm shift; exec @ARGV' 900 "$@" "$candidate" "$file" > "$out/$name.$candidate.tsv" 2>/dev/null
    code=$?
    if [ $code -ne 0 ]; then echo "$name	$candidate	abort:$code" >> "$out/aborts.tsv"; fi
  done
done
grep -h '^# ' "$out"/*.tsv
cat "$out/aborts.tsv"
