"""別に入れた IfcTester の公開 API だけを使う。実装ソースと XSD は読まない。"""

import importlib.metadata
import importlib.util
import json
from pathlib import Path
import re
import sys


def load_cases():
    spec = importlib.util.spec_from_file_location("ids_cases", Path(__file__).with_name("cases.py"))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def write_json(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n", encoding="utf-8", newline="\n")


def run(output):
    versions = {}
    for line in Path(__file__).with_name("requirements.txt").read_text().splitlines():
        name, wanted = line.split("==")
        versions[name] = importlib.metadata.version(name)
        if versions[name] != wanted:
            raise ValueError("tool.version")
    # 報告書と BCF のモジュールは呼ばず、対応する依存も導入しない。
    # https://docs.ifcopenshell.org/ifctester.html
    # https://docs.ifcopenshell.org/autoapi/ifctester/ids/index.html
    import ifcopenshell
    from ifctester import ids

    recipe = load_cases()
    document = ids.Ids(title=recipe.TITLE)
    for name, entity, value, cardinality, _, _ in recipe.CASES:
        specification = ids.Specification(name=name, ifcVersion=["IFC2X3"])
        specification.applicability.append(ids.Entity(name=entity))
        specification.requirements.append(ids.Attribute(name="Name", value=value, cardinality=cardinality))
        document.specifications.append(specification)
    utf8_path = output / "names-utf8.ids"
    if not document.to_xml(str(utf8_path)):
        raise ValueError("ids.write")
    utf8 = utf8_path.read_text(encoding="utf-8")
    model = ifcopenshell.open(output / "model.ifc")
    for label, codec, bom in recipe.ENCODINGS:
        declaration = "UTF-8" if label == "utf8" else "UTF-16"
        text, count = re.subn(r"encoding=['\"][^'\"]+['\"]", 'encoding="' + declaration + '"', utf8, count=1)
        if count != 1:
            raise ValueError("ids.encoding")
        path = output / ("names-" + label + ".ids")
        path.write_bytes(bom + text.encode(codec))
        observed = ids.open(str(path))
        observed.validate(model)
        values = []
        for item in observed.specifications:
            if type(item.status) is not bool:
                raise ValueError("ids.status")
            values.append({"name": item.name, "status": "passed" if item.status else "failed",
                           "applicable": sorted(e.GlobalId for e in item.applicable_entities),
                           "failed": sorted(e.GlobalId for e in item.failed_entities)})
        write_json(output / ("external-" + label + ".json"), {"schema": model.schema, "specifications": values})
    write_json(output / "external-tools.json", {"versions": versions, "complete": True})


if __name__ == "__main__":
    output = Path(sys.argv[1]).resolve(strict=True)
    try:
        run(output)
    except Exception as error:
        # 例外の本文とスタックは、外部実装のソース行や端末の道筋を含み得るため出さない。
        write_json(output / "external-error.json", {"errorType": type(error).__name__, "complete": False})
        raise SystemExit(2) from None
