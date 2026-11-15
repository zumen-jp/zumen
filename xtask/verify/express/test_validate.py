"""件数だけの一致・未分類の無視・規則モードの混同を成功扱いしない。"""

import copy
import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location("express_validate", Path(__file__).with_name("validate.py"))
check = importlib.util.module_from_spec(spec)
spec.loader.exec_module(check)


class ComparisonTests(unittest.TestCase):
    def pair(self):
        core = {"diagnostics": [{"code": "express.rule.unsupported"}],
                "result": {"schema": "IFC4", "file_schema": "IFC4", "total": 1, "findings": [
                    {"code": "express.value.missing", "count": 1, "at": [{"id": "#1"}],
                     "params": {"kind": "IFCCARTESIANPOINT", "attribute": "COORDINATES"}}]}}
        mode = {"count": 1, "explicit": [{"id": 1, "entity": "IFCCARTESIANPOINT", "attribute": "COORDINATES",
                                          "reason": "express.value.missing"}],
                "inverse": [], "header": [], "rules": [], "unknown": []}
        external = {"schema": "IFC4", "headerAlias": False,
                    "modes": [{"expressRules": flag, **copy.deepcopy(mode)} for flag in (False, True)]}
        return core, external

    def test_compares_locations_attributes_reasons_and_multiplicity(self):
        core, external = self.pair()
        self.assertEqual(check.compare(core, external), [])
        for field, value in (("id", 2), ("entity", "IFCDIRECTION"), ("attribute", "OTHER"), ("reason", "express.value.type")):
            changed = copy.deepcopy(external)
            for mode in changed["modes"]:
                mode["explicit"][0][field] = value
            self.assertIn("explicit-findings", check.compare(core, changed))
        changed = copy.deepcopy(external)
        for mode in changed["modes"]:
            mode["explicit"] *= 2
            mode["count"] = 2
        self.assertIn("explicit-findings", check.compare(core, changed))

    def test_rejects_incomplete_unknown_and_switched_modes(self):
        core, external = self.pair()
        core["result"]["next_offset"] = 1
        self.assertIn("core-incomplete", check.compare(core, external))
        core, external = self.pair()
        external["modes"][0]["unknown"].append({"type": "new-check"})
        external["modes"][0]["count"] += 1
        self.assertIn("external-unclassified", check.compare(core, external))
        core, external = self.pair()
        external["modes"].reverse()
        self.assertIn("external-modes", check.compare(core, external))

    def test_keeps_rule_additions_separate_and_preserves_base_findings(self):
        core, external = self.pair()
        full = external["modes"][1]
        full["rules"].append({"type": "entity_rule", "attribute": "IfcCartesianPoint.CP2DOr3D"})
        full["count"] += 1
        self.assertEqual(check.compare(core, external), [])
        full["explicit"].clear()
        full["count"] -= 1
        self.assertIn("rule-mode-changed-explicit", check.compare(core, external))

    def test_classifies_real_missing_and_inverse_attributes(self):
        import ifcopenshell
        import ifcopenshell.validate
        model = ifcopenshell.file(schema="IFC4")
        point = model.create_entity("IfcCartesianPoint", Coordinates=None)
        logger = ifcopenshell.validate.json_logger()
        ifcopenshell.validate.validate(model, logger, express_rules=False)
        found = check.normalize_external(logger.statements, ifcopenshell.schema_by_name("IFC4"))
        self.assertEqual(found["explicit"], [{"id": point.id(), "entity": "IFCCARTESIANPOINT", "attribute": "COORDINATES",
                                              "reason": "express.value.missing"}])
        self.assertEqual(found["unknown"], [])
        model = ifcopenshell.file(schema="IFC2X3")
        item = model.create_entity("IfcPropertySet")
        row = {"type": "schema", "level": "error", "instance": item,
               "attribute": "IfcPropertySet.DefinesType", "message": "With inverse:"}
        schema = ifcopenshell.schema_by_name("IFC2X3")
        self.assertEqual(len(check.normalize_external([row], schema)["inverse"]), 1)
        row["attribute"] = "IfcPropertySet.UnknownAttribute"
        self.assertEqual(len(check.normalize_external([row], schema)["unknown"]), 1)

    def test_does_not_treat_arbitrary_unlocated_errors_as_headers(self):
        import ifcopenshell
        row = {"type": "schema", "level": "error", "message": "For instance:\nFILE_NAME($);\nAttribute 'name' has invalid type:\nCurrent value type: NoneType"}
        schema = ifcopenshell.schema_by_name("IFC4")
        self.assertEqual(len(check.normalize_external([row], schema)["header"]), 1)
        row["message"] = "Unknown parser error"
        self.assertEqual(len(check.normalize_external([row], schema)["unknown"]), 1)

    def test_checks_exact_external_bytes_and_only_the_declared_header_alias(self):
        data = b"FILE_SCHEMA(('IFC4X3')); DATA; #1=IFCCARTESIANPOINT((0.,0.)); ENDSEC;"
        expected = data.replace(b"'IFC4X3'", b"'IFC4X3_ADD2'", 1)
        external = {"headerAlias": True, "inputSha256": check.common.sha(expected)}
        self.assertEqual(check.input_differences(data, external), [])
        external["inputSha256"] = check.common.sha(expected.replace(b"0.,0.", b"1.,0."))
        self.assertEqual(check.input_differences(data, external), ["external-input"])
        external["headerAlias"] = False
        self.assertIn("header-alias", check.input_differences(data, external))


if __name__ == "__main__":
    unittest.main()
