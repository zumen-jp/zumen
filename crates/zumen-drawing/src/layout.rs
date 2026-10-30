//! 1 行の文字（[`Text`]）を、呼び出し側が渡したフォントの字形の寸法で置く試作（決定 0176）。
//!
//! フォントは同梱せず、[`Glyphs`] で寸法だけを受け取る（決定 0022）。結果は、字形の単位の座標を
//! 図面の座標へ写すアフィン変換と、文字ごとの送りの位置で返す。字形の輪郭は扱わない。
//!
//! 位置合わせの規則は、DXF の TEXT（72・73）の説明と旧版の知見から推定したもので、AutoCAD・Jw_cad の
//! 表示とはまだ比べていない。
//!
//! - 左・中央・右は、送りの合計の 0・半分・全部の位置。外形の中央（[`Horizontal::Middle`]）は、
//!   字形の外形の中央で、縦も外形の中央にする。
//! - 基準線・下・中央・上は、基準線・ディセンダーの線・大文字の高さの半分・大文字の高さの線。
//! - 2 点の間は、送りの合計が 2 点の距離になるよう幅を変え、Aligned は高さも同じ比で変える。
//! - 斜体は字形にだけ当て、位置合わせは斜体を当てる前の寸法で決める。左右・上下の反転は、
//!   置く点（2 点の間では 2 点の中点）のまわりで当てる。

use crate::{Fit, HeightMeaning, Horizontal, Length, Placement, Text, Vertical};

/// フォント全体の寸法。値は字形の単位で、上を正とする
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FontMetrics {
    /// em の大きさ
    pub units_per_em: f64,
    /// アセンダー（基準線から上の高さ）
    pub ascent: f64,
    /// ディセンダー（基準線から下の深さ。正の値）
    pub descent: f64,
    /// 大文字の高さ
    pub cap_height: f64,
}

/// 1 文字の字形の寸法。値は字形の単位
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Glyph {
    /// 送りの幅
    pub advance: f64,
    /// 外形（左・下・右・上）。空白などの外形のない字形は `None`
    pub bounds: Option<[f64; 4]>,
}

/// 呼び出し側が渡すフォント
pub trait Glyphs {
    /// フォント全体の寸法
    fn metrics(&self) -> FontMetrics;
    /// 文字の字形。フォントになければ `None`
    fn glyph(&self, ch: char) -> Option<Glyph>;
    /// フォントにない文字に使う字形（TrueType の `.notdef` など）
    fn fallback(&self) -> Glyph;
}

/// 宣言された高さが、フォントのどの寸法に当たるか
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reference {
    /// 大文字の高さ
    CapHeight,
    /// em
    Em,
    /// アセンダーとディセンダーを合わせた高さ
    Frame,
}

/// 置いた結果
#[derive(Clone, Debug, PartialEq)]
pub struct Laid {
    /// 字形の単位の座標（基準線の始まりが原点）を、図面の座標へ写す変換 `[a, b, c, d, e, f]`。
    /// `X = a·u + c·v + e`、`Y = b·u + d·v + f`
    pub transform: [f64; 6],
    /// 文字ごとの送りの始まり（字形の単位）。内容の文字の順
    pub pens: Vec<f64>,
    /// 送りの合計（字形の単位）
    pub advance: f64,
    /// 字形の単位から図面の長さへの倍率（横・縦）。反転は含まない
    pub scale: [f64; 2],
    /// フォントになく、代わりの字形で置いた文字の数
    pub missing: usize,
}

