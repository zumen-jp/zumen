#!/bin/sh
# Android のエミュレーターで、Java の包みの適合試験を流す（決定 0224）。
# 使い方: sh bindings/java/android/run.sh <置き場所>
# 置き場所には、classes.zip（d8 で作った包みと試験と JNA の dex）、libjnidispatch.so、libzumen_abi1.so を置く。
# dalvikvm ではなく app_process で流す。シェルの利用者は dalvik-cache に書けないため。
# java.library.path は渡さない。システムのライブラリ（SHA-256 に使う libjavacrypto）を探せなくなるため。
set -eu
local=${1:?使い方: sh bindings/java/android/run.sh <置き場所>}
remote=/data/local/tmp/zumen

adb logcat -c
adb shell rm -rf "$remote"
adb shell mkdir -p "$remote/repo/bindings/conformance"
adb push "$local/classes.zip" "$local/libjnidispatch.so" "$local/libzumen_abi1.so" "$remote/" >/dev/null
adb push bindings/conformance/cases bindings/conformance/hashes.txt "$remote/repo/bindings/conformance/" >/dev/null
adb push samples "$remote/repo/" >/dev/null

# 終了の状態を、adb の版によらずに受け取るため、最後の行に書かせる
output=$(adb shell "cd $remote && CLASSPATH=$remote/classes.zip app_process \
  -Djna.boot.library.path=$remote -Djna.nosys=true \
  $remote dev.zumen.Conformance $remote/libzumen_abi1.so $remote/repo; echo \"exit=\$?\"" | tr -d '\r')
echo "$output"
summary=$(echo "$output" | grep '^包みを通した試験の組' || true)
if [ -n "${GITHUB_STEP_SUMMARY:-}" ] && [ -n "$summary" ]; then
  printf '### Java の包みの適合試験（Android のエミュレーター）\n\n%s\n' "$summary" >> "$GITHUB_STEP_SUMMARY"
fi
if ! echo "$output" | grep -q '^exit=0$'; then
  # 落ちた理由（ART・リンカー・SELinux）を、ログに残す
  adb logcat -d -t 300 | tr -d '\r'
  exit 1
fi
