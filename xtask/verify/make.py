"""検証キットを作る（#503、決定 0254）。

zumen の CLI（`zumen serve --stdio`）で、公開サンプル（samples/）と適合テストの中で作った図面を開き、書き戻し
（apply）と変換（convert）を当てて、書いたファイルをキットのフォルダーに置く。zumen が読んだ値（drawing・tables・
entities の応答）を、入力と書いたファイルのそれぞれについて置き、ファイルごとの出所・ライセンス・変更・期待する
値を manifest.json に書く。外部の実装（check_ezdxf.py など）と、AutoCAD・Jw_cad・SXF のビューアーで開く人が、
これを照らす。手順と期待する結果は README.md に書く。

使い方: python make.py <zumen の実行ファイル> <キットのフォルダー> [--inputs フォルダー]

キットのフォルダーがあれば、前のキット（manifest.json がある）か空のときだけ消して作り直す。--inputs のフォルダーの
図面も入力にする（受け取った図面を使うとき）。そのキットは、リポジトリの外に作り、Git にも CI の成果物にも入れない。
"""

import argparse
import base64
import json
import math
import shutil
import struct
import subprocess
import sys
import tomllib
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
FIELDS = ["id", "kind", "layer", "source", "bbox", "hash", "geometry"]
# 診断ごとに返す場所の数。既定の 16 では、黙って捨てない照合で、場所のない図形を誤って数える
MAX_PLACES = 100000
# 作った DXF（input/made-dropped.dxf）の、zumen が要素にしない図形の数。既定の場所の数 16 より多くする
DROPPED = 20
# 作った入力が役目を果たすこと: 移さなかったことを言う診断の、code ごとの場所の数
PLACES = {
    "input/made-dropped.dxf": {"dxf.lift.dropped": DROPPED, "dxf.non-planar.dropped": 2},
    "input/made-structure.sfc": {"sxf.lift.invalid": 2, "sxf.lift.dropped": 5},
    "input/made-duplicates.sfc": {"sxf.lift.invalid": 1},
    "input/made-structure.p21": {
        "sxf.lift.invalid": 1,
        "sxf.p21-instance.dropped": 1,
        "sxf.p21-sheet.dropped": 1,
        "sxf.p21-layer-member.dropped": 2,
        "sxf.raster-frame.missing": 1,
    },
    "input/made-non-finite.jww": {"jww.lift.invalid": 1},
    "input/made-classes.jww": {"jww.lift.invalid": 1},
}
# 適合テストの図面とキットのスクリプトのライセンス（このリポジトリと同じ）
REPOSITORY = ("MIT OR Apache-2.0", "zumen-jp（このリポジトリ）。本文は licenses/LICENSE-MIT と licenses/LICENSE-APACHE")
SUFFIXES = {".dxf": "dxf", ".dwg": "dwg", ".jww": "jww", ".sfc": "sfc", ".p21": "p21"}
# 適合テストの中で作った図面（cases の push）。Jw_cad・SXF の公開サンプルがまだないので、これを入力にする
CASES = [
    ("open_jww.json", "a.jww"),
    ("open_sxf.json", "a.sfc"),
    ("open_sxf.json", "c.p21"),
    ("apply_p21_raw.json", "raw-layer.p21"),
]
# ezsxf がフィーチャを型で読む SFC（マーカーが `/*SXF` で、FILE_NAME がある）。適合テストの SFC はマーカーが `/*SXF3` で、
# ezsxf はそのフィーチャの値を読まないので、外部の実装と照らすために作る
SFC = r"""ISO-10303-21;
HEADER;
FILE_DESCRIPTION(('SCADEC level2 feature_mode'),'2;1');
FILE_NAME('made-features.sfc','2026-11-03T00:00:00',(''),(''),'zumen$$3.1','zumen','');
FILE_SCHEMA(('ASSOCIATIVE_DRAUGHTING'));
ENDSEC;
DATA;
/*SXF
#10 = layer_feature(\'L\','1')
SXF*/
/*SXF
#20 = pre_defined_font_feature(\'continuous\')
SXF*/
/*SXF
#30 = pre_defined_colour_feature(\'black\')
SXF*/
/*SXF
#40 = width_feature('0.13')
SXF*/
/*SXF
#50 = line_feature('1','1','1','1','0.0','0.0','100.0','50.0')
SXF*/
/*SXF
#60 = circle_feature('1','1','1','1','50.0','25.0','10.0')
SXF*/
/*SXF
#90 = drawing_sheet_feature(\'S\','3','1','420','297')
SXF*/
ENDSEC;
END-ISO-10303-21;
"""
# 照らすスクリプトの、置き方と黙って捨てない照合の枝を毎晩通す SFC と P21（決定 0121・0129・0130、docs/formats/sxf.md）。
# 作図部品（P）と作図グループ（G）の定義と配置、表示する複合曲線と表示しない複合曲線、それを輪にするハッチング、
# 文字、P21 ではバルーン、zumen が移さない半径 0 の円を持つ。SFC の非表示の複合曲線の前にも、文字・点マーカー・
# ハッチング・作図部品と作図グループの配置を置く（#549）。書き戻しと変換は当てない（CHECKS）
STRUCTURE_SFC = r"""ISO-10303-21;
HEADER;
FILE_DESCRIPTION(('SCADEC level2 feature_mode'),'2;1');
FILE_NAME('made-structure.sfc','2026-11-03T00:00:00',(''),(''),'zumen$$3.1','zumen','');
FILE_SCHEMA(('ASSOCIATIVE_DRAUGHTING'));
ENDSEC;
DATA;
/*SXF
#10 = layer_feature(\'L\','1')
SXF*/
/*SXF
#20 = pre_defined_font_feature(\'continuous\')
SXF*/
/*SXF
#30 = pre_defined_colour_feature(\'black\')
SXF*/
/*SXF
#40 = width_feature('0.13')
SXF*/
/*SXF
#50 = text_font_feature(\'MS\')
SXF*/
/*SXF
#60 = line_feature('1','1','1','1','0.0','0.0','5.0','5.0')
SXF*/
/*SXF
#70 = sfig_org_feature(\'P\','4')
SXF*/
/*SXF
#80 = line_feature('1','1','1','1','0.0','0.0','10.0','0.0')
SXF*/
/*SXF
#90 = sfig_org_feature(\'G\','3')
SXF*/
/*SXF
#100 = polyline_feature('1','1','1','1','5','(50.0,60.0,60.0,50.0,50.0)','(50.0,50.0,60.0,60.0,50.0)')
SXF*/
/*SXF
#110 = composite_curve_org_feature('1','1','1','1')
SXF*/
/*SXF
#120 = polyline_feature('1','1','1','1','5','(54.0,56.0,56.0,54.0,54.0)','(54.0,54.0,56.0,56.0,54.0)')
SXF*/
/*SXF
#121 = text_string_feature('1','1','1',\'BEFORE\','0.0','25.0','3.5','20.0','0.0','0.0','0.0','1','1')
SXF*/
/*SXF
#122 = point_marker_feature('1','1','30.0','30.0','2','0.0','1.0')
SXF*/
/*SXF
#123 = fill_area_style_colour_feature('1','3','1','0','()')
SXF*/
/*SXF
#124 = sfig_locate_feature('1',\'P\','35.0','35.0','0.0','1.0','1.0')
SXF*/
/*SXF
#125 = sfig_locate_feature('1',\'G\','0.0','0.0','0.0','1.0','1.0')
SXF*/
/*SXF
#130 = composite_curve_org_feature('1','1','1','0')
SXF*/
/*SXF
#140 = line_feature('1','1','1','1','0.0','0.0','100.0','0.0')
SXF*/
/*SXF
#150 = circle_feature('1','1','1','1','10.0','10.0','0.0')
SXF*/
/*SXF
#160 = sfig_locate_feature('1',\'P\','20.0','20.0','0.0','1.0','1.0')
SXF*/
/*SXF
#170 = sfig_locate_feature('1',\'G\','0.0','0.0','0.0','1.0','1.0')
SXF*/
/*SXF
#180 = fill_area_style_colour_feature('1','3','1','1','(2)')
SXF*/
/*SXF
#190 = text_string_feature('1','1','1',\'TEXT\','0.0','20.0','3.5','20.0','0.0','0.0','0.0','1','1')
SXF*/
/*SXF
#200 = drawing_sheet_feature(\'S\','3','1','420','297')
SXF*/
ENDSEC;
END-ISO-10303-21;
"""
STRUCTURE_P21 = """ISO-10303-21;
HEADER;
FILE_DESCRIPTION(('SCADEC level2 AP202_mode'),'2;1');
FILE_NAME('made-structure.p21','2026-11-03T00:00:00',(''),(''),'zumen$$3.1','zumen','');
FILE_SCHEMA(('ASSOCIATIVE_DRAUGHTING'));
ENDSEC;
DATA;
#1=DRAUGHTING_PRE_DEFINED_COLOUR('black');
#2=DRAUGHTING_PRE_DEFINED_CURVE_FONT('continuous');
#3=(LENGTH_UNIT() NAMED_UNIT(*) SI_UNIT(.MILLI.,.METRE.));
#4=LENGTH_MEASURE_WITH_UNIT(POSITIVE_LENGTH_MEASURE(0.13),#3);
#5=CURVE_STYLE(' ',#2,#4,#1);
#6=PRESENTATION_STYLE_ASSIGNMENT((#5));
#7=PRESENTATION_STYLE_ASSIGNMENT((NULL_STYLE(.NULL.)));
#8=DIRECTION(' ',(1.,0.));
#9=DIRECTION(' ',(0.,1.));
#10=CARTESIAN_POINT(' ',(0.,0.));
#11=AXIS2_PLACEMENT_2D(' ',#10,#8);
#12=GEOMETRIC_REPRESENTATION_CONTEXT(' ',' ',2);
#13=VECTOR(' ',#8,1.);
#14=VECTOR(' ',#9,1.);
#15=EXTERNALLY_DEFINED_TEXT_FONT('MS',$);
#16=TEXT_STYLE_FOR_DEFINED_FONT(#1);
#17=(TEXT_STYLE(' ',#16) TEXT_STYLE_WITH_BOX_CHARACTERISTICS((BOX_SLANT_ANGLE(0.))) TEXT_STYLE_WITH_SPACING(LENGTH_MEASURE(0.)));
#18=PRESENTATION_STYLE_ASSIGNMENT((#17));
#20=CARTESIAN_POINT(' ',(100.,0.));
#21=LINE(' ',#10,#13);
#22=TRIMMED_CURVE(' ',#21,(#10),(#20),.T.,.CARTESIAN.);
#23=ANNOTATION_CURVE_OCCURRENCE(' ',(#6),#22);
#24=CARTESIAN_POINT(' ',(10.,10.));
#25=AXIS2_PLACEMENT_2D(' ',#24,$);
#26=CIRCLE(' ',#25,0.);
#27=ANNOTATION_CURVE_OCCURRENCE(' ',(#6),#26);
#30=CARTESIAN_POINT(' ',(0.,20.));
#31=AXIS2_PLACEMENT_2D(' ',#30,#8);
#32=PLANAR_EXTENT(' ',20.,3.5);
#33=TEXT_LITERAL_WITH_EXTENT('$$SXF_baseline left','TEXT',#31,'baseline left',.RIGHT.,#15,#32);
#34=(ANNOTATION_OCCURRENCE() ANNOTATION_TEXT_OCCURRENCE() DRAUGHTING_ANNOTATION_OCCURRENCE() GEOMETRIC_REPRESENTATION_ITEM() REPRESENTATION_ITEM(' ') STYLED_ITEM((#18),#33));
#40=CARTESIAN_POINT(' ',(0.,10.));
#41=LINE(' ',#10,#14);
#42=TRIMMED_CURVE(' ',#41,(#10),(#40),.T.,.CARTESIAN.);
#43=ANNOTATION_CURVE_OCCURRENCE(' ',(#6),#42);
#44=DRAUGHTING_SUBFIGURE_REPRESENTATION('$$SXF_G_G',(#43,#11),#12);
#45=SYMBOL_REPRESENTATION_MAP(#11,#44);
#46=SYMBOL_TARGET(' ',#11,1.,1.);
#47=(ANNOTATION_SYMBOL() GEOMETRIC_REPRESENTATION_ITEM() MAPPED_ITEM(#45,#46) REPRESENTATION_ITEM(' '));
#48=(ANNOTATION_OCCURRENCE() ANNOTATION_SUBFIGURE_OCCURRENCE() ANNOTATION_SYMBOL_OCCURRENCE() DRAUGHTING_ANNOTATION_OCCURRENCE() GEOMETRIC_REPRESENTATION_ITEM() REPRESENTATION_ITEM(' ') STYLED_ITEM((#7),#47));
#50=CARTESIAN_POINT(' ',(5.,5.));
#51=DIRECTION(' ',(0.7071067811865475,0.7071067811865475));
#52=VECTOR(' ',#51,1.);
#53=LINE(' ',#10,#52);
#54=TRIMMED_CURVE(' ',#53,(#10),(#50),.T.,.CARTESIAN.);
#55=ANNOTATION_CURVE_OCCURRENCE(' ',(#6),#54);
#56=DRAUGHTING_SUBFIGURE_REPRESENTATION('$$SXF_P_P',(#55,#11),#12);
#57=SYMBOL_REPRESENTATION_MAP(#11,#56);
#58=CARTESIAN_POINT(' ',(20.,20.));
#59=AXIS2_PLACEMENT_2D(' ',#58,#8);
#60=SYMBOL_TARGET(' ',#59,1.,1.);
#61=(ANNOTATION_SYMBOL() GEOMETRIC_REPRESENTATION_ITEM() MAPPED_ITEM(#57,#60) REPRESENTATION_ITEM(' '));
#62=(ANNOTATION_OCCURRENCE() ANNOTATION_SUBFIGURE_OCCURRENCE() ANNOTATION_SYMBOL_OCCURRENCE() DRAUGHTING_ANNOTATION_OCCURRENCE() GEOMETRIC_REPRESENTATION_ITEM() REPRESENTATION_ITEM('$$SXF_P_P') STYLED_ITEM((#7),#61));
#70=CARTESIAN_POINT(' ',(50.,50.));
#71=CARTESIAN_POINT(' ',(60.,50.));
#72=LINE(' ',#70,#13);
#73=TRIMMED_CURVE(' ',#72,(#70),(#71),.T.,.CARTESIAN.);
#74=CARTESIAN_POINT(' ',(60.,60.));
#75=CARTESIAN_POINT(' ',(50.,60.));
#76=POLYLINE(' ',(#71,#74,#75,#70));
#77=COMPOSITE_CURVE_SEGMENT(.CONTINUOUS.,.T.,#73);
#78=COMPOSITE_CURVE_SEGMENT(.CONTINUOUS.,.T.,#76);
#79=COMPOSITE_CURVE(' ',(#77,#78),.F.);
#80=ANNOTATION_CURVE_OCCURRENCE(' ',(#6),#79);
#81=CARTESIAN_POINT(' ',(55.,55.));
#82=AXIS2_PLACEMENT_2D(' ',#81,#8);
#83=CIRCLE(' ',#82,2.);
#84=COMPOSITE_CURVE_SEGMENT(.CONTINUOUS.,.T.,#83);
#85=COMPOSITE_CURVE(' ',(#84),.F.);
#86=ANNOTATION_CURVE_OCCURRENCE(' ',(#6),#85);
#87=INVISIBILITY((#86));
#88=DRAUGHTING_PRE_DEFINED_COLOUR('green');
#89=FILL_AREA_STYLE_COLOUR(' ',#88);
#90=FILL_AREA_STYLE(' ',(#89));
#91=PRESENTATION_STYLE_ASSIGNMENT((#90));
#92=ANNOTATION_FILL_AREA(' ',(#79,#85));
#93=ANNOTATION_FILL_AREA_OCCURRENCE(' ',(#91),#92,#70);
#100=CARTESIAN_POINT(' ',(80.,10.));
#101=CARTESIAN_POINT(' ',(90.,20.));
#102=POLYLINE(' ',(#100,#101));
#103=(ANNOTATION_CURVE_OCCURRENCE() ANNOTATION_OCCURRENCE() DRAUGHTING_ANNOTATION_OCCURRENCE() GEOMETRIC_REPRESENTATION_ITEM() LEADER_CURVE() REPRESENTATION_ITEM(' ') STYLED_ITEM((#6),#102));
#104=CARTESIAN_POINT(' ',(95.,25.));
#105=AXIS2_PLACEMENT_2D(' ',#104,#8);
#106=CIRCLE(' ',#105,5.);
#107=ANNOTATION_CURVE_OCCURRENCE(' ',(#6),#106);
#108=CARTESIAN_POINT(' ',(95.,23.));
#109=AXIS2_PLACEMENT_2D(' ',#108,#8);
#110=PLANAR_EXTENT(' ',2.,4.);
#111=TEXT_LITERAL_WITH_EXTENT('$$SXF_middleline centre','3',#109,'middleline centre',.RIGHT.,#15,#110);
#112=ANNOTATION_TEXT_OCCURRENCE(' ',(#18),#111);
#113=(DRAUGHTING_CALLOUT((#103,#107,#112)) DRAUGHTING_ELEMENTS() GEOMETRIC_REPRESENTATION_ITEM() LEADER_DIRECTED_CALLOUT() REPRESENTATION_ITEM(' '));
#123=DRAWING_SHEET_REVISION('A4_vertical',(#27),$,'02');
#124=PRESENTATION_LAYER_ASSIGNMENT('OTHER',' ',(#23,#34));
#120=PRESENTATION_LAYER_ASSIGNMENT('L',' ',(#23,#27,#34,#43,#48,#55,#62,#80,#86,#93,#103,#107,#112));
#130=POLYLINE(' ',(#70,#71));
#131=ANNOTATION_CURVE_OCCURRENCE(' ',(#6),#130);
#132=POLYLINE(' ',(#70,#71,/* points */#74,#75));
#133=ANNOTATION_CURVE_OCCURRENCE(' ',/* style */(#6),#132);
#134=POLYLINE(' ',(#71,#74,#75,#70));
#135=ANNOTATION_CURVE_OCCURRENCE(' ',(#6),#134);
#136=DRAUGHTING_SUBFIGURE_REPRESENTATION('$$SXF_G_$$RASTER$$frame.png',(#131,#133,#135,#11),#12);
#137=SYMBOL_REPRESENTATION_MAP(#11,#136);
#138=(ANNOTATION_SYMBOL() GEOMETRIC_REPRESENTATION_ITEM() MAPPED_ITEM(#137,#46) REPRESENTATION_ITEM(' '));
#139=(ANNOTATION_OCCURRENCE() ANNOTATION_SUBFIGURE_OCCURRENCE() ANNOTATION_SYMBOL_OCCURRENCE() DRAUGHTING_ANNOTATION_OCCURRENCE() GEOMETRIC_REPRESENTATION_ITEM() REPRESENTATION_ITEM(' ') STYLED_ITEM((#7),#138));
#140=DRAUGHTING_SUBFIGURE_REPRESENTATION('$$SXF_G_$$RASTER$$missing.png',(#131,#11),#12);
#141=SYMBOL_REPRESENTATION_MAP(#11,#140);
#142=(ANNOTATION_SYMBOL() GEOMETRIC_REPRESENTATION_ITEM() MAPPED_ITEM(#141,#46) REPRESENTATION_ITEM(' '));
#143=(ANNOTATION_OCCURRENCE() ANNOTATION_SUBFIGURE_OCCURRENCE() ANNOTATION_SYMBOL_OCCURRENCE() DRAUGHTING_ANNOTATION_OCCURRENCE() GEOMETRIC_REPRESENTATION_ITEM() REPRESENTATION_ITEM(' ') STYLED_ITEM((#7),#142));
#144=PRESENTATION_LAYER_ASSIGNMENT('FRAME/*L*/',' ',(/* members */#131,#133,#135));
#121=DRAWING_SHEET_REVISION('A3_horizontal',(#23,#27,#34,#48,#62,#93,#113,#139,#143,#133),$,'01');
#122=DRAUGHTING_TITLE((),'JAPANESE','S');
#20=CARTESIAN_POINT(' ',(999.,999.));
ENDSEC;
END-ISO-10303-21;
"""
# 外部の実装と照らすためだけに作る入力。書き戻しと変換を当てない
CHECKS = {"input/made-structure.sfc", "input/made-structure.p21", "input/made-duplicates.sfc", "input/made-non-finite.jww", "input/made-classes.jww"}


