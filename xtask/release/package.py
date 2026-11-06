"""ライセンスを配布物に入れ、包んだ中身を確かめる。公開はしない（決定 0269）。"""

import argparse
import base64
import csv
import gzip
import hashlib
import io
import json
import shutil
import subprocess
import sys
import tarfile
import tempfile
import tomllib
import xml.etree.ElementTree as ET
import zipfile
from email.parser import BytesParser
from pathlib import Path

import sbom
from build import read_receipt
from licenses import MODULES, ROOT, TARGETS, WASM, license_expression, read_info, wasm_name

OWN_LICENSES = ("LICENSE-MIT", "LICENSE-APACHE")
VERSION = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))["workspace"]["package"]["version"]


def require(condition, message):
    if not condition:
        raise ValueError(message)


def target_for(rid):
    return next(target for target in TARGETS if target["rid"] == rid)


def write_archive(path, files, executable):
    """端末の時刻・利用者の名前を圧縮ファイルに入れない。"""
    if path.suffix == ".zip":
        with zipfile.ZipFile(path, "w", zipfile.ZIP_DEFLATED) as archive:
            for name, content in sorted(files.items()):
                info = zipfile.ZipInfo(name, date_time=(1980, 1, 1, 0, 0, 0))
                info.external_attr = (0o755 if name == executable else 0o644) << 16
                info.compress_type = zipfile.ZIP_DEFLATED
                archive.writestr(info, content)
    else:
        with path.open("wb") as raw, gzip.GzipFile(filename="", mode="wb", fileobj=raw, mtime=0) as compressed:
            with tarfile.open(fileobj=compressed, mode="w") as archive:
                for name, content in sorted(files.items()):
                    info = tarfile.TarInfo(name)
                    info.size = len(content)
                    info.mode = 0o755 if name == executable else 0o644
                    archive.addfile(info, io.BytesIO(content))


def read_archive(path):
    if path.suffix == ".zip":
        with zipfile.ZipFile(path) as archive:
            require(len(archive.namelist()) == len(set(archive.namelist())), "zip 内の名前が重複している")
            return {name: archive.read(name) for name in archive.namelist()}
    with tarfile.open(path, "r:gz") as archive:
        files = [m for m in archive if not m.isdir()]
        require(all(m.isfile() for m in files) and len(files) == len({m.name for m in files}),
                "tar 内に通常のファイル以外のものか重複がある")
        return {member.name: archive.extractfile(member).read() for member in files}


def verify_wheel(path, target, library, notice, expression, abi_bom):
    with zipfile.ZipFile(path) as archive:
        names = archive.namelist()
        require(len(names) == len(set(names)), "wheel に同じ名前のファイルが複数ある")
        info = f"zumen-{VERSION}.dist-info"
        expected = {f"{info}/licenses/{name}": (ROOT / name).read_bytes() for name in OWN_LICENSES}
        expected[f"{info}/licenses/THIRD_PARTY_LICENSES"] = notice
        expected[f"zumen/_native/{target['library']}"] = library
        bom_name = f"{info}/sboms/{target['library']}.cdx.json"
        expected[bom_name] = abi_bom
        require({n for n in names if n.startswith(f"{info}/sboms/")} == {bom_name}, "wheel の SBOM の組が違う")
        for name, content in expected.items():
            require(archive.read(name) == content, f"wheel の {name} が元のファイルと違う")
        metadata = BytesParser().parsebytes(archive.read(f"{info}/METADATA"))
        require(metadata["Metadata-Version"] == "2.4", "wheel のメタデータの版が違う")
        require(metadata["Version"] == VERSION, "wheel のバージョンが違う")
        require(metadata["License-Expression"] == expression, "wheel のライセンスの式が違う")
        require(set(metadata.get_all("License-File", [])) == {*OWN_LICENSES, "THIRD_PARTY_LICENSES"},
                "wheel の License-File に抜けがある")
        wheel = BytesParser().parsebytes(archive.read(f"{info}/WHEEL"))
        require(wheel["Tag"] == f"py3-none-{target['wheel']}", "wheel のプラットフォームのタグが違う")
        record = list(csv.reader(io.StringIO(archive.read(f"{info}/RECORD").decode("utf-8"))))
        require(len(record) == len(names) and {row[0] for row in record} == set(names), "wheel の RECORD に抜けがある")
        for name, digest, size in record:
            if name == f"{info}/RECORD":
                require(not digest and not size, "RECORD 自身のハッシュは空にする")
            else:
                content = archive.read(name)
                actual = base64.urlsafe_b64encode(hashlib.sha256(content).digest()).rstrip(b"=").decode("ascii")
                require(digest == f"sha256={actual}" and size == str(len(content)), f"RECORD の {name} が違う")
        sbom.verify(archive.read(f"zumen/_native/{target['library']}"), archive.read(bom_name), abi_bom)


