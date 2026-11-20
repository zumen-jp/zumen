// CSS 色文字列の検査・正規化・RGBA 変換。Canvas 2D の fillStyle 代入に頼らない。
import { valid } from "./validate.js";

const NAMED_COLORS = Object.freeze({
  aliceblue: 0xf0f8ff, antiquewhite: 0xfaebd7, aqua: 0x00ffff, aquamarine: 0x7fffd4, azure: 0xf0ffff,
  beige: 0xf5f5dc, bisque: 0xffe4c4, black: 0x000000, blanchedalmond: 0xffebcd, blue: 0x0000ff,
  blueviolet: 0x8a2be2, brown: 0xa52a2a, burlywood: 0xdeb887, cadetblue: 0x5f9ea0, chartreuse: 0x7fff00,
  chocolate: 0xd2691e, coral: 0xff7f50, cornflowerblue: 0x6495ed, cornsilk: 0xfff8dc, crimson: 0xdc143c,
  cyan: 0x00ffff, darkblue: 0x00008b, darkcyan: 0x008b8b, darkgoldenrod: 0xb8860b, darkgray: 0xa9a9a9,
  darkgreen: 0x006400, darkgrey: 0xa9a9a9, darkkhaki: 0xbdb76b, darkmagenta: 0x8b008b, darkolivegreen: 0x556b2f,
  darkorange: 0xff8c00, darkorchid: 0x9932cc, darkred: 0x8b0000, darksalmon: 0xe9967a, darkseagreen: 0x8fbc8f,
  darkslateblue: 0x483d8b, darkslategray: 0x2f4f4f, darkslategrey: 0x2f4f4f, darkturquoise: 0x00ced1, darkviolet: 0x9400d3,
  deeppink: 0xff1493, deepskyblue: 0x00bfff, dimgray: 0x696969, dimgrey: 0x696969, dodgerblue: 0x1e90ff,
  firebrick: 0xb22222, floralwhite: 0xfffaf0, forestgreen: 0x228b22, fuchsia: 0xff00ff, gainsboro: 0xdcdcdc,
  ghostwhite: 0xf8f8ff, gold: 0xffd700, goldenrod: 0xdaa520, gray: 0x808080, green: 0x008000,
  greenyellow: 0xadff2f, grey: 0x808080, honeydew: 0xf0fff0, hotpink: 0xff69b4, indianred: 0xcd5c5c,
  indigo: 0x4b0082, ivory: 0xfffff0, khaki: 0xf0e68c, lavender: 0xe6e6fa, lavenderblush: 0xfff0f5,
  lawngreen: 0x7cfc00, lemonchiffon: 0xfffacd, lightblue: 0xadd8e6, lightcoral: 0xf08080, lightcyan: 0xe0ffff,
  lightgoldenrodyellow: 0xfafad2, lightgray: 0xd3d3d3, lightgreen: 0x90ee90, lightgrey: 0xd3d3d3, lightpink: 0xffb6c1,
  lightsalmon: 0xffa07a, lightseagreen: 0x20b2aa, lightskyblue: 0x87cefa, lightslategray: 0x778899, lightslategrey: 0x778899,
  lightsteelblue: 0xb0c4de, lightyellow: 0xffffe0, lime: 0x00ff00, limegreen: 0x32cd32, linen: 0xfaf0e6,
  magenta: 0xff00ff, maroon: 0x800000, mediumaquamarine: 0x66cdaa, mediumblue: 0x0000cd, mediumorchid: 0xba55d3,
  mediumpurple: 0x9370db, mediumseagreen: 0x3cb371, mediumslateblue: 0x7b68ee, mediumspringgreen: 0x00fa9a, mediumturquoise: 0x48d1cc,
  mediumvioletred: 0xc71585, midnightblue: 0x191970, mintcream: 0xf5fffa, mistyrose: 0xffe4e1, moccasin: 0xffe4b5,
  navajowhite: 0xffdead, navy: 0x000080, oldlace: 0xfdf5e6, olive: 0x808000, olivedrab: 0x6b8e23,
  orange: 0xffa500, orangered: 0xff4500, orchid: 0xda70d6, palegoldenrod: 0xeee8aa, palegreen: 0x98fb98,
  paleturquoise: 0xafeeee, palevioletred: 0xdb7093, papayawhip: 0xffefd5, peachpuff: 0xffdab9, peru: 0xcd853f,
  pink: 0xffc0cb, plum: 0xdda0dd, powderblue: 0xb0e0e6, purple: 0x800080, rebeccapurple: 0x663399,
  red: 0xff0000, rosybrown: 0xbc8f8f, royalblue: 0x4169e1, saddlebrown: 0x8b4513, salmon: 0xfa8072,
  sandybrown: 0xf4a460, seagreen: 0x2e8b57, seashell: 0xfff5ee, sienna: 0xa0522d, silver: 0xc0c0c0,
  skyblue: 0x87ceeb, slateblue: 0x6a5acd, slategray: 0x708090, slategrey: 0x708090, snow: 0xfffafa,
  springgreen: 0x00ff7f, steelblue: 0x4682b4, tan: 0xd2b48c, teal: 0x008080, thistle: 0xd8bfd8,
  tomato: 0xff6347, turquoise: 0x40e0d0, violet: 0xee82ee, wheat: 0xf5deb3, white: 0xffffff,
  whitesmoke: 0xf5f5f5, yellow: 0xffff00, yellowgreen: 0x9acd32,
});

