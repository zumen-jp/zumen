"""国内積算用 IDS の固定原本と自作 IFC2X3 で、対象と不適合を比較する。"""

import argparse
import base64
import hashlib
import importlib.metadata
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import threading
import urllib.request
import xml.etree.ElementTree as ET

HERE = Path(__file__).resolve().parent
DIGEST = "26c98dccc6812bc941bacde073c457b0821bc765c44419761dc38b0c7fa2ca9c"
VARIANTS = ("pass", "missing", "other")
IDS = "http://standards.buildingsmart.org/IDS"
XSD = "http://www.w3.org/2001/XMLSchema"
XSI = "http://www.w3.org/2001/XMLSchema-instance"


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def ifc_text(value):
    return "'\\X2\\" + value.encode("utf-16-be").hex().upper() + "\\X0\\'"


def fixture(specs, selected):
    records = [
        "#1=IFCPROJECT('0000000000000000000001',#2,'Synthetic',$,$,$,$,(#8),#7);",
        "#2=IFCOWNERHISTORY(#3,#4,$,.ADDED.,$,$,$,0);",
        "#3=IFCPERSONANDORGANIZATION(#5,#6,$);",
        "#4=IFCAPPLICATION(#6,'1','Test','Test');",
        "#5=IFCPERSON($,'Test',$,$,$,$,$,$);",
        "#6=IFCORGANIZATION($,'Test',$,$,$);",
        "#7=IFCUNITASSIGNMENT((#11));",
        "#8=IFCGEOMETRICREPRESENTATIONCONTEXT($,'Model',3,0.001,#9,$);",
        "#9=IFCAXIS2PLACEMENT3D(#10,$,$);",
        "#10=IFCCARTESIANPOINT((0.,0.,0.));",
        "#11=IFCSIUNIT(*,.LENGTHUNIT.,$,.METRE.);",
    ]
    for index in selected:
        number, variant = divmod(index, len(VARIANTS))
        spec = specs[number]
        base = 100 * (index + 1)
        obj_type = spec.findtext("./{*}applicability/{*}attribute/{*}value/{*}simpleValue")
        if VARIANTS[variant] == "other":
            obj_type = "Other"
        records.append(f"#{base}=IFCBUILDINGELEMENTPROXY('{base:022}',#2,'Test',$,{ifc_text(obj_type)},$,$,$,.ELEMENT.);")
        app = spec.find("./{*}applicability/{*}property")
        set_name = app.findtext("./{*}propertySet/{*}simpleValue")
        props = [app, *spec.findall("./{*}requirements/{*}property")]
        refs = []
        for i, prop in enumerate(props):
            name = prop.findtext("./{*}baseName/{*}simpleValue")
            if i == 0:
                value = "IFCREAL(1.)"
            elif i == len(props) - 1 and VARIANTS[variant] == "missing":
                value = "$"
            else:
                literal = prop.findtext("./{*}value/{*}simpleValue")
                if literal is None:
                    literal = prop.find("./{*}value/{*}restriction/{*}enumeration").attrib["value"]
                value = f"IFCTEXT({ifc_text(literal)})"
            ref = f"#{base + 10 + i}"
            refs.append(ref)
            records.append(f"{ref}=IFCPROPERTYSINGLEVALUE({ifc_text(name)},$,{value},$);")
        records.append(f"#{base+1}=IFCPROPERTYSET('{base+1:022}',#2,{ifc_text(set_name)},$,({','.join(refs)}));")
        records.append(f"#{base+2}=IFCRELDEFINESBYPROPERTIES('{base+2:022}',#2,$,$,(#{base}),#{base+1});")
    return ("ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION(('Synthetic'),'2;1');\n"
            "FILE_NAME('','',(''),(''),'','','');\nFILE_SCHEMA(('IFC2X3'));\nENDSEC;\nDATA;\n"
            + "\n".join(records) + "\nENDSEC;\nEND-ISO-10303-21;\n").encode("ascii")


def qualified(data):
    """原本と区別し、名前空間を補った参考入力だけをメモリに作る。"""
    root = ET.fromstring(data)
    for prefix, name in (("", IDS), ("xs", XSD), ("xsi", XSI)):
        ET.register_namespace(prefix, name)
    for node in root.iter():
        uri = XSD if node.tag in ("restriction", "enumeration") else IDS
        node.tag = f"{{{uri}}}{node.tag}"
    root.set(f"{{{XSI}}}schemaLocation", root.attrib.pop("schemaLocation"))
    return ET.tostring(root, encoding="utf-8")


