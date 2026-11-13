//! 書き方の違う図形を足した入力を作る。今の実装と照らす範囲を、合成の図面や公開サンプルにない書き方へ広げる。
//!
//! 線・円・円弧・折れ線・文字・ブロックの挿入（と、続く ATTRIB・SEQEND）を、任意のタグの有無、数の書式、拡張データ、
//! 押し出しの向き、読めない値などを乱数で変えながら、ENTITIES のセクションの終わりに足す。挿入が指すブロックの定義、
//! 名前の表のレコード、足した図形を指すオブジェクト（辞書とリアクター）も足す。新しいハンドルを振り、`$HANDSEED` を直す。
//! 図面の全体の書き方（`$HANDSEED` の有無、セクションの数、属性の付き方）は、変種で選ぶ。
//! 照らすための入力であり、ほかのアプリケーションに渡す図面ではない。

use std::ops::Range;

use zumen_bench_live::grow;
use zumen_bench_live::scan::{self, Next};
use zumen_bench_live::store::Document;
use zumen_bench_live::value::format_real;

use crate::random::Random;

/// 図形を書き出す先
struct Writer {
    out: Vec<u8>,
    eol: &'static [u8],
    /// グループ・コードを 3 桁に右寄せする
    padded: bool,
    random: Random,
    /// この図形の実数のどれか 1 つを、読めない値にする
    spoil: bool,
}

/// 実数を、指数の桁を 2 桁にした表記にする（`1.250000E+01`）
fn exponent(value: f64) -> String {
    let text = format!("{value:.6E}");
    let Some((mantissa, power)) = text.split_once('E') else {
        return text;
    };
    let power: i32 = power.parse().unwrap_or(0);
    let sign = if power < 0 { '-' } else { '+' };
    format!("{mantissa}E{sign}{:02}", power.unsigned_abs())
}

impl Writer {
    fn raw(&mut self, code: i16, value: &str) {
        let line = if self.padded {
            format!("{code:>3}")
        } else {
            code.to_string()
        };
        self.out.extend_from_slice(line.as_bytes());
        self.out.extend_from_slice(self.eol);
        self.out.extend_from_slice(value.as_bytes());
        self.out.extend_from_slice(self.eol);
    }

    /// 実数の書式を、乱数で選ぶ。どれも、読むと同じ値になる
    fn format(&mut self, value: f64) -> String {
        let shortest = String::from_utf8_lossy(&format_real(value)).into_owned();
        let whole = value == value.trunc() && value.abs() < 1e15;
        match self.random.below(24) {
            0 | 1 => format!("{value:.6}"),
            2 => format!("{value:.16}"),
            3 => format!("{value:E}"),
            4 => exponent(value),
            5 if whole => format!("{}", value as i64),
            6 => format!("   {shortest}"),
            7 => format!("{shortest}  "),
            8 if value > 0.0 => format!("+{shortest}"),
            9 if value != 0.0 && value.abs() < 1.0 => shortest.replacen("0.", ".", 1),
            10 if value == 0.0 => "-0.0".to_owned(),
            _ => shortest,
        }
    }

    fn real(&mut self, code: i16, value: f64) {
        if self.spoil && self.random.below(3) == 0 {
            self.spoil = false;
            let broken = ["abc", "", "nan", "inf", "1e999", "1,5", "0x10", "1.0.0"];
            let text = broken
                .get(self.random.below(broken.len()))
                .copied()
                .unwrap_or("abc");
            self.raw(code, text);
            return;
        }
        // 小数の桁の多い書式（`{:.6}`）は値を丸める。書いた文字列が、その図形の値
        let text = self.format(value);
        self.raw(code, &text);
    }

    fn int(&mut self, code: i16, value: i64) {
        let text = match self.random.below(4) {
            0 => value.to_string(),
            1 => format!("{value:>9}"),
            _ => format!("{value:>6}"),
        };
        self.raw(code, &text);
    }

    fn chance(&mut self, one_in: usize) -> bool {
        self.random.below(one_in) == 0
    }

