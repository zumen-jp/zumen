"""サンプルの差し替え、未実行の誤認、上書きを拒否する。"""

import importlib.util
import tempfile
import unittest
from pathlib import Path

spec = importlib.util.spec_from_file_location("ply_make", Path(__file__).with_name("make.py"))
make = importlib.util.module_from_spec(spec)
spec.loader.exec_module(make)


class KitTests(unittest.TestCase):
    def test_external_result_requires_every_named_test_and_success_exit(self):
        output = "\n".join(f"test {name} ... ok" for name in make.TESTS)
        output += "\ntest result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out\n"
        self.assertEqual(make.test_result(output, 0), make.TESTS)
        for changed, code in [(output, 1), (output.replace(make.TESTS[0], "other_test"), 0),
                              (output.replace("4 passed", "3 passed"), 0), ("", 0)]:
            with self.assertRaises(ValueError):
                make.test_result(changed, code)

    def test_rejects_worktree_output_and_nonempty_output(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            (root / ".git").write_text("gitdir: unused", encoding="utf8")
            with self.assertRaises(ValueError):
                make.output_folder(root / "output")
            (root / ".git").unlink()
            (root / "keep.txt").write_text("keep", encoding="utf8")
            with self.assertRaises(ValueError):
                make.output_folder(root)
            self.assertEqual(make.output_folder(root / "new"), root / "new")

    def test_public_sample_is_unchanged_and_hash_mismatch_is_rejected(self):
        _, data, license_bytes = make.sample_files(make.ROOT)
        self.assertEqual(make.sha(data), make.SAMPLE_SHA)
        self.assertIn(b"Redistribution and use", license_bytes)
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            (root / "samples/ply").mkdir(parents=True)
            (root / "samples/ledger.toml").write_bytes((make.ROOT / "samples/ledger.toml").read_bytes())
            (root / "samples" / make.SAMPLE).write_bytes(data + b"\n")
            with self.assertRaises(ValueError):
                make.sample_files(root)


if __name__ == "__main__":
    unittest.main()