def duplicate_sfc():
    """同じ番号のレイヤ 2 つと、引数の欠けた線・線・円・別の線をファイルの順に置く（#553）。"""
    features = [
        (70, r"layer_feature(\'A\','1')"),
        (70, r"layer_feature(\'B\','1')"),
        (20, r"pre_defined_font_feature(\'continuous\')"),
        (30, r"pre_defined_colour_feature(\'black\')"),
        (40, "width_feature('0.13')"),
        (60, "line_feature('1')"),
        (60, "line_feature('1','1','1','1','0','0','10','5')"),
        (60, "circle_feature('2','1','1','1','5','5','2')"),
        (60, "line_feature('2','1','1','1','8','2','18','7')"),
        (90, r"drawing_sheet_feature(\'S\','3','1','420','297')"),
    ]
    header = SFC.split("/*SXF", 1)[0].replace("made-features.sfc", "made-duplicates.sfc")
    return header + "".join(f"/*SXF\n#{i} = {value}\nSXF*/\n" for i, value in features) + "ENDSEC;\nEND-ISO-10303-21;\n"


class Zumen:
    """`zumen serve --stdio` の JSON-RPC（bindings/cli/README.md）"""

    def __init__(self, exe):
        self.child = subprocess.Popen([str(Path(exe).resolve()), "serve", "--stdio"], stdin=subprocess.PIPE, stdout=subprocess.PIPE)
        self.next = 0
        self.rpc("session_new", {})

    def rpc(self, method, params):
        self.next += 1
        line = json.dumps({"jsonrpc": "2.0", "id": self.next, "method": method, "params": params}) + "\n"
        self.child.stdin.write(line.encode())
        self.child.stdin.flush()
        message = json.loads(self.child.stdout.readline())
        if "error" in message:
            raise RuntimeError(f"{method}: {message['error']}")
        return message["result"]

    def push(self, name, data):
        status = self.rpc("push", {"name": name, "base64": base64.b64encode(data).decode()})["status"]
        if status != "ok":
            raise RuntimeError(f"push {name}: {status}")

    def call(self, request):
        result = self.rpc("call", {"request": {"v": 1, "max_places": MAX_PLACES, **request}})
        return result["status"], json.loads(result["response"]) if result.get("response") else None

    def take(self, blob):
        result = self.rpc("take", {"blob": blob})
        if result["status"] != "ok":
            raise RuntimeError(f"take {blob}: {result['status']}")
        return base64.b64decode(result["base64"])

    def close(self):
        self.child.stdin.close()
        self.child.wait()