    fn pick<'a>(&mut self, items: &[&'a str]) -> &'a str {
        items
            .get(self.random.below(items.len()))
            .copied()
            .unwrap_or("")
    }

    fn pick_int(&mut self, items: &[i64]) -> i64 {
        items
            .get(self.random.below(items.len()))
            .copied()
            .unwrap_or(0)
    }

    fn pick_real(&mut self, items: &[f64]) -> f64 {
        self.random.pick(items)
    }

    /// 升目の中の座標。整数・1/8 の倍数・桁の多い値・大きい値・小さい値・0 を混ぜる
    fn coordinate(&mut self, base: f64) -> f64 {
        match self.random.below(16) {
            0 => base,
            1 | 2 => base + self.random.below(80) as f64 / 8.0,
            3 => (base + self.random.unit() * 10.0) * 1e6,
            4 => self.random.between(-1.0, 1.0) * 1e-7,
            5 => 0.0,
            6 => -(base + self.random.unit() * 10.0),
            _ => base + self.random.unit() * 10.0,
        }
    }

    /// z の値。書かない・0 を書く・0 でない値を書く
    fn z(&mut self, code: i16) {
        match self.random.below(40) {
            0 => self.real(code, 2.5),
            1..=18 => {}
            _ => self.real(code, 0.0),
        }
    }

    fn point(&mut self, codes: [i16; 3], origin: [f64; 2]) {
        let x = self.coordinate(origin[0]);
        let y = self.coordinate(origin[1]);
        self.real(codes[0], x);
        self.real(codes[1], y);
        self.z(codes[2]);
    }

    /// 厚さ
    fn thickness(&mut self) {
        if self.chance(10) {
            let value = self.pick_real(&[0.0, 1.5, -2.0]);
            self.real(39, value);
        }
    }

    /// 押し出しの向き。書かない・既定の値を書く・既定でない値を書く
    fn extrusion(&mut self) {
        let normal = match self.random.below(30) {
            0..=3 => [0.0, 0.0, 1.0],
            4 => [0.0, 0.0, -1.0],
            5 => [0.6, 0.0, 0.8],
            _ => return,
        };
        self.real(210, normal[0]);
        self.real(220, normal[1]);
        self.real(230, normal[2]);
    }

    /// 拡張データ
    fn xdata(&mut self, origin: [f64; 2]) {
        if !self.chance(4) {
            return;
        }
        self.raw(1001, "ACAD");
        for _ in 0..1 + self.random.below(6) {
            match self.random.below(13) {
                0 => self.raw(1000, "note 1"),
                1 => {
                    self.raw(1002, "{");
                    self.int(1070, 3);
                    self.raw(1002, "}");
                }
                2 => self.real(1040, 1.5),
                3 => {
                    let value = self.pick_real(&[2.5, 0.0, -4.0, 100.125]);
                    self.real(1041, value);
                }
                4 => {
                    let value = self.pick_real(&[0.5, 1.0, 2.0, -1.0]);
                    self.real(1042, value);
                }
                5..=8 => {
                    let first: i16 = match self.random.below(4) {
                        0 => 1010,
                        1 => 1011,
                        2 => 1012,
                        _ => 1013,
                    };
                    let x = self.coordinate(origin[0]);
                    let y = self.coordinate(origin[1]);
                    self.real(first, x);
                    // 3 つそろわない点も混ぜる
                    if !self.chance(10) {
                        self.real(first + 10, y);
                        if !self.chance(10) {
                            let z = self.pick_real(&[0.0, 0.0, 3.0]);
                            self.real(first + 20, z);
                        }
                    }
                }
                9 => self.int(1071, 100_000),
                10 => self.raw(1005, "1F"),
                11 => self.raw(1004, "0A0B0C"),
                _ => self.raw(1003, "0"),
            }
        }
    }
}

/// 足す図形に共通の値
struct Common<'a> {
    /// モデル空間のブロック・レコードのハンドル（元の図形から写す）
    owner: Option<&'a str>,
    /// 図形が使う画層
    layers: &'a [String],
}

/// ほかの図形を指すリアクターと拡張ディクショナリの組を書く。`made` は、先に足した図形のハンドル
fn references(writer: &mut Writer, made: &[u64]) {
    if made.is_empty() {
        return;
    }
    let pick = |writer: &mut Writer| {
        let handle = made
            .get(writer.random.below(made.len()))
            .copied()
            .unwrap_or(0);
        format!("{handle:X}")
    };
    if writer.chance(6) {
        writer.raw(102, "{ACAD_REACTORS");
        for _ in 0..1 + writer.random.below(3) {
            let handle = pick(writer);
            writer.raw(330, &handle);
        }
        writer.raw(102, "}");
    }
    if writer.chance(14) {
        writer.raw(102, "{ACAD_XDICTIONARY");
        let handle = pick(writer);
        writer.raw(360, &handle);
        writer.raw(102, "}");
    }
}

/// 図形の先頭の、種類に依らないタグを書く。サブクラスマーカーを書くかどうかを返す
fn head(writer: &mut Writer, kind: &str, handle: u64, common: &Common, made: &[u64]) -> bool {
    writer.raw(0, kind);
    let handle = if writer.chance(10) {
        format!("{handle:x}")
    } else {
        format!("{handle:X}")
    };
    writer.raw(5, &handle);
    references(writer, made);
    let paper = writer.chance(40);
    if let Some(owner) = common.owner
        && !paper
        && !writer.chance(3)
    {
        writer.raw(330, owner);
    }
    let markers = !writer.chance(8);
    if markers {
        writer.raw(100, "AcDbEntity");
    }
    if paper {
        writer.int(67, 1);
    } else if writer.chance(20) {
        writer.int(67, 0);
    }
    let layer = common
        .layers
        .get(writer.random.below(common.layers.len().max(1)))
        .map_or("0", String::as_str);
    writer.raw(8, layer);
    if writer.chance(6) {
        let name = writer.pick(&[
            "ByLayer",
            "BYLAYER",
            "ByBlock",
            "Continuous",
            "CONTINUOUS",
            "CASE_DASH",
        ]);
        writer.raw(6, name);
    }
    if writer.chance(3) {
        let color = writer.pick_int(&[0, 1, 2, 7, 8, 255, 256, 257, -5]);
        writer.int(62, color);
    }
    if writer.chance(6) {
        let weight = writer.pick_int(&[-3, -2, -1, 0, 5, 9, 13, 25, 211, 212, 7]);
        writer.int(370, weight);
    }
    if writer.chance(8) {
        let scale = writer.pick_real(&[1.0, 0.5, 2.0, 0.0, -1.0]);
        writer.real(48, scale);
    }
    if writer.chance(12) {
        let hidden = writer.pick_int(&[0, 1]);
        writer.int(60, hidden);
    }
    if writer.chance(8) {
        let color = writer.pick_int(&[0xFF_8000, 0, 0xFF_FFFF, 0x0101_0101]);
        writer.int(420, color);
    }
    if writer.chance(10) {
        let alpha = writer.pick_int(&[0x0200_00FF, 0x0200_007F, 0x0100_0000, 0, 0x0300_0000]);
        writer.int(440, alpha);
    }
    markers
}