/// 置けない理由
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unplaced {
    /// 宣言された高さが、フォントのどの寸法に当たるか決まらない。形式と欄を指すキー
    Height(&'static str),
    /// 高さ・幅の倍率・フォントの寸法・2 点の距離・送りの合計が 0 か有限でないか、斜体の角度が 90 度以上
    Degenerate,
}

/// 文字を置く。`xy` は点を図面の座標にし、`declared` は形式の宣言値の高さの意味を決める
pub fn place<P>(
    text: &Text<P>,
    xy: impl Fn(&P) -> [f64; 2],
    font: &impl Glyphs,
    declared: impl Fn(&'static str) -> Option<Reference>,
) -> Result<Laid, Unplaced> {
    let metrics = font.metrics();
    let reference = match text.height.meaning {
        HeightMeaning::CapHeight => Reference::CapHeight,
        HeightMeaning::Em => Reference::Em,
        HeightMeaning::Frame => Reference::Frame,
        HeightMeaning::Declared(key) => declared(key).ok_or(Unplaced::Height(key))?,
    };
    let unit = match reference {
        Reference::CapHeight => metrics.cap_height,
        Reference::Em => metrics.units_per_em,
        Reference::Frame => metrics.ascent + metrics.descent,
    };
    let height = match text.height.value {
        Length::Paper(length) => length.0,
        Length::Model(length) => length.value,
    };
    let scale = height / unit;
    if !positive(scale) || !positive(text.width_factor) || !finite(&metrics) {
        return Err(Unplaced::Degenerate);
    }

    let (pens, advance, ink, missing) = run(&text.content, font);
    let mut sx = scale * text.width_factor;
    let mut sy = scale;

    let (point, anchor, (sin, cos)) = match &text.placement {
        Placement::Anchor {
            point,
            horizontal,
            vertical,
        } => {
            let width = advance * sx;
            let ink_center = ink.map(|[l, b, r, t]| [(l + r) / 2.0 * sx, (b + t) / 2.0 * sy]);
            let x = match *horizontal {
                Horizontal::Left => 0.0,
                Horizontal::Center => width / 2.0,
                Horizontal::Right => width,
                Horizontal::Middle => ink_center.map_or(width / 2.0, |[x, _]| x),
            };
            let y = match (*horizontal, *vertical) {
                (Horizontal::Middle, _) => {
                    ink_center.map_or(metrics.cap_height / 2.0 * sy, |[_, y]| y)
                }
                (_, Vertical::Baseline) => 0.0,
                (_, Vertical::Bottom) => -metrics.descent * sy,
                (_, Vertical::Middle) => metrics.cap_height / 2.0 * sy,
                (_, Vertical::Top) => metrics.cap_height * sy,
            };
            let radians = text.rotation.to_radians();
            (xy(point), [x, y], (libm::sin(radians), libm::cos(radians)))
        }
        Placement::Between { first, second, fit } => {
            let [x0, y0] = xy(first);
            let [x1, y1] = xy(second);
            let distance = libm::hypot(x1 - x0, y1 - y0);
            let width = advance * sx;
            if !positive(distance) || !positive(width) {
                return Err(Unplaced::Degenerate);
            }
            let ratio = distance / width;
            sx *= ratio;
            if *fit == Fit::Aligned {
                sy *= ratio;
            }
            let direction = ((y1 - y0) / distance, (x1 - x0) / distance);
            // 反転を 2 点の中点のまわりで当て、2 点の間に収める
            let middle = [(x0 + x1) / 2.0, (y0 + y1) / 2.0];
            (middle, [distance / 2.0, 0.0], direction)
        }
    };

    // 斜体は 90 度より小さい角度に限る
    if !text.oblique.is_finite() || text.oblique.abs() >= 90.0 {
        return Err(Unplaced::Degenerate);
    }
    let shear = libm::tan(text.oblique.to_radians());
    let mx = if text.backward { -1.0 } else { 1.0 };
    let my = if text.upside_down { -1.0 } else { 1.0 };
    let [ax, ay] = anchor;
    let transform = [
        cos * mx * sx,
        sin * mx * sx,
        cos * mx * sy * shear - sin * my * sy,
        sin * mx * sy * shear + cos * my * sy,
        -cos * mx * ax + sin * my * ay + point[0],
        -sin * mx * ax - cos * my * ay + point[1],
    ];
    Ok(Laid {
        transform,
        pens,
        advance,
        scale: [sx, sy],
        missing,
    })
}

/// 文字を左から並べ、送りの始まり・送りの合計・外形・フォントにない文字の数を返す
fn run(content: &str, font: &impl Glyphs) -> (Vec<f64>, f64, Option<[f64; 4]>, usize) {
    let mut pens = Vec::new();
    let mut pen = 0.0;
    let mut ink: Option<[f64; 4]> = None;
    let mut missing = 0;
    for ch in content.chars() {
        let glyph = font.glyph(ch).unwrap_or_else(|| {
            missing += 1;
            font.fallback()
        });
        pens.push(pen);
        if let Some([l, b, r, t]) = glyph.bounds {
            let [l, r] = [pen + l, pen + r];
            ink = Some(match ink {
                Some([l0, b0, r0, t0]) => [l0.min(l), b0.min(b), r0.max(r), t0.max(t)],
                None => [l, b, r, t],
            });
        }
        pen += glyph.advance;
    }
    (pens, pen, ink, missing)
}

fn positive(value: f64) -> bool {
    value.is_finite() && value > 0.0
}

fn finite(metrics: &FontMetrics) -> bool {
    [
        metrics.units_per_em,
        metrics.ascent,
        metrics.descent,
        metrics.cap_height,
    ]
    .iter()
    .all(|value| value.is_finite())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Height, PaperLength};

    /// 合成のフォント。em は 1000、大文字の高さ 700、アセンダー 800、ディセンダー 200。
    /// `A` は送り 600・外形 0〜600 × 0〜700、`g` は送り 500・外形 50〜450 × -200〜500、空白は送り 250
    struct Synthetic;

    impl Glyphs for Synthetic {
        fn metrics(&self) -> FontMetrics {
            FontMetrics {
                units_per_em: 1000.0,
                ascent: 800.0,
                descent: 200.0,
                cap_height: 700.0,
            }
        }

        fn glyph(&self, ch: char) -> Option<Glyph> {
            match ch {
                'A' => Some(Glyph {
                    advance: 600.0,
                    bounds: Some([0.0, 0.0, 600.0, 700.0]),
                }),
                'g' => Some(Glyph {
                    advance: 500.0,
                    bounds: Some([50.0, -200.0, 450.0, 500.0]),
                }),
                ' ' => Some(Glyph {
                    advance: 250.0,
                    bounds: None,
                }),
                _ => None,
            }
        }

        fn fallback(&self) -> Glyph {
            Glyph {
                advance: 1000.0,
                bounds: Some([0.0, 0.0, 1000.0, 700.0]),
            }
        }
    }

    fn text(content: &str, placement: Placement<[f64; 2]>) -> Text<[f64; 2]> {
        Text {
            content: content.to_owned(),
            height: Height {
                value: Length::Paper(PaperLength(7.0)),
                meaning: HeightMeaning::CapHeight,
            },
            placement,
            rotation: 0.0,
            width_factor: 1.0,
            oblique: 0.0,
            backward: false,
            upside_down: false,
            style: String::new(),
        }
    }

    fn anchor(point: [f64; 2], horizontal: Horizontal, vertical: Vertical) -> Placement<[f64; 2]> {
        Placement::Anchor {
            point,
            horizontal,
            vertical,
        }
    }

    fn laid(text: &Text<[f64; 2]>) -> Laid {
        place(text, |p| *p, &Synthetic, |_| None).unwrap()
    }

    /// 字形の単位の点を、結果の変換で図面の座標へ写す
    fn map(laid: &Laid, [u, v]: [f64; 2]) -> [f64; 2] {
        let [a, b, c, d, e, f] = laid.transform;
        [a * u + c * v + e, b * u + d * v + f]
    }

    fn assert_near([x, y]: [f64; 2], [ex, ey]: [f64; 2]) {
        assert!(
            (x - ex).abs() < 1e-9 && (y - ey).abs() < 1e-9,
            "({x}, {y}) != ({ex}, {ey})"
        );
    }

    #[test]
    fn nine_points_and_baseline() {
        // 送りの合計 1100、倍率 0.01 で幅 11、大文字の高さ 7、ディセンダー 2
        let cases = [
            (Horizontal::Left, Vertical::Baseline, [0.0, 0.0]),
            (Horizontal::Center, Vertical::Baseline, [5.5, 0.0]),
            (Horizontal::Right, Vertical::Baseline, [11.0, 0.0]),
            (Horizontal::Left, Vertical::Bottom, [0.0, -2.0]),
            (Horizontal::Center, Vertical::Middle, [5.5, 3.5]),
            (Horizontal::Right, Vertical::Top, [11.0, 7.0]),
            // 外形は 0〜1050 × -200〜700 なので、中央は 5.25・2.5
            (Horizontal::Middle, Vertical::Baseline, [5.25, 2.5]),
        ];
        for (horizontal, vertical, offset) in cases {
            let laid = laid(&text("Ag", anchor([100.0, 50.0], horizontal, vertical)));
            // 置く点に当たる字形の座標が、置く点に写る
            let at = [offset[0] / 0.01, offset[1] / 0.01];
            assert_near(map(&laid, at), [100.0, 50.0]);
            assert_near(
                map(&laid, [0.0, 0.0]),
                [100.0 - offset[0], 50.0 - offset[1]],
            );
        }
    }

    #[test]
    fn pens_and_missing_glyphs() {
        let laid = laid(&text(
            "A g字",
            anchor([0.0, 0.0], Horizontal::Left, Vertical::Baseline),
        ));
        assert_eq!(laid.pens, vec![0.0, 600.0, 850.0, 1350.0]);
        assert_eq!(laid.advance, 2350.0);
        assert_eq!(laid.missing, 1);
    }

    #[test]
    fn rotation_width_factor_and_oblique() {
        let mut text = text(
            "A",
            anchor([10.0, 0.0], Horizontal::Left, Vertical::Baseline),
        );
        text.rotation = 90.0;
        text.width_factor = 2.0;
        text.oblique = 45.0;
        let laid = laid(&text);
        assert_eq!(laid.scale, [0.02, 0.01]);
        // 送りの終わり（600, 0）は 12 だけ上へ、大文字の上（0, 700）は斜体で 7 だけ前へ寄り、7 だけ左へ
        assert_near(map(&laid, [600.0, 0.0]), [10.0, 12.0]);
        assert_near(map(&laid, [0.0, 700.0]), [3.0, 7.0]);
    }

    #[test]
    fn between_two_points() {
        let placement = |fit| Placement::Between {
            first: [0.0, 0.0],
            second: [0.0, 22.0],
            fit,
        };
        // 送りの合計 1100 を、距離 22 に合わせる
        let aligned = laid(&text("Ag", placement(Fit::Aligned)));
        assert_near([aligned.scale[0], aligned.scale[1]], [0.02, 0.02]);
        assert_near(map(&aligned, [1100.0, 0.0]), [0.0, 22.0]);
        assert_near(map(&aligned, [0.0, 700.0]), [-14.0, 0.0]);

        let fit = laid(&text("Ag", placement(Fit::Fit)));
        assert_near([fit.scale[0], fit.scale[1]], [0.02, 0.01]);
        assert_near(map(&fit, [0.0, 0.0]), [0.0, 0.0]);
        assert_near(map(&fit, [0.0, 700.0]), [-7.0, 0.0]);
    }

    #[test]
    fn mirrors_about_the_point() {
        let mut text = text(
            "A",
            anchor([0.0, 0.0], Horizontal::Left, Vertical::Baseline),
        );
        text.backward = true;
        text.upside_down = true;
        let laid = laid(&text);
        assert_near(map(&laid, [600.0, 700.0]), [-6.0, -7.0]);

        let mut between = self::text(
            "A",
            Placement::Between {
                first: [0.0, 0.0],
                second: [6.0, 0.0],
                fit: Fit::Fit,
            },
        );
        between.backward = true;
        let laid = place(&between, |p| *p, &Synthetic, |_| None).unwrap();
        // 反転しても 2 点の間に収まり、始まりが 2 点目に来る
        assert_near(map(&laid, [0.0, 0.0]), [6.0, 0.0]);
        assert_near(map(&laid, [600.0, 0.0]), [0.0, 0.0]);
    }

    #[test]
    fn height_meanings() {
        let mut text = text(
            "A",
            anchor([0.0, 0.0], Horizontal::Left, Vertical::Baseline),
        );
        text.height.meaning = HeightMeaning::Em;
        assert_eq!(laid(&text).scale, [0.007, 0.007]);
        text.height.meaning = HeightMeaning::Frame;
        assert_eq!(laid(&text).scale, [0.007, 0.007]);

        text.height.meaning = HeightMeaning::Declared("dxf.TEXT.40");
        assert_eq!(
            place(&text, |p| *p, &Synthetic, |_| None),
            Err(Unplaced::Height("dxf.TEXT.40"))
        );
        let laid = place(
            &text,
            |p| *p,
            &Synthetic,
            |key| (key == "dxf.TEXT.40").then_some(Reference::CapHeight),
        )
        .unwrap();
        assert_eq!(laid.scale, [0.01, 0.01]);
    }

    #[test]
    fn degenerate_inputs() {
        let mut text = text(
            "",
            Placement::Between {
                first: [0.0, 0.0],
                second: [1.0, 0.0],
                fit: Fit::Fit,
            },
        );
        let attempt = |text: &Text<[f64; 2]>| place(text, |p| *p, &Synthetic, |_| None);
        assert_eq!(attempt(&text), Err(Unplaced::Degenerate));
        text.content = "A".to_owned();
        assert!(attempt(&text).is_ok());
        text.width_factor = 0.0;
        assert_eq!(attempt(&text), Err(Unplaced::Degenerate));
        text.width_factor = 1.0;
        text.oblique = -90.0;
        assert_eq!(attempt(&text), Err(Unplaced::Degenerate));
        text.oblique = 0.0;
        text.height.value = Length::Paper(PaperLength(f64::NAN));
        assert_eq!(attempt(&text), Err(Unplaced::Degenerate));
    }
}
