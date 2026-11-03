"""確認の束の SFC・P21 を ezsxf で読み、zumen が読んだ値と照らす（#503、決定 0254）。

使い方: python check_ezsxf.py <束のフォルダー>

ファイルごとに、次を確かめる。

- ezsxf が読める（strict にせず、警告を集める）。書き戻したファイルで、入力にない種類の警告が出ない。警告は code の
  種類で比べる（フィーチャごとの警告は、足したフィーチャの分だけ増えるため）。
- zumen の要素の出所のレコードの番号が、docs/formats/sxf.md の「判定」と「レコードの区切り」で数えた番号と同じ。
  zumen が読めないと言った文とコメント（sxf.record.invalid の範囲）は数えない（「壊れた所」）。
- zumen の要素の種類（kind）と形が、出所のフィーチャ（SFC）か実体（P21）の種類に合う。P21 の曲線の要素は、出所の
  曲線の実体（トリムした曲線は基の曲線）で決まる種類と形を持つ。
- SFC: 要素の出所のフィーチャを ezsxf が型で読み、レイヤと、線・円・点マーカーでは形の値が同じ。形の欄は両向きに
  比べ、ezsxf の読みにレイヤや形があれば、zumen の要素も持つ。用紙の要素は、ezsxf の用紙（drawing_sheet_feature）か、
  そこに置いた作図グループの中にあるか、ezsxf の読みで表示する複合曲線の曲線（ezsxf は複合曲線を置かず、ハッチングの
  境界の定義として持つ。sheet_members）。ezsxf がフィーチャを型で読まない SFC（印が `/*SXF3` など）は、種類
  だけを比べ、結果の notes に書く。ezsxf が複合曲線を読めなかったか飛ばした SFC と、ハッチングの指す複合曲線が
  ないと言った SFC は、用紙の中にあることを比べず、notes に書く。make.py が作った SFC（made-features・
  made-structure）とその書き戻しを ezsxf が型で読まなければ、失敗にする。
- P21: 要素の出所のインスタンス（複合のインスタンスを含む）があり、レイヤ（PRESENTATION_LAYER_ASSIGNMENT。2 つの
  レイヤに入れた要素は先のもの）が同じ。複合曲線の曲線の要素は、複合曲線を指す曲線の表記の実体のレイヤと比べる。
  用紙の要素は、用紙（番号がもっとも小さい DRAWING_SHEET_REVISION）の並びか、そこに置いた作図グループと表示する
  複合曲線の中にある。同じ番号の 2 つ目のインスタンスは使わない（zumen と同じ）。画像の範囲の要素は、画像の作図
  グループの中の 4 点以上の折れ線から作る（逆の向きは比べない。#558）。レイヤと作図グループの名前は、ファイルのバイトから zumen と同じく戻す
  （ezsxf は P21 の文字列のエスケープを戻さず、8 ビットのバイトを置き換える）。参照をたどって、点の位置・回転・倍率と、
  線分の端点（点か媒介変数のトリム）を比べる。端を点にできない線分は、形を比べず notes に書く。印の種類と色、寸法と
  引出線のレイヤは比べない。ezsxf が読み飛ばした文（引数の中にコメントを持つ文）を指す要素は、食い違いになる
  （#558）。
- 黙って捨てない: 置く所（用紙と部分図）のフィーチャ（SFC）と、用紙の並びの要素（P21）は、どれも zumen の要素になる
  か、移さなかったことを言う zumen の診断（compare.py の LIFTED_AWAY）がそのレコードを指す。SFC は、診断の kind が
  フィーチャの名前と同じことも確かめる。置く所は、決定 0121・0129・0130 と docs/formats/sxf.md の「P21 の組み直し」の
  置き方でたどる。作図グループは置いた所に広げ、表示しない複合曲線の曲線と、作図部品の定義の中身は数えない（zumen の
  要素の一覧にないため）。表示しない複合曲線がまとめた、曲線でないフィーチャ（文字・点マーカーなど）は数える。
- 書き戻したファイルでは、変更で決めた形・レイヤ（manifest.json の expect）と、SFC では色（既定義色の番号）が、
  ezsxf で読んだ値と同じ。
"""

import math
import re
import sys
from collections import Counter
from importlib.metadata import version
from pathlib import Path

import ezsxf

sys.path.insert(0, str(Path(__file__).resolve().parent))
from compare import diagnostics, differences, lifted_away_records, run, told, usage  # noqa: E402

