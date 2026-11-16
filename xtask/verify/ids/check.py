"""公開 IFC と自作 IDS の判定を、別のプログラムで動かす IfcTester と照らす。"""

import argparse
import copy
import hashlib
import importlib.util
import json
from pathlib import Path
import platform
import shutil
import subprocess
import sys
import threading
import tomllib

ROOT = Path(__file__).resolve().parents[3]
HERE = Path(__file__).resolve().parent


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


recipe = load("ids_cases", HERE / "cases.py")


def sha(data):
    return hashlib.sha256(data).hexdigest()


def json_bytes(value):
    return (json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True, allow_nan=False) + "\n").encode("utf-8")


def write_json(path, value):
    path.write_bytes(json_bytes(value))


def differences(wanted, observed, path=""):
    """値をログに写さず、型・欠落・余分な項目・順序を含む不一致の場所を返す。"""
    if type(wanted) is not type(observed):
        return [path + ":type"]
    if isinstance(wanted, dict):
        errors = [path + ":keys"] if wanted.keys() != observed.keys() else []
        for key in sorted(wanted.keys() & observed.keys()):
            errors.extend(differences(wanted[key], observed[key], path + "/" + key))
        return errors
    if isinstance(wanted, list):
        errors = [path + ":length"] if len(wanted) != len(observed) else []
        for index, (left, right) in enumerate(zip(wanted, observed)):
            errors.extend(differences(left, right, path + "/" + str(index)))
        return errors
    return [] if wanted == observed else [path + ":value"]


def ordered_findings(response):
    result = copy.deepcopy(response)
    if isinstance(result, dict) and isinstance(result.get("result"), dict):
        findings = result["result"].get("findings")
        if isinstance(findings, list):
            # 結果の列挙順は問わない。集合には変えず、同じ指摘の重複も数える。
            result["result"]["findings"] = sorted(findings, key=json_bytes)
    return result


def compare(external, core):
    expected = {"v": 1, "blobs": [], "diagnostics": [], "result": recipe.core_expected()}
    errors = differences(recipe.external_expected(), external, "external")
    errors.extend(differences(ordered_findings(expected), ordered_findings(core), "core"))
    return {"matched": not errors, "differences": errors}


def fingerprint(exe):
    commit = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    clean = subprocess.run(["git", "diff", "--quiet", "HEAD", "--", "Cargo.toml", "Cargo.lock", "rust-toolchain.toml", "crates", "bindings"], cwd=ROOT, check=False)
    if clean.returncode != 0:
        raise ValueError("runtime.dirty")
    sources = ["xtask/verify/make.py", "samples/ledger.toml"]
    sources += [path.relative_to(ROOT).as_posix() for path in sorted(HERE.iterdir()) if path.is_file()]
    return {"checkoutCommit": commit, "cliSha256": sha(exe.read_bytes()),
            "sourceSha256": {name: sha((ROOT / name).read_bytes()) for name in sources}}


def output_folder(path):
    path = path.resolve()
    if path.exists() or any((parent / ".git").exists() for parent in [path, *path.parents]):
        raise ValueError("output.must_be_new_and_outside_git")
    path.mkdir(parents=True)
    return path


def run(exe, output, result):
    result["stage"] = "inputs"
    before = fingerprint(exe)
    result.update(before)
    source = next(item for item in tomllib.loads((ROOT / "samples/ledger.toml").read_text(encoding="utf-8"))["samples"] if item["path"] == recipe.SOURCE)
    model = (ROOT / "samples" / recipe.SOURCE).read_bytes()
    if sha(model) != source["sha256"] or sha(model) != recipe.SOURCE_SHA256 or len(model) != source["bytes"]:
        raise ValueError("input.sha256")
    result["source"] = source
    (output / "model.ifc").write_bytes(model)
    (output / "licenses").mkdir()
    for name in ("text2bim-MIT.txt",):
        shutil.copyfile(ROOT / "licenses" / name, output / "licenses" / name)
    write_json(output / "expected.json", {"external": recipe.external_expected(), "core": recipe.core_expected()})
    result["stage"] = "external"
    external = subprocess.run([sys.executable, "-I", str(HERE / "external.py"), str(output)],
                              stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=120, check=False)
    if external.returncode != 0:
        raise ValueError("external.failed")
    tool_record = json.loads((output / "external-tools.json").read_bytes())
    if tool_record.get("complete") is not True:
        raise ValueError("external.incomplete")
    result["externalTools"] = tool_record["versions"]
    result["stage"] = "core"
    transport = load("ids_transport", ROOT / "xtask/verify/make.py")
    z = transport.Zumen(exe)
    timeout = threading.Timer(120, z.child.kill)
    timeout.start()
    records, comparisons = [], []

    def call(request):
        status, response = z.call(request)
        records.append({"request": request, "status": status, "response": response})
        write_json(output / "api.json", records)
        if status != "ok":
            raise ValueError("api.failed")
        return response

    try:
        z.push("model.ifc", model)
        opened = call({"op": "open", "input": "model.ifc"})
        result["openDiagnosticCodes"] = [item["code"] for item in opened["diagnostics"]]
        for label, _, _ in recipe.ENCODINGS:
            name = "names-" + label + ".ids"
            data = (output / name).read_bytes()
            z.push(name, data)
            request = {"op": "ids_check", "doc": "model.ifc", "ids": name, "limit": 1000}
            core = call(request)
            repeated = call(request)
            observed = json.loads((output / ("external-" + label + ".json")).read_bytes())
            comparison = compare(observed, core)
            repeated_equal = json_bytes(core) == json_bytes(repeated)
            comparison.update({"encoding": label, "inputSha256": sha(data), "repeatedResponseEqual": repeated_equal})
            comparison["matched"] &= repeated_equal
            write_json(output / ("core-" + label + ".json"), core)
            write_json(output / ("comparison-" + label + ".json"), comparison)
            comparisons.append(comparison)
        result["cases"] = comparisons
    finally:
        try:
            z.close()
        finally:
            timeout.cancel()
    if z.child.returncode != 0:
        raise ValueError("cli.exit")
    result["stage"] = "provenance"
    if fingerprint(exe) != before or sha((output / "model.ifc").read_bytes()) != recipe.SOURCE_SHA256:
        raise ValueError("inputs.changed")
    result["matched"] = all(item["matched"] for item in comparisons)
    result["complete"] = len(comparisons) == len(recipe.ENCODINGS)
    result["artifacts"] = {path.relative_to(output).as_posix(): sha(path.read_bytes())
                           for path in sorted(output.rglob("*")) if path.is_file() and path.name != "results.json"}
    result["stage"] = "complete"
    return 0 if result["matched"] and result["complete"] else 1


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()
    exe = args.cli.resolve(strict=True)
    output = output_folder(args.out)
    result = {"complete": False, "matched": False, "python": platform.python_version(), "os": platform.system(),
              "schemaBundled": False, "externalSourceRead": False, "applicationCompared": False, "wasmCompared": False}
    try:
        code = run(exe, output, result)
    except Exception as error:
        result["errorType"] = type(error).__name__
        code = 2
    finally:
        write_json(output / "results.json", result)
    print(json.dumps({"complete": result["complete"], "matched": result["matched"], "encodings": len(result.get("cases", []))}))
    return code


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as error:
        print(json.dumps({"complete": False, "errorType": type(error).__name__}))
        raise SystemExit(2) from None
