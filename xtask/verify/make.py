"""確認の束を作る（#503、決定 0254）。

zumen の CLI（`zumen serve --stdio`）で、公開サンプル（samples/）と適合テストの中で作った図面を開き、書き戻し
（apply）と変換（convert）を当てて、書いたファイルを束のフォルダーに置く。zumen が読んだ値（drawing・tables・
entities の応答）を、入力と書いたファイルのそれぞれについて置き、ファイルごとの出所・ライセンス・変更・期待する
値を manifest.json に書く。外部の実装（check_ezdxf.py など）と、AutoCAD・Jw_cad・SXF のビューアーで開く人が、
これを照らす。手順と期待する結果は README.md に書く。

使い方: python make.py <zumen の実行ファイル> <束のフォルダー> [--inputs フォルダー]

束のフォルダーがあれば、前の束（manifest.json がある）か空のときだけ消して作り直す。--inputs のフォルダーの
図面も入力にする（受け取った図面を使うとき）。その束は、リポジトリの外に作り、Git にも CI の成果物にも入れない。
"""

import argparse
import base64
import json
import math
import shutil
import subprocess
import sys
import tomllib
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
FIELDS = ["id", "kind", "layer", "source", "bbox", "hash", "geometry"]
# 診断ごとに返す場所の数。既定の 16 では、黙って捨てない照合で、場所のない図形を誤って数える
MAX_PLACES = 100000
# 適合テストの図面と束の台本のライセンス（このリポジトリと同じ）
REPOSITORY = ("MIT OR Apache-2.0", "zumen-jp（このリポジトリ）。本文は licenses/LICENSE-MIT と licenses/LICENSE-APACHE")
SUFFIXES = {".dxf": "dxf", ".dwg": "dwg", ".jww": "jww", ".sfc": "sfc", ".p21": "p21"}
# 適合テストの中で作った図面（cases の push）。Jw_cad・SXF の公開サンプルがまだないので、これを入力にする
CASES = [("open_jww.json", "a.jww"), ("open_sxf.json", "a.sfc"), ("open_sxf.json", "c.p21")]


class Zumen:
    """`zumen serve --stdio` の JSON-RPC（bindings/cli/README.md）"""

    def __init__(self, exe):
        self.child = subprocess.Popen([str(Path(exe).resolve()), "serve", "--stdio"], stdin=subprocess.PIPE, stdout=subprocess.PIPE)
        self.next = 0
        self.rpc("session_new", {})

    def rpc(self, method, params):
        self.next += 1
        line = json.dumps({"jsonrpc": "2.0", "id": self.next, "method": method, "params": params}) + "\n"
        self.child.stdin.write(line.encode())
        self.child.stdin.flush()
        message = json.loads(self.child.stdout.readline())
        if "error" in message:
            raise RuntimeError(f"{method}: {message['error']}")
        return message["result"]

    def push(self, name, data):
        status = self.rpc("push", {"name": name, "base64": base64.b64encode(data).decode()})["status"]
        if status != "ok":
            raise RuntimeError(f"push {name}: {status}")

    def call(self, request):
        result = self.rpc("call", {"request": {"v": 1, "max_places": MAX_PLACES, **request}})
        return result["status"], json.loads(result["response"]) if result.get("response") else None

    def take(self, blob):
        result = self.rpc("take", {"blob": blob})
        if result["status"] != "ok":
            raise RuntimeError(f"take {blob}: {result['status']}")
        return base64.b64decode(result["base64"])

    def close(self):
        self.child.stdin.close()
        self.child.wait()


def read(z, doc):
    """文書の図面モデルの要約・表・要素の一覧"""
    _, drawing = z.call({"op": "drawing", "doc": doc})
    _, tables = z.call({"op": "tables", "doc": doc})
    items, offset = [], 0
    while True:
        _, page = z.call({"op": "entities", "doc": doc, "fields": FIELDS, "offset": offset, "limit": 10000})
        items += page["result"]["items"]
        offset = page["result"].get("next_offset")
        if offset is None:
            break
    return {
        "drawing": drawing.get("result"),
        "diagnostics": drawing["diagnostics"],
        "tables": tables.get("result"),
        "entities": items,
    }


def open_file(z, name, data):
    z.push(name, data)
    status, response = z.call({"op": "open", "input": name})
    if status != "ok":
        raise RuntimeError(f"open {name}: {status} {response and response['diagnostics']}")
    return response["diagnostics"]


# 変更の作り方。どれも読んだ値（entities）から、その図面で当てられる変更を作る。期待する値は、zumen の読み手に
# 頼らず、変更そのものと変換の行列から決める。


def model(reading, kind=None, shape=None):
    """実寸の空間（space/0）の要素"""
    return [
        e
        for e in reading["entities"]
        if e["id"].startswith("space/0/")
        and (kind is None or e["kind"] == kind)
        and (shape is None or e.get("geometry", {}).get("shape") == shape)
    ]


