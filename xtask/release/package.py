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

from licenses import ROOT, TARGETS, license_expression, read_info

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
            return {name: archive.read(name) for name in archive.namelist()}
    with tarfile.open(path, "r:gz") as archive:
        return {member.name: archive.extractfile(member).read() for member in archive.getmembers() if member.isfile()}


def verify_wheel(path, target, library, notice, expression):
    with zipfile.ZipFile(path) as archive:
        names = archive.namelist()
        require(len(names) == len(set(names)), "wheel に同じ名前のファイルが複数ある")
        info = f"zumen-{VERSION}.dist-info"
        expected = {f"{info}/licenses/{name}": (ROOT / name).read_bytes() for name in OWN_LICENSES}
        expected[f"{info}/licenses/THIRD_PARTY_LICENSES"] = notice
        expected[f"zumen/_native/{target['library']}"] = library
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


def package_native(rid, built, licenses, output):
    target = target_for(rid)
    notice_root = licenses / "licenses" / "native" / rid
    abi_info, abi_notice = read_info(notice_root / "abi", "zumen-abi", target["target"])
    _, cli_notice = read_info(notice_root / "cli", "zumen-cli", target["target"])
    library = (built / target["library"]).read_bytes()
    executable = (built / target["exe"]).read_bytes()
    own = {name: (ROOT / name).read_bytes() for name in OWN_LICENSES}
    native = output / "native" / rid
    native.mkdir(parents=True, exist_ok=True)
    native_files = {**own, target["library"]: library, "THIRD_PARTY_LICENSES": abi_notice}
    for name, content in native_files.items():
        (native / name).write_bytes(content)
    require({path.name: path.read_bytes() for path in native.iterdir()} == native_files,
            "native の配布物に抜けか余分なファイルがある")

    name = f"zumen-{VERSION}-{rid}"
    files = {f"{name}/{filename}": content for filename, content in {
        **own, "README.md": (ROOT / "README.md").read_bytes(),
        target["exe"]: executable, "THIRD_PARTY_LICENSES": cli_notice,
    }.items()}
    archive = output / (name + (".zip" if rid.startswith("win-") else ".tar.gz"))
    write_archive(archive, files, f"{name}/{target['exe']}")
    require(read_archive(archive) == files, "CLI の圧縮ファイルの中身が元のファイルと違う")
    wheels = output / "wheels"
    subprocess.run([sys.executable, str(ROOT / "bindings/python/build_wheel.py"), str(built / target["library"]),
                    target["wheel"], str(wheels), "--licenses", str(notice_root / "abi")], check=True)
    wheel = wheels / f"zumen-{VERSION}-py3-none-{target['wheel']}.whl"
    verify_wheel(wheel, target, library, abi_notice, abi_info["license_expression"])
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
    args = parser.parse_args()
    if args.command == "targets":
        print(json.dumps(TARGETS, separators=(",", ":")))
    elif args.command == "native":
        package_native(args.rid, args.built.resolve(), args.licenses.resolve(), args.out.resolve())
    else:
        package_nuget(args.artifacts.resolve(), args.out.resolve())


if __name__ == "__main__":
    main()
