"""R2007 より前の DXF の Windows のコード・ページの文字を、Windows の API・Python の codec・ezdxf に照らす（#327）。

コード・ページごとに、割り当てのあるすべてのバイト列を分けて持つ TEXT と、表の文字を名前に含むレイヤの R2000 の
DXF を作り、zumen の読み・書き戻し・開き直しを照らす。割り当ては、Windows では MultiByteToWideChar（Python の
codecs.code_page_decode）が私用領域と C1 制御文字でない文字に戻すバイト列、ほかの OS では Python の codec が戻す
バイト列とする。Python の codec と Windows の表の違いは、既知の数と照らす。割り当てのないバイト列を持つ TEXT は、
zumen が dxf.lift.invalid で落とすことを照らす。
"""

import argparse
import codecs
import io
import json
from pathlib import Path
import sys

import ezdxf
from ezdxf.tools.codepage import toencoding

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))
from make import Zumen  # noqa: E402

SINGLE = (874, 1250, 1251, 1252, 1253, 1254, 1255, 1256, 1257, 1258)
# 2 バイトのコード・ページと、先頭のバイトの範囲（Unicode の MICSFT/WINDOWS/CP<番号>.TXT）
DOUBLE = {936: (0x81, 0xFE), 949: (0x81, 0xFD), 950: (0xA1, 0xF9)}
# Python の codec が Windows の表と違うバイト列の数。CP1255 の 0xCA（U+05BA）と CP936 の 0x80（€）を Python は
# 戻さず、cp950 は Windows の表にない 249 の 2 バイトを戻す
KNOWN = {1255: 1, 936: 1, 950: 249}
# 1 つの TEXT に入れる文字の数
CHUNK = 200
WINDOWS = hasattr(codecs, "code_page_decode")


def windows(page, data):
    """Windows の MultiByteToWideChar で戻した 1 文字。私用領域・C1 制御文字・戻せないものは None"""
    try:
        text, used = codecs.code_page_decode(page, data, "strict", True)
    except UnicodeDecodeError:
        return None
    if used != len(data) or len(text) != 1 or 0x80 <= ord(text) <= 0x9F or 0xE000 <= ord(text) <= 0xF8FF:
        return None
    return text


def python(page, data):
    """ezdxf が ANSI_<番号> に使う Python の codec で戻した 1 文字"""
    try:
        text = data.decode(toencoding(f"ANSI_{page}"))
    except UnicodeDecodeError:
        return None
    return text if len(text) == 1 else None


def reference(page, data):
    return windows(page, data) if WINDOWS else python(page, data)


def leads(page, byte):
    return page in DOUBLE and DOUBLE[page][0] <= byte <= DOUBLE[page][1]


def sequences(page):
    """1 バイトの列と、先頭のバイトの範囲の 2 バイトの列"""
    singles = [bytes([b]) for b in range(0x80, 0x100) if not leads(page, b)]
    if page not in DOUBLE:
        return singles
    low, high = DOUBLE[page]
    return singles + [bytes([lead, trail]) for lead in range(low, high + 1) for trail in range(0x40, 0x100)]


def fixture(page, chunks, layer):
    """R2000 の DXF。ezdxf で ASCII の印を書き、印を表のバイト列に置き換える"""
    # https://ezdxf.readthedocs.io/en/stable/options.html
    ezdxf.options.write_fixed_meta_data_for_testing = True
    doc = ezdxf.new("R2000")
    # 書く codec（encoding）と、ヘッダーのコード・ページの名前は別に置く。encoding がないと ANSI_1252 を書く
    doc.encoding = toencoding(f"ANSI_{page}")
    doc.header["$DWGCODEPAGE"] = f"ANSI_{page}"
    doc.layers.new("QALAYER")
    for index in range(len(chunks)):
        doc.modelspace().add_text(f"QACHUNK{index:04d}", dxfattribs={"layer": "QALAYER", "height": 2.5})
    stream = io.StringIO()
    doc.write(stream)
    data = stream.getvalue().encode("ascii")
    for index, chunk in enumerate(chunks):
        data = data.replace(f"QACHUNK{index:04d}\n".encode(), chunk + b"\n")
    return data.replace(b"QALAYER", layer)


def read_ezdxf(path):
    """ezdxf が $DWGCODEPAGE から選ぶ codec で読んだ TEXT の文字列と、その codec"""
    doc = ezdxf.readfile(path)
    return [e.dxf.text for e in doc.modelspace().query("TEXT")], doc.encoding


def zumen_texts(z, name):
    status, response = z.call({"op": "entities", "doc": name, "fields": ["id", "kind", "layer", "geometry"]})
    if status != "ok":
        raise RuntimeError(f"entities {name}: {status} {response}")
    return [(item.get("layer"), item.get("geometry", {}).get("content")) for item in response["result"]["items"]]


def add(layer_id, content):
    return {
        "kind": "add",
        "place": "space/0",
        "layer_id": layer_id,
        "geometry": {"shape": "text", "content": content, "position": [1, 2], "height": 2.5, "rotation": 0},
    }


