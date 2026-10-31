"""zumen の C ABI を、標準ライブラリの ctypes で呼ぶ薄い包み（決定 0011・0195）。

ネイティブのライブラリ（zumen_abi1.dll・libzumen_abi1.so・libzumen_abi1.dylib）を読み込み、境界の関数
（schema/boundary.toml）を、セッションのメソッドにして出す。要求と応答の型は messages.py にある。
ctypes は、ネイティブの関数を呼ぶ間 GIL を手放すので、ほかのスレッドから cancel を呼べる。
"""

from __future__ import annotations

import ctypes
import json
import os
import struct
import sys
from typing import Any, Mapping, Optional, Union

from .constants import (
    ABI_VERSION,
    FUNCTIONS,
    SCENE_MAJOR,
    SCENE_MINOR,
    SCENE_SECTIONS,
    SCENE_VALUES,
    SCHEMA_MAJOR,
    SCHEMA_MINOR,
    STATUS,
    VERSION,
    Buf,
)
from .scene import Scene, read_scene

__all__ = [
    "ABI_VERSION",
    "SCENE_MAJOR",
    "SCENE_MINOR",
    "SCENE_SECTIONS",
    "SCENE_VALUES",
    "SCHEMA_MAJOR",
    "SCHEMA_MINOR",
    "STATUS",
    "VERSION",
    "Scene",
    "Session",
    "Zumen",
    "ZumenError",
    "load",
    "read_scene",
]

BytesLike = Union[str, bytes, bytearray, memoryview, None]


class ZumenError(Exception):
    """境界の関数が ok でない状態を返した"""

    def __init__(self, operation: str, code: int, data: Optional[bytes]) -> None:
        self.operation = operation
        """呼んだ境界の関数"""
        self.code = code
        """状態コード"""
        self.status = STATUS.get(code, f"unknown({code})")
        """状態コードの名前"""
        self.data = data
        """応答の JSON。書かれなかったときは None"""
        super().__init__(f"{operation} が {self.status} を返した")

    @property
    def response(self) -> Optional[Any]:
        """応答。書かれなかったときは None"""
        return None if self.data is None else json.loads(self.data)


def _to_bytes(value: BytesLike) -> bytes:
    if value is None:
        return b""
    if isinstance(value, str):
        return value.encode("utf-8")
    if isinstance(value, bytes):
        return value
    if isinstance(value, (bytearray, memoryview)):
        return bytes(value)
    raise TypeError("バイト列か文字列を渡す")


def _dumps(value: Any) -> bytes:
    return json.dumps(value, ensure_ascii=False, separators=(",", ":")).encode("utf-8")


class _Core:
    """読み込んだライブラリと、境界の関数"""

    def __init__(self, library: ctypes.CDLL) -> None:
        missing = []
        for name, (restype, argtypes) in FUNCTIONS.items():
            try:
                function = getattr(library, name)
            except AttributeError:
                missing.append(name)
                continue
            function.restype = restype
            function.argtypes = argtypes
        if missing:
            raise OSError(f"zumen のライブラリでない（ない関数: {', '.join(missing)}）")
        version = library.zm_abi_version()
        if version != ABI_VERSION:
            raise OSError(f"ABI の大版が {version}（この包みは {ABI_VERSION}）")
        self.lib = library

    def with_buf(self, fn: Any) -> tuple:
        """zm_buf を用意して fn に渡し、書かれたバイト列を写して返す"""
        buf = Buf()
        code = fn(ctypes.byref(buf))
        try:
            if buf.ptr:
                data: Optional[bytes] = ctypes.string_at(buf.ptr, buf.len)
            else:
                data = b"" if code == 0 else None
        finally:
            self.lib.zm_free(ctypes.byref(buf))
        return code, data


