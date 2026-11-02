"""building_apply で書いた IFC を IfcOpenShell で開き、変更が見えることを確かめる（#482）。

使い方: python check.py <フォルダー>

フォルダーには、`edit.rs` の `writes_the_edited_files_for_other_readers` が、環境変数 `ZUMEN_IFC_EDITED` の
フォルダーに書いた `ifc4.ifc`・`ifc4.edited.ifc`・`ifc2x3.ifc`・`ifc2x3.edited.ifc` がある。

- 変えた値・足したセットとプロパティ・名前と説明が、IfcOpenShell で読める。
- IfcOpenShell の検査（スキーマと EXPRESS の規則）で、変える前のファイルにない誤りが出ない。
"""

import sys
from pathlib import Path

import ifcopenshell
import ifcopenshell.util.element
import ifcopenshell.validate


def gid(n):
    return f"{n:0>22}"


# GlobalId の番号 → セット → プロパティ → 値
PSETS = {
    32: {"Pset_DoorCommon": {"FireRating": "EI90", "HandicapAccessible": True}},
    30: {"Pset_WallCommon": {"FireRating": "EI120", "IsExternal": False}},
    31: {"Pset_WallCommon": {"FireRating": "EI30", "IsExternal": False}},
    33: {"Pset_SlabCommon": {"Reference": "S1", "Thickness": 200.0}},
    34: {"Pset_DoorTypeCommon": {"Reference": "D2"}, "Pset_Extra": {"Note": "備考"}},
}

# GlobalId の番号 → (Name, Description)
NAMES = {
    30: ("壁 'A' \\ 🧱", "説明"),
    31: (None, None),
    32: ("Door", None),
}


def errors(model):
    logger = ifcopenshell.validate.json_logger()
    ifcopenshell.validate.validate(model, logger, express_rules=True)
    # 規則の誤りの文は、評価した値の行を続けるので、最初の行で比べる
    return {
        (s.get("attribute"), str(s.get("message")).split("\n", 1)[0])
        for s in logger.statements
        if s.get("level") == "error"
    }


def check(folder, schema):
    failures = []
    before = ifcopenshell.open(str(folder / f"{schema}.ifc"))
    after = ifcopenshell.open(str(folder / f"{schema}.edited.ifc"))
    if after.schema != before.schema:
        failures.append(f"スキーマが {before.schema} から {after.schema} に変わった")
    for n, sets in PSETS.items():
        element = after.by_guid(gid(n))
        found = ifcopenshell.util.element.get_psets(element)
        for set_name, properties in sets.items():
            for name, value in properties.items():
                got = found.get(set_name, {}).get(name)
                if got != value:
                    failures.append(f"#{n} の {set_name}.{name} が {got!r}（期待は {value!r}）")
    for n, (name, description) in NAMES.items():
        element = after.by_guid(gid(n))
        if (element.Name, element.Description) != (name, description):
            failures.append(f"#{n} の名前と説明が {(element.Name, element.Description)!r}")
    added = sorted(errors(after) - errors(before), key=str)
    for attribute, message in added:
        failures.append(f"検査の誤りが増えた: {attribute}: {message}")
    return failures


def main():
    if len(sys.argv) != 2:
        print("使い方: python check.py <フォルダー>", file=sys.stderr)
        return 2
    folder = Path(sys.argv[1])
    failures = []
    for schema in ("ifc4", "ifc2x3"):
        failures += [f"{schema}: {f}" for f in check(folder, schema)]
    if failures:
        print("\n".join(failures), file=sys.stderr)
        return 1
    print(f"IfcOpenShell {ifcopenshell.version} で、IFC4 と IFC2X3 の変更が見えた")
    return 0


if __name__ == "__main__":
    sys.exit(main())
