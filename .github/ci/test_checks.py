"""差分に応じて選ぶテストと、定期実行・PR で必ず流すテストを確かめる。"""

import unittest

from checks import ALL, NATIVE, WASM, select


class SelectionTest(unittest.TestCase):
    def test_tests_stay_in_their_crate_but_implementation_reaches_users(self):
        tests = "crates/zumen-ifc/tests/units.rs"
        plan = select([tests], "push")
        self.assertEqual(plan["cargo_args"], "-p zumen-ifc")
        self.assertEqual(plan["codecs"], "zumen-ifc")
        self.assertFalse({"native", "wasm"} & set(plan["checks"]))

        plan = select([tests, "crates/zumen-ifc/src/lib.rs"], "push")
        self.assertTrue({"xtask", "zumen-ifc", *NATIVE} <= set(plan["cargo_args"].split()))
        self.assertTrue({"native", "abi", *WASM} <= set(plan["checks"]))

    def test_shared_inputs_and_required_pr_checks(self):
        for path in ["crates/zumen-render/tests/fixtures/mod.rs", "bindings/c/include/zumen.h"]:
            self.assertIn("wasm", select([path], "push")["checks"])
        docs = ["CONTRIBUTING.md", "packages/view2d/README.md", "fuzz/README.md", "xtask/release/README.md"]
        self.assertEqual(select(docs, "push")["checks"], ["text"])
        self.assertIn("site", select([*docs, "packages/view2d/src/index.js"], "push")["checks"])
        pr = select(["CONTRIBUTING.md"], "pull_request")
        self.assertTrue({"native", "codegen", "deny", *WASM} <= set(pr["checks"]))
        self.assertEqual(pr["linux_args"], pr["native_args"])
        self.assertTrue(NATIVE <= set(pr["linux_args"].split()))

    def test_selector_only_push_runs_its_own_tests(self):
        files = [".github/ci/checks.py", ".github/ci/test_checks.py"]
        self.assertEqual(select(files, "push")["checks"], ["selector"])
        plan = select([*files, "bindings/go/input_test.go"], "push")
        self.assertEqual(plan["checks"], ["go", "selector"])
        self.assertEqual(set(select([".github/workflows/ci.yml"], "push")["checks"]), ALL)

    def test_wasm_consumers_on_push(self):
        cases = [
            (["bindings/go/conformance_test.go"], {"go"}),
            (["bindings/go/messages.go"], {"go", "codegen"}),
            (["bindings/js/test/browser/view2d.mjs", "packages/view2d/test/browser.mjs"], {"view2d"}),
            (["packages/view2d/src/raster.js"], {"view2d", "react", "site"}),
            (["packages/react/src/client.js", "playground/src/playground.jsx"], {"react", "site"}),
            (["playground/test/lifecycle.jsx", "bindings/js/test/browser/react.mjs"], {"react"}),
            (["bindings/js/src/index.js"], {"wasm", "view2d", "react", "site", "codegen"}),
            (["bindings/go/input_test.go", "playground/test/lifecycle.jsx"], {"go", "react"}),
        ]
        for paths, expected in cases:
            with self.subTest(paths=paths):
                plan = select(paths, "push")
                self.assertEqual(set(plan["checks"]), expected)

    def test_builds_and_browser_setup_follow_the_consumer(self):
        all_modules = "drawing bim civil pointcloud plugin"
        cases = [
            (["CONTRIBUTING.md"], "push", "", False, False),
            (["bindings/go/input_test.go"], "push", "", True, False),
            (["packages/view2d/test/browser.mjs"], "push", "", True, True),
            (["playground/src/main.jsx"], "push", "drawing", False, True),
            (["bindings/go/input_test.go", "playground/src/main.jsx"], "push", "drawing", True, True),
            (["CONTRIBUTING.md"], "pull_request", all_modules, True, True),
            (["unknown"], "push", all_modules, True, True),
        ]
        for paths, event, modules, full, browser in cases:
            with self.subTest(paths=paths, event=event):
                plan = select(paths, event)
                self.assertEqual((plan["wasm_modules"], plan["wasm_full"], plan["wasm_browser"]),
                                 (modules, full, browser))

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
