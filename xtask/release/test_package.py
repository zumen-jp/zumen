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
import sbom
from test_linux_abi import elf_fixture


def notice(directory, kind, target):
    directory.mkdir(parents=True)
    content = f"test {kind} {target}\n".encode() + (licenses.ROOT / "LICENSE-MIT").read_bytes()
    info = {"package": f"zumen-{kind}", "target": target, "features": "default", "sha256": hashlib.sha256(content).hexdigest(),
            "license_expression": licenses.license_expression(["MIT"]),
            "dependencies": [{"name": "fixture", "version": "0.0.0", "licenses": ["MIT"]}]}
    (directory / "THIRD_PARTY_LICENSES").write_bytes(content)
    (directory / "license-info.json").write_text(json.dumps(info), encoding="utf-8")
    platform = next(t for t in licenses.TARGETS if t["target"] == target)
    filename = platform["library" if kind == "abi" else "exe"]
    ref = sbom.bom_ref("local", info["package"], package.VERSION)
    deps = [{"ref": ref, "dependsOn": []}]
    meta = {"component": {"bom-ref": ref, "scope": "required"}, "properties": [
        {"name": k, "value": v} for k, v in {"zumen:crate": info["package"], "zumen:artifact": filename,
        "zumen:features": "default", "zumen:commit": "fixture", "zumen:lock-sha256": "fixture",
        "cdx:rustc:sbom:target:triple": target}.items()]}
    (directory / sbom.DRAFT).write_bytes(sbom.encode({"metadata": meta, "components": [], "dependencies": deps}))
    return info, content


class PackageTests(unittest.TestCase):
    def test_colored_cargo_about_warning_is_rejected(self):
        result = subprocess.CompletedProcess([], 0, "", "\x1b[33mWARN\x1b[0m changed clarify checksum")
        with patch("licenses.subprocess.run", return_value=result), self.assertRaisesRegex(ValueError, "警告"):
            licenses.run_about(["cargo-about", "about", "generate"])

    def test_cargo_about_failure_keeps_the_reason(self):
        result = subprocess.CompletedProcess([], 1, "", "fixture failure")
        with patch("licenses.subprocess.run", return_value=result), self.assertRaisesRegex(ValueError, "fixture failure"):
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
                library = elf_fixture() if rid == "linux-x64" else b"synthetic library for packaging tests"
                (built / target["library"]).write_bytes(library)
                executable = elf_fixture(executable=True) if rid == "linux-x64" else b"synthetic CLI for packaging tests"
                (built / target["exe"]).write_bytes(executable)
                notices = root / "licenses" / "native" / rid
                info, abi = notice(notices / "abi", "abi", target["target"])
                _, cli = notice(notices / "cli", "cli", target["target"])
                output = root / "out"
                for kind, filename in (("abi", target["library"]), ("cli", target["exe"])):
                    ref = sbom.bom_ref("local", "zumen-" + kind, package.VERSION)
                    receipt = {"package": "zumen-" + kind, "target": target["target"], "features": "default",
                               "artifact": filename, "sha256": sbom.digest((built / filename).read_bytes()),
                               "commit": "fixture", "lock": "fixture", "compiled": [ref],
                               "root": ref, "tree": {ref: ["runtime", []]}}
                    (built / (filename + ".build.json")).write_bytes(sbom.encode(receipt))

                def extract_fixture(binary):
                    name = "zumen-abi" if binary == library else "zumen-cli"
                    ref = sbom.bom_ref("local", name, package.VERSION)
                    return ref, {ref: ("runtime", set())}

                with patch("sbom.extracted_graph", side_effect=extract_fixture):
                    package.package_native(rid, built, root, output)
                self.assertEqual((output / "native" / rid / "THIRD_PARTY_LICENSES").read_bytes(), abi)
                archive = next(output.glob("zumen-*"))
                self.assertEqual(package.read_archive(archive)[f"zumen-{package.VERSION}-{rid}/THIRD_PARTY_LICENSES"], cli)
                wheel = next((output / "wheels").glob("*.whl"))
                with zipfile.ZipFile(wheel) as source:
                    entries = {name: source.read(name) for name in source.namelist()}
                notice_name = f"zumen-{package.VERSION}.dist-info/licenses/THIRD_PARTY_LICENSES"
                self.assertEqual(entries[notice_name], abi)
                abi_bom = (output / "native" / rid / (target["library"] + ".cdx.json")).read_bytes()
                self.assertEqual(entries[f"zumen-{package.VERSION}.dist-info/sboms/{target['library']}.cdx.json"], abi_bom)
                # 本文があっても RECORD に載っていなければ拒む。
                record_name = f"zumen-{package.VERSION}.dist-info/RECORD"
                entries[record_name] = b"\n".join(line for line in entries[record_name].splitlines()
                                                  if not line.startswith(notice_name.encode())) + b"\n"
                with zipfile.ZipFile(wheel, "w") as corrupt:
                    for name, content in entries.items():
                        corrupt.writestr(name, content)
                with self.assertRaisesRegex(ValueError, "RECORD"):
                    package.verify_wheel(wheel, target, library, abi, info["license_expression"], abi_bom)

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

    def test_linux_libraries_and_cli_above_the_floor_are_not_packaged(self):
        for rid, machine in (("linux-x64", 62), ("linux-arm64", 183)):
            for newer in ("abi", "cli"):
                with self.subTest(rid=rid, newer=newer), tempfile.TemporaryDirectory() as temporary:
                    root = Path(temporary)
                    target = package.target_for(rid)
                    built = root / "built"
                    built.mkdir()
                    for kind, filename in (("abi", target["library"]), ("cli", target["exe"])):
                        versions = ("GLIBC_2.34",) if kind == newer else ("GLIBC_2.28",)
                        (built / filename).write_bytes(elf_fixture(versions, machine, executable=kind == "cli"))
                        notice(root / "licenses" / "native" / rid / kind, kind, target["target"])
                    with self.assertRaisesRegex(ValueError, "GLIBC_2.34"):
                        package.package_native(rid, built, root, root / "out")
                    self.assertFalse((root / "out").exists())

    def test_standalone_linux_wheel_does_not_hide_a_newer_runtime_with_an_old_tag(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            library = root / "libzumen_abi1.so"
            library.write_bytes(elf_fixture(("GLIBC_2.34",)))
            notice(root / "notice", "abi", "x86_64-unknown-linux-gnu")
            result = subprocess.run([sys.executable, str(licenses.ROOT / "bindings/python/build_wheel.py"),
                                     str(library), "manylinux_2_28_x86_64", str(root / "out"),
                                     "--licenses", str(root / "notice")], capture_output=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn(b"GLIBC_2.34", result.stderr)
            self.assertFalse((root / "out").exists())


if __name__ == "__main__":
    unittest.main()