def similar(degrees, scale, dx, dy):
    """回転・等方の尺度・平行移動の行列 [a, b, c, d, e, f]"""
    c, s = math.cos(math.radians(degrees)) * scale, math.sin(math.radians(degrees)) * scale
    return [c, -s, dx, s, c, dy]


def mapped(m, p):
    return [m[0] * p[0] + m[1] * p[1] + m[2], m[3] * p[0] + m[4] * p[1] + m[5]]


def transformed(geometry, m):
    """相似の変換（鏡に映さない）で写した形。回転と尺度は行列から読む。写し方を決めていない形は例外にする"""
    turn = math.degrees(math.atan2(m[3], m[0]))
    scale = math.hypot(m[0], m[3])
    g = dict(geometry)
    shape = g["shape"]
    if shape in ("point", "symbol", "text", "formatted_text"):
        g["position"] = mapped(m, g["position"])
        g["rotation"] = g.get("rotation", 0) + turn
    if shape in ("text", "formatted_text"):
        g["height"] = g["height"] * scale
        if "end" in g:
            g["end"] = mapped(m, g["end"])
        if "width" in g:
            g["width"] = g["width"] * scale
    if shape == "symbol":
        g["scale"] = [v * scale for v in g["scale"]]
        if "grid" in g:
            g["grid"] = {**g["grid"], "column_spacing": g["grid"]["column_spacing"] * scale, "row_spacing": g["grid"]["row_spacing"] * scale}
    if shape == "line":
        g["start"], g["end"] = mapped(m, g["start"]), mapped(m, g["end"])
    if shape == "polyline":
        g["points"] = [mapped(m, p) for p in g["points"]]
    if shape in ("circle", "arc"):
        g["center"], g["radius"] = mapped(m, g["center"]), g["radius"] * scale
    if shape == "arc":
        g["start_angle"], g["end_angle"] = g["start_angle"] + turn, g["end_angle"] + turn
    if shape == "ellipse":
        # 角度は長軸から測るので変わらない
        g["center"], g["major"] = mapped(m, g["center"]), mapped(m, g["major"])
    if shape not in ("point", "symbol", "text", "formatted_text", "line", "polyline", "circle", "arc", "ellipse"):
        raise ValueError(f"形 {shape} の変換の写し方を決めていない")
    return g


def points_steps(fmt):
    """#342: 点を置き換え・変換・追加し、DXF・DWG は文書の点の表し方も変える"""

    def step(reading):
        edits = []
        if fmt in ("dxf", "dwg"):
            found = [e for e in model(reading, "point") if not e["geometry"].get("construction")]
            if found:
                p = found[0]["geometry"]["position"]
                edits.append(replace(found[0], {"shape": "point", "position": [p[0] + 5, p[1] + 7], "rotation": 30}))
            if len(found) > 1:
                edits.append(transform(found[1], similar(90, 1, 10, 20)))
            edits.append(add("space/0", {"shape": "point", "position": [100, 50], "rotation": 0}))
            edits.append(add("space/0", {"shape": "point", "position": [120, 50], "rotation": 45}))
            edits.append({"kind": "point_style", "mark": "cross", "circle": False, "square": True, "size": 4, "size_kind": "length"})
        elif fmt == "jww":
            # 普通の点は角度 0 だけ。記号の点は jww.point の番号（決定 0183）。レイヤは線分と同じもので、JWW も
            # レイヤに従う色を持たないので、ペンの番号を書く（2 は黒）
            layer, color = model(reading)[0]["layer"], {"jww": 2}
            edits.append(add("space/0", {"shape": "point", "position": [1000, 2000], "rotation": 0}, layer=layer, color=color))
            point = {"shape": "point", "position": [3000, 2000], "rotation": 30, "marker": {"namespace": "jww.point", "code": "3", "scale": 2}}
            edits.append(add("space/0", point, layer=layer, color=color))
        else:
            # SXF はレイヤに従う色を持たないので、色の番号を書く（2 は赤）
            layer = reading["entities"][0]["layer"]
            marker = {"namespace": "sxf.marker", "code": "1", "scale": 1}
            color = {"sxf": 2}
            edits.append(add("sheet/0", {"shape": "point", "position": [50, 60], "rotation": 30, "marker": marker}, layer=layer, color=color))
            point = {"shape": "point", "position": [80, 60], "rotation": 0, "marker": {**marker, "code": "3"}}
            edits.append(add("sheet/0", point, layer=layer, color=color))
        return edits

    return [step]


