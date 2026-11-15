//! 線種を初めて使う順。部分図・作図部品を定義順に読み、その後に用紙を読む。

use super::*;
use crate::drawing::Arg;
use zumen_drawing::{LineType, LineTypeId};
use zumen_live::{RowId, VersionedTable};

#[derive(Clone)]
enum Use {
    Row(RowId),
    Fixed(usize),
    Born,
}

#[derive(Clone)]
struct Fixed {
    code: i64,
    source: Ref,
}

#[derive(Clone, Default)]
pub(super) struct TypeOrder {
    uses: Vec<Use>,
    fixed: Vec<Fixed>,
    pub(super) sheet_insertion: usize,
}

struct Definition {
    flag: i64,
    members: Vec<usize>,
}

struct Plan<'a> {
    records: &'a [FeatureRecord],
    file: &'a SxfFile,
    owners: &'a BTreeMap<u64, (usize, bool)>,
    definitions: BTreeMap<&'a str, Definition>,
    composites: BTreeMap<usize, Vec<usize>>,
    sheet_records: &'a [usize],
}

impl TypeOrder {
    pub(super) fn build(
        records: &[FeatureRecord],
        file: &SxfFile,
        owners: &BTreeMap<u64, (usize, bool)>,
        sheet_records: &[usize],
        cx: &mut Cx<'_>,
    ) -> Result<Self, Error> {
        // 保留する添字・定義順・表の節点・再帰中の名前を先に見積もる。
        allocated(cx, (records.len() as u64).saturating_mul(160))?;
        let mut plan = Plan {
            records,
            file,
            owners,
            definitions: BTreeMap::new(),
            composites: BTreeMap::new(),
            sheet_records,
        };
        let mut pending = Vec::new();
        let mut sheet = Vec::new();
        let mut definitions = Vec::new();
        let mut born = false;
        for (index, record) in records.iter().enumerate() {
            inspected(record, cx)?;
            let is = |name: &str| record.name.eq_ignore_ascii_case(name);
            if [
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
            .any(|name| is(name))
            {
                continue;
            }
            if is("drawing_sheet") {
                sheet.append(&mut pending);
                if !born {
                    sheet.push(usize::MAX);
                    born = true;
                }
            } else if is("sfig_org") {
                let members = core::mem::take(&mut pending);
                if let Some(name) = text(record, 0)
                    && let Some(flag @ 1..=4) = integer(record, 1)
                    && !plan.definitions.contains_key(name)
                {
                    definitions.push(name);
                    plan.definitions.insert(name, Definition { flag, members });
                }
            } else if is("composite_curve_org") {
                work(cx, pending.len() as u64, &Ref::default())?;
                let (kept, members) = pending.into_iter().partition(|&at| {
                    records.get(at).is_some_and(|record| {
                        record.name.eq_ignore_ascii_case("composite_curve_org")
                    })
                });
                pending = kept;
                plan.composites.insert(index, members);
                pending.push(index);
            } else {
                pending.push(index);
            }
        }
        sheet.append(&mut pending);
        if !born {
            sheet.push(usize::MAX);
        }
        let mut stack = Vec::new();
        let mut out = Self::default();
        let mut sheet_position = 0;
        for name in definitions {
            let Some(definition) = plan.definitions.get(name) else {
                continue;
            };
            if definition.flag != 3 {
                stack.push(name);
                plan.members(&definition.members, &mut stack, &mut out, None, cx)?;
                stack.pop();
            }
        }
        plan.members(&sheet, &mut stack, &mut out, Some(&mut sheet_position), cx)?;
        Ok(out)
    }

    pub(super) fn candidates(&self) -> impl Iterator<Item = (i64, &Ref)> + '_ {
        self.fixed
            .iter()
            .filter_map(|site| missing_type(site.code).then_some((site.code, &site.source)))
    }

