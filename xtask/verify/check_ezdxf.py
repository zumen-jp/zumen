"""確認の束の DXF を ezdxf で読み、zumen が読んだ値と照らす（#503、決定 0253）。

使い方: python check_ezdxf.py <束のフォルダー>

束は make.py が作る。DXF のファイルごとに、次を確かめる。

- ezdxf が読める（ezdxf.recover）。ezdxf の監査（Drawing.audit）で、書き戻したファイルに入力にない誤りが出ず、
  変換したファイルに誤りが出ない。
- zumen が読んだ画層の名前が、ezdxf の画層の表と同じ。
- zumen の要素の出所のハンドルの図形が ezdxf にあり、種類・画層と、形の値（端点・中心・半径・角度・位置・
  文字・尺度・格子など）が同じ。
- モデル空間の図形は、どれも zumen の要素になるか、zumen がそのレコードを指す診断を返している（黙って捨てない）。
- 書き戻したファイルでは、変更で決めた形（manifest.json の expect）が、ezdxf で読んだ図形と同じ。
"""

import json
import math
import sys
from collections import Counter
from pathlib import Path

import ezdxf
from ezdxf import recover

# 図面モデルの形 → DXF の図形の種類
TYPES = {
    "line": {"LINE"},
    "circle": {"CIRCLE"},
    "arc": {"ARC"},
    "ellipse": {"ELLIPSE"},
    "polyline": {"LWPOLYLINE", "POLYLINE"},
    "spline": {"SPLINE"},
    "point": {"POINT"},
    "text": {"TEXT"},
    "formatted_text": {"MTEXT"},
    "symbol": {"INSERT"},
    "dimension": {"DIMENSION", "ARC_DIMENSION"},
    "leader": {"LEADER"},
    "callout": {"MULTILEADER", "MLEADER"},
}
# PDMODE の形（決定 0183）
MARKS = {"dot": 0, "none": 1, "plus": 2, "cross": 3, "tick": 4}
# MTEXT の 71 → 寄せ方
ATTACHMENTS = [
    "top_left",
    "top_center",
    "top_right",
    "middle_left",
    "middle_center",
    "middle_right",
    "bottom_left",
    "bottom_center",
    "bottom_right",
]
# zumen の中の ID で、ezdxf と比べない欄
SKIP = {"style_id"}
# 既知の食い違い: (ファイルの道筋の初め, 食い違いの文の初め, issue)。直ったら消す。出なくなった項目は失敗にする
KNOWN = [
    ("convert/", "ezdxf が読めない: AttributeError", 509),
]


def close(a, b, tolerance=1e-9):
    return math.isclose(a, b, rel_tol=tolerance, abs_tol=tolerance)


def close_angle(a, b):
    d = (a - b) % 360.0
    return min(d, 360.0 - d) <= 1e-9 * max(1.0, abs(a), abs(b))


def xy(v):
    return [float(v[0]), float(v[1])]


