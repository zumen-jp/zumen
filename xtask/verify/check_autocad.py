"""検証キットの DXF・DWG を、画面のない AutoCAD（accoreconsole.exe）で 1 つずつ開き、監査の誤りの数と、AutoCAD が
読んだ値を、zumen の読みと変更で決めた値に照らす（#503・#683）。

使い方: python check_autocad.py <キットのフォルダー> [--exe <accoreconsole.exe>] [--issue <番号>]...
        [--timeout <秒>] [--keep <フォルダー>]

キットは make.py が作る。AutoCAD のある Windows で流す。動かすのは accoreconsole.exe だけで、流すのは dump.lsp を
埋めたスクリプトだけ。図面は一時フォルダーの写しを開き、保存しない（AutoCAD は保存のたびに Windows のログイン名を
図面に書く。samples/README.md）。AutoCAD のコンソールの出力は、道筋にログイン名を含むので、一時フォルダーに置いて
終わりに消し、結果（キットの autocad.json）には、状態・数・自分たちのスクリプトの書き出しから読んだ値だけを書く。

ファイルごとに、開いた結果の状態を記録する。

- ok: 開けて、書き出しと監査（AUDIT、直さない）が終わった。
- unopened: AutoCAD が図面を開けなかった（スクリプトの初めの印が出ない）。
- timeout: --timeout の秒数（既定 300）で止めた。どこまで進んだか（stage）を残す。
- error: 開けたが、書き出しか監査が終わらなかった。どこで止まったか（stage）を残す。

状態が ok のファイルで、次を確かめる。

- 監査の誤りの数が、書き戻したファイルでは入力より増えず、変換したファイルでは 0。
- zumen が読んだ画層の名前が、AutoCAD の画層の表と同じ。AutoCAD は、図形が指すだけで表にない画層も作るので、
  zumen が読んだ画層のすべて（定義のないものも）と比べる。
- zumen の要素の出所のハンドルの図形が AutoCAD にあり、種類・画層と、モデル空間では形の値（端点・中心・半径・
  角度・位置・文字・尺度・格子・属性など）が同じ。
- 書き戻したファイルでは、変更で決めた形・画層・色・線種（manifest.json の expect。記号の定義の中に足した要素も）
  と、足した画層の色・線種・状態、足した線種の説明と列、足した文字スタイルのフォント・高さ・幅の倍率・傾き・向き、
  変えた色、点の表し方（$PDMODE・$PDSIZE）が、AutoCAD で読んだ値と同じ。

AutoCAD が読み込むときに自分で決め直す値は、比べない。比べなかった数は、結果に残す。

- DXF の、2 点の間に置く文字（TEXT の 72 が 3・5）の、両端揃えの高さと、フィットの幅の倍率。AutoCAD は、DXF を
  読み込むときに文字の幅から決め直す（ezdxf が書いた公開サンプルの text_alignments.dxf でも変わる）。DWG では、
  ファイルの値のまま読む。
- 見出しの $ACADVER が AC1012（R13）より前か、見出しのない DXF のハンドル。AutoCAD がハンドルを振り直すので、要素を
  ハンドルで結べない。このファイルは、開けるかと監査の誤りの数だけを見る（結果の handles が false）。

書き戻したファイルと変換したファイルは、状態が ok でなければ食い違いにする。入力は、AutoCAD が開けなくても記録だけに
する（わざと壊した入力がある）。#314 の実験（role が experiment）は食い違いにせず、監査を「直す」でも流して、寸法の
部品（匿名のブロック）を AutoCAD が作り直したかを記録する。

既知の食い違い（KNOWN）は、issue と当たるファイルの数と一緒に置き、失敗にしない。数が変わったら失敗にする。

監査の誤りの数は、AutoCAD の文の言語によらず、監査の出力の終わりの「2 つの整数で終わる行」から読む。読み違えて 0 に
しないよう、初めに、監査の誤りが出る自作の図面と、出ない自作の図面（CONTROL）を流し、0 より大きい数と 0 を読めなければ
止まる。

要素にならない図形を黙って捨てていないかは、ezdxf と ACadSharp の照合で確かめ、ここでは確かめない。

終了コードは、0 が食い違いなし、1 が食い違いあり、2 が流せない（引数の誤り・accoreconsole がない・対照を読めない）。
"""

import argparse
import glob
import json
import math
import os
import re
import shutil
import subprocess
import sys
import tempfile
import time
from collections import Counter
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
from compare import ATTACHMENTS, check_handles, run  # noqa: E402

