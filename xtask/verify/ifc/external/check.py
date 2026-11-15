"""#526 の固定キットを IfcOpenShell で読み、編集・保持・検査の指摘を照らす。"""

import argparse
from collections import Counter
from datetime import datetime, timezone
import importlib.util
from importlib.metadata import version
import json
import math
from pathlib import Path
import platform
import subprocess
import sys
import tomllib

import ifcopenshell
import ifcopenshell.util.element
import ifcopenshell.validate

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[3]
spec = importlib.util.spec_from_file_location("ifc_kit", HERE.parent / "make.py")
kit = importlib.util.module_from_spec(spec)
spec.loader.exec_module(kit)
PINS = {"ifcopenshell": "0.9.0.post1", "pytest": "9.2.0"}
MUTABLE = {"ifc4": {628: {"Name", "Description"}, 638: {"HasProperties"}},
           "ifc2x3": {1732: {"Name", "Description"}, 1956: {"NominalValue"}}}


def need(condition, message):
    if not condition:
        raise ValueError(message)


def canonical(value):
    if isinstance(value, ifcopenshell.entity_instance):
        if value.id():
            return {"ref": value.id(), "type": value.is_a()}
        return {"inline": value.is_a(), "value": canonical(value.wrappedValue)}
    if isinstance(value, (list, tuple)):
        return [canonical(v) for v in value]
    if isinstance(value, dict):
        return {k: canonical(v) for k, v in value.items()}
    if isinstance(value, float):
        need(math.isfinite(value), "比較範囲に非有限値があります")
    need(value is None or isinstance(value, (str, int, float, bool)), "比較できない属性型です")
    return value


def same(a, b):
    # boolと数値、未指定のセット名と文字列を同じ値として扱わない。
    def equal(x, y):
        if type(x) is not type(y):
            return False
        if isinstance(x, dict):
            return {(type(k), k) for k in x} == {(type(k), k) for k in y} and all(equal(v, y[k]) for k, v in x.items())
        if isinstance(x, list):
            return len(x) == len(y) and all(equal(v, w) for v, w in zip(x, y))
        return x == y
    return equal(canonical(a), canonical(b))


def records(model):
    out = {entity.id(): canonical(entity.get_info()) for entity in model}
    need(len(out) == len(list(model)), "STEPの番号が重複しています")
    return out


def properties(element):
    """直接指定されたプロパティを、名前の重複を隠さずに取得する。"""
    found, sets = {}, set()
    for relation in element.IsDefinedBy:
        if not relation.is_a("IfcRelDefinesByProperties"):
            continue
        group = relation.RelatingPropertyDefinition
        if not group.is_a("IfcPropertySet"):
            continue
        need(group.Name not in sets, "対象のセット名が重複しています")
        sets.add(group.Name)
        for prop in group.HasProperties:
            key = (group.Name, prop.Name)
            need(key not in found, "対象のプロパティ名が重複しています")
            found[key] = canonical(prop.get_info())
    return found


def references(value):
    if isinstance(value, dict):
        if set(value) == {"ref", "type"}:
            yield value["ref"]
        else:
            for item in value.values():
                yield from references(item)
    elif isinstance(value, list):
        for item in value:
            yield from references(item)


