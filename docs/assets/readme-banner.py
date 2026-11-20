"""README の見出しの画像（readme-banner-light.svg・readme-banner-dark.svg）を作る。

文字は Noto Sans JP（SIL OFL 1.1）の輪郭にして、閲覧環境の字体に頼らない。字体の道筋は FONT を
手元に合わせる（Windows 11 には入っている）。平面図はサイトの見出しの図（site/src/components/PlanDrawing.astro）。

使い方: uv run --no-project --with fonttools python -X utf8 docs/assets/readme-banner.py docs/assets
"""

import sys
from pathlib import Path

from fontTools.pens.svgPathPen import SVGPathPen
from fontTools.pens.transformPen import TransformPen
from fontTools.ttLib import TTFont
from fontTools.varLib.instancer import instantiateVariableFont

FONT = "C:/Windows/Fonts/NotoSansJP-VF.ttf"
_fonts = {}


def font(weight):
    if weight not in _fonts:
        f = TTFont(FONT)
        _fonts[weight] = instantiateVariableFont(f, {"wght": weight})
    return _fonts[weight]


def measure(s, size, weight, tracking=0.0):
    f = font(weight)
    scale = size / f["head"].unitsPerEm
    cmap, hmtx = f.getBestCmap(), f["hmtx"]
    return sum(hmtx[cmap[ord(c)]][0] * scale + tracking for c in s) - tracking


def text(s, size, weight, x, y, anchor="start", tracking=0.0):
    """文字列を輪郭の path の d にする。y はベースライン。"""
    f = font(weight)
    scale = size / f["head"].unitsPerEm
    cmap, hmtx, gs = f.getBestCmap(), f["hmtx"], f.getGlyphSet()
    width = measure(s, size, weight, tracking)
    if anchor == "middle":
        x -= width / 2
    elif anchor == "end":
        x -= width
    pen = SVGPathPen(gs, ntos=lambda v: f"{v:.1f}".rstrip("0").rstrip("."))
    for c in s:
        name = cmap[ord(c)]
        gs[name].draw(TransformPen(pen, (scale, 0, 0, -scale, x, y)))
        x += hmtx[name][0] * scale + tracking
    return pen.getCommands()


def cap_center(size, weight):
    """数字・大文字の高さの半分（ベースラインからの上げ幅）。"""
    return font(weight)["OS/2"].sCapHeight * size / font(weight)["head"].unitsPerEm / 2


W, H = 1280, 420

THEMES = {
    "light": dict(
        bg="#f6f8fc", edge="#d5dbe8", grid="#0017c1", grid_op=0.045, frame="#0017c1",
        ink="#0f172a", brand="#0017c1", muted="#4b5563", axis="#c4161c", wall="#1a1a1a",
        win="#f6f8fc", dim="#0017c1", room="#1a1a1a", area="#666666",
    ),
    "dark": dict(
        bg="#0a1a42", edge="#1f3a7a", grid="#9cbcff", grid_op=0.07, frame="#7fa6ff",
        ink="#ffffff", brand="#a9c4ff", muted="#b4c3e6", axis="#ff8f94", wall="#e3eaf7",
        win="#0a1a42", dim="#7fa6ff", room="#e3eaf7", area="#b4c3e6",
    ),
}


