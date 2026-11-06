"""crate・target ごとの依存のライセンスを作る（決定 0269）。Python 3.11 以降、標準ライブラリだけを使う。"""

import argparse
import hashlib
import json
import re
import shutil
import subprocess
import tempfile
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
REPOSITORY = "https://github.com/zumen-jp/zumen"
ABOUT_VERSION = "cargo-about 0.9.2"
PLACEHOLDER = re.compile(r"<year>|<copyright holders>|<owner>", re.IGNORECASE)
MANIFESTS = {"abi": "crates/zumen-abi/Cargo.toml", "cli": "bindings/cli/Cargo.toml"}
TARGETS = json.loads((Path(__file__).parent / "targets.json").read_text(encoding="utf-8"))
WASM = "wasm32-unknown-unknown"
MODULES = ("default", "drawing", "bim", "civil", "pointcloud")


def feature_args(module):
    return [] if module == "default" else ["--no-default-features", "--features", module]


def wasm_name(module):
    return "zumen_abi1" + ("" if module == "default" else "_" + module) + ".wasm"


def license_expression(ids):
    """このリポジトリの選択式を保ち、同梱する第三者のライセンスを AND で併記する。"""
    project = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))["workspace"]["package"]["license"]
    return " AND ".join([f"({project})", *sorted(set(ids))])


def third_party(data):
    """本文の置換と抜けを拒み、端末のパスを持たない台帳にする。"""
    expected = {
        (entry["package"]["name"], entry["package"]["version"])
        for entry in data["crates"]
        if entry["package"].get("repository") != REPOSITORY
    }
    found = {}
    for license_ in data["licenses"]:
        users = [entry["crate"] for entry in license_["used_by"] if entry["crate"].get("repository") != REPOSITORY]
        if users and PLACEHOLDER.search(license_["text"]):
            raise ValueError("第三者のライセンスが SPDX の定型の文に代わっている。about.toml の clarify を確認する")
        for crate in users:
            found.setdefault((crate["name"], crate["version"]), set()).add(license_["id"])
    if not expected or set(found) != expected:
        raise ValueError("第三者の crate の一覧と、ライセンス本文の一覧が一致しない")
    return [{"name": name, "version": version, "licenses": sorted(ids)} for (name, version), ids in sorted(found.items())]


def run_about(command):
    result = subprocess.run(command, cwd=ROOT, capture_output=True, text=True, encoding="utf-8", check=False)
    if result.returncode:
        raise ValueError("cargo-about が失敗した\n" + result.stderr)
    if re.search(r"WARN|ERROR", result.stderr, re.IGNORECASE):
        raise ValueError("cargo-about が警告を出した。about.toml の clarify とライセンスを確認する\n" + result.stderr)
    return result


def generate(tool, kind, target, output, module="default"):
    output.mkdir(parents=True, exist_ok=True)
    package = "zumen-abi" if kind == "abi" else "zumen-cli"
    args = [tool, "about", "generate", "--config", "about.toml", "--manifest-path", MANIFESTS[kind],
            "--target", target, "--locked", "--fail", *feature_args(module)]
    with tempfile.TemporaryDirectory(prefix="zumen-licenses-") as temporary:
        raw = Path(temporary) / "raw.json"
        rendered = Path(temporary) / "licenses.txt"
        run_about([*args, "--format", "json", "--output-file", str(raw)])
        dependencies = third_party(json.loads(raw.read_text(encoding="utf-8")))
        run_about([*args, "--output-file", str(rendered), "about.hbs"])
        text = rendered.read_text(encoding="utf-8")
        if PLACEHOLDER.search(text) or any(f"- {dep['name']} {dep['version']}" not in text for dep in dependencies):
            raise ValueError("ライセンス本文に定型の文か crate の抜けがある")
        text = f"対象 crate: {package}\n対象 target: {target}\n\n{text}"
        content = text.encode("utf-8")
        info = {
            "package": package, "target": target, "features": module,
            "license_expression": license_expression(license_ for dep in dependencies for license_ in dep["licenses"]),
            "sha256": hashlib.sha256(content).hexdigest(), "dependencies": dependencies,
        }
        (output / "THIRD_PARTY_LICENSES").write_bytes(content)
        (output / "license-info.json").write_text(json.dumps(info, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(f"{package} / {target}: 第三者の crate {len(dependencies)} 件")


def read_info(directory, package=None, target=None, module="default"):
    directory = Path(directory)
    content = (directory / "THIRD_PARTY_LICENSES").read_bytes()
    info = json.loads((directory / "license-info.json").read_text(encoding="utf-8"))
    if hashlib.sha256(content).hexdigest() != info["sha256"]:
        raise ValueError("ライセンス本文のハッシュが台帳と一致しない")
    if package is not None and info["package"] != package:
        raise ValueError("ライセンスの crate が違う")
    if target is not None and info["target"] != target:
        raise ValueError("ライセンスの target が違う")
    if info.get("features") != module:
        raise ValueError("ライセンスの feature が違う")
    if info["license_expression"] != license_expression(license_ for dep in info["dependencies"] for license_ in dep["licenses"]):
        raise ValueError("ライセンスの式が依存の一覧と一致しない")
    return info, content


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--cargo-about", default=shutil.which("cargo-about"))
    args = parser.parse_args()
    if not args.cargo_about or run_about([args.cargo_about, "--version"]).stdout.strip() != ABOUT_VERSION:
        parser.error("cargo-about 0.9.2 を入れ、PATH か --cargo-about で指定する")
    output = args.output.resolve()
    wasm = output / "licenses" / "wasm" / "abi"
    for module in MODULES:
        directory = wasm if module == "default" else wasm.parent / module
        generate(args.cargo_about, "abi", WASM, directory, module)
    shutil.copyfile(wasm / "THIRD_PARTY_LICENSES", output / "THIRD_PARTY_LICENSES")
    for target in TARGETS:
        for kind in MANIFESTS:
            generate(args.cargo_about, kind, target["target"], output / "licenses" / "native" / target["rid"] / kind)


if __name__ == "__main__":
    main()
