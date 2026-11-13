"""検証キットを外部の実装と照らすスクリプト（check_*.py）が共有する、値の比べ方とキットの回し方（#503）。"""

import json
import math
import sys
import tomllib
from collections import Counter
from pathlib import Path

# zumen の中の ID で、外部の実装と比べないフィールド
SKIP = {"style_id"}
# 角度のフィールド。360 度の違いは同じとみなす
ANGLES = {"rotation", "start_angle", "end_angle"}
# 比べた数の名前（結果の JSON のキー）→ 表示
LABELS = {"elements": "出所で結んだ要素", "shapes": "形を比べた要素", "expected": "変更で決めた形"}
# 図形を図面モデルに移さなかったことを言う診断（catalog/codes.toml）。黙って捨てない照合は、これだけを数える。
# params の kind は、DXF・DWG では図形の種類（DXF の名前）、JWW ではクラスの名前、SFC ではフィーチャの名前、P21
# では実体の名前（crates/zumen-sxf/src/drawing.rs の FeatureRecord）。kind を持たない code は、レコードだけで結ぶ
# （DWG は、KINDLESS の種類の件数に足す）
LIFTED_AWAY = {
    "dxf.lift.dropped",
    "dxf.lift.invalid",
    "dxf.non-planar.dropped",
    "dxf.paper-space.dropped",
    "dxf.code-page.unsupported",
    "dxf.mtext-columns.unsupported",
    "dxf.viewport.unsupported",
    "jww.lift.dropped",
    "jww.lift.invalid",
    "sxf.lift.dropped",
    "sxf.lift.invalid",
    "sxf.sfig.missing",
    "sxf.sfig-cycle.invalid",
    "sxf.raster-frame.missing",
    "sxf.p21-sheet.missing",
    # 後続の同番号実体と、採用しない用紙そのもののレコードを指す（#550）。
    "sxf.p21-instance.dropped",
    "sxf.p21-sheet.dropped",
}
# kind を持たない code が言いうる図形の種類（DXF の名前）。段組みは、MTEXT と、MULTILEADER の文字と、複数行の
# 属性で言う（crates/zumen-acad/src/drawing.rs の columns を返す所）
KINDLESS = {
    "dxf.mtext-columns.unsupported": {"MTEXT", "MULTILEADER", "MLEADER", "ATTRIB", "ATTDEF"},
    "dxf.viewport.unsupported": {"VIEWPORT"},
}
# catalog/codes.toml の fidelity が dropped でも、図形は移し、その一部だけを落としたことを言う診断と、書き出すときの
# 診断。黙って捨てない照合では数えない
PARTLY = {
    # 再符号化の出力で除いたもの。L2 に図形を移さなかったことの根拠にはしない。
    "dwg.dxf-header.dropped",
    "dwg.dxf-record.dropped",
    "dwg.dxf-reference.dropped",
    "dwg.table-format.unsupported",
    "dxf.arc-dimension-leader.dropped",
    "dxf.cell-value.dropped",
    "dxf.clip.dropped",
    "dxf.extension-dictionary.dropped",
    "dxf.extension-dictionary.unsupported",
    "dxf.leader-line-style.dropped",
    "dxf.line-type-element.dropped",
    "sxf.saf-figure.dropped",
    # 要素は採用したレイヤ上に残り、ほかのレイヤへの所属だけを落とす（#550）。
    "sxf.p21-layer-member.dropped",
    # 書き出すとき
    "dxf.entity.dropped",
    "dxf.group-code.dropped",
    "dxf.header-variable.dropped",
    "dxf.section.dropped",
    "dxf.table.dropped",
}
# 診断の code の台帳
CATALOG = Path(__file__).resolve().parents[2] / "catalog" / "codes.toml"

