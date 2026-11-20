import tempfile
import unittest
from pathlib import Path

from plane import metres, site_rows, ticks


class PlaneComparison(unittest.TestCase):
    def test_four_decimal_metres_include_signed_zero_and_submetre_values(self):
        for text in ["0.0000", "-0.0001", "10000.0002"]:
            self.assertEqual(metres(ticks(text)), text)
        self.assertEqual(ticks("-0.0000"), 0)
        for text in ["-9999.", "2.500", "NaN", "2e-4"]:
            with self.assertRaises(ValueError):
                ticks(text)

    def test_chain_requires_the_actual_tky_output_for_every_id(self):
        source = {"P1": (1, 0), "P2": (5, -3)}
        output = b"0.0005 -0.0003 0.0006 -0.0004 P2\r\n0.0001 0.0000 0.0002 0.0000 P1\r\n"
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / "site.out"
            path.write_bytes(output)
            self.assertEqual(site_rows(path, source), {"P1": (2, 0), "P2": (6, -4)})
            for wrong in [output.replace(b"0.0001", b"0.0000"), output * 2, output.splitlines()[0]]:
                path.write_bytes(wrong)
                with self.assertRaises(ValueError):
                    site_rows(path, source)