fn line(writer: &mut Writer, origin: [f64; 2], markers: bool) {
    if markers {
        writer.raw(100, "AcDbLine");
    }
    writer.thickness();
    writer.point([10, 20, 30], origin);
    writer.point([11, 21, 31], origin);
    writer.extrusion();
}

fn circle(writer: &mut Writer, origin: [f64; 2], markers: bool, arc: bool) {
    if markers {
        writer.raw(100, "AcDbCircle");
    }
    writer.thickness();
    writer.point([10, 20, 30], origin);
    let radius = writer.pick_real(&[1.0, 2.5, 0.125, 3.3333333333333335, 1e-6, 0.0, -1.0, 1e5]);
    writer.real(40, radius);
    writer.extrusion();
    if arc {
        if markers {
            writer.raw(100, "AcDbArc");
        }
        let free = writer.random.between(-720.0, 720.0);
        let start = writer.pick_real(&[0.0, 90.0, 45.0, 359.9999, 360.0, -90.0, 450.0, free]);
        let free = writer.random.between(-720.0, 720.0);
        let end = writer.pick_real(&[90.0, 180.0, 0.0, 360.0, -45.0, 720.0, free]);
        writer.real(50, start);
        writer.real(51, end);
    }
}

fn polyline(writer: &mut Writer, origin: [f64; 2], markers: bool) {
    if markers {
        writer.raw(100, "AcDbPolyline");
    }
    let count = match writer.random.below(20) {
        0 => 0,
        1 => 1,
        _ => 2 + writer.random.below(5),
    };
    // 頂点の数と合わない 90 も混ぜる
    let declared = if writer.chance(40) { count + 1 } else { count };
    writer.int(90, declared as i64);
    let flags = writer.pick_int(&[0, 0, 1, 1, 128, 129]);
    writer.int(70, flags);
    if writer.chance(4) {
        let width = writer.pick_real(&[0.0, 0.0, 0.25, 2.0, -1.0]);
        writer.real(43, width);
    }
    if writer.chance(10) {
        let elevation = writer.pick_real(&[0.0, 0.0, 0.0, 5.0]);
        writer.real(38, elevation);
    }
    writer.thickness();
    for _ in 0..count {
        let x = writer.coordinate(origin[0]);
        let y = writer.coordinate(origin[1]);
        writer.real(10, x);
        writer.real(20, y);
        if writer.chance(6) {
            let start = writer.pick_real(&[0.0, 0.5, 1.0]);
            let end = writer.pick_real(&[0.0, 0.5, 2.0]);
            writer.real(40, start);
            writer.real(41, end);
        }
        if writer.chance(3) {
            let free = writer.random.between(-2.0, 2.0);
            let bulge = writer.pick_real(&[1.0, -1.0, 0.5, -0.25, 0.0, 0.41421356237309503, free]);
            writer.real(42, bulge);
        }
        if writer.chance(12) {
            writer.int(91, 7);
        }
    }
    writer.extrusion();
}

fn text(writer: &mut Writer, origin: [f64; 2], markers: bool) {
    if markers {
        writer.raw(100, "AcDbText");
    }
    writer.thickness();
    writer.point([10, 20, 30], origin);
    let height = writer.pick_real(&[2.5, 0.2, 1.0, 3.3333333333333335, 0.0, -1.0]);
    writer.real(40, height);
    let content = writer.pick(&[
        "ABC",
        "note 12",
        "",
        " padded ",
        "45%%d",
        "%%c20 %%uunder%%u",
        "a\\b",
        "\\U+00B0",
        "{braces}",
        "x;y^z",
    ]);
    writer.raw(1, content);
    if writer.chance(3) {
        let free = writer.random.between(-720.0, 720.0);
        let angle = writer.pick_real(&[0.0, 90.0, 45.0, -30.0, 360.0, free]);
        writer.real(50, angle);
    }
    if writer.chance(5) {
        let factor = writer.pick_real(&[1.0, 0.8, 2.0, 0.0, -1.0]);
        writer.real(41, factor);
    }
    if writer.chance(6) {
        let oblique = writer.pick_real(&[0.0, 15.0, -15.0, 85.0, 90.0]);
        writer.real(51, oblique);
    }
    if writer.chance(4) {
        let style = writer.pick(&["Standard", "STANDARD", "missing-style"]);
        writer.raw(7, style);
    }
    if writer.chance(8) {
        let flags = writer.pick_int(&[0, 2, 4, 6]);
        writer.int(71, flags);
    }
    let horizontal = if writer.chance(2) {
        let value = writer.pick_int(&[0, 1, 2, 3, 4, 5, 6]);
        writer.int(72, value);
        value
    } else {
        0
    };
    // 2 つめの点は、揃えがあるときに多く書く。ないのに書くもの、あるのに書かないものも混ぜる
    let second = if horizontal != 0 {
        !writer.chance(6)
    } else {
        writer.chance(6)
    };
    if second {
        writer.point([11, 21, 31], origin);
    }
    writer.extrusion();
    if markers && !writer.chance(6) {
        writer.raw(100, "AcDbText");
    }
    if writer.chance(3) {
        let vertical = writer.pick_int(&[0, 1, 2, 3, 4]);
        writer.int(73, vertical);
    }
}