def compare_models(before, after, schema):
    expected = kit.recipe(schema)
    need(before.schema == after.schema == kit.CASES[schema]["schema"], "IFCのバージョンが一致しません")
    old, new = records(before), records(after)
    need(old.keys() <= new.keys(), "元のSTEPインスタンスが欠けています")
    need(set(MUTABLE[schema]) == set(kit.CASES[schema]["mutable_records"]), "変更を許すインスタンスが変わりました")
    for number, original in old.items():
        allowed = MUTABLE[schema].get(number, set())
        need(same({k: v for k, v in original.items() if k not in allowed},
                  {k: v for k, v in new[number].items() if k not in allowed}), f"非対象の属性・参照が変わりました: #{number}")
    old_objects = {obj.id(): obj for obj in before.by_type("IfcObjectDefinition")}
    new_objects = {obj.id(): obj for obj in after.by_type("IfcObjectDefinition")}
    need(old_objects.keys() == new_objects.keys(), "対象外のオブジェクトが増減しました")
    target = kit.CASES[schema]["record"]
    for number, obj in old_objects.items():
        need(obj.GlobalId == new_objects[number].GlobalId, "元のGlobalIdが変わりました")
        if number != target:
            need(same(ifcopenshell.util.element.get_psets(obj), ifcopenshell.util.element.get_psets(new_objects[number])),
                 f"非対象のプロパティが変わりました: #{number}")
    for model in (before, after):
        need(sum(obj.GlobalId == expected["target"] for obj in model.by_type("IfcObjectDefinition")) == 1, "対象のGlobalIdが一意ではありません")
        need(sum(obj.GlobalId == expected["control"] for obj in model.by_type("IfcObjectDefinition")) == 1, "対照のGlobalIdが一意ではありません")
    actual = after.by_id(target)
    need(actual.GlobalId == expected["target"], "対象の番号とGlobalIdが一致しません")
    need((actual.Name, actual.Description) == (expected["name"], expected["description"]), "名前か説明が一致しません")
    old_props, new_props = properties(before.by_id(target)), properties(actual)
    wanted = {(prop["set"], prop["name"]): prop["value"] for prop in expected["properties"]}
    need(new_props.keys() == old_props.keys() | wanted.keys(), "対象のプロパティの名前が増減しました")
    observed = []
    for key, prop in new_props.items():
        if key not in wanted:
            need(same(prop, old_props[key]), "対象の変更しないプロパティか参照が変わりました")
            continue
        value = wanted[key]
        scalar = value.get("boolean", value.get("text", value.get("number")))
        need(prop["type"] == "IfcPropertySingleValue" and same(prop.get("NominalValue"), {"inline": value["type_name"], "value": scalar}),
             "プロパティの値かIFC型が一致しません: " + ".".join(key))
        metadata = {k: v for k, v in prop.items() if k not in {"id", "NominalValue"}}
        original = {k: v for k, v in old_props.get(key, {}).items() if k not in {"id", "NominalValue"}}
        if not original:
            original = {"type": "IfcPropertySingleValue", "Name": key[1], "Description": None, "Unit": None}
        need(same(metadata, original), "プロパティの付随属性が変わりました")
        observed.append({"set": key[0], "name": key[1], "type": value["type_name"], "value": scalar})
    # 型継承のプロパティや数量も、変更した4値以外は保つ。
    def values(obj):
        return {name: {key: value for key, value in group.items() if key != "id"}
                for name, group in ifcopenshell.util.element.get_psets(obj).items()}
    target_values = values(before.by_id(target))
    for prop in observed:
        target_values.setdefault(prop["set"], {})[prop["name"]] = prop["value"]
    need(same(values(actual), target_values), "対象のプロパティ・数量の値が一致しません")
    added = new.keys() - old.keys()
    need(added, "追加したプロパティのインスタンスがありません")
    allowed_types = {"IfcPropertySet", "IfcPropertySingleValue", "IfcRelDefinesByProperties"}
    need(all(new[n]["type"] in allowed_types for n in added), "予定外の型を追加しました")
    old_gids = {obj.GlobalId for obj in before.by_type("IfcRoot")}
    gids, roots = set(), set(MUTABLE[schema])
    for number in added:
        obj = after.by_id(number)
        if obj.is_a("IfcRoot"):
            need(obj.GlobalId and obj.GlobalId not in old_gids | gids, "追加したGlobalIdが重複しています")
            gids.add(obj.GlobalId)
        if obj.is_a("IfcRelDefinesByProperties"):
            need([o.id() for o in obj.RelatedObjects] == [target], "追加したプロパティの対象が違います")
            roots.add(number)
    reached, pending = set(), list(roots)
    while pending:
        number = pending.pop()
        if number in reached or number not in added | roots:
            continue
        need(number in new, "新しい属性から存在しない番号を参照しています")
        reached.add(number)
        pending.extend(references(new[number]))
    need(added <= reached, "編集対象から到達できないインスタンスを追加しました")
    return {"schema": before.schema, "inputRecords": len(old), "outputRecords": len(new),
            "unchangedRecords": len(old) - len(MUTABLE[schema]), "objects": len(old_objects),
            "unchangedObjectProperties": len(old_objects) - 1, "addedTypes": dict(sorted(Counter(new[n]["type"] for n in added).items())),
            "target": expected["target"], "control": expected["control"], "name": actual.Name,
            "description": actual.Description, "properties": sorted(observed, key=lambda p: (p["set"], p["name"]))}


def audit(model):
    logger = ifcopenshell.validate.json_logger()
    ifcopenshell.validate.validate(model, logger, express_rules=True)
    found = Counter()
    for row in logger.statements:
        instance = row.get("instance")
        if isinstance(instance, ifcopenshell.entity_instance):
            instance = instance.id()
        need(instance is None or isinstance(instance, int), "検査のインスタンスを識別できません")
        first_line = str(row.get("message", "")).split("\n", 1)[0]
        key = (row.get("level"), row.get("type"), instance, row.get("attribute"), kit.sha(first_line.encode()))
        found[key] += 1
    return found


def audit_rows(found):
    return [{"level": k[0], "type": k[1], "instance": k[2], "attribute": k[3], "messageSha256": k[4], "count": v}
            for k, v in sorted(found.items(), key=lambda item: kit.json_bytes(item[0]))]


