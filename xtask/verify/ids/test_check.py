"""実測した観測を変え、IDS の比較器が欠落・重複・件数の違いを検出するか確かめる。"""

from copy import deepcopy
import importlib.util
import json
from pathlib import Path
import sys
import tempfile
import unittest

spec = importlib.util.spec_from_file_location("check_ids", Path(__file__).with_name("check.py"))
check = importlib.util.module_from_spec(spec)
spec.loader.exec_module(check)
KIT = Path(sys.argv[1]).resolve(strict=True)
sys.argv = sys.argv[:1]


class CompareTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.samples = [(json.loads((KIT / ("external-" + label + ".json")).read_bytes()),
                        json.loads((KIT / ("core-" + label + ".json")).read_bytes())) for label, _, _ in check.recipe.ENCODINGS]

    def changed(self, mutate, side="core"):
        for external, core in self.samples:
            with self.subTest(side=side):
                left, right = deepcopy(external), deepcopy(core)
                mutate(right if side == "core" else left)
                self.assertFalse(check.compare(left, right)["matched"])

    def test_original_and_artifacts(self):
        result = json.loads((KIT / "results.json").read_bytes())
        self.assertIs(result["complete"], True)
        self.assertIs(result["matched"], True)
        self.assertEqual(len(result["cases"]), 3)
        for item in result["cases"]:
            self.assertIs(item["repeatedResponseEqual"], True)
        for name, digest in result["artifacts"].items():
            self.assertEqual(check.sha((KIT / name).read_bytes()), digest, name)
        for external, core in self.samples:
            self.assertEqual(check.compare(external, core), {"matched": True, "differences": []})

    def test_each_specification_field(self):
        for key, value in (("applicable", 8), ("failed", 0), ("name", "unknown"), ("status", "passed"), ("unsupported", 1)):
            with self.subTest(key=key):
                self.changed(lambda core: core["result"]["specifications"][0].__setitem__(key, value))

    def test_each_diagnostic_field(self):
        for key, value in (("code", "other"), ("count", 2), ("severity", "warning"), ("at", []), ("params", {})):
            with self.subTest(key=key):
                self.changed(lambda core: core["result"]["findings"][0]["diagnostic"].__setitem__(key, value))

    def test_each_parameter(self):
        for key in ("cardinality", "facet", "specification"):
            self.changed(lambda core: core["result"]["findings"][0]["diagnostic"]["params"].__setitem__(key, "wrong"))

    def test_wrong_failure_id(self):
        self.changed(lambda core: core["result"]["findings"][0]["diagnostic"]["at"][0].__setitem__("id", check.recipe.WALLS[0]))

    def test_duplicate_failure(self):
        self.changed(lambda core: core["result"]["findings"].append(deepcopy(core["result"]["findings"][0])))

    def test_missing_failure(self):
        self.changed(lambda core: core["result"]["findings"].pop())

    def test_duplicate_replaces_missing_failure(self):
        def mutate(core):
            items = core["result"]["findings"]
            items[-1] = deepcopy(items[-2])
        self.changed(mutate)

    def test_wrong_specification_of_failure(self):
        self.changed(lambda core: core["result"]["findings"][0].__setitem__("specification", "unknown"))

    def test_missing_specification(self):
        self.changed(lambda core: core["result"]["specifications"].pop())

    def test_duplicate_specification(self):
        self.changed(lambda core: core["result"]["specifications"].append(deepcopy(core["result"]["specifications"][0])))

    def test_specification_order(self):
        self.changed(lambda core: core["result"]["specifications"].reverse())

    def test_partial_page(self):
        self.changed(lambda core: core["result"].__setitem__("next_offset", 17))

    def test_wrong_total_or_title(self):
        self.changed(lambda core: core["result"].__setitem__("total", 16))
        self.changed(lambda core: core["result"].__setitem__("title", "different"))

    def test_extra_diagnostic_or_blob(self):
        self.changed(lambda core: core["diagnostics"].append(deepcopy(core["result"]["findings"][0]["diagnostic"])))
        self.changed(lambda core: core["blobs"].append({"id": "unexpected"}))

    def test_missing_response_fields(self):
        for key in ("v", "blobs", "diagnostics", "result"):
            self.changed(lambda core: core.pop(key))

    def test_boolean_or_float_is_not_integer(self):
        self.changed(lambda core: core.__setitem__("v", True))
        self.changed(lambda core: core["result"]["specifications"][0].__setitem__("unsupported", False))
        self.changed(lambda core: core["result"]["specifications"][0].__setitem__("applicable", 7.0))

    def test_external_wrong_schema(self):
        self.changed(lambda data: data.__setitem__("schema", "IFC4"), "external")

    def test_external_wrong_name_or_status(self):
        for key, value in (("name", "unknown"), ("status", "passed")):
            self.changed(lambda data: data["specifications"][0].__setitem__(key, value), "external")

    def test_external_missing_or_duplicate_id(self):
        for key in ("applicable", "failed"):
            self.changed(lambda data: data["specifications"][0][key].pop(), "external")
            self.changed(lambda data: data["specifications"][0][key].append(data["specifications"][0][key][0]), "external")

    def test_external_wrong_id_with_same_count(self):
        self.changed(lambda data: data["specifications"][0]["failed"].__setitem__(0, check.recipe.WALLS[0]), "external")

    def test_external_missing_or_extra_specification(self):
        self.changed(lambda data: data["specifications"].pop(), "external")
        self.changed(lambda data: data["specifications"].append(deepcopy(data["specifications"][0])), "external")

    def test_shuffled_findings_are_equivalent(self):
        for external, core in self.samples:
            reordered = deepcopy(core)
            reordered["result"]["findings"].reverse()
            self.assertTrue(check.compare(external, reordered)["matched"])


class OutputTests(unittest.TestCase):
    def test_existing_output_is_preserved(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory)
            (path / "marker").write_bytes(b"preserve")
            with self.assertRaises(ValueError):
                check.output_folder(path)
            self.assertEqual((path / "marker").read_bytes(), b"preserve")

    def test_git_worktree_output_is_refused(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory)
            (path / ".git").write_text("gitdir: elsewhere", encoding="utf-8")
            with self.assertRaises(ValueError):
                check.output_folder(path / "nested" / "kit")
            self.assertFalse((path / "nested").exists())


if __name__ == "__main__":
    unittest.main()
