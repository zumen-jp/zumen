"""#316 の合成図面から、操作した値と対照の生の DXF タグだけを取り出す。"""

import argparse
import hashlib
import json
import re
import sys
from pathlib import Path

MISSING = None
try:
    import ezdxf
    from ezdxf.lldxf.const import DXFStructureError
    from ezdxf.lldxf.loader import load_dxf_structure
    from ezdxf.lldxf.tagger import ascii_tags_loader
except ImportError as error:
    # 流し方の誤り（3）として、main が理由を付けて返す。元の文はパスを含み得るので、モジュールの名前だけを使う。
    ezdxf, MISSING = None, f"{error.name or 'ezdxf'} を読み込めない"


EZDXF = "1.4.5"
NUMBER = r"([+-]?(?:\d+(?:\.\d*)?|\.\d+)(?:[eE][+-]?\d+)?|\*)"
STYLES = {"REGULAR": 0, "BOLD": 2, "ITALIC": 1, "BOTH": 3}
TEXTS = ("CONTROL", "RGB_A", "RGB_B", "PARA_25", "PARA_50")
# DXF の線内のコード、現行の DWG リーダーが対応づける 93 のビット、画面で指定する値。
LEADER = {"leader-type": (170, 1, 2), "leader-color": (92, 2, 0xC2112233),
          "leader-linetype": (340, 4, "Z316_DASH"), "leader-weight": (171, 8, 50),
          "leader-size": (40, 16, 4.5), "leader-arrow": (341, 32, "_DOT")}
# JSON にそのまま出す線種と部品の名前。ほかの名前は出すときだけ伏せ、比べるのは伏せる前の名前にする。
NAMES = ("Z316_DASH", "_DOT", "BYBLOCK", "BYLAYER", "CONTINUOUS")


class Usage(Exception):
    """流し方の誤り（ezdxf・ファイル・保存の形式）。終了コード 3 にし、記録せずに直して流し直す。"""


def one(items, message):
    items = list(items)
    if len(items) != 1:
        raise ValueError(message)
    return items[0]


def inline(text, marker):
    """各 MTEXT の既知の文字の直前までを見る。閉じた書式の組とエスケープされた符号を除く。"""
    if text.count(marker) != 1:
        raise ValueError("MTEXT の目印の文字が一つでない")
    color, paragraphs, stack = None, [], []
    for match in re.finditer(r"\\(?:[\\{}]|([cCp])([^;\\{}]*);|.)|[{}]", text[:text.index(marker)]):
        kind, value = match.groups()
        if match[0] == "{":
            stack.append((color, paragraphs.copy()))
        elif match[0] == "}":
            if not stack:
                raise ValueError("MTEXT の書式の組が対応しない")
            color, paragraphs = stack.pop()
        elif kind == "c":
            if not re.fullmatch(r"[0-9]{1,10}", value):
                raise ValueError("色の引数が整数でない")
            color = int(value)
        elif kind == "C":
            color = None
        elif kind == "p":
            # 引数はそのまま記録に残す。このキットで読む範囲かは compare で調べる。
            if not re.fullmatch(r"[A-Za-z0-9+*,.\-]{0,160}", value):
                raise ValueError("段落書式の引数に、英数字と + * , . - のほかの文字がある")
            paragraphs.append(value)
    return {"c": color, "p": paragraphs}


def style_values(record):
    app = None
    families, flags = [], []
    for code, value in record:
        if code == 1001:
            app = value
        elif app == "ACAD" and code == 1000:
            families.append(value)
        elif app == "ACAD" and code == 1071:
            flags.append(int(value) & 0xFFFFFFFF)
    return {"Arial": families == ["Arial"], "1071": one(flags, "STYLE の ACAD 1071 が複数ある") if flags else None}


def name_of(refs, raw):
    # 保存時に変わり得るハンドルは参照先の名前で比べる。解決できないハンドルは None。名前は hidden が出す前に伏せる。
    return refs.get(raw.strip().upper())


def field_values(tags, refs):
    result = {}
    for code, raw in tags:
        if code in (340, 341):
            result[code] = name_of(refs, raw)
        elif code == 40:
            result[code] = float(raw)
        elif code in (93, 92, 170, 171):
            result[code] = int(raw) & 0xFFFFFFFF if code in (93, 92) else int(raw)
    return result