/// ATTRIB を書く。`parent` は、付く INSERT のハンドル
fn attrib(
    writer: &mut Writer,
    origin: [f64; 2],
    markers: bool,
    handle: u64,
    parent: u64,
    common: &Common,
    loose: bool,
) {
    writer.raw(0, "ATTRIB");
    writer.raw(5, &format!("{handle:X}"));
    // 所有者。書かないもの、親でないものを指すもの（付き方がそろわない）も混ぜる
    match writer.random.below(8) {
        0 => {}
        1 if loose => {
            if let Some(owner) = common.owner {
                writer.raw(330, owner);
            }
        }
        _ => writer.raw(330, &format!("{parent:X}")),
    }
    if markers {
        writer.raw(100, "AcDbEntity");
    }
    let layer = common
        .layers
        .get(writer.random.below(common.layers.len().max(1)))
        .map_or("0", String::as_str);
    writer.raw(8, layer);
    if writer.chance(5) {
        let color = writer.pick_int(&[1, 3, 256, 0]);
        writer.int(62, color);
    }
    if markers {
        writer.raw(100, "AcDbText");
    }
    writer.point([10, 20, 30], origin);
    let height = writer.pick_real(&[2.5, 0.2, 1.0, 3.3333333333333335, 0.0]);
    writer.real(40, height);
    let value = writer.pick(&["A-1", "", "value 2", "%%c5", " padded "]);
    writer.raw(1, value);
    if writer.chance(3) {
        let free = writer.random.between(-720.0, 720.0);
        let angle = writer.pick_real(&[0.0, 90.0, 45.0, -30.0, free]);
        writer.real(50, angle);
    }
    if writer.chance(6) {
        let factor = writer.pick_real(&[1.0, 0.8, 2.0, 0.0]);
        writer.real(41, factor);
    }
    if writer.chance(8) {
        let oblique = writer.pick_real(&[0.0, 15.0, -15.0]);
        writer.real(51, oblique);
    }
    if writer.chance(5) {
        let style = writer.pick(&["Standard", "CASE_STYLE", "missing-style"]);
        writer.raw(7, style);
    }
    if writer.chance(10) {
        let flags = writer.pick_int(&[0, 2, 4]);
        writer.int(71, flags);
    }
    let horizontal = if writer.chance(2) {
        let value = writer.pick_int(&[0, 1, 2, 3, 4, 5]);
        writer.int(72, value);
        value
    } else {
        0
    };
    let second = if horizontal != 0 {
        !writer.chance(6)
    } else {
        writer.chance(8)
    };
    if second {
        writer.point([11, 21, 31], origin);
    }
    writer.extrusion();
    if markers {
        writer.raw(100, "AcDbAttribute");
    }
    if writer.chance(4) {
        writer.int(280, 0);
    }
    let tag = writer.pick(&["TAG", "NO", "tag2", ""]);
    writer.raw(2, tag);
    let flags = writer.pick_int(&[0, 0, 0, 1, 2, 4, 8]);
    writer.int(70, flags);
    if writer.chance(6) {
        writer.int(73, 0);
    }
    if writer.chance(3) {
        let vertical = writer.pick_int(&[0, 1, 2, 3, 4]);
        writer.int(74, vertical);
    }
    if writer.chance(8) {
        writer.int(280, 1);
    }
    writer.xdata(origin);
}

/// INSERT が指す名前。定義のない名前と、大文字と小文字の違う綴りも混ぜる
const REFERENCES: [&str; 13] = [
    "CASE_PLAIN",
    "CASE_PLAIN",
    "CASE_PLAIN",
    "CASE_BASE",
    "CASE_BASE",
    "CASE_ATTDEF",
    "CASE_CONST",
    "CASE_EMPTY",
    "case_lower",
    "CASE_NEST",
    "CASE_TWICE",
    "case_plain",
    "CASE_MISSING",
];

