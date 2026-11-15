"""公開例と自作入力のレイヤの元属性を、外部プログラムの公開 API だけで照らす。"""

import json
import sys
from pathlib import Path

import ifcopenshell


def logical(value):
    assert value in (True, False, "UNKNOWN")
    return None if value == "UNKNOWN" else value


def surface(value):
    assert value.is_a() == "IfcSurfaceStyle" and len(value.Styles) == 1
    shading = value.Styles[0]
    assert shading.is_a() == "IfcSurfaceStyleShading"
    colour = shading.SurfaceColour
    return {"instance": value.id(), "name": value.Name,
            "rgb": [colour.Red, colour.Green, colour.Blue],
            "transparency": getattr(shading, "Transparency", None)}


def layer(value):
    styled = value.is_a() == "IfcPresentationLayerWithStyle"
    return {"instance": value.id(), "name": value.Name, "description": value.Description,
            "identifier": value.Identifier,
            "state": [logical(value.LayerOn), logical(value.LayerFrozen),
                      logical(value.LayerBlocked)] if styled else None,
            "styles": [surface(v) for v in value.LayerStyles] if styled else []}


def compare(path):
    data = json.loads(path.read_text(encoding="utf-8"))
    model = ifcopenshell.open(str(path.parent / data["file"]))
    item = model.by_id(data["item"])
    representation = model.by_id(data["representation"])
    assert item in representation.Items
    styled = list(item.StyledByItem)
    assert len(styled) == 1 and styled[0].id() == data["styled_item"]
    actual = [layer(v) for v in model.by_type("IfcPresentationLayerAssignment")
              if item in v.AssignedItems or representation in v.AssignedItems]
    # 出力順は Rust の原本順で検査し、ここでは元レコードごとに属性を照らす。
    assert sorted(actual, key=lambda v: v["instance"]) == sorted(
        data["layers"], key=lambda v: v["instance"]), path.name
    return len(actual), sum(len(v["styles"]) for v in actual)


if __name__ == "__main__":
    paths = sorted(Path(sys.argv[1]).glob("*.json"))
    assert len(paths) == 4, "合成 3 版と公開サンプルが必要"
    counts = [compare(path) for path in paths]
    print(f"IfcOpenShell {ifcopenshell.version}: レイヤ {sum(v[0] for v in counts)}、"
          f"面の表示 {sum(v[1] for v in counts)}、差異 0")
