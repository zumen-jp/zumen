"""公開版で JWW の注記を書き換え、同じグループの別レイヤに移す（#921）。"""

import argparse
import math
from pathlib import Path
import runpy
import struct


TEXT = {
    "shape": "text", "content": "施工前", "position": [2000, 3000], "height": 300,
    "rotation": 30, "horizontal": "left", "vertical": "bottom", "style": "ＭＳ ゴシック",
}


def request(session, op, **fields):
    response = session.call({"v": 1, "op": op, **fields})
    if response["diagnostics"]:
        raise ValueError(f"{op}: {response['diagnostics']}")
    return response


def opened(session, name, data):
    session.push(name, data)
    return request(session, "open", input=name)["result"]["doc"]


def apply(session, doc, edits):
    response = request(session, "apply", doc=doc, edits=edits)
    file = next(blob for blob in response["blobs"] if blob["kind"] == "file")
    return session.take(file["id"])


def annotation(session, doc):
    items = request(session, "entities", doc=doc, fields=["kind", "id", "layer", "hash"])["result"]["items"]
    text, = (item for item in items if item["kind"] == "text")
    return text


def seed_with_note():
    """公開の JWW 700 の CDataMoji を、線分だけの自作入力に加える。"""
    fixtures = runpy.run_path(str(Path(__file__).parent / "points" / "fixtures.py"))
    header = fixtures["jww_header"]()
    # jww() の図形一覧は件数、線分、ブロックと画像の空の一覧。
    line = fixtures["jww"]()[len(header) + 2:-6]
    tag = struct.pack("<3H", 0xFFFF, 1, 9) + b"CDataMoji"
    base = struct.pack("<IB5H", 0, 1, 1, 0, 0, 0, 0)
    # https://www.jwcad.net/jwdatafmt.txt の CDataMoji::Serialize。
    # 全角3字、字幅3 mm、字間0。終点は文字列の傾きに沿う。
    end_x = 20 + 9 * math.cos(math.radians(30))
    body = struct.pack("<4dI4d", 20, 30, end_x, 34.5, 0, 3, 3, 0, 30)
    for value in (TEXT["style"], TEXT["content"]):
        encoded = value.encode("cp932")
        body += bytes([len(encoded)]) + encoded
    return header + struct.pack("<H", 2) + line + tag + base + body + bytes(6)


def create_and_edit(folder):
    import zumen

    library = zumen.load()
    with library.open() as session:
        doc = opened(session, "seed.jww", seed_with_note())
        original = apply(session, doc, [{"kind": "add_layer", "name": "注記"}])

    # 受け取ったファイルを新しいセッションで開く。0.3.0 は文字の geometry を返さない。
    with library.open() as session:
        doc = opened(session, "input.jww", original)
        text = annotation(session, doc)
        apply(session, doc, [{
            "kind": "replace", "id": text["id"], "hash": text["hash"],
            "geometry": {**TEXT, "content": "施工後"},
        }])
        text = annotation(session, doc)
        edited = apply(session, doc, [{
            "kind": "restyle", "id": text["id"], "hash": text["hash"], "layer": "注記",
        }])
    folder.mkdir(parents=True, exist_ok=False)
    (folder / "input.jww").write_bytes(original)
    (folder / "edited.jww").write_bytes(edited)


def check(path, edited):
    import ezjww

    doc = ezjww.read_document(str(path))
    # Jw_cad の別名保存で加わる用紙外の設定文字を除く。
    settings = {"Printer_Orientation", "Printer_PaperSize", "Printer_D2dBMP",
                "Printer_BmpZENTAI", "View_Direct2d", "Draw_BmpTOUKA"}
    drawing = [item for item in doc["entities"] if not (
        item["type"] == "TEXT" and (item["start_x"], item["start_y"]) == (0, -1000)
        and item["content"].partition(" = ")[0] in settings
    )]
    line, = (item for item in drawing if item["type"] == "LINE")
    text, = (item for item in drawing if item["type"] == "TEXT")
    group = doc["header"]["layer_groups"][0]
    layer = text["base"]["layer"]
    expected = {
        "count": 2, "scale": 100, "line": (0, 0, 100, 50), "line_layer": (0, 0),
        "note": "施工後" if edited else "施工前", "position": (20, 30), "height": 3,
        "end": (20 + 9 * math.cos(math.radians(30)), 34.5), "width": 3, "font": TEXT["style"],
        "rotation": 30, "group": 0, "layer": 1 if edited else 0, "layer_name": "注記" if edited else "0-0",
    }
    actual = {
        "count": len(drawing), "scale": group["scale"],
        "line": (line["start_x"], line["start_y"], line["end_x"], line["end_y"]),
        "line_layer": (line["base"]["layer_group"], line["base"]["layer"]),
        "note": text["content"], "position": (text["start_x"], text["start_y"]), "height": text["size_y"],
        "end": (text["end_x"], text["end_y"]), "width": text["size_x"], "font": text["font_name"],
        "rotation": text["angle"], "group": text["base"]["layer_group"], "layer": layer,
        "layer_name": group["layers"][layer]["name"],
    }
    for field, value in expected.items():
        if actual[field] != value:
            raise ValueError(f"{field}: {actual[field]!r} != {value!r}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    make = commands.add_parser("make", help="新しいフォルダーに自作入力と編集後の JWW を作る")
    make.add_argument("folder", type=Path)
    verify = commands.add_parser("check", help="入力と編集後を ezjww で照合する")
    verify.add_argument("input", type=Path)
    verify.add_argument("edited", type=Path)
    args = parser.parse_args()
    if args.command == "make":
        create_and_edit(args.folder)
    else:
        check(args.input, False)
        check(args.edited, True)
        print("JWW の入力と編集後が期待値に一致しました。")
