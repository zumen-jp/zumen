"""自作の SAF 付き P2Z・SFZ を外部実装と照らし、実機用のキットを作る（#494）。"""

import argparse
from collections import Counter
import hashlib
import importlib.util
from importlib.metadata import version
import io
import json
from pathlib import Path
import subprocess
import sys
import xml.etree.ElementTree as ET
import zipfile

ROOT = Path(__file__).resolve().parents[3]
HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent))
import fixtures
import check_ezsxf

spec = importlib.util.spec_from_file_location("verify_make", HERE.parent / "make.py")
shared = importlib.util.module_from_spec(spec)
spec.loader.exec_module(shared)
CONTROL = b"zumen SAF verification: unchanged member\n"
MATRIX = [1, 0, 5, 0, 1, -3]


def need(condition, message):
    if not condition:
        raise ValueError(message)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def write_json(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2, allow_nan=False) + "\n", encoding="utf8", newline="\n")


def output_folder(path):
    path = path.resolve()
    need(not any((parent / ".git").exists() for parent in [path, *path.parents]), "出力先は Git の作業ツリーの外にしてください")
    need(not path.exists() or path.is_dir() and not any(path.iterdir()), "空でない出力先は上書きしません")
    return path


def packed(drawing_name, saf_name, compression):
    data = fixtures.sfc(saf_name) if drawing_name.endswith("SFC") else fixtures.p21(saf_name)
    out = io.BytesIO()
    with zipfile.ZipFile(out, "w") as archive:
        for name, content in [(drawing_name, data), (saf_name, fixtures.saf(drawing_name)), ("control.txt", CONTROL)]:
            info = zipfile.ZipInfo(name, (1980, 1, 1, 0, 0, 0))
            info.compress_type = compression
            info.external_attr = 0o600 << 16
            archive.writestr(info, content)
    return out.getvalue()


def members(data, drawing_name, saf_name):
    with zipfile.ZipFile(io.BytesIO(data)) as archive:
        need(archive.namelist() == [drawing_name, saf_name, "control.txt"], "ZIP の名前か順序が一致しません")
        need(archive.testzip() is None, "ZIP の CRC が一致しません")
        result = {name: archive.read(name) for name in archive.namelist()}
    need(result["control.txt"] == CONTROL, "非対象の ZIP エントリーが変わりました")
    need(f"$$ATRF$$10$${saf_name}".encode("ascii") in result[drawing_name], "SAF の図形番号かファイル名が一致しません")
    return result


def xml_fields(data, drawing_name):
    need(b"<!DOCTYPE" not in data.upper() and b"<!ENTITY" not in data.upper(), "このキットの XML は外部宣言を持ちません")
    root = ET.fromstring(data)
    need(root.tag == "SxfAttributeXML" and root.attrib == {
        "version": "3.1", "date": "2026-11-16", "sxfFile": drawing_name, "application": "zumen verification",
    }, "SAF の版か図面参照が一致しません")
    need([item.tag for item in root] == ["AttributeSet", "Figure"], "SAF の要素の数か順序が一致しません")
    group_set, figure = root
    need(group_set.text == "1" and group_set.attrib == {"name": "検証セット", "version": "1.0", "designedBy": "zumen"}, "属性セットが一致しません")
    need(figure.attrib == {"id": "10", "name": "検証線分"} and len(figure) == 1, "図形の参照が一致しません")
    ref = figure[0]
    need(ref.tag == "AttrSetRef" and ref.attrib == {"id": "1"} and len(ref) == 1, "属性セットの参照が一致しません")
    group = ref[0]
    need(group.tag == "AttrGroup" and group.attrib == {"name": "測点"} and len(group) == 2, "属性グループが一致しません")
    expected = [({"name": "測点名", "type": "STR"}, "No.1"),
                ({"name": "現地盤高", "type": "LEN", "unit": "m"}, "102.0400")]
    result = {}
    for attr, (attributes, text) in zip(group, expected, strict=True):
        need(attr.tag == "Attr" and attr.attrib == attributes and attr.text == text and len(attr) == 0, "属性の名前・型・単位・値が一致しません")
        fields = {"figure": "10", "figure_name": "検証線分", "set": "1", "set_name": "検証セット",
                  "set_version": "1.0", "designed_by": "zumen", "group": "測点", "type": attributes["type"], "value": text}
        if "unit" in attributes:
            fields["unit"] = attributes["unit"]
        result[attributes["name"]] = fields
    return result


