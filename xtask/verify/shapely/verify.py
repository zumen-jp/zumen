import json
import sys

try:
    import shapely
except ImportError:
    sys.exit(77)

if shapely.__version__ != "2.2.0":
    sys.stderr.write(f"expected shapely 2.2.0, got {shapely.__version__}\n")
    sys.exit(1)

def out(text):
    sys.stdout.buffer.write(text.encode("utf-8"))


if "--check" in sys.argv:
    out(f"shapely\t{shapely.__version__}\n")
    sys.exit(0)

if "--emit" in sys.argv:
    geoms = [
        shapely.Point(139.767125, 35.681236, 12.5),
        shapely.MultiPoint([(139.0, 35.0), (140.0, 36.0, 5.0)]),
        shapely.LineString([(139.0, 35.0, 1.0), (139.5, 35.5, 2.0), (140.0, 36.0, 3.0)]),
        shapely.MultiLineString(
            [
                [(139.0, 35.0), (139.1, 35.1)],
                [(139.2, 35.2), (139.3, 35.3)],
            ]
        ),
        shapely.Polygon(
            [(139.0, 35.0), (140.0, 35.0), (140.0, 36.0), (139.0, 36.0)],
            [[(139.2, 35.2), (139.2, 35.8), (139.8, 35.8), (139.8, 35.2)]],
        ),
        shapely.MultiPolygon(
            [
                shapely.Polygon([(139.0, 35.0), (139.1, 35.0), (139.1, 35.1), (139.0, 35.1)]),
                shapely.Polygon([(139.5, 35.5), (139.6, 35.5), (139.6, 35.6), (139.5, 35.6)]),
            ]
        ),
        shapely.GeometryCollection(
            [
                shapely.Point(139.7, 35.6),
                shapely.LineString([(139.0, 35.0), (140.0, 36.0)]),
            ]
        ),
    ]
    for geom in geoms:
        out(shapely.to_geojson(geom) + "\n")
    sys.exit(0)

raw = sys.stdin.buffer.read().decode("utf-8")
fc = json.loads(raw)
if fc.get("type") != "FeatureCollection" or not isinstance(fc.get("features"), list):
    sys.stderr.write("expected FeatureCollection\n")
    sys.exit(1)


def check_polygon_rings(poly, context):
    if not poly.exterior.is_ccw:
        sys.stderr.write(f"{context} exterior is not CCW\n")
        sys.exit(1)
    for interior in poly.interiors:
        if interior.is_ccw:
            sys.stderr.write(f"{context} hole is not CW\n")
            sys.exit(1)


def check_geom(geom_obj):
    g = shapely.from_geojson(json.dumps(geom_obj))
    if not shapely.is_valid(g):
        reason = shapely.is_valid_reason(g)
        sys.stderr.write(f"invalid geometry: {reason}: {geom_obj}\n")
        sys.exit(1)
    if g.geom_type == "Polygon":
        check_polygon_rings(g, "polygon")
    elif g.geom_type == "MultiPolygon":
        for part in g.geoms:
            check_polygon_rings(part, "multipolygon")
    elif g.geom_type == "GeometryCollection":
        for part in g.geoms:
            if part.geom_type == "Polygon":
                check_polygon_rings(part, "collection polygon")
            elif part.geom_type == "MultiPolygon":
                for sub in part.geoms:
                    check_polygon_rings(sub, "collection multipolygon")
    return g


for feat in fc["features"]:
    geom = feat.get("geometry")
    if geom is None:
        out(f"{feat.get('id', '')}\tNull\t0\t{{}}\t[]\n")
        continue
    g = check_geom(geom)
    count = len(g.geoms) if hasattr(g, "geoms") else 1
    props = json.dumps(feat.get("properties", {}), ensure_ascii=False, sort_keys=True, separators=(",", ":"))
    coords = json.dumps(
        shapely.get_coordinates(g, include_z=bool(g.has_z)).tolist(),
        separators=(",", ":"),
    )
    out(f"{feat.get('id', '')}\t{g.geom_type}\t{count}\t{props}\t{coords}\n")
