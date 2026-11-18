"""R2007 より前の DXF の Windows のコード・ページの文字を、Windows の API・Python の codec・ezdxf に照らす（#327）。

$DWGCODEPAGE の名前（ANSI_<番号>・DOS<番号>と、AutoCAD 2027 が Windows のコード・ページとして読んだ ISO8859-1 などの
別名）ごとに、割り当てのあるすべてのバイト列を分けて持つ TEXT と、表の文字を名前に含むレイヤの R2000 の
DXF を作り、zumen の読み・書き戻し・開き直しを照らす。割り当ては、Windows では MultiByteToWideChar（Python の
codecs.code_page_decode）が私用領域と C1 制御文字でない文字に戻すバイト列、ほかの OS では Python の codec が戻す
バイト列とする。Python の codec と Windows の表の違いは、既知の数と照らす。割り当てのないバイト列を持つ TEXT は、
zumen が dxf.lift.invalid で落とすことを照らす。

MTEXT の `\\M+<番号><16 進 4 桁>` は、番号ごとにすべての 2 バイトを MTEXT に入れ、割り当てのあるものはシーンの
文字列が基準と同じこと、ないものは文字を描かないことを照らす。基準を ezdxf の decode_mif_to_unicode とも比べる。
"""

import argparse
import codecs
import io
import json
import re
from pathlib import Path
import sys

import ezdxf
from ezdxf.lldxf.encoding import decode_mif_to_unicode
from ezdxf.tools.codepage import toencoding

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))
from make import Zumen  # noqa: E402

SINGLE = (874, 1250, 1251, 1252, 1253, 1254, 1255, 1256, 1257, 1258)
# DOS の 1 バイトのコード・ページ。$DWGCODEPAGE の名前は DOS<番号>
DOS = (437, 850, 852, 855, 857, 860, 861, 863, 864, 865, 866, 869)
# ANSI_ と DOS のほかに、AutoCAD 2027 が Windows のコード・ページとして読んだ名前（#327 の実験）
ALIASES = {"ISO8859-1": 1252, "ISO8859-2": 1250, "ISO8859-7": 1253, "ISO8859-9": 1254, "BIG5": 950, "KSC5601": 949, "GB2312": 936, "JOHAB": 1361}
# 2 バイトのコード・ページと、先頭のバイトの範囲（Unicode の MICSFT/WINDOWS/CP<番号>.TXT）
DOUBLE = {936: (0x81, 0xFE), 949: (0x81, 0xFD), 950: (0xA1, 0xF9), 1361: (0x84, 0xF9)}
# 2 バイト目の最小。Johab の記号と漢字は 0x31 から
TRAIL = {1361: 0x31}
# Python の codec が Windows の表と違うバイト列の数。CP1255 の 0xCA（U+05BA）と CP936 の 0x80（€）を Python は
# 戻さず、cp950 は Windows の表にない 249 の 2 バイトを戻す。johab（cp1361）は、字母だけの 110 の 2 バイトを互換の字母
# （U+3131〜）に戻し（Windows は結合用の字母 U+1100〜）、Windows が戻さない 0x8441 を U+3000 に戻す
KNOWN = {1255: 1, 936: 1, 950: 249, 1361: 111}
# MTEXT の `\M+` の番号とコード・ページ（ezdxf の MIF_CODE_PAGE）
MULTIBYTE = {"1": 932, "2": 950, "3": 949, "4": 1361, "5": 936}
# ezdxf の decode_mif_to_unicode が Windows の表と違う 2 バイトの数。Python の cp950 が Windows の表にない 249 を戻し、
# johab は上の 111 が違う
KNOWN_MULTIBYTE = {950: 249, 1361: 111}
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


def codec(page):
    """コード・ページの番号の Python の codec の名前"""
    return codecs.lookup(f"cp{page}").name


def python(page, data):
    """Python の codec の cp<番号> で戻した 1 文字"""
    try:
        text = data.decode(codec(page))
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
    return singles + [bytes([lead, trail]) for lead in range(low, high + 1) for trail in range(TRAIL.get(page, 0x40), 0x100)]