ROOT = HERE.parents[1]
# 既知の食い違い: (ファイルの道筋の初め, 食い違いの文の初め, issue, 当たるファイルの数)。直ったら消す
KNOWN = [
    # 拡張データでつないだ R2007 までの段組みの MTEXT を、AutoCAD は組ごとに 1 つにまとめ、zumen は別々に読む
    ("input/ezdxf-mtext-columns-r2007.dxf", "space/0/h", 174, 1),
]
# スクリプトの進みを知らせる印。dump.lsp の関数の名前（AutoCAD は defun の名前を大文字で表示する）と重ならない文字にする
MARK = "@@zv"
# 一時フォルダーに写した図面の名前。キットの道筋（日本語のフォルダーを含む）を AutoCAD に渡さない
DRAWING = "drawing"
# 監査の結果の行。見つけた誤りの数と直した数の 2 つの整数だけを持ち、2 つ目で終わる（文は言語で変わるので、数だけを読む）
SUMMARY = re.compile(r"^\D*(\d+)\D+(\d+)$")
# 監査の数を読めることを確かめる、自作の図面（R2000 の DXF。「コード:値」の並び）。表・ブロック・辞書は、AutoCAD が
# 読み込むのに要る最小限（画層は、印刷スタイルの参照 390 が要る）。{circle} に、円の残りの値を入れる。見出しのない
# DXF では、AutoCAD が読み込むときに値を直すので、監査の誤りにならない
CONTROL = """
0:SECTION 2:HEADER 9:$ACADVER 1:AC1015 9:$HANDSEED 5:FF 0:ENDSEC
0:SECTION 2:TABLES
0:TABLE 2:VPORT 5:8 330:0 100:AcDbSymbolTable 70:0 0:ENDTAB
0:TABLE 2:LTYPE 5:5 330:0 100:AcDbSymbolTable 70:3
0:LTYPE 5:14 330:5 100:AcDbSymbolTableRecord 100:AcDbLinetypeTableRecord 2:ByBlock 70:0 3: 72:65 73:0 40:0.0
0:LTYPE 5:15 330:5 100:AcDbSymbolTableRecord 100:AcDbLinetypeTableRecord 2:ByLayer 70:0 3: 72:65 73:0 40:0.0
0:LTYPE 5:16 330:5 100:AcDbSymbolTableRecord 100:AcDbLinetypeTableRecord 2:Continuous 70:0 3: 72:65 73:0 40:0.0
0:ENDTAB
0:TABLE 2:LAYER 5:2 330:0 100:AcDbSymbolTable 70:1
0:LAYER 5:10 330:2 100:AcDbSymbolTableRecord 100:AcDbLayerTableRecord 2:0 70:0 62:7 6:Continuous 370:-3 390:F
0:ENDTAB
0:TABLE 2:STYLE 5:3 330:0 100:AcDbSymbolTable 70:1
0:STYLE 5:11 330:3 100:AcDbSymbolTableRecord 100:AcDbTextStyleTableRecord 2:Standard 70:0 40:0.0 41:1.0 50:0.0 71:0 42:2.5 3:arial.ttf 4:
0:ENDTAB
0:TABLE 2:VIEW 5:6 330:0 100:AcDbSymbolTable 70:0 0:ENDTAB
0:TABLE 2:UCS 5:7 330:0 100:AcDbSymbolTable 70:0 0:ENDTAB
0:TABLE 2:APPID 5:9 330:0 100:AcDbSymbolTable 70:1
0:APPID 5:12 330:9 100:AcDbSymbolTableRecord 100:AcDbRegAppTableRecord 2:ACAD 70:0
0:ENDTAB
0:TABLE 2:DIMSTYLE 5:A 330:0 100:AcDbSymbolTable 70:0 100:AcDbDimStyleTable 0:ENDTAB
0:TABLE 2:BLOCK_RECORD 5:1 330:0 100:AcDbSymbolTable 70:2
0:BLOCK_RECORD 5:1F 330:1 100:AcDbSymbolTableRecord 100:AcDbBlockTableRecord 2:*Model_Space
0:BLOCK_RECORD 5:1B 330:1 100:AcDbSymbolTableRecord 100:AcDbBlockTableRecord 2:*Paper_Space
0:ENDTAB
0:ENDSEC
0:SECTION 2:BLOCKS
0:BLOCK 5:20 330:1F 100:AcDbEntity 8:0 100:AcDbBlockBegin 2:*Model_Space 70:0 10:0.0 20:0.0 30:0.0 3:*Model_Space 1:
0:ENDBLK 5:21 330:1F 100:AcDbEntity 8:0 100:AcDbBlockEnd
0:BLOCK 5:1C 330:1B 100:AcDbEntity 67:1 8:0 100:AcDbBlockBegin 2:*Paper_Space 70:0 10:0.0 20:0.0 30:0.0 3:*Paper_Space 1:
0:ENDBLK 5:1D 330:1B 100:AcDbEntity 67:1 8:0 100:AcDbBlockEnd
0:ENDSEC
0:SECTION 2:ENTITIES
0:CIRCLE 5:30 330:1F 100:AcDbEntity 8:0 100:AcDbCircle 10:0.0 20:0.0 30:0.0 40:1.0 {circle}
0:ENDSEC
0:SECTION 2:OBJECTS
0:DICTIONARY 5:C 330:0 100:AcDbDictionary 281:1 3:ACAD_GROUP 350:D 3:ACAD_PLOTSTYLENAME 350:E
0:DICTIONARY 5:D 330:C 100:AcDbDictionary 281:1
0:ACDBDICTIONARYWDFLT 5:E 330:C 100:AcDbDictionary 281:1 3:Normal 350:F 100:AcDbDictionaryWithDefault 340:F
0:ACDBPLACEHOLDER 5:F 330:E
0:ENDSEC 0:EOF
"""
# 押し出しの向き（210）が零ベクトルの円。AutoCAD の監査が誤りに数える
ZERO_NORMAL = "210:0.0 220:0.0 230:0.0"
STATES = {"unopened": "図面を開けない", "timeout": "時間切れで止めた", "error": "書き出しか監査が終わらない"}


