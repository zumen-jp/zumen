"""npm の再試行で、配布物の取り違えと検証の失敗を断ることを確かめる。"""

import copy
import hashlib
import io
import json
import subprocess
import tarfile
import tempfile
import unittest
import urllib.error
from pathlib import Path
from unittest.mock import Mock, call, patch

import retry_npm


def release_for(entry):
    return {"tag_name": entry["tag"], "draft": False, "prerelease": False,
            "assets": [{"name": entry["asset"], "digest": "sha256:" + entry["sha256"]}]}


def ref_for(entry):
    return {"ref": "refs/tags/" + entry["tag"], "object": {"type": "commit", "sha": entry["commit"]}}


def metadata_for(entry):
    return {"name": retry_npm.PACKAGE, "version": entry["version"],
            "repository": {"url": "git+https://github.com/zumen-jp/zumen.git"}}


def tarball_for(directory, entry, metadata=None, *, copies=1, name="package/package.json", kind=tarfile.REGTYPE):
    directory.mkdir(parents=True, exist_ok=True)
    path = directory / entry["asset"]
    content = json.dumps(metadata if metadata is not None else metadata_for(entry)).encode()
    with tarfile.open(path, "w:gz") as archive:
        for _ in range(copies):
            member = tarfile.TarInfo(name)
            member.type = kind
            if kind in (tarfile.SYMTYPE, tarfile.LNKTYPE):
                member.linkname = "other-package.json"
                archive.addfile(member)
            else:
                member.size = len(content)
                archive.addfile(member, io.BytesIO(content))
    checked_entry = dict(entry, sha256=hashlib.sha256(path.read_bytes()).hexdigest())
    return path, checked_entry


def not_found():
    error = urllib.error.HTTPError("https://registry.example.invalid/package", 404, "fixture", None, None)
    error.close()
    return error


