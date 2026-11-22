"""検査した配布物を Swift・Go の公開鏡の履歴につなぐ。plan は読み取りだけ。"""

import argparse
import json
import os
import re
import subprocess
import tempfile
from pathlib import Path, PurePosixPath

from package import VERSION, read_archive, require

REPO = "zumen-jp/zumen"
MIRRORS = {"swift": "https://github.com/zumen-jp/zumen-swift.git", "go": "https://github.com/zumen-jp/zumen-go.git"}
VERSION_TAG = re.compile(r"v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)"
                         r"(?:-([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?\Z")


def version_order(tag):
    match = VERSION_TAG.fullmatch(tag)
    if match is None:
        return None
    prerelease = match[4].split(".") if match[4] else []
    if any(item.isdigit() and len(item) > 1 and item.startswith("0") for item in prerelease):
        return None
    parts = tuple((0, int(item)) if item.isdigit() else (1, item) for item in prerelease)
    return (*map(int, match.groups()[:3]), not prerelease, parts)


def asset_name(kind, tag):
    return f"zumen-{kind}-{tag[1:]}" + ("-source.tar.gz" if kind == "swift" else ".tar.gz")


def source_files(kind, tag, archive):
    prefix = f"zumen-swift-{tag[1:]}-source/" if kind == "swift" else ""
    files = {}
    for name, data in read_archive(archive).items():
        require(name.startswith(prefix), "鏡用アーカイブの先頭のフォルダーが違う")
        path = name.removeprefix(prefix)
        parts = PurePosixPath(path).parts
        require(parts and path == "/".join(parts) and not PurePosixPath(path).is_absolute()
                and not any(part.lower() in (".git", "..") for part in parts)
                and not any(char in path for char in "\0\r\n\t\\:"), "鏡用アーカイブのパスが不正")
        files[path] = data
    require(("Package.swift" if kind == "swift" else "go.mod") in files, "言語のパッケージ定義がない")
    return files


def previous_files(kind, tag):
    # 前版で配ったファイルだけを更新対象にする。鏡に後から追加された運用ファイルは残す。
    with tempfile.TemporaryDirectory() as temporary:
        archive = Path(temporary) / asset_name(kind, tag)
        sha = subprocess.check_output(["gh", "api", f"repos/{REPO}/commits/{tag}", "--jq", ".sha"], text=True).strip()
        subprocess.run(["gh", "release", "download", tag, "--repo", REPO, "--pattern", archive.name,
                        "--dir", temporary], check=True)
        subprocess.run(["gh", "attestation", "verify", archive, "--repo", REPO,
                        "--signer-workflow", REPO + "/.github/workflows/release.yml",
                        "--source-ref", "refs/tags/" + tag, "--source-digest", sha], check=True)
        return source_files(kind, tag, archive)


def git(repo, *args, data=None, env=None):
    return subprocess.check_output(["git", "-c", "credential.helper=", "-c", "http.followRedirects=false",
                                    "-c", "core.hooksPath=/dev/null", *args], cwd=repo, input=data, env=env)


def entries(repo, revision):
    items = {}
    for row in git(repo, "ls-tree", "-rz", revision).split(b"\0"):
        if row:
            header, path = row.split(b"\t", 1)
            mode, _, oid = header.split()
            items[path.decode()] = (mode, oid)
    return items


def blobs(repo, files):
    return {name: (b"100644", git(repo, "hash-object", "-w", "--stdin", data=content).strip())
            for name, content in files.items()}


def release_tags(repo, revision):
    tags = git(repo, "tag", "--merged", revision).decode().splitlines()
    return [(order, tag) for tag in tags if (order := version_order(tag)) is not None]


