"""公開 PLY の外部照合と、MeshLab に渡す期待値表を作る（#870）。"""

import argparse
import hashlib
import json
import re
import subprocess
import sys
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
SAMPLE = "ply/pdal-mesh-fixed.ply"
SAMPLE_SHA = "cae59908c8e0b105a75cac91363068159508c5731a46b48993b5f4a391afbeb0"
LICENSE = "licenses/pdal-2017.txt"
VERTICES = [[1, 1, 0], [2, 1, 0], [1, 2, 0], [2, 2, 2]]
TESTS = sorted([
    "public_sample_stops_at_input_elements_and_work_budgets",
    "public_pdal_sample_matches_external_vertices_and_reports_faces",
    "compares_synthetic_positions_and_attributes_in_all_encodings",
    "compares_ascii_float32_positions_with_external_precision",
])


def sha(data):
    return hashlib.sha256(data).hexdigest()


def encoded(value):
    return (json.dumps(value, ensure_ascii=False, indent=2, allow_nan=False) + "\n").encode("utf8")


def output_folder(path):
    path = path.resolve()
    if any((parent / ".git").exists() for parent in [path, *path.parents]):
        raise ValueError("出力先は Git の作業ツリーの外にしてください")
    if path.exists() and (not path.is_dir() or any(path.iterdir())):
        raise ValueError("出力先は新しいフォルダーか空のフォルダーにしてください")
    return path


def sample_files(root):
    ledger = tomllib.loads((root / "samples/ledger.toml").read_text("utf8"))
    entry = next(item for item in ledger["samples"] if item["path"] == SAMPLE)
    data = (root / "samples" / SAMPLE).read_bytes()
    if entry["sha256"] != SAMPLE_SHA or sha(data) != SAMPLE_SHA or len(data) != entry["bytes"]:
        raise ValueError("公開サンプルと台帳のハッシュが一致しません")
    if entry["license"] != "BSD-3-Clause" or len(data) != 277:
        raise ValueError("確認済みの公開サンプルの条件が変わっています")
    license_bytes = (root / LICENSE).read_bytes()
    return entry, data, license_bytes


def test_result(stdout, code):
    passed = sorted(re.findall(r"^test ([a-z0-9_]+) \.\.\. ok\s*$", stdout, re.MULTILINE))
    if code != 0 or passed != TESTS or "4 passed; 0 failed" not in stdout:
        raise ValueError("PLY の外部照合 4 本がそろって通過しませんでした")
    return passed


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()
    output = output_folder(args.out)
    entry, data, license_bytes = sample_files(ROOT)
    lock = tomllib.loads((ROOT / "Cargo.lock").read_text("utf8"))
    external = next(item for item in lock["package"] if item["name"] == "ply-rs")
    if external["version"] != "0.1.3":
        raise ValueError("照合器の版が変わっています")
    source_paths = ["Cargo.lock", "crates/zumen-ply/Cargo.toml", "crates/zumen-ply/tests/external.rs",
                    "xtask/verify/ply/make.py", "samples/ledger.toml", LICENSE]
    source_paths += sorted(path.relative_to(ROOT).as_posix() for path in (ROOT / "crates/zumen-ply/src").rglob("*.rs"))
    sources = [{"path": path, "sha256": sha((ROOT / path).read_bytes())} for path in source_paths]
    command = ["cargo", "test", "--locked", "-p", "zumen-ply", "--test", "external", "--", "--color", "never"]
    result = subprocess.run(command, cwd=ROOT, capture_output=True, timeout=600)
    passed = test_result(result.stdout.decode("utf8", errors="replace"), result.returncode)
    if any(sha((ROOT / item["path"]).read_bytes()) != item["sha256"] for item in sources):
        raise ValueError("検証中にソースが変わりました")
    commit = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT).decode().strip()
    expected = {
        "sample": SAMPLE, "sha256": SAMPLE_SHA, "vertices": VERTICES,
        "vertexCount": 4, "faceCountInFile": 2,
        "bounds": {"min": [1, 1, 0], "max": [2, 2, 2]},
        "coreDiagnostics": [
            {"code": "ply.element.dropped", "element": "face", "found": 2},
            {"code": "ply.crs.missing"},
        ],
        "limits": "コアは頂点を点群へ移す。2 面は欠落を診断し、面の描画を確認済みとしない。",
    }
    manifest = {
        "schemaVersion": 1, "issue": "https://github.com/zumen-jp/zumen/issues/870",
        "coreCommit": commit, "sources": sources,
        "input": {**entry, "outputPath": "input/pdal-mesh-fixed.ply"},
        "license": {"path": "licenses/pdal-2017.txt", "sha256": sha(license_bytes)},
        "external": {"implementation": "ply-rs", "version": external["version"],
                     "status": "passed", "tests": passed, "command": command},
        "application": {"name": "MeshLab", "status": "not_run", "reason": "待ち: 利用者"},
    }
    report = {
        "schemaVersion": 1, "inputSha256": SAMPLE_SHA, "status": "not_run",
        "application": "MeshLab", "version": None, "date": None,
        "opened": None, "vertexCount": None, "faceCount": None,
        "vertices": None, "bounds": None, "differences": [],
        "note": "利用者が許可した環境で実測して記入する。期待値の転記で合格にしない。個人名・絶対パスを記録しない。",
    }
    readme = """# PLY の頂点を確かめる

公開 PDAL サンプルを変更せず同梱しています。出典と利用条件は manifest.json、全文は licenses/pdal-2017.txt にあります。

外部実装との自動照合は ply-rs 0.1.3 による 4 本です。実行結果は manifest.json に記録しています。MeshLab は起動していません。

利用者が許可した環境で input/pdal-mesh-fixed.ply を開き、expected.json の 4 頂点の座標、点数、2 面、範囲を確認して application-report.json に実測値を記入します。座標を取り出せなければ、その欄を未確認のまま残します。開けただけでは合格にしません。

コアの読みは点群で、面 2 件を移さない診断と CRS がない診断を返します。MeshLab の面表示をコアの面対応と数えません。製品の画面やログ全文、利用者名や絶対パスを公開記録に含めません。
"""
    output.mkdir(parents=True, exist_ok=True)
    files = {
        "input/pdal-mesh-fixed.ply": data, "licenses/pdal-2017.txt": license_bytes,
        "expected.json": encoded(expected), "manifest.json": encoded(manifest),
        "application-report.json": encoded(report), "README.md": readme.encode("utf8"),
    }
    for name, contents in files.items():
        target = output / name
        target.parent.mkdir(parents=True, exist_ok=True)
        with target.open("xb") as stream:
            stream.write(contents)
    print(json.dumps({"external": "passed", "tests": len(passed), "application": "not_run", "sampleSha256": SAMPLE_SHA}))


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        print(json.dumps({"status": "failed", "errorType": type(error).__name__}))
        sys.exit(1)
