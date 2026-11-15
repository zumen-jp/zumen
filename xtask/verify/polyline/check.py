"""2 頂点の POLYLINE の共通属性を、監査前の ezdxf の読みと照合する。"""

import argparse
from copy import deepcopy
import hashlib
import io
import json
import math
from pathlib import Path
import subprocess
import sys

import ezdxf

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
sys.path.insert(0, str(HERE.parent))
from make import Zumen, read  # noqa: E402

VERSIONS = ("R2000", "R2004", "R2007", "R2010", "R2013", "R2018")
STYLES = {
    "unchanged": {"color": 2},
    "color": {"color": 6},
    "layer": {"layer": "QA_TARGET"},
    "linetype": {"linetype": "QA_DASH"},
    "combined": {"color": 6, "layer": "QA_TARGET", "linetype": "QA_DASH"},
}


def sha(data):
    return hashlib.sha256(data).hexdigest()


def write_json(path, value):
    with path.open("x", encoding="utf-8", newline="\n") as stream:
        stream.write(json.dumps(value, ensure_ascii=False, indent=2, allow_nan=False) + "\n")


def fixture(version, binary):
    """付属図面・線種を使わず、表も線種の模様も自作する。"""
    # https://ezdxf.readthedocs.io/en/stable/options.html
    ezdxf.options.write_fixed_meta_data_for_testing = True
    doc = ezdxf.new(version)
    doc.layers.new("QA_TARGET")
    doc.linetypes.new("QA_DASH", dxfattribs={"description": "QA dash", "pattern": [2, 1, -1]})
    model = doc.modelspace()
    model.add_line((0, 0), (3, 4), dxfattribs={"color": 3})
    # 公開 API: https://ezdxf.readthedocs.io/en/stable/dxfentities/polyline.html
    entity = model.add_polyline2d([(1, 2), (4, 6)], dxfattribs={"color": 2, "linetype": "Continuous"})
    for vertex in entity.vertices:
        vertex.dxf.color = 2
    stream = io.BytesIO() if binary else io.StringIO()
    doc.write(stream, fmt="bin" if binary else "asc")
    data = stream.getvalue()
    return data if binary else data.encode(doc.output_encoding)


def plain(value):
    if isinstance(value, dict):
        return {key: plain(item) for key, item in value.items()}
    if isinstance(value, (str, int, float, bool)) or value is None:
        return value
    return [plain(item) for item in value]


def entity_state(entity):
    return {"type": entity.dxftype(), "attributes": plain(entity.dxfattribs())}


def external(path):
    doc = ezdxf.readfile(path)
    polylines, others = [], {}
    for entity in doc.modelspace():
        if entity.dxftype() == "POLYLINE":
            polylines.append({"parent": entity_state(entity),
                              "vertices": [entity_state(vertex) for vertex in entity.vertices],
                              "seqend": entity_state(entity.seqend)})
        else:
            others[entity.dxf.handle] = entity_state(entity)
    state = {"polylines": polylines, "others": others,
             "tables": {name: [entity_state(item) for item in getattr(doc, name)] for name in ("layers", "linetypes")},
             "header": [doc.header.get("$PDMODE", 0), doc.header.get("$PDSIZE", 0)]}
    # audit が修復件数へ数えずに子の画層をそろえるため、値は必ず監査前に取得する。
    auditor = doc.audit()
    return {"state": state, "errors": len(auditor.errors), "fixes": len(auditor.fixes)}


def differences(expected, observed, path="state"):
    if isinstance(expected, dict):
        if not isinstance(observed, dict) or expected.keys() != observed.keys():
            return [path + ".keys"]
        return [item for key in sorted(expected) for item in differences(expected[key], observed[key], f"{path}.{key}")]
    if isinstance(expected, list):
        if not isinstance(observed, list) or len(expected) != len(observed):
            return [path + ".length"]
        return [item for index, (a, b) in enumerate(zip(expected, observed)) for item in differences(a, b, f"{path}[{index}]")]
    if isinstance(expected, (int, float)) and not isinstance(expected, bool):
        equal = isinstance(observed, (int, float)) and not isinstance(observed, bool) and math.isfinite(expected) and math.isfinite(observed) and math.isclose(expected, observed, rel_tol=1e-9, abs_tol=1e-9)
    else:
        equal = type(expected) is type(observed) and expected == observed
    return [] if equal else [path]


