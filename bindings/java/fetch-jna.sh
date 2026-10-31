#!/bin/sh
# JNA の jar（JVM）と aar（Android）を Maven Central から取り、SHA-256 を確かめる（決定 0224）。
# 版は pom.xml の依存と同じにする。使い方: sh bindings/java/fetch-jna.sh <置き場所>
set -eu
version=5.19.1
jar_sha256=4fb141dd8ef6b0585ffceea4bc49602fbc6312fa977e2c488794ea3e6aafecae
aar_sha256=b57125cb7d16253f0d65a80f7d3a4c3664effa711b8bdbb7f87fb572ce1624ed
out=${1:?使い方: sh bindings/java/fetch-jna.sh <置き場所>}
mkdir -p "$out"

sha256() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | cut -d ' ' -f 1
  else
    shasum -a 256 "$1" | cut -d ' ' -f 1
  fi
}

fetch() {
  extension=$1
  expected=$2
  file="$out/jna.$extension"
  if [ -f "$file" ] && [ "$(sha256 "$file")" = "$expected" ]; then
    return
  fi
  curl -fsSL --retry 3 -o "$file.part" "https://repo1.maven.org/maven2/net/java/dev/jna/jna/$version/jna-$version.$extension"
  actual=$(sha256 "$file.part")
  if [ "$actual" != "$expected" ]; then
    rm -f "$file.part"
    echo "jna-$version.$extension の SHA-256 が $actual（期待は $expected）" >&2
    exit 1
  fi
  mv "$file.part" "$file"
}

fetch jar "$jar_sha256"
fetch aar "$aar_sha256"
