"""workspace の別 member の feature が混ざる実例で、SBOM・埋込情報とビルドの照合を検査する。"""

import json
import os
import re
import shutil
import tempfile
from pathlib import Path

import sbom
from build import build_graph, compare_build_graph
from licenses import ROOT, feature_args


def workspace_fixture(workspace):
    (workspace / "Cargo.toml").write_text('[workspace]\nmembers = ["app","base","common","extra","other"]\nresolver = "3"\n')
    shutil.copyfile(ROOT / "rust-toolchain.toml", workspace / "rust-toolchain.toml")
    manifests = {
        "app": ('zumen-cli', '''
[[bin]]
name = "zumen"
path = "src/main.rs"
[features]
drawing = []
keep = ["dep:extra"]
plugin = ["dep:extra", "base/plugin"]
[dependencies]
base = {path="../base"}
common = {path="../common"}
extra = {path="../extra", optional=true}
'''),
        "base": ('base', '''
[features]
plugin = ["dep:common"]
[dependencies]
common = {path="../common", optional=true}
'''),
        "other": ('other', '[dependencies]\nzumen-cli = {path="../app", features=["plugin"]}\n'),
        "common": ('common', ''), "extra": ('extra', ''),
    }
    for folder, (name, settings) in manifests.items():
        crate = workspace / folder
        (crate / "src").mkdir(parents=True)
        (crate / "Cargo.toml").write_text(f'[package]\nname="{name}"\nversion="1.0.0"\nedition="2021"\n' + settings)
        (crate / "src" / "lib.rs").write_text("pub fn value() -> u32 { 1 }\n")
    (workspace / "app/src/main.rs").write_text("fn main() { assert_eq!(base::value(), common::value()); }\n")


def run(workspace):
    workspace_fixture(workspace)
    env = {**os.environ, "CARGO_NET_OFFLINE": "true", "SOURCE_DATE_EPOCH": "1", "CARGO_TARGET_DIR": str(workspace / "target")}
    sbom.raw_command(["cargo", "generate-lockfile", "--offline"], workspace, env)
    rustc = sbom.raw_command(["rustc", "-vV"], workspace, env).decode()
    host = re.search(r"^host: (\S+)", rustc, re.MULTILINE)[1]
    manifest = workspace / "app/Cargo.toml"
    lock = (workspace / "Cargo.lock").read_bytes()
    for module, failure in (("drawing", "コンパイラーの実際の依存"), ("keep", "cargo tree と SBOM の辺"), ("plugin", None)):
        # drawing は extra を作らない。keep は全 crate を作るが、base -> common の辺は使わない。
        flags = ["--manifest-path", str(manifest), "--target", host, *feature_args(module)]
        output = sbom.raw_command(["cargo", "auditable", "build", "--release", "--locked", "--bin", "zumen",
                                   "--message-format=json-render-diagnostics", *flags], workspace, env)
        messages = [json.loads(line) for line in output.splitlines()]
        root = next(m for m in messages if m["reason"] == "compiler-artifact" and m["target"]["name"] == "zumen")
        binary = Path(root["executable"]).read_bytes()
        actual = build_graph(workspace, manifest, host, module, messages, root["package_id"], root["features"])
        sbom.raw_command(["cargo", "cyclonedx", "--describe", "binaries", "--all", "--format", "json",
                          "--spec-version", "1.5", *flags], workspace, env)
        metadata = sbom.metadata_for(workspace, str(manifest), host, module)
        raw = json.loads(next(manifest.parent.glob("*.cdx.json")).read_bytes())
        notice = {"package": "zumen-cli", "target": host, "features": module, "dependencies": []}
        document = sbom.normalize(raw, metadata, notice, Path(root["executable"]).name, module, "1" * 40, lock, "1")
        expected = sbom.document_graph(document)
        sbom.check(sbom.extracted_graph(binary) == expected and len(expected[1]) == 4, "2 ツールが同じ 4 crate を返す条件が違う")
        sbom.check(len(actual["compiled"]) == (3 if module == "drawing" else 4), "実際にコンパイルした crate 数が違う")
        try:
            compare_build_graph(actual, document)
        except ValueError as err:
            if failure is None or failure not in str(err):
                raise
        else:
            sbom.check(failure is None, "別 member の feature を含む SBOM を受け入れた")
        print(f"{module}: コンパイラー {len(actual['compiled'])} 件、SBOM と埋込情報 4 件、"
              + ("一致" if failure is None else "不一致を拒否"))


if __name__ == "__main__":
    with tempfile.TemporaryDirectory(prefix="zumen-feature-scope-") as tmp:
        run(Path(tmp))
