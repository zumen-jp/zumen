"""JPEG の Exif と XMP を Pillow で読み、比較する値だけを TSV で返す（#505）。

Exif は Pillow 13.0.0 が読んだタグの値を使う。XMP は Pillow が APP1 から取り出したパケットを、Python の
ElementTree（Expat）で読む。どちらも標準入力の 1 つの JPEG を読み、1 行に 1 つの値を書く。
"""

import io
import re
import sys
import xml.etree.ElementTree as ET

RDF = "http://www.w3.org/1999/02/22-rdf-syntax-ns#"
XMLNS = "http://www.w3.org/2000/xmlns/"
ARRAYS = {f"{{{RDF}}}{name}" for name in ("Alt", "Seq", "Bag")}


def text(value):
    """Exif の ASCII の値。Pillow は Latin-1 で復号するので、バイト列に戻して UTF-8 で読む"""
    if value is None:
        return None
    raw = value.encode("latin-1") if isinstance(value, str) else bytes(value)
    try:
        decoded = raw.decode("utf-8")
    except UnicodeDecodeError:
        return None
    decoded = decoded.rstrip("\0").strip()
    return decoded or None


def number(value):
    return None if value is None else float(value)


def byte(value):
    if isinstance(value, bytes):
        return value[0] if value else None
    return value


def degrees(value, ref, negative):
    if value is None or len(value) != 3:
        return None
    d, m, s = (float(v) for v in value)
    sign = -1.0 if text(ref) == negative else 1.0
    return sign * (d + m / 60.0 + s / 3600.0)


def taken(exif_ifd):
    value = text(exif_ifd.get(0x9003))
    if value is None:
        return None
    found = re.fullmatch(r"(\d{4}):(\d{2}):(\d{2}) (\d{2}:\d{2}:\d{2})", value)
    if found is None:
        return None
    year, month, day, time = found.groups()
    return f"{year}-{month}-{day}T{time}{text(exif_ifd.get(0x9011)) or ''}"


def local(tag):
    namespace, _, name = tag[1:].partition("}")
    return namespace, name


def xmp(packet):
    """rdf:Description の属性と、文字だけを持つ子の要素。値の前後の空白は除く。配列は最初の rdf:li の値"""
    if packet is None:
        return []
    if isinstance(packet, str):
        packet = packet.encode("utf-8")
    root = ET.fromstring(packet)
    out = []
    for description in root.iter(f"{{{RDF}}}Description"):
        for key, value in description.attrib.items():
            if key.startswith("{"):
                namespace, name = local(key)
                if namespace not in (RDF, XMLNS):
                    out.append((namespace, name, value))
        for child in description:
            namespace, name = local(child.tag)
            items = list(child)
            if not items:
                out.append((namespace, name, (child.text or "").strip()))
            elif len(items) == 1 and items[0].tag in ARRAYS:
                # 子の要素を持たない最初の rdf:li
                for item in items[0].findall(f"{{{RDF}}}li"):
                    if len(item) == 0:
                        out.append((namespace, name, (item.text or "").strip()))
                        break
    return out


def show(value):
    if value is None:
        return "-"
    if isinstance(value, float):
        return repr(value)
    return str(value)


def main():
    try:
        import PIL
        from PIL import Image
    except ModuleNotFoundError as error:
        if error.name != "PIL":
            raise
        return 77
    if PIL.__version__ != "13.0.0":
        raise RuntimeError(f"外部ツールのバージョンが違う: {PIL.__version__}")
    if sys.argv[1:] == ["--check"]:
        print("pillow\t13.0.0")
        return 0
    if sys.argv[1:]:
        raise ValueError("引数は --check だけ")

    image = Image.open(io.BytesIO(sys.stdin.buffer.read()))
    if image.format != "JPEG":
        raise ValueError(f"JPEG でない: {image.format}")
    exif = image.getexif()
    exif_ifd = exif.get_ifd(0x8769)
    gps = exif.get_ifd(0x8825)
    latitude = degrees(gps.get(2), gps.get(1), "S")
    longitude = degrees(gps.get(4), gps.get(3), "W")
    if latitude is None or longitude is None:
        latitude = longitude = None
    altitude = number(gps.get(6))
    if altitude is not None and byte(gps.get(5)) == 1:
        altitude = -altitude
    rows = [
        ("size", *image.size),
        ("orientation", exif.get(0x0112)),
        ("taken", taken(exif_ifd)),
        ("make", text(exif.get(0x010F))),
        ("model", text(exif.get(0x0110))),
        ("position", latitude, longitude),
        ("altitude", altitude),
        ("direction", number(gps.get(0x11))),
        ("direction-reference", text(gps.get(0x10))),
        ("datum", text(gps.get(0x12))),
    ]
    for name, *values in rows:
        print(name, *(show(v) for v in values), sep="\t")
    for namespace, name, value in xmp(image.info.get("xmp")):
        print("xmp", namespace, name, value.replace("\\", "\\\\").replace("\n", "\n").replace("\t", "\t"), sep="\t")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
