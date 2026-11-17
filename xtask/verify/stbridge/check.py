"""公開サンプルの ST-Bridge 2.0.2 を、zumen の公開 API と STBDotNet（v202 のクラス）で読んで照らす。

照らすのは、部材と階の要素の数・ID・名前、部材・階・断面・鋼材の規格の属性の値（数は値で、文字は文字列で）、
部材から断面・鋼材の規格への参照、部材の配置の原点と材軸の向き（STBDotNet が読んだ節点の座標から求める）。
"""

import argparse
import hashlib
import json
import math
from pathlib import Path
import subprocess
import sys

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
sys.path.insert(0, str(HERE.parent))
from make import Zumen  # noqa: E402

SAMPLES = ("samples/stbridge/hoaryfox-sample-building.stb",)
# 線の部材。始めと終わりの節点の属性
LINEAR = {
    "StbColumn": ("id_node_bottom", "id_node_top"),
    "StbPost": ("id_node_bottom", "id_node_top"),
    "StbGirder": ("id_node_start", "id_node_end"),
    "StbBeam": ("id_node_start", "id_node_end"),
    "StbBrace": ("id_node_start", "id_node_end"),
}
TOLERANCE = 1e-9


def same(expected, found):
    """STBDotNet の値（数・真偽・文字）と、zumen が文字のまま持つ属性の値"""
    if isinstance(expected, bool):
        return found == ("true" if expected else "false")
    if isinstance(expected, (int, float)):
        try:
            value = float(found)
        except ValueError:
            return False
        return math.isfinite(value) and math.isclose(expected, value, rel_tol=1e-12, abs_tol=1e-12)
    return expected == found


def attributes(element):
    """STBDotNet の要素から、子の要素（dict・list）を除いた属性"""
    return {k: v for k, v in element.items() if not isinstance(v, (dict, list)) and k != "$type"}


def properties(element):
    return {s["name"]: {p["name"]: p["value"].get("text") for p in s["properties"]} for s in element.get("properties", [])}


def compare_set(where, expected, found, mismatches):
    if found is None:
        mismatches.append(f"{where}: zumen に属性のセットがない")
        return
    for key in sorted(set(expected) | set(found)):
        if key not in found:
            mismatches.append(f"{where}.{key}: zumen にない（STBDotNet は {expected[key]!r}）")
        elif key not in expected:
            mismatches.append(f"{where}.{key}: STBDotNet にない（zumen は {found[key]!r}）")
        elif not same(expected[key], found[key]):
            mismatches.append(f"{where}.{key}: STBDotNet {expected[key]!r} / zumen {found[key]!r}")


def shapes(section):
    """断面の形を書く子の要素が引く鋼材の規格の名前"""
    found = []
    for value in section.values():
        for child in value if isinstance(value, list) else [value] if isinstance(value, dict) else []:
            if "shape" in child:
                found.append(child["shape"])
            found.extend(shapes(child))
    return found


