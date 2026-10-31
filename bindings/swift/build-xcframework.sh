#!/bin/sh
# ネイティブのライブラリ（静的）とヘッダーを、XCFramework にまとめる（決定 0225）。
# 使い方: sh bindings/swift/build-xcframework.sh [debug|release] [Rust の対象 ...]
# 対象を渡さなければ、macOS（arm64・x86_64）・iOS（arm64）・iOS のシミュレーター（arm64・x86_64）を作る。
# 出力は bindings/swift/build/CZumen.xcframework。
set -eu
profile=${1:-release}
[ $# -gt 0 ] && shift
targets=${*:-aarch64-apple-darwin x86_64-apple-darwin aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios}
root=$(cd "$(dirname "$0")/../.." && pwd)
out=$root/bindings/swift/build
case $profile in
  release) flag=--release ;;
  debug) flag= ;;
  *) echo "版の作り方は debug か release: $profile" >&2; exit 2 ;;
esac

for target in $targets; do
  cargo build --manifest-path "$root/Cargo.toml" --package zumen-abi --locked $flag --target "$target"
done

rm -rf "$out/headers" "$out/lib" "$out/CZumen.xcframework"
mkdir -p "$out/headers"
cp "$root/bindings/c/include/zumen.h" "$root/bindings/c/include/zumen_scene.h" "$out/headers/"
cat > "$out/headers/module.modulemap" <<'MAP'
module CZumen {
    header "zumen.h"
    header "zumen_scene.h"
    export *
}
MAP

# プラットフォームごとに、アーキテクチャーの違うライブラリを 1 つにする
set --
for platform in macos ios ios-simulator; do
  libraries=
  for target in $targets; do
    case $target in
      *-apple-darwin) kind=macos ;;
      *-apple-ios-sim | x86_64-apple-ios) kind=ios-simulator ;;
      *-apple-ios) kind=ios ;;
      *) echo "Apple の対象でない: $target" >&2; exit 2 ;;
    esac
    [ "$kind" = "$platform" ] && libraries="$libraries $root/target/$target/$profile/libzumen_abi1.a"
  done
  [ -z "$libraries" ] && continue
  mkdir -p "$out/lib/$platform"
  # shellcheck disable=SC2086
  lipo -create $libraries -output "$out/lib/$platform/libzumen_abi1.a"
  set -- "$@" -library "$out/lib/$platform/libzumen_abi1.a" -headers "$out/headers"
done
xcodebuild -create-xcframework "$@" -output "$out/CZumen.xcframework"
