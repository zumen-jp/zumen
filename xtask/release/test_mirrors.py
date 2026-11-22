"""鏡と作業用の Git リポジトリを一時フォルダーに作り、ネットワークを使わずに確かめる。"""

import os
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import mirrors
from package import write_archive


class MirrorTests(unittest.TestCase):
    def setUp(self):
        isolated = patch.dict(os.environ, {"GIT_CONFIG_GLOBAL": os.devnull, "GIT_CONFIG_NOSYSTEM": "1",
                                          "GIT_TERMINAL_PROMPT": "0"})
        isolated.start()
        self.addCleanup(isolated.stop)
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.remote = self.root / "remote.git"
        self.remote.mkdir()
        mirrors.git(self.remote, "init", "--bare", "-b", "main", "--quiet")
        self.step = 0
        self.files = {"v0.4.0": {"go.mod": b"module a\n", "old.go": b"package a\n"},
                      "v0.5.0": {"go.mod": b"module b\n", "new.go": b"package b\n"}}

    def plan(self, tag, files=None):
        self.step += 1
        repo = self.root / f"stage{self.step}"
        plan = mirrors.prepare(repo, self.remote, "go", tag, files or self.files[tag],
                               lambda kind, version: self.files[version])
        return repo, plan

    def publish(self, tag):
        repo, plan = self.plan(tag)
        mirrors.push(repo, self.remote, tag, plan)

    def edit(self, path, content, tag=None):
        work = self.root / "working"
        mirrors.git(self.root, "clone", "--quiet", str(self.remote), str(work))
        (work / path).parent.mkdir(parents=True, exist_ok=True)
        (work / path).write_bytes(content)
        mirrors.git(work, "add", "--", path)
        mirrors.git(work, "-c", "user.name=fixture", "-c", "user.email=fixture@example.invalid", "commit", "-qm", "fixture")
        mirrors.git(work, "push", "--quiet", "origin", "HEAD:refs/heads/main")
        if tag:
            mirrors.git(work, "-c", "user.name=fixture", "-c", "user.email=fixture@example.invalid",
                        "tag", "-a", tag, "-m", "manual release")
            mirrors.git(work, "push", "--quiet", "origin", "refs/tags/" + tag)

    def test_initial_and_next_release_keep_unmanaged_files_and_history(self):
        self.publish("v0.4.0")
        previous = mirrors.git(self.remote, "rev-parse", "refs/tags/v0.4.0")
        self.edit("docs/owners.md", b"maintainer\n")
        self.publish("v0.5.0")
        self.assertEqual(set(mirrors.entries(self.remote, "v0.5.0")), {"docs/owners.md", "go.mod", "new.go"})
        self.assertEqual(mirrors.git(self.remote, "show", "v0.5.0:docs/owners.md"), b"maintainer\n")
        self.assertEqual(mirrors.git(self.remote, "rev-parse", "refs/tags/v0.4.0"), previous)
        mirrors.git(self.remote, "merge-base", "--is-ancestor", "v0.4.0", "v0.5.0")
        self.assertIsNone(self.plan("v0.5.0")[1])
        self.assertIsNone(self.plan("v0.4.0")[1])

    def test_manual_tag_and_conflicting_tag(self):
        self.edit("go.mod", self.files["v0.4.0"]["go.mod"], tag="v0.4.0")
        self.assertIsNone(self.plan("v0.4.0", {"go.mod": b"module a\n"})[1])
        with self.assertRaisesRegex(ValueError, "既存タグ"):
            self.plan("v0.4.0")

    def test_conflicting_work_and_stale_files_stop_without_writing(self):
        self.publish("v0.4.0")
        self.edit("go.mod", b"local edit\n")
        with self.assertRaisesRegex(ValueError, "衝突"):
            self.plan("v0.5.0")
        self.assertFalse(mirrors.git(self.remote, "tag", "--list", "v0.5.0").strip())
        self.assertEqual(mirrors.git(self.remote, "show", "main:go.mod"), b"local edit\n")

    def test_concurrent_edit_does_not_create_tag_on_failed_push(self):
        self.publish("v0.4.0")
        repo, plan = self.plan("v0.5.0")
        self.edit("notes.txt", b"arrived after planning\n")
        with self.assertRaises(subprocess.CalledProcessError):
            mirrors.push(repo, self.remote, "v0.5.0", plan)
        self.assertFalse(mirrors.git(self.remote, "tag", "--list", "v0.5.0").strip())
        self.publish("v0.5.0")
        self.assertEqual(mirrors.git(self.remote, "show", "v0.5.0:notes.txt"), b"arrived after planning\n")

    def test_source_archive_layout_and_paths(self):
        for kind, filename in (("go", "go.mod"), ("swift", "Package.swift")):
            prefix = "" if kind == "go" else "zumen-swift-0.4.0-source/"
            archive = self.root / mirrors.asset_name(kind, "v0.4.0")
            write_archive(archive, {prefix + filename: b"file"}, None)
            self.assertEqual(mirrors.source_files(kind, "v0.4.0", archive), {filename: b"file"})
            write_archive(archive, {prefix + filename: b"file", prefix + "../oops": b"oops"}, None)
            with self.assertRaisesRegex(ValueError, "パス"):
                mirrors.source_files(kind, "v0.4.0", archive)


if __name__ == "__main__":
    unittest.main()
