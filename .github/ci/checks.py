"""変更したパスから通常の CI の検査を選ぶ。未分類の入力はすべて検査する。"""

import json
import os
from pathlib import Path
import subprocess
import sys


ALL = {"rust", "native", "wasm", "codegen", "site", "deny", "workflows", "text"}
CORE = {"rust", "native", "wasm", "codegen"}
TEXT_CONFIG = {
    ".textlintignore", ".textlint-allow.yml", ".textlintrc.yml", ".textlintrc.code.yml",
    "prh.yml", "package.json", "package-lock.json",
}
CODE_EXTENSIONS = (".rs", ".toml", ".yml")


def checks_for(path):
    if path in TEXT_CONFIG:
        return {"text"}
    if path.startswith("docs/formats/") and path.endswith(".md"):
        return {"codegen"}  # 台帳から参照する文書と、生成する一覧
    if path.endswith(".md") and ("/" not in path or path.startswith(("docs/", ".github/"))):
        return set()
    if path in {"catalog/README.md", "formats/README.md", "profiles/README.md", "schema/README.md"}:
        return set()
    if path.startswith(".github/workflows/") and path != ".github/workflows/ci.yml":
        # xtask はリリースで配る crate の一覧も読む。
        return {"workflows", "codegen"} if path.endswith("/release.yml") else {"workflows"}
    if path == "Cargo.toml" or path.endswith("/Cargo.toml"):
        return CORE | {"deny"}
    if path.startswith("site/"):
        return {"site"}
    if path.startswith(("playground/", "packages/")):
        return {"site", "wasm"}
    if path.startswith("bindings/js/"):
        return {"site", "wasm", "codegen"}
    if path.startswith("bindings/go/"):
        return {"wasm", "codegen"}
    if path.startswith(("bindings/python/", "bindings/dotnet/", "bindings/vba/")):
        return {"native", "codegen"}
    if path.startswith(("samples/", "licenses/", "bindings/conformance/")):
        return CORE | {"site"}  # ビューアーも公開サンプルと適合テストの組を読む。
    if path.startswith(("crates/", "bindings/c/", "bindings/cli/", "templates/plugin/",
                        "xtask/src/", "xtask/verify/acc/", "xtask/verify/pdfjs/",
                        "catalog/", "formats/", "profiles/", "schema/")):
        return CORE
    return ALL


def selected_checks(paths):
    if paths is None:
        return ALL
    checks = set()
    for path in paths:
        checks.update(checks_for(path))
        if path.endswith((".md", *CODE_EXTENSIONS)) and Path(path).is_file():
            checks.add("text")
    return checks


def lint_text(paths):
    # 辞書や規則が変われば、本文を編集していないファイルにも当たる。
    if paths is None or TEXT_CONFIG.intersection(paths):
        npm = "npm.cmd" if os.name == "nt" else "npm"
        subprocess.run([npm, "run", "lint:text"], check=True)
        return
    rules = [((".md",), []), (CODE_EXTENSIONS, ["--config", ".textlintrc.code.yml"])]
    for suffixes, options in rules:
        files = ["./" + path for path in paths if path.endswith(suffixes) and Path(path).is_file()]
        # 大きな差分でも、コマンドに渡せる引数の長さに収める。
        for start in range(0, len(files), 100):
            batch = files[start:start + 100]
            subprocess.run(["node", "node_modules/textlint/bin/textlint.js", *options, *batch], check=True)


if __name__ == "__main__":
    mode, filename = sys.argv[1:]
    diff = Path(filename)
    paths = None
    if diff.exists():
        paths = [os.fsdecode(name) for name in diff.read_bytes().split(b"\0") if name]
    if mode == "select":
        print("checks=" + json.dumps(sorted(selected_checks(paths))))
    elif mode == "text":
        lint_text(paths)
    else:
        sys.exit("使い方: checks.py select|text <git diff のパス一覧>")