# DXF・DWG: 図面モデルの形 → 図形の種類（DXF の名前）
TYPES = {
    "line": {"LINE"},
    "circle": {"CIRCLE"},
    "arc": {"ARC"},
    "ellipse": {"ELLIPSE"},
    "polyline": {"LWPOLYLINE", "POLYLINE"},
    "spline": {"SPLINE"},
    "point": {"POINT"},
    "text": {"TEXT"},
    "formatted_text": {"MTEXT"},
    "symbol": {"INSERT"},
    "dimension": {"DIMENSION", "ARC_DIMENSION"},
    "leader": {"LEADER"},
    "callout": {"MULTILEADER", "MLEADER"},
}
# DXF・DWG の要素の種類 → 出所の図形。entities が geometry を持たない要素も kind を返す。
KINDS = {
    **{k: TYPES[k] for k in ("line", "circle", "polyline", "point", "text", "formatted_text", "symbol", "dimension", "leader", "callout")},
    "curve": TYPES["arc"] | TYPES["ellipse"] | TYPES["spline"],
    "hatch": {"HATCH", "SOLID", "TRACE"},
    "underlay": {"IMAGE", "PDFUNDERLAY", "DWFUNDERLAY", "DGNUNDERLAY"},
    "table": {"ACAD_TABLE"},
}
# DXF・DWG: 形を比べる種類。モデル空間のこの種類の図形には、zumen の要素の形がある
COMPARABLE = {"LINE", "CIRCLE", "ARC", "ELLIPSE", "LWPOLYLINE", "POINT", "TEXT", "MTEXT", "INSERT"}
# DXF・DWG: PDMODE の形（決定 0183）
MARKS = {"dot": 0, "none": 1, "plus": 2, "cross": 3, "tick": 4}
# DXF・DWG: MTEXT の寄せ方（DXF Reference の MTEXT の 71。1 から）
ATTACHMENTS = [
    "top_left",
    "top_center",
    "top_right",
    "middle_left",
    "middle_center",
    "middle_right",
    "bottom_left",
    "bottom_center",
    "bottom_right",
]


def close(a, b, tolerance=1e-9):
    return math.isclose(a, b, rel_tol=tolerance, abs_tol=tolerance)


def close_angle(a, b):
    d = (a - b) % 360.0
    return min(d, 360.0 - d) <= 1e-9 * max(1.0, abs(a), abs(b))


def differences(want, got, path="", both=False):
    """want のフィールドが got と同じでない所。both なら、got にだけあるフィールドも食い違いにする（zumen の読みと外部の実装の読みを
    比べるとき。zumen がフィールドを落としても見つける）。変更で決めた形は、既定の値のフィールドを書かないので both にしない。
    want の null は、got も null のときだけ同じとする（zumen の JSON は NaN と無限大を書かず、null にする。決定 0012、
    crates/zumen-json/src/write.rs）"""
    out = []
    if want is None:
        if got is not None:
            out.append(f"{path}: {got!r}（期待は None）")
    elif isinstance(want, dict):
        if not isinstance(got, dict):
            return [f"{path}: {got!r}（期待は {want!r}）"]
        if both:
            out += [f"{path}/{key}: zumen の読みにない（外部の実装は {value!r}）" for key, value in got.items() if key not in want and key not in SKIP]
        for key, value in want.items():
            if key in SKIP:
                continue
            if key not in got:
                out.append(f"{path}/{key}: ない（期待は {value!r}）")
                continue
            if key in ANGLES and isinstance(value, (int, float)):
                if not isinstance(got[key], (int, float)) or not close_angle(float(value), float(got[key])):
                    out.append(f"{path}/{key}: {got[key]!r}（期待は {value!r}）")
                continue
            # スタイルの名前は、大文字と小文字を区別しない（AutoCAD と同じ）
            if key == "style" and isinstance(value, str) and isinstance(got[key], str):
                if value.lower() != got[key].lower():
                    out.append(f"{path}/{key}: {got[key]!r}（期待は {value!r}）")
                continue
            out += differences(value, got[key], f"{path}/{key}", both)
    elif isinstance(want, list):
        if not isinstance(got, list) or len(want) != len(got):
            return [f"{path}: {got!r}（期待は {want!r}）"]
        for i, (a, b) in enumerate(zip(want, got)):
            out += differences(a, b, f"{path}/{i}", both)
    elif isinstance(want, bool) or isinstance(want, str):
        if want != got:
            out.append(f"{path}: {got!r}（期待は {want!r}）")
    elif isinstance(want, (int, float)):
        if not isinstance(got, (int, float)) or not close(float(want), float(got)):
            out.append(f"{path}: {got!r}（期待は {want!r}）")
    return out


def diagnostics(entry, reading):
    """zumen が開いたときと読んだときの診断"""
    return reading["diagnostics"] + entry.get("open", [])


def lifted_away(entry, reading):
    """zumen の診断のうち、図形を移さなかったことを言うもの（LIFTED_AWAY）"""
    return [d for d in diagnostics(entry, reading) if d["code"] in LIFTED_AWAY]


