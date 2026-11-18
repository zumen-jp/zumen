"""R2007 より前の DWG のコード・ページの番号と、zumen が書いた DXF の文字を、AutoCAD 2027 の読みに照らす（#327）。

AutoCAD は図面を開くと端末のコード・ページに変えて保存するので、番号の違う DWG は AutoCAD では作れない。そこで次の順で
番号ごとの読みを見る。

1. ASCII だけの R12 の DXF を accoreconsole で R2000 の DWG にし、ヘッダーの番号を 30（ANSI_1252）に変えて
   （ヘッダーの CRC も直す）、zumen で TEXT を足して書き戻す。TEXT のバイト列は、1 バイトの 0xC0〜0xDF と、
   CP936・CP949・CP950・CP932・Johab の 2 文字ずつ。
2. その DWG の番号を 0〜70 に変えて accoreconsole で開き、R2018 の DWG（文字は UTF-16）に保存する。保存した DWG を
   zumen で読んだ TEXT が、番号を変えた DWG を zumen で読んだ TEXT と同じことを確かめる。zumen が割り当てのない
   バイト列として落とす TEXT は比べない。

DXF は、$DWGCODEPAGE の名前ごとに、表の文字を zumen で足して書き戻した R2000 の DXF を開き、R2018 に保存した DWG の
文字列が足した文字列と同じことを確かめる。

AutoCAD が保存した図面とコンソールの出力はログイン名を含むので、作業のフォルダーは終わりに消す（--keep を除く）。
"""

import argparse
import codecs
import io
import json
import re
import shutil
import struct
import subprocess
import sys
import tempfile
from pathlib import Path

import ezdxf

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))
from check_autocad import decode, find_exe, mark, markers  # noqa: E402
from make import Zumen  # noqa: E402

# 番号を変えて開く範囲。ODA の列挙は 1〜44 で、45 からは表にない番号として確かめる
NUMBERS = range(0, 71)
# ヘッダーの CRC に重ねる値（セクションの記録の数ごと。ODS 5.4.1 の 3）
MASK = {3: 0xA598, 4: 0x8101, 5: 0x3CC4, 6: 0x8461}
# DWG に足す TEXT のバイト列
SAMPLES = {
    "single": bytes(range(0xC0, 0xE0)),
    "cp936": bytes.fromhex("D6D0CEC4"),
    "cp949": bytes.fromhex("C7D1B1DB"),
    "cp950": bytes.fromhex("A4A4A4E5"),
    "cp932": bytes.fromhex("93FA967B"),
    "johab": bytes.fromhex("D0658A82"),
}
# DXF で照らす名前と、文字を選ぶ Windows のコード・ページ
NAMES = {f"ANSI_{p}": p for p in (874, 932, 936, 949, 950, 1250, 1251, 1252, 1253, 1254, 1255, 1256, 1257, 1258, 1361)}
NAMES |= {f"DOS{p}": p for p in (437, 850, 852, 855, 857, 860, 861, 863, 864, 865, 866, 869, 932)}
NAMES |= {"ISO8859-1": 1252, "ISO8859-2": 1250, "ISO8859-7": 1253, "ISO8859-9": 1254, "BIG5": 950, "KSC5601": 949, "GB2312": 936, "JOHAB": 1361}
DOUBLE = {932: "日本語の図面", 936: "中文图纸", 949: "한국어 도면", 950: "中文圖紙", 1361: "한국어 도면"}


def crc16(data):
    crc = 0
    for byte in data:
        crc ^= byte
        for _ in range(8):
            crc = (crc >> 1) ^ 0xA001 if crc & 1 else crc >> 1
    return crc


def renumber(data, number):
    """R2000 の DWG のヘッダーのコード・ページの番号（0x13）を変え、ヘッダーの CRC を直す"""
    out = bytearray(data)
    struct.pack_into("<H", out, 0x13, number)
    count = struct.unpack_from("<I", out, 0x15)[0]
    end = 0x19 + 9 * count
    if crc16(data[:end]) ^ MASK[count] != struct.unpack_from("<H", data, end)[0]:
        raise RuntimeError("ヘッダーの CRC が合わない")
    struct.pack_into("<H", out, end, crc16(bytes(out[:end])) ^ MASK[count])
    return bytes(out)


def characters(page):
    """コード・ページで表せる文字の列"""
    if page in DOUBLE:
        return DOUBLE[page]
    found = ""
    for byte in range(0xA0, 0x100):
        try:
            text = codecs.code_page_decode(page, bytes([byte]), "strict", True)[0]
        except UnicodeDecodeError:
            continue
        if len(text) == 1 and text.isprintable() and not text.isspace() and not 0xE000 <= ord(text) <= 0xF8FF:
            found += text
    return found


class AutoCad:
    def __init__(self, exe, work, timeout):
        self.exe, self.work, self.timeout = exe, work, timeout

    def save(self, name, data, suffix, version):
        """図面を開き、version（2000・2018）の DWG に保存する。保存した DWG のバイト列か None"""
        folder = self.work / name
        folder.mkdir(parents=True)
        (folder / f"drawing{suffix}").write_bytes(data)
        out = str(folder / "out.dwg").replace("\\", "/")
        lines = [mark("open", '(getvar "DWGNAME")'), '(setvar "FILEDIA" 0)', f'(command "_.SAVEAS" "{version}" "{out}")', mark("done"), "_.QUIT _Y"]
        (folder / "run.scr").write_text("".join(line + "\r\n" for line in lines), encoding="ascii", newline="")
        with open(folder / "console.log", "wb") as log:
            command = [str(self.exe), "/i", str(folder / f"drawing{suffix}"), "/s", str(folder / "run.scr")]
            try:
                subprocess.run(command, cwd=folder, stdin=subprocess.DEVNULL, stdout=log, stderr=subprocess.STDOUT, timeout=self.timeout)
            except subprocess.TimeoutExpired:
                return None
        names = [name_ for name_, _ in markers(decode((folder / "console.log").read_bytes()))]
        path = folder / "out.dwg"
        return path.read_bytes() if "done" in names and path.exists() else None


