"""POLYLINE の親だけの変更や、監査が隠す頂点属性の不一致を検出する。"""

from copy import deepcopy
import importlib.util
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location("polyline_check", Path(__file__).with_name("check.py"))
check = importlib.util.module_from_spec(spec)
spec.loader.exec_module(check)


class ComparisonTests(unittest.TestCase):
    def setUp(self):
        self.work = tempfile.TemporaryDirectory(prefix="zumen-polyline-test-")
        self.addCleanup(self.work.cleanup)
        self.file = Path(self.work.name) / "input.dxf"
        self.file.write_bytes(check.fixture("R2000", False))
        self.before = check.external(self.file)

    def changed(self, operation):
        after = deepcopy(self.before)
        poly = after["state"]["polylines"][0]
        for entity in [poly["parent"], *poly["vertices"]]:
            entity["attributes"].update(check.STYLES[operation])
        return after

    def test_matching_style_and_noop_pass(self):
        for operation, style in check.STYLES.items():
            with self.subTest(operation=operation):
                self.assertEqual(check.evaluate(style, self.before, self.changed(operation)), [])

    def test_parent_only_restyle_fails_with_zero_audit_errors(self):
        for operation, count in (("color", 2), ("layer", 2), ("linetype", 2), ("combined", 6)):
            after = deepcopy(self.before)
            after["state"]["polylines"][0]["parent"]["attributes"].update(check.STYLES[operation])
            self.assertEqual(after["errors"], 0)
            self.assertEqual(len(check.evaluate(check.STYLES[operation], self.before, after)), count)

    def test_geometry_owner_order_and_other_entity_changes_fail(self):
        for mutate in (
            lambda p: p["vertices"][0]["attributes"]["location"].__setitem__(2, 9),
            lambda p: p["vertices"][0]["attributes"].update(owner="WRONG"),
            lambda p: p["vertices"].reverse(),
            lambda p: p["vertices"].pop(),
            lambda p: p["seqend"]["attributes"].update(owner="WRONG"),
            lambda p: p["seqend"]["attributes"].update(layer="WRONG"),
        ):
            after = self.changed("combined")
            mutate(after["state"]["polylines"][0])
            self.assertTrue(check.evaluate(check.STYLES["combined"], self.before, after))
        after = self.changed("color")
        next(iter(after["state"]["others"].values()))["attributes"]["color"] = 7
        self.assertTrue(check.evaluate(check.STYLES["color"], self.before, after))

    def test_seqend_accepts_only_original_or_requested_layer(self):
        after = self.changed("layer")
        after["state"]["polylines"][0]["seqend"]["attributes"]["layer"] = "QA_TARGET"
        self.assertEqual(check.evaluate(check.STYLES["layer"], self.before, after), [])

    def test_audit_failures_and_missing_polyline_fail(self):
        after = self.changed("color")
        after["fixes"] = 1
        self.assertIn("external.audit", check.evaluate(check.STYLES["color"], self.before, after))
        after = self.changed("color")
        after["state"]["polylines"] = []
        self.assertTrue(check.evaluate(check.STYLES["color"], self.before, after))

    def test_nonfinite_boolean_and_missing_attributes_fail(self):
        for value in (True, None, float("nan"), float("inf")):
            self.assertTrue(check.differences(2, value))
        self.assertTrue(check.differences({"color": 2}, {}))

    def test_reader_captures_values_before_audit_normalizes_layer(self):
        # ezdxf の保存が不整合を直す前に、親の group 8 だけを変えた生入力を作る。
        lines = self.file.read_text().splitlines()
        parent = next(i for i in range(0, len(lines), 2) if lines[i].strip() == "0" and lines[i + 1] == "POLYLINE")
        layer = next(i for i in range(parent + 2, len(lines), 2) if lines[i].strip() == "8")
        lines[layer + 1] = "QA_TARGET"
        self.file.write_text("\n".join(lines) + "\n")
        found = check.external(self.file)
        polyline, = found["state"]["polylines"]
        self.assertEqual(polyline["parent"]["attributes"]["layer"], "QA_TARGET")
        self.assertEqual([v["attributes"]["layer"] for v in polyline["vertices"]], ["0", "0"])
        self.assertEqual((found["errors"], found["fixes"]), (0, 0))

    def test_all_versions_and_encodings_have_expected_minimal_input(self):
        for version in check.VERSIONS:
            for binary in (False, True):
                with self.subTest(version=version, binary=binary):
                    self.file.write_bytes(check.fixture(version, binary))
                    value = check.external(self.file)
                    self.assertEqual(check.evaluate(check.STYLES["unchanged"], value, value), [])
                    self.assertEqual((value["errors"], value["fixes"]), (0, 0))


if __name__ == "__main__":
    unittest.main()
