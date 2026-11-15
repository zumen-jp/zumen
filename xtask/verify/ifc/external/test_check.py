"""実際の公開モデルの編集出力を改変し、外部比較器が不一致を見逃さないことを確かめる。"""

from collections import Counter
import importlib.util
from pathlib import Path
import sys
import unittest

import ifcopenshell

spec = importlib.util.spec_from_file_location("ifc_external", Path(__file__).with_name("check.py"))
check = importlib.util.module_from_spec(spec)
spec.loader.exec_module(check)


class ExternalTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        check.verify_kit(KIT)
        cls.before = {schema: ifcopenshell.open(str(KIT / schema / "input.ifc")) for schema in check.kit.CASES}

    def edited(self, schema):
        return ifcopenshell.open(str(KIT / schema / "edited.ifc"))

    def target(self, model, schema):
        return model.by_id(check.kit.CASES[schema]["record"])

    def prop(self, model, schema, set_name, name):
        found = []
        for relation in self.target(model, schema).IsDefinedBy:
            if relation.is_a("IfcRelDefinesByProperties"):
                group = relation.RelatingPropertyDefinition
                if group.is_a("IfcPropertySet") and group.Name == set_name:
                    found += [prop for prop in group.HasProperties if prop.Name == name]
        self.assertEqual(len(found), 1)
        return found[0]

    def reject(self, schema, model):
        with self.assertRaises(ValueError):
            check.compare_models(self.before[schema], model, schema)

    def test_two_fixed_outputs_match(self):
        for schema in check.kit.CASES:
            with self.subTest(schema=schema):
                result = check.compare_models(self.before[schema], self.edited(schema), schema)
                self.assertEqual(len(result["properties"]), 4)
                self.assertGreater(result["unchangedRecords"], 5000)
                self.assertGreater(result["unchangedObjectProperties"], 50)

    def test_names_and_descriptions_are_checked(self):
        for schema in check.kit.CASES:
            for field in ("Name", "Description"):
                with self.subTest(schema=schema, field=field):
                    model = self.edited(schema)
                    setattr(self.target(model, schema), field, "Different")
                    self.reject(schema, model)

    def test_all_values_and_nominal_types_are_checked(self):
        for schema in check.kit.CASES:
            for set_name, name, type_name, value in (
                ("Pset_WallCommon", "IsExternal", "IfcBoolean", True),
                ("Zumen_Check526", "Note", "IfcLabel", "Different"),
                ("Zumen_Check526", "Ratio", "IfcReal", 1.5),
                ("Zumen_Check526", "Count", "IfcInteger", 8),
                ("Pset_WallCommon", "IsExternal", "IfcLogical", False),
                ("Zumen_Check526", "Note", "IfcText", "検証用の備考"),
                ("Zumen_Check526", "Ratio", "IfcRatioMeasure", 1.25),
                ("Zumen_Check526", "Count", "IfcReal", 7.0),
            ):
                with self.subTest(schema=schema, property=name, type=type_name):
                    model = self.edited(schema)
                    self.prop(model, schema, set_name, name).NominalValue = model.create_entity(type_name, value)
                    self.reject(schema, model)

    def test_unrelated_coordinate_and_target_references_are_checked(self):
        for schema in check.kit.CASES:
            for field in ("coordinate", "ObjectPlacement", "Representation", "GlobalId"):
                with self.subTest(schema=schema, field=field):
                    model = self.edited(schema)
                    if field == "coordinate":
                        point = model.by_type("IfcCartesianPoint")[0]
                        values = list(point.Coordinates)
                        values[0] += 1
                        point.Coordinates = values
                    else:
                        setattr(self.target(model, schema), field, "0000000000000000000000" if field == "GlobalId" else None)
                    self.reject(schema, model)

    def test_shared_control_and_property_metadata_are_checked(self):
        model = self.edited("ifc4")
        control = model.by_guid(check.kit.recipe("ifc4")["control"])
        found = [prop for rel in control.IsDefinedBy if rel.is_a("IfcRelDefinesByProperties")
                 and rel.RelatingPropertyDefinition.is_a("IfcPropertySet")
                 and rel.RelatingPropertyDefinition.Name == "Pset_WallCommon"
                 for prop in rel.RelatingPropertyDefinition.HasProperties if prop.Name == "IsExternal"]
        self.assertEqual(len(found), 1)
        found[0].NominalValue = model.create_entity("IfcBoolean", False)
        self.reject("ifc4", model)
        for schema in check.kit.CASES:
            model = self.edited(schema)
            self.prop(model, schema, "Zumen_Check526", "Note").Description = "Unexpected"
            self.reject(schema, model)

    def test_missing_records_and_orphan_additions_are_checked(self):
        for schema in check.kit.CASES:
            for mode in ("missing", "orphan"):
                with self.subTest(schema=schema, mode=mode):
                    model = self.edited(schema)
                    if mode == "missing":
                        model.remove(model.by_type("IfcCartesianPoint")[0])
                    else:
                        model.create_entity("IfcPropertySingleValue", Name="Orphan", NominalValue=model.create_entity("IfcInteger", 1))
                    self.reject(schema, model)

    def test_added_relationships_and_duplicate_properties_are_checked(self):
        for schema in check.kit.CASES:
            old = {e.id() for e in self.before[schema]}
            for mode in ("control", "duplicate", "gid"):
                with self.subTest(schema=schema, mode=mode):
                    model = self.edited(schema)
                    group = next(e for e in model.by_type("IfcPropertySet") if e.id() not in old)
                    if mode == "control":
                        relation = next(e for e in model.by_type("IfcRelDefinesByProperties") if e.id() not in old)
                        relation.RelatedObjects = (model.by_guid(check.kit.recipe(schema)["control"]),)
                    elif mode == "duplicate":
                        group.HasProperties = (*group.HasProperties, group.HasProperties[0])
                    else:
                        group.GlobalId = self.target(model, schema).GlobalId
                    self.reject(schema, model)

    def test_boolean_number_and_missing_set_names_do_not_collide(self):
        self.assertTrue(check.same({None: {"x": 1}, "A": {}}, {"A": {}, None: {"x": 1}}))
        self.assertFalse(check.same({None: 1}, {"None": 1}))
        self.assertFalse(check.same({"x": False}, {"x": 0}))
        self.assertFalse(check.same({"x": 7}, {"x": 7.0}))

    def test_audit_multiplicity_and_new_findings_do_not_pass(self):
        key = ("error", "schema", 42, "Attribute", "0" * 64)
        before = Counter({key: 2})
        self.assertEqual(check.compare_audits(before, before)["added"], [])
        with self.assertRaises(ValueError):
            check.compare_audits(before, Counter({key: 3}))
        other = ("warning", "entity_rule", 43, "Rule", "1" * 64)
        with self.assertRaises(ValueError):
            check.compare_audits(before, before + Counter({other: 1}))


if __name__ == "__main__":
    if len(sys.argv) != 2:
        sys.exit("使い方: python test_check.py <IFCのキット>")
    KIT = Path(sys.argv.pop()).resolve()
    unittest.main()