def lifted_away_records(entry, reading):
    """移さなかった図形の (kind, レコードの番号) の集まり。kind を持たない code は、kind を None にする"""
    return {(d.get("params", {}).get("kind"), p.get("record")) for d in lifted_away(entry, reading) for p in d.get("at", [])}


def told(away, kind, record):
    """種類 kind・レコード record の図形を移さなかったことを、zumen が言ったか。種類のない診断は、レコードだけで結ぶ"""
    return (kind, record) in away or (None, record) in away


def dropped_codes():
    """catalog/codes.toml で fidelity が dropped の、図面の形式（dxf・dwg・jww・sxf）の使っている code"""
    codes = tomllib.loads(CATALOG.read_text(encoding="utf-8"))["codes"]
    return {
        code
        for code, c in codes.items()
        if c.get("fidelity") == "dropped" and c.get("status") == "active" and code.split(".")[0] in {"dxf", "dwg", "jww", "sxf"}
    }


def handle_of(element):
    """DXF・DWG の要素の出所のハンドル（大文字）"""
    handle = element.get("source", {}).get("handle")
    return handle.upper() if handle else None


def source_type_error(element, actual, kinds, shapes):
    """要素の種類と、あれば形の両方を出所の種類と照らす。"""
    kind = element.get("kind")
    shape = element.get("geometry", {}).get("shape")
    if kind not in kinds or (shape is not None and shape not in shapes):
        return f"{element['id']}: 要素の種類 {kind}・形 {shape} の対応がない（KINDS・TYPES に足す）"
    if actual not in kinds[kind] or (shape is not None and actual not in shapes[shape]):
        return f"{element['id']}: 要素の種類 {kind}・形 {shape} の出所が {actual}"
    return None


def point_style(edit):
    """DXF・DWG: 点の表し方の変更（point_style）が決める $PDMODE と $PDSIZE（DXF Reference の HEADER）"""
    mode = MARKS[edit["mark"]] + (32 if edit.get("circle") else 0) + (64 if edit.get("square") else 0)
    size = edit["size"] if edit["size_kind"] == "length" else -edit["size"]
    return mode, size


def without_derived(view, want, got):
    """外部の実装が自分で決め直す値（view の derived が、外部の実装の形 got から返すキー）を、比べる 2 つの形から除く。
    除いた 2 つの形と、除いたキーの数を返す"""
    derive = view.get("derived")
    keys = derive(got) if derive else set()
    if not keys:
        return want, got, 0
    return {k: v for k, v in want.items() if k not in keys}, {k: v for k, v in got.items() if k not in keys}, len(keys)


