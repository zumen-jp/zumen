"""地物の図面・ZMS3・診断・予算の全バイトをネイティブと wasm で照合する。"""
import argparse
import hashlib
from pathlib import Path
import subprocess
import sys


def main():
    sys.stdout.reconfigure(encoding="utf-8")
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("executable", type=Path)
    parser.add_argument("--wasm", action="store_true")
    parser.add_argument("--record", action="store_true")
    args = parser.parse_args()
    executable = args.executable.resolve()
    if not executable.exists() and executable.with_suffix(".exe").exists():
        executable = executable.with_suffix(".exe")
    folder = Path(__file__).resolve().parent
    command = [str(executable)]
    if args.wasm:
        command = ["node", "--no-warnings", str(folder / "../../zumen-geom2d/tests/topology-wasi.mjs"), str(executable)]
    result = subprocess.run(command, capture_output=True, timeout=60)
    if result.returncode:
        raise SystemExit(result.stderr.decode("utf-8", errors="replace"))
    if not result.stdout.startswith(b"zumen-geo visual 1\n"):
        raise SystemExit("地物の記録のヘッダーが違う")
    digest = hashlib.sha256(result.stdout).hexdigest()
    expected = folder / "visual.sha256"
    if args.record:
        expected.write_text(digest + "\n", encoding="ascii")
    elif digest != expected.read_text(encoding="ascii").strip():
        raise SystemExit(f"地物の記録が違う: {digest}")
    print(f"地物の図面とシーン: {len(result.stdout)} バイト、SHA-256 {digest}")


if __name__ == "__main__":
    main()
