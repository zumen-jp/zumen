//! 用紙の mm で表した線種・円の半径・文字の高さを、空間を置くビューの縮尺で実寸に直す。
//!
//! 切り出しの図面は、通り芯の記号の大きさと線種の列を用紙の mm で持つ。DXF の実寸の空間には用紙の長さを
//! 書けないので、提出に使う縮尺を 1 つに決めたときだけ、文書の単位の値へ直す。

use super::Drawing;
use crate::building::Error;
use zumen_base::{Cx, Kind};
use zumen_drawing as d;

/// 空間を置くビューが 1 つで、回転・鏡映・ゆがみのない等方の縮尺なら、用紙 1 mm の文書の単位での長さ。
fn factor(drawing: &d::Drawing2D, space: d::SpaceId) -> Option<f64> {
    let mut views = drawing
        .sheets
        .iter()
        .flat_map(|sheet| &sheet.views)
        .filter(|view| view.space == space);
    let view = views.next()?;
    if views.next().is_some() {
        return None;
    }
    let [a, b, _, c, e, _] = view.transform.m;
    let factor = 1.0 / a;
    (a == e && b == 0.0 && c == 0.0 && a > 0.0 && factor.is_finite() && factor > 0.0)
        .then_some(factor)
}

fn model(length: &mut d::Length, space: d::SpaceId, factor: f64) -> bool {
    match *length {
        d::Length::Paper(d::PaperLength(value)) if (value * factor).is_finite() => {
            *length = d::Length::Model(d::ModelLength {
                space,
                value: value * factor,
            });
            true
        }
        _ => false,
    }
}

/// 円の半径と文字の高さ・枠の高さのうち、用紙の長さを実寸に直す。直したら true。
fn resize(shape: &mut d::Shape<d::ModelPoint>, space: d::SpaceId, factor: f64) -> bool {
    match shape {
        d::Shape::Circle { radius, .. } => model(radius, space, factor),
        d::Shape::Text(text) => model(&mut text.height.value, space, factor),
        d::Shape::Formatted(text) => {
            let height = model(&mut text.height.value, space, factor);
            let frame = text
                .frame_height
                .as_mut()
                .is_some_and(|frame| model(frame, space, factor));
            height || frame
        }
        _ => false,
    }
}

impl Drawing {
    /// 用紙の mm で表した値を、空間を置くビューの縮尺で文書の単位に直した図面を返す。
    ///
    /// ビューが 1 つで等方の縮尺の空間について、円の半径と文字の高さを実寸の長さにする。すべての空間が同じ
    /// 縮尺で、要素の線種の尺度がどれも 1 なら、用紙の mm の線種の列も、図面の線種の尺度で割った文書の単位に
    /// する。直せない値は変えず、書き手が出所付きで診断する。線と元の要素への対応・ID・順は変えない。
    /// 直した追加図形の記録は、新しい形のハッシュに合わせる。
    ///
    /// # Errors
    ///
    /// 予算の上限か中断の合図で止めたときは [`Error::Stopped`]。部分結果は返さない。
    pub fn at_view_scale(&self, cx: &mut Cx<'_>) -> Result<Self, Error> {
        let drawing = &self.output.drawing;
        let elements = drawing.spaces.iter().fold(0u64, |sum, space| {
            sum.saturating_add(space.elements.len() as u64)
        });
        // 複製する図面と、直す要素ごとの判定。
        cx.check(
            Kind::Work,
            elements
                .saturating_add(drawing.extensions.len() as u64)
                .saturating_add(drawing.line_types.len() as u64),
        )?;
        cx.check(Kind::DecodedBytes, elements.saturating_mul(512))?;
        let mut out = self.clone();
        let drawing = &mut out.output.drawing;
        let mut common = None;
        let mut uniform = true;
        let mut changed = std::collections::BTreeSet::new();
        for index in 0..drawing.spaces.len() {
            let Some(space) = drawing.spaces.get(index).map(|space| space.id) else {
                continue;
            };
            let factor = factor(drawing, space);
            match (factor, common) {
                (Some(f), None) => common = Some(f),
                (Some(f), Some(c)) if f == c => {}
                _ => uniform = false,
            }
            let Some(model) = drawing.spaces.get_mut(index) else {
                continue;
            };
            for (at, element) in model.elements.iter_mut().enumerate() {
                if element.style.line_type_scale != 1.0 {
                    uniform = false;
                }
                if let Some(factor) = factor
                    && resize(&mut element.shape, space, factor)
                {
                    cx.check(Kind::DecodedBytes, 32)?;
                    changed.insert((index, at));
                }
            }
        }
        let line_scale = drawing.line_type_scale.unwrap_or(1.0);
        if let Some(factor) = common.filter(|_| uniform)
            && line_scale.is_finite()
            && line_scale > 0.0
        {
            for line_type in &mut drawing.line_types {
                if let Some(definition) = line_type.definition.as_mut()
                    && definition.basis == d::PatternBasis::Paper
                    && definition.marks.is_empty()
                    && definition
                        .pattern
                        .iter()
                        .all(|v| (v * factor / line_scale).is_finite())
                {
                    for value in &mut definition.pattern {
                        *value = *value * factor / line_scale;
                    }
                    definition.basis = d::PatternBasis::Element;
                }
            }
        }
        // 追加図形の記録は、足したときの形のハッシュを持つ。直した形のハッシュに合わせる。
        let mut bases = std::collections::BTreeMap::new();
        for decoration in &out.decorations {
            cx.check(Kind::Work, 1)?;
            if !changed.contains(&(0, decoration.index)) {
                continue;
            }
            if let Some(element) = out
                .output
                .drawing
                .spaces
                .first()
                .and_then(|space| space.elements.get(decoration.index))
            {
                cx.check(Kind::DecodedBytes, decoration.drawing_id.len() as u64 + 64)?;
                bases.insert(
                    decoration.drawing_id.as_str(),
                    d::Basis::Hash(d::Hash::element_checked(element, cx)?),
                );
            }
        }
        for extension in &mut out.output.drawing.extensions {
            if extension.namespace == "zumen.profile.building-drawing"
                && extension.name == "decoration"
                && let d::Target::Element(id) = &extension.target
                && let Some(basis) = bases.get(id.as_str())
            {
                extension.basis = *basis;
            }
        }
        Ok(out)
    }
}
