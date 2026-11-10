"""キットの比較器が欠落・共有値の誤変更を見逃さず、既存のファイルを上書きしないことを調べる。"""

import copy
import importlib.util
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

spec = importlib.util.spec_from_file_location("ifc_kit", Path(__file__).with_name("make.py"))
kit = importlib.util.module_from_spec(spec)
spec.loader.exec_module(kit)


def models():
    expected = kit.recipe("ifc4")
    wall = {"id": expected["target"], "hash": "v1:before", "identity": {"name": "Original", "description": "Before"},
            "class": {"ifc_class": "IfcWall"}, "placement": {"translation": [0, 0, 0]},
            "properties": [{"name": "Pset_WallCommon", "properties": [
                {"name": "IsExternal", "value": {"kind": "boolean", "type_name": "IFCBOOLEAN", "boolean": True}},
                {"name": "Reference", "value": {"kind": "text", "text": "Unchanged"}}]}]}
    control = copy.deepcopy(wall)
    control["id"] = expected["control"]
    before = {"schema": "IFC4", "units": {"length": "m"}, "elements": [wall, control], "diagnostics": {"ifc.entity.dropped": 3}}
    after = copy.deepcopy(before)
    target = after["elements"][0]
    target["hash"] = "v1:after"
    target["identity"].update(name=expected["name"], description=expected["description"])
    target["properties"][0]["properties"][0]["value"]["boolean"] = False
    target["properties"].append({"name": "Zumen_Check526", "properties": [
        {"name": prop["name"], "value": copy.deepcopy(prop["value"])} for prop in expected["properties"][1:]]})
    return before, after, expected


class ComparisonTests(unittest.TestCase):
    def test_sums_diagnostic_occurrences_instead_of_entries(self):
        response = {"diagnostics": [{"code": "ifc.entity.dropped", "count": 25},
                                    {"code": "ifc.entity.dropped", "count": 4},
                                    {"code": "ifc.unit.missing", "count": 1}]}
        self.assertEqual(kit.diagnostic_counts(response), {"ifc.entity.dropped": 29, "ifc.unit.missing": 1})
        before, after, expected = models()
        before["diagnostics"] = kit.diagnostic_counts({"diagnostics": [{"code": "ifc.entity.dropped", "count": 3}]})
        after["diagnostics"] = kit.diagnostic_counts({"diagnostics": [{"code": "ifc.entity.dropped", "count": 4}]})
        self.assertIn("diagnostics", kit.differences(before, after, expected))

    def test_changed_values_and_unchanged_control_match(self):
        before, after, expected = models()
        self.assertEqual(kit.differences(before, after, expected), [])

    def test_detects_each_intended_field(self):
        before, after, expected = models()
        for field in ["name", "description"]:
            changed = copy.deepcopy(after)
            changed["elements"][0]["identity"][field] = "Different"
            self.assertIn("identity", kit.differences(before, changed, expected))
        for index, prop in enumerate(expected["properties"]):
            with self.subTest(property=prop["name"]):
                changed = copy.deepcopy(after)
                group = 0 if index == 0 else 1
                offset = 0 if index == 0 else index - 1
                changed["elements"][0]["properties"][group]["properties"][offset]["value"] = {"kind": "text", "text": "Wrong"}
                self.assertIn("property:" + prop["set"] + "." + prop["name"], kit.differences(before, changed, expected))

    def test_detects_shared_property_changed_on_control(self):
        before, after, expected = models()
        after["elements"][1]["properties"][0]["properties"][0]["value"]["boolean"] = False
        self.assertIn("unchanged_element:" + expected["control"], kit.differences(before, after, expected))

    def test_detects_missing_duplicate_extra_and_different_ids(self):
        before, after, expected = models()
        for elements in [after["elements"][:1], [after["elements"][0]] * 2, after["elements"] + after["elements"][:1]]:
            self.assertIn("element_ids", kit.differences(before, {**after, "elements": elements}, expected))
        after["elements"][0]["id"] = "different"
        self.assertIn("element_ids", kit.differences(before, after, expected))

    def test_detects_unchanged_placement_property_and_diagnostics(self):
        before, after, expected = models()
        after["elements"][0]["placement"]["translation"][0] = 1
        after["elements"][0]["properties"][0]["properties"][1]["value"]["text"] = "Wrong"
        after["diagnostics"] = {}
        failures = kit.differences(before, after, expected)
        self.assertIn("placement", failures)
        self.assertIn("unchanged_property:Pset_WallCommon.Reference", failures)
        self.assertIn("diagnostics", failures)

    def test_detects_different_schema_and_units(self):
        before, after, expected = models()
        after["schema"] = "IFC2X3"
        after["units"] = {"length": "mm"}
        failures = kit.differences(before, after, expected)
        self.assertIn("schema", failures)
        self.assertIn("units", failures)

    def test_rejects_duplicate_property_names(self):
        before, after, expected = models()
        group = after["elements"][0]["properties"][0]
        group["properties"].append(copy.deepcopy(group["properties"][0]))
        with self.assertRaises(ValueError):
            kit.differences(before, after, expected)

    def test_detects_added_unit_on_changed_property(self):
        before, after, expected = models()
        after["elements"][0]["properties"][0]["properties"][0]["unit"] = {"kind": "length"}
        self.assertIn("property_metadata:Pset_WallCommon.IsExternal", kit.differences(before, after, expected))

    def test_does_not_treat_boolean_as_number_or_ignore_type(self):
        self.assertFalse(kit.same_value({"kind": "integer", "type_name": "IfcInteger", "number": True},
                                        {"kind": "integer", "type_name": "IfcInteger", "number": 1}))
        self.assertFalse(kit.same_value({"kind": "real", "type_name": "IfcLengthMeasure", "number": 1.25},
                                        {"kind": "real", "type_name": "IfcReal", "number": 1.25}))

    def test_preserves_unedited_records_and_header(self):
        before = b"HEADER;\r\n#1=IFCWALL('Before');\r\n#2=IFCCARTESIANPOINT((0.,0.,0.));\r\nENDSEC;\r\n"
        after = before.replace(b"'Before'", b"'After'").replace(b"ENDSEC;", b"#3=IFCPROPERTYSET();\r\nENDSEC;")
        self.assertTrue(kit.unchanged_records(before, after, [1]))
        self.assertFalse(kit.unchanged_records(before, after.replace(b"(0.,0.,0.)", b"(1.,0.,0.)"), [1]))
        self.assertFalse(kit.unchanged_records(before, after.replace(b"HEADER;", b"CHANGED;"), [1]))

    def test_checks_geometry_reference_even_in_changed_root_record(self):
        before = b"#1= IFCWALL('guid',#2,'Old',$,$,#3,#4,'tag');\n"
        after = b"#1=IFCWALL('guid',#2,'New','Description',$,#3,#4,'tag');\n"
        self.assertTrue(kit.unchanged_root_references(before, after, 1))
        for changed in [after.replace(b"#4", b"#40"), after.replace(b"#3", b"#30"), after.replace(b"'guid'", b"'changed'")]:
            self.assertFalse(kit.unchanged_root_references(before, changed, 1))


