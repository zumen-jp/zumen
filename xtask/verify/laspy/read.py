"""非圧縮 LAS を laspy で読み、比較する値だけを TSV で返す（#804）。"""

import io
import sys


def main():
    try:
        import laspy
        import numpy
    except ModuleNotFoundError as error:
        if error.name not in ("laspy", "numpy"):
            raise
        return 77

    versions = (laspy.__version__, numpy.__version__)
    if versions != ("2.7.0", "2.5.4"):
        raise RuntimeError(f"外部ツールのバージョンが違う: {versions}")
    if sys.argv[1:] == ["--check"]:
        print("laspy\t2.7.0\nnumpy\t2.5.4")
        return 0
    if sys.argv[1:]:
        raise ValueError("引数は --check だけ")

    data = laspy.read(io.BytesIO(sys.stdin.buffer.read()))
    header = data.header
    if header.are_points_compressed:
        raise ValueError("照合の対象は非圧縮 LAS だけ")
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
