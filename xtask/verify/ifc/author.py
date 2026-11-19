"""新規作成（create）と意図編集（building_apply）で作った 3 版の 2 階建て IFC を BIM ソフトで確かめるキットを作る（#898・#526）。"""

import argparse
import hashlib
import importlib.util
import json
import math
import shutil
import sys
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
MAX_INPUT = 64 * 1024 * 1024
VERSIONS = {
    "ifc2x3": "IFC2X3",
    "ifc4": "IFC4",
    "ifc4x3": "IFC4X3_ADD2",
}
HEADER = {
    "name": "authored.ifc",
    "time_stamp": "2026-01-01T00:00:00",
    "author": ["Person"],
    "organization": ["Organization"],
    "originating_system": "zumen",
}
OWNER = {
    "person": "Person",
    "organization": "Organization",
    "developer": "zumen",
    "application": "zumen",
    "version": "0.3.0",
    "identifier": "zumen",
    "created": 1767225600,
}
GIDS = {
    "site": "0000000000000000000010",
    "building": "0000000000000000001000",
    "storey_1f": "0000000000000000001001",
    "storey_2f": "0000000000000000001002",
    "grid": "0000000000000000001003",
    "wall_type": "0000000000000000001010",
    "wall_1f": "0000000000000000002000",
    "opening_1f": "0000000000000000002020",
    "door_1f": "0000000000000000002021",
    "wall_2f": "0000000000000000003000",
    "opening_2f": "0000000000000000003020",
    "window_2f": "0000000000000000003021",
}


def json_bytes(value):
    return (json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True, allow_nan=False) + "\n").encode("utf-8")


def write_json(path, value):
    path.write_bytes(json_bytes(value))


def sha(data):
    return hashlib.sha256(data).hexdigest()


def output_folder(path):
    path = path.resolve()
    if any((parent / ".git").exists() for parent in [path, *path.parents]):
        raise ValueError("出力先は Git の作業ツリーの外にしてください")
    if path.exists() and (not path.is_dir() or any(path.iterdir())):
        raise ValueError("出力先は新しいフォルダーか空のフォルダーにしてください")
    path.mkdir(parents=True, exist_ok=True)
    return path


def read_input(path):
    if path.stat().st_size > MAX_INPUT:
        raise ValueError("入力は 64 MiB 以下にしてください")
    with path.open("rb") as source:
        data = source.read(MAX_INPUT + 1)
    if len(data) > MAX_INPUT:
        raise ValueError("入力は 64 MiB 以下にしてください")
    return data


def client(exe):
    spec = importlib.util.spec_from_file_location("verify_make", ROOT / "xtask/verify/make.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module.Zumen(exe)


def close_client(z):
    try:
        z.close()
    finally:
        if z.child.stdout is not None:
            z.child.stdout.close()


def call(z, request, records):
    status, response = z.call(request)
    records.append({"request": request, "status": status, "response": response})
    if status != "ok":
        codes = [item["code"] for item in (response or {}).get("diagnostics", [])]
        raise ValueError("API の失敗: " + request["op"] + " " + ",".join(codes))
    return response


def diagnostic_counts(response):
    counts = Counter()
    for item in response["diagnostics"]:
        counts[item["code"]] += item["count"]
    return dict(counts)


def identity_matrix(z=0.0):
    return [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, z]]