# 要素の種類（schema/messages.json の entity_kind）→ 出所の SFC のフィーチャの名前（docs/formats/sxf.md の
# 「図面モデルへの変換」）
FEATURES = {
    "line": {"line_feature"},
    "circle": {"circle_feature"},
    "curve": {"arc_feature", "ellipse_feature", "ellipse_arc_feature", "spline_feature", "clothoid_feature"},
    "polyline": {"polyline_feature"},
    "formatted_text": {"text_string_feature"},
    "point": {"point_marker_feature"},
    "symbol": {"sfig_locate_feature", "externally_defined_symbol_feature"},
    "hatch": {
        "fill_area_style_colour_feature",
        "fill_area_style_hatching_feature",
        "fill_area_style_tiles_feature",
        "externally_defined_hatch_feature",
    },
    "dimension": {"linear_dim_feature", "curve_dim_feature", "angular_dim_feature", "radius_dim_feature", "diameter_dim_feature"},
    "callout": {"label_feature", "balloon_feature"},
    "underlay": {"polyline_feature"},
}
# 形 → 出所の SFC のフィーチャの名前。種類（FEATURES）より細かく比べる
SHAPES = {
    "line": {"line_feature"},
    "circle": {"circle_feature"},
    "arc": {"arc_feature"},
    "ellipse": {"ellipse_feature", "ellipse_arc_feature"},
    "polyline": {"polyline_feature"},
    "spline": {"spline_feature"},
    "point": {"point_marker_feature"},
    "formatted_text": {"text_string_feature"},
    "dimension": FEATURES["dimension"],
    "callout": FEATURES["callout"],
}
# 要素にならない、表と表題欄のフィーチャ（crates/zumen-sxf/src/drawing.rs の Lifter::run）
TABLES = {
    "layer_feature",
    "user_defined_colour_feature",
    "user_defined_font_feature",
    "width_feature",
    "text_font_feature",
    "pre_defined_colour_feature",
    "pre_defined_font_feature",
    "drawing_attribute_feature",
}
# 要素の種類 → 出所の P21 の実体の型（表記の実体は ISO 10303-46、寸法と引出線は ISO 10303-101 の draughting_callout）。
# 曲線の要素は、出所が曲線の表記の実体か、複合曲線の曲線の実体で、種類と形は曲線の実体で決まる（Step.curve）
SOURCES = {
    "formatted_text": {"ANNOTATION_TEXT_OCCURRENCE"},
    "point": {"ANNOTATION_SYMBOL_OCCURRENCE"},
    "symbol": {"ANNOTATION_SYMBOL_OCCURRENCE", "ANNOTATION_SUBFIGURE_OCCURRENCE"},
    "hatch": {"ANNOTATION_FILL_AREA_OCCURRENCE"},
    "dimension": {"DRAUGHTING_CALLOUT"},
    "callout": {"DRAUGHTING_CALLOUT"},
}
# P21 の曲線の実体 → 要素の種類と形（crates/zumen-sxf/src/p21.rs の curve。トリムした曲線は基の曲線で決まり、
# クロソイドは形を持たない）
TRIMMED = [("LINE", "line", "line"), ("CIRCLE", "curve", "arc"), ("ELLIPSE", "curve", "ellipse"), ("CLOTHOID", "curve", None)]
UNTRIMMED = [("POLYLINE", "polyline", "polyline"), ("CIRCLE", "circle", "circle"), ("ELLIPSE", "curve", "ellipse"), ("BEZIER_CURVE", "curve", "spline")]
# 複合のインスタンスの型にしない部分（どの表記の実体も持つ上位の型）
GENERIC = {"ANNOTATION_OCCURRENCE", "DRAUGHTING_ANNOTATION_OCCURRENCE"}
# P21 の複合図形の名前の接頭語と区分（docs/formats/sxf.md の「P21 の組み直し」。接頭語がなければ作図グループ）
SUBFIGURES = [("$$SXF_FM_", 1), ("$$SXF_FG_", 2), ("$$SXF_G_", 3), ("$$SXF_P_", 4)]
# SFC のフィーチャの印と閉じ（docs/formats/sxf.md の「レコードの区切り」。長いものから比べる）
MARKS = [("/*SXF3.1", "SXF3.1*/"), ("/*SXF3", "SXF3*/"), ("/*SXF", "SXF*/")]
# 複合曲線がまとめて曲線にするフィーチャ（決定 0129 の「背景」の曲線）。表示しない複合曲線も、ほかのフィーチャは
# まとめても曲線にしないので、黙って捨てない照合で数える
CURVES = {
    "line_feature",
    "polyline_feature",
    "circle_feature",
    "arc_feature",
    "ellipse_feature",
    "ellipse_arc_feature",
    "spline_feature",
    "clothoid_feature",
}
# ezsxf が複合曲線を読めなかったか、ハッチングの指す複合曲線がなかったときの警告。ezsxf の用紙の中のフィーチャの
# 一覧が欠ける。SKIPPED（フィーチャを飛ばした警告）は、文に複合曲線のフィーチャの名前があるときだけ（ezsxf 0.3.7 の
# 警告は、フィーチャの番号と名前を文にだけ書く）
BROKEN_SHEET = {"sfc-composite-curve-component", "sfc-hatch-reference"}
SKIPPED = {"sfc-incomplete-feature-skipped", "sfc-invalid-feature-skipped"}
# make.py が、ezsxf がフィーチャを型で読むように作った SFC
MADE = {"input/made-features.sfc", "input/made-structure.sfc"}
# 既知の食い違い: (ファイルの道筋の初め, 食い違いの文の初め, issue, 当たるファイルの数)。直ったら消す
KNOWN = []


def string_end(text, i):
    """text[i] の ' で始まる ISO 10303-21 の文字列の、閉じの ' の位置。'' は ' を 1 つ書いたもの。閉じがなければ text の長さ"""
    i += 1
    while i < len(text) and (text[i] != "'" or text.startswith("''", i)):
        i += 2 if text.startswith("''", i) else 1
    return i


def records(text, skipped, statements=None):
    """インスタンスの番号 → zumen のレコードの番号。docs/formats/sxf.md の「レコードの区切り」のとおり、`;` で終わる文と、
    データの節の `/*SXF` から閉じまでを、ファイルの初めから 0 で数える。間のコメントは数えない。FILE_DESCRIPTION の最初の
    文字列に AP202 とあれば（大文字と小文字を問わない）、データの節のフィーチャの印も、ただのコメントとする（「判定」）。
    skipped は、zumen が読めないと言った範囲 (始め, 終わり) の一覧で、そこから始まる文とコメントは数えず、範囲の終わりから
    読む（「壊れた所」）。P21 の同じ番号の 2 つ目の文もレコードとして数え、インスタンスの番号には 1 つ目の文のレコードの
    番号を使う（zumen は 2 つ目を使わない）。statements を渡せば、P21 のインスタンスの番号 → 1 つ目の文を入れる"""
    found, n, i, data, description = {}, 0, 0, False, ""
    end = len(text)

    def stop(start):
        return next((last for first, last in skipped if first <= start < last), None)

    while i < end:
        if text[i].isspace():
            i += 1
            continue
        start = i
        if text.startswith("/*", i):
            marked = data and "AP202" not in description.upper()
            mark = next((m for m in MARKS if marked and text.startswith(m[0], i)), None)
            opening, closing = mark or ("/*", "*/")
            close = text.find(closing, i + len(opening))
            i = end if close < 0 else close + len(closing)
            if mark:
                if stop(start) is not None:
                    i = stop(start)
                    continue
                m = re.match(r"\s*#(\d+)", text[start + len(mark[0]) : i])
                if m:
                    found[int(m.group(1))] = n
                n += 1
            continue
        while i < end and text[i] != ";":
            if text[i] == "'":
                # 文字列の中の ; は区切りでない
                i = string_end(text, i)
            elif text.startswith("/*", i):
                close = text.find("*/", i + 2)
                i = end if close < 0 else close + 2
                continue
            i += 1
        statement = text[start:i].strip()
        i += 1
        if stop(start) is not None:
            i = stop(start)
            continue
        m = re.match(r"#(\d+)\s*=", statement)
        if m:
            found.setdefault(int(m.group(1)), n)
            if statements is not None:
                statements.setdefault(int(m.group(1)), statement)
        elif re.match(r"DATA\b", statement):
            data = True
        elif re.match(r"ENDSEC\b", statement):
            data = False
        elif re.match(r"FILE_DESCRIPTION\b", statement):
            first = re.search(r"'((?:[^']|'')*)'", statement)
            description = first.group(1) if first else ""
        elif re.match(r"END-ISO-10303-21\b", statement):
            # 後のバイトは読まない
            break
        n += 1
    return found