def basic_steps(_fmt):
    """#156: 画層を足し、単純な図形を足し・置き換え・変換し、色を変える"""

    def layer(_reading):
        return [{"kind": "add_layer", "name": "ZUMEN_ADDED", "color": {"aci": 3}}]

    def shapes(reading):
        edits = [
            add("space/0", {"shape": "line", "start": [0, -50], "end": [100, -30]}, layer="ZUMEN_ADDED"),
            add("space/0", {"shape": "circle", "center": [150, -40], "radius": 15}),
            add("space/0", {"shape": "arc", "center": [200, -40], "radius": 15, "start_angle": 0, "end_angle": 135}),
            # major は長軸の端の点（中心からの向きでない）
            add("space/0", {"shape": "ellipse", "center": [250, -40], "major": [270, -40], "ratio": 0.5, "start_angle": 0, "end_angle": 360}),
            add("space/0", {"shape": "polyline", "points": [[0, -100], [40, -100], [40, -70]], "bulges": [0, 0.5, 0], "closed": False}),
            add("space/0", {"shape": "text", "content": "ZUMEN TEXT", "position": [0, -130], "height": 5, "rotation": 0}),
            add("space/0", {"shape": "formatted_text", "content": "ZUMEN\\PMTEXT", "position": [100, -120], "height": 5, "rotation": 0}),
        ]
        named = [e for e in model(reading, shape="symbol") if not e["geometry"]["name"].startswith("*")]
        if named:
            edits.append(add("space/0", {"shape": "symbol", "name": named[0]["geometry"]["name"], "position": [300, -100], "scale": [1, 1], "rotation": 0}))
        circles = model(reading, shape="circle")
        if circles:
            c = circles[0]["geometry"]
            edits.append(replace(circles[0], {"shape": "circle", "center": [c["center"][0] + 10, c["center"][1]], "radius": c["radius"] * 1.5}))
        return edits

    def moves(reading):
        edits = []
        lines = [e for e in model(reading, shape="polyline") if not any(e["geometry"].get("bulges", []))]
        if lines:
            edits.append(transform(lines[0], similar(0, 1, 0, -20)))
        circles = [e for e in model(reading, shape="circle") if e["geometry"]["radius"] != 15]
        if circles:
            edits.append({"kind": "restyle", "id": circles[0]["id"], "hash": circles[0]["hash"], "color": {"aci": 1}})
        return edits

    return [layer, shapes, moves]


def minsert_steps(_fmt):
    """#316: 格子の記号（MINSERT）を変換し・置き換え、足す"""

    def step(reading):
        grids = [e for e in model(reading, shape="symbol") if "grid" in e["geometry"]]
        edits = []
        if grids:
            edits.append(transform(grids[0], similar(30, 2, 50, 0)))
        if len(grids) > 1:
            g = dict(grids[1]["geometry"])
            p = g["position"]
            g["position"] = [p[0], p[1] + 100]
            g["grid"] = {**g["grid"], "column_spacing": g["grid"]["column_spacing"] * 1.5, "row_spacing": g["grid"]["row_spacing"] * 1.5}
            edits.append(replace(grids[1], g))
        if grids:
            grid = {"columns": 3, "rows": 2, "column_spacing": 30, "row_spacing": 20}
            edits.append(add("space/0", {"shape": "symbol", "name": grids[0]["geometry"]["name"], "position": [300, 300], "scale": [1, 1], "rotation": 0, "grid": grid}))
        return edits

    return [step]


def replace(e, geometry):
    return {"kind": "replace", "id": e["id"], "hash": e["hash"], "geometry": geometry, "_expect": geometry}


def transform(e, m):
    return {"kind": "transform", "id": e["id"], "hash": e["hash"], "matrix": m, "_expect": transformed(e["geometry"], m)}


def add(place, geometry, layer=None, color=None):
    edit = {"kind": "add", "place": place, "geometry": geometry, "_expect": geometry}
    if layer:
        edit["layer"] = layer
    if color:
        edit["color"] = color
    return edit


# 束の組。名前・対象の issue・入力の選び方・変更の作り方
RECIPES = [
    ("points", 342, lambda name, fmt: fmt in ("jww", "sfc", "p21") or "points-" in name, points_steps),
    ("basic", 156, lambda name, fmt: fmt in ("dxf", "dwg") and ("-layout-" in name or "-xclip-r2013" in name), basic_steps),
    ("minsert", 316, lambda name, fmt: fmt in ("dxf", "dwg") and "-minsert-" in name, minsert_steps),
]


