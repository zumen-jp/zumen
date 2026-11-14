"""check_autocad.py の、AutoCAD なしで確かめられる所を確かめる。対照の図面とスクリプトの組み立て、コンソールの印と
監査の数の読み、dump.lsp の書き出しの読み、図面モデルの形への直し、寸法の部品の比べ方。

使い方: python -I -m unittest discover -s xtask/verify -p 'test_check_autocad.py'

毎晩の CI には AutoCAD がないので、check_autocad.py の読みは、このテストで保つ。コンソールの出力と書き出しは、
このテストが作る行だけを使う（AutoCAD の文を写さない）。
"""

import sys
import tempfile
import unittest
from collections import Counter
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))
import check_autocad as c  # noqa: E402
from compare import applied, differences  # noqa: E402

# 書き出しの行。タブの代わりに「|」で書く
ROWS = (
    "@LAYER|10|0|6=Continuous|62=7|70=0|290=1|370=-3",
    "@LAYER|40|HIDDEN|6=DASH|62=-3|70=5|290=0",
    "@LTYPE|16|Continuous|3=Solid line|40=0.000000000000|72=65|73=0",
    "@LTYPE|41|DASH|3=dash __ __|40=8.000000000000|72=65|73=2|49=6.000000000000|74=0|49=-2.000000000000|74=0",
    "@STYLE|11|Standard|3=arial.ttf|4=|40=0.000000000000|41=1.000000000000|42=2.500000000000|50=0.000000000000|70=0|71=0",
    "@STYLE|42|NARROW|3=txt|4=bigfont|40=2.500000000000|41=0.800000000000|42=2.500000000000|50=15.000000000000|70=4|71=6",
    "30|LINE|0|10=0.000000000000,0.000000000000|11=10.000000000000,5.000000000000",
    "31|TEXT|HIDDEN|1=a^Ib^ c|6=DASH|7=NARROW|10=1.0,2.0|11=3.0,4.0|40=2.5|41=0.8|50=0.0|51=15.0|62=3|71=2|72=1|73=2",
    "32|INSERT|0|2=PART|10=5.0,5.0|41=2.0|42=3.0|44=4.0|45=6.0|50=90.0|66=1|70=3|71=2",
    "33|ATTRIB|0|1=value|2=TAG|10=5.0,5.0|40=2.5",
    "34|SEQEND|0",
    "@BLOCK|50|PART|10=0.0,0.0|70=2",
    "51|CIRCLE|0|10=0.0,0.0|40=1.5",
)
# コンソールの出力。スクリプトの行の表示（印の文字が行の初めにない）、印、進みの表示（復帰で同じ行を書き直す）、
# 監査の結果の行（2 つの整数で終わる）
CONSOLE = "\r\n".join(
    [
        'echo (progn (princ "\\n@@zv open") (princ))',
        "@@zv open drawing.dxf",
        "@@zv dbmod 0",
        "@@zv version 26.0 X.1.0.0 (TEST)",
        "@@zv dumped",
        "@@zv audit-begin",
        "pass 1 10\rpass 1 100",
        "errors 2 fixed 0",
        "@@zv audit-end",
        "@@zv done",
        "",
    ]
)


def dump(*rows):
    """書き出しの行から、読みを作る。終わりの行に、図形の行の数を入れる"""
    count = sum(1 for row in rows if not row.startswith("@"))
    lines = ["PDMODE=34|PDSIZE=5.000000000000", *rows, f"@END|{count}"]
    return c.Dump("\n".join(line.replace("|", "\t") for line in lines) + "\n")


def geometry(row):
    """1 つの図形の行の、図面モデルの形"""
    return c.geometry(dump(row).rows[0])