/// INSERT の値と、続く ATTRIB・SEQEND を書く。`loose` なら、付き方のそろわないものも混ぜる
fn insert(
    writer: &mut Writer,
    origin: [f64; 2],
    markers: bool,
    handle: u64,
    next: &mut u64,
    common: &Common,
    loose: bool,
) {
    let attribs = match writer.random.below(10) {
        0..=4 => 0,
        5..=7 => 1,
        8 => 2,
        _ => 3,
    };
    let grid = writer.chance(6);
    if markers {
        let class = if grid && writer.chance(2) {
            "AcDbMInsertBlock"
        } else {
            "AcDbBlockReference"
        };
        writer.raw(100, class);
    }
    // 属性が続く印（66）。続かないのに書くもの、0 を書くものも混ぜる。印なしに属性が続くものは、付き方がそろわない
    let mut opens = false;
    match (attribs > 0, writer.random.below(12)) {
        (true, 0) if loose => {}
        (true, 1) if loose => writer.int(66, 0),
        (true, _) | (false, 0) => {
            writer.int(66, 1);
            opens = true;
        }
        (false, 1) => writer.int(66, 0),
        (false, _) => {}
    }
    let name = writer.pick(&REFERENCES);
    writer.raw(2, name);
    writer.point([10, 20, 30], origin);
    for code in [41, 42, 43] {
        if writer.chance(3) {
            let scale = writer.pick_real(&[1.0, 2.0, 0.5, -1.0, -2.0, 1.25, 0.0, 1e-3]);
            writer.real(code, scale);
        }
    }
    if writer.chance(3) {
        let free = writer.random.between(-720.0, 720.0);
        let angle = writer.pick_real(&[0.0, 90.0, 45.0, -30.0, 360.0, free]);
        writer.real(50, angle);
    }
    if grid {
        if !writer.chance(6) {
            let columns = writer.pick_int(&[1, 2, 3, 4, 0, -1]);
            writer.int(70, columns);
        }
        if !writer.chance(6) {
            let rows = writer.pick_int(&[1, 2, 3, 0]);
            writer.int(71, rows);
        }
        if !writer.chance(4) {
            let spacing = writer.pick_real(&[10.0, 2.5, 0.0, -3.0]);
            writer.real(44, spacing);
        }
        if !writer.chance(4) {
            let spacing = writer.pick_real(&[10.0, 7.5, 0.0]);
            writer.real(45, spacing);
        }
    }
    writer.extrusion();
    writer.xdata(origin);
    for _ in 0..attribs {
        *next += 1;
        attrib(writer, origin, markers, *next, handle, common, loose);
    }
    // 終わりの印。書かないものも混ぜる
    let closes = if attribs > 0 {
        !writer.chance(12)
    } else {
        opens && writer.chance(2)
    };
    if closes || (loose && writer.chance(10)) {
        *next += 1;
        writer.raw(0, "SEQEND");
        writer.raw(5, &format!("{:X}", *next));
        if !writer.chance(4) {
            writer.raw(330, &format!("{handle:X}"));
        }
        if markers {
            writer.raw(100, "AcDbEntity");
        }
        writer.raw(8, "0");
    }
}

/// 名前の表のレコードを書く。`table` は、表のハンドル
fn table_record(
    writer: &mut Writer,
    kind: &str,
    class: &str,
    handle: u64,
    table: &str,
    name: &str,
    rest: &[(i16, &str)],
) {
    writer.raw(0, kind);
    writer.raw(5, &format!("{handle:X}"));
    if !table.is_empty() {
        writer.raw(330, table);
    }
    writer.raw(100, "AcDbSymbolTableRecord");
    writer.raw(100, class);
    writer.raw(2, name);
    for (code, value) in rest {
        writer.raw(*code, value);
    }
}

/// ブロックの中のレコードの先頭。`record` は、ブロック・レコードのハンドル
fn member(writer: &mut Writer, kind: &str, next: &mut u64, record: Option<u64>) {
    *next += 1;
    writer.raw(0, kind);
    writer.raw(5, &format!("{:X}", *next));
    if let Some(record) = record {
        writer.raw(330, &format!("{record:X}"));
    }
    writer.raw(100, "AcDbEntity");
    writer.raw(8, "0");
}

/// 属性の定義
fn attdef(writer: &mut Writer, next: &mut u64, record: Option<u64>, tag: &str, flags: i64) {
    member(writer, "ATTDEF", next, record);
    writer.raw(100, "AcDbText");
    writer.real(10, 0.0);
    writer.real(20, 0.0);
    writer.real(30, 0.0);
    writer.real(40, 2.5);
    writer.raw(1, "default");
    writer.raw(100, "AcDbAttributeDefinition");
    writer.raw(3, "prompt");
    writer.raw(2, tag);
    writer.int(70, flags);
}

/// 足すブロックの定義。名前と、BLOCK_RECORD の表にレコードを足すか。同じ名前の定義が 2 つあるものも混ぜる
const BLOCKS: [(&str, bool); 9] = [
    ("CASE_PLAIN", true),
    ("CASE_BASE", true),
    ("CASE_ATTDEF", true),
    ("CASE_CONST", true),
    ("CASE_EMPTY", true),
    ("case_lower", false),
    ("CASE_NEST", true),
    ("CASE_TWICE", true),
    ("CASE_TWICE", false),
];

