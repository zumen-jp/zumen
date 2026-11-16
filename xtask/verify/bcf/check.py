"""BCF 2.1/3.0 の書き出しと外部実装からの再読を、独立した期待値と照らす。"""

import argparse
import copy
import importlib.util
import json
from pathlib import Path
import platform
import subprocess
import sys
import threading

ROOT = Path(__file__).resolve().parents[3]
HERE = Path(__file__).resolve().parent


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


comparison = load("bcf_compare", HERE / "compare.py")
prepare = load("bcf_prepare", HERE / "prepare.py")
acc = load("bcf_inputs", ROOT / "xtask/verify/acc/make.py")


def write_json(path, value):
    path.write_bytes((json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True, allow_nan=False) + "\n").encode("utf-8"))


def fingerprint(exe):
    commit = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    clean = subprocess.run(["git", "diff", "--quiet", "HEAD", "--", "Cargo.toml", "Cargo.lock", "rust-toolchain.toml", "crates", "bindings"], cwd=ROOT, check=False)
    if clean.returncode != 0:
        raise ValueError("runtime.dirty")
    paths = [ROOT / name for name in ("xtask/verify/make.py", "xtask/verify/acc/make.py", "samples/ledger.toml")]
    paths += [path for path in HERE.rglob("*") if path.is_file() and path.suffix in (".py", ".cs", ".csproj", ".Config", ".md")]
    return {"checkoutCommit": commit, "cliSha256": prepare.sha(exe.read_bytes()),
            "sourceSha256": {path.relative_to(ROOT).as_posix(): prepare.sha(path.read_bytes()) for path in sorted(paths)}}


def check_tools(folder):
    record = json.loads((folder / "tools.json").read_bytes())
    if record["complete"] is not True or record["runtime"]["regexIsFramework"] is not True:
        raise ValueError("tools.incomplete")
    expected_sources = {"prepare.py", "external/ReadBcf.csproj", "external/Program.cs", "external/NuGet.Config"}
    if set(record["sourceSha256"]) != expected_sources:
        raise ValueError("tools.sources")
    expected_packages = [(name, version, digest, Path(entry).name) for name, version, _, entry, digest in prepare.PACKAGES]
    observed_packages = [(item["name"], item["version"], item["packageSha256"], item["assembly"]) for item in record["packages"]]
    if observed_packages != expected_packages or "ReadBcf.dll" not in record["binarySha256"]:
        raise ValueError("tools.packages")
    for name, digest in record["sourceSha256"].items():
        if prepare.sha((HERE / name).read_bytes()) != digest:
            raise ValueError("tools.source_changed")
    for name, digest in record["binarySha256"].items():
        if Path(name).name != name or "/" in name or "\\" in name:
            raise ValueError("tools.binary_path")
        if prepare.sha((folder / "bin" / name).read_bytes()) != digest:
            raise ValueError("tools.binary_changed")
    for package in record["packages"]:
        if prepare.sha((folder / "bin" / package["assembly"]).read_bytes()) != package["assemblySha256"]:
            raise ValueError("tools.assembly_changed")
    return record


