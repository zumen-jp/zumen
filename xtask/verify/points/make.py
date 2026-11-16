"""#342 の合成入力4件を編集し、外部照合と実機用の期待値をまとめる。"""

import argparse
from collections import Counter
from datetime import datetime, timezone
import hashlib
import importlib.util
from importlib.metadata import version
import json
from pathlib import Path
import platform
import shutil
import subprocess
import sys

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
sys.path.insert(0, str(HERE.parent))
import check_ezjww
import check_ezsxf

spec = importlib.util.spec_from_file_location("verify_make", HERE.parent / "make.py")
shared = importlib.util.module_from_spec(spec)
spec.loader.exec_module(shared)

fixture_spec = importlib.util.spec_from_file_location("points_fixtures", HERE / "fixtures.py")
fixtures = importlib.util.module_from_spec(fixture_spec)
fixture_spec.loader.exec_module(fixtures)

CASES = [
    ("made-points.jww", None, None),
    ("conformance-open_sxf-a.sfc", "open_sxf.json", "a.sfc"),
    ("conformance-open_sxf-c.p21", "open_sxf.json", "c.p21"),
    ("made-features.sfc", None, None),
]
PINS = {"ezjww": "0.7.6", "ezsxf": "0.3.7"}


def need(condition, message):
    if not condition:
        raise ValueError(message)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def write_json(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2, allow_nan=False) + "\n", encoding="utf8", newline="\n")


def output_folder(path):
    path = path.resolve()
    need(not any((p / ".git").exists() for p in [path, *path.parents]), "出力先は Git の作業ツリーの外にしてください")
    need(not path.exists() or path.is_dir() and not any(path.iterdir()), "空でない出力先は上書きしません")
    return path


def source(name, case, member):
    if case is None:
        if name == "made-points.jww":
            return fixtures.jww(), "xtask/verify/points/fixtures.py"
        need(name == "made-features.sfc", "合成入力の生成器がありません")
        return shared.SFC.encode("ascii"), "xtask/verify/make.py"
    relative = f"bindings/conformance/cases/{case}"
    steps = json.loads((ROOT / relative).read_text(encoding="utf8"))["steps"]
    push = next(s["push"] for s in steps if s.get("push", {}).get("name") == member)
    data = bytes.fromhex(push["hex"]) if "hex" in push else push["text"].encode("utf8")
    return data, relative


def complete_edit(entry):
    """候補なしや空の期待値を、通過した編集として扱わない。"""
    need(entry.get("status") == "ok" and not entry.get("reread"), "編集失敗か、再読で診断が増えました")
    need(len(entry["steps"]) == 1, "このキットは1段の点追加だけを扱います")
    step = entry["steps"][0]
    expected = 5 if entry["format"] == "jww" else 2
    need(not step.get("skipped") and step.get("status") == "ok", "編集を実行していません")
    need(len(step.get("edits", [])) == expected and len(step.get("expect", [])) == expected, "期待値の件数が一致しません")
    need(all(e["kind"] == "add" for e in step["edits"]), "点追加用キットに別の操作が入りました")
    ids = [e.get("id") for e in step["expect"]]
    need(all(ids) and len(set(ids)) == expected, "期待値のIDが空か重複しています")
    need(sum(e["geometry"]["shape"] == "point" for e in step["expect"]) == 2, "点の期待値は2件です")
    return expected


def preserved_reading(before, after, added):
    """非対象のID・種類・値・ハッシュ・出所を、CLIの読みの範囲でも照らす。"""
    old = {e["id"]: e for e in before["entities"]}
    new = {e["id"]: e for e in after["entities"]}
    need(len(old) == len(before["entities"]) and len(new) == len(after["entities"]), "要素IDが重複しています")
    need(set(new) == set(old) | added and not set(old) & added, "非対象要素の欠落か、意図しない追加があります")
    need(all(new[key] == value for key, value in old.items()), "非対象要素の値・ID・ハッシュ・出所が変わりました")
    need(before["tables"] == after["tables"], "点追加で表が変わりました")
    return len(old)


def jww_display(header):
    """外部実装の読みで、黒地に黒や幅 0 の実機用入力を渡さない。"""
    colors, pens = header["palette"]["pen_colors"], header["pens"]
    need(len(colors) == 10 and colors[0] == 0x00FFFFFF and colors[1] == colors[2] == 0,
         "実機用 JWW は白背景と黒のペン 1・2 にしてください")
    widths = pens["screen_widths"]
    need(len(widths) == 10 and all(1 <= width <= 16 for width in widths), "JWW の画面線幅が範囲外です")
    for name in ("printer", "extended_printer"):
        need(len(pens[name]) == (10 if name == "printer" else 257), "JWW の印刷ペンが欠けています")
        need(all(1 <= pen["width"] <= 500 and 0.1 <= pen["point_radius"] <= 10 for pen in pens[name]),
             "JWW の印刷線幅か実点半径が範囲外です")
    need(pens["printer_dpi"] == 300, "JWW の線幅の基準は 300 dpi にしてください")
    return {"background": colors[0], "pointPen": colors[2], "screenWidths": widths,
            "printerDpi": pens["printer_dpi"], "pointRadiusMm": pens["printer"][2]["point_radius"]}