/// ブロックの定義と、BLOCK_RECORD の表のレコードを書く。定義のバイト列と、表のレコードのバイト列を返す
fn definitions(writer: &mut Writer, next: &mut u64, table: Option<&str>) -> (Vec<u8>, Vec<u8>) {
    let mut records = Vec::new();
    let mut blocks = Vec::new();
    let origin = [0.0, 0.0];
    for (name, recorded) in BLOCKS {
        let record = match table {
            Some(table) if recorded => {
                *next += 1;
                let rest = [(340, "0")];
                table_record(
                    writer,
                    "BLOCK_RECORD",
                    "AcDbBlockTableRecord",
                    *next,
                    table,
                    name,
                    &rest,
                );
                records.append(&mut writer.out);
                Some(*next)
            }
            _ => None,
        };
        member(writer, "BLOCK", next, record);
        writer.raw(100, "AcDbBlockBegin");
        writer.raw(2, name);
        let attdefs = matches!(name, "CASE_ATTDEF" | "CASE_CONST");
        writer.int(70, if attdefs { 2 } else { 0 });
        let base = if name == "CASE_BASE" {
            [1.5, -2.0]
        } else {
            origin
        };
        writer.real(10, base[0]);
        writer.real(20, base[1]);
        writer.real(30, 0.0);
        writer.raw(3, name);
        writer.raw(1, "");
        match name {
            "CASE_PLAIN" => {
                member(writer, "LINE", next, record);
                line(writer, origin, true);
                member(writer, "CIRCLE", next, record);
                circle(writer, origin, true, false);
            }
            "CASE_BASE" => {
                member(writer, "ARC", next, record);
                circle(writer, origin, true, true);
                member(writer, "LWPOLYLINE", next, record);
                polyline(writer, origin, true);
                member(writer, "TEXT", next, record);
                text(writer, origin, true);
            }
            "CASE_ATTDEF" => {
                member(writer, "LINE", next, record);
                line(writer, origin, true);
                attdef(writer, next, record, "TAG", 0);
                attdef(writer, next, record, "NO", 1);
            }
            "CASE_CONST" => {
                member(writer, "CIRCLE", next, record);
                circle(writer, origin, true, false);
                attdef(writer, next, record, "FIXED", 2);
            }
            "CASE_EMPTY" => {}
            "CASE_NEST" => {
                member(writer, "INSERT", next, record);
                writer.raw(100, "AcDbBlockReference");
                writer.raw(2, "CASE_PLAIN");
                writer.real(10, 3.0);
                writer.real(20, 4.0);
                writer.real(30, 0.0);
                member(writer, "LINE", next, record);
                line(writer, origin, true);
            }
            _ => {
                member(writer, "LINE", next, record);
                line(writer, origin, true);
            }
        }
        member(writer, "ENDBLK", next, record);
        writer.raw(100, "AcDbBlockEnd");
        blocks.append(&mut writer.out);
    }
    (blocks, records)
}

/// 足した図形を指す辞書と、リアクターを持つオブジェクトを書く。`made` は、足した図形のハンドル
fn objects(writer: &mut Writer, next: &mut u64, made: &[u64]) {
    if made.is_empty() {
        return;
    }
    let pick = |writer: &mut Writer| {
        let handle = made
            .get(writer.random.below(made.len()))
            .copied()
            .unwrap_or(0);
        format!("{handle:X}")
    };
    for index in 0..(made.len() / 8).clamp(1, 64) {
        *next += 1;
        if index % 2 == 0 {
            writer.raw(0, "DICTIONARY");
            writer.raw(5, &format!("{:X}", *next));
            writer.raw(100, "AcDbDictionary");
            writer.int(281, 1);
            for entry in 0..1 + writer.random.below(4) {
                writer.raw(3, &format!("CASE_{entry}"));
                let handle = pick(writer);
                let code = if writer.chance(3) { 360 } else { 350 };
                writer.raw(code, &handle);
            }
        } else {
            writer.raw(0, "XRECORD");
            writer.raw(5, &format!("{:X}", *next));
            writer.raw(102, "{ACAD_REACTORS");
            for _ in 0..1 + writer.random.below(3) {
                let handle = pick(writer);
                writer.raw(330, &handle);
            }
            writer.raw(102, "}");
            // 組の外の参照は、指す図形を除いても残る
            if writer.chance(2) {
                let handle = pick(writer);
                writer.raw(330, &handle);
            }
            writer.raw(100, "AcDbXrecord");
            writer.int(280, 1);
            writer.raw(1, "note");
            if writer.chance(2) {
                let handle = pick(writer);
                writer.raw(340, &handle);
            }
        }
    }
}

/// 元の図形から、モデル空間の持ち主のハンドルと、使われている画層を集める
fn learn(src: &[u8]) -> (Option<String>, Vec<String>) {
    let mut owner = None;
    let mut layers = vec!["0".to_owned()];
    let Ok(document) = Document::open(src.to_vec(), 1) else {
        return (owner, layers);
    };
    for row in document.rows.iter().take(64) {
        let range = document.record_range(row.record);
        let mut pos = range.start;
        while pos < range.end {
            let Next::Tag(tag) = scan::next_tag(src, pos) else {
                break;
            };
            pos = tag.end as usize;
            let value = src
                .get(tag.value as usize..tag.value_end as usize)
                .unwrap_or_default();
            let Ok(value) = core::str::from_utf8(value) else {
                continue;
            };
            if !value.is_ascii() {
                continue;
            }
            match tag.code {
                330 if owner.is_none() => owner = Some(value.trim().to_owned()),
                8 if !layers.iter().any(|layer| layer == value) && layers.len() < 8 => {
                    layers.push(value.to_owned());
                }
                _ => {}
            }
        }
    }
    (owner, layers)
}

/// 入力の作り方の変種
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Variant {
    /// 属性と終わりの印の付き方がそろった図面
    Plain,
    /// 付き方のそろわない図面（持ち主がほかを指す ATTRIB、印のない INSERT に続く ATTRIB、親のない ATTRIB と SEQEND）
    Loose,
    /// `$HANDSEED` のない図面
    NoSeed,
    /// `$HANDSEED` が、ハンドルの最大より小さい図面
    LowSeed,
    /// ENTITIES のセクションが 2 つある図面
    TwoSections,
    /// TABLES のセクションのない図面
    NoTables,
    /// ENTITIES のセクションが空の図面。図形は足さず、定義と表のレコードだけを足す
    Empty,
}

impl Variant {
    /// `--variant` の値から
    pub fn parse(name: &str) -> Option<Variant> {
        Some(match name {
            "plain" => Variant::Plain,
            "loose" => Variant::Loose,
            "no-seed" => Variant::NoSeed,
            "low-seed" => Variant::LowSeed,
            "two-sections" => Variant::TwoSections,
            "no-tables" => Variant::NoTables,
            "empty" => Variant::Empty,
            _ => return None,
        })
    }
}

