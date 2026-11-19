"""公開 DXF の実行結果を AutoCAD Core Console の出力から取り出し、共通の期待値と照らす。"""

import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile


def main(cli, console):
    here = Path(__file__).resolve().parent
    drawing = here.parents[2] / "samples/dwg/zumen-layout-r2000_ascii.dxf"
    expected = (here.parent / "expected.txt").read_bytes()
    source = (here / "cli.lsp").read_text(encoding="utf-8")
    # 自作の式だけをスクリプトに渡す。製品の信頼するフォルダーを変えない。
    forms = "\n".join(line for line in source.splitlines() if not line.lstrip().startswith(";"))
    script = forms + """
(setq observed (zumen-summary (getenv "ZUMEN_TIER3_EXE") (getenv "ZUMEN_TIER3_DRAWING")))
(if observed (princ (strcat "\\nZUMEN-TIER3-BEGIN\\n" observed "ZUMEN-TIER3-END\\n")))
_.QUIT
"""
    with tempfile.TemporaryDirectory(prefix="zumen-autolisp-") as directory:
        work = Path(directory)
        sample = work / drawing.name
        shutil.copyfile(drawing, sample)
        batch = work / "summary.scr"
        batch.write_text(script, encoding="ascii")
        env = dict(os.environ, ZUMEN_TIER3_EXE=str(Path(cli).resolve()), ZUMEN_TIER3_DRAWING=str(sample))
        run = subprocess.run(
            [str(Path(console).resolve()), "/i", str(sample), "/s", str(batch)],
            cwd=work, env=env, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT, timeout=60, check=True,
        )

    text = run.stdout.decode("utf-16le").replace("\r\n", "\n")
    _, begin, remainder = text.partition("\nZUMEN-TIER3-BEGIN\n")
    observed, end, _ = remainder.partition("ZUMEN-TIER3-END\n")
    if not begin or not end:
        raise SystemExit("AutoLISP の結果を取得できませんでした。")
    if observed.encode("utf-8") != expected:
        raise SystemExit("AutoLISP の結果が expected.txt と違います。")
    sys.stdout.write(observed)


if __name__ == "__main__":
    if len(sys.argv) != 3:
        raise SystemExit("verify.py <zumen.exe> <accoreconsole.exe>")
    main(*sys.argv[1:])
