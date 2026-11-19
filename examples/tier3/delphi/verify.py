"""Delphi の Win64 コンパイラーで公開 DXF のサンプルをビルド・実行する。"""

import argparse
from pathlib import Path
import shutil
import subprocess
import sys
from tempfile import TemporaryDirectory


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("dll", type=Path, help="Win64 の zumen_abi1.dll")
    parser.add_argument("--compiler", default="dcc64.exe", help="Delphi の Win64 コンパイラー")
    args = parser.parse_args()
    compiler = shutil.which(args.compiler)
    if not compiler:
        parser.error("dcc64.exe を含む RAD Studio の環境で実行するか、--compiler で指定してください。")
    if not args.dll.is_file():
        parser.error("zumen_abi1.dll が見つかりません。")

    source = Path(__file__).resolve().with_name("zumen_summary.dpr")
    tier3 = source.parent.parent
    drawing = tier3.parents[1] / "samples/dwg/zumen-layout-r2000_ascii.dxf"
    expected = (tier3 / "expected.txt").read_bytes().replace(b"\r\n", b"\n")
    with TemporaryDirectory() as tmp:
        work = Path(tmp)
        shutil.copyfile(source, work / source.name)
        shutil.copyfile(args.dll, work / "zumen_abi1.dll")
        subprocess.run([Path(compiler).resolve(), f"-E{work}", source.name], cwd=work, check=True)
        actual = subprocess.run(
            [work / "zumen_summary.exe", drawing], cwd=work, stdout=subprocess.PIPE, check=True
        ).stdout

    actual = actual.replace(b"\r\n", b"\n")
    if actual != expected:
        raise SystemExit(f"出力が期待値と異なります。期待値 {expected!r}、出力 {actual!r}")
    sys.stdout.buffer.write(actual)


if __name__ == "__main__":
    main()
