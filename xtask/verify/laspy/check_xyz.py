"""PDAL が配る同じ点群の XYZ と LAS を、公開 API・NumPy・laspy で照合する。"""

from collections import Counter
from pathlib import Path
import sys

import laspy
import numpy as np

VERIFY = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(VERIFY))
from make import Zumen


def compare(cli):
    samples = VERIFY.parents[1] / "samples"
    text = samples / "xyz/pdal-tile-1.xyz"
    cloud = laspy.read(samples / "las/pdal-tile-1.las")
    source = text.read_bytes()
    assert source.splitlines()[0] == b"X Y Z"
    xyz = np.loadtxt(text, skiprows=1)
    assert xyz.shape == (9, 3) and np.array_equal(xyz, cloud.xyz)
    # XYZ と LAS の X・Y・Z を公開 API の北・東・高さの並びにする。
    expected = xyz[:, [1, 0, 2]]
    session = Zumen(cli)
    try:
        doc = text.name
        session.push(doc, source)
        status, opened = session.call({"op": "open", "format": "xyz", "input": doc})
        assert status == "ok" and opened["result"]["format"] == "xyz"
        assert [(d["code"], d["count"], d["params"]) for d in opened["diagnostics"]] == [
            ("xyz.line.invalid", 1, {"line": 1, "lines": 1}),
            ("xyz.crs.missing", 1, {}),
        ]
        status, reply = session.call({"op": "pointcloud", "doc": doc, "limit": len(xyz)})
        assert status == "ok" and not reply["diagnostics"]
        result = reply["result"]
        assert result["points"] == len(xyz) and result["attributes"] == ["position"]
        assert result["bounds"] == {"min": expected.min(axis=0).tolist(), "max": expected.max(axis=0).tolist()}
        selected = result["selected"]
        assert not selected["truncated"]
        assert Counter(map(tuple, selected["positions"])) == Counter(map(tuple, expected))
    finally:
        session.close()
    print(f"公開 XYZ と LAS の {len(xyz)} 点で座標と範囲が一致")


if __name__ == "__main__":
    compare(Path(sys.argv[1]))
