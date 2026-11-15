//! 保存時のレコード順に、常駐モデルの出所を対応させる。

use std::collections::BTreeMap;

use zumen_base::{Cx, Key, Kind, Ref};
use zumen_drawing::{CalloutContent, Drawing2D, Element, Placed, Shape};
use zumen_sdk::Error;

use crate::drawing::work;

pub(super) struct SourceMap {
    removed: Vec<u64>,
    insertion: Option<u64>,
    born: BTreeMap<u64, u64>,
}

impl SourceMap {
    pub(super) fn removed(&self, source: &Ref) -> bool {
        source
            .record()
            .is_some_and(|record| self.removed.binary_search(&record).is_ok())
    }

    pub(super) fn build(
        removed: impl Iterator<Item = u64>,
        insertion: Option<u64>,
        born: impl Iterator<Item = u64>,
        cx: &mut Cx<'_>,
    ) -> Result<Self, Error> {
        let mut out = Self {
            removed: Vec::new(),
            insertion,
            born: BTreeMap::new(),
        };
        for record in removed {
            work(cx, 1, &Ref::default())?;
            allocated(cx, 32)?;
            out.removed.push(record);
        }
        let at = insertion.unwrap_or(0);
        let mut next = at.saturating_sub(out.removed.partition_point(|r| *r < at) as u64);
        for instance in born {
            work(cx, 1, &Ref::default())?;
            allocated(cx, 96)?;
            out.born.insert(instance, next);
            next = next.saturating_add(1);
        }
        Ok(out)
    }

    pub(super) fn remap(&self, source: &mut Ref, cx: &mut Cx<'_>) -> Result<(), Error> {
        work(cx, 1, source)?;
        let born = match source.key() {
            Some(Key::Instance(instance)) => self.born.get(&instance).copied(),
            _ => None,
        };
        let effective = born.or_else(|| {
            let record = source.record()?;
            let removed = self.removed.partition_point(|r| *r < record) as u64;
            let inserted = if self.insertion.is_some_and(|at| at <= record) {
                self.born.len() as u64
            } else {
                0
            };
            Some(record.saturating_sub(removed).saturating_add(inserted))
        });
        if let Some(record) = effective {
            *source = core::mem::take(source).with_record(record);
        }
        Ok(())
    }

    pub(super) fn placed(&self, placed: &mut Placed, cx: &mut Cx<'_>) -> Result<(), Error> {
        match placed {
            Placed::Model(element) => self.element(element, cx),
            Placed::Paper(element) => self.element(element, cx),
        }
    }

    fn element<P>(&self, element: &mut Element<P>, cx: &mut Cx<'_>) -> Result<(), Error> {
        self.remap(&mut element.source, cx)?;
        match &mut element.shape {
            Shape::Symbol(symbol) => {
                for attribute in &mut symbol.attributes {
                    self.remap(&mut attribute.source, cx)?;
                }
            }
            Shape::Leader(leader) => {
                if let Some(annotation) = &mut leader.annotation {
                    self.remap(annotation, cx)?;
                }
            }
            Shape::Hatch(hatch) => {
                for boundary in &mut hatch.loops {
                    work(cx, 1, &Ref::default())?;
                    for source in &mut boundary.sources {
                        self.remap(source, cx)?;
                    }
                }
            }
            Shape::Callout(callout) => {
                if let Some(style) = &mut callout.style {
                    self.remap(style, cx)?;
                }
                if let CalloutContent::Symbol { values, .. } = &mut callout.content {
                    for value in values {
                        self.remap(&mut value.definition, cx)?;
                    }
                }
            }
            Shape::Table(table) => {
                if let Some(style) = &mut table.style {
                    self.remap(style, cx)?;
                }
            }
            Shape::Underlay(underlay) => self.remap(&mut underlay.resource.definition, cx)?,
            _ => {}
        }
        Ok(())
    }

    pub(super) fn drawing(&self, drawing: &mut Drawing2D, cx: &mut Cx<'_>) -> Result<(), Error> {
        for space in &mut drawing.spaces {
            work(cx, 1, &Ref::default())?;
            for element in &mut space.elements {
                self.element(element, cx)?;
            }
        }
        for symbol in &mut drawing.symbols {
            self.remap(&mut symbol.source, cx)?;
            for element in &mut symbol.space.elements {
                self.element(element, cx)?;
            }
            for attribute in &mut symbol.attributes {
                self.remap(&mut attribute.attribute.source, cx)?;
            }
        }
        for sheet in &mut drawing.sheets {
            self.remap(&mut sheet.source, cx)?;
            for element in &mut sheet.elements {
                self.element(element, cx)?;
            }
            for view in &mut sheet.views {
                self.remap(&mut view.source, cx)?;
            }
        }
        for layer in &mut drawing.layers {
            self.remap(&mut layer.source, cx)?;
        }
        for line_type in &mut drawing.line_types {
            self.remap(&mut line_type.source, cx)?;
        }
        for style in &mut drawing.dimension_styles {
            self.remap(&mut style.source, cx)?;
        }
        for style in &mut drawing.text_styles {
            self.remap(&mut style.source, cx)?;
        }
        for title in &mut drawing.title_blocks {
            self.remap(&mut title.source, cx)?;
        }
        Ok(())
    }
}

fn allocated(cx: &mut Cx<'_>, bytes: u64) -> Result<(), Error> {
    cx.check(Kind::DecodedBytes, bytes).map_err(|stop| {
        cx.report(stop.to_diagnostic());
        Error::Stopped(stop)
    })
}
