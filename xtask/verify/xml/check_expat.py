"""受理済みの合成入力の公開値を、Python 3.14 標準の Expat と照らす。"""

import argparse
from pathlib import Path
import subprocess
import sys
from xml.parsers import expat


def string(value):
    return value.encode("utf-8").hex()


def expanded(name):
    if "\x1f" in name:
        namespace, local = name.rsplit("\x1f", 1)
        return string(local), string(namespace)
    return string(name), "-"


def coalesce(events):
    result = []
    for event in events:
        if (event[0] == "TEXT" and result and result[-1][0] == "TEXT"
                and event[1:3] == result[-1][1:3]):
            result[-1][-1] += event[-1]
        else:
            result.append(event.copy())
    return result


def independent_events(src):
    if src.startswith(b"\xff\xfe"):
        codec, bom = "utf-16-le", 2
    elif src.startswith(b"\xfe\xff"):
        codec, bom = "utf-16-be", 2
    elif src.startswith(b"<\0"):
        codec, bom = "utf-16-le", 0
    elif src.startswith(b"\0<"):
        codec, bom = "utf-16-be", 0
    else:
        codec, bom = "utf-8", 0
    text = src[bom:].decode(codec)
    offsets = [bom]
    for character in text:
        offsets.append(offsets[-1] + len(character.encode(codec)))
    characters = {offset: index for index, offset in enumerate(offsets)}

    def tag_end(offset):
        begin = characters[offset]
        quote = None
        for index in range(begin, len(text)):
            character = text[index]
            if quote:
                if character == quote:
                    quote = None
            elif character in "\"'":
                quote = character
            elif character == ">":
                return offsets[index + 1], text[begin:index + 1].endswith("/>")
        raise ValueError("タグの終わりがない")

    parser = expat.ParserCreate(namespace_separator="\x1f")
    parser.ordered_attributes = True
    events, stack = [], []
    cdata = False
    count = 0

    def start(name, attrs):
        nonlocal count
        index = count
        count += 1
        offset = parser.CurrentByteIndex
        opening_end, empty = tag_end(offset)
        local, namespace = expanded(name)
        event = ["START", str(index), str(stack[-1][0]) if stack else "-",
                 str(offset), "pending", local, namespace]
        events.append(event)
        stack.append((index, event, opening_end, empty))
        for position in range(0, len(attrs), 2):
            local, namespace = expanded(attrs[position])
            events.append(["ATTR", str(index), local, namespace, string(attrs[position + 1])])

    def end(_name):
        index, event, opening_end, empty = stack.pop()
        event[4] = str(opening_end if empty else tag_end(parser.CurrentByteIndex)[0])
        events.append(["END", str(index)])

    def character(value):
        if stack and value:
            events.append(["TEXT", str(stack[-1][0]), "cdata" if cdata else "text", string(value)])

    def cdata_start():
        nonlocal cdata
        cdata = True

    def cdata_end():
        nonlocal cdata
        cdata = False

    parser.StartElementHandler = start
    parser.EndElementHandler = end
    parser.CharacterDataHandler = character
    parser.StartCdataSectionHandler = cdata_start
    parser.EndCdataSectionHandler = cdata_end
    # 外部の DTD・実体を取得せず、ネットワークとファイルを参照しない。
    parser.SetParamEntityParsing(expat.XML_PARAM_ENTITY_PARSING_NEVER)
    parser.ExternalEntityRefHandler = lambda *_args: 1
    parser.Parse(src, True)
    return coalesce(events)


def check(content):
    name, src, expected = None, None, []
    count = 0
    for line in content.splitlines():
        fields = line.split("\t")
        if fields[0] == "CASE":
            name, src, expected = fields[1], bytes.fromhex(fields[2]), []
        elif fields[0] == "CASEEND":
            actual = independent_events(src)
            expected = coalesce(expected)
            if expected != actual:
                at = next((index for index, pair in enumerate(zip(expected, actual))
                           if pair[0] != pair[1]), min(len(expected), len(actual)))
                left = expected[at] if at < len(expected) else "missing"
                right = actual[at] if at < len(actual) else "missing"
                raise ValueError(f"{name}: event {at} が違う: {left!r} / {right!r}")
            count += 1
        else:
            expected.append(fields)
    if count == 0:
        raise ValueError("照合する入力がない")
    return count


def main():
    sys.stdout.reconfigure(encoding="utf-8")
    sys.stderr.reconfigure(encoding="utf-8")
    if sys.version_info[:2] != (3, 14):
        raise ValueError("Python 3.14 を指定する")
    parser = argparse.ArgumentParser(description=__doc__)
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument("--probe")
    group.add_argument("--input", type=Path)
    args = parser.parse_args()
    if args.probe:
        result = subprocess.run([args.probe, "--external"], capture_output=True, check=False)
        if result.returncode:
            raise ValueError(f"probe が失敗した: exit {result.returncode}")
        content = result.stdout.decode("ascii")
    else:
        content = args.input.read_text(encoding="ascii")
    count = check(content)
    print(f"XML: {expat.EXPAT_VERSION} と {count} 入力が一致")


if __name__ == "__main__":
    try:
        main()
    except (ValueError, OSError, expat.ExpatError) as error:
        # 受理条件の一致は主張しない。入力は、このテストで作る valid ケースだけ。
        detail = "入出力またはプロセスの起動に失敗した" if isinstance(error, OSError) else str(error)[:600]
        print(f"Expat 比較が失敗: {detail}", file=sys.stderr)
        raise SystemExit(1) from None
