"""合成の土木図面の Rust/API の lower を ezdxf で照合する（#643）。"""

import sys
from pathlib import Path

from ezdxf import recover


def attributes(entity, *, text=False):
    values = entity.dxf.all_existing_dxf_attribs()
    if text and entity.dxftype() == "TEXT":
        for key in ("align_point", "halign", "valign"):
            values.pop(key, None)
    return entity.dxftype(), values


def check(folder):
    documents = []
    for name in ("api.dxf", "rust.dxf"):
        document, auditor = recover.readfile(folder / name)
        assert not auditor.errors and not auditor.fixes
        assert document.dxfversion == "AC1018"
        documents.append(document)
    api, rust = documents
    assert list(api.layouts.names()) == list(rust.layouts.names())
    assert [attributes(layer) for layer in api.layers] == [
        attributes(layer) for layer in rust.layers
    ]
    assert [attributes(line_type) for line_type in api.linetypes] == [
        attributes(line_type) for line_type in rust.linetypes
    ]
    aligned = 0
    elements = 0
    for name in api.layouts.names():
        first, second = api.layouts.get(name), rust.layouts.get(name)
        assert attributes(first.dxf_layout) == attributes(second.dxf_layout)
        assert [attributes(entity, text=True) for entity in first] == [
            attributes(entity, text=True) for entity in second
        ]
        for left, right in zip(first, second, strict=True):
            elements += 1
            if left.dxftype() == "TEXT" and attributes(left) != attributes(right):
                assert left.dxf.halign == 0 and left.dxf.valign == 0
                aligned += 1
    assert elements > 0 and aligned > 0
    print(f"elements={elements}, text_alignment_differences={aligned}, audit_errors=0, audit_fixes=0")


if __name__ == "__main__":
    check(Path(sys.argv[1]))