def read(z, doc):
    """文書の図面モデルの要約・表・要素の一覧"""
    _, drawing = z.call({"op": "drawing", "doc": doc})
    _, tables = z.call({"op": "tables", "doc": doc})
    items, offset = [], 0
    while True:
        _, page = z.call({"op": "entities", "doc": doc, "fields": FIELDS, "offset": offset, "limit": 10000})
        items += page["result"]["items"]
        offset = page["result"].get("next_offset")
        if offset is None:
            break
    return {
        "drawing": drawing.get("result"),
        "diagnostics": drawing["diagnostics"],
        "tables": tables.get("result"),
        "entities": items,
    }


def open_file(z, name, data):
    z.push(name, data)
    status, response = z.call({"op": "open", "input": name})
    if status != "ok":
        raise RuntimeError(f"open {name}: {status} {response and response['diagnostics']}")
    return response["diagnostics"]


# 変更の作り方。どれも読んだ値（entities）から、その図面で当てられる変更を作る。期待する値は、zumen の読み手に
# 頼らず、変更そのものと変換の行列から決める。


def model(reading, kind=None, shape=None):
    """実寸の空間（space/0）の要素"""
    return [
        e
        for e in reading["entities"]
        if e["id"].startswith("space/0/")
        and (kind is None or e["kind"] == kind)
        and (shape is None or e.get("geometry", {}).get("shape") == shape)
    ]


def similar(degrees, scale, dx, dy):
    """回転・等方の尺度・平行移動の行列 [a, b, c, d, e, f]"""
    c, s = math.cos(math.radians(degrees)) * scale, math.sin(math.radians(degrees)) * scale
    return [c, -s, dx, s, c, dy]


def mapped(m, p):
    return [m[0] * p[0] + m[1] * p[1] + m[2], m[3] * p[0] + m[4] * p[1] + m[5]]


def transformed(geometry, m):
    """相似の変換（鏡に映さない）で写した形。回転と尺度は行列から読む。写し方を決めていない形は例外にする"""
    turn = math.degrees(math.atan2(m[3], m[0]))
    scale = math.hypot(m[0], m[3])
    g = dict(geometry)
    shape = g["shape"]
    if shape in ("point", "symbol", "text", "formatted_text"):
        g["position"] = mapped(m, g["position"])
        g["rotation"] = g.get("rotation", 0) + turn
    if shape in ("text", "formatted_text"):
        g["height"] = g["height"] * scale
        if "end" in g:
            g["end"] = mapped(m, g["end"])
        if "width" in g:
            g["width"] = g["width"] * scale
    if shape == "symbol":
        g["scale"] = [v * scale for v in g["scale"]]
        if "grid" in g:
            g["grid"] = {**g["grid"], "column_spacing": g["grid"]["column_spacing"] * scale, "row_spacing": g["grid"]["row_spacing"] * scale}
    if shape == "line":
        g["start"], g["end"] = mapped(m, g["start"]), mapped(m, g["end"])
    if shape == "polyline":
        g["points"] = [mapped(m, p) for p in g["points"]]
    if shape in ("circle", "arc"):
        g["center"], g["radius"] = mapped(m, g["center"]), g["radius"] * scale
    if shape == "arc":
        g["start_angle"], g["end_angle"] = g["start_angle"] + turn, g["end_angle"] + turn
    if shape == "ellipse":
        # 角度は長軸から測るので変わらない
        g["center"], g["major"] = mapped(m, g["center"]), mapped(m, g["major"])
    if shape not in ("point", "symbol", "text", "formatted_text", "line", "polyline", "circle", "arc", "ellipse"):
        raise ValueError(f"形 {shape} の変換の写し方を決めていない")
    return g


def points_steps(fmt):
    """#342: 点を置き換え・変換・追加し、DXF・DWG は文書の点の表し方も変える。JWW は線・円・円弧も足す（#702）"""

    def step(reading):
        edits = []
        if fmt in ("dxf", "dwg"):
            found = [e for e in model(reading, "point") if not e["geometry"].get("construction")]
            if found:
                p = found[0]["geometry"]["position"]
                edits.append(replace(found[0], {"shape": "point", "position": [p[0] + 5, p[1] + 7], "rotation": 30}))
            if len(found) > 1:
                edits.append(transform(found[1], similar(90, 1, 10, 20)))
            edits.append(add("space/0", {"shape": "point", "position": [100, 50], "rotation": 0}))
            edits.append(add("space/0", {"shape": "point", "position": [120, 50], "rotation": 45}))
            edits.append({"kind": "point_style", "mark": "cross", "circle": False, "square": True, "size": 4, "size_kind": "length"})
        elif fmt == "jww":
            # 普通の点は角度 0 だけ。記号の点は jww.point の番号（決定 0183）。レイヤは線分と同じもので、JWW も
            # レイヤに従う色を持たないので、ペンの番号を書く（2 は黒）。レイヤを取る要素がなければ、当てる候補のない段にする
            if not model(reading):
                return edits
            layer, color = model(reading)[0]["layer"], {"jww": 2}
            edits.append(add("space/0", {"shape": "point", "position": [1000, 2000], "rotation": 0}, layer=layer, color=color))
            point = {"shape": "point", "position": [3000, 2000], "rotation": 30, "marker": {"namespace": "jww.point", "code": "3", "scale": 2}}
            edits.append(add("space/0", point, layer=layer, color=color))
            # 線・円・円弧は、JWW がレイヤに従う線種も持たないので、線種も書く。線幅は書かず、ペンの印刷の幅に従う（#702）
            solid = {"id": "line-type/1"}
            edits.append(add("space/0", {"shape": "line", "start": [1000, 3000], "end": [4000, 3500]}, layer=layer, color=color, line_type=solid))
            edits.append(add("space/0", {"shape": "circle", "center": [5000, 3000], "radius": 500}, layer=layer, color=color, line_type=solid))
            arc = {"shape": "arc", "center": [6500, 3000], "radius": 500, "start_angle": 0, "end_angle": 135}
            edits.append(add("space/0", arc, layer=layer, color=color, line_type=solid))
        else:
            # SXF はレイヤに従う色を持たないので、色の番号を書く（2 は赤）。レイヤは、表で定義したレイヤ（defined）に
            # ある最初の要素のものか、表の最初のもの。表にないレイヤ（レイヤのコードが 0 のフィーチャなど）に足すと、
            # zumen は断る（change.edit.unsupported）。定義したレイヤがなければ、当てる候補のない段にする
            defined = [t["name"] for t in reading.get("tables", {}).get("layers", []) if t.get("defined")]
            layer = next((e["layer"] for e in reading["entities"] if e.get("layer") in defined), next(iter(defined), None))
            if layer is None:
                return edits
            marker = {"namespace": "sxf.marker", "code": "1", "scale": 1}
            color = {"sxf": 2}
            edits.append(add("sheet/0", {"shape": "point", "position": [50, 60], "rotation": 30, "marker": marker}, layer=layer, color=color))
            point = {"shape": "point", "position": [80, 60], "rotation": 0, "marker": {**marker, "code": "3"}}
            edits.append(add("sheet/0", point, layer=layer, color=color))
        return edits

    return [step]