def first_text(statement, keyword):
    """P21 の文（ファイルのバイトを latin-1 で読んだもの）の、型 keyword の部分の最初の引数の文字列を、zumen と同じく戻す
    （p21_text）。最初の引数が型付きの値なら、その最初の引数（crates/zumen-sxf/src/p21.rs の text）。文字列でなければ None"""
    # 文字列とコメントの中を伏せてから、型の名前を探す
    masked, i = list(statement), 0
    while i < len(statement):
        if statement[i] == "'":
            close = string_end(statement, i)
            masked[i + 1 : close] = "x" * (close - i - 1)
            i = close + 1
        elif statement.startswith("/*", i):
            close = statement.find("*/", i + 2)
            close = len(statement) if close < 0 else close + 2
            masked[i:close] = " " * (close - i)
            i = close
        else:
            i += 1
    masked = "".join(masked)
    m = re.search(rf"(?<![A-Za-z0-9_]){keyword}\s*\(", masked)
    if m is None:
        return None
    i = m.end()
    while typed := re.match(r"\s*[A-Za-z_][A-Za-z0-9_]*\s*\(", masked[i:]):
        i += typed.end()
    quote = re.match(r"\s*'", masked[i:])
    if quote is None:
        return None
    start = i + quote.end()
    return p21_text(statement[start : string_end(statement, start - 1)])


HEX = frozenset(b"0123456789ABCDEFabcdef")


def shift_jis(data):
    """Shift_JIS のバイトを、zumen（encoding_rs の Shift_JIS。WHATWG の Encoding Standard）と同じく読む。読める文字は
    cp932 と同じ。読めない 2 バイトは 1 つの置き換えにし、2 バイト目が ASCII なら、それを読み直す"""
    out, i = [], 0
    while i < len(data):
        b = data[i]
        if b <= 0x80:
            out.append(chr(b))
            i += 1
        elif 0xA1 <= b <= 0xDF:
            out.append(chr(0xFF61 + b - 0xA1))
            i += 1
        elif (0x81 <= b <= 0x9F or 0xE0 <= b <= 0xFC) and i + 1 < len(data):
            trail, char = data[i + 1], None
            if 0x40 <= trail <= 0x7E or 0x80 <= trail <= 0xFC:
                try:
                    char = data[i : i + 2].decode("cp932")
                except UnicodeDecodeError:
                    char = None
            out.append(char or "�")
            i += 2 if char or trail >= 0x80 else 1
        else:
            out.append("�")
            i += 1
    return "".join(out)


def p21_text(raw):
    """P21 の文字列の中身（ファイルのバイトを latin-1 で読んだもの）を、zumen と同じく戻す。ISO 10303-21 のエスケープ
    （''・\\X2\\…\\X0\\・\\X4\\…\\X0\\・\\X\\hh・\\S\\c・\\P?\\・\\\\）を戻し、規格の外の 8 ビットのバイトは Shift_JIS として
    読む（docs/formats/sxf.md。crates/zumen-step/src/text.rs と crates/zumen-sxf/src/p21.rs の decode）"""
    data, out, i = raw.encode("latin-1"), [], 0

    def run(opening, width):
        # opening で始まり \X0\ で閉じる、width 桁ずつの 16 進の値と、閉じの後の位置
        if not data.startswith(opening, i):
            return None
        start = end = i + len(opening)
        while end < len(data) and data[end] in HEX:
            end += 1
        if (end - start) % width or not data.startswith(b"\\X0\\", end):
            return None
        return [int(data[k : k + width], 16) for k in range(start, end, width)], end + 4

    while i < len(data):
        if data.startswith(b"''", i):
            out.append("'")
            i += 2
            continue
        if data[i] == 0x5C:
            if found := run(b"\\X2\\", 4):
                # 16 ビットの単位。サロゲートの対は最後に組む
                out += map(chr, found[0])
                i = found[1]
                continue
            if found := run(b"\\X4\\", 8):
                out += (chr(v) if v <= 0x10FFFF and not 0xD800 <= v <= 0xDFFF else "�" for v in found[0])
                i = found[1]
                continue
            if data.startswith(b"\\X\\", i) and len(data) >= i + 5 and all(b in HEX for b in data[i + 3 : i + 5]):
                out.append(chr(int(data[i + 3 : i + 5], 16)))
                i += 5
                continue
            if data.startswith(b"\\S\\", i) and len(data) > i + 3:
                out.append(chr((data[i + 3] + 128) & 0xFF))
                i += 4
                continue
            if re.fullmatch(rb"\\P[A-I]\\", data[i : i + 4]):
                i += 4
                continue
            if data.startswith(b"\\\\", i):
                out.append("\\")
                i += 2
                continue
        if data[i] >= 0x80:
            # 2 バイトの文字の 2 バイト目は 0x40〜0x7E のこともあるので、1 バイト目の次も含める
            end = i
            while end < len(data) and data[end] >= 0x80:
                end += 2 if 0x81 <= data[end] <= 0x9F or 0xE0 <= data[end] <= 0xFC else 1
            out.append(shift_jis(data[i:end]))
            i = min(end, len(data))
            continue
        out.append(chr(data[i]))
        i += 1
    # 対にならないサロゲートは、読めない文字にする（zumen の String::from_utf16_lossy と同じ）
    return "".join(out).encode("utf-16-le", "surrogatepass").decode("utf-16-le", "replace")


def skipped_ranges(entry, reading):
    """zumen が読めないと言った文とコメントのバイトの範囲（sxf.record.invalid）"""
    return sorted(
        (p["bytes"]["start"], p["bytes"]["end"])
        for d in diagnostics(entry, reading)
        if d["code"] == "sxf.record.invalid"
        for p in d.get("at", [])
        if "bytes" in p
    )


def xy(p):
    return [p["x"], p["y"]]


