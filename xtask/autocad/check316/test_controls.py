"""#316 の比較が合成 DXF の値を読み、対照や対象外の書式を取り違えないことを確かめる。"""

import hashlib
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

import ezdxf
from ezdxf.lldxf.tagger import ascii_tags_loader
from ezdxf.render.arrows import ARROWS
from ezdxf.tools.text import MTextEditor, MTextParagraphAlignment, ParagraphProperties

from make import make
from observe import compare, inline, read


def with_line_fields(source, dest, changes):
    # ezdxf 1.4.5 は線の 93 を書かない。対象の LEADER_LINE の中だけに生のコードを置く。
    with source.open(encoding="utf-8") as stream:
        tags = list(ascii_tags_loader(stream))
    start = None
    result = []
    for code, value in tags:
        if (code, value) == (304, "LEADER_LINE{"):
            start = len(result)
        elif start is not None and (code, value) == (305, "}"):
            x = next(float(v) for c, v in result[start:] if c == 10)
            fields = changes.get(x, {})
            result[start:] = [(c, v) for c, v in result[start:] if c not in fields]
            result.extend(fields.items())
            start = None
        result.append((code, value))
    with dest.open("w", encoding="utf-8", newline="\n") as stream:
        for code, value in result:
            stream.write(f"{code}\n{value}\n")