def run_recipe(z, out, entry, data, recipe):
    """入力に変更の段を順に当て、最後に書いたファイルを置く。どれかの段が当たらなければ、その診断を残す"""
    name, issue, _, steps = recipe
    stem, suffix = Path(entry["path"]).stem, Path(entry["path"]).suffix
    doc = f"{issue}/{stem}.{name}{suffix}"
    open_file(z, doc, data)
    record = {"path": doc, "role": "apply", "issue": issue, "recipe": name, "from": entry["path"], "format": entry["format"], "steps": []}
    current = data
    for make in steps(entry["format"]):
        reading = read(z, doc)
        edits = make(reading)
        if not edits:
            # 当てる候補がない段（例: 膨らみのないポリラインがない図面の移動）も、束に残して見えるようにする
            record["steps"].append({"step": make.__name__, "edits": [], "skipped": True})
            continue
        request = [{k: v for k, v in e.items() if not k.startswith("_")} for e in edits]
        status, response = z.call({"op": "apply", "doc": doc, "edits": request})
        step = {"step": make.__name__, "edits": request, "status": status, "diagnostics": response["diagnostics"]}
        record["steps"].append(step)
        if status != "ok":
            record["status"] = "failed"
            return record
        applied = response["result"]["applied"]
        # 期待する形と、足した要素の画層
        step["expect"] = [
            {"id": a.get("id"), "edit": i, "geometry": e["_expect"], **({"layer": e["layer"]} if "layer" in e else {})}
            for i, (e, a) in enumerate(zip(edits, applied))
            if "_expect" in e and a.get("id")
        ]
        current = z.take(response["blobs"][0]["id"])
    (out / doc).parent.mkdir(parents=True, exist_ok=True)
    (out / doc).write_bytes(current)
    record["status"] = "ok"
    record["open"] = open_file(z, doc + ".reopened", current)
    save_reading(out, doc, read(z, doc + ".reopened"))
    return record


def run_convert(z, out, entry, data):
    """図面モデルを経て DXF に変換する（決定 0206）"""
    source = f"convert/{Path(entry['path']).name}"
    target = source + ".dxf"
    open_file(z, source, data)
    status, response = z.call({"op": "convert", "doc": source, "format": "dxf", "to": target})
    record = {"path": target, "role": "convert", "issue": 302, "from": entry["path"], "format": "dxf", "status": status}
    record["diagnostics"] = response["diagnostics"]
    if status != "ok":
        return record
    record["elements"] = response["result"]["elements"]
    data = z.take(response["blobs"][0]["id"])
    (out / target).parent.mkdir(parents=True, exist_ok=True)
    (out / target).write_bytes(data)
    record["open"] = open_file(z, target + ".reopened", data)
    save_reading(out, target, read(z, target + ".reopened"))
    return record


def tag_lines(data):
    """DXF のタグの行（改行を含むバイト列）の組"""
    lines = data.splitlines(keepends=True)
    return [(lines[i], lines[i + 1]) for i in range(0, len(lines) - 1, 2)]


def dimension_experiments(out, entry, data):
    """#314: 寸法の部品（匿名のブロック）を書き直さずに寸法を変えた DXF。zumen の書き手を通さない実験で、AutoCAD が
    部品を作り直すかを見る。apply は寸法の変更を断る（change.edit.unsupported）"""
    tags = tag_lines(data)
    starts = [i for i, (code, value) in enumerate(tags) if code.strip() == b"0" and value.strip() == b"ARC_DIMENSION"]
    if not starts:
        return []

    def end_of(i):
        return next((j for j in range(i + 1, len(tags)) if tags[j][0].strip() == b"0"), len(tags))

    def handle(i):
        return next(tags[j][1].strip().decode() for j in range(i, end_of(i)) if tags[j][0].strip() == b"5")

    def write(name, new_tags, note):
        path = f"314/{Path(entry['path']).stem}.{name}.dxf"
        (out / path).parent.mkdir(parents=True, exist_ok=True)
        (out / path).write_bytes(b"".join(c + v for c, v in new_tags))
        return {"path": path, "role": "experiment", "issue": 314, "from": entry["path"], "format": "dxf", "note": note}

    def eol(value):
        return value[len(value.rstrip()) :]

    records = []
    first = starts[0]
    end = end_of(first)
    # 1. 寸法の弧の点（10・20）を動かす。部品は元のまま
    moved = list(tags)
    for j in range(first, end):
        code, value = moved[j]
        if code.strip() in (b"10", b"20"):
            moved[j] = (code, f"{float(value) + 20.0!r}".encode() + eol(value))
    note = f"ARC_DIMENSION（ハンドル {handle(first)}）の寸法の弧の点（10・20）を 20 ずつ動かし、部品（2 のブロック）は元のまま"
    records.append(write("exp-moved", moved, note))
    # 2. 部品の名前（2）を除く
    unnamed = [t for j, t in enumerate(tags) if not (first <= j < end and t[0].strip() == b"2")]
    records.append(write("exp-unnamed", unnamed, f"ARC_DIMENSION（ハンドル {handle(first)}）の部品の名前（2）を除く。ブロックは残る"))
    # 3. 利用者の文字（1）を変える
    for i in starts:
        k = next((j for j in range(i, end_of(i)) if tags[j][0].strip() == b"1"), None)
        if k is not None:
            changed = list(tags)
            changed[k] = (tags[k][0], "変更<>".encode("utf-8") + eol(tags[k][1]))
            note = f"ARC_DIMENSION（ハンドル {handle(i)}）の利用者の文字（1）を「変更<>」にし、部品は元のまま"
            records.append(write("exp-text", changed, note))
            break
    return records


