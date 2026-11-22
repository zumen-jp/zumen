"""Apple の 3 対象の静的 ABI を検査し、XCFramework と鏡用の SwiftPM ソースを包む。"""

import argparse
import hashlib
import json
import os
import plistlib
import shutil
import subprocess
import tempfile
from functools import cache
from pathlib import Path

import sbom
from build import build, read_receipt
from licenses import ROOT, SWIFT_LIBRARY, SWIFT_SLICES, read_info
from package import OWN_LICENSES, VERSION, read_archive, require, write_archive

SWIFT = ROOT / "bindings/swift"
FRAMEWORK = "CZumen.xcframework"
AUDIT_OBJECT = "zumen_abi1_audit_data.o"
ZIP = f"zumen-swift-{VERSION}-xcframework.zip"
LOCAL_TARGET = '.binaryTarget(name: "CZumen", path: "build/CZumen.xcframework")'
CONFORMANCE_TARGET = '        .executableTarget(name: "ZumenConformance", dependencies: ["Zumen"]),\n'


@cache
def llvm_ar():
    sysroot = Path(subprocess.check_output(["rustc", "--print", "sysroot"], text=True).strip())
    rustc = subprocess.check_output(["rustc", "-vV"], text=True)
    host = next(line.removeprefix("host: ") for line in rustc.splitlines() if line.startswith("host: "))
    return sysroot / "lib/rustlib" / host / "bin/llvm-ar"


def ar(*args):
    return subprocess.check_output([llvm_ar(), *args])


def verify_static(archive, metadata, shipped):
    # バイナリのハッシュは .a 全体から、埋込グラフはその書庫にある Mach-O のオブジェクトから読む。
    info, _ = read_info(metadata, "zumen-abi", SWIFT_SLICES[metadata.name])
    expected = sbom.bind(metadata, info, SWIFT_LIBRARY, archive.read_bytes())
    require(shipped == expected, "Swift の SBOM と静的ライブラリのハッシュか対象が違う")
    require(ar("t", archive).splitlines().count(AUDIT_OBJECT.encode()) == 1, "Swift の書庫の依存情報が一意でない")
    document = json.loads(shipped)
    sbom.reject_paths(document)
    require(sbom.extracted_graph(ar("p", archive, AUDIT_OBJECT)) == sbom.document_graph(document),
            "Swift の書庫の依存・区分・辺が SBOM と違う")


def build_slices(stage, metadata):
    built = stage / "built"
    libraries = stage / "libraries"
    documents = {}
    for identifier, triple in SWIFT_SLICES.items():
        directory = metadata / identifier
        info, notice = read_info(directory, "zumen-abi", triple)
        receipt_dir = built / triple
        original = build("abi", triple, "default", SWIFT_LIBRARY, SWIFT_LIBRARY, receipt_dir)
        library = receipt_dir / SWIFT_LIBRARY
        read_receipt(receipt_dir, directory, info, SWIFT_LIBRARY, library.read_bytes())

        # cargo-auditable 0.7.7 は、同じ rustc が作る cdylib のリンカーにこのオブジェクトを渡す。
        # staticlib ではこの引数を使わないので、同じ対象のオブジェクトを配布用の書庫にも収める。
        audit_object = original.parent / "deps" / AUDIT_OBJECT
        destination = libraries / triple / SWIFT_LIBRARY
        destination.parent.mkdir(parents=True)
        shutil.copyfile(library, destination)
        ar("rcsD", destination, audit_object)
        documents[f"{identifier}/THIRD_PARTY_LICENSES"] = notice
        documents[f"{identifier}/{SWIFT_LIBRARY}.cdx.json"] = sbom.bind(directory, info, SWIFT_LIBRARY, destination.read_bytes())
    return libraries, documents


def validate_slices(plist):
    rows = plist["AvailableLibraries"]
    require(len(rows) == len(SWIFT_SLICES)
            and {row["LibraryIdentifier"] for row in rows} == set(SWIFT_SLICES), "XCFramework の対象が違う")
    for row in rows:
        name = row["LibraryIdentifier"]
        require(row["LibraryPath"] == SWIFT_LIBRARY and row["HeadersPath"] == "Headers"
                and row["SupportedArchitectures"] == ["arm64"]
                and row["SupportedPlatform"] == name.split("-")[0]
                and row.get("SupportedPlatformVariant") == ("simulator" if name.endswith("-simulator") else None),
                "XCFramework のライブラリ・ヘッダー・プラットフォームが違う")


