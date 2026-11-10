"""IFC4・IFC2X3 の書き戻しを BIM ソフトで確かめるキットを作る（#526）。"""

import argparse
import copy
import hashlib
import importlib.util
import json
import re
import shutil
import sys
import tomllib
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
MAX_INPUT = 64 * 1024 * 1024
CASES = {
    "ifc4": {
        "source": "ifc/gni/model_9.ifc",
        "target": "2q2CBrfQTCKPY0zxCdZwK$",
        "control": "2q2CBrfQTCKPY0zxCdZwHh",
        "record": 628,
        "mutable_records": [628, 638],
        "schema": "IFC4",
    },
    "ifc2x3": {
        "source": "ifc/text2bim/sample_model.ifc",
        "target": "13v5x2IG0HxwZGA6irHSbP",
        "control": "13v5wuIG0HxwZGA6irHSbP",
        "record": 1732,
        "mutable_records": [1732, 1956],
        "schema": "IFC2X3",
    },
}


def json_bytes(value):
    return (json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True, allow_nan=False) + "\n").encode()


def sha(data):
    return hashlib.sha256(data).hexdigest()


def output_folder(path):
    path = path.resolve()
    if any((parent / ".git").exists() for parent in [path, *path.parents]):
        raise ValueError("出力先は Git の作業ツリーの外にしてください")
    if path.exists() and (not path.is_dir() or any(path.iterdir())):
        raise ValueError("出力先は新しいフォルダーか空のフォルダーにしてください")
    path.mkdir(parents=True, exist_ok=True)
    return path


def read_input(path):
    if path.stat().st_size > MAX_INPUT:
        raise ValueError("入力は 64 MiB 以下にしてください")
    with path.open("rb") as source:
        data = source.read(MAX_INPUT + 1)
    if len(data) > MAX_INPUT:
        raise ValueError("入力は 64 MiB 以下にしてください")
    return data