def feature_geometry(f):
    """ezsxf の SFC のフィーチャを、図面モデルの形の値にする。比べない種類は None"""
    if f["kind"] == "line":
        return {"shape": "line", "start": xy(f["start"]), "end": xy(f["end"])}
    if f["kind"] == "circle":
        return {"shape": "circle", "center": xy(f["center"]), "radius": f["radius"]}
    if f["kind"] == "point_marker":
        return {
            "shape": "point",
            "position": xy(f["position"]),
            "rotation": f["rotation_angle_deg"],
            "marker": {"namespace": "sxf.marker", "code": str(f["marker_code"]), "scale": f["scale"]},
        }
    return None


def instance_of(element):
    instance = element.get("source", {}).get("instance")
    return None if instance is None else int(instance)


def check_records(reading, at):
    """zumen の要素の出所のレコードの番号が、数えた番号と同じか"""
    failures = []
    for element in reading["entities"]:
        instance, record = instance_of(element), element.get("source", {}).get("record")
        if instance is not None and record is not None and at.get(instance) != record:
            failures.append(f"{element['id']}: #{instance} のレコードの番号が {record}、数えると {at.get(instance)}")
    return failures


def text_arg(params, i):
    return str(params[i]).strip() if i < len(params) else None


def int_arg(params, i):
    try:
        return int(str(params[i]).strip())
    except (IndexError, ValueError):
        return None


def visible(params):
    """複合曲線の表示の有無（引数の 3 番目）。ないものは表示し、0・1 でなければ読めないので置かない（決定 0129）"""
    if len(params) < 4:
        return True
    return int_arg(params, 3) == 1


def sfc_placed(raw):
    """zumen が要素にするか、移さなかったことを言うはずの SFC のフィーチャ（インスタンスの番号, 名前）の一覧。

    決定 0121・0129・0130 の置き方（crates/zumen-sxf/src/drawing.rs の Lifter::run と members）でたどる。前の定義の
    後に書いたフィーチャは、次の複合図形の定義（sfig_org）か用紙のものになる。次の複合曲線（composite_curve_org）は、
    その前のフィーチャを、曲線でないものもまとめる（zumen の Lifter::run も同じ）。用紙と部分図（区分 1・2）のものを数え、
    作図グループ（区分 3）は置いた所に広げ、作図部品（区分 4）は配置だけを数える。表示しない複合曲線の曲線（CURVES）と、
    作図部品の定義の中身は、zumen の要素の一覧にないので数えない。表示しない複合曲線の、曲線でないフィーチャは数える
    （zumen は、診断なしで落とす。#549）"""
    keyword = {i: k for i, k, _ in raw}
    params = {i: p for i, _, p in raw}
    pending, sheet, definitions, composites = [], [], {}, {}
    for i, k, p in raw:
        if k in TABLES:
            continue
        if k == "drawing_sheet_feature":
            sheet += pending
            pending = []
        elif k == "sfig_org_feature":
            # 名前が同じ 2 つ目の定義と、区分の読めない定義は使わない。どちらも、前の定義の後のフィーチャを持っていく
            name, flag = text_arg(p, 0), int_arg(p, 1)
            if name and flag in (1, 2, 3, 4) and name not in definitions:
                definitions[name] = (flag, pending)
            pending = []
        elif k == "composite_curve_org_feature":
            # 前の複合曲線は、曲線にしない
            composites[i] = [m for m in pending if m not in composites]
            pending = [m for m in pending if m in composites] + [i]
        else:
            pending.append(i)
    sheet += pending
    out = []

    def place(members, stack):
        for i in members:
            k, p = keyword[i], params[i]
            if k == "composite_curve_org_feature":
                out.extend((m, keyword[m]) for m in composites[i] if visible(p) or keyword[m] not in CURVES)
            elif k == "sfig_locate_feature":
                name = text_arg(p, 1)
                flag, inner = definitions.get(name, (None, []))
                if flag == 3 and name not in stack:
                    place(inner, stack + [name])
                elif flag not in (1, 2):
                    # 作図部品の配置と、定義のない配置・輪になる配置（診断）
                    out.append((i, k))
            else:
                out.append((i, k))

    place(sheet, [])
    for name, (flag, members) in definitions.items():
        if flag in (1, 2):
            place(members, [name])
    return list(dict.fromkeys(out))


def sheet_members(model, typed):
    """ezsxf の用紙の中のフィーチャ。用紙に置いた作図グループ（区分 3）の中と、表示する複合曲線の曲線を含む。
    ezsxf は複合曲線を用紙にも作図グループにも置かず、ハッチングの境界の定義として持つ（0.3.7）。そのため、ezsxf の
    読みで表示する（visibility_flag が 1）複合曲線の曲線は、置いた所を問わずに用紙の中に数える"""
    definitions = {d["entity_id"]: d for d in model.get("sfig_definitions", [])}
    placed = {r["placement_id"]: r["definition_id"] for r in model.get("sfig_references", [])}
    out, todo = set(), list((model.get("sheet") or {}).get("component_ids", []))
    while todo:
        i = todo.pop()
        if i in out:
            continue
        out.add(i)
        d = definitions.get(placed.get(i))
        if d is not None and d.get("kind_flag") == 3:
            todo += d.get("component_ids", [])
    for c in model.get("composite_curve_definitions", []):
        if typed.get(c["entity_id"], {}).get("visibility_flag") == 1:
            out.update(c.get("component_ids", []))
    return out


def unlifted(placed, seen, away, at, what, kinds=True):
    """置く所の図形のうち、要素にならず、移さなかったことを言う診断がレコードを指さないもの。kinds なら、診断の kind
    が図形の名前と同じことも確かめ、そうでなければレコードだけで結ぶ"""
    if not kinds:
        away = {(None, record) for _, record in away}
    dropped = Counter(k for i, k in placed if i not in seen and not told(away, k if kinds else None, at.get(i)))
    return [f"要素にならず、移さなかったことを言う診断もない{what}: {dict(dropped)}"] if dropped else []


def sfc_kind(where, element, keyword):
    """要素の種類と形が出所のフィーチャの名前に合わなければ、その食い違い"""
    kind = element.get("kind")
    shape = element.get("geometry", {}).get("shape")
    if kind not in FEATURES or (shape is not None and shape not in SHAPES):
        return f"{where}: 要素の種類 {kind}・形 {shape} とフィーチャの名前の対応がない（check_ezsxf.py の FEATURES・SHAPES に足す）"
    if keyword not in FEATURES[kind] or (shape is not None and keyword not in SHAPES[shape]):
        return f"{where}: 要素の種類 {kind}・形 {shape} の出所が {keyword}"
    return None


