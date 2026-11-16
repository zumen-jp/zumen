"""mesh_audit の --elements の公開入力の結果を、別に入れた IfcOpenShell と照らす。"""

import argparse
from collections import Counter, defaultdict
import math
from pathlib import Path

import ifcopenshell
import ifcopenshell.geom


def volume(vertices, faces):
    """最初の点を原点にした符号付き四面体の和。単位は IfcOpenShell 既定の m³。"""
    origin = vertices[:3]
    terms = []
    for i in range(0, len(faces), 3):
        a, b, c = [tuple(vertices[faces[i + j] * 3 + k] - origin[k] for k in range(3)) for j in range(3)]
        terms.append(a[0] * (b[1] * c[2] - b[2] * c[1])
                     + a[1] * (b[2] * c[0] - b[0] * c[2])
                     + a[2] * (b[0] * c[1] - b[1] * c[0]))
    return math.fsum(terms) / 6.0


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("results", type=Path)
    parser.add_argument("samples", type=Path)
    args = parser.parse_args()
    files = sorted(args.samples.rglob("*.ifc"))
    rows = defaultdict(list)
    for line in args.results.read_text(encoding="utf-8-sig").splitlines():
        if line.startswith("element "):
            row = dict(item.split("=", 1) for item in line.split()[1:])
            if row["solid"] == "true" and row["complete"] == "true":
                rows[int(row["input"])].append(row)
    total = Counter()
    for input_index, path in enumerate(files, 1):
        data = path.read_text(encoding="utf-8")
        # 公開試料の旧い IFC4X3 の表記だけを、外部実装が受ける名前へ直したメモリ上の写しで開く。
        data = data.replace("FILE_SCHEMA(('IFC4X3'))", "FILE_SCHEMA(('IFC4X3_ADD2'))", 1)
        model = ifcopenshell.file.from_string(data)
        counts = Counter()
        worst = []
        settings = ifcopenshell.geom.settings()
        for row in rows[input_index]:
            entity = model.by_id(int(row["instance"]))
            if entity.is_a("IfcOpeningElement"):
                counts["opening"] += 1
                continue
            try:
                shape = ifcopenshell.geom.create_shape(settings, entity)
                theirs = volume(shape.geometry.verts, shape.geometry.faces)
            except Exception as error:
                counts["external_" + type(error).__name__] += 1
                continue
            ours = float(row["volume"])
            if max(abs(ours), abs(theirs)) < 1e-5:
                counts["flat"] += 1
                continue
            relative = abs(ours - theirs) / max(abs(theirs), 1e-12)
            counts["compared"] += 1
            for threshold in [1e-6, 1e-4, 1e-3, 1e-2]:
                if relative <= threshold:
                    counts[f"within_{threshold:g}"] += 1
            worst.append((relative, int(row["instance"]), ours, theirs))
        total.update(counts)
        print(f"input={input_index} " + " ".join(f"{k}={v}" for k, v in sorted(counts.items())), flush=True)
        for relative, instance, ours, theirs in sorted(worst, reverse=True)[:5]:
            if relative > 1e-4:
                print(f"difference input={input_index} instance={instance} relative={relative:.9g} ours={ours:.12g} external={theirs:.12g}", flush=True)
    print("total " + " ".join(f"{k}={v}" for k, v in sorted(total.items())), flush=True)


if __name__ == "__main__":
    main()
