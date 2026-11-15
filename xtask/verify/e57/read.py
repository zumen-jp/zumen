"""公開の色付き立方体2件だけをpye57で読み、全点と色の比較用バイトを返す（#504）。"""

import hashlib
from importlib.metadata import PackageNotFoundError, version
from pathlib import Path
import struct
import sys
import tempfile


PINS = {"pye57": "0.4.19", "numpy": "2.5.4", "pyquaternion": "0.9.9"}
SAMPLES = {
    "58e7cfbb3e9cef777c4cd8f52aff2ff704fcf6dd3aea4b420430adaa1cdb978d": "E57_DOUBLE",
    "6dbf7972b358bd7dd0864c7893a4aa7b61a339fd6ee27c71b3031f763c977d33": "E57_SINGLE",
}


def require_tools():
    for name, expected in PINS.items():
        try:
            actual = version(name)
        except PackageNotFoundError:
            raise SystemExit(77) from None
        if actual != expected:
            raise ValueError(f"{name} version mismatch: {actual}")


def observe(content):
    import numpy as np
    import pye57
    from pye57 import libe57 as lib

    expected_precision = SAMPLES[hashlib.sha256(content).hexdigest()]

    def cast(kind, node):
        return node if isinstance(node, kind) else kind(node)

    with tempfile.TemporaryDirectory(prefix="zumen-e57-oracle-") as directory:
        path = Path(directory) / "sample.e57"
        path.write_bytes(content)
        with pye57.E57(str(path)) as file:
            root = file.image_file.root()
            assert file.scan_count == 1
            assert not root.isDefined("coordinateMetadata")
            assert cast(lib.VectorNode, root["images2D"]).childCount() == 0
            assert [cast(lib.IntegerNode, root[k]).value() for k in ("versionMajor", "versionMinor")] == [1, 0]
            scan = cast(lib.StructureNode, root["data3D"][0])
            prototype = cast(lib.StructureNode, cast(lib.CompressedVectorNode, scan["points"]).prototype())
            for axis in ("cartesianX", "cartesianY", "cartesianZ"):
                assert cast(lib.FloatNode, prototype[axis]).precision() == getattr(lib.FloatPrecision, expected_precision)
            limits = cast(lib.StructureNode, scan["colorLimits"])
            for channel in ("Red", "Green", "Blue"):
                assert cast(lib.IntegerNode, limits[f"color{channel}Minimum"]).value() == 0
                assert cast(lib.IntegerNode, limits[f"color{channel}Maximum"]).value() == 255
            header = file.get_header(0)
            assert header.point_count == 7680
            assert np.array_equal(header.rotation_matrix, np.eye(3))
            assert np.array_equal(header.translation, np.zeros(3))
            raw = file.read_scan_raw(0)
            fields = ("cartesianX", "cartesianY", "cartesianZ", "colorRed", "colorGreen", "colorBlue")
            assert set(raw) == set(fields)
            assert all(len(raw[k]) == header.point_count for k in fields)
            assert all(np.isfinite(raw[k]).all() for k in fields)
            xyz = np.column_stack([raw[k] for k in fields[:3]]).astype(np.float64)
            rgb = np.column_stack([raw[k] for k in fields[3:]])
            assert np.equal(rgb, np.floor(rgb)).all() and ((0 <= rgb) & (rgb <= 255)).all()
            # 公開試料の0〜255をコアの16 bitへ換算する。端数の丸めはこの2件にはない。
            colors = rgb.astype(np.uint16) * 257
            low, high = xyz.min(axis=0), xyz.max(axis=0)
            assert low.tolist() == [-0.5] * 3 and high.tolist() == [0.5] * 3
            output = bytearray(b"zumen-e57 external 1\n")
            output.extend(struct.pack("<QQ6d", file.scan_count, len(xyz), *low, *high))
            for point, color in zip(xyz, colors, strict=True):
                output.extend(struct.pack("<3d3H", *point, *color))
            return bytes(output)


def main():
    require_tools()
    if sys.argv[1:] == ["--check"]:
        for name, value in PINS.items():
            print(f"{name}\t{value}")
        return
    if len(sys.argv) != 1:
        raise ValueError("only --check or standard input is accepted")
    content = sys.stdin.buffer.read(210945)
    if len(content) > 210944:
        raise ValueError("only the two pinned public samples are accepted")
    sys.stdout.buffer.write(observe(content))


if __name__ == "__main__":
    main()
