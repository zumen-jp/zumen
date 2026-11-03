"""シーンの blob のリーダー（決定 0137・0195）。配置は schema/scene.toml にあり、セクションの定義は constants.py に生成する。

ヘッダーは `ZMSC`、メジャーバージョン（u16）、マイナーバージョン（u16）、塊の数（u32）、セクションの数（u32）で、続けてセクションごとに始まりと長さ（u32、
バイト）を置く。数はリトルエンディアン。知らないセクション（新しいマイナーバージョンで足したもの）は読み飛ばし、ないセクション（古いマイナーバージョン）は
空の列にする。メジャーバージョンが違えば読まない。
"""

from __future__ import annotations

import array
import struct
import sys
from types import MappingProxyType
from typing import Mapping, Union

from .constants import SCENE_MAGIC, SCENE_MAJOR, SCENE_SECTIONS

_HEADER = struct.Struct("<4sHHII")
_ENTRY = struct.Struct("<II")
_LITTLE_ENDIAN = sys.byteorder == "little"

for _section in SCENE_SECTIONS:
    if array.array(_section.format).itemsize != struct.calcsize("<" + _section.format):
        raise ImportError(f"セクション {_section.name} の値の大きさが、この環境の array と合わない")


class Scene:
    """読んだシーン"""

    def __init__(self, major: int, minor: int, chunks: int, sections: int, columns: Mapping[str, memoryview]) -> None:
        self.major = major
        """メジャーバージョン"""
        self.minor = minor
        """マイナーバージョン"""
        self.chunks = chunks
        """塊の数"""
        self.sections = sections
        """blob にあるセクションの数。このラッパーが知らないセクションも数える"""
        self.columns = columns
        """セクションごとの列（1 次元の memoryview）。行ごとに width 個の値を並べる"""

    def rows(self, name: str) -> int:
        """セクションの行の数"""
        for section in SCENE_SECTIONS:
            if section.name == name:
                return len(self.columns[name]) // section.width
        raise KeyError(f"知らないセクション: {name}")

    def text(self, start: int, length: int) -> str:
        """strings のセクションから、始まりと長さ（バイト）で文字列を引く"""
        strings = self.columns["strings"]
        if start + length > len(strings):
            raise IndexError("strings のセクションの外を指す")
        return bytes(strings[start : start + length]).decode("utf-8")


def _column(view: memoryview, fmt: str, start: int, length: int) -> memoryview:
    """列を作る。環境がリトルエンディアンなら写さずに見る"""
    part = view[start : start + length]
    if _LITTLE_ENDIAN:
        return part.cast(fmt)
    values = array.array(fmt, part.tobytes())
    values.byteswap()
    return memoryview(values)


def read_scene(blob: Union[bytes, bytearray, memoryview]) -> Scene:
    """シーンの blob を読む。列は blob のバイト列を共有するので、blob を書き換えない"""
    view = memoryview(blob).cast("B")
    if len(view) < _HEADER.size:
        raise ValueError("シーンの blob でない")
    magic, major, minor, chunks, sections = _HEADER.unpack_from(view, 0)
    if magic != SCENE_MAGIC:
        raise ValueError("シーンの blob でない")
    if major != SCENE_MAJOR:
        raise ValueError(f"シーンのメジャーバージョンが {major}（このラッパーは {SCENE_MAJOR}）")
    if _HEADER.size + _ENTRY.size * sections > len(view):
        raise ValueError("セクションの表が blob に収まらない")
    columns = {}
    for index, section in enumerate(SCENE_SECTIONS):
        if index >= sections:
            columns[section.name] = memoryview(array.array(section.format))
            continue
        start, length = _ENTRY.unpack_from(view, _HEADER.size + _ENTRY.size * index)
        row = struct.calcsize("<" + section.format) * section.width
        if start + length > len(view) or length % row != 0:
            raise ValueError(f"セクション {section.name} の始まりか長さが合わない")
        columns[section.name] = _column(view, section.format, start, length)
    return Scene(major, minor, chunks, sections, MappingProxyType(columns))