    pub(super) fn current(
        &self,
        table: &VersionedTable<SxfRow>,
        base_rows: u32,
        base_types: &[LineType],
        cx: &mut Cx<'_>,
    ) -> Result<(Vec<LineType>, TypeSelection), Error> {
        for line_type in base_types {
            let before = cx.budget.used(Kind::Work);
            zumen_drawing::Hash::line_type_checked(line_type, cx).map_err(|stop| {
                cx.report(stop.to_diagnostic());
                Error::Stopped(stop)
            })?;
            // ハッシュの走査は可変長の値を 1 KiB 以下の単位で数える。出所は別に数える。
            let bytes = cx
                .budget
                .used(Kind::Work)
                .saturating_sub(before)
                .saturating_mul(1024)
                .saturating_add(256)
                .saturating_add(line_type.source.id().map_or(0, |id| id.len() as u64));
            allocated(cx, bytes)?;
        }
        let mut types = base_types.to_vec();
        let mut selected = TypeSelection::default();
        let mut seen = BTreeSet::new();
        for line_type in base_types {
            work(cx, 1, &line_type.source)?;
            allocated(cx, 32)?;
            seen.insert(line_type.id.0);
        }
        for site in &self.uses {
            work(cx, 1, &Ref::default())?;
            match site {
                Use::Row(row) => {
                    if let Some(row) = table.row(*row) {
                        current_row(row, &mut types, &mut seen, &mut selected, cx)?;
                    }
                }
                Use::Fixed(index) => {
                    let Some(site) = self.fixed.get(*index) else {
                        continue;
                    };
                    current_type(
                        site.code,
                        &site.source,
                        &mut types,
                        &mut seen,
                        &mut selected,
                        cx,
                    )?;
                }
                Use::Born => {
                    for index in base_rows..table.len() as u32 {
                        work(cx, 1, &Ref::default())?;
                        if let Some(row) = table.row(RowId(index)) {
                            current_row(row, &mut types, &mut seen, &mut selected, cx)?;
                        }
                    }
                }
            }
        }
        Ok((types, selected))
    }

    pub(super) fn bytes(&self) -> usize {
        self.uses.capacity() * 16 + self.fixed.capacity() * 160
    }
}

impl<'a> Plan<'a> {
    fn members(
        &self,
        members: &[usize],
        stack: &mut Vec<&'a str>,
        out: &mut TypeOrder,
        mut sheet_position: Option<&mut usize>,
        cx: &mut Cx<'_>,
    ) -> Result<(), Error> {
        for &index in members {
            work(cx, 1, &Ref::default())?;
            if index == usize::MAX {
                allocated(cx, 32)?;
                out.uses.push(Use::Born);
                out.sheet_insertion = sheet_position.as_deref().copied().unwrap_or_default();
                continue;
            }
            let Some(record) = self.records.get(index) else {
                continue;
            };
            inspected(record, cx)?;
            let mut source = Ref::at_record(record.record as u64);
            if let Some(instance) = self
                .file
                .records()
                .get(record.record)
                .and_then(|record| record.id())
            {
                source = source.with_key(zumen_base::Key::Instance(instance));
            }
            if record.name.eq_ignore_ascii_case("sfig_locate") {
                if let Some(name) = text(record, 1)
                    && let Some(definition) = self.definitions.get(name)
                    && definition.flag == 3
                {
                    work(
                        cx,
                        (stack.len() as u64).saturating_add(name.len() as u64),
                        &source,
                    )?;
                    if stack.contains(&name) {
                        continue;
                    }
                    cx.enter().map_err(|stop| {
                        cx.report(stop.to_diagnostic().at(source));
                        Error::Stopped(stop)
                    })?;
                    stack.push(name);
                    let result = self.members(
                        &definition.members,
                        stack,
                        out,
                        sheet_position.as_deref_mut(),
                        cx,
                    );
                    stack.pop();
                    cx.leave();
                    result?;
                } else if let Some(position) = sheet_position.as_deref_mut()
                    && self.consume(record.record, position)
                    && let Some(name) = text(record, 1)
                    && let Some(definition) = self.definitions.get(name)
                    && matches!(definition.flag, 1 | 2)
                {
                    cx.enter().map_err(|stop| {
                        cx.report(stop.to_diagnostic().at(source));
                        Error::Stopped(stop)
                    })?;
                    stack.push(name);
                    let result = self.nested(&definition.members, stack, position, cx);
                    stack.pop();
                    cx.leave();
                    result?;
                }
            } else if record.name.eq_ignore_ascii_case("composite_curve_org") {
                let visible = record.args.len() < 4 || integer(record, 3) == Some(1);
                if visible {
                    allocated(cx, 352)?;
                    out.uses.push(Use::Fixed(out.fixed.len()));
                    out.fixed.push(Fixed {
                        code: integer(record, 1).unwrap_or_default(),
                        source,
                    });
                    if let Some(position) = sheet_position.as_deref_mut()
                        && let Some(members) = self.composites.get(&index)
                    {
                        for &member in members {
                            work(cx, 1, &Ref::default())?;
                            if let Some(record) = self.records.get(member) {
                                self.consume(record.record, position);
                            }
                        }
                    }
                }
            } else if let Some(&(index, _)) = self.owners.get(&(record.record as u64)) {
                allocated(cx, 32)?;
                out.uses.push(Use::Row(RowId(index as u32)));
                if let Some(position) = sheet_position.as_deref_mut() {
                    self.consume(record.record, position);
                }
            }
        }
        Ok(())
    }

