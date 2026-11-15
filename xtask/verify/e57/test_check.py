"""保存した公開E57の観測を変え、比較器が点・色・診断・CRSの違いを見逃さないことを確かめる。"""

from copy import deepcopy
import importlib.util
import json
from pathlib import Path
import sys
import unittest

spec = importlib.util.spec_from_file_location("check_e57", Path(__file__).with_name("check_api.py"))
check = importlib.util.module_from_spec(spec)
spec.loader.exec_module(check)
KIT = Path(sys.argv[1]).resolve()
sys.argv = sys.argv[:1]


class CompareTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.samples = [(json.loads((KIT / f"{name}-api.json").read_text(encoding="utf-8")),
                        (KIT / f"{name}-external.bin").read_bytes()) for name in ("double", "float")]

    def changed(self, mutate):
        for original, expected in self.samples:
            with self.subTest():
                altered = deepcopy(original)
                mutate(altered)
                with self.assertRaises(ValueError):
                    check.compare(altered, expected)

    def test_original(self):
        for observed, expected in self.samples:
            self.assertTrue(check.compare(observed, expected)["matched"])

    def test_coordinate(self):
        self.changed(lambda data: data["pointcloud"]["result"]["selected"]["positions"][0].__setitem__(0, 0.123))

    def test_color(self):
        self.changed(lambda data: data["pointcloud"]["result"]["selected"]["color"][0].__setitem__(0, 1))

    def test_duplicate(self):
        def mutate(data):
            selected = data["pointcloud"]["result"]["selected"]
            selected["positions"][1] = selected["positions"][0]
            selected["color"][1] = selected["color"][0]
        self.changed(mutate)

    def test_missing_point(self):
        self.changed(lambda data: data["pointcloud"]["result"]["selected"]["positions"].pop())

    def test_missing_diagnostic(self):
        self.changed(lambda data: data["open"].__setitem__("diagnostics", []))

    def test_crs(self):
        self.changed(lambda data: data["pointcloud"]["result"].__setitem__("crs", 6677))

    def test_extra_diagnostic(self):
        self.changed(lambda data: data["pointcloud"].__setitem__("diagnostics", data["open"]["diagnostics"]))


if __name__ == "__main__":
    unittest.main()
