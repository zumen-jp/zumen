"""シーンの blob の読み手（決定 0137・0195）。配置は schema/scene.toml にあり、区画の定義は constants.py に生成する。

見出しは `ZMSC`、大版（u16）、小版（u16）、塊の数（u32）、区画の数（u32）で、続けて区画ごとに始まりと長さ（u32、
バイト）を置く。数はリトルエンディアン。知らない区画（新しい小版で足したもの）は読み飛ばし、ない区画（古い小版）は
空の列にする。大版が違えば読まない。
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
        raise ImportError(f"区画 {_section.name} の値の大きさが、この環境の array と合わない")


class Scene:
    """読んだシーン"""

    def __init__(self, major: int, minor: int, chunks: int, sections: int, columns: Mapping[str, memoryview]) -> None:
        self.major = major
        """大版"""
        self.minor = minor
        """小版"""
        self.chunks = chunks
        """塊の数"""
        self.sections = sections
        """blob にある区画の数。この包みが知らない区画も数える"""
        self.columns = columns
        """区画ごとの列（1 次元の memoryview）。行ごとに width 個の値を並べる"""

    def rows(self, name: str) -> int:
        """区画の行の数"""
        for section in SCENE_SECTIONS:
            if section.name == name:
                return len(self.columns[name]) // section.width
        raise KeyError(f"知らない区画: {name}")

    def text(self, start: int, length: int) -> str:
        """strings の区画から、始まりと長さ（バイト）で文字列を引く"""
        strings = self.columns["strings"]
        if start + length > len(strings):
            raise IndexError("strings の区画の外を指す")
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
        raise ValueError(f"シーンの大版が {major}（この包みは {SCENE_MAJOR}）")
    if _HEADER.size + _ENTRY.size * sections > len(view):
        raise ValueError("区画の表が blob に収まらない")
    columns = {}
    for index, section in enumerate(SCENE_SECTIONS):
        if index >= sections:
            columns[section.name] = memoryview(array.array(section.format))
            continue
        start, length = _ENTRY.unpack_from(view, _HEADER.size + _ENTRY.size * index)
        row = struct.calcsize("<" + section.format) * section.width
        if start + length > len(view) or length % row != 0:
            raise ValueError(f"区画 {section.name} の始まりか長さが合わない")
        columns[section.name] = _column(view, section.format, start, length)
    return Scene(major, minor, chunks, sections, MappingProxyType(columns))