const NUMBER_RE = /^[+-]?(?:\d+\.?\d*|\.\d+)(?:e[+-]?\d+)?$/i;

function parseComponent(token, max) {
  const trimmed = token.trim();
  if (trimmed.endsWith("%")) {
    const raw = trimmed.slice(0, -1);
    if (!NUMBER_RE.test(raw)) return null;
    const n = Number(raw);
    if (!Number.isFinite(n)) return null;
    return Math.max(0, Math.min(max, (n / 100) * max));
  }
  if (!NUMBER_RE.test(trimmed)) return null;
  const n = Number(trimmed);
  if (!Number.isFinite(n)) return null;
  return Math.max(0, Math.min(max, n));
}

function parseAlpha(token) {
  const trimmed = token.trim();
  if (trimmed.endsWith("%")) {
    const raw = trimmed.slice(0, -1);
    if (!NUMBER_RE.test(raw)) return null;
    const n = Number(raw);
    return Number.isFinite(n) ? Math.max(0, Math.min(1, n / 100)) : null;
  }
  if (!NUMBER_RE.test(trimmed)) return null;
  const n = Number(trimmed);
  return Number.isFinite(n) ? Math.max(0, Math.min(1, n)) : null;
}

function parseAngle(token) {
  const trimmed = token.trim().toLowerCase();
  let scale = 1;
  let numStr = trimmed;
  if (trimmed.endsWith("deg")) { numStr = trimmed.slice(0, -3); scale = 1; }
  else if (trimmed.endsWith("rad")) { numStr = trimmed.slice(0, -3); scale = 180 / Math.PI; }
  else if (trimmed.endsWith("grad")) { numStr = trimmed.slice(0, -4); scale = 0.9; }
  else if (trimmed.endsWith("turn")) { numStr = trimmed.slice(0, -4); scale = 360; }
  if (!NUMBER_RE.test(numStr)) return null;
  const n = Number(numStr);
  return Number.isFinite(n) ? (((n * scale) % 360) + 360) % 360 : null;
}

function hslToRgb(h, s, l) {
  const c = (1 - Math.abs(2 * l - 1)) * s;
  const x = c * (1 - Math.abs(((h / 60) % 2) - 1));
  const m = l - c / 2;
  let r = 0, g = 0, b = 0;
  if (h < 60) { r = c; g = x; }
  else if (h < 120) { r = x; g = c; }
  else if (h < 180) { g = c; b = x; }
  else if (h < 240) { g = x; b = c; }
  else if (h < 300) { r = x; b = c; }
  else { r = c; b = x; }
  return [Math.round((r + m) * 255), Math.round((g + m) * 255), Math.round((b + m) * 255)];
}