def compare_audits(original, edited):
    need(not edited - original, "IfcOpenShellの検査で新しい指摘か件数の増加があります")
    return {"expressRules": True, "before": audit_rows(original), "after": audit_rows(edited), "added": []}


def check_case(folder, schema):
    before = ifcopenshell.open(str(folder / schema / "input.ifc"))
    after = ifcopenshell.open(str(folder / schema / "edited.ifc"))
    result = compare_models(before, after, schema)
    original, edited = audit(before), audit(after)
    return {**result, "audit": compare_audits(original, edited), "verified": True}


def read_kit(folder, relative):
    path = folder / relative
    need(path.resolve().is_relative_to(folder), "キットの参照先が外へ出ています")
    return kit.read_input(path)


def verify_kit(folder):
    need(not any((p / ".git").exists() for p in [folder, *folder.parents]), "キットはGitの作業ツリーの外に置いてください")
    manifest = json.loads(read_kit(folder, "manifest.json"))
    need(manifest.get("issue") == 526 and set(manifest["cases"]) == set(kit.CASES), "#526の2件のキットではありません")
    ledger = {s["path"]: s for s in tomllib.loads((ROOT / "samples/ledger.toml").read_text(encoding="utf8"))["samples"]}
    hashes = {}
    for schema, case in kit.CASES.items():
        entry = manifest["cases"][schema]
        need(same(entry["source"], ledger[case["source"]]) and not entry["failures"], "入力の出所かCLIの検査結果が一致しません")
        for name in ("input.ifc", "edited.ifc", "request.json", "expected.json"):
            hashes[f"{schema}/{name}"] = kit.sha(read_kit(folder, f"{schema}/{name}"))
        need(hashes[f"{schema}/input.ifc"] == entry["input_sha256"] == ledger[case["source"]]["sha256"], "公開入力のハッシュが一致しません")
        need(hashes[f"{schema}/edited.ifc"] == entry["edited_sha256"], "編集出力のハッシュが一致しません")
        expected = json.loads(read_kit(folder, f"{schema}/expected.json"))
        need(same(expected, kit.recipe(schema)), "期待値が固定したレシピと一致しません")
    return hashes


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("kit", type=Path)
    parser.add_argument("--cli", type=Path)
    parser.add_argument("--worker", choices=kit.CASES)
    parser.add_argument("--out", type=Path)
    args = parser.parse_args()
    versions = {name: version(name) for name in PINS}
    need(versions == PINS, "外部実装のバージョンを固定してください")
    folder = args.kit.resolve()
    hashes = verify_kit(folder)
    if args.worker:
        need(args.out is not None and not args.out.exists(), "子プロセスの出力先は新しいファイルにしてください")
        kit.write_json(args.out, check_case(folder, args.worker))
        return 0
    need(args.cli is not None, "比較対象のCLIを--cliで指定してください")
    out = folder / "external"
    need(not out.exists(), "既存の外部照合結果は上書きしません")
    out.mkdir()
    sources = ["xtask/verify/ifc/external/check.py", "xtask/verify/ifc/make.py", "xtask/verify/make.py", "samples/ledger.toml"]
    result = {"issue": 526, "coreCommit": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
              "sourceState": "working-tree" if subprocess.check_output(["git", "status", "--porcelain", "--untracked-files=normal"], cwd=ROOT) else "clean",
              "executedAt": datetime.now(timezone.utc).isoformat(), "cliSha256": kit.sha(args.cli.read_bytes()),
              "python": platform.python_version(), "external": versions, "inputs": hashes,
              "manifestSha256": kit.sha(kit.read_input(folder / "manifest.json")),
              "sources": {path: kit.sha((ROOT / path).read_bytes()) for path in sources},
              "cases": {}, "complete": False, "applicationExecuted": False, "countedInApplicationNumerator": False}
    try:
        for schema in kit.CASES:
            path = out / (schema + ".json")
            child = subprocess.run([sys.executable, "-X", "utf8", "-I", str(Path(__file__).resolve()), str(folder), "--worker", schema, "--out", str(path)],
                                   capture_output=True, timeout=180)
            need(child.returncode == 0 and path.exists(), f"{schema}の外部照合が正常に完了しませんでした")
            result["cases"][schema] = json.loads(path.read_bytes())
        result["complete"] = all(c["verified"] for c in result["cases"].values()) and len(result["cases"]) == 2
    finally:
        kit.write_json(out / "results.json", result)
    print("IfcOpenShell: IFC4・IFC2X3の各6編集と非対象の属性・参照を照合。実機は未実行")
    return 0


if __name__ == "__main__":
    sys.exit(main())
