#!/bin/sh
# 候補と組ごとに別のプロセスで流し、60 秒で打ち切る。パニックやアボートでも、ほかの組を流す。
# 使い方: ./run.sh ./target/release/compare
#         ./run.sh node --no-warnings wasi.mjs target/wasm32-wasip1/release/compare.wasm
set -u
cases=$("$@" own | tail -n +2 | cut -f2)
echo "candidate	case	status	volume_error	bounds	closed	triangles	fingerprint	micros	section"
for candidate in own manifold-rust csgrs; do
  for case in $cases; do
    out=$(perl -e 'alarm shift; exec @ARGV' 60 "$@" "$candidate" "$case" 2>/dev/null)
    code=$?
    if [ $code -eq 0 ]; then echo "$out" | tail -n +2; else echo "$candidate	$case	abort:$code"; fi
  done
done
