"""通り芯と軸の元属性を外部の IfcOpenShell で照らす。配置・反転・幾何は評価しない。

合成の 3 版は synthetic、既存公開サンプルの通り芯 1 件は external。
単位・曲線の補助レコード・診断・予算は Rust の検査と native/WASI の一致で確認する。
"""
import importlib.util
import json
import sys
from pathlib import Path
import ifcopenshell

# 自分たちの属性照合だけを共有する。外部実装のソースは読まない。
spec = importlib.util.spec_from_file_location("curve_comparison", Path(__file__).with_name("curves.py"))
curves = importlib.util.module_from_spec(spec)
spec.loader.exec_module(curves)


def graph(root):
    result, seen, stack = [], set(), [root]
    while stack:
        entity = stack.pop()
        if entity.id() in seen:
            continue
        seen.add(entity.id())
        result.append(curves.curve(entity))
        if entity.is_a() == "IfcCompositeCurve":
            stack.extend(reversed([segment.ParentCurve for segment in entity.Segments]))
        elif entity.is_a() == "IfcTrimmedCurve":
            stack.append(entity.BasisCurve)
    return result


def compare(path):
    data = json.loads(path.read_text(encoding="utf-8"))
    model = curves.open_model(path.parent / data["file"])
    axes_count = nodes_count = 0
    for item in data["grids"]:
        grid = model.by_id(item["instance"])
        assert grid.is_a() == "IfcGrid"
        actual = {"instance": grid.id(), "placement": grid.ObjectPlacement.id() if grid.ObjectPlacement else None,
                  "representation": grid.Representation.id() if grid.Representation else None,
                  "layout": getattr(grid, "PredefinedType", None), "axes": []}
        for name, values in (("U", grid.UAxes), ("V", grid.VAxes), ("W", grid.WAxes or ())):
            for axis in values:
                assert axis.is_a() == "IfcGridAxis"
                nodes = graph(axis.AxisCurve)
                actual["axes"].append({"instance": axis.id(), "set": name, "tag": axis.AxisTag,
                                       "same_sense": axis.SameSense, "root": axis.AxisCurve.id(), "items": nodes})
                axes_count += 1
                nodes_count += len(nodes)
        expected = {key: item[key] for key in ("instance", "placement", "representation", "layout")}
        expected["axes"] = [{key: axis[key] for key in ("instance", "set", "tag", "same_sense", "root", "items")}
                            for axis in item["axes"]]
        failures = list(curves.differences(expected, actual, str(grid.id())))
        assert not failures, failures[:20]
    print(f"{path.name}: 軸 {axes_count}、曲線 {nodes_count}、差異 0")
    return axes_count, nodes_count


if __name__ == "__main__":
    paths = sorted(Path(sys.argv[1]).glob("*.json"))
    assert len(paths) == 4, "合成 3 版と公開サンプルが必要"
    counts = [compare(path) for path in paths]
    print(f"IfcOpenShell {ifcopenshell.version}: 軸 {sum(x[0] for x in counts)}、曲線 {sum(x[1] for x in counts)}、差異 0")
