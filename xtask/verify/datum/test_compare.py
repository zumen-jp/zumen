"""外部の数値を使わず、結果の照合と、不正な結果の拒否を確かめる。"""

import unittest

from compare import angle, compare


class DatumComparison(unittest.TestCase):
    def setUp(self):
        self.point = ["10000.00000", "20000.00000"]
        self.shifted = ["10000.01000", "20000.02000"]
        self.outside = ["00000.00000", "00000.00000"]
        self.calculated = {"N": self.point + self.shifted, "OUT": self.outside + ["-", "-"]}
        self.site = {"N": self.point + ["ok"] + self.shifted, "OUT": self.outside + ["sea"]}

    def test_site_order_and_expected_missing_are_independent(self):
        self.assertEqual(compare("tky2jgd", self.calculated, dict(reversed(self.site.items()))), (1, 1, 0))
        for result in [["unchanged"], ["ok"] + self.outside]:
            self.site["OUT"] = self.outside + result
            self.assertEqual(compare("patchjgd", self.calculated, self.site), (1, 1, 0))
        with self.assertRaises(ValueError):
            compare("tky2jgd", self.calculated, self.site)

    def test_result_changes_are_measured_at_the_display_precision(self):
        self.site["N"][-1] = "20000.02001"
        self.assertEqual(compare("tky2jgd", self.calculated, self.site), (1, 1, 1))
        self.assertEqual(angle("10000.00000", 90) - angle("05959.99999", 90), 1)

    def test_stale_input_missing_id_and_unexpected_status_are_rejected(self):
        for observed in [
            {**self.site, "N": self.outside + ["ok"] + self.shifted},
            {"N": self.site["N"]},
            {**self.site, "N": self.point + ["sea"]},
            {**self.site, "OUT": self.outside + ["unchanged"]},
        ]:
            with self.assertRaises(ValueError):
                compare("tky2jgd", self.calculated, observed)


if __name__ == "__main__":
    unittest.main()
