"""検証キットの合成 SXF で、照合が正しい読みと、形・種類・所属・出所の食い違いを区別することを確かめる。"""

import sys
from collections import Counter
from copy import deepcopy
from pathlib import Path
from tempfile import TemporaryDirectory

import ezsxf

sys.path.insert(0, str(Path(__file__).resolve().parent))
from check_ezsxf import check_file, check_records, records, sfc_records  # noqa: E402
from compare import kit_case, usage  # noqa: E402


def check_missing_layers(folder):
    path = "342/conformance-apply_p21_raw-raw-layer.points.p21"
    entry, reading = kit_case(folder, path)
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


def check_duplicate_features(folder):
    entry, original = kit_case(folder, "input/made-duplicates.sfc")

    def compare(reading, *expected):
        failures, _ = check_file(folder, entry, reading, None, Counter())
        if len(failures) != len(expected) or any(not any(text in f for f in failures) for text in expected):
            raise AssertionError(f"同じ番号の SFC の照合: {failures}（期待は {expected}）")

    compare(original)
    rows = original["entities"]
    if [(e["source"]["record"], e["kind"], e["layer"]) for e in rows] != [(13, "line", "A"), (14, "circle", "B"), (15, "line", "B")]:
        raise AssertionError("合成 SFC の 3 要素が変わった")
    lost = "要素にならず、移さなかったことを言う診断もないフィーチャ"
    for index, row in enumerate(rows):
        # 同じ番号の異なる種類と同じ種類のどちらも、各レコードに固有の形・種類・レイヤを検査する。
        reading = deepcopy(original)
        edge = "start" if row["kind"] == "line" else "center"
        reading["entities"][index]["geometry"][edge][0] += 100
        compare(reading, f"{row['id']}（{row['kind']} #60）/{edge}/0")
        reading = deepcopy(original)
        reading["entities"][index]["kind"] = "circle" if row["kind"] == "line" else "line"
        compare(reading, f"{row['id']}: 要素の種類")
        reading = deepcopy(original)
        reading["entities"][index]["layer"] = "B" if row["layer"] == "A" else "A"
        compare(reading, f"{row['id']}: レイヤが")
        reading = deepcopy(original)
        del reading["entities"][index]
        compare(reading, lost)

    # 先頭の #60 は ezsxf の型がなく、zumen がレコード 12 の線を診断した。この診断を隣の
    # 線に付け替えても、同じ番号のほかの要素や診断で先頭の欠落を隠せない。
    reading = deepcopy(original)
    reading["diagnostics"][0]["at"][0]["record"] = 13
    compare(reading, lost)
    reading = deepcopy(original)
    reading["diagnostics"][0]["params"]["kind"] = "circle_feature"
    compare(reading, lost)
    reading = deepcopy(original)
    reading["entities"][0]["source"]["record"] = 12
    compare(reading, "レコード 12 の #60（line_feature）を型で読まない", lost)

    reading = deepcopy(original)
    del reading["entities"][1]["source"]["record"]
    compare(reading, "レコード None の #60 のフィーチャがない", lost)

    # 番号を振ったコピーの値が元の読みと違えば、その値で照合してはならない。
    data = (folder / entry["path"]).read_bytes()
    parsed = ezsxf.parse_sfc(data, strict=False)
    ids = {}
    records(data.decode("latin-1"), [], feature_ids=ids)
    next(f for f in parsed["typed_features"] if f["kind"] == "circle")["radius"] += 1
    try:
        sfc_records(parsed, data, ids, [])
    except ValueError as error:
        if "番号を戻した値が違い" not in str(error):
            raise
    else:
        raise AssertionError("元の ezsxf の読みと違う値を採用した")

    statements = {}
    at = records("DATA;#60=FIRST();#60=SECOND();ENDSEC;", [], statements)
    element = {"id": "sheet/0/i60", "source": {"instance": "60", "record": 1}}
    if check_records({"entities": [element]}, at) or statements != {60: "#60=FIRST()"}:
        raise AssertionError("P21 は先頭の実体で照合する")
    element["source"]["record"] = 2
    if not check_records({"entities": [element]}, at):
        raise AssertionError("P21 の後の同番号の実体を出所として受け付けた")
    print("同じ番号の SFC 3 要素は食い違い 0 件。各要素の形・種類・レイヤ・欠落、出所と診断の取り違えを検出した")


