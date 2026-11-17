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
# 置き場所: 実寸の空間、用紙（Layout1）、名前付きブロックの定義（モデル空間に INSERT を 1 つ置く）
PLACEMENTS = ("model", "paper", "block")
BLOCK = "QA_BLOCK"
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


def container(doc, placement):
    if placement == "paper":
        return doc.paperspace()
    if placement == "block":
        return doc.blocks.get(BLOCK)
    return doc.modelspace()


def fixture(version, binary, placement="model"):
    """付属図面・線種を使わず、表も線種の模様も自作する。"""
    # https://ezdxf.readthedocs.io/en/stable/options.html
    ezdxf.options.write_fixed_meta_data_for_testing = True
    doc = ezdxf.new(version)
    doc.layers.new("QA_TARGET")
    doc.linetypes.new("QA_DASH", dxfattribs={"description": "QA dash", "pattern": [2, 1, -1]})
    if placement == "block":
        doc.blocks.new(BLOCK)
        doc.modelspace().add_blockref(BLOCK, (10, 20))
    model = container(doc, placement)
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


def external(path, placement="model"):
    doc = ezdxf.readfile(path)
    polylines, others = [], {}
    for entity in container(doc, placement):
        if entity.dxftype() == "POLYLINE":
            polylines.append({"parent": entity_state(entity),
                              "vertices": [entity_state(vertex) for vertex in entity.vertices],
                              "seqend": entity_state(entity.seqend)})
        else:
            others[entity.dxf.handle] = entity_state(entity)
    # 置き場所の外（ブロックの INSERT など）も、変わらないことを比べる
    outside = {} if placement == "model" else {entity.dxf.handle: entity_state(entity) for entity in doc.modelspace()}
    state = {"polylines": polylines, "others": others, "outside": outside,
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


class WasiZumen(Zumen):
    """wasm32-wasip1 の zumen.wasm を、Node.js の WASI（bindings/conformance/wasi.mjs）の上の `serve --stdio` で動かす"""

    def __init__(self, wasm):
        command = ["node", "--no-warnings", str(ROOT / "bindings/conformance/wasi.mjs"), str(wasm), "serve", "--stdio"]
        self.child = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=subprocess.PIPE)
        self.next = 0
        self.rpc("session_new", {})


def block_place(path):
    """定義の空間の ID は BLOCK のハンドルから決まる（`space/h<ハンドル>`。xtask/verify/make.py の definition_places）"""
    handle = ezdxf.readfile(path).blocks.get(BLOCK).block.dxf.handle
    return f"space/h{int(handle, 16):X}"


def apply(runtime, original, style, request=None, place=None):
    client = runtime()
    try:
        client.push("input.dxf", original)
        call(client, {"op": "open", "input": "input.dxf"})
        if request is None:
            reading = read(client, "input.dxf", {BLOCK: place} if place else None)
            items = reading["definitions"][BLOCK]["entities"] if place else reading["entities"]
            entity, = [item for item in items if item["kind"] == "polyline"]
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


def check_case(runtime, folder, version, binary, operation, placement):
    folder.mkdir()
    original = fixture(version, binary, placement)
    (folder / "input.dxf").write_bytes(original)
    before = external(folder / "input.dxf", placement)
    style = STYLES[operation]
    place = block_place(folder / "input.dxf") if placement == "block" else None
    output, request = apply(runtime, original, style, place=place)
    repeated, _ = apply(runtime, original, style, request)
    (folder / "output.dxf").write_bytes(output)
    after = external(folder / "output.dxf", placement)
    failures = evaluate(style, before, after)
    if repeated != output:
        failures.append("determinism")
    result = {"case": folder.name, "version": version, "format": "binary" if binary else "ascii", "placement": placement,
              "operation": operation,
              "expectedStyle": style, "inputExternal": before, "external": after,
              "inputSha256": sha(original), "outputSha256": sha(output),
              "deterministic": repeated == output, "failures": failures}
    write_json(folder / "requests.json", [request])
    write_json(folder / "result.json", result)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    engine = parser.add_mutually_exclusive_group(required=True)
    engine.add_argument("--cli", type=Path, help="ネイティブの zumen")
    engine.add_argument("--wasi", type=Path, help="wasm32-wasip1 の zumen.wasm（Node.js 22 以降で動かす）")
    parser.add_argument("--out", required=True, type=Path)
    parser.add_argument("--same-as", type=Path, help="別の実行の results.json。各条件の出力の SHA-256 が同じことも確かめる")
    args = parser.parse_args()
    output = args.out.resolve()
    if any((parent / ".git").exists() for parent in (output, *output.parents)) or (output.exists() and any(output.iterdir())):
        parser.error("リポジトリ外の空の --out を指定してください")
    if ezdxf.__version__ != "1.4.5":
        parser.error("ezdxf は 1.4.5 に固定してください")
    output.mkdir(parents=True, exist_ok=True)
    cli = (args.cli or args.wasi).resolve(strict=True)
    runtime = (lambda: Zumen(cli)) if args.cli else (lambda: WasiZumen(cli))
    others = None
    if args.same_as:
        others = {row["case"]: row["outputSha256"] for row in json.loads(args.same_as.read_text(encoding="utf-8"))["cases"]}
    sources = [{"path": path, "sha256": sha((ROOT / path).read_bytes())}
               for path in ("xtask/verify/polyline/check.py", "xtask/verify/make.py", "bindings/conformance/wasi.mjs")]
    commit = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT).decode().strip()
    cli_sha = sha(cli.read_bytes())
    if sys.platform == "win32":
        original_popen = subprocess.Popen
        def hidden_popen(*args, **kwargs):
            return original_popen(*args, **{**kwargs, "creationflags": subprocess.CREATE_NO_WINDOW})
        subprocess.Popen = hidden_popen
    cases = []
    for placement in PLACEMENTS:
        for version in VERSIONS:
            for binary in (False, True):
                for operation in STYLES:
                    folder = output / f"{placement}-{version.lower()}-{'bin' if binary else 'ascii'}-{operation}"
                    case = check_case(runtime, folder, version, binary, operation, placement)
                    if others is not None and others.get(case["case"]) != case["outputSha256"]:
                        case["failures"].append("same-as")
                    cases.append(case)
    if any(sha((ROOT / item["path"]).read_bytes()) != item["sha256"] for item in sources) or sha(cli.read_bytes()) != cli_sha:
        raise ValueError("検証中にソースか CLI が変わりました")
    report = {"schemaVersion": 1, "issue": 885, "sourceCommit": commit,
              "runtime": "native" if args.cli else "wasm32-wasip1", "cliSha256": cli_sha,
              "sameAs": sha(args.same_as.read_bytes()) if args.same_as else None, "ezdxf": ezdxf.__version__, "python": sys.version.split()[0], "sources": sources,
              "inputProvenance": "ezdxf で自作した2頂点の2D POLYLINEと非対象のLINE各1件を、実寸・用紙・名前付きブロックに置く。表と線種も自作。",
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
