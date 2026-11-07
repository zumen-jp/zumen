"""#316 の実機確認用に、対象の書式をまだ設定していない R2018 の ASCII DXF を作る。"""

import argparse
import hashlib
import json
from pathlib import Path
import shutil

import ezdxf
from ezdxf.math import Vec2
from ezdxf.render.mleader import ConnectionSide


def write_fixture(path):
    doc = ezdxf.new("R2018")
    doc.units = 4  # mm
    model = doc.modelspace()
    for row, key in enumerate(("REGULAR", "BOLD", "ITALIC", "BOTH")):
        name = "Z316_" + key
        style = doc.styles.new(name, dxfattribs={"font": "arial.ttf"})
        style.set_extended_font_data("Arial")  # 4 つとも同じ標準書体で始める。
        model.add_text(name, dxfattribs={"style": name, "height": 2.5, "insert": (0, 80 - 6 * row)})
    for row, (key, height) in enumerate((
        ("CONTROL", 2.5), ("RGB_A", 2.5), ("RGB_B", 2.5), ("PARA_25", 2.5), ("PARA_50", 5.0),
    )):
        name = "Z316_" + key
        doc.layers.new(name)
        model.add_mtext(name, dxfattribs={
            "layer": name, "style": "Z316_REGULAR", "char_height": height,
            "width": 80, "insert": (0, 52 - 8 * row),
        })
    doc.layers.new("Z316_LEADER")
    doc.linetypes.add("Z316_DASH", [6.0, 3.0, -3.0], description="Z316")
    builder = model.add_multileader_mtext(dxfattribs={"layer": "Z316_LEADER"})
    builder.set_content("Z316", char_height=2.5, style="Z316_REGULAR")
    builder.add_leader_line(ConnectionSide.left, [Vec2(-30, 0)])
    builder.add_leader_line(ConnectionSide.left, [Vec2(30, 0)])
    builder.build(Vec2(45, 10))
    doc.set_modelspace_vport(height=115, center=(20, 40))
    with path.open("x", encoding="utf-8", newline="\n") as stream:
        doc.write(stream)


def make(folder):
    if ezdxf.__version__ != "1.4.5":
        raise ValueError("検証キットと同じ ezdxf 1.4.5 を使ってください")
    folder.mkdir(parents=True, exist_ok=False)
    path = folder / "neutral.dxf"
    previous = ezdxf.options.write_fixed_meta_data_for_testing
    try:
        ezdxf.options.write_fixed_meta_data_for_testing = True
        write_fixture(path)
    finally:
        ezdxf.options.write_fixed_meta_data_for_testing = previous
    for name in ("README.md", "make.py", "observe.py"):
        shutil.copyfile(Path(__file__).with_name(name), folder / name)
    metadata = {"file": path.name, "source": "zumen-jp/zumen #316", "license": "MIT OR Apache-2.0",
                "generator": "ezdxf 1.4.5", "synthetic": True, "sha256": hashlib.sha256(path.read_bytes()).hexdigest()}
    (folder / "source.json").write_text(json.dumps(metadata, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path, help="新しく作るキットのフォルダー")
    try:
        make(parser.parse_args().output)
    except (FileExistsError, ValueError) as error:
        parser.exit(2, f"作成できません: {error}\n")
