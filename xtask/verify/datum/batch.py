"""TKY2JGD・PatchJGD の経緯度の一括入力を作り、サイトが出力したファイルを照合する。"""

import argparse
from pathlib import Path

from compare import compare, position, rows


def make_input(expected):
    lines = []
    for key, fields in expected.items():
        if len(fields) != 4:
            raise ValueError(f"{key} の zumen の列数が不正です")
        latitude, longitude = fields[:2]
        position(latitude, longitude)
        lines.append(f"{latitude} {longitude} {key}\n")
    return "".join(lines)


def batch_rows(raw, mode):
    try:
        content = raw.decode("utf-8-sig")
    except UnicodeDecodeError:
        content = raw.decode("cp932")
    observed = {}
    for index, line in enumerate(content.splitlines(), 1):
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        fields = line.split()
        if len(fields) != 5:
            raise ValueError(f"一括出力の {index} 行目の列数が不正です")
        latitude, longitude, after_lat, after_lon, key = fields
        if key in observed:
            raise ValueError(f"一括出力の {index} 行目の ID が重複しています")
        if (after_lat, after_lon) == ("-9999.", "-9999."):
            status = ["sea" if mode == "tky2jgd" else "unchanged"]
        else:
            position(after_lat, after_lon)
            status = ["ok", after_lat, after_lon]
        observed[key] = [latitude, longitude, *status]
    return observed


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="action", required=True)
    make = commands.add_parser("input")
    make.add_argument("zumen", type=Path)
    make.add_argument("destination", type=Path)
    check = commands.add_parser("compare")
    check.add_argument("mode", choices=("tky2jgd", "patchjgd"))
    check.add_argument("zumen", type=Path)
    check.add_argument("site", type=Path)
    args = parser.parse_args()
    try:
        expected = rows(args.zumen)
        if args.action == "input":
            data = make_input(expected)
            with args.destination.open("x", encoding="ascii") as stream:
                stream.write(data)
            return
        observed = batch_rows(args.site.read_bytes(), args.mode)
        corrected, missing, worst = compare(args.mode, expected, observed)
    except (OSError, ValueError) as error:
        parser.exit(1, f"一括照合失敗: {error}\n")
    print(f"補正後 {corrected} 点、補正量なし {missing} 点、差の最大 {worst / 100_000:.5f} 秒")
    if worst:
        parser.exit(1, "一括出力の桁で経緯度が一致しません\n")


if __name__ == "__main__":
    main()
