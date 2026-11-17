"""check.py の compare が、値・参照・配置の食い違いを見逃さないことを、合成の入力で確かめる。"""

import copy
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent))
from check import compare  # noqa: E402


def text(value):
    return {"kind": "text", "text": value}


def pset(name, values):
    return {"name": name, "properties": [{"name": k, "value": text(v)} for k, v in values.items()]}


DUMP = {
    "nodes": [{"id": "1", "X": 0, "Y": 0, "Z": 0}, {"id": "2", "X": 0, "Y": 0, "Z": 4000}],
    "stories": [{"id": "1", "name": "1F", "height": 0}],
    "members": {
        "StbColumn": [
            {"id": "7", "name": "C", "id_node_bottom": "1", "id_node_top": "2", "id_section": "3", "rotate": 90}
        ]
    },
    "sections": {
        "StbSecColumn_S": [
            {"id": "3", "name": "C1", "StbSecSteelFigureColumn_S": {"Same": [{"shape": "BOX", "$type": "Same"}]}}
        ]
    },
    "steels": {"StbSecRoll-BOX": [{"name": "BOX", "A": 400, "B": 400, "t": 19, "r": 18.000000000000004}]},
}

ELEMENTS = [
    {
        "identity": {"name": "1F", "tag": "1"},
        "class": {"original": "StbStory"},
        "properties": [pset("StbStory", {"id": "1", "name": "1F", "height": "0"})],
    },
    {
        "identity": {"name": "C", "tag": "7"},
        "class": {"original": "StbColumn"},
        "placement": {"world": [[0, -1, 0, 0], [1, 0, 0, 0], [0, 0, 1, 0]]},
        "properties": [
            pset("StbColumn", {"id": "7", "name": "C", "id_node_bottom": "1", "id_node_top": "2", "id_section": "3", "rotate": "90"}),
            pset("StbSecColumn_S", {"id": "3", "name": "C1"}),
            pset("StbSecRoll-BOX", {"name": "BOX", "A": "400", "B": "400", "t": "19", "r": "18.000000000000004"}),
        ],
    },
]


def column(elements):
    return elements[1]


class CompareTest(unittest.TestCase):
    def check(self, change_dump=None, change_elements=None, expect=None):
        dump = copy.deepcopy(DUMP)
        elements = copy.deepcopy(ELEMENTS)
        if change_dump:
            change_dump(dump)
        if change_elements:
            change_elements(elements)
        mismatches, _ = compare(elements, dump)
        if expect is None:
            self.assertEqual(mismatches, [])
        else:
            self.assertTrue(any(expect in m for m in mismatches), mismatches)

    def test_matches(self):
        self.check()

    def test_steel_dimension(self):
        self.check(change_dump=lambda d: d["steels"]["StbSecRoll-BOX"][0].update(t=22), expect="StbSecRoll-BOX.t")

    def test_node_moves_the_origin(self):
        self.check(change_dump=lambda d: d["nodes"][0].update(X=1), expect="原点")

    def test_node_turns_the_axis(self):
        self.check(change_dump=lambda d: d["nodes"][1].update(X=4000), expect="材軸")

    def test_missing_member(self):
        self.check(change_elements=lambda e: e.pop(1), expect="StbColumn 7: zumen の要素が 0 個")

    def test_extra_member(self):
        def extra(elements):
            other = copy.deepcopy(column(elements))
            other["identity"]["tag"] = "8"
            elements.append(other)

        self.check(change_elements=extra, expect="StbColumn 8: STBDotNet にない")

    def test_attribute_only_in_zumen(self):
        self.check(
            change_elements=lambda e: column(e)["properties"][0]["properties"].append({"name": "guid", "value": text("x")}),
            expect="StbColumn 7.guid: STBDotNet にない",
        )

    def test_other_steel(self):
        self.check(
            change_elements=lambda e: column(e)["properties"][2].update(name="StbSecRoll-H"),
            expect="StbColumn 7 StbSecRoll-BOX: zumen に属性のセットがない",
        )

    def test_name(self):
        self.check(change_elements=lambda e: column(e)["identity"].update(name="D"), expect="名前が違う")


if __name__ == "__main__":
    unittest.main()