def probe_dimension(z, entry, data):
    """#314: 今の apply が寸法の変更を断ることを、応答の診断で残す"""
    doc = f"314/{Path(entry['path']).name}.probe"
    open_file(z, doc, data)
    dims = model(read(z, doc), "dimension")
    if not dims:
        return []
    g = dict(dims[0]["geometry"])
    g["arc"] = [g["arc"][0] + 20, g["arc"][1] + 20]
    status, response = z.call({"op": "apply", "doc": doc, "edits": [{"kind": "replace", "id": dims[0]["id"], "hash": dims[0]["hash"], "geometry": g}]})
    return [{"path": doc, "role": "probe", "issue": 314, "from": entry["path"], "format": "dxf", "status_of_apply": status, "diagnostics": response["diagnostics"]}]


def save_reading(out, path, reading):
    target = out / "zumen" / (path + ".json")
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(json.dumps(reading, ensure_ascii=False, indent=1, sort_keys=True) + "\n", encoding="utf-8")


# README.md（束の手順と期待する結果）


def num(v):
    v = round(float(v), 6)
    return f"{v:g}" if v != 0 else "0"


def pt(p):
    return f"({num(p[0])}, {num(p[1])})"


def describe(g):
    """形の値を、開いたソフトで確かめる言葉にする"""
    s = g["shape"]
    if s == "point":
        text = f"点 {pt(g['position'])}・角度 {num(g.get('rotation', 0))}°"
        if "marker" in g:
            m = g["marker"]
            text += f"・マーカー {m['namespace']} の {m['code']}（倍率 {num(m['scale'])}）"
        return text
    if s == "line":
        return f"線分 {pt(g['start'])} から {pt(g['end'])}"
    if s == "circle":
        return f"円 中心 {pt(g['center'])}・半径 {num(g['radius'])}"
    if s == "arc":
        return f"円弧 中心 {pt(g['center'])}・半径 {num(g['radius'])}・{num(g['start_angle'])}° から {num(g['end_angle'])}°（反時計回り）"
    if s == "ellipse":
        return f"楕円 中心 {pt(g['center'])}・長軸の端 {pt(g['major'])}・比 {num(g['ratio'])}"
    if s == "polyline":
        bulges = "・膨らみ " + "/".join(num(b) for b in g["bulges"]) if any(g.get("bulges", [])) else ""
        return f"ポリライン {'閉じた' if g['closed'] else '開いた'} " + " ".join(pt(p) for p in g["points"]) + bulges
    if s in ("text", "formatted_text"):
        kind = "文字" if s == "text" else "書式つきの文字"
        return f"{kind}「{g['content']}」・位置 {pt(g['position'])}・高さ {num(g['height'])}・角度 {num(g.get('rotation', 0))}°"
    if s == "symbol":
        text = f"記号「{g['name']}」・位置 {pt(g['position'])}・尺度 {num(g['scale'][0])}×{num(g['scale'][1])}・角度 {num(g['rotation'])}°"
        if "grid" in g:
            grid = g["grid"]
            text += f"・格子 {grid['columns']} 列 {grid['rows']} 行（間隔 {num(grid['column_spacing'])}・{num(grid['row_spacing'])}）"
        return text
    return s


VIEWERS = {"dxf": "AutoCAD", "dwg": "AutoCAD", "jww": "Jw_cad", "sfc": "SXF ブラウザ", "p21": "SXF ブラウザ"}

