"""差分に応じて選ぶテストと、定期実行・PR で必ず流すテストを確かめる。"""

import unittest

from checks import ALL, NATIVE, select


class SelectionTest(unittest.TestCase):
    def test_tests_stay_in_their_crate_but_implementation_reaches_users(self):
        tests = "crates/zumen-ifc/tests/units.rs"
        plan = select([tests], "push")
        self.assertEqual(plan["cargo_args"], "-p zumen-ifc")
        self.assertEqual(plan["codecs"], "zumen-ifc")
        self.assertFalse({"native", "wasm"} & set(plan["checks"]))

        plan = select([tests, "crates/zumen-ifc/src/lib.rs"], "push")
        self.assertTrue({"xtask", "zumen-ifc", *NATIVE} <= set(plan["cargo_args"].split()))
        self.assertTrue({"native", "wasm", "abi"} <= set(plan["checks"]))

    def test_shared_inputs_and_required_pr_checks(self):
        for path in ["crates/zumen-render/tests/fixtures/mod.rs", "bindings/c/include/zumen.h"]:
            self.assertIn("wasm", select([path], "push")["checks"])
        self.assertEqual(select(["CONTRIBUTING.md"], "push")["checks"], ["text"])
        pr = select(["CONTRIBUTING.md"], "pull_request")
        self.assertTrue({"native", "wasm", "codegen", "deny"} <= set(pr["checks"]))
        self.assertEqual(pr["linux_args"], pr["native_args"])
        self.assertTrue(NATIVE <= set(pr["linux_args"].split()))

    def test_full_runs_and_unclassified_changes(self):
        for event in ["schedule", "workflow_dispatch"]:
            plan = select([], event)
            self.assertEqual(set(plan["checks"]), ALL)
            self.assertEqual(plan["native_args"], "--workspace")
            self.assertEqual(plan["linux_args"], "--workspace")
        for paths in [None, ["Cargo.lock", "crates/zumen-ifc/tests/units.rs"], ["unknown"]]:
            plan = select(paths, "push")
            self.assertEqual(set(plan["checks"]), ALL)
            self.assertEqual(plan["cargo_args"], "--workspace")