def plan(t):
    """サイトの見出しの住戸の平面図（site/src/components/PlanDrawing.astro）を小さくして描く。"""
    s, ox, oy = 0.5, 888, 132  # 縮尺と、通り芯 X1・Y1 の交点の位置
    tf = f'transform="translate({ox} {oy}) scale({s})"'
    bubbles = [(0, -152, "X1"), (364, -152, "X2"), (637, -152, "X3"),
               (-150, 0, "Y1"), (-150, 273, "Y2"), (-150, 637, "Y3")]
    labels = "".join(
        f'<path d="{text(n, 20, 700, x, y + cap_center(20, 700), "middle")}" fill="{t["axis"]}"/>'
        for x, y, n in bubbles)
    rooms = "".join(
        f'<path d="{text(n, sz, wt, x, y, "middle")}" fill="{c}"/>'
        for x, y, n, sz, wt, c in [
            (182, 140, "洋室", 40, 700, t["room"]), (182, 186, "6帖", 28, 400, t["area"]),
            (500, 196, "浴室・洗面", 34, 700, t["room"]),
            (318, 470, "LDK", 44, 700, t["room"]), (318, 516, "14帖", 28, 400, t["area"]),
            (586, 494, "玄関", 28, 400, t["area"])])
    dims = "".join(
        f'<path d="{text(n, 24, 500, 0, 0, "middle")}" fill="{t["dim"]}" transform="translate({x} {y}) rotate({r})"/>'
        for x, y, r, n in [(182, -70, 0, "3,640"), (500, -70, 0, "2,730"), (318, -110, 0, "6,370"),
                           (-70, 136, -90, "2,730"), (-70, 455, -90, "3,640"), (-110, 318, -90, "6,370")])
    return f'''<g {tf}>
    <g stroke="{t["axis"]}" stroke-width="1.6" stroke-dasharray="18 5 3 5" fill="none" opacity=".8">
      <path d="M0 -128V672M364 -128V672M637 -128V672M-126 0H672M-126 273H672M-126 637H672"/>
    </g>
    <g fill="{t["bg"]}" stroke="{t["axis"]}" stroke-width="2">{"".join(f'<circle cx="{x}" cy="{y}" r="24"/>' for x, y, _ in bubbles)}</g>
    {labels}
    <g stroke="{t["dim"]}" stroke-width="1.4" fill="none">
      <path d="M0 -14V-110M364 -14V-70M637 -14V-110M0 -60H637M0 -100H637M-14 0H-110M-14 273H-70M-14 637H-110M-60 0V637M-100 0V637"/>
      <path stroke-width="2.6" d="M-7 -53L7 -67M357 -53L371 -67M630 -53L644 -67M-7 -93L7 -107M630 -93L644 -107M-67 7L-53 -7M-67 280L-53 266M-67 644L-53 630M-107 7L-93 -7M-107 644L-93 630"/>
    </g>
    {dims}
    <g fill="{t["wall"]}">
      <rect x="-6" y="-6" width="106" height="12"/><rect x="260" y="-6" width="383" height="12"/>
      <rect x="-6" y="631" width="86" height="12"/><rect x="300" y="631" width="80" height="12"/>
      <rect x="560" y="631" width="83" height="12"/><rect x="-6" y="-6" width="12" height="406"/>
      <rect x="-6" y="540" width="12" height="103"/><rect x="631" y="-6" width="12" height="436"/>
      <rect x="631" y="520" width="12" height="123"/><rect x="359" y="0" width="10" height="273"/>
      <rect x="0" y="268" width="60" height="10"/><rect x="140" y="268" width="300" height="10"/>
      <rect x="520" y="268" width="117" height="10"/>
    </g>
    <g fill="{t["win"]}" stroke="{t["wall"]}" stroke-width="1.6">
      <rect x="100" y="-6" width="160" height="12"/><rect x="80" y="631" width="220" height="12"/>
      <rect x="380" y="631" width="180" height="12"/><rect x="-6" y="400" width="12" height="140"/>
    </g>
    <g stroke="{t["wall"]}" stroke-width="1.6" fill="none">
      <path d="M100 0H260M80 637H300M380 637H560M0 400V540M60 273V193M520 273V193M631 430H541"/>
      <path stroke-dasharray="5 4" d="M60 193A80 80 0 0 1 140 273M520 193A80 80 0 0 0 440 273M541 430A90 90 0 0 0 631 520"/>
      <rect x="470" y="22" width="148" height="92" rx="10"/><rect x="160" y="288" width="240" height="56"/>
      <circle cx="230" cy="316" r="12"/><circle cx="330" cy="316" r="16"/>
    </g>
    {rooms}
  </g>'''


def banner(t):
    # 図面の枠: 外枠と内枠、辺の中央の中心マーク、区分の目盛り
    ticks = "".join(f"M{x:.1f} 28V40M{x:.1f} 380V392" for x in [28 + 1224 * i / 8 for i in range(1, 8)])
    ticks += "".join(f"M28 {y:.1f}H40M1240 {y:.1f}H1252" for y in [28 + 364 * i / 4 for i in range(1, 4)])
    left = 96
    eyebrow = text("RUST  ·  MIT / APACHE-2.0  ·  MCP", 15, 700, left + 2, 118, tracking=2.2)
    word = text("zumen", 132, 800, left - 6, 236, tracking=-3)
    tagline = text("日本の図面のための、オープンライブラリ。", 34, 700, left, 298)
    sub = text("SXF・Jw_cad・DXF・DWG・IFC・LandXML・点群・電子納品", 19, 400, left, 340)
    return f'''<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W} {H}" width="{W}" height="{H}" role="img" aria-label="zumen — 日本の図面のための、オープンライブラリ。">
  <defs>
    <pattern id="g" width="20" height="20" patternUnits="userSpaceOnUse"><path d="M20 0H0V20" fill="none" stroke="{t["grid"]}" stroke-width="1"/></pattern>
    <clipPath id="c"><rect x="40" y="40" width="1200" height="340"/></clipPath>
  </defs>
  <rect x=".5" y=".5" width="{W - 1}" height="{H - 1}" rx="18" fill="{t["bg"]}" stroke="{t["edge"]}"/>
  <rect x="40" y="40" width="1200" height="340" fill="url(#g)" opacity="{t["grid_op"] / 0.045 * 0.045}"/>
  <g fill="none" stroke="{t["frame"]}">
    <rect x="28" y="28" width="1224" height="364" stroke-width="2"/>
    <rect x="40" y="40" width="1200" height="340" stroke-width="1"/>
    <path stroke-width="1" d="{ticks}"/>
    <path stroke-width="2.4" d="M640 12V56M640 364V408M12 210H56M1224 210H1268"/>
  </g>
  <path d="{eyebrow}" fill="{t["brand"]}"/>
  <path d="{word}" fill="{t["ink"]}"/>
  <rect x="{left}" y="256" width="56" height="5" fill="{t["brand"]}"/>
  <path d="{tagline}" fill="{t["ink"]}"/>
  <path d="{sub}" fill="{t["muted"]}"/>
  <g clip-path="url(#c)">{plan(t)}</g>
</svg>
'''


out = Path(sys.argv[1])
out.mkdir(parents=True, exist_ok=True)
for name, t in THEMES.items():
    (out / f"readme-banner-{name}.svg").write_text(banner(t), encoding="utf-8", newline="\n")
print("ok")