def find_exe(given):
    """accoreconsole.exe の場所。--exe、環境変数 ZUMEN_ACCORECONSOLE、既定の導入先の順に見る。実行ファイルの場所を
    探すだけで、製品のフォルダーの中身は読まない"""
    for candidate in (given, os.environ.get("ZUMEN_ACCORECONSOLE")):
        if candidate:
            return Path(candidate) if Path(candidate).is_file() else None
    root = os.environ.get("ProgramFiles", r"C:\Program Files")
    found = sorted(glob.glob(os.path.join(root, "Autodesk", "AutoCAD *", "accoreconsole.exe")))
    return Path(found[-1]) if found else None


def control(circle):
    """対照の図面の中身。circle は、円に足す値（ZERO_NORMAL なら監査の誤りが出て、空なら出ない）"""
    tokens = CONTROL.format(circle=circle).split()
    return "".join(f"{code}\n{value}\n" for code, _, value in (token.partition(":") for token in tokens)).encode("ascii")


def lisp():
    """dump.lsp の、スクリプトに埋める行（注記と空の行を除く）。スクリプトの空の行は、前のコマンドの繰り返しになる"""
    lines = [line.rstrip() for line in (HERE / "dump.lsp").read_text(encoding="utf-8").splitlines()]
    lines = [line for line in lines if line.strip() and not line.lstrip().startswith(";")]
    wrong = [line for line in lines if len(line) > 200 or not line.isascii()]
    if wrong:
        raise ValueError(f"dump.lsp の式の行は、ASCII で 200 文字までにする: {wrong[0][:40]}")
    return lines


def mark(name, *values):
    """印の行を書く式。values は、印の後ろに書く値を返す LISP の式"""
    shown = "".join(f' (princ " ") (princ {value})' for value in values)
    return f'(progn (princ "\\n{MARK} {name}"){shown} (princ "\\n") (princ))'


def script(fix):
    """1 つの図面に流すスクリプト。開いたことの印、書き出し、監査（直さない）、fix なら監査（直す）と書き出しの順。
    LISP の式が誤りで止まっても、スクリプトは次の行へ進むので、段ごとに印を置き、書き出しは終わりの行（@END）で確かめる"""
    lines = [
        mark("open", '(getvar "DWGNAME")'),
        mark("dbmod", '(getvar "DBMOD")'),
        mark("version", '(getvar "ACADVER")', '(getvar "_VERNUM")'),
        *lisp(),
        '(zv-dump (strcat (getvar "DWGPREFIX") "before.txt"))',
        mark("dumped"),
        mark("audit-begin"),
        "_.AUDIT _N",
        mark("audit-end"),
    ]
    if fix:
        lines += [mark("fix-begin"), "_.AUDIT _Y", mark("fix-end"), '(zv-dump (strcat (getvar "DWGPREFIX") "after.txt"))', mark("fixed")]
    lines += [mark("done"), "_.QUIT _Y"]
    return "".join(line + "\r\n" for line in lines)


def decode(raw):
    """accoreconsole の出力（パイプやファイルへは UTF-16LE で書く）"""
    if raw.count(b"\0") > len(raw) // 4:
        return raw.decode("utf-16-le", "replace")
    return raw.decode("mbcs" if os.name == "nt" else "utf-8", "replace")


def console_lines(text):
    """コンソールの出力の行。行の中の復帰（CR）は、進みの表示が同じ行を書き直すのに使うので、最後に書いた分だけを取る"""
    return [line.rstrip("\r").rpartition("\r")[2].strip() for line in text.split("\n")]


def markers(text):
    """印の行の (名前, 値) の並び。スクリプトの行の表示は、印の文字が行の初めにないので拾わない"""
    found = []
    for line in console_lines(text):
        if line.startswith(MARK + " "):
            name, _, value = line[len(MARK) + 1 :].partition(" ")
            found.append((name, value.strip()))
    return found


def audit_counts(text, begin, end):
    """begin と end の印の間の、監査の結果の行（SUMMARY に当たる最後の行）の (誤りの数, 直した数)。読めなければ None"""
    lines = console_lines(text)
    try:
        i = lines.index(f"{MARK} {begin}")
        j = lines.index(f"{MARK} {end}", i)
    except ValueError:
        return None
    found = [m for m in (SUMMARY.match(line) for line in lines[i + 1 : j]) if m]
    if not found:
        return None
    errors, fixed = int(found[-1][1]), int(found[-1][2])
    # 直した数は、見つけた数を超えない。超えるなら、結果の行でないものを読んでいる
    return (errors, fixed) if fixed <= errors else None


class Row:
    """書き出しの 1 行（図形か表の項目）。fields は (グループ・コード, 値の文字) の並び。文字は、dump.lsp が書いた
    DXF の書き方（制御文字は「^」と英字、「^」は「^ 」）のまま持つ。zumen の読みも、同じ書き方で文字を持つ"""

    def __init__(self, handle, kind, layer, parts, block=None):
        self.handle, self.type, self.layer, self.block = handle.upper(), kind, layer, block
        self.fields = [(int(code), value) for code, _, value in (part.partition("=") for part in parts)]
        self.attributes = []

    def values(self, code):
        return [value for c, value in self.fields if c == code]

    def text(self, code, default=None):
        found = self.values(code)
        return found[0] if found else default

    def number(self, code, default=None):
        found = self.values(code)
        return float(found[0]) if found else default

    def integer(self, code, default=0):
        found = self.values(code)
        return int(found[0]) if found else default

    def point(self, code):
        found = self.values(code)
        return [float(v) for v in found[0].split(",")[:2]] if found else None