def package_native(rid, built, licenses, output):
    target = target_for(rid)
    notice_root = licenses / "licenses" / "native" / rid
    abi_info, abi_notice = read_info(notice_root / "abi", "zumen-abi", target["target"])
    cli_info, cli_notice = read_info(notice_root / "cli", "zumen-cli", target["target"])
    library = (built / target["library"]).read_bytes()
    executable = (built / target["exe"]).read_bytes()
    read_receipt(built, notice_root / "abi", abi_info, target["library"], library)
    read_receipt(built, notice_root / "cli", cli_info, target["exe"], executable)
    abi_name, cli_name = target["library"] + ".cdx.json", target["exe"] + ".cdx.json"
    abi_bom = sbom.bind(notice_root / "abi", abi_info, target["library"], library)
    cli_bom = sbom.bind(notice_root / "cli", cli_info, target["exe"], executable)
    own = {name: (ROOT / name).read_bytes() for name in OWN_LICENSES}
    native = output / "native" / rid
    native.mkdir(parents=True, exist_ok=True)
    native_files = {**own, target["library"]: library, "THIRD_PARTY_LICENSES": abi_notice, abi_name: abi_bom}
    for name, content in native_files.items():
        (native / name).write_bytes(content)
    require({path.name: path.read_bytes() for path in native.iterdir()} == native_files,
            "native の配布物に抜けか余分なファイルがある")
    sbom.verify((native / target["library"]).read_bytes(), (native / abi_name).read_bytes(), abi_bom)

    name = f"zumen-{VERSION}-{rid}"
    files = {f"{name}/{filename}": content for filename, content in {
        **own, "README.md": (ROOT / "README.md").read_bytes(),
        target["exe"]: executable, "THIRD_PARTY_LICENSES": cli_notice, cli_name: cli_bom,
    }.items()}
    archive = output / (name + (".zip" if rid.startswith("win-") else ".tar.gz"))
    write_archive(archive, files, f"{name}/{target['exe']}")
    packed = read_archive(archive)
    require(packed == files, "CLI の圧縮ファイルの中身が元のファイルと違う")
    sbom.verify(packed[f"{name}/{target['exe']}"], packed[f"{name}/{cli_name}"], cli_bom)
    wheels = output / "wheels"
    subprocess.run([sys.executable, str(ROOT / "bindings/python/build_wheel.py"), str(built / target["library"]),
                    target["wheel"], str(wheels), "--licenses", str(notice_root / "abi"),
                    "--sbom", str(native / abi_name)], check=True)
    wheel = wheels / f"zumen-{VERSION}-py3-none-{target['wheel']}.whl"
    verify_wheel(wheel, target, library, abi_notice, abi_info["license_expression"], abi_bom)
    print(f"{rid}: native・CLI の圧縮・wheel の本文、バイナリ、メタデータを確認した")