def fixture(name, page, chunks, layer):
    """$DWGCODEPAGE が `name` の R2000 の DXF。ezdxf で ASCII の印を書き、印を表のバイト列に置き換える"""
    # https://ezdxf.readthedocs.io/en/stable/options.html
    ezdxf.options.write_fixed_meta_data_for_testing = True
    doc = ezdxf.new("R2000")
    # ezdxf は $DWGCODEPAGE を encoding から書く（DOS437 や ISO8859-1 は ANSI_1252 になる）ので、書いた後で置き換える
    doc.encoding = codec(page)
    doc.layers.new("QALAYER")
    for index in range(len(chunks)):
        doc.modelspace().add_text(f"QACHUNK{index:04d}", dxfattribs={"layer": "QALAYER", "height": 2.5})
    stream = io.StringIO()
    doc.write(stream)
    header = rb"(\n\$DWGCODEPAGE\n *3\n)[^\n]*\n"
    data = re.sub(header, lambda m: m[1] + name.encode() + b"\n", stream.getvalue().encode("ascii"))
    if data.count(b"\n" + name.encode() + b"\n") != 1:
        raise RuntimeError(f"$DWGCODEPAGE を {name} にできない")
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


def check(z, name, page, out):
    failures = []
    stem = name.lower()
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
    data = fixture(name, page, chunks, b"L" + b"".join(data for data, _ in letters))
    source = out / f"{stem}-input.dxf"
    source.write_bytes(data)
    doc = f"{stem}.dxf"
    z.push(doc, data)
    status, response = z.call({"op": "open", "input": doc})
    if status != "ok":
        return [f"開けない: {status} {response}"], len(table)
    expected = [(layer, text) for text in texts]
    read = zumen_texts(z, doc)
    if read != expected:
        wrong = [i for i, (a, b) in enumerate(zip(read, expected)) if a != b]
        failures.append(f"zumen の読みが表と違う: {len(read)} 件中、違う TEXT {wrong[:5]}")
    # Python の codec で戻せる TEXT は、ezdxf の読みとも照らす。ezdxf が名前から同じ codec を選ばないもの（DOS・ISO8859
    # など）は ezdxf と照らさない
    plain = [i for i, group in enumerate(groups) if all(python(page, d) == t for d, t in group)]
    same = codecs.lookup(toencoding(name)).name == codec(page)
    if not same:
        plain = []
    by_ezdxf, chosen = read_ezdxf(source)
    if same and codecs.lookup(chosen).name != codec(page):
        failures.append(f"ezdxf の codec が {codec(page)} でない: {chosen}")
    if [by_ezdxf[i] for i in plain] != [texts[i] for i in plain]:
        failures.append("ezdxf の読みが zumen と違う")
    # 書き戻し: 表のすべての文字を、TEXT ごとに逆の順で足す
    _, tables = z.call({"op": "tables", "doc": doc})
    ids = [item["id"] for item in tables["result"]["layers"] if item.get("name") == layer]
    if len(ids) != 1:
        return failures + [f"zumen の表にレイヤ {layer!r} が 1 つない"], len(table)
    status, response = z.call({"op": "apply", "doc": doc, "edits": [add(ids[0], text[::-1]) for text in texts]})
    if status != "ok":
        return failures + [f"書き戻せない: {status} {response}"], len(table)
    output = z.take(response["blobs"][0]["id"])
    target = out / f"{stem}-output.dxf"
    target.write_bytes(output)
    for text in texts:
        # CP950 には同じ文字を表す 2 バイトが 2 つあるものがあり、Windows の WideCharToMultiByte が書く方と照らす
        if WINDOWS:
            encoded = codecs.code_page_encode(page, text[::-1], "strict")[0]
        else:
            encoded = text[::-1].encode(codec(page))
        if b"\n" + encoded + b"\n" not in output:
            failures.append(f"書き戻した TEXT が、Windows が書くバイト列を持たない: {text[:10]!r}")
            break
    reopened = f"{stem}-output.dxf"
    z.push(reopened, output)
    z.call({"op": "open", "input": reopened})
    if zumen_texts(z, reopened) != expected + [(layer, text[::-1]) for text in texts]:
        failures.append("書き戻した DXF を開き直した読みが違う")
    by_ezdxf, _ = read_ezdxf(target)
    if [by_ezdxf[len(texts) + i] for i in plain] != [texts[i][::-1] for i in plain]:
        failures.append("書き戻した DXF の ezdxf の読みが違う")
    # 表せない文字は、変更全体を拒む
    status, _ = z.call({"op": "apply", "doc": doc, "edits": [add(ids[0], "ภ" if page != 874 else "日")]})
    if status != "failed":
        failures.append(f"表せない文字を拒まない: {status}")
    # 割り当てのないすべての 1 バイトと 2 バイトを、1 つずつ TEXT に入れる。どれも dxf.lift.invalid で落とす
    defined = {d for d, _ in table}
    unassigned = [d for d in sequences(page) if d not in defined]
    if unassigned:
        bad = f"{stem}-unassigned.dxf"
        bad_data = fixture(name, page, unassigned, b"L")
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


