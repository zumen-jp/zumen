"""配る Cargo target を作り、コンパイラーが返した feature とファイルのハッシュを記録する。"""

import argparse
import json
import shutil
import subprocess
import tomllib
from pathlib import Path

from licenses import MANIFESTS, MODULES, ROOT, TARGETS, WASM, feature_args, wasm_name
from sbom import DRAFT, check, digest, encode, properties, source_revision


def read_receipt(directory, template, notice, artifact, binary):
    record = json.loads((directory / (artifact + ".build.json")).read_bytes())
    props = properties(json.loads((template / DRAFT).read_bytes()))
    expected = (notice["package"], notice["target"], notice["features"], artifact, digest(binary),
                props["zumen:commit"], props["zumen:lock-sha256"])
    actual = tuple(record[k] for k in ("package", "target", "features", "artifact", "sha256", "commit", "lock"))
    check(actual == expected, "ビルド時の crate・target・feature・バイナリと違う")
    return record


def build(kind, target, module, filename, shipped, output):
    revision = source_revision()
    lock_hash = digest((ROOT / "Cargo.lock").read_bytes())
    manifest = ROOT / MANIFESTS[kind]
    command = ["cargo", "auditable", "build", "--release", "--locked", "--target", target, "--manifest-path",
               str(manifest), "--message-format=json-render-diagnostics", *feature_args(module)]
    result = subprocess.run(command, cwd=ROOT, stdout=subprocess.PIPE, check=True)
    matches = []
    for line in result.stdout.splitlines():
        message = json.loads(line)
        if message["reason"] == "compiler-artifact" and Path(message["manifest_path"]) == manifest:
            matches.extend((Path(path), message["features"]) for path in message["filenames"] if Path(path).name == filename)
    check(len(matches) == 1, "コンパイラーの出力に対象のバイナリがないか複数ある")
    source, enabled = matches[0]
    declared = tomllib.loads(manifest.read_text(encoding="utf-8")).get("features", {})
    if module == "default" and "default" in declared:
        check("default" in enabled, "既定の feature が有効でない")
    elif module != "default":
        check(set(MODULES).intersection(enabled) == {module}, "分けた wasm の feature が違う")
    output.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(source, output / shipped)
    receipt = {"package": "zumen-" + kind, "target": target, "features": module, "enabled": sorted(enabled),
               "artifact": shipped, "sha256": digest((output / shipped).read_bytes()), "commit": revision, "lock": lock_hash}
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
        for kind, filename in (("abi", target["library"]), ("cli", target["exe"])):
            build(kind, target["target"], "default", filename, filename, args.out)
