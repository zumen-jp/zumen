"""シーンの blob のリーダー（決定 0213・0195）。配置は schema/scene3d.toml にあり、セクションの定義は constants.py に生成する。

ヘッダーは `ZMS3`、メジャーバージョン（u16）、マイナーバージョン（u16）、塊の数（u32）、セクションの数（u32）で、続けてセクションごとに始まりと長さ（u32、
バイト）を置く。数はリトルエンディアン。知らないセクションは読み飛ばす。基本の 20 セクションがないときは
読まない。メジャーバージョンが違う場合も読まない。
"""

from __future__ import annotations

import array
import struct
import sys
from types import MappingProxyType
from typing import Mapping, Union

from .scene3d_layout import SCENE3D_MAGIC, SCENE3D_MAJOR, SCENE3D_SECTIONS

_HEADER = struct.Struct("<4sHHII")
_ENTRY = struct.Struct("<II")
_LITTLE_ENDIAN = sys.byteorder == "little"

for _section in SCENE3D_SECTIONS:
    if array.array(_section.format).itemsize != struct.calcsize("<" + _section.format):
        raise ImportError(f"セクション {_section.name} の値の大きさが、この環境の array と合わない")


class Scene3D:
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
        for section in SCENE3D_SECTIONS:
            if section.name == name:
                return len(self.columns[name]) // section.width
        raise KeyError(f"知らないセクション: {name}")

    def text(self, start: int, length: int) -> str:
        """strings のセクションから、始まりと長さ（バイト）で文字列を引く"""
        strings = self.columns["strings"]
        if start < 0 or length < 0 or start + length > len(strings):
            raise IndexError("strings のセクションの外を指す")
        return bytes(strings[start : start + length]).decode("utf-8")


    def _u32_row(self, name: str, row: int, component: int = 0) -> int:
        width = next(s.width for s in SCENE3D_SECTIONS if s.name == name)
        if not isinstance(row, int) or row < 0 or row >= self.rows(name) or component < 0 or component >= width:
            raise IndexError("3D シーンの行の範囲")
        return self.columns[name][row * width + component]

    def element_id(self, row: int) -> str:
        """元の建物の要素 ID"""
        return self.text(self._u32_row("element_ids", row), self._u32_row("element_ids", row, 1))

    def instance_mesh(self, row: int) -> int:
        """インスタンスが指すメッシュの行"""
        value = self._u32_row("instance_meshes", row)
        if value >= self.rows("mesh_vertices"):
            raise IndexError("メッシュの参照")
        return value

    def instance_element(self, row: int) -> int:
        """インスタンスが指す要素の行"""
        value = self._u32_row("instance_elements", row)
        if value >= self.rows("element_ids"):
            raise IndexError("要素の参照")
        return value

    def instance_color(self, row: int) -> int:
        """インスタンスが指す色の行"""
        value = self._u32_row("instance_colors", row)
        if value >= self.rows("color_rgba"):
            raise IndexError("色の参照")
        return value


def _column(view: memoryview, fmt: str, start: int, length: int) -> memoryview:
    """列を作る。環境がリトルエンディアンなら写さずに見る"""
    part = view[start : start + length]
    if _LITTLE_ENDIAN:
        return part.cast(fmt)
    values = array.array(fmt, part.tobytes())
    values.byteswap()
    return memoryview(values)


def read_scene3d(blob: Union[bytes, bytearray, memoryview]) -> Scene3D:
    """シーンの blob を読む。列は blob のバイト列を共有するので、blob を書き換えない"""
    view = memoryview(blob).cast("B")
    if len(view) > 512 * 1024 * 1024:
        raise ValueError("3D シーンのバイト数の上限")
    if len(view) < _HEADER.size:
        raise ValueError("シーンの blob でない")
    magic, major, minor, chunks, sections = _HEADER.unpack_from(view, 0)
    if magic != SCENE3D_MAGIC:
        raise ValueError("シーンの blob でない")
    if major != SCENE3D_MAJOR:
        raise ValueError(f"シーンのメジャーバージョンが {major}（このラッパーは {SCENE3D_MAJOR}）")
    if _HEADER.size + _ENTRY.size * sections > len(view):
        raise ValueError("セクションの表が blob に収まらない")
    if sections < len(SCENE3D_SECTIONS):
        raise ValueError("3D シーンの基本列がない")
    previous = _HEADER.size + _ENTRY.size * sections
    for index in range(sections):
        start, length = _ENTRY.unpack_from(view, _HEADER.size + _ENTRY.size * index)
        if start % 8 or start < previous or start + length > len(view):
            raise ValueError("3D シーンのセクションの範囲")
        previous = start + length
    columns = {}
    for index, section in enumerate(SCENE3D_SECTIONS):
        if index >= sections:
            columns[section.name] = memoryview(array.array(section.format))
            continue
        start, length = _ENTRY.unpack_from(view, _HEADER.size + _ENTRY.size * index)
        row = struct.calcsize("<" + section.format) * section.width
        if start + length > len(view) or length % row != 0:
            raise ValueError(f"セクション {section.name} の始まりか長さが合わない")
        columns[section.name] = _column(view, section.format, start, length)
    return Scene3D(major, minor, chunks, sections, MappingProxyType(columns))