def basic_steps(_fmt):
    """#156: 画層を足し、単純な図形を足し・置き換え・変換し、色を変える"""

    def layer(_reading):
        return [{"kind": "add_layer", "name": "ZUMEN_ADDED", "color": {"aci": 3}}]

    def shapes(reading):
        edits = [
            add("space/0", {"shape": "line", "start": [0, -50], "end": [100, -30]}, layer="ZUMEN_ADDED"),
            add("space/0", {"shape": "circle", "center": [150, -40], "radius": 15}),
            add("space/0", {"shape": "arc", "center": [200, -40], "radius": 15, "start_angle": 0, "end_angle": 135}),
            # major は長軸の端の点（中心からの向きでない）
            add("space/0", {"shape": "ellipse", "center": [250, -40], "major": [270, -40], "ratio": 0.5, "start_angle": 0, "end_angle": 360}),
            add("space/0", {"shape": "polyline", "points": [[0, -100], [40, -100], [40, -70]], "bulges": [0, 0.5, 0], "closed": False}),
            add("space/0", {"shape": "text", "content": "ZUMEN TEXT", "position": [0, -130], "height": 5, "rotation": 0}),
            add("space/0", {"shape": "formatted_text", "content": "ZUMEN\\PMTEXT", "position": [100, -120], "height": 5, "rotation": 0}),
        ]
        named = [e for e in model(reading, shape="symbol") if not e["geometry"]["name"].startswith("*")]
        if named:
            edits.append(add("space/0", {"shape": "symbol", "name": named[0]["geometry"]["name"], "position": [300, -100], "scale": [1, 1], "rotation": 0}))
        circles = model(reading, shape="circle")
        if circles:
            c = circles[0]["geometry"]
            edits.append(replace(circles[0], {"shape": "circle", "center": [c["center"][0] + 10, c["center"][1]], "radius": c["radius"] * 1.5}))
        return edits

    def angles(_reading):
        # 角度と寄せ方を持つ形。外部の実装の、ラジアンから度への直しと、文字の寄せ方の読みを確かめる
        aligned = math.degrees(math.atan2(30, 40))
        return [
            add("space/0", {"shape": "formatted_text", "content": "ZUMEN ROTATED", "position": [0, -200], "height": 5, "rotation": 30, "attachment": "middle_center", "width": 40}),
            add("space/0", {"shape": "text", "content": "ALIGNED", "position": [100, -200], "end": [140, -170], "fit": "aligned", "height": 3, "rotation": aligned}),
            add("space/0", {"shape": "text", "content": "FIT", "position": [200, -200], "end": [250, -200], "fit": "fit", "height": 4, "rotation": 0}),
            add("space/0", {"shape": "text", "content": "R45", "position": [300, -200], "height": 2, "rotation": 45}),
            add("space/0", {"shape": "ellipse", "center": [0, -260], "major": [20, -250], "ratio": 0.4, "start_angle": 30, "end_angle": 200}),
            add("space/0", {"shape": "arc", "center": [100, -260], "radius": 7, "start_angle": 350, "end_angle": 10}),
            add("space/0", {"shape": "point", "position": [200, -260], "rotation": 60}),
        ]

    def moves(reading):
        edits = []
        lines = [e for e in model(reading, shape="polyline") if not any(e["geometry"].get("bulges", []))]
        if lines:
            edits.append(transform(lines[0], similar(0, 1, 0, -20)))
        circles = [e for e in model(reading, shape="circle") if e["geometry"]["radius"] != 15]
        if circles:
            edits.append({"kind": "restyle", "id": circles[0]["id"], "hash": circles[0]["hash"], "color": {"aci": 1}})
        return edits

    return [layer, shapes, angles, moves]


def minsert_steps(_fmt):
    """#316: 格子の記号（MINSERT）を変換し・置き換え、足す"""

    def step(reading):
        grids = [e for e in model(reading, shape="symbol") if "grid" in e["geometry"]]
        edits = []
        if grids:
            edits.append(transform(grids[0], similar(30, 2, 50, 0)))
        if len(grids) > 1:
            g = dict(grids[1]["geometry"])
            p = g["position"]
            g["position"] = [p[0], p[1] + 100]
            g["grid"] = {**g["grid"], "column_spacing": g["grid"]["column_spacing"] * 1.5, "row_spacing": g["grid"]["row_spacing"] * 1.5}
            edits.append(replace(grids[1], g))
        if grids:
            grid = {"columns": 3, "rows": 2, "column_spacing": 30, "row_spacing": 20}
            edits.append(add("space/0", {"shape": "symbol", "name": grids[0]["geometry"]["name"], "position": [300, 300], "scale": [1, 1], "rotation": 0, "grid": grid}))
        return edits

    return [step]


def replace(e, geometry):
    return {"kind": "replace", "id": e["id"], "hash": e["hash"], "geometry": geometry, "_expect": geometry}


def transform(e, m):
    return {"kind": "transform", "id": e["id"], "hash": e["hash"], "matrix": m, "_expect": transformed(e["geometry"], m)}


def add(place, geometry, layer=None, color=None, line_type=None):
    edit = {"kind": "add", "place": place, "geometry": geometry, "_expect": geometry}
    if layer:
        edit["layer"] = layer
    if color:
        edit["color"] = color
    if line_type:
        edit["line_type"] = line_type
    return edit


# キットの組。名前・対象の issue・入力の選び方・変更の作り方
RECIPES = [
    ("points", 342, lambda name, fmt: fmt in ("jww", "sfc", "p21") or "points-" in name, points_steps),
    ("basic", 156, lambda name, fmt: fmt in ("dxf", "dwg") and ("-layout-" in name or "-xclip-r2013" in name), basic_steps),
    ("minsert", 316, lambda name, fmt: fmt in ("dxf", "dwg") and "-minsert-" in name, minsert_steps),
]


def dropped_dxf():
    """zumen が要素にしない図形（3DFACE）を DROPPED 持つ DXF。黙って捨てない照合で、診断が図形のレコードを指すことを
    確かめる（既定の場所の数 16 を超える）。平面の図面モデルに移さない図形（Z が 0 でない線分と、押し出しの向きが X の
    円。dxf.non-planar.dropped）も持つ。図形はハンドルを持ち、ezdxf の図形と zumen の要素を結べる"""

    def tags(*pairs):
        return "".join(f"{code}\n{value}\n" for code, value in pairs)

    out = tags((0, "SECTION"), (2, "TABLES"), (0, "TABLE"), (2, "LAYER"), (70, 1))
    out += tags((0, "LAYER"), (2, "0"), (70, 0), (62, 7), (6, "CONTINUOUS"), (0, "ENDTAB"), (0, "ENDSEC"))
    out += tags((0, "SECTION"), (2, "ENTITIES"))
    out += tags((0, "LINE"), (5, "100"), (8, "0"), (10, 0.0), (20, 0.0), (30, 0.0), (11, 100.0), (21, 50.0), (31, 0.0))
    for i in range(DROPPED):
        pairs = [(0, "3DFACE"), (5, f"{0x101 + i:X}"), (8, "0")]
        for k, (x, y) in enumerate([(0, 0), (5, 0), (5, 5), (0, 5)]):
            pairs += [(10 + k, float(i * 10 + x)), (20 + k, float(y)), (30 + k, 0.0)]
        out += tags(*pairs)
    out += tags((0, "LINE"), (5, "200"), (8, "0"), (10, 0.0), (20, 20.0), (30, 5.0), (11, 50.0), (21, 20.0), (31, 5.0))
    out += tags((0, "CIRCLE"), (5, "201"), (8, "0"), (10, 0.0), (20, 30.0), (30, 0.0), (40, 5.0), (210, 1.0), (220, 0.0), (230, 0.0))
    return out + tags((0, "ENDSEC"), (0, "EOF"))


