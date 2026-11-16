"""リポジトリ外の IFC・PDF の組で、室の断面積・外部の投影面積・面積表を別々に照らす。"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess

import ifcopenshell
import ifcopenshell.geom
import ifcopenshell.util.shape


def checked_file(root, relative, digest):
    path = (root / relative).resolve()
    if not path.is_relative_to(root) or not path.is_file():
        raise SystemExit("照合するファイルは ZUMEN_SAMPLES 内で指定する")
    if hashlib.sha256(path.read_bytes()).hexdigest() != digest:
        raise SystemExit("照合するファイルの SHA-256 が違う")
    return path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("executable", type=Path)
    parser.add_argument("reference", type=Path, help="外に置いた比較条件の JSON")
    args = parser.parse_args()
    root = Path(os.environ["ZUMEN_SAMPLES"]).resolve()
    reference = json.loads(args.reference.read_text(encoding="utf-8"))
    model_path = checked_file(root, reference["ifc"], reference["ifc_sha256"])
    checked_file(root, reference["pdf"], reference["pdf_sha256"])
    model = ifcopenshell.open(str(model_path))
    settings = ifcopenshell.geom.settings()
    settings.set(settings.USE_WORLD_COORDS, True)
    for index, case in enumerate(reference["cases"], 1):
        space = model.by_guid(case["global_id"])
        assert space.is_a("IfcSpace")
        geometry = ifcopenshell.geom.create_shape(settings, space)
        # この照合は高さで断面が変わらない室に限る。外部の投影面積を計算へ渡さない。
        external = ifcopenshell.util.shape.get_footprint_area(geometry.geometry, axis="Z")
        command = [str(args.executable.resolve()), reference["ifc"], space.GlobalId,
                   *[str(v) for v in case["origin_doc"]], "shape", "auto"]
        result = subprocess.run(command, capture_output=True, timeout=180)
        if result.returncode:
            raise SystemExit(f"計測例が失敗した: {index}")
        computed = json.loads(result.stdout)["area_m2"]
        assert computed is not None and abs(computed - external) <= 0.00001
        print(json.dumps({"case": index, "computed_m2": computed,
                          "external_geometry_m2": external, "table_m2": case["table_m2"],
                          "difference_m2": computed - case["table_m2"]}))


if __name__ == "__main__":
    main()