def verify_nuget(path, expected, expression):
    with zipfile.ZipFile(path) as archive:
        names = archive.namelist()
        require(len(names) == len(set(names)), "NuGet に同じ名前のファイルが複数ある")
        for name, content in expected.items():
            require(archive.read(name) == content, f"NuGet の {name} が元のファイルと違う")
        require({name for name in names if name.startswith("runtimes/")} ==
                {name for name in expected if name.startswith("runtimes/")}, "NuGet の対象の一覧が違う")
        metadata = ET.fromstring(archive.read("Zumen.nuspec"))
        license_ = metadata.find("{*}metadata/{*}license")
        require(license_ is not None and license_.get("type") == "expression" and license_.text == expression,
                "NuGet のライセンスの式が違う")
        require(metadata.findtext("{*}metadata/{*}version") == VERSION, "NuGet のバージョンが違う")
        for framework in ("netstandard2.0", "net8.0"):
            require(f"lib/{framework}/Zumen.dll" in names, "NuGet のアセンブリに抜けがある")
        for target in TARGETS:
            binary = f"runtimes/{target['rid']}/native/{target['library']}"
            sbom.verify(archive.read(binary), archive.read(binary + ".cdx.json"), expected[binary + ".cdx.json"])


def package_nuget(artifacts, output):
    expected = {name: (ROOT / name).read_bytes() for name in OWN_LICENSES}
    ids = set()
    for target in TARGETS:
        rid = target["rid"]
        info, notice = read_info(artifacts / "licenses" / "native" / rid / "abi", "zumen-abi", target["target"])
        ids.update(license_ for dep in info["dependencies"] for license_ in dep["licenses"])
        native = artifacts / "native" / rid
        require((native / "THIRD_PARTY_LICENSES").read_bytes() == notice, f"{rid} の native のライセンスが違う")
        expected[f"runtimes/{rid}/native/THIRD_PARTY_LICENSES"] = notice
        expected[f"runtimes/{rid}/native/{target['library']}"] = (native / target["library"]).read_bytes()
        binary = expected[f"runtimes/{rid}/native/{target['library']}"]
        bom = sbom.bind(artifacts / "licenses" / "native" / rid / "abi", info, target["library"], binary)
        name = target["library"] + ".cdx.json"
        require((native / name).read_bytes() == bom, f"{rid} の native の SBOM が違う")
        expected[f"runtimes/{rid}/native/{name}"] = bom
    expression = license_expression(ids)
    output.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="zumen-nuget-") as temporary:
        stage = Path(temporary)
        project = stage / "bindings" / "dotnet" / "Zumen"
        shutil.copytree(ROOT / "bindings/dotnet/Zumen", project, ignore=shutil.ignore_patterns("bin", "obj", "runtimes"))
        shutil.copyfile(ROOT / "bindings/dotnet/README.md", project.parent / "README.md")
        for name, content in expected.items():
            destination = project / name if name.startswith("runtimes/") else stage / name
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_bytes(content)
        subprocess.run(["dotnet", "pack", str(project / "Zumen.csproj"), "-c", "Release", "-o", str(output),
                        f"-p:PackageLicenseExpression={expression}", f"-p:PathMap={stage}=/_/"], check=True)
    verify_nuget(output / f"Zumen.{VERSION}.nupkg", expected, expression)
    print("NuGet: 7 対象のライセンス本文、バイナリ、メタデータを確認した")


WASM_BOMS = f"zumen-{VERSION}-wasm-sboms.tar.gz"


def wasm_bom(licenses, module, binary):
    directory = licenses / "licenses" / "wasm" / ("abi" if module == "default" else module)
    info, _ = read_info(directory, "zumen-abi", WASM, module)
    return info, sbom.bind(directory, info, wasm_name(module), binary)


def package_wasm(built, licenses, output):
    output.mkdir(parents=True, exist_ok=True)
    default_info, notice = read_info(licenses / "licenses" / "wasm" / "abi", "zumen-abi", WASM)
    require((licenses / "THIRD_PARTY_LICENSES").read_bytes() == notice, "wasm 用のライセンスが違う")
    covered = {(d["name"], d["version"], tuple(d["licenses"])) for d in default_info["dependencies"]}
    documents = {}
    for module in MODULES:
        name = wasm_name(module)
        binary = (built / name).read_bytes()
        info, bom = wasm_bom(licenses, module, binary)
        require(all((d["name"], d["version"], tuple(d["licenses"])) in covered for d in info["dependencies"]),
                "配るライセンス本文に分割 wasm の依存がない")
        directory = licenses / "licenses" / "wasm" / ("abi" if module == "default" else module)
        read_receipt(built, directory, info, name, binary)
        (output / name).write_bytes(binary)
        documents["sboms/" + name + ".cdx.json"] = bom
    write_archive(output / WASM_BOMS, documents, None)
    packed = read_archive(output / WASM_BOMS)
    require(packed == documents, "wasm 用の SBOM の書庫が違う")
    for module in MODULES:
        name = wasm_name(module)
        key = "sboms/" + name + ".cdx.json"
        sbom.verify((output / name).read_bytes(), packed[key], documents[key])