class Dump:
    """dump.lsp の書き出し。complete は、終わりの行（@END）があり、図形の行の数が合うこと"""

    def __init__(self, text):
        self.header = (None, None)
        self.tables = {"LAYER": {}, "LTYPE": {}, "STYLE": {}}
        self.rows, self.blocks, self.by_handle = [], {}, {}
        self.complete = False
        block, owner, count = None, None, 0
        for n, line in enumerate(text.split("\n")):
            parts = line.rstrip("\r").split("\t")
            if parts == [""]:
                continue
            if n == 0:
                values = dict(part.partition("=")[::2] for part in parts)
                self.header = (int(values["PDMODE"]), float(values["PDSIZE"]))
            elif parts[0] == "@END":
                self.complete = int(parts[1]) == count
            elif parts[0] == "@BLOCK":
                block = parts[2]
                self.blocks[block] = {"handle": parts[1].upper(), "rows": []}
                owner = None
            elif parts[0].startswith("@"):
                self.tables.setdefault(parts[0][1:], {})[parts[2]] = Row(parts[1], parts[0][1:], None, parts[3:])
            else:
                row = Row(parts[0], parts[1], parts[2], parts[3:], block)
                count += 1
                (self.rows if block is None else self.blocks[block]["rows"]).append(row)
                self.by_handle.setdefault(row.handle, row)
                # 属性（ATTRIB）は、属性を持つ記号の配置（66 が 1 の INSERT）の後ろに SEQEND まで並ぶ
                if row.type == "ATTRIB" and owner is not None:
                    owner.attributes.append(row)
                else:
                    owner = row if row.type == "INSERT" and row.integer(66) == 1 else None


def load_dump(path):
    if not path.exists():
        return None
    data = path.read_bytes()
    try:
        text = data.decode("utf-8")
    except UnicodeDecodeError:
        # LISPSYS が 0 の AutoCAD は、ANSI のコード・ページで書き、書けない文字を \U+XXXX にする
        text = re.sub(r"\\U\+([0-9A-Fa-f]{4})", lambda m: chr(int(m[1], 16)), data.decode("mbcs" if os.name == "nt" else "latin-1", "replace"))
    try:
        return Dump(text)
    except (ValueError, KeyError, IndexError):
        return None


def geometry(row):
    """AutoCAD の図形の行を、図面モデルの形の値（entities の geometry）の形にする。比べない種類は None。値の意味は
    DXF Reference の同じ名前の図形と同じ。角度の 50・51 は、dump.lsp が度に直している"""
    t = row.type
    if t == "LINE":
        return {"shape": "line", "start": row.point(10), "end": row.point(11)}
    if t == "CIRCLE":
        return {"shape": "circle", "center": row.point(10), "radius": row.number(40)}
    if t == "ARC":
        return {"shape": "arc", "center": row.point(10), "radius": row.number(40), "start_angle": row.number(50), "end_angle": row.number(51)}
    if t == "ELLIPSE":
        # 11 は中心から長軸の端への向き、41・42 は媒介変数（ラジアン）。図面モデルの major は長軸の端の点
        center, axis = row.point(10), row.point(11)
        return {
            "shape": "ellipse",
            "center": center,
            "major": [center[0] + axis[0], center[1] + axis[1]],
            "ratio": row.number(40),
            "start_angle": math.degrees(row.number(41)),
            "end_angle": math.degrees(row.number(42)),
        }
    if t == "LWPOLYLINE":
        # 幅（全体の 43 と、頂点ごとの 40・41）を持つ折れ線は、ezdxf の照合と同じく比べない
        if any(float(value) != 0.0 for code, value in row.fields if code in (40, 41, 43)):
            return None
        # 頂点ごとに 10 が並び、その後ろの 42 が膨らみ
        points, bulges = [], []
        for code, value in row.fields:
            if code == 10:
                points.append([float(v) for v in value.split(",")[:2]])
                bulges.append(0.0)
            elif code == 42 and bulges:
                bulges[-1] = float(value)
        g = {"shape": "polyline", "points": points, "closed": bool(row.integer(70) & 1)}
        if any(bulges):
            g["bulges"] = bulges
        return g
    if t == "POINT":
        return {"shape": "point", "position": row.point(10), "rotation": row.number(50, 0.0)}
    if t == "TEXT":
        g = {"shape": "text", "content": row.text(1, ""), "height": row.number(40), "rotation": row.number(50, 0.0), "style": row.text(7, "Standard")}
        # 72 が 3（aligned）・5（fit）なら 10 と 11 の 2 点の間に置き、ほかで 72・73 が 0・0 でなければ 11 に寄せる
        h, v = row.integer(72), row.integer(73)
        if h in (3, 5):
            g["position"], g["end"] = row.point(10), row.point(11)
            # 2 点が重なる文字は、zumen が 2 点の間に置かない（ezdxf・ACadSharp の照合と同じく比べない）
            if g["position"] == g["end"]:
                return None
            g["fit"] = "aligned" if h == 3 else "fit"
            g["rotation"] = math.degrees(math.atan2(g["end"][1] - g["position"][1], g["end"][0] - g["position"][0]))
        else:
            g["position"] = row.point(10) if h == 0 and v == 0 else row.point(11)
            if name := {1: "center", 2: "right", 4: "middle"}.get(h):
                g["horizontal"] = name
            if name := {1: "bottom", 2: "middle", 3: "top"}.get(v):
                g["vertical"] = name
        # 既定の値（幅の倍率 1、傾き 0、向きなし）は、zumen の entities と同じく省く。71 は、2 が裏返し、4 が上下反転
        if (width_factor := row.number(41, 1.0)) != 1.0:
            g["width_factor"] = width_factor
        if (oblique := row.number(51, 0.0)) != 0.0:
            g["oblique"] = oblique
        if row.integer(71) & 2:
            g["backward"] = True
        if row.integer(71) & 4:
            g["upside_down"] = True
        return g
    if t == "MTEXT":
        # 250 文字を超える文字は、3 に分けて並び、終わりが 1。71 は寄せ方、41 は枠の幅（0 なら枠なし）
        g = {
            "shape": "formatted_text",
            "content": "".join(row.values(3)) + row.text(1, ""),
            "position": row.point(10),
            "height": row.number(40),
            "rotation": row.number(50, 0.0),
            "attachment": ATTACHMENTS[row.integer(71, 1) - 1],
            "style": row.text(7, "Standard"),
        }
        if row.number(41, 0.0) > 0:
            g["width"] = row.number(41)
        return g
    if t == "INSERT":
        # 41・42 は尺度、70・71 は列と行の数、44・45 は列と行の間隔（MINSERT）
        g = {
            "shape": "symbol",
            "name": row.text(2),
            "position": row.point(10),
            "scale": [row.number(41, 1.0), row.number(42, 1.0)],
            "rotation": row.number(50, 0.0),
        }
        columns, rows = row.integer(70) or 1, row.integer(71) or 1
        if columns > 1 or rows > 1:
            g["grid"] = {"columns": columns, "rows": rows, "column_spacing": row.number(44, 0.0), "row_spacing": row.number(45, 0.0)}
        if row.attributes:
            g["attributes"] = [{"tag": a.text(2), "value": a.text(1, "")} for a in row.attributes]
        return g
    return None