INTRO = """# 確認の束

zumen の CLI で、公開サンプル（`samples/`）と適合テストの中で作った図面に、書き戻し（apply）と変換（convert）を当てて書いたファイルです（#503、決定 0254）。`xtask/verify/make.py` が作りました。

- `manifest.json`: ファイルごとの出所・当てた変更・期待する形・診断
- `zumen/`: zumen が読んだ値（`drawing`・`tables`・`entities` の応答）。入力と書いたファイルのそれぞれ
- `ezdxf.json`: DXF を ezdxf で読んで照らした結果（`check_ezdxf.py`）
- `input/`: 入力の図面。書いたファイルを開く前に、同じソフトで入力も開き、入力から変わった所だけを比べる
- `licenses/`: 入力のライセンスの本文。入力ごとのライセンスと表示は、下の「出所とライセンス」と `manifest.json` にある

結果は、対象の issue に、ソフトとバージョン・開けたか・監査の誤りの数・期待と違った所を書きます。確かめたことと、確かめられなかったことを分けて書きます。

## AutoCAD の共通の手順

1. ファイルを開く（DXF は「開く」でファイルの種類を DXF にする）。開けなければ、出た文を記録して次へ進む。
2. `AUDIT` を「いいえ」（直さない）で流し、見つかった誤りの数を記録する。期待は 0。
3. フォルダー `C:/zumen-verify/` を作り、`(load "dump.lsp")` と `(zv-dump "C:/zumen-verify/<ファイルの名前>.txt")` で、図形をハンドルごとに書き出す（`dump.lsp` は束にある）。下の表のハンドルの行を、期待する値と比べる。角度（50・51）は度に直して書く。楕円の 11（中心から長軸の端への向き）と 41・42（ラジアン）は、ファイルの値のまま書くので、表の「長軸の端」は中心に 11 を足して比べる。
4. 画面でも、下の表の形が見えることを確かめる。

画面なしで流すときは、`accoreconsole.exe /i <ファイル> /s <台本>` に、`(load …)`・`_.AUDIT _N`・`(zv-dump …)`・`_.QUIT _Y` の台本を渡す（`xtask/autocad/run.ps1` と同じ）。
"""

SECTIONS = {
    342: (
        "#342 書き戻した点",
        """点を置き換え・変換・追加して書き戻したファイルです（決定 0183・0190）。

- **AutoCAD（DXF・DWG）。** 共通の手順のあと、点の形が $PDMODE 67（× に四角）・$PDSIZE 4 で描かれることを確かめる。角度のある点のマーカーは、その角度だけ回って描かれる。
- **Jw_cad（JWW）。** 開いて、下の 2 つの点が実寸の座標（mm、用紙の中心が原点）にあることを、「測定」か「属性取得」で確かめる。普通の点の線種が、Jw_cad で打った点と同じ（線種 1、決定 0183 の推定）かを確かめる。記号の点（線種 100）は、番号 3・角度 30°・倍率 2 で描かれる。
- **SXF ブラウザ（SFC・P21）。** 開いて、用紙の座標（mm）の 2 つの点マーカーが、コードの形・回転・赤（既定義色 2）で描かれることを確かめる。SXF ブラウザがなければ、Jw_cad の SXF の読み込みで開いて、同じことを確かめる。
""",
    ),
    156: (
        "#156 書き戻した DWG を AutoCAD で開く",
        """R2000・R2004・R2013・R2018 の DWG と、同じ変更を当てた DXF です。画層を足し、線分・円・円弧・楕円・ポリライン・文字・書式つきの文字（と記号）を足し、円を置き換えて赤（ACI 1）にし、長方形のポリラインを下へ 20 動かしました（決定 0079〜0104・0155・0162）。

- **AutoCAD。** 共通の手順で、開けること・監査の誤りが 0 であること・下の表の形を確かめる。足した線分は、足した画層 ZUMEN_ADDED（緑、ACI 3）にある。
""",
    ),
    316: (
        "#316 AutoCAD で作ったサンプルが要る値",
        """**格子の記号（MINSERT）。** AutoCAD で作った MINSERT のサンプルに、変換（30° 回し、2 倍にし、X に 50 動かす）・置き換え（Y に 100 動かし、間隔を 1.5 倍にする）・追加（3 列 2 行）を当てて書き戻しました（決定 0161）。

- **AutoCAD。** 共通の手順で、格子の列・行・間隔と、属性（TAGGED の NO・KIND）が記号とともに動いたことを確かめる。

**点の印刷（決定 0159）。** `input/zumen-points-a-r2018.dwg` を、モデルのタブから PDF に印刷する。期待は、画層 0 の点（ハンドル 2DB）と記号 BYBLOCKPOINT（2E5）の中の点が印刷され、印刷しない画層（NOPLOT の 2DC、Defpoints の 2DD）の点が印刷されないこと。zumen は、表示し、実効の画層が印刷するものを印刷するとしている（推定）。

**まだサンプルのない値。** MTEXT の `\\c`・`\\p`、MULTILEADER の線ごとの上書き、文字スタイルの太字・斜体（1071 のビット 24・25）は、画面のある AutoCAD（COM か操作）で作る。作ったら、`samples/dwg` に足し（決定 0171）、zumen の読みと照らす。
""",
    ),
    314: (
        "#314 寸法の部品を作り直すかを確かめる実験",
        """今の apply は、寸法の変更を `change.edit.unsupported` で断ります（下の probe）。#314 で部品（匿名のブロック）を作り直す前に、部品を書き直さずに寸法を変えた DXF を、AutoCAD がどう描くかを確かめます。どれも zumen の書き手を通さず、`make.py` が入力の DXF のタグを書き換えたものです。

- **AutoCAD。** 入力と 3 つの実験のファイルを開き、寸法の見え方を比べる。それぞれ、(1) 元の部品のまま描くか、(2) 部品を作り直して描くか、(3) 開けないか、と監査の誤りの数を記録する。`exp-unnamed` は、`AUDIT` を「はい」で流したときに部品を作り直すかも記録する。
""",
    ),
    302: (
        "変換した DXF（図面モデルを経た変換、#302・#509）",
        """入力のどの形式も、図面モデルを経て DXF（R2004）に変換しました（決定 0206）。書けない要素は `convert.element.unsupported` で落とします。ひな形の表と VIEWPORT が R12 の形で、ezdxf が読めないものがあります（#509）。

- **AutoCAD。** 共通の手順で開けるかと監査の誤りの数を記録する。ビューポートを持つもの（layout・points・JWW）が開けるかは、#509 を直す前と後で比べる。
""",
    ),
    174: (
        "#174 DXF から図面モデルへ移す値",
        """入力の DXF と DWG（AutoCAD で作ったサンプル）を zumen が読んだ値は `zumen/input/` に、ezdxf と照らした結果は `ezdxf.json` にあります。

- **AutoCAD。** 共通の手順の 3 で書き出した値を、`zumen/input/<ファイル>.json` の `entities` の `geometry` と比べる。違えば、ハンドル・フィールド・2 つの値を issue に書く。
- **まだサンプルのない推定。** MULTILEADER の色の値（AcCmColor）、HATCH の SPLINE の辺のフィット点（R2010 から）、表のセルの 2007 からの値、アンダーレイの切り抜きの頂点、R2018 の 101 を持つ MTEXT は、AutoCAD で作ったサンプルが要る。作り方は `xtask/autocad/samples.lsp` に主題を足して、`make.sh` で作る。
""",
    ),
}


