"""変更したパスから通常の CI の検査を選ぶ。未分類の入力はすべて検査する。"""

import json
import os
from pathlib import Path
import subprocess
import sys
import tomllib


WASM = {"wasm", "view2d", "react", "go"}
CORE = WASM | {"rust", "native", "abi", "acc", "codegen"}
ALL = CORE | {"site", "deny", "workflows", "selector", "text"}
NATIVE = {"zumen-abi", "zumen-c", "zumen-cli", "zumen-conformance"}
CODECS = {"zumen-acad", "zumen-pdf", "zumen-ifc", "zumen-jww", "zumen-sxf"}
TEXT_CONFIG = {
    ".textlintignore", ".textlint-allow.yml", ".textlintrc.yml", ".textlintrc.code.yml",
    "prh.yml", "package.json", "package-lock.json",
}
CODE_EXTENSIONS = (".rs", ".toml", ".yml")
# パッケージに同梱して照合する bindings/js や、キットに写す acc の README は個別に扱う。
PROSE_READMES = {
    "catalog/README.md", "formats/README.md", "fuzz/README.md", "profiles/README.md", "schema/README.md",
    "packages/react/README.md", "packages/view2d/README.md", "playground/README.md", "site/README.md",
    "xtask/release/README.md", "xtask/verify/README.md",
    "xtask/verify/pdfjs/README.md", "xtask/verify/pdfmeasure/README.md",
}


def checks_for(path):
    if path in TEXT_CONFIG:
        return {"text"}
    if path.startswith("docs/formats/") and path.endswith(".md"):
        return {"codegen"}  # 台帳から参照する文書と、生成する一覧
    if path.endswith(".md") and ("/" not in path or path.startswith(("docs/", ".github/"))):
        return set()
    if path in PROSE_READMES:
        return set()
    if path in {".github/ci/checks.py", ".github/ci/test_checks.py"}:
        return {"selector"}
    if path.startswith(".github/workflows/") and path != ".github/workflows/ci.yml":
        # xtask はリリースで配る crate の一覧も読む。
        return {"workflows", "codegen"} if path.endswith("/release.yml") else {"workflows"}
    if path == "Cargo.toml" or path.endswith("/Cargo.toml"):
        return CORE | {"deny"}
    if path.startswith("site/"):
        return {"site"}
    if path == "bindings/js/test/browser/view2d.mjs" or path.startswith("packages/view2d/example/"):
        return {"view2d"}
    if path.startswith("packages/view2d/test/") and path.endswith(".mjs"):
        return {"view2d"}
    if path.startswith("packages/view2d/"):
        return {"site", "view2d", "react"}  # React とサイトも表示部品を使う。
    if path in {"bindings/js/test/browser/react.mjs", "bindings/js/test/browser/react-interactions.mjs",
                "bindings/js/test/browser/react-resize.mjs"} or path.startswith("playground/test/"):
        return {"react"}
    if path.startswith(("playground/", "packages/react/")):
        return {"site", "react"}
    if path.startswith("packages/"):
        return WASM | {"site"}
    if path.startswith("bindings/js/"):
        return {"site", "wasm", "view2d", "react", "codegen"}
    if path.startswith("bindings/go/"):
        if path in {"bindings/go/constants.go", "bindings/go/messages.go"}:
            return {"go", "codegen"}
        return {"go"}
    if path.startswith(("bindings/python/", "bindings/dotnet/", "bindings/vba/")):
        return {"native", "codegen"}
    if path.startswith(("samples/", "licenses/", "bindings/conformance/")):
        return CORE | {"site"}  # ビューアーも公開サンプルと適合テストの組を読む。
    if path.startswith(("crates/", "bindings/c/", "bindings/cli/", "templates/plugin/",
                        "xtask/src/", "catalog/", "formats/", "profiles/", "schema/")):
        return CORE
    if path.startswith("xtask/verify/acc/"):
        return {"acc"}
    return ALL