def compare_case(folder, before_entry, entry):
    expected = complete_edit(entry)
    readings = [json.loads((folder / "zumen" / (e["path"] + ".json")).read_text(encoding="utf8")) for e in (before_entry, entry)]
    kept = preserved_reading(*readings, {e["id"] for e in entry["steps"][0]["expect"]})
    checker = check_ezjww if entry["format"] == "jww" else check_ezsxf
    counts, audits, failures = Counter(), [], []
    for item, reading in zip((before_entry, entry), readings):
        wrong, audit = checker.check_file(folder, item, reading, audits[0] if audits else None, counts)
        failures += wrong
        audits.append(audit)
    need(not failures, "外部実装との不一致: " + " / ".join(failures))
    # 古い適合試料のSFCはezsxfが型で読まない。同じ点の値はmade-featuresで別に照らす。
    untyped = before_entry["path"] == "input/conformance-open_sxf-a.sfc"
    need(counts["expected"] == (0 if untyped else expected), "外部実装が比較した編集の件数が期待と異なります")
    external_kept, display = None, None
    if entry["format"] == "jww":
        before, after = [check_ezjww.ezjww.read_document(str(folder / e["path"])) for e in (before_entry, entry)]
        display = jww_display(before["header"])
        need(before["header"] == after["header"], "JWW のヘッダーが変わりました")
        header = fixtures.jww_header()
        need(all((folder / e["path"]).read_bytes().startswith(header) for e in (before_entry, entry)),
             "実機用 JWW の表示設定を含むヘッダーが変わりました")
        n = len(before["entities"])
        need(after["entities"][:n] == before["entities"] and len(after["entities"]) == n + expected, "ezjwwの非対象要素が変わりました")
        external_kept = n
        by_id = {e["id"]: e for e in readings[1]["entities"]}
        for want in entry["steps"][0]["expect"]:
            if want["geometry"]["shape"] == "point":
                entity = after["entities"][by_id[want["id"]]["source"]["record"] - 1]
                need(entity["base"]["pen_style"] == (100 if "marker" in want["geometry"] else 1), "JWWの点の線種が期待と異なります")
    partial = untyped or entry["format"] == "p21"
    return {
        "path": entry["path"],
        "input": before_entry["path"],
        "format": entry["format"],
        "inputSha256": sha((folder / before_entry["path"]).read_bytes()),
        "outputSha256": sha((folder / entry["path"]).read_bytes()),
        "readingSha256": sha((folder / "zumen" / (entry["path"] + ".json")).read_bytes()),
        "counts": dict(counts),
        "preservedCoreElements": kept,
        "preservedExternalEntities": external_kept,
        **({"display": display, "headerPreserved": True, "headerSha256": sha(header)} if display is not None else {}),
        "audits": audits,
        "expected": entry["steps"][0]["expect"],
        "status": "partial" if partial else "verified",
        "verified": not partial,
        "applicationStatus": "unverified",
        "unverified": ["実機の見え方・印刷", "外部実装による全属性の保持"]
        + (["このSFCの点の位置・角度・種類・倍率・色・レイヤ（ezsxfが型で読まない）"] if untyped else [])
        + (["P21のマーカー種類・色"] if entry["format"] == "p21" else [])
        + (["Jw_cadで打った普通の点の既定線種との一致"] if entry["format"] == "jww" else []),
    }


