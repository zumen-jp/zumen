"""組み込み色表を、別に入れた MIT の実装の全符号と照らす。"""

from importlib.metadata import version
from pathlib import Path
import sys

from ezdxf.colors import DXF_DEFAULT_COLORS
from ezjww import new_jww_document
from ezsxf._drawing_style import _PREDEFINED_COLORS


def rgb(value: int) -> str:
    return f"#{value:06x}"


def main() -> None:
    for package, expected in [("ezdxf", "1.4.5"), ("ezjww", "0.7.6"), ("ezsxf", "0.3.7")]:
        if version(package) != expected:
            raise SystemExit(f"{package} {expected} が必要")
    expected = {
        ("aci", code): rgb(DXF_DEFAULT_COLORS[code]) for code in range(1, 256)
    }
    expected.update({
        ("sxf", code): "#" + "".join(f"{v:02x}" for v in value)
        for code, value in _PREDEFINED_COLORS.items()
    })
    # COLORREF は下位バイトが R。文書の画面色だけを読み、テンプレートは保存しない。
    for code, color in enumerate(new_jww_document()["options"]["palette"]["pen_colors"]):
        if 1 <= code <= 9:
            expected["jww", code] = "#" + "".join(f"{(color >> shift) & 255:02x}" for shift in [0, 8, 16])
    expected["jww", 100] = "#000000"
    for code in range(1, 17):
        expected["jww", code + 100] = expected["sxf", code]
    actual = {}
    for line in Path(sys.argv[1]).read_text(encoding="utf-8-sig").splitlines():
        system, code, color = line.split()
        key = (system, int(code))
        if key in actual:
            raise SystemExit(f"符号が重なった: {key}")
        actual[key] = color
    if actual != expected:
        mismatch = sorted(key for key in actual.keys() | expected.keys() if actual.get(key) != expected.get(key))
        raise SystemExit(f"色表が異なる: {mismatch}")
    print(f"{len(actual)} 符号が一致（ACI 255・SXF 16・JWW 26）")


if __name__ == "__main__":
    main()
