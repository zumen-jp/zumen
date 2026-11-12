"""manylinux_2_28 の実環境で、梱包対象のライブラリ・CLI と wheel を確かめる（#789）。"""

import argparse
import os
import subprocess
import tempfile
from pathlib import Path

from licenses import ROOT, TARGETS
from linux_abi import verify_environment, verify_linux
from package import VERSION


def run(command, **kwargs):
    subprocess.run([str(item) for item in command], cwd=ROOT, check=True, **kwargs)


def check(target, built, packages):
    verify_environment(target)
    library, cli = built / target["library"], built / target["exe"]
    for path in (library, cli):
        verify_linux(path.read_bytes(), target)
    wheel = packages / "wheels" / f"zumen-{VERSION}-py3-none-{target['wheel']}.whl"
    for tag in ("cp39-cp39", "cp314-cp314"):
        python = Path("/opt/python") / tag / "bin/python"
        run([python, ROOT / "bindings/python/tests/conformance.py", library])
        with tempfile.TemporaryDirectory(prefix="zumen-wheel-") as temporary:
            run([python, "-m", "pip", "install", "--no-index", "--no-deps", "--target", temporary, wheel])
            # 適合テストはソースのラッパーを使う。wheel 自体の import と同梱ライブラリの load は別に確かめる。
            run([python, "-c", "from zumen import load; "
                 "session = load().open(); "
                 "assert session.call({'v': 1, 'op': 'capabilities'})['result']['package']; session.close()"],
                env={**os.environ, "PYTHONPATH": temporary})
    run(["cargo", "test", "--locked", "--release", "--target", target["target"],
         "-p", "zumen-cli", "--test", "conformance"], env={**os.environ, "ZUMEN_CONFORMANCE_CLI": str(cli)})
    run(["dotnet", "run", "--project", "bindings/dotnet/tests/Conformance", "--framework", "net10.0", "--", library],
        env={**os.environ, "DOTNET_CLI_TELEMETRY_OPTOUT": "1", "DOTNET_NOLOGO": "1"})
    print(f"{target['rid']}: glibc 2.28 で梱包対象の Python・CLI・.NET の適合テストと wheel の読み込みを確認した")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--rid", choices=("linux-x64", "linux-arm64"), required=True)
    parser.add_argument("--built", type=Path, required=True)
    parser.add_argument("--packages", type=Path, required=True)
    args = parser.parse_args()
    check(next(target for target in TARGETS if target["rid"] == args.rid), args.built.resolve(), args.packages.resolve())
