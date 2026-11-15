//! 未集約の変換診断を保持し、保存順の場所で再報告する。

use std::collections::{BTreeMap, BTreeSet};

use zumen_base::{Cx, Diagnostic, Kind, Ref, Value, codes};
use zumen_sdk::Error;

use super::{SourceMap, SxfBacking, SxfRow, placed_source, placed_style};
use crate::SxfFile;
use crate::drawing::{Args, FeatureRecord, LINE_TYPES, LifterTables, work};
use zumen_drawing::Inherit;

// 予算と履歴の概算は、32 bit と 64 bit で同じ上限を使う。
const DIAGNOSTIC_BYTES: usize = 256;
const PLACE_BYTES: usize = 128;
const PARAMETER_BYTES: usize = 64;

#[derive(Clone, Default)]
pub(super) struct LiveDiagnostics {
    fixed: Vec<Diagnostic>,
    tail: Vec<Ref>,
}

impl LiveDiagnostics {
    pub(super) fn build(
        events: Vec<Diagnostic>,
        records: &[FeatureRecord],
        file: &SxfFile,
        rows: &mut [SxfRow],
        tables: &LifterTables,
        cx: &mut Cx<'_>,
    ) -> Result<Self, Error> {
        let mut out = Self::default();
        let mut owners = BTreeMap::new();
        for (index, row) in rows.iter().enumerate() {
            work(cx, 1, placed_source(&row.placed))?;
            allocated(cx, 64)?;
            owners
                .entry(row.record_idx as u64)
                .and_modify(|owner| *owner = None)
                .or_insert(Some(index));
        }
        for event in events {
            work(cx, 1, &Ref::default())?;
            match event.code() {
                // SAF の対応は現在の拡張から、drawing のたびに判定する。
                code if code == codes::SXF_SAF_FIGURE_MISSING
                    || code == codes::SXF_SAF_FIGURE_DROPPED =>
                {
                    continue;
                }
                code if code == codes::SXF_MEMBER_ASSUMED => {
                    if let Some(first) = event.places().first().and_then(Ref::record) {
                        out.tail = tail_sources(first, records, file, cx)?;
                    }
                    continue;
                }
                _ => {}
            }
            let owner = event
                .places()
                .first()
                .and_then(Ref::record)
                .and_then(|record| owners.get(&record).copied().flatten());
            let target = if !event.code().name().starts_with("sxf.p21.")
                && let Some(row) = owner.and_then(|index| rows.get_mut(index))
            {
                &mut row.diagnostics
            } else {
                &mut out.fixed
            };
            allocated(cx, (DIAGNOSTIC_BYTES * 2) as u64)?;
            target.push(event);
        }
        for record in records {
            work(cx, 1, &Ref::default())?;
            let Some(row) = owners
                .get(&(record.record as u64))
                .copied()
                .flatten()
                .and_then(|index| rows.get_mut(index))
            else {
                continue;
            };
            // 複合曲線の構成要素は定義側の線種を使う。単独の要素だけ候補を補う。
            if row.grouped {
                continue;
            }
            let Inherit::Own(line_type) = placed_style(&row.placed).line_type else {
                continue;
            };
            work(
                cx,
                tables
                    .base_line_types
                    .len()
                    .saturating_add(row.diagnostics.len()) as u64,
                &Ref::default(),
            )?;
            if tables
                .base_line_types
                .iter()
                .any(|defined| defined.id == line_type)
            {
                continue;
            }
            let code = Args {
                values: &record.args,
            }
            .int(2)
            .unwrap_or_default();
            if usize::try_from(code.saturating_sub(1))
                .ok()
                .and_then(|index| LINE_TYPES.get(index))
                .is_some()
                || row
                    .diagnostics
                    .iter()
                    .any(|event| line_type_code(event).is_some())
            {
                continue;
            }
            // 線種の未定義は最初の使用でしか報告されないため、次の使用の候補も残す。
            let event = Diagnostic::new(codes::SXF_CODE_MISSING)
                .with("kind", "line type")
                .with("code", code)
                .at(placed_source(&row.placed).clone());
            allocated(cx, heap_bytes(core::slice::from_ref(&event)) as u64 * 2)?;
            let position = row.diagnostics.iter().position(|event| matches!(event.param("kind"), Some(Value::Text(kind)) if kind == "width"))
                .unwrap_or(row.diagnostics.len());
            row.diagnostics.insert(position, event);
        }
        Ok(out)
    }

