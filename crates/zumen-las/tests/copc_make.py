"""COPC の試験の入力（`tests/copc.rs` の `COPC`）を作る（#473）。

lazrs（Python、MIT・Apache-2.0）の可変長チャンクの圧縮で、八分木の節ごとに 1 つのチャンクを書き、
COPC 1.0（https://copc.io/）の情報の VLR と階層の EVLR を自前で組む。lazrs は成果物に入れない。

    uv venv --python 3.14 --no-python-downloads target/copc-venv
    uv pip install --python target/copc-venv/Scripts/python.exe --only-binary :all: lazrs==0.9.0
    target/copc-venv/Scripts/python.exe crates/zumen-las/tests/copc_make.py

標準出力に 16 進の文字列を 1 行で書く。
"""

import io
import struct

import lazrs

SCALE = 0.01
OFFSET = (1000.0, 2000.0, 0.0)
CENTER = (1000.0, 2000.0, 50.0)
HALFSIZE = 8.0
SPACING = 2.0
WKT = (
    'PROJCS["JGD2011 / Japan Plane Rectangular CS IX",'
    'GEOGCS["JGD2011",AUTHORITY["EPSG","6668"]],'
    'AUTHORITY["EPSG","6677"]]'
).encode() + b"\0"

# 節の番号（段・X・Y・Z）と点（東・北・標高・強度）。根の頁に根と (1,0,0,0) を、
# 子の頁に (1,1,1,0) と (2,3,2,0) を置く
NODES = [
    ((0, 0, 0, 0), [(993.0, 1993.0, 43.0, 1), (1007.0, 2007.0, 57.0, 2), (1000.5, 2000.5, 50.0, 3)]),
    ((1, 0, 0, 0), [(995.25, 1996.5, 44.0, 4), (999.0, 1999.0, 49.0, 5)]),
    ((1, 1, 1, 0), [(1001.0, 2001.0, 45.0, 6), (1003.5, 2002.25, 48.5, 7), (1002.0, 2003.0, 46.0, 8)]),
    ((2, 3, 2, 0), [(1005.0, 2001.5, 43.5, 9), (1006.75, 2003.5, 45.0, 10)]),
]


def record(e, n, h, intensity, index):
    x = round((e - OFFSET[0]) / SCALE)
    y = round((n - OFFSET[1]) / SCALE)
    z = round((h - OFFSET[2]) / SCALE)
    # 点の形式 6: 座標・強度・反射・フラグ・分類・利用者・走査角・点の出所・GPS 時刻
    return struct.pack("<iiiHBBBBhHd", x, y, z, intensity, 0x11, 0, 2, 0, 0, 1, 100.0 + index)


def vlr(user, record_id, data, description=b""):
    return struct.pack("<H16sHH32s", 0, user, record_id, len(data), description) + data


def main():
    laz = lazrs.LazVlr.new_for_compression(6, 0, True)
    laz_data = bytes(laz.record_data())
    vlrs_without_info = vlr(b"laszip encoded", 22204, laz_data) + vlr(b"LASF_Projection", 2112, WKT)
    info_vlr_len = 54 + 160
    point_offset = 375 + info_vlr_len + len(vlrs_without_info)

    out = io.BytesIO()
    out.write(b"\0" * point_offset)
    compressor = lazrs.LasZipCompressor(out, laz)
    chunks = []
    index = 0
    for key, points in NODES:
        # 点の初めの 8 バイトは、チャンクの表の位置。lazrs は最初の点を書くときに置く
        start = max(out.tell(), point_offset + 8)
        data = b""
        for e, n, h, intensity in points:
            data += record(e, n, h, intensity, index)
            index += 1
        compressor.compress_many(data)
        compressor.finish_current_chunk()
        chunks.append((key, len(points), start))
    compressor.done()
    # finish_current_chunk の後の位置が、次のチャンクの始まり
    sized = []
    for i, (key, count, start) in enumerate(chunks):
        end = chunks[i + 1][2] if i + 1 < len(chunks) else None
        sized.append((key, count, start, end))
    out.seek(0, io.SEEK_END)
    file_end = out.tell()
    # 最後のチャンクの終わりは、チャンクの表の始まり（点の初めの 8 バイトの位置）
    table_at = struct.unpack("<q", out.getvalue()[point_offset : point_offset + 8])[0]
    sized[-1] = (*sized[-1][:3], table_at)

    def entry(key, offset, size, count):
        return struct.pack("<iiiiQii", *key, offset, size, count)

    evlr_start = file_end
    page_start = evlr_start + 60
    root_entries = 3
    child_at = page_start + root_entries * 32
    child_page = b"".join(entry(k, s, e - s, c) for k, c, s, e in sized[2:])
    root_page = (
        entry(sized[0][0], sized[0][2], sized[0][3] - sized[0][2], sized[0][1])
        + entry(sized[1][0], sized[1][2], sized[1][3] - sized[1][2], sized[1][1])
        + entry((1, 1, 1, 0), child_at, len(child_page), -1)
    )
    pages = root_page + child_page
    evlr = struct.pack("<H16sHQ32s", 0, b"copc", 1000, len(pages), b"") + pages
    out.write(evlr)

    info = struct.pack(
        "<5d2Q2d11Q",
        *CENTER,
        HALFSIZE,
        SPACING,
        page_start,
        len(root_page),
        100.0,
        100.0 + index - 1,
        *([0] * 11),
    )
    all_points = [p for _, ps in NODES for p in ps]
    es = [p[0] for p in all_points]
    ns = [p[1] for p in all_points]
    hs = [p[2] for p in all_points]
    header = bytearray(375)
    header[0:4] = b"LASF"
    struct.pack_into("<H", header, 6, 0x10 | 0x01)  # WKT と GPS 時刻の基準
    header[24], header[25] = 1, 4
    header[26:58] = b"zumen copc test".ljust(32, b"\0")
    header[58:90] = b"copc_make.py".ljust(32, b"\0")
    struct.pack_into("<HHHI", header, 90, 1, 2026, 375, point_offset)
    struct.pack_into("<I", header, 100, 3)
    header[104] = 6 | 0x80
    struct.pack_into("<HI", header, 105, 30, 0)
    struct.pack_into("<3d3d", header, 131, SCALE, SCALE, SCALE, *OFFSET)
    struct.pack_into("<6d", header, 179, max(es), min(es), max(ns), min(ns), max(hs), min(hs))
    struct.pack_into("<QQIQ", header, 227, 0, evlr_start, 1, len(all_points))
    data = bytearray(out.getvalue())
    data[0:375] = header
    data[375 : 375 + info_vlr_len] = vlr(b"copc", 1, info)
    data[375 + info_vlr_len : point_offset] = vlrs_without_info
    print(bytes(data).hex())


main()