def check(z, page, out):
    failures = []
    table = [(data, reference(page, data)) for data in sequences(page)]
    table = [(data, text) for data, text in table if text is not None]
    if WINDOWS:
        differ = [data.hex() for data in sequences(page) if python(page, data) != windows(page, data)]
        if len(differ) != KNOWN.get(page, 0):
            failures.append(f"Python の codec と Windows の表の違いが {len(differ)} ある（既知は {KNOWN.get(page, 0)}）: {differ[:10]}")
    groups = [table[start : start + CHUNK] for start in range(0, len(table), CHUNK)]
    chunks = [b"".join(data for data, _ in group) for group in groups]
    texts = ["".join(text for _, text in group) for group in groups]
    letters = [(data, text) for data, text in table if text.isalpha()][:3]
    layer = "L" + "".join(text for _, text in letters)
    data = fixture(page, chunks, b"L" + b"".join(data for data, _ in letters))
    source = out / f"cp{page}-input.dxf"
    source.write_bytes(data)
    name = f"cp{page}.dxf"
    z.push(name, data)
    status, response = z.call({"op": "open", "input": name})
    if status != "ok":
        return [f"開けない: {status} {response}"], len(table)
    expected = [(layer, text) for text in texts]
    read = zumen_texts(z, name)
    if read != expected:
        wrong = [i for i, (a, b) in enumerate(zip(read, expected)) if a != b]
        failures.append(f"zumen の読みが表と違う: {len(read)} 件中、違う TEXT {wrong[:5]}")
    # Python の codec で戻せる TEXT は、ezdxf の読みとも照らす
    plain = [i for i, group in enumerate(groups) if all(python(page, d) == t for d, t in group)]
    by_ezdxf, codec = read_ezdxf(source)
    if codec != toencoding(f"ANSI_{page}"):
        failures.append(f"ezdxf の codec が {toencoding(f'ANSI_{page}')} でない: {codec}")
    if [by_ezdxf[i] for i in plain] != [texts[i] for i in plain]:
        failures.append("ezdxf の読みが zumen と違う")
    # 書き戻し: 表のすべての文字を、TEXT ごとに逆の順で足す
    _, tables = z.call({"op": "tables", "doc": name})
    ids = [item["id"] for item in tables["result"]["layers"] if item.get("name") == layer]
    if len(ids) != 1:
        return failures + [f"zumen の表にレイヤ {layer!r} が 1 つない"], len(table)
    status, response = z.call({"op": "apply", "doc": name, "edits": [add(ids[0], text[::-1]) for text in texts]})
    if status != "ok":
        return failures + [f"書き戻せない: {status} {response}"], len(table)
    output = z.take(response["blobs"][0]["id"])
    target = out / f"cp{page}-output.dxf"
    target.write_bytes(output)
    for text in texts:
        # CP950 には同じ文字を表す 2 バイトが 2 つあるものがあり、Windows の WideCharToMultiByte が書く方と照らす
        if WINDOWS:
            encoded = codecs.code_page_encode(page, text[::-1], "strict")[0]
        else:
            encoded = text[::-1].encode(toencoding(f"ANSI_{page}"))
        if b"\n" + encoded + b"\n" not in output:
            failures.append(f"書き戻した TEXT が、Windows が書くバイト列を持たない: {text[:10]!r}")
            break
    reopened = f"cp{page}-output.dxf"
    z.push(reopened, output)
    z.call({"op": "open", "input": reopened})
    if zumen_texts(z, reopened) != expected + [(layer, text[::-1]) for text in texts]:
        failures.append("書き戻した DXF を開き直した読みが違う")
    by_ezdxf, _ = read_ezdxf(target)
    if [by_ezdxf[len(texts) + i] for i in plain] != [texts[i][::-1] for i in plain]:
        failures.append("書き戻した DXF の ezdxf の読みが違う")
    # 表せない文字は、変更全体を拒む
    status, _ = z.call({"op": "apply", "doc": name, "edits": [add(ids[0], "ภ" if page != 874 else "日")]})
    if status != "failed":
        failures.append(f"表せない文字を拒まない: {status}")
    # 割り当てのないすべての 1 バイトと 2 バイトを、1 つずつ TEXT に入れる。どれも dxf.lift.invalid で落とす
    defined = {d for d, _ in table}
    unassigned = [d for d in sequences(page) if d not in defined]
    if unassigned:
        bad = f"cp{page}-unassigned.dxf"
        bad_data = fixture(page, unassigned, b"L")
        (out / bad).write_bytes(bad_data)
        z.push(bad, bad_data)
        z.call({"op": "open", "input": bad})
        kept = zumen_texts(z, bad)
        _, drawing = z.call({"op": "drawing", "doc": bad})
        dropped = sum(
            d.get("count", 1) for d in (drawing or {}).get("diagnostics", []) if d.get("code") == "dxf.lift.invalid"
        )
        if kept or dropped != len(unassigned):
            failures.append(f"割り当てのない {len(unassigned)} のバイト列のうち、{len(kept)} を読み、{dropped} を dxf.lift.invalid で落とした")
    return failures, len(table)


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
        for page in SINGLE + tuple(DOUBLE):
            failures, count = check(z, page, out)
            results[f"ANSI_{page}"] = {"assigned": count, "failures": failures}
    finally:
        z.close()
    source = "Windows MultiByteToWideChar" if WINDOWS else "Python codec"
    with (out / "results.json").open("x", encoding="utf-8", newline="\n") as stream:
        record = {"ezdxf": ezdxf.__version__, "reference": source, "results": results}
        stream.write(json.dumps(record, ensure_ascii=False, indent=2) + "\n")
    for page, result in results.items():
        print(page, result["assigned"], "一致" if not result["failures"] else result["failures"])
    return 1 if any(result["failures"] for result in results.values()) else 0


if __name__ == "__main__":
    sys.exit(main())