def keeps_handles(data, suffix):
    """AutoCAD が、ファイルのハンドル（5）を保って読む図面か。DWG と、見出しの $ACADVER が AC1012（R13）からの DXF。
    見出しのない DXF と、それより前の形の DXF は、AutoCAD がハンドルを振り直す（R12 までの形でハンドルを持つ DXF は
    確かめていないので、結ばない側にする）"""
    if suffix.lower() != ".dxf" or data.startswith(b"AutoCAD Binary DXF"):
        return True
    lines = data[:65536].split(b"\n")
    for i in range(0, len(lines) - 3, 2):
        code, value = lines[i].strip(), lines[i + 1].strip()
        if code == b"9" and value == b"$ACADVER":
            version = lines[i + 3].strip().decode("ascii", "replace")
            return version.startswith("AC") and version >= "AC1012"
        if code == b"0" and value == b"ENDSEC":
            break
    return False


def derived(geometry):
    """DXF を読み込むときに、AutoCAD が文字の幅から決め直す値のキー。2 点の間に置く文字（TEXT の 72 が 3・5）の、
    両端揃えの高さと、フィットの幅の倍率"""
    if geometry.get("shape") != "text":
        return set()
    return {"aligned": {"height"}, "fit": {"width_factor"}}.get(geometry.get("fit"), set())


def part_of(row):
    """寸法・表の定義のフィールド（ブロックの名前・測定値・文字や表の位置・向き・上書き文字）"""
    if row.type in ("DIMENSION", "ARC_DIMENSION"):
        return {
            "block": row.text(2, ""),
            "measurement": row.number(42),
            "text_position": row.point(11),
            "override": row.text(1, ""),
        }
    if row.type == "ACAD_TABLE":
        return {
            "block": row.text(2, ""),
            "position": row.point(10),
            "direction": row.point(11) or [1.0, 0.0],
        }
    return None


def view(dump, fmt):
    """共有の照らし方（compare.check_handles）に渡す AutoCAD の読み。fmt は、開いたファイルの形式（dxf か dwg）"""
    blocks = {name.lower(): info["rows"] for name, info in dump.blocks.items()}

    def entity(handle):
        row = dump.by_handle.get(handle)
        if row is None:
            return None
        # 62 がなければ画層に従う色（256）、6 がなければ画層に従う線種。owner は、図形を持つブロックの名前（モデル空間と
        # レイアウトの図形は None）
        out = {"type": row.type, "layer": row.layer, "color": row.integer(62, 256), "line_type": row.text(6), "owner": row.block, "geometry": geometry(row)}
        if (part := part_of(row)) is not None:
            out["part"] = part
        return out

    def block(name):
        rows = blocks.get(name.lower())
        if rows is None:
            return None
        return [{"type": r.type, "layer": r.layer, "geometry": geometry(r)} for r in rows]

    layers = dump.tables["LAYER"]
    return {
        "tool": "AutoCAD",
        "layers": set(layers),
        "creates_layers": True,
        "derived": derived if fmt == "dxf" else None,
        "layer_colors": {name.lower(): abs(row.integer(62)) for name, row in layers.items() if row.values(62)},
        "layer_line_types": {name.lower(): row.text(6) for name, row in layers.items()},
        # 画層の状態: 62 が負なら非表示、70 の 1 は凍結、4 はロック、290 が 0 なら印刷しない
        "layer_states": {
            name.lower(): {
                "visible": row.integer(62, 7) > 0 and not row.integer(70) & 1,
                "frozen": bool(row.integer(70) & 1),
                "editable": not row.integer(70) & 4,
                "printable": row.integer(290, 1) != 0,
            }
            for name, row in layers.items()
        },
        # 線種: 説明（3）と、線と間の長さの列（49）
        "line_types": {name.lower(): {"description": row.text(3, ""), "pattern": [float(v) for v in row.values(49)]} for name, row in dump.tables["LTYPE"].items()},
        # 文字スタイル: フォント（3）、大きいフォント（4）、高さ（40）、幅の倍率（41）、傾き（50）、裏返し（71 の 2）・
        # 上下反転（71 の 4）、縦書き（70 の 4）
        "text_styles": {
            name.lower(): {
                "font": row.text(3, ""),
                "big_font": row.text(4, ""),
                "height": row.number(40, 0.0),
                "width": row.number(41, 1.0),
                "oblique": row.number(50, 0.0),
                "backward": bool(row.integer(71) & 2),
                "upside_down": bool(row.integer(71) & 4),
                "vertical": bool(row.integer(70) & 4),
            }
            for name, row in dump.tables["STYLE"].items()
        },
        "entity": entity,
        "block": block,
        "header": dump.header,
    }


