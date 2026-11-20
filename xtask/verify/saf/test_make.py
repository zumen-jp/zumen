"""属性・参照・ZIP メンバー・円の不一致を、合成入力で検出する。"""

import copy
import importlib.util
import io
from pathlib import Path
import tempfile
import unittest
import zipfile

spec = importlib.util.spec_from_file_location("saf_make", Path(__file__).with_name("make.py"))
make = importlib.util.module_from_spec(spec)
spec.loader.exec_module(make)


class KitTests(unittest.TestCase):
    def test_xml_rejects_value_name_reference_unit_and_missing_attribute(self):
        data = make.fixtures.saf("DRAWING.SFC")
        self.assertEqual(len(make.xml_fields(data, "DRAWING.SFC")), 2)
        mutations = [(b"102.0400", b"102.0500"), ("測点名".encode(), "別の名前".encode()),
                     (b'sxfFile="DRAWING.SFC"', b'sxfFile="OTHER.SFC"'), (b'Figure id="10"', b'Figure id="11"'),
                     (b'AttrSetRef id="1"', b'AttrSetRef id="2"'), (b'unit="m"', b'unit="mm"'),
                     (b'<Attr name="', b'<Missing name="')]
        for before, after in mutations:
            with self.subTest(after=after), self.assertRaises((ValueError, make.ET.ParseError)):
                make.xml_fields(data.replace(before, after), "DRAWING.SFC")

    def test_binding_rejects_wrong_target_stale_and_mismatched_value(self):
        expected = make.xml_fields(make.fixtures.saf("DRAWING.P21"), "DRAWING.P21")
        bindings = [{"target": "sheet/0/i23", "name": name, "version": "3.1", "current": True, "fields": fields}
                    for name, fields in expected.items()]
        make.check_bindings(bindings, expected, "sheet/0/i23")
        for field, value in [("target", "sheet/0/i27"), ("current", False), ("version", "3.0"), ("name", "別名")]:
            changed = copy.deepcopy(bindings)
            changed[0][field] = value
            with self.subTest(field=field), self.assertRaises(ValueError):
                make.check_bindings(changed, expected, "sheet/0/i23")
        changed = copy.deepcopy(bindings)
        changed[0]["fields"]["value"] = "changed"
        for bad in [changed, bindings[:1], [bindings[0], bindings[0]]]:
            with self.assertRaises(ValueError):
                make.check_bindings(bad, expected, "sheet/0/i23")

    def test_zip_rejects_missing_saf_changed_control_and_broken_figure_link(self):
        original = make.packed("DRAWING.SFC", "DRAWING.SAF", zipfile.ZIP_DEFLATED)
        members = make.members(original, "DRAWING.SFC", "DRAWING.SAF")
        for mutation in ("missing", "control", "link"):
            changed = dict(members)
            if mutation == "missing":
                del changed["DRAWING.SAF"]
            elif mutation == "control":
                changed["control.txt"] = b"changed"
            else:
                changed["DRAWING.SFC"] = changed["DRAWING.SFC"].replace(b"$$ATRF$$10$$", b"$$ATRF$$11$$")
            out = io.BytesIO()
            with zipfile.ZipFile(out, "w") as archive:
                for name, data in changed.items():
                    archive.writestr(name, data)
            with self.subTest(mutation=mutation), self.assertRaises(ValueError):
                make.members(out.getvalue(), "DRAWING.SFC", "DRAWING.SAF")

    def test_p21_circle_rejects_external_center_radius_and_source_mismatch(self):
        data = make.fixtures.p21("DRAWING.SAF")
        # source.instance は外部の STEP インスタンス番号。モデルの ID だけに頼らない。
        element = {"id": "sheet/0/i27", "source": {"instance": 27}, "layer": "L",
                   "geometry": {"shape": "circle", "center": [50, 25], "radius": 10}}
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / "test.p21"
            path.write_bytes(data)
            self.assertTrue(make.p21_circle(path, element, False)["matched"])
            for changed in [data.replace(b"(50.,25.)", b"(51.,25.)"), data.replace(b"#25,10.)", b"#25,11.)")]:
                self.assertNotEqual(changed, data)
                path.write_bytes(changed)
                with self.assertRaises(ValueError):
                    make.p21_circle(path, element, False)
            path.write_bytes(data)
            element["source"]["instance"] = 23
            with self.assertRaises(ValueError):
                make.p21_circle(path, element, False)

    def test_p21_sheet_rejects_lost_references_dimensions_and_units(self):
        data = make.fixtures.p21("DRAWING.SAF")
        for before, after in [(b"PRESENTATION_SIZE(#121,#122)", b"PRESENTATION_SIZE(#121,#10)"),
                              (b"420.,297.", b"297.,420."), (b"TITLE((#125)", b"TITLE((#121)"),
                              (b"420.,297.,#11", b"420.,297.,#10"), (b".MILLI.", b".CENTI.")]:
            with self.subTest(before=before), tempfile.TemporaryDirectory() as folder:
                path = Path(folder) / "wrong.p21"
                changed = data.replace(before, after)
                self.assertNotEqual(changed, data)
                path.write_bytes(changed)
                step = make.check_ezsxf.Step(make.check_ezsxf.ezsxf.parse_p21(str(path), strict=False), {})
                with self.assertRaises(ValueError):
                    make.p21_sheet(step)

    def test_rejects_nonempty_and_repository_output(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            (root / ".git").write_text("gitdir: unused", encoding="utf8")
            with self.assertRaises(ValueError):
                make.output_folder(root / "output")
            (root / ".git").unlink()
            (root / "keep.txt").write_text("keep", encoding="utf8")
            with self.assertRaises(ValueError):
                make.output_folder(root)


if __name__ == "__main__":
    unittest.main()
