#!/bin/sh
# IFC の試料を、候補と試料ごとに別のプロセスで流し、15 分で打ち切る（#5）。
# パニックやアボートでも、ほかの組を流す。行は <出力のフォルダー>/<試料>.<候補>.tsv に書く。
# 試料のフォルダーは環境変数 SAMPLES で渡す（既定は公開サンプルの ../../samples/ifc）。手元の試料（ZUMEN_SAMPLES）
# は、SAMPLES="$ZUMEN_SAMPLES/ifc" のように渡す。候補は環境変数 CANDIDATES で絞れる（既定はすべて）。
# 使い方: ./samples.sh <出力のフォルダー> ./target/release/samples
#         ./samples.sh <出力のフォルダー> node --no-warnings wasi-samples.mjs target/wasm32-wasip1/release/samples.wasm
set -u
out=$1
shift
dir=${SAMPLES:-../../samples/ifc}
mkdir -p "$out"
: > "$out/aborts.tsv"
find "$dir" -iname '*.ifc' | sort | while IFS= read -r file; do
  name=$(printf '%s' "${file#"$dir"/}" | sed 's#/#__#g; s#\.[iI][fF][cC]$##')
  for candidate in ${CANDIDATES:-own manifold-rust csgrs}; do
    perl -e 'alarm shift; exec @ARGV' 900 "$@" "$candidate" "$file" < /dev/null > "$out/$name.$candidate.tsv" 2>/dev/null
    code=$?
    if [ $code -ne 0 ]; then echo "$name	$candidate	abort:$code" >> "$out/aborts.tsv"; fi
  done
done
grep -h '^# ' "$out"/*.tsv
cat "$out/aborts.tsv"
