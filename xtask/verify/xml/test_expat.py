"""外部照合の原本位置と、差分を見つける処理を小さな独立入力で確かめる。"""

import unittest

from check_expat import check, independent_events


class ExpatComparisonTests(unittest.TestCase):
    def test_positions_names_and_text_in_each_byte_order(self):
        body = "<p:根 xmlns:p='urn:r' a='>日本😀'>前&#38;<![CDATA[本\r\n😀]]><p:子/></p:根>"
        for codec, declaration, bom in [
            ("utf-8", "", b""),
            ("utf-16-le", "<?xml version='1.0' encoding='UTF-16'?>", b"\xff\xfe"),
            ("utf-16-be", "<?xml version='1.0' encoding='UTF-16'?>", b"\xfe\xff"),
            ("utf-16-le", "<?xml version='1.0' encoding='UTF-16LE'?>", b""),
            ("utf-16-be", "<?xml version='1.0' encoding='UTF-16BE'?>", b""),
        ]:
            with self.subTest(codec=codec, bom=bom):
                src = bom + (declaration + body).encode(codec)
                events = independent_events(src)
                starts = [event for event in events if event[0] == "START"]
                self.assertEqual(len(starts), 2)
                self.assertEqual(starts[0][3:5], [str(len(bom + declaration.encode(codec))), str(len(src))])
                child = "<p:子/>".encode(codec)
                child_start = src.index(child)
                self.assertEqual(starts[1][3:5], [str(child_start), str(child_start + len(child))])
                self.assertEqual(starts[0][5:], ["根".encode().hex(), "urn:r".encode().hex()])
                self.assertIn(["ATTR", "0", "61", "-", ">日本😀".encode().hex()], events)
                self.assertIn(["TEXT", "0", "text", "前&".encode().hex()], events)
                self.assertIn(["TEXT", "0", "cdata", "本\n😀".encode().hex()], events)

    def test_detects_wrong_original_position_and_text(self):
        src = b"<r>value</r>"
        expected = independent_events(src)
        for position in [3, 4]:
            wrong = [event.copy() for event in expected]
            wrong[0][position] = str(int(wrong[0][position]) + 1)
            content = f"CASE\ttest\t{src.hex()}\n" + "\n".join("\t".join(event) for event in wrong) + "\nCASEEND\n"
            with self.assertRaises(ValueError):
                check(content)
        wrong = [event.copy() for event in expected]
        wrong[1][-1] = "wrong".encode().hex()
        content = f"CASE\ttest\t{src.hex()}\n" + "\n".join("\t".join(event) for event in wrong) + "\nCASEEND\n"
        with self.assertRaises(ValueError):
            check(content)


if __name__ == "__main__":
    unittest.main()