def build_edits(project_ref):
    edits = [
        {
            "kind": "add_space",
            "space": "site",
            "parent": project_ref,
            "name": "Site",
            "placement": {"matrix": identity_matrix(0.0)},
            "global_id": GIDS["site"],
        },
        {
            "kind": "add_space",
            "space": "building",
            "parent": {"added": 0},
            "name": "Building",
            "placement": {"relative_to": {"added": 0}, "matrix": identity_matrix(0.0)},
            "global_id": GIDS["building"],
        },
        {
            "kind": "add_space",
            "space": "storey",
            "parent": {"added": 1},
            "name": "1F",
            "elevation": 0.0,
            "placement": {"relative_to": {"added": 1}, "matrix": identity_matrix(0.0)},
            "global_id": GIDS["storey_1f"],
        },
        {
            "kind": "add_space",
            "space": "storey",
            "parent": {"added": 1},
            "name": "2F",
            "elevation": 3000.0,
            "placement": {"relative_to": {"added": 1}, "matrix": identity_matrix(3000.0)},
            "global_id": GIDS["storey_2f"],
        },
        {
            "kind": "add_grid",
            "storey": {"added": 2},
            "name": "G1",
            "u": [
                {"tag": "A", "start": [0.0, -1000.0], "end": [0.0, 5000.0]},
                {"tag": "B", "start": [6000.0, -1000.0], "end": [6000.0, 5000.0]},
            ],
            "v": [
                {"tag": "1", "start": [-1000.0, 0.0], "end": [7000.0, 0.0]},
                {"tag": "2", "start": [-1000.0, 4000.0], "end": [7000.0, 4000.0]},
            ],
            "global_id": GIDS["grid"],
        },
        {
            "kind": "add_type",
            "class": "IfcWallType",
            "predefined": "SHEAR",
            "name": "WT-200",
            "global_id": GIDS["wall_type"],
        },
        {
            "kind": "material",
            "element": {"added": 5},
            "material": {
                "kind": "layers",
                "layers": [
                    {"material": "Concrete", "thickness": 150.0},
                    {"material": "Insulation", "thickness": 50.0},
                ],
            },
        },
    ]
    corners = [[0.0, 0.0], [6000.0, 0.0], [6000.0, 4000.0], [0.0, 4000.0]]
    for level, storey_idx in [(1, 2), (2, 3)]:
        base = 1000 * (level + 1)
        first_wall_idx = len(edits)
        for i, start in enumerate(corners):
            end = corners[(i + 1) % len(corners)]
            gid = f"{base + i:022d}"
            edits.append(
                {
                    "kind": "add_wall",
                    "storey": {"added": storey_idx},
                    "start": start,
                    "end": end,
                    "height": 2800.0,
                    "element_type": {"added": 5},
                    "name": f"W{base + i}",
                    "global_id": gid,
                }
            )
        edits.extend(
            [
                {
                    "kind": "add_slab",
                    "storey": {"added": storey_idx},
                    "outline": corners,
                    "layers": [{"material": "Concrete", "thickness": 200.0}],
                    "name": f"S{base + 10}",
                    "global_id": f"{base + 10:022d}",
                },
                {
                    "kind": "add_column",
                    "storey": {"added": storey_idx},
                    "position": [0.0, 0.0],
                    "height": 2800.0,
                    "section": {"width": 400.0, "depth": 400.0},
                    "direction": [0.0, 1.0],
                    "material": "Concrete",
                    "name": f"C{base + 11}",
                    "global_id": f"{base + 11:022d}",
                },
                {
                    "kind": "add_beam",
                    "storey": {"added": storey_idx},
                    "start": [0.0, 4000.0, 2800.0],
                    "end": [6000.0, 4000.0, 2800.0],
                    "section": {"width": 300.0, "depth": 600.0},
                    "material": "Steel",
                    "name": f"B{base + 12}",
                    "global_id": f"{base + 12:022d}",
                },
            ]
        )
        opening_idx = len(edits)
        if level == 1:
            edits.append(
                {
                    "kind": "add_opening",
                    "host": {"added": first_wall_idx},
                    "distance": 1000.0,
                    "sill": 0.0,
                    "width": 900.0,
                    "height": 2100.0,
                    "name": "O2020",
                    "global_id": GIDS["opening_1f"],
                }
            )
            edits.append(
                {
                    "kind": "add_door",
                    "opening": {"added": opening_idx},
                    "width": 900.0,
                    "height": 2100.0,
                    "name": "D2021",
                    "global_id": GIDS["door_1f"],
                }
            )
        else:
            edits.append(
                {
                    "kind": "add_opening",
                    "host": {"added": first_wall_idx},
                    "distance": 1000.0,
                    "sill": 900.0,
                    "width": 1200.0,
                    "height": 1200.0,
                    "name": "O3020",
                    "global_id": GIDS["opening_2f"],
                }
            )
            edits.append(
                {
                    "kind": "add_window",
                    "opening": {"added": opening_idx},
                    "width": 1200.0,
                    "height": 1200.0,
                    "name": "WIN3021",
                    "global_id": GIDS["window_2f"],
                }
            )
    return edits


