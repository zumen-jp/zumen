"""検証キットの合成 P21 から、追加した点のレイヤ所属だけを外し、照合が食い違いを検出することを確かめる。"""

import json
import sys
from collections import Counter
from pathlib import Path
from tempfile import TemporaryDirectory

sys.path.insert(0, str(Path(__file__).resolve().parent))
from check_ezsxf import check_file  # noqa: E402
from compare import usage  # noqa: E402


def check_missing_layers(folder):
    path = "342/conformance-apply_p21_raw-raw-layer.points.p21"
    manifest = json.loads((folder / "manifest.json").read_text(encoding="utf-8"))
    entry = next(item for item in manifest["files"] if item["path"] == path)
    reading = json.loads((folder / "zumen" / (path + ".json")).read_text(encoding="utf-8"))
    failures, _ = check_file(folder, entry, reading, None, Counter())
    if failures:
        raise AssertionError(f"元の P21 で食い違いが出た: {failures}")

    # 用紙の並びと形はそのまま、レイヤの所属だけを欠く複製を一時フォルダーに作る。
    original = (folder / path).read_bytes()
    assignment = "#20=PRESENTATION_LAYER_ASSIGNMENT('図枠',' ',(#18,#42,#52));".encode("cp932")
    if original.count(assignment) != 1:
        raise AssertionError("合成 P21 のレイヤの所属一覧が変わった")
    with TemporaryDirectory() as directory:
        copy = Path(directory) / "missing-layer.p21"
        copy.write_bytes(original.replace(assignment, assignment.replace(b",#42,#52", b"")))
        failures, _ = check_file(copy.parent, {**entry, "path": copy.name}, reading, None, Counter())
    expected = [
        f"変更 {edit}（sheet/0/i{instance}）のレイヤが None（期待は '図枠'）"
        for edit, instance in enumerate((42, 52))
    ]
    if failures != expected:
        raise AssertionError(f"所属を外した 2 点のレイヤの食い違いを検出できなかった: {failures}")
    print("元の P21 は食い違い 0 件、所属を外した P21 は 2 点のレイヤの食い違いを検出した")


if __name__ == "__main__":
    kit = usage("test_ezsxf.py")
    if kit is None:
        sys.exit(2)
    check_missing_layers(kit)
