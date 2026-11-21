"""自作の有理 2 次スプラインを AutoCAD で DWG にし、公開 apply と 2 つの読み手で照合する。

ezdxf 1.4.5 と .NET 10 SDK のある環境で、
python weighted_spline.py <zumen.exe> <accoreconsole.exe> <リポジトリ外の空の出力先> を実行する。
"""
import argparse
import math
from pathlib import Path
import subprocess
import sys

import ezdxf

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))
from check_autocad import audit_counts, decode, mark, markers
from make import Zumen, open_file, read


def console(exe, source, script):
    path = source.with_suffix(".scr")
    path.write_text("\n".join(['(setvar "FILEDIA" 0)', *script, mark("done"), "_.QUIT _Y", ""]),
                    encoding="ascii")
    run = subprocess.run([exe, "/i", source, "/s", path], cwd=source.parent, stdin=subprocess.DEVNULL,
                         stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=90, check=True)
    output = decode(run.stdout)
    assert "done" in dict(markers(output)), "自作スクリプトが完了しませんでした。"
    return output


def write_edits(cli, folder):
    for name in ("move", "add"):
        z = Zumen(str(cli))
        try:
            open_file(z, "src", (folder / "base.dwg").read_bytes())
            entity, = read(z, "src")["entities"]
            g = entity["geometry"]
            assert g["weights"] == [1, math.sqrt(0.5), 1] and g["control"] == [[12, 0], [12, 12], [0, 12]]
            if name == "move":
                edit = {"kind": "transform", "id": entity["id"], "hash": entity["hash"],
                        "matrix": [1, 0, 10, 0, 1, 20]}
            else:
                geometry = dict(g, control=[[x + 30, y + 40] for x, y in g["control"]])
                edit = {"kind": "add", "place": "space/0", "geometry": geometry}
            status, result = z.call({"op": "apply", "doc": "src", "edits": [edit]})
            assert status == "ok", result["diagnostics"]
            (folder / (name + ".dwg")).write_bytes(z.take(result["blobs"][0]["id"]))
        finally:
            z.close()


def observe(exe, source, version, offsets):
    # 等分点はメモリ上だけに作り、書き出した元の DWG・DXF を変更しない。
    text = console(exe, source, [
        mark("dbmod", '(getvar "DBMOD")'), mark("audit-begin"), "_.AUDIT _N", mark("audit-end"),
        f'(command "_.DXFOUT" (strcat (getvar "DWGPREFIX") "{source.stem}.dxf") "_V" "{version}" "16")',
        f'(setq f (open (strcat (getvar "DWGPREFIX") "{source.stem}.points") "w"))',
        '(setq sel (ssget "_X" (list (cons 0 "SPLINE"))))',
        '(defun z-div (e / old p q) (setq old (entlast)) (command "_.DIVIDE" e 4) (setq p (entnext old))'
        ' (while p (setq q (cdr (assoc 10 (entget p))))'
        ' (write-line (strcat (rtos (car q) 2 14) " " (rtos (cadr q) 2 14) " " (rtos (caddr q) 2 14)) f)'
        ' (setq p (entnext p))))',
        '(setq i 0)', '(repeat (sslength sel) (z-div (ssname sel i)) (setq i (1+ i)))', '(close f)',
    ])
    assert dict(markers(text))["dbmod"] == "0" and audit_counts(text, "audit-begin", "audit-end") == (0, 0)
    points = [tuple(map(float, line.split())) for line in
              source.with_suffix(".points").read_text(encoding="ascii").splitlines()]
    expected = [(dx + 12 * math.cos(math.pi * i / 8), dy + 12 * math.sin(math.pi * i / 8), 0)
                for dx, dy in offsets for i in (1, 2, 3)]
    assert len(points) == len(expected)
    error = max(math.dist(p, q) for p, q in zip(sorted(points), sorted(expected)))
    assert error < 0.0002, (source.stem, error)
    curves = list(ezdxf.readfile(source.with_suffix(".dxf")).modelspace().query("SPLINE"))
    assert len(curves) == len(offsets)
    curves.sort(key=lambda s: s.control_points[0][0])
    for spline, (dx, dy) in zip(curves, offsets):
        assert list(spline.weights) == [1, math.sqrt(0.5), 1] and list(spline.knots) == [0, 0, 0, 1, 1, 1]
        midpoint = spline.construction_tool().point(0.5)
        assert math.dist(midpoint, (dx + 12 / math.sqrt(2), dy + 12 / math.sqrt(2), 0)) < 1e-12
    return error


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("cli", type=Path)
    parser.add_argument("accoreconsole", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    root = args.output.resolve()
    if root.is_relative_to(HERE.parents[2]) or root.exists() and any(root.iterdir()):
        parser.error("リポジトリ外の空の出力先を指定してください。")
    error = 0.0
    for version in ("2013", "2018"):
        folder = root / version
        folder.mkdir(parents=True, exist_ok=True)
        seed = folder / "seed.dxf"
        doc = ezdxf.new("R" + version)
        doc.modelspace().add_rational_spline([(12, 0, 0), (12, 12, 0), (0, 12, 0)],
            [1, math.sqrt(0.5), 1], degree=2, knots=[0, 0, 0, 1, 1, 1])
        doc.saveas(seed)
        console(args.accoreconsole.resolve(), seed, [
            f'(command "_.SAVEAS" "{version}" (strcat (getvar "DWGPREFIX") "base.dwg"))'])
        write_edits(args.cli.resolve(), folder)
        for name, offsets in [("base", [(0, 0)]), ("move", [(10, 20)]), ("add", [(0, 0), (30, 40)])]:
            error = max(error, observe(args.accoreconsole.resolve(), folder / (name + ".dwg"), version, offsets))
    subprocess.run(["dotnet", "run", "--file", HERE / "weighted_spline.cs", "--", root], check=True)
    print(f"AutoCAD 6 DWG、8 曲線、等分点 24 個、最大差 {error:.8f} 図面単位")


if __name__ == "__main__":
    main()
