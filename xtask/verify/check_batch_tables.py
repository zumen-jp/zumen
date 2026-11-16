"""同じ apply で足した表への参照を、ezdxf と ACadSharp の読みで照らす。

使い方: python check_batch_tables.py <合成出力のフォルダー>
apply_batch_tables_case の --output と ACadSharp の出力を使う。
"""

import json
import sys
from importlib.metadata import version
from pathlib import Path

import ezdxf


def check(folder):
    for name in ["DXF", "DXF-keyless"]:
        doc = ezdxf.readfile(folder / f"{name}.dxf")
        assert doc.layers.get("BatchLayer").dxf.linetype == "BatchType"
        assert [tag.value for tag in doc.linetypes.get("BatchType").pattern_tags.tags if tag.code == 49] == [3.0, -1.0]
        texts = [e for e in doc.modelspace().query("TEXT") if e.dxf.text == "New"]
        assert len(texts) == 1 and texts[0].dxf.style == "BatchReplacement"
        lines = [e for e in doc.modelspace().query("LINE")
                 if list(e.dxf.start)[:2] == [1.0, 2.0]]
        assert len(lines) == 1
        assert lines[0].dxf.layer == "BatchLayer" and lines[0].dxf.linetype == "BatchType"
        leaders = list(doc.modelspace().query("LEADER"))
        assert len(leaders) == 1 and leaders[0].dxf.dimstyle == "BatchDim"
        assert doc.dimstyles.get("BatchDim").dxf.dimasz == 3.0
        print(f"{name}: レイヤ・線種・文字スタイル・寸法スタイルの参照が一致")
    dump = json.loads((folder / "acadsharp-dump.json").read_text(encoding="utf-8"))
    for release in ["2000", "2004", "2018"]:
        data = dump["files"][f"DWG-{release}.dwg"]
        assert not data.get("error"), data.get("error")
        entities = list(data["entities"].values())
        texts = [e for e in entities if e["type"] == "TEXT" and e.get("text") == "New"]
        assert len(texts) == 1 and texts[0]["style"] == "BatchReplacement"
        lines = [e for e in entities if e["type"] == "LINE" and e.get("start") == [1, 2]]
        assert len(lines) == 1
        assert lines[0]["layer"] == "BatchLayer" and lines[0]["line_type"] == "BatchType"
        print(f"DWG-{release}: レイヤ・線種・文字スタイルの参照が一致")
    print(f"ezdxf {version('ezdxf')} / ACadSharp {dump['acadsharp']}")


if __name__ == "__main__":
    check(Path(sys.argv[1]))