def dimension_parts(dump):
    """寸法（図形の 2 が、部品のブロックの名前）のハンドル → (部品の名前, 部品の図形の行。ブロックがなければ None)"""
    found = {}
    for row in dump.rows:
        if row.type.endswith("DIMENSION"):
            name = row.text(2)
            block = dump.blocks.get(name)
            found[row.handle] = (name, None if block is None else block["rows"])
    return found


def same_numbers(x, y):
    """2 つの値の文字が、数（か点）として同じか。AutoCAD が計算し直した値は、終わりの桁が揺れる"""
    try:
        xs, ys = [float(v) for v in x.split(",")], [float(v) for v in y.split(",")]
    except ValueError:
        return False
    return len(xs) == len(ys) and all(math.isclose(p, q, rel_tol=1e-9, abs_tol=1e-9) for p, q in zip(xs, ys))


def unlike(a, b):
    """2 つの図形の行の、初めの違い（種類・画層・値）。同じなら None"""
    if (a.type, a.layer) != (b.type, b.layer):
        return f"{a.type}（画層 {a.layer}）と {b.type}（画層 {b.layer}）"
    # MTEXT の 42・43 は、AutoCAD が描いたときに入れる文字の幅と高さで、描く前は 0
    skip = {42, 43} if a.type == "MTEXT" else set()
    fields_a, fields_b = ([f for f in row.fields if f[0] not in skip] for row in (a, b))
    if [code for code, _ in fields_a] != [code for code, _ in fields_b]:
        return f"{a.type} のコードの並び"
    for (code, x), (_, y) in zip(fields_a, fields_b):
        if x != y and not same_numbers(x, y):
            return f"{a.type} の {code} が {x} と {y}"
    return None


def observe(was, now):
    """#314 の実験: 読み was から読み now で、寸法の部品（匿名のブロック）がどう変わったか。変わった寸法のハンドル →
    変わり方。部品の名前が変わったか、名前は同じで図形を作り直したか（図形のハンドルが変わる）、値だけが変わったか"""
    before, after = dimension_parts(was), dimension_parts(now)
    changed = {}
    for handle, (name, rows) in sorted(after.items()):
        if handle not in before:
            changed[handle] = "前の読みにない寸法"
            continue
        old_name, old_rows = before[handle]
        if rows is None or old_rows is None:
            if name != old_name or (rows is None) != (old_rows is None):
                changed[handle] = f"部品が {old_name} から {name} に変わった（ブロックのない部品がある）"
            continue
        if len(rows) != len(old_rows):
            difference = f"図形の数が {len(old_rows)} と {len(rows)}"
        else:
            difference = next((d for d in map(unlike, old_rows, rows) if d), None)
        shape = "形は同じ" if difference is None else f"形が違う: {difference}"
        if name != old_name:
            changed[handle] = f"部品が {old_name} から {name} に変わった（{shape}）"
        elif [r.handle for r in rows] != [r.handle for r in old_rows]:
            changed[handle] = f"部品 {name} の図形を作り直した（{shape}）"
        elif difference:
            changed[handle] = f"部品 {name} の値が変わった（{difference}）"
    return {"dimensions": len(after), "blocks": len(now.blocks), "changed": changed, "missing": sorted(set(before) - set(after))}


