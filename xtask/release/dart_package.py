"""初回の手動公開と、後続のタグ公開で使う同じ Dart の配布物を作る。"""

import argparse
import subprocess
import tarfile
import tempfile
from pathlib import Path

from package import OWN_LICENSES, ROOT, VERSION, require, write_archive


def dart_files():
    source = ROOT / "bindings/dart"
    paths = subprocess.check_output(
        ["git", "ls-files", "-z", "--", "lib/*.dart"], cwd=source
    ).decode("utf-8").split("\0")
    files = {name: (source / name).read_bytes() for name in paths if name}
    if "lib/zumen.dart" not in files:
        raise ValueError("Dart の公開 API が見つからない")
    manifest = (source / "pubspec.yaml").read_text(encoding="utf-8").splitlines()
    for field in ("name: zumen", f"version: {VERSION}", "publish_to: none"):
        if manifest.count(field) != 1:
            raise ValueError(f"Dart の名前・版・公開先を確認できない: {field}")
    manifest[manifest.index("publish_to: none")] = "publish_to: https://pub.dev"
    files["pubspec.yaml"] = ("\n".join(manifest) + "\n").encode("utf-8")
    files["README.md"] = (source / "README.md").read_bytes()
    files["CHANGELOG.md"] = (
        f"## {VERSION}\n\n[この版の変更](https://github.com/zumen-jp/zumen/blob/v{VERSION}/docs/releases/{VERSION}.md)\n"
    ).encode("utf-8")
    files["LICENSE"] = b"MIT OR Apache-2.0\n\n" + b"\n\n".join(
        (ROOT / name).read_bytes() for name in OWN_LICENSES
    )
    return files


def check_contents(archive, files, *, published=False):
    remaining = files.copy()
    for member in archive:
        name = member.name.removeprefix("./") if published else member.name
        if published and member.isdir():
            continue
        require(member.isfile() and name in remaining, f"Dart の配布物に想定外のファイルがある: {name}")
        content = remaining.pop(name)
        require(member.size == len(content) and archive.extractfile(member).read() == content,
                f"Dart の配布物の {name} がタグと違う")
    require(not remaining, "Dart の配布物にファイルの抜けがある")


def verify_dart(bundle, files):
    with tarfile.open(bundle, "r|gz") as archive:
        check_contents(archive, files)


def package_dart(out):
    files = dart_files()

    out.mkdir(parents=True, exist_ok=True)
    name = f"zumen-dart-{VERSION}.tar.gz"
    with tempfile.TemporaryDirectory(dir=out) as temporary:
        archive = Path(temporary) / name
        write_archive(archive, files, executable=None)
        verify_dart(archive, files)
        archive.replace(out / name)
    return out / name


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument("--out", type=Path)
    group.add_argument("--check", type=Path)
    args = parser.parse_args()
    if args.check:
        verify_dart(args.check, dart_files())
    else:
        print(package_dart(args.out))