def leader_values(record, refs):
    lines = {}
    current = None
    shared = []
    context = False
    for code, value in record:
        if (code, value) == (300, "CONTEXT_DATA{"):
            context = True
        elif context and (code, value) == (304, "LEADER_LINE{"):
            if current is not None:
                raise ValueError("線の開始が重複")
            current = []
        elif context and (code, value) == (305, "}") and current is not None:
            # 先頭の頂点（矢の先）で枝を識別する。線や枝の index の採番には依存しない。
            x = next(float(v) for c, v in current if c == 10)
            y = next(float(v) for c, v in current if c == 20)
            if y != 0 or x not in (-30.0, 30.0) or x in lines:
                raise ValueError("対照とする左右の矢の先が違う")
            lines[x] = field_values(current, refs)
            current = None
        elif current is not None:
            current.append((code, value))
        elif (code, value) == (301, "}"):
            context = False
        elif not context and code in (6, 62, 370, 420, 90, 91, 170, 171, 341, 342, 42):
            # 図形全体の色・スタイルの上書きを、線の 92・93 と混同しない。
            if code in (341, 342):
                value = name_of(refs, value)
            elif code != 6:
                value = float(value) if code == 42 else int(value)
            shared.append((code, value))
    if set(lines) != {-30.0, 30.0} or current is not None:
        raise ValueError("同じ引出線に左と右の枝が一つずつ必要")
    return {"left": lines[-30.0], "right": lines[30.0], "shared": shared}


def read(path, topic=None):
    if path.stat().st_size > 8 * 1024 * 1024:
        raise Usage("このキットの 8 MiB までの合成 DXF を指定してください")
    try:
        with path.open(encoding="utf-8-sig") as stream:
            sections = load_dxf_structure(ascii_tags_loader(stream))
    except (UnicodeDecodeError, DXFStructureError):
        raise Usage("DXF として読めない。R2018 の ASCII DXF を指定してください") from None
    header = sections.get("HEADER", [[]])[0]
    if not any(a == (9, "$ACADVER") and b == (1, "AC1032") for a, b in zip(header, header[1:])):
        raise Usage("R2018 の ASCII DXF を指定してください")
    tables, entities = sections["TABLES"], sections["ENTITIES"]
    refs = {r.get_first_value(5, "").upper(): r.get_first_value(2, "").upper() for r in tables
            if r.dxftype() in ("BLOCK_RECORD", "LTYPE")}
    styles = {}
    for key in STYLES if topic in (None, "styles") else ():
        r = one((r for r in tables if r.dxftype() == "STYLE" and r.get_first_value(2) == "Z316_" + key),
                "検査対象の STYLE が一つでない")
        styles[key] = style_values(r)
    texts = {}
    keys = {"color": ("CONTROL", "RGB_A", "RGB_B"), "paragraph": ("CONTROL", "PARA_25", "PARA_50")}
    for key in TEXTS if topic is None else keys.get(topic, ("CONTROL",)):
        r = one((r for r in entities if r.dxftype() == "MTEXT" and r.get_first_value(8) == "Z316_" + key),
                "検査対象の MTEXT が一つでない")
        texts[key] = {**inline("".join(v for c, v in r if c in (1, 3)), "Z316_" + key),
                      "height": float(r.get_first_value(40)), "color": int(r.get_first_value(62, "256")),
                      "true_color": int(r.get_first_value(420, "-1"))}
    leader = None
    if topic is None or topic in LEADER:
        r = one((r for r in entities if r.dxftype() in ("MLEADER", "MULTILEADER") and r.get_first_value(8) == "Z316_LEADER"),
                "検査対象の MULTILEADER が一つでない")
        leader = leader_values(r, refs)
    return {"styles": styles, "text": texts, "leader": leader}


def compare(before, after, topic):
    if before["text"]["CONTROL"] != after["text"]["CONTROL"]:
        raise ValueError("対照の MTEXT の書式が変わった")
    if topic == "styles":
        old, new = before["styles"], after["styles"]
        if not all(s["Arial"] for s in [*old.values(), *new.values()]):
            raise ValueError("4 書体をすべて Arial にする")
        plain = old["REGULAR"]["1071"]
        if plain is None or plain & (3 << 24) or any(s["1071"] != plain for s in old.values()):
            raise ValueError("変更前の 4 書体の 1071 が同じ標準書体を表していない")
        values = {k: s["1071"] for k, s in new.items()}
        return {"before": plain, "after": values, "matches": all(v == plain | STYLES[k] << 24 for k, v in values.items())}
    if topic == "color":
        keys = ("RGB_A", "RGB_B")
        if any(before["text"][key]["c"] is not None for key in keys):
            raise ValueError("変更前に既に色の符号がある")
        if any(before["text"][k][p] != after["text"][k][p] for k in keys for p in ("color", "true_color", "height", "p")):
            raise ValueError("MTEXT の図形全体の色・高さか段落も変わった")
        values = [after["text"][k]["c"] for k in keys]
        order = {(0x332211, 0x112233): "BGR", (0x112233, 0x332211): "RGB"}.get(tuple(values))
        return {"values": values, "high_to_low_bytes": order, "matches": order is not None}
    if topic == "paragraph":
        result = {}
        left = []
        for key, height in (("PARA_25", 2.5), ("PARA_50", 5.0)):
            old, new = before["text"][key], after["text"][key]
            if old["height"] != height or new["height"] != height:
                raise ValueError("段落用の MTEXT の高さが違う")
            if not all(re.fullmatch(r"[xilrqtcdjeE0-9+*,.\-]*", raw) for raw in old["p"] + new["p"]):
                raise ValueError("このキットで観察する段落書式の範囲外")
            old_first, old_left, old_align = paragraph_values(old["p"])
            if old_first not in (None, 0.0) or old_left not in (None, 0.0) or old_align == "c":
                raise ValueError("変更前に段落のインデントか中央揃えが設定されている")
            first, indent, align = paragraph_values(new["p"])
            if indent is None and first is not None:
                raise ValueError("1 行目のインデントだけが変わった。README の手順で、1 行目とぶら下げの両方を設定する")
            left.append(indent)
            result[key] = {"before": old["p"], "after": new["p"], "height": height, "first_line": first,
                           "left": indent, "alignment": align}
        units = {(2.0, 1.0): "text_height", (5.0, 5.0): "drawing_units"}.get(tuple(left))
        return {"paragraphs": result, "left_units": units, "matches": units is not None and
                all(r["before"] != r["after"] and r["alignment"] == "c" and r["first_line"] in (None, 0.0)
                    for r in result.values())}
    code, bit, expected = LEADER[topic]
    old, new = before["leader"], after["leader"]
    if old["shared"] != new["shared"] or old["right"] != new["right"]:
        raise ValueError("MULTILEADER 共通部か、対照にした右の枝も変わった")
    if old["left"].get(93, 0) != 0 or old["right"].get(93, 0) != 0:
        raise ValueError("変更前に線の上書きがある")
    fields = new["left"]
    others = (set(old["left"]) | set(fields)) - {code, 93}
    matches = (fields.get(93, 0) == bit and fields.get(code) == expected and
               fields.get(code) != old["left"].get(code) and all(fields.get(k) == old["left"].get(k) for k in others))
    return {"before": old["left"], "after": fields, "expected_code": code, "expected_flag": bit, "matches": matches}


