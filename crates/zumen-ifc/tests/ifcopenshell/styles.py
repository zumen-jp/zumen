"""直接の面の表示の元属性を IfcOpenShell と照らす。継承・反射式・最終表示は評価しない。

合成の 3 版は synthetic、既存公開サンプルの柱 1 件は external の範囲。
元属性と参照の比較であり、診断・予算・原本範囲は Rust 側で検査する。
"""

import importlib.util
import json
import sys
from pathlib import Path

import ifcopenshell

# 自分たちの比較コードだけを共有する。外部実装のソースは読まない。
spec = importlib.util.spec_from_file_location("curve_comparison", Path(__file__).with_name("curves.py"))
curves = importlib.util.module_from_spec(spec)
spec.loader.exec_module(curves)


def colour(entity):
    assert entity.is_a() == "IfcColourRgb"
    return {"name": entity.Name, "rgb": [entity.Red, entity.Green, entity.Blue]}


def colour_or_factor(entity):
    if entity is None:
        return None
    if entity.is_a() == "IfcColourRgb":
        return {"colour": colour(entity)}
    assert entity.is_a() == "IfcNormalisedRatioMeasure"
    return {"factor": entity.wrappedValue}


def highlight(entity):
    if entity is None:
        return None
    names = {"IfcSpecularExponent": "exponent", "IfcSpecularRoughness": "roughness"}
    return {names[entity.is_a()]: entity.wrappedValue}


def surface(entity):
    assert entity.is_a() == "IfcSurfaceStyle" and len(entity.Styles) == 1
    shading = entity.Styles[0]
    assert shading.is_a() in ("IfcSurfaceStyleShading", "IfcSurfaceStyleRendering")
    colours = [shading.SurfaceColour.id()]
    rendering = None
    if shading.is_a() == "IfcSurfaceStyleRendering":
        rendering = {}
        for output, attribute in (("diffuse", "DiffuseColour"), ("transmission", "TransmissionColour"),
                                  ("diffuse_transmission", "DiffuseTransmissionColour"),
                                  ("reflection", "ReflectionColour"), ("specular", "SpecularColour")):
            value = getattr(shading, attribute)
            rendering[output] = colour_or_factor(value)
            if value is not None and value.is_a() == "IfcColourRgb":
                colours.append(value.id())
        rendering["highlight"] = highlight(shading.SpecularHighlight)
        rendering["method"] = shading.ReflectanceMethod
    return {"style": entity.id(), "shading": shading.id(), "colour_records": colours,
            "name": entity.Name, "side": entity.Side, "colour": colour(shading.SurfaceColour),
            "transparency": getattr(shading, "Transparency", None), "rendering": rendering}


def compare(path):
    data = json.loads(path.read_text(encoding="utf-8"))
    model = ifcopenshell.open(str(path.parent / data["file"]))
    total = 0
    for item in data["items"]:
        target = model.by_id(item["instance"])
        direct = [value for value in target.StyledByItem if value.Item == target]
        assert len(direct) == 1
        styled = direct[0]
        assignments, styles = [], []
        for value in styled.Styles:
            if value.is_a() == "IfcPresentationStyleAssignment":
                assignments.append(value.id())
                styles.extend(value.Styles)
            else:
                styles.append(value)
        expected = {k: item[k] for k in ("instance", "styled_item", "name", "assignments", "styles")}
        actual = {"instance": target.id(), "styled_item": styled.id(), "name": styled.Name,
                  "assignments": assignments, "styles": [surface(value) for value in styles]}
        failures = list(curves.differences(expected, actual, str(item["instance"])))
        assert not failures, failures[:20]
        total += len(styles)
    print(f"{path.name}: 面の表示 {total}、差異 0")
    return total


if __name__ == "__main__":
    paths = sorted(Path(sys.argv[1]).glob("*.json"))
    assert len(paths) == 4, "合成 3 版と公開サンプルが必要"
    total = sum(compare(path) for path in paths)
    print(f"IfcOpenShell {ifcopenshell.version}: 面の表示 {total} 件、差異 0")
