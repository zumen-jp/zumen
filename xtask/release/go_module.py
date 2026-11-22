"""Go の鏡に置くソースと、同じコミットから作った wasm を包む。"""

import argparse
import os
import shutil
import subprocess
import tempfile
from pathlib import Path

import sbom
from licenses import ROOT, WASM, read_info
from package import OWN_LICENSES, VERSION, WASM_BOMS, read_archive, require, wasm_bom, write_archive


def package_go(artifacts, output):
    source = ROOT / "bindings/go"
    name = "zumen_abi1.wasm"
    binary = (artifacts / name).read_bytes()
    _, expected_bom = wasm_bom(artifacts, "default", binary)
    bom = read_archive(artifacts / WASM_BOMS)["sboms/" + name + ".cdx.json"]
    _, notice = read_info(artifacts / "licenses/wasm/abi", "zumen-abi", WASM)
    paths = subprocess.check_output(["git", "ls-files", "-z", "--", "*.go", ":(exclude)*_test.go"], cwd=source)
    files = {path: (source / path).read_bytes() for path in paths.decode("utf-8").split("\0") if path}
    files.update({n: (source / n).read_bytes() for n in ("go.mod", "go.sum")})
    files.update({n: (ROOT / n).read_bytes() for n in OWN_LICENSES})
    files.update({name: binary, name + ".cdx.json": bom, "THIRD_PARTY_LICENSES": notice})
    files["README.md"] = (
        "# zumen.dev/go\n\n"
        f"`go get zumen.dev/go@v{VERSION}` で導入します。\n\n"
        f"[使い方](https://github.com/zumen-jp/zumen/blob/v{VERSION}/bindings/go/README.md)\n"
    ).encode()
    require(files["go.mod"].splitlines()[0] == b"module zumen.dev/go", "Go のモジュール名が違う")
    require(b"//go:embed " + name.encode() in files["zumen.go"], "Go が同梱の wasm を参照していない")
    output.mkdir(parents=True, exist_ok=True)
    archive = output / f"zumen-go-{VERSION}.tar.gz"
    with tempfile.TemporaryDirectory(dir=output) as directory:
        candidate = Path(directory) / archive.name
        write_archive(candidate, files, None)
        packed = read_archive(candidate)
        require(packed == files, "Go の配布物が元のファイルと違う")
        sbom.verify(packed[name], packed[name + ".cdx.json"], expected_bom)
        stage = Path(directory) / "module"
        stage.mkdir()
        for filename, content in packed.items():
            (stage / filename).write_bytes(content)
        for test in source.glob("*_test.go"):
            shutil.copyfile(test, stage / test.name)
        subprocess.run(["go", "test", "-mod=readonly", "-count=1", "./..."], cwd=stage, check=True,
                       env={**os.environ, "ZUMEN_REPOSITORY": str(ROOT), "GOWORK": "off"})
        candidate.replace(archive)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--artifacts", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    package_go(args.artifacts, args.out)