    fn consume(&self, record: usize, position: &mut usize) -> bool {
        if self.sheet_records.get(*position) == Some(&record) {
            *position += 1;
            true
        } else {
            false
        }
    }

    fn nested(
        &self,
        members: &[usize],
        stack: &mut Vec<&'a str>,
        position: &mut usize,
        cx: &mut Cx<'_>,
    ) -> Result<(), Error> {
        for &index in members {
            let Some(record) = self.records.get(index) else {
                continue;
            };
            inspected(record, cx)?;
            if !record.name.eq_ignore_ascii_case("sfig_locate") {
                continue;
            }
            let Some(name) = text(record, 1) else {
                continue;
            };
            let Some(definition) = self.definitions.get(name) else {
                continue;
            };
            work(cx, stack.len() as u64, &Ref::default())?;
            if stack.contains(&name) || definition.flag == 4 {
                continue;
            }
            if definition.flag != 3 && !self.consume(record.record, position) {
                continue;
            }
            cx.enter().map_err(|stop| {
                cx.report(
                    stop.to_diagnostic()
                        .at(Ref::at_record(record.record as u64)),
                );
                Error::Stopped(stop)
            })?;
            stack.push(name);
            let result = self.nested(&definition.members, stack, position, cx);
            stack.pop();
            cx.leave();
            result?;
        }
        Ok(())
    }
}

#[derive(Default)]
pub(in crate::drawing::live) struct TypeSelection {
    selected: BTreeMap<u32, (i64, Ref)>,
    emitted: BTreeSet<u32>,
}

impl TypeSelection {
    pub(super) fn accepts(&mut self, code: i64, event: &Diagnostic) -> bool {
        let key = u32::try_from(code).unwrap_or_default();
        self.selected.get(&key).is_some_and(|(wanted, source)| {
            *wanted == code && event.places().first() == Some(source)
        }) && self.emitted.insert(key)
    }
}

fn current_row(
    row: &SxfRow,
    types: &mut Vec<LineType>,
    seen: &mut BTreeSet<u32>,
    selected: &mut TypeSelection,
    cx: &mut Cx<'_>,
) -> Result<(), Error> {
    let Inherit::Own(id) = placed_style(&row.placed).line_type else {
        return Ok(());
    };
    work(cx, row.diagnostics.len() as u64, placed_source(&row.placed))?;
    let code = row
        .diagnostics
        .iter()
        .find_map(line_type_code)
        .unwrap_or(i64::from(id.0));
    current_type(code, placed_source(&row.placed), types, seen, selected, cx)
}

fn current_type(
    code: i64,
    source: &Ref,
    types: &mut Vec<LineType>,
    seen: &mut BTreeSet<u32>,
    selected: &mut TypeSelection,
    cx: &mut Cx<'_>,
) -> Result<(), Error> {
    work(
        cx,
        1_u64.saturating_add(source.id().map_or(0, |id| (id.len() as u64).div_ceil(1024))),
        source,
    )?;
    let id = u32::try_from(code).unwrap_or_default();
    if seen.contains(&id) {
        return Ok(());
    }
    allocated(
        cx,
        512_u64.saturating_add(source.id().map_or(0, |id| id.len() as u64)),
    )?;
    seen.insert(id);
    let name = usize::try_from(code.saturating_sub(1))
        .ok()
        .and_then(|i| LINE_TYPES.get(i));
    if name.is_none() {
        selected.selected.insert(id, (code, source.clone()));
    }
    types.push(LineType {
        id: LineTypeId(id),
        name: name.map_or_else(|| code.to_string(), |name| (*name).to_owned()),
        definition: None,
        source: Ref::default(),
    });
    Ok(())
}

fn missing_type(code: i64) -> bool {
    usize::try_from(code.saturating_sub(1))
        .ok()
        .and_then(|i| LINE_TYPES.get(i))
        .is_none()
}

fn text(record: &FeatureRecord, index: usize) -> Option<&str> {
    match record.args.get(index)? {
        Arg::Text(text) => Some(text),
        _ => None,
    }
}

fn integer(record: &FeatureRecord, index: usize) -> Option<i64> {
    let value = crate::drawing::number(text(record, index)?.trim())?;
    (value.fract() == 0.0 && value.abs() < 9.0e15).then_some(value as i64)
}

fn inspected(record: &FeatureRecord, cx: &mut Cx<'_>) -> Result<(), Error> {
    let bytes = (0..4)
        .filter_map(|index| text(record, index))
        .fold(record.name.len() as u64, |sum, text| {
            sum.saturating_add(text.len() as u64)
        });
    work(
        cx,
        1_u64.saturating_add(bytes.div_ceil(1024)),
        &Ref::at_record(record.record as u64),
    )
}