def texts(z, key, data):
    """zumen が読んだ TEXT の文字列。開けなければ None"""
    z.push(key, data)
    status, _ = z.call({"op": "open", "input": key})
    if status != "ok":
        return None
    _, response = z.call({"op": "entities", "doc": key, "fields": ["kind", "geometry"]})
    return [item.get("geometry", {}).get("content") for item in response["result"]["items"] if item.get("kind") == "text"]


def layer_zero(z, key):
    _, tables = z.call({"op": "tables", "doc": key})
    return next(item["id"] for item in tables["result"]["layers"] if item.get("name") == "0")


def add(layer, content, index=0):
    return {"kind": "add", "place": "space/0", "layer_id": layer, "geometry": {"shape": "text", "content": content, "position": [1, 2 + 5 * index], "height": 2.5, "rotation": 0}}


def blank(version):
    doc = ezdxf.new(version)
    doc.modelspace().add_text("BASE", dxfattribs={"height": 2.5})
    stream = io.StringIO()
    doc.write(stream)
    return stream.getvalue().encode("ascii")


def check_dwg(z, acad):
    """番号ごとの不一致。値は番号 → 不一致の説明"""
    base = acad.save("base", blank("R12"), ".dxf", "2000")
    if base is None:
        raise RuntimeError("AutoCAD が R2000 の DWG を保存しない")
    z.push("base.dwg", renumber(base, 30))
    status, response = z.call({"op": "open", "input": "base.dwg"})
    if status != "ok":
        raise RuntimeError(f"zumen が R2000 の DWG を開かない: {response}")
    layer = layer_zero(z, "base.dwg")
    edits = [add(layer, f"{key}:{data.decode('cp1252')}", i) for i, (key, data) in enumerate(SAMPLES.items())]
    status, response = z.call({"op": "apply", "doc": "base.dwg", "edits": edits})
    if status != "ok":
        raise RuntimeError(f"zumen が TEXT を足さない: {response}")
    written = z.take(response["blobs"][0]["id"])
    failures = {}
    for number in NUMBERS:
        data = renumber(written, number)
        ours = texts(z, f"n{number}.dwg", data)
        saved = acad.save(f"n{number:02d}", data, ".dwg", "2018")
        theirs = texts(z, f"a{number}.dwg", saved) if saved else None
        if ours is None or theirs is None:
            failures[number] = "開けない"
            continue
        wrong = [text for text in ours if text not in theirs]
        if wrong or not any(not text.isascii() for text in ours):
            failures[number] = {"zumen": ours, "autocad": theirs}
    return failures


def check_dxf(z, acad):
    """名前ごとの不一致"""
    failures = {}
    for index, (name, page) in enumerate(NAMES.items()):
        data = re.sub(rb"(\n\$DWGCODEPAGE\r?\n *3\r?\n)[^\n]*\n", lambda m: m[1] + name.encode() + b"\n", blank("R2000"))
        key = f"x{index}.dxf"
        z.push(key, data)
        z.call({"op": "open", "input": key})
        text = characters(page)
        status, response = z.call({"op": "apply", "doc": key, "edits": [add(layer_zero(z, key), text)]})
        if status != "ok":
            failures[name] = f"書き戻せない: {response}"
            continue
        saved = acad.save(f"x{index:02d}", z.take(response["blobs"][0]["id"]), ".dxf", "2018")
        theirs = texts(z, f"y{index}.dwg", saved) if saved else None
        if theirs != ["BASE", text]:
            failures[name] = {"added": text, "autocad": theirs}
    return failures


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--cli", required=True, help="zumen の CLI")
    parser.add_argument("--exe", help="accoreconsole.exe の場所（既定は環境変数 ZUMEN_ACCORECONSOLE か、既定の導入先）")
    parser.add_argument("--timeout", type=float, default=120, help="1 つの図面に待つ秒数（既定 120）")
    parser.add_argument("--keep", help="AutoCAD の作業のフォルダーを、ここに残す（リポジトリの外の空のフォルダー）。図面とコンソールの出力はログイン名を含む")
    args = parser.parse_args()
    exe = find_exe(args.exe)
    if exe is None:
        print("accoreconsole.exe が見つからない", file=sys.stderr)
        return 2
    work = Path(args.keep).resolve() if args.keep else Path(tempfile.mkdtemp(prefix="zumen-codepage-"))
    z = Zumen(args.cli)
    try:
        acad = AutoCad(exe, work, args.timeout)
        dwg, dxf = check_dwg(z, acad), check_dxf(z, acad)
    finally:
        z.close()
        if not args.keep:
            shutil.rmtree(work, ignore_errors=True)
    print(f"DWG の番号 {len(NUMBERS)} のうち不一致 {len(dwg)}、DXF の名前 {len(NAMES)} のうち不一致 {len(dxf)}")
    for key, value in [*dwg.items(), *dxf.items()]:
        print(f"  {key}: {json.dumps(value, ensure_ascii=False)}")
    return 1 if dwg or dxf else 0


if __name__ == "__main__":
    sys.exit(main())