def pack_framework(libraries, metadata, documents, output):
    subprocess.run(["sh", SWIFT / "build-xcframework.sh", "release", *SWIFT_SLICES.values()], cwd=ROOT, check=True,
                   env={**os.environ, "ZUMEN_SWIFT_LIBRARIES": str(libraries)})
    framework = SWIFT / "build" / FRAMEWORK
    plist = (framework / "Info.plist").read_bytes()
    validate_slices(plistlib.loads(plist))
    files = {"Info.plist": plist}
    for identifier, triple in SWIFT_SLICES.items():
        key = f"{identifier}/{SWIFT_LIBRARY}"
        files[key] = (libraries / triple / SWIFT_LIBRARY).read_bytes()
        archive = framework / key
        require(subprocess.check_output(["xcrun", "lipo", "-archs", archive]).split() == [b"arm64"],
                "Swift の静的ライブラリのアーキテクチャーが違う")
        verify_static(archive, metadata / identifier, documents[key + ".cdx.json"])
        for header in ("zumen.h", "zumen_scene.h"):
            files[f"{identifier}/Headers/{header}"] = (ROOT / "bindings/c/include" / header).read_bytes()
        files[f"{identifier}/Headers/module.modulemap"] = (SWIFT / "build/headers/module.modulemap").read_bytes()
    assembled = {path.relative_to(framework).as_posix(): path.read_bytes() for path in framework.rglob("*") if path.is_file()}
    require(assembled == files, "XCFramework の中身が対象ごとのビルドと違う")
    files.update(documents)
    files.update({name: (ROOT / name).read_bytes() for name in OWN_LICENSES})
    contents = {f"{FRAMEWORK}/{name}": data for name, data in files.items()}
    write_archive(output / ZIP, contents, None)
    require(read_archive(output / ZIP) == contents, "XCFramework の ZIP の中身が違う")


def mirror_manifest(checksum):
    manifest = (SWIFT / "Package.swift").read_text(encoding="utf-8")
    require(manifest.count(LOCAL_TARGET) == manifest.count(CONFORMANCE_TARGET) == 1, "SwiftPM のローカルの宣言が変わった")
    url = f"https://github.com/zumen-jp/zumen/releases/download/v{VERSION}/{ZIP}"
    remote = f'.binaryTarget(name: "CZumen", url: "{url}", checksum: "{checksum}")'
    return manifest.replace(CONFORMANCE_TARGET, "").replace(LOCAL_TARGET, remote), remote


def pack_mirror(output, stage):
    archive = output / ZIP
    checksum = subprocess.check_output(["swift", "package", "compute-checksum", archive], text=True).strip()
    require(hashlib.sha256(archive.read_bytes()).hexdigest() == checksum, "SwiftPM のチェックサムが ZIP と違う")
    manifest, remote = mirror_manifest(checksum)
    files = {name: (ROOT / name).read_bytes() for name in OWN_LICENSES}
    files.update({"README.md": (SWIFT / "README.md").read_bytes(), "Package.swift": manifest.encode()})
    for source in sorted((SWIFT / "Sources/Zumen").rglob("*.swift")):
        files[source.relative_to(SWIFT).as_posix()] = source.read_bytes()
    require(f'public static let version = "{VERSION}"'.encode() in files["Sources/Zumen/Constants.swift"],
            "Swift のラッパーと配布物の版が違う")
    name = f"zumen-swift-{VERSION}-source"
    contents = {f"{name}/{key}": data for key, data in files.items()}
    source_archive = output / (name + ".tar.gz")
    write_archive(source_archive, contents, None)
    packed = read_archive(source_archive)
    require(packed == contents, "SwiftPM の鏡用ソースの中身が違う")
    for key, data in packed.items():
        path = stage / key
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)
    mirror = stage / name
    subprocess.run(["swift", "package", "--package-path", mirror, "dump-package"], check=True, stdout=subprocess.DEVNULL)
    # タグ前は URL が存在しない。検査用のコピーで同じ ZIP の展開先を参照して SwiftPM でコンパイルする。
    (mirror / "Package.swift").write_text(manifest.replace(remote, LOCAL_TARGET), encoding="utf-8")
    for key, content in read_archive(archive).items():
        path = mirror / "build" / key
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(content)
    subprocess.run(["swift", "build", "--package-path", mirror, "-c", "release"], check=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--licenses", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    metadata = args.licenses.resolve() / "licenses/swift"
    output = args.out.resolve()
    output.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="zumen-swift-") as temporary:
        stage = Path(temporary)
        libraries, documents = build_slices(stage, metadata)
        pack_framework(libraries, metadata, documents, output)
        pack_mirror(output, stage)


if __name__ == "__main__":
    main()
