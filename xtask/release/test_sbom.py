"""SBOM と埋込情報の区分、参照、正規化と取り違えを確かめる。"""

import copy
import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import sbom

VERSION = "1.0.0"


def fixture(directory):
    root = directory / "crates" / "zumen-abi"
    root_id = "path+" + root.as_uri() + "#" + VERSION
    names = ("normal", "macro", "tool", "both", "generated", "dev")
    keys = {name: sbom.REGISTRY + "#" + name + "@" + VERSION for name in names}
    keys["root"] = root_id
    deps = {"root": [("normal", None), ("tool", "build"), ("macro", None), ("dev", "dev")],
            "normal": [("both", None)], "tool": [("both", None)], "macro": [("both", None), ("generated", None)]}
    packages = [{"id": keys[n], "name": n, "version": VERSION, "source": sbom.REGISTRY,
                 "targets": [{"kind": ["proc-macro" if n == "macro" else "lib"]}]} for n in names]
    packages.append({"id": root_id, "name": "zumen-abi", "manifest_path": str(root / "Cargo.toml"), "source": None,
                     "version": VERSION, "targets": [{"kind": ["cdylib"]}]})
    nodes = [{"id": key, "deps": [{"pkg": keys[c], "dep_kinds": [{"kind": kind}]} for c, kind in deps.get(n, [])]}
             for n, key in keys.items()]
    meta = {"packages": packages, "workspace_root": str(directory), "workspace_members": [root_id],
            "resolve": {"root": root_id, "nodes": nodes}}
    checksum = "a" * 64
    lock = "".join(f'[[package]]\nname = "{n}"\nversion = "{VERSION}"\nsource = "{sbom.REGISTRY}"\nchecksum = "{checksum}"\n'
                   for n in names).encode()
    components = [{"bom-ref": keys[n], "name": n, "version": VERSION, "scope": "excluded" if n == "tool" else "required",
                   "type": "library", "hashes": [{"alg": "SHA-256", "content": checksum}]} for n in names if n != "dev"]
    top = {"bom-ref": root_id, "type": "library", "name": "zumen_abi1", "version": VERSION, "scope": "required"}
    notice = {"package": "zumen-abi", "target": sbom.WASM, "features": "drawing",
              "dependencies": [{"name": n, "version": VERSION} for n in ("normal", "macro", "both", "generated")]}
    edges = [{"ref": key, "dependsOn": [keys[c] for c, kind in deps.get(n, []) if kind != "dev"]}
             for n, key in keys.items() if n != "dev"]
    raw = {"bomFormat": "CycloneDX", "specVersion": "1.5", "metadata": {
        "tools": [sbom.TOOL], "timestamp": "1970-01-01T00:00:01Z", "component": top,
        "properties": [{"name": "cdx:rustc:sbom:target:triple", "value": sbom.WASM}]},
        "components": components, "dependencies": edges}
    return raw, meta, notice, "zumen_abi1_drawing.wasm", "drawing", "1" * 40, lock, "1"


class SbomTests(unittest.TestCase):
    def test_different_directories_and_order_produce_identical_bytes(self):
        with tempfile.TemporaryDirectory() as tmp:
            inputs = fixture(Path(tmp) / "first")
            other = fixture(Path(tmp) / "second")
            other[0]["components"].reverse()
            other[0]["dependencies"].reverse()
            for edge in other[0]["dependencies"]:
                edge["dependsOn"].reverse()
            document = sbom.normalize(*inputs)
            self.assertEqual(sbom.encode(document), sbom.encode(sbom.normalize(*other)))
            self.assertNotIn(b"path+file", sbom.encode(document))
            self.assertEqual({c["name"]: c["scope"] for c in document["components"]},
                             {"both": "required", "normal": "required", "macro": "excluded",
                              "generated": "excluded", "tool": "excluded"})

    def test_rejects_bad_references_lockfile_timestamps_features_and_licenses(self):
        with tempfile.TemporaryDirectory() as tmp:
            inputs = fixture(Path(tmp))
            for change in (
                lambda r, n: r["dependencies"][0]["dependsOn"].append("missing"),
                lambda r, n: r["dependencies"].append(r["dependencies"][0]),
                lambda r, n: r["components"].append(r["components"][0]),
                lambda r, n: r["components"][0]["hashes"][0].update(content="0" * 64),
                lambda r, n: r["metadata"].update(timestamp="2000-01-01T00:00:00Z"),
                lambda r, n: r.update(serialNumber="urn:uuid:random"),
                lambda r, n: n.update(features="bim"),
                lambda r, n: n.update(target="x86_64-pc-windows-msvc"),
                lambda r, n: n["dependencies"].pop(),
            ):
                with self.subTest(change=change), self.assertRaises(ValueError):
                    args = copy.deepcopy(inputs)
                    change(args[0], args[2])
                    sbom.normalize(*args)

    def test_path_qualifiers_and_escaped_paths(self):
        for value in (r"C:\work\lib", "/runner/work/lib", r"\\server\share", "download_url=file%253A%252F%252Ftmp"):
            with self.subTest(value=value), self.assertRaises(ValueError):
                sbom.reject_paths({"property": [value]})

    def test_binary_and_independent_graph_must_match(self):
        with tempfile.TemporaryDirectory() as tmp:
            directory = Path(tmp)
            inputs = fixture(directory)
            doc = sbom.normalize(*inputs)
            (directory / sbom.DRAFT).write_bytes(sbom.encode(doc))
            binary = b"synthetic"
            shipped = sbom.bind(directory, inputs[2], inputs[3], binary)
            data = json.loads(shipped)
            root = data["metadata"]["component"]["bom-ref"]
            nodes = [data["metadata"]["component"], *data["components"]]
            ids = {c["bom-ref"]: i for i, c in enumerate(nodes)}
            edges = {e["ref"]: e["dependsOn"] for e in data["dependencies"]}
            packages = [{"source": "local" if c["bom-ref"] == root else "crates.io",
                         "name": "zumen-abi" if c["bom-ref"] == root else c["name"], "version": c["version"],
                         "kind": "build" if c["scope"] == "excluded" else "runtime", "root": c["bom-ref"] == root,
                         "dependencies": [ids[k] for k in edges[c["bom-ref"]]]} for c in nodes]
            with patch("sbom.raw_command", return_value=sbom.encode({"format": 1, "packages": packages})):
                sbom.verify(binary, shipped, shipped)
                with self.assertRaisesRegex(ValueError, "ハッシュ"):
                    sbom.verify(b"different binary", shipped, shipped)
            for change in (lambda p: p[0]["dependencies"].pop(), lambda p: p[0].update(kind="build"),
                           lambda p: p[0]["dependencies"].append(-1), lambda p: p[0].update(root=False)):
                mutated = copy.deepcopy(packages)
                change(mutated)
                with patch("sbom.raw_command", return_value=sbom.encode({"format": 1, "packages": mutated})):
                    with self.assertRaises(ValueError):
                        sbom.verify(binary, shipped, shipped)
