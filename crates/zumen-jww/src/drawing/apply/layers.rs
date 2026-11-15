//! 16 × 16 のレイヤ枠の名前と状態を、ほかのヘッダーのバイトを保って書き戻す。

use std::collections::{BTreeMap, BTreeSet};

use zumen_base::{Diagnostic, Kind, codes};
use zumen_drawing::{Hash, Layer, LayerId, LayerState, Seen, id};
use zumen_sdk::{Cx, Error};

use super::{Base, Header, JwwFile, Reader, RecordKind, text, unsupported, work};

#[derive(Default)]
pub(super) struct Changes {
    slots: BTreeMap<usize, (Layer, u32)>,
    used: Option<BTreeSet<LayerId>>,
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
        header: &Header,
        layers: &[Layer],
        target: &Seen,
        layer: &Layer,
        cx: &mut Cx,
    ) -> Result<Option<Layer>, Error> {
        work(cx, layers.len() as u64)?;
        let Some((at, was)) = layers
            .iter()
            .enumerate()
            .find(|(_, l)| id::layer(l) == target.id)
        else {
            cx.report(
                Diagnostic::new(codes::CHANGE_ELEMENT_MISSING).with("id", target.id.as_str()),
            );
            return Ok(None);
        };
        if hash(was, cx)? != target.hash {
            cx.report(Diagnostic::new(codes::CHANGE_ELEMENT_STALE).with("id", target.id.as_str()));
            return Ok(None);
        }
        if self.slots.contains_key(&at) {
            cx.report(unsupported("replace_layer", "shared").at(was.source.clone()));
            return Ok(None);
        }
        self.put(header, was, layer, "replace_layer", cx)
    }

    pub(super) fn add(
        &mut self,
        file: &JwwFile,
        header: &Header,
        layers: &[Layer],
        layer: &Layer,
        cx: &mut Cx,
    ) -> Result<Option<Layer>, Error> {
        // 領域モデルに変換できない図形も使う枠を確保する。追加したレイヤの選択で上書きしない。
        if self.used.is_none() {
            let mut used = BTreeSet::new();
            for record in file.records() {
                work(cx, 1)?;
                if record.kind() == RecordKind::Entity {
                    let Some(base) = Base::read(&file.src, record.body().start, header.version)
                    else {
                        cx.report(unsupported("add_layer", "layer"));
                        return Ok(None);
                    };
                    used.insert(base.layer());
                }
            }
            self.used = Some(used);
        }
        work(cx, layers.len() as u64)?;
        let group = usize::try_from(header.write_group).ok().filter(|g| *g < 16);
        let slot = group.and_then(|group| {
            let g = header.groups.get(group)?;
            (0..16).find_map(|slot| {
                let at = group * 16 + slot;
                let was = layers.get(at)?;
                (header.layer_names.get(at)?.is_empty()
                    && *g.layers.get(slot)? == 0
                    && *g.protects.get(slot)? == 0
                    && g.write_layer != slot as u32
                    && !self.slots.contains_key(&at)
                    && !self.used.as_ref()?.contains(&was.id))
                .then_some(was)
            })
        });
        let Some(was) = slot else {
            cx.report(unsupported("add_layer", "layer"));
            return Ok(None);
        };
        self.put(header, was, layer, "add_layer", cx)
    }

    fn put(
        &mut self,
        header: &Header,
        was: &Layer,
        layer: &Layer,
        edit: &str,
        cx: &mut Cx,
    ) -> Result<Option<Layer>, Error> {
        let size = layer.name.len() as u64;
        work(cx, size.div_ceil(1024))?;
        cx.check(
            Kind::DecodedBytes,
            size.saturating_mul(4).saturating_add(16),
        )
        .map_err(|stop| {
            cx.report(stop.to_diagnostic());
            Error::Stopped(stop)
        })?;
        let Some((expected, state)) = normalize(header, was, layer) else {
            cx.report(unsupported(edit, "layer").at(was.source.clone()));
            return Ok(None);
        };
        let at = (was.id.0 - 1) as usize;
        self.slots.insert(at, (expected.clone(), state));
        Ok(Some(expected))
    }

    /// 外側の `None` は不正なヘッダー、内側は変更なし。
    pub(super) fn bytes(
        &self,
        file: &JwwFile,
        cx: &mut Cx,
    ) -> Result<Option<Option<Vec<u8>>>, Error> {
        if self.slots.is_empty() {
            return Ok(Some(None));
        }
        let Some(end) = file.records().first().map(|r| r.span().end) else {
            return Ok(None);
        };
        work(cx, (end as u64).div_ceil(1024))?;
        let result = (|| {
            let original = file.src.get(..end)?;
            let mut r = Reader::new(original, 12);
            r.string().ok()?;
            r.skip(8).ok()?;
            let mut out = original.to_vec();
            // Jw_cad データ形式のヘッダー: レイヤグループ情報、線幅、レイヤ名。
            // https://www.jwcad.net/jwdatafmt.txt
            for group in 0..16 {
                r.skip(20).ok()?;
                for slot in 0..16 {
                    if let Some((_, state)) = self.slots.get(&(group * 16 + slot)) {
                        out.get_mut(r.pos..r.pos + 4)?
                            .copy_from_slice(&state.to_le_bytes());
                    }
                    r.skip(8).ok()?;
                }
            }
            r.skip(80 + 4 + 72).ok()?;
            out.truncate(r.pos);
            for at in 0..256 {
                let start = r.pos;
                let (unicode, _) = r.string().ok()?;
                match self.slots.get(&at) {
                    Some((layer, _)) => text::write_string(&mut out, &layer.name, unicode)?,
                    None => out.extend_from_slice(original.get(start..r.pos)?),
                }
            }
            out.extend_from_slice(original.get(r.pos..)?);
            Some(Some(out))
        })();
        Ok(result)
    }
}

fn normalize(header: &Header, was: &Layer, new: &Layer) -> Option<(Layer, u32)> {
    if new.name.is_empty()
        || new.name.chars().any(char::is_control)
        || new.description.as_ref().is_some_and(|d| !d.is_empty())
        || new.style.is_some_and(|s| {
            s.color.is_some()
                || s.line_type.is_some()
                || s.line_weight.is_some()
                || s.transparency.is_some()
        })
    {
        return None;
    }
    let state = new.state?;
    let at = usize::try_from(was.id.0.checked_sub(1)?).ok()?;
    let group = header.groups.get(at / 16)?;
    let original = *group.layers.get(at % 16)?;
    let raw = if Some(state) == was.state {
        original
    } else if state.editable {
        2
    } else if state.visible {
        1
    } else {
        0
    };
    let visible = group.state != 0 && raw != 0;
    let actual = LayerState {
        visible,
        printable: visible,
        editable: group.state >= 2 && raw >= 2,
        frozen: false,
    };
    if actual != state {
        return None;
    }
    let mut result = was.clone();
    result.name.clone_from(&new.name);
    result.state = Some(actual);
    Some((result, raw))
}
