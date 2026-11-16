"""実測した BCF の観測を変え、値・参照・重複・画像の差を検出するか確かめる。"""

import base64
from copy import deepcopy
import importlib.util
import json
from pathlib import Path
import sys
import tempfile
import unittest

spec = importlib.util.spec_from_file_location("bcf_compare", Path(__file__).with_name("compare.py"))
compare = importlib.util.module_from_spec(spec)
spec.loader.exec_module(compare)
spec = importlib.util.spec_from_file_location("bcf_prepare", Path(__file__).with_name("prepare.py"))
prepare = importlib.util.module_from_spec(spec)
spec.loader.exec_module(prepare)
KIT = Path(sys.argv[1]).resolve(strict=True)
sys.argv = sys.argv[:1]
LABELS = ("rewritten-2.1", "new-2.1", "new-3.0")


def views(document, markup):
    return markup["Viewpoints"] if document["Version"]["VersionId"] == "2.1" else markup["Topic"]["Viewpoints"]


def comments(document, markup):
    return markup["Comment"] if document["Version"]["VersionId"] == "2.1" else markup["Topic"]["Comments"]


class CompareTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.mutations = 0
        cls.external, cls.core = [], []
        cls.reference = json.loads((KIT / "core-reference.json").read_bytes())
        for label in LABELS:
            wanted = json.loads((KIT / (label + "-expected.json")).read_bytes())
            for kind in ("direct", "reread"):
                cls.external.append((wanted, json.loads((KIT / (label + "-" + kind + "-raw.json")).read_bytes())))
            cls.core.append((wanted, json.loads((KIT / (label + "-core.json")).read_bytes())))

    def changed_external(self, mutate, predicate=lambda _: True):
        count = 0
        for wanted, original in self.external:
            if not predicate(original):
                continue
            with self.subTest(version=original["Version"]["VersionId"]):
                data = deepcopy(original)
                mutate(data)
                self.assertFalse(compare.compare_external(wanted, data)["matched"])
                type(self).mutations += 1
                count += 1
        self.assertGreater(count, 0)

    def changed_core(self, mutate):
        for wanted, original in self.core:
            data = deepcopy(original)
            mutate(data)
            self.assertFalse(compare.compare_core(wanted, data["response"], data["review"], self.reference)["matched"])
            type(self).mutations += 1

    def test_original_and_fingerprints(self):
        summary = json.loads((KIT / "results.json").read_bytes())
        self.assertIs(summary["complete"], True)
        self.assertIs(summary["matched"], True)
        self.assertEqual(len(summary["cases"]), 3)
        for name, digest in summary["artifacts"].items():
            self.assertEqual(compare.sha((KIT / name).read_bytes()), digest, name)
        for wanted, data in self.external:
            self.assertTrue(compare.compare_external(wanted, data)["matched"])
        for wanted, data in self.core:
            self.assertTrue(compare.compare_core(wanted, data["response"], data["review"], self.reference)["matched"])

    def test_topic_values(self):
        for key in ("Guid", "Title", "Description", "TopicType", "TopicStatus", "CreationDate", "CreationAuthor"):
            self.changed_external(lambda data: data["Markups"][0]["Topic"].__setitem__(key, "wrong"))

    def test_topic_missing_fields(self):
        for key in ("Guid", "Title", "Description", "TopicType", "TopicStatus", "CreationDate", "CreationAuthor"):
            self.changed_external(lambda data: data["Markups"][0]["Topic"].pop(key))

    def test_version(self):
        self.changed_external(lambda data: data["Version"].__setitem__("VersionId", "4.0"))

    def test_missing_or_duplicate_topic(self):
        self.changed_external(lambda data: data["Markups"].pop())
        self.changed_external(lambda data: data["Markups"].append(deepcopy(data["Markups"][0])))

    def test_duplicate_replaces_missing_topic(self):
        self.changed_external(lambda data: data["Markups"].__setitem__(1, deepcopy(data["Markups"][0])))

    def test_comment_values(self):
        for key in ("Guid", "Author", "Date", "CommentProperty"):
            self.changed_external(lambda data: comments(data, data["Markups"][0])[0].__setitem__(key, "wrong"))

    def test_comment_reference(self):
        self.changed_external(lambda data: comments(data, data["Markups"][0])[0]["Viewpoint"].__setitem__("Guid", "wrong"))

    def test_missing_or_duplicate_comment(self):
        self.changed_external(lambda data: comments(data, data["Markups"][0]).clear())
        self.changed_external(lambda data: comments(data, data["Markups"][0]).append(deepcopy(comments(data, data["Markups"][0])[0])))

    def test_missing_or_duplicate_view(self):
        self.changed_external(lambda data: views(data, data["Markups"][0]).clear())
        self.changed_external(lambda data: views(data, data["Markups"][0]).append(deepcopy(views(data, data["Markups"][0])[0])))

    def test_view_and_visualization_ids(self):
        self.changed_external(lambda data: views(data, data["Markups"][0])[0].__setitem__("Guid", "wrong"))
        self.changed_external(lambda data: views(data, data["Markups"][0])[0]["VisualizationInfo"].__setitem__("Guid", "wrong"))

    def test_camera_vectors(self):
        for key in ("CameraViewPoint", "CameraDirection", "CameraUpVector"):
            for axis in ("X", "Y", "Z"):
                def mutate(data):
                    info = views(data, data["Markups"][0])[0]["VisualizationInfo"]
                    camera = info.get("PerspectiveCamera") or info["OrthogonalCamera"]
                    camera[key][axis] += 0.25
                self.changed_external(mutate)

    def test_camera_scale(self):
        for kind, field in (("PerspectiveCamera", "FieldOfView"), ("OrthogonalCamera", "ViewToWorldScale")):
            def candidates(data):
                return [view["VisualizationInfo"][kind] for markup in data["Markups"] for view in views(data, markup) if view["VisualizationInfo"].get(kind)]
            self.changed_external(lambda data: candidates(data)[0].__setitem__(field, 23.0), lambda data: bool(candidates(data)))

    def test_missing_camera(self):
        def mutate(data):
            info = views(data, data["Markups"][0])[0]["VisualizationInfo"]
            info["PerspectiveCamera"] = info["OrthogonalCamera"] = None
        self.changed_external(mutate)

    def test_boolean_is_not_coordinate(self):
        def mutate(data):
            info = views(data, data["Markups"][0])[0]["VisualizationInfo"]
            camera = info.get("PerspectiveCamera") or info["OrthogonalCamera"]
            camera["CameraUpVector"]["X"] = False
        self.changed_external(mutate)

    def test_aspect_ratio(self):
        def mutate(data):
            info = views(data, data["Markups"][0])[0]["VisualizationInfo"]
            camera = info.get("PerspectiveCamera") or info["OrthogonalCamera"]
            camera["AspectRatio"] = 2.0
        self.changed_external(mutate)

    def test_visibility(self):
        for value in (False, 1):
            self.changed_external(lambda data: views(data, data["Markups"][0])[0]["VisualizationInfo"]["Components"]["Visibility"].__setitem__("DefaultVisibility", value))

    def test_visibility_exception(self):
        self.changed_external(lambda data: views(data, data["Markups"][0])[0]["VisualizationInfo"]["Components"]["Visibility"].__setitem__("Exceptions", [{"IfcGuid": "wrong"}]))

    def test_selection_changed_missing_or_duplicate(self):
        def selected(data):
            return next(view["VisualizationInfo"]["Components"]["Selection"] for markup in data["Markups"] for view in views(data, markup) if view["VisualizationInfo"]["Components"].get("Selection"))
        self.changed_external(lambda data: selected(data)[0].__setitem__("IfcGuid", "wrong"))
        self.changed_external(lambda data: selected(data).clear())
        self.changed_external(lambda data: selected(data).append(deepcopy(selected(data)[0])))

    def test_snapshot_changed_missing_or_extra(self):
        def has_image(data):
            return any(view.get("snapshot_data") for markup in data["Markups"] for view in views(data, markup))
        def mutate(data):
            image = views(data, data["Markups"][0])[0]["snapshot_data"]
            raw = bytearray(base64.b64decode(image["Data"]))
            raw[-1] ^= 1
            image["Data"] = base64.b64encode(raw).decode("ascii")
        self.changed_external(mutate, has_image)
        self.changed_external(lambda data: views(data, data["Markups"][0])[0].__setitem__("snapshot_data", None), has_image)
        self.changed_external(lambda data: views(data, data["Markups"][0])[0].__setitem__("snapshot_data", {"Data": "AA=="}), lambda data: not has_image(data))

    def test_core_response(self):
        self.changed_core(lambda data: data["response"]["result"].__setitem__("topics", 3))
        self.changed_core(lambda data: data["response"]["result"].__setitem__("version", "4.0"))
        self.changed_core(lambda data: data["response"].__setitem__("v", True))
        self.changed_core(lambda data: data["response"]["diagnostics"].append({"code": "unexpected"}))

    def test_core_target_reference(self):
        for key in ("id", "name", "hash", "format"):
            self.changed_core(lambda data: data["review"]["targets"][0].__setitem__(key, "wrong"))
        self.changed_core(lambda data: data["review"]["topics"][0]["locations"][0].__setitem__("target", "wrong"))

    def test_core_values_and_missing_topic(self):
        self.changed_core(lambda data: data["review"]["topics"][0].__setitem__("title", "wrong"))
        self.changed_core(lambda data: data["review"]["topics"].pop())
        self.changed_core(lambda data: data["review"]["topics"][0]["comments"][0].__setitem__("location", "wrong"))

    def test_reordering_is_equivalent(self):
        for wanted, original in self.external:
            data = deepcopy(original)
            data["Markups"].reverse()
            self.assertTrue(compare.compare_external(wanted, data)["matched"])
        for wanted, original in self.core:
            data = deepcopy(original)
            data["review"]["topics"].reverse()
            self.assertTrue(compare.compare_core(wanted, data["response"], data["review"], self.reference)["matched"])