class OutputTests(unittest.TestCase):
    def test_refuses_git_checkout_and_nonempty_output(self):
        with tempfile.TemporaryDirectory() as name:
            folder = Path(name)
            (folder / ".git").write_text("gitdir: ignored", encoding="utf-8")
            with self.assertRaises(ValueError):
                kit.output_folder(folder / "output")
        with tempfile.TemporaryDirectory() as name:
            folder = Path(name)
            marker = folder / "keep.txt"
            marker.write_text("keep", encoding="utf-8")
            with self.assertRaises(ValueError):
                kit.output_folder(folder)
            self.assertEqual(marker.read_text(), "keep")

    def test_limits_file_size(self):
        with tempfile.TemporaryDirectory() as name:
            path = Path(name) / "input.ifc"
            path.write_bytes(b"12345")
            with mock.patch.object(kit, "MAX_INPUT", 4), self.assertRaises(ValueError):
                kit.read_input(path)


@unittest.skipUnless(os.environ.get("ZUMEN_CLI"), "ZUMEN_CLI があれば実際の CLI でも流す")
class CliTests(unittest.TestCase):
    def run_command(self, *args, expected_status=0):
        result = subprocess.run([sys.executable, "-I", str(Path(__file__).with_name("make.py")), *map(str, args)],
                                capture_output=True)
        self.assertEqual(result.returncode, expected_status, result.stdout.decode("utf-8") + result.stderr.decode("utf-8"))

    def test_generates_both_schemas_deterministically_and_observes(self):
        exe = os.environ["ZUMEN_CLI"]
        with tempfile.TemporaryDirectory() as name:
            root = Path(name)
            first, second = root / "first", root / "second"
            self.run_command("make", exe, first)
            self.run_command("make", exe, second)
            manifest = json.loads((first / "manifest.json").read_bytes())
            for schema in kit.CASES:
                case = manifest["cases"][schema]
                self.assertEqual(case["failures"], [])
                self.assertGreater(case["elements"], 2)
                self.assertEqual((first / schema / "edited.ifc").read_bytes(), (second / schema / "edited.ifc").read_bytes())
                self.run_command("observe", exe, first, schema, first / schema / "edited.ifc")
                # 編集前の IFC を製品の出力として渡しても、編集後の期待値とは一致しない。
                self.run_command("observe", exe, second, schema, second / schema / "input.ifc", expected_status=1)
                failures = json.loads((second / "observed" / schema / "comparison.json").read_bytes())["failures"]
                self.assertIn("identity", failures)
                self.assertTrue(any(item.startswith("property:") for item in failures))
            # 属性は同じで、集約された診断だけが増えた出力も一致にはしない。
            third = root / "third"
            shutil.copytree(first, third, ignore=shutil.ignore_patterns("observed"))
            for schema in kit.CASES:
                data = (first / schema / "edited.ifc").read_bytes()
                number = max(map(int, re.findall(rb"(?m)^#([0-9]+)\s*=", data))) + 1
                end = data.rfind(b"ENDSEC;")
                changed = root / (schema + ".extra.ifc")
                changed.write_bytes(data[:end] + f"#{number}=IFCCARTESIANPOINT((0.,0.,0.));\n".encode() + data[end:])
                self.run_command("observe", exe, third, schema, changed, expected_status=1)
                comparison = json.loads((third / "observed" / schema / "comparison.json").read_bytes())
                self.assertEqual(comparison["failures"], ["diagnostics"])
            self.run_command("make", exe, first, expected_status=1)


if __name__ == "__main__":
    unittest.main()
