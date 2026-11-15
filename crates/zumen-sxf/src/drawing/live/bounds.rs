use super::*;
use zumen_live::CommittedTxn;

#[derive(Clone, Copy, Debug)]
pub(super) struct SymbolBoundsDiff {
    pub(super) index: u32,
    before: Option<[f64; 4]>,
    after: Option<[f64; 4]>,
}

pub(super) struct PreparedSymbols {
    pub(super) next: Option<BTreeMap<SymbolKey, [f64; 4]>>,
    pub(super) diffs: Vec<SymbolBoundsDiff>,
    pub(super) affected: Vec<RowId>,
}

fn check(cx: &mut Cx<'_>, kind: Kind, amount: u64) -> Result<(), Error> {
    cx.check(kind, amount).map_err(|stop| {
        cx.report(stop.to_diagnostic());
        Error::Stopped(stop)
    })
}

fn key_bytes(key: &SymbolKey) -> u64 {
    (key.namespace.len() as u64).saturating_add(key.name.len() as u64)
}

impl SxfLive {
    pub(super) fn prepare_symbols(
        &self,
        verified: &[VerifiedOp],
        cx: &mut Cx<'_>,
    ) -> Result<PreparedSymbols, Error> {
        check(cx, Kind::Work, (verified.len() as u64).max(1))?;
        check(
            cx,
            Kind::DecodedBytes,
            (verified.len() as u64)
                .saturating_mul(256)
                .saturating_add(1024),
        )?;
        let mut staged = BTreeMap::new();
        let mut roots = Vec::with_capacity(verified.len());
        for op in verified {
            let (row, placed) = match op {
                VerifiedOp::Remove(row) => (*row, None),
                VerifiedOp::Update { row_id, placed, .. } => (*row_id, Some(placed)),
                VerifiedOp::AddSfc { .. } | VerifiedOp::AddP21 { .. } => continue,
            };
            staged.insert(row, placed);
            if let Some(SxfContainer::Symbol(index)) = self.table.row(row).map(|r| r.container)
                && let Some(node) = u32::try_from(index).ok().and_then(RowId::virtual_node)
            {
                roots.push(node);
            }
        }
        if roots.is_empty() {
            return Ok(PreparedSymbols {
                next: None,
                diffs: Vec::new(),
                affected: Vec::new(),
            });
        }
        let affected = self.dep.affected_rows_with_cx(&roots, cx).map_err(|stop| {
            cx.report(stop.to_diagnostic());
            Error::Stopped(stop)
        })?;
        let mut dirty = BTreeSet::new();
        for node in roots.iter().chain(&affected) {
            check(cx, Kind::Work, 1)?;
            if let Some(index) = node.as_virtual()
                && !dirty.contains(&index)
            {
                check(cx, Kind::DecodedBytes, 128)?;
                dirty.insert(index);
            }
        }
        check(
            cx,
            Kind::DecodedBytes,
            (self.symbol_bounds.len() as u64)
                .saturating_mul(160)
                .saturating_add(1024),
        )?;
        for key in self.symbol_bounds.keys() {
            check(cx, Kind::Work, 1)?;
            check(cx, Kind::DecodedBytes, key_bytes(key))?;
        }
        let mut next = self.symbol_bounds.clone();
        // 循環する定義も、開いた直後と同じ空の境界から上流順に作る。
        for &index in &dirty {
            check(cx, Kind::Work, 1)?;
            if let Some(template) = self.symbol_templates.get(index as usize) {
                next.remove(&template.key);
            }
        }
        check(
            cx,
            Kind::DecodedBytes,
            (dirty.len() as u64).saturating_mul(96),
        )?;
        let mut diffs = Vec::with_capacity(dirty.len());
        for index in ordered_symbol_indices(self.symbol_templates.len(), &self.dep, cx)? {
            check(cx, Kind::Work, 1)?;
            let Ok(raw) = u32::try_from(index) else {
                continue;
            };
            if !dirty.contains(&raw) {
                continue;
            }
            let Some(template) = self.symbol_templates.get(index) else {
                continue;
            };
            let Some(rows) = self.symbol_rows.get(index) else {
                continue;
            };
            let mut accumulated: Option<[f64; 4]> = None;
            for row in rows {
                check(cx, Kind::Work, 1)?;
                let placed = staged
                    .get(row)
                    .copied()
                    .unwrap_or_else(|| self.table.row(*row).map(|r| &r.placed));
                if let Some(Placed::Model(element)) = placed {
                    count_element_copy(element, cx, 1)?;
                    let bounds = model_shape_bounds(&element.shape, &next);
                    let [x0, y0, x1, y1] = bounds;
                    if x0 <= x1 && y0 <= y1 {
                        match &mut accumulated {
                            Some(current) => {
                                current[0] = current[0].min(x0);
                                current[1] = current[1].min(y0);
                                current[2] = current[2].max(x1);
                                current[3] = current[3].max(y1);
                            }
                            None => accumulated = Some(bounds),
                        }
                    }
                }
            }
            if let Some(bounds) = accumulated {
                check(
                    cx,
                    Kind::DecodedBytes,
                    key_bytes(&template.key).saturating_add(1152),
                )?;
                next.insert(template.key.clone(), bounds);
            }
            // 変更フラグの集合へ入れる分も、確定前に数える。
            check(cx, Kind::DecodedBytes, 128)?;
            diffs.push(SymbolBoundsDiff {
                index: raw,
                before: self.symbol_bounds.get(&template.key).copied(),
                after: accumulated,
            });
        }
        Ok(PreparedSymbols {
            next: Some(next),
            diffs,
            affected,
        })
    }

    pub(super) fn check_symbol_history(
        &self,
        committed: &CommittedTxn<SxfRow, SharedPatch>,
        backwards: bool,
        cx: &mut Cx<'_>,
    ) -> Result<(), Error> {
        for patch in committed.patches() {
            check(cx, Kind::Work, 1)?;
            for diff in &patch.0.symbol_bounds {
                check(cx, Kind::Work, 1)?;
                check(cx, Kind::DecodedBytes, 128)?;
                let value = if backwards { diff.before } else { diff.after };
                if value.is_some()
                    && let Some(template) = self.symbol_templates.get(diff.index as usize)
                {
                    check(
                        cx,
                        Kind::DecodedBytes,
                        key_bytes(&template.key).saturating_add(1152),
                    )?;
                }
            }
        }
        Ok(())
    }

    pub(super) fn restore_symbol_bounds(&mut self, diffs: &[SymbolBoundsDiff], backwards: bool) {
        for diff in diffs {
            self.dirty_symbols.insert(diff.index);
            if let Some(template) = self.symbol_templates.get(diff.index as usize) {
                match if backwards { diff.before } else { diff.after } {
                    Some(value) => {
                        self.symbol_bounds.insert(template.key.clone(), value);
                    }
                    None => {
                        self.symbol_bounds.remove(&template.key);
                    }
                }
            }
        }
    }
}
