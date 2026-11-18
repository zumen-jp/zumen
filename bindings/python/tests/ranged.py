"""open の needs にラッパーが答え、入力の範囲を読んで渡すことを確かめる（決定 0232・#474）。

使い方: python bindings/python/tests/ranged.py <ネイティブのライブラリ>

適合試験の 3 点の LAS（形式 0）の頭と VLR に点を足し、全体を渡したときと、初めの部分から範囲で渡したときの
pointcloud の応答を比べる。JavaScript の側（bindings/js/test/ranged.mjs）と同じ入力を作る。
"""

from __future__ import annotations

import io
import json
import struct
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))

from zumen import ZumenError, load  # noqa: E402

CASES = HERE.parent.parent / "conformance" / "cases" / "pointcloud.json"
COUNT = 3000


def sample_las(count: int) -> bytes:
    """適合試験の LAS に、点を count 個まで足したもの"""
    steps = json.loads(CASES.read_text(encoding="utf-8"))["steps"]
    base = bytes.fromhex(next(s for s in steps if s.get("push", {}).get("name") == "b.las#0-365")["push"]["hex"])
    (offset,) = struct.unpack_from("<I", base, 96)
    (length,) = struct.unpack_from("<H", base, 105)
    assert length == 20
    out = bytearray(base[:offset]) + bytearray(count * length)
    struct.pack_into("<I", out, 107, count)
    struct.pack_into("<5I", out, 111, count, 0, 0, 0, 0)
    scale = struct.unpack_from("<3d", out, 131)
    shift = struct.unpack_from("<3d", out, 155)
    low, high = [float("inf")] * 3, [float("-inf")] * 3
    for i in range(count):
        at = offset + i * length
        xyz = ((i * 37) % 1000, (i * 53) % 1000, (i * 11) % 300)
        struct.pack_into("<3iHBB", out, at, *xyz, i % 65536, 0x09, 2 + i % 3)
        for k in range(3):
            real = xyz[k] * scale[k] + shift[k]
            low[k] = min(low[k], real)
            high[k] = max(high[k], real)
    for k in range(3):
        struct.pack_into("<2d", out, 179 + 16 * k, high[k], low[k])
    return bytes(out)


def points(session, doc: str) -> tuple:
    response = session.call({"v": 1, "op": "pointcloud", "doc": doc, "limit": COUNT})
    return response["result"], [session.take(blob) for blob in response.get("blobs") or []]


def expect_error(fn) -> None:
    try:
        fn()
    except ZumenError:
        return
    raise AssertionError("ZumenError にならない")


def main() -> None:
    zumen = load(sys.argv[1])
    las = sample_las(COUNT)
    with zumen.open() as whole:
        whole.push("a.las", las)
        assert whole.call({"v": 1, "op": "open", "input": "a.las"})["result"]["format"] == "las"
        expected = points(whole, "a.las")
    assert expected[0]["points"] == COUNT

    def via(source, **options) -> tuple:
        with zumen.open() as session:
            response = session.open_ranged({"v": 1, "input": "a.las"}, source, **options)
            assert not response["result"].get("needs")
            got = points(session, "a.las")
            # 読み終えた範囲は捨ててある。名指ししたものがなければ close は失敗する
            for r in response["result"].get("done") or []:
                expect_error(lambda: session.call({"v": 1, "op": "close", "inputs": [r["input"]]}))
            return got

    reads = []

    def read(start: int, end: int) -> bytes:
        reads.append((start, end))
        return las[start:end]

    assert via(read, size=len(las), head=512) == expected
    assert reads[0] == (0, 512) and len(reads) >= 2, reads
    assert via(io.BytesIO(las), head=1024) == expected
    assert via(io.BytesIO(las), head=len(las) + 1) == expected
    with tempfile.TemporaryDirectory() as folder:
        path = Path(folder) / "a.las"
        path.write_bytes(las)
        assert via(path, head=700) == expected
        assert via(str(path)) == expected

    # 予算の input_bytes を超える範囲は読まない。失敗したら、渡した入力を捨てる
    with zumen.open() as session:
        before = len(reads)
        try:
            session.open_ranged({"v": 1, "input": "a.las", "budget": {"input_bytes": 600}}, read, size=len(las), head=512)
            raise AssertionError("予算を超えても止まらない")
        except ValueError as error:
            assert "input_bytes" in str(error)
        assert len(reads) == before + 1
        expect_error(lambda: session.call({"v": 1, "op": "close", "inputs": ["a.las"]}))
        session.push("a.las", las)
        assert session.call({"v": 1, "op": "open", "input": "a.las"})["result"]["format"] == "las"

    # 読み口が短いバイト列を返したら止め、渡した範囲を捨てる
    with zumen.open() as session:
        try:
            session.open_ranged({"v": 1, "input": "a.las"}, lambda s, e: las[s:e] if s == 0 else las[s : e - 1], size=len(las), head=512)
            raise AssertionError("短い範囲でも止まらない")
        except ValueError as error:
            assert "長さ" in str(error)
        expect_error(lambda: session.call({"v": 1, "op": "close", "inputs": ["a.las"]}))

    # 範囲で読めない入力は、初めの部分を全体として扱う
    with zumen.open() as session:
        dxf = b"0\nSECTION\n2\nENTITIES\n0\nLINE\n10\n0\n20\n0\n11\n1\n21\n2\n0\nENDSEC\n0\nEOF\n"
        assert session.open_ranged({"v": 1, "input": "a.dxf"}, io.BytesIO(dxf))["result"]["format"] == "dxf"

    print(f"open の needs に、関数・ファイル・パスの読み口で答えた（{COUNT} 点、範囲の読み出し {len(reads) - 1} 回）")


if __name__ == "__main__":
    # Windows の端末の既定（cp1252 など）では日本語を書けないので、UTF-8 で書く
    for stream in (sys.stdout, sys.stderr):
        stream.reconfigure(encoding="utf-8")
    main()