def parse_probe(data):
    lines = data.decode("ascii").splitlines()
    need(lines[0] == "SAF\t1" and lines[-1] == "END\t2", "SAF の読取件数が一致しません")
    result, current = [], None
    decode = lambda value: bytes.fromhex(value).decode("utf8")
    for line in lines[1:-1]:
        fields = line.split("\t")
        if fields[0] == "A" and len(fields) == 5:
            current = {"target": decode(fields[1]), "name": decode(fields[2]), "version": decode(fields[3]),
                       "current": fields[4] == "true", "fields": {}}
            need(fields[4] in ("true", "false"), "SAF の状態が読めません")
            result.append(current)
        else:
            need(fields[0] == "F" and len(fields) == 3 and current is not None, "SAF のフィールドが読めません")
            key = decode(fields[1])
            need(key not in current["fields"], "SAF のフィールドが重複しています")
            current["fields"][key] = decode(fields[2])
    need(len(result) == 2, "SAF の属性数が一致しません")
    return result


def check_bindings(bindings, expected, target):
    need(len(bindings) == len(expected) and {item["name"] for item in bindings} == set(expected), "SAF の名前か件数が一致しません")
    for item in bindings:
        need(item["target"] == target and item["version"] == "3.1" and item["current"], "SAF の結び先か状態が一致しません")
        need(item["fields"] == expected[item["name"]], "SAF の外部 XML とコアの読みが一致しません")


def p21_sheet(step):
    # 自作入力の固定番号を起点に、編集後も同じ図面版・用紙枠・単位を参照することを照らす。
    usage = step.part(126, "DRAWING_SHEET_REVISION_USAGE")
    need(usage and len(usage) == 3, "用紙と図面版の対応がありません")
    sheet = step.get(usage[0], "DRAWING_SHEET_REVISION")
    revision = step.get(usage[1], "DRAUGHTING_DRAWING_REVISION")
    need(sheet and revision and step.get(revision[1], "DRAWING_DEFINITION"), "用紙か図面の定義がありません")
    title = step.part(127, "DRAUGHTING_TITLE")
    need(title and step.ref(usage[1]) in step.refs(title[0]), "タイトルが図面版を指していません")
    size = step.part(123, "PRESENTATION_SIZE")
    need(size and size[0] == usage[0] and step.ref(size[1]) in step.refs(sheet[1]), "用紙と寸法が同じ枠を参照していません")
    box = step.get(size[1], "PLANAR_BOX")
    need(box and box[1:3] == [420, 297] and step.get(box[3], "AXIS2_PLACEMENT_2D"), "用紙枠が A3 横の寸法か配置と一致しません")
    units = step.get(sheet[2], "GLOBAL_UNIT_ASSIGNED_CONTEXT")
    need(units and step.get(sheet[2], "GEOMETRIC_REPRESENTATION_CONTEXT") == [2], "用紙の単位か次元がありません")
    symbols = [step.get(unit, "SI_UNIT") for unit in units[0]]
    mm = [{"kind": "enum", "value": "MILLI"}, {"kind": "enum", "value": "METRE"}]
    rad = [{"kind": "unset"}, {"kind": "enum", "value": "RADIAN"}]
    need(len(symbols) == 2 and mm in symbols and rad in symbols, "用紙の単位が mm と rad に一致しません")
    return {"dimensionsMm": box[1:3], "drawingRevision": revision[0]}


def p21_circle(path, element, changed):
    # 既存の P21 照合器は円を種類だけ比べる。この自作入力の円は ezsxf の参照から値も照合する。
    step = check_ezsxf.Step(check_ezsxf.ezsxf.parse_p21(str(path), strict=False), {})
    instance = check_ezsxf.instance_of(element)
    need(instance == 27, "自作 P21 の円の出所が一致しません")
    _, item = step.styled(instance)
    circle = step.get(item, "CIRCLE")
    need(circle is not None and len(circle) == 3, "ezsxf が円を読めません")
    placement = step.get(circle[1], "AXIS2_PLACEMENT_2D")
    need(placement is not None, "円の配置が読めません")
    shape = {"shape": "circle", "center": step.point(placement[1]), "radius": float(circle[2])}
    expected = {"shape": "circle", "center": [55, 22] if changed else [50, 25], "radius": 10}
    need(shape == expected == element["geometry"], "P21 の円が外部の読みと期待値に一致しません")
    need(step.layers().get(instance) == element["layer"], "P21 の円の画層が一致しません")
    return {"instance": instance, "geometry": shape, "layer": element["layer"], "sheet": p21_sheet(step), "matched": True}