class Runner:
    """accoreconsole で図面を 1 つずつ開く。図面は一時フォルダーの写しを開き、保存しない"""

    def __init__(self, exe, work, timeout, keep):
        self.exe, self.work, self.timeout, self.keep = exe, work, timeout, keep
        self.count = 0

    def open(self, data, suffix, fix=False):
        """図面（中身 data、拡張子 suffix）を開き、状態と読みを返す。

        返す値は、status（ok・unopened・timeout・error）、exit（終了コード。時間切れなら None）、stage（最後に出た印）、
        modified（開いた直後に、AutoCAD が図面を変えていたか。DBMOD）、version、errors・fixed（監査、直さない）、
        dump（書き出し）。fix なら、fix_errors・fix_fixed（監査、直す）と after（その後の書き出し）も"""
        self.count += 1
        work = self.work / f"{self.count:04d}"
        work.mkdir(parents=True)
        name = DRAWING + suffix.lower()
        (work / name).write_bytes(data)
        (work / "run.scr").write_text(script(fix), encoding="ascii", newline="")
        result = {"status": "ok", "exit": None}
        try:
            with open(work / "console.log", "wb") as log:
                # 道筋は絶対にする（相対の /s は、スクリプトが見つからないまま終了コード 0 で終わる）
                command = [str(self.exe), "/i", str(work / name), "/s", str(work / "run.scr")]
                try:
                    done = subprocess.run(command, cwd=work, stdin=subprocess.DEVNULL, stdout=log, stderr=subprocess.STDOUT, timeout=self.timeout)
                    result["exit"] = done.returncode
                except subprocess.TimeoutExpired:
                    result["status"] = "timeout"
            text = decode((work / "console.log").read_bytes())
            marks = markers(text)
            names = [name_ for name_, _ in marks]
            values = dict(marks)
            result["stage"] = names[-1] if names else None
            if "version" in values:
                result["version"] = values["version"]
            opened = values.get("open", "").lower() == name
            if result["status"] == "timeout":
                return result
            if not opened:
                result["status"] = "unopened"
                return result
            result["modified"] = values.get("dbmod") not in (None, "0")
            dump = load_dump(work / "before.txt")
            counts = audit_counts(text, "audit-begin", "audit-end")
            if dump is None or not dump.complete:
                return result | {"status": "error", "stage": "書き出し"}
            if counts is None:
                return result | {"status": "error", "stage": "監査"}
            result |= {"errors": counts[0], "fixed": counts[1], "dump": dump}
            if fix:
                after, fixed = load_dump(work / "after.txt"), audit_counts(text, "fix-begin", "fix-end")
                if after is None or not after.complete or fixed is None:
                    return result | {"status": "error", "stage": "直す監査"}
                result |= {"fix_errors": fixed[0], "fix_fixed": fixed[1], "after": after}
            if "done" not in names:
                return result | {"status": "error", "stage": "終わり"}
            return result
        finally:
            if not self.keep:
                shutil.rmtree(work, ignore_errors=True)


def progress(result):
    """開けなかった・終わらなかった図面の、終了コードと、最後に出た印（結果の文に書く）"""
    code = "なし" if result.get("exit") is None else result["exit"]
    return f"終了コード {code}、進んだ所 {result.get('stage') or 'なし'}"


def summary(result):
    """結果の JSON に書く、開いた結果（図形の値は書かない）"""
    out = {"status": result["status"], "exit": result["exit"]}
    if result["status"] != "ok":
        out["stage"] = result.get("stage")
        return out
    dump = result["dump"]
    out |= {"modified": result["modified"], "errors": result["errors"], "fixed": result["fixed"]}
    out |= {"entities": len(dump.by_handle), "blocks": len(dump.blocks)}
    return out


def check_file(runner, sources, entry, reading, baseline, folder):
    """1 つの図面の食い違いと、開いた結果。sources は、実験の入力の読み（道筋 → 書き出し）。実験と、その入力は、
    監査を「直す」でも流し、直す前後で寸法の部品がどう変わったか（after_fix）を残す。入力の結果が、実験の対照になる。
    実験は、開いたときの読みを入力の読みと比べた結果（opened）も残す"""
    path = folder / entry["path"]
    role = entry["role"]
    fix = role == "experiment" or entry["path"] in sources
    data = path.read_bytes()
    result = runner.open(data, path.suffix, fix=fix)
    audit = summary(result)
    audit["handles"] = keeps_handles(data, path.suffix)
    if result["status"] != "ok":
        # 入力も開けない図面（$HANDSEED のない R2000 からの DXF など）を書き戻したファイルは、開けなくても、書き戻しの
        # 食い違いにしない。開けなかったことは、結果の状態に残る
        unopened_input = role == "apply" and baseline is not None and baseline["status"] != "ok"
        failed = role in ("apply", "convert") and not unopened_input
        return ([f"AutoCAD が{STATES[result['status']]}（{progress(result)}）"] if failed else []), audit, None
    dump = result["dump"]
    if entry["path"] in sources:
        sources[entry["path"]] = dump
    failures = []
    if role == "convert" and result["errors"]:
        failures.append(f"AutoCAD の監査の誤りが {result['errors']}（期待は 0）")
    if role == "apply" and baseline is not None and baseline["status"] == "ok" and result["errors"] > baseline["errors"]:
        failures.append(f"AutoCAD の監査の誤りが {result['errors']}（入力は {baseline['errors']}）")
    if fix:
        audit["fix"] = {"errors": result["fix_errors"], "fixed": result["fix_fixed"]}
        source = sources.get(entry.get("from"))
        if role == "experiment" and source is not None:
            audit["opened"] = observe(source, dump)
        audit["after_fix"] = observe(dump, result["after"])
    return failures, audit, dump


def shown_parts(observed):
    """寸法の部品の変わり方を、1 行で"""
    changed = "・".join(f"{handle}: {how}" for handle, how in observed["changed"].items()) or "変わらない"
    missing = f"、なくなった寸法 {'・'.join(observed['missing'])}" if observed["missing"] else ""
    return f"寸法 {observed['dimensions']}・部品のブロック {observed['blocks']}、{changed}{missing}"