class Script(unittest.TestCase):
    def test_the_control_drawings_are_code_and_value_pairs(self):
        for circle in ("", c.ZERO_NORMAL):
            lines = c.control(circle).decode("ascii").splitlines()
            self.assertEqual(len(lines) % 2, 0)
            codes = [int(code) for code in lines[::2]]
            self.assertEqual((lines[:2], lines[-2:]), (["0", "SECTION"], ["0", "EOF"]))
            # 誤りの出る図面だけが、円の押し出しの向きを持つ
            self.assertEqual(210 in codes, bool(circle))

    def test_the_script_has_no_empty_line_and_never_saves(self):
        for fix in (False, True):
            text = c.script(fix)
            self.assertTrue(text.isascii())
            self.assertTrue(text.endswith("_.QUIT _Y\r\n"))
            lines = text.split("\r\n")[:-1]
            # 空の行は、前のコマンドの繰り返しになる
            self.assertTrue(all(line.strip() for line in lines))
            self.assertEqual((lines.count("_.AUDIT _N"), lines.count("_.AUDIT _Y")), (1, int(fix)))
            self.assertEqual(sum("(zv-dump " in line for line in lines), 1 + int(fix))
            self.assertFalse(any(word in text.upper() for word in ("SAVE", "WBLOCK", "DXFOUT", "EXPORT")))

    def test_lisp_lines_are_short_ascii_without_comments(self):
        with tempfile.TemporaryDirectory() as folder:
            source = Path(folder) / "dump.lsp"
            with mock.patch.object(c, "HERE", Path(folder)):
                source.write_text(";; 注記\n\n(princ 1)\n  ; 注記\n(princ 2)\n", encoding="utf-8")
                self.assertEqual(c.lisp(), ["(princ 1)", "(princ 2)"])
                for body in ('(princ "図面")', '(princ "' + "x" * 200 + '")'):
                    source.write_text(body + "\n", encoding="utf-8")
                    with self.assertRaises(ValueError):
                        c.lisp()


class Console(unittest.TestCase):
    def test_the_output_is_decoded_from_utf16(self):
        self.assertEqual(c.decode(CONSOLE.encode("utf-16-le")), CONSOLE)
        self.assertEqual(c.decode(b"@@zv open drawing.dxf\r\n"), "@@zv open drawing.dxf\r\n")

    def test_marks_are_read_from_the_start_of_a_line(self):
        marks = [("open", "drawing.dxf"), ("dbmod", "0"), ("version", "26.0 X.1.0.0 (TEST)")]
        marks += [(name, "") for name in ("dumped", "audit-begin", "audit-end", "done")]
        self.assertEqual(c.markers(CONSOLE), marks)

    def test_the_audit_counts_are_the_last_line_ending_with_two_integers(self):
        self.assertEqual(c.audit_counts(CONSOLE, "audit-begin", "audit-end"), (2, 0))
        self.assertEqual(c.audit_counts(CONSOLE.replace("errors 2 fixed 0", "errors 3 fixed 3"), "audit-begin", "audit-end"), (3, 3))
        # 直した数が見つけた数を超える行は、結果の行でない
        self.assertIsNone(c.audit_counts(CONSOLE.replace("errors 2 fixed 0", "errors 2 fixed 3"), "audit-begin", "audit-end"))
        # 結果の行がない出力と、印のない出力は、0 と読まない
        bare = CONSOLE.replace("pass 1 10\rpass 1 100\r\n", "").replace("errors 2 fixed 0", "nothing to report")
        self.assertIsNone(c.audit_counts(bare, "audit-begin", "audit-end"))
        self.assertIsNone(c.audit_counts(CONSOLE, "fix-begin", "fix-end"))
        self.assertIsNone(c.audit_counts(CONSOLE.replace("@@zv audit-end\r\n", ""), "audit-begin", "audit-end"))


