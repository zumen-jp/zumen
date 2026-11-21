"""水平な DXF 寸法を変換し、実寸、紙への配置、表示文字を ezsxf の解析値から照らす。"""

import argparse
from importlib.metadata import version
import math
from pathlib import Path
import subprocess
import sys
import tempfile

import ezdxf
import ezsxf

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))
from check_ezsxf import Step


def make(path, label):
    doc = ezdxf.new("R2018")
    doc.units = 4  # ミリメートル
    style = {"dimtxt": 25, "dimasz": 20, "dimlfac": 1}
    dimension = doc.modelspace().add_linear_dim(base=(0, 100), p1=(0, 0), p2=(1000, 0),
                                                angle=0, text=label, dimstyle="Standard", override=style)
    dimension.render()
    dimension.dimension.dxf.actual_measurement = 1000
    sheet = doc.layout()
    sheet.page_setup(size=(420, 297), margins=(0, 0, 0, 0))
    sheet.add_viewport(center=(120, 90), size=(200, 100), view_center_point=(500, 0), view_height=1000)
    doc.saveas(path)


def only(step, items, kind):
    found, = [item for item in items if step.has(item, kind)]
    return found


def check_p21(path, expected_text):
    parsed = ezsxf.parse_p21(path.read_bytes(), strict=True)
    assert not parsed["warnings"], parsed["warnings"]
    step = Step(parsed, {})
    all_items = step.order
    unit = only(step, all_items, "LENGTH_UNIT")
    assert step.part(unit, "SI_UNIT") == [{"kind": "enum", "value": s} for s in ("MILLI", "METRE")]
    sheet = only(step, all_items, "DRAWING_SHEET_REVISION")
    sheet_items = step.refs(step.part(sheet, "DRAWING_SHEET_REVISION")[1])
    box = only(step, sheet_items, "PLANAR_BOX")
    assert step.part(box, "PLANAR_BOX")[1:3] == [420.0, 297.0]
    placed = only(step, sheet_items, "ANNOTATION_SUBFIGURE_OCCURRENCE")
    mapping, transform = step.get(step.styled(placed)[1], "MAPPED_ITEM")
    base, definition = step.get(mapping, "SYMBOL_REPRESENTATION_MAP")
    assert step.point(step.get(base, "AXIS2_PLACEMENT_2D")[1]) == [0, 0]
    target = step.get(transform, "SYMBOL_TARGET")
    offset = step.point(step.get(target[1], "AXIS2_PLACEMENT_2D")[1])
    assert offset == [70, 90] and target[2:] == [0.1, 0.1]
    model_items = step.refs(step.get(definition, "DRAUGHTING_SUBFIGURE_REPRESENTATION")[1])
    dimension = only(step, model_items, "LINEAR_DIMENSION")
    assert dimension == only(step, all_items, "LINEAR_DIMENSION")
    members = step.refs(step.part(dimension, "DRAUGHTING_CALLOUT")[0])
    line = only(step, members, "DIMENSION_CURVE")
    assert step.shape(line) == {"shape": "line", "start": [0, 100], "end": [1000, 100]}
    projection = sorted((step.shape(i)["start"], step.shape(i)["end"])
                        for i in members if step.has(i, "PROJECTION_CURVE"))
    assert projection == [([0, 0], [0, 100]), ([1000, 0], [1000, 100])]
    assert math.dist(projection[0][0], projection[1][0]) == 1000
    assert [[offset[j] + p[j] * target[2+j] for j in (0, 1)] for p, _ in projection] == [[70, 90], [170, 90]]
    assert not any(step.has(i, "DIMENSION_CURVE_TERMINATOR") for i in all_items)
    text = only(step, members, "ANNOTATION_TEXT_OCCURRENCE")
    assert step.get(step.styled(text)[1], "TEXT_LITERAL_WITH_EXTENT")[1] == expected_text
    relation = only(step, all_items, "DIMENSION_CALLOUT_RELATIONSHIP")
    source, structured = step.refs(step.part(relation, "DIMENSION_CALLOUT_RELATIONSHIP")[2:])
    assert source == dimension and step.has(structured, "STRUCTURED_DIMENSION_CALLOUT")
    assert step.refs(step.part(structured, "DRAUGHTING_CALLOUT")[0]) == [text]


def run(folder, cli, wasm):
    assert version("ezdxf") == "1.4.5" and version("ezsxf") == "0.3.7"
    for name, label in (("measured", "<>"), ("override", "995")):
        make(folder / f"{name}.dxf", label)
    subprocess.run(["node", str(HERE / "convert.mjs"), str(folder), str(cli.resolve()), str(wasm.resolve())],
                   check=True, timeout=180)
    for name, text in (("measured", "1000"), ("override", "995")):
        check_p21(folder / f"{name}.p21", text)
    print("ezsxf 0.3.7: 2 P21 dimensions, 1000 mm, 1:10, labels 1000 / 995")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", required=True, type=Path)
    parser.add_argument("--wasm", required=True, type=Path)
    parser.add_argument("--out", type=Path, help="実機で開くファイルを残す、リポジトリ外の新しいフォルダー")
    args = parser.parse_args()
    if args.out:
        folder = args.out.resolve()
        if any((parent / ".git").exists() for parent in (folder, *folder.parents)):
            parser.error("出力先はリポジトリの外にしてください")
        folder.mkdir()
        run(folder, args.cli, args.wasm)
    else:
        with tempfile.TemporaryDirectory() as tmp:
            run(Path(tmp), args.cli, args.wasm)