def readme(out, manifest):
    files = manifest["files"]
    lines = [INTRO]
    lines += [
        "## 出所とライセンス",
        "",
        "`input/` の外の図面は、どれも下の入力を変えたものです（書き戻しと変換は zumen が、#314 の実験は `make.py` が変えた）。ライセンスの本文は `licenses/` にあります。",
        "",
        "| 入力 | ライセンス | 表示 |",
        "| --- | --- | --- |",
    ]
    lines += [f"| `{f['path']}` | {f['license']} | {f['attribution']} |" for f in files if f["role"] == "input" and "license" in f]
    lines.append("")
    for issue, (title, body) in SECTIONS.items():
        lines += [f"## {title}", "", body]
        chosen = [f for f in files if f.get("issue") == issue]
        if issue == 174:
            chosen = [f for f in files if f["role"] == "input" and f["source"].startswith("samples/")]
        if not chosen:
            continue
        lines += ["| ファイル | 開くソフト | 期待する結果 |", "| --- | --- | --- |"]
        for f in chosen:
            lines.append(f"| `{f['path']}` | {VIEWERS.get(f['format'], '')} | {expectation(out, f)} |")
        lines.append("")
    (out / "README.md").write_text("\n".join(lines), encoding="utf-8")


def expectation(out, f):
    """表の 1 行の、期待する結果"""
    if f["role"] == "input":
        return "開けて、監査の誤りが 0。値は zumen の読み（`zumen/` の JSON）と同じ"
    if f["role"] == "experiment":
        return f["note"] + "。見え方を記録する"
    if f["role"] == "probe":
        return "（ファイルなし）apply の応答: " + ", ".join(d["code"] for d in f["diagnostics"])
    if f.get("status") != "ok":
        return "（書けなかった）" + ", ".join(d["code"] for s in f.get("steps", []) for d in s.get("diagnostics", []))
    if f["role"] == "convert":
        dropped = Counter()
        for d in f["diagnostics"]:
            if d["code"] == "convert.element.unsupported":
                dropped[d["params"].get("kind")] += d["count"]
        text = f"開けて、監査の誤りが 0。要素 {f['elements']}"
        if dropped:
            text += "。落とした要素: " + "・".join(f"{k} {n}" for k, n in sorted(dropped.items()))
        return text
    reading = json.loads((out / "zumen" / (f["path"] + ".json")).read_text(encoding="utf-8"))
    handles = {e["id"]: e.get("source", {}).get("handle") for e in reading["entities"]}
    items = []
    for step in f["steps"]:
        if step.get("skipped"):
            items.append(f"段 {step['step']} は当てる候補がなく、当てていない")
        for want in step.get("expect", []):
            handle = handles.get(want["id"])
            layer = f"・画層 {want['layer']}" if "layer" in want else ""
            items.append(f"{'ハンドル ' + handle if handle else want['id']}: {describe(want['geometry'])}{layer}")
        for edit in step["edits"]:
            if edit["kind"] == "point_style":
                items.append("点の表し方: $PDMODE 67・$PDSIZE 4")
            if edit["kind"] == "add_layer":
                items.append(f"画層 {edit['name']}（ACI {edit['color']['aci']}）")
            if edit["kind"] == "restyle":
                items.append(f"ハンドル {handles.get(edit['id'])} の色が ACI {edit['color']['aci']}")
    return "開けて、監査の誤りが 0。" + "<br>".join(items)