class Reading(unittest.TestCase):
    def test_rows_tables_blocks_and_attributes(self):
        d = dump(*ROWS)
        self.assertTrue(d.complete)
        self.assertEqual(d.header, (34, 5.0))
        tables = {"LAYER": ["0", "HIDDEN"], "LTYPE": ["Continuous", "DASH"], "STYLE": ["NARROW", "Standard"]}
        self.assertEqual({name: sorted(rows) for name, rows in d.tables.items()}, tables)
        self.assertEqual([row.handle for row in d.rows], ["30", "31", "32", "33", "34"])
        self.assertEqual({name: (block["handle"], [row.handle for row in block["rows"]]) for name, block in d.blocks.items()}, {"PART": ("50", ["51"])})
        # 属性は、属性を持つ記号の配置の後ろに並ぶ。定義の中の図形は、ブロックの名前を持つ
        self.assertEqual([row.handle for row in d.by_handle["32"].attributes], ["33"])
        self.assertEqual((d.by_handle["30"].block, d.by_handle["51"].block), (None, "PART"))

    def test_a_dump_without_its_last_line_is_incomplete(self):
        line = "PDMODE=0\tPDSIZE=0.0\n30\tLINE\t0\t10=0.0,0.0\t11=1.0,1.0\n"
        self.assertTrue(c.Dump(line + "@END\t1\n").complete)
        self.assertFalse(c.Dump(line).complete)
        self.assertFalse(c.Dump(line + "@END\t2\n").complete)

    def test_a_dump_file_that_cannot_be_read_is_none(self):
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / "before.txt"
            self.assertIsNone(c.load_dump(path))
            path.write_text("PDMODE=x\tPDSIZE=0.0\n", encoding="utf-8", newline="")
            self.assertIsNone(c.load_dump(path))
            path.write_text("PDMODE=0\tPDSIZE=0.0\n@LAYER\t10\t図面\t62=7\n@END\t0\n", encoding="utf-8", newline="")
            self.assertEqual(set(c.load_dump(path).tables["LAYER"]), {"図面"})

    def test_the_view_gives_the_tables(self):
        view = c.view(dump(*ROWS), "dxf")
        self.assertEqual((view["tool"], view["layers"], view["header"]), ("AutoCAD", {"0", "HIDDEN"}, (34, 5.0)))
        self.assertEqual(view["layer_colors"], {"0": 7, "hidden": 3})
        self.assertEqual(view["layer_line_types"], {"0": "Continuous", "hidden": "DASH"})
        # 62 が負なら非表示、70 の 1 は凍結、4 はロック、290 が 0 なら印刷しない
        self.assertEqual(view["layer_states"]["0"], {"visible": True, "frozen": False, "editable": True, "printable": True})
        self.assertEqual(view["layer_states"]["hidden"], {"visible": False, "frozen": True, "editable": False, "printable": False})
        self.assertEqual(view["line_types"]["dash"], {"description": "dash __ __", "pattern": [6.0, -2.0]})
        style = {"font": "txt", "big_font": "bigfont", "height": 2.5, "width": 0.8, "oblique": 15.0, "backward": True, "upside_down": True, "vertical": True}
        self.assertEqual(view["text_styles"]["narrow"], style)
        # 文字の幅から決め直す値を除くのは、DXF を読み込んだときだけ
        self.assertIs(view["derived"], c.derived)
        self.assertIsNone(c.view(dump(*ROWS), "dwg")["derived"])

    def test_the_view_gives_entities_by_handle(self):
        view = c.view(dump(*ROWS), "dwg")
        self.assertIsNone(view["entity"]("99"))
        line = {"shape": "line", "start": [0.0, 0.0], "end": [10.0, 5.0]}
        # 62 と 6 がなければ、画層に従う色（256）と線種（None）
        self.assertEqual(view["entity"]("30"), {"type": "LINE", "layer": "0", "color": 256, "line_type": None, "owner": None, "geometry": line})
        text = view["entity"]("31")
        self.assertEqual((text["layer"], text["color"], text["line_type"]), ("HIDDEN", 3, "DASH"))
        self.assertEqual(view["entity"]("51")["owner"], "PART")