class Controls(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        temp = tempfile.TemporaryDirectory()
        cls.addClassCleanup(temp.cleanup)
        cls.folder = Path(temp.name)
        cls.kit = cls.folder / "controls"
        make(cls.kit)
        cls.neutral = cls.kit / "neutral.dxf"

    def setUp(self):
        self.doc = ezdxf.readfile(self.neutral)
        self.edited = self.folder / "edited.dxf"
        self.text = {e.dxf.layer: e for e in self.doc.modelspace().query("MTEXT")}

    def check(self, topic):
        return compare(read(self.neutral, topic), read(self.edited, topic), topic)

    def test_kit(self):
        audit = self.doc.audit()
        self.assertEqual((len(audit.errors), len(audit.fixes)), (0, 0))
        self.assertEqual((self.doc.dxfversion, self.doc.units), ("AC1032", 4))
        self.assertEqual(len(self.text), 5)
        self.assertEqual({e.dxf.style for e in self.doc.modelspace().query("TEXT")},
                         {"Z316_REGULAR", "Z316_BOLD", "Z316_ITALIC", "Z316_BOTH"})
        branches = self.doc.modelspace().query("MULTILEADER")[0].context.leaders[0].lines
        self.assertEqual([(b.index, b.vertices[0].xy) for b in branches], [(0, (-30, 0)), (1, (30, 0))])
        metadata = json.loads((self.kit / "source.json").read_text(encoding="utf-8"))
        self.assertEqual(metadata["sha256"], hashlib.sha256(self.neutral.read_bytes()).hexdigest())
        self.assertTrue(metadata["synthetic"])
        self.assertEqual({p.name for p in self.kit.iterdir()}, {"README.md", "make.py", "observe.py", "neutral.dxf", "source.json"})
        another = self.folder / "second"
        make(another)
        self.assertEqual(self.neutral.read_bytes(), (another / "neutral.dxf").read_bytes())
        with self.assertRaises(FileExistsError):
            make(self.kit)

    def test_unchanged(self):
        self.edited = self.neutral
        for topic in ("styles", "color", "paragraph", "leader-color"):
            with self.subTest(topic=topic):
                self.assertFalse(self.check(topic)["matches"])

    def test_styles(self):
        for suffix, italic, bold in (("BOLD", False, True), ("ITALIC", True, False), ("BOTH", True, True)):
            self.doc.styles.get("Z316_" + suffix).set_extended_font_data("Arial", italic=italic, bold=bold)
        self.doc.appids.new("Z316_OTHER")
        self.doc.styles.get("Z316_BOLD").set_xdata("Z316_OTHER", [(1071, 0)])
        self.doc.saveas(self.edited)
        result = self.check("styles")
        self.assertTrue(result["matches"])
        self.assertEqual(result["before"], 0x22)
        self.assertEqual(result["after"], {"REGULAR": 0x22, "BOLD": 0x02000022, "ITALIC": 0x01000022, "BOTH": 0x03000022})
        self.doc.styles.get("Z316_ITALIC").set_extended_font_data("Arial", bold=True)
        self.doc.saveas(self.edited)
        self.assertFalse(self.check("styles")["matches"])

    def test_color(self):
        for a, b, order in ((0x332211, 0x112233, "BGR"), (0x112233, 0x332211, "RGB"), (0x112233, 0x112233, None)):
            for suffix, value in (("A", a), ("B", b)):
                name = "Z316_RGB_" + suffix
                self.text[name].text = "{\\c%d;%s}" % (value, name)
            self.doc.saveas(self.edited)
            result = self.check("color")
            self.assertEqual((result["high_to_low_bytes"], result["matches"]), (order, order is not None))
        self.text["Z316_RGB_A"].dxf.true_color = 0x112233
        self.doc.saveas(self.edited)
        with self.assertRaisesRegex(ValueError, "図形全体の色"):
            self.check("color")

    def test_paragraph(self):
        for left_a, left_b, units in ((2, 1, "text_height"), (5, 5, "drawing_units"), (5, 1, None)):
            for name, left in (("Z316_PARA_25", left_a), ("Z316_PARA_50", left_b)):
                props = ParagraphProperties(left=left, align=MTextParagraphAlignment.CENTER)
                self.text[name].text = str(MTextEditor().paragraph(props).append(name))
            self.doc.saveas(self.edited)
            result = self.check("paragraph")
            self.assertEqual((result["left_units"], result["matches"]), (units, units is not None))
        # 1 行目（i）は l からの差。両方を 5 にしたなら i は 0 か省略
        for prefix, matches in ((r"\pxi0,l5,qc;", True), (r"\pxi5,l5,qc;", False)):
            for name in ("Z316_PARA_25", "Z316_PARA_50"):
                self.text[name].text = prefix + name
            self.doc.saveas(self.edited)
            result = self.check("paragraph")
            self.assertEqual((result["left_units"], result["matches"]), ("drawing_units", matches))
        # 1 行目だけを変えた操作と、読まない引数を含む書式は、左インデントの読みにしない
        for prefix, message in ((r"\pxi5,qc;", "1 行目"), (r"\pxsm1.5,l2,qc;", "範囲外")):
            self.text["Z316_PARA_25"].text = prefix + "Z316_PARA_25"
            self.doc.saveas(self.edited)
            with self.assertRaisesRegex(ValueError, message):
                self.check("paragraph")
        self.text["Z316_PARA_25"].text = r"\pxl2,ql;Z316_PARA_25"
        self.doc.saveas(self.edited)
        self.assertFalse(self.check("paragraph")["matches"])
        self.text["Z316_CONTROL"].text = r"\pxqc;Z316_CONTROL"
        self.doc.saveas(self.edited)
        with self.assertRaisesRegex(ValueError, "対照"):
            self.check("paragraph")

    def observe(self, *args, options=("-I",)):
        # 利用者と同じく、キットに入れた observe.py を別のプロセスで流す。
        process = subprocess.run([sys.executable, *options, str(self.kit / "observe.py"), *map(str, args)],
                                 capture_output=True, encoding="utf-8", check=False)
        return process.returncode, json.loads(process.stdout)

    def test_main(self):
        for suffix, value in (("A", 0x332211), ("B", 0x112233)):
            name = "Z316_RGB_" + suffix
            self.text[name].text = "{\\c%d;%s}" % (value, name)
        self.doc.saveas(self.edited)
        code, result = self.observe(self.neutral, self.edited, "color")
        self.assertEqual((code, result["matches"], result["ezdxf"]), (0, True, "1.4.5"))
        self.assertEqual(result["after_sha256"], hashlib.sha256(self.edited.read_bytes()).hexdigest())
        self.assertEqual(result["observed"]["after"]["text"]["RGB_A"]["c"], 0x332211)
        code, result = self.observe(self.neutral, self.neutral, "color")
        self.assertEqual((code, result["matches"]), (1, False))
        # 比べられないときも、理由と読めた値を出す
        self.text["Z316_CONTROL"].text = r"\pxqc;Z316_CONTROL"
        self.doc.saveas(self.edited)
        code, result = self.observe(self.neutral, self.edited, "color")
        self.assertEqual((code, "matches" in result), (2, False))
        self.assertIn("対照", result["error"])
        self.assertEqual(result["observed"]["after"]["text"]["CONTROL"]["p"], ["xqc"])
        # 流し方の誤りは 3 にし、理由には渡したパスを書かず、変更前か変更後かだけを書く
        code, result = self.observe(self.neutral, self.neutral, "color", options=("-I", "-S"))
        self.assertEqual((code, result["ezdxf"], result["observed"]), (3, None, {}))
        self.assertIn("ezdxf 1.4.5", result["error"])
        code, result = self.observe(self.neutral, self.folder / "missing.dxf", "color")
        self.assertEqual((code, list(result["observed"])), (3, ["before"]))
        self.assertIn("変更後", result["error"])
        self.assertNotIn(str(self.folder), result["error"])
        ezdxf.new("R2013").saveas(self.edited)
        code, result = self.observe(self.edited, self.neutral, "color")
        self.assertEqual((code, result["observed"]), (3, {}))
        self.assertIn("R2018", result["error"])
        self.assertIn("変更前", result["error"])

    def test_mtext_scope(self):
        self.assertEqual(inline(r"\\c4;{\c1;\pxqc;X}M", "M"), {"c": None, "p": []})
        self.assertIsNone(inline(r"{\c1;\C3;M}", "M")["c"])
        self.assertEqual(inline(r"{\c7;{\c4;X}M}", "M")["c"], 7)

    def test_leader(self):
        ARROWS.create_block(self.doc.blocks, "DOT")
        self.doc.saveas(self.folder / "with-dot.dxf")
        expected = (
            ("leader-type", 1, 170, 2),
            ("leader-color", 2, 92, 0xC2112233 - (1 << 32)),
            ("leader-linetype", 4, 340, self.doc.linetypes.get("Z316_DASH").dxf.handle),
            ("leader-weight", 8, 171, 50),
            ("leader-size", 16, 40, 4.5),
            ("leader-arrow", 32, 341, self.doc.blocks.get("_DOT").block_record_handle),
        )
        for topic, flag, code, value in expected:
            with self.subTest(topic=topic):
                with_line_fields(self.folder / "with-dot.dxf", self.edited, {-30: {93: flag, code: value}})
                self.assertTrue(self.check(topic)["matches"])
        for changes in ({93: 8, 171: 25}, {93: 16, 171: 50}, {171: 50}, {93: 8, 171: 50, 170: 2}):
            with_line_fields(self.neutral, self.edited, {-30: changes})
            self.assertFalse(self.check("leader-weight")["matches"])
        with_line_fields(self.neutral, self.edited, {30: {93: 8, 171: 50}})
        with self.assertRaisesRegex(ValueError, "右の枝"):
            self.check("leader-weight")
        self.doc.modelspace().query("MULTILEADER")[0].dxf.leader_lineweight = 50
        self.doc.saveas(self.edited)
        with self.assertRaisesRegex(ValueError, "共通部"):
            self.check("leader-weight")
        # 共通部の線種（341）と矢印（342）は、伏せる前の名前で比べる。JSON では、キットの外の名前を番号で伏せる
        ARROWS.create_block(self.doc.blocks, "OPEN")
        ARROWS.create_block(self.doc.blocks, "CLOSEDBLANK")
        linetypes = [self.doc.linetypes.add(name, pattern=[0.75, 0.5, -0.25]).dxf.handle for name in ("DASHED", "HIDDEN")]
        arrows = [self.doc.blocks.get(name).block_record_handle for name in ("_OPEN", "_CLOSEDBLANK")]
        leader = self.doc.modelspace().query("MULTILEADER")[0]
        before = self.folder / "shared.dxf"
        for key, (old, new) in (("leader_linetype_handle", linetypes), ("arrow_head_handle", arrows)):
            with self.subTest(key=key):
                leader.dxf.set(key, old)
                self.doc.saveas(before)
                leader.dxf.set(key, new)
                self.doc.saveas(self.edited)
                code, result = self.observe(before, self.edited, "leader-weight")
                self.assertEqual(code, 2)
                self.assertIn("共通部", result["error"])
                self.assertNotEqual(*(result["observed"][k]["leader"]["shared"] for k in ("before", "after")))
                text = json.dumps(result, ensure_ascii=False)
                self.assertFalse([name for name in ("DASHED", "HIDDEN", "_OPEN", "_CLOSEDBLANK") if name in text])


if __name__ == "__main__":
    unittest.main()