def check_handles(entry, reading, counts, view):
    """DXF・DWG: 要素を出所のハンドルで外部の実装の図形と結び、画層の名前・図形の種類と画層と形と、書き戻した値を
    照らす。食い違いの文の一覧と、結んだハンドルの集まりを返す。

    view は外部の実装の読み。tool（名前）、layers（画層の名前の集まり）、layer_colors（小文字の画層の名前 → ACI）、
    entity（ハンドル → {type, layer, color, geometry} か None。geometry は図面モデルの形に直した値で、比べない種類は
    None）、header（$PDMODE と $PDSIZE）。次は、外部の実装が読めるときだけ持つ。creates_layers（真なら、図形が指す
    だけの画層も外部の実装が作るので、zumen が読んだ画層のすべてと比べる）、derived（外部の実装の形 → その実装が自分で
    決め直すので比べないキーの集まり）、entity の owner（図形を持つブロックの名前）と line_type（線種の名前。画層に
    従うなら None）、layer_line_types（小文字の画層の名前 → 線種の名前）、layer_states（小文字の画層の名前 →
    {visible, frozen, editable, printable}）、line_types（小文字の線種の名前 → {description, pattern}）、
    text_styles（小文字の文字スタイルの名前 → {font, big_font, height, width, oblique, backward, upside_down,
    vertical}）"""
    tool = view["tool"]
    failures = []
    defined = {l["name"] for l in reading["tables"]["layers"] if l.get("defined") or view.get("creates_layers")}
    if view["layers"] != defined:
        failures.append(f"画層の名前が違う: zumen だけ {sorted(defined - view['layers'])}、{tool} だけ {sorted(view['layers'] - defined)}")
    seen = set()
    # 記号の定義の中の要素（definitions。make.py が、定義の空間を指して読んだもの）は、種類と画層と、どの定義の中に
    # あるか（外部の実装が owner で返すとき）を比べる
    inside = {e["id"]: name for name, d in reading.get("definitions", {}).items() for e in d["entities"]}
    for element in [*reading["entities"], *(e for d in reading.get("definitions", {}).values() for e in d["entities"])]:
        handle = handle_of(element)
        if handle is None:
            continue
        seen.add(handle)
        e = view["entity"](handle)
        where = element["id"]
        if e is None:
            failures.append(f"{where}: {tool} にハンドル {handle} の図形がない")
            continue
        shape = element.get("geometry", {}).get("shape")
        if wrong := source_type_error(element, e["type"], KINDS, TYPES):
            failures.append(wrong)
            continue
        # 外部の実装に画層があれば、zumen の要素も持つ
        if e["layer"] is not None and (element.get("layer") or "").lower() != e["layer"].lower():
            failures.append(f"{where}: 画層が {element.get('layer')!r}、{tool} は {e['layer']!r}")
        # 定義の空間の ID は、make.py が DXF の BLOCK のハンドルから当てたもの。名前の違う定義を読んでいないことを確かめる
        if where in inside and e.get("owner") is not None and e["owner"].lower() != inside[where].lower():
            failures.append(f"{where}: 記号の定義 {inside[where]!r} の中の要素が、{tool} では {e['owner']!r} の中にある")
        counts["elements"] += 1
        # 用紙の要素は mm に直しているので、形はモデル空間だけで比べる
        top = where.startswith("space/0/") and "/" not in where[len("space/0/") :]
        if top and shape is None and e["type"] in COMPARABLE and e["geometry"] is not None:
            failures.append(f"{where}: {e['type']} {handle} の要素が形を持たない")
        if top and shape in TYPES and e["geometry"] is not None:
            counts["shapes"] += 1
            want, got, skipped = without_derived(view, element["geometry"], e["geometry"])
            if skipped:
                counts["derived"] += skipped
            for d in differences(want, got, both=True):
                failures.append(f"{where}（{e['type']} {handle}）{d}")
    if entry["role"] == "apply":
        failures += applied(entry, reading, counts, view)
    return failures, seen


# DXF・DWG: 画層か、要素を置いた記号の配置に従う色（62 の 256 と 0）と線種の名前
INHERITED_COLORS = {"layer": 256, "block": 0}
INHERITED_LINE_TYPES = {"layer": "ByLayer", "block": "ByBlock"}


def style_differences(want, e):
    """足した要素の色と線種（変更の color・line_type）が、外部の実装の読み e と違う所。番号の色（aci）と、画層か記号の
    配置に従う色、名前で指した線種と、従う線種だけを比べる。外部の実装の読みが線種を持たなければ、線種は比べない"""
    out = []
    color = want.get("color", {})
    aci = color.get("aci", INHERITED_COLORS.get(color.get("inherit")))
    if aci is not None and "rgb" not in color and e.get("color") != aci:
        out.append(f"の色が ACI {e.get('color')}（期待は {aci}）")
    line_type = want.get("line_type", {})
    name = line_type.get("name", INHERITED_LINE_TYPES.get(line_type.get("inherit")))
    if name is not None and "line_type" in e and (e["line_type"] or "ByLayer").lower() != name.lower():
        out.append(f"の線種が {e['line_type'] or 'ByLayer'!r}（期待は {name!r}）")
    return out


def added_layer(edit, view):
    """足した画層の線種と状態（表示・凍結・編集・印刷）が、外部の実装の読みと違う所。読みが持つ値だけを比べる"""
    out = []
    name = edit["name"].lower()
    line_type = edit.get("line_type", {}).get("name")
    got = view.get("layer_line_types", {}).get(name)
    if line_type is not None and got is not None and got.lower() != line_type.lower():
        out.append(f"足した画層 {edit['name']!r} の線種が {got!r}（期待は {line_type!r}）")
    states = view.get("layer_states", {}).get(name, {})
    for key in ("visible", "frozen", "editable", "printable"):
        if key in edit and key in states and states[key] != edit[key]:
            out.append(f"足した画層 {edit['name']!r} の {key} が {states[key]}（期待は {edit[key]}）")
    return out