class RetryNpmTests(unittest.TestCase):
    def setUp(self):
        self.entry = copy.deepcopy(retry_npm.RETRIES["v0.2.0"])
        # mock の設定漏れでも、gh やネットワークを実際には呼ばない。
        self.run = self.enterContext(patch.object(retry_npm.subprocess, "run",
                                                 side_effect=AssertionError("unexpected subprocess")))
        self.check_output = self.enterContext(patch.object(retry_npm.subprocess, "check_output",
                                                          side_effect=AssertionError("unexpected gh API")))
        self.urlopen = self.enterContext(patch.object(retry_npm.urllib.request, "urlopen",
                                                     side_effect=AssertionError("unexpected network request")))

    def test_unapproved_tags_stop_before_fetching(self):
        with tempfile.TemporaryDirectory() as temporary:
            for tag in ("", "v0.2.1", "0.2.0", "v0.2.0/../v0.2.0", "v0.2.0\n"):
                with self.subTest(tag=tag), self.assertRaisesRegex(ValueError, "not approved"):
                    retry_npm.prepare(tag, Path(temporary) / "out")
            self.check_output.assert_not_called()
            self.run.assert_not_called()
            self.assertFalse((Path(temporary) / "out").exists())

    def test_gh_api_is_scoped_to_the_repository(self):
        self.check_output.side_effect = None
        self.check_output.return_value = b'{"fixture": true}'
        self.assertEqual(retry_npm.gh_json("git/ref/tags/v0.2.0"), {"fixture": True})
        self.check_output.assert_called_once_with(
            ["gh", "api", "repos/zumen-jp/zumen/git/ref/tags/v0.2.0"])

    def test_lightweight_and_annotated_tags_resolve_to_the_commit(self):
        fetch = Mock(side_effect=AssertionError("unexpected annotated tag"))
        self.assertEqual(retry_npm.resolve_commit(ref_for(self.entry), fetch), self.entry["commit"])
        fetch.assert_not_called()
        ref = {"object": {"type": "tag", "sha": "1" * 40}}
        fetch.side_effect = [{"object": {"type": "tag", "sha": "2" * 40}},
                             {"object": {"type": "commit", "sha": self.entry["commit"]}}]
        self.assertEqual(retry_npm.resolve_commit(ref, fetch), self.entry["commit"])
        self.assertEqual(fetch.call_args_list, [call("1" * 40), call("2" * 40)])

    def test_invalid_git_object_type_and_sha_are_rejected(self):
        fetch = Mock(side_effect=AssertionError("unexpected tag fetch"))
        for kind, sha in (("tree", "a" * 40), ("blob", "a" * 40), ("commit", "a" * 39),
                          ("tag", "g" * 40), ("commit", "A" * 40)):
            with self.subTest(kind=kind, sha=sha), self.assertRaises(ValueError):
                retry_npm.resolve_commit({"object": {"type": kind, "sha": sha}}, fetch)
        fetch.assert_not_called()

    def test_deep_and_cyclic_annotated_tags_are_bounded(self):
        for cycle in (False, True):
            with self.subTest(cycle=cycle):
                fetch = Mock(side_effect=lambda sha: {"object": {
                    "type": "tag", "sha": sha if cycle else f"{int(sha, 16) + 1:040x}"}})
                with self.assertRaisesRegex(ValueError, "too many annotated tags"):
                    retry_npm.resolve_commit({"object": {"type": "tag", "sha": "1" * 40}}, fetch)
                self.assertLessEqual(fetch.call_count, 4)

    def test_different_tag_ref_or_commit_stops_before_release_and_download(self):
        for key in ("ref", "commit"):
            with self.subTest(key=key), tempfile.TemporaryDirectory() as temporary:
                ref = ref_for(self.entry)
                if key == "ref":
                    ref["ref"] = "refs/heads/main"
                else:
                    ref["object"]["sha"] = "0" * 40
                with patch.object(retry_npm, "gh_json", return_value=ref) as gh:
                    with self.assertRaisesRegex(ValueError, "git ref differs|tag commit differs"):
                        retry_npm.prepare(self.entry["tag"], Path(temporary) / "out")
                    gh.assert_called_once_with("git/ref/tags/" + self.entry["tag"])
                self.run.assert_not_called()
                self.assertFalse((Path(temporary) / "out").exists())

    def test_public_stable_release_with_one_matching_asset_is_accepted(self):
        release = release_for(self.entry)
        release["assets"].append({"name": "unrelated.zip", "digest": "sha256:" + "0" * 64})
        retry_npm.validate_release(release, self.entry)

    def test_release_tag_draft_and_prerelease_are_rejected(self):
        changes = [("tag_name", "v0.2.1"), ("draft", True), ("prerelease", True),
                   ("draft", 0), ("prerelease", None)]
        for field, value in changes:
            with self.subTest(field=field, value=value), self.assertRaises(ValueError):
                release = release_for(self.entry)
                release[field] = value
                retry_npm.validate_release(release, self.entry)

    def test_missing_or_ambiguous_release_asset_is_rejected(self):
        asset = release_for(self.entry)["assets"][0]
        for assets in ([], [{"name": "other.tgz", "digest": asset["digest"]}], [asset, dict(asset)]):
            with self.subTest(count=len(assets)), self.assertRaisesRegex(ValueError, "missing or ambiguous"):
                release = release_for(self.entry)
                release["assets"] = assets
                retry_npm.validate_release(release, self.entry)

    def test_different_release_asset_digest_is_rejected(self):
        release = release_for(self.entry)
        release["assets"][0]["digest"] = "sha256:" + "0" * 64
        with self.assertRaisesRegex(ValueError, "asset digest differs"):
            retry_npm.validate_release(release, self.entry)

    def test_valid_tarball_is_read_without_unpacking_or_running_scripts(self):
        with tempfile.TemporaryDirectory() as temporary:
            metadata = metadata_for(self.entry)
            metadata["scripts"] = {"prepublishOnly": "fixture-must-not-run"}
            path, entry = tarball_for(Path(temporary), self.entry, metadata)
            with patch.object(tarfile.TarFile, "extractall", side_effect=AssertionError("unexpected unpacking")):
                retry_npm.validate_tarball(path, entry)
            self.run.assert_not_called()

    def test_different_download_digest_is_rejected_before_reading_the_archive(self):
        with tempfile.TemporaryDirectory() as temporary:
            path, entry = tarball_for(Path(temporary), self.entry)
            entry["sha256"] = "0" * 64
            with patch.object(tarfile, "open", side_effect=AssertionError("unexpected archive read")):
                with self.assertRaisesRegex(ValueError, "downloaded asset digest differs"):
                    retry_npm.validate_tarball(path, entry)

    def test_oversized_asset_is_rejected_before_reading(self):
        path = Mock(spec=Path)
        path.stat.return_value.st_size = 128 * 1024 * 1024 + 1
        with self.assertRaisesRegex(ValueError, "asset is too large"):
            retry_npm.validate_tarball(path, self.entry)
        path.open.assert_not_called()

    def test_wrong_package_name_version_and_repository_are_rejected(self):
        for field in ("name", "version", "repository"):
            with self.subTest(field=field), tempfile.TemporaryDirectory() as temporary:
                metadata = metadata_for(self.entry)
                if field == "repository":
                    metadata[field]["url"] = "git+https://github.com/other/fixture.git"
                else:
                    metadata[field] = "other"
                path, entry = tarball_for(Path(temporary), self.entry, metadata)
                with self.assertRaisesRegex(ValueError, "package " + field + " differs"):
                    retry_npm.validate_tarball(path, entry)

    def test_missing_and_duplicate_package_metadata_are_rejected(self):
        for copies, name in ((1, "other/package.json"), (2, "package/package.json")):
            with self.subTest(copies=copies, name=name), tempfile.TemporaryDirectory() as temporary:
                path, entry = tarball_for(Path(temporary), self.entry, copies=copies, name=name)
                with self.assertRaisesRegex(ValueError, "metadata is missing or ambiguous"):
                    retry_npm.validate_tarball(path, entry)

    def test_symlink_and_hardlink_package_metadata_are_rejected(self):
        for kind in (tarfile.SYMTYPE, tarfile.LNKTYPE):
            with self.subTest(kind=kind), tempfile.TemporaryDirectory() as temporary:
                path, entry = tarball_for(Path(temporary), self.entry, kind=kind)
                with self.assertRaisesRegex(ValueError, "invalid package metadata"):
                    retry_npm.validate_tarball(path, entry)

    def test_oversized_package_metadata_is_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            metadata = metadata_for(self.entry)
            metadata["fixture"] = "a" * 65536
            path, entry = tarball_for(Path(temporary), self.entry, metadata)
            with self.assertRaisesRegex(ValueError, "invalid package metadata"):
                retry_npm.validate_tarball(path, entry)

    def test_registry_404_is_the_only_missing_version_response(self):
        self.urlopen.side_effect = not_found()
        retry_npm.require_unpublished(self.entry)
        args, kwargs = self.urlopen.call_args
        self.assertEqual(args[0].full_url, "https://registry.npmjs.org/@zumen-jp%2fzumen/0.2.0")
        self.assertEqual(kwargs, {"timeout": 30})

    def test_existing_registry_version_is_rejected(self):
        self.urlopen.side_effect = None
        with self.assertRaisesRegex(ValueError, "already published"):
            retry_npm.require_unpublished(self.entry)
        self.urlopen.return_value.__exit__.assert_called_once()

    def test_other_registry_http_errors_are_preserved(self):
        for code in (401, 403, 429, 500, 503):
            with self.subTest(code=code):
                error = urllib.error.HTTPError("https://registry.example.invalid/package", code, "fixture", None, None)
                error.close()
                self.urlopen.side_effect = error
                with self.assertRaises(urllib.error.HTTPError) as raised:
                    retry_npm.require_unpublished(self.entry)
                self.assertIs(raised.exception, error)

    def test_registry_transport_failures_are_preserved(self):
        for error in (urllib.error.URLError("fixture connection failure"), TimeoutError("fixture timeout")):
            with self.subTest(kind=type(error).__name__):
                self.urlopen.side_effect = error
                with self.assertRaises(type(error)) as raised:
                    retry_npm.require_unpublished(self.entry)
                self.assertIs(raised.exception, error)

    def test_attestation_pins_source_signer_ref_and_hosted_runner(self):
        self.run.side_effect = None
        path = Path("fixture.tgz")
        retry_npm.verify_attestation(path, self.entry)
        self.run.assert_called_once()
        args, kwargs = self.run.call_args
        command = args[0]
        self.assertEqual(command[:4], ["gh", "attestation", "verify", str(path)])
        expected = {"--repo": "zumen-jp/zumen",
                    "--signer-workflow": "zumen-jp/zumen/.github/workflows/release.yml",
                    "--signer-digest": self.entry["commit"], "--source-digest": self.entry["commit"],
                    "--source-ref": "refs/tags/" + self.entry["tag"]}
        for option, value in expected.items():
            with self.subTest(option=option):
                self.assertIn(option, command)
                self.assertEqual(command[command.index(option) + 1], value)
        self.assertIn("--deny-self-hosted-runners", command)
        self.assertEqual(kwargs, {"check": True})

    def test_failed_attestation_is_preserved(self):
        error = subprocess.CalledProcessError(1, ["gh", "attestation", "verify"])
        self.run.side_effect = error
        with self.assertRaises(subprocess.CalledProcessError) as raised:
            retry_npm.verify_attestation(Path("fixture.tgz"), self.entry)
        self.assertIs(raised.exception, error)

    def test_prepare_checks_the_synthetic_asset_before_the_registry(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source, entry = tarball_for(root / "source", self.entry)
            out = root / "out"
            events = []

            def run(command, *, check):
                self.assertTrue(check)
                if command[1:3] == ["release", "download"]:
                    events.append("download")
                    self.assertEqual(command, ["gh", "release", "download", entry["tag"],
                                              "--repo", "zumen-jp/zumen", "--pattern", entry["asset"],
                                              "--dir", str(out)])
                    (out / entry["asset"]).write_bytes(source.read_bytes())
                else:
                    self.assertEqual(command[1:3], ["attestation", "verify"])
                    events.append("attestation")
                return subprocess.CompletedProcess(command, 0)

            def registry(*args, **kwargs):
                events.append("registry")
                raise not_found()

            self.run.side_effect = run
            self.urlopen.side_effect = registry
            with patch.dict(retry_npm.RETRIES, {entry["tag"]: entry}):
                with patch.object(retry_npm, "gh_json", side_effect=[ref_for(entry), release_for(entry)]) as gh:
                    path = retry_npm.prepare(entry["tag"], out)
                    self.assertEqual(gh.call_args_list,
                                     [call("git/ref/tags/" + entry["tag"]), call("releases/tags/" + entry["tag"])])
            self.assertEqual(path, out / entry["asset"])
            self.assertEqual(path.read_bytes(), source.read_bytes())
            self.assertEqual(events, ["download", "attestation", "registry"])

    def test_prepare_stops_after_failed_attestation(self):
        error = subprocess.CalledProcessError(1, ["gh", "attestation", "verify"])
        self.run.side_effect = None
        with tempfile.TemporaryDirectory() as temporary:
            out = Path(temporary) / "out"
            with patch.object(retry_npm, "gh_json", side_effect=[ref_for(self.entry), release_for(self.entry)]):
                with patch.object(retry_npm, "validate_tarball") as validate:
                    with patch.object(retry_npm, "verify_attestation", side_effect=error) as verify:
                        with patch.object(retry_npm, "require_unpublished") as unpublished:
                            with self.assertRaises(subprocess.CalledProcessError) as raised:
                                retry_npm.prepare(self.entry["tag"], out)
                            self.assertIs(raised.exception, error)
                            validate.assert_called_once_with(out / self.entry["asset"], self.entry)
                            verify.assert_called_once_with(out / self.entry["asset"], self.entry)
                            unpublished.assert_not_called()
            self.run.assert_called_once()
            self.urlopen.assert_not_called()

    def test_prepare_does_not_overwrite_an_existing_output(self):
        with tempfile.TemporaryDirectory() as temporary:
            out = Path(temporary)
            existing = out / self.entry["asset"]
            existing.write_bytes(b"existing fixture")
            with patch.object(retry_npm, "gh_json", side_effect=[ref_for(self.entry), release_for(self.entry)]):
                with self.assertRaisesRegex(ValueError, "output already exists"):
                    retry_npm.prepare(self.entry["tag"], out)
            self.assertEqual(existing.read_bytes(), b"existing fixture")
            self.run.assert_not_called()


if __name__ == "__main__":
    unittest.main()
