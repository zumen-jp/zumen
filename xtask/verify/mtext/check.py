"""MTEXT の DXF 角度を、操作で決めた度数と ezdxf の読みで照合する。"""

import argparse
import hashlib
import io
import json
import math
from pathlib import Path
import subprocess
import sys

import ezdxf
from ezdxf import recover

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
sys.path.insert(0, str(HERE.parent))
from make import Zumen, read  # noqa: E402

VERSIONS = ("R2000", "R2004", "R2007", "R2010", "R2013", "R2018")


def sha(data):
    return hashlib.sha256(data).hexdigest()


def write_json(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8", newline="\n")


def close(expected, actual):
    return isinstance(actual, (int, float)) and not isinstance(actual, bool) and math.isfinite(actual) and math.isclose(expected, actual, rel_tol=1e-9, abs_tol=1e-9)


def fixture(version, binary, rotation):
    """既定の表も ezdxf が生成する。製品に付属する図面や文字のファイルを使わない。"""
    doc = ezdxf.new(version)
    model = doc.modelspace()
    model.add_line((0, 0), (3, 4))
    if rotation is not None:
        model.add_mtext("L5", dxfattribs={"insert": (10, 20), "char_height": 2.5, "width": 20, "rotation": rotation})
    if binary:
        stream = io.BytesIO()
        doc.write(stream, fmt="bin")
        return stream.getvalue()
    stream = io.StringIO()
    doc.write(stream)
    return stream.getvalue().encode(doc.output_encoding)


def external(path):
    if path.read_bytes().startswith(b"AutoCAD Binary DXF"):
        doc = ezdxf.readfile(path)
        auditor = doc.audit()
    else:
        doc, auditor = recover.readfile(path)
    model = doc.modelspace()
    return {
        "rotation": [item.get_rotation() for item in model.query("MTEXT")],
        "lines": [{"start": list(item.dxf.start), "end": list(item.dxf.end)} for item in model.query("LINE")],
        "errors": len(auditor.errors), "fixes": len(auditor.fixes),
    }


def call(z, request):
    status, response = z.call(request)
    if status != "ok" or response.get("diagnostics"):
        raise ValueError(f"{request['op']}: diagnostics or status")
    return response


def opened(z, data):
    z.push("input.dxf", data)
    call(z, {"op": "open", "input": "input.dxf"})
    return read(z, "input.dxf")


def target(reading):
    item, = [row for row in reading["entities"] if row["kind"] == "formatted_text"]
    return item


def shape(rotation):
    return {"shape": "formatted_text", "content": "L5", "position": [10, 20], "height": 2.5,
            "width": 20, "rotation": rotation, "attachment": "top_left", "style": "Standard"}


def evaluate(expected, core_rotation, observed, control):
    failures = []
    if not close(expected, core_rotation):
        failures.append("core.rotation")
    if len(observed["rotation"]) != 1 or not close(expected, observed["rotation"][0]):
        failures.append("external.rotation")
    if observed["lines"] != control["lines"]:
        failures.append("non_target.line")
    if any(row["errors"] or row["fixes"] for row in (control, observed)):
        failures.append("external.audit")
    return failures


def check_case(cli, folder, version, binary, operation):
    folder.mkdir()
    original = fixture(version, binary, 35 if operation == "read" else None if operation == "add" else 0)
    (folder / "input.dxf").write_bytes(original)
    before = external(folder / "input.dxf")
    expected = 90 if operation == "transform" else 35
    z = Zumen(cli)
    requests = []
    try:
        reading = opened(z, original)
        if operation == "read":
            output = original
        else:
            if operation == "add":
                edit = {"kind": "add", "place": "space/0", "layer": "0", "geometry": shape(expected)}
            else:
                item = target(reading)
                edit = {"kind": operation, "id": item["id"], "hash": item["hash"]}
                if operation == "replace":
                    edit["geometry"] = {**shape(expected), "style": item["geometry"]["style"]}
                else:
                    edit["matrix"] = [0, -1, 0, 1, 0, 0]
            request = {"op": "apply", "doc": "input.dxf", "edits": [edit]}
            requests.append(request)
            response = call(z, request)
            output = z.take(response["blobs"][0]["id"])
            repeated = Zumen(cli)
            try:
                opened(repeated, original)
                again = call(repeated, request)
                if repeated.take(again["blobs"][0]["id"]) != output:
                    raise ValueError("same input and operation produced different bytes")
            finally:
                repeated.close()
            z.push("output.dxf", output)
            call(z, {"op": "open", "input": "output.dxf"})
            reading = read(z, "output.dxf")
        core_rotation = target(reading)["geometry"]["rotation"]
    finally:
        z.close()
    (folder / "output.dxf").write_bytes(output)
    observed = external(folder / "output.dxf")
    result = {"version": version, "format": "binary" if binary else "ascii", "operation": operation,
              "expectedDegrees": expected, "coreDegrees": core_rotation, "external": observed,
              "inputExternal": before, "inputSha256": sha(original), "outputSha256": sha(output),
              "failures": evaluate(expected, core_rotation, observed, before),
              "deterministic": None if operation == "read" else True}
    write_json(folder / "requests.json", requests)
    write_json(folder / "result.json", result)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()
    output = args.out.resolve()
    if output.is_relative_to(ROOT) or (output.exists() and any(output.iterdir())):
        parser.error("リポジトリ外の空の --out を指定してください")
    if ezdxf.__version__ != "1.4.5":
        parser.error("ezdxf は 1.4.5 に固定してください")
    output.mkdir(parents=True, exist_ok=True)
    cli = args.cli.resolve(strict=True)
    sources = [{"path": path, "sha256": sha((ROOT / path).read_bytes())}
               for path in ("xtask/verify/mtext/check.py", "xtask/verify/make.py")]
    commit = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT).decode().strip()
    cli_sha = sha(cli.read_bytes())
    cases = []
    for version in VERSIONS:
        for binary in (False, True):
            for operation in ("read", "add", "replace", "transform"):
                folder = output / f"{version.lower()}-{'bin' if binary else 'ascii'}-{operation}"
                cases.append(check_case(cli, folder, version, binary, operation))
    if any(sha((ROOT / item["path"]).read_bytes()) != item["sha256"] for item in sources) or sha(cli.read_bytes()) != cli_sha:
        raise ValueError("検証中にソースか CLI が変わりました")
    report = {"schemaVersion": 1, "issue": 882, "sourceCommit": commit,
              "cliSha256": cli_sha, "ezdxf": ezdxf.__version__, "python": sys.version.split()[0],
              "sources": sources,
              "inputProvenance": "全入力を ezdxf で自作。MTEXT 1 件と非対象の LINE 1 件。",
              "application": "not_run", "cases": cases}
    write_json(output / "results.json", report)
    failed = sum(bool(row["failures"]) for row in cases)
    print(json.dumps({"cases": len(cases), "failed": failed, "application": "not_run"}))
    return int(failed > 0)


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Exception as error:
        print(json.dumps({"status": "runner_error", "kind": type(error).__name__}))
        sys.exit(2)