def run(exe, tools, output, summary):
    summary["stage"] = "inputs"
    before = fingerprint(exe)
    tool_record = check_tools(tools)
    summary.update(before)
    write_json(output / "tools.json", tool_record)
    kit = output / "input"
    acc.make(exe, kit)
    manifest = json.loads((kit / "manifest.json").read_bytes())
    summary["source"] = manifest["source"]
    expected_review = json.loads((kit / "edited-review.json").read_bytes())
    reference = {key: value for key, value in expected_review.items() if key != "topics"}
    if len(reference["targets"]) != 1 or {key: reference["targets"][0][key] for key in ("id", "name", "format")} != {"id": "model", "name": "model.ifc", "format": "ifc"}:
        raise ValueError("review.target")
    write_json(output / "core-reference.json", reference)
    records, results = [], []
    z = acc.client(exe)
    timeout = threading.Timer(180, z.child.kill)
    timeout.start()

    def call(request):
        response = acc.call(z, request, records)
        write_json(output / "api.json", records)
        return response

    def external(version, source, observation, rewritten=None):
        command = ["dotnet", str(tools / "bin/ReadBcf.dll"), version, str(source), str(observation)]
        if rewritten is not None:
            command.append(str(rewritten))
        # 外部実装の一時ファイルが Git の作業ツリーへ出ないよう、作業場所も Git の外にする。
        completed = subprocess.run(command, cwd=output, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, timeout=120, check=False)
        status = json.loads(completed.stdout)
        if completed.returncode != 0 or status.get("complete") is not True:
            write_json(output / "external-error.json", status)
            raise ValueError("external.failed")
        return json.loads(observation.read_bytes())

    try:
        acc.open_model(z, (kit / "model.ifc").read_bytes(), records)
        summary["stage"] = "cases"
        for label, version, mixed, snapshots in (("rewritten-2.1", "2.1", False, True), ("new-2.1", "2.1", True, False), ("new-3.0", "3.0", True, False)):
            wanted = comparison.expected(acc, version, mixed, snapshots)
            write_json(output / (label + "-expected.json"), wanted)
            source = output / (label + ".bcf")
            repeat_equal = None
            if label == "rewritten-2.1":
                source.write_bytes((kit / "for-acc.bcf").read_bytes())
            else:
                review = copy.deepcopy(expected_review)
                for topic in review["topics"]:
                    if int(topic["id"][:8], 16) % 2 == 0:
                        topic["locations"][0]["camera"] = {"projection": "orthographic", "position": [-5, 1.5, 1.5], "direction": [1, 0, 0], "up": [0, 0, 1], "width": 12}
                name = label + "-review.json"
                write_json(output / name, review)
                z.push(name, (output / name).read_bytes())
                request = {"op": "bcf_write", "review": name, "target": acc.TARGET, "version": version}
                first = call(request)
                data = acc.blob(z, first)
                second = call(request)
                repeat_equal = data == acc.blob(z, second) and first["diagnostics"] == second["diagnostics"] == []
                source.write_bytes(data)
            rewritten = output / (label + "-external.bcf")
            direct = external(version, source, output / (label + "-direct-raw.json"), rewritten)
            reread = external(version, rewritten, output / (label + "-reread-raw.json"))
            direct_check = comparison.compare_external(wanted, direct)
            reread_check = comparison.compare_external(wanted, reread)
            name = label + "-external.bcf"
            z.push(name, rewritten.read_bytes())
            response = call({"op": "bcf_read", "doc": acc.MODEL, "input": name, "target": acc.TARGET})
            review = json.loads(acc.blob(z, response))
            core_check = comparison.compare_core(wanted, response, review, reference)
            write_json(output / (label + "-core.json"), {"response": response, "review": review})
            checks = {"direct": direct_check, "reread": reread_check, "core": core_check}
            write_json(output / (label + "-comparison.json"), checks)
            results.append({"case": label, "version": version, "topics": len(wanted["topics"]), "snapshotCount": 4 if snapshots else 0,
                            "repeatedWriteEqual": repeat_equal, "checks": {key: value["matched"] for key, value in checks.items()},
                            "matched": all(value["matched"] for value in checks.values()) and repeat_equal is not False,
                            "coreFileSha256": prepare.sha(source.read_bytes()), "externalFileSha256": prepare.sha(rewritten.read_bytes())})
        summary["cases"] = results
    finally:
        try:
            z.close()
        finally:
            timeout.cancel()
    if z.child.returncode != 0:
        raise ValueError("cli.exit")
    summary["stage"] = "provenance"
    if fingerprint(exe) != before or check_tools(tools) != tool_record:
        raise ValueError("inputs.changed")
    summary["matched"] = all(item["matched"] for item in results)
    summary["complete"] = len(results) == 3
    summary["artifacts"] = {path.relative_to(output).as_posix(): prepare.sha(path.read_bytes()) for path in sorted(output.rglob("*"))
                            if path.is_file() and path.name != "results.json"}
    summary["stage"] = "complete"
    return 0 if summary["matched"] and summary["complete"] else 1


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", required=True, type=Path)
    parser.add_argument("--tools", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()
    exe, tools = args.cli.resolve(strict=True), args.tools.resolve(strict=True)
    output = prepare.new_folder(args.out)
    summary = {"complete": False, "matched": False, "python": platform.python_version(), "os": platform.system(),
               "externalSourceRead": False, "schemaPublished": False, "applicationCompared": False, "wasmCompared": False}
    try:
        code = run(exe, tools, output, summary)
    except Exception as error:
        summary["errorType"] = type(error).__name__
        code = 2
    finally:
        write_json(output / "results.json", summary)
    print(json.dumps({"complete": summary["complete"], "matched": summary["matched"], "cases": len(summary.get("cases", []))}))
    return code


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    try:
        raise SystemExit(main())
    except Exception as error:
        print(json.dumps({"complete": False, "errorType": type(error).__name__}))
        raise SystemExit(2) from None