def paragraph_values(raw):
    """`\\p` の 1 行目（`l` からの差の `i`）と、ほかの行（`l`）のインデントと、揃え（`q`）。"""
    args = ",".join(raw)

    def last(letter):
        values = re.findall(letter + NUMBER, args)
        return float(values[-1]) if values and values[-1] != "*" else None

    align = re.findall(r"q([lcrjd*])", args)
    return last("i"), last("l"), align[-1] if align else None


def hidden(result, topic):
    """JSON に出す前に、線種と部品の名前のうち、このキットの名前のほかを伏せる。同じ名前には同じ番号を付ける。"""
    labels = {}

    def name(value):
        if value is None or value.upper() in NAMES:
            return value
        return labels.setdefault(value.upper(), f"other-{len(labels) + 1}")

    def line(fields):
        return {code: name(value) if code in (340, 341) else value for code, value in fields.items()}

    for values in result["observed"].values():
        leader = values["leader"]
        if leader is not None:
            values["leader"] = {"left": line(leader["left"]), "right": line(leader["right"]),
                                "shared": [(code, name(value) if code in (6, 341, 342) else value)
                                           for code, value in leader["shared"]]}
    if topic in LEADER and "after" in result:
        result["before"], result["after"] = line(result["before"]), line(result["after"])


def sha256(path):
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("before", type=Path)
    parser.add_argument("after", type=Path)
    parser.add_argument("topic", choices=["styles", "color", "paragraph", *LEADER])
    args = parser.parse_args()
    # 終了コード 0 は予想どおり、1 は予想と違う、2 は比較できない、3 は流し方の誤り。どれも JSON を出し、2 と 3 には
    # 理由を添える。理由には、渡したパスを書かず、変更前か変更後かだけを書く。
    result = {"topic": args.topic, "ezdxf": getattr(ezdxf, "__version__", None)}
    observed, status = {}, None
    try:
        if ezdxf is None or ezdxf.__version__ != EZDXF:
            raise Usage(f"ezdxf {EZDXF} を入れた Python で流してください（{MISSING or result['ezdxf']}）")
        for name, label, path in (("before", "変更前", args.before), ("after", "変更後", args.after)):
            try:
                observed[name] = read(path, args.topic)
                result[name + "_sha256"] = sha256(path)
            except OSError as error:
                raise Usage(f"ファイルを読めない（{label}、{error.strerror or type(error).__name__}）") from None
            except Usage as error:
                raise Usage(f"{error}（{label}）") from None
        result.update(compare(observed["before"], observed["after"], args.topic))
    except Usage as error:
        result["error"], status = f"流し方の誤り: {error}", 3
    except Exception as error:  # 比較できない失敗を、予想と違う（1）と取り違えないよう 2 にする。
        result["error"], status = f"比較できません: {error}", 2
    result["observed"] = observed
    hidden(result, args.topic)
    # 理由は日本語なので、ファイルに書き出しても端末の文字コードによらず UTF-8 にする。
    sys.stdout.reconfigure(encoding="utf-8")
    print(json.dumps(result, ensure_ascii=False, indent=2))
    return status or (0 if result["matches"] else 1)


if __name__ == "__main__":
    raise SystemExit(main())
