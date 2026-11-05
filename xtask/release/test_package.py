"""梱包の抜け・取り違えを確かめる。合成のバイナリは一時ディレクトリだけに置く。"""

import hashlib
import json
import subprocess
import sys
import tempfile
import unittest
import zipfile
from pathlib import Path
from unittest.mock import patch

import licenses
import package


def notice(directory, kind, target):
    directory.mkdir(parents=True)
    content = f"test {kind} {target}\n".encode() + (licenses.ROOT / "LICENSE-MIT").read_bytes()
    info = {"package": f"zumen-{kind}", "target": target, "sha256": hashlib.sha256(content).hexdigest(),
            "license_expression": licenses.license_expression(["MIT"]),
            "dependencies": [{"name": "fixture", "version": "0.0.0", "licenses": ["MIT"]}]}
    (directory / "THIRD_PARTY_LICENSES").write_bytes(content)
    (directory / "license-info.json").write_text(json.dumps(info), encoding="utf-8")
    return info, content


class PackageTests(unittest.TestCase):
    def test_colored_cargo_about_warning_is_rejected(self):
        result = subprocess.CompletedProcess([], 0, "", "\x1b[33mWARN\x1b[0m changed clarify checksum")
        with patch("licenses.subprocess.run", return_value=result), self.assertRaisesRegex(ValueError, "警告"):
            licenses.run_about(["cargo-about", "about", "generate"])

    def test_wrong_target_crate_and_changed_text_are_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary) / "notice"
            notice(directory, "abi", "x86_64-pc-windows-msvc")
            with self.assertRaisesRegex(ValueError, "target"):
                licenses.read_info(directory, "zumen-abi", "aarch64-pc-windows-msvc")
            with self.assertRaisesRegex(ValueError, "crate"):
                licenses.read_info(directory, "zumen-cli", "x86_64-pc-windows-msvc")
            (directory / "THIRD_PARTY_LICENSES").write_bytes(b"changed")
            with self.assertRaisesRegex(ValueError, "ハッシュ"):
                licenses.read_info(directory)

    def test_missing_body_and_spdx_placeholder_are_rejected(self):
        crate = {"name": "fixture", "version": "0.0.0", "repository": "https://example.test/fixture"}
        data = {"crates": [{"package": crate}], "licenses": []}
        with self.assertRaisesRegex(ValueError, "一致"):
            licenses.third_party(data)
        data["licenses"] = [{"id": "MIT", "text": "Copyright <year> <copyright holders>", "used_by": [{"crate": crate}]}]
        with self.assertRaisesRegex(ValueError, "定型"):
            licenses.third_party(data)

    def test_archives_and_wheel_keep_the_correct_notices(self):
        # 2 種類の圧縮を試す。実バイナリの 7 target は package-check.yml が同じ関数で確かめる。
        for rid in ("win-x64", "linux-x64"):
            with self.subTest(rid=rid), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                target = package.target_for(rid)
                built = root / "built"
                built.mkdir()
                library = b"synthetic library for packaging tests"
                (built / target["library"]).write_bytes(library)
                (built / target["exe"]).write_bytes(b"synthetic CLI for packaging tests")
                notices = root / "licenses" / "native" / rid
                info, abi = notice(notices / "abi", "abi", target["target"])
                _, cli = notice(notices / "cli", "cli", target["target"])
                output = root / "out"
                package.package_native(rid, built, root, output)
                self.assertEqual((output / "native" / rid / "THIRD_PARTY_LICENSES").read_bytes(), abi)
                archive = next(output.glob("zumen-*"))
                self.assertEqual(package.read_archive(archive)[f"zumen-{package.VERSION}-{rid}/THIRD_PARTY_LICENSES"], cli)
                wheel = next((output / "wheels").glob("*.whl"))
                with zipfile.ZipFile(wheel) as source:
                    entries = {name: source.read(name) for name in source.namelist()}
                notice_name = f"zumen-{package.VERSION}.dist-info/licenses/THIRD_PARTY_LICENSES"
                self.assertEqual(entries[notice_name], abi)
                # 本文があっても RECORD に載っていなければ拒む。
                record_name = f"zumen-{package.VERSION}.dist-info/RECORD"
                entries[record_name] = b"\n".join(line for line in entries[record_name].splitlines()
                                                  if not line.startswith(notice_name.encode())) + b"\n"
                with zipfile.ZipFile(wheel, "w") as corrupt:
                    for name, content in entries.items():
                        corrupt.writestr(name, content)
                with self.assertRaisesRegex(ValueError, "RECORD"):
                    package.verify_wheel(wheel, target, library, abi, info["license_expression"])

    def test_standalone_wheel_rejects_another_target_notice(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            library = root / "zumen_abi1.dll"
            library.write_bytes(b"synthetic library")
            notice(root / "notice", "abi", "aarch64-pc-windows-msvc")
            result = subprocess.run([sys.executable, str(licenses.ROOT / "bindings/python/build_wheel.py"),
                                     str(library), "win_amd64", str(root / "out"), "--licenses", str(root / "notice")],
                                    capture_output=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertFalse((root / "out").exists())


if __name__ == "__main__":
    unittest.main()