def package_npm(artifacts, output):
    output.mkdir(parents=True, exist_ok=True)
    documents = read_archive(artifacts / WASM_BOMS)
    require(set(documents) == {"sboms/" + wasm_name(m) + ".cdx.json" for m in MODULES}, "wasm の SBOM の組が違う")
    contents = {**documents, "THIRD_PARTY_LICENSES": (artifacts / "THIRD_PARTY_LICENSES").read_bytes()}
    _, notice = read_info(artifacts / "licenses" / "wasm" / "abi", "zumen-abi", WASM)
    require(contents["THIRD_PARTY_LICENSES"] == notice, "npm に入れる wasm のライセンス本文が違う")
    for name in OWN_LICENSES:
        contents[name] = (ROOT / name).read_bytes()
    for module in MODULES:
        name = wasm_name(module)
        contents[name] = (artifacts / name).read_bytes()
        _, expected = wasm_bom(artifacts, module, contents[name])
        require(documents["sboms/" + name + ".cdx.json"] == expected, "wasm の SBOM とビルドが違う")
    with tempfile.TemporaryDirectory(prefix="zumen-npm-") as temporary:
        stage = Path(temporary)
        for name in ("package.json", "README.md"):
            shutil.copyfile(ROOT / "bindings/js" / name, stage / name)
        shutil.copytree(ROOT / "bindings/js/src", stage / "src")
        for name, content in contents.items():
            path = stage / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(content)
        packed_dir = stage / "packed"
        packed_dir.mkdir()
        subprocess.run([shutil.which("npm") or "npm", "pack", "--ignore-scripts", "--silent",
                        "--pack-destination", str(packed_dir)], cwd=stage, check=True)
        archives = list(packed_dir.glob("*.tgz"))
        require(len(archives) == 1, "npm のパッケージが一意でない")
        packed = read_archive(archives[0])
        require({n.removeprefix("package/") for n in packed if n.startswith("package/sboms/")} == set(documents),
                "npm の SBOM の組が違う")
        for name, content in contents.items():
            require(packed["package/" + name] == content, f"npm の {name} が違う")
        for module in MODULES:
            name = wasm_name(module)
            key = "sboms/" + name + ".cdx.json"
            sbom.verify(packed["package/" + name], packed["package/" + key], documents[key])
        shutil.copyfile(archives[0], output / archives[0].name)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    commands.add_parser("targets")
    native = commands.add_parser("native")
    native.add_argument("--rid", choices=[target["rid"] for target in TARGETS], required=True)
    native.add_argument("--built", type=Path, required=True)
    native.add_argument("--licenses", type=Path, required=True)
    native.add_argument("--out", type=Path, required=True)
    nuget = commands.add_parser("nuget")
    nuget.add_argument("--artifacts", type=Path, required=True)
    nuget.add_argument("--out", type=Path, required=True)
    wasm = commands.add_parser("wasm")
    wasm.add_argument("--built", type=Path, required=True)
    wasm.add_argument("--licenses", type=Path, required=True)
    wasm.add_argument("--out", type=Path, required=True)
    npm = commands.add_parser("npm")
    npm.add_argument("--artifacts", type=Path, required=True)
    npm.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    if args.command == "targets":
        print(json.dumps(TARGETS, separators=(",", ":")))
    elif args.command == "native":
        package_native(args.rid, args.built.resolve(), args.licenses.resolve(), args.out.resolve())
    elif args.command == "wasm":
        package_wasm(args.built.resolve(), args.licenses.resolve(), args.out.resolve())
    elif args.command == "npm":
        package_npm(args.artifacts.resolve(), args.out.resolve())
    else:
        package_nuget(args.artifacts.resolve(), args.out.resolve())


if __name__ == "__main__":
    main()
