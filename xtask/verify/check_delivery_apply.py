"""自作の管理 XML の書き戻しを Python の文字コード変換と Expat で照らす。"""

from pathlib import Path
import sys
import xml.etree.ElementTree as ET
from xml.parsers import expat


def verify(directory: Path) -> None:
    encodings = {
        "utf8": "utf-8",
        "utf8-bom": "utf-8-sig",
        "sjis": "cp932",
        "utf16le": "utf-16",
        "utf16be": "utf-16",
        "utf16le-no-bom": "utf-16-le",
        "utf16be-no-bom": "utf-16-be",
    }
    for kind, encoding in encodings.items():
        before = (directory / f"{kind}-before.xml").read_bytes()
        after = (directory / f"{kind}-after.xml").read_bytes()
        original = before.decode(encoding)
        actual = after.decode(encoding)
        newline = "\n" if kind == "utf8-bom" else "\r" if kind == "sjis" else "\r\n"
        expected = original.replace(
            "old<![CDATA[ tail]]>", f"更新&lt;&amp;&gt;{newline}&#13;値"
        ).replace("MISSING.P21", "TWO.P21").replace("<empty />", "<empty >filled</empty>")
        expected = expected.replace(
            "</package>",
            f"  <p:extra>新<child>value</child></p:extra>{newline}"
            f"  <entry><file>THREE.P21</file></entry>{newline}</package>",
        )
        assert actual == expected, kind
        # バイト順・BOM と、指定した変更箇所以外のバイト列も独立に照らす。
        codec = "utf-16-le" if kind == "utf16le" else "utf-16-be" if kind == "utf16be" else encoding
        prefix = b"\xff\xfe" if kind == "utf16le" else b"\xfe\xff" if kind == "utf16be" else b""
        assert after == prefix + expected.encode(codec), kind
        # Expat は Shift_JIS の複数バイト変換を持たないので、Python の cp932 で先に復号する。
        # 外部の DTD を取得するハンドラーは登録しない。
        parser = ET.XMLParser(target=ET.TreeBuilder(insert_comments=True, insert_pis=True))
        root = ET.fromstring(actual, parser=parser)
        assert root.tag == "package" and root.attrib == {"version": "1"}, kind
        title = root.find("title")
        assert title is not None and title.attrib == {"code": "x"}, kind
        assert title.text == "更新<&>\n\r値", kind
        assert len(title) == 1 and title[0].tag is ET.ProcessingInstruction, kind
        assert title[0].text == "keep x", kind
        assert [element.text for element in root.findall("entry/file")] == ["ONE.P21", "TWO.P21", "THREE.P21"], kind
        assert [element.text for element in root.findall("entry/note")] == ["A", "B"], kind
        extra = root.find("{urn:test}extra")
        assert extra is not None and extra.text == "新" and extra.findtext("child") == "value", kind
        assert root.findtext("empty") == "filled", kind
        assert any(element.tag is ET.Comment and element.text == " keep " for element in root), kind
    print(f"管理 XML {len(encodings)} 組: バイト列・名前空間・文字・参照が一致（Expat {expat.EXPAT_VERSION}）")


if __name__ == "__main__":
    verify(Path(sys.argv[1]))
