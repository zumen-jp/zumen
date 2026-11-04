"""`samples` の出力の体積を、IfcOpenShell が作った形の体積と照らす（#5）。

使い方: python ifcopenshell_volumes.py <samples.sh の出力のフォルダー> [候補]

候補（省けば manifold-rust）の行のうち、閉じたメッシュ（`ok`）・部品ごとに閉じた組（`parts`）・辺で接する閉じた部分
（`touching`）になった要素ごとに、IfcOpenShell の既定の設定（開口を引く）で形を作り、三角形から体積を求める。zumen の
体積（ファイルの長さの単位の 3 乗）を m³ にして比べ、相対の差の分布と、差の大きい要素を書く。IfcOpenShell はテストの
ツールで、依存にしない。

IfcOpenShell 0.9 は、スキーマの名前 IFC4X3 を読まない。IFC4X3 のファイルは、名前を IFC4X3_ADD2 にした写しを読む。
"""

import atexit
import csv
import shutil
import sys
import tempfile
from collections import defaultdict
from pathlib import Path

import ifcopenshell
import ifcopenshell.geom
import ifcopenshell.util.unit

# 差の大きい要素を、サンプルごとに書く数
SHOWN = 5
# これより小さい体積（m³）は、厚さのない形として比べない
FLAT = 1e-5


def open_model(path):
    """IFC を開く。スキーマの名前が IFC4X3 なら、IFC4X3_ADD2 にした写しを、バイト列のまま書いて読む"""
    data = path.read_bytes()
    if b"FILE_SCHEMA(('IFC4X3'))" not in data:
        return ifcopenshell.open(str(path))
    folder = tempfile.mkdtemp()
    atexit.register(shutil.rmtree, folder, ignore_errors=True)
    copy = Path(folder) / path.name
    copy.write_bytes(data.replace(b"FILE_SCHEMA(('IFC4X3'))", b"FILE_SCHEMA(('IFC4X3_ADD2'))", 1))
    return ifcopenshell.open(str(copy))


def volume(verts, faces):
    total = 0.0
    for i in range(0, len(faces), 3):
        a, b, c = (faces[i + k] * 3 for k in range(3))
        ax, ay, az = verts[a : a + 3]
        bx, by, bz = verts[b : b + 3]
        cx, cy, cz = verts[c : c + 3]
        total += ax * (by * cz - bz * cy) - ay * (bx * cz - bz * cx) + az * (bx * cy - by * cx)
    return total / 6.0


def ours(folder, candidate):
    """サンプルの道筋から、インスタンスの番号・クラス・体積（ファイルの単位）"""
    found = defaultdict(list)
    for path in sorted(folder.glob(f"*.{candidate}.tsv")):
        with path.open(encoding="utf-8", newline="") as f:
            for row in csv.reader(f, delimiter="\t"):
                if not row or row[0].startswith("#") or row[0] == "file" or len(row) < 10:
                    continue
                body, full = row[6], row[7]
                # 閉じたメッシュ・部品ごとに閉じた組（体積は部品の和）・辺で接する閉じた部分の形を比べる
                if (full if full != "-" else body) not in ("ok", "parts", "touching"):
                    continue
                found[row[0]].append((int(row[1]), row[2], float(row[9])))
    return found


def main():
    # Windows で出力を写すと、既定の文字コード（cp932）で「m³」を書けない
    sys.stdout.reconfigure(encoding="utf-8")
    if len(sys.argv) not in (2, 3):
        print("使い方: python ifcopenshell_volumes.py <samples.sh の出力のフォルダー> [候補]", file=sys.stderr)
        return 2
    folder = Path(sys.argv[1])
    candidate = sys.argv[2] if len(sys.argv) == 3 else "manifold-rust"
    settings = ifcopenshell.geom.settings()
    bins = [1e-6, 1e-4, 1e-3, 1e-2]
    total = defaultdict(int)
    for file, elements in sorted(ours(folder, candidate).items()):
        model = open_model(Path(file))
        scale = ifcopenshell.util.unit.calculate_unit_scale(model) ** 3
        counts = defaultdict(int)
        worst = []
        for instance, cls, mine in elements:
            try:
                shape = ifcopenshell.geom.create_shape(settings, model.by_id(instance))
                theirs = volume(shape.geometry.verts, shape.geometry.faces)
            except Exception as error:  # IfcOpenShell が形を作れない要素は数える
                counts[f"ifcopenshell:{type(error).__name__}"] += 1
                continue
            mine *= scale
            # 厚さのない面を両面で閉じた形（中野区の点群の面など）は、体積がほぼ 0 なので比べない
            if max(abs(mine), abs(theirs)) < FLAT:
                counts["flat"] += 1
                continue
            diff = abs(mine - theirs) / max(abs(theirs), 1e-12)
            for b in bins:
                if diff <= b:
                    counts[f"<={b:g}"] += 1
                    break
            else:
                counts["more"] += 1
            worst.append((diff, instance, cls, mine, theirs))
        worst.sort(reverse=True)
        print(f"{file}: {len(elements)} 要素、" + "、".join(f"{k} {n}" for k, n in sorted(counts.items())))
        for diff, instance, cls, mine, theirs in worst[:SHOWN]:
            if diff > bins[0]:
                print(f"  #{instance} {cls}: zumen {mine:.6g} m³、IfcOpenShell {theirs:.6g} m³（差 {diff:.2e}）")
        for k, n in counts.items():
            total[k] += n
    print(f"IfcOpenShell {ifcopenshell.version}、{candidate}: " + "、".join(f"{k} {n}" for k, n in sorted(total.items())))
    return 0


if __name__ == "__main__":
    sys.exit(main())