def shapeless_dxf():
    """zumen の entities の応答が形を返さない要素になる図形を持つ DXF（R2010）。HATCH・SOLID・TRACE はハッチング、
    IMAGE・PDFUNDERLAY は画像の範囲、ACAD_TABLE は表の要素になる。ACAD_TABLE は AC1018 から、PDFUNDERLAY と
    PDFDEFINITION は AC1024 からの図形とオブジェクトなので、$ACADVER を AC1024 にする。要素の種類を出所の図形の種類と比べる照合（compare.py の KINDS）を、
    毎晩のキットで通す（#558）。画像と PDF の定義は OBJECTS 節の辞書に置く（DXF Reference「IMAGEDEF」
    「UNDERLAYDEFINITION」）"""

    def tags(*pairs):
        return "".join(f"{code}\n{value}\n" for code, value in pairs)

    def entity(kind, handle, subclass, *pairs):
        return tags((0, kind), (5, handle), (100, "AcDbEntity"), (8, "0"), (100, subclass), *pairs)

    def item(kind, handle, owner, subclass, *pairs):
        return tags((0, kind), (5, handle), (330, owner), (100, subclass), *pairs)

    def corners(kind, handle, x):
        # SOLID・TRACE の 4 隅。3 つ目と 4 つ目は、辺をたどる順と逆に置く（DXF Reference「SOLID」）
        pairs = []
        for k, (dx, y) in enumerate([(0, 0), (10, 0), (0, 10), (10, 10)]):
            pairs += [(10 + k, float(x + dx)), (20 + k, float(y)), (30 + k, 0.0)]
        return entity(kind, handle, "AcDbTrace", *pairs)

    out = tags((0, "SECTION"), (2, "HEADER"), (9, "$ACADVER"), (1, "AC1024"), (9, "$HANDSEED"), (5, "40C"))
    out += tags((9, "$INSUNITS"), (70, 4), (0, "ENDSEC"))
    out += tags((0, "SECTION"), (2, "TABLES"), (0, "TABLE"), (2, "LAYER"), (5, "2"), (330, "0"), (100, "AcDbSymbolTable"), (70, 1))
    out += item("LAYER", "10", "2", "AcDbSymbolTableRecord", (100, "AcDbLayerTableRecord"), (2, "0"), (70, 0), (62, 7), (6, "Continuous"))
    out += tags((0, "ENDTAB"), (0, "ENDSEC"), (0, "SECTION"), (2, "BLOCKS"))
    # 表を描いたブロック（表の枠）
    out += entity("BLOCK", "306", "AcDbBlockBegin", (2, "*T1"), (70, 1), (10, 0.0), (20, 0.0), (30, 0.0), (3, "*T1"), (1, ""))
    for k, (x1, y1, x2, y2) in enumerate([(0, 0, 20, 0), (20, 0, 20, 5), (20, 5, 0, 5), (0, 5, 0, 0)]):
        out += entity("LINE", f"{0x307 + k:X}", "AcDbLine", (10, x1 + 0.0), (20, y1 + 0.0), (30, 0.0), (11, x2 + 0.0), (21, y2 + 0.0), (31, 0.0))
    out += entity("ENDBLK", "30B", "AcDbBlockEnd") + tags((0, "ENDSEC"), (0, "SECTION"), (2, "ENTITIES"))
    # 塗りつぶしのハッチング。境界は閉じた折れ線 1 つ（92 の 2 は折れ線、1 は外側）
    out += entity("HATCH", "300", "AcDbHatch", (10, 0.0), (20, 0.0), (30, 0.0), (210, 0.0), (220, 0.0), (230, 1.0), (2, "SOLID"), (70, 1))
    out += tags((71, 0), (91, 1), (92, 3), (72, 0), (73, 1), (93, 4))
    out += tags((10, 0.0), (20, 0.0), (10, 10.0), (20, 0.0), (10, 10.0), (20, 10.0), (10, 0.0), (20, 10.0))
    out += tags((97, 0), (75, 0), (76, 1), (98, 0))
    out += corners("SOLID", "301", 20)
    out += corners("TRACE", "302", 40)
    # 100 × 50 画素の画像。1 画素は 0.1
    out += entity("IMAGE", "303", "AcDbRasterImage", (90, 0), (10, 60.0), (20, 0.0), (30, 0.0), (11, 0.1), (21, 0.0), (31, 0.0))
    out += tags((12, 0.0), (22, 0.1), (32, 0.0), (13, 100.0), (23, 50.0), (340, "403"), (70, 1), (280, 0), (281, 50), (282, 50), (283, 0))
    out += entity("PDFUNDERLAY", "304", "AcDbUnderlayReference", (340, "404"), (10, 80.0), (20, 0.0), (30, 0.0), (41, 1.0), (42, 1.0))
    out += tags((43, 1.0), (50, 0.0), (280, 2), (281, 100), (282, 0))
    # 1 行 1 列の表。セルは文字（171 が 1）
    out += entity("ACAD_TABLE", "305", "AcDbBlockReference", (2, "*T1"), (10, 100.0), (20, 0.0), (30, 0.0), (100, "AcDbTable"), (280, 0))
    out += tags((11, 1.0), (21, 0.0), (31, 0.0), (90, 22), (91, 1), (92, 1), (93, 0), (94, 0), (95, 0), (96, 0), (141, 5.0), (142, 20.0))
    out += tags((171, 1), (172, 0), (173, 0), (174, 0), (175, 1), (176, 1), (91, 0), (178, 0), (145, 0.0), (1, "A"))
    out += tags((0, "ENDSEC"), (0, "SECTION"), (2, "OBJECTS"))
    out += item("DICTIONARY", "400", "0", "AcDbDictionary", (281, 1), (3, "ACAD_IMAGE_DICT"), (350, "401"), (3, "ACAD_PDFDEFINITIONS"), (350, "402"))
    out += item("DICTIONARY", "401", "400", "AcDbDictionary", (281, 1), (3, "picture"), (350, "403"))
    out += item("DICTIONARY", "402", "400", "AcDbDictionary", (281, 1), (3, "drawing - 1"), (350, "404"))
    out += item("IMAGEDEF", "403", "401", "AcDbRasterImageDef", (90, 0), (1, "picture.png"), (10, 100.0), (20, 50.0), (11, 0.1), (21, 0.1))
    out += tags((280, 1), (281, 0))
    out += item("PDFDEFINITION", "404", "402", "AcDbUnderlayDefinition", (1, "drawing.pdf"), (2, "1"))
    return out + tags((0, "ENDSEC"), (0, "EOF"))


def japanese_dxf(version, encoding):
    """日本語の画層を持つ DXF。R2004 は ANSI_932、R2007 は同じコード・ページの指定でも UTF-8 で書く。
    変換で足す線種・画層・文字スタイルも持ち、TABLE の最終件数を照らす（DXF Reference「TABLES Section」）"""

    def tags(*pairs):
        return "".join(f"{code}\n{value}\n" for code, value in pairs)

    def table(name, handle, count):
        return tags((0, "TABLE"), (2, name), (5, handle), (330, "0"), (100, "AcDbSymbolTable"), (70, count))

    def record(kind, handle, owner, subclass):
        return tags((0, kind), (5, handle), (330, owner), (100, "AcDbSymbolTableRecord"), (100, subclass))

    out = tags((0, "SECTION"), (2, "HEADER"), (9, "$ACADVER"), (1, version), (9, "$DWGCODEPAGE"), (3, "ANSI_932"), (0, "ENDSEC"))
    out += tags((0, "SECTION"), (2, "TABLES"))
    out += table("LTYPE", "10", 2)
    out += record("LTYPE", "11", "10", "AcDbLinetypeTableRecord")
    out += tags((2, "Continuous"), (70, 0), (3, ""), (72, 65), (73, 0), (40, 0))
    out += record("LTYPE", "12", "10", "AcDbLinetypeTableRecord")
    out += tags((2, "DASH"), (70, 0), (3, ""), (72, 65), (73, 2), (40, 1), (49, 0.5), (74, 0), (49, -0.5), (74, 0), (0, "ENDTAB"))
    out += table("LAYER", "20", 2)
    for handle, name in [("21", "0"), ("22", "日本語")]:
        out += record("LAYER", handle, "20", "AcDbLayerTableRecord")
        out += tags((2, name), (70, 0), (62, 7), (6, "Continuous"))
    out += tags((0, "ENDTAB"))
    out += table("STYLE", "30", 2)
    for handle, name in [("31", "Standard"), ("32", "Caption")]:
        out += record("STYLE", handle, "30", "AcDbTextStyleTableRecord")
        out += tags((2, name), (70, 0), (40, 0), (41, 1), (50, 0), (71, 0), (42, 1), (3, "txt"), (4, ""))
    out += tags((0, "ENDTAB"), (0, "ENDSEC"), (0, "SECTION"), (2, "ENTITIES"))
    out += tags((0, "LINE"), (5, "100"), (100, "AcDbEntity"), (8, "日本語"), (100, "AcDbLine"))
    out += tags((10, 0), (20, 0), (30, 0), (11, 100), (21, 50), (31, 0), (0, "ENDSEC"), (0, "EOF"))
    return out.encode(encoding)


def codes(*lists):
    """診断の (code, kind) ごとの件数"""
    found = Counter()
    for diagnostics in lists:
        for d in diagnostics:
            found[(d["code"], d.get("params", {}).get("kind"))] += d.get("count", 1)
    return found


def reopen(z, out, record, data, baseline):
    """書いたファイルを zumen で読み直して読みを置く。baseline（入力の診断の (code, kind) ごとの件数）より件数の
    増えた診断があれば、reread に残す"""
    record["open"] = open_file(z, record["path"] + ".reopened", data)
    reading = read(z, record["path"] + ".reopened")
    save_reading(out, record["path"], reading)
    found = codes(record["open"], reading["diagnostics"])
    new = sorted(f"{code}（{kind}）{n}" if kind else f"{code} {n}" for (code, kind), n in found.items() if n > baseline[(code, kind)])
    if new:
        record["reread"] = new


def run_recipe(z, out, entry, data, recipe, baseline):
    """入力に変更の段を順に当て、最後に書いたファイルを置く。どれかの段が当たらなければ、その診断を残す"""
    name, issue, _, steps = recipe
    stem, suffix = Path(entry["path"]).stem, Path(entry["path"]).suffix
    doc = f"{issue}/{stem}.{name}{suffix}"
    open_file(z, doc, data)
    record = {"path": doc, "role": "apply", "issue": issue, "recipe": name, "from": entry["path"], "format": entry["format"], "steps": []}
    current = data
    for make in steps(entry["format"]):
        reading = read(z, doc)
        edits = make(reading)
        if not edits:
            # 当てる候補がない段（例: 膨らみのないポリラインがない図面の移動）も、キットに残して見えるようにする
            record["steps"].append({"step": make.__name__, "edits": [], "skipped": True})
            continue
        request = [{k: v for k, v in e.items() if not k.startswith("_")} for e in edits]
        status, response = z.call({"op": "apply", "doc": doc, "edits": request})
        step = {"step": make.__name__, "edits": request, "status": status, "diagnostics": response["diagnostics"]}
        record["steps"].append(step)
        if status != "ok":
            record["status"] = "failed"
            return record
        applied = response["result"]["applied"]
        # 期待する形と、足した要素の画層と色
        step["expect"] = [
            {"id": a.get("id"), "edit": i, "geometry": e["_expect"], **{k: e[k] for k in ("layer", "color", "line_type") if k in e}}
            for i, (e, a) in enumerate(zip(edits, applied))
            if "_expect" in e and a.get("id")
        ]
        current = z.take(response["blobs"][0]["id"])
    (out / doc).parent.mkdir(parents=True, exist_ok=True)
    (out / doc).write_bytes(current)
    record["status"] = "ok"
    reopen(z, out, record, current, baseline)
    return record


def run_convert(z, out, entry, data, baseline, layer=None):
    """公開 API で DXF に変換する（決定 0206・0281）"""
    folder = f"convert/{layer}" if layer else "convert"
    source = f"{folder}/{Path(entry['path']).name}"
    target = source + ".dxf"
    record = {"path": target, "role": "convert", "issue": 302, "from": entry["path"], "format": "dxf"}
    open_file(z, source, data)
    if layer:
        record["added_layer"] = layer
        edit = {"kind": "add_layer", "name": layer, "color": {"aci": 3}}
        status, response = z.call({"op": "apply", "doc": source, "edits": [edit]})
        if status != "ok":
            return {**record, "status": status, "diagnostics": response["diagnostics"]}
        z.take(response["blobs"][0]["id"])
    status, response = z.call({"op": "convert", "doc": source, "format": "dxf", "to": target})
    record["status"] = status
    record["diagnostics"] = response["diagnostics"]
    if status != "ok":
        return record
    record["elements"] = response["result"]["elements"]
    data = z.take(response["blobs"][0]["id"])
    (out / target).parent.mkdir(parents=True, exist_ok=True)
    (out / target).write_bytes(data)
    # 入力が持つ値の欠け（例: 単位のない DXF）は、変換した DXF にも残るので、入力の診断と比べる
    reopen(z, out, record, data, baseline)
    return record


def tag_lines(data):
    """DXF のタグの行（改行を含むバイト列）の組"""
    lines = data.splitlines(keepends=True)
    return [(lines[i], lines[i + 1]) for i in range(0, len(lines) - 1, 2)]


