//! view2d と同じ公開レイヤ ID と UNIT_LAYERS で、描画単位を除く。

use zumen_base::{Cx, Kind};
use zumen_scene::layout as l;

use crate::{Error, scene::Scene};

pub(crate) struct Visibility {
    hidden: Vec<bool>,
}

impl Visibility {
    pub fn new(scene: &Scene<'_>, ids: &[String], cx: &mut Cx<'_>) -> Result<Self, Error> {
        if ids.is_empty() {
            return Ok(Self { hidden: Vec::new() });
        }
        let count = scene.rows(l::LAYER_IDS);
        let mut text_bytes = 0_u64;
        for row in 0..count {
            cx.check(Kind::Work, 1)?;
            text_bytes = text_bytes.saturating_add(u64::from(scene.uint(l::LAYER_IDS, row, 1)?));
        }
        let work = (ids.len() as u64)
            .checked_mul(text_bytes.saturating_add(count as u64))
            .ok_or(Error::Options)?;
        cx.check(Kind::Work, work)?;
        cx.check(Kind::DecodedBytes, count as u64)?;
        if count > i32::MAX as usize {
            return Err(Error::Allocation);
        }
        let mut hidden = Vec::new();
        hidden
            .try_reserve_exact(count)
            .map_err(|_| Error::Allocation)?;
        hidden.resize(count, false);
        for id in ids {
            cx.check(Kind::Work, 0)?;
            let mut found = false;
            for (row, hide) in hidden.iter_mut().enumerate() {
                cx.check(Kind::Work, 0)?;
                if scene.text(l::LAYER_IDS, row, 0)? == id {
                    *hide = true;
                    found = true;
                }
            }
            if !found {
                return Err(Error::Options);
            }
        }
        Ok(Self { hidden })
    }

    pub fn visible(&self, scene: &Scene<'_>, unit: usize) -> Result<bool, Error> {
        if self.hidden.is_empty() {
            return Ok(true);
        }
        let layer = scene.uint(l::UNIT_LAYERS, unit, 0)?;
        if layer == l::NONE {
            return Ok(true);
        }
        self.hidden
            .get(layer as usize)
            .map(|hide| !hide)
            .ok_or(Error::Scene)
    }
}