def main():
    parser = argparse.ArgumentParser(description="検証キットの DXF・DWG を accoreconsole で開いて照らす")
    parser.add_argument("kit", help="make.py が作ったキットのフォルダー")
    parser.add_argument("--exe", help="accoreconsole.exe の場所（既定は環境変数 ZUMEN_ACCORECONSOLE か、既定の導入先）")
    parser.add_argument("--issue", type=int, action="append", help="この issue のファイルと、その入力だけを流す（繰り返せる）")
    parser.add_argument("--timeout", type=float, default=300, help="1 つの図面に待つ秒数（既定 300）")
    parser.add_argument("--keep", help="AutoCAD の作業のフォルダーを、ここに残す（リポジトリの外）。コンソールの出力はログイン名を含む")
    args = parser.parse_args()
    folder = Path(args.kit)
    if not (folder / "manifest.json").exists():
        print(f"{folder} に manifest.json がない（make.py でキットを作る）", file=sys.stderr)
        return 2
    exe = find_exe(args.exe)
    if exe is None:
        print("accoreconsole.exe が見つからない（--exe か環境変数 ZUMEN_ACCORECONSOLE で指す）", file=sys.stderr)
        return 2
    if args.keep and Path(args.keep).resolve().is_relative_to(ROOT):
        print("AutoCAD の作業のフォルダーは、リポジトリの外に置く", file=sys.stderr)
        return 2
    work = Path(args.keep).resolve() if args.keep else Path(tempfile.mkdtemp(prefix="zumen-autocad-"))
    if args.keep and work.exists() and any(work.iterdir()):
        print(f"{work} は空でないので使わない", file=sys.stderr)
        return 2
    started = time.monotonic()
    try:
        runner = Runner(exe, work, args.timeout, bool(args.keep))
        # 監査の誤りの数を読めることを、誤りの出る自作の図面と、出ない自作の図面で確かめる
        bad, good = runner.open(control(ZERO_NORMAL), ".dxf"), runner.open(control(""), ".dxf")
        if bad["status"] != "ok" or good["status"] != "ok" or bad["errors"] < 1 or good["errors"] != 0:
            shown = "、".join(f"{name}は状態 {r['status']}・誤り {r.get('errors')}" for name, r in (("誤りの出る図面", bad), ("出ない図面", good)))
            print(f"対照の図面で、監査の誤りの数を読めない（{shown}）。監査の 0 を信じられないので止める", file=sys.stderr)
            return 2
        manifest = json.loads((folder / "manifest.json").read_text(encoding="utf-8"))
        only = None
        if args.issue:
            chosen = {f["path"] for f in manifest["files"] if f.get("issue") in args.issue}
            chosen |= {f["from"] for f in manifest["files"] if f["path"] in chosen and "from" in f}
            only = lambda entry: entry["path"] in chosen  # noqa: E731
        # 実験の入力の読みは、実験と比べるために持っておく
        sources = {f["from"]: None for f in manifest["files"] if f["role"] == "experiment" and (only is None or only(f))}
        seen, totals = [], []

        def check(_folder, entry, reading, baseline, counts):
            failures, audit, dump = check_file(runner, sources, entry, reading, baseline, folder)
            if dump is not None and reading is not None and audit["handles"]:
                before = counts["derived"]
                found, _ = check_handles(entry, reading, counts, view(dump, entry["format"]))
                failures += found
                # AutoCAD が決め直すので比べなかった値の数
                audit["derived"] = counts["derived"] - before
                totals.append(audit["derived"])
            seen.append((entry, audit))
            shown = f"監査の誤り {audit['errors']}" if audit["status"] == "ok" else progress(audit)
            print(f"{entry['path']}: {audit['status']}・{shown}", flush=True)
            return failures, audit

        extra = {"control": {"errors": bad["errors"], "clean": good["errors"]}}
        if args.issue:
            extra["issues"] = sorted(args.issue)
        code = run(folder, "autocad", bad.get("version", ""), {"dxf", "dwg"}, check, KNOWN, only=only, extra=extra)
    finally:
        if not args.keep:
            shutil.rmtree(work, ignore_errors=True)
    states = Counter(audit["status"] for _, audit in seen)
    print("状態: " + "・".join(f"{name} {n}" for name, n in sorted(states.items())) + f"（{time.monotonic() - started:.0f} 秒）")
    print(f"AutoCAD が読み込むときに決め直すので比べなかった値（DXF の両端揃えの文字の高さ・フィットの文字の幅の倍率）: {sum(totals)}")
    for title, chosen in (
        ("AutoCAD が開けなかった・終わらなかったファイル", [f"{e['path']}（{a['status']}）" for e, a in seen if a["status"] != "ok"]),
        ("監査の誤りが 0 でないファイル", [f"{e['path']}（{a['errors']}）" for e, a in seen if a.get("errors")]),
        ("開いた直後に AutoCAD が図面を変えていたファイル", [e["path"] for e, a in seen if a.get("modified")]),
        ("AutoCAD がハンドルを振り直す形の DXF なので、値を照らさなかったファイル", [e["path"] for e, a in seen if a["status"] == "ok" and not a["handles"]]),
    ):
        if chosen:
            print(f"{title}: {len(chosen)}")
            for item in chosen:
                print(f"  {item}")
    for entry, audit in seen:
        if "after_fix" in audit:
            print(f"寸法の部品（{entry['path']}、{'実験' if entry['role'] == 'experiment' else '実験の入力'}）:")
            if "opened" in audit:
                print(f"  開いたとき（入力と比べて）: {shown_parts(audit['opened'])}")
            print(f"  直す監査（誤り {audit['fix']['errors']}・直した数 {audit['fix']['fixed']}）の後（前と比べて）: {shown_parts(audit['after_fix'])}")
    return code


if __name__ == "__main__":
    sys.exit(main())
