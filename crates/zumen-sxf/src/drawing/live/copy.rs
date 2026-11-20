//! 用紙に写す要素の前提を確かめ、SFC のフィーチャか P21 の実体を変更に足す。

use super::*;
use crate::drawing::FeatureRecord;
use crate::drawing::apply::{copy as batch, symbol_arguments};

pub(super) fn extensions_match(drafts: &[Draft], expected: &[batch::ExpectedExt]) -> bool {
    expected.iter().all(|(ns, fields)| {
        drafts.iter().any(|draft| {
            draft.namespace.as_ref() == ns && batch::fields_match(&draft.fields, fields)
        })
    })
}

#[derive(Clone, Copy)]
pub(super) enum Place<'a> {
    OriginalSheet,
    Other,
    Missing(&'a str),
}

impl SxfLive {
    pub(super) fn copy_place<'a>(
        &self,
        place: Option<&'a str>,
        cx: &mut Cx<'_>,
    ) -> Result<Place<'a>, Error> {
        let Some(id) = place else {
            return Ok(Place::OriginalSheet);
        };
        if self.sheet_name_map.contains_key(id) {
            return Ok(Place::OriginalSheet);
        }
        if let Some(n) = id
            .strip_prefix("space/")
            .and_then(|s| s.parse::<u32>().ok())
            && id == zumen_drawing::id::space(SpaceId(n))
        {
            work(cx, self.symbol_templates.len() as u64 + 1, &Ref::default())?;
            if self.space_spatial.contains_key(&SpaceId(n))
                || self
                    .symbol_templates
                    .iter()
                    .any(|s| s.space_id == SpaceId(n))
            {
                return Ok(Place::Other);
            }
        }
        Ok(Place::Missing(id))
    }

    fn copy_refusal(
        &self,
        row: &SxfRow,
        edit: usize,
        (field, value): batch::Refusal,
        cx: &mut Cx,
    ) -> Result<(), Error> {
        let kind = self.row_p21_kind(row);
        self.report_at(
            Diagnostic::new(codes::CHANGE_COPY_UNSUPPORTED)
                .with("edit", edit as u64)
                .with("kind", if kind.is_empty() { "element" } else { kind })
                .with("field", field)
                .with("value", value)
                .with("format", self.backing.format_name()),
            placed_source(&row.placed),
            cx,
        )
    }

    fn copy_source(
        &self,
        target: &Seen,
        edit: usize,
        place: Place<'_>,
        targets: &BTreeSet<u64>,
        cx: &mut Cx,
    ) -> Result<Option<&SxfRow>, Error> {
        let Some((rid, row)) = self.lookup_seen(target, cx)? else {
            return Ok(None);
        };
        work(cx, row.drafts.len() as u64, placed_source(&row.placed))?;
        let refusal = if let Some(inst) = row
            .unique_handle
            .filter(|&inst| self.table.find_handle(inst) == Some(rid))
        {
            if targets.contains(&inst) {
                Some(("source", "changed"))
            } else if row.grouped {
                Some(("reference", "composite_curve"))
            } else if let Some(attr) = row.drafts.iter().find(|d| d.namespace == "sxf.attribute") {
                Some(("extensions", batch::attribute_kind(&attr.fields)))
            } else if let Place::Missing(id) = place {
                cx.report(Diagnostic::new(codes::CHANGE_ELEMENT_MISSING).with("id", id));
                return Ok(None);
            } else if matches!(place, Place::Other) || !matches!(row.placed, Placed::Paper(_)) {
                Some(("place", "unsupported"))
            } else {
                None
            }
        } else {
            Some(("source", "shared"))
        };
        if let Some(reason) = refusal {
            self.copy_refusal(row, edit, reason, cx)?;
            return Ok(None);
        }
        Ok(Some(row))
    }

    #[allow(clippy::too_many_arguments)]
    fn prepare_copy(
        &self,
        row: &SxfRow,
        transform: &Affine,
        original: Option<&FeatureRecord>,
        tables: &Tables,
        fonts: &mut Fonts<'_>,
        writer: &mut dyn Writer,
        cx: &mut Cx,
    ) -> Result<Result<batch::Prepared, batch::Refusal>, Error> {
        count_placed_copy(&row.placed, cx, 2)?;
        let Some(Placed::Paper(moved)) = transformed_placed(&row.placed, transform) else {
            return Ok(Err(("transform", "unsupported")));
        };
        work(cx, tables.style_work(&moved.style, None), &Ref::default())?;
        if let Shape::Symbol(symbol) = &moved.shape {
            if !self.symbol_indices.contains_key(&symbol.key) {
                return Ok(Err(("symbol", "missing")));
            }
            return Ok(symbol_arguments(&moved, tables, cx)?
                .map(|args| (args, moved, Vec::new()))
                .ok_or(("kind", "unsupported")));
        }
        if self.row_p21_name(row) == text::NAME && original.is_none() {
            return Ok(Err(("source", "missing")));
        }
        batch::prepare_paper(tables, &moved, original, false, fonts, writer, cx)
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn stage_sfc_copy(
        &self,
        sfc: &SfcState,
        tx: &mut SfcStepTx,
        tables: &Tables,
        target: &Seen,
        transform: &Affine,
        edit: usize,
        place: Place<'_>,
        targets: &BTreeSet<u64>,
        stage: &mut TableStage,
        cx: &mut Cx,
    ) -> Result<Option<StagedSfcOp>, Error> {
        let Some(row) = self.copy_source(target, edit, place, targets, cx)? else {
            return Ok(None);
        };
        let original = if self.row_p21_name(row) == text::NAME
            && let Some(slot) = row.sfc_slot
        {
            sfc.parse_staged_feature(tx, slot, cx)?
                .map(|(_, record)| record)
        } else {
            None
        };
        let prepared = self.prepare_copy(
            row,
            transform,
            original.as_ref(),
            tables,
            &mut stage.fonts,
            &mut LiveSfcWriter { sfc, tx },
            cx,
        )?;
        let staged = prepared.and_then(|(args, expected, extensions)| {
            let (seq, inst) = sfc.stage_add(tx, &args).ok_or(("kind", "unsupported"))?;
            Ok(StagedSfcOp::Added {
                seq,
                inst,
                sheet_id: expected.shape_sheet(),
                place_name: target
                    .id
                    .rsplit_once('/')
                    .ok_or(("place", "unsupported"))?
                    .0
                    .to_owned(),
                expected: Box::new(Placed::Paper(expected)),
                extensions,
            })
        });
        match staged {
            Ok(op) => Ok(Some(op)),
            Err(reason) => {
                self.copy_refusal(row, edit, reason, cx)?;
                Ok(None)
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn stage_p21_copy(
        &self,
        p21: &P21State,
        tx: &mut P21StepTx,
        tables: &Tables,
        target: &Seen,
        transform: &Affine,
        edit: usize,
        place: Place<'_>,
        targets: &BTreeSet<u64>,
        stage: &mut TableStage,
        cx: &mut Cx,
    ) -> Result<Option<StagedP21Op>, Error> {
        let Some(row) = self.copy_source(target, edit, place, targets, cx)? else {
            return Ok(None);
        };
        let original = if self.row_p21_name(row) == text::NAME
            && let Some(inst) = row.unique_handle
        {
            p21.parse_staged_occurrence(tx, inst, &tables.widths, row.record_idx, cx)?
        } else {
            None
        };
        let prepared = self.prepare_copy(
            row,
            transform,
            original.as_ref(),
            tables,
            &mut stage.fonts,
            &mut LiveP21Writer {
                p21,
                tx,
                widths: &tables.widths,
            },
            cx,
        )?;
        let (args, expected, extensions) = match prepared {
            Ok(result) => result,
            Err(reason) => {
                self.copy_refusal(row, edit, reason, cx)?;
                return Ok(None);
            }
        };
        let inst = if let Shape::Symbol(symbol) = &expected.shape {
            let definition = self
                .symbol_indices
                .get(&symbol.key)
                .and_then(|i| self.symbol_templates.get(*i as usize))
                .ok_or(Error::Rejected)?;
            p21.stage_symbol(tx, &args, &definition.source, cx)?
        } else {
            p21.stage_add(tx, &tables.widths, &args, cx)?
        };
        let Some(inst) = inst else {
            self.copy_refusal(row, edit, ("kind", "unsupported"), cx)?;
            return Ok(None);
        };
        Ok(Some(StagedP21Op::Added {
            inst,
            place_name: target
                .id
                .rsplit_once('/')
                .ok_or(Error::Rejected)?
                .0
                .to_owned(),
            sheet_id: expected.shape_sheet(),
            add_name: args.name,
            expected: Box::new(Placed::Paper(expected)),
            extensions,
        }))
    }
}