def readme(folder, cases):
    lines = [
        "# 書き戻した点の確認キット（#342）", "",
        "自作の合成入力4件に点を足しました。JWWには線・円・円弧も足しています。自動検査の結果は `results.json`、要求と期待値は `manifest.json`、入力と出力のCLIの読みは `zumen/` にあります。", "",
        "固定版ezjww/ezsxfで確認した範囲と実機の確認を分けます。古い適合試料のSFCはezsxfが型で読まないため、種類だけを照らしたpartialです。同じ点の値はmade-featuresのSFCで照らします。P21のマーカー種類・色も外部比較器の範囲外でpartialです。非対象のID・ハッシュ・出所・値と表はCLIの読みで保持を確認します。JWWはezjwwが返す非対象要素もすべて保持しますが、外部実装によるファイル全体の全属性の保持は確認済みとしません。", "",
        "JWW は公開仕様から作った専用入力です。白背景・黒のペン 1/2・画面線幅 1・印刷線幅 1（300 dpi）・実点半径 0.2 mm・保存倍率 0.5 を持ちます。元の適合用入力の表示設定が 0 だった問題を、コアの書き戻しとは分けて直しています。以前のキットの実測やハッシュは引き継がず、この出力を改めて測ります。", "",
        "## 操作する前に", "",
        "アプリの操作は、記録済みの対象と範囲の許可に従います。この生成器はアプリを起動しません。保存し直した図面はリポジトリ外に置きます。", "",
        "1. `results.json` の出力SHA-256を確認し、対応する `342/` のファイルを開く。",
        "2. JWWは実寸mm・用紙の中心原点で測る。普通の点は線種1、記号点は線種100・番号3・角度30度・倍率2。利用者が打った普通の点の線種とも比べる。",
        "3. SFC/P21は用紙mmで測る。点は(50,60)・角度30度・種類1、(80,60)・角度0度・種類3。倍率1、赤（既定義色2）。SXFビューアー、または許可されたJw_cadのSXF読み込みで比べる。",
        "4. 以下のID・レイヤ・値と照らし、見え方・非対象の形も確認する。未実行や比較できない値は合格にしない。`report.json` にソフトの版と実測を記録する。", "",
        "## ファイルと期待値", "",
        "| ファイル | 出力SHA-256 | 期待値 |", "| --- | --- | --- |",
    ]
    manifest = json.loads((folder / "manifest.json").read_text(encoding="utf8"))
    by_path = {f["path"]: f for f in manifest["files"]}
    for case in cases:
        lines.append(f"| `{case['path']}` | `{case['outputSha256']}` | {shared.expectation(folder, by_path[case['path']])} |")
    lines += ["", "## 出所とライセンス", "", "入力は `xtask/verify/points/fixtures.py`・`bindings/conformance/cases/open_sxf.json`・`xtask/verify/make.py` の自作試料です。JWW は[公式の公開データ形式](https://www.jwcad.net/jwdatafmt.txt)を参照して組み立てています。MIT OR Apache-2.0。zumen-jp。本文は `licenses/` にあります。製品の付属図面・コード・資源は含みません。", ""]
    (folder / "README.md").write_text("\n".join(lines), encoding="utf8", newline="\n")


def build(exe, folder):
    versions = {name: version(name) for name in PINS}
    need(versions == PINS, "比較する外部実装の版を固定してください")
    folder = output_folder(folder)
    folder.mkdir(parents=True, exist_ok=True)
    z = shared.Zumen(exe)
    files, cases = [], []
    try:
        status, capabilities = z.call({"op": "capabilities"})
        need(status == "ok", "CLIの能力を取得できません")
        for name, case, member in CASES:
            data, origin = source(name, case, member)
            path = "input/" + name
            target = folder / path
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(data)
            entry = {"path": path, "role": "input", "format": target.suffix[1:], "source": origin,
                     "license": shared.REPOSITORY[0], "attribution": shared.REPOSITORY[1], "open": shared.open_file(z, path, data)}
            reading = shared.read(z, path)
            shared.save_reading(folder, path, reading)
            baseline = shared.codes(entry["open"], reading["diagnostics"])
            output = shared.run_recipe(z, folder, entry, data, ("points", 342, None, shared.points_steps), baseline)
            files += [entry, output]
            cases.append(compare_case(folder, entry, output))
    finally:
        z.close()
    write_json(folder / "manifest.json", {"version": 1, "local": False, "capabilities": capabilities["result"], "files": files})
    sources = ["xtask/verify/points/make.py", "xtask/verify/points/fixtures.py", "xtask/verify/make.py", "xtask/verify/compare.py", "xtask/verify/check_ezjww.py", "xtask/verify/check_ezsxf.py", "bindings/conformance/cases/open_sxf.json"]
    result = {
        "version": 1, "issue": 342,
        "coreCommit": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
        "sourceState": "working-tree" if subprocess.check_output(["git", "status", "--porcelain", "--untracked-files=normal"], cwd=ROOT) else "clean",
        "executedAt": datetime.now(timezone.utc).isoformat(),
        "cliSha256": sha(Path(exe).read_bytes()),
        "python": platform.python_version(), "external": versions,
        "sources": {path: sha((ROOT / path).read_bytes()) for path in sources},
        "manifestSha256": sha((folder / "manifest.json").read_bytes()),
        "cases": cases, "complete": True,
        "applicationExecuted": False, "countedInApplicationNumerator": False,
    }
    write_json(folder / "results.json", result)
    write_json(folder / "report.json", {"issue": 342, "status": "unexecuted", "software": None, "version": None, "date": None,
               "cases": [{"path": c["path"], "outputSha256": c["outputSha256"], "status": "unexecuted", "observations": None} for c in cases]})
    (folder / "licenses").mkdir()
    for name in ("LICENSE-MIT", "LICENSE-APACHE"):
        shutil.copyfile(ROOT / name, folder / "licenses" / name)
    readme(folder, cases)
    print(f"点のキット: {len(cases)}組（部分照合{sum(c['status'] == 'partial' for c in cases)}組）、外部照合した変更{sum(c['counts'].get('expected', 0) for c in cases)}件、実機は未実行")
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("zumen", type=Path)
    parser.add_argument("out", type=Path)
    args = parser.parse_args()
    build(args.zumen, args.out)
    return 0


if __name__ == "__main__":
    sys.exit(main())