def dimension_experiments(out, entry, data):
    """#314: 寸法の部品（匿名のブロック）を書き直さずに寸法を変えた DXF。zumen の書き手を通さない実験で、AutoCAD が
    部品を作り直すかを見る。apply は寸法の変更を断る（change.edit.unsupported）"""
    tags = tag_lines(data)
    starts = [i for i, (code, value) in enumerate(tags) if code.strip() == b"0" and value.strip() == b"ARC_DIMENSION"]
    if not starts:
        return []

    def end_of(i):
        return next((j for j in range(i + 1, len(tags)) if tags[j][0].strip() == b"0"), len(tags))

    def handle(i):
        return next(tags[j][1].strip().decode() for j in range(i, end_of(i)) if tags[j][0].strip() == b"5")

    def write(name, new_tags, note):
        path = f"314/{Path(entry['path']).stem}.{name}.dxf"
        (out / path).parent.mkdir(parents=True, exist_ok=True)
        (out / path).write_bytes(b"".join(c + v for c, v in new_tags))
        return {"path": path, "role": "experiment", "issue": 314, "from": entry["path"], "format": "dxf", "note": note}

    def eol(value):
        return value[len(value.rstrip()) :]

    records = []
    first = starts[0]
    end = end_of(first)
    # 1. 寸法の弧の点（10・20）を動かす。部品は元のまま
    moved = list(tags)
    for j in range(first, end):
        code, value = moved[j]
        if code.strip() in (b"10", b"20"):
            moved[j] = (code, f"{float(value) + 20.0!r}".encode() + eol(value))
    note = f"ARC_DIMENSION（ハンドル {handle(first)}）の寸法の弧の点（10・20）を 20 ずつ動かし、部品（2 のブロック）は元のまま"
    records.append(write("exp-moved", moved, note))
    # 2. 部品の名前（2）を除く
    unnamed = [t for j, t in enumerate(tags) if not (first <= j < end and t[0].strip() == b"2")]
    records.append(write("exp-unnamed", unnamed, f"ARC_DIMENSION（ハンドル {handle(first)}）の部品の名前（2）を除く。ブロックは残る"))
    # 3. 利用者の文字（1）を変える
    for i in starts:
        k = next((j for j in range(i, end_of(i)) if tags[j][0].strip() == b"1"), None)
        if k is not None:
            changed = list(tags)
            changed[k] = (tags[k][0], "変更<>".encode("utf-8") + eol(tags[k][1]))
            note = f"ARC_DIMENSION（ハンドル {handle(i)}）の利用者の文字（1）を「変更<>」にし、部品は元のまま"
            records.append(write("exp-text", changed, note))
            break
    return records


def probe_dimension(z, entry, data):
    """#314: 今の apply が寸法の変更を断ることを、応答の診断で残す"""
    doc = f"314/{Path(entry['path']).name}.probe"
    open_file(z, doc, data)
    dims = model(read(z, doc), "dimension")
    if not dims:
        return []
    g = dict(dims[0]["geometry"])
    g["arc"] = [g["arc"][0] + 20, g["arc"][1] + 20]
    status, response = z.call({"op": "apply", "doc": doc, "edits": [{"kind": "replace", "id": dims[0]["id"], "hash": dims[0]["hash"], "geometry": g}]})
    return [{"path": doc, "role": "probe", "issue": 314, "from": entry["path"], "format": "dxf", "status_of_apply": status, "diagnostics": response["diagnostics"]}]


def save_reading(out, path, reading):
    target = out / "zumen" / (path + ".json")
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(json.dumps(reading, ensure_ascii=False, indent=1, sort_keys=True) + "\n", encoding="utf-8")


# README.md（キットの手順と期待する結果）


def num(v):
    v = round(float(v), 6)
    return f"{v:g}" if v != 0 else "0"


def pt(p):
    return f"({num(p[0])}, {num(p[1])})"


def describe(g):
    """形の値を、開いたソフトで確かめる言葉にする"""
    s = g["shape"]
    if s == "point":
        text = f"点 {pt(g['position'])}・角度 {num(g.get('rotation', 0))}°"
        if "marker" in g:
            m = g["marker"]
            text += f"・マーカー {m['namespace']} の {m['code']}（倍率 {num(m['scale'])}）"
        return text
    if s == "line":
        return f"線分 {pt(g['start'])} から {pt(g['end'])}"
    if s == "circle":
        return f"円 中心 {pt(g['center'])}・半径 {num(g['radius'])}"
    if s == "arc":
        return f"円弧 中心 {pt(g['center'])}・半径 {num(g['radius'])}・{num(g['start_angle'])}° から {num(g['end_angle'])}°（反時計回り）"
    if s == "ellipse":
        text = f"楕円 中心 {pt(g['center'])}・長軸の端 {pt(g['major'])}・比 {num(g['ratio'])}"
        if (g["start_angle"], g["end_angle"]) != (0, 360):
            # 図面モデルの楕円の角度は媒介変数の角度（DXF の 41・42）。AutoCAD のプロパティの角度とは違う
            text += f"・媒介変数で {num(g['start_angle'])}° から {num(g['end_angle'])}°（楕円を円に引き延ばしたときの角度）"
        return text
    if s == "polyline":
        bulges = "・膨らみ " + "/".join(num(b) for b in g["bulges"]) if any(g.get("bulges", [])) else ""
        return f"ポリライン {'閉じた' if g['closed'] else '開いた'} " + " ".join(pt(p) for p in g["points"]) + bulges
    if s in ("text", "formatted_text"):
        kind = "文字" if s == "text" else "書式つきの文字"
        text = f"{kind}「{g['content']}」・位置 {pt(g['position'])}・高さ {num(g['height'])}・角度 {num(g.get('rotation', 0))}°"
        if "end" in g:
            # 2 点の間に置く文字（DXF の TEXT の 72 が 3 は両端揃え、5 はフィット）
            text += f"・{pt(g['end'])} までの間に置く（{'両端揃え' if g.get('fit') == 'aligned' else 'フィット'}）"
        if "attachment" in g:
            text += f"・寄せ方 {g['attachment']}"
        if "width" in g:
            text += f"・枠の幅 {num(g['width'])}"
        return text
    if s == "symbol":
        text = f"記号「{g['name']}」・位置 {pt(g['position'])}・尺度 {num(g['scale'][0])}×{num(g['scale'][1])}・角度 {num(g['rotation'])}°"
        if "grid" in g:
            grid = g["grid"]
            text += f"・格子 {grid['columns']} 列 {grid['rows']} 行（間隔 {num(grid['column_spacing'])}・{num(grid['row_spacing'])}）"
        return text
    return s


VIEWERS = {"dxf": "AutoCAD", "dwg": "AutoCAD", "jww": "Jw_cad", "sfc": "SXF ブラウザ", "p21": "SXF ブラウザ"}
# 形式の用語。AutoCAD の「画層」と、CAD製図基準・SXF・Jw_cad の「レイヤ」（docs/style.md）
LAYER_WORDS = {"dxf": "画層", "dwg": "画層"}

INTRO = """# 検証キット

zumen の CLI で、公開サンプル（`samples/`）と適合テストの中で作った図面に、書き戻し（apply）と変換（convert）を当てて書いたファイルです（#503、決定 0254）。`xtask/verify/make.py` が作りました。

- `manifest.json`: ファイルごとの出所・当てた変更・期待する形・診断
- `zumen/`: zumen が読んだ値（`drawing`・`tables`・`entities` の応答）。入力と書いたファイルのそれぞれ
- `ezdxf.json`・`acadsharp.json`・`ezjww.json`・`ezsxf.json`: DXF・DWG・JWW・SFC と P21 を、外部の実装で読んで照らした結果（`check_ezdxf.py` など）。`acadsharp-dump.json` は、ACadSharp で読んだ DWG の値
- `input/`: 入力の図面。書いたファイルを開く前に、同じソフトで入力も開き、入力から変わった所だけを比べる
- `licenses/`: 入力のライセンスの本文。入力ごとのライセンスと表示は、下の「出所とライセンス」と `manifest.json` にある

結果は、対象の issue に、ソフトとバージョン・開けたか・監査の誤りの数・期待と違った所を書きます。確かめたことと、確かめられなかったことを分けて書きます。

## AutoCAD の共通の手順

1. ファイルを開く（DXF は「開く」でファイルの種類を DXF にする）。開けなければ、出た文を記録して次へ進む。
2. `AUDIT` を「いいえ」（直さない）で流し、見つかった誤りの数を記録する。期待は 0。
3. フォルダー `C:/zumen-verify/` を作り、`(load "dump.lsp")` と `(zv-dump "C:/zumen-verify/<ファイルの名前>.txt")` で、図形をハンドルごとに書き出す（`dump.lsp` はキットにある）。下の表のハンドルの行を、期待する値と比べる。角度（50・51）は度に直して書く。楕円の 11（中心から長軸の端への向き）と 41・42（ラジアン）は、ファイルの値のまま書くので、表の「長軸の端」は中心に 11 を足して比べる。
4. 画面でも、下の表の形が見えることを確かめる。

画面なしで流すときは、`accoreconsole.exe /i <ファイル> /s <スクリプト>` に、`(load …)`・`_.AUDIT _N`・`(zv-dump …)`・`_.QUIT _Y` のスクリプトを渡す（`xtask/autocad/run.ps1` と同じ）。
"""

