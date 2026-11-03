"""公開サンプルの IFC（samples/ifc）を、zumen の建物モデルと IfcOpenShell で読んだ値で照らす（#412、決定 0254）。

使い方: python samples.py <samples のフォルダー> <JSON のフォルダー>

JSON は、`samples.rs` の `writes_the_sample_models_for_other_readers` が、環境変数 `ZUMEN_IFC_SAMPLES_OUT` の
フォルダーに書いたもの。zumen が要素にしたものごとに、次を照らす。

- インスタンスの番号の実体のクラス・GlobalId・名前
- 所属の先（空間への包含・集約・入れ子）と型
- プロジェクトの空間への変換（ifcopenshell.util.placement）
- 開口があける要素と、建具が埋める開口
- プロパティと数量の値。zumen が写したものだけを比べ、IfcOpenShell だけが読んだものは数えて書く

IfcOpenShell が IfcObjectDefinition として読み、zumen が要素にしなかったものは、クラスごとに数えて書く。
そこへの所属と型も数えるだけにする。どれも誤りとはしない（zumen が写さない実体は、lift の診断で数えている）。

IfcOpenShell 0.9 は、スキーマの名前 IFC4X3 を読まない。IFC4X3 のファイルは、名前を IFC4X3_ADD2 にして読む
（zumen も IFC4X3 を IFC 4.3 ADD2 の属性の位置で読む）。
"""

import json
import math
import sys
from collections import Counter
from pathlib import Path

import ifcopenshell
import ifcopenshell.util.element
import ifcopenshell.util.placement

# 1 つの試料で書く食い違いの数の上限
SHOWN = 20


def close(a, b):
    return math.isclose(a, b, rel_tol=1e-9, abs_tol=1e-6)


def same_value(ours, theirs):
    """zumen の値（JSON）と、IfcOpenShell の値が同じか"""
    if isinstance(ours, bool) or isinstance(theirs, bool):
        return ours == theirs
    if isinstance(ours, (int, float)) and isinstance(theirs, (int, float)):
        return close(float(ours), float(theirs))
    if isinstance(ours, list) and isinstance(theirs, (list, tuple)):
        return len(ours) == len(theirs) and all(same_value(a, b) for a, b in zip(ours, theirs))
    if ours is None:
        return theirs is None or theirs == "UNKNOWN"
    if isinstance(theirs, ifcopenshell.entity_instance):
        return ours == theirs.id()
    return ours == theirs


def memberships(entity):
    """所属の先。種類から、所属の先のインスタンスの番号"""
    found = {}
    for rel in getattr(entity, "ContainedInStructure", None) or ():
        found.setdefault("contained", rel.RelatingStructure.id())
    for rel in getattr(entity, "Decomposes", None) or ():
        kind = "nests" if rel.is_a("IfcRelNests") else "aggregates"
        found.setdefault(kind, rel.RelatingObject.id())
    for rel in getattr(entity, "Nests", None) or ():
        found.setdefault("nests", rel.RelatingObject.id())
    return found


def sets(entity, **kind):
    """セットの名前から、プロパティの名前と値"""
    found = ifcopenshell.util.element.get_psets(entity, should_inherit=False, **kind)
    return {name: {k: v for k, v in values.items() if k != "id"} for name, values in found.items()}


def open_model(path):
    """IFC を開く。スキーマの名前が IFC4X3 なら、IFC4X3_ADD2 にして読む"""
    data = path.read_bytes()
    if b"FILE_SCHEMA(('IFC4X3'))" in data:
        text = data.decode("latin-1").replace("FILE_SCHEMA(('IFC4X3'))", "FILE_SCHEMA(('IFC4X3_ADD2'))", 1)
        return ifcopenshell.file.from_string(text)
    return ifcopenshell.open(str(path))