class OutputTests(unittest.TestCase):
    def test_sdk_ten_is_selected_among_other_releases(self):
        listing = "11.0.100 [sdk]\n10.0.100 [sdk]\n10.0.404 [sdk]\n9.0.999 [sdk]\n10.0.999-preview [sdk]\n"
        self.assertEqual(prepare.select_sdk(listing), "10.0.404")

    def test_sdk_ten_is_required(self):
        with self.assertRaises(ValueError):
            prepare.select_sdk("11.0.100 [sdk]\n10.0.999-preview [sdk]\n")

    def test_existing_output_is_preserved(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory)
            (path / "marker").write_bytes(b"preserve")
            with self.assertRaises(ValueError):
                prepare.new_folder(path)
            self.assertEqual((path / "marker").read_bytes(), b"preserve")

    def test_git_worktree_is_refused(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory)
            (path / ".git").write_text("gitdir: elsewhere", encoding="utf-8")
            with self.assertRaises(ValueError):
                prepare.new_folder(path / "nested" / "output")
            self.assertFalse((path / "nested").exists())


if __name__ == "__main__":
    program = unittest.main(exit=False)
    result = {"successful": program.result.wasSuccessful(), "tests": program.result.testsRun, "negativeObservations": CompareTests.mutations,
              "harnessSha256": compare.sha(Path(__file__).read_bytes())}
    prepare.write_json(KIT / "test-results.json", result)
    print(json.dumps(result))
    raise SystemExit(0 if result["successful"] else 1)
