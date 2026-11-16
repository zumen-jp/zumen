//! 要素の値を複製する前に条件を照合する。公開 ID の命名と並びは変えない。

use std::collections::{BTreeMap, BTreeSet};
use zumen_base::{Cx, Key, Kind, Stop};
use zumen_drawing::{Drawing2D, Element, LayerId, id};

use crate::messages::{EntitiesRequest, EntityKind, EntitySourceFilter};
use crate::session::{Failed, fail, invalid_value, stopped};
use crate::tables::table_id;

#[derive(Default)]
pub(in crate::session) struct Filters {
    ids: Option<Strings>,
    layers: Option<Vec<LayerId>>,
    kinds: Option<Vec<EntityKind>>,
    source: Source,
}

#[derive(Default)]
struct Source {
    record: Option<u64>,
    bytes: Option<std::ops::Range<u64>>,
    key: Option<Key>,
}

fn reserve<T>(count: usize, slot: u64, path: &str, cx: &mut Cx<'_>) -> Result<Vec<T>, Failed> {
    let bytes = (count as u64).saturating_mul(slot);
    cx.check(Kind::DecodedBytes, bytes)
        .map_err(|stop| stopped(cx, stop))?;
    if bytes > i32::MAX as u64 {
        return Err(fail(cx, invalid_value(path)));
    }
    let mut result = Vec::new();
    result
        .try_reserve_exact(count)
        .map_err(|_| fail(cx, invalid_value(path)))?;
    Ok(result)
}

struct Strings {
    values: Vec<String>,
    max_bytes: u64,
}
impl Strings {
    fn new(values: &[String], path: &str, cx: &mut Cx<'_>) -> Result<Self, Failed> {
        let mut copied = reserve(values.len(), 64, path, cx)?;
        let mut max_bytes = 1;
        for value in values {
            let bytes = value.len() as u64;
            cx.check(Kind::Work, bytes.saturating_add(1))
                .and_then(|()| cx.check(Kind::DecodedBytes, bytes))
                .map_err(|stop| stopped(cx, stop))?;
            if bytes > i32::MAX as u64 {
                return Err(fail(cx, invalid_value(path)));
            }
            let mut text = String::new();
            text.try_reserve_exact(value.len())
                .map_err(|_| fail(cx, invalid_value(path)))?;
            text.push_str(value);
            copied.push(text);
            max_bytes = max_bytes.max(bytes.saturating_add(1));
        }
        let work = (values.len() as u64)
            .saturating_mul(u64::from(values.len().max(1).ilog2()) + 1)
            .saturating_mul(max_bytes)
            .saturating_mul(64);
        cx.check(Kind::Work, work)
            .map_err(|stop| stopped(cx, stop))?;
        copied.sort_unstable();
        copied.dedup();
        Ok(Self {
            values: copied,
            max_bytes,
        })
    }

    fn contains(&self, value: &str, cx: &mut Cx<'_>) -> Result<bool, Stop> {
        let work = self
            .max_bytes
            .saturating_mul(u64::from(self.values.len().max(1).ilog2()) + 1);
        cx.check(Kind::Work, work)?;
        Ok(self
            .values
            .binary_search_by(|key| key.as_str().cmp(value))
            .is_ok())
    }
}

impl Source {
    fn new(value: Option<&EntitySourceFilter>, cx: &mut Cx<'_>) -> Result<Self, Failed> {
        let Some(value) = value else {
            return Ok(Self::default());
        };
        if value.handle.is_some() && value.instance.is_some() {
            return Err(fail(cx, invalid_value("/source")));
        }
        let bytes = value.bytes.as_ref().map(|range| range.start..range.end);
        if bytes.as_ref().is_some_and(|range| range.start >= range.end) {
            return Err(fail(cx, invalid_value("/source/bytes")));
        }
        let mut key = None;
        for (text, radix, length, path) in [
            (value.handle.as_deref(), 16, 16, "/source/handle"),
            (value.instance.as_deref(), 10, 20, "/source/instance"),
        ] {
            if let Some(text) = text {
                cx.check(Kind::Work, text.len() as u64)
                    .map_err(|stop| stopped(cx, stop))?;
                let digits = text.bytes().all(|b| {
                    if radix == 16 {
                        b.is_ascii_hexdigit()
                    } else {
                        b.is_ascii_digit()
                    }
                });
                if text.is_empty() || text.len() > length || !digits {
                    return Err(fail(cx, invalid_value(path)));
                }
                let number =
                    u64::from_str_radix(text, radix).map_err(|_| fail(cx, invalid_value(path)))?;
                key = Some(if radix == 16 {
                    Key::Handle(number)
                } else {
                    Key::Instance(number)
                });
            }
        }
        Ok(Self {
            record: value.record,
            bytes,
            key,
        })
    }

