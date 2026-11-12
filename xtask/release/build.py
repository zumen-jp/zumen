"""配る Cargo target を作り、コンパイラーが返した feature とファイルのハッシュを記録する。"""

import argparse
import json
import re
import shutil
import subprocess
import tomllib
from pathlib import Path

from licenses import MANIFESTS, MODULES, ROOT, TARGETS, WASM, feature_args, wasm_name
from linux_abi import verify_environment, verify_linux
from sbom import DRAFT, bom_ref, check, digest, document_graph, encode, metadata_for, properties, raw_command, source_of, source_revision


def target_tree(workspace, manifest, crate, target, module, kinds, names):
    """選んだ package・target・feature の Cargo の木から、深さを使って直接の辺を復元する。"""
    output = raw_command(["cargo", "tree", "--locked", "--manifest-path", str(manifest), "-p", crate, "--target", target,
                          *feature_args(module), "--edges", kinds, "--prefix", "depth", "--charset", "ascii",
                          "--color", "never", "--format", "|{p}|{f}|"], workspace)
    parents, edges = [], {}
    root_features = ()
    for line in output.decode("utf-8").splitlines():
        row = re.fullmatch(r"([0-9]+)\|(.+)\|([^|]*)\|(?: \(\*\))?", line)
        check(row is not None and row[2] in names, "cargo tree の部品をコンパイラーの package ID と照合できない")
        depth, ref = int(row[1]), names[row[2]]
        check(depth <= len(parents) and (depth != 0 or not parents), "cargo tree の深さか起点が不正")
        edges.setdefault(ref, set())
        if depth:
            edges[parents[depth - 1]].add(ref)
        else:
            root_features = tuple(sorted(row[3].split(","))) if row[3] else ()
        parents[depth:] = [ref]
    check(parents, "cargo tree に起点がない")
    return parents[0], root_features, edges


def build_graph(workspace, manifest, target, module, messages, root_id, enabled):
    """metadata は ID の翻訳だけに使い、依存の集合は実ビルド、辺は対象を絞った cargo tree から得る。"""
    metadata = metadata_for(workspace, str(manifest), target, module)
    packages = {p["id"]: p for p in metadata["packages"]}
    members = set(metadata["workspace_members"])
    compiled_ids = {m["package_id"] for m in messages if m["reason"] == "compiler-artifact"}
    refs, names = {}, {}
    for key in compiled_ids:
        pkg = packages[key]
        origin = source_of(pkg, members)
        ref = bom_ref(origin, pkg["name"], pkg["version"])
        refs[key] = ref
        # cargo tree の {p} はローカルのパスと proc-macro の印を含む。生の ID と表示名は保存しない。
        label = f"{pkg['name']} v{pkg['version']}"
        proc = " (proc-macro)" if any("proc-macro" in t["kind"] for t in pkg["targets"]) else ""
        path = f" ({Path(pkg['manifest_path']).parent})" if origin == "local" else ""
        names[label + proc + path] = ref
    check(len(names) == len(compiled_ids) == len(set(refs.values())), "コンパイラーの package ID を一意に識別できない")
    args = (workspace, manifest, packages[root_id]["name"], target, module)
    root, features, all_edges = target_tree(*args, "normal,build", names)
    runtime_root, runtime_features, runtime = target_tree(*args, "normal,no-proc-macro", names)
    check(root == runtime_root == refs[root_id] and features == runtime_features == tuple(sorted(enabled)),
          "ビルドの起点か有効な feature が cargo tree と違う")
    check(set(all_edges) == set(refs.values()), "コンパイラーと cargo tree の依存が違う")
    return {"compiled": sorted(refs.values()), "root": root,
            "tree": {ref: ["runtime" if ref in runtime else "build", sorted(children)] for ref, children in all_edges.items()}}


def compare_build_graph(record, document):
    root, graph = document_graph(document)
    check(record["compiled"] == sorted(graph), "コンパイラーの実際の依存と SBOM の部品が違う")
    expected = {key: [kind, sorted(children)] for key, (kind, children) in graph.items()}
    check(record["root"] == root and record["tree"] == expected, "対象と feature を絞った cargo tree と SBOM の辺・区分が違う")


def read_receipt(directory, template, notice, artifact, binary):
    record = json.loads((directory / (artifact + ".build.json")).read_bytes())
    document = json.loads((template / DRAFT).read_bytes())
    props = properties(document)
    expected = (notice["package"], notice["target"], notice["features"], artifact, digest(binary),
                props["zumen:commit"], props["zumen:lock-sha256"])
    actual = tuple(record[k] for k in ("package", "target", "features", "artifact", "sha256", "commit", "lock"))
    check(actual == expected, "ビルド時の crate・target・feature・バイナリと違う")
    compare_build_graph(record, document)
    return record


def build(kind, target, module, filename, shipped, output):
    revision = source_revision()
    lock_hash = digest((ROOT / "Cargo.lock").read_bytes())
    manifest = ROOT / MANIFESTS[kind]
    command = ["cargo", "auditable", "build", "--release", "--locked", "--target", target, "--manifest-path",
               str(manifest), "--message-format=json-render-diagnostics", *feature_args(module)]
    result = subprocess.run(command, cwd=ROOT, stdout=subprocess.PIPE, check=True)
    matches = []
    messages = [json.loads(line) for line in result.stdout.splitlines()]
    for message in messages:
        if message["reason"] == "compiler-artifact" and Path(message["manifest_path"]) == manifest:
            matches.extend((Path(path), message["features"], message["package_id"])
                           for path in message["filenames"] if Path(path).name == filename)
    check(len(matches) == 1, "コンパイラーの出力に対象のバイナリがないか複数ある")
    source, enabled, root_id = matches[0]
    if target.endswith("-linux-gnu"):
        verify_linux(source.read_bytes(), next(item for item in TARGETS if item["target"] == target))
    declared = tomllib.loads(manifest.read_text(encoding="utf-8")).get("features", {})
    if module == "default" and "default" in declared:
        check("default" in enabled, "既定の feature が有効でない")
    elif module != "default":
        check(set(MODULES).intersection(enabled) == {module}, "分けた wasm の feature が違う")
    graph = build_graph(ROOT, manifest, target, module, messages, root_id, enabled)
    output.mkdir(parents=True, exist_ok=True)
    # CLI を同じファイルから実行して検査するため、実行の権限も保つ。時刻はコピーしない。
    shutil.copy(source, output / shipped)
    receipt = {"package": "zumen-" + kind, "target": target, "features": module, "enabled": sorted(enabled),
               "artifact": shipped, "sha256": digest((output / shipped).read_bytes()), "commit": revision, "lock": lock_hash,
               **graph}
    (output / (shipped + ".build.json")).write_bytes(encode(receipt))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--rid", choices=["wasm", *(t["rid"] for t in TARGETS)], required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    if args.rid == "wasm":
        for module in MODULES:
            build("abi", WASM, module, "zumen_abi1.wasm", wasm_name(module), args.out)
    else:
        target = next(t for t in TARGETS if t["rid"] == args.rid)
        verify_environment(target)
        for kind, filename in (("abi", target["library"]), ("cli", target["exe"])):
            build(kind, target["target"], "default", filename, filename, args.out)
