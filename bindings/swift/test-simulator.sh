#!/bin/sh
# iOS のシミュレーターで、Swift の包みの適合試験を流す（決定 0225）。
# 使い方: sh bindings/swift/test-simulator.sh [シミュレーターの名前か UDID]
# 先に build-xcframework.sh で、iOS のシミュレーターの対象を含む XCFramework を作っておく。
# 包みと試験を swiftc でシミュレーターに向けて作り、`xcrun simctl spawn` で流す。シミュレーターの中からも、Mac のファイルを読める。
set -eu
root=$(cd "$(dirname "$0")/../.." && pwd)
swift=$root/bindings/swift
build=$swift/build/ios-simulator
framework=$swift/build/CZumen.xcframework
slice=$(find "$framework" -type d -path '*simulator*' -name 'ios-*' -maxdepth 1 | head -n 1)
[ -n "$slice" ] || { echo "XCFramework に iOS のシミュレーターの対象がない: $framework" >&2; exit 2; }

sdk=$(xcrun --sdk iphonesimulator --show-sdk-path)
target=$(uname -m)-apple-ios15.0-simulator
rm -rf "$build"
mkdir -p "$build"
xcrun --sdk iphonesimulator swiftc -sdk "$sdk" -target "$target" -O -parse-as-library -emit-library -static -module-name Zumen \
  -emit-module -emit-module-path "$build/Zumen.swiftmodule" -I "$slice/Headers" \
  "$swift"/Sources/Zumen/*.swift -o "$build/libZumen.a"
xcrun --sdk iphonesimulator swiftc -sdk "$sdk" -target "$target" -O -I "$build" -I "$slice/Headers" \
  "$swift/Sources/ZumenConformance/main.swift" -L "$build" -lZumen -L "$slice" -lzumen_abi1 \
  -o "$build/ZumenConformance"

# 渡されなければ、使える iPhone のシミュレーターの 1 つめ
device=${1:-$(xcrun simctl list devices available | sed -n 's/^ *iPhone[^(]*(\([0-9A-F-]\{36\}\)).*/\1/p' | head -n 1)}
[ -n "$device" ] || { echo "使える iPhone のシミュレーターがない" >&2; exit 2; }
xcrun simctl boot "$device" 2>/dev/null || true
xcrun simctl bootstatus "$device" -b >/dev/null
# simctl spawn は、SIMCTL_CHILD_ を付けた環境変数を、付けない名前で渡す
SIMCTL_CHILD_GITHUB_STEP_SUMMARY=${GITHUB_STEP_SUMMARY:-} xcrun simctl spawn "$device" "$build/ZumenConformance" "$root"