    fn empty(&self) -> bool {
        self.record.is_none() && self.bytes.is_none() && self.key.is_none()
    }
}

impl Filters {
    pub(super) fn new(
        request: &EntitiesRequest,
        drawing: &Drawing2D,
        cx: &mut Cx<'_>,
    ) -> Result<Option<Self>, Failed> {
        let source = Source::new(request.source.as_ref(), cx)?;
        if request.ids.is_none()
            && request.layers.is_none()
            && request.kinds.is_none()
            && source.empty()
        {
            return Ok(None);
        }
        let ids = request
            .ids
            .as_deref()
            .map(|ids| Strings::new(ids, "/ids", cx))
            .transpose()?;
        let layers = if let Some(wanted) = &request.layers {
            let wanted = Strings::new(wanted, "/layers", cx)?;
            let mut output = Vec::new();
            if !wanted.values.is_empty() {
                let count = drawing.layers.len();
                cx.check(
                    Kind::Work,
                    (count as u64)
                        .saturating_mul(u64::from(count.max(1).ilog2()) + 1)
                        .saturating_mul(4096),
                )
                .map_err(|stop| stopped(cx, stop))?;
                // 公開 ID と重複の番号、一時の木を固定の量で数える。最初の内部 ID が drawing.layer と対応する。
                output = reserve(count, 512, "/layers", cx)?;
                let mut names = BTreeMap::new();
                let mut seen = BTreeSet::new();
                for layer in &drawing.layers {
                    cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
                    let public = table_id(id::layer(layer), &mut names);
                    if seen.insert(layer.id)
                        && wanted
                            .contains(&public, cx)
                            .map_err(|stop| stopped(cx, stop))?
                    {
                        output.push(layer.id);
                    }
                }
                output.sort_unstable();
            }
            Some(output)
        } else {
            None
        };
        let kinds = if let Some(values) = &request.kinds {
            let mut kinds = reserve(values.len(), 8, "/kinds", cx)?;
            cx.check(
                Kind::Work,
                (values.len() as u64)
                    .saturating_mul(u64::from(values.len().max(1).ilog2()) + 1)
                    .saturating_mul(64),
            )
            .map_err(|stop| stopped(cx, stop))?;
            kinds.extend_from_slice(values);
            kinds.sort_unstable();
            kinds.dedup();
            Some(kinds)
        } else {
            None
        };
        Ok(Some(Self {
            ids,
            layers,
            kinds,
            source,
        }))
    }

    pub(super) fn matches<P>(
        &self,
        id: &str,
        element: &Element<P>,
        cx: &mut Cx<'_>,
    ) -> Result<bool, Stop> {
        cx.check(Kind::Work, 8)?;
        if let Some(ids) = &self.ids
            && !ids.contains(id, cx)?
        {
            return Ok(false);
        }
        if let Some(layers) = &self.layers {
            cx.check(Kind::Work, u64::from(layers.len().max(1).ilog2()) + 1)?;
            if layers.binary_search(&element.layer).is_err() {
                return Ok(false);
            }
        }
        if let Some(kinds) = &self.kinds {
            cx.check(Kind::Work, u64::from(kinds.len().max(1).ilog2()) + 1)?;
            if kinds
                .binary_search(&super::kind_of(&element.shape))
                .is_err()
            {
                return Ok(false);
            }
        }
        let source = &element.source;
        Ok(self
            .source
            .record
            .is_none_or(|number| source.record() == Some(number))
            && self.source.key.is_none_or(|key| source.key() == Some(key))
            && self.source.bytes.as_ref().is_none_or(|wanted| {
                source.bytes().is_some_and(|actual| {
                    actual.start < actual.end
                        && actual.start < wanted.end
                        && wanted.start < actual.end
                })
            }))
    }
}
