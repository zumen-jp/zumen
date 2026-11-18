"""LAS と LAZ を laspy で読み、比較する値だけを TSV で返す（#804・#504）。LAZ の展開は lazrs。"""

import importlib.metadata
import io
import sys


def main():
    try:
        import laspy
        import lazrs  # noqa: F401
        import numpy
    except ModuleNotFoundError as error:
        if error.name not in ("laspy", "lazrs", "numpy"):
            raise
        return 77

    versions = (laspy.__version__, importlib.metadata.version("lazrs"), numpy.__version__)
    if versions != ("2.7.0", "0.9.0", "2.5.4"):
        raise RuntimeError(f"外部ツールのバージョンが違う: {versions}")
    if sys.argv[1:] == ["--check"]:
        print("laspy\t2.7.0\nlazrs\t0.9.0\nnumpy\t2.5.4")
        return 0
    if sys.argv[1:]:
        raise ValueError("引数は --check だけ")

    data = laspy.read(io.BytesIO(sys.stdin.buffer.read()), laz_backend=laspy.LazBackend.Lazrs)
    header = data.header
    print(
        "header", header.version.major, header.version.minor,
        data.point_format.id, data.point_format.size, header.point_count, sep="\t",
    )
    for name, values in (
        ("scale", header.scales),
        ("offset", header.offsets),
        ("bounds", [*header.mins, *header.maxs]),
    ):
        print(name, *(repr(float(value)) for value in values), sep="\t")

    dimensions = set(data.point_format.dimension_names)
    optional = ["gps_time", "red", "green", "blue"]
    columns = [
        data.x, data.y, data.z, data.intensity, data.classification,
        data.return_number, data.number_of_returns,
        *(data[name] if name in dimensions else [None] * len(data.points)
          for name in optional),
    ]
    for row in zip(*columns, strict=True):
        values = [
            "-" if value is None else repr(float(value)) if index in (0, 1, 2, 7)
            else str(int(value))
            for index, value in enumerate(row)
        ]
        print("point", *values, sep="\t")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
