"""R2007 より前の DXF の Windows の 1 バイトのコード・ページの文字を、Python の codec と ezdxf の読みに照らす（#327）。

コード・ページごとに、割り当てのあるすべてのバイトを持つ TEXT と、表の文字を名前に含むレイヤを ezdxf で作り、
zumen の読み・書き戻し・開き直しを照らす。割り当てのないバイトを持つ TEXT は、Python の codec が拒み、zumen が
dxf.lift.invalid で落とすことを照らす。
"""

import argparse
import io
import json
from pathlib import Path
import sys

import ezdxf

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))
from make import Zumen  # noqa: E402

PAGES = (874, 1250, 1251, 1252, 1253, 1254, 1255, 1256, 1257, 1258)


def assigned(page):
    """コード・ページの 0x80〜0xFF のうち、Python の codec が文字に戻すバイトと、その文字"""
    found = []
    for byte in range(0x80, 0x100):
        try:
            found.append((byte, bytes([byte]).decode(f"cp{page}")))
        except UnicodeDecodeError:
            pass
    return found


def fixture(page, text, layer):
    """R2000 の DXF。文字は ezdxf がコード・ページの名前から選ぶ codec で書く"""
    # https://ezdxf.readthedocs.io/en/stable/options.html
    ezdxf.options.write_fixed_meta_data_for_testing = True
    doc = ezdxf.new("R2000")
    # https://ezdxf.readthedocs.io/en/stable/drawing/drawing.html#ezdxf.document.Drawing.encoding
    # 書く codec（encoding）と、ヘッダーのコード・ページの名前は別に置く
    doc.encoding = f"cp{page}"
    doc.header["$DWGCODEPAGE"] = f"ANSI_{page}"
    doc.layers.new(layer)
    doc.modelspace().add_text(text, dxfattribs={"layer": layer, "height": 2.5})
    stream = io.StringIO()
    doc.write(stream)
    return stream.getvalue().encode(doc.output_encoding)


def read_ezdxf(path):
    """ezdxf が $DWGCODEPAGE から選ぶ codec で読んだ文書の TEXT のレイヤと文字列、その codec"""
    doc = ezdxf.readfile(path)
    return [(e.dxf.layer, e.dxf.text) for e in doc.modelspace().query("TEXT")], doc.encoding


def zumen_texts(z, name):
    status, response = z.call({"op": "entities", "doc": name, "fields": ["id", "kind", "layer", "geometry"]})
    if status != "ok":
        raise RuntimeError(f"entities {name}: {status} {response}")
    return [(item.get("layer"), item.get("geometry", {}).get("content")) for item in response["result"]["items"]]


def check(z, page, out):
    failures = []
    table = assigned(page)
    text = "".join(c for _, c in table)
    letters = [c for _, c in table if c.isalpha()][:3]
    layer = "L" + "".join(letters)
    data = fixture(page, text, layer)
    source = out / f"cp{page}-input.dxf"
    source.write_bytes(data)
    if bytes(b for b, _ in table) not in data:
        failures.append("入力の TEXT が、割り当てのあるバイトを 1 バイトずつ持たない")
    expected, codec = read_ezdxf(source)
    if codec != f"cp{page}":
        failures.append(f"ezdxf の codec が cp{page} でない: {codec}")
    if expected != [(layer, text)]:
        failures.append("ezdxf の読みが作った値と違う")
    name = f"cp{page}.dxf"
    z.push(name, data)
    status, response = z.call({"op": "open", "input": name})
    if status != "ok":
        return [f"開けない: {status} {response}"]
    if zumen_texts(z, name) != expected:
        failures.append(f"zumen の読みが ezdxf と違う: {zumen_texts(z, name)!r}")
    _, tables = z.call({"op": "tables", "doc": name})
    ids = [item["id"] for item in tables["result"]["layers"] if item.get("name") == layer]
    if len(ids) != 1:
        return failures + [f"zumen の表にレイヤ {layer!r} が 1 つない"]
    # 書き戻し: 表のすべての文字を逆の順で足す
    reversed_text = text[::-1]
    status, response = z.call(
        {
            "op": "apply",
            "doc": name,
            "edits": [
                {
                    "kind": "add",
                    "place": "space/0",
                    "layer_id": ids[0],
                    "geometry": {"shape": "text", "content": reversed_text, "position": [1, 2], "height": 2.5, "rotation": 0},
                }
            ],
        }
    )
    if status != "ok":
        return failures + [f"書き戻せない: {status} {response}"]
    output = z.take(response["blobs"][0]["id"])
    target = out / f"cp{page}-output.dxf"
    target.write_bytes(output)
    if bytes(b for b, _ in table)[::-1] not in output:
        failures.append("書き戻した TEXT が、表のバイトを逆の順で持たない")
    written, _ = read_ezdxf(target)
    if written != [(layer, text), (layer, reversed_text)]:
        failures.append(f"書き戻した DXF の ezdxf の読みが違う: {written!r}")
    # 表せない文字は、変更全体を拒む
    status, _ = z.call(
        {
            "op": "apply",
            "doc": name,
            "edits": [
                {
                    "kind": "add",
                    "place": "space/0",
                    "layer_id": ids[0],
                    "geometry": {"shape": "text", "content": "日", "position": [1, 2], "height": 2.5, "rotation": 0},
                }
            ],
        }
    )
    if status != "failed":
        failures.append(f"表せない文字を拒まない: {status}")
    # 割り当てのないバイト
    unassigned = [b for b in range(0x80, 0x100) if b not in {byte for byte, _ in table}]
    if unassigned:
        byte = unassigned[0]
        broken = data.replace(bytes([table[0][0]]), bytes([byte]), 1)
        try:
            broken.decode(f"cp{page}")
            failures.append(f"Python の codec が 0x{byte:02X} を拒まない")
        except UnicodeDecodeError:
            pass
        bad = f"cp{page}-unassigned.dxf"
        z.push(bad, broken)
        z.call({"op": "open", "input": bad})
        _, drawing = z.call({"op": "drawing", "doc": bad})
        codes = [d.get("code") for d in (drawing or {}).get("diagnostics", [])]
        if "dxf.lift.invalid" not in codes:
            failures.append(f"割り当てのないバイト 0x{byte:02X} の TEXT を dxf.lift.invalid で落とさない: {codes}")
    return failures


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", required=True, help="zumen の CLI")
    parser.add_argument("--out", required=True, help="結果を書く空のフォルダー")
    args = parser.parse_args()
    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    if any(out.iterdir()):
        print(f"{out} が空でない", file=sys.stderr)
        return 2
    results = {}
    z = Zumen(args.cli)
    try:
        for page in PAGES:
            results[f"ANSI_{page}"] = check(z, page, out)
    finally:
        z.close()
    with (out / "results.json").open("x", encoding="utf-8", newline="\n") as stream:
        stream.write(json.dumps({"ezdxf": ezdxf.__version__, "results": results}, ensure_ascii=False, indent=2) + "\n")
    for page, failures in results.items():
        print(page, "一致" if not failures else failures)
    return 1 if any(results.values()) else 0


if __name__ == "__main__":
    sys.exit(main())
