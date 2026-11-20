import unittest

from batch import batch_rows, make_input
from compare import compare


class BatchComparison(unittest.TestCase):
    expected = {
        "N": ["10000.00000", "20000.00000", "10001.00000", "20001.00000"],
        "OUT": ["00000.00000", "00000.00000", "-", "-"],
    }
    output = (
        "# 測地系の一括出力\n"
        "00000.00000 00000.00000 -9999. -9999. OUT\n"
        "10000.00000 20000.00000 10001.00000 20001.00000 N\n"
    )

    def test_input_contains_only_source_coordinates_and_id(self):
        self.assertEqual(make_input(self.expected).splitlines()[0], "10000.00000 20000.00000 N")

    def test_official_encodings_and_sentinel(self):
        for encoding, mode in [("cp932", "tky2jgd"), ("utf-8-sig", "patchjgd")]:
            observed = batch_rows(self.output.encode(encoding), mode)
            self.assertEqual(compare(mode, self.expected, observed), (1, 1, 0))

    def test_partial_missing_and_duplicate_ids_are_rejected(self):
        for data in [self.output.replace("-9999. -9999.", "-9999. 00000.00000"), self.output * 2]:
            with self.assertRaises(ValueError):
                batch_rows(data.encode(), "tky2jgd")
