"""曲線 getter の元の属性値を IfcOpenShell と照らす（#419）。

使い方: python curves.py <JSON のフォルダー>

JSON は `curves.rs` の外部照合用のテストが `ZUMEN_IFC_CURVES_OUT` に書く。
各ファイルは {"file": "IFC のパス", "items": [...]} で、相対パスは JSON の親から引く。
合成の IFC2X3・IFC4・IFC4X3 と、公開サンプルの曲線を対象にする。

座標の次元・方向の比・半径・ベクトルの大きさ・添字・区間の順・向き・切る位置を比べる。
単位の換算、軸の正規化、配置の適用、曲線の評価は行わない。長さと平面角の単位そのもの、
出所のレコードの列、診断と予算は Rust のテストで確かめ、ここでは比べない。
"""

import json
import math
import sys
import tempfile
from pathlib import Path

import ifcopenshell


# 1 つのファイルで書く食い違いの数の上限
SHOWN = 20


def open_model(path):
    """IfcOpenShell 0.9 が読む IFC4X3_ADD2 の名前にした一時の写しを使う"""
    data = path.read_bytes()
    schema = b"FILE_SCHEMA(('IFC4X3'))"
    if schema not in data:
        return ifcopenshell.open(str(path))
    with tempfile.TemporaryDirectory() as folder:
        copy = Path(folder) / "curve.ifc"
        copy.write_bytes(data.replace(schema, b"FILE_SCHEMA(('IFC4X3_ADD2'))", 1))
        return ifcopenshell.open(str(copy))


def coordinates(point):
    return list(point.Coordinates)


def logical(value):
    return None if value == "UNKNOWN" else value


def trim(value):
    if value.is_a("IfcCartesianPoint"):
        return {"point": coordinates(value)}
    if value.is_a("IfcParameterValue"):
        return {"parameter": value.wrappedValue}
    raise ValueError(f"切る位置の種類 {value.is_a()} を扱わない")


def curve(entity):
    """IfcOpenShell の属性から、zumen と照らす値を作る"""
    found = {"instance": entity.id()}
    kind = entity.is_a()
    if kind == "IfcPolyline":
        found.update(kind="polyline", points=[coordinates(p) for p in entity.Points])
    elif kind == "IfcLine":
        found.update(
            kind="line",
            point=coordinates(entity.Pnt),
            direction=list(entity.Dir.Orientation.DirectionRatios),
            magnitude=entity.Dir.Magnitude,
        )
    elif kind == "IfcCircle":
        position = entity.Position
        axis = getattr(position, "Axis", None)
        direction = position.RefDirection
        found.update(
            kind="circle",
            location=coordinates(position.Location),
            axis=list(axis.DirectionRatios) if axis is not None else None,
            reference_direction=list(direction.DirectionRatios) if direction is not None else None,
            radius=entity.Radius,
        )
    elif kind == "IfcIndexedPolyCurve":
        segments = entity.Segments
        found.update(
            kind="indexed",
            points=[list(p) for p in entity.Points.CoordList],
            segments=None if segments is None else [indexed_segment(s) for s in segments],
            self_intersect=logical(entity.SelfIntersect),
        )
    elif kind == "IfcCompositeCurve":
        found.update(
            kind="composite",
            segments=[
                {
                    "instance": segment.id(),
                    "curve": segment.ParentCurve.id(),
                    "same_sense": segment.SameSense,
                    "transition": segment.Transition,
                }
                for segment in entity.Segments
            ],
            self_intersect=logical(entity.SelfIntersect),
        )
    elif kind == "IfcTrimmedCurve":
        found.update(
            kind="trimmed",
            basis=entity.BasisCurve.id(),
            trim1=[trim(v) for v in entity.Trim1],
            trim2=[trim(v) for v in entity.Trim2],
            sense_agreement=entity.SenseAgreement,
            preference=entity.MasterRepresentation,
        )
    else:
        raise ValueError(f"曲線の種類 {kind} を扱わない")
    return found


def indexed_segment(segment):
    kind = {"IfcLineIndex": "line", "IfcArcIndex": "arc"}.get(segment.is_a())
    if kind is None:
        raise ValueError(f"添字の区間の種類 {segment.is_a()} を扱わない")
    return {"kind": kind, "indices": list(segment.wrappedValue)}


def differences(ours, theirs, at):
    """値と列の順、キーの有無を厳密に比べ、食い違いの位置を返す"""
    if isinstance(ours, dict) and isinstance(theirs, dict):
        for key in sorted(ours.keys() | theirs.keys()):
            position = f"{at}.{key}"
            if key not in ours:
                yield f"{position}: zumen の JSON にキーがない"
            elif key not in theirs:
                yield f"{position}: IfcOpenShell の値にキーがない"
            else:
                yield from differences(ours[key], theirs[key], position)
        return
    if isinstance(ours, list) and isinstance(theirs, list):
        if len(ours) != len(theirs):
            yield f"{at}: 項目の数が {len(ours)}（IfcOpenShell は {len(theirs)}）"
        for index, (a, b) in enumerate(zip(ours, theirs)):
            yield from differences(a, b, f"{at}[{index}]")
        return
    if isinstance(ours, bool) or isinstance(theirs, bool):
        equal = type(ours) is bool and type(theirs) is bool and ours == theirs
    elif isinstance(ours, (int, float)) and isinstance(theirs, (int, float)):
        # 元の数の読み取りと JSON への書き出しなので、許容差で違いを隠さない
        equal = math.isfinite(ours) and math.isfinite(theirs) and ours == theirs
    else:
        equal = type(ours) is type(theirs) and ours == theirs
    if not equal:
        yield f"{at}: {str(ours)[:160]}（IfcOpenShell は {str(theirs)[:160]}）"


def check(path):
    data = json.loads(path.read_text(encoding="utf-8"))
    model = open_model(path.parent / data["file"])
    items = data["items"]
    if not items:
        raise ValueError("照らす曲線がない")
    failures = []
    count = 0
    for item in items:
        instance = item["instance"]
        at = f"#{instance}"
        try:
            found = curve(model.by_id(instance))
        except (AttributeError, RuntimeError, TypeError, ValueError) as error:
            count += 1
            if len(failures) < SHOWN:
                failures.append(f"{at}: IfcOpenShell から読めない: {error}")
            continue
        for failure in differences(item, found, at):
            count += 1
            if len(failures) < SHOWN:
                failures.append(failure)
    return len(items), count, failures


def main():
    if len(sys.argv) != 2:
        print("使い方: python curves.py <JSON のフォルダー>", file=sys.stderr)
        return 2
    paths = sorted(Path(sys.argv[1]).glob("*.json"))
    if not paths:
        print("フォルダーに JSON がない", file=sys.stderr)
        return 2
    failed = 0
    total = 0
    for path in paths:
        try:
            items, count, failures = check(path)
        except (OSError, KeyError, RuntimeError, TypeError, ValueError) as error:
            items, count, failures = 0, 1, [f"照合を行えない: {error}"]
        total += items
        failed += count != 0
        print(f"{path.name}: 曲線 {items}、食い違い {count}")
        for failure in failures:
            print(f"  {failure}")
        if count > len(failures):
            print(f"  ほか {count - len(failures)} 件")
    print(f"IfcOpenShell {ifcopenshell.version} で {len(paths)} 個のファイルの曲線 {total} 個を照らし、"
          f"{failed} 個のファイルに食い違いがあった")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
