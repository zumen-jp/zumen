"""公開済み Release の npm 配布物を、来歴を照合して再公開用に用意する（決定 0294）。"""

import argparse
import hashlib
import json
import re
import subprocess
import tarfile
import urllib.error
import urllib.request
from pathlib import Path

REPOSITORY = "zumen-jp/zumen"
PACKAGE = "@zumen-jp/zumen"
RETRIES = {
    "v0.2.0": {
        "tag": "v0.2.0",
        "version": "0.2.0",
        "commit": "ce01f02dc6725914f43627b3cdb353007dbd83dc",
        "asset": "zumen-jp-zumen-0.2.0.tgz",
        "sha256": "da0e5b5701bd06b6f023167aa763e082b1e6d7c76a7765168c38a24f2b2bcaa0",
    },
}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def retry_for(tag):
    require(tag in RETRIES, "tag is not approved for npm retry")
    return RETRIES[tag]


def gh_json(endpoint):
    data = subprocess.check_output(["gh", "api", f"repos/{REPOSITORY}/{endpoint}"])
    return json.loads(data)


def resolve_commit(ref, fetch_tag):
    """注釈付きタグを上限付きでたどり、コミット以外を拒む。"""
    obj = ref["object"]
    for _ in range(4):
        require(re.fullmatch(r"[0-9a-f]{40}", obj["sha"]) is not None, "invalid git object")
        if obj["type"] == "commit":
            return obj["sha"]
        require(obj["type"] == "tag", "tag does not name a commit")
        obj = fetch_tag(obj["sha"])["object"]
    raise ValueError("too many annotated tags")


def validate_release(release, entry):
    require(release["tag_name"] == entry["tag"], "release tag differs")
    require(release["draft"] is False and release["prerelease"] is False, "release is not public and stable")
    assets = [a for a in release["assets"] if a["name"] == entry["asset"]]
    require(len(assets) == 1, "npm asset is missing or ambiguous")
    require(assets[0]["digest"] == "sha256:" + entry["sha256"], "release asset digest differs")


def validate_tarball(path, entry):
    require(path.stat().st_size <= 128 * 1024 * 1024, "npm asset is too large")
    with path.open("rb") as stream:
        digest = hashlib.file_digest(stream, "sha256").hexdigest()
    require(digest == entry["sha256"], "downloaded asset digest differs")
    # 展開せず、メタデータだけを読む。配布物のスクリプトは実行しない。
    with tarfile.open(path, "r:gz") as archive:
        members = [m for m in archive.getmembers() if m.name == "package/package.json"]
        require(len(members) == 1, "package metadata is missing or ambiguous")
        member = members[0]
        require(member.isfile() and member.size <= 65536, "invalid package metadata")
        with archive.extractfile(member) as stream:
            metadata = json.load(stream)
    require(metadata["name"] == PACKAGE, "package name differs")
    require(metadata["version"] == entry["version"], "package version differs")
    require(metadata["repository"]["url"] == "git+https://github.com/zumen-jp/zumen.git", "package repository differs")


def verify_attestation(path, entry):
    # 元のビルドの出所を確かめる。再公開の npm provenance は実行中の main を指す。
    subprocess.run([
        "gh", "attestation", "verify", str(path), "--repo", REPOSITORY,
        "--signer-workflow", f"{REPOSITORY}/.github/workflows/release.yml",
        "--signer-digest", entry["commit"], "--source-digest", entry["commit"],
        "--source-ref", f"refs/tags/{entry['tag']}", "--deny-self-hosted-runners",
    ], check=True)


def require_unpublished(entry):
    request = urllib.request.Request(
        f"https://registry.npmjs.org/@zumen-jp%2fzumen/{entry['version']}",
        headers={"User-Agent": "zumen-release (github.com/zumen-jp/zumen)"},
    )
    try:
        with urllib.request.urlopen(request, timeout=30):
            raise ValueError("npm version is already published; inspect its integrity and provenance")
    except urllib.error.HTTPError as error:
        if error.code != 404:
            raise


def prepare(tag, out):
    entry = retry_for(tag)
    ref = gh_json(f"git/ref/tags/{tag}")
    require(ref["ref"] == f"refs/tags/{tag}", "git ref differs")
    commit = resolve_commit(ref, lambda sha: gh_json(f"git/tags/{sha}"))
    require(commit == entry["commit"], "tag commit differs")
    validate_release(gh_json(f"releases/tags/{tag}"), entry)
    out.mkdir(parents=True, exist_ok=True)
    path = out / entry["asset"]
    require(not path.exists(), "output already exists")
    subprocess.run([
        "gh", "release", "download", tag, "--repo", REPOSITORY,
        "--pattern", entry["asset"], "--dir", str(out),
    ], check=True)
    validate_tarball(path, entry)
    verify_attestation(path, entry)
    require_unpublished(entry)
    return path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("tag")
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    path = prepare(args.tag, args.out)
    print(f"Verified npm retry asset: {path}")


if __name__ == "__main__":
    main()
