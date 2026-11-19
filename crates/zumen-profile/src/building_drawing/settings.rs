//! 読み込み時の表の検査と、切り出した線への見た目の割り当て。
use super::*;
use crate::building::vocabulary_error;
use crate::profile::{GetKey, text};
use zumen_json::Value;

/// 切り出しに使う既定値。形や入力に依存しない、プロファイルの選択。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Defaults {
    /// 階の基点から切る高さ、m。
    pub height: f64,
    /// 面から向こうを見る深さ、m。
    pub depth: f64,
    /// 縮尺の分母。
    pub denominator: f64,
    /// 用紙の大きさ、mm。
    pub size: [f64; 2],
}
impl Defaults {
    /// 原点を紙上の原点へ置く既定の用紙。
    pub fn paper(self) -> b::Paper {
        b::Paper {
            denominator: self.denominator,
            origin: [d::PaperLength(0.0); 2],
            size: Some(self.size.map(d::PaperLength)),
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct Settings {
    pub defaults: Defaults,
    pub styles: Vec<Style>,
    pub lines: Vec<(String, Vec<f64>)>,
    pub hatches: Vec<Hatch>,
    pub grid: [f64; 3],
    pub grid_font: Option<String>,
    pub levels: Vec<Level>,
    pub doors: Vec<Door>,
    pub style_work: u64,
}
#[derive(Clone, Debug)]
pub(super) struct Style {
    pub role: String,
    pub classes: Vec<String>,
    pub width: f64,
    pub line: usize,
    pub color: [u8; 3],
    pub layer: String,
}
#[derive(Clone, Debug)]
pub(super) struct Hatch {
    pub name: String,
    pub materials: Vec<String>,
    pub angles: Vec<f64>,
    pub pitch: f64,
}
#[derive(Clone, Debug)]
pub(super) struct Level {
    pub kind: String,
    pub names: Vec<String>,
    pub numbers: Vec<(String, String)>,
}
#[derive(Clone, Debug)]
pub(super) struct Door {
    pub operation: String,
    pub types: Vec<String>,
    pub lines: Vec<[[f64; 2]; 2]>,
    pub arcs: Vec<Arc>,
}
#[derive(Clone, Debug)]
pub(super) struct Arc {
    pub center: [f64; 2],
    pub radius: f64,
    pub start: f64,
    pub end: f64,
}

fn array(v: Option<&Value>) -> Option<&[Value]> {
    if let Some(Value::Array(v)) = v {
        Some(v)
    } else {
        None
    }
}
fn string(v: &Value, key: &str) -> Option<String> {
    Some(
        v.get_key(key)
            .and_then(text)
            .filter(|s| !s.is_empty())?
            .into(),
    )
}
fn strings(v: Option<&Value>) -> Option<Vec<String>> {
    array(v)?
        .iter()
        .map(|v| text(v).filter(|v| !v.is_empty()).map(str::to_owned))
        .collect()
}
fn numbers(v: Option<&Value>) -> Option<Vec<f64>> {
    array(v)?.iter().map(crate::number).collect()
}
fn number(v: &Value, key: &str) -> Option<f64> {
    v.get_key(key)
        .and_then(crate::number)
        .filter(|n| n.is_finite())
}
fn positive(v: &Value, key: &str) -> Option<f64> {
    number(v, key).filter(|n| *n > 0.0)
}
fn xy(v: Option<&Value>) -> Option<[f64; 2]> {
    let [x, y] = array(v)? else { return None };
    Some([crate::number(x)?, crate::number(y)?])
}

impl Settings {
    pub fn parse(profile: &BuildingProfile) -> Result<Self, Error> {
        // load が JSON の大きさから配列・文字列・全組み合わせの走査を先に確保している。
        let values = |key| {
            array(
                profile
                    .resolved
                    .vocabularies
                    .get(key)
                    .and_then(|v| v.get_key("values")),
            )
            .ok_or_else(|| vocabulary_error(key, "values"))
        };
        let key = "drawing-line-types";
        let lines: Vec<(String, Vec<f64>)> = values(key)?
            .iter()
            .map(|v| {
                let name = string(v, "name")?;
                let pattern = numbers(v.get_key("pattern"))?;
                if !pattern.iter().all(|n| n.is_finite())
                    || !pattern.iter().map(|n| n.abs()).sum::<f64>().is_finite()
                    || (!pattern.is_empty()
                        && (!pattern.iter().any(|v| *v < 0.0)
                            || !pattern.iter().any(|v| *v >= 0.0)))
                {
                    return None;
                }
                Some((name, pattern))
            })
            .collect::<Option<_>>()
            .ok_or_else(|| vocabulary_error(key, "values"))?;
        if lines
            .iter()
            .enumerate()
            .any(|(i, v)| lines.iter().take(i).any(|a| a.0 == v.0))
        {
            return Err(vocabulary_error(key, "name"));
        }
        let key = "drawing-styles";
        let styles: Vec<Style> = values(key)?
            .iter()
            .map(|v| {
                let role = string(v, "role")?;
                if !["cut", "visible", "hidden", "grid", "annotation", "hatch"]
                    .contains(&role.as_str())
                {
                    return None;
                }
                let rgb = numbers(v.get_key("color"))?;
                let [r, g, b] = rgb.as_slice() else {
                    return None;
                };
                if !rgb
                    .iter()
                    .all(|v| (0.0..=255.0).contains(v) && libm::trunc(*v) == *v)
                {
                    return None;
                }
                let line = lines.iter().position(|(name, _)| {
                    Some(name.as_str()) == v.get_key("line").and_then(text)
                })?;
                Some(Style {
                    role,
                    classes: strings(v.get_key("classes"))?,
                    width: positive(v, "width")?,
                    line,
                    color: [*r as u8, *g as u8, *b as u8],
                    layer: string(v, "layer")?,
                })
            })
            .collect::<Option<_>>()
            .ok_or_else(|| vocabulary_error(key, "values"))?;
        for role in ["cut", "visible", "hidden", "grid", "annotation", "hatch"] {
            if styles
                .iter()
                .filter(|v| v.role == role && v.classes.is_empty())
                .count()
                != 1
            {
                return Err(vocabulary_error(key, "role"));
            }
        }
        for (i, style) in styles.iter().enumerate() {
            if styles.iter().take(i).any(|prior| {
                prior.role == style.role && prior.classes.iter().any(|c| style.classes.contains(c))
            }) {
                return Err(vocabulary_error(key, "classes"));
            }
        }
        let key = "drawing-hatches";
        let hatches: Vec<Hatch> = values(key)?
            .iter()
            .map(|v| {
                let angles = numbers(v.get_key("angles"))?;
                if angles.is_empty()
                    || angles
                        .iter()
                        .any(|v| !v.is_finite() || !(0.0..360.0).contains(v))
                {
                    return None;
                }
                Some(Hatch {
                    name: string(v, "name")?,
                    materials: strings(v.get_key("materials"))?,
                    angles,
                    pitch: positive(v, "pitch")?,
                })
            })
            .collect::<Option<_>>()
            .ok_or_else(|| vocabulary_error(key, "values"))?;
        for (i, hatch) in hatches.iter().enumerate() {
            if hatches.iter().take(i).any(|prior| {
                prior.name == hatch.name
                    || prior.materials.iter().any(|m| hatch.materials.contains(m))
            }) {
                return Err(vocabulary_error(key, "materials"));
            }
            // 資料 3.3(3) の間隔。下位のプロファイルが太さを変えた場合も検査する。
            let width = styles
                .iter()
                .filter(|s| s.role == "hatch")
                .map(|s| s.width)
                .fold(0.0, f64::max);
            if hatch.pitch < width * if hatch.angles.len() > 1 { 4.0 } else { 3.0 } {
                return Err(vocabulary_error(key, "pitch"));
            }
        }
        let key = "drawing-cut-defaults";
        let [v] = values(key)? else {
            return Err(vocabulary_error(key, "values"));
        };
        let defaults = (|| {
            let size = xy(v.get_key("paper"))?;
            if size.iter().any(|v| !v.is_finite() || *v <= 0.0) {
                return None;
            }
            Some(Defaults {
                height: number(v, "height")?,
                depth: positive(v, "depth")?,
                denominator: positive(v, "denominator")?,
                size,
            })
        })()
        .ok_or_else(|| vocabulary_error(key, "values"))?;
        let key = "drawing-grid-symbol";
        let [v] = values(key)? else {
            return Err(vocabulary_error(key, "values"));
        };
        let grid = (|| {
            Some([
                positive(v, "radius")?,
                positive(v, "text_height")?,
                positive(v, "offset")?,
            ])
        })()
        .ok_or_else(|| vocabulary_error(key, "values"))?;
        let grid_font = match v.get_key("font") {
            None => None,
            Some(font) => Some(
                text(font)
                    .filter(|s| !s.is_empty() && !s.chars().any(char::is_control))
                    .ok_or_else(|| vocabulary_error(key, "values"))?
                    .to_owned(),
            ),
        };
        let key = "drawing-level-kinds";
        let levels = values(key)?
            .iter()
            .map(|v| {
                Some(Level {
                    kind: string(v, "kind")?,
                    names: strings(v.get_key("names"))?,
                    numbers: array(v.get_key("number"))?
                        .iter()
                        .map(|v| {
                            Some((
                                v.get_key("prefix").and_then(text)?.into(),
                                v.get_key("suffix").and_then(text)?.into(),
                            ))
                        })
                        .collect::<Option<_>>()?,
                })
            })
            .collect::<Option<_>>()
            .ok_or_else(|| vocabulary_error(key, "values"))?;
        let key = "drawing-door-symbols";
        let doors = values(key)?
            .iter()
            .map(|v| {
                let operation = string(v, "operation")?;
                if operation == "NOTDEFINED" {
                    return None;
                }
                let lines = array(v.get_key("lines"))?
                    .iter()
                    .map(|v| {
                        let [a, b] = array(Some(v))? else { return None };
                        Some([xy(Some(a))?, xy(Some(b))?])
                    })
                    .collect::<Option<Vec<_>>>()?;
                let arcs = array(v.get_key("arcs"))?
                    .iter()
                    .map(|v| {
                        let start = number(v, "start")?;
                        let end = number(v, "end")?;
                        if !(0.0..360.0).contains(&start) || end <= start || end - start > 360.0 {
                            return None;
                        }
                        Some(Arc {
                            center: xy(v.get_key("center"))?,
                            radius: positive(v, "radius")?,
                            start,
                            end,
                        })
                    })
                    .collect::<Option<Vec<_>>>()?;
                if lines.is_empty() && arcs.is_empty()
                    || lines
                        .iter()
                        .flatten()
                        .flatten()
                        .chain(arcs.iter().flat_map(|a| &a.center))
                        .any(|v| !v.is_finite())
                {
                    return None;
                }
                Some(Door {
                    operation,
                    types: strings(v.get_key("types"))?,
                    lines,
                    arcs,
                })
            })
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| vocabulary_error(key, "values"))?;
        for (i, door) in doors.iter().enumerate() {
            if doors.iter().take(i).any(|d| {
                d.operation == door.operation && d.types.iter().any(|t| door.types.contains(t))
            }) {
                return Err(vocabulary_error(key, "operation"));
            }
        }
        let style_work = styles
            .iter()
            .map(|s| {
                32 + s.role.len() as u64
                    + s.classes
                        .iter()
                        .map(|c| 8 + 2 * c.len() as u64)
                        .sum::<u64>()
            })
            .sum();
        Ok(Self {
            defaults,
            styles,
            lines,
            hatches,
            grid,
            grid_font,
            levels,
            doors,
            style_work,
        })
    }

    pub fn select(
        &self,
        building: &Building,
        at: ElementRef,
        role: &str,
    ) -> Option<(usize, &Style)> {
        let class = building.components.class.get(at);
        let mut specific = None;
        let mut fallback = None;
        for (index, style) in self
            .styles
            .iter()
            .enumerate()
            .filter(|(_, s)| s.role == role)
        {
            if style.classes.is_empty() {
                fallback = Some((index, style));
            } else if class.is_some_and(|c| {
                style.classes.contains(&c.name) || style.classes.contains(&c.original)
            }) && specific.replace((index, style)).is_some()
            {
                return None;
            }
        }
        specific.or(fallback)
    }
}

pub(super) fn element_style(style: &Style, line_base: usize) -> d::Style {
    d::Style {
        color: d::Inherit::Own(d::Color {
            code: None,
            rgb: Some(style.color),
        }),
        line_type: d::Inherit::Own(d::LineTypeId((line_base + style.line) as u32)),
        line_type_scale: 1.0,
        line_weight: Some(d::Inherit::Own(d::LineWeight::Width(d::PaperLength(
            style.width,
        )))),
        transparency: d::Inherit::Layer,
    }
}

pub(super) fn style(
    profile: &DrawingProfile,
    building: &Building,
    output: &mut b::Output,
    cx: &mut Cx<'_>,
) -> Result<(), Error> {
    let settings = &profile.settings;
    let drawing = &mut output.drawing;
    let line_base = drawing.line_types.len();
    let layer_base = drawing.layers.len();
    if line_base + settings.lines.len() > u32::MAX as usize
        || layer_base + settings.styles.len() > u32::MAX as usize
    {
        return Err(Error::Input("drawing_size"));
    }
    reserve(&mut drawing.line_types, settings.lines.len(), 256, cx)?;
    reserve(&mut drawing.layers, settings.styles.len(), 256, cx)?;
    if let Some(font) = &settings.grid_font {
        if drawing
            .text_styles
            .iter()
            .any(|s| s.name.eq_ignore_ascii_case("Standard"))
        {
            return Err(Error::Input("drawing_modified"));
        }
        reserve(&mut drawing.text_styles, 1, 256, cx)?;
        cx.check(Kind::DecodedBytes, font.len() as u64 + 64)?;
        drawing.text_styles.push(d::TextStyle {
            name: "Standard".into(),
            definition: Some(d::TextStyleDefinition {
                font: Some(d::Font {
                    name: font.clone(),
                    kind: d::FontKind::Named {
                        bold: false,
                        italic: false,
                        charset: None,
                        pitch: None,
                    },
                }),
                big_font: None,
                height: None,
                width: 1.0,
                oblique: 0.0,
                backward: false,
                upside_down: false,
                vertical: false,
            }),
            source: Ref::default(),
        });
    }
    for (i, (name, pattern)) in settings.lines.iter().enumerate() {
        cx.check(Kind::DecodedBytes, (name.len() + pattern.len() * 8) as u64)?;
        drawing.line_types.push(d::LineType {
            id: d::LineTypeId((line_base + i) as u32),
            name: name.clone(),
            definition: Some(d::LineTypeDefinition {
                description: String::new(),
                pattern: pattern.clone(),
                basis: d::PatternBasis::Paper,
                marks: vec![],
            }),
            source: Ref::default(),
        });
    }
    for (i, style) in settings.styles.iter().enumerate() {
        cx.check(Kind::DecodedBytes, style.layer.len() as u64)?;
        drawing.layers.push(d::Layer {
            id: d::LayerId((layer_base + i) as u32),
            name: style.layer.clone(),
            state: None,
            style: Some(d::LayerStyle {
                color: Some(d::Color {
                    code: None,
                    rgb: Some(style.color),
                }),
                line_type: Some(d::LineTypeId((line_base + style.line) as u32)),
                line_weight: Some(d::LineWeight::Width(d::PaperLength(style.width))),
                transparency: None,
            }),
            follows_placement: false,
            description: None,
            source: Ref::default(),
        });
    }
    let model = drawing
        .spaces
        .first_mut()
        .ok_or(Error::Input("drawing_space"))?;
    for (line, extension) in output.lines.iter().zip(&mut drawing.extensions) {
        cx.check(Kind::Work, settings.style_work)?;
        let source = output
            .sources
            .get(line.source)
            .ok_or(Error::Input("drawing_source"))?;
        let (index, style) = settings
            .select(building, source.element, line.role.name())
            .ok_or(Error::Input("drawing_style_ambiguous"))?;
        let element = model
            .elements
            .get_mut(line.index)
            .ok_or(Error::Input("drawing_index"))?;
        element.layer = d::LayerId((layer_base + index) as u32);
        element.style = element_style(style, line_base);
        extension.basis = d::Basis::Hash(d::Hash::element_checked(element, cx)?);
    }
    Ok(())
}
