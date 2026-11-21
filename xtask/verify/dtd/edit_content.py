"""公開 API の合成例で、編集後の XML を libxml2 の DTD 検証と照らす。"""

import io
import json
import sys
from pathlib import Path

from lxml import etree

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from make import ROOT, Zumen


def main(exe):
    dtd = etree.DTD(io.StringIO(
        "<!ELEMENT root (group,note*)>\n<!ELEMENT group (item*)>\n"
        "<!ELEMENT item EMPTY>\n<!ELEMENT note (#PCDATA)>"
    ))
    case = ROOT / "bindings/conformance/cases/delivery_edit_content.json"
    z = Zumen(exe)
    counts = {"matched": 0, "violated": 0, "unchecked": 0}
    try:
        for step in json.loads(case.read_text(encoding="utf-8"))["steps"]:
            if "push" in step:
                source = step["push"]
                z.push(source["name"], source["text"].encode())
            elif "call" in step:
                status, reply = z.call(step["call"])
                assert status == "ok" and not reply["diagnostics"]
                result = reply["result"]
                assert result["applied"] == 1
                check, = result["validation"]["checks"]
                data = z.take(0)
                valid = dtd.validate(etree.fromstring(data))
                actual = check["status"]
                expected = step["expect"]["/result/validation/checks/0/status"]
                assert actual == expected
                if actual != "unchecked":
                    assert valid == (actual == "matched")
                assert b"<!--keep-->" in data
                element = etree.fromstring(data[check["start"]:check["end"]])
                edit = step["call"]["edits"][0]
                assert element.tag == (edit["path"][0]["name"] if edit["path"] else "root")
                counts[actual] += 1
    finally:
        z.close()
    assert counts == {"matched": 2, "violated": 2, "unchecked": 2}
    print(counts)


if __name__ == "__main__":
    main(sys.argv[1])