def call(z, request, records):
    status, response = z.call(request)
    records.append({"request": request, "status": status, "response": response})
    need(status == "ok" and response is not None, "公開操作が失敗しました: " + request["op"])
    need(not response["diagnostics"], "公開操作に診断があります: " + request["op"])
    return response


def reading(z, name, data, records):
    z.push(name, data)
    call(z, {"op": "open", "input": name}, records)
    result = shared.read(z, name)
    need(not result["diagnostics"] and len(result["entities"]) == 2, "図形の診断か数が一致しません")
    return result


def expected_geometry(result, changed):
    lines = [item for item in result["entities"] if item["geometry"]["shape"] == "line"]
    circles = [item for item in result["entities"] if item["geometry"]["shape"] == "circle"]
    need(len(lines) == len(circles) == 1, "線分と円が 1 つずつありません")
    need(lines[0]["geometry"] == {"shape": "line", "start": [0, 0], "end": [100, 0]}, "非対象の線分が一致しません")
    need(circles[0]["geometry"] == {"shape": "circle", "center": [55, 22] if changed else [50, 25], "radius": 10}, "円の形が一致しません")
    need(lines[0]["layer"] == circles[0]["layer"], "図形の画層が一致しません")
    return lines[0], circles[0]


def bindings(z, doc, probe, folder, label, expected, target, records):
    result = call(z, {"op": "lift", "doc": doc}, records)
    data = z.take(result["result"]["blob"])
    path = folder / f"{label}.zmdr"
    path.write_bytes(data)
    run = subprocess.run([str(probe), str(path)], capture_output=True, timeout=30)
    need(run.returncode == 0, "SAF の検証用読取が失敗しました")
    actual = parse_probe(run.stdout)
    check_bindings(actual, expected, target)
    write_json(folder / f"{label}-bindings.json", actual)
    return actual


