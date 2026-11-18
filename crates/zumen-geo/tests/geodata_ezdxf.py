"""`tests/geodata.rs` の GEODATA を ezdxf で読み、期待する値を書く（#685）。

`geodata.rs` の `GEODATA` の表と `DRAWING` の点を読み、同じ DXF を作って ezdxf 1.4.5（MIT）で読む。GEODATA の値と、
`GeoData.get_crs_transformation` で写した地図の点を標準出力に書く。ezdxf は成果物に入れない。

    uv venv --python 3.14 --no-python-downloads target/geo-venv
    uv pip install --python target/geo-venv/Scripts/python.exe ezdxf==1.4.5
    target/geo-venv/Scripts/python.exe -X utf8 crates/zumen-geo/tests/geodata_ezdxf.py
"""

import io
import pathlib
import re

import ezdxf
from ezdxf.math import Vec3

SOURCE = pathlib.Path(__file__).with_name("geodata.rs").read_text(encoding="utf-8")


def table():
    body = SOURCE.split("const GEODATA: &[(i16, &str)] = &[", 1)[1].split("\n];", 1)[0]
    found = re.findall(r'\(\s*(\d+),\s*(?:r#"(.*?)"#|"(.*?)"),?\s*\)', body, re.S)
    return [(int(code), raw or plain) for code, raw, plain in found]


def drawing():
    body = SOURCE.split("const DRAWING: [[f64; 3]; 4] = [", 1)[1].split("\n];", 1)[0]
    return [tuple(float(v) for v in row.split(",")) for row in re.findall(r"\[([^\[\]]+)\]", body)]


def main():
    assert ezdxf.__version__ == "1.4.5", ezdxf.__version__
    lines = [
        "0", "SECTION", "2", "HEADER", "9", "$ACADVER", "1", "AC1024", "9", "$INSUNITS", "70", "4",
        "0", "ENDSEC", "0", "SECTION", "2", "OBJECTS",
        "0", "DICTIONARY", "5", "C", "330", "0", "100", "AcDbDictionary", "0", "GEODATA", "5", "20", "330", "0",
        "100", "AcDbGeoData",
    ]
    for code, value in table():
        lines += [str(code), value]
    lines += ["0", "ENDSEC", "0", "EOF"]
    doc = ezdxf.read(io.StringIO("\n".join(lines) + "\n"))
    geodata = doc.entitydb.query("GEODATA")[0]
    dxf = geodata.dxf
    for name in (
        "version", "coordinate_type", "design_point", "reference_point", "horizontal_unit_scale",
        "vertical_unit_scale", "up_direction", "north_direction", "scale_estimation_method",
        "user_scale_factor", "sea_level_correction",
    ):
        print(name, repr(dxf.get(name)), sep="\t")
    print("definition", len(geodata.coordinate_system_definition), sep="\t")
    print("crs", *geodata.get_crs(), sep="\t")
    matrix, epsg = geodata.get_crs_transformation()
    print("epsg", epsg, sep="\t")
    for point in drawing():
        mapped = matrix.transform(Vec3(point))
        print("point", repr(mapped.x), repr(mapped.y), sep="\t")


main()