/// レコードを足す表
const TABLES: [&[u8]; 4] = [b"LAYER", b"LTYPE", b"STYLE", b"BLOCK_RECORD"];

/// 足す場所と、除く範囲
#[derive(Default)]
struct Places {
    /// BLOCKS のセクションを閉じる ENDSEC の位置
    blocks_end: Option<usize>,
    /// OBJECTS のセクションを閉じる ENDSEC の位置
    objects_end: Option<usize>,
    /// 最初の ENTITIES のセクションの、中身の始まり
    entities_start: Option<usize>,
    /// TABLES のセクションの範囲（SECTION の先頭から、ENDSEC の後ろまで）
    tables: Option<Range<usize>>,
    /// 表（`TABLES` の順）を閉じる ENDTAB の位置と、表のハンドル
    table_ends: [Option<(usize, String)>; 4],
    /// `$HANDSEED` の変数の範囲（9 のタグの先頭から、次の変数かレコードの前まで）
    seed_variable: Option<Range<usize>>,
}

fn places(src: &[u8]) -> Places {
    let mut places = Places::default();
    let mut section: &[u8] = &[];
    let mut section_start = 0;
    let mut section_head = false;
    // TABLE のレコードの中にいるか。2 が表の種類、5 が表のハンドル
    let mut table_head = false;
    let mut table: Option<usize> = None;
    let mut table_handle = String::new();
    let mut seed_from: Option<usize> = None;
    let mut pos = 0;
    while let Next::Tag(tag) = scan::next_tag(src, pos) {
        pos = tag.end as usize;
        let start = tag.start as usize;
        let value = src
            .get(tag.value as usize..tag.value_end as usize)
            .unwrap_or_default()
            .trim_ascii();
        if matches!(tag.code, 0 | 9)
            && let Some(from) = seed_from.take()
        {
            places.seed_variable = Some(from..start);
        }
        match tag.code {
            0 => {
                match value {
                    b"SECTION" => section_start = start,
                    b"ENDSEC" => {
                        match section {
                            b"BLOCKS" if places.blocks_end.is_none() => {
                                places.blocks_end = Some(start);
                            }
                            b"OBJECTS" if places.objects_end.is_none() => {
                                places.objects_end = Some(start);
                            }
                            b"TABLES" if places.tables.is_none() => {
                                places.tables = Some(section_start..tag.end as usize);
                            }
                            _ => {}
                        }
                        section = &[];
                    }
                    b"ENDTAB" => {
                        if let Some(at) = table.take()
                            && let Some(place) = places.table_ends.get_mut(at)
                            && place.is_none()
                        {
                            *place = Some((start, table_handle.clone()));
                        }
                    }
                    _ => {}
                }
                section_head = value == b"SECTION";
                table_head = value == b"TABLE" && section == b"TABLES";
                if table_head {
                    table = None;
                    table_handle.clear();
                }
            }
            2 if section_head => {
                section = value;
                section_head = false;
                if value == b"ENTITIES" && places.entities_start.is_none() {
                    places.entities_start = Some(tag.end as usize);
                }
            }
            2 if table_head => table = TABLES.iter().position(|name| *name == value),
            5 if table_head => table_handle = String::from_utf8_lossy(value).into_owned(),
            9 if value == b"$HANDSEED" && places.seed_variable.is_none() => {
                seed_from = Some(start);
            }
            _ => {}
        }
    }
    places
}

/// 場所の順に、差し込みと置き換えを当てる。前の置き換えと重なるものは当てない
fn patched(src: &[u8], mut patches: Vec<(Range<usize>, Vec<u8>)>) -> Vec<u8> {
    patches.sort_by_key(|(range, _)| (range.start, range.end));
    let grown: usize = patches.iter().map(|(_, bytes)| bytes.len()).sum();
    let mut out = Vec::with_capacity(src.len() + grown);
    let mut written = 0;
    for (range, bytes) in patches {
        if range.start < written {
            continue;
        }
        out.extend_from_slice(src.get(written..range.start).unwrap_or_default());
        out.extend_from_slice(&bytes);
        written = range.end;
    }
    out.extend_from_slice(src.get(written..).unwrap_or_default());
    out
}