    pub(super) fn report(
        &self,
        backing: &SxfBacking,
        sources: &SourceMap,
        cx: &mut Cx<'_>,
    ) -> Result<BTreeSet<u32>, Error> {
        let mut line_types = BTreeSet::new();
        for event in &self.fixed {
            if let SxfBacking::P21(state) = backing
                && !state.diagnostic_current(event, cx)?
            {
                continue;
            }
            report(event, sources, &mut line_types, cx)?;
        }
        for source in &self.tail {
            work(cx, 1, source)?;
            if !sources.removed(source) {
                let mut source = source.clone();
                sources.remap(&mut source, cx)?;
                cx.report(Diagnostic::new(codes::SXF_MEMBER_ASSUMED).at(source));
                break;
            }
        }
        Ok(line_types)
    }

    pub(super) fn bytes(&self) -> usize {
        heap_bytes(&self.fixed) + self.tail.capacity() * PLACE_BYTES
    }
}

fn tail_sources(
    first: u64,
    records: &[FeatureRecord],
    file: &SxfFile,
    cx: &mut Cx<'_>,
) -> Result<Vec<Ref>, Error> {
    // Lifter の末尾の pending は、用紙・部分図・複合曲線が回収した後の列である。
    // 最初の診断位置より前のフィーチャは含めず、表のレコードも飛ばす。
    let mut found = false;
    let mut out = Vec::new();
    for record in records {
        work(cx, 1, &Ref::default())?;
        found |= record.record as u64 == first;
        if !found
            || [
                "layer",
                "user_defined_colour",
                "user_defined_font",
                "width",
                "text_font",
                "pre_defined_colour",
                "pre_defined_font",
                "drawing_attribute",
            ]
            .iter()
            .any(|name| record.name.eq_ignore_ascii_case(name))
        {
            continue;
        }
        allocated(cx, (PLACE_BYTES * 2) as u64)?;
        let mut source = Ref::at_record(record.record as u64);
        if let Some(instance) = file
            .records()
            .get(record.record)
            .and_then(|entry| entry.id())
        {
            source = source.with_key(zumen_base::Key::Instance(instance));
        }
        out.push(source);
    }
    Ok(out)
}

pub(super) fn report(
    event: &Diagnostic,
    sources: &SourceMap,
    line_types: &mut BTreeSet<u32>,
    cx: &mut Cx<'_>,
) -> Result<(), Error> {
    work(
        cx,
        1_u64
            .saturating_add(event.params().len() as u64)
            .saturating_add(event.places().len() as u64),
        &Ref::default(),
    )?;
    if event
        .places()
        .first()
        .is_some_and(|source| sources.removed(source))
    {
        return Ok(());
    }
    if let Some(code) = line_type_code(event) {
        allocated(cx, 32)?;
        if !line_types.insert(u32::try_from(code).unwrap_or_default()) {
            return Ok(());
        }
    }
    // 未集約の報告は通常 1 件。集約済みの入力でも件数と場所を重複させない。
    for count in 0..event.count() {
        work(cx, 1, &Ref::default())?;
        let bytes = heap_bytes(core::slice::from_ref(event)) as u64;
        work(cx, 1 + bytes / 1024, &Ref::default())?;
        allocated(cx, bytes)?;
        let mut mapped = Diagnostic::new(event.code());
        for (key, value) in event.params() {
            mapped = mapped.with(key, value.clone());
        }
        if let Some(spec) = event.spec() {
            mapped = mapped.cite(spec);
        }
        if count == 0 {
            for source in event.places() {
                let mut source = source.clone();
                sources.remap(&mut source, cx)?;
                mapped = mapped.at(source);
            }
        }
        cx.report(mapped);
    }
    Ok(())
}

pub(super) fn heap_bytes(events: &[Diagnostic]) -> usize {
    events.iter().fold(
        events.len().saturating_mul(DIAGNOSTIC_BYTES),
        |sum, event| {
            sum.saturating_add(event.places().len().saturating_mul(PLACE_BYTES))
                .saturating_add(event.params().len().saturating_mul(PARAMETER_BYTES))
                .saturating_add(
                    event
                        .places()
                        .iter()
                        .filter_map(Ref::id)
                        .map(str::len)
                        .sum::<usize>(),
                )
                .saturating_add(
                    event
                        .params()
                        .iter()
                        .map(|(_, value)| match value {
                            Value::Text(text) => text.len(),
                            _ => 0,
                        })
                        .sum::<usize>(),
                )
        },
    )
}

fn allocated(cx: &mut Cx<'_>, bytes: u64) -> Result<(), Error> {
    cx.check(Kind::DecodedBytes, bytes).map_err(|stop| {
        cx.report(stop.to_diagnostic());
        Error::Stopped(stop)
    })
}

fn line_type_code(event: &Diagnostic) -> Option<i64> {
    if event.code() != codes::SXF_CODE_MISSING
        || !matches!(event.param("kind"), Some(Value::Text(kind)) if kind == "line type")
    {
        return None;
    }
    match event.param("code") {
        Some(Value::Int(code)) => Some(*code),
        _ => None,
    }
}