SECTIONS = {
    342: (
        "#342 書き戻した点",
        """点を置き換え・変換・追加して書き戻したファイルです（決定 0183・0190）。

- **AutoCAD（DXF・DWG）。** 共通の手順のあと、点の形が $PDMODE 67（× に四角）・$PDSIZE 4 で描かれることを確かめる。角度のある点のマーカーは、その角度だけ回って描かれる。
- **Jw_cad（JWW）。** 開いて、下の 2 つの点が実寸の座標（mm、用紙の中心が原点）にあることを、「測定」か「属性取得」で確かめる。普通の点の線種が、Jw_cad で打った点と同じ（線種 1、決定 0183 の推定）かを確かめる。記号の点（線種 100）は、番号 3・角度 30°・倍率 2 で描かれる。
- **SXF ブラウザ（SFC・P21）。** 開いて、用紙の座標（mm）の 2 つの点マーカーが、コードの形・回転・赤（既定義色 2）で描かれることを確かめる。SXF ブラウザがなければ、Jw_cad の SXF の読み込みで開いて、同じことを確かめる。
""",
    ),
    156: (
        "#156 書き戻した DWG を AutoCAD で開く",
        """R2000・R2004・R2013・R2018 の DWG と、同じ変更を当てた DXF です。画層を足し、線分・円・円弧・楕円・ポリライン・文字・書式つきの文字（と記号）を足し、角度と寄せ方を持つ形（回した書式つきの文字・2 点の間に置く文字・回した文字・部分の楕円・0° をまたぐ円弧・回した点）を足し、円を置き換えて赤（ACI 1）にし、長方形のポリラインを下へ 20 動かしました（決定 0079〜0104・0155・0162）。

- **AutoCAD。** 共通の手順で、開けること・監査の誤りが 0 であること・下の表の形を確かめる。足した線分は、足した画層 ZUMEN_ADDED（緑、ACI 3）にある。
""",
    ),
    316: (
        "#316 AutoCAD で作ったサンプルが要る値",
        """**格子の記号（MINSERT）。** AutoCAD で作った MINSERT のサンプルに、変換（30° 回し、2 倍にし、X に 50 動かす）・置き換え（Y に 100 動かし、間隔を 1.5 倍にする）・追加（3 列 2 行）を当てて書き戻しました（決定 0161）。

- **AutoCAD。** 共通の手順で、格子の列・行・間隔と、属性（TAGGED の NO・KIND）が記号とともに動いたことを確かめる。

**点の印刷（決定 0159）。** `input/zumen-points-a-r2018.dwg` を、モデルのタブから PDF に印刷する。期待は、画層 0 の点（ハンドル 2DB）と記号 BYBLOCKPOINT（2E5）の中の点が印刷され、印刷しない画層（NOPLOT の 2DC、Defpoints の 2DD）の点が印刷されないこと。zumen は、表示し、実効の画層が印刷するものを印刷するとしている（推定）。

**まだサンプルのない値。** MTEXT の `\\c` と `\\p` の左インデント・揃え、MULTILEADER の線ごとの上書き、文字スタイルの太字・斜体（1071 のビット 24・25）は、`316/controls/README.md` の手順で確かめる。合成の `neutral.dxf` を画面のある AutoCAD で開き、項目ごとに設定して保存した DXF を、`observe.py` で変更前の DXF と照らして、結果を #316 に記録する。保存した図面はリポジトリの外のキットのフォルダーに残し、`samples/dwg` に足すかは記録を受けて決める（足すときは決定 0171 に従う）。
""",
    ),
    314: (
        "#314 寸法の部品を作り直すかを確かめる実験",
        """今の apply は、寸法の変更を `change.edit.unsupported` で断ります（下の probe）。#314 で部品（匿名のブロック）を作り直す前に、部品を書き直さずに寸法を変えた DXF を、AutoCAD がどう描くかを確かめます。どれも zumen の書き手を通さず、`make.py` が入力の DXF のタグを書き換えたものです。

- **AutoCAD。** 入力と 3 つの実験のファイルを開き、寸法の見え方を比べる。それぞれ、(1) 元の部品のまま描くか、(2) 部品を作り直して描くか、(3) 開けないか、と監査の誤りの数を記録する。`exp-unnamed` は、`AUDIT` を「はい」で流したときに部品を作り直すかも記録する。
""",
    ),
    302: (
        "変換した DXF（図面モデルを経た変換、#302・#509）",
        """公開 API で DXF（R2004）に変換しました。R2004 DWG の弧長寸法は、対応する図形と参照がそろえば共有のオブジェクト表から書きます（決定 0281）。ほかは図面モデルを経ます（決定 0206）。書けない要素と、足せなかった画層の要素は、元の場所を付けて `convert.element.unsupported` で落とします。記号の定義の中身も書き、表・ブロック・VIEWPORT の所有者とサブクラス、レイアウトの辞書を R2004 の形で持ちます（#509）。ezdxf では全変換ファイルを読め、監査の誤りと修復が 0 であることを確かめます。

- **AutoCAD。** 全変換ファイルを共通の手順で開き、開けることと `AUDIT` の誤りの数を記録する。ビューポートを持つもの（layout・points・JWW）は、モデルと `Layout1` の両方を確認し、用紙の大きさとビューの位置・尺度を比べる。記号を持つものは、配置だけでなく定義の中身も描かれることを確かめる。Defpoints などの追加できない画層は、その要素が落ちたことを `manifest.json` の変換の診断と照らし、画層 0 に移っていないことを確かめる。結果にはファイル・ハンドル・期待する値・実際の値を記録する。
""",
    ),
    174: (
        "#174 DXF から図面モデルへ移す値",
        """入力の DXF と DWG（AutoCAD で作ったサンプル）を zumen が読んだ値は `zumen/input/` に、ezdxf・ACadSharp と照らした結果は `ezdxf.json`・`acadsharp.json` にあります。

- **AutoCAD。** 共通の手順の 3 で書き出した値を、`zumen/input/<ファイル>.json` の `entities` の `geometry` と比べる。違えば、ハンドル・フィールド・2 つの値を issue に書く。
- **まだサンプルのない推定。** MULTILEADER の色の値（AcCmColor）、HATCH の SPLINE の辺のフィット点（R2010 から）、表のセルの 2007 からの値、アンダーレイの切り抜きの頂点、R2018 の 101 を持つ MTEXT は、AutoCAD で作ったサンプルが要る。作り方は `xtask/autocad/samples.lsp` に主題を足して、`make.sh` で作る。
""",
    ),
}


def readme(out, manifest):
    files = manifest["files"]
    lines = [INTRO]
    lines += [
        "## 出所とライセンス",
        "",
        "`input/` の外の図面は、どれも下の入力を変えたものです（書き戻しと変換は zumen が、#314 の実験は `make.py` が変えた）。ライセンスの本文は `licenses/` にあります。",
        "",
        "| 入力 | ライセンス | 表示 |",
        "| --- | --- | --- |",
    ]
    lines += [f"| `{f['path']}` | {f['license']} | {f['attribution']} |" for f in files if f["role"] == "input" and "license" in f]
    lines.append("")
    for issue, (title, body) in SECTIONS.items():
        lines += [f"## {title}", "", body]
        chosen = [f for f in files if f.get("issue") == issue]
        if issue == 174:
            chosen = [f for f in files if f["role"] == "input" and f["source"].startswith("samples/")]
        if not chosen:
            continue
        lines += ["| ファイル | 開くソフト | 期待する結果 |", "| --- | --- | --- |"]
        for f in chosen:
            lines.append(f"| `{f['path']}` | {VIEWERS.get(f['format'], '')} | {expectation(out, f)} |")
        lines.append("")
    (out / "README.md").write_text("\n".join(lines), encoding="utf-8")


def expectation(out, f):
    """表の 1 行の、期待する結果"""
    if f["role"] == "input":
        return "開けて、監査の誤りが 0。値は zumen の読み（`zumen/` の JSON）と同じ"
    if f["role"] == "experiment":
        return f["note"] + "。見え方を記録する"
    if f["role"] == "probe":
        return "（ファイルなし）apply の応答: " + ", ".join(d["code"] for d in f["diagnostics"])
    if f.get("status") != "ok":
        return "（書けなかった）" + ", ".join(d["code"] for s in f.get("steps", []) for d in s.get("diagnostics", []))
    if f["role"] == "convert":
        dropped = Counter()
        for d in f["diagnostics"]:
            if d["code"] == "convert.element.unsupported":
                dropped[d["params"].get("kind")] += d["count"]
        text = f"開けて、監査の誤りが 0。要素 {f['elements']}"
        if layer := f.get("added_layer"):
            text += f"。追加した画層 {layer} が ACI 3 で、弧長寸法 6 件と描画ブロックの図形 67 件が残る"
        if dropped:
            text += "。落とした要素: " + "・".join(f"{k} {n}" for k, n in sorted(dropped.items()))
        return text
    reading = json.loads((out / "zumen" / (f["path"] + ".json")).read_text(encoding="utf-8"))
    handles = {e["id"]: e.get("source", {}).get("handle") for e in reading["entities"]}
    word = LAYER_WORDS.get(f["format"], "レイヤ")
    items = []
    for step in f["steps"]:
        if step.get("skipped"):
            items.append(f"段 {step['step']} は当てる候補がなく、当てていない")
        for want in step.get("expect", []):
            handle = handles.get(want["id"])
            layer = f"・{word} {want['layer']}" if "layer" in want else ""
            color = want.get("color", {})
            pen = f"・ペン {color['jww']}" if "jww" in color else f"・既定義色 {color['sxf']}" if "sxf" in color else ""
            items.append(f"{'ハンドル ' + handle if handle else want['id']}: {describe(want['geometry'])}{layer}{pen}")
        for edit in step["edits"]:
            if edit["kind"] == "point_style":
                items.append("点の表し方: $PDMODE 67・$PDSIZE 4")
            if edit["kind"] == "add_layer":
                items.append(f"画層 {edit['name']}（ACI {edit['color']['aci']}）")
            if edit["kind"] == "restyle":
                items.append(f"ハンドル {handles.get(edit['id'])} の色が ACI {edit['color']['aci']}")
    # 監査（AUDIT）は AutoCAD の手順にだけある
    opened = "開けて、監査の誤りが 0。" if VIEWERS.get(f["format"]) == "AutoCAD" else "開ける。"
    return opened + "<br>".join(items)


def non_finite_jww(data):
    """適合テストの JWW の線の終点 Y を NaN にする。図形を落とした診断とレコードを結ぶ照合用（#548）"""
    # バージョン 700 の CDataSen は、クラスの名前、15 バイトの共通フィールド、4 つの実数の順。
    # 適合テストの形が変わったら、違うフィールドを書き換える前に止める。
    if data[:12] != b"JwwData." + struct.pack("<I", 700) or data.count(b"CDataSen") != 1:
        raise ValueError("非有限値の検査に使う JWW のバージョンかクラスが変わった")
    at = data.index(b"CDataSen") + len(b"CDataSen") + 15
    if data[at:at + 32] != struct.pack("<4d", 0.0, 0.0, 100.0, 50.0):
        raise ValueError("非有限値の検査に使う JWW の線が変わった")
    return data[:at + 24] + struct.pack("<d", math.nan) + data[at + 32:]


