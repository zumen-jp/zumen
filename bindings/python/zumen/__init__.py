"""zumen の C ABI を、標準ライブラリの ctypes で呼ぶ薄いラッパー（決定 0011・0195）。

ネイティブのライブラリ（zumen_abi1.dll・libzumen_abi1.so・libzumen_abi1.dylib）を読み込み、API の関数
（schema/boundary.toml）を、セッションのメソッドにして出す。要求と応答の型は messages.py にある。
ctypes は、ネイティブの関数を呼ぶ間 GIL を手放すので、ほかのスレッドから cancel を呼べる。
"""

from __future__ import annotations

import ctypes
import json
import os
import sys
from typing import Any, BinaryIO, Callable, Mapping, Optional, Union

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
from .scene3d import Scene3D, read_scene3d
from .scene3d_layout import SCENE3D_MAJOR, SCENE3D_MINOR, SCENE3D_SECTIONS, SCENE3D_VALUES

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
    "Scene3D",
    "read_scene3d",
    "SCENE3D_MAJOR",
    "SCENE3D_MINOR",
    "SCENE3D_SECTIONS",
    "SCENE3D_VALUES",
]

BytesLike = Union[str, bytes, bytearray, memoryview, None]
RangeSource = Union[str, "os.PathLike[str]", BinaryIO, Callable[[int, int], bytes]]

# open_ranged が 1 回目に渡す初めの部分の既定の大きさ。LAS の頭と VLR が入る
_HEAD = 64 * 1024
# needs に答える回数の上限
_ROUNDS = 64


class ZumenError(Exception):
    """API の関数が ok でない状態を返した"""

    def __init__(self, operation: str, code: int, data: Optional[bytes]) -> None:
        self.operation = operation
        """呼んだ API の関数"""
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
    """読み込んだライブラリと、API の関数"""

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
            raise OSError(f"ABI のメジャーバージョンが {version}（このラッパーは {ABI_VERSION}）")
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

    def open_ranged(
        self,
        request: Mapping[str, Any],
        source: RangeSource,
        head: int = _HEAD,
        size: Optional[int] = None,
    ) -> Any:
        """入力の範囲を読みながら open する（決定 0232）。

        request の input の名前で初めの部分を渡し、size を書いて open を呼ぶ。needs の範囲をその名前で push して同じ
        要求を呼び直し、読み終えた done の入力を close で捨てる。1 回の範囲の合計が要求の budget の input_bytes を
        超えるときと、同じ範囲を求め続けるときは止める。失敗したら、渡した入力を捨てる。source はパス・バイナリの
        ファイル・範囲 [start, end) のバイト列を返す関数。関数なら size に全体の大きさを渡す。応答は最後の open の応答
        """
        name = request.get("input")
        if not isinstance(name, str):
            raise TypeError("request には input を書いた open の要求を渡す")
        if not isinstance(head, int) or head <= 0:
            raise ValueError("head は正の整数")
        if isinstance(source, (str, os.PathLike)):
            with open(source, "rb") as file:
                return self.open_ranged(request, file, head, size)
        if callable(source):
            read = source
        elif hasattr(source, "seek") and hasattr(source, "read"):
            file = source

            def read(start: int, end: int) -> bytes:
                file.seek(start)
                return file.read(end - start)

            if size is None:
                size = file.seek(0, os.SEEK_END)
        else:
            raise TypeError("source にはパス・バイナリのファイル・(start, end) を読む関数を渡す")
        if size is not None and (not isinstance(size, int) or size < 0):
            raise ValueError("size は 0 以上の整数")
        budget = request.get("budget") or {}
        limit = budget.get("input_bytes")
        limit = limit if isinstance(limit, int) and not isinstance(limit, bool) else None
        call = dict(request, op="open")
        if size is not None:
            call["size"] = size
        pushed = []
        try:
            self.push(name, read(0, head if size is None else min(head, size)))
            pushed.append(name)
            previous = None
            for round_ in range(_ROUNDS + 1):
                response = self.call(call)
                result = response.get("result") or {}
                done = [r["input"] for r in result.get("done") or []]
                if done:
                    self.call({"v": 1, "op": "close", "inputs": done})
                    pushed = [n for n in pushed if n not in done]
                needs = result.get("needs") or []
                if not needs:
                    # 文書ができたら、初めの部分は普段の open の入力と同じく残す
                    pushed.remove(name)
                    return response
                if needs == previous or round_ >= _ROUNDS:
                    raise RuntimeError("open が同じ範囲を求め続ける")
                previous = needs
                total = 0
                for r in needs:
                    start, end = r.get("start"), r.get("end")
                    if not (isinstance(start, int) and isinstance(end, int) and 0 <= start <= end) or (size is not None and end > size):
                        raise ValueError("open の needs の範囲が入力の外")
                    total += end - start
                if limit is not None and total > limit:
                    raise ValueError("needs の範囲の合計が予算の input_bytes を超える")
                for r in needs:
                    part = _to_bytes(read(r["start"], r["end"]))
                    if len(part) != r["end"] - r["start"]:
                        raise ValueError("読み口が求めた長さを返さない")
                    self.push(r["input"], part)
                    pushed.append(r["input"])
            raise RuntimeError("open が同じ範囲を求め続ける")
        except BaseException:
            # 読み終える前に止まったら、渡した初めの部分と範囲を捨てる。捨てられなくても、先の例外を返す
            if pushed and not self.closed:
                try:
                    self.call({"v": 1, "op": "close", "inputs": pushed})
                except Exception:
                    pass
            raise

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
        """ABI のメジャーバージョン"""
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
    """ネイティブのライブラリを、ファイルの道筋から読み込み、ABI のメジャーバージョンを確かめる。

    道筋を渡さなければ、wheel に入れたライブラリ（zumen/_native）を読む（決定 0205）。
    """
    path = os.path.abspath(os.fspath(path)) if path is not None else _bundled()
    if not os.path.isfile(path):
        raise OSError(f"zumen のネイティブのライブラリがない: {path}")
    return Zumen(ctypes.CDLL(path))
