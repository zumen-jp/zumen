"""確認の束の DXF を ezdxf で読み、zumen が読んだ値と照らす（#503、決定 0254）。

使い方: python check_ezdxf.py <束のフォルダー>

束は make.py が作る。DXF のファイルごとに、次を確かめる。

- ezdxf が読める（ezdxf.recover）。読む中で流れる ezdxf の監査で、書き戻したファイルの誤りと直した誤りが、code
  ごとに入力より増えず、変換したファイルには誤りも直した誤りもない。
- zumen が読んだ画層の名前が、ezdxf の画層の表と同じ。
- zumen の要素の出所のハンドルの図形が ezdxf にあり、種類・画層と、形の値（端点・中心・半径・角度・位置・
  文字・尺度・格子など）が同じ。形の欄は両向きに比べ、どちらかにだけある欄も食い違いにする。ezdxf で比べられる
  種類のモデル空間の図形は、zumen の要素が形を持つ。
- モデル空間の図形は、どれも zumen の要素になるか、移さなかったことを言う zumen の診断（compare.py の
  LIFTED_AWAY）が、その図形の種類とレコードを指す（黙って捨てない）。
- 書き戻したファイルでは、変更で決めた形と画層（manifest.json の expect）と、足した画層の色・変えた色・点の
  表し方（$PDMODE・$PDSIZE）が、ezdxf で読んだ値と同じ。
"""

import math
import sys
from collections import Counter
from pathlib import Path

import ezdxf
from ezdxf import recover

sys.path.insert(0, str(Path(__file__).resolve().parent))
from compare import ATTACHMENTS, check_handles, lifted_away_records, run, told, usage  # noqa: E402

# 既知の食い違い: (ファイルの道筋の初め, 食い違いの文の初め, issue, 当たるファイルの数)。直ったら消す。当たる数が
# 変わったら失敗にする（直ったものと、同じ形で新たに食い違うものを知らせる）
KNOWN = [
    ("convert/", "ezdxf が読めない: AttributeError: 'NoneType' object has no attribute 'get_xlist'", 509, 15),
    ("convert/", "ezdxf の監査で直した誤り: {'INVALID_TABLE_HANDLE': 5}", 509, 19),
]


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
        # DXF Reference の ELLIPSE: 11 は中心から長軸の端への向き、41・42 は媒介変数（ラジアン）。図面モデルの
        # major は長軸の端の点
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
        # DXF Reference の POINT: 50 は点を描いたときの X 軸の角度（度）
        return {"shape": "point", "position": xy(d.location), "rotation": d.get("angle", 0.0)}
    if t == "TEXT":
        g = {"shape": "text", "content": d.text, "height": d.height, "rotation": d.get("rotation", 0.0), "style": d.get("style", "Standard")}
        # DXF Reference の TEXT: 72 は横、73 は縦の寄せ方。72 が 3（aligned）・5（fit）なら 10 と 11 の 2 点の間に
        # 置き、ほかで 0・0 でなければ 11 に寄せる
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
        # DXF Reference の MTEXT: 71 は寄せ方、41 は枠の幅（0 なら枠なし）
        g = {
            "shape": "formatted_text",
            "content": e.text,
            "position": xy(d.insert),
            "height": d.char_height,
            "rotation": e.get_rotation(),
            "attachment": ATTACHMENTS[d.get("attachment_point", 1) - 1],
            "style": d.get("style", "Standard"),
        }
        if d.get("width", 0.0) > 0:
            g["width"] = d.width
        return g
    if t == "INSERT":
        # DXF Reference の INSERT: 41・42 は尺度、70・71 は列と行の数、44・45 は列と行の間隔（MINSERT）
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


def view(doc, raw):
    """共有の照らし方（compare.check_handles）に渡す ezdxf の読み"""

    def entity(handle):
        e = doc.entitydb.get(handle)
        if e is None:
            return None
        return {"type": e.dxftype(), "layer": e.dxf.get("layer"), "color": e.dxf.get("color", 256), "geometry": geometry(e)}

    return {
        "tool": "ezdxf",
        "layers": {l.dxf.name for l in doc.layers if raw is None or l.dxf.name.lower() in raw},
        "layer_colors": {l.dxf.name.lower(): abs(l.dxf.color) for l in doc.layers},
        "entity": entity,
        "header": (doc.header.get("$PDMODE"), doc.header.get("$PDSIZE", 0.0)),
    }


def check_file(folder, entry, reading, baseline, counts):
    """1 つの DXF のファイルの食い違い。比べた数を counts に足す"""
    path = folder / entry["path"]
    try:
        doc, auditor = recover.readfile(str(path))
    except Exception as error:  # noqa: BLE001 ezdxf が読めないことそのものを結果にする
        return [f"ezdxf が読めない: {type(error).__name__}: {error}"], None
    failures = []
    # recover.readfile は読む中で監査を流し、直せない誤り（errors）と直した誤り（fixes）を残す。直した後の文書に
    # もう一度監査を流しても、直した誤りは出ない
    audit = {
        kind: dict(sorted(Counter(getattr(e.code, "name", str(e.code)) for e in found).items()))
        for kind, found in (("errors", auditor.errors), ("fixes", auditor.fixes))
    }
    if entry["role"] == "convert":
        if audit["errors"]:
            failures.append(f"ezdxf の監査の誤り: {audit['errors']} {[e.message for e in auditor.errors][:3]}")
        if audit["fixes"]:
            failures.append(f"ezdxf の監査で直した誤り: {audit['fixes']}")
    if entry["role"] == "apply" and baseline is not None:
        added = {f"{kind} {code}": n for kind in audit for code, n in audit[kind].items() if n > baseline[kind].get(code, 0)}
        if added:
            failures.append(f"ezdxf の監査で、入力より誤りが増えた: {added} {[e.message for e in auditor.errors + auditor.fixes][:3]}")
    if reading is None:
        return failures, audit

    # 画層の名前・ハンドルで結んだ図形・書き戻した値
    numbers, raw = scan(path)
    found, seen = check_handles(entry, reading, counts, view(doc, raw))
    failures += found

    # 黙って捨てない: モデル空間の図形は、要素になるか、移さなかったことを言う診断が、その図形の種類とレコードを指す
    away = {(kind.upper() if kind else None, record) for kind, record in lifted_away_records(entry, reading)}
    dropped = Counter()
    for e in doc.modelspace():
        handle = e.dxf.handle.upper()
        if handle in seen:
            continue
        if told(away, e.dxftype().upper(), numbers.get(handle)):
            continue
        dropped[e.dxftype()] += 1
    if dropped:
        failures.append(f"要素にならず、移さなかったことを言う診断もないモデル空間の図形: {dict(dropped)}")
    return failures, audit


def main():
    folder = usage("check_ezdxf.py")
    if folder is None:
        return 2
    return run(folder, "ezdxf", ezdxf.__version__, {"dxf"}, check_file, KNOWN)


if __name__ == "__main__":
    sys.exit(main())
