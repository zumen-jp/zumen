"""公開 PLATEAU の面を ElementTree で読み、Rust の GeoModel と照らす。"""

import argparse
import hashlib
import itertools
import math
import re
import struct
import subprocess
import xml.etree.ElementTree as ET
from collections import Counter
from pathlib import Path

GML = "{http://www.opengis.net/gml}"
HREF = "{http://www.w3.org/1999/xlink}href"
COORDINATE = re.compile(r"[0-9a-f]{16}( [0-9a-f]{16}){2}")
LOD_PROPERTY = re.compile(r"lod([0-4])(?:FootPrint|Solid|MultiSurface)")
SURFACE_CONTAINERS = {GML + name for name in (
    "Solid", "exterior", "CompositeSurface", "MultiSurface", "surfaceMember", "surfaceMembers"
)}


def polygons(shape, ids, ancestors=()):
    """公開原本の面を文書順にたどり、同じ文書の surfaceMember の参照を解決する。"""
    if shape in ancestors:
        raise ValueError("形に循環する参照があります")
    if shape.tag == GML + "Polygon":
        yield shape
        return
    if shape.tag not in SURFACE_CONTAINERS:
        raise ValueError(f"想定外の形 {shape.tag}")
    target = shape.get(HREF)
    if target is None:
        children = shape
    elif target.startswith("#") and len(shape) == 0:
        children = [ids[target[1:]]]
    else:
        raise ValueError(f"想定外の参照 {target}")
    for child in children:
        yield from polygons(child, ids, (*ancestors, shape))


def reference(data, counts):
    root = ET.fromstring(data)
    envelope = root.find(".//" + GML + "Envelope")
    if (envelope is None or envelope.get("srsDimension") != "3"
            or envelope.get("srsName") != "http://www.opengis.net/def/crs/EPSG/0/6697"):
        raise ValueError("EPSG:6697 の 3 軸を想定した試料です")
    yield "CRS Some(6668) Some(6695)"
    ids = {}
    for element in root.iter():
        key = element.get(GML + "id")
        if key is not None:
            if key in ids:
                raise ValueError(f"gml:id の重複 {key}")
            ids[key] = element
    for key, feature in sorted(ids.items()):
        geometries = []
        for prop in feature:
            match = LOD_PROPERTY.fullmatch(prop.tag.split("}")[-1])
            if match:
                geometries.extend((match[1], geom) for geom in prop)
        if not geometries:
            continue
        counts["features"] += 1
        yield f"ID {key}"
        for lod, geom in geometries:
            counts["LOD" + lod] += 1
            yield f"LOD {lod} {geom.tag.removeprefix(GML)}"
            for polygon in polygons(geom, ids):
                for boundary in polygon:
                    kind = {GML + "exterior": "outer", GML + "interior": "hole"}[boundary.tag]
                    coords = boundary.find(f"{GML}LinearRing/{GML}posList")
                    if coords is None or coords.get("srsDimension", "3") != "3":
                        raise ValueError("3 次元の posList が必要です")
                    values = list(map(float, coords.text.split()))
                    if len(values) % 3 or not all(map(math.isfinite, values)):
                        raise ValueError("不正な座標")
                    points = list(zip(values[::3], values[1::3], values[2::3]))
                    if not points or points[-1] != points[0]:
                        raise ValueError("閉じた輪が必要です")
                    if kind == "outer":
                        if len(points) < 4:
                            if len(polygon) != 1:
                                raise ValueError("不正な外周を持つ面に内周があります")
                            counts["invalid"] += 1
                            continue
                        counts["faces"] += 1
                    yield f"{kind} {len(points) - 1}"
                    for latitude, longitude, height in points[:-1]:
                        counts["points"] += 1
                        yield " ".join(struct.pack(">d", v).hex() for v in (latitude, longitude, height))
    yield f"invalid {counts['invalid']}"


def gdal_rows(sample, counts):
    """1 つの MultiPolygon Z を持つ土地利用・浸水原本を GDAL で読む。"""
    import shapely
    from pyogrio.raw import read

    meta, _, geometries, columns = read(sample, columns=["gml_id"])
    if meta["crs"] != "EPSG:6668" or meta["geometry_type"] != "MultiPolygon Z":
        raise ValueError("経緯度の MultiPolygon Z が必要です")
    for key, wkb in sorted(zip(columns[0], geometries, strict=True)):
        yield f"ID {key}"
        counts["features"] += 1
        for polygon in shapely.from_wkb(wkb).geoms:
            counts["faces"] += 1
            rings = [("outer", polygon.exterior)] + [("hole", ring) for ring in polygon.interiors]
            for kind, ring in rings:
                points = list(ring.coords)
                if len(points) < 4 or points[0] != points[-1]:
                    raise ValueError("閉じた輪が必要です")
                yield f"{kind} {len(points) - 1}"
                for longitude, latitude, height in points[:-1]:
                    counts["points"] += 1
                    yield " ".join(struct.pack(">d", v).hex() for v in (latitude, longitude, height))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--gdal", action="store_true", help="土地利用・浸水原本の ID と全頂点を GDAL と照らす")
    parser.add_argument("--wasm", type=Path, help="WASI で作った plateau_probe の全出力を照らす")
    parser.add_argument("probe", type=Path)
    parser.add_argument("samples", type=Path, nargs="+")
    args = parser.parse_args()
    for sample in args.samples:
        data = sample.read_bytes()
        run = subprocess.run([args.probe.resolve()], input=data, stdout=subprocess.PIPE, check=True)
        if args.wasm:
            wasi = Path(__file__).resolve().parents[3] / "bindings/conformance/wasi.mjs"
            wasm = subprocess.run(["node", "--disable-warning=ExperimentalWarning", wasi, args.wasm],
                                  input=data, stdout=subprocess.PIPE, check=True)
            if run.stdout != wasm.stdout:
                raise ValueError(f"{sample.name} のネイティブと WASI の出力が一致しません")
        counts = Counter()
        actual = run.stdout.decode("ascii").splitlines()
        if args.gdal:
            expected = gdal_rows(sample, counts)
            # GDAL の GML レイヤーは LOD と高さ基準を別の項目として返さない。
            actual = (line for line in actual if not line.startswith(("CRS ", "LOD ", "invalid ")))
        else:
            expected = reference(data, counts)
        rows = itertools.zip_longest(expected, actual)
        max_error = [0.0, 0.0, 0.0]
        for row, (want, got) in enumerate(rows, 1):
            if want == got:
                continue
            if args.gdal and want and got and COORDINATE.fullmatch(want) and COORDINATE.fullmatch(got):
                a, b = (struct.unpack(">ddd", bytes.fromhex(line)) for line in (want, got))
                errors = [abs(x - y) for x, y in zip(a, b)]
                if all(error <= limit for error, limit in zip(errors, (1e-12, 1e-12, 1e-10))):
                    max_error = list(map(max, max_error, errors))
                    continue
            raise ValueError(f"{sample.name} 行 {row} で不一致: 期待値={want!r}, Rust={got!r}")
        if not counts["points"]:
            raise ValueError("照合する点がありません")
        print(f"{sample.name} {hashlib.sha256(data).hexdigest()} {dict(counts)}")
        if args.gdal:
            print(f"max |Δ| (deg, deg, m): {max_error}")


if __name__ == "__main__":
    main()
