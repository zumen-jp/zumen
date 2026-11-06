"""Cargo の SBOM を正規化し、実際の配布バイナリの依存と照合する（決定 0279）。"""

import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess
import tempfile
import tomllib
from datetime import datetime, timezone
from pathlib import Path
from urllib.parse import quote, unquote

from licenses import MANIFESTS, MODULES, ROOT, TARGETS, WASM, feature_args, read_info, wasm_name

DRAFT = "cargo.cdx.json"
REGISTRY = "registry+https://github.com/rust-lang/crates.io-index"
CDX_VERSION = "0.5.9"
SOURCE_URL = "git+https://github.com/zumen-jp/zumen@"
TOOL = {"vendor": "CycloneDX", "name": "cargo-cyclonedx", "version": CDX_VERSION}


def check(condition, message):
    if not condition:
        raise ValueError(message)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def encode(value):
    return (json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")) + "\n").encode()


def bom_ref(source, name, version):
    check(source in {"local", "crates.io"}, "依存の入手先を識別できない")
    return f"urn:zumen:cargo:{source}:{name}@{version}"


def source_of(package, members):
    if package["id"] in members and package["source"] is None:
        return "local"
    check(package["source"] == REGISTRY, "workspace と crates.io 以外の依存の識別規則が要る")
    return "crates.io"


def dependency_graph(metadata, root):
    """通常・build の辺をたどる。proc-macro を通る道は build、両方から使う crate は実行時とする。"""
    packages = {p["id"]: p for p in metadata["packages"]}
    nodes = {n["id"]: n for n in metadata["resolve"]["nodes"]}
    edges = {}
    runtime = set()
    normal = set()
    seen = set()
    # in_normal は cargo-about の対象（Cargo の通常の辺）、at_runtime は埋込形式の区分。
    todo = [(root, True, True)]
    while todo:
        key, in_normal, at_runtime = todo.pop()
        if (key, in_normal, at_runtime) in seen:
            continue
        seen.add((key, in_normal, at_runtime))
        if in_normal:
            normal.add(key)
        if at_runtime:
            runtime.add(key)
        children = edges.setdefault(key, set())
        for dep in nodes[key]["deps"]:
            kinds = {kind["kind"] for kind in dep["dep_kinds"]}
            if not kinds.intersection({None, "build"}):
                continue
            child = dep["pkg"]
            children.add(child)
            child_normal = None in kinds
            proc_macro = any("proc-macro" in target["kind"] for target in packages[child]["targets"])
            todo.append((child, in_normal and child_normal, at_runtime and child_normal and not proc_macro))
    return edges, normal, runtime


def reject_paths(value):
    """生成元の追加項目や、パーセント符号化した file URL にも端末のパスを残さない。"""
    if isinstance(value, dict):
        for key, item in value.items():
            reject_paths(key)
            reject_paths(item)
    elif isinstance(value, list):
        for item in value:
            reject_paths(item)
    elif isinstance(value, str):
        for _ in range(4):
            decoded = unquote(value)
            if decoded == value:
                break
            value = decoded
        check(not re.search(r"(?i)(?:file:|(?<![a-z0-9])[a-z]:[/\\]|\\|(?:^|[\s=\"'(])/)", value),
              "SBOM にローカルのパスがある")


def properties(document):
    items = document["metadata"]["properties"]
    values = {item["name"]: item["value"] for item in items}
    check(len(items) == len(values), "SBOM の属性が重複している")
    return values


def normalize(raw, metadata, notice, artifact, module, revision, lock, epoch):
    check(raw["bomFormat"] == "CycloneDX" and raw["specVersion"] == "1.5", "SBOM の形式が違う")
    check("serialNumber" not in raw and raw["metadata"]["tools"] == [TOOL], "SBOM の生成ツールか識別子が違う")
    timestamp = datetime.fromtimestamp(int(epoch), timezone.utc)
    check(datetime.fromisoformat(raw["metadata"]["timestamp"]) == timestamp, "SBOM の生成時刻が違う")
    checksums = {(p["name"], p["version"]): p["checksum"] for p in tomllib.loads(lock.decode())["package"]
                 if p.get("source") == REGISTRY}
    target, crate = notice["target"], notice["package"]
    check(properties(raw).get("cdx:rustc:sbom:target:triple") == target, "SBOM の target が違う")
    check(notice["features"] == module, "ライセンスの feature が違う")
    packages = {p["id"]: p for p in metadata["packages"]}
    root = metadata["resolve"]["root"]
    root_component = raw["metadata"]["component"]
    check(root_component["bom-ref"] == root and packages[root]["name"] == crate, "SBOM の対象 crate が違う")
    check(root_component["name"] == ("zumen_abi1" if crate == "zumen-abi" else "zumen"), "SBOM の対象バイナリが違う")
    graph, normal, runtime = dependency_graph(metadata, root)
    members = set(metadata["workspace_members"])
    refs = {key: bom_ref(source_of(packages[key], members), packages[key]["name"], packages[key]["version"])
            for key in graph}
    check(len(set(refs.values())) == len(refs), "SBOM の依存を一意に識別できない")
    inventory = {(packages[key]["name"], packages[key]["version"]) for key in normal if key not in members}
    check(inventory == {(dep["name"], dep["version"]) for dep in notice["dependencies"]},
          "cargo-about と SBOM の第三者の通常依存が違う")

    raw_components = [root_component, *raw["components"]]
    check(len(raw_components) == len(refs) and {c["bom-ref"] for c in raw_components} == set(refs),
          "Cargo と SBOM の部品が違う")
    dependencies = {}
    for edge in raw["dependencies"]:
        parent, children = edge["ref"], edge.get("dependsOn", [])
        check(parent not in dependencies and len(set(children)) == len(children), "SBOM の辺が重複している")
        dependencies[parent] = set(children)
    check(graph == dependencies, "Cargo と SBOM の依存関係が違う")

    def component(original):
        key = original["bom-ref"]
        pkg = packages[key]
        name, version = pkg["name"], pkg["version"]
        check(original["version"] == version and (key == root or original["name"] == name), "SBOM の名前か版が違う")
        check(original["scope"] == ("required" if key in normal else "excluded"), "Cargo と元の SBOM の区分が違う")
        purl = f"pkg:cargo/{name}@{version}"
        if key in members:
            directory = Path(pkg["manifest_path"]).parent.relative_to(metadata["workspace_root"]).as_posix()
            purl += "?vcs_url=" + quote(SOURCE_URL + revision, safe="") + "#" + quote(directory, safe="/")
        else:
            check(original.get("hashes") == [{"alg": "SHA-256", "content": checksums.get((name, version))}]
                  and (name, version) in checksums, "依存のハッシュが Cargo.lock と違う")
        item = {field: original[field] for field in ("type", "name", "version", "licenses", "hashes") if field in original}
        item.update({"bom-ref": refs[key], "scope": "required" if key in runtime else "excluded", "purl": purl})
        return item

    result = {
        "bomFormat": "CycloneDX", "specVersion": "1.5", "version": 1,
        "metadata": {"timestamp": raw["metadata"]["timestamp"], "tools": [TOOL], "component": component(root_component),
                     "properties": [{"name": k, "value": v} for k, v in sorted({
                         "cdx:rustc:sbom:target:triple": target, "zumen:crate": crate, "zumen:features": module,
                         "zumen:artifact": artifact, "zumen:commit": revision, "zumen:lock-sha256": digest(lock),
                         "zumen:source-date-epoch": str(epoch),
                     }.items())]},
        "components": sorted((component(c) for c in raw["components"]), key=lambda c: c["bom-ref"]),
        "dependencies": sorted(({"ref": refs[key], "dependsOn": sorted(refs[c] for c in children)}
                                for key, children in dependencies.items()), key=lambda edge: edge["ref"]),
    }
    reject_paths(result)
    return result


def raw_command(command, cwd, env=None):
    result = subprocess.run(command, cwd=cwd, env=env, capture_output=True, check=False)
    # cargo-cyclonedx の生の出力には作業端末のパスがある。
    check(result.returncode == 0, f"{Path(command[0]).stem} が失敗した（終了コード {result.returncode}）")
    return result.stdout


def metadata_for(stage, manifest, target, module):
    return json.loads(raw_command(["cargo", "metadata", "--format-version", "1", "--locked",
                                   "--manifest-path", manifest, "--filter-platform", target, *feature_args(module)], stage))


def source_revision():
    inputs = ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml", ".cargo", "crates", "bindings/cli", ":(glob)**/Cargo.toml"]
    changes = raw_command(["git", "--no-optional-locks", "status", "--porcelain", "--", *inputs], ROOT)
    check(not changes, "ビルドの入力に未コミットの変更がある")
    return raw_command(["git", "rev-parse", "HEAD"], ROOT).decode().strip()


def copy_workspace(stage):
    # Cargo が使う追跡ファイルだけを写す。生成物と作業端末のファイルを一時領域へ持ち込まない。
    meta = json.loads(raw_command(["cargo", "metadata", "--format-version", "1", "--locked", "--no-deps"], ROOT))
    roots = [Path(p["manifest_path"]).parent.relative_to(ROOT).as_posix() for p in meta["packages"]]
    paths = raw_command(["git", "ls-files", "-z", "--", "Cargo.toml", "Cargo.lock", "rust-toolchain.toml", ".cargo", *roots], ROOT)
    for entry in paths.split(b"\0"):
        if entry:
            name = os.fsdecode(entry)
            source = ROOT / name
            check(source.resolve().is_relative_to(ROOT), "workspace の外を指す入力がある")
            destination = stage / name
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(source, destination)


def generate(output, tool):
    check(raw_command([tool, "cyclonedx", "--version"], ROOT).decode().strip() == f"cargo-cyclonedx-cyclonedx {CDX_VERSION}",
          "cargo-cyclonedx の版が違う")
    revision = source_revision()
    epoch = raw_command(["git", "show", "-s", "--format=%ct", "HEAD"], ROOT).decode().strip()
    with tempfile.TemporaryDirectory(prefix="zumen-cdx-") as temporary:
        stage = Path(temporary)
        copy_workspace(stage)
        lock = (ROOT / "Cargo.lock").read_bytes()
        env = {**os.environ, "SOURCE_DATE_EPOCH": epoch, "CARGO_NET_OFFLINE": "true"}
        configs = []
        for t in TARGETS:
            for kind in MANIFESTS:
                configs.append((kind, t["target"], "default", t["library" if kind == "abi" else "exe"],
                                output / "licenses" / "native" / t["rid"] / kind))
        for module in MODULES:
            configs.append(("abi", WASM, module, wasm_name(module),
                            output / "licenses" / "wasm" / ("abi" if module == "default" else module)))
        for kind, target, module, artifact, directory in configs:
            manifest = MANIFESTS[kind]
            notice, _ = read_info(directory, "zumen-" + kind, target, module)
            metadata = metadata_for(stage, manifest, target, module)
            raw_command([tool, "cyclonedx", "--manifest-path", manifest, "--format", "json", "--spec-version", "1.5",
                         "--target", target, "--all", "--describe", "binaries", *feature_args(module)], stage, env)
            check((stage / "Cargo.lock").read_bytes() == lock, "SBOM 生成で Cargo.lock が変わった")
            files = list((stage / manifest).parent.glob("*.cdx.json"))
            check(len(files) == 1, "対象 crate の SBOM が一意でない")
            raw = json.loads(files[0].read_bytes())
            bom = normalize(raw, metadata, notice, artifact, module, revision, lock, epoch)
            (directory / DRAFT).write_bytes(encode(bom))
            print(f"{target} / {kind} / {module}: SBOM {1 + len(bom['components'])} 件")


def extracted_graph(binary):
    with tempfile.TemporaryDirectory(prefix="zumen-audit-") as temporary:
        path = Path(temporary) / "artifact"
        path.write_bytes(binary)
        data = json.loads(raw_command(["rust-audit-info", str(path), str(len(binary)), str(8 * 1024 * 1024)], temporary))
    check(data.get("format") == 1, "埋込情報の形式が違う")
    packages = data["packages"]
    refs = [bom_ref(p["source"], p["name"], p["version"]) for p in packages]
    roots = [refs[i] for i, p in enumerate(packages) if p.get("root", False)]
    check(len(roots) == 1 and len(refs) == len(set(refs)), "埋込情報の起点か ID が一意でない")
    graph = {}
    for ref, package in zip(refs, packages):
        indices = package.get("dependencies", [])
        check(all(type(i) is int and 0 <= i < len(refs) for i in indices) and len(indices) == len(set(indices)),
              "埋込情報の依存先が不正")
        graph[ref] = (package.get("kind", "runtime"), {refs[i] for i in indices})
    return roots[0], graph


def bind(directory, notice, artifact, binary):
    document = json.loads((directory / DRAFT).read_bytes())
    props = properties(document)
    check((props["zumen:artifact"], props["zumen:crate"], props["cdx:rustc:sbom:target:triple"], props["zumen:features"])
          == (artifact, notice["package"], notice["target"], notice["features"]), "SBOM の配布対象が違う")
    document["metadata"]["component"]["hashes"] = [{"alg": "SHA-256", "content": digest(binary)}]
    return encode(document)


def document_graph(doc):
    root = doc["metadata"]["component"]
    components = [root, *doc["components"]]
    refs = {c["bom-ref"] for c in components}
    deps = {e["ref"]: set(e["dependsOn"]) for e in doc["dependencies"]}
    check(len(components) == len(refs) and len(deps) == len(doc["dependencies"]) and set(deps) == refs,
          "配布した SBOM の参照が重複しているか解決できない")
    for edge in doc["dependencies"]:
        targets = deps[edge["ref"]]
        check(len(targets) == len(edge["dependsOn"]) and targets <= refs, "配布した SBOM の依存先が不正")
    graph = {c["bom-ref"]: ({"required": "runtime", "excluded": "build"}[c["scope"]], deps[c["bom-ref"]])
             for c in components}
    return root["bom-ref"], graph


def verify(binary, shipped, expected):
    check(shipped == expected, "配布した SBOM が対象の SBOM と違う")
    doc = json.loads(shipped)
    reject_paths(doc)
    check(doc["metadata"]["component"]["hashes"] == [{"alg": "SHA-256", "content": digest(binary)}],
          "配布バイナリのハッシュが SBOM と違う")
    check(extracted_graph(binary) == document_graph(doc), "配布バイナリの埋込情報と SBOM の依存・区分・辺が違う")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--cargo-cyclonedx", default="cargo-cyclonedx")
    args = parser.parse_args()
    generate(args.output.resolve(), args.cargo_cyclonedx)
