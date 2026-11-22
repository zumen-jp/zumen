"""v0.4.0 の zumen-stbridge を初回だけ手動公開するため、前後の crate と公開済みの SHA-256 を確かめる。"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
from urllib.error import HTTPError
from urllib.request import urlopen

VERSION = "0.4.0"
FIRST = "zumen-stbridge"


def groups(packages, publish_args):
    """Cargo の通常・build 依存から、初回公開より先に必要な crate を推移的に求める。"""
    names = publish_args[1::2]
    if publish_args[::2] != ["-p"] * len(names) or len(names) != len(set(names)):
        raise ValueError("CRATES の -p 指定が不正または重複している")
    public = {p["name"]: p for p in packages if p["publish"] != []}
    if set(names) != public.keys() or any(p["version"] != VERSION for p in public.values()):
        raise ValueError("CRATES がワークスペースの公開対象または版と一致しない")
    dependencies = {name: {d["name"] for d in pkg["dependencies"] if d.get("path") and d["kind"] != "dev"}
                    for name, pkg in public.items()}
    required = set()
    todo = list(dependencies[FIRST])
    while todo:
        name = todo.pop()
        if name not in required:
            required.add(name)
            todo.extend(dependencies[name])
    if not required or FIRST in required:
        raise ValueError("初回公開する crate の依存が空か循環している")
    return {"before": [n for n in names if n in required],
            "after": [n for n in names if n != FIRST and n not in required]}


def registry_digest(name):
    # 対象は zumen で始まる crate。Cargo の sparse index の公式パス（先頭 2 字 / 次の 2 字 / 名前）。
    url = f"https://index.crates.io/{name[:2]}/{name[2:4]}/{name}"
    try:
        with urlopen(url, timeout=30) as response:
            for line in response:
                entry = json.loads(line)
                if entry["vers"] == VERSION:
                    if entry["name"] != name or entry["yanked"]:
                        raise ValueError(f"{name} の名前が違うか、この版が yank されている")
                    return entry["cksum"]
    except HTTPError as error:
        if error.code != 404:
            raise
        error.close()
    return None


def cargo_args(names):
    return [arg for name in names for arg in ("-p", name)]


def package_digest(directory, name):
    with (directory / f"{name}-{VERSION}.crate").open("rb") as package:
        return hashlib.file_digest(package, "sha256").hexdigest()


def remaining(names, remote, local):
    for name, digest in remote.items():
        if digest is not None and digest != local(name):
            raise ValueError(f"{name} {VERSION} の crates.io の SHA-256 がタグの梱包結果と違う")
    return [n for n in names if remote[n] is None]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=["check", "before", "after"])
    parser.add_argument("--first-sha256")
    args = parser.parse_args()
    metadata = json.loads(subprocess.check_output(["cargo", "metadata", "--no-deps", "--locked", "--format-version", "1"]))
    plan = groups(metadata["packages"], os.environ["CRATES"].split())
    if args.action == "check":
        for key, stage in plan.items():
            print(f"{key} ({len(stage)}): {' '.join(stage)}")
        return
    if os.environ.get("GITHUB_REF") != f"refs/tags/v{VERSION}" or os.environ.get("GITHUB_EVENT_NAME") != "push":
        raise ValueError("初回用の公開対象は v0.4.0 のタグを push した実行でだけ出す")
    if args.action == "after" and (not args.first_sha256 or registry_digest(FIRST) != args.first_sha256):
        raise ValueError("zumen-stbridge 0.4.0 の手動公開を待つ。公開前またはタグの梱包結果と SHA-256 が違う")
    names = plan[args.action]
    remote = {n: registry_digest(n) for n in names}
    if any(remote.values()):
        # 再実行時、同じ名前と版だけを根拠に済んだとは見なさない。
        subprocess.run(["cargo", "package", "--locked", "--no-verify", *cargo_args(names)], check=True)
    directory = Path(metadata["target_directory"]) / "package"
    print(" ".join(cargo_args(remaining(names, remote, lambda n: package_digest(directory, n)))))


if __name__ == "__main__":
    main()