def p21_kind(where, element, step, instance, curve, rasters):
    """要素の種類と形が出所の実体に合わなければ、その食い違い。curve は、曲線の要素の出所の曲線の実体の番号。rasters は、
    画像の作図グループの中のインスタンスの番号"""
    kind = element.get("kind")
    shape = element.get("geometry", {}).get("shape")
    source = step.type(instance)
    if kind == "underlay":
        # 画像の範囲は、画像の作図グループの中の、4 点以上の折れ線（docs/formats/sxf.md、決定 0125）
        points = (step.part(curve, "POLYLINE") or [None, []])[1]
        if instance not in rasters or len(step.refs(points)) < 4:
            return f"{where}: 画像の範囲の出所が {source} #{instance}（画像の作図グループの中の、4 点以上の折れ線でない）"
        return None
    if curve is not None:
        found = step.curve(curve)
        if found is None:
            return f"{where}: {source} #{instance} の曲線 #{curve} を、要素の形にできない"
        if kind != found[0] or (shape is not None and shape != found[1]):
            return f"{where}: 要素の種類 {kind}・形 {shape} の出所が {step.type(curve)} #{curve}（期待は {found[0]}・{found[1]}）"
        if found[1] is not None and shape is None:
            return f"{where}: {step.type(curve)} #{curve} の要素が形を持たない"
        return None
    if kind not in SOURCES:
        return f"{where}: 要素の種類 {kind} と実体の型の対応がない（check_ezsxf.py の SOURCES に足す）"
    if source not in SOURCES[kind]:
        return f"{where}: 要素の種類 {kind} の出所が {source}"
    return None


def check_sfc(parsed, entry, reading, counts, notes, at):
    # 数えたレコードのフィーチャを、ファイルの順に並べる
    raw = sorted(
        ((e["id"], e["record"]["keyword"].lower(), e["record"]["parameters"]) for e in parsed["entities"] if e.get("body_type") == "simple" and e["id"] in at),
        key=lambda f: at[f[0]],
    )
    keywords = {i: k for i, k, _ in raw}
    typed = {f["id"]: f for f in parsed["typed_features"]}
    reads = any(i in typed for i, k, _ in raw if k not in TABLES)
    if not reads:
        notes.append("ezsxf がフィーチャを型で読まないので、要素の種類だけを比べる")
    model = parsed["model"]
    layers = {t["code"]: typed.get(t["entity_id"], {}).get("name") for t in model["code_tables"]["layers"]}
    warned = {
        w["code"]
        for w in parsed["warnings"]
        if w["code"] in BROKEN_SHEET or (w["code"] in SKIPPED and "composite_curve_org_feature" in str(w.get("message", "")))
    }
    if reads and warned:
        notes.append(f"ezsxf が複合曲線を読めない（{', '.join(sorted(warned))}）ので、用紙の中にあることを比べない")
    sheet = sheet_members(model, typed)
    failures, matched = [], {}
    for element in reading["entities"]:
        instance = instance_of(element)
        if instance is None:
            continue
        where = element["id"]
        keyword = keywords.get(instance)
        if keyword is None:
            failures.append(f"{where}: ezsxf に #{instance} のフィーチャがない")
            continue
        wrong = sfc_kind(where, element, keyword)
        if wrong:
            failures.append(wrong)
            continue
        counts["elements"] += 1
        if not reads:
            continue
        f = typed.get(instance)
        if f is None:
            failures.append(f"{where}: ezsxf が #{instance}（{keyword}）を型で読まない")
            continue
        matched[where] = f
        # ezsxf の読みにレイヤがあれば、zumen の要素も持つ
        layer = layers.get((f.get("style") or {}).get("layer_code"))
        if layer is not None and element.get("layer") != layer:
            failures.append(f"{where}: レイヤが {element.get('layer')!r}、ezsxf は {layer!r}")
        if where.startswith("sheet/") and not warned and instance not in sheet:
            failures.append(f"{where}: #{instance} が ezsxf の用紙の中にない")
        got = feature_geometry(f)
        if got is not None:
            if "geometry" not in element:
                failures.append(f"{where}: {keyword} #{instance} の要素が形を持たない")
                continue
            counts["shapes"] += 1
            for d in differences(element["geometry"], got, both=True):
                failures.append(f"{where}（{f['kind']} #{instance}）{d}")
    # 黙って捨てない。ezsxf が型で読まずに飛ばしたフィーチャ（引数の誤りなど）も、生のレコードの名前で数える
    seen = {instance_of(e) for e in reading["entities"]}
    away = {(kind.lower() if kind else None, record) for kind, record in lifted_away_records(entry, reading)}
    failures += unlifted(sfc_placed(raw), seen, away, at, "フィーチャ")
    if not reads:
        return failures, None
    out = {}
    for where, f in matched.items():
        style = f.get("style") or {}
        out[where] = (feature_geometry(f), layers.get(style.get("layer_code")), style.get("color_code"))
    return failures, out


