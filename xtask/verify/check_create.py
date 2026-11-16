"""create と直後の apply が書く DXF を、ezdxf の読みと監査で照らす。"""

import sys
from importlib.metadata import version
from pathlib import Path

import ezdxf


def check(folder):
    units = {"default": 4, "millimeter": 4, "centimeter": 5,
             "meter": 6, "inch": 1, "foot": 2, "unknown": 0}
    for unit, value in units.items():
        for state in ["empty", "edited"]:
            doc = ezdxf.readfile(folder / f"{state}-{unit}.dxf")
            assert doc.dxfversion == "AC1018"
            assert doc.header["$INSUNITS"] == value
            assert doc.layers.has_entry("0")
            assert all(doc.linetypes.has_entry(n) for n in ["ByBlock", "ByLayer", "Continuous"])
            assert doc.styles.has_entry("Standard")
            assert doc.dimstyles.has_entry("Standard")
            assert doc.appids.has_entry("ACAD")
            assert set(doc.layouts.names()) == {"Model", "Layout1"}
            for layout in doc.layouts:
                block = layout.block_record
                assert doc.entitydb[block.dxf.layout] is layout.dxf_layout
                assert layout.dxf_layout.dxf.block_record_handle == block.dxf.handle
            assert len(doc.layout("Layout1")) == 0
            model = doc.modelspace()
            if state == "empty":
                assert len(model) == 0
                # ezdxf が読み取り時に補う Defpoints は、書いたレイヤに数えない。
                assert b"\nDefpoints\n" not in (folder / f"{state}-{unit}.dxf").read_bytes()
                assert {e.dxf.name for e in doc.layers} - {"Defpoints"} == {"0"}
            else:
                assert len(model) == 3
                line, circle, text = list(model)
                assert [e.dxftype() for e in model] == ["LINE", "CIRCLE", "TEXT"]
                assert list(line.dxf.start) == [1, 2, 0]
                assert list(line.dxf.end) == [3, 4, 0]
                assert line.dxf.layer == "Draft"
                assert doc.layers.get("Draft").dxf.color == 3
                assert list(circle.dxf.center) == [8, 9, 0]
                assert circle.dxf.radius == 2
                assert list(text.dxf.insert) == [10, 20, 0]
                assert text.dxf.text == "Test" and text.dxf.style == "Standard"
                assert text.dxf.height == 2.5
            audit = doc.audit()
            assert not audit.errors and not audit.fixes, (state, unit, audit.errors, audit.fixes)
    print(f"ezdxf {version('ezdxf')}: 14 ファイルの単位・表・参照・図形が一致、監査の誤りと修復は 0")


if __name__ == "__main__":
    check(Path(sys.argv[1]))
