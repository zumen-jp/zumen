"""#316 の比較が合成 DXF の値を読み、対照や対象外の書式を取り違えないことを確かめる。"""

import hashlib
import json
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
        self.text["Z316_PARA_25"].text = r"\pxl2,ql;Z316_PARA_25"
        self.doc.saveas(self.edited)
        self.assertFalse(self.check("paragraph")["matches"])
        self.text["Z316_CONTROL"].text = r"\pxqc;Z316_CONTROL"
        self.doc.saveas(self.edited)
        with self.assertRaisesRegex(ValueError, "対照"):
            self.check("paragraph")

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


if __name__ == "__main__":
    unittest.main()