/// `count` 個の図形と、ブロックの定義・表のレコード・図形を指すオブジェクトを足した DXF を返す
pub fn cases(src: &[u8], count: usize, seed: u64, variant: Variant) -> Result<Vec<u8>, String> {
    let found = grow::outline(src);
    let insert_at = found.insert_at.ok_or("ENTITIES のセクションがない")?;
    let seed_at = found.seed.ok_or("$HANDSEED がない")?;
    if seed_at.end > insert_at {
        return Err("$HANDSEED が ENTITIES のセクションより後ろにある".to_owned());
    }
    let places = places(src);
    let head_bytes = src.get(..4096.min(src.len())).unwrap_or_default();
    let eol: &'static [u8] = if head_bytes.windows(2).any(|pair| pair == b"\r\n") {
        b"\r\n"
    } else {
        b"\n"
    };
    let (owner, mut layers) = learn(src);
    let loose = variant == Variant::Loose;
    let mut writer = Writer {
        out: Vec::with_capacity(count * 256),
        eol,
        padded: src.first() == Some(&b' '),
        random: Random(seed),
        spoil: false,
    };
    let mut next = found.largest;
    let mut patches: Vec<(Range<usize>, Vec<u8>)> = Vec::new();

    // 表のレコード
    let table = |at: usize| {
        places
            .table_ends
            .get(at)
            .and_then(Option::as_ref)
            .filter(|_| variant != Variant::NoTables)
    };
    if let Some((at, handle)) = table(0) {
        for (name, color, line_type) in [
            ("CASE_A", "3", "Continuous"),
            ("Case_b", "5", "CASE_DASH"),
            ("CASE_TWICE", "1", "Continuous"),
            ("case_twice", "2", "Continuous"),
        ] {
            next += 1;
            let rest = [(70, "0"), (62, color), (6, line_type)];
            table_record(
                &mut writer,
                "LAYER",
                "AcDbLayerTableRecord",
                next,
                handle,
                name,
                &rest,
            );
        }
        patches.push((*at..*at, std::mem::take(&mut writer.out)));
    }
    if let Some((at, handle)) = table(1) {
        next += 1;
        let rest = [
            (70, "0"),
            (3, "dashes"),
            (72, "65"),
            (73, "2"),
            (40, "0.75"),
            (49, "0.5"),
            (74, "0"),
            (49, "-0.25"),
            (74, "0"),
        ];
        table_record(
            &mut writer,
            "LTYPE",
            "AcDbLinetypeTableRecord",
            next,
            handle,
            "CASE_DASH",
            &rest,
        );
        patches.push((*at..*at, std::mem::take(&mut writer.out)));
    }
    if let Some((at, handle)) = table(2) {
        next += 1;
        let rest = [
            (70, "0"),
            (40, "0.0"),
            (41, "1.0"),
            (50, "0.0"),
            (71, "0"),
            (42, "2.5"),
            (3, "txt"),
            (4, ""),
        ];
        table_record(
            &mut writer,
            "STYLE",
            "AcDbTextStyleTableRecord",
            next,
            handle,
            "CASE_STYLE",
            &rest,
        );
        patches.push((*at..*at, std::mem::take(&mut writer.out)));
    }
    layers.extend(["CASE_A", "Case_b", "CASE_TWICE", "CASE_LOOSE"].map(str::to_owned));

    // ブロックの定義
    if let Some(at) = places.blocks_end {
        let record_table = table(3);
        let (blocks, records) = definitions(
            &mut writer,
            &mut next,
            record_table.map(|(_, handle)| handle.as_str()),
        );
        if let Some((record_at, _)) = record_table {
            patches.push((*record_at..*record_at, records));
        }
        patches.push((at..at, blocks));
    }

    // 図形
    let common = Common {
        owner: owner.as_deref(),
        layers: &layers,
    };
    let count = if variant == Variant::Empty { 0 } else { count };
    let mut made: Vec<u64> = Vec::with_capacity(count);
    for index in 0..count {
        if variant == Variant::TwoSections && index > 0 && index == count / 2 {
            writer.raw(0, "ENDSEC");
            writer.raw(0, "SECTION");
            writer.raw(2, "ENTITIES");
        }
        next += 1;
        let handle = next;
        let origin = [(index % 40) as f64 * 10.0, (index / 40) as f64 * 10.0];
        writer.spoil = writer.chance(50);
        let kind = index % 6;
        let name = ["LINE", "CIRCLE", "ARC", "LWPOLYLINE", "TEXT", "INSERT"]
            .get(kind)
            .copied()
            .unwrap_or("LINE");
        let markers = head(&mut writer, name, handle, &common, &made);
        match kind {
            0 => line(&mut writer, origin, markers),
            1 => circle(&mut writer, origin, markers, false),
            2 => circle(&mut writer, origin, markers, true),
            3 => polyline(&mut writer, origin, markers),
            4 => text(&mut writer, origin, markers),
            _ => insert(
                &mut writer,
                origin,
                markers,
                handle,
                &mut next,
                &common,
                loose,
            ),
        }
        if kind != 5 {
            writer.xdata(origin);
        }
        made.push(handle);
    }
    writer.spoil = false;
    if loose && count > 0 {
        // 親のない ATTRIB と SEQEND
        next += 1;
        attrib(&mut writer, [0.0, 0.0], true, next, 0, &common, true);
        next += 1;
        writer.raw(0, "SEQEND");
        writer.raw(5, &format!("{next:X}"));
        writer.raw(8, "0");
    }
    let entities = std::mem::take(&mut writer.out);

    // 足した図形を指すオブジェクト
    if let Some(at) = places.objects_end {
        objects(&mut writer, &mut next, &made);
        patches.push((at..at, std::mem::take(&mut writer.out)));
    }

    match variant {
        Variant::NoSeed => {
            if let Some(range) = places.seed_variable.clone() {
                patches.push((range, Vec::new()));
            }
        }
        Variant::LowSeed => patches.push((seed_at, b"1".to_vec())),
        _ => patches.push((seed_at, format!("{:X}", next + 1).into_bytes())),
    }
    match (variant, places.entities_start) {
        (Variant::Empty, Some(start)) if start <= insert_at => {
            patches.push((start..insert_at, Vec::new()));
        }
        _ => patches.push((insert_at..insert_at, entities)),
    }
    if variant == Variant::NoTables
        && let Some(range) = places.tables.clone()
    {
        patches.push((range, Vec::new()));
    }
    Ok(patched(src, patches))
}
