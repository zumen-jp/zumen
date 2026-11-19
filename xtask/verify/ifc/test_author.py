"""新規作成 2 階建て IFC キット（#898・#526）の生成・検証・再読み込み比較を試験する。"""

import copy
import importlib.util
import json
import os
import tempfile
import unittest
from pathlib import Path

spec = importlib.util.spec_from_file_location("ifc_author_kit", Path(__file__).with_name("author.py"))
author_kit = importlib.util.module_from_spec(spec)
spec.loader.exec_module(author_kit)


class AuthorKitUnitTests(unittest.TestCase):
    def test_verify_snapshot_catches_elevation_layers_and_joinery_body(self):
        exp = author_kit.expected_state("ifc4", "built")
        elements = [{"id": gid, "class": {"name": "IfcWall"}, "placement": {"world": author_kit.identity_matrix()}}
                    for gid in author_kit.GIDS.values()]
        by_id = {e["id"]: e for e in elements}
        by_id[author_kit.GIDS["storey_2f"]]["placement"]["world"] = author_kit.identity_matrix(3000.0)
        by_id[author_kit.GIDS["window_2f"]]["placement"]["world"] = author_kit.identity_matrix(3900.0)
        by_id[author_kit.GIDS["wall_1f"]]["material"] = {
            "parts": [{"thickness": {"value": 150.0}}, {"thickness": {"value": 50.0}}]
        }
        snap = {"schema": "IFC4", "elements": elements, "joinery_scene3d_instances": 2}
        self.assertEqual(author_kit.verify_snapshot(snap, exp), [])

        no_body = copy.deepcopy(snap)
        no_body["joinery_scene3d_instances"] = 0
        self.assertIn("joinery_body_missing", author_kit.verify_snapshot(no_body, exp))

        wrong_z = copy.deepcopy(snap)
        wrong_z["elements"] = copy.deepcopy(elements)
        for e in wrong_z["elements"]:
            if e["id"] == author_kit.GIDS["window_2f"]:
                e["placement"]["world"] = author_kit.identity_matrix(3000.0)
        self.assertIn("window_2f_z", author_kit.verify_snapshot(wrong_z, exp))

    def test_compare_observed_detects_missing_global_id_and_moved_placement(self):
        ref = {
            "schema": "IFC4",
            "elements": [
                {"id": author_kit.GIDS["wall_1f"], "class": {"name": "IfcWall"}, "placement": {"world": author_kit.identity_matrix(0.0)}},
                {"id": author_kit.GIDS["door_1f"], "class": {"name": "IfcDoor"}, "placement": {"world": author_kit.identity_matrix(0.0)}},
            ],
        }
        self.assertEqual(author_kit.compare_observed(ref, copy.deepcopy(ref)), [])
        dropped = copy.deepcopy(ref)
        dropped["elements"].pop()
        self.assertIn("global_ids", author_kit.compare_observed(ref, dropped))
        moved = copy.deepcopy(ref)
        moved["elements"][0]["placement"]["world"] = author_kit.identity_matrix(100.0)
        self.assertIn("placement:" + author_kit.GIDS["wall_1f"], author_kit.compare_observed(ref, moved))

    @unittest.skipUnless(os.environ.get("ZUMEN_CLI"), "ZUMEN_CLI が指定されたときだけ実 CLI で生成と再読込を確かめる")
    def test_cli_generates_three_versions_and_observes_clean_roundtrip(self):
        cli = Path(os.environ["ZUMEN_CLI"])
        with tempfile.TemporaryDirectory() as tmp:
            kit = Path(tmp) / "authored-kit"
            self.assertEqual(author_kit.main(["make", str(cli), str(kit)]), 0)
            manifest = json.loads((kit / "manifest.json").read_text(encoding="utf-8"))
            self.assertEqual(set(manifest["cases"]), {"ifc2x3", "ifc4", "ifc4x3"})
            for version in ("ifc2x3", "ifc4", "ifc4x3"):
                self.assertEqual(manifest["cases"][version]["joinery_scene3d_instances"], 2)
                code = author_kit.main(["observe", str(cli), str(kit), version, "changed", str(kit / version / "changed.ifc")])
                self.assertEqual(code, 0)