def geometry(e):
    """ezdxf の図形を、図面モデルの形の値（entities の geometry）の形にする。比べない種類は None"""
    t = e.dxftype()
    d = e.dxf
    if t == "LINE":
        return {"shape": "line", "start": xy(d.start), "end": xy(d.end)}
    if t == "CIRCLE":
        return {"shape": "circle", "center": xy(d.center), "radius": d.radius}
    if t == "ARC":
        return {"shape": "arc", "center": xy(d.center), "radius": d.radius, "start_angle": d.start_angle, "end_angle": d.end_angle}
    if t == "ELLIPSE":
        # 11 は中心からの向き。図面モデルの major は長軸の端の点
        return {
            "shape": "ellipse",
            "center": xy(d.center),
            "major": xy(d.center + d.major_axis),
            "ratio": d.ratio,
            "start_angle": math.degrees(d.start_param),
            "end_angle": math.degrees(d.end_param),
        }
    if t == "LWPOLYLINE":
        points = list(e.get_points("xyb"))
        g = {"shape": "polyline", "points": [[p[0], p[1]] for p in points], "closed": e.closed}
        if any(p[2] for p in points):
            g["bulges"] = [p[2] for p in points]
        return g
    if t == "POINT":
        return {"shape": "point", "position": xy(d.location), "rotation": d.get("angle", 0.0)}
    if t == "TEXT":
        g = {"shape": "text", "content": d.text, "height": d.height, "rotation": d.get("rotation", 0.0), "style": d.get("style", "Standard")}
        h, v = d.get("halign", 0), d.get("valign", 0)
        if h in (3, 5):
            # 2 点の間に置く文字の向きは、2 点が決める（50 は使わない）
            g["position"], g["end"] = xy(d.insert), xy(d.align_point)
            g["fit"] = "aligned" if h == 3 else "fit"
            g["rotation"] = math.degrees(math.atan2(g["end"][1] - g["position"][1], g["end"][0] - g["position"][0]))
        elif h == 0 and v == 0:
            g["position"] = xy(d.insert)
        else:
            g["position"] = xy(d.align_point)
        return g
    if t == "MTEXT":
        return {
            "shape": "formatted_text",
            "content": e.text,
            "position": xy(d.insert),
            "height": d.char_height,
            "rotation": e.get_rotation(),
            "attachment": ATTACHMENTS[d.get("attachment_point", 1) - 1],
            "style": d.get("style", "Standard"),
        }
    if t == "INSERT":
        g = {
            "shape": "symbol",
            "name": d.name,
            "position": xy(d.insert),
            "scale": [d.get("xscale", 1.0), d.get("yscale", 1.0)],
            "rotation": d.get("rotation", 0.0),
        }
        if d.get("column_count", 1) > 1 or d.get("row_count", 1) > 1:
            g["grid"] = {
                "columns": d.get("column_count", 1),
                "rows": d.get("row_count", 1),
                "column_spacing": d.get("column_spacing", 0.0),
                "row_spacing": d.get("row_spacing", 0.0),
            }
        if e.attribs:
            g["attributes"] = [{"tag": a.dxf.tag, "value": a.dxf.text} for a in e.attribs]
        return g
    return None


def differences(want, got, path=""):
    """want の欄が got と同じでない所。want にない欄は比べない"""
    out = []
    if isinstance(want, dict):
        if not isinstance(got, dict):
            return [f"{path}: {got!r}（期待は {want!r}）"]
        for key, value in want.items():
            if key == "shape" or key in SKIP:
                continue
            if key not in got:
                out.append(f"{path}/{key}: ない（期待は {value!r}）")
                continue
            if key in ("rotation", "start_angle", "end_angle") and isinstance(value, (int, float)):
                if not close_angle(float(value), float(got[key])):
                    out.append(f"{path}/{key}: {got[key]!r}（期待は {value!r}）")
                continue
            # スタイルの名前は、大文字と小文字を区別しない（AutoCAD と同じ）
            if key == "style" and isinstance(value, str) and isinstance(got[key], str):
                if value.lower() != got[key].lower():
                    out.append(f"{path}/{key}: {got[key]!r}（期待は {value!r}）")
                continue
            out += differences(value, got[key], f"{path}/{key}")
    elif isinstance(want, list):
        if not isinstance(got, list) or len(want) != len(got):
            return [f"{path}: {got!r}（期待は {want!r}）"]
        for i, (a, b) in enumerate(zip(want, got)):
            out += differences(a, b, f"{path}/{i}")
    elif isinstance(want, bool) or isinstance(want, str):
        if want != got:
            out.append(f"{path}: {got!r}（期待は {want!r}）")
    elif isinstance(want, (int, float)):
        if not isinstance(got, (int, float)) or not close(float(want), float(got)):
            out.append(f"{path}: {got!r}（期待は {want!r}）")
    return out