def added_item(edit, items, word, tool, names=()):
    """足した表の項目（線種か文字スタイル）の値が、外部の実装の読み items と違う所。変更に書いた値だけを比べる。names は、
    フォントの指定（名前を、大文字と小文字を区別せずに比べる）のキー"""
    got = items.get(edit["name"].lower())
    if got is None:
        return [f"足した{word} {edit['name']!r} が {tool} にない"]
    want = {k: v for k, v in edit.items() if k in got and k not in names}
    out = differences(want, got)
    for key in names:
        if key in edit and edit[key]["name"].lower() != got[key].lower():
            out.append(f"/{key}: {got[key]!r}（期待は {edit[key]['name']!r}）")
    return [f"足した{word} {edit['name']!r} の {d}" for d in out]


def applied(entry, reading, counts, view):
    """DXF・DWG の書き戻した値: 変更で決めた形・画層・色・線種、足した画層の色・線種・状態、足した線種と文字スタイルの
    値、色と画層を変えた図形の色と画層、点の表し方"""
    tool = view["tool"]
    failures = []
    by_id = {e["id"]: e for e in reading["entities"]}
    # 記号の定義の中に足した要素は、定義の空間の読みにある
    for d in reading.get("definitions", {}).values():
        by_id.update((e["id"], e) for e in d["entities"])
    for step in entry["steps"]:
        for want in step.get("expect", []):
            element = by_id.get(want["id"])
            if element is None:
                failures.append(f"変更 {want['edit']} の要素 {want['id']} を、書き戻したファイルで zumen が読まない")
                continue
            e = view["entity"](handle_of(element) or "")
            if e is None or e["geometry"] is None:
                failures.append(f"変更 {want['edit']} の要素 {want['id']} を {tool} で比べられない")
                continue
            counts["expected"] += 1
            expected, got, _ = without_derived(view, want["geometry"], e["geometry"])
            for d in differences(expected, got):
                failures.append(f"変更 {want['edit']}（{want['id']}）{d}")
            if "layer" in want and (e["layer"] or "").lower() != want["layer"].lower():
                failures.append(f"変更 {want['edit']}（{want['id']}）の画層が {e['layer']!r}（期待は {want['layer']!r}）")
            failures += [f"変更 {want['edit']}（{want['id']}）{d}" for d in style_differences(want, e)]
        for edit in step["edits"]:
            aci = edit.get("color", {}).get("aci")
            if edit["kind"] == "add_layer":
                name = edit["name"].lower()
                if name not in view["layer_colors"]:
                    failures.append(f"足した画層 {edit['name']!r} が {tool} にない")
                elif aci is not None and view["layer_colors"][name] != aci:
                    failures.append(f"足した画層 {edit['name']!r} の色が ACI {view['layer_colors'][name]}（期待は {aci}）")
                failures += added_layer(edit, view)
            if edit["kind"] == "add_line_type" and "line_types" in view:
                failures += added_item(edit, view["line_types"], "線種", tool)
            if edit["kind"] == "add_text_style" and "text_styles" in view:
                failures += added_item(edit, view["text_styles"], "文字スタイル", tool, ("font", "big_font"))
            if edit["kind"] == "restyle" and aci is not None:
                e = view["entity"](handle_of(by_id.get(edit["id"], {})) or "")
                if e is None:
                    failures.append(f"色を変えた {edit['id']} が {tool} にない")
                elif e["color"] != aci:
                    failures.append(f"色を変えた {edit['id']} の色が ACI {e['color']}（期待は {aci}）")
            if edit["kind"] == "restyle" and "layer" in edit:
                e = view["entity"](handle_of(by_id.get(edit["id"], {})) or "")
                if e is None:
                    failures.append(f"画層を変えた {edit['id']} が {tool} にない")
                elif (e["layer"] or "").lower() != edit["layer"].lower():
                    failures.append(f"画層を変えた {edit['id']} の画層が {e['layer']!r}（期待は {edit['layer']!r}）")
            if edit["kind"] == "point_style":
                mode, size = point_style(edit)
                got_mode, got_size = view["header"]
                if got_mode != mode or not close(got_size, size):
                    failures.append(f"点の表し方が $PDMODE {got_mode}・$PDSIZE {got_size}（期待は {mode}・{size}）")
    return failures


