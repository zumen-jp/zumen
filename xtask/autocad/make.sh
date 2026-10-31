#!/bin/sh
# samples/dwg の図面を、SSH でつなぐ Windows の AutoCAD で作り直す。使い方: make.sh [ホスト]（既定は home-win）
# Windows に AutoCAD 2027 と OpenSSH のサーバーが要る。作業のフォルダーは C:\zumen-work
set -eu
host=${1:-home-win}
here=$(cd "$(dirname "$0")" && pwd)
dest=$here/../../samples/dwg
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
ssh "$host" 'powershell -NoProfile -Command "Remove-Item -Recurse -Force C:\zumen-work -EA 0; New-Item -ItemType Directory C:\zumen-work\out | Out-Null"' 2>/dev/null
scp -q "$here/samples.lsp" "$here/run.ps1" "$host:C:/zumen-work/"
# 主題と、関数に渡す引数
for topic in "arcdim" "xclip" "layout" "minsert" "points 34 5 \"points-a\"" "points 99 -10 \"points-b\""; do
  name=$(echo "$topic" | sed -n 's/.*"\(.*\)"/\1/p')
  name=${name:-$topic}
  fn=${topic%% *}
  args=$(echo "$topic" | cut -s -d' ' -f2-)
  printf '(setvar "TRUSTEDPATHS" "C:\\\\zumen-work")\r\n(load "C:/zumen-work/samples.lsp")\r\n(zs-%s %s)\r\n(command "_.QUIT" "_Y")\r\n' "$fn" "$args" > "$tmp/$name.scr"
  scp -q "$tmp/$name.scr" "$host:C:/zumen-work/$name.scr"
  ssh "$host" "powershell -NoProfile -ExecutionPolicy Bypass -File C:\\zumen-work\\run.ps1 C:\\zumen-work\\$name.scr 150" 2>/dev/null
  scp -q "$host:C:/zumen-work/$name.out" "$tmp/$name.out"
  # QUIT は最後に「関数は取り消されました」を返すので、QUIT より前の誤りだけを見る
  if iconv -f UTF-16LE -t UTF-8 "$tmp/$name.out" | sed "/_.QUIT/,\$d" | grep -q "エラー"; then
    echo "$name: AutoCAD が誤りを返した（$tmp/$name.out）" >&2
    trap - EXIT
    exit 1
  fi
done
mkdir -p "$dest"
scp -q "$host:C:/zumen-work/out/*" "$dest/"
ls "$dest"