class Geometry(unittest.TestCase):
    def test_text_omits_the_default_settings(self):
        plain = geometry("31|TEXT|0|1=abc|7=Standard|10=1.0,2.0|11=0.0,0.0|40=2.5|41=1.0|50=30.0|51=0.0|71=0|72=0|73=0")
        self.assertEqual(plain, {"shape": "text", "content": "abc", "height": 2.5, "rotation": 30.0, "style": "Standard", "position": [1.0, 2.0]})
        # 寄せた文字は 11 に置く。文字は、DXF の書き方（^I と「^ 」）のまま
        placed = c.geometry(dump(*ROWS).by_handle["31"])
        expected = {"shape": "text", "content": "a^Ib^ c", "height": 2.5, "rotation": 0.0, "style": "NARROW", "position": [3.0, 4.0]}
        expected |= {"horizontal": "center", "vertical": "middle", "width_factor": 0.8, "oblique": 15.0, "backward": True}
        self.assertEqual(placed, expected)
        self.assertEqual(c.derived(placed), set())
        flipped = geometry("31|TEXT|0|1=abc|10=1.0,2.0|40=2.5|71=4")
        self.assertEqual((flipped["upside_down"], "backward" in flipped, flipped["style"]), (True, False, "Standard"))

    def test_text_between_two_points_has_no_alignment(self):
        for code, fit, key in ((3, "aligned", "height"), (5, "fit", "width_factor")):
            g = geometry(f"31|TEXT|0|1=abc|7=Standard|10=0.0,0.0|11=10.0,10.0|40=2.5|41=0.5|50=0.0|72={code}|73=0")
            self.assertEqual((g["position"], g["end"], g["fit"], g["width_factor"]), ([0.0, 0.0], [10.0, 10.0], fit, 0.5))
            self.assertAlmostEqual(g["rotation"], 45.0)
            self.assertFalse({"horizontal", "vertical"} & set(g))
            # AutoCAD が DXF を読み込むときに決め直す値
            self.assertEqual(c.derived(g), {key})
        self.assertIsNone(geometry("31|TEXT|0|1=abc|10=1.0,1.0|11=1.0,1.0|40=2.5|72=3"))
        self.assertEqual(c.derived({"shape": "line"}), set())

    def test_polylines_have_bulges_only_when_curved(self):
        straight = geometry("35|LWPOLYLINE|0|90=3|70=1|43=0.0|10=0.0,0.0|40=0.0|41=0.0|42=0.0|10=4.0,0.0|40=0.0|41=0.0|42=0.0|10=4.0,3.0|40=0.0|41=0.0|42=0.0")
        self.assertEqual(straight, {"shape": "polyline", "points": [[0.0, 0.0], [4.0, 0.0], [4.0, 3.0]], "closed": True})
        curved = geometry("35|LWPOLYLINE|0|90=2|70=0|10=0.0,0.0|42=0.5|10=4.0,0.0|42=0.0")
        self.assertEqual(curved, {"shape": "polyline", "points": [[0.0, 0.0], [4.0, 0.0]], "closed": False, "bulges": [0.5, 0.0]})
        # 幅を持つ折れ線は比べない
        self.assertIsNone(geometry("35|LWPOLYLINE|0|90=2|70=0|43=0.5|10=0.0,0.0|10=4.0,0.0"))
        self.assertIsNone(geometry("35|LWPOLYLINE|0|90=2|70=0|10=0.0,0.0|40=0.0|41=1.0|10=4.0,0.0"))

    def test_curves_points_and_formatted_text(self):
        self.assertEqual(geometry("36|CIRCLE|0|10=1.0,2.0|40=3.0"), {"shape": "circle", "center": [1.0, 2.0], "radius": 3.0})
        arc = {"shape": "arc", "center": [1.0, 2.0], "radius": 3.0, "start_angle": 30.0, "end_angle": 120.0}
        self.assertEqual(geometry("36|ARC|0|10=1.0,2.0|40=3.0|50=30.0|51=120.0"), arc)
        # 楕円の 11 は中心から長軸の端への向き、41・42 は媒介変数（ラジアン）
        ellipse = geometry("36|ELLIPSE|0|10=1.0,2.0|11=4.0,0.0|40=0.5|41=0.0|42=3.141592653590")
        self.assertEqual((ellipse["major"], ellipse["ratio"], ellipse["start_angle"]), ([5.0, 2.0], 0.5, 0.0))
        self.assertAlmostEqual(ellipse["end_angle"], 180.0)
        self.assertEqual(geometry("36|POINT|0|10=1.0,2.0"), {"shape": "point", "position": [1.0, 2.0], "rotation": 0.0})
        # 長い文字は 3 に分けて並び、終わりが 1。41 が 0 なら枠の幅を持たない
        text = geometry("36|MTEXT|0|3=first |3=second |1=last^J|7=NARROW|10=1.0,2.0|40=2.5|41=0.0|50=90.0|71=5")
        expected = {"shape": "formatted_text", "content": "first second last^J", "position": [1.0, 2.0], "height": 2.5, "rotation": 90.0}
        self.assertEqual(text, expected | {"attachment": "middle_center", "style": "NARROW"})
        self.assertEqual(geometry("36|MTEXT|0|1=a|10=1.0,2.0|40=2.5|41=20.0|71=1")["width"], 20.0)
        self.assertIsNone(geometry("36|HATCH|0|2=SOLID"))

    def test_symbols_have_a_grid_and_attributes(self):
        symbol = c.geometry(dump(*ROWS).by_handle["32"])
        expected = {"shape": "symbol", "name": "PART", "position": [5.0, 5.0], "scale": [2.0, 3.0], "rotation": 90.0}
        expected |= {"grid": {"columns": 3, "rows": 2, "column_spacing": 4.0, "row_spacing": 6.0}, "attributes": [{"tag": "TAG", "value": "value"}]}
        self.assertEqual(symbol, expected)
        self.assertEqual(geometry("32|INSERT|0|2=PART|10=5.0,5.0"), {"shape": "symbol", "name": "PART", "position": [5.0, 5.0], "scale": [1.0, 1.0], "rotation": 0.0})