def run(args):
    import ifcopenshell
    from ifctester import ids

    for line in (HERE / "requirements.txt").read_text().splitlines():
        package, version = line.split("==")
        if importlib.metadata.version(package) != version:
            raise ValueError("tool.version")
    data = args.ids.read_bytes()
    if len(data) != 15808 or hashlib.sha256(data).hexdigest() != DIGEST:
        raise ValueError("ids.sha256")
    specs = ET.fromstring(data).findall(".//{*}specification")
    if len(specs) != 3 or any(spec.get("ifcVersion") != "IFC2X3" for spec in specs):
        raise ValueError("ids.specifications")
    try:
        ids.open(str(args.ids))
    except ids.IdsXmlValidationError:
        errors = list(ids.get_schema().iter_errors(data))
        if len(errors) != 1 or errors[0].path != "/ids":
            raise ValueError("external.root")
    else:
        raise ValueError("external.original")
    supplemental = qualified(data)
    ids.get_schema().validate(supplemental)
    schema = load("cost_express", HERE.parent / "express/check.py")
    name, path, length, digest = schema.SOURCES[0]
    with urllib.request.urlopen(schema.BASE + path, timeout=30) as receipt:
        express = receipt.read(length + 1)
    if name != "IFC2X3" or len(express) != length or schema.sha(express) != digest:
        raise ValueError("express.sha256")
    cli = load("cost_transport", HERE.parent / "make.py").Zumen(args.cli)
    timeout = threading.Timer(180, cli.child.kill)
    timeout.start()
    rows = range(len(specs) * len(VARIANTS))
    models = [list(rows)] + [[i] for i in rows]
    steps = []
    hashes = []

    def push(name, value):
        cli.push(name, value)
        steps.append({"push": name, "base64": base64.b64encode(value).decode("ascii")})

    def call(request, expected):
        request = {"v": 1, **request}
        steps.append({"call": request})
        reply = cli.rpc("call", {"request": request})
        if reply["status"] != expected:
            raise ValueError("core.status")
        raw = reply["response"].encode("utf-8")
        hashes.append([expected, hashlib.sha256(raw).hexdigest()])
        return json.loads(raw)

    try:
        push("original.ids", data)
        push("qualified.ids", supplemental)
        push("input.exp", express)
        for i, selected in enumerate(models):
            model = fixture(specs, selected)
            checker = ids.from_string(supplemental.decode())
            checker.validate(ifcopenshell.file.from_string(model.decode()))
            doc = f"case-{i}.ifc"
            push(doc, model)
            call({"op": "open", "input": doc}, "ok")
            request = {"op": "ids_check", "doc": doc, "express": "input.exp"}
            original = call({**request, "ids": "original.ids"}, "failed")
            diag, = original["diagnostics"]
            if "result" in original or diag["code"] != "ids.file.invalid" or diag["params"] != {"near": "ids"}:
                raise ValueError("core.original")
            reply = call({**request, "ids": "qualified.ids"}, "ok")
            if "next_offset" in reply["result"]:
                raise ValueError("core.check")
            core = reply["result"]
            if len(checker.specifications) != len(specs) or len(core["specifications"]) != len(specs):
                raise ValueError("specifications.count")
            for j, (external, actual) in enumerate(zip(checker.specifications, core["specifications"])):
                expected = [f"{100*(k+1):022}" for k in selected if k // 3 == j and k % 3 != 2]
                failed = [f"{100*(k+1):022}" for k in selected if k // 3 == j and k % 3 == 1]
                observed = sorted(e.GlobalId for e in external.applicable_entities)
                invalid = sorted(e.GlobalId for e in external.failed_entities)
                findings = [f["diagnostic"] for f in core["findings"] if f["specification"] == actual["name"]]
                actual_ids = sorted(p["id"] for f in findings for p in f["at"])
                wanted_status = "failed" if failed else "passed"
                matches = (external.name == actual["name"] == specs[j].get("name")
                           and external.status == (not failed) and actual["status"] == wanted_status
                           and observed == expected and invalid == failed and actual_ids == failed
                           and actual["applicable"] == len(expected) and actual["failed"] == len(failed)
                           and actual["unsupported"] == 0
                           and all(f["code"] == "ids.requirement.invalid" and f["params"]["facet"] == "property" for f in findings))
                if not matches:
                    print(json.dumps(dict(model=i, spec=j, expected=expected, failed=failed, external=observed,
                                          external_failed=invalid, core=actual, findings=actual_ids), ensure_ascii=True))
                    return 1
        node = subprocess.run(["node", str(HERE / "cost_wasm.mjs"), str(args.wasm)],
                              input=json.dumps(steps).encode(), stdout=subprocess.PIPE, check=True, timeout=180)
        if hashes != json.loads(node.stdout):
            raise ValueError("wasm.responses")
        print("original: IfcTester IdsXmlValidationError, ids_check ids.file.invalid")
        print(f"qualified: {len(specs)} specifications, {len(models)} models, 6 applicable, 3 failed in the combined model")
        print(f"native/wasm: {len(hashes)} matching statuses and response hashes")
        return 0
    finally:
        try:
            cli.close()
        finally:
            timeout.cancel()


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--ids", required=True, type=Path)
    parser.add_argument("--cli", required=True, type=Path)
    parser.add_argument("--wasm", required=True, type=Path)
    try:
        sys.exit(run(parser.parse_args()))
    except Exception as error:
        # 外部実装の例外本文には入力の内容や端末の道筋が含まれることがある。
        print(type(error).__name__, file=sys.stderr)
        sys.exit(2)
