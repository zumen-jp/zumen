#!/bin/sh
# 形式ごとに wasm を作り、何も符号化しないものとの大きさの差を出す（#6）。
set -eu
cd "$(dirname "$0")"
size() {
  cargo build -q --release --lib --target wasm32-unknown-unknown --no-default-features ${1:+--features "$1"}
  file=target/wasm32-unknown-unknown/release/zumen_bench_transfer.wasm
  printf '%s %s' "$(wc -c < "$file" | tr -d ' ')" "$(gzip -9c "$file" | wc -c | tr -d ' ')"
}
set -- $(size "")
base=$1 base_gzip=$2
printf 'format\tbytes\tincrease\tgzip_increase\n(none)\t%s\t0\t0\n' "$base"
for feature in json columnar flatbuffers arrow; do
  set -- $(size "$feature")
  printf '%s\t%s\t%s\t%s\n' "$feature" "$1" "$(($1 - base))" "$(($2 - base_gzip))"
done