class Handles(unittest.TestCase):
    def test_only_drawings_from_r13_keep_their_handles(self):
        def dxf(version):
            return f"0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\n{version}\n0\nENDSEC\n0\nEOF\n".encode("ascii")

        self.assertTrue(c.keeps_handles(dxf("AC1015"), ".dxf"))
        self.assertTrue(c.keeps_handles(dxf("AC1012"), ".DXF"))
        self.assertFalse(c.keeps_handles(dxf("AC1009"), ".dxf"))
        # 見出しのない DXF は、AutoCAD がハンドルを振り直す
        self.assertFalse(c.keeps_handles(b"0\nSECTION\n2\nENTITIES\n0\nENDSEC\n0\nEOF\n", ".dxf"))
        self.assertTrue(c.keeps_handles(b"AC1015", ".dwg"))
        self.assertTrue(c.keeps_handles(b"AutoCAD Binary DXF\r\n\x1a\0", ".dxf"))


class Defaults(unittest.TestCase):
    """既定の値のときに書かないフィールドの比べ方（compare.py の DEFAULTS）"""

    def test_a_value_as_close_as_the_default_is_the_omitted_field(self):
        # 計算で終わりの桁が揺れた幅の倍率を、AutoCAD の読みは小数 12 桁で書き出すので、既定の 1 として省く
        computed = {"shape": "text", "content": "abc", "width_factor": 1.0000000000000002, "oblique": 1e-13}
        plain = {"shape": "text", "content": "abc"}
        self.assertEqual(differences(computed, plain, both=True), [])
        self.assertEqual(differences(plain, computed, both=True), [])

    def test_a_value_away_from_the_default_is_a_difference(self):
        narrow = {"shape": "text", "content": "abc", "width_factor": 0.8}
        plain = {"shape": "text", "content": "abc"}
        self.assertEqual(differences(narrow, plain, both=True), ["/width_factor: ない（期待は 0.8）"])
        self.assertEqual(differences(plain, narrow, both=True), ["/width_factor: zumen の読みにない（外部の実装は 0.8）"])
        # 既定の値を持たないフィールドは、値によらず食い違いにする
        self.assertEqual(differences({"height": 0.0}, {}), ["/height: ない（期待は 0.0）"])
        self.assertEqual(differences({"width_factor": True}, {}), ["/width_factor: ない（期待は True）"])