def run(folder, tool, version, formats, check, known=(), only=None, extra=None):
    """キットの formats の形式のファイルを check で照らし、<tool>.json に結果を書く。食い違いがあれば 1 を返す。

    check(folder, entry, reading, baseline, counts) は、食い違いの文の一覧と、ファイルの監査の結果（入力と
    比べる値）を返す。入力の監査の結果は、その入力から書いたファイルの baseline になる。known は、既知の食い違い
    （ファイルの道筋の初め, 食い違いの文の初め, issue, 当たるファイルの数）の一覧。当たる数が変わったら失敗にする
    （直ったものと、同じ形で新たに食い違うものを知らせる）。only は、照らすファイルを選ぶ関数（manifest の項目を
    受けて真偽を返す）。選んだときは、既知の食い違いの数を確かめない。extra は、結果の JSON に足す項目"""
    manifest = json.loads((folder / "manifest.json").read_text(encoding="utf-8"))
    dropped = dropped_codes()
    # 表の code を書き誤ったら、照らす前に止める
    stray = sorted((LIFTED_AWAY | PARTLY) - dropped)
    if stray:
        print(f"compare.py の LIFTED_AWAY・PARTLY に、catalog/codes.toml で fidelity が dropped でない code がある: {stray}", file=sys.stderr)
        return 1
    unsorted = dropped - LIFTED_AWAY - PARTLY
    baselines = {}
    counts = Counter()
    report, failed, known_files, matched = [], 0, 0, Counter()
    for entry in manifest["files"]:
        # probe は apply の応答だけを残し、ファイルを持たない
        if entry["format"] not in formats or entry["role"] == "probe" or entry.get("status") not in (None, "ok"):
            continue
        if only is not None and not only(entry):
            continue
        reading_path = folder / "zumen" / (entry["path"] + ".json")
        reading = json.loads(reading_path.read_text(encoding="utf-8")) if reading_path.exists() else None
        try:
            failures, audit = check(folder, entry, reading, baselines.get(entry.get("from")), counts)
        except Exception as error:  # noqa: BLE001 1 つのファイルで止まらず、ほかのファイルも照らす
            failures, audit = [f"スクリプトが止まった: {type(error).__name__}: {error}"], None
        # 落としたことを言う code で、図形ごとか一部だけかを分けていないものは、黙って捨てない照合で扱えない
        if reading is not None:
            failures += [
                f"診断 {code} を、図形ごと移さなかったものか（compare.py の LIFTED_AWAY）、一部だけか（PARTLY）に分ける"
                for code in sorted({d["code"] for d in diagnostics(entry, reading)} & unsorted)
            ]
        if entry["role"] == "input":
            baselines[entry["path"]] = audit
        hits = [k for k in known if entry["path"].startswith(k[0]) and any(f.startswith(k[1]) for f in failures)]
        matched.update(hits)
        issues = sorted({k[2] for k in hits})
        new = [f for f in failures if not any(entry["path"].startswith(k[0]) and f.startswith(k[1]) for k in known)]
        report.append({"path": entry["path"], "role": entry["role"], "audit": audit, "failures": failures, "known": issues})
        if new:
            failed += 1
            print(f"{entry['path']}:", file=sys.stderr)
            for f in new:
                print(f"  {f}", file=sys.stderr)
        elif failures:
            known_files += 1
            print(f"{entry['path']}: 既知の食い違い（{', '.join(f'#{n}' for n in issues)}）", file=sys.stderr)
    for k in known if only is None else ():
        if matched[k] != k[3]:
            failed += 1
            print(f"既知の食い違い「{k[1]}」（#{k[2]}）が {matched[k]} のファイルに出た（期待は {k[3]}）。直ったなら KNOWN を直す", file=sys.stderr)
    out = {"tool": tool, "version": version, **(extra or {}), "counts": counts, "files": report}
    (folder / f"{tool}.json").write_text(json.dumps(out, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
    shown = "・".join(f"{label} {counts[name]}" for name, label in LABELS.items())
    print(f"{tool} {version} で {len(report)} のファイルを照らした。{shown}。食い違い {failed}、既知の食い違い {known_files}")
    return 1 if failed else 0


def kit_case(folder, path):
    manifest = json.loads((folder / "manifest.json").read_text(encoding="utf-8"))
    entry = next(item for item in manifest["files"] if item["path"] == path)
    reading = json.loads((folder / "zumen" / (path + ".json")).read_text(encoding="utf-8"))
    return entry, reading


def usage(script):
    if len(sys.argv) != 2:
        print(f"使い方: python {script} <キットのフォルダー>", file=sys.stderr)
        return None
    return Path(sys.argv[1])