def evaluate(style, before, after):
    failures = []
    expected, observed = deepcopy(before["state"]), deepcopy(after["state"])
    if len(expected["polylines"]) != 1 or len(expected["polylines"][0]["vertices"]) != 2 or len(expected["others"]) != 1:
        return ["input.population"]
    polyline = expected["polylines"][0]
    polyline["parent"]["attributes"].update(style)
    for vertex in polyline["vertices"]:
        vertex["attributes"].update(style)
    # SEQEND の画層は元の値か変更先だけを許す。残る属性・順序・所有者は比べる。
    if len(observed["polylines"]) == 1:
        old_end = polyline["seqend"]["attributes"]
        new_end = observed["polylines"][0]["seqend"]["attributes"]
        if new_end.get("layer") not in (old_end.get("layer"), style.get("layer", old_end.get("layer"))):
            failures.append("seqend.layer")
        new_end["layer"] = old_end.get("layer")
    failures.extend(differences(expected, observed))
    if any(item["errors"] or item["fixes"] for item in (before, after)):
        failures.append("external.audit")
    return failures


def call(client, request):
    status, response = client.call(request)
    if status != "ok" or response.get("diagnostics"):
        raise ValueError(f"{request['op']}: diagnostics or status")
    return response


def apply(cli, original, style, request=None):
    client = Zumen(cli)
    try:
        client.push("input.dxf", original)
        call(client, {"op": "open", "input": "input.dxf"})
        if request is None:
            reading = read(client, "input.dxf")
            entity, = [item for item in reading["entities"] if item["kind"] == "polyline"]
            edit = {"kind": "restyle", "id": entity["id"], "hash": entity["hash"]}
            for key, value in style.items():
                if key == "color":
                    edit[key] = {"aci": value}
                elif key == "linetype":
                    edit["line_type"] = {"name": value}
                else:
                    edit[key] = value
            request = {"op": "apply", "doc": "input.dxf", "edits": [edit]}
        response = call(client, request)
        return client.take(response["blobs"][0]["id"]), request
    finally:
        client.close()


def check_case(cli, folder, version, binary, operation):
    folder.mkdir()
    original = fixture(version, binary)
    (folder / "input.dxf").write_bytes(original)
    before = external(folder / "input.dxf")
    style = STYLES[operation]
    output, request = apply(cli, original, style)
    repeated, _ = apply(cli, original, style, request)
    (folder / "output.dxf").write_bytes(output)
    after = external(folder / "output.dxf")
    failures = evaluate(style, before, after)
    if repeated != output:
        failures.append("determinism")
    result = {"case": folder.name, "version": version, "format": "binary" if binary else "ascii", "operation": operation,
              "expectedStyle": style, "inputExternal": before, "external": after,
              "inputSha256": sha(original), "outputSha256": sha(output),
              "deterministic": repeated == output, "failures": failures}
    write_json(folder / "requests.json", [request])
    write_json(folder / "result.json", result)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()
    output = args.out.resolve()
    if any((parent / ".git").exists() for parent in (output, *output.parents)) or (output.exists() and any(output.iterdir())):
        parser.error("リポジトリ外の空の --out を指定してください")
    if ezdxf.__version__ != "1.4.5":
        parser.error("ezdxf は 1.4.5 に固定してください")
    output.mkdir(parents=True, exist_ok=True)
    cli = args.cli.resolve(strict=True)
    sources = [{"path": path, "sha256": sha((ROOT / path).read_bytes())} for path in ("xtask/verify/polyline/check.py", "xtask/verify/make.py")]
    commit = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT).decode().strip()
    cli_sha = sha(cli.read_bytes())
    if sys.platform == "win32":
        original_popen = subprocess.Popen
        def hidden_popen(*args, **kwargs):
            return original_popen(*args, **{**kwargs, "creationflags": subprocess.CREATE_NO_WINDOW})
        subprocess.Popen = hidden_popen
    cases = []
    for version in VERSIONS:
        for binary in (False, True):
            for operation in STYLES:
                folder = output / f"{version.lower()}-{'bin' if binary else 'ascii'}-{operation}"
                cases.append(check_case(cli, folder, version, binary, operation))
    if any(sha((ROOT / item["path"]).read_bytes()) != item["sha256"] for item in sources) or sha(cli.read_bytes()) != cli_sha:
        raise ValueError("検証中にソースか CLI が変わりました")
    report = {"schemaVersion": 1, "issue": 885, "sourceCommit": commit,
              "cliSha256": cli_sha, "ezdxf": ezdxf.__version__, "python": sys.version.split()[0], "sources": sources,
              "inputProvenance": "ezdxf で自作した2頂点の2D POLYLINEと非対象のLINE各1件。表と線種も自作。",
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
