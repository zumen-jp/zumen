"""公開E57の全点をCLIとpye57で照らし、新しいリポジトリ外の検証キットへ記録する。"""

import argparse
from collections import Counter
from datetime import datetime, timezone
import hashlib
import importlib.util
import json
import math
from pathlib import Path
import shutil
import struct
import subprocess
import sys
import tomllib


HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]


def module(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    loaded = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(loaded)
    return loaded


oracle = module("e57_oracle", HERE / "read.py")
rpc = module("verify_make", HERE.parent / "make.py")


def digest(data):
    return hashlib.sha256(data).hexdigest()


def need(ok, reason):
    if not ok:
        raise ValueError(reason)


def compare(observation, expected):
    prefix = b"zumen-e57 external 1\n"
    need(expected.startswith(prefix), "external header")
    scans, count, *bounds = struct.unpack_from("<QQ6d", expected, len(prefix))
    need(scans == 1 and count == 7680, "external count")
    offset = len(prefix) + struct.calcsize("<QQ6d")
    need(len(expected) == offset + count * 30, "external length")
    need(observation["openStatus"] == observation["pointcloudStatus"] == "ok", "API status")
    opened, cloud = observation["open"], observation["pointcloud"]
    need(opened["result"]["format"] == "e57", "format")
    need(opened["diagnostics"] == [{"code": "e57.crs.missing", "severity": "warning",
                                   "count": 1, "at": [], "params": {}}], "open diagnostics")
    need(cloud["diagnostics"] == [], "pointcloud diagnostics")
    result = cloud["result"]
    need(result["points"] == count and result["attributes"] == ["position", "color"], "point metadata")
    need("crs" not in result and "vertical" not in result, "missing CRS")
    need(result["bounds"] == {"min": [bounds[1], bounds[0], bounds[2]],
                              "max": [bounds[4], bounds[3], bounds[5]]}, "bounds")
    selected = result["selected"]
    positions, colors = selected["positions"], selected["color"]
    need(selected["truncated"] is False, "truncated")
    need(len(positions) == len(colors) == count, "selected count")
    need(not ({"intensity", "classification"} & selected.keys()), "extra attributes")
    actual = []
    for position, color in zip(positions, colors, strict=True):
        need(len(position) == len(color) == 3, "row length")
        need(all(isinstance(v, (int, float)) and not isinstance(v, bool) and math.isfinite(v)
                 for v in position), "coordinates")
        need(all(isinstance(v, int) and not isinstance(v, bool) and 0 <= v <= 65535 for v in color), "color")
        north, east, height = position
        actual.append(struct.pack("<3d3H", east, north, height, *color))
    wanted = [expected[i:i + 30] for i in range(offset, len(expected), 30)]
    # APIは八分木の浅い段から選ぶ。ファイル順はRustで別に照合し、ここでは重複数も保って全点を比べる。
    need(Counter(actual) == Counter(wanted), "positions and colors")
    return {"points": count, "scans": scans, "selected": len(actual), "matched": True,
            "externalRecordSha256": digest(expected), "apiOrderSha256": digest(b"".join(actual)),
            "allPointsSha256": digest(b"".join(sorted(actual))),
            "diagnostics": [{"code": "e57.crs.missing", "count": 1}]}


def write_json(path, value):
    with path.open("x", encoding="utf-8", newline="\n") as file:
        json.dump(value, file, ensure_ascii=False, indent=2)
        file.write("\n")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    output, cli = args.out.resolve(), args.cli.resolve(strict=True)
    need(not output.exists(), "既存の出力先は使わない")
    need(not any((at / ".git").exists() for at in (output, *output.parents)), "出力先はGitの外にする")
    oracle.require_tools()
    ledger = tomllib.loads((ROOT / "samples/ledger.toml").read_text(encoding="utf-8"))
    output.mkdir(parents=True)
    z = rpc.Zumen(cli)
    samples = []
    try:
        for kind in ("double", "float"):
            relative = f"e57/libe57-coloured-cube-{kind}.e57"
            source = ROOT / "samples" / relative
            entry = next(item for item in ledger["samples"] if item["path"] == relative)
            content = source.read_bytes()
            need(entry["sha256"] == digest(content) and entry["bytes"] == len(content), "ledger fingerprint")
            need(entry["license"] == "CC0-1.0", "sample license")
            expected = oracle.observe(content)
            doc = f"cube-{kind}.e57"
            z.push(doc, content)
            open_status, opened = z.call({"op": "open", "input": doc})
            cloud_status, cloud = z.call({"op": "pointcloud", "doc": doc, "limit": 7680})
            observation = {"openStatus": open_status, "pointcloudStatus": cloud_status,
                           "open": opened, "pointcloud": cloud}
            result = compare(observation, expected)
            shutil.copyfile(source, output / doc)
            (output / f"{kind}-external.bin").write_bytes(expected)
            write_json(output / f"{kind}-api.json", observation)
            samples.append({"kind": kind, "source": entry, "input": doc, **result})
    finally:
        z.close()
    paths = ["xtask/verify/e57/read.py", "xtask/verify/e57/check_api.py",
             "crates/zumen-e57/tests/external.rs", "crates/zumen-e57/src/lib.rs",
             "crates/zumen-e57/src/lift.rs", "crates/zumen-abi/src/pointcloud.rs",
             "crates/zumen-pointcloud/src/lib.rs"]
    report = {"schemaVersion": 1, "createdAt": datetime.now(timezone.utc).isoformat(),
              "coreCommit": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
              "cliSha256": digest(cli.read_bytes()), "tools": oracle.PINS,
              "python": sys.version.split()[0], "platform": sys.platform,
              "sources": [{"path": path, "sha256": digest((ROOT / path).read_bytes())} for path in paths],
              "samples": samples, "application": "未実行",
              "limits": ["合成立方体2件だけ。実測・球座標・姿勢・複数スキャン・座標系は未確認。",
                         "両ファイルの点の値は同じで、float64の格納でもfloat32を超える精度は確認していない。",
                         "色は赤・緑・青の3色で、各成分の中間値は確認していない。"]}
    write_json(output / "result.json", report)
    (output / "README.md").write_text(
        "# E57の確認キット\n\nCC0の公開合成立方体2件。各7,680点、1スキャン、RGB、座標範囲は各軸−0.5〜0.5。\n"
        "出所・利用条件・入力の指紋・比較結果はresult.jsonにあります。APIの位置は北・東・高さです。\n"
        "*-external.binはpye57の観測値、*-api.jsonはCLIの実測応答です。実機表示は未確認です。\n",
        encoding="utf-8", newline="\n")
    print(json.dumps({"samples": len(samples), "points": sum(s["points"] for s in samples),
                      "matched": all(s["matched"] for s in samples)}, ensure_ascii=True))


if __name__ == "__main__":
    main()
