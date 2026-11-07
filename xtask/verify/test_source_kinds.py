"""DXF・DWG・JWW の照合が、形を返さない要素の種類と、描かない JWW のレコードを区別することを確かめる。"""

import json
import sys
from collections import Counter
from copy import deepcopy
from functools import partial
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import check_acadsharp  # noqa: E402
import check_ezdxf  # noqa: E402
import check_ezjww  # noqa: E402
from compare import kit_case, usage  # noqa: E402


def check_kind(folder, path, check, kind, wrong_kind):
    entry, original = kit_case(folder, path)
    reading = deepcopy(original)
    element = next(e for e in reading["entities"] if e["kind"] == kind)
    # 公開 DXF・DWG の寸法は geometry も返す。要求するフィールドから geometry を外したときの読みを照合する。
    element.pop("geometry", None)
    failures, _ = check(entry, reading, None, Counter())
    if failures:
        raise AssertionError(f"形を持たない {kind} で食い違いが出た: {failures}")
    for value, expected in [(wrong_kind, "の出所が"), ("unknown", "の対応がない")]:
        element["kind"] = value
        failures, _ = check(entry, reading, None, Counter())
        if len(failures) != 1 or not failures[0].startswith(f"{element['id']}: 要素の種類 {value}・形 None {expected}"):
            raise AssertionError(f"{kind} の種類を {value} に変えた食い違いが違う: {failures}")


def check_hidden_text(folder):
    entry, original = kit_case(folder, "input/made-classes.jww")
    expected = [(4, "text"), (5, "hatch"), (6, "dimension"), (7, "underlay")]
    if [(e["source"]["record"], e["kind"]) for e in original["entities"]] != expected:
        raise AssertionError("合成 JWW の描く文字・ソリッド・寸法・画像が変わった")
    for record in (2, 3):
        reading = deepcopy(original)
        text = deepcopy(reading["entities"][0])
        text["source"]["record"] = record
        text["id"] = f"space/0/test-{record}"
        reading["entities"].append(text)
        failures, _ = check_ezjww.check_file(folder, entry, reading, None, Counter())
        if failures != [f"{text['id']}: 描かない文字（TEXT のレコード {record}）が要素になった"]:
            raise AssertionError(f"設定か制御の文字を要素にした食い違いを検出できなかった: {failures}")
    reading = deepcopy(original)
    reading["diagnostics"].clear()
    failures, _ = check_ezjww.check_file(folder, entry, reading, None, Counter())
    if len(failures) != 1 or "診断もない図形: {'LINE': 1}" not in failures[0]:
        raise AssertionError(f"レコード 1 の線の診断の欠落を検出できなかった: {failures}")


if __name__ == "__main__":
    folder = usage("test_source_kinds.py")
    if folder is None:
        sys.exit(2)
    dumps = json.loads((folder / "acadsharp-dump.json").read_text(encoding="utf-8"))
    check_kind(folder, "input/zumen-arcdim-r2004_ascii.dxf", partial(check_ezdxf.check_file, folder), "dimension", "hatch")
    check_kind(folder, "input/zumen-arcdim-r2004.dwg", partial(check_acadsharp.check_file, dumps["files"]), "dimension", "hatch")
    check_kind(folder, "input/made-classes.jww", partial(check_ezjww.check_file, folder), "hatch", "text")
    check_hidden_text(folder)
    print("DXF・DWG・JWW で形のない要素の種類の違い、JWW で設定・制御の文字と診断の欠落を検出した")