def classes_jww(data):
    """適合テストの JWW 700 の図形リストに、設定・制御・描く文字・ソリッド・寸法を置く（docs/formats/jww.md）。"""
    def tag(name):
        return struct.pack("<3H", 0xffff, 1, len(name)) + name

    def text_body(x, y, text):
        return struct.pack("<4dI4d", x, y, x + 5, y, 0, 3, 3, 0, 0) + bytes([0, len(text)]) + text

    # 元の線を、診断と結ぶレコード 1 にする。先頭のクラスのタグと要素数、後ろの空のブロックと画像のリストを確かめる。
    invalid = non_finite_jww(data)
    line_header = struct.pack("<H", 1) + tag(b"CDataSen")
    begin = invalid.index(line_header)
    fields = begin + len(line_header)
    base = invalid[fields:fields + 15]
    end = fields + 15 + 32
    if invalid[end:] != bytes(6):
        raise ValueError("クラスの照合に使う JWW の図形リストが変わった")

    # CArchive の番号はクラスとオブジェクトで共通。線のクラスと線に続く文字のクラスは 3（docs/formats/jww.md）。
    records = [invalid[begin + 2:end]]
    for x, y, text in [(0, -1000, b"Printer_Orientation = 0"), (0, 0, b"^@XX"), (2, 4, b"TEXT")]:
        header = tag(b"CDataMoji") if len(records) == 1 else struct.pack("<H", 0x8003)
        records.append(header + base + text_body(x, y, text))
    records.append(tag(b"CDataSolid") + base + struct.pack("<8d", 0, 0, 1, 0, 1, 1, 0, 1))
    # 寸法の線・文字・SXF のフィールド（引出線 2 本と点 4 個）。crates/zumen-jww/tests/drawing.rs と同じ配置。
    line = base + struct.pack("<4d", 0, 0, 10, 0)
    dimension = base + line + base + text_body(0, 0, b"10") + struct.pack("<H", 1)
    for x in (0, 10):
        dimension += base + struct.pack("<4d", x, -2, x, 1)
    for x, y in [(0, 0), (10, 0), (0, -2), (10, -2)]:
        dimension += base + struct.pack("<2dI", x, y, 0)
    records.append(tag(b"CDataSunpou") + dimension)
    records.append(struct.pack("<H", 0x8003) + base + text_body(2, 4, b"^@BMimage.png,40,30"))
    return invalid[:begin] + struct.pack("<H", len(records)) + b"".join(records) + invalid[end:]


def inputs(extra):
    """入力の図面: 公開サンプル、適合テストの中で作った図面、--inputs のフォルダー。(道筋, 中身, 出所, ライセンス, 表示)"""
    ledger = tomllib.loads((ROOT / "samples" / "ledger.toml").read_text(encoding="utf-8"))["samples"]
    ledger = {item["path"]: item for item in ledger}
    found = []
    for path in sorted((ROOT / "samples").rglob("*")):
        if path.suffix.lower() in SUFFIXES:
            item = ledger[path.relative_to(ROOT / "samples").as_posix()]
            found.append((f"input/{path.name}", path.read_bytes(), path.relative_to(ROOT).as_posix(), item["license"], item["attribution"]))
    for case, name in CASES:
        steps = json.loads((ROOT / "bindings/conformance/cases" / case).read_text(encoding="utf-8"))["steps"]
        push = next(s["push"] for s in steps if s.get("push", {}).get("name") == name)
        data = bytes.fromhex(push["hex"]) if "hex" in push else push["text"].encode("utf-8")
        found.append((f"input/conformance-{Path(case).stem}-{name}", data, f"bindings/conformance/cases/{case}", *REPOSITORY))
        if name == "a.jww":
            found.append(("input/made-non-finite.jww", non_finite_jww(data), "xtask/verify/make.py", *REPOSITORY))
            found.append(("input/made-classes.jww", classes_jww(data), "xtask/verify/make.py", *REPOSITORY))
    found.append(("input/made-features.sfc", SFC.encode("ascii"), "xtask/verify/make.py", *REPOSITORY))
    found.append(("input/made-duplicates.sfc", duplicate_sfc().encode("ascii"), "xtask/verify/make.py", *REPOSITORY))
    found.append(("input/made-dropped.dxf", dropped_dxf().encode("ascii"), "xtask/verify/make.py", *REPOSITORY))
    found.append(("input/made-shapeless.dxf", shapeless_dxf().encode("ascii"), "xtask/verify/make.py", *REPOSITORY))
    found.append(("input/made-japanese-r2004.dxf", japanese_dxf("AC1018", "cp932"), "xtask/verify/make.py", *REPOSITORY))
    found.append(("input/made-japanese-r2007.dxf", japanese_dxf("AC1021", "utf-8"), "xtask/verify/make.py", *REPOSITORY))
    found.append(("input/made-structure.sfc", STRUCTURE_SFC.encode("ascii"), "xtask/verify/make.py", *REPOSITORY))
    found.append(("input/made-structure.p21", STRUCTURE_P21.encode("ascii"), "xtask/verify/make.py", *REPOSITORY))
    if extra:
        # 受け取った図面のキットは配らないので、ライセンスを書かない
        for path in sorted(Path(extra).rglob("*")):
            if path.suffix.lower() in SUFFIXES:
                found.append((f"input/local-{path.name}", path.read_bytes(), "--inputs", None, None))
    return found


def main():
    parser = argparse.ArgumentParser(description="検証キットを作る")
    parser.add_argument("zumen")
    parser.add_argument("out")
    parser.add_argument("--inputs")
    args = parser.parse_args()
    out = Path(args.out)
    if args.inputs and out.resolve().is_relative_to(ROOT):
        print("受け取った図面のキットは、リポジトリの外に作る", file=sys.stderr)
        return 2
    if out.exists():
        # 誤って作業ツリーを指しても消さないよう、前のキットか空のフォルダーだけを消す
        if any(out.iterdir()) and not (out / "manifest.json").exists():
            print(f"{out} は前のキットでも空でもないので、消さない", file=sys.stderr)
            return 2
        shutil.rmtree(out)
    out.mkdir(parents=True)
    z = Zumen(args.zumen)
    _, capabilities = z.call({"op": "capabilities"})
    files, broken = [], []
    for path, data, source, terms, attribution in inputs(args.inputs):
        fmt = SUFFIXES[Path(path).suffix.lower()]
        (out / path).parent.mkdir(parents=True, exist_ok=True)
        (out / path).write_bytes(data)
        entry = {"path": path, "role": "input", "format": fmt, "source": source, "open": open_file(z, path, data)}
        if terms:
            entry["license"], entry["attribution"] = terms, attribution
        reading = read(z, path)
        save_reading(out, path, reading)
        files.append(entry)
        baseline = codes(entry["open"], reading["diagnostics"])
        # 作った入力が役目を果たすこと: 要素にしない図形のどれもが、移さなかったことを言う診断の場所にある
        for code, want in PLACES.get(path, {}).items():
            places = sum(len(d.get("at", [])) for d in entry["open"] + reading["diagnostics"] if d["code"] == code)
            if places != want:
                broken.append(f"{path}: {code} の場所が {places}（期待は {want}）。黙って捨てない照合を確かめられない")
        if path.startswith("input/made-japanese-"):
            layers = {layer["name"] for layer in reading["tables"]["layers"] if layer.get("defined")}
            if layers != {"0", "日本語"} or [e.get("layer") for e in reading["entities"]] != ["日本語"]:
                broken.append(f"{path}: 日本語の画層とその上の線を読めない。文字コードによる照合を確かめられない")
        if path == "input/made-shapeless.dxf":
            shapeless = Counter(e["kind"] for e in reading["entities"] if "geometry" not in e)
            if shapeless != {"hatch": 3, "underlay": 2, "table": 1} or len(reading["entities"]) != 6:
                broken.append(f"{path}: 形を返さないハッチング・画像の範囲・表の要素を読めない。要素の種類の照合を確かめられない")
        if path in CHECKS:
            continue
        for recipe in RECIPES:
            if recipe[2](Path(path).name, fmt):
                files.append(run_recipe(z, out, entry, data, recipe, baseline))
        files.append(run_convert(z, out, entry, data, baseline))
        if source == "samples/dwg/zumen-arcdim-r2004.dwg":
            for layer in ("Walls", "外壁"):
                files.append(run_convert(z, out, entry, data, baseline, layer))
        # 文字を UTF-8 で書けるのは 2007 からの DXF
        if fmt == "dxf" and "-arcdim-r2018" in path:
            files += dimension_experiments(out, entry, data)
            files += probe_dimension(z, entry, data)
    z.close()
    manifest = {"version": 1, "capabilities": capabilities["result"], "local": bool(args.inputs), "files": files}
    (out / "manifest.json").write_text(json.dumps(manifest, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
    readme(out, manifest)
    shutil.copy(Path(__file__).with_name("dump.lsp"), out / "dump.lsp")
    # キットは公開サンプルと適合テストの図面の写しと、それを変えたファイルを配るので、ライセンスの本文を写す
    (out / "licenses").mkdir()
    for path in [*sorted((ROOT / "licenses").iterdir()), ROOT / "LICENSE-MIT", ROOT / "LICENSE-APACHE"]:
        shutil.copy(path, out / "licenses" / path.name)
    # 当たらなかった変更と変換は、キットを書き終えてから失敗にする（前は当たっていたものが当たらなくなったら知らせる）
    failed = [f for f in files if f.get("status") not in (None, "ok")]
    for f in failed:
        print(f"{f['path']}: {f.get('status')} {[d['code'] for s in f.get('steps', []) for d in s.get('diagnostics', [])] or f.get('diagnostics')}", file=sys.stderr)
    reread = [f for f in files if f.get("reread")]
    for f in reread:
        print(f"{f['path']}: zumen が読み直すと、入力より件数の増えた診断が出た: {f['reread']}", file=sys.stderr)
    for message in broken:
        print(message, file=sys.stderr)
    print(f"{len(files)} のファイル（当たらなかった組 {len(failed)}、読み直すと診断が増えた組 {len(reread)}）を {out} に置いた")
    return 1 if failed or reread or broken else 0


if __name__ == "__main__":
    sys.exit(main())