class Step:
    """P21 のインスタンスの参照をたどる。複合のインスタンス（`#n=(A(…) B(…));`）は、部分の名前ごとに引数を持つ。
    引数の位置は、ISO 10303-42（幾何）・-46（表示）・-101（製図）の実体の定義の属性の順（下の各所に書く）。statements は、
    インスタンスの番号 → ファイルの文（records）"""

    def __init__(self, parsed, statements):
        self.statements = statements
        self.parts = {}
        for e in parsed["entities"]:
            if e.get("body_type") == "simple":
                parts = {e["record"]["keyword"]: e["record"]["parameters"]}
            elif e.get("body_type") == "complex":
                parts = {r["keyword"]: r["parameters"] for r in e["records"]}
            else:
                continue
            # 同じ番号の 2 つ目は使わない（zumen と同じ。crates/zumen-sxf/src/p21.rs）
            self.parts.setdefault(e["id"], parts)
        # zumen と同じく、インスタンスを番号の順に見る
        self.order = sorted(self.parts)
        # 端を点にできないトリムの線（shape）
        self.unpointed = set()
        # 非表示の要素（invisibility (invisible_items)、ISO 10303-46）と、複合曲線を指す最初の曲線の表記の実体
        self.hidden = {i for j in self.order for i in self.refs((self.part(j, "INVISIBILITY") or [[]])[0])}
        self.styling = {}
        for i in self.order:
            if self.has(i, "ANNOTATION_CURVE_OCCURRENCE"):
                self.styling.setdefault(self.ref(self.styled(i)[1]), i)

    def has(self, instance, keyword):
        return keyword in self.parts.get(instance, {})

    def part(self, instance, keyword):
        """インスタンスの、型 keyword の部分の引数。なければ None"""
        return self.parts.get(instance, {}).get(keyword)

    def type(self, instance):
        """インスタンスの型。複合のインスタンスは、表記の実体（…_OCCURRENCE）か DRAUGHTING_CALLOUT の部分の名前"""
        parts = self.parts.get(instance, {})
        if len(parts) == 1:
            return next(iter(parts))
        named = [k for k in parts if k.endswith("_OCCURRENCE") and k not in GENERIC]
        if named:
            return named[0]
        return "DRAUGHTING_CALLOUT" if "DRAUGHTING_CALLOUT" in parts else next(iter(parts), None)

    @staticmethod
    def ref(value):
        """参照なら、インスタンスの番号"""
        return value["value"] if isinstance(value, dict) and value.get("kind") == "reference" else None

    def get(self, value, keyword):
        """参照の先の、型 keyword の部分の引数。型が違えば None"""
        return self.part(self.ref(value), keyword)

    def refs(self, values):
        """参照の並びの、インスタンスの番号"""
        return [v["value"] for v in values or [] if isinstance(v, dict) and v.get("kind") == "reference"]

    def styled(self, instance):
        """表記の実体の (styles, item)。styled_item (name, styles, item)（ISO 10303-46）で、複合のインスタンスは
        STYLED_ITEM の部分が (styles, item) を持つ。表記の実体でなければ (None, None)"""
        args = self.part(instance, "STYLED_ITEM")
        if args is None:
            parts = self.parts.get(instance, {})
            name = next(iter(parts), "")
            args = parts[name][1:] if len(parts) == 1 and name.endswith("_OCCURRENCE") else None
        if args is None or len(args) < 2:
            return None, None
        return args[0], args[1]

    def point(self, value):
        # cartesian_point (name, coordinates)（ISO 10303-42）
        args = self.get(value, "CARTESIAN_POINT")
        return [float(c) for c in args[1]] if args else None

    def text(self, instance, keyword):
        """インスタンスの、型 keyword の部分の最初の引数の文字列。ezsxf は P21 の文字列のエスケープを戻さず、8 ビットの
        バイトを UTF-8 として読むので、ファイルの文から zumen と同じく戻す（first_text）。文字列でなければ空"""
        statement = self.statements.get(instance)
        if statement is None:
            args = self.part(instance, keyword)
            return str(args[0]) if args else ""
        return first_text(statement, keyword) or ""

    def layers(self):
        """インスタンスの番号 → レイヤの名前。presentation_layer_assignment (name, description, assigned_items)
        （ISO 10303-46）。名前が $$SXF_dummy で始まるものは、複合図形の仮のレイヤでレイヤにしない（docs/formats/sxf.md）。
        2 つのレイヤに入れた要素は、先のレイヤにする（zumen と同じ。crates/zumen-sxf/src/p21.rs）"""
        out = {}
        for i in self.order:
            args = self.part(i, "PRESENTATION_LAYER_ASSIGNMENT")
            name = self.text(i, "PRESENTATION_LAYER_ASSIGNMENT") if args else ""
            if args and not name.startswith("$$SXF_dummy"):
                for instance in self.refs(args[2]):
                    out.setdefault(instance, name)
        return out

    def sheet(self):
        """番号がもっとも小さい用紙の並び（zumen と同じ。docs/formats/sxf.md の「最初の DRAWING_SHEET_REVISION」、#550）。
        drawing_sheet_revision は representation (name, items, context_of_items) の下位の型（ISO 10303-101）"""
        first = next((i for i in self.order if self.has(i, "DRAWING_SHEET_REVISION")), None)
        return self.refs(self.part(first, "DRAWING_SHEET_REVISION")[1]) if first is not None else []

    def subfigure(self, item):
        """複合図形の配置の、定義の番号と区分と名前（区分の接頭語を除く）。mapped_item (name, mapping_source,
        mapping_target)・symbol_representation_map (mapping_origin, mapped_representation)・symbol_target (name,
        placement, x_scale, y_scale)（ISO 10303-43・-46）。どれかがなければ None"""
        mapped = self.get(item, "MAPPED_ITEM")
        if mapped is None:
            return None
        at = 1 if len(mapped) >= 3 else 0
        source = self.get(mapped[at], "SYMBOL_REPRESENTATION_MAP")
        target = self.get(mapped[at + 1], "SYMBOL_TARGET") if len(mapped) > at + 1 else None
        if source is None or target is None or self.get(target[1], "AXIS2_PLACEMENT_2D") is None:
            return None
        definition = self.ref(source[1])
        if not self.has(definition, "DRAUGHTING_SUBFIGURE_REPRESENTATION"):
            return None
        name = self.text(definition, "DRAUGHTING_SUBFIGURE_REPRESENTATION")
        prefix, flag = next(((prefix, flag) for prefix, flag in SUBFIGURES if name.startswith(prefix)), ("", 3))
        return definition, flag, name[len(prefix) :]

    def visible(self, composite):
        """複合曲線を表示するか。複合曲線を指す最初の曲線の表記の実体が非表示になければ表示し、指す表記の実体が
        なければ表示する（docs/formats/sxf.md の「P21 の組み直し」）"""
        occurrence = self.styling.get(composite)
        return occurrence is None or occurrence not in self.hidden

    def curve(self, instance):
        """曲線の実体の、要素の種類と形の名前。trimmed_curve (name, basis_curve, …)（ISO 10303-42）は基の曲線で決まる。
        曲線にできなければ None"""
        trimmed = self.part(instance, "TRIMMED_CURVE")
        if trimmed is not None:
            basis = self.ref(trimmed[1])
            return next(((kind, shape) for keyword, kind, shape in TRIMMED if self.has(basis, keyword)), None)
        return next(((kind, shape) for keyword, kind, shape in UNTRIMMED if self.has(instance, keyword)), None)

    def segments(self, composite):
        """複合曲線の曲線の実体の番号。composite_curve (name, segments, self_intersect)・composite_curve_segment
        (transition, same_sense, parent_curve)（ISO 10303-42）"""
        args = self.part(composite, "COMPOSITE_CURVE")
        out = []
        for segment in self.refs(args[1]) if args else []:
            parent = self.ref((self.part(segment, "COMPOSITE_CURVE_SEGMENT") or [None, None, None])[2])
            if parent is not None:
                out.append(parent)
        return out

    def shape(self, instance):
        """形を比べる要素の、図面モデルの形の値。点（点マーカーの記号）と線分だけで、ほかは None"""
        _, item = self.styled(instance)
        if self.has(instance, "ANNOTATION_SYMBOL_OCCURRENCE"):
            # defined_symbol (name, definition, target)・symbol_target (name, placement, x_scale, y_scale)（ISO 10303-46）
            symbol = self.get(item, "DEFINED_SYMBOL")
            if symbol is None or self.get(symbol[1], "PRE_DEFINED_POINT_MARKER_SYMBOL") is None:
                return None
            target = self.get(symbol[2], "SYMBOL_TARGET")
            # axis2_placement_2d (name, location, ref_direction)・direction (name, direction_ratios)（ISO 10303-42）
            placement = target and self.get(target[1], "AXIS2_PLACEMENT_2D")
            if not placement:
                return None
            direction = self.get(placement[2], "DIRECTION")
            dx, dy = (float(c) for c in direction[1]) if direction else (1.0, 0.0)
            return {
                "shape": "point",
                "position": self.point(placement[1]),
                "rotation": math.degrees(math.atan2(dy, dx)),
                "marker": {"scale": float(target[2])},
            }
        # 曲線の表記の実体か、複合曲線の曲線の実体
        curve = self.get(item, "TRIMMED_CURVE") if self.has(instance, "ANNOTATION_CURVE_OCCURRENCE") else self.part(instance, "TRIMMED_CURVE")
        # trimmed_curve (name, basis_curve, trim_1, trim_2, sense_agreement, master_representation)（ISO 10303-42）
        line = curve and self.get(curve[1], "LINE")
        if not line:
            return None
        start, end = (self.trimmed(line, trim) for trim in curve[2:4])
        # 端を点にできないトリムは、形を比べず、notes に書く
        if start is None or end is None:
            self.unpointed.add(instance)
            return None
        return {"shape": "line", "start": start, "end": end}

    def trimmed(self, line, trim):
        """線のトリムの端の点。trimming_select は点か媒介変数（PARAMETER_VALUE）で、点があれば点を使う。媒介変数は、
        line (name, pnt, dir)・vector (name, orientation, magnitude) の pnt + 媒介変数 × dir（ISO 10303-42）"""
        for value in trim or []:
            point = self.point(value)
            if point is not None:
                return point
        for value in trim or []:
            if isinstance(value, dict) and value.get("kind") == "typed" and value.get("keyword") == "PARAMETER_VALUE":
                origin = self.point(line[1])
                vector = self.get(line[2], "VECTOR")
                direction = vector and self.get(vector[1], "DIRECTION")
                if origin is None or not direction:
                    return None
                dx, dy = (float(c) for c in direction[1][:2])
                length = math.hypot(dx, dy)
                if length == 0:
                    return None
                t = float(value["parameters"][0]) * float(vector[2]) / length
                return [origin[0] + t * dx, origin[1] + t * dy]
        return None