/** CSS 色文字列を [r(0..255), g(0..255), b(0..255), a(0..1)] に解く。不正なら null。 */
export function tryParseCssColor(value) {
  if (typeof value !== "string") return null;
  const s = value.trim().toLowerCase();
  if (s === "") return null;
  if (s === "transparent") return [0, 0, 0, 0];
  if (Object.prototype.hasOwnProperty.call(NAMED_COLORS, s)) {
    const hex = NAMED_COLORS[s];
    return [(hex >> 16) & 255, (hex >> 8) & 255, hex & 255, 1];
  }
  if (s.startsWith("#")) {
    const hex = s.slice(1);
    if (!/^[\da-f]+$/.test(hex)) return null;
    if (hex.length === 3 || hex.length === 4) {
      const r = parseInt(hex[0] + hex[0], 16);
      const g = parseInt(hex[1] + hex[1], 16);
      const b = parseInt(hex[2] + hex[2], 16);
      const a = hex.length === 4 ? parseInt(hex[3] + hex[3], 16) / 255 : 1;
      return [r, g, b, a];
    }
    if (hex.length === 6 || hex.length === 8) {
      const r = parseInt(hex.slice(0, 2), 16);
      const g = parseInt(hex.slice(2, 4), 16);
      const b = parseInt(hex.slice(4, 6), 16);
      const a = hex.length === 8 ? parseInt(hex.slice(6, 8), 16) / 255 : 1;
      return [r, g, b, a];
    }
    return null;
  }
  const fnMatch = /^(rgba?|hsla?)\(([^)]+)\)$/.exec(s);
  if (!fnMatch) return null;
  const [, fn, body] = fnMatch;
  let parts, alphaPart = null;
  if (body.includes(",")) {
    if (body.includes("/")) return null;
    parts = body.split(",");
    if (parts.length !== 3 && parts.length !== 4) return null;
    if (parts.length === 4) alphaPart = parts.pop();
  } else {
    const slash = body.split("/");
    if (slash.length > 2) return null;
    parts = slash[0].trim().split(/\s+/);
    if (parts.length !== 3) return null;
    if (slash.length === 2) alphaPart = slash[1];
  }
  const a = alphaPart === null ? 1 : parseAlpha(alphaPart);
  if (a === null) return null;
  if (fn.startsWith("rgb")) {
    const isPct = parts.map((p) => p.trim().endsWith("%"));
    if (body.includes(",") && isPct[0] !== isPct[1] || body.includes(",") && isPct[0] !== isPct[2]) return null;
    const rgb = parts.map((p) => parseComponent(p, 255));
    if (rgb.some((v) => v === null)) return null;
    return [Math.round(rgb[0]), Math.round(rgb[1]), Math.round(rgb[2]), a];
  }
  const h = parseAngle(parts[0]);
  if (h === null || !parts[1].trim().endsWith("%") || !parts[2].trim().endsWith("%")) return null;
  const sat = parseComponent(parts[1], 100), lig = parseComponent(parts[2], 100);
  if (sat === null || lig === null) return null;
  const [r, g, b] = hslToRgb(h, sat / 100, lig / 100);
  return [r, g, b, a];
}

function formatNormalized([r, g, b, a]) {
  if (a >= 1) {
    const hex = ((r << 16) | (g << 8) | b).toString(16).padStart(6, "0");
    return `#${hex}`;
  }
  if (a <= 0) return "rgba(0, 0, 0, 0)";
  const cleanA = Number(a.toFixed(6));
  return `rgba(${r}, ${g}, ${b}, ${cleanA})`;
}

/** HTML Canvas の直列化と同じ正規化文字列を返し、不正な入力では RangeError を投げる。 */
export function normalizeCssColor(value, field) {
  valid(typeof value === "string", field);
  const parsed = tryParseCssColor(value);
  valid(parsed !== null, field);
  return formatNormalized(parsed);
}

/** 正規化または有効な CSS 色文字列から事前乗算済み [r*a, g*a, b*a, a]（0..1）を返す。 */
export function premultipliedRgba(style, extraAlpha = 1) {
  const parsed = tryParseCssColor(style);
  if (!parsed) return [0, 0, 0, 0];
  const a = Math.max(0, Math.min(1, parsed[3] * extraAlpha));
  return [(parsed[0] / 255) * a, (parsed[1] / 255) * a, (parsed[2] / 255) * a, a];
}
