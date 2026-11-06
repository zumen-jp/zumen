"""ezdxf で作った画層の説明と透過性を、公開 API で R2004 DXF に変換して照らす（#635）。"""

import io
import sys
from pathlib import Path

import ezdxf
from ezdxf import recover
from ezdxf.lldxf.tagger import ascii_tags_loader
from ezdxf.lldxf.tags import group_tags

sys.path.insert(0, str(Path(__file__).resolve().parent))
from make import Zumen  # noqa: E402

DESCRIPTION = "AcAecLayerStandard"
TRANSPARENCY = "AcCmTransparency"


def check(z, name, description, transparency, number):
    doc = ezdxf.new("R2004")
    doc.encoding = "cp932"
    layer = doc.layers.get("0") if name == "0" else doc.layers.new(name)
    other = doc.layers.new("予備")
    doc.modelspace().add_line((0, 0), (3, 4), dxfattribs={"layer": name})
    if description is not None:
        layer.description = description
    if transparency is not None:
        layer.transparency = transparency
    if description is not None and transparency is not None:
        shared = doc.layers.new("共通")
        shared.description, shared.transparency = "同じ APPID を使う画層", 1.0
    text = io.StringIO()
    doc.write(text)
    data = text.getvalue().encode(doc.output_encoding)
    source, target = f"in{number}.dxf", f"out{number}.dxf"
    z.push(source, data)
    assert z.call({"op": "open", "input": source})[0] == "ok"
    status, response = z.call({"op": "convert", "doc": source, "format": "dxf", "to": target})
    assert status == "ok"
    assert not any(d["code"] == "convert.layer-field.dropped" for d in response["diagnostics"])
    output = z.take(0)
    written, auditor = recover.read(io.BytesIO(output))
    assert written.dxfversion == "AC1018" and not auditor.errors and not auditor.fixes
    result = written.layers.get(name)
    assert (result.description, result.transparency) == (layer.description, layer.transparency)
    # ezdxf が補う前の APPID と生の XDATA も比べる。明示した不透明度と値なしを区別する。
    records = list(group_tags(ascii_tags_loader(io.StringIO(output.decode("cp932")))))
    table, = (r for r in records if r.dxftype() == "TABLE" and r.get_first_value(2) == "APPID")
    rows = [r for r in records if r.dxftype() == "APPID"]
    assert int(table.get_first_value(70)) == len(rows)
    assert all(r.get_first_value(330) == table.get_first_value(5) for r in rows)
    ids = [int(r.get_first_value(5), 16) for r in [table, *rows]]
    assert len(set(ids)) == len(ids) and max(ids) < int(written.header["$HANDSEED"], 16)
    registrations = [row.get_first_value(2) for row in rows]
    for app in (DESCRIPTION, TRANSPARENCY):
        present = layer.has_xdata(app)
        assert registrations.count(app) == int(present)
        assert result.has_xdata(app) == present
        if present:
            assert result.get_xdata(app) == layer.get_xdata(app)
        if description is not None and transparency is not None:
            assert written.layers.get("共通").get_xdata(app) == shared.get_xdata(app)
        assert not written.layers.get(other.dxf.name).has_xdata(app)
    assert any(e.dxf.layer == name for e in written.modelspace().query("LINE"))
    for opened, expected in [(source, data), (target, output)]:
        assert z.call({"op": "apply", "doc": opened, "edits": []})[0] == "ok"
        assert z.take(0) == expected


def main():
    if len(sys.argv) != 2:
        raise SystemExit("使い方: python -I test_dxf_layer_fields.py <zumen>")
    z = Zumen(sys.argv[1])
    try:
        cases = [("外壁の仕上げ", None), (None, 0.5), ("図" * 127 + "A", 0.0)]
        for index, layer in enumerate(("0", "外壁")):
            for offset, (description, transparency) in enumerate(cases):
                check(z, layer, description, transparency, index * len(cases) + offset)
    finally:
        z.close()
    print("画層 0 と追加画層の 6 通りで、説明・透過性・APPID が一致し、監査の誤り・修復は 0 件")


if __name__ == "__main__":
    main()