def client(exe):
    # 既存の CLI の JSON-RPC の運搬を使う。実装をキット側に複製しない。
    spec = importlib.util.spec_from_file_location("verify_make", ROOT / "xtask/verify/make.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module.Zumen(exe)


def call(z, request, records):
    status, response = z.call(request)
    records.append({"request": request, "status": status, "response": response})
    if status != "ok":
        codes = [item["code"] for item in (response or {}).get("diagnostics", [])]
        raise ValueError("API の失敗: " + request["op"] + " " + ",".join(codes))
    return response


def read_model(z, name, data, records):
    z.push(name, data)
    opened = call(z, {"op": "open", "input": name}, records)
    summary = call(z, {"op": "building", "doc": name}, records)["result"]
    elements, offset = [], 0
    for _ in range(1000):
        page = call(z, {"op": "building_elements", "doc": name, "offset": offset, "limit": 1000,
                        "components": ["identity", "class", "placement", "properties"]}, records)
        elements += page["result"]["elements"]
        following = page["result"].get("next_offset")
        if following is None:
            break
        if type(following) is not int or following <= offset:
            raise ValueError("要素の一覧の次の位置が進みません")
        offset = following
    else:
        raise ValueError("要素の一覧が 1000 ページを超えました")
    return {"schema": summary["schema"], "hash": summary["hash"], "units": summary["units"],
            "elements": elements, "diagnostics": dict(Counter(item["code"] for item in opened["diagnostics"]))}


def recipe(schema):
    case = CASES[schema]
    return {
        "target": case["target"], "control": case["control"],
        "name": f"検証526 {case['schema']} '壁' \\ A",
        "description": "名前・説明・プロパティの書き戻しを確認する壁",
        "properties": [
            {"set": "Pset_WallCommon", "name": "IsExternal", "value": {"kind": "boolean", "type_name": "IfcBoolean", "boolean": False}},
            {"set": "Zumen_Check526", "name": "Note", "value": {"kind": "text", "type_name": "IfcLabel", "text": "検証用の備考"}},
            {"set": "Zumen_Check526", "name": "Ratio", "value": {"kind": "real", "type_name": "IfcReal", "number": 1.25}},
            {"set": "Zumen_Check526", "name": "Count", "value": {"kind": "integer", "type_name": "IfcInteger", "number": 7}},
        ],
    }


def request_for(schema, before):
    expected = recipe(schema)
    targets = [item for item in before["elements"] if item["id"] == expected["target"]]
    if len(targets) != 1:
        raise ValueError("編集対象の GlobalId が一意ではありません")
    common = {"id": expected["target"], "hash": targets[0]["hash"]}
    edits = [{"kind": "property", **common, **copy.deepcopy(prop)} for prop in expected["properties"]]
    edits += [{"kind": "attribute", **common, "attribute": key, "value": expected[key]} for key in ["name", "description"]]
    return {"v": 1, "op": "building_apply", "doc": schema + ".ifc", "hash": before["hash"], "edits": edits}


def property_map(element):
    result = {}
    for group in element.get("properties", []):
        for prop in group["properties"]:
            key = (group["name"], prop["name"])
            if key in result:
                raise ValueError("比較するプロパティの名前が重複しています")
            result[key] = prop
    return result


def same_value(actual, expected):
    if not isinstance(actual, dict) or actual.get("type_name", "").lower() != expected["type_name"].lower():
        return False
    return all(type(actual.get(key)) is type(value) and actual.get(key) == value
               for key, value in expected.items() if key != "type_name")


def differences(before, after, expected):
    """変更のレシピと不変の対照を照らす。件数だけで一致としない。"""
    failures = []
    for field in ["schema", "units"]:
        if before[field] != after[field]:
            failures.append(field)
    original = {item["id"]: item for item in before["elements"]}
    found = {item["id"]: item for item in after["elements"]}
    if len(found) != len(after["elements"]) or len(original) != len(before["elements"]) or original.keys() != found.keys():
        failures.append("element_ids")
    if before["diagnostics"] != after["diagnostics"]:
        failures.append("diagnostics")
    for element_id, element in original.items():
        actual = found.get(element_id)
        if actual is None:
            continue
        if element_id != expected["target"]:
            if actual != element:
                failures.append("unchanged_element:" + element_id)
            continue
        for component, value in element.items():
            if component not in {"hash", "identity", "properties"} and actual.get(component) != value:
                failures.append(component)
        wanted_identity = {**element["identity"], "name": expected["name"], "description": expected["description"]}
        if actual.get("identity") != wanted_identity:
            failures.append("identity")
        wanted_properties = {(prop["set"], prop["name"]): prop["value"] for prop in expected["properties"]}
        old_properties, properties = property_map(element), property_map(actual)
        if properties.keys() != old_properties.keys() | wanted_properties.keys():
            failures.append("property_names")
        for key, value in wanted_properties.items():
            actual_property = properties.get(key, {})
            if not same_value(actual_property.get("value"), value):
                failures.append("property:" + ".".join(key))
            metadata = {key: value for key, value in old_properties.get(key, {"name": key[1]}).items() if key != "value"}
            if {key: value for key, value in actual_property.items() if key != "value"} != metadata:
                failures.append("property_metadata:" + ".".join(key))
        for key, value in old_properties.items():
            if key not in wanted_properties and properties.get(key) != value:
                failures.append("unchanged_property:" + ".".join(key))
    if expected["target"] not in original or expected["control"] not in original:
        failures.append("missing_target_or_control")
    return failures


def unchanged_records(before, after, mutable):
    """ハッシュを台帳と照らした公開入力は 1 インスタンス 1 行。形・配置を含むほかの行を保つ。"""
    pattern = re.compile(rb"^#([0-9]+)\s*=")
    numbers = [int(match.group(1)) for line in before.splitlines(keepends=True) if (match := pattern.match(line))]
    largest = max(numbers)

    def retained(data, output):
        lines = []
        for line in data.splitlines(keepends=True):
            match = pattern.match(line)
            number = int(match.group(1)) if match else None
            if number in mutable or (output and number is not None and number > largest):
                continue
            lines.append(line)
        return b"".join(lines)

    return retained(before, False) == retained(after, True)


def unchanged_root_references(before, after, record):
    # この 2 件の公開入力とレシピの Name・Description にはコンマがない。
    # 変更を許したルートの行でも、それ以外の値（型・GlobalId・OwnerHistory・形・配置等）を照らす。
    pattern = re.compile(rb"^#" + str(record).encode() + rb"\s*=\s*([A-Z0-9_]+)\((.*)\);$")

    def fields(data):
        rows = [match for line in data.splitlines() if (match := pattern.match(line))]
        if len(rows) != 1:
            return None
        parts = rows[0].group(2).split(b",", 4)
        if len(parts) != 5:
            return None
        return rows[0].group(1), parts[:2], parts[4]

    original, edited = fields(before), fields(after)
    return original is not None and original == edited


def write_json(path, value):
    path.write_bytes(json_bytes(value))


def make(exe, output):
    out = output_folder(output)
    ledger = tomllib.loads((ROOT / "samples/ledger.toml").read_text(encoding="utf-8"))["samples"]
    records, cases = [], {}
    z = client(exe)
    try:
        capabilities = call(z, {"op": "capabilities"}, records)["result"]
        for schema, case in CASES.items():
            source = next(item for item in ledger if item["path"] == case["source"])
            data = read_input(ROOT / "samples" / case["source"])
            if sha(data) != source["sha256"] or len(data) != source["bytes"]:
                raise ValueError("公開 IFC のハッシュかバイト数が台帳と違います")
            before = read_model(z, schema + ".ifc", data, records)
            if before["schema"] != case["schema"]:
                raise ValueError("公開 IFC のスキーマが予定と違います")
            expected = recipe(schema)
            request = request_for(schema, before)
            applied = call(z, request, records)
            if applied["diagnostics"]:
                raise ValueError("building_apply の診断があります")
            edited = z.take(next(item["id"] for item in applied["blobs"] if item["kind"] == "file"))
            after = read_model(z, schema + ".edited.ifc", edited, records)
            failures = differences(before, after, expected)
            if not unchanged_records(data, edited, case["mutable_records"]):
                failures.append("unchanged_records")
            if not unchanged_root_references(data, edited, case["record"]):
                failures.append("unchanged_root_references")
            folder = out / schema
            folder.mkdir()
            (folder / "input.ifc").write_bytes(data)
            (folder / "edited.ifc").write_bytes(edited)
            for name, value in [("request", request), ("before", before), ("after", after), ("expected", expected)]:
                write_json(folder / (name + ".json"), value)
            cases[schema] = {"source": source, "target": expected["target"], "control": expected["control"],
                             "record": case["record"], "input_sha256": sha(data), "edited_sha256": sha(edited),
                             "changes": "対象の名前と説明、IsExternal を false、新しい Zumen_Check526 の Note・Ratio・Count",
                             "elements": len(before["elements"]), "input_diagnostics": before["diagnostics"], "failures": failures}
            if failures:
                raise ValueError(schema + " の比較で違いがありました: " + ",".join(failures))
        write_json(out / "manifest.json", {"issue": 526, "verification": "synthetic", "capabilities": capabilities, "cases": cases})
        licenses = out / "licenses"
        licenses.mkdir()
        for name in ["LICENSE-MIT", "LICENSE-APACHE"]:
            shutil.copyfile(ROOT / name, licenses / name)
        shutil.copyfile(ROOT / "licenses/text2bim-MIT.txt", licenses / "text2bim-MIT.txt")
        for name in ["README.md", "report.md"]:
            shutil.copyfile(Path(__file__).with_name(name), out / name)
        print("IFC4・IFC2X3 の編集各 6 件と再読み込み、ほかの要素・元の形の行の保持を確認しました。BIM ソフトでは未確認です。")
    finally:
        z.close()
        write_json(out / "api.json", records)


def observe(exe, kit, schema, exported):
    kit = kit.resolve()
    out = output_folder(kit / "observed" / schema)
    folder = kit / schema
    manifest = json.loads((kit / "manifest.json").read_bytes())
    original = read_input(folder / "input.ifc")
    if sha(original) != manifest["cases"][schema]["input_sha256"]:
        raise ValueError("キットの入力のハッシュが違います")
    expected = json.loads((folder / "expected.json").read_bytes())
    if expected != recipe(schema):
        raise ValueError("キットの期待値がレシピと違います")
    data, records = read_input(exported), []
    z = client(exe)
    try:
        before = read_model(z, "input.ifc", original, records)
        after = read_model(z, "observed.ifc", data, records)
        failures = differences(before, after, expected)
        (out / "exported.ifc").write_bytes(data)
        write_json(out / "after.json", after)
        write_json(out / "comparison.json", {"schema": schema, "sha256": sha(data), "failures": failures})
        print(f"{schema}: 比較の違い {len(failures)} 件。結果は observed の中に保存しました。")
        return 1 if failures else 0
    finally:
        z.close()
        write_json(out / "api.json", records)


def main():
    sys.stdout.reconfigure(encoding="utf-8")
    sys.stderr.reconfigure(encoding="utf-8")
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    generate = commands.add_parser("make")
    generate.add_argument("exe", type=Path)
    generate.add_argument("output", type=Path)
    observed = commands.add_parser("observe")
    observed.add_argument("exe", type=Path)
    observed.add_argument("kit", type=Path)
    observed.add_argument("schema", choices=CASES)
    observed.add_argument("exported", type=Path)
    args = parser.parse_args()
    try:
        if args.command == "make":
            make(args.exe, args.output)
            return 0
        return observe(args.exe, args.kit, args.schema, args.exported)
    except ValueError as error:
        print(str(error), file=sys.stderr)
        return 1
    except OSError:
        print("入力・出力のファイルか CLI を確認してください", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
