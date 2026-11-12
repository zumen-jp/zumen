"""共有 Lifter の SI 単位の元属性を IfcOpenShell のプログラム API と照らす。

自作 3 版の全 SI 名・同次元別名と、既存の公開サンプル。
診断・出所・予算・Hash は Rust と native/WASI で確認し、ここでは扱わない。
単位換算、SI の次元関数やファイル全体の適合性を外部比較したとは扱わない。
"""
import json
import sys
from pathlib import Path
import ifcopenshell


def compare(folder):
    count = 0
    for path in sorted(folder.glob("*.json")):
        for case in json.loads(path.read_text(encoding="utf-8")):
            model = ifcopenshell.open(str(folder / case["file"]))
            assignment = model.by_type("IfcProject")[0].UnitsInContext
            units = []
            for unit in assignment.Units:
                if unit.is_a() != "IfcSIUnit":
                    continue  # 換算単位は今回の SI 元属性の比較には含めない。
                units.append({"kind": unit.UnitType, "prefix": unit.Prefix,
                              "name": unit.Name, "scale": None})
            actual = [unit for unit in case["units"] if unit["scale"] is None]
            assert actual == units, f"{path.name}: {case['file']}"
            count += 1
    print(f"SI単位: {count}件の元属性が一致（IfcOpenShell {ifcopenshell.version}）")


if __name__ == "__main__":
    compare(Path(sys.argv[1]))