def scan(path):
    """ハンドル → レコードの番号（zumen の DXF のレコードの区切り。グループ・コード 0 のタグから次の 0 の前まで）と、
    ファイルの LAYER のレコードの名前。ezdxf は、読むときに Defpoints の画層を足すことがあるので、ファイルの名前と分ける"""
    data = Path(path).read_bytes()
    if data.startswith(b"AutoCAD Binary DXF"):
        return {}, None
    lines = data.splitlines()
    numbers, layers, record, kind, named = {}, set(), -1, None, False
    for i in range(0, len(lines) - 1, 2):
        code, value = lines[i].strip(), lines[i + 1].strip()
        if code == b"0":
            record += 1
            kind, named = value, False
        elif code == b"5" or (code == b"105" and kind == b"DIMSTYLE"):
            numbers.setdefault(value.decode("ascii", "replace").upper(), max(record, 0))
        elif code == b"2" and kind == b"LAYER" and not named:
            layers.add(value.decode("utf-8", "replace").lower())
            named = True
    return numbers, layers


def handle_of(element):
    handle = element.get("source", {}).get("handle")
    return handle.upper() if handle else None


def check_file(folder, entry, reading, baseline, counts):
    """1 つの DXF のファイルの食い違い。比べた数を counts に足す"""
    path = folder / entry["path"]
    try:
        doc, recovered = recover.readfile(str(path))
    except Exception as error:  # noqa: BLE001 ezdxf が読めないことそのものを結果にする
        return [f"ezdxf が読めない: {type(error).__name__}: {error}"], None
    failures = []
    audit = doc.audit()
    errors = sorted({f"{e.code}" for e in audit.errors} | {f"recover {e.code}" for e in recovered.errors})
    if entry["role"] == "convert" and errors:
        failures.append(f"ezdxf の監査の誤り: {errors} {[e.message for e in audit.errors][:3]}")
    if entry["role"] == "apply" and baseline is not None:
        added = sorted(set(errors) - set(baseline))
        if added:
            failures.append(f"ezdxf の監査で、入力にない誤りが出た: {added} {[e.message for e in audit.errors][:3]}")
    if reading is None:
        return failures, errors

    # 画層の名前
    numbers, raw = scan(path)
    layers = {l.dxf.name for l in doc.layers if raw is None or l.dxf.name.lower() in raw}
    defined = {l["name"] for l in reading["tables"]["layers"] if l.get("defined")}
    if layers != defined:
        failures.append(f"画層の名前が違う: zumen だけ {sorted(defined - layers)}、ezdxf だけ {sorted(layers - defined)}")

    # 要素の出所の図形
    db = doc.entitydb
    seen = set()
    for element in reading["entities"]:
        handle = handle_of(element)
        if handle is None:
            continue
        seen.add(handle)
        e = db.get(handle)
        where = element["id"]
        if e is None:
            failures.append(f"{where}: ezdxf にハンドル {handle} の図形がない")
            continue
        shape = element.get("geometry", {}).get("shape")
        if shape in TYPES and e.dxftype() not in TYPES[shape]:
            failures.append(f"{where}: 形 {shape} の出所が {e.dxftype()}")
            continue
        if element.get("layer") is not None and e.dxf.hasattr("layer") and element["layer"].lower() != e.dxf.layer.lower():
            failures.append(f"{where}: 画層が {element['layer']!r}、ezdxf は {e.dxf.layer!r}")
        # 用紙の要素は mm に直しているので、形はモデル空間だけで比べる
        counts["elements"] += 1
        if where.startswith("space/0/") and shape in TYPES and "/" not in where[len("space/0/") :]:
            got = geometry(e)
            if got is not None:
                counts["shapes"] += 1
                for d in differences(element["geometry"], got):
                    failures.append(f"{where}（{e.dxftype()} {handle}）{d}")

    # 黙って捨てない: モデル空間の図形は、要素になるか、診断がそのレコードを指す
    told = {p.get("record") for d in reading["diagnostics"] + entry.get("open", []) for p in d.get("at", [])}
    dropped = Counter()
    for e in doc.modelspace():
        handle = e.dxf.handle.upper()
        if handle in seen:
            continue
        if numbers.get(handle) in told:
            continue
        dropped[e.dxftype()] += 1
    if dropped:
        failures.append(f"要素にならず、診断もないモデル空間の図形: {dict(dropped)}")

    # 書き戻した値
    if entry["role"] == "apply":
        by_id = {e["id"]: e for e in reading["entities"]}
        for step in entry["steps"]:
            for want in step.get("expect", []):
                element = by_id.get(want["id"])
                if element is None:
                    failures.append(f"変更 {want['edit']} の要素 {want['id']} を、書き戻したファイルで zumen が読まない")
                    continue
                e = db.get(handle_of(element) or "")
                got = geometry(e) if e is not None else None
                if got is None:
                    failures.append(f"変更 {want['edit']} の要素 {want['id']} を ezdxf で比べられない")
                    continue
                counts["expected"] += 1
                for d in differences(want["geometry"], got):
                    failures.append(f"変更 {want['edit']}（{want['id']}）{d}")
            for edit in step["edits"]:
                if edit["kind"] == "point_style":
                    mode = MARKS[edit["mark"]] + (32 if edit.get("circle") else 0) + (64 if edit.get("square") else 0)
                    size = edit["size"] if edit["size_kind"] == "length" else -edit["size"]
                    if doc.header.get("$PDMODE") != mode or not close(doc.header.get("$PDSIZE", 0.0), size):
                        failures.append(f"点の表し方が $PDMODE {doc.header.get('$PDMODE')}・$PDSIZE {doc.header.get('$PDSIZE')}（期待は {mode}・{size}）")
    return failures, errors


