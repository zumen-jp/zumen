//! 書き方の違う図形を足した入力を作る。今の実装と照らす範囲を、合成の図面や公開サンプルにない書き方へ広げる。
//!
//! 線・円・円弧・折れ線・文字を、任意のタグの有無、数の書式、拡張データ、押し出しの向き、読めない値などを
//! 乱数で変えながら、ENTITIES のセクションの終わりに足す。新しいハンドルを振り、`$HANDSEED` を直す。
//! 照らすための入力であり、ほかのアプリケーションに渡す図面ではない。

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
    /// 元の図形が使う画層
    layers: &'a [String],
}

/// 図形の先頭の、種類に依らないタグを書く。サブクラスマーカーを書くかどうかを返す
fn head(writer: &mut Writer, kind: &str, handle: u64, common: &Common) -> bool {
    writer.raw(0, kind);
    let handle = if writer.chance(10) {
        format!("{handle:x}")
    } else {
        format!("{handle:X}")
    };
    writer.raw(5, &handle);
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
        let name = writer.pick(&["ByLayer", "BYLAYER", "ByBlock", "Continuous", "CONTINUOUS"]);
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

/// `count` 個の図形を足した DXF を返す
pub fn cases(src: &[u8], count: usize, seed: u64) -> Result<Vec<u8>, String> {
    let found = grow::outline(src);
    let insert_at = found.insert_at.ok_or("ENTITIES のセクションがない")?;
    let seed_at = found.seed.ok_or("$HANDSEED がない")?;
    if seed_at.end > insert_at {
        return Err("$HANDSEED が ENTITIES のセクションより後ろにある".to_owned());
    }
    let head_bytes = src.get(..4096.min(src.len())).unwrap_or_default();
    let eol: &'static [u8] = if head_bytes.windows(2).any(|pair| pair == b"\r\n") {
        b"\r\n"
    } else {
        b"\n"
    };
    let (owner, layers) = learn(src);
    let common = Common {
        owner: owner.as_deref(),
        layers: &layers,
    };
    let mut writer = Writer {
        out: Vec::with_capacity(count * 256),
        eol,
        padded: src.first() == Some(&b' '),
        random: Random(seed),
        spoil: false,
    };
    let mut next = found.largest;
    for index in 0..count {
        next += 1;
        let origin = [(index % 40) as f64 * 10.0, (index / 40) as f64 * 10.0];
        writer.spoil = writer.chance(50);
        let kind = index % 5;
        let name = ["LINE", "CIRCLE", "ARC", "LWPOLYLINE", "TEXT"]
            .get(kind)
            .copied()
            .unwrap_or("LINE");
        let markers = head(&mut writer, name, next, &common);
        match kind {
            0 => line(&mut writer, origin, markers),
            1 => circle(&mut writer, origin, markers, false),
            2 => circle(&mut writer, origin, markers, true),
            3 => polyline(&mut writer, origin, markers),
            _ => text(&mut writer, origin, markers),
        }
        writer.xdata(origin);
    }
    let mut out = Vec::with_capacity(src.len() + writer.out.len() + 16);
    out.extend_from_slice(src.get(..seed_at.start).unwrap_or_default());
    out.extend_from_slice(format!("{:X}", next + 1).as_bytes());
    out.extend_from_slice(src.get(seed_at.end..insert_at).unwrap_or_default());
    out.extend_from_slice(&writer.out);
    out.extend_from_slice(src.get(insert_at..).unwrap_or_default());
    Ok(out)
}