class Session:
    """セッション。入力と、直前の呼び出しのバイナリの結果を持つ。使い終わったら close で閉じる。

    with の文で使うと、抜けるときに閉じる。1 つのセッションは、一度に 1 つのスレッドだけが使う。cancel だけは、
    どのスレッドからも呼べる。
    """

    def __init__(self, core: _Core, handle: Optional[int]) -> None:
        self._core = core
        self._handle = handle

    @property
    def closed(self) -> bool:
        """閉じたか"""
        return self._handle is None

    def push(self, name: BytesLike, data: BytesLike) -> None:
        """名前のついた入力に、バイト列を足す。同じ名前で何回か呼ぶと、後ろに足していく"""
        name = _to_bytes(name)
        data = _to_bytes(data)
        code = self._core.lib.zm_push(self._handle, name, len(name), data, len(data))
        if code != 0:
            raise ZumenError("zm_push", code, None)

    def call_bytes(self, request: BytesLike) -> bytes:
        """要求の JSON（文字列かバイト列）を渡し、応答の JSON のバイト列を受け取る"""
        request = _to_bytes(request)
        lib = self._core.lib
        code, data = self._core.with_buf(lambda buf: lib.zm_call(self._handle, request, len(request), buf))
        if code != 0:
            raise ZumenError("zm_call", code, data)
        return data

    def call(self, request: Mapping[str, Any]) -> Any:
        """要求を渡し、応答（messages.Response の形の dict）を受け取る。失敗した応答は、ZumenError の response にある"""
        return json.loads(self.call_bytes(_dumps(request)))

    def take(self, blob: int) -> bytes:
        """直前の call が作ったバイナリの結果を、応答の blobs にある番号で取り出す"""
        lib = self._core.lib
        code, data = self._core.with_buf(lambda buf: lib.zm_take(self._handle, blob, buf))
        if code != 0:
            raise ZumenError("zm_take", code, None)
        return data

    def cancel(self) -> None:
        """動いている呼び出しを止める。動いている呼び出しがなければ、何もしない"""
        self._core.lib.zm_cancel(self._handle)

    def close(self) -> None:
        """セッションを閉じる。閉じたあとの呼び出しは、invalid の ZumenError になる"""
        handle, self._handle = self._handle, None
        if handle is not None:
            self._core.lib.zm_session_close(handle)

    def __enter__(self) -> Session:
        return self

    def __exit__(self, *_: object) -> None:
        self.close()

    def __del__(self) -> None:
        try:
            self.close()
        except Exception:
            # 終了の途中で、ライブラリが先に消えていることがある
            pass


class Zumen:
    """読み込んだネイティブのライブラリ"""

    def __init__(self, library: ctypes.CDLL) -> None:
        self._core = _Core(library)

    def abi_version(self) -> int:
        """ABI の大版"""
        return self._core.lib.zm_abi_version()

    def open(self, options: Union[Mapping[str, Any], BytesLike] = None) -> Session:
        """セッションを作る。設定（session_options）は、値か JSON の文字列かバイト列で渡す。渡さなければ既定の設定"""
        data = _dumps(options) if isinstance(options, Mapping) else _to_bytes(options)
        handle = ctypes.c_void_p()
        code = self._core.lib.zm_session_new(data, len(data), ctypes.byref(handle))
        if code != 0:
            raise ZumenError("zm_session_new", code, None)
        return Session(self._core, handle.value)


def _bundled() -> str:
    """wheel に入れたネイティブのライブラリの道筋（決定 0205）"""
    if sys.platform == "win32":
        name = "zumen_abi1.dll"
    elif sys.platform == "darwin":
        name = "libzumen_abi1.dylib"
    else:
        name = "libzumen_abi1.so"
    return os.path.join(os.path.dirname(os.path.abspath(__file__)), "_native", name)


def load(path: Union[str, os.PathLike, None] = None) -> Zumen:
    """ネイティブのライブラリを、ファイルの道筋から読み込み、ABI の大版を確かめる。

    道筋を渡さなければ、wheel に入れたライブラリ（zumen/_native）を読む（決定 0205）。
    32 bit の Windows では、関数を stdcall で呼ぶ（決定 0011）。
    """
    path = os.path.abspath(os.fspath(path)) if path is not None else _bundled()
    if not os.path.isfile(path):
        raise OSError(f"zumen のネイティブのライブラリがない: {path}")
    if sys.platform == "win32" and struct.calcsize("P") == 4:
        library: ctypes.CDLL = ctypes.WinDLL(path)
    else:
        library = ctypes.CDLL(path)
    return Zumen(library)