def prepare(repo, remote, kind, tag, files, get_previous):
    version = version_order(tag)
    require(version, "鏡のタグには v とパッケージの版数を指定する")
    repo.mkdir(parents=True)
    git(repo, "init", "--bare", "--quiet")
    git(repo, "fetch", "--quiet", "--no-tags", remote, "refs/heads/*:refs/remotes/mirror/*", "refs/tags/*:refs/tags/*")
    desired = blobs(repo, files)
    if git(repo, "tag", "--list", tag).strip():
        ref = "refs/tags/" + tag
        published = entries(repo, ref)
        require(all(published.get(k) == val for k, val in desired.items()), "鏡の既存タグの中身が配布物と違う")
        earlier = [(order, old_tag) for order, old_tag in release_tags(repo, ref) if order < version]
        if earlier:
            old_files = get_previous(kind, max(earlier)[1])
            require(not ((old_files.keys() - desired.keys()) & published.keys()), "鏡の既存タグに前版のファイルが残っている")
        return None

    default = git(repo, "ls-remote", "--symref", remote, "HEAD").splitlines()
    if default:
        require(default[0].startswith(b"ref: refs/heads/") and default[0].endswith(b"\tHEAD"), "鏡の既定ブランチが不明")
        branch = default[0][len(b"ref: refs/heads/"):-len(b"\tHEAD")].decode()
    else:
        require(not git(repo, "for-each-ref", "refs/remotes/mirror", "refs/tags").strip(), "空でない鏡の既定ブランチが不明")
        branch = "main"
    git(repo, "check-ref-format", "refs/heads/" + branch)
    current = git(repo, "rev-parse", "--verify", "refs/remotes/mirror/" + branch).strip().decode() if default else None
    before = entries(repo, current) if current else {}
    previous = {}
    if current:
        releases = release_tags(repo, current)
        if releases:
            numbers, old_tag = max(releases)
            require(numbers < version, "鏡の既定ブランチに新しい版がある")
            previous = blobs(repo, get_previous(kind, old_tag))
            old_tree = entries(repo, "refs/tags/" + old_tag)
            require(all(old_tree.get(k) == val for k, val in previous.items()), "鏡の前版と検証済み配布物が違う")
        git(repo, "read-tree", current)
    else:
        git(repo, "read-tree", "--empty")
    planned = before.copy()
    update = bytearray()
    null_oid = b"0" * len(next(iter(desired.values()))[1])
    for name in sorted(previous.keys() | desired.keys()):
        item = desired.get(name)
        current_item = before.get(name)
        require(current_item in (item, previous.get(name)), f"配布物と鏡の変更が衝突している: {name}")
        if item == current_item:
            continue
        if item:
            planned[name] = item
        else:
            planned.pop(name)
        mode, oid = item or (b"0", null_oid)
        update.extend(mode + b" " + oid + b"\t" + name.encode() + b"\0")
    git(repo, "update-index", "-z", "--index-info", data=update)
    tree = git(repo, "write-tree").strip().decode()
    require(entries(repo, tree) == planned, "鏡にある配布物以外のファイルと衝突している")
    commit = current
    if not current or before != planned:
        parent = ["-p", current] if current else []
        commit = git(repo, "-c", "user.name=github-actions[bot]", "-c",
                     "user.email=41898282+github-actions[bot]@users.noreply.github.com",
                     "commit-tree", tree, *parent, "-m", f"chore(release): {tag} の配布物を反映する").strip().decode()
    return {"branch": branch, "commit": commit}


def push(repo, remote, tag, plan, env=None):
    if plan:
        commit, branch = plan["commit"], plan["branch"]
        git(repo, "push", "--atomic", "--", remote, f"{commit}:refs/heads/{branch}", f"{commit}:refs/tags/{tag}", env=env)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("operation", choices=("plan", "push"))
    parser.add_argument("--kind", choices=tuple(MIRRORS), required=True)
    parser.add_argument("--tag", required=True)
    parser.add_argument("--work", type=Path, required=True)
    parser.add_argument("--artifacts", type=Path)
    args = parser.parse_args()
    require(args.tag == f"v{VERSION}", "タグと配布物の版が違う")
    repo, record = args.work / "mirror.git", args.work / "plan.json"
    remote = MIRRORS[args.kind]
    if args.operation == "plan":
        require(args.artifacts is not None, "同じ実行の配布物のフォルダーを指定する")
        owner_repo = f"zumen-jp/zumen-{args.kind}"
        metadata = json.loads(subprocess.check_output(["gh", "api", "repos/" + owner_repo]))
        require(metadata["full_name"] == owner_repo and metadata["visibility"] == "public"
                and not metadata["archived"] and not metadata["disabled"], "準備した公開の鏡を確認できない")
        files = source_files(args.kind, args.tag, args.artifacts / asset_name(args.kind, args.tag))
        plan = prepare(repo, remote, args.kind, args.tag, files, previous_files)
        record.write_text(json.dumps(plan), encoding="utf-8")
    else:
        plan = json.loads(record.read_bytes())
        if plan is None:
            print("同じ配布物のタグを確認した")
            return
        require(os.environ.get("MIRROR_PUBLISH_TOKEN"), "鏡への公開用の資格情報が設定されていない")
        helper = args.work.resolve() / "askpass.sh"
        helper.write_text('#!/bin/sh\ncase "$1" in Username*) echo x-access-token;; Password*) printf "%s\\n" "$MIRROR_PUBLISH_TOKEN";; *) exit 1;; esac\n',
                          encoding="ascii")
        helper.chmod(0o700)
        push(repo, remote, args.tag, plan, {**os.environ, "GIT_ASKPASS": str(helper), "GIT_TERMINAL_PROMPT": "0",
                                           "LC_ALL": "C"})


if __name__ == "__main__":
    main()