def read_snapshot(z, doc_name, records):
    summary = call(z, {"op": "building", "doc": doc_name}, records)["result"]
    elements = call(
        z,
        {
            "op": "building_elements",
            "doc": doc_name,
            "limit": 1000,
            "components": ["identity", "class", "parent", "placement", "opening", "type_of", "material"],
        },
        records,
    )["result"]["elements"]
    scene = call(z, {"op": "scene3d", "doc": doc_name, "tolerance": 1.0}, records)
    for blob in scene.get("blobs", []):
        z.take(blob["id"])
    joinery_scene = call(
        z,
        {"op": "scene3d", "doc": doc_name, "tolerance": 1.0, "classes": ["IfcDoor", "IfcWindow"]},
        records,
    )
    for blob in joinery_scene.get("blobs", []):
        z.take(blob["id"])
    return {
        "schema": summary["schema"],
        "hash": summary["hash"],
        "units": summary["units"],
        "elements": elements,
        "scene3d_instances": scene["result"]["counts"]["instances"],
        "joinery_scene3d_instances": joinery_scene["result"]["counts"]["instances"],
    }


def expected_state(version, state):
    changed = state == "changed"
    return {
        "schema": VERSIONS[version],
        "storey_2f_z": 3200.0 if changed else 3000.0,
        "window_2f_z": 4200.0 if changed else 3900.0,
        "wall_1f_layers": [200.0, 50.0, 20.0] if changed else [150.0, 50.0],
        "joinery_scene3d_instances": 2,
    }


def verify_snapshot(snapshot, expected):
    failures = []
    if snapshot["schema"] != expected["schema"]:
        failures.append("schema")
    if snapshot.get("joinery_scene3d_instances", 0) != expected["joinery_scene3d_instances"]:
        failures.append("joinery_body_missing")
    by_id = {elem["id"]: elem for elem in snapshot["elements"]}
    for key, gid in GIDS.items():
        if gid not in by_id:
            failures.append(f"missing:{key}")
    storey_2f = by_id.get(GIDS["storey_2f"], {})
    world_2f = (storey_2f.get("placement") or {}).get("world")
    if not world_2f or not math.isclose(world_2f[2][3], expected["storey_2f_z"], abs_tol=1e-6):
        failures.append("storey_2f_z")
    win_2f = by_id.get(GIDS["window_2f"], {})
    world_win = (win_2f.get("placement") or {}).get("world")
    if not world_win or not math.isclose(world_win[2][3], expected["window_2f_z"], abs_tol=1e-6):
        failures.append("window_2f_z")
    wall_1f = by_id.get(GIDS["wall_1f"], {})
    parts = (wall_1f.get("material") or {}).get("parts") or []
    thicknesses = [part.get("thickness", {}).get("value") for part in parts]
    if thicknesses != expected["wall_1f_layers"]:
        failures.append("wall_1f_layers")
    return failures


def compare_observed(reference, observed):
    diffs = []
    if reference["schema"] != observed["schema"]:
        diffs.append("schema")
    ref_by_id = {e["id"]: e for e in reference["elements"] if len(e["id"]) == 22}
    obs_by_id = {e["id"]: e for e in observed["elements"] if len(e["id"]) == 22}
    if set(ref_by_id) != set(obs_by_id):
        diffs.append("global_ids")
    for gid, ref_elem in ref_by_id.items():
        obs_elem = obs_by_id.get(gid)
        if not obs_elem:
            continue
        if (ref_elem.get("class") or {}).get("name") != (obs_elem.get("class") or {}).get("name"):
            diffs.append(f"class:{gid}")
        ref_world = (ref_elem.get("placement") or {}).get("world")
        obs_world = (obs_elem.get("placement") or {}).get("world")
        if ref_world and obs_world:
            for r_row, o_row in zip(ref_world, obs_world):
                if any(not math.isclose(a, b, abs_tol=1e-3) for a, b in zip(r_row, o_row)):
                    diffs.append(f"placement:{gid}")
                    break
    return sorted(set(diffs))


