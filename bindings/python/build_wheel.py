"""ネイティブのライブラリを入れた wheel を、標準ライブラリだけで作る（決定 0205）。

    python build_wheel.py <ライブラリの道筋> <プラットフォームのタグ> [出力のディレクトリ]

例: python build_wheel.py ../../target/release/libzumen_abi1.so manylinux_2_28_x86_64
ライブラリは zumen/_native に入れ、load() が道筋なしで読む。拡張のモジュールはないので、タグは py3-none-<プラットフォーム>。
"""

import base64
import hashlib
import os
import sys
import tomllib
import zipfile

HERE = os.path.dirname(os.path.abspath(__file__))


def _record(name: str, data: bytes) -> str:
    digest = base64.urlsafe_b64encode(hashlib.sha256(data).digest()).rstrip(b"=").decode()
    return f"{name},sha256={digest},{len(data)}"


def main(argv: list) -> None:
    if len(argv) not in (3, 4):
        sys.exit(__doc__)
    library, platform = argv[1], argv[2]
    out = argv[3] if len(argv) == 4 else os.path.join(HERE, "dist")
    with open(os.path.join(HERE, "pyproject.toml"), "rb") as f:
        project = tomllib.load(f)["project"]
    name, version = project["name"], project["version"]
    with open(os.path.join(HERE, "README.md"), encoding="utf-8") as f:
        readme = f.read()
    metadata = [
        "Metadata-Version: 2.4",
        f"Name: {name}",
        f"Version: {version}",
        f"Summary: {project['description']}",
        f"License-Expression: {project['license']}",
        f"Requires-Python: {project['requires-python']}",
        *(f"Classifier: {c}" for c in project.get("classifiers", [])),
        *(f"Project-URL: {k}, {v}" for k, v in project.get("urls", {}).items()),
        "Description-Content-Type: text/markdown",
        "",
        readme,
    ]
    tag = f"py3-none-{platform}"
    wheel = ["Wheel-Version: 1.0", "Generator: zumen build_wheel.py", "Root-Is-Purelib: false", f"Tag: {tag}", ""]
    files = []
    package = os.path.join(HERE, "zumen")
    for entry in sorted(os.listdir(package)):
        if entry.endswith(".py"):
            with open(os.path.join(package, entry), "rb") as f:
                files.append((f"zumen/{entry}", f.read()))
    with open(library, "rb") as f:
        files.append((f"zumen/_native/{os.path.basename(library)}", f.read()))
    info = f"{name}-{version}.dist-info"
    files.append((f"{info}/METADATA", "\n".join(metadata).encode()))
    files.append((f"{info}/WHEEL", "\n".join(wheel).encode()))
    record = [_record(n, d) for n, d in files] + [f"{info}/RECORD,,"]
    files.append((f"{info}/RECORD", ("\n".join(record) + "\n").encode()))
    os.makedirs(out, exist_ok=True)
    path = os.path.join(out, f"{name}-{version}-{tag}.whl")
    with zipfile.ZipFile(path, "w", zipfile.ZIP_DEFLATED) as z:
        for n, d in files:
            info_ = zipfile.ZipInfo(n, date_time=(1980, 1, 1, 0, 0, 0))
            info_.external_attr = 0o644 << 16
            info_.compress_type = zipfile.ZIP_DEFLATED
            z.writestr(info_, d)
    print(path)


if __name__ == "__main__":
    main(sys.argv)