def p21_placed(step):
    """用紙の並びの要素で、zumen が要素にするか、移さなかったことを言うはずのもの（インスタンスの番号, 型）の一覧と、
    用紙に置いたインスタンスの番号（表示する複合曲線の曲線を含む）と、複合曲線の曲線の実体 → 複合曲線の番号と、
    画像の作図グループの中のインスタンスの番号。

    docs/formats/sxf.md の「P21 の組み直し」（crates/zumen-sxf/src/p21.rs の item）の置き方でたどる。作図グループ
    （区分 3）は置いた所に広げ、部分図（区分 1・2）の中は部分図の要素として数え、作図部品（区分 4）は配置だけを数える。
    複合曲線は、表示するものの曲線を数える。見え方を持たない要素（用紙の枠など）は数えない"""
    placed, sheet, curves, rasters, done = [], set(), {}, set(), set()

    def composite(c, on_sheet):
        members = step.segments(c)
        for curve in members:
            curves.setdefault(curve, c)
        visible = step.visible(c)
        if on_sheet and visible:
            sheet.update(members)
        if c in done:
            return
        done.add(c)
        if visible:
            placed.extend((curve, step.type(curve)) for curve in members)

    def items(ids, on_sheet, stack):
        for i in ids:
            if i not in step.parts:
                continue
            if on_sheet:
                sheet.add(i)
            styles, item = step.styled(i)
            target = step.ref(item)
            if step.has(i, "ANNOTATION_SUBFIGURE_OCCURRENCE"):
                found = step.subfigure(item)
                if found is None or found[1] == 4 or found[0] in stack:
                    # 作図部品の配置と、読めない配置・輪になる配置（診断）
                    placed.append((i, step.type(i)))
                elif found[0] not in done:
                    definition, flag, name = found
                    if flag in (1, 2):
                        done.add(definition)
                    members = step.part(definition, "DRAUGHTING_SUBFIGURE_REPRESENTATION")[1]
                    if flag == 3 and raster(name):
                        rasters.update(step.refs(members))
                    items(step.refs(members), on_sheet and flag == 3, stack + [definition])
            elif step.has(i, "ANNOTATION_FILL_AREA_OCCURRENCE"):
                placed.append((i, step.type(i)))
                # annotation_fill_area (name, boundaries)（ISO 10303-46）。外形と中抜きの複合曲線を置く
                area = step.get(item, "ANNOTATION_FILL_AREA")
                for boundary in step.refs(area[1]) if area else []:
                    if step.has(boundary, "COMPOSITE_CURVE"):
                        composite(boundary, on_sheet)
            elif step.has(i, "DRAUGHTING_CALLOUT"):
                placed.append((i, step.type(i)))
            elif target is not None and step.has(target, "DRAUGHTING_CALLOUT"):
                placed.append((target, step.type(target)))
                if on_sheet:
                    sheet.add(target)
            elif step.has(i, "ANNOTATION_CURVE_OCCURRENCE") and step.has(target, "COMPOSITE_CURVE"):
                composite(target, on_sheet)
            elif styles is not None:
                placed.append((i, step.type(i)))

    items(step.sheet(), True, [])
    return list(dict.fromkeys(placed)), sheet, curves, rasters