def compare(elements, dump):
    """zumen の building_elements の要素と、STBDotNet の書き出しを照らし、食い違いの一覧と数えた件数を返す"""
    mismatches = []
    counts = {"members": 0, "stories": 0, "sections": 0, "steels": 0, "placements": 0, "values": 0}
    nodes = {n["id"]: (n["X"], n["Y"], n["Z"]) for n in dump["nodes"]}
    sections = {}
    for kind, items in dump["sections"].items():
        for item in items:
            sections.setdefault(item["id"], []).append((kind, item))
    steels = {}
    for kind, items in dump["steels"].items():
        for item in items:
            steels[item["name"]] = (kind, item)
    zumen = {}
    for element in elements:
        original = element.get("class", {}).get("original")
        tag = element.get("identity", {}).get("tag")
        if original and tag is not None:
            zumen.setdefault((original, tag), []).append(element)

    expected = [("StbStory", s) for s in dump["stories"]]
    expected += [(kind, m) for kind, items in dump["members"].items() for m in items]
    for kind, item in expected:
        where = f"{kind} {item['id']}"
        found = zumen.pop((kind, item["id"]), [])
        if len(found) != 1:
            mismatches.append(f"{where}: zumen の要素が {len(found)} 個")
            continue
        element = found[0]
        counts["stories" if kind == "StbStory" else "members"] += 1
        if element.get("identity", {}).get("name") != item.get("name"):
            mismatches.append(f"{where}: 名前が違う（STBDotNet {item.get('name')!r} / zumen {element['identity'].get('name')!r}）")
        sets = properties(element)
        compare_set(where, attributes(item), sets.get(kind), mismatches)
        counts["values"] += len(attributes(item))
        # 断面と鋼材の規格
        if "id_section" in item:
            candidates = [(k, s) for k, s in sections.get(item["id_section"], []) if k in sets]
            if len(candidates) != 1:
                mismatches.append(f"{where}: 断面 {item['id_section']} を zumen のセット {sorted(sets)} に 1 つに決められない")
            else:
                section_kind, section = candidates[0]
                compare_set(f"{where} {section_kind}", attributes(section), sets[section_kind], mismatches)
                counts["sections"] += 1
                counts["values"] += len(attributes(section))
                for shape in sorted(set(shapes(section))):
                    if shape not in steels:
                        mismatches.append(f"{where}: 鋼材の規格 {shape!r} が STBDotNet にない")
                        continue
                    steel_kind, steel = steels[shape]
                    compare_set(f"{where} {steel_kind}", attributes(steel), sets.get(steel_kind), mismatches)
                    counts["steels"] += 1
                    counts["values"] += len(attributes(steel))
        # 配置。原点は始めの節点、局所の Z は始めから終わりの節点へ向かう
        if kind in LINEAR:
            start, end = (nodes.get(item.get(name)) for name in LINEAR[kind])
            world = element.get("placement", {}).get("world")
            if start is None or end is None or world is None:
                mismatches.append(f"{where}: 節点か配置がない（{start}, {end}, {world}）")
                continue
            origin = [row[3] for row in world]
            axis = [row[2] for row in world]
            length = math.dist(start, end)
            direction = [(e - s) / length for s, e in zip(start, end)]
            if any(abs(a - b) > TOLERANCE for a, b in zip(origin, start)):
                mismatches.append(f"{where}: 原点 {origin} / 始めの節点 {list(start)}")
            if any(abs(a - b) > TOLERANCE for a, b in zip(axis, direction)):
                mismatches.append(f"{where}: 材軸 {axis} / 節点から {direction}")
            counts["placements"] += 1
    stb = {key: found for key, found in zumen.items() if key[0] in LINEAR or key[0] == "StbStory"}
    for (kind, tag) in sorted(stb):
        mismatches.append(f"{kind} {tag}: STBDotNet にない要素を zumen が持つ")
    return mismatches, counts


def sha(data):
    return hashlib.sha256(data).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", required=True, help="zumen の CLI")
    parser.add_argument("--out", required=True, help="結果を書く、リポジトリの外の新しいか空のフォルダー")
    args = parser.parse_args()
    out = Path(args.out).resolve()
    if out.is_relative_to(ROOT) or (out.exists() and any(out.iterdir())):
        print(f"{out}: リポジトリの外の、新しいか空のフォルダーを指定する", file=sys.stderr)
        return 2
    out.mkdir(parents=True, exist_ok=True)
    results = []
    z = Zumen(args.cli)
    try:
        for sample in SAMPLES:
            path = ROOT / sample
            data = path.read_bytes()
            dump_path = out / (path.stem + ".stbdotnet.json")
            subprocess.run(
                ["dotnet", "run", "--project", str(HERE), "-c", "Release", "--", str(path), str(dump_path)],
                check=True,
            )
            dump = json.loads(dump_path.read_text(encoding="utf-8"))
            z.push(path.name, data)
            status, opened = z.call({"op": "open", "input": path.name})
            if status != "ok":
                raise RuntimeError(f"open {sample}: {status} {opened}")
            status, reply = z.call(
                {"op": "building_elements", "doc": path.name, "components": ["class", "identity", "properties", "placement"]}
            )
            if status != "ok":
                raise RuntimeError(f"building_elements {sample}: {status} {reply}")
            elements = reply["result"]["elements"]
            if reply["result"]["total"] != len(elements):
                raise RuntimeError(f"{sample}: 要素の一覧が途中で切れた")
            mismatches, counts = compare(elements, dump)
            results.append(
                {
                    "sample": sample,
                    "sha256": sha(data),
                    "implementation": dump["implementation"],
                    "counts": counts,
                    "mismatches": mismatches,
                }
            )
            print(f"{sample}: {counts} 食い違い {len(mismatches)}")
            for line in mismatches[:20]:
                print(f"  {line}")
    finally:
        z.close()
    (out / "results.json").write_text(json.dumps(results, ensure_ascii=False, indent=2) + "\n", encoding="utf-8", newline="\n")
    return 1 if any(r["mismatches"] for r in results) else 0


if __name__ == "__main__":
    sys.exit(main())
