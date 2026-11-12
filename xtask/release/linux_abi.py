"""GNU Linux の配布物が glibc 2.28 の下限を超えないことを、実行せずに確かめる（決定 0014）。"""

import argparse
import json
import os
import re
import struct
import sys
from pathlib import Path

FLOOR = (2, 28)
MACHINES = {"x86_64-unknown-linux-gnu": (62, "x86_64"), "aarch64-unknown-linux-gnu": (183, "aarch64")}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def glibc_versions(binary, machine):
    """ELF64 の節と GNU の version requirements を読む。gABI の ELF Header・Section Header、LSB の Symbol Versioning。"""
    def part(offset, size):
        require(0 <= offset <= len(binary) and 0 <= size <= len(binary) - offset, "ELF の範囲が不正")
        return binary[offset:offset + size]

    def unpack(format_, offset):
        return struct.unpack(format_, part(offset, struct.calcsize(format_)))

    require(part(0, 7) == b"\x7fELF\x02\x01\x01", "配布対象の little-endian ELF64 ではない")
    require(unpack("<HH", 16) in ((2, machine), (3, machine)), "ELF の実行形式かアーキテクチャが違う")
    offset, = unpack("<Q", 40)
    stride, count = unpack("<HH", 58)
    require(offset >= 64 and stride == 64 and count > 0, "ELF の節の一覧がないか形式が違う")
    part(offset, stride * count)
    sections = [unpack("<IIQQQQIIQQ", offset + i * stride) for i in range(count)]
    # SHT_GNU_verneed の sh_link は、その節の名前が参照する文字列の節を指す。
    needs = [section for section in sections if section[1] == 0x6ffffffe]
    require(len(needs) == 1, "ELF のバージョン要求の節がないか複数ある")
    need = needs[0]
    require(0 < need[6] < count and sections[need[6]][1] == 3, "ELF のバージョン要求の文字列の節が不正")
    strings = part(sections[need[6]][4], sections[need[6]][5])
    entries = part(need[4], need[5])

    def name(index):
        require(0 <= index < len(strings), "ELF のバージョンの名前の範囲が不正")
        end = strings.find(b"\0", index)
        require(end >= index, "ELF のバージョンの名前に終端がない")
        try:
            return strings[index:end].decode("ascii")
        except UnicodeDecodeError as error:
            raise ValueError("ELF のバージョンの名前が ASCII ではない") from error

    def entry(format_, position):
        size = struct.calcsize(format_)
        require(0 <= position <= len(entries) - size, "ELF のバージョン要求の範囲が不正")
        return struct.unpack_from(format_, entries, position)

    require(0 < need[7] <= len(entries) // 16, "ELF のバージョン要求の件数が不正")
    versions, position = set(), 0
    for index in range(need[7]):
        version, count, library, auxiliary, following = entry("<HHIII", position)
        require(version == 1 and 0 < count <= len(entries) // 16 and auxiliary >= 16,
                "ELF のバージョン要求の形式が不正")
        name(library)
        cursor = position + auxiliary
        for item in range(count):
            _, _, _, value, next_ = entry("<IHHII", cursor)
            value = name(value)
            if value.startswith("GLIBC_"):
                require(re.fullmatch(r"GLIBC_[0-9]+(?:\.[0-9]+)+", value), "ELF に数値でない GLIBC の要求がある")
                versions.add(value)
            require((next_ >= 16) if item + 1 < count else next_ == 0, "ELF のバージョン要求の連結が不正")
            cursor += next_
        require((following >= 16) if index + 1 < need[7] else following == 0, "ELF のバージョン要求の連結が不正")
        position += following
    require(versions, "ELF に GLIBC のバージョン要求がない")
    return sorted(versions, key=lambda value: tuple(map(int, value[6:].split("."))))


def verify_linux(binary, target):
    machine = MACHINES.get(target["target"])
    if machine is None:
        require(not target["target"].endswith("-linux-gnu"), "検査に対応していない GNU Linux の target")
        return []
    require(target["wheel"] == f"manylinux_2_28_{machine[1]}", "Linux wheel のタグが glibc 2.28 の下限と違う")
    versions = glibc_versions(binary, machine[0])
    require(all(tuple(map(int, version[6:].split("."))) <= FLOOR for version in versions),
            "ELF が glibc 2.28 の下限を超えるバージョンを要求する: " + versions[-1])
    return versions


def verify_environment(target):
    machine = MACHINES.get(target["target"])
    if machine is None:
        require(not target["target"].endswith("-linux-gnu"), "検査に対応していない GNU Linux の target")
        return
    require(sys.platform == "linux" and os.confstr("CS_GNU_LIBC_VERSION") == "glibc 2.28",
            "GNU Linux の配布物は manylinux_2_28 の環境で作り、実行を確かめる")
    require(os.uname().machine == machine[1], "Linux のビルド・実行のアーキテクチャが対象と違う")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--rid", choices=("linux-x64", "linux-arm64"), required=True)
    parser.add_argument("binary", type=Path, nargs="+")
    args = parser.parse_args()
    targets = json.loads(Path(__file__).with_name("targets.json").read_text(encoding="utf-8"))
    target = next(target for target in targets if target["rid"] == args.rid)
    for path in args.binary:
        versions = verify_linux(path.read_bytes(), target)
        print(f"{path.name}: glibc 2.28 以下（最大の要求 {versions[-1]}）")