def raster(name):
    """画像の作図グループの名前なら、画像のファイルの名前。なければ空。属性の `$$ATRU$$番号$$名称$$画像$$` か、名前の
    `$$RASTER$$` の後をファイルの名前とする（docs/formats/sxf.md、決定 0125。zumen の raster_path と同じく、ATRU が
    あれば RASTER を見ない）"""
    parts = name.split("$$")
    if "ATRU" in parts:
        at = parts.index("ATRU")
        return "$$".join(parts[at + 4 :]) if parts[at + 3 : at + 4] == ["画像"] else ""
    return "$$".join(parts[parts.index("RASTER") + 1 :]) if "RASTER" in parts else ""


def check_p21(parsed, entry, reading, counts, notes, at, statements):
    step = Step(parsed, statements)
    layers = step.layers()
    placed, sheet, curves, rasters = p21_placed(step)
    failures, matched = [], {}
    for element in reading["entities"]:
        instance = instance_of(element)
        if instance is None:
            continue
        where = element["id"]
        if instance not in step.parts:
            failures.append(f"{where}: ezsxf に #{instance} のインスタンスがない")
            continue
        source = step.type(instance)
        # 曲線の要素は、出所が曲線の表記の実体か、複合曲線の曲線の実体
        if instance in curves:
            curve = instance
        elif source == "ANNOTATION_CURVE_OCCURRENCE":
            curve = step.ref(step.styled(instance)[1])
        else:
            curve = None
        wrong = p21_kind(where, element, step, instance, curve, rasters)
        if wrong:
            failures.append(wrong)
            continue
        counts["elements"] += 1
        # 複合曲線の曲線の要素のレイヤは、複合曲線を指す曲線の表記の実体のもの（docs/formats/sxf.md の「P21 の組み直し」）
        layer = layers.get(step.styling.get(curves[instance]) if instance in curves else instance)
        if layer is not None and element.get("layer") != layer:
            failures.append(f"{where}: レイヤが {element.get('layer')!r}、PRESENTATION_LAYER_ASSIGNMENT は {layer!r}")
        if where.startswith("sheet/") and instance not in sheet:
            failures.append(f"{where}: #{instance} が用紙（DRAWING_SHEET_REVISION）の中にない")
        got = step.shape(instance)
        matched[where] = (got, layer, None)
        if got is not None:
            if "geometry" not in element:
                failures.append(f"{where}: {source} #{instance} の要素が形を持たない")
                continue
            counts["shapes"] += 1
            want = dict(element["geometry"])
            # 印の種類（PRE_DEFINED_POINT_MARKER_SYMBOL の名前）は比べず、倍率だけを比べる
            if "marker" in want:
                want["marker"] = {"scale": want["marker"]["scale"]}
            for d in differences(want, got, both=True):
                failures.append(f"{where}（{source} #{instance}）{d}")
    # 黙って捨てない: 用紙の並びの要素。P21 の診断の kind は組み直したフィーチャの実体の名前で、表記の実体の型と違う
    # ことがあるので、レコードだけで結ぶ
    seen = {instance_of(e) for e in reading["entities"]}
    failures += unlifted(placed, seen, lifted_away_records(entry, reading), at, "用紙の要素", kinds=False)
    if step.unpointed:
        notes.append(f"端を点にできないトリムの線 {sorted(step.unpointed)} は、形を比べない")
    return failures, matched


def check_file(folder, entry, reading, baseline, counts):
    """1 つの SFC・P21 のファイルの食い違い。比べた数を counts に足す"""
    path = folder / entry["path"]
    parse = ezsxf.parse_sfc if entry["format"] == "sfc" else ezsxf.parse_p21
    try:
        parsed = parse(str(path), strict=False)
    except Exception as error:  # noqa: BLE001 ezsxf が読めないことそのものを結果にする
        return [f"ezsxf が読めない: {type(error).__name__}: {error}"], None
    found = sorted(Counter(w["code"] for w in parsed["warnings"]).items())
    failures, notes = [], []
    if entry["role"] == "apply" and baseline is not None:
        added = sorted({code for code, _ in found} - {code for code, _ in baseline["warnings"]})
        if added:
            failures.append(f"ezsxf の警告で、入力にない種類が出た: {added}")
    audit = {"warnings": found, "notes": notes}
    if reading is None:
        return failures, audit
    # 区切りの文字は ASCII なので、バイトをそのまま文字にして数える（Shift_JIS の 2 バイト目は ' と ; にならない）。
    # 文字の位置は、zumen の診断のバイトの位置と同じになる
    statements = {}
    at = records(path.read_bytes().decode("latin-1"), skipped_ranges(entry, reading), statements)
    failures += check_records(reading, at)
    if entry["format"] == "sfc":
        found_failures, matched = check_sfc(parsed, entry, reading, counts, notes, at)
    else:
        found_failures, matched = check_p21(parsed, entry, reading, counts, notes, at, statements)
    failures += found_failures
    # make.py の SFC は、ezsxf が型で読むように作った。読まなくなったら、SFC の形を照らせなくなる
    if matched is None and MADE & {entry["path"], entry.get("from")}:
        failures.append("ezsxf が、型で読むように作った SFC のフィーチャを型で読まない")

    # 書き戻した値
    if entry["role"] == "apply" and matched is not None:
        for step in entry["steps"]:
            for want in step.get("expect", []):
                got, layer, color = matched.get(want["id"], (None, None, None))
                if got is None:
                    failures.append(f"変更 {want['edit']} の要素 {want['id']} を ezsxf で比べられない")
                    continue
                counts["expected"] += 1
                geometry = dict(want["geometry"])
                if entry["format"] == "p21" and "marker" in geometry:
                    geometry["marker"] = {"scale": geometry["marker"]["scale"]}
                for d in differences(geometry, got):
                    failures.append(f"変更 {want['edit']}（{want['id']}）{d}")
                if "layer" in want and layer is not None and want["layer"] != layer:
                    failures.append(f"変更 {want['edit']}（{want['id']}）のレイヤが {layer!r}（期待は {want['layer']!r}）")
                # 色は既定義色の番号（SFC のフィーチャの色の欄）
                code = want.get("color", {}).get("sxf")
                if entry["format"] == "sfc" and code is not None and color != code:
                    failures.append(f"変更 {want['edit']}（{want['id']}）の色が {color}（期待は既定義色 {code}）")
    return failures, audit


def main():
    folder = usage("check_ezsxf.py")
    if folder is None:
        return 2
    return run(folder, "ezsxf", version("ezsxf"), {"sfc", "p21"}, check_file, KNOWN)


if __name__ == "__main__":
    sys.exit(main())