REPORT_TEMPLATE = """# 新規作成 2 階建て IFC の実機確認様式（#898・#526）

- 実施日:
- 確認した BIM ソフトとバージョン:

| 版 | ファイル | 開くときの誤り・警告 | 2F 階高・配置 | 1F 壁厚・開口・扉 Body | 2F 窓位置・窓 Body | 再書き出し比較 |
| --- | --- | --- | --- | --- | --- | --- |
| IFC2X3 | `ifc2x3/built.ifc` | | 3,000 mm | 200 mm / 900×2,100 | Z=3,900 / 1,200×1,200 | |
| IFC2X3 | `ifc2x3/changed.ifc` | | 3,200 mm | 270 mm / 900×2,100 | Z=4,200 / 1,600×1,000 | |
| IFC4 | `ifc4/built.ifc` | | 3,000 mm | 200 mm / 900×2,100 | Z=3,900 / 1,200×1,200 | |
| IFC4 | `ifc4/changed.ifc` | | 3,200 mm | 270 mm / 900×2,100 | Z=4,200 / 1,600×1,000 | |
| IFC4X3 | `ifc4x3/built.ifc` | | 3,000 mm | 200 mm / 900×2,100 | Z=3,900 / 1,200×1,200 | |
| IFC4X3 | `ifc4x3/changed.ifc` | | 3,200 mm | 270 mm / 900×2,100 | Z=4,200 / 1,600×1,000 | |
"""


def make(exe, output):
    out = output_folder(output)
    records, cases = [], {}
    z = client(exe)
    try:
        capabilities = call(z, {"op": "capabilities"}, records)["result"]
        for version, schema_name in VERSIONS.items():
            doc = f"{version}_built.ifc"
            created = call(
                z,
                {
                    "op": "create",
                    "format": "ifc",
                    "to": doc,
                    "unit": "millimeter",
                    "ifc": {"version": version, "header": HEADER, "owner": OWNER},
                },
                records,
            )
            for blob in created.get("blobs", []):
                z.take(blob["id"])
            proj = call(
                z,
                {"op": "building_elements", "doc": doc, "class": "ifcproject"},
                records,
            )["result"]["elements"][0]
            project_ref = {"id": proj["id"], "hash": proj["hash"]}
            applied_built = call(
                z,
                {"op": "building_apply", "doc": doc, "edits": build_edits(project_ref)},
                records,
            )
            if applied_built["diagnostics"]:
                raise ValueError(f"{version} built の診断: {applied_built['diagnostics']}")
            built_bytes = z.take(next(b["id"] for b in applied_built["blobs"] if b["kind"] == "file"))
            built_snap = read_snapshot(z, doc, records)
            built_exp = expected_state(version, "built")
            built_failures = verify_snapshot(built_snap, built_exp)
            if built_failures:
                raise ValueError(f"{version} built の検証失敗: {built_failures}")

            by_id = {e["id"]: e for e in built_snap["elements"]}
            change_doc = f"{version}_changed.ifc"
            z.push(change_doc, built_bytes)
            call(z, {"op": "open", "input": change_doc}, records)
            change_edits = [
                {
                    "kind": "elevation",
                    "storey": {"id": GIDS["storey_2f"], "hash": by_id[GIDS["storey_2f"]]["hash"]},
                    "elevation": 3200.0,
                },
                {
                    "kind": "wall_shape",
                    "wall": {"id": GIDS["wall_1f"], "hash": by_id[GIDS["wall_1f"]]["hash"]},
                    "start": [0.0, 0.0],
                    "end": [6500.0, 0.0],
                    "height": 3000.0,
                    "layers": [
                        {"material": "Concrete", "thickness": 200.0},
                        {"material": "Insulation", "thickness": 50.0},
                        {"material": "Board", "thickness": 20.0},
                    ],
                },
                {
                    "kind": "opening_shape",
                    "opening": {"id": GIDS["opening_2f"], "hash": by_id[GIDS["opening_2f"]]["hash"]},
                    "distance": 2000.0,
                    "sill": 1000.0,
                    "width": 1600.0,
                    "height": 1000.0,
                },
            ]
            applied_changed = call(
                z,
                {"op": "building_apply", "doc": change_doc, "edits": change_edits},
                records,
            )
            if applied_changed["diagnostics"]:
                raise ValueError(f"{version} changed の診断: {applied_changed['diagnostics']}")
            changed_bytes = z.take(next(b["id"] for b in applied_changed["blobs"] if b["kind"] == "file"))
            changed_snap = read_snapshot(z, change_doc, records)
            changed_exp = expected_state(version, "changed")
            changed_failures = verify_snapshot(changed_snap, changed_exp)
            if changed_failures:
                raise ValueError(f"{version} changed の検証失敗: {changed_failures}")

            folder = out / version
            folder.mkdir()
            (folder / "built.ifc").write_bytes(built_bytes)
            (folder / "changed.ifc").write_bytes(changed_bytes)
            write_json(folder / "built.json", built_snap)
            write_json(folder / "changed.json", changed_snap)
            write_json(folder / "expected.json", {"built": built_exp, "changed": changed_exp})
            cases[version] = {
                "schema": schema_name,
                "built_sha256": sha(built_bytes),
                "built_bytes": len(built_bytes),
                "changed_sha256": sha(changed_bytes),
                "changed_bytes": len(changed_bytes),
                "elements": len(built_snap["elements"]),
                "scene3d_instances": built_snap["scene3d_instances"],
                "joinery_scene3d_instances": built_snap["joinery_scene3d_instances"],
            }
    finally:
        close_client(z)

    licenses = out / "licenses"
    licenses.mkdir()
    shutil.copyfile(ROOT / "LICENSE-MIT", licenses / "LICENSE-MIT")
    shutil.copyfile(ROOT / "LICENSE-APACHE", licenses / "LICENSE-APACHE")
    (out / "report.md").write_text(REPORT_TEMPLATE, encoding="utf-8", newline="\n")
    write_json(out / "api.json", records)
    write_json(
        out / "manifest.json",
        {
            "issue": 898,
            "handoffIssue": 526,
            "license": "MIT OR Apache-2.0",
            "cli": {"path": str(Path(exe)), "sha256": sha(Path(exe).read_bytes()), "capabilities": capabilities},
            "globalIds": GIDS,
            "cases": cases,
        },
    )