def check_element(model, element, ours, failures, missing):
    instance = element["instance"]
    at = f"#{instance}"
    try:
        entity = model.by_id(instance)
    except RuntimeError:
        failures.append(f"{at}: IfcOpenShell にインスタンスがない")
        return
    info = entity.get_info(recursive=False)
    if entity.is_a().lower() != element["class"].lower():
        failures.append(f"{at}: クラスが {element['class']}（IfcOpenShell は {entity.is_a()}）")
    if "GlobalId" in info and info["GlobalId"] != element["global_id"]:
        failures.append(f"{at}: GlobalId が {element['global_id']!r}（IfcOpenShell は {info['GlobalId']!r}）")
    if "Name" in info and info["Name"] != element["name"]:
        failures.append(f"{at}: 名前が {element['name']!r}（IfcOpenShell は {info['Name']!r}）")

    found = memberships(entity)
    kind = element["parent_kind"]
    lifted = {k: v for k, v in found.items() if v in ours}
    for k, v in found.items():
        if v not in ours:
            missing[f"zumen が要素にしなかったものへの {k}"] += 1
    if kind is None:
        if lifted:
            failures.append(f"{at}: 所属の先がない（IfcOpenShell は {lifted}）")
    elif found.get(kind) != element["parent"]:
        failures.append(f"{at}: {kind} の先が #{element['parent']}（IfcOpenShell は {found}）")

    if entity.is_a("IfcObject") or entity.is_a("IfcTypeObject"):
        theirs = ifcopenshell.util.element.get_type(entity)
        theirs = theirs.id() if theirs is not None and theirs != entity else None
        if theirs is not None and theirs not in ours:
            missing["zumen が要素にしなかった型"] += 1
        elif theirs != element["type"]:
            failures.append(f"{at}: 型が {element['type']}（IfcOpenShell は {theirs}）")

    world = element["world"]
    placement = info.get("ObjectPlacement")
    if world is not None:
        if placement is None:
            failures.append(f"{at}: 配置がない（zumen は配置を解いた）")
        else:
            matrix = ifcopenshell.util.placement.get_local_placement(placement)
            for r in range(3):
                for k in range(4):
                    if not close(world[r][k], float(matrix[r][k])):
                        failures.append(f"{at}: 変換の [{r}][{k}] が {world[r][k]}（IfcOpenShell は {matrix[r][k]}）")
                        break

    for key, inverse, attribute in [("voids", "VoidsElements", "RelatingBuildingElement"),
                                    ("fills", "FillsVoids", "RelatingOpeningElement")]:
        rels = getattr(entity, inverse, None) or ()
        theirs = getattr(rels[0], attribute).id() if rels else None
        if theirs != element[key]:
            failures.append(f"{at}: {key} が {element[key]}（IfcOpenShell は {theirs}）")

    for key, kind in [("properties", {"psets_only": True}), ("quantities", {"qtos_only": True})]:
        theirs = sets(entity, **kind)
        # IfcOpenShell と同じく、同じ名前のセットは 1 つにまとめ、後の値を採る
        ours_sets = {}
        for set_name, values in element[key]:
            ours_sets.setdefault(set_name, {}).update(values)
        for set_name, values in ours_sets.items():
            if set_name not in theirs:
                failures.append(f"{at}: {key} の {set_name} が IfcOpenShell にない")
                continue
            for name, value in values.items():
                if name not in theirs[set_name]:
                    failures.append(f"{at}: {set_name}.{name} が IfcOpenShell にない")
                elif not same_value(value, theirs[set_name][name]):
                    failures.append(
                        f"{at}: {set_name}.{name} が {value!r}（IfcOpenShell は {theirs[set_name][name]!r}）")
        for set_name, values in theirs.items():
            ours = ours_sets.get(set_name, {})
            for name in values:
                if name not in ours:
                    missing[f"{key}: {set_name}.{name}"] += 1


def check(samples, path):
    model_json = json.loads(path.read_text(encoding="utf-8"))
    model = open_model(samples / model_json["path"])
    failures = []
    missing = Counter()
    ours = {element["instance"] for element in model_json["elements"]}
    for element in model_json["elements"]:
        check_element(model, element, ours, failures, missing)
    skipped = Counter(e.is_a() for e in model.by_type("IfcObjectDefinition") if e.id() not in ours)
    return model_json["path"], len(model_json["elements"]), failures, missing, skipped


def main():
    if len(sys.argv) != 3:
        print("使い方: python samples.py <samples のフォルダー> <JSON のフォルダー>", file=sys.stderr)
        return 2
    samples, folder = Path(sys.argv[1]), Path(sys.argv[2])
    paths = sorted(folder.glob("*.json"))
    if not paths:
        print(f"{folder} に JSON がない", file=sys.stderr)
        return 2
    failed = 0
    for path in paths:
        name, count, failures, missing, skipped = check(samples, path)
        print(f"{name}: 要素 {count}、食い違い {len(failures)}")
        for failure in failures[:SHOWN]:
            print(f"  {failure}")
        if len(failures) > SHOWN:
            print(f"  ほか {len(failures) - SHOWN} 件")
        if skipped:
            print("  zumen が要素にしなかった: " + "、".join(f"{k} {n}" for k, n in sorted(skipped.items())))
        for what, n in sorted(missing.items(), key=lambda kv: (-kv[1], kv[0]))[:SHOWN]:
            print(f"  IfcOpenShell だけが読んだ {what}: {n}")
        failed += bool(failures)
    print(f"IfcOpenShell {ifcopenshell.version} で {len(paths)} 個の試料を照らし、{failed} 個に食い違いがあった")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