def affected_packages(sources, tests):
    # Cargo.lock は任意・開発用・対象環境別の依存も持つ。選別だけのために
    # ランナーへ Rust を入れずに、パス依存の利用側をたどる。
    with open("Cargo.lock", "rb") as file:
        locked = tomllib.load(file)["package"]
    local = {
        p["name"]: {dep.split(" ", 1)[0] for dep in p.get("dependencies", [])}
        for p in locked if "source" not in p
    }
    names = {}
    for directory in sources | tests:
        manifest = Path(directory, "Cargo.toml")
        if not manifest.is_file():
            return None
        with manifest.open("rb") as file:
            name = tomllib.load(file).get("package", {}).get("name")
        if name not in local:
            return None
        names[directory] = name

    affected = {names[directory] for directory in sources}
    while True:
        users = {name for name, dependencies in local.items() if dependencies & affected}
        if users <= affected:
            break
        affected.update(users)
    return affected | {names[directory] for directory in tests}


def select(paths, event):
    full_run = event in {"schedule", "workflow_dispatch"}
    if full_run:
        paths = None
    checks = set()
    sources, tests = set(), set()
    for path in paths or []:
        parts = path.split("/", 3)
        directory = "/".join(parts[:2])
        member = parts[0] == "crates" or directory in {"bindings/c", "bindings/cli", "templates/plugin"}
        if path.startswith("xtask/src/"):
            sources.add("xtask")
        elif path.startswith(("bindings/c/include/", "bindings/cli/resources/")):
            sources.add(directory)
            if path == "bindings/c/include/zumen.h":
                checks.add("wasm")  # wasm.mjs はヘッダーから ABI の版と状態コードを読む。
        elif member and len(parts) == 4 and parts[2] in {"src", "tests", "examples"}:
            # ABI と図面モデルは、利用側が有効にする feature でもテストする。
            if parts[2] == "src" or parts[1] in {"zumen-abi", "zumen-drawing"}:
                sources.add(directory)
            else:
                tests.add(directory)
        elif path.startswith(("xtask/verify/pdfjs/", "xtask/verify/pdfmeasure/")) and not path.endswith(".md"):
            tests.add("crates/zumen-pdf")
            if path.startswith("xtask/verify/pdfjs/") and path != "xtask/verify/pdfjs/read.mjs":
                checks.add("view2d")  # Canvas 2D は同じ pdf.js をブラウザーへ直接読み込む。
        else:
            checks.update(checks_for(path))
        if path.endswith((".md", *CODE_EXTENSIONS)) and Path(path).is_file():
            checks.add("text")

    packages = set()
    if paths is None:
        checks.update(ALL)
    if "rust" in checks:
        packages = None
    elif sources or tests:
        packages = affected_packages(sources, tests)
        checks.update({"rust", "codegen"})
        if packages is None:
            checks.update(ALL)
        else:
            if packages & NATIVE:
                checks.add("native")
            if packages & {"zumen-abi", "zumen-render", "zumen-plugin-template"}:
                checks.update(WASM)
            if "zumen-abi" in packages:
                checks.add("abi")
            if "zumen-cli" in packages:
                checks.add("acc")

    codecs = CODECS if packages is None else CODECS & packages
    if event == "pull_request":
        checks.update(WASM | {"native", "codegen", "deny"})
    modules = ""
    if "wasm" in checks:
        modules = "drawing bim civil pointcloud plugin"
    elif "react" in checks:
        modules = "drawing"
    return {
        "checks": sorted(checks),
        "wasm_modules": modules,
        "wasm_full": bool(checks & {"wasm", "view2d", "go"}),
        "wasm_browser": bool(checks & {"wasm", "view2d", "react"}),
        "cargo_args": "--workspace" if packages is None else cargo_args(packages),
        "codecs": " ".join(sorted(codecs)),
        "native_args": "--workspace" if full_run else cargo_args(NATIVE),
        "linux_args": "--workspace" if packages is None else cargo_args(packages | NATIVE),
    }


def cargo_args(packages):
    return " ".join(f"-p {name}" for name in sorted(packages))


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
        plan = select(paths, os.environ.get("GITHUB_EVENT_NAME"))
        print("plan=" + json.dumps(plan))
    elif mode == "text":
        lint_text(paths)
    else:
        sys.exit("使い方: checks.py select|text <git diff のパス一覧>")