def inputs(extra):
    """入力の図面: 公開サンプル、適合テストの中で作った図面、--inputs のフォルダー。(道筋, 中身, 出所, ライセンス, 表示)"""
    ledger = tomllib.loads((ROOT / "samples" / "ledger.toml").read_text(encoding="utf-8"))["samples"]
    ledger = {item["path"]: item for item in ledger}
    found = []
    for path in sorted((ROOT / "samples").rglob("*")):
        if path.suffix.lower() in SUFFIXES:
            item = ledger[path.relative_to(ROOT / "samples").as_posix()]
            found.append((f"input/{path.name}", path.read_bytes(), path.relative_to(ROOT).as_posix(), item["license"], item["attribution"]))
    for case, name in CASES:
        steps = json.loads((ROOT / "bindings/conformance/cases" / case).read_text(encoding="utf-8"))["steps"]
        push = next(s["push"] for s in steps if s.get("push", {}).get("name") == name)
        data = bytes.fromhex(push["hex"]) if "hex" in push else push["text"].encode("utf-8")
        found.append((f"input/conformance-{Path(case).stem}-{name}", data, f"bindings/conformance/cases/{case}", *REPOSITORY))
    if extra:
        # 受け取った図面の束は配らないので、ライセンスを書かない
        for path in sorted(Path(extra).rglob("*")):
            if path.suffix.lower() in SUFFIXES:
                found.append((f"input/local-{path.name}", path.read_bytes(), "--inputs", None, None))
    return found


def main():
    parser = argparse.ArgumentParser(description="確認の束を作る")
    parser.add_argument("zumen")
    parser.add_argument("out")
    parser.add_argument("--inputs")
    args = parser.parse_args()
    out = Path(args.out)
    if args.inputs and out.resolve().is_relative_to(ROOT):
        print("受け取った図面の束は、リポジトリの外に作る", file=sys.stderr)
        return 2
    if out.exists():
        # 誤って作業ツリーを指しても消さないよう、前の束か空のフォルダーだけを消す
        if any(out.iterdir()) and not (out / "manifest.json").exists():
            print(f"{out} は前の束でも空でもないので、消さない", file=sys.stderr)
            return 2
        shutil.rmtree(out)
    out.mkdir(parents=True)
    z = Zumen(args.zumen)
    _, capabilities = z.call({"op": "capabilities"})
    files = []
    for path, data, source, terms, attribution in inputs(args.inputs):
        fmt = SUFFIXES[Path(path).suffix.lower()]
        (out / path).parent.mkdir(parents=True, exist_ok=True)
        (out / path).write_bytes(data)
        entry = {"path": path, "role": "input", "format": fmt, "source": source, "open": open_file(z, path, data)}
        if terms:
            entry["license"], entry["attribution"] = terms, attribution
        save_reading(out, path, read(z, path))
        files.append(entry)
        for recipe in RECIPES:
            if recipe[2](Path(path).name, fmt):
                files.append(run_recipe(z, out, entry, data, recipe))
        files.append(run_convert(z, out, entry, data))
        # 文字を UTF-8 で書けるのは 2007 からの DXF
        if fmt == "dxf" and "-arcdim-r2018" in path:
            files += dimension_experiments(out, entry, data)
            files += probe_dimension(z, entry, data)
    z.close()
    manifest = {"version": 1, "capabilities": capabilities["result"], "local": bool(args.inputs), "files": files}
    (out / "manifest.json").write_text(json.dumps(manifest, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
    readme(out, manifest)
    shutil.copy(Path(__file__).with_name("dump.lsp"), out / "dump.lsp")
    # 束は公開サンプルと適合テストの図面の写しと、それを変えたファイルを配るので、ライセンスの本文を写す
    (out / "licenses").mkdir()
    for path in [*sorted((ROOT / "licenses").iterdir()), ROOT / "LICENSE-MIT", ROOT / "LICENSE-APACHE"]:
        shutil.copy(path, out / "licenses" / path.name)
    # 当たらなかった変更と変換は、束を書き終えてから失敗にする（前は当たっていたものが当たらなくなったら知らせる）
    failed = [f for f in files if f.get("status") not in (None, "ok")]
    for f in failed:
        print(f"{f['path']}: {f.get('status')} {[d['code'] for s in f.get('steps', []) for d in s.get('diagnostics', [])] or f.get('diagnostics')}", file=sys.stderr)
    print(f"{len(files)} のファイル（当たらなかった組 {len(failed)}）を {out} に置いた")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
