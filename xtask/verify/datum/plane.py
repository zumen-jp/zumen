"""平面直角座標の一括出力を読み、TKY2JGD、PatchJGD と連続変換の差を集計する。"""

import argparse
import re
from pathlib import Path

from batch import batch_fields


HEADER = "id tokyo_x tokyo_y tky_x tky_y patch_x patch_y chain_x chain_y".split()


def ticks(value):
    if not re.fullmatch(r"[+-]?[0-9]{1,9}\.[0-9]{4}", value):
        raise ValueError("平面直角座標の列には m の小数 4 桁が必要です")
    return int(value.replace(".", ""))


def metres(value):
    magnitude = abs(value)
    return f"{'-' if value < 0 else ''}{magnitude // 10_000}.{magnitude % 10_000:04}"


def positions(values):
    return tuple((ticks(x), ticks(y)) for x, y in zip(values[::2], values[1::2], strict=True))


def expected_rows(path):
    with path.open(encoding="ascii") as stream:
        if stream.readline().split() != HEADER:
            raise ValueError("expected.tsv の見出しが一致しません")
        result = {}
        for line in stream:
            fields = line.split()
            if len(fields) != len(HEADER) or fields[0] in result:
                raise ValueError("expected.tsv の列数または点名の重複を確認してください")
            result[fields[0]] = positions(fields[1:])
    if set(result) != {"P1", "P2"}:
        raise ValueError("expected.tsv に 2 点の期待値が必要です")
    return result


def site_rows(path, expected_sources):
    result = {}
    for number, fields in batch_fields(path.read_bytes()):
        key = fields[-1]
        if key not in expected_sources or key in result:
            raise ValueError(f"一括出力の {number} 行目の点名が不正または重複しています")
        source, after = positions(fields[:-1])
        if source != expected_sources[key]:
            raise ValueError(f"一括出力の {number} 行目の入力座標が一致しません")
        result[key] = after
    if result.keys() != expected_sources.keys():
        raise ValueError("一括出力に足りない点があります")
    return result


def worst(expected, observed):
    return max(abs(a - b) for k in expected for a, b in zip(expected[k], observed[k]))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="action", required=True)
    for name in ("chain-input", "compare"):
        command = commands.add_parser(name)
        command.add_argument("expected", type=Path)
        command.add_argument("tky", type=Path)
        if name == "chain-input":
            command.add_argument("destination", type=Path)
        else:
            command.add_argument("patch", type=Path)
            command.add_argument("chain", type=Path)
    args = parser.parse_args()
    try:
        expected = expected_rows(args.expected)
        source, middle, single, full = (
            {key: stages[index] for key, stages in expected.items()} for index in range(4)
        )
        tky = site_rows(args.tky, source)
        if args.action == "chain-input":
            lines = [f"{metres(x)} {metres(y)} {key}\r\n" for key, (x, y) in tky.items()]
            with args.destination.open("xb") as stream:
                stream.write("".join(lines).encode("ascii"))
            return
        patch = site_rows(args.patch, middle)
        chain = site_rows(args.chain, tky)
        differences = (worst(middle, tky), worst(single, patch), worst(full, chain))
        for name, value in zip(("TKY2JGD", "PatchJGD", "連続変換"), differences):
            print(f"{name} {len(expected)} 点、座標成分の差の最大 {metres(value)} m")
        print(f"zumen で中間座標を丸めた場合と丸めない場合の差の最大 {metres(worst(single, full))} m")
        print(f"サイトで中間座標の入力を替えた場合の差の最大 {metres(worst(patch, chain))} m")
        if any(value > limit for value, limit in zip(differences, (1, 1, 2))):
            parser.exit(1, "座標成分の差が再照合の上限を超えました\n")
    except (OSError, ValueError) as error:
        parser.exit(1, f"照合失敗: {error}\n")


if __name__ == "__main__":
    main()