def main():
    if len(sys.argv) != 2:
        print("使い方: python check_ezdxf.py <束のフォルダー>", file=sys.stderr)
        return 2
    folder = Path(sys.argv[1])
    manifest = json.loads((folder / "manifest.json").read_text(encoding="utf-8"))
    baselines = {}
    counts = Counter()
    report, failed, known, matched = [], 0, 0, set()
    for entry in manifest["files"]:
        # probe は apply の応答だけを残し、ファイルを持たない
        if entry["format"] != "dxf" or entry["role"] == "probe" or entry.get("status") not in (None, "ok"):
            continue
        reading_path = folder / "zumen" / (entry["path"] + ".json")
        reading = json.loads(reading_path.read_text(encoding="utf-8")) if reading_path.exists() else None
        failures, errors = check_file(folder, entry, reading, baselines.get(entry.get("from")), counts)
        if entry["role"] == "input":
            baselines[entry["path"]] = errors or []
        issues = []
        for f in failures:
            hit = next((k for k in KNOWN if entry["path"].startswith(k[0]) and f.startswith(k[1])), None)
            if hit:
                matched.add(hit)
                issues.append(hit[2])
        new = [f for f in failures if not any(entry["path"].startswith(k[0]) and f.startswith(k[1]) for k in KNOWN)]
        report.append({"path": entry["path"], "role": entry["role"], "audit": errors, "failures": failures, "known": sorted(set(issues))})
        if new:
            failed += 1
            print(f"{entry['path']}:", file=sys.stderr)
            for f in new:
                print(f"  {f}", file=sys.stderr)
        elif failures:
            known += 1
            print(f"{entry['path']}: 既知の食い違い（{', '.join(f'#{n}' for n in sorted(set(issues)))}）", file=sys.stderr)
    for k in KNOWN:
        if k not in matched:
            failed += 1
            print(f"既知の食い違い「{k[1]}」（#{k[2]}）が出なくなった。直ったなら KNOWN から消す", file=sys.stderr)
    (folder / "ezdxf.json").write_text(json.dumps({"ezdxf": ezdxf.__version__, "counts": counts, "files": report}, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
    print(
        f"ezdxf {ezdxf.__version__} で {len(report)} の DXF を照らした。ハンドルで結んだ要素 {counts['elements']}・形を比べた要素 "
        f"{counts['shapes']}・変更で決めた形 {counts['expected']}。食い違い {failed}、既知の食い違い {known}"
    )
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