class Dimensions(unittest.TestCase):
    """#314 の実験: 寸法の部品（匿名のブロック）の変わり方"""

    BEFORE = ("60|DIMENSION|0|2=*D1|10=0.0,5.0", "@BLOCK|70|*D1", "71|LINE|0|10=0.0,0.0|11=10.0,0.0", "72|MTEXT|0|1=10|10=5.0,1.0|40=2.5|42=0.0|43=0.0")

    def changed(self, *rows):
        return c.observe(dump(*self.BEFORE), dump(*rows))

    def test_unchanged_parts_are_not_reported(self):
        # MTEXT の 42・43 は、AutoCAD が描いたときに入れる
        rows = (*self.BEFORE[:3], "72|MTEXT|0|1=10|10=5.0,1.0|40=2.5|42=3.2|43=2.5")
        self.assertEqual(self.changed(*rows), {"dimensions": 1, "blocks": 1, "changed": {}, "missing": []})

    def test_renamed_and_rebuilt_parts_are_told_apart(self):
        renamed = self.changed("60|DIMENSION|0|2=*D2|10=0.0,5.0", *self.BEFORE[1:], "@BLOCK|80|*D2", "81|LINE|0|10=0.0,0.0|11=10.0,0.0", "82|MTEXT|0|1=10|10=5.0,1.0|40=2.5")
        self.assertEqual((renamed["blocks"], list(renamed["changed"])), (2, ["60"]))
        self.assertIn("*D1 から *D2", renamed["changed"]["60"])
        self.assertIn("形は同じ", renamed["changed"]["60"])
        rebuilt = self.changed(*self.BEFORE[:2], "91|LINE|0|10=0.0,0.0|11=10.0,0.0", "92|MTEXT|0|1=10|10=5.0,1.0|40=2.5|42=0.0|43=0.0")
        self.assertIn("作り直した（形は同じ）", rebuilt["changed"]["60"])
        moved = self.changed(*self.BEFORE[:2], "71|LINE|0|10=0.0,0.0|11=12.0,0.0", self.BEFORE[3])
        self.assertIn("値が変わった", moved["changed"]["60"])
        self.assertEqual(self.changed("@BLOCK|70|*D1", *self.BEFORE[2:])["missing"], ["60"])

    def test_rebuilt_dimension_and_table_parts_are_checked_in_applied(self):
        d = dump(
            "60|DIMENSION|0|1=L=<>mm|2=*D9|10=100.0,20.0|11=57.5,25.0|42=115.0",
            "61|ACAD_TABLE|0|2=*T2|10=5.0,-65.0|11=1.0,0.0",
            "@BLOCK|70|*D9",
            "71|LINE|0|10=0.0,0.0|11=0.0,25.0",
            "72|LINE|0|10=115.0,0.0|11=115.0,25.0",
            "73|LINE|0|10=0.0,25.0|11=115.0,25.0",
            "74|MTEXT|0|1=L=115mm|10=57.5,25.0|40=2.5|50=0.0|71=5",
            "@BLOCK|80|*T2",
            "81|LINE|0|10=5.0,-65.0|11=132.0,-65.0",
            "82|LINE|0|10=5.0,-94.0|11=132.0,-94.0",
            "83|MTEXT|0|1=\\U+54C1A|10=10.0,-70.0|40=3.5|50=0.0|71=5",
        )
        entry = {
            "steps": [
                {
                    "step": "parts",
                    "edits": [],
                    "parts": [
                        {
                            "id": "space/0/h60",
                            "edit": 0,
                            "kind": "dimension",
                            "prefix": "*D",
                            "measurement": 115.0,
                            "override": "L=<>mm",
                            "curves": 3,
                            "mtext": "L=115mm",
                        },
                        {
                            "id": "space/0/h61",
                            "edit": 1,
                            "kind": "table",
                            "prefix": "*T",
                            "position": [5.0, -65.0],
                            "direction": [1.0, 0.0],
                            "lines": 2,
                            "texts": ["品A"],
                            "rotation": 0.0,
                        },
                    ],
                }
            ]
        }
        reading = {
            "entities": [
                {"id": "space/0/h60", "source": {"handle": "60"}, "geometry": {"shape": "dimension", "text_position": [57.5, 25.0]}},
                {"id": "space/0/h61", "source": {"handle": "61"}, "geometry": {"shape": "table"}},
            ]
        }
        counts = Counter()
        self.assertEqual(applied(entry, reading, counts, c.view(d, "dxf")), [])
        self.assertEqual(counts["expected"], 2)
        # 測定値と部品の文字のずれを見つける
        wrong_entry = {
            "steps": [
                {
                    "step": "parts",
                    "edits": [],
                    "parts": [{**entry["steps"][0]["parts"][0], "measurement": 100.0, "mtext": "L=100mm"}],
                }
            ]
        }
        failures = applied(wrong_entry, reading, Counter(), c.view(d, "dxf"))
        self.assertEqual(len(failures), 2)


