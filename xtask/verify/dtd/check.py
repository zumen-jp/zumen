"""電子納品の XML を、要領が配る DTD で lxml に検証させ、要素の木を TSV で返す（#505）。

標準入力の 1 つの XML を読む。文書型宣言のシステム識別子は、引数のフォルダーからの相対の名前として解く。
ネットワークからは取得しない。1 行目は `valid` か、`invalid` とタブと lxml の最初の誤り。valid なら、
`doctype`・根の名前・システム識別子の行と、要素ごとに `element`・深さ・名前・DTD の内容の種類・値の有無・
繰り返せるか・値の行を、文書の順に書く。内容の種類は lxml の `DTD.elements()` の `type`（`#PCDATA` は `mixed`）で、
宣言がなければ `-`。繰り返せるかは、親の内容モデルでその要素が単独で `*`・`+` を持つか、`*`・`+` を持つ選択（`|`）の
中にあれば `1`、ほかは `0`（記録の追加の照合に使う）。

値は zumen の読み（`zumen_delivery::Record`）と同じ規則で作る。直下の文字（子の要素・注記の後ろの文字を含む）を
つなげて前後の空白を除き、空でないか、子の要素がなければ値を持つ。
"""

import io
import pathlib
import sys


def escape(value):
    return (
        value.replace("\\", "\\\\")
        .replace("\t", "\\t")
        .replace("\n", "\\n")
        .replace("\r", "\\r")
    )


def repeatable(content, name, repeated=False):
    """内容モデルの木で、`name` の要素が単独で繰り返せるか。libxml2 は並び（`,`）と選択を 2 分木で持ち、
    組の `*`・`+` は組の初めの節に付く"""
    if content is None:
        return False
    many = content.occur in ("mult", "plus")
    if content.type == "element":
        return content.name == name and (many or repeated)
    if content.type == "seq":
        # 並びの中では、組の繰り返しは要素の単独の繰り返しにならない
        return repeatable(content.left, name) or repeatable(content.right, name)
    if content.type == "or":
        inner = many or repeated
        return repeatable(content.left, name, inner) or repeatable(content.right, name, inner)
    return False


def ledger_input(etree, path):
    """施設・機器台帳の DTD から、並びにある各要素を 1 つずつ持つ自作入力を組む。"""
    def children(content):
        if content.type == "seq":
            yield from children(content.left)
            yield from children(content.right)
        elif content.type == "element":
            yield content.name
        elif content.type != "pcdata":
            raise ValueError("施設・機器台帳の DTD の対象外の内容モデルです")

    dtd = etree.DTD(str(path))
    elements = {entry.name: list(children(entry.content)) for entry in dtd.iterelements()}
    root, = set(elements) - {name for names in elements.values() for name in names}

    def build(name, ancestors):
        if name in ancestors:
            raise ValueError("施設・機器台帳の要素が循環しています")
        entry = etree.Element(name)
        for child in elements[name]:
            entry.append(build(child, ancestors | {name}))
        if not elements[name]:
            entry.text = "入力"
        return entry

    return etree.tostring(
        build(root, set()),
        encoding="Shift_JIS",
        xml_declaration=True,
        doctype=f'<!DOCTYPE {root} SYSTEM "{path.name}">',
    )


def main():
    try:
        import lxml
        from lxml import etree
    except ImportError:
        sys.exit(77)
    if sys.argv[1:] == ["--check"]:
        print(f"lxml\t{lxml.__version__}")
        return
    if sys.argv[1:2] == ["--ledger-input"] and len(sys.argv) == 3:
        sys.stdout.buffer.write(ledger_input(etree, pathlib.Path(sys.argv[2])))
        return
    folder = pathlib.Path(sys.argv[1]).resolve()
    data = sys.stdin.buffer.read()
    parser = etree.XMLParser(
        dtd_validation=True,
        load_dtd=True,
        no_network=True,
        resolve_entities=False,
        huge_tree=False,
    )
    out = sys.stdout
    try:
        tree = etree.parse(io.BytesIO(data), parser, base_url=folder.as_uri() + "/")
    except etree.XMLSyntaxError as error:
        out.write(f"invalid\t{escape(str(error))}\n")
        return
    out.write("valid\n")
    info = tree.docinfo
    kinds = {}
    contents = {}
    for dtd in (info.internalDTD, info.externalDTD):
        if dtd is not None:
            for declared in dtd.elements():
                kinds.setdefault(declared.name, declared.type)
                contents.setdefault(declared.name, declared.content)
    out.write(f"doctype\t{escape(info.root_name or '')}\t{escape(info.system_url or '')}\n")

    def walk(element, depth, parent):
        text = element.text or ""
        elements = []
        for child in element:
            text += child.tail or ""
            if isinstance(child.tag, str):
                elements.append(child)
        text = text.strip()
        has = bool(text) or not elements
        kind = kinds.get(element.tag) or "-"
        repeat = parent is not None and repeatable(contents.get(parent), element.tag)
        out.write(
            f"element\t{depth}\t{escape(element.tag)}\t{kind}\t{int(has)}\t{int(repeat)}"
            f"\t{escape(text)}\n"
        )
        for child in elements:
            walk(child, depth + 1, element.tag)

    walk(tree.getroot(), 0, None)


if __name__ == "__main__":
    main()
