"""角度と非対象の不一致を、外部照合で検出する。"""

import copy
import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location("mtext_check", Path(__file__).with_name("check.py"))
check = importlib.util.module_from_spec(spec)
spec.loader.exec_module(check)


class ComparisonTests(unittest.TestCase):
    def test_shared_radians_mistake_is_not_hidden_by_core_roundtrip(self):
        control = {"rotation": [0], "lines": [{"start": [0, 0, 0], "end": [3, 4, 0]}], "errors": 0, "fixes": 0}
        observed = {**control, "rotation": [35 * check.math.pi / 180]}
        self.assertEqual(check.evaluate(35, 35, observed, control), ["external.rotation"])
        self.assertEqual(check.evaluate(35, 35 * 180 / check.math.pi, {**control, "rotation": [35]}, control), ["core.rotation"])

    def test_non_target_and_audit_failures_are_not_success(self):
        control = {"rotation": [35], "lines": [{"start": [0, 0, 0], "end": [3, 4, 0]}], "errors": 0, "fixes": 0}
        self.assertEqual(check.evaluate(35, 35, control, control), [])
        changed = copy.deepcopy(control)
        changed["lines"][0]["end"][0] = 99
        changed["fixes"] = 1
        self.assertEqual(check.evaluate(35, 35, changed, control), ["non_target.line", "external.audit"])

    def test_missing_and_non_finite_rotation_are_rejected(self):
        for actual in (None, True, float("nan"), float("inf")):
            self.assertFalse(check.close(35, actual))


if __name__ == "__main__":
    unittest.main()
