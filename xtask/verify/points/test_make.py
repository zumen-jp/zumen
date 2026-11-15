"""実際の点のキットで、比較しないままの通過と期待値の改変を検出する。"""

from copy import deepcopy
import importlib.util
import json
from pathlib import Path
import sys
import unittest

spec = importlib.util.spec_from_file_location("points_make", Path(__file__).with_name("make.py"))
make = importlib.util.module_from_spec(spec)
spec.loader.exec_module(make)


class KitTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.manifest = json.loads((KIT / "manifest.json").read_text(encoding="utf8"))
        cls.entries = {f["path"]: f for f in cls.manifest["files"]}

    def pair(self, filename):
        after = deepcopy(self.entries["342/" + filename])
        return deepcopy(self.entries[after["from"]]), after

    def test_measured_scope_has_two_full_and_two_partial_cases(self):
        found = []
        for entry in self.manifest["files"]:
            if entry["role"] == "apply":
                result = make.compare_case(KIT, self.entries[entry["from"]], entry)
                found.append((result["status"], result["counts"].get("expected", 0)))
                self.assertEqual(result["applicationStatus"], "unverified")
        self.assertEqual(found, [("verified", 5), ("partial", 0), ("partial", 2), ("verified", 2)])

    def test_empty_skipped_duplicate_or_failed_edits_do_not_pass(self):
        _, original = self.pair("made-features.points.sfc")
        for mutation in ("skipped", "failed", "missing", "duplicate", "empty-id"):
            with self.subTest(mutation=mutation):
                entry = deepcopy(original)
                step = entry["steps"][0]
                if mutation == "skipped":
                    step["skipped"] = True
                elif mutation == "failed":
                    step["status"] = "error"
                elif mutation == "missing":
                    step["expect"].pop()
                elif mutation == "duplicate":
                    step["expect"][1]["id"] = step["expect"][0]["id"]
                else:
                    step["expect"][0]["id"] = None
                with self.assertRaises(ValueError):
                    make.complete_edit(entry)

    def test_sfc_rejects_wrong_point_geometry_style_and_layer(self):
        before, original = self.pair("made-features.points.sfc")
        for field in ("position", "rotation", "code", "scale", "color", "layer"):
            with self.subTest(field=field):
                entry = deepcopy(original)
                want = entry["steps"][0]["expect"][0]
                if field == "position":
                    want["geometry"][field][0] += 1
                elif field == "rotation":
                    want["geometry"][field] += 5
                elif field in ("code", "scale"):
                    want["geometry"]["marker"][field] = "9" if field == "code" else 4
                elif field == "color":
                    want["color"]["sxf"] = 5
                else:
                    want["layer"] = "OTHER"
                with self.assertRaisesRegex(ValueError, "外部実装との不一致"):
                    make.compare_case(KIT, before, entry)

    def test_jww_rejects_wrong_marker_and_pen(self):
        before, original = self.pair("conformance-open_jww-a.points.jww")
        for field in ("code", "color"):
            with self.subTest(field=field):
                entry = deepcopy(original)
                want = entry["steps"][0]["expect"][1]
                if field == "code":
                    want["geometry"]["marker"]["code"] = "9"
                else:
                    want["color"]["jww"] = 6
                with self.assertRaisesRegex(ValueError, "外部実装との不一致"):
                    make.compare_case(KIT, before, entry)

    def test_non_target_values_ids_hashes_and_missing_elements_are_rejected(self):
        before_entry, entry = self.pair("made-features.points.sfc")
        before, original = [json.loads((KIT / "zumen" / (e["path"] + ".json")).read_text(encoding="utf8")) for e in (before_entry, entry)]
        added = {e["id"] for e in entry["steps"][0]["expect"]}
        for field in ("geometry", "id", "hash", "missing", "tables"):
            with self.subTest(field=field):
                after = deepcopy(original)
                old = next(e for e in after["entities"] if e["id"] == before["entities"][0]["id"])
                if field == "missing":
                    after["entities"].remove(old)
                elif field == "tables":
                    after["tables"]["layers"] = []
                else:
                    old[field] = "changed"
                with self.assertRaises(ValueError):
                    make.preserved_reading(before, after, added)

    def test_nonempty_kit_and_repository_output_are_rejected(self):
        for folder in (KIT, make.ROOT / "points-test-output"):
            with self.subTest(folder=folder.name), self.assertRaises(ValueError):
                make.output_folder(folder)


if __name__ == "__main__":
    if len(sys.argv) != 2:
        sys.exit("使い方: python test_make.py <点のキット>")
    KIT = Path(sys.argv.pop()).resolve()
    unittest.main()