def observe(exe, kit, version, state, exported):
    if version not in VERSIONS or state not in ("built", "changed"):
        raise ValueError("版は ifc2x3・ifc4・ifc4x3、状態は built・changed のどれかです")
    kit = kit.resolve()
    manifest = json.loads(read_input(kit / "manifest.json"))
    if manifest.get("issue") != 898:
        raise ValueError("このフォルダーは #898 の新規作成キットではありません")
    reference = json.loads(read_input(kit / version / f"{state}.json"))
    data = read_input(exported)
    destination = output_folder(kit / "observed" / version / state)
    records = []
    z = client(exe)
    try:
        doc = f"observed_{version}_{state}.ifc"
        z.push(doc, data)
        opened = call(z, {"op": "open", "input": doc}, records)
        observed = read_snapshot(z, doc, records)
        observed["diagnostics"] = diagnostic_counts(opened)
    finally:
        close_client(z)
    diffs = compare_observed(reference, observed)
    (destination / "exported.ifc").write_bytes(data)
    write_json(destination / "observed.json", observed)
    write_json(destination / "api.json", records)
    write_json(
        destination / "comparison.json",
        {
            "version": version,
            "state": state,
            "exported_sha256": sha(data),
            "differences": diffs,
            "diagnostics": observed["diagnostics"],
        },
    )
    return 1 if diffs else 0


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    create_cmd = sub.add_parser("make", help="3 版の 2 階建て新規作成・変更キットを作る")
    create_cmd.add_argument("exe", type=Path)
    create_cmd.add_argument("output", type=Path)
    obs_cmd = sub.add_parser("observe", help="BIM ソフトが再書き出しした IFC を読み直して比べる")
    obs_cmd.add_argument("exe", type=Path)
    obs_cmd.add_argument("kit", type=Path)
    obs_cmd.add_argument("version", choices=sorted(VERSIONS))
    obs_cmd.add_argument("state", choices=["built", "changed"])
    obs_cmd.add_argument("exported", type=Path)
    args = parser.parse_args(argv)
    if args.command == "make":
        make(args.exe, args.output)
        return 0
    return observe(args.exe, args.kit, args.version, args.state, args.exported)


if __name__ == "__main__":
    sys.exit(main())
