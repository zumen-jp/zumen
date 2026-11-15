//! SXF のレイヤフィーチャの追加・名前・表示状態を書き戻す。

use super::{Writer, text, unsupported, work};
use std::collections::BTreeSet;
use zumen_base::{Diagnostic, Key, Kind, Ref, codes};
use zumen_drawing::{Hash, Layer, LayerId, Seen, id};
use zumen_sdk::{Cx, Error};

#[derive(Default)]
pub(super) struct Changes {
    changed: BTreeSet<LayerId>,
    added: u32,
}

pub(super) fn hash(layer: &Layer, cx: &mut Cx) -> Result<Hash, Error> {
    Hash::layer_checked(layer, cx).map_err(|stop| {
        cx.report(stop.to_diagnostic().at(layer.source.clone()));
        Error::Stopped(stop)
    })
}

impl Changes {
    pub(super) fn replace(
        &mut self,
        layers: &[Layer],
        target: &Seen,
        layer: &Layer,
        writer: &mut dyn Writer,
        format: &str,
        cx: &mut Cx,
    ) -> Result<Option<Layer>, Error> {
        work(cx, layers.len() as u64, &Ref::default())?;
        let Some(was) = layers.iter().find(|l| id::layer(l) == target.id) else {
            cx.report(
                Diagnostic::new(codes::CHANGE_ELEMENT_MISSING).with("id", target.id.as_str()),
            );
            return Ok(None);
        };
        if hash(was, cx)? != target.hash {
            cx.report(Diagnostic::new(codes::CHANGE_ELEMENT_STALE).with("id", target.id.as_str()));
            return Ok(None);
        }
        if !self.changed.insert(was.id) {
            cx.report(unsupported(format, "replace_layer", "shared").at(was.source.clone()));
            return Ok(None);
        }
        count(layer, cx)?;
        let result = normalize(layer).and_then(|mut layer| {
            was.state?;
            let record = usize::try_from(was.source.record()?).ok()?;
            layer.id = was.id;
            layer.source = was.source.clone();
            Some((layer, record))
        });
        if let Some((layer, record)) = result
            && writer.layer_change(Some(record), &layer, cx)?.is_some()
        {
            return Ok(Some(layer));
        }
        cx.report(unsupported(format, "replace_layer", "layer_feature").at(was.source.clone()));
        Ok(None)
    }

    pub(super) fn add(
        &mut self,
        layers: &[Layer],
        layer: &Layer,
        writer: &mut dyn Writer,
        format: &str,
        cx: &mut Cx,
    ) -> Result<Option<Layer>, Error> {
        work(cx, layers.len() as u64, &Ref::default())?;
        count(layer, cx)?;
        let next = u32::try_from(layers.iter().filter(|l| l.state.is_some()).count())
            .ok()
            .and_then(|n| n.checked_add(self.added)?.checked_add(1));
        let candidate = normalize(layer)
            .zip(next)
            .filter(|(_, next)| !layers.iter().any(|l| l.id.0 == *next));
        if let Some((mut layer, next)) = candidate
            && let Some(instance) = writer.layer_change(None, &layer, cx)?
        {
            layer.id = LayerId(next);
            layer.source = Ref::default().with_key(Key::Instance(instance));
            self.changed.insert(layer.id);
            self.added += 1;
            return Ok(Some(layer));
        }
        cx.report(unsupported(format, "add_layer", "layer_feature"));
        Ok(None)
    }

    pub(super) fn verify(
        &self,
        before: &[Layer],
        after: &[Layer],
        cx: &mut Cx,
    ) -> Result<bool, Error> {
        if self.changed.is_empty() {
            return Ok(true);
        }
        if before.len().checked_add(self.added as usize) != Some(after.len()) {
            return Ok(false);
        }
        for was in before {
            work(cx, after.len() as u64, &was.source)?;
            if self.changed.contains(&was.id) {
                continue;
            }
            let Some(now) = after.iter().find(|l| l.id == was.id) else {
                return Ok(false);
            };
            if hash(was, cx)? != hash(now, cx)? {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

fn count(layer: &Layer, cx: &mut Cx) -> Result<(), Error> {
    let size = layer.name.len() as u64;
    work(cx, 1 + size, &layer.source)?;
    cx.check(
        Kind::DecodedBytes,
        size.saturating_mul(8).saturating_add(256),
    )
    .map_err(|stop| {
        cx.report(stop.to_diagnostic().at(layer.source.clone()));
        Error::Stopped(stop)
    })
}

fn normalize(layer: &Layer) -> Option<Layer> {
    let state = layer.state?;
    if layer.name.is_empty()
        || !text::valid_string(&layer.name)
        || layer.name.starts_with("$$SXF_dummy")
        || state.visible != state.printable
        || !state.editable
        || state.frozen
        || layer.description.as_ref().is_some_and(|s| !s.is_empty())
        || layer.style.is_some_and(|s| {
            s.color.is_some()
                || s.line_type.is_some()
                || s.line_weight.is_some()
                || s.transparency.is_some()
        })
    {
        return None;
    }
    Some(Layer {
        id: layer.id,
        name: layer.name.clone(),
        state: Some(state),
        style: None,
        follows_placement: false,
        description: None,
        source: Ref::default(),
    })
}
