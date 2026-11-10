"""Linux の配布物の下限を、実行せずに確認する検査を確かめる。"""

import struct
import unittest
from types import SimpleNamespace
from unittest.mock import patch

from linux_abi import verify_environment, verify_linux


def elf_fixture(versions=("GLIBC_2.17", "GLIBC_2.28"), machine=62, executable=False, groups=None):
    groups = groups or [versions]
    strings = bytearray(b"\0libc.so.6\0")
    records = bytearray()
    for index, group in enumerate(groups):
        length = 16 + 16 * len(group)
        records.extend(struct.pack("<HHIII", 1, len(group), 1, 16, length if index + 1 < len(groups) else 0))
        for item, version in enumerate(group):
            offset = len(strings)
            strings.extend(version.encode("ascii") + b"\0")
            records.extend(struct.pack("<IHHII", 0, 0, item + 2, offset, 16 if item + 1 < len(group) else 0))
    header = struct.pack("<16sHHIQQQIHHHHHH", b"\x7fELF\x02\x01\x01" + b"\0" * 9,
                         2 if executable else 3, machine, 1, 0, 0, 64, 0, 64, 0, 0, 64, 3, 0)
    sections = bytes(64)
    sections += struct.pack("<IIQQQQIIQQ", 0, 3, 0, 0, 256, len(strings), 0, 0, 1, 0)
    sections += struct.pack("<IIQQQQIIQQ", 0, 0x6ffffffe, 0, 0, 256 + len(strings), len(records), 1, len(groups), 8, 0)
    return header + sections + strings + records


def target(machine=62):
    return {"target": "x86_64-unknown-linux-gnu" if machine == 62 else "aarch64-unknown-linux-gnu",
            "wheel": "manylinux_2_28_x86_64" if machine == 62 else "manylinux_2_28_aarch64"}


class LinuxAbiTests(unittest.TestCase):
    def test_both_architectures_and_executable_types_accept_the_floor(self):
        for machine in (62, 183):
            for executable in (False, True):
                with self.subTest(machine=machine, executable=executable):
                    self.assertEqual(verify_linux(elf_fixture(machine=machine, executable=executable), target(machine)),
                                     ["GLIBC_2.17", "GLIBC_2.28"])

    def test_newer_requirements_in_later_records_are_rejected(self):
        binary = elf_fixture(groups=[("GLIBC_2.28",), ("GLIBC_2.29", "GLIBC_2.34")])
        with self.assertRaisesRegex(ValueError, "GLIBC_2.34"):
            verify_linux(binary, target())
        with self.assertRaisesRegex(ValueError, "GLIBC_2.100"):
            verify_linux(elf_fixture(("GLIBC_2.100",)), target())

    def test_architecture_and_wheel_tag_must_match_the_declared_target(self):
        with self.assertRaisesRegex(ValueError, "アーキテクチャ"):
            verify_linux(elf_fixture(machine=183), target())
        wrong = {**target(), "wheel": "manylinux_2_35_x86_64"}
        with self.assertRaisesRegex(ValueError, "タグ"):
            verify_linux(elf_fixture(), wrong)

    def test_missing_or_private_requirements_do_not_pass(self):
        for versions in (("GLIBC_PRIVATE",), ("GCC_3.0",)):
            with self.subTest(versions=versions), self.assertRaises(ValueError):
                verify_linux(elf_fixture(versions), target())
        binary = bytearray(elf_fixture())
        struct.pack_into("<I", binary, 64 * 3 + 4, 0)
        with self.assertRaisesRegex(ValueError, "バージョン要求の節"):
            verify_linux(binary, target())

    def test_truncated_or_wrongly_linked_records_are_rejected(self):
        binary = elf_fixture()
        for end in (0, 6, 62, 200, len(binary) - 1):
            with self.subTest(end=end), self.assertRaises(ValueError):
                verify_linux(binary[:end], target())
        broken = bytearray(binary)
        # sh_link が節の一覧の外を指す。
        struct.pack_into("<I", broken, 64 * 3 + 40, 3)
        with self.assertRaisesRegex(ValueError, "文字列の節"):
            verify_linux(broken, target())
        broken = bytearray(binary)
        records = 256 + len(b"\0libc.so.6\0GLIBC_2.17\0GLIBC_2.28\0")
        # 補助項目の件数が 2 なのに、最初の項目で連結が切れる。
        struct.pack_into("<I", broken, records + 16 + 12, 0)
        with self.assertRaisesRegex(ValueError, "連結"):
            verify_linux(broken, target())

    def test_non_linux_artifacts_are_outside_this_check(self):
        self.assertEqual(verify_linux(b"synthetic Windows DLL", {"target": "x86_64-pc-windows-msvc"}), [])

    def test_build_environment_requires_the_floor_and_native_architecture(self):
        with patch("linux_abi.sys.platform", "win32"), self.assertRaisesRegex(ValueError, "manylinux_2_28"):
            verify_environment(target())
        with patch("linux_abi.sys.platform", "linux"), patch("linux_abi.os.confstr", return_value="glibc 2.35", create=True):
            with self.assertRaisesRegex(ValueError, "manylinux_2_28"):
                verify_environment(target())
        with patch("linux_abi.sys.platform", "linux"), patch("linux_abi.os.confstr", return_value="glibc 2.28", create=True):
            with patch("linux_abi.os.uname", return_value=SimpleNamespace(machine="aarch64"), create=True):
                verify_environment(target(183))
                with self.assertRaisesRegex(ValueError, "アーキテクチャ"):
                    verify_environment(target())


if __name__ == "__main__":
    unittest.main()
