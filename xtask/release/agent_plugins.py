"""固定した公式 CLI と各環境のライセンスを、デスクトップ用のプラグインに包む。"""

import argparse
import hashlib
import json
import tempfile
import zipfile
from pathlib import Path

from package import read_archive

ROOT = Path(__file__).resolve().parents[2]
PLUGIN = ROOT / "plugins/zumen"
SERVER = PLUGIN / "server"


def pack_native(archive, inputs, release, rid):
    stem = f"zumen-{release['version']}-{rid}"
    windows = rid.startswith("win-")
    source = inputs / (stem + (".zip" if windows else ".tar.gz"))
    checksums = release["targets"][rid]
    if hashlib.sha256(source.read_bytes()).hexdigest() != checksums["archive"]:
        raise ValueError(f"{source.name} の SHA-256 が一致しない")
    contents = read_archive(source)
    exe = "zumen.exe" if windows else "zumen"
    for name in [exe, exe + ".cdx.json", "THIRD_PARTY_LICENSES", "LICENSE-MIT", "LICENSE-APACHE"]:
        content = contents[f"{stem}/{name}"]
        if name == exe and hashlib.sha256(content).hexdigest() != checksums["executable"]:
            raise ValueError(f"{rid} の CLI の SHA-256 が一致しない")
        put(archive, f"server/native/{rid}/{name}", content, name == exe)


def put(archive, name, data, executable=False):
    info = zipfile.ZipInfo(name)
    info.create_system = 3
    info.external_attr = (0o100755 if executable else 0o100644) << 16
    archive.writestr(info, data, compress_type=zipfile.ZIP_DEFLATED)


def build(inputs, output):
    release = json.loads((SERVER / "release.json").read_text(encoding="utf-8"))
    manifest = json.loads((PLUGIN / "plugin.json").read_text(encoding="utf-8"))
    output.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(dir=output) as temp:
        for desktop in [True, False]:
            suffix = ".mcpb" if desktop else "-desktop.zip"
            filename = f"zumen-plugin-{manifest['version']}{suffix}"
            stage = Path(temp) / filename
            with zipfile.ZipFile(stage, "w") as archive:
                if desktop:
                    mcpb = json.loads(Path(__file__).with_name("mcpb.json").read_text(encoding="utf-8"))
                    mcpb["version"] = manifest["version"]
                    put(archive, "manifest.json", json.dumps(mcpb, ensure_ascii=False).encode())
                else:
                    for name in ["plugin.json", "mcp.json", "skills/drawing/SKILL.md"]:
                        put(archive, name, (PLUGIN / name).read_bytes())
                    for asset in sorted((PLUGIN / "assets").glob("*")):
                        put(archive, "assets/" + asset.name, asset.read_bytes())
                put(archive, "README.md", (PLUGIN / "README.md").read_bytes())
                for name in ["LICENSE-MIT", "LICENSE-APACHE"]:
                    put(archive, name, (ROOT / name).read_bytes())
                for name in ["zumen.mjs", "release.json"]:
                    put(archive, f"server/{name}", (SERVER / name).read_bytes())
                for rid in sorted(release["targets"]):
                    if desktop and rid.startswith("linux-"):
                        continue
                    pack_native(archive, inputs, release, rid)
            stage.replace(output / filename)
            print(output / filename)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("inputs", type=Path, help="公開済み CLI の zip と tar.gz のフォルダー")
    parser.add_argument("output", type=Path, help="プラグインの書き出し先")
    args = parser.parse_args()
    build(args.inputs, args.output)