class Chains(unittest.TestCase):
    """変更を 1 つずつ続けて当てたファイルの、列を通して決まる終わりの値の照らし方（compare.py の applied）"""

    LINE = {"shape": "line", "start": [0, 0], "end": [10, 5]}

    def failures(self, entities, drawn):
        # entities は zumen の読みのハンドル、drawn は外部の実装の読み（ハンドル → 図形）
        entry = {
            "steps": [{"step": "styled", "edits": [{"kind": "restyle", "id": "space/0/h31", "hash": "", "color": {"aci": 2}}]}],
            "final": {
                "expect": [{"id": "space/0/h30", "geometry": self.LINE, "layer": "A", "color": {"aci": 1}}],
                "removed": [{"id": "space/0/h31", "handle": "31"}],
                "reused": [],
            },
        }
        reading = {"entities": [{"id": f"space/0/h{handle}", "source": {"handle": handle}} for handle in entities]}
        view = {"tool": "tool", "entity": drawn.get, "layer_colors": {}, "header": (0, 0)}
        return applied(entry, reading, Counter(), view)

    def test_the_last_values_and_the_removed_handles_are_compared(self):
        kept = {"type": "LINE", "layer": "a", "color": 1, "geometry": self.LINE}
        # 様式を変えてから除いた要素（31）は、様式を照らさない
        self.assertEqual(self.failures(["30"], {"30": kept}), [])
        moved = {**kept, "color": 3, "geometry": {**self.LINE, "end": [10, 6]}}
        self.assertEqual(
            self.failures(["30"], {"30": moved}),
            ["列の終わり（space/0/h30）/end/1: 6（期待は 5）", "列の終わり（space/0/h30）の色が ACI 3（期待は 1）"],
        )

    def test_a_removed_element_that_is_still_drawn_is_a_difference(self):
        kept = {"type": "LINE", "layer": "A", "color": 1, "geometry": self.LINE}
        self.assertEqual(
            self.failures(["30", "31"], {"30": kept, "31": kept}),
            ["除いた要素 space/0/h31 を、書き戻したファイルで zumen が読む", "除いた要素 space/0/h31 のハンドル 31 の図形が tool にある"],
        )


class Results(unittest.TestCase):
    def test_a_drawing_that_did_not_finish_says_how_far_it_got(self):
        # 開けなかった図面は印を出さず、時間切れで止めた図面は終了コードを持たない
        self.assertEqual(c.progress({"exit": 53, "stage": None}), "終了コード 53、進んだ所 なし")
        self.assertEqual(c.progress({"exit": None, "stage": "dumped"}), "終了コード なし、進んだ所 dumped")
        self.assertEqual(c.summary({"status": "timeout", "exit": None, "stage": "open"}), {"status": "timeout", "exit": None, "stage": "open"})


if __name__ == "__main__":
    unittest.main()
