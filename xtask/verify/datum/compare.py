"""測量計算サイトの 1 点ごとの結果と zumen の値を、点の ID と入力座標を合わせて照合する。"""

import argparse
import re
from pathlib import Path


def angle(text, maximum):
    match = re.fullmatch(r"([+-]?)([0-9]{1,3})([0-5][0-9])([0-5][0-9])\.([0-9]{5})", text)
    if not match:
        raise ValueError("経緯度には区切りなしの度分秒（秒の小数 5 桁）が必要です")
    sign, degree, minute, second, fraction = match.groups()
    ticks = ((int(degree) * 60 + int(minute)) * 60 + int(second)) * 100_000 + int(fraction)
    if ticks > maximum * 3600 * 100_000:
        raise ValueError("経緯度が範囲外です")
    return -ticks if sign == "-" else ticks


def position(latitude, longitude):
    return angle(latitude, 90), angle(longitude, 180)


def rows(path):
    result = {}
    with path.open(encoding="utf-8-sig") as stream:
        for line_number, line in enumerate(stream, 1):
            fields = line.rstrip("\r\n").split("\t")
            key = fields[0]
            if not re.fullmatch(r"[A-Za-z0-9-]+", key) or key in result:
                raise ValueError(f"{line_number} 行目の ID が不正または重複しています")
            result[key] = fields[1:]
    return result


def compare(mode, expected, observed):
    if set(expected) != set(observed):
        raise ValueError("点の ID に欠落または余分な行があります")
    if "OUT" not in expected:
        raise ValueError("範囲外の点がありません")
    corrected = 0
    outside = 0
    worst = 0
    for key, calculated in expected.items():
        if len(calculated) != 4:
            raise ValueError(f"{key} の zumen の列数が不正です")
        source = position(*calculated[:2])
        values = observed[key]
        if len(values) < 3 or position(*values[:2]) != source:
            raise ValueError(f"{key} の入力座標が一致しません")
        status = values[2]
        if status == "ok" and len(values) == 5:
            site = position(*values[3:])
        elif status in ("sea", "unchanged") and len(values) == 3:
            site = None
        else:
            raise ValueError(f"{key} のサイト結果の状態または列数が不正です")
        missing = calculated[2:] == ["-", "-"]
        if key == "OUT" and not missing:
            raise ValueError("範囲外の点に補正量があります")
        if missing:
            valid = status == "sea" if mode == "tky2jgd" else (
                status == "unchanged" or site == source
            )
            if not valid:
                raise ValueError(f"{key} の補正量がない場合の扱いが一致しません")
            outside += 1
            continue
        if site is None:
            raise ValueError(f"{key} でサイトの補正結果がありません")
        expected_result = position(*calculated[2:])
        worst = max(worst, *(abs(a - b) for a, b in zip(expected_result, site)))
        corrected += 1
    if not corrected:
        raise ValueError("補正後の値を比較した点がありません")
    return corrected, outside, worst


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=("tky2jgd", "patchjgd"))
    parser.add_argument("zumen", type=Path)
    parser.add_argument("site", type=Path)
    args = parser.parse_args()
    try:
        corrected, outside, worst = compare(args.mode, rows(args.zumen), rows(args.site))
    except (OSError, ValueError) as error:
        parser.exit(1, f"照合失敗: {error}\n")
    print(f"補正後 {corrected} 点、補正量なし {outside} 点、差の最大 {worst / 100_000:.5f} 秒")
    if worst:
        parser.exit(1, "サイトの表示の桁で補正後の経緯度が一致しません\n")


if __name__ == "__main__":
    main()