def check_case(cli, probe, folder, kind, full_name, compression):
    drawing_name = "DRAWING.SFC" if kind == "sfz" else "DRAWING.P21"
    saf_name = drawing_name + ".SAF" if full_name else "DRAWING.SAF"
    original = packed(drawing_name, saf_name, compression)
    before_members = members(original, drawing_name, saf_name)
    expected = xml_fields(before_members[saf_name], drawing_name)
    folder.mkdir(parents=True)
    (folder / f"input.{kind}").write_bytes(original)
    records = []
    z = shared.Zumen(cli)
    try:
        source = f"input.{kind}"
        before = reading(z, source, original, records)
        line, circle = expected_geometry(before, False)
        before_bindings = bindings(z, source, probe, folder, "input", expected, line["id"], records)
        edit = {"kind": "transform", "id": circle["id"], "hash": circle["hash"], "matrix": MATRIX}
        applied = call(z, {"op": "apply", "doc": source, "edits": [edit]}, records)
        edited = z.take(next(item["id"] for item in applied["blobs"] if item["kind"] == "file"))
        need(edited != original, "変更がバイト列に反映されていません")
        after_members = members(edited, drawing_name, saf_name)
        need(all(after_members[name] == content for name, content in before_members.items() if name != drawing_name), "非対象の ZIP エントリーが変わりました")
        need(after_members[drawing_name] != before_members[drawing_name], "図面が変わっていません")
        xml_fields(after_members[saf_name], drawing_name)
        (folder / f"edited.{kind}").write_bytes(edited)
        after = reading(z, f"edited.{kind}", edited, records)
        new_line, new_circle = expected_geometry(after, True)
        need(new_line == line, "非対象の線分の値か ID が変わりました")
        need(after["tables"] == before["tables"] and new_circle["id"] == circle["id"], "表か ID が変わりました")
        after_bindings = bindings(z, f"edited.{kind}", probe, folder, "edited", expected, line["id"], records)
        need(before_bindings == after_bindings, "編集後に属性が変わりました")
        reading(z, f"repeat.{kind}", original, records)
        repeated = call(z, {"op": "apply", "doc": f"repeat.{kind}", "edits": [edit]}, records)
        need(z.take(next(item["id"] for item in repeated["blobs"] if item["kind"] == "file")) == edited, "同じ入力と変更でバイト列が一致しません")
    finally:
        z.close()
    external = []
    baseline = None
    for label, archive_members, core in [("input", before_members, before), ("edited", after_members, after)]:
        unpacked = folder / label
        unpacked.mkdir()
        for name, data in archive_members.items():
            (unpacked / name).write_bytes(data)
        counts = Counter()
        expect = [{"id": new_circle["id"], "edit": 0, "geometry": {"shape": "circle", "center": [55, 22], "radius": 10}}]
        entry = {"path": f"{label}/{drawing_name}", "format": "sfc" if kind == "sfz" else "p21",
                 "role": "input" if label == "input" else "apply", "steps": [{"expect": expect if kind == "sfz" else []}]}
        failures, audit = check_ezsxf.check_file(folder, entry, core, baseline, counts)
        need(not failures, "ezsxf との食い違い: " + "; ".join(failures))
        supplement = None
        if kind == "p2z":
            supplement = p21_circle(unpacked / drawing_name, circle if label == "input" else new_circle, label == "edited")
        need(not audit["notes"] and not audit["warnings"] and counts["elements"] == 2 and counts["shapes"] == (2 if kind == "sfz" else 1), "ezsxf が全図形を照合できません")
        need(label == "input" or counts["expected"] == (1 if kind == "sfz" else 0), "ezsxf が変更の期待値を照合できません")
        if label == "input":
            baseline = audit
        external.append({"role": label, "counts": dict(counts), "audit": audit, "circleSupplement": supplement, "failures": failures})
        write_json(folder / f"{label}-external.json", external[-1])
        write_json(folder / f"{label}-reading.json", core)
    write_json(folder / "requests.json", records)
    result = {"kind": kind, "drawing": drawing_name, "saf": saf_name,
              "compression": "stored" if compression == zipfile.ZIP_STORED else "deflate",
              "inputSha256": sha(original), "editedSha256": sha(edited), "external": external,
              "attributeCount": len(expected), "deterministic": True, "nonTargetPreserved": True,
              "application": "not_run", "verified": True}
    write_json(folder / "result.json", result)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", required=True, type=Path)
    parser.add_argument("--probe", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()
    output = output_folder(args.out)
    need(version("ezsxf") == "0.3.7", "ezsxf は 0.3.7 に固定してください")
    cli, probe = args.cli.resolve(strict=True), args.probe.resolve(strict=True)
    source_paths = ["Cargo.lock", "crates/zumen-sxf/Cargo.toml", "crates/zumen-sxf/examples/verify_saf.rs",
                    "xtask/verify/saf/make.py", "xtask/verify/saf/fixtures.py", "xtask/verify/saf/test_make.py",
                    "xtask/verify/saf/README.md", "xtask/verify/saf/kit-readme.md", "xtask/verify/check_ezsxf.py",
                    "xtask/verify/compare.py", "xtask/verify/make.py"]
    sources = [{"path": path, "sha256": sha((ROOT / path).read_bytes())} for path in source_paths]
    results = []
    for kind in ("sfz", "p2z"):
        for full_name in (False, True):
            for compression in (zipfile.ZIP_STORED, zipfile.ZIP_DEFLATED):
                name = f"{kind}-{'full' if full_name else 'stem'}-{compression}"
                results.append(check_case(cli, probe, output / name, kind, full_name, compression))
                print(json.dumps({"case": name, "verified": True}))
    need(all(sha((ROOT / item["path"]).read_bytes()) == item["sha256"] for item in sources), "検証中にソースが変わりました")
    manifest = {"schemaVersion": 1, "issue": 494, "sourceCommit": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT).decode().strip(),
                "sources": sources, "cliSha256": sha(cli.read_bytes()), "probeSha256": sha(probe.read_bytes()),
                "python": sys.version.split()[0], "ezsxf": version("ezsxf"), "cases": results,
                "inputProvenance": "全入力を fixtures.py で自作。製品資源と配布制限のある試料を含まない。",
                "limits": ["ネイティブ CLI の照合。wasm や別 OS との一致は未確認。", "XML の構造と値を照合。SAF DTD の適合性検証ではない。", "SXF ブラウザの操作・表示・印刷は未実行。"]}
    write_json(output / "report-template.json", {"issue": 494, "application": "", "version": "", "results": [
        {"case": f"{row['kind']}-{'full' if row['saf'].count('.') == 2 else 'stem'}-{0 if row['compression'] == 'stored' else 8}",
         "opened": None, "geometryMatched": None, "attributeValuesMatched": None, "attributeTargetMatched": None,
         "status": "not_run"} for row in results]})
    (output / "README.md").write_bytes((HERE / "kit-readme.md").read_bytes())
    write_json(output / "manifest.json", manifest)
    print(json.dumps({"cases": len(results), "verified": len(results), "application": "not_run"}))


if __name__ == "__main__":
    main()
