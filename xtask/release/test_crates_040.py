import io
import unittest
from unittest.mock import patch
from urllib.error import HTTPError

import crates_040 as crates


class RustReleaseTest(unittest.TestCase):
    def test_dependency_groups_cover_all_public_crates_once(self):
        def dep(name, kind=None, path="crates"):
            return {"name": name, "kind": kind, "path": path}

        def pkg(name, *deps, private=False):
            return {"name": name, "version": "0.4.0", "dependencies": deps, "publish": [] if private else None}

        packages = [pkg("consumer", dep(crates.FIRST)), pkg("base"), pkg("builder", dep("base")),
                    pkg(crates.FIRST, dep("builder", "build"), dep("consumer", "dev"), dep("external", path=None)),
                    pkg("tool", private=True)]
        flags = crates.cargo_args(["consumer", crates.FIRST, "base", "builder"])
        self.assertEqual(crates.groups(packages, flags), {"before": ["base", "builder"], "after": ["consumer"]})
        for wrong in (flags[:-2], flags + flags[:2], flags + ["-p", "tool"]):
            with self.assertRaises(ValueError):
                crates.groups(packages, wrong)

    def test_partial_retry_requires_equal_registry_bytes(self):
        names = ["a", "b"]
        self.assertEqual(crates.remaining(names, {"a": "checksum", "b": None},
                                          {"a": "checksum"}.__getitem__), ["b"])
        with self.assertRaisesRegex(ValueError, "SHA-256"):
            crates.remaining(names, {"a": "different", "b": None}, {"a": "checksum"}.__getitem__)

    def test_registry_missing_version_and_yanked(self):
        with patch("crates_040.urlopen", side_effect=HTTPError("example", 404, "", {}, None)):
            self.assertIsNone(crates.registry_digest(crates.FIRST))
        index = b'{"name":"zumen-stbridge","vers":"0.3.0","cksum":"old","yanked":false}\n'
        with patch("crates_040.urlopen", return_value=io.BytesIO(index)):
            self.assertIsNone(crates.registry_digest(crates.FIRST))
        current = b'{"name":"zumen-stbridge","vers":"0.4.0","cksum":"expected","yanked":false}\n'
        with patch("crates_040.urlopen", return_value=io.BytesIO(index + current)):
            self.assertEqual(crates.registry_digest(crates.FIRST), "expected")
        with patch("crates_040.urlopen", return_value=io.BytesIO(current.replace(b"false", b"true"))):
            with self.assertRaisesRegex(ValueError, "yank"):
                crates.registry_digest(crates.FIRST)
