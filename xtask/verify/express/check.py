"""公開 IFC EXPRESS の宣言を、同じ入力を読んだ IfcOpenShell と照らす。"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import urllib.request

ROOT = Path(__file__).resolve().parents[3]
COMMIT = "dcf43f7f167b2cdf8007c1ea1ceec32de772f44f"
BASE = f"https://raw.githubusercontent.com/buildingSMART/Standards-builds/{COMMIT}/"
SOURCES = [
    ("IFC2X3", "IFC/RELEASE/IFC2x3/TC1/EXPRESS/IFC2X3_TC1.exp", 257080,
     "bf89eda341bccec041df37dffb79f0b8d4e3d5411d07541560d05bf69d53fc3a"),
    ("IFC4", "IFC/RELEASE/IFC4/ADD2_TC1/EXPRESS/IFC4.exp", 359976,
     "a3e46d39a85c2b683e7167572165d74b4ff6f8ef7e7c1e7f314a4980a63c9edb"),
    ("IFC4X3_ADD2", "IFC/RELEASE/IFC4_3/HTML/IFC4X3_ADD2.exp", 403825,
     "f67c8762b13a099c28082061e6f16b9ef1284ceec34069792afc702725675860"),
]


def sha(data):
    return hashlib.sha256(data).hexdigest()


def write_json(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8", newline="\n")


def in_repository(path):
    return any((parent / ".git").exists() for parent in (path, *path.parents))


def parameter_type(value):
    levels = []
    while aggregate := value.as_aggregation_type():
        levels.append({"kind": aggregate.type_of_aggregation_string().upper(),
                       "lower": None if aggregate.bound1() < 0 else aggregate.bound1(),
                       "upper": None if aggregate.bound2() < 0 else aggregate.bound2()})
        value = aggregate.type_of_element()
    if named := value.as_named_type():
        item = named.declared_type().name().upper()
    else:
        item = value.as_simple_type().declared_type().upper()
    return {"levels": levels, "item": item}


def external_worker(source, output):
    import ifcopenshell
    import ifcopenshell.express
    if ifcopenshell.version != "0.9.0.post1":
        raise ValueError("IfcOpenShell は 0.9.0.post1 に固定してください")
    parsed = ifcopenshell.express.parse(str(source))
    try:
        entities = []
        for entity in parsed.schema.entities():
            parent = entity.supertype()
            entities.append({"name": entity.name().upper(), "abstract": entity.is_abstract(),
                             "supertypes": [parent.name().upper()] if parent else [],
                             "attributes": [{"name": attr.name().upper(), "optional": attr.optional(),
                                             "type": parameter_type(attr.type_of_attribute())} for attr in entity.attributes()]})
        write_json(output, {"schema": parsed.schema.name().upper(), "entities": sorted(entities, key=lambda e: e["name"])})
    finally:
        # 文書化された所有権の手順を使う。観測後は子プロセスが終了し、スキーマを再利用しない。
        parsed.disown()


def differences(core, external):
    result = []
    if core["schema"].upper() != external["schema"]:
        result.append("schema")
    left = {entity["name"]: entity for entity in core["entities"]}
    right = {entity["name"]: entity for entity in external["entities"]}
    if len(left) != len(core["entities"]) or len(right) != len(external["entities"]):
        result.append("duplicate-entity")
    for name in sorted(left.keys() | right.keys()):
        if name not in left or name not in right:
            result.append(f"{name}.missing")
            continue
        for field in ("abstract", "supertypes", "attributes"):
            if left[name][field] != right[name][field]:
                result.append(f"{name}.{field}")
    return result


def check_one(temporary, source):
    name, path, size, expected_sha = source
    with urllib.request.urlopen(BASE + path, timeout=30) as response:
        data = response.read(size + 1)
    if len(data) != size or sha(data) != expected_sha:
        raise ValueError(f"{name}: 公開元のハッシュまたはサイズが違います")
    schema = temporary / f"{name}.exp"
    core_path, external_path = temporary / f"{name}-core.json", temporary / f"{name}-external.json"
    schema.write_bytes(data)
    env = {**os.environ, "ZUMEN_EXPRESS_SOURCE": str(schema), "ZUMEN_EXPRESS_OBSERVED": str(core_path)}
    core = subprocess.run(["cargo", "test", "--locked", "-p", "zumen-ifc", "--lib",
                           "express::external_test::writes_external_declarations", "--", "--exact"],
                          cwd=ROOT, env=env, capture_output=True, timeout=300)
    record = {"schema": name, "sourceUrl": BASE + path, "sourceBytes": size, "sourceSha256": expected_sha,
              "coreExit": core.returncode, "verified": False}
    if core.returncode != 0 or not core_path.exists():
        record["failure"] = "core-read"
        return record
    child = subprocess.run([sys.executable, "-I", str(Path(__file__).resolve()), "--worker", str(schema), "--out", str(external_path)],
                           cwd=temporary, capture_output=True, timeout=300)
    record["externalExit"] = child.returncode
    if child.returncode != 0 or not external_path.exists():
        record["failure"] = "external-read"
        return record
    core_bytes, external_bytes = core_path.read_bytes(), external_path.read_bytes()
    observed, other = json.loads(core_bytes), json.loads(external_bytes)
    mismatch = differences(observed, other)
    record.update({"entities": len(observed["entities"]), "externalEntities": len(other["entities"]),
                   "attributes": sum(len(entity["attributes"]) for entity in observed["entities"]),
                   "externalAttributes": sum(len(entity["attributes"]) for entity in other["entities"]),
                   "rulesNotEvaluated": observed["rulesNotEvaluated"], "mismatchCount": len(mismatch),
                   "mismatchFields": mismatch[:20], "coreObservationSha256": sha(core_bytes),
                   "externalObservationSha256": sha(external_bytes), "verified": not mismatch})
    return record


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--worker", type=Path, help=argparse.SUPPRESS)
    args = parser.parse_args()
    output = args.out.resolve()
    if in_repository(output) or output.exists():
        parser.error("リポジトリ外の新規の --out を指定してください")
    if args.worker:
        if in_repository(args.worker.resolve()):
            parser.error("スキーマはリポジトリ外の一時領域で扱ってください")
        external_worker(args.worker, output)
        return 0
    output.mkdir(parents=True)
    sources = ["crates/zumen-ifc/src/express.rs", "crates/zumen-ifc/tests/support/express_external.rs",
               "xtask/verify/express/check.py", "xtask/verify/express/test_check.py"]
    fingerprints = [{"path": path, "sha256": sha((ROOT / path).read_bytes())} for path in sources]
    report = {"schemaVersion": 1, "issue": 663, "complete": False,
              "sourceCommit": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT).decode().strip(),
              "sources": fingerprints, "ifcopenshell": "0.9.0.post1", "python": sys.version.split()[0], "cases": [],
              "limits": ["自分で宣言した明示属性の名前・順序・型・省略可否と、実体の名前・抽象指定・上位型を比較。",
                         "集まりの種別と上下限、直接指す型名を比較。別名の定義、列挙と選択の全項目は未比較。",
                         "導出・逆属性、継承属性の再宣言、集まりの項目の省略可否と UNIQUE は未比較。",
                         "公開 IFC の validate と WHERE・UNIQUE・RULE の評価は未実行。別 OS/wasm は未照合。"]}
    with tempfile.TemporaryDirectory(prefix="zumen-express-check-") as folder:
        temporary = Path(folder).resolve()
        assert temporary.parent == Path(tempfile.gettempdir()).resolve() and temporary.name.startswith("zumen-express-check-")
        for source in SOURCES:
            record = check_one(temporary, source)
            report["cases"].append(record)
            write_json(output / "results.json", report)
            print(json.dumps({"schema": record["schema"], "verified": record["verified"], "entities": record.get("entities"),
                              "mismatchCount": record.get("mismatchCount"), "failure": record.get("failure")}), flush=True)
    if any(sha((ROOT / item["path"]).read_bytes()) != item["sha256"] for item in fingerprints):
        raise ValueError("検証中にソースが変わりました")
    report["complete"] = True
    write_json(output / "results.json", report)
    return 0 if all(record["verified"] for record in report["cases"]) else 1


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Exception as error:
        print(json.dumps({"status": "runner_error", "kind": type(error).__name__}))
        sys.exit(2)
