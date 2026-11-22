"""公開済み Release の npm 配布物を、来歴を照合して再公開用に用意する（決定 0294）。"""

import argparse
import base64
import hashlib
import json
import re
import subprocess
import tarfile
import urllib.error
import urllib.request
from pathlib import Path

REPOSITORY = "zumen-jp/zumen"
RETRIES = {
    "v0.2.0": {
        "commit": "ce01f02dc6725914f43627b3cdb353007dbd83dc",
        "packages": [
            ("zumen", "da0e5b5701bd06b6f023167aa763e082b1e6d7c76a7765168c38a24f2b2bcaa0"),
        ],
    },
    "v0.4.0": {
        "commit": "1b8cdee7b32a52f181ca781b449a5bd12e788bf7",
        "packages": [
            ("view2d", "4c4d65a301fd33faa19a2fd7f248c81b5320cd0ea85f6df759db9eb348569fbf"),
            ("view3d", "b13fc9bf2dd2192213341d20100daeb7b502de9e9fc62ac490c2a688d4367ddf"),
            ("react", "ebb502aa641c0568d3712c8d92cdaf6fb71e78a8970b29ebd768e9330e3bce33"),
        ],
    },
}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def retry_for(tag):
    require(tag in RETRIES, "tag is not approved for npm retry")
    release = RETRIES[tag]
    version = tag.removeprefix("v")
    return [
        {"tag": tag, "version": version, "commit": release["commit"], "name": "@zumen-jp/" + name,
         "asset": f"zumen-jp-{name}-{version}.tgz", "sha256": digest}
        for name, digest in release["packages"]
    ]


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
    validate_metadata(metadata, entry)


def validate_metadata(metadata, entry):
    require(metadata["name"] == entry["name"], "package name differs")
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


def needs_publish(entry, path):
    encoded_name = entry["name"].replace("/", "%2f")
    request = urllib.request.Request(
        f"https://registry.npmjs.org/{encoded_name}/{entry['version']}",
        headers={"User-Agent": "zumen-release (github.com/zumen-jp/zumen)"},
    )
    try:
        with urllib.request.urlopen(request, timeout=30) as response:
            require(entry["tag"] != "v0.2.0", "npm version is already published; inspect its integrity and provenance")
            metadata = json.load(response)
    except urllib.error.HTTPError as error:
        if error.code == 404:
            return True
        raise
    validate_metadata(metadata, entry)
    with path.open("rb") as stream:
        digest = hashlib.file_digest(stream, "sha512").digest()
    integrity = "sha512-" + base64.b64encode(digest).decode("ascii")
    require(metadata["dist"]["integrity"] == integrity, "published tarball integrity differs")
    print(f"Verified existing npm version: {entry['name']}@{entry['version']}")
    return False


def prepare(tag, out):
    entries = retry_for(tag)
    ref = gh_json(f"git/ref/tags/{tag}")
    require(ref["ref"] == f"refs/tags/{tag}", "git ref differs")
    commit = resolve_commit(ref, lambda sha: gh_json(f"git/tags/{sha}"))
    require(commit == entries[0]["commit"], "tag commit differs")
    release = gh_json(f"releases/tags/{tag}")
    for entry in entries:
        validate_release(release, entry)
    out.mkdir(parents=True, exist_ok=True)
    pending = []
    for entry in entries:
        path = out / entry["asset"]
        require(not path.exists(), "output already exists")
        subprocess.run([
            "gh", "release", "download", tag, "--repo", REPOSITORY,
            "--pattern", entry["asset"], "--dir", str(out),
        ], check=True)
        validate_tarball(path, entry)
        verify_attestation(path, entry)
        if needs_publish(entry, path):
            pending.append(path)
    return pending


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("tag")
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--pending-file", type=Path, help="未公開の tarball のパスを公開順に記録する")
    args = parser.parse_args()
    pending = prepare(args.tag, args.out)
    if args.pending_file:
        with args.pending_file.open("x", encoding="utf-8", newline="\n") as stream:
            for path in pending:
                print(path.resolve(), file=stream)
    print(f"Verified npm retry: {len(pending)} package(s) to publish.")


if __name__ == "__main__":
    main()
