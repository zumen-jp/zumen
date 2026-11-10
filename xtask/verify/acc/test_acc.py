"""比較器が保持したい値の違いを見逃さず、キットが既存のファイルを上書きしないことを確かめる。"""

import copy
import importlib.util
import io
import struct
import tempfile
import unittest
import zipfile
import zlib
from pathlib import Path

spec = importlib.util.spec_from_file_location("acc_make", Path(__file__).with_name("make.py"))
kit = importlib.util.module_from_spec(spec)
spec.loader.exec_module(kit)


def review_for(expected):
    return {"topics": [{"id": want["id"], "title": want["title"], "description": want["description"], "status": want["status"],
                        "comments": [{"text": want["comment"]}], "locations": [{"camera": copy.deepcopy(want["camera"]),
                        "selected": [{"id": value, "hash": want["element_hash"]} for value in want["selected"]]}]} for want in expected]}


class ComparisonTests(unittest.TestCase):
    def test_detects_each_changed_field(self):
        expected = kit.expected_cases("v1:123")
        original = review_for(expected)
        self.assertTrue(all(item["equal"] for item in kit.differences(original, expected)))
        changes = {
            "title": lambda t: t.update(title="別の題"),
            "description": lambda t: t.update(description="別の説明"),
            "status": lambda t: t.update(status="closed"),
            "comment": lambda t: t["comments"][0].update(text="別のコメント"),
            "selected": lambda t: t["locations"][0]["selected"][0].update(id="別の要素"),
            "element_hash": lambda t: t["locations"][0]["selected"][0].update(hash="v1:different"),
            "camera": lambda t: t["locations"][0]["camera"]["position"].__setitem__(0, 10),
        }
        for field, change in changes.items():
            with self.subTest(field=field):
                changed = copy.deepcopy(original)
                change(changed["topics"][0])
                self.assertIn(field, [item["field"] for item in kit.differences(changed, expected) if not item["equal"]])

    def test_detects_missing_and_duplicate_topics(self):
        expected = kit.expected_cases("v1:123")
        review = review_for(expected)
        for topics in [review["topics"][:-1], [review["topics"][0]] * 4]:
            self.assertTrue(any(not item["equal"] for item in kit.differences({"topics": topics}, expected)))

    def test_product_allows_new_ids_but_checks_documented_status(self):
        expected = kit.expected_cases("v1:123")
        review = review_for(expected)
        for topic, want in zip(review["topics"], expected):
            topic["id"] = "reassigned-" + topic["id"]
            topic["status"] = want["acc_status"].lower()
        self.assertTrue(all(item["equal"] for item in kit.differences(review, expected, product=True)))
        review["topics"][1]["status"] = "resolved"
        self.assertTrue(any(item["field"] == "status" and not item["equal"] for item in kit.differences(review, expected, product=True)))

    def test_snapshot_reference_and_bytes(self):
        image = kit.snapshot()
        bcf = kit.seed_bcf(image)
        hashes = kit.xml_snapshots(bcf)
        self.assertEqual(4, len(hashes))
        self.assertTrue(all(values == [kit.sha(image)] for values in hashes.values()))
        output = io.BytesIO()
        with zipfile.ZipFile(io.BytesIO(bcf)) as source, zipfile.ZipFile(output, "w") as changed:
            for name in source.namelist():
                changed.writestr(name, b"different" if name.endswith("snapshot.png") else source.read(name))
        self.assertNotEqual(hashes, kit.xml_snapshots(output.getvalue()))
        missing = io.BytesIO()
        with zipfile.ZipFile(io.BytesIO(bcf)) as source, zipfile.ZipFile(missing, "w") as changed:
            for name in source.namelist():
                if not name.endswith("snapshot.png"):
                    changed.writestr(name, source.read(name))
        self.assertTrue(all(values == [None] for values in kit.xml_snapshots(missing.getvalue()).values()))

    def test_png_is_valid_rgb_image(self):
        image = kit.snapshot()
        self.assertEqual(b"\x89PNG\r\n\x1a\n", image[:8])
        offset, chunks = 8, {}
        while offset < len(image):
            length = struct.unpack(">I", image[offset:offset + 4])[0]
            kind, value = image[offset + 4:offset + 8], image[offset + 8:offset + 8 + length]
            checksum = struct.unpack(">I", image[offset + 8 + length:offset + 12 + length])[0]
            self.assertEqual(checksum, zlib.crc32(kind + value))
            chunks[kind] = value
            offset += length + 12
        self.assertEqual((320, 200, 8, 2, 0, 0, 0), struct.unpack(">IIBBBBB", chunks[b"IHDR"]))
        self.assertEqual(200 * (320 * 3 + 1), len(zlib.decompress(chunks[b"IDAT"])))
        self.assertEqual(b"", chunks[b"IEND"])

    def test_refuses_zip_expansion_over_limit_before_reading(self):
        output = io.BytesIO()
        with zipfile.ZipFile(output, "w") as archive:
            archive.writestr("topic/markup.bcf", b"<Markup/>")
        data = bytearray(output.getvalue())
        # ZIP APPNOTE 4.3.12。中央ディレクトリーが大きな展開後長を宣言しても、展開する前に拒否する。
        struct.pack_into("<I", data, data.index(b"PK\x01\x02") + 24, 64 * 1024 * 1024 + 1)
        with self.assertRaises(ValueError):
            kit.xml_snapshots(data)


class OutputTests(unittest.TestCase):
    def test_refuses_nonempty_folder_and_preserves_file(self):
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder)
            (path / "keep.txt").write_text("keep", encoding="utf-8")
            with self.assertRaises(ValueError):
                kit.output_folder(path)
            self.assertEqual("keep", (path / "keep.txt").read_text(encoding="utf-8"))

    def test_refuses_git_worktree(self):
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder)
            (path / ".git").write_text("gitdir: elsewhere", encoding="utf-8")
            with self.assertRaises(ValueError):
                kit.output_folder(path / "out")
            self.assertFalse((path / "out").exists())


if __name__ == "__main__":
    unittest.main()