def check_header_features(folder):
    """ezsxf がヘッダーから回復したフィーチャを、DATA 節の同じ番号に結ばない。"""
    entry, reading = kit_case(folder, "input/made-features.sfc")
    original = (folder / entry["path"]).read_bytes()
    features = original.split(b"DATA;", 1)[1].split(b"ENDSEC;", 1)[0]
    header = features.replace(b"'0.0','0.0','100.0'", b"'43.0','0.0','100.0'", 1)
    if header == features:
        raise AssertionError("合成 SFC の線の始点が変わった")
    altered = deepcopy(reading)
    next(e for e in altered["entities"] if e["kind"] == "line")["geometry"]["start"][0] = 43
    expected = [
        f"{e['id']}: ezsxf にレコード {e['source']['record']} の #{e['source']['instance']} のフィーチャがない"
        for e in reading["entities"]
    ]
    with TemporaryDirectory() as directory:
        copy = Path(directory) / "header.sfc"
        copy.write_bytes(original.replace(b"FILE_SCHEMA", header + b"FILE_SCHEMA", 1))
        for result in [reading, altered]:
            failures, audit = check_file(copy.parent, {**entry, "path": copy.name}, result, None, Counter())
            if "salvage-header" not in dict(audit["warnings"]) or failures != expected:
                raise AssertionError(f"ヘッダーのフィーチャと DATA 節の出所を区別できなかった: {failures}")
    print("ezsxf がヘッダーから回復したフィーチャを DATA 節の 2 要素の出所にしないことを確認した")


def check_raster_frames(folder):
    entry, original = kit_case(folder, "input/made-structure.p21")

    def compare(reading, expect=None):
        failures, audit = check_file(folder, entry, reading, None, Counter())
        if not any("[132, 133, 144]" in note for note in audit["notes"]):
            raise AssertionError("コメントを持つ実体・その参照とレイヤを読んでいない")
        if failures != ([] if expect is None else [expect]):
            raise AssertionError(f"P21 の画像枠の照合: {failures}（期待は {expect}）")

    compare(original)
    # 同じ実体 #133 は、作図グループの中で画像枠、用紙の上では通常の折れ線。
    by_id = {e["id"]: e for e in original["entities"]}
    frame, plain = by_id["sheet/0/i133"], by_id["sheet/0/i133/1"]
    if (frame["kind"], plain["kind"], frame["layer"]) != ("underlay", "polyline", "FRAME/*L*/"):
        raise AssertionError("画像枠・同じ実体の折れ線・コメントに似た文字列を持つレイヤが変わった")
    reading = deepcopy(original)
    row = next(e for e in reading["entities"] if e["id"] == frame["id"])
    row.update(kind="polyline", geometry=plain["geometry"])
    compare(reading, "#133 の画像の範囲が 0 件（配置は 1 件）")

    reading = deepcopy(original)
    row = next(e for e in reading["entities"] if e["id"] == "sheet/0/i135")
    row["kind"] = "underlay"
    row.pop("geometry")
    compare(reading, "sheet/0/i135: 画像の範囲の出所が ANNOTATION_CURVE_OCCURRENCE #135（画像の作図グループの最初の 4 点以上の折れ線でない）")

    reading = deepcopy(original)
    next(d for d in reading["diagnostics"] if d["code"] == "sxf.raster-frame.missing")["at"][0]["record"] += 1
    compare(reading, "画像の作図グループ #140 に枠がなく、その定義を指す sxf.raster-frame.missing もない")
    reading = deepcopy(original)
    next(e for e in reading["entities"] if e["id"] == frame["id"])["layer"] = "OTHER"
    compare(reading, "sheet/0/i133: レイヤが 'OTHER'、PRESENTATION_LAYER_ASSIGNMENT は 'FRAME/*L*/'")
    print("P21 の画像枠と折れ線の取り違え、枠がない定義の診断、コメントを持つ実体のレイヤの違いを検出した")


if __name__ == "__main__":
    kit = usage("test_ezsxf.py")
    if kit is None:
        sys.exit(2)
    check_missing_layers(kit)
    check_duplicate_features(kit)
    check_header_features(kit)
    check_raster_frames(kit)
