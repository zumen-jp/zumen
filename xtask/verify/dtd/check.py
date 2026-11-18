"""電子納品の XML を、要領が配る DTD で lxml に検証させ、要素の木を TSV で返す（#505）。

標準入力の 1 つの XML を読む。文書型宣言のシステム識別子は、引数のフォルダーからの相対の名前として解く。
ネットワークからは取得しない。1 行目は `valid` か、`invalid` とタブと lxml の最初の誤り。valid なら、
`doctype`・根の名前・システム識別子の行と、要素ごとに `element`・深さ・名前・DTD の内容の種類・値の有無・値の行を、
文書の順に書く。内容の種類は lxml の `DTD.elements()` の `type`（`#PCDATA` は `mixed`）で、宣言がなければ `-`。

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


def main():
    try:
        import lxml
        from lxml import etree
    except ImportError:
        sys.exit(77)
    if sys.argv[1:] == ["--check"]:
        print(f"lxml\t{lxml.__version__}")
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
    for dtd in (info.internalDTD, info.externalDTD):
        if dtd is not None:
            for declared in dtd.elements():
                kinds.setdefault(declared.name, declared.type)
    out.write(f"doctype\t{escape(info.root_name or '')}\t{escape(info.system_url or '')}\n")

    def walk(element, depth):
        text = element.text or ""
        elements = []
        for child in element:
            text += child.tail or ""
            if isinstance(child.tag, str):
                elements.append(child)
        text = text.strip()
        has = bool(text) or not elements
        kind = kinds.get(element.tag) or "-"
        out.write(
            f"element\t{depth}\t{escape(element.tag)}\t{kind}\t{int(has)}\t{escape(text)}\n"
        )
        for child in elements:
            walk(child, depth + 1)

    walk(tree.getroot(), 0)


if __name__ == "__main__":
    main()
