"""ネイティブのライブラリを入れた wheel を、標準ライブラリだけで作る（決定 0205）。

    python build_wheel.py <ライブラリの道筋> <プラットフォームのタグ> [出力のディレクトリ] --licenses <ライセンスのディレクトリ> --sbom <対応する .cdx.json>

ライブラリは zumen/_native に入れ、load() が道筋なしで読む。拡張のモジュールはないので、タグは py3-none-<プラットフォーム>。
SBOM とライセンスの生成から wheel の梱包・検査まで進める手順は、xtask/release/README.md の
「ネイティブのライブラリ・CLI・wheel を作る」を参照する。作成には Python 3.11 以降を使う。
"""

import argparse
import base64
import hashlib
import json
import os
import sys
import tomllib
import zipfile
from pathlib import Path

HERE = os.path.dirname(os.path.abspath(__file__))


def _record(name: str, data: bytes) -> str:
    digest = base64.urlsafe_b64encode(hashlib.sha256(data).digest()).rstrip(b"=").decode()
    return f"{name},sha256={digest},{len(data)}"


def main(argv: list) -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("library")
    parser.add_argument("platform")
    parser.add_argument("out", nargs="?", default=os.path.join(HERE, "dist"))
    parser.add_argument("--licenses", required=True)
    parser.add_argument("--sbom", type=Path, help="package.py native が作る対象 ABI の SBOM。既定はライブラリの隣の .cdx.json")
    args = parser.parse_args(argv[1:])
    library, platform, out = args.library, args.platform, args.out
    with open(os.path.join(args.licenses, "THIRD_PARTY_LICENSES"), "rb") as f:
        third_party = f.read()
    with open(os.path.join(args.licenses, "license-info.json"), encoding="utf-8") as f:
        notice_info = json.load(f)
    with open(os.path.join(HERE, "../../xtask/release/targets.json"), encoding="utf-8") as f:
        target = next(item for item in json.load(f) if item["wheel"] == platform)
    if (notice_info["package"] != "zumen-abi" or notice_info["target"] != target["target"]
            or os.path.basename(library) != target["library"]
            or hashlib.sha256(third_party).hexdigest() != notice_info["sha256"]):
        raise ValueError("zumen-abi のライセンス本文と台帳が一致しない")
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
        f"License-Expression: {notice_info['license_expression']}",
        # Core metadata 2.4 / PEP 639: .dist-info/licenses からの相対パス
        "License-File: LICENSE-MIT",
        "License-File: LICENSE-APACHE",
        "License-File: THIRD_PARTY_LICENSES",
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
    binary = files[-1][1]
    sbom_name = os.path.basename(library) + ".cdx.json"
    document = (args.sbom or Path(library).with_name(sbom_name)).read_bytes()
    meta = json.loads(document)["metadata"]
    attributes = {p["name"]: p["value"] for p in meta["properties"]}
    if (attributes["zumen:crate"] != "zumen-abi" or attributes["cdx:rustc:sbom:target:triple"] != target["target"]
            or attributes["zumen:artifact"] != os.path.basename(library) or attributes["zumen:features"] != "default"
            or meta["component"]["hashes"] != [{"alg": "SHA-256", "content": hashlib.sha256(binary).hexdigest()}]):
        raise ValueError("wheel のライブラリと SBOM が違う")
    info = f"{name}-{version}.dist-info"
    files.append((f"{info}/sboms/{sbom_name}", document))
    for filename in ("LICENSE-MIT", "LICENSE-APACHE"):
        with open(os.path.join(HERE, "../..", filename), "rb") as f:
            files.append((f"{info}/licenses/{filename}", f.read()))
    files.append((f"{info}/licenses/THIRD_PARTY_LICENSES", third_party))
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
