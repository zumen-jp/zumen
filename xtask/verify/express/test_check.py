"""宣言の不一致と、外部スキーマを読む子プロセスの正常終了を確かめる。"""

import copy
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

spec = importlib.util.spec_from_file_location("express_check", Path(__file__).with_name("check.py"))
check = importlib.util.module_from_spec(spec)
spec.loader.exec_module(check)


def observation():
    return {"schema": "LANE5_TEST", "entities": [{"name": "ITEM", "abstract": False, "supertypes": [],
            "attributes": [{"name": "VALUES", "optional": True, "type": {
                "levels": [{"kind": "LIST", "lower": 1, "upper": None}], "item": "INTEGER"}}]}]}


class ComparisonTests(unittest.TestCase):
    def test_detects_names_supertype_abstract_optionality_type_and_bounds(self):
        original = observation()
        self.assertEqual(check.differences(original, copy.deepcopy(original)), [])
        for mutate in (
            lambda row: row.update(name="OTHER"),
            lambda row: row.update(abstract=True),
            lambda row: row.update(supertypes=["BASE"]),
            lambda row: row["attributes"][0].update(name="OTHER"),
            lambda row: row["attributes"][0].update(optional=False),
            lambda row: row["attributes"][0]["type"].update(item="REAL"),
            lambda row: row["attributes"][0]["type"]["levels"][0].update(kind="SET"),
            lambda row: row["attributes"][0]["type"]["levels"][0].update(lower=0),
            lambda row: row["attributes"][0]["type"]["levels"][0].update(upper=10),
            lambda row: row.update(attributes=[]),
        ):
            changed = copy.deepcopy(original)
            mutate(changed["entities"][0])
            self.assertTrue(check.differences(original, changed))

    def test_rejects_missing_duplicate_and_different_schema(self):
        original = observation()
        for changed in ({**original, "schema": "OTHER"}, {**original, "entities": []},
                        {**original, "entities": original["entities"] * 2}):
            self.assertTrue(check.differences(original, changed))

    def test_external_worker_finishes_and_reads_own_schema(self):
        with tempfile.TemporaryDirectory(prefix="zumen-express-unit-") as folder:
            temporary = Path(folder).resolve()
            self.assertEqual(temporary.parent, Path(tempfile.gettempdir()).resolve())
            source, output = temporary / "own.exp", temporary / "observed.json"
            source.write_text("SCHEMA LANE5_TEST; ENTITY ITEM; VALUES : OPTIONAL LIST [1:?] OF INTEGER; END_ENTITY; END_SCHEMA;", encoding="ascii")
            result = subprocess.run([sys.executable, "-I", str(Path(check.__file__)), "--worker", str(source), "--out", str(output)],
                                    capture_output=True, timeout=30)
            self.assertEqual(result.returncode, 0)
            self.assertEqual(json.loads(output.read_text()), observation())


if __name__ == "__main__":
    unittest.main()
