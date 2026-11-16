"""公開 IFC の既知の値から定めた、名前の要件と期待する対象。"""

TITLE = "zumen IDS external comparison"
SOURCE = "ifc/text2bim/sample_model.ifc"
SOURCE_SHA256 = "f3e6b444dfc08bea7b1a8c74746ce4ad4752951fd2912df067f1bf65dff72797"
ENCODINGS = (("utf8", "utf-8", b""), ("utf16le", "utf-16le", b"\xff\xfe"), ("utf16be", "utf-16be", b"\xfe\xff"))
WALLS = [
    "13v5waIG0HxwZGA6irHSbP", "13v5wkIG0HxwZGA6irHSbP", "13v5wuIG0HxwZGA6irHSbP",
    "13v5xyIG0HxwZGA6irHSbP", "13v5y0IG0HxwZGA6irHSbP", "13v5y4IG0HxwZGA6irHSbP",
    "1HvF7tIG0HxwZGA6irHSbP",
]
STANDARD_WALLS = ["13v5x2IG0HxwZGA6irHSbP", "13v5y8IG0HxwZGA6irHSbP"]
ABSENT = "1HvF7tIG0HxwZGA6irHSbP"
# 名前、直接のクラス、値、必須か禁止か、対象の全 ID、不適合の全 ID。
CASES = [
    ("wall-name-required", "IFCWALL", None, "required", WALLS, [ABSENT]),
    ("wall-name-exact", "IFCWALL", "Exterior Concrete Wall", "required", WALLS, [ABSENT]),
    ("wall-name-wrong", "IFCWALL", "Unmatched verification name", "required", WALLS, WALLS),
    ("wall-name-prohibited", "IFCWALL", None, "prohibited", WALLS, [item for item in WALLS if item != ABSENT]),
    ("standard-name-required", "IFCWALLSTANDARDCASE", None, "required", STANDARD_WALLS, []),
    ("standard-name-wrong", "IFCWALLSTANDARDCASE", "Unmatched verification name", "required", STANDARD_WALLS, STANDARD_WALLS),
]


def external_expected():
    return {"schema": "IFC2X3", "specifications": [
        {"name": name, "status": "failed" if failed else "passed", "applicable": sorted(applicable), "failed": sorted(failed)}
        for name, _, _, _, applicable, failed in CASES
    ]}


def core_expected():
    return {"title": TITLE, "total": sum(len(failed) for *_, failed in CASES), "specifications": [
        {"name": name, "status": "failed" if failed else "passed", "applicable": len(applicable), "failed": len(failed), "unsupported": 0}
        for name, _, _, _, applicable, failed in CASES
    ], "findings": [
        {"specification": name, "diagnostic": {"code": "ids.requirement.invalid", "severity": "error", "count": 1,
         "at": [{"id": identifier}], "params": {"cardinality": cardinality, "facet": "attribute", "specification": name}}}
        for name, _, _, cardinality, _, failed in CASES for identifier in sorted(failed)
    ]}
