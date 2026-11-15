//! 表現と共有写像を通る使用位置から、近い項目の面の表示を選ぶ。

use super::{
    Acquired, Geometry, GeometryError, GeometryLinks, GeometryRef, GeometryStyle,
    GeometrySurfaceStyle, SurfaceStyleSelection,
};
use zumen_base::{Cx, Kind};

/// 表現と写像を通る経路で選んだ面の表示。色・反射式・面への適用は未評価
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SurfaceAppearance<'a> {
    /// 経路上に面の表示の指定がない
    Unspecified,
    /// 指定を取得できた。上書きされた元値は GeometryData に残る
    Value {
        /// 渡した経路内で、指定を持つ項目の位置
        path_index: usize,
        /// 元の表示と出所
        style: &'a GeometryStyle,
        /// 選んだ面の表示。正負の面の指定を合成しない
        surfaces: &'a [GeometrySurfaceStyle],
    },
    /// もっとも近い指定が未対応。外側の指定へ黙って置き換えない
    Unsupported {
        /// 渡した経路内の位置
        path_index: usize,
        /// 未対応を保つ元の指定
        style: &'a GeometryStyle,
    },
    /// もっとも近い指定に競合するレイヤがある。並びで勝者を決めない
    Ambiguous {
        /// 渡した経路内の位置
        path_index: usize,
        /// 競合するレイヤを保つ元の指定
        style: &'a GeometryStyle,
    },
}

impl Geometry {
    /// 使用位置の経路で、項目にもっとも近い面の表示を引く。
    ///
    /// 経路は根の Definition から、Representation と Mapped / Map を通って対象の項目で終わる。
    /// 同じ共有項目でも、使用する写像と表現を省略しない。集合演算や曲線の内部へは降りない。
    /// 直接の表示とレイヤの優先関係は各項目の取得結果に従う。写像では対象に近い指定を先に使い、
    /// なければ外側へたどる。これは元値を変更せず、描画や幾何の評価とも分けた選択の規則である。
    /// 深さ・探索・中断は Cx を共有し、停止と不正では結果を返さない。確保は行わない。
    pub fn surface_appearance<'a>(
        &'a self,
        path: &[GeometryRef],
        cx: &mut Cx,
    ) -> Result<SurfaceAppearance<'a>, GeometryError> {
        cx.check(Kind::Work, 1)?;
        if path.first().copied() != self.data.root {
            return Err(GeometryError::Reference);
        }
        if path.is_empty() {
            return Ok(SurfaceAppearance::Unspecified);
        }
        let mut entered = 0usize;
        let result = (|| {
            for (at, &reference) in path.iter().enumerate() {
                cx.check(Kind::Work, 1)?;
                cx.enter()?;
                entered += 1;
                let node = self
                    .data
                    .nodes
                    .get(reference.0 as usize)
                    .ok_or(GeometryError::Reference)?;
                if at == 0 && !matches!(node.links, GeometryLinks::Definition(_)) {
                    return Err(GeometryError::Reference);
                }
                if let Some(next) = path.get(at + 1) {
                    let child = self
                        .data
                        .nodes
                        .get(next.0 as usize)
                        .ok_or(GeometryError::Reference)?;
                    let valid = match &node.links {
                        GeometryLinks::Definition(items) => {
                            cx.check(Kind::Work, items.len() as u64)?;
                            items.contains(next)
                                && matches!(child.links, GeometryLinks::Representation { .. })
                        }
                        GeometryLinks::Representation { items, .. } => {
                            cx.check(Kind::Work, items.len() as u64)?;
                            items.contains(next)
                                && !matches!(
                                    child.links,
                                    GeometryLinks::Definition(_)
                                        | GeometryLinks::Representation { .. }
                                        | GeometryLinks::Map(_)
                                )
                        }
                        GeometryLinks::Mapped(target) => {
                            target == next && matches!(child.links, GeometryLinks::Map(_))
                        }
                        GeometryLinks::Map(target) => {
                            target == next
                                && matches!(child.links, GeometryLinks::Representation { .. })
                        }
                        _ => false,
                    };
                    if !valid {
                        return Err(GeometryError::Reference);
                    }
                } else if matches!(
                    node.links,
                    GeometryLinks::Definition(_)
                        | GeometryLinks::Representation { .. }
                        | GeometryLinks::Map(_)
                ) {
                    return Err(GeometryError::Reference);
                }
            }
            for (index, pair) in path.windows(2).enumerate().rev() {
                cx.check(Kind::Work, 1)?;
                let [parent, item] = pair else {
                    return Err(GeometryError::Reference);
                };
                let parent_node = self
                    .data
                    .nodes
                    .get(parent.0 as usize)
                    .ok_or(GeometryError::Reference)?;
                if !matches!(parent_node.links, GeometryLinks::Representation { .. }) {
                    continue;
                }
                let mut found = None;
                for style in &self.data.styles {
                    cx.check(Kind::Work, 1)?;
                    if style.item != *item || style.representation != Some(*parent) {
                        continue;
                    }
                    if found.replace(style).is_some() {
                        return Err(GeometryError::Reference);
                    }
                }
                if let Some(style) = found {
                    let path_index = index + 1;
                    let Acquired::Value(presentation) = &style.presentation else {
                        return Ok(SurfaceAppearance::Unsupported { path_index, style });
                    };
                    let surfaces = match presentation.surface_source {
                        SurfaceStyleSelection::None => continue,
                        SurfaceStyleSelection::Direct => &presentation.direct,
                        SurfaceStyleSelection::Layer(layer) => {
                            &presentation
                                .layers
                                .get(layer as usize)
                                .ok_or(GeometryError::Reference)?
                                .styles
                        }
                        SurfaceStyleSelection::MultipleLayers => {
                            return Ok(SurfaceAppearance::Ambiguous { path_index, style });
                        }
                    };
                    return Ok(SurfaceAppearance::Value {
                        path_index,
                        style,
                        surfaces,
                    });
                }
            }
            Ok(SurfaceAppearance::Unspecified)
        })();
        for _ in 0..entered {
            cx.leave();
        }
        result
    }
}
