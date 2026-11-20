"""公開 J-LandXML の TIN を ElementTree で読み、Rust の土木モデルと照合する。"""

import argparse
import hashlib
import itertools
import math
import struct
import subprocess
import xml.etree.ElementTree as ET
from pathlib import Path

NS = {"lx": "http://www.landxml.org/schema/LandXML-1.2"}


def reference(data):
    root = ET.fromstring(data)
    metric = root.find("lx:Units/lx:Metric", NS)
    if metric is None or metric.get("linearUnit") != "meter":
        raise ValueError("メートル単位の XML が必要です")
    surfaces = root.findall("lx:Surfaces/lx:Surface", NS)
    if not surfaces:
        raise ValueError("照合する地表面がありません")
    rows = []
    counts = []
    for surface in surfaces:
        definitions = surface.findall("lx:Definition", NS)
        if len(definitions) != 1 or definitions[0].get("surfType") != "TIN":
            raise ValueError("地表面に TIN の定義が 1 件必要です")
        definition = definitions[0]
        points = definition.findall("lx:Pnts/lx:P", NS)
        faces = [f for f in definition.findall("lx:Faces/lx:F", NS) if f.get("i", "").strip() != "1"]
        if not points or not faces:
            raise ValueError("照合する頂点または可視面がありません")
        name = surface.get("name", "").encode("utf-8").hex()
        counts.append((len(points), len(faces)))
        rows.append(f"S {len(points)} {len(faces)} {name}")
        indexes = {}
        for index, point in enumerate(points):
            key = int(point.attrib["id"])
            values = [float(token) for token in point.text.split()]
            if key in indexes or len(values) != 3 or not all(map(math.isfinite, values)):
                raise ValueError("頂点の ID または座標が不正です")
            indexes[key] = index
            rows.append("P " + " ".join(struct.pack(">d", v).hex() for v in values))
        for face in faces:
            corners = [indexes[int(key)] for key in face.text.split()]
            if len(corners) != 3 or len(set(corners)) != 3:
                raise ValueError("三角形の頂点が不正です")
            rows.append("F " + " ".join(map(str, corners)))
    return rows, counts


def compare(probe, sample):
    data = sample.read_bytes()
    expected, counts = reference(data)
    actual = subprocess.run([probe], input=data, stdout=subprocess.PIPE, check=True)
    lines = actual.stdout.decode("ascii").splitlines()
    for row, (want, got) in enumerate(itertools.zip_longest(expected, lines), 1):
        if got != want:
            raise ValueError(f"TIN の照合行 {row} で不一致: XML={want!r}, Rust={got!r}")
    return hashlib.sha256(data).hexdigest(), counts


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("probe", type=Path)
    parser.add_argument("samples", type=Path, nargs="+")
    args = parser.parse_args()
    for sample in args.samples:
        digest, counts = compare(args.probe.resolve(), sample)
        print(f"{digest}  " + ", ".join(f"{p} 頂点・{f} 面" for p, f in counts))


if __name__ == "__main__":
    main()