def multibyte(page, data):
    """MTEXT の `\\M+` の 2 バイトの基準の 1 文字。ANSI_932 は私用領域（外字）も戻す"""
    if page != 932:
        return reference(page, data)
    if WINDOWS:
        try:
            text, used = codecs.code_page_decode(page, data, "strict", True)
        except UnicodeDecodeError:
            return None
    else:
        try:
            text, used = data.decode("cp932"), len(data)
        except UnicodeDecodeError:
            return None
    return text if used == len(data) and len(text) == 1 and not 0x80 <= ord(text) <= 0x9F else None


def mtext_fixture(contents):
    """R2000 の ANSI_1252 の DXF。MTEXT ごとに `contents` の 1 つを持つ"""
    ezdxf.options.write_fixed_meta_data_for_testing = True
    doc = ezdxf.new("R2000")
    doc.header["$DWGCODEPAGE"] = "ANSI_1252"
    for index, content in enumerate(contents):
        doc.modelspace().add_mtext(content, dxfattribs={"char_height": 2.5, "insert": (0, index * 10)})
    stream = io.StringIO()
    doc.write(stream)
    return stream.getvalue().encode("ascii")


def scene(z, name, data):
    """文書 `name` を開いたシーンの結果と blob"""
    z.push(name, data)
    status, response = z.call({"op": "open", "input": name})
    if status != "ok":
        raise RuntimeError(f"open {name}: {status} {response}")
    status, response = z.call({"op": "scene", "doc": name, "from": "space/0"})
    if status != "ok":
        raise RuntimeError(f"scene {name}: {status} {response}")
    return response["result"], z.take(response["blobs"][0]["id"])


def check_multibyte(z, number, page, out):
    """MTEXT の `\\M+<番号><16 進 4 桁>` の、すべての 2 バイトを照らす。割り当てのあるものは文字にし、ないものは描かない"""
    failures = []
    codes = [bytes([high, low]) for high in range(0x100) for low in range(0x100)]
    table = [(data, multibyte(page, data)) for data in codes]
    assigned = [(data, text) for data, text in table if text is not None]
    unassigned = [data for data, text in table if text is None]
    # ezdxf の decode_mif_to_unicode と照らし、Python の codec が Windows の表と違う数は既知の数と照らす
    by_ezdxf = [(data, decode_mif_to_unicode(f"\\M+{number}{data.hex().upper()}")) for data in codes]
    differ = [
        data.hex()
        for data, text in by_ezdxf
        if (text if not text.startswith("\\M+") and len(text) == 1 else None) != multibyte(page, data)
    ]
    if WINDOWS and len(differ) != KNOWN_MULTIBYTE.get(page, 0):
        failures.append(f"ezdxf と Windows の表の違いが {len(differ)} ある（既知は {KNOWN_MULTIBYTE.get(page, 0)}）: {differ[:10]}")
    groups = [assigned[start : start + CHUNK] for start in range(0, len(assigned), CHUNK)]
    contents = ["".join(f"\\M+{number}{data.hex().upper()}" for data, _ in group) for group in groups]
    data = mtext_fixture(contents)
    (out / f"mif{number}-input.dxf").write_bytes(data)
    result, blob = scene(z, f"mif{number}.dxf", data)
    if result["counts"]["texts"] != len(groups):
        failures.append(f"シーンの文字が {result['counts']['texts']} ある（MTEXT は {len(groups)}）")
    missing = [i for i, group in enumerate(groups) if "".join(t for _, t in group).encode() not in blob]
    if missing:
        failures.append(f"シーンに基準の文字列がない MTEXT: {missing[:5]}")
    groups = [unassigned[start : start + CHUNK] for start in range(0, len(unassigned), CHUNK)]
    contents = ["".join(f"\\M+{number}{data.hex().upper()}" for data in group) for group in groups]
    data = mtext_fixture(contents)
    (out / f"mif{number}-unassigned.dxf").write_bytes(data)
    result, _ = scene(z, f"mif{number}-unassigned.dxf", data)
    if result["counts"]["texts"] != 0:
        failures.append(f"割り当てのない {len(unassigned)} の 2 バイトから、文字を {result['counts']['texts']} 描いた")
    return failures, len(assigned)


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
        names = [(f"ANSI_{page}", page) for page in SINGLE + tuple(DOUBLE)]
        names += [(f"DOS{page}", page) for page in DOS] + list(ALIASES.items())
        for name, page in names:
            failures, count = check(z, name, page, out)
            results[name] = {"assigned": count, "failures": failures}
        for number, page in MULTIBYTE.items():
            failures, count = check_multibyte(z, number, page, out)
            results[f"\\M+{number}"] = {"assigned": count, "failures": failures}
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
