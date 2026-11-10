"""生成宣言の幅・名前と共通手順を確認する。Office の実行試験とは区別する。"""

from __future__ import annotations

import ctypes as c
import hashlib
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import unittest

from prepare import ROOT, rows

LIBRARY = Path(sys.argv.pop()).resolve() if len(sys.argv) > 1 and not sys.argv[-1].startswith("-") else None


class Buffer(c.Structure):
    _fields_ = [("ptr", c.c_void_p), ("length", c.c_size_t)]


class Contract(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        # CI の Windows x86 と同じ出力のコードページでも、CLI が実ファイルを書けることを確かめる。
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "conformance.tsv"
            environment = dict(os.environ, PYTHONIOENCODING="cp1252")
            result = subprocess.run([sys.executable, str(ROOT / "bindings/vba/prepare.py"), str(path)],
                                    env=environment, capture_output=True, check=True)
            cls.prepare_output = result.stdout.decode("utf-8")
            cls.fixture_bytes = path.read_bytes()
        cls.prepared_rows = [line.decode("ascii").split("\t") for line in cls.fixture_bytes.split(b"\r\n") if line]

    def test_written_fixture_uses_vba_line_endings(self):
        fixture = rows()
        self.assertTrue(self.fixture_bytes.endswith(b"\r\n"))
        self.assertEqual(self.fixture_bytes.count(b"\r\n"), len(fixture))
        remainder = self.fixture_bytes.replace(b"\r\n", b"")
        self.assertNotIn(b"\r", remainder)
        self.assertNotIn(b"\n", remainder)
        self.assertEqual(self.prepared_rows, fixture)
        self.assertEqual(self.prepare_output.splitlines(), [f"{len({row[0] for row in fixture})} 組、{sum(bool(row[6]) for row in fixture)} ハッシュ"])

    def test_native_declarations(self):
        source = (ROOT / "bindings/vba/ZumenNative.bas").read_text(encoding="ascii")
        boundary = (ROOT / "schema/boundary.toml").read_text(encoding="utf-8")
        declarations = dict(re.findall(r"Public Declare PtrSafe (?:Function|Sub) (zm_\w+) Lib (.*?)(?=\n\n)", source, re.S))
        expected = {}
        for block in boundary.split("[[functions]]")[1:]:
            block = block.split("[[modules]]")[0]
            if 'targets = ["wasm"]' not in block:
                expected[re.search(r'name = "(zm_\w+)"', block)[1]] = block
        self.assertEqual(set(declarations), set(expected))
        widths = {"size": "ByVal {name} As LongPtr", "bytes": "ByVal {name} As LongPtr", "bytes_mut": "ByVal {name} As LongPtr", "session": "ByVal {name} As LongPtr", "arrow_stream": "ByVal {name} As LongPtr", "session_out": "ByRef {name} As LongPtr", "buf": "ByRef {name} As ZmBuf", "u32": "ByVal {name} As Long"}
        for name, function in expected.items():
            for param_name, param_type in re.findall(r'\{ name = "(\w+)", type = "(\w+)"', function):
                self.assertIn(widths[param_type].format(name="arg_" + param_name), declarations[name])
            result = re.search(r'returns = "(\w+)"', function)[1]
            self.assertTrue(declarations[name].rstrip().endswith(")" if result == "void" else ") As Long"))
        package = (ROOT / "Cargo.toml").read_text(encoding="utf-8").split("[workspace.package]", 1)[1]
        version = re.search(r'^version = "([^"]+)"', package, re.M)[1]
        self.assertIn('Public Const ZM_PACKAGE_VERSION As String = "' + version + '"', source)
        self.assertEqual(c.sizeof(Buffer), 2 * c.sizeof(c.c_void_p))
        for path in (ROOT / "bindings/vba").glob("*"):
            if path.suffix in (".bas", ".cls"):
                text = path.read_text(encoding="ascii")
                self.assertTrue(all(len(line) < 1024 for line in text.splitlines()))

    def test_fixture_covers_all_hashes_and_cases(self):
        fixture = self.prepared_rows
        self.assertEqual(len({row[0] for row in fixture}), len(list((ROOT / "bindings/conformance/cases").glob("*.json"))))
        hashes = [line for line in (ROOT / "bindings/conformance/hashes.txt").read_text(encoding="utf-8").splitlines() if line and not line.startswith("#")]
        self.assertEqual(sum(bool(row[6]) for row in fixture), len(hashes))
        self.assertTrue(all(len(row) == 7 and "\t" not in "".join(row) and "\n" not in "".join(row) for row in fixture))

    @unittest.skipUnless(LIBRARY, "DLL の指定時に、共通手順と ABI を照合する")
    def test_fixture_against_native_dll(self):
        library = c.WinDLL(str(LIBRARY))
        signatures = {
            "zm_abi_version": (c.c_uint32, []),
            "zm_session_new": (c.c_int32, [c.c_void_p, c.c_size_t, c.POINTER(c.c_void_p)]),
            "zm_push": (c.c_int32, [c.c_void_p, c.c_void_p, c.c_size_t, c.c_void_p, c.c_size_t]),
            "zm_call": (c.c_int32, [c.c_void_p, c.c_void_p, c.c_size_t, c.POINTER(Buffer)]),
            "zm_take": (c.c_int32, [c.c_void_p, c.c_uint32, c.POINTER(Buffer)]),
            "zm_take_arrow": (c.c_int32, [c.c_void_p, c.c_uint32, c.c_void_p]),
            "zm_free": (None, [c.POINTER(Buffer)]),
            "zm_session_close": (None, [c.c_void_p]),
            "zm_cancel": (None, [c.c_void_p]),
        }
        for name, (result, args) in signatures.items():
            function = getattr(library, name)
            function.restype = result
            function.argtypes = args
        source = (ROOT / "bindings/vba/ZumenNative.bas").read_text(encoding="ascii")
        self.assertEqual(set(signatures), set(re.findall(r"Public Declare PtrSafe (?:Function|Sub) (zm_\w+) Lib", source)))
        session = c.c_void_p()
        self.assertEqual(library.zm_abi_version(), 1)
        hashes = scenes = 0
        try:
            for case, step, command, status, name, content, digest in self.prepared_rows:
                data = bytes.fromhex(content)
                buffer = Buffer()
                if command == "session":
                    library.zm_session_close(session)
                    session = c.c_void_p()
                    actual = library.zm_session_new(data or None, len(data), c.byref(session))
                elif command == "push":
                    encoded_name = bytes.fromhex(name)
                    actual = library.zm_push(session, encoded_name or None, len(encoded_name), data or None, len(data))
                elif command == "call":
                    actual = library.zm_call(session, data or None, len(data), c.byref(buffer))
                else:
                    actual = library.zm_take(session, int(name), c.byref(buffer))
                try:
                    self.assertEqual(actual, int(status), f"{case}:{step}")
                    received = c.string_at(buffer.ptr, buffer.length) if buffer.length else b""
                    if digest:
                        self.assertEqual(hashlib.sha256(received).hexdigest(), digest, f"{case}:{step}")
                        hashes += 1
                    else:
                        self.assertFalse(received)
                    scenes += received.startswith(b"ZMSC")
                finally:
                    library.zm_free(c.byref(buffer))
                    self.assertEqual((buffer.ptr, buffer.length), (None, 0))
        finally:
            library.zm_session_close(session)
        print(f"DLL: {hashes} ハッシュ、{scenes} シーン")


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    sys.stderr.reconfigure(encoding="utf-8")
    unittest.main()
